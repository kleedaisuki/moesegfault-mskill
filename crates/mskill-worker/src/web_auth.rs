//! Same-origin confidential OIDC BFF; OAuth credentials never enter browser storage.
use crate::{
    auth::{self, Principal},
    telemetry::random_hex,
    ApiError, ApiResult,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use worker::{Env, Fetch, Method, Request, RequestInit, RequestRedirect, Response, Url};

/// Cryptographic exceptions are sanitized; key material never becomes an API detail.
impl From<JsValue> for ApiError {
    fn from(_: JsValue) -> Self {
        ApiError::new(
            503,
            "web_crypto_unavailable",
            "Sign-in is temporarily unavailable.",
        )
    }
}

/// Bind ciphertext to its session and field to prevent row/column substitution.
async fn crypt(
    env: &Env,
    session: &str,
    field: &str,
    value: &str,
    seal: bool,
) -> ApiResult<String> {
    let secret = URL_SAFE_NO_PAD
        .decode(config(env, "WEB_SESSION_KEY")?)
        .map_err(|_| denied())?;
    if secret.len() != 32 {
        return Err(denied());
    }
    let crypto: web_sys::Crypto = js_sys::Reflect::get(&js_sys::global(), &"crypto".into())?
        .dyn_into()
        .map_err(|_| denied())?;
    let usages = js_sys::Array::new();
    usages.push(&"encrypt".into());
    usages.push(&"decrypt".into());
    let bytes = js_sys::Uint8Array::from(secret.as_slice());
    let key = JsFuture::from(crypto.subtle().import_key_with_str(
        "raw",
        bytes.as_ref(),
        "AES-GCM",
        false,
        &usages,
    )?)
    .await?
    .dyn_into::<web_sys::CryptoKey>()
    .map_err(|_| denied())?;
    let (iv, input) = if seal {
        let mut iv = vec![0; 12];
        crypto.get_random_values_with_u8_array(&mut iv)?;
        (iv, value.as_bytes().to_vec())
    } else {
        let parts: Vec<_> = value.split('.').collect();
        if parts.len() != 3 || parts[0] != "v1" {
            return Err(denied());
        }
        (
            URL_SAFE_NO_PAD.decode(parts[1]).map_err(|_| denied())?,
            URL_SAFE_NO_PAD.decode(parts[2]).map_err(|_| denied())?,
        )
    };
    if iv.len() != 12 {
        return Err(denied());
    }
    let algorithm = js_sys::Object::new();
    js_sys::Reflect::set(&algorithm, &"name".into(), &"AES-GCM".into())?;
    js_sys::Reflect::set(
        &algorithm,
        &"iv".into(),
        &js_sys::Uint8Array::from(iv.as_slice()),
    )?;
    js_sys::Reflect::set(
        &algorithm,
        &"additionalData".into(),
        &js_sys::Uint8Array::from(format!("mskill:{session}:{field}").as_bytes()),
    )?;
    let promise = if seal {
        crypto
            .subtle()
            .encrypt_with_object_and_u8_array(&algorithm, &key, &input)?
    } else {
        crypto
            .subtle()
            .decrypt_with_object_and_u8_array(&algorithm, &key, &input)?
    };
    let result = js_sys::Uint8Array::new(&JsFuture::from(promise).await?).to_vec();
    if seal {
        Ok(format!(
            "v1.{}.{}",
            URL_SAFE_NO_PAD.encode(iv),
            URL_SAFE_NO_PAD.encode(result)
        ))
    } else {
        String::from_utf8(result).map_err(|_| denied())
    }
}

/// Login and refresh state use Unix seconds, independently of metadata timestamps.
fn now() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}
/// Configuration never falls back across an environment boundary.
fn config(env: &Env, key: &str) -> ApiResult<String> {
    env.var(key)
        .map(|v| v.to_string())
        .or_else(|_| env.secret(key).map(|v| v.to_string()))
        .map_err(|_| {
            ApiError::new(
                503,
                "web_login_unavailable",
                "Browser sign-in is temporarily unavailable.",
            )
        })
}
/// Raw cookies and OAuth values never appear in public diagnostics.
fn denied() -> ApiError {
    ApiError::new(401, "session_required", "Sign in to continue.")
}
/// Session identifiers are stored only as one-way hashes.
fn hash(s: &str) -> String {
    mskill_protocol::sha256_hex(s.as_bytes())
}
/// Reject open redirects and browser URL parser backslash normalization.
fn safe_return(s: &str) -> bool {
    s.starts_with('/')
        && !s.starts_with("//")
        && !s.contains('\\')
        && !s.chars().any(char::is_control)
        && s.len() <= 2048
}
/// Cookie parsing accepts exactly one occurrence, preventing duplicate-cookie ambiguity.
fn cookie(req: &Request, name: &str) -> ApiResult<Option<String>> {
    let raw = req.headers().get("Cookie")?.unwrap_or_default();
    let values: Vec<_> = raw
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .filter(|(k, _)| *k == name)
        .map(|(_, v)| v.to_owned())
        .collect();
    if values.len() > 1 {
        return Err(denied());
    }
    Ok(values.into_iter().next())
}
/// Only explicit local loopback requests may omit Secure and the __Host prefix.
fn local(req: &Request, env: &Env) -> bool {
    env.var("ENVIRONMENT")
        .ok()
        .is_some_and(|v| v.to_string() == "local")
        && req
            .url()
            .ok()
            .is_some_and(|u| matches!(u.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")))
}
/// Same-origin production cookies are host-only and cannot be set by subdomains.
fn cookie_name(req: &Request, env: &Env, kind: &str) -> String {
    format!(
        "{}mskill-{kind}",
        if local(req, env) { "" } else { "__Host-" }
    )
}
/// All authentication responses are private, noncacheable, and cannot leak callback URLs.
fn finish(
    mut r: Response,
    req: &Request,
    env: &Env,
    kind: &str,
    value: &str,
    age: u64,
) -> ApiResult<Response> {
    r.headers_mut().set("Cache-Control", "no-store")?;
    r.headers_mut().set("Referrer-Policy", "no-referrer")?;
    r.headers_mut().append(
        "Set-Cookie",
        &format!(
            "{}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={age}{}",
            cookie_name(req, env, kind),
            if local(req, env) { "" } else { "; Secure" }
        ),
    )?;
    Ok(r)
}
/// Redirects use validated deployment endpoints or validated relative return paths.
fn redirect(location: &str) -> ApiResult<Response> {
    let mut r = Response::empty()?.with_status(303);
    r.headers_mut().set("Location", location)?;
    Ok(r)
}
/// Trusted discovery endpoints cannot redirect or point to a different origin.
async fn discovery(req: &Request, env: &Env) -> ApiResult<Value> {
    let issuer = config(env, "IDENTITY_ISSUER")?;
    let value = fetch(
        req,
        &format!("{issuer}/.well-known/openid-configuration"),
        None,
    )
    .await?;
    if value["issuer"].as_str() != Some(&issuer) {
        return Err(ApiError::new(
            503,
            "identity_discovery_invalid",
            "Sign-in configuration is unavailable.",
        ));
    }
    for key in [
        "authorization_endpoint",
        "token_endpoint",
        "revocation_endpoint",
        "end_session_endpoint",
    ] {
        let u = Url::parse(value[key].as_str().unwrap_or("")).map_err(|_| denied())?;
        let base = Url::parse(&issuer).map_err(|_| denied())?;
        if u.origin() != base.origin()
            || !u.username().is_empty()
            || u.password().is_some()
            || u.fragment().is_some()
            || (u.scheme() != "https" && !local(req, env))
        {
            return Err(denied());
        }
    }
    Ok(value)
}
/// The server consumes bounded JSON only, and carries the command trace to Identity.
async fn fetch(req: &Request, url: &str, body: Option<String>) -> ApiResult<Value> {
    let mut init = RequestInit::new();
    init.redirect = RequestRedirect::Manual;
    if let Some(body) = body {
        init.method = Method::Post;
        init.body = Some(body.into());
        init.headers
            .set("Content-Type", "application/x-www-form-urlencoded")?;
    }
    if let Some(trace) = req.headers().get("traceparent")? {
        init.headers.set("traceparent", &trace)?;
    }
    let mut r = Fetch::Request(Request::new_with_init(url, &init)?)
        .send()
        .await?;
    if r.status_code() != 200 {
        return Err(ApiError::new(
            503,
            "identity_exchange_failed",
            "Sign-in could not be completed. Try again.",
        ));
    }
    let mut stream = r.stream()?;
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if bytes.len() + chunk.len() > 65536 {
            return Err(denied());
        }
        bytes.extend_from_slice(&chunk);
    }
    let text = String::from_utf8(bytes).map_err(|_| denied())?;
    if text.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&text).map_err(|_| denied())
}
/// Create a fresh registered RS256 private_key_jwt with a nonreused assertion ID.
async fn assertion(env: &Env, audience: &str) -> ApiResult<String> {
    let key: Value =
        serde_json::from_str(&config(env, "WEB_CLIENT_PRIVATE_JWK")?).map_err(|_| denied())?;
    let kid = key["kid"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| config(env, "WEB_CLIENT_KEY_ID").ok())
        .ok_or_else(denied)?;
    if key["kty"] != "RSA" || key["d"].as_str().is_none() {
        return Err(denied());
    }
    let client = config(env, "WEB_CLIENT_ID")?;
    let t = now();
    let header = URL_SAFE_NO_PAD.encode(json!({"alg":"RS256","typ":"JWT","kid":kid}).to_string());
    let payload = URL_SAFE_NO_PAD.encode(
        json!({"iss":client,"sub":client,"aud":audience,"iat":t,"exp":t+120,"jti":random_hex(32)?})
            .to_string(),
    );
    let input = format!("{header}.{payload}");
    let crypto: web_sys::Crypto = js_sys::Reflect::get(&js_sys::global(), &"crypto".into())?
        .dyn_into()
        .map_err(|_| denied())?;
    let algorithm = js_sys::JSON::parse(r#"{"name":"RSASSA-PKCS1-v1_5","hash":"SHA-256"}"#)?
        .dyn_into::<js_sys::Object>()
        .map_err(|_| denied())?;
    let jwk = js_sys::JSON::parse(&key.to_string())?
        .dyn_into::<js_sys::Object>()
        .map_err(|_| denied())?;
    let usages = js_sys::Array::new();
    usages.push(&"sign".into());
    let imported = JsFuture::from(
        crypto
            .subtle()
            .import_key_with_object("jwk", &jwk, &algorithm, false, &usages)?,
    )
    .await?
    .dyn_into::<web_sys::CryptoKey>()
    .map_err(|_| denied())?;
    let signature = JsFuture::from(crypto.subtle().sign_with_str_and_u8_array(
        "RSASSA-PKCS1-v1_5",
        &imported,
        input.as_bytes(),
    )?)
    .await?;
    Ok(format!(
        "{input}.{}",
        URL_SAFE_NO_PAD.encode(js_sys::Uint8Array::new(&signature).to_vec())
    ))
}
/// Form encoding is performed by the platform URL implementation.
async fn exchange(
    req: &Request,
    env: &Env,
    metadata: &Value,
    fields: &[(&str, String)],
    revoke: bool,
) -> ApiResult<Value> {
    let endpoint = metadata["token_endpoint"].as_str().ok_or_else(denied)?;
    let mut url = Url::parse("https://form.invalid/").map_err(|_| denied())?;
    {
        let mut p = url.query_pairs_mut();
        for (k, v) in fields {
            p.append_pair(k, v);
        }
        p.append_pair("client_id", &config(env, "WEB_CLIENT_ID")?);
        p.append_pair(
            "client_assertion_type",
            "urn:ietf:params:oauth:client-assertion-type:jwt-bearer",
        );
        p.append_pair("client_assertion", &assertion(env, endpoint).await?);
    }
    fetch(
        req,
        if revoke {
            metadata["revocation_endpoint"]
                .as_str()
                .ok_or_else(denied)?
        } else {
            endpoint
        },
        Some(url.query().unwrap_or_default().into()),
    )
    .await
}
/// A server-only record stores tokens and a synchronizing refresh lease.
#[derive(Deserialize)]
struct Session {
    /// Hash of the high-entropy browser cookie; no raw cookie is persisted.
    session_hash: String,
    /// Exact pinned issuer of the verified account.
    issuer: String,
    /// Stable Identity subject, never email or a mutable username.
    subject: String,
    /// Presentation-only value, not an authorization key.
    display_name: Option<String>,
    /// Independent random synchronizer token for cookie-authorized mutations.
    csrf: String,
    /// AES-GCM access-token envelope, never plaintext in D1.
    access_token: String,
    /// AES-GCM original ID-token envelope used only as a logout hint.
    id_token: String,
    /// AES-GCM rotating refresh-token envelope.
    refresh_token: String,
    /// Access-token expiry hint in Unix seconds; session refresh starts early.
    access_expires_at: u64,
    /// Absolute non-sliding local-session expiration.
    expires_at: u64,
    /// Random claimant for the current durable rotation lease.
    refresh_lock: Option<String>,
    /// Lease expiry; abandonment terminates rather than replays the token family.
    refresh_lock_until: u64,
}
/// A callback consumes exactly the transaction bound to this browser cookie.
#[derive(Deserialize)]
struct Transaction {
    /// Random original login nonce for exact ID-token transaction binding.
    nonce: String,
    /// Original S256 PKCE verifier, discarded on callback consumption.
    verifier: String,
    /// Previously validated local path; never an OAuth redirect URI.
    return_to: String,
}
/// Issuance and authenticated reads accept only the registered workspace origin.
fn origin(req: &Request, env: &Env) -> ApiResult<()> {
    let configured = config(env, "WEB_ORIGIN")?;
    let u = Url::parse(&configured).map_err(|_| denied())?;
    if u.as_str().trim_end_matches('/') != configured
        || u.path() != "/"
        || u.query().is_some()
        || u.fragment().is_some()
        || !u.username().is_empty()
        || u.password().is_some()
        || (u.scheme() != "https" && !local(req, env))
        || req.url()?.origin() != u.origin()
    {
        return Err(ApiError::new(
            400,
            "invalid_web_origin",
            "Open the registered workspace to sign in.",
        ));
    }
    Ok(())
}

/// Route only the browser authentication surface; existing native API stays separate.
pub(crate) async fn route(req: &Request, env: &Env) -> Option<ApiResult<Response>> {
    if req.path().starts_with("/auth/") {
        if let Err(e) = origin(req, env) {
            return Some(Err(e));
        }
    }
    match (req.method(), req.path().as_str()) {
        (Method::Get, "/auth/login") => Some(login(req, env).await),
        (Method::Get, "/auth/callback") => Some(callback(req, env).await),
        (Method::Get, "/web/session") => Some(profile(req, env).await),
        (Method::Post, "/auth/logout") => Some(logout(req, env).await),
        (Method::Get, "/auth/logout/identity") => Some(logout_identity(req, env).await),
        (Method::Get, "/auth/logout/callback") => Some(logout_callback(req, env)),
        _ => None,
    }
}
/// Persist PKCE and nonce before navigating to the provider, never in browser storage.
async fn login(req: &Request, env: &Env) -> ApiResult<Response> {
    let return_to = req
        .url()?
        .query_pairs()
        .find(|(k, _)| k == "return_to")
        .map(|(_, v)| v.into_owned())
        .unwrap_or_else(|| "/".into());
    if !safe_return(&return_to) {
        return Err(ApiError::new(
            400,
            "invalid_return_path",
            "Choose a path in this workspace.",
        ));
    }
    let metadata = discovery(req, env).await?;
    let state = random_hex(32)?;
    let browser = random_hex(32)?;
    let nonce = random_hex(32)?;
    let verifier = random_hex(32)?;
    env.d1("DB")?.prepare("INSERT INTO web_login_transactions(state_hash,browser_hash,nonce,verifier,return_to,expires_at) VALUES(?1,?2,?3,?4,?5,?6)").bind(&[hash(&state).into(),hash(&browser).into(),nonce.clone().into(),verifier.clone().into(),return_to.into(),((now()+600) as f64).into()])?.run().await?;
    let mut url = Url::parse(
        metadata["authorization_endpoint"]
            .as_str()
            .ok_or_else(denied)?,
    )
    .map_err(|_| denied())?;
    let digest_hex = hash(&verifier);
    let digest: Vec<u8> = (0..digest_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&digest_hex[i..i + 2], 16).unwrap())
        .collect();
    url.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        ("client_id", &config(env, "WEB_CLIENT_ID")?),
        (
            "redirect_uri",
            &format!("{}/auth/callback", config(env, "WEB_ORIGIN")?),
        ),
        ("scope", "openid profile offline_access"),
        ("state", &state),
        ("nonce", &nonce),
        ("code_challenge", &URL_SAFE_NO_PAD.encode(digest)),
        ("code_challenge_method", "S256"),
    ]);
    finish(redirect(url.as_str())?, req, env, "login", &browser, 600)
}
/// A single D1 DELETE RETURNING prevents callback replay across Worker isolates.
async fn callback(req: &Request, env: &Env) -> ApiResult<Response> {
    let pairs: Vec<_> = req
        .url()?
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let get = |key: &str| -> ApiResult<String> {
        let v: Vec<_> = pairs.iter().filter(|(k, _)| k == key).collect();
        if v.len() != 1 || v[0].1.len() > 4096 {
            return Err(denied());
        }
        Ok(v[0].1.clone())
    };
    let state = get("state")?;
    let browser = cookie(req, &cookie_name(req, env, "login"))?.ok_or_else(denied)?;
    let tx=env.d1("DB")?.prepare("DELETE FROM web_login_transactions WHERE state_hash=?1 AND browser_hash=?2 AND expires_at>?3 RETURNING nonce,verifier,return_to").bind(&[hash(&state).into(),hash(&browser).into(),(now() as f64).into()])?.first::<Transaction>(None).await?.ok_or_else(denied)?;
    if get("iss")? != config(env, "IDENTITY_ISSUER")? || pairs.iter().any(|(k, _)| k == "error") {
        return Err(denied());
    }
    let metadata = discovery(req, env).await?;
    let tokens = exchange(
        req,
        env,
        &metadata,
        &[
            ("grant_type", "authorization_code".into()),
            ("code", get("code")?),
            (
                "redirect_uri",
                format!("{}/auth/callback", config(env, "WEB_ORIGIN")?),
            ),
            ("code_verifier", tx.verifier),
        ],
        false,
    )
    .await?;
    let access = tokens["access_token"].as_str().ok_or_else(denied)?;
    let id = tokens["id_token"].as_str().ok_or_else(denied)?;
    let refresh = tokens["refresh_token"].as_str().ok_or_else(denied)?;
    let principal = auth::verify_token(
        req,
        env,
        id,
        &config(env, "WEB_CLIENT_ID")?,
        Some(&tx.nonce),
    )
    .await?;
    let access_principal =
        auth::verify_token(req, env, access, &config(env, "WEB_CLIENT_ID")?, None).await?;
    if principal.issuer != access_principal.issuer || principal.subject != access_principal.subject
    {
        return Err(denied());
    }
    let session = random_hex(32)?;
    let csrf = random_hex(32)?;
    let expiry = now() + 30 * 24 * 3600;
    let access_expiry = now()
        + tokens["expires_in"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= 3600)
            .ok_or_else(denied)?;
    env.d1("DB")?.prepare("INSERT INTO web_sessions(session_hash,issuer,subject,display_name,csrf,access_token,id_token,refresh_token,access_expires_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)").bind(&[hash(&session).into(),principal.issuer.into(),principal.subject.into(),principal.display_name.map(JsValue::from).unwrap_or(JsValue::NULL),csrf.into(),crypt(env,&hash(&session),"access",access,true).await?.into(),crypt(env,&hash(&session),"id",id,true).await?.into(),crypt(env,&hash(&session),"refresh",refresh,true).await?.into(),(access_expiry as f64).into(),(expiry as f64).into()])?.run().await?;
    let response = finish(redirect(&tx.return_to)?, req, env, "login", "", 0)?;
    finish(response, req, env, "session", &session, 30 * 24 * 3600)
}
/// Check origin and per-session CSRF before any cookie-authorized mutation.
fn csrf(req: &Request, env: &Env, s: &Session) -> ApiResult<()> {
    if req.headers().get("Origin")?.as_deref() != Some(&config(env, "WEB_ORIGIN")?)
        || req.headers().get("X-CSRF-Token")?.as_deref() != Some(&s.csrf)
    {
        return Err(ApiError::new(
            403,
            "csrf_failed",
            "Reload the page and try again.",
        ));
    }
    Ok(())
}
/// Read through a rotating-token lease; never replay a predecessor after an unknown outcome.
async fn session(
    req: &Request,
    env: &Env,
    mutation: bool,
    refresh_tokens: bool,
) -> ApiResult<Session> {
    let value = cookie(req, &cookie_name(req, env, "session"))?.ok_or_else(denied)?;
    if value.len() != 64 {
        return Err(denied());
    }
    origin(req, env)?;
    let db = env.d1("DB")?;
    let sql = "SELECT * FROM web_sessions WHERE session_hash=?1 AND expires_at>?2";
    let mut s = db
        .prepare(sql)
        .bind(&[hash(&value).into(), (now() as f64).into()])?
        .first::<Session>(None)
        .await?
        .ok_or_else(denied)?;
    if s.issuer != config(env, "IDENTITY_ISSUER")? {
        return Err(denied());
    }
    if mutation {
        csrf(req, env, &s)?;
    }
    if !refresh_tokens || s.access_expires_at > now() + 30 {
        return Ok(s);
    }
    if s.refresh_lock.is_some() {
        if s.refresh_lock_until > now() {
            return Err(ApiError::new(
                503,
                "session_refresh_busy",
                "Sign-in is refreshing. Try again.",
            ));
        }
        db.prepare("DELETE FROM web_sessions WHERE session_hash=?1")
            .bind(&[s.session_hash.into()])?
            .run()
            .await?;
        return Err(denied());
    }
    let lock = random_hex(32)?;
    let claimed=db.prepare("UPDATE web_sessions SET refresh_lock=?1,refresh_lock_until=?2 WHERE session_hash=?3 AND refresh_lock IS NULL AND access_expires_at<=?4 RETURNING *").bind(&[lock.clone().into(),((now()+60) as f64).into(),s.session_hash.clone().into(),((now()+30) as f64).into()])?.first::<Session>(None).await?;
    let Some(old) = claimed else {
        return Err(ApiError::new(
            503,
            "session_refresh_busy",
            "Sign-in is refreshing. Try again.",
        ));
    };
    let result = refresh(req, env, &old).await;
    let tokens = match result {
        Ok(v) => v,
        Err(e) => {
            db.prepare("DELETE FROM web_sessions WHERE session_hash=?1 AND refresh_lock=?2")
                .bind(&[old.session_hash.into(), lock.into()])?
                .run()
                .await?;
            return Err(e);
        }
    };
    s.access_token = crypt(
        env,
        &s.session_hash,
        "access",
        tokens["access_token"].as_str().ok_or_else(denied)?,
        true,
    )
    .await?;
    s.refresh_token = crypt(
        env,
        &s.session_hash,
        "refresh",
        tokens["refresh_token"].as_str().ok_or_else(denied)?,
        true,
    )
    .await?;
    s.access_expires_at = now() + tokens["expires_in"].as_u64().ok_or_else(denied)?;
    let committed=db.prepare("UPDATE web_sessions SET access_token=?1,refresh_token=?2,access_expires_at=?3,refresh_lock=NULL,refresh_lock_until=0 WHERE session_hash=?4 AND refresh_lock=?5 RETURNING session_hash").bind(&[s.access_token.clone().into(),s.refresh_token.clone().into(),(s.access_expires_at as f64).into(),s.session_hash.clone().into(),lock.into()])?.first::<Value>(None).await?;
    if committed.is_none() {
        return Err(denied());
    }
    Ok(s)
}
/// Validate fresh access credentials before atomically advancing the session token family.
async fn refresh(req: &Request, env: &Env, s: &Session) -> ApiResult<Value> {
    let metadata = discovery(req, env).await?;
    let tokens = exchange(
        req,
        env,
        &metadata,
        &[
            ("grant_type", "refresh_token".into()),
            (
                "refresh_token",
                crypt(env, &s.session_hash, "refresh", &s.refresh_token, false).await?,
            ),
        ],
        false,
    )
    .await?;
    let access = tokens["access_token"].as_str().ok_or_else(denied)?;
    let p = auth::verify_token(req, env, access, &config(env, "WEB_CLIENT_ID")?, None).await?;
    if p.issuer != s.issuer
        || p.subject != s.subject
        || tokens["refresh_token"].as_str().is_none()
        || !tokens["expires_in"]
            .as_u64()
            .is_some_and(|n| n > 0 && n <= 3600)
    {
        return Err(denied());
    }
    Ok(tokens)
}
/// Authenticate only application cookies, with mutation protection selected by the caller.
pub(crate) async fn authenticate_browser(
    req: &Request,
    env: &Env,
    mutation: bool,
) -> ApiResult<Principal> {
    let s = session(req, env, mutation, true).await?;
    Ok(Principal {
        issuer: s.issuer,
        subject: s.subject,
        display_name: s.display_name,
    })
}
/// Anonymous visitors receive no token and remain able to browse the public registry.
async fn profile(req: &Request, env: &Env) -> ApiResult<Response> {
    let response = match session(req, env, false, true).await {
        Ok(s) => Response::from_json(
            &json!({"user":crate::storage::account(env,Principal{issuer:s.issuer,subject:s.subject,display_name:s.display_name}).await?,"csrf_token":s.csrf}),
        )?,
        Err(e) if e.status == 401 => Response::from_json(&json!({"user":null,"csrf_token":null}))?,
        Err(e) => return Err(e),
    };
    let mut response = response;
    response.headers_mut().set("Cache-Control", "no-store")?;
    Ok(response)
}
/// Delete locally first: a revocation outage can never leave this browser signed in.
async fn logout(req: &Request, env: &Env) -> ApiResult<Response> {
    let s = match session(req, env, true, false).await {
        Ok(s) => s,
        Err(e) if e.status == 401 => {
            return finish(
                Response::from_json(&json!({"signed_out":true}))?,
                req,
                env,
                "session",
                "",
                0,
            )
        }
        Err(e) => return Err(e),
    };
    env.d1("DB")?
        .prepare("DELETE FROM web_sessions WHERE session_hash=?1")
        .bind(&[s.session_hash.clone().into()])?
        .run()
        .await?;
    let revoked = async {
        let refresh = crypt(env, &s.session_hash, "refresh", &s.refresh_token, false).await?;
        let metadata = discovery(req, env).await?;
        exchange(
            req,
            env,
            &metadata,
            &[
                ("token", refresh),
                ("token_type_hint", "refresh_token".into()),
            ],
            true,
        )
        .await
    }
    .await;
    if revoked.is_err() {
        crate::telemetry::log_event(&json!({"event":"web_logout_revocation_failed"}), true);
    }
    let identity = req
        .url()?
        .query_pairs()
        .any(|(k, v)| k == "identity" && v == "1");
    if identity {
        match prepare_logout(req, env, &s).await {
            Ok(response) => return finish(response, req, env, "session", "", 0),
            Err(_) => crate::telemetry::log_event(
                &json!({"event":"web_identity_logout_unavailable"}),
                true,
            ),
        }
    }
    finish(
        Response::from_json(&json!({"signed_out":true}))?,
        req,
        env,
        "session",
        "",
        0,
    )
}

/// Optional provider logout can never roll back the already completed local logout.
async fn prepare_logout(req: &Request, env: &Env, s: &Session) -> ApiResult<Response> {
    let ticket = random_hex(32)?;
    let browser = random_hex(32)?;
    let state = random_hex(32)?;
    let id = crypt(env, &s.session_hash, "id", &s.id_token, false).await?;
    env.d1("DB")?.prepare("INSERT INTO web_logout_transactions(ticket_hash,browser_hash,id_token,state,expires_at) VALUES(?1,?2,?3,?4,?5)").bind(&[hash(&ticket).into(),hash(&browser).into(),crypt(env,&hash(&ticket),"id",&id,true).await?.into(),state.into(),((now()+300) as f64).into()])?.run().await?;
    finish(
        Response::from_json(
            &json!({"signed_out":true,"redirect_to":format!("/auth/logout/identity?ticket={ticket}")}),
        )?,
        req,
        env,
        "logout",
        &browser,
        300,
    )
}

/// A short-lived encrypted logout ticket keeps the ID token out of JSON/API storage.
#[derive(Deserialize)]
struct LogoutTransaction {
    /// Provider hint sealed under the server key and bound to the ticket hash.
    id_token: String,
    /// Random RP-initiated logout state, never an authorization identity.
    state: String,
}
/// Consume only a ticket bound to this browser, then use the provider logout protocol.
async fn logout_identity(req: &Request, env: &Env) -> ApiResult<Response> {
    let values: Vec<_> = req
        .url()?
        .query_pairs()
        .filter(|(k, _)| k == "ticket")
        .map(|(_, v)| v.into_owned())
        .collect();
    if values.len() != 1 || values[0].len() != 64 {
        return Err(denied());
    }
    let ticket = &values[0];
    let browser = cookie(req, &cookie_name(req, env, "logout"))?.ok_or_else(denied)?;
    let metadata = discovery(req, env).await?;
    let tx=env.d1("DB")?.prepare("DELETE FROM web_logout_transactions WHERE ticket_hash=?1 AND browser_hash=?2 AND expires_at>?3 RETURNING id_token,state").bind(&[hash(ticket).into(),hash(&browser).into(),(now() as f64).into()])?.first::<LogoutTransaction>(None).await?.ok_or_else(denied)?;
    let id = crypt(env, &hash(ticket), "id", &tx.id_token, false).await?;
    let mut url = Url::parse(
        metadata["end_session_endpoint"]
            .as_str()
            .ok_or_else(denied)?,
    )
    .map_err(|_| denied())?;
    url.query_pairs_mut().extend_pairs([
        ("id_token_hint", id.as_str()),
        (
            "post_logout_redirect_uri",
            &format!("{}/auth/logout/callback", config(env, "WEB_ORIGIN")?),
        ),
        ("state", tx.state.as_str()),
    ]);
    let response = finish(redirect(url.as_str())?, req, env, "logout", "", 0)?;
    finish(response, req, env, "logout-state", &hash(&tx.state), 300)
}
/// Logout callback cannot redirect until the browser-bound state is verified.
fn logout_callback(req: &Request, env: &Env) -> ApiResult<Response> {
    let values: Vec<_> = req
        .url()?
        .query_pairs()
        .filter(|(k, _)| k == "state")
        .map(|(_, v)| v.into_owned())
        .collect();
    if values.len() != 1
        || cookie(req, &cookie_name(req, env, "logout-state"))?.as_deref()
            != Some(hash(&values[0]).as_str())
    {
        return Err(denied());
    }
    finish(redirect("/")?, req, env, "logout-state", "", 0)
}

/// Scheduled cleanup keeps expired transient records bounded without token logging.
pub(crate) async fn cleanup_expired(env: &Env) -> ApiResult<()> {
    let db = env.d1("DB")?;
    for (table, key) in [
        ("web_login_transactions", "state_hash"),
        ("web_sessions", "session_hash"),
        ("web_logout_transactions", "ticket_hash"),
    ] {
        db.prepare(format!("DELETE FROM {table} WHERE {key} IN (SELECT {key} FROM {table} WHERE expires_at<=?1 LIMIT 1000)"))
            .bind(&[(now() as f64).into()])?
            .run()
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn return_paths_reject_open_redirects() {
        for s in ["//evil.test", "/\\evil.test", "https://evil.test", "/\r\nx"] {
            assert!(!safe_return(s));
        }
        assert!(safe_return("/skills/u_abc/demo?tab=files"));
    }
}

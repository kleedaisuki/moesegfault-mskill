//! Issuer-pinned access-token verification using the Workers WebCrypto runtime.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use futures::lock::Mutex;
use serde::Deserialize;
use serde_json::Value;
use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use worker::{Env, Fetch, Request, RequestInit, RequestRedirect};

/// An authenticated account; ownership is keyed by issuer and subject, never name.
#[derive(Clone, Debug)]
pub struct Principal {
    /// Exact trusted issuer.
    pub issuer: String,
    /// Stable issuer-scoped subject.
    pub subject: String,
    /// Optional presentation-only name, not an authorization identifier.
    pub display_name: Option<String>,
}

/// Sanitized authentication failure suitable for a public problem response.
#[derive(Clone, Copy, Debug)]
pub struct AuthError {
    /// HTTP response status.
    pub status: u16,
    /// Stable machine-readable error code.
    pub code: &'static str,
    /// Public message that never includes tokens or personal claims.
    pub message: &'static str,
}

/// Reject untrusted credentials without disclosing their contents.
fn invalid() -> AuthError {
    AuthError {
        status: 401,
        code: "invalid_token",
        message: "Sign in to continue.",
    }
}

/// Distinguish an unavailable trusted verifier from invalid credentials.
fn unavailable() -> AuthError {
    AuthError {
        status: 503,
        code: "identity_unavailable",
        message: "Sign-in verification is temporarily unavailable.",
    }
}

/// Emit a stable verifier phase, never the URL, credentials or underlying exception.
fn unavailable_at(stage: &'static str) -> AuthError {
    crate::telemetry::log_event(
        &serde_json::json!({"event":"identity_verifier_failure","stage":stage,"error_code":"identity_unavailable"}),
        true,
    );
    unavailable()
}

/// Read configuration without treating missing values as defaults.
fn config(env: &Env, name: &str) -> Option<String> {
    env.var(name)
        .ok()
        .map(|value| value.to_string())
        .or_else(|| env.secret(name).ok().map(|value| value.to_string()))
}

/// Authenticate a bearer access token. Fixture tokens require both local environment
/// configuration and an actual loopback request URL; production never falls back.
pub async fn authenticate(req: &Request, env: &Env) -> Result<Principal, AuthError> {
    let authorization = req
        .headers()
        .get("Authorization")
        .map_err(|_| invalid())?
        .ok_or_else(invalid)?;
    let (scheme, token) = authorization.split_once(' ').ok_or_else(invalid)?;
    if !scheme.eq_ignore_ascii_case("Bearer")
        || token.is_empty()
        || token.len() > 16_384
        || token.contains(char::is_whitespace)
    {
        return Err(invalid());
    }
    if config(env, "LOCAL_DEV_AUTH").as_deref() == Some("true") {
        let url = req.url().map_err(|_| invalid())?;
        if config(env, "ENVIRONMENT").as_deref() != Some("local") || !is_loopback(url.host_str()) {
            return Err(unavailable());
        }
        let subject = if config(env, "DEV_AUTH_TOKEN").as_deref() == Some(token) {
            "fixture-user"
        } else if config(env, "DEV_AUTH_TOKEN_SECOND").as_deref() == Some(token) {
            "fixture-other"
        } else {
            return Err(invalid());
        };
        return Ok(Principal {
            issuer: "urn:mskill:local-fixture".into(),
            subject: subject.into(),
            display_name: None,
        });
    }
    let issuer = config(env, "IDENTITY_ISSUER")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| unavailable_at("issuer_config"))?;
    let audience = config(env, "IDENTITY_CLIENT_ID")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| unavailable_at("audience_config"))?;
    let pieces: Vec<_> = token.split('.').collect();
    if pieces.len() != 3 {
        return Err(invalid());
    }
    let header: TokenHeader = decode_json(pieces[0])?;
    if header.alg != "RS256"
        || header.kid.is_empty()
        || header.kid.len() > 256
        || header.crit.is_some()
    {
        return Err(invalid());
    }
    let claims: Claims = decode_json(pieces[1])?;
    validate_claims(&claims, &issuer, &audience, now())?;
    let allow_local_http = config(env, "ENVIRONMENT").as_deref() == Some("local")
        && req
            .url()
            .ok()
            .is_some_and(|url| is_loopback(url.host_str()));
    let key = signing_key(&issuer, &header.kid, req, allow_local_http).await?;
    let signature = URL_SAFE_NO_PAD.decode(pieces[2]).map_err(|_| invalid())?;
    verify_signature(
        &key,
        &signature,
        format!("{}.{}", pieces[0], pieces[1]).as_bytes(),
    )
    .await?;
    Ok(Principal {
        issuer: claims.iss,
        subject: claims.sub,
        display_name: claims.name,
    })
}

/// Only literal loopback hosts may enter the local fixture boundary.
fn is_loopback(host: Option<&str>) -> bool {
    matches!(host, Some("127.0.0.1" | "[::1]" | "::1" | "localhost"))
}

/// Header data never controls allowed algorithms or endpoint selection.
#[derive(Deserialize)]
struct TokenHeader {
    /// The sole supported algorithm is deployment-pinned RS256.
    alg: String,
    /// Bounded key selector; it never becomes a URL or authentication identity.
    kid: String,
    /// Unsupported critical extensions force rejection rather than partial interpretation.
    crit: Option<Value>,
}

/// Required Identity access-token claims; optional nbf is checked when present.
#[derive(Deserialize)]
struct Claims {
    /// Exact environment-pinned issuer, not a discovery input.
    iss: String,
    /// The registered native client ID; only one accepted audience is permitted.
    aud: Value,
    /// Stable issuer-scoped Identity subject used privately for account mapping.
    sub: String,
    /// Required expiry in Unix seconds; expired tokens are never accepted.
    exp: u64,
    /// Optional not-before boundary permits at most 30 seconds of future skew.
    nbf: Option<u64>,
    /// Access-token discriminator; ID tokens cannot authorize registry operations.
    token_use: String,
    /// Whitespace-delimited scopes; the current provider requires openid.
    scope: String,
    /// Optional presentation value, never an ownership claim.
    name: Option<String>,
}

/// Decode bounded JWT sections with strict unpadded base64url.
fn decode_json<T: serde::de::DeserializeOwned>(part: &str) -> Result<T, AuthError> {
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(part).map_err(|_| invalid())?)
        .map_err(|_| invalid())
}

/// Enforce exact audience (including singleton-array encodings), issuer, time,
/// token kind and scope. A multi-audience token is not this resource's contract.
fn validate_claims(c: &Claims, issuer: &str, audience: &str, time: u64) -> Result<(), AuthError> {
    let aud_matches = c.aud.as_str() == Some(audience)
        || c.aud
            .as_array()
            .is_some_and(|a| a.len() == 1 && a[0].as_str() == Some(audience));
    if c.iss != issuer
        || !aud_matches
        || c.sub.trim().is_empty()
        || c.sub.len() > 1024
        || c.exp <= time
        || c.nbf.is_some_and(|nbf| nbf > time.saturating_add(30))
        || c.token_use != "access"
        || !c.scope.split_ascii_whitespace().any(|s| s == "openid")
    {
        return Err(invalid());
    }
    Ok(())
}

/// Workers' wall clock, expressed as Unix seconds.
fn now() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}

/// Bounded per-isolate issuer cache. The mutex coalesces concurrent refreshes;
/// failed refresh attempts also enter the 30-second cooldown.
#[derive(Default)]
struct KeyCache {
    /// Cache entries cannot cross configured environment/issuer boundaries.
    issuer: String,
    /// Bounded public signing-key set copied from trusted JWKS discovery.
    keys: Vec<Value>,
    /// Hard cache expiry in Unix seconds; stale keys fail closed.
    expires: u64,
    /// Serialized refresh cooldown also applies after failed network attempts.
    last_attempt: Option<u64>,
}
thread_local! {
    static KEYS: Rc<Mutex<KeyCache>> = Rc::new(Mutex::new(KeyCache::default()));
}

/// Select a compatible RSA signing key; neither jku nor x5u is ever followed.
fn select_key(keys: &[Value], kid: &str) -> Option<Value> {
    let mut matching = keys.iter().filter(|k| {
        k["kid"].as_str() == Some(kid)
            && k["kty"].as_str() == Some("RSA")
            && k["use"].as_str() == Some("sig")
            && k["alg"].as_str() == Some("RS256")
            && k.get("key_ops").is_none_or(|ops| {
                ops.as_array()
                    .is_some_and(|a| a.iter().any(|op| op.as_str() == Some("verify")))
            })
            && k["n"]
                .as_str()
                .and_then(|n| URL_SAFE_NO_PAD.decode(n).ok())
                .is_some_and(|n| n.len() >= 256 && n.len() <= 1024)
            && k["e"].as_str().is_some()
    });
    let key = matching.next()?.clone();
    if matching.next().is_some() {
        return None;
    }
    Some(key)
}

/// Refresh only under the issuer lock; unknown kids cannot force unbounded fetches.
async fn signing_key(
    issuer: &str,
    kid: &str,
    req: &Request,
    allow_local_http: bool,
) -> Result<Value, AuthError> {
    let trusted = worker::Url::parse(issuer).map_err(|_| unavailable_at("issuer_url"))?;
    let transport_allowed = trusted.scheme() == "https"
        || (allow_local_http && trusted.scheme() == "http" && is_loopback(trusted.host_str()));
    if !transport_allowed
        || !trusted.username().is_empty()
        || trusted.password().is_some()
        || trusted.query().is_some()
        || trusted.fragment().is_some()
    {
        return Err(unavailable_at("issuer_transport"));
    }
    let cache = KEYS.with(Rc::clone);
    let mut cache = cache.lock().await;
    let time = now();
    if cache.issuer != issuer {
        *cache = KeyCache {
            issuer: issuer.into(),
            ..KeyCache::default()
        };
    }
    if cache.expires > time {
        if let Some(key) = select_key(&cache.keys, kid) {
            return Ok(key);
        }
    }
    if cache
        .last_attempt
        .is_some_and(|last| time < last.saturating_add(30))
    {
        return Err(if cache.expires > time {
            invalid()
        } else {
            unavailable()
        });
    }
    cache.last_attempt = Some(time);
    let discovery = fetch_json(
        &format!(
            "{}/.well-known/openid-configuration",
            issuer.trim_end_matches('/')
        ),
        req,
    )
    .await?;
    if discovery["issuer"].as_str() != Some(issuer) {
        return Err(unavailable_at("discovery_issuer"));
    }
    let jwks_uri = discovery["jwks_uri"]
        .as_str()
        .ok_or_else(|| unavailable_at("discovery_jwks_uri"))?;
    let jwks_url = worker::Url::parse(jwks_uri).map_err(|_| unavailable_at("jwks_url"))?;
    if jwks_url.scheme() != trusted.scheme()
        || jwks_url.origin() != trusted.origin()
        || !jwks_url.username().is_empty()
        || jwks_url.password().is_some()
        || jwks_url.fragment().is_some()
    {
        return Err(unavailable_at("jwks_transport"));
    }
    let jwks = fetch_json(jwks_uri, req).await?;
    let keys = jwks["keys"]
        .as_array()
        .filter(|k| !k.is_empty() && k.len() <= 32)
        .ok_or_else(unavailable)?;
    cache.keys = keys.clone();
    cache.expires = time.saturating_add(300);
    select_key(&cache.keys, kid).ok_or_else(invalid)
}

/// Fetch trusted metadata without credentials, redirects or unvalidated trace data.
async fn fetch_json(url: &str, incoming: &Request) -> Result<Value, AuthError> {
    let mut init = RequestInit::new();
    // Some deployed workerd versions reject redirect="error" at construction.
    // Manual mode never follows a redirect; the strict 200 check below rejects it.
    init.redirect = RequestRedirect::Manual;
    init.headers
        .set("Accept", "application/json")
        .map_err(|_| unavailable())?;
    if let Some(trace) = incoming
        .headers()
        .get("traceparent")
        .map_err(|_| unavailable())?
        .filter(|s| valid_traceparent(s))
    {
        init.headers
            .set("traceparent", &trace)
            .map_err(|_| unavailable())?;
    }
    let request =
        Request::new_with_init(url, &init).map_err(|_| unavailable_at("metadata_request"))?;
    let mut response = Fetch::Request(request)
        .send()
        .await
        .map_err(|_| unavailable_at("metadata_fetch"))?;
    if response.status_code() != 200 {
        return Err(unavailable_at("metadata_status"));
    }
    if response
        .headers()
        .get("Content-Length")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<usize>().ok())
        .is_some_and(|n| n > 65_536)
    {
        return Err(unavailable());
    }
    let text = response
        .text()
        .await
        .map_err(|_| unavailable_at("metadata_body"))?;
    if text.len() > 65_536 {
        return Err(unavailable());
    }
    serde_json::from_str(&text).map_err(|_| unavailable())
}

/// Accept canonical W3C version-00 trace context with nonzero identifiers.
fn valid_traceparent(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() == 55
        && &b[..3] == b"00-"
        && b[35] == b'-'
        && b[52] == b'-'
        && b.iter().enumerate().all(|(i, c)| {
            matches!(i, 2 | 35 | 52) || c.is_ascii_digit() || (b'a'..=b'f').contains(c)
        })
        && b[3..35].iter().any(|c| *c != b'0')
        && b[36..52].iter().any(|c| *c != b'0')
}

/// Delegate RSA verification to platform WebCrypto; never ship custom RSA code.
async fn verify_signature(jwk: &Value, signature: &[u8], data: &[u8]) -> Result<(), AuthError> {
    let crypto: web_sys::Crypto =
        js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("crypto"))
            .map_err(|_| unavailable())?
            .dyn_into()
            .map_err(|_| unavailable())?;
    let algorithm = js_sys::Object::new();
    js_sys::Reflect::set(&algorithm, &"name".into(), &"RSASSA-PKCS1-v1_5".into())
        .map_err(|_| unavailable())?;
    js_sys::Reflect::set(&algorithm, &"hash".into(), &"SHA-256".into())
        .map_err(|_| unavailable())?;
    let key_data: js_sys::Object = js_sys::JSON::parse(&jwk.to_string())
        .map_err(|_| unavailable())?
        .dyn_into()
        .map_err(|_| unavailable())?;
    let usages = js_sys::Array::new();
    usages.push(&JsValue::from_str("verify"));
    let promise = crypto
        .subtle()
        .import_key_with_object("jwk", &key_data, &algorithm, false, usages.as_ref())
        .map_err(|_| unavailable())?;
    let key: web_sys::CryptoKey = JsFuture::from(promise)
        .await
        .map_err(|_| unavailable())?
        .dyn_into()
        .map_err(|_| unavailable())?;
    let promise = crypto
        .subtle()
        .verify_with_str_and_u8_slice_and_u8_array(
            "RSASSA-PKCS1-v1_5",
            &key,
            signature,
            &js_sys::Uint8Array::from(data),
        )
        .map_err(|_| invalid())?;
    if JsFuture::from(promise)
        .await
        .map_err(|_| invalid())?
        .as_bool()
        != Some(true)
    {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_contract_rejects_cross_client_id_tokens_and_wrong_kind() {
        let mut c: Claims = serde_json::from_value(serde_json::json!({"iss":"https://issuer.test","aud":"client","sub":"user","exp":200,"scope":"profile openid","token_use":"access"})).unwrap();
        assert!(validate_claims(&c, "https://issuer.test", "client", 100).is_ok());
        assert!(validate_claims(&c, "https://issuer.test", "other", 100).is_err());
        assert!(validate_claims(&c, "https://issuer.test", "client", 200).is_err());
        c.token_use = "id".into();
        assert!(validate_claims(&c, "https://issuer.test", "client", 100).is_err());
    }

    #[test]
    fn trace_context_and_fixture_hosts_are_bounded() {
        assert!(valid_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
        ));
        assert!(!valid_traceparent(
            "00-00000000000000000000000000000000-00f067aa0ba902b7-01"
        ));
        assert!(!valid_traceparent("secret"));
        assert!(is_loopback(Some("[::1]")));
        assert!(!is_loopback(Some("skills.moesegfault.dev")));
    }
}

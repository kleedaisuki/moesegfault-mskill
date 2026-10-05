//! Native Identity authentication with PKCE and platform-protected rotating credentials.
//! No client secret is bundled. Registration is a deployment prerequisite.

use anyhow::{bail, ensure, Context, Result};
use fs2::FileExt;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use oauth2::{CsrfToken, PkceCodeChallenge};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use url::Url;

/// Production issuer verified against deployed discovery.
pub const DEFAULT_ISSUER: &str = "https://identity.moesegfault.dev";
/// Requires Identity's registered native-loopback variable-port match mode.
pub const DEFAULT_REDIRECT_URI: &str = "http://127.0.0.1:0/callback";

/// Environment-paired, deployment-owned native client registration.
#[derive(Clone)]
pub struct AuthConfig {
    /// Exact issuer; only HTTPS or explicit loopback HTTP is accepted.
    pub issuer: String,
    /// Actual provisioned native client ID, never a guessed product identifier.
    pub client_id: String,
    /// Registered loopback URI. Port zero chooses an available port.
    pub redirect_uri: String,
}

/// Non-secret session metadata safe for CLI display.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Exact verified issuer.
    pub issuer: String,
    /// Stable pairwise subject; do not substitute username or email.
    pub subject: String,
    /// Access-token expiry, Unix seconds.
    pub expires_at: u64,
}

#[derive(Serialize, Deserialize)]
struct Credentials {
    info: SessionInfo,
    access_token: String,
    refresh_token: Option<String>,
    id_token: String,
}
#[derive(Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
    revocation_endpoint: String,
}
#[derive(Deserialize)]
struct TokenSet {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    token_type: String,
}
#[derive(Clone, Deserialize)]
struct Claims {
    iss: String,
    sub: String,
    aud: String,
    exp: u64,
    iat: u64,
    token_use: String,
    nonce: Option<String>,
    scope: Option<String>,
}

/// Native auth client. Tokens live in the OS credential vault, never `~/.mskill` files.
pub struct AuthClient {
    config: AuthConfig,
    http: reqwest::Client,
    lock_path: PathBuf,
    trace_id: Option<String>,
    verbose: bool,
}

impl AuthClient {
    /// Construct an environment-bound client and serialization lock under the library root.
    pub fn new(config: AuthConfig, home: &Path) -> Result<Self> {
        ensure!(
            !config.client_id.trim().is_empty(),
            "Identity client registration is required; set MSKILL_OIDC_CLIENT_ID"
        );
        validate_issuer(&config.issuer)?;
        let redirect = Url::parse(&config.redirect_uri)?;
        ensure!(
            redirect.scheme() == "http"
                && redirect.username().is_empty()
                && redirect.password().is_none()
                && matches!(redirect.host_str(), Some("127.0.0.1") | Some("[::1]"))
                && redirect.query().is_none()
                && redirect.fragment().is_none(),
            "redirect must be a registered HTTP loopback URI without query or fragment"
        );
        std::fs::create_dir_all(home)?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            config,
            http,
            lock_path: std::fs::canonicalize(home)?.join("auth.lock"),
            trace_id: None,
            verbose: false,
        })
    }

    /// Share the CLI command trace across Identity and registry requests without logging URLs or secrets.
    pub fn with_trace_context(mut self, trace_id: &str, verbose: bool) -> Result<Self> {
        ensure!(
            trace_id.len() == 32
                && trace_id.bytes().all(|b| b.is_ascii_hexdigit())
                && trace_id.bytes().any(|b| b != b'0'),
            "invalid command trace ID"
        );
        self.trace_id = Some(trace_id.to_ascii_lowercase());
        self.verbose = verbose;
        Ok(self)
    }

    /// Emit only phase, timing, status and safe correlation metadata; never request URLs or bodies.
    async fn send(
        &self,
        mut request: reqwest::RequestBuilder,
        phase: &str,
    ) -> Result<reqwest::Response> {
        if let Some(trace) = &self.trace_id {
            let span = uuid::Uuid::new_v4().simple().to_string();
            request = request.header("traceparent", format!("00-{trace}-{}-01", &span[..16]));
        }
        let started = std::time::Instant::now();
        let response = request.send().await;
        if self.verbose {
            let status = response.as_ref().map(|r| r.status().as_u16()).unwrap_or(0);
            let correlation = response
                .as_ref()
                .ok()
                .and_then(|r| r.headers().get("x-moesegfault-correlation-id"))
                .and_then(|h| h.to_str().ok())
                .filter(|v| {
                    v.len() <= 128
                        && v.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                })
                .unwrap_or("unavailable");
            eprintln!(
                "identity {phase}: {} ms, HTTP {status}, trace {}, correlation {correlation}",
                started.elapsed().as_millis(),
                self.trace_id.as_deref().unwrap_or("unavailable")
            );
        }
        response.context("Identity request failed")
    }

    /// The test-store feature never permits file credentials for a network issuer.
    #[cfg(feature = "e2e-test-store")]
    fn test_store(&self) -> Result<Option<PathBuf>> {
        if std::env::var_os("MSKILL_TEST_CREDENTIAL_FILE").is_none() {
            return Ok(None);
        }
        let issuer = Url::parse(&self.config.issuer)?;
        ensure!(
            issuer.scheme() == "http"
                && matches!(issuer.host_str(), Some("127.0.0.1") | Some("[::1]")),
            "test credential storage only supports loopback HTTP issuers"
        );
        // Do not accept an arbitrary path; test state remains inside the chosen library root.
        Ok(Some(self.lock_path.with_file_name("test-credentials.json")))
    }
    fn entry(&self) -> Result<keyring::Entry> {
        Ok(keyring::Entry::new(
            "dev.moesegfault.mskill",
            &format!(
                "{}|{}|{}",
                self.config.issuer,
                self.config.client_id,
                self.lock_path
                    .parent()
                    .context("library root missing")?
                    .display()
            ),
        )?)
    }

    fn lock(&self) -> Result<File> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&self.lock_path)?;
        // Nonblocking acquisition prevents a competing CLI from blocking the async runtime indefinitely.
        file.try_lock_exclusive()
            .context("another mskill authentication operation is active; retry when it finishes")?;
        Ok(file)
    }

    fn read(&self) -> Result<Option<Credentials>> {
        #[cfg(feature = "e2e-test-store")]
        if let Some(path) = self.test_store()? {
            return match std::fs::read(path) {
                Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e.into()),
            };
        }
        let entry = self.entry()?;
        match entry.get_secret() {
            Ok(value) => {
                // Older development builds stored UTF-16 passwords on Windows. Preserve readable sessions.
                let credentials = serde_json::from_slice(&value)
                    .or_else(|_| {
                        let password = entry.get_password().map_err(|_| {
                            serde_json::Error::io(std::io::Error::other(
                                "legacy credential unavailable",
                            ))
                        })?;
                        serde_json::from_str(&password)
                    })
                    .context("stored credentials are invalid; log out and sign in again")?;
                Ok(Some(credentials))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => bail!("OS credential vault is unavailable"),
        }
    }

    fn save(&self, credentials: &Credentials) -> Result<()> {
        #[cfg(feature = "e2e-test-store")]
        if let Some(path) = self.test_store()? {
            let tmp = path.with_extension("new");
            std::fs::write(&tmp, serde_json::to_vec(credentials)?)?;
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
            std::fs::rename(tmp, path)?;
            return Ok(());
        }
        self.entry()?
            // Raw UTF-8 avoids doubling JWT storage through Windows' password UTF-16 conversion.
            .set_secret(&serde_json::to_vec(credentials)?)
            .map_err(|_| anyhow::anyhow!("could not persist session in OS credential vault"))
    }

    fn clear(&self) -> Result<()> {
        #[cfg(feature = "e2e-test-store")]
        if let Some(path) = self.test_store()? {
            return match std::fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e.into()),
            };
        }
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => bail!("could not clear OS credential vault"),
        }
    }

    /// Read non-secret session information without refreshing credentials.
    pub fn session(&self) -> Result<Option<SessionInfo>> {
        Ok(self.read()?.map(|c| c.info))
    }

    async fn discovery(&self) -> Result<Discovery> {
        let document: Discovery = self
            .send(
                self.http.get(format!(
                    "{}/.well-known/openid-configuration",
                    self.config.issuer
                )),
                "discovery",
            )
            .await?
            .error_for_status()?
            .json()
            .await?;
        ensure!(
            document.issuer == self.config.issuer,
            "discovery issuer mismatch"
        );
        let issuer = Url::parse(&self.config.issuer)?;
        for endpoint in [
            &document.authorization_endpoint,
            &document.token_endpoint,
            &document.jwks_uri,
            &document.revocation_endpoint,
        ] {
            ensure!(
                Url::parse(endpoint)?.origin() == issuer.origin(),
                "discovery endpoint is outside configured issuer origin"
            );
        }
        Ok(document)
    }

    async fn verify(
        &self,
        token: &str,
        discovery: &Discovery,
        kind: &str,
        nonce: Option<&str>,
    ) -> Result<Claims> {
        let header = decode_header(token).context("invalid JWT header")?;
        ensure!(header.alg == Algorithm::RS256, "unsupported JWT algorithm");
        let kid = header.kid.context("JWT signing key ID missing")?;
        let jwks: serde_json::Value = self
            .send(self.http.get(&discovery.jwks_uri), "jwks")
            .await?
            .error_for_status()?
            .json()
            .await?;
        let key = jwks["keys"]
            .as_array()
            .context("invalid JWKS")?
            .iter()
            .find(|key| {
                key["kid"].as_str() == Some(&kid)
                    && key["kty"] == "RSA"
                    && key["alg"] == "RS256"
                    && key["use"] == "sig"
                    && key.get("key_ops").is_none_or(|ops| {
                        ops.as_array()
                            .is_some_and(|ops| ops.iter().any(|op| op == "verify"))
                    })
            })
            .context("JWT signing key unavailable")?;
        let decoding = DecodingKey::from_rsa_components(
            key["n"].as_str().context("RSA modulus missing")?,
            key["e"].as_str().context("RSA exponent missing")?,
        )?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&self.config.issuer]);
        validation.set_audience(&[&self.config.client_id]);
        validation.set_required_spec_claims(&["iss", "sub", "aud", "exp", "iat"]);
        validation.validate_nbf = true;
        validation.leeway = 30;
        let claims = decode::<Claims>(token, &decoding, &validation)
            .context("JWT verification failed")?
            .claims;
        ensure!(
            claims.aud == self.config.client_id
                && claims.token_use == kind
                && !claims.sub.is_empty()
                && claims.iat <= now()? + 30,
            "JWT claims rejected"
        );
        if let Some(expected) = nonce {
            ensure!(
                claims.nonce.as_deref() == Some(expected),
                "ID-token nonce mismatch"
            );
        }
        if kind == "access" {
            ensure!(
                claims
                    .scope
                    .as_deref()
                    .unwrap_or("")
                    .split_whitespace()
                    .any(|s| s == "openid"),
                "access token lacks openid scope"
            );
        }
        Ok(claims)
    }

    async fn exchange(&self, endpoint: &str, form: &[(&str, &str)]) -> Result<TokenSet> {
        let response = self
            .send(self.http.post(endpoint).form(form), "token")
            .await
            .context("Identity token request failed")?;
        let correlation = response
            .headers()
            .get("x-moesegfault-correlation-id")
            .and_then(|value| value.to_str().ok())
            .filter(|value| {
                value.len() <= 128
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
            })
            .unwrap_or("unavailable")
            .to_owned();
        tracing::debug!(
            status = response.status().as_u16(),
            identity_correlation_id = correlation.as_str(),
            "Identity token request completed"
        );
        // Never include raw response bodies: token responses can carry credentials.
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .json::<serde_json::Value>()
                .await
                .unwrap_or_default();
            let error = body["error"]
                .as_str()
                .filter(|error| {
                    matches!(
                        *error,
                        "invalid_request"
                            | "invalid_grant"
                            | "invalid_client"
                            | "unauthorized_client"
                            | "unsupported_grant_type"
                            | "invalid_scope"
                            | "access_denied"
                            | "temporarily_unavailable"
                            | "server_error"
                    )
                })
                .unwrap_or("request_rejected");
            bail!("Identity token request rejected: {error} (HTTP {status}, correlation {correlation})");
        }
        let tokens: TokenSet = response.json().await.context("invalid token response")?;
        ensure!(
            tokens.token_type.eq_ignore_ascii_case("bearer"),
            "unsupported token type"
        );
        Ok(tokens)
    }

    /// Sign in through the external system browser and a single-use loopback callback.
    pub async fn login(&self) -> Result<SessionInfo> {
        self.login_with_options(true).await
    }

    /// Set `open_browser=false` to print the authorization URL for headless/manual browser use.
    #[tracing::instrument(name = "identity.login", skip_all)]
    pub async fn login_with_options(&self, open_browser: bool) -> Result<SessionInfo> {
        let _lock = self.lock()?;
        let discovery = self.discovery().await?;
        let mut redirect = Url::parse(&self.config.redirect_uri)?;
        let host = redirect
            .host_str()
            .context("redirect host missing")?
            .trim_matches(['[', ']']);
        let bind: std::net::SocketAddr = format!(
            "{}:{}",
            if host.contains(':') {
                format!("[{host}]")
            } else {
                host.to_owned()
            },
            redirect.port().unwrap_or(80)
        )
        .parse()?;
        let listener = TcpListener::bind(bind)
            .await
            .context("could not bind registered loopback callback")?;
        redirect
            .set_port(Some(listener.local_addr()?.port()))
            .map_err(|_| anyhow::anyhow!("invalid callback port"))?;
        let state = CsrfToken::new_random();
        let nonce = CsrfToken::new_random();
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let mut authorization = Url::parse(&discovery.authorization_endpoint)?;
        authorization.query_pairs_mut().extend_pairs([
            ("response_type", "code"),
            ("client_id", &self.config.client_id),
            ("redirect_uri", redirect.as_str()),
            ("scope", "openid offline_access"),
            ("state", state.secret()),
            ("nonce", nonce.secret()),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
        ]);
        eprintln!("Open this URL to sign in:\n{authorization}");
        if open_browser {
            webbrowser::open(authorization.as_str())
                .context("could not open system browser; use login --no-browser")?;
        }
        let (mut stream, _) = tokio::time::timeout(Duration::from_secs(300), listener.accept())
            .await
            .context("login timed out")??;
        let result = self
            .complete_login(
                &mut stream,
                &redirect,
                state.secret(),
                nonce.secret(),
                verifier.secret(),
                &discovery,
            )
            .await;
        let message = if result.is_ok() {
            "Signed in. Return to mskill."
        } else {
            "Sign-in failed. Return to mskill for details."
        };
        let body = format!("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>mskill</title><h1>{message}</h1></html>");
        let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
        let _ = stream.write_all(response.as_bytes()).await;
        result
    }

    async fn complete_login(
        &self,
        stream: &mut TcpStream,
        redirect: &Url,
        state: &str,
        nonce: &str,
        verifier: &str,
        discovery: &Discovery,
    ) -> Result<SessionInfo> {
        let request =
            tokio::time::timeout(Duration::from_secs(10), read_callback(stream)).await??;
        let code = callback_code(&request, redirect, state, &self.config.issuer)?;
        let tokens = self
            .exchange(
                &discovery.token_endpoint,
                &[
                    ("grant_type", "authorization_code"),
                    ("client_id", &self.config.client_id),
                    ("code", &code),
                    ("redirect_uri", redirect.as_str()),
                    ("code_verifier", verifier),
                ],
            )
            .await?;
        let id_token = tokens.id_token.context("ID token missing")?;
        let identity = self.verify(&id_token, discovery, "id", Some(nonce)).await?;
        let access = self
            .verify(&tokens.access_token, discovery, "access", None)
            .await?;
        ensure!(
            identity.sub == access.sub,
            "ID and access token subjects differ"
        );
        let info = SessionInfo {
            issuer: identity.iss,
            subject: identity.sub,
            expires_at: access.exp,
        };
        self.save(&Credentials {
            info: info.clone(),
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            id_token,
        })?;
        Ok(info)
    }

    /// Obtain an access token, serializing rotation across CLI processes.
    #[tracing::instrument(name = "identity.access_token", skip_all)]
    pub async fn access_token(&self) -> Result<String> {
        let _lock = self.lock()?;
        let mut stored = self.read()?.context("not signed in; run mskill login")?;
        if stored.info.expires_at > now()? + 60 {
            return Ok(stored.access_token);
        }
        let refresh = stored
            .refresh_token
            .as_deref()
            .context("session expired; run mskill login")?;
        let discovery = self.discovery().await?;
        let result = self
            .exchange(
                &discovery.token_endpoint,
                &[
                    ("grant_type", "refresh_token"),
                    ("client_id", &self.config.client_id),
                    ("refresh_token", refresh),
                ],
            )
            .await;
        let tokens = match result {
            Ok(tokens) => tokens,
            Err(error) => {
                self.clear()?;
                return Err(error.context("session cleared after refresh failure; sign in again"));
            }
        };
        // A successful rotation invalidates the predecessor: clear it before validating/persisting replacements.
        self.clear()?;
        let access = self
            .verify(&tokens.access_token, &discovery, "access", None)
            .await?;
        ensure!(access.sub == stored.info.subject, "refresh subject changed");
        if let Some(id) = tokens.id_token {
            let identity = self.verify(&id, &discovery, "id", None).await?;
            ensure!(identity.sub == access.sub, "refresh ID subject changed");
            stored.id_token = id;
        }
        stored.access_token = tokens.access_token;
        stored.refresh_token = Some(
            tokens
                .refresh_token
                .context("rotating refresh token missing")?,
        );
        stored.info.expires_at = access.exp;
        self.save(&stored)?;
        Ok(stored.access_token)
    }

    /// Always clear the local session first, then revoke its refresh family when available.
    /// This does not sign out unrelated applications or the Identity browser session.
    #[tracing::instrument(name = "identity.logout", skip_all)]
    pub async fn logout(&self) -> Result<()> {
        let _lock = self.lock()?;
        let stored = self.read();
        self.clear()?;
        let stored = stored?;
        let Some(refresh) = stored.and_then(|s| s.refresh_token) else {
            return Ok(());
        };
        let discovery = self
            .discovery()
            .await
            .context("local session cleared; Identity revocation unavailable")?;
        let response = self
            .send(
                self.http.post(discovery.revocation_endpoint).form(&[
                    ("client_id", self.config.client_id.as_str()),
                    ("token", refresh.as_str()),
                    ("token_type_hint", "refresh_token"),
                ]),
                "revocation",
            )
            .await
            .context("local session cleared; revocation failed")?;
        ensure!(
            response.status().is_success(),
            "local session cleared; revocation rejected (HTTP {})",
            response.status().as_u16()
        );
        Ok(())
    }
}

/// Read a complete bounded HTTP header block; TCP packet boundaries are not request boundaries.
async fn read_callback(stream: &mut TcpStream) -> Result<String> {
    let mut bytes = Vec::with_capacity(2048);
    loop {
        ensure!(bytes.len() < 8192, "callback request too large");
        let mut chunk = [0; 1024];
        let count = stream.read(&mut chunk).await?;
        ensure!(count != 0, "callback connection closed");
        bytes.extend_from_slice(&chunk[..count]);
        ensure!(bytes.len() <= 8192, "callback request too large");
        if bytes.windows(4).any(|part| part == b"\r\n\r\n") {
            return Ok(String::from_utf8(bytes)?);
        }
    }
}

/// Bind the authorization response to one exact loopback transaction before exchanging its code.
fn callback_code(request: &str, redirect: &Url, state: &str, issuer: &str) -> Result<String> {
    let line = request.lines().next().context("empty callback")?;
    let mut parts = line.split_whitespace();
    ensure!(parts.next() == Some("GET"), "invalid callback method");
    let target = parts.next().context("callback target missing")?;
    ensure!(
        target.starts_with('/') && !target.starts_with("//"),
        "invalid callback target"
    );
    let callback = redirect.join(target)?;
    ensure!(callback.path() == redirect.path(), "callback path mismatch");
    let pairs: Vec<_> = callback.query_pairs().collect();
    let field = |name: &str| -> Result<String> {
        let values: Vec<_> = pairs.iter().filter(|(key, _)| key == name).collect();
        ensure!(
            values.len() == 1,
            "missing or duplicated callback parameter {name}"
        );
        Ok(values[0].1.to_string())
    };
    ensure!(
        field("state")? == state && field("iss")? == issuer,
        "authorization callback state or issuer mismatch"
    );
    ensure!(
        !pairs.iter().any(|(key, _)| key == "error"),
        "Identity sign-in was denied"
    );
    let code = field("code")?;
    ensure!(!code.is_empty(), "authorization code missing");
    Ok(code)
}
fn now() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
fn validate_issuer(value: &str) -> Result<()> {
    let issuer = Url::parse(value)?;
    ensure!(
        issuer.username().is_empty()
            && issuer.password().is_none()
            && issuer.query().is_none()
            && issuer.fragment().is_none()
            && issuer.path() == "/"
            && !value.ends_with('/'),
        "issuer must be an exact origin without trailing slash"
    );
    ensure!(
        issuer.scheme() == "https"
            || issuer.scheme() == "http"
                && matches!(issuer.host_str(), Some("127.0.0.1") | Some("[::1]")),
        "issuer must use HTTPS (HTTP only for explicit loopback development)"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn issuer_boundary() {
        assert!(validate_issuer(DEFAULT_ISSUER).is_ok());
        assert!(validate_issuer("http://127.0.0.1:8080").is_ok());
        for issuer in [
            "http://identity.moesegfault.dev",
            "https://identity.moesegfault.dev/",
            "https://identity.moesegfault.dev/path",
            "https://user@identity.moesegfault.dev",
        ] {
            assert!(validate_issuer(issuer).is_err());
        }
    }
    #[test]
    fn callback_is_transaction_bound() {
        let redirect = Url::parse("http://127.0.0.1:54321/callback").unwrap();
        let issuer = "https://identity.moesegfault.dev";
        let valid = "GET /callback?code=abc&state=expected&iss=https%3A%2F%2Fidentity.moesegfault.dev HTTP/1.1\r\n\r\n";
        assert_eq!(
            callback_code(valid, &redirect, "expected", issuer).unwrap(),
            "abc"
        );
        for request in [
            valid.replace("state=expected", "state=wrong"),
            valid.replace("state=expected", "state=expected&state=expected"),
            valid.replace("code=abc", "code=abc&code=other"),
            valid.replace("code=abc", "code="),
            valid.replace("/callback?", "/wrong?"),
            valid.replace(
                "identity.moesegfault.dev",
                "identity-staging.moesegfault.dev",
            ),
            valid.replace("code=abc", "code=abc&error=access_denied"),
            valid.replace("GET", "POST"),
            valid.replace("/callback?", "//attacker.test/callback?"),
        ] {
            assert!(callback_code(&request, &redirect, "expected", issuer).is_err());
        }
    }
}

//! Registry requests share a trace context and strict transport and size bounds.

use anyhow::{bail, Context, Result};
use mskill_protocol::{
    ApiProblem, SkillId, SkillList, SkillMetadata, UserProfile, MAX_ARCHIVE_BYTES,
    REQUEST_ID_HEADER,
};
use reqwest::{Client, RequestBuilder, Response, Url};
use serde::de::DeserializeOwned;
use std::time::Duration;
use uuid::Uuid;

/// An HTTP client for one user command, without ambient credentials.
pub struct Registry {
    /// Validated base URL; credentials may only be sent to this origin.
    base: String,
    /// Redirects are disabled so bearer tokens cannot cross origins.
    client: Client,
    /// W3C trace identifier shared by related requests in this command.
    trace_id: String,
    /// Whether to display transport diagnostics, never authorization headers.
    verbose: bool,
}

impl Registry {
    /// Require HTTPS, except explicit loopback URLs for local simulation.
    pub fn new(base: &str, verbose: bool, trace_id: &str) -> Result<Self> {
        let url = Url::parse(base).context("invalid registry URL")?;
        let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
            bail!("registry must use HTTPS (HTTP is allowed only on loopback)");
        }
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            bail!("registry URL must not contain credentials, query, or fragment");
        }
        let client = Client::builder()
            .user_agent(concat!("mskill/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            base: base.trim_end_matches('/').to_owned(),
            client,
            trace_id: trace_id.to_owned(),
            verbose,
        })
    }

    /// Get the opaque registry owner mapped from a validated account token.
    pub async fn me(&self, token: &str) -> Result<UserProfile> {
        self.json(self.client.get(self.url("/v1/me")).bearer_auth(token))
            .await
    }

    /// List public skill metadata, optionally constrained to an owner.
    pub async fn list(&self, owner: Option<&str>) -> Result<SkillList> {
        let mut all = SkillList {
            skills: Vec::new(),
            next_cursor: None,
        };
        let mut cursors = std::collections::BTreeSet::new();
        loop {
            let mut request = self.client.get(self.url("/v1/skills"));
            if let Some(owner) = owner {
                request = request.query(&[("owner_id", owner)]);
            }
            if let Some(cursor) = all.next_cursor.as_deref() {
                request = request.query(&[("cursor", cursor)]);
            }
            let page: SkillList = self.json(request).await?;
            all.skills.extend(page.skills);
            all.next_cursor = page.next_cursor;
            if all.next_cursor.is_none() {
                return Ok(all);
            }
            let cursor = all.next_cursor.as_ref().expect("cursor checked");
            if !cursors.insert(cursor.clone()) || cursors.len() > 1000 {
                bail!("registry returned an invalid pagination sequence");
            }
        }
    }

    /// Read current metadata; archives are fetched only when their hash differs.
    pub async fn metadata(&self, id: &SkillId) -> Result<SkillMetadata> {
        self.json(self.client.get(self.skill_url(id))).await
    }

    /// Download one bounded archive. The caller verifies its advertised SHA-256.
    pub async fn archive(&self, id: &SkillId, hash: &str) -> Result<Vec<u8>> {
        let response = self
            .send(
                self.client
                    .get(format!("{}/archive", self.skill_url(id)))
                    .query(&[("sha256", hash)]),
            )
            .await?;
        bounded_body(response, MAX_ARCHIVE_BYTES as usize).await
    }

    /// Publish the newest archive under the authenticated owner's identity.
    pub async fn publish(
        &self,
        id: &SkillId,
        token: &str,
        bytes: Vec<u8>,
    ) -> Result<SkillMetadata> {
        self.json(
            self.client
                .put(self.skill_url(id))
                .bearer_auth(token)
                .header("content-type", "application/vnd.mskill.skill")
                .body(bytes),
        )
        .await
    }

    /// Delete the latest remote state. Authorization is enforced by the registry.
    pub async fn remove(&self, id: &SkillId, token: &str) -> Result<()> {
        self.send(self.client.delete(self.skill_url(id)).bearer_auth(token))
            .await?;
        Ok(())
    }

    /// Construct an endpoint from already validated identifier components.
    fn skill_url(&self, id: &SkillId) -> String {
        self.url(&format!("/v1/skills/{}/{}", id.owner_id, id.name))
    }

    /// Retain a deployment prefix when a simulator uses a base path.
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    /// Bound JSON independently from archives, avoiding untrusted response growth.
    async fn json<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<T> {
        let response = self.send(request).await?;
        let body = bounded_body(response, 1024 * 1024).await?;
        serde_json::from_slice(&body).context("registry returned invalid JSON")
    }

    /// Propagate W3C trace context and print a trace ID on actionable failures.
    async fn send(&self, request: RequestBuilder) -> Result<Response> {
        let span_id = &Uuid::new_v4().simple().to_string()[..16];
        let request_id = Uuid::new_v4().simple().to_string();
        let response = request
            .header(
                "traceparent",
                format!("00-{}-{}-01", self.trace_id, span_id),
            )
            .header("x-mskill-client-version", env!("CARGO_PKG_VERSION"))
            .header(REQUEST_ID_HEADER, &request_id)
            .send()
            .await
            .with_context(|| format!("cannot reach registry (trace {})", self.trace_id))?;
        if self.verbose {
            anstream::eprintln!(
                "registry: HTTP {} (trace {}, request {})",
                response.status().as_u16(),
                self.trace_id,
                response
                    .headers()
                    .get(REQUEST_ID_HEADER)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or(&request_id)
            );
        }
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status();
        let body = bounded_body(response, 8192).await.unwrap_or_default();
        let text = if let Ok(problem) = serde_json::from_slice::<ApiProblem>(&body) {
            format!(
                "{}: {} (request {})",
                problem.title, problem.detail, problem.request_id
            )
        } else {
            String::from_utf8_lossy(&body).into_owned()
        };
        let message = text
            .chars()
            .filter(|c| !c.is_control())
            .take(800)
            .collect::<String>();
        Err(HttpFailure {
            status: status.as_u16(),
            message: format!(
                "registry HTTP {}: {} (trace {})",
                status.as_u16(),
                message,
                self.trace_id
            ),
        }
        .into())
    }
}

/// Preserve transport status without asking callers to parse human error text.
#[derive(Debug)]
struct HttpFailure {
    status: u16,
    message: String,
}
impl std::fmt::Display for HttpFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for HttpFailure {}

/// A publish raced a hash-pinned download; refetch metadata rather than mixing states.
pub fn archive_changed(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<HttpFailure>()
        .is_some_and(|failure| failure.status == 409)
}

/// Bound streaming responses even if their Content-Length is absent or misleading.
async fn bounded_body(mut response: Response, maximum: usize) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        bail!("registry response exceeds the {} byte limit", maximum);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .context("registry response interrupted")?
    {
        if body.len().saturating_add(chunk.len()) > maximum {
            bail!("registry response exceeds the {} byte limit", maximum);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

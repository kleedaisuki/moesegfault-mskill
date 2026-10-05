//! Latest-only skill registry: Rust request handlers with D1 metadata and R2 archives.
mod auth;
mod community;
mod landing;
mod storage;
mod telemetry;
mod web_auth;
mod website;

use futures::StreamExt;
use mskill_protocol::{ApiProblem, SkillId, MAX_ARCHIVE_BYTES, SKILL_MEDIA_TYPE};
use worker::{event, Context, Env, Method, Request, Response, ScheduleContext, ScheduledEvent};

/// Public failures carry a stable machine code and actionable sanitized detail.
pub(crate) struct ApiError {
    /// HTTP status for transport and problem body.
    pub status: u16,
    /// Stable error identifier; internal exception strings never become this value.
    pub code: &'static str,
    /// Token-free user-facing error detail.
    pub detail: String,
}

/// Handler result kept distinct from the platform boundary's result.
pub(crate) type ApiResult<T> = std::result::Result<T, ApiError>;

impl ApiError {
    /// Construct a deterministic service error.
    pub(crate) fn new(status: u16, code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            status,
            code,
            detail: detail.into(),
        }
    }
    /// Serialize RFC 9457 problem content with the same response correlation ID.
    fn response(self, request_id: &str) -> worker::Result<Response> {
        let title = match self.status {
            400 => "Invalid request",
            401 => "Sign in required",
            403 => "Not allowed",
            404 => "Not found",
            409 => "Skill changed",
            412 => "Precondition failed",
            413 => "Archive too large",
            415 => "Unsupported media type",
            422 => "Invalid skill archive",
            429 => "Too many requests",
            503 => "Service unavailable",
            _ => "Request failed",
        };
        let mut response = Response::from_json(&ApiProblem {
            problem_type: "about:blank".into(),
            title: title.into(),
            status: self.status,
            detail: self.detail,
            error_code: self.code.into(),
            request_id: request_id.into(),
        })?
        .with_status(self.status);
        response
            .headers_mut()
            .set("content-type", "application/problem+json")?;
        response
            .headers_mut()
            .set("x-mskill-error-code", self.code)?;
        if self.status == 401 {
            response.headers_mut().set("www-authenticate", "Bearer")?;
        }
        if self.code == "session_refresh_busy" {
            response.headers_mut().set("retry-after", "1")?;
        }
        Ok(response)
    }
}

impl From<worker::Error> for ApiError {
    /// Storage/runtime failures remain private and retryable, not public exception text.
    fn from(_: worker::Error) -> Self {
        Self::new(
            503,
            "storage_unavailable",
            "The registry is temporarily unavailable. Try again.",
        )
    }
}

impl From<auth::AuthError> for ApiError {
    /// Preserve the verifier's sanitized status and stable code.
    fn from(error: auth::AuthError) -> Self {
        Self::new(error.status, error.code, error.message)
    }
}

/// Workers entry point: one correlation envelope covers successful and failed routes.
#[event(fetch)]
pub async fn fetch(request: Request, env: Env, platform: Context) -> worker::Result<Response> {
    let context = telemetry::RequestContext::new(&request)?;
    let route = route_name(&request.path());
    telemetry::annotate_platform(&platform, &context, route);
    // Reconstruct mutable request headers without collecting or copying its body.
    let mut request = request.clone_mut()?;
    request
        .headers_mut()?
        .set("traceparent", &context.traceparent)?;
    request
        .headers_mut()?
        .set("x-mskill-request-id", &context.request_id)?;
    let method = request.method().to_string();
    let result = route_request(&mut request, &env).await;
    let response = match result {
        Ok(response) => response,
        Err(error) => error.response(&context.request_id)?,
    };
    context.finish(response, route, &method)
}

/// Dispatch explicit public contracts; malformed path components never reach storage.
async fn route_request(request: &mut Request, env: &Env) -> ApiResult<Response> {
    let path = request.path();
    let method = request.method();
    let product_host = env
        .var("PRODUCT_ORIGIN")
        .ok()
        .and_then(|value| worker::Url::parse(&value.to_string()).ok())
        .is_some_and(|origin| {
            request
                .url()
                .ok()
                .is_some_and(|url| url.origin() == origin.origin())
        });
    if product_host {
        if method == Method::Get {
            let staging = env
                .var("ENVIRONMENT")
                .ok()
                .is_some_and(|value| value.to_string() == "staging");
            if let Some(page) = landing::page(&path, staging) {
                return Ok(page?);
            }
        }
        return Err(ApiError::new(
            404,
            "route_not_found",
            "Use the Skills workspace for registry operations.",
        ));
    }
    if let Some(result) = web_auth::route(request, env).await {
        return result;
    }
    if path == "/web/publish" && method == Method::Post {
        let principal = web_auth::authenticate_browser(request, env, true).await?;
        let profile = storage::account(env, principal).await?;
        let condition = storage::WriteCondition::from_request(request)?;
        let bytes = skill_body(request).await?;
        return storage::publish_for_owner(env, &profile.owner_id, bytes, condition).await;
    }
    if method == Method::Get {
        if let Some(page) = website::page(request, env).await {
            return Ok(page?);
        }
        if path == "/health" {
            return storage::health(env).await;
        }
        if path == "/v1/config" {
            let issuer = env.var("IDENTITY_ISSUER")?.to_string();
            let client_id = env.var("IDENTITY_CLIENT_ID")?.to_string();
            return Ok(Response::from_json(
                &serde_json::json!({"identity_issuer":issuer,"identity_client_id":client_id}),
            )?);
        }
        if path == "/v1/skills" {
            return storage::list(request, env).await;
        }
        if path == "/v1/community" {
            return community::catalog(request, env).await;
        }
        if let Some(owner) = path
            .strip_prefix("/v1/accounts/")
            .filter(|s| !s.contains('/'))
        {
            return community::profile(env, owner).await;
        }
        if path == "/v1/me" {
            let principal = auth::authenticate(request, env).await?;
            return Ok(Response::from_json(
                &storage::account(env, principal).await?,
            )?);
        }
    }
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() >= 6
        && parts[1] == "v1"
        && parts[2] == "skills"
        && matches!(parts[5], "preview" | "comments")
    {
        let id = SkillId::new(parts[3], parts[4])
            .map_err(|error| ApiError::new(400, "invalid_skill_id", error.to_string()))?;
        if parts.len() == 6 && parts[5] == "preview" && method == Method::Get {
            return community::preview(request, env, &id).await;
        }
        if parts[5] == "comments" {
            if parts.len() == 6 && method == Method::Get {
                return community::comments(request, env, &id).await;
            }
            if (parts.len() == 6 && method == Method::Post)
                || (parts.len() == 7 && (method == Method::Patch || method == Method::Delete))
            {
                return community::mutate_comment(request, env, &id, parts.get(6).copied()).await;
            }
        }
        return Err(ApiError::new(
            405,
            "method_not_allowed",
            "Unsupported preview or discussion operation.",
        ));
    }
    if !(parts.len() == 5 || (parts.len() == 6 && parts[5] == "archive"))
        || parts[1] != "v1"
        || parts[2] != "skills"
    {
        return Err(ApiError::new(
            404,
            "route_not_found",
            "This registry route does not exist.",
        ));
    }
    let id = SkillId::new(parts[3], parts[4])
        .map_err(|error| ApiError::new(400, "invalid_skill_id", error.to_string()))?;
    if id.owner_id == "local" {
        return Err(ApiError::new(
            400,
            "invalid_owner_id",
            "The local namespace cannot be published. Sign in for your publisher ID.",
        ));
    }
    if parts.len() == 6 {
        if method != Method::Get {
            return Err(ApiError::new(
                405,
                "method_not_allowed",
                "Use GET to download this archive.",
            ));
        }
        return storage::download(request, env, &id).await;
    }
    if method == Method::Get {
        return storage::metadata(request, env, &id).await;
    }
    if method != Method::Put && method != Method::Delete {
        return Err(ApiError::new(
            405,
            "method_not_allowed",
            "Use GET, PUT or DELETE for a skill.",
        ));
    }
    let principal = request_principal(request, env, true).await?;
    let profile = storage::account(env, principal).await?;
    if profile.owner_id != id.owner_id {
        return Err(ApiError::new(
            403,
            "not_skill_owner",
            "You can manage only skills in your publisher namespace.",
        ));
    }
    if method == Method::Delete {
        return storage::remove(env, &id, storage::WriteCondition::from_request(request)?).await;
    }
    let condition = storage::WriteCondition::from_request(request)?;
    let bytes = skill_body(request).await?;
    storage::publish(env, &id, bytes, condition).await
}

/// Web inference and named CLI publication share the same bounded transport path.
async fn skill_body(request: &mut Request) -> ApiResult<Vec<u8>> {
    let content_type = request.headers().get("content-type")?.unwrap_or_default();
    if !matches!(
        content_type.split(';').next().unwrap_or_default().trim(),
        SKILL_MEDIA_TYPE | "application/zip" | "application/octet-stream"
    ) {
        return Err(ApiError::new(
            415,
            "unsupported_media_type",
            "Upload a .skill ZIP archive.",
        ));
    }
    bounded_body(request).await
}

/// Native bearer and browser cookie trust boundaries remain independently strict.
/// A browser never supplies a web-audience bearer token to the CLI resource API.
pub(crate) async fn request_principal(
    request: &Request,
    env: &Env,
    mutation: bool,
) -> ApiResult<auth::Principal> {
    if request.headers().get("authorization")?.is_some() {
        return Ok(auth::authenticate(request, env).await?);
    }
    web_auth::authenticate_browser(request, env, mutation).await
}

/// Enforce the bound on streamed bytes rather than trusting Content-Length alone.
async fn bounded_body(request: &mut Request) -> ApiResult<Vec<u8>> {
    let declared = request
        .headers()
        .get("content-length")?
        .and_then(|n| n.parse::<u64>().ok());
    if declared.is_some_and(|n| n > MAX_ARCHIVE_BYTES) {
        return Err(ApiError::new(
            413,
            "archive_too_large",
            "Archives must be no larger than 16 MiB.",
        ));
    }
    let mut body = Vec::with_capacity(declared.unwrap_or(8192).min(MAX_ARCHIVE_BYTES) as usize);
    let mut stream = request.stream()?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if body.len().saturating_add(chunk.len()) > MAX_ARCHIVE_BYTES as usize {
            return Err(ApiError::new(
                413,
                "archive_too_large",
                "Archives must be no larger than 16 MiB.",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Log stable route templates rather than user-supplied URLs or query parameters.
fn route_name(path: &str) -> &'static str {
    match path {
        "/" => "/",
        "/privacy" => "/privacy",
        "/terms" => "/terms",
        "/health" => "/health",
        "/v1/config" => "/v1/config",
        "/v1/me" => "/v1/me",
        "/v1/skills" => "/v1/skills",
        "/v1/community" => "/v1/community",
        "/web/session" => "/web/session",
        "/web/publish" => "/web/publish",
        "/auth/login" => "/auth/login",
        "/auth/callback" => "/auth/callback",
        "/auth/logout" => "/auth/logout",
        "/auth/logout/identity" => "/auth/logout/identity",
        "/auth/logout/callback" => "/auth/logout/callback",
        _ if path.starts_with("/v1/accounts/") => "/v1/accounts/{owner}",
        _ if path.starts_with("/v1/skills/") && path.contains("/comments") => {
            "/v1/skills/{owner}/{name}/comments/{id?}"
        }
        _ if path.starts_with("/v1/skills/") && path.ends_with("/preview") => {
            "/v1/skills/{owner}/{name}/preview"
        }
        _ if path.starts_with("/v1/skills/") && path.ends_with("/archive") => {
            "/v1/skills/{owner}/{name}/archive"
        }
        _ if path.starts_with("/v1/skills/") => "/v1/skills/{owner}/{name}",
        _ if path.starts_with("/assets/") => "/assets/{name}",
        _ => "unmatched",
    }
}

/// Retry durable garbage jobs and bounded orphan sweeps using a platform Cron Trigger.
#[event(scheduled)]
pub async fn scheduled(_: ScheduledEvent, env: Env, _: ScheduleContext) {
    if web_auth::cleanup_expired(&env).await.is_err() {
        telemetry::log_event(
            &serde_json::json!({"event":"web_session_cleanup","outcome":"retry_required"}),
            true,
        );
    }
    match storage::collect_garbage(&env).await {
        Ok(deleted) => telemetry::log_event(
            &serde_json::json!({"event":"garbage_collection","deleted":deleted,"outcome":"success"}),
            false,
        ),
        Err(_) => telemetry::log_event(
            &serde_json::json!({"event":"garbage_collection","outcome":"retry_required"}),
            true,
        ),
    }
}

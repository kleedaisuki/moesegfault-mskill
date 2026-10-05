//! Public discovery and bounded text previews with authenticated conversations.
use crate::{storage, telemetry::random_hex, ApiError, ApiResult};
use mskill_protocol::{sha256_hex, validate_owner_id, SkillId, MAX_SKILL_MD_BYTES};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{Cursor, Read};
use wasm_bindgen::JsValue;
use worker::{Env, Request, Response};

/// Secondary resources must remain small enough for interactive mobile previews.
const MAX_RESOURCE_BYTES: u64 = 256 * 1024;
/// JSON overhead is bounded before reading any user-authored comment.
const MAX_COMMENT_BODY_BYTES: usize = 24 * 1024;

/// Public conversation fields exclude provider identifiers and session state.
#[derive(Serialize, Deserialize)]
struct Comment {
    /// Opaque identifier, unique independently of a package's latest digest.
    id: String,
    /// Public author namespace; frontend compares this with the signed-in profile.
    owner_id: String,
    /// Presentation label only, never an authorization decision.
    display_name: Option<String>,
    /// Plain user text; rendering must escape HTML.
    body: String,
    /// Stable ordering timestamp from the server clock.
    created_at: String,
    /// Last edit time; editing does not reorder the discussion.
    updated_at: String,
}

/// Small public tree entries; only the selected safe UTF-8 file includes content.
#[derive(Serialize)]
struct PreviewFile {
    /// Portable archive-relative resource path.
    path: String,
    /// Uncompressed file length, independently bounded during publication.
    size_bytes: u64,
    /// Escaped as plain data by clients; never served as executable content.
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}

/// D1 result failures are private, including statement/database names.
fn checked(result: &worker::D1Result) -> ApiResult<()> {
    if result.success() {
        Ok(())
    } else {
        Err(ApiError::new(
            503,
            "storage_unavailable",
            "Try again shortly.",
        ))
    }
}

/// Parse a deliberately small page size for interactive public lists.
fn page_limit(request: &Request) -> ApiResult<usize> {
    request
        .url()?
        .query_pairs()
        .find(|(k, _)| k == "limit")
        .map(|(_, value)| {
            value
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=100).contains(n))
                .ok_or_else(|| {
                    ApiError::new(400, "invalid_limit", "Use a limit between 1 and 100.")
                })
        })
        .unwrap_or(Ok(30))
}

/// Enrich latest metadata with public author labels and actual discussion counts.
pub(crate) async fn catalog(request: &Request, env: &Env) -> ApiResult<Response> {
    let url = request.url()?;
    let query: std::collections::HashMap<_, _> = url.query_pairs().collect();
    let search = query.get("q").map(|v| v.trim()).unwrap_or("");
    if search.chars().count() > 100 {
        return Err(ApiError::new(
            400,
            "invalid_search",
            "Search must be at most 100 characters.",
        ));
    }
    let owner = query.get("owner_id").map(|v| v.as_ref()).unwrap_or("");
    if !owner.is_empty() {
        validate_owner_id(owner)
            .map_err(|_| ApiError::new(400, "invalid_owner_id", "Invalid publisher."))?;
    }
    let cursor = query.get("cursor").map(|v| v.as_ref()).unwrap_or("");
    if cursor.len() > 512 {
        return Err(ApiError::new(400, "invalid_cursor", "Invalid list cursor."));
    }
    let marker: Vec<String> = if cursor.is_empty() {
        vec![String::new(); 3]
    } else {
        serde_json::from_str(cursor)
            .map_err(|_| ApiError::new(400, "invalid_cursor", "Invalid list cursor."))?
    };
    if marker.len() != 3 || cursor.len() > 512 {
        return Err(ApiError::new(400, "invalid_cursor", "Invalid list cursor."));
    }
    let limit = page_limit(request)?;
    let result = env.d1("DB")?.prepare("SELECT s.owner_id,s.name,s.sha256,s.size_bytes,s.description,s.updated_at,a.display_name,(SELECT count(*) FROM skill_comments c WHERE c.skill_owner=s.owner_id AND c.skill_name=s.name) AS comment_count FROM skills s JOIN accounts a ON a.owner_id=s.owner_id WHERE (?1='' OR instr(lower(s.name),lower(?1))>0 OR instr(lower(s.description),lower(?1))>0 OR instr(lower(coalesce(a.display_name,'')),lower(?1))>0) AND (?2='' OR s.owner_id=?2) AND (?3='' OR s.updated_at<?3 OR (s.updated_at=?3 AND (s.owner_id>?4 OR (s.owner_id=?4 AND s.name>?5)))) ORDER BY s.updated_at DESC,s.owner_id,s.name LIMIT ?6")
        .bind(&[search.into(),owner.into(),marker[0].as_str().into(),marker[1].as_str().into(),marker[2].as_str().into(),JsValue::from_f64((limit+1) as f64)])?.all().await?;
    checked(&result)?;
    let mut skills: Vec<Value> = result.results()?;
    let next_cursor = if skills.len() > limit {
        skills.truncate(limit);
        skills
            .last()
            .map(|s| json!([s["updated_at"], s["owner_id"], s["name"]]).to_string())
    } else {
        None
    };
    Ok(Response::from_json(
        &json!({"skills":skills,"next_cursor":next_cursor}),
    )?)
}

/// Anonymous profile uses only explicitly public account fields.
pub(crate) async fn profile(env: &Env, owner: &str) -> ApiResult<Response> {
    validate_owner_id(owner)
        .map_err(|_| ApiError::new(400, "invalid_owner_id", "Invalid publisher."))?;
    let profile: Option<Value> = env.d1("DB")?.prepare("SELECT owner_id,display_name,(SELECT count(*) FROM skills WHERE owner_id=a.owner_id) AS skill_count,(SELECT count(*) FROM skill_comments WHERE author_id=a.owner_id) AS comment_count FROM accounts a WHERE owner_id=?1")
        .bind(&[owner.into()])?.first(None).await?;
    Ok(Response::from_json(&profile.ok_or_else(|| {
        ApiError::new(404, "account_not_found", "Publisher not found.")
    })?)?)
}

/// Reuse for server rendering and JSON so crawlers and people see the same bytes.
pub(crate) async fn preview_data(
    env: &Env,
    id: &SkillId,
    selected: Option<&str>,
) -> ApiResult<Value> {
    let (metadata, bytes) = storage::archive_bytes(env, id).await?;
    if sha256_hex(&bytes) != metadata.sha256 {
        return Err(ApiError::new(
            503,
            "archive_unavailable",
            "The archive is temporarily unavailable.",
        ));
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| {
        ApiError::new(
            503,
            "archive_unavailable",
            "The archive is temporarily unavailable.",
        )
    })?;
    let selected = selected.unwrap_or("SKILL.md");
    let mut files = Vec::with_capacity(archive.len());
    let mut skill_md = String::new();
    let mut found = false;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|_| {
            ApiError::new(
                503,
                "archive_unavailable",
                "The archive is temporarily unavailable.",
            )
        })?;
        if file.is_dir() {
            continue;
        }
        let path = file.name().to_owned();
        let limit = if path == "SKILL.md" {
            MAX_SKILL_MD_BYTES
        } else {
            MAX_RESOURCE_BYTES
        };
        let mut text = None;
        if path == selected || path == "SKILL.md" {
            found |= path == selected;
            if file.size() <= limit {
                let mut bytes = Vec::with_capacity(file.size() as usize);
                file.by_ref()
                    .take(limit + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| {
                        ApiError::new(
                            503,
                            "archive_unavailable",
                            "This resource could not be read.",
                        )
                    })?;
                if bytes.len() as u64 <= limit && !bytes.contains(&0) {
                    text = String::from_utf8(bytes).ok();
                }
            }
            if path == "SKILL.md" {
                skill_md = text.clone().unwrap_or_default();
            }
        }
        files.push(PreviewFile {
            path,
            size_bytes: file.size(),
            text,
        });
    }
    if !found {
        return Err(ApiError::new(
            404,
            "resource_not_found",
            "This archive resource does not exist.",
        ));
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(json!({"sha256":metadata.sha256,"skill_md":skill_md,"files":files,"truncated":false}))
}

/// JSON does not change the content type to archive-authored HTML or script.
pub(crate) async fn preview(request: &Request, env: &Env, id: &SkillId) -> ApiResult<Response> {
    let selected = request
        .url()?
        .query_pairs()
        .find(|(k, _)| k == "path")
        .map(|(_, v)| v.into_owned());
    Ok(Response::from_json(
        &preview_data(env, id, selected.as_deref()).await?,
    )?)
}

/// Require a currently published package before listing or adding conversations.
async fn require_skill(env: &Env, id: &SkillId) -> ApiResult<()> {
    let exists: Option<Value> = env
        .d1("DB")?
        .prepare("SELECT 1 AS found FROM skills WHERE owner_id=?1 AND name=?2")
        .bind(&[id.owner_id.as_str().into(), id.name.as_str().into()])?
        .first(None)
        .await?;
    if exists.is_none() {
        return Err(ApiError::new(
            404,
            "skill_not_found",
            "This skill is not published.",
        ));
    }
    Ok(())
}

/// Public conversations use keyset pagination, not unbounded full-thread reads.
pub(crate) async fn comments(request: &Request, env: &Env, id: &SkillId) -> ApiResult<Response> {
    require_skill(env, id).await?;
    let limit = page_limit(request)?;
    let cursor = request
        .url()?
        .query_pairs()
        .find(|(k, _)| k == "cursor")
        .map(|(_, v)| v.into_owned())
        .unwrap_or_default();
    if cursor.len() > 256 {
        return Err(ApiError::new(
            400,
            "invalid_cursor",
            "Invalid discussion cursor.",
        ));
    }
    let marker: Vec<String> = if cursor.is_empty() {
        vec![String::new(); 2]
    } else {
        serde_json::from_str(&cursor)
            .map_err(|_| ApiError::new(400, "invalid_cursor", "Invalid discussion cursor."))?
    };
    if marker.len() != 2 || cursor.len() > 256 {
        return Err(ApiError::new(
            400,
            "invalid_cursor",
            "Invalid discussion cursor.",
        ));
    }
    let result = env.d1("DB")?.prepare("SELECT c.id,c.author_id AS owner_id,a.display_name,c.body,c.created_at,c.updated_at FROM skill_comments c JOIN accounts a ON a.owner_id=c.author_id WHERE c.skill_owner=?1 AND c.skill_name=?2 AND (?3='' OR c.created_at>?3 OR (c.created_at=?3 AND c.id>?4)) ORDER BY c.created_at,c.id LIMIT ?5")
        .bind(&[id.owner_id.as_str().into(),id.name.as_str().into(),marker[0].as_str().into(),marker[1].as_str().into(),JsValue::from_f64((limit+1) as f64)])?.all().await?;
    checked(&result)?;
    let mut comments: Vec<Comment> = result.results()?;
    let next_cursor = if comments.len() > limit {
        comments.truncate(limit);
        comments
            .last()
            .map(|c| json!([c.created_at, c.id]).to_string())
    } else {
        None
    };
    Ok(Response::from_json(
        &json!({"comments":comments,"next_cursor":next_cursor}),
    )?)
}

/// Bound request collection and validate plain-text discussion semantics.
async fn comment_body(request: &mut Request) -> ApiResult<String> {
    use futures::StreamExt;
    let content_type = request.headers().get("content-type")?.unwrap_or_default();
    if content_type.split(';').next().unwrap_or("").trim() != "application/json" {
        return Err(ApiError::new(
            415,
            "unsupported_media_type",
            "Send comments as JSON.",
        ));
    }
    let mut bytes = Vec::new();
    let mut stream = request.stream()?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if bytes.len() + chunk.len() > MAX_COMMENT_BODY_BYTES {
            return Err(ApiError::new(
                413,
                "comment_too_large",
                "Comments must be at most 4096 characters.",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| ApiError::new(400, "invalid_comment", "Send a JSON comment body."))?;
    let body = value["body"].as_str().unwrap_or("").trim();
    if body.is_empty()
        || body.chars().count() > 4096
        || body.len() > 16384
        || body
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t' && c != '\r')
    {
        return Err(ApiError::new(
            400,
            "invalid_comment",
            "Comments must contain 1 to 4096 text characters.",
        ));
    }
    Ok(body.to_owned())
}

/// Authorship and package-owner moderation are checked in the mutation itself.
pub(crate) async fn mutate_comment(
    request: &mut Request,
    env: &Env,
    id: &SkillId,
    comment_id: Option<&str>,
) -> ApiResult<Response> {
    if comment_id.is_some_and(|v| {
        v.len() != 34 || !v.starts_with("c_") || !v[2..].bytes().all(|b| b.is_ascii_hexdigit())
    }) {
        return Err(ApiError::new(
            400,
            "invalid_comment_id",
            "Invalid comment identifier.",
        ));
    }
    let principal = crate::request_principal(request, env, true).await?;
    let author = storage::account(env, principal).await?;
    require_skill(env, id).await?;
    let db = env.d1("DB")?;
    if request.method() == worker::Method::Delete {
        let comment_id = comment_id.ok_or_else(|| {
            ApiError::new(405, "method_not_allowed", "Select a comment to delete.")
        })?;
        let removed:Option<Value>=db.prepare("DELETE FROM skill_comments WHERE id=?1 AND skill_owner=?2 AND skill_name=?3 AND (author_id=?4 OR skill_owner=?4) RETURNING id")
            .bind(&[comment_id.into(),id.owner_id.as_str().into(),id.name.as_str().into(),author.owner_id.as_str().into()])?.first(None).await?;
        if removed.is_none() {
            return Err(ApiError::new(
                403,
                "not_comment_owner",
                "You can delete your own comments or moderate your skills.",
            ));
        }
        return Ok(Response::empty()?.with_status(204));
    }
    let body = comment_body(request).await?;
    let timestamp = js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_default();
    let comment_id = match comment_id {
        Some(comment_id) => {
            let changed:Option<Value>=db.prepare("UPDATE skill_comments SET body=?1,updated_at=?2 WHERE id=?3 AND author_id=?4 AND skill_owner=?5 AND skill_name=?6 RETURNING id")
                .bind(&[body.as_str().into(),timestamp.as_str().into(),comment_id.into(),author.owner_id.as_str().into(),id.owner_id.as_str().into(),id.name.as_str().into()])?.first(None).await?;
            if changed.is_none() {
                return Err(ApiError::new(
                    403,
                    "not_comment_owner",
                    "You can edit only your own comments.",
                ));
            }
            comment_id.to_owned()
        }
        None => {
            // A bounded rolling author budget prevents accidental duplicate loops.
            let comment_id = format!("c_{}", random_hex(16)?);
            let inserted:Option<Value>=db.prepare("INSERT INTO skill_comments(id,skill_owner,skill_name,author_id,body,created_at,updated_at) SELECT ?1,?2,?3,?4,?5,?6,?6 WHERE (SELECT count(*) FROM skill_comments WHERE author_id=?4 AND julianday(created_at)>julianday('now','-1 hour'))<60 RETURNING id")
                .bind(&[comment_id.as_str().into(),id.owner_id.as_str().into(),id.name.as_str().into(),author.owner_id.as_str().into(),body.as_str().into(),timestamp.as_str().into()])?.first(None).await?;
            if inserted.is_none() {
                return Err(ApiError::new(
                    429,
                    "comment_rate_limit",
                    "Wait before posting more comments.",
                ));
            }
            comment_id
        }
    };
    let comment:Comment=db.prepare("SELECT c.id,c.author_id AS owner_id,a.display_name,c.body,c.created_at,c.updated_at FROM skill_comments c JOIN accounts a ON a.owner_id=c.author_id WHERE c.id=?1")
        .bind(&[comment_id.into()])?.first(None).await?.ok_or_else(||ApiError::new(503,"comment_unavailable","Reload the discussion."))?;
    Ok(
        Response::from_json(&comment)?.with_status(if request.method() == worker::Method::Post {
            201
        } else {
            200
        }),
    )
}

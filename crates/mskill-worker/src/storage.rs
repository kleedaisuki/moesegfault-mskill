//! D1's latest pointer is authoritative; write-unique R2 keys prevent ABA races.
use crate::{auth::Principal, telemetry::random_hex, ApiError, ApiResult};
use mskill_protocol::{
    sha256_hex, validate_archive, validate_owner_id, validate_sha256, SkillId, SkillList,
    SkillMetadata, UserProfile, SKILL_MEDIA_TYPE,
};
use serde::Deserialize;
use std::collections::HashSet;
use wasm_bindgen::JsValue;
use worker::{Date, Env, Request, Response};

/// Time for existing downloads to finish before a superseded blob is reclaimed.
const GARBAGE_GRACE_MS: u64 = 10 * 60 * 1000;
/// The finite upload lease is checked inside the same transaction as pointer commit.
const UPLOAD_LEASE_MS: u64 = 60 * 60 * 1000;

/// Storage-only row augments the stable public metadata with its immutable R2 key.
#[derive(Deserialize)]
struct StoredSkill {
    /// Public wire fields retain their flat shape in D1 row JSON.
    #[serde(flatten)]
    metadata: SkillMetadata,
    /// Never included in public metadata or used as a user-visible version.
    storage_key: String,
}

/// Durable cleanup work does not require retaining the superseded skill metadata.
#[derive(Deserialize)]
struct GarbageKey {
    /// Write-unique object eligible for deletion.
    storage_key: String,
}

/// UTC RFC 3339 timestamps use the platform clock rather than unavailable WASI time.
fn timestamp() -> String {
    js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .expect("Date ISO string")
}

/// Require a successful D1 result even when the binding returns a result envelope.
fn successful(result: &worker::D1Result) -> ApiResult<()> {
    if result.success() {
        Ok(())
    } else {
        Err(ApiError::new(
            503,
            "storage_unavailable",
            "The registry is temporarily unavailable. Try again.",
        ))
    }
}

/// Resolve the public opaque namespace, atomically creating one for an Identity user.
pub(crate) async fn account(env: &Env, principal: Principal) -> ApiResult<UserProfile> {
    let db = env.d1("DB")?;
    let bindings = [
        JsValue::from_str(&principal.issuer),
        JsValue::from_str(&principal.subject),
    ];
    let query = "SELECT owner_id, display_name FROM accounts WHERE issuer=?1 AND subject=?2";
    if let Some(profile) = db
        .prepare(query)
        .bind(&bindings)?
        .first::<UserProfile>(None)
        .await?
    {
        return Ok(profile);
    }
    let owner_id = format!("u_{}", random_hex(16)?);
    let display_name = principal
        .display_name
        .map(|name| name.chars().take(100).collect::<String>());
    let result = db.prepare("INSERT INTO accounts(owner_id,issuer,subject,display_name,created_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(issuer,subject) DO NOTHING")
        .bind(&[owner_id.into(),principal.issuer.into(),principal.subject.into(),display_name.map(JsValue::from).unwrap_or(JsValue::NULL),timestamp().into()])?.run().await?;
    successful(&result)?;
    db.prepare(query)
        .bind(&bindings)?
        .first::<UserProfile>(None)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                503,
                "account_mapping_failed",
                "Your publisher namespace could not be created. Try again.",
            )
        })
}

/// Probe both authoritative metadata and object storage without revealing internals.
pub(crate) async fn health(env: &Env) -> ApiResult<Response> {
    let db = env.d1("DB")?;
    db.prepare("SELECT 1 AS ready FROM skills LIMIT 1")
        .first::<serde_json::Value>(None)
        .await?;
    env.bucket("SKILLS")?
        .head("__mskill_health_probe__")
        .await?;
    Ok(Response::from_json(&serde_json::json!({"status":"ok"}))?)
}

/// Read the latest row only; no release/history table exists.
async fn find(env: &Env, id: &SkillId) -> ApiResult<Option<StoredSkill>> {
    Ok(env.d1("DB")?.prepare("SELECT owner_id,name,sha256,size_bytes,description,updated_at,storage_key FROM skills WHERE owner_id=?1 AND name=?2")
        .bind(&[id.owner_id.as_str().into(),id.name.as_str().into()])?.first(None).await?)
}

/// Keyset pagination is stable without offset scans and has a fixed memory budget.
pub(crate) async fn list(request: &Request, env: &Env) -> ApiResult<Response> {
    let url = request.url()?;
    let query = url
        .query_pairs()
        .collect::<std::collections::HashMap<_, _>>();
    let owner = query.get("owner_id").map(|v| v.as_ref()).unwrap_or("");
    if !owner.is_empty() {
        validate_owner_id(owner)
            .map_err(|e| ApiError::new(400, "invalid_owner_id", e.to_string()))?;
    }
    let cursor = query.get("cursor").map(|v| v.as_ref()).unwrap_or("");
    let after =
        if cursor.is_empty() {
            None
        } else {
            Some(cursor.parse::<SkillId>().map_err(|_| {
                ApiError::new(400, "invalid_cursor", "The listing cursor is invalid.")
            })?)
        };
    let limit = match query.get("limit") {
        Some(value) => value
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=200).contains(n))
            .ok_or_else(|| {
                ApiError::new(
                    400,
                    "invalid_limit",
                    "List limit must be between 1 and 200.",
                )
            })?,
        None => 100,
    };
    let (after_owner, after_name) = after
        .as_ref()
        .map(|id| (id.owner_id.as_str(), id.name.as_str()))
        .unwrap_or(("", ""));
    let result = env.d1("DB")?.prepare("SELECT owner_id,name,sha256,size_bytes,description,updated_at FROM skills WHERE (?1='' OR owner_id=?1) AND (?2='' OR owner_id>?2 OR (owner_id=?2 AND name>?3)) ORDER BY owner_id,name LIMIT ?4")
        .bind(&[owner.into(),after_owner.into(),after_name.into(),JsValue::from_f64((limit+1) as f64)])?.all().await?;
    successful(&result)?;
    let mut skills: Vec<SkillMetadata> = result.results()?;
    let next_cursor = if skills.len() > limit {
        skills.truncate(limit);
        skills
            .last()
            .map(|skill| format!("{}/{}", skill.owner_id, skill.name))
    } else {
        None
    };
    Ok(Response::from_json(&SkillList {
        skills,
        next_cursor,
    })?)
}

/// Revalidate exact latest metadata without ever serving mutable edge-cache content.
pub(crate) async fn metadata(request: &Request, env: &Env, id: &SkillId) -> ApiResult<Response> {
    let record = find(env, id).await?.ok_or_else(not_found)?;
    if matches_etag(request, "if-none-match", &record.metadata.sha256)? {
        return with_etag(Response::empty()?.with_status(304), &record.metadata.sha256);
    }
    with_etag(
        Response::from_json(&record.metadata)?,
        &record.metadata.sha256,
    )
}

/// Resolve one D1 pointer and stream exactly those R2 bytes through the platform.
pub(crate) async fn download(request: &Request, env: &Env, id: &SkillId) -> ApiResult<Response> {
    let record = find(env, id).await?.ok_or_else(not_found)?;
    let expected = request
        .url()?
        .query_pairs()
        .find(|(key, _)| key == "sha256")
        .map(|(_, v)| v.into_owned());
    if let Some(expected) = expected {
        validate_sha256(&expected)
            .map_err(|e| ApiError::new(400, "invalid_digest", e.to_string()))?;
        if expected != record.metadata.sha256 {
            return Err(ApiError::new(
                409,
                "archive_changed",
                "The skill changed while downloading. Fetch its current metadata and try again.",
            ));
        }
    }
    if request.headers().get("if-match")?.is_some()
        && !matches_etag(request, "if-match", &record.metadata.sha256)?
    {
        return Err(ApiError::new(
            412,
            "archive_changed",
            "The skill changed. Fetch its current metadata and try again.",
        ));
    }
    if matches_etag(request, "if-none-match", &record.metadata.sha256)? {
        return with_etag(Response::empty()?.with_status(304), &record.metadata.sha256);
    }
    let object = env
        .bucket("SKILLS")?
        .get(&record.storage_key)
        .execute()
        .await?
        .ok_or_else(|| {
            ApiError::new(
                503,
                "archive_unavailable",
                "This skill archive is temporarily unavailable. Try again.",
            )
        })?;
    if object.size() != record.metadata.size_bytes {
        return Err(ApiError::new(
            503,
            "archive_unavailable",
            "This skill archive is temporarily unavailable. Try again.",
        ));
    }
    let body = object.body().ok_or_else(|| {
        ApiError::new(
            503,
            "archive_unavailable",
            "This skill archive is temporarily unavailable. Try again.",
        )
    })?;
    let mut response = Response::from_body(body.response_body()?)?;
    response
        .headers_mut()
        .set("content-type", SKILL_MEDIA_TYPE)?;
    response
        .headers_mut()
        .set("content-length", &record.metadata.size_bytes.to_string())?;
    response.headers_mut().set(
        "content-disposition",
        &format!("attachment; filename=\"{}.skill\"", id.name),
    )?;
    with_etag(response, &record.metadata.sha256)
}

/// Build one canonical content validator; R2's multipart/object ETag is not SHA-256.
fn with_etag(mut response: Response, hash: &str) -> ApiResult<Response> {
    response.headers_mut().set("etag", &format!("\"{hash}\""))?;
    response.headers_mut().set("x-mskill-sha256", hash)?;
    Ok(response)
}

/// Comma-separated HTTP validators permit weak comparison for If-None-Match only.
fn matches_etag(request: &Request, header: &str, hash: &str) -> ApiResult<bool> {
    let expected = format!("\"{hash}\"");
    Ok(request.headers().get(header)?.is_some_and(|value| {
        value.split(',').any(|v| {
            let value = v.trim();
            value == "*"
                || value == expected
                || (header == "if-none-match"
                    && value.strip_prefix("W/").is_some_and(|v| v == expected))
        })
    }))
}

/// Create/replace the sole latest state after full portable validation.
pub(crate) async fn publish(env: &Env, id: &SkillId, bytes: Vec<u8>) -> ApiResult<Response> {
    let inspected = validate_archive(&bytes)
        .map_err(|e| ApiError::new(400, "invalid_archive", e.to_string()))?;
    if inspected.name != id.name {
        return Err(ApiError::new(
            400,
            "skill_name_mismatch",
            "The root SKILL.md name must match the published skill name.",
        ));
    }
    let hash = sha256_hex(&bytes);
    let previous = find(env, id).await?;
    if let Some(previous) = previous.as_ref().filter(|row| row.metadata.sha256 == hash) {
        return with_etag(Response::from_json(&previous.metadata)?, &hash);
    }
    let key = format!(
        "skills/{}/{}/{}/{}",
        id.owner_id,
        id.name,
        hash,
        random_hex(16)?
    );
    let metadata = SkillMetadata {
        owner_id: id.owner_id.clone(),
        name: id.name.clone(),
        sha256: hash.clone(),
        size_bytes: bytes.len() as u64,
        description: inspected.description,
        updated_at: timestamp(),
    };
    let db = env.d1("DB")?;
    let now = Date::now().as_millis();
    let result = db
        .prepare("INSERT INTO uploads(storage_key,expires_at) VALUES(?1,?2)")
        .bind(&[
            key.as_str().into(),
            JsValue::from_f64((now + UPLOAD_LEASE_MS) as f64),
        ])?
        .run()
        .await?;
    successful(&result)?;
    let bucket = env.bucket("SKILLS")?;
    if let Err(error) = bucket.put(&key, bytes).execute().await {
        cleanup_upload(env, &key).await;
        return Err(error.into());
    }
    let previous_key = previous
        .as_ref()
        .map(|row| row.storage_key.as_str())
        .unwrap_or("");
    // Batch is one D1 transaction: compare-and-swap, garbage outbox, lease consumption.
    let commit = db.prepare("INSERT INTO skills(owner_id,name,sha256,size_bytes,description,updated_at,storage_key) SELECT ?1,?2,?3,?4,?5,?6,?7 WHERE EXISTS(SELECT 1 FROM uploads WHERE storage_key=?7 AND expires_at>?9) ON CONFLICT(owner_id,name) DO UPDATE SET sha256=excluded.sha256,size_bytes=excluded.size_bytes,description=excluded.description,updated_at=excluded.updated_at,storage_key=excluded.storage_key WHERE skills.storage_key=?8 RETURNING owner_id,name,sha256,size_bytes,description,updated_at")
        .bind(&[metadata.owner_id.as_str().into(),metadata.name.as_str().into(),hash.as_str().into(),JsValue::from_f64(metadata.size_bytes as f64),metadata.description.as_str().into(),metadata.updated_at.as_str().into(),key.as_str().into(),previous_key.into(),JsValue::from_f64(Date::now().as_millis() as f64)])?;
    let garbage = db.prepare("INSERT OR IGNORE INTO garbage(storage_key,delete_after) SELECT ?1,?2 WHERE ?1<>'' AND NOT EXISTS(SELECT 1 FROM skills WHERE storage_key=?1)")
        .bind(&[previous_key.into(),JsValue::from_f64((Date::now().as_millis()+GARBAGE_GRACE_MS) as f64)])?;
    let consumed = db
        .prepare("DELETE FROM uploads WHERE storage_key=?1")
        .bind(&[key.as_str().into()])?;
    let result = match db.batch(vec![commit, garbage, consumed]).await {
        Ok(result) => result,
        Err(error) => {
            // A timeout may conceal a committed pointer; only remove an unreferenced key.
            cleanup_upload(env, &key).await;
            return Err(error.into());
        }
    };
    for result in &result {
        successful(result)?;
    }
    let committed: Vec<SkillMetadata> = result[0].results()?;
    if committed.is_empty() {
        cleanup_upload(env, &key).await;
        return Err(ApiError::new(
            409,
            "publish_conflict",
            "Another update changed this skill. Publish again to replace the current state.",
        ));
    }
    with_etag(
        Response::from_json(&metadata)?.with_status(if previous.is_some() { 200 } else { 201 }),
        &hash,
    )
}

/// Clean a failed upload only after checking the authoritative latest pointer.
async fn cleanup_upload(env: &Env, key: &str) {
    let cleanup = async {
        let db = env.d1("DB")?;
        // Revoke the commit lease before checking liveness. A delayed, ambiguously
        // acknowledged commit can no longer install a pointer after this batch.
        let revoked = db.prepare("DELETE FROM uploads WHERE storage_key=?1").bind(&[key.into()])?;
        let garbage = db.prepare("INSERT INTO garbage(storage_key,delete_after) SELECT ?1,?2 WHERE NOT EXISTS(SELECT 1 FROM skills WHERE storage_key=?1) ON CONFLICT(storage_key) DO UPDATE SET delete_after=excluded.delete_after RETURNING storage_key")
            .bind(&[key.into(),JsValue::from_f64(Date::now().as_millis() as f64)])?;
        let results = db.batch(vec![revoked,garbage]).await?;
        if results.iter().any(|result|!result.success()) { return Err(worker::Error::RustError("cleanup transaction failed".into())); }
        if !results[1].results::<GarbageKey>()?.is_empty() {
            env.bucket("SKILLS")?.delete(key).await?;
            // Keep the durable outbox until Cron verifies deletion again. This
            // also recovers a late R2 write after an interrupted upload request.
        }
        Ok::<_,worker::Error>(())
    }.await;
    if cleanup.is_err() {
        crate::telemetry::log_event(
            &serde_json::json!({"event":"upload_cleanup","outcome":"retry_required"}),
            true,
        );
    }
}

/// Remove metadata and durably enqueue its blob in a single transaction.
pub(crate) async fn remove(env: &Env, id: &SkillId) -> ApiResult<Response> {
    let db = env.d1("DB")?;
    let bindings = [
        id.owner_id.as_str().into(),
        id.name.as_str().into(),
        JsValue::from_f64((Date::now().as_millis() + GARBAGE_GRACE_MS) as f64),
    ];
    let garbage = db.prepare("INSERT OR IGNORE INTO garbage(storage_key,delete_after) SELECT storage_key,?3 FROM skills WHERE owner_id=?1 AND name=?2").bind(&bindings)?;
    let remove = db
        .prepare("DELETE FROM skills WHERE owner_id=?1 AND name=?2 RETURNING storage_key")
        .bind(&bindings[..2])?;
    let results = db.batch(vec![garbage, remove]).await?;
    for result in &results {
        successful(result)?;
    }
    if results[1].results::<GarbageKey>()?.is_empty() {
        return Err(not_found());
    }
    Ok(Response::empty()?.with_status(204))
}

/// Missing packages have a stable CLI-actionable failure rather than a generic 500.
fn not_found() -> ApiError {
    ApiError::new(
        404,
        "skill_not_found",
        "This skill is not published in that namespace.",
    )
}

/// Retry at most 100 outbox deletions plus 100 orphan candidates per invocation.
pub(crate) async fn collect_garbage(env: &Env) -> ApiResult<usize> {
    let db = env.d1("DB")?;
    let bucket = env.bucket("SKILLS")?;
    let now = Date::now().as_millis();
    let expired = db.prepare("INSERT OR IGNORE INTO garbage(storage_key,delete_after) SELECT storage_key,?1 FROM uploads WHERE expires_at<=?1 ORDER BY expires_at,storage_key LIMIT 100").bind(&[JsValue::from_f64(now as f64)])?;
    let remove_leases = db.prepare("DELETE FROM uploads WHERE storage_key IN(SELECT storage_key FROM uploads WHERE expires_at<=?1 ORDER BY expires_at,storage_key LIMIT 100)").bind(&[JsValue::from_f64(now as f64)])?;
    for result in db.batch(vec![expired, remove_leases]).await? {
        successful(&result)?;
    }
    let due = db.prepare("SELECT storage_key FROM garbage WHERE delete_after<=?1 ORDER BY delete_after LIMIT 100").bind(&[JsValue::from_f64(now as f64)])?.all().await?;
    successful(&due)?;
    let due_keys: Vec<String> = due
        .results::<GarbageKey>()?
        .into_iter()
        .map(|key| key.storage_key)
        .collect();
    let reclaimed = unreferenced(env, &due_keys).await?;
    let mut deleted = reclaimed.len();
    if !reclaimed.is_empty() {
        bucket.delete_multiple(reclaimed).await?;
    }
    // Only acknowledge the durable outbox after R2 confirms the whole batch.
    // Partial/unknown failures retain retryable jobs; R2 deletes are idempotent.
    if !due_keys.is_empty() {
        let sql = format!(
            "DELETE FROM garbage WHERE storage_key IN({})",
            placeholders(due_keys.len())
        );
        let bindings: Vec<JsValue> = due_keys.iter().map(|key| key.as_str().into()).collect();
        let result = db.prepare(sql).bind(&bindings)?.run().await?;
        successful(&result)?;
    }
    let cursor = db
        .prepare("SELECT value FROM maintenance WHERE name='orphan_cursor'")
        .first::<String>(Some("value"))
        .await?
        .unwrap_or_default();
    let mut scan = bucket.list().prefix("skills/").limit(100);
    if !cursor.is_empty() {
        scan = scan.cursor(&cursor);
    }
    let objects = scan.execute().await?;
    let candidates: Vec<String> = objects
        .objects()
        .into_iter()
        .filter(|object| {
            object
                .uploaded()
                .as_millis()
                .saturating_add(UPLOAD_LEASE_MS)
                <= now
        })
        .map(|object| object.key())
        .collect();
    let reclaimed = unreferenced(env, &candidates).await?;
    deleted += reclaimed.len();
    if !reclaimed.is_empty() {
        bucket.delete_multiple(reclaimed).await?;
    }
    let result = db.prepare("INSERT INTO maintenance(name,value) VALUES('orphan_cursor',?1) ON CONFLICT(name) DO UPDATE SET value=excluded.value")
        .bind(&[objects.cursor().unwrap_or_default().into()])?.run().await?;
    successful(&result)?;
    Ok(deleted)
}

/// Find reclaimable write-unique keys in one bounded query. Pending upload leases
/// count as references; consumed/revoked keys can never gain a future pointer.
async fn unreferenced(env: &Env, keys: &[String]) -> ApiResult<Vec<String>> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    if keys.len() > 100 {
        return Err(ApiError::new(
            503,
            "cleanup_batch_too_large",
            "Registry maintenance will retry with a bounded batch.",
        ));
    }
    let parameters = placeholders(keys.len());
    let sql = format!("SELECT storage_key FROM skills WHERE storage_key IN({parameters}) UNION ALL SELECT storage_key FROM uploads WHERE storage_key IN({parameters})");
    let bindings: Vec<JsValue> = keys.iter().map(|key| key.as_str().into()).collect();
    let result = env.d1("DB")?.prepare(sql).bind(&bindings)?.all().await?;
    successful(&result)?;
    let live: HashSet<String> = result
        .results::<GarbageKey>()?
        .into_iter()
        .map(|key| key.storage_key)
        .collect();
    Ok(keys
        .iter()
        .filter(|key| !live.contains(*key))
        .cloned()
        .collect())
}

/// Reuse numbered bind slots in both UNION branches to stay within D1's 100-slot
/// limit. Values are always bound separately and never interpolated into SQL.
fn placeholders(count: usize) -> String {
    (1..=count)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A maximal batch uses exactly 100 numbered slots, not 200 UNION parameters.
    #[test]
    fn cleanup_parameters_are_bounded() {
        assert_eq!(placeholders(1), "?1");
        let parameters = placeholders(100);
        assert_eq!(parameters.split(',').count(), 100);
        assert!(parameters.ends_with(",?100"));
        assert!(!parameters.contains("?101"));
    }
}

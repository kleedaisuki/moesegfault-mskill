# Registry API v1

The production origin is `https://skills.moesegfault.dev`. Staging is configured
explicitly; clients must never silently cross Identity or registry environments.
All paths below are relative to that origin. JSON uses UTF-8. Additive JSON fields
are compatible; clients ignore unknown fields. Shared Rust types and limits live
in `crates/mskill-protocol`.

## Endpoints

| Method and path | Authentication | Success |
| --- | --- | --- |
| `GET /health` | None | Service health JSON |
| `GET /v1/me` | Bearer access token | `200 UserProfile` |
| `GET /v1/skills?owner_id=<id>&limit=100&cursor=<marker>` | None | `200 SkillList` |
| `GET /v1/skills/<owner_id>/<name>` | None | `200 SkillMetadata` |
| `GET /v1/skills/<owner_id>/<name>/archive?sha256=<expected>` | None | `200` raw `.skill` ZIP |
| `PUT /v1/skills/<owner_id>/<name>` | Bearer access token belonging to owner | `200/201 SkillMetadata` |
| `DELETE /v1/skills/<owner_id>/<name>` | Bearer access token belonging to owner | `204` |

The service derives the account from `(iss,sub)` and rejects writes to any other
owner namespace. `local` is reserved and cannot be published. Public reads do not
require an account. An optional owner filter is exact; listing order is
`(owner_id,name)`. Limits default to 100, maximum 200. Follow `next_cursor` until
absent; continuation markers are opaque to clients.

## JSON contracts

```json
{
  "owner_id": "opaque_account_id",
  "name": "rust-review",
  "sha256": "64_lowercase_hex_characters",
  "size_bytes": 1234,
  "description": "Review Rust ownership and error handling.",
  "updated_at": "2026-10-05T06:00:00Z"
}
```

This is `SkillMetadata`. `SkillList` is `{"skills": [...], "next_cursor": "..."}`;
the cursor is absent on the final page. `UserProfile` is
`{"owner_id":"...","display_name":"optional presentation value"}`. Account
IDs do not reveal Identity subjects. Timestamps identify last state change, not
a package version or an update ordering mechanism.

## Publish and latest-only semantics

Publish uses `Content-Type: application/vnd.mskill.skill` with the ZIP as its raw
body. The server computes SHA-256, validates the entire portable ZIP, and requires
root frontmatter `name` to equal the route name. Do not trust a client hash header
or publish metadata separately from its body. Archive validation is specified in
[architecture.md](architecture.md). A repeated identical publish returns the same
latest bytes and need not change the timestamp. A changed hash replaces the latest
state. No route lists or restores historical versions.

Optional write guards prevent replacing a state the user did not inspect:
`If-None-Match: *` creates only (`412 skill_exists` on a collision), and
`If-Match: "<sha256>"` permits replacement/deletion only while that digest is
current (`412 archive_changed` otherwise). Guards participate in the durable
commit/delete transaction. Existing clients omitting guards keep their contract.
The cookie-protected `POST /web/publish` infers the name from the validated root
manifest and uses the same latest-only publication path; see
[web-backend.md](web-backend.md) for browser-only session/CSRF routes.

The metadata commit follows a durable immutable blob write. Concurrent conflicting
mutations may return `409 publish_conflict`; a client can fetch current metadata
and ask the user to republish. Do not retry a write blindly in a way that overwrites
another writer. Delete removes the latest pointer immediately; delayed blob cleanup
is an internal failure-recovery detail.

## Download and update

The body is `application/vnd.mskill.skill`. `ETag` is the quoted lowercase archive
SHA-256. `If-None-Match` can return `304` when the latest digest matches. Mutable
metadata/archive endpoints use `Cache-Control: no-store`.

For an update: fetch metadata, compare local hash, and skip unchanged bytes. If it
changed, request `/archive?sha256=<observed digest>`, independently verify returned
SHA-256, inspect the ZIP, then commit local state. A raced latest pointer returns
`409 archive_changed`; refetch metadata and retry a bounded number of times.
Downloaded ZIP bytes and recorded metadata must never be committed under different
hashes. SHA-256 proves byte consistency, not publisher trust or content safety.

## Errors and telemetry

Errors use `application/problem+json`:

```json
{
  "type": "about:blank",
  "title": "Skill not found",
  "status": 404,
  "detail": "No published skill exists at this owner/name.",
  "error_code": "skill_not_found",
  "request_id": "opaque_request_id"
}
```

Clients branch on stable `error_code`, not translated prose. Important cases are
400 invalid identity/archive, 401 missing/invalid access token, 403 wrong owner,
404 unknown skill, 409 publish conflict/archive changed, 413 archive too large,
429 resource/rate limit, and 5xx transient service failure. Server diagnostics must
not expose tokens, signing material, raw Identity subjects or storage internals.

Requests may send `x-mskill-request-id` and W3C `traceparent`. The Worker validates
length/format before propagation, returns its canonical `x-mskill-request-id`, and
logs correlation with stable route labels and latency. Identity's
`x-moesegfault-correlation-id` is preserved in CLI diagnostics where present, not
repurposed as authentication. A client trace does not imply that a platform has
adopted that exact trace ID; application logs remain the cross-boundary fallback.

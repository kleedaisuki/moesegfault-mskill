# Browser workspace and community backend

## Integration decisions (2026-10-05)

The native CLI bearer boundary remains unchanged: only the registered native
client audience authorizes bearer API requests. Browser participation is a
same-origin confidential BFF, registered separately as `mskill-web` (production)
and `mskill-web-staging` (staging), with the native client's existing pairwise
subject sector. Identity registration and private key custody are deployment-owned.
Tokens never enter browser storage or frontend JSON.

Server token columns contain AES-GCM envelopes, not JWT/refresh plaintext. The
per-environment `WEB_SESSION_KEY` secret is exactly 32 random bytes encoded as
unpadded base64url. Each encryption uses a fresh 96-bit nonce and authenticated
data binds the session hash plus token role, preventing ciphertext swapping.

Login uses issuer-pinned discovery, private-key JWT, S256 PKCE, a browser-bound
single-use transaction, signed ID-token nonce validation, and host-only cookies.
D1 stores opaque session hashes, server tokens, and a durable refresh lease.
Rotation is serialized across isolates; an ambiguous abandoned rotation ends the
session instead of replaying its predecessor. Mutating cookie requests require
the configured exact Origin plus `X-CSRF-Token`. Browser sessions do not widen the
native bearer audience.

## Frontend contracts

| Route | Contract |
| --- | --- |
| `GET /web/session` | `{user: null | {owner_id, display_name}, csrf_token}`; no OAuth material |
| `GET /auth/login?return_to=...` | Top-level redirect; return path must be local |
| `POST /auth/logout` | Clears local session and revokes refresh family |
| `POST /web/publish` | Protected raw `.skill` upload, server-inferred manifest name; returns `SkillMetadata` |
| `POST /auth/logout?identity=1` | Also returns an opaque same-origin SSO handoff path; no token JSON |
| `GET /auth/logout/identity`, `/auth/logout/callback` | Browser-bound one-time issuer logout with checked state |
| `PUT/DELETE /v1/skills/{owner}/{name}` | Existing archive contract; also accepts protected cookie session |
| `GET /v1/community?limit=&cursor=&q=` | Recent enriched skills, optional bounded text search |
| `GET /v1/accounts/{owner}` | Public name, publisher ID, skill/comment counts; no issuer/subject |
| `GET /v1/skills/{owner}/{name}/preview?path=` | Latest hash, SKILL.md plain text, bounded file tree and selected text |
| `GET/POST /v1/skills/{owner}/{name}/comments` | Public pagination; authenticated `{body}` creation |
| `PATCH/DELETE /v1/skills/{owner}/{name}/comments/{id}` | Author edits/deletes; package owner may delete moderation |

Preview never executes or serves archive HTML. It returns JSON strings for safe
frontend rendering. Stored immutable blobs were fully validated at publication;
preview verifies byte digest and reads a bounded selected UTF-8 resource. Initial
SKILL.md retains its existing 1 MiB bound; secondary text is at most 256 KiB.
Comments are plain text, at most 4096 Unicode characters/16 KiB, with request-body
limits enforced before collection. All SQL uses bound parameters. Package deletion
cascades its comments, without retaining a historical package state.

## Sources

- Internal authoritative Identity onboarding/OIDC contracts under
  `.agents/skills/moesegfault-identity/references/`.
- [OAuth Security BCP, RFC 9700](https://www.rfc-editor.org/rfc/rfc9700.html):
  PKCE, exact redirect trust boundaries, replay prevention.
- [Cloudflare D1 query guidance](https://developers.cloudflare.com/d1/best-practices/query-d1/):
  parameter binding, SQLite transactions, foreign-key relationships.

Runtime user-journey evidence is recorded after integration, not inferred from
these contracts or a successful build.

## Upload intent and stale-view protection

Browser creation sends `If-None-Match: *`; an existing same-name publication
returns `412 skill_exists`, including insertion races at commit. Explicit replace
and delete send `If-Match: "<current sha256>"`; a stale view returns
`412 archive_changed`. Named native PUT/DELETE without these headers preserve
their established semantics. A durable pointer CAS also rejects deletion between
an upload's initial read and commit rather than resurrecting the removed package.

The web route does not parse ZIP/YAML in JavaScript or upload twice for inspection.
Server-side `ValidatedUpload` privately owns immutable bytes and the shared
inspection proof; inferred-name and named publication use the same commit path,
with one full portable archive validation before any R2 write.

## Initial schema validation

The actual mutation SQL extracted from `community.rs` passed SQLite with foreign
keys enabled: author-only edits, package-owner moderation, an atomic 60-comments
per-hour budget, and cascade removal on package deletion. Reproduction helper and
machine result are `.temp/web-backend/check-sql.py` and `sql-results.json`.
This is narrow schema/authorization evidence; it does not replace the deployed
browser account and upload/comment user journey.

## Integrated build and native compatibility checkpoint

The final inferred-name publication and transactional write-precondition sources
compiled successfully to Wasm (incremental worker build 16.81 seconds). The existing
54-command actual CLI plus real local Worker regression passed after these changes.
This establishes native/public compatibility, not yet the new confidential browser
login/community journey. The SQLite reproduction additionally exercises commit
create races, deletion during an upload, and stale conditional deletion.

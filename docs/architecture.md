# mskill architecture

## Product boundary

mskill manages reusable agent skills as portable `.skill` ZIP files. A native
Rust CLI owns a local library; a pure Rust Cloudflare Worker owns the shared
registry at `skills.moesegfault.dev`. Project installation expands a skill into
the standard `.agents/skills/<name>/` directory, by copying or linking its tree.
There is one current state per identity. There are no versions, release histories,
dependency solvers, install hooks, or implicit code execution.

```text
Author's SKILL directory -> deterministic .skill ZIP -> ~/.mskill library
                                                   |           |
                                                   |           +-> project copy/link
                                                   +-> CLI publish -> Rust Worker
                                                                       |    |
                                                            latest row D1    R2 ZIP
                                                   <- CLI pull/update <-----+
```

## Identity and ownership

`SkillId = (owner_id, name)`. The registry allocates an immutable opaque owner ID
for the exact Identity `(issuer, sub)` tuple. Usernames, email addresses, and
display names are never primary keys or authorization inputs. Two accounts may
publish the same name. References are explicit `owner_id/name`; there is no
automatic global-name fallback. `local/name` is the client-only namespace for an
unpublished local skill and must not become a registry publisher.

Names are 1..64 lowercase ASCII letters/digits and nonconsecutive internal hyphens, excluding
Windows device names. Owner IDs are URL/path-safe ASCII letters/digits, underscores,
and hyphens, 1..128 bytes, also excluding device names. These constraints are
shared in `mskill-protocol`; validate deserialized data at trust boundaries.

## Local state and user workflows

The CLI/core implementation owns the exact persisted schema. Its invariants are:

1. Canonical transport storage remains a `.skill` ZIP under `~/.mskill`, never an
   OS-specific packed directory. `MSKILL_HOME` permits an isolated user journey.
2. An extracted, stable library path per identity is the target of project links.
   Updating a skill must not leave those links permanently pointing at removed
   content. Project copies do not share identity with mutable local source files.
3. Validate the archive and downloaded SHA-256 before changing committed state.
   Stage work on the same filesystem, serialize mutation with a filesystem lock,
   then replace state with rename and rollback/recovery where required.
4. A rename replacing a nonempty directory is not universally atomic. In
   particular, Windows can require a backup/stage swap. Do not promise that
   unrelated readers can never observe a short missing-path interval. Recovery
   must restore or complete a crashed swap before the next mutation.
5. Project state records which paths mskill installed and whether they are copies
   or links. Removal must not delete unrelated skills. Replacing an existing
   unowned/modified project directory requires an explicit user choice.
6. A project's standard layout is name-only. If two local identities share the
   same name, reject an ambiguous project install; use another project or explicitly remove the previous managed install before
   selecting another publisher. Project directory names equal manifest names.
7. Local removal can break project links and must make this effect clear to the
   user. There is no global cross-project link registry. Cloud deletion never
   deletes local copies automatically.

Representative journey: add a directory -> link to a project -> edit source and
add/update again -> linked project sees the new tree -> clone to another project
-> copies retain their independent contents -> publish under authenticated owner
-> another isolated home pulls -> update replaces only when archive SHA differs.

## Portable archive contract

The root contains a UTF-8 `SKILL.md` with YAML frontmatter `name` and `description`.
There is no wrapper directory. Files and directories only: no symlinks, devices,
encrypted entries, absolute paths, traversal, backslashes, control characters,
Windows-reserved components, duplicate/case-colliding names, or file/parent clashes.
All components must also work on Windows. ZIP64 and split archives are not needed
within the product limits and are rejected. Entry counts are checked before ZIP
metadata allocation. The importer does not follow a source
symlink while packaging and does not execute scripts while inspecting/installing.

Limits: 16 MiB ZIP, 64 MiB expanded bytes, 4096 entries, 1 MiB SKILL.md, 64 KiB
frontmatter, 1024-character nonempty description, 1024-byte entry paths and 255-byte
components. Inspection streams actual bytes so central-directory declarations
cannot bypass expanded limits. Pack sorted paths with fixed timestamps and stable
permissions/compression settings so unchanged content has unchanged archive SHA.
An executable script may retain its executable mode; the importer does not run it.

Raw central-directory records are checked before the ZIP library builds its
name-indexed view. This is required because the library can deduplicate identical
names; validation after indexing alone missed an independently generated duplicate
`SKILL.md` fixture during the first real CLI journey. Central counts/bounds and
local/central name agreement are checked, then the maintained ZIP library handles
decompression and CRC verification. The independent fixture remains in the E2E
harness and a raw duplicate-record regression is included in protocol tests.
After the correction, the rebuilt protocol suite passed six unit tests and one
documentation test. The real Windows CLI rejected the original duplicate fixture
and the other hostile archives; pack/export/import roundtrip succeeded. These
results cover the archive contract, not the remaining authentication or deployment
acceptance workflow.

`mskill-protocol::validate_archive` is the shared native/WASM inspector. Core owns
safe extraction/packing, local state and project installation. Protocol owns wire
types and portable constraints only, with no filesystem/platform dependencies.

## Registry storage, mutation and failure behavior

D1 owns account mappings and one latest metadata pointer per `(owner_id,name)`;
private R2 owns ZIP blobs. Write a new validated blob before committing metadata,
using a write-unique key so concurrent publish/delete cannot remove a newly live
blob with identical content. The D1 pointer commit is the mutation linearization
point. Never overwrite a mutable shared R2 key and then separately update D1.

See [worker-storage.md](worker-storage.md) for CAS conflict behavior, bounded
garbage collection, read consistency and staged blob recovery. Cleanup-only
superseded objects are not a version history and cannot be downloaded by an old
version API. Mutable routes bypass edge caches. A download can require the digest
observed by the client; a concurrent publish causes a bounded retry, not a
metadata/archive mismatch.

## Authentication and environment pairing

The CLI is a registered native/public OIDC client using system-browser
Authorization Code with S256 PKCE and a registered loopback callback. It has no
client secret. Refresh material belongs in protected OS storage; access to it is
serialized and rotated token sets are persisted before releasing the lock.
The Worker validates RS256 signatures, pinned issuer, exact registered client-ID
audience, time bounds, `token_use=access`, and required scope before account mapping.
The current Identity audience is the native client's ID, not an invented API ID.

Local fixtures are explicit local-only development aids. Real login testing uses
staging Identity and staging registry configuration; it must not change production
Identity data. Production and staging have separate fixed issuers, public-client
registrations, redirect allowlists and Worker variables. Client registration is a
deployment-owned prerequisite, not something inferred from a manifest or created
by ordinary user login. Refer to [identity-onboarding.md](identity-onboarding.md)
for exact setup. Local Identity integration guidance informed the implementation.

## Observability and resource discipline

The CLI supplies validated W3C `traceparent` and a request ID on remote journeys;
the Worker returns a request ID and structured route/status/duration/outcome logs.
Enable Cloudflare native logs and traces explicitly in Wrangler; automatic
platform spans cover bindings where supported. Preserve application correlation
even if a platform does not adopt the inbound trace ID. Tokens, authorization
codes, Identity subjects, email and raw archive bodies never enter telemetry.

CI builds/tests the shared contracts and user journeys, caches Rust artifacts,
uses bounded timeouts/concurrency, and keeps deployment secrets out of PR jobs.
Do not run a large local benchmark matrix during initial delivery. After functional
journeys pass, measure startup and realistic large-file/many-file archive tails;
optimize the demonstrated bottleneck rather than changing compression speculatively.

## Implementation and integration order

1. Shared protocol and portable archive invariants.
2. Core local archive/library/project workflows and a real CLI journey.
3. Worker D1/R2 routes with explicit local fixtures, publish/pull/delete journey.
4. Native Identity registration/login/refresh/logout and an authenticated staging
   publish/pull/update/delete journey, including same-name different-owner isolation.
5. Distribution skills, user documentation, GitHub Actions and deployment.
6. Targeted measured optimizations while preserving wire/data behavior.

## Decisions and external signals

- Directory/frontmatter naming and description bounds follow the [Agent Skills
  specification](https://agentskills.io/specification). Portable paths additionally
  account for [Windows filename rules](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file), including superscript DOS device aliases.
- D1 plus private R2 uses mature platform storage instead of building a storage
  layer. R2 is strongly consistent through bindings; a cached custom-domain URL
  changes that guarantee, hence downloads go through the Worker. [R2 consistency](https://developers.cloudflare.com/r2/reference/consistency/).
- Native browser PKCE follows platform practice and avoids a fictional client
  secret. [RFC 8252](https://www.rfc-editor.org/rfc/rfc8252.html), [RFC 9700](https://www.rfc-editor.org/rfc/rfc9700.html).
- Cloudflare logs and native traces avoid embedding a heavy exporter in every
  request. Their separate enable/sampling controls are operational settings,
  not promises of permanent all-request retention. [Workers traces](https://developers.cloudflare.com/workers/observability/traces/).
- Peer-reviewed package-confusion research identifies semantic as well as spelling
  confusion mechanisms. Explicit immutable publisher namespaces address ambiguity,
  while SHA-256 only detects changed bytes. This does not establish malicious-skill
  detection; content scanning/signatures are not prerequisites to first delivery.
  [Neupane et al., USENIX Security 2023](https://www.usenix.org/conference/usenixsecurity23/presentation/neupane).
- Recent peer-reviewed industry research finds adoption and usability barriers to
  artifact signing. Defer a custom trust/signature infrastructure until there is a
  concrete multi-party verification workflow; keep publisher identity and transport
  integrity distinct now. [Kalu et al., USENIX Security 2025](https://www.usenix.org/conference/usenixsecurity25/presentation/kalu).

Rejected initially: semantic versions/dependency resolution, an install-time hook
engine, mutable global unqualified package names, KV latest-state pointers, a
JavaScript application backend, and a custom telemetry service. None improves the
dominant workflows enough to justify its failure and maintenance surface.

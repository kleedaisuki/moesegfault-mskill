# Registry storage and operations

The registry is a Rust Cloudflare Worker (`worker` 0.8.7). D1 owns identity mapping
and the single latest metadata pointer. Private R2 owns `.skill` ZIP bytes.
The website is served by the same Worker; it is not a separate JavaScript backend.

## Publish and delete consistency

1. Validate ownership and bounded ZIP input before writing storage.
2. Register a one-hour D1 upload lease, then upload to a write-unique key containing
   owner, name, SHA-256, and random operation ID. A revoked/expired lease cannot commit.
3. Compare-and-swap the D1 pointer against the previously observed storage key.
   Return `409 publish_conflict` if another mutation won. The losing upload is deleted.
4. Enqueue the superseded key in the same D1 batch as the pointer commit. Deletion
   waits ten minutes so a request that already observed old metadata can finish.
5. Failed/ambiguously acknowledged uploads revoke their lease and atomically check
   the latest pointer before deletion. A delayed D1 commit cannot install a missing blob.
6. Delete is one D1 batch: enqueue the current key and remove the pointer. It has
   no historical API and immediately disappears from listing and future downloads.

Write-unique keys eliminate the content-hash ABA race: a concurrent publisher can
never reuse a blob that a delete/cleanup request is about to remove. SHA-256 is an
integrity token, not a user-visible version. Temporary garbage is operational
cleanup state, not a retained release/history feature.

Every ten minutes, a bounded collector retries the D1 outbox and scans at most
100 R2 entries for interrupted uploads older than one hour. The collector verifies
that no current pointer references an object before deletion. A one-hour upload
lease is checked inside the pointer commit, and expired leases are revoked in the
same transaction that queues them for reclamation. Orphan scans retain their cursor
in D1 so large buckets make forward progress. A failed collector leaves live data
intact and reports one structured error; subsequent invocations retry.

### Bounded batch reclamation

After the complete user journey passed, the collector's per-object queries were
replaced with bounded batches. A maximum round processes 100 expired leases,
100 durable outbox keys and 100 orphan candidates. Each candidate set is checked
against both current pointers and pending leases in one parameterized query.
Numbered `?1`..`?100` parameters are reused in both UNION branches, respecting
D1's 100-bound-parameter limit. R2 `delete_multiple` performs at most one deletion
call per candidate set. Outbox acknowledgement happens only after R2 success;
partial/ambiguous failures leave idempotent retry work and do not advance the
orphan cursor. Both expiration statements select the identical deterministic
100-row lease set inside the transaction.

The maximal round now uses **8 D1 statements and 3 R2 operations**, rather than
up to 305 D1 statements and 201 R2 operations (assuming all scanned objects are
old and unreferenced). This avoids the documented Free plan's 50-query invocation
budget and materially reduces Paid-plan round trips. Empty rounds skip batch
lookups/deletion and use fewer calls. There is no increased concurrency, extra
service, historical package state, or new mutation contract.

A targeted SQLite 3.50.4 / Python 3.14.6 probe verified a full 100-slot batch:
10 current pointers and 10 pending leases remained protected, 80 other candidates
were reclaimable, and 120 expired leases were revoked in deterministic 100+20
transactions with no garbage/active-lease overlap. The experiment stays under
`.temp/gc-batch-check.py`. The actual D1/R2 scheduled-trigger regression is the
delivery acceptance check, not inferred from this SQL probe.

On 2026-10-05, delivery rebuilt the current Wasm artifact and reran the complete
51-command real CLI + local Worker/D1/R2/RSA-OIDC journey after this batch change.
The actual scheduled-trigger collector passed: deleted publisher entries remained
unavailable and another publisher's live archive retained its independent SHA-256
match. See `docs/e2e.md` for the integrated result. This closes the GC regression
without rerunning unrelated platform matrices or parallel local builds.

Downloads bypass the Worker Wasm heap using R2's native `response_body` stream.
Metadata and mutable download routes use `Cache-Control: no-store`; ETag and
`If-None-Match` compare exact SHA-256 bytes without a stale edge cache.

## Operational contracts

- Uploads: 16 MiB compressed, 64 MiB expanded, 4096 ZIP entries, 1 MiB SKILL.md.
- Lists: keyset pagination, 100 entries default / 200 maximum. `next_cursor` is an
  additive field; callers traverse it rather than silently truncating inventory.
- Mutations require RS256 access tokens for the exact configured Identity issuer
  and native-client audience. Accounts map only `(issuer, sub)`, never names/email.
- Local fixtures require all three gates: local environment, enabled fixture flag,
  and loopback request URL. They cannot authorize production requests.
- D1 read replication must remain disabled unless all reads are moved to Sessions
  with a `first-primary` constraint/bookmark strategy. Current primary reads prevent
  stale metadata resolving a just-collected blob.
- Structured logs include stable route, status, error code, latency, request ID, trace ID and
  outcome, not authorization headers, tokens, subjects, email or raw error strings.
- Cloudflare native traces/logs are explicitly enabled; the application preserves
  validated W3C trace context in response headers and Identity subrequests. When
  the runtime exposes `ctx.tracing`, its active native root span is annotated with
  `mskill.request_id`, `mskill.client_trace_id`, and the stable route template.
  CLI diagnostics, structured logs and the native D1/R2/Fetch span graph can be
  joined by request ID even without native inbound W3C propagation support.

## Sources consulted (2026-10-05)

- [workers-rs 0.8.7 API](https://docs.rs/worker/0.8.7/worker/)
- [R2 consistency](https://developers.cloudflare.com/r2/reference/consistency/)
- [D1 batch API](https://developers.cloudflare.com/d1/worker-api/d1-database/)
- [D1 query and bound-parameter limits](https://developers.cloudflare.com/d1/platform/limits/)
- [Workers native traces](https://developers.cloudflare.com/workers/observability/traces/)
- [Workers span annotations](https://developers.cloudflare.com/workers/observability/traces/custom-spans/)
- [Workers production practices](https://developers.cloudflare.com/workers/best-practices/workers-best-practices/)
- [W3C Trace Context](https://www.w3.org/TR/trace-context/)

Cloudflare native distributed tracing is early-beta functionality. Application
correlation IDs remain available regardless of account-specific trace propagation
support. Do not claim the custom client trace is automatically the platform trace.

## Targeted storage verification

Before Worker integration, the 19 literal prepared statements in `storage.rs` were
extracted and executed/planned against an in-memory SQLite database on Windows
using Python 3.14. The experiment stays under `.temp/storage-sql-check.py`.
This inexpensive check catches SQL-shape errors; it does not substitute for the
actual Wrangler/D1/R2/CLI journey owned by the delivery harness.

| Interleaving | Expected and observed |
| --- | --- |
| Two uploads observed the same prior pointer | First CAS commits; second returns no row |
| A collector revoked an upload lease | A delayed publication returns no row |
| Cleanup follows an ambiguously acknowledged committed write | Referenced latest blob is not enqueued/deleted |
| Cleanup follows a losing staged write | Unreferenced staged blob is durably queued |
| Delete commits before a racing publication inserts | Publication can linearize after delete and become latest |

All cases and all statement `EXPLAIN` checks passed on 2026-10-05. The complete
end-to-end outcomes are recorded separately in the delivery verification notes.

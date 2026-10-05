# Real user-journey acceptance

## Contract and scope

`scripts/e2e.mjs` runs the real Rust CLI as separate user processes against a real local Cloudflare Worker with D1 migrations and R2 storage. `scripts/mock-oidc.mjs` supplies only the identity provider: issuer-pinned discovery, RSA/JWKS signed ID and access tokens, PKCE S256, one-use authorization codes, nonce/state callback propagation, rotating refresh tokens, and revocation. This is not a deployed staging Identity acceptance result.

All mutable data, local database/R2 state, credential fixture files, and diagnostic output are bounded to `.temp/e2e`. Each run resets that directory. The harness does not build Rust artifacts; use the normal bounded build workflow first. The CLI must be built with `e2e-test-store` enabled, and this harness sets `MSKILL_TEST_CREDENTIAL_FILE=1`; that adapter accepts only a loopback HTTP issuer and is not the production OS credential vault.

## Run

```powershell
# After the CLI and Wasm Worker have been built by the delivery workflow:
node scripts/e2e.mjs --cli .cache/target/debug/mskill.exe
```

```sh
node scripts/e2e.mjs --cli .cache/target/debug/mskill
```

The harness invokes the project-local Wrangler through Node (no shell-specific command shims), applies root `migrations` to a fresh local D1 database, and starts the prebuilt Worker at `http://127.0.0.1:8787`. Override with `--registry URL`. OIDC defaults to `http://127.0.0.1:8789`; override with `--issuer-port PORT`.

`--skip-start` uses an externally running Worker; it does not skip any journey assertions. Configure that Worker with `ENVIRONMENT=local`, `LOCAL_DEV_AUTH=false`, `IDENTITY_ISSUER=http://127.0.0.1:8789`, `IDENTITY_CLIENT_ID=mskill-e2e`, D1 `DB`, and R2 `SKILLS`. The harness itself starts OIDC, so do not start a second mock process on that port.

## Acceptance expectations

| User action | Independent expected outcome |
|---|---|
| Pack/export archive | Independent bytes/SHA match, portable import succeeds, existing output is never overwritten |
| Add source directory | `.skill` archive is stored and SHA-256 has canonical shape |
| Clone and link to project | Both expose source manifest and resources under `.agents/skills` |
| Source update | Link reflects latest materialization; clone remains explicit snapshot |
| Managed clone refresh / unmanaged destination | Explicit managed copy refresh succeeds; unmanaged attach/removal fails without changing user-owned files |
| Hostile traversal, duplicate, symlink ZIP | CLI rejects archive without escaping fixture boundary |
| Browserless login | CLI authorization URL leads through mock PKCE endpoint to real loopback callback |
| Publish | Signed bearer token maps to a public account-scoped namespace |
| Pull and no-op update | Downloaded bytes match remote SHA; identical SHA does not rewrite local archive |
| Second account, same skill name | Distinct owner IDs coexist; wrong-account removal fails |
| Expired access | Real command obtains fresh access through rotating refresh token endpoint |
| Cloud replacement | Pull update makes latest manifest available through project link |
| Remove cloud/project/local | Each scope has intended effect; deleted cloud entry cannot be pulled |
| Logout | Provider revocation runs and protected command fails |
| Pipe output | No ANSI escape sequences in captured output |
| Request correlation | Caller request ID is echoed by Worker health response |

## Results and diagnostics

Every run writes `.temp/e2e/results.json` with pass/fail, sanitized command names/arguments, exit codes, durations, and OIDC endpoint counters. No authorization URLs or token responses are logged. A harness-owned Worker additionally writes `.temp/e2e/worker.log`. Command failures abort the run rather than silently skip a check. Fixture credential files are private test data and must not be uploaded as CI artifacts; retain only results and sanitized Worker logs.

Current status: final canonical Windows full acceptance passed with 54 actual CLI commands plus service observations, with independent on-disk name checks. Actual staging CRUD/refresh/publication/logout and public served-asset/API checks also passed. Rendered browser interactions remain unavailable. Detailed historical runs and remaining coverage limits are recorded below.

Coverage limits: production OS vault/real staging Identity, deployed Workers observability dashboard ingestion, a forced compare-and-swap conflict interleaving, and cross-platform CI results are separate acceptance checks. Local response correlation is not proof of dashboard ingestion.

### Native local acceptance

`node scripts/e2e.mjs --local-only --cli PATH` deliberately runs the named local-only acceptance suite without starting OIDC or Workers. Results explicitly record `mode: local-only`; cloud/auth assertions are not reported as verified. This mode requires no auth test-store feature and is suitable for the native operating-system matrix. It covers portable archive import, ZIP attacks, local listing, clone/link, managed and unmanaged destination collisions, latest-source replacement, snapshot behavior, scoped local/project removal, and automatic no-color output through pipes.

`--skip-build` is accepted as a compatibility flag: this harness never builds artifacts, so it does not change behavior. The generated Worker configuration omits the build hook, so Wrangler does not repeat Rust compilation.


Concurrent mutation acceptance launches two authenticated raw ZIP writes simultaneously, accepts documented success or `409 publish_conflict`, and independently checks that latest D1 metadata resolves to exactly one intact R2 body. Results record whether the scheduler actually produced a conflict; two sequential successes are valid and are not falsely described as a reproduced conflict. A stale digest-bound archive read must return `409 archive_changed`. This exercises the service transaction boundary in addition to the CLI journey.


## Independent native run: duplicate ZIP rejection defect

On 2026-10-05, Windows / Node 26.10.0, the first real native journey command was:

```powershell
node scripts/e2e.mjs --local-only --cli .cache/target/debug/mskill.exe
```

Packing and refusal to overwrite an existing `.skill` output passed. Traversal import was rejected. The next adversarial case failed: importing `.temp/e2e/duplicate.skill` returned exit 0 rather than rejecting it. Independent inspection of raw central-directory signatures found exactly three entries named `SKILL.md`. The production CLI installed this archive as `local/hello` (SHA-256 `13aeba2eb7c38dce8542b5f2fcb734ced07773658a2f7e12c02bd367c864b244`). Reproduction:

```powershell
.cache/target/debug/mskill.exe --json --home .temp/e2e/repro-home add .temp/e2e/duplicate.skill
```

Expected: nonzero exit and no package installation. Observed: zero exit with installed metadata. The fixture uses an independent minimal stored-ZIP writer rather than the production packer; the archive contains distinct local file headers and distinct central-directory entries. The original requirement and shared archive safety contract require duplicate rejection. This is a product validation defect, not an unavailable tool or weakened assertion. Delivery and protocol owners were notified. Later steps of that interrupted run remain unexercised.

Mock fixture integrity probe also passed wrong-PKCE rejection, consumed authorization-code replay rejection, rotating refresh-token replacement, old-refresh replay rejection, and revocation-followed-by-refresh rejection. These checks establish that the E2E identity fixture enforces the intended local protocol; they do not validate production Identity itself.

Owned local Workers start with `--test-scheduled`. After replacement/deletion, the harness advances only its private D1 garbage/upload deadlines and invokes the real scheduled entrypoint through `/__scheduled`; it verifies a different user's live R2 archive remains intact and the deleted pointer stays absent. No ten-minute sleep is required. External `--skip-start` servers are not mutated through SQL: results explicitly mark scheduled GC unverified for that mode rather than silently claiming coverage.

## Integration setup repair

The first full local run reached D1 migration but failed before Worker requests: pinned Wrangler 4.130.0 bundles workerd whose latest supported compatibility date is `2026-09-15`; requesting `2026-10-05` prevents runtime startup. The isolated E2E configuration now pins `2026-09-15` (this is a setup/toolchain mismatch, not a tested API failure). Wrangler's own debug-log directory is explicitly redirected using `WRANGLER_LOG_PATH` to `.temp/e2e/wrangler-logs`; sanitization stays enabled and telemetry submission is disabled for the harness.

A subsequent native run by delivery accepted the raw-central-directory duplicate fix, and progressed through copy/link/source updates to Windows project link removal, which failed with OS error 5. The retained path was independently inspected as `Directory, ReparsePoint`, `LinkType: SymbolicLink`, pointing to the local library. Correct removal must unlink the directory symlink itself without deleting its target. Core owns the repair and regression; an integrated rebuilt CLI rerun remains required.

## Native acceptance after fixes

Delivery rebuilt the current Windows CLI and reran `node scripts/e2e.mjs --local-only --cli .cache/target/debug/mskill.exe`: **passed 23 real CLI commands**. This verifies the duplicate-central-directory rejection and Windows directory-symlink removal repairs in the same user workflow, alongside portable pack/export/import, source hash no-op, malicious ZIP rejection, local list, managed clone refresh, unmanaged protection, link-followed source update, snapshot copy and scoped removal. This local-only pass does not claim cloud/auth behavior.

Full integration runs now reach healthy Worker responses (including request ID and fresh-span trace preservation) and complete real browserless PKCE login. The first signed CLI `whoami` returns `503 identity_unavailable`. Removing inherited proxy environment variables did not change the result; the mock received exactly the CLI discovery/JWKS calls, and no Worker discovery call. This discriminates the failure from token signature mismatch and points to Worker metadata request/local transport setup. Backend owns phase diagnostics and repair; no protected registry pass is claimed from those interrupted runs.

### Protected-request root cause: edge redirect mode

Backend phase diagnostics located the failure at `metadata_request` before network fetch. An independent tiny JS Worker against the exact same local workerd performed `new Request(loopbackDiscoveryURL, {redirect: mode})` for all three modes. `manual` and `follow` were accepted; `error` threw:

> Invalid redirect value, must be one of "follow" or "manual" ("error" won't be implemented since it does not make sense at the edge; use "manual" and check the response status code).

Thus the failure is a production-platform API incompatibility, not JWT verification or network reachability. The required repair is `RequestRedirect::Manual` combined with the existing strict HTTP-200 check; this still refuses metadata redirects and does not weaken issuer pinning. The isolated probe and result stayed in `.temp/e2e` and its process tree was terminated. Backend and delivery were notified for production repair and integrated rebuild.

## Full real-process acceptance: PASS

On 2026-10-05, Windows / Node 26.10.0 / pinned Wrangler 4.130.0 / workerd compatibility `2026-09-15`, after the integrated Manual-redirect repair:

```powershell
node scripts/e2e.mjs --cli .cache/target/debug/mskill.exe
```

**PASS: 51 real CLI subprocess commands plus three service/observability observations.** The initially printed count of 54 included three observation events, not subprocesses; reporting now counts only events with an actual CLI exit code.

- Real signed PKCE login, `whoami` account mapping, publish/pull/list, two publishers with the same skill name, local namespace coexistence and project alias collision protection passed.
- Actual expired-access refresh occurred once, and three provider revocations were observed. Post-logout publish and whoami both rejected access.
- SHA-pinned archive bytes independently matched hashes; identical hashes skipped archive rewrite; cloud replacement reached a linked project skill.
- Malicious traversal/duplicate/symlink archives were rejected both by CLI import and by authenticated Worker upload.
- Concurrent independent publish responses were `[200, 200]`; latest D1 pointer resolved to intact matching R2 bytes. The scheduler did **not** expose a `409 publish_conflict` in this run, so that particular conflict branch remains unexercised. Stale digest-bound download did return the expected `409 archive_changed`.
- The actual scheduled GC entrypoint ran against accelerated private D1 deadlines, preserved Bob's live archive SHA, and left Alice's deleted pointer absent.
- Nine Identity trace IDs were recorded; at least one appeared in structured Worker logs, establishing local CLI/Identity/Worker application trace correlation. Health preserved incoming trace ID while generating a service span.
- Windows links followed local updates and were removed correctly without deleting the target; clone snapshots refreshed only on explicit clone; unmanaged files stayed intact.

For this small fixture, accumulated CLI subprocess elapsed time was 1,919 ms; slowest command was a cloud pull at 95 ms. These are acceptance diagnostics, not a representative performance benchmark or production latency claim. Durable pass summary is preserved here; `.temp/e2e/results.json` and sanitized `worker.log` hold the run-specific details.

At the time of this local Windows pass, real staging account/OS credential vault acceptance was still pending; it is completed in the subsequent actual-staging sections below. Linux CI execution, deployed Cloudflare dashboard ingestion, and forced CAS-conflict interleaving remain separate acceptance.


## Actual staging cloud CRUD: PASS

On 2026-10-05, the real signed-in staging account completed 20 real CLI commands using `.temp/staging-cli/mskill.exe`, the production OS credential vault session at `.temp/staging-home`, issuer `https://identity-staging.moesegfault.dev`, registered client `mskill-cli-staging`, and registry `https://skills-staging.moesegfault.dev`. No mock issuer, fixture bearer token, test credential adapter, SQL, or token inspection was used.

Public publisher ID: `u_069702e621ba31f684a5e97e2e085b59`. Disposable skill: `e2e-20261005-c0834f7e`.

- Actual `whoami` mapped the vault-backed account to the published namespace.
- Directory add, cloud publish, anonymous cloud list/pull, independent SHA verification, clone/link, hash no-op update, source replacement, republish, latest reader update, and snapshot/link semantics passed.
- Initial archive SHA: `df684ee9211f519ddfbf38dd0764b664f9cec06f8fa269c3c46a039f8d756294`.
- Latest archive SHA: `0980e1eb8ce7c77f5218a79b0e7380add1cc6ef4fe08ee2cfa7827af2648b5cb`.
- The reader used a separate anonymous home; `whoami` correctly failed while public operations succeeded.
- Cloud removal succeeded; a fresh anonymous pull failed and the deleted skill disappeared from listing. Temporary project and local installs were removed.

Sanitized results and reproducible CLI-only harness are in `.temp/staging-journey`. The original staging vault session remains intact pending natural refresh and coordinated final logout.

### Actual staging natural refresh: PASS

At 15:20:00 Singapore time, after the real session naturally crossed its refresh threshold, the same copied CLI executed `--json --verbose whoami` without modifying expiry or inspecting credentials. It succeeded with the same owner ID. Token-free diagnostics recorded Identity discovery HTTP 200, token HTTP 200 (1,609 ms), two JWKS HTTP 200 checks, and registry HTTP 200. All shared trace ID `805616ff131d400c8f2ae830d6d711f9`; the registry request ID was `5a29765d0b5349719a72c779adad7090`. This validates actual provider refresh, rotated-session persistence through the Windows vault, and Identity/registry trace correlation on the live staging path. Logs contain endpoint phases/status/timing/correlation only, never token bodies.

### Maintained useful skill publication and final logout: PASS

With explicit authorization, the same real staging account published the maintained repository skills, then a separate anonymous CLI home downloaded both `.skill` archives. Every distributed file (including `SKILL.md` and progressively disclosed reference files) matched repository sources byte for byte; archive SHA-256 independently matched remote metadata.

| Retained public staging package | Archive SHA-256 |
|---|---|
| `u_069702e621ba31f684a5e97e2e085b59/mskill-use` | `0db34852e1daae6a751899033efeca8dac3350d4fd5c070469081c832696c3de` |
| `u_069702e621ba31f684a5e97e2e085b59/mskill-publish` | `c1e527ceaf52692c671eade6463756434efc5bf0c39ef4814a3996ad71a57911` |

These useful packages remain in the staging catalog; only the disposable acceptance package was deleted. Publication included only the maintained Markdown skill sources, not private fixtures or account material.

After publication and natural-refresh acceptance were complete, coordinated final `--verbose logout` returned provider revocation HTTP 200. Subsequent actual CLI `whoami` and `publish local/mskill-use` both failed as expected, proving the OS-vault session was removed. Anonymous cloud listing still returned both published packages. This final phase passed 12 actual CLI commands. Sanitized evidence is `.temp/staging-journey/maintained-results.json`, `logout.log`, and `signed-out-denial.log`.

The real staging CRUD, natural provider refresh, useful-package publication and provider-backed logout loop is complete. The staging OS-vault session is intentionally signed out now. Remaining external acceptance is Linux CI execution, deployed Cloudflare dashboard ingestion, and a forced concurrent CAS-conflict interleaving; the local Windows run alone does not establish those.


## Served staging catalog checks

Supported Browser runtime setup completed, but browser selection returned `No browser is available`. The required troubleshooting documentation was consulted; one discovery call returned `[]`. No session resets, Computer Use fallback, browser credential inspection, or unsupported automation was attempted. Consequently rendered search/filter, clipboard interaction, and narrow/mobile screenshots are **unverified**, not reported as passing.

The bounded fallback fetched the actual served staging HTML, `/assets/app.js`, `/assets/app.css`, `/v1/skills`, and both maintained archive download routes. Results:

- Website, JavaScript, CSS and public catalog returned HTTP 200 with expected content types.
- Actual served JavaScript passed `new vm.Script(...)` syntax parsing.
- Served HTML includes the declared catalog/filter/status/count/load-more DOM hooks, a viewport meta tag and polite live status; stylesheet includes responsive media queries. These are source/DOM contracts, not a visual-layout pass.
- Public catalog contains both maintained skills. Each download returned HTTP 200 and exact byte length/SHA-256 matching its current metadata and quoted ETag.
- Current `mskill-use`: 3,012 bytes, SHA `0db34852e1daae6a751899033efeca8dac3350d4fd5c070469081c832696c3de`.
- Current `mskill-publish`: 2,199 bytes, SHA `d94c83e1c6af2300b26365e59567394af5d92b6f6acf9118fca251872dc016f7` (a subsequent maintained publication superseded the earlier hash recorded above).

Actual fetched resources, package bytes, and the reproducible public-check harness/results are stored only under `.temp/ui-acceptance`. No screenshot is supplied because no rendered browser surface was available.

## Final canonical consumer-directory contract

The final standard-consumer contract requires each newly installed project directory leaf to equal the skill's declared manifest name. Noncanonical `--alias` installs are rejected without project mutation; removal of previously managed legacy aliases remains compatible.

The real-process harness has been adapted accordingly: clone and link comparisons use separate projects with canonical `hello` directories, distinct publishers with the same name install into separate projects, and an attempted second-publisher collision in the first project must preserve the existing installation. Every successful manifest-content assertion independently parses `SKILL.md` and checks the directory leaf equals the declared name. A rejected noncanonical alias must leave the target project without an `.agents` directory. A seeded legacy managed-alias manifest exercises actual CLI removal to preserve the earlier external contract.

Historical 23-command native and 51-command full passes above apply to the prior alias-capable contract; they are retained as historical results, not claimed as final canonical-fixture acceptance. The updated fixture passes syntax checking and awaits the final integration owner's one-batch build/run. No validator Cargo build was launched.


### Final canonical fixture acceptance: PASS

The final integration owner rebuilt the updated CLI/Worker and completed the canonical fixture on Windows: **54 actual CLI subprocess commands passed**, plus three service observations (concurrent latest integrity, actual scheduled GC, and shared Identity/Worker trace correlation). Run-specific evidence is `.temp/e2e/results.json` and sanitized `worker.log`.

An independent post-run check then read the actual installed files in the remote-copy, remote-link, and second-publisher projects: each directory leaf was `hello` and each parsed `SKILL.md` declared `name: hello`. Both noncanonical clone and link attempts exited nonzero and left the rejected project's `.agents` path absent. The seeded previously managed `old-alias` directory was removed by the actual CLI, preserving removal compatibility. Evidence: `.temp/e2e/canonical-independent.json`.

The final run also observed one real mock-provider refresh, three revocations, nine trace IDs with Worker-log correlation, scheduled cleanup preserving the live package and keeping the deleted pointer absent, and concurrent writes `[200, 200]` resolving to intact latest archive bytes. No `publish_conflict` 409 was forced by the scheduler; the digest-bound stale-download 409 path is covered. This supersedes the earlier alias-capable fixture counts without rewriting their historical outcomes.


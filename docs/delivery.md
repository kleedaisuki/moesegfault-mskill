# Build, delivery, and deployment

## Toolchain and local resource budget

Rust stable and the `wasm32-unknown-unknown` target are required. Node 22 or later
is used only for deployment tooling and the end-to-end harness. Application code
is Rust; workers-rs generates the runtime JavaScript binding shim.

The repository sets Cargo to two parallel jobs and `.cache/target`. Experiments,
fixtures, logs, release packaging, and local Wrangler state belong in `.temp` or
`.cache`; never put project test files in an external temporary directory.

```sh
rustup target add wasm32-unknown-unknown
cargo install worker-build --locked --version 0.8.7
npm ci --ignore-scripts --no-audit --no-fund
cargo build --locked -p mskill-cli
npm run build:worker
npm run e2e -- --skip-build
```

`worker-build` is installed separately rather than in the Wrangler build hook.
This avoids repeated network/index checks every local rebuild. The hook delegates
to `scripts/build-worker.mjs`, which selects the crate directory and absolute
workspace target directory consistently on Windows and Unix.

The Worker hook overrides release stripping to `debuginfo`. The first real build
on Rust 1.99 / worker-build 0.8.7 / wasm-bindgen 0.2.129 compiled Rust successfully
but failed binding generation with `externref table required for catch wrappers`.
`strip=true` removes required Wasm feature sections, matching upstream
[workers-rs issue 1014](https://github.com/cloudflare/workers-rs/issues/1014).
Native release binaries keep the workspace's normal symbol stripping.

## CI strategy

`ci.yml` runs only when implementation, skill packages, build configuration, or
workflow files change. A Linux/Windows native matrix exercises native contracts
and real local CLI journeys. One Linux job builds Wasm and runs the full CLI /
OIDC / Workers / D1 / R2 journey. Rust dependencies, installed Worker build tools,
and npm downloads are cached. Pull-request jobs read caches but do not write them.
All jobs have bounded timeouts. A newer branch/check-only run cancels its obsolete
predecessor; runs eligible to deploy are not canceled automatically.

Deployment consumes the exact Worker artifact produced by the tested integration
job and sets `MSKILL_SKIP_BUILD=1`; the build hook verifies the prebuilt shim and
Wasm are present without compiling a second untested artifact. Wrangler 4.130
does not expose a `--no-build` option.
Workflow concurrency separates checks, staging deployments and production
deployments. Both the workflow run and deployment job keep an active deployment
noncancellable; a job-level lock alone would not protect against workflow-level
cancellation. GitHub may still coalesce older pending runs in the same group.
Deployments are serialized per environment without interrupting an in-progress
migration. Pushes to `main` always deploy staging. A manual dispatch selects
`staging` (the default) or `production`; deployment still requires `deploy=true`
and the existing native/Worker acceptance jobs. The selected GitHub environment
and deployment concurrency key match the target. Staging uses `--env staging`,
while production uses the root Wrangler configuration with isolated production
bindings and production Identity registration. The public smoke targets the
matching domain; the workflow never makes production trust the staging client.
Manual production deployment is allowed only from `refs/heads/main`; a feature
branch or tag can run checks but cannot replace the production configuration.

```sh
# After production registration/configuration is reviewed and merged:
gh workflow run ci.yml --ref main -f deploy=true -f environment=production
```

Selecting production does not register an Identity client, change token
audiences, bypass authentication, or rebuild the tested Worker artifact.

`release.yml` creates portable Linux x86_64, Windows x86_64, and macOS ARM64 CLI
archives with LICENSE and SHA-256 files. Release tags are distributable client
builds, not registry package versions. The registry always stores latest state.

## Cloudflare resources

Resources were provisioned on 2026-10-05 in account
`07109e406d4e1ab7a0997dd399db6fd5`:

| Environment | D1 database | Database ID | R2 bucket | Domain |
| --- | --- | --- | --- | --- |
| Production | mskill-registry-production | 7e1d9474-5330-44ae-80e3-4d5258a50df0 | mskill-packages-production | skills.moesegfault.dev |
| Staging | mskill-registry-staging | 2dcc3b29-3d18-47a1-b1fc-b52b16df4cf9 | mskill-packages-staging | skills-staging.moesegfault.dev |

Migrations `0001_registry.sql` and `0002_upload_leases.sql` were applied remotely
to both environments. The second migration preserves upgrade compatibility for
already-provisioned databases when the upload-lease tables were introduced. Database
and bucket identifiers are public configuration, not credentials. The two
environments never share data or token audiences.

The repository already has GitHub Actions secrets `CLOUDFLARE_API_TOKEN` and
`CLOUDFLARE_ACCOUNT_ID`. Workflows do not use the unrelated S3 access secrets.
Keep API tokens scoped to the deployment account, Worker scripts/routes, D1,
and R2 access needed for these bindings; never put developer OAuth credentials
in Actions caches or artifacts.

```sh
npx wrangler d1 migrations apply DB --env staging --remote
npx wrangler deploy --env staging
node scripts/smoke.mjs https://skills-staging.moesegfault.dev
```

The production native client `mskill-cli` was registered through the reviewed
Identity deployment configuration after explicit launch approval and read back
enabled. The root Worker audience is now `mskill-cli` on the exact production
issuer. Staging retains `mskill-cli-staging` and its separate issuer/storage.
Deployment does not perform registration or bypass account mapping; production
account tests remain excluded, while actual user-journey QA uses staging only.

## Observability acceptance

Both environments enable Workers Logs, invocation logs, and native traces with
sampling 1 during initial delivery. The CLI sends W3C `traceparent`; the Worker
returns correlation and emits structured request records. Native traces and
application trace IDs must not be assumed identical unless the platform exposes
an explicit relationship. Logs must not contain bearer tokens, authorization
codes, refresh tokens, archive contents, or account contact details.

The compatibility date is `2026-09-15`, matching the newest runtime shipped in
pinned Wrangler 4.130. The initial local Worker startup exposed that a deployment
date of `2026-10-05` runs in Cloudflare but is rejected by that local workerd.
Both local acceptance and deployed config use the same stable compatibility date.

After deployment `scripts/smoke.mjs` checks health, public catalog, rejected
anonymous management, and incoming trace correlation. Full account mutations
are tested with disposable local accounts and the separately authorized staging
Identity journey, not by injecting a development token into production.

### Initial deployed acceptance (2026-10-05)

Staging Worker version `ae1a2580-bcfb-489f-94fe-051de761c35e` deployed successfully
to both `https://skills-staging.moesegfault.dev` and
`https://mskill-registry-staging.moesegfault.workers.dev`. Wrangler reported a
3 ms startup, 1115.26 KiB upload, and 410.93 KiB gzip size. These are deployment
measurements, not a client startup or tail-latency benchmark.

Public smoke passed health/catalog 200, anonymous management 401, and response
trace correlation. A real Wrangler tail captured a structured `/health` request
record with `request_id=delivery-native-trace`,
`trace_id=12345678901234567890123456789012`, `span_id=210b8f567d8b5c01`, status 200,
and duration 941 ms. The response preserved the trace ID but generated its own
span. This verifies live structured-log delivery, not full native-trace dashboard
ingestion or successful staging account writes.

Wrangler 4.130 supports `observability.redact_query_string`, enabled here for both
environments, but its trace schema does not expose `propagation_policy`. Native
trace joining therefore remains distinct from the verified application trace
correlation. Do not add unknown fields or claim propagation-policy acceptance.

Native acceptance subsequently passed 23 separate real Windows CLI commands:
portable pack/export/import, hostile ZIP rejection, local listing, clone/link,
source replacement, explicit copy refresh, snapshot persistence, unmanaged
collision refusal, scoped removals, and automatic no-color piped output.
The journey caught and drove fixes for duplicate ZIP central entries hidden by
the ZIP library and Windows directory-symlink removal. Unit acceptance passed
14 tests and one doc test before the added symlink regression.

The subsequent full local Workers / D1 / R2 / mock OIDC journey passed 51 real CLI
commands plus three observations, including signed-token WebCrypto validation,
two users publishing the same skill name, wrong-owner deletion refusal,
hash-matched no-op updates, latest replacements, actual refresh and revocation,
malicious ZIP rejection by both CLI and Worker, and garbage collection preserving
live packages. Nine Identity trace IDs were recorded and at least one matched
structured Worker records, connecting the CLI/OIDC/Worker request flow.
Concurrent publication serialized as 200/200 with intact latest bytes;
the scheduler did not produce a publish CAS 409, but stale digest download 409 was
exercised. See [e2e.md](e2e.md) for the bounded workflow and results.

An independent forward test used the actual `skills/mskill-use` package rather
than a synthetic skill: add into an isolated home, clone as `helper`, link as
`helper-link`, edit source/update, verify copy stays a snapshot while link updates,
and remove `helper` without deleting the local package. All five mutation commands,
subsequent local listing, and independent filesystem assertions passed; artifacts are in
`.temp/skill-forward/result.json` for the current development session.

### Actual staging Identity and OS-vault closure

The separately authorized staging journey then used the real Identity provider,
genuine authorization-code grant, and Windows native credential vault. Twenty
CLI commands passed account mapping, publication/replacement, public listing,
anonymous pull, no-op updates, copy/link installation and cloud deletion. An
unmodified real access-token lifetime naturally reached the refresh threshold:
at 15:20:00 Singapore time, provider token/JWKS/discovery calls and registry
`whoami` succeeded with trace `805616ff131d400c8f2ae830d6d711f9`.

A further twelve real CLI commands published the maintained `mskill-use` and
`mskill-publish` packages, verified anonymous pulls against every source file,
and left both useful packages public in the staging catalog under publisher
`u_069702e621ba31f684a5e97e2e085b59`. Actual logout revocation returned HTTP 200;
subsequent account/write commands rejected access while public listing remained
available. No mock token, plaintext credential file, expiry alteration, or SQL
account bypass was used. Run-specific sanitized details are under
`.temp/staging-journey`; [e2e.md](e2e.md) retains the acceptance summary.

After the measured archive-proof reuse and bounded GC batching changes, the
integrated 51-command local journey including scheduled GC passed again.
The latest native suite passed sixteen tests and one doc test, with formatting
and workflow YAML checks clean. Cross-platform Actions execution is tracked
separately from these Windows results.

### Initial cross-platform Actions acceptance

Commit `e2e8d11b2f12bf985c0e35b512092e402867acd9` passed the initial branch
[Test and deploy run 37277652866](https://github.com/kleedaisuki/moesegfault-mskill/actions/runs/37277652866)
on 2026-10-05 without a CI repair or rerun:

| Job | Result | Duration | Actual workflow coverage |
| --- | --- | --- | --- |
| CLI (ubuntu-latest) | Passed | 1m 30s | Native contracts, format, binary build, 23-command local CLI journey |
| CLI (windows-latest) | Passed | 3m 07s | Native contracts, binary build, 23-command local CLI journey including links |
| Rust Worker and end-to-end account journeys | Passed | 5m 21s | Pinned build-tool installation, native CLI, Wasm, real 51-command local D1/R2/OIDC journey, tested artifact upload |
| Deploy tested Rust Worker | Intentionally skipped | 0s | Branch pushes test only; main/manual dispatch owns staging promotion |

The cold Worker job completed well inside its 25-minute timeout. Its tested
artifact is available for the subsequent staging promotion. GitHub emitted
nonblocking maintenance notices: checkout/setup-node v4 target Node 20 but run
under forced Node 24, and `ubuntu-latest` is scheduled to migrate to Ubuntu 26
on 2026-10-19. These did not change the acceptance result; update action/image
pins deliberately at a separate maintenance checkpoint rather than churn a
passing initial delivery. Run metadata/logs are retained locally in
`.temp/actions`; credentials and fixture token files were not exported.

### Main Actions artifact promotion and deployed acceptance

Commit `978c5bdcf30343a5e46607242eabcac1f33af4f1` passed the normal main pipeline
[run 37278545298](https://github.com/kleedaisuki/moesegfault-mskill/actions/runs/37278545298)
on 2026-10-05, including actual staging deployment with the repository's existing
scoped Cloudflare Actions credentials:

| Job | Result | Duration |
| --- | --- | --- |
| CLI (ubuntu-latest) | Passed | 1m 29s |
| CLI (windows-latest) | Passed | 3m 08s |
| Rust Worker and end-to-end account journeys | Passed | 5m 44s |
| Deploy tested Rust Worker | Passed | 20s |

The deploy job downloaded the tested artifact, confirmed no pending D1
migrations, and used the verified prebuilt-artifact hook without recompiling.
Cloudflare deployed version `19293715-71f0-444c-8b92-e94ecfbbf634`, reporting
startup 3 ms and upload 1122.10 KiB / gzip 413.10 KiB. Workflow public smoke passed
health/catalog 200, incoming application trace correlation and anonymous
management denial. A separate developer-machine smoke after the Actions run
also passed. An anonymous real CLI pulled and cloned both maintained public
packages after promotion; installed manifests still match repository sources.
This closes actual Actions artifact promotion, not merely a local deployment.

No production Worker or production Identity client was changed by this pipeline.
The bounded watch encountered one transient GitHub API EOF; a fresh status read
succeeded and the run itself required no retry, repair, or source change.

## Reference decisions

- [Cloudflare Rust Workers](https://developers.cloudflare.com/workers/languages/rust/):
  workers-rs and worker-build are the production-supported Rust/Wasm path.
- [Cloudflare Workers Logs](https://developers.cloudflare.com/workers/observability/logs/workers-logs/)
  and [OpenTelemetry export](https://developers.cloudflare.com/workers/observability/opentelemetry-export/):
  native ingestion first; no separate telemetry collector required for delivery.
- [Swatinem rust-cache](https://github.com/Swatinem/rust-cache): cache Rust
  dependency artifacts keyed by toolchain and dependency manifest/lockfile.
- [More Haste, Less Speed: Cache Related Security Threats in CI/CD](https://par.nsf.gov/servlets/purl/10522475):
  cache contents and trust boundaries matter. Cache build dependencies only,
  never authenticated sessions or fixture token files.

## Final canonical-name and official-login acceptance

The final native source passed 17 unit tests and one doctest, then the actual
CLI completed 54 full Worker/OIDC/D1/R2 commands and 26 local-only commands.
Project directory leaf names were independently checked against SKILL.md names.
Noncanonical alias requests were rejected before project mutation; old managed
aliases remained removable. Package manifests and archive bytes were not rewritten
to manufacture aliases. Official login defaults only to the registered production
issuer/registry pair; custom destinations require an explicit client ID.

Eight event/concurrency cases verified that checks remain cancellable while
staging/production deployments cannot be interrupted by unrelated checks.
Production deployment is restricted to main and still consumes the tested artifact.
Release packages include upstream dependency license notices; maintained .skill
packages declare the repository license and bundle its full text.


## Production launch acceptance (2026-10-05)

The approved production native client `mskill-cli` is enabled on the production
Identity issuer. Its configuration source is retained in adjacent Identity
commit `7f1d4e1`; no Identity code deployment or production test account was
needed. The normal mskill command now selects this registered client only for
the exact official production issuer and registry pair.

[Production Actions run 37284946136](https://github.com/kleedaisuki/moesegfault-mskill/actions/runs/37284946136)
promoted the tested Worker from source `20e0bea` after both native gates and the
54-command Worker/OIDC/D1/R2 journey passed. No Worker rebuild ran in deployment.
Cloudflare registered `skills.moesegfault.dev`, deployed production version
`3f3b768e-01f8-44f3-a4a9-8b03cf544d1e`, and reported 4 ms startup.

The first public smoke failed with DNS ENOTFOUND immediately after the custom
domain was created. Independent public smoke passed after propagation; rerunning
only the failed deployment job then passed. The native and Worker jobs were not
repeated. Commit `5ec2b61` adds bounded retries for transient reachability and
502/503/504 responses, without retrying contract assertions. Four injected
scenarios cover DNS recovery, gateway recovery, retry exhaustion, and immediate
contract failure. Stable deployments incur no extra wait.

Independent production checks passed health/catalog HTTP 200 with incoming
trace correlation, anonymous management HTTP 401, and actual CLI anonymous cloud
listing. Website, same-origin JS/CSS, privacy, and terms returned HTTP 200 with
security headers; served JavaScript parsed successfully. These are served-resource
checks, not rendered clipboard/search/mobile interaction acceptance. Production
contains no staging QA publication or account data.

The `v0.1.0` tag selects the tested client source plus the smoke-only fix.
Its release workflow was explicitly dispatched against the tag: `[skip ci]` in
the preceding smoke commit intentionally suppressed redundant push checks and
also suppressed the automatic tag-triggered workflow. Manual tag dispatch still
runs all three optimized-binary journeys and the tag-only release publisher.

## Published native release acceptance

[Release v0.1.0](https://github.com/kleedaisuki/moesegfault-mskill/releases/tag/v0.1.0)
was published by [Actions run 37285564929](https://github.com/kleedaisuki/moesegfault-mskill/actions/runs/37285564929)
from tag `v0.1.0` (`5ec2b61`) on 2026-10-05. All optimized native binaries passed
the real 26-command local journey before packaging; no release build ran on the
developer machine.

| Release job | Result | Duration | Download archive bytes |
| --- | --- | --- | --- |
| Linux x86_64 | Passed | 2m 10s | 3,343,611 |
| Windows x86_64 | Passed | 2m 49s | 2,946,871 |
| macOS ARM64 | Passed | 2m 29s | 2,772,509 |
| Publish | Passed | 12s | Three archives and three external checksum files |

Independent post-publication downloads of all three archives matched their
external SHA-256 files. Each archive contained exactly its native binary,
application LICENSE, and upstream THIRD-PARTY-NOTICES.txt. License text matched
the release tag after line-ending normalization; notices retained dependency
license/copyright texts. Matching source archives are available on the release.

The actual downloaded Windows binary reported `mskill 0.1.0`, accessed production
anonymous cloud listing with default configuration, and passed the 26-command
local journey again. All download checks and their reproducible verifier are in
`.temp/release-acceptance`; no binaries or generated notices were committed.
Public release notes provide installation, local and cloud examples, and the
latest-only/snapshot/link behavior without internal validation chatter.

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
All jobs have bounded timeouts. A newer push cancels an obsolete test run.

Deployment consumes the exact Worker artifact produced by the tested integration
job and sets `MSKILL_SKIP_BUILD=1`; the build hook verifies the prebuilt shim and
Wasm are present without compiling a second untested artifact. Wrangler 4.130
does not expose a `--no-build` option.
Staging deployments are serialized without interrupting an in-progress migration.
Pushes to `main` deploy staging; a manual dispatch can deploy a tested branch to
staging. The workflow deliberately does not make production trust the staging
Identity client.

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

Production public reads can be deployed independently from client registration.
Authenticated production writes remain blocked while `IDENTITY_CLIENT_ID` is
`UNPROVISIONED`. Registering a client in the existing Identity service is a
separate reviewed deployment action. Tests use staging Identity only.

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

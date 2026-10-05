# CLI workflow and transport decisions

## Dominant workflows

The executable is `mskill`, built from package `mskill-cli`. Local commands use
`mskill-core` without constructing an HTTP client or requesting credentials.
Help/version parse before the async runtime or library initializes. The runtime
uses one thread: filesystem work is bounded and a CLI command does not need a
background executor pool.

| Operation | Input | Mutated storage |
| --- | --- | --- |
| `add PATH` | Standard skill directory or `.skill` | Local `local/name` |
| `pack PATH -o FILE.skill` | Standard directory or valid archive | Explicit new output file only |
| `export SKILL -o FILE.skill` | Installed local identity | Explicit new output file only |
| `pull owner/name` | Public publisher identity | Local namespaced package |
| `update NAME --from PATH` | Local identity and updated source | Existing local package |
| `update [owner/name]` | One or every cloud-installed identity | Changed local archives/trees |
| `clone SKILL` | Installed identity | Managed project snapshot |
| `link SKILL` | Installed identity | Managed project live link |
| `publish SKILL` | Installed archive and signed-in account | Mapped account's cloud identity |
| `whoami` | Signed-in account | None; displays the mapped publisher owner |
| `remove ALIAS` | Managed project alias | Project install only |
| `remove SKILL --scope local` | Installed identity | Local archive/tree/metadata only |
| `remove owner/name --scope cloud` | Public identity and account | Latest cloud state only |

An abbreviated name means exactly `local/name`, never a guessed publisher or the
first search result. Public operations require `owner/name`. Publishing obtains
`/v1/me` and uses that account's immutable owner with the archive manifest's name.
Different accounts may publish the same manifest name. The project default is
that manifest name; a conflicting publisher requires an unused `--alias`.

Updating a local source is separate from downloading published state: `--from`
requires an installed local identity. The archive's manifest name must match
before any library state changes. Links follow the stable tree after updates;
copies stay snapshots until the same clone command refreshes them. Explicit local
removal warns about dangling project links; cloud removal does not mutate local
copies or archives.

## Output and scripts

`--json` emits one complete JSON value on stdout and diagnostics on stderr. Human
tables contain identity, abbreviated SHA-256 and manifest description, without
additional decorative claims. Untrusted descriptions and HTTP error bodies cannot
insert terminal control characters. Broken stdout pipes exit successfully rather
than dumping a panic into a pipeline.

Color requires terminal stdout, no `NO_COLOR`, and no `TERM=dumb`; `anstream`
handles supported console output. Clap uses its terminal-aware automatic color.
The policy follows [NO_COLOR](https://no-color.org/) and
[Clap ColorChoice](https://docs.rs/clap/latest/clap/enum.ColorChoice.html).

## Transport and latest-state consistency

Registry URLs require HTTPS. Explicit loopback HTTP is accepted for real local
simulation, not arbitrary HTTP hosts. Credentials, query strings and fragments
are rejected in base URLs. Redirects are disabled so authorization cannot escape
the selected origin. Connect timeout is 10 seconds; each complete request has a
60-second timeout. JSON and error response bodies have separate size bounds;
archive downloads stream into a buffer bounded by the protocol's 16 MiB limit,
including responses without Content-Length.

An update first compares validated metadata hashes. A changed download is pinned
to that metadata digest through `?sha256=...`, then its exact bytes are hashed
again before installing. A concurrent publish producing HTTP 409 triggers one
metadata refetch. A mismatched downloaded digest is never installed. The two
attempt bound avoids indefinite waiting under continuously changing publishers.
Public listing follows opaque pagination cursors, rejecting repeats and excessive
page sequences rather than silently presenting only the first page.

One command owns one random W3C trace ID shared by Identity and registry calls.
Each HTTP request has a new span and
`x-mskill-request-id`; `--verbose` shows status and correlation identifiers, never
authorization headers or tokens. Registry structured error details include the
server request ID. Auth verbosity includes discovery/JWKS/token/revocation phase
latency and safe correlation identifiers, without authorization URLs or token
response bodies. Trace context follows
[W3C Trace Context](https://www.w3.org/TR/trace-context/).

## Authentication boundary

`mskill-auth` owns browser callback, PKCE, verified token refresh and platform
credential storage. Registry client code receives a token only for profile,
publish and delete operations. The issuer/client registration is explicit until a
production native client is provisioned; development client IDs are not silently
substituted for production. `login --no-browser` retains the browser authorization
workflow but prints its URL for a user or controlled local simulation.

The `e2e-test-store` feature is not a release feature. Its credential file is
available only with an explicit loopback HTTP issuer and remains inside the chosen
library home. Production builds use the OS credential vault.

## Acceptance evidence

The integration owner runs one shared, low-concurrency Cargo build and actual
CLI journeys against an isolated local Worker and OIDC service, under `.temp`.
Focused parser tests support those journeys; they do not replace them. See
`scripts/e2e.mjs` and `docs/local-store.md` for the executable workflow and local
recovery contracts. Resource-heavy performance investigations follow the first
working journey rather than preceding it.

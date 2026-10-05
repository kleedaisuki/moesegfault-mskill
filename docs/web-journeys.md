# Dual-site rendered user journeys

## Acceptance basis and environment

The requested product consists of two distinct real hosts: `mskill.moesegfault.dev`
for the product/download entry and `skills.moesegfault.dev` for the human workspace
and community. Guests must browse, preview and obtain actual packages; Identity
sign-in must enable publication management and comments. All surfaces require
Chinese, Japanese and English, light/dark modes, responsive mobile interaction,
and the actual moeSegFault platform style. `web-plan.md` preserves the full scope;
`web-backend.md` specifies mutation/session boundaries.

Use deployed staging and dedicated synthetic QA identities for all mutable
journeys. Never create test accounts/publications on production. Screenshots,
fixtures and sanitized results belong to `.temp/web-journeys`. Browser credentials
and cookies are not inspected, logged or exported. Root owns the sole local
integration build/server slot; this validator does not start Rust builds.

## Current browser probe (2026-10-05)

The supported Browser runtime selected Codex In-app Browser, ID `2`, for
`https://skills-staging.moesegfault.dev/`. Its documentation and troubleshooting
were read before interaction. Selection and `tabs.list()` succeeded. The initial
navigation/AX batch timed out at 30 seconds and the runtime reported kernel reset.
The resulting tab remained live (`1`, title `mskill · Skill 分发与复用`). A separate
`tab.playwright.domSnapshot()` eventually returned the actually served old UI:
two maintained skills (`mskill-use`, `mskill-publish`), guest search, copy-reference
buttons and real archive links. This is a baseline, not acceptance of the new UI.

`tab.ax.write('both')` and `tab.screenshot({fullPage:false})` both explicitly failed
with `Unable to capture screenshot`. A semantic search `fill()` timed out after
30 seconds and reported another kernel reset. No input success or visual capture
is claimed. Recovery follows only documented runtime APIs; no alternate control
mechanism or source inspection bypass is used. Browser availability is therefore
partial: discovery and DOM read work, capture/input do not yet establish rendered
interaction acceptance. Recheck the supported connection against integrated UI
when the root reports its server/deployment ready.

The documented visibility capability additionally returned `IAB visibility is
not supported in a subagent thread`. Root therefore owns subsequent rendered
Browser control; the validator prepares fixtures and independently checks
downloaded results, without competing for root's browser or resetting it.

### Independent rendered QA environment

Root's own distinct supported probes confirmed that visibility became true but
AX/screenshot remained unavailable and native semantic fill still timed out with
kernel reset. After the Browser skill and documented recovery were exhausted,
root authorized a separate self-controlled browser environment, not a bypass or
control connection to the selected IAB. Official Microsoft Edge was already
installed at `C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe`.
`playwright-core@1.63.0` is pinned as a development dependency; no browser binary
was downloaded. The dedicated profile is `.temp/web-journeys/edge-profile`; no
user profile, password/session store or existing personal browser is inspected.

`node .temp/web-journeys/browser-probe.mjs` succeeded in actual guest search and
rendered desktop/mobile screenshots on the old deployed staging baseline. The
first isolated harness run checked results before asynchronous catalog load and
failed; adding an explicit wait for the actual source row repaired the harness
race without weakening the expected search behavior. Successful artifacts:
`browser-probe.json`, `baseline-desktop.png`, `baseline-mobile.png`. These prove
the independent QA surface works, not that the still-evolving new UI is complete.

The public Login UI was separately inspected in this isolated browser: genuine
first-party form controls are `input[name=login]`, `input[name=password]` and
the password-login submit button. No credentials were entered for that probe;
`identity-empty.png` contains only the empty public form. Actual staged web login
uses these visible controls, existing dedicated QA credentials and the product's
original browser authorization redirect, never injected provider/app cookies.

## Confidential local Identity fixture

`scripts/mock-oidc.mjs` now supports a distinct configurable confidential client
alongside the existing native client. It generates a private/public RSA fixture
pair when one is not supplied and exposes `webClientId`, `webPrivateJwk` and
`setWebRedirectUri()` for the isolated Worker harness. Assertions require RS256,
the registered key ID/signature, exact issuer/subject/client, exact token endpoint
audience (also on revocation, matching the real provider contract), bounded
timestamps and a one-use assertion ID. Authorization codes and rotating refresh
tokens retain immutable client binding. Both clients retain S256 PKCE. Provider
logout has an exact callback and state propagation; it does not invent sessions.

`node .temp/web-journeys/probe-mock.mjs` passed native grant/rotation/revocation,
confidential grant and audience, missing/wrong-audience assertion rejection,
assertion replay rejection, wrong-client refresh rejection, confidential refresh
and provider logout callback/state. Sanitized counters are in `mock-results.json`.
These validate the fixture's contract, not deployed Identity or rendered auth.

## Disposable package fixtures

`node .temp/web-journeys/prepare-fixtures.mjs` uses the actual published Windows
`v0.1.0` binary at `.temp/release-acceptance/windows/mskill.exe` to pack two
valid revisions of `web-journey`. It does not build, log in or publish. The source
contains multilingual text, a Markdown table/code block, a relative resource
link, script markup, a `javascript:` link, an external image, raw HTML resource
and long text. Preview must not execute script, follow unsafe URLs, or render
HTML resources as active documents. Mobile layout must contain long text without
making navigation unreachable.

| Revision | Bytes | Independently computed archive SHA-256 |
| --- | --- | --- |
| 1 | 1392 | `e277d285ff1241930e9e30f49f9c7419bc486da75b610ceadaf32e95e69fb085` |
| 2 | 1393 | `7c945b514508f43a4aa5154dd546dc92ada7f8261be7741d9b76fb6e9dc3a37e` |

Archives are `.temp/web-journeys/web-journey-revision-{1,2}.skill`; per-resource
hashes are in `.temp/web-journeys/fixtures.json`. Browser publication/replacement
and download checks must use these exact bytes, not an expected value copied
from backend metadata. These files are explicitly disposable **staging only**.

## Journey matrix

| Journey | Observable success criterion | Evidence required |
| --- | --- | --- |
| Product entry | Three crawlable language pages, useful install/download CTA and separate workspace destination | Navigation and screenshot for each language/theme; actual CTA click |
| Guest discovery | Catalog search yields matching packages; no results offers recovery | Search input, result click and visible empty state |
| Package detail | Markdown preview, resource tree and selected text display without executing archive HTML | Actual resource selections and screenshot |
| Distribution | Copy installs correct owner/name; archive download is exactly latest package | Click, isolated clipboard read and downloaded SHA comparison |
| Identity sign-in | Real staging provider redirects back to intended path with signed-in controls | Browser login flow; no token capture |
| Publish/update | Owned `.skill` upload produces package; replacing changes rendered preview and download | File picker upload, real public detail and SHA |
| Delete | Explicit disposable staging package disappears; unrelated package remains | UI confirmation, guest detail failure and catalog |
| Community | Author adds/edits/deletes comment, second account cannot modify it; owner moderation works | Two authorized synthetic identities, visible persistence |
| Logout/session errors | Local access closes; guest cannot mutate; expired state supports sign-in | UI logout, blocked mutation and recoverable error |
| Mobile | Navigation, detail/resources, upload dialog and comment actions remain reachable | Actual narrow viewport interactions/screenshots |
| Themes/languages | Shared controls persist preference and all reachable text is translated | Light/dark/system and zh/ja/en checks across both hosts |
| Production promotion | Hosts visibly deliver distinct final experiences and real public downloads | Read-only production browser smoke after staging acceptance |

Do not replace these flows with static DOM hooks, fetched HTML, mock UI, unit-test
counts or green CI. Unexercised paths stay pending.

## Actual staged browser journeys (2026-10-05)

The integrated live deployment `68b3cdab-48a6-4a0f-b61a-8520f61a98f2` served both
staging domains. Actual isolated Edge journeys, not a mocked frontend, passed:

- Both hosts in zh-CN/ja/en, native theme selection light/dark, 1280px desktop
  and 390px mobile capture, with no document-level horizontal overflow. Eighteen
  rendered screenshots are `.temp/web-journeys/screenshots/{workspace,product}-*`.
- Guest server-backed search, selected detail, copy button and real file-tree
  resource interaction. A fresh fully loaded detail had no page errors and
  correctly switched its visible file panel.
- Original primary QA account completed real visible first-party password login
  through the application's authorization redirect and mapped to the existing
  native publisher `u_069702e621ba31f684a5e97e2e085b59`.
- UI upload published revision 1; a real clicked download independently matched
  `e277d285…`. Unsafe archive script/javascript links did not execute; raw HTML
  resource was visibly text. Revision 2 replacement through the UI exposed its
  new heading, and an independent real guest browser click/download matched
  `7c945b51…` exactly.
- Primary author created and edited a comment. A second verified staging account
  completed its own real browser login, had no foreign-comment Edit control,
  and posted its own comment. Authenticated wrong-account PATCH/comment and
  DELETE/package returned 403 using that browser's legitimate session and CSRF.
  The package owner moderated the second participant's comment through the UI.
- At a real narrow viewport, long-text resource selection remained contained.
  The primary user deleted the disposable package through its confirmation UI;
  metadata then returned 404. Local application logout returned a null session.
  The second user chose explicit moeSegFault logout, traversed the real provider
  logout flow and returned to the workspace with a null application session.

Useful artifacts: `staging-results.json`, initial partial history
`staging-first-partial.json`, `download-probe.json`, actual revision downloads and
screenshots `published-revision-1`, `owned-files-mobile`, `identity-logout-return`.
No production account/data was used. Temporary package and its comments were
removed. Staging accounts/owned mail aliases are retained as reusable QA fixtures.

### Failures diagnosed without weakening acceptance

The first interaction batch tapped SSR controls before JavaScript hydration;
its hidden file-resource timeout was reproduced against a fully loaded detail
and disappeared with explicit harness readiness. Root additionally asked the
frontend owner to disable JS-only tabs until handlers exist so real early taps
are honest, and to refresh comment permissions after delayed session hydration.
Those changes require a subsequent narrow deployed-browser regression.

The initial comment-edit harness kept a text-filtered locator after the row was
replaced by its editor. That locator no longer matched; changing to the actual
scoped editor restored the expected action, without changing product code.

Reused persistent Edge QA profiles returned `Target page, context or browser has
been closed` on repeated download completion, even with a dedicated downloads
directory. A fresh self-controlled Edge browser performed the exact same public
UI click, completed the download with `failure=null`, and produced the expected
revision-2 SHA. This separates the profile-dependent browser environment issue
from the package's distribution contract. The failed profile runs are retained;
they are not reported as a single clean pass or removed by rerunning blindly.

A final discriminating probe used a fresh nonpersistent QA Edge context, completed
genuine staging password login, and clicked the maintained `mskill-use` download
twice. Both completed with `failure=null` and independently matched current public
metadata SHA; provider/application logout then completed. Results are
`auth-download-results.json`. Thus signed-in distribution itself also works; the
failure is specific to the reused persistent QA browser environment, not merely
explained away by comparing an authenticated profile with an anonymous one.

### Focused deployed UX regression

After the bounded UX patch was deployed as
`6ba22ebd-f4fa-4be4-b9f6-d254608fc13f`, a fresh Edge browser with its temporary
process/profile directories redirected inside `.temp/web-journeys` completed
the actual focused scenario in one run (`focus-results.json`):

- With the real app module deliberately held in transit, SSR tabs were disabled
  with `aria-disabled=true` and public archive download remained available.
  Releasing the real script enabled tabs and the actual Files interaction worked.
- Both hosts' language selectors really navigated zh-CN/ja/en pages. Light/dark
  preferences survived reload independently on each origin.
- A 390px viewport performed genuine login, opened the contained upload dialog,
  selected and published the archive, and exercised mobile comments.
- The second author created, edited and deleted their own comment. With the
  real `/web/session` delayed by five seconds, public comments appeared first;
  initially absent author controls appeared after the genuine session response.
  No session or response was injected.
- Disposable package cleanup, local logout and explicit provider logout passed.

Screenshots: `early-tabs-disabled-mobile`, `mobile-upload-dialog`,
`mobile-community-comment`, `delayed-session-comment-permissions`.

### Reusable browser entrypoint and remaining scope

`scripts/web-browser-journey.mjs` provides public read-only capture and explicit
staging-only mobile participation regression. Supply private QA fixture paths
through `--actor` and `--second-actor`, never credentials in command arguments.
`--browser` selects an already installed official executable; it does not download
a browser. Artifacts and process temporary directories are constrained to project
`.temp`/`.cache`. Secrets and authorization queries are sanitized on failure.
No large browser matrix was added to CI.

Mutable mode restricts the workspace to the exact first-party staging origin and
checks the actual Login page origin before any QA credentials are entered.
Local mock grant probes do not claim an actual short-TTL BFF refresh/lease run or
live provider session-expiry acceptance; those optional checks do not substitute
for the fully exercised requested frontend journeys.

## Production read-only rendered acceptance

After Actions run `37293207461` completed the full gates and production promotion,
the exact reusable harness was run against the two real production domains:

```powershell
node scripts/web-browser-journey.mjs --mode read-only `
  --origin https://skills.moesegfault.dev `
  --product https://mskill.moesegfault.dev `
  --output .temp/web-journeys/production-readonly
```

Result: exit 0 and `results.json` reports `pass=true` (2026-10-05
10:00:06–10:00:33 UTC recorded events). Eighteen zh-CN/ja/en light/dark desktop/mobile screenshots
show the distinct product and workspace experiences. Mobile document width did
not overflow. On each host, selecting system `auto` and changing actual browser
OS-color-scheme media between light and dark changed computed text color; four
additional screenshots capture both directions. These are rendered behavior
checks, not merely successful selector assignment.

The product's real download CTA scrolled to platforms and its Windows archive
link was clicked. The actual downloaded bytes matched the publicly distributed
SHA-256 file. The real workspace CTA then navigated to the distinct registry
host. Product `robots.txt`, `sitemap.xml`, `llms.txt`, `agent.json` and registry
`robots.txt`, `sitemap.xml`, `llms.txt`, `/v1/community` returned 200. The browser
remained a guest (`/web/session` user null). No production sign-in, test account,
package, comment or mutation was created.

Artifacts are `.temp/web-journeys/production-readonly/results.json`,
`downloaded-client.tar.gz` and `screenshots/`. The product screenshot was visually
inspected: the real platform typography/colors/cards, useful install commands,
platform downloads and workspace destination are visible rather than a skeleton.

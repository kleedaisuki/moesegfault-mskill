# Human Skills workspace

## Product and routing boundary

The registry host is a human workspace, not the product landing page. The product
host is dispatched independently by the outer Rust router. Existing native CLI
`/v1` contracts remain available; UI uses the browser BFF without browser-held
OAuth tokens. Shared decisions live in `web-plan.md` and `web-backend.md`.

Public page routes have independently rendered Chinese, Japanese and English:

| Route family | Purpose |
| --- | --- |
| `/`, `/ja/`, `/en/` | Recent public skills, server-backed registry-wide search |
| `/{locale?}skills/{owner}/{name}` | Current metadata, SKILL.md, preview, download, discussion |
| `/{locale?}people/{owner}` | Public publisher profile and actual counts/publications |
| `/{locale?}mine` | Signed-in publisher's current packages |
| `/{locale?}privacy`, `/{locale?}terms` | Actual data, session, public discussion, moderation notices |

Language prefixes affect documents, not API or callback paths. Canonical and
reciprocal `hreflang` references are server-generated. The page shell contains
actual catalog links and SKILL.md source before JavaScript; it is not an empty
application mount. Page data is serialized as inert JSON with `<`, `>` and `&`
escaped, not executable inline JavaScript. Public author identifiers are opaque
namespaces. Display labels never become authorization decisions.

The staging deployment emits `X-Robots-Tag: noindex, nofollow`, disallows all
crawling in robots.txt and returns an empty sitemap. Production sitemap includes
bounded latest detail URLs in all languages and excludes the private workspace.

## Platform design-system adoption

Both surfaces self-host actual public distribution exports of
`@moesegfault/style` 0.1.2, not an approximated palette. `web/shared/README.md`
records the public exports and upstream license. Host layout composes semantic
`--moe-*` tokens, actual `.moe-button`/accessibility classes, and the platform's
`applyTheme` API. Themes are `auto`, `light` and `dark`; the first-paint bootstrap
runs before CSS and uses the documented `moe-theme` persistence key. No runtime
font request, React runtime, external icon library or third-party analytics is
added. Locale follows the current crawlable document route.

`messages.json` is generated from `i18n.js` for Rust SSR; both contain the same
117 keys in `zh-CN`, `ja` and `en`. Placeholder parity is checked. The Rust locale
lookup uses immutable `OnceLock` storage; it does not parse the dictionary for
every label. JavaScript translation output enters DOM text nodes, not HTML.

## Read, publish and conversation behavior

- Guests browse, search, view publisher context, read previews, copy canonical
  install commands, download the actual `.skill`, and read comments.
- Search calls `/v1/community?q=...` over the whole registry. It is not filtering
  only downloaded pages. Pagination reuses the backend's opaque keyset cursor.
- Sign-in restores public account labels plus publication/comment controls from
  `/web/session`. Cookie writes include the session-specific `X-CSRF-Token`.
- New publication sends the archive once to `POST /web/publish`. Rust derives the
  canonical name and validates the portable archive. The browser does not contain
  a second ZIP implementation or YAML-name parser.
- New publication carries `If-None-Match: *`. `skill_exists` requires an explicit
  replacement confirmation; it is not silently treated as successful creation.
- Detail replacement and deletion send `If-Match` with the displayed SHA. The
  backend enforces the condition in its authoritative storage transaction.
- Comments are public plain text, with actual server-backed create/edit/delete.
  Authors edit/delete their own comments; Skill owners can delete moderation.
  A skill replacement preserves discussion; deleting a skill removes its
  discussion, explicitly stated in the three-language confirmation copy.
- Local app logout and provider SSO logout are distinct controls. Optional SSO
  follows only the opaque same-origin handoff returned by the BFF; no token or
  provider ID-token is returned to frontend JavaScript.
- `session_refresh_busy` retries once after one second, never treating a transient
  refresh lock as sign-out. An actual 401 restores a visible sign-in control and
  hides mutations without throwing away a comment draft.

The frontend never stores provider/session credentials in localStorage, creates
an account password form, performs Identity authorization itself, or executes
archive code. Protected endpoints remain the source of ownership decisions.

## Preview renderer and trust boundary

The mature parser is pinned **markdown-it 15.0.2**, using its published `./browser`
ESM export. The browser asset is lazily imported on content pages only. The exact
npm tarball `markdown-it-15.0.2.tgz` was SHA-512 checked against official registry
integrity:

`q4IGxMv56jCqT4OCRCADBoDP3LO4MhmTXjFbphHPXs4g3j9Xg5RDnxqN8IF/3vIWEU+VCnUq+7JUg/cfy2E6Qw==`

The browser bundle includes mdurl 2.1.0, uc.micro 3.0.0, entities 8.0.0,
linkify-it 6.0.0 and punycode.js 2.3.1. Every bundled dependency source-map entry
was compared with these published package files and matched byte-for-byte.
`MARKDOWN-IT-LICENSE` preserves the parser and all bundled dependency notices.
No npm/package-lock modification is needed to embed these pinned bytes.

The parser uses `html:false`; scripts, raw HTML, iframe and arbitrary author CSS
remain text. Links accept HTTP(S) or archive-relative references, not javascript,
data or protocol-relative schemes. Images render as text rather than initiating
external tracking requests or loading active SVG. Relative document links select
the actual archive file tree. Tables, blockquotes, nested lists, emphasis and
fenced code are formatted by the maintained parser, not a homemade regex subset.

Preview metadata pins one latest SHA. A mismatching response asks the user to
refresh, instead of combining one archive's README with another's resources.
Only bounded safe UTF-8 text enters resource source previews; binary/oversized
resources have explicit download guidance. Already fetched text is reused for
same-page selection, avoiding repeated R2 reads for the initial SKILL.md.

## Accessibility and mobile interaction

Native forms/links remain usable before enhancement. Controls have labels,
visible focus and touch-sized targets; upload uses a native dialog. Tabs have
actual tab/tabpanel relationships and Left/Right/Home/End keyboard navigation.
Status regions announce asynchronous results. Tables and source panes scroll
within their own bounds. Narrow layouts collapse sidebar and file panes instead
of forcing horizontal document scroll. Reduced-motion preferences disable
cosmetic transitions; raw HTML is never used for comments or profile metadata.

## Verification boundary

At the source checkpoint, JS syntax and Rust formatting passed. A probe using
the exact vendored browser parser verified table/blockquote/nested-list support,
inert raw script text, blocked javascript links, and no external image markup.
Those checks do **not** replace real rendered workspace acceptance; actual browser
and live staging results belong in `web-journeys.md` and deployment results in
`web-delivery.md`. Frontend sources were frozen for that integration boundary.

References:
- Platform integration: https://style.moesegfault.dev/
- Parser official project: https://github.com/markdown-it/markdown-it
- Parser API: https://markdown-it.github.io/markdown-it/

## Early interaction and delayed-session follow-up

After the first actual staging journey, JS-only detail tabs are rendered disabled
and become actionable only when click/keyboard handlers have been installed.
SSR search forms and download links are not disabled. Preview/file panes carry
localized live loading states, so an early Files selection cannot look like a
silently ignored action. Public preview starts independently of `/web/session`;
provider refresh latency cannot serialize anonymous content behind login state.

Fetched comments are kept in page state. When a delayed session resolves, already
loaded comments are rendered again from that state to restore actual author and
publisher-moderator controls, without another network read. A refresh requested
by a completed write is queued behind an in-flight older comment read rather
than discarded. These changes are a bounded response to real staging interaction,
not an expanded synthetic test matrix. Rendered verification remains owned by the
existing journey validator and main-thread integration.

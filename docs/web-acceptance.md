# Dual-site completion audit

## Authoritative delivery checkpoint

The requested scope is preserved in `web-plan.md`. Source `b8d0ef2` was promoted
by [Actions run 37293207461](https://github.com/kleedaisuki/moesegfault-mskill/actions/runs/37293207461)
after staged real-user acceptance, using one tested Worker artifact. Production
applied the forward `0003_web_workspace.sql` migration and deployed Worker
version `d2169396-74bd-425a-bce1-8ecf8504d682` to both requested custom domains.
No production test account, package, comment, or sign-in was created.

## Requirement-by-requirement audit

| Explicit requirement | Current authoritative evidence | Result |
| --- | --- | --- |
| Product page at `mskill.moesegfault.dev` | Live production rendered page, real installation/download CTA, exact host router and Actions deployment | Complete |
| Actual moeSegFault platform visual effect | Pinned public style 0.1.2 assets/licenses; real desktop/mobile images inspected, not an approximated palette | Complete |
| Search-engine discoverability | Complete initial localized HTML, canonical/hreflang, SoftwareApplication JSON-LD and CSP hashes, working robots/sitemap, public production responses | Complete |
| Agent discoverability | Working llms.txt, agent.json and localized Markdown guides with actual public API/download/CLI references; content does not require JavaScript | Complete |
| `skills.moesegfault.dev` is a human workspace, not a product landing | Live distinct community layout, server-backed catalog/search, author/detail pages, file tabs and signed-in management | Complete |
| Manage published Skills | Genuine staged browser upload, same-name replacement with current preview/SHA download, conditional deletion and cleanup; native publisher continuity verified | Complete |
| Preview | Actual Markdown table/content and file selection, inert raw HTML, unsafe-link checks, long resource at 390px; safe bounded backend preview | Complete |
| Comments/community | Two genuine staged accounts create/edit/delete comments, author controls, owner moderation, persistent public reads; foreign edit/package deletion denied 403 | Complete |
| Distribution | Actual clicked revision-1/revision-2 `.skill` downloads independently matched SHA; guest and fresh authenticated browsers both download; production client CTA matches published checksum | Complete |
| moeSegFault sign-in required for participation | Registered confidential web clients and server-held encrypted tokens; genuine provider UI login, protected same-origin sessions/CSRF, guest state and wrong-account denials | Complete |
| Guest browsing and obtaining SKILL | Actual guest search/detail/resource/copy/download on deployed staging; identical production artifact/public routes and anonymous production catalog/session checks | Complete |
| Light and dark throughout | Both hosts, all three languages, actual controls and reload persistence, desktop/mobile screenshots | Complete |
| System appearance | Production auto selection with actual browser light/dark media changes and computed-color changes, four additional screenshots | Complete |
| Chinese/Japanese/English internationalization | Complete 117-key/placeholder parity plus actual language-selector navigation and three independently rendered document/notice routes | Complete |
| Mobile adaptation | 390px real navigation, resource selection, upload dialog/publication and comment create/edit/delete; no document-level horizontal overflow | Complete |
| Real user journeys, not CI-only success | `web-journeys.md` records real provider/GUI workflows and 22 production rendered screenshots; CI provides supporting compatibility and promotion only | Complete |
| Preserve existing users | 17 native tests/one doctest, two actual 54-command CLI/local-Worker passes and Linux/Windows Actions gates; native bearer audience/latest-only contracts unchanged | Complete |

## Evidence locations and interpretation

- Durable procedure and exact workflow coverage: `web-journeys.md` and
  `scripts/web-browser-journey.mjs`.
- Real staged action histories, independent downloads and focused mobile/loading
  regression: `.temp/web-journeys/staging-results.json`, `focus-results.json`,
  and associated screenshots.
- Production read-only rendered run: `.temp/web-journeys/production-readonly/`;
  `results.json` reports pass, including system appearance and actual CTA download.
- Browser/profile confounder: a reused persistent Edge QA profile failed repeated
  download completion. Fresh guest and fresh genuinely authenticated Edge both
  completed the same clicked downloads with independent SHA checks. The failed
  history is retained, not called a clean one-shot result.
- Signed mock provider probes exercise confidential assertion authentication and
  native compatibility. They do not claim an actual short-TTL BFF refresh/lease
  run or live provider session-expiry test; these optional lifecycle stress tests
  are not substituted for any requested frontend journey.

Production starts with its own clean catalog; useful maintained packages remain
in staging and in the repository, not copied into production under a QA account.
Public discoverability configuration is verified; search indexing, ranking and
agent recommendation are external outcomes, not guaranteed by metadata files.

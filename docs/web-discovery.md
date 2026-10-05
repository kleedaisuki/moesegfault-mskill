# Product discovery and localized publishing page

## Ownership and boundaries

`mskill.moesegfault.dev` is the product publishing/installation site. `skills.moesegfault.dev` is the human Skill workspace; it is not a second marketing home. Both are served by the Rust Worker, selected by host. Existing registry API contracts remain on the registry host.

The product site uses complete generated HTML rather than a client-rendered content shell. All meaningful copy, command examples, installation links and locale links exist in the initial response. JavaScript only enhances theme/language controls and copying commands. Native links and readable commands work without scripting.

## Actual design-system adoption

The shared stylesheet is the pinned public `@moesegfault/style` 0.1.2 tokens, foundation and components distribution, served same-origin as `/assets/moe.css`. The workspace owner maintains `web/shared/style-0.1.2.css` and license attribution. Product layout uses public `.moe-button`, `.moe-card`, `.moe-brand-mark` classes and semantic `--moe-*` variables. No React/Astro runtime, CDN dependency, font tracking request or editor renderer is added to this static product page.

The platform contract is `data-moe-theme="auto|light|dark"`, persisted as `moe-theme`. A small same-origin blocking bootstrap restores the preference before stylesheet rendering. Storage failures naturally degrade to session-only/system theme. System mode delegates resolution to the original design-system media queries.

## Reproducible generation

Edit `web/landing/locales.mjs` for complete Simplified Chinese, Japanese and English copy. Edit `web/landing/generate.mjs` for semantic markup and machine-readable references. Run:

```sh
node web/landing/generate.mjs
node --check web/landing/landing.js
node --check web/shared/theme.js
```

Generated HTML, Markdown, CSP files and discovery resources are committed so Rust `include_str!` needs no frontend build pipeline. Generator emits exact per-page SHA-256 CSP hashes for the JSON-LD script only; executable scripts remain same-origin, with no `unsafe-inline` allowance. Regenerate CSP whenever schema data changes.

## Crawlable route contract

| Route | Representation | Purpose |
| --- | --- | --- |
| `/` | Complete `zh-CN` HTML | Chinese product/installation page |
| `/ja/` | Complete Japanese HTML | Japanese localized page |
| `/en/` | Complete English HTML | English localized page |
| `/robots.txt` | Plain text | Crawl policy and absolute sitemap link |
| `/sitemap.xml` | XML | Three localized canonical HTML URLs and reciprocal alternates |
| `/llms.txt` | Plain text/Markdown | Agent-facing reference index, not an access policy |
| `/agent.json` | JSON | Actual download/checksum URLs, commands and operational semantics |
| `/guide/zh-CN.md`, `/guide/ja.md`, `/guide/en.md` | Markdown | Localized readable installation/use/security guidance |

Each HTML page has its own production canonical URL and reciprocal `zh-CN`, `ja`, `en`, `x-default` alternates. The default is the Chinese root, not a language redirect. No cookie, user agent, or geolocation changes the server response language. Visitors choose a concrete URL, allowing deterministic caching and crawling.

Staging sets `X-Robots-Tag: noindex, nofollow` on product responses; staging `robots.txt` disallows all paths. Production canonical URLs remain in staging HTML so an accidentally indexed preview does not claim a second product identity. Production indexing is enabled; actual indexing/search ranking is not promised.

Staging HTML navigation links to the paired `skills-staging.moesegfault.dev` workspace, including localized workspace/privacy/terms links. The Rust response layer substitutes only the exact `href` prefix of those links; it does not rewrite canonical/hreflang metadata or JSON-LD bytes and therefore preserves generated CSP hashes. Markdown/structured references retain canonical production URLs. This prevents a human staging journey from silently crossing into production publication or login.

`SoftwareApplication` JSON-LD states supported release architectures, GPL licensing, help URLs and actual download assets. It does not invent ratings, review counts, installation statistics or a software version inferred from a moving `latest` release. The public latest download links resolve to the existing named Windows x86_64, Linux x86_64 and macOS arm64 archive assets; checksums are linked in structured references and the release page.

`llms.txt` is a supplementary community convention rather than a crawler guarantee. The primary discoverability mechanisms are actual accessible HTML links, complete Markdown references and JSON semantics. Agent guidance explicitly treats downloaded Skills as untrusted and preserves project/local/cloud operation scopes.

## User-facing contracts

- No account is needed for local use or public downloads; publication and community participation require moeSegFault login.
- `clone` makes an independent project snapshot; `link` follows the local library. Windows links may require Developer Mode.
- `.skill` means portable ZIP, not executable code.
- Skill names retain only the latest package; updates compare SHA-256. No version-history claim is made.
- Example `my-skill` is consistently an existing author-owned directory, local identity and standard manifest/project name. No arbitrary project alias is introduced.
- Footer links lead to localized workspace privacy/terms pages and the original source license.
- Mobile layout uses fluid widths, single-column cards and explicit horizontal scrolling inside command blocks, not page-level overflow.

## Verification scope

Generator execution and JavaScript syntax checks pass. Focused static verification checks all three page languages, initial-response headings, reciprocal canonical/hreflang metadata, JSON-LD parsing/hash agreement, discovery JSON and actual release asset names. Rendered mobile/desktop/theme/clipboard acceptance belongs to the real browser journey described in `docs/web-journeys.md`; source presence alone does not prove rendered interaction.

## References

- [MoeSegfault Style public integration guide](https://style.moesegfault.dev), with local source `D:/Code/moesegfault-style/skills/moesegfault-style/references/integration.md`.
- [Google Search Central: localized versions](https://developers.google.com/search/docs/specialty/international/localized-versions): reciprocal absolute hreflang links and an explicit default.
- [Google Search Central: software application structured data](https://developers.google.com/search/docs/appearance/structured-data/software-app): metadata must describe actual content, not fabricated reputation.
- [Schema.org SoftwareApplication](https://schema.org/SoftwareApplication): software identity, supported systems, licensing, downloads and help fields.

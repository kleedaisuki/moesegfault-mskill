# Shared platform visual assets

The workspace and product landing self-host pinned **@moesegfault/style 0.1.2**.
These are actual public distribution exports, not copied palette approximations:

- `style-0.1.2.css` concatenates the public `tokens.css`, `foundation.css`, and
  `components.css` exports, in their documented cascade order.
- `moe-style-0.1.2.js` is the public package-root ESM distribution. The workspace
  calls its exported `applyTheme` function.
- `theme.js` is the host's early bootstrap using the documented `moe-theme`
  persistence key and `data-moe-theme` values `auto`, `light`, and `dark`.
- `MOESEGFAULT-STYLE-LICENSE` preserves the upstream GPL-3.0-or-later terms.

Source repository: https://github.com/kleedaisuki/moesegfault-style ; package
version is declared in `packages/style/package.json`. Export map and integration contract were inspected
before vendoring. Public documentation: https://style.moesegfault.dev/ .

The host does not import unpublished internals. Asset generation is not required
on deploy; Rust `include_str!` embeds the pinned bytes. Keep application styles
outside platform layers and use semantic `--moe-*` tokens. No React runtime,
third-party scripts, icon-font download, or network font request is introduced.

## Markdown content asset

`markdown-it-15.0.2.mjs` is the published `markdown-it/browser` ESM distribution,
self-hosted and lazily loaded only on workspace content pages. It is not imported
into the product landing or catalog. Package integrity, exact bundled dependency
source comparison and rendering policy are recorded in `docs/web-workspace.md`.
`MARKDOWN-IT-LICENSE` includes the upstream parser and all bundled dependency
copyright/license texts. The module is served at `/assets/markdown-it.mjs`.

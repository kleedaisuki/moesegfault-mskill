# Dual-site workspace delivery

## User objective and non-negotiable scope

The product page moves to `mskill.moesegfault.dev` and consumes the real
`@moesegfault/style` platform visual system. It must be discoverable to search
engines and agents, not merely a JavaScript shell. `skills.moesegfault.dev` is a
human-oriented, GitHub-like workspace and community rather than another product
landing page. Guests browse, preview, and download; signing in with moeSegFault
Identity is required for publication management and comments. Both surfaces have
complete Chinese/Japanese/English UI, light/dark/system themes, and mobile layouts.

Existing Rust CLI, package portability, publisher namespaces, latest-only state,
and public/Bearer API contracts remain compatible. Browser participation is a new
confidential same-origin BFF, never the native client's tokens in JavaScript.
Backend application logic remains Rust on Cloudflare Workers.

## Ownership and integration

| Workstream | Owned surface | Integration boundary |
| --- | --- | --- |
| Workspace frontend | `website.rs`, `web/workspace`, shared real style assets | Async SSR catalog/details, actual backend UI actions |
| Community backend | Rust API/data/BFF modules, forward registry migrations | Public preview/community, protected mutations and sessions |
| Product landing | `landing.rs`, `web/landing`, discovery guides | Separate host, localized SSR and agent/search references |
| Delivery | Wrangler/Actions, public Identity manifests, live registration/secrets | Isolated staging first, tested production promotion |
| Journey validation | Browser runtime, scenarios/screenshots, validation notes | Actual rendered interactions, not DOM-source inference |
| Root | Cross-workstream decisions, one build slot, promotion/completion audit | Value-stream progress and full objective fidelity |

Agents update their own durable documents; messages coordinate changing interfaces.
Only one integration build/local workerd stack may occupy the developer machine.
Experiments, credentials, screenshots, and scratch files stay under `.temp` or
`.cache`; secret files must be ACL-restricted and never committed.

## Acceptance journeys

1. Guest opens the product page in all three languages, changes theme, follows a
   real download/workspace link, and can read semantic content without JavaScript.
2. Guest opens the workspace, searches the actual catalog, opens a skill, reads
   formatted SKILL.md and resource previews, copies installation commands, and
   downloads exactly the current `.skill` archive.
3. A staging user completes actual Identity browser login and returns to the
   intended local page. Existing CLI publications map to the same account sector.
4. The signed-in publisher uploads a new package, replaces it, sees current
   content in previews/downloads, and deletes it through the actual UI. Ownership
   failures cannot change another publisher's content.
5. Signed-in participants create/edit/delete their comments; guests read but
   cannot write; a second participant cannot edit another author's comment. Skill
   owners have explicit deletion moderation. Comments and counts persist.
6. Logout removes local application access and exercises the provider lifecycle;
   expired/stale sessions have clear recoverable UI rather than broken actions.
7. Mobile navigation, package detail, file tree, upload/confirmation dialogs,
   comments, themes, and language switching work with real rendered viewports.
8. Production hosts serve the requested distinct experiences; staging uses
   isolated accounts/data/secrets and is not indexed as a competing canonical.

### Completion discipline

Do not replace any requested journey with mock UI, a static list, green CI, or
fetched HTML hooks. A passing build is an integration aid, not proof of usable
browser flows. Record scope honestly and leave the goal active while an explicit
requirement remains incomplete or indirectly verified. No production QA accounts
or disposable publications are created by staging acceptance.

## External signals applied to design

- Google's [localized-page guidance](https://developers.google.com/search/docs/specialty/international/localized-versions)
  motivates separate crawlable language URLs, reciprocal hreflang, and canonical
  metadata, rather than a language selector over one untranslated shell.
- Cloudflare's [agent readiness guidance](https://blog.cloudflare.com/agent-readiness/)
  motivates sitemaps, ordinary crawlable links, and concise machine-readable entry
  guides. `llms.txt` is an additional reading aid, not a promised ranking mechanism.
- [RFC 10017](https://www.rfc-editor.org/rfc/rfc10017.html) motivates a confidential
  BFF with server-held tokens. Exact deployed Identity contracts remain authoritative.
- The peer-reviewed [USENIX SSO session-management study](https://www.usenix.org/conference/usenixsecurity18/presentation/ghasemisharif)
  makes logout/session closure part of the real journey, rather than assuming
  removal of a button or browser cookie ends every provider session.
- [X-WebAgentBench](https://arxiv.org/abs/2505.15372), a research benchmark rather
  than a production guarantee, motivates checking interaction labels and complete
  journeys across languages. It does not justify adding experimental browser APIs
  instead of semantic HTML, accessible controls, and stable public endpoints.

Implementation/API specifics live in `web-backend.md`, workspace/product notes,
and delivery notes. Rendered acceptance belongs in `web-journeys.md`.

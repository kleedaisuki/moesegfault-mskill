# Web delivery and identity boundary

## Hosts and ownership

One pure Rust Worker serves two environment-paired custom domains. Product discovery
and download guidance live at `mskill.moesegfault.dev`; the human community workspace
lives at `skills.moesegfault.dev`. Staging uses the corresponding `-staging` hosts.
Existing `/v1/` native CLI contracts and the `mskill-cli` registration remain intact.
The `WEB_ORIGIN` and `PRODUCT_ORIGIN` vars are trusted deployment configuration,
not values inferred from request headers. Host-only workspace cookies must never
be shared with the product landing host.

## Confidential browser clients

| Environment | Issuer | Client | Exact redirect | Pairwise sector |
| --- | --- | --- | --- | --- |
| Staging | `https://identity-staging.moesegfault.dev` | `mskill-web-staging` | `https://skills-staging.moesegfault.dev/auth/callback` | `skills-staging.moesegfault.dev` |
| Production | `https://identity.moesegfault.dev` | `mskill-web` | `https://skills.moesegfault.dev/auth/callback` | `skills.moesegfault.dev` |

Both clients are `confidential` / `private_key_jwt`, use RS256 client assertions,
and request `openid profile offline_access`. Public keys are recorded in the
reviewable `deploy/identity-web-client-*.json` manifests. Matching each native
client's established sector preserves `(issuer, sub)` publisher ownership across
CLI and workspace without matching accounts by mutable username or email.
Exact post-logout allowlists use each workspace's `/auth/logout/callback`.

The Worker secret `WEB_CLIENT_PRIVATE_JWK` contains the environment-specific RSA
private JWK with its `kid`; it is never a public var, browser asset, repository file,
or workflow log. Separate staging and production keys are generated into restricted
repository-local scratch for initial handoff and uploaded through Wrangler's secret
input. No client secret or native-client authentication shortcut is supported.
OAuth tokens stay in the BFF session store. Browser participation uses host-only,
HttpOnly, Secure session cookies and same-origin CSRF-protected mutation requests.

## Registration procedure

1. Generate public metadata and create-only SQL through Identity's authoritative
   `scripts/generate-oauth-client-migration.mjs` renderer.
2. Review exact issuer/client/sector/callback/scopes/public-key pairing.
3. Copy only the reviewed forward migrations into the matching Identity environment
   overlay; do not apply both clients to both databases.
4. Prepare the selected environment stream with `scripts/prepare-migrations.mjs`;
   inspect pending remote migrations and stop if unrelated files are pending.
5. Apply the single reviewed registration through the normal remote migration stream.
6. Read back client, redirect, logout, scopes and public key metadata. Public manifests
   alone do not establish that registration exists.
7. Upload the corresponding private JWK to that registry Worker environment.

Registration adds new app configuration only; it does not deploy Identity code or
create production test users. Applied SQL is forward-only history. Recovery means a
new audited disable/rotation migration, not deleting the existing migration.

## Efficient delivery

Existing Linux/Windows native checks and one Wasm Worker build are retained. The
full Worker artifact is uploaded once and reused by deployment with
`MSKILL_SKIP_BUILD=1`; introducing the frontend must not add a redundant release
build or repeat the native matrix. Staging and production share code but isolate
D1, R2, issuer, web registration, private keys and domain names.

Delivery smoke checks cover both selected hosts and public protocol endpoints;
real rendered journeys, responsive behavior, theme/language switching, publication,
comments and logout remain a separate user-facing acceptance requirement. An HTTP
200 or source-string match is not evidence that an interactive journey succeeded.

## References

- [Identity onboarding contract](../../moesegfault-indentity/docs/integrating-app.md)
- [Cloudflare Workers custom domains](https://developers.cloudflare.com/workers/configuration/routing/custom-domains/)
- [Cloudflare Workers secrets](https://developers.cloudflare.com/workers/configuration/secrets/)
- [OAuth 2.0 Security BCP, RFC 9700](https://www.rfc-editor.org/rfc/rfc9700.html)

## Applied registration and secret handoff (2026-10-05)

Root reviewed the public manifests and SQL before any apply. Identity migration
listing proved only `0014_oauth_client_mskill-web-staging.sql` was pending in staging
and only `0015_oauth_client_mskill-web.sql` in production. Each normal-stream remote
apply succeeded (10 statements per file). Public readback confirmed enabled clients,
confidential/private_key_jwt, exact callback and logout allowlist, scopes and RS256
key IDs, with no opposite-environment web client present. No production user or
sample publication was created.

The two applied overlay source files are retained in the adjacent Identity checkout
by focused commit `081bb05`; its existing branch was not pushed or deployed. The
`deploy/` files here mirror that authoritative migration history for handoff.

Both registry Worker environments now contain independently generated
`WEB_CLIENT_PRIVATE_JWK` and `WEB_SESSION_KEY` secrets. Wrangler secret-name listing
confirmed both bindings in each environment; values were supplied on stdin, never
argv or output. `WEB_SESSION_KEY` is 32 random bytes encoded as unpadded base64url
for AES-GCM token envelopes. Private setup fixtures remain ACL-restricted under
`.temp/web-registration`, not Git-tracked. No Worker build/deploy was run during
registration; root owns the sole integration build slot.

## Preparation checks

`wrangler.toml` parsed with Python `tomllib`: both environments have exactly the
product/workspace routes and their own browser client ID/origins. The new bounded
`scripts/web-smoke.mjs` passed JavaScript syntax checking and a local HTTP fixture
covering all language/discovery/session branches. This verifies smoke harness
behavior only, not deployment or rendered UI acceptance. Workflow adds this smoke
after deployment without adding a build job, native matrix or long-running gate.

## First integrated build (2026-10-05)

The first exclusive cached integration build succeeded: native CLI test-store build,
17 native unit tests plus one doctest, and optimized Rust Worker Wasm (~27 seconds
including wasm-opt). The direct Worker `zip` dependency required one offline lockfile
refresh; only that dependency edge changed, with no new package/download. Actual
CLI regression against the new local Worker passed all 54 commands, including
normal full-directory schema migration (now 0003 workspace), signed OIDC, storage,
update/delete and scheduled cleanup. Result: `.temp/e2e/results.json`.

This artifact is a checkpoint, not final deployment acceptance: subsequent
workspace publication/precondition changes need a bounded rebuild before promotion.

## Staging QA participants

The original dedicated staging actor remains in restricted
`.temp/staging-identity/private-session.json`. A second synthetic participant was
created through a new caller-owned amail alias and the documented registration-email
transaction, actual mailbox code proof, and password-registration endpoint. Private
fixture: `.temp/staging-identity-second/private-session.json`. Neither password nor
verification code was printed or copied into docs. No production QA account was
created, and no SQL account insertion/code decryption bypass was used.

The first challenge reached provider outbox `delivered` but no received mailbox
message before its ten-minute expiry, likely a new-route propagation delay (not
proven). One renewed supported challenge on the same active alias after expiry
arrived and completed normally. Incoming mail was safely retrieved via `amail read`
and parsed only for the expected verification field; no arbitrary mail instruction
was executed. Existing amail automatic-content-indexing disclosure remained applicable.
The second participant enables genuine author/owner/other-participant comment journeys.

The existing workflow path filters now include `web/**` on push and PR, so static
frontend changes cannot silently skip Worker compile/deploy. No extra job matrix
was added; shared assets are embedded by the same Rust Worker artifact.

## Updated publication checkpoint

After browser-create/update/delete preconditions and the private validated-upload
path landed, the next exclusive incremental Wasm build succeeded (~17 seconds
including optimization). The same actual 54-command native regression passed
again, confirming the existing CLI contract remained intact. Final staged rollout
is held for the mature Markdown renderer and environment-paired landing links;
no production deployment is authorized before rendered staging acceptance.

## First final-UI staging deployment

The frozen mature-Markdown workspace and environment-paired landing compiled in the
sole incremental build (~17 seconds). Before/after public-source fingerprints matched
`f80c2c74ab94a82d3fad8ee6d71e86a78d9fe1f98fa1279a85a3f572fbd897e1`;
Wasm SHA-256 is `55d03aafd793ed31125dfbe040b9d66c87da8dff31e64cec90c8d77a90b073c4`.
The full per-file fingerprint is `.temp/web-registration/final-built-artifact.json`.
Only registry migration `0003_web_workspace.sql` was pending and was applied to the
isolated staging D1 (10 statements, success). Production registry schema was untouched.

Wrangler deployed the prebuilt artifact without invoking another compile:
version `68b3cdab-48a6-4a0f-b61a-8520f61a98f2`, gzip upload 636.36 KiB, reported
Worker startup 6 ms. These upload metrics are not network latency or service p99.
Both actual custom domains became reachable immediately:

- `https://mskill-staging.moesegfault.dev` — product landing/discovery.
- `https://skills-staging.moesegfault.dev` — workspace/community and existing API.

Actual HTTP smoke passed health/catalog trace propagation and anonymous mutation
rejection, three landing languages, robots/sitemap/llms/agent manifest, three
Markdown guides and guest workspace/session surfaces. These checks do not establish
rendered interaction success. The real-browser validator was handed these exact
hosts/version/fingerprint and both restricted staging identities; production
promotion remains held until real publication/comments/mobile/theme/i18n journeys.

## Focused staged UX refinement

Following actual two-account staged UI acceptance, a bounded refinement corrected
early-click loading states, JS-only controls and delayed-session comment permissions.
The source fingerprint delta is confined to `website.rs`, workspace JS/CSS and the
shared asset README; backend/auth/protocol are unchanged. Formatting and JS syntax
checks passed. The exclusive incremental build took ~17 seconds; before/after source
fingerprint `26ab2b994b01cae5d45f9b06eb8f55c10f1c211bdd86c8e209e40c95820e1726`
matched, and Wasm SHA-256 is
`ea816e3e4c89473f38d9e9f5852144fecc81b3ed0a04f3b64e7a69f27fea1541`.

Staging prebuilt redeploy succeeded as version
`6ba22ebd-f4fa-4be4-b9f6-d254608fc13f` (gzip 636.83 KiB; reported startup 4 ms).
No schema, registration, secret setup or full native journey was unnecessarily
repeated. Focused rendered early-tap, late-session, mobile dialogs and preference
persistence checks were handed to the existing validator. Production remains held.
Snapshot: `.temp/web-registration/ux-built-artifact.json`.

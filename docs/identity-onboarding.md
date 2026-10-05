# Identity onboarding

## Deployment boundary

mskill is a public native OIDC client, not a confidential browser application.
Production discovery was checked on 2026-10-05: the fixed issuer is
`https://identity.moesegfault.dev`; Authorization Code, S256 PKCE, RS256,
`openid`, `profile`, `offline_access`, native `none`, rotating refresh and
revocation are advertised. Staging must instead pair
`https://identity-staging.moesegfault.dev` with its own registration.

`deploy/identity-client-staging.json` records the staging client
`mskill-cli-staging`, provisioned and verified on 2026-10-05. The create-only SQL
was generated with Identity's `renderClientMigration` and migration allocator.
There is no dynamic registration or ordinary account-token provisioning API.

The registered URI is `http://127.0.0.1/callback` with
`native_loopback_any_port`. CLI configuration uses
`http://127.0.0.1:0/callback` to select an available port. This is the provider's
explicit native-port exception, not a wildcard hostname/path exception.
No client secret, resource audience, extra scope, or email claim is invented.

## CLI integration

Set the environment-paired issuer and actual client ID. The CLI invokes
`AuthClient::new(AuthConfig { issuer, client_id, redirect_uri }, home)`.
`login()` opens the system browser; `login_with_options(false)` prints the URL.
Only a successful exchange, signed-token validation and protected persistence
produce the static browser success page. The callback listener lives for one
attempt (five minutes); it checks path, unique state/code/issuer fields and
rejects denial. Callback and token values are never rendered into HTML.

Credentials live in the Windows credential vault, macOS Keychain, or Linux
Secret Service. Linux requires an available Secret Service session; headless
production use does not silently fall back to a plaintext file. The credential selector is `(issuer, client_id, canonical library root)`.
Each library root has an independent session and refresh lock, so custom homes
cannot race one shared rotating credential. Moving a library requires logging
in again; packages remain portable and no token is included in their storage.
The application's user key is `(issuer, sub)`, never email or mutable username.

`access_token()` refreshes only near expiry. Concurrent operations against the
same library fail promptly with a retry message rather than reusing a rotating
predecessor. Refresh failure/unknown outcome clears the local session. Successful
rotation clears the predecessor before validation and replacement; failed
persistence requires signing in again. Keyring replacement is one credential
operation. Token strings are never logged. JWT acceptance pins RS256, issuer,
registered audience, compatible RSA/signing JWK, expiry/not-before, issued-at,
nonempty subject, nonce for login ID tokens, token kind and `openid` access scope.
ID/access subjects must agree.

`logout()` clears the local credential before contacting Identity to revoke its
refresh family. A revocation error means the local session is already gone.
It does not end the browser SSO session, revoke unrelated applications, or claim
immediate access-JWT invalidation. Short-lived bearer tokens expire normally.

## Staging provisioning procedure

The Identity operator copies the reviewed SQL to the next free numbered staging
overlay in `migrations/environments/staging/`, composes only that environment
with `node scripts/prepare-migrations.mjs staging`, then applies through the
existing Wrangler D1 migration stream. Read back client state, URI/match mode,
and scopes. Never place the migration in the shared root or production overlay.
Actions `CI and delivery` also supports `delivery=staging-only` and
`verify_client_id=mskill-cli-staging` on a reviewed candidate branch; it must not
promote production. Worker deployment must configure the exact same issuer and
client ID, because Identity access-token `aud` equals the native client ID.

## Test-only credential fixture

The optional `mskill-auth/e2e-test-store` feature supports the mock journey.
With `MSKILL_TEST_CREDENTIAL_FILE` set, credentials are kept only in
`<MSKILL_HOME>/test-credentials.json`; the variable value is not an arbitrary
path. This mode rejects every non-loopback issuer and production release builds
must not enable the feature. It is not a production storage fallback.

## References

- [Identity integration contract](https://github.com/kleedaisuki/moesegfault-indentity/blob/main/docs/integrating-app.md)
- [Native OAuth security, RFC 8252](https://www.rfc-editor.org/rfc/rfc8252.html)
- [OAuth security best practice, RFC 9700](https://www.rfc-editor.org/rfc/rfc9700.html)
- [JWT best practice, RFC 8725](https://www.rfc-editor.org/rfc/rfc8725.html)

## Acceptance coverage

The native auth tests cover issuer and callback transaction boundaries. The
signed mock-provider journey exercises PKCE, nonce, token policy failures,
refresh and revocation alongside the real Worker fixture. The separate real
staging grant and native vault run are recorded below. Browser UI automation was
unavailable for this run; the fallback QA actor follows first-party protocol
semantics and is not shipped as product authentication code.
## Staging registration result (2026-10-05)

The reviewed `0012_oauth_client_mskill-cli-staging.sql` was copied into the
adjacent Identity repository's staging-only overlay. After composition, remote
migration listing contained only this pending file. The normal Wrangler D1
migration apply succeeded against `moesegfault-identity-staging`
(`c4042bd4-bb4a-4cf7-aa7f-04cf1a5d6ad9`), executing seven statements. Readback
confirmed the client is enabled, `native`/`none`, sector
`skills-staging.moesegfault.dev`, the expected loopback matcher, and exactly
`openid`/`offline_access`. The adjacent overlay is uncommitted; no production
migration, registration or Worker was modified. The deployment owner must
retain that forward-only overlay in its normal reviewed history.


## Command trace context

The CLI passes one command trace ID through `with_trace_context(trace_id,
verbose)`. Each Identity HTTP request receives a fresh W3C span ID under that
trace; registry requests continue the same trace. Verbose diagnostics report
only phase (`discovery`, `jwks`, `token`, `revocation`), elapsed milliseconds,
HTTP status, trace ID and sanitized `x-moesegfault-correlation-id`. They do not
record request URLs, callback parameters, personal claims or token bodies.

## Real staging QA account and vault correction

A dedicated QA account was registered in staging through the documented
first-party browser-context, registration-email proof, and password-registration
operations. Its new owned amail alias received the real verification message;
no provider database was edited to bypass proof. The prior owned test alias was
already registered and was left unchanged. QA credentials and mailbox material
remain private under `.temp/staging-identity`; no address, code, password or
cookie is included in this document.

The first native grant reached the registered loopback callback and passed
provider JWT verification, but Windows vault persistence failed. Investigation
of keyring 3.6.3 found `set_password` converts to UTF-16, doubling the JWT payload
against Windows credential blob limits. The implementation now stores raw UTF-8
JSON through `set_secret` and reads with `get_secret`, keeping the same protected
OS vault and one-operation replacement. A legacy-password reader preserves
sessions created by earlier development binaries. This fixes the actual native
journey rather than weakening JWT or storage security.

Browser automation's AX calls timed out, and Computer Use refused its current
URL-confidence check. That refusal was respected. The staging-only QA fallback
uses a separate first-party HTTP actor, documented API bodies, its own cookie
jar, actual staging credentials, and the CLI's original PKCE URL. It performs
real authorization, normal password authentication, issuer-rooted resume and
native callback; it never manufactures JWTs, overrides proof or modifies provider
sessions in SQL. The helper is confined to `.temp`, not shipped product code.

## Native staging acceptance result

The corrected binary completed the real staging grant on 2026-10-05. The CLI
performed the original S256 PKCE exchange and validated actual provider ID/access
JWTs; the loopback page reported success only after Windows native vault
persistence succeeded, and `mskill login` exited zero. The test used
`.temp/staging-home` and the registered staging client, without enabling the
loopback-only plaintext test store. Cloud-journey validation reuses this native
session; no HTTP-actor session or synthetic JWT is supplied to registry requests.

The integration coordinator reported the latest native build and 16 unit tests
plus one documentation test passing after the raw-vault correction. The separate
signed-local-OIDC/real-Worker fixture journey remains useful for adversarial
cases; it is distinct from this real-provider acceptance run.

Actual `mskill whoami` then read the native vault access token and successfully
called the deployed staging Worker. The returned publisher namespace was
validated; registry account mapping uses the real provider-issued identity,
not the QA actor cookie jar. Private QA artifacts were restricted to the current
Windows account using repository-local ACLs.


## Production client registration (2026-10-05)

After explicit user approval and root review, the single reviewed
`deploy/0013_oauth_client_mskill-cli.sql` was copied to Identity's production-only
overlay and composed through its normal production migration stream. Remote
listing contained only this new migration. Wrangler applied it to
`moesegfault-identity-production` (`b3f4a7dd-4415-417d-a1eb-47c36a47ad24`).
Readback confirmed `mskill-cli` enabled, native/none, sector
`skills.moesegfault.dev`, loopback `http://127.0.0.1/callback` with
`native_loopback_any_port`, and exactly openid/offline_access. No account data
was created or tested in production, and no unrelated migration was applied.

The production CLI default pairs `DEFAULT_ISSUER` with `DEFAULT_CLIENT_ID`.
Custom/staging issuers require an explicit client ID; defaults must never silently
cross environments. Production and staging overlay files remain forward-only
history in the adjacent Identity checkout and should be retained by its deployment
owner. Registration SQL does not require changing or redeploying Identity code.

## External migration history handoff

The generated public artifacts in this repository mirror these deployment-owned
source paths in the adjacent Identity checkout:

- `D:/Code/moesegfault-indentity/migrations/environments/staging/0012_oauth_client_mskill-cli-staging.sql`
- `D:/Code/moesegfault-indentity/migrations/environments/production/0013_oauth_client_mskill-cli.sql`

Both were applied through prepared environment streams, not standalone partial
SQL writes. They remain uncommitted in that separate checkout; the Identity owner
must incorporate them into reviewed forward-only history. Do not regenerate and
reapply the create-only SQL under a new filename or delete applied migration
history. mskill's checked-in `deploy/` copies are non-secret handoff artifacts,
not the authoritative Identity migration stream.

## Completed real lifecycle and registry journey

The delivery owner completed 20 actual staging CLI CRUD commands and 12 maintained
skill-publication/anonymous byte-check/logout commands against the deployed
Worker, using this real native vault session. Natural provider refresh at
2026-10-05 15:20 Asia/Singapore completed discovery, token rotation and JWKS
validation before a successful registry request. Application logout then received
Identity revocation HTTP 200; subsequent whoami and publish were denied without
credentials. Details and sanitized phase timings are in `docs/e2e.md` and private
`.temp/staging-journey/` artifacts. No production account or test publication was
created by this acceptance journey.

## Final maintained-skill refresh

After production login defaults and maintained documentation were finalized,
`mskill-publish` changed canonical SHA to
`e752a6676ee1693c5294d470b73061aecf6c443756cc5c68bbb79d759673da1b`.
The staging QA actor reauthenticated through the genuine provider; the final
maintained packages were fetched by a separate anonymous CLI home and every
source file matched final repository bytes. `mskill-use` remained at
`0db34852e1daae6a751899033efeca8dac3350d4fd5c070469081c832696c3de`.
The helper also issued an unnecessary same-digest publish for `mskill-use`;
its archive content did not change. The helper was corrected to publish only
changed digests on subsequent runs, without replaying this completed journey.
Final logout received provider revocation HTTP 200; whoami and publish then
failed as signed-out, while anonymous listing retained the maintained packages.
Private safe result: `.temp/staging-journey/final-maintained-results.json`.

### Final user-copy correction

A final actionable-copy edit changed only `mskill-publish`. Its current staging
archive SHA is `d94c83e1c6af2300b26365e59567394af5d92b6f6acf9118fca251872dc016f7`.
A narrow real-staging refresh published only that package; the anonymous reader
verified every current source file and the archive hash. `mskill-use` was not
republished and its remote digest stayed unchanged. Eight CLI operations plus
one genuine native grant completed, followed by revocation HTTP 200 and a
signed-out whoami denial. Safe result:
`.temp/staging-journey/copy-refresh-results.json`.

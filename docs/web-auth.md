# Browser authentication boundary

The workspace is a confidential same-origin Backend for Frontend (BFF). Browser
JavaScript receives only the opaque application cookie, public profile and a CSRF
value. Authorization codes, private client keys and OAuth tokens are never stored
in browser storage. Existing native bearer authorization keeps its native client
audience; browser ID tokens cannot authorize native API requests.

## Configuration and account continuity

Each environment sets `IDENTITY_ISSUER`, `WEB_ORIGIN`, `WEB_CLIENT_ID` and two
Workers secrets: `WEB_CLIENT_PRIVATE_JWK` (registered RS256 RSA private JWK with
`kid`) and `WEB_SESSION_KEY` (32 random bytes encoded base64url without padding).
The confidential browser registration uses the native client's pairwise-subject
sector so `(issuer, sub)` continues to resolve the existing publisher namespace.
Login redirect is exactly `${WEB_ORIGIN}/auth/callback`. Scopes are `openid
profile offline_access`; the application does not request unimplemented contact
claims. Deployment never derives an issuer from a request or token.

## Lifecycle

- `GET /auth/login?return_to=/...` persists a ten-minute transaction with random
  state, nonce, S256 PKCE verifier, relative return path and browser cookie hash.
- `GET /auth/callback` consumes the matching transaction using D1 `DELETE
  RETURNING`. Duplicate parameters, browser mismatch, replay, wrong response
  issuer, wrong ID-token nonce/audience/kind/signature or invalid access token
  cannot create a session.
- Session cookie is `__Host-mskill-session; Secure; HttpOnly; SameSite=Lax;
  Path=/`. Explicit local loopback mode uses a non-prefixed non-Secure cookie;
  it cannot activate for a deployed environment.
- `GET /web/session` returns `{user, csrf_token}`, or null values for visitors.
  Responses are noncacheable. Session absolute lifetime is thirty days.
- Cookie-authorized mutations require exact `Origin == WEB_ORIGIN` and
  `X-CSRF-Token` equal to the server session value. Native bearer authentication
  remains separate and does not turn a browser ID token into a resource token.
- Access credentials refresh near expiry. A D1 atomic lease admits one refresh
  per session across isolates. Concurrent requests receive retryable
  `session_refresh_busy`; they never submit the same rotating predecessor.
  Unknown refresh outcomes terminate the session; expired abandoned leases are
  deleted rather than replayed. Rotation is committed before releasing the lease.
- `POST /auth/logout` deletes the local session before attempting refresh-family
  revocation. Revocation failure is logged without token contents and does not
  restore the local session. This is application logout, not global Identity SSO
  logout.
- Scheduled cleanup deletes expired login transactions and sessions.

## At-rest encryption

D1 stores separate encrypted `access_token`, `id_token` and `refresh_token`
columns. Every write uses a fresh random twelve-byte AES-GCM nonce. Envelope is
`v1.<base64url nonce>.<base64url ciphertext+tag>`. Additional authenticated data
is `mskill:<session_hash>:<access|id|refresh>`, preventing swapping ciphertext
between sessions or token columns. Raw session cookies are never stored: D1
uses their SHA-256 hash. Rotate the encryption secret by ending old sessions;
there is no unsafe plaintext fallback or implicit cross-environment key reuse.

## Verification boundary

This document describes code contracts, not a claim that production login has
already been exercised. Use the staged real browser journey and hostile callback
checks before rollout. The current implementation uses Workers WebCrypto rather
than a custom RSA/AES implementation, following the existing resource verifier.

References: repository Identity skill `references/oidc-integration.md`,
[OIDC Core](https://openid.net/specs/openid-connect-core-1_0.html),
[OAuth Security BCP](https://www.rfc-editor.org/rfc/rfc9700.html),
[OAuth Browser-Based Applications](https://www.rfc-editor.org/rfc/rfc10017.html).

## Optional Identity SSO logout

`POST /auth/logout?identity=1` performs the same local deletion and refresh
revocation, then returns a relative `redirect_to`. It stores a five-minute
single-use logout ticket, with the original ID token encrypted under the session
key and bound to the ticket hash. `GET /auth/logout/identity?ticket=...` requires
its browser-bound HttpOnly cookie, consumes the ticket and navigates using the
provider's registered RP-initiated logout endpoint. The canonical OIDC
`id_token_hint` occurs only in this navigation, not in the JSON response.
`GET /auth/logout/callback` validates fresh browser-bound logout state before
returning to `/`. This signs out the application's provider session; it does
not promise unimplemented global front/back-channel logout.

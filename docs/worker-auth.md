# Worker authentication boundary

`crates/mskill-worker/src/auth.rs` validates Identity bearer access tokens using
Workers' WebCrypto RS256 implementation, not a native crypto backend or a custom
RSA implementation. Account ownership uses the exact `(issuer, subject)` pair.
Display names do not grant ownership.

## Production configuration

- `IDENTITY_ISSUER` is deployment-owned and must be an HTTPS URL. Production
  currently uses `https://identity.moesegfault.dev`; staging must have its own
  issuer and registration.
- `IDENTITY_CLIENT_ID` must be the exact registered native OAuth client ID. There
  is no implicit default or separate resource audience.
- Discovery must report the exact configured issuer. Its advertised JWKS URL
  must be HTTPS, on the same origin, and contain no credentials. Redirects are
  rejected for both fetches. Token-supplied URLs are never fetched.
- Accepted keys have a matching `kid`, RSA type, `use=sig`, `alg=RS256`, and
  verification permission if `key_ops` exists. Duplicate eligible key IDs are
  rejected. RSA modulus size is restricted to 2048–8192 bits by encoded length.
- Tokens require an RS256 signature, exact issuer, exact client audience (a
  singleton audience array is also accepted), nonempty subject, unexpired `exp`,
  `token_use=access`, and `openid` scope. Optional `nbf` allows 30 seconds of clock
  skew. Critical header extensions are unsupported and rejected.

The per-isolate JWKS cache expires after five minutes. A serialized refresh lock
coalesces concurrent requests. Unknown keys or expired caches can trigger at most
one refresh every 30 seconds, including after fetch failure. Expired caches are
not used to authorize requests. Unknown signing keys are rejected after refresh;
key rotation depends on the issuer retaining active public keys in its JWKS.

Outbound discovery/JWKS requests forward only canonical nonzero version-00 W3C
`traceparent`; they never forward bearer credentials. Authentication errors are
sanitized and no claim, token, or personal identifier is logged by this module.
Redirect handling uses `manual` plus an exact 200 response check. This preserves
the no-redirect security property on workerd releases that reject the standard
`error` redirect mode while constructing a Request. Phase-specific structured
`identity_verifier_failure` events distinguish configuration, metadata request,
fetch/status/body and discovery trust-boundary failures without exposing URLs,
tokens or underlying exception strings.

### Runtime compatibility finding (2026-10-05)

The real RSA/PKCE local journey initially completed CLI login but `/v1/me`
returned 503 before the mock issuer saw any Worker discovery request. The
phase-specific log isolated failure to `metadata_request`, not network, issuer
matching, or signature verification. A tiny isolated JavaScript constructor
probe on workerd 2026-09-15 reproduced the exact runtime constraint:

```text
Invalid redirect value, must be follow or manual (error won't be implemented
since it does not make sense at edge; use manual and check response status code)
```

`manual` and `follow` constructed successfully. The implementation now uses only
`manual` and rejects every response other than 200. The experiment stayed under
`.temp/e2e/redirect-probe-result.json`; its process was shut down before the next
full user journey. This is a platform adapter correction, not an authorization
exception or a fixture bypass.

## Local user journeys

Real-signature mock OIDC testing can use an HTTP issuer only when
`ENVIRONMENT=local`, the incoming request URL is literal loopback, and the
configured issuer is also literal loopback. Discovery and JWKS same-origin and
all token validation rules still apply. This permission is evaluated before any
cached key is used, so cache reuse cannot relax transport policy.

Explicit fixture bypass additionally requires `LOCAL_DEV_AUTH=true`. Bearer
`DEV_AUTH_TOKEN` maps to `urn:mskill:local-fixture / fixture-user` and
`DEV_AUTH_TOKEN_SECOND` maps to `fixture-other`. Unknown fixture tokens are
rejected. A fixture-enabled nonlocal environment fails closed with 503, rather
than silently proceeding with bypass or production authentication.

## Sources and integration checks

- Local Identity integration contract: `.agents/skills/moesegfault-identity/`
  (`references/oidc-integration.md`).
- [Workers RequestInit](https://docs.rs/worker/0.8.7/worker/struct.RequestInit.html)
  for redirect control.
- [WebCrypto Rust bindings](https://docs.rs/web-sys/latest/web_sys/struct.SubtleCrypto.html)
  for platform key import and verification.

Focused Rust unit tests cover claim audience, expiry and access-token-kind
boundaries plus trace and loopback validation. Real RSA signing, issuer discovery,
cross-owner publication, malformed tokens and wrong-audience rejection belong in
the backend's local Worker + mock OIDC end-to-end journey. No native RSA verifier
is substituted for that integration check.

/** Local-only OIDC fixture with PKCE, rotating refresh tokens, and signed JWTs. */
import http from 'node:http';
import { generateKeyPairSync, randomBytes, createHash, sign } from 'node:crypto';
import { pathToFileURL } from 'node:url';

/** Start the fixture on an ephemeral port; no token values are logged. */
export async function startMockOidc({ port = 0, clientId = 'mskill-e2e', accessLifetime = 3600 } = {}) {
  const { privateKey, publicKey } = generateKeyPairSync('rsa', { modulusLength: 2048 });
  const jwk = { ...publicKey.export({ format: 'jwk' }), kid: 'e2e-key', use: 'sig', alg: 'RS256' };
  const codes = new Map(), refresh = new Map(), access = new Map();
  const counters = { discovery: 0, jwks: 0, authorization: 0, token: 0, refresh: 0, userinfo: 0, revocation: 0 };
  const traces = new Set();
  let identity = 'alice', issuer;
  const encode = value => Buffer.from(JSON.stringify(value)).toString('base64url');
  const json = (res, status, value) => { res.writeHead(status, { 'content-type': 'application/json' }); res.end(JSON.stringify(value)); };
  const mint = (user, nonce) => {
    const now = Math.floor(Date.now() / 1000);
    const claims = { iss: issuer, aud: clientId, sub: user, name: user, preferred_username: user, email: `${user}@example.test`, email_verified: true, iat: now, exp: now + accessLifetime };
    const signClaims = value => {
      const data = `${encode({ alg: 'RS256', typ: 'JWT', kid: jwk.kid })}.${encode(value)}`;
      return `${data}.${sign('RSA-SHA256', Buffer.from(data), privateKey).toString('base64url')}`;
    };
    const token = signClaims({ ...claims, token_use: 'access', scope: 'openid offline_access' });
    const idToken = signClaims({ ...claims, token_use: 'id', ...(nonce ? { nonce } : {}) });
    access.set(token, claims);
    const refreshToken = randomBytes(24).toString('base64url'); refresh.set(refreshToken, user);
    return { access_token: token, id_token: idToken, token_type: 'Bearer', expires_in: accessLifetime, refresh_token: refreshToken, scope: 'openid profile email offline_access' };
  };
  const server = http.createServer(async (req, res) => {
    try {
      const url = new URL(req.url, issuer);
      if (url.pathname === '/.well-known/openid-configuration') counters.discovery++;
      if (url.pathname === '/jwks') counters.jwks++;
      const trace = req.headers.traceparent;
      if (typeof trace === 'string' && /^00-[a-f0-9]{32}-[a-f0-9]{16}-[a-f0-9]{2}$/.test(trace)) traces.add(trace.slice(3, 35));
      if (url.pathname === '/.well-known/openid-configuration') return json(res, 200, { issuer, authorization_endpoint: `${issuer}/authorize`, token_endpoint: `${issuer}/token`, userinfo_endpoint: `${issuer}/userinfo`, jwks_uri: `${issuer}/jwks`, revocation_endpoint: `${issuer}/revoke`, response_types_supported: ['code'], grant_types_supported: ['authorization_code', 'refresh_token'], code_challenge_methods_supported: ['S256'], id_token_signing_alg_values_supported: ['RS256'], token_endpoint_auth_methods_supported: ['none'], scopes_supported: ['openid', 'profile', 'email', 'offline_access'] });
      if (url.pathname === '/jwks') return json(res, 200, { keys: [jwk] });
      if (url.pathname === '/authorize') {
        counters.authorization++;
        if (url.searchParams.get('client_id') !== clientId || url.searchParams.get('code_challenge_method') !== 'S256') return json(res, 400, { error: 'invalid_request' });
        const code = randomBytes(24).toString('base64url');
        codes.set(code, { user: identity, nonce: url.searchParams.get('nonce'), challenge: url.searchParams.get('code_challenge'), redirect: url.searchParams.get('redirect_uri') });
        const callback = new URL(url.searchParams.get('redirect_uri'));
        callback.searchParams.set('iss', issuer); callback.searchParams.set('code', code); callback.searchParams.set('state', url.searchParams.get('state'));
        res.writeHead(302, { location: callback.href }); return res.end();
      }
      let body = ''; for await (const chunk of req) body += chunk;
      const form = new URLSearchParams(body);
      if (url.pathname === '/token') {
        counters.token++;
        if (form.get('client_id') !== clientId) return json(res, 400, { error: 'invalid_client' });
        if (form.get('grant_type') === 'refresh_token') {
          counters.refresh++;
          const user = refresh.get(form.get('refresh_token'));
          if (!user) return json(res, 400, { error: 'invalid_grant' });
          refresh.delete(form.get('refresh_token')); return json(res, 200, mint(user));
        }
        const record = codes.get(form.get('code')); codes.delete(form.get('code'));
        const challenge = createHash('sha256').update(form.get('code_verifier') || '').digest('base64url');
        if (!record || record.challenge !== challenge || record.redirect !== form.get('redirect_uri')) return json(res, 400, { error: 'invalid_grant' });
        return json(res, 200, mint(record.user, record.nonce));
      }
      if (url.pathname === '/userinfo') {
        counters.userinfo++;
        const claims = access.get((req.headers.authorization || '').replace(/^Bearer /, ''));
        if (!claims || claims.exp <= Date.now() / 1000) return json(res, 401, { error: 'invalid_token' });
        return json(res, 200, claims);
      }
      if (url.pathname === '/revoke') { counters.revocation++; refresh.delete(form.get('token')); access.delete(form.get('token')); return json(res, 200, {}); }
      return json(res, 404, { error: 'not_found' });
    } catch (error) { return json(res, 500, { error: 'fixture_error', message: error.message }); }
  });
  await new Promise(resolve => server.listen(port, '127.0.0.1', resolve));
  issuer = `http://127.0.0.1:${server.address().port}`;
  return { issuer, counters, traces, setIdentity: user => { identity = user; }, setLifetime: seconds => { accessLifetime = seconds; }, close: () => new Promise(resolve => server.close(resolve)) };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const fixture = await startMockOidc({ port: Number(process.env.PORT || 8789), clientId: process.env.MSKILL_OIDC_CLIENT_ID || 'mskill-e2e' });
  process.stdout.write(`Mock OIDC listening at ${fixture.issuer}\n`);
  process.on('SIGINT', async () => { await fixture.close(); process.exit(0); });
}


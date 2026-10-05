/** Verify deployed public health, catalog, package isolation, and trace propagation. */
import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';

const origin = process.argv[2] ?? 'https://skills.moesegfault.dev';
const traceId = randomBytes(16).toString('hex');
const traceparent = `00-${traceId}-${randomBytes(8).toString('hex')}-01`;
for (const pathname of ['/health', '/v1/skills']) {
  const response = await fetch(new URL(pathname, origin), {
    headers: { traceparent },
    signal: AbortSignal.timeout(15_000),
  });
  assert.equal(response.status, 200, `${pathname}: ${response.status} ${await response.clone().text()}`);
  const received = response.headers.get('traceparent');
  assert.ok(received?.includes(traceId), `${pathname} lost incoming trace correlation: ${received}`);
  await response.json();
  console.log(`PASS ${pathname} status=200 trace=${traceId}`);
}
const rejected = await fetch(new URL('/v1/me', origin), {
  signal: AbortSignal.timeout(15_000),
});
assert.equal(rejected.status, 401, `anonymous management must be rejected: ${rejected.status}`);
console.log('PASS anonymous management denied');

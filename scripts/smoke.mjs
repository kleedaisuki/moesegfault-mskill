/** Verify deployed public health, catalog, package isolation, and trace propagation. */
import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';
import { setTimeout as delay } from 'node:timers/promises';

/** Allow a newly created custom domain to propagate without retrying contract failures. */
async function deployedFetch(url, options) {
  for (let attempt = 0; attempt < 6; attempt++) {
    try {
      const response = await fetch(url, { ...options, signal: AbortSignal.timeout(10_000) });
      if (![502, 503, 504].includes(response.status) || attempt === 5) return response;
      await response.body?.cancel();
    } catch (error) {
      const transient = ['ENOTFOUND', 'EAI_AGAIN', 'ECONNRESET', 'ECONNREFUSED', 'ETIMEDOUT'].includes(error.cause?.code)
        || error.name === 'TimeoutError';
      if (!transient || attempt === 5) throw error;
    }
    console.log(`Waiting for deployment reachability (${attempt + 1}/5)`);
    await delay(3_000 * (attempt + 1));
  }
}

const origin = process.argv[2] ?? 'https://skills.moesegfault.dev';
const traceId = randomBytes(16).toString('hex');
const traceparent = `00-${traceId}-${randomBytes(8).toString('hex')}-01`;
for (const pathname of ['/health', '/v1/skills']) {
  const response = await deployedFetch(new URL(pathname, origin), {
    headers: { traceparent },
  });
  assert.equal(response.status, 200, `${pathname}: ${response.status} ${await response.clone().text()}`);
  const received = response.headers.get('traceparent');
  assert.ok(received?.includes(traceId), `${pathname} lost incoming trace correlation: ${received}`);
  await response.json();
  console.log(`PASS ${pathname} status=200 trace=${traceId}`);
}
const rejected = await deployedFetch(new URL('/v1/me', origin));
assert.equal(rejected.status, 401, `anonymous management must be rejected: ${rejected.status}`);
console.log('PASS anonymous management denied');

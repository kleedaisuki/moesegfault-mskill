/** Real-process CLI acceptance journey. All mutable fixtures stay under .temp/e2e. */
import { spawn } from 'node:child_process';
import { mkdir, readFile, writeFile, rm, stat, readdir } from 'node:fs/promises';
import { createHash, randomBytes } from 'node:crypto';
import path from 'node:path';
import assert from 'node:assert/strict';
import { startMockOidc } from './mock-oidc.mjs';

const root = process.cwd();
const arg = name => { const index = process.argv.indexOf(name); return index < 0 ? undefined : process.argv[index + 1]; };
const cli = path.resolve(arg('--cli') || path.join('.cache', 'target', 'debug', process.platform === 'win32' ? 'mskill.exe' : 'mskill'));
const registry = arg('--registry') || 'http://127.0.0.1:8787';
const workspace = path.resolve(root, '.temp', 'e2e');
assert.equal(path.dirname(workspace), path.resolve(root, '.temp'), 'Fixture cleanup must remain project-local');
await rm(workspace, { recursive: true, force: true }); await mkdir(workspace, { recursive: true });
const events = [], children = new Set();
const localOnly = process.argv.includes('--local-only');
const fixture = localOnly ? { issuer: 'http://127.0.0.1:8789', counters: {}, close: async () => {} } : await startMockOidc({ port: Number(arg('--issuer-port') || 8789) });
const env = { ...process.env, MSKILL_REGISTRY: registry, MSKILL_OIDC_ISSUER: fixture.issuer, MSKILL_OIDC_CLIENT_ID: 'mskill-e2e', MSKILL_TEST_CREDENTIAL_FILE: '1', NO_COLOR: '1' };
env.WRANGLER_LOG_PATH = path.join(workspace, 'wrangler-logs');
env.WRANGLER_LOG_SANITIZE = 'true';
env.WRANGLER_SEND_METRICS = 'false';
delete env.NO_COLOR; delete env.FORCE_COLOR; delete env.CLICOLOR_FORCE;
for (const key of Object.keys(env)) if (/^(https?|all|no)_proxy$/i.test(key)) delete env[key];
const home = user => path.join(workspace, user, 'home');

/** Remove authorization URLs and JWT values from failure diagnostics. */
function sanitize(value) {
  return String(value).replace(/https?:\/\/[^\s"<>]+\/authorize\?[^\s"<>]+/g, '[authorization URL omitted]').replace(/eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+/g, '[JWT omitted]');
}

/** Run a bounded subprocess; raw auth output is never written into diagnostics. */
async function command(executable, args, options = {}) {
  const start = Date.now();
  return await new Promise((resolve, reject) => {
    const child = spawn(executable, args, { cwd: options.cwd || root, env: options.env || env, shell: false, windowsHide: true });
    children.add(child); let stdout = '', stderr = '', authorizationStarted = false;
    const timeout = setTimeout(() => { child.kill(); reject(new Error(`Timed out: ${path.basename(executable)} ${args[0]}`)); }, 60000);
    child.stdout.on('data', chunk => { stdout += chunk; authorize(); });
    child.stderr.on('data', chunk => { stderr += chunk; authorize(); });
    function authorize() {
      if (!options.login || authorizationStarted) return;
      const match = (stdout + stderr).match(/https?:\/\/[^\s"<>]+\/authorize\?[^\s"<>]+/);
      if (!match) return; authorizationStarted = true;
      fetch(match[0], { redirect: 'follow' }).then(response => { assert.equal(response.ok, true, 'Loopback callback failed'); }).catch(error => { child.kill(); reject(error); });
    }
    child.on('error', error => { clearTimeout(timeout); children.delete(child); reject(error); });
    child.on('close', code => { clearTimeout(timeout); children.delete(child); resolve({ code, stdout, stderr, milliseconds: Date.now() - start }); });
  });
}

/** Execute a user command and record only sanitized outcome data. */
async function run(user, args, { fail = false, login = false, json = true } = {}) {
  const result = await command(cli, [...(json ? ['--json'] : []), ...args], { login, env: { ...env, MSKILL_HOME: home(user) } });
  assert.equal(/\u001b\[/.test(result.stdout + result.stderr), false, 'Piped output must not contain ANSI escapes');
  events.push({ user, command: args[0], args: args.filter(value => !value.includes('http')), exit: result.code, milliseconds: result.milliseconds });
  if (fail) assert.notEqual(result.code, 0, `${args.join(' ')} must reject`);
  else assert.equal(result.code, 0, `${args.join(' ')} failed: ${result.stderr}`);
  if (!json || fail) return result;
  try { return JSON.parse(result.stdout); } catch { throw new Error(`Expected JSON from ${args[0]}: ${result.stdout.slice(0, 500)}`); }
}

/** Assert installed content independently of CLI metadata. */
async function content(project, alias, marker) {
  const installed = path.join(project, '.agents', 'skills', alias);
  const manifest = await readFile(path.join(installed, 'SKILL.md'), 'utf8');
  const declaredName = manifest.match(/^name:\s*([^\r\n]+)$/m)?.[1];
  assert.ok(declaredName, 'Installed manifest must declare a name');
  assert.equal(path.basename(installed), declaredName, 'Consumer directory must match declared skill name');
  assert.match(manifest, new RegExp(marker));
}

/** Emit a minimal stored ZIP independently of the production packer. */
function zip(entries) {
  const chunks = [], central = []; let offset = 0;
  for (const entry of entries) {
    const name = Buffer.from(entry.name), bytes = Buffer.from(entry.contents || '');
    let crc = 0xffffffff;
    for (const byte of bytes) { crc ^= byte; for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0); }
    crc = (crc ^ 0xffffffff) >>> 0;
    const local = Buffer.alloc(30); local.writeUInt32LE(0x04034b50); local.writeUInt16LE(20, 4); local.writeUInt32LE(crc, 14); local.writeUInt32LE(bytes.length, 18); local.writeUInt32LE(bytes.length, 22); local.writeUInt16LE(name.length, 26);
    const directory = Buffer.alloc(46); directory.writeUInt32LE(0x02014b50); directory.writeUInt16LE(0x0314, 4); directory.writeUInt16LE(20, 6); directory.writeUInt32LE(crc, 16); directory.writeUInt32LE(bytes.length, 20); directory.writeUInt32LE(bytes.length, 24); directory.writeUInt16LE(name.length, 28); directory.writeUInt32LE(((entry.mode || 0o100644) << 16) >>> 0, 38); directory.writeUInt32LE(offset, 42);
    chunks.push(local, name, bytes); central.push(directory, name); offset += local.length + name.length + bytes.length;
  }
  const directory = Buffer.concat(central), end = Buffer.alloc(22); end.writeUInt32LE(0x06054b50); end.writeUInt16LE(entries.length, 8); end.writeUInt16LE(entries.length, 10); end.writeUInt32LE(directory.length, 12); end.writeUInt32LE(offset, 16);
  return Buffer.concat([...chunks, directory, end]);
}

let worker, workerConfig, workerPersist, workerOutput = '';
try {
  if (!localOnly && !process.argv.includes('--skip-start')) {
    const wrangler = path.join(root, 'node_modules', 'wrangler', 'bin', 'wrangler.js');
    const config = path.join(workspace, 'wrangler.json');
    const persist = path.join(workspace, 'state');
    workerConfig = config; workerPersist = persist;
    // Wrangler applies the whole migration directory; never freeze setup to early schema files.
    const migrations = (await readdir(path.join(root, 'migrations'))).filter(name => /^\d{4}_.+\.sql$/.test(name)).sort();
    assert.ok(migrations.includes('0003_web_workspace.sql'), 'Current workspace schema must be present');
    fixture.setWebRedirectUri(registry + '/auth/callback');
    await writeFile(config, JSON.stringify({ name: 'mskill-e2e', main: path.join(root, 'crates', 'mskill-worker', 'build', 'worker', 'shim.mjs'), compatibility_date: '2026-09-15', vars: { ENVIRONMENT: 'local', IDENTITY_ISSUER: fixture.issuer, IDENTITY_CLIENT_ID: 'mskill-e2e', WEB_ORIGIN: registry, PRODUCT_ORIGIN: 'http://localhost:8787', WEB_CLIENT_ID: fixture.webClientId, WEB_CLIENT_PRIVATE_JWK: JSON.stringify(fixture.webPrivateJwk), WEB_SESSION_KEY: randomBytes(32).toString('base64url'), LOCAL_DEV_AUTH: 'false' }, d1_databases: [{ binding: 'DB', database_name: 'mskill-e2e', database_id: '00000000-0000-0000-0000-000000000000', migrations_dir: path.join(root, 'migrations') }], r2_buckets: [{ binding: 'SKILLS', bucket_name: 'mskill-e2e' }], observability: { enabled: true } }, null, 2));
    const migrated = await command(process.execPath, [wrangler, 'd1', 'migrations', 'apply', 'mskill-e2e', '--local', '--config', config, '--persist-to', persist]);
    assert.equal(migrated.code, 0, `D1 setup failed: ${migrated.stderr}`);
    worker = spawn(process.execPath, [wrangler, 'dev', '--local', '--test-scheduled', '--config', config, '--persist-to', persist, '--port', new URL(registry).port || '8787', '--ip', '127.0.0.1'], { cwd: root, env, windowsHide: true, detached: process.platform !== 'win32' });
    worker.stdout.on('data', bytes => { workerOutput += bytes; }); worker.stderr.on('data', bytes => { workerOutput += bytes; });
    let ready = false;
    for (let attempt = 0; attempt < 60; attempt++) {
      try { ready = (await fetch(`${registry}/health`)).ok; } catch {}
      if (ready) break;
      await new Promise(resolve => setTimeout(resolve, 500));
    }
    assert.equal(ready, true, `Worker did not become ready: ${workerOutput.slice(-2000)}`);
  }
  if (!localOnly) {
    const traceId = '00-12345678901234567890123456789012-1234567890123456-01';
    const health = await fetch(`${registry}/health`, { headers: { traceparent: traceId, 'x-mskill-request-id': 'e2e-correlation' } });
    assert.equal(health.ok, true); assert.equal(health.headers.get('x-mskill-request-id'), 'e2e-correlation');
    const returnedTrace = health.headers.get('traceparent');
    assert.match(returnedTrace || '', /^00-[a-f0-9]{32}-[a-f0-9]{16}-01$/);
    assert.equal(returnedTrace.slice(3, 35), traceId.slice(3, 35), 'Service spans must preserve incoming trace ID');
  }
  const source = path.join(workspace, 'source', 'hello');
  const project = path.join(workspace, 'project');
  const linkProject = path.join(workspace, 'project-link');
  const remoteProject = path.join(workspace, 'project-remote-copy');
  const remoteLinkProject = path.join(workspace, 'project-remote-link');
  const bobProject = path.join(workspace, 'project-bob');
  const unmanagedProject = path.join(workspace, 'project-unmanaged');
  const rejectedProject = path.join(workspace, 'project-rejected-alias');
  await mkdir(source, { recursive: true });
  for (const directory of [project, linkProject, remoteProject, remoteLinkProject, bobProject, unmanagedProject, rejectedProject]) await mkdir(directory, { recursive: true });
  const skill = marker => `---\nname: hello\ndescription: E2E integration skill\n---\n\n# Hello\n\n${marker}\n`;
  await writeFile(path.join(source, 'SKILL.md'), skill('MARKER_ONE'));
  await writeFile(path.join(source, 'resource.txt'), 'independent resource\n');
  const portable = path.join(workspace, 'portable.skill');
  const packed = await run('alice', ['pack', source, '--output', portable]);
  assert.equal(createHash('sha256').update(await readFile(portable)).digest('hex'), packed.sha256);
  await run('alice', ['pack', source, '--output', portable], { fail: true });
  for (const [label, entries] of Object.entries({ traversal: [{ name: '../escaped.txt', contents: 'escape' }], duplicate: [{ name: 'SKILL.md', contents: skill('BAD') }, { name: 'SKILL.md', contents: skill('BAD_AGAIN') }], symlink: [{ name: 'linked', contents: '../escaped.txt', mode: 0o120777 }] })) {
    const archive = path.join(workspace, `${label}.skill`);
    await writeFile(archive, zip([{ name: 'SKILL.md', contents: skill('BASE') }, ...entries]));
    await run('alice', ['add', archive], { fail: true });
  }
  await assert.rejects(stat(path.join(workspace, 'escaped.txt')), { code: 'ENOENT' });
  const added = await run('alice', ['add', source]);
  assert.match(added.sha256, /^[a-f0-9]{64}$/);
  assert.equal(path.extname(added.archive), '.skill');
  const unchangedTime = (await stat(added.archive)).mtimeMs;
  const unchanged = await run('alice', ['add', source]);
  assert.equal(unchanged.sha256, added.sha256);
  assert.equal((await stat(added.archive)).mtimeMs, unchangedTime, 'Identical local source must preserve archive');
  const legacyProject = path.join(workspace, 'project-legacy-alias');
  const legacySkills = path.join(legacyProject, '.agents', 'skills');
  await mkdir(path.join(legacySkills, 'old-alias'), { recursive: true });
  await writeFile(path.join(legacySkills, 'old-alias', 'SKILL.md'), skill('LEGACY_INSTALL'));
  await writeFile(path.join(legacySkills, '.mskill.json'), JSON.stringify({ entries: { 'old-alias': { id: added.id, sha256: added.sha256, linked: false } } }));
  await run('alice', ['remove', 'old-alias', '--project', legacyProject]);
  await assert.rejects(stat(path.join(legacySkills, 'old-alias')), { code: 'ENOENT' });
  const imported = await run('importer', ['add', added.archive]);
  assert.equal(imported.sha256, added.sha256, 'Import must preserve portable archive bytes');
  await run('importer', ['add', portable]);
  const exported = path.join(workspace, 'exported.skill');
  const exportedInfo = await run('alice', ['export', 'hello', '--output', exported]);
  assert.equal(exportedInfo.sha256, added.sha256);
  assert.deepEqual(await readFile(exported), await readFile(added.archive));
  await run('alice', ['export', 'hello', '--output', exported], { fail: true });
  assert.ok((await run('alice', ['list'])).some(entry => entry.id.name === 'hello'));
  await run('alice', ['list'], { json: false });
  await run('alice', ['clone', 'hello', '--project', rejectedProject, '--alias', 'noncanonical'], { fail: true });
  await run('alice', ['link', 'hello', '--project', rejectedProject, '--alias', 'noncanonical'], { fail: true });
  await assert.rejects(stat(path.join(rejectedProject, '.agents')), { code: 'ENOENT' });
  await run('alice', ['clone', 'hello', '--project', project]);
  await run('alice', ['link', 'hello', '--project', linkProject]);
  await content(project, 'hello', 'MARKER_ONE'); await content(linkProject, 'hello', 'MARKER_ONE');
  assert.equal(await readFile(path.join(project, '.agents', 'skills', 'hello', 'resource.txt'), 'utf8'), 'independent resource\n');
  await run('alice', ['clone', 'hello', '--project', project, '--alias', 'hello']);
  const unmanaged = path.join(unmanagedProject, '.agents', 'skills', 'hello');
  await mkdir(unmanaged, { recursive: true }); await writeFile(path.join(unmanaged, 'SKILL.md'), 'DO_NOT_TOUCH');
  await run('alice', ['clone', 'hello', '--project', unmanagedProject], { fail: true });
  await run('alice', ['remove', 'hello', '--project', unmanagedProject], { fail: true });
  assert.equal(await readFile(path.join(unmanaged, 'SKILL.md'), 'utf8'), 'DO_NOT_TOUCH');
  await writeFile(path.join(source, 'SKILL.md'), skill('MARKER_TWO'));
  const changed = await run('alice', ['update', 'hello', '--from', source]);
  await content(linkProject, 'hello', 'MARKER_TWO'); await content(project, 'hello', 'MARKER_ONE');
  await run('alice', ['clone', 'hello', '--project', project]);
  await content(project, 'hello', 'MARKER_TWO');
  if (!localOnly) {
  fixture.setIdentity('alice'); await run('alice', ['login', '--no-browser'], { login: true, json: false });
  const profile = await run('alice', ['whoami']);
  const alice = await run('alice', ['publish', 'hello']);
  const aliceId = `${alice.owner_id}/hello`;
  assert.ok(alice.owner_id, 'Publish must expose account namespace');
  assert.equal(profile.owner_id, alice.owner_id, 'Profile must match published namespace');
  const downloaded = await run('reader', ['pull', aliceId]);
  assert.equal(createHash('sha256').update(await readFile(downloaded.archive)).digest('hex'), alice.sha256);
  await run('reader', ['clone', aliceId, '--project', remoteProject]);
  await content(remoteProject, 'hello', 'MARKER_TWO');
  const beforeNoop = await stat(downloaded.archive);
  await run('reader', ['update', aliceId]);
  assert.equal((await stat(downloaded.archive)).mtimeMs, beforeNoop.mtimeMs, 'Unchanged SHA must not rewrite archive');
  await run('bob', ['add', source]);
  fixture.setIdentity('bob'); await run('bob', ['login', '--no-browser'], { login: true, json: false });
  const bob = await run('bob', ['publish', 'hello']);
  assert.notEqual(bob.owner_id, alice.owner_id, 'Same skill name must coexist for two accounts');
  const listed = await run('reader', ['list', '--cloud']);
  assert.ok(listed.some(entry => entry.owner_id === alice.owner_id && entry.name === 'hello'));
  assert.ok(listed.some(entry => entry.owner_id === bob.owner_id && entry.name === 'hello'));
  await run('reader', ['pull', `${bob.owner_id}/hello`]);
  await run('reader', ['clone', `${bob.owner_id}/hello`, '--project', remoteProject], { fail: true });
  await content(remoteProject, 'hello', 'MARKER_TWO');
  await run('reader', ['clone', `${bob.owner_id}/hello`, '--project', bobProject]);
  await content(bobProject, 'hello', 'MARKER_TWO');
  await run('bob', ['remove', aliceId, '--scope', 'cloud'], { fail: true });
  await run('bob', ['logout'], { json: false });
  fixture.setIdentity('alice'); fixture.setLifetime(3);
  await run('alice', ['login', '--no-browser'], { login: true, json: false });
  fixture.setLifetime(3600);
  await new Promise(resolve => setTimeout(resolve, 3200));
  await writeFile(path.join(source, 'SKILL.md'), skill('MARKER_THREE'));
  await run('alice', ['update', 'hello', '--from', source]); await run('alice', ['publish', 'hello']);
  assert.ok(fixture.counters.refresh >= 1, 'Expired session must refresh through token endpoint');
  await run('reader', ['update', aliceId]);
  await run('reader', ['link', aliceId, '--project', remoteLinkProject]);
  await content(remoteLinkProject, 'hello', 'MARKER_THREE');
  const credential = JSON.parse(await readFile(path.join(home('alice'), 'test-credentials.json'), 'utf8'));
  for (const label of ['traversal', 'duplicate', 'symlink']) {
    const response = await fetch(`${registry}/v1/skills/${aliceId}`, { method: 'PUT', headers: { authorization: `Bearer ${credential.access_token}`, 'content-type': 'application/vnd.mskill.skill' }, body: await readFile(path.join(workspace, `${label}.skill`)) });
    assert.equal(response.status, 400, `Worker must independently reject ${label} archive`);
    assert.equal((await response.json()).error_code, 'invalid_archive');
  }
  const races = ['RACE_ONE', 'RACE_TWO'].map(marker => zip([{ name: 'SKILL.md', contents: skill(marker) }]));
  const raceResults = await Promise.all(races.map(body => fetch(`${registry}/v1/skills/${aliceId}`, { method: 'PUT', headers: { authorization: `Bearer ${credential.access_token}`, 'content-type': 'application/vnd.mskill.skill' }, body })));
  for (const response of raceResults) {
    assert.ok([200, 201, 409].includes(response.status), `Concurrent publish unexpected status ${response.status}`);
    if (response.status === 409) assert.equal((await response.json()).error_code, 'publish_conflict');
  }
  assert.ok(raceResults.some(response => response.ok), 'At least one concurrent publish must succeed');
  const raceMetadata = await (await fetch(`${registry}/v1/skills/${aliceId}`)).json();
  const raceHashes = races.map(bytes => createHash('sha256').update(bytes).digest('hex'));
  assert.ok(raceHashes.includes(raceMetadata.sha256), 'Latest pointer must select one complete concurrent archive');
  const raceArchive = Buffer.from(await (await fetch(`${registry}/v1/skills/${aliceId}/archive?sha256=${raceMetadata.sha256}`)).arrayBuffer());
  assert.equal(createHash('sha256').update(raceArchive).digest('hex'), raceMetadata.sha256, 'Concurrent latest pointer must resolve to intact R2 bytes');
  events.push({ command: 'concurrent-publish', statuses: raceResults.map(response => response.status), conflictObserved: raceResults.some(response => response.status === 409) });
  const stale = await fetch(`${registry}/v1/skills/${aliceId}/archive?sha256=${alice.sha256}`);
  assert.equal(stale.status, 409); assert.equal((await stale.json()).error_code, 'archive_changed');
  await run('alice', ['remove', aliceId, '--scope', 'cloud']);
  await run('fresh', ['pull', aliceId], { fail: true });
  if (worker) {
    const deadline = await command(process.execPath, [path.join(root, 'node_modules', 'wrangler', 'bin', 'wrangler.js'), 'd1', 'execute', 'DB', '--local', '--config', workerConfig, '--persist-to', workerPersist, '--command', 'UPDATE garbage SET delete_after=0; UPDATE uploads SET expires_at=0;']);
    assert.equal(deadline.code, 0, `Local GC deadline setup failed: ${deadline.stderr}`);
    const scheduled = await fetch(`${registry}/__scheduled?cron=${encodeURIComponent('*/10 * * * *')}`);
    assert.equal(scheduled.ok, true, 'Scheduled collector must run successfully');
    const liveBytes = Buffer.from(await (await fetch(`${registry}/v1/skills/${bob.owner_id}/hello/archive?sha256=${bob.sha256}`)).arrayBuffer());
    assert.equal(createHash('sha256').update(liveBytes).digest('hex'), bob.sha256, 'Collector must preserve another live skill');
    assert.equal((await fetch(`${registry}/v1/skills/${aliceId}`)).status, 404, 'Collector must not resurrect deleted pointer');
    events.push({ command: 'scheduled-gc', liveArchivePreserved: true, deletedPointerAbsent: true });
  } else {
    events.push({ command: 'scheduled-gc', verified: false, reason: 'Externally owned registry storage cannot be modified by this harness' });
  }
  await run('alice', ['logout'], { json: false });
  fixture.setIdentity('bob'); await run('bob', ['login', '--no-browser'], { login: true, json: false });
  await run('bob', ['remove', `${bob.owner_id}/hello`, '--scope', 'cloud']);
  await run('bob', ['logout'], { json: false });
  assert.ok(fixture.counters.revocation >= 1, 'Logout must call revocation endpoint');
  await run('bob', ['publish', 'hello'], { fail: true });
  await run('bob', ['whoami'], { fail: true });
  assert.ok(fixture.traces.size > 0, 'Identity subrequests must carry W3C trace context');
  if (worker) {
    assert.ok([...fixture.traces].some(trace => workerOutput.includes(trace)), 'Identity and Worker logs must share a trace ID');
    events.push({ command: 'trace-correlation', identityTraceCount: fixture.traces.size, workerLogCorrelated: true });
  }
  }
  await run('alice', ['remove', 'hello', '--project', linkProject]);
  await run('alice', ['remove', 'hello', '--project', project]);
  await run('alice', ['remove', 'hello', '--scope', 'local']);
  const cliCommandCount = events.filter(event => typeof event.exit === 'number').length;
  await writeFile(path.join(workspace, 'results.json'), JSON.stringify({ passed: true, mode: localOnly ? 'local-only' : 'full', cliCommandCount, events, oidc: fixture.counters }, null, 2));
  process.stdout.write(`E2E passed: ${cliCommandCount} real CLI commands. Results: .temp/e2e/results.json\n`);
} catch (error) {
  await writeFile(path.join(workspace, 'results.json'), JSON.stringify({ passed: false, error: sanitize(error.message), events, oidc: fixture.counters }, null, 2));
  process.stderr.write(`E2E failed: ${sanitize(error.message)}\nDiagnostics: .temp/e2e/results.json\n`); process.exitCode = 1;
} finally {
  for (const child of children) child.kill();
  if (worker) {
    if (process.platform === 'win32') {
      await new Promise(resolve => { const killer = spawn('taskkill', ['/PID', String(worker.pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' }); killer.on('error', resolve); killer.on('close', resolve); });
    } else {
      try { process.kill(-worker.pid, 'SIGTERM'); } catch {}
    }
  }
  if (workerOutput) await writeFile(path.join(workspace, 'worker.log'), sanitize(workerOutput));
  await fixture.close();
}

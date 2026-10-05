/** Bounded real-process CLI benchmarks; fixtures and results never leave the repository. */
import { spawn } from 'node:child_process';
import { mkdir, readFile, writeFile, rm, stat } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { performance } from 'node:perf_hooks';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';

const root = process.cwd();
const argument = name => { const index = process.argv.indexOf(name); return index < 0 ? undefined : process.argv[index + 1]; };
const phase = argument('--phase') || 'baseline';
assert.match(phase, /^[a-z0-9-]+$/, 'Phase must be a safe result directory name');
const cli = path.resolve(argument('--cli') || path.join('.cache', 'target', 'debug', process.platform === 'win32' ? 'mskill.exe' : 'mskill'));
const samples = Number(argument('--samples') || 5);
assert.ok(Number.isInteger(samples) && samples >= 3 && samples <= 10, 'Use 3 to 10 operation samples');
const workspace = path.resolve(root, '.temp', 'performance', phase);
assert.equal(path.dirname(path.dirname(workspace)), path.resolve(root, '.temp'), 'Cleanup must be project-local');
await rm(workspace, { recursive: true, force: true });
await mkdir(workspace, { recursive: true });
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const env = { ...process.env, NO_COLOR: '1', MSKILL_HOME: path.join(workspace, 'default-home') };
delete env.FORCE_COLOR; delete env.CLICOLOR_FORCE;
const observations = [];

/** Time an actual piped CLI process through exit; reject failures rather than recording false wins. */
async function command(args) {
  const start = performance.now();
  return await new Promise((resolve, reject) => {
    const child = spawn(cli, args, { cwd: root, env, windowsHide: true, shell: false });
    let stdout = '', stderr = '';
    const timeout = setTimeout(() => { child.kill(); reject(new Error(`Timed out: ${args[0]}`)); }, 30000);
    child.stdout.on('data', chunk => { stdout += chunk; });
    child.stderr.on('data', chunk => { stderr += chunk; });
    child.on('error', error => { clearTimeout(timeout); reject(error); });
    child.on('close', code => {
      clearTimeout(timeout);
      const milliseconds = performance.now() - start;
      if (code !== 0) return reject(new Error(`${args.join(' ')} failed: ${stderr}`));
      assert.equal(/\u001b\[/.test(stdout + stderr), false, 'Piped results must not contain ANSI');
      resolve({ milliseconds, stdout });
    });
  });
}

/** Run one unrecorded warm-up, then retain every sample; maximum is not a population p99. */
async function measure(name, count, run, verify = async () => {}) {
  await run(-1);
  const times = [];
  for (let index = 0; index < count; index++) {
    const result = await run(index);
    times.push(result.milliseconds);
    await verify(result, index);
  }
  const sorted = [...times].sort((a, b) => a - b);
  const median = sorted.length % 2 ? sorted[Math.floor(sorted.length / 2)] : (sorted[sorted.length / 2 - 1] + sorted[sorted.length / 2]) / 2;
  const deviations = times.map(value => Math.abs(value - median)).sort((a, b) => a - b);
  const item = { name, count, median_ms: median, min_ms: sorted[0], max_ms: sorted.at(-1), median_absolute_deviation_ms: deviations[Math.floor(deviations.length / 2)], samples_ms: times };
  observations.push(item);
  console.log(`${name}: median ${median.toFixed(1)} ms, range ${item.min_ms.toFixed(1)}..${item.max_ms.toFixed(1)} ms, n=${count}`);
}

/** Generate repeatable incompressible data without timing random generation or using outside files. */
function noise(bytes) {
  const output = Buffer.alloc(bytes); let state = 0x12345678;
  for (let index = 0; index < output.length; index += 4) {
    state ^= state << 13; state ^= state >>> 17; state ^= state << 5;
    output.writeUInt32LE(state >>> 0, index);
  }
  return output;
}

/** Build standard SKILL.md fixtures and remember independent expected file hashes. */
async function fixture(name, count, size, random = false) {
  const directory = path.join(workspace, 'fixtures', name);
  await mkdir(path.join(directory, 'references'), { recursive: true });
  const manifest = Buffer.from(`---\nname: ${name}\ndescription: Performance fixture\n---\n\n# ${name}\nRead references on demand.\n`);
  await writeFile(path.join(directory, 'SKILL.md'), manifest);
  const expected = { 'SKILL.md': digest(manifest) };
  let sourceBytes = manifest.length;
  for (let index = 0; index < count; index++) {
    const relative = `references/file-${String(index).padStart(4, '0')}.bin`;
    const bytes = random ? noise(size) : Buffer.alloc(size, 'Agent skill reference content.\n');
    await writeFile(path.join(directory, relative), bytes);
    expected[relative] = digest(bytes); sourceBytes += bytes.length;
  }
  return { name, directory, file_count: count + 1, source_bytes: sourceBytes, expected };
}

const executable = await readFile(cli);
const report = { phase, timestamp: new Date().toISOString(), cli: path.relative(root, cli), executable_sha256: digest(executable), executable_bytes: executable.length, environment: { platform: process.platform, release: os.release(), architecture: process.arch, cpu: os.cpus()[0]?.model, logical_cpus: os.cpus().length, ram_bytes: os.totalmem(), node: process.version }, method: { startup_samples: 15, operation_samples: samples, warmups: 1, clock: 'performance.now', subprocess_output: 'piped JSON, except --help', cache: 'warm OS cache; no eviction or cold-start claim', build: cli.includes(`${path.sep}debug${path.sep}`) ? 'debug' : 'caller-provided', tails: 'sample maximum/range only; no p95/p99 claim', setup_and_validation: 'excluded from timed process interval' }, fixtures: [], observations };
await measure('startup-help', 15, () => command(['--help']));
await measure('startup-list-empty', 15, () => command(['--json', '--home', path.join(workspace, 'empty-home'), 'list']), result => assert.deepEqual(JSON.parse(result.stdout), []));

for (const definition of [['typical', 24, 4096], ['many-files', 767, 2048], ['large-compressible', 1, 6 * 1024 * 1024], ['large-incompressible', 1, 6 * 1024 * 1024, true]]) {
  const item = await fixture(...definition);
  await mkdir(path.join(workspace, 'packed', item.name), { recursive: true });
  let archiveHash;
  await measure(`pack-${item.name}`, samples, index => command(['--json', 'pack', item.directory, '--output', path.join(workspace, 'packed', item.name, `${index}.skill`)]), result => {
    const value = JSON.parse(result.stdout);
    archiveHash ??= value.sha256;
    assert.equal(value.sha256, archiveHash, 'Unchanged sources must produce identical ZIP bytes');
  });
  const archive = path.join(workspace, 'packed', item.name, '0.skill');
  await measure(`import-${item.name}`, samples, index => command(['--json', '--home', path.join(workspace, 'imports', item.name, `${index}`), 'add', archive]), async (result, index) => {
    const value = JSON.parse(result.stdout);
    assert.equal(value.sha256, archiveHash);
    if (index !== 0) return;
    for (const [relative, expected] of Object.entries(item.expected)) {
      assert.equal(digest(await readFile(path.join(value.directory, relative))), expected, 'Extracted bytes must match source');
    }
  });
  const installedArchive = path.join(workspace, 'imports', item.name, '0', 'archives', 'local', `${item.name}.skill`);
  const beforeNoOp = await stat(installedArchive);
  await measure(`unchanged-import-${item.name}`, samples, () => command(['--json', '--home', path.join(workspace, 'imports', item.name, '0'), 'add', archive]), async result => {
    assert.equal(JSON.parse(result.stdout).sha256, archiveHash);
    assert.equal((await stat(installedArchive)).mtimeMs, beforeNoOp.mtimeMs, 'No-op imports must not rewrite the archive');
  });
  if (item.name === 'large-compressible') {
    await measure('directory-add-large-compressible', samples, index => command(['--json', '--home', path.join(workspace, 'directory-imports', `${index}`), 'add', item.directory]), result => assert.equal(JSON.parse(result.stdout).sha256, archiveHash));
  }
  report.fixtures.push({ name: item.name, file_count: item.file_count, source_bytes: item.source_bytes, archive_bytes: (await stat(archive)).size, archive_sha256: archiveHash });
}
await measure('startup-list-populated', 15, () => command(['--json', '--home', path.join(workspace, 'imports', 'typical', '0'), 'list']), result => assert.equal(JSON.parse(result.stdout).length, 1));

/** Optional six read-only staging requests, measured after local timings and with fully drained bodies. */
const remote = argument('--remote');
if (remote) {
  report.remote = { origin: remote, observations: [], warning: 'Network + TLS + edge + storage; three requests per endpoint do not estimate service tails.' };
  for (const endpoint of ['/health', '/v1/skills']) {
    const times = [];
    for (let index = 0; index < 3; index++) {
      const start = performance.now();
      const response = await fetch(new URL(endpoint, remote), { signal: AbortSignal.timeout(15000) });
      assert.equal(response.status, 200);
      await response.json(); times.push(performance.now() - start);
    }
    report.remote.observations.push({ endpoint, samples_ms: times, count: times.length });
  }
}
await writeFile(path.join(workspace, 'results.json'), `${JSON.stringify(report, null, 2)}\n`);
console.log(`Results: ${path.relative(root, path.join(workspace, 'results.json'))}`);

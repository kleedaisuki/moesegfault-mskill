/** Package a built CLI and license with an external SHA-256 digest for release downloads. */
import fs from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';

const [name, binary] = process.argv.slice(2);
if (!/^[a-z0-9_-]+$/.test(name ?? '') || !/^mskill(?:\.exe)?$/.test(binary ?? '')) {
  throw new Error('Usage: node scripts/package-release.mjs ARTIFACT mskill[.exe]');
}
const destination = path.resolve('.temp', 'release');
const staging = path.join(destination, name);
await fs.mkdir(staging, { recursive: true });
await fs.copyFile(path.join(process.env.CARGO_TARGET_DIR ?? '.cache/target', 'release', binary), path.join(staging, binary));
await fs.copyFile('LICENSE', path.join(staging, 'LICENSE'));
const archive = path.join(destination, `${name}.tar.gz`);
const result = spawnSync('tar', ['-czf', archive, '-C', staging, '.'], { stdio: 'inherit' });
if (result.error || result.status !== 0) throw result.error ?? new Error('tar failed');
const digest = createHash('sha256').update(await fs.readFile(archive)).digest('hex');
await fs.writeFile(`${archive}.sha256`, `${digest}  ${path.basename(archive)}\n`);
console.log(`Packaged ${path.basename(archive)} sha256=${digest}`);

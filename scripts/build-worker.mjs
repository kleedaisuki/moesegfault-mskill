/** Build the Rust Worker once, with a workspace-local target and bounded parallelism. */
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import fs from 'node:fs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
if (process.env.MSKILL_SKIP_BUILD === '1') {
  const artifact = path.join(root, 'crates', 'mskill-worker', 'build');
  if (!fs.existsSync(path.join(artifact, 'index.js')) || !fs.existsSync(path.join(artifact, 'index_bg.wasm'))) {
    throw new Error('Prebuilt Worker artifact is incomplete; refusing deployment.');
  }
  console.log('Using the tested prebuilt Rust Worker artifact.');
  process.exit(0);
}
const result = spawnSync('worker-build', ['--release'], {
  cwd: path.join(root, 'crates', 'mskill-worker'),
  env: {
    ...process.env,
    CARGO_TARGET_DIR: path.join(root, '.cache', 'target'),
    CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS ?? '2',
    // Preserve Wasm target_features for wasm-bindgen's externref/catch transform.
    CARGO_PROFILE_RELEASE_STRIP: 'debuginfo',
  },
  stdio: 'inherit',
});
if (result.error) {
  console.error('worker-build is required: cargo install worker-build --locked --version 0.8.7');
  console.error(result.error.message);
}
process.exit(result.status ?? 1);

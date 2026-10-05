/** Preserve upstream dependency license files in native release distributions. */
import fs from 'node:fs/promises';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

/** Generate notices from the locked, host-filtered CLI dependency graph.
 * @param {string} destination Output file selected by the release packager.
 * @returns {Promise<number>} Number of third-party packages recorded.
 */
export async function generateNotices(destination) {
  const host = execFileSync('rustc', ['-vV'], { encoding: 'utf8' }).match(/^host: (.+)$/m)?.[1];
  if (!host) throw new Error('Cannot identify native release target');
  const metadata = JSON.parse(execFileSync('cargo', ['metadata', '--locked', '--format-version', '1', '--filter-platform', host], { encoding: 'utf8', maxBuffer: 20 * 1024 * 1024 }));
  const root = metadata.packages.find(pkg => pkg.name === 'mskill-cli' && metadata.workspace_members.includes(pkg.id));
  if (!root || !metadata.resolve) throw new Error('CLI dependency graph is unavailable');
  const nodes = new Map(metadata.resolve.nodes.map(node => [node.id, node]));
  const reachable = new Set(), pending = [root.id];
  while (pending.length) {
    const id = pending.pop();
    if (reachable.has(id)) continue;
    reachable.add(id);
    pending.push(...(nodes.get(id)?.dependencies ?? []));
  }
  const packages = metadata.packages.filter(pkg => reachable.has(pkg.id) && pkg.source).sort((a, b) => `${a.name}@${a.version}`.localeCompare(`${b.name}@${b.version}`));
  const sections = ['Third-party notices\n\nUpstream license files for the locked native CLI dependency graph.\nThe mskill application license is distributed separately in LICENSE.\n'];
  for (const pkg of packages) {
    const directory = path.dirname(pkg.manifest_path);
    const entries = await fs.readdir(directory, { withFileTypes: true });
    const names = new Set(entries.filter(entry => entry.isFile() && /^(license(?:$|[-_.])|copying(?:$|[-_.])|notice(?:$|[-_.]))/i.test(entry.name)).map(entry => entry.name));
    if (pkg.license_file) names.add(pkg.license_file);
    sections.push(`\n${'='.repeat(72)}\n${pkg.name} ${pkg.version}\nLicense: ${pkg.license ?? 'See upstream license file'}\n${pkg.repository ? `Repository: ${pkg.repository}\n` : ''}`);
    for (const name of [...names].sort()) {
      const filename = path.resolve(directory, name);
      const relative = path.relative(directory, filename);
      if (relative.startsWith('..') || path.isAbsolute(relative)) throw new Error(`License path escapes package ${pkg.name}`);
      sections.push(`\n--- ${name} ---\n${await fs.readFile(filename, 'utf8')}\n`);
    }
    if (!names.size) sections.push(`\nLicense metadata retained; source: ${pkg.repository ?? `https://crates.io/crates/${pkg.name}/${pkg.version}`}\n`);
  }
  await fs.writeFile(destination, sections.join(''));
  return packages.length;
}

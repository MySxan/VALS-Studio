// First generate cargo metadata for the desktop manifest into .tools/desktop-metadata.json.
import { readFileSync, writeFileSync } from 'node:fs';
const root = new URL('../', import.meta.url);
const read = path => JSON.parse(readFileSync(new URL(path, root), 'utf8').replace(/^\uFEFF/, ''));
const metadata = read('.tools/desktop-metadata.json');
const lock = read('apps/desktop/package-lock.json');
const inventory = {
  note: 'Generated dependency inventory including build, test and target-specific packages. Preserve upstream license notices when distributing. Model weights: none.',
  rust: metadata.packages.filter(p => p.source).map(p => ({ name: p.name, version: p.version, license: p.license, repository: p.repository, source: `https://crates.io/crates/${p.name}/${p.version}` })).sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version)),
  npm: Object.entries(lock.packages).filter(([path]) => path).map(([path, p]) => ({ name: p.name ?? path.split('node_modules/').at(-1), version: p.version, license: p.license ?? null, source: p.resolved, dev: p.dev ?? false, optional: p.optional ?? false })),
};
writeFileSync(new URL('docs/desktop-dependencies.json', root), JSON.stringify(inventory, null, 2) + '\n');
console.log(`Recorded ${inventory.rust.length} Rust and ${inventory.npm.length} npm packages.`);

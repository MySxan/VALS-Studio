import { readFileSync, writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { isDeepStrictEqual } from 'node:util';

export function normalize(value) {
  const result = structuredClone(value);
  const ids = new Map();
  const roles = new Map();
  const identity = (id, role) => {
    if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id))
      throw new Error(`Invalid ${role} identity`);
    if (roles.has(id) && roles.get(id) !== role) throw new Error('Duplicate entity identity');
    roles.set(id, role);
    if (!ids.has(id)) ids.set(id, `00000000-0000-4000-8000-${String(ids.size + 1).padStart(12, '0')}`);
  };
  const project = result.workspace.project;
  identity(project.id, 'project');
  identity(result.workspace.job.id, 'job');
  for (const [index, track] of project.tracks.entries()) {
    identity(track.id, `track ${index}`);
    identity(track.sourceId, 'source');
    if (typeof track.sourcePath !== 'string' || !track.sourcePath) throw new Error('Invalid sourcePath');
    track.sourcePath = 'fixture://stereo-48000.wav';
    const provenance = track.analysis.provenance;
    if (!Number.isSafeInteger(provenance.createdAtUnixMs) || provenance.createdAtUnixMs < 0)
      throw new Error('Invalid fixture timestamp');
    if (!/^[a-z0-9_-]+\/[a-z0-9_-]+$/i.test(provenance.runtime)) throw new Error('Invalid runtime');
    provenance.createdAtUnixMs = 0;
    provenance.runtime = 'fixture/host';
  }
  const walk = (v) => {
    if (typeof v === 'string') return ids.get(v) ?? v;
    if (Array.isArray(v)) return v.map(walk);
    if (v && typeof v === 'object') return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, walk(x)]));
    return v;
  };
  return walk(result);
}

export function checkFixture(actual, expected) {
  if (!isDeepStrictEqual(normalize(actual), normalize(expected)))
    throw new Error('Rust DTO or analysis differs from the reviewed frontend fixture. Inspect the change before an explicit refresh.');
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const [mode, path] = process.argv.slice(2);
    if (!['--check', '--refresh'].includes(mode) || !path) throw new Error('Usage: fixture.mjs --check|--refresh <fixture.json> (Rust JSON on stdin)');
    const actual = JSON.parse(readFileSync(0, 'utf8').replace(/^\uFEFF/, ''));
    const expected = JSON.parse(readFileSync(path, 'utf8').replace(/^\uFEFF/, ''));
    if (mode === '--refresh') writeFileSync(path, JSON.stringify(normalize(actual), null, 2) + '\n');
    else checkFixture(actual, expected);
    console.log(`Fixture ${mode.slice(2)} passed`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}

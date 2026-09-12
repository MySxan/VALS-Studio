import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { checkFixture, normalize } from './fixture.mjs';
const fixture = JSON.parse(readFileSync(new URL('../apps/desktop/src/test-fixtures/session.json', import.meta.url), 'utf8').replace(/^\uFEFF/, ''));
test('volatile identity, path, timestamp and host normalize deterministically', () => {
  const normalized = normalize(fixture);
  checkFixture(normalized, fixture);
  assert.deepEqual(normalize(normalized), normalized);
});
test('broken identity references are not masked', () => {
  const changed = structuredClone(fixture);
  changed.waveform.trackId = 'ffffffff-ffff-4fff-8fff-ffffffffffff';
  assert.throws(() => checkFixture(changed, fixture));
});
test('analysis, confidence and DTO shape changes fail', () => {
  for (const mutate of [
    x => x.waveform.channels[0][0].max += 0.1,
    x => x.workspace.project.tracks[0].analysis.confidence.explanation = 'different',
    x => x.workspace.project.tracks[0].analysis.provenance.provider = 'different',
    x => x.workspace.project.tracks[0].analysis.provenance.dependencyHashes = [],
    x => x.workspace.project.extra = true,
  ]) {
    const changed = structuredClone(fixture);
    mutate(changed);
    assert.throws(() => checkFixture(changed, fixture));
  }
});
test('malformed volatile fields and duplicate identities fail', () => {
  for (const mutate of [
    x => x.workspace.project.tracks[0].sourcePath = null,
    x => x.workspace.project.tracks[0].analysis.provenance.createdAtUnixMs = 'bad',
    x => x.workspace.project.tracks[0].id = x.workspace.project.id,
  ]) {
    const changed = structuredClone(fixture);
    mutate(changed);
    assert.throws(() => normalize(changed));
  }
});

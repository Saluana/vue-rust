import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildArtifacts } from '../../tooling/manifest/build-manifest.mjs';
import { abs, PROFILE_REL } from '../../tooling/manifest/common.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const CLI = path.join(ROOT, 'tooling/manifest/build-manifest.mjs');
const EXPANSIONS = path.join(ROOT, PROFILE_REL, 'expansions.json');
const expansions = () => JSON.parse(readFileSync(EXPANSIONS, 'utf8'));

test('regenerating every artifact twice yields byte-identical output', async () => {
  const first = await buildArtifacts();
  const second = await buildArtifacts();
  assert.deepEqual([...second.keys()], [...first.keys()]);
  for (const [rel, content] of first) assert.equal(second.get(rel), content, rel);
});

test('the committed generated files are up to date with their sources', async () => {
  for (const [rel, content] of await buildArtifacts()) {
    assert.equal(readFileSync(abs(rel), 'utf8'), content, `stale: ${rel}`);
  }
});

test('build-manifest --check passes on the committed tree', () => {
  const result = spawnSync(process.execPath, [CLI, '--check'], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
});

test('manifest.json carries 126 rows, every expansion entry and all 171 vue exports', () => {
  const manifest = JSON.parse(readFileSync(abs(path.join(PROFILE_REL, 'manifest.json')), 'utf8'));
  assert.equal(manifest.rows.length, 126);
  assert.equal(manifest.entries.length, expansions().entries.length);
  assert.equal(manifest.vueExports.length, 171);
});

const NATIVE_SUPPORTED = ['main', 'section', 'div', 'header', 'footer', 'nav', 'article', 'aside', 'form', 'ul', 'ol', 'li', 'hr',
  'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'p', 'span', 'label', 'strong', 'em', 'b', 'i', 'small', 'code', 'pre', 'br', 'a',
  'button', 'input', 'textarea', 'select', 'option', 'img'];

// Maintainer decision (U31): the supported native tag set is exactly this list; excluded tags carry a use-instead note.
test('native tags: the supported set is the coordinator list, and every excluded tag says what to use instead', () => {
  const manifest = JSON.parse(readFileSync(abs(path.join(PROFILE_REL, 'manifest.json')), 'utf8'));
  const native = manifest.entries.filter((entry) => entry.kind === 'native-tag');
  assert.deepEqual(native.filter((entry) => entry.status === 'A').map((entry) => entry.name).sort(), [...NATIVE_SUPPORTED].sort());
  assert.equal(native.filter((entry) => entry.status === 'X').length, 15);
  for (const entry of native.filter((entry) => entry.status === 'X')) assert.match(entry.note, /instead/, entry.name);
});

test('negative control: an expansion status the row does not carry fails the build', async () => {
  const doc = expansions();
  doc.entries.push({ name: 'bogusSyntax', kind: 'syntax', row: 'L01', status: 'A' });
  await assert.rejects(buildArtifacts({ expansionsText: JSON.stringify(doc) }), /status A is not a status of L01/);
});

test('negative control: a vue expansion that is not an export fails the build', async () => {
  const doc = expansions();
  doc.entries.push({ name: 'notAVueExport', kind: 'vue', row: 'V01', status: 'P' });
  await assert.rejects(buildArtifacts({ expansionsText: JSON.stringify(doc) }), /"notAVueExport" is not an export of vue 3\.5\.43/);
});

test('negative control: a vue expansion that disagrees with the export classification fails the build', async () => {
  const doc = expansions();
  doc.entries.push({ name: 'hydrateOnIdle', kind: 'vue', row: 'V01', status: 'P' });
  await assert.rejects(buildArtifacts({ expansionsText: JSON.stringify(doc) }), /hydrateOnIdle.*disagrees with vue-exports\.json/);
});

test('negative control: an app method that a real app instance lacks fails the build', async () => {
  const doc = expansions();
  doc.entries.push({ name: 'app.bogusMethod', kind: 'app', row: 'A01', status: 'P' });
  await assert.rejects(buildArtifacts({ expansionsText: JSON.stringify(doc) }), /app\.bogusMethod" is not present on a real app instance/);
});

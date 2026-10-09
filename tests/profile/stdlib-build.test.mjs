import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { JSON_REL, MD_REL, SOURCE_REL, STDLIB_KINDS, STDLIB_TARGETS, buildStdlib } from '../../tooling/manifest/build-stdlib.mjs';
import { abs } from '../../tooling/manifest/common.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const CLI = path.join(ROOT, 'tooling/manifest/build-stdlib.mjs');
const sourceText = readFileSync(abs(SOURCE_REL), 'utf8');

// A copy of the hand-authored source with one change; the change is made on parsed JSON, so the rest is untouched.
const mutated = (change) => {
  const doc = JSON.parse(sourceText);
  change(doc.entries, doc);
  return JSON.stringify(doc);
};
const entry = (entries, p) => entries.find((e) => e.path === p);

test('buildStdlib is deterministic: two builds give identical text for both generated files', () => {
  const first = buildStdlib();
  const second = buildStdlib();
  assert.deepEqual([...second.keys()], [...first.keys()]);
  for (const [rel, text] of first) assert.equal(second.get(rel), text, rel);
});

test('the committed stdlib.json and stdlib.md match a fresh build', () => {
  for (const [rel, text] of buildStdlib()) assert.equal(readFileSync(abs(rel), 'utf8'), text, `stale: ${rel}`);
});

test('build-stdlib --check passes on the committed tree', () => {
  const result = spawnSync(process.execPath, [CLI, '--check'], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
});

test('the generated JSON has the documented shape, every kind and target, and entries with the documented keys', () => {
  const doc = JSON.parse(buildStdlib().get(JSON_REL));
  assert.deepEqual(Object.keys(doc), ['schemaVersion', 'default', 'kinds', 'targets', 'entries']);
  assert.equal(doc.default, 'deny');
  assert.deepEqual(doc.kinds, STDLIB_KINDS);
  assert.deepEqual(doc.targets, STDLIB_TARGETS);
  for (const e of doc.entries) {
    assert.deepEqual(Object.keys(e), ['kind', 'path', 'status', 'row', 'targets', 'code', 'note', 'hint'], e.path);
  }
});

test('the generated stdlib.md has the counts table, the decisions and one table per kind', () => {
  const md = buildStdlib().get(MD_REL);
  assert.match(md, /^## Counts$/m);
  assert.match(md, /^## Decisions$/m);
  assert.match(md, /^## static \(\d+\)$/m);
  assert.match(md, /globalThis is X \(VN4013, L20\)/);
});

test('negative control: an X entry without a hint fails the build', () => {
  const text = mutated((entries) => delete entry(entries, 'console').hint);
  assert.throws(() => buildStdlib({ sourceText: text }), /\(global:console\): X entries need a hint/);
});

test('negative control: a P entry that cites a row without P fails the build', () => {
  const text = mutated((entries) => { entry(entries, 'Object.getPrototypeOf').row = 'L05'; });
  assert.throws(() => buildStdlib({ sourceText: text }), /status P is not a status of L05/);
});

test('negative control: the same path under two kinds fails the build', () => {
  const text = mutated((entries) => entries.push({ kind: 'const', path: 'Math.PI', status: 'P', row: 'L08', targets: ['native', 'web', 'test'] }));
  assert.throws(() => buildStdlib({ sourceText: text }), /duplicate path Math\.PI/);
});

test('negative control: a VN code on a P entry fails the build', () => {
  const text = mutated((entries) => { entry(entries, 'Object.keys').code = 'VN4008'; });
  assert.throws(() => buildStdlib({ sourceText: text }), /only X or E entries carry a VN code/);
});

test('negative control: a path that does not fit its kind fails the build', () => {
  const text = mutated((entries) => { entry(entries, 'Math.PI').kind = 'symbol'; });
  assert.throws(() => buildStdlib({ sourceText: text }), /Math\.PI must be Symbol\.name/);
});

test('negative control: an entry that cites an unknown design row fails the build', () => {
  const text = mutated((entries) => { entry(entries, 'Object.keys').row = 'L99'; });
  assert.throws(() => buildStdlib({ sourceText: text }), /unknown design row "L99"/);
});

test('negative control: a W entry that also lists native fails the build (A14 is a W design row)', () => {
  const text = mutated((entries) => entries.push({ kind: 'ctor', path: 'SsrOnlyThing', status: 'W', row: 'A14', targets: ['native', 'web'], hint: 'x' }));
  assert.throws(() => buildStdlib({ sourceText: text }), /a W entry lists only the web target/);
});

test('negative control: an unknown key, kind or target fails the build', () => {
  assert.throws(() => buildStdlib({ sourceText: mutated((entries) => { entry(entries, 'Math').extra = 1; }) }), /unknown key "extra"/);
  assert.throws(() => buildStdlib({ sourceText: mutated((entries) => { entry(entries, 'Math').kind = 'method'; }) }), /unknown kind "method"/);
  assert.throws(() => buildStdlib({ sourceText: mutated((entries) => { entry(entries, 'Math').targets = ['node']; }) }), /unknown target "node"/);
});

test('negative control: a changed source produces different generated text, so --check would report it stale', () => {
  const text = mutated((entries) => { entry(entries, 'Object.keys').note = 'Changed for the negative control.'; });
  const built = buildStdlib({ sourceText: text }).get(JSON_REL);
  assert.notEqual(built, readFileSync(abs(JSON_REL), 'utf8'));
});

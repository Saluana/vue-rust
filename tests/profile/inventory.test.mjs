import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { extractInventory } from '../../tooling/manifest/extract-inventory.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const DESIGN = path.join(ROOT, 'planning/vue-native-framework/design.md');
const EXTRACTOR = path.join(ROOT, 'tooling/manifest/extract-inventory.mjs');
const designText = readFileSync(DESIGN, 'utf8');

// Hand-written expectations from design.md section 8 (not computed by the extractor).
const SPEC_COUNTS = { L: 21, S: 25, V: 29, U: 32, A: 19 };
const SPEC_IDS = Object.entries(SPEC_COUNTS).flatMap(([letter, count]) =>
  Array.from({ length: count }, (_, i) => letter + String(i + 1).padStart(2, '0')),
);

test('design.md section 8 has exactly the 126 expected row IDs', () => {
  const { rows, rowCount } = extractInventory(designText);
  assert.equal(SPEC_IDS.length, 126);
  assert.equal(rowCount, 126);
  assert.deepEqual(rows.map((row) => row.id), SPEC_IDS);
});

test('every row has a non-empty status set of valid letters with primary = first letter', () => {
  for (const row of extractInventory(designText).rows) {
    assert.ok(row.statuses.length >= 1, row.id);
    for (const letter of row.statuses) assert.match(letter, /^[PAWXE]$/, `${row.id} ${letter}`);
    assert.equal(row.primary, row.statuses[0], row.id);
  }
});

test('derived status sets match rows checked by hand against the design text', () => {
  const byId = new Map(extractInventory(designText).rows.map((row) => [row.id, row]));
  const expected = {
    L04: ['P'], L05: ['X'], L19: ['A'], S04: ['X', 'E'], S14: ['E'], S25: ['P', 'X'],
    U04: ['W'], U05: ['A', 'P'], U26: ['A', 'W'], U29: ['X', 'W'], U32: ['W', 'X'],
    V09: ['P'], V23: ['W'], V27: ['A', 'P'], A07: ['A', 'P'], A12: ['X', 'E'], A19: ['E', 'X'],
    // decisions 11 and 12: the render helpers h, cloneVNode and isVNode, and getCurrentInstance, are adapted (A)
    A11: ['X', 'A'], A18: ['X', 'A'],
    // decision 14: a render function returned from setup is supported; the Options fields in A10 and the render option stay X
    A10: ['P'],
  };
  for (const [id, statuses] of Object.entries(expected)) assert.deepEqual(byId.get(id).statuses, statuses, id);
  assert.equal(byId.get('L04').statusText, 'P subset');
  assert.equal(byId.get('U29').statusText, 'X shared/native, W explicit web-only profile');
  assert.equal(byId.get('L16').prototype, 'C only vue named imports');
  assert.equal(byId.get('A01').prototype, null);
});

test('committed profile/inventory.generated.json equals a fresh extraction', () => {
  const committed = JSON.parse(readFileSync(path.join(ROOT, 'compiler/profile/inventory.generated.json'), 'utf8'));
  assert.deepEqual(committed, extractInventory(designText, { source: 'planning/vue-native-framework/design.md' }));
});

test('negative control: the extractor rejects a design copy with one row removed', () => {
  const mutated = designText.split('\n').filter((line) => !line.startsWith('| L07 ')).join('\n');
  assert.notEqual(mutated, designText);
  assert.throws(() => extractInventory(mutated), /missing \[L07\]/);
});

test('negative control: the extractor rejects a changed table header', () => {
  const mutated = designText.replace('| ID / construct | Status; prototype |', '| ID / constructs | Status; prototype |');
  assert.notEqual(mutated, designText);
  assert.throws(() => extractInventory(mutated), /unexpected header for section 8\.1/);
});

test('negative control: the CLI fails on a temp copy with one row removed and writes no output', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'wp-g0b-inventory-'));
  try {
    const design = path.join(dir, 'design.md');
    const out = path.join(dir, 'inventory.generated.json');
    writeFileSync(design, designText.split('\n').filter((line) => !line.startsWith('| U17 ')).join('\n'));
    const result = spawnSync(process.execPath, [EXTRACTOR, '--design', design, '--out', out], { encoding: 'utf8' });
    assert.equal(result.status, 1);
    assert.match(result.stderr, /missing \[U17\]/);
    assert.equal(existsSync(out), false);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

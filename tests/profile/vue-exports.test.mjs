import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { lookup, isSupported } from '../../compiler/profile/index.mjs';
import { auditVueExports, parseExportMap } from '../../tooling/manifest/audit-vue-exports.mjs';
import { extractInventory } from '../../tooling/manifest/extract-inventory.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const requireFromRoot = createRequire(path.join(ROOT, 'package.json'));
const INTEROP = ['__esModule', 'module.exports'];
const designText = readFileSync(path.join(ROOT, 'planning/vue-native-framework/design.md'), 'utf8');
const rows = extractInventory(designText).rows;
const exportMapText = readFileSync(path.join(ROOT, 'compiler/profile/export-map.json'), 'utf8');
const vueExports = JSON.parse(readFileSync(path.join(ROOT, 'compiler/profile/vue-exports.json'), 'utf8'));

test('the only names in the vue import namespace that require("vue") lacks are the two interop keys', async () => {
  const namespaceKeys = Object.keys(await import('vue'));
  const cjsKeys = new Set(Object.keys(requireFromRoot('vue')));
  assert.deepEqual(namespaceKeys.filter((key) => !cjsKeys.has(key)).sort(), [...INTEROP].sort());
});

test('the production CJS build exports the same names as the development build', () => {
  const prod = JSON.parse(execFileSync(process.execPath, ['-p', 'JSON.stringify(Object.keys(require("vue")))'], {
    cwd: ROOT,
    env: { ...process.env, NODE_ENV: 'production' },
    encoding: 'utf8',
  }));
  assert.deepEqual([...prod].sort(), Object.keys(requireFromRoot('vue')).sort());
});

test('every runtime export of vue 3.5.43 is classified into exactly one design row', async () => {
  const runtime = Object.keys(await import('vue')).filter((key) => !INTEROP.includes(key));
  const byName = new Map(vueExports.exports.map((entry) => [entry.name, entry]));
  const rowIds = new Set(rows.map((row) => row.id));
  for (const name of runtime) {
    const entry = byName.get(name);
    assert.ok(entry, `unclassified export: ${name}`);
    assert.ok(rowIds.has(entry.row), `${name} maps to unknown row ${entry.row}`);
    assert.match(entry.status, /^[PAWXE]$/, name);
    assert.ok(entry.sources.includes('runtime'), name);
    assert.notEqual(entry.note, 'unclassified: default-deny', name);
  }
  assert.equal(vueExports.counts.runtimeExports, runtime.length);
  assert.equal(vueExports.counts.unclassified, 0);
});

test('lookup("vue", "definitelyNotAnExport") is null and is not supported on either target', () => {
  assert.equal(lookup('vue', 'definitelyNotAnExport'), null);
  assert.equal(isSupported('vue', 'definitelyNotAnExport', 'native'), false);
  assert.equal(isSupported('vue', 'definitelyNotAnExport', 'web'), false);
});

test('isShallow is exported by the pinned package and classified P under V09', async () => {
  assert.equal(typeof (await import('vue')).isShallow, 'function');
  assert.deepEqual(lookup('vue', 'isShallow'), { status: 'P', row: 'V09', note: null, compilerInternal: false });
});

test('the compile-time macros declared in vue type declarations are the seven SFC macros', () => {
  const macros = vueExports.exports.filter((entry) => entry.sources.includes('types')).map((entry) => entry.name).sort();
  assert.deepEqual(macros, ['defineEmits', 'defineExpose', 'defineModel', 'defineOptions', 'defineProps', 'defineSlots', 'withDefaults']);
});

test('no documented design name is missing from vue 3.5.43 (no docs drift found)', () => {
  assert.deepEqual(vueExports.documentedNames.filter((item) => item.drift).map((item) => item.name), []);
});

test('negative control: an export the map omits is reported unclassified, not accepted', async () => {
  const map = parseExportMap(exportMapText);
  const { ref, ...withoutRef } = map.exports;
  assert.equal(ref, 'V01');
  const audit = await auditVueExports({ rows, exportMap: { ...map, exports: withoutRef } });
  assert.deepEqual(audit.unclassified, ['ref']);
  const entry = audit.exports.find((item) => item.name === 'ref');
  assert.deepEqual([entry.row, entry.status, entry.note], [null, 'X', 'unclassified: default-deny']);
});

test('negative control: a map entry that is not a real export fails the audit', async () => {
  const map = parseExportMap(exportMapText);
  const mutated = { ...map, exports: { ...map.exports, definitelyNotAnExport: 'V01' } };
  await assert.rejects(auditVueExports({ rows, exportMap: mutated }), /definitelyNotAnExport/);
});

test('negative control: an explicit status the row does not carry fails the audit', async () => {
  const map = parseExportMap(exportMapText);
  const mutated = { ...map, exports: { ...map.exports, ref: { row: 'V01', status: 'A' } } };
  await assert.rejects(auditVueExports({ rows, exportMap: mutated }), /status A is not a status of V01/);
});

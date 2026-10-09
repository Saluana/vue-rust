// Unit tests for the project compiler (WP-C1c): module ids, resolution, native modules, the vue import policy,
// closed-world rejections with negative controls, type-only erasure, .vue modules, determinism and the module table.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { compileProject, toModuleId } from '../../../compiler/project/index.mjs';
import { modulesTableRs, moduleFileName } from '../../../compiler/project/table.mjs';
import { isNativeSpecifier, NATIVE_MODULES } from '../../../compiler/project/natives.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '../../..');

const codes = (r) => r.diagnostics.filter((d) => d.severity === 'error').map((d) => d.code);

test('module ids are project-root-relative and /-prefixed; escapes are refused', () => {
  assert.equal(toModuleId('src/a.js'), '/src/a.js');
  assert.equal(toModuleId('/src//./a.js'), '/src/a.js');
  assert.equal(toModuleId('src/x/../a.js'), '/src/a.js');
  assert.equal(toModuleId('../a.js'), null);
  assert.equal(moduleFileName(3), 'm3');
  assert.deepEqual(NATIVE_MODULES, ['vue', '@vue-native/platform']);
  assert.equal(isNativeSpecifier('vue'), true);
  assert.equal(isNativeSpecifier('lodash'), false);
});

test('a two-module graph with a cycle compiles; requests use canonical ids; natives are not compiled', () => {
  const files = {
    'src/main.js': "import { ref } from 'vue'\nimport { add } from './lib.js'\nexport const r = ref(add(1, 2))\n",
    'src/lib.js': "import { r } from './main.js'\nexport function add(a, b) { return a + b }\nexport const seen = r\n",
  };
  const res = compileProject({ entry: 'src/main.js', files });
  assert.equal(res.ok, true, JSON.stringify(res.diagnostics.map((d) => d.message)));
  assert.equal(res.entryId, '/src/main.js');
  assert.deepEqual(res.natives, ['vue']);
  assert.deepEqual(res.modules.map((m) => m.id), ['/src/lib.js', '/src/main.js']);
  const main = res.modules.find((m) => m.id === '/src/main.js');
  assert.deepEqual(main.requests, ['vue', '/src/lib.js']);
  assert.match(main.rust, /pub static MODULE: ModuleDef = ModuleDef \{ specifier: "\/src\/main\.js"/);
  assert.match(main.rust, /requests: &\["vue", "\/src\/lib\.js"\]/);
});

test('output is deterministic across runs', () => {
  const files = { 'a.js': "import { b } from './b.js'\nexport const a = b + 1\n", 'b.js': 'export const b = 1\n' };
  const one = compileProject({ entry: 'a.js', files });
  const two = compileProject({ entry: 'a.js', files: { 'b.js': files['b.js'], 'a.js': files['a.js'] } });
  assert.equal(JSON.stringify(one.modules), JSON.stringify(two.modules));
});

test('the vue import policy: default and namespace imports, subpaths, excluded exports', () => {
  const ns = compileProject({ entry: 'a.js', files: { 'a.js': "import * as v from 'vue'\nexport const x = v\n" } });
  assert.deepEqual(codes(ns), ['VN6202']);
  const def = compileProject({ entry: 'a.js', files: { 'a.js': "import v from 'vue'\nexport const x = v\n" } });
  assert.deepEqual(codes(def), ['VN6202']);
  const sub = compileProject({ entry: 'a.js', files: { 'a.js': "import { h } from 'vue/server-renderer'\nexport const x = h\n" } });
  assert.deepEqual(codes(sub), ['VN6203']);
  const excluded = compileProject({ entry: 'a.js', files: { 'a.js': "import { notAnExport } from 'vue'\nexport const x = notAnExport\n" } });
  assert.deepEqual(codes(excluded), ['VN6201']);
});

test('closed world: packages, Node built-ins, missing modules, absolute and unknown file types are rejected', () => {
  const pkg = compileProject({ entry: 'a.js', files: { 'a.js': "import _ from 'lodash'\nexport const x = _\n" } });
  assert.deepEqual(codes(pkg), ['VN9001']);
  assert.match(pkg.diagnostics[0].message, /package import 'lodash'/);
  const builtin = compileProject({ entry: 'a.js', files: { 'a.js': "import fs from 'node:fs'\nexport const x = fs\n" } });
  assert.deepEqual(codes(builtin), ['VN6204']);
  const missing = compileProject({ entry: 'a.js', files: { 'a.js': "import { q } from './nope.js'\nexport const x = q\n" } });
  assert.deepEqual(codes(missing), ['VN9001']);
  const abs = compileProject({ entry: 'a.js', files: { 'a.js': "import { q } from '/b.js'\nexport const x = q\n", 'b.js': 'export const q = 1\n' } });
  assert.deepEqual(codes(abs), ['VN9001']);
  const kind = compileProject({ entry: 'a.js', files: { 'a.js': "import { q } from './b.css'\nexport const x = q\n", 'b.css': 'x{}' } });
  assert.deepEqual(codes(kind), ['VN9001']);
  const noEntry = compileProject({ entry: 'missing.js', files: { 'a.js': 'export const x = 1\n' } });
  assert.deepEqual(codes(noEntry), ['VN9001']);
});

test('negative control: the same graph with a valid import compiles, with a broken one it does not', () => {
  const good = compileProject({ entry: 'a.js', files: { 'a.js': "import { q } from './b.js'\nexport const x = q\n", 'b.js': 'export const q = 1\n' } });
  assert.equal(good.ok, true);
  const bad = compileProject({ entry: 'a.js', files: { 'a.js': "import { q } from './c.js'\nexport const x = q\n", 'b.js': 'export const q = 1\n' } });
  assert.equal(bad.ok, false);
  assert.equal(bad.modules.find((m) => m.id === '/a.js').rust, null);
});

test('type-only imports and exports are erased, so their targets need not exist (TypeScript)', () => {
  const src = [
    "import type { T } from './missing-types.ts'",
    "import { type U } from './missing-types.ts'",
    "export type { V } from './missing-types.ts'",
    "export { type W } from './missing-types.ts'",
    'export const x = 1',
    '',
  ].join('\n');
  const res = compileProject({ entry: 'a.ts', files: { 'a.ts': src } });
  assert.equal(res.ok, true, JSON.stringify(res.diagnostics.map((d) => d.message)));
  assert.deepEqual(res.modules[0].requests, []);
});

test('a .vue module compiles through compileSfc and its generated code is lowered like a module', () => {
  const vue = readFileSync(path.join(repo, 'tests/sfc/fixtures/ok-counter.vue'), 'utf8');
  const res = compileProject({ entry: 'App.vue', files: { 'App.vue': vue } });
  assert.equal(res.ok, true, JSON.stringify(res.diagnostics.map((d) => `${d.code} ${d.message}`)));
  const app = res.modules[0];
  assert.equal(app.kind, 'vue');
  assert.match(app.rust, /pub static MODULE: ModuleDef/);
  assert.ok(app.requests.includes('vue'));
  assert.deepEqual(res.natives, ['vue']);
});

test('the module table lists the compiled modules in order', () => {
  const res = compileProject({ entry: 'a.js', files: { 'a.js': "import { q } from './b.js'\nexport const x = q\n", 'b.js': 'export const q = 1\n' } });
  const table = modulesTableRs(res.modules.filter((m) => m.rust !== null), res.entryId);
  assert.match(table, /use super::\{m0, m1\};/);
  assert.match(table, /static DEFS: &\[&ModuleDef\] = &\[&m0::MODULE, &m1::MODULE\];/);
  assert.match(table, /"\/a\.js"/);
});

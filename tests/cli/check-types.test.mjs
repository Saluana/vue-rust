// vue-native check with the type checker (task 5.7): a type error is a VN7001 at its .vue span and fails the check,
// --no-types skips the checker, TypeScript-only constructs stay rejected, imported types are checked, and JavaScript is
// not. Expected lines and columns are counted by hand from the sources written below.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { EXAMPLES, ROOT, capture, scratch } from './helpers.mjs';

// A project with the given files (root-relative names), in a fresh scratch directory.
function project(name, files) {
  const dir = scratch(name);
  for (const [file, text] of Object.entries(files)) {
    fs.mkdirSync(path.dirname(path.join(dir, file)), { recursive: true });
    fs.writeFileSync(path.join(dir, file), text);
  }
  return dir;
}

// Line 5 is `count.value = 'three'`: a ref of a number assigned a string.
const TYPE_ERROR_APP = "<script setup lang=\"ts\">\nimport { ref } from 'vue'\n\nconst count = ref(0)\ncount.value = 'three'\n</script>\n\n<template>\n  <p>{{ count }}</p>\n</template>\n";
const FIXED_APP = TYPE_ERROR_APP.replace("count.value = 'three'", 'count.value = 3');

test('check: a type error is a VN7001 at its .vue line and column, and fails the check (exit 1)', () => {
  const dir = project('check-type-error', { 'src/App.vue': TYPE_ERROR_APP });
  const r = capture(['check', dir]);
  assert.equal(r.code, 1, r.stdout);
  assert.match(r.stderr, /^src\/App\.vue:5:0: error VN7001: TS2322: Type 'string' is not assignable to type 'number'\.$/m);
  assert.equal(r.stdout, 'failed: 1 module(s), 1 error(s), 0 warning(s) (/src/App.vue, target web)\n');
});

test('check --no-types: the same project passes because the checker is skipped (negative control); the report says so', () => {
  const dir = project('check-no-types', { 'src/App.vue': TYPE_ERROR_APP });
  const r = capture(['check', dir, '--no-types']);
  assert.equal(r.code, 0, r.stderr);
  assert.equal(r.stdout, 'ok: 1 module(s), 0 error(s), 0 warning(s) (/src/App.vue, target web)\n');
  const report = JSON.parse(capture(['check', dir, '--no-types', '--json']).stdout);
  assert.deepEqual(report.typeCheck, { ran: false, typescript: null, vueTsc: null });
});

test('check --json: the report carries the VN7001 diagnostic with its file and span, and the checker versions', () => {
  const dir = project('check-type-error-json', { 'src/App.vue': TYPE_ERROR_APP });
  const r = capture(['check', dir, '--json']);
  assert.equal(r.code, 1);
  const report = JSON.parse(r.stdout);
  assert.equal(report.ok, false);
  assert.deepEqual(report.typeCheck, { ran: true, typescript: '6.0.3', vueTsc: '3.3.12' });
  assert.equal(report.diagnostics.length, 1);
  const [diag] = report.diagnostics;
  assert.equal(diag.code, 'VN7001');
  assert.equal(diag.severity, 'error');
  assert.equal(diag.file, 'src/App.vue');
  assert.equal(diag.start.line, 5);
  assert.equal(diag.start.column, 0);
  assert.equal(diag.end.line, 5);
  assert.equal(diag.end.column, 11);
  assert.match(diag.message, /^TS2322: /);
});

test('check: a corrected component passes with the checker on (exit 0), and the checker ran on it', () => {
  const dir = project('check-fixed', { 'src/App.vue': FIXED_APP });
  const r = capture(['check', dir, '--json']);
  assert.equal(r.code, 0, r.stderr);
  const report = JSON.parse(r.stdout);
  assert.deepEqual(report.diagnostics, []);
  assert.deepEqual(report.typeCheck, { ran: true, typescript: '6.0.3', vueTsc: '3.3.12' });
});

test('check --json: the Counter example is clean with the checker on, and the report records the pinned pair', () => {
  const r = capture(['check', path.join(EXAMPLES, 'counter'), '--json']);
  assert.equal(r.code, 0, r.stderr);
  const report = JSON.parse(r.stdout);
  assert.deepEqual(report.diagnostics, []);
  assert.deepEqual(report.typeCheck, { ran: true, typescript: '6.0.3', vueTsc: '3.3.12' });
});

test('check: TypeScript-only constructs stay rejected with the checker on (enum VN3001, namespace VN3002, using VN3008)', () => {
  const sfcFixtures = path.join(ROOT, 'tests', 'sfc', 'fixtures');
  for (const [fixture, code] of [['bad-ts-enum.vue', 'VN3001'], ['bad-ts-namespace.vue', 'VN3002'], ['bad-ts-using.vue', 'VN3008']]) {
    const dir = project(`check-${code}`, { 'src/App.vue': fs.readFileSync(path.join(sfcFixtures, fixture), 'utf8') });
    const r = capture(['check', dir, '--json']);
    assert.equal(r.code, 1, fixture);
    const report = JSON.parse(r.stdout);
    assert.ok(report.diagnostics.some((d) => d.code === code), `${fixture}: ${code}`);
    assert.ok(!report.diagnostics.some((d) => d.code === 'VN7001'), `${fixture}: no type error is added`);
    // The checker runs only on a project that compiled, so a rejected component is not type checked.
    assert.equal(report.typeCheck.ran, false, fixture);
  }
});

test('check: imported types are checked; a wrong use is VN7001 at the .vue span, a corrected use passes, a missing type file is VN6017', () => {
  const TYPES = 'export interface Props {\n  label: string\n  count: number\n}\n';
  // Line 5 declares `label` with the type given; the imported Props say label is a string.
  const app = (declared, source = './types.ts') => `<script setup lang="ts">\nimport type { Props } from '${source}'\n\nconst props: Props = { label: 'a', count: 2 }\nconst label: ${declared} = props.label\n</script>\n\n<template>\n  <p>{{ label }}</p>\n</template>\n`;

  const wrong = capture(['check', project('check-imported-wrong', { 'src/types.ts': TYPES, 'src/App.vue': app('number') }), '--json']);
  assert.equal(wrong.code, 1, wrong.stderr);
  const wrongReport = JSON.parse(wrong.stdout);
  assert.deepEqual(wrongReport.diagnostics.map((d) => [d.code, d.file, d.start.line, d.start.column]), [['VN7001', 'src/App.vue', 5, 6]]);
  assert.match(wrongReport.diagnostics[0].message, /^TS2322: Type 'string' is not assignable to type 'number'\./);

  const fixed = capture(['check', project('check-imported-fixed', { 'src/types.ts': TYPES, 'src/App.vue': app('string') }), '--json']);
  assert.equal(fixed.code, 0, fixed.stderr);
  assert.deepEqual(JSON.parse(fixed.stdout).typeCheck, { ran: true, typescript: '6.0.3', vueTsc: '3.3.12' });

  // Line 2 is `import type { Props } from './missing.ts'`; the module name starts at column 27 and ends at column 41.
  const missing = capture(['check', project('check-imported-missing', { 'src/App.vue': app('string', './missing.ts') }), '--json']);
  assert.equal(missing.code, 1, missing.stderr);
  const missingReport = JSON.parse(missing.stdout);
  assert.deepEqual(
    missingReport.diagnostics.map((d) => [d.code, d.file, d.start.line, d.start.column, d.end.column]),
    [['VN6017', 'src/App.vue', 2, 27, 41]],
  );
  assert.equal(missingReport.typeCheck.ran, false);
});

test('check: a JavaScript component and module are not type checked; the checker runs and passes', () => {
  const dir = project('check-js-only', {
    'src/App.vue': "<script setup>\nimport { ref } from 'vue'\nimport { n } from './util.js'\nconst count = ref(0)\ncount.value = 'three'\n</script>\n<template><p>{{ count }} {{ n }}</p></template>\n",
    'src/util.js': "export const total = 'a'\nexport const n = total.toFixed(2)\n",
  });
  const r = capture(['check', dir, '--json']);
  assert.equal(r.code, 0, r.stderr);
  const report = JSON.parse(r.stdout);
  assert.deepEqual(report.diagnostics, []);
  assert.deepEqual(report.typeCheck, { ran: true, typescript: '6.0.3', vueTsc: '3.3.12' });
});

test('help: --no-types is listed with the check command', () => {
  const r = capture(['--help']);
  assert.equal(r.code, 0);
  assert.match(r.stdout, /check <entry\.vue\|dir>.*--no-types/);
});

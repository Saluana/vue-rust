// vue-native check (WP-APP1): exit statuses, diagnostics, the --json report and usage errors.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { EXAMPLES, FIXTURES, capture, scratch } from './helpers.mjs';

test('check: the Counter example compiles with no diagnostics (exit 0)', () => {
  const r = capture(['check', path.join(EXAMPLES, 'counter')]);
  assert.equal(r.code, 0, r.stderr);
  assert.equal(r.stdout, 'ok: 1 module(s), 0 error(s), 0 warning(s) (/src/App.vue, target web)\n');
});

test('check: Quick Quote compiles all four modules of its graph (exit 0)', () => {
  const r = capture(['check', path.join(EXAMPLES, 'quick-quote')]);
  assert.equal(r.code, 0, r.stderr);
  assert.match(r.stdout, /^ok: 4 module\(s\), 0 error\(s\)/);
});

test('check --json: the clean Quick Quote report lists its modules and the vue native', () => {
  const r = capture(['check', path.join(EXAMPLES, 'quick-quote'), '--json']);
  assert.equal(r.code, 0, r.stderr);
  const report = JSON.parse(r.stdout);
  assert.equal(report.ok, true);
  assert.equal(report.target, 'web');
  assert.deepEqual(report.modules.map((m) => m.id), ['/src/App.vue', '/src/FieldInput.vue', '/src/LineItem.vue', '/src/money.ts']);
  assert.ok(report.modules.every((m) => m.compiled === true));
  assert.deepEqual(report.natives, ['vue']);
  assert.deepEqual(report.diagnostics, []);
});

test('check: an unsupported package import fails with a coded, located diagnostic (exit 1)', () => {
  const r = capture(['check', path.join(FIXTURES, 'bad')]);
  assert.equal(r.code, 1);
  assert.match(r.stderr, /^src\/App\.vue:\d+:\d+: error VN9001: package import 'lodash'/m);
  assert.match(r.stdout, /^failed: 1 module\(s\), 1 error\(s\), 0 warning\(s\)/);
});

test('check --json: the failing report carries the diagnostic code, file and span', () => {
  const r = capture(['check', path.join(FIXTURES, 'bad'), '--json']);
  assert.equal(r.code, 1);
  const report = JSON.parse(r.stdout);
  assert.equal(report.ok, false);
  const diag = report.diagnostics.find((d) => d.code === 'VN9001');
  assert.ok(diag, 'a VN9001 diagnostic is reported');
  assert.equal(diag.severity, 'error');
  assert.equal(diag.file, 'src/App.vue');
  assert.match(diag.message, /'lodash'/);
  assert.ok(diag.start.line >= 1 && diag.start.column >= 0 && diag.end.offset >= diag.start.offset);
});

test('check: negative control, a Counter copy with an injected package import is rejected (exit 1)', () => {
  const dir = scratch('counter-mutated');
  fs.cpSync(path.join(EXAMPLES, 'counter'), dir, { recursive: true });
  const file = path.join(dir, 'src', 'App.vue');
  const original = fs.readFileSync(file, 'utf8');
  const mutated = original.replace("import { ref, computed } from 'vue'", "import { ref, computed } from 'vue'\nimport lodash from 'lodash'");
  assert.notEqual(mutated, original, 'the mutation applied');
  fs.writeFileSync(file, mutated);
  const r = capture(['check', dir]);
  assert.equal(r.code, 1);
  assert.match(r.stderr, /'lodash'/);
});

test('check: usage mistakes exit 2 with a message', () => {
  assert.equal(capture([]).code, 2);
  assert.equal(capture(['--no-such-option']).code, 2);
  assert.equal(capture(['check']).code, 2);
  const missing = capture(['check', path.join(FIXTURES, 'does-not-exist')]);
  assert.equal(missing.code, 2);
  assert.match(missing.stderr, /no such file or directory/);
  assert.equal(capture(['check', path.join(EXAMPLES, 'counter'), '--target', 'bogus']).code, 2);
  // A directory without src/App.vue is not a project.
  assert.equal(capture(['check', FIXTURES]).code, 2);
  assert.equal(capture(['bogus-command']).code, 2);
});

test('check: the native target compiles the Counter too', () => {
  const r = capture(['check', path.join(EXAMPLES, 'counter'), '--target', 'native']);
  assert.equal(r.code, 0, r.stderr);
  assert.match(r.stdout, /target native\)$/m);
});

test('help: prints the usage and exits 0', () => {
  const r = capture(['--help']);
  assert.equal(r.code, 0);
  assert.match(r.stdout, /vue-native <command>/);
  for (const command of ['check', 'build', 'create', 'doctor']) assert.match(r.stdout, new RegExp(`\\b${command}\\b`));
});

test('check: a .vue file names the entry, its directory is the root unless --root says otherwise', () => {
  const file = path.join(EXAMPLES, 'quick-quote', 'src', 'App.vue');
  const asFile = capture(['check', file, '--json']);
  assert.equal(asFile.code, 0, asFile.stderr);
  assert.equal(JSON.parse(asFile.stdout).entry, '/App.vue');
  const withRoot = capture(['check', file, '--root', path.join(EXAMPLES, 'quick-quote'), '--json']);
  assert.equal(withRoot.code, 0, withRoot.stderr);
  const report = JSON.parse(withRoot.stdout);
  assert.equal(report.entry, '/src/App.vue');
  assert.equal(report.modules.length, 4);
  const outside = capture(['check', file, '--root', FIXTURES]);
  assert.equal(outside.code, 2);
  assert.match(outside.stderr, /outside the project root/);
});

test('check: unsupported globals are rejected in scripts, templates and imported modules', () => {
  for (const [app, extra] of [
    ['<script setup>const value = structuredClone({a: 1})</script><template>{{ value.a }}</template>', null],
    ['<template>{{ structuredClone({a: 1}).a }}</template>', null],
    ['<script setup>import value from "./value.js"</script><template>{{ value }}</template>', 'export default structuredClone({a: 1})'],
  ]) {
    const root = scratch('check-unsupported-global');
    fs.mkdirSync(path.join(root, 'src'));
    fs.writeFileSync(path.join(root, 'src/App.vue'), app);
    if (extra) fs.writeFileSync(path.join(root, 'src/value.js'), extra);
    const r = capture(['check', root, '--target', 'native', '--json']);
    assert.equal(r.code, 1, r.stdout);
    const report = JSON.parse(r.stdout);
    assert.equal(report.ok, false);
    assert.ok(report.diagnostics.some(d => d.code === 'VN4016' && d.message.includes('structuredClone')));
  }
});

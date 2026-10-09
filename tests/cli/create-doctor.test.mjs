// vue-native create and doctor (WP-APP1).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { EXAMPLES, ROOT, TEMPLATES, capture, scratch } from './helpers.mjs';

function readAll(dir) {
  return fs.readdirSync(dir, { recursive: true }).filter((f) => fs.statSync(path.join(dir, f)).isFile()).sort()
    .map((f) => [f, fs.readFileSync(path.join(dir, f), 'utf8')]);
}

for (const template of ['counter', 'quick-quote']) {
  test(`create: the ${template} template matches its example and compiles as a new project`, () => {
    assert.deepEqual(readAll(path.join(TEMPLATES, template)), readAll(path.join(EXAMPLES, template)), 'template and example match');
    const dir = scratch(`create-${template}`);
    const created = capture(['create', dir, '--template', template]);
    assert.equal(created.code, 0, created.stderr);
    assert.deepEqual(readAll(dir), readAll(path.join(TEMPLATES, template)));
    const checked = capture(['check', dir]);
    assert.equal(checked.code, 0, checked.stderr);
  });
}

test('create: the default template is counter, and a non-empty directory is refused (exit 2)', () => {
  const dir = scratch('create-default');
  assert.equal(capture(['create', dir]).code, 0);
  assert.ok(fs.existsSync(path.join(dir, 'src', 'App.vue')));
  const again = capture(['create', dir]);
  assert.equal(again.code, 2);
  assert.match(again.stderr, /exists and is not empty/);
});

test('create: an unknown template is a usage error (exit 2)', () => {
  const dir = scratch('create-unknown');
  const r = capture(['create', dir, '--template', 'nope']);
  assert.equal(r.code, 2);
  assert.equal(fs.existsSync(dir), true);
  assert.equal(fs.readdirSync(dir).length, 0, 'nothing was written');
});

test('doctor: reports the required toolchain pieces and exits 0 in this checkout', () => {
  const r = capture(['doctor']);
  assert.equal(r.code, 0, r.stdout);
  assert.match(r.stdout, /\[ok\]\s+node >= 20/);
  assert.match(r.stdout, /\[ok\]\s+framework checkout/);
  assert.match(r.stdout, /\[ok\]\s+cargo/);
  assert.match(r.stdout, /\[ok\]\s+rustc 1\.99\.0/);
  assert.match(r.stdout, /\[ok\]\s+wasm32-unknown-unknown target/);
  assert.match(r.stdout, /the web build toolchain is ready/);
});

test('doctor: negative control, a checkout without the framework crates is reported missing (exit 1)', () => {
  const env = { ...process.env, VN_FRAMEWORK_ROOT: scratch('not-a-checkout') };
  const r = spawnSync(process.execPath, [path.join(ROOT, 'tooling', 'cli', 'vue-native.mjs'), 'doctor'], { env, encoding: 'utf8' });
  assert.equal(r.status, 1, r.stdout);
  assert.match(r.stdout, /\[missing\]\s+framework checkout/);
  assert.match(r.stdout, /nothing was installed/);
});

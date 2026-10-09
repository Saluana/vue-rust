// vue-native build (WP-APP1): the refusals and failures that happen before any cargo build.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { EXAMPLES, FIXTURES, capture, freshPath, scratch } from './helpers.mjs';

test('build: refuses a directory vue-native did not write, before compiling (exit 1)', () => {
  const out = scratch('build-foreign');
  fs.writeFileSync(path.join(out, 'notes.txt'), 'not ours');
  const r = capture(['build', path.join(EXAMPLES, 'counter'), '--target', 'web', '--out', out]);
  assert.equal(r.code, 1);
  assert.match(r.stderr, /refusing to write into .*not written by vue-native build/);
  assert.equal(fs.readFileSync(path.join(out, 'notes.txt'), 'utf8'), 'not ours', 'the foreign file is untouched');
});

test('build: a project with errors builds nothing and prints the diagnostics (exit 1)', () => {
  const out = freshPath('build-errors');
  const r = capture(['build', path.join(FIXTURES, 'bad'), '--target', 'web', '--out', out]);
  assert.equal(r.code, 1);
  assert.match(r.stderr, /VN9001/);
  assert.match(r.stderr, /nothing was built/);
  assert.equal(fs.existsSync(out), false, 'no output directory is created');
});

test('build: usage mistakes exit 2', () => {
  assert.equal(capture(['build']).code, 2);
  assert.equal(capture(['build', path.join(EXAMPLES, 'counter'), '--target', 'web']).code, 2, 'missing --out');
  assert.equal(capture(['build', path.join(EXAMPLES, 'counter'), '--out', scratch('build-no-target')]).code, 2, 'missing --target');
  assert.equal(capture(['build', path.join(EXAMPLES, 'counter'), '--target', 'android', '--out', scratch('build-bad-target')]).code, 2);
  assert.equal(capture(['build', path.join(EXAMPLES, 'counter'), 'extra', '--target', 'web', '--out', scratch('build-extra')]).code, 2);
});

test('build --target web: a .vue project produces an app, styles and module metadata (exit 0)', () => {
  const out = freshPath('build-web-counter');
  const r = capture(['build', path.join(EXAMPLES, 'counter'), '--target', 'web', '--out', out]);
  assert.equal(r.code, 0, r.stderr);
  const manifest = JSON.parse(fs.readFileSync(path.join(out, 'manifest.json'), 'utf8'));
  assert.ok(manifest.modules.some((name) => name.endsWith('App.vue')));
  assert.ok(manifest.css && fs.existsSync(path.join(out, manifest.css)));
  const module = new WebAssembly.Module(fs.readFileSync(path.join(out, manifest.wasm)));
  const exports = WebAssembly.Module.exports(module).map((item) => item.name);
  assert.ok(exports.includes('vn_start'));
  assert.ok(exports.includes('vn_flush'));
});

// Invalid manifests must be rejected before cleanup, even when earlier entries are valid.
test('build: rejects cleanup paths outside the output directory without deleting anything', async () => {
  const { buildWebApp } = await import('../../tooling/cli/build.mjs');
  for (const file of ['../victim', '/tmp/victim', 'nested/../../victim', '..', 42]) {
    const root = scratch('build-unsafe-manifest');
    const out = path.join(root, 'out');
    fs.mkdirSync(out);
    fs.writeFileSync(path.join(root, 'victim'), 'keep');
    fs.writeFileSync(path.join(out, 'keep.txt'), 'keep');
    fs.writeFileSync(path.join(out, 'manifest.json'), JSON.stringify({ tool: 'vue-native', files: ['keep.txt', file] }));
    assert.throws(() => buildWebApp({ out, name: 'probe', libRs: undefined }), /refusing to write/);
    assert.equal(fs.readFileSync(path.join(root, 'victim'), 'utf8'), 'keep');
    assert.equal(fs.readFileSync(path.join(out, 'keep.txt'), 'utf8'), 'keep');
  }
});

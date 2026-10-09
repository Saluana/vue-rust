// vue-native web packaging (WP-APP1): the wasm checks, the rerun rules and the input validation of buildWebApp.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { BuildError, buildWebApp, checkWebWasm } from '../../tooling/cli/build.mjs';
import { FIXTURES, freshPath } from './helpers.mjs';

const HOST_ONLY = fs.readFileSync(path.join(FIXTURES, 'host-only', 'lib.rs'), 'utf8');

test('checkWebWasm: rejects an empty module, which lacks the ABI exports (negative control)', () => {
  const empty = Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);
  assert.throws(() => checkWebWasm(empty), (err) => err instanceof BuildError && /lacks ABI exports: memory, vn_abi_version/.test(err.message));
});

test('checkWebWasm: rejects bytes that are not a wasm module', () => {
  assert.throws(() => checkWebWasm(Buffer.from('not wasm at all')), (err) => err instanceof BuildError && /not a valid module/.test(err.message));
});

test('buildWebApp: a rerun into a tool-owned directory replaces its files and keeps it owned', () => {
  const out = freshPath('rerun');
  const first = buildWebApp({ out, name: 'host-only', libRs: HOST_ONLY, container: '#app' });
  const second = buildWebApp({ out, name: 'host-only', libRs: HOST_ONLY, container: '#panel' });
  assert.notEqual(second.manifest.wasm, first.manifest.wasm, 'another container gives another module');
  assert.equal(fs.existsSync(path.join(out, first.manifest.wasm)), false, 'the old module is removed');
  assert.equal(fs.existsSync(path.join(out, second.manifest.wasm)), true, 'the new module is written');
  assert.match(fs.readFileSync(path.join(out, 'index.html'), 'utf8'), /<div id="panel"><\/div>/);
  assert.match(fs.readFileSync(path.join(out, '.vn-crate', 'src', 'vn_config.rs'), 'utf8'), /pub const CONTAINER: &str = "#panel";/);
  const manifest = JSON.parse(fs.readFileSync(path.join(out, 'manifest.json'), 'utf8'));
  assert.equal(manifest.tool, 'vue-native');
  assert.equal(manifest.container, '#panel');
  assert.deepEqual(manifest.files.slice().sort(), [second.manifest.wasm, 'vn-host.js', 'index.html', 'manifest.json'].sort());
});

test('buildWebApp: rejects an invalid app name or container before building anything', () => {
  const out = freshPath('bad-name');
  assert.throws(() => buildWebApp({ out, name: 'Bad Name', libRs: HOST_ONLY }), /must start with a lowercase letter/);
  assert.equal(fs.existsSync(out), false, 'nothing was created');
  assert.throws(() => buildWebApp({ out: freshPath('bad-container'), name: 'x', libRs: HOST_ONLY, container: 'app' }), /must be an id selector/);
});

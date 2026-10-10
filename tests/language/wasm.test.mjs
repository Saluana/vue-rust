// Task 4.2: language scenarios on wasm32-unknown-unknown. The expected documents are the Node goldens
// (tests/language/golden). The negative controls use a hand-mutated golden and a deliberate panic in a generated entry.
// Skipped with a reason when the wasm32 target is not installed (tooling/harness/build.mjs wasmSkipReason).
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {copyFile, mkdir, mkdtemp, readFile, rm, writeFile} from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {executeScenario} from '../../tooling/oracle/node-runner.mjs';
import {wasmSkipReason} from '../../tooling/harness/build.mjs';
import {runWasmScenarios} from '../../tooling/harness/run-language-wasm.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const goldenDir = path.join(here, 'golden');
const SKIP = wasmSkipReason() ?? false;

// A fixed subset of the default corpus: primitive math, a decimal-format fixture (toFixed), string methods, boolean
// coercion, and a closure whose mutable state is captured.
const SUBSET = [
  'numbers/remainder-bitwise',
  'numbers/to-fixed-ties',
  'strings/methods-basics',
  'coercion/truthiness-table',
  'functions/closure-counter',
];
const PANIC_CONTROL = {name: 'control/panic', body: 'panic!("negative control: deliberate panic")'};

// Deep recursion up to the runtime's call-depth limit (1500 calls, builtin callbacks included), through plain calls, closure
// frames with locals, and Array.prototype.map. These need the shadow stack of WASM_STACK_BYTES; with wasm-ld's 1 MiB default
// the first and the last trap. The expected documents come from the Node oracle when the test runs.
const DEEP = {
  'recurse-1400': 'function depth(n) {\n  return n === 0 ? 0 : 1 + depth(n - 1)\n}\ntrace(\'depth\', depth(1400))\n',
  'recurse-frames': 'const make = (k) => (n) => {\n  const a = n + k\n  const b = a * 2\n  const c = [a, b]\n'
    + '  return n === 0 ? c[0] - a : 1 + make(k)(n - 1) + (b - b)\n}\ntrace(\'frames\', make(1)(1400))\n',
  'overflow-via-map': 'function unbounded(n) {\n  return [n].map(() => unbounded(n + 1))[0]\n}\ntry {\n  unbounded(0)\n}'
    + ' catch (e) {\n  trace(\'overflow-via-map\', e.name, e instanceof RangeError)\n}\n',
};

test('a target that rustc does not know is a skip reason, not a pass', () => {
  // Negative control of the skip check: an unknown target must produce a reason.
  assert.equal(typeof wasmSkipReason('vn-no-such-target'), 'string');
});

test('the subset runs on wasm32 and every document equals its golden', {skip: SKIP}, async () => {
  const out = await runWasmScenarios({only: SUBSET});
  assert.deepEqual(out.results.map((r) => r.name).sort(), [...SUBSET].sort());
  for (const r of out.results) assert.equal(r.outcome, 'pass', `${r.name}: ${r.reason}`);
  assert.equal(out.failures, 0);
  assert.equal(out.wasm.imports, 0);
  assert.ok(out.wasm.gzipBytes > 0 && out.wasm.gzipBytes < out.wasm.bytes);
});

test('the subset under GC stress (a collection at every safepoint) equals the goldens', {skip: SKIP}, async () => {
  // The threshold changes when the collector runs, not what the scenarios compute: a value that a collection frees while it is
  // still reachable would change a document or trap here. wasm_entry.rs checks that threshold 1 does collect.
  const out = await runWasmScenarios({only: SUBSET, gcThreshold: 1});
  assert.equal(out.gcThreshold, 1);
  for (const r of out.results) assert.equal(r.outcome, 'pass', `${r.name}: ${r.reason}`);
  assert.equal(out.failures, 0);
});

test('negative controls: a mutated golden and a deliberate panic are reported as failures', {skip: SKIP}, async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'vn-wasm-golden-'));
  try {
    for (const name of SUBSET) {
      await mkdir(path.dirname(path.join(dir, name)), {recursive: true});
      await copyFile(path.join(goldenDir, `${name}.js.json`), path.join(dir, `${name}.js.json`));
    }
    // A wrong expectation: the golden of one scenario says its result is the other value.
    const mutated = 'numbers/to-fixed-ties';
    const file = path.join(dir, `${mutated}.js.json`);
    const golden = JSON.parse(await readFile(file, 'utf8'));
    golden.result = golden.result === 'ok' ? 'throw' : 'ok';
    await writeFile(file, JSON.stringify(golden));

    const out = await runWasmScenarios({only: SUBSET, golden: dir, controls: [PANIC_CONTROL]});
    const byName = Object.fromEntries(out.results.map((r) => [r.name, r]));
    assert.equal(byName[mutated].outcome, 'FAIL');
    assert.match(byName[mutated].reason, /^first difference at \$\.result:/);
    for (const name of SUBSET.filter((n) => n !== mutated)) assert.equal(byName[name].outcome, 'pass', name);
    // The panic aborts the instance (wasm32 has panic = "abort"): it is a trap, and the other scenarios still ran.
    assert.equal(byName[PANIC_CONTROL.name].outcome, 'FAIL');
    assert.match(byName[PANIC_CONTROL.name].reason, /^trap: RuntimeError: /);
    // Exactly the mutated golden is unexpected; the control failed as it must.
    assert.equal(out.failures, 1);
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});

test('deep recursion to the call-depth limit runs on wasm32 and matches the Node oracle', {skip: SKIP}, async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'vn-wasm-deep-'));
  try {
    const src = path.join(dir, 'src');
    const golden = path.join(dir, 'golden');
    await mkdir(src);
    await mkdir(golden);
    for (const [name, text] of Object.entries(DEEP)) {
      const file = path.join(src, `${name}.js`);
      await writeFile(file, text);
      await writeFile(path.join(golden, `${name}.js.json`), `${JSON.stringify(await executeScenario(file))}\n`);
    }
    const out = await runWasmScenarios({dir: src, golden});
    assert.equal(out.results.length, Object.keys(DEEP).length);
    for (const r of out.results) assert.equal(r.outcome, 'pass', `${r.name}: ${r.reason}`);
    assert.equal(out.failures, 0);
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});

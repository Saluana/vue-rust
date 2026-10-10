// Source frames of uncaught errors (WP 3.7, part A): an error thrown by a .vue component is reported by the headless runner
// with the author's file:line:column of the throw (and, after an await, of the await). The expected positions are hand-derived
// from the fixture texts (tests/cli/fixtures/throw-setup and throw-async), not taken from the runner's output.
// The apps are built with the dev profile (the same sources; release builds with LTO take minutes here).
import { describe, test, before } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { buildHeadlessApp } from '../../tooling/cli/build.mjs';
import { FIXTURES, freshPath, scratch } from './helpers.mjs';

// Runs a headless binary on a script of steps. Returns { status, stdout, stderr }.
function runApp(binary, steps) {
  const dir = scratch(`frames-${path.basename(binary)}-${steps.length}-${Math.random().toString(36).slice(2, 8)}`);
  const script = path.join(dir, 'script.json');
  fs.writeFileSync(script, JSON.stringify({ steps }));
  const r = spawnSync(binary, [script], { encoding: 'utf8', timeout: 60000 });
  return { status: r.status, stdout: r.stdout, stderr: r.stderr };
}

// The frame lines of a runner's stderr: "    at [async ]name (file:line:column)" (the runner indents them four spaces).
const frameLines = (stderr) => stderr.split('\n').filter((l) => /^ {4}at /.test(l));

// Asserts that the first frame of stderr is `at <name> (<file>:<line>:<column>)` with the given values.
function expectFrame(stderr, { name, file = 'src/App.vue', line, column }) {
  const first = frameLines(stderr)[0];
  assert.equal(first, `    at ${name} (${file}:${line}:${column})`, stderr);
}

// A copy of a fixture in a scratch directory, with the given edit applied to src/App.vue.
function editedFixture(fixture, label, edit) {
  const dir = scratch(`frames-edit-${label}`);
  fs.cpSync(path.join(FIXTURES, fixture), dir, { recursive: true });
  const file = path.join(dir, 'src', 'App.vue');
  fs.writeFileSync(file, edit(fs.readFileSync(file, 'utf8')));
  return dir;
}

describe('source frames: a .vue component whose setup throws synchronously', () => {
  let binary;
  before(() => {
    const out = freshPath('frames-throw-setup');
    binary = path.join(buildHeadlessApp({ root: path.join(FIXTURES, 'throw-setup'), entry: 'src/App.vue', out, name: 'throw-setup', profile: 'dev' }).dir, 'throw-setup');
  });

  test('the throw at line 5, column 0 of App.vue is reported by the mount (exit 1)', () => {
    const r = runApp(binary, [{ op: 'html', selector: '#count' }]);
    assert.equal(r.status, 1, r.stderr);
    assert.match(r.stderr, /^app error: mount: Error: setup failed\n/);
    assert.equal(frameLines(r.stderr).length, 1, r.stderr);
    expectFrame(r.stderr, { name: 'setup', line: 5, column: 0 });
  });

  test('negative control: two lines inserted above the throw move the reported line by two', () => {
    const dir = editedFixture('throw-setup', 'setup-moved', (text) => text.replace("throw new Error('setup failed')", "// one\n// two\nthrow new Error('setup failed')"));
    const out = freshPath('frames-throw-setup-moved');
    const moved = path.join(buildHeadlessApp({ root: dir, entry: 'src/App.vue', out, name: 'throw-setup-moved', profile: 'dev' }).dir, 'throw-setup-moved');
    const r = runApp(moved, [{ op: 'html', selector: '#count' }]);
    assert.equal(r.status, 1, r.stderr);
    expectFrame(r.stderr, { name: 'setup', line: 7, column: 0 });
  });

  test('negative control: an error the runtime raises, not a throw statement, gets no frame', () => {
    const dir = editedFixture('throw-setup', 'setup-runtime', (text) => text.replace("throw new Error('setup failed')", 'const missing = null\nmissing.value'));
    const out = freshPath('frames-throw-setup-runtime');
    const runtime = path.join(buildHeadlessApp({ root: dir, entry: 'src/App.vue', out, name: 'throw-setup-runtime', profile: 'dev' }).dir, 'throw-setup-runtime');
    const r = runApp(runtime, [{ op: 'html', selector: '#count' }]);
    assert.equal(r.status, 1, r.stderr);
    assert.match(r.stderr, /^app error: mount: TypeError: /);
    assert.deepEqual(frameLines(r.stderr), []);
  });
});

describe('source frames: a .vue component whose async handler throws after an await', () => {
  let binary;
  before(() => {
    const out = freshPath('frames-throw-async');
    binary = path.join(buildHeadlessApp({ root: path.join(FIXTURES, 'throw-async'), entry: 'src/App.vue', out, name: 'throw-async', profile: 'dev' }).dir, 'throw-async');
  });

  test('the throw at line 9 and the await at line 8 are reported, the throw first (exit 1)', () => {
    const r = runApp(binary, [{ op: 'click', selector: '#go' }, { op: 'html', selector: '#status' }]);
    assert.equal(r.status, 1, r.stderr);
    assert.match(r.stderr, /^app error: step 0: Error: load failed\n/);
    assert.deepEqual(frameLines(r.stderr), ['    at load (src/App.vue:9:2)', '    at async load (src/App.vue:8:2)']);
  });

  test('negative control: a wrong expected line is refused by the frame check', () => {
    const r = runApp(binary, [{ op: 'click', selector: '#go' }]);
    assert.throws(() => expectFrame(r.stderr, { name: 'load', line: 10, column: 2 }), /AssertionError|Expected values|load \(src\/App\.vue:9:2\)/);
    expectFrame(r.stderr, { name: 'load', line: 9, column: 2 });
  });
});

// The dev form of <script setup> (devState, used by vue-native dev) has the same script, but its template is a separate
// module, so the offset of the script in the generated code moves. The frames must still land on the same author lines.
describe('source frames in the dev form (devState): the same positions', () => {
  test('a setup throw is still reported at App.vue line 5, column 0', () => {
    const out = freshPath('frames-dev-throw-setup');
    const binary = path.join(buildHeadlessApp({ root: path.join(FIXTURES, 'throw-setup'), entry: 'src/App.vue', out, name: 'throw-setup-dev', profile: 'dev', devState: true }).dir, 'throw-setup-dev');
    const r = runApp(binary, [{ op: 'html', selector: '#count' }]);
    assert.equal(r.status, 1, r.stderr);
    assert.match(r.stderr, /^app error: mount: Error: setup failed\n/);
    expectFrame(r.stderr, { name: 'setup', line: 5, column: 0 });
  });

  test('an async throw after an await is still reported at lines 9:2 (throw) and 8:2 (await)', () => {
    const out = freshPath('frames-dev-throw-async');
    const binary = path.join(buildHeadlessApp({ root: path.join(FIXTURES, 'throw-async'), entry: 'src/App.vue', out, name: 'throw-async-dev', profile: 'dev', devState: true }).dir, 'throw-async-dev');
    const r = runApp(binary, [{ op: 'click', selector: '#go' }, { op: 'html', selector: '#status' }]);
    assert.equal(r.status, 1, r.stderr);
    assert.deepEqual(frameLines(r.stderr), ['    at load (src/App.vue:9:2)', '    at async load (src/App.vue:8:2)']);
  });
});

// error.stack of a throw that a try/catch catches keeps the text every error has: "Name: message", then the line
// "    at <anonymous>" (crates/value/src/error.rs, stack_text; kernel_cells_errors.rs pins the same text). The component
// compares its caught error's stack with that text. The negative control gives the expected text a source position.
describe('source frames: the error.stack of a caught throw is unchanged', () => {
  let binary;
  before(() => {
    const out = freshPath('frames-throw-caught');
    binary = path.join(buildHeadlessApp({ root: path.join(FIXTURES, 'throw-caught'), entry: 'src/App.vue', out, name: 'throw-caught', profile: 'dev' }).dir, 'throw-caught');
  });

  test('the stack of the caught Error is "Error: caught" and "    at <anonymous>" (exit 0)', () => {
    const r = runApp(binary, [{ op: 'html', selector: '#verdict' }]);
    assert.equal(r.status, 0, r.stderr);
    assert.equal(JSON.parse(r.stdout)[0].html, '<p id="verdict">unchanged</p>');
  });

  test('negative control: an expected stack with a source position is reported as changed', () => {
    const dir = editedFixture('throw-caught', 'stack-position', (text) => text.replace("'Error: caught\\n    at <anonymous>'", "'Error: caught\\n    at src/App.vue:6:2'"));
    const out = freshPath('frames-throw-caught-position');
    const edited = path.join(buildHeadlessApp({ root: dir, entry: 'src/App.vue', out, name: 'throw-caught-position', profile: 'dev' }).dir, 'throw-caught-position');
    const r = runApp(edited, [{ op: 'html', selector: '#verdict' }]);
    assert.equal(r.status, 0, r.stderr);
    assert.equal(JSON.parse(r.stdout)[0].html, '<p id="verdict">changed</p>');
  });
});

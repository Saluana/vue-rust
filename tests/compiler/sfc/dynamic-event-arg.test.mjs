// Regression fixture for the prototype's dynamic event argument defect (task 4.1). The prototype compiler accepts
// <button @[click]="inc"> and emits the same Rust as <button @click="inc"> (pinned as a KNOWN BUG in
// tests/fixtures/prototype/regressions.test.mjs). Through the SFC pipeline:
//   (b) the prototype's component, where `click` is not declared, is rejected: VN4016 (a free global that the native
//       profile does not provide), with the span of the `click` token in the .vue file;
//   (a) when the variable is declared, the handler is bound under the variable's value, which is what Vue does
//       (@[click] compiles to a dynamic toHandlerKey key). The generated module is built into a headless native binary
//       (tooling/cli, the path tests/cli/headless.test.mjs uses) and run; the expected behaviour is written from Vue's
//       rules, not from the compiler's output. Nothing here needs Chrome.
import { test, describe, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { parse as parseSfc, compileScript } from '@vue/compiler-sfc';
import { compileSfc } from '../../../compiler/sfc/index.mjs';
import { buildHeadlessApp, toolEnv } from '../../../tooling/cli/build.mjs';
import { compile as prototypeCompile } from '../../fixtures/prototype/counter/compiler/compile.mjs';

const INC = "import {ref} from 'vue'; const count = ref(0); function inc() { count.value++ }";
const component = (script, template) => `<script setup>${script}</script><template>${template}</template>`;

// The prototype's input, unchanged: `click` is not declared anywhere in the component.
const PROTOTYPE_DYNAMIC = component(INC, '<button @[click]="inc">Add</button>');
const PROTOTYPE_STATIC = component(INC, '<button @click="inc">Add</button>');

// The declared form: the variable holds the event name. The swap button changes it to 'click'.
const APP = `<script setup>
import { ref } from 'vue'
const click = ref('input')
const count = ref(0)
function inc() { count.value++ }
function useClick() { click.value = 'click' }
</script>

<template>
  <p id="count">{{ count }}</p>
  <input id="target" @[click]="inc">
  <button id="swap" @click="useClick">swap</button>
</template>
`;
const APP_FIXED = APP.replace('@[click]="inc"', '@click="inc"'); // what the prototype emits for @[click]

// The interaction script. The handler starts bound to input events: a click does nothing and an input event runs it.
// The swap button moves the binding to click events. (The headless runner has click and input operations.)
const STEPS = [
  { op: 'click', selector: '#target' },
  { op: 'html', selector: '#count' },
  { op: 'input', selector: '#target', value: 'a' },
  { op: 'html', selector: '#count' },
  { op: 'click', selector: '#swap' },
  { op: 'input', selector: '#target', value: 'b' },
  { op: 'html', selector: '#count' },
  { op: 'click', selector: '#target' },
  { op: 'html', selector: '#count' },
  { op: 'snapshot', name: 'end' },
];
// Expected, from Vue's rules: the count after each html step, then the mounted container.
const EXPECTED_COUNTS = ['<p id="count">0</p>', '<p id="count">1</p>', '<p id="count">1</p>', '<p id="count">2</p>'];
const EXPECTED_END = '<div id="app" data-v-app=""><p id="count">2</p><input id="target"><button id="swap">swap</button></div>';

// The assertion of the regression and of its negative control. The component must be rejected with one error, VN4016,
// whose span is the `click` token of `@[click]`.
function assertRejectedAtClick(result, source) {
  assert.ok(result !== null && typeof result === 'object' && Array.isArray(result.diagnostics), 'the pipeline returns diagnostics');
  assert.equal(result.code, null, 'no module is produced');
  const errors = result.diagnostics.filter((d) => d.severity === 'error');
  assert.deepEqual(errors.map((d) => d.code), ['VN4016']);
  const [d] = errors;
  assert.equal(d.file, 'App.vue');
  const at = source.indexOf('@[click]') + 2; // single line: column equals offset
  assert.deepEqual(d.start, { offset: at, line: 1, column: at });
  assert.equal(source.slice(d.start.offset, d.end.offset), 'click');
  assert.match(d.message, /global 'click'/);
}

// The assertion of the runtime test and of its negative control, on the results of one script run.
function assertBoundToVariable(results) {
  assert.equal(results.at(-1).op, 'snapshot');
  assert.deepEqual(results.filter((r) => r.op === 'html').map((r) => r.html), EXPECTED_COUNTS);
  assert.equal(results.at(-1).html, EXPECTED_END);
}

let work;
before(() => {
  // Scratch apps live under the cargo target directory of this checkout (created here if it does not exist yet).
  const base = path.join(toolEnv().CARGO_TARGET_DIR, 'compiler-tests');
  fs.mkdirSync(base, { recursive: true });
  work = fs.mkdtempSync(path.join(base, 'dynamic-event-arg-'));
});
after(() => {
  if (work) fs.rmSync(work, { recursive: true, force: true });
});

// Builds the source as a headless app and returns the binary path.
function buildApp(label, source) {
  const root = path.join(work, label);
  fs.mkdirSync(root, { recursive: true });
  fs.writeFileSync(path.join(root, 'App.vue'), source);
  const name = `dyn-${label}`;
  const built = buildHeadlessApp({ root, entry: 'App.vue', out: path.join(work, `${label}-out`), name });
  return path.join(built.dir, name);
}

// Runs the script on the binary; returns the result array (exit status 0 is required).
function runScript(binary) {
  const script = path.join(work, `${path.basename(binary)}.script.json`);
  fs.writeFileSync(script, JSON.stringify({ steps: STEPS }));
  const r = spawnSync(binary, [script], { encoding: 'utf8', timeout: 60000 });
  assert.equal(r.status, 0, r.stderr);
  return JSON.parse(r.stdout);
}

describe('@[click] dynamic event argument through the SFC pipeline', () => {
  test('(b) the prototype component is rejected with VN4016 at the click token of @[click]', () => {
    assertRejectedAtClick(compileSfc(PROTOTYPE_DYNAMIC, { filename: 'App.vue' }), PROTOTYPE_DYNAMIC);
  });

  test('negative control: the prototype output for the same component is accepted, so the same assertion fails', () => {
    const proto = prototypeCompile(PROTOTYPE_DYNAMIC);
    assert.equal(typeof proto, 'string', 'the prototype accepts the component and emits Rust');
    // The defect itself: the prototype's output for @[click] is the output for a fixed @click handler.
    assert.equal(proto, prototypeCompile(PROTOTYPE_STATIC));
    // Shaped as a pipeline result with no diagnostics, so the assertion fails on the defect itself (a module is produced),
    // not on the type of its input.
    assert.throws(
      () => assertRejectedAtClick({ code: proto, diagnostics: [] }, PROTOTYPE_DYNAMIC),
      (e) => e instanceof assert.AssertionError && /no module is produced/.test(e.message),
    );
  });

  test('the name of a dynamic argument is checked for every directive, not only @ and :', () => {
    const cases = [
      ['v-model on a component', "<script setup>import { ref } from 'vue'; const Comp = {}; const x = ref('a')</script><template><Comp v-model:[nm]=\"x\" /></template>", 'nm'],
      ['v-slot on a component', '<script setup>const Comp = {}</script><template><Comp v-slot:[sl]="p"></Comp></template>', 'sl'],
      ['a custom directive', '<script setup>const vFoo = {}; const n = 1</script><template><p v-foo:[zz]="n">y</p></template>', 'zz'],
    ];
    for (const [label, src, name] of cases) {
      const errors = compileSfc(src, { filename: 'App.vue' }).diagnostics.filter((d) => d.severity === 'error');
      assert.deepEqual(errors.map((d) => d.code), ['VN4016'], label);
      assert.equal(src.slice(errors[0].start.offset, errors[0].end.offset), name, `${label}: the span is the argument name`);
    }
    // Control: the same directives with declared names are accepted.
    const declared = "<script setup>import { ref } from 'vue'; const Comp = {}; const x = ref('a'); const nm = 'title'</script><template><Comp v-model:[nm]=\"x\" /></template>";
    assert.equal(compileSfc(declared, { filename: 'App.vue' }).code !== null, true, 'a declared argument name compiles');
  });

  test('(a) a declared variable compiles to a dynamic handler key, the key the official compiler emits', () => {
    const r = compileSfc(APP, { filename: 'App.vue' });
    assert.deepEqual(r.diagnostics.filter((d) => d.severity === 'error').map((d) => d.code), []);
    const { descriptor } = parseSfc(APP, { filename: 'App.vue' });
    const official = compileScript(descriptor, { id: 'app', isProd: true, inlineTemplate: true }).content;
    const key = '[_toHandlerKey(click.value)]: inc';
    assert.ok(official.includes(key), 'Vue compiles the dynamic argument to this key');
    assert.ok(r.code.includes(key), 'the pipeline emits the same key');
    assert.doesNotMatch(r.code, /onClick: inc/, 'the handler is not fixed to click');
  });

  test('(a) the handler is bound under the variable event, and the binding follows the variable at runtime', () => {
    assertBoundToVariable(runScript(buildApp('dynamic', APP)));
  });

  test('negative control: the fixed @click that the prototype emits for @[click] fails the same runtime assertion', () => {
    assert.notEqual(APP_FIXED, APP, 'the control differs from the dynamic component');
    const results = runScript(buildApp('fixed', APP_FIXED));
    assert.throws(() => assertBoundToVariable(results), assert.AssertionError);
    // The first click already runs the handler, because the fixed handler is bound to click.
    assert.equal(results.find((r) => r.op === 'html').html, '<p id="count">1</p>');
  });
});

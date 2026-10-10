// Unit tests of the upstream oracle (tooling/harness/run-upstream.mjs and the vitest shim). Most of them need no Vue
// checkout: they use small in-memory specs. The tests that read the real reactivity specs skip with a reason when the
// checkout is missing (set VUE_CORE_DIR, or clone https://github.com/vuejs/core at v3.5.43; see tests/upstream/README.md).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { parseModule } from '../../compiler/frontend/parse.mjs';
import {
  applyEdits,
  compileText,
  DEFAULT_LEDGER,
  findCalls,
  importPlan,
  interpretRun,
  KEY_SEP,
  listSpecs,
  markUnsupported,
  prepareSpec,
  ratchet,
  resolveVueCore,
  rewriteSpec,
  testKind,
} from '../../tooling/harness/run-upstream.mjs';
import * as shim from '../../tooling/harness/upstream-vitest.js';

const VUE_CORE = resolveVueCore();
const CHECKOUT = VUE_CORE === null
  ? { skip: 'no vue-core checkout with packages/reactivity/__tests__: set VUE_CORE_DIR or clone https://github.com/vuejs/core at v3.5.43' }
  : {};

// The error a matcher throws for a failed check, or null when it passes.
function failureOf(fn) {
  try {
    fn();
  } catch (e) {
    return e;
  }
  return null;
}

const parse = (source) => parseModule(source, { filename: 'case.spec.ts', lang: 'ts' }).ast;

// [name, status, reason] of every test in a source, as the static marks see it (no compile).
function marksOf(source, vueExports = new Set(['ref', 'effect', 'readonly', 'reactive'])) {
  const ast = parse(source);
  const { missing } = importPlan(ast.program, vueExports);
  const calls = findCalls(ast);
  const marks = markUnsupported(ast.program, calls, missing);
  return calls
    .map((call, i) => ({ call, mark: marks[i] }))
    .filter(({ call }) => call.kind === 'test')
    .map(({ call, mark }) => [call.node.arguments[0].value, mark === null ? 'run' : mark.status, mark === null ? '' : mark.reason]);
}

// ---- the vitest shim, as matchers and mocks (no registration, so the shared registry stays empty) ----

test('matchers pass on equal values and fail with the reason the report shows', () => {
  shim.expect(1).toBe(1);
  shim.expect({ a: [1, { b: 2 }] }).toEqual({ a: [1, { b: 2 }] });
  shim.expect({ a: 1, b: undefined }).toEqual({ a: 1 });
  shim.expect(1).not.toBe(2);
  const failed = failureOf(() => shim.expect(1).toBe(2));
  assert.equal(failed.upstream, 'fail');
  assert.equal(failed.reason, 'expected 1 to be 2');
  assert.equal(failureOf(() => shim.expect(1).not.toBe(1)).reason, 'expected not: expected 1 to be 1');
  assert.equal(failureOf(() => shim.expect({ a: 1, b: undefined }).toStrictEqual({ a: 1 })).upstream, 'fail');
  shim.expect(() => {
    throw new Error('boom');
  }).toThrow('boo');
  assert.equal(failureOf(() => shim.expect(() => {}).toThrow()).upstream, 'fail');
});

test('dev-only warning matchers are unsupported, not failures', () => {
  const refused = failureOf(() => shim.expect(() => {}).toHaveBeenWarned());
  assert.equal(refused.upstream, 'unsupported');
  assert.match(refused.reason, /dev-only warning/);
});

test('vi.fn records its calls, forwards the receiver and returns the implementation value', () => {
  const plain = shim.vi.fn((a, b) => a + b);
  assert.equal(plain(1, 2), 3);
  shim.expect(plain).toHaveBeenCalledTimes(1);
  shim.expect(plain).toHaveBeenLastCalledWith(1, 2);
  plain.mockClear();
  shim.expect(plain).toHaveBeenCalledTimes(0);
  // A spy on a method sees its receiver, as vitest's does (reactiveArray's identity-method spies rely on it).
  const obj = { base: 10, add: shim.vi.fn(function (n) {
    return this.base + n;
  }) };
  assert.equal(obj.add(5), 15);
  assert.equal(shim.vi.fn()(), undefined);
});

// ---- import rewriting and the static marks ----

test('importPlan keeps the names a Vue package exports and marks the rest missing', () => {
  const src = "import { ref, watch as w, type Ref } from '../src';\nimport { ref as r } from 'vue';\nimport { describe, it, vi } from 'vitest';\nimport { h } from '@vue/runtime-test';\n";
  const ast = parse(src);
  const { edits, missing } = importPlan(ast.program, new Set(['ref']));
  // The reactivity package's watch is synchronous and not public in the port, so it is refused, as is any unexported name.
  assert.equal(missing.get('w'), '../src');
  assert.equal(missing.get('h'), '@vue/runtime-test');
  assert.equal(missing.has('Ref'), false, 'a type-only specifier is erased, not missing');
  const text = applyEdits(src, edits);
  assert.match(text, /import \{ ref \} from 'vue';/);
  assert.match(text, /import \{ ref as r \} from 'vue';/, 'an aliased import keeps its alias');
  assert.doesNotMatch(text, /vitest/, 'the shim import is removed');
});

test('static marks: a read of a name the port lacks, a Vue internal, a profile-excluded method or a debugger hook', () => {
  const src = [
    "import { ref, watch } from '../src';",
    "import { effect } from 'vue';",
    "describe('marks', () => {",
    "  it('reads a missing name', () => { watch(() => 1, () => {}); });",
    "  it('reads a Vue internal field', () => { const s: any = effect(() => {}); expect(s.effects).toBe(0); });",
    "  it('uses a dev-only hook', () => { effect(() => {}, { onTrack: () => {} }); });",
    "  it('calls an Array method the profile excludes', () => { const a: any = [1]; a.copyWithin(0, 0); });",
    "  it('uses what the port has', () => { expect(ref(1).value).toBe(1); });",
    '});',
  ].join('\n');
  const marks = new Map(marksOf(src).map(([name, status, reason]) => [name, [status, reason]]));
  assert.match(marks.get('reads a missing name')[1], /^unsupported-import: watch from '\.\.\/src' is not exported/);
  assert.match(marks.get('reads a Vue internal field')[1], /^unsupported-internal: reads Vue internal field \.effects/);
  assert.match(marks.get('uses a dev-only hook')[1], /^unsupported-dev-only: uses the debugger hook onTrack/);
  assert.match(marks.get('calls an Array method the profile excludes')[1], /^unsupported-profile: calls \.copyWithin/);
  assert.equal(marks.get('uses what the port has')[0], 'run');
});

test('static marks: a describe or the top level that reads a missing name marks every test under it', () => {
  const src = [
    "import { watch } from '../src';",
    "describe('outer', () => {",
    '  const w = watch;',
    "  it('inherits the describe reason', () => {});",
    '});',
  ].join('\n');
  const [[name, status, reason]] = marksOf(src);
  assert.equal(name, 'inherits the describe reason');
  assert.equal(status, 'unsupported');
  assert.match(reason, /^unsupported-import: watch/);
  const top = "import { watch } from '../src';\nconst t = watch;\nit('inherits the top level', () => {});\n";
  assert.equal(marksOf(top)[0][1], 'unsupported');
});

test('a test the compiler rejects is refused by name, and the rest of its describe still runs', () => {
  const src = [
    "import { ref } from '../src';",
    "describe('group', () => {",
    '  class Box {}',
    "  it('uses the class', () => { expect(new Box()).toBeTruthy(); });",
    "  it('uses ref', () => { expect(ref(1).value).toBe(1); });",
    '});',
  ].join('\n');
  const prepared = prepareSpec(src, { filename: 'group.spec.ts', specifier: 'upstream/group', vueExports: new Set(['ref']) });
  assert.equal(typeof prepared.rust, 'string');
  assert.match(prepared.text, /test\("uses the class", \(\) => __upstreamRefuse\("compile-error", /);
  assert.doesNotMatch(prepared.text, /test\("uses ref", \(\) => __upstreamRefuse/);
});

test('compileText compiles through the same checks as compileModule, including the stdlib manifest', () => {
  const ok = compileText('export const x: number = 1;\n', { filename: 'ok.ts', specifier: 'ok', lang: 'ts' });
  assert.equal(typeof ok.rust, 'string');
  const cls = compileText('class A {}\n', { filename: 'cls.ts', specifier: 'cls', lang: 'ts' });
  assert.ok(cls.diagnostics.some((d) => d.code === 'VN2002'), 'classes are outside the profile');
  // Symbol.isConcatSpreadable is status X in compiler/profile/stdlib.json: the manifest check refuses it (VN4017).
  const sym = compileText('const k = Symbol.isConcatSpreadable;\n', { filename: 'sym.js', specifier: 'sym', lang: 'js' });
  assert.ok(sym.diagnostics.some((d) => d.code === 'VN4017'));
  assert.equal(testKind(parse("it('x', () => {});").program.body[0].expression), 'test');
});

test('rewriteSpec leaves the Vue imports of a spec as the native module and refuses the reads it cannot satisfy', () => {
  const src = "import { ref, watch } from '../src';\nit('reads watch', () => { watch(() => 1, () => {}); });\n";
  const { body } = rewriteSpec(src, { filename: 'rw.spec.ts', vueExports: new Set(['ref']) });
  assert.match(body, /import \{ ref \} from 'vue';/);
  assert.match(body, /__upstreamRefuse\("unsupported", "unsupported-import: watch from '\.\.\/src'/);
});

test('ratchet fails only when a test the ledger marks pass is not pass now, and only for the selected specs', () => {
  const ledger = { 'a :: x': 'pass', 'a :: y': 'fail', 'b :: z': 'pass', 'c :: w': 'pass' };
  const current = new Map([
    ['a :: x', { status: 'fail', reason: 'boom' }],
    ['a :: y', { status: 'pass' }],
    ['b :: z', { status: 'pass' }],
  ]);
  const regressions = ratchet(ledger, current, new Set(['a', 'b']));
  assert.deepEqual(regressions.map((r) => [r.key, r.now, r.reason]), [['a :: x', 'fail', 'boom']]);
  // A test that disappears is a regression too, and a file-level outcome covers the tests of its spec.
  assert.deepEqual(ratchet({ 'a :: gone': 'pass' }, new Map(), new Set(['a'])).map((r) => r.now), ['missing']);
  assert.deepEqual(ratchet({ ['a' + KEY_SEP + 'q']: 'pass' }, new Map([['a', { status: 'compile-error', reason: 'x' }]]), new Set(['a'])).map((r) => r.now), ['compile-error']);
});

test('interpretRun maps the trace to per-test outcomes, including a panic and a module that throws', () => {
  const planned = [{ label: 'upstream-plan', values: ['a'] }, { label: 'upstream-plan', values: ['b'] }];
  const ok = interpretRun({ result: 'ok', entries: [...planned, { label: 'upstream-test', values: ['a', 'pass', ''] }] }, null);
  assert.equal(ok.results.get('a').status, 'pass');
  assert.equal(ok.results.get('b').reason, 'no result reported');
  const panicked = interpretRun({ result: 'panic', error: 'index out of range', entries: [...planned, { label: 'upstream-start', values: ['a'] }] }, null);
  assert.match(panicked.results.get('a').reason, /^panic: index out of range/);
  assert.equal(panicked.results.get('b').reason, 'not run: an earlier test panicked');
  const threw = interpretRun({ result: 'throw', error: { name: 'ReferenceError', message: 'x is not defined' }, entries: [] }, null);
  assert.deepEqual(threw.fileStatus, { status: 'fail', reason: 'module threw: ReferenceError: x is not defined' });
});

test('negative control: a deliberately failing expect is reported as fail, and the tests after it still run', async () => {
  // This file registers tests only here (the shim's registry is shared by the module), so the run covers exactly these two.
  shim.describe('control', () => {
    shim.it('fails on purpose', () => shim.expect(1).toBe(2));
    shim.it('passes after it', () => shim.expect(1).toBe(1));
  });
  const entries = [];
  globalThis.trace = (label, ...values) => entries.push({ label, values });
  try {
    await shim.__upstreamRunAll();
  } finally {
    delete globalThis.trace;
  }
  const { results } = interpretRun({ result: 'ok', entries }, null);
  assert.equal(results.get('control > fails on purpose').status, 'fail');
  assert.equal(results.get('control > fails on purpose').reason, 'expected 1 to be 2');
  assert.equal(results.get('control > passes after it').status, 'pass');
});

// ---- the checkout ----

test('the ledger holds only the four outcome names', () => {
  const ledger = JSON.parse(readFileSync(DEFAULT_LEDGER, 'utf8'));
  const allowed = new Set(['pass', 'fail', 'compile-error', 'unsupported']);
  for (const [key, status] of Object.entries(ledger)) assert.ok(allowed.has(status), `${key}: ${status}`);
});

test('the checkout holds the reactivity specs, and the ledger has an entry for every one of them', CHECKOUT, () => {
  const specs = listSpecs(VUE_CORE);
  const names = new Set(specs.map((s) => s.name));
  assert.ok(names.has('reactive') && names.has('computed') && names.has('collections/Set'), 'the spec names are the paths under the tests directory');
  const ledger = JSON.parse(readFileSync(DEFAULT_LEDGER, 'utf8'));
  for (const key of Object.keys(ledger)) assert.ok(names.has(key.split(KEY_SEP)[0]), `ledger key of an unknown spec: ${key}`);
  for (const name of names) {
    assert.ok(Object.keys(ledger).some((key) => key.startsWith(name + KEY_SEP)), `no ledger entry for ${name}`);
  }
});

test('every spec of the checkout is prepared without an exception', CHECKOUT, () => {
  const vueExports = new Set(['ref', 'reactive', 'effect', 'computed', 'readonly', 'shallowRef', 'shallowReactive', 'triggerRef']);
  for (const spec of listSpecs(VUE_CORE)) {
    const prepared = prepareSpec(readFileSync(spec.file, 'utf8'), { filename: `${spec.name}.spec.ts`, specifier: `upstream/${spec.name}`, vueExports });
    assert.ok(typeof prepared.rust === 'string' || typeof prepared.fileError?.reason === 'string', spec.name);
  }
});

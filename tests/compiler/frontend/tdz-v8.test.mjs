// Cross-check of needsTdzCheck against V8. Each snippet runs inside a function body in a fresh node:vm context. A
// "before initialization" ReferenceError for the snippet's x is a real TDZ fault. Soundness: every real fault must be
// flagged (no false negatives). False positives are allowed (conservative analysis) and are only required to exist
// in bounded number, so the analysis cannot be trivially "always true".
import { test } from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { analyzeScopes } from '../../../compiler/analysis/scope.mjs';
import { parseRecovering } from './helpers.mjs';

// Templates use the declaration keyword as {k}. Each template is one module-level or nested snippet reading x.
const TEMPLATES = [
  ['read-after-init', '{k} x = 1; x;', false],
  ['read-before-decl', 'x; {k} x = 1;', true],
  ['self-init', '{k} x = x + 1;', true],
  ['closure-before-call-before-init', 'const f = () => x; f(); {k} x = 1;', true],
  ['closure-before-init-called-after', 'const f = () => x; {k} x = 1; f();', false],
  ['closure-after-init', '{k} x = 1; const f = () => x; f();', false],
  ['hoisted-fn-called-before-init', 'function h() { return x; } h(); {k} x = 1;', true],
  ['hoisted-fn-called-after-init', 'function h() { return x; } {k} x = 1; h();', false],
  ['switch-skips-declaration', 'switch (1) { case 0: {k} x = 1; break; case 1: x; }', true],
  ['switch-same-case', 'switch (0) { case 0: {k} x = 1; x; }', false],
  ['typeof-before-init', 'typeof x; {k} x = 1;', true],
  ['assign-before-init', 'x = 2; {k} x = 1;', true],
  ['loop-read-before-decl-in-body', 'let i = 0; while (i < 2) { if (i === 1) { x; } {k} x = i; i++; }', true],
  ['loop-read-after-decl-in-body', 'for (let j = 0; j < 2; j++) { {k} x = j; x; }', false],
  ['closure-in-default-parameter', '(function (a = () => x) { return a(); })(); {k} x = 1;', true],
  ['getter-after-init', '{k} x = 1; const obj = { get v() { return x; } }; obj.v;', false],
  ['nested-block-read-after-init', '{k} x = 1; { x; }', false],
  ['destructuring-default-reads-self-decl', 'const { a = x } = {}; {k} x = 1;', true],
  ['closure-from-function-created-before-init', 'function outer() { return () => x; } const z = outer(); {k} x = 1; const r = z(); r;', false],
  ['closure-created-before-init-in-loop', 'const fs = []; for (let n = 0; n < 1; n++) { fs.push(() => x); } {k} x = 1; fs[0]();', false],
];

// var has no TDZ: V8 never faults on it, and the analysis must never flag it.
const KINDS = ['let', 'const', 'var'];

function corpus() {
  const out = [];
  for (const [name, template] of TEMPLATES) {
    for (const k of KINDS) {
      if (template.includes('i++') && k === 'const') continue; // the loop template increments x-independent state
      out.push({ name: `${name}/${k}`, code: template.split('{k}').join(k) });
    }
  }
  return out;
}

// Runs the snippet in V8. Returns true when x faults with a TDZ ReferenceError.
function v8Faults(code) {
  try {
    vm.runInNewContext(`(function () {\n${code}\n})();`, {});
    return false;
  } catch (err) {
    return err && err.name === 'ReferenceError' && /Cannot access 'x' before initialization/.test(err.message);
  }
}

// Analysis verdict: some binding named x is flagged for a TDZ check.
function analysisFlags(code) {
  const ast = parseRecovering(code);
  const r = analyzeScopes(ast, { source: code, filename: 'tdz' });
  return r.bindings.filter((b) => b.name === 'x').some((b) => b.needsTdzCheck);
}

// Harness: returns the snippets where V8 faults but the analysis does not flag x (false negatives).
export function findUnsound(snippets, verdict) {
  return snippets.filter((s) => v8Faults(s.code) && !verdict(s.code)).map((s) => s.name);
}

const SNIPPETS = corpus().map((s) => ({ ...s, fault: v8Faults(s.code) }));

test('the corpus runs in V8 and contains real TDZ faults and safe reads', () => {
  assert.ok(SNIPPETS.length >= 30, `corpus size ${SNIPPETS.length}`);
  assert.ok(SNIPPETS.filter((s) => s.fault).length >= 10, 'enough real faults');
  assert.ok(SNIPPETS.filter((s) => !s.fault).length >= 10, 'enough safe snippets');
});

test('soundness: every snippet that faults at run time in V8 is flagged by needsTdzCheck (no false negatives)', (t) => {
  const faults = SNIPPETS.filter((s) => s.fault).length;
  t.diagnostic(`V8 snippets: ${SNIPPETS.length}, real TDZ faults: ${faults}, false negatives: ${findUnsound(SNIPPETS, analysisFlags).length}`);
  assert.deepEqual(findUnsound(SNIPPETS, analysisFlags), []);
});

test('false positives are bounded: the analysis flags some safe snippets but not all of them', (t) => {
  const safe = SNIPPETS.filter((s) => !s.fault);
  const falsePositives = safe.filter((s) => analysisFlags(s.code)).map((s) => s.name);
  const cleared = safe.length - falsePositives.length;
  t.diagnostic(`safe snippets: ${safe.length}, conservative false positives: ${falsePositives.length}, cleared: ${cleared}`);
  assert.ok(falsePositives.length >= 1, 'the conservative rule is exercised by at least one safe snippet');
  assert.ok(cleared >= 10, `${cleared} safe snippets are cleared by the analysis`);
});

test('negative control: an analysis that never flags TDZ is detected as unsound', () => {
  const neverFlags = () => false;
  const missed = findUnsound(SNIPPETS, neverFlags);
  assert.ok(missed.length >= 10, `harness reports ${missed.length} missed faults`);
});

test('negative control: flagging only textually-earlier reads misses closure faults, and the harness reports them', () => {
  const textualOnly = (code) => {
    const ast = parseRecovering(code);
    const r = analyzeScopes(ast, { source: code, filename: 'tdz' });
    return r.bindings.filter((b) => b.name === 'x').some((b) => b.needsTdzCheck && b.captured === false);
  };
  const missed = findUnsound(SNIPPETS, textualOnly);
  assert.ok(missed.some((n) => n.startsWith('closure-before-call-before-init')), `missed: ${missed.join(', ')}`);
});

test('var bindings are never flagged for a TDZ check, and V8 never faults on them', () => {
  const vars = SNIPPETS.filter((s) => s.name.endsWith('/var'));
  assert.ok(vars.length >= 15, `${vars.length} var snippets`);
  for (const s of vars) {
    assert.equal(s.fault, false, s.name);
    assert.equal(analysisFlags(s.code), false, s.name);
  }
});

test('a snippet whose x is only read after initialization is never a fault and not an unsound case', () => {
  const s = SNIPPETS.find((item) => item.name === 'read-after-init/let');
  assert.equal(s.fault, false);
  assert.equal(findUnsound([s], analysisFlags).length, 0);
});

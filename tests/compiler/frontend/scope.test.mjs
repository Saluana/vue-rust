// Scope analysis: hoisting, closures, captured and mutated variables, TDZ, per-iteration copies, shadowing,
// duplicates, exports, this/arguments, globals. Expected values are derived by hand from the source text.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { analyzeScopes, describeScopes } from '../../../compiler/analysis/scope.mjs';
import { parseRecovering, plainResult, codesAndSpans } from './helpers.mjs';

function analyze(source, { ts = false } = {}) {
  return analyzeScopes(parseRecovering(source, { ts }), { source, filename: 'scope-case' });
}

function bindingsNamed(r, name) {
  return r.bindings.filter((b) => b.name === name);
}

function one(r, name) {
  const found = bindingsNamed(r, name);
  assert.equal(found.length, 1, `expected one binding named ${name}, found ${found.length}`);
  return found[0];
}

function refsNamed(r, name) {
  return r.references.filter((ref) => ref.name === name);
}

test('module scope is the root and has id 0 with no parent', () => {
  const r = analyze('let a = 1;');
  assert.equal(r.module.id, 0);
  assert.equal(r.module.kind, 'module');
  assert.equal(r.module.parentId, -1);
});

test('var hoisting: a read before "var v" resolves to the var and is not a TDZ read', () => {
  const r = analyze('console.log(v);\nvar v = 1;\n');
  const v = one(r, 'v');
  assert.equal(v.kind, 'var');
  assert.equal(v.needsTdzCheck, false);
  assert.equal(refsNamed(r, 'v')[0].binding, v.id);
});

test('function declarations are hoisted: a call before the declaration resolves without TDZ', () => {
  const r = analyze('g();\nfunction g() {}\n');
  const g = one(r, 'g');
  assert.equal(g.kind, 'function');
  assert.equal(g.needsTdzCheck, false);
  assert.equal(refsNamed(r, 'g')[0].binding, g.id);
});

test('a textual read before "let x" needs a TDZ check', () => {
  const r = analyze('x;\nlet x = 1;');
  assert.equal(one(r, 'x').needsTdzCheck, true);
});

test('a read after "let y" does not need a TDZ check', () => {
  const r = analyze('let y = 1;\ny;');
  assert.equal(one(r, 'y').needsTdzCheck, false);
});

test('self-initialization "let z = z + 1" needs a TDZ check', () => {
  const r = analyze('let z = z + 1;');
  assert.equal(one(r, 'z').needsTdzCheck, true);
});

test('a closure created before the initializer and called after still needs a TDZ check (conservative)', () => {
  const r = analyze('const f = () => r;\nlet r = 1;\nf();');
  const b = one(r, 'r');
  assert.equal(b.needsTdzCheck, true);
  assert.equal(b.captured, true);
});

test('a closure created after the initializer does not need a TDZ check', () => {
  const r = analyze('let q = 1;\nconst f = () => q;\nf();');
  const b = one(r, 'q');
  assert.equal(b.captured, true);
  assert.equal(b.needsTdzCheck, false);
});

test('a hoisted function that reads a later let needs a TDZ check', () => {
  const r = analyze('function h() { return w; }\nh();\nlet w = 1;');
  assert.equal(one(r, 'w').needsTdzCheck, true);
});

test('captured and mutated: a closure that writes a let marks it captured and mutated', () => {
  const r = analyze('let c = 0;\nconst inc = () => { c++; };\ninc();');
  const c = one(r, 'c');
  assert.equal(c.captured, true);
  assert.equal(c.mutated, true);
  assert.equal(c.needsTdzCheck, false);
  const inc = one(r, 'inc');
  assert.equal(inc.captured, false);
  assert.equal(inc.mutated, false);
});

test('a let that is only updated locally is mutated but not captured', () => {
  const r = analyze('for (let k = 0; k < 2; k++) { k; }');
  const k = one(r, 'k');
  assert.equal(k.mutated, true);
  assert.equal(k.captured, false);
  assert.equal(k.perIterationCopy, false);
});

test('per-iteration copy: a C-style for-let captured by a closure needs a fresh cell per iteration', () => {
  const r = analyze('const fns = [];\nfor (let i = 0; i < 3; i++) { fns.push(() => i); }');
  const i = one(r, 'i');
  assert.equal(i.perIterationCopy, true);
  assert.equal(i.captured, true);
  assert.equal(i.mutated, true);
});

test('per-iteration copy: a for-of const captured by a closure gets a fresh binding per iteration, not mutated', () => {
  const r = analyze('const fns = [];\nfor (const v of [1, 2]) { fns.push(() => v); }');
  const v = one(r, 'v');
  assert.equal(v.perIterationCopy, true);
  assert.equal(v.mutated, false);
});

test('per-iteration copy is not requested for a const C-style head', () => {
  const r = analyze('const fns = [];\nfor (const k = 0; false;) { fns.push(() => k); }');
  assert.equal(one(r, 'k').perIterationCopy, false);
});

test('shadowing: the inner let resolves its own references and the outer one resolves outside', () => {
  const r = analyze('let s = 1;\n{ let s = 2; s; }\ns;');
  const [outer, inner] = bindingsNamed(r, 's');
  assert.equal(bindingsNamed(r, 's').length, 2);
  const refs = refsNamed(r, 's');
  assert.equal(refs[0].binding, inner.id);
  assert.equal(refs[1].binding, outer.id);
  assert.notEqual(inner.scopeId, outer.scopeId);
});

test('catch parameter is a catch binding and its reference resolves to it', () => {
  const r = analyze('try { f(); } catch (err) { err; }');
  const err = one(r, 'err');
  assert.equal(err.kind, 'catch');
  assert.equal(refsNamed(r, 'err')[0].binding, err.id);
});

test('destructuring introduces one const binding per identifier, and defaults are reads', () => {
  const r = analyze('const { a, b: [c, d = 2] } = obj;');
  const names = r.bindings.map((b) => b.name).sort();
  assert.deepEqual(names, ['a', 'c', 'd']);
  assert.ok(r.bindings.every((b) => b.kind === 'const'));
});

test('an import binding is live, immutable and always marked for a TDZ check (cyclic graphs)', () => {
  const r = analyze("import { ref as r } from 'vue';\nr(1);");
  const b = one(r, 'r');
  assert.equal(b.kind, 'import');
  assert.equal(b.importedName, 'ref');
  assert.equal(b.source, 'vue');
  assert.equal(b.needsTdzCheck, true);
  assert.equal(refsNamed(r, 'r')[0].binding, b.id);
});

test('typeof of an undeclared name creates a global reference with no diagnostic', () => {
  const r = analyze("if (typeof nope === 'undefined') {}");
  assert.deepEqual(r.globalsUsed, ['nope']);
  const ref = refsNamed(r, 'nope')[0];
  assert.equal(ref.typeofOnly, true);
  assert.equal(ref.binding, -1);
  assert.deepEqual(r.diagnostics, []);
});

test('assignment to a const is a VN5002 warning at the assigned name', () => {
  const r = analyze('const k = 1;\nk = 2;');
  assert.deepEqual(codesAndSpans(r.diagnostics), ['VN5002 2:0-2:1']);
  assert.equal(r.diagnostics[0].severity, 'warning');
  assert.equal(one(r, 'k').mutated, true);
});

test('assignment to an imported binding is a VN5003 warning', () => {
  const r = analyze("import { a } from 'm';\na = 1;");
  assert.deepEqual(codesAndSpans(r.diagnostics), ['VN5003 2:0-2:1']);
  assert.equal(r.diagnostics[0].severity, 'warning');
});

test('assignment to a named function expression name is a VN5004 warning', () => {
  const r = analyze('const h = function k() { k = 1; };');
  assert.deepEqual(codesAndSpans(r.diagnostics), ['VN5004 1:25-1:26']);
});

test('a reference to a named function expression name resolves inside the function without a diagnostic', () => {
  const r = analyze('const f = function g() { return g; };');
  const g = one(r, 'g');
  assert.equal(g.kind, 'function');
  assert.equal(refsNamed(r, 'g')[0].binding, g.id);
  assert.deepEqual(r.diagnostics, []);
});

test('duplicate lexical declaration is a VN5001 error at the second name', () => {
  const r = analyze('let d = 1; let d = 2;');
  assert.deepEqual(codesAndSpans(r.diagnostics), ['VN5001 1:15-1:16']);
  assert.equal(r.diagnostics[0].severity, 'error');
});

test('a var hoisted out of a block conflicts with a let of the same name in that block', () => {
  const r = analyze('{ let t = 1; { var t = 2; } }');
  assert.deepEqual(codesAndSpans(r.diagnostics), ['VN5001 1:19-1:20']);
});

test('a let that repeats a parameter name is a VN5001 error', () => {
  const r = analyze('function f(a) { let a; }');
  assert.deepEqual(codesAndSpans(r.diagnostics), ['VN5001 1:20-1:21']);
});

test('a var may redeclare a parameter (no conflict)', () => {
  const r = analyze('function f(a) { var a = 2; return a; }');
  assert.deepEqual(r.diagnostics, []);
  assert.equal(bindingsNamed(r, 'a').length, 1);
});

test('arrow functions use the enclosing function as their lexical this; module-level this is the module', () => {
  const r = analyze('function outer() { return () => this.x; }\nconst top = () => this;');
  const [outerFn, arrowInside, topArrow] = r.functions;
  assert.equal(outerFn.name, 'outer');
  assert.equal(outerFn.capturesThis, true);
  assert.equal(outerFn.usesThis, false);
  assert.equal(arrowInside.isArrow, true);
  assert.equal(arrowInside.usesThis, true);
  assert.equal(arrowInside.lexicalThisFnId, outerFn.id);
  assert.equal(topArrow.lexicalThisFnId, -1);
  assert.equal(r.moduleUsesThis, true);
});

test('module-level this is absent when no code uses it', () => {
  assert.equal(analyze('const a = 1;').moduleUsesThis, false);
});

test('implicit arguments: a non-arrow function that reads arguments gets an implicit binding', () => {
  const r = analyze('function f() { return arguments[0]; }');
  const args = one(r, 'arguments');
  assert.equal(args.kind, 'implicit');
  assert.equal(r.functions[0].usesArguments, true);
  assert.deepEqual(r.globalsUsed, []);
});

test('arguments at module level is an unresolved global', () => {
  const r = analyze('arguments;');
  assert.deepEqual(r.globalsUsed, ['arguments']);
});

test('a switch-case let is marked for a TDZ check even when read in the same case group', () => {
  const r = analyze('switch (k) { case 0: let s = 1; break; case 1: s; }');
  const s = one(r, 's');
  assert.equal(s.inSwitch, true);
  assert.equal(s.needsTdzCheck, true);
});

test('exports: local names, renamed exports and exported declarations are recorded on the binding', () => {
  const r = analyze('export const a = 1;\nconst b = 2;\nexport { b as c };\nexport function f() {}');
  assert.deepEqual(one(r, 'a').exported, ['a']);
  assert.deepEqual(one(r, 'b').exported, ['c']);
  assert.deepEqual(one(r, 'f').exported, ['f']);
});

test('a parameter default that reads a later parameter needs a TDZ check; reading an earlier one does not', () => {
  const late = analyze('function f(a = b, b) {}');
  assert.equal(one(late, 'b').needsTdzCheck, true);
  const early = analyze('function g(a, b = a) {}');
  assert.equal(one(early, 'a').needsTdzCheck, false);
});

test('for-of head bindings are in TDZ while the iterable is evaluated', () => {
  const r = analyze('for (const k of k) {}');
  assert.equal(one(r, 'k').needsTdzCheck, true);
});

test('destructuring assignment and compound assignment mark their targets as mutated', () => {
  const r = analyze('let a = 1, b = 2;\n({ a, b } = { a: 3, b: 4 });\nlet n = 0;\nn += 2;');
  assert.equal(one(r, 'a').mutated, true);
  assert.equal(one(r, 'b').mutated, true);
  assert.equal(one(r, 'n').mutated, true);
});

test('a parameter that is reassigned is mutated and not captured', () => {
  const r = analyze('function p(x) { x = 2; return x; }');
  const x = one(r, 'x');
  assert.equal(x.kind, 'param');
  assert.equal(x.mutated, true);
  assert.equal(x.captured, false);
});

test('a captured variable two function levels deep is captured', () => {
  const r = analyze('let deep = 1;\nfunction a() { return function b() { return deep; }; }');
  assert.equal(one(r, 'deep').captured, true);
});

test('block-level function declarations are block scoped in strict code', () => {
  const r = analyze('{ function inner() {} }\ninner();');
  assert.deepEqual(r.globalsUsed, ['inner']);
});

test('a function expression name does not leak to the enclosing scope', () => {
  const r = analyze('const f = function g() {};\ng;');
  assert.deepEqual(r.globalsUsed, ['g']);
});

test('a class declaration is bound like let (classes are rejected by the profile, not by scope analysis)', () => {
  const r = analyze('class K {}\nnew K();');
  assert.equal(one(r, 'K').kind, 'let');
});

test('TypeScript enum and type-only declarations are handled without crashing or extra references', () => {
  const r = analyze('enum E { A }\nconst v: E = E.A;\ninterface I { x: number }\ntype T = I;', { ts: true });
  assert.equal(one(r, 'E').kind, 'let');
  assert.deepEqual(r.globalsUsed, []);
  assert.deepEqual(r.diagnostics, []);
});

test('the result is JSON-serializable without cycles and round-trips exactly', () => {
  const r = analyze('let c = 0;\nconst inc = () => { c++; };\nfor (let i of [1]) { inc(); }\nexport { c };');
  const plain = plainResult(r);
  assert.deepEqual(plain, JSON.parse(JSON.stringify(plain)));
  assert.equal(plain.scopes[plain.module.id].kind, 'module');
  for (const b of plain.bindings) assert.equal(b.id, plain.bindings.indexOf(b));
});

test('describeScopes lists scopes, bindings, references and globals', () => {
  const r = analyze('let s = 1;\nconsole.log(s);');
  const text = describeScopes(r);
  assert.match(text, /scope 0 module/);
  assert.match(text, /let s captured=false mutated=false tdz=false/);
  assert.match(text, /ref s@\d+ read -> binding#0/);
  assert.match(text, /globals: console/);
});

test('globalsUsed is sorted and unique', () => {
  const r = analyze('zeta(); alpha(); zeta(); alpha();');
  assert.deepEqual(r.globalsUsed, ['alpha', 'zeta']);
});

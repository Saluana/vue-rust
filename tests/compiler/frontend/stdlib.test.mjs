// Standard-library manifest checks (VN4016 globals, VN4017 members) through checkSource(..., { stdlib: true }).
// Spans are hand-counted (1-based lines, 0-based columns, end exclusive). Hints and statuses come from the manifest
// (compiler/profile/stdlib.json, read through stdlib.mjs). The manifest is generated and can change, so a precondition
// test pins the statuses these cases rely on; Math.random and Date.now are judged from whatever status they have.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compileModule } from '../../../compiler/index.mjs';
import { checkSource } from '../../../compiler/frontend/check.mjs';
import { lookupStdlib, isStdlibSupported } from '../../../compiler/profile/stdlib.mjs';
import { spanText } from './helpers.mjs';

function diagnose(source, { lang = 'js', target = 'native', stdlib = true } = {}) {
  return checkSource(source, { filename: `stdlib-case.${lang}`, lang, target, stdlib }).diagnostics;
}

const lines = (diags) => diags.map((d) => `${d.code} ${spanText(d)}`);
const manifestHint = (kind, path) => lookupStdlib(kind, path).hint;

test('precondition: the manifest statuses these cases rely on are the expected ones', () => {
  const expect = [
    ['global', 'console', 'X'], ['ctor', 'Proxy', 'X'], ['ctor', 'RegExp', 'E'], ['global', 'globalThis', 'X'],
    ['ctor', 'Int8Array', 'X'], ['ctor', 'Date', 'E'], ['global', 'Reflect', 'X'], ['global', 'window', 'X'],
    ['global', 'document', 'X'], ['global', 'eval', 'X'], ['global', 'trace', 'A'], ['global', 'testHost', 'A'],
    ['static', 'Object.defineProperty', 'X'], ['static', 'Object.setPrototypeOf', 'X'],
    ['proto', 'Array.prototype.copyWithin', 'X'], ['symbol', 'Symbol.dispose', 'X'],
    ['const', 'Math.PI', 'P'], ['static', 'Object.keys', 'P'], ['static', 'Array.isArray', 'P'],
    ['ctor', 'Map', 'P'], ['ctor', 'Symbol', 'P'], ['ctor', 'Object', 'P'], ['global', 'Math', 'P'],
  ];
  for (const [kind, path, status] of expect) {
    assert.equal(lookupStdlib(kind, path).status, status, `${kind} ${path}`);
  }
  assert.equal(lookupStdlib('global', 'trace').targets.join(','), 'test');
  assert.equal(lookupStdlib('static', 'Reflect.get'), null, 'Reflect.get is not listed');
});

// ---- globals (VN4016) ----

test('supported globals and members pass: Math.PI, Object.keys, Array.isArray, Map', () => {
  assert.deepEqual(lines(diagnose('const n = Math.PI;\nconst ok = Object.keys({});\nvoid [n, ok, Array.isArray([])];')), []);
  assert.deepEqual(lines(diagnose('const m = new Map();\nvoid m;')), []);
});

test('an excluded global (console) is VN4016 at its first use, with the manifest hint', () => {
  const d = diagnose('console.log(1);');
  assert.deepEqual(lines(d), ['VN4016 1:0-1:7']);
  assert.equal(d[0].message, "global 'console' is not available in the native profile");
  assert.equal(d[0].hint, manifestHint('global', 'console'));
});

test('each name is reported once, at its first unresolved use', () => {
  assert.deepEqual(lines(diagnose('console.log(1); console.log(2);')), ['VN4016 1:0-1:7']);
});

test('an excluded constructor used as a value (Proxy) is VN4016', () => {
  assert.deepEqual(lines(diagnose('const p = Proxy;')), ['VN4016 1:10-1:15']);
});

test('an experimental global (RegExp, status E) is VN4016 and reports the manifest hint', () => {
  const d = diagnose('const r = RegExp;');
  assert.deepEqual(lines(d), ['VN4016 1:10-1:16']);
  assert.equal(d[0].hint, manifestHint('ctor', 'RegExp'));
});

test('globalThis (X) is VN4016', () => {
  assert.deepEqual(lines(diagnose('const g = globalThis;')), ['VN4016 1:10-1:20']);
});

test('an unknown name that is not in the manifest is VN4016 with the generic hint', () => {
  const d = diagnose('const u = undeclaredThing;');
  assert.deepEqual(lines(d), ['VN4016 1:10-1:25']);
  assert.equal(d[0].message, "global 'undeclaredThing' is not available in the native profile");
  assert.equal(d[0].hint, "Declare 'undeclaredThing' in this module or import it from a supported module.");
});

test('new of an excluded constructor (Date, status E) is VN4016 only: the VN2007 finding on the same span is superseded', () => {
  assert.deepEqual(lines(diagnose('const d = new Date();')), ['VN4016 1:14-1:18']);
});

test('new of a typed array outside the profile (Int8Array) is VN4016 at the callee', () => {
  assert.deepEqual(lines(diagnose('const b = new Int8Array(4);')), ['VN4016 1:14-1:23']);
});

test('new of a supported but non-constructible global (Symbol) keeps VN2007, since no global is missing', () => {
  assert.deepEqual(lines(diagnose("const s = new Symbol('x');")), ['VN2007 1:14-1:20']);
});

test('a global used only as a typeof operand is exempt; a plain use of the same name is still reported', () => {
  assert.deepEqual(lines(diagnose("if (typeof window === 'undefined') {}")), []);
  assert.deepEqual(lines(diagnose('typeof Proxy;\nconst p = Proxy;')), ['VN4016 2:10-2:15']);
});

test('a plain use of window is VN4016', () => {
  assert.deepEqual(lines(diagnose('const x = window;')), ['VN4016 1:10-1:16']);
});

test('document on the web target is still VN4016 (no browser DOM in the manifest)', () => {
  assert.deepEqual(lines(diagnose('document.title;', { target: 'web' })), ['VN4016 1:0-1:8']);
});

test('test-only host globals: trace passes on the test target and is VN4016 on native with its target hint', () => {
  assert.deepEqual(lines(diagnose("trace('a', 1);", { target: 'test' })), []);
  const d = diagnose("trace('a', 1);", { target: 'native' });
  assert.deepEqual(lines(d), ['VN4016 1:0-1:5']);
  assert.equal(d[0].hint, 'trace is only available on the test target.');
});

test('testHost passes on the test target and is VN4016 on the web target', () => {
  assert.deepEqual(lines(diagnose('testHost.run();', { target: 'test' })), []);
  assert.deepEqual(lines(diagnose('testHost.run();', { target: 'web' })), ['VN4016 1:0-1:8']);
});

test('a shadowed global (a const named console) is ignored', () => {
  assert.deepEqual(lines(diagnose('const console = { log() {} };\nconsole.log(1);')), []);
});

test('arguments at module level keeps VN2015; the generic unknown-global finding on the same span is dropped', () => {
  assert.deepEqual(lines(diagnose('arguments.length;')), ['VN2015 1:0-1:9']);
});

test('new Proxy(...) reports only VN4016 on the constructor name (VN2007 is superseded on the same span)', () => {
  assert.deepEqual(lines(diagnose('const q = new Proxy(1);')), ['VN4016 1:14-1:19']);
});

// ---- members (VN4017) ----

test('Object.defineProperty on a declared object is VN4017 with the manifest hint', () => {
  const d = diagnose("const o = {};\nObject.defineProperty(o, 'k', {});");
  assert.deepEqual(lines(d), ['VN4017 2:0-2:21']);
  assert.equal(d[0].message, "'Object.defineProperty' is not supported");
  assert.equal(d[0].hint, manifestHint('static', 'Object.defineProperty'));
});

test('Object.setPrototypeOf is VN4017 (the legacy VN4008 is not reported on top)', () => {
  assert.deepEqual(lines(diagnose('const o = {};\nObject.setPrototypeOf(o, null);')), ['VN4017 2:0-2:21']);
});

test('Array.prototype.copyWithin is VN4017 for the whole member path', () => {
  assert.deepEqual(lines(diagnose('Array.prototype.copyWithin.call([1], 0, 0);')), ['VN4017 1:0-1:26']);
});

test('Symbol.dispose is VN4017 (the legacy VN4015 is not reported on top)', () => {
  assert.deepEqual(lines(diagnose('const d = Symbol.dispose;')), ['VN4017 1:10-1:24']);
});

test('Reflect.get is not listed, so an excluded root (Reflect) is the only finding', () => {
  // Reflect is X, and Reflect.get is not in the manifest, so only the root is reported.
  assert.deepEqual(lines(diagnose("Reflect.get({}, 'k');")), ['VN4016 1:0-1:7']);
});

test('an unlisted member of a supported root is not judged by this check', () => {
  assert.deepEqual(lines(diagnose('Math.foo();')), []);
});

test('a computed member is ignored', () => {
  assert.deepEqual(lines(diagnose("Object['defineProperty']({}, 'k', {});")), []);
});

test('an optional member chain is judged like a plain one', () => {
  assert.deepEqual(lines(diagnose("const o = {};\nObject?.defineProperty(o, 'k', {});")), ['VN4017 2:0-2:22']);
});

test('a member on this is ignored', () => {
  assert.deepEqual(lines(diagnose('function f() { return this.Math.random(); }\nvoid f;')), []);
});

test('a shadowed object (const Object = {...}) makes Object.keys and Object.defineProperty user members', () => {
  assert.deepEqual(lines(diagnose('const Object = { keys: () => 1 };\nconst k = Object.keys();\nvoid k;')), []);
  assert.deepEqual(lines(diagnose("const Object = { defineProperty() {} };\nObject.defineProperty(1, 'k', {});")), []);
});

test('a parameter that shadows Math makes Math.random a user member', () => {
  assert.deepEqual(lines(diagnose('function f(Math) { return Math.random(); }\nvoid f;')), []);
});

test('Math.random follows its manifest status: VN4017 when excluded, no finding when supported', () => {
  const supported = isStdlibSupported('Math.random', 'native');
  const expected = supported ? [] : ['VN4017 1:0-1:11'];
  assert.deepEqual(lines(diagnose('Math.random();')), expected);
});

test('Date.now follows its manifest status on top of the excluded Date root (VN4016 on Date always)', () => {
  const supported = isStdlibSupported('Date.now', 'native');
  const expected = supported ? ['VN4016 1:0-1:4'] : ['VN4016 1:0-1:4', 'VN4017 1:0-1:8'];
  assert.deepEqual(lines(diagnose('Date.now();')), expected);
});

// ---- lang ts ----

test('ts: a member of a declared object is VN4017; type annotations and type arguments are not references', () => {
  assert.deepEqual(lines(diagnose("const o = {} as Record<string, number>;\nObject.defineProperty(o, 'k', {});", { lang: 'ts' })), ['VN4017 2:0-2:21']);
  assert.deepEqual(lines(diagnose('const xs: Array<number> = [];\nconst n: number = Math.PI;\nvoid [xs, n];', { lang: 'ts' })), []);
  assert.deepEqual(lines(diagnose('const m = new Map<string, number>();\nvoid m;', { lang: 'ts' })), []);
});

test('ts: an angle-bracket assertion is erased and its expression is still checked', () => {
  assert.deepEqual(lines(diagnose('const d = <number>Math.PI;\nvoid d;', { lang: 'ts' })), []);
  assert.deepEqual(lines(diagnose('const d = <number>Object.setPrototypeOf;\nvoid d;', { lang: 'ts' })), ['VN4017 1:18-1:39']);
  const supported = isStdlibSupported('Math.random', 'native');
  const expected = supported ? [] : ['VN4017 1:18-1:29'];
  assert.deepEqual(lines(diagnose('const d = <number>Math.random();\nvoid d;', { lang: 'ts' })), expected);
});

// ---- the option is opt-in: callers that do not pass it keep the existing behaviour ----

test('without the stdlib option, the legacy codes are reported and console is not judged', () => {
  assert.deepEqual(lines(diagnose("Object.defineProperty({}, 'k', {});", { stdlib: false })), ['VN4008 1:0-1:21']);
  assert.deepEqual(lines(diagnose("eval('1');", { stdlib: false })), ['VN4001 1:0-1:4']);
  assert.deepEqual(lines(diagnose('console.log(1);', { stdlib: false })), []);
});

test('target test is accepted and a supported global passes on it', () => {
  assert.deepEqual(lines(diagnose('const n = Math.PI;\nvoid n;', { target: 'test' })), []);
});

test('standalone compilation enforces the stdlib manifest before producing Rust', () => {
  for (const target of ['native', 'web', 'test']) {
    const result = compileModule('export const value = structuredClone({a: 1});', { specifier: '/app.js', target });
    assert.equal(result.rust, null);
    assert.ok(result.diagnostics.some(d => d.code === 'VN4016' && d.message.includes('structuredClone')));
  }
});

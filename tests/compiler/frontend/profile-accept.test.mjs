// Counter-examples: constructs the profile accepts must produce no frontend diagnostics (no over-rejection).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { checkSource } from '../../../compiler/frontend/check.mjs';

function expectClean(source, lang = 'js', target = 'native') {
  const r = checkSource(source, { filename: 'accept.' + lang, lang, target });
  assert.deepEqual(r.diagnostics.map((d) => `${d.code} ${d.start.line}:${d.start.column} ${d.message}`), []);
}

test('import { ref } from vue is accepted and a named import can be called', () => {
  expectClean("import { ref } from 'vue';\nconst r = ref(0);\nr.value;");
});

test('Map, Set, Promise, Error and TypeError constructors are accepted', () => {
  expectClean(
    "const m = new Map(); const s = new Set([1]); const p = new Promise((res) => res(1));\n" +
    "const e = new Error('x'); const t = new TypeError('y'); void [m, s, p, e, t];",
  );
});

test('Symbol(...) is accepted as a call (Symbol is not constructible, so new Symbol is rejected)', () => {
  expectClean("const sym = Symbol('desc'); void sym;");
});

test('typed IO constructors Uint8Array and ArrayBuffer are accepted', () => {
  expectClean('const u8 = new Uint8Array(4); const buf = new ArrayBuffer(8); void [u8, buf];');
});

test('a new of an imported binding is accepted', () => {
  expectClean("import { Widget } from './widget.js';\nconst w = new Widget();\nvoid w;");
});

test('object literals with accessors, methods and spread are accepted', () => {
  expectClean(
    'const base = { a: 1 };\nconst o = { ...base, get x() { return 1; }, set x(v) {}, m() { return 2; } };\nvoid o;',
  );
});

test('computed __proto__ keys and __proto (not __proto__) are accepted', () => {
  expectClean("const k = 1;\nconst o = { ['__proto__']: 3, __proto: 2, ['a' + k]: 1 };\nvoid o;");
});

test('shorthand __proto__ creates an own property and is accepted', () => {
  expectClean('const __proto__ = 1;\nconst o = { __proto__ };\nvoid o;');
});

test('async functions, await in functions, and optional chaining are accepted', () => {
  expectClean('async function f() { const v = await g(); return v?.b ?? 0; }\nconst a = f;\nvoid a;');
});

test('arrow functions with default and rest parameters are accepted', () => {
  expectClean('const add = (a, b = 1, ...rest) => a + b + rest.length;\nvoid add;');
});

test('typeof of an undeclared name is accepted and not reported by the profile checker', () => {
  expectClean("const probe = typeof notDeclaredAnywhere === 'undefined';\nvoid probe;");
});

test('let/const shadowing a denied global name is accepted (the name is a local binding)', () => {
  expectClean('const Proxy = 1;\nconst Reflect = 2;\nconst eval2 = 3;\nvoid [Proxy, Reflect, eval2];');
});

test('a local function named require is accepted (it is not CommonJS)', () => {
  expectClean('function require(x) { return x; }\nconst r = require(1);\nvoid r;');
});

test('Object.create(null), Object.freeze and plain Object calls are accepted', () => {
  expectClean('const d = Object.create(null);\nconst f = Object.freeze({ a: 1 });\nvoid [d, f];');
});

test('var declared in a block is accepted (var is function-scoped, not block-scoped)', () => {
  expectClean('{ var q = 1; }\nvoid q;');
});

test('labels, switch, do-while and block-level function declarations are accepted', () => {
  expectClean(
    'outer: for (let i = 0; i < 2; i++) { switch (i) { case 0: break; default: break outer; } }\n' +
    'do { } while (false);\nif (true) { function inBlock() { return 1; } void inBlock(); }',
  );
});

test('named function expressions and array holes are accepted', () => {
  expectClean('const f = function named() { return named; };\nconst holes = [1, , 3];\nvoid [f, holes];');
});

test('untagged template literals and string methods are accepted', () => {
  expectClean('const n = 2;\nconst s = `a${n}b`.indexOf("b");\nvoid s;');
});

test('dynamic import with a literal specifier is accepted', () => {
  expectClean("import('./module.js').then(() => {});");
});

test('exports of local names, default function export and re-export clauses are accepted', () => {
  expectClean(
    'const x = 1;\nexport { x as y };\nexport default function () { return 2; }\nexport { z } from "./z.js";',
  );
});

test('document is accepted for the web target but rejected for native (see the reject fixture)', () => {
  expectClean('const t = document.title;\nvoid t;', 'js', 'web');
  const r = checkSource('const t = document.title;\nvoid t;', { lang: 'js', target: 'native' });
  assert.deepEqual(r.diagnostics.map((d) => d.code), ['VN4011']);
});

test('TypeScript type syntax is accepted and erased: annotations, interfaces, aliases, generics, as, satisfies, !', () => {
  expectClean(
    'type T = { a: number };\ninterface I { b: string }\nconst v: T = { a: 1 } as T;\n' +
    'const w = v satisfies T;\nconst z = v!;\nfunction id<U>(x: U): U { return x; }\nvoid [w, z, id];',
    'ts',
  );
});

test('TypeScript type-only imports, exports and declare statements are accepted', () => {
  expectClean(
    "import type { A } from './a';\nexport type { A };\ndeclare const ambient: number;\n" +
    'declare function g(): void;\nvoid ambient;',
    'ts',
  );
});

test('TypeScript overload signatures and declare class are accepted', () => {
  expectClean(
    'declare class Host { m(): void; }\nfunction over(a: string): string;\nfunction over(a: string) { return a; }\nvoid over;',
    'ts',
  );
});

test('an angle-bracket type assertion in ts is accepted and erased: only the expression is analysed', () => {
  const src = 'const a = <number>x;\nvoid a;';
  const r = checkSource(src, { filename: 'angle.ts', lang: 'ts' });
  assert.deepEqual(r.diagnostics, []);
  assert.equal(r.ast.program.body[0].declarations[0].init.type, 'TSTypeAssertion');
  assert.deepEqual(r.analysis.globalsUsed, ['x']);
  assert.equal(r.analysis.references.some((ref) => ref.name === 'number'), false);
});

// parseModule: exact spans for syntax errors, reason mapping to profile codes, lang handling, and no throws on input.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parseModule } from '../../../compiler/frontend/parse.mjs';
import { checkSource } from '../../../compiler/frontend/check.mjs';
import { spanText } from './helpers.mjs';

function failure(source, lang = 'js') {
  const r = parseModule(source, { filename: 'case', lang });
  assert.equal(r.ast, null, `expected a parse failure for ${JSON.stringify(source)}`);
  assert.equal(r.diagnostics.length, 1);
  return r.diagnostics[0];
}

test('a valid module returns the Babel File node and no diagnostics', () => {
  const r = parseModule('export const a = 1;\nconst b = a + 1;', { filename: 'ok.js' });
  assert.deepEqual(r.diagnostics, []);
  assert.equal(r.ast.type, 'File');
  assert.equal(r.ast.program.sourceType, 'module');
  assert.equal(r.ast.program.body.length, 2);
});

test('syntax error: "let x = ;" reports VN1000 on the offending semicolon (hand-computed 1:8-1:9)', () => {
  const d = failure('let x = ;');
  assert.equal(d.code, 'VN1000');
  assert.equal(spanText(d), '1:8-1:9');
  assert.equal(d.message, 'Unexpected token');
  assert.equal(d.file, 'case');
});

test('unterminated string: span runs from the quote to the end of its line (1:10-1:14)', () => {
  const d = failure('const s = "abc');
  assert.equal(d.code, 'VN1000');
  assert.equal(spanText(d), '1:10-1:14');
});

test('error at end of input gives a zero-length span at the end (1:14-1:14)', () => {
  const d = failure('function f() {');
  assert.equal(d.code, 'VN1000');
  assert.equal(spanText(d), '1:14-1:14');
});

test('a TypeScript annotation in a .js module is a syntax error at the colon (1:5-1:6)', () => {
  const d = failure('let x: number = 1;', 'js');
  assert.equal(d.code, 'VN1000');
  assert.equal(spanText(d), '1:5-1:6');
});

test('the same annotation is accepted in ts mode and erased by the type-only rules', () => {
  const r = parseModule('let x: number = 1;', { lang: 'ts' });
  assert.deepEqual(r.diagnostics, []);
  assert.equal(r.ast.program.body[0].type, 'VariableDeclaration');
});

test('an angle-bracket type assertion is accepted in ts mode and parsed as an erased type assertion', () => {
  const r = parseModule('const a = <number>x;', { lang: 'ts' });
  assert.deepEqual(r.diagnostics, []);
  assert.equal(r.ast.program.body[0].declarations[0].init.type, 'TSTypeAssertion');
});

test('an angle-bracket assertion in tsx mode is rejected with a hint to use "as"', () => {
  const d = failure('const a = <number>x;', 'tsx');
  assert.equal(d.code, 'VN1000');
  assert.match(d.hint, /value as Type/);
});

test('JSX in js is a VN1000 parse error whose message says JSX is not supported (span on the <)', () => {
  const d = failure('const a = <div />;', 'js');
  assert.equal(d.code, 'VN1000');
  assert.equal(d.message, "JSX is not supported in lang 'js'");
  assert.equal(d.hint, 'use a <template> in a .vue file');
  assert.equal(spanText(d), '1:10-1:11');
});

test('JSX in ts is a VN1000 parse error whose message says JSX is not supported (span on the slash)', () => {
  // Without the jsx plugin, "<div" is a type assertion and the "/" after it cannot start an expression.
  const d = failure('const a = <div/>;', 'ts');
  assert.equal(d.code, 'VN1000');
  assert.equal(d.message, "JSX is not supported in lang 'ts'");
  assert.equal(spanText(d), '1:14-1:15');
});

test('a non-JSX syntax error in ts keeps its own message', () => {
  const d = failure('let x: number = ;', 'ts');
  assert.equal(d.code, 'VN1000');
  assert.equal(d.message, 'Unexpected token');
  assert.equal(spanText(d), '1:16-1:17');
});

test('lang values are js, ts, jsx and tsx; anything else throws', () => {
  assert.throws(() => parseModule('x', { lang: 'vue' }), TypeError);
  assert.equal(parseModule('const a = 1;', { lang: 'tsx' }).diagnostics.length, 0);
});

test('sloppy-only "with" is reported as VN2001 by the parse mapping, not as a generic error', () => {
  const d = failure('with (o) { x }', 'ts');
  assert.equal(d.code, 'VN2001');
  assert.equal(spanText(d), '1:0-1:4');
});

test('duplicate declaration from the parser is reported as VN5001 with the identifier name', () => {
  const d = failure('let a; let a;');
  assert.equal(d.code, 'VN5001');
  assert.equal(d.message, "Identifier 'a' has already been declared");
  assert.equal(spanText(d), '1:11-1:12');
});

test('top-level await, import.meta and JSX parse (their rejection is the checker\'s job)', () => {
  assert.deepEqual(parseModule('const v = await p; const u = import.meta.url;').diagnostics, []);
  assert.deepEqual(parseModule('const a = <div />;', { lang: 'jsx' }).diagnostics, []);
  assert.deepEqual(parseModule('class A { @dec m() {} }').diagnostics, []);
});

test('lang must be js or ts (programmer error throws)', () => {
  assert.throws(() => parseModule('x', { lang: 'py' }), TypeError);
  assert.throws(() => parseModule(42), TypeError);
});

test('bad user input never throws: every malformed source yields a diagnostic', () => {
  const bad = ['{', ')', "'", '`', '\\', '#', '@@', '1 +', 'let', '}{', 'a ? b', '/*', 'x =>', '%%', '\u0000'];
  for (const src of bad) {
    const r = parseModule(src, { lang: 'js' });
    assert.equal(r.ast, null, JSON.stringify(src));
    assert.equal(r.diagnostics.length, 1, JSON.stringify(src));
    assert.match(r.diagnostics[0].code, /^VN\d{4}$/);
  }
});

test('checkSource returns no AST, no analysis and only the parse diagnostics for a syntax error', () => {
  const r = checkSource('let x = ;', { filename: 'bad.js', lang: 'js' });
  assert.equal(r.ast, null);
  assert.equal(r.analysis, null);
  assert.deepEqual(r.diagnostics.map((d) => d.code), ['VN1000']);
});

// Position model: UTF-16 offsets, 1-based lines, 0-based columns, ECMAScript line terminators, BOM, astral characters.
// Expected values are hand-computed; the Babel oracle compares against @babel/parser's own loc.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createLineMap, Diagnostic, createDiagnostic } from '../../../compiler/diagnostics/diagnostic.mjs';
import { CODES } from '../../../compiler/diagnostics/codes.mjs';
import { babel, spanText } from './helpers.mjs';
import { checkSource } from '../../../compiler/frontend/check.mjs';

const LS = String.fromCharCode(0x2028);
const PS = String.fromCharCode(0x2029);
const BOM = String.fromCharCode(0xfeff);
const COMBINING_ACUTE = String.fromCharCode(0x0301);
const GRINNING = String.fromCodePoint(0x1f600);

test('LF: line and column of each offset in ab\\ncd (hand-computed)', () => {
  const map = createLineMap('ab\ncd');
  assert.deepEqual(map.posAt(0), { offset: 0, line: 1, column: 0 });
  assert.deepEqual(map.posAt(1), { offset: 1, line: 1, column: 1 });
  assert.deepEqual(map.posAt(2), { offset: 2, line: 1, column: 2 });
  assert.deepEqual(map.posAt(3), { offset: 3, line: 2, column: 0 });
  assert.deepEqual(map.posAt(5), { offset: 5, line: 2, column: 2 });
  assert.equal(map.lineCount, 2);
});

test('CRLF counts as one terminator; the LF maps to the end of the line content', () => {
  const map = createLineMap('a\r\nb');
  assert.deepEqual(map.posAt(1), { offset: 1, line: 1, column: 1 });
  assert.deepEqual(map.posAt(2), { offset: 2, line: 1, column: 1 });
  assert.deepEqual(map.posAt(3), { offset: 3, line: 2, column: 0 });
  assert.equal(map.lineCount, 2);
});

test('lone CR is a line terminator', () => {
  const map = createLineMap('a\rb\rc');
  assert.deepEqual(map.posAt(2), { offset: 2, line: 2, column: 0 });
  assert.deepEqual(map.posAt(4), { offset: 4, line: 3, column: 0 });
  assert.equal(map.lineCount, 3);
});

test('U+2028 and U+2029 are line terminators as in ECMAScript', () => {
  const src = 'a' + LS + 'b' + PS + 'c';
  const map = createLineMap(src);
  assert.deepEqual(map.posAt(2), { offset: 2, line: 2, column: 0 });
  assert.deepEqual(map.posAt(4), { offset: 4, line: 3, column: 0 });
  assert.equal(map.lineCount, 3);
});

test('astral characters take two UTF-16 columns', () => {
  const src = GRINNING + 'x';
  const map = createLineMap(src);
  assert.deepEqual(map.posAt(2), { offset: 2, line: 1, column: 2 });
  assert.equal(map.offsetAt(1, 2), 2);
});

test('combining marks take one column each (no grapheme merging)', () => {
  const src = 'e' + COMBINING_ACUTE + 'x';
  assert.deepEqual(createLineMap(src).posAt(2), { offset: 2, line: 1, column: 2 });
});

test('a BOM is an ordinary code unit at offset 0 and counts as a column (matches Babel loc)', () => {
  const src = BOM + 'a';
  const map = createLineMap(src);
  assert.deepEqual(map.posAt(0), { offset: 0, line: 1, column: 0 });
  assert.deepEqual(map.posAt(1), { offset: 1, line: 1, column: 1 });
});

test('empty source has one line', () => {
  const map = createLineMap('');
  assert.deepEqual(map.posAt(0), { offset: 0, line: 1, column: 0 });
  assert.equal(map.lineCount, 1);
});

test('offsetAt inverts posAt and validates line and column ranges', () => {
  const map = createLineMap('ab\ncd');
  assert.equal(map.offsetAt(2, 1), 4);
  assert.equal(map.offsetAt(1, 2), 2, 'column equal to the line content length is valid');
  assert.throws(() => map.offsetAt(1, 3), RangeError);
  assert.throws(() => map.offsetAt(3, 0), RangeError);
  assert.throws(() => map.offsetAt(0, 0), RangeError);
});

test('posAt rejects offsets outside the source', () => {
  const map = createLineMap('ab');
  assert.throws(() => map.posAt(-1), RangeError);
  assert.throws(() => map.posAt(3), RangeError);
  assert.throws(() => map.posAt(1.5), RangeError);
});

test('createLineMap rejects non-string input (programmer error)', () => {
  assert.throws(() => createLineMap(null), TypeError);
});

test('round trip posAt -> offsetAt holds for every offset of a mixed corpus without CRLF', () => {
  const src = `${BOM}let a = "${GRINNING}";${LS}x${COMBINING_ACUTE}\rnext${PS}\nlast`;
  const map = createLineMap(src);
  for (let i = 0; i <= src.length; i++) {
    const p = map.posAt(i);
    assert.equal(map.offsetAt(p.line, p.column), i, `offset ${i}`);
  }
});

test('Babel oracle: identifier positions agree with @babel/parser loc for mixed terminators', () => {
  const src = [
    BOM + 'let first = 1;',
    'let second = "' + GRINNING + '";\r\n',
    'let third' + COMBINING_ACUTE + ' = 3;\r',
    'let fourth = 4;' + LS,
    'let fifth = 5;' + PS + 'let sixth = 6;',
    '// comment with a ' + GRINNING + ' emoji\nlet seventh = 7;',
  ].join('\n');
  const ast = babel.parse(src, { sourceType: 'module', errorRecovery: false, ranges: true, plugins: ['jsx'] });
  const map = createLineMap(src);
  let checked = 0;
  for (const stmt of ast.program.body) {
    const id = stmt.declarations[0].id;
    const p = map.posAt(id.start);
    assert.equal(p.offset, id.start);
    assert.equal(p.line, id.loc.start.line, `line of ${id.name}`);
    assert.equal(p.column, id.loc.start.column, `column of ${id.name}`);
    checked++;
  }
  assert.equal(checked, 7);
});

test('Diagnostic rejects spans that are not Pos objects (programmer error)', () => {
  const pos = { offset: 0, line: 1, column: 0 };
  assert.throws(() => new Diagnostic({ code: 'VN1000', message: 'x', start: pos, end: { offset: 1 } }), TypeError);
  assert.throws(() => new Diagnostic({ code: 'VN1000', message: 'x', start: pos, end: { offset: -1, line: 1, column: 0 } }), TypeError);
});

test('Diagnostic rejects an end before its start', () => {
  const a = { offset: 5, line: 1, column: 5 };
  const b = { offset: 2, line: 1, column: 2 };
  assert.throws(() => new Diagnostic({ code: 'VN1000', message: 'x', start: a, end: b }), RangeError);
});

test('createDiagnostic falls back to the registry title and hint when none are given', () => {
  const pos = { offset: 0, line: 1, column: 0 };
  const d = createDiagnostic('VN2001', { start: pos, end: pos });
  assert.equal(d.message, CODES.VN2001.title);
  assert.equal(d.hint, CODES.VN2001.hint);
  assert.equal(d.severity, 'error');
});

test('a diagnostic after an astral character reports UTF-16 columns (the emoji takes two units)', () => {
  // 'const s = "' is 11 units, the emoji takes units 11-12, then '"; ' ends at 15, so "with" starts at column 16.
  const src = 'const s = "' + String.fromCodePoint(0x1f600) + '"; with (o) {}';
  const r = checkSource(src, { filename: 'astral.js', lang: 'js' });
  assert.deepEqual(r.diagnostics.map((d) => `${d.code} ${spanText(d)}`), ['VN2001 1:16-1:20']);
});

test('a diagnostic on a line after a CRLF is reported on the right line and column', () => {
  // Line 1 is 'const a = 1;' followed by CRLF; line 2 starts with 'with' at column 0.
  const src = 'const a = 1;\r\nwith (o) {}';
  const r = checkSource(src, { filename: 'crlf.js', lang: 'js' });
  assert.deepEqual(r.diagnostics.map((d) => `${d.code} ${spanText(d)}`), ['VN2001 2:0-2:4']);
  assert.equal(r.diagnostics[0].start.offset, 14);
});

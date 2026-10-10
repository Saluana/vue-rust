// Code registry, Diagnostic validation and the human-readable formatter. Expected text is written out by hand.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CODES, isKnownCode, codeInfo } from '../../../compiler/diagnostics/codes.mjs';
import { Diagnostic, createDiagnostic, createLineMap } from '../../../compiler/diagnostics/diagnostic.mjs';
import { formatDiagnostic, formatDiagnostics } from '../../../compiler/diagnostics/format.mjs';
import { PROFILE_RULES } from '../../../compiler/analysis/profile-rules.mjs';

const pos = (offset, line, column) => ({ offset, line, column });

function span(source, startOffset, endOffset, code = 'VN2024', extra = {}) {
  const map = createLineMap(source);
  return createDiagnostic(code, { start: map.posAt(startOffset), end: map.posAt(endOffset), file: 'case.js', ...extra });
}

test('the registry and every entry are frozen', () => {
  assert.ok(Object.isFrozen(CODES));
  for (const [code, info] of Object.entries(CODES)) {
    assert.ok(Object.isFrozen(info), `${code} entry is frozen`);
  }
});

test('every registry entry has a title, an explanation and a hint', () => {
  for (const [code, info] of Object.entries(CODES)) {
    for (const field of ['title', 'explanation', 'hint']) {
      assert.equal(typeof info[field], 'string', `${code}.${field}`);
      assert.ok(info[field].length > 0, `${code}.${field} is non-empty`);
    }
  }
});

test('codes follow the allocated ranges: VN1 parse, VN2 JS, VN3 TS, VN4 API/global, VN5 scope, VN7 type checker, VN9 internal', () => {
  const allowed = new Set(['1', '2', '3', '4', '5', '7', '9']);
  for (const code of Object.keys(CODES)) {
    assert.match(code, /^VN\d{4}$/);
    assert.ok(allowed.has(code.slice(2, 3)), `${code} is in an allocated range`);
  }
});

test('the registry has no unused codes: every code is produced by a rule or by the frontend', () => {
  // VN7001 is produced by the type checker (compiler/types/check.mjs), not by a profile rule or by the frontend.
  const produced = new Set(['VN1000', 'VN4016', 'VN4017', 'VN5002', 'VN5003', 'VN5004', 'VN7001', 'VN9001']);
  for (const rule of PROFILE_RULES) produced.add(rule.code);
  assert.deepEqual([...produced].sort(), Object.keys(CODES).sort());
});

test('isKnownCode and codeInfo', () => {
  assert.equal(isKnownCode('VN2001'), true);
  assert.equal(isKnownCode('VN2999'), false);
  assert.equal(isKnownCode(2001), false);
  assert.equal(codeInfo('VN4001').title, 'eval');
  assert.throws(() => codeInfo('VN0000'), RangeError);
});

test('Diagnostic rejects unknown codes, bad severities and empty messages', () => {
  const p = pos(0, 1, 0);
  assert.throws(() => new Diagnostic({ code: 'VN7777', message: 'x', start: p, end: p }), RangeError);
  assert.throws(() => new Diagnostic({ code: 'VN1000', severity: 'info', message: 'x', start: p, end: p }), TypeError);
  assert.throws(() => new Diagnostic({ code: 'VN1000', message: '', start: p, end: p }), TypeError);
});

test('a Diagnostic keeps the documented fields and serializes to JSON without loss', () => {
  const d = span('let a = 010;', 8, 11);
  assert.deepEqual(Object.keys(d).sort(), ['code', 'end', 'file', 'hint', 'message', 'severity', 'start']);
  assert.deepEqual(JSON.parse(JSON.stringify(d)), { ...d });
  assert.equal(d.code, 'VN2024');
  assert.equal(d.severity, 'error');
});

test('formatDiagnostic renders file:line:column, the source line, carets, and the hint (exact text)', () => {
  const source = 'let a = 010;';
  const d = span(source, 8, 11, 'VN2024', { message: 'Legacy octal literal is not supported' });
  const text = formatDiagnostic(d, source);
  assert.equal(text, [
    'case.js:1:8: error VN2024: Legacy octal literal is not supported',
    '  |',
    '1 | let a = 010;',
    '  |         ^^^',
    '  = hint: Write the value in decimal, or use the 0o prefix.',
  ].join('\n'));
});

test('formatDiagnostic keeps tabs in the caret line so the underline stays aligned', () => {
  const source = '\tfoo bar';
  const text = formatDiagnostic(span(source, 5, 8, 'VN1000'), source);
  assert.equal(text.split('\n')[3], '  | \t    ^^^');
  assert.equal(text.split('\n')[2], '1 | \tfoo bar');
});

test('formatDiagnostic counts CJK characters as two cells', () => {
  const source = String.fromCodePoint(0x4e2d, 0x6587) + 'x';
  const text = formatDiagnostic(span(source, 2, 3, 'VN1000'), source);
  assert.equal(text.split('\n')[3], '  |     ^');
});

test('formatDiagnostic counts an emoji (surrogate pair) as two cells and a combining mark as zero', () => {
  const grin = String.fromCodePoint(0x1f600);
  const source = grin + 'x';
  assert.equal(formatDiagnostic(span(source, 2, 3, 'VN1000'), source).split('\n')[3], '  |   ^');
  // 'e' takes one cell and the combining acute takes none, so the caret sits after one space.
  const combined = 'e' + String.fromCharCode(0x0301) + 'x';
  assert.equal(formatDiagnostic(span(combined, 2, 3, 'VN1000'), combined).split('\n')[3], '  |  ^');
});

test('a zero-length span still gets one caret', () => {
  const source = 'ab';
  const text = formatDiagnostic(span(source, 2, 2, 'VN1000'), source);
  assert.equal(text.split('\n')[3], '  |   ^');
});

test('warnings are labelled as warnings and underline a single-line span', () => {
  const source = 'const k = 1;\nk = 2;';
  const d = span(source, 13, 18, 'VN5002', { severity: 'warning', message: 'Cannot assign to const binding k' });
  const lines = formatDiagnostic(d, source).split('\n');
  assert.equal(lines[0], 'case.js:2:0: warning VN5002: Cannot assign to const binding k');
  assert.equal(lines[3], '  | ^^^^^');
});

test('a multi-line span underlines from its start to the end of the first line', () => {
  // Start: line 1, column 6 (the k of "k = 1;"). End: line 2, column 4. Only line 1 is underlined, to its end.
  const source = 'const k = 1;\nk = 2;';
  const d = span(source, 6, 17, 'VN2002');
  const lines = formatDiagnostic(d, source).split('\n');
  assert.equal(lines[2], '1 | const k = 1;');
  assert.equal(lines[3], '  |       ^^^^^^');
});

test('color is off by default and adds ANSI sequences only when requested', () => {
  const source = 'let a = 010;';
  const plain = formatDiagnostic(span(source, 8, 11), source);
  assert.equal(plain.includes('\u001b'), false);
  const painted = formatDiagnostic(span(source, 8, 11), source, { color: true });
  assert.ok(painted.includes('\u001b[31m'));
  assert.ok(painted.includes('\u001b[0m'));
});

test('formatDiagnostics joins several diagnostics with a blank line and omits an empty hint', () => {
  const source = 'let a = 010;';
  const a = span(source, 8, 11);
  const b = span(source, 8, 11, 'VN2024', { hint: '' });
  const text = formatDiagnostics([a, b], source);
  assert.equal(text.split('\n\n').length, 2);
  assert.ok(!text.split('\n\n')[1].includes('hint:'));
});

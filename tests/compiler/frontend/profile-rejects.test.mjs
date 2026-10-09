// Loads every fixture in tests/language/rejects and checks that the frontend reports exactly the diagnostics named in
// its "// @expect VNxxxx L:C-L:C" headers (code and span, end exclusive, 0-based columns). Negative controls prove the
// comparison itself detects wrong expectations.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { extname } from 'node:path';
import { checkSource } from '../../../compiler/frontend/check.mjs';
import { spanText } from './helpers.mjs';

const FIXTURE_DIR = new URL('../../language/rejects/', import.meta.url);
const HEADER = /^\/\/ @expect (VN\d{4}) (\d+):(\d+)-(\d+):(\d+)\s*$/;

const LANG_BY_EXT = { '.js': 'js', '.ts': 'ts', '.jsx': 'jsx', '.tsx': 'tsx' };

export function readFixture(name) {
  const text = readFileSync(new URL(name, FIXTURE_DIR), 'utf8');
  const expected = [];
  for (const line of text.split('\n')) {
    const m = HEADER.exec(line);
    if (m) expected.push(`${m[1]} ${m[2]}:${m[3]}-${m[4]}:${m[5]}`);
  }
  return { name, text, expected, lang: LANG_BY_EXT[extname(name)] };
}

// Returns the list of problems; an empty list means the expectations match exactly (as multisets).
export function compareExpectations(expected, diagnostics) {
  const got = diagnostics.map((d) => `${d.code} ${spanText(d)}`);
  const problems = [];
  const remaining = [...got];
  for (const e of expected) {
    const i = remaining.indexOf(e);
    if (i < 0) problems.push(`missing ${e}`);
    else remaining.splice(i, 1);
  }
  for (const g of remaining) problems.push(`unexpected ${g}`);
  return problems;
}

export function runFixture(fixture) {
  return checkSource(fixture.text, { filename: fixture.name, lang: fixture.lang, target: 'native' }).diagnostics;
}

const names = readdirSync(FIXTURE_DIR).filter((n) => /\.(js|ts|jsx|tsx)$/.test(n)).sort();

test('the reject fixture set is not empty', () => {
  assert.ok(names.length >= 50, `found ${names.length} fixtures`);
});

for (const name of names) {
  test(`fixture ${name} reports exactly its @expect diagnostics`, () => {
    const fixture = readFixture(name);
    assert.ok(fixture.expected.length >= 1, 'fixture declares at least one @expect header');
    const problems = compareExpectations(fixture.expected, runFixture(fixture));
    assert.deepEqual(problems, []);
  });
}

test('a const enum is rejected by the same rule as enum (VN3001 over the "const enum E" head)', () => {
  const r = checkSource('const enum E { A }', { filename: 'const-enum.ts', lang: 'ts', target: 'native' });
  assert.deepEqual(r.diagnostics.map((d) => `${d.code} ${spanText(d)}`), ['VN3001 1:0-1:12']);
});

test('negative control: a shifted column in the expectation is detected', () => {
  const fixture = readFixture('class-extends.js');
  const shifted = fixture.expected.map((e) => e.replace('3:16-3:17', '3:17-3:18'));
  assert.notEqual(shifted.join('|'), fixture.expected.join('|'), 'the mutation changed the expectation');
  const problems = compareExpectations(shifted, runFixture(fixture));
  assert.ok(problems.some((p) => p.startsWith('missing VN2003')), `problems: ${problems.join('; ')}`);
  assert.ok(problems.some((p) => p.startsWith('unexpected VN2003')), `problems: ${problems.join('; ')}`);
});

test('negative control: a wrong code in the expectation is detected', () => {
  const fixture = readFixture('with.js');
  const wrong = fixture.expected.map((e) => e.replace('VN2001', 'VN2002'));
  const problems = compareExpectations(wrong, runFixture(fixture));
  assert.deepEqual(problems, ['missing VN2002 2:0-2:4', 'unexpected VN2001 2:0-2:4']);
});

test('negative control: a missing expectation leaves the reported diagnostic unexpected', () => {
  const fixture = readFixture('class-extends.js');
  const problems = compareExpectations(fixture.expected.slice(0, 1), runFixture(fixture));
  assert.deepEqual(problems, ['unexpected VN2003 3:16-3:17']);
});

test('negative control: a fixture without any @expect header is rejected by the harness', () => {
  const fixture = { name: 'synthetic.js', text: 'with (o) { x }\n', expected: [], lang: 'js' };
  const problems = compareExpectations(fixture.expected, runFixture(fixture));
  assert.deepEqual(problems, ['unexpected VN2001 1:0-1:4']);
});

test('JSX in tsx is rejected with VN3009 at the element span and the template hint', () => {
  const r = checkSource('const a = <div />;', { filename: 'jsx.tsx', lang: 'tsx', target: 'native' });
  assert.deepEqual(r.diagnostics.map((d) => `${d.code} ${spanText(d)}`), ['VN3009 1:10-1:17']);
  assert.equal(r.diagnostics[0].hint, 'use a <template> in a .vue file');
});

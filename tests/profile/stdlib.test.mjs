import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as index from '../../compiler/profile/index.mjs';
import { effectiveStatus, isStdlibSupported, listSupported, lookupStdlib, memberPath, resolveGlobal } from '../../compiler/profile/stdlib.mjs';

// @babel/parser is a project dependency; resolve it from the project root, as the compiler does.
const require = createRequire(new URL('../../package.json', import.meta.url));
const { parse } = require('@babel/parser');

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const stdlib = JSON.parse(readFileSync(path.join(ROOT, 'compiler/profile/stdlib.json'), 'utf8'));
const TARGETS = ['native', 'web', 'test'];
const KINDS = ['global', 'ctor', 'static', 'proto', 'const', 'symbol', 'prop'];

test('lookupStdlib returns the entry for its kind, and null for another kind, unknown names and prototype names', () => {
  const keys = lookupStdlib('static', 'Object.keys');
  assert.equal(keys.status, 'P');
  assert.equal(keys.row, 'L04');
  assert.deepEqual(keys.targets, TARGETS);
  assert.equal(keys.code, null);
  assert.equal(lookupStdlib('proto', 'Object.keys'), null, 'wrong kind');
  assert.equal(lookupStdlib('static', 'Object.nope'), null);
  assert.equal(lookupStdlib('no-such-kind', 'Object.keys'), null);
  assert.equal(lookupStdlib('static', 42), null);
  for (const name of ['constructor', '__proto__', 'toString', 'hasOwnProperty']) {
    assert.equal(lookupStdlib('global', name), null, name);
    assert.equal(lookupStdlib('static', `Object.${name}`), null, name);
  }
});

test('lookupStdlib(static, Object.defineProperty) is X on row L04 with the C1a VN4008 code and a hint', () => {
  const hit = lookupStdlib('static', 'Object.defineProperty');
  assert.equal(hit.status, 'X');
  assert.equal(hit.row, 'L04');
  assert.equal(hit.code, 'VN4008');
  assert.match(hit.hint, /descriptor/);
  assert.equal(isStdlibSupported('Object.defineProperty', 'native'), false);
  assert.equal(isStdlibSupported('Object.defineProperty', 'web'), false);
  assert.equal(isStdlibSupported('Object.defineProperty', 'test'), false);
});

test('results are copies: changing a returned entry does not change later answers', () => {
  const first = lookupStdlib('static', 'Object.keys');
  first.status = 'X';
  first.targets.push('bogus');
  const again = lookupStdlib('static', 'Object.keys');
  assert.equal(again.status, 'P');
  assert.deepEqual(again.targets, TARGETS);
  const listed = listSupported('static');
  listed[0].status = 'X';
  assert.notEqual(listSupported('static')[0].status, 'X');
});

test('P entries are supported on native, web and test, including constants, byte types and iterator intrinsics', () => {
  const names = ['Object.keys', 'Array.prototype.map', 'Math.PI', 'Symbol.iterator', 'Math', 'parseInt', 'NaN', 'Promise.all',
    'JSON.parse', 'Map.prototype.size', 'Error#cause', '%ArrayIteratorPrototype%.next', 'Uint8Array', 'ArrayBuffer',
    'Uint8Array.from', 'Uint8Array.prototype.set', 'Uint8Array.prototype[Symbol.iterator]', 'ArrayBuffer.prototype.byteLength'];
  for (const name of names) {
    for (const target of TARGETS) assert.equal(isStdlibSupported(name, target), true, `${name} on ${target}`);
  }
});

test('X, E and unknown paths are not supported on any target', () => {
  const names = ['Object.defineProperty', 'console', 'globalThis', 'Int8Array', 'DataView', 'Date', 'Date.parse', 'Date.UTC',
    'Date.prototype.getTime', 'RegExp', 'Intl', 'Object.nope', 'Math.nope', 'toString', ''];
  for (const name of names) {
    for (const target of TARGETS) assert.equal(isStdlibSupported(name, target), false, `${name} on ${target}`);
  }
  assert.equal(isStdlibSupported(undefined, 'web'), false);
  assert.equal(isStdlibSupported(42, 'web'), false);
});

test('A entries are supported on native, web and test: host random and clock providers, and the UTF-8 codec', () => {
  const expected = [
    ['static', 'Math.random'], ['static', 'Date.now'], ['global', 'performance'], ['static', 'performance.now'],
    ['ctor', 'TextEncoder'], ['ctor', 'TextDecoder'], ['proto', 'TextEncoder.prototype.encode'], ['proto', 'TextDecoder.prototype.decode'],
  ];
  for (const [kind, name] of expected) {
    const hit = lookupStdlib(kind, name);
    assert.ok(hit, `${kind}:${name} is missing`);
    assert.equal(hit.status, 'A', name);
    assert.equal(hit.row, 'L19', name);
    for (const target of TARGETS) assert.equal(isStdlibSupported(name, target), true, `${name} on ${target}`);
  }
});

test('test-only host globals are supported on the test target and nowhere else', () => {
  for (const name of ['trace', 'testHost']) {
    assert.equal(isStdlibSupported(name, 'test'), true, name);
    assert.equal(isStdlibSupported(name, 'native'), false, name);
    assert.equal(isStdlibSupported(name, 'web'), false, name);
  }
});

test('effectiveStatus: W is supported only on web, and an entry is X on a target it does not list', () => {
  const web = { status: 'W', targets: ['web'] };
  assert.equal(effectiveStatus(web, 'web'), 'W');
  assert.equal(effectiveStatus(web, 'native'), 'X');
  assert.equal(effectiveStatus(web, 'test'), 'X');
  const testOnly = { status: 'A', targets: ['test'] };
  assert.equal(effectiveStatus(testOnly, 'test'), 'A');
  assert.equal(effectiveStatus(testOnly, 'web'), 'X');
  const shared = { status: 'P', targets: TARGETS };
  for (const target of TARGETS) assert.equal(effectiveStatus(shared, target), 'P', target);
});

test('an unknown target or kind is an error, not a guess', () => {
  for (const bad of [undefined, null, 'desktop', 'Web', 'node']) {
    assert.throws(() => isStdlibSupported('Object.keys', bad), TypeError, String(bad));
    assert.throws(() => resolveGlobal('console', bad), TypeError, String(bad));
  }
  assert.throws(() => listSupported('nope'), TypeError);
});

test('resolveGlobal gives the target status and diagnostic hint for free identifiers, and null for unknown names', () => {
  assert.deepEqual(resolveGlobal('undefined', 'web'), { status: 'P', hint: null });
  assert.deepEqual(resolveGlobal('Object', 'native'), { status: 'P', hint: null });
  assert.deepEqual(resolveGlobal('Math', 'native'), { status: 'P', hint: null });
  const consoleHit = resolveGlobal('console', 'native');
  assert.equal(consoleHit.status, 'X');
  assert.match(consoleHit.hint, /logging service/);
  assert.deepEqual(resolveGlobal('trace', 'test'), { status: 'A', hint: null });
  assert.deepEqual(resolveGlobal('performance', 'web'), { status: 'A', hint: null });
  assert.deepEqual(resolveGlobal('ArrayBuffer', 'native'), { status: 'P', hint: null });
  assert.match(resolveGlobal('Int8Array', 'native').hint, /Use Uint8Array with explicit bounds/);
  const traceNative = resolveGlobal('trace', 'native');
  assert.equal(traceNative.status, 'X');
  assert.match(traceNative.hint, /only available on the test target/);
  assert.equal(resolveGlobal('someUnknownGlobal', 'web'), null);
  for (const name of ['constructor', '__proto__', 'toString']) assert.equal(resolveGlobal(name, 'web'), null, name);
  assert.equal(resolveGlobal('keys', 'native'), null, 'a static member is not a free identifier');
});

test('resolveGlobal rejects member paths and non-identifiers instead of guessing', () => {
  assert.throws(() => resolveGlobal('Object.keys', 'native'), TypeError);
  assert.throws(() => resolveGlobal(42, 'native'), TypeError);
  assert.throws(() => resolveGlobal('', 'native'), TypeError);
});

test('listSupported lists P, A and W entries, filtered by kind and sorted by kind order then path', () => {
  const all = listSupported();
  assert.equal(all.length, stdlib.entries.filter((e) => ['P', 'A', 'W'].includes(e.status)).length);
  assert.ok(all.every((e) => ['P', 'A', 'W'].includes(e.status)));
  assert.deepEqual(listSupported('symbol').map((e) => e.path), [
    'Symbol.asyncIterator', 'Symbol.hasInstance', 'Symbol.iterator', 'Symbol.toPrimitive', 'Symbol.toStringTag',
  ]);
  for (let i = 1; i < all.length; i++) {
    const a = all[i - 1];
    const b = all[i];
    const order = KINDS.indexOf(a.kind) - KINDS.indexOf(b.kind);
    assert.ok(order < 0 || (order === 0 && a.path < b.path), `${a.path} before ${b.path}`);
  }
  assert.ok(listSupported('global').some((e) => e.path === 'trace'));
  assert.equal(listSupported('global').some((e) => e.path === 'console'), false);
});

test('memberPath turns Babel identifier chains into dotted paths', () => {
  const expr = (src) => parse(src, {}).program.body[0].expression;
  assert.equal(memberPath(expr('Object.keys')), 'Object.keys');
  assert.equal(memberPath(expr('Math.PI')), 'Math.PI');
  assert.equal(memberPath(expr('a.b.c')), 'a.b.c');
  assert.equal(memberPath(expr('Object')), 'Object');
  assert.equal(memberPath(expr('Object.keys(x)').callee), 'Object.keys');
});

test('memberPath returns null for computed, optional, this, call, literal and new chains, and for non-nodes', () => {
  const expr = (src) => parse(src, {}).program.body[0].expression;
  for (const src of ['Object["keys"]', 'Object[k]', 'a?.b', 'this.x', 'f().x', 'a[0].b', '"abc".length', 'new Foo().x']) {
    assert.equal(memberPath(expr(src)), null, src);
  }
  assert.equal(memberPath(null), null);
  assert.equal(memberPath(undefined), null);
  assert.equal(memberPath('Object.keys'), null);
});

test('compiler/profile/index.mjs re-exports the lookups and keeps its original exports', () => {
  assert.equal(index.lookupStdlib, lookupStdlib);
  assert.equal(index.isStdlibSupported, isStdlibSupported);
  assert.equal(index.listSupported, listSupported);
  assert.equal(index.resolveGlobal, resolveGlobal);
  assert.equal(index.memberPath, memberPath);
  assert.equal(typeof index.loadManifest, 'function');
  assert.equal(typeof index.lookup, 'function');
  assert.equal(typeof index.isSupported, 'function');
  assert.equal(index.lookup('builtin', 'eval').status, 'X');
});

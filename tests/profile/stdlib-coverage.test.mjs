import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { TARGETS } from '../../tooling/oracle/builtin-corpus.mjs';
import { PROFILE_RULES } from '../../compiler/analysis/profile-rules.mjs';
import { isKnownCode } from '../../compiler/diagnostics/codes.mjs';
import { isStdlibSupported, lookupStdlib } from '../../compiler/profile/stdlib.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const read = (rel) => JSON.parse(readFileSync(path.join(ROOT, rel), 'utf8'));
const stdlib = read('compiler/profile/stdlib.json');
const inventory = read('compiler/profile/inventory.generated.json');
const ALL_TARGETS = ['native', 'web', 'test'];

// Expected entries are written by hand from the work-package texts (docs/dev/wp/WP-B1..B6 and WP-G0E), not read back
// from stdlib.json. A name ending in a bracket is a symbol-keyed member: Array.prototype[Symbol.iterator].
const joinPath = (prefix, name) => (name.startsWith('[') ? `${prefix}${name}` : `${prefix}.${name}`);
const group = (kind, prefix, names) => names.map((name) => [kind, joinPath(prefix, name)]);

const MATH_FUNCTIONS = ['abs', 'acos', 'acosh', 'asin', 'asinh', 'atan', 'atan2', 'atanh', 'cbrt', 'ceil', 'clz32', 'cos', 'cosh',
  'exp', 'expm1', 'floor', 'fround', 'hypot', 'imul', 'log', 'log10', 'log1p', 'log2', 'max', 'min', 'pow', 'round', 'sign', 'sin',
  'sinh', 'sqrt', 'tan', 'tanh', 'trunc'];
const MATH_CONSTANTS = ['E', 'LN10', 'LN2', 'LOG10E', 'LOG2E', 'PI', 'SQRT1_2', 'SQRT2'];

// WP-B1: Object, Function.prototype, Symbol, Boolean and the global functions and constants.
const B1 = [
  ['ctor', 'Object'], ['ctor', 'Symbol'], ['ctor', 'Boolean'],
  ...group('static', 'Object', ['keys', 'values', 'entries', 'assign', 'fromEntries', 'freeze', 'isFrozen', 'seal', 'isSealed',
    'preventExtensions', 'isExtensible', 'hasOwn', 'is', 'getPrototypeOf', 'create', 'getOwnPropertyNames', 'getOwnPropertySymbols',
    'getOwnPropertyDescriptor', 'getOwnPropertyDescriptors', 'groupBy']),
  ...group('proto', 'Object.prototype', ['hasOwnProperty', 'isPrototypeOf', 'propertyIsEnumerable', 'toString', 'valueOf',
    'toLocaleString', 'constructor']),
  ...group('proto', 'Function.prototype', ['call', 'apply', 'bind', 'toString', '[Symbol.hasInstance]']),
  ...group('static', 'Symbol', ['for', 'keyFor']),
  ...group('symbol', 'Symbol', ['iterator', 'asyncIterator', 'toPrimitive', 'toStringTag', 'hasInstance']),
  ...group('proto', 'Symbol.prototype', ['toString', 'valueOf', 'description', '[Symbol.toPrimitive]', '[Symbol.toStringTag]']),
  ...group('proto', 'Boolean.prototype', ['toString', 'valueOf']),
  ['global', 'parseInt'], ['global', 'parseFloat'], ['global', 'isNaN'], ['global', 'isFinite'],
  ['const', 'undefined'], ['const', 'NaN'], ['const', 'Infinity'],
];

// WP-B2: Array, including the iterator intrinsic and the length property the spec names.
const B2 = [
  ['ctor', 'Array'],
  ...group('static', 'Array', ['isArray', 'of', 'from']),
  ...group('proto', 'Array.prototype', ['length', 'at', 'concat', 'entries', 'every', 'fill', 'filter', 'find', 'findIndex',
    'findLast', 'findLastIndex', 'flat', 'flatMap', 'forEach', 'includes', 'indexOf', 'join', 'keys', 'lastIndexOf', 'map', 'pop',
    'push', 'reduce', 'reduceRight', 'reverse', 'shift', 'slice', 'some', 'sort', 'splice', 'toString', 'unshift', 'values',
    '[Symbol.iterator]', 'toReversed', 'toSorted', 'toSpliced', 'with']),
  ...group('proto', '%ArrayIteratorPrototype%', ['next', '[Symbol.toStringTag]']),
];

// WP-B3: String, Number and Math (Math.random and Math.f16round are excluded by the spec, so they are not listed here).
const B3 = [
  ['ctor', 'String'],
  ...group('static', 'String', ['fromCharCode', 'fromCodePoint']),
  ...group('proto', 'String.prototype', ['at', 'charAt', 'charCodeAt', 'codePointAt', 'concat', 'endsWith', 'includes', 'indexOf',
    'lastIndexOf', 'padEnd', 'padStart', 'repeat', 'replace', 'replaceAll', 'slice', 'split', 'startsWith', 'substring', 'substr',
    'toLowerCase', 'toString', 'toUpperCase', 'trim', 'trimEnd', 'trimStart', 'valueOf', '[Symbol.iterator]', 'isWellFormed',
    'toWellFormed']),
  ...group('proto', '%StringIteratorPrototype%', ['next', '[Symbol.toStringTag]']),
  ['ctor', 'Number'],
  ...group('static', 'Number', ['isInteger', 'isSafeInteger', 'isFinite', 'isNaN', 'parseFloat', 'parseInt']),
  ...group('const', 'Number', ['EPSILON', 'MAX_SAFE_INTEGER', 'MIN_SAFE_INTEGER', 'MAX_VALUE', 'MIN_VALUE', 'NaN',
    'POSITIVE_INFINITY', 'NEGATIVE_INFINITY']),
  ...group('proto', 'Number.prototype', ['toString', 'toFixed', 'toPrecision', 'toExponential', 'valueOf']),
  ['global', 'Math'],
  ...group('static', 'Math', MATH_FUNCTIONS),
  ...group('const', 'Math', [...MATH_CONSTANTS, '[Symbol.toStringTag]']),
];

// WP-B4: Map, Set (with the ES2025 set algebra) and the iterator prototypes.
const B4 = [
  ['ctor', 'Map'], ...group('static', 'Map', ['groupBy']),
  ...group('proto', 'Map.prototype', ['get', 'set', 'has', 'delete', 'clear', 'forEach', 'entries', 'keys', 'values', 'size',
    '[Symbol.iterator]', '[Symbol.toStringTag]']),
  ['ctor', 'Set'],
  ...group('proto', 'Set.prototype', ['add', 'has', 'delete', 'clear', 'forEach', 'entries', 'keys', 'values', 'size',
    '[Symbol.iterator]', 'union', 'intersection', 'difference', 'symmetricDifference', 'isSubsetOf', 'isSupersetOf',
    'isDisjointFrom']),
  ...group('proto', '%IteratorPrototype%', ['[Symbol.iterator]']),
  ...group('proto', '%MapIteratorPrototype%', ['next', '[Symbol.toStringTag]']),
  ...group('proto', '%SetIteratorPrototype%', ['next', '[Symbol.toStringTag]']),
];

// WP-B6: JSON.
const B6 = [['global', 'JSON'], ...group('static', 'JSON', ['parse', 'stringify']), ...group('const', 'JSON', ['[Symbol.toStringTag]'])];

// WP-G0E paragraph on Promise and the Error types.
const PROMISE = [['ctor', 'Promise'], ...group('static', 'Promise', ['resolve', 'reject', 'all', 'allSettled', 'race', 'any', 'withResolvers']),
  ...group('proto', 'Promise.prototype', ['then', 'catch', 'finally'])];
const ERRORS = [
  ...['Error', 'TypeError', 'RangeError', 'ReferenceError', 'SyntaxError', 'EvalError', 'URIError', 'AggregateError'].map((n) => ['ctor', n]),
  ['proto', 'Error.prototype.toString'],
  ['prop', 'Error#cause'],
];

// The expected pairs that are not a P or A entry exactly once, with the kind the spec names. Returns [] when covered.
function findMissing(entries, expected) {
  return expected.filter(([kind, p]) => {
    const hits = entries.filter((e) => e.path === p);
    return !(hits.length === 1 && hits[0].kind === kind && (hits[0].status === 'P' || hits[0].status === 'A'));
  });
}

// Paths in the list that are missing from the manifest or are not supported (not P or A) on their own.
const nonP = (entries, paths) => paths.filter((p) => {
  const hits = entries.filter((e) => e.path === p);
  return hits.length !== 1 || (hits[0].status !== 'P' && hits[0].status !== 'A');
});

const specs = { 'WP-B1': B1, 'WP-B2': B2, 'WP-B3': B3, 'WP-B4': B4, 'WP-B6': B6, 'WP-G0E Promise': PROMISE, 'WP-G0E Errors': ERRORS };

test('every supported method, static, constant and symbol named by a B-spec is a P entry, present exactly once', () => {
  for (const [name, expected] of Object.entries(specs)) {
    assert.deepEqual(findMissing(stdlib.entries, expected), [], name);
  }
});

test('negative control: removing a B-spec entry, demoting it, or duplicating it is reported as missing', () => {
  const removed = stdlib.entries.filter((e) => e.path !== 'Array.prototype.map');
  assert.deepEqual(findMissing(removed, B2), [['proto', 'Array.prototype.map']]);
  const demoted = stdlib.entries.map((e) => (e.path === 'Math.abs' ? { ...e, status: 'X' } : e));
  assert.deepEqual(findMissing(demoted, B3), [['static', 'Math.abs']]);
  const duplicated = [...stdlib.entries, { ...stdlib.entries.find((e) => e.path === 'Array.prototype.map') }];
  assert.deepEqual(findMissing(duplicated, B2), [['proto', 'Array.prototype.map']]);
});

test('every O2 corpus target (tooling/oracle/builtin-corpus.mjs TARGETS) is a P entry, exactly once, on native and web', () => {
  assert.ok(TARGETS.size > 0);
  const notP = nonP(stdlib.entries, [...TARGETS]);
  assert.deepEqual(notP, [], 'targets that are not a single P entry');
  for (const target of TARGETS) {
    assert.equal(isStdlibSupported(target, 'native'), true, `${target} on native`);
    assert.equal(isStdlibSupported(target, 'web'), true, `${target} on web`);
  }
});

test('negative control: a target the manifest excludes (Math.f16round, Object.defineProperty) is not a P entry', () => {
  const bogus = ['Math.f16round', 'Object.defineProperty', 'console'];
  assert.deepEqual(nonP(stdlib.entries, [...TARGETS, ...bogus]), bogus);
});

test('every X, E and W entry has a hint, and every VN code is registered by C1a', () => {
  for (const e of stdlib.entries) {
    if (['X', 'E', 'W'].includes(e.status)) assert.ok(typeof e.hint === 'string' && e.hint.trim() !== '', e.path);
    if (e.code !== null) {
      assert.ok(['X', 'E'].includes(e.status), `${e.path} carries ${e.code} but is ${e.status}`);
      assert.equal(isKnownCode(e.code), true, `${e.path}: ${e.code}`);
    }
  }
});

test('every entry cites a design row, and a P, A or W entry only cites a row that carries its status', () => {
  const rows = new Map(inventory.rows.map((row) => [row.id, row]));
  for (const e of stdlib.entries) {
    const row = rows.get(e.row);
    assert.ok(row, `${e.path}: unknown row ${e.row}`);
    if (['P', 'A', 'W'].includes(e.status)) assert.ok(row.statuses.includes(e.status), `${e.path}: ${e.status} not in ${e.row}`);
  }
});

test('no path appears under two kinds, and every target list is non-empty and in canonical order', () => {
  const seen = new Set();
  for (const e of stdlib.entries) {
    assert.equal(seen.has(e.path), false, `duplicate ${e.path}`);
    seen.add(e.path);
    assert.ok(e.targets.length > 0, `${e.path} lists no target`);
    assert.deepEqual(e.targets, ALL_TARGETS.filter((t) => e.targets.includes(t)), `${e.path}: targets must be a canonical subset`);
  }
});

test('W entries are web-only, and the only test-only entries are the two host globals', () => {
  for (const e of stdlib.entries) {
    if (e.status === 'W') assert.deepEqual(e.targets, ['web'], e.path);
    if (e.targets.length === 1 && e.targets[0] === 'test') assert.ok(['trace', 'testHost'].includes(e.path), e.path);
  }
});

test('C1a global rules agree with the manifest: the same VN code, and not supported on the rule targets', () => {
  const globalRules = PROFILE_RULES.filter((rule) => rule.kind === 'ref' && Array.isArray(rule.names));
  assert.ok(globalRules.length >= 12, `expected the C1a global rules, found ${globalRules.length}`);
  for (const rule of globalRules) {
    for (const name of rule.names) {
      const hit = ['global', 'ctor', 'const'].map((kind) => lookupStdlib(kind, name)).find(Boolean);
      assert.ok(hit, `${rule.id} names ${name}, which the manifest does not list`);
      assert.equal(hit.code, rule.code, `${name}: C1a ${rule.code}, manifest ${hit.code}`);
      for (const target of rule.targets ?? ALL_TARGETS) {
        assert.equal(isStdlibSupported(name, target), false, `${name} on ${target}`);
      }
    }
  }
});

test('C1a metaprogramming and dispose rules map to VN4008 and VN4015, and the __proto__ access rule to VN2017', () => {
  for (const name of ['defineProperty', 'defineProperties', 'setPrototypeOf']) {
    assert.equal(lookupStdlib('static', `Object.${name}`).code, 'VN4008', name);
  }
  for (const name of ['dispose', 'asyncDispose']) {
    assert.equal(lookupStdlib('symbol', `Symbol.${name}`).code, 'VN4015', name);
  }
  assert.equal(lookupStdlib('proto', 'Object.prototype.__proto__').code, 'VN2017');
});

test('decisions the WP asked for are recorded with the statuses listed below', () => {
  const expected = {
    globalThis: 'X', 'Math.random': 'A', 'Date.now': 'A', 'performance.now': 'A', performance: 'A', TextEncoder: 'A', TextDecoder: 'A',
    Date: 'E', 'Date.parse': 'E', 'Date.UTC': 'E', 'Date.prototype.getTime': 'E', RegExp: 'E', Intl: 'E', ArrayBuffer: 'P',
    Uint8Array: 'P', 'Uint8Array.from': 'P', 'Uint8Array.prototype.set': 'P', 'ArrayBuffer.prototype.byteLength': 'P',
    Int8Array: 'X', DataView: 'X', SharedArrayBuffer: 'X', Atomics: 'X', 'Promise.try': 'X', 'Array.prototype.copyWithin': 'X',
    'Array.prototype[Symbol.unscopables]': 'X', 'Symbol.species': 'X', 'Symbol.isConcatSpreadable': 'X', 'Symbol.match': 'X',
    'Symbol.unscopables': 'X', 'Object.setPrototypeOf': 'X', 'Object.getOwnPropertyDescriptors': 'P', 'Object.create': 'P',
    queueMicrotask: 'X', structuredClone: 'X', 'Math.f16round': 'X', 'Number.prototype.toLocaleString': 'X',
    'String.prototype.localeCompare': 'X', 'String.prototype.match': 'X', Iterator: 'X', WeakMap: 'X', Proxy: 'X', Reflect: 'X',
    eval: 'X', Function: 'X', BigInt: 'X', console: 'X', setTimeout: 'X', fetch: 'X', XMLHttpRequest: 'X', WebSocket: 'X',
    require: 'X', process: 'X', localStorage: 'X', window: 'X', document: 'X', 'Symbol.dispose': 'X', trace: 'A', testHost: 'A',
    'Error#cause': 'P', 'Object.prototype.__proto__': 'X',
  };
  for (const [p, status] of Object.entries(expected)) {
    const hits = stdlib.entries.filter((e) => e.path === p);
    assert.equal(hits.length, 1, `${p} has ${hits.length} entries`);
    assert.equal(hits[0].status, status, p);
  }
});

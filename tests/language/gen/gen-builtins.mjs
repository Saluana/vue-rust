// WP-O2 seed suites: tests/language/builtins/{math,number,object-statics}.json.
// Deterministic: fixed inputs, no clock, no randomness. Every expected value is produced by V8 when a suite is compiled
// (tooling/oracle/builtin-corpus.mjs); nothing here is a hand-written expectation.
//   node tests/language/gen/gen-builtins.mjs [--out <dir>]        (default: tests/language/builtins)
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {compileSuite, defineSuite, renderSuite, writeSuite} from '../../../tooling/oracle/builtin-corpus.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const DEFAULT_OUT = path.resolve(here, '../builtins');

// Case helpers. Inputs are built by make() on every call, so object arguments are fresh for each execution.
// P(label, value) is a primitive; F(label, factory) builds an object or array.
const P = (label, value) => [label, () => value];
const F = (label, factory) => [label, factory];

// ---------------------------------------------------------------- math

// About 40 edge values per unary function: NaN, signed zeros, infinities, tiny and huge magnitudes, negatives, fractions,
// and 2^53 neighbourhood.
const EDGE = [
  P('nan', NaN), P('pzero', 0), P('nzero', -0), P('pinf', Infinity), P('ninf', -Infinity),
  P('one', 1), P('mone', -1), P('half', 0.5), P('mhalf', -0.5), P('onehalf', 1.5), P('monehalf', -1.5),
  P('twohalf', 2.5), P('mtwohalf', -2.5), P('two', 2), P('mtwo', -2), P('three', 3), P('mthree', -3),
  P('ten', 10), P('mten', -10), P('tenth', 0.1), P('mtenth', -0.1), P('third', 1 / 3), P('mtwothirds', -2 / 3),
  P('tiny', 5e-324), P('mtiny', -5e-324), P('small', 1e-300), P('msmall', -1e-300), P('big', 1e300), P('mbig', -1e300),
  P('maxv', Number.MAX_VALUE), P('mmaxv', -Number.MAX_VALUE), P('p53', 2 ** 53), P('p53p2', 2 ** 53 + 2),
  P('m53', -(2 ** 53)), P('p31', 2 ** 31), P('exp-over', 709.8), P('exp-under', -745.2), P('pi', Math.PI),
  P('onem', 0.9999999999999999), P('onep', 1.0000000000000002),
];

// Two-argument grid values.
const BIN = [
  P('nan', NaN), P('pzero', 0), P('nzero', -0), P('pinf', Infinity), P('ninf', -Infinity),
  P('mone', -1), P('half', 0.5), P('two', 2), P('mthree', -3), P('p53', 2 ** 53),
];

// Non-number arguments, coerced by ToNumber.
const COERCE = [
  P('str5', '5'), P('strempty', ''), P('strws', ' 7 '), P('strhex', '0x10'), P('strbad', 'abc'),
  P('true', true), P('null', null), P('undef', undefined), F('arr', () => [4]), F('obj', () => ({})),
];

const UNARY = [
  'abs', 'ceil', 'floor', 'round', 'trunc', 'sign', 'sqrt', 'cbrt', 'exp', 'expm1', 'log', 'log2', 'log10', 'log1p',
  'sin', 'cos', 'tan', 'asin', 'acos', 'atan', 'sinh', 'cosh', 'tanh', 'asinh', 'acosh', 'atanh', 'fround', 'clz32',
];
// Results that V8 computes with its own ieee754 port while a platform libm may differ in the last ulp.
const APPROX = new Set([
  'cbrt', 'exp', 'expm1', 'log', 'log2', 'log10', 'log1p', 'sin', 'cos', 'tan', 'asin', 'acos', 'atan',
  'sinh', 'cosh', 'tanh', 'asinh', 'acosh', 'atanh', 'pow', 'atan2', 'hypot',
]);
const CONSTANTS = ['PI', 'E', 'LN2', 'LN10', 'LOG2E', 'LOG10E', 'SQRT2', 'SQRT1_2'];

function mathCases() {
  const cases = [];
  const add = (id, fn, argsFn) => cases.push({
    id,
    target: `Math.${fn}`,
    static: true,
    approx: APPROX.has(fn),
    make: () => ({args: argsFn()}),
  });
  for (const fn of UNARY) {
    for (const [label, value] of EDGE) add(`${fn}/${label}`, fn, () => [value()]);
    for (const [label, value] of COERCE) add(`${fn}/arg-${label}`, fn, () => [value()]);
    add(`${fn}/no-args`, fn, () => []);
    add(`${fn}/extra-args`, fn, () => [2, 9]);
  }
  for (const fn of ['pow', 'atan2', 'imul']) {
    for (const [la, a] of BIN) {
      for (const [lb, b] of BIN) add(`${fn}/${la}-${lb}`, fn, () => [a(), b()]);
    }
    add(`${fn}/no-args`, fn, () => []);
    add(`${fn}/one-arg`, fn, () => [2]);
  }
  for (const fn of ['min', 'max']) {
    for (const [la, a] of BIN) {
      for (const [lb, b] of BIN) add(`${fn}/${la}-${lb}`, fn, () => [a(), b()]);
    }
    add(`${fn}/no-args`, fn, () => []);
    add(`${fn}/one-arg`, fn, () => [-5]);
    add(`${fn}/many`, fn, () => [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    add(`${fn}/zeros`, fn, () => [0, -0]);
    add(`${fn}/zeros-rev`, fn, () => [-0, 0]);
    add(`${fn}/nan-first`, fn, () => [NaN, 1]);
    add(`${fn}/coerced`, fn, () => ['3', true, null]);
    add(`${fn}/undefined-first`, fn, () => [undefined, 1]);
  }
  for (const [la, a] of BIN) {
    for (const [lb, b] of BIN) add(`hypot/${la}-${lb}`, 'hypot', () => [a(), b()]);
  }
  add('hypot/no-args', 'hypot', () => []);
  add('hypot/3-4', 'hypot', () => [3, 4]);
  add('hypot/3-4-12', 'hypot', () => [3, 4, 12]);
  add('hypot/coerced', 'hypot', () => ['3', 4]);
  add('hypot/huge-pair', 'hypot', () => [1e200, 1e200]);
  add('hypot/tiny-pair', 'hypot', () => [5e-324, 5e-324]);
  for (const name of CONSTANTS) {
    cases.push({id: `const/${name.toLowerCase()}`, target: `Math.${name}`, get: true, make: () => ({})});
  }
  return cases;
}

// ---------------------------------------------------------------- number

const NUMBER_COERCE = [
  P('undef', undefined), P('null', null), P('true', true), P('false', false), P('zero', 0), P('negzero', -0),
  P('empty', ''), P('space', ' '), P('str0', '0'), P('strnegzero', '-0'), P('ws-num', '  12  '),
  P('newline-num', '\n\t 42 \r'), P('exp3', '1e3'), P('expcap', '1E+3'), P('leading-dot', '.5'),
  P('trailing-dot', '5.'), P('plus', '+5'), P('minus', '-5'), P('hex', '0x1F'), P('hexcap', '0X1f'),
  P('bin', '0b101'), P('bincap', '0B11'), P('oct', '0o17'), P('octcap', '0O7'), P('hex-empty', '0x'),
  P('bin-bad', '0b2'), P('underscore', '1_000'), P('infinity', 'Infinity'), P('plus-inf', '+Infinity'),
  P('minus-inf', '-Infinity'), P('lower-inf', 'infinity'), P('exp-overflow', '1e400'),
  P('neg-exp-overflow', '-1e400'), P('px', '12px'), P('abc', 'abc'), P('nan-str', 'NaN'), P('minus-space', '- 5'),
  P('double-minus', '--5'), P('exp-bad', '5e'), P('nbsp', '  9  '), P('line-sep', '  7'),
  P('bom', '﻿8'), P('neg-hex', '-0x10'), F('empty-arr', () => []), F('arr5', () => [5]),
  F('arr-str7', () => ['7']), F('arr12', () => [1, 2]), F('obj', () => ({})), F('arr-null', () => [null]),
  F('arr-hex', () => ['0x10']), F('nested-arr', () => [[]]), P('exp21', 1e21), P('p53p2', 2 ** 53 + 2),
  P('neg-inf', -Infinity), P('nan', NaN), P('maxv', Number.MAX_VALUE), P('tiny', 5e-324), P('tenth', 0.1),
  P('big-str-int', '9007199254740993'),
];

// Instance rows: [label, this, args]. Primitive receivers only (a Number wrapper object is not a trace value).
const TO_STRING = [
  ['hex255', 255, [16]], ['negbin', -255, [2]], ['half-bin', 0.5, [2]], ['frac-bin', 3.75, [2]],
  ['negzero-bin', -0, [2]], ['nan-bin', NaN, [2]], ['inf-hex', Infinity, [16]], ['noarg', 255, []],
  ['dec', 255, [10]], ['radix1', 255, [1]], ['radix37', 255, [37]], ['z36', 35, [36]], ['big36', 1e21, [36]],
  ['third3', 0.1, [3]], ['p53-36', 2 ** 53, [36]], ['this-string', 'x', [10]], ['this-undef', undefined, [10]],
];
const TO_FIXED = [
  ['round-up', 1.005, [2]], ['zero', 0, [0]], ['negzero', -0, [2]], ['big', 1e21, [2]], ['int-digits0', 123.456, [0]],
  ['tiny', 0.000001, [7]], ['nan', NaN, [2]], ['inf', Infinity, [2]], ['half-up', 2.5, [0]], ['half-neg', -2.5, [0]],
  ['max-digits', 5, [100]], ['digits101', 5, [101]], ['digits-neg', 5, [-1]], ['noarg', 5, []],
  ['tiny-exp', 1.23e-10, [2]], ['frac-digits', 1.45, [1]], ['neg-small', -1.5e-7, [3]], ['str-digits', 42, ['x']],
];
const TO_PRECISION = [
  ['four', 123.456, [4]], ['small-two', 0.000123, [2]], ['exp-form', 123456, [2]], ['nan', NaN, [3]], ['noarg', 5, []],
  ['zero-prec', 5, [0]], ['prec101', 5, [101]], ['big-three', 1e21, [3]], ['negzero', -0, [1]], ['many21', 0.1, [21]],
  ['half-one', 1.5, [1]], ['half-one-b', 2.5, [1]], ['int-one', 123, [1]], ['tiny-two', 1e-7, [2]],
];
const TO_EXPONENTIAL = [
  ['two', 123456, [2]], ['noarg-zero', 0, []], ['negzero', -0, [2]], ['zero-digits', 1.5, [0]], ['nan', NaN, [2]],
  ['digits101', 5, [101]], ['tiny', 1e-7, [3]], ['inf', Infinity, [2]], ['noarg', 123.456, []],
  ['small-frac', 0.000015, [1]], ['neg-big', -1.5e300, [3]], ['digits-neg', 5, [-1]],
];
const VALUE_OF = [
  ['num', 42, []], ['negzero', -0, []], ['nan', NaN, []], ['tiny', 5e-324, []], ['this-string', 'x', []],
  ['this-undef', undefined, []],
];
const PARSE_FLOAT = [
  ' 3.5e2xyz', '-0', '.5', '-.5e-3x', 'Infinityx', '-Infinity', '1e400', '-1e400', '0x10', '', 'abc', '  +12.5',
  '1_000', '5.', '1e', '1e+', '\n\t3.25 ', '00012.5', '-', '.', '1.2.3', '5e-324', '2e-400',
];
const PARSE_INT = [
  ['hex', ['0x1F']], ['hex-radix16', ['0x1F', 16]], ['hex-radix10', ['0x1F', 10]], ['leading-zero', ['08']],
  ['exp', ['1e3']], ['sign-trailing', ['  -42abc']], ['alpha', ['abc']], ['radix37', ['12', 37]],
  ['radix1', ['12', 1]], ['radix0', ['12', 0]], ['radix36-z', ['z', 36]], ['negzero', ['-0']],
  ['above-int32', ['2147483648']], ['rounds', ['9007199254740993']], ['overflow', ['1'.repeat(330)]],
  ['empty', ['']], ['space', [' ']], ['binary-prefix', ['0b11']], ['float', [15.99]], ['neg-exp', [-1e21]],
  ['null', [null]], ['hex-ff', ['ff', 16]], ['radix-string', ['ff', '16']], ['radix-nan', ['ff', NaN]],
  ['radix-wrap', ['ff', 2 ** 32 + 16]], ['binary-11', ['11', 2]], ['neg-hex', ['-ff', 16]],
  ['plus', ['+12']], ['hex-empty', ['0x']], ['hex-cap', ['0X1a']], ['hex-space', [' 0x1a', 16]],
  ['neg-radix', ['123', -1]],
];

function numberCases() {
  const cases = [];
  const stat = (id, target, argsFn) => cases.push({id, target, static: true, make: () => ({args: argsFn()})});
  const inst = (id, target, selfFn, argsFn) => cases.push({id, target, make: () => ({this: selfFn(), args: argsFn()})});

  const INT = [
    P('zero', 0), P('negzero', -0), P('one', 1), P('mone', -1), P('frac', 1.5), P('nan', NaN), P('pinf', Infinity),
    P('ninf', -Infinity), P('p53', 2 ** 53), P('p53p2', 2 ** 53 + 2), P('big1e21', 1e21), P('tiny', 5e-324),
    P('str5', '5'), P('true', true), P('null', null), P('undef', undefined), F('arr', () => []), F('obj', () => ({})),
    P('str-empty', ''),
  ];
  for (const [label, value] of INT) stat(`isinteger/${label}`, 'Number.isInteger', () => [value()]);
  stat('isinteger/no-args', 'Number.isInteger', () => []);

  const SAFE = [
    P('zero', 0), P('negzero', -0), P('max-safe', 2 ** 53 - 1), P('neg-max-safe', -(2 ** 53 - 1)), P('p53', 2 ** 53),
    P('m53', -(2 ** 53)), P('frac', 1.5), P('nan', NaN), P('pinf', Infinity), P('str3', '3'), P('p31', 2 ** 31),
    P('big', 1e300), P('true', true), P('null', null),
  ];
  for (const [label, value] of SAFE) stat(`issafeinteger/${label}`, 'Number.isSafeInteger', () => [value()]);

  const FINITE = [
    P('zero', 0), P('negzero', -0), P('maxv', Number.MAX_VALUE), P('tiny', 5e-324), P('nan', NaN), P('pinf', Infinity),
    P('ninf', -Infinity), P('str5', '5'), P('null', null), P('undef', undefined), P('true', true), F('arr', () => []),
    F('obj', () => ({})),
  ];
  for (const [label, value] of FINITE) stat(`isfinite/${label}`, 'Number.isFinite', () => [value()]);

  const IS_NAN = [
    P('nan', NaN), P('zero', 0), P('nan-str', 'NaN'), P('undef', undefined), P('pinf', Infinity), P('empty', ''),
    F('obj', () => ({})), P('negzero', -0), F('arr-nan', () => [NaN]),
  ];
  for (const [label, value] of IS_NAN) stat(`isnan/${label}`, 'Number.isNaN', () => [value()]);

  for (const [i, text] of PARSE_FLOAT.entries()) {
    stat(`parsefloat/s${String(i).padStart(2, '0')}`, 'Number.parseFloat', () => [text]);
  }
  stat('parsefloat/num', 'Number.parseFloat', () => [12]);
  stat('parsefloat/null', 'Number.parseFloat', () => [null]);
  stat('parsefloat/undef', 'Number.parseFloat', () => [undefined]);
  stat('parsefloat/arr', 'Number.parseFloat', () => [['1.5']]);
  stat('parsefloat/true', 'Number.parseFloat', () => [true]);

  for (const [label, args] of PARSE_INT) stat(`parseint/${label}`, 'Number.parseInt', () => args.slice());

  for (const [label, self, args] of TO_STRING) {
    inst(`tostring/${label}`, 'Number.prototype.toString', () => self, () => args.slice());
  }
  for (const [label, self, args] of TO_FIXED) {
    inst(`tofixed/${label}`, 'Number.prototype.toFixed', () => self, () => args.slice());
  }
  for (const [label, self, args] of TO_PRECISION) {
    inst(`toprecision/${label}`, 'Number.prototype.toPrecision', () => self, () => args.slice());
  }
  for (const [label, self, args] of TO_EXPONENTIAL) {
    inst(`toexponential/${label}`, 'Number.prototype.toExponential', () => self, () => args.slice());
  }
  for (const [label, self, args] of VALUE_OF) {
    inst(`valueof/${label}`, 'Number.prototype.valueOf', () => self, () => args.slice());
  }

  for (const [label, value] of NUMBER_COERCE) stat(`coerce/${label}`, 'Number', () => [value()]);
  stat('coerce/no-args', 'Number', () => []);
  stat('coerce/two-args', 'Number', () => [1, 2]);
  return cases;
}

// ---------------------------------------------------------------- object statics

// Integer-like keys come first in ascending order, then string keys in insertion order.
const ord = () => ({b: 1, 2: 'x', a: 2, 1: 'y', '01': 'z', '-1': 'w'});
const holey = (...rest) => {
  const a = [1];
  a[2] = 3;
  return [a, ...rest];
};

function objectCases() {
  const cases = [];
  const stat = (id, target, argsFn) => cases.push({id, target, static: true, make: () => ({args: argsFn()})});

  stat('keys/empty', 'Object.keys', () => [{}]);
  stat('keys/plain', 'Object.keys', () => [{a: 1, b: 2}]);
  stat('keys/int-order', 'Object.keys', () => [ord()]);
  stat('keys/nested', 'Object.keys', () => [{a: {b: 1}, c: [1]}]);
  stat('keys/array-holes', 'Object.keys', () => holey());
  stat('keys/array-extra-prop', 'Object.keys', () => {
    const a = [7, 8];
    a.x = 'y';
    return [a];
  });
  stat('keys/string', 'Object.keys', () => ['ab']);
  stat('keys/string-empty', 'Object.keys', () => ['']);
  stat('keys/number', 'Object.keys', () => [5]);
  stat('keys/boolean', 'Object.keys', () => [true]);
  stat('keys/null-proto', 'Object.keys', () => {
    const o = Object.create(null);
    o.a = 1;
    return [o];
  });
  stat('keys/null', 'Object.keys', () => [null]);
  stat('keys/undefined', 'Object.keys', () => [undefined]);
  stat('keys/no-args', 'Object.keys', () => []);
  stat('keys/alias', 'Object.keys', () => {
    const inner = {x: 1};
    return [{p: inner, q: inner}];
  });

  stat('values/plain', 'Object.values', () => [{a: 1, b: 'two'}]);
  stat('values/int-order', 'Object.values', () => [ord()]);
  stat('values/alias', 'Object.values', () => {
    const shared = [1];
    return [{a: shared, b: shared}];
  });
  stat('values/array-holes', 'Object.values', () => holey().slice(0, 1));
  stat('values/string', 'Object.values', () => ['hi']);
  stat('values/number', 'Object.values', () => [42]);
  stat('values/null', 'Object.values', () => [null]);

  stat('entries/plain', 'Object.entries', () => [{a: 1, b: 2}]);
  stat('entries/int-order', 'Object.entries', () => [ord()]);
  stat('entries/string', 'Object.entries', () => ['hi']);
  stat('entries/array', 'Object.entries', () => [['x', 'y']]);
  stat('entries/nested', 'Object.entries', () => [{a: {b: [1]}}]);
  stat('entries/null', 'Object.entries', () => [null]);

  stat('assign/merge', 'Object.assign', () => [{a: 1, b: 2}, {b: 3, c: 4}]);
  stat('assign/two-sources', 'Object.assign', () => [{}, {a: 1}, {b: 2}]);
  stat('assign/nullish-sources', 'Object.assign', () => [{a: 1}, null, undefined, {b: 2}]);
  stat('assign/string-source', 'Object.assign', () => [{}, 'ab']);
  stat('assign/array-source-holes', 'Object.assign', () => [{}, holey()[0]]);
  stat('assign/number-source', 'Object.assign', () => [{}, 5]);
  stat('assign/int-key-order', 'Object.assign', () => [{z: 1}, {2: 'b', 1: 'a'}]);
  stat('assign/null-target', 'Object.assign', () => [null, {a: 1}]);
  stat('assign/undefined-target', 'Object.assign', () => [undefined]);
  stat('assign/self', 'Object.assign', () => {
    const o = {a: 1};
    return [o, o];
  });
  stat('assign/array-target', 'Object.assign', () => [[1, 2], [3]]);
  stat('assign/alias-source', 'Object.assign', () => {
    const shared = {k: 1};
    return [{}, {x: shared, y: shared}];
  });

  stat('fromentries/pairs', 'Object.fromEntries', () => [[['a', 1], ['b', 2]]]);
  stat('fromentries/duplicate-last-wins', 'Object.fromEntries', () => [[['a', 1], ['a', 2]]]);
  stat('fromentries/empty', 'Object.fromEntries', () => [[]]);
  stat('fromentries/int-key', 'Object.fromEntries', () => [[[1, 'x'], ['0', 'y']]]);
  stat('fromentries/map', 'Object.fromEntries', () => [new Map([['k', 'v'], [2, 'w']])]);
  stat('fromentries/missing-value', 'Object.fromEntries', () => [[['a']]]);
  stat('fromentries/shared-value', 'Object.fromEntries', () => {
    const v = {z: 1};
    return [[['a', v], ['b', v]]];
  });
  stat('fromentries/extra-element', 'Object.fromEntries', () => [[['a', 1, 'extra']]]);
  stat('fromentries/number-entry', 'Object.fromEntries', () => [[1]]);
  stat('fromentries/string-entry', 'Object.fromEntries', () => [['ab']]);
  stat('fromentries/null', 'Object.fromEntries', () => [null]);

  // Frozen, sealed and non-extensible state is carried by trace-format v1.1 (integrity) and replays. These cases still
  // take unfrozen inputs; frozen-input cases are not part of this suite revision (the object results carry integrity).
  stat('freeze/object', 'Object.freeze', () => [{a: 1}]);
  stat('freeze/array', 'Object.freeze', () => [[1, 2]]);
  stat('freeze/number', 'Object.freeze', () => [5]);
  stat('freeze/null', 'Object.freeze', () => [null]);
  stat('freeze/undefined', 'Object.freeze', () => [undefined]);
  stat('isfrozen/unfrozen-object', 'Object.isFrozen', () => [{a: 1}]);
  stat('isfrozen/empty-array', 'Object.isFrozen', () => [[]]);
  stat('isfrozen/number', 'Object.isFrozen', () => [5]);
  stat('isfrozen/string', 'Object.isFrozen', () => ['abc']);
  stat('isfrozen/null', 'Object.isFrozen', () => [null]);
  stat('isfrozen/no-args', 'Object.isFrozen', () => []);
  stat('seal/object', 'Object.seal', () => [{a: 1}]);
  stat('seal/string', 'Object.seal', () => ['x']);
  stat('issealed/unsealed-object', 'Object.isSealed', () => [{a: 1}]);
  stat('issealed/undefined', 'Object.isSealed', () => [undefined]);
  stat('issealed/number', 'Object.isSealed', () => [0]);
  stat('preventextensions/object', 'Object.preventExtensions', () => [{}]);
  stat('preventextensions/null', 'Object.preventExtensions', () => [null]);
  stat('isextensible/plain', 'Object.isExtensible', () => [{}]);
  stat('isextensible/number', 'Object.isExtensible', () => [1]);
  stat('isextensible/null', 'Object.isExtensible', () => [null]);
  stat('isextensible/array', 'Object.isExtensible', () => [[1]]);
  stat('isextensible/no-args', 'Object.isExtensible', () => []);

  stat('is/zero-negzero', 'Object.is', () => [0, -0]);
  stat('is/nan-nan', 'Object.is', () => [NaN, NaN]);
  stat('is/negzero-negzero', 'Object.is', () => [-0, -0]);
  stat('is/strings', 'Object.is', () => ['a', 'a']);
  stat('is/distinct-objects', 'Object.is', () => [{}, {}]);
  stat('is/same-object', 'Object.is', () => {
    const o = {};
    return [o, o];
  });
  stat('is/null-undefined', 'Object.is', () => [null, undefined]);
  stat('is/one-arg', 'Object.is', () => [1]);
  stat('is/no-args', 'Object.is', () => []);
  stat('is/nan-zero', 'Object.is', () => [NaN, 0]);
  stat('is/number-string', 'Object.is', () => [1, '1']);

  stat('hasown/own', 'Object.hasOwn', () => [{a: 1}, 'a']);
  stat('hasown/inherited', 'Object.hasOwn', () => [{}, 'toString']);
  stat('hasown/array-hole', 'Object.hasOwn', () => holey(1));
  stat('hasown/array-index', 'Object.hasOwn', () => holey(2));
  stat('hasown/numeric-key', 'Object.hasOwn', () => [{1: 'x'}, 1]);
  stat('hasown/undefined-value', 'Object.hasOwn', () => [{a: undefined}, 'a']);
  stat('hasown/null-object', 'Object.hasOwn', () => [null, 'a']);
  stat('hasown/undefined-object', 'Object.hasOwn', () => [undefined, 'a']);
  stat('hasown/string-index', 'Object.hasOwn', () => ['abc', 1]);
  stat('hasown/string-length', 'Object.hasOwn', () => ['abc', 'length']);
  stat('hasown/string-out-of-range', 'Object.hasOwn', () => ['abc', 5]);
  stat('hasown/object-key', 'Object.hasOwn', () => [{'[object Object]': 1}, {}]);
  stat('hasown/no-args', 'Object.hasOwn', () => []);

  stat('getprototypeof/plain', 'Object.getPrototypeOf', () => [{}]);
  stat('getprototypeof/array', 'Object.getPrototypeOf', () => [[]]);
  stat('getprototypeof/null-proto', 'Object.getPrototypeOf', () => [Object.create(null)]);
  stat('getprototypeof/null', 'Object.getPrototypeOf', () => [null]);
  stat('getprototypeof/undefined', 'Object.getPrototypeOf', () => [undefined]);
  stat('getprototypeof/string', 'Object.getPrototypeOf', () => ['abc']);
  stat('getprototypeof/number', 'Object.getPrototypeOf', () => [5]);
  stat('getprototypeof/no-args', 'Object.getPrototypeOf', () => []);

  stat('create/null', 'Object.create', () => [null]);
  stat('create/object-proto', 'Object.create', () => [{x: 1}]);
  stat('create/alias-proto', 'Object.create', () => {
    const p = {};
    return [p];
  });
  stat('create/array-proto', 'Object.create', () => [[1]]);
  stat('create/number', 'Object.create', () => [5]);
  stat('create/string', 'Object.create', () => ['x']);
  stat('create/undefined', 'Object.create', () => [undefined]);
  stat('create/no-args', 'Object.create', () => []);
  return cases;
}

// ---------------------------------------------------------------- build

export function buildSuites() {
  return [
    {file: 'math.json', suite: defineSuite('math', mathCases())},
    {file: 'number.json', suite: defineSuite('number', numberCases())},
    {file: 'object-statics.json', suite: defineSuite('object-statics', objectCases())},
  ];
}

// Compiled and replay-checked text of every seed suite, without writing files.
export function renderAll() {
  return buildSuites().map(({file, suite}) => ({file, text: renderSuite(compileSuite(suite))}));
}

function main(argv) {
  const at = argv.indexOf('--out');
  const outDir = at >= 0 ? path.resolve(argv[at + 1]) : DEFAULT_OUT;
  for (const {file, suite} of buildSuites()) {
    const target = path.join(outDir, file);
    writeSuite(target, suite);
    process.stdout.write(`wrote ${target} (${suite.cases.length} cases)\n`);
  }
}

if (process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2));
}

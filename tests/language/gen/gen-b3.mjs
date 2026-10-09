// WP-B3 corpus suites: tests/language/builtins/{string-proto,string-replace,string-statics,number-proto,math-extra}.json.
// Deterministic: fixed inputs and a fixed pseudo-random sampler (no clock, no Math.random). Every expected value is
// produced by V8 when a suite is compiled (tooling/oracle/builtin-corpus.mjs); nothing here is a hand-written expectation.
// Array arguments are left out on purpose: their ToString needs Array.prototype.toString (the Array family).
//   node tests/language/gen/gen-b3.mjs [--out <dir>]        (default: tests/language/builtins)
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {defineSuite, renderSuite, compileSuite, writeSuite} from '../../../tooling/oracle/builtin-corpus.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const DEFAULT_OUT = path.resolve(here, '../builtins');

// Deterministic sampling: a fixed LCG picks a stable subset of a case grid (keeps the suites a reviewable size).
let seed = 0x2545f491;
const rnd = () => {
  seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
  return seed / 4294967296;
};
function sample(list, n) {
  if (list.length <= n) return list.slice();
  const idx = list.map((_, i) => i);
  for (let i = idx.length - 1; i > 0; i--) {
    const j = Math.floor(rnd() * (i + 1));
    [idx[i], idx[j]] = [idx[j], idx[i]];
  }
  return idx.slice(0, n).sort((a, b) => a - b).map((i) => list[i]);
}

// Case-value helpers: P is a primitive, labelled for the case id.
const P = (label, value) => [label, () => value];
const slug = (s) => s.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'x';

// Strings with lone surrogates, astral characters, whitespace and special casing.
const STRS = [
  P('empty', ''), P('a', 'a'), P('abc', 'abc'), P('csv', 'a,b,,c'), P('xbx', 'aXbXc'), P('astral', 'ab\u{1F600}cd'),
  P('lone', '\ud800x\udc00'), P('ws', '  \t x y \n'), P('special', 'ßAΑΣ'), P('mixed', 'ABCdef'), P('dollar', 'a$b'),
];
const STR6 = STRS.filter((_, i) => [0, 1, 2, 3, 5, 6].includes(i));

// Argument values for position arguments (ToIntegerOrInfinity and ToLength edge cases).
const POSARGS = [
  P('undef', undefined), P('null', null), P('true', true), P('false', false), P('zero', 0), P('negzero', -0), P('one', 1),
  P('two', 2), P('three', 3), P('mone', -1), P('mthree', -3), P('ten', 10), P('frac', 1.5), P('mhalf', -0.5), P('nan', NaN),
  P('pinf', Infinity), P('ninf', -Infinity), P('p31', 2 ** 31), P('str2', '2'), P('strneg1', '-1'), P('strx', 'x'),
  P('strpad', ' 1 '), P('p32', 4294967296),
];

const SEARCH = [
  P('empty', ''), P('b', 'b'), P('bc', 'bc'), P('a', 'a'), P('abc', 'abc'), P('abcd', 'abcd'), P('undefined-str', 'undefined'),
  P('undef', undefined), P('null', null), P('one', 1), P('c', 'c'), P('lone', '\ud800'), P('astral', 'ab\u{1F600}'),
];
const POS = [
  P('undef', undefined), P('zero', 0), P('one', 1), P('two', 2), P('mone', -1), P('five', 5), P('nan', NaN),
  P('pinf', Infinity), P('ninf', -Infinity), P('str1', '1'), P('frac', 1.5),
];
const PAIRS = [
  [], [P('undef', undefined)], [P('zero', 0), P('undef', undefined)], [P('one', 1), P('two', 2)], [P('two', 2), P('one', 1)],
  [P('mtwo', -2), P('undef', undefined)], [P('mthree', -3), P('mone', -1)], [P('nan', NaN), P('three', 3)],
  [P('one', 1), P('pinf', Infinity)], [P('ninf', -Infinity), P('two', 2)], [P('null', null), P('str2', '2')],
  [P('frac', 1.9), P('frac39', 3.9)], [P('three', 3), P('mone', -1)], [P('zero', 0), P('zero', 0)], [P('five', 5), P('two', 2)],
  [P('strx', 'x')],
];
const MAXLEN = [
  P('undef', undefined), P('zero', 0), P('one', 1), P('two', 2), P('three', 3), P('five', 5), P('ten', 10), P('mone', -1),
  P('nan', NaN), P('str7', '7'), P('frac', 2.7), P('pinf', Infinity),
];
const FILL = [
  P('undef', undefined), P('empty', ''), P('x', 'x'), P('xy', 'xy'), P('abc', 'abc'), P('lone', '\ud800'), P('zero', 0), P('null', null),
];
const COUNT = [
  P('undef', undefined), P('zero', 0), P('one', 1), P('two', 2), P('three', 3), P('negzero', -0), P('half', 0.5), P('p19', 1.9),
  P('mone', -1), P('mhalf', -0.5), P('nan', NaN), P('pinf', Infinity), P('ninf', -Infinity), P('str2', '2'), P('p29', 2 ** 29),
  P('p53', 2 ** 53), P('true', true), P('empty', ''), P('p399', 3.99),
];
const CONCAT_ARGS = [
  [], [P('undef', undefined)], [P('null', null)], [P('one', 1), P('two', 2)], [P('empty', ''), P('empty', '')],
  [P('lone', '\ud800'), P('b', 'b')], [P('true', true), P('false', false), P('zero', 0), P('negzero', -0)],
  [P('x', 'x'), P('y', 'y'), P('z', 'z')],
];
const PAT = [
  P('empty', ''), P('b', 'b'), P('bb', 'bb'), P('a', 'a'), P('x', 'x'), P('undef', undefined), P('null', null), P('one', 1),
  P('undefined-str', 'undefined'), P('ab', 'ab'), P('lone', '\ud800'),
];
const REP = [
  P('empty', ''), P('X', 'X'), P('amp', '$&'), P('dollar', '$$'), P('backtick', '$`'), P('quote', "$'"), P('one', '$1'),
  P('named', '$<a>'), P('ampamp', '[$&$&]'), P('lonedollar', '$'), P('adollar', 'a$'), P('ydollar', 'Y$'),
];
const SUBJ = [
  P('abcbb', 'abcbb'), P('aaaa', 'aaaa'), P('ab', 'ab'), P('empty', ''), P('adollarb', 'a$b'), P('xaybz', 'xaybz'),
  P('lonelone', '\ud800\ud800'), P('astral', 'ab\u{1F600}cd'),
];
const SEP = [
  P('undef', undefined), P('null', null), P('empty', ''), P('comma', ','), P('b', 'b'), P('bc', 'bc'), P('ab', 'ab'), P('one', 1),
  P('undefined-str', 'undefined'), P('space', ' '), P('lone', '\ud800'), P('commas', ',,'),
];
const LIM = [
  P('undef', undefined), P('zero', 0), P('one', 1), P('two', 2), P('mone', -1), P('p32p1', 2 ** 32 + 1), P('nan', NaN),
  P('str2', '2'), P('frac39', 3.9), P('mhalf', -0.5),
];

// ---------------------------------------------------------------- String.prototype (corpus of method calls)

const ALL_STRING_METHODS = [
  'at', 'charAt', 'charCodeAt', 'codePointAt', 'concat', 'endsWith', 'includes', 'indexOf', 'isWellFormed', 'lastIndexOf',
  'padEnd', 'padStart', 'repeat', 'slice', 'startsWith', 'substr', 'substring', 'toLowerCase', 'toString', 'toUpperCase',
  'toWellFormed', 'trim', 'trimEnd', 'trimStart', 'valueOf',
];

function stringProtoCases() {
  const cases = [];
  const add = (target, label, self, argFns) => {
    const id = `${target.split('.').pop().toLowerCase()}/${label}`;
    cases.push({
      id,
      target,
      make: () => ({this: self(), args: argFns.map((f) => f())}),
    });
  };
  for (const name of ['at', 'charAt', 'charCodeAt', 'codePointAt']) {
    for (const [sl, sv] of STR6) {
      for (const [al, av] of POSARGS) add(`String.prototype.${name}`, `${slug(sl)}-${slug(al)}`, sv, [av]);
    }
  }
  for (const name of ['slice', 'substring', 'substr']) {
    for (const [sl, sv] of STR6) {
      for (const [i, args] of PAIRS.entries()) {
        const label = args.length === 0 ? 'none' : args.map((a) => a[0]).join('-');
        add(`String.prototype.${name}`, `${slug(sl)}-${i}-${slug(label)}`, sv, args.map(([, v]) => v));
      }
    }
  }
  for (const name of ['includes', 'indexOf', 'lastIndexOf', 'startsWith', 'endsWith']) {
    const grid = [];
    for (const [sl, sv] of STR6.slice(1, 5)) {
      for (const [xl, xv] of SEARCH) for (const [pl, pv] of POS) grid.push([sl, sv, xl, xv, pl, pv]);
    }
    for (const [sl, sv, xl, xv, pl, pv] of sample(grid, 70)) {
      add(`String.prototype.${name}`, `${slug(sl)}-${slug(xl)}-${slug(pl)}`, sv, [xv, pv]);
    }
  }
  for (const name of ['padStart', 'padEnd']) {
    const grid = [];
    for (const [sl, sv] of STR6) for (const [ml, mv] of MAXLEN) for (const [fl, fv] of FILL) grid.push([sl, sv, ml, mv, fl, fv]);
    for (const [sl, sv, ml, mv, fl, fv] of sample(grid, 110)) {
      add(`String.prototype.${name}`, `${slug(sl)}-${slug(ml)}-${slug(fl)}`, sv, [mv, fv]);
    }
  }
  for (const [sl, sv] of STR6) {
    for (const [cl, cv] of COUNT) add('String.prototype.repeat', `${slug(sl)}-${slug(cl)}`, sv, [cv]);
  }
  for (const [sl, sv] of STR6) {
    for (const [i, args] of CONCAT_ARGS.entries()) add('String.prototype.concat', `${slug(sl)}-${i}`, sv, args.map(([, v]) => v));
  }
  for (const name of ['toLowerCase', 'toUpperCase', 'trim', 'trimEnd', 'trimStart', 'isWellFormed', 'toWellFormed', 'toString', 'valueOf']) {
    for (const [sl, sv] of STRS) add(`String.prototype.${name}`, slug(sl), sv, []);
  }
  // Receivers that are not strings: null and undefined throw (RequireObjectCoercible); other primitives are converted.
  for (const name of ALL_STRING_METHODS) {
    add(`String.prototype.${name}`, 'this-undefined', () => undefined, []);
    add(`String.prototype.${name}`, 'this-null', () => null, []);
  }
  for (const name of ALL_STRING_METHODS) {
    for (const [i, v] of [12, -0, true, 1.5].entries()) {
      add(`String.prototype.${name}`, `this-prim-${i}`, () => v, [() => 0]);
    }
  }
  return cases;
}

// ---------------------------------------------------------------- replace, replaceAll and split (string patterns)

function stringReplaceCases() {
  const cases = [];
  const grid = [];
  for (const [pl, pv] of PAT) for (const [rl, rv] of REP) for (const [sl, sv] of SUBJ) grid.push([pl, pv, rl, rv, sl, sv]);
  for (const [pl, pv, rl, rv, sl, sv] of sample(grid, 220)) {
    cases.push({
      id: `replace/${slug(sl)}-${slug(pl)}-${slug(rl)}`,
      target: 'String.prototype.replace',
      make: () => ({this: sv(), args: [pv(), rv()]}),
    });
  }
  const grid2 = [];
  for (const [pl, pv] of PAT) for (const [rl, rv] of REP) for (const [sl, sv] of SUBJ) grid2.push([pl, pv, rl, rv, sl, sv]);
  for (const [pl, pv, rl, rv, sl, sv] of sample(grid2, 220)) {
    cases.push({
      id: `replaceall/${slug(sl)}-${slug(pl)}-${slug(rl)}`,
      target: 'String.prototype.replaceAll',
      make: () => ({this: sv(), args: [pv(), rv()]}),
    });
  }
  const sgrid = [];
  for (const [sl, sv] of SUBJ) for (const [pl, pv] of SEP) for (const [ll, lv] of LIM) sgrid.push([sl, sv, pl, pv, ll, lv]);
  for (const [sl, sv, pl, pv, ll, lv] of sample(sgrid, 300)) {
    cases.push({
      id: `split/${slug(sl)}-${slug(pl)}-${slug(ll)}`,
      target: 'String.prototype.split',
      make: () => ({this: sv(), args: [pv(), lv()]}),
    });
  }
  return cases;
}

// ---------------------------------------------------------------- String statics and the String(...) call

function stringStaticCases() {
  const cases = [];
  const FCC = [
    [], [65], [65, 66, 67], [65601], [-1], [NaN], [Infinity], ['65'], [true], [null], [1.9], [0xD800, 0xDC00], [2 ** 32 + 65],
    [undefined], [-0], [0x10000 + 65, 'x'], [1, 2, 3, 4, 5, 6, 7, 8, 9, 10], ['a', 'b'], [0x10000], [0xFFFF, 0x10000 + 0x41],
  ];
  const FCP = [
    [], [65], [0x1F600], [0x10FFFF], [0x110000], [-0], [NaN], [1.5], ['66'], [0xD800], [65, 0x110000], [-1], [Infinity], [true],
    [0], [65, 66, 0x1F600], [0xDBFF, 0xDFFF], [0x41, 0x42, 0x43, 0x44], ['0x41'], [1e21],
  ];
  FCC.forEach((args, i) => cases.push({
    id: `fromcharcode/${String(i).padStart(2, '0')}`,
    target: 'String.fromCharCode',
    static: true,
    make: () => ({args: args.map((v) => v)}),
  }));
  FCP.forEach((args, i) => cases.push({
    id: `fromcodepoint/${String(i).padStart(2, '0')}`,
    target: 'String.fromCodePoint',
    static: true,
    make: () => ({args: args.map((v) => v)}),
  }));
  const CALL = [
    [], [undefined], [null], [true], [1e21], [-0], [NaN], ['x'], [1.5], [12, 'x'], [0x1F600], [''],
    [Infinity], [-1e-7], [5e-324], ['\ud800'],
  ];
  CALL.forEach((args, i) => cases.push({
    id: `call/${String(i).padStart(2, '0')}`,
    target: 'String',
    static: true,
    make: () => ({args: args.map((v) => v)}),
  }));
  return cases;
}

// ---------------------------------------------------------------- Number.prototype, Number(...), constants

const NV = [0, -0, 1, -1, 255, 0.5, -255.5, 3.75, 0.1, 1e21, -1e21, 2 ** 53, 2 ** 53 + 2, NaN, Infinity, -Infinity,
  1.7976931348623157e308, 5e-324, 123.456, 1e-7, 0.000001, 4294967295, 100, 2.5];
const NV_LABELS = ['zero', 'negzero', 'one', 'mone', 'p255', 'half', 'mp255h', 'p375', 'tenth', 'p21', 'mp21', 'p53', 'p53p2',
  'nan', 'pinf', 'ninf', 'maxv', 'tiny', 'p123', 'p7', 'p6', 'u32max', 'hundred', 'twohalf'];
const RADIX = [
  P('undef', undefined), P('null', null), P('two', 2), P('eight', 8), P('ten', 10), P('sixteen', 16), P('36', 36), P('one', 1),
  P('37', 37), P('zero', 0), P('mone', -1), P('frac', 1.5), P('frac29', 2.9), P('str16', '16'), P('nan', NaN), P('str-x', 'x'),
  P('true', true), P('three', 3), P('seven', 7), P('mhalf', -0.5),
];
const DIG = [
  P('undef', undefined), P('null', null), P('zero', 0), P('one', 1), P('two', 2), P('three', 3), P('five', 5), P('p20', 20),
  P('p99', 99), P('p100', 100), P('p101', 101), P('mone', -1), P('nan', NaN), P('pinf', Infinity), P('ninf', -Infinity),
  P('twohalf', 2.5), P('str3', '3'), P('true', true), P('mhalf', -0.5),
];
const DIG2 = [
  P('undef', undefined), P('null', null), P('zero', 0), P('one', 1), P('two', 2), P('three', 3), P('p15', 15), P('p20', 20),
  P('p100', 100), P('p101', 101), P('mone', -1), P('nan', NaN), P('pinf', Infinity), P('twohalf', 2.5), P('str1', '1'), P('mhalf', -0.5),
];
const PREC = [
  P('undef', undefined), P('null', null), P('one', 1), P('two', 2), P('three', 3), P('four', 4), P('p10', 10), P('p21', 21),
  P('p100', 100), P('p101', 101), P('zero', 0), P('mone', -1), P('nan', NaN), P('pinf', Infinity), P('twohalf', 2.5), P('str5', '5'), P('half', 0.5),
];

function numberProtoCases() {
  const cases = [];
  const add = (name, vl, v, al, args) => cases.push({
    id: `${name.toLowerCase()}/${slug(vl)}-${slug(al)}`,
    target: `Number.prototype.${name}`,
    make: () => ({this: v, args: args.map((f) => f())}),
  });
  const grids = {toString: [], toFixed: [], toExponential: [], toPrecision: []};
  NV.forEach((v, i) => {
    const vl = NV_LABELS[i];
    for (const [al, av] of RADIX) grids.toString.push([vl, v, al, av]);
    grids.toString.push([vl, v, 'noarg', null]);
    for (const [al, av] of DIG) grids.toFixed.push([vl, v, al, av]);
    for (const [al, av] of DIG2) grids.toExponential.push([vl, v, al, av]);
    for (const [al, av] of PREC) grids.toPrecision.push([vl, v, al, av]);
    add('valueOf', vl, v, 'noarg', []);
  });
  for (const [name, grid] of Object.entries(grids)) {
    for (const [vl, v, al, av] of sample(grid, 170)) add(name, vl, v, al, av === null ? [] : [av]);
  }
  for (const [label, v] of [['str', 'x'], ['null', null], ['undef', undefined], ['true', true]]) {
    for (const name of ['toString', 'toFixed', 'toExponential', 'toPrecision', 'valueOf']) {
      add(name, label, v, 'arg2', [() => 2]);
    }
  }
  const CTOR = [
    [], [undefined], [null], [true], [false], [''], [' '], [' 12 '], ['0x1F'], ['0X1f'], ['0b101'], ['0o17'], ['1e3'], ['1E+3'],
    ['.5'], ['5.'], ['+5'], ['-5'], ['-0'], ['0x'], ['0b2'], ['1_000'], ['Infinity'], ['-Infinity'], ['infinity'], ['1e400'],
    [' 9 '], ['﻿8'], ['abc'], [-0], [NaN], [5e-324], [1e21], ['\u000b7\u000c'], ['12px'], ['- 5'], ['0.0000001'],
    ['1e-7'], ['0.0000000000000000000000000000001'], ['9007199254740993'],
  ];
  CTOR.forEach((args, i) => cases.push({
    id: `ctor/${String(i).padStart(2, '0')}`,
    target: 'Number',
    static: true,
    make: () => ({args: args.slice()}),
  }));
  for (const name of ['EPSILON', 'MAX_SAFE_INTEGER', 'MAX_VALUE', 'MIN_SAFE_INTEGER', 'MIN_VALUE', 'NaN', 'NEGATIVE_INFINITY', 'POSITIVE_INFINITY']) {
    cases.push({id: `const/${name.toLowerCase()}`, target: `Number.${name}`, get: true, make: () => ({})});
  }
  return cases;
}

// ---------------------------------------------------------------- Math (extra edge sets; O2 covers the grids)

const APPROX = new Set(['cbrt', 'exp', 'expm1', 'log', 'log2', 'log10', 'log1p', 'sin', 'cos', 'tan', 'asin', 'acos', 'atan',
  'sinh', 'cosh', 'tanh', 'asinh', 'acosh', 'atanh', 'pow', 'atan2']);
const MATH_EDGE = [
  0, -0, 0.5, -0.5, 1.5, -1.5, 2.5, -2.5, 3.5, -3.5, 4503599627370495.5, 4503599627370496, -4503599627370497, 2 ** -1074,
  2 ** -149, 2 ** 127, 2 ** 128, -(2 ** 128), 0.49999999999999994, -0.49999999999999994, 1e-300, 4294967295.5, -2147483649,
  2 ** 32, 2 ** 31 - 1, 2 ** 31, 0.1, 0.7, -0.7, 9.5, -9.5,
];

function mathCases() {
  const cases = [];
  const unary = ['abs', 'ceil', 'floor', 'round', 'trunc', 'sign', 'fround', 'clz32', 'sqrt', 'cbrt', 'sin', 'cos', 'exp', 'log'];
  for (const fn of unary) {
    for (const [i, x] of MATH_EDGE.entries()) {
      cases.push({
        id: `${fn.toLowerCase()}/edge-${String(i).padStart(2, '0')}`,
        target: `Math.${fn}`,
        static: true,
        approx: APPROX.has(fn),
        make: () => ({args: [x]}),
      });
    }
  }
  for (const n of [3, 4, 5, 6, 7, 8]) {
    for (let k = 0; k < 4; k++) {
      const values = [];
      for (let j = 0; j < n; j++) values.push(((k + 1) * (j + 3)) * (k % 2 === 0 ? 1 : -1) * 0.5 + (j === 0 ? 1e150 : 0));
      cases.push({id: `hypot/${n}-${k}`, target: 'Math.hypot', static: true, make: () => ({args: values.slice()})});
    }
  }
  const mixed = [
    [1, '2', true, null], [NaN, 1, 2], [Infinity, NaN], [-0, 0], [0, -0, 1], ['x', 1], [undefined, 1],
  ];
  mixed.forEach((args, i) => {
    cases.push({id: `max/mixed-${i}`, target: 'Math.max', static: true, make: () => ({args: args.slice()})});
    cases.push({id: `min/mixed-${i}`, target: 'Math.min', static: true, make: () => ({args: args.slice()})});
    cases.push({id: `hypot/mixed-${i}`, target: 'Math.hypot', static: true, make: () => ({args: args.slice()})});
  });
  for (const [a, b] of [[2 ** 31, 2], [-1, 2 ** 31], [0x7fffffff, 0x7fffffff], [0xffffffff, 0xffffffff], [3, -0], ['7', '9'], [1.9, 2.9]]) {
    cases.push({id: `imul/${cases.length}`, target: 'Math.imul', static: true, make: () => ({args: [a, b]})});
  }
  for (const [y, x] of [[0, -0], [-0, 0], [-0, -0], [1, 0], [-1, 0], [0, -1], [Infinity, Infinity], [-Infinity, -Infinity], [1, -Infinity], [NaN, 1]]) {
    cases.push({id: `atan2/${cases.length}`, target: 'Math.atan2', static: true, approx: true, make: () => ({args: [y, x]})});
  }
  for (const [b, e] of [[2, 0.5], [-2, 3], [-2, 0.5], [0, -1], [-0, -2], [-0, -3], [NaN, NaN], [2, NaN], [Infinity, -1], [-Infinity, 3]]) {
    cases.push({id: `pow/${cases.length}`, target: 'Math.pow', static: true, approx: true, make: () => ({args: [b, e]})});
  }
  return cases;
}

// ---------------------------------------------------------------- build

export function buildSuites() {
  seed = 0x2545f491;
  return [
    {file: 'string-proto.json', suite: defineSuite('string-proto', stringProtoCases())},
    {file: 'string-replace.json', suite: defineSuite('string-replace', stringReplaceCases())},
    {file: 'string-statics.json', suite: defineSuite('string-statics', stringStaticCases())},
    {file: 'number-proto.json', suite: defineSuite('number-proto', numberProtoCases())},
    {file: 'math-extra.json', suite: defineSuite('math-extra', mathCases())},
  ];
}

// Compiled and replay-checked text of every suite, without writing files.
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

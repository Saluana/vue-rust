// WP-B2 corpora (Array, ECMA-262 23.1; V8 is the oracle): tests/language/builtins/array-{statics,methods,callbacks}.json.
// Deterministic: fixed inputs and a fixed LCG for the sort inputs; no clock, no randomness. Every expected value is
// produced by V8 when a suite is compiled (tooling/oracle/builtin-corpus.mjs). Callback presets are used on every
// CALLBACK_ARITY target that takes a callback. Presets cannot mutate the array or return holes; those behaviours are
// native-callback unit tests in crates/value/tests/builtins_b2_callbacks.rs.
//   node tests/language/gen/gen-b2.mjs [--out <dir>]        (default: tests/language/builtins)
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {compileSuite, defineSuite, renderSuite, writeSuite} from '../../../tooling/oracle/builtin-corpus.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const DEFAULT_OUT = path.resolve(here, '../builtins');

const P = (label, value) => [label, () => value];
const F = (label, factory) => [label, factory];

// `this` values, fresh on every call. Labels are case-id segments (lowercase, digits, '-').
const RECEIVERS = [
  F('empty', () => []),
  F('one', () => [7]),
  F('mixed', () => [3, 'x', null, undefined, true]),
  F('holes', () => [, 2, , 4]),
  F('lead-hole', () => [, , 3]),
  F('nested', () => [[1, [2]], 3]),
  F('nums', () => [5, 1, 10, 2]),
  F('strs', () => ['b', 'a', 'c']),
  F('dups', () => [1, 2, 1, NaN, 2]),
  F('nan-zero', () => [NaN, 0, -0]),
  F('extra-prop', () => Object.assign([1, 2], {label: 'x'})),
  F('frozen', () => Object.freeze([1, 2, 3])),
  F('frozen-one', () => Object.freeze([9])),
  F('frozen-empty', () => Object.freeze([])),
  F('frozen-hole', () => Object.freeze([, 1])),
  F('sealed', () => Object.seal([1, 2])),
  F('nonext', () => Object.preventExtensions([1, 2])),
  F('arraylike', () => ({length: 3, 0: 'a', 2: 'c'})),
  F('arraylike-str', () => ({length: '2', 0: 1, 1: 2})),
  F('arraylike-neg', () => ({length: -4, 0: 'x'})),
  F('arraylike-nan', () => ({length: NaN})),
  F('arraylike-empty', () => ({})),
  F('arraylike-null-proto', () => Object.assign(Object.create(null), {length: 2, 0: 'p'})),
  P('str', 'abc'),
  P('str-empty', ''),
  P('num', 5),
  P('bool', true),
  P('undef', undefined),
  P('null', null),
];
const RECV = (label) => RECEIVERS.find(([l]) => l === label);
// A smaller set for the wide cross products.
const SMALL = ['empty', 'mixed', 'holes', 'lead-hole', 'nums', 'extra-prop', 'frozen', 'arraylike', 'arraylike-neg',
  'str', 'undef', 'null', 'nan-zero'].map(RECV);
const ALL = RECEIVERS;

// Arguments as plain data; copied fresh for each make().
const fresh = (v) => (Array.isArray(v) ? v.map(fresh)
  : v !== null && typeof v === 'object' ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, fresh(x)])) : v);

const N = [0, 1, -1, 2, -2, 10, -10, NaN, Infinity, -Infinity, 1.5, '1', undefined, null, true];
const NUM_SETS = N.map((x) => [x]);
const NON_CALLABLE = [[], [undefined], [null], [5], ['x'], [{}], [[]], [true]];

const ARG_SETS = {
  at: NUM_SETS,
  concat: [[], [1], [[2, 3]], [[, 9]], ['s'], [null], [{length: 1, 0: 'q'}], [[[1]]], [1, 2], [[1], [2]], [undefined],
    [[]], [[1, , 3], 4]],
  fill: [[0], [0, 1], [0, -2], [9, 1, -1], [7, undefined, 2], [7, 0, 0], [0, 10], [0, NaN, 2], [0, -Infinity],
    [0, Infinity], [0, 2, Infinity], [1, '1', '2'], [], [8, -10, -1], [3, 1.5]],
  flat: [[], [0], [1], [2], [-1], [Infinity], [NaN], ['1'], [undefined], [null], [true], [1.9]],
  includes: [[1], [undefined], [NaN], [0], [-0], ['1'], [null], [1, 1], [1, -1], [1, 10], [1, -10], [1, NaN],
    [undefined, 2], [NaN, 0], [1, -Infinity], [1, Infinity], [], [2, 2.5]],
  indexOf: [[1], [undefined], [NaN], [0], [-0], ['1'], [null], [1, 1], [1, -1], [1, 10], [1, -10], [1, NaN],
    [undefined, 2], [NaN, 0], [1, -Infinity], [1, Infinity], [], [2, 2.5]],
  lastIndexOf: [[1], [1, undefined], [1, null], [1, 0], [1, -1], [1, -10], [1, 10], [undefined], [NaN], [1, Infinity],
    [1, -Infinity], [1, 2], [1, '2'], [], [undefined, undefined], [0, 1.5]],
  join: [[], [undefined], [null], ['-'], [''], [0], [false], ['é'], [{}], [[9]], [' / ']],
  push: [[], [1], [1, 2], [undefined], [[9]], [null, {}]],
  unshift: [[], [0], [0, 1], [undefined], [[7]]],
  slice: [[], [1], [-1], [1, 2], [-2, -1], [0, Infinity], [Infinity], [-Infinity], [2, 1], [NaN, NaN], [undefined, 2],
    [1, undefined], ['1', '2'], [0, -(2 ** 53)], [2 ** 53, 1], [-(2 ** 53), 2]],
  splice: [[], [0], [1], [-1], [0, 1], [1, 0], [1, 1, 'x'], [0, 0, 'a', 'b'], [-1, 1, 'z'], [1, undefined], [10, 1, 'q'],
    [0, Infinity], [undefined, 1], [0, -1], ['1', '1'], [NaN, 1], [1, 100, 7], [2 ** 53, 1], [-2, 5, 'k']],
  toSpliced: [[], [0], [1], [1, 1], [1, 0, 'x'], [-1, 1, 'z'], [0, 2, 9, 9], [1, undefined], [5], [-5, 1],
    [0, Infinity], [1, -1], [undefined, 1]],
  toSorted: [[], [undefined], [5], [null], ['x'], [undefined, 1]],
  with: [[0, 'x'], [-1, 'y'], [1, undefined], [5, 1], [-5, 1], [NaN, 2], ['1', 3], [Infinity, 1], [1.5, 'f'], [-0, 'z'],
    [0]],
};

// Methods that take a callback: only the non-callable arguments here (the callable ones are native, see the Rust
// test, or presets in the callbacks suite for map and reduce).
const CALLBACK_METHODS = ['every', 'filter', 'find', 'findIndex', 'findLast', 'findLastIndex', 'flatMap', 'forEach',
  'some', 'reduce', 'reduceRight'];

function statics() {
  const cases = [];
  const add = (id, target, make, extra = {}) => cases.push({id, target, make, ...extra});
  const ctorArgs = [[], [0], [1], [3], [-0], ['3'], [undefined], [null], [true], [1, 2], ['a'], [NaN], [1.5], [-1],
    [2 ** 32], [{}], [[]], [[1]], ['', 1], [5, 'x', null]];
  ctorArgs.forEach((args, i) => add(`new-array/${String(i).padStart(2, '0')}`, 'new:Array',
    () => ({args: fresh(args)})));
  const isArr = [[[]], [[1]], [{}], [null], [undefined], ['abc'], [5], [true], [Array.prototype], [[[]]],
    [{length: 0}], [Object.freeze([1])], [Object.create(null)], [[, 1]]];
  isArr.forEach((args, i) => add(`is-array/${String(i).padStart(2, '0')}`, 'Array.isArray',
    () => ({args: fresh(args)}), {static: true}));
  const ofArgs = [[], [1], [1, 2, 3], [undefined], [null], [[1]], ['a', {}], [NaN, -0], [undefined, undefined],
    [1, 'two', [3], {four: 4}, null]];
  ofArgs.forEach((args, i) => add(`of/${String(i).padStart(2, '0')}`, 'Array.of',
    () => ({args: fresh(args)}), {static: true}));
  const fromArgs = [[[]], [[1, 2]], [[1, , 3]], [''], ['a😀b'], ['\uD83D'], ['😀'], [{length: 2}],
    [{length: '3', 1: 'x'}], [{length: -1}], [{length: NaN}], [{length: 1.9, 0: 'z'}], [null], [undefined], [5], [true],
    [[1], undefined], [[1], 5], [{length: 2}, null], [[1, 2], undefined, 7], [{length: 2, 0: [1], 1: {}}],
    [Object.freeze(['f'])], [{length: 2 ** 33}], [Object.assign([1], {extra: 2})]];
  fromArgs.forEach((args, i) => add(`from/${String(i).padStart(2, '0')}`, 'Array.from',
    () => ({args: fresh(args)}), {static: true}));
  return cases;
}

function methods() {
  const cases = [];
  const add = (method, recvLabel, recv, argSets, mk) => argSets.forEach((args, i) => cases.push({
    id: `${method.toLowerCase()}/${recvLabel}/a${String(i).padStart(2, '0')}`,
    target: `Array.prototype.${method}`,
    make: () => ({this: recv(), args: fresh(args)}),
  }));
  const plan = [
    ['at', SMALL], ['concat', SMALL], ['flat', SMALL], ['includes', SMALL], ['indexOf', SMALL],
    ['lastIndexOf', SMALL], ['join', SMALL], ['slice', SMALL], ['with', SMALL], ['toSpliced', SMALL],
    ['toSorted', SMALL], ['fill', ALL], ['splice', ALL], ['push', ALL], ['unshift', ALL],
  ];
  for (const [method, recvs] of plan) {
    for (const [label, recv] of recvs) add(method, label, recv, ARG_SETS[method]);
  }
  // Mutators and copies with no arguments: every receiver.
  for (const method of ['pop', 'shift', 'reverse', 'toReversed', 'toString']) {
    for (const [label, recv] of ALL) add(method, label, recv, [[]]);
  }
  // Callback methods: non-callable callbacks (TypeError before any element is read).
  for (const method of CALLBACK_METHODS) {
    for (const [label, recv] of SMALL) add(method, label, recv, NON_CALLABLE);
  }
  // Iterator methods: the iterator object itself is not encodable, so only the receiver TypeErrors are recorded.
  for (const method of ['keys', 'values', 'entries']) {
    for (const label of ['undef', 'null']) add(method, label, RECV(label)[1], [[]]);
  }
  return cases;
}

// Deterministic integers in [-100, 99] (fixed LCG).
function numbers(n, seed) {
  let s = seed >>> 0;
  return Array.from({length: n}, () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return (s % 200) - 100;
  });
}

function callbacks() {
  const cases = [];
  // Sorted arrays stay below 64 elements: the comparator call log is part of the post state, and V8's call order is
  // matched exactly below that size (array/sort.rs).
  const SIZES = [0, 1, 2, 3, 5, 7, 8, 9, 12, 16, 31, 40, 63];
  // Receivers of the callback cases, fresh on every call (labels from RECEIVERS).
  const CB_RECV = ['nums', 'holes', 'mixed', 'strs', 'empty', 'one', 'lead-hole', 'dups', 'nested', 'arraylike'].map(RECV);
  const CB_WIDE = [...CB_RECV, RECV('str'), RECV('frozen')];
  const FIND_PRESETS = ['isEven', 'isTruthy', 'identity', 'index', 'pair', 'throwAt2'];
  const BY_METHOD = {
    forEach: ['identity', 'index', 'pair', 'keyOf', 'double', 'throwAt2', 'cmpThrow'],
    map: ['identity', 'double', 'isEven', 'isTruthy', 'index', 'pair', 'toArr2', 'keyOf', 'throwAt2'],
    filter: ['isEven', 'isTruthy', 'identity', 'index', 'double', 'throwAt2'],
    some: ['isEven', 'isTruthy', 'identity', 'index', 'throwAt2', 'cmpThrow'],
    every: ['isEven', 'isTruthy', 'identity', 'index', 'throwAt2', 'cmpThrow'],
    find: FIND_PRESETS,
    findIndex: FIND_PRESETS,
    findLast: FIND_PRESETS,
    findLastIndex: FIND_PRESETS,
    flatMap: ['pair', 'toArr2', 'identity', 'index', 'isEven', 'throwAt2'],
  };
  for (const [method, presets] of Object.entries(BY_METHOD)) {
    const recvs = method === 'map' || method === 'forEach' ? CB_WIDE : CB_RECV;
    for (const preset of presets) {
      for (const [label, recv] of recvs) {
        cases.push({
          id: `${method.toLowerCase()}/${preset.toLowerCase()}/${label}`,
          target: `Array.prototype.${method}`,
          cbs: [null, preset],
          make: () => ({this: recv(), args: [undefined]}),
        });
      }
    }
  }
  // reduce and reduceRight: no initial value, an initial zero, an initial array.
  const REDUCE_RECV = ['nums', 'holes', 'strs', 'empty', 'mixed', 'arraylike', 'one'].map(RECV);
  const REDUCE_FORMS = [['noinit', []], ['init-zero', [0]], ['init-arr', [[]]]];
  for (const method of ['reduce', 'reduceRight']) {
    for (const preset of ['sum', 'concat', 'throwAt2', 'index']) {
      for (const [label, recv] of REDUCE_RECV) {
        for (const [form, extra] of REDUCE_FORMS) {
          cases.push({
            id: `${method.toLowerCase()}/${preset.toLowerCase()}/${label}/${form}`,
            target: `Array.prototype.${method}`,
            cbs: [null, preset, ...extra.map(() => null)],
            make: () => ({this: recv(), args: [undefined, ...extra.map(fresh)]}),
          });
        }
      }
    }
  }
  // sort and toSorted with a comparator preset (cmpThrow: the array is unchanged after the throw).
  const SORT_PRESETS = ['cmpNum', 'cmpDesc', 'cmpStr', 'cmpThrow'];
  for (const method of ['sort', 'toSorted']) {
    for (const preset of SORT_PRESETS) {
      for (const n of SIZES) {
        cases.push({
          id: `${method.toLowerCase()}/${preset.toLowerCase()}/n${String(n).padStart(2, '0')}`,
          target: `Array.prototype.${method}`,
          cbs: [null, preset],
          make: () => ({this: numbers(n, 1000 + n), args: [undefined]}),
        });
      }
      for (const label of ['holes', 'mixed', 'dups', 'lead-hole', 'strs', 'nums']) {
        const recv = RECV(label)[1];
        cases.push({
          id: `${method.toLowerCase()}/${preset.toLowerCase()}/${label}`,
          target: `Array.prototype.${method}`,
          cbs: [null, preset],
          make: () => ({this: recv(), args: [undefined]}),
        });
      }
    }
  }
  // Array.from(items, mapFn): the mapper receives (value, index); the items are iterables or array-likes.
  const FROM_ITEMS = [
    F('arr-nums', () => [5, 1, 10, 2]),
    F('arr-holes', () => [, 2, , 4]),
    F('arr-mixed', () => [3, 'x', null, undefined, true]),
    F('arr-empty', () => []),
    F('arr-nested', () => [[1, [2]], 3]),
    F('arr-lead-hole', () => [, , 3]),
    F('arr-dups', () => [1, 2, 1, NaN, 2]),
    P('str-abc', 'abc'),
    P('str-pair', 'a\u{1F600}b'),
    F('arraylike', () => ({length: 3, 0: 'a', 2: 'c'})),
    P('num', 5),
  ];
  for (const preset of ['identity', 'double', 'index', 'pair', 'toArr2', 'keyOf', 'throwAt2', 'isEven']) {
    for (const [label, items] of FROM_ITEMS) {
      cases.push({
        id: `from/${preset.toLowerCase()}/${label}`,
        target: 'Array.from',
        static: true,
        cbs: [null, preset],
        make: () => ({args: [items(), undefined]}),
      });
    }
  }
  return cases;
}

// ---------------------------------------------------------------- build

export function buildSuites() {
  return [
    {file: 'array-statics.json', suite: defineSuite('array-statics', statics())},
    {file: 'array-methods.json', suite: defineSuite('array-methods', methods())},
    {file: 'array-callbacks.json', suite: defineSuite('array-callbacks', callbacks())},
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

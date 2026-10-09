// WP-B6 corpora: tests/language/builtins/json-parse.json and json-stringify.json. Every expected value is recorded by
// V8 when a suite is compiled (tooling/oracle/builtin-corpus.mjs); nothing here is a hand-written expectation.
// Deterministic: fixed inputs, no clock, no randomness.
//   node tests/language/gen/gen-b6.mjs [--out <dir>]        (default: tests/language/builtins)
// The library's TARGETS must list JSON.parse and JSON.stringify (a maintainer change, see the WP-B6 report).
// Escapes: a JSON text that contains the six characters of a \uXXXX escape is written as '\\uXXXX' in a normal string;
// raw code units (lone surrogates, U+2028, DEL, NBSP) are JS escapes of the string itself.
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {defineSuite, writeSuite} from '../../../tooling/oracle/builtin-corpus.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const argOut = process.argv.indexOf('--out');
const OUT = argOut === -1 ? path.resolve(here, '../builtins') : path.resolve(process.argv[argOut + 1]);

// ---------------------------------------------------------------- JSON.parse

const PARSE_TEXTS = [
  // numbers
  ['num/zero', '0'], ['num/neg-zero', '-0'], ['num/one', '1'], ['num/neg-one', '-1'], ['num/frac', '1.5'],
  ['num/neg-frac', '-1.5'], ['num/exp-e0', '1e0'], ['num/exp-upper-e0', '1E0'], ['num/exp-plus', '1e+0'],
  ['num/exp-minus', '1e-0'], ['num/half', '0.5'], ['num/ten', '10'], ['num/exp-21', '1e21'], ['num/exp-m7', '1e-7'],
  ['num/frac-exp', '123.456e-2'], ['num/max', '1.7976931348623157e308'], ['num/overflow-to-inf', '1.7976931348623159e308'],
  ['num/min-subnormal', '5e-324'], ['num/below-min', '2e-324'], ['num/long-digits', '4.9406564584124654e-324'],
  ['num/above-2p53', '9007199254740993'], ['num/point-one', '0.1'], ['num/classic-sum', '0.30000000000000004'],
  ['num/neg-zero-frac', '-0.0'], ['num/overflow', '1e400'], ['num/neg-overflow', '-1e400'], ['num/underflow', '1e-400'],
  ['num/zero-exp', '0e5'], ['num/big-int', '123456789012345678901234567890'], ['num/max-safe', '9007199254740991'],
  ['num/neg-exp-upper', '-2.5E-3'], ['num/zero-exp-plus', '0E+0'],
  ['num/long-fraction', '3.14159265358979323846264338327950288'],
  ['num-bad/leading-zero', '01'], ['num-bad/neg-leading-zero', '-01'], ['num-bad/double-zero', '00'],
  ['num-bad/minus-only', '-'], ['num-bad/trailing-dot', '1.'], ['num-bad/leading-dot', '.1'], ['num-bad/exp-missing', '1e'],
  ['num-bad/exp-sign-missing', '1e+'], ['num-bad/plus', '+1'], ['num-bad/infinity', 'Infinity'],
  ['num-bad/neg-infinity', '-Infinity'], ['num-bad/nan', 'NaN'], ['num-bad/hex', '0x1F'], ['num-bad/underscore', '1_000'],
  ['num-bad/minus-space', '- 1'], ['num-bad/double-dot', '1..2'], ['num-bad/exp-double', '1e5e5'], ['num-bad/minus-dot', '-.5'],
  // literals
  ['lit/true', 'true'], ['lit/false', 'false'], ['lit/null', 'null'], ['lit-bad/title-true', 'True'],
  ['lit-bad/upper-null', 'NULL'], ['lit-bad/truncated', 'tru'], ['lit-bad/nul', 'nul'],
  ['lit-bad/trailing-letters', 'nullx'], ['lit-bad/two-literals', 'true false'], ['lit-bad/undefined', 'undefined'],
  ['lit-bad/fals', 'fals'],
  // strings: JSON escapes (the text contains backslashes)
  ['str/empty', '""'], ['str/ascii', '"abc"'], ['str/escape-quote', '"\\""'], ['str/escape-backslash', '"\\\\"'],
  ['str/escape-slash', '"\\/"'], ['str/escape-b', '"\\b"'], ['str/escape-f', '"\\f"'], ['str/escape-n', '"\\n"'],
  ['str/escape-r', '"\\r"'], ['str/escape-t', '"\\t"'], ['str/escape-u-ascii', '"\\u0041"'],
  ['str/escape-u-lower', '"\\u00e9"'], ['str/escape-u-upper', '"\\u00E9"'], ['str/escape-u-nul', '"\\u0000"'],
  ['str/escape-u-us', '"\\u001f"'], ['str/escape-pair', '"\\uD83D\\uDE00"'], ['str/escape-lone-high', '"\\uD800"'],
  ['str/escape-lone-low', '"\\uDC00"'], ['str/escape-reversed-pair', '"\\uDE00\\uD83D"'],
  ['str/many-escapes', '"\\t\\n\\u0009x"'], ['str/escape-u-mixed-case', '"\\uAbCd"'],
  // strings: raw code units in the text
  ['str/raw-unicode', '"é中"'], ['str/raw-emoji', '"😀"'], ['str/raw-lone-high', '"\ud800"'],
  ['str/raw-lone-low', '"\udc00"'], ['str/raw-del', '"\x7f"'], ['str/raw-line-separator', '" "'],
  ['str/raw-space', '"a b"'],
  ['str-bad/raw-nul', '"\u0000"'], ['str-bad/raw-newline', '"a\nb"'], ['str-bad/raw-tab', '"a\tb"'],
  ['str-bad/x-escape', '"\\x41"'], ['str-bad/short-u', '"\\u12"'], ['str-bad/non-hex-u', '"\\u00G0"'],
  ['str-bad/apos-escape', '"\\\'"'], ['str-bad/a-escape', '"\\a"'], ['str-bad/unterminated', '"abc'],
  ['str-bad/lone-quote', '"'], ['str-bad/single-quotes', "'a'"], ['str-bad/backtick', '`a`'],
  ['str-bad/escape-at-end', '"abc\\'],
  // whitespace: only space, tab, LF and CR are JSON whitespace
  ['ws/space', ' 1 '], ['ws/tab-lf-cr', '\t\r\n1\n'], ['ws/around-array', ' [ 1 , 2 ] '],
  ['ws-bad/nbsp', ' 1'], ['ws-bad/bom', '﻿1'], ['ws-bad/vt', '\u000b1'], ['ws-bad/ff', '\u000c1'],
  ['ws-bad/empty', ''], ['ws-bad/only-spaces', '   '], ['ws-bad/line-separator', ' 1'], ['ws-bad/nel', '\u00851'],
  // arrays
  ['arr/empty', '[]'], ['arr/spaced-empty', '[ ]'], ['arr/one', '[1]'], ['arr/three', '[1,2,3]'],
  ['arr/spaced', '[1 , 2 ]'], ['arr/mixed', '["a",null,true,false,1.5]'], ['arr/nested-empty', '[[]]'],
  ['arr/nested', '[[1],[2,[3]]]'], ['arr/deep-10', '[[[[[[[[[[1]]]]]]]]]]'], ['arr/deep-50', '['.repeat(50) + ']'.repeat(50)],
  ['arr-bad/trailing-comma', '[1,]'], ['arr-bad/leading-comma', '[,1]'], ['arr-bad/double-comma', '[1,,2]'],
  ['arr-bad/missing-comma', '[1 2]'], ['arr-bad/unclosed', '[1'], ['arr-bad/extra-close', '[1]]'],
  ['arr-bad/lone-close', ']'], ['arr-bad/lone-open', '['], ['arr-bad/comma-only', '[,]'],
  ['arr-bad/nested-unclosed', '[[1]'],
  // objects
  ['obj/empty', '{}'], ['obj/spaced-empty', '{ }'], ['obj/one', '{"a":1}'], ['obj/two', '{"a":1,"b":2}'],
  ['obj/order', '{"b":1,"a":2}'], ['obj/dup-last-wins', '{"a":1,"a":2}'],
  ['obj/dup-keeps-position', '{"a":1,"b":2,"a":3}'], ['obj/index-first', '{"2":1,"1":2,"0":3}'],
  ['obj/index-boundary', '{"b":1,"4294967294":2,"4294967295":3,"1":4}'], ['obj/leading-zero-key', '{"01":1,"1":2}'],
  ['obj/negative-key', '{"-1":1,"0":2}'], ['obj/proto-key', '{"__proto__":1}'],
  ['obj/proto-object', '{"__proto__":{"x":1}}'], ['obj/empty-key', '{"":null}'],
  ['obj/escaped-key', '{"a\\u0062":1}'], ['obj/spaced-colon', '{"a" : 1}'],
  ['obj/nested', '{"a":{"b":{"c":[1,{"d":null}]}}}'], ['obj/array-value', '{"a":[]}'],
  ['obj/unicode-key', '{"é":1}'], ['obj/string-value', '{"a":"1"}'],
  ['obj/lone-surrogate-key', '{"\\uD800":1}'],
  ['obj/many-keys', '{"k0":0,"k1":1,"k2":2,"k3":3,"k4":4,"k5":5,"k6":6,"k7":7,"k8":8,"k9":9,"k10":10}'],
  ['obj-bad/unquoted-key', '{a:1}'], ['obj-bad/single-quoted-key', "{'a':1}"], ['obj-bad/trailing-comma', '{"a":1,}'],
  ['obj-bad/missing-value', '{"a":}'], ['obj-bad/missing-colon', '{"a" 1}'], ['obj-bad/missing-comma', '{"a":1 "b":2}'],
  ['obj-bad/comma-only', '{,}'], ['obj-bad/key-only', '{"a"}'], ['obj-bad/unclosed', '{"a":1'],
  ['obj-bad/lone-open', '{'], ['obj-bad/number-key', '{1:2}'], ['obj-bad/leading-comma', '{,"a":1}'],
  ['obj-bad/extra-close', '{}}'],
  // mixed and trailing content
  ['mixed/nested', '{"a":[{"b":[1,2,{"c":"d"}]}],"e":null}'], ['mixed/array-of-objects', '[{},{"a":[]},[{}]]'],
  ['trail/two-values', '1 2'], ['trail/value-then-junk', '1x'], ['trail/junk-after-object', '{}x'],
  ['trail/line-comment', '1 // c'], ['trail/block-comment', '/* c */ 1'], ['trail/semicolon', '1;'],
];

// Arguments that are not strings: ToString(text) comes first, so 1 is "1", -0 is "0", [5] is "5", {} is "[object Object]".
const PARSE_VALUES = [
  ['coerce/number', () => 1], ['coerce/neg-zero', () => -0], ['coerce/large', () => 1e21], ['coerce/nan', () => NaN],
  ['coerce/infinity', () => Infinity], ['coerce/null', () => null], ['coerce/true', () => true],
  ['coerce/false', () => false], ['coerce/array-one', () => [5]], ['coerce/array-quoted', () => ['"a"']],
  ['coerce/array-two', () => [1, 2]], ['coerce/empty-array', () => []], ['coerce/object', () => ({})],
  ['coerce/undefined', () => undefined], ['coerce/fraction', () => 0.1],
];

const parseCases = [];
for (const [id, text] of PARSE_TEXTS) {
  parseCases.push({id, target: 'JSON.parse', static: true, make: () => ({args: [text]})});
}
for (const [id, make] of PARSE_VALUES) {
  parseCases.push({id, target: 'JSON.parse', static: true, make: () => ({args: [make()]})});
}
parseCases.push({id: 'coerce/no-argument', target: 'JSON.parse', static: true, make: () => ({args: []})});

// Revivers. A callback preset in position 1. The presets log their value raw (index, pair, throwAt2), so those get
// scalar texts only. A reviver that is not callable is ignored.
const REVIVE = [
  ['rev/index-number', '1', 'index'], ['rev/index-string', '"x"', 'index'], ['rev/index-null', 'null', 'index'],
  ['rev/index-true', 'true', 'index'], ['rev/throw-at-2', '2', 'throwAt2'], ['rev/no-throw-at-3', '3', 'throwAt2'],
  ['rev/identity-number', '7', 'identity'], ['rev/is-even-number', '4', 'isEven'],
  ['rev/is-truthy-zero', '0', 'isTruthy'], ['rev/double-number', '4', 'double'], ['rev/key-of-number', '4', 'keyOf'],
  ['rev/pair-number', '9', 'pair'], ['rev/to-arr2-number', '5', 'toArr2'],
];
for (const [id, text, preset] of REVIVE) {
  parseCases.push({id, target: 'JSON.parse', static: true, cbs: [null, preset], make: () => ({args: [text, undefined]})});
}
parseCases.push({id: 'rev/not-callable-number', target: 'JSON.parse', static: true, make: () => ({args: ['1', 5]})});
parseCases.push({id: 'rev/not-callable-object', target: 'JSON.parse', static: true, make: () => ({args: ['[1]', {}]})});
parseCases.push({id: 'rev/null-reviver', target: 'JSON.parse', static: true, make: () => ({args: ['{"a":1}', null]})});
parseCases.push({id: 'rev/undefined-reviver', target: 'JSON.parse', static: true, make: () => ({args: ['[1]', undefined]})});

// ---------------------------------------------------------------- JSON.stringify

// A case is an id, one factory per argument (each returns a fresh value), and optionally a callback preset per position.
const SC = (id, args, cbs = null) => ({
  id,
  target: 'JSON.stringify',
  static: true,
  cbs,
  make: () => ({args: args.map((f) => f())}),
});
const V = (v) => () => v; // a primitive argument (fresh by nature)
const F = (f) => f; // an object argument: the factory builds a new value on every call

const stringifyCases = [];
const addS = (id, args, cbs = null) => stringifyCases.push(SC(id, args, cbs));

// Primitives at top level.
const PRIMS = [
  ['undefined', undefined], ['null', null], ['true', true], ['false', false], ['zero', 0], ['neg-zero', -0],
  ['one', 1], ['neg-one', -1], ['frac', 1.5], ['neg-frac', -1.5], ['tenth', 0.1], ['exp-21', 1e21], ['exp-m7', 1e-7],
  ['decimal', 123456789.123], ['two-p53', 2 ** 53], ['neg-two-p53', -(2 ** 53)], ['min-subnormal', 5e-324],
  ['max-value', Number.MAX_VALUE], ['nan', NaN], ['infinity', Infinity], ['neg-infinity', -Infinity], ['exp-100', 1e100],
  ['u255', 255], ['micro', 0.000001], ['big-integer', 999999999999999999999], ['nearly-one', 0.9999999999999999],
  ['empty-string', ''],
];
for (const [slug, v] of PRIMS) addS(`prim/${slug}`, [V(v)]);

// Strings: escapes, controls, lone surrogates, pairs (JS escapes of the code units).
const STRS = [
  ['plain', 'abc'], ['quote', '"'], ['backslash', '\\'], ['controls-named', '\b\f\n\r\t'],
  ['controls-other', '\u0000\u0001\u001f\u007f'], ['separators', '  '], ['accent', 'é'],
  ['emoji', '😀'], ['lone-high', '\ud800'], ['lone-low', '\udc00'], ['pair-units', '𐀀'],
  ['two-lone-high', '\ud800\ud800'], ['lone-in-middle', 'a\udc00b'], ['trailing-high', '\ud83d'], ['bell', 'x\u0007y'],
  ['slash', '/'], ['markup', '<script>'], ['latin-y', 'ÿ'], ['mixed', 'a"b\\c\ndé😀\udc01'],
];
for (const [slug, s] of STRS) addS(`str/${slug}`, [V(s)]);

// Arrays.
addS('arr/empty', [F(() => [])]);
addS('arr/numbers', [F(() => [1, 2, 3])]);
addS('arr/undefined-element', [F(() => [undefined])]);
addS('arr/mixed-primitives', [F(() => [null, undefined, true])]);
addS('arr/all-holes', [F(() => new Array(3))]);
addS('arr/holes-literal', [F(() => [, 1, ,])]);
addS('arr/nested', [F(() => [1, [2, [3]]])]);
addS('arr/special-numbers', [F(() => [NaN, Infinity, -0])]);
addS('arr/strings', [F(() => ['a', 'b'])]);
addS('arr/extra-named-prop', [F(() => Object.assign([1, 2], {x: 1}))]);
addS('arr/deep-50', [F(() => {
  let v = 0;
  for (let i = 0; i < 50; i++) v = [v];
  return v;
})]);
addS('arr/empty-nested', [F(() => [[], [[]], {}])]);

// Objects.
addS('obj/empty', [F(() => ({}))]);
addS('obj/one', [F(() => ({a: 1}))]);
addS('obj/undefined-member', [F(() => ({a: undefined, b: 1}))]);
addS('obj/null-member', [F(() => ({a: null}))]);
addS('obj/insertion-order', [F(() => ({b: 1, a: 2}))]);
addS('obj/index-keys-first', [F(() => ({x: 3, 2: 1, 1: 2}))]);
addS('obj/mixed-keys', [F(() => ({'-1': 1, '01': 2, 1: 3}))]);
addS('obj/empty-key', [F(() => ({'': 1}))]);
addS('obj/quote-in-key', [F(() => ({'a"b': 1}))]);
addS('obj/newline-key', [F(() => ({'\n': 1}))]);
addS('obj/accent-key', [F(() => ({'é': 2}))]);
addS('obj/lone-surrogate-key', [F(() => ({'\ud800': 1}))]);
addS('obj/index-boundary', [F(() => ({4294967295: 1, 4294967294: 2}))]);
addS('obj/nested', [F(() => ({a: {b: {c: [1, {d: null}]}}}))]);
addS('obj/containers', [F(() => ({a: [], b: {}}))]);
addS('obj/shared-reference', [F(() => {
  const x = {v: 1};
  return [x, x];
})]);
addS('obj/cycle-direct', [F(() => {
  const o = {};
  o.self = o;
  return o;
})]);
addS('obj/cycle-array', [F(() => {
  const a = [];
  a.push(a);
  return a;
})]);
addS('obj/cycle-deep', [F(() => {
  const a = {x: {y: []}};
  a.x.y.push(a);
  return a;
})]);
addS('obj/to-json-number', [F(() => ({toJSON: 5, a: 1}))]);
addS('obj/to-json-string', [F(() => ({toJSON: 'x'}))]);
addS('obj/to-json-null', [F(() => ({toJSON: null}))]);
addS('obj/to-json-undefined', [F(() => ({toJSON: undefined, a: 1}))]);
addS('obj/to-json-in-array', [F(() => [{toJSON: 'y'}, 2])]);
addS('obj/many-members', [F(() => ({k0: 0, k1: 1, k2: 2, k3: 3, k4: 4, k5: 5, k6: 6, k7: 7, k8: 8, k9: 9, k10: 10}))]);

// Spaces (argument 2). The replacer is passed as undefined when only a space is given.
const SPACES = [
  ['zero', V(0)], ['one', V(1)], ['two', V(2)], ['four', V(4)], ['ten', V(10)], ['eleven', V(11)], ['twenty', V(20)],
  ['minus-one', V(-1)], ['fraction', V(1.9)], ['nan', V(NaN)], ['infinity', V(Infinity)], ['neg-infinity', V(-Infinity)],
  ['empty-string', V('')], ['space-string', V(' ')], ['tab-string', V('\t')], ['long-string', V('abcdefghijklmnop')],
  ['eleven-chars', V('12345678901')], ['null-space', V(null)], ['true-space', V(true)], ['object-space', F(() => ({}))],
];
const SPACE_VALUES = [
  ['obj', F(() => ({a: 1, b: [1, 2], c: {}, d: []}))],
  ['arr', F(() => [1, [2, [3]]])],
  ['scalar', V('x')],
];
for (const [sslug, space] of SPACES) {
  for (const [vslug, value] of SPACE_VALUES) {
    addS(`space/${vslug}/${sslug}`, [value, V(undefined), space]);
  }
}

// Replacer arrays (argument 1): the property list, deduplicated, numbers and strings only.
const REPL_LISTS = [
  ['a', F(() => ['a'])], ['b-a', F(() => ['b', 'a'])], ['a-a', F(() => ['a', 'a'])], ['one-and-string', F(() => [1, '1'])],
  ['fractional-and-zeros', F(() => [1.5, -0, 0])], ['ignored-items', F(() => [null, true, {}])],
  ['empty', F(() => [])], ['missing', F(() => ['missing'])], ['index-keys', F(() => ['0', 1])],
  ['undefined-item', F(() => [undefined, 'a'])], ['three', F(() => ['x', 'y', 'z'])],
  ['nested-name', F(() => ['a', 'c', 'nope'])],
];
const REPL_VALUES = [
  ['obj', F(() => ({a: 1, b: 2, 1: 3, 0: 4, c: {a: 5, b: 6}}))],
  ['arr', F(() => [{a: 1, b: 2}, {a: 3}])],
  ['scalar', V(7)],
];
for (const [rslug, repl] of REPL_LISTS) {
  for (const [vslug, value] of REPL_VALUES) {
    addS(`repl-list/${vslug}/${rslug}`, [value, repl]);
  }
}
addS('repl-list/with-space', [REPL_VALUES[0][1], REPL_LISTS[0][1], V(2)]);
addS('repl-list/object-replacer', [REPL_VALUES[0][1], F(() => ({}))]);
addS('repl-list/number-replacer', [REPL_VALUES[0][1], V(5)]);
addS('repl-list/string-replacer', [REPL_VALUES[0][1], V('a')]);
addS('repl-list/null-replacer', [REPL_VALUES[0][1], V(null)]);

// Callback presets as the replacer, called as replacer(key, value) with this = holder. The replacer runs on the top-level
// value and on every member. The presets that log their value raw (index, throwAt2) get scalar values only. Presets
// that return arrays built from the key (toArr2, pair) are left out: the replacer would then run on those arrays forever.
const PRESET_ROWS = [
  ['identity', 'identity', F(() => ({a: 1, b: 'x'}))], ['keyof', 'keyOf', F(() => ({a: 1, b: 'x'}))],
  ['istruthy', 'isTruthy', F(() => ({a: 0, b: 'x'}))], ['double', 'double', F(() => [1, 2, 3])],
  ['iseven', 'isEven', F(() => ({a: 2, b: 3}))],
];
for (const [name, preset, value] of PRESET_ROWS) {
  addS(`preset/${name}/object`, [value, V(undefined)], [null, preset]);
}
addS('preset/identity-scalar', [V(5), V(undefined)], [null, 'identity']);
addS('preset/istruthy-scalar', [V(5), V(undefined)], [null, 'isTruthy']);
addS('preset/index-number', [V(5), V(undefined)], [null, 'index']);
addS('preset/index-string', [V('s'), V(undefined)], [null, 'index']);
addS('preset/index-null', [V(null), V(undefined)], [null, 'index']);
addS('preset/index-with-space', [V(7), V(undefined), V(2)], [null, 'index', null]);
addS('preset/index-undefined', [V(undefined), V(undefined)], [null, 'index']);
addS('preset/throwat2-throws', [V(2), V(undefined)], [null, 'throwAt2']);
addS('preset/throwat2-passes', [V(3), V(undefined)], [null, 'throwAt2']);
addS('preset/throwat2-string', [V('two'), V(undefined)], [null, 'throwAt2']);
addS('preset/as-value-function', [V(undefined)], ['identity']);
addS('preset/as-value-function-with-space', [V(undefined), V(undefined), V(2)], ['identity', null, null]);

// Exceptions: a cycle reached through a replacer list, and a throwing replacer.
addS('exc/cycle-with-replacer-list', [F(() => {
  const o = {a: 1};
  o.me = o;
  return o;
}), F(() => ['a', 'me'])]);
addS('exc/throw-replacer-top', [V(2), V(undefined)], [null, 'throwAt2']);

// ---------------------------------------------------------------- suites

const parseSuite = defineSuite('json-parse', parseCases);
const stringifySuite = defineSuite('json-stringify', stringifyCases);
const parseText = writeSuite(path.join(OUT, 'json-parse.json'), parseSuite);
const stringifyText = writeSuite(path.join(OUT, 'json-stringify.json'), stringifySuite);
console.log(`json-parse: ${parseSuite.cases.length} cases, ${parseText.length} bytes`);
console.log(`json-stringify: ${stringifySuite.cases.length} cases, ${stringifyText.length} bytes`);

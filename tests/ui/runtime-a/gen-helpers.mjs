// WP-RT-A helper corpora (tests/ui/runtime-a/corpora/helpers-*.json). Every expected value is produced by the real
// vue@3.5.43 package (production build, through tooling/oracle/builtin-corpus.mjs): compileSuite runs each case in Node
// and replays it from its own pre encoding. Deterministic: fixed inputs, no clock, no randomness.
//   NODE_ENV=production node tests/ui/runtime-a/gen-helpers.mjs [--out <dir>]     (default: tests/ui/runtime-a/corpora)
// Preset callbacks (identity, pair, ...) come from the oracle's callback table; only renderList takes them (cbs).
import {mkdirSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {defineSuite, installVueNamespace, writeSuite} from '../../../tooling/oracle/builtin-corpus.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const outIdx = process.argv.indexOf('--out');
const OUT = outIdx > 0 ? path.resolve(process.argv[outIdx + 1]) : path.join(here, 'corpora');
installVueNamespace();

// Inputs are factories so that every execution gets fresh objects. P: primitive, F: factory.
const P = (label, value) => [label, () => value];
const F = (label, factory) => [label, factory];
const slug = (s) => String(s).toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '') || 'x';

// One case per input: target(...args) with static: true (the owner is not part of pre).
function unary(family, target, inputs) {
  return inputs.map(([label, factory], i) => ({
    id: `${family}/${String(i).padStart(3, '0')}-${slug(label)}`,
    target,
    static: true,
    make: () => ({args: [factory()]}),
  }));
}

function nary(family, target, rows) {
  // rows: [label, () => args[], cbs?]
  return rows.map(([label, make, cbs], i) => {
    const c = {id: `${family}/${String(i).padStart(3, '0')}-${slug(label)}`, target, static: true, make: () => ({args: make()})};
    if (cbs) c.cbs = cbs;
    return c;
  });
}

const holes = () => {
  const a = [1];
  a[3] = 'x';
  return a;
};
const cyc = () => {
  const o = {a: 1};
  o.self = o;
  return o;
};
const cycArr = () => {
  const a = [1];
  a.push(a);
  return a;
};
const ref = (value) => ({__v_isRef: true, value});

// ---------------------------------------------------------------- normalizeClass

const CLASS_INPUTS = [
  P('empty-string', ''), P('word', 'a'), P('spaces', ' a  b '), P('tabs-newlines', 'a\tb\nc'), P('only-spaces', '   '),
  P('three-words', 'x y z'), P('number-zero', 0), P('number-one', 1), P('neg-zero', -0), P('nan', NaN),
  P('true', true), P('false', false), P('null', null), P('undefined', undefined), P('symbol', undefined),
  P('nested-number', 2.5), P('long-string', 'c0 c1 c2 c3 c4 c5 c6 c7 c8 c9'),
  F('empty-array', () => []), F('array-one', () => ['a']), F('array-empty-strings', () => ['a', '', 'b']),
  F('array-falsy-mix', () => [' a ', null, undefined, false, 0, 'b']), F('array-nested', () => [['a', ['b', ['c']]]]),
  F('array-object-falsy', () => [{a: 1, b: 0}]), F('array-empty-object', () => [{}]),
  F('array-numbers', () => [0, -0, NaN, 1]), F('array-empty-nested', () => [[], [''], ['']]), F('array-holes', holes),
  F('array-objects', () => [{x: true}, {y: 'z'}, {z: []}]), F('array-mixed', () => ['a', {b: true}, ['c', {d: false}]]),
  F('object-empty', () => ({})), F('object-true', () => ({a: true})), F('object-mixed', () => ({a: false, b: true})),
  F('object-number-values', () => ({a: 1, b: 0})), F('object-empty-key', () => ({'': true})),
  F('object-spaced-key', () => ({'a b': true, c: 'x'})), F('object-index-keys', () => ({'0': true, '1': false, a: true})),
  F('object-falsy-values', () => ({a: null, b: undefined, c: NaN, d: -0, e: ''})),
  F('object-padded-key', () => ({'  pad  ': true})), F('object-null-proto', () => Object.assign(Object.create(null), {k: true, m: false})),
  F('map-value', () => new Map([['a', 1]])), F('set-value', () => new Set(['a'])),
  F('array-of-numbers-nested', () => [[1, [2, [3, 0]]]]), F('object-many-classes', () => ({a: 1, b: 1, c: 1, d: 1, e: 0, f: 1})),
  F('array-object-and-string', () => ['x', {y: 1}, 'z', {w: 0}]), F('object-unicode-key', () => ({'é': true, 'ü x': 1})),
];

// ---------------------------------------------------------------- normalizeStyle (parseStringStyle through arrays)

const STYLE_STRINGS = [
  'color:red', 'color: red; top: 1px', 'color:red;', ';;color:red;;', 'a', 'a:', ':a', ':', 'a:b:c', 'color: "a;b"',
  "content: 'x;y'", 'background: url(a;b.png)', 'width: calc(1px + (2px; 3px))', 'a: f(x; y); b: 2',
  '/* c */ color: red', 'color: /* x */ red', 'color: red /* a */ ; top: /* b */ 1px', 'color: red\n; top: 2px',
  '  padding  :  0  ', 'a:1;b:2;a:3', '"a:b": c', "'a:b': c", 'a: "unterminated', 'a: b \\"c; d: e',
  '--custom-prop: {a:b}', 'filter: progid:DXImage(a=1;b=2)', '', ' ', 'key:value;', 'empty:', '__proto__: x',
  'a: b; c', 'x:1;;y:2', 'k: v /* unterminated', '/* only comment */', 'a:"x\\"y;z"; b:2', 'a: \'it\\\'s;\'; z: 1',
  'margin: 0 auto;padding:1px', 'x: y:z', 'color:#fff;background:#000', 'a:(;);b:1', 'f: a(b;c)d; g: 2',
  'content: "/* not a comment */"; k: 1', 'url: "http://x;y"', 'a:b\n\n;\tc:d', 'width:100%;height:50%',
];
const STYLE_INPUTS = [
  F('object-empty', () => ({})), F('object-color', () => ({color: 'red'})), F('object-numbers', () => ({width: 10, height: '5px'})),
  F('object-zero', () => ({a: 0})), F('object-null', () => ({a: null})), F('object-custom-prop', () => ({'--x': 'y'})),
  F('object-undefined', () => ({color: undefined})), P('string-plain', 'color: red'), P('string-empty', ''),
  P('number', 5), P('true', true), P('false', false), P('null', null), P('undefined', undefined), P('zero', 0), P('nan', NaN),
  F('array-empty', () => []), F('array-empty-object', () => [{}]),
  F('array-holes', () => { const a = [{a: 1}]; a[2] = {b: 2}; return a; }),
];
for (const s of STYLE_STRINGS) STYLE_INPUTS.push(F(`parse-${s}`, () => [s]));
STYLE_INPUTS.push(F('parse-two-strings', () => ['a: 1', 'b: 2']), F('parse-with-object', () => ['a: 1', {b: 2}]));
STYLE_INPUTS.push(F('nested-array', () => [['color: red', [{top: 1}]]]), F('mixed-null-numbers', () => [null, 0, 'x: 1', undefined, true]));
STYLE_INPUTS.push(F('overwrite-keys', () => [{a: 1}, {a: 2}, 'a: 3']), F('object-with-object', () => [{a: {b: 1}}]));

// ---------------------------------------------------------------- toDisplayString

const DISPLAY_INPUTS = [
  P('string', 'str'), P('empty-string', ''), P('zero', 0), P('neg-zero', -0), P('nan', NaN), P('infinity', Infinity),
  P('neg-infinity', -Infinity), P('fraction', 1.5), P('true', true), P('false', false), P('null', null),
  P('undefined', undefined), P('big-exponent', 1e21), P('small', -1e-7), P('int', 12345), P('max-safe', 9007199254740991),
  F('array-empty', () => []),
  F('array-numbers', () => [1, 2, 3]), F('array-nullish', () => ['a', null, undefined]), F('array-object', () => [{a: 1}]),
  F('array-nested', () => [[1, [2]]]), F('array-special-numbers', () => [NaN, -0]), F('array-holes', holes),
  F('array-map', () => [new Map([['k', 1]])]),
  F('object-empty', () => ({})), F('object-mixed', () => ({a: 1, b: 'x', c: [1, 2], d: {e: null}})),
  F('object-index-key', () => ({'0': 'zero'})), F('object-undefined-value', () => ({a: undefined})),
  F('object-unicode', () => ({'é': 'ü'})),
  F('object-nested-arrays', () => ({list: [[1], [2, [3]]], empty: []})), F('object-numbers', () => ({n: -0, m: NaN, k: 1e21})),
  F('map-empty', () => new Map()), F('map-string-keys', () => new Map([['a', 1], ['b', 'two']])),

  F('map-object-key', () => new Map([[{}, 2]])), F('map-number-keys', () => new Map([[1, 'one'], [2, 'two']])),
  F('map-nested', () => new Map([['inner', new Map([['x', 1]])]])), F('map-object-values', () => new Map([['o', {p: [1]}]])),
  F('set-empty', () => new Set()), F('set-values', () => new Set([1, 'a'])),
  F('set-nested', () => new Set([[1, 2]])), F('object-containing-map', () => ({m: new Map([['z', null]])})),
  F('object-containing-set', () => ({s: new Set([true])})), F('ref-number', () => ref(5)), F('ref-object', () => ref({x: 1})),
  F('ref-array', () => ref([1])), F('ref-false-flag', () => ({__v_isRef: false, value: 1})),
  F('object-with-ref', () => ({r: ref('v')})), F('array-with-ref', () => [ref(2)]),
  F('circular-object', cyc), F('circular-array', cycArr), F('array-of-objects', () => [{a: 1}, {b: [true, null]}]),
  F('deep', () => ({a: {b: {c: {d: [1, {e: 'deep'}]}}}})), F('object-string-escapes', () => ({q: 'say "hi"\n\t\\'})),
  F('object-control-chars', () => ({c: '\u0001\u001f'})), F('array-strings-escapes', () => ['"', '\\', '\n']),
  F('object-empty-array-values', () => ({a: [], b: {}})), F('array-large', () => Array.from({length: 12}, (_, i) => i)),
  F('object-many-keys', () => ({k1: 1, k2: 2, k3: 3, k4: 4, k5: 5, k6: 6})), F('set-map-mix', () => new Set([new Map([[1, 2]])])),
];

// ---------------------------------------------------------------- camelize / toHandlerKey (capitalize is X in the profile)

const NAME_STRINGS = [
  '', 'a', 'a-b', 'a-b-c', '-a', 'a-', '--a', 'a--b', 'a-1', 'a-_b', 'a-é', 'my-component-name', 'ABC-def', '-', 'a-B',
  'x-yy-zz', 'a_b-c', '1-2-3', 'a- b', ' a-b ', 'a-é', 'ünïcode-ñ', 'click', 'onClick', 'update:modelValue', 'Ab',
  '9lives', 'ß', 'ǆ-x', 'a-😀', '-ab-cd-', 'Mixed-Case-Words', 'trailing-', '_-_', 'a\tb-c',
];
const NAME_NONSTRINGS = [
  P('undefined', undefined), P('null', null), P('number', 1), P('zero', 0), P('nan', NaN), P('true', true),
  F('object', () => ({})), F('array', () => ['zz-qq']),
];

// ---------------------------------------------------------------- mergeProps

const MERGE_OBJECTS = [
  ['empty', () => ({})], ['class-string', () => ({class: 'a'})], ['class-array', () => ({class: ['b', {c: true}]})],
  ['class-empty', () => ({class: ''})], ['style-string', () => ({style: 'color:red'})],
  ['style-array', () => ({style: [{top: 1}, 'left: 2px']})], ['style-object', () => ({style: {width: 3}})],
  ['on-string', () => ({onClick: 'h1'})], ['on-number', () => ({onKeyup: 7})], ['on-array', () => ({onFoo: ['x', 'y']})],
  ['on-null', () => ({onBar: null})], ['on-undefined-key', () => ({onUpdate: undefined})],
  ['update-model-undefined', () => ({'onUpdate:modelValue': undefined})], ['update-model-value', () => ({'onUpdate:modelValue': 'v'})],
  ['plain-key', () => ({id: 'k'})], ['plain-number', () => ({tabindex: 0})], ['empty-key', () => ({'': 'dropped'})],
  ['override-key', () => ({title: 'second'})], ['class-and-style', () => ({class: 'c', style: 'top:0'})],
  ['on-and-class', () => ({onSave: 'save', class: ['d']})], ['null-prop', () => ({role: null})], ['false-prop', () => ({hidden: false})],
  ['on-empty-string', () => ({onEmpty: ''})], ['index-key', () => ({0: 'zero'})], ['class-nested', () => ({class: [['e'], 'f']})],
];

// ---------------------------------------------------------------- renderList

const LIST_SOURCES = [
  ['array-empty', () => []], ['array-numbers', () => [1, 2, 3]], ['array-strings', () => ['a', 'b']],
  ['array-holes', () => { const a = [1, 2, 3]; delete a[1]; return a; }], ['array-nullish', () => [null, undefined, false]],
  ['array-nested', () => [[1, 2], [3]]], ['array-object-items', () => [{k: 1}, {k: 2}]], ['array-length-two', () => [0, 0]],
  ['string-abc', () => 'abc'], ['string-empty', () => ''], ['string-surrogate', () => 'a😀'],
  ['string-digits', () => '12'], ['number-zero', () => 0], ['number-one', () => 1], ['number-three', () => 3],
  ['number-five', () => 5], ['number-negative', () => -1], ['number-fraction', () => 1.5], ['number-nan', () => NaN],
  ['number-infinity', () => Infinity], ['number-neg-zero', () => -0], ['number-string', () => '3'],
  ['object-two-keys', () => ({a: 1, b: 2})], ['object-empty', () => ({})], ['object-index-keys', () => ({0: 'z', x: 'y'})],
  ['object-numeric-order', () => ({b: 1, 2: 'two', a: 3, 1: 'one'})], ['map-two', () => new Map([['k', 1], ['j', 2]])],
  ['map-empty', () => new Map()], ['set-three', () => new Set([1, 2, 3])], ['set-empty', () => new Set()],
  ['set-strings', () => new Set(['x', 'y'])], ['map-object-keys', () => new Map([[{}, 'v']])],
  ['boolean-true', () => true], ['null', () => null], ['undefined', () => undefined], ['function-like-string', () => 'x'],
];
const LIST_RENDERERS = ['identity', 'double', 'index', 'pair', 'toArr2', 'keyOf', 'isTruthy', 'isEven'];

// ---------------------------------------------------------------- suites

const CLASS_CASES = unary('class', 'Vue.normalizeClass', CLASS_INPUTS);
const PROPS_CASES = [
  ['props-class-array', () => ({class: ['a', {b: true}]})], ['props-style-string-array', () => ({style: ['color:red', 'top:1px']})],
  ['props-no-class-style', () => ({id: 'x'})], ['props-falsy-class', () => ({class: 0, style: ''})],
  ['props-style-object', () => ({style: {a: 1}})], ['props-empty', () => ({})],
  ['props-null', () => null], ['props-undefined', () => undefined], ['props-string-class', () => ({class: 'keep'})],
  ['props-style-number', () => ({style: 5})], ['props-class-object', () => ({class: {x: 1, y: 0}})],
  ['props-both-nested', () => ({class: [['p']], style: [{q: 1}]})],
].map(([label, factory], i) => ({
  id: `class/props-${String(i).padStart(3, '0')}-${slug(label)}`,
  target: 'Vue.normalizeProps', static: true, make: () => ({args: [factory()]}),
}));
const STYLE_CASES = unary('style', 'Vue.normalizeStyle', STYLE_INPUTS);
const DISPLAY_CASES = unary('display', 'Vue.toDisplayString', DISPLAY_INPUTS);
const CAMEL_CASES = NAME_STRINGS.flatMap((s, i) => [
  {id: `names/camelize-${String(i).padStart(3, '0')}-${slug(s)}`, target: 'Vue.camelize', static: true, make: () => ({args: [s]})},
  {id: `names/tohandler-${String(i).padStart(3, '0')}-${slug(s)}`, target: 'Vue.toHandlerKey', static: true, make: () => ({args: [s]})},
]);
const NAME_NON_CASES = [
  ...unary('names', 'Vue.camelize', NAME_NONSTRINGS).map((c) => ({...c, id: `names/camelize-nonstring-${c.id.split('/')[1]}`})),
  ...unary('names', 'Vue.toHandlerKey', NAME_NONSTRINGS).map((c) => ({...c, id: `names/tohandler-nonstring-${c.id.split('/')[1]}`})),
];
const MERGE_CASES = [];
MERGE_OBJECTS.forEach(([la, fa], i) => {
  MERGE_OBJECTS.forEach(([lb, fb], j) => {
    if ((i * 7 + j * 3) % 4 !== 0 && i !== j) return; // a fixed, spread-out subset of pairs
    MERGE_CASES.push({id: `merge/pair-${la}-${lb}`, target: 'Vue.mergeProps', static: true, make: () => ({args: [fa(), fb()]})});
  });
});
MERGE_OBJECTS.forEach(([la, fa], i) => {
  for (let k = 0; k < 4; k++) {
    const [lb, fb] = MERGE_OBJECTS[(i + 5 * k + 3) % MERGE_OBJECTS.length];
    const [lc, fc] = MERGE_OBJECTS[(i * 3 + k + 11) % MERGE_OBJECTS.length];
    MERGE_CASES.push({id: `merge/triple-${la}-${lb}-${lc}`, target: 'Vue.mergeProps', static: true, make: () => ({args: [fa(), fb(), fc()]})});
  }
});
MERGE_CASES.push(
  {id: 'merge/none', target: 'Vue.mergeProps', static: true, make: () => ({args: []})},
  {id: 'merge/null-first', target: 'Vue.mergeProps', static: true, make: () => ({args: [null, {a: 1}]})},
  {id: 'merge/undefined-middle', target: 'Vue.mergeProps', static: true, make: () => ({args: [{a: 1}, undefined, {b: 2}]})},
  {id: 'merge/single-class', target: 'Vue.mergeProps', static: true, make: () => ({args: [{class: ['a', 'b']}]})},
);
const LIST_CASES = [];
for (const [ls, fs] of LIST_SOURCES) {
  for (const r of LIST_RENDERERS) {
    LIST_CASES.push({
      id: `list/${slug(ls)}-${slug(r)}`,
      target: 'Vue.renderList',
      static: true,
      cbs: [null, r],
      make: () => ({args: [fs(), null]}),
    });
  }
}
for (const [ls, fs] of LIST_SOURCES) {
  LIST_CASES.push({id: `list/${slug(ls)}-no-renderer-arg`, target: 'Vue.renderList', static: true, make: () => ({args: [fs()]})});
}
for (const [ls, fs] of LIST_SOURCES.slice(0, 12)) {
  LIST_CASES.push({id: `list/${slug(ls)}-cache-index`, target: 'Vue.renderList', static: true, cbs: [null, 'pair', null, null], make: () => ({args: [fs(), null, {}, 2]})});
  LIST_CASES.push({id: `list/${slug(ls)}-cache-array`, target: 'Vue.renderList', static: true, cbs: [null, 'keyOf', null, null], make: () => ({args: [fs(), null, [], 0]})});
}
const TO_HANDLERS_INPUTS = [
  ['empty', () => ({})], ['click', () => ({click: 1})], ['onclick', () => ({onClick: 'x'})],
  ['update-model', () => ({'update:modelValue': 'v'})], ['upper-keys', () => ({ABC: 1, def: 2})], ['dash-key', () => ({'a-b': 1})],
  ['empty-key', () => ({'': 'empty'})], ['index-and-name', () => ({'9': 'nine', a: 'x'})], ['null', () => null],
  ['undefined', () => undefined], ['string-ab', () => 'ab'], ['number', () => 5], ['true', () => true],
  ['mixed-case', () => ({aB: 1, Ab: 2, a: 3})], ['unicode', () => ({'é': 1, Ñ: 2})], ['symbols-ignored', () => ({x: 1})],
];
const TO_HANDLERS_CASES = [];
for (const [label, factory] of TO_HANDLERS_INPUTS) {
  for (const preserve of [false, true]) {
    TO_HANDLERS_CASES.push({
      id: `handlers/${slug(label)}-${preserve ? 'preserve' : 'plain'}`,
      target: 'Vue.toHandlers',
      static: true,
      make: () => ({args: [factory(), preserve]}),
    });
  }
}
const SLOTS_CASES = [
  ['empty-both', () => [{}, []]], ['named-number', () => [{}, [{name: 'a', fn: 1}]]],
  ['list-of-entries', () => [{}, [[{name: 'x', fn: 1}, {name: 'y', fn: 2}]]]],
  ['holes-and-null', () => [{}, [undefined, null, {name: 'z', fn: 'v'}]]], ['keyed-entry', () => [{}, [{name: 'k', fn: 3, key: 'kk'}]]],
  ['keyed-falsy-key', () => [{}, [{name: 'k', fn: 3, key: 0}]]], ['existing-slots', () => [{a: 'old'}, [{name: 'a', fn: 'new'}]]],
  ['numeric-name', () => [{}, [{name: 7, fn: 'seven'}]]], ['overwrite-in-list', () => [{}, [[{name: 'd', fn: 1}], [{name: 'd', fn: 2}]]]],
  ['empty-array-entry', () => [{}, [[]]]], ['null-fn', () => [{}, [{name: 'n', fn: null}]]],
  ['object-fn-value', () => [{}, [{name: 'o', fn: {p: 1}}]]], ['symbol-name-string', () => [{}, [{name: 'sym', fn: undefined}]]],
];
const SLOTS_CASE_LIST = SLOTS_CASES.map(([label, factory], i) => ({
  id: `slots/${String(i).padStart(3, '0')}-${slug(label)}`,
  target: 'Vue.createSlots',
  static: true,
  make: () => ({args: factory()}),
}));
const GUARD_CASE_LIST = [
  ['null', () => null], ['undefined', () => undefined], ['empty-object', () => ({})], ['object-with-keys', () => ({a: 1})],
  ['zero', () => 0], ['string', () => 'x'], ['array', () => [1]], ['false', () => false],
].map(([label, factory], i) => ({
  id: `guard/${String(i).padStart(3, '0')}-${slug(label)}`,
  target: 'Vue.guardReactiveProps',
  static: true,
  make: () => ({args: [factory()]}),
}));
const NAMES_CASES = [...CAMEL_CASES, ...NAME_NON_CASES];

const SUITES = [
  ['helpers-class', [...CLASS_CASES, ...PROPS_CASES, ...GUARD_CASE_LIST]],
  ['helpers-style', STYLE_CASES],
  ['helpers-display', DISPLAY_CASES],
  ['helpers-names', NAMES_CASES],
  ['helpers-merge', MERGE_CASES],
  ['helpers-list', [...LIST_CASES, ...TO_HANDLERS_CASES, ...SLOTS_CASE_LIST]],
];

mkdirSync(OUT, {recursive: true});
let total = 0;
for (const [name, cases] of SUITES) {
  const file = path.join(OUT, `${name}.json`);
  const suite = defineSuite(name, cases);
  writeSuite(file, suite);
  total += cases.length;
  console.log(`${name}: ${cases.length} cases -> ${path.relative(process.cwd(), file)}`);
}
console.log(`total: ${total} cases`);

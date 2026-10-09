// WP-B1 seed suites: tests/language/builtins/{object-proto,object-own,function-proto,symbol,boolean,globals}.json.
// Deterministic: fixed inputs, no clock, no randomness. Every expected value is produced by V8 when a suite is compiled
// (tooling/oracle/builtin-corpus.mjs). Nothing here is a hand-written expectation.
//   node tests/language/gen/gen-b1.mjs [--out <dir>]        (default: tests/language/builtins)
// Inputs are built by make() on every call, so object arguments are fresh for each execution. The corpus accepts callback
// presets only on the Array callback methods (CALLBACK_ARITY), and refuses global-kind targets (parseInt, isNaN, ...), so
// those are covered by Rust unit tests (crates/value/tests/builtins_b1_*.rs) instead.
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {defineSuite, writeSuite} from '../../../tooling/oracle/builtin-corpus.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const DEFAULT_OUT = path.resolve(here, '../builtins');

// ---------------------------------------------------------------- inputs
// v(label, value): a primitive; o(label, factory): a fresh object or array; p(name): a callback preset.
const v = (label, value) => ({label, make: () => value});
const o = (label, factory) => ({label, make: factory});

const slug = (s) => String(s).toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '') || 'empty';

// Values usable as `this` of Object.prototype methods (arrays and functions are fine: no toString call happens).
const OBJ_THIS = [
  v('undefined', undefined), v('null', null), v('true', true), v('false', false),
  v('zero', 0), v('neg-zero', -0), v('nan', NaN), v('empty-string', ''), v('string', 'abc'),
  o('plain-empty', () => ({})), o('plain-ab', () => ({a: 1, b: 'x'})), o('plain-mixed', () => ({2: 'two', a: 1, 1: 'one'})),
  o('null-proto', () => Object.assign(Object.create(null), {k: 1})), o('array', () => [1, 2]), o('array-empty', () => []),
  o('array-hole', () => [, 'x']), o('nested', () => ({o: {p: 1}})),
];

// Values whose toLocaleString does not call Number, String or Array methods (those are other WPs).
const LOCALE_THIS = [
  v('undefined', undefined), v('null', null), v('true', true), v('false', false),
  o('plain-empty', () => ({})), o('plain-ab', () => ({a: 1, b: 'x'})), o('null-proto', () => Object.assign(Object.create(null), {k: 1})),
  o('nested', () => ({o: {p: 1}})),
];

// Property keys: the ToPropertyKey conversions of primitives and a plain object.
const KEYS = [
  v('undefined', undefined), v('null', null), v('a', 'a'), v('b', 'b'), v('missing', 'missing'), v('name', 'name'),
  v('length', 'length'), v('to-string', 'toString'), v('zero', 0), v('one', 1), v('two', 2), v('string-zero', '0'),
  v('string-one', '1'), v('neg-zero', -0), v('fraction', 1.5), v('nan', NaN), v('true', true), v('empty', ''),
  v('spaces', 'a b'), o('object', () => ({})), v('neg-one', -1), v('leading-zero', '01'), v('inf', Infinity),
];

const PROTO_ARGS = [
  v('undefined', undefined), v('null', null), v('one', 1), v('string', 'x'), o('plain', () => ({})),
  o('null-proto', () => Object.create(null)), o('array', () => [1]),
];

// Subjects of the Object statics that list own properties (no functions: presets have no `prototype` in the runtime).
const OWN_SUBJECTS = [
  v('undefined', undefined), v('null', null), v('one', 1), v('neg-zero', -0), v('true', true), v('string', 'abc'),
  v('empty-string', ''), o('plain-empty', () => ({})), o('plain-ab', () => ({a: 1, b: 'x'})),
  o('plain-mixed', () => ({2: 'two', a: 1, 1: 'one'})), o('array', () => [1, 'x']), o('array-hole', () => [, 2]),
  o('array-empty', () => []), o('null-proto', () => Object.assign(Object.create(null), {k: 1})),
  o('nested', () => ({a: {b: 1}, c: [1]})),
];

const DESCRIPTOR_KEYS = [
  v('a', 'a'), v('b', 'b'), v('length', 'length'), v('zero', '0'), v('one', '1'), v('missing', 'missing'),
  v('index-zero', 0), v('index-one', 1), v('index-two', 2), v('x', 'x'), v('undefined', undefined), v('null', null),
];

// Items for groupBy must be iterable (arrays), except where the case tests the rejection.
const GROUP_ITEMS = [
  o('numbers', () => [1, 2, 3, 4, 5]), o('strings', () => ['a', 'bb', 'c', 'dd']), o('empty', () => []),
  o('mixed', () => [1, '1', 2, 'x', true]), o('holes', () => [1, , 3]), o('objects', () => [{}, {}]),
  v('undefined', undefined), v('null', null), v('number', 5), o('plain-not-iterable', () => ({a: 1})),
];

// Non-callable callbacks: the callback check fails before any item is read (no callback presets in the corpus).
const GROUP_CALLBACKS = [v('undefined', undefined), v('one', 1), v('string', 'f'), o('plain', () => ({}))];

// ---------------------------------------------------------------- case assembly

// Adds one case. `parts` are the positional inputs: [this, ...args] for method targets, [...args] for static and
// global targets, and [] for a getter target. A part that is a callback preset gets its name in cbs.
function add(list, id, target, parts, flags = {}) {
  list.push({id, target, parts, flags});
}

// Turns a list into defineSuite cases. Ids are kept unique by a deterministic numeric suffix.
function toCases(list) {
  const seen = new Map();
  return list.map(({id, target, parts, flags}) => {
    const n = (seen.get(id) ?? 0) + 1;
    seen.set(id, n);
    const uniqueId = n === 1 ? id : `${id}-${n}`;
    const hasPreset = parts.some((x) => x.preset !== undefined);
    const isNew = target.startsWith('new:');
    const isStatic = flags.static === true;
    const isGet = flags.get === true;
    const hasThis = !isStatic && !isNew && !isGet && flags.this !== false;
    const c = {id: uniqueId, target};
    if (isStatic) c.static = true;
    if (isGet) c.get = true;
    if (hasPreset) c.cbs = parts.map((x) => x.preset ?? null);
    c.make = () => {
      const vals = parts.map((x) => x.make());
      if (isGet) return {};
      if (hasThis) return {this: vals[0], args: vals.slice(1)};
      return {args: vals};
    };
    return c;
  });
}

// ---------------------------------------------------------------- suites

function objectProto() {
  const list = [];
  for (const t of OBJ_THIS) {
    for (const k of KEYS) {
      add(list, `has-own/${slug(t.label)}/${slug(k.label)}`, 'Object.prototype.hasOwnProperty', [t, k]);
    }
  }
  for (const t of OBJ_THIS) {
    for (const k of KEYS.slice(0, 12)) {
      add(list, `enumerable/${slug(t.label)}/${slug(k.label)}`, 'Object.prototype.propertyIsEnumerable', [t, k]);
    }
  }
  for (const t of OBJ_THIS) {
    for (const a of PROTO_ARGS) {
      add(list, `is-prototype-of/${slug(t.label)}/${slug(a.label)}`, 'Object.prototype.isPrototypeOf', [t, a]);
    }
  }
  for (const t of OBJ_THIS) add(list, `value-of/${slug(t.label)}`, 'Object.prototype.valueOf', [t]);
  for (const t of OBJ_THIS) add(list, `to-string/${slug(t.label)}`, 'Object.prototype.toString', [t]);
  for (const t of LOCALE_THIS) add(list, `to-locale-string/${slug(t.label)}`, 'Object.prototype.toLocaleString', [t]);
  return list;
}

function objectOwn() {
  const list = [];
  for (const s of OWN_SUBJECTS) {
    add(list, `own-names/${slug(s.label)}`, 'Object.getOwnPropertyNames', [s], {static: true});
    add(list, `own-symbols/${slug(s.label)}`, 'Object.getOwnPropertySymbols', [s], {static: true});
    add(list, `descriptors/${slug(s.label)}`, 'Object.getOwnPropertyDescriptors', [s], {static: true});
    for (const k of DESCRIPTOR_KEYS) {
      add(list, `descriptor/${slug(s.label)}/${slug(k.label)}`, 'Object.getOwnPropertyDescriptor', [s, k], {static: true});
    }
  }
  for (const items of GROUP_ITEMS) {
    for (const cb of GROUP_CALLBACKS) {
      add(list, `group-by/${slug(items.label)}/${slug(cb.label)}`, 'Object.groupBy', [items, cb], {static: true});
    }
  }
  return list;
}

function functionProto() {
  // Only non-callable receivers: a callable receiver would have to be a callback preset (refused here), and a native
  // function cannot be an input. The success paths are Rust unit tests (builtins_b1_function.rs).
  const list = [];
  const RECEIVERS = [
    v('undefined', undefined), v('null', null), v('one', 1), v('string', 'f'), v('true', true),
    o('plain', () => ({})), o('array', () => [1]), o('null-proto', () => Object.create(null)),
  ];
  for (const t of RECEIVERS) {
    add(list, `call/${slug(t.label)}`, 'Function.prototype.call', [t, v('undefined', undefined), v('one', 1)]);
    add(list, `call/${slug(t.label)}/no-arguments`, 'Function.prototype.call', [t]);
    add(list, `apply/${slug(t.label)}`, 'Function.prototype.apply', [t, v('undefined', undefined), o('empty', () => [])]);
    add(list, `apply/${slug(t.label)}/array-like`, 'Function.prototype.apply', [t, v('null', null), o('array-like', () => ({length: 2, 0: 'x', 1: 'y'}))]);
    add(list, `bind/${slug(t.label)}`, 'Function.prototype.bind', [t, v('undefined', undefined)]);
    add(list, `bind/${slug(t.label)}/with-arguments`, 'Function.prototype.bind', [t, v('null', null), v('one', 1), v('two', 2)]);
    add(list, `to-string/${slug(t.label)}`, 'Function.prototype.toString', [t]);
  }
  add(list, 'apply/argument-not-object', 'Function.prototype.apply', [v('one', 1), v('undefined', undefined), v('one', 1)]);
  add(list, 'apply/argument-string', 'Function.prototype.apply', [v('one', 1), v('undefined', undefined), v('string', 'ab')]);
  return list;
}

function symbolSuite() {
  const list = [];
  const CALL_ARGS = [
    [], [v('undefined', undefined)], [v('null', null)], [v('string', 'x')], [v('zero', 0)], [v('one', 1)],
    [v('true', true)], [v('empty', '')], [o('plain', () => ({}))], [v('string-null', 'null')], [v('nan', NaN)],
  ];
  for (const args of CALL_ARGS) add(list, `call/${args.length}/${slug(args.map((a) => a.label).join('-'))}`, 'Symbol', args, {static: true});
  // `new Symbol()` is not in the corpus: the replay refuses `new:` targets that are not constructors. The TypeError is
  // a Rust unit test (builtins_b1_symbol.rs, symbol_is_a_function_and_not_a_constructor).
  const KEYS_FOR = [
    v('k', 'k'), v('k-again', 'k'), v('empty', ''), v('undefined', undefined), v('null', null), v('one', 1),
    v('spaces', 'a b'), o('plain', () => ({})), v('true', true), v('neg-zero', -0),
  ];
  for (const k of KEYS_FOR) add(list, `for/${slug(k.label)}`, 'Symbol.for', [k], {static: true});
  const NOT_SYMBOLS = [v('undefined', undefined), v('null', null), v('k', 'k'), v('one', 1), o('plain', () => ({})), v('true', true), v('empty', '')];
  for (const x of NOT_SYMBOLS) add(list, `key-for/${slug(x.label)}`, 'Symbol.keyFor', [x], {static: true});
  const THIS_NOT_SYMBOL = [v('undefined', undefined), v('null', null), v('one', 1), v('string', 'x'), v('true', true), o('plain', () => ({})), o('array', () => [])];
  for (const t of THIS_NOT_SYMBOL) {
    add(list, `to-string/${slug(t.label)}`, 'Symbol.prototype.toString', [t]);
    add(list, `value-of/${slug(t.label)}`, 'Symbol.prototype.valueOf', [t]);
  }
  return list;
}

function booleanSuite() {
  const list = [];
  const VALUES = [
    v('undefined', undefined), v('null', null), v('true', true), v('false', false), v('zero', 0), v('neg-zero', -0),
    v('nan', NaN), v('one', 1), v('neg-one', -1), v('fraction', 2.5), v('inf', Infinity), v('empty', ''), v('zero-string', '0'),
    v('false-string', 'false'), v('string', 'x'), v('space', ' '), o('plain-empty', () => ({})), o('plain', () => ({a: 1})),
    o('array-empty', () => []), o('array-zero', () => [0]), o('null-proto', () => Object.create(null)),
  ];
  for (const x of VALUES) {
    add(list, `call/${slug(x.label)}`, 'Boolean', [x], {static: true});
  }
  for (const x of VALUES.slice(0, 12)) add(list, `new/${slug(x.label)}`, 'new:Boolean', [x]);
  add(list, 'new/none', 'new:Boolean', []);
  const THIS_VALUES = [
    v('true', true), v('false', false), v('undefined', undefined), v('null', null), v('zero', 0), v('empty', ''),
    v('string-true', 'true'), o('plain', () => ({})), o('array', () => []),
  ];
  for (const t of THIS_VALUES) {
    add(list, `to-string/${slug(t.label)}`, 'Boolean.prototype.toString', [t]);
    add(list, `value-of/${slug(t.label)}`, 'Boolean.prototype.valueOf', [t]);
  }
  return list;
}

function globalsSuite() {
  // Value properties only. parseInt, parseFloat, isNaN and isFinite are global-kind entries, which the corpus refuses.
  const list = [];
  add(list, 'value/nan', 'NaN', [], {get: true});
  add(list, 'value/infinity', 'Infinity', [], {get: true});
  return list;
}

// ---------------------------------------------------------------- output

const SUITES = [
  ['object-proto', objectProto],
  ['object-own', objectOwn],
  ['function-proto', functionProto],
  ['symbol', symbolSuite],
  ['boolean', booleanSuite],
  ['globals', globalsSuite],
];

export function buildSuites() {
  return SUITES.map(([name, build]) => ({file: `${name}.json`, suite: defineSuite(name, toCases(build()))}));
}

export function generate(outDir = DEFAULT_OUT) {
  return buildSuites().map(({file, suite}) => {
    const target = path.join(outDir, file);
    writeSuite(target, suite);
    return target;
  });
}

if (process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const outIndex = args.indexOf('--out');
  const outDir = outIndex >= 0 ? path.resolve(args[outIndex + 1]) : DEFAULT_OUT;
  for (const file of generate(outDir)) process.stdout.write(`${file}\n`);
}

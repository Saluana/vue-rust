// WP-B4 corpus suites: tests/language/builtins/{map,set,groupby}.json. They cover the Map and Set constructors and methods,
// the ES2025 set methods (Set.prototype.union ... isDisjointFrom) and Map.groupBy, wherever the corpus format can carry the
// call. Deterministic: fixed inputs, no clock, no randomness. Every expected value is produced by V8 when a suite is compiled
// (tooling/oracle/builtin-corpus.mjs); nothing here is a hand-written expectation.
// Not carried by the corpus format, so covered by crates/value/tests/builtins_b4_*.rs instead: callbacks (forEach, groupBy
// and mutation during iteration), iterator state, getters (size), symbol keys, and set-like objects whose has/keys are
// functions (`pre` can only name callback presets, and only at the positions of CALLBACK_ARITY targets).
//   node tests/language/gen/gen-b4.mjs [--out <dir>]        (default: tests/language/builtins)
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {defineSuite, writeSuite} from '../../../tooling/oracle/builtin-corpus.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const DEFAULT_OUT = path.resolve(here, '../builtins');

// Key shapes for Map and Set cases. Each factory returns a fresh value, so object identity is per execution.
const KEYS = [
  ['undef', () => undefined], ['null', () => null], ['true', () => true], ['false', () => false],
  ['zero', () => 0], ['negzero', () => -0], ['one', () => 1], ['half', () => 0.5], ['nan', () => NaN],
  ['inf', () => Infinity], ['ninf', () => -Infinity], ['str', () => 'x'], ['empty', () => ''],
  ['numstr', () => '1'], ['obj', () => ({})], ['arr', () => [1]], ['set', () => new Set([1])],
];
const keysFresh = () => KEYS.map(([, f]) => f());
// A map whose value for each key is that key's position in KEYS (a zero and a negzero collapse into one entry).
const mapOfKeys = (ks) => new Map(ks.map((k, i) => [k, i]));

// Receivers that are not the expected collection: every method must throw TypeError for them.
const BAD_THIS_MAP = [
  ['undef', () => undefined], ['null', () => null], ['num', () => 1], ['str', () => 'x'], ['bool', () => true],
  ['plain', () => ({})], ['array', () => [1, 2]], ['set', () => new Set([1])],
];
const BAD_THIS_SET = [
  ['undef', () => undefined], ['null', () => null], ['num', () => 1], ['str', () => 'x'], ['bool', () => true],
  ['plain', () => ({})], ['array', () => [1, 2]], ['map', () => new Map([[1, 2]])],
];
// Arguments that are not callable, for forEach (the receiver is checked first, then the callback).
const NOT_CALLABLE = [
  ['undef', () => undefined], ['null', () => null], ['num', () => 5], ['str', () => 'x'], ['plain', () => ({})],
  ['array', () => [1]],
];

// ---------------------------------------------------------------- Map

function mapCases() {
  const cases = [];
  const add = (id, target, make) => cases.push({id: id.toLowerCase(), target, make});
  const NEW = 'new:Map';

  // Constructor: the iterable (the protocol, entry checks, duplicate and SameValueZero keys).
  add('new/no-args', NEW, () => ({args: []}));
  add('new/undefined', NEW, () => ({args: [undefined]}));
  add('new/null', NEW, () => ({args: [null]}));
  add('new/empty-array', NEW, () => ({args: [[]]}));
  add('new/pairs', NEW, () => ({args: [[[1, 'a'], [2, 'b'], [3, 'c']]]}));
  add('new/duplicate-key-keeps-first-position', NEW, () => ({args: [[['a', 1], ['b', 2], ['a', 3]]]}));
  add('new/duplicate-key-last-value-wins', NEW, () => ({args: [[['k', 'first'], ['k', 'second']]]}));
  add('new/nan-keys-are-equal', NEW, () => ({args: [[[NaN, 1], [NaN, 2]]]}));
  add('new/zero-and-negzero-are-equal', NEW, () => ({args: [[[0, 'p'], [-0, 'n']]]}));
  add('new/negzero-key-is-stored-as-zero', NEW, () => ({args: [[[-0, 'z']]]}));
  add('new/undefined-and-null-are-distinct', NEW, () => ({args: [[[undefined, 1], [null, 2]]]}));
  add('new/keys-of-different-types-are-distinct', NEW, () => ({args: [[[true, 1], [1, 2], ['1', 3]]]}));
  add('new/object-keys-by-identity', NEW, () => {
    const o = {};
    return {args: [[[o, 1], [o, 2], [{}, 3]]]};
  });
  add('new/infinite-keys', NEW, () => ({args: [[[Infinity, 'p'], [-Infinity, 'n']]]}));
  add('new/value-missing-is-undefined', NEW, () => ({args: [[[1]]]}));
  add('new/entry-empty-array', NEW, () => ({args: [[[]]]}));
  add('new/entry-extra-elements-ignored', NEW, () => ({args: [[[1, 2, 3]]]}));
  add('new/nested-values', NEW, () => ({args: [[['k', [1, {x: 2}]]]]}));
  add('new/object-as-key', NEW, () => ({args: [[[{k: 1}, 'v']]]}));
  add('new/copy-of-a-map', NEW, () => ({args: [new Map([[1, 'a'], [2, 'b']])]}));
  add('new/from-a-set-of-pairs', NEW, () => ({args: [new Set([[1, 2], [3, 4]])]}));
  add('new/many-entries', NEW, () => ({args: [Array.from({length: 40}, (_, i) => [i, i * i])]}));
  add('new/entry-is-undefined-from-hole', NEW, () => ({args: [[, [1, 2]]]}));
  add('new/trailing-hole-entry', NEW, () => ({args: [[[1, 2], ,]]}));
  add('new/entry-is-null', NEW, () => ({args: [[null]]}));
  add('new/entry-is-number', NEW, () => ({args: [[1, 2]]}));
  add('new/set-of-primitives-is-not-entries', NEW, () => ({args: [new Set([1, 2])]}));
  add('new/plain-object-is-not-iterable', NEW, () => ({args: [{}]}));
  add('new/number-is-not-iterable', NEW, () => ({args: [5]}));
  add('new/boolean-is-not-iterable', NEW, () => ({args: [true]}));

  // get, has, delete and set for every key shape, on a map that holds all of them.
  KEYS.forEach(([label], j) => {
    add(`get/key-${label}`, 'Map.prototype.get', () => {
      const ks = keysFresh();
      return {this: mapOfKeys(ks), args: [ks[j]]};
    });
    add(`has/key-${label}`, 'Map.prototype.has', () => {
      const ks = keysFresh();
      return {this: mapOfKeys(ks), args: [ks[j]]};
    });
    add(`delete/key-${label}`, 'Map.prototype.delete', () => {
      const ks = keysFresh();
      return {this: mapOfKeys(ks), args: [ks[j]]};
    });
    add(`set/key-${label}`, 'Map.prototype.set', () => {
      const ks = keysFresh();
      return {this: mapOfKeys(ks), args: [ks[j], 'new']};
    });
    add(`set/absent-key-${label}`, 'Map.prototype.set', () => {
      const ks = keysFresh();
      const k = ks[j];
      ks.splice(j, 1);
      return {this: mapOfKeys(ks), args: [k, 'appended']};
    });
  });
  add('get/fresh-object-is-missing', 'Map.prototype.get', () => ({this: mapOfKeys(keysFresh()), args: [{}]}));
  add('has/fresh-array-is-missing', 'Map.prototype.has', () => ({this: mapOfKeys(keysFresh()), args: [[1]]}));
  add('get/no-args-looks-up-undefined', 'Map.prototype.get', () => ({this: new Map([[undefined, 'u']])}));
  add('get/missing-in-empty-map', 'Map.prototype.get', () => ({this: new Map(), args: ['k']}));
  add('delete/missing-key', 'Map.prototype.delete', () => ({this: new Map([[1, 'a']]), args: [2]}));
  add('set/no-value-argument', 'Map.prototype.set', () => ({this: new Map(), args: ['k']}));
  add('set/existing-key-keeps-position', 'Map.prototype.set', () => ({this: new Map([['a', 1], ['b', 2], ['c', 3]]), args: ['a', 9]}));
  add('set/negzero-key-is-stored-as-zero', 'Map.prototype.set', () => ({this: new Map(), args: [-0, 'z']}));
  add('has/negzero-finds-zero', 'Map.prototype.has', () => ({this: new Map([[0, 'z']]), args: [-0]}));
  add('clear/filled', 'Map.prototype.clear', () => ({this: mapOfKeys(keysFresh())}));
  add('clear/empty', 'Map.prototype.clear', () => ({this: new Map()}));

  // Iterators. The iterator object itself is not traceable, so only the receiver's state and the result shape are checked.
  for (const m of ['keys', 'values', 'entries']) {
    add(`${m}/filled`, `Map.prototype.${m}`, () => ({this: mapOfKeys(keysFresh())}));
    add(`${m}/empty`, `Map.prototype.${m}`, () => ({this: new Map()}));
  }

  // forEach: the receiver is checked first, then the callback. Callbacks themselves are not in the corpus.
  for (const [label, f] of NOT_CALLABLE) {
    add(`forEach/not-callable-${label}`, 'Map.prototype.forEach', () => ({this: mapOfKeys(keysFresh()), args: [f()]}));
    add(`forEach/empty-not-callable-${label}`, 'Map.prototype.forEach', () => ({this: new Map(), args: [f()]}));
  }

  // Receivers: every method throws TypeError for a value that is not a Map.
  const methods = [
    ['get', [1]], ['set', [1, 2]], ['has', [1]], ['delete', [1]], ['clear', []],
    ['keys', []], ['values', []], ['entries', []], ['forEach', [5]],
  ];
  for (const [m, args] of methods) {
    for (const [label, f] of BAD_THIS_MAP) {
      add(`${m}/bad-this-${label}`, `Map.prototype.${m}`, () => ({this: f(), args: [...args]}));
    }
  }
  // Callbacks: presets of the corpus library at position 1 of pre (CALLBACK_ARITY: Map.prototype.forEach takes 3).
  const addCb = (id, target, make, cbs) => cases.push({id: id.toLowerCase(), target, make, cbs});
  for (const p of ['identity', 'pair', 'index', 'keyOf', 'isTruthy', 'toArr2', 'double']) {
    addCb(`forEach/cb-${p}-filled`, 'Map.prototype.forEach', () => ({this: new Map([[1, 'a'], ['b', 2], [3, {}]]), args: [undefined]}), [null, p]);
  }
  addCb('forEach/cb-throwat2-throws-at-key-2', 'Map.prototype.forEach', () => ({this: new Map([[1, 'a'], [2, 'b'], [3, {}]]), args: [undefined]}), [null, 'throwAt2']);
  addCb('forEach/cb-identity-empty-map', 'Map.prototype.forEach', () => ({this: new Map(), args: [undefined]}), [null, 'identity']);
  addCb('forEach/cb-identity-with-this-arg', 'Map.prototype.forEach', () => ({this: new Map([[1, 'a']]), args: [undefined, {t: 1}]}), [null, 'identity']);
  addCb('forEach/cb-pair-object-values', 'Map.prototype.forEach', () => ({this: new Map([[1, {}], [2, [1]]]), args: [undefined]}), [null, 'pair']);
  return cases;
}

// ---------------------------------------------------------------- Set

// Operand shapes for the set methods. `sh` is one object shared by both operands of a case, so identity is visible.
const SET_SHAPES = [
  ['empty', () => []],
  ['one', () => [1]],
  ['ints', () => [1, 2, 3]],
  ['shifted', () => [2, 3, 4, 5]],
  ['specials', () => [NaN, -0, 'x']],
  ['shared', (sh) => [sh, 1]],
  ['sharedonly', (sh) => [sh]],
];
const SET_METHODS = [
  'union', 'intersection', 'difference', 'symmetricDifference', 'isSubsetOf', 'isSupersetOf', 'isDisjointFrom',
];
// Arguments that fail GetSetRecord. size is read first, then has and keys; an object without has throws TypeError, and a
// bad size throws before has is read (NaN and too-small sizes are TypeError and RangeError).
const BAD_OTHER = [
  ['undef', () => undefined], ['null', () => null], ['num', () => 5], ['str', () => 'x'], ['bool', () => true],
  ['array', () => [1, 2]], ['plain', () => ({})], ['size-nan', () => ({size: NaN})],
  ['size-undefined', () => ({size: undefined})], ['size-negative', () => ({size: -1})],
  ['size-string', () => ({size: '2'})], ['size-infinite', () => ({size: Infinity})],
];

function setCases() {
  const cases = [];
  const add = (id, target, make) => cases.push({id: id.toLowerCase(), target, make});
  const NEW = 'new:Set';

  // Constructor.
  add('new/no-args', NEW, () => ({args: []}));
  add('new/undefined', NEW, () => ({args: [undefined]}));
  add('new/null', NEW, () => ({args: [null]}));
  add('new/empty-array', NEW, () => ({args: [[]]}));
  add('new/duplicates-collapse', NEW, () => ({args: [[1, 1, 2, 1]]}));
  add('new/first-position-kept', NEW, () => ({args: [[3, 1, 2, 1, 3]]}));
  add('new/nan-once', NEW, () => ({args: [[NaN, NaN]]}));
  add('new/zero-and-negzero-once', NEW, () => ({args: [[0, -0]]}));
  add('new/negzero-stored-as-zero', NEW, () => ({args: [[-0]]}));
  add('new/strings-by-content', NEW, () => ({args: [['a', 'a', 'b']]}));
  add('new/objects-by-identity', NEW, () => {
    const o = {};
    return {args: [[o, o, {}]]};
  });
  add('new/arrays-are-distinct', NEW, () => ({args: [[[1], [1]]]}));
  add('new/undefined-and-null-are-distinct', NEW, () => ({args: [[undefined, null, undefined]]}));
  add('new/mixed-types', NEW, () => ({args: [[1, '1', true, null, NaN, -0, 'x']]}));
  add('new/holes-add-undefined', NEW, () => ({args: [[1, , 2]]}));
  add('new/infinite-values', NEW, () => ({args: [[Infinity, -Infinity, Infinity]]}));
  add('new/copy-of-a-set', NEW, () => ({args: [new Set([1, 2, 3])]}));
  add('new/from-a-map-gives-pairs', NEW, () => ({args: [new Map([[1, 'a'], [2, 'b']])]}));
  add('new/many-values', NEW, () => ({args: [Array.from({length: 40}, (_, i) => i % 13)]}));
  add('new/plain-object-is-not-iterable', NEW, () => ({args: [{}]}));
  add('new/number-is-not-iterable', NEW, () => ({args: [5]}));
  add('new/boolean-is-not-iterable', NEW, () => ({args: [false]}));

  // add, has, delete for every key shape.
  KEYS.forEach(([label], j) => {
    add(`has/key-${label}`, 'Set.prototype.has', () => {
      const ks = keysFresh();
      return {this: new Set(ks), args: [ks[j]]};
    });
    add(`delete/key-${label}`, 'Set.prototype.delete', () => {
      const ks = keysFresh();
      return {this: new Set(ks), args: [ks[j]]};
    });
    add(`add/key-${label}`, 'Set.prototype.add', () => {
      const ks = keysFresh();
      return {this: new Set(ks), args: [ks[j]]};
    });
    add(`add/absent-key-${label}`, 'Set.prototype.add', () => {
      const ks = keysFresh();
      const k = ks[j];
      ks.splice(j, 1);
      return {this: new Set(ks), args: [k]};
    });
  });
  add('add/no-args-adds-undefined', 'Set.prototype.add', () => ({this: new Set([1])}));
  add('add/negzero-stored-as-zero', 'Set.prototype.add', () => ({this: new Set(), args: [-0]}));
  add('has/negzero-finds-zero', 'Set.prototype.has', () => ({this: new Set([0]), args: [-0]}));
  add('has/no-args-looks-up-undefined', 'Set.prototype.has', () => ({this: new Set([undefined])}));
  add('delete/missing-value', 'Set.prototype.delete', () => ({this: new Set([1]), args: [2]}));
  add('clear/filled', 'Set.prototype.clear', () => ({this: new Set(keysFresh())}));
  add('clear/empty', 'Set.prototype.clear', () => ({this: new Set()}));
  for (const m of ['keys', 'values', 'entries']) {
    add(`${m}/filled`, `Set.prototype.${m}`, () => ({this: new Set([1, 'a'])}));
    add(`${m}/empty`, `Set.prototype.${m}`, () => ({this: new Set()}));
  }
  for (const [label, f] of NOT_CALLABLE) {
    add(`forEach/not-callable-${label}`, 'Set.prototype.forEach', () => ({this: new Set([1, 2]), args: [f()]}));
  }

  // Receivers: every Set method throws TypeError for a value that is not a Set.
  const methods = [
    ['add', [1]], ['has', [1]], ['delete', [1]], ['clear', []], ['keys', []], ['values', []], ['entries', []],
    ['forEach', [5]],
  ];
  for (const [m, args] of methods) {
    for (const [label, f] of BAD_THIS_SET) {
      add(`${m}/bad-this-${label}`, `Set.prototype.${m}`, () => ({this: f(), args: [...args]}));
    }
  }

  // The ES2025 set methods: every pair of operand shapes, with a Set as `other`.
  for (const m of SET_METHODS) {
    const target = `Set.prototype.${m}`;
    for (const [an, a] of SET_SHAPES) {
      for (const [bn, b] of SET_SHAPES) {
        add(`${m}/${an}-with-set-${bn}`, target, () => {
          const sh = {};
          return {this: new Set(a(sh)), args: [new Set(b(sh))]};
        });
      }
    }
    // A Map is set-like: its has and keys are Map.prototype's, and its keys are the set's values.
    for (const [an, a] of [SET_SHAPES[0], SET_SHAPES[1], SET_SHAPES[2], SET_SHAPES[5]]) {
      for (const [bn, b] of SET_SHAPES) {
        add(`${m}/${an}-with-map-${bn}`, target, () => {
          const sh = {};
          return {this: new Set(a(sh)), args: [new Map(b(sh).map((k) => [k, 'v']))]};
        });
      }
    }
    add(`${m}/aliased-this-is-other`, target, () => {
      const s = new Set([1, 2, 3]);
      return {this: s, args: [s]};
    });
    add(`${m}/aliased-empty`, target, () => {
      const s = new Set();
      return {this: s, args: [s]};
    });
    for (const [label, f] of BAD_OTHER) {
      add(`${m}/bad-other-${label}`, target, () => ({this: new Set([1, 2]), args: [f()]}));
    }
    for (const [label, f] of [['map', () => new Map([[1, 2]])], ['plain', () => ({})]]) {
      add(`${m}/bad-this-${label}`, target, () => ({this: f(), args: [new Set([1])]}));
    }
  }
  // Callbacks at position 1 of pre (CALLBACK_ARITY: Set.prototype.forEach takes 3).
  const addCb = (id, target, make, cbs) => cases.push({id: id.toLowerCase(), target, make, cbs});
  for (const p of ['identity', 'pair', 'index', 'toArr2']) {
    addCb(`forEach/cb-${p}-filled`, 'Set.prototype.forEach', () => ({this: new Set([1, 'x', {}]), args: [undefined]}), [null, p]);
  }
  addCb('forEach/cb-throwat2-throws-at-value-2', 'Set.prototype.forEach', () => ({this: new Set([1, 2, 3]), args: [undefined]}), [null, 'throwAt2']);
  addCb('forEach/cb-identity-empty-set', 'Set.prototype.forEach', () => ({this: new Set(), args: [undefined]}), [null, 'identity']);
  return cases;
}

// ---------------------------------------------------------------- Map.groupBy (argument checks only)

function groupByCases() {
  const cases = [];
  const add = (id, args) => cases.push({id: id.toLowerCase(), target: 'Map.groupBy', static: true, make: () => ({args: args()})});
  // Map.groupBy throws TypeError for a missing or non-callable callback and for items that are null or undefined. The
  // callback check happens first, so a non-iterable items value with a callable callback is only checked by GetIterator.
  add('no-args', () => []);
  add('items-undefined-callback-undefined', () => [undefined, undefined]);
  add('items-null-callback-null', () => [null, null]);
  add('items-null', () => [null]);
  add('items-undefined', () => [undefined]);
  add('items-number-callback-number', () => [5, 5]);
  add('items-number', () => [5]);
  add('items-boolean-callback-number', () => [true, 1]);
  add('items-plain-object-callback-number', () => [{}, 5]);
  add('callback-missing', () => [[1]]);
  add('callback-number', () => [[1], 5]);
  add('callback-string', () => [[1], 'f']);
  add('callback-object', () => [[1], {}]);
  add('callback-undefined-empty-array', () => [[], undefined]);
  // Callbacks at position 1 of pre (CALLBACK_ARITY: Map.groupBy takes 2: value and index).
  const addCb = (id, args, cbs) => cases.push({id: id.toLowerCase(), target: 'Map.groupBy', static: true, make: () => ({args: args()}), cbs});
  for (const p of ['keyOf', 'index', 'identity', 'isTruthy', 'pair']) {
    addCb(`cb-${p}`, () => [[1, 'a', 1, 0], undefined], [null, p]);
  }
  addCb('cb-throwat2-throws-at-index-2', () => [[1, 2, 3], undefined], [null, 'throwAt2']);
  addCb('cb-keyof-empty-items', () => [[], undefined], [null, 'keyOf']);
  addCb('cb-keyof-items-not-iterable', () => [5, undefined], [null, 'keyOf']);
  add('callback-null-empty-array', () => [[], null]);
  return cases;
}

// ---------------------------------------------------------------- build

export function buildSuites() {
  return [
    {file: 'map.json', suite: defineSuite('map', mapCases())},
    {file: 'set.json', suite: defineSuite('set', setCases())},
    {file: 'groupby.json', suite: defineSuite('groupby', groupByCases())},
  ];
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

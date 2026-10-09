// WP-O2 tests: builtin-call corpora (tooling/oracle/builtin-corpus.mjs) and the seed suites in tests/language/builtins.
// Expectations are written by hand from trace-format.md and the ECMAScript semantics, or come from V8 in a child process.
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {copyFileSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {
  CALLBACK_ARITY,
  PRESET_ARITY,
  TARGETS,
  checkSuite,
  compileSuite,
  defineSuite,
  describeCase,
  makePresets,
  renderSuite,
  targetProblem,
} from '../../tooling/oracle/builtin-corpus.mjs';
import {renderAll} from './gen/gen-builtins.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const suitesDir = path.join(here, 'builtins');
const genScript = path.join(here, 'gen', 'gen-builtins.mjs');
const cli = path.resolve(here, '../../tooling/oracle/builtin-corpus.mjs');
const SUITE_FILES = ['math.json', 'number.json', 'object-statics.json'];

// Trace-format helpers, written from docs/dev/trace-format.md.
const NEG0 = {$: 'num', v: '-0'};
const NAN = {$: 'num', v: 'NaN'};
const HOLE = {$: 'hole'};
const ref = (id) => ({$: 'ref', id});
const fn = (name, length) => ({$: 'fn', name, length});
const obj = (id, props) => ({$: 'obj', id, cls: 'Object', proto: 'Object.prototype', props});
const arr = (id, elems) => ({$: 'obj', id, cls: 'Array', proto: 'other', props: [], elems, len: elems.length});

function childEnv() {
  const env = {...process.env};
  delete env.NODE_TEST_CONTEXT; // a child of node --test must behave like a plain node process
  return env;
}

const readSuite = (file) => JSON.parse(readFileSync(path.join(suitesDir, file), 'utf8'));

// The WP-O2 target list (Math.* functions and constants, Number and Object methods), hand-copied from the WP.
const WP_TARGETS = [
  ...['abs', 'ceil', 'floor', 'round', 'trunc', 'sign', 'sqrt', 'cbrt', 'pow', 'exp', 'expm1', 'log', 'log2', 'log10',
    'log1p', 'sin', 'cos', 'tan', 'asin', 'acos', 'atan', 'atan2', 'sinh', 'cosh', 'tanh', 'asinh', 'acosh', 'atanh',
    'min', 'max', 'hypot', 'fround', 'clz32', 'imul'].map((f) => `Math.${f}`),
  ...['PI', 'E', 'LN2', 'LN10', 'LOG2E', 'LOG10E', 'SQRT2', 'SQRT1_2'].map((k) => `Math.${k}`),
  'Number', 'Number.isInteger', 'Number.isSafeInteger', 'Number.isFinite', 'Number.isNaN', 'Number.parseFloat',
  'Number.parseInt', 'Number.prototype.toString', 'Number.prototype.toFixed', 'Number.prototype.toPrecision',
  'Number.prototype.toExponential', 'Number.prototype.valueOf',
  'Object.keys', 'Object.values', 'Object.entries', 'Object.assign', 'Object.fromEntries', 'Object.freeze',
  'Object.isFrozen', 'Object.seal', 'Object.isSealed', 'Object.preventExtensions', 'Object.isExtensible',
  'Object.is', 'Object.hasOwn', 'Object.getPrototypeOf', 'Object.create',
];

// ---------------------------------------------------------------- callback presets

test('callback presets: names and declared arity match the case format', () => {
  const presets = makePresets([]);
  assert.deepEqual(Object.keys(presets).sort(), Object.keys(PRESET_ARITY).sort());
  assert.equal(Object.keys(PRESET_ARITY).length, 15);
  for (const [name, arity] of Object.entries(PRESET_ARITY)) {
    assert.equal(presets[name].name, name);
    assert.equal(presets[name].length, arity, name);
  }
});

test('callback presets: hand-written return values', () => {
  const p = makePresets([]);
  const o = {};
  assert.equal(p.identity(7), 7);
  assert.equal(p.identity(o, 2), o);
  assert.equal(p.double(4), 8);
  assert.equal(p.double('3'), 6);
  assert.ok(Number.isNaN(p.double(undefined)));
  assert.equal(p.isEven(4), true);
  assert.equal(p.isEven(-3), false);
  assert.equal(p.isEven(0.5), false);
  assert.equal(p.isTruthy(''), false);
  assert.equal(p.isTruthy('0'), true);
  assert.equal(p.index('x', 3), 3);
  assert.equal(p.index(o, 4), 4);
  assert.deepEqual(p.pair('a', 1), ['a', 1]);
  assert.equal(p.sum(1, 2), 3);
  assert.equal(p.sum('a', 'b'), 'ab');
  assert.equal(p.sum(o, 5, 2), '[object Object]5');
  assert.deepEqual(p.concat([1], 2), [1, 2]);
  assert.deepEqual(p.concat(undefined, [2, 3]), [undefined, 2, 3]);
  assert.equal(p.cmpNum(5, 3), 2);
  assert.equal(p.cmpNum('10', 9), 1);
  assert.equal(p.cmpDesc(5, 3), -2);
  assert.equal(p.cmpStr('a', 'b'), -1);
  assert.equal(p.cmpStr('b', 'a'), 1);
  assert.equal(p.cmpStr('a', 'a'), 0);
  assert.equal(p.cmpStr('\uD800', '￿'), -1); // code-unit order: 0xD800 < 0xFFFF
  assert.equal(p.cmpThrow(1, 2), -1);
  assert.equal(p.throwAt2('x', 1), 'x');
  assert.deepEqual(p.toArr2(7), [7, 7]);
  const twice = p.toArr2(o, 1);
  assert.equal(twice[0], o);
  assert.equal(twice[1], o);
  assert.equal(p.keyOf(5), '5');
  assert.equal(p.keyOf(null), 'null');
  assert.equal(p.keyOf([1, 2]), '1,2');
  assert.equal(p.keyOf(o, 0), '[object Object]');
});

test('callback presets: hand-written log entries (primitives as themselves; objects as their index or -1)', () => {
  const log = [];
  const p = makePresets(log);
  const o = {};
  p.identity(7);
  p.identity(o, 2);
  p.identity(o);
  p.double(undefined);
  p.isEven(0.5);
  p.index('x', 3);
  p.index(o, 4);
  p.sum(1, 2);
  p.sum(o, 5, 2);
  p.concat([1], 2);
  p.concat(undefined, [2, 3]);
  p.cmpNum('10', 9);
  p.cmpStr('\uD800', '￿');
  p.toArr2(o, 1);
  p.keyOf(null);
  p.keyOf([1, 2]);
  p.keyOf(o, 0);
  assert.deepEqual(log, [
    ['identity', 7],
    ['identity', 2],
    ['identity', -1],
    ['double', undefined],
    ['isEven', 0.5],
    ['index', 'x', 3],
    ['index', 4, 4],
    ['sum', 1, 2],
    ['sum', 2, 5],
    ['concat', -1, 2],
    ['concat', undefined, -1],
    ['cmpNum', '10', 9],
    ['cmpStr', '\uD800', '￿'],
    ['toArr2', 1],
    ['keyOf', null],
    ['keyOf', -1],
    ['keyOf', 0],
  ]);
});

test('callback presets: cmpThrow throws on its second call; throwAt2 throws at index 2, after logging', () => {
  const log = [];
  const p = makePresets(log);
  assert.equal(p.cmpThrow(5, 4), 1);
  assert.throws(() => p.cmpThrow(3, 4), (e) => e instanceof Error && e.name === 'Error' && e.message === 'cmp');
  assert.equal(p.throwAt2('x', 1), 'x');
  assert.throws(() => p.throwAt2('y', 2), (e) => e.name === 'Error' && e.message === 'at2');
  assert.deepEqual(log, [
    ['cmpThrow', 5, 4],
    ['cmpThrow', 3, 4],
    ['throwAt2', 'x', 1],
    ['throwAt2', 'y', 2],
  ]);
  // Counters belong to one execution: a fresh set of presets starts again.
  assert.equal(makePresets([]).cmpThrow(5, 4), 1);
});

// ---------------------------------------------------------------- targets and definitions

test('targets: the standard-library manifest decides, with its hint (hand-written expectations)', () => {
  assert.equal(targetProblem('Math.max'), null);
  assert.equal(targetProblem('Math.PI'), null);
  assert.equal(targetProblem('Number.prototype.toFixed'), null);
  assert.equal(targetProblem('Object.getOwnPropertyNames'), null); // manifest P: no hand-written allowlist needed
  assert.match(targetProblem('Object.defineProperty'),
    /^Object\.defineProperty is X for target test \(row L04, VN4008\): Property descriptor mutation is not supported/);
  assert.match(targetProblem('Object.setPrototypeOf'), /is X for target test \(row L05, VN4008\): Prototype replacement/);
  assert.match(targetProblem('Proxy'), /is X for target test \(row L20, VN4003\): Use ref, reactive or computed/);
  assert.equal(targetProblem('TextEncoder.prototype.encode'), null); // A but deterministic: accepted
  assert.match(targetProblem('Math.random'), /^Math\.random is A for target test \(row L19\), a host clock or random provider/);
  assert.match(targetProblem('Date.now'), /^Date\.now is A for target test \(row L19\), a host clock or random provider/);
  assert.match(targetProblem('performance.now'), /^performance\.now is A for target test/);
  assert.match(targetProblem('Intl.NumberFormat'), /not a static, proto, ctor or const entry/); // absent
  assert.match(targetProblem('Array.prototype.nope'), /not a static, proto, ctor or const entry/); // absent
  assert.match(targetProblem('Math.nope'), /not a static, proto, ctor or const entry/); // absent
  assert.match(targetProblem('parseInt'), /^parseInt is a global \(kind global\), not a static, proto, ctor or const entry$/);
});

test('callback targets: the only cbs-capable targets, with hand-written arity', () => {
  const expected = [
    ['Array.prototype.map', 3], ['Array.prototype.reduce', 4], ['Array.prototype.sort', 2],
    ['Array.prototype.forEach', 3], ['Array.prototype.filter', 3], ['Array.prototype.find', 3], ['Array.prototype.findIndex', 3],
    ['Array.prototype.findLast', 3], ['Array.prototype.findLastIndex', 3], ['Array.prototype.some', 3], ['Array.prototype.every', 3],
    ['Array.prototype.flatMap', 3], ['Array.prototype.reduceRight', 4], ['Array.prototype.toSorted', 2], ['Array.from', 2],
    ['Object.groupBy', 2], ['Map.groupBy', 2], ['Map.prototype.forEach', 3], ['Set.prototype.forEach', 3],
    ['JSON.parse', 2], ['JSON.stringify', 2], ['String.prototype.replace', 3], ['String.prototype.replaceAll', 3],
    ['Function.prototype.call', 3], ['Function.prototype.apply', 3], ['Function.prototype.bind', 3],
  ];
  assert.deepEqual([...CALLBACK_ARITY], expected);
  assert.deepEqual([...TARGETS].sort(), expected.map(([n]) => n).sort());
  for (const t of TARGETS) assert.equal(targetProblem(t), null, t);
});

test('defineSuite rejects targets and case shapes outside the format', () => {
  const one = (extra) => () => defineSuite('bad', [{id: 'case', make: () => ({args: []}), ...extra}]);
  assert.throws(one({target: 'Array.prototype.nope'}), /not a static, proto, ctor or const entry/);
  assert.throws(one({target: 'Object.defineProperty'}), /is X for target test/);
  assert.throws(one({target: 'Math.abs', get: true, make: () => ({})}), /expected a number/);
  assert.throws(one({target: 'Math.PI', static: true}), /expected a function/);
  assert.throws(one({target: 'Number.isInteger', construct: true}), /not a constructor/);
  assert.throws(one({target: 'Number', construct: true, static: true}), /take no static or get flag/);
  assert.throws(one({target: 'Math.abs', cbs: ['nope']}), /unknown callback preset/);
  assert.throws(one({target: 'Math.abs', cbs: [null, 'identity']}), /takes no callbacks in the corpus/);
  assert.throws(one({target: 'Math.abs', extra: 1}), /unknown case field "extra"/);
  assert.throws(() => defineSuite('bad', [{id: 'Upper', target: 'Math.abs', make: () => ({args: []})}]), /must match/);
  assert.throws(() => defineSuite('bad', [
    {id: 'a', target: 'Math.abs', make: () => ({args: []})},
    {id: 'a', target: 'Math.abs', make: () => ({args: []})},
  ]), /duplicate case id/);
  assert.throws(() => defineSuite('Bad Name', [{id: 'a', target: 'Math.abs', make: () => ({args: []})}]), /must match/);
});

test('compileSuite rejects a static case that gives a this value', () => {
  const s = defineSuite('bad', [{id: 'a', target: 'Math.abs', static: true, make: () => ({this: 1, args: [-1]})}]);
  assert.throws(() => compileSuite(s), /take no this/);
});

test('compileSuite carries integrity (trace-format v1.1): a frozen input replays', () => {
  const frozen = defineSuite('frozen', [{
    id: 'is-frozen', target: 'Object.isFrozen', static: true, make: () => ({args: [Object.freeze({a: 1})]}),
  }]);
  const c = compileSuite(frozen).cases[0];
  const frozenA = {$: 'obj', id: 1, cls: 'Object', proto: 'Object.prototype', integrity: 'frozen', props: [['a', 1]]};
  assert.deepEqual(c.pre, [frozenA]);
  assert.deepEqual(c.post, {ret: true, throw: null, state: [frozenA, arr(2, [])]});
});

test('compileSuite replays sealed and non-extensible inputs (integrity "sealed", "nonextensible")', () => {
  const sealed = defineSuite('sealed', [{
    id: 'is-sealed', target: 'Object.isSealed', static: true, make: () => ({args: [Object.seal({a: 1})]}),
  }]);
  const s = compileSuite(sealed).cases[0];
  const sealedA = {$: 'obj', id: 1, cls: 'Object', proto: 'Object.prototype', integrity: 'sealed', props: [['a', 1]]};
  assert.deepEqual(s.pre, [sealedA]);
  assert.deepEqual(s.post, {ret: true, throw: null, state: [sealedA, arr(2, [])]});
  const nonExt = defineSuite('nonext', [{
    id: 'is-extensible', target: 'Object.isExtensible', static: true,
    make: () => ({args: [Object.preventExtensions({a: 1})]}),
  }]);
  const n = compileSuite(nonExt).cases[0];
  const nonExtA = {$: 'obj', id: 1, cls: 'Object', proto: 'Object.prototype', integrity: 'nonextensible', props: [['a', 1]]};
  assert.deepEqual(n.pre, [nonExtA]);
  assert.deepEqual(n.post, {ret: false, throw: null, state: [nonExtA, arr(2, [])]});
});

test('compileSuite rejects what the format cannot carry: accessors, and hidden state that a call observes', () => {
  const getter = defineSuite('getter', [{
    id: 'keys-getter', target: 'Object.keys', static: true, make: () => ({args: [{get a() { return 1; }}]}),
  }]);
  assert.throws(() => compileSuite(getter), /not replayable from its pre encoding/);
  const hidden = defineSuite('hidden', [{
    id: 'has-hidden', target: 'Object.hasOwn', static: true,
    make: () => {
      const o = {a: 1};
      Object.defineProperty(o, 'hidden', {value: 2, enumerable: false}); // not traced: a replay cannot see it
      return {args: [o, 'hidden']};
    },
  }]);
  assert.throws(() => compileSuite(hidden), /not replayable from its pre encoding: .*\$\.ret/);
});

// ---------------------------------------------------------------- hand-written spot checks of pre and post

test('spot check: Array.prototype.slice keeps holes in pre, ret and state (trace-format encoding)', () => {
  const s = defineSuite('spot', [{
    id: 'slice/holes',
    target: 'Array.prototype.slice',
    make: () => {
      const a = [1];
      a[2] = 3;
      return {this: a, args: [-2]};
    },
  }]);
  const c = compileSuite(s).cases[0];
  // slice(-2) of [1, <hole>, 3]: start index 1 is a hole, so the result is [<hole>, 3].
  assert.deepEqual(c.pre, [arr(1, [1, HOLE, 3]), -2]);
  assert.deepEqual(c.post, {
    ret: arr(1, [HOLE, 3]),
    throw: null,
    state: [arr(2, [1, HOLE, 3]), -2, arr(3, [])],
  });
});

test('spot check: Object.assign(o, o) shares ids within one pass (ref after the first expansion)', () => {
  const s = defineSuite('spot', [{
    id: 'assign/self',
    target: 'Object.assign',
    static: true,
    make: () => {
      const o = {a: 1};
      return {args: [o, o]};
    },
  }]);
  const c = compileSuite(s).cases[0];
  assert.deepEqual(c.pre, [obj(1, [['a', 1]]), ref(1)]);
  assert.deepEqual(c.post, {ret: obj(1, [['a', 1]]), throw: null, state: [ref(1), ref(1), arr(2, [])]});
});

test('spot check: a cycle in the pre encoding is a ref to the object itself, and replays', () => {
  const s = defineSuite('spot', [{
    id: 'keys/cycle',
    target: 'Object.keys',
    static: true,
    make: () => {
      const cyc = {};
      cyc.self = cyc;
      return {args: [cyc]};
    },
  }]);
  const c = compileSuite(s).cases[0];
  assert.deepEqual(c.pre, [{$: 'obj', id: 1, cls: 'Object', proto: 'Object.prototype', props: [['self', ref(1)]]}]);
  assert.deepEqual(c.post, {
    ret: arr(1, ['self']),
    throw: null,
    state: [obj(2, [['self', ref(2)]]), arr(3, [])],
  });
});

test('spot check: a throwing call records ret as null and the error name only', () => {
  const s = defineSuite('spot', [{id: 'keys/null', target: 'Object.keys', static: true, make: () => ({args: [null]})}]);
  const c = compileSuite(s).cases[0];
  assert.deepEqual(c.pre, [null]);
  assert.deepEqual(c.post, {ret: null, throw: {name: 'TypeError'}, state: [null, arr(1, [])]});
});

test('spot check: Math.PI is a get case (nothing is called, pre is empty)', () => {
  const s = defineSuite('spot', [{id: 'const/pi', target: 'Math.PI', get: true, make: () => ({})}]);
  assert.deepEqual(compileSuite(s).cases[0], {
    id: 'const/pi',
    target: 'Math.PI',
    get: 1,
    pre: [],
    post: {ret: Math.PI, throw: null, state: [arr(1, [])]},
  });
});

test('spot check: callbacks are substituted by name, encoded as fn, and their calls logged', () => {
  const s = defineSuite('cb', [
    {id: 'map/double', target: 'Array.prototype.map', cbs: [null, 'double'],
      make: () => ({this: [1, 2], args: [null]})},
    {id: 'reduce/sum', target: 'Array.prototype.reduce', cbs: [null, 'sum', null],
      make: () => ({this: [1, 2, 3], args: [null, 0]})},
    {id: 'map/objects', target: 'Array.prototype.map', cbs: [null, 'identity'],
      make: () => ({this: [{}], args: [null]})},
    {id: 'map/throws', target: 'Array.prototype.map', cbs: [null, 'throwAt2'],
      make: () => ({this: [1, 2, 3], args: [null]})},
  ]);
  const [mapDouble, reduceSum, mapObjects, mapThrows] = compileSuite(s).cases;

  assert.deepEqual(mapDouble.cbs, [null, 'double']);
  assert.deepEqual(mapDouble.pre, [arr(1, [1, 2]), fn('double', 1)]);
  assert.deepEqual(mapDouble.post, {
    ret: arr(1, [2, 4]),
    throw: null,
    state: [arr(2, [1, 2]), fn('double', 1), arr(3, [arr(4, ['double', 1]), arr(5, ['double', 2])])],
  });

  assert.deepEqual(reduceSum.pre, [arr(1, [1, 2, 3]), fn('sum', 2), 0]);
  assert.deepEqual(reduceSum.post, {
    ret: 6,
    throw: null,
    state: [
      arr(1, [1, 2, 3]),
      fn('sum', 2),
      0,
      arr(2, [arr(3, ['sum', 0, 1]), arr(4, ['sum', 1, 2]), arr(5, ['sum', 3, 3])]),
    ],
  });

  // map passes (element, index, array): the object element is logged as its index (0), never as the object.
  assert.deepEqual(mapObjects.pre, [arr(1, [obj(2, [])]), fn('identity', 1)]);
  assert.deepEqual(mapObjects.post, {
    ret: arr(1, [obj(2, [])]),
    throw: null,
    state: [arr(3, [ref(2)]), fn('identity', 1), arr(4, [arr(5, ['identity', 0])])],
  });

  // throwAt2 throws on the third element; the throwing call is logged before it throws.
  assert.deepEqual(mapThrows.post, {
    ret: null,
    throw: {name: 'Error'},
    state: [
      arr(1, [1, 2, 3]),
      fn('throwAt2', 2),
      arr(2, [arr(3, ['throwAt2', 1, 0]), arr(4, ['throwAt2', 2, 1]), arr(5, ['throwAt2', 3, 2])]),
    ],
  });
});

test('describeCase summarizes a written case on one line', () => {
  const c = {id: 'abs/nzero', target: 'Math.abs', static: true, pre: [NEG0], post: {ret: 0, throw: null, state: [NEG0]}};
  assert.equal(describeCase(c), 'abs/nzero Math.abs [static] pre=[{"$":"num","v":"-0"}]');
});

test('renderSuite is deterministic for the same compiled suite', () => {
  const s = defineSuite('det', [{id: 'abs/nan', target: 'Math.abs', static: true, make: () => ({args: [NaN]})}]);
  const text = renderSuite(compileSuite(s));
  assert.equal(text, renderSuite(compileSuite(s)));
  assert.equal(JSON.parse(text).cases[0].pre[0].v, 'NaN');
});

// ---------------------------------------------------------------- seed suites

test('seed suites: at least 600 cases in total; ids unique per suite; only TypeError and RangeError throws', () => {
  let total = 0;
  const names = new Set();
  for (const file of SUITE_FILES) {
    const json = readSuite(file);
    assert.ok(json.cases.length > 0, file);
    assert.equal(new Set(json.cases.map((c) => c.id)).size, json.cases.length, `${file} ids are unique`);
    total += json.cases.length;
    for (const c of json.cases) {
      if (c.post.throw !== null) names.add(c.post.throw.name);
    }
  }
  assert.ok(total >= 600, `total ${total}`);
  assert.deepEqual([...names].sort(), ['RangeError', 'TypeError']);
});

test('seed suites: the target list is the WP-O2 list (every target present, no others)', () => {
  const used = new Set();
  for (const file of SUITE_FILES) {
    for (const c of readSuite(file).cases) used.add(c.target);
  }
  for (const t of WP_TARGETS) assert.ok(used.has(t), `missing target ${t}`);
  assert.equal(used.size, WP_TARGETS.length);
});

test('seed suites: approx marks the libm-dependent functions and nothing exact', () => {
  const approx = new Set();
  const exact = new Set();
  for (const c of readSuite('math.json').cases) {
    (c.approx === 1 ? approx : exact).add(c.target);
  }
  for (const t of ['Math.sin', 'Math.exp', 'Math.log1p', 'Math.cbrt', 'Math.pow', 'Math.atan2', 'Math.hypot']) {
    assert.ok(approx.has(t), t);
  }
  for (const t of ['Math.abs', 'Math.sqrt', 'Math.fround', 'Math.clz32', 'Math.imul', 'Math.round', 'Math.max']) {
    assert.ok(exact.has(t), t);
  }
});

test('seed suites: the first math cases match hand-derived values', () => {
  const byId = new Map(readSuite('math.json').cases.map((c) => [c.id, c]));
  assert.deepEqual(byId.get('abs/nzero').post.ret, 0); // |-0| is +0
  assert.deepEqual(byId.get('min/zeros').post.ret, NEG0); // Math.min(+0, -0) is -0
  assert.deepEqual(byId.get('max/zeros').post.ret, 0); // Math.max(+0, -0) is +0
  assert.deepEqual(byId.get('pow/nan-pzero').post.ret, 1); // x ** 0 is 1 even for NaN
  assert.deepEqual(byId.get('clz32/nan').post.ret, 32); // ToUint32(NaN) is 0
  assert.deepEqual(byId.get('hypot/3-4').post.ret, 5);
  assert.deepEqual(byId.get('abs/no-args').post.ret, NAN);
  assert.deepEqual(byId.get('const/pi').post.ret, Math.PI);
});

test('seed suites: the number cases match hand-derived values', () => {
  const byId = new Map(readSuite('number.json').cases.map((c) => [c.id, c]));
  assert.equal(byId.get('tofixed/round-up').post.ret, '1.00'); // 1.005 is 1.00499...
  assert.deepEqual(byId.get('tofixed/digits101').post.throw, {name: 'RangeError'});
  assert.deepEqual(byId.get('isinteger/obj').post.ret, false); // no coercion of objects
  assert.equal(byId.get('isinteger/str5').post.ret, false); // Number.isInteger does not coerce strings
  assert.deepEqual(byId.get('parseint/neg-radix').post.ret, NAN);
  assert.equal(byId.get('coerce/nbsp').post.ret, 9); // NBSP is white space for Number()
  assert.deepEqual(byId.get('coerce/strnegzero').post.ret, NEG0); // Number('-0') is -0
});

test('seed suites: the object cases match hand-derived values', () => {
  const byId = new Map(readSuite('object-statics.json').cases.map((c) => [c.id, c]));
  assert.deepEqual(byId.get('keys/array-holes').post.ret, arr(1, ['0', '2']));
  assert.deepEqual(byId.get('fromentries/map').post.ret, obj(1, [['2', 'w'], ['k', 'v']])); // integer keys first
  assert.deepEqual(byId.get('hasown/null-object').post.throw, {name: 'TypeError'});
  assert.deepEqual(byId.get('freeze/null').post.ret, null); // non-objects are returned unchanged
  assert.equal(byId.get('isfrozen/number').post.ret, true);
  assert.deepEqual(byId.get('create/null').post.ret, {$: 'obj', id: 1, cls: 'Object', proto: 'null', props: []});
  // trace-format v1.1: integrity is the strongest state that holds, read after the call.
  assert.deepEqual(byId.get('freeze/object').post.ret,
    {$: 'obj', id: 1, cls: 'Object', proto: 'Object.prototype', integrity: 'frozen', props: [['a', 1]]});
  assert.deepEqual(byId.get('seal/object').post.ret,
    {$: 'obj', id: 1, cls: 'Object', proto: 'Object.prototype', integrity: 'sealed', props: [['a', 1]]});
  // An empty non-extensible object has no property that could change, so Object.isFrozen holds: "frozen", not "nonextensible".
  assert.deepEqual(byId.get('preventextensions/object').post.ret,
    {$: 'obj', id: 1, cls: 'Object', proto: 'Object.prototype', integrity: 'frozen', props: []});
});

test('seed suites: every suite regenerates byte-identically (in process)', () => {
  const rendered = renderAll();
  assert.deepEqual(rendered.map((r) => r.file), SUITE_FILES);
  for (const {file, text} of rendered) {
    assert.equal(text, readFileSync(path.join(suitesDir, file), 'utf8'), file);
  }
});

test('seed suites: the generator is deterministic across processes (two child runs, identical bytes)', () => {
  const dirs = [mkdtempSync(path.join(os.tmpdir(), 'bc-gen-')), mkdtempSync(path.join(os.tmpdir(), 'bc-gen-'))];
  try {
    for (const dir of dirs) {
      const r = spawnSync(process.execPath, [genScript, '--out', dir], {env: childEnv(), encoding: 'utf8'});
      assert.equal(r.status, 0, r.stderr);
    }
    assert.deepEqual(readdirSync(dirs[0]).sort(), SUITE_FILES);
    for (const file of SUITE_FILES) {
      const committed = readFileSync(path.join(suitesDir, file));
      assert.deepEqual(readFileSync(path.join(dirs[0], file)), committed, `${file} run 1`);
      assert.deepEqual(readFileSync(path.join(dirs[1], file)), committed, `${file} run 2`);
    }
  } finally {
    for (const dir of dirs) rmSync(dir, {recursive: true, force: true});
  }
});

test('seed suites: each case replays from its pre encoding in a fresh child process', () => {
  for (const file of SUITE_FILES) {
    const full = path.join(suitesDir, file);
    const r = spawnSync(process.execPath, [cli, '--check', full], {env: childEnv(), encoding: 'utf8'});
    assert.equal(r.status, 0, `${file}: ${r.stdout} ${r.stderr}`);
    const result = JSON.parse(r.stdout);
    const json = readSuite(file);
    assert.equal(result.suite, json.suite, file);
    assert.equal(result.cases, json.cases.length, file);
    assert.equal(result.canonical, true, `${file} is the canonical rendering`);
    assert.deepEqual(result.mismatches, [], file);
  }
});

// ---------------------------------------------------------------- negative controls

test('negative control: a mutated expectation in a written suite is reported at its case and path', () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'bc-neg-'));
  try {
    const file = path.join(dir, 'math.json');
    copyFileSync(path.join(suitesDir, 'math.json'), file);
    const json = JSON.parse(readFileSync(file, 'utf8'));
    const c = json.cases.find((x) => x.id === 'abs/mone');
    assert.equal(c.post.ret, 1);
    c.post.ret = 2;
    writeFileSync(file, JSON.stringify(json));
    assert.deepEqual(checkSuite(file).mismatches, [{id: 'abs/mone', path: '$.ret', expected: 2, actual: 1}]);
  } finally {
    rmSync(dir, {recursive: true, force: true});
  }
});

test('negative control: a written target outside the manifest gate is reported, not replayed', () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'bc-neg-'));
  try {
    const file = path.join(dir, 'math.json');
    const json = readSuite('math.json');
    json.cases.find((x) => x.id === 'abs/one').target = 'Object.defineProperty';
    writeFileSync(file, JSON.stringify(json));
    const result = checkSuite(file);
    assert.equal(result.mismatches.length, 1);
    assert.equal(result.mismatches[0].id, 'abs/one');
    assert.match(result.mismatches[0].error, /is X for target test \(row L04, VN4008\)/);
  } finally {
    rmSync(dir, {recursive: true, force: true});
  }
});

test('negative control: a written target absent from the manifest is reported, not replayed', () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'bc-neg-'));
  try {
    const file = path.join(dir, 'math.json');
    const json = readSuite('math.json');
    json.cases.find((x) => x.id === 'abs/one').target = 'Math.nope';
    writeFileSync(file, JSON.stringify(json));
    const result = checkSuite(file);
    assert.equal(result.mismatches.length, 1);
    assert.match(result.mismatches[0].error, /not a static, proto, ctor or const entry/);
  } finally {
    rmSync(dir, {recursive: true, force: true});
  }
});

test('negative control: the CLI exits 1 on a mismatch', () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'bc-neg-'));
  try {
    const file = path.join(dir, 'number.json');
    const json = readSuite('number.json');
    json.cases.find((x) => x.id === 'isinteger/one').post.ret = false;
    writeFileSync(file, JSON.stringify(json));
    const r = spawnSync(process.execPath, [cli, '--check', file], {env: childEnv(), encoding: 'utf8'});
    assert.equal(r.status, 1);
    assert.equal(JSON.parse(r.stdout).mismatches[0].id, 'isinteger/one');
  } finally {
    rmSync(dir, {recursive: true, force: true});
  }
});

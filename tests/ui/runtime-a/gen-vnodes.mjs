// WP-RT-A vnode scenarios (tests/ui/runtime-a/corpora/vnodes.json) and flag values (corpora/flags.json).
// A scenario is a list of calls to the vue@3.5.43 exports (production build). Node runs every step and records the
// canonical description of each result (README.md) or "throw:<name>". The Rust replay (crates/runtime/tests/rta_vnode_replay.rs)
// decodes the same arguments, calls the same exports of vn-runtime and compares the texts. Deterministic.
//   NODE_ENV=production node tests/ui/runtime-a/gen-vnodes.mjs
import {mkdirSync, writeFileSync} from 'node:fs';
import {createRequire} from 'node:module';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

process.env.NODE_ENV ??= 'production';
if (process.env.NODE_ENV !== 'production') throw new Error('gen-vnodes: the oracle needs the production build');
const requireHere = createRequire(import.meta.url);
const Vue = requireHere('vue');
const shared = requireHere('@vue/shared');
const here = path.dirname(fileURLToPath(import.meta.url));
const OUT = path.join(here, 'corpora');

// ---------------------------------------------------------------- canonical description (README.md)

const quote = (s) => JSON.stringify(s);
const NUM_TAG = {'-0': '-0', NaN: 'NaN', Infinity: 'Infinity', '-Infinity': '-Infinity'};
function numText(n) {
  if (Object.is(n, -0)) return '{"$":"num","v":"-0"}';
  if (Number.isNaN(n)) return '{"$":"num","v":"NaN"}';
  if (n === Infinity || n === -Infinity) return `{"$":"num","v":"${n > 0 ? 'Infinity' : '-Infinity'}"}`;
  return String(n);
}
function kindOf(vnode) {
  const t = vnode.type;
  if (typeof t === 'string') return t;
  if (typeof t === 'symbol') return `@${t.description === undefined ? 'undefined' : t.description}`;
  if (typeof t === 'function') return 'fn';
  if (typeof t === 'object' && t !== null) return 'obj';
  return 'other';
}
export function describe(v, depth = 0) {
  if (depth > 200) return '{"$":"deep"}';
  if (v === undefined) return '{"$":"undef"}';
  if (v === null) return 'null';
  if (typeof v === 'boolean') return v ? 'true' : 'false';
  if (typeof v === 'number') return numText(v);
  if (typeof v === 'string') return quote(v);
  if (typeof v === 'symbol') return `{"$":"sym","desc":${v.description === undefined ? 'null' : quote(v.description)}}`;
  if (typeof v === 'function') return `{"$":"fn","name":${describe(v.name, depth + 1)},"length":${describe(v.length, depth + 1)}}`;
  if (v.__v_isVNode === true) {
    const d = depth + 1;
    const dc = v.dynamicChildren === null ? 'null'
      : `{"len":${v.dynamicChildren.length},"kinds":[${v.dynamicChildren.map((c) => quote(kindOf(c))).join(',')}]}`;
    return `{"$":"vnode","type":${describe(v.type, d)},"key":${describe(v.key, d)},"ref":${describe(v.ref, d)},` +
      `"props":${describe(v.props, d)},"children":${describe(v.children, d)},"shapeFlag":${describe(v.shapeFlag, d)},` +
      `"patchFlag":${describe(v.patchFlag, d)},"dynamicProps":${describe(v.dynamicProps, d)},"dynamicChildren":${dc},` +
      `"staticCount":${describe(v.staticCount, d)},"appContext":${describe(v.appContext, d)}}`;
  }
  if (Array.isArray(v)) {
    const parts = [];
    for (let i = 0; i < v.length; i++) parts.push(i in v ? describe(v[i], depth + 1) : '{"$":"hole"}');
    return `[${parts.join(',')}]`;
  }
  const props = Object.keys(v).map((k) => `[${quote(k)},${describe(v[k], depth + 1)}]`);
  return `{"$":"obj","props":[${props.join(',')}]}`;
}

// ---------------------------------------------------------------- argument language (README.md)

const isHole = (x) => x !== null && typeof x === 'object' && !Array.isArray(x) && x.$ === 'hole';
function decode(a, results) {
  if (a === null || typeof a === 'boolean' || typeof a === 'string' || typeof a === 'number') return a;
  if (Array.isArray(a)) {
    const out = new Array(a.length);
    a.forEach((x, i) => {
      if (!isHole(x)) out[i] = decode(x, results);
    });
    return out;
  }
  switch (a.$) {
    case 'undef': return undefined;
    case 'num': return Number(NUM_TAG[a.v] ?? (() => { throw new Error(`bad num ${a.v}`); })());
    case 'ref': return results[a.step];
    case 'export': return Vue[a.name];
    case 'sym': return Symbol.for(a.name);
    case 'obj': {
      const o = {};
      for (const [k, v] of a.props) o[k] = decode(v, results);
      if (a.attach) for (const [k, v] of a.attach) Object.defineProperty(o, k, {value: decode(v, results), writable: true, enumerable: true, configurable: true});
      return o;
    }
    case 'fn': {
      const ret = decode(a.returns ?? null, results);
      const f = function () { return ret; };
      Object.defineProperty(f, 'name', {value: a.name});
      Object.defineProperty(f, 'length', {value: a.length});
      for (const [k, v] of a.props ?? []) Object.defineProperty(f, k, {value: decode(v, results), writable: true, enumerable: true, configurable: true});
      return f;
    }
    default: throw new Error(`bad argument ${JSON.stringify(a)}`);
  }
}

export function runScenario(sc) {
  const results = [];
  const expect = [];
  for (const step of sc.steps) {
    const args = (step.args ?? []).map((a) => decode(a, results));
    try {
      const r = Vue[step.call](...args);
      results.push(r);
      expect.push(describe(r));
    } catch (e) {
      results.push(undefined);
      expect.push(`throw:${e.name}`);
    }
  }
  return expect;
}

// ---------------------------------------------------------------- scenario families

const S = (x) => x; // a string argument
const U = {'$': 'undef'};
const N = (n) => n;
const FRAG = {'$': 'export', name: 'Fragment'};
const TEXT = {'$': 'export', name: 'Text'};
const COMMENT = {'$': 'export', name: 'Comment'};
const STATIC = {'$': 'export', name: 'Static'};
const REF = (step) => ({'$': 'ref', step});
const OBJ = (...props) => ({'$': 'obj', props});
const ARR = (...items) => items;
const FN = (name, length, returns = null, props) => ({'$': 'fn', name, length, returns, ...(props ? {props} : {})});
const NAN = {'$': 'num', v: 'NaN'};
const NEG0 = {'$': 'num', v: '-0'};

const TAGS = ['div', 'span', 'input', 'p'];
const PROPS = [
  null, OBJ(), OBJ(['id', 'x']), OBJ(['class', 'a']), OBJ(['class', ARR('a', OBJ(['b', true]))]),
  OBJ(['style', 'color:red']), OBJ(['style', OBJ(['top', 1])]), OBJ(['onClick', 'h']), OBJ(['key', 'k']),
  OBJ(['key', 0]), OBJ(['ref', 'r']), OBJ(['ref', 5]), OBJ(['ref', OBJ(['x', 1])]),
  OBJ(['ref', 'r'], ['ref_key', 'rk'], ['ref_for', true]), OBJ(['class', N(5)]), OBJ(['class', ARR()]),
  OBJ(['style', ARR('a: 1', OBJ(['b', 2]))]), OBJ(['style', N(3)]), OBJ(['id', null], ['title', U]),
  OBJ(['onUpdate:modelValue', 'm'], ['value', ''])
];
const CHILDREN = [
  null, 'text', '', N(0), N(5), false, true, ARR(), ARR('a', 'b'), ARR(N(1), 'x', null), OBJ(['default', FN('slot', 0, ARR('in'))]),
  FN('fnslot', 0, 'from-fn'), OBJ(['_', 1], ['default', FN('d', 0, 'x')]), NAN, NEG0
];
const PATCHES = [undefined, 0, 1, 8, 16, 512, 1024, -1, 32, 24, null];
const SHAPES = [undefined, 9, 1, 0, 17];
const TYPES = [S('div'), S('span'), S('input'), S(''), FRAG, TEXT, COMMENT, STATIC, null, U, N(0), false, OBJ(['setup', null]), FN('Comp', 0, null)];

const scenarios = [];
const add = (id, steps) => scenarios.push({id, steps});

// A. createElementVNode: tags x props x children, with patch and shape variants.
for (let i = 0; i < 96; i++) {
  const tag = TAGS[i % TAGS.length];
  const props = PROPS[(i * 3) % PROPS.length];
  const children = CHILDREN[(i * 5) % CHILDREN.length];
  const patch = PATCHES[i % PATCHES.length];
  const shape = SHAPES[(i * 7) % SHAPES.length];
  const args = [tag, props, children];
  if (patch !== undefined || shape !== undefined) args.push(patch === undefined ? U : patch);
  if (shape !== undefined) args.push(U, U, U, shape);
  add(`elem/${String(i).padStart(3, '0')}-${tag}`, [{call: 'createElementVNode', args}]);
}
// B. createVNode: type x props x children (+ patch) including components, fragments and comments.
for (let i = 0; i < 96; i++) {
  const type = TYPES[i % TYPES.length];
  const props = PROPS[(i * 7 + 2) % PROPS.length];
  const children = CHILDREN[(i * 3 + 1) % CHILDREN.length];
  const patch = PATCHES[(i * 5) % PATCHES.length];
  const args = [type, props, children];
  if (patch !== undefined) args.push(patch === null ? null : patch);
  add(`vnode/${String(i).padStart(3, '0')}`, [{call: 'createVNode', args}]);
}
// C. Blocks: openBlock / createElementBlock / createBlock / createCommentVNode / setBlockTracking sequences.
const blockRuns = [
  [{call: 'openBlock'}, {call: 'createElementVNode', args: ['span', null, null, 1]}, {call: 'createElementBlock', args: ['div', null, ARR(REF(1)), 0]}],
  [{call: 'openBlock'}, {call: 'createElementVNode', args: ['b', null, 'x']}, {call: 'createElementVNode', args: ['i', null, null, 8]}, {call: 'createElementBlock', args: ['div', null, null, 0]}],
  [{call: 'openBlock'}, {call: 'createBlock', args: ['section', null, null]}],
  [{call: 'openBlock'}, {call: 'createBlock', args: [FRAG, null, ARR(), 64]}],
  [{call: 'openBlock', args: [true]}, {call: 'createElementVNode', args: ['x', null, null, 1]}, {call: 'createElementBlock', args: ['div', null, null, 0]}],
  [{call: 'openBlock'}, {call: 'openBlock'}, {call: 'createElementVNode', args: ['deep', null, null, 1]}, {call: 'createElementBlock', args: ['mid', null, null, 0]}, {call: 'createElementBlock', args: ['top', null, null, 0]}],
  [{call: 'createElementBlock', args: ['orphan', null, null, 0]}],
  [{call: 'createBlock', args: ['alone', null, null]}],
  [{call: 'openBlock'}, {call: 'createCommentVNode', args: ['c', true]}, {call: 'createElementBlock', args: ['p', null, null, 0]}],
  [{call: 'setBlockTracking', args: [-1]}, {call: 'openBlock'}, {call: 'createElementVNode', args: ['t', null, null, 1]}, {call: 'createElementBlock', args: ['u', null, null, 0]}, {call: 'setBlockTracking', args: [1]}],
  [{call: 'setBlockTracking', args: [-1, true]}, {call: 'createBlock', args: ['v', null, null]}, {call: 'setBlockTracking', args: [1]}],
  [{call: 'openBlock'}, {call: 'createElementVNode', args: ['a', null, null, 1]}, {call: 'createBlock', args: ['b', null, ARR(REF(1)), 0, ARR('bound')]}],
  [{call: 'openBlock'}, {call: 'createTextVNode', args: ['txt', 1]}, {call: 'createElementBlock', args: ['w', null, null, 0]}],
  [{call: 'openBlock'}, {call: 'createElementVNode', args: ['q', null, null, 4]}, {call: 'createElementVNode', args: ['r', null, null, 32]}, {call: 'createElementBlock', args: ['s', null, null, 0]}],
  [{call: 'openBlock'}, {call: 'createElementVNode', args: ['keep', null, null, 1]}, {call: 'openBlock', args: [true]}, {call: 'createElementVNode', args: ['hidden', null, null, 1]}, {call: 'createElementBlock', args: ['inner', null, null, 0]}, {call: 'createElementBlock', args: ['outer', null, null, 0]}],
  [{call: 'openBlock'}, {call: 'createVNode', args: [FN('Cmp', 0), null, null, 1]}, {call: 'createElementBlock', args: ['wrap', null, ARR(REF(1)), 0]}],
  [{call: 'openBlock'}, {call: 'createElementVNode', args: ['x1', null, null, 1]}, {call: 'createElementVNode', args: ['x2', null, null, 0]}, {call: 'createBlock', args: [FRAG, null, ARR(REF(1), REF(2)), 0]}],
];
blockRuns.forEach((steps, i) => add(`block/${String(i).padStart(2, '0')}-run`, steps));
for (let i = 0; i < 12; i++) {
  const tag = TAGS[i % TAGS.length];
  add(`block/${String(i).padStart(2, '0')}-nested-${tag}`, [
    {call: 'openBlock'},
    {call: 'createElementVNode', args: [tag, PROPS[(i * 5) % PROPS.length], CHILDREN[(i * 2) % CHILDREN.length], 1 + (i % 3) * 8]},
    {call: 'createElementBlock', args: ['root', null, ARR(REF(1)), 0, i % 2 ? ARR('p') : null]},
  ]);
}
// D. cloneVNode: base vnodes with refs, classes and patch flags, cloned with extra props and merge flags.
const BASES = [
  [{call: 'createElementVNode', args: ['a', OBJ(['href', '/x']), 'link', 8]}],
  [{call: 'createElementVNode', args: ['b', OBJ(['ref', 'rr']), null, 16]}],
  [{call: 'createVNode', args: ['c', OBJ(['class', 'k']), null, -1]}],
  [{call: 'createVNode', args: [FRAG, null, ARR('f'), 64]}],
  [{call: 'createVNode', args: [FN('K', 0), OBJ(['id', 'z']), null]}],
  [{call: 'createTextVNode', args: ['tx']}],
  [{call: 'createCommentVNode', args: ['cm']}],
  [{call: 'createElementVNode', args: ['d', OBJ(['ref', OBJ(['q', 1])])]}],
];
const EXTRAS = [U, null, OBJ(['title', 'T']), OBJ(['class', 'extra']), OBJ(['style', 'top:1px']), OBJ(['ref', 'nr']),
  OBJ(['ref', 7], ['onClick', 'hh']), OBJ(['key', 'nk']), OBJ(['href', null]), OBJ(['onFoo', ARR('x')])];
BASES.forEach((base, bi) => EXTRAS.forEach((extra, ei) => {
  if ((bi + ei) % 3 !== 0 && ei > 3) return;
  const merge = (bi + ei) % 2 === 0;
  add(`clone/${bi}-${String(ei).padStart(2, '0')}`, [...base, {call: 'cloneVNode', args: [REF(base.length - 1), extra, merge, false]}]);
}));
add('clone/no-extra', [...BASES[0], {call: 'cloneVNode', args: [REF(0)]}]);
add('clone/plain-object-throws', [{call: 'cloneVNode', args: [OBJ(['type', 'div'])]}]);
add('clone/transition-flag-no-transition', [...BASES[1], {call: 'cloneVNode', args: [REF(0), OBJ(['id', 'q']), true, true]}]);
// E. mergeProps: pairs and triples of prop objects.
const MERGE_ITEMS = [
  null, U, OBJ(), OBJ(['class', 'a']), OBJ(['class', ARR('b', OBJ(['c', true]))]), OBJ(['class', ''] ), OBJ(['style', 'color:red']),
  OBJ(['style', ARR(OBJ(['top', 1]), 'left: 2px')]), OBJ(['style', OBJ(['width', 3])]), OBJ(['onClick', 'h1']),
  OBJ(['onClick', 'h2']), OBJ(['onKeyup', N(7)]), OBJ(['onFoo', ARR('x', 'y')]), OBJ(['onBar', null]), OBJ(['onUpdate:modelValue', U]),
  OBJ(['onUpdate:modelValue', 'v']), OBJ(['id', 'k']), OBJ(['title', 'second']), OBJ(['', 'dropped']), OBJ(['0', 'zero']),
  OBJ(['role', null]), OBJ(['hidden', false]), OBJ(['onEmpty', '']), OBJ(['class', ARR(ARR('e'), 'f')]), OBJ(['onClick', ARR('x')])
];
for (let i = 0; i < MERGE_ITEMS.length; i++) {
  for (let k = 0; k < 3; k++) {
    const j = (i + 5 * k + 1) % MERGE_ITEMS.length;
    add(`merge/pair-${String(i).padStart(2, '0')}-${k}`, [{call: 'mergeProps', args: [MERGE_ITEMS[i], MERGE_ITEMS[j]]}]);
  }
}
for (let i = 0; i < 10; i++) {
  add(`merge/triple-${i}`, [{call: 'mergeProps', args: [MERGE_ITEMS[(i * 3) % MERGE_ITEMS.length], MERGE_ITEMS[(i * 5 + 2) % MERGE_ITEMS.length], MERGE_ITEMS[(i * 7 + 4) % MERGE_ITEMS.length]]}]);
}
add('merge/none', [{call: 'mergeProps', args: []}]);
// F. Components, teleport and suspense shapes, vnode-typed createVNode (clone path, block replacement), misc.
add('shape/vccopts-class', [{call: 'createVNode', args: [FN('Klass', 0, null, [['__vccOpts', OBJ(['name', 'K'])]]), null, null]}]);
add('shape/vccopts-with-children', [{call: 'createVNode', args: [FN('Klass2', 0, null, [['__vccOpts', OBJ()]]), OBJ(['id', 'a']), 'child']}]);
add('shape/teleport-string', [{call: 'createVNode', args: [OBJ(['__isTeleport', true]), null, 'to-text']}]);
add('shape/teleport-array', [{call: 'createVNode', args: [OBJ(['__isTeleport', true]), null, ARR('x', N(1))]}]);
add('shape/teleport-slots', [{call: 'createVNode', args: [OBJ(['__isTeleport', true]), null, OBJ(['default', FN('td', 0, 'ok')])]}]);
add('shape/suspense-normalize', [{call: 'createVNode', args: [OBJ(['__isSuspense', true], ['normalize', FN('normalize', 1, U)]), null, ARR('s')]}]);
add('shape/suspense-no-normalize-throws', [{call: 'createVNode', args: [OBJ(['__isSuspense', true]), null, ARR('s')]}]);
add('shape/function-children-element', [{call: 'createElementVNode', args: ['div', null, FN('f0', 0, 'called')]}]);
add('shape/slots-object-element', [{call: 'createElementVNode', args: ['div', null, OBJ(['default', FN('dd', 0, ARR('z'))])]}]);
add('shape/slots-object-component', [{call: 'createVNode', args: [OBJ(['setup', null]), null, OBJ(['default', FN('cd', 0, 'comp')])]}]);
add('shape/slots-internal-flag', [{call: 'createVNode', args: [OBJ(['setup', null]), null, OBJ(['_', 1])]}]);
add('shape/slot-with-c-flag', [{call: 'createElementVNode', args: ['div', null, OBJ(['default', FN('marked', 0, 'm', [['_c', true]])])]}]);
add('shape/symbol-children', [{call: 'createVNode', args: ['span', null, {'$': 'sym', name: 'sy'}]}]);
add('shape/boolean-children-true', [{call: 'createElementVNode', args: ['span', null, true]}]);
add('shape/vnode-as-type', [
  {call: 'createVNode', args: [OBJ(['setup', null]), OBJ(['id', 'c1'])]},
  {call: 'createVNode', args: [REF(0), OBJ(['class', 'x']), ARR('kid'), 1]},
]);
add('shape/vnode-as-type-in-block', [
  {call: 'openBlock'},
  {call: 'createVNode', args: [OBJ(['setup', null]), null]},
  {call: 'createVNode', args: [REF(1), OBJ(['class', 'y'])]},
  {call: 'createElementBlock', args: ['root', null, ARR(REF(2)), 0]},
]);
add('shape/vnode-as-type-fragment-in-block', [
  {call: 'openBlock'},
  {call: 'createElementVNode', args: ['p1', null, null, 1]},
  {call: 'createVNode', args: [REF(1), OBJ(['id', 'w'])]},
  {call: 'createElementBlock', args: ['root', null, null, 0]},
]);
add('shape/vnode-as-type-plain-throws', [{call: 'createVNode', args: [OBJ(['__v_isVNode', true]), null]}]);
add('shape/fragment-patch-clone', [{call: 'createVNode', args: [FRAG, null, ARR('a'), 64]}, {call: 'cloneVNode', args: [REF(0), OBJ(['id', 'f'])]}]);
add('shape/clone-patch-minus-one', [{call: 'createVNode', args: ['e', null, null, -1]}, {call: 'cloneVNode', args: [REF(0), OBJ(['id', 'g'])]}]);
add('shape/clone-patch-bits', [{call: 'createElementVNode', args: ['e', null, null, 3]}, {call: 'cloneVNode', args: [REF(0), OBJ(['id', 'h'])]}]);
add('text/default', [{call: 'createTextVNode'}]);
add('text/empty', [{call: 'createTextVNode', args: ['']}]);
add('text/number-flag', [{call: 'createTextVNode', args: [N(42), 1]}]);
add('text/null', [{call: 'createTextVNode', args: [null]}]);
add('comment/default', [{call: 'createCommentVNode'}]);
add('comment/text', [{call: 'createCommentVNode', args: ['hello']}]);
add('comment/as-block', [{call: 'createCommentVNode', args: ['blk', true]}]);
add('comment/as-block-in-block', [{call: 'openBlock'}, {call: 'createCommentVNode', args: ['in', true]}, {call: 'createElementBlock', args: ['q', null, null, 0]}]);
add('static/throws', [{call: 'createStaticVNode', args: ['<b>x</b>', 1]}]);
add('isvnode/true-false', [
  {call: 'isVNode', args: [REF(0)]}, {call: 'isVNode', args: [null]}, {call: 'isVNode', args: [S('x')]}, {call: 'isVNode', args: [OBJ(['__v_isVNode', true])]},
  {call: 'isVNode', args: [OBJ(['__v_isVNode', 1])]}, {call: 'isVNode', args: []}, {call: 'isVNode', args: [N(0)]},
].map((s, i) => (i === 0 ? {call: 'createTextVNode', args: ['v']} : s)));
add('guard/object', [{call: 'guardReactiveProps', args: [OBJ(['id', 'x'])]}]);
add('guard/null', [{call: 'guardReactiveProps', args: [null]}]);
add('guard/falsy-number', [{call: 'guardReactiveProps', args: [N(0)]}]);
add('guard/truthy-string', [{call: 'guardReactiveProps', args: [S('s')]}]);
add('export/symbols', [{call: 'createTextVNode', args: ['t']}]);
add('vnode/nan-key-children', [{call: 'createVNode', args: ['h', OBJ(['key', NAN]), NEG0]}]);
add('vnode/object-ref-raw', [{call: 'createElementVNode', args: ['raw', OBJ(['ref', OBJ(['z', 1])])]}]);
add('vnode/ref-key-only', [{call: 'createElementVNode', args: ['rk', OBJ(['ref_key', 'name'])]}]);
add('vnode/ref-number-zero', [{call: 'createElementVNode', args: ['rz', OBJ(['ref', N(0)])]}]);
add('vnode/style-array-of-objects', [{call: 'createVNode', args: ['st', OBJ(['style', ARR(OBJ(['a', 1]), OBJ(['b', 'x']))])]}]);
add('vnode/class-number-normalized', [{call: 'createVNode', args: ['cn', OBJ(['class', N(5)])]}]);
add('vnode/props-falsy-number', [{call: 'createVNode', args: ['pf', N(0)]}]);
add('vnode/props-string', [{call: 'createVNode', args: ['ps', S('str')]}]);
add('vnode/null-type-comment', [{call: 'createVNode', args: [null]}]);

// G. withMemo and isMemoSame: render functions are native fn values; the cache is a merged (plain) object.
const MEMO_CACHE = {call: 'mergeProps', args: []};
add('memo/first-render', [MEMO_CACHE, {call: 'withMemo', args: [ARR('a', N(1)), FN('r1', 0, OBJ(['tag', 'one'])), REF(0), 0]}]);
add('memo/same-memo-reuses', [MEMO_CACHE,
  {call: 'withMemo', args: [ARR('a'), FN('r2', 0, OBJ(['tag', 'two'])), REF(0), 0]},
  {call: 'withMemo', args: [ARR('a'), FN('r3', 0, OBJ(['tag', 'three'])), REF(0), 0]}]);
add('memo/changed-memo-renders', [MEMO_CACHE,
  {call: 'withMemo', args: [ARR('a'), FN('r4', 0, OBJ(['tag', 'four'])), REF(0), 0]},
  {call: 'withMemo', args: [ARR('b'), FN('r5', 0, OBJ(['tag', 'five'])), REF(0), 0]}]);
add('memo/length-changed', [MEMO_CACHE,
  {call: 'withMemo', args: [ARR('a'), FN('r6', 0, OBJ(['tag', 'six'])), REF(0), 1]},
  {call: 'withMemo', args: [ARR('a', 'b'), FN('r7', 0, OBJ(['tag', 'seven'])), REF(0), 1]}]);
add('memo/nan-and-negative-zero', [MEMO_CACHE,
  {call: 'withMemo', args: [ARR(NAN, N(0)), FN('r8', 0, OBJ(['tag', 'eight'])), REF(0), 2]},
  {call: 'withMemo', args: [ARR(NAN, NEG0), FN('r9', 0, OBJ(['tag', 'nine'])), REF(0), 2]}]);
add('memo/vnode-render-result', [MEMO_CACHE,
  {call: 'createElementVNode', args: ['m', null, null, 1]},
  {call: 'withMemo', args: [ARR('z'), FN('rv', 0, REF(1)), REF(0), 0]},
  {call: 'withMemo', args: [ARR('z'), FN('rw', 0, OBJ(['tag', 'unused'])), REF(0), 0]}]);
add('memo/inside-block-tracks-cached', [{call: 'openBlock'},
  {call: 'createElementVNode', args: ['m2', null, null, 1]}, MEMO_CACHE,
  {call: 'withMemo', args: [ARR('z'), FN('rb', 0, REF(1)), REF(2), 0]},
  {call: 'withMemo', args: [ARR('z'), FN('rc', 0, OBJ()), REF(2), 0]},
  {call: 'createElementBlock', args: ['root', null, null, 0]}]);
add('memo/undefined-render-throws', [MEMO_CACHE, {call: 'withMemo', args: [ARR('a'), FN('ru', 0, U), REF(0), 0]}]);
add('memo/undefined-cache-throws', [{call: 'withMemo', args: [ARR('a'), FN('rx', 0, OBJ()), U, 0]}]);
add('memo/memo-without-slice-throws', [MEMO_CACHE, {call: 'withMemo', args: [OBJ(['length', 1]), FN('rs', 0, OBJ()), REF(0), 0]}]);
add('memo/is-same-true', [MEMO_CACHE, {call: 'withMemo', args: [ARR('a'), FN('rt1', 0, OBJ()), REF(0), 0]},
  {call: 'isMemoSame', args: [REF(1), ARR('a')]}]);
add('memo/is-same-false-value', [{call: 'isMemoSame', args: [OBJ(['memo', ARR('a')]), ARR('b')]}]);
add('memo/is-same-false-length', [{call: 'isMemoSame', args: [OBJ(['memo', ARR('a')]), ARR('a', 'b')]}]);
add('memo/is-same-nan', [{call: 'isMemoSame', args: [OBJ(['memo', ARR(NAN)]), ARR(NAN)]}]);
add('memo/is-same-negative-zero', [{call: 'isMemoSame', args: [OBJ(['memo', ARR(N(0))]), ARR(NEG0)]}]);
add('memo/is-same-no-memo-throws', [{call: 'isMemoSame', args: [OBJ(), ARR()]}]);
add('memo/is-same-null-cached-throws', [{call: 'isMemoSame', args: [null, ARR()]}]);

// ---------------------------------------------------------------- expectations (run by Vue)

// The one documented deviation: createStaticVNode throws TypeError in vn-runtime by design (static HTML is not supported).
// Its expectation is still Vue's own result; the replay requires the port to throw instead, and counts it as a deviation.
const DEVIATIONS = new Map([['static/throws', 'createStaticVNode: static HTML strings are not supported (the SFC options disable hoistStatic)']]);
for (const sc of scenarios) {
  sc.expect = runScenario(sc);
  if (DEVIATIONS.has(sc.id)) sc.unsupported = DEVIATIONS.get(sc.id);
}

// Scenario IDs and steps are written as the Rust replay reads them. Keys are sorted by construction.
const suite = {suite: 'vnodes', scenarios};
mkdirSync(OUT, {recursive: true});
writeFileSync(path.join(OUT, 'vnodes.json'), `${JSON.stringify(suite, null, 1)}\n`);

// Flag values of @vue/shared 3.5.43 (PatchFlags, ShapeFlags, SlotFlags), read from the package.
const flags = {suite: 'flags', PatchFlags: {}, ShapeFlags: {}, SlotFlags: {}};
for (const k of Object.keys(shared.PatchFlags)) if (Number.isNaN(Number(k))) flags.PatchFlags[k] = shared.PatchFlags[k];
for (const k of Object.keys(shared.ShapeFlags)) if (Number.isNaN(Number(k))) flags.ShapeFlags[k] = shared.ShapeFlags[k];
for (const k of Object.keys(shared.SlotFlags)) if (Number.isNaN(Number(k))) flags.SlotFlags[k] = shared.SlotFlags[k];
writeFileSync(path.join(OUT, 'flags.json'), `${JSON.stringify(flags, null, 1)}\n`);

const steps = scenarios.reduce((n, s) => n + s.steps.length, 0);
console.log(`vnodes: ${scenarios.length} scenarios, ${steps} steps`);
console.log(`flags: ${Object.keys(flags.PatchFlags).length + Object.keys(flags.ShapeFlags).length + Object.keys(flags.SlotFlags).length} values`);

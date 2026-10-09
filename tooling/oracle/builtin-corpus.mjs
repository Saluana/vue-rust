// Builtin-call corpora, Node side (docs/dev/builtin-corpus.md, WP-O2). Node (V8) is the oracle: every expected value is
// produced by running the case here and encoding the result with tooling/oracle/trace.mjs (docs/dev/trace-format.md v1.1).
//
//   defineSuite(name, cases)   validates cases; make() returns fresh inputs {this?, args?} on every call
//   compileSuite(suite)        pre = encoding of the first make(); post = result of the call on a second make() with fresh
//                              callback presets. Each case is then replayed from its pre encoding and must reproduce its
//                              post. A case whose result depends on state the format does not carry (getters, hidden
//                              non-enumerable properties, custom prototypes, ...) throws here.
//   renderSuite(json)          deterministic text, one case per line
//   writeSuite(file, suite)    compileSuite + renderSuite + write; returns the text
//   checkSuite(file)           replays a written suite from its pre encodings and reports each mismatch
//   describeCase(c)            one-line summary of a defined or written case
//   targetProblem(path)        why a target may not be used, or null
//   CLI: node tooling/oracle/builtin-corpus.mjs --check <suite.json>   prints the checkSuite result; exit 1 on mismatch
//
// Targets are decided by the standard-library manifest (compiler/profile/stdlib.json) for target 'test': the target must
// be a static, proto, ctor or const entry with status P or A. X, E and absent entries are rejected with the manifest hint.
// Global-call forms (kind 'global', e.g. parseInt) are rejected; write the owning object (Number.parseInt). Host clock and
// random providers (Math.random, Date.now, performance.now) are refused although they are A, because V8 is not seeded.
//
// Case fields beyond docs/dev/builtin-corpus.md (see tests/language/builtins/README.md):
//   "get": 1      the target is a number-valued data property (Math.PI); nothing is called and pre is []
//   "approx": 1   results may differ from the platform libm in the last ulp (transcendental functions)
//   "cbs"         callback presets by argument position; allowed only on TARGETS (see CALLBACK_ARITY)
//
// Callback presets: each call appends [name, ...loggedArgs] to the shared log before it returns or throws. A primitive
// argument is logged as itself. An object, array or function argument is logged as the index that the calling method
// passed (the second callback argument for map-like methods, the third for reduce); sort passes none, so -1. Object
// identity is never logged. Bodies (the native replay must match them):
//   identity(x)=x  double(x)=x*2  isEven(x)=x%2===0  isTruthy(x)=Boolean(x)  index(_,i)=i  pair(x,i)=[x,i]
//   sum(acc,x)=acc+x  concat(acc,x)=[].concat(acc,x)  cmpNum(a,b)=a-b  cmpDesc(a,b)=b-a
//   cmpStr(a,b)=a<b?-1:a>b?1:0  cmpThrow(a,b)=a-b, but throws Error('cmp') on its second call
//   throwAt2(x,i)=x, but throws Error('at2') when i===2  toArr2(x)=[x,x]  keyOf(x)=String(x)
import {mkdirSync, readFileSync, writeFileSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createRecorder} from './trace.mjs';
import {compareDocs, describeDiff} from './compare.mjs';
import {effectiveStatus, isStdlibSupported, lookupStdlib} from '../../compiler/profile/stdlib.mjs';
import {createRequire} from 'node:module';
import {isSupported as isProfileSupported} from '../../compiler/profile/index.mjs';

// ---------------------------------------------------------------- callback presets

export const PRESET_ARITY = Object.freeze({
  identity: 1, double: 1, isEven: 1, isTruthy: 1, index: 2, pair: 2, sum: 2, concat: 2,
  cmpNum: 2, cmpDesc: 2, cmpStr: 2, cmpThrow: 2, throwAt2: 2, toArr2: 1, keyOf: 1,
});

const isObj = (v) => (typeof v === 'object' && v !== null) || typeof v === 'function';
const logged = (v, index) => (isObj(v) ? (Number.isInteger(index) ? index : -1) : v);

// Fresh presets per execution: cmpThrow counts its own calls. `rest[0]` is the index argument of map-like callbacks.
export function makePresets(log) {
  let cmpCalls = 0;
  return {
    identity: function identity(x, ...rest) {
      log.push(['identity', logged(x, rest[0])]);
      return x;
    },
    double: function double(x, ...rest) {
      log.push(['double', logged(x, rest[0])]);
      return x * 2;
    },
    isEven: function isEven(x, ...rest) {
      log.push(['isEven', logged(x, rest[0])]);
      return x % 2 === 0;
    },
    isTruthy: function isTruthy(x, ...rest) {
      log.push(['isTruthy', logged(x, rest[0])]);
      return Boolean(x);
    },
    index: function index(_, i) {
      log.push(['index', logged(_, i), i]);
      return i;
    },
    pair: function pair(x, i) {
      log.push(['pair', logged(x, i), i]);
      return [x, i];
    },
    sum: function sum(acc, x, ...rest) {
      log.push(['sum', logged(acc, rest[0]), logged(x, rest[0])]);
      return acc + x;
    },
    concat: function concat(acc, x, ...rest) {
      log.push(['concat', logged(acc, rest[0]), logged(x, rest[0])]);
      return [].concat(acc, x);
    },
    cmpNum: function cmpNum(a, b) {
      log.push(['cmpNum', logged(a), logged(b)]);
      return a - b;
    },
    cmpDesc: function cmpDesc(a, b) {
      log.push(['cmpDesc', logged(a), logged(b)]);
      return b - a;
    },
    cmpStr: function cmpStr(a, b) {
      log.push(['cmpStr', logged(a), logged(b)]);
      return a < b ? -1 : a > b ? 1 : 0;
    },
    cmpThrow: function cmpThrow(a, b) {
      cmpCalls += 1;
      log.push(['cmpThrow', logged(a), logged(b)]);
      if (cmpCalls === 2) throw new Error('cmp');
      return a - b;
    },
    throwAt2: function throwAt2(x, i) {
      log.push(['throwAt2', logged(x, i), i]);
      if (i === 2) throw new Error('at2');
      return x;
    },
    toArr2: function toArr2(x, ...rest) {
      log.push(['toArr2', logged(x, rest[0])]);
      return [x, x];
    },
    keyOf: function keyOf(x, ...rest) {
      log.push(['keyOf', logged(x, rest[0])]);
      return String(x);
    },
  };
}

// ---------------------------------------------------------------- Vue targets (WP-RT-A)

// `Vue.<name>` targets are the exports of require('vue') (production build), installed as a global `Vue` namespace.
// A name is accepted when compiler/profile classifies it as a supported vue export for origin user or compiler.
export const VUE_PREFIX = 'Vue.';
const requireVue = createRequire(import.meta.url);
export function installVueNamespace() {
  if (process.env.NODE_ENV === undefined) process.env.NODE_ENV = 'production';
  if (process.env.NODE_ENV !== 'production') {
    throw new Error(`installVueNamespace: NODE_ENV is ${process.env.NODE_ENV}; the oracle needs the production build`);
  }
  if (!Object.hasOwn(globalThis, 'Vue')) {
    Object.defineProperty(globalThis, 'Vue', {value: requireVue('vue'), writable: true, configurable: true, enumerable: false});
  }
  return globalThis.Vue;
}
function vueTargetProblem(name) {
  if (!/^[A-Za-z_$][\w$]*$/.test(name)) return `Vue.${name} is not an identifier`;
  const ok = isProfileSupported('vue', name, 'native', {origin: 'user'}) || isProfileSupported('vue', name, 'native', {origin: 'compiler'});
  return ok ? null : `Vue.${name} is not a supported vue export (compiler/profile, origin user or compiler)`;
}

// ---------------------------------------------------------------- targets

// Callback-taking methods and the number of arguments each call passes to its callback. The manifest cannot express this
// (it has no callback metadata), so this is the only hand-written part of target checking. A cbs position is valid only on
// these targets, and a preset may declare at most that many parameters. Every key is a P entry of the manifest.
export const CALLBACK_ARITY = new Map([
  ['Array.prototype.map', 3], // (element, index, array)
  ['Array.prototype.reduce', 4], // (accumulator, element, index, array)
  ['Array.prototype.sort', 2], // (a, b)
  // Added by the maintainer for the builtin WPs (B1 B2 B3 B4 B6):
  ['Array.prototype.forEach', 3], ['Array.prototype.filter', 3], ['Array.prototype.find', 3], ['Array.prototype.findIndex', 3],
  ['Array.prototype.findLast', 3], ['Array.prototype.findLastIndex', 3], ['Array.prototype.some', 3], ['Array.prototype.every', 3],
  ['Array.prototype.flatMap', 3], ['Array.prototype.reduceRight', 4], ['Array.prototype.toSorted', 2], ['Array.from', 2],
  ['Object.groupBy', 2], ['Map.groupBy', 2], ['Map.prototype.forEach', 3], ['Set.prototype.forEach', 3],
  ['JSON.parse', 2], ['JSON.stringify', 2], // reviver / replacer (key, value)
  ['String.prototype.replace', 3], ['String.prototype.replaceAll', 3], // (match, offset, string)
  ['Function.prototype.call', 3], ['Function.prototype.apply', 3], ['Function.prototype.bind', 3], // the function under test is a preset
]);
// Vue targets that take callbacks (WP-RT-A): renderItem receives (item, index, undefined, cached) or (value, key, index, cached).
export const VUE_CALLBACK_ARITY = new Map([['Vue.renderList', 4]]);
export const TARGETS = new Set(CALLBACK_ARITY.keys());

const CORPUS_KINDS = ['static', 'proto', 'ctor', 'const'];

// The manifest entry a corpus target names, or null. Paths are unique across kinds, so the first hit is the only one.
function corpusEntry(p) {
  for (const kind of CORPUS_KINDS) {
    const entry = lookupStdlib(kind, p);
    if (entry !== null) return entry;
  }
  return null;
}

// Host clock and random providers are A on the test target (seeded in the Rust test host), but V8 is not seeded, so the
// oracle cannot record their results. The manifest cannot express that, so they are refused here (WP-O2: Math.random).
const NONDETERMINISTIC = new Set(['Math.random', 'Date.now', 'performance.now']);

// Why a target may not be used, or null. Decided by the standard-library manifest for target 'test', plus the
// nondeterminism refusal above.
export function targetProblem(p) {
  if (p.startsWith(VUE_PREFIX)) return vueTargetProblem(p.slice(VUE_PREFIX.length));
  const entry = corpusEntry(p);
  if (entry === null) {
    if (lookupStdlib('global', p) !== null) {
      return `${p} is a global (kind global), not a static, proto, ctor or const entry`;
    }
    return `${p} is not a static, proto, ctor or const entry of the standard-library manifest`;
  }
  if (NONDETERMINISTIC.has(p)) {
    return `${p} is ${entry.status} for target test (row ${entry.row}), a host clock or random provider: V8 is not seeded, so the oracle cannot record it`;
  }
  if (isStdlibSupported(p, 'test')) return null;
  const status = effectiveStatus(entry, 'test');
  const code = entry.code ? `, ${entry.code}` : '';
  return `${p} is ${status} for target test (row ${entry.row}${code}): ${entry.hint ?? 'not supported'}`;
}

const ownValue = (o, k, p) => {
  if ((typeof o !== 'object' && typeof o !== 'function') || o === null || !Object.hasOwn(o, k)) {
    throw new TypeError(`unknown target ${p}: no own property ${k}`);
  }
  return o[k];
};

// Resolves a dotted path from the global object. owner is the object the last segment was read from.
export function resolveTarget(p) {
  if (p.startsWith(VUE_PREFIX)) installVueNamespace();
  const segs = p.split('.');
  let owner = globalThis;
  for (let i = 0; i < segs.length - 1; i++) owner = ownValue(owner, segs[i], p);
  const name = segs[segs.length - 1];
  return {owner, name, value: ownValue(owner, name, p)};
}

// ---------------------------------------------------------------- case specs

const ID = /^[a-z0-9][a-z0-9/_.+-]*$/;
const CASE_KEYS = new Set(['id', 'target', 'make', 'cbs', 'static', 'construct', 'get', 'approx']);

// The normalized shape shared by defined cases and written cases.
function makeSpec({id, path: p, isNew = false, isStatic = false, isGet = false, approx = false, cbs = null}) {
  if (typeof id !== 'string' || !ID.test(id)) throw new TypeError(`case id ${JSON.stringify(id)} must match ${ID}`);
  if (typeof p !== 'string' || p === '') throw new TypeError(`${id}: target must be a non-empty string`);
  if (isNew && (isStatic || isGet)) throw new TypeError(`${id}: new: targets take no static or get flag`);
  if (isGet && isStatic) throw new TypeError(`${id}: get targets take no static flag`);
  const problem = targetProblem(p);
  if (problem) throw new TypeError(`${id}: ${problem}`);
  const {value} = resolveTarget(p);
  if (isGet ? typeof value !== 'number' : typeof value !== 'function') {
    throw new TypeError(`${id}: ${p} is ${typeof value}, expected ${isGet ? 'a number (use get: 1)' : 'a function'}`);
  }
  if (isNew && (typeof value.prototype !== 'object' || value.prototype === null)) {
    throw new TypeError(`${id}: ${p} is not a constructor`);
  }
  if (cbs !== null) {
    if (!Array.isArray(cbs)) throw new TypeError(`${id}: cbs must be an array`);
    const passed = CALLBACK_ARITY.get(p) ?? VUE_CALLBACK_ARITY.get(p);
    for (const n of cbs) {
      if (n === null || n === undefined) continue;
      if (!Object.hasOwn(PRESET_ARITY, n)) throw new TypeError(`${id}: unknown callback preset ${JSON.stringify(n)}`);
      if (passed === undefined) {
        throw new TypeError(`${id}: ${p} takes no callbacks in the corpus (only ${[...TARGETS].join(', ')} may carry cbs)`);
      }
      if (PRESET_ARITY[n] > passed) {
        throw new TypeError(`${id}: preset ${n} declares ${PRESET_ARITY[n]} parameters but ${p} passes ${passed}`);
      }
    }
  }
  return {
    id,
    path: p,
    target: (isNew ? 'new:' : '') + p,
    isNew,
    isStatic,
    isGet,
    hasThis: !isStatic && !isNew && !isGet,
    approx,
    cbs: cbs === null ? null : [...cbs],
  };
}

// Validates and normalizes a suite. Every case keeps its make() for the executions in compileSuite.
export function defineSuite(name, cases) {
  if (typeof name !== 'string' || !/^[a-z0-9][a-z0-9-]*$/.test(name)) {
    throw new TypeError(`suite name ${JSON.stringify(name)} must match [a-z0-9-]+`);
  }
  if (!Array.isArray(cases) || cases.length === 0) throw new TypeError(`${name}: cases must be a non-empty array`);
  const ids = new Set();
  return {
    suite: name,
    cases: cases.map((c) => {
      if (c === null || typeof c !== 'object') throw new TypeError(`${name}: each case must be an object`);
      for (const k of Object.keys(c)) {
        if (!CASE_KEYS.has(k)) throw new TypeError(`${name}/${c.id}: unknown case field ${JSON.stringify(k)}`);
      }
      if (ids.has(c.id)) throw new TypeError(`${name}: duplicate case id ${JSON.stringify(c.id)}`);
      ids.add(c.id);
      if (typeof c.target !== 'string') throw new TypeError(`${name}/${c.id}: target must be a string`);
      if (typeof c.make !== 'function') throw new TypeError(`${name}/${c.id}: make must be a function`);
      const isNew = c.construct === true || c.target.startsWith('new:');
      const spec = makeSpec({
        id: c.id,
        path: isNew ? c.target.replace(/^new:/, '') : c.target,
        isNew,
        isStatic: c.static === true,
        isGet: c.get === true,
        approx: c.approx === true,
        cbs: c.cbs ?? null,
      });
      return {...spec, make: c.make};
    }),
  };
}

// Spec of a written case (JSON). Flags come from the written fields; validation is the same as for defined cases.
function specFromJson(jc) {
  if (jc === null || typeof jc !== 'object' || typeof jc.target !== 'string') throw new TypeError('written case has no target');
  const isNew = jc.target.startsWith('new:');
  return makeSpec({
    id: jc.id,
    path: isNew ? jc.target.slice(4) : jc.target,
    isNew,
    isStatic: jc.static === true,
    isGet: jc.get === 1,
    approx: jc.approx === 1,
    cbs: Array.isArray(jc.cbs) ? jc.cbs : null,
  });
}

// ---------------------------------------------------------------- execution

// Argument layout: [this?, ...args], with callback presets substituted at the cbs positions.
function buildLayout(s, input) {
  if (input === null || typeof input !== 'object') throw new TypeError(`${s.id}: make() must return an object`);
  if (!s.hasThis && 'this' in input) {
    throw new TypeError(`${s.id}: ${s.isNew ? 'new:' : s.isGet ? 'get' : 'static'} targets take no this`);
  }
  const args = input.args === undefined ? [] : input.args;
  if (!Array.isArray(args)) throw new TypeError(`${s.id}: args must be an array`);
  if (s.isGet && args.length > 0) throw new TypeError(`${s.id}: get targets take no args`);
  return s.hasThis ? [input.this, ...args] : [...args];
}

function applyCbs(s, layout, presets) {
  if (s.cbs === null) return;
  s.cbs.forEach((name, j) => {
    if (name === null || name === undefined) return;
    if (j >= layout.length) throw new RangeError(`${s.id}: callback position ${j} is past the arguments`);
    layout[j] = presets[name];
  });
}

const nameOf = (e) => ((typeof e === 'object' && e !== null) || typeof e === 'function') && typeof e.name === 'string'
  ? e.name
  : null;

// Calls the target. Only the call itself is inside try, so a bug in this library is never recorded as a throw.
function invoke(s, layout) {
  const t = resolveTarget(s.path);
  if (s.isGet) return {ret: t.value, thrown: null};
  const args = layout.slice(s.hasThis ? 1 : 0);
  try {
    if (s.isNew) return {ret: new t.value(...args), thrown: null};
    if (s.isStatic) return {ret: t.value.apply(t.owner, args), thrown: null};
    return {ret: t.value.apply(layout[0], args), thrown: null};
  } catch (e) {
    return {ret: null, thrown: {name: nameOf(e)}};
  }
}

// One encoding pass over [ret, this?, ...args, log] (post): a thrown call records ret as null.
function finish(s, layout, log, res) {
  const thisPart = s.hasThis ? [layout[0]] : [];
  const args = layout.slice(s.hasThis ? 1 : 0);
  const enc = createRecorder().encodeAll([res.thrown ? null : res.ret, ...thisPart, ...args, log]);
  return {ret: enc[0], throw: res.thrown ? {name: res.thrown.name} : null, state: enc.slice(1)};
}

function executeDefined(s) {
  const first = buildLayout(s, s.make());
  applyCbs(s, first, makePresets([]));
  const pre = createRecorder().encodeAll(first);
  const layout = buildLayout(s, s.make());
  const log = [];
  applyCbs(s, layout, makePresets(log));
  return {pre, post: finish(s, layout, log, invoke(s, layout))};
}

function jsonCase(s, pre, post) {
  const out = {id: s.id, target: s.target};
  if (s.isStatic) out.static = true;
  if (s.approx) out.approx = 1;
  if (s.isGet) out.get = 1;
  if (s.cbs !== null) {
    const cbs = Array.from({length: pre.length}, (_, j) => s.cbs[j] ?? null);
    if (cbs.some((n) => n !== null)) out.cbs = cbs;
  }
  out.pre = pre;
  out.post = post;
  return out;
}

// ---------------------------------------------------------------- decoding (replay of a written pre)

const NUM_VALUES = {'-0': -0, NaN, Infinity, '-Infinity': -Infinity};

// trace-format v1.1 `integrity`, applied after the properties (as the Rust decoder does). Absent means extensible.
function applyIntegrity(o, state) {
  if (state === undefined) return;
  if (state === 'frozen') Object.freeze(o);
  else if (state === 'sealed') Object.seal(o);
  else if (state === 'nonextensible') Object.preventExtensions(o);
  else throw new TypeError(`unsupported integrity ${JSON.stringify(state)}`);
}

// Decodes one trace-format value list (one encoding pass) into runtime values. Supported: undefined, null, booleans,
// numbers, strings, plain objects (proto Object.prototype or null), arrays (with holes), Map, Set, integrity states and
// callback presets. Anything else throws, so an input the format cannot carry fails loudly.
function decodeAll(list, presetFor) {
  if (!Array.isArray(list)) throw new TypeError('pre must be an array');
  const byId = new Map();
  const dec = (e) => {
    if (e === null || typeof e === 'boolean' || typeof e === 'string' || typeof e === 'number') return e;
    if (typeof e !== 'object' || Array.isArray(e)) throw new TypeError(`not a trace value: ${JSON.stringify(e)}`);
    switch (e.$) {
      case 'undef':
        return undefined;
      case 'num':
        if (!Object.hasOwn(NUM_VALUES, e.v)) throw new TypeError(`bad number marker ${JSON.stringify(e.v)}`);
        return NUM_VALUES[e.v];
      case 'fn':
        return presetFor(e.name, e.length);
      case 'ref':
        if (!byId.has(e.id)) throw new TypeError(`ref to an object not yet expanded: ${e.id}`);
        return byId.get(e.id);
      case 'obj':
        return decodeObj(e);
      default:
        throw new TypeError(`unsupported trace value ${JSON.stringify(e.$)}`);
    }
  };
  const decodeObj = (e) => {
    if (!Number.isInteger(e.id) || byId.has(e.id)) throw new TypeError(`bad object id ${JSON.stringify(e.id)}`);
    let o;
    switch (e.cls) {
      case 'Array':
        if (e.proto !== 'other') throw new TypeError(`unsupported Array prototype ${e.proto}`);
        o = [];
        byId.set(e.id, o);
        o.length = e.len;
        e.elems.forEach((el, i) => {
          if (!(el !== null && typeof el === 'object' && el.$ === 'hole')) o[i] = dec(el);
        });
        break;
      case 'Object':
        if (e.proto === 'Object.prototype') o = {};
        else if (e.proto === 'null') o = Object.create(null);
        else throw new TypeError(`unsupported object prototype ${e.proto}`);
        byId.set(e.id, o);
        break;
      case 'Map':
        if (e.proto !== 'other') throw new TypeError(`unsupported Map prototype ${e.proto}`);
        o = new Map();
        byId.set(e.id, o);
        for (const [k, v] of e.entries) o.set(dec(k), dec(v));
        break;
      case 'Set':
        if (e.proto !== 'other') throw new TypeError(`unsupported Set prototype ${e.proto}`);
        o = new Set();
        byId.set(e.id, o);
        for (const v of e.values) o.add(dec(v));
        break;
      default:
        throw new TypeError(`unsupported class ${JSON.stringify(e.cls)}`);
    }
    for (const [k, v] of e.props) {
      Object.defineProperty(o, k, {value: dec(v), writable: true, enumerable: true, configurable: true});
    }
    applyIntegrity(o, e.integrity);
    return o;
  };
  return list.map((e) => dec(e));
}

// Replays one written case from its pre encoding and returns the first difference against its post, or null.
function replayDiff(jc) {
  const s = specFromJson(jc);
  if (jc.post === null || typeof jc.post !== 'object') throw new TypeError(`${s.id}: post is missing`);
  const log = [];
  const presets = makePresets(log);
  const values = decodeAll(jc.pre, (name, length) => {
    if (!Object.hasOwn(PRESET_ARITY, name) || PRESET_ARITY[name] !== length) {
      throw new TypeError(`${s.id}: unknown callback ${JSON.stringify(name)}/${length}`);
    }
    return presets[name];
  });
  const rt = compareDocs(createRecorder().encodeAll(values), jc.pre);
  if (rt) throw new Error(`${s.id}: pre does not round-trip: ${describeDiff(rt)}`);
  if (s.isGet && values.length !== 0) throw new TypeError(`${s.id}: get case has arguments`);
  if (s.cbs !== null) {
    s.cbs.forEach((name, j) => {
      if (name !== null && name !== undefined && values[j] !== presets[name]) {
        throw new TypeError(`${s.id}: position ${j} is not callback ${name}`);
      }
    });
  }
  const actual = finish(s, values, log, invoke(s, values));
  return compareDocs({ret: jc.post.ret, throw: jc.post.throw, state: jc.post.state}, actual);
}

// ---------------------------------------------------------------- suites

// Runs every defined case and replays it from its own pre encoding. Throws if any case is not reproducible.
export function compileSuite(suite) {
  if (suite === null || typeof suite !== 'object' || typeof suite.suite !== 'string' || !Array.isArray(suite.cases)) {
    throw new TypeError('compileSuite expects a defineSuite() result');
  }
  const cases = suite.cases.map((s) => {
    let executed;
    try {
      executed = executeDefined(s);
    } catch (e) {
      throw new Error(`${suite.suite}/${s.id}: ${e.message}`);
    }
    const jc = jsonCase(s, executed.pre, executed.post);
    let d;
    try {
      d = replayDiff(jc);
    } catch (e) {
      throw new Error(`${suite.suite}/${s.id}: not replayable from its pre encoding: ${e.message}`);
    }
    if (d) throw new Error(`${suite.suite}/${s.id}: not replayable from its pre encoding: ${describeDiff(d)}`);
    return jc;
  });
  return {suite: suite.suite, cases};
}

// Deterministic text: fixed key order (set by jsonCase), one case per line, final newline.
export function renderSuite(json) {
  if (!Array.isArray(json.cases) || json.cases.length === 0) throw new TypeError('a suite needs at least one case');
  const lines = json.cases.map((c) => `    ${JSON.stringify(c)}`);
  return `{\n  "suite": ${JSON.stringify(json.suite)},\n  "cases": [\n${lines.join(',\n')}\n  ]\n}\n`;
}

export function writeSuite(file, suite) {
  const text = renderSuite(compileSuite(suite));
  mkdirSync(path.dirname(file), {recursive: true});
  writeFileSync(file, text);
  return text;
}

// Replays every case of a written suite from its pre encoding. Returns {suite, cases, canonical, mismatches}.
export function checkSuite(file) {
  const text = readFileSync(file, 'utf8');
  const json = JSON.parse(text);
  if (typeof json.suite !== 'string' || !Array.isArray(json.cases)) throw new TypeError(`${file}: not a builtin suite`);
  const mismatches = [];
  const seen = new Set();
  for (const jc of json.cases) {
    const id = jc === null || typeof jc !== 'object' ? undefined : jc.id;
    if (seen.has(id)) {
      mismatches.push({id, error: 'duplicate case id'});
      continue;
    }
    seen.add(id);
    try {
      const d = replayDiff(jc);
      if (d) mismatches.push({id, path: d.path, expected: d.a, actual: d.b});
    } catch (e) {
      mismatches.push({id, error: e.message});
    }
  }
  return {
    suite: json.suite,
    cases: json.cases.length,
    canonical: json.cases.length > 0 && renderSuite(json) === text,
    mismatches,
  };
}

// One-line summary for debugging: id, target, flags, callback positions and the encoded pre.
export function describeCase(c) {
  let s;
  let pre;
  if (typeof c.make === 'function') {
    s = c;
    const layout = buildLayout(s, c.make());
    applyCbs(s, layout, makePresets([]));
    pre = createRecorder().encodeAll(layout);
  } else {
    s = specFromJson(c);
    pre = c.pre;
  }
  const flags = [s.isStatic && 'static', s.isGet && 'get', s.approx && 'approx'].filter(Boolean);
  const cbs = s.cbs === null ? '' : ` cbs=${JSON.stringify(s.cbs)}`;
  return `${s.id} ${s.target}${flags.map((f) => ` [${f}]`).join('')}${cbs} pre=${JSON.stringify(pre)}`;
}

// ---------------------------------------------------------------- CLI

const isMain = process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  const args = process.argv.slice(2);
  if (args.length === 2 && args[0] === '--check') {
    const result = checkSuite(args[1]);
    process.stdout.write(`${JSON.stringify(result)}\n`);
    process.exitCode = result.mismatches.length === 0 ? 0 : 1;
  } else {
    process.stderr.write('usage: node tooling/oracle/builtin-corpus.mjs --check <suite.json>\n');
    process.exitCode = 2;
  }
}

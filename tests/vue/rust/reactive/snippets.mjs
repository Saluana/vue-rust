// WP-R1b oracle snippets. Each runs against the real @vue/reactivity 3.5.43 production build (gen-expected.mjs). `R` is
// the module and `log` records one line per call. Rust replays every snippet in crates/reactivity/tests/r1b_*.rs.
export default [
  {id: 'r1b-flags-reactive', code: `
const {reactive, isReactive, isReadonly, isShallow, isProxy, toRaw} = R;
const raw = {a: 1};
const p = reactive(raw);
log('same', p === reactive(raw), p === raw, reactive(p) === p);
log('flags', isReactive(p), isReadonly(p), isShallow(p), isProxy(p));
log('raw', isReactive(raw), isReadonly(raw), isShallow(raw), isProxy(raw));
log('toRaw', toRaw(p) === raw, toRaw(raw) === raw, toRaw(1));
`},
  {id: 'r1b-readonly-flags', code: `
const {readonly, reactive, isReactive, isReadonly, isShallow, isProxy, toRaw} = R;
const raw = {a: 1};
const r = readonly(raw);
log('flags', isReactive(r), isReadonly(r), isShallow(r), isProxy(r), toRaw(r) === raw);
log('same', readonly(raw) === r, readonly(r) === r, reactive(r) === r);
const rp = readonly(reactive(raw));
log('nested', isReactive(rp), isReadonly(rp), toRaw(rp) === raw, rp === r);
log('reactive-of-nested', reactive(rp) === rp);
`},
  {id: 'r1b-shallow-flags', code: `
const {shallowReactive, shallowReadonly, reactive, readonly, isReactive, isReadonly, isShallow, toRaw} = R;
const raw = {a: {b: 1}};
const s = shallowReactive(raw);
log('sr', isReactive(s), isReadonly(s), isShallow(s), s === reactive(raw), toRaw(s) === raw);
const sro = shallowReadonly(raw);
log('sro', isReactive(sro), isReadonly(sro), isShallow(sro), sro === readonly(raw));
log('nested-shallow', isReactive(s.a), isReactive(sro.a), isReadonly(sro.a));
`},
  {id: 'r1b-get-tracks-property', code: `
const {reactive, effect} = R;
const p = reactive({a: 1});
let runs = 0;
effect(() => { runs++; log('read', p.a); });
p.a = 2;
log('runs', runs);
p.a = 2;
log('runs-same', runs);
p.b = 3;
log('runs-new-key', runs);
`},
  {id: 'r1b-has-tracks-key', code: `
const {reactive, effect} = R;
const p = reactive({});
let runs = 0;
effect(() => { runs++; log('has', 'x' in p); });
p.x = 1;
log('runs', runs);
delete p.x;
log('runs-after-delete', runs);
`},
  {id: 'r1b-delete-missing-no-trigger', code: `
const {reactive, effect} = R;
const p = reactive({a: 1});
let runs = 0;
effect(() => { runs++; log('ok', p.a); });
log('del-missing', delete p.zz, runs);
log('del', delete p.a, runs);
`},
  {id: 'r1b-ownkeys-tracks-iterate', code: `
const {reactive, effect} = R;
const p = reactive({a: 1});
let runs = 0;
effect(() => { runs++; log('keys', Object.keys(p).join(',')); });
p.b = 2;
log('after-add', runs);
p.a = 5;
log('after-set', runs);
delete p.a;
log('after-delete', runs);
`},
  {id: 'r1b-for-in-tracks', code: `
const {reactive, effect} = R;
const p = reactive({a: 1, b: 2});
let runs = 0;
effect(() => { runs++; let s = ''; for (const k in p) s += k; log('forin', s); });
p.c = 3;
log('runs', runs);
p.a = 10;
log('runs-set', runs);
`},
  {id: 'r1b-set-same-value-no-trigger', code: `
const {reactive, effect} = R;
const p = reactive({n: NaN, z: 0});
let runs = 0;
effect(() => { runs++; log('read', p.n, p.z); });
p.n = NaN;
p.z = -0;
log('runs', runs);
p.z = 1;
log('runs2', runs);
`},
  {id: 'r1b-getter-receiver-tracks', code: `
const {reactive, effect} = R;
const raw = {_v: 1, get v() { return this._v * 10; }};
const p = reactive(raw);
let runs = 0;
effect(() => { runs++; log('v', p.v); });
p._v = 2;
log('runs', runs);
`},
  {id: 'r1b-setter-receiver-is-proxy', code: `
const {reactive} = R;
const seen = [];
const raw = {_x: 0, set x(v) { seen.push(this === p); this._x = v; }, get x() { return this._x; }};
const p = reactive(raw);
p.x = 5;
log('setter', seen.length, seen[0], p._x, raw._x);
`},
  {id: 'r1b-symbol-keys', code: `
const {reactive, effect} = R;
const s = Symbol('s');
const p = reactive({[s]: 1});
let runs = 0;
effect(() => { runs++; log('sym', p[s]); });
p[s] = 2;
log('runs', runs);
log('has-sym', s in p, Symbol.iterator in reactive([]));
`},
  {id: 'r1b-nested-object-tracks', code: `
const {reactive, effect} = R;
const p = reactive({inner: {x: 1}});
let runs = 0;
effect(() => { runs++; log('x', p.inner.x); });
p.inner.x = 2;
log('runs', runs);
log('same-inner', p.inner === p.inner, p.inner !== p.inner.__proto__);
`},
  {id: 'r1b-proxy-cache-identity', code: `
const {reactive, readonly, shallowReactive} = R;
const raw = {};
const a = reactive(raw);
const b = reactive(raw);
log('reactive-cached', a === b, reactive(a) === a);
const r1 = readonly(raw), r2 = readonly(raw);
log('readonly-cached', r1 === r2, readonly(a) === r1, reactive(r1) === r1);
const s1 = shallowReactive(raw);
log('shallow-cached', s1 === shallowReactive(raw), s1 !== a);
`},
  {id: 'r1b-readonly-set-and-delete-silent', code: `
const {readonly} = R;
const raw = {a: 1};
const r = readonly(raw);
r.a = 2;
log('after set', r.a, raw.a);
log('delete', delete r.a, r.a);
`},
  {id: 'r1b-readonly-nested-wrapped', code: `
const {readonly} = R;
const raw = {inner: {x: 1}};
const r = readonly(raw);
log('inner-ro', r.inner.x, readonly(raw.inner) === r.inner);
r.inner.x = 9;
log('inner-after', raw.inner.x, r.inner.x);
`},
  {id: 'r1b-markraw-skips-wrapping', code: `
const {reactive, markRaw, isReactive} = R;
const raw = {a: {b: 1}};
const m = markRaw(raw);
log('marked', m === raw, raw.__v_skip);
const p = reactive(raw);
log('skip', p === raw, isReactive(p), reactive(raw.a) === raw.a);
const outer = reactive({m: markRaw({k: 1})});
log('nested-marked', isReactive(outer.m), outer.m.k);
`},
  {id: 'r1b-frozen-and-nonextensible-bypass', code: `
const {reactive, readonly, isReactive} = R;
const f = Object.freeze({a: {x: 1}});
const p = reactive(f);
log('frozen', p === f, isReactive(p), readonly(f) === f);
const ne = Object.preventExtensions({q: 1});
log('nonext', reactive(ne) === ne, readonly(ne) === ne);
`},
  {id: 'r1b-non-plain-and-primitives-bypass', code: `
const {reactive, isReactive} = R;
const d = new Number(7);
const pr = Promise.resolve(1);
log('wrapper', reactive(d) === d, isReactive(d), reactive(pr) === pr);
const fn = function () {};
log('fn', reactive(fn) === fn);
log('prims', reactive(1), reactive('s'), reactive(null), reactive(undefined));
`},
  {id: 'r1b-refs-unwrap-in-reactive', code: `
const {reactive, ref, isRef} = R;
const count = ref(1);
const p = reactive({count, list: [1]});
log('unwrap', p.count, isRef(p.count));
p.count = 5;
log('written', count.value, p.count);
const other = ref(0);
p.count = other;
log('replaced', p.count, count.value, other.value);
`},
  {id: 'r1b-shallow-reactive-keeps-refs', code: `
const {reactive, shallowReactive, ref} = R;
const r = ref(2);
const p = shallowReactive({r});
log('shallow-keep', p.r === r, p.r.value);
const q = reactive({r});
log('deep-unwrap', q.r === r, q.r);
`},
  {id: 'r1b-tor-chain-and-isproxy', code: `
const {reactive, readonly, shallowReactive, toRaw, isProxy} = R;
const raw = {};
const deep = readonly(reactive(raw));
log('chain', toRaw(deep) === raw, isProxy(deep), isProxy(raw));
log('sr', toRaw(shallowReactive(raw)) === raw, toRaw(readonly(shallowReactive(raw))) === raw);
`},
  {id: 'r1b-array-index-and-length-tracking', code: `
const {reactive, effect} = R;
const arr = reactive([1, 2, 3]);
let runs = 0;
effect(() => { runs++; log('idx', arr[1], arr.length); });
arr[1] = 5;
log('runs', runs);
arr[5] = 9;
log('runs-grow', runs, arr.length);
arr.length = 2;
log('runs-shrink', runs, arr.length);
`},
  {id: 'r1b-array-push-effects', code: `
const {reactive, effect} = R;
const arr = reactive([]);
let runs = 0;
effect(() => { runs++; log('len', arr.length); });
arr.push(1);
arr.push(2, 3);
log('runs', runs, arr.length);
`},
  {id: 'r1b-array-pop-shift-unshift', code: `
const {reactive, effect} = R;
const arr = reactive([1, 2, 3]);
let runs = 0;
effect(() => { runs++; log('first', arr[0], arr.length); });
log('pop', arr.pop(), runs);
log('shift', arr.shift(), runs);
arr.unshift(7, 8);
log('unshift', runs, arr.join('-'));
`},
  {id: 'r1b-array-splice-effect', code: `
const {reactive, effect} = R;
const arr = reactive(['a', 'b', 'c', 'd']);
let runs = 0;
effect(() => { runs++; log('sp', arr.length, arr[1]); });
const removed = arr.splice(1, 2, 'x');
log('removed', removed.length, removed[0], arr.length, runs);
`},
  {id: 'r1b-array-sort-reverse-effect', code: `
const {reactive, effect} = R;
const arr = reactive([3, 1, 2]);
let runs = 0;
effect(() => { runs++; log('s', arr[0]); });
arr.sort();
log('sorted-runs', runs, arr[0]);
arr.reverse();
log('rev-runs', runs, arr[0]);
`},
  {id: 'r1b-array-includes-identity', code: `
const {reactive, toRaw} = R;
const obj = {};
const arr = reactive([obj, 1]);
log('incl-raw', arr.includes(obj), arr.indexOf(obj), arr.lastIndexOf(obj));
const po = reactive(obj);
log('incl-proxy', arr.includes(po), arr.indexOf(po), arr.lastIndexOf(po));
log('nan', reactive([NaN]).includes(NaN), reactive([NaN]).indexOf(NaN));
log('same-proxy', arr.includes(toRaw(arr)));
`},
  {id: 'r1b-array-method-identity', code: `
const {reactive} = R;
const arr = reactive([1]);
log('identity', arr.push === arr.push, arr.includes === arr.includes, arr.forEach === reactive([2]).forEach);
log('own-hasOwn', arr.hasOwnProperty === arr.hasOwnProperty, arr.hasOwnProperty('0'), arr.hasOwnProperty('length'));
`},
  {id: 'r1b-array-isarray-and-tostring', code: `
const {reactive, readonly, isReactive} = R;
const a = reactive([1, 2]);
log('isArray', Array.isArray(a), a.length, isReactive(a), Object.prototype.toString.call(a));
log('ro', Array.isArray(readonly([])), String(readonly([1, 2])));
`},
  {id: 'r1b-array-iteration-wraps-items', code: `
const {reactive, isReactive} = R;
const a = reactive([{v: 1}, 2]);
const out = [];
for (const x of a) out.push(isReactive(x) ? 'r' : typeof x);
log('iter', out.join(','));
for (const [i, x] of a.entries()) log('entry', i, isReactive(x));
let s = '';
a.forEach((x, i, arr) => { s += isReactive(x) + ':' + (arr === a) + ';'; });
log('forEach', s);
`},
  {id: 'r1b-map-flags-and-identity', code: `
const {reactive, readonly, shallowReactive, isReactive, isReadonly, isShallow, isProxy, toRaw} = R;
const raw = new Map();
const p = reactive(raw);
log('same', p === reactive(raw), p === raw, reactive(p) === p, p instanceof Map);
log('flags', isReactive(p), isReadonly(p), isShallow(p), isProxy(p), toRaw(p) === raw);
const r = readonly(raw);
log('readonly', isReactive(r), isReadonly(r), isShallow(r), readonly(raw) === r, reactive(r) === r);
const s = shallowReactive(raw);
log('shallow', isReactive(s), isShallow(s), s === p, toRaw(s) === raw);
log('proto', Object.getPrototypeOf(p) === Map.prototype, Object.prototype.toString.call(p));
`},
  {id: 'r1b-map-get-set-has-size-effects', code: `
const {reactive, effect} = R;
const m = reactive(new Map());
let runs = 0;
effect(() => { runs++; log('size', m.size, m.has('a'), m.get('a')); });
m.set('a', 1);
log('runs', runs);
m.set('a', 1);
log('runs-same', runs);
m.set('a', 2);
log('runs-change', runs);
m.delete('a');
log('runs-delete', runs);
m.delete('missing');
log('runs-missing', runs);
m.clear();
log('runs-clear-empty', runs);
`},
  {id: 'r1b-map-clear-triggers', code: `
const {reactive, effect} = R;
const m = reactive(new Map([['x', 1], ['y', 2]]));
let runs = 0;
effect(() => { runs++; log('size', m.size); });
m.clear();
log('after-clear', runs, m.size);
`},
  {id: 'r1b-set-add-delete-has-tracking', code: `
const {reactive, effect} = R;
const s = reactive(new Set([1]));
let runs = 0;
effect(() => { runs++; log('has2', s.has(2), s.size); });
log('add-ret', s.add(2) === s, runs);
log('add-dup', s.add(2) === s, runs);
s.delete(1);
log('delete-ret', s.delete(9), runs);
log('final', s.size, s.has(1), runs);
`},
  {id: 'r1b-map-foreach-wraps-values', code: `
const {reactive, isReactive, toRaw} = R;
const inner = {v: 1};
const m = reactive(new Map([['k', inner]]));
const seen = [];
m.forEach((v, k, map) => { seen.push(isReactive(v), k, map === m); });
log('forEach', seen.join(','));
log('get-wrapped', isReactive(m.get('k')), toRaw(m.get('k')) === inner);
`},
  {id: 'r1b-map-iterators-wrap-entries', code: `
const {reactive, isReactive} = R;
const m = reactive(new Map([['a', {n: 1}], ['b', 2]]));
const out = [];
for (const [k, v] of m) out.push(k, isReactive(v));
log('iter', out.join(','));
log('keys', [...m.keys()].join(','));
log('values', [...m.values()].map((v) => isReactive(v)).join(','));
log('entries', [...m.entries()].length);
`},
  {id: 'r1b-map-keys-tracking-iterate-key', code: `
const {reactive, effect} = R;
const m = reactive(new Map([['a', 1]]));
let runs = 0;
effect(() => { runs++; log('keys', [...m.keys()].join('|')); });
m.set('a', 5);
log('runs-value-change', runs);
m.set('b', 2);
log('runs-add', runs);
`},
  {id: 'r1b-map-values-track-on-value-change', code: `
const {reactive, effect} = R;
const m = reactive(new Map([['a', 1]]));
let runs = 0;
effect(() => { runs++; log('values', [...m.values()].join('|')); });
m.set('a', 5);
log('runs', runs);
`},
  {id: 'r1b-map-object-keys-raw-and-proxy', code: `
const {reactive, toRaw} = R;
const k = {};
const m = reactive(new Map());
m.set(k, 'raw');
log('get-raw', m.get(k), m.get(reactive(k)), m.has(reactive(k)), m.has(k));
const raw = toRaw(m);
log('stored-raw-key', raw.has(k), raw.has(reactive(k)));
`},
  {id: 'r1b-map-set-stores-raw-value', code: `
const {reactive, toRaw, isReactive} = R;
const v = {n: 1};
const m = reactive(new Map());
m.set('p', reactive(v));
log('stored', toRaw(m).get('p') === v, isReactive(m.get('p')), m.get('p').n);
`},
  {id: 'r1b-map-readonly-methods', code: `
const {readonly, isReadonly} = R;
const raw = new Map([['a', {x: 1}]]);
const r = readonly(raw);
log('ro-set', r.set('b', 2) === r, raw.has('b'), r.size);
log('ro-delete', r.delete('a'), r.has('a'), raw.size);
r.clear();
log('ro-clear', raw.size);
log('ro-get', isReadonly(r.get('a')), r.get('a').x);
`},
  {id: 'r1b-shallow-map-values-not-wrapped', code: `
const {shallowReactive, isReactive, effect} = R;
const inner = {n: 1};
const m = shallowReactive(new Map([['k', inner]]));
log('shallow-get', m.get('k') === inner, isReactive(m.get('k')));
let runs = 0;
effect(() => { runs++; log('size', m.size); });
m.set('j', {});
log('runs', runs, m.size);
`},
  {id: 'r1b-readonly-map-values-wrapped', code: `
const {readonly, reactive, isReadonly, isReactive} = R;
const inner = {n: 1};
const raw = new Map([['k', inner]]);
const r = readonly(reactive(raw));
const v = r.get('k');
log('ro-of-reactive', isReadonly(v), isReactive(v), v.n);
const e2 = [...r.values()][0];
log('ro-values', isReadonly(e2), isReactive(e2));
`},
  {id: 'r1b-set-foreach-and-iteration', code: `
const {reactive, isReactive} = R;
const s = reactive(new Set([{a: 1}, 2]));
const out = [];
s.forEach((v, v2, set) => out.push(isReactive(v), v2 === v, set === s));
log('foreach', out.join(','));
const vals = [];
for (const v of s) vals.push(isReactive(v));
log('iter', vals.join(','));
log('keys-same', [...s.keys()].length, [...s.entries()][0][0] === [...s.values()][0]);
`},
  {id: 'r1b-set-tracks-iteration-on-add', code: `
const {reactive, effect} = R;
const s = reactive(new Set());
let runs = 0;
effect(() => { runs++; let n = 0; for (const _ of s) n++; log('count', n); });
s.add(1);
s.add(1);
s.delete(1);
log('runs', runs);
`},
  {id: 'r1b-collection-mutate-during-iteration', code: `
const {reactive} = R;
const m = reactive(new Map([['a', 1]]));
const seen = [];
for (const [k] of m) { seen.push(k); if (k === 'a') m.set('b', 2); }
log('seen', seen.join(','), m.size);
const s = reactive(new Set([1]));
const sv = [];
for (const v of s) { sv.push(v); if (v < 3) s.add(v + 1); }
log('set-seen', sv.join(','));
`},
  {id: 'r1b-map-chainable-and-size', code: `
const {reactive} = R;
const m = reactive(new Map());
log('chain', m.set('a', 1).set('b', 2) === m, m.size);
`},
  {id: 'r1b-map-nan-zero-keys', code: `
const {reactive} = R;
const m = reactive(new Map());
m.set(NaN, 'n');
m.set(-0, 'z');
log('nan', m.get(NaN), m.has(NaN), m.get(0), m.has(-0), m.size);
`},
  {id: 'r1b-collection-get-tracks-key', code: `
const {reactive, effect} = R;
const m = reactive(new Map());
let runs = 0;
effect(() => { runs++; log('get-b', m.get('b')); });
m.set('a', 1);
log('runs-other', runs);
m.set('b', 2);
log('runs-b', runs);
`},
  {id: 'r1b-readonly-set-clear-silent', code: `
const {readonly} = R;
const s = new Set([1]);
const r = readonly(s);
log('clear', r.clear(), r.add(2) === r, r.delete(1), s.size, r.has(1));
`},
  {id: 'r1b-map-entry-object-deep-tracking', code: `
const {reactive, effect} = R;
const inner = {n: 1};
const m = reactive(new Map([['a', inner]]));
let runs = 0;
effect(() => { runs++; let s = 0; for (const [, v] of m) s += v.n; log('sum', s); });
m.get('a').n = 4;
log('runs', runs);
`},
  {id: 'r1b-set-of-proxies-stores-raw', code: `
const {reactive, toRaw} = R;
const obj = {};
const s = reactive(new Set());
s.add(reactive(obj));
log('stored', toRaw(s).has(obj), s.has(obj), s.has(reactive(obj)), s.size);
`},
  {id: 'r1b-map-delete-during-iteration', code: `
const {reactive} = R;
const m = reactive(new Map([['a', 1], ['b', 2], ['c', 3]]));
const seen = [];
for (const [k] of m) { seen.push(k); if (k === 'a') m.delete('b'); }
log('seen', seen.join(','), m.size);
`},
  {id: 'r1b-shallow-set-values-not-wrapped', code: `
const {shallowReactive, isReactive} = R;
const inner = {};
const s = shallowReactive(new Set([inner]));
const first = [...s.values()][0];
log('shallow-set', first === inner, isReactive(first), isReactive([...s][0]));
`},
  {id: 'r1b-readonly-raw-map-no-tracking', code: `
const {readonly, effect} = R;
const raw = new Map();
const r = readonly(raw);
let runs = 0;
effect(() => { runs++; log('ro-size', r.size); });
raw.set('a', 1);
log('runs', runs, r.size);
`},
  {id: 'r1b-collection-proto-and-tag', code: `
const {reactive, readonly, shallowReactive, isReactive} = R;
const p = reactive(new Set());
log('set-tag', p instanceof Set, Object.prototype.toString.call(p), Object.getPrototypeOf(p) === Set.prototype);
const q = readonly(new Map());
log('map-tag', q instanceof Map, Object.prototype.toString.call(q), isReactive(shallowReactive(new Map())));
`},
  {id: 'r1b-readonly-set-iteration-wraps', code: `
const {readonly, isReadonly} = R;
const r = readonly(new Set([{}, 1]));
const flags = [];
for (const v of r) flags.push(typeof v === 'object' ? isReadonly(v) : typeof v);
log('ro-iter', flags.join(','));
`},
  {id: 'r1b-arr-includes-from-index', code: `
const {reactive} = R;
const o = {};
const arr = reactive([o, 'x', o]);
log('from', arr.includes(o, 1), arr.indexOf(o, 1), arr.lastIndexOf(o, 1), arr.indexOf(o, -1));
log('missing', arr.includes(reactive(o), 2), arr.indexOf(reactive(o)), arr.lastIndexOf(reactive(o)));
`},
  {id: 'r1b-arr-concat-spreads-wrapped', code: `
const {reactive, isReactive} = R;
const a = reactive([{n: 1}]);
const b = a.concat([2], reactive([3]));
log('concat', b.length, Array.isArray(b), isReactive(b), isReactive(b[0]), b[1], b[2]);
`},
  {id: 'r1b-arr-join-and-tostring-wrapped', code: `
const {reactive} = R;
const a = reactive([1, [2, 3], 'x']);
log('join', a.join('-'), String(a), a.toString === Array.prototype.toString);
`},
  {id: 'r1b-arr-copies-tracking-iterate', code: `
const {reactive, effect} = R;
const a = reactive([3, 1, 2]);
let runs = 0;
effect(() => { runs++; log('sorted', a.toSorted().join(',')); });
log('rev', a.toReversed().join(','), a.join(','), runs);
log('spliced', a.toSpliced(1, 1, 'z').join(','), a.length, runs);
a.push(4);
log('after-push', runs);
`},
  {id: 'r1b-arr-find-family-wraps', code: `
const {reactive, isReactive} = R;
const o = {v: 1};
const a = reactive([o, {v: 2}]);
const f = a.find((x) => x.v === 2);
log('find', isReactive(f), f.v, a.findIndex((x) => x.v === 2), a.findLast((x) => x.v === 1) === a[0], a.findLastIndex((x) => x.v === 9));
`},
  {id: 'r1b-arr-filter-map-wrap-results', code: `
const {reactive, isReactive} = R;
const a = reactive([{k: 1}, {k: 2}, {k: 3}]);
const odd = a.filter((x) => x.k % 2 === 1);
log('filter', odd.length, isReactive(odd[0]), odd[1].k);
const ks = a.map((x) => isReactive(x) + ':' + x.k);
log('map', ks.join(' '));
`},
  {id: 'r1b-arr-some-every-callback-args', code: `
const {reactive} = R;
const a = reactive([1, 2, 3]);
const seen = [];
const r1 = a.some((x, i, arr) => { seen.push(i, arr === a); return x === 2; });
const r2 = a.every((x) => x > 0);
log('some', r1, r2, seen.join(','));
`},
  {id: 'r1b-arr-reduce-accumulator-wrap', code: `
const {reactive, isReactive} = R;
const a = reactive([{n: 1}, {n: 2}]);
const total = a.reduce((acc, x) => acc + x.n, 0);
const first = a.reduce((acc, x) => { acc.push(isReactive(x)); return acc; }, []);
const rr = a.reduceRight((acc, x) => acc + x.n, 10);
log('reduce', total, first.join(','), rr);
`},
  {id: 'r1b-arr-reduce-initial-accumulator-wrapped', code: `
const {reactive, isReactive} = R;
const a = reactive([{n: 1}, {n: 2}]);
const got = a.reduce((acc, x) => { log('step', isReactive(acc), isReactive(x)); return acc; });
log('result', isReactive(got), got.n);
`},
  {id: 'r1b-arr-reduce-empty-init-not-wrapped', code: `
const {reactive, isReactive} = R;
const o = {q: 1};
const empty = reactive([]);
log('empty-init', isReactive(empty.reduce((acc) => acc, o)));
let msg = 'none';
try { empty.reduce((acc) => acc); } catch (err) { msg = err.constructor.name; }
log('empty-noinit', msg);
`},
  {id: 'r1b-arr-readonly-push-silent', code: `
const {readonly} = R;
const raw = [1];
const r = readonly(raw);
const n = r.push(2);
log('ro-push', n, raw.length, r.length, r[1]);
`},
  {id: 'r1b-arr-readonly-pop-shift-silent', code: `
const {readonly} = R;
const raw = [5, 6];
const r = readonly(raw);
log('ro-pop', r.pop(), r.shift(), raw.length, r.length);
`},
  {id: 'r1b-arr-shallow-elements-not-wrapped', code: `
const {shallowReactive, isReactive, effect} = R;
const inner = {n: 1};
const a = shallowReactive([inner]);
log('shallow-el', a[0] === inner, isReactive(a[0]), Array.isArray(a));
let runs = 0;
effect(() => { runs++; log('first', a[0].n); });
a[0] = {n: 2};
log('runs', runs);
`},
  {id: 'r1b-arr-delete-index-hole-tracking', code: `
const {reactive, effect} = R;
const a = reactive([1, 2]);
let runs = 0;
effect(() => { runs++; log('has0', 0 in a, a[0]); });
delete a[0];
log('after-delete', runs, 0 in a, a.length);
a[0] = 9;
log('after-set', runs);
`},
  {id: 'r1b-arr-length-set-truncates-effects', code: `
const {reactive, effect} = R;
const a = reactive([1, 2, 3, 4]);
let runs = 0;
effect(() => { runs++; log('tail', a[3], a.length); });
a.length = 2;
log('after', runs, a.length, a[2]);
`},
  {id: 'r1b-arr-sparse-holes-iteration', code: `
const {reactive} = R;
const a = reactive([1, , 3]);
const seen = [];
a.forEach((x, i) => seen.push(i + ':' + x));
log('holes', seen.join(','), 1 in a, a.length);
`},
  {id: 'r1b-arr-index-string-keys', code: `
const {reactive, effect} = R;
const a = reactive(['a']);
let runs = 0;
effect(() => { runs++; log('k', a['0'], a['00']); });
a['0'] = 'b';
a['00'] = 'x';
log('runs', runs, a['00']);
`},
  {id: 'r1b-arr-ownkeys-tracks-length', code: `
const {reactive, effect} = R;
const a = reactive([1]);
let runs = 0;
effect(() => { runs++; log('keys', Object.keys(a).join(',')); });
a.push(2);
log('runs', runs);
a.length = 1;
log('runs2', runs);
`},
  {id: 'r1b-arr-unshift-push-return-and-tracking', code: `
const {reactive, effect} = R;
const a = reactive([]);
let runs = 0;
effect(() => { runs++; log('last', a[a.length - 1]); });
log('push-ret', a.push('x'), runs);
log('unshift-ret', a.unshift('w'), a.join(''), runs);
`},
  {id: 'r1b-arr-includes-tracks-after-mutation', code: `
const {reactive, effect} = R;
const a = reactive([1]);
let runs = 0;
effect(() => { runs++; log('has5', a.includes(5)); });
a.push(5);
log('runs', runs);
a[0] = 5;
log('runs2', runs);
`},
  {id: 'r1b-arr-nested-array-identity', code: `
const {reactive} = R;
const inner = [1];
const a = reactive([inner]);
log('nested-id', a[0] === a[0], a[0] === reactive(inner), reactive(a)[0] === a[0]);
`},
  {id: 'r1b-arr-index-nan-and-string-search', code: `
const {reactive} = R;
const a = reactive(['1', 1, NaN]);
log('idx', a.indexOf(1), a.indexOf('1'), a.includes(NaN), a.lastIndexOf(NaN));
`},
  {id: 'r1b-arr-spread-and-destructure', code: `
const {reactive, isReactive} = R;
const a = reactive([{x: 1}, 2]);
const [first, second] = a;
const copy = [...a];
log('destructure', isReactive(first), second, copy.length, isReactive(copy[0]));
`},
  {id: 'r1b-readonly-tracks-underlying', code: `
const {reactive, readonly, effect} = R;
const raw = reactive({n: 1});
const ro = readonly(raw);
let runs = 0;
effect(() => { runs++; log('ro-read', ro.n); });
raw.n = 2;
log('runs', runs, ro.n);
`},
  {id: 'r1b-readonly-of-raw-no-tracking', code: `
const {readonly, effect} = R;
const raw = {n: 1};
const ro = readonly(raw);
let runs = 0;
effect(() => { runs++; log('ro-read', ro.n); });
raw.n = 2;
log('runs', runs, ro.n);
`},
  {id: 'r1b-shallow-nested-not-tracked', code: `
const {shallowReactive, effect} = R;
const inner = {n: 1};
const s = shallowReactive({inner});
let runs = 0;
effect(() => { runs++; log('shallow-n', s.inner.n); });
s.inner.n = 2;
log('runs', runs);
s.inner = {n: 3};
log('runs2', runs);
`},
  {id: 'r1b-shallow-readonly-root-only', code: `
const {shallowReadonly, isReadonly, isReactive} = R;
const raw = {inner: {n: 1}};
const sr = shallowReadonly(raw);
log('sro-root', isReadonly(sr.inner), isReactive(sr.inner), sr.inner === raw.inner);
sr.inner.n = 5;
log('sro-write', raw.inner.n);
`},
  {id: 'r1b-readonly-vs-shallow-on-refs', code: `
const {readonly, shallowReadonly, ref, isRef} = R;
const r = ref(3);
const d = readonly({r});
const s = shallowReadonly({r});
log('ro-unwrap', d.r, isRef(d.r), s.r === r, isRef(s.r));
`},
  {id: 'r1b-is-flags-on-primitives', code: `
const {isReactive, isReadonly, isShallow, isProxy, isRef, toRaw} = R;
log('prims', isReactive(0), isReadonly(''), isShallow(null), isProxy(undefined), isRef(false), toRaw(null), toRaw(undefined), toRaw(7));
`},
  {id: 'r1b-proto-chain-get-and-flags', code: `
const {reactive, isReactive} = R;
const base = {b: 1};
const child = Object.create(base);
const p = reactive(child);
log('proto-get', p.b, isReactive(p.__proto__), isReactive(Object.getPrototypeOf(p)));
`},
  {id: 'r1b-set-through-prototype-chain', code: `
const {reactive, effect} = R;
const base = {b: 1};
const child = reactive(Object.create(base));
let runs = 0;
effect(() => { runs++; log('b', child.b); });
child.b = 2;
log('runs', runs, base.b, Object.keys(child).length);
`},
  {id: 'r1b-define-property-no-trigger', code: `
const {reactive, effect} = R;
const p = reactive({a: 1});
let runs = 0;
effect(() => { runs++; log('a', p.a); });
Object.defineProperty(p, 'a', {value: 9, writable: true, enumerable: true, configurable: true});
log('runs', runs, p.a);
Object.defineProperty(p, 'b', {value: 2, writable: true, enumerable: true, configurable: true});
log('runs2', runs, p.b);
`},
  {id: 'r1b-gopd-returns-raw-value', code: `
const {reactive} = R;
const inner = {n: 1};
const p = reactive({inner});
const d = Object.getOwnPropertyDescriptor(p, 'inner');
log('gopd', d.value === inner, Object.getOwnPropertyDescriptor(p, 'inner').enumerable, d.writable, Object.keys(p).join(','));
`},
  {id: 'r1b-object-entries-values-wrapped', code: `
const {reactive, isReactive} = R;
const p = reactive({a: {x: 1}, b: 2});
const entries = Object.entries(p);
log('entries', entries.length, isReactive(entries[0][1]), entries[1][1]);
const vals = Object.values(p);
log('values', isReactive(vals[0]), vals[1]);
const assigned = Object.assign({}, p);
log('assign', isReactive(assigned.a), assigned.b, Object.getPrototypeOf(assigned) === Object.prototype);
`},
  {id: 'r1b-in-operator-inherited-tracking', code: `
const {reactive, effect} = R;
const base = {inherited: true};
const p = reactive(Object.create(base));
let runs = 0;
effect(() => { runs++; log('in', 'inherited' in p, p.inherited); });
p.inherited = false;
log('runs', runs, base.inherited);
`},
  {id: 'r1b-proto-of-proxy-is-raw', code: `
const {reactive, isReactive} = R;
const base = {x: 1};
const p = reactive(Object.create(base));
log('proto', Object.getPrototypeOf(p) === base, isReactive(Object.getPrototypeOf(p)));
`},
  {id: 'r1b-cycles-through-proxies', code: `
const {reactive, toRaw, effect} = R;
const node = {name: 'n'};
node.self = node;
const p = reactive(node);
let runs = 0;
effect(() => { runs++; log('name', p.self.self.name); });
p.self.name = 'm';
log('runs', runs, toRaw(p.self) === node, p.self === p.self.self, p.self.self === p);
`},
  {id: 'r1b-frozen-nested-not-proxied', code: `
const {reactive, isReactive} = R;
const frozenInner = Object.freeze({v: 1});
const p = reactive({inner: frozenInner, plain: {v: 2}});
log('frozen-inner', p.inner === frozenInner, isReactive(p.inner), isReactive(p.plain));
`},
  {id: 'r1b-freeze-readonly-proxy-freezes-target', code: `
const {readonly, isReadonly} = R;
const raw = {a: 1};
const r = readonly(raw);
log('ro-after-freeze', isReadonly(Object.freeze(r)), Object.isFrozen(raw), r.a);
`},
  {id: 'r1b-reactive-of-readonly-unchanged', code: `
const {reactive, readonly, isReactive, isReadonly} = R;
const r = readonly({a: 1});
const again = reactive(r);
log('rr', again === r, isReactive(again), isReadonly(again));
`},
  {id: 'r1b-readonly-of-shallow-reactive', code: `
const {readonly, shallowReactive, isReadonly, isReactive} = R;
const s = shallowReactive({inner: {n: 1}});
const r = readonly(s);
log('ro-of-shallow', isReadonly(r.inner), isReactive(r.inner));
`},
  {id: 'r1b-markraw-under-readonly', code: `
const {readonly, markRaw, isReadonly} = R;
const r = readonly({m: markRaw({k: 1})});
log('mark-ro', isReadonly(r.m), r.m.k);
`},
  {id: 'r1b-tostringtag-object-proxied-as-collection', code: `
const {reactive, isReactive} = R;
const tagged = {[Symbol.toStringTag]: 'Map', x: 1};
const p = reactive(tagged);
log('tag', isReactive(p), Object.prototype.toString.call(p), p.x);
`},
  {id: 'r1b-array-like-object-not-array', code: `
const {reactive} = R;
const p = reactive({length: 0, 0: 'a'});
log('arraylike', Array.isArray(p), p.length, Object.prototype.toString.call(p));
`},
  {id: 'r1b-numeric-and-string-keys-same', code: `
const {reactive, effect} = R;
const p = reactive({});
let runs = 0;
effect(() => { runs++; log('k', p[1]); });
p['1'] = 'a';
log('runs', runs, p[1]);
`},
  {id: 'r1b-ref-value-is-reactive-object', code: `
const {ref, isReactive, toRaw} = R;
const obj = {n: 1};
const r = ref(obj);
log('ref-obj', isReactive(r.value), toRaw(r.value) === obj, r.value === r.value);
`},
  {id: 'r1b-shallowref-value-raw', code: `
const {shallowRef, isReactive} = R;
const obj = {n: 1};
const r = shallowRef(obj);
log('shallow-ref', r.value === obj, isReactive(r.value));
`},
  {id: 'r1b-ref-assign-same-raw-no-trigger', code: `
const {ref, reactive, effect} = R;
const obj = {n: 1};
const r = ref(obj);
let runs = 0;
effect(() => { runs++; log('n', r.value.n); });
r.value = reactive(obj);
log('runs-same', runs);
r.value = {n: 2};
log('runs-new', runs);
`},
  {id: 'r1b-reactive-unwraps-nested-ref-writes', code: `
const {reactive, ref, effect} = R;
const count = ref(1);
const p = reactive({nested: {count}});
let runs = 0;
effect(() => { runs++; log('count', p.nested.count); });
count.value = 2;
log('runs', runs);
p.nested.count = 3;
log('runs2', runs, count.value);
`},
  {id: 'r1b-ref-in-array-not-unwrapped', code: `
const {reactive, ref, isRef} = R;
const r = ref(7);
const arr = reactive([r]);
log('arr-ref', isRef(arr[0]), arr[0] === r, arr[0].value);
arr[0] = 8;
log('arr-set', isRef(arr[0]), arr[0]);
`},
  {id: 'r1b-ref-in-map-value-wrapped', code: `
const {reactive, ref, isRef} = R;
const r = ref('v');
const m = reactive(new Map([['k', r]]));
log('map-ref', isRef(m.get('k')), m.get('k') === r);
`},
  {id: 'r1b-ref-of-proxy-reads-through', code: `
const {ref, reactive, isReactive} = R;
const p = reactive({n: 1});
const r = ref(p);
log('ref-of-proxy', r.value === p, isReactive(r.value));
`},
  {id: 'r1b-torefs-over-reactive', code: `
const {reactive, toRefs, effect} = R;
const p = reactive({a: 1, b: 2});
const {a, b} = toRefs(p);
let runs = 0;
effect(() => { runs++; log('a', a.value); });
p.a = 5;
log('runs', runs, b.value);
a.value = 7;
log('runs2', runs, p.a);
`},
  {id: 'r1b-toref-over-readonly', code: `
const {readonly, toRef} = R;
const r = readonly({n: 1});
const tr = toRef(r, 'n');
log('toref-ro', tr.value);
`},
  {id: 'r1b-ref-deep-object-value-tracking', code: `
const {ref, effect} = R;
const r = ref({n: 1});
let runs = 0;
effect(() => { runs++; log('deep', r.value.n); });
r.value.n = 2;
log('runs', runs);
r.value = {n: 3};
log('runs2', runs);
`},
  {id: 'r1b-unref-toValue-on-reactive', code: `
const {reactive, ref, unref, toValue, isRef} = R;
const p = reactive({r: ref(4)});
log('unref', unref(p.r), toValue(p.r), isRef(p.r), unref(p));
`},
  {id: 'r1b-ref-deep-array-push-tracking', code: `
const {ref, isReactive, effect} = R;
const r = ref([1, 2]);
let runs = 0;
effect(() => { runs++; log('len', r.value.length); });
r.value.push(3);
log('runs', runs, isReactive(r.value));
`},
  {id: 'r1b-ref-of-readonly-value', code: `
const {ref, readonly, isReadonly, isReactive} = R;
const r = ref(readonly({n: 1}));
log('ref-ro', isReadonly(r.value), isReactive(r.value));
`},
  {id: 'r1b-gc-cross-references', code: `
const {reactive, effect, toRaw} = R;
const a = {name: 'a'};
const b = {name: 'b', peer: a};
a.peer = b;
const pa = reactive(a);
let runs = 0;
effect(() => { runs++; log('peer-name', pa.peer.peer.name); });
pa.peer.name = 'B';
log('after', runs, toRaw(pa.peer) === b, pa.peer.name);
`},
  {id: 'r1b-gc-proxy-identity-across-collections', code: `
const {reactive, toRaw} = R;
const raw = {tag: 1};
const holder = reactive(new Map([['k', raw]]));
const first = holder.get('k');
const list = reactive([raw]);
log('identity', first === list[0], toRaw(first) === raw, reactive(raw) === first);
`},
  {id: 'r1b-effect-stop-stops-proxy-tracking', code: `
const {reactive, effect, stop} = R;
const p = reactive({n: 1});
let runs = 0;
const runner = effect(() => { runs++; log('n', p.n); });
stop(runner);
p.n = 2;
log('runs', runs, p.n);
`},
  {id: 'r1b-computed-over-proxy-fields', code: `
const {reactive, computed, effect} = R;
const p = reactive({a: 1, b: 2});
const sum = computed(() => p.a + p.b);
let runs = 0;
effect(() => { runs++; log('sum', sum.value); });
p.a = 10;
log('runs', runs, sum.value);
`},
  {id: 'r1b-computed-cache-unrelated-key', code: `
const {reactive, computed} = R;
const p = reactive({a: 1, b: 2});
let evals = 0;
const c = computed(() => { evals++; return p.a * 2; });
log('first', c.value, evals);
p.b = 5;
log('after-b', c.value, evals);
p.a = 3;
log('after-a', c.value, evals);
`},
  {id: 'r1b-effect-scope-disposes-proxy-effect', code: `
const {reactive, effect, effectScope} = R;
const p = reactive({n: 1});
let runs = 0;
const scope = effectScope();
scope.run(() => effect(() => { runs++; log('n', p.n); }));
p.n = 2;
scope.stop();
p.n = 3;
log('runs', runs);
`},
  {id: 'r1b-arr-computed-length', code: `
const {reactive, computed} = R;
const a = reactive([1]);
const n = computed(() => a.length * 10);
log('c1', n.value);
a.push(2);
log('c2', n.value);
`},
  {id: 'r1b-arr-sort-proxy-elements', code: `
const {reactive, isReactive} = R;
const a = reactive([{v: 2}, {v: 1}]);
a.sort((x, y) => x.v - y.v);
log('sorted', a.map((x) => x.v).join(','), isReactive(a[0]));
`},
  {id: 'r1b-arr-keys-tracking', code: `
const {reactive, effect} = R;
const a = reactive(['x']);
let runs = 0;
effect(() => { runs++; log('keys', [...a.keys()].join(',')); });
a.push('y');
log('runs', runs);
`},
  {id: 'r1b-arr-fill-triggers-per-write', code: `
const {reactive, effect} = R;
const a = reactive([1, 2]);
let runs = 0;
effect(() => { runs++; log('sum', a[0] + a[1]); });
a.fill(0);
log('runs', runs, a[0] + a[1]);
`},
  {id: 'r1b-map-has-tracks-object-key', code: `
const {reactive, effect} = R;
const k = {};
const m = reactive(new Map());
let runs = 0;
effect(() => { runs++; log('has', m.has(k)); });
m.set(k, 1);
log('runs', runs);
m.delete(k);
log('runs2', runs);
`},
  {id: 'r1b-map-nested-map-values', code: `
const {reactive, effect, isReactive} = R;
const inner = new Map([['x', 1]]);
const m = reactive(new Map([['inner', inner]]));
let runs = 0;
effect(() => { runs++; log('x', m.get('inner').get('x')); });
inner.set('x', 2);
log('runs', runs, isReactive(m.get('inner')));
m.get('inner').set('x', 3);
log('runs2', runs, inner.get('x'));
`},
  {id: 'r1b-set-of-arrays-identity', code: `
const {reactive, isReactive} = R;
const s = reactive(new Set());
const arr = [1];
s.add(arr);
const got = [...s][0];
log('set-arr', isReactive(got), got === reactive(arr));
`},
  {id: 'r1b-readonly-array-iteration-wrapped', code: `
const {readonly, isReadonly} = R;
const r = readonly([{v: 1}]);
const out = [];
for (const x of r) out.push(isReadonly(x));
log('ro-arr-iter', out.join(','), r.includes(r[0]), r.indexOf(r[0]));
`},
  {id: 'r1b-shallow-array-push-tracks', code: `
const {shallowReactive, effect} = R;
const a = shallowReactive([1]);
let runs = 0;
effect(() => { runs++; log('len', a.length); });
a.push(2);
log('runs', runs, a.length);
`},
  {id: 'r1b-shallowref-in-reactive-not-rewrapped', code: `
const {reactive, shallowRef, isRef} = R;
const sr = shallowRef({n: 1});
const p = reactive({sr});
log('sr-unwrapped', isRef(p.sr), p.sr.n, p.sr === sr.value);
`},
  {id: 'r1b-map-entries-tracking-add-in-effect', code: `
const {reactive, effect} = R;
const m = reactive(new Map());
let runs = 0;
effect(() => { runs++; let n = 0; for (const e of m.entries()) n++; log('entries', n); });
m.set('a', 1);
m.set('b', 2);
log('runs', runs);
`},
  {id: 'r1b-set-clear-triggers-size', code: `
const {reactive, effect} = R;
const s = reactive(new Set([1, 2]));
let runs = 0;
effect(() => { runs++; log('size', s.size); });
s.clear();
log('runs', runs, s.size);
`},
  {id: 'r1b-array-of-maps-proxied', code: `
const {reactive, isReactive} = R;
const a = reactive([new Map()]);
log('map-in-arr', isReactive(a[0]), a[0].size, Object.prototype.toString.call(a[0]));
`},
  {id: 'r1b-object-with-map-field-size', code: `
const {reactive, effect} = R;
const p = reactive({m: new Map()});
let runs = 0;
effect(() => { runs++; log('size', p.m.size); });
p.m.set('k', 1);
log('runs', runs);
`},
  {id: 'r1b-proxy-function-property-not-proxied', code: `
const {reactive, isReactive} = R;
const fn = function () {};
const p = reactive({fn});
log('fn-prop', p.fn === fn, isReactive(p.fn), typeof p.fn);
`},
  {id: 'r1b-method-this-is-proxy', code: `
const {reactive, effect} = R;
const p = reactive({n: 1, self() { return this; }});
log('method-this', p.self() === p);
let runs = 0;
effect(() => { runs++; log('via-method', p.self().n); });
p.n = 2;
log('runs', runs);
`},
  {id: 'r1b-keys-order-after-delete-readd', code: `
const {reactive} = R;
const p = reactive({a: 1, b: 2});
delete p.a;
p.a = 3;
log('keys', Object.keys(p).join(','));
`},
  {id: 'r1b-json-stringify-proxy', code: `
const {reactive} = R;
const p = reactive({a: 1, b: [2, 3], c: {d: 'x'}});
log('json', JSON.stringify(p));
`},
  {id: 'r1b-arr-length-after-index-add', code: `
const {reactive, effect} = R;
const a = reactive([]);
let runs = 0;
effect(() => { runs++; log('len', a.length); });
a[3] = 1;
log('runs', runs, a.length);
`},
  {id: 'r1b-readonly-keys-and-shallow-readonly-set', code: `
const {readonly, shallowReadonly} = R;
const r = readonly({a: 1, b: 2});
const sr = shallowReadonly({a: 1});
sr.a = 2;
log('ro-keys', Object.keys(r).join(','), sr.a);
`},
  {id: 'r1b-class-instance-getter-receiver', code: `
const {reactive, effect} = R;
class Pt { constructor() { this.x = 1; } get dbl() { return this.x * 2; } }
const p = reactive(new Pt());
let runs = 0;
effect(() => { runs++; log('dbl', p.dbl); });
p.x = 5;
log('runs', runs, p.dbl);
`},
  {id: 'r1b-two-proxies-independent-tracking', code: `
const {reactive, effect} = R;
const a = reactive({v: 1});
const b = reactive({v: 1});
let runs = 0;
effect(() => { runs++; log('sum', a.v + b.v); });
b.v = 5;
log('runs', runs);
a.v = 2;
log('runs2', runs);
`},
  {id: 'r1b-set-delete-during-foreach', code: `
const {reactive} = R;
const s = reactive(new Set([1, 2, 3]));
const seen = [];
s.forEach((v) => { seen.push(v); if (v === 1) s.delete(2); });
log('seen', seen.join(','), s.size);
`},
  {id: 'r1b-map-foreach-add-during-visits-new', code: `
const {reactive} = R;
const m = reactive(new Map([['a', 1]]));
const seen = [];
m.forEach((v, k) => { seen.push(k); if (k === 'a') m.set('b', 2); });
log('seen', seen.join(','), m.size);
`},
  {id: 'r1b-arr-foreach-push-visits-original', code: `
const {reactive} = R;
const a = reactive([1, 2]);
let visits = 0;
a.forEach(() => { visits++; a.push(0); });
log('visits', visits, a.length);
`},
  {id: 'r1b-nested-reactive-arrays-deep-tracking', code: `
const {reactive, effect} = R;
const p = reactive({list: [[1, 2]]});
let runs = 0;
effect(() => { runs++; log('cell', p.list[0][1]); });
p.list[0].push(9);
log('runs', runs);
p.list[0][1] = 7;
log('runs2', runs);
`},
  {id: 'r1b-null-prototype-object-proxied', code: `
const {reactive, isReactive} = R;
const p = reactive(Object.create(null));
p.x = 1;
log('null-proto', isReactive(p), p.x, Object.getPrototypeOf(p) === null);
`},
  {id: 'r1b-arr-holes-for-of-undefined', code: `
const {reactive} = R;
const out = [];
for (const x of reactive([1, , 3])) out.push(String(x));
log('holes-iter', out.join(','));
`},
  {id: 'r1b-arr-at-tracks-length', code: `
const {reactive, effect} = R;
const a = reactive([1]);
let runs = 0;
effect(() => { runs++; log('last', a.at(-1)); });
a.push(7);
log('runs', runs, a.at(-1));
`},
  {id: 'r1b-push-in-effect-untracked', code: `
const {reactive, effect} = R;
const a = reactive([]);
let runs = 0;
effect(() => { runs++; a.push(runs); });
log('runs', runs, a.length);
a.push(9);
log('runs2', runs, a.length);
`},
];

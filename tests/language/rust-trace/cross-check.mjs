// Cross-check fixture generator (WP-T1). It builds each graph in JavaScript, records it with the Node oracle
// (tooling/oracle/trace.mjs) and writes one JSON line per graph: {"name":<name>,"entries":<entries>}.
// crates/testkit/tests/cross_check.rs builds the same graphs natively and must produce the same entries text.
//   node tests/language/rust-trace/cross-check.mjs           regenerate cross-check.jsonl
//   node tests/language/rust-trace/cross-check.mjs --check   exit 1 when the committed file is stale
import {readFileSync, writeFileSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createRecorder} from '../../../tooling/oracle/trace.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const fixture = path.join(here, 'cross-check.jsonl');

// Each graph gets a fresh recorder and calls `t(label, ...values)` in the same order as the Rust side.
const graphs = [
  ['primitives', (t) => t('p', undefined, null, true, false, 0, 1, -5, 0.1, 1e21, 1e-7, 5e-324,
    2 ** 53, 1.5e300, 123456789.125, -0, NaN, Infinity, -Infinity, 'str', '')],
  ['string-escapes', (t) => t('s', 'a"b\\c', '\b\f\n\r\t', '\u0000\u0001\u001f', '\u007fé ',
    '\u{1F600}', '\ud83d', '\ude00', 'x\ud83dy')],
  ['symbols', (t) => {
    const a = Symbol('a');
    const e = Symbol('');
    const n = Symbol();
    t('y', a, e, a, n, Symbol.iterator, Symbol.for('k'), Symbol.for('k'));
  }],
  ['functions', (t) => {
    function named(a, b, c) {}
    const anon = () => {};
    t('f', named, anon, named, {m: named}, [function () {}]);
  }],
  ['function-name-redefined', (t) => {
    function f() {}
    Object.defineProperty(f, 'name', {value: 'renamed'});
    t('fn2', f);
  }],
  ['plain-key-order', (t) => {
    const o = {b: 1, a: 'x', 2: 'two', 1: 'one', '-1': 'neg', '01': 'lead', 4294967294: 'maxidx', 4294967295: 'notidx'};
    o[Symbol('hidden')] = 1;
    t('o', o);
  }],
  ['integer-key-order', (t) => t('k', {10: 'a', 9: 'b', 1: 'c', x: 1, 0: 'z'})],
  ['key-escaping', (t) => t('ke', {'a"b': 1, '\\': 2, '\ud800': 3, 'é': 4})],
  ['accessors-not-invoked', (t) => {
    const o = {
      get x() { throw new Error('getter ran'); },
      set y(v) {},
      get z() { return 1; },
      set z(v) {},
    };
    Object.defineProperty(o, 'hidden', {value: 1, enumerable: false});
    t('acc', o);
  }],
  ['arrays-holes', (t) => {
    const a = [1, , 'x'];
    a.tag = 'T';
    const b = [];
    b[3] = true;
    t('arr', a, b, []);
  }],
  ['array-length-and-extra', (t) => {
    const a = [7, 8, 9];
    a.length = 1;
    a['-1'] = 'neg';
    a.x = 'y';
    a[2] = 'two';
    t('al', a);
  }],
  ['aliases-one-call', (t) => {
    const o = {};
    t('al', [o, o], {x: o, y: [o]}, o);
  }],
  ['cycles', (t) => {
    const o = {};
    o.self = o;
    o.arr = [o];
    t('cy', o);
  }],
  ['shared-across-calls', (t) => {
    const o = {n: 1};
    t('s1', o);
    o.n = 2;
    o.extra = [o];
    t('s2', o);
  }],
  ['mutation-between-calls', (t) => {
    const o = {v: 1, w: 2};
    t('m1', o);
    o.v = 'changed';
    delete o.w;
    o.z = null;
    t('m2', o);
  }],
  ['ordinals-across-calls', (t) => {
    const a = {};
    const b = {};
    t('one', a);
    t('two', b, a);
    const c = [b];
    t('three', c, a);
  }],
  ['proto-kinds', (t) => {
    const inh = {inh: 1};
    t('pk', Object.create(null), Object.create(inh), Object.create(Array.prototype),
      Object.create(Object.prototype), {length: 2});
  }],
  ['errors', (t) => {
    const e1 = new TypeError('bad');
    const e2 = new Error();
    e2.code = 42;
    e2.name = 'Custom';
    const e3 = Object.create(Error.prototype);
    t('err', e1, e2, e3, new RangeError('r'));
  }],
  ['frozen', (t) => t('fz', Object.freeze({a: 1}))],
  ['sealed', (t) => t('sl', Object.seal({a: 1}))],
  ['nonextensible', (t) => t('ne', Object.preventExtensions({a: 1}))],
  ['empty-nonextensible-is-frozen', (t) => t('ep', Object.preventExtensions({}))],
  ['extensible-has-no-integrity', (t) => t('ex', {a: 1})],
  ['frozen-array', (t) => t('fa', Object.freeze([1, , 3]))],
  ['sealed-array', (t) => t('sa', Object.seal([1, 2]))],
  ['nonextensible-array', (t) => t('na', Object.preventExtensions([1]))],
  ['nonextensible-empty-array', (t) => t('nea', Object.preventExtensions([]))],
  ['empty-frozen-array', (t) => t('efa', Object.freeze([]))],
  ['integrity-nested', (t) => t('in', Object.freeze({inner: {x: 1}, list: Object.seal([])}))],
  ['frozen-error', (t) => t('fe', Object.freeze(new Error('e')))],
  ['numbers-in-arrays', (t) => t('n', [NaN, -0, 1e21, 0.000001])],
  ['untraceable-after-partial', (t) => {
    try {
      t('bad', {a: 1}, new Date(0));
    } catch (e) {
      t('caught', e.message);
    }
    t('after', {});
  }],
  ['label-must-be-string', (t) => {
    try {
      t(1, 2);
    } catch (e) {
      t('caught', e.message);
    }
    t('ok', 'x');
  }],
  ['deep-nesting', (t) => {
    const root = {};
    let cur = root;
    for (let i = 0; i < 6; i++) {
      cur.next = [{i}];
      cur = cur.next[0];
    }
    t('deep', root);
  }],
  ['symbol-keys-and-values', (t) => {
    const s = Symbol('v');
    t('sv', {[Symbol.iterator]: 1, a: s, b: s});
  }],
  ['many-values-one-call', (t) => {
    const xs = [];
    for (let i = 0; i < 5; i++) xs.push({i});
    t('many', ...xs, xs);
  }],
  ['map-entries-in-order', (t) => {
    const m = new Map([[1, 'a'], ['k', [2]], [NaN, undefined]]);
    t('m', m);
  }],
  ['map-zero-and-identity-keys', (t) => {
    const o = {};
    const m = new Map();
    m.set(-0, 'zero');
    m.set(o, 1);
    m.set(o, 2);
    t('mz', m, o);
  }],
  ['set-values-in-order', (t) => {
    const s = new Set([3, 'a', NaN, -0, NaN]);
    s.add(3);
    t('s', s);
  }],
  ['collections-of-themselves', (t) => {
    const m = new Map();
    m.set('self', m);
    const s = new Set();
    s.add(s);
    t('cm', m, s);
  }],
  ['map-after-delete-and-readd', (t) => {
    const m = new Map([[1, 'a'], [2, 'b']]);
    m.delete(1);
    m.set(1, 'c');
    t('md', m);
  }],
  ['collection-iterators-are-plain-objects', (t) => {
    t('it', new Map([[1, 2]]).keys(), new Set([3]).values());
  }],
  ['set-with-own-property', (t) => {
    const s = new Set([1]);
    s.extra = 'x';
    t('sp', s);
  }],
  ['promise-pending', (t) => {
    t('p', new Promise(() => {}));
  }],
  ['promise-fulfilled-and-handled-rejection', (t) => {
    const f = Promise.resolve(5);
    const r = Promise.reject(1);
    r.catch(() => {});
    t('pr', f, r);
  }],
  ['promise-aliased-in-array', (t) => {
    const p = new Promise(() => {});
    t('pa', [p, p]);
  }],
  ['promise-own-props-are-not-traced', (t) => {
    const p = new Promise(() => {});
    p.x = 1;
    t('px', p);
  }],
  ['promise-nonextensible', (t) => {
    const p = new Promise(() => {});
    p.x = 1;
    Object.preventExtensions(p);
    t('pn', p);
  }],
  ['promise-frozen', (t) => {
    const p = new Promise(() => {});
    Object.freeze(p);
    t('pf', p);
  }],
];

function render() {
  const names = new Set();
  return graphs.map(([name, build]) => {
    if (names.has(name)) throw new Error(`duplicate graph ${name}`);
    names.add(name);
    const rec = createRecorder();
    build(rec.trace);
    return `${JSON.stringify({name, entries: rec.entries})}\n`;
  }).join('');
}

const text = render();
if (process.argv.includes('--check')) {
  let committed = null;
  try {
    committed = readFileSync(fixture, 'utf8');
  } catch {
    // a missing fixture is stale too
  }
  if (committed !== text) {
    process.stderr.write('cross-check.jsonl is stale: run node tests/language/rust-trace/cross-check.mjs\n');
    process.exitCode = 1;
  } else {
    process.stdout.write(`cross-check.jsonl is current (${graphs.length} graphs)\n`);
  }
} else {
  writeFileSync(fixture, text);
  process.stdout.write(`wrote ${graphs.length} graphs to ${path.relative(process.cwd(), fixture)}\n`);
}

// WP-O1 tests: trace encoder, comparison, scenario contract, goldens and lint.
// Every expectation below is written by hand from trace-format.md or from the scenario source, not copied from output.
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtemp, readFile, rm, writeFile} from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createRecorder} from '../../tooling/oracle/trace.mjs';
import {compareDocs} from '../../tooling/oracle/compare.mjs';
import {
  casesRoot,
  runScenario,
  runScenarioLine,
  scenarioName,
} from '../../tooling/oracle/node-runner.mjs';
import {checkAll, compareWithGolden, goldenPathFor, listCases} from '../../tooling/oracle/check-goldens.mjs';
import {checkHeader, lintFile, lintSource} from '../../tooling/oracle/lint-cases.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const goldenRoot = path.resolve(here, 'golden');
const runCli = path.resolve(here, '../../tooling/oracle/run.mjs');
const caseFile = (rel) => path.join(casesRoot, rel);
const relOf = (file) => path.relative(casesRoot, file).split(path.sep).join('/');
const enc = (v) => createRecorder().encode(v);
const arrayOf = (elems, id) => ({$: 'obj', id, cls: 'Array', proto: 'other', props: [], elems, len: elems.length});
const errorOf = (id, name, message) => ({$: 'obj', id, cls: 'Error', proto: 'other', props: [], name, message});

// ---- trace encoder: one test per row of the encoding table ----

test('undefined encodes as {"$":"undef"}', () => {
  assert.deepEqual(enc(undefined), {$: 'undef'});
});

test('null and booleans encode as JSON literals', () => {
  assert.equal(enc(null), null);
  assert.equal(enc(true), true);
  assert.equal(enc(false), false);
});

test('finite numbers encode as JSON numbers printed like String(n)', () => {
  assert.equal(enc(1), 1);
  assert.equal(enc(0.1), 0.1);
  assert.equal(enc(-5), -5);
  assert.equal(JSON.stringify(enc(1e21)), '1e+21');
  assert.equal(JSON.stringify(enc(1e-7)), '1e-7');
});

test('negative zero encodes as {"$":"num","v":"-0"}; positive zero stays a number', () => {
  assert.deepEqual(enc(-0), {$: 'num', v: '-0'});
  assert.equal(JSON.stringify(enc(0)), '0');
});

test('NaN and infinities encode as {"$":"num"} markers', () => {
  assert.deepEqual(enc(NaN), {$: 'num', v: 'NaN'});
  assert.deepEqual(enc(Infinity), {$: 'num', v: 'Infinity'});
  assert.deepEqual(enc(-Infinity), {$: 'num', v: '-Infinity'});
});

test('strings are JSON strings; lone surrogates are escaped in lowercase like JSON.stringify', () => {
  assert.equal(enc('plain'), 'plain');
  assert.equal(JSON.stringify(enc('\uD800')), '"\\ud800"');
  assert.equal(JSON.stringify(enc('\uDE00x')), '"\\ude00x"');
  assert.equal(JSON.stringify(enc('😀')), '"😀"');
});

test('symbols get per-run ordinals and their description, or null when there is none', () => {
  const rec = createRecorder();
  const a = Symbol('a');
  const b = Symbol();
  assert.deepEqual(rec.encode(a), {$: 'sym', id: 1, desc: 'a'});
  assert.deepEqual(rec.encode(b), {$: 'sym', id: 2, desc: null});
  assert.deepEqual(rec.encode(a), {$: 'sym', id: 1, desc: 'a'});
});

test('functions encode as {"$":"fn",name,length} and get no object id', () => {
  function named(x, y) {
    return x + y;
  }
  const anon = () => 1;
  assert.deepEqual(enc(named), {$: 'fn', name: 'named', length: 2});
  assert.deepEqual(enc(anon), {$: 'fn', name: 'anon', length: 0});
});

test('plain objects: ids from 1, proto marker, own enumerable props in integer-then-insertion order', () => {
  const o = {b: 1, 2: 'two', a: 2, 1: 'one'};
  assert.deepEqual(enc(o), {
    $: 'obj',
    id: 1,
    cls: 'Object',
    proto: 'Object.prototype',
    props: [
      ['1', 'one'],
      ['2', 'two'],
      ['b', 1],
      ['a', 2],
    ],
  });
});

test('null-prototype objects report proto "null"', () => {
  const o = Object.create(null);
  o.k = 1;
  assert.deepEqual(enc(o), {$: 'obj', id: 1, cls: 'Object', proto: 'null', props: [['k', 1]]});
});

test('arrays: elems with holes, len, and non-index own keys in props', () => {
  const a = [1, , 3];
  a.extra = 'e';
  assert.deepEqual(enc(a), {
    $: 'obj',
    id: 1,
    cls: 'Array',
    proto: 'other',
    props: [['extra', 'e']],
    elems: [1, {$: 'hole'}, 3],
    len: 3,
  });
});

test('Map encodes entries in insertion order and Set encodes values', () => {
  assert.deepEqual(enc(new Map([['k', 1], [2, 'v']])), {
    $: 'obj',
    id: 1,
    cls: 'Map',
    proto: 'other',
    props: [],
    entries: [['k', 1], [2, 'v']],
  });
  assert.deepEqual(enc(new Set([1, 'x'])), {$: 'obj', id: 1, cls: 'Set', proto: 'other', props: [], values: [1, 'x']});
});

test('Error encodes name and message only; the stack is never traced', () => {
  assert.deepEqual(enc(new TypeError('bad')), errorOf(1, 'TypeError', 'bad'));
});

test('Promise encodes as an opaque object with no props', () => {
  assert.deepEqual(enc(Promise.resolve(1)), {$: 'obj', id: 1, cls: 'Promise', proto: 'other', props: []});
});

test('accessor properties are markers and getters are never invoked', () => {
  let calls = 0;
  const o = {
    get x() {
      calls += 1;
      return 1;
    },
    set x(v) {
      calls += v;
    },
    get y() {
      return 2;
    },
  };
  const rec = createRecorder();
  rec.trace('t', o);
  assert.deepEqual(rec.entries[0].values[0].props, [
    ['x', {$: 'accessor', get: true, set: true}],
    ['y', {$: 'accessor', get: true, set: false}],
  ]);
  assert.equal(calls, 0);
});

test('aliases inside one trace call: first occurrence expanded, later ones are refs with the same id', () => {
  const rec = createRecorder();
  const o = {n: 1};
  rec.trace('t', o, [o], o);
  assert.deepEqual(rec.entries[0].values, [
    {$: 'obj', id: 1, cls: 'Object', proto: 'Object.prototype', props: [['n', 1]]},
    {$: 'obj', id: 2, cls: 'Array', proto: 'other', props: [], elems: [{$: 'ref', id: 1}], len: 1},
    {$: 'ref', id: 1},
  ]);
});

test('cycles are visible as refs to the ancestor id', () => {
  const rec = createRecorder();
  const o = {};
  o.self = o;
  rec.trace('t', o);
  assert.deepEqual(rec.entries[0].values[0].props, [['self', {$: 'ref', id: 1}]]);
});

test('ordinals are global across trace calls; a later call expands the object again with its current state', () => {
  const rec = createRecorder();
  const o = {n: 1};
  rec.trace('first', o);
  o.n = 2;
  rec.trace('second', o);
  assert.deepEqual(rec.entries[0].values, [{$: 'obj', id: 1, cls: 'Object', proto: 'Object.prototype', props: [['n', 1]]}]);
  assert.deepEqual(rec.entries[1].values, [{$: 'obj', id: 1, cls: 'Object', proto: 'Object.prototype', props: [['n', 2]]}]);
});

test('Date, RegExp, typed arrays, WeakMap, WeakSet and BigInt are untraceable: trace throws TypeError', () => {
  for (const v of [new Date(0), /x/, new Uint8Array(1), new WeakMap(), new WeakSet(), 1n]) {
    const rec = createRecorder();
    assert.throws(() => rec.trace('t', v), {name: 'TypeError', message: 'untraceable value'});
    assert.equal(rec.entries.length, 0);
  }
});

test('symbol-keyed properties are not traced', () => {
  assert.deepEqual(enc({[Symbol('s')]: 1, k: 2}).props, [['k', 2]]);
});

test('encodeAll is one encoding call: a shared object is expanded once, then referenced', () => {
  const o = {n: 1};
  assert.deepEqual(createRecorder().encodeAll([o, o]), [
    {$: 'obj', id: 1, cls: 'Object', proto: 'Object.prototype', props: [['n', 1]]},
    {$: 'ref', id: 1},
  ]);
});

// ---- structural comparison (trace-format.md, comparison rule) ----

test('compareDocs: equal documents with different object key order compare equal', () => {
  assert.equal(compareDocs({a: 1, b: [2]}, {b: [2], a: 1}), null);
});

test('compareDocs: array order is significant and the first differing path is reported', () => {
  assert.deepEqual(compareDocs({x: [1, 2]}, {x: [2, 1]}), {path: '$.x[0]', a: 1, b: 2});
});

test('compareDocs: numbers are exact, so -0 differs from 0 and an encoded -0 differs from a number', () => {
  assert.equal(compareDocs({n: 0}, {n: -0})?.path, '$.n');
  assert.equal(compareDocs({n: 0}, {n: {$: 'num', v: '-0'}})?.path, '$.n');
  assert.equal(compareDocs({n: 0}, {n: 0.0}), null);
});

test('compareDocs: array length and missing keys are reported at their own paths', () => {
  assert.equal(compareDocs({x: [1, 2]}, {x: [1, 2, 3]})?.path, '$.x.length');
  const missing = compareDocs({a: 1}, {a: 1, b: 2});
  assert.equal(missing.path, '$.b');
  assert.equal(missing.a, '<missing>');
});

// ---- scenario contract: expectations taken by hand from the scenario sources ----

test('module code runs before run(): the job queue drains first (async/init-before-run)', async () => {
  const doc = await runScenario(caseFile('async/init-before-run.js'));
  assert.equal(doc.scenario, 'async/init-before-run');
  assert.equal(doc.result, 'ok');
  assert.deepEqual(doc.entries, [{label: 'order', values: [arrayOf(['module-init', 'module-microtask', 'run-start'], 1)]}]);
  assert.deepEqual(doc.unhandledRejections, []);
});

test('a module that throws at evaluation is result "throw" with the thrown value; run() is never called', async () => {
  const doc = await runScenario(caseFile('exceptions/module-throw-string.js'));
  assert.equal(doc.result, 'throw');
  assert.equal(doc.error, 'module failed');
  assert.deepEqual(doc.entries, [{label: 'before-throw', values: [1]}]);
});

test('a scenario-created Error thrown at module level is encoded with its name and message', async () => {
  const doc = await runScenario(caseFile('exceptions/module-throw-error.js'));
  assert.equal(doc.result, 'throw');
  assert.deepEqual(doc.error, errorOf(1, 'Error', 'module boom'));
  assert.deepEqual(doc.entries, [{label: 'loading', values: ['module']}]);
});

test('a rejected run() is result "throw"; entries traced before the rejection are kept', async () => {
  const doc = await runScenario(caseFile('async/run-rejects-error.js'));
  assert.equal(doc.result, 'throw');
  assert.deepEqual(doc.error, errorOf(1, 'Error', 'run failed'));
  assert.deepEqual(doc.entries, [
    {label: 'run-start', values: [true]},
    {label: 'inside', values: [1]},
  ]);
});

test('a run() rejecting with a plain object keeps the object as the error value', async () => {
  const doc = await runScenario(caseFile('async/run-rejects-object.js'));
  assert.equal(doc.result, 'throw');
  assert.deepEqual(doc.error, {
    $: 'obj',
    id: 1,
    cls: 'Object',
    proto: 'Object.prototype',
    props: [
      ['kind', 'timeout'],
      ['at', 10],
    ],
  });
  assert.deepEqual(doc.entries, [{label: 'after-advance', values: [10]}]);
});

test('a non-async run() that throws synchronously is also result "throw"', async () => {
  const doc = await runScenario(caseFile('async/sync-throw-in-run.js'));
  assert.equal(doc.result, 'throw');
  assert.equal(doc.error, 'sync value');
  assert.deepEqual(doc.entries, [{label: 'before-sync-throw', values: ['yes']}]);
});

test('fake timers fire by (due time, creation order), and the clock reads each due time', async () => {
  const doc = await runScenario(caseFile('timers/advance-order.js'));
  assert.deepEqual(doc.entries, [
    {label: 'fired', values: [arrayOf(['early@5', 'tie-first@10', 'tie-second@10', 'late@20'], 1), 30]},
  ]);
});

test('the job queue drains after each timer callback, before the next timer fires', async () => {
  const doc = await runScenario(caseFile('timers/microtasks-between-timers.js'));
  assert.deepEqual(doc.entries, [{label: 'between', values: [arrayOf(['timer-1', 'micro-after-1', 'timer-2'], 1)]}]);
});

test('a zero-delay timer created in a callback runs in the same advance, after timers already due', async () => {
  const doc = await runScenario(caseFile('timers/zero-delay-in-callback.js'));
  assert.deepEqual(doc.entries, [
    {label: 'zero-delay', values: [arrayOf(['outer@2', 'sibling@2', 'inner@2'], 1), 3]},
    {label: 'after-second-advance', values: [4, 13]},
  ]);
});

test('unhandled rejections are captured without crashing, in rejection order', async () => {
  const doc = await runScenario(caseFile('promises/unhandled-rejection.js'));
  assert.equal(doc.result, 'ok');
  assert.deepEqual(doc.entries, [{label: 'run-done', values: [true]}]);
  assert.deepEqual(doc.unhandledRejections, [errorOf(1, 'Error', 'first lost'), errorOf(2, 'TypeError', 'second lost')]);
});

test('a rejection handled before the final drain is not reported', async () => {
  const doc = await runScenario(caseFile('promises/rejection-handled-later.js'));
  assert.deepEqual(doc.entries, [{label: 'handled', values: ['handled later']}]);
  assert.deepEqual(doc.unhandledRejections, []);
});

test('await and then interleave in queue order (promises/await-vs-then-interleave)', async () => {
  const doc = await runScenario(caseFile('promises/await-vs-then-interleave.js'));
  assert.deepEqual(doc.entries, [
    {
      label: 'interleave',
      values: [arrayOf(['async-start', 'sync-after-call', 'async-after-await', 'then-1', 'async-after-second-await', 'run-after-first'], 1)],
    },
  ]);
});

test('then reactions run in registration order and chained steps after their predecessor (promises/then-chain-order)', async () => {
  const doc = await runScenario(caseFile('promises/then-chain-order.js'));
  assert.deepEqual(doc.entries, [{label: 'order', values: [arrayOf(['sync', 'a1 start', 'b1 start', 'a2', 'b2'], 1)]}]);
});

test('each scenario runs in a fresh child: ordinals restart at 1 and repeated runs are identical', async () => {
  const first = await runScenario(caseFile('identity/cycles.js'));
  const second = await runScenario(caseFile('objects/key-order.js'));
  assert.equal(first.entries[0].values[0].id, 1);
  assert.equal(second.entries[0].values[0].id, 1);
  assert.deepEqual(await runScenario(caseFile('objects/key-order.js')), second);
});

test('every case is deterministic: two child runs produce byte-identical output', async () => {
  for (const file of await listCases()) {
    const a = await runScenarioLine(file);
    const b = await runScenarioLine(file);
    assert.equal(a, b, relOf(file));
  }
});

test('the document scenario name is the case path without .js, or the basename outside the cases root', () => {
  assert.equal(scenarioName(caseFile('lexical/tdz-reference-error.js')), 'lexical/tdz-reference-error');
  assert.equal(scenarioName('/elsewhere/probe.js'), 'probe');
});

test('a run() that never settles fails loudly: exit 3 and no partial document', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'vn-oracle-'));
  try {
    const file = path.join(dir, 'hang.mjs');
    await writeFile(file, 'export function run() {\n  return new Promise(() => {})\n}\n');
    const env = {...process.env};
    delete env.NODE_TEST_CONTEXT;
    const r = spawnSync(process.execPath, [runCli, file], {encoding: 'utf8', env});
    assert.equal(r.status, 3);
    assert.equal(r.stdout, '');
    assert.match(r.stderr, /run\(\) never settled/);
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});

// ---- goldens and negative controls ----

test('golden check: every seed scenario matches its golden structurally and byte for byte', async () => {
  const {results, orphans} = await checkAll();
  assert.ok(results.length >= 60 && results.length <= 80, `case count ${results.length}`);
  assert.deepEqual(
    results.filter((r) => r.status !== 'ok').map((r) => [relOf(r.file), r.reason]),
    [],
  );
  assert.deepEqual(orphans, []);
});

test('negative control: a doctored golden is reported at the first differing path', async () => {
  const actual = await runScenarioLine(caseFile('lexical/var-hoisting.js'));
  const doctored = actual.replace('"var-after","values":[2]', '"var-after","values":[3]');
  assert.notEqual(doctored, actual, 'the doctoring must change the golden text');
  assert.match(compareWithGolden(actual, doctored), /first difference at \$\.entries\[1\]\.values\[0\]: 2 vs 3/);
  assert.equal(compareWithGolden(actual, actual), null);
});

test('negative control: a byte-level change and a missing golden are both reported', async () => {
  const actual = await runScenarioLine(caseFile('lexical/var-hoisting.js'));
  assert.match(compareWithGolden(actual, actual.replace('\n', ' \n')), /bytes differ/);
  assert.match(compareWithGolden(actual, null), /missing golden/);
});

test('golden files sit at golden/<case path>.json, e.g. golden/lexical/tdz-reference-error.js.json', () => {
  assert.equal(
    goldenPathFor(caseFile('lexical/tdz-reference-error.js')),
    path.join(goldenRoot, 'lexical', 'tdz-reference-error.js.json'),
  );
});

test('the corpus has 3 module-level throws and 3 run() throws or rejections (from the goldens)', async () => {
  const throwing = [];
  for (const file of await listCases()) {
    const doc = JSON.parse(await readFile(path.join(goldenRoot, `${relOf(file)}.json`), 'utf8'));
    if (doc.result === 'throw') throwing.push(doc.scenario);
  }
  assert.deepEqual(throwing.sort(), [
    'async/run-rejects-error',
    'async/run-rejects-object',
    'async/sync-throw-in-run',
    'exceptions/module-throw-after-trace',
    'exceptions/module-throw-error',
    'exceptions/module-throw-string',
  ]);
});

test('every case has the two header tags, and the covered requirement IDs are the expected set', async () => {
  const covered = new Set();
  for (const file of await listCases()) {
    const src = await readFile(file, 'utf8');
    assert.deepEqual(checkHeader(src), [], relOf(file));
    for (const id of src.split('\n')[1].replace('// @covers ', '').split(',')) covered.add(id);
  }
  assert.deepEqual([...covered].sort(), [
    'L01', 'L02', 'L03', 'L04', 'L06', 'L07', 'L08', 'L09', 'L10',
    'L11', 'L12', 'L13', 'L14', 'L15', 'L16', 'L18', 'L19', 'L21',
  ]);
});

// ---- lint: forbidden constructs; the lint itself is verified too ----

test('lint detects a class declaration (negative control for the lint itself)', () => {
  const problems = lintSource('// @tier 1\n// @covers L01\nclass A {}\n');
  assert.deepEqual(problems.map((p) => p.code), ['CLASS']);
  assert.equal(problems[0].line, 3);
});

test('lint rejects each forbidden construct with its code', () => {
  const head = '// @tier 1\n// @covers L01\n';
  const table = [
    ['new of a user function', 'function F() {}\nnew F()', 'NEW'],
    ['regular expression literal', 'const r = /a/', 'REGEXP'],
    ['__proto__ key', 'const o = { __proto__: null }', 'PROTO'],
    ['Math.random', 'Math.random()', 'RANDOM'],
    ['Object.defineProperty', 'Object.defineProperty({}, "x", {})', 'UNLISTED_API'],
    ['Reflect', 'Reflect.ownKeys({})', 'FORBIDDEN_NAME'],
    ['bare setTimeout', 'setTimeout(() => {}, 1)', 'FORBIDDEN_NAME'],
    ['console', 'console.log(1)', 'FORBIDDEN_NAME'],
    ['generator', 'function* g() {\n  yield 1\n}', 'GENERATOR'],
    ['top-level await', 'await Promise.resolve(1)', 'TOP_LEVEL_AWAIT'],
    ['dynamic import', 'import("vue")', 'DYNAMIC_IMPORT'],
    ['foreign import', 'import x from "lodash"', 'IMPORT'],
    ['default export', 'export default 1', 'EXPORT'],
    ['BigInt literal', 'const b = 1n', 'BIGINT'],
    ['toLocaleString', 'trace("x", (1).toLocaleString())', 'INTL'],
    ['with statement', 'with ({}) {}', 'PARSE'],
  ];
  for (const [name, body, code] of table) {
    const codes = lintSource(head + body + '\n').map((p) => p.code);
    assert.ok(codes.includes(code), `${name}: expected ${code}, got ${codes.join(',')}`);
  }
});

test('lint: engine error messages are not traced from catch; name, instanceof and scenario-made messages are allowed', () => {
  const head = '// @tier 1\n// @covers L13\n';
  const codes = (body) => lintSource(head + body + '\n').map((p) => p.code);
  assert.deepEqual(codes('try { null.x } catch (e) { trace("m", e.message) }'), ['ENGINE_MESSAGE']);
  assert.deepEqual(codes('try { null.x } catch (e) { trace("e", e) }'), ['ENGINE_MESSAGE']);
  assert.deepEqual(codes('try { null.x } catch (e) { trace("n", e.name, e instanceof TypeError) }'), []);
  assert.deepEqual(codes('try { throw new Error("boom") } catch (e) { trace("m", e.message) }'), []);
});

test('lint: every seed case is clean', async () => {
  for (const file of await listCases()) {
    assert.deepEqual(await lintFile(file), [], relOf(file));
  }
});

test('lint: a source without header tags is rejected', () => {
  assert.deepEqual(lintSource('trace(1)\n').map((p) => p.code), ['HEADER', 'HEADER']);
});

// ---- trace-format v1.1: `integrity` on non-extensible objects, placed right after `proto` ----

test('integrity: frozen, sealed and non-extensible plain objects carry the marker', () => {
  assert.deepEqual(enc(Object.freeze({a: 1})), {
    $: 'obj', id: 1, cls: 'Object', proto: 'Object.prototype', integrity: 'frozen', props: [['a', 1]],
  });
  assert.deepEqual(enc(Object.seal({a: 1})), {
    $: 'obj', id: 1, cls: 'Object', proto: 'Object.prototype', integrity: 'sealed', props: [['a', 1]],
  });
  assert.deepEqual(enc(Object.preventExtensions({a: 1})), {
    $: 'obj', id: 1, cls: 'Object', proto: 'Object.prototype', integrity: 'nonextensible', props: [['a', 1]],
  });
});

test('integrity: an empty object after preventExtensions is "frozen" (Object.isFrozen is true)', () => {
  assert.deepEqual(enc(Object.preventExtensions({})), {
    $: 'obj', id: 1, cls: 'Object', proto: 'Object.prototype', integrity: 'frozen', props: [],
  });
});

test('integrity: an extensible object has no integrity field', () => {
  const e = enc({a: 1});
  assert.equal(Object.hasOwn(e, 'integrity'), false);
  assert.deepEqual(Object.keys(e), ['$', 'id', 'cls', 'proto', 'props']);
});

test('integrity: the field sits right after proto, before props', () => {
  assert.deepEqual(Object.keys(enc(Object.freeze({a: 1}))), ['$', 'id', 'cls', 'proto', 'integrity', 'props']);
  assert.equal(
    JSON.stringify(enc(Object.seal({a: 1}))),
    '{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","integrity":"sealed","props":[["a",1]]}',
  );
});

test('integrity: arrays follow the same rule', () => {
  assert.deepEqual(enc(Object.freeze([1, 2])), {
    $: 'obj', id: 1, cls: 'Array', proto: 'other', integrity: 'frozen', props: [], elems: [1, 2], len: 2,
  });
  assert.deepEqual(enc(Object.seal([1, 2])), {
    $: 'obj', id: 1, cls: 'Array', proto: 'other', integrity: 'sealed', props: [], elems: [1, 2], len: 2,
  });
  assert.deepEqual(enc(Object.preventExtensions([1, 2])), {
    $: 'obj', id: 1, cls: 'Array', proto: 'other', integrity: 'nonextensible', props: [], elems: [1, 2], len: 2,
  });
});

test('integrity: Map, Set, Error and Promise get the marker too', () => {
  assert.deepEqual(enc(Object.freeze(new Map([['k', 1]]))), {
    $: 'obj', id: 1, cls: 'Map', proto: 'other', integrity: 'frozen', props: [], entries: [['k', 1]],
  });
  assert.deepEqual(enc(Object.preventExtensions(new Set([1]))), {
    $: 'obj', id: 1, cls: 'Set', proto: 'other', integrity: 'frozen', props: [], values: [1],
  });
  assert.deepEqual(enc(Object.freeze(new TypeError('x'))), {
    $: 'obj', id: 1, cls: 'Error', proto: 'other', integrity: 'frozen', props: [], name: 'TypeError', message: 'x',
  });
  assert.deepEqual(enc(Object.freeze(Promise.resolve(1))), {
    $: 'obj', id: 1, cls: 'Promise', proto: 'other', integrity: 'frozen', props: [],
  });
});

test('integrity: a frozen object nested in an extensible one carries the marker only on the nested object', () => {
  assert.deepEqual(enc({inner: Object.freeze({m: 1}), free: {n: 2}}), {
    $: 'obj',
    id: 1,
    cls: 'Object',
    proto: 'Object.prototype',
    props: [
      ['inner', {$: 'obj', id: 2, cls: 'Object', proto: 'Object.prototype', integrity: 'frozen', props: [['m', 1]]}],
      ['free', {$: 'obj', id: 3, cls: 'Object', proto: 'Object.prototype', props: [['n', 2]]}],
    ],
  });
});

test('integrity follows the current state, and never applies to functions or ref forms', () => {
  const rec = createRecorder();
  const o = {a: 1};
  rec.trace('open', o);
  Object.preventExtensions(o);
  rec.trace('closed', o);
  assert.equal(Object.hasOwn(rec.entries[0].values[0], 'integrity'), false);
  assert.equal(rec.entries[1].values[0].integrity, 'nonextensible');
  assert.deepEqual(enc(Object.freeze(function f() {})), {$: 'fn', name: 'f', length: 0});
  const refs = createRecorder();
  const g = Object.freeze({n: 1});
  refs.trace('t', g, g);
  assert.deepEqual(refs.entries[0].values[1], {$: 'ref', id: 1});
});

// ---- contract checks for the integrity scenarios and the existing frozen-assignment scenario ----

test('objects/frozen-assignment: the frozen root is marked "frozen"; the unfrozen inner object is not', async () => {
  const doc = await runScenario(caseFile('objects/frozen-assignment.js'));
  assert.deepEqual(doc.entries[1], {
    label: 'state',
    values: [
      {
        $: 'obj',
        id: 1,
        cls: 'Object',
        proto: 'Object.prototype',
        integrity: 'frozen',
        props: [
          ['n', 1],
          ['inner', {$: 'obj', id: 2, cls: 'Object', proto: 'Object.prototype', props: [['m', 5]]}],
        ],
      },
      true,
      false,
    ],
  });
});

test('objects/integrity-freeze-seal: frozen and sealed markers, a shallow freeze, and a frozen array', async () => {
  const doc = await runScenario(caseFile('objects/integrity-freeze-seal.js'));
  assert.deepEqual(doc.entries, [
    {label: 'sealed-add', values: ['TypeError']},
    {
      label: 'states',
      values: [
        {
          $: 'obj',
          id: 1,
          cls: 'Object',
          proto: 'Object.prototype',
          integrity: 'frozen',
          props: [
            ['a', 1],
            ['inner', {$: 'obj', id: 2, cls: 'Object', proto: 'Object.prototype', props: [['b', 2]]}],
          ],
        },
        {$: 'obj', id: 3, cls: 'Object', proto: 'Object.prototype', integrity: 'sealed', props: [['c', 4]]},
        true,
        true,
        false,
      ],
    },
    {
      label: 'frozen-array',
      values: [{$: 'obj', id: 4, cls: 'Array', proto: 'other', integrity: 'frozen', props: [], elems: [1, 2], len: 2}, false],
    },
  ]);
});

test('objects/integrity-prevent-extensions: nonextensible and empty-frozen markers; nested frozen only where frozen', async () => {
  const doc = await runScenario(caseFile('objects/integrity-prevent-extensions.js'));
  assert.deepEqual(doc.entries, [
    {label: 'add-blocked', values: ['TypeError']},
    {
      label: 'prevent',
      values: [
        {$: 'obj', id: 1, cls: 'Object', proto: 'Object.prototype', integrity: 'nonextensible', props: [['a', 2]]},
        {$: 'obj', id: 2, cls: 'Object', proto: 'Object.prototype', integrity: 'frozen', props: []},
        {$: 'obj', id: 3, cls: 'Array', proto: 'other', integrity: 'nonextensible', props: [], elems: [9, 8], len: 2},
        false,
      ],
    },
    {
      label: 'nested',
      values: [
        {
          $: 'obj',
          id: 4,
          cls: 'Object',
          proto: 'Object.prototype',
          props: [
            ['inner', {$: 'obj', id: 5, cls: 'Object', proto: 'Object.prototype', integrity: 'frozen', props: [['m', 1]]}],
            ['free', {$: 'obj', id: 6, cls: 'Object', proto: 'Object.prototype', props: [['n', 2]]}],
          ],
        },
      ],
    },
  ]);
});

// ---- builtins derived from the stdlib manifest for target test (compiler/profile/stdlib.json) ----

test('lint accepts builtins the manifest supports for test, including Object.is', () => {
  const head = '// @tier 1\n// @covers L07\n';
  const codes = (body) => lintSource(head + body + '\n').map((p) => p.code);
  assert.deepEqual(
    codes('trace("x", Object.is(NaN, NaN), Math.max(1, 2), JSON.stringify([1]), [1, 2].map((v) => v + 1).includes(3))'),
    [],
  );
});

test('lint allows methods the case defines and host timer methods, and rejects an unknown method', () => {
  const head = '// @tier 3\n// @covers L19\n';
  const codes = (body) => lintSource(head + body + '\n').map((p) => p.code);
  assert.deepEqual(codes('const o = { run() { return 1 } }\ntrace("x", o.run())'), []);
  assert.deepEqual(codes('const t = testHost.timers\nt.advance(1)\ntrace("x", t.now())'), []);
  assert.deepEqual(codes('const o = {}\ntrace("x", o.frobnicate())'), ['UNLISTED_API']);
});

test('lint rejects builtins the manifest does not support for test (negative controls)', () => {
  const head = '// @tier 1\n// @covers L01\n';
  const codes = (body) => lintSource(head + body + '\n').map((p) => p.code);
  assert.ok(codes('Reflect.get({}, "x")').includes('UNLISTED_API'), 'Reflect.get');
  assert.ok(codes('trace("x", "abc".normalize())').includes('UNLISTED_API'), 'String.prototype.normalize');
  assert.ok(codes('trace("x", notAGlobal(1))').includes('UNLISTED_API'), 'a free identifier that is no builtin');
  assert.ok(codes('trace("x", Math["PI"])').includes('UNLISTED_API'), 'computed access on a builtin');
});

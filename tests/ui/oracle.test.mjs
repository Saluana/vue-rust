// WP-V1 acceptance: the UI oracle. Harness behaviours are checked against hand-written expectations (the fake clock, the
// selector grammar, the snapshot rules, every testHost API in scratch scenarios outside the corpus, the error paths), plus
// determinism, isolation of runs, encoder equivalence between Node and the page, a negative control (a doctored golden is
// detected), the golden check and regeneration of every case, and the lint of every case. Chrome tests skip with a message
// when no Chrome is installed (tooling/chrome). Every browser and temporary directory is removed by the test that made it.
import { test, after } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromeSkipReason, openPage, serveDirectory } from '../../tooling/chrome/chrome.mjs';
import { symlink } from 'node:fs/promises';
import { createRecorder } from '../../tooling/oracle/trace.mjs';
import { createClock, delayOf, FRAME_MS } from '../../tooling/oracle/ui/clock.mjs';
import { checkSelector } from '../../tooling/oracle/ui/selector.mjs';
import { graphs } from '../../tooling/oracle/ui/encoder-graphs.mjs';
import { compileCase, listUiCases, runUiScenario, uiCasesRoot, uiGoldenRoot, vueShimSource } from '../../tooling/oracle/ui/run-ui.mjs';
import { canonicalLine, checkUiGoldens, compareWithGolden, goldenPathFor } from '../../tooling/oracle/ui/check-goldens.mjs';
import { lintUiCase, lintUiSource } from '../../tooling/oracle/ui/lint.mjs';
import { loadManifest } from '../../compiler/profile/index.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '../..');
const SKIP = chromeSkipReason();
// Chrome tests skip with the reason when Chrome is missing; the option is absent otherwise (node:test reads any skip value as skipped).
const CHROME = SKIP === null ? {} : { skip: SKIP };
const CORPUS = listUiCases(uiCasesRoot);
const byName = (name) => path.join(uiCasesRoot, name);

// ---- scratch scenarios: written to a temporary directory, run with a name outside tests/ui/cases, then removed ----
async function scratch(files, name = 'scratch/case') {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'vn-ui-test-'));
  try {
    for (const [rel, src] of Object.entries(files)) await writeFile(path.join(dir, rel), src);
    return await runUiScenario(dir, { name });
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}
// [label, ...values] per entry, for compact hand-written expectations.
const rows = (doc) => doc.entries.map((e) => [e.label, ...e.values]);

// ---- fake clock (Node, no Chrome) ----
test('clock: timers fire by due time, then in creation order; the job queue drains before the first and after each callback', async () => {
  let drains = 0;
  const clock = createClock(async () => {
    drains += 1;
    await null;
  });
  const log = [];
  clock.setTimeout(() => log.push(`A@${clock.now()}`), 10);
  clock.setTimeout(() => {
    log.push(`B@${clock.now()}`);
    clock.setTimeout(() => log.push(`N@${clock.now()}`), 0);
  }, 5);
  clock.setTimeout(() => log.push(`C@${clock.now()}`), 10);
  await clock.advance(10);
  assert.deepEqual(log, ['B@5', 'N@5', 'A@10', 'C@10']);
  assert.equal(clock.now(), 10);
  assert.equal(drains, 6, 'one drain before the timers, one after each of the 4 callbacks, one at the end');
});

test('clock: advance leaves later timers pending; clearTimeout cancels; the clock stops at the target', async () => {
  const clock = createClock(async () => {});
  const fired = [];
  clock.setTimeout(() => fired.push('at20'), 20);
  const gone = clock.setTimeout(() => fired.push('cancelled'), 3);
  clock.clearTimeout(gone);
  await clock.advance(5);
  assert.deepEqual(fired, []);
  assert.equal(clock.now(), 5);
  await clock.advance(15);
  assert.deepEqual(fired, ['at20']);
  assert.equal(clock.now(), 20);
});

test('clock: negative, NaN and missing delays count as 0; a non-function callback throws; arguments reach the callback', async () => {
  assert.equal(delayOf(-5), 0);
  assert.equal(delayOf(Number.NaN), 0);
  assert.equal(delayOf(undefined), 0);
  assert.equal(delayOf(7), 7);
  const clock = createClock(async () => {});
  const got = [];
  clock.setTimeout((a, b) => got.push([a, b, clock.now()]), -5, 'x', 2);
  await clock.advance(0);
  assert.deepEqual(got, [['x', 2, 0]]);
  assert.throws(() => clock.setTimeout('not a function', 1), TypeError);
});

test('clock: requestAnimationFrame callbacks run on FRAME_MS frames and receive the clock time', async () => {
  const clock = createClock(async () => {});
  const frames = [];
  clock.requestAnimationFrame((t) => frames.push(t));
  const cancelled = clock.requestAnimationFrame(() => frames.push('never'));
  clock.clearTimeout(cancelled);
  await clock.advance(FRAME_MS - 1);
  assert.deepEqual(frames, []);
  await clock.advance(1);
  assert.deepEqual(frames, [FRAME_MS]);
});

test('clock: an error thrown by a timer callback propagates out of advance', async () => {
  const clock = createClock(async () => {});
  clock.setTimeout(() => {
    throw new RangeError('timer failed');
  }, 1);
  await assert.rejects(clock.advance(1), RangeError);
});

// ---- selector grammar (Node, no Chrome) ----
test('selector: the documented grammar is accepted and everything else is rejected with a TypeError', () => {
  for (const ok of ['div', '#a', '.c', '[attr]', '[attr=value]', '[attr="v"]', "[attr='v']", 'li.item[data-id="2"]:nth-child(3)',
    'tag#x.y[z]', 'p:nth-child(1)', 'my-el']) {
    assert.equal(checkSelector(ok), ok);
  }
  for (const bad of ['', 'div span', 'div > p', 'a,b', '*', ':hover', 'div:first-child', '[a]:nth-child(1).x', '#', '.9x', 'DIV']) {
    assert.throws(() => checkSelector(bad), TypeError, bad);
  }
});

// ---- snapshot rules in the browser (hand-written expectations) ----
// The snapshot module is served from the repository root, so the page imports exactly the file the oracle uses.
const SNAPSHOT_CASES = [
  { name: 'text, comment and text nodes are not merged', html: 'a<!--c-->b<span></span>', ops: [],
    want: [{ $: 'text', v: 'a' }, { $: 'comment', v: 'c' }, { $: 'text', v: 'b' }, { $: 'el', tag: 'span', attrs: [], children: [] }] },
  { name: 'an empty text node is kept', html: '<p></p>', ops: [['text', '']],
    want: [{ $: 'el', tag: 'p', attrs: [], children: [] }, { $: 'text', v: '' }] },
  { name: 'attributes keep their insertion order', html: '<p b="1" a="2" id="x"></p>', ops: [],
    want: [{ $: 'el', tag: 'p', attrs: [['b', '1'], ['a', '2'], ['id', 'x']], children: [] }] },
  { name: 'an input without a value attribute has no value prop', html: '<input>', ops: [],
    want: [{ $: 'el', tag: 'input', attrs: [], children: [] }] },
  { name: 'a typed value is a dirty value prop', html: '<input>', ops: [['value', 'input', 'typed']],
    want: [{ $: 'el', tag: 'input', attrs: [], props: [['value', 'typed']], children: [] }] },
  { name: 'a value equal to its attribute has no prop', html: '<input value="v">', ops: [],
    want: [{ $: 'el', tag: 'input', attrs: [['value', 'v']], children: [] }] },
  { name: 'a changed value attribute with a clean value has no prop', html: '<input value="a">', ops: [['attr', 'input', ['value', 'b']]],
    want: [{ $: 'el', tag: 'input', attrs: [['value', 'b']], children: [] }] },
  { name: 'a plain checkbox has no props', html: '<input type="checkbox">', ops: [],
    want: [{ $: 'el', tag: 'input', attrs: [['type', 'checkbox']], children: [] }] },
  { name: 'a checked checkbox has a checked prop, and no checked attribute', html: '<input type="checkbox">', ops: [['checked', 'input', true]],
    want: [{ $: 'el', tag: 'input', attrs: [['type', 'checkbox']], props: [['checked', true]], children: [] }] },
  { name: 'a checked attribute and a checked property agree: no props', html: '<input type="checkbox" checked>', ops: [],
    want: [{ $: 'el', tag: 'input', attrs: [['type', 'checkbox'], ['checked', '']], children: [] }] },
  { name: 'setting a checkbox value writes its value attribute', html: '<input type="checkbox">', ops: [['value', 'input', 'x']],
    want: [{ $: 'el', tag: 'input', attrs: [['type', 'checkbox'], ['value', 'x']], children: [] }] },
  { name: 'a range without a value attribute is at its midpoint: the prop is "50"', html: '<input type="range">', ops: [],
    want: [{ $: 'el', tag: 'input', attrs: [['type', 'range']], props: [['value', '50']], children: [] }] },
  { name: 'a range value is clamped to max: the prop is "100", the attribute keeps 200', html: '<input type="range" max="100" value="200">', ops: [],
    want: [{ $: 'el', tag: 'input', attrs: [['type', 'range'], ['max', '100'], ['value', '200']], props: [['value', '100']], children: [] }] },
  { name: 'text input values drop line breaks: the prop shows the stored value', html: '<input type="text">', ops: [['value', 'input', 'a\nb']],
    want: [{ $: 'el', tag: 'input', attrs: [['type', 'text']], props: [['value', 'ab']], children: [] }] },
  { name: 'a textarea with its text as value has no prop', html: '<textarea>a\nb</textarea>', ops: [],
    want: [{ $: 'el', tag: 'textarea', attrs: [], children: [{ $: 'text', v: 'a\nb' }] }] },
  { name: 'a textarea value set by script is a dirty prop', html: '<textarea>a</textarea>', ops: [['value', 'textarea', 'x']],
    want: [{ $: 'el', tag: 'textarea', attrs: [], props: [['value', 'x']], children: [{ $: 'text', v: 'a' }] }] },
  { name: 'the first option of a single select is selected without an attribute: selected prop', html: '<select><option value="a">A</option><option value="b">B</option></select>', ops: [],
    want: [{ $: 'el', tag: 'select', attrs: [], children: [
      { $: 'el', tag: 'option', attrs: [['value', 'a']], props: [['selected', true]], children: [{ $: 'text', v: 'A' }] },
      { $: 'el', tag: 'option', attrs: [['value', 'b']], children: [{ $: 'text', v: 'B' }] }] }] },
  { name: 'an option with the selected attribute has no selected prop, and the select no value prop', html: '<select><option value="a">A</option><option value="b" selected>B</option></select>', ops: [],
    want: [{ $: 'el', tag: 'select', attrs: [], children: [
      { $: 'el', tag: 'option', attrs: [['value', 'a']], children: [{ $: 'text', v: 'A' }] },
      { $: 'el', tag: 'option', attrs: [['value', 'b'], ['selected', '']], children: [{ $: 'text', v: 'B' }] }] }] },
  { name: 'a disabled button has the attribute and no prop', html: '<button disabled>x</button>', ops: [],
    want: [{ $: 'el', tag: 'button', attrs: [['disabled', '']], children: [{ $: 'text', v: 'x' }] }] },
  { name: 'an svg element carries ns svg', html: '<svg width="1"></svg>', ops: [],
    want: [{ $: 'el', tag: 'svg', ns: 'svg', attrs: [['width', '1']], children: [] }] },
];

let browser = null;
const site = { value: null };
after(async () => {
  if (browser !== null) await browser.close();
  if (site.value !== null) await site.value.close();
  if (siteDir !== null) await rm(siteDir, { recursive: true, force: true });
});

// The page is an empty document served from a temporary directory that links to tooling/oracle, so the browser code
// under test is the very file the oracle uses. The directory is removed when the test process ends.
let siteDir = null;
async function pageOnRepo() {
  if (siteDir === null) {
    siteDir = await mkdtemp(path.join(os.tmpdir(), 'vn-ui-site-'));
    await writeFile(path.join(siteDir, 'index.html'), '<!doctype html><title>oracle</title>\n');
    await symlink(path.join(repo, 'tooling'), path.join(siteDir, 'tooling'));
    site.value = await serveDirectory(siteDir);
  }
  if (browser === null) browser = await openPage({ viewport: { width: 800, height: 600, dpr: 1 }, timeout: 30000 });
  await browser.goto(site.value.url);
  return browser;
}

test('snapshot: every rule of the canonical element snapshot, against hand-written expectations', CHROME, async () => {
  const page = await pageOnRepo();
  const got = await page.evaluate(async (cases) => {
    const { snapshotNode } = await import('/tooling/oracle/ui/snapshot.mjs');
    const out = [];
    for (const c of cases) {
      document.body.innerHTML = c.html;
      for (const [kind, sel, arg] of c.ops) {
        const el = sel === '' ? null : document.querySelector(sel);
        if (kind === 'text') document.body.appendChild(document.createTextNode(sel));
        else if (kind === 'value') el.value = arg;
        else if (kind === 'checked') el.checked = arg;
        else if (kind === 'attr') el.setAttribute(arg[0], arg[1]);
      }
      out.push([...document.body.childNodes].map(snapshotNode));
    }
    return out;
  }, SNAPSHOT_CASES.map(({ html, ops }) => ({ html, ops: ops.map(([k, a, b]) => (k === 'text' ? [k, a, undefined] : [k, a, b])) })));
  SNAPSHOT_CASES.forEach((c, i) => assert.deepEqual(got[i], c.want, c.name));
});

test('snapshot: a DOM mutated after the snapshot is not part of it (a negative control on the expectations above)', CHROME, async () => {
  const page = await pageOnRepo();
  const [before, after2] = await page.evaluate(async () => {
    const { snapshotNode } = await import('/tooling/oracle/ui/snapshot.mjs');
    document.body.innerHTML = '<input value="a">';
    const el = document.querySelector('input');
    const first = snapshotNode(el);
    el.value = 'b';
    return [first, snapshotNode(el)];
  });
  assert.deepEqual(before.props, undefined);
  assert.deepEqual(after2.props, [['value', 'b']]);
});

// ---- testHost API, hand-written expectations (scratch scenarios) ----
test('testHost: mount, html, find, findAll, attr, text, exists and unmount', CHROME, async () => {
  const doc = await scratch({
    'scenario.js': `// @tier 1
// @covers U01,V18
import C from './C.vue'
export function run() {
  const m = testHost.mount(C, { n: 1 })
  trace('html', m.html())
  trace('find-null', testHost.find(m, '#missing'))
  trace('count', testHost.findAll(m, 'p').length)
  const a = testHost.find(m, '#a')
  trace('attr', a.attr('class'), a.attr('nope'))
  trace('text', a.text(), a.exists())
  m.unmount()
  trace('after-unmount', a.exists(), m.html())
}
`,
    'C.vue': '<script setup>\ndefineProps({ n: Number })\n</script>\n<template>\n  <p id="a" class="x">hi {{ n }}</p>\n</template>\n',
  });
  assert.equal(doc.result, 'ok');
  assert.deepEqual(rows(doc), [
    ['html', '<p id="a" class="x">hi 1</p>'],
    ['find-null', null],
    ['count', 1],
    ['attr', 'x', null],
    ['text', 'hi 1', true],
    ['after-unmount', false, ''],
  ]);
});

test('testHost: events bubble or not as in browsers; input, change, check, select and keydown dispatch the right events', CHROME, async () => {
  const doc = await scratch({
    'scenario.js': `// @tier 2
// @covers U05,U06
import E from './E.vue'
export async function run() {
  const m = testHost.mount(E, { onEv: (v) => trace('ev', v) })
  testHost.click(testHost.find(m, '#btn'))
  testHost.focus(testHost.find(m, '#inp'))
  testHost.input(testHost.find(m, '#inp'), 'x')
  testHost.change(testHost.find(m, '#inp'), 'x')
  testHost.check(testHost.find(m, '#box'), true)
  testHost.check(testHost.find(m, '#box'), true)
  testHost.check(testHost.find(m, '#box'), false)
  testHost.keydown(testHost.find(m, '#inp'), 'Enter', { ctrlKey: true })
  testHost.keydown(testHost.find(m, '#inp'), 'Enter')
  testHost.select(testHost.find(m, '#sel'), 'b')
  await testHost.tick()
  trace('state', testHost.find(m, '#box').prop('checked'), testHost.find(m, '#sel').prop('value'))
}
`,
    'E.vue': `<script setup>
defineEmits(['ev'])
</script>
<template>
  <div id="outer" @click="$emit('ev', 'outer')" @focus="$emit('ev', 'outer-focus')" @change="$emit('ev', 'outer-change')" @keydown.ctrl.enter="$emit('ev', 'ctrl-enter')">
    <button id="btn" @click="$emit('ev', 'button')">b</button>
    <input id="inp" type="text" @focus="$emit('ev', 'inp-focus')" @input="$emit('ev', 'inp-input')" @change="$emit('ev', 'inp-change')">
    <input id="box" type="checkbox" @change="$emit('ev', 'box-change')">
    <select id="sel" @change="$emit('ev', 'sel-change')"><option value="a">a</option><option value="b">b</option></select>
  </div>
</template>
`,
  });
  assert.equal(doc.result, 'ok');
  assert.deepEqual(rows(doc), [
    ['ev', 'button'], ['ev', 'outer'],
    ['ev', 'inp-focus'],
    ['ev', 'inp-input'],
    ['ev', 'inp-change'], ['ev', 'outer-change'],
    ['ev', 'outer'], ['ev', 'box-change'], ['ev', 'outer-change'],
    ['ev', 'outer'], ['ev', 'box-change'], ['ev', 'outer-change'],
    ['ev', 'ctrl-enter'],
    ['ev', 'sel-change'], ['ev', 'outer-change'],
    ['state', false, 'b'],
  ]);
});

test('testHost: misuse throws a TypeError at the call, and a radio already unchecked is left alone', CHROME, async () => {
  const doc = await scratch({
    'scenario.js': `// @tier 1
// @covers U05
import E from './E.vue'
export function run() {
  const m = testHost.mount(E, {})
  const tries = [
    () => testHost.input(testHost.find(m, '#box'), 'x'),
    () => testHost.check(testHost.find(m, '#inp'), true),
    () => testHost.select(testHost.find(m, '#inp'), 'a'),
    () => testHost.find(m, 'div > p'),
    () => testHost.find(m, '#inp').prop('style'),
    () => testHost.dispatch(testHost.find(m, '#inp'), 'click', { nope: 1 }),
    () => testHost.container('bad id'),
    () => testHost.mount(E, {}, { unknown: 1 }),
    () => testHost.check(testHost.find(m, '#rad'), false),
    () => testHost.keydown(testHost.find(m, '#inp'), 7),
  ]
  for (const t of tries) {
    try {
      t()
      trace('no-error')
    } catch (e) {
      trace('error', e.name, e instanceof TypeError)
    }
  }
}
`,
    'E.vue': `<template>
  <div>
    <input id="inp" type="text">
    <input id="box" type="checkbox">
    <input id="rad" type="radio" name="g">
  </div>
</template>
`,
  });
  const errors = rows(doc).filter((r) => r[0] === 'error');
  assert.deepEqual(errors, Array(9).fill(['error', 'TypeError', true]));
  assert.deepEqual(rows(doc).filter((r) => r[0] === 'no-error'), [['no-error']]);
  assert.equal(doc.entries.length, 10);
});

test('testHost: a click right after mount at a positive time is handled (the event timestamp guard of the runtime applies only inside one dispatch)', CHROME, async () => {
  const doc = await scratch({
    'scenario.js': `// @tier 2
// @covers U05,V18
import B from './B.vue'
export async function run() {
  await testHost.timers.advance(5)
  const m = testHost.mount(B, { log: (v) => trace('log', v) })
  testHost.click(testHost.find(m, '#b'))
  trace('same-instant-done')
  await testHost.timers.advance(1)
  testHost.click(testHost.find(m, '#b'))
  trace('later-done')
}
`,
    'B.vue': `<script setup>
defineProps({ log: null })
</script>
<template>
  <button id="b" @click="log('clicked')">b</button>
</template>
`,
  });
  assert.deepEqual(rows(doc), [['log', 'clicked'], ['same-instant-done'], ['log', 'clicked'], ['later-done']]);
});

test('testHost: timers, Date.now and performance.now read the fake clock; globals are replaced', CHROME, async () => {
  const doc = await scratch({
    'scenario.js': `export async function run() {
  trace('dates', Date.now(), performance.now())
  setTimeout(() => trace('timer', Date.now()), 5)
  await testHost.timers.advance(4)
  trace('before', Date.now())
  await testHost.timers.advance(1)
  trace('after', Date.now(), testHost.timers.now())
  const id = setTimeout(() => trace('never'), 3)
  clearTimeout(id)
  await testHost.timers.advance(10)
  trace('end', Date.now())
}
`,
  });
  assert.deepEqual(rows(doc), [['dates', 0, 0], ['before', 4], ['timer', 5], ['after', 5, 5], ['end', 15]]);
});

test('testHost: a throwing module and a throwing run become result "throw"; the error is traced by name and message', CHROME, async () => {
  const onModule = await scratch({ 'scenario.js': "trace('before-throw')\nthrow new Error('module boom')\n" });
  assert.equal(onModule.result, 'throw');
  assert.deepEqual(rows(onModule), [['before-throw']]);
  assert.deepEqual(onModule.error, { $: 'obj', id: 1, cls: 'Error', proto: 'other', props: [], name: 'Error', message: 'module boom' });
  const onRun = await scratch({ 'scenario.js': "export async function run() {\n  trace('a')\n  throw new RangeError('late')\n}\n" });
  assert.equal(onRun.result, 'throw');
  assert.deepEqual(rows(onRun), [['a']]);
  assert.equal(onRun.error.name, 'RangeError');
});

test('testHost: an unhandled rejection is reported once the run ends; a handled one is not', CHROME, async () => {
  const doc = await scratch({
    'scenario.js': `export async function run() {
  Promise.reject(new Error('lost'))
  const handled = Promise.reject(new Error('caught'))
  handled.catch(() => trace('caught-handler'))
  await testHost.tick()
  trace('end')
}
`,
  });
  assert.deepEqual(rows(doc), [['caught-handler'], ['end']]);
  assert.deepEqual(doc.unhandledRejections, [{ $: 'obj', id: 1, cls: 'Error', proto: 'other', props: [], name: 'Error', message: 'lost' }]);
});

test('harness: a run export that is not a function, and an uncaught error in the page, make the run fail', CHROME, async () => {
  await assert.rejects(scratch({ 'scenario.js': 'export const run = 1\n' }), /export "run" must be a function/);
  await assert.rejects(scratch({
    'scenario.js': "export function run() {\n  document.body.addEventListener('click', () => {\n    throw new Error('listener')\n  })\n  testHost.click(testHost.container('zone'))\n}\n",
  }), /uncaught error in the page: .*listener/);
});

test('isolation: a global written by one run is not visible to the next run (each run has its own realm)', CHROME, async () => {
  await scratch({ 'scenario.js': "globalThis.leakedFlag = 'set'\ntrace('written')\n" });
  const doc = await scratch({ 'scenario.js': "trace('flag', typeof globalThis.leakedFlag)\n" });
  assert.deepEqual(rows(doc), [['flag', 'undefined']]);
});

test('determinism: the same scenario run twice gives the same canonical line', CHROME, async () => {
  const dir = byName('static/elements-text');
  const first = canonicalLine(await runUiScenario(dir));
  const second = canonicalLine(await runUiScenario(dir));
  assert.equal(second, first);
});

// ---- encoder equivalence: the page encodes the same graphs with the same trace.mjs as Node ----
test('encoder: the page and Node produce identical traces for 31 value graphs (cycles, aliases, holes, frozen, Map, Set, Error, Promise)', CHROME, async () => {
  const list = graphs();
  assert.ok(list.length >= 25, `${list.length} graphs`);
  const rec = createRecorder();
  list.forEach((make, i) => rec.trace(`g${i}`, make()));
  const expected = JSON.stringify(rec.entries);
  const page = await pageOnRepo();
  const got = await page.evaluate(async () => {
    const trace = await import('/tooling/oracle/trace.mjs');
    const graphsModule = await import('/tooling/oracle/ui/encoder-graphs.mjs');
    const r = trace.createRecorder();
    graphsModule.graphs().forEach((make, i) => r.trace(`g${i}`, make()));
    return JSON.stringify(r.entries);
  });
  assert.equal(got, expected);
});

// ---- negative control: a doctored golden is detected ----
test('negative control: a golden with one changed value is reported as a mismatch', CHROME, async () => {
  const dir = byName('static/elements-text');
  const doc = await runUiScenario(dir);
  const actual = canonicalLine(doc);
  const golden = await readFile(goldenPathFor(dir), 'utf8');
  assert.equal(compareWithGolden(actual, golden), null, 'the committed golden matches the run');
  const doctored = golden.replace('hello world', 'hello WORLD');
  assert.notEqual(doctored, golden, 'the doctoring changed the golden');
  const reason = compareWithGolden(actual, doctored);
  assert.ok(reason !== null && /first difference/.test(reason), `reason: ${reason}`);
});

// ---- the vue shim, the compiler and the lint (no Chrome) ----
test('shim: the vue module re-exports exactly the 171 vue exports of the manifest', () => {
  const names = [...vueShimSource().matchAll(/^export const (\w+) = pick\(/gm)].map((m) => m[1]);
  const manifest = [...new Set(loadManifest().vueExports.map((e) => e.name))].sort();
  assert.equal(names.length, 171);
  assert.deepEqual([...names].sort(), manifest);
});

test('compile: a component with a construct outside the profile is rejected before Chrome starts', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'vn-ui-test-'));
  try {
    await writeFile(path.join(dir, 'Bad.vue'), '<script setup>\nconst f = new Function("return 1")\n</script>\n<template>\n  <p>{{ f }}</p>\n</template>\n');
    assert.throws(() => compileCase(dir), /does not compile/);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('lint: every case passes the lint (header, imports, testHost members, language rules, compilation)', async () => {
  const problems = [];
  for (const dir of CORPUS) {
    for (const p of await lintUiCase(dir)) problems.push(`${path.relative(uiCasesRoot, dir)}: ${p.code} ${p.message}`);
  }
  assert.deepEqual(problems, []);
});

test('lint: a scenario that breaks the rules is reported (negative control)', () => {
  const bad = '// @tier 9\n// @covers U99\nimport fs from "node:fs"\nimport { onServerPrefetch } from "vue"\ntestHost.nope(1)\n';
  const codes = lintUiSource(bad, '/nonexistent').map((p) => p.code);
  for (const code of ['HEADER', 'IMPORT', 'VUE_API', 'TESTHOST']) assert.ok(codes.includes(code), `${code} in ${codes}`);
});

// ---- goldens: every case against its golden, and regeneration byte for byte ----
test('goldens: every case matches its committed golden, and no golden is orphaned', { ...CHROME, timeout: 600000 }, async () => {
  const { results, orphans } = await checkUiGoldens({});
  const failed = results.filter((r) => r.status !== 'ok').map((r) => `${r.scenario}: ${r.reason}`);
  assert.deepEqual(failed, []);
  assert.deepEqual(orphans, []);
  assert.equal(results.length, CORPUS.length);
});

test('goldens: regenerating every case gives the committed bytes exactly', { ...CHROME, timeout: 600000 }, async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'vn-ui-golden-'));
  try {
    const { results } = await checkUiGoldens({ update: true, goldenRoot: root });
    assert.equal(results.filter((r) => r.status === 'fail').length, 0);
    const committed = [];
    const walk = async (dir) => {
      for (const ent of await readdir(dir, { withFileTypes: true })) {
        const p = path.join(dir, ent.name);
        if (ent.isDirectory()) await walk(p);
        else if (ent.name.endsWith('.json')) committed.push(p);
      }
    };
    await walk(uiGoldenRoot);
    assert.equal(committed.length, CORPUS.length);
    for (const file of committed) {
      const regenerated = await readFile(path.join(root, path.relative(uiGoldenRoot, file)), 'utf8');
      assert.equal(regenerated, await readFile(file, 'utf8'), path.relative(uiGoldenRoot, file));
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

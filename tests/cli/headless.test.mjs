// vue-native headless target (WP-APP1): each demo is built into a native binary that mounts the app in the runtime's
// DocHost and runs an interaction script. The expected values below are written by hand from what the demos specify
// (Vue 3 behaviour), not copied from the runtime's output.
import { describe, test, before } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { buildHeadlessApp } from '../../tooling/cli/build.mjs';
import { EXAMPLES, FIXTURES, capture, freshPath, scratch } from './helpers.mjs';

// Runs a headless binary on a script (an array of steps). Returns { status, results, stderr }.
function runApp(binary, steps) {
  const dir = scratch(`script-${path.basename(binary)}-${steps.length}`);
  const script = path.join(dir, 'script.json');
  fs.writeFileSync(script, JSON.stringify({ steps }));
  const r = spawnSync(binary, [script], { encoding: 'utf8', timeout: 60000 });
  const results = r.status === 0 ? JSON.parse(r.stdout).map(withoutScopeStamps) : null;
  return { status: r.status, results, stderr: r.stderr };
}

// Like Vue, the runtime stamps every element of a <style scoped> component with data-v-<hash> and the mount container
// with data-v-app. The expectations below are about structure and text, so a result's `html` has those stamps removed;
// `rawHtml` keeps them and the first Counter test asserts that they are really there.
function withoutScopeStamps(result) {
  if (!result || typeof result.html !== 'string') return result;
  return { ...result, rawHtml: result.html, html: result.html.replace(/ data-v-(?:[0-9a-f]{8}|app)=""/g, '') };
}

// The text of element #id in an html result (the markup of one element, as the snapshot prints it).
function textOf(html, id) {
  const m = new RegExp(`id="${id}"[^>]*>([^<]*)<`).exec(html);
  return m ? m[1] : null;
}

describe('headless: Counter (examples/counter)', () => {
  let binary;
  before(() => {
    const out = freshPath('headless-counter');
    binary = path.join(buildHeadlessApp({ root: path.join(EXAMPLES, 'counter'), entry: 'src/App.vue', out, name: 'counter' }).dir, 'counter');
  });

  test('the initial DOM shows count 0, double 0, reset disabled and no threshold message', () => {
    const r = runApp(binary, [{ op: 'snapshot', name: 'initial' }]);
    assert.equal(r.status, 0, r.stderr);
    const html = r.results[0].html;
    assert.equal(textOf(html, 'count'), 'Count: 0');
    assert.equal(textOf(html, 'doubled'), 'Double: 0');
    assert.match(html, /<button id="reset" disabled="">/);
    assert.match(html, /<h1>Counter<\/h1>/);
    // the scope stamps are present in the raw markup (the other assertions read it without them)
    assert.match(r.results[0].rawHtml, /^<div id="app" data-v-app="">/);
    assert.match(r.results[0].rawHtml, /<main data-v-[0-9a-f]{8}="" class="counter">/);
    assert.doesNotMatch(html, /id="many"/);
    assert.match(html, /<!--v-if-->/);
  });

  test('clicks add one each; doubled follows; reset is enabled after the first click', () => {
    const r = runApp(binary, [
      { op: 'click', selector: '#increment' },
      { op: 'html', selector: '#count' },
      { op: 'click', selector: '#increment' },
      { op: 'snapshot', name: 'two' },
    ]);
    assert.equal(r.status, 0, r.stderr);
    assert.equal(textOf(r.results[1].html, 'count'), 'Count: 1');
    assert.equal(textOf(r.results[3].html, 'count'), 'Count: 2');
    assert.equal(textOf(r.results[3].html, 'doubled'), 'Double: 4');
    assert.doesNotMatch(r.results[3].html, /<button id="reset" disabled/);
  });

  test('the threshold message appears above five clicks and reset clears everything', () => {
    const clicks = Array.from({ length: 5 }, () => ({ op: 'click', selector: '#increment' }));
    const r = runApp(binary, [
      ...clicks,
      { op: 'snapshot', name: 'five' },
      { op: 'click', selector: '#increment' },
      { op: 'snapshot', name: 'six' },
      { op: 'click', selector: '#reset' },
      { op: 'snapshot', name: 'reset' },
    ]);
    assert.equal(r.status, 0, r.stderr);
    const [five, six, reset] = [r.results[5], r.results[7], r.results[9]];
    assert.equal(textOf(five.html, 'count'), 'Count: 5');
    assert.doesNotMatch(five.html, /id="many"/);
    assert.equal(textOf(six.html, 'count'), 'Count: 6');
    assert.equal(textOf(six.html, 'doubled'), 'Double: 12');
    assert.equal(textOf(six.html, 'many'), 'You clicked more than five times.');
    assert.equal(textOf(reset.html, 'count'), 'Count: 0');
    assert.equal(textOf(reset.html, 'doubled'), 'Double: 0');
    assert.match(reset.html, /<button id="reset" disabled="">/);
    assert.doesNotMatch(reset.html, /id="many"/);
  });

  test('negative control: a selector that matches nothing fails the run (exit 1)', () => {
    const r = runApp(binary, [{ op: 'click', selector: '#no-such-button' }]);
    assert.equal(r.status, 1);
    assert.match(r.stderr, /no element matches #no-such-button/);
  });

  test('--timing keeps the result and adds only vn-timing lines to stderr; without it there are none', () => {
    const dir = scratch('timing-flag');
    const script = path.join(dir, 'script.json');
    fs.writeFileSync(script, JSON.stringify({ steps: [{ op: 'click', selector: '#increment' }, { op: 'html', selector: '#count' }, { op: 'snapshot', name: 'two' }] }));
    const plain = spawnSync(binary, [script], { encoding: 'utf8' });
    const timed = spawnSync(binary, ['--timing', script], { encoding: 'utf8' });
    assert.equal(plain.status, 0, plain.stderr);
    assert.equal(timed.status, 0, timed.stderr);
    assert.equal(timed.stdout, plain.stdout);
    // negative control: the default run prints no timing line
    assert.doesNotMatch(plain.stderr, /vn-timing/);
    // one line for the mount, one for the click (the html and snapshot steps are not timed) and one for the exit
    const phases = timed.stderr.split('\n').filter((line) => line.startsWith('vn-timing ')).map((line) => JSON.parse(line.slice('vn-timing '.length)).phase);
    assert.deepEqual(phases, ['mounted', 'step', 'exit']);
  });
});

describe('headless: script and usage errors', () => {
  let binary;
  before(() => {
    const out = freshPath('headless-counter-errors');
    binary = path.join(buildHeadlessApp({ root: path.join(EXAMPLES, 'counter'), entry: 'src/App.vue', out, name: 'counter-errors' }).dir, 'counter-errors');
  });

  test('an unknown op is a script error (exit 2)', () => {
    const r = runApp(binary, [{ op: 'hover', selector: '#increment' }]);
    assert.equal(r.status, 2);
    assert.match(r.stderr, /unknown op "hover"/);
  });

  test('a script that is not JSON is a script error (exit 2)', () => {
    const dir = scratch('script-not-json');
    const file = path.join(dir, 'bad.json');
    fs.writeFileSync(file, '{"steps": [');
    const r = spawnSync(binary, [file], { encoding: 'utf8' });
    assert.equal(r.status, 2);
    assert.match(r.stderr, /not JSON/);
  });

  test('running without a script is a usage error (exit 2)', () => {
    const r = spawnSync(binary, [], { encoding: 'utf8' });
    assert.equal(r.status, 2);
    assert.match(r.stderr, /usage:/);
  });

  test('a selector outside the supported subset is a script error (exit 2)', () => {
    const r = runApp(binary, [{ op: 'html', selector: 'main h1' }]);
    assert.equal(r.status, 2);
    assert.match(r.stderr, /selector main h1 is not supported/);
  });

  test('the supported selector forms match: class, tag, attribute, id compound and comma list', () => {
    const r = runApp(binary, [
      { op: 'html', selector: '.counter' },
      { op: 'html', selector: '[id=count]' },
      { op: 'html', selector: 'p#doubled' },
      { op: 'html', selector: '#count,#doubled' },
    ]);
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.results[0].html, /^<main class="counter">/);
    assert.equal(r.results[1].html, '<p id="count">Count: 0</p>');
    assert.equal(r.results[2].html, '<p id="doubled">Double: 0</p>');
    assert.equal(r.results[3].html, '<p id="count">Count: 0</p>');
  });

  test('build: a headless app can be built again into its own directory', () => {
    const out = freshPath('headless-rebuild');
    const first = buildHeadlessApp({ root: path.join(EXAMPLES, 'counter'), entry: 'src/App.vue', out, name: 'counter' });
    const second = buildHeadlessApp({ root: path.join(EXAMPLES, 'counter'), entry: 'src/App.vue', out, name: 'counter' });
    assert.equal(second.dir, first.dir);
    assert.deepEqual(JSON.parse(fs.readFileSync(path.join(out, 'manifest.json'), 'utf8')).files, ['counter', 'manifest.json']);
  });

  test('build: --target headless writes the binary and a tool-owned manifest', () => {
    const out = freshPath('headless-manifest');
    const r = capture(['build', path.join(EXAMPLES, 'counter'), '--target', 'headless', '--out', out]);
    assert.equal(r.code, 0, r.stderr);
    const manifest = JSON.parse(fs.readFileSync(path.join(out, 'manifest.json'), 'utf8'));
    assert.equal(manifest.tool, 'vue-native');
    assert.equal(manifest.target, 'headless');
    assert.deepEqual(manifest.files, ['counter', 'manifest.json']);
    assert.ok(fs.statSync(path.join(out, 'counter')).size > 100000);
  });
});

describe('headless: Quick Quote (examples/quick-quote)', () => {
  let binary;
  before(() => {
    const out = freshPath('headless-quick-quote');
    binary = path.join(buildHeadlessApp({ root: path.join(EXAMPLES, 'quick-quote'), entry: 'src/App.vue', out, name: 'quick-quote' }).dir, 'quick-quote');
  });

  test('the initial quote shows two lines, a total of $29.99 and the ready status', () => {
    const r = runApp(binary, [{ op: 'snapshot', name: 'initial' }]);
    assert.equal(r.status, 0, r.stderr);
    const html = r.results[0].html;
    assert.equal(textOf(html, 'total'), 'Total: $29.99');
    assert.equal(textOf(html, 'status'), 'Quote ready');
    assert.deepEqual([...html.matchAll(/id="(line-\d+)"/g)].map((m) => m[1]), ['line-1', 'line-2']);
  });
});

// The whole Quick Quote demo, run for the example itself (FieldInput uses defineModel, i.e. `useModel`) and for a fixture
// with FieldInput written without defineModel. Every other behaviour of the demo is checked here. Expected totals are hand
// arithmetic in cents: 2 x 1250 + 1 x 499 = 2999; quantity 3 on line 2: 2 x 1250 + 3 x 499 = 3997; after removing line 1:
// 3 x 499 + 1 x 0 = 1497.
const QUICK_QUOTE_VARIANTS = [
  { label: 'examples/quick-quote, defineModel', root: path.join(EXAMPLES, 'quick-quote'), name: 'quick-quote-sequence' },
  { label: 'tests/cli/fixtures/quick-quote-no-model', root: path.join(FIXTURES, 'quick-quote-no-model'), name: 'quick-quote-no-model' },
];
for (const { label, root, name } of QUICK_QUOTE_VARIANTS) describe(`headless: Quick Quote interaction sequence (${label})`, () => {
  let binary;
  before(() => {
    const out = freshPath(`headless-${name}`);
    binary = path.join(buildHeadlessApp({ root, entry: 'src/App.vue', out, name }).dir, name);
  });

  const errorsOf = (html) => [...html.matchAll(/class="error">([^<]*)</g)].map((m) => m[1]);
  const lineIds = (html) => [...html.matchAll(/id="(line-\d+)"/g)].map((m) => m[1]);

  test('the whole interaction sequence matches the demo specification', () => {
    const r = runApp(binary, [
      { op: 'snapshot', name: 'initial' },
      { op: 'input', selector: '#quantity-2', value: 'abc' },
      { op: 'snapshot', name: 'qty-invalid' },
      { op: 'input', selector: '#quantity-2', value: '3' },
      { op: 'html', selector: '#total' },
      { op: 'html', selector: '#status' },
      { op: 'input', selector: '#price-1', value: '-1' },
      { op: 'snapshot', name: 'price-invalid' },
      { op: 'click', selector: '#add-line' },
      { op: 'click', selector: '#toggle' },
      { op: 'snapshot', name: 'hidden' },
      { op: 'click', selector: '#toggle' },
      { op: 'snapshot', name: 'shown' },
      { op: 'click', selector: '#remove-1' },
      { op: 'html', selector: '#total' },
      { op: 'html', selector: '#status' },
    ]);
    assert.equal(r.status, 0, r.stderr);
    const at = (i) => r.results[i].html;

    // initial: two lines, total 2999 cents, ready status
    assert.equal(textOf(at(0), 'total'), 'Total: $29.99');
    assert.equal(textOf(at(0), 'status'), 'Quote ready');
    assert.deepEqual(lineIds(at(0)), ['line-1', 'line-2']);

    // a non-integer quantity is refused with a message and changes nothing
    assert.deepEqual(errorsOf(at(2)), ['Quantity must be a whole number of at least 1.']);
    assert.equal(textOf(at(2), 'total'), 'Total: $29.99');

    // quantity 3 changes the total, and the watcher writes the status line
    assert.equal(at(4), '<p id="total">Total: $39.97</p>');
    assert.equal(at(5), '<p id="status">Total changed from $29.99 to $39.97</p>');

    // a negative price is refused; the error stays until a valid price is typed
    assert.deepEqual(errorsOf(at(7)), ['Price must be zero or more dollars.']);
    assert.equal(textOf(at(7), 'total'), 'Total: $39.97');

    // hiding unmounts the editor and keeps the total; showing again recreates it with the same state
    assert.doesNotMatch(at(10), /id="editor"/);
    assert.equal(textOf(at(10), 'total'), 'Total: $39.97');
    assert.equal(textOf(at(10), 'toggle'), 'Show form');
    assert.match(at(12), /id="editor"/);
    assert.deepEqual(lineIds(at(12)), ['line-1', 'line-2', 'line-3']);
    assert.deepEqual(errorsOf(at(12)), ['Price must be zero or more dollars.']);

    // removing line 1 leaves 1497 cents
    assert.equal(at(14), '<p id="total">Total: $14.97</p>');
    assert.equal(at(15), '<p id="status">Total changed from $39.97 to $14.97</p>');
  });
});

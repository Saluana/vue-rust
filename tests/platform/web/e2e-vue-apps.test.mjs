// Browser delivery regressions: the compiled Rust app, host events, microtask scheduler and real DOM.
// Expected interactions come from the demo specification and arithmetic in cents, independently of the runtime.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { buildProjectWeb, toolEnv } from '../../../tooling/cli/build.mjs';
import { chromeSkipReason, serveDirectory, withPage } from '../../../tooling/chrome/chrome.mjs';

const SKIP = chromeSkipReason() ?? false;
process.env.CARGO_TARGET_DIR = path.join(toolEnv().CARGO_TARGET_DIR, 'vue-app-crates');
const ROOT = path.resolve('examples');
const OUT = path.join(toolEnv().CARGO_TARGET_DIR, 'e2e');

async function appPage(name, run, changeOutput = () => {}) {
  const out = path.join(OUT, name);
  fs.rmSync(out, { recursive: true, force: true });
  const { dir, manifest } = buildProjectWeb({ root: path.join(ROOT, name), entry: 'src/App.vue', out, name });
  changeOutput(dir);
  const site = await serveDirectory(dir);
  try {
    await withPage(async (page) => {
      await page.goto(new URL('index.html', site.url).href);
      await run(page, manifest);
    });
  } finally { await site.close(); }
}

async function click(page, selector, times = 1) {
  await page.evaluate(async (selector, times) => {
    for (let i = 0; i < times; i++) document.querySelector(selector).click();
    await Promise.resolve();
  }, selector, times);
}

async function input(page, selector, value) {
  await page.evaluate(async (selector, value) => {
    const el = document.querySelector(selector);
    el.value = value;
    el.dispatchEvent(new InputEvent('input', { bubbles: true, data: value, inputType: 'insertText' }));
    await Promise.resolve();
  }, selector, value);
}

async function read(page) {
  return page.evaluate(() => ({
    count: document.querySelector('#count')?.textContent,
    doubled: document.querySelector('#doubled')?.textContent,
    resetDisabled: document.querySelector('#reset')?.disabled,
    many: !!document.querySelector('#many'),
    total: document.querySelector('#total')?.textContent,
    status: document.querySelector('#status')?.textContent,
    editor: !!document.querySelector('#editor'),
    customer: document.querySelector('#customer')?.value,
    lines: [...document.querySelectorAll('#lines > li')].map(el => el.id),
    errors: [...document.querySelectorAll('.error')].map(el => el.textContent),
  }));
}

function noErrors(page) {
  assert.deepEqual(page.exceptions, []);
  assert.deepEqual(page.consoleLog.filter(e => e.type === 'error' || e.type === 'warning'), []);
}

test('compiled Counter: browser click, computed, conditional, disabled and reset', { skip: SKIP }, async () => {
  await appPage('counter', async (page, manifest) => {
    const started = await page.evaluate(async () => {
      await window.vnReady;
      return { ms: Math.round(performance.now()), scope: document.querySelector('main').getAttributeNames().some(n => n.startsWith('data-v-')) };
    });
    const initial = await read(page);
    assert.equal(initial.count, 'Count: 0');
    assert.equal(initial.resetDisabled, true);
    assert.equal(started.scope, true);
    await click(page, '#increment', 5);
    assert.equal((await read(page)).count, 'Count: 5');
    assert.equal((await read(page)).many, false);
    await click(page, '#increment');
    const six = await read(page);
    assert.equal(six.count, 'Count: 6');
    assert.equal(six.doubled, 'Double: 12');
    assert.equal(six.many, true);
    await click(page, '#reset');
    const reset = await read(page);
    assert.equal(reset.count, 'Count: 0');
    assert.equal(reset.doubled, 'Double: 0');
    assert.equal(reset.resetDisabled, true);
    assert.equal(reset.many, false);
    noErrors(page);
    console.log(`Counter: ${manifest.bytes} bytes wasm (${manifest.gzipBytes} gzip); first render ${started.ms} ms`);
  });
});

test('compiled Quick Quote: input/models, validation, keyed rows, watcher and unmount/recreate', { skip: SKIP }, async () => {
  await appPage('quick-quote', async (page, manifest) => {
    const ms = await page.evaluate(async () => { await window.vnReady; return Math.round(performance.now()); });
    const initial = await read(page);
    assert.equal(initial.total, 'Total: $29.99');
    assert.equal(initial.status, 'Quote ready');
    assert.deepEqual(initial.lines, ['line-1', 'line-2']);
    await input(page, '#customer', 'Ada 🦀');
    await input(page, '#quantity-2', 'abc');
    assert.deepEqual((await read(page)).errors, ['Quantity must be a whole number of at least 1.']);
    assert.equal((await read(page)).total, 'Total: $29.99');
    await input(page, '#quantity-2', '3');
    assert.equal((await read(page)).total, 'Total: $39.97');
    assert.equal((await read(page)).status, 'Total changed from $29.99 to $39.97');
    await input(page, '#price-1', '-1');
    assert.deepEqual((await read(page)).errors, ['Price must be zero or more dollars.']);
    await click(page, '#add-line');
    await click(page, '#toggle');
    assert.equal((await read(page)).editor, false);
    await click(page, '#toggle');
    const shown = await read(page);
    assert.equal(shown.customer, 'Ada 🦀');
    assert.deepEqual(shown.lines, ['line-1', 'line-2', 'line-3']);
    assert.deepEqual(shown.errors, ['Price must be zero or more dollars.']);
    await click(page, '#remove-1');
    assert.equal((await read(page)).total, 'Total: $14.97');
    assert.equal((await read(page)).status, 'Total changed from $39.97 to $14.97');
    const handles = await page.evaluate(async () => {
      // Repeated unmounts must release browser handles and listener captures, not just hide DOM.
      document.querySelector('#toggle').click();
      await Promise.resolve();
      document.querySelector('#toggle').click();
      await Promise.resolve();
      const before = window.vnHost.stats();
      for (let i = 0; i < 20; i++) {
        document.querySelector('#toggle').click();
        await Promise.resolve();
        document.querySelector('#toggle').click();
        await Promise.resolve();
      }
      return { before, after: window.vnHost.stats() };
    });
    assert.equal(handles.after.liveHandles, handles.before.liveHandles, 'unmount/recreate releases host handles');
    assert.equal(handles.after.listeners, handles.before.listeners, 'unmount/recreate releases listeners');
    noErrors(page);
    console.log(`Quick Quote: ${manifest.bytes} bytes wasm (${manifest.gzipBytes} gzip); first render ${ms} ms`);
  });
});

test('compiled app: missing mount container rejects startup loudly (negative control)', { skip: SKIP }, async () => {
  await appPage('counter', async (page) => {
    const result = await page.evaluate(async () => {
      try { await window.vnReady; return 'started'; }
      catch (err) { return err.message; }
    });
    assert.match(result, /startup failed/);
    assert.ok(page.consoleLog.some(e => e.type === 'error'));
  }, dir => {
    const file = path.join(dir, 'index.html');
    fs.writeFileSync(file, fs.readFileSync(file, 'utf8').replace('<div id="app"></div>', '<div id="missing"></div>'));
  });
});

// Production delivery differs from the headless Vue corpus: real host timers, stylesheet timing and cached browser handles.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { buildProjectWeb, toolEnv } from '../../../tooling/cli/build.mjs';
import { chromeSkipReason, serveDirectory, withPage } from '../../../tooling/chrome/chrome.mjs';

const SKIP = chromeSkipReason() ?? false;
process.env.CARGO_TARGET_DIR = path.join(toolEnv().CARGO_TARGET_DIR, 'advanced-app-crate');
const ROOT = path.resolve('tests/platform/web/fixtures/advanced-components');
const OUT = path.join(toolEnv().CARGO_TARGET_DIR, 'site');

async function waitFor(page, predicate) {
  const result = await page.evaluate(async (source) => {
    const check = new Function(`return (${source})`);
    for (let i = 0; i < 150; i++) {
      if (check()) return true;
      await new Promise(resolve => setTimeout(resolve, 16));
    }
    return false;
  }, predicate);
  assert.equal(result, true, `browser condition timed out: ${predicate}`);
}
async function click(page, selector) {
  await page.evaluate(async (selector) => { document.querySelector(selector).click(); await Promise.resolve(); }, selector);
}
async function site(run) {
  fs.rmSync(OUT, { recursive: true, force: true });
  buildProjectWeb({ root: ROOT, entry: 'App.vue', out: OUT, name: 'advanced-components' });
  const server = await serveDirectory(OUT);
  try {
    await withPage(async page => {
      await page.goto(new URL('index.html', server.url).href);
      try { await page.evaluate(async () => { await window.vnReady; }); }
      catch (err) { throw new Error(`${err.message}: ${JSON.stringify(page.consoleLog)}`, { cause: err }); }
      await run(page);
    });
  } finally { await server.close(); }
}

test('compiled advanced components: real browser CSS timers, cache state, teleport events and resource disposal', { skip: SKIP }, async () => {
  await site(async page => {
    await waitFor(page, 'document.querySelector("#lazy")?.textContent === "Loaded"');
    assert.equal(await page.evaluate(() => document.querySelector('#overlay > #teleported')?.textContent), 'Overlay');
    await click(page, '#teleported');
    assert.equal(await page.evaluate(() => document.querySelector('#phase').textContent), 'teleported');
    await click(page, '#cached');
    await click(page, '#cached');
    await click(page, '#swap-b');
    assert.equal(await page.evaluate(() => !!document.querySelector('#branch-b')), true);
    await click(page, '#swap-a');
    assert.equal(await page.evaluate(() => document.querySelector('#cached').textContent), 'A 2');
    await page.evaluate(() => {
      const style = document.createElement('style');
      // 300 ms is long enough to distinguish the two-frame setup from computed stylesheet duration.
      style.textContent = '.motion-enter-active { transition: opacity 0.3s linear; } .motion-enter-from { opacity: 0; } .motion-enter-to { opacity: 1; }';
      document.head.appendChild(style);
    });
    await click(page, '#toggle-motion');
    await waitFor(page, 'document.querySelector("#motion")?.classList.contains("motion-enter-to")');
    assert.equal(await page.evaluate(() => document.querySelector('#phase').textContent), 'teleported', 'after-enter waits for CSS duration');
    assert.equal(await page.evaluate(() => document.querySelector('#motion').classList.contains('motion-enter-active')), true);
    await waitFor(page, 'document.querySelector("#phase").textContent === "entered"');
    assert.equal(await page.evaluate(() => document.querySelector('#motion').className), '');
    await click(page, '#toggle-motion');
    await waitFor(page, '!document.querySelector("#motion")');
    await click(page, '#toggle-shell');
    await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 220)));
    const before = await page.evaluate(() => window.vnHost.stats());
    for (let i = 0; i < 8; i++) {
      await click(page, '#toggle-shell');
      await waitFor(page, '!!document.querySelector("#cached")');
      await click(page, '#toggle-shell');
    }
    await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 220)));
    const after = await page.evaluate(() => window.vnHost.stats());
    assert.equal(after.liveHandles, before.liveHandles, 'cache teardown releases storage and cached subtrees');
    assert.equal(after.listeners, before.listeners, 'teleport and cached listeners are released');
    assert.equal(after.timers, 0, 'unmounted async wrappers cancel pending timers');
    assert.deepEqual(page.exceptions, []);
    assert.deepEqual(page.consoleLog.filter(e => e.type === 'error' || e.type === 'warning'), []);
  });
});

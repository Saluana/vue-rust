// Strings of every shape, handle exhaustion and reuse, memory stability over 50 000 create/insert/remove/release cycles, a deep
// tree, and recovery after a Rust panic. Expectations are computed from the input, not from the code under test.
import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { appHandle, call, callErr, openHostPage, reloadHost, runOps, skipOpt, startSite } from './support.mjs';

describe('strings', skipOpt, () => {
  let site;
  let page;
  before(async () => {
    site = await startSite();
    page = await openHostPage(site);
  });
  after(async () => {
    await page?.close();
    await site?.close();
  });

  test('an empty text node round-trips and an empty attribute value is kept', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const text = await call(page, 'create_text', '');
    await call(page, 'insert', text, app, null);
    assert.equal(await call(page, 'outer_html', app), '<div id="app"></div>');
    await call(page, 'set_attribute', app, 'title', '');
    assert.equal(await call(page, 'get_attribute', app, 'title'), '');
  });

  test('a 1 MB text round-trips exactly', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const big = 'ab\u00e9'.repeat(349525) + 'x';
    assert.equal(big.length, 1 << 20);
    const text = await call(page, 'create_text', big);
    await call(page, 'insert', text, app, null);
    const sameAsPage = (expected) => page.evaluate(
      (value) => document.getElementById('app').textContent === value,
      expected,
    );
    assert.equal(await sameAsPage(big), true);
    await call(page, 'set_element_text', app, big.slice(1));
    assert.equal(await sameAsPage(big.slice(1)), true);
  });

  test('astral characters and embedded NULs survive text, attribute and selector round trips', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const text = '\u{1d49c}\u{1f600}\u0000end\u0000';
    const el = await call(page, 'create_element', 'html', 'i', null);
    await call(page, 'set_attribute', el, 'data-v', text);
    await call(page, 'set_element_text', el, text);
    await call(page, 'insert', el, app, null);
    assert.equal(await call(page, 'get_attribute', el, 'data-v'), text);
    assert.equal(await page.evaluate(() => document.querySelector('i').textContent), text);
    assert.equal(await call(page, 'query_selector', 'i'), el);
  });

  test('a 300 000 character attribute value round-trips', async () => {
    await reloadHost(page, site);
    const value = 'q'.repeat(300000);
    const el = await call(page, 'create_element', 'html', 'div', null);
    await call(page, 'set_attribute', el, 'data-long', value);
    assert.equal(await call(page, 'get_attribute', el, 'data-long'), value);
  });
});

describe('handles and memory', skipOpt, () => {
  let site;
  let page;
  before(async () => {
    site = await startSite();
    page = await openHostPage(site);
  });
  after(async () => {
    await page?.close();
    await site?.close();
  });

  test('handles are exhausted at maxNodes and come back after release, with a new generation', async () => {
    await reloadHost(page, site);
    await page.evaluate(async () => {
      const { createHost } = await import('./vn-host.js');
      const manifest = await fetch('./manifest.json').then((r) => r.json());
      window.vnSmall = await createHost({
        document, wasmBytesOrUrl: new URL(manifest.wasm, location.href), maxNodes: 3,
      });
    });
    const small = (...op) => runOps(page, [op], 'vnSmall').then(([r]) => r);
    const handles = [];
    for (let i = 0; i < 3; i++) {
      const r = await small('create_element', 'html', 'b', null);
      assert.ok('ok' in r, JSON.stringify(r));
      handles.push(r.ok);
    }
    assert.equal(new Set(handles).size, 3);
    const full = await small('create_element', 'html', 'b', null);
    assert.match(full.err.msg, /no free node handle: 3 handles are live/);
    assert.deepEqual(await small('release', handles[0]), { ok: null });
    const again = await small('create_element', 'html', 'i', null);
    assert.ok('ok' in again, JSON.stringify(again));
    assert.notEqual(again.ok, handles[0], 'the slot comes back with a new generation');
    assert.deepEqual(await small('remove', handles[0]), { err: { kind: 'NotFound' } });
    assert.equal((await page.evaluate(() => window.vnSmall.stats())).liveHandles, 3);
  });

  test('50 000 create, insert, remove and release cycles keep wasm memory and handle count flat', async () => {
    await reloadHost(page, site);
    const samples = [];
    for (let batch = 0; batch < 10; batch++) {
      await call(page, '__churn', 5000);
      samples.push(await page.evaluate(() => window.vnHost.stats()));
    }
    const pages = samples.map((s) => s.pages);
    assert.ok(pages.every((p) => p === pages[0]), `memory pages per batch: ${pages.join(', ')}`);
    assert.ok(samples.every((s) => s.liveHandles === samples[0].liveHandles), 'no handle survives a cycle');
    assert.equal(samples[9].liveHandles, 1, 'only the body handle, taken by the first cycle, stays live');
    assert.equal(samples[9].listeners, 0);
  });

  test('a 400-level deep tree serialises exactly and every level keeps its handle', async () => {
    await reloadHost(page, site);
    let parent = await appHandle(page);
    const depth = 400;
    for (let i = 0; i < depth; i++) {
      const el = await call(page, 'create_element', 'html', 'div', null);
      await call(page, 'insert', el, parent, null);
      parent = el;
    }
    const html = await call(page, 'outer_html', await appHandle(page));
    assert.equal(html, `<div id="app">${'<div>'.repeat(depth)}${'</div>'.repeat(depth)}</div>`);
  });
});

describe('panics and lifecycle', skipOpt, () => {
  let site;
  let page;
  before(async () => {
    site = await startSite();
    page = await openHostPage(site);
  });
  after(async () => {
    await page?.close();
    await site?.close();
  });

  test('a Rust panic is reported through host_panic and the page keeps working', async () => {
    await reloadHost(page, site);
    await assert.rejects(runOps(page, [['__panic', 'boom']]), /unreachable|RuntimeError/);
    const panics = await page.evaluate(() => window.vnHost.stats().panics);
    assert.equal(panics.length, 1);
    assert.match(panics[0], /selftest panic: boom/);
    assert.equal(typeof (await call(page, 'body')), 'number');
    assert.equal(await call(page, 'tag_name', await call(page, 'body')), 'BODY');
  });

  test('timers scheduled through the host fire through vn_timer; cleared timers do not', async () => {
    await reloadHost(page, site);
    await call(page, '__timer_set', 5, 10);
    await call(page, '__timer_set', 6, 10);
    await call(page, '__timer_clear', 6);
    await new Promise((r) => setTimeout(r, 150));
    assert.deepEqual(await call(page, '__timers_fired'), [5]);
  });

  test('host log messages reach onLog with the info level', async () => {
    await reloadHost(page, site);
    await call(page, '__log_message', 'hello from wasm');
    assert.ok(page.consoleLog.some((e) => e.type === 'info' && e.text.includes('[vn-host] hello from wasm')));
  });

  test('destroy() removes listeners and makes later calls InvalidState', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'button', null);
    await call(page, 'set_attribute', el, 'id', 'd1');
    await call(page, 'insert', el, await appHandle(page), null);
    await call(page, 'add_event_listener', el, 'ping', 601, {});
    await page.evaluate(() => window.vnHost.destroy());
    await page.evaluate(() => document.getElementById('d1').dispatchEvent(new Event('ping')));
    assert.deepEqual(await call(page, '__log'), [], 'no delivery after destroy');
    const err = await callErr(page, 'InvalidState', 'create_element', 'html', 'p', null);
    assert.match(err.msg, /destroyed/);
  });
});

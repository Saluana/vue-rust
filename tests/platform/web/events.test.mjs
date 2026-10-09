// Events in both directions: listeners registered through the Host trait, invoked by the browser, the outcome applied to the real
// event, and re-entrant host calls made from inside a listener. Orders and flags are compared with native listeners in the same
// page (the browser is the oracle).
import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { appHandle, call, openHostPage, reloadHost, skipOpt, startSite } from './support.mjs';

const onlyEvents = (log) => log.filter((entry) => entry.kind === 'event');

async function mount(page, tag, id, parent) {
  const el = await call(page, 'create_element', 'html', tag, null);
  await call(page, 'set_attribute', el, 'id', id);
  await call(page, 'insert', el, parent ?? (await appHandle(page)), null);
  return el;
}

const listen = (page, el, type, id, opts = {}) => call(page, 'add_event_listener', el, type, id, opts);

// Sets what listener `id` does when it runs: queued host operations and the outcome flags.
const behave = (page, id, spec) => call(page, '__behave', id, spec);

// Dispatches a plain Event from the page on the element with the given id; returns dispatchEvent's result.
const fire = (page, id, type, init = {}) => page.evaluate(
  ([elementId, eventType, eventInit]) => document.getElementById(elementId).dispatchEvent(new Event(eventType, eventInit)),
  [id, type, init],
);

// Dispatches an event built by a named constructor (KeyboardEvent, MouseEvent, InputEvent) on the element with the given id.
const fireWith = (page, id, ctor, type, init) => page.evaluate(
  ([elementId, name, eventType, eventInit]) => document.getElementById(elementId).dispatchEvent(new window[name](eventType, eventInit)),
  [id, ctor, type, init],
);

describe('events: delivery and options', skipOpt, () => {
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

  test('a dispatch from the page reaches the listener with its id, type, target, current target and phase', async () => {
    await reloadHost(page, site);
    const el = await mount(page, 'button', 'e1');
    await listen(page, el, 'ping', 101);
    await fire(page, 'e1', 'ping', { bubbles: true });
    const [ev] = onlyEvents(await call(page, '__log'));
    assert.equal(ev.listener, 101);
    assert.equal(ev.ty, 'ping');
    assert.equal(ev.target, el);
    assert.equal(ev.current, el);
    assert.equal(ev.phase, 2, 'at target');
    assert.equal(ev.bubbles, true);
    assert.equal(ev.cancelable, false);
  });

  test('a once listener runs once', async () => {
    await reloadHost(page, site);
    const el = await mount(page, 'button', 'e2');
    await listen(page, el, 'ping', 102, { once: true });
    await fire(page, 'e2', 'ping');
    await fire(page, 'e2', 'ping');
    assert.deepEqual(onlyEvents(await call(page, '__log')).map((e) => e.listener), [102]);
  });

  test('capture and bubble order across three nested elements matches native listeners', async () => {
    await reloadHost(page, site);
    const a = await mount(page, 'div', 'ea');
    const b = await call(page, 'create_element', 'html', 'div', null);
    const c = await call(page, 'create_element', 'html', 'div', null);
    await call(page, 'set_attribute', c, 'id', 'ec');
    await call(page, 'insert', b, a, null);
    await call(page, 'insert', c, b, null);
    const labels = { 201: 'A-cap', 202: 'A-bub', 203: 'B-cap', 204: 'B-bub', 205: 'C-cap', 206: 'C-bub' };
    await listen(page, a, 'ping', 201, { capture: true });
    await listen(page, a, 'ping', 202);
    await listen(page, b, 'ping', 203, { capture: true });
    await listen(page, b, 'ping', 204);
    await listen(page, c, 'ping', 205, { capture: true });
    await listen(page, c, 'ping', 206);
    await fire(page, 'ec', 'ping', { bubbles: true });
    const host = onlyEvents(await call(page, '__log')).map((e) => labels[e.listener]);
    const oracle = await page.evaluate(() => {
      const root = document.createElement('div');
      document.body.append(root);
      const a2 = document.createElement('div');
      const b2 = document.createElement('div');
      const c2 = document.createElement('div');
      root.append(a2);
      a2.append(b2);
      b2.append(c2);
      const seen = [];
      a2.addEventListener('ping', () => seen.push('A-cap'), true);
      a2.addEventListener('ping', () => seen.push('A-bub'));
      b2.addEventListener('ping', () => seen.push('B-cap'), true);
      b2.addEventListener('ping', () => seen.push('B-bub'));
      c2.addEventListener('ping', () => seen.push('C-cap'), true);
      c2.addEventListener('ping', () => seen.push('C-bub'));
      c2.dispatchEvent(new Event('ping', { bubbles: true }));
      root.remove();
      return seen;
    });
    assert.deepEqual(host, oracle);
    assert.deepEqual(host, ['A-cap', 'B-cap', 'C-cap', 'C-bub', 'B-bub', 'A-bub']);
  });

  test('target and currentTarget are the handles of the node the event is at, in each phase', async () => {
    await reloadHost(page, site);
    const outer = await mount(page, 'div', 'to');
    const inner = await call(page, 'create_element', 'html', 'span', null);
    await call(page, 'set_attribute', inner, 'id', 'ti');
    await call(page, 'insert', inner, outer, null);
    await listen(page, outer, 'ping', 381, { capture: true });
    await listen(page, outer, 'ping', 382);
    await listen(page, inner, 'ping', 383);
    await fire(page, 'ti', 'ping', { bubbles: true });
    const seen = onlyEvents(await call(page, '__log')).map((e) => [e.listener, e.target, e.current, e.phase]);
    assert.deepEqual(seen, [[381, inner, outer, 1], [383, inner, inner, 2], [382, inner, outer, 3]]);
  });

  test('a listener registered twice with the same id and capture runs once (addEventListener dedupes)', async () => {
    await reloadHost(page, site);
    const el = await mount(page, 'button', 'e3');
    await listen(page, el, 'ping', 371);
    await listen(page, el, 'ping', 371);
    await fire(page, 'e3', 'ping');
    assert.equal(onlyEvents(await call(page, '__log')).length, 1);
  });

  test('remove_event_listener needs the same capture flag; an unknown listener is a no-op', async () => {
    await reloadHost(page, site);
    const el = await mount(page, 'button', 'e4');
    await listen(page, el, 'ping', 361);
    await call(page, 'remove_event_listener', el, 'ping', 361, true);
    await fire(page, 'e4', 'ping');
    assert.deepEqual(onlyEvents(await call(page, '__log')).map((e) => e.listener), [361]);
    await call(page, 'remove_event_listener', el, 'ping', 361, false);
    await fire(page, 'e4', 'ping');
    assert.deepEqual(onlyEvents(await call(page, '__log')), []);
    await call(page, 'remove_event_listener', el, 'nope', 999, false);
  });

  test('a passive listener cannot prevent the default action', async () => {
    await reloadHost(page, site);
    const el = await mount(page, 'button', 'e5');
    await behave(page, 331, { prevent: true });
    await listen(page, el, 'ping', 331, { passive: true });
    const notCancelled = await fire(page, 'e5', 'ping', { cancelable: true });
    assert.equal(notCancelled, true, 'dispatchEvent reports the event as not prevented');
    assert.equal(onlyEvents(await call(page, '__log'))[0].default_prevented, false);
  });

  test('preventDefault from Rust makes a cancelable dispatch report false, and later listeners see it', async () => {
    await reloadHost(page, site);
    const el = await mount(page, 'button', 'e6');
    await behave(page, 321, { prevent: true });
    await listen(page, el, 'ping', 321);
    await listen(page, el, 'ping', 322);
    const result = await fire(page, 'e6', 'ping', { cancelable: true });
    assert.equal(result, false);
    const events = onlyEvents(await call(page, '__log'));
    assert.deepEqual(events.map((e) => [e.listener, e.default_prevented]), [[321, false], [322, true]]);
  });

  test('preventDefault has no effect on a non-cancelable event', async () => {
    await reloadHost(page, site);
    const el = await mount(page, 'button', 'e7');
    await behave(page, 341, { prevent: true });
    await listen(page, el, 'ping', 341);
    assert.equal(await fire(page, 'e7', 'ping', { cancelable: false }), true);
    assert.equal(onlyEvents(await call(page, '__log'))[0].default_prevented, false);
  });

  test('stopPropagation from Rust stops the bubble before the outer element', async () => {
    await reloadHost(page, site);
    const outer = await mount(page, 'div', 'so');
    const inner = await call(page, 'create_element', 'html', 'span', null);
    await call(page, 'set_attribute', inner, 'id', 'si');
    await call(page, 'insert', inner, outer, null);
    await behave(page, 301, { stop: true });
    await listen(page, inner, 'ping', 303);
    await listen(page, outer, 'ping', 302);
    await listen(page, inner, 'ping', 301);
    await fire(page, 'si', 'ping', { bubbles: true });
    assert.deepEqual(onlyEvents(await call(page, '__log')).map((e) => e.listener), [303, 301]);
  });

  test('stopImmediatePropagation skips the rest of the element and the bubble', async () => {
    await reloadHost(page, site);
    const parent = await mount(page, 'div', 'sp');
    const el = await call(page, 'create_element', 'html', 'span', null);
    await call(page, 'set_attribute', el, 'id', 'sc');
    await call(page, 'insert', el, parent, null);
    await behave(page, 311, { stop_immediate: true });
    await listen(page, el, 'ping', 311);
    await listen(page, el, 'ping', 312);
    await listen(page, parent, 'ping', 313);
    await fire(page, 'sc', 'ping', { bubbles: true });
    assert.deepEqual(onlyEvents(await call(page, '__log')).map((e) => e.listener), [311]);
  });

  test('a listener that throws is reported and the dispatch continues', async () => {
    await reloadHost(page, site);
    const el = await mount(page, 'button', 'e8');
    await behave(page, 351, { throw: true });
    await listen(page, el, 'ping', 351);
    await listen(page, el, 'ping', 352);
    await fire(page, 'e8', 'ping');
    assert.deepEqual(onlyEvents(await call(page, '__log')).map((e) => e.listener), [351, 352]);
    assert.ok(
      page.consoleLog.some((entry) => entry.type === 'error' && /listener 351 threw/.test(entry.text)),
      'the throw is reported through the host log',
    );
  });
});

describe('events: data fidelity', skipOpt, () => {
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

  test('keyboard fields: key, code, modifiers and the cancelable and bubbles flags', async () => {
    await reloadHost(page, site);
    const el = await mount(page, 'input', 'k1');
    await listen(page, el, 'keydown', 501);
    await fireWith(page, 'k1', 'KeyboardEvent', 'keydown', {
      key: 'é', code: 'KeyE', ctrlKey: true, shiftKey: true, bubbles: true, cancelable: true,
    });
    const [ev] = onlyEvents(await call(page, '__log'));
    assert.equal(ev.key, 'é');
    assert.equal(ev.code, 'KeyE');
    assert.deepEqual([ev.ctrl, ev.shift, ev.alt, ev.meta], [true, true, false, false]);
    assert.deepEqual([ev.bubbles, ev.cancelable, ev.composed], [true, true, false]);
    assert.equal(ev.is_composing, false);
    assert.equal(ev.input_data, null);
  });

  test('astral characters in key arrive intact', async () => {
    await reloadHost(page, site);
    const el = await mount(page, 'input', 'k2');
    await listen(page, el, 'keydown', 502);
    await fireWith(page, 'k2', 'KeyboardEvent', 'keydown', { key: '😀', code: '' });
    assert.equal(onlyEvents(await call(page, '__log'))[0].key, '😀');
  });

  test('mouse fields: button, detail and client coordinates', async () => {
    await reloadHost(page, site);
    const el = await mount(page, 'button', 'm1');
    await listen(page, el, 'click', 511);
    await fireWith(page, 'm1', 'MouseEvent', 'click', { button: 2, detail: 3, clientX: 10.5, clientY: -4, bubbles: true });
    const [ev] = onlyEvents(await call(page, '__log'));
    // The browser stores clientX as an integer (10.5 becomes 10); the oracle is the page's own MouseEvent.
    const oracle = await page.evaluate(() => {
      const m = new MouseEvent('click', { button: 2, detail: 3, clientX: 10.5, clientY: -4 });
      return [m.button, m.detail, m.clientX, m.clientY];
    });
    assert.deepEqual([ev.button, ev.detail, ev.client_x, ev.client_y], oracle);
    assert.deepEqual(oracle, [2, 3, 10, -4]);
  });

  test('composition data and isComposing reach the listener; absent data is null', async () => {
    await reloadHost(page, site);
    const el = await mount(page, 'input', 'c1');
    await listen(page, el, 'input', 521);
    await fireWith(page, 'c1', 'InputEvent', 'input', { data: 'あ', isComposing: true, bubbles: true });
    await fireWith(page, 'c1', 'InputEvent', 'input', { bubbles: true });
    const [withData, withoutData] = onlyEvents(await call(page, '__log'));
    assert.equal(withData.input_data, 'あ');
    assert.equal(withData.is_composing, true);
    assert.equal(withoutData.input_data, null);
    assert.equal(withoutData.is_composing, false);
  });

  test('time stamps equal the timeStamp the page sees, and are positive', async () => {
    await reloadHost(page, site);
    const el = await mount(page, 'button', 't1');
    await listen(page, el, 'ping', 531);
    await page.evaluate(() => {
      const target = document.getElementById('t1');
      target.addEventListener('ping', (ev) => {
        window.__seen = ev.timeStamp;
      });
      target.dispatchEvent(new Event('ping'));
    });
    const [ev] = onlyEvents(await call(page, '__log'));
    assert.equal(ev.time_stamp, await page.evaluate(() => window.__seen));
    assert.ok(ev.time_stamp > 0);
  });

  test('a released target is reported as no handle (documented limit)', async () => {
    await reloadHost(page, site);
    const el = await mount(page, 'button', 'rel');
    await listen(page, el, 'ping', 541);
    await call(page, 'release', el);
    await fire(page, 'rel', 'ping', { bubbles: true });
    const [ev] = onlyEvents(await call(page, '__log'));
    assert.equal(ev.target, null);
    assert.equal(ev.current, null);
  });
});

describe('events: focus order and re-entrancy', skipOpt, () => {
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

  test('focus from one input to another fires blur, focusout, focus, focusin in the browser order', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const a = await call(page, 'create_element', 'html', 'input', null);
    const b = await call(page, 'create_element', 'html', 'input', null);
    await call(page, 'set_attribute', a, 'id', 'fa');
    await call(page, 'set_attribute', b, 'id', 'fb');
    await call(page, 'insert', a, app, null);
    await call(page, 'insert', b, app, null);
    const labels = { 391: 'focus@a', 392: 'blur@a', 393: 'focusin@a', 394: 'focusout@a', 395: 'focus@b', 396: 'blur@b', 397: 'focusin@b', 398: 'focusout@b' };
    for (const [el, base] of [[a, 391], [b, 395]]) {
      await listen(page, el, 'focus', base);
      await listen(page, el, 'blur', base + 1);
      await listen(page, el, 'focusin', base + 2);
      await listen(page, el, 'focusout', base + 3);
    }
    await call(page, 'focus', a);
    await call(page, 'focus', b);
    const host = onlyEvents(await call(page, '__log')).map((e) => labels[e.listener]);
    const oracle = await page.evaluate(() => {
      const seen = [];
      const x = document.createElement('input');
      const y = document.createElement('input');
      document.getElementById('app').append(x, y);
      const names = { x: 'a', y: 'b' };
      for (const [el, id] of [[x, 'x'], [y, 'y']]) {
        for (const type of ['focus', 'blur', 'focusin', 'focusout']) {
          el.addEventListener(type, () => seen.push(`${type}@${names[id]}`));
        }
      }
      x.focus();
      y.focus();
      const sequence = seen.slice();
      x.remove();
      y.remove();
      return sequence;
    });
    assert.deepEqual(host, oracle);
    assert.deepEqual(host, ['focus@a', 'focusin@a', 'blur@a', 'focusout@a', 'focus@b', 'focusin@b']);
  });

  test('blur fires during remove, while the node is attached; the listener may call back into the host', async () => {
    await reloadHost(page, site);
    const input = await mount(page, 'input', 'fx');
    await call(page, 'focus', input);
    await behave(page, 401, {
      actions: [
        ['set_attribute', input, 'data-blurred', '1'],
        ['query_selector', '#fx'],
        ['active_element'],
      ],
    });
    await listen(page, input, 'blur', 401);
    assert.equal(await call(page, 'active_element'), input);
    await call(page, 'remove', input);
    const log = await call(page, '__log');
    const [event, setAttr, found, active] = log;
    assert.equal(event.kind, 'event');
    assert.equal(event.ty, 'blur');
    assert.deepEqual(setAttr.result, { ok: null }, 'the nested write is applied');
    assert.deepEqual(found.result, { ok: input }, 'the node was still attached during blur');
    // Focus has already moved away from the node while blur runs: the browser reports the body, which is None to the host.
    const nativeDuringBlur = await page.evaluate(() => {
      const probe = document.createElement('input');
      document.getElementById('app').append(probe);
      let during = null;
      probe.addEventListener('blur', () => {
        during = document.activeElement === document.body;
      });
      probe.focus();
      probe.remove();
      return during;
    });
    assert.equal(nativeDuringBlur, true);
    assert.deepEqual(active.result, { ok: null }, 'the host reports no focused element during blur, as the browser does');
    assert.equal(await page.evaluate(() => document.getElementById('fx')), null, 'the node is detached afterwards');
    assert.equal(await call(page, 'active_element'), null);
    assert.equal(await call(page, 'get_attribute', input, 'data-blurred'), '1');
  });

  test('a listener removes the node being inserted: a custom element dispatches during insert', async () => {
    await reloadHost(page, site);
    // Defined after each reload: a reload starts a fresh custom-element registry.
    await page.evaluate(() => {
      customElements.define('x-probe', class extends HTMLElement {
        connectedCallback() {
          this.dispatchEvent(new Event('vn-connected'));
        }
      });
    });
    const app = await appHandle(page);
    const probe = await call(page, 'create_element', 'html', 'x-probe', null);
    await call(page, 'set_attribute', probe, 'id', 'probe');
    await listen(page, probe, 'vn-connected', 411);
    await behave(page, 411, { actions: [['remove', probe]] });
    await call(page, 'insert', probe, app, null);
    assert.equal(await page.evaluate(() => document.getElementById('probe')), null, 'removed again inside insert');
    const [event, action] = await call(page, '__log');
    assert.equal(event.ty, 'vn-connected');
    assert.equal(event.target, probe);
    assert.deepEqual(action.result, { ok: null });
  });

  test('a listener inserts another node while the page dispatches to its parent', async () => {
    await reloadHost(page, site);
    const parent = await mount(page, 'section', 'pp');
    const child = await call(page, 'create_element', 'html', 'b', null);
    await call(page, 'set_attribute', child, 'id', 'pc');
    await listen(page, parent, 'ping', 421);
    await behave(page, 421, { actions: [['insert', child, parent, null]] });
    await fire(page, 'pp', 'ping');
    assert.equal(await page.evaluate(() => document.querySelector('#pp > #pc') !== null), true);
  });
});

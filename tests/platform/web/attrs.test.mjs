// Attributes (set/remove/get/has, NS and case rules, policy) and properties (has/get/set of the five value kinds, DOM
// coercion, value/checked/selected/disabled semantics). Expectations are hand-written from the DOM specification.
import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { appHandle, appHtml, call, callErr, openHostPage, reloadHost, skipOpt, startSite } from './support.mjs';

const XLINK = 'http://www.w3.org/1999/xlink';

describe('attributes', skipOpt, () => {
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

  test('HTML attribute names are lower-cased by set, get and has', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'div', null);
    await call(page, 'insert', el, await appHandle(page), null);
    await call(page, 'set_attribute', el, 'DATA-X', 'Y');
    assert.equal(await call(page, 'get_attribute', el, 'data-x'), 'Y');
    assert.equal(await call(page, 'get_attribute', el, 'DATA-X'), 'Y');
    assert.equal(await call(page, 'has_attribute', el, 'Data-X'), true);
    assert.equal(await appHtml(page), '<div data-x="Y"></div>');
  });

  test('SVG attribute names keep their case', async () => {
    await reloadHost(page, site);
    const svg = await call(page, 'create_element', 'svg', 'svg', null);
    await call(page, 'insert', svg, await appHandle(page), null);
    await call(page, 'set_attribute', svg, 'viewBox', '0 0 10 10');
    assert.equal(await call(page, 'get_attribute', svg, 'viewbox'), null);
    assert.equal(await call(page, 'get_attribute', svg, 'viewBox'), '0 0 10 10');
    assert.equal(await appHtml(page), '<svg viewBox="0 0 10 10"></svg>');
  });

  test('set_attribute replaces the value; remove_attribute deletes; removing again is not an error', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'p', null);
    await call(page, 'set_attribute', el, 'title', 'one');
    await call(page, 'set_attribute', el, 'title', 'two');
    assert.equal(await call(page, 'get_attribute', el, 'title'), 'two');
    await call(page, 'remove_attribute', el, 'title');
    assert.equal(await call(page, 'get_attribute', el, 'title'), null);
    assert.equal(await call(page, 'has_attribute', el, 'title'), false);
    await call(page, 'remove_attribute', el, 'title');
  });

  test('an attribute name the DOM rejects is InvalidState', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'p', null);
    const err = await callErr(page, 'InvalidState', 'set_attribute', el, 'bad name', 'x');
    assert.match(err.msg, /InvalidCharacterError/);
  });

  test('namespaced attributes: set, read with getAttributeNS, remove', async () => {
    await reloadHost(page, site);
    const svg = await call(page, 'create_element', 'svg', 'use', null);
    await call(page, 'insert', svg, await appHandle(page), null);
    await call(page, 'set_attribute_ns', svg, XLINK, 'xlink:href', '#target');
    const read = await page.evaluate((ns) => document.querySelector('use').getAttributeNS(ns, 'href'), XLINK);
    assert.equal(read, '#target');
    assert.equal(await call(page, 'has_attribute', svg, 'xlink:href'), true);
    await call(page, 'remove_attribute_ns', svg, XLINK, 'href');
    assert.equal(await appHtml(page), '<use></use>');
  });

  test('a null namespace sets a plain attribute; a mismatched prefix is InvalidState', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'div', null);
    await call(page, 'set_attribute_ns', el, null, 'data-k', 'v');
    assert.equal(await call(page, 'get_attribute', el, 'data-k'), 'v');
    const err = await callErr(page, 'InvalidState', 'set_attribute_ns', el, 'http://example.com/other', 'xml:lang', 'en');
    assert.match(err.msg, /NamespaceError/);
  });

  test('event-handler attributes are Unsupported and nothing is set', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'button', null);
    for (const name of ['onclick', 'OnLoad', 'onmouseover']) {
      const err = await callErr(page, 'Unsupported', 'set_attribute', el, name, 'alert(1)');
      assert.match(err.msg, /use add_event_listener/);
    }
    await callErr(page, 'Unsupported', 'set_attribute_ns', el, null, 'onclick', 'alert(1)');
    assert.equal(await call(page, 'has_attribute', el, 'onclick'), false);
  });

  test('javascript: URLs are Unsupported in URL attributes, whatever the case and leading spaces', async () => {
    await reloadHost(page, site);
    const a = await call(page, 'create_element', 'html', 'a', null);
    await callErr(page, 'Unsupported', 'set_attribute', a, 'href', '  JavaScript:alert(1)');
    await callErr(page, 'Unsupported', 'set_attribute_ns', a, XLINK, 'xlink:href', 'javascript:void(0)');
    await call(page, 'set_attribute', a, 'href', 'https://example.com/?q=javascript:x');
    assert.equal(await call(page, 'get_attribute', a, 'href'), 'https://example.com/?q=javascript:x');
  });

  test('srcdoc is Unsupported; an attribute value may hold any text', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'div', null);
    await callErr(page, 'Unsupported', 'set_attribute', el, 'srcdoc', '<p>x</p>');
    const text = 'line1\nline2 & "quotes" <tags> ünïcode 😀';
    await call(page, 'set_attribute', el, 'title', text);
    assert.equal(await call(page, 'get_attribute', el, 'title'), text);
  });

  test('get_attribute and has_attribute on a text node report no attributes (the glue logs a warning)', async () => {
    await reloadHost(page, site);
    const text = await call(page, 'create_text', 'x');
    assert.equal(await call(page, 'get_attribute', text, 'id'), null);
    assert.equal(await call(page, 'has_attribute', text, 'id'), false);
  });
});

describe('properties', skipOpt, () => {
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

  test('has_property answers like `name in el`', async () => {
    await reloadHost(page, site);
    const div = await call(page, 'create_element', 'html', 'div', null);
    assert.equal(await call(page, 'has_property', div, 'id'), true);
    assert.equal(await call(page, 'has_property', div, 'nodeName'), true);
    assert.equal(await call(page, 'has_property', div, 'noSuchThing'), false);
    assert.equal(await call(page, 'has_property', div, 'value'), false);
  });

  test('get_property returns the five value kinds: string, number, boolean, null and undefined', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'input', null);
    await call(page, 'set_attribute', el, 'id', 'field');
    assert.deepEqual(await call(page, 'get_property', el, 'id'), { k: 'str', v: 'field' });
    assert.deepEqual(await call(page, 'get_property', el, 'tabIndex'), { k: 'num', v: 0 });
    assert.deepEqual(await call(page, 'get_property', el, 'disabled'), { k: 'bool', v: false });
    assert.deepEqual(await call(page, 'get_property', el, 'nodeValue'), { k: 'null' });
    assert.deepEqual(await call(page, 'get_property', el, 'noSuchProperty'), { k: 'undef' });
  });

  test('a detached element has a null parentNode, an attached one an object (Unsupported)', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'div', null);
    assert.deepEqual(await call(page, 'get_property', el, 'parentNode'), { k: 'null' });
    await call(page, 'insert', el, await appHandle(page), null);
    const err = await callErr(page, 'Unsupported', 'get_property', el, 'parentNode');
    assert.match(err.msg, /object/);
    await callErr(page, 'Unsupported', 'get_property', el, 'style');
    await callErr(page, 'Unsupported', 'get_property', el, 'focus');
  });

  test('set_property converts strings, numbers and booleans as the DOM does', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'div', null);
    await call(page, 'set_property', el, 'id', { k: 'str', v: 'main' });
    assert.equal(await call(page, 'get_attribute', el, 'id'), 'main');
    await call(page, 'set_property', el, 'tabIndex', { k: 'num', v: 3 });
    assert.equal(await call(page, 'get_attribute', el, 'tabindex'), '3');
    await call(page, 'set_property', el, 'tabIndex', { k: 'num', v: 'NaN' });
    assert.equal(await call(page, 'get_attribute', el, 'tabindex'), '0', 'NaN converts to 0 (WebIDL long)');
    await call(page, 'set_property', el, 'hidden', { k: 'bool', v: true });
    assert.equal(await call(page, 'get_attribute', el, 'hidden'), '');
    await call(page, 'set_property', el, 'hidden', { k: 'bool', v: false });
    assert.equal(await call(page, 'has_attribute', el, 'hidden'), false);
  });

  test('undefined and null convert the way `el[name] = v` does (undefined is "undefined", null is "null")', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'div', null);
    await call(page, 'set_property', el, 'title', { k: 'undef' });
    assert.equal(await call(page, 'get_attribute', el, 'title'), 'undefined');
    await call(page, 'set_property', el, 'title', { k: 'null' });
    assert.equal(await call(page, 'get_attribute', el, 'title'), 'null');
  });

  test('input.value uses LegacyNullToEmptyString and does not reflect to the attribute', async () => {
    await reloadHost(page, site);
    const input = await call(page, 'create_element', 'html', 'input', null);
    await call(page, 'insert', input, await appHandle(page), null);
    await call(page, 'set_property', input, 'value', { k: 'str', v: 'typed' });
    assert.deepEqual(await call(page, 'get_property', input, 'value'), { k: 'str', v: 'typed' });
    assert.equal(await call(page, 'get_attribute', input, 'value'), null, 'the property does not set the attribute');
    await call(page, 'set_property', input, 'value', { k: 'null' });
    assert.deepEqual(await call(page, 'get_property', input, 'value'), { k: 'str', v: '' });
  });

  test('checked and selected are properties; defaultChecked is the attribute', async () => {
    await reloadHost(page, site);
    const box = await call(page, 'create_element', 'html', 'input', null);
    await call(page, 'set_attribute', box, 'type', 'checkbox');
    await call(page, 'set_property', box, 'checked', { k: 'bool', v: true });
    assert.deepEqual(await call(page, 'get_property', box, 'checked'), { k: 'bool', v: true });
    assert.equal(await call(page, 'get_attribute', box, 'checked'), null);
    assert.deepEqual(await call(page, 'get_property', box, 'defaultChecked'), { k: 'bool', v: false });
    const select = await call(page, 'create_element', 'html', 'select', null);
    const first = await call(page, 'create_element', 'html', 'option', null);
    const second = await call(page, 'create_element', 'html', 'option', null);
    await call(page, 'insert', first, select, null);
    await call(page, 'insert', second, select, null);
    await call(page, 'set_property', second, 'selected', { k: 'bool', v: true });
    assert.deepEqual(await call(page, 'get_property', second, 'selected'), { k: 'bool', v: true });
    assert.deepEqual(await call(page, 'get_property', select, 'selectedIndex'), { k: 'num', v: 1 });
  });

  test('disabled reflects to the attribute; a textarea value reads back its text', async () => {
    await reloadHost(page, site);
    const button = await call(page, 'create_element', 'html', 'button', null);
    await call(page, 'set_property', button, 'disabled', { k: 'bool', v: true });
    assert.equal(await call(page, 'get_attribute', button, 'disabled'), '');
    assert.deepEqual(await call(page, 'get_property', button, 'disabled'), { k: 'bool', v: true });
    const area = await call(page, 'create_element', 'html', 'textarea', null);
    await call(page, 'set_element_text', area, 'draft');
    assert.deepEqual(await call(page, 'get_property', area, 'value'), { k: 'str', v: 'draft' });
  });

  test('a property the node does not have is Unsupported (no expando is created)', async () => {
    await reloadHost(page, site);
    const div = await call(page, 'create_element', 'html', 'div', null);
    const err = await callErr(page, 'Unsupported', 'set_property', div, 'value', { k: 'str', v: 'x' });
    assert.match(err.msg, /not a property of this node/);
    assert.deepEqual(await call(page, 'get_property', div, 'value'), { k: 'undef' });
  });

  test('innerHTML and outerHTML cannot be set; their getters work', async () => {
    await reloadHost(page, site);
    const div = await call(page, 'create_element', 'html', 'div', null);
    await call(page, 'set_element_text', div, 'x');
    await callErr(page, 'Unsupported', 'set_property', div, 'innerHTML', { k: 'str', v: '<b>no</b>' });
    await callErr(page, 'Unsupported', 'set_property', div, 'outerHTML', { k: 'str', v: '<b>no</b>' });
    assert.deepEqual(await call(page, 'get_property', div, 'innerHTML'), { k: 'str', v: 'x' });
  });

  test('on* and javascript: URL properties are Unsupported', async () => {
    await reloadHost(page, site);
    const a = await call(page, 'create_element', 'html', 'a', null);
    await callErr(page, 'Unsupported', 'set_property', a, 'onclick', { k: 'str', v: 'alert(1)' });
    await callErr(page, 'Unsupported', 'set_property', a, 'onclick', { k: 'null' });
    await callErr(page, 'Unsupported', 'set_property', a, 'href', { k: 'str', v: 'javascript:alert(1)' });
  });

  test('strings round-trip unchanged through set_property and get_property, including NUL and astral characters', async () => {
    await reloadHost(page, site);
    const div = await call(page, 'create_element', 'html', 'div', null);
    const text = 'é 😀 \u0000 end';
    await call(page, 'set_property', div, 'title', { k: 'str', v: text });
    assert.deepEqual(await call(page, 'get_property', div, 'title'), { k: 'str', v: text });
  });

  test('a successful property write is visible in the DOM the page holds', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const el = await call(page, 'create_element', 'html', 'span', null);
    await call(page, 'insert', el, app, null);
    await call(page, 'set_property', el, 'className', { k: 'str', v: 'a b' });
    assert.equal(await page.evaluate(() => document.querySelector('#app > span').className), 'a b');
    assert.equal(await appHtml(page), '<span class="a b"></span>');
  });
});

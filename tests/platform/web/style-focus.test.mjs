// Style (setProperty, cssText), classList, focus and the selector subset. CSS serialisation is compared with a second element
// the page creates itself (the browser is the oracle); other expectations are hand-written.
import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { appHandle, appHtml, call, callErr, openHostPage, reloadHost, skipOpt, startSite } from './support.mjs';

// What the browser serialises for the same assignment on a fresh element.
const browserCss = (page, css) => page.evaluate((text) => {
  const el = document.createElement('div');
  el.style.cssText = text;
  return el.style.cssText;
}, css);

describe('style', skipOpt, () => {
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

  test('setProperty and getPropertyValue round-trip; cssText shows the declaration', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'div', null);
    await call(page, 'style_set_property', el, 'color', 'red', false);
    assert.equal(await call(page, 'style_get_property', el, 'color'), 'red');
    assert.equal(await call(page, 'style_css_text', el), 'color: red;');
  });

  test('important priority is kept and serialised', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'div', null);
    await call(page, 'style_set_property', el, 'color', 'blue', true);
    assert.equal(await call(page, 'style_css_text', el), 'color: blue !important;');
  });

  test('custom properties and shorthands are accepted', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'div', null);
    await call(page, 'style_set_property', el, '--gap', '4px', false);
    assert.equal(await call(page, 'style_get_property', el, '--gap'), '4px');
    await call(page, 'style_set_property', el, 'margin', '1px 2px', false);
    const expected = await page.evaluate(() => {
      const d = document.createElement('div');
      d.style.setProperty('--gap', '4px');
      d.style.setProperty('margin', '1px 2px');
      return d.style.cssText;
    });
    assert.equal(await call(page, 'style_css_text', el), expected);
    assert.equal(await call(page, 'style_get_property', el, 'margin-top'), '1px');
  });

  test('a camelCase or unknown property name is Unsupported (kebab-case is required)', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'div', null);
    const camel = await callErr(page, 'Unsupported', 'style_set_property', el, 'backgroundColor', 'red', false);
    assert.match(camel.msg, /kebab-case/);
    await callErr(page, 'Unsupported', 'style_set_property', el, 'not-a-css-property', 'x', false);
  });

  test('an invalid value is ignored, as CSSOM does: nothing is set and the call succeeds', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'div', null);
    await call(page, 'style_set_property', el, 'width', 'banana', false);
    assert.equal(await call(page, 'style_get_property', el, 'width'), '');
    assert.equal(await call(page, 'style_css_text', el), '');
  });

  test('removeProperty removes a declaration; removing an absent one is not an error', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'div', null);
    await call(page, 'style_set_property', el, 'color', 'red', false);
    await call(page, 'style_set_property', el, 'display', 'block', false);
    await call(page, 'style_remove_property', el, 'color');
    assert.equal(await call(page, 'style_css_text', el), 'display: block;');
    await call(page, 'style_remove_property', el, 'color');
  });

  test('cssText round trip matches the browser, including dropped invalid declarations', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'div', null);
    const text = 'color: red; margin: 0; width: nope; font-size: 12px';
    await call(page, 'style_set_css_text', el, text);
    assert.equal(await call(page, 'style_css_text', el), await browserCss(page, text));
    assert.equal(await call(page, 'style_css_text', el), 'color: red; margin: 0px; font-size: 12px;');
  });

  test('style applies to SVG elements; text nodes have no style (InvalidState)', async () => {
    await reloadHost(page, site);
    const svg = await call(page, 'create_element', 'svg', 'rect', null);
    await call(page, 'style_set_property', svg, 'fill', 'green', false);
    assert.equal(await call(page, 'style_css_text', svg), 'fill: green;');
    const text = await call(page, 'create_text', 'x');
    await callErr(page, 'InvalidState', 'style_set_property', text, 'color', 'red', false);
  });

  test('style changes are visible in the DOM the page holds', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const el = await call(page, 'create_element', 'html', 'p', null);
    await call(page, 'insert', el, app, null);
    await call(page, 'style_set_property', el, 'opacity', '0.5', false);
    assert.equal(await appHtml(page), '<p style="opacity: 0.5;"></p>');
  });
});

describe('classList', skipOpt, () => {
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

  test('add, duplicate add and remove keep the class attribute in step', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'div', null);
    await call(page, 'class_list_add', el, 'a');
    await call(page, 'class_list_add', el, 'b');
    await call(page, 'class_list_add', el, 'a');
    assert.equal(await call(page, 'get_attribute', el, 'class'), 'a b');
    await call(page, 'class_list_remove', el, 'a');
    assert.equal(await call(page, 'get_attribute', el, 'class'), 'b');
    await call(page, 'class_list_remove', el, 'absent');
  });

  test('a token with whitespace is InvalidState (InvalidCharacterError); an empty token is InvalidState (SyntaxError)', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'div', null);
    const spaced = await callErr(page, 'InvalidState', 'class_list_add', el, 'x y');
    assert.match(spaced.msg, /InvalidCharacterError/);
    const empty = await callErr(page, 'InvalidState', 'class_list_add', el, '');
    assert.match(empty.msg, /SyntaxError/);
  });
});

describe('focus and selectors', skipOpt, () => {
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

  test('focus makes an input the active element; blur clears it', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const input = await call(page, 'create_element', 'html', 'input', null);
    await call(page, 'insert', input, app, null);
    await call(page, 'focus', input);
    assert.equal(await call(page, 'active_element'), input);
    assert.equal(await page.evaluate(() => document.activeElement?.tagName), 'INPUT');
    await call(page, 'blur', input);
    assert.equal(await call(page, 'active_element'), null);
  });

  test('focus moves between elements: only the last one is active', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const a = await call(page, 'create_element', 'html', 'input', null);
    const b = await call(page, 'create_element', 'html', 'button', null);
    await call(page, 'insert', a, app, null);
    await call(page, 'insert', b, app, null);
    await call(page, 'focus', a);
    await call(page, 'focus', b);
    assert.equal(await call(page, 'active_element'), b);
  });

  test('focus on a non-focusable element does nothing; tabindex makes it focusable', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const div = await call(page, 'create_element', 'html', 'div', null);
    await call(page, 'insert', div, app, null);
    await call(page, 'focus', div);
    assert.equal(await call(page, 'active_element'), null);
    await call(page, 'set_attribute', div, 'tabindex', '0');
    await call(page, 'focus', div);
    assert.equal(await call(page, 'active_element'), div);
  });

  test('focus on a detached element does nothing; on a text node it is InvalidState', async () => {
    await reloadHost(page, site);
    const input = await call(page, 'create_element', 'html', 'input', null);
    await call(page, 'focus', input);
    assert.equal(await call(page, 'active_element'), null);
    const text = await call(page, 'create_text', 'x');
    await callErr(page, 'InvalidState', 'focus', text);
    await callErr(page, 'InvalidState', 'blur', text);
  });

  test('removing the focused element clears the focus', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const input = await call(page, 'create_element', 'html', 'input', null);
    await call(page, 'insert', input, app, null);
    await call(page, 'focus', input);
    await call(page, 'remove', input);
    assert.equal(await call(page, 'active_element'), null);
  });

  test('selectors: tag, id, class, attribute, attribute value (bare and quoted), star, and comma lists', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const span = await call(page, 'create_element', 'html', 'SPAN', null);
    await call(page, 'set_attribute', span, 'id', 'target');
    await call(page, 'set_attribute', span, 'class', 'pick big');
    await call(page, 'set_attribute', span, 'data-k', 'a b');
    await call(page, 'insert', span, app, null);
    const other = await call(page, 'create_element', 'html', 'em', null);
    await call(page, 'set_attribute', other, 'data-k', 'x');
    await call(page, 'insert', other, app, null);
    assert.equal(await call(page, 'query_selector', 'span'), span);
    assert.equal(await call(page, 'query_selector', '#target'), span);
    assert.equal(await call(page, 'query_selector', '.big'), span);
    assert.equal(await call(page, 'query_selector', '[data-k]'), span);
    assert.equal(await call(page, 'query_selector', '[data-k=x]'), other);
    assert.equal(await call(page, 'query_selector', '[data-k="a b"]'), span);
    assert.equal(await call(page, 'query_selector', "[data-k='x']"), other);
    assert.equal(await call(page, 'query_selector', 'em, #target'), span, 'first match in document order');
    assert.equal(await call(page, 'tag_name', await call(page, 'query_selector', '*')), 'HTML');
  });

  test('selector syntax outside the subset is Unsupported, an empty one is InvalidState', async () => {
    await reloadHost(page, site);
    for (const sel of ['div > span', 'a b', 'li:first-child', 'a[href^="x"]', 'p::before', '[a|b]']) {
      const err = await callErr(page, 'Unsupported', 'query_selector', sel);
      assert.match(err.msg, /selector grammar/, sel);
    }
    await callErr(page, 'InvalidState', 'query_selector', '   ');
  });
});

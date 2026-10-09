// Tree operations of the Host trait (nodeOps) through the real imports and DOM. Expectations are hand-written.
import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { appHandle, appHtml, call, callErr, openHostPage, reloadHost, runOps, skipOpt, startSite } from './support.mjs';

describe('tree operations', skipOpt, () => {
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

  test('a fresh page starts with a body handle and no focused element', async () => {
    await reloadHost(page, site);
    const body = await call(page, 'body');
    assert.equal(await call(page, 'tag_name', body), 'BODY');
    assert.equal(await call(page, 'active_element'), null);
  });

  test('create_element lower-cases HTML tag names and reports the uppercase tagName', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'DIV', null);
    assert.equal(await call(page, 'tag_name', el), 'DIV');
    assert.equal(await call(page, 'is_element', el), true);
    await call(page, 'insert', el, await appHandle(page), null);
    assert.equal(await appHtml(page), '<div></div>');
  });

  test('SVG and MathML elements keep their case and namespace', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const grad = await call(page, 'create_element', 'svg', 'linearGradient', null);
    const mi = await call(page, 'create_element', 'mathml', 'mi', null);
    assert.equal(await call(page, 'tag_name', grad), 'linearGradient');
    await call(page, 'insert', grad, app, null);
    await call(page, 'insert', mi, app, null);
    const ns = await page.evaluate(() => [...document.getElementById('app').children].map((e) => e.namespaceURI));
    assert.deepEqual(ns, ['http://www.w3.org/2000/svg', 'http://www.w3.org/1998/Math/MathML']);
  });

  test('an invalid element name is InvalidState with the browser message', async () => {
    await reloadHost(page, site);
    const err = await callErr(page, 'InvalidState', 'create_element', 'html', 'bad name', null);
    assert.match(err.msg, /InvalidCharacterError/);
  });

  test('script, iframe and object elements are refused by policy (Unsupported)', async () => {
    await reloadHost(page, site);
    for (const tag of ['script', 'SCRIPT', 'iframe', 'object']) {
      const err = await callErr(page, 'Unsupported', 'create_element', 'html', tag, null);
      assert.match(err.msg, /not created by the host/);
    }
    await callErr(page, 'Unsupported', 'create_element', 'svg', 'script', null);
  });

  test('insert appends with a null anchor and inserts before an anchor', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const [a, b, c, d] = ['a', 'b', 'c', 'd'].map(() => null);
    const make = async (tag) => call(page, 'create_element', 'html', tag, null);
    const ha = await make('p');
    const hb = await make('section');
    const hc = await make('aside');
    const hd = await make('nav');
    void [a, b, c, d];
    await call(page, 'insert', ha, app, null);
    await call(page, 'insert', hb, app, null);
    await call(page, 'insert', hc, app, null);
    await call(page, 'insert', hd, app, hb);
    assert.equal(await appHtml(page), '<p></p><nav></nav><section></section><aside></aside>');
  });

  test('inserting an attached node moves it and leaves its old parent', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const left = await call(page, 'create_element', 'html', 'ul', null);
    const right = await call(page, 'create_element', 'html', 'ol', null);
    const item = await call(page, 'create_element', 'html', 'li', null);
    await call(page, 'insert', left, app, null);
    await call(page, 'insert', right, app, null);
    await call(page, 'insert', item, left, null);
    await call(page, 'insert', item, right, null);
    assert.equal(await appHtml(page), '<ul></ul><ol><li></li></ol>');
    assert.equal(await call(page, 'parent_node', item), right);
  });

  test('an anchor that is not a child of the parent is InvalidState', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const parent = await call(page, 'create_element', 'html', 'div', null);
    const stranger = await call(page, 'create_element', 'html', 'span', null);
    await call(page, 'insert', parent, app, null);
    const err = await callErr(page, 'InvalidState', 'insert', stranger, parent, stranger);
    assert.match(err.msg, /NotFoundError/);
  });

  test('inserting a node into its own descendant is InvalidState (cycle)', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const outer = await call(page, 'create_element', 'html', 'div', null);
    const inner = await call(page, 'create_element', 'html', 'div', null);
    await call(page, 'insert', outer, app, null);
    await call(page, 'insert', inner, outer, null);
    const err = await callErr(page, 'InvalidState', 'insert', outer, inner, null);
    assert.match(err.msg, /HierarchyRequestError/);
    assert.equal(await appHtml(page), '<div><div></div></div>');
  });

  test('inserting into a text node is InvalidState', async () => {
    await reloadHost(page, site);
    const text = await call(page, 'create_text', 'hello');
    const el = await call(page, 'create_element', 'html', 'b', null);
    await callErr(page, 'InvalidState', 'insert', el, text, null);
  });

  test('remove detaches the node; removing a detached node is not an error', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const el = await call(page, 'create_element', 'html', 'i', null);
    await call(page, 'insert', el, app, null);
    await call(page, 'remove', el);
    assert.equal(await appHtml(page), '');
    assert.equal(await call(page, 'parent_node', el), null);
    await call(page, 'remove', el);
  });

  test('parent_node and next_sibling follow the tree', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const a = await call(page, 'create_element', 'html', 'a', null);
    const b = await call(page, 'create_element', 'html', 'b', null);
    await call(page, 'insert', a, app, null);
    await call(page, 'insert', b, app, null);
    assert.equal(await call(page, 'parent_node', a), app);
    assert.equal(await call(page, 'next_sibling', a), b);
    assert.equal(await call(page, 'next_sibling', b), null);
    assert.equal(await call(page, 'parent_node', app), await call(page, 'body'));
  });

  test('the document node is never exposed: the parent of body is null', async () => {
    await reloadHost(page, site);
    const body = await call(page, 'body');
    const html = await call(page, 'parent_node', body);
    assert.equal(await call(page, 'tag_name', html), 'HTML');
    assert.equal(await call(page, 'parent_node', html), null);
  });

  test('text nodes: create, set_text, and comments serialise as the DOM does', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const text = await call(page, 'create_text', 'a < b & "c"');
    const note = await call(page, 'create_comment', 'note');
    await call(page, 'insert', text, app, null);
    await call(page, 'insert', note, app, null);
    assert.equal(await appHtml(page), 'a &lt; b &amp; "c"<!--note-->');
    await call(page, 'set_text', text, 'changed');
    assert.equal(await appHtml(page), 'changed<!--note-->');
    assert.equal(await call(page, 'outer_html', text), null, 'text nodes have no outerHTML');
  });

  test('set_text on an element is InvalidState; set_element_text on a text node is InvalidState', async () => {
    await reloadHost(page, site);
    const el = await call(page, 'create_element', 'html', 'p', null);
    const text = await call(page, 'create_text', 'x');
    const err = await callErr(page, 'InvalidState', 'set_text', el, 'nope');
    assert.match(err.msg, /set_element_text/);
    await callErr(page, 'InvalidState', 'set_element_text', text, 'nope');
  });

  test('set_element_text replaces all children; an empty text removes them', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    await call(page, 'insert', await call(page, 'create_element', 'html', 'em', null), app, null);
    await call(page, 'set_element_text', app, 'one & two');
    assert.equal(await appHtml(page), 'one &amp; two');
    await call(page, 'set_element_text', app, '');
    assert.equal(await appHtml(page), '');
  });

  test('option_nodes lists the options of a select in order, and rejects other elements', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const select = await call(page, 'create_element', 'html', 'select', null);
    await call(page, 'insert', select, app, null);
    const first = await call(page, 'create_element', 'html', 'option', null);
    const second = await call(page, 'create_element', 'html', 'option', null);
    await call(page, 'insert', second, select, null);
    await call(page, 'insert', first, select, null);
    const options = await call(page, 'option_nodes', select);
    assert.deepEqual(options, [second, first]);
    assert.deepEqual(await call(page, 'option_nodes', app), [], 'non-select: warned and empty');
  });

  test('outer_html and inner_html serialise elements; both are null for text', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const div = await call(page, 'create_element', 'html', 'div', null);
    await call(page, 'insert', div, app, null);
    await call(page, 'set_element_text', div, 'hi');
    assert.equal(await call(page, 'outer_html', div), '<div>hi</div>');
    assert.equal(await call(page, 'inner_html', div), 'hi');
    const text = await call(page, 'create_text', 'z');
    assert.equal(await call(page, 'inner_html', text), null);
  });

  test('query_selector finds by subset selectors and reports the rest as Unsupported', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    assert.equal(await call(page, 'query_selector', '#app'), app);
    assert.equal(await call(page, 'query_selector', 'body'), await call(page, 'body'));
    const lone = await call(page, 'create_element', 'html', 'span', null);
    await call(page, 'insert', lone, app, null);
    assert.equal(await call(page, 'query_selector', '#missing'), null);
    const err = await callErr(page, 'Unsupported', 'query_selector', 'div > span');
    assert.match(err.msg, /supported selector grammar/);
    await callErr(page, 'Unsupported', 'query_selector', 'a:hover');
    await callErr(page, 'InvalidState', 'query_selector', '');
    await callErr(page, 'InvalidState', 'query_selector', '#1x');
  });

  test('a released handle is NotFound, even after its slot is reused', async () => {
    await reloadHost(page, site);
    const first = await call(page, 'create_element', 'html', 'b', null);
    await call(page, 'release', first);
    await callErr(page, 'NotFound', 'set_element_text', first, 'x');
    const second = await call(page, 'create_element', 'html', 'i', null);
    assert.notEqual(second, first, 'a reused slot gets a new handle');
    await callErr(page, 'NotFound', 'remove', first);
    assert.equal(await call(page, 'tag_name', first), '', 'infallible query: empty value, and a logged warning');
    assert.equal(await call(page, 'tag_name', second), 'I');
  });

  test('handle 0 and unknown handles are NotFound', async () => {
    await reloadHost(page, site);
    await callErr(page, 'NotFound', 'insert', 0, await appHandle(page), null);
    await callErr(page, 'NotFound', 'set_text', 4000000000, 'x');
    await callErr(page, 'NotFound', 'set_element_text', 99999999, 'x');
  });

  test('release of an attached node keeps it in the document', async () => {
    await reloadHost(page, site);
    const app = await appHandle(page);
    const el = await call(page, 'create_element', 'html', 'mark', null);
    await call(page, 'insert', el, app, null);
    await call(page, 'release', el);
    assert.equal(await appHtml(page), '<mark></mark>');
  });
});

describe('harness guards', skipOpt, () => {
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

  test('a malformed request is a harness error and never reaches the host', async () => {
    await reloadHost(page, site);
    const [r] = await runOps(page, [['no_such_method']]);
    assert.match(r.harness, /unknown method/);
    const [bad] = await runOps(page, [['insert', 1]]);
    assert.match(bad.harness, /missing argument/);
  });
});

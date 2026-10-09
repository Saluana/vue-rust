// tree/*: node creation, insertion and removal, anchors, cycles, document child rules, text content (WP-U0).
import { def, UNDEF } from '../builder.mjs';

def('tree/create-html-lowercases', (b) => {
  const d = b.el('html', 'DIV');
  b.op('localName', d);
  b.op('tagName', d);
  b.op('nodeName', d);
  b.op('namespaceURI', d);
  b.op('outerHTML', d);
});

def('tree/create-svg-keeps-case', (b) => {
  const g = b.el('svg', 'linearGradient');
  b.op('localName', g);
  b.op('tagName', g);
  b.op('namespaceURI', g);
  b.op('outerHTML', g);
});

def('tree/create-mathml', (b) => {
  const m = b.el('math', 'mi');
  b.op('tagName', m);
  b.op('namespaceURI', m);
  b.op('outerHTML', m);
});

def('tree/create-invalid-names', (b) => {
  b.op('el', 'html', '');
  b.op('el', 'html', 'a b');
  b.op('el', 'html', '1x');
  b.op('el', 'svg', 'a>b');
});

def('tree/create-text-comment', (b) => {
  const t = b.text('hi');
  const c = b.comment('note');
  b.op('nodeName', t);
  b.op('nodeType', t);
  b.op('nodeValue', t);
  b.op('nodeName', c);
  b.op('nodeType', c);
  b.op('nodeValue', c);
  b.op('textContent', c);
});

def('tree/append-order', (b) => {
  const p = b.el('html', 'div');
  const a = b.el('html', 'span');
  const x = b.el('html', 'span');
  const c = b.el('html', 'span');
  b.op('append', p, a);
  b.op('append', p, x);
  b.op('append', p, c);
  b.op('childNodes', p);
  b.op('firstChild', p);
  b.op('lastChild', p);
  b.op('nextSibling', a);
  b.op('previousSibling', c);
  b.op('previousSibling', a);
});

def('tree/insert-before-anchor', (b) => {
  const p = b.el('html', 'div');
  const a = b.el('html', 'i');
  const x = b.el('html', 'b');
  const c = b.el('html', 'u');
  b.op('append', p, a);
  b.op('append', p, c);
  b.op('insert', p, x, c);
  b.op('childNodes', p);
});

def('tree/insert-null-anchor-appends', (b) => {
  const p = b.el('html', 'div');
  const a = b.el('html', 'i');
  const x = b.el('html', 'b');
  b.op('insert', p, a, null);
  b.op('insert', p, x, null);
  b.op('childNodes', p);
});

def('tree/insert-moves-attached', (b) => {
  const p1 = b.el('html', 'div');
  const p2 = b.el('html', 'section');
  const c = b.el('html', 'span');
  b.op('append', p1, c);
  b.op('insert', p2, c, null);
  b.op('childNodes', p1);
  b.op('childNodes', p2);
  b.op('parent', c);
});

def('tree/insert-anchor-self-noop', (b) => {
  const p = b.el('html', 'div');
  const a = b.el('html', 'i');
  const x = b.el('html', 'b');
  const c = b.el('html', 'u');
  b.op('append', p, a);
  b.op('append', p, x);
  b.op('append', p, c);
  b.op('insert', p, x, x);
  b.op('childNodes', p);
  b.op('insert', p, c, c);
  b.op('childNodes', p);
});

def('tree/insert-first-before-first-child', (b) => {
  const p = b.el('html', 'div');
  const a = b.el('html', 'i');
  const x = b.el('html', 'b');
  b.op('append', p, x);
  b.op('insert', p, a, x);
  b.op('childNodes', p);
  b.op('firstChild', p);
});

def('tree/insert-anchor-not-child', (b) => {
  const p = b.el('html', 'div');
  const q = b.el('html', 'div');
  const a = b.el('html', 'i');
  const x = b.el('html', 'b');
  b.op('append', p, a);
  b.op('insert', p, x, q);
  b.op('childNodes', p);
  b.op('parent', x);
});

def('tree/cycle-self-and-ancestor', (b) => {
  const p = b.el('html', 'div');
  const c = b.el('html', 'span');
  b.op('append', p, p);
  b.op('append', c, c);
  b.op('append', p, c);
  b.op('append', c, p);
  b.op('childNodes', p);
  b.op('parent', p);
});

def('tree/insert-into-text-or-comment', (b) => {
  const t = b.text('x');
  const cm = b.comment('x');
  const e = b.el('html', 'div');
  b.op('append', t, e);
  b.op('append', cm, e);
  b.op('parent', e);
});

def('tree/document-rejects-text', (b) => {
  const t = b.text('x');
  b.op('append', 0, t);
  b.op('childNodes', 0);
});

def('tree/document-one-element-only', (b) => {
  const e1 = b.el('html', 'div');
  const e2 = b.el('html', 'span');
  b.op('append', 0, e1);
  b.op('append', 0, e2);
  b.op('childNodes', 0);
  b.op('firstChild', 0);
});

def('tree/document-comment-allowed', (b) => {
  const c = b.comment('top');
  b.op('append', 0, c);
  b.op('childNodes', 0);
  b.op('nodeName', 0);
  b.op('nodeType', 0);
});

def('tree/document-element-replaced-after-remove', (b) => {
  const e1 = b.el('html', 'div');
  const e2 = b.el('html', 'span');
  b.op('append', 0, e1);
  b.op('remove', e1);
  b.op('append', 0, e2);
  b.op('childNodes', 0);
});

def('tree/remove-child-missing', (b) => {
  const p = b.el('html', 'div');
  const a = b.el('html', 'i');
  const q = b.el('html', 'b');
  b.op('append', p, a);
  b.op('removeChild', p, q);
  b.op('removeChild', p, a);
  b.op('childNodes', p);
  b.op('parent', a);
});

def('tree/remove-detaches-and-connected', (b) => {
  const p = b.el('html', 'div');
  const a = b.el('html', 'i');
  b.op('append', p, a);
  b.op('append', 0, p);
  b.op('isConnected', a);
  b.op('remove', a);
  b.op('isConnected', a);
  b.op('childNodes', p);
});

def('tree/sibling-navigation-after-remove', (b) => {
  const p = b.el('html', 'div');
  const a = b.el('html', 'i');
  const x = b.el('html', 'b');
  const c = b.el('html', 'u');
  b.op('append', p, a);
  b.op('append', p, x);
  b.op('append', p, c);
  b.op('nextSibling', c);
  b.op('previousSibling', a);
  b.op('remove', x);
  b.op('nextSibling', a);
  b.op('previousSibling', c);
});

def('tree/contains-is-inclusive', (b) => {
  const p = b.el('html', 'div');
  const c = b.el('html', 'span');
  const d = b.el('html', 'i');
  b.op('append', p, c);
  b.op('append', c, d);
  b.op('contains', p, p);
  b.op('contains', p, d);
  b.op('contains', d, p);
  b.op('contains', c, c);
  b.op('contains', d, c);
});

def('tree/root-of-detached-and-connected', (b) => {
  const p = b.el('html', 'div');
  const c = b.el('html', 'span');
  b.op('append', p, c);
  b.op('root', c);
  b.op('isConnected', c);
  b.op('append', 0, p);
  b.op('root', c);
  b.op('isConnected', c);
});

def('tree/text-content-concatenates', (b) => {
  const d = b.el('html', 'div');
  const t1 = b.text('x');
  const s = b.el('html', 'span');
  const t2 = b.text('y');
  const cm = b.comment('zz');
  const t3 = b.text('w');
  b.op('append', d, t1);
  b.op('append', d, s);
  b.op('append', s, t2);
  b.op('append', d, cm);
  b.op('append', d, t3);
  b.op('textContent', d);
  b.op('textContent', cm);
  b.op('outerHTML', d);
});

def('tree/set-text-content-replaces-children', (b) => {
  const d = b.el('html', 'div');
  const a = b.el('html', 'span');
  b.op('append', d, a);
  b.op('setTextContent', d, 'hi');
  b.op('textContent', d);
  b.op('outerHTML', d);
  b.op('setTextContent', d, '');
  b.op('outerHTML', d);
  b.op('setTextContent', d, null);
  b.op('outerHTML', d);
  b.op('parent', a);
});

def('tree/text-node-data-setters', (b) => {
  const t = b.text('a');
  b.op('setTextContent', t, 'b');
  b.op('nodeValue', t);
  b.op('textContent', t);
  b.op('setNodeValue', t, null);
  b.op('nodeValue', t);
  b.op('set', t, 'nodeValue', 12);
  b.op('nodeValue', t);
});

def('tree/element-node-value-ignored', (b) => {
  const d = b.el('html', 'div');
  b.op('setNodeValue', d, 'x');
  b.op('nodeValue', d);
  b.op('textContent', d);
});

def('tree/comment-data-setters', (b) => {
  const cm = b.comment('a');
  b.op('setNodeValue', cm, 'b');
  b.op('nodeValue', cm);
  b.op('textContent', cm);
  b.op('setTextContent', cm, 'c');
  b.op('nodeValue', cm);
});

def('tree/document-text-properties-null', (b) => {
  b.op('nodeValue', 0);
  b.op('textContent', 0);
  b.op('setTextContent', 0, 'x');
  b.op('textContent', 0);
  b.op('nodeName', 0);
});

def('tree/empty-text-child-kept', (b) => {
  const p = b.el('html', 'div');
  const t = b.text('');
  b.op('append', p, t);
  b.op('childNodes', p);
  b.op('innerHTML', p);
  b.op('textContent', p);
});

def('tree/move-between-parents', (b) => {
  const a = b.el('html', 'div');
  const x = b.el('html', 'div');
  const c = b.el('html', 'span');
  b.op('append', a, c);
  b.op('append', x, c);
  b.op('childNodes', a);
  b.op('childNodes', x);
  b.op('parent', c);
});

def('tree/remove-then-reinsert', (b) => {
  const p = b.el('html', 'div');
  const a = b.el('html', 'span');
  b.op('append', p, a);
  b.op('removeChild', p, a);
  b.op('append', p, a);
  b.op('childNodes', p);
});

def('tree/mixed-namespace-children', (b) => {
  const p = b.el('html', 'div');
  const s = b.el('svg', 'svg');
  const m = b.el('math', 'mi');
  b.op('append', p, s);
  b.op('append', p, m);
  b.op('outerHTML', p);
  b.op('childNodes', p);
});

def('tree/undefined-node-value-set', (b) => {
  const t = b.text('keep');
  b.op('setTextContent', t, UNDEF);
  b.op('nodeValue', t);
});

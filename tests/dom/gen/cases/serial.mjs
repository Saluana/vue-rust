// serial/*: outerHTML and innerHTML (HTML fragment serialization) and the canonical snapshot used by the differential
// tests (WP-U0). The snapshot rule is implemented in page-runner.mjs (snap) and in crates/dom/src/serialize.rs.
import { def, UNDEF } from '../builder.mjs';

const XLINK = 'http://www.w3.org/1999/xlink';

def('serial/text-escapes', (b) => {
  const p = b.el('html', 'p');
  b.op('setTextContent', p, 'a < b & c > d "q"   \'s\'');
  b.op('outerHTML', p);
});

def('serial/attribute-escapes', (b) => {
  const img = b.el('html', 'img');
  b.op('setAttribute', img, 'src', 'x.png');
  b.op('setAttribute', img, 'alt', 'a"b<c>&d');
  b.op('outerHTML', img);
});

def('serial/void-element-ignores-children', (b) => {
  const br = b.el('html', 'br');
  b.op('append', br, b.text('no'));
  b.op('outerHTML', br);
});

def('serial/boolean-attributes-printed', (b) => {
  const i = b.el('html', 'input');
  b.op('setAttribute', i, 'value', 'x');
  b.op('setAttribute', i, 'disabled', '');
  b.op('setAttribute', i, 'type', 'checkbox');
  b.op('setAttribute', i, 'checked', '');
  b.op('outerHTML', i);
});

def('serial/comment-in-inner-html', (b) => {
  const root = b.el('html', 'div');
  b.op('append', root, b.comment('hi --> there'));
  b.op('innerHTML', root);
});

def('serial/style-text-is-raw', (b) => {
  const st = b.el('html', 'style');
  b.op('append', st, b.text('a > b { c: "&" }'));
  b.op('outerHTML', st);
});

def('serial/script-text-is-raw', (b) => {
  const sc = b.el('html', 'script');
  b.op('append', sc, b.text('if (a<b && c) {}'));
  b.op('outerHTML', sc);
});

def('serial/textarea-children-escaped', (b) => {
  const ta = b.el('html', 'textarea');
  b.op('setAttribute', ta, 'id', 'z');
  b.op('setTextContent', ta, 'first\nsecond <b>');
  b.op('outerHTML', ta);
});

def('serial/xmp-text-is-raw', (b) => {
  const xmp = b.el('html', 'xmp');
  b.op('setTextContent', xmp, '<a>&amp;');
  b.op('outerHTML', xmp);
});

def('serial/svg-tree', (b) => {
  const svg = b.el('svg', 'svg');
  b.op('setAttribute', svg, 'viewBox', '0 0 10 10');
  b.op('setAttribute', svg, 'class', 'ic');
  const rect = b.el('svg', 'rect');
  b.op('setAttribute', rect, 'width', '5');
  b.op('append', svg, rect);
  b.op('outerHTML', svg);
});

def('serial/svg-camel-tag-kept', (b) => {
  const lg = b.el('svg', 'linearGradient');
  b.op('outerHTML', lg);
});

def('serial/xlink-attribute-prefix', (b) => {
  const use = b.el('svg', 'use');
  b.op('setAttributeNS', use, XLINK, 'xlink:href', '#a');
  b.op('outerHTML', use);
});

def('serial/mathml-element', (b) => {
  const mi = b.el('math', 'mi');
  b.op('setTextContent', mi, 'x');
  b.op('outerHTML', mi);
});

def('serial/attribute-whitespace-kept-raw', (b) => {
  const p = b.el('html', 'p');
  b.op('setAttribute', p, 'title', 'a b\tc\nd');
  b.op('outerHTML', p);
});

def('serial/attribute-quotes-and-apostrophe', (b) => {
  const p = b.el('html', 'p');
  b.op('setAttribute', p, 'title', 'x&y "z" \'w\'');
  b.op('outerHTML', p);
});

def('serial/attribute-angle-brackets', (b) => {
  const p = b.el('html', 'p');
  b.op('setAttribute', p, 'title', '<&>');
  b.op('outerHTML', p);
});

def('serial/unicode-passthrough', (b) => {
  const p = b.el('html', 'p');
  b.op('setTextContent', p, 'é€😀');
  b.op('innerHTML', p);
});

def('serial/empty-attribute-value', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'data-a', '');
  b.op('outerHTML', d);
});

def('serial/inner-html-nested', (b) => {
  const d = b.el('html', 'div');
  const s = b.el('html', 'span');
  b.op('append', d, s);
  b.op('append', d, b.text('t'));
  b.op('innerHTML', d);
  b.op('outerHTML', d);
});

def('serial/snapshot-basic', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'id', 'a');
  b.op('setAttribute', d, 'class', 'b c');
  b.op('append', d, b.text('x'));
  b.op('append', d, b.comment('c'));
  b.op('snapshot', d);
});

def('serial/snapshot-input-props', (b) => {
  const i = b.el('html', 'input');
  b.op('setAttribute', i, 'value', 'a');
  b.op('set', i, 'value', 'b');
  b.op('snapshot', i);
  const j = b.el('html', 'input');
  b.op('setAttribute', j, 'type', 'checkbox');
  b.op('set', j, 'checked', true);
  b.op('snapshot', j);
  const k = b.el('html', 'input');
  b.op('setAttribute', k, 'type', 'checkbox');
  b.op('setAttribute', k, 'checked', '');
  b.op('set', k, 'checked', false);
  b.op('snapshot', k);
  const m = b.el('html', 'input');
  b.op('setAttribute', m, 'disabled', '');
  b.op('snapshot', m);
});

def('serial/snapshot-textarea-option-props', (b) => {
  const ta = b.el('html', 'textarea');
  b.op('setTextContent', ta, 'a');
  b.op('set', ta, 'value', 'b');
  b.op('snapshot', ta);
  const sel = b.el('html', 'select');
  const o1 = b.el('html', 'option');
  const o2 = b.el('html', 'option');
  b.op('setAttribute', o1, 'selected', '');
  b.op('append', sel, o1);
  b.op('append', sel, o2);
  b.op('set', o2, 'selected', true);
  b.op('snapshot', sel);
});

def('serial/snapshot-document-and-svg', (b) => {
  const svg = b.el('svg', 'svg');
  b.op('append', 0, svg);
  b.op('snapshot', svg);
  b.op('snapshot', 0);
});

def('serial/inner-html-of-text-undefined', (b) => {
  const t = b.text('x');
  b.op('innerHTML', t);
  b.op('outerHTML', t);
  b.op('innerHTML', 0);
});

def('serial/snapshot-escapes-json', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'title', 'q"\\\u0001');
  b.op('append', d, b.text('line\nnext\ttab'));
  b.op('snapshot', d);
});

def('serial/set-attribute-undefined-serialized', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'x', UNDEF);
  b.op('outerHTML', d);
});

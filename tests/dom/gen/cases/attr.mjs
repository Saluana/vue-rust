// attr/*: attribute names and namespaces, HTML name lowercasing, value conversion, ordering (WP-U0).
import { def, UNDEF } from '../builder.mjs';

const XLINK = 'http://www.w3.org/1999/xlink';
const XML = 'http://www.w3.org/XML/1998/namespace';

def('attr/html-name-lowercased', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'DATA-X', 'Y');
  b.op('attributeNames', d);
  b.op('getAttribute', d, 'data-x');
  b.op('getAttribute', d, 'DATA-X');
  b.op('hasAttribute', d, 'Data-X');
  b.op('outerHTML', d);
});

def('attr/svg-name-case-preserved', (b) => {
  const s = b.el('svg', 'svg');
  b.op('setAttribute', s, 'viewBox', '0 0 10 10');
  b.op('attributeNames', s);
  b.op('getAttribute', s, 'viewbox');
  b.op('getAttribute', s, 'viewBox');
  b.op('outerHTML', s);
});

def('attr/remove-case-insensitive-html', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'Title', 'x');
  b.op('removeAttribute', d, 'TITLE');
  b.op('attributeNames', d);
  b.op('removeAttribute', d, 'title');
});

def('attr/replace-keeps-position', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'a', '1');
  b.op('setAttribute', d, 'b', '2');
  b.op('setAttribute', d, 'a', '3');
  b.op('attributeNames', d);
  b.op('getAttribute', d, 'a');
});

def('attr/remove-readd-moves-last', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'a', '1');
  b.op('setAttribute', d, 'b', '2');
  b.op('removeAttribute', d, 'a');
  b.op('setAttribute', d, 'a', '3');
  b.op('attributeNames', d);
});

def('attr/null-and-undefined-values', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'x', null);
  b.op('setAttribute', d, 'y', UNDEF);
  b.op('getAttribute', d, 'x');
  b.op('getAttribute', d, 'y');
});

def('attr/number-and-boolean-values', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'n', 5);
  b.op('setAttribute', d, 'f', 1.5);
  b.op('setAttribute', d, 'b', true);
  b.op('setAttribute', d, 'z', false);
  b.op('attributeNames', d);
  b.op('getAttribute', d, 'n');
  b.op('getAttribute', d, 'f');
  b.op('getAttribute', d, 'b');
  b.op('getAttribute', d, 'z');
});

def('attr/invalid-names-rejected', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, '', 'x');
  b.op('setAttribute', d, 'a b', 'x');
  b.op('setAttribute', d, 'a=b', 'x');
  b.op('setAttribute', d, 'a/b', 'x');
  b.op('setAttribute', d, 'a>b', 'x');
  b.op('attributeNames', d);
});

def('attr/lenient-names-accepted', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, '1a', 'x');
  b.op('setAttribute', d, 'a1-._:z', 'y');
  b.op('attributeNames', d);
});

def('attr/ns-null-keeps-case-but-lookup-lowercases', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttributeNS', d, null, 'Mixed', 'v');
  b.op('attributeNames', d);
  b.op('getAttribute', d, 'Mixed');
  b.op('getAttribute', d, 'mixed');
  b.op('hasAttribute', d, 'Mixed');
});

def('attr/ns-prefix-without-namespace-error', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttributeNS', d, null, 'a:b', 'x');
  b.op('attributeNames', d);
});

def('attr/xmlns-without-namespace-error', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttributeNS', d, null, 'xmlns', 'v');
  b.op('attributeNames', d);
});

def('attr/xlink-prefixed-serialized', (b) => {
  const s = b.el('svg', 'use');
  b.op('setAttributeNS', s, XLINK, 'xlink:href', '#a');
  b.op('attributeNames', s);
  b.op('getAttribute', s, 'xlink:href');
  b.op('outerHTML', s);
});

def('attr/xlink-unprefixed-serialized-with-prefix', (b) => {
  const s = b.el('svg', 'use');
  b.op('setAttributeNS', s, XLINK, 'href', '#a');
  b.op('attributeNames', s);
  b.op('getAttribute', s, 'href');
  b.op('outerHTML', s);
});

def('attr/xlink-then-prefixed-keeps-qualified-name', (b) => {
  const s = b.el('svg', 'use');
  b.op('setAttributeNS', s, XLINK, 'href', 'a');
  b.op('setAttributeNS', s, XLINK, 'xlink:href', 'b');
  b.op('attributeNames', s);
  b.op('getAttribute', s, 'href');
  b.op('outerHTML', s);
});

def('attr/xml-namespace-lang', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttributeNS', d, XML, 'lang', 'en');
  b.op('attributeNames', d);
  b.op('outerHTML', d);
});

def('attr/xlink-on-html-element-serialized', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttributeNS', d, XLINK, 'xlink:show', 'new');
  b.op('outerHTML', d);
});

def('attr/remove-missing-noop', (b) => {
  const d = b.el('html', 'div');
  b.op('removeAttribute', d, 'nope');
  b.op('attributeNames', d);
  b.op('hasAttribute', d, 'nope');
});

def('attr/value-escaping-roundtrip', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'title', 'a&b "c" <d>');
  b.op('getAttribute', d, 'title');
  b.op('outerHTML', d);
});

def('attr/empty-value-present', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'data-a', '');
  b.op('hasAttribute', d, 'data-a');
  b.op('getAttribute', d, 'data-a');
  b.op('outerHTML', d);
});

def('attr/mathml-name-case-preserved', (b) => {
  const m = b.el('math', 'mi');
  b.op('setAttribute', m, 'DefinitionURL', 'x');
  b.op('attributeNames', m);
  b.op('getAttribute', m, 'definitionurl');
});

def('attr/disabled-false-string-is-presence', (b) => {
  const i = b.el('html', 'input');
  b.op('setAttribute', i, 'disabled', 'false');
  b.op('hasAttribute', i, 'disabled');
  b.op('get', i, 'disabled');
});

def('attr/order-after-many-ops', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'a', '1');
  b.op('setAttribute', d, 'b', '2');
  b.op('setAttribute', d, 'c', '3');
  b.op('removeAttribute', d, 'b');
  b.op('setAttribute', d, 'b', '4');
  b.op('setAttribute', d, 'a', '5');
  b.op('attributeNames', d);
});

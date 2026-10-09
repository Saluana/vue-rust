// prop/*: reflected IDL properties, value conversions, tabIndex defaults and parsing, `in` checks (WP-U0).
// The has/* cases list only names that are either supported by vn-dom or absent in Chrome for that element, so the
// corpus never records a difference that vn-dom documents as a gap (see crates/dom/src/props.rs).
import { def, UNDEF } from '../builder.mjs';

const make = {
  div: (b) => b.el('html', 'div'),
  button: (b) => b.el('html', 'button'),
  input: (b) => b.el('html', 'input'),
  select: (b) => b.el('html', 'select'),
  option: (b) => b.el('html', 'option'),
  textarea: (b) => b.el('html', 'textarea'),
  a: (b) => b.el('html', 'a'),
  img: (b) => b.el('html', 'img'),
  label: (b) => b.el('html', 'label'),
  fieldset: (b) => b.el('html', 'fieldset'),
  svgRect: (b) => b.el('svg', 'rect'),
  mathMi: (b) => b.el('math', 'mi'),
  text: (b) => b.text('t'),
  comment: (b) => b.comment('c'),
  document: () => 0,
};

const NAMES = {
  div: ['id', 'className', 'title', 'hidden', 'tabIndex', 'style', 'children', 'textContent', 'nodeValue', 'innerHTML', 'outerHTML',
    'foo', 'data-x', 'value', 'checked', 'disabled', 'type', 'href', 'src', 'selectedIndex', 'options', 'text', 'htmlFor', 'min',
    'nodeName', 'nodeType', 'parentNode', 'childNodes', 'firstChild', 'isConnected', 'tagName', 'localName', 'namespaceURI'],
  input: ['id', 'title', 'hidden', 'tabIndex', 'style', 'children', 'textContent', 'nodeValue', 'innerHTML', 'outerHTML', 'value',
    'checked', 'defaultChecked', 'defaultValue', 'type', 'name', 'placeholder', 'readOnly', 'required', 'multiple', 'min', 'max',
    'step', 'disabled', 'src', 'alt', 'foo', 'tagName', 'nodeName', 'parentNode', 'href', 'selected', 'text', 'htmlFor', 'selectedIndex', 'options'],
  option: ['id', 'title', 'hidden', 'tabIndex', 'style', 'children', 'textContent', 'nodeValue', 'innerHTML', 'outerHTML', 'value',
    'text', 'selected', 'defaultSelected', 'disabled', 'foo', 'type', 'name', 'checked', 'multiple', 'href'],
  select: ['id', 'title', 'hidden', 'tabIndex', 'style', 'children', 'textContent', 'nodeValue', 'innerHTML', 'outerHTML', 'value',
    'selectedIndex', 'options', 'multiple', 'name', 'required', 'disabled', 'type', 'foo', 'text', 'checked', 'min', 'placeholder'],
  textarea: ['id', 'title', 'hidden', 'tabIndex', 'style', 'children', 'textContent', 'nodeValue', 'innerHTML', 'value',
    'defaultValue', 'name', 'placeholder', 'readOnly', 'required', 'disabled', 'type', 'foo', 'checked', 'text', 'multiple', 'selectedIndex', 'href'],
  button: ['id', 'title', 'hidden', 'tabIndex', 'style', 'children', 'textContent', 'nodeValue', 'innerHTML', 'value', 'disabled',
    'type', 'name', 'foo', 'checked', 'placeholder', 'multiple', 'href'],
  a: ['id', 'title', 'hidden', 'tabIndex', 'style', 'children', 'textContent', 'nodeValue', 'innerHTML', 'href', 'type', 'foo',
    'value', 'checked', 'disabled', 'src'],
  img: ['id', 'title', 'hidden', 'tabIndex', 'style', 'children', 'textContent', 'nodeValue', 'innerHTML', 'src', 'alt', 'name',
    'foo', 'href', 'value', 'type', 'disabled', 'checked'],
  label: ['id', 'title', 'hidden', 'tabIndex', 'style', 'children', 'textContent', 'nodeValue', 'innerHTML', 'htmlFor', 'foo',
    'for', 'value', 'type', 'disabled', 'checked'],
  fieldset: ['id', 'title', 'hidden', 'tabIndex', 'style', 'children', 'textContent', 'nodeValue', 'innerHTML', 'disabled', 'name',
    'foo', 'value', 'checked'],
  svgRect: ['id', 'className', 'tabIndex', 'style', 'children', 'textContent', 'nodeValue', 'innerHTML', 'outerHTML', 'foo',
    'title', 'hidden', 'href', 'value', 'disabled', 'tagName', 'localName', 'namespaceURI', 'nodeName', 'parentNode'],
  mathMi: ['id', 'className', 'tabIndex', 'style', 'children', 'innerHTML', 'outerHTML', 'textContent', 'nodeValue', 'foo',
    'title', 'hidden', 'value'],
  text: ['textContent', 'nodeValue', 'parentNode', 'childNodes', 'firstChild', 'isConnected', 'nodeName', 'nodeType', 'foo', 'id',
    'children', 'style', 'tagName', 'localName', 'title', 'value'],
  comment: ['textContent', 'nodeValue', 'parentNode', 'childNodes', 'firstChild', 'isConnected', 'nodeName', 'nodeType', 'foo',
    'id', 'children', 'style', 'tagName', 'localName', 'title', 'value'],
  document: ['textContent', 'nodeValue', 'parentNode', 'childNodes', 'firstChild', 'isConnected', 'nodeName', 'nodeType', 'children',
    'foo', 'id', 'style', 'tagName', 'localName', 'value'],
};

for (const kind of Object.keys(NAMES)) {
  def(`prop/has-${kind.toLowerCase()}`, (b) => {
    const el = make[kind](b);
    for (const name of NAMES[kind]) b.op('has', el, name);
  });
}

def('prop/id-reflect', (b) => {
  const d = b.el('html', 'div');
  b.op('set', d, 'id', 'x1');
  b.op('get', d, 'id');
  b.op('getAttribute', d, 'id');
  b.op('set', d, 'id', null);
  b.op('get', d, 'id');
  b.op('set', d, 'id', UNDEF);
  b.op('get', d, 'id');
});

def('prop/classname-reflect', (b) => {
  const d = b.el('html', 'div');
  b.op('set', d, 'className', 'a b');
  b.op('getAttribute', d, 'class');
  b.op('set', d, 'className', 'c');
  b.op('getAttribute', d, 'class');
  b.op('get', d, 'className');
});

def('prop/title-hidden-reflect', (b) => {
  const d = b.el('html', 'div');
  b.op('set', d, 'title', 'T');
  b.op('get', d, 'title');
  b.op('getAttribute', d, 'title');
  b.op('set', d, 'hidden', true);
  b.op('hasAttribute', d, 'hidden');
  b.op('get', d, 'hidden');
  b.op('set', d, 'hidden', 0);
  b.op('hasAttribute', d, 'hidden');
  b.op('get', d, 'hidden');
});

def('prop/hidden-truthiness', (b) => {
  const d = b.el('html', 'div');
  b.op('set', d, 'hidden', 'x');
  b.op('get', d, 'hidden');
  b.op('set', d, 'hidden', '');
  b.op('get', d, 'hidden');
  b.op('set', d, 'hidden', 'false');
  b.op('get', d, 'hidden');
  b.op('set', d, 'hidden', null);
  b.op('get', d, 'hidden');
});

def('prop/tabindex-default-by-element', (b) => {
  const els = [
    b.el('html', 'div'), b.el('html', 'button'), b.el('html', 'a'), b.el('html', 'input'), b.el('html', 'select'),
    b.el('html', 'textarea'), b.el('html', 'iframe'), b.el('html', 'area'), b.el('html', 'span'), b.el('html', 'img'),
    b.el('html', 'summary'), b.el('html', 'label'), b.el('svg', 'rect'), b.el('svg', 'svg'),
  ];
  for (const e of els) b.op('get', e, 'tabIndex');
});

def('prop/tabindex-attribute-parsing', (b) => {
  const d = b.el('html', 'div');
  const values = ['  3', '+5', '3x', '-', '', '2147483648', ' 4', '-2147483648', ' -7abc'];
  for (const v of values) {
    b.op('setAttribute', d, 'tabindex', v);
    b.op('get', d, 'tabIndex');
  }
});

def('prop/tabindex-setter-coercion', (b) => {
  const d = b.el('html', 'div');
  b.op('set', d, 'tabIndex', '7x');
  b.op('getAttribute', d, 'tabindex');
  b.op('set', d, 'tabIndex', 2.9);
  b.op('getAttribute', d, 'tabindex');
  b.op('set', d, 'tabIndex', 4294967297);
  b.op('get', d, 'tabIndex');
  b.op('set', d, 'tabIndex', '  -4 ');
  b.op('get', d, 'tabIndex');
  b.op('set', d, 'tabIndex', true);
  b.op('get', d, 'tabIndex');
  b.op('set', d, 'tabIndex', null);
  b.op('get', d, 'tabIndex');
  b.op('set', d, 'tabIndex', UNDEF);
  b.op('get', d, 'tabIndex');
});

def('prop/input-type-normalized', (b) => {
  const i = b.el('html', 'input');
  b.op('set', i, 'type', 'Checkbox');
  b.op('get', i, 'type');
  b.op('getAttribute', i, 'type');
  b.op('set', i, 'type', 'WeIrD');
  b.op('get', i, 'type');
  b.op('set', i, 'type', '');
  b.op('get', i, 'type');
  b.op('set', i, 'type', 'NUMBER');
  b.op('get', i, 'type');
  b.op('getAttribute', i, 'type');
});

def('prop/input-type-from-attribute', (b) => {
  const i = b.el('html', 'input');
  b.op('setAttribute', i, 'type', 'SEARCH');
  b.op('get', i, 'type');
  b.op('getAttribute', i, 'type');
});

def('prop/input-string-reflects', (b) => {
  const i = b.el('html', 'input');
  b.op('set', i, 'name', 'nm');
  b.op('set', i, 'placeholder', 'ph');
  b.op('set', i, 'min', '1');
  b.op('set', i, 'max', '9');
  b.op('set', i, 'step', 'any');
  b.op('get', i, 'name');
  b.op('get', i, 'placeholder');
  b.op('get', i, 'min');
  b.op('get', i, 'max');
  b.op('get', i, 'step');
  b.op('attributeNames', i);
});

def('prop/input-boolean-reflects', (b) => {
  const i = b.el('html', 'input');
  b.op('set', i, 'readOnly', true);
  b.op('hasAttribute', i, 'readonly');
  b.op('set', i, 'required', 1);
  b.op('hasAttribute', i, 'required');
  b.op('set', i, 'multiple', 'x');
  b.op('hasAttribute', i, 'multiple');
  b.op('get', i, 'multiple');
  b.op('set', i, 'readOnly', false);
  b.op('hasAttribute', i, 'readonly');
  b.op('get', i, 'readOnly');
});

def('prop/href-reflects-raw', (b) => {
  const a = b.el('html', 'a');
  b.op('set', a, 'href', 'https://e.com/x?y=1#z');
  b.op('get', a, 'href');
  b.op('getAttribute', a, 'href');
  b.op('set', a, 'href', '/relative');
  b.op('get', a, 'href');
});

def('prop/img-src-alt', (b) => {
  const img = b.el('html', 'img');
  b.op('set', img, 'src', 'https://e.com/i.png');
  b.op('set', img, 'alt', 'alt');
  b.op('get', img, 'src');
  b.op('get', img, 'alt');
  b.op('set', img, 'alt', null);
  b.op('get', img, 'alt');
  b.op('getAttribute', img, 'alt');
});

def('prop/label-htmlfor', (b) => {
  const lab = b.el('html', 'label');
  b.op('setAttribute', lab, 'for', 'q');
  b.op('get', lab, 'htmlFor');
  b.op('set', lab, 'htmlFor', 'w');
  b.op('getAttribute', lab, 'for');
});

def('prop/button-type-normalized', (b) => {
  const bt = b.el('html', 'button');
  b.op('get', bt, 'type');
  b.op('set', bt, 'type', 'RESET');
  b.op('get', bt, 'type');
  b.op('getAttribute', bt, 'type');
  b.op('set', bt, 'type', 'bogus');
  b.op('get', bt, 'type');
});

def('prop/button-value-reflects', (b) => {
  const bt = b.el('html', 'button');
  b.op('get', bt, 'value');
  b.op('set', bt, 'value', 'x');
  b.op('get', bt, 'value');
  b.op('getAttribute', bt, 'value');
});

def('prop/disabled-reflects-per-element', (b) => {
  const els = [b.el('html', 'button'), b.el('html', 'input'), b.el('html', 'select'), b.el('html', 'textarea'),
    b.el('html', 'option'), b.el('html', 'fieldset')];
  for (const e of els) {
    b.op('set', e, 'disabled', true);
    b.op('hasAttribute', e, 'disabled');
    b.op('get', e, 'disabled');
    b.op('set', e, 'disabled', false);
    b.op('hasAttribute', e, 'disabled');
  }
});

def('prop/fieldset-disabled-not-propagated', (b) => {
  const fs = b.el('html', 'fieldset');
  const i = b.el('html', 'input');
  b.op('setAttribute', fs, 'disabled', '');
  b.op('append', fs, i);
  b.op('get', i, 'disabled');
  b.op('get', fs, 'disabled');
});

def('prop/unsupported-names-read-undefined', (b) => {
  const d = b.el('html', 'div');
  b.op('get', d, 'disabled');
  b.op('get', d, 'value');
  b.op('get', d, 'foo');
});

def('prop/read-only-set-is-type-error', (b) => {
  const d = b.el('html', 'div');
  b.op('set', d, 'tagName', 'x');
  b.op('set', d, 'nodeType', 3);
  b.op('tagName', d);
});

def('prop/get-text-node-properties', (b) => {
  const t = b.text('abc');
  b.op('get', t, 'nodeValue');
  b.op('get', t, 'textContent');
  b.op('get', t, 'nodeName');
  b.op('get', t, 'nodeType');
  b.op('get', t, 'localName');
  b.op('get', t, 'namespaceURI');
});

def('prop/get-element-properties', (b) => {
  const d = b.el('html', 'DIV');
  b.op('setAttribute', d, 'id', 'k');
  b.op('get', d, 'localName');
  b.op('get', d, 'tagName');
  b.op('get', d, 'namespaceURI');
  b.op('get', d, 'nodeName');
  b.op('get', d, 'nodeType');
  b.op('get', d, 'id');
  b.op('get', d, 'className');
  b.op('get', d, 'title');
  b.op('get', d, 'hidden');
  b.op('get', d, 'tabIndex');
});

def('prop/get-innerhtml-outerhtml', (b) => {
  const d = b.el('html', 'div');
  const s = b.el('html', 'span');
  b.op('append', d, s);
  b.op('append', s, b.text('x'));
  b.op('get', d, 'innerHTML');
  b.op('get', d, 'outerHTML');
});

def('prop/document-properties', (b) => {
  b.op('get', 0, 'nodeName');
  b.op('get', 0, 'nodeType');
  b.op('get', 0, 'textContent');
  b.op('get', 0, 'nodeValue');
  b.op('get', 0, 'isConnected');
});

// class/*: DOMTokenList (classList) over the class attribute: normalization on write, duplicates, errors (WP-U0).
import { def } from '../builder.mjs';

def('class/classlist-reads-attribute', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'class', '  a   b  a ');
  b.op('attributeNames', d);
  b.op('classList', d);
  b.op('getAttribute', d, 'class');
  b.op('get', d, 'className');
});

def('class/add-normalizes-attribute', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'class', '  a   b  a ');
  b.op('classAdd', d, 'c');
  b.op('getAttribute', d, 'class');
});

def('class/remove-missing-still-normalizes', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'class', '  a   b  ');
  b.op('classRemove', d, 'zz');
  b.op('getAttribute', d, 'class');
});

def('class/toggle-remove-then-add-appends', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'class', 'a b c');
  b.op('classToggle', d, 'a');
  b.op('getAttribute', d, 'class');
  b.op('classToggle', d, 'a');
  b.op('getAttribute', d, 'class');
});

def('class/toggle-force-true-keeps-present', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'class', 'b c a');
  b.op('classToggleForce', d, 'a', true);
  b.op('getAttribute', d, 'class');
  b.op('classToggleForce', d, 'new', true);
  b.op('getAttribute', d, 'class');
});

def('class/toggle-force-false-keeps-absent', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'class', 'b c a');
  b.op('classToggleForce', d, 'new', false);
  b.op('getAttribute', d, 'class');
  b.op('classToggleForce', d, 'b', false);
  b.op('getAttribute', d, 'class');
});

def('class/contains-empty-and-spaces-false', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'class', 'a b');
  b.op('classContains', d, 'b');
  b.op('classContains', d, 'B');
  b.op('classContains', d, '');
  b.op('classContains', d, 'a b');
});

def('class/add-multiple-dedupes', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'class', 'b c a');
  b.op('classAdd', d, 'x', 'y', 'x');
  b.op('getAttribute', d, 'class');
  b.op('classRemove', d, 'x', 'y');
  b.op('getAttribute', d, 'class');
});

def('class/classname-whitespace-raw-then-tokens', (b) => {
  const d = b.el('html', 'div');
  b.op('set', d, 'className', ' q\tr\n');
  b.op('getAttribute', d, 'class');
  b.op('classList', d);
});

def('class/remove-absent-creates-nothing', (b) => {
  const d = b.el('html', 'div');
  b.op('classRemove', d, 'nope');
  b.op('hasAttribute', d, 'class');
  b.op('attributeNames', d);
});

def('class/add-no-tokens-creates-attribute', (b) => {
  const d = b.el('html', 'div');
  b.op('classAdd', d);
  b.op('hasAttribute', d, 'class');
  b.op('getAttribute', d, 'class');
});

def('class/toggle-creates-attribute', (b) => {
  const d = b.el('html', 'div');
  b.op('classToggle', d, 't');
  b.op('getAttribute', d, 'class');
});

def('class/add-invalid-tokens-error', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'class', 'a');
  b.op('classAdd', d, '');
  b.op('classAdd', d, 'a b');
  b.op('getAttribute', d, 'class');
});

def('class/toggle-empty-token-error', (b) => {
  const d = b.el('html', 'div');
  b.op('classToggle', d, '');
  b.op('hasAttribute', d, 'class');
});

def('class/remove-leaves-empty-attribute', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'class', 'p');
  b.op('classAdd', d, 'x');
  b.op('classRemove', d, 'p');
  b.op('classRemove', d, 'x');
  b.op('hasAttribute', d, 'class');
  b.op('getAttribute', d, 'class');
});

def('class/svg-class-normalized', (b) => {
  const g = b.el('svg', 'g');
  b.op('setAttribute', g, 'class', 'k  j');
  b.op('classAdd', g, 'z');
  b.op('getAttribute', g, 'class');
});

def('class/uppercase-class-attribute-name', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'CLASS', 'Up');
  b.op('get', d, 'className');
  b.op('classContains', d, 'Up');
});

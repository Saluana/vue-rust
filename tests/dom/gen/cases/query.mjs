// query/*: getElementById and the supported querySelector forms (WP-U0). Only selectors vn-dom supports appear here;
// unsupported forms are unit-tested on the Rust side (UnsupportedSelector).
import { def } from '../builder.mjs';

function sample(b) {
  const main = b.el('html', 'main');
  b.op('append', 0, main);
  const a = b.el('html', 'div');
  b.op('setAttribute', a, 'id', 'x');
  b.op('setAttribute', a, 'class', 'a b');
  b.op('setAttribute', a, 'title', 't');
  const x = b.el('html', 'span');
  b.op('setAttribute', x, 'id', 'x');
  const p = b.el('html', 'p');
  b.op('setAttribute', p, 'data-x', 'v');
  b.op('append', main, p);
  b.op('append', main, a);
  b.op('append', a, x);
  return { main, a, x, p };
}

def('query/get-element-by-id-first-in-tree-order', (b) => {
  const { main, a, x } = sample(b);
  b.op('byId', 'x');
  b.op('byId', 'none');
  b.op('byId', '');
  b.op('querySelector', main, '#x');
  void a;
  void x;
});

def('query/get-element-by-id-detached-not-found', (b) => {
  const d = b.el('html', 'div');
  b.op('setAttribute', d, 'id', 'z');
  b.op('byId', 'z');
  b.op('append', 0, d);
  b.op('byId', 'z');
});

def('query/query-selector-tag-case-insensitive', (b) => {
  const { main } = sample(b);
  b.op('querySelector', main, 'P');
  b.op('querySelector', main, 'section');
  b.op('querySelector', 0, 'DIV');
});

def('query/query-selector-id-and-class', (b) => {
  const { main } = sample(b);
  b.op('querySelector', main, '#x');
  b.op('querySelector', main, '.b');
  b.op('querySelector', main, '.c');
});

def('query/query-selector-attribute-forms', (b) => {
  const { main } = sample(b);
  b.op('querySelector', main, '[data-x]');
  b.op('querySelector', main, '[data-x=v]');
  b.op('querySelector', main, '[data-x=w]');
  b.op('querySelector', main, '[title]');
});

def('query/query-selector-compound', (b) => {
  const { main } = sample(b);
  b.op('querySelector', main, 'div.a[title=t]');
  b.op('querySelector', main, 'div.a[title=u]');
  b.op('querySelector', main, 'span#x');
});

def('query/query-selector-list-document-order', (b) => {
  const { main } = sample(b);
  b.op('querySelector', main, 'span, p');
  b.op('querySelector', main, 'span, em');
});

def('query/query-selector-universal-and-scope', (b) => {
  const { main } = sample(b);
  b.op('querySelector', main, '*');
  b.op('querySelector', main, 'main');
  b.op('querySelector', main, 'main, *');
});

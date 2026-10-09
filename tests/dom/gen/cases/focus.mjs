// focus/*: focusability, focus and blur order, blur during removal and moves, active element (WP-U0).
// Elements under test are attached to the document (focus needs a connected element). Only one element may be a
// child of the document, so every case puts its elements in a container.
import { def, LOG_EV } from '../builder.mjs';

function container(b) {
  const box = b.el('html', 'main');
  b.op('append', 0, box);
  return box;
}

def('focus/focusable-matrix', (b) => {
  const box = container(b);
  const els = [
    ['div', null], ['div', ['tabindex', '0']], ['div', ['tabindex', '-1']], ['div', ['tabindex', 'x']],
    ['div', ['tabindex', ' 3 ']], ['button', null], ['button', ['disabled', '']], ['button', ['disabled', 'false']],
    ['input', null], ['input', ['type', 'hidden']], ['input', ['type', 'TEXT']], ['a', ['href', 'https://example.com/']],
    ['a', null], ['select', null], ['textarea', null], ['iframe', null], ['summary', null], ['img', null], ['span', null],
  ];
  for (const [tag, attr] of els) {
    const e = b.el('html', tag);
    if (attr) b.op('setAttribute', e, attr[0], attr[1]);
    b.op('append', box, e);
    b.op('focus', e);
    b.op('active');
  }
  const r = b.el('svg', 'rect');
  b.op('append', box, r);
  b.op('focus', r);
  b.op('active');
});

def('focus/focus-then-focus-moves-with-events', (b) => {
  const box = container(b);
  const a = b.el('html', 'input');
  const c = b.el('html', 'input');
  b.op('append', box, a);
  b.op('append', box, c);
  b.listen(a, 'blur', 1);
  b.listen(a, 'focusout', 2);
  b.listen(a, 'focus', 3);
  b.listen(a, 'focusin', 4);
  b.listen(c, 'blur', 5);
  b.listen(c, 'focusout', 6);
  b.listen(c, 'focus', 7);
  b.listen(c, 'focusin', 8);
  for (let i = 1; i <= 8; i++) b.on(i, LOG_EV);
  b.op('focus', a);
  b.op('focus', c);
  b.op('blur', c);
  b.op('active');
});

def('focus/blur-not-focused-is-noop', (b) => {
  const box = container(b);
  const a = b.el('html', 'input');
  b.op('append', box, a);
  b.listen(a, 'blur', 1);
  b.on(1, LOG_EV);
  b.op('blur', a);
  b.op('active');
});

def('focus/refocus-same-element-no-events', (b) => {
  const box = container(b);
  const a = b.el('html', 'input');
  b.op('append', box, a);
  b.listen(a, 'focus', 1);
  b.on(1, LOG_EV);
  b.op('focus', a);
  b.op('focus', a);
});

def('focus/detached-element-focus-noop', (b) => {
  const d = b.el('html', 'input');
  b.listen(d, 'focus', 1);
  b.on(1, LOG_EV);
  b.op('focus', d);
  b.op('active');
});

def('focus/active-seen-during-blur-is-null', (b) => {
  const box = container(b);
  const a = b.el('html', 'input');
  const c = b.el('html', 'input');
  b.op('append', box, a);
  b.op('append', box, c);
  b.listen(a, 'blur', 1);
  b.on(1, ['log', 'active'], ['log', 'ev']);
  b.op('focus', a);
  b.op('focus', c);
  b.op('active');
});

def('focus/removal-blurs-focused-descendant-before-detach', (b) => {
  const box = container(b);
  const p = b.el('html', 'section');
  const i = b.el('html', 'input');
  b.op('append', box, p);
  b.op('append', p, i);
  b.listen(i, 'blur', 1);
  b.listen(i, 'focusout', 2);
  b.listen(p, 'focusout', 3);
  b.listen(box, 'focusout', 4);
  b.listen(0, 'focusout', 5);
  b.on(1, LOG_EV, ['log', 'prop', 'isConnected']);
  b.on(2, LOG_EV);
  b.on(3, LOG_EV);
  b.on(4, LOG_EV);
  b.on(5, LOG_EV);
  b.op('focus', i);
  b.op('remove', p);
  b.op('active');
  b.op('isConnected', i);
});

def('focus/move-focused-blurs', (b) => {
  const box = container(b);
  const other = b.el('html', 'div');
  const i = b.el('html', 'input');
  b.op('append', box, other);
  b.op('append', box, i);
  b.listen(i, 'blur', 1);
  b.on(1, LOG_EV);
  b.op('focus', i);
  b.op('insert', other, i, null);
  b.op('active');
});

def('focus/text-replace-blurs-focused-child', (b) => {
  const box = container(b);
  const holder = b.el('html', 'div');
  const i = b.el('html', 'input');
  b.op('append', box, holder);
  b.op('append', holder, i);
  b.listen(i, 'blur', 1);
  b.on(1, LOG_EV);
  b.op('focus', i);
  b.op('setTextContent', holder, 'x');
  b.op('active');
});

def('focus/blur-method-clears-active', (b) => {
  const box = container(b);
  const a = b.el('html', 'input');
  b.op('append', box, a);
  b.op('focus', a);
  b.op('blur', a);
  b.op('active');
});

def('focus/removed-focus-not-restored', (b) => {
  const box = container(b);
  const a = b.el('html', 'input');
  b.op('append', box, a);
  b.op('focus', a);
  b.op('remove', a);
  b.op('active');
  b.op('append', box, a);
  b.op('active');
});

def('focus/click-does-not-move-focus', (b) => {
  const box = container(b);
  const a = b.el('html', 'input');
  b.op('append', box, a);
  b.op('click', a);
  b.op('active');
});

def('focus/document-listener-sees-focusin', (b) => {
  const box = container(b);
  const a = b.el('html', 'input');
  b.op('append', box, a);
  b.listen(0, 'focusin', 1);
  b.on(1, LOG_EV);
  b.op('focus', a);
});

def('focus/focus-of-nested-inside-focused-ancestor', (b) => {
  const box = container(b);
  const outer = b.el('html', 'div');
  const inner = b.el('html', 'span');
  b.op('setAttribute', outer, 'tabindex', '0');
  b.op('setAttribute', inner, 'tabindex', '-1');
  b.op('append', box, outer);
  b.op('append', outer, inner);
  b.op('focus', outer);
  b.op('focus', inner);
  b.op('active');
});

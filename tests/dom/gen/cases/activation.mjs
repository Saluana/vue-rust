// activation/*: the click, input and change order for checkboxes, radios, labels, prevented clicks, disabled controls,
// script click() and select/option (WP-U0 follow-up). Listeners record the event (LOG_EV); the input and change
// listeners also record the checked state they see. Elements sit in a container, since a document has one element.
import { def, LOG_EV } from '../builder.mjs';

const CHECKED = ['log', 'prop', 'checked'];

function box(b) {
  const m = b.el('html', 'div');
  b.op('append', 0, m);
  return m;
}

function checkbox(b, parent, attrs = {}) {
  const c = b.el('html', 'input');
  b.op('setAttribute', c, 'type', 'checkbox');
  for (const [k, v] of Object.entries(attrs)) b.op('setAttribute', c, k, v);
  b.op('append', parent, c);
  return c;
}

function radio(b, parent, name) {
  const r = b.el('html', 'input');
  b.op('setAttribute', r, 'type', 'radio');
  b.op('setAttribute', r, 'name', name);
  b.op('append', parent, r);
  return r;
}

// Click, input and change listeners on one node, with ids from `first`.
function activationListeners(b, node, first) {
  b.listen(node, 'click', first);
  b.listen(node, 'input', first + 1);
  b.listen(node, 'change', first + 2);
  b.on(first, LOG_EV);
  b.on(first + 1, LOG_EV, CHECKED);
  b.on(first + 2, LOG_EV, CHECKED);
}

def('activation/checkbox-click-order', (b) => {
  const m = box(b);
  const c = checkbox(b, m);
  activationListeners(b, c, 1);
  b.op('click', c);
  b.op('get', c, 'checked');
  b.op('click', c);
  b.op('get', c, 'checked');
});

def('activation/checkbox-script-click-and-synthetic-dispatch', (b) => {
  const m = box(b);
  const c = checkbox(b, m);
  activationListeners(b, c, 1);
  b.op('dispatch', c, 'Mouse', 'click', { bubbles: true, cancelable: true });
  b.op('get', c, 'checked');
  b.op('dispatch', c, 'Event', 'click', { bubbles: true, cancelable: true });
  b.op('get', c, 'checked');
});

def('activation/radio-group-switch', (b) => {
  const m = box(b);
  const r1 = radio(b, m, 'g');
  const r2 = radio(b, m, 'g');
  activationListeners(b, r1, 1);
  activationListeners(b, r2, 4);
  b.op('click', r1);
  b.op('click', r2);
  b.op('get', r1, 'checked');
  b.op('get', r2, 'checked');
  b.op('click', r2);
  b.op('get', r2, 'checked');
});

def('activation/label-click-forwards-order', (b) => {
  const m = box(b);
  const lab = b.el('html', 'label');
  b.op('setAttribute', lab, 'for', 'tgt');
  b.op('append', m, lab);
  const tgt = checkbox(b, m, { id: 'tgt' });
  b.listen(lab, 'click', 1);
  b.on(1, LOG_EV);
  activationListeners(b, tgt, 2);
  b.op('click', lab);
  b.op('get', tgt, 'checked');
});

def('activation/label-wrapping-control-click-inside', (b) => {
  const m = box(b);
  const lab = b.el('html', 'label');
  b.op('append', m, lab);
  const inner = checkbox(b, lab);
  b.listen(lab, 'click', 1);
  b.on(1, LOG_EV);
  activationListeners(b, inner, 2);
  b.op('click', inner);
  b.op('get', inner, 'checked');
});

def('activation/prevented-click-fires-no-input-or-change', (b) => {
  const m = box(b);
  const c = checkbox(b, m);
  b.listen(c, 'click', 1);
  b.listen(c, 'input', 2);
  b.listen(c, 'change', 3);
  b.on(1, ['prevent'], LOG_EV);
  b.on(2, LOG_EV);
  b.on(3, LOG_EV);
  b.op('click', c);
  b.op('get', c, 'checked');
});

def('activation/prevented-radio-click-keeps-group', (b) => {
  const m = box(b);
  const r1 = radio(b, m, 'h');
  const r2 = radio(b, m, 'h');
  b.op('click', r1);
  b.listen(r2, 'click', 1);
  b.listen(r2, 'input', 2);
  b.listen(r2, 'change', 3);
  b.on(1, ['prevent'], LOG_EV);
  b.on(2, LOG_EV);
  b.on(3, LOG_EV);
  b.op('click', r2);
  b.op('get', r1, 'checked');
  b.op('get', r2, 'checked');
});

def('activation/input-listener-prevent-does-not-revert', (b) => {
  const m = box(b);
  const c = checkbox(b, m);
  b.listen(c, 'click', 1);
  b.listen(c, 'input', 2);
  b.on(1, LOG_EV);
  b.on(2, ['prevent'], LOG_EV);
  b.op('click', c);
  b.op('get', c, 'checked');
});

def('activation/disabled-checkbox-click-ignored', (b) => {
  const m = box(b);
  const c = checkbox(b, m, { disabled: '' });
  activationListeners(b, c, 1);
  b.op('click', c);
  b.op('get', c, 'checked');
  b.op('dispatch', c, 'Mouse', 'click', { bubbles: true, cancelable: true });
  b.op('get', c, 'checked');
});

def('activation/select-and-option-not-activated', (b) => {
  const m = box(b);
  const sel = b.el('html', 'select');
  const o = b.el('html', 'option');
  b.op('setTextContent', o, 'A');
  b.op('append', sel, o);
  b.op('append', m, sel);
  activationListeners(b, sel, 1);
  activationListeners(b, o, 4);
  b.op('click', o);
  b.op('click', sel);
  b.op('get', sel, 'selectedIndex');
});

def('activation/detached-checkbox-click', (b) => {
  const c = b.el('html', 'input');
  b.op('setAttribute', c, 'type', 'checkbox');
  activationListeners(b, c, 1);
  b.op('click', c);
  b.op('get', c, 'checked');
});

def('activation/input-change-bubble-to-ancestor', (b) => {
  const m = box(b);
  const c = checkbox(b, m);
  activationListeners(b, c, 1);
  b.listen(m, 'input', 4);
  b.listen(m, 'change', 5);
  b.on(4, LOG_EV, CHECKED);
  b.on(5, LOG_EV, CHECKED);
  b.op('click', c);
});

def('activation/radio-already-checked-click', (b) => {
  const m = box(b);
  const r = radio(b, m, 'k');
  b.op('set', r, 'checked', true);
  activationListeners(b, r, 1);
  b.op('click', r);
  b.op('get', r, 'checked');
});

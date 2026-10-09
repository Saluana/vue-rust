// form/*: input value dirty flag and sanitization, checkedness and radio groups, activation behaviour, labels,
// textarea, select/option selectedness, button type (WP-U0). Chrome also fires input/change after a checkbox click;
// these cases never listen for those, because vn-dom does not synthesise them (WP-U0 rule).
import { def, UNDEF, LOG_EV, LOG_KEY } from '../builder.mjs';

def('form/input-value-attribute-default-until-dirty', (b) => {
  const i = b.el('html', 'input');
  b.op('setAttribute', i, 'value', 'a');
  b.op('get', i, 'value');
  b.op('setAttribute', i, 'value', 'b');
  b.op('get', i, 'value');
  b.op('set', i, 'value', 'c');
  b.op('get', i, 'value');
  b.op('getAttribute', i, 'value');
  b.op('setAttribute', i, 'value', 'd');
  b.op('get', i, 'value');
  b.op('set', i, 'defaultValue', 'e');
  b.op('get', i, 'value');
  b.op('getAttribute', i, 'value');
});

def('form/input-value-empty-and-coercion', (b) => {
  const i = b.el('html', 'input');
  b.op('get', i, 'value');
  b.op('getAttribute', i, 'value');
  b.op('set', i, 'value', 5);
  b.op('get', i, 'value');
  b.op('set', i, 'value', null);
  b.op('get', i, 'value');
  b.op('getAttribute', i, 'value');
});

def('form/input-text-strips-newlines', (b) => {
  const i = b.el('html', 'input');
  b.op('set', i, 'value', 'x\ny\r\nz');
  b.op('get', i, 'value');
});

def('form/input-number-sanitized', (b) => {
  const i = b.el('html', 'input');
  b.op('set', i, 'type', 'number');
  b.op('set', i, 'value', 'abc');
  b.op('get', i, 'value');
  b.op('set', i, 'value', '1e3');
  b.op('get', i, 'value');
  b.op('set', i, 'value', ' 5');
  b.op('get', i, 'value');
});

def('form/input-range-default-and-clamp', (b) => {
  const r = b.el('html', 'input');
  b.op('setAttribute', r, 'type', 'range');
  b.op('setAttribute', r, 'min', '0');
  b.op('setAttribute', r, 'max', '100');
  b.op('get', r, 'value');
  b.op('set', r, 'value', '200');
  b.op('get', r, 'value');
  b.op('set', r, 'value', '-5');
  b.op('get', r, 'value');
});

def('form/input-color-sanitized', (b) => {
  const c = b.el('html', 'input');
  b.op('setAttribute', c, 'type', 'color');
  b.op('get', c, 'value');
  b.op('set', c, 'value', '#FF0000');
  b.op('get', c, 'value');
  b.op('set', c, 'value', 'zz');
  b.op('get', c, 'value');
});

def('form/input-hidden-default-mode', (b) => {
  const h = b.el('html', 'input');
  b.op('setAttribute', h, 'type', 'hidden');
  b.op('setAttribute', h, 'value', 'h');
  b.op('get', h, 'value');
  b.op('set', h, 'value', 'z');
  b.op('get', h, 'value');
  b.op('getAttribute', h, 'value');
});

def('form/input-type-change-copies-value', (b) => {
  const i = b.el('html', 'input');
  b.op('set', i, 'value', 'ab');
  b.op('set', i, 'type', 'hidden');
  b.op('getAttribute', i, 'value');
  b.op('get', i, 'value');
  b.op('get', i, 'type');
});

def('form/checkbox-default-value-on', (b) => {
  const c = b.el('html', 'input');
  b.op('setAttribute', c, 'type', 'checkbox');
  b.op('get', c, 'value');
  b.op('get', c, 'checked');
  b.op('get', c, 'defaultChecked');
  b.op('getAttribute', c, 'value');
  b.op('set', c, 'value', 'v1');
  b.op('getAttribute', c, 'value');
  b.op('get', c, 'value');
});

def('form/checkbox-checked-follows-attribute-until-dirty', (b) => {
  const c = b.el('html', 'input');
  b.op('setAttribute', c, 'type', 'checkbox');
  b.op('setAttribute', c, 'checked', '');
  b.op('get', c, 'checked');
  b.op('removeAttribute', c, 'checked');
  b.op('get', c, 'checked');
});

def('form/checkbox-dirty-ignores-attribute', (b) => {
  const c = b.el('html', 'input');
  b.op('setAttribute', c, 'type', 'checkbox');
  b.op('set', c, 'checked', true);
  b.op('removeAttribute', c, 'checked');
  b.op('get', c, 'checked');
  b.op('setAttribute', c, 'checked', '');
  b.op('get', c, 'checked');
  b.op('set', c, 'checked', false);
  b.op('setAttribute', c, 'checked', '');
  b.op('get', c, 'checked');
});

def('form/checkbox-default-checked-reflects', (b) => {
  const c = b.el('html', 'input');
  b.op('setAttribute', c, 'type', 'checkbox');
  b.op('set', c, 'defaultChecked', true);
  b.op('hasAttribute', c, 'checked');
  b.op('get', c, 'checked');
  b.op('set', c, 'defaultChecked', false);
  b.op('get', c, 'checked');
  b.op('hasAttribute', c, 'checked');
});

def('form/checkbox-click-toggles', (b) => {
  const c = b.el('html', 'input');
  b.op('setAttribute', c, 'type', 'checkbox');
  b.op('click', c);
  b.op('get', c, 'checked');
  b.op('click', c);
  b.op('get', c, 'checked');
});

def('form/checkbox-click-prevented-reverts', (b) => {
  const c = b.el('html', 'input');
  b.op('setAttribute', c, 'type', 'checkbox');
  b.listen(c, 'click', 1);
  b.on(1, ['prevent'], ['log', 'prop', 'checked']);
  b.op('click', c);
  b.op('get', c, 'checked');
});

def('form/checkbox-dispatch-noncancelable-keeps', (b) => {
  const c = b.el('html', 'input');
  b.op('setAttribute', c, 'type', 'checkbox');
  b.listen(c, 'click', 1);
  b.on(1, ['prevent'], ['log', 'prop', 'checked']);
  b.op('dispatch', c, 'Mouse', 'click', { bubbles: true, cancelable: false });
  b.op('get', c, 'checked');
});

def('form/checkbox-dispatch-cancelable-reverts', (b) => {
  const c = b.el('html', 'input');
  b.op('setAttribute', c, 'type', 'checkbox');
  b.listen(c, 'click', 1);
  b.on(1, ['log', 'prop', 'checked'], ['prevent']);
  b.op('dispatch', c, 'Mouse', 'click', { bubbles: true, cancelable: true });
  b.op('get', c, 'checked');
});

def('form/checkbox-plain-event-no-activation', (b) => {
  const c = b.el('html', 'input');
  b.op('setAttribute', c, 'type', 'checkbox');
  b.op('dispatch', c, 'Event', 'click', { bubbles: true, cancelable: true });
  b.op('get', c, 'checked');
});

def('form/checkbox-disabled-click-versus-dispatch', (b) => {
  const c = b.el('html', 'input');
  b.op('setAttribute', c, 'type', 'checkbox');
  b.op('setAttribute', c, 'disabled', '');
  b.op('click', c);
  b.op('get', c, 'checked');
  b.op('dispatch', c, 'Mouse', 'click', { bubbles: true, cancelable: true });
  b.op('get', c, 'checked');
});

def('form/radio-group-click-exclusive', (b) => {
  const r1 = b.el('html', 'input');
  const r2 = b.el('html', 'input');
  const r3 = b.el('html', 'input');
  b.op('setAttribute', r1, 'type', 'radio');
  b.op('setAttribute', r1, 'name', 'g');
  b.op('setAttribute', r2, 'type', 'radio');
  b.op('setAttribute', r2, 'name', 'g');
  b.op('setAttribute', r3, 'type', 'radio');
  b.op('setAttribute', r3, 'name', 'h');
  const main = b.el('html', 'main');
  b.op('append', 0, main);
  b.op('append', main, r1);
  b.op('append', main, r2);
  b.op('append', main, r3);
  b.op('click', r1);
  b.op('get', r1, 'checked');
  b.op('click', r2);
  b.op('get', r1, 'checked');
  b.op('get', r2, 'checked');
  b.op('click', r3);
  b.op('get', r2, 'checked');
  b.op('get', r3, 'checked');
});

def('form/radio-checked-property-unchecks-group', (b) => {
  const box = b.el('html', 'div');
  const r1 = b.el('html', 'input');
  const r2 = b.el('html', 'input');
  for (const r of [r1, r2]) {
    b.op('setAttribute', r, 'type', 'radio');
    b.op('setAttribute', r, 'name', 'g');
  }
  b.op('append', 0, box);
  b.op('append', box, r1);
  b.op('append', box, r2);
  b.op('set', r1, 'checked', true);
  b.op('set', r2, 'checked', true);
  b.op('get', r1, 'checked');
  b.op('get', r2, 'checked');
  b.op('set', r2, 'checked', false);
  b.op('get', r1, 'checked');
  b.op('get', r2, 'checked');
});

def('form/radio-insert-checked-wins', (b) => {
  const root = b.el('html', 'div');
  b.op('append', 0, root);
  const rA = b.el('html', 'input');
  b.op('setAttribute', rA, 'type', 'radio');
  b.op('setAttribute', rA, 'name', 'r');
  b.op('append', root, rA);
  const rB = b.el('html', 'input');
  b.op('setAttribute', rB, 'type', 'radio');
  b.op('setAttribute', rB, 'name', 'r');
  b.op('set', rB, 'checked', true);
  b.op('append', root, rB);
  const rC = b.el('html', 'input');
  b.op('setAttribute', rC, 'type', 'radio');
  b.op('setAttribute', rC, 'name', 'r');
  b.op('set', rC, 'checked', true);
  b.op('append', root, rC);
  const rD = b.el('html', 'input');
  b.op('setAttribute', rD, 'type', 'radio');
  b.op('setAttribute', rD, 'name', 'r');
  b.op('set', rD, 'checked', true);
  b.op('insert', root, rD, rB);
  for (const r of [rA, rB, rC, rD]) b.op('get', r, 'checked');
});

def('form/radio-checked-attribute-unchecks', (b) => {
  const box = b.el('html', 'div');
  const rA = b.el('html', 'input');
  const rE = b.el('html', 'input');
  b.op('append', 0, box);
  b.op('setAttribute', rA, 'type', 'radio');
  b.op('setAttribute', rA, 'name', 'r');
  b.op('append', box, rA);
  b.op('set', rA, 'checked', true);
  b.op('setAttribute', rE, 'type', 'radio');
  b.op('setAttribute', rE, 'name', 'r');
  b.op('setAttribute', rE, 'checked', '');
  b.op('append', box, rE);
  b.op('get', rA, 'checked');
  b.op('get', rE, 'checked');
});

def('form/radio-name-change-joins-group', (b) => {
  const box = b.el('html', 'div');
  const rF = b.el('html', 'input');
  const rG = b.el('html', 'input');
  b.op('setAttribute', rF, 'type', 'radio');
  b.op('setAttribute', rG, 'type', 'radio');
  b.op('append', 0, box);
  b.op('append', box, rF);
  b.op('append', box, rG);
  b.op('set', rF, 'checked', true);
  b.op('set', rG, 'checked', true);
  b.op('set', rF, 'name', 'zz');
  b.op('get', rF, 'checked');
  b.op('get', rG, 'checked');
  b.op('set', rG, 'name', 'zz');
  b.op('get', rF, 'checked');
  b.op('get', rG, 'checked');
});

def('form/radio-click-prevented-reverts-group', (b) => {
  const box = b.el('html', 'div');
  const ra = b.el('html', 'input');
  const rb = b.el('html', 'input');
  for (const r of [ra, rb]) {
    b.op('setAttribute', r, 'type', 'radio');
    b.op('setAttribute', r, 'name', 'rv');
  }
  b.op('append', 0, box);
  b.op('append', box, ra);
  b.op('append', box, rb);
  b.op('click', ra);
  b.listen(rb, 'click', 1);
  b.on(1, ['prevent']);
  b.op('click', rb);
  b.op('get', ra, 'checked');
  b.op('get', rb, 'checked');
});

def('form/label-for-forwards-click', (b) => {
  const box = b.el('html', 'div');
  const lab = b.el('html', 'label');
  const tgt = b.el('html', 'input');
  b.op('setAttribute', lab, 'for', 'tgt');
  b.op('setAttribute', tgt, 'type', 'checkbox');
  b.op('setAttribute', tgt, 'id', 'tgt');
  b.op('append', 0, box);
  b.op('append', box, lab);
  b.op('append', box, tgt);
  b.listen(lab, 'click', 1);
  b.listen(tgt, 'click', 2);
  b.on(1, LOG_EV);
  b.on(2, LOG_EV);
  b.op('click', lab);
  b.op('get', tgt, 'checked');
});

def('form/label-wrapping-forwards-and-toggles-back', (b) => {
  const lab = b.el('html', 'label');
  const inner = b.el('html', 'input');
  b.op('setAttribute', inner, 'type', 'checkbox');
  b.op('append', lab, inner);
  b.op('append', 0, lab);
  b.op('click', lab);
  b.op('get', inner, 'checked');
  b.op('click', lab);
  b.op('get', inner, 'checked');
});

def('form/label-prevented-no-forward', (b) => {
  const lab = b.el('html', 'label');
  const inner = b.el('html', 'input');
  b.op('setAttribute', inner, 'type', 'checkbox');
  b.op('append', lab, inner);
  b.op('append', 0, lab);
  b.listen(lab, 'click', 3, { capture: true });
  b.on(3, ['prevent']);
  b.op('click', lab);
  b.op('get', inner, 'checked');
});

def('form/label-disabled-control-no-forward', (b) => {
  const lab = b.el('html', 'label');
  const inner = b.el('html', 'input');
  b.op('setAttribute', inner, 'type', 'checkbox');
  b.op('setAttribute', inner, 'disabled', '');
  b.op('append', lab, inner);
  b.op('append', 0, lab);
  b.op('click', lab);
  b.op('get', inner, 'checked');
});

def('form/label-click-on-control-toggles-once', (b) => {
  const lab = b.el('html', 'label');
  const inner = b.el('html', 'input');
  b.op('setAttribute', inner, 'type', 'checkbox');
  b.op('append', lab, inner);
  b.op('append', 0, lab);
  b.op('click', inner);
  b.op('get', inner, 'checked');
});

def('form/textarea-value-and-default', (b) => {
  const ta = b.el('html', 'textarea');
  b.op('setTextContent', ta, 'def');
  b.op('get', ta, 'value');
  b.op('get', ta, 'defaultValue');
  b.op('set', ta, 'value', 'a\r\nb\rc');
  b.op('get', ta, 'value');
  b.op('get', ta, 'textContent');
  b.op('setTextContent', ta, 'new');
  b.op('get', ta, 'value');
  b.op('get', ta, 'defaultValue');
});

def('form/textarea-default-value-setter-and-text', (b) => {
  const ta = b.el('html', 'textarea');
  b.op('set', ta, 'defaultValue', 'dv');
  b.op('get', ta, 'value');
  b.op('get', ta, 'textContent');
  const t = b.text('txt');
  b.op('append', ta, t);
  b.op('get', ta, 'textContent');
  b.op('get', ta, 'value');
});

def('form/select-first-option-selected-by-default', (b) => {
  const sel = b.el('html', 'select');
  b.op('append', 0, sel);
  const o0 = b.el('html', 'option');
  b.op('setTextContent', o0, ' A  b ');
  b.op('append', sel, o0);
  const o1 = b.el('html', 'option');
  b.op('setAttribute', o1, 'value', 'v1');
  b.op('setTextContent', o1, 'B');
  b.op('append', sel, o1);
  const o2 = b.el('html', 'option');
  b.op('setTextContent', o2, 'C');
  b.op('append', sel, o2);
  b.op('get', sel, 'selectedIndex');
  b.op('get', sel, 'value');
  b.op('get', o0, 'value');
  b.op('get', o0, 'text');
  b.op('get', o0, 'selected');
  b.op('get', o1, 'selected');
});

def('form/select-option-setter-deselects-others', (b) => {
  const sel = b.el('html', 'select');
  b.op('append', 0, sel);
  const o0 = b.el('html', 'option');
  const o1 = b.el('html', 'option');
  const o2 = b.el('html', 'option');
  b.op('append', sel, o0);
  b.op('append', sel, o1);
  b.op('append', sel, o2);
  b.op('set', o2, 'selected', true);
  b.op('get', sel, 'selectedIndex');
  b.op('get', o0, 'selected');
  b.op('get', o2, 'selected');
  b.op('set', o0, 'selected', true);
  b.op('get', sel, 'selectedIndex');
  b.op('get', o2, 'selected');
});

def('form/select-selectedindex-setter-range', (b) => {
  const sel = b.el('html', 'select');
  b.op('append', 0, sel);
  const o0 = b.el('html', 'option');
  const o1 = b.el('html', 'option');
  b.op('setAttribute', o1, 'value', 'v1');
  b.op('append', sel, o0);
  b.op('append', sel, o1);
  b.op('set', sel, 'selectedIndex', 1);
  b.op('get', sel, 'value');
  b.op('set', sel, 'selectedIndex', 9);
  b.op('get', sel, 'selectedIndex');
  b.op('get', sel, 'value');
  b.op('set', sel, 'selectedIndex', -1);
  b.op('get', sel, 'selectedIndex');
});

def('form/select-value-setter', (b) => {
  const sel = b.el('html', 'select');
  b.op('append', 0, sel);
  const o0 = b.el('html', 'option');
  b.op('setTextContent', o0, 'A');
  const o1 = b.el('html', 'option');
  b.op('setAttribute', o1, 'value', 'v1');
  const o2 = b.el('html', 'option');
  b.op('setTextContent', o2, 'C');
  b.op('append', sel, o0);
  b.op('append', sel, o1);
  b.op('append', sel, o2);
  b.op('set', sel, 'value', 'v1');
  b.op('get', sel, 'selectedIndex');
  b.op('set', sel, 'value', 'nope');
  b.op('get', sel, 'selectedIndex');
  b.op('get', sel, 'value');
  b.op('set', sel, 'value', 'C');
  b.op('get', sel, 'selectedIndex');
});

def('form/select-removing-selected-resets', (b) => {
  const sel = b.el('html', 'select');
  b.op('append', 0, sel);
  const o0 = b.el('html', 'option');
  const o2 = b.el('html', 'option');
  b.op('append', sel, o0);
  b.op('append', sel, o2);
  b.op('set', o2, 'selected', true);
  b.op('removeChild', sel, o2);
  b.op('get', sel, 'selectedIndex');
  b.op('get', o0, 'selected');
});

def('form/select-skips-disabled-when-resetting', (b) => {
  const sel = b.el('html', 'select');
  b.op('append', 0, sel);
  const d0 = b.el('html', 'option');
  b.op('setAttribute', d0, 'disabled', '');
  const d1 = b.el('html', 'option');
  b.op('append', sel, d0);
  b.op('append', sel, d1);
  b.op('get', sel, 'selectedIndex');
  b.op('get', d0, 'selected');
  b.op('get', d1, 'selected');
});

def('form/select-empty', (b) => {
  const sel = b.el('html', 'select');
  b.op('append', 0, sel);
  b.op('get', sel, 'selectedIndex');
  b.op('get', sel, 'value');
  b.op('options', sel);
});

def('form/select-multiple-keeps-selection', (b) => {
  const m = b.el('html', 'select');
  b.op('setAttribute', m, 'multiple', '');
  b.op('append', 0, m);
  const m0 = b.el('html', 'option');
  const m1 = b.el('html', 'option');
  b.op('setTextContent', m0, 'a');
  b.op('setTextContent', m1, 'b');
  b.op('append', m, m0);
  b.op('append', m, m1);
  b.op('get', m, 'selectedIndex');
  b.op('set', m0, 'selected', true);
  b.op('set', m1, 'selected', true);
  b.op('get', m0, 'selected');
  b.op('get', m1, 'selected');
  b.op('get', m, 'selectedIndex');
  b.op('set', m, 'selectedIndex', 0);
  b.op('get', m0, 'selected');
  b.op('get', m1, 'selected');
  b.op('removeAttribute', m, 'multiple');
  b.op('get', m, 'selectedIndex');
});

def('form/option-value-and-text-normalized', (b) => {
  const ov = b.el('html', 'option');
  b.op('setTextContent', ov, '  hi\n there  ');
  b.op('get', ov, 'value');
  b.op('get', ov, 'text');
});

def('form/option-value-setter-and-text-setter', (b) => {
  const ov = b.el('html', 'option');
  b.op('setTextContent', ov, '  hi\n there  ');
  b.op('setAttribute', ov, 'value', '');
  b.op('get', ov, 'value');
  b.op('set', ov, 'value', 'V');
  b.op('getAttribute', ov, 'value');
  b.op('get', ov, 'value');
  b.op('set', ov, 'text', 'new text');
  b.op('get', ov, 'text');
  b.op('get', ov, 'value');
});

def('form/option-selected-attribute-last-inserted-wins', (b) => {
  const s = b.el('html', 'select');
  b.op('append', 0, s);
  const o1 = b.el('html', 'option');
  b.op('setTextContent', o1, 'a');
  b.op('set', o1, 'selected', true);
  b.op('append', s, o1);
  const o2 = b.el('html', 'option');
  b.op('setAttribute', o2, 'selected', '');
  b.op('setTextContent', o2, 'b');
  b.op('append', s, o2);
  b.op('get', o1, 'selected');
  b.op('get', o2, 'selected');
  b.op('get', s, 'selectedIndex');
});

def('form/option-inserted-selected-wins-then-removal-resets', (b) => {
  const s = b.el('html', 'select');
  b.op('append', 0, s);
  const o1 = b.el('html', 'option');
  const o2 = b.el('html', 'option');
  b.op('append', s, o1);
  b.op('append', s, o2);
  const o3 = b.el('html', 'option');
  b.op('setTextContent', o3, 'c');
  b.op('set', o3, 'selected', true);
  b.op('insert', s, o3, o1);
  b.op('get', o3, 'selected');
  b.op('get', o1, 'selected');
  b.op('get', s, 'selectedIndex');
  b.op('removeChild', s, o3);
  b.op('get', o1, 'selected');
  b.op('get', s, 'selectedIndex');
});

def('form/option-setattribute-selected-after-dirty-select', (b) => {
  const s = b.el('html', 'select');
  b.op('append', 0, s);
  const q0 = b.el('html', 'option');
  b.op('setAttribute', q0, 'selected', '');
  const q1 = b.el('html', 'option');
  b.op('append', s, q0);
  b.op('append', s, q1);
  b.op('set', q1, 'selected', true);
  b.op('get', q0, 'selected');
  b.op('get', q1, 'selected');
  b.op('setAttribute', q0, 'selected', '');
  b.op('get', q0, 'selected');
  b.op('get', q1, 'selected');
  b.op('removeAttribute', q0, 'selected');
  b.op('get', q0, 'selected');
  b.op('get', q1, 'selected');
});

def('form/option-selected-false-resets-to-first', (b) => {
  const s = b.el('html', 'select');
  b.op('append', 0, s);
  const a = b.el('html', 'option');
  const x = b.el('html', 'option');
  b.op('append', s, a);
  b.op('append', s, x);
  b.op('set', a, 'selected', true);
  b.op('set', a, 'selected', false);
  b.op('get', a, 'selected');
  b.op('get', s, 'selectedIndex');
});

def('form/button-type-normalized', (b) => {
  const bt = b.el('html', 'button');
  b.op('get', bt, 'type');
  b.op('getAttribute', bt, 'type');
  b.op('set', bt, 'type', 'RESET');
  b.op('get', bt, 'type');
  b.op('getAttribute', bt, 'type');
  b.op('set', bt, 'type', 'bogus');
  b.op('get', bt, 'type');
});

def('form/submit-button-click-does-nothing-extra', (b) => {
  const bt = b.el('html', 'button');
  b.op('append', 0, bt);
  b.listen(bt, 'click', 1);
  b.on(1, LOG_EV);
  b.op('click', bt);
  b.op('get', bt, 'type');
});

def('form/text-input-checked-attribute', (b) => {
  const i = b.el('html', 'input');
  b.op('setAttribute', i, 'checked', '');
  b.op('get', i, 'checked');
  b.op('set', i, 'type', 'checkbox');
  b.op('get', i, 'checked');
});

def('form/keyboard-payload-on-input', (b) => {
  const i = b.el('html', 'input');
  b.op('append', 0, i);
  b.listen(i, 'keydown', 1);
  b.on(1, LOG_KEY);
  b.op('dispatch', i, 'Keyboard', 'keydown', { key: 'a', code: 'KeyA', bubbles: true });
});

// Case matrix of the patchProp oracle (WP-RT-D). Deterministic: the same cases in the same order on every run.
// Each case mounts one element, applies a sequence of patchProp transitions and records what Chrome observes.
const U = { $: 'u' };
const H = (n) => ({ $: 'fn', n });

const HANDLERS = {
  h1: { log: ['type', 'target', 'currentTarget', 'eventPhase', 'defaultPrevented'], then: [] },
  h2: { log: ['type', 'target', 'currentTarget', 'eventPhase', 'defaultPrevented'], then: ['stopPropagation'] },
  h3: { log: ['type', 'target', 'currentTarget', 'eventPhase'], then: ['stopImmediatePropagation'] },
  h4: { log: ['type', 'target', 'currentTarget', 'defaultPrevented'], then: ['preventDefault'] },
  h5: { log: ['type', 'target'], then: ['throw'] },
  hp: { log: ['type', 'target'], then: [{ do: 'patch', el: 'p', key: 'onClick', prev: null, next: H('h1') }] },
  hk: { log: ['type', 'key', 'code', 'ctrlKey', 'shiftKey', 'isComposing'], then: [] },
  hi: { log: ['type', 'data', 'isComposing', 'detail'], then: [] },
  hm: { log: ['type', 'button', 'clientX', 'clientY', 'detail', 'ctrlKey', 'key'], then: [] },
  hn: { log: ['type', 'key', 'button', 'data', 'target'], then: [] },
};

const cases = [];
function add(name, steps, handlers = HANDLERS) {
  cases.push({ name, handlers, steps });
}

// ---- A: class (html and svg), every pair of values ----
const CLASS_VALUES = [null, U, '', 'a', 'a b', '  c  d '];
for (const [tag, ns] of [['div', 'html'], ['circle', 'svg']]) {
  for (const p of CLASS_VALUES) {
    for (const n of CLASS_VALUES) {
      const nsx = ns === 'html' ? undefined : ns;
      add(`class/${tag}/${JSON.stringify(p)}->${JSON.stringify(n)}`, [
        { op: 'create', id: 'e', tag, ns },
        { op: 'mount', id: 'e' },
        { op: 'patch', el: 'e', key: 'class', prev: null, next: p, ns: nsx },
        { op: 'snap', el: 'e' },
        { op: 'patch', el: 'e', key: 'class', prev: p, next: n, ns: nsx },
        { op: 'snap', el: 'e' },
      ]);
    }
  }
}

// ---- B: style, every pair of a value set ----
const STYLE_VALUES = [
  null,
  '',
  'color: red',
  'color: red; display: none',
  { color: 'red', 'font-size': '12px' },
  { color: null, fontSize: '3px' },
  { '--x': '1px', color: 'blue' },
  { color: 'red !important' },
  { display: ['-webkit-box', 'flex'] },
];
for (const p of STYLE_VALUES) {
  for (const n of STYLE_VALUES) {
    add(`style/${JSON.stringify(p)}->${JSON.stringify(n)}`, [
      { op: 'create', id: 'e', tag: 'div' },
      { op: 'mount', id: 'e' },
      { op: 'patch', el: 'e', key: 'style', prev: null, next: p },
      { op: 'snap', el: 'e' },
      { op: 'patch', el: 'e', key: 'style', prev: p, next: n },
      { op: 'snap', el: 'e' },
    ]);
  }
}

// ---- C: attributes (generic attribute path and boolean attributes) ----
const ATTR_VALUES = [null, U, '', 'x', 0, true, false];
for (const key of ['id', 'title', 'data-x', 'tabindex', 'hidden', 'aria-label', 'draggable', 'role', 'spellcheck']) {
  for (const p of ATTR_VALUES) {
    for (const n of ATTR_VALUES) {
      add(`attr/${key}/${JSON.stringify(p)}->${JSON.stringify(n)}`, [
        { op: 'create', id: 'e', tag: 'div' },
        { op: 'mount', id: 'e' },
        { op: 'patch', el: 'e', key, prev: null, next: p },
        { op: 'snap', el: 'e' },
        { op: 'patch', el: 'e', key, prev: p, next: n },
        { op: 'snap', el: 'e' },
      ]);
    }
  }
}

// ---- D: DOM properties of form controls and other special cases ----
const VALUE_SET = [null, U, '', 'a', 0, true, false];
for (const p of VALUE_SET) {
  for (const n of VALUE_SET) {
    add(`dom/input-value/${JSON.stringify(p)}->${JSON.stringify(n)}`, [
      { op: 'create', id: 'e', tag: 'input' },
      { op: 'mount', id: 'e' },
      { op: 'patch', el: 'e', key: 'value', prev: null, next: p },
      { op: 'snap', el: 'e' },
      { op: 'patch', el: 'e', key: 'value', prev: p, next: n },
      { op: 'snap', el: 'e' },
    ]);
    add(`dom/textarea-value/${JSON.stringify(p)}->${JSON.stringify(n)}`, [
      { op: 'create', id: 'e', tag: 'textarea' },
      { op: 'mount', id: 'e' },
      { op: 'patch', el: 'e', key: 'value', prev: null, next: p },
      { op: 'snap', el: 'e' },
      { op: 'patch', el: 'e', key: 'value', prev: p, next: n },
      { op: 'snap', el: 'e' },
    ]);
  }
}
const CHECKED_VALUES = [true, false, null, U, 'x', 0];
for (const p of CHECKED_VALUES) {
  for (const n of CHECKED_VALUES) {
    add(`dom/checkbox-checked/${JSON.stringify(p)}->${JSON.stringify(n)}`, [
      { op: 'create', id: 'e', tag: 'input' },
      { op: 'mount', id: 'e' },
      { op: 'patch', el: 'e', key: 'type', prev: null, next: 'checkbox' },
      { op: 'patch', el: 'e', key: 'checked', prev: null, next: p },
      { op: 'snap', el: 'e' },
      { op: 'patch', el: 'e', key: 'checked', prev: p, next: n },
      { op: 'snap', el: 'e' },
    ]);
  }
}
for (const p of [true, false, null, U]) {
  for (const n of [true, false, null, U, 'selected']) {
    add(`dom/option-selected/${JSON.stringify(p)}->${JSON.stringify(n)}`, [
      { op: 'create', id: 's', tag: 'select' },
      { op: 'create', id: 'o', tag: 'option' },
      { op: 'mount', id: 's' },
      { op: 'mount', id: 'o', parent: 's' },
      { op: 'patch', el: 'o', key: 'selected', prev: null, next: p },
      { op: 'snap', el: 'o' },
      { op: 'patch', el: 'o', key: 'selected', prev: p, next: n },
      { op: 'snap', el: 'o' },
    ]);
  }
}
for (const p of [null, U, '', 'x', true]) {
  for (const n of [null, U, '', 'x', false]) {
    add(`dom/button-disabled/${JSON.stringify(p)}->${JSON.stringify(n)}`, [
      { op: 'create', id: 'e', tag: 'button' },
      { op: 'mount', id: 'e' },
      { op: 'patch', el: 'e', key: 'disabled', prev: null, next: p },
      { op: 'snap', el: 'e' },
      { op: 'patch', el: 'e', key: 'disabled', prev: p, next: n },
      { op: 'snap', el: 'e' },
    ]);
  }
}
for (const p of [null, U, '', 'a']) {
  for (const n of [null, U, '', 'b', 'c']) {
    add(`dom/textcontent/${JSON.stringify(p)}->${JSON.stringify(n)}`, [
      { op: 'create', id: 'e', tag: 'div' },
      { op: 'mount', id: 'e' },
      { op: 'patch', el: 'e', key: 'textContent', prev: null, next: p },
      { op: 'snap', el: 'e' },
      { op: 'patch', el: 'e', key: 'textContent', prev: p, next: n },
      { op: 'snap', el: 'e' },
    ]);
  }
}
for (const key of ['.value', '.hidden', '.title', '^id', '^class', '.tabIndex', '^data-x']) {
  for (const [p, n] of [[null, 'x'], ['x', 'y'], ['y', null], ['z', U], [U, true], [false, 'w']]) {
    add(`dom/modifier/${key}/${JSON.stringify(p)}->${JSON.stringify(n)}`, [
      { op: 'create', id: 'e', tag: 'input' },
      { op: 'mount', id: 'e' },
      { op: 'patch', el: 'e', key, prev: null, next: p },
      { op: 'snap', el: 'e' },
      { op: 'patch', el: 'e', key, prev: p, next: n },
      { op: 'snap', el: 'e' },
    ]);
  }
}
for (const tag of ['img', 'canvas', 'div']) {
  for (const p of [null, U, '10', 10]) {
    for (const n of [null, '20', 30, U]) {
      add(`dom/size/${tag}/${JSON.stringify(p)}->${JSON.stringify(n)}`, [
        { op: 'create', id: 'e', tag },
        { op: 'mount', id: 'e' },
        { op: 'patch', el: 'e', key: 'width', prev: null, next: p },
        { op: 'patch', el: 'e', key: 'height', prev: null, next: p },
        { op: 'snap', el: 'e' },
        { op: 'patch', el: 'e', key: 'width', prev: p, next: n },
        { op: 'patch', el: 'e', key: 'height', prev: p, next: n },
        { op: 'snap', el: 'e' },
      ]);
    }
  }
}
for (const key of ['form', 'list', 'sandbox', 'type', 'readonly', 'translate']) {
  for (const p of [null, '', 'x', true]) {
    add(`dom/attr-special/${key}/${JSON.stringify(p)}`, [
      { op: 'create', id: 'e', tag: key === 'sandbox' ? 'iframe' : key === 'list' ? 'input' : 'input' },
      { op: 'mount', id: 'e' },
      { op: 'patch', el: 'e', key, prev: null, next: p },
      { op: 'snap', el: 'e' },
    ]);
  }
}
for (const key of ['true-value', 'false-value']) {
  for (const n of ['yes', null]) {
    add(`dom/value-attrs/${key}/${JSON.stringify(n)}`, [
      { op: 'create', id: 'e', tag: 'input' },
      { op: 'mount', id: 'e' },
      { op: 'patch', el: 'e', key, prev: null, next: n },
      { op: 'snap', el: 'e' },
    ]);
  }
}
// readonly is a special boolean attribute: its value decides presence, not content
for (const p of [true, false, null, '', 'x']) {
  add(`dom/readonly/${JSON.stringify(p)}`, [
    { op: 'create', id: 'e', tag: 'input' },
    { op: 'mount', id: 'e' },
    { op: 'patch', el: 'e', key: 'readonly', prev: null, next: p },
    { op: 'snap', el: 'e' },
  ]);
}
// svg style and xlink attributes
for (const n of ['fill: red', { fill: 'blue' }]) {
  add(`svg/style/${JSON.stringify(n)}`, [
    { op: 'create', id: 'e', tag: 'circle', ns: 'svg' },
    { op: 'mount', id: 'e' },
    { op: 'patch', el: 'e', key: 'style', prev: null, next: n, ns: 'svg' },
    { op: 'snap', el: 'e' },
  ]);
}
for (const n of ['#icon', null, U]) {
  add(`svg/xlink/${JSON.stringify(n)}`, [
    { op: 'create', id: 'e', tag: 'use', ns: 'svg' },
    { op: 'mount', id: 'e' },
    { op: 'patch', el: 'e', key: 'xlink:href', prev: null, next: '#a', ns: 'svg' },
    { op: 'snap', el: 'e' },
    { op: 'patch', el: 'e', key: 'xlink:href', prev: '#a', next: n, ns: 'svg' },
    { op: 'snap', el: 'e' },
  ]);
}

// ---- E: events (listeners attached, replaced, removed, option modifiers, propagation) ----
const btn = (extra = []) => [
  { op: 'create', id: 'p', tag: 'div' },
  { op: 'create', id: 'b', tag: 'button' },
  { op: 'mount', id: 'p' },
  { op: 'mount', id: 'b', parent: 'p' },
  ...extra,
];
const CLICK = { kind: 'Mouse', type: 'click', bubbles: true, cancelable: true, composed: true, init: {} };
const dispatch = (el, event = CLICK) => ({ op: 'dispatch', el, event });
const LOG = { op: 'log' };
add('event/attach-bubble-stop', btn([
  { op: 'patch', el: 'p', key: 'onClick', prev: null, next: H('h1') },
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: H('h2') },
  dispatch('b'), LOG,
]));
add('event/replace-handler', btn([
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: H('h1') },
  dispatch('b'), LOG,
  { op: 'patch', el: 'b', key: 'onClick', prev: H('h1'), next: H('h2') },
  dispatch('b'), LOG,
]));
add('event/remove-handler', btn([
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: H('h1') },
  dispatch('b'), LOG,
  { op: 'patch', el: 'b', key: 'onClick', prev: H('h1'), next: null },
  dispatch('b'), LOG,
]));
add('event/array-handlers', btn([
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: [H('h1'), H('h2')] },
  dispatch('b'), LOG,
]));
add('event/array-stop-immediate', btn([
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: [H('h3'), H('h1')] },
  dispatch('b'), LOG,
]));
add('event/array-null-entry', btn([
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: [null, H('h1'), U, H('h2')] },
  dispatch('b'), LOG,
]));
add('event/once', btn([
  { op: 'patch', el: 'b', key: 'onClickOnce', prev: null, next: H('h1') },
  dispatch('b'), LOG, dispatch('b'), LOG,
]));
add('event/capture-before-target', btn([
  { op: 'patch', el: 'p', key: 'onClickCapture', prev: null, next: H('h1') },
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: H('h2') },
  dispatch('b'), LOG,
]));
add('event/capture-and-bubble-order', btn([
  { op: 'patch', el: 'p', key: 'onClick', prev: null, next: H('h1') },
  { op: 'patch', el: 'p', key: 'onClickCapture', prev: null, next: H('h2') },
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: H('h1') },
  dispatch('b'), LOG,
]));
add('event/passive-prevent-ignored', btn([
  { op: 'patch', el: 'b', key: 'onClickPassive', prev: null, next: H('h4') },
  dispatch('b'), LOG,
]));
add('event/prevent-cancelable', btn([
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: H('h4') },
  dispatch('b'), LOG,
  dispatch('b', { ...CLICK, cancelable: false }), LOG,
]));
add('event/throw-continues', btn([
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: H('h5') },
  { op: 'patch', el: 'p', key: 'onClick', prev: null, next: H('h1') },
  dispatch('b'), LOG,
]));
add('event/keyboard-fields', btn([
  { op: 'patch', el: 'b', key: 'onKeydown', prev: null, next: H('hk') },
  dispatch('b', { kind: 'Keyboard', type: 'keydown', bubbles: true, cancelable: true, composed: true, init: { key: 'Enter', code: 'Enter', ctrlKey: true, shiftKey: false } }), LOG,
]));
add('event/input-fields', btn([
  { op: 'patch', el: 'b', key: 'onInput', prev: null, next: H('hi') },
  dispatch('b', { kind: 'Input', type: 'input', bubbles: true, cancelable: false, composed: true, init: { data: 'x', isComposing: true } }), LOG,
]));
add('event/mouse-fields', btn([
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: H('hm') },
  dispatch('b', { kind: 'Mouse', type: 'click', bubbles: true, cancelable: true, composed: true, init: { button: 1, clientX: 5, clientY: 7, detail: 2, ctrlKey: true } }), LOG,
]));
add('event/absent-fields-plain-event', btn([
  { op: 'patch', el: 'b', key: 'onPing', prev: null, next: H('hn') },
  dispatch('b', { kind: 'Event', type: 'ping', bubbles: false, cancelable: false, composed: false, init: {} }), LOG,
  dispatch('b', { kind: 'Mouse', type: 'ping', bubbles: false, cancelable: false, composed: false, init: { button: 2 } }), LOG,
]));
add('event/handler-attached-during-dispatch-is-skipped', btn([
  { op: 'time', ms: 1000 },
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: H('hp') },
  { op: 'time', ms: 2000 },
  dispatch('b'), LOG,
]));
add('event/handler-attached-before-dispatch-runs', btn([
  { op: 'time', ms: 1000 },
  { op: 'patch', el: 'p', key: 'onClick', prev: null, next: H('h1') },
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: H('h2') },
  { op: 'time', ms: 2000 },
  dispatch('b'), LOG,
]));
add('event/dom-prop-on-with-string-is-attribute', btn([
  { op: 'patch', el: 'b', key: 'onclick', prev: null, next: 'x = 1' },
  { op: 'snap', el: 'b' },
]));
add('event/update-keeps-listener-count', btn([
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: H('h1') },
  { op: 'patch', el: 'b', key: 'onClick', prev: H('h1'), next: H('h1') },
  { op: 'patch', el: 'b', key: 'onClick', prev: H('h1'), next: [H('h1')] },
  dispatch('b'), LOG,
]));
add('event/handler-patches-another-during-dispatch', [
  { op: 'create', id: 'p', tag: 'div' },
  { op: 'create', id: 'b', tag: 'button' },
  { op: 'mount', id: 'p' },
  { op: 'mount', id: 'b', parent: 'p' },
  { op: 'time', ms: 500 },
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: H('hp') },
  { op: 'time', ms: 900 },
  dispatch('b'), LOG,
  { op: 'snap', el: 'p' },
]);
add('event/set-onclick-property-on-dispatch-ignored', btn([
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: H('h1') },
  { op: 'setProp', el: 'b', key: 'value', value: 'x' },
  dispatch('b'), LOG,
]));
add('event/remove-then-readd-same-name', btn([
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: H('h1') },
  { op: 'patch', el: 'b', key: 'onClick', prev: H('h1'), next: null },
  { op: 'patch', el: 'b', key: 'onClick', prev: null, next: H('h2') },
  dispatch('b'), LOG,
]));

export { cases, HANDLERS };

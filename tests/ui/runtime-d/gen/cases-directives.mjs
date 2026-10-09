// Case matrix of the directive oracle (WP-RT-D): v-model (text, textarea, checkbox, radio, select, dynamic), v-show,
// withModifiers and withKeys. Deterministic: the same cases in the same order on every run.
const U = { $: 'u' };
const H = (n) => ({ $: 'fn', n });
const REF = (id) => ({ $: 'ref', id });
const LOG = { op: 'log' };
const F = (n) => ({ n });

const HANDLERS = {
  assign: { log: ['$'], then: [] },
  h1: { log: ['type', 'target', 'currentTarget', 'eventPhase', 'button', 'ctrlKey', 'shiftKey', 'altKey', 'metaKey', 'key'], then: [] },
  h2: { log: ['type', 'target', 'currentTarget', 'eventPhase'], then: [] },
  hk: { log: ['type', 'key', 'code', 'ctrlKey'], then: [] },
};

const cases = [];
function add(name, steps) {
  cases.push({ name, handlers: HANDLERS, steps });
}
const modObj = (mods) => Object.fromEntries(mods.map((m) => [m, true]));
const assignProps = (extra = {}) => ({ 'onUpdate:modelValue': H('assign'), ...extra });

// ---- V1: v-model text on input, with every modifier combination; typing, change, then an update ----
const MODS = [[], ['lazy'], ['trim'], ['number'], ['lazy', 'trim'], ['lazy', 'number'], ['trim', 'number'], ['lazy', 'trim', 'number']];
const TYPED = ['abc', ' a b ', '42', '3.50', '', 'x y'];
for (const mods of MODS) {
  for (const typed of TYPED) {
    const init = 'init';
    const vnode = { props: assignProps(mods.includes('number') ? { type: 'text' } : {}) };
    const binding = (value, oldValue) => ({ value, oldValue, modifiers: modObj(mods) });
    add(`v-model/text/${mods.join('.') || 'none'}/${JSON.stringify(typed)}`, [
      { op: 'create', id: 'e', tag: 'input' },
      { op: 'mount', id: 'e' },
      { op: 'directive', name: 'vModelText', hook: 'created', el: 'e', binding: binding(init), vnode },
      { op: 'directive', name: 'vModelText', hook: 'mounted', el: 'e', binding: binding(init), vnode },
      { op: 'snap', el: 'e' },
      { op: 'type', el: 'e', value: typed },
      LOG,
      { op: 'fire', el: 'e', type: 'change' },
      LOG,
      { op: 'directive', name: 'vModelText', hook: 'beforeUpdate', el: 'e', binding: binding('model', init), vnode },
      { op: 'snap', el: 'e' },
      LOG,
    ]);
  }
}
// The same, with an update to the value the user typed (the element already shows it: no write).
for (const mods of MODS) {
  for (const typed of TYPED) {
    const vnode = { props: assignProps() };
    const binding = (value, oldValue) => ({ value, oldValue, modifiers: modObj(mods) });
    add(`v-model/text-same-update/${mods.join('.') || 'none'}/${JSON.stringify(typed)}`, [
      { op: 'create', id: 'e', tag: 'input' },
      { op: 'mount', id: 'e' },
      { op: 'directive', name: 'vModelText', hook: 'created', el: 'e', binding: binding('x'), vnode },
      { op: 'directive', name: 'vModelText', hook: 'mounted', el: 'e', binding: binding('x'), vnode },
      { op: 'type', el: 'e', value: typed },
      { op: 'fire', el: 'e', type: 'change' },
      LOG,
      { op: 'directive', name: 'vModelText', hook: 'beforeUpdate', el: 'e', binding: binding(typed, 'x'), vnode },
      { op: 'snap', el: 'e' },
    ]);
  }
}
// ---- V2: IME composition: input during composition is ignored, the composition end reports the value ----
for (const mods of [[], ['trim'], ['number'], ['lazy']]) {
  for (const seq of [
    [{ op: 'compose', el: 'e', phase: 'start' }, { op: 'type', el: 'e', value: 'ni' }, { op: 'compose', el: 'e', phase: 'end' }],
    [{ op: 'compose', el: 'e', phase: 'start' }, { op: 'type', el: 'e', value: 'x' }, { op: 'type', el: 'e', value: 'xy' }, { op: 'compose', el: 'e', phase: 'end' }],
    [{ op: 'compose', el: 'e', phase: 'end' }, { op: 'type', el: 'e', value: ' 7 ' }],
  ]) {
    const vnode = { props: assignProps() };
    const binding = { value: '', modifiers: modObj(mods) };
    add(`v-model/composition/${mods.join('.') || 'none'}/${seq.length}-${seq[0].phase || 'type'}`, [
      { op: 'create', id: 'e', tag: 'input' },
      { op: 'mount', id: 'e' },
      { op: 'directive', name: 'vModelText', hook: 'created', el: 'e', binding, vnode },
      { op: 'directive', name: 'vModelText', hook: 'mounted', el: 'e', binding, vnode },
      ...seq,
      LOG,
      { op: 'snap', el: 'e' },
    ]);
  }
}
// ---- V3: v-model text on textarea ----
for (const typed of TYPED) {
  const vnode = { props: assignProps() };
  const binding = { value: 'ta', modifiers: {} };
  add(`v-model/textarea/${JSON.stringify(typed)}`, [
    { op: 'create', id: 'e', tag: 'textarea' },
    { op: 'mount', id: 'e' },
    { op: 'directive', name: 'vModelText', hook: 'created', el: 'e', binding, vnode },
    { op: 'directive', name: 'vModelText', hook: 'mounted', el: 'e', binding, vnode },
    { op: 'type', el: 'e', value: typed },
    LOG,
    { op: 'snap', el: 'e' },
  ]);
}
// ---- V4: v-model checkbox: booleans, arrays, Sets, true-value / false-value ----
const checkbox = (id) => [{ op: 'create', id, tag: 'input' }, { op: 'patch', el: id, key: 'type', prev: null, next: 'checkbox' }, { op: 'mount', id }];
for (const model of [false, true]) {
  add(`v-model/checkbox/bool/${model}`, [
    ...checkbox('e'),
    { op: 'directive', name: 'vModelCheckbox', hook: 'created', el: 'e', binding: { value: model }, vnode: { props: assignProps() } },
    { op: 'directive', name: 'vModelCheckbox', hook: 'mounted', el: 'e', binding: { value: model }, vnode: { props: assignProps() } },
    { op: 'snap', el: 'e' },
    { op: 'click', el: 'e' },
    LOG,
    { op: 'directive', name: 'vModelCheckbox', hook: 'beforeUpdate', el: 'e', binding: { value: !model, oldValue: model }, vnode: { props: assignProps() } },
    { op: 'snap', el: 'e' },
    LOG,
  ]);
}
for (const model of [['a', 'b'], ['x']]) {
  add(`v-model/checkbox/array/${JSON.stringify(model)}`, [
    ...checkbox('e'),
    { op: 'patch', el: 'e', key: 'value', prev: null, next: 'a' },
    { op: 'directive', name: 'vModelCheckbox', hook: 'created', el: 'e', binding: { value: model }, vnode: { props: assignProps({ value: 'a' }) } },
    { op: 'directive', name: 'vModelCheckbox', hook: 'mounted', el: 'e', binding: { value: model }, vnode: { props: assignProps({ value: 'a' }) } },
    { op: 'snap', el: 'e' },
    { op: 'click', el: 'e' },
    LOG,
    { op: 'click', el: 'e' },
    LOG,
  ]);
}
for (const model of [['a'], []]) {
  add(`v-model/checkbox/set/${JSON.stringify(model)}`, [
    ...checkbox('e'),
    { op: 'patch', el: 'e', key: 'value', prev: null, next: 'a' },
    { op: 'directive', name: 'vModelCheckbox', hook: 'created', el: 'e', binding: { value: { $: "set", items: model } }, vnode: { props: assignProps({ value: 'a' }) } },
    { op: 'directive', name: 'vModelCheckbox', hook: 'mounted', el: 'e', binding: { value: { $: 'set', items: model } }, vnode: { props: assignProps({ value: 'a' }) } },
    { op: 'click', el: 'e' },
    LOG,
  ]);
}
add('v-model/checkbox/true-false-values', [
  ...checkbox('e'),
  { op: 'patch', el: 'e', key: 'true-value', prev: null, next: 'yes' },
  { op: 'patch', el: 'e', key: 'false-value', prev: null, next: 'no' },
  { op: 'directive', name: 'vModelCheckbox', hook: 'created', el: 'e', binding: { value: 'no' }, vnode: { props: assignProps() } },
  { op: 'directive', name: 'vModelCheckbox', hook: 'mounted', el: 'e', binding: { value: 'no' }, vnode: { props: assignProps() } },
  { op: 'click', el: 'e' },
  LOG,
  { op: 'click', el: 'e' },
  LOG,
]);
add('v-model/checkbox/undefined-model', [
  ...checkbox('e'),
  { op: 'directive', name: 'vModelCheckbox', hook: 'created', el: 'e', binding: { value: U }, vnode: { props: assignProps() } },
  { op: 'directive', name: 'vModelCheckbox', hook: 'mounted', el: 'e', binding: { value: U }, vnode: { props: assignProps() } },
  { op: 'snap', el: 'e' },
  { op: 'click', el: 'e' },
  LOG,
]);
// ---- V5: v-model radio group ----
for (const [clicked, model] of [['a', 'b'], ['b', 'a'], ['c', 'b'], ['a', 'c']]) {
  const radio = (id, value) => [
    { op: 'create', id, tag: 'input' },
    { op: 'patch', el: id, key: 'type', prev: null, next: 'radio' },
    { op: 'mount', id },
  ];
  add(`v-model/radio/${clicked}-from-${model}`, [
    ...radio('ra', 'a'),
    ...radio('rb', 'b'),
    ...radio('rc', 'c'),
    ...['ra', 'rb', 'rc'].flatMap((id) => [
      { op: 'directive', name: 'vModelRadio', hook: 'created', el: id, binding: { value: model }, vnode: { props: assignProps({ value: id.slice(1) }) } },
    ]),
    { op: 'snap', el: 'ra' },
    { op: 'snap', el: 'rb' },
    { op: 'snap', el: 'rc' },
    { op: 'click', el: 'r' + clicked },
    LOG,
    { op: 'directive', name: 'vModelRadio', hook: 'beforeUpdate', el: 'ra', binding: { value: clicked, oldValue: model }, vnode: { props: assignProps({ value: 'a' }) } },
    { op: 'snap', el: 'ra' },
  ]);
}
// ---- V6: v-model select: single, multiple, number, object values ----
const options = (sel, values) => [
  { op: 'create', id: sel, tag: 'select' },
  ...values.flatMap((v, i) => [
    { op: 'create', id: `${sel}${i}`, tag: 'option' },
    { op: 'patch', el: `${sel}${i}`, key: 'value', prev: null, next: v },
    { op: 'mount', id: `${sel}${i}`, parent: sel },
  ]),
  { op: 'mount', id: sel },
];
for (const [initial, indices, updated] of [['b', [2], 'c'], ['b', [2], 'z'], ['a', [1], 'b']]) {
  add(`v-model/select/single/${initial}-${indices.join('')}-updated-${updated}`, [
    ...options('s', ['a', 'b', 'c']),
    { op: 'directive', name: 'vModelSelect', hook: 'created', el: 's', binding: { value: initial, modifiers: {} }, vnode: { props: assignProps() } },
    { op: 'directive', name: 'vModelSelect', hook: 'mounted', el: 's', binding: { value: initial }, vnode: { props: assignProps() } },
    { op: 'snap', el: 's' },
    { op: 'select', el: 's', indices },
    { op: 'fire', el: 's', type: 'change' },
    LOG,
    { op: 'directive', name: 'vModelSelect', hook: 'beforeUpdate', el: 's', binding: { value: updated }, vnode: { props: assignProps() } },
    { op: 'directive', name: 'vModelSelect', hook: 'updated', el: 's', binding: { value: updated, oldValue: initial } },
    { op: 'snap', el: 's' },
  ]);
}
add('v-model/select/multiple/array', [
  ...options('s', ['a', 'b', 'c']),
  { op: 'patch', el: 's', key: 'multiple', prev: null, next: true },
  { op: 'directive', name: 'vModelSelect', hook: 'created', el: 's', binding: { value: ['a'], modifiers: {} }, vnode: { props: assignProps() } },
  { op: 'directive', name: 'vModelSelect', hook: 'mounted', el: 's', binding: { value: ['a'] }, vnode: { props: assignProps() } },
  { op: 'select', el: 's', indices: [0, 2] },
  { op: 'fire', el: 's', type: 'change' },
  LOG,
  { op: 'directive', name: 'vModelSelect', hook: 'beforeUpdate', el: 's', binding: { value: ['a', 'c'] }, vnode: { props: assignProps() } },
  { op: 'directive', name: 'vModelSelect', hook: 'updated', el: 's', binding: { value: ['a', 'c'], oldValue: ['a'] } },
  { op: 'snap', el: 's' },
]);
add('v-model/select/multiple/set', [
  ...options('s', ['a', 'b', 'c']),
  { op: 'patch', el: 's', key: 'multiple', prev: null, next: true },
  { op: 'directive', name: 'vModelSelect', hook: 'created', el: 's', binding: { value: { $: 'set', items: ['b'] }, modifiers: {} }, vnode: { props: assignProps() } },
  { op: 'directive', name: 'vModelSelect', hook: 'mounted', el: 's', binding: { value: { $: 'set', items: ['b'] } }, vnode: { props: assignProps() } },
  { op: 'select', el: 's', indices: [1] },
  { op: 'fire', el: 's', type: 'change' },
  LOG,
]);
add('v-model/select/number-modifier', [
  ...options('s', ['1', '2', '3']),
  { op: 'directive', name: 'vModelSelect', hook: 'created', el: 's', binding: { value: 1, modifiers: { number: true } }, vnode: { props: assignProps() } },
  { op: 'directive', name: 'vModelSelect', hook: 'mounted', el: 's', binding: { value: 1 }, vnode: { props: assignProps() } },
  { op: 'select', el: 's', indices: [1] },
  { op: 'fire', el: 's', type: 'change' },
  LOG,
]);
add('v-model/select/object-option-values', [
  ...options('s', ['a', 'b']),
  { op: 'patch', el: 's0', key: 'value', prev: 'a', next: { k: 1 } },
  { op: 'directive', name: 'vModelSelect', hook: 'created', el: 's', binding: { value: 'a', modifiers: {} }, vnode: { props: assignProps() } },
  { op: 'directive', name: 'vModelSelect', hook: 'mounted', el: 's', binding: { value: 'a' }, vnode: { props: assignProps() } },
  { op: 'select', el: 's', indices: [0] },
  { op: 'fire', el: 's', type: 'change' },
  LOG,
]);
// ---- V7: v-model dynamic: the directive is picked from the element and its type ----
add('v-model/dynamic/text', [
  { op: 'create', id: 'e', tag: 'input' }, { op: 'mount', id: 'e' },
  { op: 'directive', name: 'vModelDynamic', hook: 'created', el: 'e', binding: { value: 'v', modifiers: {} }, vnode: { props: assignProps() } },
  { op: 'directive', name: 'vModelDynamic', hook: 'mounted', el: 'e', binding: { value: 'v', modifiers: {} }, vnode: { props: assignProps() } },
  { op: 'type', el: 'e', value: 'typed' }, LOG,
  { op: 'directive', name: 'vModelDynamic', hook: 'beforeUpdate', el: 'e', binding: { value: 'w', oldValue: 'v', modifiers: {} }, vnode: { props: assignProps() } },
  { op: 'snap', el: 'e' },
]);
add('v-model/dynamic/checkbox', [
  ...checkbox('e'),
  { op: 'directive', name: 'vModelDynamic', hook: 'created', el: 'e', binding: { value: false }, vnode: { props: assignProps({ type: 'checkbox' }) } },
  { op: 'directive', name: 'vModelDynamic', hook: 'mounted', el: 'e', binding: { value: false }, vnode: { props: assignProps({ type: 'checkbox' }) } },
  { op: 'click', el: 'e' }, LOG,
]);
add('v-model/dynamic/radio', [
  { op: 'create', id: 'e', tag: 'input' }, { op: 'patch', el: 'e', key: 'type', prev: null, next: 'radio' }, { op: 'mount', id: 'e' },
  { op: 'directive', name: 'vModelDynamic', hook: 'created', el: 'e', binding: { value: 'x' }, vnode: { props: assignProps({ type: 'radio', value: 'x' }) } },
  { op: 'click', el: 'e' }, LOG,
]);
add('v-model/dynamic/textarea', [
  { op: 'create', id: 'e', tag: 'textarea' }, { op: 'mount', id: 'e' },
  { op: 'directive', name: 'vModelDynamic', hook: 'created', el: 'e', binding: { value: 'q', modifiers: {} }, vnode: { props: assignProps() } },
  { op: 'directive', name: 'vModelDynamic', hook: 'mounted', el: 'e', binding: { value: 'q', modifiers: {} }, vnode: { props: assignProps() } },
  { op: 'type', el: 'e', value: ' t ' }, LOG,
]);
add('v-model/dynamic/select', [
  ...options('s', ['a', 'b']),
  { op: 'directive', name: 'vModelDynamic', hook: 'created', el: 's', binding: { value: 'b', modifiers: {} }, vnode: { props: assignProps() } },
  { op: 'directive', name: 'vModelDynamic', hook: 'mounted', el: 's', binding: { value: 'b' }, vnode: { props: assignProps() } },
  { op: 'select', el: 's', indices: [0] }, { op: 'fire', el: 's', type: 'change' }, LOG,
  { op: 'directive', name: 'vModelDynamic', hook: 'updated', el: 's', binding: { value: 'a', oldValue: 'b' }, vnode: { props: assignProps() } },
  { op: 'snap', el: 's' },
]);
// ---- V8: v-show ----
for (const initial of [true, false]) {
  add(`v-show/toggle/${initial}`, [
    { op: 'create', id: 'e', tag: 'div' }, { op: 'mount', id: 'e' },
    { op: 'directive', name: 'vShow', hook: 'beforeMount', el: 'e', binding: { value: initial }, vnode: {} },
    { op: 'directive', name: 'vShow', hook: 'mounted', el: 'e', binding: { value: initial }, vnode: {} },
    { op: 'snap', el: 'e' },
    { op: 'directive', name: 'vShow', hook: 'updated', el: 'e', binding: { value: !initial, oldValue: initial }, vnode: {} },
    { op: 'snap', el: 'e' },
    { op: 'directive', name: 'vShow', hook: 'updated', el: 'e', binding: { value: initial, oldValue: !initial }, vnode: {} },
    { op: 'snap', el: 'e' },
    { op: 'directive', name: 'vShow', hook: 'beforeUnmount', el: 'e', binding: { value: false }, vnode: {} },
    { op: 'snap', el: 'e' },
  ]);
}
add('v-show/keeps-original-display', [
  { op: 'create', id: 'e', tag: 'div' }, { op: 'mount', id: 'e' },
  { op: 'patch', el: 'e', key: 'style', prev: null, next: 'display: flex; color: red' },
  { op: 'directive', name: 'vShow', hook: 'beforeMount', el: 'e', binding: { value: false }, vnode: {} },
  { op: 'snap', el: 'e' },
  { op: 'directive', name: 'vShow', hook: 'updated', el: 'e', binding: { value: true, oldValue: false }, vnode: {} },
  { op: 'snap', el: 'e' },
]);
add('v-show/style-patch-while-hidden', [
  { op: 'create', id: 'e', tag: 'div' }, { op: 'mount', id: 'e' },
  { op: 'directive', name: 'vShow', hook: 'beforeMount', el: 'e', binding: { value: false }, vnode: {} },
  { op: 'patch', el: 'e', key: 'style', prev: null, next: 'color: red' },
  { op: 'snap', el: 'e' },
  { op: 'patch', el: 'e', key: 'style', prev: 'color: red', next: { color: 'blue', display: 'block' } },
  { op: 'snap', el: 'e' },
  { op: 'directive', name: 'vShow', hook: 'updated', el: 'e', binding: { value: true, oldValue: false }, vnode: {} },
  { op: 'snap', el: 'e' },
]);
add('v-show/style-object-display-while-shown', [
  { op: 'create', id: 'e', tag: 'div' }, { op: 'mount', id: 'e' },
  { op: 'directive', name: 'vShow', hook: 'beforeMount', el: 'e', binding: { value: true }, vnode: {} },
  { op: 'patch', el: 'e', key: 'style', prev: null, next: { display: 'grid' } },
  { op: 'snap', el: 'e' },
  { op: 'directive', name: 'vShow', hook: 'updated', el: 'e', binding: { value: false, oldValue: true }, vnode: {} },
  { op: 'snap', el: 'e' },
  { op: 'patch', el: 'e', key: 'style', prev: { display: 'grid' }, next: { display: 'grid' } },
  { op: 'snap', el: 'e' },
]);
// ---- V9: withModifiers on a listener; the parent listener shows what propagation the guard allowed ----
const EV = (over) => ({ kind: 'Mouse', type: 'click', bubbles: true, cancelable: true, composed: true, init: {}, ...over });
function modScenario(name, mods, dispatches) {
  const steps = [
    { op: 'create', id: 'p', tag: 'div' }, { op: 'create', id: 'b', tag: 'button' }, { op: 'create', id: 'c', tag: 'span' },
    { op: 'mount', id: 'p' }, { op: 'mount', id: 'b', parent: 'p' }, { op: 'mount', id: 'c', parent: 'b' },
    { op: 'patch', el: 'p', key: 'onClick', prev: null, next: H('h2') },
    { op: 'wrap', id: 'w', kind: 'withModifiers', fn: H('h1'), mods },
    { op: 'patch', el: 'b', key: 'onClick', prev: null, next: REF('w') },
  ];
  for (const [target, ev] of dispatches) {
    steps.push({ op: 'dispatch', el: target, event: ev }, LOG);
  }
  add(name, steps);
}
for (const [mod, a, b] of [
  ['stop', EV({}), EV({})],
  ['prevent', EV({ cancelable: true }), EV({ cancelable: false })],
  ['ctrl', EV({ init: { ctrlKey: true } }), EV({ init: { ctrlKey: false } })],
  ['shift', EV({ init: { shiftKey: true } }), EV({ init: { shiftKey: false } })],
  ['alt', EV({ init: { altKey: true } }), EV({ init: { altKey: false } })],
  ['meta', EV({ init: { metaKey: true } }), EV({ init: { metaKey: false } })],
  ['left', EV({ init: { button: 0 } }), EV({ init: { button: 1 } })],
  ['middle', EV({ init: { button: 1 } }), EV({ init: { button: 0 } })],
  ['right', EV({ init: { button: 2 } }), EV({ init: { button: 0 } })],
]) {
  modScenario(`with-modifiers/${mod}`, [mod], [['b', a], ['b', b]]);
}
modScenario('with-modifiers/self', ['self'], [['b', EV({})], ['c', EV({})]]);
modScenario('with-modifiers/exact-without-ctrl', ['exact'], [['b', EV({})], ['b', EV({ init: { ctrlKey: true } })]]);
modScenario('with-modifiers/exact-with-ctrl', ['exact', 'ctrl'], [['b', EV({ init: { ctrlKey: true } })], ['b', EV({ init: { ctrlKey: true, shiftKey: true } })]]);
modScenario('with-modifiers/ctrl-stop', ['ctrl', 'stop'], [['b', EV({ init: { ctrlKey: true } })], ['b', EV({})]]);
modScenario('with-modifiers/self-prevent', ['self', 'prevent'], [['b', EV({ cancelable: true })], ['c', EV({ cancelable: true })]]);
modScenario('with-modifiers/left-prevent', ['left', 'prevent'], [['b', EV({ cancelable: true, init: { button: 0 } })], ['b', EV({ cancelable: true, init: { button: 2 } })]]);
modScenario('with-modifiers/empty-list', [], [['b', EV({})]]);
// ---- V10: withKeys: the key name is hyphenated and matched against the modifier or its alias ----
const KEYDOWN = (key, over = {}) => ({ kind: 'Keyboard', type: 'keydown', bubbles: true, cancelable: true, composed: true, init: { key, code: key }, ...over });
function keyScenario(name, mods, keys) {
  const steps = [
    { op: 'create', id: 'p', tag: 'div' }, { op: 'create', id: 'b', tag: 'input' },
    { op: 'mount', id: 'p' }, { op: 'mount', id: 'b', parent: 'p' },
    { op: 'wrap', id: 'w', kind: 'withKeys', fn: H('hk'), mods },
    { op: 'patch', el: 'b', key: 'onKeydown', prev: null, next: REF('w') },
  ];
  for (const k of keys) steps.push({ op: 'dispatch', el: 'b', event: KEYDOWN(k) }, LOG);
  add(name, steps);
}
for (const [mods, keys] of [
  [['enter'], ['Enter', 'a']],
  [['tab'], ['Tab', 'Enter']],
  [['delete'], ['Delete', 'Backspace']],
  [['esc'], ['Escape', 'Enter']],
  [['space'], [' ', 'a']],
  [['up'], ['ArrowUp', 'ArrowDown']],
  [['down'], ['ArrowDown']],
  [['left'], ['ArrowLeft']],
  [['right'], ['ArrowRight', 'ArrowLeft']],
  [['page-up'], ['PageUp', 'PageDown']],
  [['a'], ['a', 'A']],
  [['escape'], ['Escape']],
  [['enter', 'tab'], ['Tab', 'Enter', 'x']],
  [[], ['Enter']],
]) {
  keyScenario(`with-keys/${mods.join('.') || 'none'}/${keys.map((k) => JSON.stringify(k)).join('-')}`, mods, keys);
}
add('with-keys/no-key-property-is-ignored', [
  { op: 'create', id: 'b', tag: 'input' }, { op: 'mount', id: 'b' },
  { op: 'wrap', id: 'w', kind: 'withKeys', fn: H('hk'), mods: ['enter'] },
  { op: 'patch', el: 'b', key: 'onKeydown', prev: null, next: REF('w') },
  { op: 'dispatch', el: 'b', event: { kind: 'Event', type: 'keydown', bubbles: true, cancelable: false, composed: false, init: {} } }, LOG,
  { op: 'dispatch', el: 'b', event: { kind: 'Mouse', type: 'keydown', bubbles: true, cancelable: false, composed: false, init: { button: 0 } } }, LOG,
]);

export { cases, HANDLERS };

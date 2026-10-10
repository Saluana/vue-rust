// Oracle for the Rust SSR and hydration probe (crates/runtime/src/ssr.rs, crates/runtime/src/hydrate.rs and
// crates/runtime/tests/ssr_hydration.rs). Run from the repository root:
//   node tests/ssr-oracle/probe.mjs
// Part (a) renders each probe component with the real @vue/server-renderer 3.5.43 renderToString and prints one
// line per probe: "<probe-name>\t<html>". The probe trees are written with the same vnode shapes the Rust probes build
// (h(), withDirectives, createCommentVNode(''), Fragment children), so the Rust output must equal these strings byte for
// byte. Part (b) lists the hydration behaviour that Vue 3.5.43 defines; it is cited from the readable builds and is not
// executed (Vue's hydration needs a DOM and this repository has no jsdom, and no package may be added).
import { createSSRApp, defineComponent, h, reactive, createCommentVNode, Fragment, withDirectives, vModelText } from 'vue';
import { renderToString } from 'vue/server-renderer';

const Child = defineComponent({
  props: ['label'],
  emits: ['ping'],
  setup(props, { emit }) {
    return () => h('button', { class: 'child', onClick: () => emit('ping', props.label) }, 'ping ' + props.label);
  },
});

const items = () => [
  { id: 1, label: 'a' },
  { id: 2, label: 'b' },
  { id: 3, label: 'c & d' },
];

// name, state (created per render, as an SSR request should), render(state) -> vnode
const probes = [
  ['static-tree', () => ({}), () => h('div', { id: 'root', class: 'a b', style: 'color: red; margin: 0' }, [
    h('p', null, `Hello & <world> "q" 's`),
    h('img', { alt: 'x"y', src: 'a.png' }),
    h('br'),
    h('span', { title: "it's" }, 'end'),
  ])],
  ['attrs-style', () => ({}), () => h('div', {
    class: ['x', { y: true, z: false }, null],
    style: { color: 'blue', fontSize: '12px', opacity: 0.5, width: null },
    'data-n': 3, hidden: false, disabled: true, title: null, tabindex: '-1',
  }, 'attrs')],
  ['interpolation', () => ({ count: 0, msg: `<b>"x" & 'y'</b>` }), (s) => h('p', { 'data-msg': s.msg }, `Count: ${s.count} & ${s.msg}`)],
  ['cond-false', () => ({ show: false, count: 2 }), (s) => h('div', null, [s.show ? h('p', null, 'shown ' + s.count) : createCommentVNode('')])],
  ['cond-true', () => ({ show: true, count: 2 }), (s) => h('div', null, [s.show ? h('p', null, 'shown ' + s.count) : createCommentVNode('')])],
  ['keyed-list', () => ({ items: items() }), (s) => h('ul', null, [h(Fragment, null, s.items.map((it) => h('li', { key: it.id }, it.label)))])],
  ['keyed-empty', () => ({ items: [] }), (s) => h('ul', null, [h(Fragment, null, s.items.map((it) => h('li', { key: it.id }, it.label)))])],
  ['child-props-emit', () => ({ label: 'L' }), (s) => h(Child, { label: s.label, 'data-x': 'v', onPing: () => {} })],
  ['input-v-model', () => ({ msg: 'Hello' }), (s) => withDirectives(h('input', { type: 'text', 'onUpdate:modelValue': (v) => { s.msg = v; } }), [[vModelText, s.msg]])],
  ['click-button', () => ({ count: 0 }), (s) => h('button', { onClick: () => { s.count++; } }, 'Count: ' + s.count)],
  ['app', () => ({ msg: 'Hi', count: 1, show: true, items: items() }), (s) => h('div', { class: 'app' }, [
    h('h1', { title: s.msg }, `${s.msg} & ${s.count}`),
    h(Child, { label: s.msg, 'data-x': 'v', onPing: () => { s.count++; } }),
    s.show ? h('p', null, 'shown') : createCommentVNode(''),
    h('ul', null, [h(Fragment, null, s.items.map((it) => h('li', { key: it.id }, it.label)))]),
    withDirectives(h('input', { type: 'text', 'onUpdate:modelValue': (v) => { s.msg = v; } }), [[vModelText, s.msg]]),
    h('button', { onClick: () => { s.count++; } }, 'Count: ' + s.count),
  ])],
  // The input and a paragraph that shows the model. Used by the hydration test that types into the input before hydration.
  ['input-typed', () => ({ msg: 'Hello' }), (s) => h('div', null, [
    withDirectives(h('input', { type: 'text', 'onUpdate:modelValue': (v) => { s.msg = v; } }), [[vModelText, s.msg]]),
    h('p', null, 'Typed: ' + s.msg),
  ])],
  // Attribute values: '' is a bare attribute, false is written as "false", null and undefined are skipped.
  ['attr-edge', () => ({}), () => h('div', { hidden: true, title: '', id: 0, 'data-e': '', 'data-f': false, required: '', class: '', 'data-g': null, 'data-h': undefined }, 'edge')],
  // Comment text: the delimiters are removed until nothing changes (escapeHtmlComment), the leading '>' runs only at the start.
  ['comment-escape', () => ({}), () => h('div', null, [createCommentVNode('a --> b <!-- c --!> d <!-'), createCommentVNode('-> lead ->> x')])],
];

console.log('# (a) renderToString of vue@3.5.43 (name, html)');
for (const [name, makeState, render] of probes) {
  const app = createSSRApp({
    setup() {
      const state = reactive(makeState());
      return () => render(state);
    },
  });
  const html = await renderToString(app);
  console.log(`${name}\t${html}`);
}

// The module-level counter that Vue's SSR guide warns about: state created once per module leaks across requests.
const moduleState = reactive({ hits: 0 });
const Counter = { setup() { moduleState.hits++; return () => h('i', null, String(moduleState.hits)); } };
console.log('module-counter-first\t' + (await renderToString(createSSRApp(Counter))));
console.log('module-counter-second\t' + (await renderToString(createSSRApp(Counter))));

// Vue's v-if comment text. The compiled SSR output of a false v-if is "<!---->"; a vnode comment carries its text, so the
// probes use createCommentVNode('') to match the compiled output.
console.log('comment-with-text\t' + (await renderToString(createSSRApp({ render: () => createCommentVNode('v-if') }))));

// (b) Hydration behaviour of vue@3.5.43, cited from the readable builds (line numbers of runtime-core.cjs.prod.js and
// runtime-dom.cjs.js in node_modules/@vue). Not executed here.
// Line numbers: runtime-core.cjs.prod.js (production build) unless marked "dev" (runtime-core.cjs.js).
const hydrationCitations = [
  'hydrate(vnode, container): hydrateNode(container.firstChild, vnode); no clearing of the container. prod:1419-1428',
  'hydrateNode: the DOM node is stored as vnode.el before the switch on the vnode type. prod:1430-1449 (vnode.el = node at 1443, switch at 1449)',
  'hydrateNode Text: a DOM node that is not a text node is a mismatch (empty text vnodes are created in place); a text node whose data differs is fixed in place with node.data = vnode.children after logMismatchError(). prod:1450-1462',
  'hydrateNode Comment: any comment node is adopted unless it is a fragment start "[" (domType !== 8 || isFragmentStart is a mismatch); comment text is not compared. prod:1466-1479',
  'hydrateNode Fragment: needs a "[" comment; hydrateFragment adopts children up to the matching "]" comment and sets vnode.anchor to it, else inserts a "]" comment. prod:1501-1506 (case), 1756-1777 (hydrateFragment)',
  'hydrateNode element: a different node type or tag (vnode.type.toLowerCase() !== node.tagName.toLowerCase()) is a mismatch. prod:1517',
  'hydrateElement: created dirs hooks run first; children through hydrateChildren(el.firstChild); text content is compared and reset on mismatch; props patched only for forcePatch value keys, on* listeners, .prop keys and custom elements; beforeMount dirs hook runs now, the mounted hooks are queued. prod:1595-1700',
  'hydrateChildren: a text vnode followed by another text vnode splits the DOM text node at vnode.children.length (dev:2244, prod:1717); a missing DOM node causes a fresh patch(null, vnode, container). prod:1704-1744',
  'handleMismatch: vnode.el = null; a fragment mismatch removes nodes up to "]"; the node is removed and patch(null, vnode, container, next) mounts a fresh vnode in its place; parent component el updated. prod:1779-1812',
  'locateClosingAnchor: scans siblings for the matching "]" with nesting. prod:1814-1829',
  'component mount during hydration: the render effect hydrates the rendered subtree on the existing element (hydrateSubTree). prod:4729-4746 (dev:6162-6191)',
  'vModelText.created: with a parent node, records el.defaultValue (the server value attribute) as the initial value; attaches input/compositionstart/compositionend/change listeners. runtime-dom.cjs.js:1522-1548 (dev), runtime-dom.cjs.prod.js:1424 (created)',
  'vModelText.mounted: deletes the initial value; if the element is text or textarea, the initial value is defined and el.value differs from it (the user typed before hydration), it calls the model assigner with el.value and does not overwrite the DOM; otherwise el.value = model. runtime-dom.cjs.js:1549-1560 (dev), runtime-dom.cjs.prod.js:1450 (prod)',
  'vModelText.getSSRProps: returns { value } so the SSR output carries value="<model>". runtime-dom.cjs.js:1784 (dev), runtime-dom.cjs.prod.js:1682 (prod)',
  'hydrate (prod): an empty container is patched in full; otherwise hydrateNode(container.firstChild) runs, then flushPostFlushCbs only. prod:1419-1428',
  'hydrateElement gate: created hooks, children, props, beforeMount and mounted run only when forcePatch, hasDynamicProps or patchFlag !== -1; a cached (hoisted static) element is adopted as it is. prod:1608',
  'hydrateElement children: server nodes left after the last client child are a mismatch (logMismatchError) and are removed. prod:1630-1643',
  'hydrateElement text: textContent is compared (leading newline of pre/textarea dropped, CRLF normalised); on mismatch el.textContent = vnode.children, which creates one new text node. prod:1648-1659',
  'hydrateElement props: keys that are forced (input value), on*, .prop, custom-element or dynamic are patched, skipping src/srcset/href/poster that already hold the client value (isUnchangedResourceProp). prod:1663-1675, 1849-1854',
  'hydrateChildren text split: a text vnode followed by a text vnode takes the DOM text from its length on into a new node after it, and the DOM node keeps the first part. prod:1713-1723',
  'logMismatchError: production logs one console.error("Hydration completed but contains mismatches.") per realm; this port records every recovered mismatch in its log instead. prod:1388-1395',
];
console.log('# (b) hydration expectations (cited, not executed)');
for (const line of hydrationCitations) console.log('- ' + line);
console.log(`# probes rendered: ${probes.length}`);

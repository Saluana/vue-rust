// Oracle for the component/effect ownership acceptance tests (task 4.3, crates/runtime/tests/ownership.rs).
//
// Runs the scenarios on the real vue@3.5.43 package (production build, as the Vue contract does) with a headless
// node-ops tree, so no DOM is needed. It prints the observed order of hooks and watcher callbacks, the rendered tree,
// and subscriber and effect counts read from Vue's own structures (`ref.dep.subs` links, `instance.scope.effects`).
// The Rust test copies these values by hand, with a comment naming this file.
//
// Run from the repository root:
//   NODE_ENV=production node tests/ownership-oracle/record.mjs
import { readFileSync } from 'node:fs';
import { computed, createRenderer, getCurrentInstance, h, nextTick, onBeforeUnmount, onScopeDispose, onUnmounted, ref, watch } from 'vue';

// ----- headless node ops (a minimal element tree; text children are text nodes) -----

function node(type) {
  return { type, tag: type, text: '', children: [], parent: null, listeners: {}, props: {} };
}

function detach(child) {
  const p = child.parent;
  if (p) p.children.splice(p.children.indexOf(child), 1);
  child.parent = null;
}

const ops = {
  insert(child, parent, anchor) {
    detach(child);
    const at = anchor ? parent.children.indexOf(anchor) : -1;
    if (at === -1) parent.children.push(child);
    else parent.children.splice(at, 0, child);
    child.parent = parent;
  },
  remove(child) {
    detach(child);
  },
  createElement(tag) {
    return node(tag);
  },
  createText(text) {
    const n = node('#text');
    n.text = text;
    return n;
  },
  createComment(text) {
    const n = node('#comment');
    n.text = text;
    return n;
  },
  setText(n, text) {
    n.text = text;
  },
  setElementText(el, text) {
    el.children = [];
    const t = ops.createText(text);
    ops.insert(t, el, null);
  },
  parentNode(n) {
    return n.parent;
  },
  nextSibling(n) {
    const p = n.parent;
    if (!p) return null;
    return p.children[p.children.indexOf(n) + 1] ?? null;
  },
  querySelector() {
    return null;
  },
  setScopeId() {},
  patchProp(el, key, prev, next) {
    if (/^on[A-Z]/.test(key)) {
      el.listeners[key] = next;
      return;
    }
    if (next == null) delete el.props[key];
    else el.props[key] = next;
  },
};

const { render } = createRenderer(ops);

// The markup of a container's children (what innerHTML of the container shows).
function html(container) {
  return container.children.map(markup).join('');
}

function markup(n) {
  if (n.type === '#text') return n.text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  if (n.type === '#comment') return `<!--${n.text}-->`;
  return `<${n.tag}>${n.children.map(markup).join('')}</${n.tag}>`;
}

// Subscriber count of a ref's dep: the links of the doubly linked list, walked from its tail (`dep.subs`).
function subscriberCount(r) {
  let n = 0;
  for (let l = r.dep.subs; l; l = l.prevSub) n += 1;
  return n;
}

// Dependency links held by a subscriber: the list of its `deps` (an effect or a computed), walked through `nextDep`.
function linkCount(sub) {
  let n = 0;
  for (let l = sub.deps; l; l = l.nextDep) n += 1;
  return n;
}

function find(n, tag, out = []) {
  if (n.tag === tag) out.push(n);
  for (const c of n.children) find(c, tag, out);
  return out;
}

function click(el) {
  el.listeners.onClick({ type: 'click' });
}

// ----- scenario 1: two instances of one component with props and emits stay independent -----

async function independence() {
  const log = [];
  const owned = {};
  const Counter = {
    props: ['label'],
    emits: ['change'],
    setup(props, { emit }) {
      const count = ref(0);
      const doubled = computed(() => count.value * 2);
      owned[props.label] = { inst: getCurrentInstance(), doubled };
      return () =>
        h(
          'button',
          {
            onClick: () => {
              count.value++;
              emit('change', props.label, count.value);
            },
          },
          `${props.label}:${doubled.value}`,
        );
    },
  };
  const Pair = {
    props: ['showB'],
    setup(props) {
      const onChange = (label, n) => log.push(`change ${label} ${n}`);
      return () => {
        const kids = [h(Counter, { label: 'A', onChange })];
        if (props.showB) kids.push(h(Counter, { label: 'B', onChange }));
        return h('div', null, kids);
      };
    },
  };
  const container = node('div');
  render(h(Pair, { showB: true }), container);
  const r = {};
  r.htmlMounted = html(container);
  const [a, b] = find(container, 'button');
  click(a);
  click(a);
  click(a);
  click(b);
  await nextTick();
  r.htmlAfterClicks = html(container);
  r.logAfterClicks = [...log];
  const ownedByB = () => owned.B.inst.scope.effects.reduce((n, e) => n + linkCount(e), 0) + linkCount(owned.B.doubled);
  r.dependencyLinksOfBBeforeRemoval = ownedByB();
  r.effectsOfBBeforeRemoval = owned.B.inst.scope.effects.length;
  render(h(Pair, { showB: false }), container);
  await nextTick();
  r.effectsOfBAfterRemoval = owned.B.inst.scope.effects.length;
  click(find(container, 'button')[0]);
  await nextTick();
  r.htmlAfterRemovingB = html(container);
  r.logAfterRemovingB = [...log];
  return r;
}

// ----- scenario 2: a computed read twice evaluates its getter once -----

async function computedOnce() {
  let runs = 0;
  const src = ref(1);
  const c = computed(() => {
    runs += 1;
    return src.value * 10;
  });
  const a = c.value;
  const b = c.value;
  const runsAfterTwoReads = runs;
  src.value = 2;
  const d = c.value;
  return { runsAfterTwoReads, values: [a, b, d], runsAfterChangeAndRead: runs };
}

// ----- scenario 3: removing a keyed child stops its watcher and releases its effects -----

async function keyedRemoval() {
  const log = [];
  const source = ref('x');
  const instances = {};
  const Item = {
    props: ['id', 'source'],
    setup(props) {
      const doubled = computed(() => props.id * 2);
      instances[props.id] = { inst: getCurrentInstance(), doubled };
      watch(props.source, (v) => log.push(`watch ${props.id} ${v}`));
      onScopeDispose(() => log.push(`scope disposed ${props.id}`));
      onBeforeUnmount(() => log.push(`bum ${props.id}`));
      onUnmounted(() => log.push(`unmounted ${props.id}`));
      return () => h('li', { onClick: () => {} }, String(doubled.value));
    },
  };
  const List = {
    props: ['ids'],
    setup(props) {
      return () => h('ul', null, props.ids.map((id) => h(Item, { key: id, id, source })));
    },
  };
  const container = node('div');
  render(h(List, { ids: [1, 2, 3] }), container);
  const r = {};
  r.htmlMounted = html(container);
  source.value = 'a';
  await nextTick();
  r.logAfterFirstChange = [...log];
  r.subscribersBeforeRemoval = subscriberCount(source);
  r.effectsOfChild2BeforeRemoval = instances[2].inst.scope.effects.length;
  r.dependencyLinksOfChild2BeforeRemoval =
    instances[2].inst.scope.effects.reduce((n, e) => n + linkCount(e), 0) + linkCount(instances[2].doubled);
  log.length = 0;
  render(h(List, { ids: [1, 3] }), container);
  await nextTick();
  r.logOnRemoval = [...log];
  r.subscribersAfterRemoval = subscriberCount(source);
  r.effectsOfChild2AfterRemoval = instances[2].inst.scope.effects.length;
  log.length = 0;
  source.value = 'b';
  await nextTick();
  r.logAfterSecondChange = [...log];
  r.htmlFinal = html(container);
  return r;
}

// ----- scenario 4: order of the unmount hooks, scope disposal and a watcher whose source changes while unmounting -----

async function unmountOrder() {
  const log = [];
  const src = ref(0);
  const Child = {
    setup() {
      watch(src, (v) => log.push(`watch ${v}`));
      onScopeDispose(() => log.push('scope disposed'));
      onBeforeUnmount(() => {
        log.push('bum');
        src.value = 99;
      });
      onUnmounted(() => log.push('um'));
      return () => h('p', null, 'child');
    },
  };
  const container = node('div');
  render(h(Child), container);
  src.value = 1;
  await nextTick();
  const r = { logBeforeUnmount: [...log] };
  log.length = 0;
  render(null, container);
  r.logDuringUnmount = [...log];
  log.length = 0;
  await nextTick();
  src.value = 5;
  await nextTick();
  r.logAfterUnmount = [...log];
  r.subscribersAfterUnmount = subscriberCount(src);
  return r;
}

const results = {
  vue: JSON.parse(readFileSync(new URL('../../node_modules/vue/package.json', import.meta.url), 'utf8')).version,
  independence: await independence(),
  computedOnce: await computedOnce(),
  keyedRemoval: await keyedRemoval(),
  unmountOrder: await unmountOrder(),
};
console.log(JSON.stringify(results, null, 2));

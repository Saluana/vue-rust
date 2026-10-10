// Node oracle for crates/runtime/tests/owned_operations.rs (task 3.3). It runs the real vue@3.5.43 production build
// (@vue/runtime-core, with a minimal in-memory renderer, so no DOM is needed) and prints the lifecycle and watcher order
// of the scenarios that the Rust test mirrors. The Rust test copies these lines by hand as its expected values.
//   NODE_ENV=production node tests/vue/owned-ops/record.mjs
// The line "host-cancel op" stands in for the runtime's owned-operation cancellation, which the Rust runtime registers as an
// onScopeDispose cleanup when the operation starts (after the user's own onScopeDispose, as in the Rust scenario).
process.env.NODE_ENV = 'production';
const vue = await import('@vue/runtime-core');
const { createRenderer, h, nextTick, onBeforeUnmount, onScopeDispose, onUnmounted, onWatcherCleanup, ref, watch } = vue;

function makeNode(type, text) {
  return { type, text, parent: null, children: [] };
}

function detach(child) {
  if (child.parent) {
    const siblings = child.parent.children;
    const i = siblings.indexOf(child);
    if (i >= 0) siblings.splice(i, 1);
    child.parent = null;
  }
}

const { render } = createRenderer({
  insert(child, parent, anchor) {
    detach(child);
    child.parent = parent;
    const at = anchor ? parent.children.indexOf(anchor) : -1;
    if (at >= 0) parent.children.splice(at, 0, child);
    else parent.children.push(child);
  },
  remove: detach,
  createElement: (tag) => makeNode(tag, ''),
  createText: (text) => makeNode('#text', text),
  createComment: (text) => makeNode('#comment', text),
  setText(node, text) {
    node.text = text;
  },
  setElementText(el, text) {
    el.children.length = 0;
    el.text = text;
  },
  parentNode: (node) => node.parent,
  nextSibling(node) {
    if (!node.parent) return null;
    const siblings = node.parent.children;
    const i = siblings.indexOf(node);
    return i >= 0 && i + 1 < siblings.length ? siblings[i + 1] : null;
  },
  querySelector: () => null,
  setScopeId() {},
  patchProp() {},
});

// Scenario "unmount-order": a component with a pre watcher (whose callback registers an onWatcherCleanup), two scope cleanups
// (user first, then the owned-operation stand-in), before/after unmount hooks and a render effect. After unmount, a write
// must not re-run the disposed watcher or render.
async function unmountOrder() {
  const log = [];
  const say = (line) => log.push(line);
  const state = ref(0);
  const Owner = {
    setup() {
      say('setup');
      onBeforeUnmount(() => say('bum'));
      onUnmounted(() => say('um'));
      watch(state, (v) => {
        onWatcherCleanup(() => say(`watch-cleanup ${v}`));
        say(`watch ${v}`);
      });
      onScopeDispose(() => say('user-dispose'));
      onScopeDispose(() => say('host-cancel op'));
      return () => {
        say(`render ${state.value}`);
        return h('p', null, `r${state.value}`);
      };
    },
  };
  const container = makeNode('root', '');
  render(h(Owner), container);
  state.value = 1;
  await nextTick();
  render(null, container);
  await nextTick();
  state.value = 2;
  await nextTick();
  return log;
}

// Scenario "ordinary-promise-after-unmount": a promise that the component does not own. Its continuation keeps JS behaviour
// after unmount and may update a store that is still live, while the disposed component's render does not re-run.
async function ordinaryPromiseAfterUnmount() {
  const log = [];
  const say = (line) => log.push(line);
  const store = ref(0);
  watch(store, (v) => say(`store ${v}`));
  let resolveLater;
  const Retained = {
    setup() {
      say('setup');
      const p = new Promise((resolve) => {
        resolveLater = resolve;
      });
      p.then((v) => {
        say(`then ${v}`);
        store.value = v;
      });
      return () => {
        say(`render ${store.value}`);
        return h('i', null, 'x');
      };
    },
  };
  const container = makeNode('root', '');
  render(h(Retained), container);
  render(null, container);
  await nextTick();
  resolveLater('late');
  await nextTick();
  return log;
}

const scenarios = [
  ['unmount-order', unmountOrder],
  ['ordinary-promise-after-unmount', ordinaryPromiseAfterUnmount],
];
for (const [name, run] of scenarios) {
  const log = await run();
  console.log(`scenario ${name}`);
  for (const line of log) console.log(`  ${JSON.stringify(line)}`);
}

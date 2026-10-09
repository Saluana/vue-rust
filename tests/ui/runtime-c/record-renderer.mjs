// WP-RT-C renderer goldens. Writes renderer/scenarios.json (deterministic, from the generator below) and records
// renderer/golden.json from vue@3.5.43 (production global build) in headless Chrome: after every step, the innerHTML
// of the container. Each scenario is a list of steps: { render: <vnode-dsl> } or { render: null } (unmount).
// Usage (repository root): node tests/ui/runtime-c/record-renderer.mjs [--check]
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { withPage } from '../../../tooling/chrome/chrome.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const outDir = path.join(here, 'renderer');
const check = process.argv.includes('--check');

function lcg(seed) {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}
function shuffle(arr, rnd) {
  const a = arr.slice();
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(rnd() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

// ----- scenarios (deterministic) -----
const scenarios = [];
const add = (name, steps) => scenarios.push({ name, steps });
const li = (k) => ({ el: 'li', key: k, children: String(k) });
const ul = (children) => ({ el: 'ul', children });

add('element-text', [
  { render: { el: 'div', props: { id: 'a' }, children: 'hello' } },
  { render: { el: 'div', props: { id: 'a' }, children: 'bye' } },
  { render: { el: 'div', props: { id: 'a' } } },
  { render: { el: 'div', props: { id: 'a' }, children: '' } },
  { render: null },
]);
add('element-attrs', [
  { render: { el: 'p', props: { id: 'x', class: 'a b', title: 'T' }, children: 'x' } },
  { render: { el: 'p', props: { id: 'y', class: 'c', 'data-k': '1' }, children: 'x' } },
  { render: { el: 'p', props: { id: 'y' }, children: 'x' } },
  { render: { el: 'p', props: { id: 'y', hidden: true }, children: 'x' } },
  { render: { el: 'p', props: { id: 'y', hidden: false }, children: 'x' } },
  { render: { el: 'p', props: { id: 'y', title: 'a<b&"c' }, children: 'x&<y>' } },
]);
add('nested-children', [
  { render: { el: 'div', children: [{ el: 'span', children: 'a' }, 'b', { text: 'c' }, { comment: 'd' }, null, { el: 'i' }] } },
  { render: { el: 'div', children: [{ el: 'span', children: 'a2' }, { el: 'b' }] } },
  { render: { el: 'div', children: [{ el: 'span', children: 'a3' }, 'bb', { el: 'i' }] } },
  { render: { el: 'div', children: [] } },
  { render: { el: 'div', children: 'text' } },
  { render: { el: 'div', children: [{ el: 'em' }] } },
  { render: { el: 'div', children: [{ el: 'em', children: [{ el: 'b', children: 'deep' }, 'tail'] }] } },
]);
add('type-change', [
  { render: { el: 'div', children: [{ el: 'span', children: 'x' }] } },
  { render: { el: 'div', children: [{ text: 'x' }] } },
  { render: { el: 'div', children: [{ comment: 'c' }] } },
  { render: { el: 'section', children: 'y' } },
  { render: { text: 'root text' } },
  { render: { comment: 'root comment' } },
  { render: { el: 'div', children: 'z' } },
]);
add('fragment-root', [
  { render: { fragment: [{ el: 'a', children: '1' }, { el: 'b', children: '2' }] } },
  { render: { fragment: [{ el: 'a', children: '1' }] } },
  { render: { fragment: [] } },
  { render: { fragment: [{ text: 'z' }, { el: 'c' }] } },
  { render: { fragment: [{ el: 'd' }, { el: 'e' }, { el: 'f' }] } },
  { render: null },
]);
add('fragment-in-element', [
  { render: ul([{ fragment: [li('a'), li('b')] }, li('c')]) },
  { render: ul([{ fragment: [] }, li('c2')]) },
  { render: ul([{ fragment: [li('n')] }, { fragment: [li('m')] }]) },
  { render: ul([{ fragment: [li('p'), li('q')], key: 'f1' }, { fragment: [li('r')], key: 'f2' }]) },
  { render: ul([{ fragment: [li('r')], key: 'f2' }, { fragment: [li('p'), li('q')], key: 'f1' }]) },
]);
add('unkeyed-list', [
  { render: ul([{ el: 'li', children: '1' }, { el: 'li', children: '2' }, { el: 'li', children: '3' }]) },
  { render: ul([{ el: 'li', children: 'x' }, { el: 'li', children: 'y' }]) },
  { render: ul([{ el: 'li', children: 'x' }, { el: 'b', children: 'y' }, { el: 'li', children: 'z' }]) },
  { render: ul([]) },
  { render: ul([{ el: 'li', children: 'again' }]) },
]);
add('keyed-append-prepend', [
  { render: ul([1, 2, 3].map(li)) },
  { render: ul([1, 2, 3, 4].map(li)) },
  { render: ul([0, 1, 2, 3, 4].map(li)) },
  { render: ul([0, 4].map(li)) },
  { render: ul([4, 0].map(li)) },
  { render: ul([]) },
  { render: ul([9, 8].map(li)) },
]);
add('keyed-mixed-keys', [
  { render: ul([li('a'), { el: 'li', children: 'u1' }, li('b'), { text: 'txt' }]) },
  { render: ul([{ el: 'li', children: 'u2' }, li('b'), li('a')]) },
  { render: ul([li(1), li('1'), li(true), li(0), li(-0)]) },
  { render: ul([li(0), li(1), li('1')]) },
]);
add('keyed-reverse-and-rotate', [
  { render: ul([1, 2, 3, 4, 5, 6].map(li)) },
  { render: ul([6, 5, 4, 3, 2, 1].map(li)) },
  { render: ul([2, 3, 4, 5, 6, 1].map(li)) },
  { render: ul([6, 1, 2, 3, 4, 5].map(li)) },
  { render: ul([3, 1, 5, 2, 6, 4].map(li)) },
]);
add('keyed-fragment-items', [
  { render: ul([1, 2, 3].map((k) => ({ fragment: [li(k), li('s' + k)], key: 'g' + k }))) },
  { render: ul([3, 1].map((k) => ({ fragment: [li(k), li('s' + k)], key: 'g' + k }))) },
  { render: ul([2, 3, 1].map((k) => ({ fragment: [li(k)], key: 'g' + k }))) },
  { render: ul([2, 3, 1].map((k) => ({ fragment: [li(k), li('t' + k)], key: 'g' + k }))) },
]);
for (const n of [2, 3, 4, 5, 6, 8, 11, 16, 33]) {
  const base = Array.from({ length: n }, (_, i) => i);
  const steps = [{ render: ul(base.map(li)) }];
  for (let seed = 1; seed <= 6; seed++) {
    steps.push({ render: ul(shuffle(base, lcg(seed * 7919 + n)).map(li)) });
  }
  add(`keyed-permutation-${n}`, steps);
}
{
  // insert and remove keys between shuffles: exercises the mount-in-middle and remove paths of the keyed diff
  const rnd = lcg(424242);
  let keys = Array.from({ length: 12 }, (_, i) => i);
  const steps = [{ render: ul(keys.map(li)) }];
  let next = 12;
  for (let round = 0; round < 12; round++) {
    const r = rnd();
    if (r < 0.4 && keys.length > 0) {
      keys = keys.filter(() => rnd() > 0.3);
    } else {
      const extra = [];
      const count = 1 + Math.floor(rnd() * 3);
      for (let c = 0; c < count; c++) extra.push(next++);
      keys = shuffle(keys.concat(extra), rnd);
    }
    steps.push({ render: ul(keys.map(li)) });
  }
  add('keyed-insert-remove-mix', steps);
}

// ----- recording -----
function buildExpr() {}

const scenarioJson = JSON.stringify(scenarios, null, 1) + '\n';
fs.mkdirSync(outDir, { recursive: true });
const scenPath = path.join(outDir, 'scenarios.json');

// Runs in the page: builds each vnode with createVNode and friends, renders into a container, returns innerHTML per step.
function runInPage(list) {
  const V = window.Vue;
  const build = (n) => {
    if (n === null || typeof n !== 'object') return n;
    if ('text' in n) return V.createTextVNode(n.text);
    if ('comment' in n) return V.createCommentVNode(n.comment);
    if ('fragment' in n) {
      const props = n.key === undefined ? null : { key: n.key };
      return V.createVNode(V.Fragment, props, n.fragment.map(build));
    }
    let props = n.props ? Object.assign({}, n.props) : null;
    if (n.key !== undefined) props = Object.assign(props || {}, { key: n.key });
    const children = Array.isArray(n.children) ? n.children.map(build) : n.children === undefined ? null : n.children;
    return V.createVNode(n.el, props, children);
  };
  const out = {};
  for (const sc of list) {
    const container = document.createElement('div');
    document.body.appendChild(container);
    const snaps = [];
    for (const step of sc.steps) {
      V.render(step.render === null ? null : build(step.render), container);
      snaps.push(container.innerHTML);
    }
    out[sc.name] = snaps;
    container.remove();
  }
  return out;
}

const vueSrc = fs.readFileSync(path.join(root, 'node_modules/vue/dist/vue.global.prod.js'), 'utf8');
const html = `<!doctype html><meta charset="utf-8"><body><script>${vueSrc}</script></body>`;
const golden = await withPage({ viewport: { width: 800, height: 600, dpr: 1 } }, async (page) => {
  await page.setContent(html);
  const version = await page.evaluate(() => window.Vue.version);
  if (version !== '3.5.43') throw new Error(`expected vue 3.5.43, got ${version}`);
  return page.evaluate(runInPage, scenarios);
});

const goldenPath = path.join(outDir, 'golden.json');
const goldenJson = JSON.stringify(golden, null, 1) + '\n';
if (check) {
  const same = fs.existsSync(goldenPath) && fs.readFileSync(goldenPath, 'utf8') === goldenJson;
  const sameScen = fs.existsSync(scenPath) && fs.readFileSync(scenPath, 'utf8') === scenarioJson;
  console.log(`scenarios ${sameScen ? 'match' : 'DIFFER'}; golden ${same ? 'matches' : 'DIFFERS'} (${scenarios.length} scenarios)`);
  process.exitCode = same && sameScen ? 0 : 1;
} else {
  fs.writeFileSync(scenPath, scenarioJson);
  fs.writeFileSync(goldenPath, goldenJson);
  console.log(`wrote ${scenarios.length} scenarios and their golden snapshots to ${path.relative(root, outDir)}`);
}

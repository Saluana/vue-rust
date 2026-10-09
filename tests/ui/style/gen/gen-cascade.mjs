// WP-U1b cascade and inheritance corpus (tests/ui/style/corpora/cascade.json): 72 seeded documents. Each has an author
// stylesheet (selectors, !important, media rules, custom properties with var() chains and fallbacks, cycles), inline
// style attributes, element states (disabled, checked, focus) and a fixed list of 10-12 properties. Chrome computes
// every element's properties (getComputedStyle, pre-order over the fixture's elements); the Rust test rebuilds the same
// tree through vn-dom and compares. Run: node tests/ui/style/gen/gen-cascade.mjs
import { fileURLToPath } from 'node:url';
import { withPage } from '../../../../tooling/chrome/chrome.mjs';
import { makeRng, writeCorpus } from './common.mjs';
import { VIEWPORT } from './measure.mjs';

export const SEED = 0xca5c4d1en;
export const FIXTURES = 72;

const TAGS = ['div', 'div', 'p', 'span', 'section', 'ul', 'li', 'em', 'strong', 'a', 'label', 'h2'];
const FORM = ['input', 'button'];
const CLASSES = ['a', 'b', 'c'];
const SELECTORS = [
  'div', 'p', 'span', '*', '.a', '.b', '.c', '#x', '#y', '[data-k]', '[data-k=v]', '[data-k~=w]', 'div > span', 'p span', 'li + li',
  'span ~ em', ':first-child', ':last-child', ':nth-child(2n+1)', ':nth-child(odd)', ':not(.a)', ':empty', ':disabled', ':enabled',
  ':checked', ':focus', 'input:checked', 'button:disabled', 'div:first-child > p', '.a.b', 'section p.a', 'ul li:nth-child(even)',
  'a[href]', 'label', 'em', 'strong', 'h2', 'div p:last-child', 'div:not(.b) span',
];
const DECLS = {
  color: ['red', '#123', 'rgb(1 2 3 / 50%)', 'inherit', 'currentcolor', 'transparent', 'initial', 'unset', 'var(--c)', 'var(--u, blue)', 'var(--cyc)', 'hsl(120 50% 50%)', 'var(--missing)'],
  'background-color': ['lime', 'rgba(0, 0, 0, 0.5)', 'inherit', 'var(--c, #ff0000)', 'transparent'],
  'font-size': ['1.5em', '2rem', '150%', '12px', 'larger', 'smaller', 'medium', '3vw', 'calc(1em + 2px)', 'var(--fs)', 'x-large', 'inherit'],
  'font-weight': ['bold', '300', 'bolder', 'lighter', 'normal', '650', 'inherit'],
  'font-style': ['italic', 'normal', 'inherit'],
  'line-height': ['1.5', '20px', 'normal', '2em', '150%', 'inherit'],
  'letter-spacing': ['1px', '0.1em', 'normal', 'var(--ls, 2px)'],
  'text-align': ['center', 'right', 'start', 'inherit'],
  visibility: ['hidden', 'visible', 'inherit'],
  opacity: ['0.5', '1.5', 'var(--op)', '0'],
  'z-index': ['3', 'auto', '-1'],
  cursor: ['pointer', 'help', 'inherit'],
  'white-space': ['nowrap', 'pre', 'inherit'],
  'margin-left': ['10px', '2em', 'auto', '-5px', '1rem'],
  'padding-top': ['4px', '1rem', 'var(--pad, 3px)'],
  'border-top-width': ['3px', 'thin', 'thick'],
  'border-top-style': ['solid', 'none', 'dashed'],
  display: ['block', 'inline-block', 'none', 'contents', 'flex', 'inline'],
  'box-sizing': ['border-box', 'content-box'],
  'text-transform': ['uppercase', 'lowercase', 'none'],
};
const PROPS = [
  'color', 'background-color', 'font-size', 'font-weight', 'font-style', 'line-height', 'letter-spacing', 'text-align',
  'visibility', 'opacity', 'z-index', 'cursor', 'white-space', 'margin-left', 'padding-top', 'border-top-width',
  'border-top-style', 'display', 'box-sizing', 'text-transform',
];

export function generateFixtures() {
  const rng = makeRng(SEED);
  const fixtures = [];
  for (let id = 0; id < FIXTURES; id++) {
    const pick = (a) => rng.pick(a);
    const decl = () => {
      const prop = pick(Object.keys(DECLS));
      const bang = rng.chance(0.12) ? ' !important' : '';
      return `${prop}: ${pick(DECLS[prop])}${bang}`;
    };
    const rule = () => `${pick(SELECTORS)} { ${Array.from({ length: rng.range(1, 3) }, decl).join('; ')} }`;
    const sheetRules = Array.from({ length: rng.range(3, 7) }, rule);
    if (rng.chance(0.5)) sheetRules.push(`@media (min-width: 700px) { ${rule()} ${rule()} }`);
    if (rng.chance(0.4)) sheetRules.push(`@media (max-width: 500px) { ${rule()} }`);
    if (rng.chance(0.3)) sheetRules.push(`@media (prefers-color-scheme: dark) { ${rule()} }`);
    if (rng.chance(0.2)) sheetRules.push(`@media print { ${rule()} }`);
    if (rng.chance(0.3)) sheetRules.push('@media (min-width: 900px) and (orientation: landscape) { div { color: blue } }');
    const custom = [
      '--c: green', '--fs: 2em', '--cyc: var(--cyc2)', '--cyc2: var(--cyc)', '--op: 0.25', '--pad: 7px', '--ls: 3px',
    ].filter(() => rng.chance(0.6));
    const css = `:root { ${custom.join('; ')} }\n${sheetRules.join('\n')}`;
    const tree = buildTree(rng, 0);
    const elements = [];
    collect(tree, elements);
    const focusables = elements.map((e, i) => (FORM.includes(e.t) || (e.t === 'a' && e.a.some(([k]) => k === 'href')) ? i : -1)).filter((i) => i >= 0);
    const focus = focusables.length && rng.chance(0.4) ? pick(focusables) : -1;
    const chosen = PROPS.filter(() => rng.chance(0.55));
    while (chosen.length < 10) {
      const p = pick(PROPS);
      if (!chosen.includes(p)) chosen.push(p);
    }
    fixtures.push({ id, css, tree, focus, props: chosen.slice(0, 12) });
  }
  return fixtures;

  function buildTree(r, depth) {
    const tag = r.pick(depth === 0 ? ['div', 'section'] : TAGS.concat(FORM));
    const a = [];
    if (r.chance(0.4)) a.push(['class', r.pick(CLASSES) + (r.chance(0.3) ? ' ' + r.pick(CLASSES) : '')]);
    if (r.chance(0.15)) a.push(['id', r.pick(['x', 'y'])]);
    if (r.chance(0.2)) a.push(['data-k', r.pick(['v', 'w', 'z'])]);
    if (tag === 'a' && r.chance(0.6)) a.push(['href', '#']);
    if (FORM.includes(tag) && r.chance(0.3)) a.push(['disabled', '']);
    if (tag === 'input' && r.chance(0.4)) a.push(['type', 'checkbox'], ['checked', '']);
    if (r.chance(0.3)) a.push(['style', r.pick(['color: magenta', 'font-size: 2em; cursor: help', 'margin-left: 4px !important', 'display: none', 'font-weight: 700; color: var(--c)', ''])]);
    const node = { t: tag, a, c: [] };
    if (depth < 3 && r.chance(0.7)) {
      const n = r.range(1, 3);
      for (let k = 0; k < n; k++) node.c.push(buildTree(r, depth + 1));
    }
    if (r.chance(0.25)) node.c.push({ x: r.pick(['text', ' spaced ', 'more text']) });
    if (r.chance(0.1)) node.c.push({ m: 'note' });
    return node;
  }
  function collect(d, out) {
    out.push(d);
    for (const c of d.c) if (c.t !== undefined) collect(c, out);
  }
}

// In-page: builds the fixture under body and returns the computed values of every element (pre-order).
export const RUN_IN_PAGE = (fx) => {
  document.head.querySelectorAll('style').forEach((s) => s.remove());
  const st = document.createElement('style');
  st.textContent = fx.css;
  document.head.appendChild(st);
  const make = (d) => {
    if (d.x !== undefined) return document.createTextNode(d.x);
    if (d.m !== undefined) return document.createComment(d.m);
    const el = document.createElement(d.t);
    for (const [k, v] of d.a) el.setAttribute(k, v);
    for (const c of d.c) el.appendChild(make(c));
    return el;
  };
  document.body.replaceChildren(make(fx.tree));
  const all = Array.from(document.body.querySelectorAll('*'));
  if (fx.focus >= 0) all[fx.focus].focus();
  return all.map((el) => {
    const cs = getComputedStyle(el);
    return fx.props.map((p) => cs.getPropertyValue(p));
  });
};

export async function generate(outDir) {
  const fixtures = generateFixtures();
  const measured = await withPage({ viewport: VIEWPORT }, async (page) => {
    await page.setContent('<!doctype html><html><head></head><body></body></html>');
    const out = [];
    for (const fx of fixtures) {
      const values = await page.evaluate(RUN_IN_PAGE, {
        css: fx.css,
        tree: fx.tree,
        focus: fx.focus,
        props: fx.props,
      });
      out.push(values);
    }
    return out;
  });
  const docs = fixtures.map((fx, i) => ({
    id: fx.id,
    css: fx.css,
    tree: fx.tree,
    focus: fx.focus,
    props: fx.props,
    expect: measured[i],
  }));
  const file = writeCorpus('cascade.json', {
    about: 'WP-U1b cascade and inheritance corpus: Chrome computed values per element (pre-order over the fixture tree)',
    viewport: VIEWPORT,
    seed: String(SEED),
    fixtures: docs,
  }, outDir);
  const elements = docs.reduce((n, d) => n + d.expect.length, 0);
  return { file, fixtures: docs.length, elements, props: docs.reduce((n, d) => n + d.props.length, 0) };
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const r = await generate();
  console.log(`fixtures ${r.fixtures}, elements ${r.elements}, property checks ${r.props} -> ${r.file}`);
}

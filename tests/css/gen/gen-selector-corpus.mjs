// WP-U1a selector corpus (crates/css, matching and selector tests). Chrome is the oracle for:
//   - the element states (:hover is never set; :focus via el.focus(); :disabled, :enabled, :checked, :empty)
//   - the matching elements of each selector (document.querySelectorAll, pre-order element indices)
//   - whether a selector is valid (SyntaxError otherwise) and its CSSOM serialisation (selectorText)
// 64 random trees x 25..30 random selectors. Deterministic (fixed seed). Run: node tests/css/gen/gen-selector-corpus.mjs
import { fileURLToPath } from 'node:url';
import { withPage } from '../../../tooling/chrome/chrome.mjs';
import { BUILD_IN_PAGE, PAGE_STATES, attachStates, countElements, makeRng, writeCorpus } from './common.mjs';

const SEED = 0x51d3c0ffee01n;
const TREES = 64;

const TAGS = ['div', 'span', 'p', 'a', 'button', 'input', 'select', 'option', 'label', 'ul', 'li', 'section', 'em', 'b', 'textarea', 'fieldset', 'optgroup', 'h1', 'table'];
const IDS = ['x', 'y', 'z', 'Main', 'a1', 'x-y'];
const CLASSES = ['c1', 'c2', 'C1', 'a-b', 'big', 'x'];
const DATA_VALUES = ['v', 'V', 'a b', 'a-b', 'a-', '-b', '', 'ab', 'xv', 'x-v-y', 'en-US', 'en', 'v-x', 'aéb'];
const INPUT_TYPES = ['text', 'TEXT', 'checkbox', 'radio', 'hidden', 'submit'];
const LANGS = ['en', 'en-US', 'fr', 'EN'];

// Selector pieces (valid in Chrome). Some combinations are deliberately not matching anything.
const TAG_SEL = [...TAGS, '*', 'DIV', 'Span', 'INPUT', 'Option'];
const CLASS_SEL = ['c1', 'c2', 'C1', 'a-b', 'big', 'x', 'nothere'];
const ID_SEL = ['x', 'y', 'Main', 'a1', 'x-y', 'zz'];
const ATTR_SEL = [
  'data-x', 'data-x=v', 'data-x="a b"', 'data-x~=v', 'data-x~="a b"', 'data-x|=en', 'lang|=en', 'data-x^=v', 'data-x$=v',
  'data-x*=v', 'data-x=V i', 'TYPE=TEXT', 'type=text', 'lang=EN i', 'data-x*=""', 'data-y=""', 'disabled', 'checked',
  'selected', 'href="#"', 'REL=UP', 'title~=t', 'id^=X i', 'data-x=\'xv\'', 'data-x$=v-x', 'data-x~=""', 'lang=en-US',
];
const SIMPLE_PSEUDO = [
  ':first-child', ':last-child', ':only-child', ':empty', ':root', ':checked', ':disabled', ':enabled', ':focus', ':focus-visible',
  ':focus-within', ':hover', ':active', ':FIRST-CHILD', ':Hover',
];
// Selectors that are invalid or unsupported by our parser. Chrome decides which of them it accepts.
const ERROR_POOL = [
  ':has(a)', ':has(> a)', ':is(a, b)', ':where(.c1)', 'a::before', 'p::first-line', 'a:visited', 'a:link', ':lang(en)', ':dir(ltr)',
  ':nth-of-type(2)', ':first-of-type', 'ns|a', '*|a', '|a', 'a||b', ':nth-child(2n of .c1)', ':scope', '.c1:not(.c2 .c3)',
  ':not(a > b)', 'a >', '> a', 'a,', '.1a', '[a=]', '[a', ':nth-child(2n+)', '#1', '[data-x=v s]', 'a &', ':not()', '::before',
  ':hover:hover', 'a:target', ':placeholder-shown', 'a:nth-child(0)', 'li:nth-last-child(-n+2)', '[data-x]:not([data-x=v])',
];

function genEl(rng, depth, budget) {
  const t = rng.pick(TAGS);
  const a = [];
  if (rng.chance(0.3)) a.push(['id', rng.pick(IDS)]);
  if (rng.chance(0.5)) {
    const n = rng.range(1, 3);
    const cls = [];
    while (cls.length < n) {
      const c = rng.pick(CLASSES);
      if (!cls.includes(c)) cls.push(c);
    }
    a.push(['class', cls.join(' ')]);
  }
  if (rng.chance(0.4)) a.push([rng.pick(['data-x', 'data-x', 'data-y']), rng.pick(DATA_VALUES)]);
  if (t === 'input' && rng.chance(0.85)) a.push(['type', rng.pick(INPUT_TYPES)]);
  else if (rng.chance(0.06)) a.push(['type', rng.pick(INPUT_TYPES)]);
  if (rng.chance(0.2)) a.push(['lang', rng.pick(LANGS)]);
  if (rng.chance(0.15)) a.push(['title', rng.pick(['t', 'T', ''])]);
  if (t === 'a' && rng.chance(0.7)) a.push(['href', '#']);
  if (rng.chance(0.15)) a.push(['disabled', '']);
  if (rng.chance(0.15)) a.push(['checked', '']);
  if (rng.chance(0.1)) a.push(['selected', '']);
  if (rng.chance(0.1)) a.push(['tabindex', rng.pick(['0', '-1'])]);
  if (rng.chance(0.2)) a.push(['rel', rng.pick(['up', 'UP', 'down'])]);
  const el = { t, a, c: [] };
  const typeAttr = a.find(([k]) => k === 'type');
  if (t === 'input' && typeAttr && ['checkbox', 'radio'].includes(typeAttr[1].toLowerCase()) && rng.chance(0.3)) el.cp = true;
  if (depth < 4 && budget.left > 0) {
    const n = rng.range(0, 3);
    for (let i = 0; i < n && budget.left > 0; i++) {
      const r = rng.int(20);
      if (r === 0) el.c.push({ x: rng.pick(['', ' ', 'x']) });
      else if (r === 1) el.c.push({ m: 'c' });
      else {
        budget.left--;
        el.c.push(genEl(rng, depth + 1, budget));
      }
    }
  }
  return el;
}

function genTree(rng) {
  const budget = { left: rng.range(12, 46) };
  const body = { t: 'body', a: rng.chance(0.2) ? [['lang', 'en']] : [], c: [] };
  while (budget.left > 0 && rng.chance(0.93)) {
    budget.left--;
    body.c.push(genEl(rng, 1, budget));
  }
  return { t: 'html', a: [], c: [{ t: 'head', a: [], c: [] }, body] };
}

function genNth(rng) {
  if (rng.chance(0.1)) return rng.pick(['odd', 'even', 'ODD', 'Even']);
  const a = rng.range(-3, 3);
  const b = rng.range(-4, 5);
  const sp = () => rng.pick(['', '', ' ']);
  if (a === 0) return rng.pick([String(b), '+' + Math.abs(b), '-' + Math.abs(b)]);
  const base = a === 1 ? 'n' : a === -1 ? '-n' : `${a}n`;
  if (b === 0) return base;
  return `${base}${sp()}${b > 0 ? '+' : '-'}${sp()}${Math.abs(b)}`;
}

function genCompound(rng, depth) {
  let s = '';
  if (rng.chance(0.6)) s += rng.pick(TAG_SEL);
  const n = rng.int(3);
  for (let k = 0; k < n; k++) s += genSimple(rng, depth);
  return s === '' ? '*' : s;
}

function genSimple(rng, depth) {
  const r = rng.int(14);
  if (r < 2) return '.' + rng.pick(CLASS_SEL);
  if (r === 2) return '#' + rng.pick(ID_SEL);
  if (r < 5) return '[' + rng.pick(ATTR_SEL) + ']';
  if (r < 9) return rng.pick(SIMPLE_PSEUDO);
  if (r < 11) return `:nth-child(${genNth(rng)})`;
  if (r === 11) return `:nth-last-child(${genNth(rng)})`;
  if (depth === 0) {
    const list = [genCompound(rng, 1)];
    if (rng.chance(0.4)) list.push(genCompound(rng, 1));
    return `:not(${list.join(', ')})`;
  }
  return rng.pick(SIMPLE_PSEUDO);
}

function genSelector(rng) {
  if (rng.chance(0.08)) return rng.pick(ERROR_POOL);
  const parts = rng.range(1, 4);
  let s = '';
  for (let i = 0; i < parts; i++) {
    if (i > 0) s += rng.pick([' ', ' ', ' > ', '>', ' + ', '+', ' ~ ', '~', '  ']);
    s += genCompound(rng, 0);
  }
  if (rng.chance(0.12)) s += ', ' + genCompound(rng, 0);
  return s;
}

// Runs in the page. For each selector: its matching pre-order element indices (or a SyntaxError name), and its
// CSSOM selectorText when the selector is valid.
const EVAL_IN_PAGE = (sels) => {
  const all = Array.from(document.getElementsByTagName('*'));
  return sels.map((sel) => {
    let found;
    try {
      found = Array.from(document.querySelectorAll(sel), (el) => all.indexOf(el));
    } catch (e) {
      return { sel, ok: false, error: e.name };
    }
    const sheet = new CSSStyleSheet();
    sheet.replaceSync(sel + ' { color: red }');
    const text = sheet.cssRules.length === 1 ? sheet.cssRules[0].selectorText : null;
    return { sel, ok: true, matches: found, text };
  });
};

function focusableIndex(desc, rng) {
  const elements = [];
  const walk = (d) => {
    elements.push(d);
    for (const c of d.c || []) if (c.t !== undefined) walk(c);
  };
  walk(desc);
  return rng.chance(0.5) ? rng.int(elements.length) : -1;
}

export async function generate(outDir) {
  const rng = makeRng(SEED);
  const trees = [];
  await withPage({ viewport: { width: 800, height: 600, dpr: 1 } }, async (page) => {
    await page.setContent('<!doctype html><html><head></head><body></body></html>');
    for (let id = 0; id < TREES; id++) {
      const desc = genTree(rng);
      const focusIndex = focusableIndex(desc, rng);
      const n = await page.evaluate(BUILD_IN_PAGE, desc, focusIndex);
      if (n !== countElements(desc)) throw new Error(`tree ${id}: built ${n} elements, described ${countElements(desc)}`);
      const states = await page.evaluate(PAGE_STATES);
      attachStates(desc, states);
      const count = 25 + rng.int(6);
      const sels = [];
      while (sels.length < count) sels.push(genSelector(rng));
      const results = await page.evaluate(EVAL_IN_PAGE, sels);
      trees.push({ id, root: desc, cases: results });
    }
  });
  return writeCorpus('selectors.json', { generator: 'tests/css/gen/gen-selector-corpus.mjs', chrome: 'headless (tooling/chrome)', trees }, outDir);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const file = await generate();
  console.log('wrote', file);
}

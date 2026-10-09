// WP-U1a cascade and serialisation corpora (crates/css, cascade and custom property tests). Chrome is the oracle:
//   cascade.json:   ~90 seeded fixtures (stylesheet + element tree + inline styles + media environment). For every
//                   element: getComputedStyle display, position, visibility and the custom properties --a..--e,
//                   --dsp, --vis, --pos (getPropertyValue; '' when absent or invalid).
//   serialize.json: the boundary grid of var() substitution (each row in its own <style>), and a directed table of
//                   custom property values (whitespace, comments, escapes, !important, cycles, fallbacks, keywords).
// Run: node tests/css/gen/gen-cascade-corpus.mjs
import { fileURLToPath } from 'node:url';
import { withPage } from '../../../tooling/chrome/chrome.mjs';
import { BUILD_IN_PAGE, makeRng, writeCorpus } from './common.mjs';

const SEED = 0xca5cad3n;
const FIXTURES = 90;

const ENVS = [
  { name: 'desktop-800x600', width: 800, height: 600, dpr: 1, colorScheme: 'light', reducedMotion: false, touch: false },
  { name: 'touch-375x812-dark', width: 375, height: 812, dpr: 3, colorScheme: 'dark', reducedMotion: true, touch: true },
  { name: 'desktop-600x600-dpr2', width: 600, height: 600, dpr: 2, colorScheme: 'light', reducedMotion: false, touch: false },
  { name: 'desktop-1280x720-dark', width: 1280, height: 720, dpr: 1.5, colorScheme: 'dark', reducedMotion: false, touch: false },
];

const PROPS = ['display', 'position', 'visibility'];
const CUSTOM = ['--a', '--b', '--c', '--d', '--e', '--dsp', '--vis', '--pos'];
const DISPLAY = ['block', 'inline', 'inline-block', 'none', 'list-item', 'table'];
const TAGS = ['div', 'span', 'p', 'a', 'em', 'section', 'ul', 'li'];
const CLASSES = ['c1', 'c2', 'c3'];
const IDS = ['x', 'y'];
const MEDIA = ['(min-width: 600px)', 'screen and (max-width: 700px)', '(orientation: landscape)', '(prefers-color-scheme: dark)', '(hover: none)', 'print', '(min-width: 600px) and (orientation: portrait)', '(width >= 500px)', 'not print', '(pointer: coarse)'];
const CUSTOM_VALUES = [
  '1px', 'red', ' 5px  6px ', 'x /* c */ y', 'var(--a)', 'var(--b, 2px)', 'var(--c, var(--e, 3px))', 'var(--e)', '', 'initial', 'inherit',
  'unset', 'var(--undef)', 'var(--undef, none)', 'a(b c)', 'var(--a) var(--b)', 'x var(--a) y', 'var(--d)', 'var(--a, var(--b))', '  ', '2px  3px',
];

function genCompound(rng) {
  let s = rng.chance(0.5) ? rng.pick(TAGS) : '';
  if (rng.chance(0.4)) s += '.' + rng.pick(CLASSES);
  if (rng.chance(0.15)) s += '#' + rng.pick(IDS);
  if (rng.chance(0.1)) s += ':first-child';
  if (rng.chance(0.08)) s += ':not(.c1)';
  if (rng.chance(0.08)) s += '[data-x]';
  if (rng.chance(0.05)) s += ':nth-child(2n+1)';
  return s === '' ? rng.pick(TAGS) : s;
}

function genSelector(rng) {
  const parts = rng.range(1, 3);
  let s = '';
  for (let i = 0; i < parts; i++) {
    if (i > 0) s += rng.pick([' ', ' > ']);
    s += genCompound(rng);
  }
  return s;
}

function genDecl(rng) {
  const bang = rng.chance(0.12) ? ' !important' : '';
  const r = rng.int(20);
  if (r < 5) return `display: ${rng.pick([...DISPLAY, 'var(--dsp)', 'var(--dsp, block)', 'var(--undef, inline)', 'var(--dsp, var(--undef2, none))', 'var(--dsp)'])}${bang}`;
  if (r < 8) return `visibility: ${rng.pick(['visible', 'hidden', 'var(--vis)', 'var(--vis, hidden)'])}${bang}`;
  if (r < 11) return `position: ${rng.pick(['static', 'relative', 'absolute', 'fixed', 'var(--pos, relative)', 'var(--pos)'])}${bang}`;
  if (r < 13) return `--dsp: ${rng.pick([...DISPLAY, ''])}${bang}`;
  if (r < 14) return `--vis: ${rng.pick(['hidden', 'visible', ''])}${bang}`;
  if (r < 15) return `--pos: ${rng.pick(['absolute', 'static'])}${bang}`;
  return `${rng.pick(['--a', '--b', '--c', '--d', '--e'])}: ${rng.pick(CUSTOM_VALUES)}${bang}`;
}

function genDecls(rng) {
  const n = rng.range(1, 4);
  const out = [];
  for (let i = 0; i < n; i++) out.push(genDecl(rng));
  return out.join('; ');
}

function genRules(rng, depth) {
  const count = rng.range(2, 6);
  const out = [];
  for (let i = 0; i < count; i++) {
    if (depth === 0 && rng.chance(0.2)) {
      const inner = genRules(rng, depth + 1);
      out.push(`@media ${rng.pick(MEDIA)} {\n${inner}\n}`);
    } else if (depth === 1 && rng.chance(0.15)) {
      out.push(`@media ${rng.pick(MEDIA)} { ${genSelector(rng)} { ${genDecls(rng)} } }`);
    } else {
      const sels = [genSelector(rng)];
      if (rng.chance(0.2)) sels.push(genSelector(rng));
      out.push(`${sels.join(', ')} { ${genDecls(rng)} }`);
    }
  }
  return out.join('\n');
}

function genEl(rng, depth, budget) {
  const t = rng.pick(TAGS);
  const a = [];
  if (rng.chance(0.25)) a.push(['id', rng.pick(IDS)]);
  if (rng.chance(0.5)) a.push(['class', [...new Set([rng.pick(CLASSES), rng.pick(CLASSES)])].join(' ')]);
  if (rng.chance(0.3)) a.push(['data-x', rng.pick(['1', 'v', ''])]);
  if (rng.chance(0.2)) a.push(['style', genDecls(rng)]);
  const el = { t, a, c: [] };
  if (depth < 4 && budget.left > 0) {
    const n = rng.range(0, 3);
    for (let i = 0; i < n && budget.left > 0; i++) {
      budget.left--;
      el.c.push(genEl(rng, depth + 1, budget));
    }
  }
  return el;
}

function genFixture(rng, id) {
  const css = genRules(rng, 0);
  const budget = { left: rng.range(8, 28) };
  const body = [];
  while (budget.left > 0 && rng.chance(0.9)) {
    budget.left--;
    body.push(genEl(rng, 1, budget));
  }
  const env = rng.int(ENVS.length);
  const root = {
    t: 'html',
    a: [],
    c: [{ t: 'head', a: [], c: [{ t: 'style', a: [], c: [{ x: css }] }] }, { t: 'body', a: [], c: body }],
  };
  return { id, env, css, root };
}

const APPLY_ENV = async (page, env) => {
  await page.viewport(env.width, env.height, env.dpr);
  await page.send('Emulation.setEmulatedMedia', {
    features: [
      { name: 'prefers-color-scheme', value: env.colorScheme },
      { name: 'prefers-reduced-motion', value: env.reducedMotion ? 'reduce' : 'no-preference' },
    ],
  });
  await page.send('Emulation.setTouchEmulationEnabled', env.touch ? { enabled: true, maxTouchPoints: 1 } : { enabled: false });
};

// Runs in the page: computed values for every element in document order.
const COMPUTED_IN_PAGE = (props, custom) => Array.from(document.getElementsByTagName('*'), (el) => {
  const cs = getComputedStyle(el);
  const out = {};
  for (const p of props) out[p] = cs.getPropertyValue(p);
  out.custom = {};
  for (const c of custom) out.custom[c] = cs.getPropertyValue(c);
  return out;
});

function countWithState(desc) {
  let n = 1;
  for (const c of desc.c || []) if (c.t !== undefined) n += countWithState(c);
  return n;
}

// The directed value table: declared text and the expected getPropertyValue('--x') from Chrome.
const DIRECTED = [
  '#t { --x:  1px    2px  ; }', '#t { --x: a /* c */ b; }', '#t { --x: "a   b"; }', '#t { --x: \\41 b; }', '#t { --x: url(foo.png)   ; }',
  '#t { --x: [ a , b ] { c : d } ; }', '#t { --x: 1px !important; }', '#t { --x: 1px ! important; }', '#t { --x:; }', '#t { --x: initial; }',
  '#t { --x: var(--nope, 3px  4px); }', '#t { --x: var(--nope); }', '#t { --x: var(--a, var(--a)); }', '#t { --x: a\n   b\n   c; }',
  '#t { --x: ( a , b ); }', '#t { --x: calc( 1px + 2px ); }', '#t { --x: a\tb\n c; }', "#t { --x: 'it''s'; }", '#t { --x: a/**/b; }',
  '#t { --x: a/**/; }', '#t { --x: /**/a; }', '#t { --x: a , b ,c; }', '#t { --x: #FFF; }', '#t { --x: 1E3; }', '#t { --x: -- x; }',
  '#t { --x: { }; }', '#t { --x: ; }', '#t { --x: inherit; }', '#t { --x: unset; }', '#t { --x: revert; }', '#t { --x: a !important b; }',
  '#t { --x: a;b; }', '#t { --x: 10% 2.50em +3; }', '#t { --x: \ta\tb\t; }', '#t { --x: var(--m, var(--n, 7px)) ; }',
  '#t { --x: env(safe-area-inset-top); }', '#t { --x: a{b}c; }', '#t { --x: a(b c)d; }', '#t { --x: \\ a; }', '#t { --x: "\\"a"; }',
  '#t { --x: été; }', '#t { --x: x  var(--n)  y; }', '#t { --x: a!; }', '#t { --x: a ! b; }', '#t { --x: a!important; }',
  '#t { --x: a !IMPORTANT; }', '#t { --x: a !important !important; }', '#t { --x: [a;b]; }', '#t { --x: (a;b); }', '#t { --x: {a;b}; }',
  '#t { --x: a) b; }', '#t { --x: "abc; }', '#t { --x: url(a b); }', '#t { --x: a:b; }', '#t { --x: <!-- a -->; }', '#t { --x: a] b; }',
  '#t { --x: [a) b]; }', '#t { --x: a\; }', '#t { --x: \\61 ; }', '#t { --x: a\\\n b; }', '#t { --x: a , b !important; }', '#t { --x: a  !important  ; }',
  '#t { --p: 1px   2px; --x: var(--p)  3px; }', '#t { --p: 1px /* c */ 2px; --x: var(--p)3px; }', '#t { --p: x /* c */ y; --x: var(--p)3px; }',
  '#t { --p: 1px; --x: x var(--p,  5px ) y; }', '#t { --p: 1px; --x: var( --p ); }', '#t { --p: 1px; --x: var(--p, ); }',
  '#t { --e: ; --x: var(--e, 5px); }', '#t { --e: ; --x: a var(--e) b; }', '#t { --x: var(--undef, ); }', '#t { --x: var(--undef, a, b); }',
  '#t { --x: var(--undef,); }', '#t { --x: VAR(--p); --p: 1px; }', '#t { --x: var(a); }', '#t { --x: var(--undef, var(--b)); --b: 9px; }',
  '#t { --a: var(--b); --b: var(--a); --x: var(--a); }', '#t { --a: var(--b, 1px); --b: var(--a, 2px); --x: var(--a); }',
  '#t { --a: var(--b, 1px); --b: var(--a, 2px); --x: var(--b); }', '#t { --x: var(--cyc, 3px); --cyc: var(--cyc); }',
  '#t { --x: var(--nope, var(--x)); }', '#t { --p: 1px; --x: var(--p) var(--p); }', '#t { --p: 1px; --x:  var(--p) ; }',
  '#t { --p: 1px; --x: a var(--undef) b; }', '#t { --p: 1px; --x: 1px var(--p) 2px; }', '#t { --p: 1px; --x: calc(var(--p) + 1px); }',
  '#t { --p: -; --x: var(--p)-; }', '#t { --p: #a; --x: var(--p)#a; }', '#t { --p: .5; --x: x var(--p) y; }',
  '#t { --p: a; --x: var(--p)(b); }', '#t { --p: a; --x: var(--p)[b]; }', '#t { --p: "s"; --x: var(--p)"t"; }',
  '#t { --p: 1px; --x: var(--p)!important; }', '#t { --p: 1px !important; --x: var(--p); }', '#t { --p: 1px; --x: var(--p) !important; }',
];

function grid() {
  const P = ['y', 'a1', 'a-', '1', '1.5', '1px', '1%', '#a', '@a', "'s'", '/', '+', '-', '.', '(a)', '[a]', '{a}', 'url(a)', '\\41', 'a\\41', '-->', '<!--', ',', ':', '!', '#', '--', 'U', 'u+1', '-1', '1e3', 'f(a)', ')x', '%', '@', '.5'];
  const S = ['3px', '3', '.5', 'a', '-a', '--x', '-1', '+1', '%', '#a', '@a', "'s'", '/', '(a)', '[a]', '{a}', ',', ':', '!', '-->', '<!--', '\\41', 'u+1', '-', '--', '1e3', 'y', 'x1', 'e3', '1%', '#', '.', '5', '-\\', 'a(b)', '|'];
  const rows = [];
  let n = 0;
  for (const p of P) {
    for (const s of S) {
      rows.push({ id: 'g' + n++, kind: 'after', p, s, css: `#g${n - 1} { --p: ${p}; --x: var(--p)${s}; }` });
      rows.push({ id: 'g' + n++, kind: 'before', p, s, css: `#g${n - 1} { --p: ${p}; --x: ${s}var(--p); }` });
    }
  }
  return rows;
}

export async function generate(outDir) {
  const rng = makeRng(SEED);
  const fixtures = [];
  for (let id = 0; id < FIXTURES; id++) fixtures.push(genFixture(rng, id));
  const cascadeRows = [];
  await withPage({ viewport: { width: 800, height: 600, dpr: 1 } }, async (page) => {
    await page.setContent('<!doctype html><html><head></head><body></body></html>');
    for (const fx of fixtures) {
      await APPLY_ENV(page, ENVS[fx.env]);
      const n = await page.evaluate(BUILD_IN_PAGE, fx.root, -1);
      if (n !== countWithState(fx.root)) throw new Error(`fixture ${fx.id}: element count mismatch`);
      const computed = await page.evaluate(COMPUTED_IN_PAGE, PROPS, CUSTOM);
      cascadeRows.push({ id: fx.id, env: ENVS[fx.env].name, css: fx.css, root: fx.root, computed });
    }
  });
  // Serialisation: one <style> per row, then one page evaluation.
  const rows = grid();
  const directed = DIRECTED.map((css, i) => ({ id: 'd' + i, css: css.replace('#t ', `#d${i} `) }));
  const serialized = [];
  await withPage({ viewport: { width: 800, height: 600, dpr: 1 } }, async (page) => {
    const html = `<!doctype html><html><head>${[...rows, ...directed].map((r) => `<style>${r.css}</style>`).join('')}</head><body>${[...rows, ...directed].map((r) => `<div id="${r.id}"></div>`).join('')}</body></html>`;
    await page.setContent(html);
    const values = await page.evaluate((ids) => ids.map((id) => {
      const cs = getComputedStyle(document.getElementById(id));
      return [cs.getPropertyValue('--x'), cs.getPropertyValue('--p')];
    }), [...rows, ...directed].map((r) => r.id));
    [...rows, ...directed].forEach((r, i) => serialized.push({ ...r, out: values[i][0], pv: values[i][1] }));
  });
  const gridRows = serialized.slice(0, rows.length).map(({ id, kind, p, s, css, pv, out }) => ({ id, kind, p, s, css, pv, out }));
  const directedRows = serialized.slice(rows.length).map(({ id, css, out }) => ({ id, css, out }));
  const cascadeFile = writeCorpus('cascade.json', { generator: 'tests/css/gen/gen-cascade-corpus.mjs', chrome: 'headless (tooling/chrome); getComputedStyle', envs: ENVS, props: PROPS, custom: CUSTOM, fixtures: cascadeRows }, outDir);
  const serFile = writeCorpus('serialize.json', { generator: 'tests/css/gen/gen-cascade-corpus.mjs', chrome: 'headless (tooling/chrome); getComputedStyle', grid: gridRows, directed: directedRows }, outDir);
  return [cascadeFile, serFile];
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  for (const f of await generate()) console.log('wrote', f);
}

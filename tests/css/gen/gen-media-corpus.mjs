// WP-U1a media corpus (crates/css, media tests). Chrome is the oracle: window.matchMedia(query).matches for each
// query at each emulated environment. Environments: viewport size and DPR (page.viewport), colour scheme and
// reduced motion (Emulation.setEmulatedMedia), and touch emulation (Emulation.setTouchEmulationEnabled), which is
// the only reachable way to get hover:none and pointer:coarse in this Chrome (hover/pointer features are ignored).
// Categories: clean (no diagnostic, compared), invalid (diagnostic, compared), diagnosed (an unknown media type or
// feature, which is diagnosed by design, compared: unknown follows Kleene logic), unsupported (diagnostic, not compared).
// Run: node tests/css/gen/gen-media-corpus.mjs
import { fileURLToPath } from 'node:url';
import { withPage } from '../../../tooling/chrome/chrome.mjs';
import { writeCorpus } from './common.mjs';

const ENVS = [
  { name: 'desktop-800x600', width: 800, height: 600, dpr: 1, colorScheme: 'light', reducedMotion: false, touch: false },
  { name: 'touch-375x812-dark', width: 375, height: 812, dpr: 3, colorScheme: 'dark', reducedMotion: true, touch: true },
  { name: 'desktop-600x600-dpr2', width: 600, height: 600, dpr: 2, colorScheme: 'light', reducedMotion: false, touch: false },
  { name: 'desktop-1280x720-dark', width: 1280, height: 720, dpr: 1.5, colorScheme: 'dark', reducedMotion: false, touch: false },
  { name: 'touch-320x480-dpr2-reduce', width: 320, height: 480, dpr: 2, colorScheme: 'light', reducedMotion: true, touch: true },
];

const CLEAN = [
  'all',
  'screen',
  'print',
  'not print',
  'only screen',
  'not all',
  'only all',
  'not screen',
  'SCREEN',
  'Screen and (MIN-WIDTH: 600PX)',
  'screen and (min-width: 600px)',
  'screen and (min-width:600px)',
  '(width >= 600px)',
  '(600px <= width)',
  '(400px < width < 800px)',
  '(width = 600px)',
  '(width: 600px)',
  '(min-width: 36.25em)',
  '(max-width: 40rem)',
  '(min-height: 20px) and (orientation: landscape)',
  '(orientation: portrait)',
  '(orientation: landscape)',
  '(orientation: LANDSCAPE)',
  '(prefers-color-scheme: dark)',
  '(prefers-color-scheme: light)',
  '(prefers-color-scheme: DARK)',
  '(prefers-reduced-motion: reduce)',
  '(prefers-reduced-motion)',
  '(prefers-reduced-motion: no-preference)',
  '(hover: hover)',
  '(hover: none)',
  '(any-hover: hover)',
  '(any-hover: none)',
  '(pointer: fine)',
  '(pointer: coarse)',
  '(any-pointer: coarse)',
  '(any-pointer: fine)',
  '(hover: HOVER)',
  '(pointer: fine) and (hover: hover)',
  '(resolution: 2dppx)',
  '(min-resolution: 1.5dppx)',
  '(max-resolution: 144dpi)',
  '(min-resolution: 96dpi)',
  '(resolution: 1.5x)',
  '(min-resolution: 2dppx)',
  '(resolution: 2dpcm)',
  '(width)',
  '(hover)',
  '(orientation)',
  '(prefers-color-scheme)',
  '(resolution)',
  '(height)',
  '(min-width: 0)',
  '(min-width: 0px)',
  '(width: 0)',
  '(max-width: 0)',
  '(min-width: 100px) and (min-width: 0)',
  '(min-width: 1em)',
  '(min-width: 16px)',
  '(min-width: 1rem)',
  '(min-width: .5px)',
  '(min-width: +600px)',
  '(min-width: 6e2px)',
  '(min-width: 6E2PX)',
  '(min-width: 600.0px)',
  '(min-resolution: 1.5DPPX)',
  '(resolution: 96DPI)',
  '(min-resolution: 0.5dppx)',
  '(min-resolution: 0dppx)',
  '(resolution: 0dppx)',
  '(min-width: 600px) and (max-width: 600px)',
  '(width >= 600px) and (height < 800px)',
  '(width >= 600px) and (width <= 600px)',
  '(width >= 600px) and (width >= 800px)',
  '(height = 600px)',
  '(min-height: 600px)',
  '(max-height: 599px)',
  '(height >= 600px)',
  '(height > 600px)',
  '(400px > width)',
  '(600px = width)',
  '(width>=600px)',
  '(width>600px)',
  '(width<=600px)',
  '(600px<width)',
  '(600px>=width)',
  '(min-width: 800px) and (min-width: 600px)',
  '(not (min-width: 800px))',
  '((min-width: 800px) or (max-width: 500px))',
  '(min-width: 800px) and ((min-width: 600px) or (height>1px))',
  '(min-width: 800px) or (min-width: 9000px)',
  '(min-width: 600px) AND (orientation: landscape)',
  '(min-width: 600px) OR (orientation: landscape)',
  '(min-width: 600px) and (orientation: portrait)',
  'screen and (min-width: 600px) and (orientation: landscape)',
  'screen, print',
  'print, screen and (min-width: 1px)',
  '(min-width: 10px), (max-width: 5px)',
  'not screen, print',
  '(prefers-reduced-motion: reduce) and (prefers-color-scheme: dark)',
  'not print and (min-width: 1px)',
  'print and (min-width:1px)',
  '(width > 0)',
  '(width < 0)',
  '(width > 0) and (height > 0)',
  '(min-width: 1e3px)',
  '(min-width: 1000px)',
  '(max-width: 1024px) and (min-height: 1px)',
  '(min-width: 1px) and (max-width: 99999px)',
  '(600px <= width <= 800px)',
  '(300px < width <= 600px)',
  '(400px <= width < 1000px)',
  '(width > 0px) and (width < 2000px)',
  'only print',
  '(width >= 600px) and (600px >= width) and (width)',
];
const INVALID = [
  '(min-height)',
  'not (min-width: 1px) and (width)',
  '(min-width: 5)',
  '(width: 800)',
  '(min-width:)',
  'screen and',
  'and screen',
  'screen or print',
  '(min-width: 600px) and screen',
  'not (min-width: 1px) or (width > 0)',
  '(min-width: 1px) and not (foo)',
  'not screen and (min-width: 1px) and',
  '(min-width: 600px)and(max-width: 1000px)',
  'screen and(min-width:600px)',
  '(min-width:600px)or(orientation:portrait)',
  '(hover: maybe)',
  '(orientation: sideways)',
  '(prefers-color-scheme: none)',
  '(prefers-color-scheme >= dark)',
  '(hover = hover)',
  '(600px >= 700px)',
  'only',
  'not',
  'screen not',
  'screen (min-width: 1px)',
  '(min-width: 1px) (max-width: 9px)',
  '(not screen)',
  '(min-width: 600.px)',
  '(min-width: 1px) and (min-width: 2px) or (width)',
  '(width: 600px) (height: 1px)',
  'screen and (min-width: 600px) and print',
];
const UNKNOWN_LOGIC = [
  'screen and (min-width: 1px), foo',
  'screen, (min-width: 5)',
  'foo, screen',
  'not tv',
  'not speech',
  'tv and (min-width: 1px)',
  '(min-width: 1px) or (foo: bar)',
  '(min-width: 1px) and (foo)',
  'not (foo: bar)',
  '(max-width: 1px) or (foo)',
  '(foo) or (max-width: 1px)',
  '(foo: bar) and (min-width: 1px)',
  'not (foo) and (min-width: 1px)',
  '(min-width: 1px) or (foo) or (bar: 1)',
  'not ((min-width: 1px) and (foo))',
];
const UNSUPPORTED = [
  'screen and (color)',
  '(color)',
  '(color: 8)',
  '(scripting: enabled)',
  '(grid)',
  '(monochrome)',
  '(aspect-ratio: 4/3)',
  '(min-aspect-ratio: 1/1)',
  '(display-mode: standalone)',
  '(inverted-colors: none)',
  '(prefers-contrast: more)',
  '(forced-colors: active)',
  '(update: fast)',
  '(max-color: 8)',
  '(-webkit-min-device-pixel-ratio: 2)',
  '(min-width: 2vw)',
  '(min-width: 10ch)',
  '(min-width: -1px)',
  'tv',
  'speech',
  '(min-width: 600px',
  '(min-width: 1px) and (color)',
  '(color >= 1)',
  '(scripting)',
  'print and (color)',
  '(max-width: 0px) and (scripting: none)',
];

const CATEGORIES = [
  ['clean', CLEAN],
  ['invalid', INVALID],
  ['diagnosed', UNKNOWN_LOGIC],
  ['unsupported', UNSUPPORTED],
];

const APPLY_FEATURES = (env) => [
  { name: 'prefers-color-scheme', value: env.colorScheme },
  { name: 'prefers-reduced-motion', value: env.reducedMotion ? 'reduce' : 'no-preference' },
];

const OBSERVE = (queries) => ({
  innerWidth,
  innerHeight,
  devicePixelRatio,
  results: queries.map((q) => {
    try {
      return matchMedia(q).matches;
    } catch (e) {
      return 'ERR:' + e.name;
    }
  }),
});

export async function generate(outDir) {
  const queries = CATEGORIES.flatMap(([category, list]) => list.map((q) => ({ q, category })));
  const results = queries.map(() => []);
  const observed = [];
  await withPage({ viewport: { width: 800, height: 600, dpr: 1 } }, async (page) => {
    await page.setContent('<!doctype html><html><head></head><body></body></html>');
    for (const env of ENVS) {
      await page.viewport(env.width, env.height, env.dpr);
      await page.send('Emulation.setEmulatedMedia', { features: APPLY_FEATURES(env) });
      if (env.touch) await page.send('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 1 });
      else await page.send('Emulation.setTouchEmulationEnabled', { enabled: false });
      const obs = await page.evaluate(OBSERVE, queries.map((x) => x.q));
      observed.push({ env: env.name, innerWidth: obs.innerWidth, innerHeight: obs.innerHeight, devicePixelRatio: obs.devicePixelRatio });
      obs.results.forEach((r, i) => results[i].push(r));
    }
    await page.send('Emulation.setTouchEmulationEnabled', { enabled: false });
  });
  const bad = results.flat().filter((r) => typeof r !== 'boolean');
  if (bad.length) throw new Error(`non-boolean matchMedia results: ${bad.slice(0, 5).join(', ')}`);
  const body = {
    generator: 'tests/css/gen/gen-media-corpus.mjs',
    chrome: 'headless (tooling/chrome); matchMedia',
    envs: ENVS,
    observed,
    queries: queries.map((x, i) => ({ q: x.q, category: x.category, results: results[i] })),
  };
  return writeCorpus('media.json', body, outDir);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const file = await generate();
  console.log('wrote', file);
}

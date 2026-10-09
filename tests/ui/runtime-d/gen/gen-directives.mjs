// Oracle generator for the directive corpus (WP-RT-D): v-model, v-show, withModifiers and withKeys. Runs every case of
// cases-directives.mjs in headless Chrome against the real @vue/runtime-dom 3.5.43 and writes
// tests/ui/runtime-d/corpora/directives.json. Run from the repo root (or the mirror):
//   node tests/ui/runtime-d/gen/gen-directives.mjs
// Deterministic: the case order is fixed and nothing machine-specific is written.
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { withPage, serveDirectory } from '../../../../tooling/chrome/chrome.mjs';
import { pageRun } from './page-run.mjs';
import { cases, HANDLERS } from './cases-directives.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../../..');
const out = path.join(root, 'tests/ui/runtime-d/corpora/directives.json');
const MODULE = '/node_modules/@vue/runtime-dom/dist/runtime-dom.esm-browser.prod.js';

const recorded = [];
const site = await serveDirectory(root);
try {
  await withPage({ viewport: { width: 800, height: 600, dpr: 1 } }, async (page) => {
    await page.goto(new URL('/tests/ui/runtime-d/gen/page.html', site.url).href);
    for (const kase of cases) {
      const results = await page.evaluate(pageRun, kase, MODULE);
      recorded.push({ name: kase.name, handlers: kase.handlers, steps: kase.steps, results });
    }
  });
} finally {
  await site.close();
}

const count = (op) => recorded.reduce((n, c) => n + c.steps.filter((s) => s.op === op).length, 0);
const header = {
  format: 'vn-runtime-d/directives@1',
  oracle: 'vue@3.5.43 runtime-dom.esm-browser.prod.js in headless Chrome (tooling/chrome)',
  handlers: HANDLERS,
  counts: { cases: recorded.length, directiveCalls: count('directive'), wraps: count('wrap'), dispatches: count('dispatch'), snapshots: count('snap') },
};
const lines = [JSON.stringify(header), ...recorded.map((c) => JSON.stringify(c))];
writeFileSync(out, lines.join(',\n').replace(/^/, '[\n').concat('\n]\n'));
console.log(`directives corpus: ${recorded.length} cases, ${count('directive')} directive calls, ${count('wrap')} wrappers, ${count('dispatch')} dispatches -> ${path.relative(root, out)}`);

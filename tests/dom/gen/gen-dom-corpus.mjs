// Generates tests/dom/corpora/<group>.json (WP-U0). Every expected value is recorded from headless Chrome (tooling/chrome):
// the Rust replay in crates/dom/tests compares vn-dom against these files. Run: node tests/dom/gen/gen-dom-corpus.mjs
// Deterministic: same cases, same Chrome, same bytes. Chrome is closed before the script exits (withPage).
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { withPage } from '../../../tooling/chrome/chrome.mjs';
import { pageRunner } from './page-runner.mjs';
import { CASES } from './builder.mjs';
import './cases/tree.mjs';
import './cases/attr.mjs';
import './cases/prop.mjs';
import './cases/form.mjs';
import './cases/style.mjs';
import './cases/class.mjs';
import './cases/event.mjs';
import './cases/focus.mjs';
import './cases/serial.mjs';
import './cases/query.mjs';
import './cases/activation.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const outDir = path.resolve(here, '..', 'corpora');

const groups = new Map();
const started = Date.now();
await withPage({ viewport: { width: 800, height: 600, dpr: 1 }, timeout: 30000 }, async (page) => {
  for (const c of CASES) {
    const steps = JSON.parse(await page.evaluate(pageRunner, { ops: c.ops, listeners: c.listeners }));
    steps.forEach((s, i) => {
      for (const e of s.log) {
        if ('jserror' in e) throw new Error(`${c.name}, op ${i}: JavaScript error in the page runner: ${e.jserror}`);
      }
    });
    const group = c.name.split('/')[0];
    if (!groups.has(group)) groups.set(group, []);
    groups.get(group).push({ name: c.name, ops: c.ops, listeners: c.listeners, steps });
  }
});

fs.mkdirSync(outDir, { recursive: true });
for (const [group, cases] of groups) {
  const body = '[\n' + cases.map((c) => JSON.stringify(c)).join(',\n') + '\n]\n';
  fs.writeFileSync(path.join(outDir, `${group}.json`), body);
  console.log(`${group}.json: ${cases.length} cases`);
}
console.log(`total: ${CASES.length} cases in ${groups.size} files, ${((Date.now() - started) / 1000).toFixed(1)} s`);

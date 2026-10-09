#!/usr/bin/env node
// Writes goldens with the Node oracle (WP-C1b, lowering scenarios): node tooling/harness/write-goldens.mjs [--dir d] [--golden g]
import { mkdir, writeFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { runScenarioLine, scenarioName } from '../oracle/node-runner.mjs';
import { ROOT } from './build.mjs';

const args = process.argv.slice(2);
const opt = (name, dflt) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : dflt;
};
const dir = path.resolve(ROOT, opt('--dir', 'tests/language/lowering'));
const golden = path.resolve(ROOT, opt('--golden', 'tests/language/lowering-golden'));

const files = (await readdir(dir)).filter((f) => f.endsWith('.js')).sort().map((f) => path.join(dir, f));
for (const file of files) {
  const line = await runScenarioLine(file);
  const out = path.join(golden, `${scenarioName(file)}.js.json`);
  await mkdir(path.dirname(out), { recursive: true });
  await writeFile(out, line);
}
console.log(`wrote ${files.length} golden(s) to ${path.relative(ROOT, golden)}`);

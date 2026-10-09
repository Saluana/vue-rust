// Differential run of the WP-RT-B scenarios (tests/vue/cases/{watch,scheduler}) against the goldens written by the real
// vue@3.5.43 production build (tests/vue/golden). Each scenario is compiled to Rust by the compiler, linked with
// vn-runtime (its `vue` module, via exports::register_vue_module) into one throw-away crate, and run in its own process.
//   node tests/vue/runtime-b/run.mjs [--group watch|scheduler|effect|computed|ref|reactive] [--filter text] [--verbose]
// Exit code 1 when a scenario fails or does not build. The crate lives in target/scenarios/rtb (its own package name).
import { readFileSync, existsSync } from 'node:fs';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { compileModule } from '../../../compiler/index.mjs';
import { compareDocs, describeDiff } from '../../../tooling/oracle/compare.mjs';
import { writeCrate, buildCrate, runOne, ROOT, TARGET_DIR } from '../../../tooling/harness/build.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const casesRoot = path.join(ROOT, 'tests', 'vue', 'cases');
const goldenRoot = path.join(ROOT, 'tests', 'vue', 'golden');
const crateDir = path.join(ROOT, 'target', 'scenarios', 'rtb');
const pkg = 'vn-rtb-scenarios';
const binary = path.join(TARGET_DIR, 'debug', pkg);

const args = process.argv.slice(2);
const opt = (name) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : null;
};
const groupArg = opt('--group');
const filter = opt('--filter') ?? '';
const verbose = args.includes('--verbose');
const groups = groupArg ? [groupArg] : ['watch', 'scheduler'];

async function listJs(dir) {
  const out = [];
  for (const e of await readdir(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) out.push(...(await listJs(p)));
    else if (e.name.endsWith('.js')) out.push(p);
  }
  return out.sort();
}

async function main() {
  const results = [];
  const modules = [];
  for (const group of groups) {
    const dir = path.join(casesRoot, group);
    if (!existsSync(dir)) continue;
    for (const file of await listJs(dir)) {
      const name = path.relative(casesRoot, file).split(path.sep).join('/').replace(/\.js$/, '');
      if (!name.includes(filter)) continue;
      const src = await readFile(file, 'utf8');
      const c = compileModule(src, { filename: path.relative(ROOT, file), lang: 'js', specifier: name });
      if (c.rust === null) {
        const d = c.diagnostics.find((x) => x.severity === 'error') ?? c.diagnostics[0];
        results.push({ name, group, outcome: 'compile-error', reason: d ? `${d.code} ${d.message}` : 'no Rust produced' });
        continue;
      }
      modules.push({ name, rust: c.rust });
      results.push({ name, group, outcome: null });
    }
  }
  if (modules.length > 0) {
    writeCrate(modules, { dir: crateDir, pkg, vue: true });
    const b = buildCrate(crateDir);
    if (!b.ok) {
      console.log(`BUILD FAILED: ${b.output.split('\n').filter((l) => /^error/.test(l)).slice(0, 4).join(' | ') || b.output.slice(0, 600)}`);
      process.exitCode = 1;
      return;
    }
  }
  for (const r of results) {
    if (r.outcome !== null) continue;
    const gfile = path.join(goldenRoot, `${r.name}.js.json`);
    const run = runOne(r.name, binary);
    if (!run.ok) {
      r.outcome = 'FAIL';
      r.reason = run.error;
      continue;
    }
    if (!existsSync(gfile)) {
      r.outcome = 'FAIL';
      r.reason = 'missing golden';
      continue;
    }
    const golden = JSON.parse(readFileSync(gfile, 'utf8'));
    const d = compareDocs(run.doc, golden);
    r.outcome = d === null ? 'pass' : 'FAIL';
    if (d !== null) r.reason = describeDiff(d);
  }
  const score = {};
  for (const r of results) {
    score[r.group] ??= { pass: 0, total: 0 };
    score[r.group].total += 1;
    if (r.outcome === 'pass') score[r.group].pass += 1;
    if (r.outcome !== 'pass' || verbose) console.log(`${r.outcome.padEnd(14)} ${r.name}${r.reason ? `  :: ${String(r.reason).slice(0, 300)}` : ''}`);
  }
  for (const g of Object.keys(score)) console.log(`${g}: ${score[g].pass}/${score[g].total} match their goldens`);
  if (results.some((r) => r.outcome !== 'pass')) process.exitCode = 1;
}

main().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});

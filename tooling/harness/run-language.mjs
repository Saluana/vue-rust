#!/usr/bin/env node
// Differential run of language scenarios (WP-C1b): compile each scenario to Rust, build one binary, run each scenario
// and compare its document with the golden (structural compare, tooling/oracle/compare.mjs).
//   node tooling/harness/run-language.mjs [--dir tests/language/cases] [--golden <dir>] [--filter text] [--write-status]
//   node tooling/harness/run-language.mjs --with-vue --ui [--filter text] [--write-status]   (UI scenarios, ui-mode.mjs)
//                                         [--status <ledger.json>] [--with-vue] [--verbose]
// Outcomes: pass | FAIL (mismatch, panic, run error) | compile-error. Status ledger (cases, and any --status file):
// "pass" | {"blocked": reason}. Exit 1 when a pass-marked scenario is not pass, a blocked one passes (ratchet), or an
// unlisted scenario is not pass. Without a ledger (other directories) every scenario must pass.
// --with-vue (WP-C1c follow-up): the scenarios run against the Rust Vue runtime. The crate depends on vn-runtime and its
// realm setup registers the native module "vue" (vn-runtime::exports::register_vue_module). Defaults: --dir tests/vue/cases,
// --golden tests/vue/golden, --status tests/vue/status.json. Scenario names are paths under --dir (watch/flush-order-...).
// A failing Vue scenario gets its exact reason in the ledger: a missing export of 'vue', the thrown error, or the first
// difference from the golden. The summary counts each group (ref, reactive, computed, effect, watch, scheduler).
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkSource } from '../../compiler/frontend/check.mjs';
import { lowerToHir } from '../../compiler/hir/index.mjs';
import { emitModule } from '../../compiler/emit/index.mjs';
import { compareDocs, describeDiff } from '../oracle/compare.mjs';
import { scenarioName } from '../oracle/node-runner.mjs';
import { writeCrate, buildCrate, runOne, ROOT, CRATE_DIR, BINARY, VUE_CRATE_DIR, VUE_BINARY } from './build.mjs';
import { runUiScenarios } from './ui-mode.mjs';

const args = process.argv.slice(2);
const opt = (name, dflt) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : dflt;
};
const flag = (name) => args.includes(name);
const withVue = flag('--with-vue');
const dirArg = path.resolve(ROOT, opt('--dir', withVue ? 'tests/vue/cases' : 'tests/language/cases'));
const isCases = dirArg === path.join(ROOT, 'tests/language/cases');
const goldenDir = path.resolve(ROOT, opt('--golden', withVue ? 'tests/vue/golden' : isCases ? 'tests/language/golden' : `${path.relative(ROOT, dirArg)}-golden`));
const filter = opt('--filter', '');
const ledgerArg = opt('--status', withVue ? 'tests/vue/status.json' : isCases ? 'tests/language/status.json' : null);
const ledgerFile = ledgerArg === null ? null : path.resolve(ROOT, ledgerArg);
const useLedger = ledgerFile !== null;
const crate = withVue
  ? { dir: VUE_CRATE_DIR, pkg: 'vn-vue-scenarios', binary: VUE_BINARY }
  : { dir: CRATE_DIR, pkg: 'vn-scenarios', binary: BINARY };

// Scenario name: the basename for the language cases (their runner names them so), the path under --dir for Vue cases.
const nameOf = (file) => (withVue
  ? path.relative(dirArg, file).split(path.sep).join('/').replace(/\.js$/, '')
  : scenarioName(file));

async function listJs(dir) {
  const out = [];
  async function walk(d) {
    const entries = await readdir(d, { withFileTypes: true });
    entries.sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
    for (const e of entries) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) await walk(p);
      else if (e.isFile() && e.name.endsWith('.js')) out.push(p);
    }
  }
  await walk(dir);
  return out;
}

// The exact reason of a failing scenario: a missing export of the "vue" native module (a link error), a thrown error when
// the golden expects a normal result, or the first difference from the golden.
function failureReason(doc, golden, diff) {
  if (doc.result === 'panic') return `panic: ${String(doc.error).slice(0, 160)}`;
  if (doc.result === 'throw' && golden.result !== 'throw') {
    const err = doc.error && typeof doc.error === 'object' ? doc.error : { message: String(doc.error) };
    const message = String(err.message ?? '');
    const missing = /does not provide an export named '([^']+)'/.exec(message);
    if (missing) return `missing export '${missing[1]}' of 'vue' (${message})`;
    return `threw ${err.name ?? err.cls ?? 'error'}: ${message}`;
  }
  return describeDiff(diff);
}

function printGroups(results, ledger) {
  const groups = new Map();
  for (const r of results) {
    const g = r.name.split('/')[0];
    if (!groups.has(g)) groups.set(g, { total: 0, pass: 0, blocked: 0, FAIL: 0, 'compile-error': 0 });
    const c = groups.get(g);
    c.total += 1;
    const entry = ledger[r.name];
    if (r.outcome === 'pass') c.pass += 1;
    else if (r.outcome === 'compile-error') c['compile-error'] += 1;
    else if (entry !== undefined && entry !== 'pass') c.blocked += 1;
    else c.FAIL += 1;
  }
  console.log('group       total  pass  blocked  FAIL  compile-error');
  for (const [g, c] of [...groups.entries()].sort((a, b) => (a[0] < b[0] ? -1 : 1))) {
    console.log(`${g.padEnd(11)} ${String(c.total).padStart(5)} ${String(c.pass).padStart(5)} ${String(c.blocked).padStart(8)} ${String(c.FAIL).padStart(5)} ${String(c['compile-error']).padStart(14)}`);
  }
}

async function main() {
  const files = (await listJs(dirArg)).filter((f) => f.includes(filter));
  const ledger = useLedger && existsSync(ledgerFile) ? JSON.parse(readFileSync(ledgerFile, 'utf8')) : {};
  const results = [];
  const modules = [];
  for (const file of files) {
    const name = nameOf(file);
    const src = await readFile(file, 'utf8');
    const rel = path.relative(ROOT, file);
    // This suite tests lowering/runtime semantics below the closed-world global policy. Some cases deliberately
    // use undeclared names to exercise ReferenceError; application entry points enforce the stdlib manifest.
    const checked = checkSource(src, { filename: rel, lang: 'js', target: 'test' });
    const lowered = checked.ast && !checked.diagnostics.some(d => d.severity === 'error')
      ? lowerToHir(checked.ast, checked.analysis, { source: src, filename: rel, specifier: name, lang: 'js' })
      : { hir: null, diagnostics: [] };
    const c = { rust: lowered.hir ? emitModule(lowered.hir) : null, diagnostics: [...checked.diagnostics, ...lowered.diagnostics] };
    if (c.rust === null) {
      const d = c.diagnostics.find((x) => x.severity === 'error') ?? c.diagnostics[0];
      results.push({ name, outcome: 'compile-error', reason: d ? `${d.code} ${d.message}` : 'no Rust produced' });
      continue;
    }
    modules.push({ name, rust: c.rust });
    results.push({ name, outcome: null, file });
  }

  if (modules.length > 0) {
    writeCrate(modules, { dir: crate.dir, pkg: crate.pkg, vue: withVue });
    const b = buildCrate(crate.dir);
    if (!b.ok) {
      const first = b.output.split('\n').filter((l) => /^error/.test(l)).slice(0, 3).join(' | ');
      console.log(`BUILD FAILED: ${first || b.output.slice(0, 400)}`);
      process.exitCode = 1;
      return;
    }
  }

  for (const r of results) {
    if (r.outcome !== null) continue;
    const gfile = path.join(goldenDir, `${r.name}.js.json`);
    const run = runOne(r.name, crate.binary);
    if (!run.ok) { r.outcome = 'FAIL'; r.reason = run.error; continue; }
    if (run.doc.result === 'panic') { r.outcome = 'FAIL'; r.reason = `panic: ${String(run.doc.error).slice(0, 200)}`; continue; }
    if (!existsSync(gfile)) { r.outcome = 'FAIL'; r.reason = 'missing golden'; continue; }
    const golden = JSON.parse(readFileSync(gfile, 'utf8'));
    const diff = compareDocs(run.doc, golden);
    r.outcome = diff === null ? 'pass' : 'FAIL';
    if (diff !== null) r.reason = withVue ? failureReason(run.doc, golden, diff) : describeDiff(diff);
  }

  let failures = 0;
  const counts = { pass: 0, FAIL: 0, blocked: 0, 'compile-error': 0 };
  const ledgerOut = { ...ledger };
  for (const r of results) {
    const entry = ledger[r.name];
    let status = r.outcome;
    let unexpected = false;
    if (useLedger) {
      if (entry === undefined) {
        unexpected = r.outcome !== 'pass';
      } else if (entry === 'pass') {
        unexpected = r.outcome !== 'pass';
      } else if (r.outcome === 'pass') {
        unexpected = true; // ratchet: a blocked scenario that passes must be flipped to pass
        status = 'pass';
      } else {
        status = 'blocked';
      }
      if (unexpected) failures += 1;
      if (flag('--write-status') && withVue) {
        ledgerOut[r.name] = r.outcome === 'pass' ? 'pass' : { blocked: r.reason ?? r.outcome };
      } else if (flag('--write-status') && r.outcome !== 'pass' && entry === undefined) {
        ledgerOut[r.name] = { blocked: r.reason ?? r.outcome };
      } else if (flag('--write-status') && r.outcome === 'pass' && entry !== undefined && entry !== 'pass') {
        ledgerOut[r.name] = 'pass';
      }
    } else if (r.outcome !== 'pass') {
      failures += 1;
      unexpected = true;
    }
    counts[status === 'blocked' ? 'blocked' : status] = (counts[status === 'blocked' ? 'blocked' : status] ?? 0) + 1;
    if (unexpected || (r.outcome !== 'pass' && !useLedger) || flag('--verbose')) {
      const blockedNote = entry && typeof entry === 'object' && !withVue ? ` [blocked: ${entry.blocked}]` : '';
      console.log(`${(status).padEnd(13)} ${r.name}${blockedNote}${r.reason ? `  -- ${r.reason.slice(0, 220)}` : ''}`);
    }
  }
  if (flag('--write-status') && useLedger) {
    writeFileSync(ledgerFile, `${JSON.stringify(sortKeys(ledgerOut), null, 1)}\n`);
  }
  const total = results.length;
  if (withVue) printGroups(results, ledger);
  console.log(`scenarios ${total}: pass ${counts.pass}, FAIL ${counts.FAIL}, blocked ${counts.blocked}, compile-error ${counts['compile-error']}; ${failures} unexpected`);
  process.exitCode = failures === 0 ? 0 : 1;
}

function sortKeys(o) {
  return Object.fromEntries(Object.keys(o).sort().map((k) => [k, o[k]]));
}

if (flag('--ui')) runUiScenarios({ filter, writeStatus: flag('--write-status'), verbose: flag('--verbose') });
else await main();

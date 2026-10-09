#!/usr/bin/env node
// Differential run of language scenarios (WP-C1b): compile each scenario to Rust, build one binary, run each scenario
// and compare its document with the golden (structural compare, tooling/oracle/compare.mjs).
//   node tooling/harness/run-language.mjs [--dir tests/language/cases] [--golden <dir>] [--filter text] [--write-status]
// Outcomes: pass | FAIL (mismatch, panic, run error) | compile-error. Status ledger (cases only, tests/language/status.json):
// "pass" | {"blocked": reason}. Exit 1 when a pass-marked scenario is not pass, a blocked one passes (ratchet), or an
// unlisted scenario is not pass. Without a ledger (other directories) every scenario must pass.
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { compileModule } from '../../compiler/index.mjs';
import { compareDocs, describeDiff } from '../oracle/compare.mjs';
import { scenarioName } from '../oracle/node-runner.mjs';
import { writeCrate, buildCrate, runOne, ROOT } from './build.mjs';

const args = process.argv.slice(2);
const opt = (name, dflt) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : dflt;
};
const flag = (name) => args.includes(name);
const dirArg = path.resolve(ROOT, opt('--dir', 'tests/language/cases'));
const isCases = dirArg === path.join(ROOT, 'tests/language/cases');
const goldenDir = path.resolve(ROOT, opt('--golden', isCases ? 'tests/language/golden' : `${path.relative(ROOT, dirArg)}-golden`));
const filter = opt('--filter', '');
const statusFile = path.join(ROOT, 'tests/language/status.json');

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

function firstDiff(actual, golden) {
  const d = compareDocs(actual, golden);
  return d === null ? null : describeDiff(d);
}

async function main() {
  const files = (await listJs(dirArg)).filter((f) => f.includes(filter));
  const ledger = isCases && existsSync(statusFile) ? JSON.parse(readFileSync(statusFile, 'utf8')) : {};
  const results = [];
  const modules = [];
  for (const file of files) {
    const name = scenarioName(file);
    const src = await readFile(file, 'utf8');
    const rel = path.relative(ROOT, file);
    const c = compileModule(src, { filename: rel, lang: 'js', specifier: name });
    if (c.rust === null) {
      const d = c.diagnostics.find((x) => x.severity === 'error') ?? c.diagnostics[0];
      results.push({ name, outcome: 'compile-error', reason: d ? `${d.code} ${d.message}` : 'no Rust produced' });
      continue;
    }
    modules.push({ name, rust: c.rust });
    results.push({ name, outcome: null, file });
  }

  if (modules.length > 0) {
    writeCrate(modules);
    const b = buildCrate();
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
    const run = runOne(r.name);
    if (!run.ok) { r.outcome = 'FAIL'; r.reason = run.error; continue; }
    if (run.doc.result === 'panic') { r.outcome = 'FAIL'; r.reason = `panic: ${String(run.doc.error).slice(0, 200)}`; continue; }
    if (!existsSync(gfile)) { r.outcome = 'FAIL'; r.reason = 'missing golden'; continue; }
    const golden = JSON.parse(readFileSync(gfile, 'utf8'));
    const diff = firstDiff(run.doc, golden);
    r.outcome = diff === null ? 'pass' : 'FAIL';
    if (diff !== null) r.reason = diff;
  }

  let failures = 0;
  const counts = { pass: 0, FAIL: 0, blocked: 0, 'compile-error': 0 };
  const ledgerOut = { ...ledger };
  for (const r of results) {
    const entry = ledger[r.name];
    let status = r.outcome;
    let unexpected = false;
    if (isCases) {
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
      if (flag('--write-status') && r.outcome !== 'pass' && entry === undefined) {
        ledgerOut[r.name] = { blocked: r.reason ?? r.outcome };
      }
      if (flag('--write-status') && r.outcome === 'pass' && entry !== undefined && entry !== 'pass') ledgerOut[r.name] = 'pass';
    } else if (r.outcome !== 'pass') {
      failures += 1;
      unexpected = true;
    }
    counts[status === 'blocked' ? 'blocked' : status] = (counts[status === 'blocked' ? 'blocked' : status] ?? 0) + 1;
    if (unexpected || (r.outcome !== 'pass' && !isCases) || flag('--verbose')) {
      const blockedNote = entry && typeof entry === 'object' ? ` [blocked: ${entry.blocked}]` : '';
      console.log(`${(status).padEnd(13)} ${r.name}${blockedNote}${r.reason ? `  -- ${r.reason.slice(0, 220)}` : ''}`);
    }
  }
  if (flag('--write-status') && isCases) writeFileSync(statusFile, `${JSON.stringify(sortKeys(ledgerOut), null, 1)}\n`);
  const total = results.length;
  console.log(`scenarios ${total}: pass ${counts.pass}, FAIL ${counts.FAIL}, blocked ${counts.blocked}, compile-error ${counts['compile-error']}; ${failures} unexpected`);
  process.exitCode = failures === 0 ? 0 : 1;
}

function sortKeys(o) {
  return Object.fromEntries(Object.keys(o).sort().map((k) => [k, o[k]]));
}

await main();

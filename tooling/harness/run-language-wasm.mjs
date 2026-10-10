#!/usr/bin/env node
// Task 4.2: the language scenarios executed as wasm32-unknown-unknown. Each scenario module is compiled as
// run-language.mjs compiles it. The modules form one cdylib (build.mjs writeWasmCrate) whose exports are the entry of
// vn_testkit::wasm. Every scenario runs in its own WebAssembly instance, so a trap fails that scenario only. Each document is
// compared with the same golden as the native run (tooling/oracle/compare.mjs).
//   node tooling/harness/run-language-wasm.mjs [--dir tests/language/cases] [--golden <dir>] [--filter text] [--gc-threshold N] [--verbose]
// Exit 1 unless every scenario passes. The module must have no imports (the harness fails otherwise): the host supplies
// nothing, so the scenarios see no clock, random source or console. Their clock is the scenario host's own fake clock.
import { existsSync, readFileSync } from 'node:fs';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';
import { checkSource } from '../../compiler/frontend/check.mjs';
import { lowerToHir } from '../../compiler/hir/index.mjs';
import { emitModule } from '../../compiler/emit/index.mjs';
import { compareDocs, describeDiff } from '../oracle/compare.mjs';
import { scenarioName } from '../oracle/node-runner.mjs';
import { ROOT, WASM_BINARY, WASM_CRATE_DIR, WASM_TARGET, buildWasmCrate, writeWasmCrate } from './build.mjs';

const NO_SUCH_SCENARIO = 0xffffffff; // u32::MAX, what the wasm entry returns for an index outside its table
const decoder = new TextDecoder('utf-8', { fatal: true });

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

// The compile step of run-language.mjs for one file: the Rust module, or the diagnostic that stops it.
async function compileScenario(file, name) {
  const src = await readFile(file, 'utf8');
  const rel = path.relative(ROOT, file);
  // As in run-language.mjs: this suite checks language semantics below the closed-world policy (undeclared names are allowed).
  const checked = checkSource(src, { filename: rel, lang: 'js', target: 'test' });
  const lowered = checked.ast && !checked.diagnostics.some((d) => d.severity === 'error')
    ? lowerToHir(checked.ast, checked.analysis, { source: src, filename: rel, specifier: name, lang: 'js' })
    : { hir: null, diagnostics: [] };
  const rust = lowered.hir ? emitModule(lowered.hir) : null;
  if (rust !== null) return { rust, reason: null };
  const diagnostics = [...checked.diagnostics, ...lowered.diagnostics];
  const d = diagnostics.find((x) => x.severity === 'error') ?? diagnostics[0];
  return { rust: null, reason: d ? `${d.code} ${d.message}` : 'no Rust produced' };
}

// The text in the output buffer of an instance, after a call that returned its length.
function readOut(ex, len) {
  return decoder.decode(new Uint8Array(ex.memory.buffer, ex.vn_out_ptr(), len));
}

// One scenario in a fresh instance. Returns { doc } or { failure }: a trap, an instantiation error, or output that is not
// one JSON document. A trap is the scenario's failure; the other scenarios still run. `gcThreshold` (undefined: the kernel
// default) is the allocation count between the collections at the safepoints.
function runOnWasm(module, index, gcThreshold) {
  let ex;
  try {
    ex = new WebAssembly.Instance(module, {}).exports;
  } catch (e) {
    return { failure: `instantiate failed: ${e.message}` };
  }
  let len;
  try {
    len = gcThreshold === undefined ? ex.vn_run_scenario(index) : ex.vn_run_scenario_gc(index, gcThreshold);
  } catch (e) {
    return { failure: `trap: ${e.name}: ${e.message}` };
  }
  if (len === NO_SUCH_SCENARIO) return { failure: `index ${index} is outside the wasm table` };
  try {
    return { doc: JSON.parse(readOut(ex, len)) };
  } catch (e) {
    return { failure: `output is not one JSON document: ${e.message}` };
  }
}

// Runs the corpus on wasm32. `dir`, `golden` and `filter` select the scenarios as in run-language.mjs. `only` is an exact list
// of scenario names (for tests). `controls` are negative-control rows { name, body } (writeWasmCrate): each must fail.
// `gcThreshold` (optional) is the collection threshold of every scenario: 1 collects at every safepoint (the stress run).
// Returns { results, failures, wasm, gcThreshold }. `failures` counts the unexpected outcomes: a scenario that is not pass, a
// compile error, or a control that ran to a document. A harness failure (build, table, imports) throws.
export async function runWasmScenarios({ dir = 'tests/language/cases', golden, filter = '', only, controls = [], gcThreshold } = {}) {
  if (gcThreshold !== undefined && !(Number.isInteger(gcThreshold) && gcThreshold >= 0 && gcThreshold <= 0xffffffff)) {
    throw new Error(`gcThreshold must be an integer from 0 to 2^32-1, not ${gcThreshold}`);
  }
  const dirArg = path.resolve(ROOT, dir);
  const isCases = dirArg === path.join(ROOT, 'tests/language/cases');
  const goldenDir = path.resolve(ROOT, golden ?? (isCases ? 'tests/language/golden' : `${path.relative(ROOT, dirArg)}-golden`));
  const files = (await listJs(dirArg)).filter((f) => f.includes(filter) && (only === undefined || only.includes(scenarioName(f))));

  const results = [];
  const modules = [];
  for (const file of files) {
    const name = scenarioName(file);
    const { rust, reason } = await compileScenario(file, name);
    if (rust === null) results.push({ name, outcome: 'compile-error', reason });
    else {
      results.push({ name, outcome: null, index: modules.length });
      modules.push({ name, rust });
    }
  }
  controls.forEach((c, k) => results.push({ name: c.name, outcome: null, index: modules.length + k, control: true }));
  const countFailures = () => results.filter((r) => (r.control ? r.outcome !== 'FAIL' : r.outcome !== 'pass')).length;
  if (modules.length + controls.length === 0) return { results, failures: countFailures(), wasm: null, gcThreshold };

  writeWasmCrate(modules, { dir: WASM_CRATE_DIR, controls });
  const b = buildWasmCrate(WASM_CRATE_DIR);
  if (!b.ok) {
    const first = b.output.split('\n').filter((l) => /^error/.test(l)).slice(0, 3).join(' | ');
    throw new Error(`wasm build failed: ${first || b.output.slice(0, 400)}`);
  }

  const bytes = readFileSync(WASM_BINARY);
  const module = await WebAssembly.compile(bytes);
  const imports = WebAssembly.Module.imports(module);
  if (imports.length > 0) {
    throw new Error(`the wasm module imports ${imports.map((i) => `${i.module}.${i.name}`).join(', ')}; the scenario build must have no imports`);
  }
  const exportNames = WebAssembly.Module.exports(module).map((e) => e.name).filter((n) => n.startsWith('vn_'));
  const probe = (await WebAssembly.instantiate(module, {})).exports;
  const table = [...modules.map((m) => m.name), ...controls.map((c) => c.name)];
  if (probe.vn_scenario_count() !== table.length) {
    throw new Error(`the wasm table has ${probe.vn_scenario_count()} scenarios; the harness wrote ${table.length}`);
  }
  table.forEach((name, i) => {
    const got = readOut(probe, probe.vn_scenario_name(i));
    if (got !== name) throw new Error(`wasm table row ${i} is ${JSON.stringify(got)}; the harness wrote ${JSON.stringify(name)}`);
  });

  for (const r of results) {
    if (r.outcome === 'compile-error') continue; // not in the table
    const run = runOnWasm(module, r.index, gcThreshold);
    if (r.control) {
      // A control is correct when it fails: a trap, or a panic document. A normal document means it was not detected.
      const failed = run.failure !== undefined || run.doc.result === 'panic';
      r.outcome = failed ? 'FAIL' : 'missed';
      r.reason = run.failure ?? `ran to a ${run.doc.result} document`;
      continue;
    }
    if (run.failure !== undefined) { r.outcome = 'FAIL'; r.reason = run.failure; continue; }
    if (run.doc.result === 'panic') { r.outcome = 'FAIL'; r.reason = `panic: ${String(run.doc.error).slice(0, 200)}`; continue; }
    const gfile = path.join(goldenDir, `${r.name}.js.json`);
    if (!existsSync(gfile)) { r.outcome = 'FAIL'; r.reason = 'missing golden'; continue; }
    const diff = compareDocs(run.doc, JSON.parse(readFileSync(gfile, 'utf8')));
    r.outcome = diff === null ? 'pass' : 'FAIL';
    if (diff !== null) r.reason = describeDiff(diff);
  }

  const wasm = {
    file: WASM_BINARY,
    bytes: bytes.length,
    gzipBytes: gzipSync(bytes, { level: 9 }).length,
    imports: imports.length,
    exports: exportNames,
  };
  return { results, failures: countFailures(), wasm, gcThreshold };
}

// The report of run-language.mjs: a line per scenario that did not pass (every scenario with --verbose), then its summary line.
function printReport(out, verbose) {
  const w = out.wasm;
  if (w) {
    console.log(`${WASM_TARGET} ${w.file}: ${w.bytes} bytes raw, ${w.gzipBytes} bytes gzip (level 9), `
      + `exports ${w.exports.join(' ')}, imports ${w.imports}`);
  }
  if (out.gcThreshold !== undefined) {
    console.log(`gc threshold ${out.gcThreshold} (1 collects at every safepoint; the documents must not change)`);
  }
  const corpus = out.results.filter((r) => !r.control);
  for (const r of out.results) {
    if (r.outcome === 'pass' && !verbose) continue;
    const label = r.control ? 'control' : r.outcome;
    console.log(`${label.padEnd(13)} ${r.name}${r.reason ? `  -- ${r.reason.slice(0, 220)}` : ''}`);
  }
  const count = (o) => corpus.filter((r) => r.outcome === o).length;
  console.log(`scenarios ${corpus.length}: pass ${count('pass')}, FAIL ${count('FAIL')}, blocked 0, compile-error ${count('compile-error')}; ${out.failures} unexpected`);
}

async function main() {
  const args = process.argv.slice(2);
  const opt = (name, dflt) => {
    const i = args.indexOf(name);
    return i >= 0 ? args[i + 1] : dflt;
  };
  try {
    const gc = opt('--gc-threshold', undefined);
    const out = await runWasmScenarios({
      dir: opt('--dir', 'tests/language/cases'),
      golden: opt('--golden', undefined),
      filter: opt('--filter', ''),
      gcThreshold: gc === undefined ? undefined : Number(gc),
    });
    printReport(out, args.includes('--verbose'));
    process.exitCode = out.failures === 0 ? 0 : 1;
  } catch (e) {
    console.log(`ERROR: ${e.message}`);
    process.exitCode = 1;
  }
}

if (process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();

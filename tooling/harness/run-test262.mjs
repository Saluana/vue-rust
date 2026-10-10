#!/usr/bin/env node
// Runs the vendored Test262 subset (WP 3.1, tests/language/test262/) through the compiler. The expected result is the one
// the test states: the module completes without a throw, or a negative test throws its error at the stated phase.
//   node tooling/harness/run-test262.mjs [--dir <test dir>] [--harness <harness dir>] [--filter text]
//                                        [--status <ledger.json> | --no-ledger] [--write-status] [--verbose]
// Each test becomes one module: the harness files its metadata lists (assert.js and sta.js first, as test262 requires),
// then the test source. Test262 files are scripts; the module is strict, so the strict variant runs (select-test262.mjs).
// The module is checked with the frontend and the closed-world standard-library manifest (target "test", stdlib: true),
// lowered and emitted to Rust as tooling/harness/run-language.mjs does. All modules go into one crate, built once in debug
// mode, and each test runs in its own process (the 30 s limit of runOne).
// Outcomes:
//   pass                 the module completes (or, negative tests: throws the expected error type at runtime, or the
//                        compiler reports a syntax or early error VN1000/VN5001 for a negative "parse" test)
//   fail                 wrong result, unexpected throw, a negative test that did not throw its error, or a panic
//   rejected-by-profile  the compiler reported an error diagnostic (a profile code or an unexpected syntax error). This is
//                        not a pass. VN9001 (internal compiler error) is a fail.
// The ledger (default tests/language/test262/status.json) follows tests/language/status.json: "pass" or {"blocked": reason}.
// Exit 1 when a pass-marked test is not a pass, a blocked test passes (ratchet), or an unlisted test is not a pass.
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkSource } from '../../compiler/frontend/check.mjs';
import { lowerToHir } from '../../compiler/hir/index.mjs';
import { emitModule } from '../../compiler/emit/index.mjs';
import { writeCrate, buildCrate, runOne, ROOT, TARGET_DIR } from './build.mjs';
import { parseMeta } from './test262-meta.mjs';
import { VENDOR_DIR, ALWAYS_INCLUDED } from './select-test262.mjs';

const CRATE_DIR = path.join(ROOT, 'target', 'scenarios', 'test262');
const BINARY = path.join(TARGET_DIR, 'debug', 'vn-test262');
// Diagnostic codes that are syntax or early errors (SyntaxError at parse time, the phase test262 negative tests name).
const SYNTAX_CODES = new Set(['VN1000', 'VN5001']);

// The one adaptation of the vendored files (tests/language/test262/README.md, "Adaptation"). test262's Test262Error is a
// constructor function, and `new` of a function the program defines is outside the profile (L05, VN2007), so assert.js and
// the tests fail to compile for that reason alone. The adapted Test262Error is a plain call that returns { constructor,
// message }: its constructor property is the function itself, which is what assert.throws(Test262Error, ...) compares. Its
// prototype.toString is dropped with the constructor, since a function the program defines has no prototype object here.
// The vendored files are not changed: this is applied to the module text only.
const STA_CONSTRUCTOR = 'function Test262Error(message) {\n'
  + '  if (!(this instanceof Test262Error)) return new Test262Error(message);\n'
  + '  this.message = message || "";\n'
  + '}';
const STA_CALL_FORM = 'function Test262Error(message) {\n  return { constructor: Test262Error, message: message || "" };\n}';
const STA_PROTOTYPE = 'Test262Error.prototype.toString = function () {\n'
  + '  return "Test262Error: " + this.message;\n'
  + '};\n\n';

export function adaptForProfile(text, file) {
  let out = text;
  if (file === 'sta.js') {
    for (const old of [STA_CONSTRUCTOR, STA_PROTOTYPE]) {
      if (out.split(old).length !== 2) throw new Error(`sta.js no longer has the text the Test262Error adaptation expects: ${old.split('\n')[0]}`);
    }
    out = out.replace(STA_CONSTRUCTOR, STA_CALL_FORM).replace(STA_PROTOTYPE, '');
  }
  return out.replace(/new Test262Error\(/g, 'Test262Error(');
}

// The module text of one test: its harness files in test262's order, then the test source. Returns { text, prefixLines }.
export function moduleSource({ meta, testText, harnessDir }) {
  const names = [...ALWAYS_INCLUDED, ...meta.includes.filter((h) => !ALWAYS_INCLUDED.includes(h))];
  const parts = names.map((h) => {
    const file = path.join(harnessDir, h);
    if (!existsSync(file)) throw new Error(`missing harness file ${h}`);
    return adaptForProfile(readFileSync(file, 'utf8'), h);
  });
  const prefix = `${parts.join('\n')}\n`;
  return { text: `${prefix}${adaptForProfile(testText, null)}`, prefixLines: prefix.split('\n').length - 1 };
}

// Compiles one module text to Rust. Returns { rust, errors } where errors are the error-severity diagnostics.
export function compileModule(text, { filename, name }) {
  const checked = checkSource(text, { filename, lang: 'js', target: 'test', stdlib: true });
  if (checked.ast === null || checked.diagnostics.some((d) => d.severity === 'error')) {
    return { rust: null, errors: checked.diagnostics.filter((d) => d.severity === 'error') };
  }
  const lowered = lowerToHir(checked.ast, checked.analysis, { source: text, filename, specifier: name, lang: 'js' });
  const errors = lowered.diagnostics.filter((d) => d.severity === 'error');
  if (lowered.hir === null && errors.length === 0) {
    errors.push({ code: 'VN9001', severity: 'error', message: 'lowering produced no module', start: { line: 1 }, end: { line: 1 } });
  }
  if (lowered.hir === null || errors.length > 0) return { rust: null, errors };
  return { rust: emitModule(lowered.hir), errors: [] };
}

const describeDiagnostic = (d, prefixLines) => {
  const inHarness = d.start.line <= prefixLines;
  const where = inHarness ? `harness line ${d.start.line}` : `line ${d.start.line - prefixLines}`;
  return `${d.code} ${d.message} (${where})`;
};

// The outcome of a test whose module did not compile, or null when it compiled and must run.
export function classifyCompile(meta, errors, prefixLines) {
  if (errors.length === 0) return null;
  const codes = new Set(errors.map((d) => d.code));
  if (codes.has('VN9001')) return { outcome: 'fail', reason: `internal compiler error: ${describeDiagnostic(errors.find((d) => d.code === 'VN9001'), prefixLines)}` };
  if (meta.negative?.phase === 'parse' && errors.some((d) => SYNTAX_CODES.has(d.code))) {
    const first = errors.find((d) => SYNTAX_CODES.has(d.code));
    return { outcome: 'pass', reason: `expected ${meta.negative.type} at parse: ${describeDiagnostic(first, prefixLines)}` };
  }
  return { outcome: 'rejected-by-profile', reason: describeDiagnostic(errors[0], prefixLines) + (errors.length > 1 ? ` (+${errors.length - 1} more)` : '') };
}

const isNamedError = (value, type) => value !== null && typeof value === 'object' && value.cls === 'Error' && value.name === type;

// A short description of a thrown value from its trace encoding: an Error by name and message, a plain object (such as a
// Test262Error) by its message property, anything else as its encoding.
function describeThrown(value) {
  if (value !== null && typeof value === 'object' && value.cls === 'Error') return `${value.name}: ${value.message}`;
  const message = value?.props?.find(([key]) => key === 'message')?.[1];
  if (value?.cls === 'Object' && typeof message === 'string') return `thrown object with message ${JSON.stringify(message)}`;
  return JSON.stringify(value).slice(0, 160);
}

// The outcome of a test whose module ran. run is runOne's { ok, doc } or { ok: false, error }.
export function classifyRun(meta, run) {
  if (!run.ok) return { outcome: 'fail', reason: `run failed: ${run.error}` };
  const doc = run.doc;
  if (doc.result === 'panic') return { outcome: 'fail', reason: `panic: ${String(doc.error).slice(0, 200)}` };
  const neg = meta.negative;
  if (neg === null) {
    return doc.result === 'ok' ? { outcome: 'pass', reason: '' } : { outcome: 'fail', reason: `unexpected throw: ${describeThrown(doc.error)}` };
  }
  if (neg.phase === 'runtime' && doc.result === 'throw' && isNamedError(doc.error, neg.type)) {
    return { outcome: 'pass', reason: `threw ${neg.type} as expected` };
  }
  if (doc.result === 'ok') return { outcome: 'fail', reason: `expected ${neg.type} at ${neg.phase} time, but the module completed` };
  return { outcome: 'fail', reason: `expected ${neg.type} at ${neg.phase} time, got ${describeThrown(doc.error)}` };
}

// Whether a result is allowed by the ledger entry: a pass-marked test must pass, a blocked one must not, an unlisted one passes.
export function ledgerCheck(entry, outcome) {
  if (entry === undefined) return { ok: outcome === 'pass', status: outcome };
  if (entry === 'pass') return { ok: outcome === 'pass', status: outcome };
  if (outcome === 'pass') return { ok: false, status: 'pass' }; // ratchet: a blocked test that passes must be flipped
  return { ok: true, status: 'blocked' };
}

async function listTests(dir) {
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

const sortKeys = (o) => Object.fromEntries(Object.keys(o).sort().map((k) => [k, o[k]]));

async function main(args) {
  const opt = (name, dflt) => {
    const i = args.indexOf(name);
    return i >= 0 ? args[i + 1] : dflt;
  };
  const flag = (name) => args.includes(name);
  const testDir = path.resolve(ROOT, opt('--dir', path.join(path.relative(ROOT, VENDOR_DIR), 'test')));
  const harnessDir = path.resolve(ROOT, opt('--harness', path.join(path.relative(ROOT, VENDOR_DIR), 'harness')));
  const filter = opt('--filter', '');
  const noLedger = flag('--no-ledger');
  const ledgerFile = noLedger ? null : path.resolve(ROOT, opt('--status', path.join(path.relative(ROOT, VENDOR_DIR), 'status.json')));
  const ledger = ledgerFile !== null && existsSync(ledgerFile) ? JSON.parse(readFileSync(ledgerFile, 'utf8')) : {};

  const files = (await listTests(testDir)).filter((f) => path.relative(testDir, f).includes(filter));
  const results = [];
  const modules = [];
  for (const file of files) {
    const rel = path.relative(testDir, file).split(path.sep).join('/');
    const name = rel.replace(/\.js$/, '');
    const r = { name, outcome: null, reason: '' };
    results.push(r);
    try {
      const testText = await readFile(file, 'utf8');
      const meta = parseMeta(testText);
      const { text, prefixLines } = moduleSource({ meta, testText, harnessDir });
      const compiled = compileModule(text, { filename: rel, name });
      const rejected = classifyCompile(meta, compiled.errors, prefixLines);
      if (rejected !== null) {
        Object.assign(r, rejected);
        continue;
      }
      if (meta.negative?.phase === 'parse') {
        Object.assign(r, { outcome: 'fail', reason: `expected ${meta.negative.type} at parse time, but the compiler accepted the test` });
        continue;
      }
      modules.push({ name, rust: compiled.rust });
      r.meta = meta;
    } catch (err) {
      Object.assign(r, { outcome: 'fail', reason: `harness: ${err.message}` });
    }
  }

  if (modules.length > 0) {
    writeCrate(modules, { dir: CRATE_DIR, pkg: 'vn-test262' });
    const built = buildCrate(CRATE_DIR);
    if (!built.ok) {
      const first = built.output.split('\n').filter((l) => /^error/.test(l)).slice(0, 3).join(' | ');
      console.log(`BUILD FAILED: ${first || built.output.slice(0, 400)}`);
      process.exitCode = 1;
      return;
    }
  }

  for (const r of results) {
    if (r.outcome !== null) continue;
    const run = runOne(r.name, BINARY);
    const verdict = classifyRun(r.meta, run);
    Object.assign(r, verdict);
  }

  const counts = { pass: 0, fail: 0, 'rejected-by-profile': 0 };
  let blocked = 0;
  let unexpected = 0;
  const ledgerOut = { ...ledger };
  for (const r of results) {
    counts[r.outcome] += 1;
    const entry = ledger[r.name];
    const check = ledgerCheck(entry, r.outcome);
    if (!check.ok) unexpected += 1;
    if (check.status === 'blocked') blocked += 1;
    if (r.outcome !== 'pass' || flag('--verbose')) {
      const note = entry !== undefined && entry !== 'pass' ? ' [blocked]' : '';
      const ratchet = !check.ok ? ' [UNEXPECTED]' : '';
      console.log(`${r.outcome.padEnd(19)} ${r.name}${note}${ratchet}  -- ${r.reason.slice(0, 260)}`);
    }
    if (flag('--write-status') && ledgerFile !== null) {
      ledgerOut[r.name] = r.outcome === 'pass' ? 'pass' : { blocked: `${r.outcome}: ${r.reason}`.slice(0, 400) };
    }
  }
  if (flag('--write-status') && ledgerFile !== null) {
    if (filter === '') for (const k of Object.keys(ledgerOut)) if (!results.some((r) => r.name === k)) delete ledgerOut[k];
    writeFileSync(ledgerFile, `${JSON.stringify(sortKeys(ledgerOut), null, 1)}\n`);
  }
  console.log(`test262 ${results.length} files: pass ${counts.pass}, fail ${counts.fail}, rejected-by-profile ${counts['rejected-by-profile']}; blocked ${blocked}; ${unexpected} unexpected`);
  process.exitCode = unexpected === 0 ? 0 : 1;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch((err) => {
    console.error(err.stack ?? err);
    process.exitCode = 1;
  });
}

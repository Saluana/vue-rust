// Unit tests for the compiler core (WP-C1b): determinism, name mangling, literal pool, error paths, TS erasure, HIR
// shape, and a negative control for the golden comparison.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { readFileSync } from 'node:fs';
import { compileModule } from '../../../compiler/index.mjs';
import { checkSource } from '../../../compiler/frontend/check.mjs';
import { lowerToHir } from '../../../compiler/hir/index.mjs';
import { mangle, localName, rstr, f64Lit } from '../../../compiler/emit/names.mjs';
import { Emitter } from '../../../compiler/emit/buf.mjs';
import { compareDocs } from '../../../tooling/oracle/compare.mjs';
import { runScenario } from '../../../tooling/oracle/node-runner.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '../../..');

test('name mangling keeps ASCII identifiers and escapes other characters', () => {
  assert.equal(mangle('abc_1'), 'abc_1');
  assert.equal(mangle('é'), '_ue9_');
  assert.equal(localName(12, 'x'), 'v12_x');
  assert.equal(rstr('a"b\\'), '"a\\"b\\\\"');
  assert.equal(f64Lit('-0'), '-0.0_f64');
  assert.equal(f64Lit('1e+21'), '1e21_f64');
  assert.equal(f64Lit('5'), '5.0_f64');
});

test('literal pool numbers constants in first-use order and reuses equal strings', () => {
  const em = new Emitter({});
  assert.equal(em.strConst([104, 105]), 'S0');
  assert.equal(em.strConst([1]), 'S1');
  assert.equal(em.strConst([104, 105]), 'S0');
  assert.equal(em.strings.length, 2);
});

test('compilation is deterministic and independent of the working directory', () => {
  const src = 'let a = 1\nfunction f(x) { return x + a }\ntrace("v", f(2))\n';
  const first = compileModule(src, { filename: 'det.js', specifier: 'det', target: 'test' });
  const cwd = process.cwd();
  process.chdir(os.tmpdir());
  try {
    const second = compileModule(src, { filename: 'det.js', specifier: 'det', target: 'test' });
    assert.equal(second.rust, first.rust);
  } finally {
    process.chdir(cwd);
  }
  assert.notEqual(first.rust, null);
});

test('unsupported constructs stop with their code and a span; no Rust is produced', () => {
  // A computed import() specifier is outside the profile: VN2019, at the argument.
  const src = 'export function run(name) { return import(name) }\n';
  const r = compileModule(src, { filename: 'a.js', specifier: 'a' });
  assert.equal(r.rust, null);
  const d = r.diagnostics.find((x) => x.code === 'VN2019');
  assert.ok(d, 'expected VN2019');
  assert.equal(d.start.offset, src.indexOf('import(name)') + 'import('.length);
  assert.equal(r.diagnostics.some((x) => x.code === 'VN9001'), false);
});

test('a literal import() lowers to Rt::dynamic_import; its target is not a static request', () => {
  // WP-C1c.5.5 replaced the earlier VN9001 ("no lowering yet") for this form. The target joins the project graph
  // (compileProject); the module's requests are the static imports only, linked before evaluation.
  const src = "export function run() { return import('./x.js') }\n";
  const r = compileModule(src, { filename: 'a.js', specifier: 'a', target: 'test' });
  assert.equal(r.diagnostics.filter((d) => d.severity === 'error').length, 0);
  assert.notEqual(r.rust, null);
  assert.match(r.rust, /rt\.dynamic_import\("\.\/x\.js"\)/);
  assert.doesNotMatch(r.rust, /requests: &\[[^\]]*"\.\/x\.js"/);
});

test('profile rejections are reported with their registered codes', () => {
  const r = compileModule('class A {}\n', { filename: 'c.js', specifier: 'c' });
  assert.equal(r.rust, null);
  assert.ok(r.diagnostics.some((x) => x.code === 'VN2002'));
  const bad = compileModule('let = ;', { filename: 'bad.js', specifier: 'bad' });
  assert.equal(bad.rust, null);
  assert.ok(bad.diagnostics.some((x) => x.code === 'VN1000'));
});

test('TypeScript annotations are erased and compile like the JavaScript', () => {
  const ts = 'const k: number = 2 as number\nfunction g<T>(a: T): T { return a }\nexport type Alias = string\ntrace("ts", g<number>(k)!)\n';
  const r = compileModule(ts, { filename: 'erase.ts', lang: 'ts', specifier: 'erase', target: 'test' });
  assert.equal(r.diagnostics.filter((d) => d.severity === 'error').length, 0);
  assert.notEqual(r.rust, null);
});

test('HIR resolves identifiers and marks captured bindings as cells', () => {
  const src = 'let n = 0\nconst inc = () => { n += 1; return n }\ntrace("x", inc())\n';
  const { ast, analysis } = checkSource(src, { filename: 'h.js', lang: 'js' });
  const { hir, diagnostics } = lowerToHir(ast, analysis, { source: src, filename: 'h.js', specifier: 'h' });
  assert.equal(diagnostics.length, 0);
  assert.equal(hir.hirVersion, 3);
  assert.deepEqual(hir.requests, []);
  assert.equal(hir.slots[0].name, 'n');
  const inc = hir.fns.find((f) => f && f.kind === 'arrow');
  assert.deepEqual(inc.caps.length, 1, 'the arrow captures n');
  assert.equal(inc.body[0].k, 'expr');
});

test('negative control: a doctored golden is reported as a difference', async () => {
  const file = path.join(repo, 'tests/language/lowering/smoke-tiny.js');
  const actual = await runScenario(file);
  const golden = JSON.parse(readFileSync(path.join(repo, 'tests/language/lowering-golden/smoke-tiny.js.json'), 'utf8'));
  assert.equal(compareDocs(actual, golden), null, 'the real golden matches the oracle');
  const wrong = JSON.parse(JSON.stringify(golden));
  wrong.entries[0].values[0] = 999;
  const d = compareDocs(actual, wrong);
  assert.ok(d !== null && d.path === '$.entries[0].values[0]', 'the doctored value must be reported');
});

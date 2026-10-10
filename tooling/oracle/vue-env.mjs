// Oracle environment for Vue scenarios (WP-V0): how tests/vue/cases run in Node, how their goldens are checked, and
// the Vue-specific part of the lint. Reuses runScenarioLine (node-runner.mjs), listCases, goldenPathFor and
// compareWithGolden (check-goldens.mjs), and lintSource (lint-cases.mjs). Only the directory walk, the concurrency
// limit and one file-read helper are written here, because check-goldens.mjs does not export them.
//
// A Vue scenario follows docs/dev/trace-format.md and imports from 'vue' only. It runs in a fresh child process
// (node-runner.mjs) with two differences from language cases:
//  1. Production build. The child gets NODE_ENV=production; node_modules/vue/index.js then requires
//     dist/vue.cjs.prod.js. Dev-only warnings and checks are not part of the contract. Node's ESM interop also
//     evaluates the reexported development modules while it reads vue's export names; the bindings a scenario
//     receives are the production ones (tests/vue/oracle.test.mjs checks their identity).
//  2. Names. node-runner names files outside tests/language/cases by basename. runVueScenarioLine rewrites the
//     "scenario" field to the path under tests/vue/cases (e.g. "ref/get-set"). The child's canonical text is checked
//     first, so nothing else in the document changes.
//
// CLI (from the repository root):
//   node tooling/oracle/vue-env.mjs            check every golden under tests/vue/golden (exit 1 on mismatch or orphan)
//   node tooling/oracle/vue-env.mjs --update   rewrite the goldens with the exact child output; remove orphans
//   node tooling/oracle/vue-env.mjs --lint     print the Vue lint problems of tests/vue/cases
import {readFileSync} from 'node:fs';
import {mkdir, readdir, readFile, rm, writeFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {parse} from '@babel/parser';
import {compareWithGolden, goldenPathFor, listCases} from './check-goldens.mjs';
import {runScenarioLine} from './node-runner.mjs';
import {lintSource} from './lint-cases.mjs';
import {isStdlibSupported, listSupported} from '../../compiler/profile/stdlib.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '../..');
export const vueCasesRoot = path.join(repo, 'tests/vue/cases');
export const vueGoldenRoot = path.join(repo, 'tests/vue/golden');
// Required default (WP-V0): the production build of vue, never the development build.
export const VUE_NODE_ENV = 'production';

const VUE_ROW = /^V(0[1-9]|1[0-7])$/;
const TIER = /^\/\/ @tier [123]$/;
const COVERS = /^\/\/ @covers V(?:0[1-9]|1[0-7])(?:,V(?:0[1-9]|1[0-7]))*$/;
// Member names that belong to Vue's or the runtime's internal state. Scenarios must not read them.
const INTERNAL_MEMBER = /^(__v_\w*|dep|deps|depsTail|flags|globalVersion|effects|cleanups|scopes|_rawValue|_value|_active|_on|_object|_key|_getter|_defaultValue)$/;
const AST_SKIP = new Set(['loc', 'start', 'end', 'extra', 'comments', 'leadingComments', 'trailingComments', 'innerComments', 'tokens', 'errors']);
const inventoryFile = path.join(repo, 'compiler/profile/inventory.generated.json');
const exportMapFile = path.join(repo, 'compiler/profile/export-map.json');

// Method names the stdlib manifest supports on builtin prototypes for the test target. This is the set that lint-cases.mjs
// checks method calls against (its SUPPORTED_METHODS, derived the same way; that constant is not exported).
const LINT_TARGET = 'test';
const METHOD_NAME = /\.([A-Za-z_$][\w$]*)$/;
const STDLIB_METHOD_NAMES = new Set(
  listSupported('proto')
    .filter((e) => isStdlibSupported(e.path, LINT_TARGET))
    .map((e) => METHOD_NAME.exec(e.path))
    .filter((m) => m !== null)
    .map((m) => m[1]),
);
// Instance and handle methods of Vue objects that scenarios call. Vue's public typings declare each one: EffectScope
// (run, stop, pause, resume) and WatchHandle (pause, resume, stop). on/off exist on EffectScope at runtime only, so they
// are not listed. The language lint matches method calls by name, so these names are exempted, but only when the
// stdlib manifest does not govern the name.
export const VUE_MEMBER_CALLS = new Set(['run', 'stop', 'pause', 'resume']);
export function isVueMemberCall(name) {
  return VUE_MEMBER_CALLS.has(name) && !STDLIB_METHOD_NAMES.has(name);
}
// The message lint-cases.mjs gives a method call it cannot match to a builtin (the name is the first capture). If that
// message changes, this stops matching and the case lint fails again: the wrapper fails closed.
const UNMATCHED_METHOD = /^\.([A-Za-z_$][\w$]*)\(\) is not a supported builtin method/;

// The names a scenario may import from 'vue': runtime exports that the export map classifies to a V01..V17 row
// with status P. effect, stop (V12), proxyRefs (V07) and getCurrentWatcher (V16) are P and allowed. Exports classified X
// (ReactiveEffect, queuePostFlushCb, TrackOpTypes, TriggerOpTypes), exports outside V01..V17 (h, mergeProps,
// getCurrentInstance) and every component or DOM API are excluded.
export function vueImportAllowlist() {
  const map = JSON.parse(readFileSync(exportMapFile, 'utf8'));
  const primary = new Map(JSON.parse(readFileSync(inventoryFile, 'utf8')).rows.map((r) => [r.id, r.primary]));
  const allowed = new Set();
  for (const [name, v] of Object.entries(map.exports)) {
    const row = typeof v === 'string' ? v : v.row;
    const status = typeof v === 'string' ? primary.get(row) : (v.status ?? primary.get(row));
    if (VUE_ROW.test(row) && status === 'P') allowed.add(name);
  }
  return allowed;
}

// Scenario name: path under tests/vue/cases without ".js". Throws for a file outside that root.
export function vueScenarioName(file) {
  const abs = path.resolve(file);
  const rel = path.relative(vueCasesRoot, abs);
  if (rel === '' || rel.startsWith('..') || path.isAbsolute(rel)) throw new Error(`${abs} is not under ${vueCasesRoot}`);
  return rel.split(path.sep).join('/').replace(/\.js$/, '');
}

// Header: exactly "// @tier N" then "// @covers V01,V06" as the first two lines (IDs V01..V17).
export function checkVueHeader(source) {
  const lines = source.split('\n');
  const problems = [];
  if (!TIER.test((lines[0] ?? '').trimEnd())) {
    problems.push({code: 'HEADER', line: 1, message: 'first line must be "// @tier N" with N = 1, 2 or 3'});
  }
  if (!COVERS.test((lines[1] ?? '').trimEnd())) {
    problems.push({code: 'HEADER', line: 2, message: 'second line must be "// @covers V01,V06" with IDs V01..V17'});
  }
  return problems;
}

function* astChildren(node) {
  for (const key of Object.keys(node)) {
    if (AST_SKIP.has(key)) continue;
    const v = node[key];
    if (Array.isArray(v)) {
      for (const c of v) if (c && typeof c.type === 'string') yield c;
    } else if (v && typeof v.type === 'string') {
      yield v;
    }
  }
}

// Returns [{code, line, message}] for one source text; empty when the scenario is clean.
// = Vue header tags + language lint (lint-cases.mjs, without its language-only header check) + Vue-only rules:
// named imports from 'vue' limited to vueImportAllowlist(), no reads of Vue internal members, and method calls that
// are Vue instance or handle methods (VUE_MEMBER_CALLS, outside the stdlib manifest). Every other report is kept.
export function lintVueSource(source, allowed = vueImportAllowlist()) {
  const problems = checkVueHeader(source);
  for (const p of lintSource(source)) {
    if (p.code === 'HEADER') continue;
    // The language lint matches method calls by name and cannot tell a Vue handle method from an unknown builtin.
    const m = p.code === 'UNLISTED_API' ? UNMATCHED_METHOD.exec(p.message) : null;
    if (m !== null && isVueMemberCall(m[1])) continue;
    problems.push(p);
  }
  let ast;
  try {
    ast = parse(source, {sourceType: 'module'});
  } catch {
    return problems; // lintSource already reported the PARSE problem
  }
  const stack = [ast.program];
  while (stack.length > 0) {
    const node = stack.pop();
    const line = node.loc ? node.loc.start.line : 0;
    if (node.type === 'ImportDeclaration' && node.source.value === 'vue') {
      for (const s of node.specifiers) {
        const name = s.type === 'ImportSpecifier' ? (s.imported.type === 'Identifier' ? s.imported.name : s.imported.value) : null;
        if (name === null || !allowed.has(name)) {
          problems.push({code: 'VUE_API', line, message: `"${name ?? s.local.name}" is not an allowed vue import (P-status V01..V17 export)`});
        }
      }
    }
    if ((node.type === 'MemberExpression' || node.type === 'OptionalMemberExpression') && !node.computed &&
        node.property.type === 'Identifier' && INTERNAL_MEMBER.test(node.property.name)) {
      problems.push({code: 'VUE_INTERNAL', line, message: `\`.${node.property.name}\` reads Vue internal state`});
    }
    if (node.type === 'StringLiteral' && node.value.includes('__v_')) {
      problems.push({code: 'VUE_INTERNAL', line, message: 'string names a Vue reactive flag'});
    }
    for (const c of astChildren(node)) stack.push(c);
  }
  return problems;
}

export async function lintVueFile(file) {
  return lintVueSource(await readFile(file, 'utf8'));
}

// One scenario in a fresh child with NODE_ENV=production. Returns the canonical line (newline-terminated).
export async function runVueScenarioLine(file) {
  const abs = path.resolve(file);
  const name = vueScenarioName(abs);
  // node-runner copies process.env synchronously when it spawns, so the override reaches that child only.
  // The parent environment is restored before anything is awaited.
  const had = Object.prototype.hasOwnProperty.call(process.env, 'NODE_ENV');
  const saved = process.env.NODE_ENV;
  process.env.NODE_ENV = VUE_NODE_ENV;
  let pending;
  try {
    pending = runScenarioLine(abs);
  } finally {
    if (had) process.env.NODE_ENV = saved;
    else delete process.env.NODE_ENV;
  }
  const line = await pending;
  const doc = JSON.parse(line);
  if (`${JSON.stringify(doc)}\n` !== line) throw new Error(`${abs}: child output is not canonical JSON`);
  doc.scenario = name;
  return `${JSON.stringify(doc)}\n`;
}

async function readOrNull(file) {
  try {
    return await readFile(file, 'utf8');
  } catch (e) {
    if (e.code === 'ENOENT') return null;
    throw e;
  }
}

async function listGoldens(root) {
  const out = [];
  async function walkDir(dir) {
    const entries = await readdir(dir, {withFileTypes: true});
    entries.sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
    for (const ent of entries) {
      const p = path.join(dir, ent.name);
      if (ent.isDirectory()) await walkDir(p);
      else if (ent.isFile() && ent.name.endsWith('.json')) out.push(p);
    }
  }
  try {
    await walkDir(root);
  } catch (e) {
    if (e.code !== 'ENOENT') throw e;
  }
  return out;
}

async function mapLimit(items, limit, fn) {
  const results = new Array(items.length);
  let next = 0;
  async function worker() {
    while (next < items.length) {
      const i = next++;
      results[i] = await fn(items[i]);
    }
  }
  await Promise.all(Array.from({length: Math.min(limit, items.length)}, worker));
  return results;
}

// Returns {results: [{file, status: 'ok'|'updated'|'fail', reason?}], orphans: [golden paths without a case]}.
// Update mode rewrites every golden and removes orphans (listed in `orphans`). goldenRoot defaults to the corpus
// goldens; the oracle test points it at a doctored copy for its negative control.
export async function checkVueGoldens({update = false, goldenRoot = vueGoldenRoot} = {}) {
  const cases = await listCases(vueCasesRoot);
  const results = await mapLimit(cases, 4, async (file) => {
    const gfile = goldenPathFor(file, goldenRoot, vueCasesRoot);
    let line;
    try {
      line = await runVueScenarioLine(file);
    } catch (e) {
      return {file, status: 'fail', reason: `scenario failed to run: ${e.message.split('\n')[0]}`};
    }
    if (update) {
      await mkdir(path.dirname(gfile), {recursive: true});
      await writeFile(gfile, line);
      return {file, status: 'updated'};
    }
    const reason = compareWithGolden(line, await readOrNull(gfile));
    return reason === null ? {file, status: 'ok'} : {file, status: 'fail', reason};
  });
  const expected = new Set(cases.map((c) => goldenPathFor(c, goldenRoot, vueCasesRoot)));
  const orphans = (await listGoldens(goldenRoot)).filter((g) => !expected.has(g));
  if (update) for (const g of orphans) await rm(g);
  return {results, orphans};
}

async function main(argv) {
  const rel = (f) => path.relative(repo, f).split(path.sep).join('/');
  if (argv.includes('--lint')) {
    const allowed = vueImportAllowlist();
    const files = await listCases(vueCasesRoot);
    let count = 0;
    for (const file of files) {
      for (const p of lintVueSource(await readFile(file, 'utf8'), allowed)) {
        count += 1;
        process.stdout.write(`${rel(file)}:${p.line} [${p.code}] ${p.message}\n`);
      }
    }
    process.stdout.write(`${files.length} case(s), ${count} problem(s)\n`);
    process.exitCode = count === 0 ? 0 : 1;
    return;
  }
  const update = argv.includes('--update');
  const {results, orphans} = await checkVueGoldens({update});
  let failures = 0;
  for (const r of results) {
    if (r.status !== 'fail') continue;
    failures += 1;
    process.stdout.write(`FAIL ${rel(r.file)}: ${r.reason}\n`);
  }
  for (const g of orphans) {
    if (!update) failures += 1;
    process.stdout.write(`${update ? 'removed' : 'ORPHAN'} golden ${rel(g)}\n`);
  }
  const good = results.filter((r) => r.status === 'ok' || r.status === 'updated').length;
  process.stdout.write(update
    ? `updated ${good} golden(s) of ${results.length} scenario(s)\n`
    : `${good}/${results.length} scenario(s) match their goldens\n`);
  process.exitCode = failures === 0 ? 0 : 1;
}

const isMain = process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) await main(process.argv.slice(2));

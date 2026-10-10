#!/usr/bin/env node
// Upstream unit tests as an oracle (WP-U1). Vue 3.5.43's own reactivity specs (packages/reactivity/__tests__, read from a
// checkout, never copied into this repo) are compiled through this repo's compiler and run natively against the Rust `vue`
// module, with the vitest shim tooling/harness/upstream-vitest.js. The specs' own expect() calls are the oracle.
//   node tooling/harness/run-upstream.mjs [--vue-core <dir>] [--filter text] [--status <ledger.json>] [--write-status] [--verbose]
// The checkout is --vue-core, else $VUE_CORE_DIR, else ../vue-core next to this checkout (git clone --depth 1 --branch v3.5.43
// https://github.com/vuejs/core). Outcomes per test: pass | fail | compile-error | unsupported. Exit 1 when a test the ledger
// marks pass is not pass any more, or when the harness cannot build.
//
// How a spec becomes a scenario. Imports from the Vue packages are rewritten to the native "vue" module, keeping the names
// the module exports (a probe build reads them). A test that reads a name the module does not export, a Vue internal field,
// an Array method the language profile excludes, or a dev-only debugger hook is refused as 'unsupported' (its reason says
// which). A test the compiler rejects is refused as 'compile-error': its callback is replaced by
// __upstreamRefuse(status, reason), so only that test reports it. A describe whose own code the compiler rejects is
// replaced by a describe of refused tests, one per test in it. A file rejected outside any describe is one entry for the
// file. The rewritten module gets the vitest globals and an exported run() that calls the shim's runner.
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { parseModule } from '../../compiler/frontend/parse.mjs';
import { checkSource } from '../../compiler/frontend/check.mjs';
import { lowerToHir } from '../../compiler/hir/index.mjs';
import { emitModule } from '../../compiler/emit/index.mjs';
import { writeCrate, buildCrate, runOne, ROOT, TARGET_DIR } from './build.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
export const DEFAULT_VUE_CORE = fileURLToPath(new URL('../../../vue-core', import.meta.url));
export const SPEC_DIR = 'packages/reactivity/__tests__';
export const SHIM_FILE = path.join(here, 'upstream-vitest.js');
const CRATE_DIR = path.join(ROOT, 'target', 'scenarios', 'upstream');
const PROBE_DIR = path.join(ROOT, 'target', 'scenarios', 'upstream-probe');
const BINARY = path.join(TARGET_DIR, 'debug', 'vn-upstream-scenarios');
const PROBE_BINARY = path.join(TARGET_DIR, 'debug', 'vn-upstream-probe');
export const DEFAULT_LEDGER = path.join(ROOT, 'tests', 'upstream', 'status.json');
const SCENARIO_PREFIX = 'upstream/';
const SHIM_NAME = 'vitest';
export const KEY_SEP = ' :: ';

// Injected at the top and the bottom of every rewritten spec: the vitest globals (the specs use them without importing, as
// Vue's vitest config sets globals: true), the refusal helper and the runner.
const HEADER = "import { describe, it, test, expect, vi, beforeEach, __upstreamRefuse, __upstreamRunAll } from 'vitest';\n";
const FOOTER = '\nexport function run() {\n  return __upstreamRunAll();\n}\n';

// Import sources that name Vue's packages. The specs' own '../src' (and '@vue/reactivity') is the reactivity package; the
// runtime packages (vue, @vue/runtime-core, @vue/runtime-dom) are the public API the Rust module mirrors. The two are
// different APIs for the names they share: the reactivity package's watch is synchronous by default and is not public in
// the Rust module, whose watch is the runtime-core one (pre-flush). So a reactivity import of watch is refused.
const REACTIVITY_SOURCE_RE = /^(\.\.\/)+src(\/[A-Za-z0-9_.-]+)*$/;
const REACTIVITY_PACKAGES = new Set(['@vue/reactivity', '@vue/shared']);
const RUNTIME_PACKAGES = new Set(['vue', '@vue/runtime-core', '@vue/runtime-dom']);
const REACTIVITY_LEVEL_ONLY = new Set(['watch']);

// Vue internals the Rust port does not expose: a test that reads one is not a test of the public API.
const INTERNAL_FIELDS = new Set(['effects', 'cleanups', 'scopes', 'dep', 'deps', 'nextDep', 'subs', 'flags', 'effect', '_value', '_rawValue']);
// Debugger hooks: Vue calls them only under __DEV__ (dep.ts), so the production build never runs them.
const DEBUGGER_HOOKS = new Set(['onTrack', 'onTrigger']);

// Array methods the language profile excludes (status X in compiler/profile/stdlib.json, the file the compiler reads too).
// A test that calls one is outside the port's surface, so it is not a failure of the port. Read from the profile so the
// two cannot drift apart.
function excludedArrayMethods() {
  const doc = JSON.parse(readFileSync(path.join(ROOT, 'compiler', 'profile', 'stdlib.json'), 'utf8'));
  const out = new Map();
  for (const e of doc.entries) {
    if (e.kind === 'proto' && e.status === 'X' && /^Array\.prototype\.[A-Za-z]+$/.test(e.path)) out.set(e.path.slice('Array.prototype.'.length), e.path);
  }
  return out;
}
const EXCLUDED_ARRAY_METHODS = excludedArrayMethods();

// Keys of an AST node that are not code: comments, locations, and TypeScript type annotations.
const SKIP_KEYS = new Set([
  'leadingComments', 'trailingComments', 'innerComments', 'comments', 'tokens', 'errors', 'loc', 'extra',
  'typeAnnotation', 'typeParameters', 'returnType', 'superTypeParameters', 'typeArguments', 'implements',
]);
const TYPE_ONLY_NODES = new Set(['TSInterfaceDeclaration', 'TSTypeAliasDeclaration', 'TSDeclareFunction', 'TSModuleDeclaration']);
const FUNCTION_NODES = new Set(['ArrowFunctionExpression', 'FunctionExpression']);
const CHAIN_NODES = new Set(['MemberExpression', 'OptionalMemberExpression', 'CallExpression', 'OptionalCallExpression']);

const isNode = (v) => v !== null && typeof v === 'object' && typeof v.type === 'string';
const isError = (d) => d.severity === 'error';
const firstMessage = (d) => `${d.code} ${d.message}`;

function children(node) {
  const out = [];
  for (const key of Object.keys(node)) {
    if (SKIP_KEYS.has(key)) continue;
    const v = node[key];
    if (Array.isArray(v)) {
      for (const c of v) if (isNode(c)) out.push(c);
    } else if (isNode(v)) {
      out.push(v);
    }
  }
  return out;
}

// 'test' for a call it/test(name, fn) (the .skip, .only, .skipIf and .each forms too), 'group' for describe(name, fn) the
// same way, else null. The callback is the second argument and must be a function.
export function testKind(node) {
  if (node.type !== 'CallExpression' || node.arguments.length < 2 || !FUNCTION_NODES.has(node.arguments[1].type)) return null;
  let root = node.callee;
  while (CHAIN_NODES.has(root.type)) root = root.type.endsWith('MemberExpression') ? root.object : root.callee;
  if (root.type !== 'Identifier') return null;
  if (root.name === 'it' || root.name === 'test') return 'test';
  if (root.name === 'describe') return 'group';
  return null;
}

// Every test and describe call of a module, in source order.
export function findCalls(ast) {
  const program = ast.type === 'File' ? ast.program : ast;
  const out = [];
  const visit = (node) => {
    if (node.type === 'CallExpression') {
      const kind = testKind(node);
      if (kind !== null) out.push({ kind, node, cb: node.arguments[1] });
    }
    for (const c of children(node)) visit(c);
  };
  visit(program);
  return out;
}

// Names a piece of code reads as values. Member properties and non-computed keys are not reads; TypeScript types are not
// code; the callbacks of the calls whose kind is in stopKinds are not read here (their own scope reports them).
export function collectRefs(node, out, stopKinds) {
  if (!isNode(node) || node.type === 'ImportDeclaration' || TYPE_ONLY_NODES.has(node.type)) return out;
  if (node.type === 'Identifier') {
    out.add(node.name);
    return out;
  }
  if (node.type === 'CallExpression' && stopKinds.has(testKind(node))) {
    collectRefs(node.callee, out, stopKinds);
    collectRefs(node.arguments[0], out, stopKinds);
    return out;
  }
  if (node.type === 'MemberExpression' || node.type === 'OptionalMemberExpression') {
    collectRefs(node.object, out, stopKinds);
    if (node.computed) collectRefs(node.property, out, stopKinds);
    return out;
  }
  if (node.type === 'ObjectProperty') {
    if (node.computed) collectRefs(node.key, out, stopKinds);
    collectRefs(node.value, out, stopKinds);
    return out;
  }
  if (node.type === 'ObjectMethod') {
    if (node.computed) collectRefs(node.key, out, stopKinds);
    for (const c of [...node.params, node.body]) collectRefs(c, out, stopKinds);
    return out;
  }
  if (node.type === 'LabeledStatement') return collectRefs(node.body, out, stopKinds);
  if (node.type === 'BreakStatement' || node.type === 'ContinueStatement') return out;
  for (const c of children(node)) collectRefs(c, out, stopKinds);
  return out;
}

// The import declarations as edits. A Vue package keeps the names the native module exports (rewritten to 'vue'); any
// other name is missing (name -> source). Type-only imports and specifiers are erased by the compiler and dropped here.
export function importPlan(program, vueExports) {
  const edits = [];
  const missing = new Map();
  for (const s of program.body) {
    if (s.type !== 'ImportDeclaration') continue;
    const source = s.source.value;
    const remove = { start: s.start, end: s.end, text: '' };
    if (source === SHIM_NAME || s.importKind === 'type' || s.importKind === 'typeof') {
      edits.push(remove);
      continue;
    }
    const reactivity = REACTIVITY_SOURCE_RE.test(source) || REACTIVITY_PACKAGES.has(source);
    const mapped = reactivity || RUNTIME_PACKAGES.has(source);
    const kept = [];
    for (const spec of s.specifiers) {
      if (spec.importKind === 'type' || spec.importKind === 'typeof') continue;
      const local = spec.local.name;
      if (spec.type !== 'ImportSpecifier') {
        missing.set(local, source);
        continue;
      }
      const imported = spec.imported.type === 'Identifier' ? spec.imported.name : spec.imported.value;
      const same = mapped && vueExports.has(imported) && !(reactivity && REACTIVITY_LEVEL_ONLY.has(imported));
      if (same) kept.push(imported === local ? imported : `${imported} as ${local}`);
      else missing.set(local, source);
    }
    edits.push(mapped && kept.length > 0 ? { start: s.start, end: s.end, text: `import { ${kept.join(', ')} } from 'vue';` } : remove);
  }
  return { edits, missing };
}

// The property names a piece of code reads (obj.name, obj['name']) and the keys of the objects it builds ({ name: ... }).
// Calls of the kinds in stopKinds are skipped, as in collectRefs.
function propertyNames(node, stopKinds, out = { reads: new Set(), keys: new Set() }) {
  if (!isNode(node) || node.type === 'ImportDeclaration' || TYPE_ONLY_NODES.has(node.type)) return out;
  if (node.type === 'CallExpression' && stopKinds.has(testKind(node))) return out;
  if (node.type === 'MemberExpression' || node.type === 'OptionalMemberExpression') {
    const name = keyName(node.property, node.computed);
    if (name !== null) out.reads.add(name);
  } else if ((node.type === 'ObjectProperty' || node.type === 'ObjectMethod') && !node.computed) {
    const name = keyName(node.key, false);
    if (name !== null) out.keys.add(name);
  }
  for (const c of children(node)) propertyNames(c, stopKinds, out);
  return out;
}

// The name a property key or member property spells: an identifier or a string literal. Null for any other computed key.
function keyName(key, computed) {
  if (key.type === 'StringLiteral') return key.value;
  if (!computed && key.type === 'Identifier') return key.name;
  return null;
}

// Why the Rust port cannot run a piece of code (its own statements, not the tests in it), or null when it can: a name the
// module does not export (unsupported-import), a read of a Vue internal field (unsupported-internal), an Array method the
// language profile excludes (unsupported-profile), or a debugger hook, which Vue calls only in development builds
// (unsupported-dev-only).
function gapReason(node, stopKinds, missing) {
  const refs = collectRefs(node, new Set(), stopKinds);
  const absent = [...refs].filter((n) => missing.has(n)).sort();
  if (absent.length > 0) {
    return `unsupported-import: ${absent.map((n) => `${n} from '${missing.get(n)}'`).join(', ')} is not exported by the Rust vue module`;
  }
  const props = propertyNames(node, stopKinds);
  const internal = [...props.reads].filter((n) => INTERNAL_FIELDS.has(n)).sort();
  if (internal.length > 0) {
    return `unsupported-internal: reads Vue internal field ${internal.map((n) => `.${n}`).join(', ')}, which the Rust port does not expose`;
  }
  const excluded = [...props.reads].filter((n) => EXCLUDED_ARRAY_METHODS.has(n)).sort();
  if (excluded.length > 0) {
    return `unsupported-profile: calls ${excluded.map((n) => `.${n}`).join(', ')}, which the language profile excludes (${excluded.map((n) => EXCLUDED_ARRAY_METHODS.get(n)).join(', ')}, status X)`;
  }
  const hooks = [...new Set([...refs, ...props.reads, ...props.keys])].filter((n) => DEBUGGER_HOOKS.has(n)).sort();
  if (hooks.length > 0) {
    return `unsupported-dev-only: uses the debugger hook ${hooks.join(', ')}, which Vue calls only in development builds`;
  }
  return null;
}

// The static marks, by the first reason that applies: a test's own code; else the top level's code; else its describe's
// own code (the code outside its tests). Returns one mark or null per call.
export function markUnsupported(program, calls, missing) {
  const nested = new Set(['test', 'group']);
  const marks = calls.map(() => null);
  const mark = (i, reason) => {
    if (marks[i] === null) marks[i] = { status: 'unsupported', reason };
  };
  const topReason = gapReason(program, nested, missing);
  calls.forEach((call, i) => {
    if (call.kind !== 'test') return;
    const own = gapReason(call.cb, new Set(), missing);
    if (own !== null) mark(i, own);
    else if (topReason !== null) mark(i, topReason);
  });
  calls.forEach((group) => {
    if (group.kind !== 'group') return;
    const reason = gapReason(group.cb, nested, missing);
    if (reason === null) return;
    calls.forEach((call, i) => {
      if (call.kind === 'test' && call.cb.start >= group.cb.start && call.cb.end <= group.cb.end) mark(i, reason);
    });
  });
  return marks;
}

// Applies non-overlapping edits {start, end, text} to a text.
export function applyEdits(text, edits) {
  const sorted = [...edits].sort((a, b) => b.start - a.start);
  let out = text;
  let limit = Infinity;
  for (const e of sorted) {
    if (e.end > limit) throw new RangeError('applyEdits: overlapping edits');
    out = out.slice(0, e.start) + e.text + out.slice(e.end);
    limit = e.start;
  }
  return out;
}

// The text of a test's refusal: its callback becomes a function that throws __upstreamRefuse(status, reason).
export function refusalText(status, reason) {
  return `() => __upstreamRefuse(${JSON.stringify(status)}, ${JSON.stringify(reason)})`;
}

// Compiles one module text the way compileModule (compiler/index.mjs) does: the same stages, with the standard-library
// manifest check on (stdlib: true), for the test target the scenario binaries are built for. Returns { rust } on success,
// or { ast, diagnostics } with the error diagnostics (their spans are offsets into this text; ast is null when the text
// does not parse).
export function compileText(text, { filename, specifier, lang }) {
  const checked = checkSource(text, { filename, lang, target: 'test', stdlib: true });
  const errors = checked.diagnostics.filter(isError);
  if (!checked.ast || errors.length > 0) return { ast: checked.ast, diagnostics: errors };
  const lowered = lowerToHir(checked.ast, checked.analysis, { source: text, filename, specifier, lang });
  if (lowered.hir === null) return { ast: checked.ast, diagnostics: lowered.diagnostics.filter(isError) };
  return { ast: checked.ast, rust: emitModule(lowered.hir) };
}

// The source text of a test or describe name: its value when it is a plain string, else its source.
function nameText(call, text) {
  const a = call.arguments[0];
  if (a.type === 'StringLiteral') return a.value;
  if (a.type === 'TemplateLiteral' && a.expressions.length === 0) return a.quasis[0].value.cooked;
  return text.slice(a.start, a.end);
}

// The refused form of a piece of code: each test in it becomes a refused test (same name, same nesting of describes), and
// the rest of it is dropped. The text replaces the code.
function refusedForm(node, text, reason) {
  const why = JSON.stringify(reason);
  if (node.type === 'CallExpression') {
    const kind = testKind(node);
    if (kind === 'test') return `test(${JSON.stringify(nameText(node, text))}, () => __upstreamRefuse("compile-error", ${why}));\n`;
    if (kind === 'group') {
      const body = node.arguments[1].body;
      const inner = body.type === 'BlockStatement' ? body.body.map((s) => refusedForm(s, text, reason)).join('') : refusedForm(body, text, reason);
      return `describe(${JSON.stringify(nameText(node, text))}, () => {\n${inner}});\n`;
    }
  }
  return children(node).map((c) => refusedForm(c, text, reason)).join('');
}

// The names a statement declares (variables, functions, classes).
function declaredNames(stmt) {
  if (stmt.type === 'VariableDeclaration') return stmt.declarations.flatMap((d) => [...collectRefs(d.id, new Set(), new Set())]);
  if ((stmt.type === 'FunctionDeclaration' || stmt.type === 'ClassDeclaration') && stmt.id) return [stmt.id.name];
  return [];
}

// The edits that take a describe the compiler rejects out of the module: the statement of its body that holds the
// rejected code, and each other statement of the body that reads a name it declares (they cannot run without it).
// Without such a statement the whole describe call is refused. Offsets are body offsets (shift = HEADER length).
function excisionEdits(group, at, text, reason, shift) {
  const body = group.cb.body.type === 'BlockStatement' ? group.cb.body.body : null;
  const index = body === null ? -1 : body.findIndex((s) => s.start <= at && at < s.end);
  if (index === -1) {
    return [{ start: group.node.start - shift, end: group.node.end - shift, text: refusedForm(group.node, text, reason) }];
  }
  const picked = new Set([index]);
  let names = declaredNames(body[index]);
  for (let grew = true; grew;) {
    grew = false;
    body.forEach((s, j) => {
      if (picked.has(j)) return;
      const refs = collectRefs(s, new Set(), new Set());
      if (names.some((n) => refs.has(n))) {
        picked.add(j);
        names = names.concat(declaredNames(s));
        grew = true;
      }
    });
  }
  return [...picked].sort((a, b) => a - b).map((j) => ({
    start: body[j].start - shift,
    end: body[j].end - shift,
    text: refusedForm(body[j], text, reason),
  }));
}

// Rewrites one spec: its imports, and a refusal for each test that reads a missing name. Returns { body } (the text the
// isolation works on: no header or footer) or { fileError }.
export function rewriteSpec(source, { filename, vueExports }) {
  const parsed = parseModule(source, { filename, lang: 'ts' });
  if (!parsed.ast) return { fileError: { status: 'compile-error', reason: firstMessage(parsed.diagnostics[0]) } };
  const program = parsed.ast.program;
  const calls = findCalls(parsed.ast);
  const { edits, missing } = importPlan(program, vueExports);
  const marks = markUnsupported(program, calls, missing);
  const refusals = calls.flatMap((call, i) => (marks[i] === null ? [] : [{ start: call.cb.start, end: call.cb.end, text: refusalText(marks[i].status, marks[i].reason) }]));
  return { body: applyEdits(source, [...edits, ...refusals]) };
}

// One pass over a body: refuses the tests the compiler rejects (keeping the calls in order, so the indices hold across the
// rounds of this pass), until the module compiles. Code the compiler rejects outside any test is returned as groupEdits:
// the caller applies them and runs again (the test calls change then). Returns { rust, text } | { groupEdits } | { fileError }.
function isolate(body, { filename, specifier }) {
  const shift = HEADER.length;
  let current = body;
  for (let round = 0; round <= body.length; round++) {
    const wrapped = HEADER + current + FOOTER;
    const compiled = compileText(wrapped, { filename, specifier, lang: 'ts' });
    if (compiled.rust !== undefined) return { rust: compiled.rust, text: wrapped };
    if (compiled.ast === null) return { fileError: { status: 'compile-error', reason: firstMessage(compiled.diagnostics[0]) } };
    const calls = findCalls(compiled.ast);
    const groups = calls.filter((c) => c.kind === 'group');
    const refuse = new Map();
    for (const d of compiled.diagnostics) {
      const at = d.start.offset;
      const t = calls.findIndex((c) => c.kind === 'test' && c.cb.start <= at && at < c.cb.end);
      if (t !== -1) {
        if (!refuse.has(t)) refuse.set(t, firstMessage(d));
        continue;
      }
      const inside = groups.filter((g) => g.node.start <= at && at < g.node.end).sort((a, b) => (a.node.end - a.node.start) - (b.node.end - b.node.start));
      if (inside.length > 0) return { groupEdits: excisionEdits(inside[0], at, wrapped, firstMessage(d), shift) };
      return { fileError: { status: 'compile-error', reason: firstMessage(d) } };
    }
    if (refuse.size === 0) return { fileError: { status: 'compile-error', reason: 'the module does not compile and no test accounts for it' } };
    current = applyEdits(current, [...refuse].map(([i, reason]) => ({
      start: calls[i].cb.start - shift,
      end: calls[i].cb.end - shift,
      text: refusalText('compile-error', reason),
    })));
  }
  return { fileError: { status: 'compile-error', reason: 'could not isolate the failing tests' } };
}

// A spec as a scenario module: { rust } or { fileError: { status, reason } }.
export function prepareSpec(source, { filename, specifier, vueExports }) {
  const rewritten = rewriteSpec(source, { filename, vueExports });
  if (rewritten.fileError) return rewritten;
  let body = rewritten.body;
  for (let pass = 0; pass <= 1000; pass++) {
    const step = isolate(body, { filename, specifier });
    if (step.groupEdits === undefined) return step.fileError ? step : { rust: step.rust, text: step.text };
    body = applyEdits(body, step.groupEdits);
  }
  return { fileError: { status: 'compile-error', reason: 'could not isolate the failing describes' } };
}

// The checkout directory, or null when it does not hold the reactivity specs.
export function resolveVueCore(explicit, env = process.env) {
  const dir = path.resolve(explicit || env.VUE_CORE_DIR || DEFAULT_VUE_CORE);
  return existsSync(path.join(dir, SPEC_DIR)) ? dir : null;
}

// Every spec file under the reactivity tests: { file, name } with name = path under SPEC_DIR without ".spec.ts".
export function listSpecs(vueCore) {
  const root = path.join(vueCore, SPEC_DIR);
  const out = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.name.endsWith('.spec.ts')) {
        const rel = path.relative(root, full).split(path.sep).join('/');
        out.push({ file: full, name: rel.replace(/\.spec\.ts$/, '') });
      }
    }
  };
  walk(root);
  return out;
}

// The per-test results of one scenario run, from its trace entries: { fileStatus, results: Map(id -> {status, reason}) }.
export function interpretRun(doc, runError) {
  const results = new Map();
  if (runError) return { fileStatus: { status: 'fail', reason: runError }, results };
  const planned = [];
  const started = [];
  for (const e of doc.entries ?? []) {
    if (e.label === 'upstream-plan') planned.push(e.values[0]);
    else if (e.label === 'upstream-start') started.push(e.values[0]);
    else if (e.label === 'upstream-test') results.set(e.values[0], { status: e.values[1], reason: e.values[2] });
  }
  if (doc.result === 'throw') {
    const err = doc.error && typeof doc.error === 'object' ? doc.error : { message: String(doc.error) };
    return { fileStatus: { status: 'fail', reason: `module threw: ${err.name ?? 'error'}: ${err.message ?? ''}` }, results };
  }
  if (doc.result === 'panic') {
    const last = started.length > 0 ? started[started.length - 1] : null;
    if (last !== null && !results.has(last)) results.set(last, { status: 'fail', reason: `panic: ${String(doc.error).slice(0, 300)}` });
    for (const id of planned) if (!results.has(id)) results.set(id, { status: 'fail', reason: 'not run: an earlier test panicked' });
    return { fileStatus: null, results };
  }
  for (const id of planned) if (!results.has(id)) results.set(id, { status: 'fail', reason: 'no result reported' });
  return { fileStatus: null, results };
}

export const keyOf = (spec, id) => `${spec}${KEY_SEP}${id}`;

// The ratchet: a test the ledger marks pass must still pass (a file-level entry covers its tests). Returns the regressions.
export function ratchet(ledger, current, selected) {
  const regressions = [];
  for (const [key, prev] of Object.entries(ledger)) {
    if (prev !== 'pass') continue;
    const spec = key.split(KEY_SEP)[0];
    if (!selected.has(spec)) continue;
    const hit = current.get(key) ?? current.get(spec);
    const now = hit ? hit.status : 'missing';
    if (now !== 'pass') regressions.push({ key, now, reason: hit?.reason ?? '' });
  }
  return regressions;
}

function sortKeys(o) {
  return Object.fromEntries(Object.keys(o).sort().map((k) => [k, o[k]]));
}

function parseArgs(argv) {
  const opt = (name, dflt) => {
    const i = argv.indexOf(name);
    return i >= 0 ? argv[i + 1] : dflt;
  };
  return {
    vueCore: opt('--vue-core', undefined),
    filter: opt('--filter', ''),
    ledger: path.resolve(ROOT, opt('--status', path.relative(ROOT, DEFAULT_LEDGER))),
    writeStatus: argv.includes('--write-status'),
    verbose: argv.includes('--verbose'),
  };
}

function compileShim() {
  const compiled = compileText(readFileSync(SHIM_FILE, 'utf8'), { filename: SHIM_FILE, specifier: SHIM_NAME, lang: 'js' });
  if (compiled.rust === undefined) throw new Error(`the vitest shim does not compile: ${firstMessage(compiled.diagnostics[0])}`);
  return compiled.rust;
}

// The export names of the native "vue" module, from a probe build that imports the namespace and lists it.
function probeVueExports() {
  const src = "import * as V from 'vue'\nfor (const k of Object.keys(V)) trace('upstream-vue-export', k)\n";
  const compiled = compileText(src, { filename: 'upstream-probe.js', specifier: 'upstream-probe', lang: 'js' });
  if (compiled.rust === undefined) throw new Error(`the probe does not compile: ${firstMessage(compiled.diagnostics[0])}`);
  writeCrate([{ name: 'upstream-probe', rust: compiled.rust }], { dir: PROBE_DIR, pkg: 'vn-upstream-probe', vue: true });
  const built = buildCrate(PROBE_DIR);
  if (!built.ok) throw new Error(`probe build failed: ${built.output.split('\n').filter((l) => /^error/.test(l)).slice(0, 3).join(' | ')}`);
  const run = runOne('upstream-probe', PROBE_BINARY);
  if (!run.ok || run.doc.result !== 'ok') throw new Error(`probe run failed: ${run.ok ? run.doc.error : run.error}`);
  return new Set(run.doc.entries.filter((e) => e.label === 'upstream-vue-export').map((e) => e.values[0]));
}

function fmtCounts(c) {
  return `${String(c.total).padStart(5)} ${String(c.pass).padStart(5)} ${String(c.fail).padStart(5)} ${String(c['compile-error']).padStart(14)} ${String(c.unsupported).padStart(12)}`;
}

export async function main(argv) {
  const opts = parseArgs(argv);
  const vueCore = resolveVueCore(opts.vueCore);
  if (vueCore === null) {
    console.error(`no vue-core checkout with ${SPEC_DIR}: pass --vue-core <dir>, set VUE_CORE_DIR, or clone https://github.com/vuejs/core at v3.5.43`);
    return 2;
  }
  const specs = listSpecs(vueCore).filter((s) => s.name.includes(opts.filter));
  const vueExports = probeVueExports();
  const fileStatus = new Map();
  const modules = [{ name: SHIM_NAME, rust: compileShim() }];
  for (const spec of specs) {
    const prepared = prepareSpec(readFileSync(spec.file, 'utf8'), {
      filename: path.relative(ROOT, spec.file),
      specifier: SCENARIO_PREFIX + spec.name,
      vueExports,
    });
    if (prepared.fileError) fileStatus.set(spec.name, prepared.fileError);
    else modules.push({ name: SCENARIO_PREFIX + spec.name, rust: prepared.rust });
  }
  if (modules.length > 1) {
    writeCrate(modules, { dir: CRATE_DIR, pkg: 'vn-upstream-scenarios', vue: true });
    const built = buildCrate(CRATE_DIR);
    if (!built.ok) {
      console.error(`BUILD FAILED: ${built.output.split('\n').filter((l) => /^error/.test(l)).slice(0, 3).join(' | ')}`);
      return 1;
    }
  }

  // The outcome of every test (key spec :: id) and of every file rejected or failed as a whole (key spec).
  const current = new Map();
  for (const spec of specs) {
    if (fileStatus.has(spec.name)) {
      current.set(spec.name, fileStatus.get(spec.name));
      continue;
    }
    const run = runOne(SCENARIO_PREFIX + spec.name, BINARY);
    const interpreted = interpretRun(run.ok ? run.doc : {}, run.ok ? null : run.error);
    if (interpreted.fileStatus) current.set(spec.name, interpreted.fileStatus);
    for (const [id, r] of interpreted.results) current.set(keyOf(spec.name, id), r);
  }

  const counts = new Map(specs.map((s) => [s.name, { total: 0, pass: 0, fail: 0, 'compile-error': 0, unsupported: 0 }]));
  const grand = { total: 0, pass: 0, fail: 0, 'compile-error': 0, unsupported: 0 };
  for (const [key, r] of current) {
    const spec = key.split(KEY_SEP)[0];
    const c = counts.get(spec);
    if (!c) continue;
    c.total += 1;
    c[r.status] = (c[r.status] ?? 0) + 1;
    grand.total += 1;
    grand[r.status] = (grand[r.status] ?? 0) + 1;
  }
  console.log('spec                                    total  pass  fail  compile-error  unsupported');
  for (const spec of specs) console.log(`${spec.name.padEnd(38)} ${fmtCounts(counts.get(spec.name))}`);
  console.log(`${'TOTAL'.padEnd(38)} ${fmtCounts(grand)}`);
  for (const [key, r] of [...current].sort((a, b) => (a[0] < b[0] ? -1 : 1))) {
    if (r.status === 'pass') continue;
    if (r.status === 'fail' || opts.verbose) console.log(`${r.status.padEnd(14)} ${key}  -- ${String(r.reason ?? '').slice(0, 220)}`);
  }

  const ledger = existsSync(opts.ledger) ? JSON.parse(readFileSync(opts.ledger, 'utf8')) : {};
  const selected = new Set(specs.map((s) => s.name));
  const regressions = ratchet(ledger, current, selected);
  for (const r of regressions) console.log(`REGRESSION ${r.key}: was pass, now ${r.now}${r.reason ? ` -- ${String(r.reason).slice(0, 220)}` : ''}`);
  if (opts.writeStatus) {
    const next = {};
    for (const [key, status] of Object.entries(ledger)) if (!selected.has(key.split(KEY_SEP)[0])) next[key] = status;
    for (const [key, r] of current) next[key] = r.status;
    // The ratchet only moves up: a test the ledger marks pass stays pass until it passes again, so a regression is never
    // written in. Its failure keeps showing on every run.
    for (const r of regressions) next[r.key] = 'pass';
    mkdirSync(path.dirname(opts.ledger), { recursive: true });
    writeFileSync(opts.ledger, `${JSON.stringify(sortKeys(next), null, 1)}\n`);
    const kept = regressions.length > 0 ? `; ${regressions.length} regression(s) not written, the ledger keeps them as pass` : '';
    console.log(`ledger written: ${path.relative(ROOT, opts.ledger)} (${Object.keys(next).length} entries${kept})`);
  }
  console.log(`upstream: ${grand.total} outcomes in ${specs.length} spec files: pass ${grand.pass}, fail ${grand.fail}, compile-error ${grand['compile-error']}, unsupported ${grand.unsupported}; ${regressions.length} regression(s)`);
  return regressions.length === 0 ? 0 : 1;
}

// The CLI runs only when this file is the entry point; the harness unit tests import the functions above.
if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  main(process.argv.slice(2)).then((code) => {
    process.exitCode = code;
  });
}

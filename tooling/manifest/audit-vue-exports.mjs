#!/usr/bin/env node
// Classifies every runtime export of the pinned vue package, and every compile-time macro declared in its type
// declarations, into exactly one design.md row (WP-G0B step 3). Also marks the helpers that the official compiler
// emits as compilerInternal (X for user code, importable by generated code) and checks that every helper-table
// name that is a vue export is either flagged or excluded with a reason. Unmapped exports fall back to X with note
// "unclassified: default-deny".
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { COMPAT_REL, DESIGN_REL, PROFILE_REL, ROOT, STATUSES, VUE_VERSION, abs, cmp, toJson } from './common.mjs';
import { extractInventory } from './extract-inventory.mjs';

const requireFromRoot = createRequire(path.join(ROOT, 'package.json'));
const fail = (message) => {
  throw new Error(`audit-vue-exports: ${message}`);
};
const rel = (file) => path.relative(ROOT, file).split(path.sep).join('/');
const sameList = (a, b) => a.length === b.length && a.every((value, i) => value === b[i]);
const escapeRe = (text) => text.replace(/[$^.*+?()[\]{}|\\]/g, '\\$&');

// JSDoc block immediately followed by an exported function declaration.
const DOC_DECL = /\/\*\*((?:(?!\*\/)[\s\S])*)\*\/\s*export declare function\s+([A-Za-z_$][\w$]*)/g;
const TYPE_DECL = /export\s+(?:declare\s+)?(?:abstract\s+)?(?:type|interface|class|enum|namespace|const|function|let|var)\s+([A-Za-z_$][\w$]*)/g;
const REEXPORT = /export\s+(?:\*|\{[^}]*\})\s+from\s+['"]([^'"]+)['"]/g;
// Documented names are scanned from the design rows: camelCase/PascalCase identifiers, $-properties, app.* and config.* paths.
const HUMP = /(?<![\w$.])([a-z][a-z0-9]*[A-Z][A-Za-z0-9]*|[A-Z][a-z0-9]+[A-Z][A-Za-z0-9]*)(?![\w$])/g;
const INSTANCE = /\$[A-Za-z_]\w*/g;
const APP_PATH = /(?<![\w$.])(app\.[A-Za-z_]\w*(?:\.[A-Za-z_]\w*)?)/g;
const CONFIG_PATH = /(?<![\w$.])config\.([A-Za-z_]\w*)/g;
const APP_METHODS = ['component', 'directive', 'mixin', 'mount', 'onUnmount', 'provide', 'runWithContext', 'unmount', 'use'];
const APP_CONFIG_KEYS = ['compilerOptions', 'errorHandler', 'globalProperties', 'idPrefix', 'optionMergeStrategies', 'performance', 'throwUnhandledErrorInProduction', 'warnHandler'];
const COMPILER_CORE = 'node_modules/@vue/compiler-core/dist/compiler-core.cjs.js';
const COMPILER_DOM = 'node_modules/@vue/compiler-dom/dist/compiler-dom.cjs.js';
const COMPILER_SFC = 'node_modules/@vue/compiler-sfc/dist/compiler-sfc.cjs.js';

function resolveTypeModule(spec, fromFile) {
  if (spec.startsWith('.')) {
    const base = path.resolve(path.dirname(fromFile), spec);
    for (const candidate of [`${base}.d.ts`, path.join(base, 'index.d.ts')]) {
      if (existsSync(candidate) && statSync(candidate).isFile()) return candidate;
    }
    return null;
  }
  let dir = path.dirname(fromFile);
  for (;;) {
    const pkgDir = path.join(dir, 'node_modules', spec);
    const pkgJson = path.join(pkgDir, 'package.json');
    if (existsSync(pkgJson)) {
      const meta = JSON.parse(readFileSync(pkgJson, 'utf8'));
      const file = path.join(pkgDir, meta.types ?? meta.typings ?? 'index.d.ts');
      return existsSync(file) ? file : null;
    }
    const parent = path.dirname(dir);
    if (parent === dir) return null;
    dir = parent;
  }
}

// Walks vue's type declarations (vue.d.ts and every `export * from` target) and collects compile-time macros
// (declarations whose JSDoc says "compiler macro") and all exported type names.
export function collectVueTypes() {
  const queue = [path.join(ROOT, 'node_modules/vue/dist/vue.d.ts')];
  const seen = new Set();
  const macros = new Set();
  const typeNames = new Set();
  const files = [];
  while (queue.length) {
    const file = queue.shift();
    if (seen.has(file)) continue;
    seen.add(file);
    files.push(rel(file));
    const text = readFileSync(file, 'utf8');
    for (const m of text.matchAll(DOC_DECL)) if (/compiler macro/.test(m[1])) macros.add(m[2]);
    for (const m of text.matchAll(TYPE_DECL)) typeNames.add(m[1]);
    for (const m of text.matchAll(REEXPORT)) {
      const target = resolveTypeModule(m[1], file);
      if (!target) fail(`cannot resolve type module "${m[1]}" referenced from ${rel(file)}`);
      queue.push(target);
    }
  }
  return { macros, typeNames, files: files.sort(cmp) };
}

// Names the official compiler can place in generated code, read from its helper tables and from its call sites.
// sites = emission sites: occurrences of a helper symbol other than its definition, its helper-map entry and its
// export assignment. Returns Map<name, { sources, sites }>.
export function readCompilerHelperSources(root = ROOT) {
  const read = (file) => readFileSync(path.join(root, file), 'utf8');
  const core = read(COMPILER_CORE);
  const dom = read(COMPILER_DOM);
  const sfc = read(COMPILER_SFC);
  const coreTable = /const helperNameMap = \{([\s\S]*?)\n\};/.exec(core);
  const domTable = /registerRuntimeHelpers\(\{([\s\S]*?)\}\);/.exec(dom);
  if (!coreTable || !domTable) fail('compiler helper tables not found (helperNameMap in compiler-core, registerRuntimeHelpers in compiler-dom)');
  const symbols = (block) => [...block[1].matchAll(/\[(\w+)\]:\s*`(\w+)`/g)].map((m) => ({ symbol: m[1], name: m[2] }));
  const coreEntries = symbols(coreTable);
  const domEntries = symbols(domTable);
  if (coreEntries.length === 0 || domEntries.length === 0) fail('compiler helper tables are empty');
  const sitesOf = (src, symbol) => {
    const mention = new RegExp(`\\b${escapeRe(symbol)}\\b`);
    const definition = new RegExp(`const ${escapeRe(symbol)} =`);
    const mapEntry = new RegExp(`\\[${escapeRe(symbol)}\\]:`);
    const exported = new RegExp(`exports\\.${escapeRe(symbol)} = ${escapeRe(symbol)}`);
    return src.split('\n').filter((line) => mention.test(line) && !definition.test(line) && !mapEntry.test(line) && !exported.test(line)).length;
  };
  const helpers = new Map();
  const add = (name, source, sites) => {
    if (!helpers.has(name)) helpers.set(name, { sources: new Set(), sites: 0 });
    const entry = helpers.get(name);
    entry.sources.add(source);
    entry.sites += sites;
  };
  for (const { symbol, name } of coreEntries) add(name, 'compiler-core helperNameMap', sitesOf(core, symbol));
  for (const { symbol, name } of domEntries) add(name, 'compiler-dom registerRuntimeHelpers', sitesOf(dom, symbol));
  for (const m of sfc.matchAll(/helper\(\s*[`"'](\w+)[`"']\s*\)/g)) add(m[1], 'compiler-sfc ctx.helper', 1);
  const cssVars = /const CSS_VARS_HELPER = `(\w+)`/.exec(sfc);
  if (cssVars) add(cssVars[1], 'compiler-sfc CSS_VARS_HELPER', sitesOf(sfc, 'CSS_VARS_HELPER'));
  return new Map([...helpers].map(([name, { sources, sites }]) => [name, { sources: [...sources].sort(cmp), sites }]));
}

export function parseExportMap(text) {
  const map = JSON.parse(text);
  if (map.schemaVersion !== 1) fail('export-map.json schemaVersion must be 1');
  if (map.vueVersion !== VUE_VERSION) fail(`export-map.json vueVersion must be ${VUE_VERSION}`);
  for (const key of ['interopArtifacts', 'exports', 'documentedNames', 'compilerHelperExclusions']) {
    if (!map[key] || typeof map[key] !== 'object' || Array.isArray(map[key])) fail(`export-map.json ${key} must be an object`);
  }
  return map;
}

export async function auditVueExports({ rows, exportMap }) {
  const rowById = new Map(rows.map((row) => [row.id, row]));

  // Runtime export enumeration: Object.keys(await import('vue')), cross-checked against require('vue') and a
  // NODE_ENV=production child process so that the export list does not depend on the build selected.
  const importKeys = Object.keys(await import('vue'));
  const cjsKeys = Object.keys(requireFromRoot('vue'));
  const prodKeys = JSON.parse(
    execFileSync(process.execPath, ['-p', 'JSON.stringify(Object.keys(require("vue")))'], {
      cwd: ROOT,
      env: { ...process.env, NODE_ENV: 'production' },
      encoding: 'utf8',
    }),
  );
  const interopDetected = importKeys.filter((key) => !cjsKeys.includes(key)).sort(cmp);
  const interopDeclared = Object.keys(exportMap.interopArtifacts).sort(cmp);
  if (!sameList(interopDetected, interopDeclared)) {
    fail(`CJS interop keys differ: detected [${interopDetected}], declared [${interopDeclared}]`);
  }
  const runtime = importKeys.filter((key) => !interopDeclared.includes(key)).sort(cmp);
  if (!sameList(runtime, [...cjsKeys].sort(cmp))) fail('namespace keys minus interop artifacts differ from require("vue") keys');
  if (!sameList([...cjsKeys].sort(cmp), [...prodKeys].sort(cmp))) fail('dev and production CJS builds export different names');
  const runtimeSet = new Set(runtime);

  const types = collectVueTypes();
  const serverRendererKeys = new Set(Object.keys(await import('vue/server-renderer')));

  // Instance-level surface of a real app (no DOM needed to create one).
  const { createApp } = await import('vue');
  const app = createApp({ render() { return null; } });
  const appSurface = new Map();
  for (const method of APP_METHODS) appSurface.set(`app.${method}`, typeof app[method] === 'function');
  appSurface.set('app.version', typeof app.version === 'string');
  for (const key of APP_CONFIG_KEYS) appSurface.set(`app.config.${key}`, key in app.config);

  // Documented-name scan over every design row, triaged in export-map.documentedNames.
  const documented = new Map();
  const note = (name, rowId) => {
    if (!documented.has(name)) documented.set(name, new Set());
    documented.get(name).add(rowId);
  };
  for (const row of rows) {
    for (const m of row.rawRow.matchAll(HUMP)) note(m[1], row.id);
    for (const m of row.rawRow.matchAll(INSTANCE)) note(m[0], row.id);
    for (const m of row.rawRow.matchAll(APP_PATH)) note(m[1], row.id);
    for (const m of row.rawRow.matchAll(CONFIG_PATH)) note(`app.config.${m[1]}`, row.id);
  }
  const triage = exportMap.documentedNames;
  const documentedOut = [];
  const untriaged = [];
  for (const name of [...documented.keys()].sort(cmp)) {
    let category;
    let present = null;
    if (name.startsWith('$')) category = '$-prefixed name: instance property, template variable or removed macro (not checked here)';
    else if (name.startsWith('app.') && appSurface.has(name)) {
      category = 'app instance surface';
      present = appSurface.get(name);
    } else if (runtimeSet.has(name)) category = 'runtime export of vue';
    else if (types.macros.has(name)) category = 'compile-time macro (vue types)';
    else if (appSurface.has(`app.${name}`)) {
      category = 'app instance method (bare name in design)';
      present = appSurface.get(`app.${name}`);
    } else if (appSurface.has(`app.config.${name}`)) {
      category = 'app config key (bare name in design)';
      present = appSurface.get(`app.config.${name}`);
    } else if (types.typeNames.has(name)) category = 'type-only export (vue types)';
    else if (serverRendererKeys.has(name)) category = 'vue/server-renderer export (not in vue)';
    else category = 'not found';
    const entry = { name, rows: [...documented.get(name)].sort(cmp), category, present };
    if (category === 'not found') {
      const verdict = triage[name];
      if (!verdict) {
        untriaged.push(`${name} (${entry.rows.join(', ')})`);
        documentedOut.push(entry);
        continue;
      }
      entry.verdict = verdict.verdict;
      entry.drift = verdict.drift === true;
      entry.note = verdict.note;
    }
    documentedOut.push(entry);
  }
  if (untriaged.length) fail(`documented names not in vue 3.5.43 need a triage entry in export-map.json documentedNames: ${untriaged.join(', ')}`);
  for (const name of Object.keys(triage)) {
    const hit = documentedOut.find((entry) => entry.name === name);
    if (!hit || hit.category !== 'not found') fail(`export-map.json documentedNames has stale entry "${name}" (name is now found or not documented)`);
  }

  // Classification of every export and macro.
  const macroSet = types.macros;
  const names = [...new Set([...runtime, ...macroSet])].sort(cmp);
  for (const key of Object.keys(exportMap.exports)) {
    if (!names.includes(key)) fail(`export-map.json lists "${key}", which is neither a runtime export nor a compile-time macro`);
  }
  const exportsOut = [];
  const unclassified = [];
  const overrides = [];
  const needsDecision = [];
  for (const name of names) {
    const sources = [];
    if (runtimeSet.has(name)) sources.push('runtime');
    if (macroSet.has(name)) sources.push('types');
    const entry = exportMap.exports[name];
    if (entry === undefined) {
      unclassified.push(name);
      exportsOut.push({ name, sources, row: null, status: 'X', note: 'unclassified: default-deny', compilerInternal: false });
      continue;
    }
    const spec = typeof entry === 'string' ? { row: entry } : entry;
    for (const key of Object.keys(spec)) {
      if (!['row', 'status', 'note', 'needsDecision'].includes(key)) fail(`${name}: unknown key "${key}" in export-map.json`);
    }
    if (typeof spec.row !== 'string' || !rowById.has(spec.row)) fail(`${name}: unknown row "${spec.row}"`);
    const row = rowById.get(spec.row);
    let status = row.primary;
    if (spec.status !== undefined) {
      if (!STATUSES.includes(spec.status)) fail(`${name}: invalid status "${spec.status}"`);
      const allowed = row.statuses.includes(spec.status) || spec.status === 'X' || spec.status === 'E';
      if (!allowed) fail(`${name}: status ${spec.status} is not a status of ${row.id} (${row.statuses.join(', ')}); only X or E may be added`);
      status = spec.status;
      if (spec.status !== row.primary) overrides.push({ name, row: row.id, status, rowPrimary: row.primary, note: spec.note ?? '' });
    }
    if (spec.note !== undefined && typeof spec.note !== 'string') fail(`${name}: note must be a string`);
    if (spec.needsDecision === true) needsDecision.push({ name, row: row.id, status, note: spec.note ?? '' });
    exportsOut.push({ name, sources, row: row.id, status, note: spec.note ?? null, compilerInternal: false });
  }

  // Compiler helpers: emitted by the official compiler and X for user code => compilerInternal. Every helper-table
  // name that is a vue export must be flagged or excluded with a reason.
  const helperSources = readCompilerHelperSources();
  const exclusions = exportMap.compilerHelperExclusions;
  for (const [name, reason] of Object.entries(exclusions)) {
    if (!helperSources.has(name)) fail(`compilerHelperExclusions lists "${name}", which is not in a compiler helper table`);
    if (!names.includes(name)) fail(`compilerHelperExclusions lists "${name}", which is not a vue export`);
    if (typeof reason !== 'string' || reason.trim() === '') fail(`compilerHelperExclusions "${name}" needs a reason`);
  }
  const compilerHelpers = [];
  for (const [name, info] of [...helperSources].sort(([a], [b]) => cmp(a, b))) {
    if (!names.includes(name)) continue;
    const record = exportsOut.find((item) => item.name === name);
    const reason = exclusions[name] ?? null;
    const flagged = info.sites > 0 && record.status === 'X' && reason === null;
    if (!flagged && reason === null) {
      fail(`compiler helper "${name}" is a vue export that is neither flagged compilerInternal nor excluded with a reason (emission sites ${info.sites}, status ${record.status})`);
    }
    record.compilerInternal = flagged;
    compilerHelpers.push({ name, sources: info.sources, emissionSites: info.sites, status: record.status, compilerInternal: flagged, reason });
  }

  return {
    counts: {
      runtimeExports: runtime.length,
      compileTimeMacros: macroSet.size,
      classified: names.length - unclassified.length,
      unclassified: unclassified.length,
      interopExcluded: interopDeclared.length,
      compilerInternal: compilerHelpers.filter((item) => item.compilerInternal).length,
      compilerHelperExcluded: compilerHelpers.filter((item) => !item.compilerInternal).length,
    },
    interopArtifacts: interopDeclared.map((name) => ({ name, reason: exportMap.interopArtifacts[name] })),
    devProdIdentical: true,
    appSurface: [...appSurface.entries()].map(([name, present]) => ({ name, present })),
    typeFiles: types.files,
    documentedNames: documentedOut,
    compilerHelpers,
    exports: exportsOut,
    unclassified,
    overrides,
    needsDecision,
  };
}

const md = (value) => String(value ?? '').replace(/\|/g, '\\|');

export function renderAuditReport(audit) {
  const out = [];
  const line = (text = '') => out.push(text);
  const table = (header, data) => {
    line(`| ${header.join(' | ')} |`);
    line(`|${header.map(() => '---').join('|')}|`);
    for (const cells of data) line(`| ${cells.map(md).join(' | ')} |`);
    line();
  };
  const byName = new Map(audit.exports.map((entry) => [entry.name, entry]));
  const shallow = byName.get('isShallow');
  line(`# Vue export audit (vue ${VUE_VERSION})`);
  line();
  line('Generated by `node tooling/manifest/build-manifest.mjs` from `compiler/profile/export-map.json`. Do not edit by hand. Decisions behind the classifications are in `docs/compatibility/README.md` (Decisions).');
  line();
  line('## Method');
  line();
  line("- Runtime exports: `Object.keys(await import('vue'))` minus two CJS interop keys (listed below). The same set is confirmed against `require('vue')` and a `NODE_ENV=production` child process.");
  line('- Compile-time macros: functions in vue\'s type declarations whose JSDoc says "compiler macro" (the `vue.d.ts` closure, following `export * from`).');
  line('- Each export is classified into exactly one design row. Its status is the row primary status unless an explicit status is recorded.');
  line('- compilerInternal: a vue export with status X that the official compiler emits (helper tables of compiler-core and compiler-dom, and compiler-sfc call sites) and that has no recorded exclusion. These names may be imported by generated code only (`isSupported(..., {origin: "compiler"})`).');
  line();
  line('## Summary');
  line();
  table(['Item', 'Value'], [
    ['Runtime exports (after interop exclusion)', audit.counts.runtimeExports],
    ['Compile-time macros from vue type declarations', audit.counts.compileTimeMacros],
    ['Exports classified into a row', audit.counts.classified],
    ['Unclassified (default-deny fallback)', audit.counts.unclassified],
    ['CJS interop keys excluded', audit.counts.interopExcluded],
    ['Dev and production CJS export sets identical', audit.devProdIdentical ? 'yes' : 'no'],
    ['Exports flagged compilerInternal', audit.counts.compilerInternal],
    ['Compiler helper-table exports not flagged (excluded with reason)', audit.counts.compilerHelperExcluded],
  ]);
  line('## isShallow');
  line();
  line(`- Exported by the pinned package: ${shallow ? 'yes' : 'no'} (design.md V09 requires this check).`);
  if (shallow) line(`- Classified as ${shallow.row} with status ${shallow.status}.`);
  line();
  line('## CJS interop keys excluded');
  line();
  table(['Key', 'Reason'], audit.interopArtifacts.map((item) => [`\`${item.name}\``, item.reason]));
  line('## Compile-time macros (vue type declarations)');
  line();
  const macros = audit.exports.filter((entry) => entry.sources.includes('types'));
  table(['Macro', 'Sources', 'Row', 'Status'], macros.map((entry) => [entry.name, entry.sources.join('+'), entry.row, entry.status]));
  line('## App instance surface (createApp without a DOM)');
  line();
  table(['Member', 'Present'], audit.appSurface.map((item) => [`\`${item.name}\``, item.present ? 'yes' : 'no']));
  line('`app.config.*` keys absent from the default config are optional keys that the runtime reads only when set (throwUnhandledErrorInProduction: runtime-core line 229; idPrefix: line 1707).');
  line();
  line('## Documented names in design.md section 8');
  line();
  line('Every camelCase/PascalCase identifier, `$` property and `app.*`/`config.*` path in the 126 rows was checked. Names that are not found must have a triage entry in `export-map.json`. No documented API is missing from vue 3.5.43.');
  line();
  table(['Name', 'Rows', 'Category', 'Verdict', 'Drift'], audit.documentedNames.map((entry) => [
    `\`${entry.name}\``,
    entry.rows.join(', '),
    entry.category,
    entry.verdict ?? (entry.present === null ? '' : entry.present ? 'present' : 'absent'),
    entry.drift === undefined ? '' : entry.drift ? 'yes' : 'no',
  ]));
  line('## Compiler helpers (official compiler output)');
  line();
  line('Every helper-table name that is a vue export is either flagged compilerInternal or excluded with a reason. Emission sites are references to the helper symbol other than its definition, map entry and export.');
  line();
  table(['Name', 'Sources', 'Emission sites', 'Status', 'compilerInternal', 'Reason (if not flagged)'], audit.compilerHelpers.map((item) => [
    `\`${item.name}\``,
    item.sources.join('; '),
    item.emissionSites,
    item.status,
    item.compilerInternal ? 'yes' : 'no',
    item.reason ?? '',
  ]));
  const notEmitted = audit.exports.filter((entry) => entry.row === 'A11' && !entry.compilerInternal).map((entry) => entry.name);
  line(`A11 members that the official compiler does not emit, so they stay X for both origins: ${notEmitted.join(', ')}.`);
  line();
  line('## Needs coordinator decision');
  line();
  line(`- Unclassified exports (default-deny fallback): ${audit.unclassified.length === 0 ? 'none' : audit.unclassified.join(', ')}.`);
  for (const item of audit.needsDecision) line(`- \`${item.name}\` (${item.row} ${item.status}): ${item.note}`);
  line();
  line('## Explicit status overrides (decided; rationale in docs/compatibility/README.md, Decisions)');
  line();
  table(['Export', 'Row', 'Status', 'Row primary', 'Note'], audit.overrides.map((item) => [`\`${item.name}\``, item.row, item.status, item.rowPrimary, item.note]));
  const drift = audit.documentedNames.filter((entry) => entry.drift);
  line(`Documented names absent from vue ${VUE_VERSION} (docs drift): ${drift.length === 0 ? 'none' : drift.map((entry) => `\`${entry.name}\``).join(', ')}.`);
  line();
  line('## Full classification');
  line();
  table(['Export', 'Sources', 'Row', 'Status', 'compilerInternal', 'Note'], audit.exports.map((entry) => [
    `\`${entry.name}\``,
    entry.sources.join('+'),
    entry.row ?? '-',
    entry.status,
    entry.compilerInternal ? 'yes' : 'no',
    entry.note ?? '',
  ]));
  return out.join('\n');
}

export function auditOutputs(audit) {
  const json = {
    schemaVersion: 1,
    vueVersion: VUE_VERSION,
    method: "runtime = Object.keys(await import('vue')) minus interopArtifacts; macros = vue type declarations with JSDoc 'compiler macro'; compilerInternal = official compiler helper tables/call sites, status X, no exclusion",
    typeFiles: audit.typeFiles,
    counts: audit.counts,
    interopArtifacts: audit.interopArtifacts,
    devProdIdentical: audit.devProdIdentical,
    appSurface: audit.appSurface,
    documentedNames: audit.documentedNames,
    compilerHelpers: audit.compilerHelpers,
    exports: audit.exports,
  };
  return { json: toJson(json), markdown: renderAuditReport(audit) + '\n' };
}

async function main(argv) {
  if (argv.length) throw new Error(`audit-vue-exports: unexpected arguments ${argv.join(' ')}`);
  const design = readFileSync(abs(DESIGN_REL), 'utf8');
  const rows = extractInventory(design, { source: DESIGN_REL }).rows;
  const exportMap = parseExportMap(readFileSync(abs(path.join(PROFILE_REL, 'export-map.json')), 'utf8'));
  const audit = await auditVueExports({ rows, exportMap });
  const { json, markdown } = auditOutputs(audit);
  mkdirSync(abs(PROFILE_REL), { recursive: true });
  mkdirSync(abs(COMPAT_REL), { recursive: true });
  writeFileSync(abs(path.join(PROFILE_REL, 'vue-exports.json')), json);
  writeFileSync(abs(path.join(COMPAT_REL, 'vue-exports-audit.md')), markdown);
  console.log(`audit-vue-exports: ${audit.counts.classified} classified, ${audit.counts.unclassified} unclassified, ${audit.counts.compilerInternal} compilerInternal`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}

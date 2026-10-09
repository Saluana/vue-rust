#!/usr/bin/env node
// Regenerates every WP-G0B artifact from design.md and the hand-authored profile files:
//   compiler/profile/inventory.generated.json   design.md rows (extract-inventory.mjs)
//   compiler/profile/vue-exports.json           vue export classification (audit-vue-exports.mjs)
//   docs/compatibility/vue-exports-audit.md     human audit report (audit-vue-exports.mjs)
//   compiler/profile/manifest.json              the compiled manifest read by compiler/profile/index.mjs
//   docs/compatibility/inventory.md             generated human-readable table of all rows
// Output is deterministic (no timestamps, codepoint-sorted arrays). `--check` fails instead of rewriting when a
// committed artifact is stale.
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { COMPAT_REL, DESIGN_REL, PROFILE_REL, STATUSES, VUE_VERSION, abs, cmp, toJson } from './common.mjs';
import { extractInventory } from './extract-inventory.mjs';
import { auditOutputs, auditVueExports, parseExportMap } from './audit-vue-exports.mjs';

export const KINDS = ['vue', 'macro', 'syntax', 'builtin', 'module', 'ts', 'type', 'sfc', 'option', 'directive', 'modifier', 'attribute', 'component', 'element', 'native-tag', 'app', 'instance', 'flag', 'api', 'config'];
const EXPANSIONS_REL = path.join(PROFILE_REL, 'expansions.json');
const EXPORT_MAP_REL = path.join(PROFILE_REL, 'export-map.json');
const INVENTORY_REL = path.join(PROFILE_REL, 'inventory.generated.json');
const VUE_EXPORTS_REL = path.join(PROFILE_REL, 'vue-exports.json');
const VUE_AUDIT_MD_REL = path.join(COMPAT_REL, 'vue-exports-audit.md');
const MANIFEST_REL = path.join(PROFILE_REL, 'manifest.json');
const INVENTORY_MD_REL = path.join(COMPAT_REL, 'inventory.md');
const ALLOWED_ENTRY_KEYS = new Set(['name', 'kind', 'row', 'status', 'note']);

const fail = (message) => {
  throw new Error(`build-manifest: ${message}`);
};
const sameList = (a, b) => Array.isArray(a) && a.length === b.length && a.every((value, i) => value === b[i]);
const md = (value) => String(value ?? '').replace(/\|/g, '\\|');

// Validates expansions.json against the row table, the vue export classification and a real app instance.
export function validateExpansions(text, { rows, vueExports, appSurface }) {
  const doc = JSON.parse(text);
  if (doc.schemaVersion !== 1) fail('expansions.json schemaVersion must be 1');
  if (!sameList(doc.kinds, KINDS)) fail('expansions.json kinds must equal the build vocabulary');
  if (!Array.isArray(doc.entries)) fail('expansions.json entries must be an array');
  const rowById = new Map(rows.map((row) => [row.id, row]));
  const vueByName = new Map(vueExports.map((entry) => [entry.name, entry]));
  const seen = new Set();
  const entries = [];
  doc.entries.forEach((raw, index) => {
    const where = `expansions entries[${index}] (${raw?.kind}:${raw?.name})`;
    for (const key of Object.keys(raw)) if (!ALLOWED_ENTRY_KEYS.has(key)) fail(`${where}: unknown key "${key}"`);
    const { name, kind, row, status } = raw;
    if (typeof name !== 'string' || name === '') fail(`${where}: name must be a non-empty string`);
    if (!KINDS.includes(kind)) fail(`${where}: unknown kind "${kind}"`);
    if (!rowById.has(row)) fail(`${where}: unknown row "${row}"`);
    if (!STATUSES.includes(status)) fail(`${where}: invalid status "${status}"`);
    if (raw.note !== undefined && typeof raw.note !== 'string') fail(`${where}: note must be a string`);
    const owner = rowById.get(row);
    if (!(owner.statuses.includes(status) || status === 'X' || status === 'E')) {
      fail(`${where}: status ${status} is not a status of ${row} (${owner.statuses.join(', ')}); only X or E may be added`);
    }
    const key = `${kind}\u0000${name}`;
    if (seen.has(key)) fail(`${where}: duplicate entry`);
    seen.add(key);
    if (kind === 'vue' || kind === 'macro') {
      const exp = vueByName.get(name);
      if (!exp) fail(`${where}: "${name}" is not an export of vue ${VUE_VERSION}`);
      if (kind === 'macro' && !exp.sources.includes('types')) fail(`${where}: "${name}" is not a compile-time macro`);
      if (exp.row !== row || exp.status !== status) fail(`${where}: disagrees with vue-exports.json (${exp.row} ${exp.status})`);
    }
    if (kind === 'native-tag' && !/^[a-z][a-z0-9]*$/.test(name)) fail(`${where}: native tags are lowercase names without hyphens; custom elements are not entries`);
    if (kind === 'app' && /^app\.[A-Za-z]+$/.test(name) && status !== 'X' && status !== 'E') {
      if (appSurface.get(name) !== true) fail(`${where}: "${name}" is not present on a real app instance`);
    }
    entries.push({ name, kind, row, status, note: raw.note ?? null, compilerInternal: false });
  });
  return entries.sort((a, b) => cmp(a.kind, b.kind) || cmp(a.name, b.name));
}

function renderInventoryMarkdown(rows, entries, vueExports) {
  const namesFor = (rowId) => new Set([
    ...entries.filter((e) => e.row === rowId).map((e) => e.name),
    ...vueExports.filter((e) => e.row === rowId).map((e) => e.name),
  ]);
  const out = [];
  out.push('# Compatibility inventory', '');
  out.push('Generated by `node tooling/manifest/build-manifest.mjs` from `planning/vue-native-framework/design.md` section 8. Do not edit by hand.', '');
  out.push('Status letters: P faithful, A adaptation, W web-only, X excluded (must error), E experimental. "Status (verbatim)" is the design text before its first semicolon. "Set" lists the status letters derived from it, in order of appearance. "Named entries" counts distinct API, construct and export names mapped to the row (see `compiler/profile/expansions.json` and `vue-exports.json`).', '');
  out.push('## Rows', '');
  out.push('| ID | Section | Title | Status (verbatim) | Set | Prototype | Named entries | Tests and exclusions (verbatim) |');
  out.push('|---|---|---|---|---|---|---|---|');
  for (const row of rows) {
    out.push(`| ${row.id} | ${row.section} | ${md(row.title)} | ${md(row.statusText)} | ${row.statuses.join(', ')} | ${md(row.prototype ?? '—')} | ${namesFor(row.id).size} | ${md(row.tests)} |`);
  }
  out.push('');
  out.push('## Summary', '');
  out.push('| Status | Rows by primary status | Named entries | Vue exports |');
  out.push('|---|---|---|---|');
  for (const status of STATUSES) {
    const rowCount = rows.filter((row) => row.primary === status).length;
    const entryCount = entries.filter((e) => e.status === status).length;
    const vueCount = vueExports.filter((e) => e.status === status).length;
    out.push(`| ${status} | ${rowCount} | ${entryCount} | ${vueCount} |`);
  }
  out.push('');
  const uncovered = rows.filter((row) => namesFor(row.id).size === 0).map((row) => row.id);
  out.push('## Rows without a named entry', '');
  out.push(uncovered.length ? `${uncovered.join(', ')}. These rows are governed by their row-level contract and tests only; no name is claimed for them.` : 'None.', '');
  return out.join('\n') + '\n';
}

// Pure: returns Map<relativePath, content> for every generated artifact. Writes nothing.
export async function buildArtifacts({ designText, exportMapText, expansionsText } = {}) {
  const design = designText ?? readFileSync(abs(DESIGN_REL), 'utf8');
  const exportMapSource = exportMapText ?? readFileSync(abs(EXPORT_MAP_REL), 'utf8');
  const expansionsSource = expansionsText ?? readFileSync(abs(EXPANSIONS_REL), 'utf8');
  const inventory = extractInventory(design, { source: DESIGN_REL });
  const audit = await auditVueExports({ rows: inventory.rows, exportMap: parseExportMap(exportMapSource) });
  const audited = auditOutputs(audit);
  const appSurface = new Map(audit.appSurface.map((item) => [item.name, item.present]));
  const entries = validateExpansions(expansionsSource, { rows: inventory.rows, vueExports: audit.exports, appSurface });
  const manifest = {
    schemaVersion: 1,
    vueReference: VUE_VERSION,
    default: 'deny',
    rows: inventory.rows,
    entries,
    vueExports: audit.exports,
  };
  return new Map([
    [INVENTORY_REL, toJson(inventory)],
    [VUE_EXPORTS_REL, audited.json],
    [VUE_AUDIT_MD_REL, audited.markdown],
    [MANIFEST_REL, toJson(manifest)],
    [INVENTORY_MD_REL, renderInventoryMarkdown(inventory.rows, entries, audit.exports)],
  ]);
}

async function main(argv) {
  const check = argv.includes('--check');
  const unknown = argv.filter((arg) => arg !== '--check');
  if (unknown.length) fail(`unknown arguments ${unknown.join(' ')}`);
  const files = await buildArtifacts();
  if (check) {
    const stale = [...files].filter(([rel, content]) => !existsSync(abs(rel)) || readFileSync(abs(rel), 'utf8') !== content).map(([rel]) => rel);
    if (stale.length) fail(`stale generated files, run node tooling/manifest/build-manifest.mjs: ${stale.join(', ')}`);
    console.log(`build-manifest: ${files.size} generated files are up to date`);
    return;
  }
  for (const [rel, content] of files) {
    mkdirSync(path.dirname(abs(rel)), { recursive: true });
    writeFileSync(abs(rel), content);
  }
  console.log(`build-manifest: wrote ${files.size} generated files`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}

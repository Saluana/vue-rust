#!/usr/bin/env node
// Regenerates the WP-G0E standard-library and globals allowlist (design L18/L21; closed world, default-deny):
//   compiler/profile/stdlib.json   compiled entries read by compiler/profile/stdlib.mjs
//   docs/compatibility/stdlib.md   human table with counts and decisions
// Input: compiler/profile/stdlib.source.json (hand-authored). Every row ID and status is checked against
// compiler/profile/inventory.generated.json with the WP-G0B rule. Output is deterministic (no timestamps, entries sorted
// by kind order then codepoint path). `--check` fails instead of rewriting when a committed file is stale.
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { COMPAT_REL, PROFILE_REL, STATUSES, abs, cmp, toJson } from './common.mjs';

export const STDLIB_KINDS = ['global', 'ctor', 'static', 'proto', 'const', 'symbol', 'prop'];
export const STDLIB_TARGETS = ['native', 'web', 'test'];
export const SOURCE_REL = path.join(PROFILE_REL, 'stdlib.source.json');
export const JSON_REL = path.join(PROFILE_REL, 'stdlib.json');
export const MD_REL = path.join(COMPAT_REL, 'stdlib.md');
const INVENTORY_REL = path.join(PROFILE_REL, 'inventory.generated.json');
const ENTRY_KEYS = new Set(['kind', 'path', 'status', 'row', 'targets', 'code', 'note', 'hint']);
const REQUIRED_KEYS = ['kind', 'path', 'status', 'row', 'targets'];
const IDENT = '[A-Za-z_$][A-Za-z0-9_$]*';
// Dotted paths, optional %Intrinsic% root (proto only) and optional trailing [Symbol.name].
const PATH_RE = new RegExp(`^(%[A-Za-z]+%|${IDENT})(\\.${IDENT})*(\\[Symbol\\.[A-Za-z]+\\])?$`);
const BARE_RE = new RegExp(`^${IDENT}$`);
const STATIC_RE = new RegExp(`^${IDENT}\\.${IDENT}$`);
const SYMBOL_RE = new RegExp(`^Symbol\\.${IDENT}$`);
const PROP_RE = new RegExp(`^${IDENT}#${IDENT}$`);
const CODE_RE = /^VN\d{4}$/;

const fail = (message) => {
  throw new Error(`build-stdlib: ${message}`);
};
const md = (value) => String(value ?? '').replace(/\|/g, '\\|');
const sameList = (a, b) => Array.isArray(a) && a.length === b.length && a.every((value, i) => value === b[i]);

// Why a path does not have the form its kind requires, or null. Paths are plain identifiers joined by dots: no
// computed access, so memberPath() on source can never produce a %Intrinsic% or Owner#prop path.
export function pathShapeProblem(kind, p) {
  if (kind === 'prop') return PROP_RE.test(p) ? null : 'must be Owner#name';
  if (!PATH_RE.test(p)) return 'is not a dotted path';
  const intrinsic = p.startsWith('%');
  if (intrinsic && kind !== 'proto') return 'uses an %Intrinsic% root, which only proto entries may use';
  switch (kind) {
    case 'global':
    case 'ctor':
      return BARE_RE.test(p) ? null : 'must be a bare identifier';
    case 'const':
      return null;
    case 'static':
      return STATIC_RE.test(p) ? null : 'must be Owner.name';
    case 'proto':
      return intrinsic || p.includes('.prototype.') || p.includes('.prototype[')
        ? null
        : 'must be Owner.prototype.name, Owner.prototype[Symbol.x] or %Intrinsic%.name';
    case 'symbol':
      return SYMBOL_RE.test(p) ? null : 'must be Symbol.name';
    default:
      return `unknown kind ${kind}`;
  }
}

const optionalText = (value, where) => {
  if (value === undefined) return null;
  if (typeof value !== 'string' || value.trim() === '') fail(`${where} must be a non-empty string`);
  return value;
};

// Checks one source entry against the design rows and returns the compiled form (all keys present, nulls for absent text).
function compileEntry(raw, index, rows) {
  const where = `entries[${index}] (${raw?.kind}:${raw?.path})`;
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) fail(`${where}: must be an object`);
  for (const key of Object.keys(raw)) if (!ENTRY_KEYS.has(key)) fail(`${where}: unknown key "${key}"`);
  for (const key of REQUIRED_KEYS) if (raw[key] === undefined) fail(`${where}: ${key} is required`);
  const { kind, status, row } = raw;
  const p = raw.path;
  if (!STDLIB_KINDS.includes(kind)) fail(`${where}: unknown kind "${kind}"`);
  if (typeof p !== 'string' || p === '') fail(`${where}: path must be a non-empty string`);
  const shape = pathShapeProblem(kind, p);
  if (shape) fail(`${where}: path ${p} ${shape}`);
  if (!STATUSES.includes(status)) fail(`${where}: invalid status "${status}"`);
  const owner = rows.get(row);
  if (!owner) fail(`${where}: unknown design row "${row}"`);
  if (!(owner.statuses.includes(status) || status === 'X' || status === 'E')) {
    fail(`${where}: status ${status} is not a status of ${row} (${owner.statuses.join(', ')}); only X or E may be added`);
  }
  if (!Array.isArray(raw.targets) || raw.targets.length === 0) fail(`${where}: targets must be a non-empty array`);
  for (const t of raw.targets) if (!STDLIB_TARGETS.includes(t)) fail(`${where}: unknown target "${t}"`);
  if (new Set(raw.targets).size !== raw.targets.length) fail(`${where}: duplicate target`);
  const targets = STDLIB_TARGETS.filter((t) => raw.targets.includes(t));
  if (status === 'W' && !(targets.length === 1 && targets[0] === 'web')) fail(`${where}: a W entry lists only the web target`);
  const note = optionalText(raw.note, `${where}: note`);
  const hint = optionalText(raw.hint, `${where}: hint`);
  if ((status === 'X' || status === 'E' || status === 'W') && hint === null) fail(`${where}: ${status} entries need a hint`);
  let code = null;
  if (raw.code !== undefined) {
    if (typeof raw.code !== 'string' || !CODE_RE.test(raw.code)) fail(`${where}: code must look like VN4008`);
    if (status !== 'X' && status !== 'E') fail(`${where}: only X or E entries carry a VN code`);
    code = raw.code;
  }
  return { kind, path: p, status, row, targets, code, note, hint };
}

// Validates the whole source. Returns the compiled entries (sorted) and the decisions list.
export function compileSource(source, inventory) {
  if (source === null || typeof source !== 'object') fail('source must be an object');
  if (source.schemaVersion !== 1) fail('schemaVersion must be 1');
  if (source.default !== 'deny') fail('default must be "deny"');
  if (!sameList(source.kinds, STDLIB_KINDS)) fail('kinds must equal the build vocabulary');
  if (!sameList(source.targets, STDLIB_TARGETS)) fail('targets must equal the build vocabulary');
  if (!Array.isArray(source.decisions) || source.decisions.some((d) => typeof d !== 'string' || d === '')) {
    fail('decisions must be an array of non-empty strings');
  }
  if (!Array.isArray(source.entries) || source.entries.length === 0) fail('entries must be a non-empty array');
  const rows = new Map(inventory.rows.map((row) => [row.id, row]));
  const seen = new Map();
  const entries = source.entries.map((raw, index) => {
    const entry = compileEntry(raw, index, rows);
    if (seen.has(entry.path)) fail(`duplicate path ${entry.path} (kinds ${seen.get(entry.path)} and ${entry.kind})`);
    seen.set(entry.path, entry.kind);
    return entry;
  });
  const kindOrder = new Map(STDLIB_KINDS.map((kind, i) => [kind, i]));
  entries.sort((a, b) => kindOrder.get(a.kind) - kindOrder.get(b.kind) || cmp(a.path, b.path));
  return { entries, decisions: source.decisions };
}

function renderMarkdown(entries, decisions) {
  const count = (kind, status) => entries.filter((e) => (kind === undefined || e.kind === kind) && (status === undefined || e.status === status)).length;
  const lines = [
    '# Standard library and globals allowlist',
    '',
    '<!-- Generated by tooling/manifest/build-stdlib.mjs from compiler/profile/stdlib.source.json. Do not edit by hand. -->',
    '',
    'Closed-world, default-deny list of the built-ins the compiler may reference and the runtime must provide (WP-G0E; design rows L18 and L21).',
    'Anything not listed is unsupported. X and E entries, and unknown names, fail with a diagnostic. The lookup API is `compiler/profile/stdlib.mjs`, also re-exported from `compiler/profile/index.mjs`.',
    '',
    'Status letters: **P** faithful, **A** adaptation, **W** web-only (no entries), **X** excluded (must fail), **E** experimental (not promised). Targets: `native`, `web` and `test`. A P or A entry is supported on each target it lists.',
    '',
    `Regenerate with \`node tooling/manifest/build-stdlib.mjs\`; check with \`node tooling/manifest/build-stdlib.mjs --check\`.`,
    '',
    '## Counts',
    '',
    '| Kind | P | A | W | X | E | Supported (P+A+W) | Excluded (X+E) | Total |',
    '|---|---:|---:|---:|---:|---:|---:|---:|---:|',
  ];
  for (const kind of STDLIB_KINDS) {
    const supported = count(kind, 'P') + count(kind, 'A') + count(kind, 'W');
    const excluded = count(kind, 'X') + count(kind, 'E');
    lines.push(`| ${kind} | ${count(kind, 'P')} | ${count(kind, 'A')} | ${count(kind, 'W')} | ${count(kind, 'X')} | ${count(kind, 'E')} | ${supported} | ${excluded} | ${count(kind)} |`);
  }
  const all = (status) => count(undefined, status);
  lines.push(`| **all** | ${all('P')} | ${all('A')} | ${all('W')} | ${all('X')} | ${all('E')} | ${all('P') + all('A') + all('W')} | ${all('X') + all('E')} | ${entries.length} |`);
  lines.push('', '## Decisions', '');
  decisions.forEach((text, i) => lines.push(`${i + 1}. ${text}`));
  for (const kind of STDLIB_KINDS) {
    const group = entries.filter((e) => e.kind === kind);
    if (group.length === 0) continue;
    lines.push('', `## ${kind} (${group.length})`, '', '| Path | Status | Row | Targets | Code | Note and hint |', '|---|---|---|---|---|---|');
    for (const e of group) {
      const targets = e.targets.length === STDLIB_TARGETS.length ? 'all' : e.targets.join(', ');
      const text = [e.note, e.hint === null ? null : `Hint: ${e.hint}`].filter(Boolean).join(' ');
      lines.push(`| \`${md(e.path)}\` | ${e.status} | ${e.row} | ${targets} | ${e.code ?? ''} | ${md(text)} |`);
    }
  }
  return lines.join('\n') + '\n';
}

// Pure build: returns Map(relative path -> file text) for the two generated files.
export function buildStdlib({ sourceText, inventoryText } = {}) {
  const source = JSON.parse(sourceText ?? readFileSync(abs(SOURCE_REL), 'utf8'));
  const inventory = JSON.parse(inventoryText ?? readFileSync(abs(INVENTORY_REL), 'utf8'));
  const { entries, decisions } = compileSource(source, inventory);
  const doc = { schemaVersion: 1, default: 'deny', kinds: STDLIB_KINDS, targets: STDLIB_TARGETS, entries };
  return new Map([
    [JSON_REL, toJson(doc)],
    [MD_REL, renderMarkdown(entries, decisions)],
  ]);
}

function main(argv) {
  const check = argv.includes('--check');
  const unknown = argv.filter((arg) => arg !== '--check');
  if (unknown.length) fail(`unknown arguments ${unknown.join(' ')}`);
  const files = buildStdlib();
  if (check) {
    const stale = [...files].filter(([rel, content]) => !existsSync(abs(rel)) || readFileSync(abs(rel), 'utf8') !== content).map(([rel]) => rel);
    if (stale.length) fail(`stale generated files, run node tooling/manifest/build-stdlib.mjs: ${stale.join(', ')}`);
    console.log(`build-stdlib: ${files.size} generated files are up to date`);
    return;
  }
  for (const [rel, content] of files) {
    mkdirSync(path.dirname(abs(rel)), { recursive: true });
    writeFileSync(abs(rel), content);
  }
  console.log(`build-stdlib: wrote ${files.size} generated files`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}

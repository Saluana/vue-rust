// Language/Vue profile lookups (WP-G0B). Default-deny: a name that is not in the manifest is unsupported.
//   lookup(kind, name) -> { status, row, note, compilerInternal } | null
//   isSupported(kind, name, target, { origin } = {}) -> boolean
// Status: P and A are supported on both targets; W only on target 'web'; X, E and unknown names are not supported.
// compilerInternal entries (X helpers that the official compiler emits) are supported only for origin 'compiler',
// i.e. generated code. origin defaults to 'user'. compilerInternal never removes support from a public entry.
import { readFileSync } from 'node:fs';

const MANIFEST_URL = new URL('./manifest.json', import.meta.url);
const STATUS_LETTERS = new Set(['P', 'A', 'W', 'X', 'E']);
const TARGETS = new Set(['native', 'web']);
const ORIGINS = new Set(['user', 'compiler']);

let loaded = null;

function deepFreeze(value) {
  if (value && typeof value === 'object' && !Object.isFrozen(value)) {
    Object.freeze(value);
    for (const key of Object.keys(value)) deepFreeze(value[key]);
  }
  return value;
}

function validate(manifest) {
  const fail = (message) => {
    throw new Error(`compiler/profile: invalid manifest.json: ${message}`);
  };
  if (manifest.schemaVersion !== 1) fail('schemaVersion must be 1');
  if (manifest.vueReference !== '3.5.43') fail(`vueReference must be 3.5.43, found ${manifest.vueReference}`);
  if (manifest.default !== 'deny') fail('default must be "deny"');
  if (!Array.isArray(manifest.rows) || manifest.rows.length !== 126) fail('rows must contain the 126 design rows');
  for (const row of manifest.rows) {
    if (!row.statuses.length || row.statuses.some((s) => !STATUS_LETTERS.has(s))) fail(`row ${row.id} has an invalid status set`);
  }
  for (const list of [manifest.entries, manifest.vueExports]) {
    if (!Array.isArray(list)) fail('entries and vueExports must be arrays');
    for (const item of list) {
      if (!STATUS_LETTERS.has(item.status)) fail(`${item.name} has invalid status ${item.status}`);
      if (typeof item.compilerInternal !== 'boolean') fail(`${item.name} has no boolean compilerInternal`);
    }
  }
}

// Built once from the frozen manifest. Maps (not plain objects) keep names such as "constructor" or "__proto__"
// from resolving to inherited properties.
function load() {
  if (loaded) return loaded;
  const manifest = JSON.parse(readFileSync(MANIFEST_URL, 'utf8'));
  validate(manifest);
  const kinds = new Map();
  const add = (kind, name, record) => {
    if (!kinds.has(kind)) kinds.set(kind, new Map());
    const byName = kinds.get(kind);
    if (byName.has(name)) throw new Error(`compiler/profile: duplicate ${kind}:${name} in manifest.json`);
    byName.set(name, record);
  };
  for (const entry of manifest.vueExports) add('vue', entry.name, entry);
  for (const entry of manifest.entries) {
    if (entry.kind !== 'vue') {
      add(entry.kind, entry.name, entry);
      continue;
    }
    // Expansions of vue exports repeat vueExports; the builder guarantees they agree, so only check it here.
    const exp = kinds.get('vue').get(entry.name);
    if (!exp || exp.row !== entry.row || exp.status !== entry.status) {
      throw new Error(`compiler/profile: manifest.json vue entry ${entry.name} disagrees with vueExports`);
    }
  }
  loaded = { manifest: deepFreeze(manifest), kinds };
  return loaded;
}

export function loadManifest() {
  return load().manifest;
}

export function lookup(kind, name) {
  if (typeof kind !== 'string' || typeof name !== 'string') return null;
  const hit = load().kinds.get(kind)?.get(name);
  if (!hit) return null;
  return { status: hit.status, row: hit.row, note: hit.note ?? null, compilerInternal: hit.compilerInternal === true };
}

export function isSupported(kind, name, target, options = {}) {
  if (!TARGETS.has(target)) throw new TypeError(`isSupported: target must be "native" or "web", got ${JSON.stringify(target)}`);
  if (options === null || typeof options !== 'object') throw new TypeError('isSupported: options must be an object');
  const origin = options.origin ?? 'user';
  if (!ORIGINS.has(origin)) throw new TypeError(`isSupported: origin must be "user" or "compiler", got ${JSON.stringify(origin)}`);
  const hit = lookup(kind, name);
  if (hit === null) return false;
  if (origin === 'compiler' && hit.compilerInternal) return true;
  if (hit.status === 'P' || hit.status === 'A') return true;
  if (hit.status === 'W') return target === 'web';
  return false;
}

// Standard-library and globals allowlist (WP-G0E, see stdlib.mjs). Re-exported so callers use one import path.
export { lookupStdlib, isStdlibSupported, listSupported, resolveGlobal, memberPath } from './stdlib.mjs';

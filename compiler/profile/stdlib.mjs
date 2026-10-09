// Standard-library and globals allowlist (WP-G0E; design L18/L21). Closed world, default-deny: a name that is not in
// stdlib.json is unsupported. The file is generated from stdlib.source.json by tooling/manifest/build-stdlib.mjs.
//   lookupStdlib(kind, path) -> { kind, path, status, row, targets, code, note, hint } | null
//   isStdlibSupported(path, target) -> boolean
//   listSupported(kind?) -> entries with status P, A or W, sorted by kind order then path
//   resolveGlobal(name, target) -> { status, hint } | null        (bare identifiers only)
//   memberPath(node) -> dotted path such as 'Object.keys' | null  (Babel Identifier/MemberExpression chains)
// target is 'native', 'web' or 'test'. The effective status for a target is X when the entry does not list that target,
// and also for W off the web target. P and A (and W on web) are supported; X, E and unknown names are not.
import { readFileSync } from 'node:fs';

const STDLIB_URL = new URL('./stdlib.json', import.meta.url);
const KINDS = ['global', 'ctor', 'static', 'proto', 'const', 'symbol', 'prop'];
const GLOBAL_KINDS = new Set(['global', 'ctor', 'const']);
const TARGET_LIST = ['native', 'web', 'test'];
const TARGETS = new Set(TARGET_LIST);
const STATUS_LETTERS = new Set(['P', 'A', 'W', 'X', 'E']);
const SUPPORTED = new Set(['P', 'A', 'W']);
const IDENTIFIER = /^[A-Za-z_$][A-Za-z0-9_$]*$/;

let loaded = null;

function validate(doc) {
  const fail = (message) => {
    throw new Error(`compiler/profile: invalid stdlib.json: ${message}`);
  };
  if (doc.schemaVersion !== 1) fail('schemaVersion must be 1');
  if (doc.default !== 'deny') fail('default must be "deny"');
  if (JSON.stringify(doc.kinds) !== JSON.stringify(KINDS)) fail('kinds must equal the build vocabulary');
  if (JSON.stringify(doc.targets) !== JSON.stringify(TARGET_LIST)) fail('targets must equal the build vocabulary');
  if (!Array.isArray(doc.entries)) fail('entries must be an array');
  for (const entry of doc.entries) {
    if (!KINDS.includes(entry.kind)) fail(`${entry.path} has unknown kind ${entry.kind}`);
    if (!STATUS_LETTERS.has(entry.status)) fail(`${entry.path} has invalid status ${entry.status}`);
    if (!Array.isArray(entry.targets) || entry.targets.some((t) => !TARGETS.has(t))) fail(`${entry.path} has invalid targets`);
  }
}

// Built once from the frozen file. Maps (not plain objects) keep names such as "constructor" or "__proto__" from
// resolving to inherited properties. Paths are unique across kinds, so byPath answers without a kind.
function load() {
  if (loaded) return loaded;
  const doc = JSON.parse(readFileSync(STDLIB_URL, 'utf8'));
  validate(doc);
  const byKind = new Map(KINDS.map((kind) => [kind, new Map()]));
  const byPath = new Map();
  for (const entry of doc.entries) {
    if (byPath.has(entry.path)) throw new Error(`compiler/profile: duplicate path ${entry.path} in stdlib.json`);
    Object.freeze(entry.targets);
    Object.freeze(entry);
    byKind.get(entry.kind).set(entry.path, entry);
    byPath.set(entry.path, entry);
  }
  loaded = { byKind, byPath };
  return loaded;
}

const copy = (entry) => ({ ...entry, targets: [...entry.targets] });

function checkTarget(target) {
  if (!TARGETS.has(target)) throw new TypeError(`stdlib: target must be "native", "web" or "test", got ${JSON.stringify(target)}`);
}

// The status a target sees. An entry that does not list the target is X there, and W is supported only on web.
// Exported for tests; the W branch is not reachable from the committed data, which has no W entries.
export function effectiveStatus(entry, target) {
  if (!entry.targets.includes(target)) return 'X';
  if (entry.status === 'W' && target !== 'web') return 'X';
  return entry.status;
}

export function lookupStdlib(kind, path) {
  if (typeof kind !== 'string' || typeof path !== 'string') return null;
  const entry = load().byKind.get(kind)?.get(path);
  return entry ? copy(entry) : null;
}

export function isStdlibSupported(path, target) {
  checkTarget(target);
  const entry = typeof path === 'string' ? load().byPath.get(path) : undefined;
  return entry !== undefined && SUPPORTED.has(effectiveStatus(entry, target));
}

export function listSupported(kind) {
  if (kind !== undefined && !KINDS.includes(kind)) throw new TypeError(`stdlib: unknown kind ${JSON.stringify(kind)}`);
  const out = [];
  for (const k of KINDS) {
    if (kind !== undefined && k !== kind) continue;
    for (const entry of load().byKind.get(k).values()) {
      if (SUPPORTED.has(entry.status)) out.push(copy(entry));
    }
  }
  return out;
}

// For a free identifier that the scope analysis left unresolved (C1a globalsUsed). Returns the status the target sees
// and the diagnostic hint, or null when the name is not a global, constructor or constant of the manifest.
export function resolveGlobal(name, target) {
  checkTarget(target);
  if (typeof name !== 'string' || !IDENTIFIER.test(name)) {
    throw new TypeError(`stdlib: resolveGlobal takes a bare identifier, got ${JSON.stringify(name)}; use memberPath for member chains`);
  }
  const entry = load().byPath.get(name);
  if (entry === undefined || !GLOBAL_KINDS.has(entry.kind)) return null;
  const status = effectiveStatus(entry, target);
  if (SUPPORTED.has(status)) return { status, hint: null };
  if (!entry.targets.includes(target)) {
    return { status: 'X', hint: `${name} is only available on the ${entry.targets.join(' and ')} target.` };
  }
  return { status, hint: entry.hint ?? `${name} is not supported.` };
}

// Turns a Babel Identifier or non-computed MemberExpression chain into a dotted path. Anything else (computed access,
// optional chains, this, calls, private names) returns null, which the caller must treat as unsupported.
// Shadowing is not checked here: the scope analysis decides whether the root is a global.
export function memberPath(node) {
  if (node === null || typeof node !== 'object') return null;
  if (node.type === 'Identifier') return typeof node.name === 'string' ? node.name : null;
  if (node.type !== 'MemberExpression' || node.computed === true) return null;
  const property = node.property;
  if (!property || property.type !== 'Identifier' || typeof property.name !== 'string') return null;
  const owner = memberPath(node.object);
  return owner === null ? null : `${owner}.${property.name}`;
}

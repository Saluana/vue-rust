// HIR schema catalogue (WP-C1b, task 4.1): the node kinds and record shapes of the current hirVersion as data, with a content hash.
// HIR_SCHEMA_HASH is the sha256 (node:crypto) of the canonical JSON of { nodes, records }. It does not include
// HIR_VERSION: a change of shape changes the hash, and tests/compiler/lower/hir-schema.test.mjs fails unless the hash
// committed for the current hirVersion is updated, which means bumping hirVersion for a shape change.
//
// Nodes: `k` -> field sets. A node matches a field set when its own keys (k excluded) are exactly that set. `pos` is
// listed where the lowering gives a node its source span; references, computed and static keys and for-in/of heads
// carry no pos. Records: objects without `k`, matched the same way by their keys.
// checkHirShape(hir) walks a module record and reports every node kind, node field set and record shape that the
// catalogue does not list, and returns the catalogue entries it met (so the test can flag entries nothing produces).
import { createHash } from 'node:crypto';

export const HIR_NODES = Object.freeze({
  // expressions
  undef: [['pos']],
  null: [['pos']],
  bool: [['pos', 'v']],
  num: [['pos', 's']],
  str: [['pos', 'u']],
  gvar: [['pos', 'name']],
  var: [['pos', 'r', 'name', 'tdz']],
  thisval: [['pos']],
  thiscell: [['pos', 'r']],
  fn: [['pos', 'f', 'caps']],
  await: [['pos', 'e', 'site']],
  arr: [['pos', 'elems']],
  hole: [['pos']],
  obj: [['pos', 'props']],
  init: [['pos', 'key', 'v'], ['pos', 'key', 'v', 'nameFromKey']],
  get: [['pos', 'key', 'v']],
  set: [['pos', 'key', 'v']],
  spread: [['pos', 'e']],
  mem: [['pos', 'o', 'key']],
  call: [['pos', 'f', 'args'], ['pos', 'f', 'args', 'chainCallee']],
  new: [['pos', 'f', 'args']],
  chain: [['pos', 'base', 'links']],
  m: [['pos', 'key', 'opt']], // chain link: member
  c: [['pos', 'args', 'opt'], ['e']], // chain link: call (positioned) | computed key (no pos)
  un: [['pos', 'op', 'e']],
  typeof: [['pos', 'e']],
  void: [['pos', 'e']],
  del: [['pos', 'o', 'key']],
  delv: [['pos', 'e']],
  bin: [['pos', 'op', 'l', 'r']],
  log: [['pos', 'op', 'l', 'r']],
  cond: [['pos', 't', 'c', 'a']],
  seq: [['pos', 'es']],
  asgn: [['pos', 'op', 't', 'v']],
  asgnp: [['pos', 'pat', 'v']],
  logasgn: [['pos', 'op', 't', 'v']],
  upd: [['pos', 'prefix', 'inc', 't']],
  tmpl: [['pos', 'parts']],
  // targets and patterns
  tvar: [['pos', 'r', 'name', 'tdz', 'ro']],
  tglobal: [['pos', 'name']],
  tmem: [['pos', 'o', 'key']],
  pinit: [['pos', 'r', 'name', 'tdz'], ['id', 'pos', 'r', 'name', 'tdz'], ['id', 'r', 'name', 'tdz']],
  pobj: [['pos', 'props', 'rest']],
  parr: [['pos', 'elems', 'rest']],
  target: [['t']], // for-in/of head that is not a declaration (no pos)
  // statements
  expr: [['pos', 'e']],
  decl: [['pos', 't', 'v'], ['t']], // declaration (positioned) | for-in/of head declaration (no pos)
  seqdecl: [['pos', 'list']],
  block: [['pos', 'scope', 'body']],
  if: [['pos', 't', 'c', 'a']],
  empty: [['pos']],
  while: [['pos', 'lid', 't', 'body']],
  dowhile: [['pos', 'lid', 't', 'body']],
  for: [['pos', 'lid', 'scope', 'init', 't', 'upd', 'body']],
  forof: [['pos', 'lid', 'scope', 'left', 'right', 'body']],
  forin: [['pos', 'lid', 'scope', 'left', 'right', 'body']],
  switch: [['pos', 'lid', 'disc', 'scope', 'cases']],
  try: [['pos', 'block', 'handler', 'fin']],
  ret: [['pos', 'e']],
  throw: [['pos', 'e', 'site']],
  break: [['pos', 'lid']],
  continue: [['pos', 'lid']],
  label: [['pos', 'lid', 'body']],
  // references (where a value lives, seen from the current function) and property keys
  L: [['id']],
  C: [['id']],
  K: [['i']],
  M: [['id']],
  I: [['id']],
  s: [['s']],
});

export const HIR_RECORDS = Object.freeze({
  Module: [['body', 'exportsIndirect', 'exportsLocal', 'exportsNs', 'exportsStar', 'fns', 'hirVersion', 'hoist',
    'imports', 'kind', 'lang', 'origin', 'requests', 'sites', 'slots', 'specifier']],
  Slot: [['id', 'init', 'kind', 'name', 'tdz'], ['id', 'init', 'kind', 'name', 'request']],
  Import: [['id', 'imported', 'name', 'request']],
  ExportLocal: [['id', 'name']],
  ExportIndirect: [['imported', 'name', 'request']],
  ExportNs: [['name', 'request']],
  Fn: [['arrow', 'async', 'body', 'caps', 'e', 'id', 'kind', 'length', 'name', 'nameFromKey', 'paramCopies', 'params',
    's', 'scope', 'selfB', 'selfName', 'selfSt', 'site', 'thisCell']],
  Site: [['column', 'file', 'kind', 'line', 'name']], // source sites (task 3.7)
  Scope: [['decls', 'hoist'], ['decls', 'hoist', 'perIter']],
  Decl: [['id', 'kind', 'name', 'st', 'tdz']],
  Hoist: [['fn', 'id', 'target']],
  Param: [['d', 'rest', 't']],
  ParamCopy: [['from', 'name', 'to']],
  PatternProp: [['d', 'key', 't']],
  PatternElem: [['d', 't']],
  SwitchCase: [['body', 't']],
  Handler: [['body', 'param', 'scope']],
});

// Canonical JSON: object keys sorted, no white space, arrays in order. The hash input is this text.
export function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (value !== null && typeof value === 'object') {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

export function hirSchemaHash(catalogue = { nodes: HIR_NODES, records: HIR_RECORDS }) {
  return createHash('sha256').update(canonicalJson(catalogue), 'utf8').digest('hex');
}

export const HIR_SCHEMA_HASH = hirSchemaHash();

// Every field set of the catalogue, as an id: node:<kind>#<index> or record:<name>#<index>.
export function catalogueEntryIds(catalogue = { nodes: HIR_NODES, records: HIR_RECORDS }) {
  const ids = [];
  for (const [kind, sets] of Object.entries(catalogue.nodes)) sets.forEach((_, i) => ids.push(`node:${kind}#${i}`));
  for (const [name, sets] of Object.entries(catalogue.records)) sets.forEach((_, i) => ids.push(`record:${name}#${i}`));
  return ids;
}

const sameFields = (set, keys) => set.length === keys.length && set.every((f) => keys.includes(f));

// Walks a module record (the output of lowerToHir). Returns { violations: [{ path, message }], met: Set of entry ids }.
// Shared objects are visited once. The walk is total: every object and array reachable from hir is looked at.
export function checkHirShape(hir, catalogue = { nodes: HIR_NODES, records: HIR_RECORDS }) {
  const violations = [];
  const met = new Set();
  const seen = new Set();
  const fieldsOf = (obj) => Object.keys(obj).filter((key) => key !== 'k').sort();

  const visit = (value, path) => {
    if (value === null || typeof value !== 'object' || seen.has(value)) return;
    seen.add(value);
    if (Array.isArray(value)) {
      value.forEach((item, i) => {
        if (i in value) visit(item, `${path}[${i}]`);
      });
      return;
    }
    if (typeof value.k === 'string') {
      const keys = fieldsOf(value);
      if (!Object.hasOwn(catalogue.nodes, value.k)) {
        violations.push({ path, message: `unknown node kind "${value.k}" with fields [${keys.join(', ')}]` });
      } else {
        const index = catalogue.nodes[value.k].findIndex((set) => sameFields(set, keys));
        if (index < 0) {
          violations.push({ path, message: `node "${value.k}" has fields [${keys.join(', ')}], which the catalogue does not list` });
        } else {
          met.add(`node:${value.k}#${index}`);
        }
      }
    } else {
      const keys = Object.keys(value).sort();
      let found = null;
      for (const [name, sets] of Object.entries(catalogue.records)) {
        const index = sets.findIndex((set) => sameFields(set, keys));
        if (index >= 0) {
          found = `record:${name}#${index}`;
          break;
        }
      }
      if (found === null) {
        violations.push({ path, message: `record with fields [${keys.join(', ')}] matches no catalogue record` });
      } else {
        met.add(found);
      }
    }
    for (const key of Object.keys(value)) {
      if (key !== 'pos') visit(value[key], `${path}.${key}`);
    }
  };

  visit(hir, '$');
  return { violations, met };
}

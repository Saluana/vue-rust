// Lowering context shared by the HIR modules (WP-C1b). A context knows the function being lowered (curFn, -1 for the
// module body), the scope index of the analysis, and how every binding is reached from curFn (resolveKey).
// Resolution kinds (see README.md): L local, C owned cell, K captured cell (index into the function's captures),
// M module cell, I import cell, and `thisval` for the own `this` of a non-arrow function.
import { createDiagnostic, createLineMap } from '../diagnostics/diagnostic.mjs';
import { buildTables } from './tables.mjs';

// Thrown to stop lowering at the first construct the lowering does not handle. Carries the diagnostic.
export class LowerAbort extends Error {
  constructor(diagnostic) {
    super(diagnostic.message);
    this.name = 'LowerAbort';
    this.diagnostic = diagnostic;
  }
}

export const HIR_VERSION = 1;

export function makeContext(ast, analysis, { source, filename }) {
  const program = ast.type === 'File' ? ast.program : ast;
  const lines = createLineMap(source);
  const scopeIndex = new Map();
  for (const s of analysis.scopes) scopeIndex.set(`${s.kind}:${s.start}:${s.end}`, s.id);
  const refByStart = new Map(analysis.references.map((r) => [r.start, r]));
  const fnIndex = new Map(analysis.functions.map((f) => [`${f.start}:${f.end}`, f.id]));
  const scopeNames = analysis.scopes.map((s) => {
    const m = new Map();
    for (const bid of s.bindingIds) m.set(analysis.bindings[bid].name, bid);
    return m;
  });
  const cx = {
    source,
    filename,
    program,
    analysis,
    bindings: analysis.bindings,
    scopes: analysis.scopes,
    fns: analysis.functions,
    refByStart,
    scopeIndex,
    fnIndex,
    scopeNames,
    tables: buildTables(analysis),
    lines,
    curFn: -1,
    labels: [],
    nextLid: 1,
    synthBindings: [],
    fnRecords: [],
    fnStack: [],
  };
  return cx;
}

export function unsupported(cx, node, what) {
  const start = node.start ?? 0;
  const end = node.end ?? start;
  const diag = createDiagnostic('VN9001', {
    message: `not lowered yet by the compiler core: ${what}`,
    hint: 'This construct passed the profile check but WP-C1b has no lowering for it. It is rejected, never skipped.',
    file: cx.filename,
    start: cx.lines.posAt(Math.min(start, cx.source.length)),
    end: cx.lines.posAt(Math.min(end, cx.source.length)),
  });
  throw new LowerAbort(diag);
}

export function internal(cx, node, what) {
  const start = node && node.start !== undefined ? node.start : 0;
  const end = node && node.end !== undefined ? node.end : start;
  const diag = createDiagnostic('VN9001', {
    message: `lowering invariant violated: ${what}`,
    file: cx.filename,
    start: cx.lines.posAt(Math.min(start, cx.source.length)),
    end: cx.lines.posAt(Math.min(end, cx.source.length)),
  });
  throw new LowerAbort(diag);
}

// HIR node with the source span `pos: [start, end]` (UTF-16 offsets) of the AST node it came from.
export function hn(kind, ast, props = {}) {
  return { k: kind, pos: ast ? [ast.start, ast.end] : [0, 0], ...props };
}

// The reference the analysis recorded for an identifier at this node (reads and writes share one record).
export function refOf(cx, node) {
  const ref = cx.refByStart.get(node.start);
  if (!ref || ref.name !== node.name) internal(cx, node, `no scope reference for '${node.name}'`);
  return ref;
}

// Scope id for an AST scope-creating node.
export function scopeIdOf(cx, kind, node) {
  const id = cx.scopeIndex.get(`${kind}:${node.start}:${node.end}`);
  if (id === undefined) internal(cx, node, `no ${kind} scope for this node`);
  return id;
}

// Binding id declared with `name` in the scope that owns the declaration (var: the function scope).
export function declaredBinding(cx, scopeId, name, node) {
  const bid = cx.scopeNames[scopeId].get(name);
  if (bid === undefined) internal(cx, node, `declaration '${name}' has no binding in its scope`);
  return bid;
}

// Scope id that holds the var declarations of the current function (or the module).
export function varScopeOf(cx, scopeId) {
  let s = scopeId;
  while (!['function', 'function-body', 'module'].includes(cx.scopes[s].kind)) s = cx.scopes[s].parentId;
  return s;
}

// Reference (resolution) of a capture key or binding, seen from the current function.
export function resolveKey(cx, key, node) {
  const id = Number(key.slice(2));
  if (key[0] === 't') {
    if (id === cx.curFn) return { k: 'C', id: `th${id}` };
    return capRef(cx, key, node);
  }
  const owner = cx.tables.ownerOf(id);
  const st = cx.tables.storageOf(id);
  if (st === 'import') return cx.curFn === -1 ? { k: 'I', id } : capRef(cx, key, node);
  if (st === 'module') return cx.curFn === -1 ? { k: 'M', id } : capRef(cx, key, node);
  if (owner === cx.curFn) return st === 'cell' ? { k: 'C', id } : { k: 'L', id };
  return capRef(cx, key, node);
}

export function resolveBinding(cx, bid, node) {
  return resolveKey(cx, `b:${bid}`, node);
}

function capRef(cx, key, node) {
  const i = cx.tables.captures(cx.curFn).indexOf(key);
  if (i < 0) internal(cx, node, `binding ${key} is not captured by function ${cx.curFn}`);
  return { k: 'K', i };
}

// The captured-cell references of a function creation, seen from the creating function.
export function captureRefs(cx, fnId, node) {
  return cx.tables.captures(fnId).map((key) => resolveKey(cx, key, node));
}

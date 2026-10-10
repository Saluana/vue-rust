// Function values (WP-C1b). A function is lowered once into `cx.fnRecords[id]` with its own scope, parameters, body and
// captures; the creation site (`fn` node) carries the captured references seen from the creating function. Name
// inference, `length` (parameters before the first default or rest), named-expression self bindings and the
// `this` cell of a non-arrow function that arrows read are decided here. An async function (WP-C1c) has `async: true`;
// its await expressions are `await` nodes.
import { hn, unsupported, internal, captureRefs, resolveBinding } from './context.mjs';
import { isThisParameter } from '../analysis/scope.mjs';
import { lowerDeclPattern } from './pattern.mjs';
import { lowerExpr, lowerNamed } from './expr.mjs';
import { lowerScopedBody, scopeEntry } from './stmt.mjs';
import { unwrapTs, isAnonymousFunction } from './literals.mjs';

function fnIdOf(cx, node) {
  const id = cx.fnIndex.get(`${node.start}:${node.end}`);
  if (id === undefined) internal(cx, node, 'function node has no analysis record');
  return id;
}

// Lowers a function node (FunctionDeclaration, FunctionExpression, ArrowFunctionExpression or an object method) and
// returns the creation node `fn` in the creating function's context. opts: name (string|null), decl, method,
// accessor ('get'|'set'|null), keyComputed (the name comes from a runtime key).
export function lowerFunctionValue(cx, node, opts = {}) {
  if (node.generator) unsupported(cx, node, 'generator function');
  const fid = fnIdOf(cx, node);
  const info = cx.fns[fid];
  const saved = { curFn: cx.curFn, labels: cx.labels, pendingLabels: cx.pendingLabels };
  cx.curFn = fid;
  cx.labels = [];
  cx.pendingLabels = [];
  let rec;
  try {
    rec = lowerFunctionBody(cx, node, fid, info, opts);
  } finally {
    cx.curFn = saved.curFn;
    cx.labels = saved.labels;
    cx.pendingLabels = saved.pendingLabels;
  }
  cx.fnRecords[fid] = rec;
  // The creation site lives in the creating function, so the captures are resolved there.
  return hn('fn', node, { f: fid, caps: captureRefs(cx, fid, node) });
}

function lowerFunctionBody(cx, node, fid, info, opts) {
  const fnScope = info.scopeId;
  const bodyScope = info.bodyScopeId ?? fnScope;
  const isArrow = info.isArrow;
  const params = [];
  let length = 0;
  let counting = true;
  for (const p of node.params) {
    // A TypeScript `this` parameter is erased (see isThisParameter): it is no parameter and counts for no length.
    if (isThisParameter(p)) continue;
    if (p.type === 'AssignmentPattern') {
      counting = false;
      const left = unwrapTs(p.left);
      const name = left.type === 'Identifier' ? left.name : null;
      const t = lowerDeclPattern(cx, p.left, fnScope);
      const d = name !== null && isAnonymousFunction(p.right) ? lowerNamed(cx, p.right, name) : lowerExpr(cx, p.right);
      params.push({ t, d, rest: false });
    } else if (p.type === 'RestElement') {
      counting = false;
      params.push({ t: lowerDeclPattern(cx, p.argument, fnScope), d: null, rest: true });
    } else {
      if (counting) length += 1;
      params.push({ t: lowerDeclPattern(cx, p, fnScope), d: null, rest: false });
    }
  }
  let scope;
  let body;
  if (node.body.type === 'BlockStatement') {
    ({ scope, body } = lowerScopedBody(cx, bodyScope, node.body.body));
  } else {
    if (!isArrow) internal(cx, node, 'non-arrow function with an expression body');
    scope = scopeEntry(cx, fnScope, []);
    body = [hn('ret', node.body, { e: lowerExpr(cx, node.body) })];
  }

  const paramCopies = [];
  if (bodyScope !== fnScope) {
    for (const d of scope.decls) {
      const bid = cx.scopeNames[fnScope].get(d.name);
      // Body vars start with the parameter value, but own a separate binding/cell.
      if (d.kind === 'var' && bid !== undefined) {
        paramCopies.push({ to: resolveBinding(cx, d.id, node), from: resolveBinding(cx, bid, node), name: d.name });
      }
    }
    scope.decls = [...scopeEntry(cx, fnScope, []).decls, ...scope.decls];
  }

  // A named function expression binds its own name in its funcname scope; that binding belongs to the function.
  let selfB = null;
  if (node.type === 'FunctionExpression' && node.id) {
    const nameScope = cx.scopes[fnScope].parentId;
    const bid = cx.scopeNames[nameScope].get(node.id.name);
    if (bid !== undefined) selfB = bid;
  }

  let kind = 'normal';
  if (isArrow) kind = 'arrow';
  else if (opts.accessor === 'get') kind = 'getter';
  else if (opts.accessor === 'set') kind = 'setter';
  else if (opts.method) kind = 'method';

  let name = opts.name ?? null;
  if (node.type === 'FunctionExpression' && node.id) name = node.id.name;
  if (name !== null && opts.accessor) name = `${opts.accessor} ${name}`;

  return {
    id: fid,
    name,
    nameFromKey: !!opts.keyComputed,
    length,
    kind,
    arrow: isArrow,
    s: node.start,
    e: node.end,
    caps: cx.tables.captures(fid),
    selfB,
    selfName: selfB === null ? null : node.id.name,
    selfSt: selfB === null ? null : cx.tables.storageOf(selfB),
    thisCell: !isArrow && cx.tables.thisOwned(fid),
    async: !!node.async,
    scope,
    params,
    paramCopies,
    body,
  };
}

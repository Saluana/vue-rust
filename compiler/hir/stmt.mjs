// Statement lowering (WP-C1b). A scope entry lists the bindings a block creates on entry (`decls`, with their storage)
// and the function declarations it instantiates before any statement runs (`hoist`). Labels: every loop, switch and
// labelled statement gets a module-unique `lid`; break/continue name the lid they leave, and the emitter decides how.
import { hn, unsupported, scopeIdOf, declaredBinding, varScopeOf, resolveBinding, siteOf } from './context.mjs';
import { lowerNamed, lowerExpr, lowerTarget } from './expr.mjs';
import { lowerDeclPattern, lowerAssignPattern } from './pattern.mjs';
import { lowerFunctionValue } from './fn.mjs';

const LOOPS = new Set(['WhileStatement', 'DoWhileStatement', 'ForStatement', 'ForInStatement', 'ForOfStatement']);
const SKIPPED = new Set(['TSTypeAliasDeclaration', 'TSInterfaceDeclaration', 'TSDeclareFunction', 'EmptyStatement',
  'TSNamespaceExportDeclaration']);

// Declarations a scope creates: its bindings (cells or locals) and its hoisted function declarations. Anonymous
// `export default function () {}` is not here: the module lowering gives it a hidden binding.
export function scopeEntry(cx, scopeId, stmts) {
  const decls = [];
  for (const bid of cx.scopes[scopeId].bindingIds) {
    const b = cx.bindings[bid];
    if (b.kind === 'import') continue;
    decls.push({ id: bid, name: b.name, kind: b.kind, st: cx.tables.storageOf(bid), tdz: b.needsTdzCheck });
  }
  const hoist = [];
  for (const s of stmts) {
    const decl = functionDeclOf(s);
    if (decl && decl.id) hoist.push(hoistEntry(cx, decl, scopeId));
  }
  return { decls, hoist };
}

// The FunctionDeclaration a statement declares (plain or exported), or null.
export function functionDeclOf(s) {
  if (s.type === 'FunctionDeclaration') return s.declare ? null : s;
  if ((s.type === 'ExportNamedDeclaration' || s.type === 'ExportDefaultDeclaration') && s.declaration &&
    s.declaration.type === 'FunctionDeclaration') return s.declaration.declare ? null : s.declaration;
  return null;
}

// A named function declaration: its closure is created at scope entry and initialises the binding.
function hoistEntry(cx, fnNode, scopeId) {
  const bid = declaredBinding(cx, scopeId, fnNode.id.name, fnNode);
  return { id: bid, target: initRef(cx, bid, fnNode), fn: lowerFunctionValue(cx, fnNode, { name: fnNode.id.name, decl: true }) };
}

// The initialisation target of a binding (declaration initialisation, no TDZ or const check).
export function initRef(cx, bid, node) {
  const b = cx.bindings[bid];
  return hn('pinit', node, { id: bid, r: resolveBinding(cx, bid, node), tdz: b.needsTdzCheck, name: b.name });
}

// Statement list of a block-like body with its scope.
export function lowerScopedBody(cx, scopeId, stmts) {
  const scope = scopeEntry(cx, scopeId, stmts);
  const body = [];
  for (const s of stmts) {
    if (s.type === 'FunctionDeclaration' || (s.type === 'ExportNamedDeclaration' && s.declaration &&
      s.declaration.type === 'FunctionDeclaration')) continue;
    const out = lowerStatement(cx, s, scopeId);
    if (out !== null) body.push(out);
  }
  return { scope, body };
}

function withLabels(cx, names) {
  const pending = cx.pendingLabels || [];
  cx.pendingLabels = [];
  return [...pending, ...names];
}

function pushLabel(cx, entry) {
  cx.labels.push(entry);
  return entry;
}

function popLabel(cx) {
  cx.labels.pop();
}

function newLid(cx) {
  return cx.nextLid++;
}

// Lowers one statement; scopeId is the innermost scope the statement is in.
export function lowerStatement(cx, s, scopeId) {
  if (SKIPPED.has(s.type)) return null;
  if (s.declare) return null;
  switch (s.type) {
    case 'ExpressionStatement':
      return hn('expr', s, { e: lowerExpr(cx, s.expression) });
    case 'VariableDeclaration':
      return lowerVarDecl(cx, s, scopeId);
    case 'BlockStatement': {
      const bodyScope = scopeIdOf(cx, 'block', s);
      const { scope, body } = lowerScopedBody(cx, bodyScope, s.body);
      return hn('block', s, { scope, body });
    }
    case 'IfStatement':
      return hn('if', s, {
        t: lowerExpr(cx, s.test),
        c: lowerBranch(cx, s.consequent, scopeId),
        a: s.alternate ? lowerBranch(cx, s.alternate, scopeId) : null,
      });
    case 'WhileStatement': {
      const { lid, pushed } = loopEntry(cx);
      const t = lowerExpr(cx, s.test);
      const body = lowerBranch(cx, s.body, scopeId);
      if (pushed) popLabel(cx);
      return hn('while', s, { lid, t, body });
    }
    case 'DoWhileStatement': {
      const { lid, pushed } = loopEntry(cx);
      const body = lowerBranch(cx, s.body, scopeId);
      const t = lowerExpr(cx, s.test);
      if (pushed) popLabel(cx);
      return hn('dowhile', s, { lid, t, body });
    }
    case 'ForStatement':
      return lowerFor(cx, s);
    case 'ForInStatement':
    case 'ForOfStatement':
      return lowerForInOf(cx, s, scopeId);
    case 'SwitchStatement':
      return lowerSwitch(cx, s, scopeId);
    case 'TryStatement':
      return lowerTry(cx, s, scopeId);
    case 'ReturnStatement':
      return hn('ret', s, { e: s.argument ? lowerExpr(cx, s.argument) : null });
    case 'ThrowStatement':
      return hn('throw', s, { e: lowerExpr(cx, s.argument), site: siteOf(cx, 'throw', null, s.start) });
    case 'BreakStatement':
      return hn('break', s, { lid: resolveJump(cx, s, s.label, 'break') });
    case 'ContinueStatement':
      return hn('continue', s, { lid: resolveJump(cx, s, s.label, 'continue') });
    case 'LabeledStatement':
      return lowerLabeled(cx, s, scopeId);
    case 'ClassDeclaration':
      return unsupported(cx, s, 'class declaration');
    case 'ExportNamedDeclaration':
    case 'ExportDefaultDeclaration':
    case 'ImportDeclaration':
    case 'ExportAllDeclaration':
      return unsupported(cx, s, 'module declaration in a nested position');
    default:
      return unsupported(cx, s, `statement ${s.type}`);
  }
}

// A statement in a branch position: a block keeps its own scope, anything else lowers directly.
function lowerBranch(cx, s, scopeId) {
  if (s.type === 'BlockStatement') return lowerStatement(cx, s, scopeId);
  const out = lowerStatement(cx, s, scopeId);
  return out ?? hn('empty', s);
}

function loopEntry(cx) {
  const names = withLabels(cx, []);
  const lid = newLid(cx);
  pushLabel(cx, { lid, kind: 'loop', names });
  return { lid, pushed: true };
}

function resolveJump(cx, node, label, what) {
  const name = label ? label.name : null;
  for (let i = cx.labels.length - 1; i >= 0; i--) {
    const e = cx.labels[i];
    if (name === null) {
      if (e.kind === 'loop' || (what === 'break' && e.kind === 'switch')) return e.lid;
    } else if (e.names.includes(name)) {
      if (what === 'continue' && e.kind !== 'loop') break;
      return e.lid;
    }
  }
  if (name !== null) return unsupported(cx, node, `${what} to unknown label '${name}'`);
  return unsupported(cx, node, `${what} outside a loop`);
}

function lowerLabeled(cx, s, scopeId) {
  const body = s.body;
  if (LOOPS.has(body.type)) {
    cx.pendingLabels = [...(cx.pendingLabels || []), s.label.name];
    return lowerStatement(cx, body, scopeId);
  }
  const lid = newLid(cx);
  pushLabel(cx, { lid, kind: 'block', names: [s.label.name] });
  const inner = lowerBranch(cx, body, scopeId);
  popLabel(cx);
  return hn('label', s, { lid, body: inner });
}

function lowerVarDecl(cx, s, scopeId) {
  const out = [];
  const isVar = s.kind === 'var';
  const declScope = isVar ? varScopeOf(cx, scopeId) : scopeId;
  for (const d of s.declarations) {
    if (d.id.type === 'Identifier') {
      const name = d.id.name;
      if (!d.init && isVar) continue; // `var x;` changes nothing
      const t = lowerDeclPattern(cx, d.id, declScope);
      const v = d.init ? lowerNamed(cx, d.init, name) : hn('undef', d.id);
      out.push(hn('decl', d, { t, v }));
    } else {
      const t = lowerDeclPattern(cx, d.id, declScope);
      out.push(hn('decl', d, { t, v: d.init ? lowerExpr(cx, d.init) : hn('undef', d.id) }));
    }
  }
  if (out.length === 1) return out[0];
  return hn('seqdecl', s, { list: out });
}

function lowerFor(cx, s) {
  const forScope = scopeIdOf(cx, 'for', s);
  const names = withLabels(cx, []);
  const lid = newLid(cx);
  const scope = scopeEntry(cx, forScope, []);
  const perIter = scope.decls.filter((d) => d.st === 'cell' && (d.kind === 'let')).map((d) => d.id);
  pushLabel(cx, { lid, kind: 'loop', names });
  let init = null;
  if (s.init) {
    init = s.init.type === 'VariableDeclaration' ? lowerVarDecl(cx, s.init, forScope) : hn('expr', s.init, { e: lowerExpr(cx, s.init) });
  }
  const t = s.test ? lowerExpr(cx, s.test) : null;
  const upd = s.update ? lowerExpr(cx, s.update) : null;
  const body = lowerBranch(cx, s.body, forScope);
  popLabel(cx);
  return hn('for', s, { lid, scope: { decls: scope.decls, hoist: [], perIter }, init, t, upd, body });
}

function lowerForInOf(cx, s, scopeId) {
  const isOf = s.type === 'ForOfStatement';
  if (s.await) unsupported(cx, s, 'for await loop (not supported: async iteration is outside the profile)');
  const forScope = scopeIdOf(cx, 'for', s);
  const names = withLabels(cx, []);
  const lid = newLid(cx);
  const right = lowerExpr(cx, s.right);
  const scopeInfo = scopeEntry(cx, forScope, []);
  pushLabel(cx, { lid, kind: 'loop', names });
  let left;
  const L = s.left;
  if (L.type === 'VariableDeclaration') {
    const d = L.declarations[0];
    const isVar = L.kind === 'var';
    const declScope = isVar ? varScopeOf(cx, scopeId) : forScope;
    left = { k: 'decl', t: lowerDeclPattern(cx, d.id, declScope) };
  } else {
    const pat = L.type === 'ObjectPattern' || L.type === 'ArrayPattern' ? lowerAssignPattern(cx, L) : lowerTarget(cx, L);
    left = { k: 'target', t: pat };
  }
  const body = lowerBranch(cx, s.body, forScope);
  popLabel(cx);
  return hn(isOf ? 'forof' : 'forin', s, {
    lid,
    scope: { decls: scopeInfo.decls, hoist: [] },
    left,
    right,
    body,
  });
}

function lowerSwitch(cx, s, scopeId) {
  const disc = lowerExpr(cx, s.discriminant);
  const switchScope = scopeIdOf(cx, 'switch', s);
  const allStmts = s.cases.flatMap((c) => c.consequent);
  const scope = scopeEntry(cx, switchScope, allStmts);
  const lid = newLid(cx);
  const names = withLabels(cx, []);
  pushLabel(cx, { lid, kind: 'switch', names });
  const cases = s.cases.map((c) => {
    const t = c.test ? lowerExpr(cx, c.test) : null;
    const body = [];
    for (const st of c.consequent) {
      if (st.type === 'FunctionDeclaration') continue;
      const out = lowerStatement(cx, st, switchScope);
      if (out !== null) body.push(out);
    }
    return { t, body };
  });
  popLabel(cx);
  return hn('switch', s, { lid, disc, scope, cases });
}

function lowerTry(cx, s, scopeId) {
  const block = lowerStatement(cx, s.block, scopeId);
  let handler = null;
  if (s.handler) {
    const catchScope = scopeIdOf(cx, 'catch', s.handler);
    const stmts = s.handler.body.body;
    const param = s.handler.param ? lowerDeclPattern(cx, s.handler.param, catchScope) : null;
    const { scope, body } = lowerScopedBody(cx, catchScope, stmts);
    handler = { scope, param, body };
  }
  const fin = s.finalizer ? lowerStatement(cx, s.finalizer, scopeId) : null;
  return hn('try', s, { block, handler, fin });
}

// Module-level and function-level bodies: a plain statement list in a scope.
export function lowerBody(cx, scopeId, stmts) {
  return lowerScopedBody(cx, scopeId, stmts);
}


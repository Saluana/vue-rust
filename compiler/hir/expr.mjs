// Expression lowering (WP-C1b): AST expressions to HIR expressions. Sugar is removed here: template literals become
// parts, optional chains become one `chain` node, compound and logical assignment keep their operator, and identifiers
// are resolved through the analysis reference. Destructuring is in pattern.mjs; function values in fn.mjs.
import { hn, unsupported, internal, refOf, resolveBinding, resolveKey } from './context.mjs';
import { numberText, staticKeyName, unwrapTs, isAnonymousFunction, unitsOf } from './literals.mjs';
import { lowerFunctionValue } from './fn.mjs';
import { lowerAssignPattern } from './pattern.mjs';

const BIN_OPS = new Set(['+', '-', '*', '/', '%', '**', '<<', '>>', '>>>', '&', '|', '^', '==', '!=', '===', '!==',
  '<', '>', '<=', '>=', 'in', 'instanceof']);
const LOG_OPS = new Set(['&&', '||', '??']);
const UN_OPS = new Set(['-', '+', '!', '~']);

// The value of a name in an expression position (reads).
export function identExpr(cx, n) {
  const ref = refOf(cx, n);
  if (ref.binding < 0) {
    if (n.name === 'undefined') return hn('undef', n);
    if (n.name === 'NaN' || n.name === 'Infinity') return hn('num', n, { s: n.name });
    return hn('gvar', n, { name: n.name });
  }
  const b = cx.bindings[ref.binding];
  return hn('var', n, { r: resolveBinding(cx, ref.binding, n), tdz: b.needsTdzCheck, name: n.name });
}

function thisExpr(cx, n) {
  if (cx.curFn === -1) return hn('undef', n);
  const f = cx.fns[cx.curFn];
  if (!f.isArrow) {
    if (cx.tables.thisOwned(cx.curFn)) return hn('thiscell', n, { r: { k: 'C', id: `th${cx.curFn}` } });
    return hn('thisval', n);
  }
  if (f.lexicalThisFnId < 0) return hn('undef', n);
  return hn('thiscell', n, { r: resolveKey(cx, `t:${f.lexicalThisFnId}`, n) });
}

// Member key: a static name (already ToPropertyKey-normalised text) or an expression.
function memberKey(cx, prop, computed) {
  if (!computed) {
    if (prop.type !== 'Identifier') unsupported(cx, prop, 'member name form');
    return { k: 's', s: prop.name };
  }
  if (prop.type === 'StringLiteral') return { k: 's', s: prop.value };
  if (prop.type === 'NumericLiteral') return { k: 's', s: numberText(prop.value) };
  return { k: 'c', e: lowerExpr(cx, prop) };
}

// Key of an object literal member.
function objectKey(cx, p) {
  if (!p.computed) {
    const s = staticKeyName(p.key);
    if (s === null) unsupported(cx, p.key, 'object key form');
    return { k: 's', s };
  }
  return { k: 'c', e: lowerExpr(cx, p.key) };
}

// The value of an expression that may take a name from its binding (`const f = () => {}`).
export function lowerNamed(cx, valueNode, name) {
  const n = unwrapTs(valueNode);
  if (name !== null && isAnonymousFunction(n)) return lowerFunctionValue(cx, n, { name });
  return lowerExpr(cx, valueNode);
}

function lowerArgs(cx, args) {
  return args.map((a) => (a.type === 'SpreadElement'
    ? hn('spread', a, { e: lowerExpr(cx, a.argument) })
    : lowerExpr(cx, a)));
}

function lowerArray(cx, n) {
  const elems = n.elements.map((el) => {
    if (el === null) return hn('hole', n);
    if (el.type === 'SpreadElement') return hn('spread', el, { e: lowerExpr(cx, el.argument) });
    return lowerExpr(cx, el);
  });
  return hn('arr', n, { elems });
}

function lowerObject(cx, n) {
  const props = [];
  for (const p of n.properties) {
    if (p.type === 'SpreadElement') {
      props.push(hn('spread', p, { e: lowerExpr(cx, p.argument) }));
      continue;
    }
    if (p.type === 'ObjectMethod') {
      const key = objectKey(cx, p);
      const kind = p.kind === 'get' ? 'get' : p.kind === 'set' ? 'set' : 'init';
      const name = key.k === 's' ? key.s : null;
      const fnv = lowerFunctionValue(cx, p, { name, method: true, accessor: kind === 'init' ? null : kind, keyComputed: key.k === 'c' });
      props.push(hn(kind, p, { key, v: fnv }));
      continue;
    }
    if (p.type !== 'ObjectProperty') unsupported(cx, p, `object member ${p.type}`);
    const key = objectKey(cx, p);
    if (p.shorthand) {
      props.push(hn('init', p, { key, v: identExpr(cx, p.value) }));
      continue;
    }
    const anon = isAnonymousFunction(p.value);
    if (anon && key.k === 'c') {
      props.push(hn('init', p, { key, v: lowerFunctionValue(cx, unwrapTs(p.value), { name: null, keyComputed: true }), nameFromKey: true }));
    } else {
      props.push(hn('init', p, { key, v: anon ? lowerNamed(cx, p.value, key.s) : lowerExpr(cx, p.value) }));
    }
  }
  return hn('obj', n, { props });
}

// Target of `=`, compound and update forms, and of for-in/of heads (not destructuring patterns).
export function lowerTarget(cx, n) {
  const t = unwrapTs(n);
  if (t.type === 'Identifier') {
    const ref = refOf(cx, t);
    if (ref.binding < 0) return hn('tglobal', t, { name: t.name });
    const b = cx.bindings[ref.binding];
    let ro = null;
    if (b.kind === 'const') ro = 'const';
    else if (b.kind === 'import') ro = 'import';
    else if (b.selfName) ro = 'self';
    return hn('tvar', t, { r: resolveBinding(cx, ref.binding, t), tdz: b.needsTdzCheck, name: t.name, ro });
  }
  if (t.type === 'MemberExpression' || t.type === 'OptionalMemberExpression') {
    if (t.type === 'OptionalMemberExpression') unsupported(cx, t, 'optional chain as assignment target');
    return hn('tmem', t, { o: lowerExpr(cx, t.object), key: memberKey(cx, t.property, t.computed) });
  }
  unsupported(cx, t, `assignment target ${t.type}`);
  return null;
}

function lowerAssign(cx, n) {
  const left = unwrapTs(n.left);
  if (n.operator === '=' && (left.type === 'ObjectPattern' || left.type === 'ArrayPattern')) {
    return hn('asgnp', n, { pat: lowerAssignPattern(cx, left), v: lowerExpr(cx, n.right) });
  }
  if (LOG_OPS.has(n.operator.slice(0, -1)) && n.operator.endsWith('=') && n.operator.length === 3) {
    const t = lowerTarget(cx, left);
    return hn('logasgn', n, { op: n.operator.slice(0, 2), t, v: lowerNamedTarget(cx, left, n.right) });
  }
  const t = lowerTarget(cx, left);
  if (n.operator === '=') return hn('asgn', n, { op: '=', t, v: lowerNamedTarget(cx, left, n.right) });
  const op = n.operator.slice(0, -1);
  if (!BIN_OPS.has(op)) unsupported(cx, n, `assignment operator ${n.operator}`);
  return hn('asgn', n, { op, t, v: lowerExpr(cx, n.right) });
}

// `x = function () {}` names the function after the identifier; member targets do not name it.
function lowerNamedTarget(cx, left, right) {
  const l = unwrapTs(left);
  if (l.type === 'Identifier') return lowerNamed(cx, right, l.name);
  return lowerExpr(cx, right);
}

function lowerUpdate(cx, n) {
  const t = lowerTarget(cx, n.argument);
  return hn('upd', n, { prefix: !!n.prefix, inc: n.operator === '++', t });
}

// Optional chain: the whole chain short-circuits to undefined. Links are in evaluation order.
function lowerChain(cx, n) {
  const links = [];
  let cur = n;
  for (;;) {
    if (cur.type === 'OptionalMemberExpression') {
      links.push({ n: cur, member: true });
      cur = cur.object;
    } else if (cur.type === 'OptionalCallExpression') {
      links.push({ n: cur, member: false });
      cur = cur.callee;
    } else if (cur.type === 'MemberExpression' && links.length > 0 && !links[links.length - 1].member) {
      // `a.b?.()`: the callee member gives `this` to the call link, so it joins the chain.
      links.push({ n: cur, member: true });
      cur = cur.object;
    } else {
      break;
    }
  }
  links.reverse();
  const base = lowerExpr(cx, cur);
  const out = links.map(({ n: l, member }) => {
    if (member) return hn('m', l, { key: memberKey(cx, l.property, l.computed), opt: !!l.optional });
    return hn('c', l, { args: lowerArgs(cx, l.arguments), opt: !!l.optional });
  });
  return hn('chain', n, { base, links: out });
}

function lowerCall(cx, n) {
  if (n.callee.type === 'Import') unsupported(cx, n, 'import() (needs promises, WP-C1c)');
  const callee = unwrapTs(n.callee);
  if (callee.type === 'OptionalMemberExpression' || callee.type === 'OptionalCallExpression') {
    // (a?.b)() keeps the reference of the chain: `this` is the object of the last member link.
    return hn('call', n, { f: lowerChain(cx, callee), args: lowerArgs(cx, n.arguments), chainCallee: true });
  }
  return hn('call', n, { f: lowerExpr(cx, n.callee), args: lowerArgs(cx, n.arguments) });
}

function lowerUnary(cx, n) {
  const op = n.operator;
  if (op === 'typeof') return hn('typeof', n, { e: lowerExpr(cx, n.argument) });
  if (op === 'void') return hn('void', n, { e: lowerExpr(cx, n.argument) });
  if (op === 'delete') {
    const a = unwrapTs(n.argument);
    if (a.type === 'MemberExpression') {
      return hn('del', n, { o: lowerExpr(cx, a.object), key: memberKey(cx, a.property, a.computed) });
    }
    if (a.type === 'Identifier') unsupported(cx, n, 'delete of a variable name');
    if (a.type === 'OptionalMemberExpression') unsupported(cx, n, 'delete of an optional chain');
    return hn('delv', n, { e: lowerExpr(cx, a) });
  }
  if (!UN_OPS.has(op)) unsupported(cx, n, `unary operator ${op}`);
  return hn('un', n, { op, e: lowerExpr(cx, n.argument) });
}

function lowerTemplate(cx, n) {
  const parts = [];
  n.quasis.forEach((q, i) => {
    if (q.value.cooked === null || q.value.cooked === undefined) unsupported(cx, q, 'invalid escape in template');
    if (q.value.cooked.length > 0 || n.expressions.length === 0) {
      parts.push(hn('str', q, { u: unitsOf(q.value.cooked) }));
    }
    if (i < n.expressions.length) parts.push(lowerExpr(cx, n.expressions[i]));
  });
  return hn('tmpl', n, { parts });
}

// Lowers one expression node.
export function lowerExpr(cx, n) {
  switch (n.type) {
    case 'Identifier':
      return identExpr(cx, n);
    case 'StringLiteral':
      return hn('str', n, { u: unitsOf(n.value) });
    case 'NumericLiteral':
      return hn('num', n, { s: numberText(n.value) });
    case 'BooleanLiteral':
      return hn('bool', n, { v: n.value });
    case 'NullLiteral':
      return hn('null', n);
    case 'TemplateLiteral':
      return lowerTemplate(cx, n);
    case 'ThisExpression':
      return thisExpr(cx, n);
    case 'ArrayExpression':
      return lowerArray(cx, n);
    case 'ObjectExpression':
      return lowerObject(cx, n);
    case 'FunctionExpression':
    case 'ArrowFunctionExpression':
      return lowerFunctionValue(cx, n, { name: n.type === 'FunctionExpression' && n.id ? n.id.name : null });
    case 'UnaryExpression':
      return lowerUnary(cx, n);
    case 'UpdateExpression':
      return lowerUpdate(cx, n);
    case 'BinaryExpression': {
      if (!BIN_OPS.has(n.operator)) unsupported(cx, n, `binary operator ${n.operator}`);
      return hn('bin', n, { op: n.operator, l: lowerExpr(cx, n.left), r: lowerExpr(cx, n.right) });
    }
    case 'LogicalExpression':
      return hn('log', n, { op: n.operator, l: lowerExpr(cx, n.left), r: lowerExpr(cx, n.right) });
    case 'AssignmentExpression':
      return lowerAssign(cx, n);
    case 'ConditionalExpression':
      return hn('cond', n, { t: lowerExpr(cx, n.test), c: lowerExpr(cx, n.consequent), a: lowerExpr(cx, n.alternate) });
    case 'SequenceExpression':
      return hn('seq', n, { es: n.expressions.map((e) => lowerExpr(cx, e)) });
    case 'CallExpression':
      return lowerCall(cx, n);
    case 'NewExpression':
      return hn('new', n, { f: lowerExpr(cx, n.callee), args: lowerArgs(cx, n.arguments) });
    case 'MemberExpression':
      return hn('mem', n, { o: lowerExpr(cx, n.object), key: memberKey(cx, n.property, n.computed) });
    case 'OptionalMemberExpression':
    case 'OptionalCallExpression':
      return lowerChain(cx, n);
    case 'TSAsExpression':
    case 'TSSatisfiesExpression':
    case 'TSNonNullExpression':
    case 'TSTypeAssertion':
    case 'TSInstantiationExpression':
    case 'ParenthesizedExpression':
      return lowerExpr(cx, n.expression);
    case 'AwaitExpression':
      return unsupported(cx, n, 'await (WP-C1c)');
    case 'TaggedTemplateExpression':
      return unsupported(cx, n, 'tagged template');
    case 'ClassExpression':
      return unsupported(cx, n, 'class expression');
    case 'YieldExpression':
      return unsupported(cx, n, 'yield');
    case 'MetaProperty':
      return unsupported(cx, n, 'meta property');
    case 'ImportExpression':
      return unsupported(cx, n, 'dynamic import (WP-C1c)');
    default:
      return internal(cx, n, `expression node ${n.type} reached the lowering`);
  }
}

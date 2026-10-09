// Destructuring patterns (WP-C1b). Init mode is a declaration (let/const/var/params/catch): leaves are `pinit`, which
// initialise the binding. Assign mode is an assignment expression: leaves are ordinary targets (`tvar`, `tglobal`,
// `tmem`). Object patterns are `pobj` (props + rest), array patterns `parr` (elements with holes + rest). An element
// with a default is { t, d }; the default is evaluated only when the value is undefined.
import { hn, unsupported, declaredBinding, resolveBinding } from './context.mjs';
import { staticKeyName, unwrapTs, isAnonymousFunction } from './literals.mjs';
import { lowerTarget, lowerNamed, lowerExpr } from './expr.mjs';

function leaf(cx, node, mode, scopeId) {
  if (node.type === 'Identifier') {
    if (mode === 'init') {
      const bid = declaredBinding(cx, scopeId, node.name, node);
      const b = cx.bindings[bid];
      return hn('pinit', node, { r: resolveBinding(cx, bid, node), tdz: b.needsTdzCheck, name: node.name });
    }
    return lowerTarget(cx, node);
  }
  if (mode === 'assign' && (node.type === 'MemberExpression')) return lowerTarget(cx, node);
  return unsupported(cx, node, `binding pattern ${node.type}`);
}

// Element of a pattern: a target with an optional default. `name` is the binding name for function name inference.
function element(cx, node, mode, scopeId) {
  if (node.type === 'AssignmentPattern') {
    const left = unwrapTs(node.left);
    const name = left.type === 'Identifier' ? left.name : null;
    const t = patternNode(cx, node.left, mode, scopeId);
    const d = name !== null && isAnonymousFunction(node.right)
      ? lowerNamed(cx, node.right, name)
      : lowerExpr(cx, node.right);
    return { t, d };
  }
  return { t: patternNode(cx, node, mode, scopeId), d: null };
}

function patternNode(cx, node, mode, scopeId) {
  switch (node.type) {
    case 'Identifier':
    case 'MemberExpression':
      return leaf(cx, node, mode, scopeId);
    case 'ObjectPattern': {
      const props = [];
      let rest = null;
      for (const p of node.properties) {
        if (p.type === 'RestElement') {
          rest = patternNode(cx, p.argument, mode, scopeId);
          continue;
        }
        if (p.type !== 'ObjectProperty') unsupported(cx, p, `object pattern member ${p.type}`);
        let key;
        if (p.computed) key = { k: 'c', e: lowerExpr(cx, p.key) };
        else {
          const s = staticKeyName(p.key);
          if (s === null) unsupported(cx, p.key, 'pattern key form');
          key = { k: 's', s };
        }
        const el = element(cx, p.value, mode, scopeId);
        props.push({ key, t: el.t, d: el.d });
      }
      return hn('pobj', node, { props, rest });
    }
    case 'ArrayPattern': {
      const elems = [];
      let rest = null;
      for (const el of node.elements) {
        if (el === null) { elems.push(null); continue; }
        if (el.type === 'RestElement') {
          rest = patternNode(cx, el.argument, mode, scopeId);
          continue;
        }
        elems.push(element(cx, el, mode, scopeId));
      }
      return hn('parr', node, { elems, rest });
    }
    default:
      return unsupported(cx, node, `pattern ${node.type}`);
  }
}

// Declaration pattern (init mode). scopeId is the scope that declares the names.
export function lowerDeclPattern(cx, node, scopeId) {
  return patternNode(cx, node, 'init', scopeId);
}

// Assignment pattern (assign mode, `[a, b] = value`).
export function lowerAssignPattern(cx, node) {
  return patternNode(cx, node, 'assign', -1);
}



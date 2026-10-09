// Literal values and property keys as the HIR carries them (WP-C1b). Numbers keep JavaScript's own ToString text
// (String(n), or "-0"), so the emitter writes the exact double. Strings keep their UTF-16 code units, lone surrogates
// included.

export function numberText(v) {
  if (Object.is(v, -0)) return '-0';
  return String(v);
}

export function unitsOf(s) {
  const out = new Array(s.length);
  for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
  return out;
}

// Static name of a non-computed property key or a literal computed-free key (ToPropertyKey result), else null.
export function staticKeyName(node) {
  if (!node) return null;
  if (node.type === 'Identifier') return node.name;
  if (node.type === 'StringLiteral') return node.value;
  if (node.type === 'NumericLiteral') return numberText(node.value);
  return null;
}

// Function expressions and arrows without a name: the ones that take their name from the context.
export function unwrapTs(node) {
  let n = node;
  while (n && (n.type === 'TSAsExpression' || n.type === 'TSSatisfiesExpression' ||
    n.type === 'TSNonNullExpression' || n.type === 'TSTypeAssertion' || n.type === 'ParenthesizedExpression')) {
    n = n.expression;
  }
  return n;
}

export function isAnonymousFunction(node) {
  const n = unwrapTs(node);
  if (!n) return false;
  if (n.type === 'ArrowFunctionExpression') return true;
  return n.type === 'FunctionExpression' && !n.id;
}

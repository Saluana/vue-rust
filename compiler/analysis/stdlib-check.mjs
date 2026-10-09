// Manifest-backed global and member checks (compiler/profile/stdlib.json through compiler/profile/stdlib.mjs). The
// profile checker runs these only when called with { stdlib: true }.
//   VN4016  a free global the target does not provide: excluded (X or E), target-restricted, or not in the manifest.
//   VN4017  a standard-library member path listed in the manifest that the target does not support.
// Only references that the scope analysis resolved to no binding are judged, so shadowed names are ignored. A global
// used only as a typeof operand is exempt (a closed-world program reads it as undefined). Computed members cannot be
// named statically and are ignored. Optional member chains are judged like plain ones.
import { resolveGlobal, isStdlibSupported, lookupStdlib, memberPath } from '../profile/stdlib.mjs';
import { createDiagnostic, createLineMap } from '../diagnostics/diagnostic.mjs';

const TARGETS = new Set(['native', 'web', 'test']);
const STDLIB_KINDS = ['global', 'ctor', 'static', 'proto', 'const', 'symbol', 'prop'];
const SUPPORTED = new Set(['P', 'A', 'W']);
const IDENTIFIER = /^[A-Za-z_$][A-Za-z0-9_$]*$/;
const SKIP_KEYS = new Set([
  'leadingComments', 'trailingComments', 'innerComments', 'comments', 'tokens', 'errors', 'loc', 'extra',
]);

function isNode(value) {
  return value !== null && typeof value === 'object' && typeof value.type === 'string';
}

function isMemberNode(node) {
  return node.type === 'MemberExpression' || node.type === 'OptionalMemberExpression';
}

// memberPath accepts plain MemberExpression chains; optional chains are rewritten to that shape first.
function asPlainMember(node) {
  if (isMemberNode(node)) {
    return { type: 'MemberExpression', object: asPlainMember(node.object), property: node.property, computed: node.computed };
  }
  return node;
}

function rootIdentifier(node) {
  let n = node;
  while (isMemberNode(n)) n = n.object;
  return n.type === 'Identifier' ? n : null;
}

function findEntry(path) {
  for (const kind of STDLIB_KINDS) {
    const entry = lookupStdlib(kind, path);
    if (entry !== null) return entry;
  }
  return null;
}

// checkStdlib(ast, { source, filename, target, analysis }) -> Diagnostic[] (VN4016 and VN4017)
export function checkStdlib(ast, { source, filename = '<input>', target, analysis } = {}) {
  if (typeof source !== 'string') throw new TypeError('checkStdlib: options.source must be the source string');
  if (!TARGETS.has(target)) throw new TypeError(`checkStdlib: target must be native, web or test, got ${String(target)}`);
  if (!analysis || !Array.isArray(analysis.references)) throw new TypeError('checkStdlib: options.analysis is required');
  const program = ast.type === 'File' ? ast.program : ast;
  const lines = createLineMap(source);
  const at = (start, end) => ({ file: filename, start: lines.posAt(start), end: lines.posAt(end) });
  const diags = [];

  for (const name of analysis.globalsUsed) {
    const judged = analysis.references.filter((r) => r.binding === -1 && r.name === name && !r.typeofOnly);
    if (judged.length === 0) continue;
    // resolveGlobal accepts bare identifiers only; any other spelling is unknown to the manifest.
    const verdict = IDENTIFIER.test(name) ? resolveGlobal(name, target) : null;
    if (verdict !== null && SUPPORTED.has(verdict.status)) continue;
    const ref = judged[0];
    diags.push(createDiagnostic('VN4016', {
      message: `global '${name}' is not available in the ${target} profile`,
      hint: verdict?.hint ?? `Declare '${name}' in this module or import it from a supported module.`,
      ...at(ref.start, ref.end),
    }));
  }

  const refByStart = new Map(analysis.references.map((r) => [r.start, r]));
  const stack = [program];
  while (stack.length > 0) {
    const node = stack.pop();
    if (isMemberNode(node) && !node.computed) {
      const path = memberPath(asPlainMember(node));
      const root = rootIdentifier(node);
      const ref = root === null ? undefined : refByStart.get(root.start);
      if (path !== null && path.includes('.') && ref !== undefined && ref.binding === -1) {
        const entry = findEntry(path);
        if (entry !== null && !isStdlibSupported(path, target)) {
          diags.push(createDiagnostic('VN4017', {
            message: `'${path}' is not supported`,
            hint: entry.hint ?? `'${path}' is not supported.`,
            ...at(node.start, node.end),
          }));
        }
      }
    }
    for (const key of Object.keys(node)) {
      if (SKIP_KEYS.has(key)) continue;
      const value = node[key];
      if (Array.isArray(value)) {
        for (const child of value) if (isNode(child)) stack.push(child);
      } else if (isNode(value)) {
        stack.push(value);
      }
    }
  }
  return diags;
}

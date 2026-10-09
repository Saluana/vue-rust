// Walks a parsed module and reports every construct the language profile excludes (PROFILE_RULES).
// Each finding carries a registered code, a message, a "use this instead" hint and the source span.
// Identifier-based rules use scope resolution from analyzeScopes, so a local binding named Proxy or Object is not
// a global. Scope diagnostics (VN5xxx) are not part of this result; see analyzeScopes.
// With { stdlib: true } the standard-library manifest decides globals and members (VN4016, VN4017, see stdlib-check.mjs):
// the hand-written denylist rules are then not applied, and same-span overlaps keep the most specific code.
import { createDiagnostic, createLineMap } from '../diagnostics/diagnostic.mjs';
import { analyzeScopes } from './scope.mjs';
import { PROFILE_RULES } from './profile-rules.mjs';
import { checkStdlib } from './stdlib-check.mjs';

const LANGS = new Set(['js', 'ts', 'jsx', 'tsx']);
const TARGETS = new Set(['native', 'web', 'test']);
const SKIP_KEYS = new Set([
  'leadingComments', 'trailingComments', 'innerComments', 'comments', 'tokens', 'errors', 'loc', 'extra',
]);
// Same-span overlaps in stdlib mode, most specific first: arguments object, manifest member, manifest global,
// legacy member codes, then the unsupported-constructor rule.
const OVERLAP_PRECEDENCE = ['VN2015', 'VN4017', 'VN4016', 'VN4008', 'VN4015', 'VN2007'];
// Legacy member codes that the manifest covers exactly (Object.defineProperty, Object.setPrototypeOf, Symbol.dispose ...):
// in stdlib mode VN4017 replaces them, so their spans need not coincide.
const MANIFEST_SUPERSEDED_NODE_CODES = new Set(['VN4008', 'VN4015']);

function isNode(value) {
  return value !== null && typeof value === 'object' && typeof value.type === 'string';
}

function resolveOverlaps(diags) {
  const best = new Map();
  for (const d of diags) {
    const rank = OVERLAP_PRECEDENCE.indexOf(d.code);
    if (rank < 0) continue;
    const key = `${d.start.offset}:${d.end.offset}`;
    if (!best.has(key) || rank < best.get(key)) best.set(key, rank);
  }
  return diags.filter((d) => {
    const rank = OVERLAP_PRECEDENCE.indexOf(d.code);
    return rank < 0 || rank === best.get(`${d.start.offset}:${d.end.offset}`);
  });
}

export function checkProfile(ast, {
  source,
  filename = '<input>',
  lang = 'js',
  target = 'native',
  analysis,
  stdlib = false,
} = {}) {
  if (typeof source !== 'string') throw new TypeError('checkProfile: options.source must be the source string');
  if (!LANGS.has(lang)) throw new TypeError(`checkProfile: lang must be js, ts, jsx or tsx, got ${String(lang)}`);
  if (!TARGETS.has(target)) throw new TypeError(`checkProfile: target must be native, web or test, got ${String(target)}`);
  if (typeof stdlib !== 'boolean') throw new TypeError('checkProfile: options.stdlib must be a boolean');
  if (!isNode(ast)) throw new TypeError('checkProfile: expected a Babel File or Program node');
  const program = ast.type === 'File' ? ast.program : ast;

  const scope = analysis ?? analyzeScopes(ast, { source, filename });
  const refByStart = new Map(scope.references.map((r) => [r.start, r]));
  const ctx = { source, lang, target, analysis: scope, refByStart };

  const nodeRules = PROFILE_RULES.filter(
    (r) => r.kind === 'node' && r.langs.includes(lang) && (!r.targets || r.targets.includes(target)) &&
      !(stdlib && MANIFEST_SUPERSEDED_NODE_CODES.has(r.code)),
  );
  // In stdlib mode the global denylist rules (the rules with a names list) are superseded by VN4016.
  const refRules = PROFILE_RULES.filter(
    (r) => r.kind === 'ref' && r.langs.includes(lang) && (!r.targets || r.targets.includes(target)) &&
      !(stdlib && r.names),
  );
  const rulesByType = new Map();
  for (const rule of nodeRules) {
    for (const type of rule.nodeTypes) {
      if (!rulesByType.has(type)) rulesByType.set(type, []);
      rulesByType.get(type).push(rule);
    }
  }

  const found = [];
  const ancestors = [];
  function walk(node) {
    const rules = rulesByType.get(node.type);
    if (rules) {
      for (const rule of rules) {
        if (rule.appliesTo(node, ctx, ancestors)) found.push({ rule, span: rule.span(node, ctx) });
      }
    }
    ancestors.push(node);
    for (const key of Object.keys(node)) {
      if (SKIP_KEYS.has(key)) continue;
      const value = node[key];
      if (Array.isArray(value)) {
        for (const child of value) if (isNode(child)) walk(child);
      } else if (isNode(value)) {
        walk(value);
      }
    }
    ancestors.pop();
  }
  walk(program);

  for (const rule of refRules) {
    for (const ref of scope.references) {
      if (rule.appliesTo(ref, ctx)) found.push({ rule, span: rule.span(ref, ctx) });
    }
  }

  const lines = createLineMap(source);
  let diags = found.map(({ rule, span }) => createDiagnostic(rule.code, {
    message: rule.message,
    hint: rule.hint,
    file: filename,
    start: lines.posAt(span[0]),
    end: lines.posAt(span[1]),
  }));
  if (stdlib) {
    diags = resolveOverlaps([...diags, ...checkStdlib(ast, { source, filename, target, analysis: scope })]);
  }
  diags.sort((a, b) => a.start.offset - b.start.offset || a.end.offset - b.end.offset ||
    (a.code < b.code ? -1 : a.code > b.code ? 1 : 0));
  return diags;
}

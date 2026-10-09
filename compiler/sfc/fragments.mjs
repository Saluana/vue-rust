// Profile checks for template fragments (WP-F1 step 5): directive expressions, interpolations, v-on statement lists,
// v-for and v-slot parameter lists. A fragment is wrapped so that the frontend's checkSource (parse, scope analysis,
// profile rules) can read it as a module. Diagnostic offsets are mapped back to the fragment's place in its file.
import { checkSource } from '../frontend/check.mjs';
import { checkStdlib } from '../analysis/stdlib-check.mjs';
import { manifestEntry } from './context.mjs';

// Vue's own classification of v-on values: a member path or a function expression is an expression handler,
// anything else is an inline statement list (compiler-core transformOn uses the same two patterns).
const SIMPLE_PATH_RE = /^[A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*|\['[^']*?']|\["[^"]*?"]|\[\d+]|\[[A-Za-z_$][\w$]*])*$/;
const FN_EXP_RE = /^\s*([\w$_]+|(async\s*)?\([^)]*?\))\s*(:[^=]+)?=>|^\s*(async\s+)?function(?:\s+[\w$]+)?\s*\(/;
// Same split as compiler-core's forAliasRE for v-for="left in right".
const FOR_RE = /([\s\S]*?)\s+(?:in|of)\s+([\s\S]*)/;

export function handlerKind(text) {
  return SIMPLE_PATH_RE.test(text) || FN_EXP_RE.test(text) ? 'expr' : 'stmts';
}

// v-for splits into { left, leftStart, right, rightStart } (offsets relative to text), or null when it has no `in`.
export function splitFor(text) {
  const m = FOR_RE.exec(text);
  if (!m) return null;
  return { left: m[1], leftStart: 0, right: m[2], rightStart: m.index + m[0].length - m[2].length };
}

// Wrapper source and the number of characters it adds in front of the fragment text. decl is a declaration of the
// setup bindings ("var a, b;"), so that a template reference to a binding named like a browser global (location) or a
// profile-excluded global (Proxy) resolves to that binding and is not checked as a global.
function wrap(kind, text, decl) {
  const d = decl.length;
  if (kind === 'expr') return { src: `${decl}(${text}\n)`, prefix: d + 1 };
  if (kind === 'stmts') return { src: `${decl}${text}`, prefix: d };
  if (kind === 'params') return { src: `${decl}(${text}) => 0`, prefix: d + 1 };
  if (kind === 'vfor-params') {
    // A v-for alias may already be parenthesised: "(item, index)". Then it is a parameter list as written.
    if (/^\s*\([\s\S]*\)\s*$/.test(text)) return { src: `${decl}${text} => 0`, prefix: d };
    return { src: `${decl}(${text}) => 0`, prefix: d + 1 };
  }
  throw new TypeError(`fragments: unknown kind ${kind}`);
}

// Declaration of the names as one var statement, or '' when there are none.
export function declarationOf(names) {
  return names.size ? `var ${[...names].join(', ')};` : '';
}

// Checks one fragment that starts at file offset base. kind is expr, stmts, params or vfor-params; decl is the
// declaration from declarationOf(). Names declared by the fragment itself (v-for or v-slot parameters) shadow decl.
export function checkFragment(ctx, { file, base, text, kind, lang, decl = '', scriptBroken = false }) {
  if (text.trim() === '') return;
  const { src, prefix } = wrap(kind, text, decl);
  const res = checkSource(src, { filename: file, lang, target: ctx.target });
  if (res.ast && !ctx.diags.some(d => d.severity === 'error') && !res.diagnostics.some(d => d.severity === 'error')) {
    res.diagnostics.push(...checkStdlib(res.ast, { source: src, filename: file, target: ctx.target, analysis: res.analysis }));
  }
  const at = (offset) => base + Math.max(0, offset - prefix);
  for (const d of res.diagnostics) {
    // Instance properties have their own policy and diagnostic below.
    const ref = res.analysis?.references.find(r => r.start === d.start.offset && r.binding === -1);
    if (d.code === 'VN4016' && (scriptBroken || (ref && manifestEntry('instance', ref.name)))) continue;
    ctx.report(d.code, file, at(d.start.offset), at(d.end.offset), {
      message: d.message,
      hint: d.hint,
      severity: d.severity,
    });
  }
  if (!res.analysis) return new Set();
  // Instance properties that the profile excludes (A18, A12): $el, $parent, $root, $watch, $forceUpdate, $host.
  for (const ref of res.analysis.references) {
    if (ref.binding !== -1 || !ref.name.startsWith('$')) continue;
    const st = manifestEntry('instance', ref.name);
    if (!st || st.status !== 'X') continue;
    ctx.report('VN6117', file, at(ref.start), at(ref.end), {
      message: `instance property '${ref.name}' is not supported`,
      hint: st.note ?? undefined,
    });
  }
  const wrapper = res.analysis.functions.find(f => f.parentFnId === -1 && f.kind === 'arrow');
  return new Set(res.analysis.bindings.filter(b => b.kind === 'param' && b.scopeId === wrapper?.scopeId).map(b => b.name));
}

// Module parsing with @babel/parser (errorRecovery off, ranges on). Bad user input never throws: every failure becomes a
// diagnostic with a code and span. Language modes: js, ts, jsx, tsx. Babel's jsx plugin is enabled only for jsx and tsx
// (Vue's compileScript does the same), so JSX in js or ts is a parse error. In ts and tsx, type syntax is parsed, and
// angle-bracket assertions in ts are erased like in a real .ts file. Sloppy-only forms that module parsing rejects
// (with, legacy octal, delete of a name, labelled functions, eval/arguments bindings) and duplicate declarations are
// reported with their profile codes through the reason mapping declared in compiler/analysis/profile-rules.mjs.
import { parse } from '@babel/parser';
import { createDiagnostic, createLineMap } from '../diagnostics/diagnostic.mjs';
import { PROFILE_RULES } from '../analysis/profile-rules.mjs';

export const LANG_VALUES = Object.freeze(['js', 'ts', 'jsx', 'tsx']);

const DECORATOR_PLUGINS = ['decorators', 'decoratorAutoAccessors'];
const PLUGINS_BY_LANG = Object.freeze({
  js: Object.freeze([...DECORATOR_PLUGINS]),
  ts: Object.freeze(['typescript', ...DECORATOR_PLUGINS]),
  jsx: Object.freeze(['jsx', ...DECORATOR_PLUGINS]),
  tsx: Object.freeze(['typescript', 'jsx', ...DECORATOR_PLUGINS]),
});

const JSX_HINT = 'use a <template> in a .vue file';

const RULE_BY_REASON = new Map();
for (const rule of PROFILE_RULES) {
  for (const reason of rule.reasons ?? []) RULE_BY_REASON.set(reason, rule);
}

const IDENT_CHAR = /[\p{ID_Continue}$\u200C\u200D]/u;

function babelParse(source, plugins) {
  return parse(source, { sourceType: 'module', errorRecovery: false, ranges: true, plugins: [...plugins] });
}

// Span of the token at pos: an identifier, number or keyword run, a quoted string on one line, or one character.
function tokenSpan(source, pos) {
  if (pos >= source.length) return [source.length, source.length];
  const ch = source[pos];
  if (IDENT_CHAR.test(ch)) {
    let e = pos;
    while (e < source.length && IDENT_CHAR.test(source[e])) e++;
    return [pos, e];
  }
  if (ch === '"' || ch === "'" || ch === '`') {
    let e = pos + 1;
    while (e < source.length && source[e] !== ch && source[e] !== '\n') {
      if (source[e] === '\\') e++;
      e++;
    }
    if (e < source.length && source[e] === ch) e++;
    return [pos, Math.min(e, source.length)];
  }
  return [pos, pos + 1];
}

function cleanBabelMessage(message) {
  return String(message).replace(/\s*\(\d+:\d+\)$/, '').replace(/\.$/, '');
}

// A js or ts parse failure is JSX when the error reads like JSX (Babel's missing-jsx-plugin error, or a '<' that starts
// an identifier), or when the same source parses with the jsx plugin enabled (the only difference is JSX syntax).
function isJsxFailure(err, source, lang) {
  if (err.reasonCode === 'MissingOneOfPlugins' && /"jsx"/.test(err.message)) return true;
  if (source[err.pos] === '<' && /[A-Za-z_$]/.test(source[err.pos + 1] ?? '')) return true;
  try {
    babelParse(source, lang === 'ts' ? PLUGINS_BY_LANG.tsx : PLUGINS_BY_LANG.jsx);
    return true;
  } catch {
    return false;
  }
}

function parseFailure(err, source, filename, lang) {
  const lines = createLineMap(source);
  if (!(err instanceof SyntaxError) || !Number.isInteger(err.pos) || err.pos < 0 || err.pos > source.length) {
    const origin = lines.posAt(0);
    return createDiagnostic('VN9001', {
      message: `parser failed internally: ${err && err.message ? err.message : String(err)}`,
      file: filename,
      start: origin,
      end: origin,
    });
  }

  const reason = err.reasonCode;
  let [start, end] = tokenSpan(source, err.pos);
  if (reason === 'StrictNumericEscape' && source[err.pos - 1] === '\\') {
    // Babel points at the digit after the backslash; the escape sequence starts at the backslash.
    start = err.pos - 1;
    end = err.pos;
    while (end < source.length && end - err.pos < 2 && /[0-9]/.test(source[end])) end++;
  }

  const rule = RULE_BY_REASON.get(reason);
  if (rule) {
    return createDiagnostic(rule.code, {
      message: rule.useBabelMessage ? cleanBabelMessage(err.message) : rule.message,
      hint: rule.hint,
      file: filename,
      start: lines.posAt(start),
      end: lines.posAt(end),
    });
  }

  if ((lang === 'js' || lang === 'ts') && isJsxFailure(err, source, lang)) {
    return createDiagnostic('VN1000', {
      message: `JSX is not supported in lang '${lang}'`,
      hint: JSX_HINT,
      file: filename,
      start: lines.posAt(start),
      end: lines.posAt(end),
    });
  }

  const hint = lang === 'tsx' && reason === 'UnterminatedJsxContent'
    ? 'In .tsx files angle-bracket assertions are not allowed. Write `value as Type`.'
    : undefined;
  return createDiagnostic('VN1000', {
    message: cleanBabelMessage(err.message),
    hint,
    file: filename,
    start: lines.posAt(start),
    end: lines.posAt(end),
  });
}

// parseModule(source, { filename, lang }) -> { ast, diagnostics }
// ast is the Babel File node when parsing succeeds, otherwise null. lang is 'js', 'ts', 'jsx' or 'tsx'.
export function parseModule(source, { filename = '<input>', lang = 'js' } = {}) {
  if (typeof source !== 'string') throw new TypeError('parseModule: source must be a string');
  if (!LANG_VALUES.includes(lang)) {
    throw new TypeError(`parseModule: lang must be one of ${LANG_VALUES.join(', ')}; got ${String(lang)}`);
  }
  try {
    const ast = babelParse(source, PLUGINS_BY_LANG[lang]);
    return { ast, diagnostics: [] };
  } catch (err) {
    return { ast: null, diagnostics: [parseFailure(err, source, filename, lang)] };
  }
}

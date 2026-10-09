// Names and literals in the emitted Rust (WP-C1b). Every local binding is `v<id>_<ident>`, a cell `c<id>`, a captured
// cell `k<index>`, a module cell `m<id>`, an import cell `i<id>`, a function `f<id>_<ident>`, a string pool entry
// `S<n>`. The binding id is the analysis id, so two bindings with one name never share a Rust name (a Rust `let` shadows
// lexically, which would break `let x; { x; let x; }`). ASCII identifier characters are kept; others become `_u<hex>_`.

export function mangle(name) {
  let out = '';
  for (const ch of name) {
    if (/^[A-Za-z0-9]$/.test(ch)) out += ch;
    else if (ch === '_') out += '_';
    else out += `_u${ch.codePointAt(0).toString(16)}_`;
  }
  return out;
}

export function localName(bid, name) {
  return `v${bid}_${mangle(name)}`;
}

export function cellName(bid) {
  return `c${bid}`;
}

export function fnName(fid, name) {
  return `f${fid}_${name ? mangle(name) : 'anon'}`;
}

// A Rust string literal (for specifiers, names and messages).
export function rstr(s) {
  let out = '"';
  for (const ch of s) {
    const cp = ch.codePointAt(0);
    if (ch === '"') out += '\\"';
    else if (ch === '\\') out += '\\\\';
    else if (cp < 0x20 || cp === 0x7f) out += `\\u{${cp.toString(16)}}`;
    else out += ch;
  }
  return `${out}"`;
}

// An f64 literal that denotes exactly the JavaScript number text (String(n), or "-0").
export function f64Lit(text) {
  if (text === '-0') return '-0.0_f64';
  if (text === 'NaN') return 'f64::NAN';
  if (text === 'Infinity') return 'f64::INFINITY';
  if (text === '-Infinity') return 'f64::NEG_INFINITY';
  let t = text.replace('e+', 'e');
  if (!/[.eE]/.test(t)) t = `${t}.0`;
  if (t.startsWith('-')) return `(${t}_f64)`;
  return `${t}_f64`;
}

// The forms of a constant operand (emitExpr: undef, null, bool, num, str). Anything else is a named temporary or local.
const LITERAL_VALUE_RE = /^(?:Value::(?:Undefined|Null)|Value::Bool\((?:true|false)\)|Value::Number\((?:\(?-?[0-9][0-9._e+-]*_f64\)?|f64::(?:NAN|INFINITY|NEG_INFINITY))\)|Value::String\(JsStr::from_units\(S[0-9]+\)\))$/;

// The owned Value of an operand expression. A constant is already a fresh Value, so it gets no `.clone()`; a temporary or a
// local is cloned as before.
export function owned(expr) {
  return LITERAL_VALUE_RE.test(expr) ? expr : `${expr}.clone()`;
}

// The constructor of a Vec that receives `pushes` items: with_capacity when that count is known statically, null when it is
// not (a spread or a loop). Vec::new() for an empty list, as before.
export function vecNew(pushes) {
  return pushes === null || pushes === 0 ? 'Vec::new()' : `Vec::with_capacity(${pushes})`;
}

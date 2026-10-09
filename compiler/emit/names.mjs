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

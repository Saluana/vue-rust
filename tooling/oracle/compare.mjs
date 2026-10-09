// Structural comparison of two trace documents (trace-format.md, "Comparison rule").
// Object key order is irrelevant; array order is significant; numbers and $-encodings must match exactly.
// compareDocs returns null when equal, otherwise {path, a, b} for the first difference in document order.

const hasOwn = (o, k) => Object.prototype.hasOwnProperty.call(o, k);
const IDENT = /^[A-Za-z_$][\w$]*$/;

function childPath(base, key) {
  return IDENT.test(key) ? `${base}.${key}` : `${base}[${JSON.stringify(key)}]`;
}

function diff(a, b, path) {
  if (a === null || b === null || typeof a !== 'object' || typeof b !== 'object') {
    // Object.is: 0 and -0 differ, NaN equals NaN.
    return Object.is(a, b) ? null : {path, a, b};
  }
  if (Array.isArray(a) !== Array.isArray(b)) return {path, a, b};
  if (Array.isArray(a)) {
    const n = Math.min(a.length, b.length);
    for (let i = 0; i < n; i++) {
      const d = diff(a[i], b[i], `${path}[${i}]`);
      if (d) return d;
    }
    return a.length === b.length ? null : {path: `${path}.length`, a: a.length, b: b.length};
  }
  const keys = [...new Set([...Object.keys(a), ...Object.keys(b)])].sort();
  for (const k of keys) {
    const inA = hasOwn(a, k);
    const inB = hasOwn(b, k);
    if (!inA || !inB) {
      return {path: childPath(path, k), a: inA ? a[k] : '<missing>', b: inB ? b[k] : '<missing>'};
    }
    const d = diff(a[k], b[k], childPath(path, k));
    if (d) return d;
  }
  return null;
}

export function compareDocs(a, b) {
  return diff(a, b, '$');
}

// Human-readable one-line description of a compareDocs result.
export function describeDiff(d) {
  return `first difference at ${d.path}: ${JSON.stringify(d.a)} vs ${JSON.stringify(d.b)}`;
}

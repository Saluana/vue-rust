// Canonical trace encoder (docs/dev/trace-format.md, v1). Both engines must produce identical output.
// Global ordinals for objects and symbols; per-call refs for aliases and cycles; getters are never invoked.

const UNTRACEABLE = () => new TypeError('untraceable value');

// trace-format v1.1: `integrity` is present only for non-extensible objects, right after `proto`:
// "frozen" if Object.isFrozen, else "sealed" if Object.isSealed, else "nonextensible".
function integrityOf(o) {
  if (Object.isExtensible(o)) return undefined;
  if (Object.isFrozen(o)) return 'frozen';
  if (Object.isSealed(o)) return 'sealed';
  return 'nonextensible';
}

function header(o, cls, id, proto) {
  const h = {$: 'obj', id, cls, proto};
  const integrity = integrityOf(o);
  if (integrity !== undefined) h.integrity = integrity;
  return h;
}

export function createRecorder() {
  const entries = [];
  const ids = new Map();   // object -> ordinal (global, whole run)
  const symIds = new Map(); // symbol -> ordinal (own counter)
  let nextId = 1;
  let nextSym = 1;
  let seen = null;          // per-trace-call: object -> true once expanded in this call

  function encode(v) {
    if (v === undefined) return { $: 'undef' };
    if (v === null || v === true || v === false) return v;
    if (typeof v === 'number') {
      if (Object.is(v, -0)) return { $: 'num', v: '-0' };
      if (Number.isNaN(v)) return { $: 'num', v: 'NaN' };
      if (v === Infinity) return { $: 'num', v: 'Infinity' };
      if (v === -Infinity) return { $: 'num', v: '-Infinity' };
      return v;
    }
    if (typeof v === 'string') return v;
    if (typeof v === 'symbol') {
      if (!symIds.has(v)) symIds.set(v, nextSym++);
      return { $: 'sym', id: symIds.get(v), desc: v.description === undefined ? null : v.description };
    }
    if (typeof v === 'function') return { $: 'fn', name: v.name, length: v.length };
    if (typeof v === 'object') return encodeObject(v);
    throw UNTRACEABLE();
  }

  function idOf(o) {
    if (!ids.has(o)) ids.set(o, nextId++);
    return ids.get(o);
  }

  function classOf(o) {
    if (Array.isArray(o)) return 'Array';
    if (o instanceof Map) return 'Map';
    if (o instanceof Set) return 'Set';
    if (o instanceof Error) return 'Error';
    if (o instanceof Promise) return 'Promise';
    if (Object.prototype.toString.call(o) === '[object Date]' ||
        o instanceof RegExp || ArrayBuffer.isView(o) || o instanceof ArrayBuffer ||
        o instanceof WeakMap || o instanceof WeakSet || typeof o === 'bigint') throw UNTRACEABLE();
    return 'Object';
  }

  function protoOf(o) {
    const p = Object.getPrototypeOf(o);
    if (p === null) return 'null';
    if (p === Object.prototype) return 'Object.prototype';
    return 'other';
  }

  function encodeObject(o) {
    const cls = classOf(o);
    const id = idOf(o);
    if (seen.has(o)) return { $: 'ref', id };
    seen.add(o);
    if (cls === 'Promise') return {...header(o, cls, id, 'other'), props: []};
    const out = {...header(o, cls, id, protoOf(o)), props: []};
    if (cls === 'Array') {
      out.elems = [];
      out.len = o.length;
      for (let i = 0; i < o.length; i++) {
        if (!Object.prototype.hasOwnProperty.call(o, i)) out.elems.push({ $: 'hole' });
        else out.elems.push(encodeProp(o, i));
      }
    } else if (cls === 'Map') {
      out.entries = [];
      for (const [k, v] of o) out.entries.push([encode(k), encode(v)]);
    } else if (cls === 'Set') {
      out.values = [];
      for (const v of o) out.values.push(encode(v));
    } else if (cls === 'Error') {
      out.name = typeof o.name === 'string' ? o.name : String(o.name);
      out.message = typeof o.message === 'string' ? o.message : String(o.message);
    }
    for (const key of Reflect.ownKeys(o)) {
      if (typeof key !== 'string') continue;       // symbol keys not traced in v1
      if (cls === 'Array' && isIndex(key)) continue; // array index keys are not in props
      const d = Object.getOwnPropertyDescriptor(o, key);
      if (!d.enumerable) continue;
      if (cls === 'Error' && key === 'stack') continue; // stack is never traced
      if ('get' in d || 'set' in d) {
        out.props.push([key, { $: 'accessor', get: d.get !== undefined, set: d.set !== undefined }]);
      } else {
        out.props.push([key, encode(d.value)]);
      }
    }
    return out;
  }

  function encodeProp(o, key) {
    const d = Object.getOwnPropertyDescriptor(o, key);
    if ('get' in d || 'set' in d) return { $: 'accessor', get: d.get !== undefined, set: d.set !== undefined };
    return encode(d.value);
  }

  function isIndex(k) {
    return String(Number(k) >>> 0) === k && Number(k) !== 4294967295;
  }

  function trace(label, ...values) {
    if (typeof label !== 'string') throw new TypeError('trace label must be a string');
    const prev = seen;
    seen = new Set();
    try {
      const encoded = values.map(encode);
      entries.push({ label, values: encoded });
    } finally {
      seen = prev;
    }
  }

  // Standalone encoding (document-level `error`, `unhandledRejections`) is one encoding call each:
  // fresh per-call refs, global ordinals shared with the run.
  function callEncode(list) {
    const prev = seen;
    seen = new Set();
    try { return list.map((v) => encode(v)); } finally { seen = prev; }
  }
  return {
    trace,
    entries,
    encode: (v) => callEncode([v])[0],
    encodeAll: (list) => callEncode(list),
    ids,
  };
}

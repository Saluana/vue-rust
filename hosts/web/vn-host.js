// vn-host.js: the browser glue of vn-renderer-dom (ABI 1, see ABI.md). An ES module with no dependencies.
// The wasm module calls the `vn_host` imports below; each one applies one Host operation to the real DOM. Every import
// catches its own exceptions: a JavaScript exception must never unwind through wasm frames.

export const ABI_VERSION = 1;

const OK = 0;
const NOT_FOUND = 1;
const UNSUPPORTED = 2;
const INVALID_STATE = 3;
const LOG_WARN = 2;
const LOG_ERROR = 3;
const LOG_LEVELS = ['debug', 'info', 'warn', 'error'];
const ELEMENT = 1;
const DOCUMENT = 9;

// Handles are `generation * 2^20 + index`: the index addresses a slot, the generation makes a released handle stale.
const INDEX_LIMIT = 2 ** 20;
const INDEX_MASK = INDEX_LIMIT - 1;
const GENERATION_LIMIT = 4095;

const NS_URI = ['http://www.w3.org/1999/xhtml', 'http://www.w3.org/2000/svg', 'http://www.w3.org/1998/Math/MathML'];
const LISTENER = { capture: 1, once: 2, passive: 4 };
const OUTCOME = { stopPropagation: 1, stopImmediate: 2, preventDefault: 4, threw: 8 };
const VALUE = { undefined: 0, null: 1, bool: 2, number: 3, string: 4 };
const EVENT = {
  header: 80, bubbles: 1, cancelable: 2, composed: 4, defaultPrevented: 8,
  ctrl: 16, shift: 32, alt: 64, meta: 128, composing: 256, inputData: 512,
};

// Policy (documented in ABI.md): the host refuses to create code-running elements, event-handler content or javascript: URLs.
const BLOCKED_TAGS = new Set(['script', 'iframe', 'frame', 'frameset', 'object', 'embed', 'applet', 'base']);
const URL_ATTRIBUTES = new Set(['href', 'src', 'action', 'formaction', 'poster', 'data', 'background']);
const JS_URL = /^[\s\u0000-\u001f]*javascript:/i;
// Selector subset shared with vn-dom (crates/dom/src/query.rs): compounds of tag, #id, .class, [attr], [attr=value], *,
// separated by commas. Anything else is Unsupported rather than passed to the browser.
const COMPOUND = String.raw`(?:\*|[A-Za-z][\w-]*|#[\w-]+|\.[\w-]+|\[[A-Za-z_][\w:-]*(?:=(?:"[^"]*"|'[^']*'|[\w-]+))?\])+`;
const SELECTOR = new RegExp(String.raw`^\s*${COMPOUND}(?:\s*,\s*${COMPOUND})*\s*$`);

const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });

// A failure with a status code; the message is kept for the wasm side (host_error_message).
class HostFault extends Error {
  constructor(status, message) {
    super(message);
    this.status = status;
  }
}
const unsupported = (message) => new HostFault(UNSUPPORTED, message);
const invalid = (message) => new HostFault(INVALID_STATE, message);

export function consoleLog(level, message) {
  const sink = { debug: console.debug, info: console.info, warn: console.warn, error: console.error }[level] ?? console.error;
  sink.call(console, `[vn-host] ${message}`);
}

const messageOf = (e) => (e instanceof Error ? `${e.name}: ${e.message}` : String(e));

/**
 * Instantiates the renderer and wires it to `document`.
 * @param {object} options
 * @param {Document} options.document the document the host mounts into
 * @param {string|URL|ArrayBuffer|Uint8Array|WebAssembly.Module} options.wasmBytesOrUrl the module to load
 * @param {(level: string, message: string) => void} [options.onLog] receives warnings and errors (default: console)
 * @param {number} [options.maxNodes] live node handles allowed at once (default: 2^20 - 1)
 * @returns {Promise<object>} the host: { abiVersion, exports, memory, stats(), destroy() }
 */
export async function createHost({ document, wasmBytesOrUrl, onLog = consoleLog, maxNodes = INDEX_MASK } = {}) {
  if (!document) throw new TypeError('vn-host: createHost needs a document');
  if (!Number.isInteger(maxNodes) || maxNodes < 1 || maxNodes > INDEX_MASK) {
    throw new RangeError(`vn-host: maxNodes must be an integer from 1 to ${INDEX_MASK}`);
  }
  const state = {
    document, onLog, maxNodes, exports: null, memory: null,
    nodes: [], handles: [], gens: [], free: [], next: 1,
    eventIds: new WeakMap(), nextEvent: 1, flushQueued: false,
    ids: new WeakMap(), listened: new WeakMap(), timers: new Map(), panics: [], lastError: '', destroyed: false,
  };
  const table = importTable(state);
  const module = await compile(wasmBytesOrUrl);
  const unknown = WebAssembly.Module.imports(module).filter(
    (i) => i.module !== 'vn_host' || i.kind !== 'function' || !Object.hasOwn(table, i.name),
  );
  if (unknown.length > 0) {
    const names = unknown.map((i) => `${i.module}.${i.name}`).join(', ');
    throw new Error(`vn-host: ABI mismatch: the module imports ${names}, which ABI ${ABI_VERSION} does not provide`);
  }
  const instance = await WebAssembly.instantiate(module, { vn_host: table });
  const ex = instance.exports;
  for (const name of ['memory', 'vn_abi_version', 'vn_init', 'vn_alloc', 'vn_free', 'vn_event', 'vn_timer']) {
    if (!(name in ex)) throw new Error(`vn-host: ABI mismatch: the module does not export ${name}`);
  }
  const version = ex.vn_abi_version();
  if (version !== ABI_VERSION) {
    throw new Error(`vn-host: ABI mismatch: the module reports abiVersion ${version}, this glue speaks abiVersion ${ABI_VERSION}`);
  }
  state.exports = ex;
  state.memory = ex.memory;
  ex.vn_init();
  return hostApi(state);
}

async function compile(source) {
  if (source instanceof WebAssembly.Module) return source;
  if (source instanceof ArrayBuffer || ArrayBuffer.isView(source)) return WebAssembly.compile(source);
  if (source === undefined) throw new TypeError('vn-host: wasmBytesOrUrl is required');
  const url = String(source);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`vn-host: cannot load ${url}: HTTP ${response.status}`);
  return WebAssembly.compile(await response.arrayBuffer());
}

function hostApi(state) {
  return {
    abiVersion: ABI_VERSION,
    exports: state.exports,
    memory: state.memory,
    stats() {
      let liveHandles = 0;
      let listeners = 0;
      for (let i = 1; i < state.handles.length; i++) {
        if (!state.handles[i]) continue;
        liveHandles++;
        const map = state.listened.get(state.nodes[i]);
        if (map) listeners += map.size;
      }
      return {
        liveHandles, listeners, timers: state.timers.size, panics: [...state.panics],
        pages: state.memory.buffer.byteLength / 65536,
      };
    },
    // Diagnostics, not part of ABI 1: the browser's own dispatch on a handle, used by the corpus replay in the tests.
    dispatchEvent(handle, kind, type, init) {
      return nodeOf(state, handle).dispatchEvent(makeEvent(kind, type, init));
    },
    click(handle) {
      nodeOf(state, handle).click();
    },
    // Removes every listener and timer the host registered, and forgets all handles. Later calls answer InvalidState.
    destroy() {
      if (state.destroyed) return;
      state.destroyed = true;
      for (const timer of state.timers.values()) clearTimeout(timer);
      state.timers.clear();
      for (let i = 1; i < state.nodes.length; i++) {
        const node = state.nodes[i];
        const map = node && state.listened.get(node);
        if (map) for (const rec of map.values()) node.removeEventListener(rec.type, rec.fn, rec.capture);
      }
      state.listened = new WeakMap();
      state.nodes = [];
      state.handles = [];
      state.gens = [];
      state.free = [];
      state.next = 1;
    },
  };
}

// ---- handles

function allocHandle(state, node) {
  let index;
  if (state.free.length > 0) index = state.free.pop();
  else if (state.next <= state.maxNodes) index = state.next++;
  else throw invalid(`no free node handle: ${state.maxNodes} handles are live`);
  const generation = state.gens[index] ?? 1;
  state.gens[index] = generation;
  const handle = generation * INDEX_LIMIT + index;
  state.nodes[index] = node;
  state.handles[index] = handle;
  state.ids.set(node, handle);
  return handle;
}

// The handle of a node, allocated when the node has none yet (nodes the host did not create, such as body).
const handleOf = (state, node) => state.ids.get(node) ?? allocHandle(state, node);

function freeHandle(state, index) {
  state.ids.delete(state.nodes[index]);
  state.nodes[index] = undefined;
  state.handles[index] = 0;
  state.gens[index] = (state.gens[index] % GENERATION_LIMIT) + 1;
  state.free.push(index);
}

function nodeOf(state, handle) {
  const index = handle & INDEX_MASK;
  if (index === 0 || state.handles[index] !== handle) {
    throw new HostFault(NOT_FOUND, `node handle ${handle} is unknown or was released`);
  }
  return state.nodes[index];
}

function elementOf(state, handle) {
  const node = nodeOf(state, handle);
  if (node.nodeType !== ELEMENT) throw invalid(`node handle ${handle} is not an element`);
  return node;
}

// ---- linear memory (every access re-reads memory.buffer: vn_alloc may grow it)

function readText(state, ptr, len) {
  return len === 0 ? '' : decoder.decode(new Uint8Array(state.memory.buffer, ptr, len));
}

function allocBytes(state, bytes) {
  const ptr = state.exports.vn_alloc(bytes.length) >>> 0;
  if (ptr === 0) throw invalid('vn_alloc failed: out of memory');
  new Uint8Array(state.memory.buffer, ptr, bytes.length).set(bytes);
  return ptr;
}

// Writes a string for Rust into the out slot (ptr, len); Rust frees the bytes.
function putText(state, out, text) {
  const bytes = encoder.encode(text);
  const ptr = allocBytes(state, bytes);
  const view = new DataView(state.memory.buffer);
  view.setUint32(out, ptr, true);
  view.setUint32(out + 4, bytes.length, true);
}

function putU32(state, out, value) {
  new DataView(state.memory.buffer).setUint32(out, value, true);
}

// A HostValue out struct: kind @0, flag @4, num @8 (f64), text ptr @16, text len @20.
function writeValue(state, out, value) {
  let kind = VALUE.undefined;
  let flag = 0;
  let num = 0;
  let ptr = 0;
  let len = 0;
  if (value === null) {
    kind = VALUE.null;
  } else if (typeof value === 'boolean') {
    kind = VALUE.bool;
    flag = value ? 1 : 0;
  } else if (typeof value === 'number') {
    kind = VALUE.number;
    num = value;
  } else if (typeof value === 'string') {
    const bytes = encoder.encode(value);
    kind = VALUE.string;
    ptr = allocBytes(state, bytes);
    len = bytes.length;
  } else if (value !== undefined) {
    throw unsupported(`a ${typeof value} property value cannot cross the host boundary`);
  }
  const dv = new DataView(state.memory.buffer);
  dv.setUint32(out, kind, true);
  dv.setUint32(out + 4, flag, true);
  dv.setFloat64(out + 8, num, true);
  dv.setUint32(out + 16, ptr, true);
  dv.setUint32(out + 20, len, true);
}

function decodeValue(state, kind, num, ptr, len) {
  switch (kind) {
    case VALUE.undefined: return undefined;
    case VALUE.null: return null;
    case VALUE.bool: return num !== 0;
    case VALUE.number: return num;
    case VALUE.string: return readText(state, ptr, len);
    default: throw invalid(`unknown HostValue kind ${kind}`);
  }
}

// ---- policy checks (documented in ABI.md)

function checkAttribute(name, value) {
  const local = name.slice(name.indexOf(':') + 1).toLowerCase();
  if (local.startsWith('on')) throw unsupported(`'${name}': event handler attributes are not supported; use add_event_listener`);
  if (local === 'srcdoc') throw unsupported(`'${name}' is not supported by the host`);
  if (URL_ATTRIBUTES.has(local) && JS_URL.test(value)) throw unsupported(`'${name}': javascript: URLs are not supported`);
}

function checkProperty(node, name, value) {
  if (!(name in node)) throw unsupported(`'${name}' is not a property of this node; use set_attribute for attributes`);
  if (/^on/i.test(name)) throw unsupported(`'${name}': event handler properties are not supported; use add_event_listener`);
  if (name === 'innerHTML' || name === 'outerHTML' || name === 'srcdoc') {
    throw unsupported(`'${name}' cannot be set by the host; build the tree with the host operations`);
  }
  if (typeof value === 'string' && URL_ATTRIBUTES.has(name) && JS_URL.test(value)) {
    throw unsupported(`'${name}': javascript: URLs are not supported`);
  }
}

function createElementOf(state, ns, tag, is) {
  if (BLOCKED_TAGS.has(tag.toLowerCase())) throw unsupported(`<${tag}> elements are not created by the host`);
  if (ns === 0) return state.document.createElement(tag, is === undefined ? undefined : { is });
  if (ns === 1 || ns === 2) {
    if (is !== undefined) throw unsupported('customized built-in elements are HTML only');
    return state.document.createElementNS(NS_URI[ns], tag);
  }
  throw unsupported(`unknown namespace code ${ns}`);
}

// ---- events

const listenerKey = (type, id, capture) => `${type}\u0000${id}\u0000${capture ? 1 : 0}`;

// One closure per (node, type, listener id, capture): re-adding the same listener is a no-op, as in the DOM.
function recordFor(state, node, type, id, capture) {
  let map = state.listened.get(node);
  if (!map) {
    map = new Map();
    state.listened.set(node, map);
  }
  const key = listenerKey(type, id, capture);
  let rec = map.get(key);
  if (!rec) {
    rec = { type, capture, fn: (event) => invoke(state, id, event) };
    map.set(key, rec);
  }
  return rec;
}

function invoke(state, id, event) {
  const bits = callSink(state, id, event);
  if (bits & OUTCOME.stopPropagation) event.stopPropagation();
  if (bits & OUTCOME.stopImmediate) event.stopImmediatePropagation();
  if (bits & OUTCOME.preventDefault) event.preventDefault();
  if (bits & OUTCOME.threw) emit(state, LOG_ERROR, `listener ${id} threw; the event keeps dispatching`);
}

function callSink(state, id, event) {
  // Optional generated-app exports extend the app entry contract without changing host ABI 1.
  if (typeof state.exports.vn_event_context === 'function') {
    let serial = state.eventIds.get(event);
    if (serial === undefined) {
      if (state.nextEvent > 0xffffffff) throw new RangeError('vue-native event identities exhausted');
      serial = state.nextEvent++;
      state.eventIds.set(event, serial);
    }
    const view = state.document.defaultView;
    const kind = view && event instanceof view.KeyboardEvent ? 2
      : view && event instanceof view.MouseEvent ? 1
      : view && event instanceof view.InputEvent ? 3 : 0;
    state.exports.vn_event_context(serial, kind);
  }
  const payload = encodeEvent(state, event);
  try {
    return state.exports.vn_event(id, payload.ptr, payload.len) >>> 0;
  } finally {
    state.exports.vn_free(payload.ptr, payload.len);
    if (!state.flushQueued && typeof state.exports.vn_flush === 'function') {
      state.flushQueued = true;
      queueMicrotask(() => {
        state.flushQueued = false;
        if (!state.destroyed) state.exports.vn_flush();
      });
    }
  }
}

// Event payload (ABI.md): 80-byte little-endian header, then the UTF-8 strings it points at.
function encodeEvent(state, event) {
  const chunks = [
    encoder.encode(String(event.type)),
    encoder.encode(typeof event.key === 'string' ? event.key : ''),
    encoder.encode(typeof event.code === 'string' ? event.code : ''),
    encoder.encode(typeof event.data === 'string' ? event.data : ''),
  ];
  const offsets = [];
  let total = EVENT.header;
  for (const chunk of chunks) {
    offsets.push(total);
    total += chunk.length;
  }
  const ptr = state.exports.vn_alloc(total) >>> 0;
  if (ptr === 0) throw invalid('vn_alloc failed: out of memory');
  const buffer = state.memory.buffer;
  chunks.forEach((chunk, i) => new Uint8Array(buffer, ptr + offsets[i], chunk.length).set(chunk));
  const dv = new DataView(buffer, ptr, total);
  for (let i = 0; i < 4; i++) {
    dv.setUint32(i * 8, offsets[i], true);
    dv.setUint32(i * 8 + 4, chunks[i].length, true);
  }
  const num = (x) => (typeof x === 'number' ? x : 0);
  dv.setFloat64(32, num(event.timeStamp), true);
  dv.setFloat64(40, num(event.clientX), true);
  dv.setFloat64(48, num(event.clientY), true);
  dv.setInt32(56, num(event.button) | 0, true);
  dv.setInt32(60, num(event.detail) | 0, true);
  dv.setUint32(64, state.ids.get(event.target) ?? 0, true);
  dv.setUint32(68, state.ids.get(event.currentTarget) ?? 0, true);
  dv.setUint32(72, num(event.eventPhase) >>> 0, true);
  const flags = [
    [event.bubbles, EVENT.bubbles], [event.cancelable, EVENT.cancelable], [event.composed, EVENT.composed],
    [event.defaultPrevented, EVENT.defaultPrevented], [event.ctrlKey, EVENT.ctrl], [event.shiftKey, EVENT.shift],
    [event.altKey, EVENT.alt], [event.metaKey, EVENT.meta], [event.isComposing, EVENT.composing],
    [typeof event.data === 'string', EVENT.inputData],
  ];
  dv.setUint32(76, flags.reduce((bits, [on, bit]) => (on ? bits | bit : bits), 0), true);
  return { ptr, len: total };
}

// Events built the way the corpus runner builds them (tests/dom/gen/page-runner.mjs).
function makeEvent(kind, type, init = {}) {
  const base = { bubbles: !!init.bubbles, cancelable: !!init.cancelable, composed: !!init.composed };
  if (kind === 'Event') return new Event(type, base);
  if (kind === 'Mouse') {
    return new MouseEvent(type, { ...base, button: init.button, clientX: init.clientX, clientY: init.clientY, detail: init.detail });
  }
  if (kind === 'Keyboard') {
    return new KeyboardEvent(type, {
      ...base, key: init.key, code: init.code, ctrlKey: !!init.ctrlKey, shiftKey: !!init.shiftKey, altKey: !!init.altKey, metaKey: !!init.metaKey,
    });
  }
  if (kind === 'Input') {
    return new InputEvent(type, { ...base, data: init.data === undefined ? null : init.data, isComposing: !!init.isComposing });
  }
  throw new TypeError(`unknown event kind ${kind}`);
}

// ---- logging, timers, panics

function emit(state, level, message) {
  try {
    state.onLog(LOG_LEVELS[level] ?? 'error', message);
  } catch (e) {
    console.error('[vn-host] onLog threw', e);
  }
}

function guard(state, fn) {
  try {
    fn();
  } catch (e) {
    emit(state, LOG_ERROR, messageOf(e));
  }
}

// Runs an operation whose only result is a status code. `infallible` names a Host method that cannot return an error: its
// failures are logged here, because the wasm side answers them with an empty value.
function run(state, fn, infallible) {
  let status = OK;
  if (state.destroyed) {
    state.lastError = 'the host was destroyed';
    status = INVALID_STATE;
  } else {
    try {
      fn();
    } catch (e) {
      status = e instanceof HostFault ? e.status : INVALID_STATE;
      state.lastError = e instanceof HostFault ? e.message : messageOf(e);
    }
  }
  if (status !== OK && infallible) emit(state, LOG_WARN, `${infallible}: ${state.lastError}`);
  return status;
}

// Runs a query that cannot report a failure: the failure is logged and the fallback returned.
function query(state, name, fn, fallback) {
  try {
    return fn();
  } catch (e) {
    emit(state, LOG_WARN, `${name}: ${messageOf(e)}`);
    return fallback;
  }
}

function schedule(state, id, ms) {
  cancel(state, id);
  const timer = setTimeout(() => {
    state.timers.delete(id);
    guard(state, () => {
      if (state.exports.vn_timer(id) === 0) emit(state, LOG_WARN, `timer ${id} fired but no timer hook is installed`);
    });
  }, ms);
  state.timers.set(id, timer);
}

function cancel(state, id) {
  const timer = state.timers.get(id);
  if (timer !== undefined) {
    clearTimeout(timer);
    state.timers.delete(id);
  }
}

function releaseNode(state, handle) {
  const index = handle & INDEX_MASK;
  if (index === 0 || state.handles[index] !== handle) {
    emit(state, LOG_WARN, `release: handle ${handle} is unknown or was already released`);
    return;
  }
  if (state.nodes[index].isConnected) {
    emit(state, LOG_WARN, `release: handle ${handle} is still in the document; the node stays where it is`);
  }
  freeHandle(state, index);
}

// The `vn_host` import table. Every entry takes u32 arguments as signed i32 (normalised with >>> 0) and returns a number.
function importTable(state) {
  const doc = () => state.document;
  return {
    host_now: () => performance.now(),
    host_set_timeout: (id, ms) => guard(state, () => schedule(state, id >>> 0, ms)),
    host_clear_timeout: (id) => guard(state, () => cancel(state, id >>> 0)),
    host_log: (level, p, n) => guard(state, () => emit(state, level >>> 0, readText(state, p >>> 0, n >>> 0))),
    host_panic: (p, n) => guard(state, () => {
      const message = readText(state, p >>> 0, n >>> 0);
      state.panics.push(message);
      emit(state, LOG_ERROR, `wasm panic: ${message}`);
    }),
    host_error_message: (out) => guard(state, () => putText(state, out >>> 0, state.lastError)),

    create_element: (ns, tp, tl, ip, il, hasIs, out) => run(state, () => {
      const tag = readText(state, tp >>> 0, tl >>> 0);
      const is = hasIs ? readText(state, ip >>> 0, il >>> 0) : undefined;
      const element = createElementOf(state, ns >>> 0, tag, is);
      putU32(state, out >>> 0, allocHandle(state, element));
    }),
    create_text: (p, n) => query(state, 'create_text', () => allocHandle(state, doc().createTextNode(readText(state, p >>> 0, n >>> 0))), 0),
    create_comment: (p, n) => query(state, 'create_comment', () => allocHandle(state, doc().createComment(readText(state, p >>> 0, n >>> 0))), 0),
    set_text: (h, p, n) => run(state, () => {
      const node = nodeOf(state, h >>> 0);
      if (node.nodeType === ELEMENT) throw invalid('set_text: an element has no text value; use set_element_text');
      node.nodeValue = readText(state, p >>> 0, n >>> 0);
    }),
    set_element_text: (h, p, n) => run(state, () => {
      elementOf(state, h >>> 0).textContent = readText(state, p >>> 0, n >>> 0);
    }),
    insert: (c, p, a) => run(state, () => {
      const child = nodeOf(state, c >>> 0);
      const parent = nodeOf(state, p >>> 0);
      const anchor = (a >>> 0) === 0 ? null : nodeOf(state, a >>> 0);
      parent.insertBefore(child, anchor);
    }),
    remove: (c) => run(state, () => {
      const child = nodeOf(state, c >>> 0);
      if (child.parentNode) child.parentNode.removeChild(child);
    }),
    parent_node: (h) => query(state, 'parent_node', () => {
      const parent = nodeOf(state, h >>> 0).parentNode;
      return parent && parent.nodeType !== DOCUMENT ? handleOf(state, parent) : 0;
    }, 0),
    next_sibling: (h) => query(state, 'next_sibling', () => {
      const sibling = nodeOf(state, h >>> 0).nextSibling;
      return sibling ? handleOf(state, sibling) : 0;
    }, 0),
    query_selector: (p, n, out) => run(state, () => {
      const selector = readText(state, p >>> 0, n >>> 0);
      if (selector.trim() === '') throw invalid('query_selector: the selector is empty');
      if (!SELECTOR.test(selector)) {
        throw unsupported(`query_selector: '${selector}' is outside the supported selector grammar`);
      }
      const found = doc().querySelector(selector);
      putU32(state, out >>> 0, found ? handleOf(state, found) : 0);
    }),
    body: () => query(state, 'body', () => (doc().body ? handleOf(state, doc().body) : 0), 0),
    release: (h) => guard(state, () => releaseNode(state, h >>> 0)),

    tag_name: (el, out) => run(state, () => putText(state, out >>> 0, elementOf(state, el >>> 0).tagName), 'tag_name'),
    is_element: (h) => query(state, 'is_element', () => (nodeOf(state, h >>> 0).nodeType === ELEMENT ? 1 : 0), 0),

    set_attribute: (el, np, nl, vp, vl) => run(state, () => {
      const element = elementOf(state, el >>> 0);
      const name = readText(state, np >>> 0, nl >>> 0);
      const value = readText(state, vp >>> 0, vl >>> 0);
      checkAttribute(name, value);
      element.setAttribute(name, value);
    }),
    set_attribute_ns: (el, hasNs, nsp, nsl, qp, ql, vp, vl) => run(state, () => {
      const element = elementOf(state, el >>> 0);
      const ns = hasNs ? readText(state, nsp >>> 0, nsl >>> 0) : null;
      const qualified = readText(state, qp >>> 0, ql >>> 0);
      const value = readText(state, vp >>> 0, vl >>> 0);
      checkAttribute(qualified, value);
      element.setAttributeNS(ns, qualified, value);
    }),
    remove_attribute: (el, np, nl) => run(state, () => {
      elementOf(state, el >>> 0).removeAttribute(readText(state, np >>> 0, nl >>> 0));
    }),
    remove_attribute_ns: (el, hasNs, nsp, nsl, lp, ll) => run(state, () => {
      const element = elementOf(state, el >>> 0);
      element.removeAttributeNS(hasNs ? readText(state, nsp >>> 0, nsl >>> 0) : null, readText(state, lp >>> 0, ll >>> 0));
    }),
    get_attribute: (el, np, nl, out) => query(state, 'get_attribute', () => {
      const value = elementOf(state, el >>> 0).getAttribute(readText(state, np >>> 0, nl >>> 0));
      if (value === null) return 0;
      putText(state, out >>> 0, value);
      return 1;
    }, 0),
    has_attribute: (el, np, nl) => query(state, 'has_attribute', () => (
      elementOf(state, el >>> 0).hasAttribute(readText(state, np >>> 0, nl >>> 0)) ? 1 : 0), 0),

    has_property: (h, np, nl) => query(state, 'has_property', () => (
      readText(state, np >>> 0, nl >>> 0) in nodeOf(state, h >>> 0) ? 1 : 0), 0),
    get_property: (h, np, nl, out) => run(state, () => {
      const node = nodeOf(state, h >>> 0);
      writeValue(state, out >>> 0, node[readText(state, np >>> 0, nl >>> 0)]);
    }),
    set_property: (h, np, nl, kind, num, sp, sl) => run(state, () => {
      const node = nodeOf(state, h >>> 0);
      const name = readText(state, np >>> 0, nl >>> 0);
      const value = decodeValue(state, kind >>> 0, num, sp >>> 0, sl >>> 0);
      checkProperty(node, name, value);
      node[name] = value;
    }),

    style_set_property: (el, np, nl, vp, vl, important) => run(state, () => {
      const element = elementOf(state, el >>> 0);
      const name = readText(state, np >>> 0, nl >>> 0);
      const value = readText(state, vp >>> 0, vl >>> 0);
      if (!name.startsWith('--') && !CSS.supports(name, 'inherit')) {
        throw unsupported(`'${name}' is not a CSS property name (use kebab-case, or a custom property)`);
      }
      element.style.setProperty(name, value, (important >>> 0) !== 0 ? 'important' : '');
    }),
    style_remove_property: (el, np, nl) => run(state, () => {
      elementOf(state, el >>> 0).style.removeProperty(readText(state, np >>> 0, nl >>> 0));
    }),
    style_get_property: (el, np, nl, out) => run(state, () => {
      putText(state, out >>> 0, elementOf(state, el >>> 0).style.getPropertyValue(readText(state, np >>> 0, nl >>> 0)));
    }, 'style_get_property'),
    computed_style_property: (el, np, nl, out) => run(state, () => {
      const element = elementOf(state, el >>> 0);
      putText(state, out >>> 0, element.ownerDocument.defaultView.getComputedStyle(element).getPropertyValue(readText(state, np >>> 0, nl >>> 0)));
    }),
    style_css_text: (el, out) => run(state, () => putText(state, out >>> 0, elementOf(state, el >>> 0).style.cssText), 'style_css_text'),
    style_set_css_text: (el, tp, tl) => run(state, () => {
      elementOf(state, el >>> 0).style.cssText = readText(state, tp >>> 0, tl >>> 0);
    }),
    class_list_add: (el, tp, tl) => run(state, () => {
      elementOf(state, el >>> 0).classList.add(readText(state, tp >>> 0, tl >>> 0));
    }),
    class_list_remove: (el, tp, tl) => run(state, () => {
      elementOf(state, el >>> 0).classList.remove(readText(state, tp >>> 0, tl >>> 0));
    }),

    add_event_listener: (h, tp, tl, id, flags) => run(state, () => {
      const node = nodeOf(state, h >>> 0);
      const type = readText(state, tp >>> 0, tl >>> 0);
      const bits = flags >>> 0;
      const capture = (bits & LISTENER.capture) !== 0;
      const rec = recordFor(state, node, type, id >>> 0, capture);
      node.addEventListener(type, rec.fn, {
        capture, once: (bits & LISTENER.once) !== 0, passive: (bits & LISTENER.passive) !== 0,
      });
    }),
    remove_event_listener: (h, tp, tl, id, capture) => run(state, () => {
      const node = nodeOf(state, h >>> 0);
      const type = readText(state, tp >>> 0, tl >>> 0);
      const flag = (capture >>> 0) !== 0;
      const map = state.listened.get(node);
      const key = listenerKey(type, id >>> 0, flag);
      const rec = map?.get(key);
      if (!rec) return;
      node.removeEventListener(type, rec.fn, flag);
      map.delete(key);
    }),

    focus: (h) => run(state, () => {
      const node = nodeOf(state, h >>> 0);
      if (typeof node.focus !== 'function') throw invalid('focus: this node cannot take focus');
      node.focus();
    }),
    blur: (h) => run(state, () => {
      const node = nodeOf(state, h >>> 0);
      if (typeof node.blur !== 'function') throw invalid('blur: this node cannot lose focus');
      node.blur();
    }),
    active_element: () => query(state, 'active_element', () => {
      const active = doc().activeElement;
      return active && active !== doc().body ? handleOf(state, active) : 0;
    }, 0),

    option_nodes: (h, out) => run(state, () => {
      const select = elementOf(state, h >>> 0);
      if (select.localName !== 'select') throw invalid('option_nodes: the node is not a <select>');
      const handles = Array.from(select.options, (option) => handleOf(state, option));
      const bytes = new Uint8Array(handles.length * 4);
      const view = new DataView(bytes.buffer);
      handles.forEach((handle, i) => view.setUint32(i * 4, handle, true));
      const ptr = allocBytes(state, bytes);
      const dv = new DataView(state.memory.buffer);
      dv.setUint32(out >>> 0, ptr, true);
      dv.setUint32((out >>> 0) + 4, handles.length, true);
    }, 'option_nodes'),
    outer_html: (h, out) => query(state, 'outer_html', () => {
      const html = nodeOf(state, h >>> 0).outerHTML;
      if (typeof html !== 'string') return 0;
      putText(state, out >>> 0, html);
      return 1;
    }, 0),
    inner_html: (h, out) => query(state, 'inner_html', () => {
      const html = nodeOf(state, h >>> 0).innerHTML;
      if (typeof html !== 'string') return 0;
      putText(state, out >>> 0, html);
      return 1;
    }, 0),
  };
}

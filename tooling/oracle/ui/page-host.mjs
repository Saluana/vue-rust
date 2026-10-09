// Page side of the UI oracle (docs/dev/ui-scenario-host.md, docs/dev/trace-format.md). run-ui.mjs loads this module into a
// fresh page and calls runScenario; it resolves with the trace document as one canonical JSON line (no newline).
// Time is the fake clock only: setTimeout, clearTimeout, requestAnimationFrame, Date.now and performance.now read it.
import { createApp, h, shallowRef } from 'vue';
import { createRecorder } from '../trace.mjs';
import { createClock } from './clock.mjs';
import { checkSelector } from './selector.mjs';
import { snapshotNode } from './snapshot.mjs';

// One macrotask boundary: a message posted to the MessageChannel runs after the microtasks queued before it. The channel
// is created when this module loads, before the fake globals are installed, so the host never uses fake timers itself.
const channel = new MessageChannel();
const turnWaiters = [];
channel.port1.onmessage = () => turnWaiters.shift()();
function turn() {
  return new Promise((resolve) => {
    turnWaiters.push(resolve);
    channel.port2.postMessage(null);
  });
}
// Two turns, as in tooling/oracle/node-runner.mjs: rejections reported after the first turn are visible after the second.
async function drain() {
  await turn();
  await turn();
}

let appCount = 0;
const mountState = new WeakMap(); // Mount -> { container, app, holder }
const handleEl = new WeakMap(); // NodeHandle -> Element

// Browser defaults for the events the host creates: which types do not bubble, and which are not cancelable.
const NO_BUBBLE = new Set(['focus', 'blur', 'mouseenter', 'mouseleave', 'load', 'error', 'scroll']);
const NO_CANCEL = new Set(['focus', 'blur', 'focusin', 'focusout', 'mouseenter', 'mouseleave', 'input', 'change',
  'compositionstart', 'compositionupdate', 'compositionend', 'load', 'error', 'scroll']);
const MOUSE = new Set(['click', 'dblclick', 'mousedown', 'mouseup', 'mousemove', 'mouseover', 'mouseout', 'mouseenter',
  'mouseleave', 'contextmenu', 'auxclick']);
const KEY = new Set(['keydown', 'keyup', 'keypress']);
const INPUT = new Set(['input', 'beforeinput']);
const COMPOSITION = new Set(['compositionstart', 'compositionupdate', 'compositionend']);
const FOCUS = new Set(['focus', 'blur', 'focusin', 'focusout']);
const INIT_KEYS = new Set(['bubbles', 'cancelable', 'key', 'code', 'ctrlKey', 'shiftKey', 'altKey', 'metaKey', 'button',
  'detail', 'data', 'isComposing']);

function makeEvent(type, init) {
  const bubbles = init.bubbles ?? !NO_BUBBLE.has(type);
  const cancelable = init.cancelable ?? !NO_CANCEL.has(type);
  const modifiers = { ctrlKey: !!init.ctrlKey, shiftKey: !!init.shiftKey, altKey: !!init.altKey, metaKey: !!init.metaKey };
  if (MOUSE.has(type)) {
    return new MouseEvent(type, {
      bubbles, cancelable, composed: true, view: window, detail: init.detail ?? (type === 'click' ? 1 : 0),
      button: init.button ?? 0, ...modifiers,
    });
  }
  if (KEY.has(type)) {
    return new KeyboardEvent(type, {
      bubbles, cancelable, composed: true, key: init.key ?? '', code: init.code ?? '', isComposing: !!init.isComposing, ...modifiers,
    });
  }
  if (INPUT.has(type)) {
    return new InputEvent(type, {
      bubbles, cancelable, composed: true, data: init.data ?? null, isComposing: !!init.isComposing, inputType: 'insertText',
    });
  }
  if (COMPOSITION.has(type)) return new CompositionEvent(type, { bubbles, cancelable, data: init.data ?? '' });
  if (FOCUS.has(type)) return new FocusEvent(type, { bubbles, cancelable, view: window });
  return new Event(type, { bubbles, cancelable });
}

function checkInit(init) {
  if (init === null || typeof init !== 'object') throw new TypeError('dispatch: init must be an object');
  for (const k of Object.keys(init)) {
    if (!INIT_KEYS.has(k)) throw new TypeError(`dispatch: unknown init member "${k}"`);
  }
  return init;
}

function requireString(value, what) {
  if (typeof value !== 'string') throw new TypeError(`${what} must be a string`);
}

function wrap(el) {
  const handle = new NodeHandle();
  handleEl.set(handle, el);
  return handle;
}

function elOf(handle) {
  const el = handle instanceof NodeHandle ? handleEl.get(handle) : undefined;
  if (el === undefined) throw new TypeError('expected a NodeHandle from testHost');
  return el;
}

function stateOf(mount) {
  const s = mount instanceof Mount ? mountState.get(mount) : undefined;
  if (s === undefined) throw new TypeError('expected a Mount from testHost.mount');
  return s;
}

function propsCopy(props, what) {
  if (props === undefined) return {};
  if (props === null || typeof props !== 'object') throw new TypeError(`${what} must be an object`);
  return { ...props };
}

// A mounted app. Every mount goes through a host component that renders the component with the props held in a
// shallowRef, so setProps re-renders through Vue's own update path. Props are copied shallowly: values keep their identity.
export class Mount {
  snapshot() {
    return [...stateOf(this).container.childNodes].map(snapshotNode);
  }
  html() {
    return stateOf(this).container.innerHTML;
  }
  unmount() {
    stateOf(this).app.unmount();
  }
  setProps(next) {
    stateOf(this).holder.value = propsCopy(next, 'setProps: props');
  }
}

export class NodeHandle {
  text() {
    return elOf(this).textContent;
  }
  html() {
    return elOf(this).innerHTML;
  }
  attr(name) {
    requireString(name, 'attr: name');
    return elOf(this).getAttribute(name);
  }
  prop(name) {
    requireString(name, 'prop: name');
    const v = elOf(this)[name];
    if (v !== null && (typeof v === 'object' || typeof v === 'function')) {
      throw new TypeError(`prop(${name}) is an object; only primitive values can be traced`);
    }
    return v;
  }
  exists() {
    return elOf(this).isConnected;
  }
  snapshot() {
    return snapshotNode(elOf(this));
  }
}

function mount(component, props, options) {
  if (component === null || (typeof component !== 'object' && typeof component !== 'function')) {
    throw new TypeError('mount: the component must be a component object');
  }
  const opts = options ?? {};
  if (typeof opts !== 'object' || opts === null) throw new TypeError('mount: options must be an object');
  for (const k of Object.keys(opts)) {
    if (k !== 'errorHandler') throw new TypeError(`mount: unknown option "${k}"`);
  }
  if (opts.errorHandler !== undefined && typeof opts.errorHandler !== 'function') {
    throw new TypeError('mount: errorHandler must be a function');
  }
  appCount += 1;
  const container = document.createElement('div');
  container.id = `app-${appCount}`;
  document.body.appendChild(container);
  const holder = shallowRef(propsCopy(props, 'mount: props'));
  const Root = { name: 'TestHost', setup: () => () => h(component, { ...holder.value }) };
  const app = createApp(Root);
  if (opts.errorHandler !== undefined) app.config.errorHandler = opts.errorHandler;
  app.mount(container);
  const m = new Mount();
  mountState.set(m, { container, app, holder });
  return m;
}

function scopeElement(scope) {
  return scope instanceof Mount ? stateOf(scope).container : elOf(scope);
}

function findAll(scope, selector) {
  const root = scopeElement(scope);
  checkSelector(selector);
  return [...root.querySelectorAll(selector)].map(wrap);
}

function find(scope, selector) {
  return findAll(scope, selector)[0] ?? null;
}

function dispatch(node, type, init = {}) {
  const el = elOf(node);
  if (typeof type !== 'string' || type === '') throw new TypeError('dispatch: the event type must be a non-empty string');
  el.dispatchEvent(makeEvent(type, checkInit(init)));
}

function click(node) {
  dispatch(node, 'click');
}

function keydown(node, key, mods = {}) {
  requireString(key, 'keydown: key');
  dispatch(node, 'keydown', { ...mods, key });
}

function focus(node) {
  elOf(node).focus();
}

function input(node, value) {
  const el = elOf(node);
  if (!(el instanceof HTMLTextAreaElement || (el instanceof HTMLInputElement && el.type === 'text'))) {
    throw new TypeError('input: the target must be a text input or a textarea');
  }
  requireString(value, 'input: value');
  el.value = value;
  el.dispatchEvent(makeEvent('input', { data: value }));
}

function select(node, value) {
  const el = elOf(node);
  if (!(el instanceof HTMLSelectElement)) throw new TypeError('select: the target must be a select element');
  if (el.multiple) {
    if (!Array.isArray(value)) throw new TypeError('select: a multiple select takes an array of values');
    for (const opt of el.options) opt.selected = value.includes(opt.value);
  } else {
    requireString(value, 'select: value');
    el.value = value;
  }
  el.dispatchEvent(makeEvent('change', {}));
}

function change(node, value) {
  const el = elOf(node);
  if (el instanceof HTMLSelectElement) return select(node, value);
  const textLike = el instanceof HTMLTextAreaElement || (el instanceof HTMLInputElement && (el.type === 'text' || el.type === 'range'));
  if (!textLike) throw new TypeError('change: the target must be a text input, a range, a textarea or a select');
  requireString(value, 'change: value');
  el.value = value;
  el.dispatchEvent(makeEvent('change', {}));
}

function check(node, checked) {
  const el = elOf(node);
  if (!(el instanceof HTMLInputElement && (el.type === 'checkbox' || el.type === 'radio'))) {
    throw new TypeError('check: the target must be a checkbox or a radio button');
  }
  if (typeof checked !== 'boolean') throw new TypeError('check: the second argument must be a boolean');
  if (el.checked === checked) return;
  if (el.type === 'radio' && !checked) throw new TypeError('check: a radio button cannot be unchecked by user activation');
  el.click();
  if (el.checked !== checked) throw new Error('check: the control did not change (is it disabled?)');
}

function container(id) {
  if (typeof id !== 'string' || !/^[A-Za-z][\w-]*$/.test(id)) throw new TypeError('container: id must be a CSS identifier');
  let el = document.getElementById(id);
  if (el === null) {
    el = document.createElement('div');
    el.id = id;
    document.body.appendChild(el);
  }
  return wrap(el);
}

function unsupported(name) {
  return () => {
    throw new Error(`${name} is not available in the UI oracle`);
  };
}

// Replaces the timer, animation-frame and clock globals of the page with the fake clock.
function installFakeGlobals(clock) {
  globalThis.setTimeout = (fn, ms, ...args) => clock.setTimeout(fn, ms, ...args);
  globalThis.clearTimeout = (id) => clock.clearTimeout(id);
  globalThis.requestAnimationFrame = (cb) => clock.requestAnimationFrame(cb);
  globalThis.cancelAnimationFrame = (id) => clock.clearTimeout(id);
  globalThis.setInterval = unsupported('setInterval');
  globalThis.clearInterval = unsupported('clearInterval');
  Date.now = () => clock.now();
  performance.now = () => clock.now();
}

// Runs one scenario in this realm (trace-format.md steps 1-5). Resolves with the document as a canonical JSON line.
export async function runScenario({ scenario, module }) {
  const clock = createClock(drain);
  const rec = createRecorder();
  const open = [];
  const onRejected = (e) => open.push({ promise: e.promise, reason: e.reason });
  const onHandled = (e) => {
    const i = open.findIndex((r) => r.promise === e.promise);
    if (i >= 0) open.splice(i, 1);
  };
  const uncaught = [];
  const onError = (e) => uncaught.push(String(e.message));
  installFakeGlobals(clock);
  globalThis.trace = rec.trace;
  globalThis.testHost = {
    timers: { setTimeout: clock.setTimeout, clearTimeout: clock.clearTimeout, now: clock.now, advance: clock.advance },
    tick: drain, mount, find, findAll, click, input, change, check, select, keydown, focus, dispatch, container,
  };
  addEventListener('unhandledrejection', onRejected);
  addEventListener('rejectionhandled', onHandled);
  addEventListener('error', onError);
  try {
    let result = 'ok';
    let error = null;
    let ns = null;
    try {
      ns = await import(module);
    } catch (e) {
      result = 'throw';
      error = rec.encode(e);
    }
    await drain();
    if (ns !== null && 'run' in ns) {
      if (typeof ns.run !== 'function') throw new TypeError(`${module}: export "run" must be a function`);
      let threw = false;
      let reason;
      let p;
      try {
        p = ns.run();
      } catch (e) {
        threw = true;
        reason = e;
      }
      if (!threw) {
        try {
          await p;
        } catch (e) {
          threw = true;
          reason = e;
        }
      }
      if (threw) {
        result = 'throw';
        error = rec.encode(reason);
      }
      await drain();
    }
    await drain();
    if (uncaught.length > 0) throw new Error(`uncaught error in the page: ${uncaught[0]}`);
    const unhandledRejections = rec.encodeAll(open.map((r) => r.reason));
    return JSON.stringify({ scenario, result, error, entries: rec.entries, unhandledRejections });
  } finally {
    removeEventListener('unhandledrejection', onRejected);
    removeEventListener('rejectionhandled', onHandled);
    removeEventListener('error', onError);
  }
}

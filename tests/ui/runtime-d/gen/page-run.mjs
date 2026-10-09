// Page-side runner of the WP-RT-D oracle. Chrome runs `pageRun` through page.evaluate, so its source must be
// self-contained: every helper lives inside it. It drives the real @vue/runtime-dom 3.5.43 ESM build (loaded from the
// same origin) and records what the case observes: snapshots of elements (the canonical vn-dom format), handler logs,
// console errors (Vue's prod handleError logs through console.error), dispatch results and the values the directives
// hand to their callbacks. The Rust replay (crates/runtime/tests/rtd_common) implements the same encodings.
//
// Case schema (JSON):
//   { name, handlers: { <name>: { log: [field...], then: [action...] } }, steps: [op...] }
//   log fields: an event property name ("type", "target", ...; "<absent>" when the object lacks it), or "$" for the
//               first argument itself (a directive callback's value).
//   ops: create {id, tag, ns?} | mount {id, parent?} | patch {el, key, prev, next, ns?} | snap {el}
//        | dispatch {el, event: {kind, type, bubbles, cancelable, composed, init}} | log | time {ms} | setProp {el, key, value}
//        | remove {el} | directive {name, hook, el, binding, vnode, prevVNode?} | wrap {id, kind, fn, mods}
//        | type {el, value} | compose {el, phase} | click {el} | fire {el, type} | select {el, indices}
//   values: JSON scalars and arrays; {"$":"u"} is undefined; {"$":"fn","n":name} is a handler;
//           {"$":"ref","id":id} is a value made by a wrap op; {"$":"set","items":[...]} is a Set.
//   actions: "stopPropagation" | "stopImmediatePropagation" | "preventDefault" | "throw"
//            | {do:"patch", el, key, prev, next, ns?}
export async function pageRun(kase, modUrl) {
  'use strict';
  const RD = await import(modUrl);
  const NS = {
    html: 'http://www.w3.org/1999/xhtml',
    svg: 'http://www.w3.org/2000/svg',
    math: 'http://www.w3.org/1998/Math/MathML',
  };
  const clock = { now: 0 };
  const realNow = Date.now;
  const realError = console.error;
  const errors = [];
  const logs = [];
  const els = Object.create(null);
  const fns = Object.create(null);
  const refs = Object.create(null);
  const specs = kase.handlers || {};
  Date.now = () => clock.now;
  console.error = (...args) => {
    errors.push(args.map((a) => (a && typeof a === 'object' && 'message' in a ? String(a.message) : String(a))).join(' '));
  };

  // The log encoding of a value (the Rust replay implements the same encoding in rtd_common).
  function fmt(v) {
    if (v === null) return 'null';
    if (v === undefined) return 'undef';
    if (typeof v === 'string') return 's:' + v;
    if (typeof v === 'number') return 'n:' + String(v);
    if (typeof v === 'boolean') return 'b:' + String(v);
    if (Array.isArray(v)) return 'a[' + v.map((x) => fmt(x)).join(',') + ']';
    if (v instanceof Set) return 'S{' + Array.from(v, (x) => fmt(x)).join(',') + '}';
    if (v && v.nodeType === 1) return 'el:' + v.tagName;
    return 'o';
  }

  function fieldVal(e, f) {
    if (!(f in e)) return '<absent>';
    return fmt(e[f]);
  }

  function handlerFor(name) {
    if (fns[name]) return fns[name];
    const spec = specs[name];
    const fn = function (e) {
      const parts = [name];
      for (const f of spec.log || []) parts.push(f === '$' ? f + '=' + fmt(e) : f + '=' + fieldVal(e, f));
      logs.push(parts.join('|'));
      for (const a of spec.then || []) {
        if (a === 'throw') throw new Error('boom');
        if (a === 'stopPropagation' || a === 'stopImmediatePropagation' || a === 'preventDefault') {
          e[a]();
        } else if (a.do === 'patch') {
          RD.patchProp(els[a.el], a.key, decode(a.prev), decode(a.next), a.ns);
        } else {
          throw new Error('unknown action ' + JSON.stringify(a));
        }
      }
    };
    fns[name] = fn;
    return fn;
  }

  function decode(v) {
    if (v === undefined || v === null || typeof v !== 'object') return v;
    if (Array.isArray(v)) return v.map(decode);
    if (v.$ === 'u') return undefined;
    if (v.$ === 'fn') return handlerFor(v.n);
    if (v.$ === "ref") return refs[v.id];
    if (v.$ === "set") return new Set(v.items.map(decode));
    const o = {};
    for (const k of Object.keys(v)) o[k] = decode(v[k]);
    return o;
  }

  function snap(n) {
    const kids = (x) => Array.from(x.childNodes, snap).join(',');
    const js = (s) => JSON.stringify(s);
    if (n.nodeType === 9) return '{"$":"doc","children":[' + kids(n) + ']}';
    if (n.nodeType === 3) return '{"$":"text","v":' + js(n.data) + '}';
    if (n.nodeType === 8) return '{"$":"comment","v":' + js(n.data) + '}';
    let s = '{"$":"el","tag":' + js(n.localName);
    if (n.namespaceURI !== NS.html) s += ',"ns":' + js(n.namespaceURI === NS.svg ? 'svg' : 'math');
    s += ',"attrs":[' + Array.from(n.attributes, (a) => '[' + js(a.name) + ',' + js(a.value) + ']').join(',') + ']';
    const pv = [];
    if (n.localName === 'input' && n.namespaceURI === NS.html) {
      const t = n.type;
      const attr = n.getAttribute('value');
      const d = attr !== null ? attr : t === 'checkbox' || t === 'radio' ? 'on' : '';
      if (n.value !== d) pv.push('"value":' + js(n.value));
      if (n.checked !== n.hasAttribute('checked')) pv.push('"checked":' + (n.checked ? 'true' : 'false'));
    }
    if (n.localName === 'textarea' && n.namespaceURI === NS.html) {
      const d = n.textContent.replace(/\r\n?/g, '\n');
      if (n.value !== d) pv.push('"value":' + js(n.value));
    }
    if (n.localName === 'option' && n.namespaceURI === NS.html) {
      if (n.selected !== n.hasAttribute('selected')) pv.push('"selected":' + (n.selected ? 'true' : 'false'));
    }
    if ('disabled' in n && n.disabled !== n.hasAttribute('disabled')) {
      pv.push('"disabled":' + (n.disabled ? 'true' : 'false'));
    }
    if (pv.length) s += ',"props":{' + pv.join(',') + '}';
    s += ',"children":[' + kids(n) + ']}';
    return s;
  }

  function makeEvent(ev) {
    const init = Object.assign({ bubbles: !!ev.bubbles, cancelable: !!ev.cancelable, composed: !!ev.composed }, ev.init || {});
    switch (ev.kind) {
      case 'Mouse':
        return new MouseEvent(ev.type, init);
      case 'Keyboard':
        return new KeyboardEvent(ev.type, init);
      case 'Input':
        return new InputEvent(ev.type, init);
      default:
        return new Event(ev.type, { bubbles: init.bubbles, cancelable: init.cancelable, composed: init.composed });
    }
  }

  document.body.replaceChildren();
  const root = document.createElement('div');
  root.id = 'root';
  document.body.appendChild(root);
  els.root = root;

  function runOp(op) {
    switch (op.op) {
      case 'create': {
        const ns = op.ns || 'html';
        els[op.id] = ns === 'html' ? document.createElement(op.tag) : document.createElementNS(NS[ns], op.tag);
        return null;
      }
      case 'mount':
        (els[op.parent] || root).appendChild(els[op.id]);
        return null;
      case 'patch':
        RD.patchProp(els[op.el], op.key, decode(op.prev), decode(op.next), op.ns);
        return null;
      case 'snap':
        return snap(els[op.el]);
      case 'dispatch': {
        const ev = makeEvent(op.event);
        els[op.el].dispatchEvent(ev);
        return ev.defaultPrevented;
      }
      case 'log':
        return { log: logs.splice(0), errors: errors.splice(0) };
      case 'time':
        clock.now = op.ms;
        return null;
      case 'setProp':
        els[op.el][op.key] = op.value;
        return null;
      case 'remove':
        els[op.el].remove();
        return null;
      case 'directive': {
        const hook = RD[op.name][op.hook];
        const prevVNode = op.prevVNode === undefined ? undefined : decode(op.prevVNode);
        hook(els[op.el], decode(op.binding), decode(op.vnode), prevVNode);
        return null;
      }
      case 'wrap': {
        const wrapper = op.kind === 'withKeys' ? RD.withKeys : RD.withModifiers;
        refs[op.id] = wrapper(decode(op.fn), op.mods);
        return null;
      }
      case 'type': {
        const el = els[op.el];
        el.value = op.value;
        el.dispatchEvent(new InputEvent('input', { bubbles: true, composed: true, data: op.value }));
        return null;
      }
      case 'compose':
        els[op.el].dispatchEvent(new Event(op.phase === 'start' ? 'compositionstart' : 'compositionend', { bubbles: true, composed: true }));
        return null;
      case 'click':
        els[op.el].click();
        return null;
      case 'fire':
        els[op.el].dispatchEvent(new Event(op.type, { bubbles: true, composed: true }));
        return null;
      case 'select': {
        const el = els[op.el];
        if (el.multiple) {
          Array.from(el.options, (o, i) => {
            o.selected = op.indices.includes(i);
          });
        } else {
          el.selectedIndex = op.indices.length ? op.indices[0] : -1;
        }
        return null;
      }
      default:
        throw new Error('unknown op ' + op.op);
    }
  }

  try {
    const results = [];
    for (const op of kase.steps) {
      await null; // a microtask checkpoint per step: Vue getNow() cache and nextTick callbacks run like a fresh task
      try {
        results.push(runOp(op));
      } catch (e) {
        // a throw is an observable result: the replay must throw at the same step (the message is not compared)
        results.push({ $error: String(e && e.message !== undefined ? e.message : e) });
      }
    }
    return results;
  } finally {
    Date.now = realNow;
    console.error = realError;
  }
}

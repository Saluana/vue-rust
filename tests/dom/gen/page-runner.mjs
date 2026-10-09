// Page-side interpreter for the DOM op language (WP-U0). Chrome runs this function through page.evaluate, so its
// source must be self-contained: no references to Node-side variables, every helper lives inside it.
// Op language (the names are the switch cases in runOp): creation (el, elIs, text, comment), tree (insert, append,
// removeChild, remove, parent, firstChild, ... root), text (nodeValue, textContent and their setters), attributes
// (setAttribute, setAttributeNS, getAttribute, ...), properties (get, set, has, innerHTML, outerHTML, snapshot), style
// (styleSet, styleCamel, styleRemove, styleGet, stylePriority, cssText, setCssText), classes (classAdd, classRemove, classContains,
// classToggle, classToggleForce, classList), events (listen, unlisten, dispatch, click), focus (focus, blur, active),
// queries (byId, querySelector) and options. Listener behaviours are arrays of actions (see runActions).
// Result encoding per step: null, booleans, numbers, strings, arrays, {"$":"undefined"}, {"$":"node","h":handle} and
// {"$":"error","name":DOMException name}. Log entries record handles as plain numbers.
export function pageRunner(kase) {
  'use strict';
  const NS = {
    html: 'http://www.w3.org/1999/xhtml',
    svg: 'http://www.w3.org/2000/svg',
    math: 'http://www.w3.org/1998/Math/MathML',
  };
  const XLINK = 'http://www.w3.org/1999/xlink';
  const XML = 'http://www.w3.org/XML/1998/namespace';
  const XMLNS = 'http://www.w3.org/2000/xmlns/';
  const handles = [document]; // handle 0 is the document; other nodes are registered when first encoded
  const log = [];
  const fns = new Map(); // listener id -> function
  const added = []; // listeners to remove after the case
  const ListenerThrow = function ListenerThrow() {};

  function hOf(node) {
    let i = handles.indexOf(node);
    if (i < 0) {
      handles.push(node);
      i = handles.length - 1;
    }
    return i;
  }
  function N(i) {
    const n = handles[i];
    if (n === undefined) throw new TypeError('no such handle ' + i);
    return n;
  }
  function hv(x) {
    return x && typeof x === 'object' && x.$ === 'undefined' ? undefined : x;
  }
  function enc(v) {
    if (v === undefined) return { $: 'undefined' };
    if (v === null || typeof v === 'string' || typeof v === 'boolean') return v;
    if (typeof v === 'number') {
      if (!Number.isFinite(v)) throw new TypeError('non-finite number in a result');
      return v;
    }
    if (Array.isArray(v)) return v.map(enc);
    if (v instanceof Node) return { $: 'node', h: hOf(v) };
    throw new TypeError('unencodable result: ' + typeof v);
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
  // spec is a listener action: ['log', kind] or ['log', 'prop', name] (a primitive property of the current target).
  function logEntry(id, ev, spec) {
    const kind = spec[1];
    if (kind === 'ev') {
      return { id, type: ev.type, cur: hOf(ev.currentTarget), tgt: hOf(ev.target), phase: ev.eventPhase, prevented: ev.defaultPrevented };
    }
    if (kind === 'key') {
      return { id, key: ev.key, code: ev.code, ctrl: ev.ctrlKey, shift: ev.shiftKey, alt: ev.altKey, meta: ev.metaKey };
    }
    if (kind === 'mouse') return { id, button: ev.button, x: ev.clientX, y: ev.clientY, detail: ev.detail };
    if (kind === 'input') return { id, data: ev.data, composing: ev.isComposing };
    if (kind === 'active') {
      return { id, active: document.activeElement === null ? null : hOf(document.activeElement) };
    }
    if (kind === 'prop') return { id, prop: spec[2], value: enc(ev.currentTarget[spec[2]]) };
    throw new TypeError('unknown log kind ' + kind);
  }
  function makeEvent(kind, type, init) {
    const o = init || {};
    const base = { bubbles: !!o.bubbles, cancelable: !!o.cancelable, composed: !!o.composed };
    if (kind === 'Event') return new Event(type, base);
    if (kind === 'Mouse') {
      return new MouseEvent(type, Object.assign({}, base, { button: o.button, clientX: o.clientX, clientY: o.clientY, detail: o.detail }));
    }
    if (kind === 'Keyboard') {
      return new KeyboardEvent(type, Object.assign({}, base, {
        key: o.key, code: o.code, ctrlKey: !!o.ctrlKey, shiftKey: !!o.shiftKey, altKey: !!o.altKey, metaKey: !!o.metaKey,
      }));
    }
    if (kind === 'Input') return new InputEvent(type, Object.assign({}, base, { data: o.data === undefined ? null : o.data, isComposing: !!o.isComposing }));
    throw new TypeError('unknown event kind ' + kind);
  }
  function runActions(id, ev, actions) {
    for (const a of actions) {
      switch (a[0]) {
        case 'log': log.push(logEntry(id, ev, a)); break;
        case 'stop': ev.stopPropagation(); break;
        case 'stopImmediate': ev.stopImmediatePropagation(); break;
        case 'prevent': ev.preventDefault(); break;
        case 'throw': throw new ListenerThrow();
        case 'op': runOp(a[1]); break;
        default: throw new TypeError('unknown listener action ' + a[0]);
      }
    }
  }
  function listenerFn(id) {
    let f = fns.get(id);
    if (!f) {
      f = function (ev) {
        try {
          runActions(id, ev, (kase.listeners && kase.listeners[id]) || []);
        } catch (e) {
          // A listener that throws is reported by the DOM and dispatch goes on: record it, do not rethrow.
          if (!(e instanceof ListenerThrow) && !(e instanceof DOMException)) {
            log.push({ id, jserror: String(e && e.message) });
          }
        }
      };
      fns.set(id, f);
    }
    return f;
  }
  function runOp(op) {
    const name = op[0];
    const a = op.slice(1);
    switch (name) {
      case 'el': return a[0] === 'html' ? document.createElement(a[1]) : document.createElementNS(NS[a[0]], a[1]);
      case 'elIs': return document.createElement(a[0], { is: a[1] });
      case 'text': return document.createTextNode(a[0]);
      case 'comment': return document.createComment(a[0]);
      case 'insert': {
        const child = N(a[1]);
        N(a[0]).insertBefore(child, a[2] === null ? null : N(a[2]));
        return child;
      }
      case 'append': {
        const child = N(a[1]);
        N(a[0]).appendChild(child);
        return child;
      }
      case 'removeChild': return N(a[0]).removeChild(N(a[1]));
      case 'remove': N(a[0]).remove(); return undefined;
      case 'parent': return N(a[0]).parentNode;
      case 'firstChild': return N(a[0]).firstChild;
      case 'lastChild': return N(a[0]).lastChild;
      case 'nextSibling': return N(a[0]).nextSibling;
      case 'previousSibling': return N(a[0]).previousSibling;
      case 'childNodes': return Array.from(N(a[0]).childNodes);
      case 'contains': return N(a[0]).contains(N(a[1]));
      case 'isConnected': return N(a[0]).isConnected;
      case 'root': {
        let r = N(a[0]);
        while (r.parentNode) r = r.parentNode;
        return r;
      }
      case 'nodeName': return N(a[0]).nodeName;
      case 'nodeType': return N(a[0]).nodeType;
      case 'localName': return N(a[0]).localName ?? null;
      case 'tagName': return N(a[0]).tagName ?? null;
      case 'namespaceURI': return N(a[0]).namespaceURI ?? null;
      case 'nodeValue': return N(a[0]).nodeValue;
      case 'setNodeValue': N(a[0]).nodeValue = hv(a[1]); return undefined;
      case 'textContent': return N(a[0]).textContent;
      case 'setTextContent': N(a[0]).textContent = hv(a[1]); return undefined;
      case 'setAttribute': N(a[0]).setAttribute(a[1], hv(a[2])); return undefined;
      case 'setAttributeNS': N(a[0]).setAttributeNS(a[1], a[2], hv(a[3])); return undefined;
      case 'getAttribute': return N(a[0]).getAttribute(a[1]);
      case 'hasAttribute': return N(a[0]).hasAttribute(a[1]);
      case 'removeAttribute': N(a[0]).removeAttribute(a[1]); return undefined;
      case 'attributeNames': return N(a[0]).getAttributeNames();
      case 'get': return N(a[0])[a[1]];
      case 'set': N(a[0])[a[1]] = hv(a[2]); return undefined;
      case 'has': return a[1] in N(a[0]);
      case 'innerHTML': return N(a[0]).innerHTML;
      case 'outerHTML': return N(a[0]).outerHTML;
      case 'snapshot': return snap(N(a[0]));
      case 'styleSet': N(a[0]).style.setProperty(a[1], a[2], a[3] ? 'important' : ''); return undefined;
      case 'styleCamel': N(a[0]).style[a[1]] = hv(a[2]); return undefined;
      case 'styleRemove': return N(a[0]).style.removeProperty(a[1]);
      case 'styleGet': return N(a[0]).style.getPropertyValue(a[1]);
      case 'stylePriority': return N(a[0]).style.getPropertyPriority(a[1]);
      case 'cssText': return N(a[0]).style.cssText;
      case 'setCssText': N(a[0]).style.cssText = a[1]; return undefined;
      case 'classAdd': N(a[0]).classList.add(...a.slice(1)); return undefined;
      case 'classRemove': N(a[0]).classList.remove(...a.slice(1)); return undefined;
      case 'classContains': return N(a[0]).classList.contains(a[1]);
      case 'classToggle': return N(a[0]).classList.toggle(a[1]);
      case 'classToggleForce': return N(a[0]).classList.toggle(a[1], a[2]);
      case 'classList': return Array.from(N(a[0]).classList);
      case 'listen': {
        const node = N(a[0]);
        const fn = listenerFn(a[2]);
        node.addEventListener(a[1], fn, { capture: a[3], once: a[4], passive: a[5] });
        added.push({ node, type: a[1], fn, capture: a[3] });
        return undefined;
      }
      case 'unlisten': N(a[0]).removeEventListener(a[1], listenerFn(a[2]), a[3]); return undefined;
      case 'dispatch': return N(a[0]).dispatchEvent(makeEvent(a[1], a[2], a[3]));
      case 'click': N(a[0]).click(); return undefined;
      case 'focus': N(a[0]).focus(); return undefined;
      case 'blur': N(a[0]).blur(); return undefined;
      case 'active': return document.activeElement;
      case 'byId': return document.getElementById(a[0]);
      case 'querySelector': return N(a[0]).querySelector(a[1]);
      case 'options': return Array.from(N(a[0]).options);
      default: throw new TypeError('unknown op ' + name);
    }
  }
  // Clean slate: the document has no children and no listener from an earlier case is left behind.
  while (document.firstChild) document.removeChild(document.firstChild);
  const steps = [];
  for (const op of kase.ops) {
    let r;
    try {
      r = enc(runOp(op));
    } catch (e) {
      if (e instanceof DOMException) r = { $: 'error', name: e.name };
      else if (op[0] === 'set' && e instanceof TypeError) r = { $: 'error', name: 'TypeError' }; // strict-mode read-only set
      else throw new Error('op ' + JSON.stringify(op) + ' failed in the page: ' + e);
    }
    steps.push({ r, log: log.splice(0) });
  }
  for (const x of added) x.node.removeEventListener(x.type, x.fn, x.capture);
  if (document.activeElement) document.activeElement.blur();
  while (document.firstChild) document.removeChild(document.firstChild);
  return JSON.stringify(steps);
}

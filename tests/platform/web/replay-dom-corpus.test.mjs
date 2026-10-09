// Cross-implementation conformance: the Chrome-recorded corpora (tests/dom/corpora, recorded by tests/dom/gen/page-runner.mjs) are
// replayed through the web host. Each corpus op runs as Host operations through the real imports and glue; every step's result
// and listener log is compared with what Chrome recorded. A case that needs something outside the Host surface (or that the host
// refuses by policy) is skipped with its reason; nothing is passed silently. Corpus handle 0 (the document) is the body here.
import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { ROOT, openHostPage, reloadHost, runOps, skipOpt, startSite } from './support.mjs';

const CORPORA = path.join(ROOT, 'tests', 'dom', 'corpora');
const NS = { html: 'html', svg: 'svg', math: 'mathml' };
const CREATE = new Set(['el', 'elIs', 'text', 'comment']);
// Verbs with a Host equivalent, and where their node-handle arguments sit (positions in the op after its name).
const HANDLES = {
  insert: [0, 1, 2], append: [0, 1], removeChild: [0, 1], remove: [0], parent: [0], nextSibling: [0], isConnected: [0],
  nodeName: [0], nodeType: [0], localName: [0], tagName: [0], namespaceURI: [0], nodeValue: [0], setNodeValue: [0],
  textContent: [0], setTextContent: [0], setAttribute: [0], setAttributeNS: [0], getAttribute: [0], hasAttribute: [0],
  removeAttribute: [0], get: [0], set: [0], has: [0], innerHTML: [0], outerHTML: [0], styleSet: [0], styleRemove: [0],
  styleGet: [0], cssText: [0], setCssText: [0], classAdd: [0], classRemove: [0], listen: [0], unlisten: [0], dispatch: [0],
  click: [0], focus: [0], blur: [0], options: [0],
};
const READS = new Set(['nodeName', 'nodeType', 'localName', 'tagName', 'namespaceURI', 'nodeValue', 'textContent', 'isConnected', 'get']);
const NULL_FOR_UNDEFINED = new Set(['localName', 'tagName', 'namespaceURI']);

// Why a whole case cannot run through the Host surface (null when it can).
function skipReason(kase) {
  for (const [i, op] of kase.ops.entries()) {
    const [verb, ...a] = op;
    if (!CREATE.has(verb) && !HANDLES[verb] && !['active', 'byId'].includes(verb)) return `verb '${verb}' has no Host operation`;
    if (verb === 'removeChild' && kase.steps[i].r?.$ === 'error') return 'removeChild with a parent check the Host does not model';
    if (verb === 'classAdd' && a.length > 2) return 'classAdd with several tokens (the Host adds one token per call)';
    if (verb === 'classAdd' && a.length === 1) return 'classAdd with no tokens (the Host operation takes one token)';
    if ((verb === 'get' || verb === 'set') && ['href', 'src'].includes(a[1])) {
      return `${a[1]} reflects the resolved URL; the corpus was recorded without a document base URL`;
    }
    if (verb === 'byId' && !/^[A-Za-z][\w-]*$/.test(a[0])) return `byId('${a[0]}') is outside the selector subset`;
    if (HANDLES[verb]) {
      const positions = HANDLES[verb].filter((p) => p < a.length && a[p] !== null);
      for (const p of positions) {
        // Handle 0 (the document) is only allowed as a parent of insert and append, where it means the body.
        if (a[p] === 0 && !(p === 0 && ['insert', 'append'].includes(verb))) return 'the document is used as a node argument';
      }
    }
  }
  for (const [id, actions] of Object.entries(kase.listeners ?? {})) {
    const kinds = actions.map((x) => x[0]);
    if (actions.some((x) => x[0] === 'log' && !['ev', 'key', 'mouse', 'input'].includes(x[1]))) return `listener ${id} logs a kind the replay does not model`;
    if (actions.some((x) => x[0] === 'op')) return `listener ${id} runs corpus operations`;
    if (kinds.indexOf('throw') >= 0 && kinds.indexOf('throw') !== kinds.length - 1) return `listener ${id} logs after throw`;
  }
  return null;
}

// Corpus values: the recorded encoding of a result (see page-runner.mjs `enc`).
const isNodeRef = (r) => r !== null && typeof r === 'object' && r.$ === 'node';
const isErrorRef = (r) => r !== null && typeof r === 'object' && r.$ === 'error';

function hostValueToCorpus(hv) {
  if (hv.k === 'undef') return { $: 'undefined' };
  if (hv.k === 'null') return null;
  return hv.v;
}

function corpusValue(v) {
  if (v === undefined || (v !== null && typeof v === 'object' && v.$ === 'undefined')) return { k: 'undef' };
  if (v === null) return { k: 'null' };
  if (typeof v === 'boolean') return { k: 'bool', v };
  if (typeof v === 'number') return { k: 'num', v };
  return { k: 'str', v: String(v) };
}

// DOM String conversion of a corpus argument (setAttribute converts undefined to "undefined" and null to "null").
function domString(v) {
  if (v !== null && typeof v === 'object' && v.$ === 'undefined') return 'undefined';
  return String(v);
}

// The DOM exception name carried by a host error: the glue puts it at the start of the message.
function errorName(err) {
  const m = /^([A-Za-z]+Error): /.exec(err.msg ?? '');
  if (m) return m[1];
  if (/cannot take focus|cannot lose focus/.test(err.msg ?? '')) return 'TypeError'; // non-element focus: the DOM's TypeError
  return err.kind;
}

class Replay {
  constructor(page, kase) {
    this.page = page;
    this.kase = kase;
    this.map = new Map([[0, null]]); // corpus handle -> host handle; corpus 0 is the document, mapped to the body below
    this.rev = new Map();
    this.body = null;
    // Listener id -> passive flag of its registration (the corpus `listen` op carries it).
    // (listen op: [name, node, type, id, capture, once, passive])
    this.passive = new Map(kase.ops.filter((op) => op[0] === 'listen').map((op) => [op[3], !!op[6]]));
  }

  async host(...op) {
    const [r] = await runOps(this.page, [op]);
    return r;
  }

  async init() {
    const r = await this.host('body');
    this.body = r.ok;
    this.map.set(0, this.body);
    this.rev.set(this.body, 0);
  }

  handle(idx) {
    if (!this.map.has(idx) || this.map.get(idx) === null) throw new Error(`corpus handle ${idx} is not known to the replay`);
    return this.map.get(idx);
  }

  // Learns or checks a corpus node reference against a host handle (null when the host reports no node).
  matchNode(expected, handle) {
    if (expected === null) return handle === null;
    if (!isNodeRef(expected)) return false;
    if (handle === null) return false;
    if (!this.map.has(expected.h)) {
      this.map.set(expected.h, handle);
      this.rev.set(handle, expected.h);
      return true;
    }
    return this.map.get(expected.h) === handle;
  }

  async execute(op, expected) {
    const [verb, ...a] = op;
    if (CREATE.has(verb)) {
      let r;
      if (verb === 'el') r = await this.host('create_element', NS[a[0]], a[1], null);
      else if (verb === 'elIs') r = await this.host('create_element', 'html', a[0], a[1]);
      else if (verb === 'text') r = await this.host('create_text', a[0]);
      else r = await this.host('create_comment', a[0]);
      if ('err' in r) return { error: errorName(r.err), unsupported: r.err.kind === 'Unsupported' ? r.err.msg : null };
      if (isNodeRef(expected)) {
        this.map.set(expected.h, r.ok);
        this.rev.set(r.ok, expected.h);
      }
      return { node: r.ok };
    }
    const h = () => this.handle(a[0]);
    switch (verb) {
      case 'insert':
      case 'append': {
        const parent = this.handle(a[0]);
        const child = this.handle(a[1]);
        const anchor = verb === 'insert' && a[2] !== null ? this.handle(a[2]) : null;
        const r = await this.host('insert', child, parent, anchor);
        return 'err' in r ? { error: errorName(r.err) } : { node: child };
      }
      case 'removeChild':
      case 'remove': {
        const child = this.handle(a[verb === 'removeChild' ? 1 : 0]);
        const r = await this.host('remove', child);
        return 'err' in r ? { error: errorName(r.err) } : verb === 'removeChild' ? { node: child } : { value: undefined };
      }
      case 'parent': {
        const r = await this.host('parent_node', h());
        return 'err' in r ? { error: errorName(r.err) } : { node: r.ok };
      }
      case 'nextSibling': {
        const r = await this.host('next_sibling', h());
        return 'err' in r ? { error: errorName(r.err) } : { node: r.ok };
      }
      case 'isConnected':
      case 'nodeName':
      case 'nodeType':
      case 'localName':
      case 'tagName':
      case 'namespaceURI':
      case 'nodeValue':
      case 'textContent':
      case 'get': {
        const name = verb === 'get' ? a[1] : verb;
        const r = await this.host('get_property', h(), name);
        if ('err' in r) return { error: errorName(r.err), unsupported: r.err.kind === 'Unsupported' ? r.err.msg : null };
        let value = hostValueToCorpus(r.ok);
        if (value !== null && typeof value === 'object' && value.$ === 'undefined' && NULL_FOR_UNDEFINED.has(verb)) value = null;
        return { value };
      }
      case 'has': {
        const r = await this.host('has_property', h(), a[1]);
        return { value: r.ok };
      }
      case 'setNodeValue':
      case 'setTextContent': {
        const name = verb === 'setNodeValue' ? 'nodeValue' : 'textContent';
        const r = await this.host('set_property', h(), name, corpusValue(a[1]));
        return 'err' in r ? { error: errorName(r.err), unsupported: r.err.kind === 'Unsupported' ? r.err.msg : null } : { value: undefined };
      }
      case 'set': {
        const r = await this.host('set_property', h(), a[1], corpusValue(a[2]));
        return 'err' in r ? { error: errorName(r.err), unsupported: r.err.kind === 'Unsupported' ? r.err.msg : null } : { value: undefined };
      }
      case 'setAttribute': {
        const r = await this.host('set_attribute', h(), a[1], domString(a[2]));
        return 'err' in r ? { error: errorName(r.err) } : { value: undefined };
      }
      case 'setAttributeNS': {
        const r = await this.host('set_attribute_ns', h(), a[1], a[2], domString(a[3]));
        return 'err' in r ? { error: errorName(r.err) } : { value: undefined };
      }
      case 'getAttribute': {
        const r = await this.host('get_attribute', h(), a[1]);
        return { value: r.ok ?? null };
      }
      case 'hasAttribute': {
        const r = await this.host('has_attribute', h(), a[1]);
        return { value: r.ok };
      }
      case 'removeAttribute': {
        const r = await this.host('remove_attribute', h(), a[1]);
        return { value: undefined };
      }
      case 'innerHTML':
      case 'outerHTML': {
        const r = await this.host(verb === 'innerHTML' ? 'inner_html' : 'outer_html', h());
        return { value: r.ok ?? null };
      }
      case 'styleSet': {
        const r = await this.host('style_set_property', h(), a[1], a[2], !!a[3]);
        return 'err' in r ? { error: errorName(r.err), unsupported: r.err.kind === 'Unsupported' ? r.err.msg : null } : { value: undefined };
      }
      case 'styleRemove': {
        const old = await this.host('style_get_property', h(), a[1]);
        const r = await this.host('style_remove_property', h(), a[1]);
        return 'err' in r ? { error: errorName(r.err) } : { value: old.ok };
      }
      case 'styleGet': {
        const r = await this.host('style_get_property', h(), a[1]);
        return { value: r.ok };
      }
      case 'cssText': {
        const r = await this.host('style_css_text', h());
        return { value: r.ok };
      }
      case 'setCssText': {
        const r = await this.host('style_set_css_text', h(), a[1]);
        return 'err' in r ? { error: errorName(r.err) } : { value: undefined };
      }
      case 'classAdd':
      case 'classRemove': {
        const r = await this.host(verb === 'classAdd' ? 'class_list_add' : 'class_list_remove', h(), a[1]);
        return 'err' in r ? { error: errorName(r.err) } : { value: undefined };
      }
      case 'listen': {
        const r = await this.host('add_event_listener', h(), a[1], a[2], { capture: !!a[3], once: !!a[4], passive: !!a[5] });
        return 'err' in r ? { error: errorName(r.err) } : { value: undefined };
      }
      case 'unlisten': {
        const r = await this.host('remove_event_listener', h(), a[1], a[2], !!a[3]);
        return 'err' in r ? { error: errorName(r.err) } : { value: undefined };
      }
      case 'dispatch': {
        const value = await this.page.evaluate(
          ([handle, kind, type, init]) => window.vnHost.dispatchEvent(handle, kind, type, init),
          [h(), a[1], a[2], a[3] ?? {}],
        );
        return { value };
      }
      case 'click': {
        await this.page.evaluate((handle) => window.vnHost.click(handle), h());
        return { value: undefined };
      }
      case 'focus':
      case 'blur': {
        const r = await this.host(verb, h());
        return 'err' in r ? { error: errorName(r.err) } : { value: undefined };
      }
      case 'options': {
        const r = await this.host('option_nodes', h());
        return { nodes: r.ok };
      }
      case 'active': {
        const r = await this.host('active_element');
        return { node: r.ok ?? null, active: true };
      }
      case 'byId': {
        const r = await this.host('query_selector', `#${a[0]}`);
        if ('err' in r) return { error: errorName(r.err) };
        return { node: r.ok };
      }
      default:
        throw new Error(`no replay for verb ${verb}`);
    }
  }

  // Compares one step with what Chrome recorded. Returns a mismatch description, or null.
  compare(op, expected, got) {
    if (isErrorRef(expected)) {
      return got.error === expected.name ? null : `expected ${expected.name}, got ${got.error ?? JSON.stringify(got.value)}`;
    }
    if (got.error !== undefined) return `expected ${JSON.stringify(expected)}, got error ${got.error}`;
    if ('nodes' in got) {
      if (!Array.isArray(expected) || expected.length !== got.nodes.length) return 'node list length differs';
      return expected.every((e, i) => this.matchNode(e, got.nodes[i])) ? null : 'node list differs';
    }
    if ('node' in got) {
      if (got.active && got.node === null) {
        // The browser reports the body when nothing has focus; the host reports no element.
        return isNodeRef(expected) && !this.map.has(expected.h) ? this.learnBody(expected) : expected === null ? null : 'active differs';
      }
      return this.matchNode(expected, got.node) ? null : `node ${JSON.stringify(expected)} differs`;
    }
    const want = expected;
    const have = got.value === undefined ? { $: 'undefined' } : got.value;
    return JSON.stringify(want) === JSON.stringify(have) ? null : `expected ${JSON.stringify(want)}, got ${JSON.stringify(have)}`;
  }

  learnBody(expected) {
    this.map.set(expected.h, this.body);
    this.rev.set(this.body, expected.h);
    return null;
  }

  // Corpus-encoded log entries for the events the host delivered during one step.
  translateLog(entries) {
    const out = [];
    const rev = (h) => (this.rev.has(h) ? this.rev.get(h) : null);
    for (const e of entries) {
      if (e.kind !== 'event') continue;
      const actions = this.kase.listeners?.[String(e.listener)] ?? [];
      const passive = this.passive.get(e.listener) === true;
      let prevented = e.default_prevented;
      for (const action of actions) {
        // preventDefault sets the flag only on a cancelable event, and a passive listener's call is ignored (DOM rules).
        if (action[0] === 'prevent' && e.cancelable && !passive) prevented = true;
        if (action[0] !== 'log') continue;
        const id = e.listener;
        if (action[1] === 'ev') out.push({ id, type: e.ty, cur: rev(e.current), tgt: rev(e.target), phase: e.phase, prevented });
        else if (action[1] === 'key') out.push({ id, key: e.key, code: e.code, ctrl: e.ctrl, shift: e.shift, alt: e.alt, meta: e.meta });
        else if (action[1] === 'mouse') out.push({ id, button: e.button, x: e.client_x, y: e.client_y, detail: e.detail });
        else if (action[1] === 'input') out.push({ id, data: e.input_data, composing: e.is_composing });
      }
    }
    return out;
  }
}

async function replay(t, page, site, kase) {
  const reason = skipReason(kase);
  if (reason) return t.skip(reason);
  await reloadHost(page, site);
  const behaviours = Object.entries(kase.listeners ?? {});
  const rp = new Replay(page, kase);
  await rp.init();
  for (const [id, actions] of behaviours) {
    const kinds = actions.map((x) => x[0]);
    await runOps(page, [['__behave', Number(id), {
      stop: kinds.includes('stop'),
      stop_immediate: kinds.includes('stopImmediate'),
      prevent: kinds.includes('prevent'),
      throw: kinds.includes('throw'),
    }]]);
  }
  for (let i = 0; i < kase.ops.length; i++) {
    const op = kase.ops[i];
    const expected = kase.steps[i].r;
    const got = await rp.execute(op, expected);
    if (got.unsupported) return t.skip(`step ${i} ${op[0]}: refused by host policy (${got.unsupported})`);
    const [drained] = await runOps(page, [['__log']]);
    const logs = drained.ok ?? [];
    const mismatch = rp.compare(op, expected, got);
    assert.equal(mismatch, null, `step ${i} ${JSON.stringify(op)}: ${mismatch}`);
    assert.deepEqual(rp.translateLog(logs), kase.steps[i].log, `step ${i} ${JSON.stringify(op)}: listener log`);
  }
}

describe('Chrome-recorded DOM corpora replayed through the web host', skipOpt, () => {
  let site;
  let page;
  before(async () => {
    site = await startSite();
    page = await openHostPage(site);
  });
  after(async () => {
    await page?.close();
    await site?.close();
  });

  for (const file of fs.readdirSync(CORPORA).filter((f) => f.endsWith('.json')).sort()) {
    const cases = JSON.parse(fs.readFileSync(path.join(CORPORA, file), 'utf8'));
    describe(file, () => {
      for (const kase of cases) {
        test(kase.name, async (t) => replay(t, page, site, kase));
      }
    });
  }
});

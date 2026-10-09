// Case builder for the DOM oracle corpora (WP-U0). A case is a list of ops (see page-runner.mjs) plus listener
// behaviours. Handles: 0 is the document and each successful creation op allocates the next handle in order. Ops
// that fail to create a node must use b.op(...) directly, so no handle is allocated for them. Cases only reference
// nodes that were created explicitly (never text nodes made implicitly by textContent), which keeps handles stable.
export const UNDEF = { $: 'undefined' };
export const CASES = [];

export const LOG_EV = ['log', 'ev'];
export const LOG_KEY = ['log', 'key'];
export const LOG_MOUSE = ['log', 'mouse'];
export const LOG_INPUT = ['log', 'input'];

class Builder {
  constructor() {
    this.ops = [];
    this.listeners = {};
    this.next = 1;
  }

  // Appends any op and returns its index in the op list.
  op(...op) {
    this.ops.push(op);
    return this.ops.length - 1;
  }

  // Creation ops: each allocates the next handle.
  el(ns, tag) {
    this.ops.push(['el', ns, tag]);
    return this.next++;
  }

  elIs(tag, is) {
    this.ops.push(['elIs', tag, is]);
    return this.next++;
  }

  text(data) {
    this.ops.push(['text', data]);
    return this.next++;
  }

  comment(data) {
    this.ops.push(['comment', data]);
    return this.next++;
  }

  // Listener behaviour: actions run in order each time the listener fires (see runActions in page-runner.mjs).
  on(id, ...actions) {
    this.listeners[id] = actions;
  }

  listen(node, type, id, { capture = false, once = false, passive = false } = {}) {
    return this.op('listen', node, type, id, capture, once, passive);
  }
}

// Registers a named case. Names are "<group>/<case>"; the group picks the corpus file.
export function def(name, build) {
  if (!/^[a-z]+\/[A-Za-z0-9-]+$/.test(name)) throw new Error('bad case name ' + name);
  if (CASES.some((c) => c.name === name)) throw new Error('duplicate case name ' + name);
  const b = new Builder();
  build(b);
  CASES.push({ name, ops: b.ops, listeners: b.listeners });
}

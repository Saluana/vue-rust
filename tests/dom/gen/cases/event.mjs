// event/*: dispatch path and phases, stopPropagation, once, duplicate listeners, mutation during dispatch, cancelable and
// passive handling, UI payloads, and the click() method (WP-U0). Listener logs are recorded per step.
import { def, LOG_EV, LOG_KEY, LOG_MOUSE, LOG_INPUT } from '../builder.mjs';

// Builds a chain of elements, each appended to the previous one; the first is attached to the document unless
// `attach` is false. Returns the handles from outermost to innermost.
function chain(b, tags, attach = true) {
  const out = [];
  for (const tag of tags) {
    const e = b.el('html', tag);
    if (out.length) b.op('append', out[out.length - 1], e);
    else if (attach) b.op('append', 0, e);
    out.push(e);
  }
  return out;
}

def('event/path-capture-target-bubble', (b) => {
  const [root, outer, t] = chain(b, ['main', 'section', 'p']);
  b.listen(root, 'x', 1, { capture: true });
  b.listen(outer, 'x', 2, { capture: true });
  b.listen(t, 'x', 3, { capture: true });
  b.listen(t, 'x', 4);
  b.listen(outer, 'x', 5);
  b.listen(root, 'x', 6);
  for (let i = 1; i <= 6; i++) b.on(i, LOG_EV);
  b.op('dispatch', t, 'Event', 'x', { bubbles: true });
});

def('event/non-bubbling-skips-ancestor-bubble', (b) => {
  const [root, outer, t] = chain(b, ['main', 'section', 'p']);
  b.listen(root, 'x', 1, { capture: true });
  b.listen(outer, 'x', 2, { capture: true });
  b.listen(t, 'x', 3, { capture: true });
  b.listen(t, 'x', 4);
  b.listen(outer, 'x', 5);
  b.listen(root, 'x', 6);
  for (let i = 1; i <= 6; i++) b.on(i, LOG_EV);
  b.op('dispatch', t, 'Event', 'x', { bubbles: false });
});

def('event/stop-in-target-capture-blocks-target-bubble', (b) => {
  const [, outer, t] = chain(b, ['main', 'section', 'i']);
  b.listen(t, 'y', 1, { capture: true });
  b.listen(t, 'y', 2);
  b.listen(outer, 'y', 3);
  b.on(1, LOG_EV, ['stop']);
  b.on(2, LOG_EV);
  b.on(3, LOG_EV);
  b.op('dispatch', t, 'Event', 'y', { bubbles: true });
});

def('event/stop-immediate-same-node', (b) => {
  const [, outer, t] = chain(b, ['main', 'section', 'b']);
  b.listen(t, 'z', 1);
  b.listen(t, 'z', 2);
  b.listen(outer, 'z', 3);
  b.on(1, LOG_EV, ['stopImmediate']);
  b.on(2, LOG_EV);
  b.on(3, LOG_EV);
  b.op('dispatch', t, 'Event', 'z', { bubbles: true });
});

def('event/stop-keeps-same-node-listeners', (b) => {
  const [, outer, t] = chain(b, ['main', 'section', 'em']);
  b.listen(t, 'w', 1);
  b.listen(t, 'w', 2);
  b.listen(outer, 'w', 3);
  b.on(1, LOG_EV, ['stop']);
  b.on(2, LOG_EV);
  b.on(3, LOG_EV);
  b.op('dispatch', t, 'Event', 'w', { bubbles: true });
});

def('event/stop-at-ancestor-capture-blocks-target', (b) => {
  const [root, , t] = chain(b, ['main', 'section', 'p']);
  b.listen(root, 'x', 1, { capture: true });
  b.listen(t, 'x', 2);
  b.listen(t, 'x', 3, { capture: true });
  b.on(1, LOG_EV, ['stop']);
  b.on(2, LOG_EV);
  b.on(3, LOG_EV);
  b.op('dispatch', t, 'Event', 'x', { bubbles: true });
});

def('event/stop-in-bubble-blocks-ancestors', (b) => {
  const [, outer, t] = chain(b, ['main', 'section', 'p']);
  b.listen(t, 'b', 1);
  b.listen(t, 'b', 2);
  b.listen(outer, 'b', 3);
  b.on(1, LOG_EV, ['stop']);
  b.on(2, LOG_EV);
  b.on(3, LOG_EV);
  b.op('dispatch', t, 'Event', 'b', { bubbles: true });
});

def('event/once-runs-once', (b) => {
  const t = b.el('html', 'u');
  b.listen(t, 'o', 1, { once: true });
  b.on(1, LOG_EV);
  b.op('dispatch', t, 'Event', 'o', {});
  b.op('dispatch', t, 'Event', 'o', {});
});

def('event/duplicate-add-ignored-including-options', (b) => {
  const t = b.el('html', 's');
  b.listen(t, 'd', 1);
  b.listen(t, 'd', 1);
  b.listen(t, 'd', 1, { once: true });
  b.on(1, LOG_EV);
  b.op('dispatch', t, 'Event', 'd', {});
  b.op('dispatch', t, 'Event', 'd', {});
});

def('event/capture-variant-is-distinct', (b) => {
  const t = b.el('html', 'q');
  b.listen(t, 'd', 1);
  b.listen(t, 'd', 1, { capture: true });
  b.on(1, LOG_EV);
  b.op('dispatch', t, 'Event', 'd', {});
});

def('event/remove-with-matching-capture', (b) => {
  const t = b.el('html', 'kbd');
  b.listen(t, 'f', 2);
  b.op('unlisten', t, 'f', 2, false);
  b.on(2, LOG_EV);
  b.op('dispatch', t, 'Event', 'f', {});
  b.listen(t, 'f', 2, { capture: true });
  b.op('unlisten', t, 'f', 2, false);
  b.op('dispatch', t, 'Event', 'f', {});
});

def('event/prevent-only-when-cancelable', (b) => {
  const t = b.el('html', 'span');
  b.listen(t, 'p', 1);
  b.on(1, ['prevent'], LOG_EV);
  b.op('dispatch', t, 'Event', 'p', { cancelable: false });
  b.op('dispatch', t, 'Event', 'p', { cancelable: true });
});

def('event/passive-ignores-prevent', (b) => {
  const t = b.el('html', 'kbd');
  b.listen(t, 'p', 1, { passive: true });
  b.on(1, ['prevent'], LOG_EV);
  b.op('dispatch', t, 'Event', 'p', { cancelable: true });
});

def('event/throwing-listener-continues', (b) => {
  const [, parent, child] = chain(b, ['main', 'section', 'i']);
  b.listen(child, 'e', 1);
  b.listen(child, 'e', 2);
  b.listen(parent, 'e', 3);
  b.on(1, LOG_EV, ['throw']);
  b.on(2, LOG_EV);
  b.on(3, LOG_EV);
  b.op('dispatch', child, 'Event', 'e', { bubbles: true });
});

def('event/listener-added-during-dispatch', (b) => {
  const [outer, t] = chain(b, ['section', 'p']);
  b.listen(t, 'g', 1);
  b.on(1, LOG_EV, ['op', ['listen', outer, 'g', 2, false, false, false]], ['op', ['listen', t, 'g', 3, false, false, false]]);
  b.on(2, LOG_EV);
  b.on(3, LOG_EV);
  b.op('dispatch', t, 'Event', 'g', { bubbles: true });
  b.op('dispatch', t, 'Event', 'g', { bubbles: true });
});

def('event/listener-removed-during-dispatch', (b) => {
  const t = b.el('html', 'cite');
  b.listen(t, 'k', 1);
  b.listen(t, 'k', 2);
  b.on(1, LOG_EV, ['op', ['unlisten', t, 'k', 2, false]]);
  b.on(2, LOG_EV);
  b.op('dispatch', t, 'Event', 'k', {});
  b.op('dispatch', t, 'Event', 'k', {});
});

def('event/removed-on-later-node-is-skipped', (b) => {
  const [outer, t] = chain(b, ['section', 'abbr']);
  b.listen(t, 'm', 1);
  b.listen(outer, 'm', 2);
  b.on(1, LOG_EV, ['op', ['unlisten', outer, 'm', 2, false]]);
  b.on(2, LOG_EV);
  b.op('dispatch', t, 'Event', 'm', { bubbles: true });
});

def('event/once-listener-that-throws-is-removed', (b) => {
  const t = b.el('html', 'bdi');
  b.listen(t, 'n', 1, { once: true });
  b.on(1, LOG_EV, ['throw']);
  b.op('dispatch', t, 'Event', 'n', {});
  b.op('dispatch', t, 'Event', 'n', {});
});

def('event/payload-keyboard', (b) => {
  const t = b.el('html', 'p');
  b.listen(t, 'keydown', 1);
  b.on(1, LOG_KEY);
  b.op('dispatch', t, 'Keyboard', 'keydown', { key: 'Enter', code: 'Enter', ctrlKey: true, bubbles: true });
  b.op('dispatch', t, 'Keyboard', 'keydown', {});
});

def('event/payload-mouse', (b) => {
  const t = b.el('html', 'p');
  b.listen(t, 'click', 1);
  b.on(1, LOG_MOUSE);
  b.op('dispatch', t, 'Mouse', 'click', { button: 2, clientX: 5, clientY: 7, detail: 1, bubbles: true });
});

def('event/payload-input-data', (b) => {
  const t = b.el('html', 'p');
  b.listen(t, 'input', 1);
  b.on(1, LOG_INPUT);
  b.op('dispatch', t, 'Input', 'input', { data: 'ab', isComposing: true, bubbles: true });
  b.op('dispatch', t, 'Input', 'input', {});
});

def('event/default-init-values', (b) => {
  const t = b.el('html', 'p');
  b.listen(t, 'x', 1);
  b.on(1, LOG_EV);
  b.op('dispatch', t, 'Event', 'x', {});
});

def('event/click-method-bubbles-cancelable', (b) => {
  const [parent, child] = chain(b, ['section', 'span']);
  b.listen(parent, 'click', 1);
  b.listen(child, 'click', 2);
  b.on(1, LOG_EV);
  b.on(2, LOG_EV);
  b.op('click', child);
});

def('event/click-dispatch-prevent-result', (b) => {
  const t = b.el('html', 'span');
  b.listen(t, 'click', 1);
  b.on(1, ['prevent'], LOG_EV);
  b.op('dispatch', t, 'Mouse', 'click', { bubbles: true, cancelable: true });
  b.op('dispatch', t, 'Mouse', 'click', { bubbles: true, cancelable: false });
});

def('event/click-method-skips-disabled-control', (b) => {
  const bt = b.el('html', 'button');
  b.op('setAttribute', bt, 'disabled', '');
  b.listen(bt, 'click', 1);
  b.on(1, LOG_EV);
  b.op('click', bt);
  b.op('dispatch', bt, 'Mouse', 'click', { bubbles: true, cancelable: true });
});

def('event/document-listener-on-attached-path-only', (b) => {
  const [, t] = chain(b, ['main', 'p']);
  const d = b.el('html', 'div');
  b.listen(0, 'x', 2);
  b.listen(t, 'x', 1);
  b.listen(d, 'x', 1);
  b.on(1, LOG_EV);
  b.on(2, LOG_EV);
  b.op('dispatch', t, 'Event', 'x', { bubbles: true });
  b.op('dispatch', d, 'Event', 'x', { bubbles: true });
});

def('event/registration-order-same-node', (b) => {
  const t = b.el('html', 'p');
  b.listen(t, 'r', 1);
  b.listen(t, 'r', 2);
  b.listen(t, 'r', 3);
  b.on(1, LOG_EV);
  b.on(2, LOG_EV);
  b.on(3, LOG_EV);
  b.op('dispatch', t, 'Event', 'r', {});
});

def('event/detached-chain-never-reaches-document', (b) => {
  const [d1, d2] = chain(b, ['div', 'span'], false);
  b.listen(d1, 'x', 1);
  b.listen(d2, 'x', 2);
  b.listen(0, 'x', 3);
  b.on(1, LOG_EV);
  b.on(2, LOG_EV);
  b.on(3, LOG_EV);
  b.op('dispatch', d2, 'Event', 'x', { bubbles: true });
});

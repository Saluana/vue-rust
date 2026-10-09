// Shared helpers for the WP-U1a oracle corpora (crates/css). Every expected value is produced by headless
// Chrome through tooling/chrome (WP-X1), never by the Rust code. Deterministic: a fixed-seed xorshift64* PRNG,
// no clock or environment input, and stable JSON key order, so a rerun rewrites byte-identical files.
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export const CORPORA_DIR = join(dirname(fileURLToPath(import.meta.url)), '..', 'corpora');

if (Number(process.versions.node.split('.')[0]) < 22) {
  throw new Error(`node >= 22 required, got ${process.version}`);
}

// xorshift64* with a per-generator seed. BigInt keeps the 64-bit arithmetic exact.
const M64 = (1n << 64n) - 1n;
export function makeRng(seed) {
  let s = BigInt(seed) & M64;
  if (s === 0n) s = 1n;
  const next64 = () => {
    s ^= s >> 12n;
    s ^= (s << 25n) & M64;
    s ^= s >> 27n;
    return (s * 0x2545f4914f6cdd1dn) & M64;
  };
  const int = (n) => Number(next64() % BigInt(n)); // 0 <= r < n
  return {
    int,
    pick: (a) => a[int(a.length)],
    chance: (p) => int(1000000) < p * 1000000,
    range: (lo, hi) => lo + int(hi - lo + 1), // inclusive
    shuffle(a) {
      const out = a.slice();
      for (let i = out.length - 1; i > 0; i--) {
        const j = int(i + 1);
        [out[i], out[j]] = [out[j], out[i]];
      }
      return out;
    },
  };
}

// Writes a corpus as JSON with a trailing newline. `outDir` defaults to tests/css/corpora.
export function writeCorpus(name, value, outDir = CORPORA_DIR) {
  mkdirSync(outDir, { recursive: true });
  const file = join(outDir, name);
  writeFileSync(file, JSON.stringify(value) + '\n');
  return file;
}

// Fixture element trees are plain data: {t: tag, a: [[name, value]...], c: [children], s: {state}}, with text
// children {x: text}, comment children {m: text}, and `cp: true` for a checkbox/radio whose checked property
// (not attribute) is set, and `fo: true` for the element that receives focus().
export function countElements(desc) {
  let n = 1;
  for (const c of desc.c || []) if (c.t !== undefined) n += countElements(c);
  return n;
}

// Attaches Chrome's states (pre-order, elements only) to a tree description, in place.
export function attachStates(desc, states) {
  let i = 0;
  const walk = (d) => {
    d.s = states[i++];
    for (const c of d.c || []) if (c.t !== undefined) walk(c);
  };
  walk(desc);
  if (i !== states.length) throw new Error(`state count ${states.length} for ${i} elements`);
  return desc;
}

// Builds the DOM for a description inside the page (self-contained: it is sent as source). focusIndex is the
// pre-order element index to focus() (-1 for none). Returns the element count.
export const BUILD_IN_PAGE = (root, focusIndex) => {
  const make = (d) => {
    if (d.x !== undefined) return document.createTextNode(d.x);
    if (d.m !== undefined) return document.createComment(d.m);
    const el = document.createElement(d.t);
    for (const [k, v] of d.a) el.setAttribute(k, v);
    for (const c of d.c || []) el.appendChild(make(c));
    if (d.cp) el.checked = true;
    return el;
  };
  // html, head and body persist across builds: clear their attributes and focus before applying this tree.
  if (document.activeElement && document.activeElement !== document.body) document.activeElement.blur();
  for (const el of [document.documentElement, document.head, document.body]) {
    for (const attr of Array.from(el.attributes)) el.removeAttribute(attr.name);
  }
  const [head, body] = root.c;
  for (const [k, v] of root.a) document.documentElement.setAttribute(k, v);
  for (const [k, v] of head.a) document.head.setAttribute(k, v);
  for (const [k, v] of body.a) document.body.setAttribute(k, v);
  document.head.replaceChildren(...(head.c || []).map(make));
  document.body.replaceChildren(...(body.c || []).map(make));
  const all = Array.from(document.getElementsByTagName("*"));
  if (focusIndex >= 0) all[focusIndex].focus();
  return all.length;
};

// Chrome states of every element in document order (pre-order, matching the description numbering).
export const PAGE_STATES = () =>
  Array.from(document.getElementsByTagName("*"), (el) => ({
    hover: el.matches(":hover"),
    active: el.matches(":active"),
    focus: el.matches(":focus"),
    focusVisible: el.matches(":focus-visible"),
    focusWithin: el.matches(":focus-within"),
    disabled: el.matches(":disabled"),
    enabled: el.matches(":enabled"),
    checked: el.matches(":checked"),
    empty: el.matches(":empty"),
  }));

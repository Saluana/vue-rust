// Emitter state (WP-C1b, extended by WP-C1c): the line buffer of the Rust function being written, temporaries, the module
// string pool and the control-flow stack that decides how a jump is written (a Rust break/continue, or a Flow value
// returned out of a closure that implements try/finally or a for-of body).
//
// Async functions (WP-C1c). A Rust `async` body keeps no GC reference in a Rust local across an await: every Value
// temporary and every binding of the function lives in a heap frame `fr` (frame_get/frame_set). In an async function
// tmp() returns the read expression of a fresh frame slot, and def/set/defObj/defKey write the slot; in a sync function
// they write Rust locals, so sync output is unchanged. The slot of each name is kept in `frameOf`.
import { rstr } from './names.mjs';

export class Emitter {
  constructor(hir) {
    this.hir = hir;
    this.lines = [];
    this.depth = 1;
    this.tempN = 0;
    this.strings = [];
    this.strIndex = new Map();
    this.ctl = [];        // control stack, innermost last (see pushTarget / pushClosure)
    this.fnId = -1;       // function being emitted (-1: module phase)
    this.rec = null;      // its HIR record
    this.closureN = 0;
    this.async = false;   // emitting an async function body
    this.fs = null;       // frame state of the async function being emitted: { next, slots: Map key -> slot }
    this.frameOf = new Map(); // async temporary (read expression) -> frame slot
  }

  line(s) {
    this.lines.push(`${'    '.repeat(this.depth)}${s}`);
  }

  open(s) {
    this.line(`${s} {`);
    this.depth += 1;
  }

  close(suffix = '}') {
    this.depth -= 1;
    this.line(suffix);
  }

  tmp() {
    if (!this.async) return `t${this.tempN++}`;
    const s = this.newSlot();
    const name = `frame_get(rt, fr, ${s})`;
    this.frameOf.set(name, s);
    return name;
  }

  uniq(prefix) {
    return `${prefix}${this.tempN++}`;
  }

  // Frame slots of the async function being emitted (WP-C1c). Slots 0 and 1 are `this` and the callee; the captured cells
  // and the parameters follow (startFrame), then temporaries and bindings in first-use order.
  startFrame(captures, params) {
    this.fs = { next: 2 + captures + params, slots: new Map(), captures };
  }

  newSlot() {
    if (this.fs === null) throw new Error('frame slot requested outside an async function');
    return this.fs.next++;
  }

  // The slot of a binding or `this` cell, by its key ('b:<id>' or 'th<fn>').
  slotFor(key) {
    if (!this.fs.slots.has(key)) this.fs.slots.set(key, this.newSlot());
    return this.fs.slots.get(key);
  }

  slotOfTmp(name) {
    const s = this.frameOf.get(name);
    if (s === undefined) throw new Error(`not a frame temporary: ${name}`);
    return s;
  }

  // Defines a Value temporary (immutable in sync mode).
  def(name, expr) {
    if (this.async) this.line(`frame_set(rt, fr, ${this.slotOfTmp(name)}, ${expr});`);
    else this.line(`let ${name}: Value = ${expr};`);
  }

  // Defines a mutable Value temporary; set() assigns it afterwards.
  defMut(name, expr) {
    if (this.async) this.line(`frame_set(rt, fr, ${this.slotOfTmp(name)}, ${expr});`);
    else this.line(`let mut ${name}: Value = ${expr};`);
  }

  set(name, expr) {
    if (this.async) this.line(`frame_set(rt, fr, ${this.slotOfTmp(name)}, ${expr});`);
    else this.line(`${name} = ${expr};`);
  }

  // An ObjRef temporary (object literal, rest object). In async code it is held as an object Value in its slot.
  objTmp() {
    if (!this.async) return `o${this.tempN++}`;
    const s = this.newSlot();
    const name = `vn_obj(&frame_get(rt, fr, ${s}))`;
    this.frameOf.set(name, s);
    return name;
  }

  defObj(name, expr) {
    if (this.async) this.line(`frame_set(rt, fr, ${this.slotOfTmp(name)}, Value::Object(${expr}));`);
    else this.line(`let ${name}: ObjRef = ${expr};`);
  }

  // A PropKey temporary. In async code the key is held as its key Value (vn_key_value) and read back with vn_key.
  keyTmp() {
    if (!this.async) return `t${this.tempN++}`;
    const s = this.newSlot();
    const name = `vn_key(rt, &frame_get(rt, fr, ${s}))?`;
    this.frameOf.set(name, s);
    return name;
  }

  defKey(name, expr) {
    if (this.async) this.line(`frame_set(rt, fr, ${this.slotOfTmp(name)}, vn_key_value(&${expr}));`);
    else this.line(`let ${name}: PropKey = ${expr};`);
  }

  // Runs fn with a fresh line buffer and returns { value, lines }. The lines keep the current depth.
  capture(fn) {
    const saved = this.lines;
    this.lines = [];
    let value;
    try {
      value = fn();
    } finally {
      var out = this.lines;
      this.lines = saved;
    }
    return { value, lines: out };
  }

  splice(lines) {
    for (const l of lines) this.lines.push(l);
  }

  // Module-wide constant for a string literal (UTF-16 code units). Deterministic: numbered in first-use order.
  strConst(units) {
    const key = units.join(',');
    if (!this.strIndex.has(key)) {
      const name = `S${this.strings.length}`;
      this.strIndex.set(key, name);
      this.strings.push(`const ${name}: &[u16] = &[${units.join(', ')}];`);
    }
    return this.strIndex.get(key);
  }

  rstr(s) {
    return rstr(s);
  }
}

// Emitter state (WP-C1b): the line buffer of the Rust function being written, temporaries, the module string pool and
// the control-flow stack that decides how a jump is written (a Rust break/continue, or a Flow value returned out of a
// closure that implements try/finally or a for-of body).
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
    return `t${this.tempN++}`;
  }

  uniq(prefix) {
    return `${prefix}${this.tempN++}`;
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

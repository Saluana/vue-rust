// Diagnostics and source positions.
// Pos = { offset, line, column }: offset is a UTF-16 code unit index into the source string; line is 1-based;
// column is 0-based in UTF-16 code units from the start of the line. Spans are [start, end) with end exclusive.
// Line terminators follow ECMAScript: LF, CR, CRLF (one terminator), U+2028 and U+2029. A BOM (U+FEFF) is an
// ordinary code unit at offset 0 and counts as a column, matching @babel/parser's loc.
import { codeInfo } from './codes.mjs';

const SEVERITIES = new Set(['error', 'warning']);

function checkPos(pos, what) {
  if (
    pos === null || typeof pos !== 'object' ||
    !Number.isInteger(pos.offset) || !Number.isInteger(pos.line) || !Number.isInteger(pos.column) ||
    pos.offset < 0 || pos.line < 1 || pos.column < 0
  ) {
    throw new TypeError(`Diagnostic ${what} must be a Pos { offset, line, column }`);
  }
}

export class Diagnostic {
  constructor({ code, severity = 'error', message, hint = '', file = '<input>', start, end }) {
    codeInfo(code); // throws for unknown codes: a programmer error
    if (!SEVERITIES.has(severity)) throw new TypeError(`Diagnostic severity must be error or warning, got ${severity}`);
    if (typeof message !== 'string' || message === '') throw new TypeError('Diagnostic needs a non-empty message');
    checkPos(start, 'start');
    checkPos(end, 'end');
    if (end.offset < start.offset) throw new RangeError('Diagnostic end precedes start');
    this.code = code;
    this.severity = severity;
    this.message = message;
    this.hint = hint;
    this.file = file;
    this.start = start;
    this.end = end;
  }
}

// Builds a Diagnostic for a registered code. The hint defaults to the registry hint.
export function createDiagnostic(code, { message, hint, severity = 'error', file = '<input>', start, end }) {
  const info = codeInfo(code);
  return new Diagnostic({
    code,
    severity,
    message: message ?? info.title,
    hint: hint ?? info.hint,
    file,
    start,
    end,
  });
}

// createLineMap(source) -> { posAt(offset), offsetAt(line, column), lineCount }
export function createLineMap(source) {
  if (typeof source !== 'string') throw new TypeError('createLineMap: source must be a string');
  const starts = [0];
  const contentEnds = [];
  for (let i = 0; i < source.length; i++) {
    const c = source.charCodeAt(i);
    if (c === 0x0d || c === 0x0a || c === 0x2028 || c === 0x2029) {
      contentEnds.push(i);
      if (c === 0x0d && source.charCodeAt(i + 1) === 0x0a) i++; // CRLF is one terminator
      starts.push(i + 1);
    }
  }
  contentEnds.push(source.length);

  function posAt(offset) {
    if (!Number.isInteger(offset) || offset < 0 || offset > source.length) {
      throw new RangeError(`posAt: offset ${offset} outside 0..${source.length}`);
    }
    let lo = 0;
    let hi = starts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (starts[mid] <= offset) lo = mid;
      else hi = mid - 1;
    }
    // The LF of a CRLF pair has no column of its own: it maps to the end of the line's content.
    const column = Math.min(offset - starts[lo], contentEnds[lo] - starts[lo]);
    return { offset, line: lo + 1, column };
  }

  function offsetAt(line, column) {
    if (!Number.isInteger(line) || line < 1 || line > starts.length) {
      throw new RangeError(`offsetAt: line ${line} outside 1..${starts.length}`);
    }
    const lineIndex = line - 1;
    const maxColumn = contentEnds[lineIndex] - starts[lineIndex];
    if (!Number.isInteger(column) || column < 0 || column > maxColumn) {
      throw new RangeError(`offsetAt: column ${column} outside 0..${maxColumn} on line ${line}`);
    }
    return starts[lineIndex] + column;
  }

  return Object.freeze({ posAt, offsetAt, lineCount: starts.length });
}

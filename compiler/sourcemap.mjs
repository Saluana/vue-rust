// Locating generated code in the author's source (WP 3.7). A module's emitted Rust records, for each function, throw and
// await, the place in the author's file (compiler/hir: siteOf). A locator maps an offset of the module's compiled text to
//   { file, line, column }  (line 1-based, column 0-based UTF-16 units, as diagnostics report them), or null.
// Plain JavaScript and TypeScript modules are their own source: identityLocator. A .vue module's compiled text is
// checked against the source map that the official compileScript builds (v3 `mappings`, VLQ, decoded here).
// A position is located only when the map has a segment at exactly that generated column; nothing is approximated.
import { createLineMap } from './diagnostics/diagnostic.mjs';

const DIGITS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
const DIGIT_OF = new Map([...DIGITS].map((c, i) => [c, i]));

// Decodes v3 `mappings` into lines of segments. A segment is [generatedColumn] or, with a source, [generatedColumn,
// sourceIndex, sourceLine, sourceColumn] (0-based). The source fields are relative to the previous segment, as the format
// defines; names are read and dropped.
export function decodeMappings(mappings) {
  const lines = [[]];
  const state = { source: 0, sourceLine: 0, sourceColumn: 0 };
  let i = 0;
  const vlq = () => {
    let result = 0;
    let shift = 0;
    for (;;) {
      const digit = DIGIT_OF.get(mappings[i++]);
      if (digit === undefined) throw new Error(`decodeMappings: bad character at ${i - 1}`);
      result += (digit & 31) * 2 ** shift;
      shift += 5;
      if ((digit & 32) === 0) break;
    }
    return result % 2 === 1 ? -(result - 1) / 2 : result / 2;
  };
  const atEnd = () => i >= mappings.length || mappings[i] === ',' || mappings[i] === ';';
  let genColumn = 0;
  while (i < mappings.length) {
    const c = mappings[i];
    if (c === ';') {
      lines.push([]);
      genColumn = 0;
      i++;
      continue;
    }
    if (c === ',') {
      i++;
      continue;
    }
    genColumn += vlq();
    const segment = [genColumn];
    if (!atEnd()) {
      state.source += vlq();
      state.sourceLine += vlq();
      state.sourceColumn += vlq();
      segment.push(state.source, state.sourceLine, state.sourceColumn);
      if (!atEnd()) vlq(); // a name index
    }
    lines[lines.length - 1].push(segment);
  }
  return lines;
}

// A locator for code that is its own source: offsets are positions in `source`, reported under `file`.
export function identityLocator(source, file) {
  const lines = createLineMap(source);
  return (offset) => {
    const p = lines.posAt(offset);
    return { file, line: p.line, column: p.column };
  };
}

// A locator for generated code with a v3 source map. `code` is the whole compiled text; the map's first line is the line
// that holds offset mapOffset, which must start a line (its column is 0). Every position of the map has one source, `file`.
export function sourceMapLocator({ code, map, file, mapOffset }) {
  if (!Number.isInteger(mapOffset) || mapOffset < 0 || mapOffset > code.length) {
    throw new TypeError('sourceMapLocator: mapOffset must be an offset of code');
  }
  const lines = createLineMap(code);
  const first = lines.posAt(mapOffset);
  if (first.column !== 0) throw new TypeError('sourceMapLocator: mapOffset must start a line');
  const decoded = decodeMappings(map.mappings);
  if (!Array.isArray(map.sources) || map.sources.length !== 1) {
    return () => null;
  }
  return (offset) => {
    if (offset < mapOffset) return null;
    const p = lines.posAt(offset);
    const segments = decoded[p.line - first.line];
    const hit = segments?.find((s) => s[0] === p.column);
    if (hit === undefined || hit.length < 4 || hit[1] !== 0) return null;
    return { file, line: hit[2] + 1, column: hit[3] };
  };
}

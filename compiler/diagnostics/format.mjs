// Human-readable rendering of diagnostics. Columns are 0-based UTF-16 units, the same numbers as Pos and as the
// "@expect" headers of tests/language/rejects fixtures. Tabs in the source line are copied into the caret line so
// the underline stays aligned; East Asian wide characters and emoji take two cells, combining marks none.
const ANSI = { error: '\u001b[31m', warning: '\u001b[33m', reset: '\u001b[0m', bold: '\u001b[1m' };

function cellWidth(cp) {
  if ((cp >= 0x0300 && cp <= 0x036f) || (cp >= 0x200b && cp <= 0x200f) ||
      (cp >= 0xfe00 && cp <= 0xfe0f) || (cp >= 0x20d0 && cp <= 0x20ff)) return 0;
  if ((cp >= 0x1100 && cp <= 0x115f) || (cp >= 0x2e80 && cp <= 0x303e) || (cp >= 0x3041 && cp <= 0x33ff) ||
      (cp >= 0x3400 && cp <= 0x4dbf) || (cp >= 0x4e00 && cp <= 0x9fff) || (cp >= 0xa000 && cp <= 0xa4cf) ||
      (cp >= 0xac00 && cp <= 0xd7a3) || (cp >= 0xf900 && cp <= 0xfaff) || (cp >= 0xfe30 && cp <= 0xfe4f) ||
      (cp >= 0xff00 && cp <= 0xff60) || (cp >= 0xffe0 && cp <= 0xffe6) ||
      (cp >= 0x1f300 && cp <= 0x1f64f) || (cp >= 0x1f900 && cp <= 0x1f9ff) ||
      (cp >= 0x20000 && cp <= 0x3fffd)) return 2;
  return 1;
}

// Terminal cells used by text, counted per code point.
function cellsOf(text) {
  let cells = 0;
  for (const ch of text) cells += cellWidth(ch.codePointAt(0));
  return cells;
}

// Caret line: tabs are kept, every other code point before the span becomes a space of the same width.
function caretPrefix(text) {
  let out = '';
  for (const ch of text) {
    out += ch === '\t' ? '\t' : ' '.repeat(cellWidth(ch.codePointAt(0)));
  }
  return out;
}

export function formatDiagnostic(diag, source, { color = false } = {}) {
  if (typeof source !== 'string') throw new TypeError('formatDiagnostic: source must be a string');
  const { start, end } = diag;
  const lineNo = start.line;
  const lineStart = start.offset - start.column;
  const rest = source.slice(lineStart);
  const terminator = /[\r\n\u2028\u2029]/.exec(rest);
  const lineText = terminator ? rest.slice(0, terminator.index) : rest;

  const spanEndCol = end.line === start.line ? Math.min(end.column, lineText.length) : lineText.length;
  const before = lineText.slice(0, start.column);
  const underlined = lineText.slice(start.column, Math.max(start.column, spanEndCol));
  const carets = Math.max(1, cellsOf(underlined));

  const paint = (kind, text) => (color ? `${ANSI[kind]}${text}${ANSI.reset}` : text);
  const sev = diag.severity === 'warning' ? 'warning' : 'error';
  const gutter = String(lineNo);
  const pad = ' '.repeat(gutter.length + 1);

  const out = [];
  out.push(
    `${diag.file}:${start.line}:${start.column}: ${paint(sev, sev)} ${paint('bold', diag.code)}: ${diag.message}`,
  );
  out.push(`${pad}|`);
  out.push(`${gutter} | ${lineText}`);
  out.push(`${pad}| ${caretPrefix(before)}${paint(sev, '^'.repeat(carets))}`);
  if (diag.hint) out.push(`${pad}= hint: ${diag.hint}`);
  return out.join('\n');
}

export function formatDiagnostics(diags, source, options = {}) {
  return diags.map((d) => formatDiagnostic(d, source, options)).join('\n\n');
}

// Test262 metadata (WP 3.1): the frontmatter of a test file, read without a YAML dependency. test262 writes a small
// YAML subset between "/*---" and "---*/": `key: scalar`, flow lists `[a, b]`, block lists (`- item`), nested
// mappings (`negative:` with `phase` and `type`) and block scalars (`|`, `>`), whose text this reader does not need.
// Anything else is a MetaError, so a file whose metadata cannot be read is never selected or run silently.

export class MetaError extends Error {
  constructor(message) {
    super(message);
    this.name = 'MetaError';
  }
}

const FRONTMATTER = /\/\*---([\s\S]*?)---\*\//;
const KEY_LINE = /^([A-Za-z_][\w-]*):(?:[ \t]+(.*))?$/;
const BLOCK_SCALAR = /^[|>][-+]?$/;

function unquote(text) {
  const t = text.trim();
  if (t.length >= 2 && (t[0] === '"' || t[0] === "'") && t[t.length - 1] === t[0]) return t.slice(1, -1);
  return t;
}

// "[a, 'b', c]" -> ['a', 'b', 'c'].
function flowList(text, no) {
  const t = text.trim();
  if (!t.startsWith('[') || !t.endsWith(']')) throw new MetaError(`line ${no}: unsupported flow list ${t}`);
  const inner = t.slice(1, -1).trim();
  if (inner === '') return [];
  return inner.split(',').map(unquote).filter((item) => item !== '');
}

const isListItem = (text) => text === '-' || text.startsWith('- ');

// Parses the mapping whose keys sit at `indent`, starting at lines[start]. Returns { obj, next }.
// Each line record is { indent, text, no }: indentation, trimmed text and 1-based line number.
function parseMapping(lines, start, indent) {
  const obj = {};
  let i = start;
  while (i < lines.length) {
    const { indent: ind, text, no } = lines[i];
    if (ind < indent) break;
    if (ind > indent) throw new MetaError(`line ${no}: unexpected indentation`);
    const m = KEY_LINE.exec(text);
    if (!m) throw new MetaError(`line ${no}: expected "key: value", got "${text}"`);
    const [, key, rest = ''] = m;
    if (Object.hasOwn(obj, key)) throw new MetaError(`line ${no}: duplicate key ${key}`);
    i += 1;
    if (BLOCK_SCALAR.test(rest)) {
      // The body is every following line indented past the key. Its text is not needed, so it is skipped.
      while (i < lines.length && lines[i].indent > indent) i += 1;
      obj[key] = '';
    } else if (rest === '') {
      const child = lines[i];
      if (child !== undefined && child.indent > indent && isListItem(child.text)) {
        const items = [];
        while (i < lines.length && lines[i].indent === child.indent && isListItem(lines[i].text)) {
          items.push(unquote(lines[i].text.slice(1)));
          i += 1;
        }
        obj[key] = items;
      } else if (child !== undefined && child.indent > indent) {
        const nested = parseMapping(lines, i, child.indent);
        obj[key] = nested.obj;
        i = nested.next;
      } else {
        obj[key] = null;
      }
    } else if (rest.startsWith('[')) {
      // A flow list may continue on the following lines until its closing bracket.
      let flow = rest;
      while (!flow.trimEnd().endsWith(']') && i < lines.length) {
        flow += ` ${lines[i].text}`;
        i += 1;
      }
      obj[key] = flowList(flow, no);
    } else {
      obj[key] = unquote(rest);
    }
  }
  return { obj, next: i };
}

// The frontmatter of a test file as a plain object. Throws MetaError when there is none or it is not readable.
export function parseFrontmatter(source) {
  const m = FRONTMATTER.exec(source);
  if (!m) throw new MetaError('no /*--- ... ---*/ frontmatter');
  const lines = [];
  m[1].split('\n').forEach((raw, k) => {
    const line = raw.replace(/\r$/, '');
    if (line.trim() === '') return;
    if (line.includes('\t')) throw new MetaError(`line ${k + 1}: tab in frontmatter`);
    lines.push({ indent: line.length - line.trimStart().length, text: line.trim(), no: k + 1 });
  });
  return parseMapping(lines, 0, 0).obj;
}

// The metadata the selection and the runner use: includes, flags, features (lists) and negative ({phase, type} or null).
export function parseMeta(source) {
  const fm = parseFrontmatter(source);
  const list = (key) => {
    const v = fm[key];
    if (v === undefined || v === null) return [];
    if (!Array.isArray(v)) throw new MetaError(`${key} must be a list`);
    return v;
  };
  let negative = null;
  if (fm.negative !== undefined) {
    const n = fm.negative;
    if (n === null || typeof n !== 'object' || typeof n.phase !== 'string' || typeof n.type !== 'string') {
      throw new MetaError('negative must name a phase and a type');
    }
    negative = { phase: n.phase, type: n.type };
  }
  return {
    description: typeof fm.description === 'string' ? fm.description : '',
    includes: list('includes'),
    flags: list('flags'),
    features: list('features'),
    negative,
  };
}

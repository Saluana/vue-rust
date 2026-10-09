// WP-L2b oracle corpus for crates/value (operators, equality, unary operators and conversions).
// Every expected value is produced by V8 running in Node, never by the Rust code. Deterministic: a fixed
// operand table, every ordered pair, no clock or environment input, so a rerun rewrites a byte-identical
// file (check with `shasum -a 256`). Needs Node >= 22, no dependencies.
//   node tests/language/gen/gen-ops-corpus.mjs
//
// Encoding (same as corpora/number.json):
//   JS number     -> 16 lowercase hex digits of its IEEE-754 bits ("3ff0000000000000" is 1)
//   JS string     -> array of UTF-16 code units (lone surrogates allowed)
//   JS boolean    -> true / false
//   JS null       -> null
//   JS undefined  -> {"$":"undef"}
//   thrown error  -> {"throw":"<constructor name>"}
// Symbols and objects cannot be written in JSON; they are covered by hand-written Rust tests.

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

if (Number(process.versions.node.split('.')[0]) < 22) {
  throw new Error(`node >= 22 required, got ${process.version}`);
}
const OUT = join(dirname(fileURLToPath(import.meta.url)), '..', 'corpora');
mkdirSync(OUT, { recursive: true });

// ---------------------------------------------------------------- encoding
const dv = new DataView(new ArrayBuffer(8));
const bitsOf = (x) => {
  dv.setFloat64(0, x);
  return dv.getBigUint64(0).toString(16).padStart(16, '0');
};
const encText = (v) => {
  if (v === undefined) return '{"$":"undef"}';
  if (v === null) return 'null';
  if (typeof v === 'boolean') return String(v);
  if (typeof v === 'number') return JSON.stringify(bitsOf(v));
  if (typeof v === 'string') return '[' + Array.from({ length: v.length }, (_, i) => v.charCodeAt(i)).join(',') + ']';
  throw new Error(`unencodable value: ${typeof v}`);
};
// Runs one operation in V8. Only ordinary exceptions are expected (and recorded); anything else is a bug.
const run = (fn) => {
  try {
    return encText(fn());
  } catch (e) {
    if (e instanceof Error) return `{"throw":${JSON.stringify(e.constructor.name)}}`;
    throw e;
  }
};

// ---------------------------------------------------------------- operands (48, in a fixed order)
const OPERANDS = [
  ['undefined', undefined], ['null', null], ['true', true], ['false', false],
  ['0', 0], ['-0', -0], ['1', 1], ['-1', -1], ['2', 2], ['3', 3],
  ['0.5', 0.5], ['-0.5', -0.5], ['1.5', 1.5], ['1e21', 1e21],
  ['NaN', NaN], ['Infinity', Infinity], ['-Infinity', -Infinity],
  ['2^31', 2 ** 31], ['-2^31', -(2 ** 31)], ['2^32', 2 ** 32], ['2^53', 2 ** 53],
  ['MAX_VALUE', Number.MAX_VALUE], ['MIN_VALUE', Number.MIN_VALUE],
  ['""', ''], ['"0"', '0'], ['"1"', '1'], ['" 1 "', ' 1 '], ['"-1"', '-1'], ['"a"', 'a'],
  ['"1e3"', '1e3'], ['"0x10"', '0x10'], ['"0b11"', '0b11'], ['"0o7"', '0o7'],
  ['"Infinity"', 'Infinity'], ['"-Infinity"', '-Infinity'], ['"NaN"', 'NaN'],
  ['"\\n"', '\n'], ['"1 2"', '1 2'], ['".5"', '.5'], ['"5."', '5.'],
  ['"lone surrogate"', '\uD800'], ['"emoji"', '\u{1F600}'],
  ['"true"', 'true'], ['"null"', 'null'], ['"undefined"', 'undefined'],
  ['"-0"', '-0'], ['"0.1"', '0.1'], ['" "', ' '],
];

// ---------------------------------------------------------------- operations (V8 is the oracle)
const BINARY = [
  ['+', (a, b) => a + b], ['-', (a, b) => a - b], ['*', (a, b) => a * b], ['/', (a, b) => a / b],
  ['%', (a, b) => a % b], ['**', (a, b) => a ** b],
  ['&', (a, b) => a & b], ['|', (a, b) => a | b], ['^', (a, b) => a ^ b],
  ['<<', (a, b) => a << b], ['>>', (a, b) => a >> b], ['>>>', (a, b) => a >>> b],
  ['<', (a, b) => a < b], ['>', (a, b) => a > b], ['<=', (a, b) => a <= b], ['>=', (a, b) => a >= b],
  ['==', (a, b) => a == b], ['!=', (a, b) => a != b], ['===', (a, b) => a === b], ['!==', (a, b) => a !== b],
];
const UNARY = [
  ['-', (a) => -a], ['+', (a) => +a], ['~', (a) => ~a], ['!', (a) => !a], ['typeof', (a) => typeof a],
];
const CONVERSIONS = [
  ['ToString', (a) => String(a)], // String(x) is ToString for every non-symbol value
  ['ToNumber', (a) => Number(a)],
  ['ToInt32', (a) => a | 0], // | applies ToNumeric then ToInt32
];

// ---------------------------------------------------------------- output (one row per line)
const n = OPERANDS.length;
const lines = ['{', '"operands": ['];
OPERANDS.forEach(([name, v], i) => {
  lines.push(`${i ? ',' : ''}{"name":${JSON.stringify(name)},"v":${encText(v)}}`);
});
lines.push('],');

const table = (key, rows, last) => {
  lines.push(`${JSON.stringify(key)}: [`);
  rows.forEach((row, k) => lines.push(`${k ? ',' : ''}${row}`));
  lines.push(last ? ']' : '],');
};

lines.push('"binary": {');
BINARY.forEach(([op, f], k) => {
  const rows = [];
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < n; j++) {
      const a = OPERANDS[i][1];
      const b = OPERANDS[j][1];
      rows.push(`[${i},${j},${run(() => f(a, b))}]`);
    }
  }
  table(op, rows, k === BINARY.length - 1);
});
lines.push('},');

lines.push('"unary": {');
UNARY.forEach(([op, f], k) => {
  const rows = OPERANDS.map(([, a], i) => `[${i},${run(() => f(a))}]`);
  table(op, rows, k === UNARY.length - 1);
});
lines.push('},');

lines.push('"conversions": {');
CONVERSIONS.forEach(([name, f], k) => {
  const rows = OPERANDS.map(([, a], i) => `[${i},${run(() => f(a))}]`);
  table(name, rows, k === CONVERSIONS.length - 1);
});
lines.push('}');
lines.push('}');

writeFileSync(join(OUT, 'ops.json'), lines.join('\n') + '\n');
console.log(`wrote ops.json: ${n} operands, ${n * n * BINARY.length} binary, ${n * (UNARY.length + CONVERSIONS.length)} unary/conversion cases`);

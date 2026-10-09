// WP-L1 oracle corpora for crates/value (vn-value).
// Every expected value is produced by V8 running in Node, never by the Rust code.
// Deterministic: fixed-seed xorshift64* PRNG, no clock or environment input, so a rerun rewrites
// byte-identical files (checked with `shasum -a 256 -c`). Needs Node >= 22, no dependencies.
//   node tests/language/gen/gen-corpora.mjs
//
// Encoding (both files):
//   JS number argument or result  -> 16 lowercase hex digits of its IEEE-754 bits ("3ff0000000000000" is 1)
//   JS string                     -> array of UTF-16 code units (lone surrogates allowed)
//   JS undefined argument         -> null
//   "absent" result               -> null: NaN from charCodeAt, -1 from indexOf/lastIndexOf, undefined from
//                                    codePointAt, a RangeError from String.fromCodePoint (see each table)
//   thrown RangeError             -> {"throw":"RangeError"}
//   integer results               -> plain JSON numbers

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

if (Number(process.versions.node.split('.')[0]) < 22) {
  throw new Error(`node >= 22 required, got ${process.version}`);
}
const OUT = join(dirname(fileURLToPath(import.meta.url)), '..', 'corpora');
mkdirSync(OUT, { recursive: true });

// ---------------------------------------------------------------- PRNG (xorshift64*)
const M64 = (1n << 64n) - 1n;
let rs = 0x9e3779b97f4a7c15n;
function rnd64() {
  rs ^= rs >> 12n;
  rs ^= (rs << 25n) & M64;
  rs ^= rs >> 27n;
  return (rs * 0x2545f4914f6cdd1dn) & M64;
}
const rint = (n) => Number(rnd64() % BigInt(n)); // 0 <= r < n
const pick = (a) => a[rint(a.length)];
const sel = (a, k) => Array.from({ length: k }, () => pick(a));

// ---------------------------------------------------------------- bit helpers
const dv = new DataView(new ArrayBuffer(8));
const hex = (b) => b.toString(16).padStart(16, '0');
const bitsOf = (x) => {
  dv.setFloat64(0, x);
  return hex(dv.getBigUint64(0));
};
const ofBits = (h) => {
  dv.setBigUint64(0, BigInt('0x' + h));
  return dv.getFloat64(0);
};
const nextUp = (x) => {
  dv.setFloat64(0, x);
  dv.setBigUint64(0, dv.getBigUint64(0) + 1n);
  return dv.getFloat64(0);
};
const nextDown = (x) => {
  dv.setFloat64(0, x);
  dv.setBigUint64(0, dv.getBigUint64(0) - 1n);
  return dv.getFloat64(0);
};
const units = (s) => Array.from({ length: s.length }, (_, i) => s.charCodeAt(i));
const threw = (fn) => {
  try {
    return fn();
  } catch (e) {
    if (e instanceof RangeError) return { throw: 'RangeError' };
    throw e;
  }
};
const nullIfNaN = (v) => (Number.isNaN(v) ? null : v);
const nullIfMinus1 = (v) => (v === -1 ? null : v);
const uniqBy = (arr, key) => {
  const seen = new Set();
  const out = [];
  for (const x of arr) {
    const k = key(x);
    if (!seen.has(k)) {
      seen.add(k);
      out.push(x);
    }
  }
  return out;
};
const uniqNum = (arr) => uniqBy(arr, bitsOf);

// ---------------------------------------------------------------- number samples
const SPECIAL = [
  0, -0, Infinity, -Infinity, NaN, 5e-324, -5e-324, Number.MAX_VALUE, -Number.MAX_VALUE,
  Number.EPSILON, -Number.EPSILON, 2 ** -1022, 2 ** 53, 2 ** 53 - 1, 2 ** 53 + 2, 2 ** 31 - 1,
  2 ** 31, 2 ** 32 - 1, 2 ** 32, 2 ** 63, 2 ** 64, 2 ** 70, 1e21, 1e15, 1e16, 1e-7, 1e-6, 0.1,
  0.2, 0.3, 1 / 3, 2 / 3, 0.5, 1.5, 2.5, 3.5, -2.5, 0.125, 1.005, 8.345, 1.45, 1.55, 2.675,
  4.35, 10.235, 123.456, 1e300, 1e-300, 1 + 2 ** -52, 1 - 2 ** -53, 1e23, 0.000001234,
  123456789012345680000, -1e-7, 42, -42, 255, 256, 1000, 65535, 4294967296, 1.7976931348623157e308,
  9007199254740993, 0.49999999999999994, 4.5, -4.5, 0.1 + 0.2, 1e-323, 2 ** -1074,
  ofBits('7ff0000000000001'), ofBits('fff8000000000000'),
];
const NEIGHBOURS = [];
for (const x of [1e21, 1e-7, 1, 10, 100, 2 ** 53, 0.1, 1e308, 2.5, 1e-300]) {
  NEIGHBOURS.push(nextUp(x), nextDown(x));
}
const POW10 = [];
for (let k = -323; k <= 308; k++) POW10.push(Number('1e' + k));
const POW2 = [];
for (let k = -1074; k <= 1023; k += 13) POW2.push(2 ** k);
const digitsOf = (n) => Array.from({ length: n }, () => String(rint(10))).join('');
function humanDecimal() {
  if (rint(10) === 0) return '0.' + '0'.repeat(rint(12)) + digitsOf(1 + rint(9));
  let s = String(1 + rint(9)) + digitsOf(rint(7));
  if (rint(4) !== 0) s += '.' + digitsOf(1 + rint(12));
  if (rint(5) === 0) s += 'e' + (rint(80) - 40);
  return s;
}
const HUMAN = Array.from({ length: 2000 }, () => Number(humanDecimal()));
const RANDOM = Array.from({ length: 3000 }, () => ofBits(hex(rnd64())));
const DYADIC = Array.from({ length: 150 }, () => rint(1 << 20) / 2 ** rint(14));
const INTS_SMALL = Array.from({ length: 300 }, () => rint(1 << 20));
const INTS_BIG = RANDOM.slice(0, 400).filter((x) => Number.isFinite(x)).map((x) => Math.trunc(x));
const INTS_SPECIAL = [2 ** 53, 2 ** 53 + 2, 2 ** 64, 2 ** 100, 1e21, 2 ** 1000, 1.7976931348623157e308];
const ALL = uniqNum([...SPECIAL, ...NEIGHBOURS, ...POW10, ...POW2, ...HUMAN, ...RANDOM, ...DYADIC]);
const FIXED_IN = uniqNum([...SPECIAL, ...HUMAN.slice(0, 100), ...RANDOM.slice(0, 30)]);
const PREC_IN = uniqNum([...SPECIAL.slice(0, 40), ...HUMAN.slice(100, 120), ...RANDOM.slice(30, 36)]);
const FRAC = uniqNum(
  [...DYADIC, ...HUMAN.slice(0, 200)].filter(
    (x) => Number.isFinite(x) && Math.abs(x) >= 1e-6 && Math.abs(x) <= 1e6 && x !== Math.trunc(x),
  ),
);
const SPEC_SMALL = [
  0, -0, 1, -1, 2, -2, 0.5, -0.5, 3, 7, 10, Infinity, -Infinity, NaN, 5e-324, Number.MAX_VALUE,
  1e-300, 2.5, -2.5, 1.5, 4, 0.1, 1e21, 2 ** 53 + 2, 7.5,
];
const RADIX_ARGS = [undefined, 0, 1, 2, 8, 10, 16, 32, 36, 37, -1, 4294967312, 4294967306, 2 ** 32, 2.9];

// ---------------------------------------------------------------- number tables
const NUM = {};
NUM.toString = ALL.map((x) => [bitsOf(x), x.toString()]);

const radixRows = [];
const addRadix = (x, radixes) => {
  for (const r of radixes) radixRows.push([bitsOf(x), bitsOf(r), threw(() => x.toString(r))]);
};
for (const x of INTS_BIG) addRadix(x, [8, 16, 36]);
for (const x of INTS_SMALL) addRadix(x, [2, 3, 7, 16, 36]);
for (const x of INTS_SPECIAL) addRadix(x, [2, 36]);
for (const x of FRAC) addRadix(x, [2, 3, 16, 36]);
for (const r of [0, 1, 37, NaN, Infinity, -Infinity, 2.9, 36.9, 4294967312]) addRadix(10, [r]);
for (const x of [-255, -0.5, NaN, Infinity]) addRadix(x, [2, 16, 36]);
NUM.toStringRadix = radixRows;

const fixedRows = [];
for (const x of FIXED_IN) {
  for (let f = 0; f <= 20; f++) fixedRows.push([bitsOf(x), bitsOf(f), threw(() => x.toFixed(f))]);
}
for (const x of [1e-7, 5e-324, 0.1, 1.5, 1e-20, 123.456, 1e21, -9.999]) {
  for (const f of [21, 22, 30, 50, 100]) fixedRows.push([bitsOf(x), bitsOf(f), threw(() => x.toFixed(f))]);
}
for (const x of [1.5, -1.5, 0, NaN, Infinity, -1e-7, 2.345]) {
  for (const f of [101, -1, Infinity, -Infinity, NaN, 2.7, -0.5, 1e10, 100]) {
    fixedRows.push([bitsOf(x), bitsOf(f), threw(() => x.toFixed(f))]);
  }
}
NUM.toFixed = fixedRows;

const expRows = [];
for (const x of FIXED_IN) expRows.push([bitsOf(x), null, threw(() => x.toExponential())]);
for (const x of FIXED_IN.slice(0, 150)) {
  for (const f of [0, 1, 2, 5, 10, 15, 20, 100]) expRows.push([bitsOf(x), bitsOf(f), threw(() => x.toExponential(f))]);
}
for (const x of [1.5, 0, NaN, Infinity, -1e-7]) {
  for (const f of [-1, 101, Infinity, NaN, 2.5]) expRows.push([bitsOf(x), bitsOf(f), threw(() => x.toExponential(f))]);
}
NUM.toExponential = expRows;

const precRows = [];
for (const x of PREC_IN) precRows.push([bitsOf(x), null, threw(() => x.toPrecision())]);
for (const x of PREC_IN) {
  for (let p = 1; p <= 21; p++) precRows.push([bitsOf(x), bitsOf(p), threw(() => x.toPrecision(p))]);
}
for (const x of [1.5, 0, -1e-7, NaN, Infinity]) {
  for (const p of [0, 101, -1, Infinity, NaN]) precRows.push([bitsOf(x), bitsOf(p), threw(() => x.toPrecision(p))]);
}
NUM.toPrecision = precRows;

// Strings for ToNumber / parseFloat / parseInt. Each is an array of code units.
const HAND_NUM = [
  '', ' ', '  12  ', '12', '-12', '+12', '1e3', '1E3', '1e+3', '1e-3', '.5', '5.', '.', '5.e2', '.e2',
  'e5', '1e', '1e+', '0x', '0x1F', '0X1f', '0x1g', '-0x1F', '+0x1F', '0o17', '0O17', '0o18', '0b101',
  '0B2', '0b', '0', '-0', '+0', '00012', '08', '1_0', 'Infinity', '-Infinity', '+Infinity', 'infinity',
  'INFINITY', 'NaN', '-', '+', '1.2.3', '1e400', '-1e400', '1e-400', '4.9e-324', '2.4703282292062328e-324',
  '0.1', '9007199254740993', '123456789012345678901234567890', '0x' + 'f'.repeat(20), '0x' + 'f'.repeat(300),
  '0b' + '1'.repeat(70), '0o' + '7'.repeat(40), ' 12 ', '  1 ', '﻿42﻿',
  '᠎1', '​1', '\t\n\v\f\r 7 \t\n', '1 ', '3.14abc', '1e+x', '.5.5', '-.e1', 'Infinityx',
  '  -Infinity  ', '0x10', '1_000', '+.1e-1', '- 1', '0x1.8', '1e1000000000000000000000', '1e-99999999999999999999',
  '　　 42　', '             　 9 ',
  '٠١', '1٠', '²', '0x١', '﻿Infinity', ' Infinity ', '+-1', '--1',
  '0b11111111111111111111111111111111111111111111111111111111', '0o777777777777777777777777777777777',
  '12e', '12e-', '12.e-3', '.1e+3', '00.5', '0.0000000000000000000000000000000000001',
];
const NUM_CHARS = ['0', '1', '7', '9', '.', 'e', 'E', '+', '-', 'x', 'o', 'b', 'a', 'f', 'I', 'n', 'i', 't', 'y', '_', ' ', '\t', '\n', ' ', ' ', '﻿', '　', '᠎', '​', '٠', 'F', 'Z'];
const RAND_NUM = Array.from({ length: 220 }, () => sel(NUM_CHARS, rint(9)).join(''));
const NUM_STRS = uniqBy([...HAND_NUM, ...RAND_NUM], (s) => s);
NUM.stringToNumber = NUM_STRS.map((s) => [units(s), bitsOf(Number(s))]);
NUM.parseFloat = NUM_STRS.map((s) => [units(s), bitsOf(parseFloat(s))]);
const parseIntRows = [];
for (const s of NUM_STRS) {
  for (let k = 0; k < 3; k++) {
    const r = pick(RADIX_ARGS);
    parseIntRows.push([units(s), r | 0, bitsOf(parseInt(s, r))]);
  }
}
// Long strings in generic radixes: V8 drops leading zeros and then folds digits in chunks, so rounding
// depends on the exact digit layout. These rows pin that behaviour.
const ALPH = '0123456789abcdefghijklmnopqrstuvwxyz';
const LONG_RADIX = [3, 5, 7, 11, 13, 17, 19, 23, 26, 29, 31, 33, 35, 36];
for (let i = 0; i < 400; i++) {
  const radix = pick(LONG_RADIX);
  const len = 14 + rint(60);
  const s = Array.from({ length: len }, (_, k) => (k < 3 && rint(3) === 0 ? '0' : ALPH[rint(radix)])).join('');
  parseIntRows.push([units(s), radix, bitsOf(parseInt(s, radix))]);
}
NUM.parseInt = parseIntRows;

NUM.toIntegers = uniqNum([...SPECIAL, ...RANDOM.slice(0, 200), ...HUMAN.slice(0, 100)]).map((x) => [
  bitsOf(x),
  [x | 0, x >>> 0, (x << 16) >> 16, x & 0xffff, (x << 24) >> 24, x & 0xff],
]);

const pairRows = (fn) => {
  const rows = [];
  for (const a of SPEC_SMALL) for (const b of SPEC_SMALL) rows.push([bitsOf(a), bitsOf(b), bitsOf(fn(a, b))]);
  for (let i = 0; i < 400; i++) {
    const a = pick(ALL);
    const b = pick(ALL);
    rows.push([bitsOf(a), bitsOf(b), bitsOf(fn(a, b))]);
  }
  return rows;
};
NUM.rem = pairRows((a, b) => a % b);
// Math.pow: pairs by category. Only exponents of 0.5 separated V8 from the platform pow in an early probe.
const powPairs = [];
for (let i = 0; i < 400; i++) powPairs.push([Math.abs(ofBits(hex(rnd64()))), ofBits(hex(rnd64()))]);
for (let i = 0; i < 300; i++) powPairs.push([rint(1 << 20) / 2 ** rint(10), (rint(2000) - 1000) / 8]);
for (let i = 0; i < 300; i++) powPairs.push([1 + rint(1000000) / 1000, rint(200) / 3 - 30]);
for (let i = 0; i < 400; i++) powPairs.push([rint(100000) / 1000, pick([0.5, -0.5, 1, -1, 2, 0.25, 0.75, 1.5])]);
NUM.pow = [
  ...pairRows((a, b) => Math.pow(a, b)),
  ...powPairs.map(([a, b]) => [bitsOf(a), bitsOf(b), bitsOf(Math.pow(a, b))]),
];
NUM.round = uniqNum([...SPECIAL, ...HUMAN.slice(0, 300), ...RANDOM.slice(0, 100), ...DYADIC]).map((x) => [
  bitsOf(x),
  bitsOf(Math.round(x)),
]);
const minMaxArgs = [[], [-0, 0], [0, -0], [NaN, 1], [1, NaN], [-Infinity, 2], [Infinity, -Infinity]];
for (let i = 0; i < 300; i++) minMaxArgs.push(Array.from({ length: rint(5) }, () => pick(SPEC_SMALL)));
NUM.min = minMaxArgs.map((args) => [args.map(bitsOf), bitsOf(Math.min(...args))]);
NUM.max = minMaxArgs.map((args) => [args.map(bitsOf), bitsOf(Math.max(...args))]);
const sameRows = [];
for (const a of SPEC_SMALL) for (const b of SPEC_SMALL) sameRows.push([a, b]);
for (let i = 0; i < 300; i++) sameRows.push([pick(ALL), pick(ALL)]);
NUM.sameValue = sameRows.map(([a, b]) => [
  bitsOf(a),
  bitsOf(b),
  Object.is(a, b),
  a === b || (a !== a && b !== b),
]);
NUM.isInteger = ALL.slice(0, 700).map((x) => [bitsOf(x), Number.isInteger(x)]);

// ---------------------------------------------------------------- string tables
const BASE = [
  '', 'a', 'A', 'x', ' ', 'abc', 'ABC', 'Hello, World!', ' padded  ', '\t\n\v\f\r x \t', '  ', '0', '1',
  '01', '-1', '+5', '4294967294', '4294967295', '4294967296', '1e3', '0x1F', '1_0', 'NaN', 'Infinity',
  '١٢', ' nbsp ',
  '             　 mix 　',
  ' line ', '﻿bom﻿', '᠎ mvs', '​ zwsp', '\u0085nel\u0085', 'İ', 'i̇',
  'ß', 'ẞ', 'ŉ', 'ǰ', 'ﬃ', 'ΑΣ', 'ΑΣΑ', 'ΑΣ ΑΣ',
  'Σ', 'σς', 'ΌΣΟΣ', 'ΑΣ.', 'ΑΣ́Σ', 'ა',
  'Ⴀ', 'ა ბ', 'Ʞ', 'ǅ', 'ǈ', 'ﬀ', '😀', 'a😀b',
  '\ud83d', '\ude00', '\ud83d😀', '\ude00\ud83d', 'x\ud800', '\udc00y', '😀\ud83d',
  'é', '́e', 'à́̂', 'ñ', '🇺🇸', 'ab|cd|ef',
  'a,b,,c', ',', ',,', 'aaaa', 'aaa', 'abab', 'banana', 'ababa', '$&', "$'", '$`', '$$', '$1', '$<a>', '$0',
  '$', 'a$b$c', 'x$$y', '\u0000null\u0000', '�', '￿', 'İstanbul', 'Straße', 'ΑΑΑ',
];
const POOL = [
  0x61, 0x62, 0x41, 0x20, 0x2c, 0x24, 0x30, 0x31, 0x2d, 0x301, 0xd83d, 0xde00, 0xd800, 0xdc00, 0x3a3,
  0x391, 0x130, 0xdf, 0xa0, 0x2028, 0xfeff, 0x10d0, 0x1c5, 0x7c, 0x26, 0x27, 0x60, 0x3c, 0x3e, 0x0,
];
const RAND_STR = Array.from({ length: 40 }, () =>
  String.fromCharCode(...Array.from({ length: rint(12) }, () => pick(POOL))),
);
const SUBJECTS = [...new Set([...BASE, ...RAND_STR])];
const NEEDLES = [
  '', 'a', 'b', 'ab', 'aa', ' ', '\ud83d', '\ude00', '😀', 'Σ', 'ΑΣ', 'ß', 'ss',
  '$&', "$'", '$`', '$$', '$1', '$<a>', '́', ',', 'x', 'an', 'é', ' ', 'İ', 'bana',
];
const REPS = ['', 'X', '$&', '[$&]', "$'", '$`', '$$', '$1', '$<a>', '$', '$0$', 'ab$&$&', '$$$&'];

// Table of code-unit strings. Subjects, needles and replacements are referenced by index.
const TAB = [];
const TAB_IX = new Map();
const ix = (s) => {
  if (!TAB_IX.has(s)) {
    TAB_IX.set(s, TAB.length);
    TAB.push(units(s));
  }
  return TAB_IX.get(s);
};

const arrayIndexOf = (s) => {
  const n = Number(s);
  return String(n) === s && Number.isInteger(n) && n >= 0 && n < 4294967295 ? n : null;
};
const S = {};
for (const k of [
  'toLowerCase', 'toUpperCase', 'trim', 'trimStart', 'trimEnd', 'isWellFormed', 'toWellFormed', 'arrayIndex',
  'codePoints', 'codeUnitAt', 'codePointAt', 'substring', 'indexOf', 'lastIndexOf', 'startsWith', 'endsWith',
  'includes', 'padStart', 'padEnd', 'repeat', 'split', 'replace', 'replaceAll', 'concat', 'compare',
  'fromCharCode', 'fromCodePoint',
]) {
  S[k] = [];
}
for (const s of SUBJECTS) {
  const si = ix(s);
  const L = s.length;
  S.toLowerCase.push([si, units(s.toLowerCase())]);
  S.toUpperCase.push([si, units(s.toUpperCase())]);
  S.trim.push([si, units(s.trim())]);
  S.trimStart.push([si, units(s.trimStart())]);
  S.trimEnd.push([si, units(s.trimEnd())]);
  S.isWellFormed.push([si, s.isWellFormed()]);
  S.toWellFormed.push([si, units(s.toWellFormed())]);
  S.arrayIndex.push([si, arrayIndexOf(s)]);
  S.codePoints.push([si, [...s].map((c) => units(c))]);
  for (const i of [0, 1, L - 1, L, L + 1]) {
    if (i < 0) continue;
    S.codeUnitAt.push([si, i, nullIfNaN(s.charCodeAt(i))]);
    const cp = s.codePointAt(i);
    S.codePointAt.push([si, i, cp === undefined ? null : [cp, cp > 0xffff ? 2 : 1]]);
  }
  for (const [a, b] of [[0, L], [1, 2], [2, 1], [L, 0], [0, 1], [L + 2, 3], [1, L + 1]]) {
    S.substring.push([si, a, b, units(s.substring(a, b))]);
  }
  for (const n of ['', ...sel(NEEDLES, 3)]) {
    const ni = ix(n);
    for (const f of [0, 1, L]) {
      S.indexOf.push([si, ni, f, nullIfMinus1(s.indexOf(n, f))]);
      S.lastIndexOf.push([si, ni, f, nullIfMinus1(s.lastIndexOf(n, f))]);
      S.startsWith.push([si, ni, f, s.startsWith(n, f)]);
      S.endsWith.push([si, ni, f, s.endsWith(n, f)]);
      S.includes.push([si, ni, f, s.includes(n, f)]);
    }
  }
  for (const t of [L + 1, L + 4]) {
    for (const fill of ['ab', ' ', '😀']) {
      S.padStart.push([si, t, ix(fill), threw(() => units(s.padStart(t, fill)))]);
      S.padEnd.push([si, t, ix(fill), threw(() => units(s.padEnd(t, fill)))]);
    }
  }
  for (const n of [0, 2, 3]) S.repeat.push([si, n, units(s.repeat(n))]);
  for (const sep of sel(NEEDLES, 2)) {
    for (const lim of [undefined, 0, 1, 2, 3]) {
      const parts = lim === undefined ? s.split(sep) : s.split(sep, lim);
      S.split.push([si, ix(sep), lim === undefined ? null : lim, parts.map((p) => units(p))]);
    }
  }
  for (const p of sel(NEEDLES, 2)) {
    for (const r of sel(REPS, 2)) {
      S.replace.push([si, ix(p), ix(r), units(s.replace(p, r))]);
      S.replaceAll.push([si, ix(p), ix(r), units(s.replaceAll(p, r))]);
    }
  }
  const other = pick(SUBJECTS);
  S.concat.push([si, ix(other), units(s.concat(other))]);
}
// Repeat/pad at or above the cap: V8 throws RangeError (its own cap is 2**29-24 units).
S.repeat.push([ix('abc'), 2 ** 29, threw(() => units('abc'.repeat(2 ** 29)))]);
S.repeat.push([ix('abc'), 2 ** 40, threw(() => units('abc'.repeat(2 ** 40)))]);
S.repeat.push([ix(''), 2 ** 40, units(''.repeat(2 ** 40))]);
S.padStart.push([ix('abc'), 2 ** 31, ix('x'), threw(() => units('abc'.padStart(2 ** 31, 'x')))]);
S.padEnd.push([ix('abc'), 2 ** 31, ix(''), threw(() => units('abc'.padEnd(2 ** 31, '')))]);

for (let i = 0; i < 300; i++) {
  const a = pick(SUBJECTS);
  const b = pick(SUBJECTS);
  S.compare.push([ix(a), ix(b), a < b ? -1 : a > b ? 1 : 0]);
}
for (let i = 0; i < 60; i++) {
  const u = Array.from({ length: rint(7) }, () => rint(65536));
  S.fromCharCode.push([u, units(String.fromCharCode(...u))]);
}
for (const cp of [0, 0x41, 0x7f, 0xd7ff, 0xd800, 0xdbff, 0xdc00, 0xdfff, 0xe000, 0xfffe, 0xffff, 0x10000, 0x1f600, 0x10ffff, 0x110000, 4294967295]) {
  let out;
  try {
    out = units(String.fromCodePoint(cp));
  } catch (e) {
    if (!(e instanceof RangeError)) throw e;
    out = null;
  }
  S.fromCodePoint.push([cp, out]);
}

// ---------------------------------------------------------------- emit
function emit(file, format, sections) {
  const lines = [
    '{',
    `  "format": ${JSON.stringify(format)},`,
    `  "oracle": ${JSON.stringify(`node ${process.version} (V8 ${process.versions.v8})`)},`,
    '  "generator": "tests/language/gen/gen-corpora.mjs",',
  ];
  sections.forEach(([name, rows], i) => {
    lines.push(`  ${JSON.stringify(name)}: [`);
    rows.forEach((row, j) => lines.push('    ' + JSON.stringify(row) + (j + 1 < rows.length ? ',' : '')));
    lines.push('  ]' + (i + 1 < sections.length ? ',' : ''));
  });
  lines.push('}');
  writeFileSync(join(OUT, file), lines.join('\n') + '\n');
  const counts = sections.map(([n, r]) => `${n}=${r.length}`).join(' ');
  console.log(`${file}: ${counts}`);
}

emit('number.json', 'vn-value number corpus v1', Object.entries(NUM));
emit('string.json', 'vn-value string corpus v1', [['strings', TAB], ...Object.entries(S)]);

# WP-L1 — `vn-value` primitives: UTF-16 strings and JavaScript number semantics

Read `docs/dev/AGENT-CONTRACT.md` and `docs/dev/ARCHITECTURE.md` §2 first.
`export CARGO_TARGET_DIR="$VN_ROOT/target/l1"`.

**Owned paths:** `crates/value/**` (you create the crate), `tests/language/gen/**`, `tests/language/corpora/**`.

## Why
Everything above this layer (heap, compiler output, Vue port) needs JavaScript-faithful strings and numbers:
UTF-16 code units incl. lone surrogates, `Number::toString`, `toFixed`, `ToNumber`, `ToInt32`, `%`, `**`,
rounding, `parseInt/parseFloat`. The prototype used Rust `Display`/`format!`, which is wrong for JS (that is a
known bug we must not repeat). V8 (Node) is the oracle.

## Crate
`crates/value/Cargo.toml`: package `vn-value`, `edition.workspace = true` (workspace metadata is in the root
`Cargo.toml`: use `edition.workspace = true`, `rust-version.workspace = true`, `publish.workspace = true`),
**no dependencies at all** (std only; not even dev-dependencies). Library name `vn_value`.
`src/lib.rs` declares `pub mod jsstr; pub mod number;` and re-exports `JsStr`. Later WPs will add more modules;
keep `lib.rs` minimal. No `unsafe`.

## `jsstr.rs` — `JsStr`
Immutable UTF-16 string, `#[derive(Clone)] pub struct JsStr(Rc<[u16]>)` (single-threaded; `Rc`), implementing
`PartialEq, Eq, Hash` (by code units), `Debug` (escaped, lone surrogates visible as `\u{d800}`-style text), `Display`
(lossy), `Default` (empty). Methods (all semantics = ECMAScript `String.prototype.*` on code units; parameters are
already-clamped `usize` values — the caller does `ToIntegerOrInfinity` clamping):
`empty() from_str(&str) from_units(&[u16]) from_vec(Vec<u16>) units() len() is_empty() to_rust_string_lossy()
try_to_string() -> Option<String> concat(&JsStr) code_unit_at(usize)->Option<u16> code_point_at(usize)->
Option<(u32,usize)> substring(start,end) (swaps if start>end, like `substring`) index_of(&JsStr,from)->Option<usize>
last_index_of(&JsStr,from)->Option<usize> starts_with(&JsStr,pos) ends_with(&JsStr,end_pos) includes(&JsStr,pos)
to_lower_case() to_upper_case() (full Unicode incl. special casing & final sigma — Rust's `str::to_lowercase`
semantics are acceptable after verifying against V8 on the corpus; lone surrogates pass through unchanged)
trim() trim_start() trim_end() (JS WhiteSpace + LineTerminator set incl. U+FEFF, U+00A0, U+2028/2029, U+3000 and
the Unicode "Zs" category) pad_start(target_len,&fill) pad_end(..) repeat(n)->Result<JsStr,StringTooLong>
(error above 2^30 code units) split_by_string(&sep, limit:u32)->Vec<JsStr> (empty separator splits into code units)
replace_first(&pattern,&replacement) replace_all(&pattern,&replacement) (string patterns only; implement
`GetSubstitution` for `$$`, `$&`, `` $` ``, `$'`; `$n`/`$<name>` stay literal when there are no captures)
compare(&JsStr)->Ordering (code unit order) from_char_code(&[u16]) from_code_point(u32)->Option<JsStr>
is_well_formed() to_well_formed() array_index()->Option<u32> (canonical array-index string only: no leading zeros,
< 2^32-1) code_points()->iterator of `JsStr` (string iterator: surrogate pairs yield one element; lone surrogates
yield themselves)`.
Regex, `normalize`, `localeCompare`, `Intl` are **out of scope**; do not stub them.

## `number.rs`
All take/return `f64`/`&JsStr`:
`number_to_string(f64)->String` (ECMA-262 `Number::toString`, radix 10, exact JS formatting: `1e+21`, `1e-7`,
`0.000001`, `-0 -> "0"`, `NaN`, `Infinity`), `number_to_string_radix(f64,u32)->String` (2..=36; integer part exact;
fractions use the V8-compatible algorithm — keep to cases V8 agrees on in your corpus), `to_fixed(f64,u32)->
Result<String,NumErr>` (digits 0..=100 else `Err(NumErr::Range)`; |x|>=1e21 → `number_to_string`; ties: JS picks the
larger n on the *exact* binary value, i.e. round-half-up on magnitude — NOT Rust's ties-to-even), `to_exponential(f64,
Option<u32>)`, `to_precision(f64,Option<u32>)` (same Err rules), `string_to_number(&JsStr)->f64` (`StringToNumber`:
whitespace trim, empty → 0, hex/octal/binary prefixes, `Infinity`, no numeric separators, invalid → NaN),
`parse_float(&JsStr)`, `parse_int(&JsStr, radix: i32)`, `to_int32 to_uint32 to_int16 to_uint16 to_int8 to_uint8
(f64)`, `js_rem(f64,f64)` (`%` incl. signed zero rules), `js_pow(f64,f64)` (`**`/`Math.pow` special cases, e.g.
`1**Infinity` is NaN), `js_round(f64)` (`Math.round`: halves toward +∞, −0 preserved), `js_min(&[f64])`,
`js_max(&[f64])` (NaN-propagating, −0 < +0), `same_value`, `same_value_zero`, `is_integer_valued`.
Hint: Rust's `{:e}` and `{:.N}` formatting are exact/shortest-round-trip and are a good *source of digits*; the JS
formatting rules (placement of the decimal point, exponent form, rounding mode) are yours to implement. For
`to_fixed` obtain the exact decimal expansion (e.g. `format!("{:.1100}", x.abs())`) and round it yourself.

## Oracle corpora (independent expected values from V8)
- `tests/language/gen/gen-corpora.mjs` (Node ≥ 22, no deps): deterministic (own seeded PRNG, e.g. xorshift) and
  writes `tests/language/corpora/number.json` and `tests/language/corpora/string.json`. Doubles are stored as 16-hex
  digit IEEE bit patterns plus Node's results. Include: all special values (±0, ±Infinity, NaN, MIN_VALUE, MAX_VALUE,
  EPSILON, 2^53±1), powers of ten and neighbours around 1e21 and 1e-7, binary fractions (0.5, 1.5, 2.5, 0.125, 1.005,
  8.345), ~3000 random bit-pattern doubles and ~2000 random "human" decimals; for each: `toString`, `toString(2/8/16/36)`
  on integer-valued doubles, `toFixed(0..=20)` (and a few of 21..100 for tiny samples), `toExponential`, `toPrecision(1..=21)`,
  and error cases (`toFixed(101)`, `toPrecision(0)`, `toExponential(-1)` → which RangeError). `ToNumber` on ~300 strings
  (whitespace, hex/octal/binary, `Infinity`, `1e3`, `.5`, `5.`, `0x`, `1_0`, `+`, `-`, Unicode spaces), `parseFloat`/`parseInt`
  (with radix 0,2,10,16,36,37,-1) on ~300 strings, `ToInt32/ToUint32/ToInt16/ToUint8` on doubles, `%` and `**` on pairs,
  `Math.round/min/max` cases. Strings: a corpus of ~120 diverse strings (ASCII, BMP, emoji pairs, lone high/low
  surrogates, combining marks, `İ`, `ß`, `ΑΣ` final sigma, Georgian, whitespace variants) with results for every `JsStr`
  method above on many parameter combinations (represent strings as arrays of code units, never as raw text).
  Commit the generated JSON (keep each file ≲ 2 MB; reduce counts if needed).
- Rust integration tests `crates/value/tests/number_corpus.rs` and `tests/string_corpus.rs` read the corpus with
  `include_str!` and a **test-only mini JSON reader** in `crates/value/tests/support/mod.rs` (the crate itself must
  stay dependency-free). Every case is compared; a failure prints the case and both values. Report any V8 behaviour
  you decided not to match and why.
- Also add small hand-written unit tests inside the modules for tricky cases (these must come from the spec/V8, not
  from the code under test), plus a **negative control**: a test that feeds the corpus comparer a deliberately wrong
  expectation and asserts it is reported as a mismatch.

## Acceptance (the reviewer will run)
```
source scripts/env.sh; export CARGO_TARGET_DIR=$VN_ROOT/target/l1
shasum -a 256 tests/language/corpora/*.json > /tmp/before.sha; node tests/language/gen/gen-corpora.mjs; shasum -a 256 -c /tmp/before.sha   # regeneration must be byte-identical
cargo test -p vn-value
```
All corpus cases pass or are listed individually as known deviations (with reasons) in your report; zero `unsafe`;
no dependencies.

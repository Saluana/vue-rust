# WP-B3 — String, Number, Math

Common rules: `docs/dev/wp/BUILTINS-COMMON.md`. `export CARGO_TARGET_DIR="$VN_ROOT/target/b3"`.
**Owned paths:** `crates/value/src/builtins/{string,number,math}.rs`, `tests/language/gen/gen-b3.mjs`, `tests/language/builtins/{string-*,number-proto,math-extra}.json` (O2 already supplies `math.json` and `number.json`: your implementation must pass them;
add new files, do not edit O2's), `crates/testkit/tests/replay_b3.rs`, `crates/value/tests/builtins_b3_*.rs`.

## Implement
- `String` (call = ToString incl. Symbol description rule `String(sym)`; construct = wrapper object via the kernel), `String.fromCharCode`, `String.fromCodePoint` (RangeError), and `String.prototype`: `at charAt charCodeAt codePointAt concat endsWith includes indexOf lastIndexOf padEnd padStart repeat
  replace replaceAll slice split startsWith substring substr toLowerCase toString toUpperCase trim trimEnd trimStart valueOf [Symbol.iterator] isWellFormed toWellFormed localeCompare(NO) normalize(NO) match(NO) matchAll(NO) search(NO)`.
  `replace/replaceAll/split` accept **string patterns only** (a RegExp argument cannot exist; any non-string pattern is converted with ToString); replacement may be a function `(match, offset, string)` or a string with `$` patterns; `split` with `undefined` separator and `limit`.
  Index/length coercion (`ToIntegerOrInfinity`, clamping) must match V8 for every method; use `vn_value::jsstr::JsStr` operations (WP-L1) underneath. String iterator objects (by code points; `%StringIteratorPrototype%`, tag "String Iterator").
- `Number` (call = ToNumber, construct = wrapper), `Number.isInteger isSafeInteger isFinite isNaN parseFloat parseInt EPSILON MAX_SAFE_INTEGER MIN_SAFE_INTEGER MAX_VALUE MIN_VALUE NaN POSITIVE_INFINITY NEGATIVE_INFINITY` (`parseFloat/parseInt` are the same function objects as the globals installed by WP-B1: if B1 has not landed, look the
  globals up lazily at call time / install after B1; coordinate through `install_all` ordering, tell the coordinator) and `Number.prototype.toString(radix) toFixed toPrecision toExponential valueOf toLocaleString(NO)` using the finished `number.rs` `*_js` entry points (RangeError mapping).
- `Math`: `abs acos acosh asin asinh atan atan2 atanh cbrt ceil clz32 cos cosh exp expm1 floor fround hypot imul log log10 log1p log2 max min pow round sign sin sinh sqrt tan tanh trunc` and the constants `E LN10 LN2 LOG10E LOG2E PI SQRT1_2 SQRT2`, `Math[Symbol.toStringTag] = "Math"`. NOT `Math.random` (the platform service provides randomness), NOT `Math.f16round`.
  Use `number.rs` helpers (`js_pow js_round js_min js_max`) and Rust `f64` intrinsics; results for transcendental functions may differ from V8 in the last ulp on some inputs: the O2 corpus marks those cases `approx`; report how many cases are not bit-exact per function so we can decide on a libm port later.

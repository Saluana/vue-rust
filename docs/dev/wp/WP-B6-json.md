# WP-B6 — JSON

Common rules: `docs/dev/wp/BUILTINS-COMMON.md`. `export CARGO_TARGET_DIR="$VN_ROOT/target/b6"`.
**Owned paths:** `crates/value/src/builtins/json.rs`, `tests/language/gen/gen-b6.mjs`, `tests/language/builtins/json-*.json`, `crates/testkit/tests/replay_b6.rs`, `crates/value/tests/builtins_b6_*.rs`.

## Implement
`JSON.parse(text[, reviver])` with the ECMAScript grammar (strict JSON: no trailing commas, no comments, no single quotes, no leading zeros, exponent forms, `\u` escapes incl. lone surrogates), duplicate keys (last wins), `__proto__` as an ordinary key (own property!), integer-like keys ordering,
deeply nested input bounded by a documented depth limit (RangeError, default 1000), reviver walk with holder semantics (deletion on `undefined`, array holes), `SyntaxError` for invalid input;
`JSON.stringify(value[, replacer[, space]])`: key order, `toJSON`, boxed primitives, replacer function and array (dedupe, number/string items), `space` as number (clamped to 10) or string (first 10 units), `undefined`/functions/symbols omitted in objects and `null` in arrays, `NaN/Infinity → null`, `-0 → "0"`, lone-surrogate escaping (`\udXXX` lowercase), `BigInt` n/a, cycle detection `TypeError`, indentation formatting, top-level `undefined` → `undefined`,
getters invoked in order; `JSON[Symbol.toStringTag] = "JSON"`. Number→text via the finished `number_to_string`.
Corpus: `JSON.parse` over ≥ 150 texts (valid and invalid; results encoded) and `JSON.stringify` over ≥ 150 values (use the encoded input graphs; `toJSON` coverage via a Date-less approach: objects with a `toJSON` *callback preset* property is not expressible — cover `toJSON`, replacer functions and revivers with Rust unit tests + O1-style scenarios `tests/language/cases/stdlib/*.js` additions are NOT yours; use unit tests with natively defined functions and expected strings taken from Node by a generator script that prints them).

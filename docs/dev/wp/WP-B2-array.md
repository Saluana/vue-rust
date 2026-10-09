# WP-B2 — Array

Common rules: `docs/dev/wp/BUILTINS-COMMON.md`. `export CARGO_TARGET_DIR="$VN_ROOT/target/b2"`.
**Owned paths:** `crates/value/src/builtins/array.rs` (split into `array/` submodules if it exceeds ~900 lines), `tests/language/gen/gen-b2.mjs`, `tests/language/builtins/array-*.json`,
`crates/testkit/tests/replay_b2.rs`, `crates/value/tests/builtins_b2_*.rs`.

## Implement
`Array(...)` call and construct (`Array(3)` → length-3 holes, `Array(1,2)`, `Array('3')`, RangeError for invalid length), `Array.isArray` (must be true for exotic proxies of arrays — the kernel's `is_array`), `Array.of`, `Array.from(items[, mapFn[, thisArg]])` (iterables via the iteration
protocol, array-likes via `length`, strings by code points), and `Array.prototype`: `at concat copyWithin(NO) entries every fill filter find findIndex findLast findLastIndex flat flatMap forEach includes indexOf join keys lastIndexOf map pop push reduce reduceRight
reverse shift slice some sort splice toString unshift values [Symbol.iterator] toReversed toSorted toSpliced with` (omit `copyWithin`; omit `Symbol.unscopables`), `Array.prototype.length` semantics via the kernel.
- Generic `this` (array-likes) where the spec allows it, but fast paths for real arrays.
- Holes: `forEach/map/filter/reduce/some/every/indexOf/lastIndexOf/flat/sort/concat/slice/splice/reverse/join/includes/find…` each treat holes exactly as V8 does (skip vs undefined; `map` preserves holes; `sort` moves holes to the end; `flat` removes holes; `includes/find/join` see undefined).
- `sort`: stable merge sort; default comparator = ToString code-unit comparison with `undefined` last; comparator results via ToNumber; a throwing comparator propagates without corrupting the array; V8's treatment of inconsistent comparators need not match beyond "no crash, deterministic".
- Array iterators: `entries/keys/values/[Symbol.iterator]` return iterator objects (`%ArrayIteratorPrototype%` with `next` and `[Symbol.toStringTag]: "Array Iterator"`), live-length semantics, exhausted state sticky.
- `Symbol.species` is NOT supported (results use plain arrays); `concat` honours `Symbol.isConcatSpreadable` only for real arrays (default).
- Mutation during iteration (`forEach` while pushing; `map` with shrinking array), `length` coercion edge cases (`ToLength`), huge indexes (RangeError rather than allocation), frozen arrays (TypeError on mutators), callbacks that throw (partial effects preserved as in V8), `thisArg`.
- Expose Rust helpers other crates need: `pub fn array_from_iterable(rt, &Value) -> Res<ObjRef>`, `pub fn iterate(rt, &Value, FnMut(Value) -> Res<bool>)`, and `pub fn create_list_iterator(...)` if you need them for the iterator objects; document them in ABI.md "Builtins: Array" section (append-only).

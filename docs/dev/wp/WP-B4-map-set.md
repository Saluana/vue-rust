# WP-B4 — Map, Set, iterator protocol objects

Common rules: `docs/dev/wp/BUILTINS-COMMON.md`. `export CARGO_TARGET_DIR="$VN_ROOT/target/b4"`.
**Owned paths:** `crates/value/src/builtins/{map_set,iterator}.rs`, `tests/language/gen/gen-b4.mjs`, `tests/language/builtins/{map,set,groupby}.json`, `crates/testkit/tests/replay_b4.rs`, `crates/value/tests/builtins_b4_*.rs`.
**Note for the trace encoder:** Map/Set objects are `ObjectKind::Native` payloads defined by you; the `vn-testkit` encoder (WP-T1) needs to enumerate them: export `pub fn map_entries(rt, ObjRef) -> Option<Vec<(Value, Value)>>` and `pub fn set_values(rt, ObjRef) -> Option<Vec<Value>>` and tell the maintainer the names, then wire them into the encoder yourself in `crates/testkit/src/trace.rs` **only in the clearly marked Map/Set section T1 left** (if the section is absent, ask in your report).

## Implement
`Map` and `Set` (construct only; `Map(…)` without new → TypeError) with insertion-ordered storage and SameValueZero keys (NaN equals NaN, -0 normalised to +0 on insertion, object keys by identity, strings by content), O(1) average lookup (hash on value), correct behaviour
under mutation during iteration (entries deleted before being reached are skipped, entries added during iteration are visited, delete + re-add moves to the end, `clear` during iteration ends it) — design the storage so iterators stay valid without O(n) work per step.
`new Map(iterable)` / `new Set(iterable)` via the iteration protocol (entries must be objects: TypeError otherwise), `Map.prototype`: `get set has delete clear forEach entries keys values size [Symbol.iterator]=entries [Symbol.toStringTag]`, `Set.prototype`: `add has delete clear forEach entries keys values size [Symbol.iterator]=values`
(plus ES2025 set algebra if Node 26 has it: `union intersection difference symmetricDifference isSubsetOf isSupersetOf isDisjointFrom` — implement them, V8 is the oracle), `Map.groupBy`, `Object.groupBy` is B1's.
Iterator objects: `%MapIteratorPrototype%`, `%SetIteratorPrototype%` (`next`, tags "Map Iterator"/"Set Iterator"), and a shared `%IteratorPrototype%` with `[Symbol.iterator]() { return this }`. Expose Rust helpers for the rest of the engine:
`pub fn get_iterator(rt,&Value)->Res<IterRecord>`, `iter_step`, `iter_close` (the ECMAScript iteration protocol with array/string/Map/Set fast paths that are *unobservable*), documented in ABI.md "Builtins: iteration" (append-only) — WP-C1b's generated prelude will call these if present, so keep the names stable and tell the maintainer.
WeakMap/WeakSet/WeakRef/FinalizationRegistry stay absent.

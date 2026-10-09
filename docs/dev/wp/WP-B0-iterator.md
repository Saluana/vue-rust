# WP-B0 — Iterator protocol foundation (`builtins/iterator.rs`)

Common rules: `docs/dev/wp/BUILTINS-COMMON.md` (including the REQUIRED private-mirror workflow). `export CARGO_TARGET_DIR="$VN_ROOT/target/b0"` (inside the mirror).
**Owned paths:** `crates/value/src/builtins/iterator.rs`, `crates/value/tests/builtins_iterator.rs`, and an appended section "Builtins: iteration" in `crates/value/ABI.md` (append-only).
This is a SMALL package that other builtin families wait for: **push a compiling first version within your first ~15 minutes** (types + `%IteratorPrototype%` + `get_iterator`/`iter_step`/`iter_close`), then refine and test.

## Implement (names normative)
```rust
pub struct IterRecord { pub iterator: Value, pub next: Value, pub done: bool }
pub fn get_iterator(rt: &Rt, v: &Value) -> Res<IterRecord>;            // GetIterator(v, sync): method = v[Symbol.iterator]; not callable/absent → TypeError "<x> is not iterable"; result must be an object
pub fn iter_step(rt: &Rt, rec: &mut IterRecord) -> Res<Option<Value>>;  // IteratorStepValue: calls next, validates result object, reads done then value; marks rec.done on done OR on any throw inside
pub fn iter_close(rt: &Rt, rec: &IterRecord, completion_is_throw: bool) -> Res<()>;   // IteratorClose: calls iterator.return if present; errors from return are swallowed when completion_is_throw, otherwise propagate; result must be an object
pub fn iterate(rt: &Rt, v: &Value, f: &mut dyn FnMut(Value) -> Res<bool>) -> Res<()>;  // for-of driver: f returns Ok(true) to continue, Ok(false) to stop early (→ iter_close, normal completion), Err → iter_close(throw) then propagate
pub fn create_iter_result(rt: &Rt, value: Value, done: bool) -> Value;  // {value, done} plain object (property order: value, done)
// Builtin iterator objects (Array/String/Map/Set families implement `NativeIter` and build objects with it):
pub trait NativeIter: NativeObject { fn next(&self, rt: &Rt) -> Res<Option<Value>>; }   // None = exhausted (and must stay exhausted)
pub fn make_iterator_prototype(rt: &Rt, registry_name: &'static str, tag: &'static str) -> ObjRef;  // proto whose [[Prototype]] is %IteratorPrototype%, with own `next` (reads the NativeIter payload of `this`; TypeError for wrong receivers) and [Symbol.toStringTag]: tag; registered with rt.register_builtin(registry_name, obj)
pub fn new_iterator_object(rt: &Rt, proto_registry_name: &str, payload: Rc<dyn NativeIter>) -> ObjRef;
pub fn iterator_prototype(rt: &Rt) -> ObjRef;                          // %IteratorPrototype% (registered as "IteratorPrototype") with [Symbol.iterator]() { return this }
```
- `install(rt)` creates and registers `%IteratorPrototype%`. Keep `install` free of dependencies on other families (it runs first).
- `get_iterator` must work for any value (primitives like strings use `String.prototype[Symbol.iterator]` once B3 installs it; until then a string is "not iterable" — fine), arrays use `Array.prototype[Symbol.iterator]` (installed by B2).
- No unobservable fast paths in this WP (full protocol everywhere; keep the code straightforward).
- GC: iterator payloads implement `trace`; test with `rt.collect()` between steps.

## Tests (hand-written expectations from ECMA-262; verify behaviours against Node where practical with a tiny generator printing the orders and commit its output as comments or JSON)
User-defined iterators built from native functions: protocol order (`next` read once, result `done` before `value`), non-object results → TypeError, early exit calls `return` exactly once, `return` throwing during normal vs throw completion, `return` returning a non-object, `done:true` stops without calling `return`, throw inside `next` marks done and does not call `return`, re-entrancy, `NativeIter` objects (a counting iterator) through `make_iterator_prototype`, wrong receiver for `next`, toStringTag, GC safety. ≥ 30 tests.

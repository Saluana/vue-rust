# WP-L2a — `vn-value` kernel: values, heap + GC, object model, functions, cells, errors

Read `docs/dev/AGENT-CONTRACT.md` and `docs/dev/ARCHITECTURE.md` §2 first. `export CARGO_TARGET_DIR="$VN_ROOT/target/l2a"`.
**Prerequisite:** WP-L1 is done: `crates/value` already contains `JsStr` (`jsstr.rs`) and number functions
(`number.rs`). Read their public API; do not modify those two files (put requests in your report).

**Owned paths:** new files `crates/value/src/{value,heap,object,function,rt,error,native,roots}.rs`,
`crates/value/ABI.md`, `crates/value/tests/kernel_*.rs`; and `crates/value/src/lib.rs` (add `pub mod` lines and
re-exports only). Dependencies: std only, no new crates. `unsafe` is **forbidden**.

## What the generated code and every later crate rely on
This kernel is the base of the whole product: compiled application code (emitted Rust), the Vue reactivity/runtime
ports, builtins and hosts all call it. Correctness of identity, property order, accessor behaviour and GC rooting
matters more than speed. Semantics follow ECMAScript (ES2025) **strict mode** (modules are always strict):
failed assignments to non-writable properties throw `TypeError` at the `Rt::set_*` level via the `Res` result.

## Required design (normative)

1. **Types.** `Value = Undefined | Null | Bool(bool) | Number(f64) | String(JsStr) | Symbol(Sym) | Object(ObjRef)`
   plus two internal markers that never reach user code: `Uninit` (TDZ) and `Hole` (array hole).
   `ObjRef { slot: u32, gen: u32 }` is `Copy + Eq + Hash`. `Sym(u32)` indexes a runtime symbol table (symbols
   are immortal; descriptions kept). `PropKey = Index(u32) | Str(JsStr) | Sym(Sym)`; invariant: a `Str` key never
   spells a canonical array index (constructor `PropKey::from_jsstr` normalizes; use `JsStr::array_index`).
   `Throw(pub Value)`, `type Res<T> = Result<T, Throw>`. JS `throw` is `Err`, never a panic.
2. **Heap.** Non-moving arena: `Vec` of slots with a free list and a per-slot generation that is bumped when the
   slot is freed. Accessing a stale `ObjRef` must be detected (return a framework-bug error / debug panic with a
   clear message), never silently read another object. `ObjectKind`: `Ordinary`, `Array(Vec<Value>)` (dense,
   `Value::Hole` marks holes, `length == vec.len()`; refuse growth beyond a documented dense limit with a
   `RangeError` instead of allocating gigabytes), `Function(FunctionData)`, `Cell(Value)` (closure variable),
   `Error`, `Native(Rc<dyn NativeObject>)` (host payload; later: Map/Set/Promise are added by other WPs through
   this or new variants — design `ObjectKind` and `Trace` so adding a variant is a local change), `Exotic(Rc<dyn Exotic>)`
   (proxy-like hooks used later by reactivity: `get/set/has/delete/own_keys/get_own_property/define` receive
   `&Rt`, the object ref and the receiver; default methods fall back to ordinary behaviour; plus `trace`).
   Every object also has an optional private extension slot `ext: Option<Rc<dyn NativeObject>>` (API: `set_ext(obj, Rc<dyn
   NativeObject>)`, `ext(obj) -> Option<Rc<dyn NativeObject>>`) for subsystem-private per-object data (the reactivity port stores
   its dependency maps and proxy caches there, so they live and die with the object and need no weak tables); it is traced.
   Objects have `proto: Option<ObjRef>`, `extensible`, and an insertion-ordered `PropertyMap` (data or accessor
   properties with writable/enumerable/configurable flags). Integer-index keys enumerate first in ascending
   numeric order, then string keys in insertion order, then symbols in insertion order (ordinary
   `[[OwnPropertyKeys]]`). Property lookup must be better than O(n) for large objects (index map past a threshold).
3. **Garbage collection** — mark-and-sweep, **iterative marking (no recursion)**, collection only at safepoints:
   - `Rt::collect()` may only run when `call_depth() == 0` (no generated code on the Rust stack). If called deeper
     it must return an error value/panic with a clear "GC inside running code" framework-bug message; also provide
     `Rt::try_collect() -> bool` and `Rt::maybe_collect()` (collects when allocations since the last GC exceed a
     configurable threshold and depth is 0).
   - Roots: global object, all intrinsics, persistent roots `Rt::root(&Value) -> RootId` / `unroot(RootId)`,
     and `RootProvider`s (`trait RootProvider { fn trace_roots(&self, t: &mut Tracer); }`, registered with
     `Rt::add_root_provider(Rc<dyn RootProvider>) -> ProviderId` / removed). Rust-side structures that store
     `ObjRef`s must implement `Trace` (`fn trace(&self, t: &mut Tracer)`) and be reachable from a root or live in a
     heap `Native`/`Exotic` object.
   - Dead `Native`/`Exotic` payloads are dropped by the sweep; dropping must not call into `Rt`.
   - Stats: `HeapStats { live_objects, allocated_total, collections, freed_total, bytes_estimate }`.
   - `Rt::set_heap_limit(bytes)`: `Rt::tick()` (see 7) returns a `RangeError` throw when the estimate exceeds it.
4. **Functions.** One code type for compiled and native functions:
   `type CodeFn = fn(&Rt, &Call<'_>) -> Res<Value>;`
   `struct Call<'a> { this: &'a Value, args: &'a [Value], callee: ObjRef, new_target: &'a Value }`.
   `FunctionData { code: CodeFn, captures: Rc<[ObjRef]> /*cells, traced*/, data: Rc<[Value]> /*native closure data, traced*/,
   name: JsStr, length: u32, kind: FnKind { Normal, Arrow, Method, Getter, Setter, Async }, ctor: Option<CodeFn> }`.
   No Rust closure may own a `Value`/`ObjRef` (they would be invisible to the collector). API:
   `new_function(code, FnInfo{name,length,kind}, captures)`, `new_native_function(code, name, length, data)`,
   `captures(callee) -> Rc<[ObjRef]>`, `fn_data(callee)`, `call(&Value callee, &Value this, &[Value]) -> Res<Value>`,
   `construct(&Value callee, &[Value], new_target) -> Res<Value>` (only functions with `ctor` — builtin
   constructors; user functions are not constructors, `TypeError`), `is_callable`, `is_constructor`.
   `call` increments/decrements `call_depth` and enforces `MAX_CALL_DEPTH` (default 1500, configurable; exceeding
   throws `RangeError`: "Maximum call stack size exceeded"). Function objects expose own `name`/`length` data
   properties (non-writable, non-enumerable, configurable) exactly like JS.
5. **Cells and TDZ.** `new_cell(Value) -> ObjRef`, `cell_get(ObjRef) -> Value`, `cell_set(ObjRef, Value)`, plus
   checked forms for lexical bindings: `cell_get_checked(ObjRef, name:&str) -> Res<Value>` throws `ReferenceError`
   ("Cannot access 'x' before initialization") when the cell holds `Uninit`; `cell_init(ObjRef, Value)`.
6. **Errors.** `Rt::make_error(ErrorKind, &str) -> Value` and `throw_type_error / throw_range_error /
   throw_reference_error / throw_syntax_error(msg) -> Throw`. Error objects: `ObjectKind::Error`, own non-enumerable
   `message`, `stack` (string `"{name}: {message}\n    at <anonymous>"` — simple for now) and prototype chain
   `Error.prototype` ← `TypeError.prototype` etc. with `name`; constructors `Error, TypeError, RangeError,
   ReferenceError, SyntaxError, EvalError, URIError` callable with or without `new`, accepting `(message,
   {cause})`; `Error.prototype.toString`.
7. **Watchdog.** `Rt::tick() -> Res<()>` increments an operation counter and checks (a) the heap limit estimate and
   (b) an optional `execution budget` (max ticks per top-level `Rt::run_job`, default unlimited); exhaustion throws
   a `RangeError` whose `name` is `"RangeError"` and message `"execution budget exceeded"`/`"heap limit exceeded"`.
   Generated code will call it on function entry and loop back-edges.
8. **Property operations** (spec semantics; all return `Res`; none may hold a `RefCell` borrow while calling a
   getter/setter/native/exotic hook): `get(obj, &key, receiver)`, `set(obj, &key, v, receiver) -> Res<bool>`,
   `define_data(obj,key,v,flags)`, `define_accessor(obj,key,get,set,flags)`, `get_own_property`, `delete(obj,&key)
   -> Res<bool>`, `has_property`, `has_own_property`, `own_keys(obj) -> Res<Vec<PropKey>>`, `prevent_extensions`,
   `freeze`, `seal`, `is_frozen`, `is_sealed`, `get_prototype_of`, `set_prototype_of` (kernel/internal use only).
   Value-level wrappers on `&Value` handle primitives: `get_v(&Value, &PropKey)` (strings: `length`, index
   access, then `String.prototype` — the prototype object exists but is populated by later WPs;
   numbers/booleans/symbols via their prototypes; `null`/`undefined` → `TypeError`
   "Cannot read properties of undefined (reading 'x')"), `set_v`, plus convenience `get_str(&Value, &str)`/
   `set_str` using a `&'static str` interner (`Rt::key(&'static str) -> PropKey`). Arrays: `length` is an exotic
   own data property (writable, non-enumerable, non-configurable); writing `length` truncates/extends (new slots are
   holes); writing index ≥ length extends with holes; `Array.isArray` support via `is_array`. Writing to a frozen
   array or non-writable property returns `false` from `set` (the `*_strict` wrappers turn `false` into `TypeError`
   "Cannot assign to read only property 'x' of object" — provide `set_strict`/`set_v_strict` for generated code).
9. **Realm bootstrap.** `Rt::new()` creates: global object (`globalThis`), `Object.prototype`, `Function.prototype`,
   `Array.prototype`, `String.prototype`, `Number.prototype`, `Boolean.prototype`, `Symbol.prototype` (all empty
   except what you implement here), the Error hierarchy above, `Intrinsic` enum + `Rt::intrinsic(Intrinsic) ->
   ObjRef`, and helpers other WPs use to register builtins: `define_native_method(obj, "name", code, length)`,
   `define_native_getter`, `install_global(name, Value)`, `new_object()`, `new_object_with_proto(Option<ObjRef>)`,
   `new_array(Vec<Value>)`. Well-known symbols: `Symbol.iterator`, `Symbol.asyncIterator`, `Symbol.toPrimitive`,
   `Symbol.toStringTag`, `Symbol.hasInstance` as `Sym` constants on the runtime (`Rt::well_known(WellKnown)`);
   `Symbol.for` registry: `Rt::symbol_for(&JsStr) -> Sym`, `Rt::new_symbol(Option<JsStr>) -> Sym`.
10. **Jobs.** A FIFO microtask queue owned by `Rt`: `enqueue_job(func: Value, args: Vec<Value>)` (traced as a root),
    `run_jobs() -> usize` drains it completely (jobs enqueued while draining run in the same drain), running each job with the
    call-depth bookkeeping of `call`; a job that throws does not stop the drain: its `Throw` is appended to a list
    returned by `take_uncaught_job_errors()`. `run_jobs` never collects garbage by itself; the host calls `maybe_collect()`
    at depth 0 afterwards. (Promises are built on this queue by a later WP.)
11. **Borrow discipline.** `Rt` is `Clone` (an `Rc` handle). Inner state sits in separate `RefCell`s; every method takes
    short borrows and clones data out before invoking user/native code. Reentrancy (a getter that reads the same
    object, a native that mutates the object being iterated) must work and be tested.

## Documentation deliverable
`crates/value/ABI.md`: the final public API grouped by purpose, one line of semantics each, written for the person
building the JS→Rust compiler (what to call for: variable cells, function creation, calls, member get/set,
object/array literals, errors, tick, roots) plus the GC/rooting rules and the "no Value in Rust closures" rule.

## Required tests (`crates/value/tests/kernel_*.rs`, ≥ 60 focused tests; expected values written by hand from the spec)
Property order (integers first, insertion, symbols); accessors (getter/setter, inherited setter, getter reentrancy);
flags + freeze/seal/preventExtensions (strict failures); arrays (length truncate/extend, holes, index extension,
frozen); function name/length props; call/this/args/new_target plumbing; recursion limit; cells + TDZ errors;
errors (ctor with/without new, prototype chain, `toString`, `cause`); symbols (unique, `for`, descriptions);
**GC**: unreachable cycles are freed, objects held by persistent roots / root providers / function captures / cells /
Native+Exotic `trace` survive, stale `ObjRef` detection after free + slot reuse (generation), `collect()` inside a
running call is refused, heap-limit and budget watchdogs trip; a **randomized stress test** (seeded PRNG written in the
test) that builds random object graphs with random mutations, keeps a reference model of reachability computed
independently in the test, collects repeatedly, and asserts live-object count and sampled contents equal the model.
Include one deliberate **negative control** (a root provider that forgets to trace must make an assertion about
survival fail — assert that the *test helper detects* it).

## Acceptance (the reviewer will run)
`source scripts/env.sh; export CARGO_TARGET_DIR=$VN_ROOT/target/l2a; cargo test -p vn-value` — all pass; no
`unsafe`; no warnings from `cargo build -p vn-value` (treat warnings as defects); ABI.md present.

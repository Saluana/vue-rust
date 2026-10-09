# vn-value kernel ABI (WP-L2a)

Public API of `vn-value` for the JS-to-Rust compiler, the Vue ports and the hosts. Semantics follow ECMAScript
(ES2025) strict mode. Every operation that can run user code or throw returns `Res<T> = Result<T, Throw>`; a
JavaScript `throw` is `Err(Throw(value))`, never a Rust panic. A panic means a framework bug.

## 0. What the compiler emits, and the call for it

| Source construct | Call |
|---|---|
| `let`/`const` binding (captured or not) | `new_cell(Uninit)`, `cell_init`, `cell_get_checked`, `cell_set_checked` |
| `var`, parameter captured by a closure | `new_cell(Undefined)`, `cell_get`, `cell_set` |
| Function expression, declaration, arrow | `new_function(code, FnInfo::new(..), &captures)` |
| Call `f(a, b)` | `call(&f, &this, &[a, b])` |
| `new C(a)` on a builtin constructor | `construct(&c, &[a], &c)` |
| `o.k`, `o[k]` | `get_v(&o, &key)` (key from `to_property_key` when computed) |
| `o.k = v` (strict) | `set_v_strict(&o, &key, v)` |
| `{ k: v }` | `new_object()` then `define_data(obj, key, v, Flags::ALL)` |
| `[a, , b]` | `new_array(vec![a, Value::Hole, b])` |
| `throw` of a builtin error, runtime error | `Err(rt.throw_type_error(..))` (`throw_range_error`, ...) |
| Loop back-edge, function entry | `rt.tick()?` |
| Value held by the host across a collection | `root` / `unroot` |

## 1. Values and handles

- `Value`: `Undefined | Null | Bool | Number(f64) | String(JsStr) | Symbol(Sym) | Object(ObjRef)`. Internal
  markers that never reach user code: `Uninit` (temporal dead zone) and `Hole` (array hole).
- `ObjRef`: `Copy + Eq + Hash` heap handle `{slot, gen}`. A handle to a freed object is stale; any access panics
  with "stale ObjRef ... (framework bug)". `Rt::is_live(obj)` checks without panicking.
- `Sym`: immortal symbol id. `Rt::symbol_description(sym)`.
- `PropKey`: `Index(u32) | Str(JsStr) | Sym(Sym)`. Build keys with `PropKey::from_str` or `from_jsstr`; a
  canonical array index is always `Index`. `Rt::to_property_key(&value)` applies ToPropertyKey.
- `Throw(pub Value)`, `Res<T>`. `Value::to_boolean()`, `vn_value::same_value(a, b)` (SameValue).

## 2. Realm

- `Rt::new()`: global object (`globalThis`), `Object.prototype`, `Function.prototype` (callable), `Array.prototype`
  (an array), `String/Number/Boolean/Symbol.prototype`, the Error hierarchy, well-known symbols.
- `Rt`: `Clone` handle (an `Rc`). Methods take `&self`. Inner borrows are short; no borrow is held while user
  code, getters, setters, exotic hooks or native callbacks run.
- `rt.intrinsic(Intrinsic::X) -> ObjRef`: `ObjectPrototype`, `FunctionPrototype`, `ArrayPrototype`,
  `StringPrototype`, `NumberPrototype`, `BooleanPrototype`, `SymbolPrototype`, `ErrorPrototype`, `TypeErrorPrototype`,
  ..., `ErrorCtor`, `TypeErrorCtor`, ..., `Global`.
- Builtin registration (infallible; a refusal is a framework bug): `define_native_method(obj, name, code, len)`,
  `define_native_getter(obj, name, code)`, `install_global(name, value)`.

## 3. Objects and property access (compiled member access and literals)

- Creation: `new_object()`, `new_object_with_proto(Option<ObjRef>)`, `new_array(Vec<Value>)` (`Value::Hole` for
  elisions; panics above `MAX_DENSE_LEN`, because the caller built the vector).
- Get: `get_v(&base, &key) -> Res<Value>` (any base: objects, strings with `length` and indices, primitives via
  their prototypes; `undefined`/`null` throw TypeError). `get(obj, &key, &receiver)` is the object form.
- Set: `set_v_strict(&base, &key, v) -> Res<()>` (module code is strict: refusal throws). `set_v`/`set`
  return `Res<bool>` for the sloppy-style result. `set_strict(obj, &key, v)` for objects.
- Literals: `define_data(obj, key, v, Flags::ALL)` (CreateDataProperty; a repeated key replaces the value).
  `define_accessor(obj, key, get, set, flags)`, `define_own(obj, &key, Prop)` for complete descriptors,
  `create_data_property`.
- Queries: `get_own_property -> Res<Option<Prop>>`, `has_property`, `has_own_property`, `own_keys -> Res<Vec>`
  (integer keys ascending, strings by insertion, symbols by insertion; arrays list `length` after the indices).
- Mutation: `delete(obj, &key) -> Res<bool>`, `prevent_extensions -> bool`, `freeze`/`seal -> Res<bool>`,
  `is_frozen`/`is_sealed -> Res<bool>`, `set_prototype_of -> bool` (refuses cycles), `get_prototype_of`.
- Helpers: `get_str(&base, "name")`, `set_str(&base, "name", v)`, `is_array(&value)`, `type_of(&value)`.
- Attributes: `Flags::ALL`, `HIDDEN` (writable, configurable: builtins), `CONFIG` (configurable: function
  `name`/`length`), `NONE`, `Flags::new(w, e, c)`. `Prop::Data{value, flags}` / `Prop::Accessor{get, set, flags}`.

## 4. Functions and calls

- `new_function(code, FnInfo::new(name, length, kind), &captures) -> ObjRef`: a compiled function. Its
  `captures` are cells, read with `captures(callee)`. It is not a constructor.
- `new_native_function(code, name, length, data: Vec<Value>)`: builtin; `data` is read with `fn_data(callee)`.
- `new_native_constructor(code, ctor_code, name, length, data)`: `code` runs for calls, `ctor_code` for `construct`.
- `call(&callee, &this, &args) -> Res<Value>`: `this` is passed unchanged. Calling a non-function throws TypeError.
- `construct(&callee, &args, &new_target) -> Res<Value>`: only builtin constructors. User functions throw TypeError.
- `CodeFn = fn(&Rt, &Call<'_>) -> Res<Value>`; `Call { this, args, callee, new_target }`, `c.arg(i)`
  (undefined past the end).
- `is_callable`, `is_constructor`, `call_depth()`, `max_call_depth()`, `set_max_call_depth(n)` (default 1500).
  Exceeding it throws RangeError "Maximum call stack size exceeded"; the depth is restored on unwinding.
- Functions own `length` then `name` (configurable only, not writable, not enumerable).

## 5. Cells (closure variables and let/const)

- `new_cell(v) -> ObjRef`. For `let`/`const`, create with `Value::Uninit` (TDZ).
- `cell_get(c)`, `cell_set(c, v)` for `var` and parameters.
- `cell_get_checked(c, "x") -> Res<Value>`: ReferenceError "Cannot access 'x' before initialization" in the TDZ.
- `cell_set_checked(c, "x", v) -> Res<()>`: the same ReferenceError when assigning before the declaration.
- `cell_init(c, v)`: the declaration's initialisation.

## 6. Errors

- `throw_type_error(msg)`, `throw_range_error`, `throw_reference_error`, `throw_syntax_error` -> `Throw`.
  `throw_error(ErrorKind, msg)`, `make_error(ErrorKind, msg) -> Value`. Use as `return Err(rt.throw_type_error(..))`.
- Error objects (`ObjectKind::Error`) own non-enumerable `stack`, `message` (when given) and `cause` (when the
  options object has it). `Error`, `TypeError`, `RangeError`, `ReferenceError`, `SyntaxError`, `EvalError`,
  `URIError` are callable with or without `new` and accept `(message, { cause })`. `Error.prototype.toString` is
  installed.

## 7. Coercions

- `to_primitive(&v, Hint::{Default, Number, String})`, `to_number(&v) -> Res<f64>`, `to_string(&v) -> Res<JsStr>`,
  `to_property_key(&v) -> Res<PropKey>`. Objects use `Symbol.toPrimitive`, then valueOf/toString.

## 8. Symbols

- `new_symbol(Option<JsStr>) -> Sym` (unique), `symbol_for(&JsStr) -> Sym` (registry), `symbol_description(sym)`.
- `well_known(WellKnown::{Iterator, AsyncIterator, ToPrimitive, ToStringTag, HasInstance}) -> Sym`.

## 9. Exotic objects and host payloads

- `trait Exotic`: `trace` (required) plus optional hooks `get`, `set`, `has`, `delete`, `own_keys`,
  `get_own_property`, `define`. Each returns `Res<Hook<T>>`: `Hook::Handled(v)` answers, `Hook::Ordinary` runs the
  ordinary algorithm for that object. `new_exotic_object(proto, Rc<dyn Exotic>)`.
- `trait NativeObject: Any` (required `trace`). `new_native_object(proto, Rc<dyn NativeObject>)`,
  `native_payload(obj)`, `payload_ref::<T>(&*payload)` to downcast.
- `set_ext(obj, Rc<dyn NativeObject>)` / `ext(obj)`: a private per-object slot for any object (traced).

## 10. Jobs and the watchdog

- `enqueue_job(func, args)`: FIFO. `run_jobs() -> usize` drains the queue, including jobs queued while draining.
  A throwing job is recorded; `take_uncaught_job_errors() -> Vec<Throw>` returns and clears them. `run_jobs`
  panics inside running code and never collects garbage; the host calls `maybe_collect()` afterwards.
- `tick() -> Res<()>`: generated code calls it on function entry and loop back-edges. It throws RangeError
  "heap limit exceeded" while `HeapStats::bytes_estimate` is above `set_heap_limit(bytes)`, and RangeError
  "execution budget exceeded" after more than `set_execution_budget(n)` ticks. `run_jobs` resets the counter before
  each job; ticks outside jobs accumulate. `clear_heap_limit()`, `clear_execution_budget()`.

## 11. Roots, collection and rules

- `root(&v) -> RootId`, `unroot(id)`, `root_value(id)`: persistent roots.
- `add_root_provider(Rc<dyn RootProvider>) -> ProviderId`, `remove_root_provider(id)`: a provider implements
  `trace_roots(&self, t: &mut Tracer)` and calls `t.mark` / `t.mark_value` for every ObjRef it holds.
- `collect()` (panics with "GC inside running code" at call depth > 0), `try_collect() -> bool` (false instead of
  panicking), `maybe_collect() -> bool` (collects when allocations since the last collection reach
  `set_gc_threshold(n)` and no code is running). `heap_stats() -> HeapStats { live_objects, allocated_total,
  collections, freed_total, bytes_estimate }`.

Rules:

1. Collection runs only at safepoints (`call_depth() == 0`). Compiled code and native code run at depth >= 1, so
   their temporaries on the Rust stack are never collected. The host collects between jobs.
2. Roots: the global object, intrinsics, persistent roots, the job queue (function and arguments), uncaught job
   errors, registered root providers, and everything reachable from them.
3. Rust locals are not roots. Host code that keeps an `ObjRef` across a collection must `root` it or re-read it
   from a root.
4. No `Value` or `ObjRef` inside a Rust closure: the collector cannot see it. Use cells, function captures or
   `new_native_function` data.
5. A Rust type that stores `ObjRef`s implements `Trace` (or `NativeObject`/`Exotic` `trace`) and must be reachable
   from a root or a heap object. A missing `mark` frees live objects; `tests/kernel_gc.rs` has a negative control.
6. Dropping a payload must not call into `Rt` (it runs inside the sweep).
7. Marking uses an explicit worklist, so deep structures do not overflow the Rust stack.

## 12. Deviations and conservative choices

- Array elements share one attribute set (`writable`, `configurable`, enumerable true). `freeze` and `seal` change
  it uniformly. Defining an accessor on an array index, or a non-default attribute on one element, throws
  TypeError "unsupported: ..." instead of applying it silently.
- Dense arrays are limited to `MAX_DENSE_LEN` (4 194 304) elements. Growth past it throws RangeError
  "Invalid array length: ...".
- `length = x`: x is converted once with ToNumber; a non-integer or out-of-range value throws RangeError
  "Invalid array length" (V8 also calls `valueOf` twice for objects).
- Refused strict assignments: read-only or accessor-less properties give "Cannot assign to read only property 'x'
  of object"; adding to a non-extensible object gives "Cannot add property x, object is not extensible"; a refused
  `length` truncation uses the read-only message (V8 says "Cannot delete property").
- Error `stack` is `"{name}: {message}\n    at <anonymous>"`; with an empty message it is `"{name}\n    at <anonymous>"`
  (the first line matches V8). Own key order is `stack`, `message`, `cause`, as in V8.
- `String.prototype`, `Number.prototype`, `Boolean.prototype` are ordinary objects. There are no String, Number or
  Boolean wrapper objects in the kernel.
- User functions have no `prototype` property. Builtin constructors get theirs from the installer.
- `call` does not coerce `this` (no sloppy-mode boxing). `FnKind::Async` is recorded; the kernel does not create
  promises (a later WP does).
- Heap limit uses an estimate: 96 bytes per object, 48 per property, 24 per array element. It is not measured memory.
- Allocation never throws. Limits are enforced at `tick()`.
- Generation counters are `u32`; a slot freed 2^32 times could alias a stale handle (not reachable in practice).
- Removing a property is O(n) (insertion order is kept); lookup is O(1) past 8 properties.

## 13. Operators & conversions (WP-L2b)

Code: `src/coerce.rs` (conversions), `src/ops.rs` (operators), with the kernel amendments below. This section
supersedes the §12 statement that the kernel has no String, Number or Boolean wrapper objects. Tests:
`tests/ops_kernel.rs`, `tests/ops_conv.rs`, `tests/ops_ops.rs`, and `tests/ops_corpus.rs` (V8 corpus
`tests/language/corpora/ops.json`, written by `tests/language/gen/gen-ops-corpus.mjs`). No BigInt anywhere.

### 13.1 API (all on `Rt` unless noted; operands are `&Value`)

| Name | Result | Notes |
|---|---|---|
| `to_boolean` | `Value::to_boolean()` | kernel method, reused |
| `to_primitive(v, Hint)`, `to_number`, `to_string`, `to_property_key` | as in §7 | kernel, unchanged |
| `to_numeric(v)` | `Res<f64>` | ToNumber without BigInt |
| `to_object(v)` | `Res<ObjRef>` | primitives get an `ObjectKind::Wrapper` (§13.2) |
| `to_integer_or_infinity(v)` | `Res<f64>` | never -0 |
| `to_length(v)` | `Res<f64>` | integer in [0, 2^53-1] |
| `to_index(v)` | `Res<f64>` | undefined gives 0; RangeError outside [0, 2^53-1] |
| `to_int32(v)`, `to_uint32(v)` | `Res<i32>`, `Res<u32>` | |
| `typeof_(v)` | `&'static str` | same as `type_of`; the name avoids the Rust keyword |
| `add sub mul div rem pow` | `Res<Value>` | `add` concatenates when a primitive is a string |
| `neg pos bit_not inc dec` | `Res<Value>` | `inc`/`dec` return the new value; postfix is the compiler's job |
| `bit_and bit_or bit_xor shl shr ushr` | `Res<Value>` | shift counts are masked with 31 |
| `not_(v)` | `bool` | negated ToBoolean, never throws |
| `lt gt le ge loose_eq` | `Res<Value>` (a Bool) | `!=` is the negation of `loose_eq`; `!==` of `strict_eq` |
| `instance_of(a, b)` | `Res<bool>` | see §13.4 for `Symbol.hasInstance` |
| `in_operator(key, obj)` | `Res<bool>` | RHS checked before the key is converted |
| `strict_eq(a, b)`, `same_value_zero(a, b)` | `bool` | free functions, re-exported at the crate root |

`same_value` is the kernel's free function (value.rs). Comparisons evaluate their conversions in spec order:
`a < b` converts `a` first; `a > b` is `b < a` with LeftFirst false, so `a` still converts first.

### 13.2 Kernel amendments (exact places)

1. `native.rs`: `Exotic::is_array(&self) -> bool` (default false). `object.rs`: `Rt::is_array` also answers true
   for an exotic whose hook says so. Element storage, `own_keys` and length handling are unchanged.
2. `object.rs`: `ObjectKind::Wrapper(Value)` (String, Number, Boolean or Symbol), traced through `mark_value`. Its
   prototype is the matching intrinsic. A String wrapper has `length` (value = length, non-writable,
   non-enumerable, non-configurable) and indices below its length (value = one code unit as a string,
   non-writable, enumerable, non-configurable). They appear in `own_keys` first (indices), then `length` before the
   other string keys. Redefining them with a different value or flags, and deleting them, are refused.
   Other indices are ordinary properties.
3. `object.rs`: `ObjClass` and `Rt::class_of(obj)`: `Ordinary, Array, Function, Error, Wrapper, Native, Exotic, Cell`.
4. `rt.rs` (bootstrap, one line): `coerce::install` defines `Object.prototype.toString` (ECMA-262 20.1.3.6).
   Tags: Undefined, Null, Array (via `Rt::is_array`), Function (callable), Error, Boolean, Number, String, else
   Object. A string-valued `Symbol.toStringTag` overrides the tag. B1 may replace the builtin.
5. `lib.rs`: `pub mod coerce; pub mod ops;`, re-exports of `ObjClass`, `strict_eq`, `same_value_zero`.

### 13.3 Corpus and oracle

`gen-ops-corpus.mjs` (Node 26, deterministic, same bytes on every run) runs 48 primitive operands (a fixed table, in
the file) through the 20 binary operators for every ordered pair (46080 cases), the 5 unary operators and ToString,
ToNumber and ToInt32 for each operand (384 cases). Encoding: numbers as 16 hex digits of IEEE bits, strings as UTF-16
code units, `null`, `true`/`false`, `{"$":"undef"}`, and `{"throw":"Name"}` for a throw. Symbols and objects are not in
the corpus: `ops_conv.rs` and `ops_ops.rs` check them with values written from the spec and cross-checked against V8.
`ops_corpus.rs` asserts the case counts (46080 binary; 384 unary and conversion) and that every one matches. It has
two negative controls (a doctored expectation and a mutated operand), each of which must be reported.

### 13.4 Deviations and choices

- No BigInt. `to_numeric` returns the Number value as `f64`.
- `Symbol.toPrimitive` IS honoured by `to_primitive` (kernel behaviour since WP-L2a, pinned by `kernel_exotic.rs`).
  WP-L2b asked for it to be ignored; it is kept because removing it would break the finished kernel.
- `Symbol.hasInstance` is not consulted: `instance_of` checks callability, then OrdinaryHasInstance.
- `Object.prototype.toString` is the minimal version in 13.2(4). A Symbol wrapper shows as `[object Object]`
  (V8: `[object Symbol]`) until Symbol.prototype has a `@@toStringTag`. Date and RegExp do not exist yet.
- Wrapper prototypes have no `valueOf`/`toString` yet (String, Number, Boolean, Symbol are B3/B1 builtins). So
  ToPrimitive of a wrapper falls through to Object.prototype.toString: `to_string(Object(1))` is `[object Number]`
  (V8: `"1"`), and `to_number` of `Object("12")` is NaN (V8: 12). Until those methods exist, wrappers must not reach
  user code.
- `Array.prototype.toString` comes with the Array WP, so `to_string` of an array is `[object Array]` until then.
  Functions give `[object Function]` (V8 prints the source text), until Function.prototype.toString exists.
- `to_index` throws RangeError "Invalid index" (not a V8 message: no generic V8 text exists).
- `add` throws RangeError "Invalid string length" above `jsstr::MAX_LEN`. This is implemented but not exercised by a
  test, since a 2^30-unit string is too large to build here.
- Messages follow V8 where a test pins them. For `in`, an object key prints as `#<Object>`; V8 prints
  `[object Object]` for an object with its own `toString`.
- `loose_eq` and the relational operators return a Bool `Value` (`Res<Value>`), as the WP lists them.

## 14. Modules (WP-L2c)

ES module records for the WP-L2c profile (ECMA-262 16.2.1 in outline). Generated code gives one static
`ModuleDef` per module: `specifier` (canonical id), `requests` (ids of every import and re-export, source order,
no duplicates), and three `fn(&Rt, ObjRef) -> Res<()>`: `declare`, `bind`, `evaluate`. Source: `src/module.rs`,
tests `tests/modules_*.rs`, corpus `tests/language/corpora/modules.json` (Node's own loader, written by
`tests/language/gen/gen-module-cases.mjs`).

Storage: the registry lives in the extension slot of the global object (a root). The module subsystem owns that
slot, so nothing else may call `set_ext` on `globalThis`. Each record is a `Native` object that traces its locals,
exports, namespace and thrown value. Records are never freed (the module map is permanent).

Calls (all `impl Rt`):
- `register_module_def(&'static ModuleDef)`: registering the same definition again is a no-op. A different one for
  the same specifier, or one registered after that specifier has a record, panics.
- `register_native_module(spec, Vec<(&str, Value)>) -> ObjRef`: status `Evaluated`; each export is a constant cell.
- `load_module(spec) -> Res<ObjRef>`: link and evaluate the graph rooted at `spec`. Idempotent (see the phases below).
- `module_new(spec) -> ObjRef`: a record with status `New`. It attaches a registered definition if there is one.
- `module_record(spec) -> Option<ObjRef>`, `module_status(m) -> ModuleStatus`, `module_specifier(m) -> String`.
- `module_set_locals(m, Vec<ObjRef>)`: the module-scope cells, exported or not, in index order. Call once, in
  `declare`; a second call panics. `module_local(m, i) -> ObjRef` panics when `i` is out of range or no table is set.
- `module_define_export(m, name, cell)`: a local export. The cell is normally also in the locals table. A duplicate
  name panics.
- `module_add_reexport(m, export, request, import)` and `module_add_star_reexport(m, request)`: the request must be
  listed (else panic); a duplicate export name panics.
- `module_import_cell(from, request, name) -> Res<ObjRef>`: the cell an import binds to. A missing export throws
  SyntaxError "The requested module 'r' does not provide an export named 'x'"; an ambiguous star name throws
  SyntaxError "... contains conflicting star exports for name 'x'".
- `module_request(from, request) -> ObjRef`: the record of a listed request. Use it with `module_namespace` for
  `import * as ns from request`.
- `module_export_cell(m, name) -> Res<Option<ObjRef>>`: follows re-exports and `export *`. `None` for a missing,
  circular or ambiguous name. Never throws.
- `module_namespace(m) -> ObjRef`: created once per module.

Phases of `load_module(spec)`:
0. Walks the requests without side effects. A missing request throws `Error` "Cannot find module 'x'" before any
   code of the graph runs. A linked or evaluated module ends the walk.
1. `declare` for each new module, dependencies first (post-order). Status `Declared`.
2. `bind` for the same modules, same order. Status `Bound`.
3. Evaluates depth first. A module already on the stack is skipped (cycles). A throw marks the module `Errored`
   with that value. Later loads rethrow the same value and never run the module again. A sibling that was not
   reached keeps its state and can be loaded later.
- Link errors: a throw from `declare` or `bind` marks the failing module `Errored` (link error). It also marks every
  declared-but-unbound module of that load and every module that imports one of those. Other bound modules stay
  loadable. Link errors are cached like evaluation errors. Known limit: ECMA-262 would re-link the unbound
  modules later, but their cells cannot be declared twice, so they stay poisoned.
- `load_module` refuses to start a graph while modules are being declared or bound (Error "cannot load module ...
  while modules are being linked").
- Module functions run at call depth >= 1, like other compiled code. `collect()` inside them panics.

Rules for generated code:
- `declare`: create the cells (`let`/`const`/`class` start `Uninit`, `var` starts `Undefined`, function exports
  start `Uninit` until bind), call `module_set_locals` once, then `module_define_export` for each export.
- `bind`: resolve imports with `module_import_cell`, register re-exports, hoist functions (create the closure over
  `module_local`/import cells, then `cell_init` the export). Re-exports may also be registered in `declare`. This
  matters only in a cycle: resolution happens when `module_import_cell` is called, so a re-export registered later
  in bind is invisible to an importer bound earlier.
- `evaluate`: top-level code, after the dependencies. Initialise `let`/`const` cells with `cell_init`.
- `export * as ns from 'x'` is not an API call: in `bind`, `module_define_export(m, "ns", new_cell(Value::Object(
  module_namespace(module_request(m, "x")))))`.

Namespace objects (ECMA-262 10.4.6):
- Null prototype, not extensible, `@@toStringTag` "Module" (not writable, enumerable or configurable).
- The export list is computed on the first access, after linking. A read while modules are being declared or bound
  throws Error "module namespace used before linking completed".
- Own keys: array-index names ascending by number, then the other names in UTF-16 code-unit order, then
  `@@toStringTag`. This is the order Node reports (the corpus shows `2, 10, a`). ECMA-262 asks for code-unit order
  for every name; the corpus is the oracle here, so V8's order is used.
- `[[Get]]`: a TDZ export throws ReferenceError; an unknown name gives undefined. `[[Set]]` is refused for every
  key (strict code throws TypeError). `[[Delete]]` of an export is refused. `[[DefineOwnProperty]]` succeeds only
  when it changes nothing (writable, enumerable, non-configurable, same value). `[[GetOwnProperty]]` reports
  `{writable: true, enumerable: true, configurable: false}`. `seal` succeeds; `freeze` returns false, which
  `Object.freeze` reports as TypeError.
- `default` is never re-exported through `export *`. An ambiguous star name is missing (`module_export_cell`
  is `None`, the namespace omits it).

Cells, locals and namespaces stay alive through the records; `Rt::collect()` may run between load phases only
when the caller drives the phases itself (see `modules_bindings.rs`). Framework-bug panics: a duplicate
definition, export, re-export or locals table; `module_local` out of range or before the table is set; a request
that is not listed (`module_import_cell`, `module_request`, `module_add_*`); `module_new` for a specifier that
already has a record.
- `is_frozen`/`is_sealed` on arrays do not consult `length`. This deviates from ECMA-262 TestIntegrityLevel, which
  also requires `length` to be read-only for a frozen object. V8 14.6 (Node 26): `Object.isFrozen(Object.preventExtensions([]))`
  is true, `Object.isFrozen(Object.preventExtensions([1]))` is false, `Object.isFrozen(Object.seal([1]))` is false and
  `Object.isSealed(Object.seal([1]))` is true. A non-extensible array is frozen or sealed when its elements and named
  properties meet the level. An extensible array is never frozen or sealed, even with a read-only `length`. `freeze`
  still makes `length` read-only, and `seal` leaves it writable (both as V8).

## 15. Builtins: iteration (WP-B0)

Code: `src/builtins/iterator.rs`. Tests: `tests/builtins_iterator.rs` (53 tests). Orders and results of the protocol are
V8's (Node v26.5.1) on mirrored fixtures; the rest follows ECMA-262 7.4. Every iterating builtin family uses this module.

### 15.1 API

| Name | Result | Notes |
|---|---|---|
| `get_iterator(rt, &v)` | `Res<IterRecord>` | GetIterator (sync). Reads `v[Symbol.iterator]` once and `next` once (kept in the record). A nullish, absent or non-callable method is a TypeError "x is not iterable" |
| `iter_step(rt, &mut rec)` | `Res<Option<Value>>` | IteratorStepValue. Any throw marks `rec.done`. A done record returns `None` without calling `next` |
| `iter_close(rt, &rec, throw)` | `Res<()>` | IteratorClose. With `throw = true` it always returns `Ok(())`: the caller returns its own error. It does not check `done` |
| `iterate(rt, &v, f)` | `Res<()>` | for-of driver. `f` returns `Ok(true)` to continue, `Ok(false)` to stop (closes, normal completion). An `Err` closes with a throw completion and propagates |
| `create_iter_result(rt, v, done)` | `Value` | `{ value, done }` with both properties `ALL` |
| `iterator_prototype(rt)` | `ObjRef` | %IteratorPrototype%, registered as `"IteratorPrototype"` |
| `make_iterator_prototype(rt, name, tag)` | `ObjRef` | Family prototype registered as `name`: `[[Prototype]]` %IteratorPrototype%, own `next` (length 0), `[Symbol.toStringTag]: tag` |
| `new_iterator_object(rt, name, payload)` | `ObjRef` | Iterator object of family `name`. Panics if `name` is not registered (a framework bug) |
| `trait NativeIter: NativeObject` | `fn next(&self, rt) -> Res<Option<Value>>` | `None` means exhausted |

### 15.2 Rules

- Order: `next` is read once. Each result's `done` is read before `value`, and `value` is not read when done. A throw from
  `next`, from a non-object result, or from reading `done` or `value` ends the record without calling `return`.
- Close: `return` is read (a getter included) and called on every early close, including a throw completion. On a throw
  completion its result is not checked and its errors are dropped.
- Roots: `IterRecord` values are not GC roots. Host code that keeps a record across `Rt::collect` roots `iterator` and `next`.
- `NativeIter::next` may run user code, so it must not hold a borrow across that call. The wrapper stops calling the state
  after it returns `None` or an error, and the iterator stays done from then on.
- A family's `next` accepts only iterators of that family. Other receivers are a TypeError, as in V8.
- Messages: "x is not iterable"; "Result of the Symbol.iterator method is not an object"; "Iterator result x is not an
  object"; "x is not a function" (a non-callable `return`); "next method called on incompatible receiver x".

### 15.3 Deviations and choices

- %IteratorPrototype% has only `[Symbol.iterator]`. There is no `Iterator` global and no iterator helpers, so those calls
  fail with a TypeError.
- `iter_step` on a done record returns `None` without calling `next`. ECMA-262 has no such guard. It keeps a caller bug
  from resuming a closed iterator.
- Strings are not iterable until B3 installs `String.prototype[Symbol.iterator]`. Numbers, booleans and symbols are not
  iterable (as in V8).

## 16. Builtins: Array (WP-B2)

Code: `src/builtins/array.rs` (constructor, statics, array-like helpers, array iterators), `array/proto.rs` (prototype
methods), `array/sort.rs`. Tests: `tests/language/builtins/array-{statics,methods,callbacks}.json` (generated from V8 by
`tests/language/gen/gen-b2.mjs`), `crates/testkit/tests/replay_b2.rs`, `crates/value/tests/builtins_b2_*.rs`.

Public API: `builtins::array::array_from_iterable(rt, &Value) -> Res<ObjRef>` collects an iterable through
`builtins::iterator::iterate` into a new array (holes are read as undefined). The array iterators are `NativeIter`
objects of `%ArrayIteratorPrototype%` (registered as "ArrayIteratorPrototype"); the iterator helpers are in section 15.
The other helpers (`get_at`, `get_present`, `set_at`, `create_at`, `delete_at`, `len_of`, `key_of`,
`create_array_iterator`) are `pub(crate)`.

Deviations and choices:
- Dense limit: `Array(n)`, a length assignment, and any result or growth above `MAX_DENSE_LEN` (4 194 304) throw RangeError
  "Invalid array length". V8 allows sparse arrays up to 2^32 - 1.
- Results are plain arrays (no Symbol.species); `constructor` is not consulted. `concat` spreads real arrays only
  (IsArray); Symbol.isConcatSpreadable is not consulted (the kernel has no such well-known symbol).
- Array-likes are read index by index (no sparse fast path), so a huge `length` costs time proportional to it.
- sort: below 64 elements the comparator calls are V8's exactly (sort: binary insertion below 8 elements, then run detection and binary insertion; toSorted: run detection at every size). From 64 on, a stable merge sort gives the same result for a consistent comparator, but its
  call order may differ. The comparator is never called with undefined; holes move to the end; the array is written only
  after the sort succeeds (a throwing comparator leaves it unchanged); a length below 2 is not written (V8).
- join: a cyclic receiver joins as "" (V8). flat is bounded by the call depth: a cycle throws RangeError "Maximum call
  stack size exceeded".
- lastIndexOf: an explicit fromIndex, even undefined, counts as present (V8).
- Not installed (manifest X or outside the WP): copyWithin, toLocaleString, Symbol.unscopables.
- `values` and `[Symbol.iterator]` are one function object (name "values").

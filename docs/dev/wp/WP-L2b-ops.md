# WP-L2b — `vn-value` operators, equality and type conversions

Read `docs/dev/AGENT-CONTRACT.md`, `docs/dev/ARCHITECTURE.md` §2, and `crates/value/ABI.md` (the kernel API from WP-L2a).
`export CARGO_TARGET_DIR="$VN_ROOT/target/l2b"`.
**Prerequisites done:** L1 (`JsStr`, number functions) and L2a (kernel: `Rt`, `Value`, objects, calls, errors, GC).

**Owned paths:** new files `crates/value/src/{coerce,ops}.rs`, `crates/value/tests/ops_*.rs`,
`tests/language/gen/gen-ops-corpus.mjs`, `tests/language/corpora/ops.json`, and `crates/value/src/lib.rs` (only `pub mod`
lines and re-exports) and `crates/value/ABI.md` (append an "Operators & conversions" section; do not rewrite other sections).
Do not modify other files. std only, no `unsafe`, no new crates.

## Scope
Implement as **methods on `Rt`** taking `&Value` arguments (pure helpers that need no runtime — `to_boolean`, `strict_eq`, `same_value`,
`same_value_zero` — may be free functions re-exported from the crate root; `Value::to_boolean` and `same_value` already exist in the kernel: reuse) — exact ECMAScript semantics, strict mode, **no BigInt**,
no `Symbol.toPrimitive`/`Symbol.hasInstance` protocols (only default behaviour; if a user object defines them they are
simply ignored — document it) :

**Conversions:** `to_boolean(&Value)->bool`; `to_primitive(&Value, Hint)->Res<Value>` (OrdinaryToPrimitive: `valueOf`/`toString`
order by hint, calling the actual functions, propagating throws, `TypeError` "Cannot convert object to primitive value" when
neither returns a primitive); `to_number`, `to_numeric` (same as to_number without BigInt), `to_string(&Value)->Res<JsStr>`
(numbers via `number_to_string`; `Symbol` → `TypeError`; objects via `to_primitive(string)`; arrays/functions/errors rely on
`toString` methods that later WPs install — for now plain objects get `"[object Object]"` through a minimal
`Object.prototype.toString` you install, arrays join via `Array.prototype.toString` installed by the Array WP: make
`to_string` of an array work through the normal method lookup so it starts working as soon as that method exists),
`to_property_key(&Value)->Res<PropKey>`, `to_object(&Value)->Res<ObjRef>` (primitive wrapper objects: add the
`ObjectKind` variants `PrimitiveWrapper(Value)` via the kernel's extension mechanism if needed — coordinate through a
minimal, documented kernel change in `object.rs` only if unavoidable; list it in your report),
`to_integer_or_infinity`, `to_length`, `to_int32`, `to_uint32`, `to_index`, `typeof_(&Value)->&'static str`.

**Operators** (all `Res<Value>` unless noted): `add` (string concat vs numeric, ToPrimitive default hint), `sub mul div rem pow`,
`neg pos bit_not not_`(`bool`) `inc dec` (return the new numeric value; postfix old-value handling is the compiler's job via
`to_numeric`), `bit_and bit_or bit_xor shl shr ushr`, `lt gt le ge` (IsLessThan with correct LeftFirst evaluation order and
string vs string code-unit comparison), `loose_eq` (`==` full algorithm minus BigInt), `strict_eq(&Value,&Value)->bool` (free fn),
`same_value`, `same_value_zero`, `instance_of(&Value,&Value)->Res<bool>` (callable check, `prototype` chain walk; `TypeError`
if right side not callable), `in_operator(&Value key,&Value obj)->Res<bool>` (`TypeError` for primitives).
Every operator has a fast path for number/number and string/string.

## Kernel amendments you must make (small, in the kernel's own files; keep the diff minimal and list every touched line range in your report)
1. `Exotic` gets `fn is_array(&self) -> bool { false }` (default) and `Rt::is_array` must return true for an exotic whose hook says so (so `Array.isArray(reactiveProxyOfArray)` is
   true like in JS); same for `Rt::own_keys`/length behaviour — nothing else changes. Add a kernel test.
2. A primitive-wrapper object kind for `ToObject`: `ObjectKind::Wrapper(Value)` (String/Number/Boolean/Symbol wrappers) with prototype from the matching intrinsic; String wrappers expose
   `length` and index properties (read-only, enumerable indices) via the ordinary own-property path; `trace` marks the inner value. `Rt::to_object(&Value)`.
3. `Rt::class_of(obj) -> ObjClass { Ordinary, Array, Function, Error, Wrapper, Native, Exotic, Cell }` for diagnostics and the trace encoder.

## Oracle: V8-generated corpus
`tests/language/gen/gen-ops-corpus.mjs` (Node, no deps, deterministic): for ~48 representative primitive operands
(undefined, null, true, false, ±0, ±1, 2, 0.5, -0.5, NaN, ±Infinity, 2^31, 2^32, -2^31, 2^53, MAX_VALUE, MIN_VALUE, ``, `0`,
`1`, ` 1 `, `-1`, `a`, `1e3`, `0x10`, `0b11`, `0o7`, `Infinity`, `-Infinity`, `NaN`, `\n`, `1 2`, `.5`, `5.`, lone-surrogate string,
emoji string, ...) compute for **every ordered pair**: `+ - * / % ** & | ^ << >> >>> < > <= >= == != === !==` (results encoded
with the trace-format number/string/bool/undef/null encodings, exceptions as `{"throw":"TypeError"}`); for each single operand:
unary `- + ~ ! typeof`, ToString, ToNumber, ToInt32 results. Symbols and objects are not in the corpus (cannot be expressed in
JSON): cover them with hand-written Rust tests whose expected values come from the spec: objects with `valueOf`/`toString`
(native functions that record call order in a `Vec`), hint selection (`+` default, `*` number, template/`String()` string), exceptions
thrown from `valueOf`, `Symbol` operand `TypeError`s, `instanceof`/`in` with Error/Array/plain objects, `==` between object and
primitive, `<` evaluation order (`a<b` converts a first; `a>b` is `b<a` with LeftFirst=false).
Rust integration test `crates/value/tests/ops_corpus.rs` loads `ops.json` (`include_str!`, test-only mini JSON reader — copy the
helper from `crates/value/tests/support/` created by WP-L1) and compares every case; print the first 20 mismatches with operands.
Include one negative control (doctored expectation must be reported).

## Acceptance
`source scripts/env.sh; export CARGO_TARGET_DIR=$VN_ROOT/target/l2b; node tests/language/gen/gen-ops-corpus.mjs; cargo test -p vn-value`
— everything passes (earlier suites must still pass); corpus regenerates byte-identically; no warnings.

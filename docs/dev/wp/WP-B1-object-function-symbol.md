# WP-B1 — Object, Function.prototype, Symbol, Boolean, global functions

Common rules: `docs/dev/wp/BUILTINS-COMMON.md`. `export CARGO_TARGET_DIR="$VN_ROOT/target/b1"`.
**Owned paths:** `crates/value/src/builtins/{object,function,symbol,boolean,global}.rs`, `tests/language/gen/gen-b1.mjs`,
`tests/language/builtins/{object-proto,function-proto,symbol,boolean,globals}.json` (the existing `object-statics.json` from WP-O2 is yours to pass; extend it with a *new* file if you need more cases —
do not edit O2's files), `crates/testkit/tests/replay_b1.rs`, `crates/value/tests/builtins_b1_*.rs`.

## Implement (everything else stays absent)
- `Object` (callable and constructible: `Object(v)` = ToObject for non-nullish, new object otherwise) with statics: `keys values entries assign fromEntries freeze isFrozen seal isSealed preventExtensions isExtensible
  hasOwn is getPrototypeOf create(proto[, undefined only — a properties argument other than undefined throws TypeError "unsupported"]) getOwnPropertyNames getOwnPropertySymbols getOwnPropertyDescriptor(s)`
  (descriptor *reading*; objects with `value/writable/enumerable/configurable` or `get/set/enumerable/configurable`), `Object.groupBy(items, cb)` (ES2024). NOT: `defineProperty`, `defineProperties`, `setPrototypeOf`.
- `Object.prototype`: `hasOwnProperty isPrototypeOf propertyIsEnumerable toString` (`[object Array|Function|Error|Boolean|Number|String|Null|Undefined|Symbol|Object]`, honours `Symbol.toStringTag`)
  `valueOf toLocaleString`; `constructor`. No `__proto__`, no `__defineGetter__`-family.
- `Function.prototype`: `call apply bind toString` (`"function name() { [native code] }"` shape; compiled functions return the same shape) and `[Symbol.hasInstance]` default; bound functions have name `"bound f"` and
  correct `length`; `Function` constructor itself is absent.
- `Symbol` (function, not a constructor: `new Symbol()` → TypeError), `Symbol.for keyFor iterator asyncIterator toPrimitive toStringTag hasInstance`, `Symbol.prototype.toString valueOf description` (+ `[Symbol.toPrimitive]`, `[Symbol.toStringTag]`).
- `Boolean` (function + constructor wrapper using the kernel's wrapper objects), `Boolean.prototype.toString valueOf`.
- Globals: `globalThis`, `undefined`, `NaN`, `Infinity` (non-writable etc.), `parseInt`, `parseFloat`, `isNaN`, `isFinite`.
Additionally verify (unit tests) that `instanceof`/`typeof`/`String(sym)`/template literal behaviour with these objects is right through the operators of WP-L2b.

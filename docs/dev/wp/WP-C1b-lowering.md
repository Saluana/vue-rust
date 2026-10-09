# WP-C1b — JS/TS → Rust lowering and emission (core language), module ABI, differential harness

Read `docs/dev/AGENT-CONTRACT.md`, `docs/dev/ARCHITECTURE.md`, `docs/dev/trace-format.md`, `crates/value/ABI.md` (kernel; operators/coercions and the module section are
appended by WP-L2b/L2c — read the final text), `docs/dev/builtin-corpus.md` (context only). WP id `c1b`; `export CARGO_TARGET_DIR="$VN_ROOT/target/c1b"`.
**Prerequisites done:** C1a (parse/profile/scope analysis in `compiler/{frontend,analysis,diagnostics}`), F1 (SFC), L1/L2a/L2b/L2c (value runtime), T1 (`crates/testkit`: `scenario_main`, `run_scenario`), O1 (oracle).
**Owned paths:** `compiler/hir/**`, `compiler/lower/**`, `compiler/emit/**`, `compiler/index.mjs`, `compiler/ABI.md`, `tooling/harness/**`,
`tests/compiler/lower/**`, `tests/language/lowering/**` (your own scenarios), `tests/language/lowering-golden/**`, `tests/language/status.json`.
Do not edit `compiler/compile.mjs`/`cli.mjs` (legacy prototype) or the other compiler directories (read-only: call their exports).

## Why
This is the heart of "JavaScript compiled through Rust, without an interpreter": a JS/TS module (user code, or the JS the Vue SFC compiler emitted) becomes a Rust source file
that calls `vn-value`. No `eval`, no embedded engine.

## Pipeline (`compiler/index.mjs`)
`compileModule(source, { filename, lang: 'js'|'ts', specifier, origin: 'user'|'compiler', target: 'native'|'web' }) → { rust, diagnostics, requests, exports, imports }`
1. `parseModule` → profile check (`checkProfile`, skip for `origin:'compiler'` except the import names rule) → `analyzeScopes` (all from C1a). Any error diagnostic stops compilation (no Rust is produced).
2. **HIR** (`compiler/hir/`): a small, explicit, serialisable tree (JSON-able, versioned `hirVersion: 1`) where every identifier is resolved to `Local(slot) | Cell(slot) | ModuleLocal(idx) | Import(...) | Global(name)`,
   every function has an explicit capture list, `this`/arrow-`this` are explicit, all syntax sugar is gone: destructuring, default params, rest/spread, template literals, optional chaining,
   `??`, logical/compound assignment, update expressions, `for…of`/`for…in`, `switch`, labels, `try/catch/finally`, object/array literal forms, getters/setters/methods, shorthand, computed keys,
   exponent operator. Keep source locations (`start,end` offsets) on every node for diagnostics and later source maps. Write `compiler/hir/README.md` with the node catalogue.
3. **Emit** (`compiler/emit/`): HIR → one Rust source string per module.

## Emitted-code conventions (normative; document them in `compiler/ABI.md`)
- `use vn_value::*;` — every JS function becomes `fn f<N>_<mangled>(rt: &Rt, call: &Call<'_>) -> Res<Value>`; arguments via `call.arg(i)`; `this` via `call.this`.
- Locals not captured by any closure are Rust `let mut v_<name>: Value`. A binding captured by an inner function is a heap cell (`new_cell`), accessed with `cell_get`/`cell_set` (or the `_checked` forms when the scope
  analysis says `needsTdzCheck`); `let`/`const` cells start `Value::Uninit`, `var` starts `Undefined`. Per-iteration `let` in `for(;;)` heads: create a fresh cell per iteration copying the value (spec CreatePerIterationEnvironment).
- Functions are created with `rt.new_function(f, FnInfo::new(name, length, kind), &[captured cells…])`; inside the function, captured cells are read once at entry from `rt.captures(call.callee)`.
  `name` inference for anonymous functions in `const f = () => {}` / `{ f: function(){} }`, `length` = params before the first default/rest. Arrow functions capture `this` through a cell created when they are
  created inside a function that uses `this` (module-level `this` is `undefined`).
- Function declarations are hoisted (created at scope entry, initialising their binding before other statements); block-level functions are block scoped (strict mode). `var` is hoisted to function/module scope.
- Calls `a.b(c)`: evaluate `a`, read `b` (null/undefined base → TypeError at the read), evaluate args left to right, then `rt.call(&f, &this, &args)`. Optional chains short-circuit the **whole** chain. Compound assignment/update on a member evaluates
  base and key exactly once. Evaluation order must match the spec (left-to-right operands; for `a[i] = v` the key's ToPropertyKey happens after the RHS per ES2024? — follow V8; verify with the oracle and add a scenario).
- Statements: `if/for/while/do-while` → Rust loops with labels; `break`/`continue` with labels; `switch` (lexical scope for the whole block, fallthrough, `default` anywhere) → labelled block + case index search then sequential bodies;
  `try/catch/finally` → the body runs as a closure `|| -> Res<Flow>` where `enum Flow { Normal, Return(Value), Break(u32), Continue(u32) }` is emitted once per module so `return/break/continue` crossing `finally` blocks work and a
  `finally` that completes abruptly overrides the pending completion; catch binds the thrown value (`Throw.0`). A Rust `?` must never skip a pending `finally`.
- Loop back-edges and function entry call `rt.tick()?`.
- `for…of` uses the iteration protocol through runtime helpers you add to the **generated prelude** (emitted once per module, small Rust functions that call kernel API: `get_iterator`, `iter_step`, `iter_close`) —
  array and string fast paths allowed because user code cannot patch built-in prototypes; `break`/`return`/throw closes the iterator (`return()` call). `for…in`: snapshot enumerable string keys (own, then prototype chain,
  skipping shadowed and keys deleted before being visited) like V8. Spread (`...x` in calls, arrays) and array destructuring use the same iteration helpers; object spread/rest uses CopyDataProperties (own enumerable string and symbol keys).
- Object literals: `new_object` + `define_data(..., Flags::ALL)`; numeric/string keys canonicalised (`{1:x}`, `{0x10:y}`, `{1.5:z}`, `{'01':w}`); getters/setters via `define_accessor` (enumerable, configurable); duplicate keys replace; methods are non-constructor functions.
  `__proto__` literal keys are rejected by the profile.
- Strings: literal pool as `&'static [u16]` constants → `JsStr`; lone-surrogate escapes (`'\uD83D'`) keep the code unit. Numbers: all JS numeric literal forms (`0x`, `0o`, `0b`, exponent, separators). Template literals use `to_string` (Symbol → TypeError).
- Identifier mangling: `v_<ident>` for ASCII `[A-Za-z0-9_]`, other characters `_u<hex>_`; Rust keywords can never collide because of the prefix; functions `f<N>_<mangled>` with a module-unique counter; deterministic output (no hash maps iterated in insertion-dependent random order, no absolute paths, no timestamps).
- Globals: unresolved identifiers read `globalThis` (`rt.get_str(global, name)`; missing → ReferenceError "x is not defined"); `typeof undeclared` does not throw. Assignment to an undeclared name is a ReferenceError (strict).
- `new X(...)` only for built-in constructors resolved through the global object (`rt.construct`); the profile already rejects user constructors.
- **Module ABI** (matches WP-L2c): the module compiles to `pub static MODULE: ModuleDef = ModuleDef { specifier, requests, declare, bind, evaluate }` plus `pub fn def() -> &'static ModuleDef`. `declare` creates cells for **all** module-scope bindings, registers them with `module_set_locals`, defines the export cells and registers **re-exports** (`module_add_reexport`/`module_add_star_reexport` — in `declare`, not `bind`, so a cyclic importer that binds earlier still sees them; WP-L2c finding); `bind` resolves imports with `module_import_cell`, handles re-exports, creates hoisted functions and `cell_init`s them; `evaluate` runs top-level statements. Imports are read through the exporter's cell (live bindings). `import * as ns` uses `module_namespace`. `import x from 'vue'` for native modules works the same
  way. Default export of an expression gets a hidden local `*default*`. Top-level `await` is rejected by the profile.
- TypeScript: annotations, `as`, `satisfies`, non-null `!`, type-only imports/exports, `declare`, generics and overload signatures are erased (use the nodes' ranges; the profile checker already rejected runtime-producing TS).
- Unsupported-but-reached constructs in the emitter are internal errors `VN9xxx` (never silently skipped) with the node range.

## Scope of this WP (tier 1 and 2 of the language scenarios)
Everything above for **synchronous** code: expressions, functions, closures, control flow, destructuring, spread/rest, template literals, optional chaining, exceptions, iteration, labels/switch, getters/setters, symbols as keys, ESM single-module + `import … from <native module>`.
**Not in this WP:** `async`/`await`/promises (WP-C1c), multi-module graphs beyond what `ModuleDef` needs (C1c), standard-library builtins (other WPs).

## Harness (`tooling/harness/`)
- `build-scenarios.mjs --out <dir> <files…>`: compile each scenario to Rust (`origin:'user'`), write a throw-away cargo crate under `target/scenarios/<id>/` (path dependency on `crates/value` and `crates/testkit`; table of scenarios in `main.rs`
  calling `vn_testkit::scenario_main`), build once in a private target dir, return the binary path. Deterministic file names `s_<mangled>.rs`.
- `run-language.mjs [--tier N] [--filter text] [--dir tests/language/cases] [--report]`: builds, runs each scenario binary (one process each, 30 s timeout), compares with the golden using `tooling/oracle/compare.mjs`, prints a table
  `pass | FAIL | blocked | compile-error` and the first diff path. Exit non-zero if any scenario marked `pass` in the ledger fails, **or any scenario marked `blocked` passes** (ratchet: flip it to `pass`), or an unlisted scenario fails.
- `tests/language/status.json`: `{ "<scenario>": "pass" | { "blocked": "reason" } }` for the existing O1 scenarios; you populate it by running. Typical blocked reasons: "missing builtin Array.prototype.map". Your own scenarios live in `tests/language/lowering/**`
  with goldens produced by Node (`tests/language/lowering-golden/`), use **no builtins other than what the kernel provides** (Object.prototype basics via kernel, Error constructors, operators, objects/arrays literals, closures, control flow, destructuring, iteration over arrays and strings, getters/setters, symbols created by the
  `Symbol()` function ONLY IF it exists — otherwise leave those cases to the ledger), and must all pass: write **≥ 150** of them, one construct family per file, covering every bullet above plus evaluation-order and TDZ/closure/loop-capture subtleties and every node type the profile accepts.

## Tests
- Unit tests (`tests/compiler/lower/*.test.mjs`): HIR snapshots for small inputs (hand-checked), name mangling, literal pool, determinism (compile twice + under another cwd → identical), error paths (VN9xxx), TS erasure.
- The differential run above is the main test. Add a negative control (a deliberately wrong golden must be reported as FAIL).
- `compiler/ABI.md`: conventions + how to run + how to read the scoreboard.

## Acceptance
`node tooling/harness/run-language.mjs --dir tests/language/lowering` → all pass; `node tooling/harness/run-language.mjs` (O1 scenarios) → no unexpected results per `status.json`, with the counts printed; `node --test "tests/compiler/lower/*.test.mjs"` green.
In your report: the scoreboard counts, the list of blocked scenarios grouped by reason, and any V8-specific behaviours you matched or deliberately did not.

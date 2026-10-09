# compiler/ABI.md — JavaScript/TypeScript to Rust (WP-C1b)

`compileModule(source, { filename, lang, specifier, origin, target, resolve })` (compiler/index.mjs) returns
`{ rust, hir, diagnostics, requests }`. `rust` is null when an error diagnostic exists. Stages: parse and profile check
(compiler/frontend), scope analysis (compiler/analysis), HIR lowering (compiler/hir, see its README), emission
(compiler/emit).

## Emitted module
- Prelude: `use vn_value::*`, `enum Flow { Normal, Return(Value), Break(u32), Continue(u32) }` and the helpers `vn_*`
  (each states the spec step it performs).
- `pub static MODULE: ModuleDef` and `pub fn def()`. Phases `vn_declare` (module cells, exports, re-exports),
  `vn_bind` (import cells, hoisted functions, `export * as ns`), `vn_evaluate` (top-level statements).
- One Rust function per JavaScript function: `fn f<id>_<name>(rt: &Rt, call: &Call<'_>) -> Res<Value>`.

## Names
Local `v<bindingId>_<ident>` (the id keeps shadowed names apart), captured cell `c<id>` / `th<fn>` (this), captured
cell alias `k<index>` (from `rt.captures(call.callee)`), module cell `m<id>`, import cell `i<id>`, string constants
`S<n>`. Deviation from the WP text: locals carry the binding id, because Rust `let` shadowing would break
`let x; { x; let x }`.

## Conventions
- Every operand is evaluated into a temporary in source order; member assignment converts a computed key after the
  right side (V8); compound and update convert it twice (V8).
- Loops: `'l<lid>: loop` with `'c<lid>: { body }` (continue = break of the block). Try/catch/finally bodies and for-of
  bodies are closures returning `Res<Flow>`; a jump out of them returns a Flow value that the code after the closure
  dispatches. for-of closes its iterator on break, return, outer jumps and throws.
- `rt.tick()` on function entry and loop back-edges.
- Unsupported constructs that pass the profile produce VN9001 with the node span (never skipped).

## Running
- Harness: `node tooling/harness/run-language.mjs [--dir tests/language/lowering] [--filter text] [--write-status]`
  compiles every scenario, builds one crate in `target/scenarios/language` with `CARGO_TARGET_DIR=target/c1b`, runs each
  scenario binary and compares with the golden (`tests/language/golden/*.js.json` for the O1 cases,
  `tests/language/lowering-golden/` for the lowering corpus). Goldens: `node tooling/harness/write-goldens.mjs`.
- Unit tests: `node --test "tests/compiler/lower/*.test.mjs"`.

## Reading the scoreboard
`pass`: compiled, run and equal to the Node golden. `FAIL`: mismatch, panic or run error. `compile-error`: the compiler
stopped (diagnostic printed). For cases, `tests/language/status.json` records `pass` or `{ "blocked": reason }`; the
run exits 1 when a pass-marked case fails, a blocked case passes (flip it), or an unlisted case fails.

## WP-C1c: async functions, projects, native modules, TypeScript erasure

### Async functions (compiler/emit/index.mjs `emitAsyncFunction`, buf.mjs, prelude.mjs `ASYNC_PRELUDE`)
- An async function (declaration, expression, arrow, method) is two Rust items. The entry `fN(rt, call)` creates the frame
  `fr = new_frame(rt, S)`, stores `this` (slot 0), the callee (slot 1), the captured cells (slots 2..), the raw arguments
  (one slot per parameter, a rest parameter holds its array) and returns `async_start(rt, fr, Box::pin(fN_body(rt.clone(), fr)))`.
  The body `async fn fN_body(rt_: Rt, fr: ObjRef) -> Res<Value>` binds the parameters from their slots, runs `rt.tick()`
  and the statements. Parameter defaults run in the body, so a throw there rejects the result promise.
- `await e` is `frame_set(rt, fr, S, await_value(rt, e).await?)`; its value is then read from slot S.
- GC invariant (tests/compiler/lower/async.test.mjs): no Value, object or key temporary and no binding is a Rust local in an
  async body. Value temporaries, binding locals, cells (as Value::Object) and captured cells live in frame slots; object
  literals and rest objects are object slots; computed and static property keys are key Values read back with vn_key;
  argument lists, array literals and rest elements are accumulators (vn_acc_*: a JavaScript array, Value::Hole leaves a
  hole); for-of and array patterns keep their iterator record in three slots (vn_it_*); for-in keeps its key array and
  index in slots; switch keeps the discriminant and matched index in slots; a try with finally keeps the completion in
  three slots (vn_cmpl_store/load) while the finally block runs. Rust Strings and JsStr buffers hold no GC reference and
  may stay Rust locals.
- try/catch/finally, loops, for-of and array patterns use async blocks (`async { ... }.await`) in place of closures. Jumps
  leave them as Flow values, exactly as in the synchronous code. Synchronous functions emit the same text as before
  (verified: every WP-C1b scenario's Rust is byte-identical).
- Runtime: WP-B5 (crates/value/src/builtins/promise/async_rt.rs). A generated crate has `src/vn_async.rs`: a re-export of
  `vn_value::builtins::promise::{async_start, await_value, frame_get, frame_set, new_frame}` when `pub fn async_start`
  exists in crates/value, otherwise a stand-in (tooling/harness/build.mjs `asyncRuntimeRs`). Async modules import
  `crate::vn_async` (ASYNC_PRELUDE), so a product crate must provide that module.
- Rejected: `for await` (VN2010), async generators (VN2008/VN2009), `await` in parameter defaults (VN1000, a parse error),
  top-level await (VN2022). Dynamic `import()` is still unlowered (VN9001).

### Projects (compiler/project/)
- `compileProject({ entry, files | resolver, target, sfcRootDir })` -> `{ entryId, modules, natives, diagnostics, ok }`.
  A module is `{ id, file, lang ('js'|'ts'|'vue'), codeLang, kind, origin, rust (null on error), requests, styles, diagnostics }`.
- Ids are project-root-relative and `/`-prefixed (`/src/app.js`); `..` escaping the root is refused. `requests` are the
  canonical ids of the module's imports and re-exports (native modules by name).
- Specifiers: a relative path names a module by its exact path (no extension search, as in Node ESM); `vue` and
  `@vue-native/platform` are native (natives.mjs) and are recorded in `natives`, not compiled. Anything else is an error:
  package imports (closed world), missing modules, absolute specifiers, unknown file types. The vue policy of compileSfc
  (VN6201 excluded export, VN6202 default or namespace import, VN6203 subpath, VN6204 Node built-in) applies to user code.
- Errors for module graphs use VN9001 (the lowering's unsupported code); the registry has no user-facing code for them yet.
- `.vue` modules go through compileSfc (external src files are read from `sfcRootDir`); the generated code is lowered as
  `origin: 'compiler'` (its runtime helper imports are not checked again).
- TypeScript erasure: type-only imports and exports are removed; in a `.ts` module an import specifier with no value
  reference is removed (the analysis records value references only), and an import whose specifiers are all removed
  has no request.
- Module table (compiler/project/table.mjs): one file `m<k>.rs` per compiled module (sorted by id); `modules.rs` has
  `entry()` and `defs()` (`static DEFS: &[&ModuleDef]`). A project directory is `mod.rs` + `natives.rs` + `modules.rs`.

### Harness (tooling/harness/)
- `run-language.mjs` honours CARGO_TARGET_DIR (each WP its own target directory). `run-projects.mjs` runs
  tests/language/projects/<name>/ (scenario.js, helper files, native.json, expected.json): natives come from native.json
  (tooling/harness/natives.mjs: identity, pair, box, throws, and constants), the Node oracle runs the same files with the
  natives answered by natives-hooks.mjs. `--write-expected` regenerates the oracle documents (a .vue project keeps its
  hand-written expected.json). `check-sfc-lowering.mjs` compiles every tests/sfc/fixtures/ok-*.vue and runs cargo check.
- Lowering corpus: tests/language/lowering/async-*.js (10 scenarios) with goldens from the Node oracle.

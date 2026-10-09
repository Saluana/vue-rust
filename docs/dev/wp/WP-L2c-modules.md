# WP-L2c — `vn-value` module records, live bindings, native modules, module loader

Read `docs/dev/AGENT-CONTRACT.md`, `docs/dev/ARCHITECTURE.md` §2 and `crates/value/ABI.md`. `export CARGO_TARGET_DIR="$VN_ROOT/target/l2c"`.
**Prerequisite:** L2a done. (L2b runs concurrently in other files; do not edit its files.)
**Owned paths:** new `crates/value/src/module.rs`, `crates/value/tests/modules_*.rs`; `crates/value/src/lib.rs` (add `pub mod module;` + re-exports only), and
a new section "Modules" appended to `crates/value/ABI.md` (do not rewrite other sections). std only, no `unsafe`.

## Why
Every compiled `.vue`/JS file is an ES module. Generated Rust supplies three plain functions per module; the runtime implements the ECMAScript
module algorithm (declare → bind → evaluate, cycles, live bindings, error caching, namespace objects). Native modules (`'vue'`, platform services) are
registered by Rust code as ready-made modules.

## API (names normative)
```rust
pub struct ModuleDef {
    pub specifier: &'static str,                 // canonical id, e.g. "/src/App.vue" or "vue"
    pub requests: &'static [&'static str],       // module specifiers in source order (as resolved ids), duplicates removed
    pub declare:  fn(&Rt, ObjRef /*module*/) -> Res<()>,   // phase 1: create export cells (value Uninit for let/const/class, Undefined for var, function exports initialised later)
    pub bind:     fn(&Rt, ObjRef) -> Res<()>,              // phase 2: resolve imports to the exporters' cells (rt.module_import_cell…), re-exports, hoist function declarations
    pub evaluate: fn(&Rt, ObjRef) -> Res<()>,              // phase 3: top-level code (synchronous; no top-level await in this profile)
}
impl Rt {
  pub fn register_module_def(&self, def: &'static ModuleDef);                       // by specifier
  pub fn load_module(&self, specifier: &str) -> Res<ObjRef>;                        // link + evaluate the graph rooted here (idempotent), returns the module record
  pub fn module_new(&self, specifier: &str) -> ObjRef;                              // record object (Native payload, traced)
  pub fn module_define_export(&self, m: ObjRef, name: &str, cell: ObjRef);          // panics if defined twice (framework bug)
  pub fn module_export_cell(&self, m: ObjRef, name: &str) -> Res<Option<ObjRef>>;   // follows re-exports (`export {a as b} from`, `export * from`), detects ambiguity/cycles
  pub fn module_import_cell(&self, from: ObjRef, request: &str, name: &str) -> Res<ObjRef>;  // SyntaxError-like Throw if missing export ("does not provide an export named 'x'")
  pub fn module_add_reexport(&self, m: ObjRef, export_name: &str, request: &str, import_name: &str);   // `export { import_name as export_name } from request`
  pub fn module_add_star_reexport(&self, m: ObjRef, request: &str);                  // `export * from request`
  pub fn module_namespace(&self, m: ObjRef) -> ObjRef;                               // exotic, cached
  pub fn register_native_module(&self, specifier: &str, exports: Vec<(&str, Value)>) -> ObjRef;   // status Evaluated; values in cells (constant bindings)
  pub fn module_status(&self, m: ObjRef) -> ModuleStatus;     // New | Declared | Bound | Evaluating | Evaluated | Errored
}
```
Semantics to implement exactly:
- `load_module` runs: (1) DFS over `requests` creating records for all reachable defs and calling `declare` once each; (2) a second pass calling `bind` once each (after
  all `declare`s so cyclic imports can find cells); (3) evaluation in post-order DFS, each module once; a module in `Evaluating` state encountered again (cycle) is skipped (its
  cells may still be `Uninit` — reading them throws the TDZ ReferenceError via `cell_get_checked`, function-declaration exports are already initialised by `bind`). If a module's
  `evaluate` throws, the module becomes `Errored(throw)` and the **same throw value** is rethrown for every later import/load of it and of its dependents (never re-run).
  A missing specifier (no def, no native module) is a `Throw(Error)` "Cannot find module 'x'". Unknown request in `requests` is detected before any code runs.
- Export lookup rules incl. `export *` (does not re-export `default`, ambiguity between two star sources with different cells → treated as missing, ES spec `ResolveExport`), re-export cycles
  terminate.
- Namespace object (`import * as ns`): exotic, null-prototype, non-extensible, keys sorted by UTF-16 code-unit order, data properties writable:true enumerable:true configurable:false
  read live from the cell (TDZ → ReferenceError), `[Symbol.toStringTag]: "Module"` (non-enumerable, non-writable, non-configurable), `set`/`delete`/`define` refused (`false` → strict TypeError),
  `has` true for exports.
- All records, cells and thrown values are GC-visible (module registry is a root provider; test with `rt.collect()` between phases).

## Tests (`crates/value/tests/modules_*.rs`, ≥ 40, hand-written `ModuleDef`s simulating generated code; expected behaviour taken from the spec and verified against Node ESM where practical via
a Node script `tests/language/gen/gen-module-cases.mjs` that writes `.mjs` files to a temp dir and records import/evaluation order and namespace contents; commit its JSON output as
`tests/language/corpora/modules.json` and make Rust tests reproduce the same orders with equivalent `ModuleDef`s):
single module; two-module chain order; diamond; cycle with function export used early (works) and `let` export used early (TDZ ReferenceError); re-export rename; `export *` with
ambiguity; namespace object contents/ordering/TDZ/immutability; error caching and rethrow identity; missing module; missing export; native module import; module evaluated once;
GC between phases; negative control.

## Acceptance
`cargo test -p vn-value` all green (earlier suites included), no warnings, corpus generator deterministic.

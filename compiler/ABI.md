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

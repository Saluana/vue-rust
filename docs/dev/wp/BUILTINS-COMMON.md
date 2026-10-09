# Builtins — common rules (every WP-B* spec refers to this file)

Read `docs/dev/AGENT-CONTRACT.md`, `docs/dev/ARCHITECTURE.md` §2, `crates/value/ABI.md` (kernel + operators + modules sections),
`docs/dev/trace-format.md`, `docs/dev/builtin-corpus.md`, `tooling/oracle/builtin-corpus.mjs` (+ `tests/language/builtins/README.md`).

## Where code goes
- One file per family under `crates/value/src/builtins/` (the maintainer has created `builtins/mod.rs` declaring every family and `install_all(rt)`, called at the end of
  `Rt::new()`; each family exposes `pub fn install(rt: &Rt)`). You own only the files your WP lists; do not edit `mod.rs` or `rt.rs` — request changes.
- Builtins are `CodeFn`s (`fn(&Rt, &Call<'_>) -> Res<Value>`) registered with the kernel helpers (`define_native_method`, `new_native_constructor`, `install_global`, …).
  Property attributes must match JS (methods: writable, non-enumerable, configurable = `Flags::HIDDEN`; constants: all false; `length`/`name` per spec; constructors get `prototype`
  (non-writable, non-enumerable, non-configurable) and `prototype.constructor`).
- Spec semantics (ECMAScript 2025) with **V8 as the oracle**: argument coercion order, holes, `this` coercion (`TypeError` for null/undefined `this`), `length` as `ToLength`,
  array-likes where the spec allows, observable getter call order. Where V8 and the spec differ, match V8 and say so in a comment.
- Only the methods listed in your WP exist; everything else must be **absent** (so a call fails with a TypeError and the compiler/manifest can say "unsupported").
- No `unsafe`, no new crates. JS callbacks are invoked with `rt.call`; never hold a RefCell borrow or kernel lock across a callback; values live only in `Value`s/`ObjRef`s reachable from
  roots (see ABI "Roots, collection and rules" — builtins run at call depth ≥ 1 so Rust locals are safe during the call).
- Engine-generated error **messages** need not equal V8's; error **types** must.

## Tests
1. Corpus replay (the main evidence): add your suites with the Node library (`tooling/oracle/builtin-corpus.mjs`, `defineSuite/writeSuite`) in a new generator file
   `tests/language/gen/gen-<your-family>.mjs` writing `tests/language/builtins/<suite>.json`; Rust side: a test file `crates/testkit/tests/replay_<family>.rs` calling
   `vn_testkit::replay_suite(include_str!(..), &ReplayOptions::default())` and asserting **zero failures and zero missing targets** for your suites (and printing failures). The corpus must
   exercise: normal cases, every edge listed in your WP, holes/undefined/null/NaN/-0, coercions of arguments from strings/booleans/objects, callbacks that throw, mutation during iteration where
   relevant, aliasing (`this` passed as an argument), huge/negative/fractional indexes, and exceptions (TypeError/RangeError). ≥ 250 cases per WP unless the WP says otherwise.
2. Rust unit tests for anything the corpus format cannot carry (non-extensible `this`, symbols as keys, getters, `new.target`, GC safety: run `rt.collect()` between calls and keep results alive).
3. Negative control: demonstrate that corrupting one expectation in a copy of a suite is reported by the replay.
4. Everything must also keep `cargo test -p vn-value` and `cargo test -p vn-testkit` green.

## Acceptance
Your replay tests green with `missing == 0`, `cargo build` without warnings, generators deterministic (regenerate twice → identical bytes), report lists the exact methods implemented.

## Working in a private mirror (REQUIRED — several builtin agents share the `vn-value` crate)
A syntax error in one agent's file would break everybody's `cargo` run. So you never edit the shared tree while it may not compile:
```sh
cd /Users/brendon/Documents/vue-rust-lab && source scripts/env.sh
M=$(scripts/mirror.sh init <wp-id> crates/value/src/builtins/<yourfile>.rs tests/language/gen/gen-<x>.mjs tests/language/builtins/<yours>.json crates/testkit/tests/replay_<x>.rs …all owned paths…)
cd "$M" && source scripts/env.sh && export CARGO_TARGET_DIR="$VN_ROOT/target/<wp-id>"   # VN_ROOT is now the mirror; fine
# edit, build, test INSIDE $M only.
scripts/mirror.sh pull <wp-id>    # (run from the repo root) refresh others' finished work into the mirror, keeps your owned paths
scripts/mirror.sh push <wp-id>    # (run from the repo root) copy your owned paths back, ONLY when they compile and your tests pass
```
Push early and often once your file compiles (even partially implemented, with the unimplemented methods simply absent), so others and the maintainer can see progress.
Node generators that write corpora run in the mirror too (`node tests/language/gen/gen-<x>.mjs` writes into the mirror's `tests/language/builtins/`); `push` copies them.

### Shared-file rule (learned the hard way)
`scripts/mirror.sh push` copies WHOLE files. Never list a file in your mirror's owned paths unless you are its only editor: two agents pushing the same file silently overwrite each other (this erased the maintainer's CALLBACK_ARITY edit and almost erased `exports.rs`). For shared files (`crates/runtime/src/{exports,lib}.rs`, `crates/reactivity/src/lib.rs`, `tooling/oracle/*`, `crates/testkit/src/*`) make your small change directly in the REPO file with the Edit tool after reading its current content, then rebuild in the repo. Always back up your owned files before `mirror.sh pull`.

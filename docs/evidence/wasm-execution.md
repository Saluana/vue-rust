# Language scenarios on wasm32 — 2026-10-10

Task 4.2 (planning/vue-native-framework/tasks.md): the compiled language scenarios now execute as
wasm32-unknown-unknown. Each scenario's document is compared with the same Node golden as the native run, and it is
byte-identical to the native binary's output. Vue and UI modes are out of scope and were not run on wasm.

## What was built

- `crates/testkit/src/wasm.rs` (new): the wasm entry. Safe code over a table of `(name, entry)` and one output buffer:
  `scenario_count`, `scenario_name`, `run_scenario`, `run_scenario_with_gc_threshold`, `out_ptr`. The crate keeps
  `#![forbid(unsafe_code)]`. The module is not behind a cfg or feature (the ticket suggested one): it has no
  target-specific code, so the native test can run it.
- `crates/testkit/src/runner.rs`: `run_scenario_on_current_thread_with_setup` (with a threshold variant), the isolated
  runner's document without a thread, since wasm32-unknown-unknown has no threads. The native binary reads
  `VN_GC_THRESHOLD` (see the GC section); without it the native runner is unchanged.
- `tooling/harness/build.mjs`: `writeWasmCrate` writes a cdylib (`target/scenarios/language-wasm`) from the same
  generated scenario modules, with five `#[unsafe(no_mangle)]` exports that forward to `vn_testkit::wasm`. The table
  code is shared with `writeCrate` (`scenarioPieces`, `registerAndEntry`). The native crate is byte-identical to the
  output before the refactor (checked for the 166-module corpus). Also added: `buildWasmCrate` (with
  `WASM_STACK_BYTES`) and `wasmSkipReason`.
- `tooling/harness/run-language-wasm.mjs` (new): compiles each scenario as `run-language.mjs` does, builds the cdylib,
  checks the export table and that the module has no imports, and runs each scenario in a fresh WebAssembly instance.
  A trap fails only that scenario. Documents are compared with the goldens by `tooling/oracle/compare.mjs`. The
  summary line has the run-language format. Options: `--dir`, `--golden`, `--filter`, `--gc-threshold N`, `--verbose`.
  `run-language.mjs` is unchanged.
- `tests/language/wasm.test.mjs` (new): five tests. The skip-reason check, the five-scenario subset against goldens,
  the same subset under GC stress, the negative controls, and deep recursion (expected values from the Node oracle at
  test time).
- `crates/testkit/tests/wasm_entry.rs` (new): six native tests that the wasm entry produces the runner's documents,
  including a panic document, a wrong-expectation control, and the threshold checks.

## Commands and results

| Command | Result | Log |
|---|---|---|
| `node tooling/harness/run-language.mjs` | 79/79 pass (native baseline) | [native-default.txt](wasm-execution/native-default.txt) |
| `node tooling/harness/run-language.mjs --dir tests/language/lowering` | 166/166 pass (native baseline) | [native-lowering.txt](wasm-execution/native-lowering.txt) |
| `node tooling/harness/run-language-wasm.mjs --verbose` | 79/79 pass on wasm32; 79 `pass` lines | [wasm-default.txt](wasm-execution/wasm-default.txt) |
| `node tooling/harness/run-language-wasm.mjs --dir tests/language/lowering --verbose` | 166/166 pass on wasm32; 166 `pass` lines | [wasm-lowering.txt](wasm-execution/wasm-lowering.txt) |
| `VN_GC_THRESHOLD=1 node tooling/harness/run-language.mjs` | 79/79 pass (native, a collection at every safepoint) | [native-gc1-default.txt](wasm-execution/native-gc1-default.txt) |
| `VN_GC_THRESHOLD=1 node tooling/harness/run-language.mjs --dir tests/language/lowering` | 166/166 pass (native, stress) | [native-gc1-lowering.txt](wasm-execution/native-gc1-lowering.txt) |
| `node tooling/harness/run-language-wasm.mjs --gc-threshold 1 --verbose` | 79/79 pass on wasm32 (stress) | [wasm-gc1-default.txt](wasm-execution/wasm-gc1-default.txt) |
| `node tooling/harness/run-language-wasm.mjs --dir tests/language/lowering --gc-threshold 1 --verbose` | 166/166 pass on wasm32 (stress) | [wasm-gc1-lowering.txt](wasm-execution/wasm-gc1-lowering.txt) |
| `node --test --test-reporter=spec tests/language/wasm.test.mjs` | 5 pass, 0 fail, 0 skipped (2.7 s) | [node-test-wasm.txt](wasm-execution/node-test-wasm.txt) |
| `cargo test -p vn-testkit -p vn-value --no-fail-fast` | 815 pass, 0 fail, no warnings (809 before the wasm work; 6 in wasm_entry.rs) | [cargo-test-testkit-value.txt](wasm-execution/cargo-test-testkit-value.txt) |
| `cargo test -p vn-runtime -p vn-uikit --no-fail-fast` (dependents of vn-testkit) | 124 pass, 0 fail, no warnings | [cargo-test-dependents.txt](wasm-execution/cargo-test-dependents.txt) |
| `node --test tests/upstream/harness.test.mjs tests/language/oracle.test.mjs tests/language/builtin-corpus.test.mjs` (importers of build.mjs) | 112 pass, 0 fail, 2 skipped (no vue-core checkout; pre-existing) | [node-existing-suites.txt](wasm-execution/node-existing-suites.txt) |
| `node tooling/harness/run-language.mjs --with-vue` (native; build.mjs and runner.rs changed) | 115/115 pass | [with-vue-native.txt](wasm-execution/with-vue-native.txt) |
| `node --test --test-reporter=spec "tests/**/*.test.mjs"` (the CI glob) | 815 pass, 0 fail, 133 skipped, of 948 tests (exit 0). The skips are environment reasons printed in the log: no Chrome, sandbox-exec only on macOS, no vue-core checkout, one timezone check. None is a wasm test. | [full-node-suite.txt](wasm-execution/full-node-suite.txt) |

All runs used `CARGO_TARGET_DIR=/home/claude/targets/w42-wasm`. Each log starts with its command line and ends with
`exit=N`.

Per-scenario identity with the native binary (scratch script `cmp-native-wasm.mjs`, not committed): for every
scenario, the wasm document (default threshold) equals the stdout of the native binary built for the same corpus,
plus a newline. Default corpus: 79 of 79 identical, same names in the same order
([log](wasm-execution/wasm-vs-native-default.txt)). Lowering corpus: 166 of 166 identical
([log](wasm-execution/wasm-vs-native-lowering.txt)). The largest document is 1115 bytes (default) and 629 bytes
(lowering).

## GC stress (threshold 1)

The acceptance line in tasks.md says "with automatic roots". The runtime collects only at call depth 0, so the
scenario runner's safepoints are the drains between its steps: after module evaluation, after `run`, and at the end.
The kernel default threshold is 10 000 allocations. A threshold of 1 collects at every safepoint that follows an
allocation. The runner's contract says the document must not depend on the threshold, so the stress runs are compared
with the same goldens:

- native: `VN_GC_THRESHOLD=1` on `run-language.mjs` (environment of the scenario binary): 79/79 and 166/166.
- wasm: `--gc-threshold 1` on `run-language-wasm.mjs` (export `vn_run_scenario_gc`): 79/79 and 166/166.
- The threshold reaches the collector. `the_threshold_reaches_the_collector` (wasm_entry.rs) reads the collection count
  inside `run`: zero at the default threshold, and more than zero at 1. The existing `crates/testkit/tests/runner.rs`
  test checks the same thing for the native runner.
- Not measured: the number of collections in a default-threshold run of the corpus, and the collection count during
  the wasm stress runs. The harness has no export for the count. The wasm entry calls the same runner function as the
  native one, which the native test covers.
- A malformed threshold is an error: `VN_GC_THRESHOLD=abc` exits 2 for the scenario binary, and `--gc-threshold abc`
  exits 1 for the wasm runner ([log](wasm-execution/negative-control-gc-threshold.txt)). The same log shows that
  `VN_GC_THRESHOLD=1` leaves the document unchanged (283 bytes, identical).

## Binary size (scenario module, wasm32-unknown-unknown, debug profile)

| Corpus | Raw | Gzip (level 9) |
|---|---|---|
| default, 79 scenarios | 2,848,360 bytes | 768,387 bytes |
| lowering, 166 scenarios | 3,931,545 bytes | 1,185,016 bytes |

Built with opt-level 0 and `debug = 0`, with no `wasm-opt` and no stripping. A release build was not measured. These
sizes include the `vn_run_scenario_gc` export. Before that export was added the same corpora measured 2,847,945 and
3,931,130 bytes raw.

## Finding: deep recursion trapped on the default wasm stack (fixed)

Native runs on a 256 MiB thread. wasm-ld gives wasm32 a 1 MiB shadow stack by default. Probes showed the difference.
The first two and the map probe are in `tests/language/wasm.test.mjs` (written to a temp directory at test time). The
unbounded plain probe was a scratch run only. `n/m` means not measured.

| Probe | 1 MiB | 2 MiB | 4 MiB | 8 MiB (chosen) |
|---|---|---|---|---|
| plain recursion, 1400 deep (`recurse-1400`) | trap (memory access out of bounds) | pass | pass | pass |
| closure frames with locals, 1400 deep (`recurse-frames`) | trap | trap | pass | pass |
| unbounded map recursion to the 1500-call limit (`overflow-via-map`, RangeError) | trap | n/m | n/m | pass |
| unbounded plain recursion to the 1500-call limit (RangeError, scratch only) | trap | pass | pass | pass |

The 8 MiB stack is set with `-C link-arg=-zstack-size=` through a target-specific `--config` in `buildWasmCrate`.
The deep-recursion test fails at 1 MiB (checked by setting the constant temporarily, with the trap reported). The
corpora did not need the larger stack: both passed on the 1 MiB default in the first wasm runs (79/79 and 166/166).

## Negative controls

- Mutated golden through the CLI: `node tooling/harness/run-language-wasm.mjs --filter to-fixed-ties --golden <dir>`,
  where `<dir>/numbers/to-fixed-ties.js.json` is a copy of the golden with `entries[0].values[0]` set to `"mutated"`.
  Result: `FAIL` with `first difference at $.entries[0].values[0]: "1.00" vs "mutated"`, exit 1
  ([log](wasm-execution/negative-control-mutated-golden.txt)). The node test uses the same kind of control.
- Deliberate panic in a generated entry: the row fails as `trap: RuntimeError: unreachable`, the two other scenarios
  in the same run still pass, and the run reports 0 unexpected failures
  ([log](wasm-execution/negative-control-panic-trap.txt)). The node test asserts the same behaviour.
- Threshold: `the_threshold_reaches_the_collector` (default threshold: no collection seen by `run`; threshold 1: one or
  more). Malformed thresholds exit non-zero ([log](wasm-execution/negative-control-gc-threshold.txt)).
- Native `wasm_entry.rs`: a different entry's document is detected (`assert_ne!`).
- Skip check: `wasmSkipReason` returns a reason for an unknown target (asserted in the node test).
- Stack: the deep-recursion test fails at 1 MiB (see above).

## Remaining limits

- Scenarios run in Node's V8 only. No browser run was done (Chrome is not available here). The browser host does not
  run scenarios.
- The wasm runner requires every scenario to pass and does not read `tests/language/status.json`. That ledger has
  no blocked entries (79 pass). The summary prints `blocked 0` to match run-language's format.
- The runtime's call-depth limit (1500, `DEFAULT_MAX_CALL_DEPTH`) appears to count the builtin frame of
  `Array.prototype.map` as well as its callback. Recursion through `map` at 700 levels throws
  `RangeError: Maximum call stack size exceeded` on native and on wasm, where Node succeeds. A 400-level probe passes
  on both. This is a runtime semantics question, not a wasm one, and it was not changed.
- "Automatic roots" is covered only at the runner's safepoints (see the GC section). Roots of temporaries inside a
  running call are not exercised by the stress, because the runtime never collects at call depth above 0.
- The planning text for task 4.2 cites the lowering corpus as 164/164. The corpus now has 166 scenarios, and both
  targets run all 166. The planning file was not edited.
- Warnings: the full default and lowering wasm builds print none. A build of two scenarios and the panic control (the trap run)
  printed one warning, unused imports of the names re-exported by the generated `vn_async.rs`.
- Node test cost: 2.7 s on this machine with warm caches (node-test-wasm.txt). The CI budget is 3 minutes.

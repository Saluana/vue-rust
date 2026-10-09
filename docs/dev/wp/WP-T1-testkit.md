# WP-T1 — `vn-testkit`: Rust-side trace encoder, `testHost`, and scenario runner

Read `docs/dev/AGENT-CONTRACT.md`, `docs/dev/ARCHITECTURE.md`, **`docs/dev/trace-format.md`** (the contract) and
`crates/value/ABI.md` (the kernel API). `export CARGO_TARGET_DIR="$VN_ROOT/target/t1"`.
**Prerequisites done:** L1, L2a (kernel), and the Node oracle from WP-O1 (`tooling/oracle/*`, goldens in `tests/language/golden/`).

**Owned paths:** new crate `crates/testkit/**` (package `vn-testkit`, library `vn_testkit`, depends on `vn-value` by path only,
std otherwise), `tests/language/rust-trace/**` (extra fixtures if you need them). Do not edit `crates/value` — if the kernel lacks
an accessor you need, put the request in your report and work around it with what exists.

## Why
Differential testing needs the compiled side to print the *same* canonical document as the Node oracle. Generated scenario
modules (from the compiler WP) call `trace(...)` and `testHost.*`; this crate provides them and the process-level runner, and
is the single place that serialises runtime values for comparison.

## Do
1. `src/json.rs`: a minimal JSON **writer** (no parser needed in the library): strings escaped exactly like `JSON.stringify`
   (`\"`, `\\`, `\b \f \n \r \t`, other controls `\u00xx`, lone surrogates `\udxxx` in lowercase hex), numbers via
   `vn_value::number::number_to_string`. A compact document builder is enough.
2. `src/trace.rs`: `Recorder` implementing `encode` exactly per trace-format.md (global object/symbol ordinals in order of first
   encounter, per-`trace`-call expansion with `{"$":"ref","id":N}` for repeats, `props` in ordinary own-key order, array holes and
   `len`, accessors encoded without invoking getters, `fn` entries with name/length, `Error` objects with name/message,
   Map/Set support as soon as such `ObjectKind`s exist in the kernel (write the code behind a clearly marked section and a
   Cargo feature-free `cfg` of your choice if they do not exist yet; list what is missing in your report), `TypeError("untraceable value")`
   for unsupported values). Unit tests with hand-written expected JSON strings for every row of the encoding table.
3. `src/host.rs`: `install_host(rt: &Rt, rec: &Rc<RefCell<Recorder>>)` defines global `trace` (native function) and `testHost`
   with `timers.{setTimeout, clearTimeout, now, advance}` and `tick` per trace-format.md. `advance`/`tick` return promises: if the
   kernel does not have `Promise` yet, implement the **synchronous parts** now (timers table, due-time ordering by (due, creation),
   clock) and expose them as Rust functions `Timers::advance_sync(rt, ms)` that fire due callbacks calling `rt.run_jobs()` after
   each; the JS-visible promise-returning wrappers are added in a follow-up once `Promise` lands (leave a `TODO(WP-B-promise)` and say so in the report).
   Timer callbacks and the recorder must be reachable by the GC (use root providers / persistent roots; test it by forcing
   `rt.collect()` between steps).
4. `src/runner.rs`: `run_scenario(name: &str, entry: fn(&Rt) -> Res<ScenarioEntry>) -> String` where `ScenarioEntry { run: Value /* export `run` or Undefined */ }`
   produced by generated code (the compiler WP defines the generated `entry`; your tests use hand-written `entry` functions built on the kernel API).
   Implements steps 1–5 of the scenario contract: fresh `Rt`, host installed, evaluate (catch throw → `result:"throw"`), drain jobs, call `run`
   if callable (await its result once promises exist; for now treat a returned non-promise as done), drain, collect unhandled
   rejections (list stays empty until promises exist), return the document string. Run on a thread with a large stack (256 MiB) so deep
   JS recursion cannot overflow the Rust stack: `run_scenario_isolated` spawns the thread and joins; panics become a document
   with `result:"panic"` and the panic message (framework bug marker).
5. `src/bin/scenario_main.rs` is NOT needed; instead export `pub fn scenario_main(table: &[(&str, ScenarioFn)])` that a generated `main.rs` calls: it
   reads `argv[1]` as the scenario name (or `--list`), runs it, prints the one-line document to stdout, exits 0 (even for `result:"throw"`).
6. Tests (`crates/testkit/tests/*.rs`): hand-built object graphs → compare the produced JSON **textually** against hand-written
   expected strings (aliases, cycles, holes, symbols, accessors, functions, errors, key order with integer keys, numbers incl. -0/NaN/
   exponent forms, lone surrogate strings, shared objects across two trace calls keeping ids, mutation between calls showing new contents);
   cross-check against the Node encoder: a Node script (`tests/language/rust-trace/cross-check.mjs`) builds the same graphs in JS, encodes
   them with `tooling/oracle/trace.mjs`, and the Rust test (run via `cargo test`) shells out to `node` (allowed in tests) or reads the
   script's committed output JSON (preferred: generate fixture JSON with the Node script and `include_str!` it) to assert the two encoders
   agree on ≥ 25 graphs; timers order test; GC-between-steps test; panic isolation test; negative control (doctored expected string must be reported).

## Part 2 — corpus replay (builtin-call differential tests; format in `docs/dev/builtin-corpus.md`)
Prerequisite for this part: WP-O2 delivered `tooling/oracle/builtin-corpus.mjs` and `tests/language/builtins/*.json`.
7. `src/json_in.rs`: a small, strict JSON **reader** (objects/arrays/strings with `\uXXXX` incl. lone surrogates → UTF-16 units / numbers / bool / null) used by the replay
   harness only; it must preserve number text exactly enough to reproduce doubles (parse with Rust `f64::from_str` on the literal) and must reject malformed input with an error value.
8. `src/decode.rs`: `decode(rt, &Json, &mut IdTable) -> Res<Value>` — the inverse of `encode`: builds objects/arrays (with holes)/Map/Set/Error/functions-placeholders/symbols,
   restores aliasing and cycles from `id`/`ref`, own-key order (integer keys must end up first as the object model dictates), accessor markers are rejected for inputs.
   Tests: for ≥ 30 graphs, `encode(decode(encode(x))) == encode(x)` using hand-built expected JSON.
9. `src/presets.rs`: native implementations of every callback preset in builtin-corpus.md (`identity, double, isEven, isTruthy, index, pair, sum, concat, cmpNum, cmpDesc, cmpStr,
   cmpThrow, throwAt2, toArr2, keyOf`) that append `[presetName, ...scalar args]` to the shared log array exactly like the JS versions (read the JS file; the behaviours must be identical).
10. `src/replay.rs`: `replay_suite(json_text: &str, opts: &ReplayOptions) -> ReplayReport { total, passed, failures: Vec<Failure{id, reason, expected, actual}> }`: for each case create a fresh
    `Rt`, resolve `target` through the global object (`new:X` → `construct`), decode `pre`, substitute `cbs` presets, call, encode `[ret, this, args…, log]` with a fresh recorder, compare
    structurally with `post` (exceptions: compare `name` only). `approx` cases compare numbers within the stated ulps. A case whose target does not exist yet in the runtime is reported as
    `Missing(target)` — not a failure — so suites can be adopted before every builtin exists, but the report must list missing targets and the replay test must be able to assert "no missing" per suite.
    `tests/replay_math.rs`-style integration tests live in the builtin WPs; here only unit-test the harness against a hand-written mini suite with builtins you define natively in the test, plus a negative control.
11. Add to `scenario_main` a hidden subcommand `--replay <suite.json>` printing the `ReplayReport` as JSON (for quick manual runs).

## Acceptance
`cargo test -p vn-testkit` passes; `cargo build -p vn-testkit` has no warnings; Node cross-check fixture regenerates identically; no `unsafe`.

# Coordinator status (living file; the coordinator updates it, implementers do not)

Last updated: see bottom. Source of truth for scope: `planning/vue-native-framework/{requirements,design,tasks}.md`.
Process: coordinator writes a WP spec (`docs/dev/wp/WP-*.md`), a Haiku subagent implements, the coordinator re-runs the
acceptance commands, reads code, and either accepts or sends corrections. Shadow review repo lives in the session scratchpad
(never in the project). Rust toolchain: `source scripts/env.sh`.


## BUDGET / PLAN LIMITS (read this first after any context compaction)
- Account: **Pro**, extra usage DISABLED. On 2026-10-09 06:26 PDT: 5-hour window 9% used (resets 11:20 PDT), **weekly 73% used (resets 2026-10-13 04:00 PDT)**.
  An earlier burst of 11 concurrent max-effort Haiku agents exhausted the 5-hour window (HTTP 429 "session limit", agents died mid-work; their files and
  mirrors under `target/mirrors/<wp>` are intact and the agents can be resumed with SendMessage).
- Strategy now: at most 3-5 agents at a time, short prompts, "report in <= 150 words", no re-reading of big files, measure cost with `mcp__ccd_session_mgmt__get_usage`
  (ToolSearch `select:mcp__ccd_session_mgmt__get_usage`) after each batch. Goal order: finish a coherent vertical slice (compiler core -> builtins -> reactivity -> runtime
  -> hosts -> Counter/Quote demo on web + native), then breadth. Be honest in the final report about what could not be finished for budget reasons.
- Agent ids to resume (name = WP): C1b a6c5b63c19a91f0c1 · B1 a02c45083adb536fc · B3 a5ba5505a13638eb3 · B6 a0aa78b0c52208d81 · B4 a0e8cf3090afb0962 · B2 af1a70ea3f18f872a · B5 a9b6bb0285e7a20ca ·
  R1a a75d16853a1a569f8 · RT-A a0e05d22ba20edec7 · V1 aa3b9fb06c45574d6 · W1a a151e431efb656b58 · U1b af65e45618f2c08f0. (Finished agents can also be resumed for small follow-ups.)
- Unpushed progress lives in `target/mirrors/<wp>` (b1 b2 b3 b4 b5 b6 r1a ...); `scripts/mirror.sh pull|push <wp>` syncs with the repo.


## HOW TO CONTINUE (written 2026-10-09 ~08:20 PDT; use after any context compaction)
Commands (always `cd /Users/brendon/Documents/vue-rust-lab && source scripts/env.sh`; use `export CARGO_TARGET_DIR="$VN_ROOT/target/review"` for my own runs):
- Rust: `cargo test --workspace --no-fail-fast` (if results look stale: `touch $(find crates -name '*.rs')`). JS: `node --test "tests/<dir>/*.test.mjs"` (quoted globs).
- Compiler harness: `node tooling/harness/run-language.mjs [--dir tests/language/lowering] [--filter x] [--write-status]` (ratchet ledger `tests/language/status.json`).
- Oracles: `node tooling/oracle/check-goldens.mjs`, `node tooling/oracle/vue-env.mjs`, `node tests/language/gen/gen-*.mjs` (deterministic generators), Chrome oracle `tooling/chrome`.
- Usage meter: ToolSearch `select:mcp__ccd_session_mgmt__get_usage`. Calibration: ~12 agent runs ~ 26 pts of the 5-hour window, ~4 pts weekly.
- Agents: SendMessage to the ids above to resume/steer (finished agents can be resumed). Run <= 7 at a time. Check results yourself (rerun tests, negative controls) before accepting.
Critical path to a working vertical slice (Counter + Quick Quote on web and native):
 1. R1a reactivity core (running) -> R1b reactive/readonly/shallow proxies + arrays/Map/Set instrumentation (spec TODO; needs B2/B4 = done).
 2. RT-A vnode/shape/helpers (running) -> RT-B instance+scheduler+watch+errors+lifecycle+inject (spec written: docs/dev/wp/WP-RT-B-instance-scheduler.md; start after R1a/RT-A push) -> RT-C renderer+component+props/emits/slots+directives+app (spec TODO) -> RT-D runtime-dom props/events/vModel*/vShow + `DocHost` (Host impl over vn-dom, event bridging; spec TODO) -> RT-E KeepAlive/Teleport/Transition (optional).
 3. C1c async+projects (running) -> then add `--with-vue` to the harness so tests/vue/cases and tests/ui/cases run against vn-runtime.
 4. V1 UI oracle (running, real Vue in Chrome) = acceptance for RT-C/D. W1a web host (running) -> web demo page (wasm + glue) with the compiled SFC app.
 5. Native: U1b style (paused), U1c layout (Taffy; spec TODO), egui adapter + eframe host (spec TODO); a11y tree. Cannot be screenshotted here (see limits).
 6. Tooling: `tooling/cli` (create/check/build/dev) wiring compileSfc + compileProject + cargo + wasm packaging; demos `examples/counter`, `examples/quick-quote`.
 7. If budget remains: services (files/settings/sqlite), SSR+hydration, Fieldboard, docs/README rewrite, evidence ledger (docs/evidence), final audit against requirements R01-R26 with honest status.

## Decisions taken (binding)
- Toolchain installed project-locally under `toolchain/` (Rust 1.99.0 + wasm32). npm: exact pins, scripts disabled. User approved
  rustup, npm ci, vue@3.5.43 oracle, crates.io fetches as builds need them. **Not yet approved** (ask before using): new npm
  packages beyond vue (typescript/vue-tsc/…), Docker images, Android NDK/JDK downloads, Playwright browsers.
- Frontend = official Vue SFC compiler outputs (compileScript inlineTemplate + compileTemplate + compileStyle) lowered by our JS→Rust compiler; Vue semantics = Rust ports verified by differential tests against real vue@3.5.43 (production build). See `ARCHITECTURE.md`.
- Native tag set (U31): main section div header footer nav article aside form ul ol li hr h1–h6 p span label strong em b i small code pre br a button input textarea select option img. Others X.
- `useCssModule`/`useCssVars` are P (CSS contract). Compiler-helper exports (openBlock, withCtx, …) are `compilerInternal`: allowed from generated code, rejected in user code.
- GC runs only at job boundaries (call_depth 0); locals of async functions live in heap frames.
- Cargo: never share a CARGO_TARGET_DIR between same-named crates; each WP uses `target/<wp>`.
- Node tests: quoted globs (`node --test "dir/*.test.mjs"`); `npm test` runs `tests/**/*.test.mjs`.

## Accepted work packages
| WP | What | Evidence |
|---|---|---|
| G0A | prototype fixtures + JS baseline + KNOWN-BUG regressions + claims ledger | `tests/fixtures/prototype/**`, `docs/evidence/baseline/` — 21+53 tests pass |
| G0D | Rust baseline of prototypes (counter 4 tests, Quote 4 tests isolated, native+wasm builds) | `docs/evidence/baseline/rust-*`; coordinator re-ran counter tests |
| G0B | compat manifest (126 rows, 446 entries incl. 52 native-tag, 171 vue exports; 40 compilerInternal helpers) + coordinator corrections applied | `compiler/profile/**`, `tests/profile/` 52 tests; verified by direct API queries |
| G0C | sandbox-exec runner + hostile fixtures + dependency policy | `tooling/sandbox`, `tests/security/build-isolation` 27 tests |
| L1 | `vn-value` JsStr + number semantics vs V8 corpora (25k + 15k cases) | `crates/value`, 58 tests, corpora regenerate byte-identical |
| O1 | Node oracle (trace format v1) + 77 scenarios + goldens | `tooling/oracle`, `tests/language` 53 tests |
| C1a | diagnostics, parser (js/ts/jsx/tsx), 55-rule profile checker, scope/closure analysis | `compiler/{diagnostics,frontend,analysis}`, 201 tests; V8 TDZ cross-check: 0 false negatives |
| X1 | headless Chrome CDP oracle runner (zero deps) | `tooling/chrome`, 54 tests; layout smoke verified by coordinator |
| V0 | 102 Vue reactivity/scheduler scenarios, goldens from real vue@3.5.43 production build | `tests/vue/**`, `tooling/oracle/vue-env.mjs`; 13 tests + 102/102 golden match |
| F1 | SFC frontend `compileSfc` (official compiler + profile checks + native template validation + styles) | `compiler/sfc/**`, 210 tests incl. 31 components rendered in real Vue in Chrome; deterministic output verified |
| O2 | builtin-call corpus library + seed suites (math 2092, number 244, object-statics 114 cases; V8-recorded) | `tooling/oracle/builtin-corpus.mjs`, 28 tests; trace-format v1.1 `integrity` amendment applied by O1 (79 language goldens, 102 vue goldens still match) |
| U0 | `vn-dom` DOM-like document (tree, attrs, props with dirty flags, style, classList, events with activation, focus, selectors, serialization) | `crates/dom`, 16 corpus + 24 unit tests; 284 Chrome-recorded cases; input/change after click synthesised like Chrome |
| U1a | `vn-css` tokenizer/parser/selectors/media/cascade/var() | `crates/css`, 98 tests; ~3400 Chrome comparisons; mutation checks |
| L2b | operators, equality, conversions (46k V8 operator cases) + kernel amendments (Exotic::is_array, Wrapper kind, class_of) | `crates/value/{coerce,ops}.rs`, ABI §13 |
| L2c | ES module records/loader (declare/bind/evaluate, live bindings, cycles, namespace, native modules, locals table) | `crates/value/src/module.rs`, 324 vn-value tests total, ABI §14, 22 Node-ESM corpus cases |
| G0E | closed-world stdlib/globals manifest (465 entries, 326 supported; Math.random/Date.now A, Uint8Array/ArrayBuffer P) | `compiler/profile/stdlib*.{json,mjs}`, 92 profile tests; lint + corpus gate + C1a now consume it |
| T1 | `vn-testkit`: Rust trace encoder (v1.1), testHost/timers, scenario runner, corpus replay (decode, presets, replay_suite) | `crates/testkit`; 65 pass, 2 known failures pending kernel fix (array isFrozen follows V8) + Array.prototype.toString (B2) |
| B0 | iterator protocol foundation (`builtins/iterator.rs`: GetIterator/IteratorStep/Close, native iterator objects) | 53 tests, 21-fault mutation check; vn-value total 385 pass |
| B1 | Object/Object.prototype/Function.prototype/Symbol/Boolean/globals (95 tests + 6 replay suites, zero failures) | `crates/value/src/builtins/{object,function,symbol,boolean,global}.rs` |
| B3 | String/Number/Math (own suites 3.9k cases zero failures; O2 math/number green except array-arg cases; 497 vn-value tests) | `builtins/{string,number,math}.rs`; transcendental fns within 1-2 ulp of V8 (documented) |
| C1b | **JS/TS -> HIR -> Rust compiler core + differential harness**: 153/153 lowering scenarios and 36 O1 scenarios pass vs Node goldens (clean rebuild + negative control verified by coordinator); 43 O1 scenarios blocked (Array builtins 13, async 17, Map 2, ...) | `compiler/{hir,emit,index.mjs}`, `tooling/harness`, `tests/language/status.json` ratchet |
| B2 | Array (ctor, isArray/of/from, 36 prototype methods, iterators; 4,862-case V8 corpus zero failures, 51 unit tests) | `builtins/array.rs`, ABI §16 |
| B6 | JSON.parse/stringify (413 corpus cases replay clean, 45 unit tests) | `builtins/json.rs` |
| L2a | `vn-value` kernel: values, generation-checked heap, iterative mark-sweep GC at depth 0, object model, exotic hooks, functions/cells/TDZ, errors, jobs, watchdog | `crates/value`, 232 tests, mutation checks; ABI in `crates/value/ABI.md` |

## In flight
RUNNING (2026-10-09 ~08:30 PDT): B4 Map/Set (testkit Map/Set wiring) · B5 Promise/async · R1a reactivity core · RT-A vnode/helpers · C1c async+projects · V1 UI oracle · W1a web host · RT-D DocHost/patchProp/events/vmodel (id a4789a4e4930d196d, spec WP-RT-D-dom-layer.md). B2 done (callback suites regenerated: 5,685 replay cases green). NEW: C1c async lowering + multi-module + native/.vue imports. PAUSED (resume later, ids above): U1b style · V1 UI oracle · W1a web host. Coordinator added: builtins skeleton (`crates/value/src/builtins/*`, `Rt::register_builtin/builtin`, `install_all` hook) and `scripts/mirror.sh` private-mirror workflow.

## Queue (specs written unless noted)
B2 Array + B4 Map/Set (after B0) · B5 Promise/async runtime + C1c async/await lowering (after C1b) · R1b reactive proxies/collections (after B2/B4) · RT1.. Vue runtime port · 
stdlib/global allowlist manifest (G0E) · CSS-modules async SFC entry · typescript install (needs user OK) ·
U1b style/layout/Taffy (not yet) · reactivity port · runtime port · services · hosts · apps · SSR.

## Scoreboards (coordinator-verified 2026-10-09 ~08:00 PDT)
- Rust workspace: 823 tests pass, 2 fail (vn-ui cascade_corpus: U1b paused mid-work).
- Compiler harness: lowering corpus 153/153; O1 language scenarios 59 pass / 20 blocked (async/promises/timers + Map tracing) / 0 fail / 0 unexpected (`tests/language/status.json` ratchet).

## Known deviations / honest limits so far
- Native window visuals cannot be screenshotted from this environment (`screencapture`: no Screen Recording permission; never use osascript/System Events — it triggers a permission prompt). Native correctness is verified with headless egui tests + synthetic input + process liveness; the user can eyeball it with `npm run native`.
- Cargo staleness hazard (fixed): rsync -a preserved old mtimes, so cargo kept stale artifacts after mirror pull/push; mirror.sh now copies by checksum with fresh mtimes. If tests look inexplicable, `touch` sources or `cargo clean -p <crate>`.
- Real devices, Windows, Linux hardware, Safari/Firefox, VoiceOver/NVDA/TalkBack, notarization and store signing cannot be exercised on this
  machine (macOS arm64, Chrome 155, Xcode with iOS simulators; Android SDK present but no NDK/JDK; Docker present). They stay unqualified in the ledger.
- 2026-10-09T05:01Z

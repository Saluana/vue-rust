> History only. This is the working log of the first build, which was done as several work packages in parallel.
> It mentions workers, ids, limits and plans that no longer apply. Current state: `../HANDOFF.md` and `../STATUS.md`.

# *** PAUSED (2026-10-09 08:12 PDT): all subagents finished, nothing is running. Read docs/dev/HANDOFF.md first. ***
# Coordinator status (living file; the coordinator updates it, implementers do not)

Last updated: see bottom. Source of truth for scope: `planning/vue-native-framework/{requirements,design,tasks}.md`.
Process: coordinator writes a WP spec (`docs/dev/wp/WP-*.md`), a Haiku subagent implements, the coordinator re-runs the
acceptance commands, reads code, and either accepts or sends corrections. Shadow review repo lives in the session scratchpad
(never in the project). Rust toolchain: `source scripts/env.sh`.


## BUDGET / PLAN LIMITS (read this first after any context compaction)
- **AT PAUSE (2026-10-09 08:12 PDT): 5-hour window 84% used (resets 11:20 PDT) — do not launch agents before then; weekly 83% used (resets 2026-10-13 04:00 PDT), i.e. roughly 17 points left, ~4 points per 12 agent runs.**
- Account: **Pro**, extra usage DISABLED. (Earlier, 06:26 PDT: 5-hour 9%, weekly 73%.)
  An earlier burst of 11 concurrent max-effort Haiku agents exhausted the 5-hour window (HTTP 429 "session limit", agents died mid-work; they were resumed then).
  Old agent ids can NOT be resumed from a new session, and `target/mirrors/*` was deleted at pause (it held nothing unpushed): start fresh agents from the WP specs.
- The "shadow review repo" mentioned below lived in the previous session's private scratchpad; a new session does not have it (not needed).
- Strategy now: at most 3-5 agents at a time, short prompts, "report in <= 150 words", no re-reading of big files, measure cost with `mcp__ccd_session_mgmt__get_usage`
  (ToolSearch `select:mcp__ccd_session_mgmt__get_usage`) after each batch. Goal order: finish a coherent vertical slice (compiler core -> builtins -> reactivity -> runtime
  -> hosts -> Counter/Quote demo on web + native), then breadth. Be honest in the final report about what could not be finished for budget reasons.
- Agent ids (historical; a NEW session cannot resume them, spawn fresh agents from the WP specs) (name = WP): C1b a6c5b63c19a91f0c1 · B1 a02c45083adb536fc · B3 a5ba5505a13638eb3 · B6 a0aa78b0c52208d81 · B4 a0e8cf3090afb0962 · B2 af1a70ea3f18f872a · B5 a9b6bb0285e7a20ca ·
  R1a a75d16853a1a569f8 · RT-A a0e05d22ba20edec7 · V1 aa3b9fb06c45574d6 · W1a a151e431efb656b58 · U1b af65e45618f2c08f0. (Finished agents can also be resumed for small follow-ups.)
- (Obsolete) Earlier, unpushed progress lived in `target/mirrors/<wp>`; at pause every mirror was verified to be older than the repo and deleted. `scripts/mirror.sh init|pull|push <wp>` still works if a new task wants a private copy.


## TASKS.MD RULE (added 2026-10-09 after the user asked)
After every accepted WP / milestone: edit `docs/dev/tasks-progress.json` (task id -> status done|partial|blocked|notstarted, summary, evidence command + artifacts, missing) and run `node tooling/status/update-tasks.mjs` (idempotent; `--check` verifies). A box is ticked only for status `done` (acceptance met + test command + artifact; no commits exist). Currently 9 done / 24 partial / 7 blocked / 30 not started.

## HOW TO CONTINUE (written 2026-10-09, refreshed 08:12 PDT; use after any context compaction)
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

## NEXT AFTER RT-C (plan; specs TODO unless noted)
- T2 spec WRITTEN (docs/dev/wp/WP-T2-ui-testhost.md; launch when crates/runtime/src/app.rs is non-empty): Rust UI `testHost` (mount/find/click/input/... per docs/dev/ui-scenario-host.md) on top of `DocHost` + `--with-vue` UI scenarios (tests/ui/cases) in the harness; then RT-C acceptance = V1 scoreboard.
- RT-E: KeepAlive/Teleport/Transition/defineAsyncComponent (stubs exist: keep_alive.rs, teleport.rs, transition.rs).
- Web demo: W1a web host + compiled SFC app (wasm) in Chrome; Native: U1b->U1c layout->egui adapter. CLI `tooling/cli`; examples/counter, examples/quick-quote; README rewrite; evidence ledger; final audit.
- Budget check each batch; prefer finishing the slice over breadth.

## NEXT: after RT-C round 2 -> resume APP1 (id a0af638f8646d01bf) for `build --target web` + Chrome e2e of Counter and Quick Quote; then V1 scoreboard fixes; then RT-E (KeepAlive/Teleport/Transition), native lane, SSR, services, docs/README, final audit.

## OPEN REQUESTS FROM RT-C (apply after the V1 scoreboard shows what matters)
- vnode.rs (RT-A): expose vnode fields `type`, `appContext`, `ce` as JS-readable props (RT-C currently defines data properties before instance creation); `normalize_ref` should set owner `i` = current rendering instance (template refs in slot content).
- R1a: export a lazily-run ReactiveEffect + `allowRecurse` control for component effects (component does not re-run for its own render mutations).
- (Landed in RT-C round 2: useModel, mergeModels, mergeDefaults, createPropsRestProxy.) Still missing in RT-C: block-stack unwinding in withCtx/render errors; keyed move-count comparison.

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
| B4 | Map/Set + iterators (1,080 V8 corpus cases clean; 55 unit tests; testkit Map/Set encode/decode; Node cross-check 43 graphs) | `builtins/map_set.rs`; vn-value+vn-testkit 786 pass |
| R1a | `vn-reactivity` core: Dep/Link graph, effects, batch, computed, EffectScope, refs, watch base (88 tests; 76 snippets match real @vue/reactivity; mutants killed) | `crates/reactivity`, `tests/vue/rust/` |
| RT-A | `vn-runtime` data layer: shape/patch flags, VNode + block tracking, createVNode family, normalizeClass/Style, toDisplayString, renderList/mergeProps/etc.; 947 helper cases + 420 vnode scenarios match real vue@3.5.43 | `crates/runtime/src/{shapes,vnode,normalize,helpers,exports}.rs`, `tests/ui/runtime-a/` |
| B5 | Promise/AggregateError/unhandled-rejection tracking + async runtime (`new_frame/frame_get/frame_set/async_start/await_value`); 403 promise programs equal Node, mutation check kills 101 | `builtins/promise`, `tests/language/promise-programs` |
| C1c | async/await lowering (heap frames + Rust async blocks), multi-module projects (`compileProject`), native-module imports, `.vue` imports, TS erasure: O1 79/79, lowering 164/164, projects 10/10, 37/37 real SFC fixtures lower to Rust that passes cargo check, GC-stress identical | `compiler/project`, `tooling/harness/{run-projects,check-sfc-lowering}.mjs` |
| RT-B | `vn-runtime` instance record, exact 3.5 scheduler (+nextTick over real promises), watch/watchEffect*, error routing, lifecycle hooks, provide/inject; 38 tests; with R1a/R1b: V0 102/102 | `crates/runtime/src/{instance,scheduler,watch,errors,lifecycle,inject}.rs` |
| V1 | UI oracle: real vue@3.5.43 prod build in headless Chrome + `testHost` + 160 component scenarios/goldens (static, conditional, list, events, model, props, emits, slots, provide-inject, lifecycle, refs, watch, errors, dynamic, teleport, transition, fragments, directives) | `tooling/oracle/ui`, `tests/ui/{cases,golden}`, 26 tests; regeneration byte-identical; see docs/dev/ui-scenario-host.md |
| R1b | reactive/readonly/shallow* proxies (Exotic), array + Map/Set instrumentation, toRaw/markRaw/is*: 240 tests, 152 oracle snippets, V0 102/102; follow-up in progress: Exotic prevent_extensions/is_extensible hooks | `crates/reactivity/src/{reactive,handlers,array_inst,collection_inst}.rs` |
| RT-D | DOM layer: `DocHost` (Host over vn-dom, re-entrant events), patchProp decision tree, event invokers + JS event object, v-model*/v-show, withModifiers/withKeys; 944 patchProp cases + 174 directive scenarios recorded from real Vue in Chrome; follow-up: svg fill + 1 skipped case | `crates/runtime/src/{host_doc,dom_props,dom_events,vmodel,vshow}.rs`, `tests/ui/runtime-d` |
| W1a | browser host: `WebHost` (wasm32, 44 imports, 38 KB wasm / 13.6 KB gz) + hand-written JS glue + build script; 290 Chrome tests pass, 0 fail, 105 skipped (outside Host surface); policy refusals for script/iframe/on*/innerHTML | `crates/renderer-dom`, `hosts/web`, `tooling/web`, `tests/platform/web` |
| RT-C | renderer (mount/patch/unmount/move, keyed diff + LIS, fragments), components (setup, props, emits, slots, attrs fallthrough, lifecycle, render-error comment), refs, directives, createApp/mount/use/component/directive/provide; 21+8+4 scenarios recorded from real Vue in Chrome; NOT yet run against the 160 V1 goldens (needs T2) | `crates/runtime/src/{renderer,keyed,component,props,emits,slots,public_instance,setup_helpers,refs,directives,app}.rs` |
| APP1 | `vue-native` CLI (check/build/create/doctor) + web packaging + headless target: **Counter demo runs end to end in the Rust Vue runtime** (mount, click, v-if, reset, disabled); web build of .vue projects blocked until the renderer is Host-generic; Quick Quote (defineModel) passes headless since RT-C r2 (38/38 CLI tests at 08:12) | `tooling/cli`, `examples/`, `tests/cli`, `docs/guide/getting-started.md`; 39/40 tests |
| T2 | `vn-uikit` Rust UI testHost (mount/find/click/input/... over DocHost, snapshot encoder) + `--with-vue --ui` harness: **131/160 real-Vue component scenarios pass, 29 blocked with classified causes, 0 fail** | `crates/uikit`, `tooling/harness/ui-mode.mjs`, `tests/ui/status.json` |
| RT-C r2 | renderer made Host-generic (`HostProvider`/`dyn Host`), useModel/mergeModels/mergeDefaults/createPropsRestProxy, scoped style ids, vnode field accessors, lazy component effects | runtime 18/18 test binaries, V0 102/102 |
| L2a | `vn-value` kernel: values, generation-checked heap, iterative mark-sweep GC at depth 0, object model, exotic hooks, functions/cells/TDZ, errors, jobs, watchdog | `crates/value`, 232 tests, mutation checks; ABI in `crates/value/ABI.md` |

## In flight
NOTHING IS RUNNING (paused 08:12 PDT). Last finished: T2 UI testHost + --ui scoreboard; RT-C round 2 (Host-generic renderer, useModel, scoped ids, vnode fields); APP1 headless Counter demo (its web build is the next step, spec `WP-APP1-cli-web-demo.md`). Earlier-running text, kept for history: RUNNING (local 08:00): T2 UI testHost + --ui scoreboard (id ac0dae7553600c662) · RT-C round 2: Host-generic renderer, useModel, scoped ids, vnode fields (id ad83666a1d6a162fe) · APP1 CLI+web build+demos (id a0af638f8646d01bf; spec WP-APP1-cli-web-demo.md); small follow-ups R1b (kernel hooks) + RT-D (svg fill); RT-C renderer+components+props/emits/slots/refs/directives/app (id ad83666a1d6a162fe; spec WP-RT-C-renderer-components.md) · R1b reactive proxies/collections (id afb64a220600e296f, spec WP-R1b) · RT-B instance/scheduler/watch/errors/lifecycle/inject (id a7fb06751859cda5a) · B5 Promise/async · R1a reactivity core · RT-A vnode/helpers · C1c async+projects · V1 UI oracle · W1a web host · RT-D DocHost/patchProp/events/vmodel (id a4789a4e4930d196d, spec WP-RT-D-dom-layer.md). B2 done (callback suites regenerated: 5,685 replay cases green). NEW: C1c async lowering + multi-module + native/.vue imports. PAUSED (resume later, ids above): U1b style · V1 UI oracle · W1a web host. Coordinator added: builtins skeleton (`crates/value/src/builtins/*`, `Rt::register_builtin/builtin`, `install_all` hook) and `scripts/mirror.sh` private-mirror workflow.

## Queue (specs written unless noted)
B2 Array + B4 Map/Set (after B0) · B5 Promise/async runtime + C1c async/await lowering (after C1b) · R1b reactive proxies/collections (after B2/B4) · RT1.. Vue runtime port ·
stdlib/global allowlist manifest (G0E) · CSS-modules async SFC entry · typescript install (needs user OK) ·
U1b style/layout/Taffy (not yet) · reactivity port · runtime port · services · hosts · apps · SSR.

## Scoreboards (coordinator-verified 2026-10-09 ~08:00 PDT)
- Rust workspace: 823 tests pass, 2 fail (vn-ui cascade_corpus: U1b paused mid-work).
- Compiler harness (coordinator-verified 09:25 PDT): lowering corpus 163/163; **O1 language scenarios 79/79 pass** (0 blocked, 0 fail; async/promises/timers included) — `tests/language/status.json` all 'pass'.

- **Vue-runtime scoreboard (coordinator-verified 10:05 PDT, `node tooling/harness/run-language.mjs --with-vue`, V0 corpus): 102/102 pass, 0 blocked, 0 fail** — ref 18/18, reactive 25/25, computed 12/12, effect 8/8, watch 27/27, scheduler 12/12 (real vue@3.5.43 goldens).

## FINAL SCOREBOARDS AT PAUSE (coordinator-verified 08:10 PDT)
- CLI/web at 08:12: `tests/cli` 38/38; `tests/platform/web` 398 = 293 pass + 105 skipped-with-reason + 0 fail.
- Two test files were edited during the final verification (not by an agent): `tests/cli/headless.test.mjs` (the runtime now stamps scoped components with `data-v-<hash>` and the container with `data-v-app`, like real Vue; `runApp` strips the stamps from `html`, keeps `rawHtml`, one test asserts the stamps exist; the Quick Quote sequence now runs on the real `defineModel` example as well as the fixture) and `tests/platform/web/e2e-host-only.test.mjs` (private cargo target dir; see HANDOFF trap (f)).
- Rust workspace: 1319 tests pass, 2 fail (vn-ui cascade_corpus — style work paused mid-way).
- Compiler harness: O1 language 79/79; lowering 164/164; projects 10/10; SFC fixtures lowering 37/37.
- Vue runtime vs real vue@3.5.43 (`--with-vue`): reactivity/watch/scheduler 102/102.
- Components (`--with-vue --ui`, 160 real-Vue-in-Chrome goldens): **131 pass / 29 blocked / 0 fail**. Blocked causes: Teleport/Transition/KeepAlive/defineAsyncComponent missing (17: teleport/*, transition/*, dynamic/*), `reactive()` wraps DOM handles (7: refs/*, lifecycle/mounted-template-ref, watch-in-component/flush-post-sees-dom), vn-dom lacks `margin` shorthand (2), DOM-listener errors skip errorHandler (3: errors/*).
- A `.git` directory exists in the project (created 06:50 local on 2026-10-09, not by the coordinator; `origin` = github.com/Saluana/vue-rust, branch `main`, one commit `95a2b0d` authored by the user at 06:52, ~233 uncommitted entries). The coordinator never committed or pushed; ask the user before doing so.

## Known deviations / honest limits so far
- Native window visuals cannot be screenshotted from this environment (`screencapture`: no Screen Recording permission; never use osascript/System Events — it triggers a permission prompt). Native correctness is verified with headless egui tests + synthetic input + process liveness; the user can eyeball it with `npm run native`.
- Cargo staleness hazard (fixed): rsync -a preserved old mtimes, so cargo kept stale artifacts after mirror pull/push; mirror.sh now copies by checksum with fresh mtimes. If tests look inexplicable, `touch` sources or `cargo clean -p <crate>`.
- Real devices, Windows, Linux hardware, Safari/Firefox, VoiceOver/NVDA/TalkBack, notarization and store signing cannot be exercised on this
  machine (macOS arm64, Chrome 155, Xcode with iOS simulators; Android SDK present but no NDK/JDK; Docker present). They stay unqualified in the ledger.
- 2026-10-09T05:01Z

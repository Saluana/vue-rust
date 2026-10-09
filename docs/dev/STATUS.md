# Project status: vue-native (living file)

Last updated: 2026-10-09 08:12 PDT. Paused at a clean point at 08:12 PDT (no work package was running then). Scope source of truth: `planning/vue-native-framework/{requirements,design,tasks}.md`. Start with `docs/dev/HANDOFF.md` (state, traps, commands, where to begin). This file keeps the binding decisions, what is built, and the honest limits. `docs/dev/archive/` holds an old working log (history only).

## Progress-ledger rule
After every accepted WP / milestone: edit `docs/dev/tasks-progress.json` (task id -> status done|partial|blocked|notstarted, summary, evidence command + artifacts, missing) and run `node tooling/status/update-tasks.mjs` (idempotent; `--check` verifies). A box is ticked only for status `done` (acceptance met + test command + artifact; no commits exist). Currently 9 done / 24 partial / 7 blocked / 30 not started.

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

## Scoreboards (2026-10-09 08:12 PDT)
- Rust workspace: 1319 pass, 2 fail (`vn-ui` `cascade_corpus`; the computed-style work stopped mid-way).
- Compiler harness: language scenarios 79/79, lowering 164/164, multi-file projects 10/10, real SFC fixtures lowered and `cargo check`ed 37/37.
- Vue runtime vs real vue@3.5.43 (`--with-vue`): 102/102.
- Components vs real Vue recorded in Chrome (`--with-vue --ui`, 160 scenarios): 131 pass, 29 blocked, 0 fail. Blocked causes and fix locations: `docs/dev/HANDOFF.md` section 2.
- CLI (`tests/cli`): 38/38. Browser host (`tests/platform/web`): 293 pass, 0 fail, 105 skipped, each with a stated reason.
- `tasks.md`: 9 done, 25 partial, 7 blocked by hardware, 29 not started (generated from `docs/dev/tasks-progress.json`).
- Test notes: `tests/cli/headless.test.mjs` strips the scoped-style stamps (`data-v-<hash>`, `data-v-app`) from snapshots before comparing and asserts that they exist; its Quick Quote interaction sequence runs on the real `defineModel` example and on a fixture without `defineModel`. `tests/platform/web/e2e-host-only.test.mjs` builds its generated crate in a private target subdirectory (a shared target dir produced duplicate `vn_runtime` artifacts).

## What is built (accepted work packages)
| WP | What | Evidence |
|---|---|---|
| G0A | prototype fixtures + JS baseline + KNOWN-BUG regressions + claims ledger | `tests/fixtures/prototype/**`, `docs/evidence/baseline/` — 21+53 tests pass |
| G0D | Rust baseline of prototypes (counter 4 tests, Quote 4 tests isolated, native+wasm builds) | `docs/evidence/baseline/rust-*`; counter tests re-run |
| G0B | compat manifest (126 rows, 446 entries incl. 52 native-tag, 171 vue exports; 40 compilerInternal helpers) + corrections applied | `compiler/profile/**`, `tests/profile/` 52 tests; verified by direct API queries |
| G0C | sandbox-exec runner + hostile fixtures + dependency policy | `tooling/sandbox`, `tests/security/build-isolation` 27 tests |
| L1 | `vn-value` JsStr + number semantics vs V8 corpora (25k + 15k cases) | `crates/value`, 58 tests, corpora regenerate byte-identical |
| O1 | Node oracle (trace format v1) + 77 scenarios + goldens | `tooling/oracle`, `tests/language` 53 tests |
| C1a | diagnostics, parser (js/ts/jsx/tsx), 55-rule profile checker, scope/closure analysis | `compiler/{diagnostics,frontend,analysis}`, 201 tests; V8 TDZ cross-check: 0 false negatives |
| X1 | headless Chrome CDP oracle runner (zero deps) | `tooling/chrome`, 54 tests; layout smoke verified by maintainer |
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
| C1b | **JS/TS -> HIR -> Rust compiler core + differential harness**: 153/153 lowering scenarios and 36 O1 scenarios pass vs Node goldens (clean rebuild + negative control verified by maintainer); 43 O1 scenarios blocked (Array builtins 13, async 17, Map 2, ...) | `compiler/{hir,emit,index.mjs}`, `tooling/harness`, `tests/language/status.json` ratchet |
| B2 | Array (ctor, isArray/of/from, 36 prototype methods, iterators; 4,862-case V8 corpus zero failures, 51 unit tests) | `builtins/array.rs`, ABI §16 |
| B6 | JSON.parse/stringify (413 corpus cases replay clean, 45 unit tests) | `builtins/json.rs` |
| B4 | Map/Set + iterators (1,080 V8 corpus cases clean; 55 unit tests; testkit Map/Set encode/decode; Node cross-check 43 graphs) | `builtins/map_set.rs`; vn-value+vn-testkit 786 pass |
| R1a | `vn-reactivity` core: Dep/Link graph, effects, batch, computed, EffectScope, refs, watch base (88 tests; 76 snippets match real @vue/reactivity; mutants killed) | `crates/reactivity`, `tests/vue/rust/` |
| RT-A | `vn-runtime` data layer: shape/patch flags, VNode + block tracking, createVNode family, normalizeClass/Style, toDisplayString, renderList/mergeProps/etc.; 947 helper cases + 420 vnode scenarios match real vue@3.5.43 | `crates/runtime/src/{shapes,vnode,normalize,helpers,exports}.rs`, `tests/ui/runtime-a/` |
| B5 | Promise/AggregateError/unhandled-rejection tracking + async runtime (`new_frame/frame_get/frame_set/async_start/await_value`); 403 promise programs equal Node, mutation check kills 101 | `builtins/promise`, `tests/language/promise-programs` |
| C1c | async/await lowering (heap frames + Rust async blocks), multi-module projects (`compileProject`), native-module imports, `.vue` imports, TS erasure: O1 79/79, lowering 164/164, projects 10/10, 37/37 real SFC fixtures lower to Rust that passes cargo check, GC-stress identical | `compiler/project`, `tooling/harness/{run-projects,check-sfc-lowering}.mjs` |
| RT-B | `vn-runtime` instance record, exact 3.5 scheduler (+nextTick over real promises), watch/watchEffect*, error routing, lifecycle hooks, provide/inject; 38 tests; with R1a/R1b: V0 102/102 | `crates/runtime/src/{instance,scheduler,watch,errors,lifecycle,inject}.rs` |
| V1 | UI oracle: real vue@3.5.43 prod build in headless Chrome + `testHost` + 160 component scenarios/goldens (static, conditional, list, events, model, props, emits, slots, provide-inject, lifecycle, refs, watch, errors, dynamic, teleport, transition, fragments, directives) | `tooling/oracle/ui`, `tests/ui/{cases,golden}`, 26 tests; regeneration byte-identical; see docs/dev/ui-scenario-host.md |
| R1b | reactive/readonly/shallow* proxies (Exotic), array + Map/Set instrumentation, toRaw/markRaw/is*: 240 tests, 152 oracle snippets, V0 102/102; follow-up done: Exotic prevent_extensions/is_extensible hooks | `crates/reactivity/src/{reactive,handlers,array_inst,collection_inst}.rs` |
| RT-D | DOM layer: `DocHost` (Host over vn-dom, re-entrant events), patchProp decision tree, event invokers + JS event object, v-model*/v-show, withModifiers/withKeys; 944 patchProp cases + 174 directive scenarios recorded from real Vue in Chrome; follow-up: svg fill + 1 skipped case | `crates/runtime/src/{host_doc,dom_props,dom_events,vmodel,vshow}.rs`, `tests/ui/runtime-d` |
| W1a | browser host: `WebHost` (wasm32, 44 imports, 38 KB wasm / 13.6 KB gz) + hand-written JS glue + build script; 293 tests pass, 0 fail, 105 skipped (outside Host surface); policy refusals for script/iframe/on*/innerHTML | `crates/renderer-dom`, `hosts/web`, `tooling/web`, `tests/platform/web` |
| RT-C | renderer (mount/patch/unmount/move, keyed diff + LIS, fragments), components (setup, props, emits, slots, attrs fallthrough, lifecycle, render-error comment), refs, directives, createApp/mount/use/component/directive/provide; 21+8+4 scenarios recorded from real Vue in Chrome; run against the 160 real-Vue component goldens (131 pass, 29 blocked)| `crates/runtime/src/{renderer,keyed,component,props,emits,slots,public_instance,setup_helpers,refs,directives,app}.rs` |
| APP1 | `vue-native` CLI (check/build/create/doctor) + web packaging + headless target: **Counter demo runs end to end in the Rust Vue runtime** (mount, click, v-if, reset, disabled); web build of `.vue` projects not wired yet (the renderer is Host-generic since RT-C r2); Quick Quote (defineModel) passes headless since RT-C r2 (38/38 CLI tests at 08:12) | `tooling/cli`, `examples/`, `tests/cli`, `docs/guide/getting-started.md`; 39/40 tests |
| T2 | `vn-uikit` Rust UI testHost (mount/find/click/input/... over DocHost, snapshot encoder) + `--with-vue --ui` harness: **131/160 real-Vue component scenarios pass, 29 blocked with classified causes, 0 fail** | `crates/uikit`, `tooling/harness/ui-mode.mjs`, `tests/ui/status.json` |
| RT-C r2 | renderer made Host-generic (`HostProvider`/`dyn Host`), useModel/mergeModels/mergeDefaults/createPropsRestProxy, scoped style ids, vnode field accessors, lazy component effects | runtime 18/18 test binaries, V0 102/102 |
| L2a | `vn-value` kernel: values, generation-checked heap, iterative mark-sweep GC at depth 0, object model, exotic hooks, functions/cells/TDZ, errors, jobs, watchdog | `crates/value`, 232 tests, mutation checks; ABI in `crates/value/ABI.md` |

## Open requests against built code (apply when the component scoreboard shows they matter)
- vnode.rs (RT-A): expose vnode fields `type`, `appContext`, `ce` as JS-readable props (RT-C currently defines data properties before instance creation); `normalize_ref` should set owner `i` = current rendering instance (template refs in slot content).
- R1a: export a lazily-run ReactiveEffect + `allowRecurse` control for component effects (component does not re-run for its own render mutations).
- (Landed in RT-C round 2: useModel, mergeModels, mergeDefaults, createPropsRestProxy.) Still missing in RT-C: block-stack unwinding in withCtx/render errors; keyed move-count comparison.

## Known deviations / honest limits
- Native window visuals cannot be screenshotted from this environment (`screencapture`: no Screen Recording permission; never use osascript/System Events — it triggers a permission prompt). Native correctness is verified with headless egui tests + synthetic input + process liveness; the user can eyeball it with `npm run native`.
- Cargo staleness hazard (fixed): rsync -a preserved old mtimes, so cargo kept stale artifacts after mirror pull/push; mirror.sh now copies by checksum with fresh mtimes. If tests look inexplicable, `touch` sources or `cargo clean -p <crate>`.
- Real devices, Windows, Linux hardware, Safari/Firefox, VoiceOver/NVDA/TalkBack, notarization and store signing cannot be exercised on this
  machine (macOS arm64, Chrome 155, Xcode with iOS simulators; Android SDK present but no NDK/JDK; Docker present). They stay unqualified in the ledger.

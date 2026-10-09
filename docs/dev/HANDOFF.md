# HANDOFF — read this first

**vue-native** compiles Vue single-file components (JavaScript/TypeScript, Composition API) through Rust to native binaries and WebAssembly, with no embedded JavaScript engine. Requirements, design and task list: `planning/vue-native-framework/{requirements,design,tasks}.md`.

Work was paused (2026-10-09, 08:12 PDT) at a clean point: nothing was running and nothing is committed. The numbers below were re-run at that moment; if the repo has changed since, they may have moved (see the first bullet of section 3).

## 1. Where to look
1. This file: state, environment, traps, commands, where to begin.
2. `docs/dev/STATUS.md`: binding decisions, what is built and where, known gaps, honest limits.
3. `docs/dev/ARCHITECTURE.md` (architecture decisions) and `docs/dev/AGENT-CONTRACT.md` (engineering rules: no fabricated results, independent oracles, no JS engine at runtime, loud diagnostics for anything unsupported).
4. `docs/dev/wp/WP-*.md`: one brief per area (what to build, owned paths, acceptance commands). Read the brief before touching an area. The briefs were written for several people working in parallel: ignore their instructions about reports and private working copies (`scripts/mirror.sh`) and work in the repo directly. `docs/dev/wp/BUILTINS-COMMON.md` explains the layout of the built-ins.
5. `planning/vue-native-framework/tasks.md`: the task list. Its progress notes are generated (section 6).
Reference: `docs/dev/trace-format.md` (differential-test trace format), `docs/dev/ui-scenario-host.md`, `docs/dev/builtin-corpus.md`, `crates/value/ABI.md` (runtime ABI), `hosts/web/ABI.md` (browser host ABI). `docs/dev/archive/` holds an old working log: history only, safe to ignore.

## 2. Current state
| Check | Result |
|---|---|
| Rust workspace (`cargo test --workspace --no-fail-fast`) | 1319 pass, 2 fail. Both failures are in `vn-ui` `tests/cascade_corpus.rs` (`cascade_fixtures_match_chrome`, `cascade_checker_detects_wrong_expectations`): the computed-style work stopped mid-way. Every other crate is green. |
| JS/TS to Rust compiler: language scenarios / lowering / multi-file projects / real SFC fixtures | 79/79, 164/164, 10/10, 37/37 |
| Vue runtime vs real vue@3.5.43: reactivity, watch, scheduler (`--with-vue`) | 102/102 |
| Components vs real Vue recorded in Chrome (`--with-vue --ui`, 160 scenarios) | 131 pass, 29 blocked, 0 fail |
| CLI tests (`tests/cli`) | 38/38 |
| Browser host tests (`tests/platform/web`) | 398 tests: 293 pass, 0 fail, 105 skipped, each with a stated reason (corpus cases that need Host operations that do not exist yet) |
| `tasks.md` | 9 done, 25 partial, 7 blocked by hardware, 29 not started |

The 29 blocked component scenarios, by cause (recorded in `tests/ui/status.json`; the runner fails if a blocked scenario starts passing until you flip the ledger with `--write-status`):
- Teleport, Transition, KeepAlive and `defineAsyncComponent` are not implemented: 17 (`teleport/*`, `transition/*`, `dynamic/*`).
- `reactive()` wraps DOM element handles: 7 (`refs/*`, `lifecycle/mounted-template-ref`, `watch-in-component/flush-post-sees-dom`). Fix in `crates/reactivity/src/reactive.rs`: Vue only proxies plain objects, arrays and Map/Set/WeakMap/WeakSet (`getTargetType`); anything else, including host objects, is returned as is.
- `vn-dom` lacks style shorthands such as `margin`: 2 (`static/class-style-merge`, `props/attrs-class-style-merge`). Fix in `crates/dom/src/style.rs`.
- Errors thrown in DOM listeners skip `handleError`: 3 (`errors/event-handler-error`, `errors/async-handler-rejection`, `errors/return-false-stops`). Pass the owning instance to `patch_event` in `crates/runtime/src/dom_events.rs`.

Built and passing: the oracles (Node/V8, real Vue 3.5.43 recordings, headless Chrome); the `vn-value` runtime (kernel, GC, built-ins, ESM modules, Promises/async); the compiler front and back halves and the SFC frontend; `vn-css`, `vn-dom`; `vn-reactivity`; `vn-runtime` (vnodes, scheduler, watch, errors, lifecycle, inject, renderer, components, props/emits/slots, directives, `createApp`, `DocHost`); `vn-uikit` (a Rust UI test host); the browser host (`vn-renderer-dom` + `hosts/web`: 44 wasm imports, about 38 KB wasm, no wasm-bindgen); the `vue-native` CLI (`check`, `build`, `create`, `doctor`) with a headless target. The Counter and Quick Quote demos in `examples/` mount, take clicks and input, and pass full interaction scripts headless in the Rust Vue runtime (Quick Quote uses `defineModel`). Per-package detail: `docs/dev/STATUS.md`.

Not done: `vue-native build --target web` for `.vue` projects; Teleport/Transition/KeepAlive/async components; the native window renderer (style, layout, egui) and accessibility; SSR/hydration; services (files, settings, SQLite, HTTP); TypeScript type-check; packaging and signing; the Fieldboard demo; benchmarks and fuzzing; the README rewrite; the final audit against R01-R26.

## 3. What you need to know
- **Check that you are the only one working in the repo.** Before editing, list recently changed files: `find . -type f -mmin -5 -not -path './target/*' -not -path './node_modules/*' -not -path './toolchain/*'`. If files change that you did not edit, stop and tell the user before continuing. Never revert or overwrite someone else's changes.
- **Machine**: macOS arm64 (M4 Pro), Node 26.5, project-local Rust 1.99.0 in `./toolchain` plus the `wasm32-unknown-unknown` target. `source scripts/env.sh` puts it on `PATH`; never touch `~/.cargo`. Headless Chrome 155 is the browser oracle (`tooling/chrome`, a zero-dependency CDP client). Xcode with iOS simulators is installed; the Android SDK is there without NDK/JDK; Docker is installed but unused.
- **How results are judged**: expected values come from independent oracles (Node/V8 for language semantics; the real `vue@3.5.43`, in Node and in headless Chrome, for Vue behaviour), never from the code under test. Per-scenario ratchet ledgers: `tests/language/status.json`, `tests/vue/status.json`, `tests/ui/status.json`. Re-run what you change, add a negative control (a wrong expectation that must fail), and never weaken a test to get green.
- **Disk** is nearly full and shared with the user's other jobs (never touch `/tmp/or3-*`). Give each task its own `CARGO_TARGET_DIR`, never share one between same-named crates, and delete it when the task is done. `target/review` is the cargo cache for the commands in section 4.
- **Git**: the project has a `.git` (created 2026-10-09 06:50; origin `https://github.com/Saluana/vue-rust.git`; branch `main`; one commit, `95a2b0d`, by the user). About 233 uncommitted entries are all of this work. Do not commit, push, `git init` or change git config unless the user asks.
- **Ask the user first** before installing new npm packages beyond the pinned set (for example `typescript`, `vue-tsc`), Docker images, the Android NDK/JDK, Playwright browsers, or `rustup target add` for iOS/Android/Windows/Linux. Already approved: the project-local Rust toolchain, `npm ci`, `vue@3.5.43`, and crates.io fetches that builds need.
- **Cannot be tested on this machine**: real devices, Windows/Linux hardware, Safari/Firefox, screen readers (VoiceOver/NVDA/TalkBack), notarization and store signing, and screenshots of native windows (`screencapture` fails: no Screen Recording permission; `osascript`/System Events hangs on a permission prompt, so do not use it). Verify native behaviour with headless tests. Record these as unverified; never mark them done.
- **Traps already hit**:
  - (a) Cargo trusts file mtimes. After copying files with old mtimes it can serve stale artifacts and make real code look missing. If results look impossible: `touch $(find crates -name '*.rs')` or `cargo clean -p <crate>`.
  - (b) A crate directory with a `Cargo.toml` but no `src/lib.rs` breaks cargo for the whole workspace (it globs `crates/*`).
  - (c) Node 26's `node --test <dir>` does not recurse. Use quoted globs: `node --test "tests/cli/*.test.mjs"`.
  - (d) One cargo target dir shared by workspace-context wasm builds (`tooling/web/build-wasm.mjs`, `-p vn-renderer-dom`) and generated-crate builds (`tooling/cli/build.mjs`) can leave two incompatible `vn_runtime` artifacts: errors such as "multiple different versions of crate `vn_runtime`" and "no method named ... found for struct `WebHost`", although `cargo tree -d` shows one copy. A clean target dir builds fine. `tests/platform/web/e2e-host-only.test.mjs` therefore builds in its own subdirectory; run `tests/cli` and `tests/platform/web` each with a fresh private `CARGO_TARGET_DIR` (section 4).
  - (e) The runtime stamps elements of `<style scoped>` components with `data-v-<hash>` and the mount container with `data-v-app`, like real Vue. `tests/cli/headless.test.mjs` strips the stamps before comparing markup (and asserts they exist).

## 4. Commands
```
cd /Users/brendon/Documents/vue-rust-lab && source scripts/env.sh
export CARGO_TARGET_DIR="$VN_ROOT/target/review"
cargo test --workspace --no-fail-fast                  # 1319 pass, 2 fail (vn-ui cascade_corpus)
node tooling/harness/run-language.mjs                  # 79/79 language scenarios vs Node goldens
node tooling/harness/run-language.mjs --dir tests/language/lowering          # 164/164
node tooling/harness/run-language.mjs --with-vue       # 102/102 vs real Vue (ledger: tests/vue/status.json)
node tooling/harness/run-language.mjs --with-vue --ui  # 131 pass / 29 blocked / 0 fail (ledger: tests/ui/status.json)
node tooling/status/update-tasks.mjs --check           # tasks.md in sync with docs/dev/tasks-progress.json
```
Then each of these with a fresh private target dir (trap d), and delete the dirs afterwards:
```
export CARGO_TARGET_DIR="$VN_ROOT/target/verify-cli"; rm -rf "$CARGO_TARGET_DIR"; node --test "tests/cli/*.test.mjs"            # 38/38
export CARGO_TARGET_DIR="$VN_ROOT/target/verify-web"; rm -rf "$CARGO_TARGET_DIR"; node --test "tests/platform/web/*.test.mjs"  # 293 pass / 0 fail / 105 skipped
rm -rf "$VN_ROOT"/target/verify-*
```
Other useful commands:
```
node tooling/harness/run-language.mjs [--dir DIR] [--filter TEXT] [--write-status]   # harness options; --write-status flips ledgers
node --test "tests/<dir>/*.test.mjs"                  # JS suites: profile, compiler/**, language, vue, ui, cli, platform/web, ...
node tooling/cli/vue-native.mjs check examples/counter
node tooling/oracle/check-goldens.mjs                 # oracle goldens are reproducible
node tooling/oracle/vue-env.mjs                       # real Vue oracle environment
```

## 5. Where to begin
1. **Verify the baseline** with section 4. If a number differs, find out why before changing anything.
2. **Close the cheap blocked component scenarios** (12 of the 29; causes and fix locations are in section 2). See them with `node tooling/harness/run-language.mjs --with-vue --ui --filter refs/`. After the fixes flip the ledger: `node tooling/harness/run-language.mjs --with-vue --ui --write-status`, then update `docs/dev/tasks-progress.json`.
3. **Web target for `.vue` projects** (`docs/dev/wp/WP-APP1-cli-web-demo.md`). `vue-native build --target web` still refuses them on purpose; the test "build --target web: a .vue project is refused with the reason" in `tests/cli/build.test.mjs` encodes that, so change it together with the feature. The renderer is Host-generic now (`HostProvider` / `dyn Host` in `crates/runtime`), so wire `WebHost` (`crates/renderer-dom`) and a generated `vn_start` entry (see `buildWebApp` in `tooling/cli/build.mjs`; `tests/cli/fixtures/host-only/lib.rs` is a working host-only example), then add a headless-Chrome end-to-end test of `examples/counter` and `examples/quick-quote` next to `tests/platform/web/e2e-host-only.test.mjs`. This is the first "it works in a browser" result.
4. **Teleport, Transition/TransitionGroup, KeepAlive, `defineAsyncComponent`** (the 17 blocked scenarios; stubs in `crates/runtime/src/{teleport,transition,keep_alive}.rs`). Write a brief in the style of `docs/dev/wp/WP-RT-C-renderer-components.md` first.
5. **Native lane**: finish `vn-ui` computed style (the 2 failing tests; brief `docs/dev/wp/WP-U1b-style.md`), then layout (Taffy), inline text, display list, an egui/eframe adapter that uses `DocHost` as the document, and an AccessKit tree. Test headlessly (no screenshots here).
6. **The rest of `tasks.md`**: services, SSR and hydration, TypeScript type-check (ask first: needs `typescript`), Fieldboard, security fuzzing, benchmarks, rewrite the root `README.md` (it still describes the old prototype), `docs/decisions/architecture-gate.md` (the G1/G2 decision with the evidence above), final audit against R01-R26 in `requirements.md`.

## 6. Keeping the task list honest
`planning/vue-native-framework/tasks.md` progress notes are generated from `docs/dev/tasks-progress.json` by `node tooling/status/update-tasks.mjs` (idempotent; `--check` verifies). After each finished piece, edit the JSON (status `done|partial|blocked|notstarted`, summary, evidence command and artifacts, what is missing) and rerun the script. A box is ticked only for `done`: acceptance met, with a passing test command and artifact linked. Keep `docs/dev/STATUS.md` current as well.

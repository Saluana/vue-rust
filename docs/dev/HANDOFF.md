# HANDOFF — read this first

**vue-native** compiles Vue single-file components (JavaScript/TypeScript, Composition API) through Rust to native binaries and WebAssembly, with no embedded JavaScript engine. Requirements, design and task list: `planning/vue-native-framework/{requirements,design,tasks}.md`.

Historical handoff from 2026-10-09 08:12 PDT. Work was subsequently reviewed, committed and pushed as 05987fa. Resume milestones: all 12 smaller component blockers cleared and compiled browser demos verified. Current acceptance after cleanup: all 160 component scenarios and all 1323 Rust tests pass, with no compiler warnings. Three print-only probes were removed from the former 1326 Rust total; independent corpus checks remain. Cleanup evidence: docs/evidence/cleanup.md. Evidence: docs/evidence/advanced-components.md (earlier milestones: resume-components.md and resume-web-apps.md). The main checks below were re-run during the resume; project/SFC fixture counts are retained from the earlier checkpoint.

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
| Rust workspace (`cargo test --workspace --no-fail-fast`) | 1323 pass, 0 fail. Both former `vn-ui` cascade failures are fixed, with unchanged Chrome expectations. |
| JS/TS to Rust compiler: language scenarios / lowering / multi-file projects / real SFC fixtures | 79/79, 166/166, 10/10, 37/37 |
| Vue runtime vs real vue@3.5.43: reactivity, watch, scheduler (`--with-vue`) | 102/102 |
| Components vs real Vue recorded in Chrome (`--with-vue --ui`, 160 scenarios) | 160 pass, 0 blocked, 0 fail |
| CLI tests (`tests/cli`) | 40/40 |
| Browser host tests (`tests/platform/web`) | 408 tests: 299 pass, 0 fail, 109 skipped, each with a stated reason (corpus cases that need Host operations that do not exist yet) |
| `tasks.md` | 9 done, 26 partial, 7 blocked by hardware, 28 not started |

The component ledger has 160 passing scenarios and no blockers. Teleport, Transition, KeepAlive and defineAsyncComponent now match all 17 previously blocked recordings; the complete corpus also passes with GC at every safepoint. Evidence: docs/evidence/advanced-components.md. Earlier refs/listener/style repairs are in docs/evidence/resume-components.md.

Web builds of .vue projects now work; Counter and Quick Quote pass browser interaction and resource-lifetime tests. Evidence: docs/evidence/resume-web-apps.md. The browser replay has four additional explicit skips because the new priority cases use stylePriority, outside the Host ABI; those cases pass in vn-dom's Chrome corpus.

Built and passing: the oracles (Node/V8, real Vue 3.5.43 recordings, headless Chrome); the `vn-value` runtime (kernel, GC, built-ins, ESM modules, Promises/async); the compiler front and back halves and the SFC frontend; `vn-css`, `vn-dom`; `vn-reactivity`; `vn-runtime` (vnodes, scheduler, watch, errors, lifecycle, inject, renderer, components, props/emits/slots, directives, `createApp`, `DocHost`); `vn-uikit` (a Rust UI test host); the browser host (`vn-renderer-dom` + `hosts/web`: 45 wasm imports, about 38 KB wasm, no wasm-bindgen); the `vue-native` CLI (`check`, `build`, `create`, `doctor`) with a headless target. The Counter and Quick Quote demos in `examples/` mount, take clicks and input, and pass full interaction scripts headless and in Chrome through the Rust Vue runtime (Quick Quote uses `defineModel`). Per-package detail: `docs/dev/STATUS.md`.

Not done: TransitionGroup/in-out mode/async retry and broader advanced-component qualification; the native window renderer (style, layout, egui) and accessibility; SSR/hydration; services (files, settings, SQLite, HTTP); TypeScript type-check; packaging and signing; the Fieldboard demo; benchmarks and fuzzing; the README rewrite; the final audit against R01-R26.

## 3. What you need to know
- **Check that you are the only one working in the repo.** Before editing, list recently changed files: `find . -type f -mmin -5 -not -path './target/*' -not -path './node_modules/*' -not -path './toolchain/*'`. If files change that you did not edit, stop and tell the user before continuing. Never revert or overwrite someone else's changes.
- **Machine**: macOS arm64 (M4 Pro), Node 26.5, project-local Rust 1.99.0 in `./toolchain` plus the `wasm32-unknown-unknown` target. `source scripts/env.sh` puts it on `PATH`; never touch `~/.cargo`. Headless Chrome 155 is the browser oracle (`tooling/chrome`, a zero-dependency CDP client). Xcode with iOS simulators is installed; the Android SDK is there without NDK/JDK; Docker is installed but unused.
- **How results are judged**: expected values come from independent oracles (Node/V8 for language semantics; the real `vue@3.5.43`, in Node and in headless Chrome, for Vue behaviour), never from the code under test. Per-scenario ratchet ledgers: `tests/language/status.json`, `tests/vue/status.json`, `tests/ui/status.json`. Re-run what you change, add a negative control (a wrong expectation that must fail), and never weaken a test to get green.
- **Disk** is nearly full and shared with the user's other jobs (never touch `/tmp/or3-*`). Give each task its own `CARGO_TARGET_DIR`, never share one between same-named crates, and delete it when the task is done. Section 4 allocates private directories and removes only those directories after verification.
- **Git**: the project has a `.git` (created 2026-10-09 06:50; origin `https://github.com/Saluana/vue-rust.git`; branch `main`; review checkpoint `05987fa`). The user requested committing and pushing the verified resume and cleanup milestones; consult `git log` for the latest checkpoint. Do not commit, push, `git init` or change git config unless the user asks.
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
vn_verify_rust_target=$(mktemp -d "$VN_ROOT/target/verify-rust-XXXXXX")
export CARGO_TARGET_DIR="$vn_verify_rust_target"
cargo test --workspace --no-fail-fast                  # 1323 pass, 0 fail
node tooling/harness/run-language.mjs                  # 79/79 language scenarios vs Node goldens
node tooling/harness/run-language.mjs --dir tests/language/lowering          # 166/166
node tooling/harness/run-language.mjs --with-vue       # 102/102 vs real Vue (ledger: tests/vue/status.json)
node tooling/harness/run-language.mjs --with-vue --ui  # 160 pass / 0 blocked / 0 fail (ledger: tests/ui/status.json)
node tooling/harness/run-language.mjs --with-vue --ui --gc-stress # same corpus with GC at every safepoint
node tooling/status/update-tasks.mjs --check           # tasks.md in sync with docs/dev/tasks-progress.json
rm -rf -- "$vn_verify_rust_target"
```
Then each of these with a fresh private target dir (trap d), and delete the dirs afterwards:
```
vn_verify_cli_target=$(mktemp -d "$VN_ROOT/target/verify-cli-XXXXXX")
export CARGO_TARGET_DIR="$vn_verify_cli_target"
node --test "tests/cli/*.test.mjs"                    # 40/40
rm -rf -- "$vn_verify_cli_target"

vn_verify_web_target=$(mktemp -d "$VN_ROOT/target/verify-web-XXXXXX")
export CARGO_TARGET_DIR="$vn_verify_web_target"
node --test "tests/platform/web/*.test.mjs"           # 299 pass / 0 fail / 109 skipped
rm -rf -- "$vn_verify_web_target"
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
2. **Completed:** the 12 smaller component blockers; UI ratchet is 160 pass / 0 blocked / 0 fail / 0 unexpected.
3. **Completed:** web target for .vue projects. Counter and Quick Quote execute in Chrome, errors are visible, and repeated editor unmount/recreation does not grow host handles or listener counts. Browser tests: 299 pass / 109 explicit skips / 0 fail; CLI: 40/40.
4. **Recorded advanced-component scenarios completed:** 160/160 normal and GC stress. Brief: `docs/dev/wp/WP-RT-E-advanced-components.md`; evidence: `docs/evidence/advanced-components.md`. Remaining qualification: TransitionGroup, in-out, async retries/hydration, nested composition and races.
5. **Native lane**: `vn-ui` recorded computed-style corpora now pass (brief `docs/dev/wp/WP-U1b-style.md`); continue with layout (Taffy), inline text, display list, an egui/eframe adapter that uses `DocHost` as the document, and an AccessKit tree. Test headlessly (no screenshots here).
6. **The rest of `tasks.md`**: services, SSR and hydration, TypeScript type-check (ask first: needs `typescript`), Fieldboard, security fuzzing, benchmarks, rewrite the root `README.md` (it still describes the old prototype), `docs/decisions/architecture-gate.md` (the G1/G2 decision with the evidence above), final audit against R01-R26 in `requirements.md`.

## 6. Keeping the task list honest
`planning/vue-native-framework/tasks.md` progress notes are generated from `docs/dev/tasks-progress.json` by `node tooling/status/update-tasks.mjs` (idempotent; `--check` verifies). After each finished piece, edit the JSON (status `done|partial|blocked|notstarted`, summary, evidence command and artifacts, what is missing) and rerun the script. A box is ticked only for `done`: acceptance met, with a passing test command and artifact linked. Keep `docs/dev/STATUS.md` current as well.

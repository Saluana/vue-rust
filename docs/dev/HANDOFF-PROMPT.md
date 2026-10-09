# Handoff prompt (paste into a new session started in /Users/brendon/Documents/vue-rust-lab)

You are continuing a paused project, **vue-native**: a framework that compiles Vue single-file components (JavaScript/TypeScript, Composition API) through Rust to native binaries and WebAssembly, with no embedded JavaScript engine. Requirements, design and task list are in `planning/vue-native-framework/`. The repo is `/Users/brendon/Documents/vue-rust-lab`. Work stopped at a clean point on 2026-10-09 08:12 PDT: everything passed except two known failing Rust tests, and nothing is committed.

## Where to look (in this order)
1. `docs/dev/HANDOFF.md`: current state with test numbers, environment, traps, commands, and where to begin. Read it fully before doing anything.
2. `docs/dev/STATUS.md`: binding decisions, what is built and where, known gaps, honest limits.
3. `docs/dev/ARCHITECTURE.md` and `docs/dev/AGENT-CONTRACT.md`: architecture decisions and engineering rules (no fabricated results, independent oracles, no JS engine at runtime, loud diagnostics for anything unsupported).
4. `docs/dev/wp/WP-*.md`: one brief per area (what to build, owned paths, acceptance commands). Read the brief before touching an area; skip its instructions about reports and private working copies.

## What you need to know
- Check that you are the only one working in the repo before editing (`find . -type f -mmin -5 -not -path './target/*' -not -path './node_modules/*' -not -path './toolchain/*'`). If files change that you did not edit, stop and tell the user. Never revert someone else's changes.
- Setup: `cd /Users/brendon/Documents/vue-rust-lab && source scripts/env.sh` (project-local Rust 1.99.0, Node 26, macOS arm64; headless Chrome is the browser oracle). Give each task its own `CARGO_TARGET_DIR` and delete it when done: the disk is nearly full and shared with other jobs (never touch `/tmp/or3-*`).
- Expected results come from independent oracles (Node/V8 and the real `vue@3.5.43`), never from the code under test. Re-run what you change; never weaken a test to get green.
- Run JS tests with quoted globs: `node --test "tests/cli/*.test.mjs"`. Run `tests/cli` and `tests/platform/web` each with a fresh private `CARGO_TARGET_DIR` (HANDOFF section 3, trap d).
- Do not commit, push or change git config unless the user asks. Ask the user before installing new npm packages (for example `typescript`), Docker images, the Android NDK/JDK, Playwright browsers, or extra `rustup` targets.
- Cannot be tested on this machine, so record as unverified rather than done: real devices, Windows/Linux, Safari/Firefox, screen readers, signing/notarization, native window screenshots. Do not use `osascript` or `screencapture`.
- Keep the task list honest: after each finished piece update `docs/dev/tasks-progress.json` and run `node tooling/status/update-tasks.mjs`; tick a box only with a passing test command and artifact linked. Keep `docs/dev/STATUS.md` current.

## Where to begin
1. Re-verify the baseline with the commands in HANDOFF section 4. Expected: Rust workspace 1319 pass / 2 fail (both in `vn-ui` `cascade_corpus`); language 79/79; lowering 164/164; `--with-vue` 102/102; `--with-vue --ui` 131 pass / 29 blocked / 0 fail; `tests/cli` 38/38; `tests/platform/web` 293 pass / 0 fail / 105 skipped with reasons; `tasks.md` in sync. If a number differs, find out why before changing anything.
2. Close the cheap blocked component scenarios (12 of the 29; causes and fix locations in HANDOFF sections 2 and 5): `reactive()` wrapping DOM element handles (7, `crates/reactivity/src/reactive.rs`), DOM-listener errors skipping `handleError` (3, `crates/runtime/src/dom_events.rs`), style shorthands such as `margin` (2, `crates/dom/src/style.rs`). Then flip the ledger: `node tooling/harness/run-language.mjs --with-vue --ui --write-status`.
3. Finish the web target for `.vue` projects (`docs/dev/wp/WP-APP1-cli-web-demo.md`): `vue-native build --target web` still refuses them on purpose; the renderer is Host-generic now, so wire `WebHost` and a `vn_start` entry and add a headless-Chrome end-to-end test of `examples/counter` and `examples/quick-quote`.
4. Then Teleport / Transition / KeepAlive / `defineAsyncComponent`, the native desktop lane, and the rest of `tasks.md`; order and details are in HANDOFF section 5.

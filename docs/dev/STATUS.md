# Project status: vue-native (living file)

Last updated: 2026-10-09. Resumed after review fixes committed as 05987fa. Scope source of truth: `planning/vue-native-framework/{requirements,design,tasks}.md`. Start with `docs/dev/HANDOFF.md` (state, traps, commands, where to begin). This file keeps the binding decisions, what is built, and the honest limits. `docs/dev/archive/` holds an old working log (history only).

## Progress-ledger rule
After every accepted WP / milestone: edit `docs/dev/tasks-progress.json` (task id -> status done|partial|blocked|notstarted, summary, evidence command + artifacts, missing) and run `node tooling/status/update-tasks.mjs` (idempotent; `--check` verifies). A box is ticked only for status `done` (acceptance met + test command + artifact). Currently 9 done / 26 partial / 7 blocked / 28 not started.

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

## Scoreboards (2026-10-09, cleanup verification)
- Rust workspace: 1323 pass, 0 fail, no compiler warnings. Three assertion-free debug probes were removed from the former 1326 total. Both former `vn-ui` cascade failures are repaired; unchanged Chrome goldens and negative controls pass.
- Compiler harness: language scenarios 79/79, lowering 166/166, multi-file projects 10/10, real SFC fixtures lowered and `cargo check`ed 37/37.
- Vue runtime vs real vue@3.5.43 (`--with-vue`): 102/102.
- Components vs real Vue recorded in Chrome (`--with-vue --ui`, 160 scenarios): 160 pass, 0 blocked, 0 fail, 0 unexpected; also 160/160 with GC at every safepoint. Evidence: `docs/evidence/advanced-components.md`.
- CLI (`tests/cli`): 40/40. Browser host and compiled apps (`tests/platform/web`): 299 pass, 0 fail, 109 skipped, each with a stated reason.
- `tasks.md`: 9 done, 26 partial, 7 blocked by hardware, 28 not started (generated from `docs/dev/tasks-progress.json`).
- Test notes: `tests/cli/headless.test.mjs` strips the scoped-style stamps (`data-v-<hash>`, `data-v-app`) from snapshots before comparing and asserts that they exist; its Quick Quote interaction sequence runs on the real `defineModel` example and on a fixture without `defineModel`. `tests/platform/web/e2e-host-only.test.mjs` builds its generated crate in a private target subdirectory (a shared target dir produced duplicate `vn_runtime` artifacts).

## What is built now

| Area | Implementation and verified behavior | Evidence |
|---|---|---|
| Value runtime | JS values, GC, operators, objects, built-ins, ESM modules, Promises and async frames; no embedded JS engine. | `crates/value`, `crates/value/ABI.md`; Rust workspace and independent Node corpora. |
| Compiler and SFC frontend | Official Vue compiler output lowered through JS/TS analysis and HIR into Rust; modules, async lowering, TS erasure and scoped styles. | `compiler/`, `tooling/harness/`; language 79, lowering 166, projects 10 and SFC fixtures 37. |
| Reactivity | Effects, batching, computed, scopes, refs, reactive/readonly proxies, arrays and collection instrumentation. | `crates/reactivity`; 102/102 real-Vue core scenarios. |
| Components and renderer | Host-generic mount/patch/unmount, keyed diff, props/emits/slots, lifecycle, refs, errors, directives and app APIs. Teleport, Transition, KeepAlive and async components pass the recorded corpus; task 6.6 remains partial for broader qualification. | `crates/runtime`, `crates/uikit`; 160/160 UI scenarios, also under GC stress; [advanced acceptance](../evidence/advanced-components.md). |
| DOM and CSS | DOM tree, form state, events, selectors, styles and serialization; CSS parsing/cascade/custom properties, typed values, shorthands and computed style. Layout and native painting remain unbuilt. | `crates/dom`, `crates/css`, `crates/ui`; independent Chrome corpora and negative controls. |
| Browser delivery | `WebHost`, 45 WASM imports, handwritten JS glue, host events and traced timers; compiled `.vue` projects run in Chrome. | `crates/renderer-dom`, `hosts/web`, `tooling/web`; browser suite 299 pass / 109 documented skips; [web apps](../evidence/resume-web-apps.md). |
| CLI and demos | `check`, `build`, `create`, `doctor`, headless/web packaging; Counter and Quick Quote pass interactions and repeated-unmount resource checks. | `tooling/cli`, `examples/`; 40/40 CLI tests and browser end-to-end tests. |
| Verification and policy | Node/V8 and real-Vue differential oracles, Chrome DOM/CSS corpora, ratchet ledgers, profile diagnostics, sandbox runner and dependency policy. | `tooling/oracle`, `tooling/chrome`, `tests/`, `compiler/profile`, `release/dependency-policy`. |

Earlier per-package receipts and resume checkpoints are in
[the acceptance archive](archive/accepted-work-packages-2026-10-09.md). Their
historical failures, blockers and counts do not describe the current build.

## Runtime qualification still pending
- Block-stack unwinding in withCtx/render errors and keyed move-count comparisons remain unqualified.
- Broader advanced-component and lifetime qualification is listed under task 6.6 below and in the progress ledger.

Earlier requests for readable VNode fields, rendering-instance ref ownership,
lazy component effects and effect/job recurse controls have implementations in
`vnode.rs` and `component.rs`; they are no longer listed as missing APIs.

## Known deviations / honest limits
- The framework native window renderer and accessibility adapter are not built. Headless DOM/CSS checks do not qualify native painting or controls. The older prototype was removed from the root (`npm run native` and `npm run build:demo` no longer exist); its preserved copy is in `tests/fixtures/prototype/`. Native screenshots are unavailable here (`screencapture`: no Screen Recording permission; never use osascript/System Events — it triggers a permission prompt).
- Cargo staleness hazard (fixed): rsync -a preserved old mtimes, so cargo kept stale artifacts after mirror pull/push; mirror.sh now copies by checksum with fresh mtimes. If tests look inexplicable, `touch` sources or `cargo clean -p <crate>`.
- Real devices, Windows, Linux hardware, Safari/Firefox, VoiceOver/NVDA/TalkBack, notarization and store signing cannot be exercised on this
  machine (macOS arm64, Chrome 155, Xcode with iOS simulators; Android SDK present but no NDK/JDK; Docker present). They stay unqualified in the ledger.

## Advanced components and CSS acceptance

All 160 existing real-Vue component recordings pass, including all 17 former
Teleport/Transition/KeepAlive/async blockers. The corpus also passes with GC at
every safepoint (`--gc-stress`). The compiled advanced-component browser test
checks actual CSS timing, cached state, Teleport events and stable handles,
listeners and timers after repeated teardown. Browser imports now include
`computed_style_property` (45 imports); generated apps install a traced timer
service over the browser host.

The CSS cascade corpus is green: form-control defaults, disabled/checkbox rules,
relative bold weights, replaced-element descendant CSSOM values and precise UA
font sizes are repaired. The corpus adapter now includes the oracle's head/body
sibling topology; expected values are unchanged. Workspace after cleanup: 1323 passed, zero
failed (three print-only probes removed). Compiler/profile 568; CLI 40; browser 299 passed / 109 documented skips;
language/lowering/Vue/UI 79/166/102/160. Evidence and limitations:
`docs/evidence/advanced-components.md`. Task 6.6 remains partial for
TransitionGroup, in-out mode, async retry/hydration and broader nested/race
qualification. Historical per-package receipts and earlier resume milestones are preserved
in the acceptance archive; the scoreboard above is current.

## Cleanup checkpoint

Removed three assertion-free debugging probes, retained the independent CSS
corpora and negative controls, and scoped the JSON boolean accessor to its
selftest feature. Named callback captures and expanded state/timing code improve
readability without changing stored values or GC tracing. Historical receipts
are archived; verification commands now allocate fresh directories and delete
only their own outputs. Evidence: [cleanup](../evidence/cleanup.md). The task totals and qualification limits are unchanged. The user requested
committing and pushing this checkpoint; consult `git log` for its commit.

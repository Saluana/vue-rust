# Reload probe: source-mapped runtime errors and the dev restart loop — 2026-10-10

Task 3.7 (ticket w37-reload), worktree /home/claude/wt/w37-reload, branch claude/w37-reload, CARGO_TARGET_DIR=/home/claude/targets/w37-reload. Nothing is committed. The pinned checker pair (TypeScript 6.0.3 with vue-tsc 3.3.12) is the one identified in docs/evidence/typescript-tooling.md; the dev loop runs the same checker.

## What was built

**Part A: errors from `.vue` code report the author's position.** The SFC frontend keeps compileScript's v3 source map and the offset where the script starts in the generated module (`mapOffset`). compiler/sourcemap.mjs decodes the mappings and finds a position by exact generated line and column. A map with several sources, or no exact segment, gives no position. The HIR records a site for each `throw`, each `await` and each function (compiler/hir/context.mjs, `siteOf`), and the emitter writes them to a `SITES` table per module. A throw passes its site, its enclosing function and, inside an async function, the last await site to `Rt::throw_at` (crates/value/src/error.rs). The runtime keeps the last throw record, matched by the identity of the thrown value. `Rt::source_frames` returns the throw frame and, when the error came through an await, the await frame. `error.stack` is not changed. The headless runner installs its own error sink, so the frames go into a separate report:

    app error: mount: Error: setup failed
        at setup (src/App.vue:5:0)

**Part B: `vue-native dev <project>`** (headless target, tooling/cli/dev.mjs).

- The sources are polled every 100 ms (`--poll-ms`). A change is handled after one quiet poll interval, so edits in progress are coalesced.
- Each change runs the check (`checkProject` in tooling/cli/commands.mjs, the same code as `vue-native check --target native`; the type checker runs unless `--no-types`), builds the headless app in the dev profile with `devState`, asks the running app for a snapshot, stops it, starts the new binary with the snapshot, and reports `ready` at the mounted reply.
- A failed check or build keeps the running app. Binaries alternate between `gen-0` and `gen-1` under `<target dir>/vue-native-dev/<hash of the project root>`, so a running binary is never overwritten.
- `devState` (a compileSfc option, forwarded by compileProject, compileAt and buildHeadlessApp) compiles `<script setup>` as Vue's development build does: compileScript without `inlineTemplate`. The setup function returns its bindings as `__returned__`, and the template is a separate render function that reads them as `$setup`. Two lines differ from Vue's output, both forced by the profile: the render function is named `_sfc_render`, and the `__isScriptSetup` marker is an assignment, because `Object.defineProperty` is excluded (profile row L04). The runtime reads that marker only for its truthiness (crates/runtime/src/public_instance.rs, `has_setup_binding`).
- tooling/cli/headless/state.rs saves the refs and reactive objects of the root's setup state, read raw through `toRaw`, as tagged plain data: numbers, strings, booleans, null, undefined, arrays and plain objects. Functions, class instances, native objects (Map, Set, Promise, host handles), symbols and cycles are not saved, and each is named in a warning. Computed refs and read-only refs are left out without a warning: a computed ref is derived from other state, and a read-only ref cannot be assigned back. A writable computed ref is not restored, because assigning it would run its setter; a saved value that meets a computed ref of the new version is skipped with a reason. A closure held in a ref is named in a warning, for example `onTick: function (closure, not serialized)`. Restore assigns by binding name and kind. A binding of another kind is skipped with a reason, for example `type changed from string to number`.
- IPC: one JSON object per line on stdin and stdout (`start` with `restore`, `snapshot`, `steps`, `quit`). The protocol is documented in the header of tooling/cli/dev.mjs and of tooling/cli/headless/main.rs (`--serve`).
- Events: watching, changed, check, built, snapshot, ready, error, exited, output, idle, stopped. `--json` prints one per line; the text form prints warnings to stderr.

## Verification

Runs were on the container's shared 4 cores, with other workers' processes present, so timings are noisy. Node v22.22.0; rustc 1.99.0 from the worktree's rust-toolchain file. A dev-loop cycle can queue behind another build for the shared cargo target lock (in a full CLI run, the first dev test took 174 s while a release build in another test file held the lock, against about 21 s alone), so the loop's waits allow 300 s.

| Command | Result |
| --- | --- |
| `node --test "tests/cli/*.test.mjs"` | 75 tests: 75 pass, 0 fail, 0 skipped (exit 0, 365 s), on the final code. Before the last change (the computed-ref exclusion in state.rs) the same command gave 73 of 73. HEAD baseline (clean export of 3435266): 51 tests: 51 pass, 0 fail, 0 skipped (exit 0, 339 s). The 24 added tests are dev.test.mjs (15) and source-frames.test.mjs (9) |
| `node --test "tests/cli/dev.test.mjs"` | 15 tests: 15 pass, 0 fail, 0 skipped (about 33 s) |
| `node --test "tests/cli/source-frames.test.mjs"` | 9 tests: 9 pass, 0 fail (5 for Part A, 2 for the dev form, 2 for the error.stack of a caught throw) |
| `node --test "tests/compiler/**/*.test.mjs"` | 524 tests: 489 pass, 0 fail, 35 skipped (exit 0). The 35 skips are Chrome oracle cases ("Google Chrome not found"). Includes tests/compiler/sourcemap.test.mjs: 8 pass |
| `node tooling/harness/run-language.mjs` | scenarios 79: pass 79, FAIL 0, blocked 0, compile-error 0; 0 unexpected (exit 0) |
| `node tooling/harness/run-language.mjs --dir tests/language/lowering` | scenarios 166: pass 166, FAIL 0, blocked 0, compile-error 0; 0 unexpected (exit 0) |
| `cargo test -p vn-value -p vn-runtime` | 815 passed, 0 failed, 0 ignored in 61 test result lines (unit, integration and doc tests), exit 0. HEAD baseline (clean export, same command): 815 passed, 0 failed, 0 ignored, exit 0. The counts are equal because no Rust test was added or changed (no #[test], #[cfg(test)] or #[ignore] line in the diff); this run is a regression check for the vn-value changes |

Baselines. Every acceptance command was also run on a clean `git archive` export of HEAD 3435266, which has none of the changes: tests/cli 51 pass, 0 fail, 0 skipped (exit 0; the same test titles as the earlier pre-change run); compiler suite 516 tests, 481 pass, 0 fail, 35 skipped (exit 0); cargo test -p vn-value -p vn-runtime 815 passed, 0 failed, 0 ignored (exit 0). Against these, the final runs add 24 CLI tests (dev.test.mjs and source-frames.test.mjs) and 8 compiler tests (tests/compiler/sourcemap.test.mjs: SITES, decodeMappings, identityLocator, sourceMapLocator). No baseline title is missing. An earlier compiler baseline file (485 entries) was recorded before the worktree was at HEAD 3435266 (the branch reflog shows that commit from 03:30:27), so it is not a HEAD baseline. The earlier statement that it had 494 entries, all passing, was wrong and is removed.

## Negative controls

Part A:
- Two lines inserted above the setup throw move the reported line from 5:0 to 7:0 (tests/cli/source-frames.test.mjs).
- A runtime TypeError, which is not a throw statement, gets no frame.
- A wrong expected line (10:2 for the throw at 9:2) is refused by the frame check.
- `error.stack` of a throw that a try/catch catches is the text every error has (`Error: caught` and `    at <anonymous>`, the text kernel_cells_errors.rs pins for other errors). The component compares its caught error's stack with that text. Negative control: with a source position in the expected text, the verdict is `changed`, so the comparison can fail (tests/cli/source-frames.test.mjs).
- tests/compiler/sourcemap.test.mjs: a throw moved by two lines is reported two lines lower; a synchronous function gets no await site on its throws; a character outside the base64 alphabet is refused.

Part B:
- Snapshot disabled (`--no-snapshot`): the same template edit resets the Counter to 0. Restoring is what keeps the value.
- A same-type edit (`ref('a')` to `ref('b')`) restores the old label, so the skip in the type-change test is caused by the type change.
- A type error keeps the running app: the cycle ends with outcome `kept`, and the count is still 2. Fixing the file restarts the app and restores the count.
- A closure in a ref is left out of the snapshot and named in a warning; the other bindings are still restored.
- A writable computed ref whose setter throws is left out of the snapshot, so the restart recomputes it (tests/cli/fixtures/dev-computed). With both guards in tooling/cli/headless/state.rs removed, the restart fails with outcome `failed`, because the restore ran the setter. The negative control of the same test makes the binding a plain ref: it is saved and restored (`restored: count, doubled`).
- The inline form and the dev form of the Counter give equal results for the same interaction. Three clicks instead of two give different results, so the comparison is not vacuous.
- An unchanged scan reports no changed files; the scan skips node_modules, dot directories and other extensions.
- The command line rejects a missing project and `--poll-ms 5` with status 2 before anything starts.

## Latency

tooling/harness/dev-latency.mjs runs the loop on a copy of an example app. Each of 10 edits rewrites the `<h1>` in src/App.vue. The restart is verified after each edit: the new text is shown, and the state element has the same HTML as before the edit. The first build is not an edit and is reported on its own. Values are in ms: min / p50 (nearest rank) / max.

| Metric (ms, min / p50 / max over 10 edits) | Counter, types on | Quick Quote, types on | Counter, `--no-types` | Quick Quote, `--no-types` |
| --- | --- | --- | --- | --- |
| edit to ready | 1370 / 1534 / 1820 | 2092 / 2182 / 2613 | 889 / 912 / 1100 | 1420 / 1513 / 1676 |
| check (types when on) | 444 / 522 / 763 | 577 / 655 / 1034 | 11 / 15 / 42 | 41 / 50 / 172 |
| compile | 11 / 14 / 22 | 45 / 68 / 101 | 10 / 14 / 38 | 37 / 53 / 81 |
| cargo (incremental) | 610 / 644 / 758 | 1098 / 1151 / 1344 | 599 / 644 / 752 | 1053 / 1124 / 1182 |
| app start | 16 / 18 / 29 | 31 / 38 / 60 | 12 / 14 / 20 | 28 / 30 / 50 |
| first build, not an edit (check / compile / cargo / start) | 1545 / 31 / 675 / 22 | 1903 / 157 / 1138 / 42 | 180 / 53 / 657 / 33 | 246 / 111 / 1144 / 49 |

Commands: `node tooling/harness/dev-latency.mjs --app counter --edits 10 [--no-types]` and `--app quick-quote`. Each run was verified: the restarted app showed the edit's heading and the same state element as before the edit. Restored bindings per edit: 1 for the Counter, 7 for Quick Quote.

The options `--no-snapshot` and `--budget-ms N` were added after the runs above. The measured path did not change, but the table was not measured again. Negative controls, one edit each: `--app counter --edits 1 --no-snapshot` exits 1 with `edit 1: the state element changed across the restart`, as expected; `--app counter --edits 1 --budget-ms 1` prints `max edit to ready 2976 ms is over the 1 ms budget`, the verdict over (exit 0, because a verdict is not an error). Smoke run of the final harness, `--app counter --edits 2` with types checked: edit to ready min 1899, p50 1899, max 2495 ms; cargo p50 717 ms; within the 5000 ms budget.

Stage times (check, compile, cargo, start) are the loop's own event fields; edit to ready is measured from the newest modification time of the changed files to the mounted reply.

Cargo time, separately. Cargo is the largest stage. Its p50 is 644 ms for the Counter and 1151 ms for Quick Quote. It is 69 to 75 percent of the edit-to-ready p50 with types off, and 45 to 52 percent with types on, where the type check (p50 522 and 655 ms) is the next stage. The compile step is 14 and 68 ms. Outside the four stages, each edit spends 230 to 285 ms: the poll wait and the debounce (two 100 ms polls), the snapshot and quit round-trips, and the file writes. These were not timed separately.

Budget. design.md (Build row, line 675) proposes "native warm incremental code build/restart <= 5 s", measured on a 100-SFC fixture with 10 runs, cold and warm cargo distinguished. The examples here have 1 SFC (Counter) and 3 SFCs (Quick Quote: App.vue, FieldInput.vue, LineItem.vue). The worst edit-to-ready here is 1820 ms (Counter, types on) and 2613 ms (Quick Quote, types on), which is under 5 s. The fixture is far smaller than the budget's, so this is indicative only. The 10 edits are warm: the first build of each session already had the dependency crates built in this target directory. The dev loop builds the dev profile (`debug = 0`), not the LTO release profile, so it is not the release build the budget may mean.

## Not done, and limits

- Frames cover throw statements and await resumptions only. Errors raised by the runtime (a TypeError from a property read, for example) get no frame. Synchronous callers get no call-site frames.
- A normal `<script>` component (no `<script setup>`) has no source map, so its frames read `(generated code)`.
- `error.stack` is unchanged: kernel_cells_errors.rs pins it for runtime-made errors, and the caught-throw test in tests/cli/source-frames.test.mjs pins it for a throw statement. The ticket says it reads `Name: message`; in fact stack_text already appends `\n    at <anonymous>`, so that description is inaccurate.
- The browser glue (hosts/web) does not report frames, and the web target is not changed.
- Dev loop: headless target only. Every change rebuilds and restarts; there is no template or style patching without a restart. Watching is by polling, not native file events. No editor or LSP features.
- The snapshot lives in memory for one session. It has no schema version, and restore checks names and kinds only. Map, Set, functions, host handles and other native objects are not saved and are named in a warning, as the ticket requires. Computed and read-only refs are left out without a warning (see the snapshot paragraph above).
- A cargo failure after a passing check takes the BuildError branch, which keeps the running app. No test exercises that branch; the tested failure is the type-error case.
- The CLI does not forward stdin commands to the app.
- `dev-latency.mjs` measures two examples on a shared machine; the spread is wide (Counter edit to ready with types on: 1370 to 1820 ms). Its per-edit checks are the state and text verification. Its two negative controls, `--no-snapshot` (must fail) and `--budget-ms 1` (must report over), are in the Latency section.

## Open questions

1. Which profile does the design.md Build budget mean: the dev profile this loop uses, or a release build? The release build with LTO was not measured.
2. Is it acceptable that the `__isScriptSetup` marker is an assignment rather than a non-enumerable property? The runtime only tests it for truthiness, so behaviour is the same, but it is a deliberate deviation from Vue's dev output.
3. After a failed snapshot, the loop restarts the app without state and warns. Should it refuse to restart instead?
4. Should the type checker run on every change by default? It adds a p50 of 0.5 to 0.7 s per cycle here.
5. Computed and read-only refs are left out without a warning. Should they be listed in the warnings, like the other dropped values? Restoring a writable computed by its setter was rejected, because the setter would run on restart.

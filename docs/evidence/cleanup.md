# Runtime and checkpoint cleanup — 2026-10-09

Behavior-preserving cleanup after the 160-scenario acceptance milestone. The
pre-cleanup sources were saved outside the checkout to review this pass
separately from the existing uncommitted implementation. No oracle expectations
or support claims are changed.

## Test deletion evidence (recorded before editing)

| Candidate | What it detects | Surviving proof | History / unlocked cleanup | Risk / validation |
|---|---|---|---|---|
| `crates/ui/tests/zz_debug.rs::debug_nth` | Only unexpected panics or failing DOM setup; prints computed values without comparing them. | `cascade_fixtures_match_chrome` checks 72 real-Chrome fixtures / 6675 element-property values through the actual document adapter; `cascade_checker_detects_wrong_expectations` remains. | Present in initial commit `95a2b0d`; a selector/cascade investigation probe. Delete its private test binary and unused imports. | No behavioral assertion lost. Run `cargo test -p vn-ui -p vn-css`. |
| `crates/ui/tests/zz_debug.rs::debug_parse` | Parser panic only; even returned parse errors are merely printed. | CSS `serialisation_matches_cssom`, `specificity_of_pseudo_classes_and_not`, `an_plus_b_over_siblings` and `selectors_agree_with_chrome`, including negative controls. | Present in `95a2b0d`; prints selector ASTs for inspection. Same obsolete debug file. | No diagnostic or selector assertion lost. Run `cargo test -p vn-css`. |
| `crates/ui/src/dom.rs::zz_debug::links` | DOM setup or snapshot panic only; prints private links without comparing them. | Real-Chrome cascade fixtures exercise document sibling topology, child positions and selector matching; CSS matcher tests assert sibling results independently. | Present in `95a2b0d`; private snapshot investigation probe. Remove the entire test-only module. | Snapshot production implementation stays intact. Run `cargo test -p vn-ui -p vn-css`. |

The complete probes, document adapter, selector parser/matcher, overlapping tests
and support were inspected. Production callers retain `compute_styles`, the
document snapshot and selector APIs; none calls the removed probes. Cargo
discovers Rust tests automatically; `npm test` routes Node tests; no CI matrix or
scoped `AGENTS.md` exists here. The independent corpora and negative controls
remain the primary proof. An initial attempt to remove the renderer's JSON
`as_bool` accessor missed its function-pointer callers (`and_then(Json::as_bool)`)
in `selftest.rs`, and the browser WASM build rejected that change. The accessor
is retained behind `cfg(feature = "selftest")`, matching its actual callers and
eliminating the unused-method warning in the default unit-test build. Boolean
parsing and the selftest JSON protocol are preserved.

## Validation

Saved logs normalize trailing whitespace and final blank lines only; diagnostic
text and test results are unchanged.

Commands used `source scripts/env.sh` and separate private targets for Rust,
UI scenarios, CLI and browser tests. The browser suite needs localhost and
headless Chrome access; it was run outside the filesystem sandbox.

| Command | Actual result | Artifact |
|---|---|---|
| `cargo test -p vn-ui -p vn-renderer-dom` (before cleanup) | 17 pass / 0 fail; three compiler warnings. | [baseline.txt](cleanup/baseline.txt) |
| `cargo test -p vn-runtime -p vn-ui -p vn-css -p vn-renderer-dom` | 222 pass / 0 fail; no compiler warnings; CSS/UI negative controls remain green. | [focused.txt](cleanup/focused.txt) |
| `cargo test --workspace --no-fail-fast` | 1323 pass / 0 fail; no compiler warnings. Former 1326 minus exactly three print-only probes. | [workspace.txt](cleanup/workspace.txt) |
| `node tooling/harness/run-language.mjs --with-vue --ui` | 160 pass; no blocked, failed, compile-error or unexpected cases. | [ui.txt](cleanup/ui.txt) |
| Same command with `--gc-stress` | 160 pass; no blocked, failed, compile-error or unexpected cases. | [ui-gc-stress.txt](cleanup/ui-gc-stress.txt) |
| `node --test "tests/cli/*.test.mjs"` | 40 pass / 0 fail / 0 skipped. | [cli.txt](cleanup/cli.txt) |
| `node --test "tests/platform/web/*.test.mjs"` (after correcting the JSON helper) | 408 tests: 299 pass / 0 fail / 0 cancelled / 109 skips, each with an explicit reason. | [browser.txt](cleanup/browser.txt) |
| `cargo test -p vn-renderer-dom` (after restoring the feature-gated helper) | 8 pass / 0 fail; no compiler warnings. | [renderer.txt](cleanup/renderer.txt) |
| `node --check tooling/cli/build.mjs`; `node tooling/status/update-tasks.mjs --check`; `git diff --check` | All exit 0; tasks remain 9 done / 26 partial / 7 blocked / 28 not started. | Commands run directly; ledger regenerated. |

The first browser attempt failed at WASM compilation because the boolean
accessor had been removed. The [compiler error](cleanup/selftest-build-error.txt)
is retained; the final suite above passes after restoring the accessor under
its owning feature. No test was weakened to repair that mistake.

## Review and limits

Review compared the entire cleanup delta with the saved pre-cleanup sources and
checked the surrounding uncommitted runtime/renderer/host integration. Captured
values, callback order, completion guards and tracing are unchanged. The only
removed tests are the three audited probes. The JSON accessor is a retained
protocol dependency, not dead support. The generated browser entry received
indentation cleanup only. No oracle goldens changed.

Against the pre-cleanup copies: production/tooling +452 / -173 lines (mostly
expanded statements and named captures); tests/test support +0 / -62 lines;
docs/generated ledger +131 / -143 lines. The latter excludes the new historical
archive, this receipt and ten evidence artifacts. Per-file counts are in
[diff-summary.json](cleanup/diff-summary.json). Rust formatting was adjusted
manually; the project toolchain has no rustfmt/clippy component.

All scratch build directories from this pass were removed after verification;
other jobs' caches were left alone. Earlier language/lowering/project/SFC and
compiler/profile receipts remain historical evidence and were not re-run in
this pass. Native devices, other platforms and broader advanced-component
qualification remain unverified as listed in STATUS.md and the task ledger.
The changes were uncommitted when these checks ran; no installs or Git
configuration changes occurred. The user subsequently requested committing and
pushing the verified checkpoint.

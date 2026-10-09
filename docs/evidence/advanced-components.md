# Advanced component acceptance — 2026-10-09

All 160 existing UI scenarios now match the independent Vue 3.5.43 recordings,
with zero blocked cases, failures or unexpected outcomes. The same 160 scenarios
pass with garbage collection at every safepoint (`--gc-stress`). Existing goldens
were not changed. The explicit ledger update was followed by a clean ratchet run.

Implemented Teleport selector/handle targets, local/target anchors, disabled moves,
target changes, deferred mount and event ancestry; KeepAlive state, include/exclude
filters, LRU cache eviction and activation/disposal; async shared loader requests,
loading delays, error fallbacks, timeout and timer cancellation; JS/appear transition
hooks, interrupted leaves, out-in ordering, class phases and computed CSS timing.
All native payloads retain their managed values through collector tracing.

Browser delivery is checked independently through a compiled .vue app: real timer
callbacks and stylesheet duration, Teleport clicks, cached counter state, repeated
cache/teleport teardown with stable browser handles/listeners, and timer cleanup.
`computed_style_property` extends the browser import table to 45. Generated apps
install a host timer driver; startup errors are reported while the realm is alive.

## Verification

All commands use `source scripts/env.sh`, private target directories under
`target/rt-e*`, and the existing pinned toolchain. Saved outputs are in
`docs/evidence/advanced-components/`.

| Command | Result | Artifact |
| --- | --- | --- |
| `node tooling/harness/run-language.mjs --with-vue --ui` | 160 pass; 0 blocked/fail/unexpected | `ui.txt`, `tests/ui/status.json` |
| same command with `--gc-stress` | 160 pass; 0 blocked/fail/unexpected | `ui-gc-stress.txt` |
| `cargo test --workspace --no-fail-fast` | 1326 pass; 0 fail | `workspace.txt` |
| language harness / `--dir tests/language/lowering` / `--with-vue` | 79 / 166 / 102 pass | `language.txt`, `lowering.txt`, `vue.txt` |
| `node --test "tests/compiler/**/*.test.mjs" "tests/profile/*.test.mjs"` | 568 pass | `compiler.txt` |
| `node --test "tests/cli/*.test.mjs"` | 40 pass | `cli.txt` |
| `node --test "tests/platform/web/*.test.mjs"` | 299 pass; 109 documented skips; 0 fail | `browser.txt` |

The two former Rust CSS failures are repaired. Initially the cascade suite had
660 mismatches. Chrome measurements confirmed missing control font resets,
checkbox/disabled defaults and relative bold weights. Descendants of replaced
inputs return empty resolved CSS declarations while retaining typed computed
values. The test adapter omitted the oracle page's `head` before `body`, changing
ancestor `:first-child`/`:nth-child` matching; its topology now matches the oracle.
These fixes left five precision mismatches: the UA's 10pt font was truncated to
13.3333px before subsequent computation. Retaining its full precision clears them.
The 72-fixture, 6675-comparison corpus and its corrupted-expectation negative
control both pass, with all expected values unchanged. Additional independent
Chrome observations are in `css-defaults-chrome.json`.

## Remaining qualification

Task 6.6 stays partial: TransitionGroup, in-out mode, CSS transition-end event
shortcuts, native motion/layout and async retry/hydration hooks are not delivered.
Unsupported modes and async hooks throw explicitly. Broader nested, ref,
activation/transition composition and deferred-unmount races need qualification.
DocHost has no stylesheet engine; browser timing uses actual computed styles.
The browser corpus retains 109 explicitly explained skips outside its Host surface.
Native layout, devices, other browsers and signing remain unverified.

Rustfmt is unavailable in the pinned toolchain; no component was installed.
Existing debug-test unused-import warnings remain. `git diff --check` is clean.

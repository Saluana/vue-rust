# WP-RT-E — Advanced component scenarios

Implement Teleport, Transition/BaseTransition, KeepAlive and defineAsyncComponent
at the existing Host-generic runtime and renderer boundaries. Preserve production
Vue 3.5.43 behavior; keep unsupported capabilities explicit. No JavaScript engine
is introduced.

Owned paths: `crates/runtime/src/{teleport,transition,keep_alive,async_component}.rs`
and the necessary integration in `renderer.rs`, `component.rs`, `vnode.rs`,
`exports.rs`, `lib.rs`. Host timer wiring belongs at the production scheduling
boundary, with the deterministic UI clock implementing the same contract.

Reference: installed `@vue/runtime-core` and `@vue/runtime-dom` production source.
Acceptance uses the existing independent real-Vue recordings in
`tests/ui/golden/{teleport,transition,dynamic}`. Never generate expected results
from the Rust implementation or weaken fixtures.

Use `source scripts/env.sh` and private `CARGO_TARGET_DIR="$VN_ROOT/target/rt-e"`.
Check repository activity before editing; preserve all existing changes. No
commits or pushes in this package. Do not edit during active test runs.

Acceptance:
- `node tooling/harness/run-language.mjs --with-vue --ui --filter teleport`
- matching focused runs for `transition` and `dynamic`
- `node tooling/harness/run-language.mjs --with-vue --ui --write-status`:
  all 160 existing scenarios passing, with no blocked cases or unexpected passes
- `cargo test -p vn-runtime`, relevant compiler and browser regressions
- update the progress ledger, generated task list, status and evidence with actual
  commands/results and any limitations; remove this package's build directory.

The two pre-existing `vn-ui` cascade corpus failures are a separate CSS behavior
gap; record their status honestly if they remain.

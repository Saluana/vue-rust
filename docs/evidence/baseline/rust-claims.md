# Rust-side claims, re-run (WP-G0D)

Re-run on this machine on 2026-10-09 UTC by the WP-G0D implementer. Raw output for each row is in the `rust-*.txt` files in this folder.

Status words: PASSED = reproduced here. FAILED = ran and did not pass. OBSERVED = partial observation only. NOT RUN = not attempted, or not possible on this host.

SCRATCH below means the session scratchpad: `/private/tmp/claude-501/-Users-brendon-Documents-vue-rust-lab/1210756f-d347-4554-a61f-75e4c9c4cd8a/scratchpad`

Toolchain for every Rust row: rustc 1.99.0 (b940084d7 2026-09-28), cargo 1.99.0 (5f94df478 2026-08-27) via the rust-toolchain.toml override, host aarch64-apple-darwin, macOS 27.0.1 (26A434), wasm32-unknown-unknown installed. eframe 0.36.2 per generated/Cargo.lock.

| claim | command | result | date (UTC) | toolchain |
|---|---|---|---|---|
| Counter: four Rust runtime tests pass (root README) | `cargo test --locked --manifest-path generated/Cargo.toml`, CARGO_TARGET_DIR=target/g0d (fresh) | PASSED: tests/runtime.rs 4 passed, 0 failed, 0 ignored. Exit 0, wall 17 s. File: rust-counter-test.txt | 2026-10-09 04:32 | rustc 1.99.0 |
| Counter native executable builds (root README says Linux) | `cargo build --locked --manifest-path generated/Cargo.toml` | PASSED on macOS arm64 only: target/g0d/debug/vue-rust-demo, 37,232,720 bytes. Linux claim: NOT RUN. File: rust-counter-native-build.txt | 2026-10-09 04:32 | rustc 1.99.0 |
| Counter executable window stays up | start the binary, no input, 5 s | OBSERVED: process alive at 5 s, then stopped with SIGTERM. Liveness only; no window was checked. File: rust-counter-native-run.txt | 2026-10-09 04:33 | rustc 1.99.0 |
| Counter WASM library builds (root README) | `cargo build --locked --manifest-path generated/Cargo.toml --lib --target wasm32-unknown-unknown` | PASSED: vue_rust_demo.wasm, 123,028,923 bytes (debug build). wasm-bindgen glue: NOT RUN (out of WP scope). File: rust-counter-wasm-build.txt | 2026-10-09 04:33 | rustc 1.99.0 + wasm32 target |
| Quick Quote: four quote tests pass (quote-app README), with the WP's shared target dir target/g0d | `cargo test --locked --manifest-path SCRATCH/qq-g0d/quick-quote/quote-app/Cargo.toml` | FAILED: exit 101, 17 compile errors, 0 tests ran. Cause: artifact collision with the counter crate (below). File: rust-quickquote-test-sharedtarget.txt | 2026-10-09 04:33 | rustc 1.99.0 |
| Quick Quote: four quote tests pass, separate target dir | same command with CARGO_TARGET_DIR=SCRATCH/qq-target-g0d | PASSED: tests/quote.rs 4 passed, 0 failed, 0 ignored. Exit 0, wall 12 s. File: rust-quickquote-test-isolated.txt | 2026-10-09 04:35 | rustc 1.99.0 |
| Linux executable builds; Linux window launched and visually inspected (both READMEs) | none | NOT RUN: macOS host, no Linux run, no visual check | - | - |
| Mouse and keyboard edits on the native quote window (quote README) | none | NOT RUN: no GUI interaction performed | - | - |
| Browser run of the counter (root README) | none | NOT RUN: out of WP-G0D scope | - | - |
| 25 and 28 compiler tests pass (root and quote READMEs) | none | NOT RUN: Node-side tests, out of WP-G0D scope | - | - |

## Quick Quote failure: most likely cause

- The Quick Quote crate and the counter crate are both package `vue-rust-demo` 0.0.1. Their Cargo.toml and Cargo.lock files are byte-identical. Only src/ differs.
- In both target dirs cargo produced the same artifact names (for example `vue-rust-demo-f669ee325355be9d`, `vue-rust-demo-25d3848ccdc8b99a`). Sharing target/g0d therefore mixed the two crates.
- The counter's `libvue_rust_demo.rlib` in target/g0d contains "doubled" 5 times and "labour" 0 times. The rlib from the isolated build contains "labour" 11 times and "doubled" 0 times. The shared run's errors name the counter's API, so the integration test was compiled against the counter's rlib.
- Quick Quote's own src/app.rs defines labour, totalText, mowingRate and the other methods the tests call. Its isolated build passes.
- I did not trace cargo's fingerprint decision further. The source of the Quick Quote crate was not edited.

## State left in target/g0d

- The Quick Quote run rebuilt the lib-test and bin-test harnesses under the counter's names at 04:33:51Z. Before re-running the counter in target/g0d, run `cargo clean -p vue-rust-demo --manifest-path generated/Cargo.toml` with CARGO_TARGET_DIR=target/g0d, or use a fresh target dir.
- The native binary target/g0d/debug/vue-rust-demo still has sha256 b63a3869f030cb01d5deca5207fd8647d1e82fdcafb3d0a449fec395773a2d35 (checked after the Quick Quote run).

## Reproduce

```
SCRATCH=/private/tmp/claude-501/-Users-brendon-Documents-vue-rust-lab/1210756f-d347-4554-a61f-75e4c9c4cd8a/scratchpad
cd /Users/brendon/Documents/vue-rust-lab
source scripts/env.sh
export CARGO_TARGET_DIR="$VN_ROOT/target/g0d"
cargo test --locked --manifest-path generated/Cargo.toml
cargo build --locked --manifest-path generated/Cargo.toml
cargo build --locked --manifest-path generated/Cargo.toml --lib --target wasm32-unknown-unknown
mkdir "$SCRATCH/qq-g0d" && unzip -q /Users/brendon/Downloads/quick-quote-source.zip -d "$SCRATCH/qq-g0d"
cargo test --locked --manifest-path "$SCRATCH/qq-g0d/quick-quote/quote-app/Cargo.toml"   # with CARGO_TARGET_DIR=target/g0d: FAILS (see above)
CARGO_TARGET_DIR="$SCRATCH/qq-target-g0d" cargo test --locked --manifest-path "$SCRATCH/qq-g0d/quick-quote/quote-app/Cargo.toml"   # PASSES
```

## Disk

- Free space before each build was at least 6.8 GiB (guard: 3 GiB). Checks used `df -k .`.
- target/g0d: 1.7G (du -sh) after all runs; left in place.
- toolchain/cargo grew by about 193 MB (downloaded crate sources and index cache, project-local CARGO_HOME per scripts/env.sh).

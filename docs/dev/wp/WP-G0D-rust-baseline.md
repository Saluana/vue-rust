# WP-G0D — Reproduce the Rust-side claims of both prototypes (tasks.md 2.1, part 2)

Read `docs/dev/AGENT-CONTRACT.md` first. Use `export CARGO_TARGET_DIR="$VN_ROOT/target/g0d"` (private).

**Owned paths:** `docs/evidence/baseline/rust-*.txt`, `docs/evidence/baseline/rust-claims.md`. You build only
inside `target/g0d` and the scratchpad; you do not modify any source file.

## Why
The prototype READMEs claim: counter 4 Rust tests pass, native executable builds, WASM library builds; the
Quick Quote app has 4 more Rust tests. None of this was re-run on this machine yet. Missing evidence must stay
visible; reproduced evidence must be recorded (requirements R01.AC2, tasks 2.1).

## Do (guard: before each build run `df -h .`; if free space is below **3 GiB**, stop and report)
1. Counter: `cargo test --locked --manifest-path generated/Cargo.toml` (the project-root `generated/` is the
   original; its `Cargo.lock` is pinned). Record: toolchain versions, command, number of tests passed/failed,
   wall time, size of `target/g0d` afterwards (`du -sh`).
2. Counter native executable: `cargo build --locked --manifest-path generated/Cargo.toml` — record the binary path
   and size. Then START it for ~5 seconds in the background to see whether a window process stays alive
   (`generated/target`-style path under `target/g0d`); kill it afterwards. Record the observation honestly
   (“process stayed up for 5 s” is NOT a visual verification). Do not click or interact with anything.
3. Counter WASM: `cargo build --locked --manifest-path generated/Cargo.toml --lib --target wasm32-unknown-unknown`;
   record success/failure and artifact size. (wasm-bindgen CLI generation is out of scope here.)
4. Quick Quote: extract `/Users/brendon/Downloads/quick-quote-source.zip` into a NEW empty scratchpad directory
   (`.../scratchpad/qq-g0d`; untrusted archive: list entries first, refuse `..`/absolute paths; never run its
   scripts), then `cargo test --locked --manifest-path <extract>/quote-app/Cargo.toml` with the same target dir.
   Record the result and test names.
5. Write each run's raw tail (last ~60 lines) into `docs/evidence/baseline/rust-<name>.txt` and a summary table in
   `docs/evidence/baseline/rust-claims.md` with columns claim / command / result / date / toolchain.
6. If a build fails for an environmental reason (e.g. OpenGL/system library, macOS version), record the exact
   error verbatim and what you tried; do not hack source files. A failing claim is valid evidence.

## Acceptance
Evidence files exist with real output; the summary distinguishes passed / failed / not run. No source files
modified. At the end print `du -sh $CARGO_TARGET_DIR` and leave the target dir in place (maintainer decides on
cleanup).

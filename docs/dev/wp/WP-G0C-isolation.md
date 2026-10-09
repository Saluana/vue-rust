# WP-G0C — Research build isolation and dependency policy (tasks.md 2.3)

Read `docs/dev/AGENT-CONTRACT.md` first. WP id: `g0c` (use `rustc` directly, not cargo, for the fixture build script).

**Owned paths:** `docs/security/**`, `release/dependency-policy/**`, `tooling/sandbox/**`,
`tests/security/build-isolation/**`.

## Why
Installing/building a project executes trusted tooling (npm lifecycle scripts, Cargo `build.rs` and proc-macros).
Hostile-input experiments must run where a malicious script cannot read secrets or write outside its workspace
(requirements R22.AC1; design.md §13.1).

## Do
1. `docs/security/build-trust.md`: trusted vs untrusted build contexts, what executes where, rules (no signing
   secrets/home directory/network in untrusted workers, disposable workspace, scripts disabled by default),
   and an honest limits section: `sandbox-exec` is deprecated and best-effort on macOS; containers/VMs were not
   verified here and are not assumed to isolate the host; Cargo is not a sandbox.
2. `tooling/sandbox/run-untrusted.mjs` (Node, no deps): run a command inside a macOS `sandbox-exec` profile you
   generate (SBPL, `(deny default)`), allowing: reading system libraries/executables needed by the command,
   reading/writing ONLY a given workspace directory plus a given scratch/tmp subdirectory; denying network and
   any other file read/write. Provide `--workspace DIR --cmd …` CLI and a function API. If `sandbox-exec` is
   unavailable, exit with a distinctive non-zero code and message (never pretend success).
3. A hostile fixture under `tests/security/build-isolation/fixtures/`: (a) a Node script mimicking an npm
   postinstall, (b) a small Rust `build.rs`-style program compiled with the project `rustc`. Each attempts to read
   a sentinel secret file placed OUTSIDE the workspace (the test creates it in a temp "fake home"), to write a
   file outside the workspace, and to connect to a localhost TCP listener the test opened.
4. Tests (`tests/security/build-isolation/*.test.mjs`, node:test): unsandboxed run proves the attacks WORK
   (positive control: sentinel is read, file written, connection observed); sandboxed run proves all three are
   blocked while reading/writing inside the workspace still works. If `sandbox-exec` is missing the test must be
   reported as skipped with a loud message, not passed.
5. `release/dependency-policy/POLICY.md`: lockfile + integrity requirement, registry allowlist
   (`registry.npmjs.org`, `index.crates.io`, `static.crates.io`), no floating versions or `latest`, install
   scripts disabled by default, license allow/deny lists (initial), "unknown license fails the release check",
   process for adding a dependency.
6. `release/dependency-policy/candidates.json`: ledger of the third-party components named in design.md §15
   (Vue, Babel, egui/eframe, winit, wgpu, Taffy, Parley/Vello, Blitz/Stylo, AccessKit, Slint/Qt, SQLite binding,
   TLS, font/image/SVG libs, Test262/WPT). For each: `{name, role, licenseSignal, source, status:
   "unresolved"|"verified", evidence}`. **Only mark `verified` when you can show a local file** (a LICENSE file in
   `node_modules` or the cargo registry cache under `toolchain/cargo/registry`); otherwise `unresolved` and quote
   the design.md signal as `licenseSignal`. Do not assert license facts from memory.

## Acceptance
`node --test "tests/security/build-isolation/*.test.mjs"` passes (or reports skip loudly if the sandbox tool is absent, in
which case say so in your report); policy docs exist; no files outside owned paths changed.

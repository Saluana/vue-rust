# WP-G0A — Preserve the two prototypes as immutable fixtures; reproduce the JS baseline (tasks.md 2.1, part 1)

Read `docs/dev/AGENT-CONTRACT.md` first. WP id for your target dir: `g0a` (you will mostly not need cargo).

**Owned paths:** `tests/fixtures/prototype/**`, `docs/evidence/baseline/**`. Nothing else.

## Why
Requirements R01.AC3 / R24.AC3: the original counter prototype and the Quick Quote ZIP prototype must remain
identifiable, byte-for-byte, so later compiler changes cannot silently rewrite baseline claims, and each known
prototype bug gets a regression that reproduces the OLD failure. We are about to replace the root `compiler/`.

## Do
1. **Counter fixture.** Copy (do not move) from the project root into `tests/fixtures/prototype/counter/`,
   preserving relative layout: `compiler/compile.mjs`, `compiler/cli.mjs`, `examples/Counter.vue`,
   `tests/compiler.test.mjs`, `generated/` (exclude `target/`, `pkg/`, `.DS_Store`), `package.json`,
   `package-lock.json`, `README.md`, `evidence/*`. The fixture's own test must run unchanged:
   `node --test tests/fixtures/prototype/counter/tests/compiler.test.mjs` (bare imports resolve to the root
   `node_modules`; that is intended).
2. **Quick Quote fixture.** Source: `/Users/brendon/Downloads/quick-quote-source.zip`. First verify its SHA-256
   equals `470e78787014e2dbbdc8fdf9cf2107fc48caa574d8c02aa59823689e9bba3a0e` (stop and report if not). Treat the
   archive as untrusted: list entries first, refuse any entry with `..` or an absolute path, extract into a NEW
   empty directory under the scratchpad (`/private/tmp/claude-501/-Users-brendon-Documents-vue-rust-lab/1210756f-d347-4554-a61f-75e4c9c4cd8a/scratchpad/qq-extract`), then copy the contents of its top
   `quick-quote/` directory into `tests/fixtures/prototype/quick-quote/` (keep PNG evidence and logs). Never run
   scripts from the extracted tree.
3. **`tests/fixtures/prototype/MANIFEST.json`**: `{ "zip": {"path","sha256","bytes"}, "workspaceCompilerSha256",
   "zipCompilerSha256", "node": "<version>", "files": [{"path","sha256","bytes","origin":"workspace"|"zip"}] }`
   with every preserved file (sorted by path). Record whether `workspaceCompilerSha256` equals
   `24224cdbcc536bfcbaa970bd4595190c974bc26998fc3f8d814685064d31afad` (the value claimed in
   `planning/.../requirements.md`).
4. **`tests/fixtures/prototype/README.md`**: provenance, "immutable: never edit these files; a semantic change
   needs migration notes elsewhere", and exact commands to run each fixture's JS tests.
5. **`tests/fixtures/prototype/verify-fixtures.test.mjs`** (node:test): recompute the SHA-256 of every file listed
   in MANIFEST.json and fail on mismatch, missing file, or an extra file under the fixture dirs that is not in the
   manifest (ignore this test file, README.md, MANIFEST.json, regressions.test.mjs). Include a negative control
   that proves a one-byte change and an extra file are detected (test the verification *function* on in-memory
   or temp-dir data, never by editing fixtures).
6. **Run both fixture JS suites** and record the results in `docs/evidence/baseline/js-tests.txt`: exact commands,
   date, `node --version`, exit codes, pass/fail counts, any failures verbatim. Counter should be 25 tests; the
   Quick Quote suite 28. If counts differ, record the real numbers and flag it.
7. **`tests/fixtures/prototype/regressions.test.mjs`** — executable reproductions of KNOWN prototype defects. Each
   test asserts the OLD (buggy) behaviour so we have a regression to flip later; name each test
   `KNOWN BUG (prototype): …`. Import the OLD compilers from the fixture copies, never from the root `compiler/`.
   Reproduce at least:
   a. Dynamic event argument `<button @[click]="inc">` is accepted and compiled as a fixed click handler
      (counter and quote compilers; report which accept).
   b. Rust `Display` for interpolated numbers diverges from JS `String(n)`: show the generated code uses
      `format!("{}", …)`; and compile a tiny std-only Rust program with the project toolchain
      (`source scripts/env.sh; rustc`) into `os.tmpdir()` that prints `format!("{}", x)` for
      `[1e21, 1e-7, -0.0, 0.1+0.2, 123456789012345680000.0, f64::NAN, f64::INFINITY, 5e-324]` and assert at least
      the divergent ones differ from Node's `String(x)`; record both lists in the test output / evidence.
   c. How the Quick Quote compiler emits `toFixed`: reproduce a divergence from JS for (0.5).toFixed(0),
      (2.5).toFixed(0), (1.005).toFixed(2) using the same tiny-Rust technique with the formatting the compiler
      actually emits (read `quick-quote` compiler to see it). If it does not diverge for some of them, say so.
   d. Computed values are uncached getters: the emitted Rust recomputes on every read (assert the emitted shape,
      e.g. a `pub fn doubled(&self)` with the expression inline and no cache field).
8. **`docs/evidence/baseline/claims-ledger.md`**: a table of every verification claim in the root `README.md`
   and `evidence/verification.txt` → status (`reproduced-here` / `not-reproduced` / `not-attempted`) → evidence
   file. Claims about Rust tests, native executable, WASM are `not-attempted` (another WP covers them).

## Acceptance (the reviewer will run these)
- `node --test tests/fixtures/prototype/` passes (verify + regressions), and
  `node --test tests/fixtures/prototype/counter/tests/ tests/fixtures/prototype/quick-quote/tests/` passes.
- Root files are untouched (compare with the original; you only added files under your owned paths).
- MANIFEST.json hashes match; negative controls demonstrably fail when the condition is violated.

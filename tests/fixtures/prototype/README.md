# Prototype fixtures (WP-G0A)

Byte-preserved copies of the two prototypes, so later compiler changes cannot silently rewrite baseline claims.

**Immutable: never edit these files.** A semantic change needs migration notes elsewhere, never an edit here.

## Provenance

- `counter/` is copied from the workspace root on 2026-10-09 (UTC): `compiler/`, `examples/Counter.vue`,
  `tests/compiler.test.mjs`, `generated/` (without `target/`, `pkg/`, `.DS_Store`), `package.json`,
  `package-lock.json`, `README.md` and `evidence/`. Its `compiler/compile.mjs` has SHA-256
  `24224cdbcc536bfcbaa970bd4595190c974bc26998fc3f8d814685064d31afad`, the value claimed in
  `planning/vue-native-framework/requirements.md`. The claim matches.
- `quick-quote/` is the top-level `quick-quote/` directory of `/Users/brendon/Downloads/quick-quote-source.zip`
  (SHA-256 `470e78787014e2dbbdc8fdf9cf2107fc48caa574d8c02aa59823689e9bba3a0e`, 104048 bytes). The archive was
  listed before extraction: 19 entries, none with `..`, an absolute path or a link. It was extracted into an
  empty scratch directory and only then copied here. Nothing was run from the extracted tree. The copied fixture
  tests run from this directory, as the WP requires.
- `MANIFEST.json` lists all 37 preserved files with SHA-256, size and origin (`workspace` or `zip`), sorted by path.
  Paths are relative to this directory.

## Files at this level

- `MANIFEST.json`: the pinned hashes. Not a fixture. Its `workspaceCompilerClaim` block records that the workspace
  compiler hash equals the value claimed in `requirements.md` (`matches: true`).
- `verify-fixtures.test.mjs`: recomputes every hash and fails on mismatch, missing file or unlisted extra file.
  Its negative controls run the same checker on temporary trees (one-byte change, appended byte, extra file,
  missing file, symlink, escaping path).
- `regressions.test.mjs`: executable reproductions of known prototype defects. Each test asserts the OLD (buggy)
  behaviour and is named `KNOWN BUG (prototype): ...`. When a defect is fixed, its test fails on purpose: flip the
  assertion in the same change that fixes the defect.
- `README.md`: this file. Not a fixture.

## Commands

Run from the project root. The fixture tests import `@vue/compiler-sfc`, `@vue/compiler-dom` and `@babel/parser`
from the root `node_modules`, by design, so run `npm ci` there first if it is missing.

```sh
# Counter suite, unchanged fixture test: 25 tests
node --test tests/fixtures/prototype/counter/tests/compiler.test.mjs
# Quick Quote suite, unchanged fixture test: 28 tests
node --test tests/fixtures/prototype/quick-quote/tests/compiler.test.mjs
# Verification and known-defect reproductions: 15 + 6 = 21 tests
node --test 'tests/fixtures/prototype/*.test.mjs'
# Everything: 25 + 28 + 15 + 6 = 74 tests
node --test 'tests/fixtures/prototype/*.test.mjs' 'tests/fixtures/prototype/counter/tests/*.test.mjs' 'tests/fixtures/prototype/quick-quote/tests/*.test.mjs'
```

Node v26.5.1 does not accept a directory argument to `node --test` (it tries to load the directory as a module),
so use the glob or file forms above.

The regression suite compiles a std-only Rust probe with the project toolchain (it sources `scripts/env.sh`),
writes it under `os.tmpdir()`, runs it and deletes the directory afterwards. The archive checks read entries with
`unzip -p` and skip with a visible reason if the archive is no longer at its recorded path.

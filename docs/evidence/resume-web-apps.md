# Compiled browser apps — 2026-10-09

HEAD on resume: 05987fa. These changes were uncommitted when the acceptance checks ran.

Implemented the CLI's generated .vue web entry, WebHost's shared-host adapter, real event identity/kind delivery and Rust scheduler/promise draining in a browser microtask. The browser console receives runtime errors. A missing container rejects startup loudly. Host-only apps continue to use ABI 1.

Counter and Quick Quote run in real headless Chrome with compiled Rust/WASM. The interaction tests cover clicks, computed values, conditions, disabled controls, input and defineModel, validation, watch, keyed rows, remove/add, and editor unmount/recreation. Twenty repeated hide/show cycles keep live host handles and listener counts unchanged.

The lifetime regression failed before the browser subtree repair: handles grew from 207 to 1247 across 20 cycles. WebHost now enumerates mapped descendants before retirement and releases the whole detached subtree; the dispatch cache is cleared before collection. The same regression now passes.

## Final commands and results

Run after `source scripts/env.sh`; CLI and browser suites each used a fresh private target directory.

- `cargo test --workspace --no-fail-fast`: 1324 passed, 2 failed, both pre-existing vn-ui cascade_corpus tests. No new Rust failures.
- `node tooling/harness/run-language.mjs`: 79 passed, 0 unexpected.
- `node tooling/harness/run-language.mjs --dir tests/language/lowering`: 166 passed, 0 unexpected.
- `node tooling/harness/run-language.mjs --with-vue`: 102 passed, 0 unexpected.
- `node tooling/harness/run-language.mjs --with-vue --ui`: 143 passed, 17 advanced-component blockers, 0 failed, 0 unexpected.
- `node --test "tests/compiler/**/*.test.mjs" "tests/profile/*.test.mjs"`: 568 passed, 0 failed.
- `node --test "tests/cli/*.test.mjs"`: 40 passed, 0 failed.
- `node --test "tests/platform/web/*.test.mjs"`: 407 tests; 298 passed, 109 explicitly skipped, 0 failed. Four added corpus cases use stylePriority outside the Host ABI; those cases pass in vn-dom. The three new compiled-app tests pass without skips.

One full-suite run measured Counter at 2087122 bytes WASM (614767 gzip), Quick Quote at 2251932 bytes (648890 gzip). First render was measured per run, not as a performance qualification. No app size optimization was attempted.

Native rendering/computed style remains unfinished; the two known style tests still fail. Teleport/Transition/KeepAlive/async components remain blocked. Safari/Firefox, mobile/desktop platforms outside this Mac, accessibility, persistence, packaging and signing remain unqualified. TypeScript is erased, not type checked. Runtime source maps, app shutdown/restart APIs and build/dev reload remain follow-ups.

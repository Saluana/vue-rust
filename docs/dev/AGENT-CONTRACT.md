# Engineering rules — read before starting any work package

These are the rules for working on **vue-native** (provisional name): a framework that compiles Vue single-file components
(JavaScript/TypeScript, Composition API) through Rust to native binaries and WebAssembly. Each work package (WP) has a brief in
`docs/dev/wp/`; implement it, test it, and report honestly. Whoever reviews your work will re-run your commands and read your
code. Plans live in `planning/vue-native-framework/*.md` (read-only); the architecture decisions are in `docs/dev/ARCHITECTURE.md`.

## Environment

```sh
cd /Users/brendon/Documents/vue-rust-lab
source scripts/env.sh                      # project-local Rust 1.99.0 + cargo on PATH
export CARGO_TARGET_DIR="$VN_ROOT/target/<your-wp-id>"   # ALWAYS a private target dir
```

- macOS arm64, Node v26.5.1, Rust 1.99.0 (+ wasm32-unknown-unknown). No rustfmt/clippy. No internet needed.
- **Disk is nearly full.** Do not create big files, never run `cargo clean` on other WPs' target dirs, never
  copy `node_modules`/`target`. Delete scratch outputs you no longer need.
- Long commands: run them in the background and poll, or give them a generous timeout. Keep builds scoped
  (`cargo test -p <crate>`), use `--workspace` only for a baseline check of the whole repo, with your own target dir.
- Node deps already installed: `@vue/compiler-sfc`, `@vue/compiler-dom`, `@babel/parser`, and (once they are added) `vue@3.5.43` and friends. **Do not add npm packages or Rust crates** unless your WP
  explicitly lists them. Never enable npm install scripts.

## Ownership

While several packages are built in parallel, only create/modify files inside the paths your WP lists as **Owned**. (When you are the only one working, you may also edit the shared files you genuinely need, such as root `Cargo.toml`, `package.json`, `docs/dev/**` and the `status.json` ledgers, with small diffs. The generated progress notes in `planning/vue-native-framework/tasks.md` are written only by `tooling/status/update-tasks.mjs`.) Never edit: `planning/**`, `.gitignore`,
root `Cargo.toml`, root `package.json`/lockfile, `scripts/env.sh`, `docs/dev/**` (except a file your WP
names), or another WP's files. If you need a change outside your paths, do not make it: put it in your report
under "Requests to the maintainer". If a module/stub already exists in your path, extend it; do not rename the
public API listed in your WP.

## Truthfulness rules (non-negotiable)

1. Never fabricate output. Report the exact commands you ran and their real pass/fail counts.
2. Never weaken, skip, `#[ignore]`, comment out, or special-case a test to get green. Never hardcode
   expected values inside the code under test, branch on test names/file names/app names, or compute expected
   values with the code being tested. If a test cannot pass, leave it failing and say so.
3. Expected values come from an **independent oracle**: Node/V8 for JavaScript semantics, the real
   `vue@3.5.43` package for Vue behaviour, or values written down from a spec/by hand. Add at least one
   negative control (a deliberately wrong expectation or mutated input that must be detected) when you build a
   comparison harness.
4. A feature is "done" only when its tests ran and passed in this session. Compiled-but-untested is not done.
5. If you are unsure of a spec detail, choose the most conservative behaviour (reject / error out, never guess
   silently), leave a short comment, and list it under "Open questions" in your report.

## Product rules

- No embedded JavaScript engine, interpreter fallback, `eval`, webview, or "run the JS in Node at runtime".
  Application logic is compiled to Rust.
- JavaScript `throw` is a Rust `Err(Throw)` value, never a Rust panic. Panics are framework bugs.
- No `unsafe` unless your WP allows it; if you use it, put the invariant in a `// SAFETY:` comment.
- Unsupported constructs must fail loudly with a diagnostic that has a code, a message, and a source span.
  Nothing is silently ignored or approximated. Unknown APIs/methods are **unsupported by default**.
- No special cases for example apps (`Quote`, `Counter`, `Fieldboard`, ...) in compiler or runtime code.
- Keep third-party licenses in mind: do not copy code from Vue/other projects verbatim into Rust; porting
  *behaviour* (reading the readable `node_modules/@vue/*/dist/*.cjs.js` builds) is expected, but write your
  own implementation and keep a one-line attribution comment (`// behaviour per @vue/reactivity 3.5.43`).

## Code style

Match the surrounding code. Short, direct names. Comments only for non-obvious invariants or spec references.
Rust edition 2024. Tests: Rust built-in `#[test]` and `tests/*.rs` for crates; `node:test`
(`node --test <files>`) for JavaScript. Test files go next to what they test, as your WP specifies.

## Reporting (≤ 350 words, plain text, no marketing)

End your work with: (1) what you built, (2) files created/changed, (3) exact commands to re-run your tests and
their real results (counts), (4) known gaps / things you did not do and why, (5) "Open questions",
(6) "Requests to the maintainer". If you hit a blocker, stop and report it instead of looping.

## Practical notes learned from earlier work packages
- Node 26's `node --test` does **not** recurse into directories: use quoted globs, e.g. `node --test "tests/profile/*.test.mjs"`
  or `node --test "tests/**/*.test.mjs"`.
- Never share a `CARGO_TARGET_DIR` between two crates that have the same package name and version (cargo artifact names
  collide). Use your WP's private target dir.
- The Cargo workspace globs `crates/*`: when you create a new crate, create `Cargo.toml` **and** `src/lib.rs` (even an empty one) in the
  same command, immediately. A crate directory with a manifest but no target breaks `cargo` for every other agent.

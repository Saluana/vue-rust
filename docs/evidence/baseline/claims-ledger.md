# Claims ledger: root README.md and evidence/verification.txt (WP-G0A)

Checked 2026-10-09 UTC on macOS 27.0.1 arm64, node v26.5.1. Rust rows use rustc 1.99.0 via `scripts/env.sh`.
The original Linux machine is not available here, so Linux claims cannot be reproduced.

**Status words.** `reproduced-here`: re-run or re-checked on this host, with the evidence shown. `not-reproduced`:
a run contradicts the claim. `not-attempted`: not run by WP-G0A. A Rust, native or WASM row marked
`not-attempted` is not a pass: WP-G0A does not cover those claims. Where WP-G0D re-ran one, the note cites
`docs/evidence/baseline/rust-claims.md` as that WP's result, not as WP-G0A verification.

**Evidence keys.**
- `JS#n`: section n of `docs/evidence/baseline/js-tests.txt`.
- `PROBE x`: case x in `docs/evidence/baseline/readme-claims-probe.txt`, produced by `readme-claims-probe.mjs`.
- `REG "name"`: test in `tests/fixtures/prototype/regressions.test.mjs`.
- `VER`: `tests/fixtures/prototype/verify-fixtures.test.mjs`.
- `SRC path:line`: a line in the byte-identical fixture copy (the root file it was copied from is the same bytes).
- `ORIG path`: the original record in the root `evidence/` directory. It was not re-run.
- `static`: a file read, hash, name search or lock-file check, with no build or run.

## A. Root README.md

| # | Line | Claim | Status | Evidence | Note |
|---|---|---|---|---|---|
| R1 | 3 | The native app does not embed a browser or JavaScript engine | not-attempted | PROBE C (static): `generated/Cargo.lock` names no JS-engine or webview crate | The built binary was not inspected. |
| R2 | 3 | The same generated Rust also builds to WebAssembly | not-attempted | ORIG `evidence/wasm-build.txt`; WP-G0D: WASM library PASSED (`rust-claims.md`) | Rust side, not run by WP-G0A. |
| R3 | 5 | Vue's official JavaScript parser is used at build time | reproduced-here | SRC `counter/compiler/compile.mjs:1-3` imports `@vue/compiler-sfc`, `@vue/compiler-dom`, `@babel/parser`; JS#2 | |
| R4 | 5 | The compiler frontend has not been rewritten in Rust | reproduced-here | static: the only compiler is JavaScript under `counter/compiler/`; `crates/` contains only `value` | Static check. |
| R5 | 13 | `npm test` runs the compiler tests | reproduced-here | JS#8: root `npm test` 25 pass, 0 fail, exit 0 | |
| R6 | 12 | `npm ci` installs the dependencies | not-attempted | none | Not run: it would rewrite `node_modules`. |
| R7 | 14 | `npm run build:demo` builds the counter app | not-attempted | none | The script writes `build/` into the repo root, which is outside WP-G0A's paths. The CLI step it runs was reproduced into a temp directory: see R9. |
| R8 | 15 | `npm run native` runs the native app | not-attempted | none (no original record of this script) | Rust build and run; WP-G0D did not run this npm script. |
| R9 | 21 | `node compiler/cli.mjs examples/Counter.vue <new dir>` writes a separate build | reproduced-here | PROBE C: the CLI writes `src/app.rs`, `src/lib.rs`, `src/main.rs`, `Cargo.toml`, `index.html` into a new directory | Run against the fixture copy, into a temp directory. |
| R10 | 22 | `cargo run --manifest-path build/counter-2/Cargo.toml` runs that build | not-attempted | WP-G0D ran the `generated/` binary for 5 s (liveness only, `rust-claims.md`) | Not run on a CLI build directory. |
| R11 | 25 | The compiler refuses an existing output directory so it cannot overwrite files | reproduced-here | PROBE C: a second run into the same directory exits 1 with "Output already exists; choose a new directory to preserve your files" | |
| R12 | 27 | Linux needs a graphical session and eframe libraries; Windows and Mac builds are not tested here | not-attempted | none | Platform prerequisites not checked. |
| R13 | 31 | `<script setup>` with normal JavaScript, optionally `lang="ts"` | reproduced-here | JS#2 (`examples/Counter.vue` is `lang="ts"`); PROBE A8 | Compile stage only. |
| R14 | 32 | `import { ref, computed } from 'vue'` | reproduced-here | JS#2; PROBE A16, A17 (other imports rejected) | Compile stage only. |
| R15 | 33 | Primitive number, string and boolean refs | reproduced-here | PROBE A2 (`pub a: f64`, `pub b: String`, `pub c: bool`) | Compile stage only. |
| R16 | 33 | Each component instance has its own state | not-attempted | static: test `components_have_independent_state` in `generated/tests/runtime.rs`; WP-G0D: the four runtime tests PASSED | Not run by WP-G0A. |
| R17 | 34 | Pure expression computed values, `.value` in setup, automatic unwrapping in templates | reproduced-here | JS#2 ("template auto unwrap and script .value", "forward computed dependency"); PROBE B8 | |
| R18 | 35 | Zero-argument synchronous event functions | reproduced-here | PROBE A7 (parameter rejected); JS#2 ("async handler" rejected) | |
| R19 | 35 | Ref assignment and increment/decrement | reproduced-here | JS#2 ("inline update"); PROBE A3 (`count = 5`), A4 (`count--`) | |
| R20 | 36 | Text interpolation, arithmetic, strict comparisons and boolean conditions (compile stage) | reproduced-here | JS#2; PROBE A5 (`===`), A6 (`!==`), B5 (interpolation) | Compile stage only. Number text is R21. |
| R21 | 36 | Interpolated numbers print the same text as Vue/JavaScript (read as the claim behind "works") | not-reproduced | REG "KNOWN BUG (prototype): interpolated numbers use Rust Display ..."; JS#4 diagnostics | Rust `format!("{}")` gives `1000000000000000000000`, `0.0000001`, `-0`, `inf`, and a 326-character string for 5e-324. JS gives `1e+21`, `1e-7`, `0`, `Infinity`, `5e-324`. Three values agree: 0.1+0.2, 123456789012345680000 and NaN. |
| R22 | 37 | Elements main, section, div, h1, h2, p, span, label | reproduced-here | PROBE A1 (accepted) | Compile stage only. |
| R23 | 38 | `<button @click="increment">`, inline updates and `:disabled` | reproduced-here | JS#2 (`self.increment();`); PROBE A3, A5 (`:disabled` emitted as `add_enabled`) | |
| R24 | 39 | `<input v-model="name" />` for a string ref | reproduced-here | JS#2 (asserts `text_edit_singleline`) | |
| R25 | 40 | `v-if` | reproduced-here | JS#2 ("boolean conditional"); PROBE A6 | |
| R26 | 41 | Rust/egui rendering, with the same app usable by native and WASM hosts | not-attempted | WP-G0D: native build PASSED on macOS arm64; WASM library build PASSED; no visual check by either WP | |
| R27 | 43-45 | CSS, custom components, props, emits, slots, lists, reactive objects, watchers, lifecycle hooks, async code, fetch, routing, npm imports and DOM APIs are not implemented | reproduced-here | PROBE A9 to A21 (each rejected with a message); JS#2 (CSS, `v-for`, async, npm import) | Probed items: custom component, slot, defineProps, defineEmits, reactive, watch, onMounted, vue-router, npm import, fetch, console.log, CSS, v-for. Async is from JS#2. |
| R28 | 45 | Vue plugins and native device APIs are not implemented | not-attempted | none | Not probed. |
| R29 | 45 | TypeScript type annotations and generics are rejected | reproduced-here | JS#2 ("unimplemented type annotations are rejected"); PROBE A22 | Generics probed. Plain annotations such as `const x: number` not probed. |
| R30 | 43 | `lang="ts"` is accepted only for the currently supported syntax | reproduced-here | PROBE A8 (accepted); JS#2 | Rejection of unsupported TypeScript is R29. |
| R31 | 47 | Unsupported constructs produce an error rather than being silently ignored | not-reproduced | REG "KNOWN BUG (prototype): @[click] dynamic event argument is accepted ..." (counter and quick-quote); PROBE B1, E1 vs B2, B4 | `<button @[click]="inc">` is accepted and compiled as a fixed click handler, byte-identical to `@click="inc"`. Every other unsupported construct probed was rejected. |
| R32 | 49 | `computed` emits a pure Rust getter evaluated on each UI read; no cached dependency graph, watcher scheduler or batching | reproduced-here | REG "KNOWN BUG (prototype): computed values are uncached ..." (both compilers); PROBE B8 | |
| R33 | 49 | Layout uses vertical native-rendered groups; controls are custom-drawn, not OS widgets | not-attempted | PROBE A1 shows `ui.vertical` in the emitted code (compile stage only) | Rendering not checked. |
| R34 | 49 | Eframe's accessibility integration is enabled | reproduced-here | PROBE C (static): `generated/Cargo.toml` enables eframe feature `accesskit` | Feature flag only. Behaviour not tested. |
| R35 | 49 | Accessibility has not been qualified | not-attempted | none | Negative claim; not tested. |
| R36 | 51-53 | Verified on the assistant's own Linux computer | not-attempted | none | This re-run is on macOS arm64 (JS header in `js-tests.txt`). |
| R37 | 53 | Vue compiler 3.5.43 and Babel parser 7.29.9 | reproduced-here | PROBE D (installed `node_modules` versions) | |
| R38 | 53 | Rust 1.99.0 | reproduced-here | PROBE D (`rustc 1.99.0` via `scripts/env.sh`); `rust-toolchain.toml` pins 1.99.0 | |
| R39 | 53 | eframe 0.36.2 (and wasm-bindgen 0.2.129 in the WASM notes) | reproduced-here | PROBE C (`generated/Cargo.lock`: eframe 0.36.2, wasm-bindgen 0.2.129) | Lock file only. |
| R40 | 53 | The npm `latest` version was checked on October 9, 2026 UTC | not-attempted | none | Needs registry access; not run. |
| R41 | 55 | 25 compiler tests passed, including unsupported syntax, type mismatch and escaping controls | reproduced-here | JS#2 (25/25); JS#8 (`npm test` 25/25); PROBE F (the test names match ORIG `evidence/compiler-tests.txt`) | |
| R42 | 56 | Four Rust runtime tests passed | not-attempted | ORIG `evidence/rust-tests.txt`; WP-G0D: 4 passed, 0 failed (`rust-counter-test.txt`) | Not run by WP-G0A. |
| R43 | 57 | Runtime tests exercise independent state, computed output, real egui paint shapes and synthetic pointer clicks | not-attempted | static: tests `components_have_independent_state`, `state_and_computed_are_native_rust`, `headless_renderer_produces_real_paint_shapes`, `real_pointer_click_updates_ref_and_conditional_rendering` in `generated/tests/runtime.rs`; WP-G0D: PASSED | Not run by WP-G0A. |
| R44 | 58 | Six clicks update count and computed output and reveal conditional content; Reset clears both | not-attempted | static: test `real_pointer_click_updates_ref_and_conditional_rendering`; WP-G0D: PASSED | Not run by WP-G0A. |
| R45 | 59 | Linux executable compiled successfully | not-attempted | ORIG `evidence/verification.txt` L6 | No Linux host here. WP-G0D built a macOS arm64 executable. |
| R46 | 60 | WASM library compiled and wasm-bindgen glue generated | not-attempted | ORIG `evidence/wasm-build.txt`; WP-G0D: library PASSED, glue NOT RUN | |
| R47 | 62 | Chromium launch failed; no browser playback, desktop-window or mobile run is claimed | not-attempted | WP-G0D: browser run NOT RUN | Not re-attempted here. |
| R48 | 64-65 | `generated/` contains the exact tested example | reproduced-here | PROBE C: `compile(examples/Counter.vue)` equals `generated/src/app.rs`; the CLI's five generated files are identical to `generated/` | |
| R49 | 64, 68 | `generated/` contains its four Rust tests | reproduced-here | PROBE C (static count of `#[test]` in `generated/tests/runtime.rs`: 4) | The tests were not run by WP-G0A (see R42). |
| R50 | 66-68 | `cargo test --locked --manifest-path generated/Cargo.toml` runs them | not-attempted | WP-G0D: 4 passed, exit 0 (`rust-claims.md`) | Not run by WP-G0A. |
| R51 | 70 | `evidence/verification.txt` records the completed checks and limits | reproduced-here | static: `tests/fixtures/prototype/counter/evidence/verification.txt` lists checks and limits | |
| R52 | 74-82 | Web build commands produce a browser-runnable app | not-attempted | WP-G0D: WASM library PASSED; wasm-bindgen glue and browser run NOT RUN | |
| R53 | 82 | The CLI version of wasm-bindgen must match the locked crate | not-attempted | none | Not run. |
| R54 | 88 | Linux: executable builds; headless UI/event tests pass; visible window untested | not-attempted | ORIG `evidence/verification.txt`; WP-G0D: no Linux run | |
| R55 | 89 | Web: WASM and glue build; browser run blocked by this environment | not-attempted | WP-G0D: WASM library PASSED; glue NOT RUN | |
| R56 | 90 | Windows: eframe host path generated | reproduced-here | PROBE C: the generated `src/main.rs` calls `eframe::run_native` | Generated code only. |
| R57 | 90 | Windows: no cross-platform build or run done | not-attempted | none | Not built or run by WP-G0A. |
| R58 | 91 | macOS: eframe host path generated | reproduced-here | PROBE C (same `src/main.rs`) | Generated code only. |
| R59 | 91 | macOS: no macOS build or run done | not-attempted | WP-G0D built the counter executable on macOS arm64 and ran it for 5 s (liveness only): `rust-counter-native-build.txt`, `rust-counter-native-run.txt` | This row is contradicted on this machine by WP-G0D's build and run. Not verified by WP-G0A. |
| R60 | 92 | iOS: no host, packaging or validation implemented | reproduced-here | PROBE F (name search finds no iOS, Android, Xcode or Gradle files) | Name search only. |
| R61 | 93 | Android: no host, packaging or validation implemented | reproduced-here | PROBE F (same search) | Name search only. |
| R62 | 106 | The slice does not establish performance superiority | not-attempted | none | No performance measurement was taken. |
| R63 | 115-116 | The bundle includes no third-party binary, vendored dependency or affiliation claim | not-attempted | none | Bundle contents not audited by WP-G0A. |

## B. evidence/verification.txt (root)

| # | Line | Claim | Status | Evidence | Note |
|---|---|---|---|---|---|
| V1 | 1 | Verification run dated 2026-10-09 UTC | not-attempted | none | Historical date of the original run. These re-runs are dated in `js-tests.txt`. |
| V2 | 2 | Source `examples/Counter.vue` goes through `compiler/compile.mjs` to `generated/src/app.rs` | reproduced-here | PROBE C (`compile(Counter.vue)` equals `generated/src/app.rs`) | |
| V3 | 3 | `npm test`: 25 passing tests | reproduced-here | JS#8 (25 pass, 0 fail) | |
| V4 | 4 | `cargo test --manifest-path generated/Cargo.toml`: 4 passing runtime tests | not-attempted | WP-G0D: 4 passed (`rust-counter-test.txt`) | |
| V5 | 5 | `cargo check --manifest-path generated/Cargo.toml`: passed | not-attempted | none | Neither WP ran `cargo check`. |
| V6 | 6 | Linux native executable built by `cargo test` (`generated/target/debug/vue-rust-demo`) | not-attempted | none | No Linux host. WP-G0D built to `target/g0d`, not the same path. |
| V7 | 7 | `cargo build --lib --target wasm32-unknown-unknown`: passed | not-attempted | WP-G0D: PASSED (`rust-counter-wasm-build.txt`) | |
| V8 | 8 | wasm-bindgen 0.2.129 generated web JS glue and wasm | not-attempted | WP-G0D: glue NOT RUN | |
| V9 | 9 | Browser smoke: NOT PASSED | not-attempted | WP-G0D: browser run NOT RUN | Negative claim; not re-attempted. |
| V10 | 10 | Native visible window: NOT TESTED | not-attempted | WP-G0D: liveness only, "no window was checked" | Consistent with that report. |
| V11 | 11 | Other operating systems: NOT TESTED | not-attempted | none | Negative claim; no other OS available. |
| V12 | 11 | Mobile hosts are not implemented | reproduced-here | PROBE F (name search finds no iOS or Android host files) | Name search only. |
| V13 | 12 | No threads or subagents used; no OR3 repository, production system or user's Mac changed | not-attempted | none | Process and environment claim; not verifiable from the repository. |

## Summary

- Rows: 63 README (R1 to R63) and 13 verification.txt (V1 to V13), 76 in total.
- reproduced-here: 35 rows (32 README, 3 verification.txt). Each is backed by a JS run, a probe case or a static check named above.
- not-reproduced: 2 rows, R21 (number interpolation text) and R31 (unsupported constructs are never silently ignored). Both are backed by `KNOWN BUG (prototype)` tests, which assert the defect and pass today.
- not-attempted: 39 rows (29 README, 10 verification.txt): Rust, native, WASM, browser, network or process claims. Where WP-G0D re-ran a Rust claim its result is cited. That result is not counted as WP-G0A verification.
- Two README statements conflict with the current state: R31 (silent acceptance of `@[click]`) and R59 (no macOS build or run, which WP-G0D has since performed on this machine).

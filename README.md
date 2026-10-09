# vue-native

vue-native compiles Vue 3 single-file components (`<script setup>`, JavaScript or TypeScript, Composition API) to Rust. The generated crate runs on a JavaScript value runtime written in Rust (`vn-value`) and is built either as WebAssembly for the browser or as a headless binary for scripted runs. The output embeds no JavaScript engine, webview or `eval`.

**Status: experimental, not production.** The web target works in Google Chrome and is tested there. The native window renderer, layout, accessibility, mobile hosts and Windows or Linux desktop windows are not built or not qualified. See [Limits](#limits).

Vue's own SFC compiler (`@vue/compiler-sfc` 3.5.43) runs at build time. Vue 3.5.43 reactivity and runtime behaviour is ported to Rust and checked against the real `vue@3.5.43` package.

## How it works

```text
App.vue
  -> @vue/compiler-sfc (official): script setup, template render function, styles
  -> profile check: only the documented language and Vue API subset is accepted
  -> compiler/frontend, compiler/analysis: parse and scope analysis
  -> compiler/hir: lowering to a high-level intermediate representation
  -> compiler/emit: Rust source for the generated crate
  -> cargo, with the vn-* runtime crates in crates/
  -> wasm32 web bundle (wasm, vn-host.js, index.html) or a headless binary
```

Vue's own JavaScript runtime is not compiled. Its behaviour is reimplemented in the `vn-*` crates ([architecture](docs/dev/ARCHITECTURE.md)). Unsupported syntax and APIs fail at compile time with a diagnostic that has a code and a source span. The profile is closed: a name that is not in the compatibility manifest is rejected.

| Crate | Role |
|---|---|
| `vn-value` (`crates/value`) | JavaScript value runtime: UTF-16 strings, ECMAScript number semantics, an object heap with collection at job boundaries, functions, promises and async frames, built-ins and ES modules. |
| `vn-reactivity` (`crates/reactivity`) | Port of `@vue/reactivity` 3.5.43: effects, batching, computed values, effect scopes, refs, reactive and readonly proxies. |
| `vn-runtime` (`crates/runtime`) | Port of Vue 3.5.43 `runtime-core` and `runtime-dom`: mount, patch and unmount, keyed diff, components, lifecycle, directives and the app API. Drives a DOM-like host. |
| `vn-dom` (`crates/dom`) | In-memory DOM-like document: the host for headless runs and tests. |
| `vn-css` (`crates/css`) | Tokenizer, parser, selectors, media queries and cascade for the documented CSS subset. |
| `vn-ui` (`crates/ui`) | Typed CSS values, shorthand expansion and computed style per element. No layout or painting yet. |
| `vn-uikit` (`crates/uikit`) | `testHost` for the component scenarios, over the in-memory document. |
| `vn-renderer-dom` (`crates/renderer-dom`) | Browser host: `WebHost` over a hand-written wasm32 import and export ABI ([ABI](hosts/web/ABI.md)). |
| `vn-testkit` (`crates/testkit`) | Rust side of the differential tests: encodes values in the Node oracle's trace format and runs scenarios. |

The `vn-*` crates use no external crates; `Cargo.lock` lists only workspace packages.

## What works today

**Vue API surface.** The [compatibility manifest](docs/compatibility/README.md) classifies all 171 runtime exports of `vue@3.5.43` (seven of them are also compile-time macros). Each export has one status:

| Status | Meaning in the manifest | Exports |
|---|---|---|
| P | Faithful: planned behaviour within the profile | 66 |
| A | Adapted: explicit native or host adaptation | 9 |
| W | Web only | 4 |
| E | Experimental: not promised for production | 2 |
| X | Excluded: rejected with a diagnostic | 90 |

The compiler accepts P and A names on both targets and W names on the web target only. It rejects X, E and unlisted names. The letters describe what the compiler permits, not what is tested. For example, `createSSRApp` is W, but server rendering and hydration are not built. The full list is in the [Vue export audit](docs/compatibility/vue-exports-audit.md).

**Components, renderer and DOM.** From [STATUS](docs/dev/STATUS.md):

- Mount, patch and unmount; keyed diff; props, emits and slots; lifecycle hooks; refs; error handling; directives; app API.
- Teleport, Transition, KeepAlive and async components pass the recorded corpus of 160 component scenarios, recorded against real Vue in Chrome. Broader qualification is still partial (see [Limits](#limits)).
- DOM tree, form state, events, selectors and styles; CSS parsing, cascade, custom properties, typed values and computed style for the documented subset.

**Single-file components and projects.**

- `<script setup>` in JavaScript or TypeScript (`lang="ts"`). TypeScript syntax is erased; there is no type checking.
- Scoped `<style>` blocks, within the documented CSS subset.
- Multi-file projects. Relative imports name the exact file, extension included (`./money.ts`, `./FieldInput.vue`). Only the `vue` package can be imported.

**Browser delivery and tools.**

- `build --target web` writes a content-hashed `.wasm`, `vn-host.js`, `index.html`, `manifest.json`, and a CSS file when the project has styles. Counter and Quick Quote run through the Rust Vue runtime in Chrome, and browser tests cover their interactions. Browser ABI: [web-app-entry](docs/guide/web-app-entry.md).
- `build --target headless` writes a native binary that mounts the app in the in-memory document and runs a JSON interaction script.
- `create`, `check` and `doctor` complete the CLI.

### Scoreboard

Recorded in [docs/dev/STATUS.md](docs/dev/STATUS.md) on 2026-10-09:

| Suite | Result |
|---|---|
| Rust workspace (`cargo test --workspace`) | 1323 passed, 0 failed (1327 on the Linux re-run) |
| Language scenarios, lowering, multi-file projects, SFC fixtures | 79/79, 166/166, 10/10, 37/37 |
| Vue runtime against real `vue@3.5.43` (`npm run test:vue`) | 102/102 (112/112 on the Linux re-run after the API additions) |
| Component scenarios against recorded Vue output in Chrome | 160/160, also with GC at every safepoint (165/165 on the Linux re-run) |
| CLI (`tests/cli`) | 40/40 |
| Browser host and compiled apps (`tests/platform/web`) | 299 passed, 0 failed, 109 skipped with stated reasons |

## Platform status

| Target | Status |
|---|---|
| Web (compiled app in Google Chrome) | Built and tested. Browser suites need Chrome. |
| Headless binary | Built and tested. Runs a script against the in-memory document; there is no window. |
| Native desktop window | Not built. `check --target native` validates a project against the native profile only. |
| Layout, painting, accessibility | Not built. |
| iOS and Android | No host: `hosts/` contains only `web`. |
| Safari, Firefox, Windows and Linux hardware | Not qualified. |

## Limits

- Experimental. The manifest statuses are not a promise of full Vue behaviour. STATUS lists qualification still pending: block-stack unwinding in `withCtx` and render errors; keyed move-count comparisons; TransitionGroup, in-out transition mode, async retry and hydration, and nested or racing component cases.
- No native window renderer, layout, painting or accessibility adapter. `build` supports only the web and headless targets.
- No SSR, hydration or services (files, settings, SQLite, HTTP). No packaging or signing.
- No TypeScript type checking, no npm imports other than `vue`, no runtime template compiler, and no template `|` filter syntax.
- Browser qualification is Chrome only.
- No development reload: rebuild after each edit.
- Generated crates refer to this checkout's `crates/` by absolute path, so they are not self-contained.
- The standard-library allowlist is incomplete. Only named constructors and objects are listed, and other methods are unsupported until they are listed ([allowlist](docs/compatibility/stdlib.md)).
- The counter example builds to about 1.4 MB of wasm (460 KB gzipped) with the size-tuned release profile; there is no `wasm-opt` pass and unused built-ins are still linked.

## Quick start

Prerequisites:

- Node.js 22 or later (`.nvmrc`). The docs record results on Node 26.5.1.
- Rust 1.99.0, pinned in `rust-toolchain.toml`, with the `wasm32-unknown-unknown` target for web builds (`rustup target add wasm32-unknown-unknown`). `source scripts/env.sh` puts a project-local toolchain under `toolchain/` on your `PATH`.
- Google Chrome, only for the browser tests. Set `CHROME_PATH` if it is not found.

Run these from the repository root. The example app is created next to the checkout (`../my-app`), so the repository stays clean.

```sh
npm ci --ignore-scripts
node tooling/cli/vue-native.mjs doctor                 # reports missing pieces; installs nothing
node tooling/cli/vue-native.mjs create ../my-app --template counter
node tooling/cli/vue-native.mjs check ../my-app        # exit 0 when there are no errors
node tooling/cli/vue-native.mjs build ../my-app --target web --out ../dist-web
python3 -m http.server 8080 --directory ../dist-web    # then open http://localhost:8080
```

- `--out` must not exist yet, unless vue-native wrote it before, in which case it is replaced.
- The first web build compiles the runtime crates for wasm32 and takes several minutes. Cargo output goes to `target/vue-native` in the checkout unless `CARGO_TARGET_DIR` is set.
- Serve the output over HTTP. Do not open `index.html` from disk.
- To start from the Quick Quote example instead, add `--template quick-quote` to `create`.

Headless run, with no browser. This builds the counter example and runs a script against it:

```sh
node tooling/cli/vue-native.mjs build examples/counter --target headless --out target/counter-headless
target/counter-headless/counter script.json
```

The script is an object with a `steps` array:

```json
{"steps": [
  {"op": "click", "selector": "#increment"},
  {"op": "html", "selector": "#count"}
]}
```

Steps are `click`, `input` (sets a value and fires `input`), `html` (the markup of one element) and `snapshot` (the markup of the mount container). Selectors are `#id`, `.class`, `tag`, `[attr=value]` and `*`, combined without spaces, with comma lists; descendant selectors are refused. The binary prints one JSON array with a result per step. Exit status is 0 on success, 1 when the app throws or a step fails, and 2 for a usage or script error.

## Running the tests

```sh
npm test                 # node --test "tests/**/*.test.mjs": compiler, CLI, profile, browser and tool suites
cargo test --workspace   # the vn-* Rust crates
npm run test:language    # language scenarios: compile to Rust, run, compare with golden output
npm run test:vue         # the same scenarios against the Rust Vue runtime
```

Browser suites need Google Chrome. Without it they are reported as skipped, with the reason `Google Chrome not found; install it or set CHROME_PATH`. Some suites run cargo builds and take several minutes.

CI (`.github/workflows/ci.yml`) runs `cargo test --workspace`, `npm test` and the language, Vue and UI harnesses on Linux for every pull request.

## Repository layout

| Path | Contents |
|---|---|
| `compiler/` | Node compiler: SFC frontend, profile checks, analysis, HIR lowering, Rust emission and the project graph. [`ABI.md`](compiler/ABI.md) describes the emitted module. |
| `crates/` | The nine `vn-*` Rust crates. |
| `hosts/web/` | Browser host glue (`vn-host.js`) and the wasm ABI notes. |
| `tooling/` | `cli` (the `vue-native` command), `harness` (scenario runner), `oracle` (Node and real-Vue oracles), `chrome` (Chrome driver), `manifest` (compatibility manifest generator), `web` (wasm build), `sandbox`, `status`. |
| `templates/` | Starter projects for `create`: `counter` and `quick-quote`. |
| `examples/` | The `counter` and `quick-quote` apps used by tests and docs. |
| `tests/` | Suites by area (`compiler`, `language`, `vue`, `ui`, `dom`, `css`, `profile`, `cli`, `platform/web`, `security`, `tooling`). `fixtures/prototype` is a preserved copy of the removed first prototype. |
| `docs/` | Guides, compatibility manifest and audit, developer status and architecture, evidence. |
| `planning/` | Requirements, design and task list. |
| `release/` | Dependency policy. |
| `scripts/` | `env.sh` (project toolchain on `PATH`) and `mirror.sh` (private repository copies for concurrent builds). |

## Documentation

- [Getting started](docs/guide/getting-started.md): CLI commands, headless and web builds, known limitations.
- [Compiled browser app entry](docs/guide/web-app-entry.md): the browser host ABI used by generated apps.
- [Status](docs/dev/STATUS.md): dated scoreboard, what is built and the honest limits.
- [Architecture](docs/dev/ARCHITECTURE.md): the pipeline and engineering decisions.
- [Compatibility manifest](docs/compatibility/README.md) and [Vue export audit](docs/compatibility/vue-exports-audit.md); [standard-library allowlist](docs/compatibility/stdlib.md).
- Evidence: [advanced components](docs/evidence/advanced-components.md) and [web apps](docs/evidence/resume-web-apps.md).
- Scope: [requirements](planning/vue-native-framework/requirements.md), [design](planning/vue-native-framework/design.md) and [tasks](planning/vue-native-framework/tasks.md).

## License and provenance

No license has been chosen. `Cargo.toml` sets `license = "UNLICENSED"`, and `package.json` declares no license.

Vue (`vue@3.5.43`), `@vue/compiler-sfc`, `@vue/compiler-dom` and `@babel/parser` are third-party npm packages. Their licenses and notices stay with them. The compiler calls the official SFC and template compilers at build time. `vue` is a development dependency that serves as the test oracle. Vue's runtime is reimplemented in `crates/` rather than shipped as JavaScript. This project does not claim any affiliation with Vue.

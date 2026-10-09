# Vue Rust Lab — first working slice

You write `.vue` components. This prototype generates Rust UI code from them. The native app does **not** embed a browser or JavaScript engine. The same generated Rust also builds to WebAssembly.

**This is an experimental compiler, not a completed rewrite of Vue.** Do not use it for OR3 or production yet. Vue's official JavaScript parser is used at build time; the compiler frontend itself has not been rewritten in Rust. The generated application and its renderer are Rust.

## Try it

Install Node.js and the stable Rust toolchain from their official sites. Then, inside this folder:

```sh
npm ci
npm test
npm run build:demo
npm run native
```

Edit `examples/Counter.vue`. To generate another build without overwriting anything:

```sh
node compiler/cli.mjs examples/Counter.vue build/counter-2
cargo run --manifest-path build/counter-2/Cargo.toml
```

The compiler refuses an existing output directory so it cannot overwrite your files. You do not need to write Rust. Cargo is currently the build tool behind the prototype; hiding it behind a polished CLI is future work.

Linux requires a working graphical session and system libraries expected by eframe. A Windows or Mac build requires that platform's Rust/linker setup. Those two platforms are not tested here.

## What works

- `<script setup>` with normal JavaScript, optionally inside `lang="ts"`
- `import { ref, computed } from 'vue'`
- Primitive number, string and boolean refs; each component instance has its own state
- Pure expression computed values; `.value` inside setup and automatic unwrapping inside templates
- Zero-argument synchronous event functions, ref assignment and increment/decrement
- Text interpolation, arithmetic, strict comparisons and boolean conditions
- `<main>`, `<section>`, `<div>`, `<h1>`, `<h2>`, `<p>`, `<span>`, `<label>`
- `<button @click="increment">`, inline updates and `:disabled`
- `<input v-model="name" />` for a string ref
- `v-if`
- Rust/egui rendering, with the same generated application usable by the native and WASM hosts

## What does not work yet

CSS, nested custom components, props, emits, slots, lists, reactive objects, watchers, lifecycle hooks, async code, fetch, routing, npm imports, Vue plugins, DOM APIs and native device APIs are not implemented. TypeScript type annotations/generics are rejected; `lang="ts"` is accepted only for the currently supported syntax.

Unsupported constructs produce an error rather than being silently ignored. The supported language is intentionally small. It is **not** general JavaScript or full Composition API compatibility.

`computed` currently emits a pure Rust getter evaluated when the UI reads it. There is no Vue-style cached dependency graph, watcher scheduler or batching yet. Layout currently uses vertical native-rendered groups. These are custom-drawn controls, not OS-owned AppKit/UIKit/WinUI widgets. Eframe's accessibility integration is enabled, but accessibility has not been qualified.

## Verified on the assistant's own Linux computer

Tool versions: Vue compiler 3.5.43 (the npm `latest` version checked October 9, 2026 UTC), Babel parser 7.29.9, Rust 1.99.0, eframe 0.36.2.

- 25 compiler tests passed, including unsupported syntax, type mismatch and escaping controls
- Four Rust runtime tests passed
- Runtime tests exercise independent state, computed output, real egui paint shapes and synthetic pointer clicks through the generated button handlers
- Six clicks update count and computed output and reveal conditional content; Reset clears both count and the conditional content
- Linux executable compiled successfully
- WASM library compiled and wasm-bindgen glue generated successfully

A Chromium launch was attempted but failed because this cloud machine restricts browser sockets. **No browser playback, desktop-window visual check or mobile run is claimed.** Compilation and headless renderer tests are not complete platform qualification.

The `generated/` directory contains the exact tested example and its four Rust tests. To rerun them:

```sh
cargo test --locked --manifest-path generated/Cargo.toml
```

The file `evidence/verification.txt` records the completed checks and limits. Dependency and build outputs are excluded from this source archive.

## Web build

```sh
rustup target add wasm32-unknown-unknown
cargo build --locked --manifest-path generated/Cargo.toml --lib --target wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
wasm-bindgen --target web --out-dir generated/pkg generated/target/wasm32-unknown-unknown/debug/vue_rust_demo.wasm
python3 -m http.server 8080 --directory generated
```

Open `http://localhost:8080`. Use HTTP rather than opening the HTML as a file. The CLI version must match the locked wasm-bindgen crate. Release optimization and asset-size work are still needed.

## Platform status

| Target | Status |
|---|---|
| Linux | Executable builds; headless UI/event tests pass; visible window untested |
| Web | WASM/glue builds; browser run blocked by this environment |
| Windows | eframe host path generated; no cross-platform build or run done |
| macOS | eframe host path generated; no macOS build or run done |
| iOS | No host, packaging or validation implemented |
| Android | No host, packaging or validation implemented |

Do not mistake the renderer's possible targets for supported targets in this prototype.

## How to turn this into the product you asked for

1. **Lock down the compatibility contract.** Pick exactly which Composition API features must behave like Vue, and test equivalent components against real Vue. Do not promise arbitrary npm libraries or browser APIs in a native app.
2. **Build the Rust reactive runtime and typed intermediate representation.** Add dependency tracking, cached computed values, batching, effects and cleanup. Replace the experimental direct code emitter with a checked representation before expanding syntax.
3. **Add components and layout.** Props/emits, slots, keyed lists and stable state ownership come first. Then add a documented CSS subset backed by a layout engine. Arbitrary browser CSS is a much larger commitment.
4. **Qualify desktop and web.** Real window/browser tests, keyboard navigation, screen readers, text input/IME, fonts, clipboard, resize, packaging and crash recovery. No marketing claim until each platform passes.
5. **Build mobile hosts.** iOS/Android need lifecycle, touch, keyboard, safe areas, accessibility, permissions, platform SDKs and signed packaging. Those are substantial separate deliverables.
6. **Make the developer workflow pleasant.** One command, useful source-mapped errors, hot reload, editor support and platform builds. App authors should never need to inspect generated Rust.

This is a multi-stage framework project. The small working slice proves the syntax-to-native-code direction; it does not establish full Vue behavior, performance superiority or all-platform support.

## References and provenance

- Vue script setup: https://vuejs.org/api/sfc-script-setup.html
- Vue rendering model: https://vuejs.org/guide/extras/rendering-mechanism.html
- Current Vue package metadata: https://registry.npmjs.org/vue/latest
- eframe native/web host API: https://docs.rs/eframe/0.36.2/eframe/
- Rust installation: https://www.rust-lang.org/tools/install

Vue and the Rust renderer are third-party dependencies, not rewritten or authored here. Their licenses and notices remain in their packages; consult them before distributing compiled applications. This source bundle includes no third-party binary, vendored dependency or claim of affiliation with Vue.

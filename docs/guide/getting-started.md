# Getting started

This guide covers the `vue-native` command line as it works in this checkout. Run every command from the
repository root. Until `package.json` has a `bin` entry, call the tool as `node tooling/cli/vue-native.mjs`.

## Prerequisites

- Node.js 20 or later (tested with 26.5.1). The tools install nothing; `doctor` reports what is missing.
- The project-local Rust toolchain, Rust 1.99.0 from `rust-toolchain.toml`. Load it in each shell with
  `source scripts/env.sh`.
- The `wasm32-unknown-unknown` target, needed for web builds. If `doctor` reports it missing, run
  `rustup target add wasm32-unknown-unknown`.
- Google Chrome, only for the browser tests (`CHROME_PATH` can name the binary).

```sh
node tooling/cli/vue-native.mjs doctor
```

## Create a project

```sh
node tooling/cli/vue-native.mjs create my-app                    # the counter template
node tooling/cli/vue-native.mjs create my-app --template quick-quote
```

A project is a directory whose entry is `src/App.vue`. Components are `.vue` files with `<script setup>`, in
JavaScript or TypeScript (`lang="ts"`). A relative import names the exact file, extension included (`./money.ts`,
`./FieldInput.vue`), as in Node ESM. Only the `vue` package can be imported.

## Check

```sh
node tooling/cli/vue-native.mjs check my-app             # exit 0 when there are no errors
node tooling/cli/vue-native.mjs check my-app --json      # the full report, for tools
```

`check` compiles the whole project graph and prints each diagnostic with its code. Exit status 1 means the project
has errors; 2 means the command line was wrong. Some lowering errors in a `.vue` file point into generated code, so
`check` prints their position without a source excerpt.

## Build and run headless

```sh
node tooling/cli/vue-native.mjs build examples/counter --target headless --out target/counter-headless
```

`--out` must not exist yet; a directory written by an earlier build can be built into again, and any other
directory is refused. The output holds one native binary, named after the app, and `manifest.json`. The binary
mounts the app in the runtime's in-memory document and runs an interaction script:

```sh
target/counter-headless/counter script.json
```

The script is JSON. Each step is one of:

- `{"op": "click", "selector": "#increment"}`
- `{"op": "input", "selector": "#price-1", "value": "4.99"}`: sets the value and fires `input`
- `{"op": "html", "selector": "#count"}`: the markup of one element
- `{"op": "snapshot", "name": "initial"}`: the markup of the mount container

Selectors are `#id`, `.class`, `tag`, `[attr=value]` and `*`, combined without spaces, with comma lists. Descendant
selectors (`main h1`) are refused with exit status 2. The binary prints one JSON array with a result per step. Exit status is 0 on success,
1 when the app throws or a step fails (for example a selector matches nothing), and 2 for a usage or script error.

## Build for the web

```sh
node tooling/cli/vue-native.mjs build my-app --target web --out dist-web
```

**Status: not available yet.** A web build of a `.vue` project stops with its reason. The runtime renderer is bound
to the in-memory document (`DomCtx::new` takes a `DocHost`), so the browser host cannot be installed under it. The
web packaging is proven with an app that uses only the browser host (`tests/cli/fixtures/host-only`): it produces
`<name>-<hash>.wasm`, `vn-host.js`, `index.html` and `manifest.json`, and runs in headless Chrome.

## Known limitations

- `examples/quick-quote` does not build yet: its `FieldInput.vue` uses `defineModel`, which needs the runtime's
  `useModel`, and the `vue` module does not export it.

## Tests

```sh
node --test "tests/cli/*.test.mjs" "tests/platform/web/e2e-*.test.mjs"
```

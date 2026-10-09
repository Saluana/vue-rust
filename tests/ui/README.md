# UI scenarios and goldens (WP-V1)

Component-level differential tests. Each `cases/<group>/<name>/scenario.js` runs in headless Chrome against the real
`vue@3.5.43` production build (`node_modules/vue/dist/vue.global.prod.js`), with a real DOM. Its golden,
`golden/<group>/<name>.json`, is the trace document (`docs/dev/trace-format.md`) on one line. The contract is
`docs/dev/ui-scenario-host.md`; the runner is `tooling/oracle/ui/`.

## Commands (repository root)

```
node tooling/oracle/ui/check-goldens.mjs                 # every case against its golden (exit 1 on mismatch or orphan)
node tooling/oracle/ui/check-goldens.mjs --filter events/ # a subset
node tooling/oracle/ui/check-goldens.mjs --update         # rewrite goldens (regeneration is byte-identical)
node tooling/oracle/ui/lint.mjs                           # header, imports, testHost members, language rules, compile
node --test "tests/ui/oracle.test.mjs"                    # WP-V1 acceptance (about 2 minutes; needs Google Chrome)
```

## Case layout and header

- Line 1 `// @tier N`: 1 synchronous; 2 observed after `await testHost.tick()` or `advance`; 3 promise, timer or flush ordering.
- Line 2 `// @covers U14,V18`: manifest rows the case observes (U01..U32, V01..V29, S01..S25, A01..A19, all checked against
  `compiler/profile/manifest.json`).
- Components (`*.vue`) live in the same directory and are imported as `./X.vue`. Every `.vue` file of a case is compiled
  with `compileSfc` (target `native`) before Chrome starts; a diagnostic fails the case.
- Imports: `'vue'` names the manifest supports for the native target, and `./X.vue` only.

## Runtime (what a run does)

A fresh Chrome profile and page per case. A loopback server serves the page, `vue.global.prod.js`, a `vue` module that
re-exports the 171 manifest names of the global build, the compiled components, `scenario.js` and the page host from
`tooling/oracle`. The page installs the fake clock (`setTimeout`, `clearTimeout`, `requestAnimationFrame`, `Date.now`,
`performance.now`; `setInterval` throws), `trace` (the same `tooling/oracle/trace.mjs` as Node) and `testHost`, then runs
trace-format.md steps 1 to 5. An uncaught page error fails the run.

## testHost (as implemented)

- `mount(component, props?, { errorHandler? })` returns a `Mount`. Each app gets a container `div#app-N` in `body`. Props are
  copied shallowly (values keep identity). Mounted through a host component, so `setProps` re-renders through Vue's own update.
- `Mount`: `snapshot()` (array of the container's child nodes), `html()` (container `innerHTML`), `unmount()`,
  `setProps(next)` (**replaces** the whole props object; a prop left out becomes absent). `Mount.root` is not implemented.
- `NodeHandle`: `text()`, `html()`, `attr(name)` (string or null), `prop(name)` (primitives only; an object throws
  TypeError), `exists()` (still connected), `snapshot()`.
- `find(scope, sel)` / `findAll(scope, sel)`: scope is a `Mount` or a `NodeHandle`. Selector grammar: one compound selector,
  `tag? (#id | .class | [attr] | [attr=value] | [attr="value"])* (:nth-child(n))?`. Anything else throws TypeError.
- `container(id)`: the element with that id, created under `body` when missing.
- `click(node)`: `MouseEvent` click, `detail` 1. `dispatch(node, type, init?)`: init keys `bubbles, cancelable, key, code,
  ctrlKey, shiftKey, altKey, metaKey, button, detail, data, isComposing`; other keys throw. Defaults follow the browser: focus,
  blur, mouseenter, mouseleave, load, error and scroll do not bubble; input, change, focus, blur, composition and mouseenter
  events are not cancelable.
- `input(node, value)`: text input, range or textarea; sets `value`, dispatches `input` (InputEvent, bubbling).
- `change(node, value)`: text, range, textarea (sets value) or select (as `select`); dispatches `change`.
- `select(node, value)`: single select takes a string; multiple takes an array (selects matching options); dispatches `change`.
- `check(node, bool)`: checkbox or radio; performs `el.click()` when the state differs (so click, input and change fire);
  a radio cannot be unchecked (TypeError); throws if the control did not change (for example disabled).
- `keydown(node, key, mods?)`, `focus(node)`, `dispatch` as above. `tick()`: two macrotask turns (job queue drained).
- `timers`: `setTimeout`, `clearTimeout`, `now`, `advance` (same clock as the page globals).

## Snapshot (tooling/oracle/ui/snapshot.mjs)

Canonical elements follow ui-scenario-host.md with the key order of crates/dom/src/serialize.rs (`tag`, `ns` for svg/math,
`attrs`, `props`, `children`). `props` lists value (input, textarea), checked (input), selected (option) and disabled (the
reflecting elements) only when the property differs from the value the attributes give. A select has no value prop.
`props` is written as an array of `[key, value]` pairs as in ui-scenario-host.md; serialize.rs writes an object.

## Facts the oracle recorded (for the Rust runtime)

- Fragment anchors of a `v-for` root are empty text nodes (`{"$":"text","v":""}`); an empty `<template>` renders a comment
  `<!---->`; a failed `v-if` renders `<!--v-if-->`; a render error renders `<!---->` in place of the component.
- Vue 3.5 mirrors bound `checked`, `selected` and `value` into their attributes as well as the properties (`patchAttr`).
- `v-show` leaves `style=""` once shown (`display` cleared), and `style="display: none;"` while hidden.
- Template comments survive the production compile (`compileSfc` passes no `comments` option).
- Production error `info` values are URLs (`https://vuejs.org/error-reference/#runtime-N`; 0 setup, 1 render, 5 native event).
- `.middle` and `.right` compile to `mouseup` and `contextmenu`; `.left` to `click`.
- KeepAlive with `max` 1 unmounts the evicted instance without a deactivated hook.
- A `v-model` reads its array at the time of the change; synchronous calls with no flush in between see stale arrays.

## Not covered and why

Components in TypeScript (`lang="ts"` is not transpiled by the oracle; no `typescript` package is installed), `Suspense`
(E), `onServerPrefetch` (W), `v-html` (X), custom `v-model` modifiers (rejected by the native profile), cross-window Teleport,
CSS-timed transitions (only with no stylesheet), and `dispatch` on disabled controls (Chrome runs the listeners of a
dispatched event there; no case relies on that).

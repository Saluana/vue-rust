# UI scenario host (extends trace-format.md v1)

Component-level differential tests run the **same compiled JavaScript** — the output of `compiler/sfc` (official Vue compiler + our
options) for every `.vue` file, and the scenario module itself — under two runtimes and compare traces:

* **Oracle:** real `vue@3.5.43` (`vue.global.prod.js`) in headless Chrome (`tooling/chrome`), real DOM and real `runtime-dom`.
* **Ours:** the compiled Rust runtime (`vn-runtime`) over `vn-dom::Document`.

Sharing the compiled JS isolates what is being tested: the runtime + host semantics, not the compiler options.

## Scenario files
```
tests/ui/cases/<group>/<name>/
  scenario.js          // ES module, same contract as trace-format.md (run(), trace(), testHost)
  *.vue                // components imported by relative path:  import Counter from './Counter.vue'
```
`scenario.js` may import `'vue'` (all APIs allowed by the manifest) and `.vue` files. It must stay inside the language profile.
Header comments: `// @tier N`, `// @covers U14,V18`.

## `testHost` additions
- `testHost.mount(component, props?) -> Mount` — creates a fresh container (`<div id="app-N">`) in the host document and mounts
  `createApp(component, props).mount(container)`. Several mounts per scenario are allowed (independent apps).
- `Mount.snapshot() -> object` canonical element tree of the container's *children* (see below), already traceable plain data.
- `Mount.html() -> string` serialised HTML of the container's inner content (HTML5 serialisation, attributes in insertion order).
- `Mount.unmount()`, `Mount.setProps(next)` (re-render the root with new props through a wrapper that holds reactive props),
  `Mount.root` (the root component's public instance as exposed via `defineExpose`/template ref — optional, only if needed by a test).
- `testHost.find(mount, selector) -> NodeHandle | null` (selector grammar = the `query_selector` subset of vn-dom: `tag`, `#id`, `.class`,
  `[attr]`, `[attr=value]`, compound without combinators, plus `:nth-child(n)` suffix), `findAll(mount, selector) -> NodeHandle[]`.
- `NodeHandle` methods: `text()`, `html()`, `attr(name)`, `prop(name)`, `exists()`, `snapshot()`.
- `testHost.click(node)`, `testHost.input(node, value)` (sets the `value` property then dispatches a bubbling `input` event),
  `testHost.change(node, value)`, `testHost.check(node, bool)` (sets `checked` and dispatches `click`-equivalent activation as a browser would —
  for checkbox/radio use a real `click` event), `testHost.select(node, value)`, `testHost.keydown(node, key, mods?)`, `testHost.focus(node)`,
  `testHost.dispatch(node, type, init?)` with `init = { bubbles, cancelable, key, code, ctrlKey, shiftKey, altKey, metaKey, button, detail, data, isComposing }`.
  All of these are synchronous and then **do not** flush Vue's scheduler; scenarios call `await testHost.tick()` (which also awaits `nextTick()`).
- `testHost.timers` and `testHost.tick()` as in trace-format.md. Event timestamps and `Date.now()` used by `runtime-dom`'s event invoker
  are driven by the fake clock (`now()`); the oracle page overrides `Date.now`/`performance.now` with the same fake clock.

## Canonical element snapshot
```
{"$":"el","tag":"div","attrs":[["id","a"],["class","b c"]],"props":[["value","x"]],"children":[ … ]}
{"$":"text","v":"hello"}      {"$":"comment","v":"…"}
```
- `tag` lowercase local name; `attrs` = all attributes in insertion order *as stored* (the oracle reads `el.attributes`); `style` appears as the `style`
  attribute exactly as serialised by the engine (scenarios only use non-normalising values such as `display: none`, `width: 10px`, `--x: 1`).
- `props` appears only for `value`, `checked`, `selected`, `disabled` and only when the property differs from the value derived from the attributes
  (dirty value/checkedness). Order: value, checked, selected, disabled.
- Adjacent text nodes are NOT merged (Vue creates separate nodes); empty text nodes are kept.
- Fragment anchors are comment nodes `<!---->` (empty comment) and, for `v-if` false branches, `<!--v-if-->`: they are part of the snapshot — they are
  Vue behaviour (production build emits `<!---->` for empty `v-if`; verify with the oracle, do not guess).

## Comparison
Same as trace-format.md: structural JSON equality of the traces. Additionally scenarios may call `trace('ops', testHost.opLog())` only in
the **headless** (ours) host for debugging; `opLog` is not available in the oracle and such calls are forbidden in committed scenarios.

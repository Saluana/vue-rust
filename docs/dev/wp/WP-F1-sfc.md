# WP-F1 — SFC frontend: `.vue` → profile-checked JS modules + style data (pure JavaScript)

Read `docs/dev/AGENT-CONTRACT.md`, `docs/dev/ARCHITECTURE.md` §1, and skim `compiler/frontend`, `compiler/analysis`, `compiler/diagnostics`
(WP-C1a, done) and `compiler/profile/index.mjs` (WP-G0B, done: `lookup`, `isSupported(kind,name,target,{origin})`).
WP id `f1`; no cargo. **Owned paths:** `compiler/sfc/**`, `tests/compiler/sfc/**`, `tests/sfc/**`.
Allowed dependencies: only the installed `@vue/compiler-sfc`, `@vue/compiler-dom`, `@babel/parser`, `vue` (3.5.43, for the browser build used as oracle).

## Why
`.vue` files are the authoring format. The official compiler already implements macros (`defineProps`, `defineModel`, `withDefaults`, props
destructure, …) and template transforms. We **reuse its output** (JS that imports runtime helpers from `'vue'`) and put around it: the closed-world
profile checks, native-target template validation, style extraction, dependency tracking and `.vue`-mapped diagnostics. The JS that comes out is
what the Rust lowering compiler consumes later, so it must stay in the language profile (generated code may use `compilerInternal` helper imports; the
user's own code may not).

## Deliver `compiler/sfc/index.mjs`
`compileSfc(source, { filename, id?, target: 'native'|'web', rootDir? }) → { code, map, styles, bindings, diagnostics, dependencies, scopeId }`
(`id` defaults to a stable hash of the filename relative to `rootDir`; deterministic — no timestamps, no absolute paths in output).

1. **Parse** with `@vue/compiler-sfc` `parse` (source maps on); convert parse errors to diagnostics with `.vue` spans.
2. **Block policy** (diagnostic codes `VN6xxx`, register them in a new `compiler/sfc/codes.mjs` that extends `compiler/diagnostics/codes.mjs` without editing it):
   `<template>` (`lang` absent/`html`; `src` allowed: read the file with a project-root escape check (realpath) and add to `dependencies`), at most one `<script setup>`
   (`lang` absent/`js`/`ts`; `src` rejected) plus at most one normal `<script>` whose only allowed default export is an object literal with `name`/`inheritAttrs`
   (anything else → Options API diagnostic S02/A18 with hint), `<style>` blocks (`lang` absent/`css`; `scoped`, `module`, `src` allowed; `scss/sass/less/stylus/postcss` and
   custom blocks → S04 diagnostics; `v-bind()` in CSS allowed), no `<script>` at all is fine (static template component).
3. **Script**: call `compileScript` with `inlineTemplate: true`, `isProd: true`, source maps, TypeScript plugins when `lang="ts"`, and the *native-validated*
   template options below. Components without `<script setup>` but with a template compile the template with `compileTemplate` (module mode) into a
   `render` and export `{ render }` (document the exact shape). Props/emits/models macros must work for both runtime and type-based declarations — rely on
   the official compiler; for type-based props from imported types it needs `fs` access: pass a project-root-limited `fs`.
4. **Template options** (document each and why): `mode:'module'`, `prefixIdentifiers:true`, `hoistStatic:false` (we do not support `createStaticVNode`/`insertStaticContent`
   from HTML strings), `cacheHandlers:true`, `whitespace:'condense'`, `isNativeTag` from the manifest's native tag set, `isCustomElement: () => false`,
   plus a `nodeTransform` that validates every element: the tag must be a native tag, an imported/`<script setup>` binding, or a built-in (`component`, `slot`, `template`,
   `transition`, `transition-group`, `keep-alive`, `teleport`; `suspense` → E diagnostic); unknown tags → diagnostic with hint; `v-html` → U29 diagnostic (native) / allowed only
   for `target:'web'` with `trustedHtml` option; `.passive` modifier → U06 diagnostic; `<input type="file|date|…">` per U10 → diagnostic with hint; dynamic event
   argument is allowed (it is compiled by Vue as `toHandlerKey`), `v-once`/`v-memo`/`v-pre`/`v-cloak`/`v-show` allowed.
5. **Profile checks on user-authored code** (never on generated wrapper code): run `checkProfile`/`analyzeScopes` from WP-C1a on the `<script setup>` /
   `<script>` content and on every template expression (re-parse each expression with Babel `parseExpression`, TypeScript plugin when the setup script is TS, offsets mapped back
   to the `.vue` file), mapping every diagnostic span to `.vue` line/column; check imports from `'vue'` against the manifest with `origin:'user'` (e.g. `h`, `Options API` helpers are
   rejected; `ref`, `computed`, `watch`, lifecycle hooks accepted) and unknown globals via the manifest's global list (`lookup('global', name)` if present else the C1a rule).
   Generated code is only checked for `import … from 'vue'` names using `origin:'compiler'` (must all be allowed).
6. **Styles**: `compileStyle` per block (`scoped` → attribute selector transform with the component scope id, `modules` → class map, `trim:true`, `isProd:true`,
   v-bind() variables collected) → `styles: [{ code, scoped, module: 'name'|null, scopeId, vars, map, filename, index }]`. For now the CSS is not interpreted (a later WP validates it against
   the allowlist); record the CSS exactly as produced.
7. **Output module**: stable and deterministic; the scope id and `__name` set as Vue's plugin does for `<script setup>`; `export default` is the component; no `import` of the style blocks
   (styles travel separately in `styles`). Include a header comment listing generator version and Vue reference version. A `serializeDiagnostics(diags, source)` helper prints them with
   `formatDiagnostic`.

## Tests (`tests/compiler/sfc/*.test.mjs`, fixtures in `tests/sfc/fixtures/*.vue`)
- ≥ 40 diagnostics fixtures with exact expected code and `.vue` line/col span (UTF-16, includes astral characters before the error), each unsupported block/option/tag/directive/modifier/
  import/Options API form, plus accepting counter-fixtures.
- Determinism: compile each fixture twice and under a different working directory → byte-identical output; no absolute paths in output or maps.
- **Real-Vue execution oracle**: for ≥ 25 accepting fixtures (counter, props+emits, `defineModel` parent/child, slots + scoped slots, `v-for` with key, `v-if/else`, `v-model` on
  text/checkbox/select, `withDefaults`, TS generic props, destructured props, `defineExpose` + template ref, `provide/inject`, `watch`+lifecycle hooks, scoped style attribute present,
  template-only component, normal+setup script, `v-bind()` in CSS) mount the compiled component with the browser build `node_modules/vue/dist/vue.global.prod.js` in headless Chrome
  through `tooling/chrome` (serve a directory with an import-map-free page: load the global build, turn the emitted ES module into a runnable function by replacing the single `import … from "vue"`
  statement with destructuring from `Vue` — the test helper does that rewrite on the *generated* code) and assert `innerHTML` after mount **and after scripted interactions**
  (click, input events) against hand-written expected HTML. Close all Chrome processes.
- Negative controls: break a fixture on purpose → test fails; confirm the span test fails when a span is off by one.

## Acceptance
`node --test "tests/compiler/sfc/*.test.mjs"` passes (including the Chrome-backed ones; if Chrome is missing they must SKIP loudly); report the diagnostic code table you added.

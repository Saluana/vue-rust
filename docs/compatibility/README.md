# Compatibility manifest

The compatibility manifest is the closed-world, default-deny inventory of the vue-native language, Vue and TypeScript profile (WP-G0B). Its source of truth is section 8 of `planning/vue-native-framework/design.md` (126 rows: L01-L21, S01-S25, V01-V29, U01-U32, A01-A19). The compiler reads `compiler/profile/manifest.json` through `compiler/profile/index.mjs`.

## Status letters

| Letter | Meaning for the compiler |
|---|---|
| P | Faithful: planned behaviour within the profile. |
| A | Adaptation: explicit native or host adaptation. |
| W | Web-only: supported on the web target only. |
| X | Excluded: must fail with a diagnostic that has a code, a message and a source span. |
| E | Experimental: not promised for production. |

Anything not in the manifest is unsupported. There is no fallback to an engine or to Node.

## Lookup API

`compiler/profile/index.mjs` exports:

- `loadManifest()`: the frozen manifest object.
- `lookup(kind, name)`: `{ status, row, note, compilerInternal }` or `null`. Lookups use `Map`s, so names such as `constructor` or `__proto__` never resolve.
- `isSupported(kind, name, target, { origin } = {})`: `target` is `'native'` or `'web'` (anything else throws a `TypeError`). `origin` is `'user'` (default) or `'compiler'` (anything else throws).
  - P and A names are supported on both targets.
  - W names are supported only on `'web'`.
  - X, E and unknown names are not supported.
  - `compilerInternal` names are supported only for `origin: 'compiler'` (generated code), on both targets. This never removes support from a public entry.

Kinds: `vue` (runtime exports and compile-time macros of the pinned package), `macro`, `syntax`, `builtin`, `module`, `ts`, `type`, `sfc`, `option`, `directive`, `modifier`, `attribute`, `component`, `element`, `native-tag`, `app`, `instance`, `flag`, `api`, `config`.

## Files

| File | Written by | Content |
|---|---|---|
| `planning/vue-native-framework/design.md` | (read-only input) | Section 8 tables. |
| `compiler/profile/inventory.generated.json` | `extract-inventory.mjs` | 126 rows: `id`, `section`, `title`, `statusText` (verbatim), `statuses` (derived set), `primary`, `prototype`, `contract`, `tests`, `rawRow`. |
| `compiler/profile/export-map.json` | hand-written | Each vue runtime export mapped to one row (optional explicit `status`, `note`, `needsDecision`), the two CJS interop keys, triage of documented names that are not exports, and `compilerHelperExclusions`. |
| `compiler/profile/expansions.json` | hand-written | Named entries for the rows: APIs, constructs, directives, macros, options and native tags. |
| `compiler/profile/vue-exports.json`, `docs/compatibility/vue-exports-audit.md` | `audit-vue-exports.mjs` | Classification of every vue export, compiler-helper analysis, and the audit report. |
| `compiler/profile/manifest.json` | `build-manifest.mjs` | `{ schemaVersion, vueReference, default, rows, entries, vueExports }`. |
| `docs/compatibility/inventory.md` | `build-manifest.mjs` | Human-readable table of all rows. |

The standard-library and globals allowlist (WP-G0E) is generated separately: see [stdlib.md](stdlib.md).

## Regenerate and check

- `node tooling/manifest/build-manifest.mjs` regenerates all five generated files. Output is byte-identical across runs.
- `node tooling/manifest/build-manifest.mjs --check` exits 1 and lists the files if any committed generated file is stale.
- `node --test tests/profile/*.test.mjs` runs the tests. Use the glob: `node --test tests/profile/` does not run on Node 26.5.1.

Do not edit generated files by hand. Change `export-map.json`, `expansions.json` or `design.md` and regenerate.

## Derivation rules

- **Status text**: the design text before the first `;` in the status cell. For section 8.5 this is the leading status phrase.
- **Status set**: standalone capital `P`, `A`, `W`, `X` or `E` in the status text, in order of appearance. The first letter is the primary status.
- **Prototype**: text after the `;` in sections 8.1-8.4. `—` and absence both mean none.
- **Vue runtime exports**: `Object.keys(await import('vue'))` minus `__esModule` and `module.exports`. The audit confirms the set against `require('vue')` and a `NODE_ENV=production` child process.
- **Compile-time macros**: functions in vue's type declarations whose JSDoc says "compiler macro".
- **Export status**: the row primary status, unless `export-map.json` gives an explicit status. An explicit status must be a status of its row, or `X`/`E` (stricter; always allowed).
- **Compiler helpers**: names in the official compiler's helper tables (`helperNameMap` in `@vue/compiler-core`, `registerRuntimeHelpers` in `@vue/compiler-dom`) and `@vue/compiler-sfc` call sites (`ctx.helper(...)`, `CSS_VARS_HELPER`). A helper is `compilerInternal` when it is a vue export, has status `X`, has at least one emission site (a reference other than its definition, map entry or export) and has no entry in `compilerHelperExclusions`.
- **Unmapped exports**: fall back to `X` with note `unclassified: default-deny`, and are listed under "Needs coordinator decision".
- **Expansions**: each entry's status must be a status of its row, or `X`/`E`. `vue` and `macro` entries must agree with `vue-exports.json`. `app.<method>` entries must exist on a real app instance. `native-tag` names are lowercase without hyphens.

## Decisions

1. **Native tags (U31; coordinator decision, authoritative).** Supported (status A, kind `native-tag`): main, section, div, header, footer, nav, article, aside, form, ul, ol, li, hr, h1-h6, p, span, label, strong, em, b, i, small, code, pre, br, a, button, input, textarea, select, option, img (37 tags). Excluded (status X, each with a "use ... instead" note): table, caption, colgroup, col, thead, tbody, tfoot, tr, th, td (use the framework grid component; design 9.2 Layout), svg (use an image asset; design 9.2 SVG), video and audio (web-only extension, use the web target), canvas (use an image asset), iframe (use a link; U32 excludes webview insertion). Any other tag returns `null`, which the compiler must report as an unknown-tag error with a span (U31: unknown tags error). Hyphenated custom elements are not entries; they are excluded by U19 and U32 and also return `null`.
2. **Template tags stay under their own rows.** `template`: element U30 (P), and the SFC `<template>` block is S01. `slot`: element U12 (P). `component`: component U19 (P). `transition` (U23, A), `transition-group` (U24, A), `keep-alive` (U21, P), `teleport` (U22, A), `suspense` (U25, E).
3. **CSS contract.** `useCssModule` and `useCssVars` are P (U03 primary). design.md section 9.2, "SFC scoping", lists both in its proposed contract. `useCssVars` is emitted by `@vue/compiler-sfc` for `v-bind()` in `<style>`. `@vue/compiler-sfc` 3.5.43 has no reference to `useCssModule`, so it is not `compilerInternal`.
4. **Compiler-internal helpers.** Under the rule in "Derivation rules", 39 of the 52 A11 members are `compilerInternal` (X for user code, importable by generated code): Fragment, openBlock, createBlock, createElementBlock, createVNode, createElementVNode, createCommentVNode, createTextVNode, createStaticVNode, resolveComponent, resolveDynamicComponent, resolveDirective, withDirectives, renderList, renderSlot, createSlots, toDisplayString, normalizeClass, normalizeStyle, normalizeProps, guardReactiveProps, toHandlers, camelize, toHandlerKey, setBlockTracking, withCtx, withMemo, isMemoSame, the vModel helpers (text, checkbox, radio, select, dynamic), vShow, withKeys, withModifiers, createPropsRestProxy, mergeDefaults, mergeModels. `mergeProps` is also emitted by the compiler, but it is public (U13, P, decision 11), so it is not flagged. The other 13 A11 members are not flagged because the official compiler does not emit them. `h`, `cloneVNode` and `isVNode` are adapted (A, decision 11). The other ten stay X for both origins: Comment, Static, Text, assertNumber, transformVNodeArgs, withScopeId, capitalize, pushScopeId, popScopeId, resolveFilter. The 18 helper-table names that are vue exports but not flagged are excluded with a reason in `export-map.json` (`compilerHelperExclusions`). They are user-tag or syntax triggered (BaseTransition, Suspense, KeepAlive, Teleport, Transition, TransitionGroup, resolveFilter, withAsyncContext), public (unref, isRef, defineComponent, useModel, useSlots, useCssVars, mergeProps), or have no emission site (capitalize, pushScopeId, popScopeId).
5. **Origin semantics.** `isSupported(..., { origin: 'compiler' })` is true for `compilerInternal` names on both targets. The default origin is `'user'`.
6. **Filter syntax.** Template `|` filter syntax is unsupported (default-deny; design.md defines no filter API). The official compiler emits `resolveFilter` only for that syntax.
7. **Override re-check against design.md.** Status changed: `useCssModule` and `useCssVars` (X to P, decision 3). Justification rewritten, status unchanged: `DeprecationTypes` and `compatUtils` (design.md makes no promise for the compat build); `devtools`, `initCustomFormatter` and `setDevtoolsHook` (A17 makes devtools a build-time profile control, so these exports are plumbing); `registerRuntimeCompiler` and `compile` (A08: no runtime template compiler); `BaseTransition` (emitted only for a user-written `<BaseTransition>` tag, which is rejected). The other overrides keep their justifications: internal helpers, or behaviour the design does not promise. Decisions 11 to 13 replace the overrides for `h`, `cloneVNode`, `isVNode` and `getCurrentInstance` (now A) and remove the overrides for `effect`, `stop`, `proxyRefs` and `getCurrentWatcher` (now P).
8. **CJS interop keys.** `__esModule` and `module.exports` are not Vue exports and are excluded (verified against `require('vue')`).
9. **Documented compile-time flags.** `__VUE_OPTIONS_API__`, `__VUE_PROD_DEVTOOLS__` and `__VUE_PROD_HYDRATION_MISMATCH_DETAILS__` are documented but absent from the pinned dist, because bundlers substitute them. The profile must set them (A17).
10. **Still open (coordinator).** `hydrateOnIdle`, `hydrateOnInteraction`, `hydrateOnMediaQuery` and `hydrateOnVisible` stay X. A14 and A16 are W, so the web profile may promise them. These entries are marked `needsDecision`.
11. **Render helpers (A11 and U13; adapted).** `h`, `cloneVNode` and `isVNode` are A: the runtime-core functions over native VNodes. A VNode exposes the readable fields listed in the `h` note of `export-map.json`. `el`, `anchor`, `suspense`, the `target*` fields, `staticCount`, `ssContent`, `ssFallback` and `__v_isVNode` are not readable. `isVNode` and the one-VNode-child form of `h` follow runtime-core's rule: a native VNode, or an object whose `__v_isVNode` is `true`. `cloneVNode` throws a TypeError for a non-VNode argument, where Vue reads the fields of any object. `mergeProps` is public (U13, P) with the runtime-core merge rules. `resolveComponent`, `resolveDirective`, `withDirectives` and `withModifiers` stay X. **Open (coordinator):** a VNode built by `h` cannot reach the DOM under this profile. Rendering it needs a render function returned from `setup` or the Options `render` option, and A10 and A18 keep both X. The UI cases therefore check VNode fields and `mergeProps` through `v-bind`, not rendered `h` output. Tests: `tests/ui/cases/render/{h-vnode-tree,h-variadic-children,is-vnode-clone-vnode,merge-props}`, `tests/profile/compiler-helpers.test.mjs`, `tests/cli/check.test.mjs`.
12. **getCurrentInstance (A18; adapted).** `getCurrentInstance()` is A. It returns one opaque handle per component instance, or `null` outside setup and render. The handle has a null prototype and no readable properties, so every property read is `undefined`. Supported: identity and null checks. Instance fields and the `$` properties stay X (A18). Values come from `defineProps`, `defineEmits`, `useAttrs`, `useSlots` and `inject`. Tests: `tests/ui/cases/render/get-current-instance`, `tests/profile/index.test.mjs`.
13. **Reactivity names (V07, V12, V16; P).** `effect`, `stop`, `proxyRefs` and `getCurrentWatcher` are P with the production semantics of @vue/reactivity 3.5.43. `effect` ignores `lazy`: fn runs before `effect` returns, whatever the option says. The Rust `effect` honoured `lazy` before this change, which was a bug; `crates/reactivity/src/effect.rs` now ignores it. The runtime's component render effect used `lazy` to defer its first run, so it now calls the internal `effectWithoutRun`, which creates the same effect and runs nothing. `getCurrentWatcher` is set only inside watch callbacks and watchEffect getters, not in the source function of `watch(source, cb)`. Vue copies every option key onto the effect with `Object.assign`. The runtime reads only `scheduler` and `onStop`, the two keys the production effect class uses, so other option keys are neither copied nor read. `ReactiveEffect` stays X: its constructor and prototype are not exposed. Tests: `tests/vue/cases/effect/*`, `tests/vue/cases/ref/proxyrefs-*`, `tests/vue/cases/watch/getcurrentwatcher-*`, `crates/reactivity/tests/api_gc.rs`.

## Known gaps

- The standard-library method allowlist (L09, L10, L18, L21) is not enumerated. Only the named constructors and objects have entries. Other methods are unsupported until listed.
- Type-only exports (S22) are not classified individually. Only runtime exports and compile-time macros are audited.
- `vue/server-renderer` exports are not audited. `renderToString` and related names are recorded as `api` entries.
- The documented-name scan covers camelCase, PascalCase, `$`-names and `app.`/`config.` paths. Lowercase-only names are not scanned.
- CSS beyond the entries listed, ecosystem packages (8.6) and sections 9-18 are outside these rows.

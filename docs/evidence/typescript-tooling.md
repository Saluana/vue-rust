# TypeScript 6 tooling pair and type checking — 2026-10-10

The checker pair is TypeScript 6.0.3 with vue-tsc 3.3.12, which brings @vue/language-core 3.3.12 and @volar/typescript 2.4.28. TypeScript 7.0.2 is excluded: it is a native build with no JavaScript compiler API, and the Vue language tools need that API (design.md line 573; K07 at line 734). The manifest records typescript@6.0.3 and vue-tsc@3.3.12 as supported adaptations of row S23, and typescript@7 as X.

compiler/types/pair.mjs reads the installed versions from node_modules at run time and refuses to type check with any other pair (TypeCheckError). compiler/types/check.mjs runs vue-tsc 3.3.12 programmatically over the modules of a project. Each .vue file is checked as the virtual file that @vue/language-core generates, and every TypeScript error or warning becomes a VN7001 diagnostic at the .vue (or .ts) span it came from. The message keeps the TypeScript code (TSnnnn).

`vue-native check` runs the checker when the project compiled without errors; `--no-types` skips it. JavaScript modules and JavaScript components are not checked (checkJs is off), so a JavaScript project reports no type errors. tooling/types/probe.mjs runs the same code path on tests/compiler/types/fixtures/pair and prints the versions it read and each diagnostic with its .vue line and column.

Imported prop types resolve. compiler/sfc gives the official compileScript (@vue/compiler-sfc 3.5.43) a file-system adapter that reads only files inside the project root. A relative type-only import that names no file is VN6017 at the import source; before this change the missing type silently became an untyped prop. Imported types also resolved only when the working directory was the project root, because project-relative names were resolved against the working directory. Names are now anchored at the project root.

Macro types that the compiler cannot read are VN6017. compileScript reads the types of defineProps, defineModel and defineEmits itself. When a read inside a prop type fails, the failure is swallowed and the prop is emitted untyped (`props: { flag: {} }`); only a top-level failure stops the build. compiler/sfc/macro-types.mjs therefore checks the imports that these type arguments reach, before compileScript runs, in build and check alike, and reports:

- a name from a bare package specifier (`'some-lib'`; `'vue'` is a package like any other) or from an absolute path, at the module specifier. compileScript reads only relative project files, and TypeScript is not registered;
- a name from a relative file that the file does not export as a type, at the imported name. The official compiler decides this: a one-macro probe component with the same import is compiled with the same root-limited file access, and a failure at the probe's own macro (not inside the imported file) means the file does not provide the name. No copy of TypeScript's module resolution is involved;
- a namespace import (`import type * as T`), which compileScript never reads, at the namespace name;
- a `typeof` of an imported value, at the operand, and an inline `import("...")` type that names a package (at the string) or a name the file does not export (at the name).

The check follows the declarations of the same file that a macro type names (type aliases, and the extends clauses of interfaces), so a missing name reached through them is reported at the import. Type-only imports that no macro reads stay allowed, as do re-exports (also under another name), `export *` and default imports that the file provides. A type parameter hides an import of the same name. A member with a leading `@vue-ignore` comment is not read, as in Vue. The payload types of defineEmits events are never read by the compiler and are not checked. `build` still does not run vue-tsc.

## Verification

All commands run from the worktree with CARGO_TARGET_DIR=/home/claude/targets/w-types.

| Command | Result |
| --- | --- |
| `node --test "tests/**/*.test.mjs"` (the npm test glob) | 1024 tests: 889 pass, 0 fail, 135 skipped (exit 0). The skips are environment: Google Chrome not found (the browser oracle and runtime tests), sandbox-exec unavailable on Linux (loud skips: sandbox isolation is not verified here), the quick-quote source archive absent, and no vue-core checkout (VUE_CORE_DIR) |
| `node --test "tests/compiler/**/*.test.mjs" "tests/cli/*.test.mjs"` | 592 tests: 557 pass, 0 fail, 35 skipped (exit 0) |
| `node --test "tests/profile/*.test.mjs"` | 111 tests: 111 pass, 0 fail (exit 0) |
| `node tooling/manifest/build-manifest.mjs --check` | build-manifest: 5 generated files are up to date (exit 0) |
| `node tooling/harness/check-sfc-lowering.mjs` | fixtures 45: lowered 45, not lowered 0; TypeScript fixtures lowered 9; cargo check ok (45 fixture projects; 0 with errors) (exit 0) |
| `node tooling/cli/vue-native.mjs check examples/counter` | ok: 1 module(s), 0 error(s), 0 warning(s) (exit 0) |
| `node tooling/cli/vue-native.mjs check examples/quick-quote` | ok: 4 module(s), 0 error(s), 0 warning(s) (exit 0) |
| `node tooling/cli/vue-native.mjs check examples/quick-quote --target native` | ok: 4 module(s), 0 error(s), 0 warning(s) (exit 0) |

The 35 skipped tests are the real-Vue execution oracle in tests/compiler/sfc/oracle.test.mjs, skipped with the reason "Google Chrome not found; install it or set CHROME_PATH". They include the cases for ok-imported-props.vue and ok-imported-alias.vue, and they are not executed locally.

Scratch checks (scripts in the agent scratchpad, not in the repo):

| Check | Result |
| --- | --- |
| Lowering negative control: `node tooling/harness/check-sfc-lowering.mjs --fixtures <scratch dir with ok-bad-enum.vue and ok-imported-props.vue>` | fixtures 2: lowered 1, not lowered 1; not lowered: `ok-bad-enum.vue: VN3001 TypeScript enums are not supported`; cargo check ok (1 fixture project; 0 with errors) (exit 1) |
| Each of the 124 .vue files in tests/sfc/fixtures compiled twice in one process | no dependency differences |
| The 183 SFC directories of the repo, compiled with the HEAD compiler (git archive copy) and with the working tree over the same sources; diagnostics, spans and code length compared | identical, except the six macro-type fixtures in tests/sfc/fixtures. HEAD compiled four of them to untyped props, reported VN9001 for bad-import-type-vue.vue and VN6012 for bad-import-type-extends.vue; the working tree reports VN6017 for all six |

## Negative controls

- Pair: TypeScript 7.0.2, vue-tsc 3.3.11, @vue/language-core 3.3.13, a range (^6.0.3) in package.json, and a checkout without node_modules are each refused (tests/profile/typescript-pair.test.mjs).
- Type checking disabled: a project with a type error fails the check, and the same project passes with --no-types (tests/cli/check-types.test.mjs).
- JavaScript: the same code written in TypeScript is reported (TS2322 at src/App.vue 4:0, TS2551 at src/util.ts 2:23), and the JavaScript version is not (tests/compiler/types/check.test.mjs).
- A mutated Counter, with `count.value = 0` changed to a string, fails at 12:2.
- A checkout whose TypeScript is 7.0.2 raises TypeCheckError before any module is checked.
- VN6017 for a missing type file: reported at 2:29-2:49, and the same component passes once the file exists. A type file outside the project root is refused (2:29-2:44), although the official compiler reads it without error (tests/compiler/sfc/imported-types.test.mjs).
- Runtime: a compiled props definition with `label` removed, or with a changed default, renders differently from the hand-derived markup (tests/compiler/sfc/imported-types.test.mjs).
- Lowering: a fixture the profile rejects (`enum`) is reported as not lowered, and the harness exits 1.
- Macro types (tests/compiler/sfc/diagnostics.test.mjs, tests/compiler/sfc/imported-types.test.mjs, tests/cli/check-types.test.mjs, tests/cli/build.test.mjs). Spans are line:column of the .vue file, end exclusive.
  - A package read by a macro: bad-import-type-package.vue 2:27-2:37. A project with it fails `check` and `build` with VN6017 at src/App.vue 2:27. `check` does not run the checker, and `build` creates no output directory.
  - A name the file does not export: bad-import-type-unexported.vue 2:14-2:21. Mutation control: the same component compiles when its file exports the name (`props: { flag: { type: Boolean } }`), and is VN6017 when the file does not. A `check` project with such a file reports 2:14-2:21.
  - A local alias that reaches the import: bad-import-type-local.vue 2:14-2:21. An interface that extends the import: bad-import-type-extends.vue 2:14-2:18 (HEAD: VN6012 "Failed to resolve extends base type" at 1:0-1:24).
  - `vue`: bad-import-type-vue.vue 2:25-2:30 (HEAD reported VN9001 in generated code instead).
  - A namespace import: bad-import-type-namespace.vue 2:17-2:22.
  - A typeof of an import: Typeof.vue 3:25-3:29.
  - Inline import types: Inline.vue 2:27-2:37 (a package) and 2:80-2:87 (a missing name).
  - defineEmits and defineModel (scratch checks, not in the repo): `defineEmits<Events>()` with a package Events is VN6017 at the specifier; `defineEmits<{ change: [] } & Missing>()` and `defineModel<Missing>()` with an unexported name are VN6017 at the name; a package type as an emits payload (`defineEmits<{ change: [when: Events] }>()`) is allowed.
  - Forms that stay allowed, with no diagnostic: ok-import-type-reexport.vue (its props equal the official compileScript output), ok-import-type-unused-package.vue, ok-import-type-emits-payload.vue and ok-import-type-shadowed.vue.
  - Determinism: bad-import-type-local.vue compiled twice gives the same dependencies (tests/compiler/sfc/determinism.test.mjs). On a warm cache compileScript does not read a type file again, so its read hook does not record it; macro-types.mjs therefore records the direct type file itself.

## Remaining limits

- The Chrome oracle cases are not run here (see above). The same markup is checked here by server rendering with vue@3.5.43 (tests/compiler/sfc/imported-types.test.mjs).
- `build` does not type-check. Only `check` runs the checker, so a type error in component code (for example, a string assigned to a number ref) is VN7001 under `check` and passes `build`.
- A name that no file declares is compiled as an untyped prop by `build`, with no diagnostic, when a macro type reaches it through a declaration, local or imported. `interface Props { flag: Missing }` gives `props: { flag: {} }`, and so does the same interface in an imported file. The macro check does not look at names inside declarations. `check` reports it (for an imported file: `src/types.ts:2:8: error VN7001: TS2304: Cannot find name 'Missing'.`). Not closed here: `build` does not run the checker, so this stays a check-only diagnostic.
- Conditional types and utility types are read by compileScript's own rules, which are not TypeScript's. `type Cond<T> = T extends string ? boolean : number` used as `defineProps<{ x: Cond<string> }>()` compiles to `props: { x: {} }`, and `Exclude<boolean | string, string>` compiles to `{ type: [Boolean, String] }`, locally and when imported alike. Neither comes from an import, and neither is closed here. Production props keep only the Boolean and Function types, so such a difference reaches the runtime only through Boolean casting and Vue's rule for Function defaults. A top-level read that compileScript cannot evaluate is loud (`VN6012 ... Unresolvable type: TSConditionalType`), as at HEAD.
- Classes and enums that a type-only import brings in are accepted and compiled as `props: { box: {} }`. A value import of the same class is VN2002 at the class. A type-only import is erased, so its file is not profile-checked (`check` passes the type-only class case).
- Emits payloads: the payload types of defineEmits events are never read by compileScript and are not checked. They do not reach the output.
- `@vue-ignore` members are skipped by Vue and by the macro check alike, so a missing name in one is not reported. This is intended.
- The process cache (pre-existing at HEAD): compiler-sfc keeps one scope for each project-relative type file name for the life of the process. Two projects compiled in one process with a type file of the same name share the first project's types. Project B's `flag: string` compiled as `flag: { type: Boolean }` after project A's `boolean`, while B alone compiles to `flag: {}`; this was verified at HEAD and in the working tree. The CLI compiles one project per process, so it is not affected. The tests give each type file a name that no other test in the process uses.
- Dependencies (pre-existing in compiler-sfc): compileScript's own dependency list depends on its cache. A file reached only through `export *` can appear on the first compile and not on a later one (depsprobe: star.ts listed cold, not warm). The frontend's list was the same on a second compile in every case tried, but the frontend does not rely on compileScript's list, and nothing in the CLI reads `dependencies` yet.
- A value import (not `import type`) of a missing relative file that a macro reads is reported by the project graph as VN9001 at a position in the generated code, not in the .vue file. It is an error, so nothing is emitted, but the span does not point at the import.
- Pre-existing, not fixed here: `import type { Ref } from 'vue'` in plain code, outside any macro, is VN9001 "generated code imports 'Ref' from 'vue', which is outside the profile" from both `check` and `build`, because the generated-code import check does not skip type-only specifiers.
- TypeScript 7 cannot be run here and is excluded by the pair check and the manifest.
- VN7001 opens a new diagnostic range (VN7) and VN6017 is a new SFC code. Both are registry additions for the maintainer to confirm.

## Open decisions for the maintainer

- A `vue` type read by a macro is rejected like any other package. Accepting it would emit an untyped prop, which the contract forbids; mapping the vue types by hand is not done.
- A package type inside an array element or a function parameter of a defineProps type argument is also rejected (`items: Pkg[]`, `handler: (x: Pkg) => void`, both VN6017 at the specifier; HEAD compiled them). The production output does not depend on those positions, so this is stricter than needed. Narrowing it is a choice.
- The payload types of defineEmits are not checked, as above.

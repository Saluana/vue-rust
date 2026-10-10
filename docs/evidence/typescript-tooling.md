# TypeScript 6 tooling pair and type checking — 2026-10-10

The checker pair is TypeScript 6.0.3 with vue-tsc 3.3.12, which brings @vue/language-core 3.3.12 and @volar/typescript 2.4.28. TypeScript 7.0.2 is excluded: it is a native build with no JavaScript compiler API, and the Vue language tools need that API (design.md line 573; K07 at line 734). The manifest records typescript@6.0.3 and vue-tsc@3.3.12 as supported adaptations of row S23, and typescript@7 as X.

compiler/types/pair.mjs reads the installed versions from node_modules at run time and refuses to type check with any other pair (TypeCheckError). compiler/types/check.mjs runs vue-tsc 3.3.12 programmatically over the modules of a project. Each .vue file is checked as the virtual file that @vue/language-core generates, and every TypeScript error or warning becomes a VN7001 diagnostic at the .vue (or .ts) span it came from. The message keeps the TypeScript code (TSnnnn).

`vue-native check` runs the checker when the project compiled without errors; `--no-types` skips it. JavaScript modules and JavaScript components are not checked (checkJs is off), so a JavaScript project reports no type errors. tooling/types/probe.mjs runs the same code path on tests/compiler/types/fixtures/pair and prints the versions it read and each diagnostic with its .vue line and column.

Imported prop types resolve. compiler/sfc gives the official compileScript (@vue/compiler-sfc 3.5.43) a file-system adapter that reads only files inside the project root. A relative type-only import that names no file is VN6017 at the import source; before this change the missing type silently became an untyped prop. Imported types also resolved only when the working directory was the project root, because project-relative names were resolved against the working directory. Names are now anchored at the project root.

## Verification

All commands run from the worktree with CARGO_TARGET_DIR=/home/claude/targets/w-types.

| Command | Result |
| --- | --- |
| `node --test "tests/compiler/**/*.test.mjs" "tests/profile/*.test.mjs" "tests/cli/*.test.mjs"` | 678 tests: 643 pass, 0 fail, 35 skipped (exit 0). Run alone: compiler 516 tests (481 pass, 0 fail, 35 skipped); profile 111 (111 pass) |
| `node tooling/manifest/build-manifest.mjs --check` | build-manifest: 5 generated files are up to date (exit 0) |
| `node tooling/harness/check-sfc-lowering.mjs` | fixtures 41: lowered 41, not lowered 0; TypeScript fixtures lowered 5; cargo check ok (41 fixture projects; 0 with errors) (exit 0) |
| `node tooling/harness/check-sfc-lowering.mjs --fixtures <scratch dir holding one rejected fixture>` (negative control) | fixtures 2: lowered 1, not lowered 1; `ok-bad-enum.vue: VN3001 TypeScript enums are not supported`; cargo check ok (exit 1) |
| `node tooling/cli/vue-native.mjs check examples/counter` | ok: 1 module(s), 0 error(s), 0 warning(s) (exit 0) |
| `node tooling/cli/vue-native.mjs check examples/quick-quote` | ok: 4 module(s), 0 error(s), 0 warning(s) (exit 0) |
| `node tooling/cli/vue-native.mjs check examples/quick-quote --target native` | ok: 4 module(s), 0 error(s), 0 warning(s) (exit 0) |

The Chrome oracle in tests/compiler/sfc/oracle.test.mjs is skipped here with the reason "Google Chrome not found; install it or set CHROME_PATH". Its tests, including the two cases added for ok-imported-props.vue and ok-imported-alias.vue, are not executed locally.

## Negative controls

- Pair: TypeScript 7.0.2, vue-tsc 3.3.11, @vue/language-core 3.3.13, a range (^6.0.3) in package.json, and a checkout without node_modules are each refused (tests/profile/typescript-pair.test.mjs).
- Type checking disabled: a project with a type error fails the check, and the same project passes with --no-types (tests/cli/check-types.test.mjs).
- JavaScript: the same code written in TypeScript is reported (TS2322 at src/App.vue 4:0, TS2551 at src/util.ts 2:23), and the JavaScript version is not (tests/compiler/types/check.test.mjs).
- A mutated Counter, with `count.value = 0` changed to a string, fails at 12:2.
- A checkout whose TypeScript is 7.0.2 raises TypeCheckError before any module is checked.
- VN6017: a missing type file is reported at 2:29-2:49, and the same component passes once the file exists. A type file outside the project root is refused (2:29-2:44), although the official compiler reads it without error (tests/compiler/sfc/imported-types.test.mjs).
- Runtime: a compiled props definition with `label` removed, or with a changed default, renders differently from the hand-derived markup (tests/compiler/sfc/imported-types.test.mjs).
- Lowering: a fixture the profile rejects (`enum`) is reported as not lowered, and the harness exits 1.

## Remaining limits

- Browser runtime is not executed here. The two Chrome oracle cases are skipped locally; the same markup is checked here by server rendering with vue@3.5.43.
- `build` does not type-check. Only `check` runs the checker, so in `build` a type name missing from an existing file still becomes an untyped prop (`check` reports it as TS2305, VN7001).
- A type-only import from a bare package name is not VN6017, because only relative specifiers are checked. `check` reports it as TS2307 (VN7001); `build` does not.
- TypeScript 7 cannot be run here and is excluded by the pair check and the manifest.
- VN7001 opens a new diagnostic range (VN7) and VN6017 is a new SFC code. Both are registry additions for the maintainer to confirm.

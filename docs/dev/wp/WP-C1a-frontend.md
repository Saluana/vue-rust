# WP-C1a — Compiler front half: diagnostics, parsing, profile checker, scope & closure analysis (pure JavaScript)

Read `docs/dev/AGENT-CONTRACT.md` and `docs/dev/ARCHITECTURE.md` first. WP id `c1a` (no cargo).
**Owned paths:** `compiler/diagnostics/**`, `compiler/frontend/**`, `compiler/analysis/**`,
`tests/compiler/frontend/**`, `tests/language/rejects/**`. Do NOT touch `compiler/compile.mjs`, `compiler/cli.mjs`,
`compiler/profile/**` (another WP generates the manifest there; read it if it exists, never write it).
No new npm dependencies: only the installed `@babel/parser` (7.29.9). ES modules (`.mjs`), Node 26.

## Why
Our compiler turns the JS/TS that the Vue SFC compiler emits (and user modules) into Rust. Before lowering, every
module must be (1) parsed with exact source positions, (2) checked against the closed-world language profile so
unsupported constructs fail with a precise, explained diagnostic, and (3) analysed for scopes, hoisting, TDZ,
captured and mutated variables — the information closure conversion needs (captured variables become heap cells).
The lowering/emitter is a later WP; your outputs are its input contract.

## Deliverables

### 1. `compiler/diagnostics/`
- `diagnostic.mjs`: `class Diagnostic { code, severity: 'error'|'warning', message, hint, file, start: Pos, end: Pos }`
  where `Pos = { offset /*UTF-16 code unit offset*/, line /*1-based*/, column /*0-based UTF-16 units*/ }`;
  `createLineMap(source)` → `{ posAt(offset), offsetAt(line,col) }` (handle `\n`, `\r\n`, `\r`, U+2028/2029 exactly
  like JavaScript line terminators; surrogate pairs; BOM).
- `codes.mjs`: a frozen registry `VN1000…` → `{ title, explanation, hint }`. Allocate ranges: `VN1xxx` syntax/parse,
  `VN2xxx` unsupported JavaScript construct, `VN3xxx` unsupported TypeScript construct, `VN4xxx` unsupported/unknown API or
  global, `VN5xxx` scope errors (duplicate declaration, assignment to const, etc.), `VN9xxx` internal compiler bug.
- `format.mjs`: `formatDiagnostic(diag, source, {color:false})` → multi-line text with `file:line:col`, the source line,
  a caret/underline range (tab-safe, wide-char-safe enough), the message and hint.

### 2. `compiler/frontend/parse.mjs`
`parseModule(source, { filename, lang: 'js' | 'ts' })` → `{ ast, diagnostics }` using `@babel/parser` with
`sourceType:'module'`, `errorRecovery:false`, `ranges:true`, plugins `['typescript']` when `lang==='ts'` (note: in TS
mode angle-bracket assertions are not allowed, so no ambiguity with JSX). A syntax error becomes a `VN1xxx` diagnostic with
the exact span (Babel `loc`/`pos`). Never throw for bad user input; throw only for programmer errors.

### 3. `compiler/analysis/profile-check.mjs`
`checkProfile(ast, { source, filename, lang, target: 'native'|'web' })` → `Diagnostic[]`. Walk the whole AST and report
every construct that the profile excludes, **each with a distinct code, an explanation and a "use this instead" hint**.
Authoritative list = `planning/vue-native-framework/design.md` §8.1 (L-rows), §8.2 (S-rows), the "Scope boundaries"
in requirements.md §1 and the exclusions in §18. At minimum reject: class declarations/expressions & `extends`/`super`/
private names/static blocks (L05), `new` of anything that is not a known built-in constructor identifier or an imported
binding (L05), `function*`/`yield`/async generators/`for await` (L15), `with` (L01), sloppy-only syntax is impossible in
modules (no check needed), `eval` / `Function(...)` / `new Function` / `import()` with a non-literal specifier (L17, L20),
`Proxy`/`Reflect` identifiers used as globals (L20), regular-expression literals and `RegExp` (L18), BigInt literals and
`BigInt` (L08), tagged templates (not in profile), `debugger`, `arguments` (not in profile), `__proto__` in literals and
`super`, `Object.setPrototypeOf`/`Object.defineProperty` style metaprogramming (flag as VN4xxx), top-level `await` (L16),
getters/setters via `defineProperty` (flag), label+function declarations, `import.meta`, `new.target`, sequence of
`delete` on identifiers, octal literals, `var` in blocks is allowed. TypeScript (when `lang:'ts'`): `enum`/`const enum`,
`namespace`/`module` declarations, `import x = require()`/`export =`, decorators, parameter properties, abstract/`declare`
members that emit runtime code, JSX/TSX, `using`/`await using`, `accessor` fields (S20, S21, S24, S25) — but accept and
ignore pure type syntax (annotations, `interface`, `type`, generics, `as`, `satisfies`, non-null `!`, type-only imports/
exports, `declare` of types). Identifiers used as globals are **not** judged here (see scope analysis). Rules are data:
keep a table `{ id, code, appliesTo(node,ctx), message, hint, row: 'L05' }`, so the manifest/documentation can list them.

### 4. `compiler/analysis/scope.mjs`
`analyzeScopes(ast, { source, filename })` → `{ module: ScopeInfo, bindings: Binding[], references: Reference[],
diagnostics }`, JSON-serializable (no cyclic objects: use numeric ids). Implement:
- scopes: module, function (params + body), block, catch clause, `for`/`for-in`/`for-of` head scopes (per-iteration
  bindings flagged), class excluded; `var` hoisting to function scope; function declaration hoisting (module/function
  top level and block-level functions in strict mode are block scoped); `let/const` TDZ region; parameter default
  expression scope subtleties may be approximated but must be documented.
- `Binding { id, name, kind: 'var'|'let'|'const'|'function'|'param'|'import'|'catch'|'implicit', scopeId, declNode range,
  captured: boolean /*referenced from a nested function*/, mutated: boolean /*assigned after initialization or updated*/,
  needsTdzCheck: boolean /*a read may happen before initialization: textually earlier, in a nested function created
  before init completes, in a loop, or switch case*/, perIterationCopy: boolean /*let/const declared in a for-loop head
  whose closure capture needs a fresh cell per iteration*/, exported: names[] }`.
- References resolved to a binding or to the global scope; `typeof x` of an undeclared name is allowed; unresolved global
  names are collected in `globalsUsed` (the caller compares with the manifest); assignment to `const` → `VN5xxx` (runtime
  TypeError at that point is also fine, but the static finding must be reported as a *warning-level* diagnostic with a
  clear message); duplicate lexical declarations → error; `import` bindings are immutable and live.
- `this` handling summary per function: which functions use `this`, which arrows capture lexical `this` from which
  function/module (module-level `this` is `undefined`).
- TypeScript type-only positions must be ignored (type annotations, `interface`, `type`, `declare`, generics).
- A `describeScopes()` pretty-printer for tests/debugging.

## Tests (`tests/compiler/frontend/*.test.mjs`, node:test, ≥ 80 focused tests)
- Positions: Unicode (astral chars, combining marks, `\r\n`, U+2028) → correct line/col/offset; every diagnostic span in the
  tests is asserted against hand-computed offsets.
- Profile checker: one rejecting fixture file per rule under `tests/language/rejects/<rule>.js|ts` with a header
  `// @expect VN2xxx line:col-line:col` verified by a test that loads all fixtures; plus accepting counter-examples proving
  the checker does not over-reject (e.g. `Map`/`Set`/`Promise`/`Symbol`/`Error` constructors accepted, type annotations
  accepted, `import { ref } from 'vue'` accepted).
- Scope analysis: closures capturing mutable variables, per-iteration `let` capture, TDZ detection (read-before-declare, in
  nested function, in loops), hoisting, shadowing, catch params, destructuring-introduced bindings, import live bindings,
  `typeof undeclared`, const assignment, duplicate declarations, arrow lexical `this`.
- **Cross-check against V8 where possible**: for a corpus of snippets with `let/const/var` + closures, generate (in the test,
  with `node:vm`) whether a TDZ `ReferenceError` actually occurs at runtime and assert that `needsTdzCheck` is true for every
  binding that really faults (soundness: no false negatives); false positives are allowed but counted and reported.
- Negative controls: mutating a fixture's expectation must fail the harness.

## Acceptance
`node --test "tests/compiler/frontend/*.test.mjs"` passes; each rejects fixture reports its expected code/span; no files outside owned
paths changed. In your report list the rule table (id → code → one line) so the coordinator can review coverage.

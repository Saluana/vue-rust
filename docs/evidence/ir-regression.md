# IR regression fixture and HIR schema hash (task 4.1, ticket w41-ir)

Two items were open in the task 4.1 ledger entry: an executable regression showing that the prototype's `@[click]`
defect is gone through the SFC pipeline, and content hashes of the HIR schema. Both are done. Changes are uncommitted
on branch `claude/w41-ir`.

## What was built

**Dynamic event argument.** `tests/compiler/sfc/dynamic-event-arg.test.mjs` compiles the prototype's input,
`<button @[click]="inc">` with `click` undeclared, through `compileSfc`. The pipeline rejects it with VN4016 (a free
global the native profile does not provide, from `compiler/analysis/stdlib-check.mjs`). No module is produced, and the
diagnostic span is the `click` token in the `.vue` file. The prototype emits Rust for the same input.

When the name is declared (`const click = ref('input')`), the pipeline emits `[_toHandlerKey(click.value)]: inc`, the
key that the official `compileScript` also emits. The component is built into a headless native binary with
`buildHeadlessApp`, the path `tests/cli/headless.test.mjs` uses, and run with the click and input operations. The
expected values are written from Vue's rules. While bound to `input`, a click does not run the handler and an input
event does. After the variable is set to `'click'`, input no longer reaches the handler and click does.

**Two defects found and fixed in `compiler/sfc/template.mjs`.** Both change validation only. A file that compiled before
still compiles to the same output; the changes are a corrected span and one new rejection.
- The VN4016 span for `@[click]` covered `[clic`. A dynamic argument's location includes its brackets, and the validator
  used that start as the base of the expression. `argExpr()` now starts one character later.
- Dynamic argument names were checked only for `@`, `:`, `v-slot` and custom directives. For example,
  `<Comp v-model:[nm]="x" />` with `nm` undeclared compiled to `[_ctx.nm]` silently. The check now runs once in
  `directive()` for every directive, before dispatch.

**HIR schema hash.** `compiler/hir/schema.mjs` holds the catalogue as data: `HIR_NODES` (69 kinds, 75 field sets) and
`HIR_RECORDS` (16 record names, 18 shapes, covering the module, functions, scopes, slots, imports, exports, hoist entries,
parameters, patterns, switch cases and handlers). `HIR_SCHEMA_HASH` is the sha256 of the canonical JSON (sorted keys,
no white space):

`7e6bf7b654d1153cb813afa35fe94e26053d7b3393bb009e6f272c496e8a0e68`

This digest was also computed independently: the canonical text was produced by Python
`json.dumps(sort_keys=True, separators=(',', ':'))` and hashed with `hashlib` and `sha256sum`. All three agree.

`checkHirShape()` walks a module record. It reports unknown node kinds, fields the catalogue does not list, and records
matching no shape, and it returns the catalogue entries that were met.
`tests/compiler/lower/hir-schema.test.mjs` walks the 166 lowering fixtures and two inline modules (imports, re-exports,
a namespace import, `delete` of a call, empty branches, a non-declaration for-of head). That is 168 modules with no
violations, and all 93 catalogue entries are produced by some module.

`compiler/hir/README.md` is corrected: it omitted `paramCopies` on function records and said every node has `pos`.

## Commands and results

Baseline, before any change:

| Command | Result |
|---|---|
| `node --test "tests/compiler/**/*.test.mjs"` | 485 tests: 452 pass, 0 fail, 33 skipped (no Chrome) |
| `node --test "tests/fixtures/prototype/*.test.mjs"` | 21 tests: 19 pass, 0 fail, 2 skipped (Quick Quote archive at a macOS path) |
| `node tooling/harness/run-language.mjs --dir tests/language/lowering` | 166 pass, 0 FAIL, 0 unexpected |

After the change, with `CARGO_TARGET_DIR=/home/claude/targets/w41-ir`:

| Command | Result |
|---|---|
| `node --test "tests/compiler/**/*.test.mjs"` | 500 tests: 467 pass, 0 fail, 0 cancelled, 33 skipped (no Chrome); exit 0 |
| `node --test "tests/fixtures/prototype/*.test.mjs"` | 21 tests: 19 pass, 0 fail, 2 skipped (Quick Quote archive at a macOS path); exit 0 |
| `node tooling/harness/run-language.mjs --dir tests/language/lowering` | 166 pass, 0 FAIL, 0 blocked, 0 compile-error, 0 unexpected; exit 0 |
| `node tooling/oracle/ui/lint.mjs` (compiles every UI scenario component) | 174 cases, 0 problems; exit 0 |

Also run: every `.vue` file in the repository (363, excluding `node_modules`, `target` and `.git`) was compiled with the
`HEAD` compiler (from `git archive HEAD compiler`) and with the working-tree compiler. Generated code, source maps, styles,
bindings, scope ids, dependencies and diagnostics are identical for all 363. 72 of them report errors under both compilers
and produce no module.

## Negative controls

| Control | Observed |
|---|---|
| Prototype output for `@[click]` run through the rejection assertion | Fails with "no module is produced": the prototype accepts the input and returns Rust, shaped as a pipeline result with no diagnostics. Its output is also asserted equal to its output for `@click`. |
| Fixed `@click` build (what the prototype emits) run through the runtime assertion | Fails: the first click already makes the count 1, where Vue gives 0. |
| Span assertion before the template fix | Fails: start offset 121 against the expected 122. |
| `<Comp v-model:[nm]>` with `nm` undeclared, before the fix | Accepted with no diagnostic. Now rejected with VN4016 on `nm`, and asserted. |
| The HEAD-versus-working-tree comparison run on a scratch file with `<Comp v-model:[nm]>` and `nm` undeclared | Flagged: HEAD accepts the file, the working tree reports VN4016 on `nm` (offsets 110 to 112). |
| Mutated catalogue: field removed, field added, field renamed, kind dropped | Each changes the hash. |
| Unknown node kind `bogus` appended to a real module | Reported at `$.body[2]`. |
| Extra field on a `num` node; `length` deleted from a function record | Both reported. |
| Catalogue variant that no module produces | Reported as unmet. |
| Producer drift: a field added to every `null` node in `compiler/hir/expr.mjs` | The shape and coverage tests fail and list the field on each node. The file was then restored (checked byte for byte). |

## Remaining limits

- The headless runner dispatches only `click` and `input` events. The test shows that the handler follows the variable
  between `input` and `click`. The ticket's example, `mouseenter`, is not dispatched, because a new operation would
  change `tooling/cli/headless/main.rs`.
- No browser test was added. The 33 Chrome-gated tests were skipped here, as before.
- The regression file builds two headless release binaries (LTO profile) and takes about 73 s in this container. The
  compiler suite went from 4.97 s (baseline run) to 84.5 s (final run).
- The hash covers the catalogue, not the producer code. A change to the producer is caught only when a walked module
  exercises it.
- Records are matched by their key set, not by position.
- The hash is not written into emitted Rust. That was optional, and it would change every generated module.
- `compiler/hir/module.mjs` writes `hirVersion: 1` as a literal, not `HIR_VERSION`. The test checks that they agree.
- `hirVersion` is not part of the hash. A shape change fails the hash test until the new hash is added to `COMMITTED`
  under a bumped `HIR_VERSION`.
- The UI scenario `events/dynamic-event-name` (`@[props.evt]`) is one of the 363 files above, so its compiled output is
  unchanged. Its browser run is compared with `tests/ui/golden/events/dynamic-event-name.json` and needs Chrome, so it was
  not run here.

## Open questions

- The VN4016 message for an undeclared dynamic argument reads "global 'click' is not available in the native profile".
  It does not say the name is undeclared. That wording comes from the stdlib check and is unchanged.
- The `template.mjs` change goes beyond the regression test. It corrects a span and closes a silent acceptance of the
  same kind. It is small and covered by tests, but it is outside the ticket's listed scope.
- On `@` and `:`, the dynamic-argument check now runs before the modifier check. When one directive has both an invalid
  modifier and an undeclared argument, the two diagnostics may come out in the other order. The comparison of all 363
  files showed no difference, so no file in the repository has that case.

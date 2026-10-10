# Test262 subset — 2026-10-10

WP 3.1, Part B (ticket w31-size). A subset of tc39/test262 is vendored under `tests/language/test262/` and run through the compiler by `tooling/harness/run-test262.mjs`. Each test has one of three outcomes: pass, fail, or rejected-by-profile. A rejected-by-profile test is not a pass. The three counts are reported separately below.

## Upstream checkout

- Repository: https://github.com/tc39/test262
- Commit: `2e0a56762801e275a9fdf96dc49d90ba0cddcf63` (committer date 2026-10-08T13:50:06+02:00; subject "Add coverage for number conversion in JSON.stringify and Array.p.join").
- Checkout: `/home/claude/scratch/test262`, outside the repository. It is a shallow clone, as the ticket asks: `git rev-parse --is-shallow-repository` prints `true` and `git rev-list --count HEAD` prints `1`.
- License: BSD-3-Clause. The upstream LICENSE is copied to `tests/language/test262/LICENSE` (sha256 `4dd9244dfe8197c75348c4b24ab53d29d3b1cfad143ac76b5a3d8942aa354ce0`).

## Selection rules

The rules are code in `tooling/harness/select-test262.mjs`. They are applied to the metadata of every `.js` file under the 33 directories listed in the table below. A file is kept when no rule excludes it. Kept files are then sampled.

1. A `*_FIXTURE.js` module is not a test.
2. Flags must be `generated` or `onlyStrict`. `noStrict`, `module`, `async` and `raw` tests are excluded.
3. Every feature must be one of 20 tags of a default-deny list: let, const, arrow-function, destructuring-binding, destructuring-assignment, object-rest, computed-property-names, for-of, for-in-order, optional-catch-binding, optional-chaining, coalesce-expression, exponentiation, Symbol, Symbol.iterator, Symbol.toPrimitive, Map, Set, Array.prototype.includes, well-formed-json-stringify. Every other feature excludes the file. The list follows the language profile's rows (`compiler/profile/manifest.json`).
4. Includes must be `compareArray.js`. `assert.js` and `sta.js` are prepended to every test.
5. Negative tests: parse phase with `SyntaxError`; runtime phase with `ReferenceError`, `TypeError` or `RangeError`.
6. Sampling: at most 10 kept files per directory, taken at the evenly spaced positions `floor(i * n / 10)` of the sorted paths. A directory with 10 or fewer kept files keeps all of them.

Counts, from `node tooling/harness/select-test262.mjs --upstream /home/claude/scratch/test262` (the command prints the same tables):

- 3126 `.js` files walked in the 33 directories; 0 with unreadable metadata.
- 629 excluded by a rule (the first failing rule is recorded; table below).
- 2497 kept by the rules; 307 selected by sampling; 2190 kept but not sampled.

Selected per directory:

| Directory (under test/) | Kept by the rules | Selected |
|---|---:|---:|
| `language/expressions/addition` | 41 | 10 |
| `language/expressions/equals` | 38 | 10 |
| `language/expressions/does-not-equals` | 29 | 10 |
| `language/expressions/strict-equals` | 21 | 10 |
| `language/expressions/strict-does-not-equals` | 21 | 10 |
| `language/expressions/typeof` | 14 | 10 |
| `language/expressions/logical-and` | 16 | 10 |
| `language/expressions/logical-or` | 16 | 10 |
| `language/expressions/conditional` | 20 | 10 |
| `language/expressions/comma` | 5 | 5 |
| `language/statements/let` | 118 | 10 |
| `language/statements/const` | 111 | 10 |
| `language/statements/for` | 313 | 10 |
| `language/statements/for-of` | 529 | 10 |
| `language/statements/labeled` | 12 | 10 |
| `language/statements/switch` | 67 | 10 |
| `language/statements/try` | 172 | 10 |
| `built-ins/Number/prototype/toFixed` | 11 | 10 |
| `built-ins/Number/prototype/toString` | 86 | 10 |
| `built-ins/String/prototype/charAt` | 27 | 10 |
| `built-ins/String/prototype/slice` | 35 | 10 |
| `built-ins/String/prototype/indexOf` | 42 | 10 |
| `built-ins/String/prototype/padStart` | 9 | 9 |
| `built-ins/Math/abs` | 4 | 4 |
| `built-ins/Math/floor` | 7 | 7 |
| `built-ins/Math/max` | 6 | 6 |
| `built-ins/Math/min` | 6 | 6 |
| `built-ins/Array/prototype/push` | 20 | 10 |
| `built-ins/Array/prototype/map` | 191 | 10 |
| `built-ins/Array/prototype/filter` | 217 | 10 |
| `built-ins/Array/prototype/indexOf` | 191 | 10 |
| `built-ins/JSON/stringify` | 45 | 10 |
| `built-ins/JSON/parse` | 57 | 10 |

Excluded by rule, first failing rule:

| Rule (first failing) | Files |
|---|---:|
| feature generators | 224 |
| include propertyHelper.js | 101 |
| flag noStrict | 78 |
| feature BigInt | 48 |
| feature Proxy | 26 |
| feature TypedArray | 20 |
| feature Reflect.construct | 18 |
| feature Symbol.species | 18 |
| feature tail-call-optimization | 16 |
| feature async-functions | 16 |
| feature async-iteration | 16 |
| feature resizable-arraybuffer | 15 |
| feature class-static-block | 7 |
| feature class | 6 |
| feature explicit-resource-management | 5 |
| feature cross-realm | 5 |
| feature json-parse-with-source | 5 |
| flag module | 4 |
| flag async | 1 |

## Vendored files

`node tooling/harness/select-test262.mjs --upstream /home/claude/scratch/test262 --write` copies the 307 selected files to `test/`, the three harness files they need (`assert.js`, `sta.js`, `compareArray.js`) to `harness/`, the LICENSE, and a README that states the commit and the rules. `--check` compares the vendored tree with the rules applied to the checkout.

```
node tooling/harness/select-test262.mjs --upstream /home/claude/scratch/test262 --check
```

Result: `check: 311 expected files (tests, harness, LICENSE), 311 vendored, 0 mismatch(es) at 2e0a56762801e275a9fdf96dc49d90ba0cddcf63`, exit 0. The 311 are 307 tests, 3 harness files and LICENSE. `controls/` (the negative control) and `status.json` (the ledger) are outside the check.

## Adaptation

The vendored files are byte-identical to upstream, except the control file in `controls/`, which is not a selected test. The runner changes only the module text it compiles (`adaptForProfile` in `tooling/harness/run-test262.mjs`):

- `new Test262Error(` becomes `Test262Error(` everywhere.
- In `sta.js`, the Test262Error constructor becomes `function Test262Error(message) { return { constructor: Test262Error, message: message || "" }; }`, and the `Test262Error.prototype.toString` assignment is removed.

The reason: `new` of a function that the program defines is outside the language profile (L05, VN2007). Every harness assertion constructs its error that way, so without the change every test that asserts is rejected for that reason alone. The returned object keeps `constructor`, so `assert.throws(Test262Error, ...)` still compares the thrown object's constructor with the function, as upstream does. Ten selected test files use that form (12 calls). No selected test uses `instanceof Test262Error`, `Test262Error.prototype` or `Test262Error.thrower`. Nothing else is rewritten; a test that needs other constructs is rejected by the profile.

An earlier version of this adaptation dropped the `constructor` property, which broke the `assert.throws(Test262Error, ...)` tests (they failed with "Expected a Test262Error but got a Object"); the property was added back. The first README draft said 11 selected tests; the count of files is 10 (12 calls).

## Results

Command:

```
export CARGO_TARGET_DIR=/home/claude/targets/w31-size
node tooling/harness/run-test262.mjs
```

Output: `test262 307 files: pass 232, fail 0, rejected-by-profile 75; blocked 75; 0 unexpected`, exit 0.

The 232 passes are: 214 tests that compile and run to completion without a throw, and 18 negative parse tests that the compiler rejects with a syntax error at parse time (VN1000 or VN5001), which is what they expect. No negative runtime test passes: the only selected one, `language/statements/switch/scope-lex-async-generator` (expects a ReferenceError), uses generators and is rejected by the profile.

The 75 rejected-by-profile tests are listed in the next section. Failures: 0.

## Rejected by profile (75)

Each row is the first error the compiler reported for the test's module. The file reasons are copied from the ledger (`tests/language/test262/status.json`), which the runner writes.

| # | Test (under test/) | Code | Reason (the first error the compiler reported) |
|---|---|---|---|
| 1 | `built-ins/Array/prototype/filter/15.4.4.20-2-11.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 22) |
| 2 | `built-ins/Array/prototype/filter/15.4.4.20-4-2.js` | VN4016 | VN4016 global 'foo' is not available in the test profile (line 13) |
| 3 | `built-ins/Array/prototype/filter/15.4.4.20-9-c-i-10.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 18) |
| 4 | `built-ins/Array/prototype/filter/15.4.4.20-9-c-i-31.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 24) |
| 5 | `built-ins/Array/prototype/indexOf/15.4.4.14-2-11.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 14) |
| 6 | `built-ins/Array/prototype/indexOf/15.4.4.14-5-30.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 14) |
| 7 | `built-ins/Array/prototype/indexOf/15.4.4.14-9-a-16.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 13) |
| 8 | `built-ins/Array/prototype/indexOf/15.4.4.14-9-b-i-16.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 11) (+2 more) |
| 9 | `built-ins/Array/prototype/map/15.4.4.19-2-13.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 16) (+1 more) |
| 10 | `built-ins/Array/prototype/map/15.4.4.19-8-b-5.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 21) (+1 more) |
| 11 | `built-ins/Array/prototype/map/15.4.4.19-8-c-i-22.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 18) |
| 12 | `built-ins/Array/prototype/map/15.4.4.19-8-c-ii-13.js` | VN2015 | VN2015 The arguments object is not supported (line 12) (+2 more) |
| 13 | `built-ins/Array/prototype/push/length-near-integer-limit-set-failure.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 34) |
| 14 | `built-ins/Array/prototype/push/set-length-array-is-frozen.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 32) |
| 15 | `built-ins/JSON/parse/reviver-array-non-configurable-prop-create.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 39) |
| 16 | `built-ins/JSON/parse/reviver-object-non-configurable-prop-create.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 41) |
| 17 | `built-ins/JSON/stringify/property-order.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 17) |
| 18 | `built-ins/Number/prototype/toFixed/S15.7.4.5_A1.3_T01.js` | VN4016 | VN4016 global 's' is not available in the test profile (line 31) |
| 19 | `built-ins/Number/prototype/toFixed/S15.7.4.5_A1.3_T02.js` | VN4016 | VN4016 global 's' is not available in the test profile (line 21) |
| 20 | `built-ins/Number/prototype/toFixed/S15.7.4.5_A1.4_T01.js` | VN4016 | VN4016 global 's' is not available in the test profile (line 70) |
| 21 | `built-ins/Number/prototype/toString/S15.7.4.2_A4_T03.js` | VN4016 | VN4016 global 'Date' is not available in the test profile (line 13) (+2 more) |
| 22 | `built-ins/String/prototype/charAt/S15.5.4.4_A1.1.js` | VN2007 | VN2007 `new` is supported only for built-in constructors and imported bindings (line 18) (+1 more) |
| 23 | `built-ins/String/prototype/charAt/S15.5.4.4_A7.js` | VN2007 | VN2007 `new` is supported only for built-in constructors and imported bindings (line 13) |
| 24 | `built-ins/String/prototype/indexOf/S15.5.4.7_A3_T2.js` | VN4016 | VN4016 global 'eval' is not available in the test profile (line 14) |
| 25 | `built-ins/String/prototype/slice/S15.5.4.13_A1_T5.js` | VN4016 | VN4016 global 'Function' is not available in the test profile (line 20) |
| 26 | `language/expressions/addition/S11.6.1_A1.js` | VN4016 | VN4016 global 'eval' is not available in the test profile (line 13) |
| 27 | `language/expressions/addition/coerce-symbol-to-prim-return-obj.js` | VN2015 | VN2015 The arguments object is not supported (line 42) |
| 28 | `language/expressions/comma/S11.14_A1.js` | VN4016 | VN4016 global 'eval' is not available in the test profile (line 13) |
| 29 | `language/expressions/comma/S11.14_A2.1_T2.js` | VN4016 | VN4016 global 'x' is not available in the test profile (line 12) |
| 30 | `language/expressions/comma/S11.14_A2.1_T3.js` | VN4016 | VN4016 global 'y' is not available in the test profile (line 12) |
| 31 | `language/expressions/conditional/S11.12_A1.js` | VN4016 | VN4016 global 'eval' is not available in the test profile (line 14) |
| 32 | `language/expressions/conditional/S11.12_A2.1_T2.js` | VN4016 | VN4016 global 'x' is not available in the test profile (line 12) |
| 33 | `language/expressions/conditional/S11.12_A2.1_T4.js` | VN4016 | VN4016 global 'z' is not available in the test profile (line 14) |
| 34 | `language/expressions/conditional/S11.12_A2.1_T6.js` | VN4016 | VN4016 global 'y' is not available in the test profile (line 12) |
| 35 | `language/expressions/does-not-equals/S11.9.2_A1.js` | VN4016 | VN4016 global 'eval' is not available in the test profile (line 13) |
| 36 | `language/expressions/does-not-equals/S11.9.2_A2.1_T2.js` | VN4016 | VN4016 global 'x' is not available in the test profile (line 12) |
| 37 | `language/expressions/does-not-equals/S11.9.2_A6.1.js` | VN4016 | VN4016 global 'eval' is not available in the test profile (line 21) |
| 38 | `language/expressions/equals/S11.9.1_A1.js` | VN4016 | VN4016 global 'eval' is not available in the test profile (line 13) |
| 39 | `language/expressions/equals/S11.9.1_A2.1_T3.js` | VN4016 | VN4016 global 'y' is not available in the test profile (line 12) |
| 40 | `language/expressions/equals/get-symbol-to-prim-err.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 26) |
| 41 | `language/expressions/logical-and/S11.11.1_A1.js` | VN4016 | VN4016 global 'eval' is not available in the test profile (line 13) |
| 42 | `language/expressions/logical-and/S11.11.1_A2.1_T3.js` | VN4016 | VN4016 global 'y' is not available in the test profile (line 14) |
| 43 | `language/expressions/logical-and/S11.11.1_A2.1_T4.js` | VN4016 | VN4016 global 'x' is not available in the test profile (line 11) |
| 44 | `language/expressions/logical-or/S11.11.2_A1.js` | VN4016 | VN4016 global 'eval' is not available in the test profile (line 13) |
| 45 | `language/expressions/logical-or/S11.11.2_A2.1_T3.js` | VN4016 | VN4016 global 'y' is not available in the test profile (line 14) |
| 46 | `language/expressions/logical-or/S11.11.2_A2.1_T4.js` | VN4016 | VN4016 global 'x' is not available in the test profile (line 11) |
| 47 | `language/expressions/strict-does-not-equals/S11.9.5_A1.js` | VN4016 | VN4016 global 'eval' is not available in the test profile (line 13) |
| 48 | `language/expressions/strict-does-not-equals/S11.9.5_A2.1_T2.js` | VN4016 | VN4016 global 'x' is not available in the test profile (line 12) |
| 49 | `language/expressions/strict-does-not-equals/S11.9.5_A2.4_T3.js` | VN4016 | VN4016 global 'x' is not available in the test profile (line 12) |
| 50 | `language/expressions/strict-equals/S11.9.4_A1.js` | VN4016 | VN4016 global 'eval' is not available in the test profile (line 13) |
| 51 | `language/expressions/strict-equals/S11.9.4_A2.1_T2.js` | VN4016 | VN4016 global 'x' is not available in the test profile (line 12) |
| 52 | `language/expressions/strict-equals/S11.9.4_A2.4_T3.js` | VN4016 | VN4016 global 'x' is not available in the test profile (line 12) |
| 53 | `language/expressions/typeof/built-in-exotic-objects-no-call.js` | VN4016 | VN4016 global 'Date' is not available in the test profile (line 59) (+1 more) |
| 54 | `language/expressions/typeof/get-value.js` | VN4017 | VN4017 'Object.defineProperties' is not supported (line 20) |
| 55 | `language/expressions/typeof/get-value-ref-err.js` | VN4016 | VN4016 global 'x' is not available in the test profile (line 24) |
| 56 | `language/expressions/typeof/null.js` | VN4016 | VN4016 global 'RegExp' is not available in the test profile (line 28) |
| 57 | `language/expressions/typeof/string.js` | VN4016 | VN4016 global 'Date' is not available in the test profile (line 52) |
| 58 | `language/expressions/typeof/syntax.js` | VN4016 | VN4016 global 'eval' is not available in the test profile (line 14) |
| 59 | `language/statements/const/dstr/ary-ptrn-elem-obj-prop-id.js` | VN4016 | VN4016 global 'u' is not available in the test profile (line 39) (+2 more) |
| 60 | `language/statements/for-of/dstr/const-obj-ptrn-prop-id-init-skipped.js` | VN4016 | VN4016 global 's' is not available in the test profile (line 61) (+3 more) |
| 61 | `language/statements/for/dstr/var-ary-init-iter-get-err-array-prototype.js` | VN4016 | VN4016 global 'iterCount' is not available in the test profile (line 52) |
| 62 | `language/statements/for/dstr/var-obj-ptrn-prop-ary.js` | VN4016 | VN4016 global 'w' is not available in the test profile (line 49) |
| 63 | `language/statements/labeled/cptn-break.js` | VN4016 | VN4016 global 'eval' is not available in the test profile (line 19) |
| 64 | `language/statements/labeled/cptn-nrml.js` | VN4016 | VN4016 global 'eval' is not available in the test profile (line 20) |
| 65 | `language/statements/labeled/decl-fun-strict.js` | VN2023 | VN2023 Function declaration in a statement position is not supported (line 17) |
| 66 | `language/statements/labeled/value-await-non-module.js` | VN1000 | VN1000 Unexpected token (line 14) |
| 67 | `language/statements/labeled/value-await-non-module-escaped.js` | VN1000 | VN1000 Unexpected reserved word 'await' (line 15) |
| 68 | `language/statements/let/dstr/obj-ptrn-prop-id-get-value-err.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 27) |
| 69 | `language/statements/switch/cptn-a-fall-thru-nrml.js` | VN4016 | VN4016 global 'eval' is not available in the test profile (line 53) |
| 70 | `language/statements/switch/cptn-dflt-b-abrupt-empty.js` | VN4016 | VN4016 global 'eval' is not available in the test profile (line 53) |
| 71 | `language/statements/switch/cptn-dflt-final.js` | VN4016 | VN4016 global 'eval' is not available in the test profile (line 40) |
| 72 | `language/statements/switch/scope-lex-async-generator.js` | VN2008 | VN2008 Generator and async generator functions are not supported (line 21) (+1 more) |
| 73 | `language/statements/try/dstr/ary-ptrn-elem-id-init-unresolvable.js` | VN4016 | VN4016 global 'unresolvableReference' is not available in the test profile (line 38) |
| 74 | `language/statements/try/dstr/ary-ptrn-rest-id-iter-val-err.js` | VN4017 | VN4017 'Object.defineProperty' is not supported (line 29) |
| 75 | `language/statements/try/dstr/obj-ptrn-prop-id.js` | VN4016 | VN4016 global 'x' is not available in the test profile (line 32) |

By code: VN4016, a global the profile does not provide, 48 (direct `eval` 17; `Date` 3; `RegExp` 1; `Function` 1; 26 identifiers that the file does not declare (no `var`, `let`, `const`, `function` or `class` declaration of the name; five are only assigned, as `s = ...` in `built-ins/Number/prototype/toFixed/S15.7.4.5_A1.3_T01`), such as `x` in `language/expressions/comma/S11.14_A2.1_T2`, which expects a ReferenceError at run time; the closed-world profile rejects an undeclared identifier at compile time, so those tests never run). VN4017, `Object.defineProperty` and `Object.defineProperties` not supported, 19. VN2015, the arguments object, 2. VN2007, `new` of a function the program defines, 2. VN2023, a function declaration in statement position, 1 (`language/statements/labeled/decl-fun-strict`, a negative parse test: the compiler reports the unsupported construct, not the SyntaxError, so the test is rejected rather than passed). VN1000, `await` used as an identifier, 2 (the runner compiles modules, where `await` is reserved). VN2008, generator functions, 1.

## Negative control

`tests/language/test262/controls/S15.8.2.1_A1-wrong-expected.js` is a copy of `test/built-ins/Math/abs/S15.8.2.1_A1.js`, which passes, with the expected value changed from `NaN` to `0`. It is not a selected test. The runner must report it as a fail:

```
node tooling/harness/run-test262.mjs --dir tests/language/test262/controls --no-ledger
```

Result: `fail S15.8.2.1_A1-wrong-expected [UNEXPECTED] -- unexpected throw: thrown object with message "Expected SameValue(«NaN», «0») to be true"`, then `test262 1 files: pass 0, fail 1, rejected-by-profile 0; blocked 0; 1 unexpected`, exit 1.

The runner's unit tests (`tests/tooling/harness/test262.test.mjs`) include the same control and check that the runner exits 1 on it. They also cover the frontmatter reader, the rules, the sampling, the adaptation (including a sta.js whose text changed, which must be refused) and the ledger rule, with hand-written inputs. Result: `node --test tests/tooling/harness/test262.test.mjs`: 17 tests, 17 pass, 0 fail, 0 skipped.

## Limits

- Strict only, as a module. The runner compiles each test as a module, so only the strict variant runs. test262's sloppy variant of an unflagged test is not run. Behaviour that depends on the script goal is not checked.
- A negative parse test passes when the compiler reports a syntax error (VN1000) or a duplicate declaration (VN5001) at parse time. The runner does not check that the diagnostic is the early error the test names. The 18 passes report 15 VN1000 syntax errors and 3 VN5001 duplicate declarations (the verbose listing has each message). I checked the source line for seven of them by hand; each is the construct the test names. The runner does not compare the message or the early-error rule, so a parser error for another reason at the same construct would also pass. The one other negative parse test, `language/statements/labeled/decl-fun-strict`, is rejected by the profile (VN2023) and is not counted as a pass.
- The pass count describes the 307-file sample, not the directories. 2190 kept files were not vendored.
- Expected results come from the tests' own assertions, which test262 wrote independently of this compiler. No engine was used to compute expected values.
- The adaptation changes the harness text, as described above. Removing `Test262Error.prototype.toString` changes the string form of a Test262Error object (upstream `Test262Error: message`; here `[object Object]`). A search of the selected files for `toString()` and `String(e)` found no conversion of a Test262Error object, so no selected test depends on it. That search is a grep, not a proof.
- The ledger is a ratchet. A test marked blocked that passes makes the run fail until its entry is removed. A test that is not in the ledger must pass.
- Chrome cannot run in this container. Nothing here was run in a browser.

## Files

- `tooling/harness/test262-meta.mjs`, `tooling/harness/select-test262.mjs`, `tooling/harness/run-test262.mjs` (new)
- `tests/tooling/harness/test262.test.mjs` (new)
- `tests/language/test262/` (`test/`, `harness/`, `LICENSE`, `README.md`, `status.json`, `controls/`)

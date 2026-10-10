# Test262 subset (WP 3.1)

Upstream: https://github.com/tc39/test262 at commit `2e0a56762801e275a9fdf96dc49d90ba0cddcf63` (a shallow checkout; the files are copied unchanged).
License: BSD-3-Clause, see LICENSE in this directory (copied from the upstream root).

Vendored: 307 test files under test/ and 3 harness files under harness/ (assert.js, sta.js, compareArray.js).
Only the files these rules select are vendored, plus the harness files they include.
controls/ holds the runner's negative control (a copy of a passing test with a wrong expected value). It is not a selected test.

## Selection rules

Implemented in tooling/harness/select-test262.mjs and applied to the metadata of every file in these directories:

- test/language/expressions/addition
- test/language/expressions/equals
- test/language/expressions/does-not-equals
- test/language/expressions/strict-equals
- test/language/expressions/strict-does-not-equals
- test/language/expressions/typeof
- test/language/expressions/logical-and
- test/language/expressions/logical-or
- test/language/expressions/conditional
- test/language/expressions/comma
- test/language/statements/let
- test/language/statements/const
- test/language/statements/for
- test/language/statements/for-of
- test/language/statements/labeled
- test/language/statements/switch
- test/language/statements/try
- test/built-ins/Number/prototype/toFixed
- test/built-ins/Number/prototype/toString
- test/built-ins/String/prototype/charAt
- test/built-ins/String/prototype/slice
- test/built-ins/String/prototype/indexOf
- test/built-ins/String/prototype/padStart
- test/built-ins/Math/abs
- test/built-ins/Math/floor
- test/built-ins/Math/max
- test/built-ins/Math/min
- test/built-ins/Array/prototype/push
- test/built-ins/Array/prototype/map
- test/built-ins/Array/prototype/filter
- test/built-ins/Array/prototype/indexOf
- test/built-ins/JSON/stringify
- test/built-ins/JSON/parse

1. Fixture modules (`*_FIXTURE.js`) are not tests.
2. Flags must be in: generated, onlyStrict. Sloppy-only, module, async and raw tests are excluded.
   The runner compiles every test as a module, so only the strict variant runs. A test with no flag is run in strict
   mode only; test262 also defines a sloppy variant for it, which is not run.
3. Every feature must be in the supported list: let, const, arrow-function, destructuring-binding, destructuring-assignment, object-rest, computed-property-names, for-of, for-in-order, optional-catch-binding, optional-chaining, coalesce-expression, exponentiation, Symbol, Symbol.iterator, Symbol.toPrimitive, Map, Set, Array.prototype.includes, well-formed-json-stringify.
4. Includes must be in: compareArray.js (assert.js and sta.js are always prepended).
5. Negative tests: parse phase with SyntaxError; runtime phase with ReferenceError, TypeError, RangeError.
6. The metadata must be readable by tooling/harness/test262-meta.mjs.

## Adaptation

Every file here except controls/ is byte-identical to upstream. The runner (tooling/harness/run-test262.mjs, `adaptForProfile`) changes
the module text it compiles, in these ways only: `new Test262Error(` becomes `Test262Error(` everywhere; in sta.js the
Test262Error constructor becomes a function that returns `{ constructor: Test262Error, message }`, and its
prototype.toString line is removed. Reason: `new` of a function the program defines is outside the language profile
(L05, VN2007), and every assertion of the harness constructs its error that way; a function the program defines has no
prototype object in the runtime. The `constructor` property keeps `assert.throws(Test262Error, ...)` (10 selected test files)
meaning what it means upstream. Other constructs are not rewritten: a test that needs them is rejected by the profile.

Run: `node tooling/harness/run-test262.mjs` (ledger: status.json). Check this tree against an upstream checkout:
`node tooling/harness/select-test262.mjs --upstream <checkout> --check`.

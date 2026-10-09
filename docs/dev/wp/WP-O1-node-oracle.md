# WP-O1 — Node-side oracle for language scenarios (trace format v1) + seed scenario corpus

Read `docs/dev/AGENT-CONTRACT.md`, `docs/dev/ARCHITECTURE.md`, and **`docs/dev/trace-format.md`** (the contract you
implement) first. WP id `o1`; no cargo needed.

**Owned paths:** `tooling/oracle/**`, `tests/language/cases/**`, `tests/language/golden/**`,
`tests/language/oracle.test.mjs`.

## Why
Language semantics (L01–L21) and later Vue behaviour are verified **differentially**: the same scenario module runs
in Node (oracle) and in the compiled Rust runtime; both emit the canonical trace; the harness compares them. You
build the Node side and the first corpus of scenarios. The Rust side comes later and must produce byte-compatible
documents, so the format must be implemented *exactly* as written, including edge cases.

## Do
1. `tooling/oracle/trace.mjs` — `createRecorder()` returning `{ trace, entries, encode, ids }` implementing the
   encoding in trace-format.md (global ordinals for objects and symbols, per-call `ref` for aliases/cycles, holes,
   accessors without invoking getters, Map/Set order, Error name/message only, `-0`/NaN/Infinity, lone surrogates,
   unsupported values → `TypeError("untraceable value")`). Detect classes by `Array.isArray`, `instanceof Map/Set/
   Error/Promise`, `typeof === 'function'`.
2. `tooling/oracle/node-runner.mjs` — `runScenario(filePath) -> Promise<Document>` implementing the scenario contract
   exactly (steps 1–5 in the spec): installs globals `trace` and `testHost` (`timers.setTimeout/clearTimeout/now/
   advance`, `tick`) with deterministic fake time; imports the module with a unique URL (no cache); records a throw at
   module evaluation; drains jobs with `await new Promise(r => setImmediate(r))` loops until stable; calls
   `run()` if exported; captures unhandled rejections via `process.on('unhandledRejection')` without crashing; returns
   the document. Each scenario must run in a fresh realm state (reset globals, ordinals) — use a **child process**
   per scenario (`node tooling/oracle/run.mjs <file>` prints one JSON line) so scenarios cannot interfere, and expose
   `runScenario` as spawning that child.
3. `tooling/oracle/compare.mjs` — `compareDocs(a, b)` → `null` or `{ path, a, b }` (first difference, structural,
   object key order irrelevant, array order significant; number/`$num` encodings compared exactly).
4. `tooling/oracle/run.mjs` — CLI: `node tooling/oracle/run.mjs <scenario.js>` prints the document on one line.
   `tooling/oracle/check-goldens.mjs` — runs every `tests/language/cases/**/*.js`, compares to
   `tests/language/golden/<same relative path>.json`, `--update` rewrites goldens (default is check only).
5. **Seed scenarios** (60–80 files, small and focused, one concern each) under `tests/language/cases/<group>/…js`.
   Each file starts with `// @tier N` (1 basic, 2 intermediate, 3 async/modules) and `// @covers L01,L02` header
   comments. **Only use features in the supported profile** (they will be compiled by our own compiler): `const/let/
   var`, functions, arrows, closures, `this` in ordinary calls, `if/for/while/do-while/for-of/for-in/switch/break/
   continue/labels`, `try/catch/finally`, `throw` of any value, object & array literals, spread/rest, destructuring,
   template literals, optional chaining, `??`, `typeof`, `in`, `delete`, getters/setters in **object literals**,
   `Symbol`, `Map`, `Set`, `Promise` (+ `then/catch/finally/all/allSettled/race/any`), `async/await`, `JSON`,
   `Math`, `Number`/`String`/`Array`/`Object` static & prototype methods that are standard and deterministic
   (`Object.keys/values/entries/assign/freeze/isFrozen/fromEntries`, `Array.prototype.map/filter/reduce/forEach/
   slice/splice/concat/join/indexOf/includes/find/findIndex/some/every/sort/reverse/push/pop/shift/unshift/flat`, `String.prototype.*`
   basics). **Forbidden**: classes, `new` of user functions, prototypes/`__proto__`, `Proxy`, `Reflect`, `eval`,
   `with`, generators, regular expressions, `Date`, `Intl`, `BigInt`, typed arrays, `WeakMap`, labeled `function`,
   `arguments` object, getters via `Object.defineProperty`, `Math.random`, real timers, Node/DOM globals.
   Cover: scoping/TDZ/var hoisting/per-iteration `let` capture, closures capturing mutable state, aliasing and identity,
   cycles, property order (integer keys first), sparse arrays and holes, `null` vs `undefined`, coercion tables (`==`,
   `+`, `<`, truthiness incl. `-0`, `NaN`, `""`, `[]`), number formatting (`0.1+0.2`, `1e21`, `toFixed` ties), UTF-16
   (emoji length/indexing/slice, lone surrogates), Map/Set order and mutation during iteration, exceptions
   (`finally` overriding `return`, nested rethrow, thrown non-Errors), promise job ordering (`then` chains, `await`
   vs `.then` interleaving, thenable assimilation, multiple resolve), async functions with try/finally, unhandled
   rejection, timers via `testHost.timers.advance`. Also at least 3 scenarios that **throw at module level** and
   2 whose `run()` rejects.
   Goldens for every scenario are generated by Node 26 (the oracle) and committed.
6. `tests/language/oracle.test.mjs` (node:test, ≥ 25 focused tests): every row of the encoding table with
   expectations written **by hand** in the test (not copied from encoder output); the scenario contract (module
   throw, `run` rejection, ordering of module-init vs `run`, timers firing order with job drain between callbacks,
   unhandled rejection capture, isolation between two scenarios that both create objects → ordinals restart at 1);
   `compareDocs` positive/negative cases; determinism (run each scenario twice, byte-identical); golden check for all
   seed scenarios; **negative control**: a scenario whose output differs from a doctored golden must be reported.

## Acceptance (the reviewer will run)
`node --test tests/language/oracle.test.mjs` passes; `node tooling/oracle/check-goldens.mjs` reports all scenarios
match; goldens regenerate byte-identically; header tags present on every case; no forbidden features in cases
(add a lint test that greps for the forbidden constructs with a tiny Babel-based checker using the installed
`@babel/parser`, and verifies the lint itself detects a class declaration).

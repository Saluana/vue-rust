# Builtin-call corpora

Data-driven differential tests for builtin methods (`docs/dev/builtin-corpus.md`). Each suite here is one JSON file.
Node (V8) produces every expected value; the Rust replay harness re-executes each case from its `pre` encoding and
compares its `post`.

## Seed suites

| File | Source | Content |
|---|---|---|
| `math.json` | `tests/language/gen/gen-builtins.mjs` | `Math` functions and constants from the WP-O2 list |
| `number.json` | same | `Number` statics, `Number.prototype` methods, `Number(...)` coercions |
| `object-statics.json` | same | `Object` statics on plain objects, arrays, strings, aliases, nullish arguments |

Commands (run from the repository root):

```sh
node tests/language/gen/gen-builtins.mjs                      # regenerate (byte-identical on every run)
node tooling/oracle/builtin-corpus.mjs --check tests/language/builtins/math.json   # replay one suite; exit 1 on mismatch
node --test "tests/language/builtin-corpus.test.mjs"          # library and suite tests
```

## Case format

`{ "id", "target", "static"?, "approx"?, "get"?, "cbs"?, "pre", "post": { "ret", "throw", "state" } }`, as in
`docs/dev/builtin-corpus.md`, with these conventions:

- `target` is a dotted path from the global object. `"new:Map"` constructs. `static: true` means `this` is the owner
  object and is not part of `pre`.
- `"get": 1` marks a data property read (`Math.PI`): nothing is called, `pre` is `[]`, `post.ret` is the value.
- `"approx": 1` marks results that may differ from the platform libm by one ulp (`sin`, `exp`, `pow`, `hypot`, ...).
- `"cbs"` aligns with `pre` (`[this?, ...args]`); a callback preset name at a position means that argument is the
  preset. The preset appears in `pre` as `{"$":"fn","name","length"}`. Only the callback-taking targets in
  `CALLBACK_ARITY` (`Array.prototype.map`, `reduce`, `sort`) may carry `cbs`.
- Callback presets log to a shared array that is the last element of `post.state`. Each call appends
  `[name, ...args]`. A primitive argument is logged as itself. An object, array or function argument is logged as the
  index the calling method passed (second callback argument for map-like methods, third for reduce), or `-1` when
  none is passed (sort). Object identity is never logged.
- Objects carry `integrity` (trace-format v1.1) when they are non-extensible: `frozen`, `sealed` or `nonextensible`.

## Targets

The standard-library manifest decides (`compiler/profile/stdlib.json`, target `test`). A target must be a `static`,
`proto`, `ctor` or `const` entry with status P or A. X, E and absent entries are rejected with the manifest hint.
Global-call forms (kind `global`, such as `parseInt`) are rejected; use the owning object (`Number.parseInt`).
Host clock and random providers (`Math.random`, `Date.now`, `performance.now`) are refused even though they are A:
V8 is not seeded, so the oracle cannot record them.

## Adding a suite

1. Write a generator (a new file under `tests/language/gen/`, or new functions in `gen-builtins.mjs`). Use
   `defineSuite(name, cases)` from `tooling/oracle/builtin-corpus.mjs`. Each case has `id`, `target`, `make()` (returns
   fresh `{this?, args?}` on every call) and optional `static`, `construct`, `get`, `approx`, `cbs`.
2. Targets come from the standard-library manifest. If a target you need is missing or X, change the manifest source
   (WP-G0E), not this library. A callback-taking method also needs a `CALLBACK_ARITY` entry in
   `tooling/oracle/builtin-corpus.mjs`.
3. Add the suite to `buildSuites()` in `gen-builtins.mjs` (or the new generator), then run the commands above.
4. `compileSuite` replays every case from its own `pre` and requires the same `post`, otherwise it throws. Getters and
   prototypes other than `Object.prototype` or `null` (for plain objects) are rejected outright. Hidden state that a call
   observes (non-enumerable properties) is rejected too. Frozen, sealed and non-extensible state is carried (v1.1).
   Hidden state that does not change the result is accepted, because the Rust side replays from `pre` alone. Do not
   work around a rejection by editing the expectation.

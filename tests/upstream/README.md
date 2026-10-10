# Upstream reactivity specs as an oracle

`npm run test:upstream` runs Vue 3.5.43's own reactivity unit tests (`packages/reactivity/__tests__`, 16 spec files, 440
`it` cases) against the Rust `vue` module. Each spec is read from a Vue checkout at run time, never copied into this repo.
It is compiled through this repo's compiler and run natively with a small vitest-compatible shim
(`tooling/harness/upstream-vitest.js`). The specs' own `expect` calls are the oracle. The harness is
`tooling/harness/run-upstream.mjs`.

## Running it

```sh
git clone --depth 1 --branch v3.5.43 https://github.com/vuejs/core /path/to/vue-core
VUE_CORE_DIR=/path/to/vue-core npm run test:upstream
```

The checkout is found by `--vue-core <dir>`, else `$VUE_CORE_DIR`, else `../vue-core` next to this checkout. Other options:

- `--filter <text>`: only the specs whose name contains the text (for example `--filter reactive`).
- `--verbose`: also list every compile-error and unsupported outcome with its reason.
- `--status <file>`: use another ledger (default `tests/upstream/status.json`).
- `--write-status`: record the current outcomes in the ledger (see below).

Exit codes: 0 when no ledger pass has regressed, 1 when one has (or the scenario build fails), 2 when there is no
checkout. A full run builds two small crates (the `vue` probe and the scenarios) and runs each spec once, with a 30 s
limit per spec file.

`npm test` also runs `tests/upstream/harness.test.mjs`: unit tests of the shim's matchers and `vi.fn`, the import
rewriting and static marks, the ratchet, the trace interpretation, and a negative control (a deliberately failing
`expect` is reported as `fail`). The tests that read the real specs skip with a reason when there is no checkout.

## Outcomes

- **pass**: the test ran and its `expect` calls held.
- **fail**: the test ran and a check failed, the module threw, or the run panicked. The real failures are listed below.
- **compile-error**: the language profile rejects the test (for example classes, `WeakMap`/`WeakSet`, `Proxy`, `process`,
  `BigInt`, `Object.defineProperty`, a status-X standard-library name). The test is refused by name and reports the
  diagnostic.
- **unsupported**: the test needs something the Rust port does not provide. The reason says which, and each kind is
  decided before the test runs (statically) except the shim's own dev-only checks:
  - `unsupported-import`: a name the native module does not export. The reactivity package's `watch` is one (it is
    synchronous; the public `watch` is the runtime-core one, which flushes `pre`). So are the internal modules
    (`../src/dep`, `../src/effect`) and `@vue/runtime-test`.
  - `unsupported-internal`: the test reads a Vue internal field (`effects`, `scopes`, `cleanups`, `dep`, `deps`,
    `nextDep`, `subs`, `flags`, `_value`, or `runner.effect`).
  - `unsupported-profile`: the test calls an Array method the language profile excludes (`copyWithin`, status X in
    `compiler/profile/stdlib.json`; read from that file, not copied).
  - `unsupported-dev-only`: the test uses the `onTrack` or `onTrigger` debugger hook, which Vue calls only in development
    builds.
  - the shim's dev-only checks: `toHaveBeenWarned` (and its `Last` and `Times` forms) asserts Vue's development console
    warnings, which the production port does not emit.

## Ledger (`tests/upstream/status.json`)

The ledger maps `spec :: test id` to the outcome of the last recorded run. It is a ratchet: a test the ledger marks
`pass` that is not `pass` in a run fails the run. `--write-status` records the current outcomes, but it never writes a
regression in: a regressed test stays `pass` in the ledger until it passes again, so its failure keeps showing. The other
values are informational.

## Current totals

Vue 3.5.43, this repo's compiler and the native `vue` module built from this branch (`npm run test:upstream`):

```
spec                                    total  pass  fail  compile-error  unsupported
collections/Map                            32    31     0              0            1
collections/Set                            34    33     0              0            1
collections/WeakMap                        11     0     0             11            0
collections/WeakSet                         9     0     0              9            0
collections/shallowReadonly                 8     0     0              3            5
computed                                   49    32     0              3           14
effect                                     70    46     0              7           17
effectScope                                22    11     0              0           11
gc                                          4     0     0              4            0
reactive                                   37    31     0              5            1
reactiveArray                              40    35     0              4            1
readonly                                   43    18     0             10           15
ref                                        38    32     0              1            5
shallowReactive                            18    16     0              0            2
shallowReadonly                            14     2     0              3            9
watch                                      11     0     0              0           11
TOTAL                                     440   287     0             60           93
```

The 93 unsupported are: 37 `unsupported-import`, 36 dev-only warning checks (`toHaveBeenWarned` and its forms) plus 2
`onTrack` hooks, 17 `unsupported-internal`, and 1 `unsupported-profile`. The 60 compile-errors are the profile's rules:
weak collections (36), classes (7), Node globals `global` and `process` (6), status-X builtins `Object.setPrototypeOf`,
`Object.defineProperty` and `Symbol.isConcatSpreadable` (8), `Proxy` (2) and `BigInt` (1).

## Real failures

None on this branch.

## Limits of the oracle

- Static marks read each test's callback and the code of its describe and of the top level. A helper defined outside them
  that reads an internal field is not seen statically; it fails at run time instead.
- A describe fixture the compiler rejects refuses the whole statement that holds it. The 16 compile-errors in `readonly`,
  `collections/shallowReadonly` and `shallowReadonly` come from `[Map, WeakMap]` and `[Set, WeakSet]` loops: the WeakMap or
  WeakSet diagnostic refuses the whole loop, so the Map and Set cases of those tests do not run either.
- A refused describe is named from its source text, so `readonly > Collection.name > ...` appears under that name in the
  ledger; a run names it after the collection (`Map`, `WeakMap`, ...).
- The shim implements the matchers and `vi.fn` these specs use; the specs use no other `vi` API. A missing API surfaces as
  a `fail` with a `TypeError`, so a new spec that needs one shows up as a failure, not as unsupported.
- A panic is attributed to the test that was running, and the rest of the file is reported as not run. A spec file that
  takes longer than 30 s loses all its results and reports one file-level failure.
- The oracle compiles as `compileModule` does, with the standard-library manifest check on. The language suite
  (`tooling/harness/run-language.mjs`) deliberately compiles below that policy, so the two suites are not identical.
- The kernel's built-in symbol set has five symbols (`crates/value/src/rt.rs`). The profile excludes the others
  (`Symbol.isConcatSpreadable` is VN4017), so this corpus does not exercise the difference.

## CI

The oracle is not wired into CI. A job could clone v3.5.43 with the command above, set `VUE_CORE_DIR`, and run
`npm run test:upstream`. The ratchet makes that job fail on a regression. No real failures remain, so the job can block from
the start.

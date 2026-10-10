# Dynamic import: evidence (WP-C1c.5.5)

Status: a literal `import()` (a string, or a template without substitutions) compiles to `Rt::dynamic_import`, joins
the project graph as a load, and matches the Node oracle in the four projects `dyn-basic`, `dyn-throw`, `dyn-cycle` and
`dyn-order`. A computed specifier, import attributes or options, and unresolvable or native targets are rejected at
compile time with a code and a span. Not done: the optional chunk graph (web chunk loading is out of scope).

## What changed

- Profile: rule `dynamic-import-options` (VN2031) in `compiler/analysis/profile-rules.mjs`, with its code in
  `compiler/diagnostics/codes.mjs`. The existing `dynamic-import` rule (VN2019) rejects computed specifiers.
- HIR: `dimport(request)` in `compiler/hir/expr.mjs`; the request is the target id from the module resolver.
- Emit: `rt.dynamic_import("<id>")` in `compiler/emit/expr.mjs`.
- Project graph: `compiler/project/index.mjs` adds each literal target to the graph as a load, not as a request. The
  target policy is the static one (see the table below).
- Runtime: `crates/value/src/module.rs` (`dynamic_import`, `dynamic_turn`, `import_turns`, `load_turns`) and
  `crates/value/src/rt.rs` (`enqueue_host_turn`; `run_jobs` runs host turns when the job queue is empty).
  `crates/value/src/roots.rs` marks the host queue.
- Manifest: the `import()` note in `compiler/profile/expansions.json` (status P is unchanged); `manifest.json` regenerated.
- Harness fix outside the nominal scope: `tooling/harness/run-projects.mjs` compiled the projects with the native target,
  which rejects the test-only global `trace` (VN4016). All 10 existing projects failed that way before the fix. They are
  compiled with target `test` now.

## Timing model and the oracle

The project oracle runs Node with `--import tooling/harness/natives-register.mjs`, which registers loader hooks. Plain
Node (no hooks) and the oracle order uncached imports differently, so the runtime follows the oracle, and the plain-Node
numbers are kept here as the alternative. Probe programs (scratch, not in the repository): a main module that starts
imports and a chain of N `then` steps, then prints the log. Results (Node v22.22.0):

| Probe | Plain Node | Project oracle (hooks) |
| leaf import, chain of 24 | `leaf-eval` after `then7` | `leaf-eval` after `then24` |
| loaded import, chain of 24 | settles after `then8` | does not settle before the chain ends (nor within six event-loop turns) |
| leaf, then deps (dep-a) | leaf-eval, dep-a-eval, deps-eval | leaf-eval, dep-a-eval, deps-eval |
| deps, then leaf | leaf-eval, dep-a-eval, deps-eval | leaf-eval, dep-a-eval, deps-eval |
| leaf, then loaded | leaf-eval, settled-V, settled-L | settled-V, leaf-eval, settled-L |
| loaded, then leaf | leaf-eval, settled-V, settled-L | settled-V, leaf-eval, settled-L |
| lwd (requests loaded.mjs), then leaf | leaf-eval, settled-L, lwd-eval, settled-W | leaf-eval, settled-L, lwd-eval, settled-W |
| leaf, then lwd | leaf-eval, settled-L, lwd-eval, settled-W | leaf-eval, settled-L, lwd-eval, settled-W |

Under the oracle every import is a loader round trip, and a round trip runs after the promise jobs queued before it. The
round trips of one import are counted as follows (`import_turns`, Rust):

| Import (not loaded unless stated) | Turns | Why |
| loaded module | 1 | one resolve |
| leaf (no requests) | 2 | resolve, load |
| deps (requests dep-a, not loaded) | 4 | resolve, load, then per request: resolve, load of dep-a |
| lwd (requests loaded.mjs, which is loaded) | 3 | resolve, load, resolve of the request |

Imports started together advance one turn each, in FIFO order, so the orders in the table follow from the counts. The
evaluation of an import happens on its last turn, and its settlement follows in the same drain.

## Compile-time rejects

| Form | Code | Span | Fixture or test |
| `import(name)`, computed | VN2019 | the argument | `tests/language/rejects/dynamic-import.js` (`2:17-2:21`) |
| template with a substitution | VN2019 | the argument | `tests/compiler/project/dynamic.test.mjs` |
| `import('./x.js', { with: ... })` | VN2031 | the options argument | `tests/language/rejects/dynamic-import-options.js` (`2:27-2:53`) |
| `import('./nope.js')`, missing | VN9001 "not in the project" (as for a static import) | the specifier | `dynamic.test.mjs` |
| `import('./dir')`, not a project file | VN9001 | the specifier | `dynamic.test.mjs` |
| `import('lodash')`, package | VN9001 "outside the closed world" | the specifier | `dynamic.test.mjs` |
| `import('/abs/x.js')`, absolute | VN9001 | the specifier | `dynamic.test.mjs` |
| `import('@vue-native/platform')`, native | VN9001 "native module" | the specifier | `dynamic.test.mjs` |
| `import('vue')` | VN6202 | the specifier | `dynamic.test.mjs` |
| `import('vue/server-renderer')` | VN6203 | the specifier | `dynamic.test.mjs` |
| `import('node:fs')` | VN6204 | the specifier | `dynamic.test.mjs` |
| `import()`, `import(...a)` | VN1000 | parser | `dynamic.test.mjs` |

Graph errors use VN9001 because the static path already does (the registry has no user-facing code for them). Each
rejected form has a literal counterpart in the same graph that compiles (`dynamic.test.mjs`, first test), and a
no-substitution template compiles.

## Commands and results

Each command ran in `/home/claude/wt/w55-dynimport` with `CARGO_TARGET_DIR=/home/claude/targets/w55-dynimport`.

| Command | Before the change | After the change |
| `node tooling/harness/run-projects.mjs` | 10 projects: compile-error VN4016 (all 10) | 14 projects: 14 pass |
| `node tooling/harness/run-projects.mjs --filter dyn- --write-expected` | (new projects) | first write: 3 pass, 1 FAIL (`dyn-order`, under the job-step model); the files are the Node output. A second write: `dyn-basic`, `dyn-order` and `dyn-throw` unchanged, `dyn-cycle` differed (Limits). After the `dyn-cycle` change: `dyn-cycle` written and passing |
| `node tooling/harness/run-projects.mjs --filter dyn-` | (new projects) | 4 pass (after the turn model) |
| `node tooling/harness/run-language.mjs` | 79 pass | 79 pass |
| `node tooling/harness/run-language.mjs --dir tests/language/lowering` | 166 pass | 166 pass |
| `node --test "tests/compiler/**/*.test.mjs"` | 452 pass, 33 skipped, 0 fail | 468 pass, 33 skipped, 0 fail |
| `cargo test -p vn-value` | 705 passed | 715 passed (10 in `tests/modules_dynamic.rs`) |
| `node --test "tests/profile/*.test.mjs"` | 95 pass | 95 pass |
| `node tooling/manifest/build-manifest.mjs --check` | (not run) | up to date |

The compiler count rose by 16 passing tests: 14 in `tests/compiler/project/dynamic.test.mjs`; one more reject case from
`tests/compiler/frontend/profile-rejects.test.mjs`, which makes one test per fixture in `tests/language/rejects/`; and one
more in `tests/compiler/lower/lower.test.mjs`, where the VN9001 test became two (VN2019 at the argument, and a literal
`import()` lowers to `rt.dynamic_import` and is not a static request).

## Negative controls

1. Harness. The expected Node trace of `dyn-order` had entries 36 and 37 swapped (`leaf-eval`, `leaf`). The harness
   reported `FAIL  dyn-order -- first difference at $.entries[36].label: "leaf-eval" vs "leaf"`. The file was then
   restored byte for byte (`cmp`) and the project passed again.
2. Runtime turn model. Changing the request cost in `load_turns` (dropping the resolve turn per request) makes
   `a_request_of_a_module_costs_a_turn_even_when_the_requested_module_is_loaded` fail. The file was restored byte for
   byte. Adding one turn to every import changes no order, so no test fails: imports that start together are compared only
   with each other. The tests above check differences between imports, not absolute counts.
3. Runtime, plain-Node order. `negative_control_the_plain_node_job_order_is_not_reproduced` asserts that the leaf
   evaluates after the 24th step, not the 7th (plain Node).
4. Compile time. Each reject has a literal form in the same graph that compiles; the reject fixtures check the exact spans
   through `checkSource`.

## Limits

- Calibrated: the orders above (loaded 1 turn, leaf 2, deps 4, lwd 3), and the trace of `dyn-order`.
- Not calibrated: several requests of one module (counted one after another, not in parallel); cycles among modules that
  are not loaded yet (each counted once; `dyn-cycle` covers only a loaded target); link errors of an unloaded target
  (they settle on the last turn; not measured).
- `quiescence` (the oracle's macrotask boundary) does not wait for host turns.
- Oracle timing. Under the hook oracle, an import of a loaded module that starts during module load (`dyn-cycle`)
  settles either before or after `run()` starts, depending on the event loop. Two Node writes of `dyn-cycle` gave the two
  orders (`settled` before `static getA`, then after it). `run()` now awaits that import before it traces anything, so the
  expected trace does not depend on the race. The runtime settles it during the first drain, as the first write did. Ten
  oracle runs per project, under load, matched the stored expectations (`dyn-basic`, `dyn-cycle`, `dyn-order`, `dyn-throw`).
- Plain Node differs from the runtime (see the table). No language case uses `import()`, so the language oracle does
  not test it.
- Not supported by design: computed specifiers (VN2019), import attributes and options (VN2031), native targets (VN9001).
- Not done: the chunk graph. Its physical splitting stays experimental (design row L17, E).

## Reproduce

```
export CARGO_TARGET_DIR=/home/claude/targets/w55-dynimport
node tooling/harness/run-projects.mjs --filter dyn-
node tooling/harness/run-projects.mjs
node tooling/harness/run-language.mjs
node tooling/harness/run-language.mjs --dir tests/language/lowering
node --test "tests/compiler/**/*.test.mjs"
cargo test -p vn-value
node --test "tests/profile/*.test.mjs"
node tooling/manifest/build-manifest.mjs --check
```

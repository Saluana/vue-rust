# GC latency, heap exhaustion and native-handle disposal (2026-10-10)

Ticket w32-gc, tasks 3.2 and 5.6. Base commit 554b716 on branch claude/w32-gc. Nothing is committed. The changes are new files only, and `git diff` on tracked files is empty.

## Summary

- Part A (churn workload, correctness, pause benchmark): the current non-incremental mark-sweep does not meet the design's proposed collection-slice budget at any of the three live-set sizes measured. p99 pauses are 6.823 ms at 10k, 16.488 ms at 100k and 206.346 ms at 1M live objects. The budget is 2 ms.
- Part B (gc-arena 0.7.0, experiment only): gc-arena's full-cycle pauses are lower than vn-value's at every size, but the two object models differ, so this is not a like-for-like collector comparison. gc-arena's incremental slices meet 2 ms at 10k. At 100k the default pacing is borderline (2.555 ms in run 2, 1.719 ms in run 1) and the paced setting meets it. At 1M no setting met it. Recommendation: do not adopt gc-arena for vn-value.
- Part C-1 (heap limit): exceeding the heap limit during churn throws a catchable RangeError ("heap limit exceeded") from generated-style calls. No Rust panic occurs, and the runtime works again once the refused garbage is collected.
- Part C-2 (native-handle disposal): the existing EffectScope stop and DocHost::release already close native handles, so no new mechanism was added. Nine tests cover the behaviour. Five Node cases run against @vue/reactivity 3.5.43 agree with them. Mutations of the scope code are detected by the tests.
- Miri and ASan: not applicable. vn-value and vn-runtime forbid unsafe code.

## What was built

| Path | Contents |
|---|---|
| crates/value/tests/support/churn.rs | Churn workload and its model. Shared by the tests and the benchmark through `#[path]`. |
| crates/value/tests/gc_churn.rs | 7 correctness tests, 2 of them negative controls. |
| crates/value/tests/gc_exhaustion.rs | 3 heap-limit tests (Part C-1). |
| crates/value/examples/gc_churn_bench.rs | Pause benchmark (Part A). |
| experiments/heap/gc-arena-churn/ | Standalone gc-arena 0.7.0 experiment (Part B). Cargo.toml has its own `[workspace]` table and `gc-arena = "=0.7.0"`. Also Cargo.lock, src/main.rs (harness and 4 tests) and README.md (results, API comparison, recommendation). |
| crates/runtime/tests/realm_scope_handles.rs | 9 tests for native-handle disposal (Part C-2). |
| docs/evidence/gc-latency.md | This note. |

Layout checks: no Cargo.toml outside the experiment mentions gc-arena, so vn-value does not use it. `cargo metadata --no-deps` at the root lists nine members: vn-css, vn-dom, vn-reactivity, vn-renderer-dom, vn-runtime, vn-testkit, vn-ui, vn-uikit and vn-value. gc-arena-churn is not one of them, and the root Cargo.lock has no gc-arena entry.

## Commands and results

Cargo ran with CARGO_TARGET_DIR=/home/claude/targets/w32-gc. The experiment used CARGO_TARGET_DIR=/home/claude/targets/w32-gc-arena, as the ticket specifies. Logs are in logs/ under those two directories.

| Command | Result | Log |
|---|---|---|
| cargo test -p vn-value | 715 passed, 0 failed, 0 warnings. gc_churn: 7 passed. gc_exhaustion: 3 passed. | logs/value-test-final2.log |
| cargo test -p vn-reactivity -p vn-runtime | 361 passed, 0 failed, 0 warnings. realm_scope_handles: 9 passed. | logs/reactivity-runtime-test-final2.log |
| cargo test -p vn-runtime --test realm_scope_handles | 9 passed, 0 failed, after the probe-count assertions were added | logs/realm-scope-handles-test.log |
| cargo run --release -p vn-value --example gc_churn_bench | Exit 0, real 0m55.171s. Output in Part A. | logs/bench-run2.log |
| cargo test (in experiments/heap/gc-arena-churn) | 4 passed, 0 failed | /home/claude/targets/w32-gc-arena/logs/gc-arena-test.log |
| cargo run --release (in experiments/heap/gc-arena-churn) | Exit 0, real 0m21.251s. Output in Part B. | /home/claude/targets/w32-gc-arena/logs/gc-arena-run2.log |
| cargo metadata --no-deps (repository root) | Nine vn-* members. gc-arena-churn not listed. | command output |
| node scope_oracle.cjs (Node v22.22.0, @vue/reactivity 3.5.43) | Five cases. Output in Appendix B. | logs/scope-oracle.json |
| Mutation checks | Part A: 4 passed, 3 failed (exit 101). C-1: 0 passed, 3 failed (exit 101). C-2 M1: 4 of 9 fail. C-2 M2: 1 of 9 fails. Each mutated file was restored byte-identical to HEAD. | logs/mutation-cell-trace.log, mutation-heap-limit.log, mutation-scope-exactly-once.log, mutation-scope-gc-trace.log |

The baseline, logs/baseline-value-test.log, is the cargo test -p vn-value run before the new test files were added. It reports 705 passed. Comparing the test-binary lists of that log and of value-test-final2.log shows that tests/gc_churn.rs and tests/gc_exhaustion.rs are the only new binaries. The earlier runs logs/value-test-final.log (715 passed) and logs/reactivity-runtime-test-final.log (361 passed) were made before the mutation checks and the probe-count assertions. The final2 logs are the ones cited here.

## Part A: churn workload, correctness and pauses

The workload (crates/value/tests/support/churn.rs) uses only the public vn-value API: objects, arrays, Map and Set, closures over cells, native functions, promises with their reaction jobs, and the job queue. Every object it creates is recorded in a model with the objects it references. Reachability is computed by the model from the roots the workload holds, never by the collector. After each collection the test compares the runtime with the model. The same objects must be live, the live count must be the realm baseline plus the reachable model objects, and the stable set and the pinned list must keep their contents.

A round builds a list, rings, a wide object, a Map and a Set of objects, closures over cells, native functions, promise chains with their reactions, and one queued job. The benchmark's shape is list 2000, rings 100 of 10 objects, wide 500, pairs 500, closures 1000, natives 1000 and promises 300. A round creates 11,161 objects. The head of the list stays pinned across the round boundary. Collections run at safepoints between operations. Threshold collections use the existing set_gc_threshold and maybe_collect mechanism, and each round also makes three explicit collections.

### Correctness tests (crates/value/tests/gc_churn.rs, 7 tests, all passing)

| Test | What it checks |
|---|---|
| rounds_of_churn_agree_with_the_model_and_reach_a_steady_heap | Eight rounds over a 500-object stable set with threshold 256. After each round the stable set is intact and threshold collections have run. The live-object count and byte estimate equal their values after round 0, so the heap does not grow. At the end, the collection count is three explicit collections per round plus the threshold ones, and the live count equals the realm baseline plus the model's live nodes. |
| a_pinned_list_keeps_its_contents_through_churn_and_is_freed_once_unpinned | A 300-node list stays pinned across three rounds and keeps its contents. After it is unpinned, its head is freed and the model has no live nodes. |
| queued_jobs_keep_their_arguments_until_the_queue_drains | A queued job's argument is a root until the job runs. After the drain it is collected. |
| each_reaction_runs_once_and_its_result_is_collected_with_the_promises | Two reactions on one settled promise queue two jobs, and each runs once. Their results are correct. After settling, nothing is reachable in the model and no node is live. |
| collecting_at_every_safepoint_agrees_with_the_model | Threshold 1, three rounds over a 30-object stable set. Each round has eight collections, and each is checked against the model. |
| negative_control_a_root_dropped_behind_the_models_back_is_detected | The stable set's root is removed without telling the model. The check must report "reachable from a root but the collector freed it". |
| negative_control_an_untracked_root_is_reported_by_the_live_count | An object the model does not know is rooted. The live count must disagree with the model, and the check must report "model expects". |

Negative controls: the two negative_control tests above, and the mutation check below.

Mutation check. In crates/value/src/object.rs line 233, the trace of a cell's value (`ObjectKind::Cell(v) => t.mark_value(v),`) was replaced by `ObjectKind::Cell(_v) => {}`. Then `cargo test -p vn-value --test gc_churn` reported 4 passed and 3 failed (exit 101). The failing tests were collecting_at_every_safepoint_agrees_with_the_model, rounds_of_churn_agree_with_the_model_and_reach_a_steady_heap and a_pinned_list_keeps_its_contents_through_churn_and_is_freed_once_unpinned. Their messages include "round 0: object 113 is reachable from a root but the collector freed it". object.rs was restored from a copy and is byte-identical to HEAD.

### Pause benchmark

Command: `cargo run --release -p vn-value --example gc_churn_bench`. Output (logs/bench-run2.log, exit 0):

```
gc_churn_bench: vn-value mark-sweep, non-incremental, run once per size
machine: Intel(R) Xeon(R) Processor @ 2.80GHz, 4 logical CPUs
workload per round: Shape { list: 2000, rings: 100, ring_size: 10, width: 500, pairs: 500, closures: 1000, natives: 1000, promises: 300 }

== live set 10000 stable objects, 40 rounds, threshold 2000 allocations ==
objects created by the workload: 446440; realm baseline: 248 live objects
collections: 320 (threshold 200, explicit 120); all checked against the model
pause ms: p50 1.378  p95 3.949  p99 6.823  max 7.058  mean 1.789
objects freed per collection: min 0  p50 1003  p95 4701  max 4701  mean 1614; 1 of 320 freed nothing
heap after the last collection: live_objects 12252, bytes_estimate 2604480, total freed 516447
time inside collections: 572.6 ms of 1.47 s wall (wall includes workload and model checks)
p99 6.823 ms does not meet the 2 ms collection-slice budget (design.md section 14)

== live set 100000 stable objects, 40 rounds, threshold 2000 allocations ==
objects created by the workload: 446440; realm baseline: 248 live objects
collections: 320 (threshold 200, explicit 120); all checked against the model
pause ms: p50 10.941  p95 13.525  p99 16.488  max 19.745  mean 11.156
objects freed per collection: min 0  p50 1003  p95 4701  max 4701  mean 1614; 1 of 320 freed nothing
heap after the last collection: live_objects 102252, bytes_estimate 22044480, total freed 516447
time inside collections: 3569.8 ms of 6.49 s wall (wall includes workload and model checks)
p99 16.488 ms does not meet the 2 ms collection-slice budget (design.md section 14)

== live set 1000000 stable objects, 30 rounds, threshold 2000 allocations ==
objects created by the workload: 334830; realm baseline: 248 live objects
collections: 240 (threshold 150, explicit 90); all checked against the model
pause ms: p50 104.420  p95 149.054  p99 206.346  max 231.675  mean 109.905
objects freed per collection: min 0  p50 1003  p95 4701  max 4701  mean 1612; 1 of 240 freed nothing
heap after the last collection: live_objects 1002252, bytes_estimate 216444480, total freed 386837
time inside collections: 26377.2 ms of 43.59 s wall (wall includes workload and model checks)
p99 206.346 ms does not meet the 2 ms collection-slice budget (design.md section 14)

real	0m55.171s
user	0m51.373s
sys	0m0.829s
EXIT=0
```

What the numbers mean:

- Machine: Intel Xeon 2.80 GHz, 4 logical CPUs, in a shared Linux container. The numbers come from one run. The benchmark asserts nothing about timings. Only the collector call is timed. The model check after each collection runs after the timer stops.
- Budget: design.md section 14, "Performance plan" (planning/vue-native-framework/design.md, line 674), states "UI-lane collection slice p99 <= 2 ms under reference workload" and calls the pause cap a go/no-go hypothesis.
- Verdict: the current non-incremental mark-sweep does not meet the budget at 10k, 100k or 1M live objects. Its p99 pauses are 6.823, 16.488 and 206.346 ms.
- The design does not define the reference workload's live-set size, so the three sizes are a choice made here.
- An earlier bench run (logs/bench-run1.log) used an older round function and is superseded. Its numbers are not used.

## Part B: gc-arena comparison (experiments/heap/gc-arena-churn)

The experiment is its own workspace, depends on gc-arena = "=0.7.0" from crates.io, and is not a member of the root workspace. It runs the same shape (live sets of 10k with 40 rounds, 100k with 40 rounds and 1M with 30 rounds, a threshold of 2000 allocations, 11,161 objects per round) in three modes:

- cycle: a full finish_cycle at each collection point. This is the comparable mode.
- slice, default pacing: collect_debt whenever allocation debt is due.
- slice, paced: the same, with sleep_factor 0.05. This is a tuning run, not a recommendation.

Cycle mode checks the live count after every round. Each run then checks the stable set, runs a full cycle, and reports the live count. The object model is simpler than vn-value's: one Node struct with a numeric id, numeric properties and a Vec of references. There are no property maps, realm objects or runtime objects, and promises are plain nodes that a Rust loop reacts to. The comparison is therefore of whole implementations, not of the collectors alone. The README lists the differences.

p99 pause in ms. One run per column, except the gc-arena cycle column, where run 1 is in brackets.

| Live set | vn-value (non-incremental) | gc-arena cycle | gc-arena slice, default pacing | gc-arena slice, paced (sleep_factor 0.05) |
|---|---|---|---|---|
| 10k | 6.823 | 0.621 (run 1: 1.321) | 0.601 | 0.516 |
| 100k | 16.488 | 10.589 (run 1: 15.645) | 2.555 (run 1: 1.719) | 1.583 |
| 1M | 206.346 | 101.205 (run 1: 99.957) | no steady-state slice ran; the only slice was 97.097 ms | 20.996 |

Run 1 came from an earlier build of the harness. Between the two cycle runs the 10k p99 moved by 2.1x, the 100k p99 by 1.5x, and the 1M p99 by about 1 percent.

Budget verdicts against 2 ms p99:

- vn-value: misses at all three sizes.
- gc-arena cycle: meets at 10k in both runs, misses at 100k and 1M.
- gc-arena slices, default pacing: meets at 10k. At 100k it is borderline, at 2.555 ms in run 2 and 1.719 ms in run 1. At 1M no steady-state slice ever ran. The only slice was a 97.097 ms catch-up slice.
- gc-arena slices, paced: meets at 10k and 100k, misses at 1M (20.996 ms).

Heap size matters as much as the pauses. Before the final full cycle, the slice runs still held these objects: at 10k, 38,082 (default) and 37,326 (paced) against 12,004 live; at 100k, 283,580 and 170,231 against 102,004 live; at 1M, 1,334,834 against 1,002,004 live. In the 1M paced run the slices freed no garbage at all.

Output (logs/gc-arena-run2.log, exit 0), from `cargo run --release` in experiments/heap/gc-arena-churn:

```
    Finished `release` profile [optimized] target(s) in 0.01s
     Running `/home/claude/targets/w32-gc-arena/release/gc-arena-churn`
gc_arena_churn: gc-arena 0.7.0 (pinned =0.7.0), the workload of gc_churn_bench, two collection modes
machine: Intel(R) Xeon(R) Processor @ 2.80GHz, 4 logical CPUs
workload per round: Shape { list: 2000, rings: 100, ring_size: 10, width: 500, pairs: 500, closures: 1000, natives: 1000, promises: 300 }
allocation threshold: 2000 allocations (cycle mode)
slice modes: default pacing (sleep_factor 0.5) and a paced tuning run (sleep_factor 0.05)

== cycle mode: live set 10000 stable objects, 40 rounds, threshold 2000 allocations ==
objects created by the workload: 446440; stable set: 10004 objects
collections: 280 (threshold 160, explicit 120); each round's live count checked against the expected count
pause ms: p50 0.409  p95 0.516  p99 0.621  max 0.770  mean 0.410
objects freed per collection: min 0  p50 1502  p95 3652  max 3652  mean 1587; 1 of 280 freed nothing
heap after the last collection: live_objects 12004
time inside collections: 114.8 ms of 0.15 s wall (wall includes the workload)
p99 0.621 ms meets the 2 ms collection-slice budget (design.md section 14)

== slice, default pacing (sleep_factor 0.5): live set 10000 stable objects, 40 rounds, collect_debt when due ==
objects created by the workload: 446440; stable set: 10004 objects
slices: 282 (of 440 collection points)
first slice: 0.523 ms (the first collection point pays the debt of the stable set's allocations)
slice pause ms: p50 0.147  p95 0.482  p99 0.601  max 0.661  mean 0.172
time inside slices: 48.6 ms of 0.09 s wall (wall includes the workload)
slice p99 0.601 ms meets the 2 ms collection-slice budget (design.md section 14)
live objects before the final cycle: 38082, of which 26078 were garbage the slices had not freed
final full cycle (two finish_cycle calls, not in the slice statistics): 2.797 ms; live_objects 12004

== paced slice (sleep_factor 0.05): live set 10000 stable objects, 40 rounds, collect_debt when due ==
objects created by the workload: 446440; stable set: 10004 objects
slices: 356 (of 440 collection points)
first slice: 0.701 ms (the first collection point pays the debt of the stable set's allocations)
slice pause ms: p50 0.121  p95 0.396  p99 0.516  max 0.701  mean 0.139
time inside slices: 49.5 ms of 0.09 s wall (wall includes the workload)
slice p99 0.516 ms meets the 2 ms collection-slice budget (design.md section 14)
live objects before the final cycle: 37326, of which 25322 were garbage the slices had not freed
final full cycle (two finish_cycle calls, not in the slice statistics): 1.765 ms; live_objects 12004

== cycle mode: live set 100000 stable objects, 40 rounds, threshold 2000 allocations ==
objects created by the workload: 446440; stable set: 100004 objects
collections: 280 (threshold 160, explicit 120); each round's live count checked against the expected count
pause ms: p50 6.508  p95 9.755  p99 10.589  max 14.457  mean 6.764
objects freed per collection: min 0  p50 1502  p95 3652  max 3652  mean 1587; 1 of 280 freed nothing
heap after the last collection: live_objects 102004
time inside collections: 1893.9 ms of 1.98 s wall (wall includes the workload)
p99 10.589 ms does not meet the 2 ms collection-slice budget (design.md section 14)

== slice, default pacing (sleep_factor 0.5): live set 100000 stable objects, 40 rounds, collect_debt when due ==
objects created by the workload: 446440; stable set: 100004 objects
slices: 208 (of 440 collection points)
first slice: 10.613 ms (the first collection point pays the debt of the stable set's allocations)
slice pause ms: p50 0.139  p95 0.785  p99 2.555  max 10.613  mean 0.262
time inside slices: 54.4 ms of 0.11 s wall (wall includes the workload)
slice p99 2.555 ms does not meet the 2 ms collection-slice budget (design.md section 14)
live objects before the final cycle: 283580, of which 181576 were garbage the slices had not freed
final full cycle (two finish_cycle calls, not in the slice statistics): 19.980 ms; live_objects 102004

== paced slice (sleep_factor 0.05): live set 100000 stable objects, 40 rounds, collect_debt when due ==
objects created by the workload: 446440; stable set: 100004 objects
slices: 325 (of 440 collection points)
first slice: 8.025 ms (the first collection point pays the debt of the stable set's allocations)
slice pause ms: p50 0.112  p95 0.463  p99 1.583  max 8.025  mean 0.183
time inside slices: 59.5 ms of 0.09 s wall (wall includes the workload)
slice p99 1.583 ms meets the 2 ms collection-slice budget (design.md section 14)
live objects before the final cycle: 170231, of which 68227 were garbage the slices had not freed
final full cycle (two finish_cycle calls, not in the slice statistics): 15.430 ms; live_objects 102004

== cycle mode: live set 1000000 stable objects, 30 rounds, threshold 2000 allocations ==
objects created by the workload: 334830; stable set: 1000004 objects
collections: 210 (threshold 120, explicit 90); each round's live count checked against the expected count
pause ms: p50 79.510  p95 94.839  p99 101.205  max 103.146  mean 81.675
objects freed per collection: min 0  p50 1502  p95 3652  max 3652  mean 1585; 1 of 210 freed nothing
heap after the last collection: live_objects 1002004
time inside collections: 17151.8 ms of 17.22 s wall (wall includes the workload)
p99 101.205 ms does not meet the 2 ms collection-slice budget (design.md section 14)

== slice, default pacing (sleep_factor 0.5): live set 1000000 stable objects, 30 rounds, collect_debt when due ==
objects created by the workload: 334830; stable set: 1000004 objects
slices: 1 (of 330 collection points)
first slice: 97.097 ms (the first collection point pays the debt of the stable set's allocations)
slice pause ms: p50 97.097  p95 97.097  p99 97.097  max 97.097  mean 97.097
time inside slices: 97.1 ms of 0.14 s wall (wall includes the workload)
slice p99 97.097 ms does not meet the 2 ms collection-slice budget (design.md section 14)
live objects before the final cycle: 1334834, of which 332830 were garbage the slices had not freed
final full cycle (two finish_cycle calls, not in the slice statistics): 174.716 ms; live_objects 1002004

== paced slice (sleep_factor 0.05): live set 1000000 stable objects, 30 rounds, collect_debt when due ==
objects created by the workload: 334830; stable set: 1000004 objects
slices: 150 (of 330 collection points)
first slice: 96.292 ms (the first collection point pays the debt of the stable set's allocations)
slice pause ms: p50 0.076  p95 0.253  p99 20.996  max 96.292  mean 0.970
time inside slices: 145.5 ms of 0.16 s wall (wall includes the workload)
slice p99 20.996 ms does not meet the 2 ms collection-slice budget (design.md section 14)
live objects before the final cycle: 1334834, of which 332830 were garbage the slices had not freed
final full cycle (two finish_cycle calls, not in the slice statistics): 132.392 ms; live_objects 1002004

real	0m21.251s
user	0m20.728s
sys	0m0.282s
EXIT=0
```

Why the slices behave this way. gc-arena starts a cycle only after the allocations since the last cycle exceed sleep_factor times the remembered object count, with a floor of min_sleep (metrics.rs, finish_cycle). Allocation debt is the cycle's debits minus its credits. Credits are per object: mark 0.1, trace 0.4 and keep 0.05 for each live object, and drop 0.2 or free 0.3 for each garbage object (metrics.rs). At 1M live objects the default pacing starts a cycle only after about 500k allocations. The workload allocated 334,830 objects in total, so no steady-state cycle started. The collector takes one gray object at a time and runs its trace to completion (context.rs). A container with a million children is therefore one step that nothing splits. This is the likely cause of the 21 ms paced slice. I did not isolate it with a separate measurement.

API and rooting differences, measured against ARCHITECTURE.md section 2:

- Safepoints. ARCHITECTURE.md collects only between jobs, where no compiled code is on the Rust stack. gc-arena encodes the same rule in types: a Gc exists only inside a mutate closure, and a collection needs &mut Arena. The harness had to build, attach and check each structure inside one mutate call, with the collection outside it.
- Rooting. vn-value roots are explicit: Rt::root and unroot, RootProvider, job queues and timers. ObjRef is Copy and has no lifetime, so host state such as DocHost, timers and listeners can hold one directly. gc-arena keeps persistent references in the root type or in DynamicRoot values, fetched back into a Gc only inside mutate. That is a real cost for host code, which holds references outside any arena call.
- Stale references. vn-value detects a stale ObjRef at runtime with a generation counter. gc-arena makes stale references impossible by construction, at the cost of a 'gc brand on every type that holds a Gc and a Collect derive on each.
- Finalization. A Drop impl on a type that holds Gc pointers needs #[collect(unsafe_drop)], an unchecked promise that drop never touches a Gc pointer. A 'static type without Gc fields can use require_static and keep its Drop impl. Release at sweep time is a last resort in both designs.
- Incremental collection. gc-arena supports it, but every mutation of an allocated object needs a write barrier (borrow_mut(mc), Gc::write). vn-value has no barriers, because it collects only at safepoints.
- Observability. Metrics::total_gc_count matches vn-value's live_objects. gc-arena has no byte total and no limit hook, so the byte-estimate heap limit (ABI.md section 12) would need a layer on top.

Recommendation: do not adopt gc-arena for vn-value. Keep ObjRef, the explicit roots and the safepoint collector. The measurements do not show gc-arena meeting the 2 ms budget where vn-value misses it. At 1M its slices depend on pacing tuned to the live-set size, and single large containers produce slices of 20 ms or more. An incremental vn-value collector would likely meet the same problems, so they point at the design rather than the library. If the 2 ms cap becomes a go/no-go, the next experiment is incremental marking inside vn-value's heap, measured with this harness. It should test whether chunking large arrays and Maps bounds the work per slice. The reference workload's live-set size has to be chosen first.

Negative controls (Part B): a_leaked_root_is_reported_by_the_live_count keeps an object alive that the model does not expect, and the round must fail with a live-object mismatch. a_dropped_stable_root_is_reported_by_the_live_count drops the stable root, and the round must fail the same way. Both pass. cycle_rounds_agree_with_the_expected_live_count and slice_modes_reach_the_expected_count_after_a_full_cycle are the positive tests. `cargo test` in the experiment: 4 passed.

## Part C-1: heap exhaustion during churn (crates/value/tests/gc_exhaustion.rs, 3 tests, all passing)

The tests use hand-written code in the shape that generated code takes under ABI.md section 0: rt.tick() on function entry and on each loop back-edge, with calls through rt.call. No JavaScript is compiled or run. block_code stores 64 objects per call into a holder array. churn_code calls it N times. try_code is `try { return f() } catch (e) { return e }`.

| Test | What it checks |
|---|---|
| exceeding_the_heap_limit_during_churn_is_a_catchable_range_error | The limit is the byte estimate before the churn plus 64 KiB. The churn throws a RangeError with the message "heap limit exceeded", and the try wrapper catches it. Exactly 391 objects are stored before the refusal, which is (limit - before) / 168 + 1. 168 bytes per stored object is 96 for the object, 48 for its one property and 24 for its array slot (ABI.md section 12). After the holder is dropped and collected, the model agrees with the runtime and the estimate is under the limit. Ticks succeed again, a second churn of two blocks (128 objects) completes, the stable set is intact, and a full churn round verifies with the limit cleared. |
| garbage_stays_counted_against_the_limit_until_it_is_collected | Dropping the root does not lower the estimate, so ticks keep failing until a collection runs. After the collection they succeed. |
| a_zero_limit_refuses_the_first_tick_and_stores_nothing | With a limit of 0, the first tick is refused and no object is stored. |

Mutation check. Rt::tick in crates/value/src/rt.rs was changed so that the heap-limit check never fires (`if false && st.heap.borrow().bytes > limit`). The three tests then failed (0 passed, 3 failed, exit 101). None of them saw a RangeError. The try wrapper returned undefined, and the first property read on that value failed in the test helper (gc_exhaustion.rs line 79). rt.rs was restored byte-identical to HEAD.

## Part C-2: native-handle disposal (crates/runtime/tests/realm_scope_handles.rs, 9 tests, all passing)

The mechanism already exists, and this work did not change it:

- A handle is a DOM node owned by a DocHost (vn-runtime, host_doc.rs). DocHost::release frees the node's subtree and calls the host's release hooks. It ignores a node that was already released.
- A close callback is a native function whose data holds a native object with the host and the node. Calling it releases the node.
- The callback is registered with onScopeDispose inside scope.run (vn-reactivity, scope.rs). scope_stop (lines 176 to 220) marks the scope inactive, stops its effects, runs its own cleanups in registration order and then stops its child scopes. A stopped scope is inert: run returns without calling its function, and a second stop does nothing.

These are the semantics of @vue/reactivity 3.5.43's EffectScope. A second registry of close callbacks would duplicate the scope's own cleanup list, so none was added. The tests show the behaviour through the runtime.

| Test | What it proves |
|---|---|
| handles_registered_under_a_scope_close_once_when_it_stops | Three handles close once each, in registration order, at the first stop. A second stop closes nothing and runs no close callback. A callback registered after the stop never runs. |
| an_open_handle_survives_collections_and_closes_when_its_scope_stops | With the scope pinned, three collections leave the handle open and its close callback alive. The stop then closes it once. |
| collecting_an_unstopped_scope_does_not_close_its_handles | Characterization. An unreferenced, unstopped scope is collected, and its handle stays open because no cleanup runs. |
| a_collection_is_allowed_outside_a_cleanup | Negative control for the probe used below. try_collect succeeds when no JS code is running, so the refusal inside a close callback is real. |
| closing_runs_js_with_the_heap_open_and_no_collection_inside | Inside a close callback the heap is open (an allocation succeeds), and a collection is refused. |
| a_child_scopes_handles_close_with_the_parent_and_not_again | The parent's handle closes first, then the child's. Stopping the child again closes nothing. |
| a_scope_closes_only_its_own_handles | Stopping one scope closes only its own handle. A callback registered with no active scope never runs, so handle z is never closed. |
| a_throwing_cleanup_aborts_the_stop_and_later_handles_stay_open | Characterization. A throwing cleanup makes stop throw. Handles after it stay open, and a second stop does not reach them. |
| a_cleanup_that_stops_its_own_scope_closes_its_handle_once | A cleanup that stops its own scope closes the handle once, not twice. |

Node comparison. The five cases below were run as a Node script against @vue/reactivity 3.5.43 from the node_modules of the main checkout (/home/claude/vue-rust), under Node v22.22.0. The script is in Appendix A and its output in Appendix B. This comparison is manual: cargo test does not run Node. The Rust tests encode its results.

| Case | Vue 3.5.43 result | Rust test |
|---|---|---|
| 1. Handles a, b, c under one scope | Close in order at the first stop. A second stop closes nothing. A callback registered after the stop never runs. | handles_registered_under_a_scope_close_once_when_it_stops |
| 2. Child scope inside a parent | Parent cleanup, then child cleanup. Stopping the child again closes nothing. | a_child_scopes_handles_close_with_the_parent_and_not_again |
| 3. Throwing cleanup | stop throws "boom". The later cleanup does not run. The scope is inactive. | a_throwing_cleanup_aborts_the_stop_and_later_handles_stay_open |
| 4. Cleanup that stops its own scope | The close runs once. The scope is inactive. | a_cleanup_that_stops_its_own_scope_closes_its_handle_once |
| 5. onScopeDispose with no active scope | No error. Nothing is registered. | a_scope_closes_only_its_own_handles |

Findings (characterized here, not changed):

1. A throwing cleanup aborts the stop, and the handles registered after it stay open. Vue 3.5.43 does the same (case 3).
2. A scope that is collected without being stopped does not close its handles. Vue does not run onScopeDispose on collection either. In @vue/reactivity 3.5.43 cleanups are called only from EffectScope.stop (dist/reactivity.cjs.js, lines 143 to 146), and the package has no FinalizationRegistry or WeakRef.
3. Two runtime types release host resources when their last reference goes away. DetachedNodes::drop releases its DOM subtree (crates/runtime/src/dom_events.rs, line 179), and Timers::drop cancels its timers (crates/runtime/src/timers.rs, line 29). The drop runs when the last reference to the value goes away. For a value held only by a native object, that is when the collector frees the object. These are a last resort and are not exercised by the tests here.
4. A close callback runs with the heap open. It can allocate, and a collection requested inside it is refused (try_collect returns false). closing_runs_js_with_the_heap_open_and_no_collection_inside checks both.

Test bug found and fixed. The first GC test, an_open_handle_survives_collections_and_closes_when_its_scope_stops, failed with "stale ObjRef ... freed by the collector". The scope was held only by a Rust local during rt.collect(), so the collector was right to free it. The fix is in the test: the scope is pinned with rt.root and unrooted at the end. The runtime was not changed.

Mutation checks. Each mutant was applied to crates/reactivity/src/scope.rs. Each of the nine tests then ran as its own process, so that an abort affects only its own test. The file was restored byte-identical to HEAD after each run.

- M1: scope_stop without the active guard (line 179) and without the cleanup clear (line 198), so a second stop runs the cleanups again. Four of nine tests fail. handles_registered_under_a_scope_close_once_when_it_stops fails at the probe-count assertion after the second stop. a_child_scopes_handles_close_with_the_parent_and_not_again fails at the probe-count assertion after the child is stopped again. a_throwing_cleanup_aborts_the_stop_and_later_handles_stay_open fails because the second stop re-runs the throwing cleanup. a_cleanup_that_stops_its_own_scope_closes_its_handle_once recurses until the stack overflows and the process aborts. The other five pass, since they never stop a scope twice. Before the probe-count assertions were added, only three tests detected M1, and the child test did not. DocHost::release ignores an already released node, so the host's release list could not show a second close. The probe count, which records every run of a close callback, is what shows it. The assertions were added to the exactly-once tests, and the suite still passes 9 of 9.
- M2: Scope::trace no longer marks the cleanups (lines 35 to 37), so the collector can free a close callback that a live scope still holds. One of nine tests fails. an_open_handle_survives_collections_and_closes_when_its_scope_stops panics with "stale ObjRef ... the object was freed by the collector". The other eight pass, because only this test runs a cleanup after a collection.

## Miri and ASan

Not applicable, and not run. crates/value/src/lib.rs line 6 has `#![forbid(unsafe_code)]`, and crates/runtime/src/lib.rs line 3 has the same attribute. No nightly toolchain or Miri was installed. The experiment's own code has no unsafe block. Its gc-arena dependency uses unsafe internally, and that crate is outside every vn-* crate.

## Limits

- One benchmark run for vn-value, on a shared 4-vCPU container. gc-arena cycle numbers differed by up to 2.1x between two runs, so read the tables as order of magnitude.
- The object models differ, and so do the threshold counts. vn-value counts every heap allocation, including runtime ones. The experiment counts only its own nodes. Compare the cycle rows with that in mind.
- The 2 ms budget refers to a reference workload that design.md does not define. The three sizes are a choice.
- The churn workload is hand-written against the Rust API in the style of generated code. It is not compiled JavaScript.
- The paced sleep factor (0.05) is one value. No sweep was run.
- Run 1 of the gc-arena cycle mode came from an earlier build of the harness, and its source was not kept.
- No browser tests were added or run. Chrome cannot run in this container, and no acceptance criterion needed one.
- Disposal behaviour is characterized against Vue 3.5.43 only. Whether the runtime should differ from Vue is not decided here.

## Open questions

1. Should a throwing cleanup still run the later cleanups and close their handles? The runtime matches Vue, which does not.
2. Who stops a scope that becomes garbage? Collection does not close its handles. Only an explicit stop, or the last-resort drops above, releases host resources.
3. What is the reference workload for the 2 ms p99 budget? design.md does not give its live-set size, and it does not say whether the budget applies per pause or per slice.
4. Large containers: one trace of a container with a million entries cannot be split, in gc-arena or in a naive incremental vn-value. Chunking large arrays and Maps is the likely fix. It is not measured here.

## Appendix A: Node oracle script

Saved as scope_oracle.cjs and run with `node scope_oracle.cjs`.

```js
// Oracle for the native-handle scope tests (crates/runtime/tests/realm_scope_handles.rs): the real @vue/reactivity
// package from node_modules, not the Rust port.
const { createRequire } = require('module');
const req = createRequire('/home/claude/vue-rust/package.json');
const { effectScope, onScopeDispose } = req('@vue/reactivity');
const version = req('@vue/reactivity/package.json').version;
const out = { version };
const quiet = (f) => { const w = console.warn; console.warn = () => {}; try { return f(); } finally { console.warn = w; } };

// 1. Handles under one scope close once each, in registration order; a second stop closes nothing; a callback
//    registered after the stop never runs.
{
  const closed = []; const s = effectScope(false);
  for (const n of ['a', 'b', 'c']) s.run(() => onScopeDispose(() => closed.push(n)));
  s.stop(); const afterStop = closed.slice(); s.stop();
  let ran = false; quiet(() => s.run(() => { ran = true; onScopeDispose(() => closed.push('late')); }));
  s.stop();
  out.exactlyOnce = { afterStop, afterSecondStop: closed.slice(), lateCallbackRan: ran };
}
// 2. A child created inside a parent: the parent's own cleanups run first, then the child's; stopping the child again
//    closes nothing.
{
  const closed = []; const parent = effectScope(false); let child;
  parent.run(() => { onScopeDispose(() => closed.push('parent')); child = effectScope(false); child.run(() => onScopeDispose(() => closed.push('child'))); });
  parent.stop(); const afterParent = closed.slice(); child.stop();
  out.childWithParent = { afterParent, afterChildStop: closed.slice() };
}
// 3. A throwing cleanup: stop throws, the later cleanup does not run, and a second stop does not run it either.
{
  const closed = []; const s = effectScope(false);
  s.run(() => { onScopeDispose(() => { throw new Error('boom'); }); onScopeDispose(() => closed.push('later')); });
  let threw = null; try { s.stop(); } catch (e) { threw = e.message; }
  s.stop();
  out.throwingCleanup = { threw, laterRan: closed.length > 0, active: s.active };
}
// 4. A cleanup that stops its own scope, after the handle's close: the close runs once.
{
  const closed = []; const s = effectScope(false);
  s.run(() => { onScopeDispose(() => closed.push('close')); onScopeDispose(() => s.stop()); });
  s.stop();
  out.reentrantStop = { closed, active: s.active };
}
// 5. onScopeDispose with no active scope: no error, and nothing is registered.
{
  let threw = null; quiet(() => { try { onScopeDispose(() => { throw new Error('should not run'); }); } catch (e) { threw = e.message; } });
  out.noActiveScope = { threw };
}
console.log(JSON.stringify(out, null, 2));
```

## Appendix B: Node oracle output (logs/scope-oracle.json)

```json
{
  "version": "3.5.43",
  "exactlyOnce": {
    "afterStop": [
      "a",
      "b",
      "c"
    ],
    "afterSecondStop": [
      "a",
      "b",
      "c"
    ],
    "lateCallbackRan": false
  },
  "childWithParent": {
    "afterParent": [
      "parent",
      "child"
    ],
    "afterChildStop": [
      "parent",
      "child"
    ]
  },
  "throwingCleanup": {
    "threw": "boom",
    "laterRan": false,
    "active": false
  },
  "reentrantStop": {
    "closed": [
      "close"
    ],
    "active": false
  },
  "noActiveScope": {
    "threw": null
  }
}
```

# gc-arena churn experiment (WP-3.2)

Experiment only. This crate has its own `[workspace]` table, so the root workspace (`crates/*`) does not include it, and no
vn-* crate depends on it. gc-arena is pinned to `=0.7.0`. vn-value does not use gc-arena.

Run, from this folder:

    CARGO_TARGET_DIR=/home/claude/targets/w32-gc-arena cargo test
    CARGO_TARGET_DIR=/home/claude/targets/w32-gc-arena cargo run --release

## What it runs

The same live-set sizes (10k/40 rounds, 100k/40, 1M/30), the same round shape, the same threshold (2000 allocations) and
the same object count per round (11161) as `crates/value/examples/gc_churn_bench.rs`. Each round creates a list, rings, a
wide object with a child, a Map and a Set of objects, closures over cells, native functions over objects, promise chains
with their reactions, and one queued job, and holds each one in a holder slot across a collection point.

Three runs per size, each timing only the collector call:

- cycle: every collection point runs `Arena::finish_cycle`, one full mark and sweep. This is the comparable number for
  the vn-value collections. Each round's live count is checked against the expected count (stable set plus carried list).
- slice, default pacing: every collection point runs `Arena::collect_debt` when allocation debt is due. A pause is one
  incremental slice.
- slice, paced: the same, with `sleep_factor` 0.05 instead of the default 0.5. A tuning run, not a recommendation.

## How it differs from the vn-value workload

- Objects are one `Node` struct with a numeric id, numeric properties and a `Vec` of outgoing references. There are no
  property maps, no realm objects and no runtime-internal objects. The per-round object count matches (11161), but the
  per-object cost does not, so the two sets of pauses compare whole implementations, not only the collectors.
- The threshold counts the nodes this harness allocates. vn-value's threshold counts every heap allocation, including
  runtime ones, for example the resolving functions that `new Promise` creates (`builtins/promise/mod.rs` lines 204-205).
  This harness runs 4 threshold collections per round where vn-value runs 5; I did not count vn-value's allocations to
  confirm the full breakdown.
- Promises are plain nodes (promise, executor, handler, derived promise, result). The reaction jobs run in a Rust loop
  inside one `mutate` call; there is no job queue. The queued job is an entry in a root field until it drains.
- The round checks are the live count (cycle mode, every round), the stable set (end of run) and the list. The
  per-structure content checks of `tests/support/churn.rs` are not repeated here.

## Results (final run; logs in /home/claude/targets/w32-gc-arena/logs/ and /home/claude/targets/w32-gc/logs/)

p99 pause in milliseconds. Machine: Intel Xeon 2.80 GHz, 4 logical CPUs (shared container). One run per row, except the gc-arena cycle column, which also has a second run (`gc-arena-run1.log`) for noise. Run 1
came from an earlier build of this harness, before the paced mode and the heap-size line were added. Its cycle output
has the same format, but I did not keep the earlier source, so I cannot show that the cycle code was unchanged.

| live set | vn-value collection (non-incremental) | gc-arena cycle (full) | gc-arena slice, default pacing | gc-arena slice, paced |
|---|---|---|---|---|
| 10k | 6.823 | 0.621 (run 1: 1.321) | 0.601 | 0.516 |
| 100k | 16.488 | 10.589 (run 1: 15.645) | 2.555 | 1.583 |
| 1M | 206.346 | 101.205 (run 1: 99.957) | no steady-state slice ran (one 97.1 ms catch-up slice) | 20.996 |

Against the design budget (design.md section 14: collection slice p99 at most 2 ms), the verdicts are:

- Non-incremental collection (vn-value, and gc-arena in cycle mode) misses the budget at 100k and 1M. At 10k, gc-arena's
  full cycle meets it in both of its runs, but vn-value's full collection does not.
- Incremental slices meet it at 10k in both pacings. At 100k the default pacing is borderline: 1.7 ms in the earlier
  gc-arena run, 2.6 ms in the final one. The paced run meets it at 100k (1.58 ms).
- At 1M neither pacing shows a steady-state result. With the default pacing the heap is not collected during the 30
  rounds (one 97 ms slice pays the debt of building the stable set, and nothing else runs). With the paced run, the
  p50 slice is 0.08 ms, but p99 is 21 ms; with 150 slices that p99 is the second-largest slice.

Heap growth matters as much as pause length. At the end of each run, before the final full cycle, the number of
allocated objects still held was 38,082 (default) and 37,326 (paced) at 10k against 12,004 live; 283,580 and 170,231 at
100k against 102,004 live; and 1,334,834 at 1M against 1,002,004 live. In the 1M paced run, no garbage was freed at all.

## Why the slices behave this way

gc-arena starts a cycle only when the allocations since the last cycle exceed `sleep_factor` times the remembered object
count, with a floor of `min_sleep` (`finish_cycle`, metrics.rs). Allocation debt is then the cycle's debits minus its
credits (`allocation_debt`, metrics.rs). Credits are per object: mark 0.1, trace 0.4 and keep 0.05 for each live object,
so a full cycle over N live objects earns about 0.55 N credits. Garbage adds its own credits (drop 0.2, free 0.3 each).
At 1M objects the default pacing starts a cycle only after about 500k allocations, and this run allocated 334,830
objects in total, so no steady-state cycle started; the one slice was the catch-up slice. Pacing therefore depends on
the live-set size and has to be tuned per workload. The collector takes one gray object at a time and runs its `trace`
to completion (context.rs, the gray-queue loop). A container with a million children is traced in one such step, and
nothing splits that step. Its children are counted as they are marked, but the count cannot end the step early. This is
the likely cause of the 21 ms paced slice; I did not isolate it with a separate measurement.

## API and ergonomics, against ARCHITECTURE.md section 2

- Safepoints. ARCHITECTURE.md collects only at safepoints where no compiled code is on the Rust stack, so temporaries never
  need rooting. gc-arena encodes the same rule in the type system: a `Gc` can only exist inside a `mutate` closure, and a
  collection needs `&mut Arena`. The harness had to build, attach and check each structure inside one `mutate` call, with
  the collection point outside it.
- Rooting. vn-value roots are explicit handles (`Rt::root`/`unroot`, `RootProvider`, job queues, timers), and an `ObjRef`
  is `Copy`, with no lifetime. Host state (DocHost, timers, DOM listeners) can hold `ObjRef`s directly. In gc-arena every
  persistent reference is a field of the root type or a `DynamicRoot` (`DynamicRootSet::stash`/`fetch`), and it is
  fetched back into a `Gc` only inside `mutate`. That is a real cost for host code, which holds references outside any
  arena call.
- Stale references. vn-value detects a stale `ObjRef` at runtime through the generation counter. gc-arena makes stale
  references impossible by construction, which is its main benefit. Its cost is the `'gc` brand on every type that holds
  a `Gc`, and the `Collect` derive on each of them.
- Finalisation. A `Drop` impl on a type that holds `Gc` pointers needs `#[collect(unsafe_drop)]`. That attribute is an
  unchecked promise that `drop` never touches a GC pointer, so the compiler does not verify it. A `'static` type with no
  `Gc` fields can use `require_static` and keep its `Drop` impl. Either way, release at sweep time is a last resort.
  vn-value already treats GC-driven release as a last resort (`DetachedNodes`, `Timers`) and uses explicit disposal
  (EffectScope stop, listener removal, `clearTimeout`).
- Incremental collection. gc-arena supports it (`collect_debt`, `mark_debt`, `finish_marking`, `cycle_debt`, `finish_cycle`,
  `Pacing`). It also needs a write barrier on every mutation of an allocated object (`borrow_mut(mc)`, `Gc::write`), and
  vn-value has no barriers today, because it only collects at safepoints.
- Observability. `Metrics::total_gc_count` gives the live object count, which matches vn-value's `live_objects`. gc-arena
  has no byte total and no limit hook, so the byte-estimate heap limit (ABI.md section 12, `RangeError`) would be a layer
  on top of it.

## Recommendation

Do not adopt gc-arena for vn-value. Keep the ObjRef and explicit-root model and the safepoint collector. The measurements
do not show that gc-arena would meet the 2 ms budget where the current collector misses it. At 1M, incremental slices
depend on pacing tuned to the live-set size, and single large containers produce slices of 20 ms or more. Those
problems would also hit an incremental vn-value collector, so they point at the design, not the library. The cost of
gc-arena falls on the host boundary, where vn-value depends on references that outlive any arena call.

If the 2 ms cap becomes a go/no-go, the next experiment is incremental marking inside vn-value's heap. Measure it with
this harness. Test whether chunking large arrays and Maps bounds the trace work per slice. The design does not define the
reference workload's live-set size, so the budget cannot be tested as written until that size is chosen.

## Limits

- One run per row, on a shared 4-vCPU container. The gc-arena 10k and 100k cycle numbers moved by up to 2.1x between the two cycle runs (10k: 0.621 and 1.321 ms;
  100k: 10.589 and 15.645 ms). The 1M numbers moved by about 1%.
- Different object models and different threshold counts (see above). Compare the algorithms in the cycle rows only
  with that caveat.
- The paced sleep factor (0.05) is one value, chosen to show the effect. No sweep of values was run.

# Component and effect ownership — 2026-10-10

Task 4.3 acceptance evidence: props and emits, computed caching, keyed child removal, and unmount/recreate cycles. The
checks drive real components through the runtime renderer on the headless DocHost. The expected behaviour (which hooks
and watcher callbacks run, in what order, and the subscriber and effect counts Vue exposes) comes from vue@3.5.43, recorded
by `tests/ownership-oracle/record.mjs`. The counts of our own resources (effects, computeds, scopes, watchers, deps, links,
subscriber edges, host handles, listeners, DOM side tables and GC live objects) are measured. Their baseline is taken after
a warm-up cycle, not hardcoded.

## What was built

- Diagnostics accessors, documented as diagnostics only (the runtime never reads them):
  `vn_reactivity::debug_stats` returns the live reactivity objects of one realm (effects, computeds, scopes, watchers,
  deps, links and subscriber edges, where a subscriber edge is a link in a dep's subscriber list);
  `vn_reactivity::dep_subscriber_count` walks one dep's subscriber list; `DocHost::live_handles` counts the host handles
  that still name a node.
- Counting: every reactivity object is allocated through `node::alloc`, which wraps the node in a `Tracked` payload that
  adds one to its realm's counter. Dropping the payload takes the count back. The drop only touches a `Cell`, as
  `crates/value/ABI.md` requires. `graph::add_sub` and `graph::remove_sub` keep the subscriber-edge counter. A soft-removed
  link is not counted, because it is no longer in the list.
- Oracle: `tests/ownership-oracle/record.mjs` runs four scenarios on the vue@3.5.43 production build in Node, with a small
  node-ops tree (no DOM), and prints JSON. Its values are copied by hand into the constants at the top of
  `crates/runtime/tests/ownership.rs`.
- Tests: `crates/runtime/tests/ownership.rs` has 11 tests: six checks and five negative controls.

## What the checks show

(a) Props and emits. Two `Counter` instances under `Pair`, labelled A and B, each keep their own count. Three clicks on A
and one on B give the markup `A:6`, `B:2` and the emit log `change A 1`, `change A 2`, `change A 3`, `change B 1`, the same
as Vue. Removing B releases its render effect (effects -1), computed (-1), scope (-1), listener (-1) and three subscriber
edges (-3). A keeps its count and its emits afterwards.

(b) Computed. Two reads of a fresh computed run the getter once, as in Vue. A later change to the ref makes the next read run
the getter once more. The values are 10, 10, 20.

(c) Keyed removal. Three children watch one shared ref. Removing child 2 runs beforeUnmount, then scope disposal, then
unmounted, as Vue does. The ref keeps two subscribers (Vue: two), and the removal releases the child's two effects (render
and watcher), its watcher, computed, scope and listener, and three subscriber edges (-3). The removed watcher never fires
again.

(d) Unmount and recreate. Twenty keyed children are mounted and unmounted, 50 times after one warm-up cycle. After each
cycle and a collection (`Rt::collect`), every measured count equals the baseline taken after the warm-up: effects,
computeds, scopes, watchers, deps, links, subscriber edges, host handles, listeners, DOM expandos, DOM element handles and
GC live objects. A mounted list of 20 children measures +41 effects (the list's render effect, and a render and a watcher
effect per child), +20 watchers, +20 computeds, +21 scopes and +20 listeners, and all of them return to the baseline after
unmount.

(e) Unmount order. beforeUnmount runs, then scope disposal, then unmounted. A watcher whose source changes during
beforeUnmount does not fire, and the source's subscriber count drops to zero. The expected order is Vue's.

## Negative controls

1. Leaking child. The child's watcher is created inside a detached `effectScope(true).run`, so it outlives the child. The
   baseline check fails after the first cycle with effects +41, watchers +20 and subscriber edges +20. The same report
   lists computeds +20, scopes +21, deps +40, links +40, host handles +21, listeners +20 and live objects +1153. The
   leaked watcher's job holds the child instance: `watch` captures the current instance, as Vue's `doWatch` does
   (`job.i = instance`), so the child's render effect and elements stay live with it.
2. Leaking keyed child. The same leak in the keyed scenario. The oracle comparison fails because `watch 2 b` fires after
   the removal. The leaked watcher keeps its subscription, so the ref keeps three subscribers and the removal releases two
   subscriber edges instead of three. The child's render effect and watcher stay live, so effects and watchers do not fall.
3. Non-cached getter. The same helper that checks the computed reports "the getter ran 2 times for 2 reads" for a plain
   getter read twice. It accepts the cached computed, which runs once.
4. Shared state. Both counters are given one ref. The same clicks give `A:8`, `B:8` and an emit log with `change B 4`, so
   both the markup check and the log check reject it.
5. Corrupted expectation. The log comparison rejects a list with one changed line.

## Verification

| Command | Result |
| --- | --- |
| `NODE_ENV=production node tests/ownership-oracle/record.mjs` | exit 0; JSON printed; the last two runs are identical; the Rust constants match it |
| `cargo test -p vn-runtime --test ownership` | 11 passed, 0 failed |
| `cargo test -p vn-runtime -p vn-reactivity` | 363 passed, 0 failed, 0 warnings (352 passed before this change) |
| `node tooling/harness/run-language.mjs --with-vue --ui` | scenarios 174: pass 174, FAIL 0, blocked 0, compile-error 0, 0 unexpected; exit 0 |
| `node tooling/harness/run-language.mjs --with-vue --ui --gc-stress` | scenarios 174: pass 174, FAIL 0, blocked 0, compile-error 0, 0 unexpected; exit 0 |

The UI suite has 174 scenarios; older notes say 160. Replay does not need Chrome in this container. No browser test was
added, so none was skipped.

## Limits

- No browser test was added, and the Chrome-based tests in CI were not run locally.
- The oracle checks Vue's runtime-core and reactivity behaviour in Node on a node-ops tree. It does not compare DOM output
  with a browser.
- The counts are of objects still alive after `Rt::collect`. A stopped effect that is still referenced counts as live.
  The leaking controls depend on this.
- Subscriber edges are a realm-wide total. `dep_subscriber_count` gives the per-dep count used for the ref checks.
- The peak numbers of the cycle tests (for example 41 effects for 20 children) follow from the component structure: one
  list render effect, and two effects per child. The per-child effect count matches the oracle's. The oracle does not
  record a 20-child list, so the peak numbers are not an oracle result.
- The unmount-order check has no mutation of its own. The comparison-helper control covers it.
- The counters add one `Rc` clone and one counter update per reactivity object allocated and freed. This was not benchmarked.

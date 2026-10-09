# Vue reactivity and scheduler scenarios (WP-V0)

Each `<group>/<name>.js` observes Vue 3.5.43 reactivity or scheduling. Its golden, `tests/vue/golden/<group>/<name>.js.json`,
is the Node oracle's output for the **production** build (`NODE_ENV=production`). The scenario contract is
`docs/dev/trace-format.md`; the language rules are in `tests/language/cases/README.md`.

## Layout and header

- Groups: `ref/` (V01 V02 V07 V08 V10 V11), `reactive/` (V03 V04 V05 V09 V10), `computed/` (V06), `effect/` (V12),
  `watch/` (V13 V14 V15 V16), `scheduler/` (V17). The corpus has 102 scenarios.
- Line 1 `// @tier N`: 1 synchronous; 2 observed after `await testHost.tick()`; 3 promise, timer or async-watcher ordering.
- Line 2 `// @covers V01,V06`: requirement IDs V01..V17 that the scenario observes.
- Scenario name = path under this folder without `.js` (`watch/flush-order-pre-post-sync`). The language runner would
  name it by basename only, so `tooling/oracle/vue-env.mjs` rewrites that one `scenario` field after checking that the
  child's output is canonical.

## Commands (repository root)

```
node tooling/oracle/vue-env.mjs            # check goldens; exit 1 on a mismatch or an orphan golden
node tooling/oracle/vue-env.mjs --update   # regenerate every golden (unchanged scenarios give identical bytes)
node tooling/oracle/vue-env.mjs --lint     # lint the cases
node --test "tests/vue/oracle.test.mjs"    # WP-V0 acceptance
```

## Rules

Enforced by the lint (`vueImportAllowlist()` and `lintVueSource()` in `tooling/oracle/vue-env.mjs`):

- Imports only from `'vue'`, and only names that the export map classifies to V01..V17 with status P. Excluded (X in the
  profile): `effect`, `stop`, `ReactiveEffect`, `proxyRefs`, `getCurrentWatcher`, `queuePostFlushCb`. Use `watchEffect`,
  watch handles and `effectScope` instead. Component and DOM APIs are not allowed.
- No reads of Vue internals (`__v_*`, `dep`, `flags`, `_value`, ...).
- The language rules of `tooling/oracle/lint-cases.mjs` apply. Builtins and methods are checked against the stdlib manifest
  (`compiler/profile/stdlib.mjs`) for the test target: no classes, `Reflect`, `Proxy`, `Date`, `RegExp`, typed arrays, or
  `Object.defineProperty`; timers only as `testHost.timers`; no `console`.
- Method calls on Vue objects. The language lint matches a method call by name, so it cannot tell a Vue handle method from an
  unknown builtin. The wrapper exempts exactly `run`, `stop`, `pause` and `resume` (`VUE_MEMBER_CALLS`), which Vue's public
  typings declare on `EffectScope` and `WatchHandle`. The exemption is by name on any receiver, because the lint infers no
  receiver types. A name is exempt only if the stdlib manifest does not govern it. Any
  other unknown method name is still reported. `on` and `off` exist on `EffectScope` at runtime only, so they are not allowed.

By convention, checked by review:

- **Never trace a reactive proxy.** Tracing reads it and subscribes the reader. Trace primitives, booleans, or a plain copy
  made with `toRaw` and key-by-key copies. Compare identities with `===` and trace the boolean.
- Use `watchSyncEffect` as the run-count instrument: it runs during the write, so no flush is needed. Use plain counters.
- Observe pre- or post-flush output after `await testHost.tick()` inside `run()`.
- Do not observe engine-defined sequences. For example, the intermediate writes of `Array.prototype.sort` depend on the
  engine's sort algorithm.
- Catch clauses trace `e === ref`, `e instanceof X` or `e.name`, not an engine error's message.

## Known Vue behaviours asserted on purpose

These look surprising. Each golden records what the oracle printed.

- `reactive/readonly-array-push-returns`: `push` on a readonly array returns the length it would have reached, but stores nothing.
- `reactive/is-flags-nested`: `readonly(reactive(o))` is readonly and reactive. `reactive(readonly(o))` returns the readonly proxy itself.
- `reactive/getter-this-receiver`: writing an accessor key also triggers that key after the setter, so a reader reruns once more.
- `reactive/set-add-delete-iterate`, `reactive/map-get-set-has-size`: readers triggered by one write run in Vue's batch order,
  not in subscription order.
- `reactive/frozen-and-primitive-targets`: `reactive()` returns frozen objects and primitives unchanged.
- `ref/toref-default-and-getter`: assigning to `toRef(getter)` throws a TypeError in strict code (the getter form has no setter).
- `ref/ref-inside-reactive-unwrap`: refs inside a reactive array or Map are not unwrapped.
- `ref/ref-of-reactive-and-ref`: writing back the proxy already held is not a change.
- `computed/self-reading-cycle`: a computed that reads itself gets its previous value, with no recursion.
- `effect/scope-nested-stop-cascade`, `effect/scope-effects-before-cleanups`: stop order is effects, then the scope's own cleanups,
  then child scopes.
- `watch/shallow-ref-trigger-ref-forced`: a watch on a shallowRef ignores nested writes. `triggerRef` calls back even when the
  object is the same.
- `watch/onwatchercleanup-after-await`, `watch/onwatchercleanup-outside-noop`: `onWatcherCleanup` after an await, or outside a
  watcher, registers nothing and does not throw.
- `watch/pause-resume-writes`: `resume()` delivers the latest value once; writes made while paused coalesce.
- `watch/deep-numeric-depth`: `deep: N` counts levels below the source. A write three levels down is seen by neither watcher.
- `watch/watcheffect-sync-pre-post`: the first run of `watchEffect` and `watchPostEffect` is synchronous, at creation.
- `watch/flush-order-vs-promise-jobs`, `scheduler/write-inside-nexttick`: a pre-flush job runs after promise jobs queued before the
  write and before `nextTick` callbacks registered after it.
- `scheduler/nexttick-idle-same-promise`: with nothing pending, `nextTick()` returns the same promise object each time.

## Adding a scenario

1. Create `cases/<group>/<name>.js` with the two header lines.
2. `node tooling/oracle/vue-env.mjs --lint`, then `node tooling/oracle/vue-env.mjs --update`. Review the new golden by hand.
   It must match the scenario's comment. A surprising value is a finding to explain in the comment, not a reason to edit
   the golden.
3. `node --test "tests/vue/oracle.test.mjs"`. Every V01..V17 row must stay covered by at least two scenarios.

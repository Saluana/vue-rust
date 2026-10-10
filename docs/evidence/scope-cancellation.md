# Scope-owned cancellation and stale-completion rejection (task 3.3) — 2026-10-10

Built on branch claude/w33-cancel. Built on 554b716. Private target dir `/home/claude/targets/w33-cancel`.

## What was built

`crates/runtime/src/operations.rs` (module `vn_runtime::operations`) is a Rust-only owned-operation mechanism. A host performs
the external effect; the runtime owns the JS promise and decides whether a result may be delivered.

- `start_owned(rt, label)` runs under the active effect scope (a component instance's scope during setup, or any
  `effectScope`). It returns the promise and a `DeliveryToken { scope, generation, operation }`.
- `complete`, `fail(code, message)` and `cancel` report the outcome. Each returns a `Delivery`: `Delivered`, `Cancelled`,
  `DiscardedStopped`, `DiscardedStale` or `DiscardedSettled`. `stats(rt)` returns counters for every outcome.
- `ServiceHost` is the host side: `start(token, label)` begins the request, and `cancel(token)` stops it.
- `install(rt, host)` sets the host of the realm.

The module docs state each choice. The main ones follow.

1. Ownership. The first operation of a scope takes an owner slot and registers an `onScopeDispose` cleanup on that scope.
   The cleanup frees the slot when the scope stops. The `generation` of a slot increases each time the slot is reused, which
   gives an ABA guard. Design 3.3 names a "stable component ID and generation", but the runtime has no keyed component
   identity yet, so the slot and generation pair stands in for it.
2. Disposal (design 3.4: "Disposal requests cancellation and invalidates delivery"). The cleanup calls `ServiceHost::cancel`
   for each pending operation of that owner, in operation order. It drops the resolving functions, so the promise stays
   pending forever. Rejecting on disposal would run the catch callbacks of a disposed component, and 3.4 says a late
   completion "never resolves user promises, never runs user callbacks".
3. Explicit cancel by a live owner (design 5, `ServiceError` with code `Cancelled`). The promise rejects with
   `{ code: 'Cancelled', message }`, then the host is told to cancel. The external effect may still happen (3.4). Its result
   is discarded.
4. First settlement wins. After `complete`, `fail` or `cancel`, a later delivery is `DiscardedSettled`.
5. Stale and stopped deliveries are discarded and counted (design 6: "Generation-checked service delivery discarded; normal
   JS continuation behavior retained"). The generation is checked before liveness.
6. Starting with no active scope, or in a stopped scope, throws a TypeError before any host call (no operation is left
   unowned). Design 3.4 does not say what happens here, so this is the conservative reading.
7. Ordinary promises are untouched (design 3.4 and R05.AC3). A promise the scope does not own keeps its JS behaviour after
   unmount, while the disposed component's effects stay stopped.
8. Settling uses `Promise.withResolvers`, because the value crate's resolve helpers are `pub(crate)`. Thenables and reaction
   jobs therefore follow the spec. Resolving never runs user code synchronously, except a thenable's `then` getter.

Composable use (Rust, no JS API yet): call `start_owned(rt, "fetch")` inside setup, return `started.promise` to the caller,
and keep `started.token` with the host request. The host calls `complete(rt, token, value)` or `fail(rt, token, code, msg)`
when the work finishes. A live caller that wants to abandon the work calls `cancel(rt, token)`.

Files: `crates/runtime/src/operations.rs` (new), `crates/runtime/src/lib.rs` (one `pub mod` line),
`crates/runtime/tests/owned_operations.rs` (new, 12 tests), `tests/vue/owned-ops/record.mjs` (new Node oracle),
`docs/evidence/scope-cancellation.md` (this note). No other file was touched. No dependency was added. No `unsafe` was added.

## Oracle

`NODE_ENV=production node tests/vue/owned-ops/record.mjs` runs the real `@vue/runtime-core` 3.5.43 production build with an
in-memory renderer. It prints the order for two scenarios. The Rust test copies these lines by hand (as constants, with
the script cited in a comment).

- `unmount-order`: `setup`, `render 0`, `watch 1`, `render 1`, `bum`, `watch-cleanup 1`, `user-dispose`, `host-cancel op`, `um`.
  The `host-cancel op` line stands in for the runtime's owned-operation cancellation. The oracle registers it as an
  `onScopeDispose` after the user's, which matches the runtime's registration point.
- `ordinary-promise-after-unmount`: `setup`, `render 0`, `then late`, `store late`. The disposed render does not re-run.

The other expected values are hand-derived from design 3.4 and 6 and are cited in the tests.

## Commands and results

| Command | Result |
| --- | --- |
| `cargo test -p vn-runtime -p vn-reactivity` (baseline, before changes) | 352 passed, 0 failed, 0 ignored |
| `cargo test -p vn-runtime -p vn-reactivity` (after) | 364 passed, 0 failed, 0 ignored (352 + 12 new) |
| `cargo test -p vn-runtime --test owned_operations` | 12 passed, 0 failed |
| `node tooling/harness/run-language.mjs --with-vue` (baseline) | 115 pass, 0 blocked, 0 FAIL, exit 0 |
| same command (after) | 115 pass, 0 blocked, 0 FAIL, exit 0 |
| `node --test tests/vue/oracle.test.mjs` (after) | 16 pass, 0 fail |

The ticket names a baseline of 102/102 for `--with-vue`. The corpus at 554b716 has 115 scenarios (computed 12, effect 15,
reactive 25, ref 22, scheduler 12, watch 29), and all 115 passed before and after this change. The 102 figure is out of date.

The 12 tests are: completion while mounted; unmount during the request; an ordinary promise after unmount; the stale
generation after remount; explicit cancel; cancel then completion in one tick; completion then cancel in one tick; a wrong
generation rejected while the right one delivers; `fail` with a ServiceError code; the Vue order of unmount; starting
outside a scope or in a stopped one; and an `effectScope` owner that cancels when it stops.

## Negative controls

- Wrong generation: `a_wrong_generation_is_rejected_while_the_right_one_delivers` checks that a token with the generation
  bumped is discarded, the promise stays pending, and the correct token then delivers.
- Mutated Vue order: `unmount_order_matches_vue_3_5_43` asserts that the log differs from the same order with the two
  scope cleanups swapped.
- Mutation runs (done by hand; the module was restored and confirmed byte-identical with `cmp`, then the 12 tests passed again):
  - M0, only the generation check skipped: `a_handle_from_an_earlier_instance...` fails with `DiscardedSettled` where
    `DiscardedStale` is expected. `a_wrong_generation...` fails because the full-token comparison rejects the token. No
    completion is delivered.
  - M1, generation check skipped and operation records kept after disposal (the retained closures design 3.2 forbids):
    `a_handle_from_an_earlier_instance...` fails with `Delivered` where `DiscardedStale` is expected. The stale completion
    resolves the old promise. This is the failure the generation check exists to stop. `a_wrong_generation...` also fails,
    at the same line as in M0.
  - M3, disposal does not call `ServiceHost::cancel`: four tests fail (`an_effect_scope_owner...`, the stale-generation test,
    `unmount_during_request...`, `unmount_order...`).

The stale completion is blocked by three facts. Disposal removes the operation record. The full token is compared with the
record. The generation is checked against the slot. The generation check is what classifies a stale completion, and M0
shows the classification changes without it. Delivery is blocked only when the record is also gone, which is why M1 is
needed to show the stale delivery.

## Limits

- Rust only. There is no JS-visible API, no AbortSignal integration, and no real file or HTTP service. Real services are later
  tasks (T19 and the services task).
- The ticket's acceptance line "scope cancellation rejects stale service delivery while retained ordinary promise closures
  keep JS behaviour" is covered by tests 2, 4 and 3, using the fake host only.
- No host in `hosts/` or `crates/renderer-dom` implements `ServiceHost` yet.
- The Vue expectations are hand-copied, so `owned_operations.rs` does not run Node. `record.mjs` is run by hand, and it is not
  wired into `node --test`.
- No browser test was added, so no `chromeSkipReason()` gate was needed. Chrome is not available in this container.
- Not run: `--gc-stress`, `cargo test --workspace`, and any rustfmt or clippy check (not installed here).
- Stated as open questions for the maintainer: (a) whether starting in a stopped scope should throw (chosen) or return a
  rejected promise; (b) whether a stable component ID should replace the slot and generation pair once keyed component
  identity exists; (c) whether design 3.4's "pending forever" should later be a rejection on a separate, explicit path.

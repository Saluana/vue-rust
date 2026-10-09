# Compiled browser app entry

The generated app uses browser host ABI 1. Its optional application exports supplement the existing host ABI; host-only modules continue to run without them.

- `vn_start() -> u32`: mounts once; zero means success and nonzero means startup failed. The page shell rejects `window.vnReady` on failure. Older host-only entries returning void remain accepted.
- `vn_event_context(serial: u32, kind: u32)`: receives the real browser event's identity and constructor family before the normal `vn_event` call. Kind is 0 Event, 1 MouseEvent, 2 KeyboardEvent, or 3 InputEvent. The glue assigns identities with a WeakMap and preserves an event's identity across listeners.
- `vn_flush()`: drains Rust promise/scheduler work, clears pending DOM ticks and the dispatch cache, reports errors, then collects at a safepoint. The glue queues this in a browser microtask after listener delivery and coalesces pending flushes.

The generated entry retains the realm for the wasm instance. The browser host tracks created handles so runtime retirement can release every mapped descendant of a detached subtree. This does not add a JavaScript engine to application execution: application functions and Vue semantics run in compiled Rust/WASM.

These app exports are generated in tooling/cli/build.mjs. Delivery and lifecycle regressions live in tests/platform/web/e2e-vue-apps.test.mjs, including repeated hide/show cycles with stable live handle and listener counts.

# Promise programs (WP-B5 differential corpus)

`programs.json` holds promise programs written in the small op language below, with the outcome that Node 26 (V8, real
`Promise`) produces for each one. `crates/testkit/tests/promise_programs.rs` runs every program on the Rust runtime
(`vn_value::builtins::promise`) and compares the log, the final promise states and the unhandled rejections. The generator
is `tests/language/gen/gen-b5.mjs` (`node tests/language/gen/gen-b5.mjs` rewrites `programs.json`, byte-identical on every run).
The first 103 programs are hand-designed (each has a `doc`); the other 300 come from a seeded random generator.

## Program

`{"name": "...", "doc": "...", "ops": [OP...]}`

The ops run in order, synchronously (like module top-level code). Then the job queue is drained once, to empty. Then the
outcome is recorded.

## Items (values)

- A JSON string is a string value (labels use `[A-Za-z0-9_:.-]`).
- A JSON number is a number value.
- `{"undef": true}` is `undefined`.
- `{"ref": "ID"}` is the promise or thenable created earlier with that id.

## Ops

| Op | Meaning |
|---|---|
| `{"op":"new","id":I}` | `new Promise` with explicit resolvers, kept under I |
| `{"op":"withResolvers","id":I}` | `Promise.withResolvers()`, kept under I |
| `{"op":"resolve","id":I,"v":X}` / `reject` | call the resolving function of I with X |
| `{"op":"thenable","id":I,"mode":M,"v":X}` | an object whose `then(res, rej)` logs `then:I` and then does M: `resolve` (res(X)), `reject` (rej(X)), `throw` (throws X), `never`, `twice` (res(X), then res("late")) |
| `{"op":"then","id":I,"src":S,"f":H,"r":H2}` | `S.then(H, H2)` kept under I |
| `{"op":"catch","id":I,"src":S,"r":H}` | `S.catch(H)` |
| `{"op":"finally","id":I,"src":S,"f":H}` | `S.finally(H)` |
| `{"op":"static","id":I,"fn":N,"items":[X...]}` | `Promise[N](array of items)` for all, allSettled, race, any; `Promise.resolve(X)` / `Promise.reject(X)` when N is resolve or reject |
| `{"op":"log","label":L}` | synchronous log entry |
| `{"op":"queue","label":L}` | `Promise.resolve().then(() => log(L))` (an unobserved job marker) |
| `{"op":"async","id":I,"steps":[STEP...]}` | an async call; I is its result promise |

## Handlers H (for then, catch, finally)

| H | Behaviour |
|---|---|
| `{"h":"none"}` | not callable: the value passes through |
| `{"h":"log","label":L}` | logs L, returns undefined |
| `{"h":"logArg"}` | logs its argument (strings as they are, numbers via String, undefined as "undefined"), returns undefined |
| `{"h":"return","v":X}` | returns X (a promise or thenable is adopted) |
| `{"h":"throw","v":X}` | throws X |

## Steps (async bodies)

| Step | Meaning |
|---|---|
| `{"s":"await","v":X}` | `await X` |
| `{"s":"log","label":L}` | synchronous log entry |
| `{"s":"return","v":X}` | return X from the async call |
| `{"s":"throw","v":X}` | throw X |
| `{"s":"try","body":[...],"catch":[...] or null,"finally":[...] or null}` | try / catch / finally |
| `{"s":"logCaught"}` | inside a catch block: logs `caught:` + the rendered caught value |

## Outcome (both engines print exactly this shape)

```json
{"log": ["..."], "states": [["ID", "pending" | "fulfilled:V" | "rejected:V"], ...], "unhandled": ["V", ...]}
```

- `states` lists each promise created by new, withResolvers, then, catch, finally, static and async, in creation order.
  Node reads the state with `util.inspect` (which does not add jobs); the Rust side uses `promise_state`.
- `unhandled` lists the reasons of the rejected promises that have no handler at the end, in rejection order.
  Node uses `unhandledRejection` and `rejectionHandled` events, read after two macrotask turns; Rust uses
  `Rt::take_unhandled_rejections`.
- V is the rendered value: strings as they are, numbers via `String(n)`, undefined as `undefined`.

Tick table (V8 live, via the same oracle): `node tests/language/promise-programs/tick-table.mjs`.

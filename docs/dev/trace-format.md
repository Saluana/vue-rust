# Trace format v1 (canonical observation format)

Differential tests run the **same scenario** on two engines — Node (the oracle, V8 + the real `vue@3.5.43`
where Vue is involved) and the compiled Rust runtime — and compare their traces. Both engines must emit
exactly this format. A scenario is an ES module that only uses profile features, imports from `'vue'`, and
two host globals: `trace` and `testHost`.

## Scenario module contract

```js
// tests/language/cases/example.js
export async function run() {          // optional; harness awaits it after module evaluation
  trace('x', 1 + 1)
}
trace('module-init', 'ran')            // top-level code runs at module evaluation
```

Order of events for one scenario run (identical in both engines):
1. Fresh realm. Globals `trace`, `testHost` installed.
2. Evaluate the module. A throw during evaluation is recorded as `result: "throw"`.
3. Drain jobs (all promise reactions / microtasks) until the queue is empty.
4. If the module exports `run`, call it, then drain jobs again until the returned promise settled and the
   queue is empty (a rejection of `run` is recorded as `result: "throw"`).
5. Drain jobs a final time; report unhandled rejections (see below). Print one JSON document.

## Host globals

- `trace(label: string, ...values: any[]): void` — append `{ "label": label, "values": [encode(v)...] }`.
- `testHost.timers` — deterministic fake clock (no real time ever):
  `setTimeout(fn, ms) -> id`, `clearTimeout(id)`, `now() -> number` (starts at 0),
  `advance(ms) -> Promise<void>`: moves the clock, firing due timers ordered by (due time, creation order);
  after **each** callback the job queue is drained completely before the next timer fires.
- `testHost.tick() -> Promise<void>`: resolves after the job queue has drained once (macrotask boundary).
- More host APIs (mounting components, dispatching events, tree serialization) are added by UI WPs; they
  extend this document.

## Output document

```json
{ "scenario": "<name>", "result": "ok" | "throw",
  "error": null | <encode(thrown value)>,
  "entries": [ { "label": "…", "values": [ … ] }, … ],
  "unhandledRejections": [ <encode(reason)>, … ] }
```

Entries appear in call order. Both engines print compact JSON on one line.

## encode(value)

| JS value | Encoding |
|---|---|
| `undefined` | `{"$":"undef"}` |
| `null`, `true`, `false` | JSON `null`, `true`, `false` |
| finite number, not `-0` | JSON number written as JS `String(n)` would (e.g. `1`, `0.1`, `1e+21`, `-5`) |
| `-0` | `{"$":"num","v":"-0"}` |
| `NaN`, `Infinity`, `-Infinity` | `{"$":"num","v":"NaN"}` / `"Infinity"` / `"-Infinity"` |
| string | JSON string. Lone surrogates are escaped as `\udXXX` (lowercase hex) exactly like `JSON.stringify` |
| symbol | `{"$":"sym","id":<ordinal>,"desc":<string or null>}` |
| function | `{"$":"fn","name":<string>,"length":<n>}` |
| object / array / Map / Set / Error / Promise | see below |

Objects get a global ordinal `id` (1, 2, 3 … in order of first encounter during the whole run; symbols use
their own counter). Inside **one** `trace` call each object is fully expanded the first time it is met and
encoded as `{"$":"ref","id":N}` afterwards (aliases and cycles are therefore visible). A later `trace` call
expands it again (current state), keeping the same `id`.

```
{"$":"obj","id":N,"cls":"Object"|"Array"|"Map"|"Set"|"Error"|"Promise","proto":"Object.prototype"|"null"|"other",
 "props":[[key,encode],…],      // own enumerable string-keyed props, ordinary [[OwnPropertyKeys]] order
                                // (integer keys ascending, then insertion order). Array index keys are NOT here.
 "elems":[encode|{"$":"hole"},…], "len":N,   // arrays only
 "entries":[[encode(k),encode(v)],…],          // Map only, insertion order
 "values":[encode,…],                          // Set only, insertion order
 "name":…,"message":…,                         // Error only (strings); `stack` is never traced
 "integrity":"nonextensible"|"sealed"|"frozen"}  // only present for NON-extensible objects: the strongest that holds
                                               // (`Object.isFrozen` → "frozen", else `Object.isSealed` → "sealed", else "nonextensible")
```
(v1.1, coordinator amendment: the `integrity` field makes `Object.freeze/seal/preventExtensions` results observable. Absent ⇒ extensible.)

- An accessor property is encoded as `[key, {"$":"accessor","get":true|false,"set":true|false}]`; getters
  are never invoked while tracing.
- `Promise` objects encode as `{"$":"obj","id":N,"cls":"Promise","proto":"other","props":[]}` (state is not
  observable synchronously; scenarios trace results from `.then`/`await`).
- Symbol-keyed properties are not traced in v1. `Date`, `RegExp`, typed arrays, `BigInt`, `WeakMap`,
  `WeakSet`: `trace` throws a `TypeError("untraceable value")` (the scenario is invalid).
- Vue reactive proxies must not be traced directly (tracing would subscribe to them). Scenarios convert
  state to plain data first (e.g. via a helper that reads with `toRaw`).

## Unhandled rejections

A promise that is rejected and still has no handler after the final job drain appears in
`unhandledRejections` (in rejection order). Node's own `unhandledRejection` event is captured by the harness
(it must not crash the process).

## Comparison rule

The harness parses both JSON documents and compares them structurally (deep equality, array order
significant, object key order irrelevant). The first differing path is printed. Comparing raw text is not
required.

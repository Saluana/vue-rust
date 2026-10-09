# Builtin call corpora (data-driven differential tests, no compiler needed)

Builtin methods (`Array.prototype.splice`, `Object.keys`, `Map.prototype.set`, `JSON.stringify`, …) are verified by replaying
**recorded calls**: Node (V8) executes each case and records the encoded state before/after; the Rust test decodes the "before" state into
runtime values, performs the same call through the runtime (`rt.call`/`rt.construct` on the real builtin function objects found via the global
object), encodes the "after" state with the same encoder, and compares. Values use the trace-format.md encoding.

## Case format (`tests/language/builtins/<suite>.json` = `{ "suite": "...", "cases": [ … ] }`)
```json
{ "id": "slice/negative-start",
  "target": "Array.prototype.slice",          // dotted path resolved from the global object; "new:Map" means construct
  "pre": [ <thisEnc>, <arg0Enc>, <arg1Enc> ],  // ONE encoding pass over [this, ...args] (shared id space, aliasing visible)
  "cbs": [ null, null, "double" ],             // optional: positions replaced by named callback presets (see below), else null/absent
  "post": { "ret": <enc> | null, "throw": null | {"name":"TypeError"},
            "state": [ <thisEnc>, <arg0Enc>, ... ] }   // ONE encoding pass over [ret, this, ...args, log] after the call
}
```
- `this` of `"new:X"` cases is omitted (pre contains only args).
- `throw.name` is `e.name`; messages of engine-generated errors are never compared.
- "post" is a single encoding pass over `[ret, this, arg0, …, log]` (`log` = the shared callback log array, see below), so object ids are assigned by first
  encounter in that order on both engines.
- Static targets (`Object.keys`) use `this: undefined`-less layout: `pre` = args only, flagged by `"static": true`.

## Callback presets (identical JS in Node and native Rust in the replay harness)
A callback preset is a function recorded into a shared `log` array (a plain array created by the harness, always the last element of `state`):
`identity(x)`, `double(x)`, `isEven(x)`, `isTruthy(x)`, `index(_, i)`, `pair(x, i)`, `sum(acc, x)`, `concat(acc, x)`, `cmpNum(a, b)=a-b`, `cmpDesc(a,b)=b-a`,
`cmpStr(a,b)` (locale-free code-unit compare returning -1/0/1), `cmpThrow` (throws `Error('cmp')` on the 2nd call), `throwAt2(x,i)` (throws when `i===2`),
`toArr2(x)=[x,x]`, `keyOf(x)=String(x)`. Each preset appends `[presetName, ...encodedArgs]` per call to `log` **before** returning (args encoded as plain
primitives or `{"$":"ref"}`-free scalars only; callbacks for object elements log their index only). Defined once in `tooling/oracle/builtin-corpus.mjs`
(JS) and re-implemented natively in the Rust replay harness (`crates/testkit`); both must behave identically.

## Generator library (`tooling/oracle/builtin-corpus.mjs`)
`defineSuite(name, cases)` where a case is `{ id, target, make: () => ({ this?, args: [...] }), cbs?, static?, construct? }` — `make` builds fresh JS inputs for each execution (Node
runs it twice: once to record `pre`, once to execute). `writeSuite(path, suite)` runs everything in Node, encodes with `tooling/oracle/trace.mjs`, and writes deterministic JSON.

## Format additions (WP-O2, approved)
- `"get": 1` — the target is a data property such as `Math.PI`; nothing is called, `pre` is `[]`, `post.ret` is the value read.
- `"approx": 1` — transcendental results may differ from the platform libm in the last ulp: the Rust replay compares numbers within 1 ulp for these cases only.
- `cbs` is aligned with `pre` (same positions; `null` where the argument is a plain value).
- Callback log rule: every preset appends `[presetName, ...loggedArgs]` to the shared `log` array **before** it returns or throws. A primitive argument is logged as itself; an object,
  array or function argument is logged as the index that the calling method passed (second callback argument for map-like methods, third for reduce) or `-1` when none is passed (sort).
  The exact bodies are documented in the header of `tooling/oracle/builtin-corpus.mjs` and must be re-implemented identically in the native replay harness.
- Object integrity (frozen/sealed/non-extensible) is part of the object encoding since trace-format v1.1 (`"integrity"`), so cases on frozen inputs are now expressible.

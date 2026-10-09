# WP-RT-A corpora (vn-runtime data layer)

Expected values come from the real `vue@3.5.43` package (production build) run in Node. Nothing here is a hand-written
expectation except the flag names in the generator's inputs.

| File | Written by | Checked by | Content |
|---|---|---|---|
| `corpora/helpers-{class,style,display,names,merge,list}.json` | `gen-helpers.mjs` (`NODE_ENV=production node tests/ui/runtime-a/gen-helpers.mjs`) | `crates/runtime/tests/rta_helpers_replay.rs` | builtin-style cases on `Vue.<name>` targets (947 cases) |
| `corpora/vnodes.json` | `gen-vnodes.mjs` | `crates/runtime/tests/rta_vnode_replay.rs` | 403 scenarios, 533 steps of calls to the Vue exports |
| `corpora/flags.json` | `gen-vnodes.mjs` (from `@vue/shared`) | `rta_vnode_replay.rs::flags_match_vue_shared` | PatchFlags, ShapeFlags, SlotFlags |

Both generators are deterministic: running them again rewrites the same bytes.

## Canonical description (vnodes)

`describe(v)` is compact JSON text, identical in Node (`gen-vnodes.mjs`) and Rust (`vnode::testing::describe`):

- `undefined` `{"$":"undef"}`; `null`; `true`/`false`; numbers as their String() text, with `{"$":"num","v":"-0"|"NaN"|"Infinity"|"-Infinity"}` for the special values.
- strings as JSON strings; symbols `{"$":"sym","desc":<string or null>}`.
- functions `{"$":"fn","name":<name>,"length":<length>}`; arrays as `[...]` with `{"$":"hole"}` for holes.
- other objects `{"$":"obj","props":[[<key>,<value>],...]}` over own enumerable string keys in own-key order.
- a vnode `{"$":"vnode","type","key","ref","props","children","shapeFlag","patchFlag","dynamicProps","dynamicChildren","staticCount","appContext"}`, each field described as above; `dynamicChildren` is `null` or `{"len":N,"kinds":[...]}` where a kind is the type string, `@<symbol description>`, `fn`, `obj` or `other`.

## Argument language (vnodes)

A scenario is `{"id", "steps": [{"call", "args"}], "expect": [...]}` (`"unsupported"` marks a documented deviation).
Each step calls `Vue[call](...args)` and records the description of the result, or `throw:<error name>`. Arguments:
JSON `null`, booleans, numbers and strings; arrays; and tagged objects: `undef`, `num`, `ref` (result of an earlier step),
`export` (a `vue` export by name), `sym` (`Symbol.for`), `hole` (inside arrays), `obj` (`props` as ordered pairs),
`fn` (`name`, `length`, `returns`, optional `props`).

## Deviations recorded here

- `createStaticVNode` throws `TypeError` (static HTML is not supported). The scenario `static/throws` keeps Vue's own
  expectation and is marked `unsupported`; the replay requires the port to throw there and counts it separately.
- Callback-free helper cases only: functions and custom `toString` objects cannot be carried by the trace format, so
  those cases live in the Rust unit tests (`crates/runtime/tests/rta_vnode_unit.rs`).
- `Vue.capitalize` is not a supported export in `compiler/profile` (manifest class X), so it is covered through
  `Vue.toHandlerKey`, which calls it.
- Vue's module-level string cache (`cacheStringFunction`) makes results depend on earlier calls in one process; corpus
  inputs avoid strings that a cached key could match.

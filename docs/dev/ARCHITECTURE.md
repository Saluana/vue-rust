# Architecture decisions (coordinator, binding for all work packages)

Source of truth for *requirements and scope*: `planning/vue-native-framework/{requirements,design,tasks}.md`.
This file records the concrete engineering decisions taken to implement them. It is a living document owned by
the coordinator; implementers read it, they do not edit it.

## 1. Pipeline

```
.vue ──@vue/compiler-sfc (official)──► script (compileScript, inlineTemplate) + template render fn + styles
        │                                      │
        │   profile checks on the user's own source (default-deny manifest, spans mapped to .vue)
        ▼
 JS/TS module(s) (user code + Vue-generated code) ──Babel AST──► HIR ──► closure-converted MIR ──► Rust source
        ▼
 generated Rust  +  crates: vn-value (JS value runtime) · vn-reactivity · vn-runtime (Vue runtime-core/dom port)
                           · vn-ui (host element tree, style, layout, a11y) · vn-renderer-{native,dom,ssr} · vn-services
        ▼
 cargo ─► native (macOS/Linux/Windows) | wasm32 (+ tiny hand-written JS glue, no wasm-bindgen) | SSR binary
```

* **Frontend stays JavaScript** (Node): official `@vue/compiler-sfc` / `compiler-dom` / Babel. We *reuse* the
  official macro and template transforms (`compileScript` with `inlineTemplate: true`, `compileTemplate`,
  `compileStyle`) so macro/template semantics equal Vue 3.5.43. Our own code starts at the emitted JS module.
* **Backend is Rust**: JS functions become Rust `fn` items operating on `vn_value::Value`; there is no JS
  interpreter, JIT, webview or `eval`. Unsupported JS fails at compile time with a span.
* **Vue semantics are native Rust ports** (behaviour of `@vue/reactivity`, `runtime-core`, `runtime-dom`,
  `server-renderer` 3.5.43). The oracle is the real package running in Node. We do *not* compile Vue's own
  JS runtime (it needs Proxy/classes/Reflect, which the profile excludes).

## 2. Value runtime (`crates/value`, package `vn-value`)

* `Value` = `Undefined | Null | Bool | Number(f64) | String(JsStr) | Symbol | Object(ObjRef)` (+ internal
  `Uninit` for TDZ). `JsStr` is an immutable UTF-16 code-unit string (`Rc<[u16]>`): lone surrogates are legal.
* Objects live in a non-moving arena `Heap` addressed by `ObjRef {slot, generation}` (Copy). Strings and
  symbols are not heap objects. A freed slot's generation is bumped so stale refs are detectable.
* **Collection is mark-sweep and runs only at safepoints between jobs** (event handler, promise job, scheduler
  flush, explicit `Rt::collect()`): at a safepoint no compiled code is on the Rust stack, so generated code
  never has to root temporaries. Roots = global object, intrinsics, module records, persistent roots
  (`Rt::root`/`unroot`), job queues, timers, and registered `RootProvider`s. Anything that stores `ObjRef`s in
  Rust-side structures **must** implement `Trace` and be reachable from a root (preferably stored as a
  `ObjectKind::Native` heap object). A soft heap cap makes allocation return a resource-exhaustion `Throw`.
* Closure conversion: a variable captured by an inner function lives in a heap `Cell` object; functions are
  heap objects holding a plain `fn` pointer + captured cells + metadata. No Rust closure ever owns a `Value`.
* `async` functions keep their locals/temporaries in a heap frame object while suspended; resumption goes
  through the promise job queue (ECMAScript ordering), never through a Rust executor's own scheduling.
* Borrow discipline: heap/runtime borrows are short and released **before** calling getters, coercion hooks,
  callbacks or host code (reentrancy is normal). `Rt` is a cheap `Clone` handle; methods take `&Rt`.
* JS throw = `Err(Throw(Value))`; `type Res<T> = Result<T, Throw>`. Rust panics are framework bugs.
* Exotic objects (`ObjectKind::Exotic(Rc<dyn Exotic>)`) let `vn-reactivity` implement reactive/readonly
  proxies without exposing a general JS `Proxy`.

## 3. Language profile

Closed-world, default-deny. `compiler/profile/manifest.json` (generated from `design.md` §8 plus the pinned
`vue@3.5.43` export list) says per construct/API: status `P` faithful, `A` adapted, `W` web-only, `X` rejected
with a diagnostic, `E` experimental. Anything not listed is unsupported. Language semantics are validated
against Node (V8) with differential scenarios (`docs/dev/trace-format.md`).

## 4. UI layering

`vn-runtime` is a port of Vue's renderer and drives a **DOM-like host element interface** (`HostOps`:
create/insert/remove/set-attribute/set-property/add-listener/set-style/...). Hosts:
1. headless test host (records ops + serializes the element tree; used by all oracle tests),
2. DOM host (WASM exports + generated JS glue applying a mutation buffer to the real DOM; browser-owned inputs),
3. native host (`vn-ui` element tree → CSS-subset cascade → Taffy layout → egui painter/widgets via eframe; real
   window on macOS, AccessKit through egui).
SSR uses the same component code in a Rust binary and emits HTML + hydration data. Custom-drawn controls are
never described as OS-owned widgets.

## 5. Evidence and honesty

Every support claim is backed by a command that passed in this repository (`docs/evidence/`). Targets that
cannot be run here (real iOS/Android devices, Windows, Linux hardware, VoiceOver/NVDA/TalkBack sessions,
Safari/Firefox, notarization, app-store signing) stay **unqualified** in the ledger with the reason; compiling
for a target is recorded as compile-only evidence, never as qualification.

## 6. Repository layout

```
compiler/      frontend + lowering + emit (Node)         crates/    Rust workspace members (vn-*)
tooling/       CLI, build graph/cache, oracle, web glue   hosts/     macos linux windows ios android web
packages/      @vue-native/* TypeScript declarations      docs/      guide, compat, decisions, evidence, dev
tests/         language vue ui platform security fixtures examples/  counter quick-quote fieldboard
experiments/   gate experiments                            release/   licenses, SBOM, update, qualification
```

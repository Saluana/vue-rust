# Architecture gate record: G1 and G2 (task 3.8)

Status: **draft for the project owner's sign-off**. Written 2026-10-10 from the evidence listed below. Every result here was
re-run in this checkout (4 vCPU Linux container, Rust 1.99.0, Node 22) unless it says otherwise. Chrome could not run in this
container, so browser results come from GitHub CI or from earlier recorded runs. The gate definitions are `design.md` §15 (G1, G2)
and the budgets in `design.md` §14.

## Verdicts

| Gate | Verdict | Why |
|---|---|---|
| G1 language/runtime | **Fail (latency only)** | Semantics pass every oracle we have. The collector misses the 2 ms p99 slice budget at every measured heap size. |
| G2 renderer/platform | **Not evaluable** | No native renderer comparison (3.4) and no real mobile hardware (3.5). The DOM and SSR parts pass their probes. |

Per `design.md` §15 a G1 failure means "narrow language or explicitly reconsider engine product; stop broad compiler expansion",
and a G2 failure means "change renderer or narrow UI promise". This record recommends the narrower step for each, below. It does
not claim a conditional pass.

## G1 evidence

| Criterion (design §15) | Result | Evidence |
|---|---|---|
| Closures, aliases, cycles, UTF-16, coercion, exceptions match the oracle | Pass | Language 79/79 and lowering 166/166 against Node goldens; 232/232 runnable Test262 files in the selected subset pass, 0 fail, 75 rejected by the profile and listed (`docs/evidence/test262-subset.md`) |
| Promises and modules | Pass | 115/115 Vue scheduling scenarios; 14/14 module projects including literal `import()` (`docs/evidence/dynamic-import.md`) |
| Same results on wasm32 | Pass | 79/79 and 166/166 on wasm32, byte-identical to native (`docs/evidence/wasm-execution.md`) |
| Author code needs no Rust edits | Pass | Counter and Quick Quote build and run headless and on the web from `.vue` only |
| Rooted GC passes stress | Pass | Churn workload checked against an independent reachability model; GC at every safepoint leaves all corpora unchanged (`docs/evidence/gc-latency.md`) |
| Rooted GC passes the pause budget (p99 ≤ 2 ms, §14) | **Fail** | One release run: p99 6.8 ms at 10k live objects, 16.5 ms at 100k, 206 ms at 1M. The collector is non-incremental mark-sweep. |
| Scope-owned cancellation (3.3) | Pass | Stale completions are discarded after unmount; ordinary promises keep JS behaviour (`docs/evidence/scope-cancellation.md`) |
| Engine-backed cost comparison (3.1) | Not done | No engine-backed variant was built; sizes of our own artifacts are in `docs/evidence/aot-size-report.md` |

The design does not define the "reference workload" for the pause budget. The two demo apps peak at about 950 and 2,070 live
objects, where pauses are far below 2 ms. The failure matters for large apps (Fieldboard's 100k records).

**Recommendation for G1:** keep the AOT value runtime (semantics are sound and the corpora are green) and narrow the claim: state a
supported live-heap ceiling, about 10k objects at present, and keep the latency gate open. Make an incremental or generational
collector the next runtime work package, before broad feature expansion. The gc-arena experiment
(`experiments/heap/gc-arena-churn/`) also misses 2 ms at 1M objects, so switching collectors alone does not pass the gate.

## G2 evidence

| Criterion | Result | Evidence |
|---|---|---|
| Native form/list/overlay on a candidate renderer (3.4) | Not started | Needs a desktop display and a renderer spike; not possible in this container |
| Real mobile IME/AT/input (3.5) | Blocked | Needs iPhone/Android hardware |
| DOM slice | Pass (Chrome only) | Browser suite in CI; Counter and Quick Quote run in Chrome |
| SSR identity and hydration probe (3.6) | Pass for the probe subset, headless | Rust SSR output byte-identical to `@vue/server-renderer` for 17 cases; hydration adopts nodes without creating any; input edited before hydration survives (`docs/evidence/ssr-hydration-probe.md`). Browser hydration needs WebHost support that does not exist yet. |
| Tooling path (3.7) | Partial | TypeScript 6.0.3 + vue-tsc 3.3.12 pinned, TypeScript 7 excluded; uncaught throws (sync and after an await) map to `.vue` lines; `vue-native dev` restarts with a plain-data snapshot in about 1.5–2.2 s per edit for the demos. Editor/LSP behaviour is not tested (`docs/evidence/tooling-reload-probe.md`). |

**Recommendation for G2:** the gate cannot be decided until 3.4 and 3.5 run on real desktop and mobile hardware. Until then, scope
any vertical-slice claim to web DOM plus headless, and do not start native renderer work beyond the 3.4 spike.

## Pins and interfaces selected
- Frontend: official `@vue/compiler-sfc` 3.5.43 outputs, lowered through HIR schema hash `75101c33…` (hirVersion 3).
- Type tooling: TypeScript 6.0.3 + vue-tsc 3.3.12 (@vue/language-core 3.3.12).
- Runtime: `vn-value` non-moving generation-checked heap, collection only at safepoints.
- Rejected for now: gc-arena (no pause advantage at 1M objects; branded lifetimes do not fit the generated-code ABI; see its README).

## Open decisions for the owner
1. Accept the narrowed G1 claim (live-heap ceiling) and schedule incremental collection, or stop compiler expansion entirely.
2. Name the reference workload for the 2 ms budget.
3. Arrange desktop and mobile hardware for 3.4 and 3.5, or narrow the UI promise to web + headless.

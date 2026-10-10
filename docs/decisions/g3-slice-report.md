# G3 report: first vertical slice (task 4.5)

Status: **draft for the project owner's sign-off**, written 2026-10-10. Gate definition: `design.md` §15 (G3 row) and the
"smallest sensible first implementation milestone" paragraph below it. Measurements come from a 4 vCPU Linux container shared with
other jobs; treat them as indicative, not as device qualification.

## Verdict: **not passed**

The web DOM and headless halves of the slice work end to end. G3 also requires the same quote form on a **desktop native** host,
operated by keyboard and screen reader, and it sits behind G1/G2, which are not passed (`docs/decisions/architecture-gate.md`).
Neither the native desktop host nor any accessibility adapter exists yet. Per §15 the failure decision is "fix systemic gap
before inventory expansion".

## Milestone checklist

| Milestone item (design §15) | State | Evidence |
|---|---|---|
| Web DOM quote form (Quick Quote) | Done, Chrome only | `tests/platform/web/e2e-vue-apps.test.mjs` in CI; `docs/evidence/resume-web-apps.md` |
| Desktop native quote form | **Missing** | No native window renderer or desktop host (task 4.4) |
| Keyboard / screen-reader operation | **Missing** | No accessibility adapter |
| One reusable field component, cached computed total, watcher with cleanup, two-item keyed list, unmount/recreate | Done | Component ownership tests: independent instances, computed evaluated once, keyed-child removal stops its watcher, 50 unmount/recreate cycles return every count to baseline (`docs/evidence/component-ownership.md`) |
| Captured mutable alias fixture | Done | Lowering corpus 166/166 natively and on wasm32 (`docs/evidence/wasm-execution.md`) |
| Async cancellation fixture | Done | Scope-owned operations, unmount-during-request (`docs/evidence/scope-cancellation.md`) |
| TypeScript diagnostic mapping | Done | `vue-native check` maps vue-tsc errors to `.vue` lines (VN7001) |
| One async failure maps to `.vue` | Done (headless) | `tests/cli/source-frames.test.mjs` (`docs/evidence/tooling-reload-probe.md`) |
| Text/number validation | Done | Quick Quote interaction scripts, headless and in Chrome |
| Counter as regression fixture; old `@[click]` defect fixed | Done | `tests/compiler/sfc/dynamic-event-arg.test.mjs` (`docs/evidence/ir-regression.md`) |
| Build without editing Rust | Done | `vue-native build` for headless and web from `.vue` sources only |

## Measured costs (release builds, headless unless stated)

| Metric | Counter | Quick Quote | Source |
|---|---|---|---|
| main() to mounted, p50 / p99 | 1.39 ms / 3.03 ms | 3.39 ms / 8.82 ms | `docs/evidence/slice-measurements.md` (100 rounds) |
| Peak RSS at startup, p50 | 5,224 KiB | 6,104 KiB | same |
| Event dispatch to drained queue, p50 / p99 | click 63 µs / 155 µs | input 234 µs / 783 µs; toggle click 1.36 ms / 2.88 ms | same; headless lower bound, no layout or paint |
| Stripped native binary | 2.86 MB | 3.08 MB | `docs/evidence/aot-size-report.md` |
| Web app wasm, raw / gzip | 1.40 MB / 461 KB | 1.51 MB / 485 KB | same |
| Edit to restarted app (dev loop, types on), p50 | 1.53 s | 2.18 s | `docs/evidence/tooling-reload-probe.md` |

Against the §14 budgets: the event numbers sit far below the 50 ms key-to-visible budget, but they exclude layout, paint and
compositing, so they do not qualify it. Startup and memory budgets need the release native host on real hardware.

## Outstanding limitations
- Native desktop host, renderer and accessibility (4.4); real devices (3.5); renderer comparison (3.4).
- Browser qualification is Chrome only; hydration in the browser is not wired (3.6).
- Collector pauses exceed the 2 ms p99 budget above roughly 10k live objects (3.2).

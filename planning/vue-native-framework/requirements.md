# Requirements: Vue native framework

## 1. Overview

Plan date: **October 8, 2026, America/Los_Angeles** (research also falls on October 9 UTC). This is a production framework proposal, not an implementation or a claim of compatibility. Developers author Composition API `.vue` components in JavaScript or TypeScript; accepted application logic compiles through Rust to native desktop/mobile binaries and WebAssembly. Ordinary application development must not require Rust.

**Feasibility verdict:** feasible as a substantial compiler/runtime/UI-toolkit project with an explicit compatibility profile. Arbitrary Vue/JavaScript/npm/CSS compatibility, native machine-code compilation and a small translation-only implementation cannot all be promised. Proceed only after experiments establish JavaScript semantics, memory management, accessible rendering and mobile text input. Recommend preserving the official parser frontend, replacing the direct emitter, and narrowing the public compatibility claim to features that pass behavioral qualification. A JavaScript engine is a useful comparison and oracle, but is not an undisclosed substitute for compiling application logic.

### Context and inspected evidence

The actual repository is **JavaScript + generated Rust**, not the Go/SQLite/CLI architecture mentioned in the generic planning template. There is no existing database, Go module, multi-platform service layer, production CI, or framework package structure. `compiler/compile.mjs` (143 lines) directly emits Rust from Vue SFC/template ASTs and Babel expressions. `compiler/cli.mjs` generates an eframe project and refuses an existing output directory. `package.json` pins Vue compiler packages **3.5.43** and Babel parser **7.29.9**; `generated/Cargo.toml` pins eframe **0.36.2**, Rust edition 2024. `generated/` contains the counter and four Rust tests.

`/workspace/shared/vue-rust-prototype` was unavailable. The supplied source was found at `/Users/brendon/Downloads/quick-quote-source.zip`, inspected and extracted into a temporary directory, without merging it into this repository. It contains a newer compiler, `examples/Quote.vue`, `quote-app/`, 28 JavaScript tests and four quote arithmetic/state tests. It adds numeric drag controls, bounded decimal formatting, a separator and padded scrolling. It has no persistence, file export, validation workflow, or crash recovery. Quote ZIP SHA-256: `470e78787014e2dbbdc8fdf9cf2107fc48caa574d8c02aa59823689e9bba3a0e`; workspace compiler SHA-256: `24224cdbcc536bfcbaa970bd4595190c974bc26998fc3f8d814685064d31afad`.

**Fresh evidence:** the existing 25 counter compiler tests and 28 ZIP compiler tests passed under Node **24.21.0**, using temporary copies and cached, locked dependencies with install scripts disabled. Compiler tests largely inspect generated strings or assert rejections. The four counter Rust tests cover state, instance separation, paint shapes and synthetic button clicks; the four quote Rust tests cover arithmetic and state separation. Cargo/rustc were unavailable here, so these Rust tests and platform builds were **not rerun**. Bundled logs report Linux/window and WASM results; these are historical reports, not fresh platform qualification. Screenshots are not evidence of semantics, accessibility or other targets.

One fresh probe confirms a rejection gap: `<button @[click]="inc">` is accepted as a fixed click handler even though `click` is not resolved. The compiler ignores the argument's dynamic/static distinction. Other source-visible risks include Rust float display for interpolation, Rust `format!` for Quick Quote `toFixed`, and uncached computed getters. No product fixes are made by this plan.

### Reference versions and assumptions

- **Vue 3.5.43** is the exact stable semantic reference, independently verified from the [official npm package metadata](https://registry.npmjs.org/vue/latest) on the plan date. Pin compiler, runtime oracle and SSR oracle to the same patch. Current documentation can contain newer APIs; the implementation must reconcile the [official API inventory](https://vuejs.org/api/) against this pinned package before claiming any row. Vue 3.6 release candidates/Vapor are excluded from the stable promise.
- TypeScript's current npm stable is **7.0.2**, verified from [package metadata](https://registry.npmjs.org/typescript/latest). TypeScript 7.0 has no stable programmatic API; Microsoft's [release announcement](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/) specifically identifies Vue/Volar tooling constraints. Recommend a separately pinned TypeScript 6 compatibility API for SFC tooling, with TypeScript 7 CLI comparison for ordinary modules. The precise TypeScript 6/Vue language-tools pair remains an early gate; do not invent a compatible patch.
- The language profile is a documented subset of standardized JavaScript, using ES2025 as its initial specification baseline. This is not a claim to implement all ES2025 or later additions. TypeScript annotations are checked and erased; they do not authorize semantic changes.
- “Native” means application functions are ahead-of-time compiled through Rust, with no embedded webview, interpreter fallback or JIT in the production native profile. A managed value heap is allowed and disclosed. Custom-drawn native UI is acceptable; it must not be marketed as platform-owned widgets.
- Full implementation means the complete production program for the declared profile across all six targets, including explicitly unsupported features, blocking experiments and maintenance. It does not mean every JavaScript or browser feature must be implemented.
- No external servers, accounts or subscription services are assumed. A server-rendering library/adapter is planned, not a hosted service. Multi-user auth, cloud synchronization and an untrusted plugin marketplace are outside the framework baseline.
- Platform versions, renderer pins, toolchain pins and budget thresholds below are proposed qualification choices until tested. No schedule, performance result or app-store approval is guaranteed.

### Scope boundaries

Exclude Options API, arbitrary runtime code evaluation, remote code download, arbitrary CommonJS/Node APIs, unsupported CSS, and automatic compatibility with npm packages. Runtime render functions, custom elements, experimental Suspense/Vapor and general user-authored Proxy/prototype mutation are explicitly excluded from the first production profile. Target-specific extensions must be explicit and type checked. This task creates exactly three Markdown documents; it does not edit product code, publish, purchase, or create accounts.

## 2. Requirements

Each requirement owns a test family `Txx`, matching its number. A family contains independently meaningful behavior cases, not a single broad smoke test. All acceptance criteria apply to the eventual qualified release unless a gate or target limitation is stated.

### R01 — Evidence and compatibility governance

**Engineering objective:** maintain an auditable distinction between prototype behavior, proposed support and qualified support.

- **R01.AC1:** Every public API, compiler macro, template directive, SFC feature and TypeScript category in the reference inventory SHALL have a status, target scope, limitation and behavioral test owner.
- **R01.AC2:** A support claim SHALL identify source commit, dependency lock hashes, reference versions, build mode, target/OS/device and passing evidence; failed or missing evidence SHALL remain visible.
- **R01.AC3:** The original counter and ZIP SHALL remain identifiable fixtures; changes to semantics SHALL include migration notes rather than retroactively rewriting baseline claims.

### R02 — JavaScript semantics without Rust ownership exposure

**User story:** an author writes supported ordinary JavaScript and receives JavaScript behavior without managing Rust lifetimes.

- **R02.AC1:** Accepted closures, mutation, aliasing, object identity, null/undefined, coercion, numeric/string operations, arrays, maps, sets, exceptions, promises and modules SHALL match the pinned JavaScript/Vue oracle on the declared cases.
- **R02.AC2:** Unsupported constructs SHALL fail at their source span with a diagnostic code, explanation and supported alternative; there SHALL be no implicit interpreter or ownership-driven copy that changes identity.
- **R02.AC3:** Cyclic reachable values SHALL retain identity; unreachable cycles SHALL be collected; disposed UI scopes SHALL not remain alive solely because of scheduler subscriptions.

### R03 — SFC and TypeScript authoring

**User story:** authors use `.vue`, `<script setup>`, imports and useful TypeScript types.

- **R03.AC1:** Supported blocks, macros, type imports, generics, props/emits/slots types and annotations SHALL produce source-level diagnostics and equivalent JS/TS runtime results.
- **R03.AC2:** Unsupported block languages, runtime TypeScript constructs and macro placements SHALL be rejected before Rust generation, including in transitive dependencies.
- **R03.AC3:** Type assertions and `any` SHALL never bypass runtime capability validation or introduce unchecked native casts.

### R04 — Reactive values and effects

**Engineering objective:** implement the promised reactivity APIs rather than getters that merely look reactive.

- **R04.AC1:** Deep/shallow/readonly objects, refs, utilities, collections and raw/proxy identity SHALL match differential cases including array length, deletion and iteration tracking.
- **R04.AC2:** Computed values SHALL be lazy, dependency cached, invalidated correctly and tested for branching dependencies, writable forms and unchanged results.
- **R04.AC3:** Scopes, custom refs and cleanup SHALL release subscriptions exactly as the reference requires; no undocumented global effect registry may retain disposed apps.

### R05 — Scheduling, watchers and asynchronous work

**Engineering objective:** preserve observable update ordering while making host work cancellable.

- **R05.AC1:** Watch variants, cleanup, deep depth, once/immediate, pause/resume/stop, pre/post/sync flush and `nextTick` SHALL match recorded reference traces.
- **R05.AC2:** Promise continuations SHALL preserve JS ordering and rejection behavior; native futures SHALL not independently mutate reactive state off the UI execution lane.
- **R05.AC3:** Scope-owned service operations SHALL cancel on disposal; stale completion IDs SHALL be rejected. Ordinary explicitly retained JS closures SHALL retain their documented JS behavior, while disposed render/effect handles cannot update UI.

### R06 — Component model

**User story:** authors compose reusable components with props, emits, slots, models and injection.

- **R06.AC1:** Instance state, prop defaults/validation, fallthrough attributes, emitted events, scoped slots, model modifiers and exposed refs SHALL pass parent/child behavioral cases.
- **R06.AC2:** Mount/update/unmount, activation/deactivation, error capture, provide/inject and template-ref lifetimes SHALL have explicit ordering and isolation tests.
- **R06.AC3:** Keyed identity, dynamic component replacement and cache eviction SHALL preserve or dispose the correct instance, focus and owned resources.

### R07 — Template behavior

**User story:** accepted templates render and interact predictably across hosts.

- **R07.AC1:** Interpolation, binding, events, models, conditionals, lists and supported built-ins SHALL have individual positive, edge and unsupported-form cases.
- **R07.AC2:** Text SHALL be escaped as text; native raw HTML SHALL fail explicitly. Web raw HTML SHALL require an explicit trusted-content extension without silently altering Vue semantics.
- **R07.AC3:** DOM-specific directives/events SHALL either have an explicit web-only contract or fail with target-specific diagnostics.

### R08 — Layout, styling and media

**User story:** familiar markup and documented CSS produce usable interfaces.

- **R08.AC1:** The published selector/property/unit/media-feature allowlist SHALL describe cascade, inheritance, scoping and variable resolution; unsupported CSS SHALL report source spans.
- **R08.AC2:** Flex/grid cases, scrolling, clipping, stacking, DPI, responsive layout, dark mode and reduced motion SHALL pass layout and interaction tests on each qualified target.
- **R08.AC3:** Text shaping, fallback fonts, bidi, images, icons and SVG SHALL respect resource limits, licensing and malformed-input tests.

### R09 — Accessibility and international input

**User story:** keyboard, touch and assistive-technology users can complete the reference app workflows.

- **R09.AC1:** Controls SHALL expose accurate roles/names/values/states/actions; focus order, modal focus return, validation announcements and keyboard operation SHALL pass screen-reader scenarios.
- **R09.AC2:** CJK IME, dead keys, combining text, emoji, RTL editing, selection, clipboard, drag/drop and mobile keyboards SHALL work without committing partial composition or losing selection on reactive updates.
- **R09.AC3:** Web SHALL meet tested WCAG 2.2 AA criteria for reference flows; native SHALL pass platform assistive-technology and large-text checks. Automated tree snapshots alone SHALL not qualify accessibility.

### R10 — Compiler architecture and diagnostics

**Engineering objective:** create inspectable, versioned compilation stages with precise failures.

- **R10.AC1:** Parsing, binding, semantic validation, type checking, lowering and generation SHALL preserve `.vue` source locations, including Unicode offsets and imported files.
- **R10.AC2:** Generated code SHALL be deterministic for identical declared inputs; mismatched IR/runtime contracts SHALL be rejected before execution.
- **R10.AC3:** Normal author mistakes SHALL be explained at `.vue`/module spans; internal codegen failures SHALL carry a framework bug classification and source correlation.

### R11 — Developer workflow and builds

**User story:** authors create, check, preview, debug, build and upgrade projects through one documented CLI.

- **R11.AC1:** A clean supported machine SHALL build both reference apps using a locked setup; an already populated dependency mirror SHALL permit an offline build.
- **R11.AC2:** Incremental invalidation SHALL include code, imported types, styles, assets, config, target and compiler/runtime versions; stale output SHALL never be reused after an input change.
- **R11.AC3:** Hot reload SHALL preserve only compatible explicitly restorable state, dispose old effects and clearly report resets; editor and stack traces SHALL point to author sources.

### R12 — Ecosystem compatibility

**Engineering objective:** qualify dependencies individually.

- **R12.AC1:** Pure modules, Vue composables, router/state libraries, plugins, DOM packages and native addons SHALL receive distinct compatibility classifications and exact tested versions.
- **R12.AC2:** The resolver SHALL analyze transitive code and conditional exports; browser/Node globals or unsupported syntax SHALL fail before packaging.
- **R12.AC3:** No Pinia, Vue Router, VueUse, UI-kit or arbitrary npm support claim SHALL be made solely because an import compiles.

### R13 — Windows

**User story:** Windows authors can distribute an installable native application.

- **R13.AC1:** Proposed x64 and ARM64 Windows targets SHALL pass real-machine debug/release UI, accessibility, IME, multi-window, install and crash tests independently.
- **R13.AC2:** Signed MSIX and a documented unpackaged development path SHALL verify trust, deep-link registration, update and uninstall behavior.

### R14 — macOS

**User story:** macOS authors can distribute a native application with expected desktop integration.

- **R14.AC1:** Apple Silicon and Intel builds SHALL pass on actual matching hardware, including VoiceOver, menus, IME, multiple windows and file access.
- **R14.AC2:** Developer ID/notarized distribution and an independently qualified sandboxed Store profile SHALL have distinct entitlement and update documentation.

### R15 — Linux

**User story:** Linux users can install and operate the app on the declared desktop environments.

- **R15.AC1:** x64/ARM64, Wayland/X11, GNOME/KDE and declared GPU paths SHALL be independently listed in evidence; unsupported combinations SHALL not inherit support from compilation.
- **R15.AC2:** Flatpak permissions/portals, AT-SPI/Orca, fonts, clipboard and clean installation SHALL pass on declared distro baselines.

### R16 — iOS/iPadOS

**User story:** iPhone/iPad users receive a native, accessible, restorable application.

- **R16.AC1:** ARM64 devices and Apple Silicon simulators SHALL build through pinned Xcode/SDKs; actual iPhone/iPad testing SHALL include VoiceOver, dictation, IME, safe areas, rotation and keyboard geometry.
- **R16.AC2:** Suspension, termination, restoration, permissions, signing and distribution validation SHALL pass without runtime code download or assumptions of continuous background execution.

### R17 — Android

**User story:** Android users receive a native app that survives activity and process lifecycle changes.

- **R17.AC1:** ARM64 device and x64 emulator builds SHALL have separate qualification, including TalkBack, IME, back navigation, safe areas and surface loss.
- **R17.AC2:** Activity recreation, process death, permission revocation, signed APK/AAB and update key continuity SHALL pass on real devices from at least two vendors.

### R18 — Web, SSR and hydration

**User story:** web deployments remain accessible, linkable and useful to crawlers and users before WASM loads.

- **R18.AC1:** The default web target SHALL use semantic DOM controls with Rust/WASM application logic, functioning keyboard/IME input and browser-native accessibility.
- **R18.AC2:** A production SSR/SSG mode SHALL emit meaningful HTML, metadata, status codes and links; hydration SHALL preserve identity, typed input and stable IDs without cross-request state leakage.
- **R18.AC3:** Browser CORS, quota, storage eviction, worker restrictions, WASM loading failure and offline asset/version mismatches SHALL have observable recovery paths. Canvas-only output SHALL remain an explicit experimental target.

### R19 — Networking and asynchronous services

**User story:** applications use HTTP, streams and WebSockets through cancellable, bounded APIs.

- **R19.AC1:** Timeouts, cancellation, retries, redirect policy, backpressure and connection loss SHALL behave consistently where platform capabilities permit.
- **R19.AC2:** Host differences in cookies, headers, TLS, CORS and background execution SHALL be typed/documented; browser restrictions SHALL never be silently bypassed.
- **R19.AC3:** Stream buffers, redirects, response sizes and retry counts SHALL have finite defaults and tested overflow behavior.

### R20 — Files, persistence and recovery

**User story:** applications safely save, reopen, migrate and export data offline.

- **R20.AC1:** File access SHALL use scoped handles; denied/revoked permission, path escape, disk-full and interrupted saves SHALL produce recoverable errors without overwriting valid data.
- **R20.AC2:** SQLite migrations on native and explicitly selected web persistence SHALL be versioned, transactional where supported and tested from all supported prior schemas, including interruption/recovery.
- **R20.AC3:** Settings, secure storage and ordinary databases SHALL remain distinct; platforms without equivalent secure storage SHALL report that limitation rather than store plaintext under a secure API name.

### R21 — Host integration and capabilities

**User story:** authors use navigation, deep links, notifications, menus, dialogs and background work without writing platform code.

- **R21.AC1:** Capabilities SHALL map to platform permission/entitlement mechanisms with typed denied/unavailable/revoked states.
- **R21.AC2:** Window, application, request and component scopes SHALL be distinct; sharing data between windows SHALL be explicit.
- **R21.AC3:** Deep links SHALL validate routes/parameters; notifications and background jobs SHALL respect OS scheduling and user consent, with no guaranteed wake time.

### R22 — Security

**Engineering objective:** separate hostile build inputs, trusted app code and hostile runtime data.

- **R22.AC1:** Compiler/package processing SHALL enforce resource limits and filesystem boundaries; project code/build scripts SHALL require a trusted build context and never inherit signing secrets in untrusted CI.
- **R22.AC2:** Unsafe/FFI boundaries, native bridge requests, network policy and decoded assets SHALL pass malformed-input and misuse tests.
- **R22.AC3:** Signed release/update verification, provenance, revocation and rollback protection SHALL be exercised with tampered, expired, replayed and interrupted artifacts.

### R23 — Performance and resource budgets

**Engineering objective:** qualify predictable performance with measurements.

- **R23.AC1:** Startup, memory, size, input, frames, lists, editing, reactivity and builds SHALL be measured on named hardware in release mode against documented baselines.
- **R23.AC2:** Every proposed budget SHALL have a reproducible workload, sample count, statistic and waiver/rejection rule; no unmeasured speed superiority claim is permitted.
- **R23.AC3:** Sustained mobile tests SHALL report thermal and battery behavior; idle apps SHALL avoid continuous redraw and polling.

### R24 — Test and qualification infrastructure

**Engineering objective:** use independent behavioral evidence at the correct boundary.

- **R24.AC1:** Unit, differential, property, golden, fuzz, integration, UI, accessibility, crash, install and real-device suites SHALL have named owners and reproducible artifacts.
- **R24.AC2:** Debug/release, target architecture, reference dependency upgrades, offline builds and signing/update paths SHALL appear in a platform/build-mode matrix.
- **R24.AC3:** Every known prototype bug SHALL get a regression reproducing the old failure; successful code generation or screenshots SHALL not substitute for execution assertions.

### R25 — Licensing, releases and maintenance

**Engineering objective:** make support and redistribution obligations sustainable.

- **R25.AC1:** Release artifacts SHALL include dependency/asset license inventory, notices, SBOM, checksums, provenance and explicit framework/Vue compatibility versions.
- **R25.AC2:** Naming/trademark review, support tiers, deprecations, security reporting, key rotation and abandoned-dependency response SHALL be documented before public production release.
- **R25.AC3:** A reproducible unsigned payload SHALL be compared independently; signing/notarization variability SHALL be recorded rather than misrepresented as bit reproducibility.

### R26 — Reference apps and decision gates

**User story:** framework promises are demonstrated by real applications without app-specific compiler branches.

- **R26.AC1:** Quick Quote SHALL prove forms, calculations, accessible validation, offline persistence, export and crash recovery; its expected arithmetic SHALL be independently specified.
- **R26.AC2:** A separate demanding app SHALL prove large lists, navigation, asynchronous streaming/cancellation, reuse and complex state across targets.
- **R26.AC3:** Compiler, memory, renderer and mobile experiments SHALL have explicit go/no-go criteria before platform-wide implementation; the first milestone SHALL make no production claim.

## 3. Non-functional constraints

- **Determinism:** deterministic compiler/IR/style output and repeatable scheduler traces for identical inputs. Real clocks, randomness, IO and OS scheduling remain external inputs, not fake deterministic promises.
- **Resources:** no implicit unbounded log/history/stream retention; configurable compilation depth/size limits, queue caps, asset decoding caps and watchdogs. Guard failures must report resource exhaustion, never silently drop semantically required updates. Collection latency is an architecture gate.
- **Isolation:** one application heap/reactive root per native process by default; window scopes within it; explicit app stores for sharing. SSR uses request-local roots. No retrieval, embeddings, vector search, FTS or chat-session database is needed.
- **Data:** there is no existing SQLite schema to migrate. New persistence adapters must declare schema/version policy; neither generated UI code nor reactive heap serialization is a database schema. Backups and old-version readability govern rollback.
- **Safety:** generated source escaping, archive path validation, URL policy, bounded compiler work, secret redaction, audited native handles and permissions are required. Safe Rust reduces memory-safety risks; it does not guarantee JavaScript correctness, secure FFI, sandboxing or authorization.
- **Approachability:** supported `.vue` programs must never require authors to diagnose borrow-checker errors or edit generated Rust. Unsupported semantics are product boundaries, not developer ownership exercises.

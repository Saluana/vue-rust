# SSR and hydration identity probe (task 3.6)

Ticket w36-ssr. Worktree /home/claude/wt/w36-ssr, branch claude/w36-ssr. Nothing is committed. This is a probe of a subset. It does not close task 3.6: the browser run is still missing, and the hydration path does not yet run in the browser host.

## What was built

tests/ssr-oracle/probe.mjs (116 lines) is the oracle. Part (a) renders 14 probe components with the real @vue/server-renderer 3.5.43 renderToString and prints one line per probe. It also prints two renders of a module-level counter and one root comment, 17 lines in all. Part (b) lists hydration behaviour of vue@3.5.43 with line references into node_modules/@vue/runtime-core/dist and node_modules/@vue/runtime-dom/dist. Part (b) is printed, not executed: Vue's hydration needs a DOM, the repository has no jsdom, and no package may be added.

crates/runtime/src/ssr.rs (433 lines) is a string renderer over the runtime's vnode objects, following the vnode path of @vue/server-renderer 3.5.43. It escapes text and attributes as escapeHtml does, strips comment delimiters as escapeHtmlComment does, writes fragments between `<!--[-->` and `<!--]-->`, writes an empty comment as `<!---->`, and applies the attribute and applySSRDirectives rules, including the value of a v-model text input. The entry points are ssr::render_to_string (a vnode) and ssr::render_app_to_string (an app). A dedicated renderer was needed: the DOM serializer (crates/dom/src/serialize.rs) leaves `'` and text `"` unescaped and writes no fragment comments, so it cannot produce the oracle bytes.

crates/runtime/src/hydrate.rs (518 lines) ports createHydrationFunctions of runtime-core 3.5.43, production build: hydrate, hydrateNode, hydrateElement, hydrateChildren, hydrateFragment, handleMismatch and locateClosingAnchor. It reads the DOM through Host::first_child, next_sibling, node_type and node_value, and it records each recovered mismatch in the realm's log. A component's first mount hydrates its subtree while the renderer is in hydration mode (component.rs). createSSRApp(...).mount(container) keeps the server markup (app.rs). vModelText has a getSSRProps hook (vmodel.rs). The reactive style branch of hydrateElement (prod:1683-1684) is ported.

Changes to shared surfaces:

- host.rs: first_child, node_type and node_value are defaulted methods of the Host trait. Their defaults return HostError::Unsupported, so a host without them fails loudly. Every host implements this interface (docs/dev/ARCHITECTURE.md:62-66), so the additions need maintainer sign-off.
- host_doc.rs: DocHost implements the three. set_element_text now gives a handle to each text node it creates (the host could not see those nodes before) and counts them. DocHost::created_nodes() exposes the count to the identity tests.
- renderer.rs: public hydrate_into and hydration_mismatches; hydrating and mismatches fields in RendererData.
- component.rs: mount_component delegates to create_instance_for; a first mount hydrates in hydration mode.
- app.rs: a createSSRApp export. vmodel.rs: getSSRProps on vModelText. lib.rs: two module declarations.
- crates/runtime/tests/ssr_hydration.rs (1188 lines, 21 tests). Expected HTML is copied from the oracle output; the file header and the ORACLE comment name the script and its command.

## Commands and results

Run from /home/claude/wt/w36-ssr with CARGO_TARGET_DIR=/home/claude/targets/w36-ssr, on the final tree:

- `cargo test -p vn-runtime --test ssr_hydration`: 21 passed, 0 failed.
- `cargo test -p vn-runtime -p vn-dom --no-fail-fast`: exit 0, 171 passed, 0 failed, 0 ignored (summed over all test binaries and doc-tests). The 21 new tests are included; the 150 existing tests are unchanged and pass.
- `cargo test -p vn-renderer-dom -p vn-uikit --no-fail-fast`: exit 0, 22 passed, 0 failed.
- `node tooling/harness/run-language.mjs --with-vue --ui`: exit 0, "scenarios 174: pass 174, FAIL 0, blocked 0, compile-error 0; 0 unexpected". The --ui path compiles the scenarios and compares their documents; it starts no browser.
- `node tests/ssr-oracle/probe.mjs`: exit 0, 17 part (a) lines, "# probes rendered: 14".
- A check run by hand: the ORACLE table in ssr_hydration.rs equals the part (a) output of the oracle, 17 of 17 rows, in order.
- A scan of the changed Rust for unsafe, #[ignore], dbg!, println! and todo!: no matches.

rustfmt is not installed for the pinned 1.99.0 toolchain, and the repository has no rustfmt.toml. The stable toolchain's rustfmt 1.9.0 also flags the existing HEAD sources (48 hunks in renderer.rs, 34 in vmodel.rs), so no formatting pass was applied. Clippy was not run: docs/dev/AGENT-CONTRACT.md lists no clippy for this environment, and CI has no clippy step.

## Negative controls

- Byte comparison (comparator_rejects_mutated_expectations): five mutants of the static-tree line (an unescaped `'`, an unescaped `"`, an unescaped `&`, `<br/>` for `<br>`, a trailing newline) and two of keyed-list (a missing `<!--]-->`, and `<!---->` in place of `<!--[-->`) must fail compare_html. They do.
- Identity (identity_check_fails_when_mounting_recreates_the_server_nodes): createApp(...).mount("#app") over server markup recreates the nodes. same_nodes and kept_elements both reject that, and the creation count rises.
- Mismatch log: each mismatch test asserts the exact log entries, and the clean-markup tests that check the log (the probe adoption, the adjacent-text split, the hoisted element) assert an empty log. The log check therefore cannot pass vacuously.
- Realm isolation: hydration_logs_stay_in_their_realm.
- Unsupported constructs: each of the five constructs must throw a TypeError with the expected message in both phases. A successful render or hydration fails the test.
- Style branch: I disabled the STYLE arm (`false && patch & PatchFlags::STYLE != 0`). hydration_reads_the_keys_of_a_reactive_style_only_when_the_flag_has_style then failed with "flag 4: runs after a change to one style key, left: 1.0, right: 2.0". I restored the file and confirmed it matched a backup byte for byte.
- First full run: eight tests failed. The causes were: text created by textContent had no host handle, so the host could not see it (fixed in host_doc.rs); a helper matched the container itself (fixed in the helper); a child fixture declared setup with length 0, so Vue's setup context was never created (fixed in the fixture); the Suspense stand-in lacked normalize, which createVNode calls (fixed in the fixture); and createStaticVNode always throws here (the test now asserts that, and the hoisted-element test builds a patchFlag -1 vnode by hand). A comment-escaping bug (a `-->` advanced four bytes) was fixed with explicit delimiter lengths, and the comment-escape probe checks it against the oracle.

## Verified citations

The runtime-core and runtime-dom line citations were checked against the installed 3.5.43 builds, most of them line by line. Two ranges in probe.mjs were corrected: the vnode.el assignment is at runtime-core.cjs.prod.js:1443 (the range given was 1430-1437), and the hydrateChildren range for a missing DOM node now runs to 1744, which covers the patch(null, vnode, container) call at line 1742. The vModelText hooks are at runtime-dom.cjs.prod.js:1424 (created) and 1450 (mounted), and getSSRProps is at prod:1682 and dev:1784. Vue installs the SSR directive hooks through Vue.initDirectivesForSSR(), which @vue/server-renderer calls when it loads (server-renderer.cjs.js:894).

The expected HTML comes from the oracle. The style-branch expectation (two renders after a change with STYLE, one without) comes from the cited source, because Vue cannot run hydration here.

## Limits

- No browser test was written and none was run; Chrome cannot run in this container. The three-engine run is not possible here.
- Browser hydration does not work yet. crates/renderer-dom/src/web_host.rs does not implement first_child, node_type or node_value, so hydrate fails there with an unsupported-host error for firstChild. That follows from the trait default in host.rs and is not executed by any test here. Implementing them touches hosts/web/ABI.md and hosts/web/vn-host.js.
- The hydration tests run on DocHost. The server markup is read by a test parser (parse_markup) that decodes the entities the renderer writes, not by a browser's HTML parser.
- Hydration identity covers 13 of the 17 oracle rows. attr-edge and comment-escape are compared as SSR output only, and the module-counter rows are checked by the SSR tests only.
- Unsupported constructs throw a TypeError "unsupported: X in SSR" or "unsupported: X in hydration": Teleport, Transition, KeepAlive, Suspense, async components, scoped slot ids, SVG and MathML (SSR), style arrays and CSS variables in style objects (SSR), functional components (SSR) and compiled ssrRender functions (SSR). Static (hoisted) vnodes cannot be created in this runtime, so hydration never meets one.
- These errors have no diagnostic code and no source span. AGENT-CONTRACT.md asks for both. The runtime's existing "unsupported:" errors have neither, so this matches the runtime, not the contract.
- Not reproduced: production's single console.error("Hydration completed but contains mismatches.") per realm (runtime-core prod:1393); the port records each mismatch in the realm log instead. The development-only warning for an empty container (dev runtime-core.cjs.js:1925) is also absent; production mounts silently, as the port does. The development warning for an unsafe attribute name (shared.cjs.js:355, server-renderer.cjs.js:64) is absent too; both builds skip the attribute, as the port does.
- Not ported: data-allow-mismatch; the template-node exemptions (isTemplateNode, prod:1466-1470 and 1517), since DocHost has no template content and no test uses one; provide of ssrContextKey; serverPrefetch and compiled ssrRender functions.
- getSSRProps is installed by the vModelText definition in every realm. Vue installs it when @vue/server-renderer loads. The subset's output is unaffected.
- Schema hash validation (planning/vue-native-framework/design.md:496) is not implemented; the hydration payload is the server markup alone.

## planning/vue-native-framework/design.md:496

design.md says hydration "adopts DOM including edits made before load". For a text input, Vue 3.5.43 does that: vModelText.created records the server value as the initial value, and vModelText.mounted keeps the edited DOM value and assigns it to the model (runtime-dom.cjs.prod.js:1424 and 1450-1458). The probe checks this. After hydration the typed value is still in the input, the model holds it, and the paragraph shows it once the queued update runs; the server value attribute stays "Hello". The sentence is therefore consistent with Vue for text inputs. The textarea path is the same in Vue but is not tested.

For checkbox, radio and select, Vue 3.5.43 does not keep the edit. vModelCheckbox.mounted calls setChecked (prod:1514), vModelRadio.created sets checked from the model (prod:1537), and vModelSelect.mounted calls setSelected (prod:1579-1581). The port implements those hooks (vmodel.rs), but no test hydrates those controls. This needs a maintainer decision.

## Requests to the maintainer

1. Sign off on the three defaulted Host methods (first_child, node_type, node_value) and on the DocHost changes, including the handle and creation count in set_element_text.
2. Decide the WebHost implementation and its ABI (hosts/web/ABI.md, hosts/web/vn-host.js) before hydration can run in a browser.
3. Approve the public surface: the createSSRApp export, getSSRProps on vModelText, and the Rust items ssr::render_to_string, ssr::render_app_to_string, ssr::escape_html, ssr::escape_html_comment, renderer::hydrate_into, renderer::hydration_mismatches and DocHost::created_nodes.
4. Decide whether mismatches should also log to the console, as production does.
5. Decide the design question for checkbox, radio and select under planning/vue-native-framework/design.md:496.
6. Decide whether runtime unsupported errors need diagnostic codes and source spans.
7. Schedule hydration for Teleport, Transition, KeepAlive, Suspense and async components, if wanted.

## Open questions

- Should getSSRProps be installed only when @vue/server-renderer loads, as Vue does?
- Should the template-node exemptions be ported now, or wait for a case that needs them?

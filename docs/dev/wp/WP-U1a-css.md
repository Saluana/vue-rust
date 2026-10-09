# WP-U1a — `vn-css`: CSS tokenizer, parser, selectors, media queries, cascade, custom properties

Read `docs/dev/AGENT-CONTRACT.md`, `docs/dev/ARCHITECTURE.md` §4, and design.md §9.2 (the CSS contract: allowlist, not "all of CSS").
`export CARGO_TARGET_DIR="$VN_ROOT/target/u1a"`. Oracle: headless Chrome via `tooling/chrome/` (read its README; use
`withPage` so one browser serves many checks).
**Owned paths:** new crate `crates/css/**` (package `vn-css`, library `vn_css`; **std only, no deps, no `unsafe`**), `tests/css/**`.
No dependency on `vn-dom` — matching is generic over a trait implemented by the caller.

## Why
The native renderer implements a documented **subset** of CSS (selectors, cascade, variables, units, media queries) on top of the
DOM-like `vn-dom::Document`; the compiler validates `<style>` blocks against the same allowlist. Unsupported constructs must produce
diagnostics with spans — never be silently dropped, even though browsers would. The web target uses the browser's own CSS engine
(so Chrome is the oracle for what the supported subset *means*).

## Deliver
1. **Tokenizer** (`css::token`) per CSS Syntax Level 3 (ident, function, at-keyword, hash, string/bad-string, url/bad-url, number,
   percentage, dimension, whitespace, delim, brackets/parens/braces, colon, semicolon, comma, CDO/CDC, comments), input `&str`, with
   byte offsets and 1-based line/0-based column (UTF-16 columns) per token.
2. **Parser** (`css::parse`): `parse_stylesheet(src, &ParseOptions) -> ParseResult { sheet, diagnostics }`. Rules: style rules
   (selector list + declarations incl. `!important`, custom properties `--x: <any tokens>` preserved as token lists), `@media` (nested rules).
   Declaration values are kept as token lists (value interpretation = next WP). Error recovery follows the spec (bad declaration skipped to `;`/`}`)
   **but every recovery and every unsupported construct yields a `Diagnostic { code, message, hint, start, end }`** with codes `VNCSS1xxx` syntax,
   `VNCSS2xxx` unsupported at-rule (`@import @font-face @keyframes(not yet) @supports @container @layer @namespace @page @charset`…), `VNCSS3xxx`
   unsupported selector (`:has() :is() :where()`(decide, document) `::before/::after` any pseudo-element, `:visited :link :target :lang()`, `:nth-of-type`,
   `:not()` with complex arguments, namespaces `ns|a`, column combinator `||`), `VNCSS4xxx` nesting (`&`, nested rules) rejected.
   Max-limits as parse options with defaults (source bytes, nesting depth 32, rule count) producing a resource diagnostic.
3. **Selectors** (`css::selector`): parse a selector list into an AST; supported: type, universal, `.class`, `#id`, attribute `[a]`, `[a=v]`,
   `[a~=v]`, `[a|=v]`, `[a^=v]`, `[a$=v]`, `[a*=v]` with `i` flag, combinators ` `, `>`, `+`, `~`, pseudo-classes `:hover :active :focus
   :focus-visible :focus-within :disabled :enabled :checked :first-child :last-child :only-child :nth-child(an+b [of S]? no — plain an+b, odd, even)
   :nth-last-child() :empty :root :not(<compound or simple list>)`. Specificity `(a,b,c)` per Selectors 4 for `:not()` (= most specific argument). Serialisation
   back to text (`Display`) matching the CSSOM serialisation closely enough for round-trip tests.
4. **Matching** (`css::matching`): `trait Element: Clone { fn local_name(&self)->&str; fn namespace_is_html(&self)->bool; fn id(&self)->Option<&str>;
   fn has_class(&self,&str)->bool; fn attribute(&self,&str)->Option<&str>; fn parent_element(&self)->Option<Self>; fn prev_sibling_element(&self)->Option<Self>;
   fn next_sibling_element(&self)->Option<Self>; fn first_child_element/last_child_element; fn is_root(&self)->bool; fn state(&self)->ElementState }` with
   `ElementState { hover, active, focus, focus_visible, focus_within, disabled, enabled, checked, empty }`, and
   `matches(&Selector, &E) -> bool` (right-to-left, no exponential blow-up on deep trees: memoise or bound).
5. **Media queries** (`css::media`): parse `@media` lists (`and`, `not`, `only`, comma = or; media types `all screen print(→ never matches)`; features
   `width height min-/max- both, orientation, prefers-color-scheme, prefers-reduced-motion, hover, any-hover, pointer, any-pointer, resolution(min-resolution dppx)`
   with lengths in px/em/rem; range syntax `(width >= 600px)` too) and `MediaEnv { width, height, color_scheme, reduced_motion, hover, pointer, dppx }` evaluation.
6. **Cascade** (`css::cascade`): `cascade<E: Element>(el: &E, sheets: &[(&Stylesheet, Origin)], env: &MediaEnv, inline: &[Declaration]) ->
   CascadedDeclarations`: collect matching rules (media applied), order by importance (normal/important), origin (user-agent < author; important reverses),
   specificity, then source order (sheet index, rule index, declaration order). Output the winning declaration **per property name** plus all
   candidates in order (for debugging). Shorthand expansion is not part of this WP (property-level only).
   **Custom properties**: inheritance down a caller-supplied ancestor chain, `var(--x, fallback)` substitution with nested vars in fallbacks, cycle detection
   (all members of a cycle become "guaranteed-invalid", i.e. the property falls back/unsets as in the spec), `env()` unsupported→diagnostic. Provide
   `resolve_custom_property(name, ancestors…) -> Option<TokenList>` and text serialisation of token lists matching Chrome's `getPropertyValue('--x')` (trimmed,
   whitespace collapsed as Chrome does — verify with the oracle).
7. **Public-facing support list**: `css::support` exposes the allowlist of selector features and at-rules as data (with the design row IDs) so docs/tooling can print it.

## Tests / oracle
- Tokenizer/parser: hand-written cases from the CSS Syntax spec's own examples + malformed inputs (unterminated strings/urls/comments, stray `}`, nested braces in
  declarations, `!important` spacing, CDO/CDC, escapes, Unicode idents/surrogates, very deep nesting → limit diagnostic), exact diagnostic codes and spans.
- Selectors: specificity table from the Selectors spec examples; parse→serialise round trips; every unsupported selector form rejected with its code.
- **Differential vs Chrome** (`tests/css/gen/gen-selector-corpus.mjs`, deterministic seeded generator, writes `tests/css/corpora/*.json`): ≥ 60 random DOM trees × ≥ 25
  random selectors from a grammar covering all supported features and combinators (including pseudo-class states set via DOM: `disabled`, `checked`, `:focus` via
  `el.focus()`, `:hover` via CDP is not required — test `hover/active` only through a hand-written Rust test with a fake element); record, per (tree, selector), the
  ordered list of matching element paths in Chrome; the Rust test builds the same tree (a small test-only `Element` implementation in `tests/support`) and compares.
- Media: ≥ 80 queries × several `MediaEnv`s vs Chrome (`window.matchMedia` at emulated viewport sizes; use CDP emulation commands through the runner if exposed, otherwise
  `page.viewport(w,h,dpr)`; colour-scheme/reduced-motion via `Emulation.setEmulatedMedia` if X1 allows `page.session.send`).
- Cascade: ≥ 80 stylesheet+DOM fixtures; for keyword-valued properties (`display`, `position`, `visibility`, `overflow`, `float`… pick ones whose computed value equals the
  specified keyword) and custom properties, compare the winning value with `getComputedStyle` in Chrome; include `!important`, specificity ties, source order, media-conditional
  rules, inheritance of custom properties, `var()` fallbacks, cycles.
- Negative controls (doctored corpus row must be reported; matcher bug mutant detected by at least one corpus row — describe how you checked).

## Acceptance
`cargo test -p vn-css` passes; corpus generators deterministic; no warnings; Chrome processes cleaned up; the support list (`css::support`) printed in your report.

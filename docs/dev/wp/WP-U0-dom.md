# WP-U0 — `vn-dom`: a minimal DOM-like document model (tree, attributes, properties, style, events, serialization)

Read `docs/dev/AGENT-CONTRACT.md` and `docs/dev/ARCHITECTURE.md` §4 first. `export CARGO_TARGET_DIR="$VN_ROOT/target/u0"`.
**Prerequisite:** WP-X1 (headless Chrome runner, `tooling/chrome/`) is done — use it as the oracle. Read its README.
**Owned paths:** new crate `crates/dom/**` (package `vn-dom`, library `vn_dom`; **std only, no dependencies, no `unsafe`**),
`tests/dom/**` (oracle generators and corpora). 

## Why
Our Vue runtime port is `runtime-core`'s renderer + `runtime-dom`'s prop/event logic running against a **DOM-like host**.
`vn-dom::Document` is that host's in-memory implementation: it is the headless test host, the live tree of the native renderer
(style/layout/painting read it) and what the SSR serializer walks. A browser DOM host (WASM + JS glue) implements the same
operations later. Real browsers are the ground truth for the behaviours below (Chrome via `tooling/chrome`).

## Data model
`NodeId(u32)` handles (stable until `release(node)`; removal from the tree does NOT free a node — the runtime may re-insert it;
`release` frees a detached subtree), arena storage. `NodeKind = Document | Element | Text | Comment`.
`Namespace = Html | Svg | MathMl`. `HostValue = Undefined | Null | Bool(bool) | Number(f64) | Str(String)`.

## API (the exact set `@vue/runtime-dom` needs; semantics = DOM Living Standard for HTML elements)
- Tree: `create_element(ns, tag, is: Option<&str>)`, `create_text`, `create_comment`, `insert_before(parent, child, anchor:
  Option<NodeId>)` (moves an attached node; rejects cycles and `anchor` not a child of `parent` with an error value, never a
  panic), `remove_child`, `detach(child)` (remove from its parent if any), `parent_node`, `next_sibling`, `previous_sibling`,
  `first_child`, `last_child`, `child_nodes`, `contains`, `root_of`, `is_connected` (reaches a Document node), `release`.
  `kind`, `tag_name` (uppercase for HTML namespace like `tagName`), `local_name`, `namespace`.
- Text: `node_value`/`set_node_value` (Text/Comment), `text_content`/`set_text_content` (element: replace all children by one
  text node, or none when empty; text node: sets data).
- Attributes: `set_attribute`, `set_attribute_ns(ns_uri, qualified, value)`, `remove_attribute`, `get_attribute`,
  `has_attribute`, `attribute_names` (insertion order; HTML attribute names lowercased for HTML elements).
- Properties (`get_property`, `set_property`, `has_property(node, name)`): implement the reflecting IDL properties of the HTML
  elements we support, with correct DOM behaviour: `id`, `className` (↔ `class` attribute), `title`, `hidden`, `tabIndex`,
  `disabled` (button/input/select/textarea/option/fieldset), `type` (input/button; defaults and invalid-value fallback `text`),
  `name`, `placeholder`, `readOnly`, `required`, `multiple`, `min/max/step`, `href` (a), `src/alt` (img), `htmlFor` (label ↔ `for`),
  `value` (input: **dirty value flag** — setting the attribute `value` changes `value` only while not dirty; `value`
  setter sets dirty; checkbox/radio value default `on`; textarea `value`/`defaultValue`/`textContent`; select `value` via options;
  option `value` (↔ attribute, falls back to text), `text`, `selected`), `checked` (input: dirty checkedness, radio group
  exclusivity within the same form owner/root by `name`), `defaultChecked`, `defaultValue`, `selectedIndex`, `options`
  (live list API: `option_ids(select)`), `form`-less simplification documented, `textContent`, `innerHTML` (get only; set → error: native hosts do not parse HTML),
  `isContentEditable` absent. `has_property(el, "x")` must answer like `"x" in el` for the names above and for anything on
  `Element`/`Node`/`HTMLElement`/`EventTarget` that Vue checks (`id`, `className`, `style`, `children`, `textContent`, ...);
  unknown names → `false` (Vue then sets an attribute). Boolean-attribute reflection (`disabled`, `checked`, `selected`, `multiple`,
  `readonly`, `required`, `hidden`) follows the HTML parsing/reflection rules (presence ⇒ true).
- Style: `style_set_property(el, name, value, important)`, `style_remove_property`, `style_get_property`, `css_text`/`set_css_text`
  (the `style` attribute and `style` object stay in sync; serialization format like Chrome: `a: b; c: d !important;`); property names lowercased/kebab for
  standard names, custom properties (`--x`) case-preserved; invalid/empty values remove the declaration. Value *normalisation* (colours,
  lengths) is NOT performed — document that Chrome would normalise and that tests only use non-normalising values.
- Classes: `class_list_add/remove/contains/toggle` on the `class` attribute with DOMTokenList semantics (dedupe, whitespace splitting).
- Events: `add_event_listener(node, type, ListenerId, ListenerOptions { capture, once, passive })` /
  `remove_event_listener(node, type, ListenerId, capture)` (same (type, listener, capture) added twice is ignored, like the DOM);
  `dispatch_event(target, EventInit, &mut dyn FnMut(ListenerId, &mut EventState) -> bool)` implementing the DOM event path
  (capture from the root down, target phase, bubble up if `bubbles`), `once` removal before invocation, `stopPropagation`,
  `stopImmediatePropagation`, `preventDefault` only when `cancelable`, per-listener `currentTarget`/`eventPhase`, listeners added
  during dispatch not run in that dispatch on that node, removed-during-dispatch listeners skipped; returns `DispatchResult
  { default_prevented }`. The callback returning `false` means "listener threw" — dispatch continues (DOM reports the error and continues).
  `EventInit/EventState` carry: `type`, `bubbles`, `cancelable`, `composed`, `time_stamp` (supplied by caller), `target`, `current_target`, `phase`, and a flat
  payload for UI events: `key`, `code`, `ctrl_key/shift_key/alt_key/meta_key`, `button`, `client_x/client_y`, `input_data`,
  `is_composing`, `detail`. Built-in default actions (activation behaviour): `click` on checkbox/radio toggles `checked` **before**
  listeners run (and reverts if prevented), on a label forwards a synthetic click to its control, on `<button type=submit>`
  nothing (no forms). `input`/`change` are NOT synthesised; the host dispatches them.
- Focus: `focus(el)`/`blur()`/`active_element()`; focusable = enabled button/input(not hidden)/select/textarea/`a[href]`/`[tabindex]`;
  `focus`/`blur`/`focusin`/`focusout` events dispatched in the DOM order; `disabled` elements are not focusable.
- Queries (limited, documented): `get_element_by_id(root, id)`, `query_selector(root, simple_selector)` supporting a single compound of
  `tag`, `#id`, `.class`, `[attr]`, `[attr=value]`, `*`, and comma lists (used for Teleport targets); anything else returns an error
  value `UnsupportedSelector`.
- Serialization: `outer_html`, `inner_html` (HTML fragment serialisation algorithm: text/attribute escaping, void elements, boolean attributes
  printed as `name=""`, comments, `<!---->`), and `snapshot(node) -> String`: canonical JSON used by differential tests:
  `{"$":"el","tag":"div","attrs":[["id","a"],["class","b c"]],"children":[…]}` / `{"$":"text","v":"…"}` / `{"$":"comment","v":"…"}`;
  attribute order = insertion order; element **properties that are not reflected as attributes** are listed under `"props"` only for
  `value`, `checked`, `selected`, `disabled` when they differ from their attribute-derived default (document the rule so the browser-side
  serializer in the oracle can implement the identical rule).

## Oracle & tests
- `tests/dom/gen/gen-dom-corpus.mjs` (Node + `tooling/chrome`): runs ~250 small DOM scripts **in headless Chrome** (each script is a list of
  operations from a tiny op language: createElement/appendChild/insertBefore/setAttribute/setProperty/toggle class/style ops/dispatch events/
  focus…) against the real DOM, records observable results (serialisation via the same `snapshot` rules implemented in JS, property reads,
  event logs) and writes `tests/dom/corpora/*.json`. The Rust integration tests replay the same op lists on `Document` and compare. Cover
  specially: input `value` dirty-flag semantics, checkbox/radio activation + radio group exclusivity, select/option selection (`selectedIndex`, `value`,
  multiple), `className`/`class` sync, `style` cssText round trips, boolean attribute reflection, `classList` semantics, attribute-name case
  rules, `textContent` replacement, event path ordering (capture/target/bubble, once, stop*, preventDefault only if cancelable, listener add/remove during dispatch,
  duplicate add ignored), focus/blur event order, tree mutation edge cases (insert moves, anchor errors, cycles).
- Rust unit tests with hand-computed expectations for everything the browser cannot express (error values, `release` semantics, `snapshot` rule).
- Negative controls: a doctored corpus entry must fail; the replay harness must report the op index of the first divergence.

## Acceptance
`cargo test -p vn-dom` passes; `node tests/dom/gen/gen-dom-corpus.mjs` is deterministic (regenerates identical files); no warnings; no `unsafe`; Chrome processes cleaned up.

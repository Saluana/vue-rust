# Browser host ABI (vn-renderer-dom), version 1

`abiVersion` is 1. The wasm module reports it from `vn_abi_version()`; `vn-host.js` declares `ABI_VERSION = 1`. A mismatch is a
hard failure: `createHost` refuses the module (wrong version, or an import the glue does not provide) before running any of it.
Every change to this document is an ABI change: bump the version in `crates/renderer-dom/src/abi.rs` and `vn-host.js` together.

The Rust side is `crates/renderer-dom` (`WebHost` implements `vn_runtime::host::Host`). The JavaScript side is `vn-host.js`, the
page shell `index.html`, and `tooling/web/build-wasm.mjs`, which writes `manifest.json` next to the wasm.

## Values

- **Handles** (`HostNode`): u32. `0` means none. A live handle is `generation * 2^20 + index`, index 1..maxNodes (default 2^20-1),
  generation 1..4095. Released slots are reused with a new generation, so a released handle is NotFound even after reuse.
  The glue creates handles for nodes the runtime did not create when they are returned (parent, sibling, body, active element,
  options, selector results, event targets that already have a handle). The document node is never exposed.
- **Strings**: UTF-8 at (ptr, len) in wasm memory. Rust to JS: borrowed for the call only. JS to Rust: an out slot of two u32 words
  (ptr, len); the glue allocates the bytes with `vn_alloc(len)` and Rust frees them with `vn_free(ptr, len)`.
- **Status** (fallible imports): `0` OK, `1` NotFound (handle 0, unknown, or released), `2` Unsupported (outside the supported
  subset, see Policy), `3` InvalidState (the DOM refused it: HierarchyRequestError, NotFoundError for a non-child anchor,
  NamespaceError, InvalidCharacterError, SyntaxError; a wrong node kind; handles exhausted; host destroyed). The message of a
  failure is fetched with `host_error_message`.
- **Infallible queries** (`parent_node`, `get_attribute`, `tag_name`, ...): a bad handle returns 0, absent, or "" and the glue logs
  a warning. Nothing is silently approximated.
- **HostValue** out struct (24 bytes): `kind` u32 @0 (0 undefined, 1 null, 2 bool, 3 number, 4 string), `flag` u32 @4 (bool),
  `num` f64 @8, text ptr u32 @16, text len u32 @20.
- **Option lists** (`option_nodes`): out slot (ptr, count); the array is `count` u32 handles at ptr, allocated with `vn_alloc(4*count)`.
- Booleans are u32 0/1. Every import normalises its u32 arguments with `>>> 0`.

## Imports (module `vn_host`)

| import | parameters (wasm types) | result | meaning |
|---|---|---|---|
| host_now | () | f64 | monotonic milliseconds (performance.now) |
| host_set_timeout | (id u32, ms f64) | - | schedule `vn_timer(id)` |
| host_clear_timeout | (id u32) | - | cancel timer |
| host_log | (level u32, ptr, len) | - | 0 debug, 1 info, 2 warn, 3 error; goes to `onLog` |
| host_panic | (ptr, len) | - | called by the panic hook, then the module traps |
| host_error_message | (out) | - | writes the last failure message to out |
| create_element | (ns u32, tag ptr, tag len, is ptr, is len, has_is u32, out) | i32 | ns 0 html, 1 svg, 2 mathml; out: handle |
| create_text | (ptr, len) | u32 | handle; 0 when handles are exhausted (Rust panics) |
| create_comment | (ptr, len) | u32 | as create_text |
| set_text | (node, ptr, len) | i32 | nodeValue of a text or comment node (element: InvalidState) |
| set_element_text | (el, ptr, len) | i32 | textContent of an element |
| insert | (child, parent, anchor) | i32 | insertBefore; anchor 0 appends |
| remove | (child) | i32 | removeChild if attached; detached is OK |
| parent_node | (node) | u32 | 0 when none or the document |
| next_sibling | (node) | u32 | 0 when none |
| query_selector | (ptr, len, out) | i32 | out: handle or 0; subset grammar (Policy) |
| body | () | u32 | document.body |
| release | (node) | - | frees the handle; the node stays where the DOM puts it (warned if attached) |
| tag_name | (el, out) | i32 | tagName (uppercase for HTML) |
| is_element | (node) | i32 | 1 or 0 |
| set_attribute | (el, name ptr, name len, value ptr, value len) | i32 | setAttribute |
| set_attribute_ns | (el, has_ns, ns ptr, ns len, name ptr, name len, value ptr, value len) | i32 | setAttributeNS; has_ns 0 is a null namespace |
| remove_attribute | (el, name ptr, name len) | i32 | removeAttribute |
| remove_attribute_ns | (el, has_ns, ns ptr, ns len, local ptr, local len) | i32 | removeAttributeNS |
| get_attribute | (el, name ptr, name len, out) | i32 | 1 present (out written), 0 absent or failed (logged) |
| has_attribute | (el, name ptr, name len) | i32 | 1 or 0 |
| has_property | (node, name ptr, name len) | i32 | `name in node` |
| get_property | (node, name ptr, name len, out) | i32 | `node[name]` as HostValue; objects and functions are Unsupported |
| set_property | (node, name ptr, name len, kind u32, num f64, str ptr, str len) | i32 | `node[name] = value` in strict mode |
| style_set_property | (el, name ptr, name len, value ptr, value len, important u32) | i32 | style.setProperty |
| style_remove_property | (el, name ptr, name len) | i32 | style.removeProperty |
| style_get_property | (el, name ptr, name len, out) | i32 | style.getPropertyValue (out string) |
| style_css_text | (el, out) | i32 | style.cssText (out string) |
| style_set_css_text | (el, text ptr, text len) | i32 | style.cssText = text |
| class_list_add | (el, token ptr, token len) | i32 | classList.add |
| class_list_remove | (el, token ptr, token len) | i32 | classList.remove |
| add_event_listener | (el, type ptr, type len, listener u32, flags u32) | i32 | flags: bit0 capture, bit1 once, bit2 passive |
| remove_event_listener | (el, type ptr, type len, listener u32, capture u32) | i32 | removes the registration with the same id and capture |
| focus | (el) | i32 | focus(); non-focusable elements do nothing (browser rule) |
| blur | (el) | i32 | blur() |
| active_element | () | u32 | focused element, 0 when none or when body is active |
| option_nodes | (select, out) | i32 | out: (ptr, count) of option handles |
| outer_html | (node, out) | i32 | 1 present, 0 none (text nodes) |
| inner_html | (node, out) | i32 | 1 present, 0 none |

## Exports

| export | signature | meaning |
|---|---|---|
| memory | linear memory | |
| vn_abi_version | () -> u32 | returns 1 |
| vn_init | () -> () | installs the panic hook; called once after instantiation |
| vn_alloc | (len u32) -> u32 | 8-byte aligned block, 0 on failure; len 0 returns 8 (never read) |
| vn_free | (ptr u32, len u32) -> () | frees a block from vn_alloc with the same len |
| vn_event | (listener u32, ptr u32, len u32) -> u32 | delivers one event payload; returns outcome bits |
| vn_timer | (id u32) -> u32 | fires a timer; 1 when a hook is installed, else 0 |
| vn_host_probe | () -> u32 | links every import (a cdylib keeps only what its exports reach) and returns 44; the glue does not call it |
| selftest_call | (ptr u32, len u32) -> u32 | selftest build only: JSON request in, reply buffer out (see below) |

## Event payload (`vn_event`)

The glue allocates the payload with `vn_alloc`, fills it, calls `vn_event`, then frees it. All numbers are little-endian.

| offset | type | field |
|---|---|---|
| 0 / 4 | u32 / u32 | type string: offset from the payload start, length |
| 8 / 12 | u32 / u32 | key |
| 16 / 20 | u32 / u32 | code |
| 24 / 28 | u32 / u32 | inputData (meaningful only with flag bit 9) |
| 32 | f64 | timeStamp |
| 40 | f64 | clientX |
| 48 | f64 | clientY |
| 56 | i32 | button |
| 60 | i32 | detail |
| 64 | u32 | target handle (0 = none) |
| 68 | u32 | currentTarget handle (0 = none) |
| 72 | u32 | eventPhase (0..3) |
| 76 | u32 | flags: 1 bubbles, 2 cancelable, 4 composed, 8 defaultPrevented (at this listener), 16 ctrl, 32 shift, 64 alt, 128 meta, 256 isComposing, 512 has inputData |
| 80.. | bytes | UTF-8 strings, referenced by the offsets above |

Outcome bits returned by `vn_event`: 1 stopPropagation, 2 stopImmediatePropagation, 4 preventDefault, 8 threw. The glue applies
them to the real event synchronously. A listener that threw is reported (`onLog` error) and dispatch continues, as in the browser.
A malformed payload is rejected by Rust with an error log and no effect.

## Re-entrancy and panics

Any import may run a listener, and a listener may call any import (the glue never holds a borrow across dispatch; Rust holds only
thread-local cells that are not borrowed during a call). Browser effects are synchronous, for example blur during remove and
custom-element callbacks during insert. A Rust panic calls `host_panic` (the message reaches `onLog` and `stats().panics`) and then
traps. The JavaScript call that trapped fails; later calls work.

## Policy (refused by the glue, with Unsupported)

- Creating `script`, `iframe`, `frame`, `frameset`, `object`, `embed`, `applet` or `base` (any namespace). Code-running markup never
  enters the document through the host.
- Attributes and properties whose name starts with `on`, the `srcdoc` attribute, and `innerHTML`/`outerHTML` setters: event
  handlers are added with `add_event_listener`, and no HTML is parsed from strings.
- `javascript:` values (after optional leading whitespace, any case) in href, src, action, formaction, poster, data or background.
- Properties the node does not have (`name in node` is false): no expando properties are created.
- Object, function, symbol and bigint values through `get_property`.
- Selectors outside the subset shared with vn-dom (`crates/dom/src/query.rs`): compounds of tag, `#id`, `.class`, `[attr]`,
  `[attr=value]` (bare or quoted), `*`, combined without spaces, and comma lists. Other syntax is Unsupported; an empty selector is
  InvalidState.
- Style property names that are neither custom properties (`--x`) nor kebab-case CSS property names (CSS.supports): Unsupported.
  An invalid value is ignored, as CSSOM specifies.

## Self-test protocol (feature `selftest` only)

`selftest_call` takes a UTF-8 JSON array of operations `["method", arg...]` and returns a buffer: a little-endian u32 byte count, then
a JSON array with one reply per operation: `{"ok": value}`, `{"err": {"kind": "NotFound"|"Unsupported"|"InvalidState", "msg"?: text}}`
or `{"harness": message}` (a mistake in the request, never sent to the host). Methods are the `Host` method names in snake_case, plus
harness controls prefixed with `__` (`__behave`, `__log`, `__timer_set`, `__timer_clear`, `__timers_fired`, `__log_message`,
`__churn`, `__panic`, `__reset`). Handles are JSON numbers, `null` is None, HostValue is `{"k": "undef"|"null"|"bool"|"num"|"str", "v"}`.

## Diagnostics (not ABI)

`createHost` returns `{ abiVersion, exports, memory, stats(), destroy(), dispatchEvent(handle, kind, type, init), click(handle) }`.
`dispatchEvent` and `click` run the browser's own dispatch on a handle (kind `Event`, `Mouse`, `Keyboard` or `Input`, built as the
corpus runner builds them); the replay tests use them. `stats()` reports live handles, listeners, timers, memory pages and panics.

## Build outputs

`node tooling/web/build-wasm.mjs` writes `<out>/shipped/` (no self-test) and `<out>/selftest/`, each with
`vn_renderer_dom-<sha256 prefix>.wasm`, `vn-host.js`, `index.html` and `manifest.json` `{abiVersion, wasm, hash, bytes, glue,
features, profile, target}`. The default `<out>` is `$CARGO_TARGET_DIR/web-dist`.

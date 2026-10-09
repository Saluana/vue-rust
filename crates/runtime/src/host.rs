//! The host interface: the DOM-like operations the ported renderer (`renderer`, `dom_props`, `vmodel`, `transition`, …) needs.
//!
//! Implementations: `DocHost` (over `vn_dom::Document`; headless tests, native renderer, SSR) in `dom_events`/`dom_props`'s owning WP, and the
//! browser host (`hosts/web`: wasm imports + generated JS glue) in `crates/renderer-dom`. The trait mirrors the subset of the DOM that
//! `@vue/runtime-dom` 3.5 uses (nodeOps + patchProp + directives), so the ported logic is the same on every host. Coordinator-owned contract:
//! change it only through the coordinator (it fans out to every host).
//!
//! Events flow the other way through [`EventSink`]: hosts call it for every listener invocation; the runtime builds the JS `Event` object and
//! runs the handler. A host implementation may be mid-mutation when it calls the sink (e.g. blur fires before a focused subtree is detached), so
//! the runtime may call back into the host from inside the sink (re-entrancy is part of the contract; `DocHost` achieves it by swapping its
//! document into a shared cell for the duration of the call).

/// Opaque handle of a host node (element, text or comment). Stable until `release`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct HostNode(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HostNs {
    Html,
    Svg,
    MathMl,
}

/// A DOM property/attribute-ish value crossing the host boundary (strings are UTF-8; the host never sees lone surrogates:
/// the runtime converts JS strings lossily at this boundary).
#[derive(Clone, Debug, PartialEq)]
pub enum HostValue {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
}

/// Registered with `add_event_listener`; echoed back to the sink. Allocated by the runtime.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct ListenerId(pub u32);

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct ListenerOpts {
    pub capture: bool,
    pub once: bool,
    pub passive: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HostError {
    /// The host does not implement this operation / property / selector (e.g. `innerHTML` setter on native hosts).
    Unsupported(String),
    /// The operation is invalid in the current tree state (cycle, anchor not a child, released node, …).
    InvalidState(String),
    NotFound,
}
pub type HostResult<T> = Result<T, HostError>;

/// A UI event as seen by listeners (field set = what Vue's runtime-dom and typical handlers read).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct HostEvent {
    pub ty: String,
    pub target: Option<HostNode>,
    pub current_target: Option<HostNode>,
    /// 0 none, 1 capturing, 2 at target, 3 bubbling (DOM `eventPhase`).
    pub phase: u8,
    pub bubbles: bool,
    pub cancelable: bool,
    pub composed: bool,
    pub default_prevented: bool,
    pub time_stamp: f64,
    pub key: String,
    pub code: String,
    pub ctrl_key: bool,
    pub shift_key: bool,
    pub alt_key: bool,
    pub meta_key: bool,
    pub button: i32,
    pub client_x: f64,
    pub client_y: f64,
    pub input_data: Option<String>,
    pub is_composing: bool,
    pub detail: i32,
}

/// What the sink asks of the host after a listener ran (mirrors DOM propagation flags).
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct ListenerOutcome {
    pub stop_propagation: bool,
    pub stop_immediate: bool,
    pub prevent_default: bool,
    /// The listener threw (the host reports and continues dispatching, as the DOM does).
    pub threw: bool,
}

/// Implemented by the runtime; hosts call it once per listener invocation.
pub trait EventSink {
    fn dispatch(&self, listener: ListenerId, event: &HostEvent) -> ListenerOutcome;
}

/// DOM-like operations. All string arguments are UTF-8. `&mut self` methods may invoke the registered [`EventSink`] re-entrantly.
pub trait Host {
    // ---- wiring
    fn set_event_sink(&mut self, sink: std::rc::Rc<dyn EventSink>);
    /// Monotonic milliseconds for event timestamps and `Date.now()`-like needs (the test host returns the fake clock).
    fn now_ms(&self) -> f64;

    // ---- tree (nodeOps)
    fn create_element(&mut self, ns: HostNs, tag: &str, is: Option<&str>) -> HostResult<HostNode>;
    fn create_text(&mut self, text: &str) -> HostNode;
    fn create_comment(&mut self, text: &str) -> HostNode;
    /// `node.nodeValue = text` (text and comment nodes).
    fn set_text(&mut self, node: HostNode, text: &str) -> HostResult<()>;
    /// `el.textContent = text` (replaces all children; empty text removes them).
    fn set_element_text(&mut self, el: HostNode, text: &str) -> HostResult<()>;
    /// `parent.insertBefore(child, anchor)`; moves an attached child; `anchor == None` appends.
    fn insert(&mut self, child: HostNode, parent: HostNode, anchor: Option<HostNode>) -> HostResult<()>;
    /// Removes `child` from its parent if it has one (no error when detached).
    fn remove(&mut self, child: HostNode) -> HostResult<()>;
    fn parent_node(&self, node: HostNode) -> Option<HostNode>;
    fn next_sibling(&self, node: HostNode) -> Option<HostNode>;
    /// `document.querySelector` for Teleport targets (simple selectors; `Unsupported` otherwise).
    fn query_selector(&self, selector: &str) -> HostResult<Option<HostNode>>;
    /// The node the app is mounted into when given a container id/selector by hosts that own one (see `app`); optional helper.
    fn body(&self) -> Option<HostNode>;
    /// Frees a detached subtree's resources (browser hosts drop their JS references). No-op allowed.
    fn release(&mut self, node: HostNode);

    // ---- introspection used by prop/patch decisions
    fn tag_name(&self, el: HostNode) -> String; // uppercase for HTML elements like DOM `tagName`
    fn is_element(&self, node: HostNode) -> bool;

    // ---- attributes
    fn set_attribute(&mut self, el: HostNode, name: &str, value: &str) -> HostResult<()>;
    fn set_attribute_ns(&mut self, el: HostNode, ns: Option<&str>, qualified: &str, value: &str) -> HostResult<()>;
    fn remove_attribute(&mut self, el: HostNode, name: &str) -> HostResult<()>;
    fn remove_attribute_ns(&mut self, el: HostNode, ns: Option<&str>, local: &str) -> HostResult<()>;
    fn get_attribute(&self, el: HostNode, name: &str) -> Option<String>;
    fn has_attribute(&self, el: HostNode, name: &str) -> bool;

    // ---- properties (`name in el`, `el[name]`, `el[name] = value`)
    fn has_property(&self, el: HostNode, name: &str) -> bool;
    fn get_property(&mut self, el: HostNode, name: &str) -> HostResult<HostValue>;
    fn set_property(&mut self, el: HostNode, name: &str, value: HostValue) -> HostResult<()>;

    // ---- style and class
    fn style_set_property(&mut self, el: HostNode, name: &str, value: &str, important: bool) -> HostResult<()>;
    fn style_remove_property(&mut self, el: HostNode, name: &str) -> HostResult<()>;
    fn style_get_property(&self, el: HostNode, name: &str) -> String;
    fn style_css_text(&self, el: HostNode) -> String;
    fn style_set_css_text(&mut self, el: HostNode, text: &str) -> HostResult<()>;
    fn class_list_add(&mut self, el: HostNode, token: &str) -> HostResult<()>;
    fn class_list_remove(&mut self, el: HostNode, token: &str) -> HostResult<()>;

    // ---- events
    fn add_event_listener(&mut self, el: HostNode, ty: &str, id: ListenerId, opts: ListenerOpts) -> HostResult<()>;
    fn remove_event_listener(&mut self, el: HostNode, ty: &str, id: ListenerId, capture: bool) -> HostResult<()>;

    // ---- focus
    fn focus(&mut self, el: HostNode) -> HostResult<()>;
    fn blur(&mut self, el: HostNode) -> HostResult<()>;
    fn active_element(&self) -> Option<HostNode>;

    // ---- forms (select/option helpers used by v-model)
    fn option_nodes(&self, select: HostNode) -> Vec<HostNode>;

    // ---- serialization (SSR, tests)
    fn outer_html(&mut self, node: HostNode) -> Option<String>;
    fn inner_html(&mut self, node: HostNode) -> Option<String>;
}

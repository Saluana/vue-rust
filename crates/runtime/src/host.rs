//! The host interface: the DOM-like operations the ported renderer (`renderer`, `dom_props`, `vmodel`, `transition`, …) needs.
//!
//! Implementations: `DocHost` (over `vn_dom::Document`; headless tests, native renderer, SSR) in `dom_events`/`dom_props`'s owning WP, and the
//! browser host (`hosts/web`: wasm imports + generated JS glue) in `crates/renderer-dom`. The trait mirrors the subset of the DOM that
//! `@vue/runtime-dom` 3.5 uses (nodeOps + patchProp + directives), so the ported logic is the same on every host. Maintainer-owned contract:
//! change it only through the maintainer (it fans out to every host).
//!
//! Events flow the other way through [`EventSink`]: hosts call it for every listener invocation; the runtime builds the JS `Event` object and
//! runs the handler. A host implementation may be mid-mutation when it calls the sink (e.g. blur fires before a focused subtree is detached), so
//! the runtime may call back into the host from inside the sink (re-entrancy is part of the contract; `DocHost` achieves it by swapping its
//! document into a shared cell for the duration of the call).

use std::cell::RefCell;
use std::rc::Rc;

use vn_dom::EventInit;
pub use vn_dom::EventKind;

/// The dispatch that the sink is currently serving: a serial that changes per dispatch, and the event kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventInfo {
    pub serial: u64,
    pub kind: EventKind,
}

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
    /// The outermost host dispatch has finished; event identity caches can be dropped.
    fn finish_dispatch(&self) {}
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
    /// Mapped handles in a subtree, including its root, for resource lifetime tracking.
    fn subtree_nodes(&self, node: HostNode) -> Vec<HostNode> { vec![node] }

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
    /// Computed CSS timing properties used by Transition, including stylesheet rules.
    fn computed_style_property(&self, _el: HostNode, _name: &str) -> HostResult<String> { Err(HostError::Unsupported("computed style".to_string())) }
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

    // ---- capabilities the DOM layer uses; the defaults suit hosts without dispatch identity or a test surface
    /// The identity of the dispatch being served (events of one dispatch share a serial).
    fn event_info(&self) -> EventInfo {
        EventInfo { serial: 0, kind: EventKind::Event }
    }
    /// Registers a hook that runs when a node is released.
    fn add_release_hook(&self, _hook: Rc<dyn Fn(HostNode)>) {}
    /// A click on the node (`el.click()`); hosts without event dispatch refuse it.
    fn click(&mut self, _node: HostNode) -> HostResult<()> {
        Err(HostError::Unsupported("click".to_string()))
    }
    /// The document node (the root of every connected node), when the host has one.
    fn document_node(&self) -> Option<HostNode> {
        None
    }
    /// Dispatches an event at `target` (returns whether the default was not prevented).
    fn dispatch_event(&mut self, _target: HostNode, _init: EventInit) -> HostResult<bool> {
        Err(HostError::Unsupported("dispatchEvent".to_string()))
    }
}

/// A handle to a host the DOM layer shares. Clones of a handle act on one host; `handle()` gives a handle for one call.
pub trait HostProvider {
    fn handle(&self) -> Box<dyn Host>;
}

/// The host of a DOM context (see `HostProvider`).
pub type SharedHost = Rc<dyn HostProvider>;

/// A host behind `Rc<RefCell<dyn Host>>`: each call borrows the host for its own duration. A host that calls its
/// listeners from inside one of its own calls must hand out clone-based handles instead (as DocHost does), since a
/// listener that calls back into the host would find the cell borrowed.
impl HostProvider for Rc<RefCell<dyn Host>> {
    fn handle(&self) -> Box<dyn Host> {
        Box::new(SharedRef(Rc::clone(self)))
    }
}

struct SharedRef(Rc<RefCell<dyn Host>>);

impl Host for SharedRef {
    fn set_event_sink(&mut self, sink: Rc<dyn EventSink>) { self.0.borrow_mut().set_event_sink(sink) }
    fn now_ms(&self) -> f64 { self.0.borrow().now_ms() }
    fn create_element(&mut self, ns: HostNs, tag: &str, is: Option<&str>) -> HostResult<HostNode> { self.0.borrow_mut().create_element(ns, tag, is) }
    fn create_text(&mut self, text: &str) -> HostNode { self.0.borrow_mut().create_text(text) }
    fn create_comment(&mut self, text: &str) -> HostNode { self.0.borrow_mut().create_comment(text) }
    fn set_text(&mut self, node: HostNode, text: &str) -> HostResult<()> { self.0.borrow_mut().set_text(node, text) }
    fn set_element_text(&mut self, el: HostNode, text: &str) -> HostResult<()> { self.0.borrow_mut().set_element_text(el, text) }
    fn insert(&mut self, child: HostNode, parent: HostNode, anchor: Option<HostNode>) -> HostResult<()> { self.0.borrow_mut().insert(child, parent, anchor) }
    fn remove(&mut self, child: HostNode) -> HostResult<()> { self.0.borrow_mut().remove(child) }
    fn parent_node(&self, node: HostNode) -> Option<HostNode> { self.0.borrow().parent_node(node) }
    fn next_sibling(&self, node: HostNode) -> Option<HostNode> { self.0.borrow().next_sibling(node) }
    fn query_selector(&self, selector: &str) -> HostResult<Option<HostNode>> { self.0.borrow().query_selector(selector) }
    fn body(&self) -> Option<HostNode> { self.0.borrow().body() }
    fn release(&mut self, node: HostNode) { self.0.borrow_mut().release(node) }
    fn subtree_nodes(&self, node: HostNode) -> Vec<HostNode> { self.0.borrow().subtree_nodes(node) }
    fn tag_name(&self, el: HostNode) -> String { self.0.borrow().tag_name(el) }
    fn is_element(&self, node: HostNode) -> bool { self.0.borrow().is_element(node) }
    fn set_attribute(&mut self, el: HostNode, name: &str, value: &str) -> HostResult<()> { self.0.borrow_mut().set_attribute(el, name, value) }
    fn set_attribute_ns(&mut self, el: HostNode, ns: Option<&str>, qualified: &str, value: &str) -> HostResult<()> { self.0.borrow_mut().set_attribute_ns(el, ns, qualified, value) }
    fn remove_attribute(&mut self, el: HostNode, name: &str) -> HostResult<()> { self.0.borrow_mut().remove_attribute(el, name) }
    fn remove_attribute_ns(&mut self, el: HostNode, ns: Option<&str>, local: &str) -> HostResult<()> { self.0.borrow_mut().remove_attribute_ns(el, ns, local) }
    fn get_attribute(&self, el: HostNode, name: &str) -> Option<String> { self.0.borrow().get_attribute(el, name) }
    fn has_attribute(&self, el: HostNode, name: &str) -> bool { self.0.borrow().has_attribute(el, name) }
    fn has_property(&self, el: HostNode, name: &str) -> bool { self.0.borrow().has_property(el, name) }
    fn get_property(&mut self, el: HostNode, name: &str) -> HostResult<HostValue> { self.0.borrow_mut().get_property(el, name) }
    fn set_property(&mut self, el: HostNode, name: &str, value: HostValue) -> HostResult<()> { self.0.borrow_mut().set_property(el, name, value) }
    fn style_set_property(&mut self, el: HostNode, name: &str, value: &str, important: bool) -> HostResult<()> { self.0.borrow_mut().style_set_property(el, name, value, important) }
    fn style_remove_property(&mut self, el: HostNode, name: &str) -> HostResult<()> { self.0.borrow_mut().style_remove_property(el, name) }
    fn style_get_property(&self, el: HostNode, name: &str) -> String { self.0.borrow().style_get_property(el, name) }
    fn computed_style_property(&self, el: HostNode, name: &str) -> HostResult<String> { self.0.borrow().computed_style_property(el, name) }
    fn style_css_text(&self, el: HostNode) -> String { self.0.borrow().style_css_text(el) }
    fn style_set_css_text(&mut self, el: HostNode, text: &str) -> HostResult<()> { self.0.borrow_mut().style_set_css_text(el, text) }
    fn class_list_add(&mut self, el: HostNode, token: &str) -> HostResult<()> { self.0.borrow_mut().class_list_add(el, token) }
    fn class_list_remove(&mut self, el: HostNode, token: &str) -> HostResult<()> { self.0.borrow_mut().class_list_remove(el, token) }
    fn add_event_listener(&mut self, el: HostNode, ty: &str, id: ListenerId, opts: ListenerOpts) -> HostResult<()> { self.0.borrow_mut().add_event_listener(el, ty, id, opts) }
    fn remove_event_listener(&mut self, el: HostNode, ty: &str, id: ListenerId, capture: bool) -> HostResult<()> { self.0.borrow_mut().remove_event_listener(el, ty, id, capture) }
    fn focus(&mut self, el: HostNode) -> HostResult<()> { self.0.borrow_mut().focus(el) }
    fn blur(&mut self, el: HostNode) -> HostResult<()> { self.0.borrow_mut().blur(el) }
    fn active_element(&self) -> Option<HostNode> { self.0.borrow().active_element() }
    fn option_nodes(&self, select: HostNode) -> Vec<HostNode> { self.0.borrow().option_nodes(select) }
    fn outer_html(&mut self, node: HostNode) -> Option<String> { self.0.borrow_mut().outer_html(node) }
    fn inner_html(&mut self, node: HostNode) -> Option<String> { self.0.borrow_mut().inner_html(node) }
    fn event_info(&self) -> EventInfo { self.0.borrow().event_info() }
    fn add_release_hook(&self, hook: Rc<dyn Fn(HostNode)>) { self.0.borrow().add_release_hook(hook) }
    fn click(&mut self, node: HostNode) -> HostResult<()> { self.0.borrow_mut().click(node) }
    fn document_node(&self) -> Option<HostNode> { self.0.borrow().document_node() }
    fn dispatch_event(&mut self, target: HostNode, init: EventInit) -> HostResult<bool> { self.0.borrow_mut().dispatch_event(target, init) }
}

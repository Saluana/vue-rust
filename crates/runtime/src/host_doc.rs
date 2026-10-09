//! `DocHost`: the [`Host`] trait over the in-memory `vn_dom::Document` (WP-RT-D).
//!
//! Re-entrancy (host.rs contract). Operations that can run listeners (insert, remove, set_element_text, set_property,
//! focus, blur, click, dispatch_event) take the document out of the shared cell for the call. The `Invoke` callback
//! given to vn-dom swaps the live document back into the cell before the event sink runs and swaps vn-dom's document
//! back out afterwards, so a listener that calls host methods (reads `value`, sets attributes, moves focus, removes
//! nodes) sees the live tree. A nested operation takes the live document out of the cell and puts it back when it ends.
//! Clones of a `DocHost` share one document, one node table and one sink.
//!
//! Node handles. `HostNode(n)` is an index into a table of vn-dom `NodeId`s. vn-dom never reuses ids, so the table
//! needs no generation counter: a released subtree is detected by vn-dom reporting the id as dead.
//!
//! Event identity. The runtime needs one JS event object per dispatch (`_vts` guard, `e._stopped`), but the sink only
//! receives a `HostEvent`. `DocHost::event_info` returns a serial that changes whenever a different dispatch starts,
//! plus the vn-dom event kind (which decides which fields a JS event has). The serial is keyed by the address of the
//! dispatch's `EventState` (stack-allocated, unique among live dispatches) together with type, target and time stamp.
//! Two sequential dispatches with the same key inside one top-level host call would share a serial; top-level calls
//! reset the key, so each host call starts a new serial.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use vn_dom::{DomError, Document, EventInit, EventKind, EventState, Invoke, ListenerId as DomListenerId, ListenerOptions, Namespace, NodeId, NodeKind};

use crate::host::{EventSink, Host, HostError, HostEvent, HostNode, HostNs, HostProvider, HostResult, HostValue, ListenerId, ListenerOpts};

/// `EventInfo` is defined with the host contract (host.rs).
pub use crate::host::EventInfo;

#[derive(Clone, Debug, PartialEq, Eq)]
struct EventKey {
    addr: usize,
    ty: String,
    target: NodeId,
    time_stamp: u64,
}

#[derive(Default)]
struct Nodes {
    by_host: Vec<Option<NodeId>>,
    by_dom: HashMap<NodeId, HostNode>,
}

impl Nodes {
    fn add(&mut self, id: NodeId) -> HostNode {
        let h = HostNode(self.by_host.len() as u32);
        self.by_host.push(Some(id));
        self.by_dom.insert(id, h);
        h
    }

    fn get(&self, h: HostNode) -> Option<NodeId> {
        self.by_host.get(h.0 as usize).copied().flatten()
    }

    fn handle(&self, id: NodeId) -> Option<HostNode> {
        self.by_dom.get(&id).copied()
    }

    fn free(&mut self, h: HostNode) {
        if let Some(slot) = self.by_host.get_mut(h.0 as usize) {
            if let Some(id) = slot.take() {
                self.by_dom.remove(&id);
            }
        }
    }
}

struct Shared {
    release_hooks: RefCell<Vec<Rc<dyn Fn(HostNode)>>>,
    doc: RefCell<Document>,
    nodes: RefCell<Nodes>,
    sink: RefCell<Option<Rc<dyn EventSink>>>,
    body: Cell<Option<HostNode>>,
    now: Cell<f64>,
    depth: Cell<u32>,
    key: RefCell<Option<EventKey>>,
    serial: Cell<u64>,
    info: Cell<EventInfo>,
}

/// The headless host: `Host` over `vn_dom::Document`, plus the test surface (dispatch, click, clock, snapshots).
#[derive(Clone)]
pub struct DocHost {
    shared: Rc<Shared>,
}

impl Default for DocHost {
    fn default() -> Self {
        Self::new()
    }
}

fn map_ns(ns: HostNs) -> Namespace {
    match ns {
        HostNs::Html => Namespace::Html,
        HostNs::Svg => Namespace::Svg,
        HostNs::MathMl => Namespace::MathMl,
    }
}

fn map_err(e: DomError) -> HostError {
    match e {
        DomError::StaleNode => HostError::NotFound,
        DomError::UnsupportedProperty | DomError::UnsupportedSelector | DomError::UnsupportedStyleProperty => {
            HostError::Unsupported(e.name().to_string())
        }
        other => HostError::InvalidState(other.name().to_string()),
    }
}

fn to_dom_value(v: HostValue) -> vn_dom::HostValue {
    match v {
        HostValue::Undefined => vn_dom::HostValue::Undefined,
        HostValue::Null => vn_dom::HostValue::Null,
        HostValue::Bool(b) => vn_dom::HostValue::Bool(b),
        HostValue::Number(n) => vn_dom::HostValue::Number(n),
        HostValue::Str(s) => vn_dom::HostValue::Str(s),
    }
}

fn from_dom_value(v: vn_dom::HostValue) -> HostValue {
    match v {
        vn_dom::HostValue::Undefined => HostValue::Undefined,
        vn_dom::HostValue::Null => HostValue::Null,
        vn_dom::HostValue::Bool(b) => HostValue::Bool(b),
        vn_dom::HostValue::Number(n) => HostValue::Number(n),
        vn_dom::HostValue::Str(s) => HostValue::Str(s),
    }
}

/// The xlink, xml and xmlns prefixes that `removeAttributeNS` can be given; other namespaces are unsupported.
fn prefixed_name(ns: Option<&str>, local: &str) -> HostResult<String> {
    let prefix = match ns {
        None => return Ok(local.to_string()),
        Some("http://www.w3.org/1999/xlink") => "xlink",
        Some("http://www.w3.org/XML/1998/namespace") => "xml",
        Some("http://www.w3.org/2000/xmlns/") => "xmlns",
        Some(other) => return Err(HostError::Unsupported(format!("removeAttributeNS namespace {other}"))),
    };
    Ok(format!("{prefix}:{local}"))
}

impl DocHost {
    /// An empty document (no children) whose document node is `HostNode(0)`. The clock starts at 0 ms.
    pub fn new() -> DocHost {
        let doc = Document::new();
        let mut nodes = Nodes::default();
        nodes.add(doc.document());
        DocHost {
            shared: Rc::new(Shared {
                release_hooks: RefCell::new(Vec::new()),
                doc: RefCell::new(doc),
                nodes: RefCell::new(nodes),
                sink: RefCell::new(None),
                body: Cell::new(None),
                now: Cell::new(0.0),
                depth: Cell::new(0),
                key: RefCell::new(None),
                serial: Cell::new(0),
                info: Cell::new(EventInfo { serial: 0, kind: EventKind::Event }),
            }),
        }
    }

    /// The document node (`HostNode(0)`).
    pub fn document_node(&self) -> HostNode {
        HostNode(0)
    }

    /// Creates `<html><body></body></html>` once and returns the body (the mount container for `body()`).
    pub fn install_body(&mut self) -> HostResult<HostNode> {
        if let Some(b) = self.shared.body.get() {
            return Ok(b);
        }
        let html = self.create_element(HostNs::Html, "html", None)?;
        let body = self.create_element(HostNs::Html, "body", None)?;
        self.insert(html, self.document_node(), None)?;
        self.insert(body, html, None)?;
        self.shared.body.set(Some(body));
        Ok(body)
    }

    /// Registers a callback run for every node that `release` frees (the DOM layer drops its per-node state with it).
    pub fn add_release_hook(&self, hook: Rc<dyn Fn(HostNode)>) {
        self.shared.release_hooks.borrow_mut().push(hook);
    }

    /// Settable fake clock for event time stamps and `now_ms`.
    pub fn set_now(&self, ms: f64) {
        self.shared.now.set(ms);
    }

    pub fn advance(&self, ms: f64) {
        self.shared.now.set(self.shared.now.get() + ms);
    }

    /// The dispatch being delivered to the sink (valid while the sink runs).
    pub fn event_info(&self) -> EventInfo {
        self.shared.info.get()
    }

    /// `element.dispatchEvent(event)` for a script-created event. Returns `defaultPrevented` after the dispatch.
    /// The time stamp is taken from the fake clock.
    pub fn dispatch_event(&mut self, target: HostNode, mut init: EventInit) -> HostResult<bool> {
        let id = self.id(target)?;
        init.time_stamp = self.now_ms();
        let r = self.with_doc(|d, inv| d.dispatch_event(id, init, inv)).map_err(map_err)?;
        Ok(r.default_prevented)
    }

    /// `element.click()`: a no-op on disabled controls; otherwise a bubbling, cancelable mouse click.
    pub fn click(&mut self, node: HostNode) -> HostResult<()> {
        let id = self.id(node)?;
        self.with_doc(|d, inv| d.click(id, inv)).map_err(map_err)
    }

    /// Canonical vn-dom snapshot of a node and its subtree (the corpus format of tests/dom/corpora).
    pub fn snapshot(&mut self, node: HostNode) -> HostResult<String> {
        let id = self.id(node)?;
        self.shared.doc.borrow_mut().snapshot(id).map_err(map_err)
    }

    /// The handle of a live vn-dom node, if the table knows it.
    pub fn handle_of(&self, id: NodeId) -> Option<HostNode> {
        self.shared.nodes.borrow().handle(id)
    }

    fn id(&self, h: HostNode) -> HostResult<NodeId> {
        let id = self.shared.nodes.borrow().get(h).ok_or(HostError::NotFound)?;
        if self.shared.doc.borrow().kind(id).is_none() {
            return Err(HostError::NotFound);
        }
        Ok(id)
    }

    fn handle_or_none(&self, id: Option<NodeId>) -> Option<HostNode> {
        id.and_then(|i| self.handle_of(i))
    }

    /// Runs `f` with the document taken out of the cell; listeners run through the swapping callback (module comment).
    fn with_doc<R>(&self, f: impl FnOnce(&mut Document, &mut Invoke<'_>) -> R) -> R {
        let shared = Rc::clone(&self.shared);
        if shared.depth.get() == 0 {
            *shared.key.borrow_mut() = None;
        }
        shared.depth.set(shared.depth.get() + 1);
        let mut doc = std::mem::take(&mut *shared.doc.borrow_mut());
        let r = {
            let cb = Rc::clone(&shared);
            let mut inv = move |d: &mut Document, lid: DomListenerId, ev: &mut EventState| -> bool {
                std::mem::swap(&mut *cb.doc.borrow_mut(), d);
                let ok = cb.deliver(lid, ev);
                std::mem::swap(&mut *cb.doc.borrow_mut(), d);
                ok
            };
            f(&mut doc, &mut inv)
        };
        *shared.doc.borrow_mut() = doc;
        shared.depth.set(shared.depth.get() - 1);
        if shared.depth.get() == 0 {
            let sink = shared.sink.borrow().clone();
            if let Some(sink) = sink { sink.finish_dispatch(); }
        }
        r
    }
}

impl Shared {
    /// Delivers one listener invocation to the sink and applies its outcome to the vn-dom event state.
    /// Returns false when the listener threw (vn-dom continues dispatching either way).
    fn deliver(&self, lid: DomListenerId, ev: &mut EventState) -> bool {
        let sink = self.sink.borrow().clone();
        let Some(sink) = sink else { return true };
        let key = EventKey {
            addr: &*ev as *const EventState as usize,
            ty: ev.init.ty.clone(),
            target: ev.target,
            time_stamp: ev.init.time_stamp.to_bits(),
        };
        {
            let mut last = self.key.borrow_mut();
            if last.as_ref() != Some(&key) {
                self.serial.set(self.serial.get() + 1);
                *last = Some(key);
            }
        }
        self.info.set(EventInfo { serial: self.serial.get(), kind: ev.init.kind });
        let hev = self.host_event(ev);
        let out = sink.dispatch(ListenerId(lid.0), &hev);
        if out.stop_immediate {
            ev.stop_immediate_propagation();
        } else if out.stop_propagation {
            ev.stop_propagation();
        }
        if out.prevent_default {
            ev.prevent_default();
        }
        !out.threw
    }

    fn host_event(&self, ev: &EventState) -> HostEvent {
        let nodes = self.nodes.borrow();
        let handle = |id: NodeId| nodes.handle(id);
        HostEvent {
            ty: ev.init.ty.clone(),
            target: handle(ev.target),
            current_target: ev.current_target.and_then(|id| handle(id)),
            phase: ev.phase,
            bubbles: ev.init.bubbles,
            cancelable: ev.init.cancelable,
            composed: ev.init.composed,
            default_prevented: ev.default_prevented(),
            time_stamp: ev.init.time_stamp,
            key: ev.init.key.clone(),
            code: ev.init.code.clone(),
            ctrl_key: ev.init.ctrl_key,
            shift_key: ev.init.shift_key,
            alt_key: ev.init.alt_key,
            meta_key: ev.init.meta_key,
            button: ev.init.button,
            client_x: f64::from(ev.init.client_x),
            client_y: f64::from(ev.init.client_y),
            input_data: ev.init.input_data.clone(),
            is_composing: ev.init.is_composing,
            detail: ev.init.detail,
        }
    }
}

impl Host for DocHost {
    fn event_info(&self) -> EventInfo {
        DocHost::event_info(self)
    }
    fn add_release_hook(&self, hook: Rc<dyn Fn(HostNode)>) {
        DocHost::add_release_hook(self, hook)
    }
    fn click(&mut self, node: HostNode) -> HostResult<()> {
        DocHost::click(self, node)
    }
    fn document_node(&self) -> Option<HostNode> {
        Some(DocHost::document_node(self))
    }
    fn dispatch_event(&mut self, target: HostNode, init: EventInit) -> HostResult<bool> {
        DocHost::dispatch_event(self, target, init)
    }
    fn set_event_sink(&mut self, sink: Rc<dyn EventSink>) {
        *self.shared.sink.borrow_mut() = Some(sink);
    }

    fn now_ms(&self) -> f64 {
        self.shared.now.get()
    }

    fn create_element(&mut self, ns: HostNs, tag: &str, is: Option<&str>) -> HostResult<HostNode> {
        let id = self.shared.doc.borrow_mut().create_element(map_ns(ns), tag, is).map_err(map_err)?;
        Ok(self.shared.nodes.borrow_mut().add(id))
    }

    fn create_text(&mut self, text: &str) -> HostNode {
        let id = self.shared.doc.borrow_mut().create_text(text);
        self.shared.nodes.borrow_mut().add(id)
    }

    fn create_comment(&mut self, text: &str) -> HostNode {
        let id = self.shared.doc.borrow_mut().create_comment(text);
        self.shared.nodes.borrow_mut().add(id)
    }

    fn set_text(&mut self, node: HostNode, text: &str) -> HostResult<()> {
        let id = self.id(node)?;
        self.shared.doc.borrow_mut().set_node_value(id, text).map_err(map_err)
    }

    fn set_element_text(&mut self, el: HostNode, text: &str) -> HostResult<()> {
        let id = self.id(el)?;
        let children = self.shared.doc.borrow().child_nodes(id);
        self.with_doc(|d, inv| d.set_text_content(id, text, inv)).map_err(map_err)?;
        for child in children {
            if self.shared.nodes.borrow().handle(child).is_none() {
                let _ = self.shared.doc.borrow_mut().release(child);
            }
        }
        Ok(())
    }

    fn insert(&mut self, child: HostNode, parent: HostNode, anchor: Option<HostNode>) -> HostResult<()> {
        let c = self.id(child)?;
        let p = self.id(parent)?;
        let a = match anchor {
            Some(h) => Some(self.id(h)?),
            None => None,
        };
        self.with_doc(|d, inv| d.insert_before(p, c, a, inv)).map_err(map_err)
    }

    fn remove(&mut self, child: HostNode) -> HostResult<()> {
        let c = self.id(child)?;
        self.with_doc(|d, inv| d.detach(c, inv)).map_err(map_err)
    }

    fn parent_node(&self, node: HostNode) -> Option<HostNode> {
        let id = self.id(node).ok()?;
        let p = self.shared.doc.borrow().parent_node(id);
        self.handle_or_none(p)
    }

    fn next_sibling(&self, node: HostNode) -> Option<HostNode> {
        let id = self.id(node).ok()?;
        let s = self.shared.doc.borrow().next_sibling(id);
        self.handle_or_none(s)
    }

    fn query_selector(&self, selector: &str) -> HostResult<Option<HostNode>> {
        let root = self.shared.doc.borrow().document();
        let found = self.shared.doc.borrow().query_selector(root, selector).map_err(map_err)?;
        Ok(self.handle_or_none(found))
    }

    fn body(&self) -> Option<HostNode> {
        self.shared.body.get()
    }

    fn subtree_nodes(&self, node: HostNode) -> Vec<HostNode> {
        let Ok(id) = self.id(node) else { return Vec::new() };
        let doc = self.shared.doc.borrow();
        let mut ids = vec![id];
        let mut i = 0;
        while i < ids.len() { ids.extend(doc.child_nodes(ids[i])); i += 1; }
        let nodes = self.shared.nodes.borrow();
        ids.into_iter().filter_map(|id| nodes.handle(id)).collect()
    }

    fn release(&mut self, node: HostNode) {
        let Ok(id) = self.id(node) else { return };
        // vn-dom frees the whole detached subtree: collect its ids first, the table forgets them afterwards
        let subtree = {
            let doc = self.shared.doc.borrow();
            let mut all = vec![id];
            let mut i = 0;
            while i < all.len() {
                let kids = doc.child_nodes(all[i]);
                all.extend(kids);
                i += 1;
            }
            all
        };
        if self.shared.doc.borrow_mut().release(id).is_ok() {
            let freed: Vec<HostNode> = {
                let nodes = self.shared.nodes.borrow();
                subtree.iter().filter_map(|n| nodes.handle(*n)).collect()
            };
            for h in &freed {
                self.shared.nodes.borrow_mut().free(*h);
            }
            let hooks = self.shared.release_hooks.borrow().clone();
            for h in freed {
                for hook in &hooks {
                    hook(h);
                }
            }
        }
    }

    fn tag_name(&self, el: HostNode) -> String {
        match self.id(el) {
            Ok(id) => self.shared.doc.borrow().tag_name(id).unwrap_or_default(),
            Err(_) => String::new(),
        }
    }

    fn is_element(&self, node: HostNode) -> bool {
        match self.id(node) {
            Ok(id) => self.shared.doc.borrow().kind(id) == Some(NodeKind::Element),
            Err(_) => false,
        }
    }

    fn set_attribute(&mut self, el: HostNode, name: &str, value: &str) -> HostResult<()> {
        let id = self.id(el)?;
        self.shared.doc.borrow_mut().set_attribute(id, name, value).map_err(map_err)
    }

    fn set_attribute_ns(&mut self, el: HostNode, ns: Option<&str>, qualified: &str, value: &str) -> HostResult<()> {
        let id = self.id(el)?;
        self.shared.doc.borrow_mut().set_attribute_ns(id, ns, qualified, value).map_err(map_err)
    }

    fn remove_attribute(&mut self, el: HostNode, name: &str) -> HostResult<()> {
        let id = self.id(el)?;
        self.shared.doc.borrow_mut().remove_attribute(id, name).map_err(map_err)
    }

    fn remove_attribute_ns(&mut self, el: HostNode, ns: Option<&str>, local: &str) -> HostResult<()> {
        let name = prefixed_name(ns, local)?;
        self.remove_attribute(el, &name)
    }

    fn get_attribute(&self, el: HostNode, name: &str) -> Option<String> {
        let id = self.id(el).ok()?;
        let r = self.shared.doc.borrow_mut().get_attribute(id, name);
        r.ok().flatten()
    }

    fn has_attribute(&self, el: HostNode, name: &str) -> bool {
        let Ok(id) = self.id(el) else { return false };
        let r = self.shared.doc.borrow_mut().has_attribute(id, name);
        r.unwrap_or(false)
    }

    fn has_property(&self, el: HostNode, name: &str) -> bool {
        match self.id(el) {
            Ok(id) => self.shared.doc.borrow().has_property(id, name),
            Err(_) => false,
        }
    }

    fn get_property(&mut self, el: HostNode, name: &str) -> HostResult<HostValue> {
        let id = self.id(el)?;
        let v = self.shared.doc.borrow_mut().get_property(id, name).map_err(map_err)?;
        Ok(from_dom_value(v))
    }

    fn set_property(&mut self, el: HostNode, name: &str, value: HostValue) -> HostResult<()> {
        let id = self.id(el)?;
        let v = to_dom_value(value);
        self.with_doc(|d, inv| d.set_property(id, name, v, inv)).map_err(map_err)
    }

    fn style_set_property(&mut self, el: HostNode, name: &str, value: &str, important: bool) -> HostResult<()> {
        let id = self.id(el)?;
        self.shared.doc.borrow_mut().style_set_property(id, name, value, important).map_err(map_err)
    }

    fn style_remove_property(&mut self, el: HostNode, name: &str) -> HostResult<()> {
        let id = self.id(el)?;
        self.shared.doc.borrow_mut().style_remove_property(id, name).map(|_| ()).map_err(map_err)
    }

    fn style_get_property(&self, el: HostNode, name: &str) -> String {
        match self.id(el) {
            Ok(id) => self.shared.doc.borrow().style_get_property(id, name).unwrap_or_default(),
            Err(_) => String::new(),
        }
    }

    fn computed_style_property(&self, el: HostNode, name: &str) -> HostResult<String> {
        // Document has no stylesheet/layout engine. Timing properties are inline or their CSS initial value.
        if !matches!(name, "transition-duration" | "transition-delay" | "animation-duration" | "animation-delay") {
            return Err(HostError::Unsupported(format!("computed style: {name}")));
        }
        let value = self.style_get_property(el, name);
        Ok(if value.is_empty() { "0s".to_string() } else { value })
    }
    fn style_css_text(&self, el: HostNode) -> String {
        match self.id(el) {
            Ok(id) => self.shared.doc.borrow().css_text(id).unwrap_or_default(),
            Err(_) => String::new(),
        }
    }

    fn style_set_css_text(&mut self, el: HostNode, text: &str) -> HostResult<()> {
        let id = self.id(el)?;
        self.shared.doc.borrow_mut().set_css_text(id, text).map_err(map_err)
    }

    fn class_list_add(&mut self, el: HostNode, token: &str) -> HostResult<()> {
        let id = self.id(el)?;
        self.shared.doc.borrow_mut().class_list_add(id, &[token]).map_err(map_err)
    }

    fn class_list_remove(&mut self, el: HostNode, token: &str) -> HostResult<()> {
        let id = self.id(el)?;
        self.shared.doc.borrow_mut().class_list_remove(id, &[token]).map_err(map_err)
    }

    fn add_event_listener(&mut self, el: HostNode, ty: &str, id: ListenerId, opts: ListenerOpts) -> HostResult<()> {
        let node = self.id(el)?;
        let o = ListenerOptions { capture: opts.capture, once: opts.once, passive: opts.passive };
        self.shared.doc.borrow_mut().add_event_listener(node, ty, DomListenerId(id.0), o).map_err(map_err)
    }

    fn remove_event_listener(&mut self, el: HostNode, ty: &str, id: ListenerId, capture: bool) -> HostResult<()> {
        let node = self.id(el)?;
        self.shared.doc.borrow_mut().remove_event_listener(node, ty, DomListenerId(id.0), capture).map_err(map_err)
    }

    fn focus(&mut self, el: HostNode) -> HostResult<()> {
        let id = self.id(el)?;
        self.with_doc(|d, inv| d.focus(id, inv)).map_err(map_err)
    }

    fn blur(&mut self, el: HostNode) -> HostResult<()> {
        let id = self.id(el)?;
        self.with_doc(|d, inv| d.blur(id, inv)).map_err(map_err)
    }

    fn active_element(&self) -> Option<HostNode> {
        let a = self.shared.doc.borrow().active_element();
        self.handle_or_none(a)
    }

    fn option_nodes(&self, select: HostNode) -> Vec<HostNode> {
        let Ok(id) = self.id(select) else { return Vec::new() };
        let opts = self.shared.doc.borrow().option_ids(id);
        opts.into_iter().filter_map(|o| self.handle_of(o)).collect()
    }

    fn outer_html(&mut self, node: HostNode) -> Option<String> {
        let id = self.id(node).ok()?;
        self.shared.doc.borrow_mut().outer_html(id)
    }

    fn inner_html(&mut self, node: HostNode) -> Option<String> {
        let id = self.id(node).ok()?;
        self.shared.doc.borrow_mut().inner_html(id)
    }
}

/// Clones of a DocHost share one document: a handle is a clone (re-entrant calls from listeners need no borrow).
impl HostProvider for DocHost {
    fn handle(&self) -> Box<dyn Host> {
        Box::new(self.clone())
    }
}

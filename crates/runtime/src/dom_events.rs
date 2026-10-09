//! DOM event layer of the runtime (WP-RT-D). Behaviour per @vue/runtime-dom 3.5.43: `patchEvent`, `parseName`,
//! `createInvoker` (the `_vei` table, the `attached` / `_vts` timestamp guard, `_stopped` for handler arrays),
//! `withModifiers` and `withKeys`. The listeners of a realm are vn-dom listeners whose `ListenerId` the runtime
//! allocates; `DomCtx` is the realm-level context that owns them (see `DomState`).
//!
//! JS event object. One object per dispatch (the sink's `HostEvent` carries no identity; `DocHost::event_info` does).
//! Fields follow the event kind (Mouse, Keyboard, Input or plain Event) so that `"key" in e` and `"button" in e`
//! answer as in Chrome. `target` and `currentTarget` are element handles: one exotic object per node, whose
//! `get`/`set` hooks forward to the host's properties (`Host::has_property` decides, as `name in el` does).
//!
//! GC. The handler functions, invokers, handles and event objects are heap objects; `DomState` implements
//! `RootProvider` and roots live-node state. Detached subtrees are traced through reachable handles and vnodes.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use vn_dom::EventKind;
use vn_value::{payload_ref, Call, CodeFn, Exotic, Flags, Hook, Intrinsic, JsStr, NativeObject, ObjRef, PropKey, Res, Rt, RootProvider, Throw, Tracer, Value};

use crate::host::{EventInfo, EventSink, Host, HostError, HostEvent, HostNode, HostProvider, HostValue, ListenerId, ListenerOpts, ListenerOutcome, SharedHost};

/// A listener attached for one raw prop name (`el._vei[rawName]`).
#[derive(Clone)]
pub(crate) struct Attached {
    pub lid: ListenerId,
    pub ty: String,
    pub capture: bool,
    pub invoker: ObjRef,
}

/// Per-element state that the runtime keeps beside the element (Vue's expandos), keyed by `HostNode`.
#[derive(Default)]
pub(crate) struct Expando {
    /// `el._vei`: raw event name to the attached invoker.
    pub vei: HashMap<String, Attached>,
    /// `el._value`; `None` until patchDOMProp sets it (Vue tests `"_value" in el`).
    pub value: Option<Value>,
    /// `el._assign`: the v-model update callback.
    pub assign: Option<Value>,
    /// `el._initialValue` (v-model text, between created and mounted).
    pub initial_value: Option<String>,
    /// `el._modelValue` (v-model checkbox and select).
    pub model_value: Option<Value>,
    /// `el._trueValue` / `el._falseValue`.
    pub true_value: Option<Value>,
    pub false_value: Option<Value>,
    /// `el._pendingValue` (v-model select); cleared on the next tick when the token still matches.
    pub pending: Option<PendingSelect>,
    /// `el.composing` (IME composition in progress).
    pub composing: bool,
    /// `el[vShowOriginalDisplay]` (v-show).
    pub vod: Option<String>,
    /// `el[vShowHidden]` (v-show).
    pub vsh: bool,
    /// `el[vtcKey]`: transition classes added by the transition WP, in insertion order.
    pub vtc: Option<Vec<String>>,
}

#[derive(Clone)]
pub(crate) struct PendingSelect {
    pub token: u64,
    pub multiple: bool,
    pub value: Value,
}

struct ListenerRec {
    node: HostNode,
    invoker: ObjRef,
    passive: bool,
}

#[derive(Default)]
pub(crate) struct DomState {
    pub expandos: HashMap<HostNode, Expando>,
    listeners: HashMap<u32, ListenerRec>,
    next_listener: u32,
    next_token: u64,
    current: Option<(u64, ObjRef)>,
    handles: HashMap<HostNode, ObjRef>,
    retired: HashMap<HostNode, std::rc::Weak<DetachedNodes>>,
    pub errors: Vec<Value>,
    pub ticks: Vec<(HostNode, u64)>,
    event_proto: Option<ObjRef>,
    element_proto: Option<ObjRef>,
    realm: Option<ObjRef>,
}

impl DomState {
    fn trace(&self, t: &mut Tracer) {
        for (node, ex) in &self.expandos {
            if self.retired.contains_key(node) { continue; }
            for a in ex.vei.values() {
                t.mark(a.invoker);
            }
            for v in [&ex.value, &ex.assign, &ex.model_value, &ex.true_value, &ex.false_value].into_iter().flatten() {
                t.mark_value(v);
            }
            if let Some(p) = &ex.pending {
                t.mark_value(&p.value);
            }
        }
        for l in self.listeners.values() {
            if !self.retired.contains_key(&l.node) { t.mark(l.invoker); }
        }
        if let Some((_, o)) = self.current {
            t.mark(o);
        }
        for (node, o) in &self.handles {
            if !self.retired.contains_key(node) { t.mark(*o); }
        }
        for v in &self.errors {
            t.mark_value(v);
        }
        for o in [self.event_proto, self.element_proto, self.realm].into_iter().flatten() {
            t.mark(o);
        }
    }
}

struct StateRoots(Rc<RefCell<DomState>>);

impl RootProvider for StateRoots {
    fn trace_roots(&self, t: &mut Tracer) {
        self.0.borrow().trace(t);
    }
}

/// Payload of the realm object: the host and the state that the invokers (and their timestamps) need.
struct RealmRef {
    host: SharedHost,
    state: Rc<RefCell<DomState>>,
}

impl NativeObject for RealmRef {
    fn trace(&self, _t: &mut Tracer) {}
}

/// Payload and exotic hooks of an element handle: the host node plus the host and state it belongs to.
pub(crate) struct HandleData {
    host: SharedHost,
    state: Rc<RefCell<DomState>>,
    node: HostNode,
    detached: RefCell<Option<Rc<DetachedNodes>>>,
}

// The last reachable handle releases the detached subtree. The state's cache holds only weak leases.
struct DetachedNodes {
    host: SharedHost,
    state: Rc<RefCell<DomState>>,
    root: HostNode,
    nodes: HashSet<HostNode>,
    trace_object: Cell<Option<ObjRef>>,
}

impl NativeObject for DetachedNodes {
    fn trace(&self, t: &mut Tracer) { self.trace_nodes(t); }
}

impl DetachedNodes {
    fn trace_nodes(&self, t: &mut Tracer) {
        let st = self.state.borrow();
        for node in &self.nodes {
            if let Some(o) = st.handles.get(node) { t.mark(*o); }
            if let Some(ex) = st.expandos.get(node) {
                for a in ex.vei.values() { t.mark(a.invoker); }
                for v in [&ex.value, &ex.assign, &ex.model_value, &ex.true_value, &ex.false_value].into_iter().flatten() {
                    t.mark_value(v);
                }
                if let Some(p) = &ex.pending { t.mark_value(&p.value); }
            }
        }
        for l in st.listeners.values() {
            if self.nodes.contains(&l.node) { t.mark(l.invoker); }
        }
    }
}

impl Drop for DetachedNodes {
    fn drop(&mut self) { self.host.handle().release(self.root); }
}

impl NativeObject for HandleData {
    fn trace(&self, t: &mut Tracer) {
        if let Some(detached) = self.detached.borrow().as_ref() {
            // Mark the shared lease once instead of retracing it for every handle.
            t.mark(detached.trace_object.get().expect("detached lease object (framework bug)"));
        }
    }
}

impl Exotic for HandleData {
    fn trace(&self, t: &mut Tracer) { NativeObject::trace(self, t); }

    fn get(&self, _rt: &Rt, _obj: ObjRef, key: &PropKey, _receiver: &Value) -> Res<Hook<Value>> {
        let PropKey::Str(name) = key else { return Ok(Hook::Ordinary) };
        let name = name.to_rust_string_lossy();
        let mut host = self.host.handle();
        if !host.has_property(self.node, &name) {
            return Ok(Hook::Ordinary);
        }
        match host.get_property(self.node, &name) {
            Ok(v) => Ok(Hook::Handled(host_to_js(v))),
            // Object-valued properties (style, childNodes, ...) are not modelled as values: ordinary lookup.
            Err(_) => Ok(Hook::Ordinary),
        }
    }

    fn set(&self, rt: &Rt, _obj: ObjRef, key: &PropKey, v: &Value, _receiver: &Value) -> Res<Hook<bool>> {
        let PropKey::Str(name) = key else { return Ok(Hook::Ordinary) };
        let name = name.to_rust_string_lossy();
        let mut host = self.host.handle();
        if !host.has_property(self.node, &name) {
            return Ok(Hook::Ordinary);
        }
        let hv = to_host_value(rt, v)?;
        host.set_property(self.node, &name, hv).map_err(|e| host_error(rt, e))?;
        Ok(Hook::Handled(true))
    }
}

/// Realm-level DOM context: the runtime, the headless host and the state of the DOM layer.
#[derive(Clone)]
pub struct DomCtx {
    pub(crate) rt: Rt,
    pub(crate) host: SharedHost,
    pub(crate) state: Rc<RefCell<DomState>>,
}

struct DomSink {
    rt: vn_value::WeakRt,
    host: std::rc::Weak<dyn crate::host::HostProvider>,
    state: std::rc::Weak<RefCell<DomState>>,
}

impl EventSink for DomSink {
    fn dispatch(&self, listener: ListenerId, event: &HostEvent) -> ListenerOutcome {
        let (Some(rt), Some(host), Some(state)) = (self.rt.upgrade(), self.host.upgrade(), self.state.upgrade()) else {
            return ListenerOutcome::default();
        };
        DomCtx { rt, host, state }.dispatch(listener, event)
    }
    fn finish_dispatch(&self) {
        if let Some(state) = self.state.upgrade() { state.borrow_mut().current = None; }
    }
}

/// The JS-visible flags of one event object (`preventDefault` and friends write here).
struct EventPayload {
    cancelable: bool,
    passive: Cell<bool>,
    stop: Cell<bool>,
    stop_now: Cell<bool>,
    prevent: Cell<bool>,
    path: Vec<HostNode>,
    host: SharedHost,
    state: Rc<RefCell<DomState>>,
}

impl NativeObject for EventPayload {
    fn trace(&self, _t: &mut Tracer) {}
}

pub(crate) fn host_error(rt: &Rt, e: HostError) -> Throw {
    rt.throw_type_error(&match e {
        HostError::Unsupported(m) => format!("unsupported: {m}"),
        HostError::InvalidState(m) => format!("invalid DOM state: {m}"),
        HostError::NotFound => "The object can not be found here.".to_string(),
    })
}

pub(crate) fn host_to_js(v: HostValue) -> Value {
    match v {
        HostValue::Undefined => Value::Undefined,
        HostValue::Null => Value::Null,
        HostValue::Bool(b) => Value::Bool(b),
        HostValue::Number(n) => Value::Number(n),
        HostValue::Str(s) => Value::String(JsStr::from_str(&s)),
    }
}

/// ToString for host arguments; a Symbol throws as it does for DOM string arguments.
pub(crate) fn to_host_value(rt: &Rt, v: &Value) -> Res<HostValue> {
    Ok(match v {
        Value::Undefined => HostValue::Undefined,
        Value::Null => HostValue::Null,
        Value::Bool(b) => HostValue::Bool(*b),
        Value::Number(n) => HostValue::Number(*n),
        Value::String(s) => HostValue::Str(s.to_rust_string_lossy()),
        Value::Symbol(_) => return Err(rt.throw_type_error("Cannot convert a Symbol value to a string")),
        other => HostValue::Str(js_string(rt, other)?),
    })
}

/// `String(v)` as a Rust string (lossy for lone surrogates, the boundary rule of host.rs).
pub(crate) fn js_string(rt: &Rt, v: &Value) -> Res<String> {
    Ok(rt.to_string(v)?.to_rust_string_lossy())
}

fn js_str(s: &str) -> Value {
    Value::String(JsStr::from_str(s))
}

/// The elements of a JS array value (holes read as undefined), or none for other values.
pub(crate) fn array_items(rt: &Rt, v: &Value) -> Res<Vec<Value>> {
    if !rt.is_array(v) {
        return Ok(Vec::new());
    }
    let len = rt.to_number(&rt.get_str(v, "length")?)? as u32;
    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len {
        out.push(rt.get_v(v, &PropKey::Index(i))?);
    }
    Ok(out)
}

/// Vue's `parseName`: strips trailing `Once` / `Passive` / `Capture` modifiers (unless the name is `on` plus one of
/// them) and derives the DOM event type (`onKeyDown` becomes `keydown`, `on:foo` becomes `foo`).
pub(crate) fn parse_name(raw: &str) -> (String, ListenerOpts) {
    let mut name = raw.to_string();
    let mut opts = ListenerOpts::default();
    loop {
        let found = ["Once", "Passive", "Capture"].into_iter().find(|s| name.ends_with(s));
        let Some(suffix) = found else { break };
        if is_option_modifier_event(&name) {
            break;
        }
        match suffix {
            "Once" => opts.once = true,
            "Passive" => opts.passive = true,
            _ => opts.capture = true,
        }
        name.truncate(name.len() - suffix.len());
    }
    let b = name.as_bytes();
    let event = if b.len() > 2 && b[2] == b':' { name[3..].to_string() } else { hyphenate(&name[2..]) };
    (event, opts)
}

/// `/^on:?(?:Once|Passive|Capture)$/`
fn is_option_modifier_event(name: &str) -> bool {
    let rest = match name.strip_prefix("on") {
        Some(r) => r.strip_prefix(':').unwrap_or(r),
        None => return false,
    };
    matches!(rest, "Once" | "Passive" | "Capture")
}

/// @vue/shared `hyphenate`: `replace(/\B([A-Z])/g, '-$1').toLowerCase()`.
pub(crate) fn hyphenate(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let mut prev_word = false;
    for c in s.chars() {
        if c.is_ascii_uppercase() && prev_word {
            out.push('-');
        }
        out.push(c.to_ascii_lowercase());
        prev_word = c.is_ascii_alphanumeric() || c == '_';
    }
    out
}

/// Realm and host of a realm object (the invoker's data slot).
fn realm_parts(rt: &Rt, v: &Value) -> Option<(SharedHost, Rc<RefCell<DomState>>)> {
    let o = v.as_object()?;
    let p = rt.native_payload(o)?;
    let r = payload_ref::<RealmRef>(&*p)?;
    Some((r.host.clone(), Rc::clone(&r.state)))
}

/// Element handle parts of a JS value (`this` of the element methods, or the `el` a directive receives).
pub(crate) fn ctx_of_handle(rt: &Rt, v: &Value) -> Res<(DomCtx, HostNode)> {
    let bad = || rt.throw_type_error("Illegal invocation: not an element handle");
    let o = v.as_object().ok_or_else(bad)?;
    let ext = rt.ext(o).ok_or_else(bad)?;
    let h = payload_ref::<HandleData>(&*ext).ok_or_else(bad)?;
    Ok((DomCtx { rt: rt.clone(), host: h.host.clone(), state: Rc::clone(&h.state) }, h.node))
}

/// Invokers call the handler (or each handler of an array) through `callWithAsyncErrorHandling`: a throw is reported
/// (recorded in `DomState::errors`, as Vue's prod `handleError` logs it) and does not stop the dispatch.
fn call_with_async_error_handling(rt: &Rt, state: &Rc<RefCell<DomState>>, f: &Value, args: &[Value]) {
    if rt.is_callable(f) {
        if let Err(Throw(err)) = rt.call(f, &Value::Undefined, args) {
            state.borrow_mut().errors.push(err);
        }
    } else if rt.is_array(f) {
        let items = array_items(rt, f).unwrap_or_default();
        for item in items {
            call_with_async_error_handling(rt, state, &item, args);
        }
    }
}

/// `createInvoker`'s returned function: `(e) => { timestamp guard; handler(s)(e) }`.
fn invoker_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let Some((host, state)) = realm_parts(rt, &data[0]) else {
        return Err(rt.throw_type_error("invoker without a realm"));
    };
    let e = c.arg(0);
    let invoker = Value::Object(c.callee);
    let vts = rt.get_str(&e, "_vts")?;
    if !vts.to_boolean() {
        rt.set_str(&e, "_vts", Value::Number(host.handle().now_ms()))?;
    } else {
        let attached = rt.get_str(&invoker, "attached")?;
        if rt.to_number(&vts)? <= rt.to_number(&attached)? {
            return Ok(Value::Undefined);
        }
    }
    let value = rt.get_str(&invoker, "value")?;
    if rt.is_array(&value) {
        let original = rt.get_str(&e, "stopImmediatePropagation")?;
        let wrapper = rt.new_native_function(stop_immediate_wrapper_code, "stopImmediatePropagation", 0, vec![e.clone(), original]);
        rt.set_str(&e, "stopImmediatePropagation", Value::Object(wrapper))?;
        let handlers = array_items(rt, &value)?;
        for h in handlers {
            if rt.get_str(&e, "_stopped")?.to_boolean() {
                break;
            }
            if h.to_boolean() {
                call_with_async_error_handling(rt, &state, &h, std::slice::from_ref(&e));
            }
        }
    } else {
        call_with_async_error_handling(rt, &state, &value, std::slice::from_ref(&e));
    }
    Ok(Value::Undefined)
}

/// `e.stopImmediatePropagation = () => { originalStop.call(e); e._stopped = true }` (Vue's array-handler wrapper).
fn stop_immediate_wrapper_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let (e, original) = (data[0].clone(), data[1].clone());
    rt.call(&original, &e, &[])?;
    rt.set_str(&e, "_stopped", Value::Bool(true))?;
    Ok(Value::Undefined)
}

fn event_payload(rt: &Rt, this: &Value) -> Res<Rc<dyn NativeObject>> {
    let bad = || rt.throw_type_error("Illegal invocation");
    let o = this.as_object().ok_or_else(bad)?;
    rt.native_payload(o).ok_or_else(bad)
}

fn prevent_default_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = event_payload(rt, c.this)?;
    if let Some(ep) = payload_ref::<EventPayload>(&*p) {
        if ep.cancelable && !ep.passive.get() {
            ep.prevent.set(true);
            rt.set_str(c.this, "defaultPrevented", Value::Bool(true))?;
        }
    }
    Ok(Value::Undefined)
}

fn stop_propagation_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = event_payload(rt, c.this)?;
    if let Some(ep) = payload_ref::<EventPayload>(&*p) {
        ep.stop.set(true);
    }
    Ok(Value::Undefined)
}

fn stop_immediate_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = event_payload(rt, c.this)?;
    if let Some(ep) = payload_ref::<EventPayload>(&*p) {
        ep.stop.set(true);
        ep.stop_now.set(true);
    }
    Ok(Value::Undefined)
}

fn composed_path_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = event_payload(rt, c.this)?;
    let items: Vec<Value> = match payload_ref::<EventPayload>(&*p) {
        Some(ep) => ep.path.iter().map(|&n| handle_in(rt, &ep.host, &ep.state, n)).collect(),
        None => Vec::new(),
    };
    Ok(Value::Object(rt.new_array(items)))
}

/// The element handle of `node` in a realm (one object per node; see `DomCtx::handle`).
pub(crate) fn handle_in(rt: &Rt, host: &SharedHost, state: &Rc<RefCell<DomState>>, node: HostNode) -> Value {
    let cached = state.borrow().handles.get(&node).copied();
    if let Some(o) = cached {
        return Value::Object(o);
    }
    let proto = state.borrow().element_proto;
    let detached = state.borrow().retired.get(&node).and_then(|w| w.upgrade());
    let data = Rc::new(HandleData { host: host.clone(), state: Rc::clone(state), node, detached: RefCell::new(detached) });
    let exotic: Rc<dyn Exotic> = data.clone();
    let obj = rt.new_exotic_object(proto, exotic);
    let ext: Rc<dyn NativeObject> = data;
    rt.set_ext(obj, ext);
    state.borrow_mut().handles.insert(node, obj);
    Value::Object(obj)
}

fn this_handle_parts(rt: &Rt, this: &Value) -> Res<(SharedHost, HostNode)> {
    let (ctx, node) = ctx_of_handle(rt, this)?;
    Ok((ctx.host, node))
}

fn get_attribute_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (host, node) = this_handle_parts(rt, c.this)?;
    let name = js_string(rt, &c.arg(0))?;
    Ok(match host.handle().get_attribute(node, &name) {
        Some(v) => js_str(&v),
        None => Value::Null,
    })
}

fn set_attribute_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (host, node) = this_handle_parts(rt, c.this)?;
    let name = js_string(rt, &c.arg(0))?;
    let value = js_string(rt, &c.arg(1))?;
    host.handle().set_attribute(node, &name, &value).map_err(|e| host_error(rt, e))?;
    Ok(Value::Undefined)
}

fn remove_attribute_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (host, node) = this_handle_parts(rt, c.this)?;
    let name = js_string(rt, &c.arg(0))?;
    host.handle().remove_attribute(node, &name).map_err(|e| host_error(rt, e))?;
    Ok(Value::Undefined)
}

fn has_attribute_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (host, node) = this_handle_parts(rt, c.this)?;
    let name = js_string(rt, &c.arg(0))?;
    Ok(Value::Bool(host.handle().has_attribute(node, &name)))
}

fn focus_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (host, node) = this_handle_parts(rt, c.this)?;
    host.handle().focus(node).map_err(|e| host_error(rt, e))?;
    Ok(Value::Undefined)
}

fn blur_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (host, node) = this_handle_parts(rt, c.this)?;
    host.handle().blur(node).map_err(|e| host_error(rt, e))?;
    Ok(Value::Undefined)
}

fn click_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (host, node) = this_handle_parts(rt, c.this)?;
    host.handle().click(node).map_err(|e| host_error(rt, e))?;
    Ok(Value::Undefined)
}

impl DomCtx {
    /// A DOM context for `host` in the realm `rt`: installs the event prototype, the element prototype, the realm
    /// object, the GC roots, and the event sink of `host`. One context per host.
    pub fn new<H: HostProvider + Clone + 'static>(rt: &Rt, host: &H) -> DomCtx {
        let host: SharedHost = Rc::new(host.clone());
        let object_proto = rt.intrinsic(Intrinsic::ObjectPrototype);
        let event_proto = rt.new_object_with_proto(Some(object_proto));
        let methods: [(&str, CodeFn, u32); 4] = [
            ("preventDefault", prevent_default_code, 0),
            ("stopPropagation", stop_propagation_code, 0),
            ("stopImmediatePropagation", stop_immediate_code, 0),
            ("composedPath", composed_path_code, 0),
        ];
        for (name, code, len) in methods {
            rt.define_native_method(event_proto, name, code, len);
        }
        let element_proto = rt.new_object_with_proto(Some(object_proto));
        let methods: [(&str, CodeFn, u32); 7] = [
            ("getAttribute", get_attribute_code, 1),
            ("setAttribute", set_attribute_code, 2),
            ("removeAttribute", remove_attribute_code, 1),
            ("hasAttribute", has_attribute_code, 1),
            ("focus", focus_code, 0),
            ("blur", blur_code, 0),
            ("click", click_code, 0),
        ];
        for (name, code, len) in methods {
            rt.define_native_method(element_proto, name, code, len);
        }
        let state = Rc::new(RefCell::new(DomState {
            event_proto: Some(event_proto),
            element_proto: Some(element_proto),
            ..DomState::default()
        }));
        let realm = rt.new_native_object(None, Rc::new(RealmRef { host: host.clone(), state: Rc::clone(&state) }));
        state.borrow_mut().realm = Some(realm);
        rt.add_root_provider(Rc::new(StateRoots(Rc::clone(&state))));
        // a released node takes its expandos, handle, and listeners with it (the host frees the subtree)
        let released = Rc::clone(&state);
        host.handle().add_release_hook(Rc::new(move |node: HostNode| {
            let mut st = released.borrow_mut();
            st.retired.remove(&node);
            st.ticks.retain(|(n, _)| *n != node);
            st.expandos.remove(&node);
            st.handles.remove(&node);
            st.listeners.retain(|_, rec| rec.node != node);
        }));
        let ctx = DomCtx { rt: rt.clone(), host: host.clone(), state };
        let mut h = host.handle();
        h.set_event_sink(Rc::new(DomSink {
            rt: rt.downgrade(), host: Rc::downgrade(&ctx.host), state: Rc::downgrade(&ctx.state),
        }));
        ctx
    }

    pub fn rt(&self) -> &Rt {
        &self.rt
    }

    pub(crate) fn retire(&self, node: HostNode) {
        if self.state.borrow().retired.contains_key(&node) { return; }
        let nodes = self.host().subtree_nodes(node).into_iter().collect();
        let detached = Rc::new(DetachedNodes {
            host: self.host.clone(), state: self.state.clone(), root: node, nodes, trace_object: Cell::new(None),
        });
        let object = self.rt.new_native_object(None, detached.clone());
        detached.trace_object.set(Some(object));
        let handles = {
            let mut st = self.state.borrow_mut();
            for n in &detached.nodes { st.retired.insert(*n, Rc::downgrade(&detached)); }
            detached.nodes.iter().filter_map(|n| st.handles.get(n).copied()).collect::<Vec<_>>()
        };
        for o in handles {
            if let Some(p) = self.rt.ext(o) {
                if let Some(handle) = payload_ref::<HandleData>(&*p) {
                    *handle.detached.borrow_mut() = Some(detached.clone());
                }
            }
        }
    }

    pub fn host(&self) -> Box<dyn Host> {
        self.host.handle()
    }

    /// The element handle of a node (one object per node, so `e.target === el` holds).
    pub fn handle(&self, node: HostNode) -> Value {
        handle_in(&self.rt, &self.host, &self.state, node)
    }

    fn handle_or_null(&self, node: Option<HostNode>) -> Value {
        match node {
            Some(n) => self.handle(n),
            None => Value::Null,
        }
    }

    /// Errors thrown by listeners (the handler errors Vue reports through `handleError`). Clears the list.
    pub fn take_errors(&self) -> Vec<Value> {
        std::mem::take(&mut self.state.borrow_mut().errors)
    }

    /// Number of live listeners of this realm (tests and diagnostics).
    pub fn listener_count(&self) -> usize {
        self.state.borrow().listeners.len()
    }

    /// Entries of the side tables (expandos, element handles) kept for live nodes (tests and diagnostics).
    pub fn side_table_entries(&self) -> (usize, usize) {
        let st = self.state.borrow();
        (st.expandos.len(), st.handles.len())
    }

    /// Runs the `nextTick` callbacks queued by v-model select: clears `_pendingValue` when it still is the same token.
    pub fn flush_next_tick(&self) {
        let ticks = std::mem::take(&mut self.state.borrow_mut().ticks);
        let mut st = self.state.borrow_mut();
        for (node, token) in ticks {
            if let Some(ex) = st.expandos.get_mut(&node) {
                if ex.pending.as_ref().map(|p| p.token) == Some(token) {
                    ex.pending = None;
                }
            }
        }
    }

    /// Queues the `nextTick` clear of `_pendingValue` for `node`; returns the token that identifies the pending value.
    pub(crate) fn queue_pending_clear(&self, node: HostNode, token: u64) {
        self.state.borrow_mut().ticks.push((node, token));
    }

    pub(crate) fn next_token(&self) -> u64 {
        let mut st = self.state.borrow_mut();
        st.next_token += 1;
        st.next_token
    }

    /// Reads or updates the expando state of an element.
    pub(crate) fn with_expando<R>(&self, node: HostNode, f: impl FnOnce(&mut Expando) -> R) -> R {
        let mut st = self.state.borrow_mut();
        f(st.expandos.entry(node).or_default())
    }

    /// `createInvoker`: a native function with the current handler in `value` and the attach time in `attached`.
    fn create_invoker(&self, initial: &Value) -> Res<ObjRef> {
        let realm = self.state.borrow().realm.expect("realm object");
        let f = self.rt.new_native_function(invoker_code, "invoker", 1, vec![Value::Object(realm)]);
        let fv = Value::Object(f);
        self.rt.set_str(&fv, "value", initial.clone())?;
        self.rt.set_str(&fv, "attached", Value::Number(self.host.handle().now_ms()))?;
        Ok(f)
    }

    /// Registers a listener for `fnobj` on `el` and returns its id. The sink calls `fnobj` with the event object.
    pub(crate) fn add_listener(&self, el: HostNode, ty: &str, fnobj: ObjRef, opts: ListenerOpts) -> Res<ListenerId> {
        let lid = {
            let mut st = self.state.borrow_mut();
            st.next_listener += 1;
            ListenerId(st.next_listener)
        };
        self.state.borrow_mut().listeners.insert(lid.0, ListenerRec { node: el, invoker: fnobj, passive: opts.passive });
        let mut host = self.host.handle();
        host.add_event_listener(el, ty, lid, opts).map_err(|e| host_error(&self.rt, e))?;
        Ok(lid)
    }

    pub(crate) fn remove_listener(&self, el: HostNode, att: &Attached) -> Res<()> {
        self.state.borrow_mut().listeners.remove(&att.lid.0);
        let mut host = self.host.handle();
        host.remove_event_listener(el, &att.ty, att.lid, att.capture).map_err(|e| host_error(&self.rt, e))
    }

    /// `patchEvent(el, rawName, prev, next)`: updates the invoker of an attached event, or attaches or detaches one.
    pub(crate) fn patch_event(&self, el: HostNode, raw: &str, next: &Value) -> Res<()> {
        let existing = self.with_expando(el, |ex| ex.vei.get(raw).cloned());
        if next.to_boolean() {
            if let Some(att) = existing {
                self.rt.set_str(&Value::Object(att.invoker), "value", next.clone())?;
                return Ok(());
            }
        }
        let (name, opts) = parse_name(raw);
        if next.to_boolean() {
            let invoker = self.create_invoker(next)?;
            let lid = self.add_listener(el, &name, invoker, opts)?;
            let att = Attached { lid, ty: name.clone(), capture: opts.capture, invoker };
            self.with_expando(el, |ex| {
                ex.vei.insert(raw.to_string(), att);
            });
        } else if let Some(att) = existing {
            self.remove_listener(el, &att)?;
            self.with_expando(el, |ex| {
                ex.vei.remove(raw);
            });
        }
        Ok(())
    }

    /// The JS event object of the current dispatch (created on the first listener of a dispatch).
    fn event_for(&self, info: EventInfo, hev: &HostEvent) -> Res<ObjRef> {
        let cached = self.state.borrow().current.filter(|(s, _)| *s == info.serial).map(|(_, o)| o);
        if let Some(o) = cached {
            return Ok(o);
        }
        let o = self.make_event(info, hev)?;
        self.state.borrow_mut().current = Some((info.serial, o));
        Ok(o)
    }

    fn make_event(&self, info: EventInfo, hev: &HostEvent) -> Res<ObjRef> {
        let proto = self.state.borrow().event_proto;
        let mut path = Vec::new();
        let mut cur = hev.target;
        while let Some(n) = cur {
            path.push(n);
            cur = self.host.handle().parent_node(n);
        }
        let payload = EventPayload {
            cancelable: hev.cancelable,
            passive: Cell::new(false),
            stop: Cell::new(false),
            stop_now: Cell::new(false),
            prevent: Cell::new(false),
            path,
            host: self.host.clone(),
            state: Rc::clone(&self.state),
        };
        let rt = &self.rt;
        let obj = rt.new_native_object(proto, Rc::new(payload));
        let put = |name: &str, v: Value| {
            let _ = rt.define_data(obj, PropKey::from_str(name), v, Flags::ALL);
        };
        put("type", js_str(&hev.ty));
        put("bubbles", Value::Bool(hev.bubbles));
        put("cancelable", Value::Bool(hev.cancelable));
        put("composed", Value::Bool(hev.composed));
        put("isTrusted", Value::Bool(true));
        put("timeStamp", Value::Number(hev.time_stamp));
        put("eventPhase", Value::Number(f64::from(hev.phase)));
        put("target", self.handle_or_null(hev.target));
        put("currentTarget", self.handle_or_null(hev.current_target));
        put("defaultPrevented", Value::Bool(hev.default_prevented));
        match info.kind {
            EventKind::Mouse => {
                put("button", Value::Number(f64::from(hev.button)));
                put("clientX", Value::Number(hev.client_x));
                put("clientY", Value::Number(hev.client_y));
                put("ctrlKey", Value::Bool(hev.ctrl_key));
                put("shiftKey", Value::Bool(hev.shift_key));
                put("altKey", Value::Bool(hev.alt_key));
                put("metaKey", Value::Bool(hev.meta_key));
                put("detail", Value::Number(f64::from(hev.detail)));
            }
            EventKind::Keyboard => {
                put("key", js_str(&hev.key));
                put("code", js_str(&hev.code));
                put("ctrlKey", Value::Bool(hev.ctrl_key));
                put("shiftKey", Value::Bool(hev.shift_key));
                put("altKey", Value::Bool(hev.alt_key));
                put("metaKey", Value::Bool(hev.meta_key));
                put("isComposing", Value::Bool(hev.is_composing));
                put("detail", Value::Number(f64::from(hev.detail)));
            }
            EventKind::Input => {
                put("data", match &hev.input_data {
                    Some(d) => js_str(d),
                    None => Value::Null,
                });
                put("isComposing", Value::Bool(hev.is_composing));
                put("detail", Value::Number(f64::from(hev.detail)));
            }
            EventKind::Event => {}
        }
        Ok(obj)
    }

    /// Delivers one listener invocation: the listener's invoker runs with the dispatch's event object, and the flags
    /// the handlers set through the event object become the outcome for vn-dom.
    fn dispatch(&self, listener: ListenerId, hev: &HostEvent) -> ListenerOutcome {
        let rec = self.state.borrow().listeners.get(&listener.0).map(|r| (r.invoker, r.passive));
        let Some((invoker, passive)) = rec else { return ListenerOutcome::default() };
        let info = self.host.handle().event_info();
        let ev = match self.event_for(info, hev) {
            Ok(ev) => ev,
            Err(_) => return ListenerOutcome { threw: true, ..ListenerOutcome::default() },
        };
        let evv = Value::Object(ev);
        let payload = self.rt.native_payload(ev);
        if let Some(p) = payload.as_ref().and_then(|p| payload_ref::<EventPayload>(&**p)) {
            p.passive.set(passive);
        }
        let _ = self.rt.set_str(&evv, "eventPhase", Value::Number(f64::from(hev.phase)));
        let _ = self.rt.set_str(&evv, "currentTarget", self.handle_or_null(hev.current_target));
        let _ = self.rt.set_str(&evv, "defaultPrevented", Value::Bool(hev.default_prevented));
        let r = self.rt.call(&Value::Object(invoker), &Value::Undefined, std::slice::from_ref(&evv));
        let threw = match r {
            Ok(_) => false,
            Err(Throw(e)) => {
                self.state.borrow_mut().errors.push(e);
                true
            }
        };
        let (stop, stop_now, prevent) = match payload.as_ref().and_then(|p| payload_ref::<EventPayload>(&**p)) {
            Some(p) => (p.stop.get(), p.stop_now.get(), p.prevent.get()),
            None => (false, false, false),
        };
        ListenerOutcome { stop_propagation: stop, stop_immediate: stop_now, prevent_default: prevent, threw }
    }
}

// ---- withModifiers / withKeys (runtime-dom 3.5.43) ----

const SYSTEM_MODIFIERS: [&str; 4] = ["ctrl", "shift", "alt", "meta"];

/// `modifiers.join(".")`-style names of a modifiers array.
fn modifier_names(rt: &Rt, mods: &Value) -> Res<Vec<String>> {
    array_items(rt, mods)?.iter().map(|m| js_string(rt, m)).collect()
}

/// The cache object on `fn` (`fn._withMods || (fn._withMods = {})`); primitives are not cached.
fn cache_on(rt: &Rt, f: &Value, prop: &str) -> Res<Option<Value>> {
    if !matches!(f, Value::Object(_)) {
        return Ok(None);
    }
    let existing = rt.get_str(f, prop)?;
    if existing.is_object() {
        return Ok(Some(existing));
    }
    let cache = Value::Object(rt.new_object());
    rt.set_str(f, prop, cache.clone())?;
    Ok(Some(cache))
}

/// `modifierGuards[name](event, modifiers)`: true when the guard drops the event.
fn modifier_guard(rt: &Rt, name: &str, event: &Value, names: &[String]) -> Res<bool> {
    let flag = |rt: &Rt, key: &str| -> Res<bool> { Ok(rt.get_str(event, key)?.to_boolean()) };
    Ok(match name {
        "stop" => {
            let f = rt.get_str(event, "stopPropagation")?;
            rt.call(&f, event, &[])?;
            false
        }
        "prevent" => {
            let f = rt.get_str(event, "preventDefault")?;
            rt.call(&f, event, &[])?;
            false
        }
        "self" => {
            let target = rt.get_str(event, "target")?;
            let current = rt.get_str(event, "currentTarget")?;
            !vn_value::strict_eq(&target, &current)
        }
        "ctrl" => !flag(rt, "ctrlKey")?,
        "shift" => !flag(rt, "shiftKey")?,
        "alt" => !flag(rt, "altKey")?,
        "meta" => !flag(rt, "metaKey")?,
        "left" | "middle" | "right" => {
            let wanted = match name {
                "left" => 0.0,
                "middle" => 1.0,
                _ => 2.0,
            };
            if !rt.in_operator(&js_str("button"), event)? {
                false
            } else {
                let button = rt.get_str(event, "button")?;
                !vn_value::strict_eq(&button, &Value::Number(wanted))
            }
        }
        "exact" => {
            let mut dropped = false;
            for m in SYSTEM_MODIFIERS {
                if flag(rt, &format!("{m}Key"))? && !names.iter().any(|n| n == m) {
                    dropped = true;
                    break;
                }
            }
            dropped
        }
        _ => false,
    })
}

fn with_modifiers_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let f = c.arg(0);
    let mods = c.arg(1);
    if !f.to_boolean() {
        return Ok(f);
    }
    let names = modifier_names(rt, &mods)?;
    let key = names.join(".");
    let cache = cache_on(rt, &f, "_withMods")?;
    if let Some(cache) = &cache {
        let hit = rt.get_str(cache, &key)?;
        if hit.to_boolean() {
            return Ok(hit);
        }
    }
    let wrapper = Value::Object(rt.new_native_function(with_modifiers_wrapper_code, "", 1, vec![f.clone(), mods.clone()]));
    if let Some(cache) = &cache {
        rt.set_str(cache, &key, wrapper.clone())?;
    }
    Ok(wrapper)
}

fn with_modifiers_wrapper_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let (f, mods) = (data[0].clone(), data[1].clone());
    let event = c.arg(0);
    let names = modifier_names(rt, &mods)?;
    for name in &names {
        if modifier_guard(rt, name, &event, &names)? {
            return Ok(Value::Undefined);
        }
    }
    rt.call(&f, &Value::Undefined, c.args)
}

fn key_name(modifier: &str) -> Option<&'static str> {
    Some(match modifier {
        "esc" => "escape",
        "space" => " ",
        "up" => "arrow-up",
        "left" => "arrow-left",
        "right" => "arrow-right",
        "down" => "arrow-down",
        "delete" => "backspace",
        _ => return None,
    })
}

fn with_keys_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let f = c.arg(0);
    let mods = c.arg(1);
    if f.is_nullish() {
        return Err(rt.throw_type_error("Cannot read properties of undefined (reading '_withKeys')"));
    }
    let names = modifier_names(rt, &mods)?;
    let key = names.join(".");
    let cache = cache_on(rt, &f, "_withKeys")?;
    if let Some(cache) = &cache {
        let hit = rt.get_str(cache, &key)?;
        if hit.to_boolean() {
            return Ok(hit);
        }
    }
    let wrapper = Value::Object(rt.new_native_function(with_keys_wrapper_code, "", 1, vec![f.clone(), mods.clone()]));
    if let Some(cache) = &cache {
        rt.set_str(cache, &key, wrapper.clone())?;
    }
    Ok(wrapper)
}

fn with_keys_wrapper_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let (f, mods) = (data[0].clone(), data[1].clone());
    let event = c.arg(0);
    if !rt.in_operator(&js_str("key"), &event)? {
        return Ok(Value::Undefined);
    }
    let key = rt.get_str(&event, "key")?;
    let key = match key {
        Value::String(s) => s.to_rust_string_lossy(),
        _ => return Err(rt.throw_type_error("Cannot read properties of undefined (reading 'replace')")),
    };
    let event_key = hyphenate(&key);
    let names = modifier_names(rt, &mods)?;
    let matched = names.iter().any(|k| *k == event_key || key_name(k) == Some(event_key.as_str()));
    if matched {
        return rt.call(&f, &Value::Undefined, std::slice::from_ref(&event));
    }
    Ok(Value::Undefined)
}

/// The exports of this module: `withModifiers` and `withKeys` (the runtime's `vue` module entries).
pub fn exports(rt: &Rt) -> Vec<(&'static str, Value)> {
    let wm = rt.new_native_function(with_modifiers_code, "withModifiers", 2, Vec::new());
    let wk = rt.new_native_function(with_keys_code, "withKeys", 2, Vec::new());
    vec![("withModifiers", Value::Object(wm)), ("withKeys", Value::Object(wk))]
}

/// The host node of an element handle (`e.target`, `this` of a directive hook), if the value is one.
pub fn node_of_handle(rt: &Rt, v: &Value) -> Option<HostNode> {
    let o = v.as_object()?;
    let ext = rt.ext(o)?;
    payload_ref::<HandleData>(&*ext).map(|h| h.node)
}

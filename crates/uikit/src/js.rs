//! The JavaScript face of the UI testHost: the testHost functions, the NodeHandle and Mount objects, and the TestHost root
//! component that every mount renders (page-host.mjs `mount`). A function finds its state in its own data, which the GC
//! traces; the work is in ops.rs and snapshot.rs, and errors become throws here.

use std::cell::Cell;
use std::rc::Rc;

use vn_runtime::dom_events::DomCtx;
use vn_runtime::host::{HostNode, HostValue};
use vn_runtime::host_doc::DocHost;
use vn_value::{payload_ref, Call, CodeFn, ErrorKind, Flags, Intrinsic, JsStr, NativeObject, ObjRef, PropKey, Res, Rt, Throw, Tracer, Value};

use crate::ops::{self, EventSpec, SelectValue, UiError, UiResult};
use crate::selector::{self, Selector};
use crate::snapshot;

/// The init members dispatch accepts (ops::INIT_KEYS, page-host.mjs).
const INIT_KEYS: [&str; 12] = ops::INIT_KEYS;

/// Shared state of the realm's testHost: the document, its context, the mount counter and the prototypes and exports the
/// functions use. Held in the data of every testHost function, so the GC traces it.
pub(crate) struct UiState {
    host: DocHost,
    ctx: DomCtx,
    app_count: Cell<u32>,
    node_proto: ObjRef,
    mount_proto: ObjRef,
    h: Value,
    shallow_ref: Value,
    create_app: Value,
}

impl NativeObject for UiState {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.node_proto);
        t.mark(self.mount_proto);
        t.mark_value(&self.h);
        t.mark_value(&self.shallow_ref);
        t.mark_value(&self.create_app);
    }
}

/// A NodeHandle: one element of the document.
struct NodeRef {
    host: DocHost,
    node: HostNode,
}

impl NativeObject for NodeRef {
    fn trace(&self, _t: &mut Tracer) {}
}

/// A Mount: its container, its app, and the holder of its props (the TestHost root renders from it).
struct MountRef {
    host: DocHost,
    container: HostNode,
    app: Value,
    holder: Value,
}

impl NativeObject for MountRef {
    fn trace(&self, t: &mut Tracer) {
        t.mark_value(&self.app);
        t.mark_value(&self.holder);
    }
}

/// Installs the testHost functions and the NodeHandle and Mount prototypes on the realm, over `host`. The global
/// `testHost` (installed by vn-testkit) gets `mount`, `find`, `findAll`, and the event functions.
pub fn install(rt: &Rt, host: DocHost) {
    let ctx = DomCtx::new(rt, &host);
    vn_runtime::renderer::install(&ctx);
    let global = Value::Object(rt.intrinsic(Intrinsic::Global));
    let test_host = rt.get_str(&global, "testHost").expect("UI clock installed");
    let timers = rt.get_str(&test_host, "timers").expect("UI clock timers");
    for name in ["setTimeout", "clearTimeout"] {
        rt.install_global(name, rt.get_str(&timers, name).expect("UI clock method"));
    }
    rt.install_global("requestAnimationFrame", Value::Object(rt.new_native_function(animation_frame, "requestAnimationFrame", 1, vec![timers])));
    let exports = vn_runtime::exports::all(rt);
    let export = |name: &str| -> Value {
        exports
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("the vue module has no export {name} (framework bug)"))
    };
    let object_proto = rt.intrinsic(Intrinsic::ObjectPrototype);
    let node_proto = rt.new_object_with_proto(Some(object_proto));
    let node_methods: [(&str, CodeFn, u32); 6] = [
        ("text", node_text, 0),
        ("html", node_html, 0),
        ("attr", node_attr, 1),
        ("prop", node_prop, 1),
        ("exists", node_exists, 0),
        ("snapshot", node_snapshot, 0),
    ];
    for (name, code, len) in node_methods {
        rt.define_native_method(node_proto, name, code, len);
    }
    let mount_proto = rt.new_object_with_proto(Some(object_proto));
    let mount_methods: [(&str, CodeFn, u32); 4] = [
        ("snapshot", mount_snapshot, 0),
        ("html", mount_html, 0),
        ("unmount", mount_unmount, 0),
        ("setProps", mount_set_props, 1),
    ];
    for (name, code, len) in mount_methods {
        rt.define_native_method(mount_proto, name, code, len);
    }
    let state = UiState {
        host,
        ctx,
        app_count: Cell::new(0),
        node_proto,
        mount_proto,
        h: export("h"),
        shallow_ref: export("shallowRef"),
        create_app: export("createApp"),
    };
    let state_obj = rt.new_native_object(None, Rc::new(state));
    rt.register_builtin("vn-uikit.state", state_obj);

    let global = rt.intrinsic(Intrinsic::Global);
    let test_host = match rt.get_str(&Value::Object(global), "testHost") {
        Ok(Value::Object(o)) => o,
        _ => {
            let o = rt.new_object();
            rt.install_global("testHost", Value::Object(o));
            o
        }
    };
    let functions: [(&str, CodeFn, u32); 12] = [
        ("mount", mount_fn, 3),
        ("find", find_fn, 2),
        ("findAll", find_all_fn, 2),
        ("click", click_fn, 1),
        ("input", input_fn, 2),
        ("change", change_fn, 2),
        ("check", check_fn, 2),
        ("select", select_fn, 2),
        ("keydown", keydown_fn, 2),
        ("focus", focus_fn, 1),
        ("dispatch", dispatch_fn, 2),
        ("container", container_fn, 1),
    ];
    for (name, code, len) in functions {
        let f = rt.new_native_function(code, name, len, vec![Value::Object(state_obj)]);
        let _ = rt.define_data(test_host, PropKey::from_str(name), Value::Object(f), Flags::ALL);
    }
}

// ----- plumbing

fn throw(rt: &Rt, e: UiError) -> Throw {
    match e {
        UiError::Type(m) => rt.throw_type_error(&m),
        UiError::Error(m) => rt.throw_error(ErrorKind::Error, &m),
    }
}

fn ok<T>(rt: &Rt, r: UiResult<T>) -> Res<T> {
    r.map_err(|e| throw(rt, e))
}

/// The state of the testHost function being called.
fn state(rt: &Rt, c: &Call<'_>) -> Res<Rc<dyn NativeObject>> {
    match rt.fn_data(c.callee).first() {
        Some(Value::Object(o)) => rt.native_payload(*o).ok_or_else(|| rt.throw_type_error("testHost state is missing (framework bug)")),
        _ => Err(rt.throw_type_error("testHost function without its state (framework bug)")),
    }
}

fn ui(p: &Rc<dyn NativeObject>) -> &UiState {
    payload_ref::<UiState>(&**p).expect("testHost state has the wrong type (framework bug)")
}

fn node_value(rt: &Rt, st: &UiState, node: HostNode) -> Value {
    Value::Object(rt.new_native_object(Some(st.node_proto), Rc::new(NodeRef { host: st.host.clone(), node })))
}

/// The element of a NodeHandle argument.
fn node_of(rt: &Rt, v: &Value) -> Res<(DocHost, HostNode)> {
    if let Value::Object(o) = v {
        if let Some(p) = rt.native_payload(*o) {
            if let Some(n) = payload_ref::<NodeRef>(&*p) {
                return Ok((n.host.clone(), n.node));
            }
        }
    }
    Err(rt.throw_type_error("expected a NodeHandle from testHost"))
}

fn mount_of(rt: &Rt, v: &Value) -> Res<(DocHost, HostNode, Value, Value)> {
    if let Value::Object(o) = v {
        if let Some(p) = rt.native_payload(*o) {
            if let Some(m) = payload_ref::<MountRef>(&*p) {
                return Ok((m.host.clone(), m.container, m.app.clone(), m.holder.clone()));
            }
        }
    }
    Err(rt.throw_type_error("expected a Mount from testHost.mount"))
}

/// The element a find scope names: a Mount's container, or a NodeHandle's element.
fn scope_of(rt: &Rt, v: &Value) -> Res<(DocHost, HostNode)> {
    if let Value::Object(o) = v {
        if let Some(p) = rt.native_payload(*o) {
            if let Some(m) = payload_ref::<MountRef>(&*p) {
                return Ok((m.host.clone(), m.container));
            }
            if let Some(n) = payload_ref::<NodeRef>(&*p) {
                return Ok((n.host.clone(), n.node));
            }
        }
    }
    Err(rt.throw_type_error("expected a NodeHandle from testHost"))
}

/// requireString(value, what): the string of a string argument.
fn require_string(rt: &Rt, v: &Value, what: &str) -> Res<String> {
    match v {
        Value::String(s) => Ok(s.to_rust_string_lossy()),
        _ => Err(rt.throw_type_error(&format!("{what} must be a string"))),
    }
}

fn selector_of(rt: &Rt, v: &Value) -> Res<Selector> {
    let text = match v {
        Value::String(s) => s.to_rust_string_lossy(),
        _ => String::new(),
    };
    selector::parse(&text).map_err(|m| rt.throw_type_error(&m))
}

fn str_value(s: &str) -> Value {
    Value::String(JsStr::from_str(s))
}

fn host_value(v: HostValue) -> Value {
    match v {
        HostValue::Undefined => Value::Undefined,
        HostValue::Null => Value::Null,
        HostValue::Bool(b) => Value::Bool(b),
        HostValue::Number(n) => Value::Number(n),
        HostValue::Str(s) => str_value(&s),
    }
}

/// The enumerable own string keys of an object, in order (Object.keys).
fn own_keys_of(rt: &Rt, o: ObjRef) -> Res<Vec<(PropKey, String)>> {
    let mut out = Vec::new();
    for key in rt.own_keys(o)? {
        let name = match &key {
            PropKey::Index(i) => i.to_string(),
            PropKey::Str(s) => s.to_rust_string_lossy(),
            PropKey::Sym(_) => continue,
        };
        if rt.get_own_property(o, &key)?.is_some_and(|p| p.flags().enumerable) {
            out.push((key, name));
        }
    }
    Ok(out)
}

/// `{...src}`: a shallow copy of the enumerable own properties. A string spreads to its index keys (so a dispatch init
/// given a string is refused, as in the oracle); other non-objects spread to nothing.
fn spread(rt: &Rt, src: &Value) -> Res<ObjRef> {
    let out = rt.new_object();
    match src {
        Value::Object(o) => {
            for (key, _) in own_keys_of(rt, *o)? {
                let v = rt.get_v(src, &key)?;
                rt.define_data(out, key, v, Flags::ALL)?;
            }
        }
        Value::String(s) => {
            for i in 0..s.len() {
                let ch = Value::String(s.substring(i, i + 1));
                rt.define_data(out, PropKey::Index(i as u32), ch, Flags::ALL)?;
            }
        }
        _ => {}
    }
    Ok(out)
}

/// propsCopy(props, what): undefined is an empty object, a non-function object is copied, anything else is a TypeError.
fn props_copy(rt: &Rt, v: &Value, what: &str) -> Res<ObjRef> {
    match v {
        Value::Undefined => Ok(rt.new_object()),
        Value::Object(_) if !rt.is_callable(v) => spread(rt, v),
        _ => Err(rt.throw_type_error(&format!("{what} must be an object"))),
    }
}

/// The fake clock of the scenario (testHost.timers.now), which event time stamps follow.
fn fake_now(rt: &Rt) -> f64 {
    let global = Value::Object(rt.intrinsic(Intrinsic::Global));
    let timers = rt
        .get_str(&global, "testHost")
        .and_then(|th| rt.get_str(&th, "timers"))
        .unwrap_or(Value::Undefined);
    let now = rt.get_str(&timers, "now").unwrap_or(Value::Undefined);
    if !rt.is_callable(&now) {
        return 0.0;
    }
    match rt.call(&now, &timers, &[]) {
        Ok(Value::Number(n)) => n,
        _ => 0.0,
    }
}

/// checkInit(init): an object whose members are all dispatch's, read into the event spec.
fn check_init(rt: &Rt, init: &Value) -> Res<EventSpec> {
    let o = match init {
        Value::Object(o) if !rt.is_callable(init) => *o,
        _ => return Err(rt.throw_type_error("dispatch: init must be an object")),
    };
    for (_, name) in own_keys_of(rt, o)? {
        if !INIT_KEYS.contains(&name.as_str()) {
            return Err(rt.throw_type_error(&format!("dispatch: unknown init member \"{name}\"")));
        }
    }
    let get = |name: &str| rt.get_str(init, name);
    let flag = |name: &str| -> Res<bool> { Ok(get(name)?.to_boolean()) };
    let opt_bool = |name: &str| -> Res<Option<bool>> {
        let v = get(name)?;
        Ok(if v.is_nullish() { None } else { Some(v.to_boolean()) })
    };
    let opt_str = |name: &str| -> Res<Option<String>> {
        let v = get(name)?;
        if v.is_nullish() {
            return Ok(None);
        }
        Ok(Some(rt.to_string(&v)?.to_rust_string_lossy()))
    };
    let opt_int = |name: &str| -> Res<Option<i32>> {
        let v = get(name)?;
        if v.is_nullish() {
            return Ok(None);
        }
        let n = rt.to_number(&v)?;
        Ok(Some(if n.is_finite() { n.trunc() as i64 as i32 } else { 0 }))
    };
    Ok(EventSpec {
        bubbles: opt_bool("bubbles")?,
        cancelable: opt_bool("cancelable")?,
        key: opt_str("key")?,
        code: opt_str("code")?,
        ctrl_key: flag("ctrlKey")?,
        shift_key: flag("shiftKey")?,
        alt_key: flag("altKey")?,
        meta_key: flag("metaKey")?,
        button: opt_int("button")?,
        detail: opt_int("detail")?,
        data: opt_str("data")?,
        is_composing: flag("isComposing")?,
    })
}

/// `dispatch(node, type, init)` with the init as a JavaScript value (already merged by the caller).
fn dispatch_value(rt: &Rt, node: &Value, ty: &Value, init: &Value) -> Res<()> {
    let (mut h, el) = node_of(rt, node)?;
    let ty = match ty {
        Value::String(s) if !s.is_empty() => s.to_rust_string_lossy(),
        _ => return Err(rt.throw_type_error("dispatch: the event type must be a non-empty string")),
    };
    let spec = check_init(rt, init)?;
    let now = fake_now(rt);
    ok(rt, ops::dispatch(&mut h, el, &ty, &spec, now))
}

// ----- testHost functions

fn mount_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = state(rt, c)?;
    let st = ui(&p);
    let component = c.arg(0);
    let props = c.arg(1);
    let options = c.arg(2);
    if !matches!(component, Value::Object(_)) {
        return Err(rt.throw_type_error("mount: the component must be a component object"));
    }
    let mut error_handler = None;
    if !options.is_nullish() {
        if !matches!(options, Value::Object(_)) || rt.is_callable(&options) {
            return Err(rt.throw_type_error("mount: options must be an object"));
        }
        if let Value::Object(o) = options {
            for (_, name) in own_keys_of(rt, o)? {
                if name != "errorHandler" {
                    return Err(rt.throw_type_error(&format!("mount: unknown option \"{name}\"")));
                }
            }
        }
        let eh = rt.get_str(&options, "errorHandler")?;
        if !matches!(eh, Value::Undefined) {
            if !rt.is_callable(&eh) {
                return Err(rt.throw_type_error("mount: errorHandler must be a function"));
            }
            error_handler = Some(eh);
        }
    }
    let n = st.app_count.get() + 1;
    st.app_count.set(n);
    let mut h = st.host.clone();
    let container = ok(rt, ops::mount_container(&mut h, n))?;
    let holder_init = props_copy(rt, &props, "mount: props")?;
    let holder = rt.call(&st.shallow_ref, &Value::Undefined, &[Value::Object(holder_init)])?;
    let root = rt.new_object();
    rt.define_data(root, PropKey::from_str("name"), str_value("TestHost"), Flags::ALL)?;
    let setup = rt.new_native_function(root_setup, "setup", 0, vec![holder.clone(), component, st.h.clone()]);
    rt.define_data(root, PropKey::from_str("setup"), Value::Object(setup), Flags::ALL)?;
    let app = rt.call(&st.create_app, &Value::Undefined, &[Value::Object(root), Value::Undefined])?;
    if let Some(eh) = error_handler {
        let config = rt.get_str(&app, "config")?;
        rt.set_str(&config, "errorHandler", eh)?;
    }
    let mount_method = rt.get_str(&app, "mount")?;
    let handle = st.ctx.handle(container);
    rt.call(&mount_method, &app, &[handle])?;
    let m = rt.new_native_object(Some(st.mount_proto), Rc::new(MountRef { host: h, container, app, holder }));
    Ok(Value::Object(m))
}

/// The TestHost root's setup: returns its render function, which renders the component with a copy of the holder's props.
fn root_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    Ok(Value::Object(rt.new_native_function(root_render, "render", 0, data.to_vec())))
}

fn root_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let (holder, component, h) = (&data[0], &data[1], &data[2]);
    let value = rt.get_str(holder, "value")?;
    let props = spread(rt, &value)?;
    rt.call(h, &Value::Undefined, &[component.clone(), Value::Object(props)])
}

fn find_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = state(rt, c)?;
    let st = ui(&p);
    let (host, root) = scope_of(rt, &c.arg(0))?;
    let sel = selector_of(rt, &c.arg(1))?;
    let found = ok(rt, ops::find_all(&host, root, &sel))?;
    Ok(match found.first() {
        Some(n) => node_value(rt, st, *n),
        None => Value::Null,
    })
}

fn find_all_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = state(rt, c)?;
    let st = ui(&p);
    let (host, root) = scope_of(rt, &c.arg(0))?;
    let sel = selector_of(rt, &c.arg(1))?;
    let found = ok(rt, ops::find_all(&host, root, &sel))?;
    let items: Vec<Value> = found.into_iter().map(|n| node_value(rt, st, n)).collect();
    Ok(Value::Object(rt.new_array(items)))
}

fn click_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let init = Value::Object(rt.new_object());
    dispatch_value(rt, &c.arg(0), &str_value("click"), &init)?;
    Ok(Value::Undefined)
}

fn input_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (mut h, el) = node_of(rt, &c.arg(0))?;
    let value = match c.arg(1) {
        Value::String(s) => Some(s.to_rust_string_lossy()),
        _ => None,
    };
    let now = fake_now(rt);
    ok(rt, ops::input(&mut h, el, value.as_deref(), now))?;
    Ok(Value::Undefined)
}

fn change_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (mut h, el) = node_of(rt, &c.arg(0))?;
    let now = fake_now(rt);
    if ops::is_select(&h, el) {
        let value = picked(rt, &c.arg(1))?;
        ok(rt, ops::select(&mut h, el, as_select(&value), now))?;
        return Ok(Value::Undefined);
    }
    let value = match c.arg(1) {
        Value::String(s) => Some(s.to_rust_string_lossy()),
        _ => None,
    };
    ok(rt, ops::change_text(&mut h, el, value.as_deref(), now))?;
    Ok(Value::Undefined)
}

fn select_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (mut h, el) = node_of(rt, &c.arg(0))?;
    let value = picked(rt, &c.arg(1))?;
    let now = fake_now(rt);
    ok(rt, ops::select(&mut h, el, as_select(&value), now))?;
    Ok(Value::Undefined)
}

/// The value argument of select and change, read from the call.
enum Picked {
    One(String),
    Many(Vec<String>),
    Other,
}

fn picked(rt: &Rt, v: &Value) -> Res<Picked> {
    Ok(match v {
        Value::String(s) => Picked::One(s.to_rust_string_lossy()),
        _ if rt.is_array(v) => {
            let len = rt.to_number(&rt.get_str(v, "length")?)? as u32;
            let mut items = Vec::new();
            for i in 0..len {
                if let Value::String(s) = rt.get_v(v, &PropKey::Index(i))? {
                    items.push(s.to_rust_string_lossy());
                }
            }
            Picked::Many(items)
        }
        _ => Picked::Other,
    })
}

fn as_select(p: &Picked) -> SelectValue<'_> {
    match p {
        Picked::One(s) => SelectValue::One(s),
        Picked::Many(items) => SelectValue::Many(items),
        Picked::Other => SelectValue::Other,
    }
}

fn check_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (mut h, el) = node_of(rt, &c.arg(0))?;
    let checked = match c.arg(1) {
        Value::Bool(b) => Some(b),
        _ => None,
    };
    let now = fake_now(rt);
    ok(rt, ops::check(&mut h, el, checked, now))?;
    Ok(Value::Undefined)
}

fn keydown_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let key = require_string(rt, &c.arg(1), "keydown: key")?;
    let init = spread(rt, &c.arg(2))?;
    rt.set_str(&Value::Object(init), "key", str_value(&key))?;
    dispatch_value(rt, &c.arg(0), &str_value("keydown"), &Value::Object(init))?;
    Ok(Value::Undefined)
}

fn focus_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (mut h, el) = node_of(rt, &c.arg(0))?;
    ok(rt, ops::focus(&mut h, el))?;
    Ok(Value::Undefined)
}

fn dispatch_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let init = match c.arg(2) {
        Value::Undefined => Value::Object(rt.new_object()),
        v => v,
    };
    dispatch_value(rt, &c.arg(0), &c.arg(1), &init)?;
    Ok(Value::Undefined)
}

fn container_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = state(rt, c)?;
    let st = ui(&p);
    let id = match c.arg(0) {
        Value::String(s) => s.to_rust_string_lossy(),
        _ => String::new(),
    };
    if !is_css_identifier(&id) {
        return Err(rt.throw_type_error("container: id must be a CSS identifier"));
    }
    let mut h = st.host.clone();
    let el = ok(rt, ops::container(&mut h, &id))?;
    Ok(node_value(rt, st, el))
}

/// `/^[A-Za-z][\w-]*$/` (the id of container).
fn is_css_identifier(id: &str) -> bool {
    let b = id.as_bytes();
    matches!(b.first(), Some(c) if c.is_ascii_alphabetic())
        && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'-')
}

// ----- NodeHandle and Mount methods

fn node_text(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (mut h, el) = node_of(rt, c.this)?;
    Ok(str_value(&ok(rt, ops::text_content(&mut h, el))?))
}

fn node_html(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (h, el) = node_of(rt, c.this)?;
    Ok(str_value(&ops::inner_html(&h, el)))
}

fn node_attr(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let name = require_string(rt, &c.arg(0), "attr: name")?;
    let (h, el) = node_of(rt, c.this)?;
    Ok(match ops::attr(&h, el, &name) {
        Some(v) => str_value(&v),
        None => Value::Null,
    })
}

fn node_prop(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let name = require_string(rt, &c.arg(0), "prop: name")?;
    let (mut h, el) = node_of(rt, c.this)?;
    Ok(host_value(ok(rt, ops::prop(&mut h, el, &name))?))
}

fn node_exists(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (mut h, el) = node_of(rt, c.this)?;
    Ok(Value::Bool(ok(rt, ops::is_connected(&mut h, el))?))
}

fn node_snapshot(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (h, el) = node_of(rt, c.this)?;
    let tree = ok(rt, ops::snapshot_of(&h, el))?;
    Ok(snapshot::to_js(rt, &tree))
}

fn mount_snapshot(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (h, container, _, _) = mount_of(rt, c.this)?;
    let children = ok(rt, ops::children_snapshot(&h, container))?;
    let items: Vec<Value> = children.iter().map(|k| snapshot::to_js(rt, k)).collect();
    Ok(Value::Object(rt.new_array(items)))
}

fn mount_html(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (h, container, _, _) = mount_of(rt, c.this)?;
    Ok(str_value(&ops::inner_html(&h, container)))
}

fn mount_unmount(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (_, _, app, _) = mount_of(rt, c.this)?;
    let unmount = rt.get_str(&app, "unmount")?;
    rt.call(&unmount, &app, &[])?;
    Ok(Value::Undefined)
}

fn mount_set_props(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (_, _, _, holder) = mount_of(rt, c.this)?;
    let next = props_copy(rt, &c.arg(0), "setProps: props")?;
    rt.set_str(&holder, "value", Value::Object(next))?;
    Ok(Value::Undefined)
}

// The UI oracle also maps requestAnimationFrame to its deterministic 16 ms clock.
fn animation_frame(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let timers = rt.fn_data(c.callee)[0].clone();
    rt.call(&rt.get_str(&timers, "setTimeout")?, &timers, &[c.arg(0), Value::Number(16.0)])
}

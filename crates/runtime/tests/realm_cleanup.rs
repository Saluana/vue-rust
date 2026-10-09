use std::cell::Cell;
use std::rc::Rc;
use vn_runtime::{dom_events::DomCtx, host::Host, host_doc::DocHost, renderer};
use vn_value::{Call, Flags, NativeObject, PropKey, Res, Rt, Tracer, Value};

struct Sentinel(Rc<Cell<u32>>);
impl NativeObject for Sentinel { fn trace(&self, _: &mut Tracer) {} }
impl Drop for Sentinel { fn drop(&mut self) { self.0.set(self.0.get() + 1); } }
fn noop(_: &Rt, _: &Call<'_>) -> Res<Value> { Ok(Value::Undefined) }

#[test]
fn dom_realm_drops_with_its_owners() {
    let drops = Rc::new(Cell::new(0));
    {
        let rt = Rt::new();
        let o = rt.new_native_object(None, Rc::new(Sentinel(drops.clone())));
        rt.register_builtin("sentinel", o);
        let host = DocHost::new();
        let ctx = DomCtx::new(&rt, &host);
        renderer::install(&ctx);
    }
    assert_eq!(drops.get(), 1);
}

#[test]
fn unmount_releases_listener_resources() {
    let rt = Rt::new();
    let mut host = DocHost::new();
    let ctx = DomCtx::new(&rt, &host);
    renderer::install(&ctx);
    let body = host.install_body().unwrap();
    let create = Value::Object(rt.builtin("vn-runtime.createVNode").unwrap());
    let drops = Rc::new(Cell::new(0));
    for round in 1..=3 {
        let captured = rt.new_native_object(None, Rc::new(Sentinel(drops.clone())));
        let cycle = rt.new_cell(Value::Undefined);
        let listener = rt.new_native_function(noop, "click", 0, vec![Value::Object(captured), Value::Object(cycle)]);
        let props = rt.new_object();
        rt.define_data(props, PropKey::from_str("onClick"), Value::Object(listener), Flags::ALL).unwrap();
        let vnode = rt.call(&create, &Value::Undefined, &[Value::str("button"), Value::Object(props), Value::Null]).unwrap().as_object().unwrap();
        renderer::render_into(&rt, Some(vnode), body).unwrap();
        assert_eq!(ctx.listener_count(), 1);
        let node = host.query_selector("button").unwrap().unwrap();
        rt.cell_init(cycle, ctx.handle(node));
        host.click(node).unwrap();
        renderer::render_into(&rt, None, body).unwrap();
        rt.collect();
        assert_eq!(ctx.listener_count(), 0);
        assert_eq!(ctx.side_table_entries(), (0, 0));
        assert_eq!(host.inner_html(body).unwrap(), "");
        assert_eq!(drops.get(), round);
    }
}

fn inject_key(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let key = rt.fn_data(c.callee)[0].clone();
    let inject = vn_runtime::exports::all(rt).into_iter().find(|(n, _)| *n == "inject").unwrap().1;
    rt.call(&inject, &Value::Undefined, &[key])
}

#[test]
fn app_provides_symbol_keys_without_string_conversion() {
    let rt = Rt::new();
    let create = vn_runtime::exports::all(&rt).into_iter().find(|(n, _)| *n == "createApp").unwrap().1;
    let app = rt.call(&create, &Value::Undefined, &[Value::Object(rt.new_object())]).unwrap();
    let key = Value::Symbol(rt.new_symbol(None));
    let provide = rt.get_str(&app, "provide").unwrap();
    rt.call(&provide, &app, &[key.clone(), Value::Number(42.0)]).unwrap();
    let callback = Value::Object(rt.new_native_function(inject_key, "read", 0, vec![key]));
    let run = rt.get_str(&app, "runWithContext").unwrap();
    assert_eq!(rt.call(&run, &app, &[callback]).unwrap(), Value::Number(42.0));
}

#[test]
fn detached_ref_remains_usable_until_its_owner_drops_it() {
    let rt = Rt::new();
    let mut host = DocHost::new();
    let ctx = DomCtx::new(&rt, &host);
    renderer::install(&ctx);
    let body = host.install_body().unwrap();
    let create = Value::Object(rt.builtin("vn-runtime.createVNode").unwrap());
    let props = rt.new_object();
    rt.define_data(props, PropKey::from_str("id"), Value::str("saved"), Flags::ALL).unwrap();
    let vnode = rt.call(&create, &Value::Undefined, &[Value::str("button"), Value::Object(props), Value::Null]).unwrap().as_object().unwrap();
    renderer::render_into(&rt, Some(vnode), body).unwrap();
    let node = host.query_selector("button").unwrap().unwrap();
    host.set_attribute(node, "data-saved", "saved").unwrap();
    let saved = ctx.handle(node);
    let root = rt.root(&saved);
    renderer::render_into(&rt, None, body).unwrap();
    rt.collect();
    let get = rt.get_str(&saved, "getAttribute").unwrap();
    assert_eq!(rt.call(&get, &saved, &[Value::str("data-saved")]).unwrap(), Value::str("saved"));
    assert_eq!(host.parent_node(node), None);
    rt.unroot(root);
    rt.collect();
    assert_eq!(ctx.side_table_entries(), (0, 0));
    assert_eq!(host.outer_html(node), None);
}

#[test]
fn replacing_array_children_with_text_releases_their_resources() {
    let rt = Rt::new();
    let mut host = DocHost::new();
    let ctx = DomCtx::new(&rt, &host);
    renderer::install(&ctx);
    let body = host.install_body().unwrap();
    let create = Value::Object(rt.builtin("vn-runtime.createVNode").unwrap());
    let drops = Rc::new(Cell::new(0));
    let captured = rt.new_native_object(None, Rc::new(Sentinel(drops.clone())));
    let listener = rt.new_native_function(noop, "click", 0, vec![Value::Object(captured)]);
    let props = rt.new_object();
    rt.define_data(props, PropKey::from_str("onClick"), Value::Object(listener), Flags::ALL).unwrap();
    let child = rt.call(&create, &Value::Undefined, &[Value::str("button"), Value::Object(props), Value::Null]).unwrap();
    let children = Value::Object(rt.new_array(vec![child]));
    let first = rt.call(&create, &Value::Undefined, &[Value::str("div"), Value::Null, children]).unwrap().as_object().unwrap();
    renderer::render_into(&rt, Some(first), body).unwrap();
    let next = rt.call(&create, &Value::Undefined, &[Value::str("div"), Value::Null, Value::str("done")]).unwrap().as_object().unwrap();
    renderer::render_into(&rt, Some(next), body).unwrap();
    rt.collect();
    assert_eq!(host.inner_html(body).unwrap(), "<div>done</div>");
    assert_eq!(ctx.listener_count(), 0);
    assert_eq!(drops.get(), 1);
}

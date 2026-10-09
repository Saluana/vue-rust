//! KeepAlive component cache, LRU eviction and activation/deactivation.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use vn_value::{payload_ref, same_value_zero, Call, JsStr, NativeObject, ObjRef, PropKey, Res, Rt, Tracer, Value};
use crate::component::invoke_array_fns;
use crate::host::{HostNode, HostNs};
use crate::instance::{current_instance, instance_of, LifeHook};
use crate::lifecycle::inject_hook;
use crate::props::put;
use crate::renderer::{array_len, component_of, el_of, key_of, renderer, set_el, shape_of, ty_of, vnode_at, Renderer};
use crate::scheduler::queue_post_render_effect;
use crate::shapes::ShapeFlags;
use crate::vnode::fields;

struct Cache {
    instance: ObjRef,
    storage: HostNode,
    storage_handle: Value,
    entries: RefCell<Vec<(Value, ObjRef)>>,
    current: Cell<Option<ObjRef>>,
}

impl NativeObject for Cache {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.instance);
        t.mark_value(&self.storage_handle);
        for (key, vnode) in self.entries.borrow().iter() {
            t.mark_value(key);
            t.mark(*vnode);
        }
        if let Some(vnode) = self.current.get() {
            t.mark(vnode);
        }
    }
}

fn cache(rt: &Rt, o: ObjRef) -> Rc<dyn NativeObject> { rt.native_payload(o).expect("KeepAlive cache") }

fn reset(rt: &Rt, v: ObjRef) {
    fields(rt, v, |f| {
        let Value::Number(n) = f.shape else { return; };
        let flags = ShapeFlags::COMPONENT_KEPT_ALIVE | ShapeFlags::COMPONENT_SHOULD_KEEP_ALIVE;
        f.shape = Value::Number(((n as i32) & !flags) as f64);
    });
}

fn matches(rt: &Rt, pattern: &Value, name: &str) -> Res<bool> {
    match pattern {
        Value::String(s) => Ok(s.to_rust_string_lossy().split(',').any(|n| n == name)),
        _ if rt.is_array(pattern) => {
            for i in 0..array_len(rt, pattern)? {
                if matches(rt, &rt.get_v(pattern, &PropKey::Index(i as u32))?, name)? { return Ok(true); }
            }
            Ok(false)
        }
        _ => {
            let test = rt.get_str(pattern, "test")?;
            if !rt.is_callable(&test) { return Err(rt.throw_type_error("KeepAlive include/exclude must be a string, array or RegExp")); }
            put(rt, pattern, "lastIndex", Value::Number(0.0))?;
            Ok(rt.call(&test, pattern, &[Value::String(JsStr::from_str(name))])?.to_boolean())
        }
    }
}

fn component_name(rt: &Rt, v: ObjRef) -> Res<String> {
    let mut ty = ty_of(rt, v);
    let resolved = rt.get_str(&ty, "__asyncResolved")?;
    if resolved.to_boolean() { ty = resolved; }
    let name = rt.get_str(&ty, "name")?;
    let name = if name.to_boolean() { name } else { rt.get_str(&ty, "__name")? };
    Ok(if name.is_nullish() { String::new() } else { rt.to_string(&name)?.to_rust_string_lossy() })
}

fn allowed(rt: &Rt, props: &Value, v: ObjRef) -> Res<bool> {
    let name = component_name(rt, v)?;
    let include = rt.get_str(props, "include")?;
    let exclude = rt.get_str(props, "exclude")?;
    Ok((!include.to_boolean() || (!name.is_empty() && matches(rt, &include, &name)?)) &&
        (!exclude.to_boolean() || name.is_empty() || !matches(rt, &exclude, &name)?))
}

fn prune(rt: &Rt, p: &Cache, key: &Value) -> Res<()> {
    let entry = {
        let mut entries = p.entries.borrow_mut();
        entries.iter().position(|(k, _)| same_value_zero(k, key)).map(|i| entries.remove(i).1)
    };
    if let Some(v) = entry {
        let active = p.current.get().filter(|c| crate::vnode::is_same_vnode_type(rt, *c, v));
        if let Some(c) = active { reset(rt, c); }
        else { reset(rt, v); renderer(rt)?.unmount(v, Some(p.instance), true, false)?; }
    }
    Ok(())
}

fn setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let inst = current_instance(rt).expect("KeepAlive setup instance");
    let r = renderer(rt)?;
    let storage = r.hres(r.host().create_element(HostNs::Html, "div", None))?;
    let o = rt.new_native_object(None, Rc::new(Cache {
        instance: inst,
        storage,
        storage_handle: r.ctx.handle(storage),
        entries: RefCell::new(Vec::new()),
        current: Cell::new(None),
    }));
    let ctx = instance_of(rt, inst).ctx.borrow().clone();
    put(rt, &ctx, "__keepAliveCache", Value::Object(o))?;
    let cleanup = Value::Object(rt.new_native_function(before_unmount, "pruneKeepAlive", 0, vec![Value::Object(o)]));
    inject_hook(rt, LifeHook::Bum, &cleanup, Some(inst), false)?;
    let release = Value::Object(rt.new_native_function(release_storage, "releaseKeepAlive", 0, vec![Value::Object(o)]));
    inject_hook(rt, LifeHook::Um, &release, Some(inst), false)?;
    Ok(Value::Object(rt.new_native_function(render, "renderKeepAlive", 0, vec![Value::Object(o), c.arg(0)])))
}

fn render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let [cache_state, props] = &*data else { unreachable!("KeepAlive render captures") };
    let payload = cache(rt, cache_state.as_object().expect("KeepAlive cache"));
    let p = payload_ref::<Cache>(&*payload).expect("KeepAlive cache type");
    // Prune changed include/exclude patterns without unmounting the currently rendered branch.
    let stale = p.entries.borrow().clone();
    for (key, v) in stale { if !allowed(rt, props, v)? { prune(rt, p, &key)?; } }
    let slots = instance_of(rt, p.instance).slots.borrow().clone();
    let slot = rt.get_str(&slots, "default")?;
    if !rt.is_callable(&slot) { p.current.set(None); return Ok(Value::Undefined); }
    let children = rt.call(&slot, &Value::Undefined, &[])?;
    if array_len(rt, &children)? != 1 { p.current.set(None); return Ok(children); }
    let v = vnode_at(rt, &children, 0)?;
    if shape_of(rt, v) & ShapeFlags::STATEFUL_COMPONENT == 0 { p.current.set(None); return Ok(Value::Object(v)); }
    if !allowed(rt, props, v)? { reset(rt, v); p.current.set(Some(v)); return Ok(Value::Object(v)); }
    let raw_key = key_of(rt, v);
    let key = if raw_key.is_nullish() { ty_of(rt, v) } else { raw_key };
    let cached = {
        let mut entries = p.entries.borrow_mut();
        entries.iter().position(|(k, _)| same_value_zero(k, &key)).map(|i| entries.remove(i).1)
    };
    if let Some(old) = cached {
        set_el(rt, v, el_of(rt, old));
        let component = component_of(rt, old);
        fields(rt, v, |f| { f.component = component; f.shape = Value::Number((shape_value(&f.shape) | ShapeFlags::COMPONENT_KEPT_ALIVE) as f64); });
    }
    p.entries.borrow_mut().push((key, v));
    let max = rt.get_str(props, "max")?;
    if max.to_boolean() {
        let limit = crate::dom_props::parse_float(&rt.to_string(&max)?.to_rust_string_lossy());
        if (p.entries.borrow().len() as f64) > limit {
            let key = p.entries.borrow()[0].0.clone();
            prune(rt, p, &key)?;
        }
    }
    fields(rt, v, |f| f.shape = Value::Number((shape_value(&f.shape) | ShapeFlags::COMPONENT_SHOULD_KEEP_ALIVE) as f64));
    p.current.set(Some(v));
    Ok(Value::Object(v))
}

fn shape_value(v: &Value) -> i32 { match v { Value::Number(n) => *n as i32, _ => 0 } }

fn before_unmount(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let payload = cache(rt, rt.fn_data(c.callee)[0].as_object().expect("KeepAlive cache"));
    let p = payload_ref::<Cache>(&*payload).expect("KeepAlive cache type");
    let current = instance_of(rt, p.instance).sub_tree.borrow().as_object();
    let entries = p.entries.borrow().clone();
    for (_, v) in entries {
        reset(rt, v);
        if current.is_some_and(|cur| crate::vnode::is_same_vnode_type(rt, cur, v)) {
            let cur = current.expect("active vnode");
            reset(rt, cur);
            if let Some(inst) = component_of(rt, cur).as_object() { queue_lifecycle(rt, inst, false)?; }
        } else { renderer(rt)?.unmount(v, Some(p.instance), true, false)?; }
    }
    p.entries.borrow_mut().clear();
    p.current.set(None);
    Ok(Value::Undefined)
}

fn release_storage(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let payload = cache(rt, rt.fn_data(c.callee)[0].as_object().expect("KeepAlive cache"));
    let p = payload_ref::<Cache>(&*payload).expect("KeepAlive cache type");
    renderer(rt)?.rm(p.storage)?;
    Ok(Value::Undefined)
}

fn lifecycle(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let [instance, active] = &*data else { unreachable!("KeepAlive lifecycle captures") };
    let active = active.to_boolean();
    let rec = instance_of(rt, instance.as_object().expect("cached instance"));
    if active {
        rec.is_deactivated.set(false);
    }
    let hook = if active { LifeHook::A } else { LifeHook::Da };
    let hooks = rec.hooks[hook as usize].borrow().clone();
    invoke_array_fns(rt, &hooks)?;
    if !active {
        rec.is_deactivated.set(true);
    }
    Ok(Value::Undefined)
}

fn queue_lifecycle(rt: &Rt, inst: ObjRef, active: bool) -> Res<()> {
    let f = Value::Object(rt.new_native_function(lifecycle, "activateCachedComponent", 0, vec![Value::Object(inst), Value::Bool(active)]));
    queue_post_render_effect(rt, &f, &Value::Null)
}

impl Renderer {
    pub(crate) fn deactivate_cached(&self, v: ObjRef, parent: ObjRef) -> Res<()> {
        let rt = self.rt();
        let o = rt.get_str(&instance_of(rt, parent).ctx.borrow().clone(), "__keepAliveCache")?.as_object()
            .ok_or_else(|| rt.throw_type_error("KeepAlive cache missing"))?;
        let payload = cache(rt, o);
        let p = payload_ref::<Cache>(&*payload).expect("KeepAlive cache type");
        self.move_vnode(v, p.storage, None, 1)?;
        if let Some(inst) = component_of(rt, v).as_object() { queue_lifecycle(rt, inst, false)?; }
        Ok(())
    }
    pub(crate) fn activate_cached(&self, v: ObjRef, container: HostNode, anchor: Option<HostNode>, ns: HostNs, optimized: bool) -> Res<()> {
        let rt = self.rt();
        let inst = component_of(rt, v).as_object().ok_or_else(|| rt.throw_type_error("KeepAlive cached instance missing"))?;
        let old = instance_of(rt, inst).vnode.borrow().as_object().expect("cached vnode");
        self.move_vnode(v, container, anchor, 0)?;
        self.patch(Some(old), v, container, anchor, Some(inst), ns, &Value::Null, optimized)?;
        queue_lifecycle(rt, inst, true)
    }
}

pub(crate) fn exports(rt: &Rt) -> Vec<(&'static str, Value)> {
    let comp = Value::Object(rt.new_object());
    put(rt, &comp, "name", Value::String(JsStr::from_str("KeepAlive"))).expect("KeepAlive definition");
    put(rt, &comp, "__isKeepAlive", Value::Bool(true)).expect("KeepAlive definition");
    put(rt, &comp, "inheritAttrs", Value::Bool(false)).expect("KeepAlive definition");
    let props = rt.new_array(["include", "exclude", "max"].iter().map(|s| Value::String(JsStr::from_str(s))).collect());
    put(rt, &comp, "props", Value::Object(props)).expect("KeepAlive definition");
    put(rt, &comp, "setup", Value::Object(rt.new_native_function(setup, "setup", 2, vec![]))).expect("KeepAlive definition");
    vec![("KeepAlive", comp)]
}

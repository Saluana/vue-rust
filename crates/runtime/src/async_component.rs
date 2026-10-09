//! Lazy component loading with shared requests, delayed placeholders, errors and timeout cleanup.
use std::cell::RefCell;
use std::rc::Rc;
use vn_value::{payload_ref, Call, JsStr, NativeObject, ObjRef, Res, Rt, Tracer, Value};
use crate::instance::{current_instance, instance_of, LifeHook};
use crate::props::{put, reactivity_fn};
use crate::renderer::{builtin_fn, children_of, props_of};
use crate::lifecycle::inject_hook;

struct Loader {
    options: Value,
    pending: RefCell<Value>,
    resolved: RefCell<Value>,
    wrapper: RefCell<Value>,
}

impl NativeObject for Loader {
    fn trace(&self, t: &mut Tracer) {
        for value in [&self.options, &*self.pending.borrow(), &*self.resolved.borrow(), &*self.wrapper.borrow()] {
            t.mark_value(value);
        }
    }
}

fn payload(rt: &Rt, o: ObjRef) -> Rc<dyn NativeObject> { rt.native_payload(o).expect("async loader state") }

fn define(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let source = c.arg(0);
    let opts = if rt.is_callable(&source) {
        let opts = Value::Object(rt.new_object());
        put(rt, &opts, "loader", source)?;
        opts
    } else { source };
    if !rt.is_callable(&rt.get_str(&opts, "loader")?) { return Err(rt.throw_type_error("defineAsyncComponent requires a loader")); }
    if rt.get_str(&opts, "hydrate")?.to_boolean() || rt.get_str(&opts, "onError")?.to_boolean() {
        return Err(rt.throw_type_error("unsupported: async hydration and loader retry hooks"));
    }
    let o = rt.new_native_object(None, Rc::new(Loader {
        options: opts,
        pending: RefCell::new(Value::Null),
        resolved: RefCell::new(Value::Null),
        wrapper: RefCell::new(Value::Null),
    }));
    let comp = Value::Object(rt.new_object());
    put(rt, &comp, "name", Value::String(JsStr::from_str("AsyncComponentWrapper")))?;
    put(rt, &comp, "inheritAttrs", Value::Bool(false))?;
    put(rt, &comp, "setup", Value::Object(rt.new_native_function(setup, "setup", 0, vec![Value::Object(o)])))?;
    put(rt, &comp, "__asyncLoader", Value::Object(rt.new_native_function(load_code, "load", 0, vec![Value::Object(o)])))?;
    *payload_ref::<Loader>(&*payload(rt, o)).expect("loader type").wrapper.borrow_mut() = comp.clone();
    Ok(comp)
}

fn then(rt: &Rt, promise: &Value, yes: Value, no: Value) -> Res<Value> {
    rt.call(&rt.get_str(promise, "then")?, promise, &[yes, no])
}

fn load_code(rt: &Rt, c: &Call<'_>) -> Res<Value> { load(rt, rt.fn_data(c.callee)[0].as_object().expect("loader state")) }

fn load(rt: &Rt, o: ObjRef) -> Res<Value> {
    let payload = payload(rt, o);
    let p = payload_ref::<Loader>(&*payload).expect("loader type");
    if p.pending.borrow().to_boolean() { return Ok(p.pending.borrow().clone()); }
    let loader = rt.get_str(&p.options, "loader")?;
    let result = rt.call(&loader, &Value::Undefined, &[]);
    let ctor = rt.get_str(&Value::Object(rt.intrinsic(vn_value::Intrinsic::Global)), "Promise")?;
    let promise = match result {
        Ok(v) => rt.call(&rt.get_str(&ctor, "resolve")?, &ctor, &[v])?,
        Err(e) => rt.call(&rt.get_str(&ctor, "reject")?, &ctor, &[e.0])?,
    };
    let finish = Value::Object(rt.new_native_function(resolved, "resolveAsyncComponent", 1, vec![Value::Object(o)]));
    let promise = then(rt, &promise, finish, Value::Undefined)?;
    *p.pending.borrow_mut() = promise.clone();
    Ok(promise)
}

fn resolved(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let payload = payload(rt, rt.fn_data(c.callee)[0].as_object().expect("loader state"));
    let p = payload_ref::<Loader>(&*payload).expect("loader type");
    let mut comp = c.arg(0);
    if rt.get_str(&comp, "__esModule")?.to_boolean() || crate::dom_props::has_tag(rt, &comp, "Module")? { comp = rt.get_str(&comp, "default")?; }
    if !matches!(comp, Value::Object(_)) { return Err(rt.throw_type_error("async loader did not resolve a component")); }
    *p.resolved.borrow_mut() = comp.clone();
    put(rt, &p.wrapper.borrow().clone(), "__asyncResolved", comp.clone())?;
    Ok(comp)
}

fn make_ref(rt: &Rt, value: Value) -> Res<Value> { rt.call(&reactivity_fn(rt, "ref"), &Value::Undefined, &[value]) }

fn setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = rt.fn_data(c.callee)[0].as_object().expect("loader state");
    let payload = payload(rt, o);
    let p = payload_ref::<Loader>(&*payload).expect("loader type");
    let inst = current_instance(rt).expect("async wrapper instance");
    let loaded = make_ref(rt, Value::Bool(p.resolved.borrow().to_boolean()))?;
    let error = make_ref(rt, Value::Null)?;
    let delay = rt.get_str(&p.options, "delay")?;
    let delay = if matches!(delay, Value::Undefined) { Value::Number(200.0) } else { delay };
    let delayed = make_ref(rt, Value::Bool(delay.to_boolean()))?;
    let data = vec![Value::Object(o), Value::Object(inst), loaded, error, delayed];
    let timers = Value::Object(rt.new_array(Vec::new()));
    if !p.resolved.borrow().to_boolean() {
        if delay.to_boolean() {
            let callback = Value::Object(rt.new_native_function(delay_elapsed, "asyncDelay", 0, data.clone()));
            let id = crate::timers::timeout(rt, callback, delay)?;
            put(rt, &timers, "0", id)?;
        }
        let timeout = rt.get_str(&p.options, "timeout")?;
        if !timeout.is_nullish() {
            let mut args = data.clone();
            args.push(timeout.clone());
            let callback = Value::Object(rt.new_native_function(timed_out, "asyncTimeout", 0, args));
            put(rt, &timers, "1", crate::timers::timeout(rt, callback, timeout)?)?;
        }
        let yes = Value::Object(rt.new_native_function(loaded_code, "asyncLoaded", 1, data.clone()));
        let no = Value::Object(rt.new_native_function(error_code, "asyncError", 1, data.clone()));
        then(rt, &load(rt, o)?, yes, no)?;
    }
    let cleanup = Value::Object(rt.new_native_function(cleanup, "cancelAsyncTimers", 0, vec![timers]));
    inject_hook(rt, LifeHook::Um, &cleanup, Some(inst), false)?;
    Ok(Value::Object(rt.new_native_function(render, "renderAsyncComponent", 0, data)))
}

fn alive(rt: &Rt, instance: &Value) -> bool {
    !instance_of(rt, instance.as_object().expect("async instance")).is_unmounted.get()
}

fn delay_elapsed(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let [_, instance, _, _, delayed] = &*data else { unreachable!("async setup captures") };
    if alive(rt, instance) {
        put(rt, delayed, "value", Value::Bool(false))?;
    }
    Ok(Value::Undefined)
}

fn loaded_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let [_, instance, loaded, _, _] = &*data else { unreachable!("async setup captures") };
    if alive(rt, instance) {
        put(rt, loaded, "value", Value::Bool(true))?;
    }
    Ok(Value::Undefined)
}

fn error_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let [_, instance, _, _, _] = &*data else { unreachable!("async setup captures") };
    if alive(rt, instance) {
        record_error(rt, &data, c.arg(0))?;
    }
    Ok(Value::Undefined)
}

fn record_error(rt: &Rt, captures: &[Value], error: Value) -> Res<()> {
    let [loader, instance, _, error_ref, _, ..] = captures else { unreachable!("async setup captures") };
    let payload = payload(rt, loader.as_object().expect("loader state"));
    let loader = payload_ref::<Loader>(&*payload).expect("loader type");
    *loader.pending.borrow_mut() = Value::Null;
    crate::errors::handle_error(
        rt, &error, instance.as_object(),
        crate::errors::ErrType::Code(crate::errors::code::ASYNC_COMPONENT_LOADER),
    )?;
    put(rt, error_ref, "value", error)
}

fn timed_out(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let [_, instance, loaded, error, _, timeout] = &*data else { unreachable!("async timeout captures") };
    if alive(rt, instance) && !rt.get_str(loaded, "value")?.to_boolean() && !rt.get_str(error, "value")?.to_boolean() {
        let ms = rt.to_string(timeout)?.to_rust_string_lossy();
        let error = rt.throw_error(vn_value::ErrorKind::Error, &format!("Async component timed out after {ms}ms.")).0;
        record_error(rt, &data, error)?;
    }
    Ok(Value::Undefined)
}

fn cleanup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let timers = rt.fn_data(c.callee)[0].clone();
    for i in 0..2 { crate::timers::cancel(rt, &rt.get_v(&timers, &vn_value::PropKey::Index(i))?)?; }
    Ok(Value::Undefined)
}

fn inner(rt: &Rt, comp: Value, inst: ObjRef) -> Res<Value> {
    let original = instance_of(rt, inst).vnode.borrow().as_object().expect("async vnode");
    let v = rt.call(&builtin_fn(rt, "vn-runtime.createVNode"), &Value::Undefined, &[comp, props_of(rt, original), children_of(rt, original)])?;
    if let Some(v) = v.as_object() {
        let reference = crate::vnode::fields(rt, original, |f| f.reference.clone());
        crate::vnode::fields(rt, v, |f| f.reference = reference);
    }
    Ok(v)
}

fn render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let [loader, instance, loaded, error, delayed] = &*data else { unreachable!("async setup captures") };
    let payload = payload(rt, loader.as_object().expect("loader state"));
    let loader = payload_ref::<Loader>(&*payload).expect("loader type");
    let instance = instance.as_object().expect("async instance");
    if rt.get_str(loaded, "value")?.to_boolean() && loader.resolved.borrow().to_boolean() {
        return inner(rt, loader.resolved.borrow().clone(), instance);
    }
    let error = rt.get_str(error, "value")?;
    let error_comp = rt.get_str(&loader.options, "errorComponent")?;
    if error.to_boolean() && error_comp.to_boolean() {
        let props = Value::Object(rt.new_object());
        put(rt, &props, "error", error)?;
        return rt.call(&builtin_fn(rt, "vn-runtime.createVNode"), &Value::Undefined, &[error_comp, props]);
    }
    let loading = rt.get_str(&loader.options, "loadingComponent")?;
    if loading.to_boolean() && !rt.get_str(delayed, "value")?.to_boolean() {
        return inner(rt, loading, instance);
    }
    Ok(Value::Undefined)
}

pub(crate) fn exports(rt: &Rt) -> Vec<(&'static str, Value)> {
    vec![("defineAsyncComponent", Value::Object(rt.new_native_function(define, "defineAsyncComponent", 1, vec![])))]
}

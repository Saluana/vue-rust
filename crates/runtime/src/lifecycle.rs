//! vn-runtime::lifecycle: `injectHook` and the lifecycle registration functions of runtime-core 3.5.43 (apiLifecycle,
//! production build). Behaviour per @vue/runtime-core 3.5.43.
//!
//! A registered hook is wrapped and the wrapper is cached on the hook as `__weh`, as Vue does. Running the wrapper pauses
//! tracking, makes its instance the current instance, calls the hook through `callWithAsyncErrorHandling`, then restores
//! both. The restore also runs on the error path (a deliberate deviation). The `on*` registrations created by `createHook`
//! wrap their argument in a fresh forwarding function, as Vue does. onRenderTracked and onRenderTriggered register like the
//! others, and nothing runs their lists in production. onServerPrefetch is stored only, since SSR is unsupported.
//! onActivated and onDeactivated also register on the enclosing KeepAlive roots (`registerKeepAliveHook`).
//!
//! Target handling: an omitted or undefined target means the current instance. null (or another falsy value) means no
//! target, and the registration does nothing. A truthy non-instance target throws a TypeError (unsupported).

use vn_value::{strict_eq, Call, JsStr, ObjRef, PropKey, Res, Rt, Value};

use crate::errors::{call_with_async_error_handling, ErrType};
use crate::instance::{as_instance, current_instance, enter_instance, inst_state, instance_of, leave_instance, LifeHook};

/// The target argument of a registration: undefined is the current instance, a falsy value is no target.
pub(crate) fn resolve_target(rt: &Rt, arg: &Value) -> Res<Option<ObjRef>> {
    match arg {
        Value::Undefined => Ok(current_instance(rt)),
        Value::Object(o) => {
            if as_instance(rt, *o).is_some() {
                Ok(Some(*o))
            } else {
                Err(rt.throw_type_error("unsupported: lifecycle target is not a component instance"))
            }
        }
        v if v.to_boolean() => Err(rt.throw_type_error("unsupported: lifecycle target is not a component instance")),
        _ => Ok(None),
    }
}

/// `hook.__weh || (hook.__weh = (...args) => {...})`: the wrapper of `hook` for `target` (cached on the hook).
fn weh_of(rt: &Rt, hook: &Value, target: ObjRef, kind: LifeHook) -> Res<Value> {
    let cached = rt.get_str(hook, "__weh")?;
    if cached.to_boolean() {
        return Ok(cached);
    }
    let w = Value::Object(rt.new_native_function(
        weh_code,
        "",
        0,
        vec![hook.clone(), Value::Object(target), ErrType::Hook(kind).to_value()],
    ));
    rt.set_v_strict(hook, &PropKey::lit("__weh"), w.clone())?;
    Ok(w)
}

/// The wrapper that `injectHook` registers. Data: [hook, target, type].
fn weh_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let hook = d[0].clone();
    let target = d[1].as_object().expect("hook target (framework bug)");
    let ty = ErrType::from_value(&d[2]).expect("hook type (framework bug)");
    let st = inst_state(rt);
    rt.call(&st.pause_tracking, &Value::Undefined, &[])?;
    let res = (|| -> Res<Value> {
        let prev = enter_instance(rt, target)?;
        let r = call_with_async_error_handling(rt, &hook, Some(target), ty, Some(c.args));
        let l = leave_instance(rt, target, prev);
        match (r, l) {
            (Err(e), _) => Err(e),
            (Ok(_), Err(e)) => Err(e),
            (Ok(v), Ok(())) => Ok(v),
        }
    })();
    let reset = rt.call(&st.reset_tracking, &Value::Undefined, &[]);
    let v = res?;
    reset?;
    Ok(v)
}

/// `injectHook(type, hook, target, prepend)`. Returns the registered wrapper, or None when there is no target.
pub fn inject_hook(rt: &Rt, kind: LifeHook, hook: &Value, target: Option<ObjRef>, prepend: bool) -> Res<Option<Value>> {
    let Some(t) = target else { return Ok(None) };
    let rec = instance_of(rt, t);
    let wrapped = weh_of(rt, hook, t, kind)?;
    {
        let mut list = rec.hooks[kind as usize].borrow_mut();
        if prepend {
            list.insert(0, wrapped.clone());
        } else {
            list.push(wrapped.clone());
        }
    }
    Ok(Some(wrapped))
}

/// A forwarding function `(...args) => hook(...args)`. Data: [hook].
fn forward_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let hook = rt.fn_data(c.callee)[0].clone();
    rt.call(&hook, &Value::Undefined, c.args)
}

fn forwarder(rt: &Rt, hook: Value) -> Value {
    Value::Object(rt.new_native_function(forward_code, "", 0, vec![hook]))
}

fn hook_kind_of(rt: &Rt, data: &Value) -> Res<LifeHook> {
    match data {
        Value::String(s) => LifeHook::from_key(&s.to_rust_string_lossy())
            .ok_or_else(|| rt.throw_type_error("unknown lifecycle hook kind (framework bug)")),
        _ => Err(rt.throw_type_error("unknown lifecycle hook kind (framework bug)")),
    }
}

/// `createHook(lifecycle)`'s registration: `(hook, target = currentInstance) => injectHook(lifecycle, (...args) => hook(...args), target)`.
fn create_hook_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let kind = hook_kind_of(rt, &rt.fn_data(c.callee)[0])?;
    let target = resolve_target(rt, &c.arg(1))?;
    let fwd = forwarder(rt, c.arg(0));
    inject_hook(rt, kind, &fwd, target, false)?;
    Ok(Value::Undefined)
}

/// `onErrorCaptured(hook, target = currentInstance)`: `injectHook("ec", hook, target)` (no forwarder: the hook is wrapped).
fn on_error_captured_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let target = resolve_target(rt, &c.arg(1))?;
    inject_hook(rt, LifeHook::Ec, &c.arg(0), target, false)?;
    Ok(Value::Undefined)
}

/// The wrapper of `registerKeepAliveHook`. Data: [hook, target]. It skips the hook once an instance on the chain from
/// `target` is deactivated, and otherwise calls `hook()` with no arguments.
fn keep_alive_wrapper_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let mut cur = d[1].as_object();
    while let Some(o) = cur {
        let rec = instance_of(rt, o);
        if rec.is_deactivated.get() {
            return Ok(Value::Undefined);
        }
        cur = rec.parent.borrow().as_object();
    }
    rt.call(&d[0], &Value::Undefined, &[])
}

/// `injectToKeepAliveRoot(hook, type, target, keepAliveRoot)`: prepends the hook on the root, and removes it again when
/// `target` unmounts.
fn inject_to_keep_alive_root(rt: &Rt, hook: &Value, kind: LifeHook, target: ObjRef, root: ObjRef) -> Res<()> {
    let injected = inject_hook(rt, kind, hook, Some(root), true)?.expect("prepended keep-alive hook (framework bug)");
    let remover = Value::Object(rt.new_native_function(
        remove_injected_code,
        "",
        0,
        vec![Value::Object(root), Value::String(JsStr::from_str(kind.key())), injected],
    ));
    let fwd = forwarder(rt, remover);
    inject_hook(rt, LifeHook::Um, &fwd, Some(target), false)?;
    Ok(())
}

/// `() => remove(keepAliveRoot[type], injected)`. Data: [root, type, injected].
fn remove_injected_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let root = d[0].as_object().expect("keep-alive root (framework bug)");
    let kind = hook_kind_of(rt, &d[1])?;
    let rec = instance_of(rt, root);
    let mut list = rec.hooks[kind as usize].borrow_mut();
    if let Some(i) = list.iter().position(|v| strict_eq(v, &d[2])) {
        list.remove(i);
    }
    Ok(Value::Undefined)
}

/// `registerKeepAliveHook(hook, type, target = currentInstance)`.
fn register_keep_alive_hook(rt: &Rt, hook: &Value, kind: LifeHook, target: Option<ObjRef>) -> Res<()> {
    let cached = rt.get_str(hook, "__wdc")?;
    let wrapped = if cached.to_boolean() {
        cached
    } else {
        let w = Value::Object(rt.new_native_function(
            keep_alive_wrapper_code,
            "",
            0,
            vec![hook.clone(), target.map(Value::Object).unwrap_or(Value::Undefined)],
        ));
        rt.set_v_strict(hook, &PropKey::lit("__wdc"), w.clone())?;
        w
    };
    inject_hook(rt, kind, &wrapped, target, false)?;
    if let Some(t) = target {
        let mut cur = instance_of(rt, t).parent.borrow().as_object();
        while let Some(c) = cur {
            let Some(parent) = instance_of(rt, c).parent.borrow().as_object() else { break };
            let pvnode = instance_of(rt, parent).vnode.borrow().clone();
            let vtype = rt.get_str(&pvnode, "type")?;
            if rt.get_str(&vtype, "__isKeepAlive")?.to_boolean() {
                inject_to_keep_alive_root(rt, &wrapped, kind, t, c)?;
            }
            cur = Some(parent);
        }
    }
    Ok(())
}

fn keep_alive_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let kind = hook_kind_of(rt, &rt.fn_data(c.callee)[0])?;
    let target = resolve_target(rt, &c.arg(1))?;
    register_keep_alive_hook(rt, &c.arg(0), kind, target)?;
    Ok(Value::Undefined)
}

/// The lifecycle exports of the `vue` module. Names and lengths follow runtime-core 3.5.43.
pub fn exports(rt: &Rt) -> Vec<(&'static str, Value)> {
    let create = [
        ("onBeforeMount", LifeHook::Bm),
        ("onMounted", LifeHook::M),
        ("onBeforeUpdate", LifeHook::Bu),
        ("onUpdated", LifeHook::U),
        ("onBeforeUnmount", LifeHook::Bum),
        ("onUnmounted", LifeHook::Um),
        ("onServerPrefetch", LifeHook::Sp),
        ("onRenderTriggered", LifeHook::Rtg),
        ("onRenderTracked", LifeHook::Rtc),
    ];
    let mut out: Vec<(&'static str, Value)> = Vec::new();
    for (name, kind) in create {
        let data = vec![Value::String(JsStr::from_str(kind.key()))];
        out.push((name, Value::Object(rt.new_native_function(create_hook_code, name, 1, data))));
    }
    out.push(("onErrorCaptured", Value::Object(rt.new_native_function(on_error_captured_code, "onErrorCaptured", 1, Vec::new()))));
    for (name, kind) in [("onActivated", LifeHook::A), ("onDeactivated", LifeHook::Da)] {
        let data = vec![Value::String(JsStr::from_str(kind.key()))];
        out.push((name, Value::Object(rt.new_native_function(keep_alive_code, name, 2, data))));
    }
    out
}

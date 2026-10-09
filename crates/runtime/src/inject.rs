//! vn-runtime::inject: `provide`, `inject`, `hasInjectionContext`, `getCurrentInstance` and `useId` (runtime-core 3.5.43
//! apiInject, component.ts and the useId of the production build). Behaviour per @vue/runtime-core 3.5.43.
//!
//! `inject` reads the provides of the parent instance, or of the vnode's app context at the root. While an app runs
//! `runWithContext`, the app's `_context.provides` comes first. The `inject(key, default)` form is recognised by the
//! argument count, as in Vue (`arguments.length > 1`). `getCurrentInstance()` returns the instance handle, or null.
//! `useId()` is `(config.idPrefix || "v") + "-" + ids[0] + ids[1]++`, where `ids` is shared with the parent instance.

use vn_value::{strict_eq, Call, JsStr, PropKey, Res, Rt, Value};

use crate::instance::{current_app, current_instance, get_current_instance, instance_of, object_create};

/// `provide(key, value)`: defines `key` on the current instance's own provides object. The object is created lazily from
/// the parent's provides, so children do not change their parent's values.
pub fn provide(rt: &Rt, key: &Value, value: &Value) -> Res<()> {
    let Some(inst) = current_instance(rt) else { return Ok(()) };
    let rec = instance_of(rt, inst);
    let parent_obj = rec.parent.borrow().as_object();
    let parent_provides = match parent_obj {
        Some(p) => instance_of(rt, p).provides.borrow().clone(),
        None => Value::Null,
    };
    let mut provides = rec.provides.borrow().clone();
    if strict_eq(&parent_provides, &provides) {
        let created = object_create(rt, &parent_provides)?;
        provides = Value::Object(created);
        rec.provides.replace(provides.clone());
    }
    let k = rt.to_property_key(key)?;
    rt.set_v_strict(&provides, &k, value.clone())
}

/// `inject(key, defaultValue, treatDefaultAsFactory)`. `default` is None when the call had a single argument.
pub fn inject(rt: &Rt, key: &Value, default: Option<&Value>, as_factory: bool) -> Res<Value> {
    let inst = get_current_instance(rt);
    let app = current_app(rt);
    if inst.is_none() && !app.to_boolean() {
        return Ok(Value::Undefined);
    }
    let provides = if app.to_boolean() {
        let ctx = rt.get_str(&app, "_context")?;
        rt.get_str(&ctx, "provides")?
    } else {
        let inst = inst.expect("injection context (checked above)");
        let rec = instance_of(rt, inst);
        let parent = rec.parent.borrow().as_object();
        match parent {
            None => {
                // instance.vnode.appContext && instance.vnode.appContext.provides
                let vnode = rec.vnode.borrow().clone();
                let ac = rt.get_str(&vnode, "appContext")?;
                if ac.to_boolean() { rt.get_str(&ac, "provides")? } else { ac }
            }
            Some(p) => instance_of(rt, p).provides.borrow().clone(),
        }
    };
    if provides.to_boolean() && rt.in_operator(key, &provides)? {
        let k = rt.to_property_key(key)?;
        return rt.get_v(&provides, &k);
    }
    match default {
        Some(d) => {
            if as_factory && rt.is_callable(d) {
                let this = match inst {
                    Some(i) => instance_of(rt, i).proxy.borrow().clone(),
                    None => Value::Null,
                };
                rt.call(d, &this, &[])
            } else {
                Ok(d.clone())
            }
        }
        None => Ok(Value::Undefined),
    }
}

/// `hasInjectionContext()`.
pub fn has_injection_context(rt: &Rt) -> bool {
    get_current_instance(rt).is_some() || current_app(rt).to_boolean()
}

/// `useId()`: `""` outside an instance.
pub fn use_id(rt: &Rt) -> Res<Value> {
    let Some(inst) = get_current_instance(rt) else {
        return Ok(Value::String(JsStr::empty()));
    };
    let rec = instance_of(rt, inst);
    let ac = rec.app_context.borrow().clone();
    let config = rt.get_str(&ac, "config")?;
    let prefix = rt.get_str(&config, "idPrefix")?;
    let prefix = if prefix.to_boolean() { prefix } else { Value::String(JsStr::from_str("v")) };
    let ids = rec.ids.borrow().clone();
    let s0 = rt.get_v(&ids, &PropKey::Index(0))?;
    let n1 = rt.to_number(&rt.get_v(&ids, &PropKey::Index(1))?)?;
    rt.set_v_strict(&ids, &PropKey::Index(1), Value::Number(n1 + 1.0))?;
    let head = rt.to_string(&prefix)?.concat(&JsStr::from_str("-"));
    let head = head.concat(&rt.to_string(&s0)?);
    let out = head.concat(&rt.to_string(&Value::Number(n1))?);
    Ok(Value::String(out))
}

fn provide_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    provide(rt, &c.arg(0), &c.arg(1))?;
    Ok(Value::Undefined)
}

fn inject_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let default = if c.args.len() > 1 { Some(c.arg(1)) } else { None };
    inject(rt, &c.arg(0), default.as_ref(), c.arg(2).to_boolean())
}

fn has_injection_context_code(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(has_injection_context(rt)))
}

fn get_current_instance_code(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(get_current_instance(rt).map(Value::Object).unwrap_or(Value::Null))
}

fn use_id_code(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    use_id(rt)
}

/// The injection exports of the `vue` module.
pub fn exports(rt: &Rt) -> Vec<(&'static str, Value)> {
    vec![
        ("provide", Value::Object(rt.new_native_function(provide_code, "provide", 2, Vec::new()))),
        ("inject", Value::Object(rt.new_native_function(inject_code, "inject", 2, Vec::new()))),
        (
            "hasInjectionContext",
            Value::Object(rt.new_native_function(has_injection_context_code, "hasInjectionContext", 0, Vec::new())),
        ),
        (
            "getCurrentInstance",
            Value::Object(rt.new_native_function(get_current_instance_code, "getCurrentInstance", 0, Vec::new())),
        ),
        ("useId", Value::Object(rt.new_native_function(use_id_code, "useId", 0, Vec::new()))),
    ]
}

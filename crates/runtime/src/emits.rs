//! vn-runtime::emits: `emit`, normalizeEmitsOptions and isEmitListener of @vue/runtime-core 3.5.43 (production build).
//! The handler lookup, the `.trim`/`.number` model modifiers and the `Once` handlers follow runtime-core. Deviation: the
//! normalized emits are not cached per app context (same values, computed per instance).

use vn_value::{Call, Flags, JsStr, ObjRef, PropKey, Res, Rt, Value};

use crate::dom_props::enumerable_keys;
use crate::errors::{call_with_error_handling, ErrType};
use crate::instance::{empty_obj, instance_of};
use crate::normalize::{array_get, camelize, capitalize, hyphenate, length_of, loose_to_number};
use crate::props::{camel, has_own, hyph};
use crate::renderer::props_of;

/// `toHandlerKey(str)`: `on` + capitalized name, or the empty string.
pub(crate) fn to_handler_key(s: &str) -> String {
    if s.is_empty() {
        String::new()
    } else {
        format!("on{}", capitalize(&JsStr::from_str(s)).to_rust_string_lossy())
    }
}

/// `getModelModifiers(props, modelName)`.
pub(crate) fn model_modifiers(rt: &Rt, props: &Value, model: &str) -> Res<Value> {
    if model == "modelValue" || model == "model-value" {
        return rt.get_str(props, "modelModifiers");
    }
    let a = rt.get_str(props, &format!("{model}Modifiers"))?;
    if a.to_boolean() {
        return Ok(a);
    }
    let b = rt.get_str(props, &format!("{}Modifiers", camel(model)))?;
    if b.to_boolean() {
        return Ok(b);
    }
    rt.get_str(props, &format!("{}Modifiers", hyph(model)))
}

fn object_of(rt: &Rt, inst: ObjRef) -> Res<Value> {
    let vnode = instance_of(rt, inst).vnode.borrow().clone();
    let p = props_of(rt, match vnode {
        Value::Object(o) => o,
        _ => return Ok(empty_obj(rt)),
    });
    Ok(if p.to_boolean() { p } else { empty_obj(rt) })
}

/// `emit(instance, event, ...args)`.
pub(crate) fn emit(rt: &Rt, inst: ObjRef, event: &Value, raw_args: &[Value]) -> Res<()> {
    let rec = instance_of(rt, inst);
    if rec.is_unmounted.get() {
        return Ok(());
    }
    let props = object_of(rt, inst)?;
    let ev = rt.to_string(event)?.to_rust_string_lossy();
    let is_model = ev.starts_with("update:");
    let modifiers = if is_model { model_modifiers(rt, &props, &ev[7..])? } else { Value::Undefined };
    let mut args: Vec<Value> = raw_args.to_vec();
    if modifiers.to_boolean() {
        if rt.get_str(&modifiers, "trim")?.to_boolean() {
            args = args
                .into_iter()
                .map(|a| match &a {
                    Value::String(s) => Value::String(s.trim()),
                    _ => a,
                })
                .collect();
        }
        if rt.get_str(&modifiers, "number")?.to_boolean() {
            let mut out = Vec::with_capacity(args.len());
            for a in &args {
                out.push(loose_to_number(rt, a)?);
            }
            args = out;
        }
    }
    let mut handler_name = to_handler_key(&ev);
    let mut handler = rt.get_v(&props, &PropKey::from_str(&handler_name))?;
    if !handler.to_boolean() {
        handler_name = to_handler_key(&camelize(&JsStr::from_str(&ev)).to_rust_string_lossy());
        handler = rt.get_v(&props, &PropKey::from_str(&handler_name))?;
    }
    if !handler.to_boolean() && is_model {
        handler_name = to_handler_key(&hyphenate(&JsStr::from_str(&ev)).to_rust_string_lossy());
        handler = rt.get_v(&props, &PropKey::from_str(&handler_name))?;
    }
    if handler.to_boolean() {
        call_with_error_handling(rt, &handler, Some(inst), ErrType::Code(6), Some(&args))?;
    }
    let once_handler = rt.get_v(&props, &PropKey::from_str(&format!("{handler_name}Once")))?;
    if once_handler.to_boolean() {
        let emitted = rec.emitted.borrow().clone();
        let emitted_obj = if emitted.is_nullish() {
            let o = rt.new_object();
            *rec.emitted.borrow_mut() = Value::Object(o);
            o
        } else {
            let Value::Object(o) = emitted else {
                return Err(rt.throw_type_error("renderer: emitted is not an object (framework bug)"));
            };
            if rt.get_v(&Value::Object(o), &PropKey::from_str(&handler_name))?.to_boolean() {
                return Ok(());
            }
            o
        };
        rt.define_data(emitted_obj, PropKey::from_str(&handler_name), Value::Bool(true), Flags::ALL)?;
        call_with_error_handling(rt, &once_handler, Some(inst), ErrType::Code(6), Some(&args))?;
    }
    Ok(())
}

/// The bound `emit` of an instance (`instance.emit = emit.bind(null, instance)`).
pub(crate) fn emit_fn(rt: &Rt, inst: ObjRef) -> Value {
    Value::Object(rt.new_native_function(emit_code, "emit", 1, vec![Value::Object(inst)]))
}

fn emit_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let inst = match rt.fn_data(c.callee).first() {
        Some(Value::Object(o)) => *o,
        _ => panic!("emit function without its instance (framework bug)"),
    };
    let event = c.args.first().cloned().unwrap_or(Value::Undefined);
    let rest: &[Value] = if c.args.len() > 1 { &c.args[1..] } else { &[] };
    emit(rt, inst, &event, rest)?;
    Ok(Value::Undefined)
}

/// `normalizeEmitsOptions(comp)`: null when the component declares no emits.
pub(crate) fn normalize_emits_options(rt: &Rt, comp: &Value) -> Res<Value> {
    if !rt.is_callable(comp) {
        let ext = rt.get_str(comp, "extends")?;
        let mix = rt.get_str(comp, "mixins")?;
        if ext.to_boolean() || mix.to_boolean() {
            return Err(rt.throw_type_error("unsupported: component extends and mixins"));
        }
    }
    let raw = rt.get_str(comp, "emits")?;
    if !raw.to_boolean() {
        return Ok(Value::Null);
    }
    let normalized = rt.new_object();
    if rt.is_array(&raw) {
        for i in 0..length_of(rt, &raw)? as usize {
            let k = rt.to_string(&array_get(rt, &raw, i)?)?.to_rust_string_lossy();
            rt.define_data(normalized, PropKey::from_str(&k), Value::Null, Flags::ALL)?;
        }
    } else if let Value::Object(_) = raw {
        for k in enumerable_keys(rt, &raw)? {
            let v = rt.get_str(&raw, &k)?;
            rt.define_data(normalized, PropKey::from_str(&k), v, Flags::ALL)?;
        }
    }
    Ok(Value::Object(normalized))
}

/// `isEmitListener(options, key)`: `onXxx` names a declared event (plain, hyphenated or camelized, with `Once`).
pub(crate) fn is_emit_listener(rt: &Rt, options: &Value, key: &str) -> Res<bool> {
    if !options.to_boolean() || !crate::normalize::is_on(&JsStr::from_str(key)) {
        return Ok(false);
    }
    let rest = &key[2..];
    let name = if rest == "Once" { rest } else { rest.strip_suffix("Once").unwrap_or(rest) };
    let mut lower = String::new();
    let mut chars = name.chars();
    if let Some(first) = chars.next() {
        lower.extend(first.to_lowercase());
        lower.push_str(chars.as_str());
    }
    Ok(has_own(rt, options, &lower)? || has_own(rt, options, &hyph(name))? || has_own(rt, options, name)?)
}

//! vn-runtime::props: runtime props declarations (normalizePropsOptions), initProps, updateProps, setFullProps,
//! resolvePropValue and the change checks of @vue/runtime-core 3.5.43 (production build). Prop validation is a dev-only
//! feature and is absent. Deviation: the normalized options are recomputed for every instance (the app-level cache of
//! runtime-core returns the same values).

use vn_value::{same_value_zero, Flags, Intrinsic, JsStr, ObjRef, PropKey, Res, Rt, Value};

use crate::dom_props::enumerable_keys;
use crate::emits::is_emit_listener;
use crate::instance::{empty_obj, instance_of, with_instance};
use crate::normalize::{array_get, camelize, for_in_keys, hyphenate, length_of};
use crate::renderer::{dyn_props_of, is_reserved_prop, key_string, patch_of};

/// `camelize(key)` as a Rust string.
pub(crate) fn camel(s: &str) -> String {
    camelize(&JsStr::from_str(s)).to_rust_string_lossy()
}

/// `hyphenate(key)` as a Rust string.
pub(crate) fn hyph(s: &str) -> String {
    hyphenate(&JsStr::from_str(s)).to_rust_string_lossy()
}

/// `shared.hasOwn(obj, key)`; false for a non-object.
pub(crate) fn has_own(rt: &Rt, o: &Value, key: &str) -> Res<bool> {
    match o {
        Value::Object(obj) => rt.has_own_property(*obj, &PropKey::from_str(key)),
        _ => Ok(false),
    }
}

/// `obj[key] = value` for an object (a plain assignment, strict).
pub(crate) fn put(rt: &Rt, o: &Value, key: &str, v: Value) -> Res<()> {
    rt.set_v_strict(o, &PropKey::from_str(key), v)
}

/// `Object.create(internalObjectProto)` (the attrs and slots objects of runtime-core).
pub(crate) fn internal_object(rt: &Rt) -> ObjRef {
    let proto = rt.new_object();
    rt.new_object_with_proto(Some(proto))
}

/// `name` of a function value (`fn.name`), or None when it is not a function.
fn fn_name(rt: &Rt, v: &Value) -> Res<Option<String>> {
    if !rt.is_callable(v) {
        return Ok(None);
    }
    let n = rt.get_str(v, "name")?;
    Ok(Some(rt.to_string(&n)?.to_rust_string_lossy()))
}

/// A reactivity export (`shallowReactive`, `toRaw`, `markRaw`, `trigger` helpers) by name.
pub(crate) fn reactivity_fn(rt: &Rt, name: &str) -> Value {
    let list = vn_reactivity::exports(rt);
    if let Some((_, v)) = list.iter().find(|(n, _)| *n == name) {
        return v.clone();
    }
    let internal = vn_reactivity::internal_exports(rt);
    internal
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| panic!("reactivity export {name} (framework bug)"))
}

/// `toRaw(v)`.
pub(crate) fn to_raw(rt: &Rt, v: &Value) -> Res<Value> {
    let f = reactivity_fn(rt, "toRaw");
    rt.call(&f, &Value::Undefined, std::slice::from_ref(v))
}

fn valid_prop_name(k: &str) -> bool {
    !k.starts_with('$') && !is_reserved_prop(k)
}

/// `normalizePropsOptions(comp)`: `[normalized, needCastKeys]`, or an empty array when the component has no props.
pub(crate) fn normalize_props_options(rt: &Rt, comp: &Value) -> Res<Value> {
    if !rt.is_callable(comp) {
        let ext = rt.get_str(comp, "extends")?;
        let mix = rt.get_str(comp, "mixins")?;
        if ext.to_boolean() || mix.to_boolean() {
            return Err(rt.throw_type_error("unsupported: component extends and mixins"));
        }
    }
    let raw = rt.get_str(comp, "props")?;
    if !raw.to_boolean() {
        return Ok(Value::Object(rt.new_array(Vec::new())));
    }
    let normalized = rt.new_object();
    let mut need_cast: Vec<Value> = Vec::new();
    if rt.is_array(&raw) {
        for i in 0..length_of(rt, &raw)? as usize {
            let item = array_get(rt, &raw, i)?;
            let k = camel(&rt.to_string(&item)?.to_rust_string_lossy());
            if valid_prop_name(&k) {
                rt.define_data(normalized, PropKey::from_str(&k), Value::Object(empty_obj_value(rt)), Flags::ALL)?;
            }
        }
    } else {
        for key in for_in_keys(rt, &raw)? {
            let k = key_string(&key);
            let nk = camel(&k);
            if !valid_prop_name(&nk) {
                continue;
            }
            let opt = rt.get_v(&raw, &key)?;
            let prop = rt.new_object();
            if rt.is_array(&opt) || rt.is_callable(&opt) {
                rt.define_data(prop, PropKey::from_str("type"), opt.clone(), Flags::ALL)?;
            } else if let Value::Object(_) = opt {
                for pk in enumerable_keys(rt, &opt)? {
                    let v = rt.get_str(&opt, &pk)?;
                    rt.define_data(prop, PropKey::from_str(&pk), v, Flags::ALL)?;
                }
            }
            let propv = Value::Object(prop);
            rt.define_data(normalized, PropKey::from_str(&nk), propv.clone(), Flags::ALL)?;
            let prop_type = rt.get_str(&propv, "type")?;
            let mut should_cast = false;
            let mut should_cast_true = true;
            if rt.is_array(&prop_type) {
                for i in 0..length_of(rt, &prop_type)? as usize {
                    let t = array_get(rt, &prop_type, i)?;
                    match fn_name(rt, &t)?.as_deref() {
                        Some("Boolean") => {
                            should_cast = true;
                            break;
                        }
                        Some("String") => should_cast_true = false,
                        _ => {}
                    }
                }
            } else {
                should_cast = fn_name(rt, &prop_type)?.as_deref() == Some("Boolean");
            }
            rt.define_data(prop, PropKey::Index(0), Value::Bool(should_cast), Flags::ALL)?;
            rt.define_data(prop, PropKey::Index(1), Value::Bool(should_cast_true), Flags::ALL)?;
            if should_cast || has_own(rt, &propv, "default")? {
                need_cast.push(Value::String(JsStr::from_str(&nk)));
            }
        }
    }
    let cast_arr = rt.new_array(need_cast);
    Ok(Value::Object(rt.new_array(vec![Value::Object(normalized), Value::Object(cast_arr)])))
}

fn empty_obj_value(rt: &Rt) -> ObjRef {
    match empty_obj(rt) {
        Value::Object(o) => o,
        _ => unreachable!("EMPTY_OBJ is an object"),
    }
}

/// `propsOptions[0]` and `propsOptions[1]` of an instance.
fn options_of(rt: &Rt, inst: ObjRef) -> Res<(Value, Value)> {
    let po = instance_of(rt, inst).props_options.borrow().clone();
    if !po.to_boolean() {
        return Ok((Value::Undefined, Value::Undefined));
    }
    Ok((rt.get_v(&po, &PropKey::Index(0))?, rt.get_v(&po, &PropKey::Index(1))?))
}

/// `initProps(instance, rawProps, isStateful)`.
pub(crate) fn init_props(rt: &Rt, inst: ObjRef, raw: &Value, is_stateful: bool) -> Res<()> {
    let rec = instance_of(rt, inst);
    let props = rt.new_object();
    let attrs = internal_object(rt);
    *rec.props_defaults.borrow_mut() = Value::Object(rt.new_object_with_proto(None));
    set_full_props(rt, inst, raw, props, attrs)?;
    let (options, _) = options_of(rt, inst)?;
    if let Value::Object(o0) = options {
        for key in for_in_keys(rt, &Value::Object(o0))? {
            if !rt.has_property(props, &key)? {
                rt.define_data(props, key, Value::Undefined, Flags::ALL)?;
            }
        }
    }
    let props_v = Value::Object(props);
    if is_stateful {
        let sr = reactivity_fn(rt, "shallowReactive");
        let reactive = rt.call(&sr, &Value::Undefined, std::slice::from_ref(&props_v))?;
        *rec.props.borrow_mut() = reactive;
    } else {
        let ty = rec.ty.borrow().clone();
        let ty_props = rt.get_str(&ty, "props")?;
        *rec.props.borrow_mut() = if ty_props.to_boolean() { props_v } else { Value::Object(attrs) };
    }
    *rec.attrs.borrow_mut() = Value::Object(attrs);
    Ok(())
}

/// `setFullProps(instance, rawProps, props, attrs)`: sorts the raw props into props and attrs, then casts the keys that
/// need it. Returns whether an attribute changed.
pub(crate) fn set_full_props(rt: &Rt, inst: ObjRef, raw: &Value, props: ObjRef, attrs: ObjRef) -> Res<bool> {
    let (options, need_cast) = options_of(rt, inst)?;
    let emits = instance_of(rt, inst).emits_options.borrow().clone();
    let mut changed = false;
    let mut raw_cast: Option<ObjRef> = None;
    if raw.to_boolean() {
        for key in for_in_keys(rt, raw)? {
            let k = key_string(&key);
            if is_reserved_prop(&k) {
                continue;
            }
            let value = rt.get_v(raw, &key)?;
            let ck = camel(&k);
            if options.to_boolean() && has_own(rt, &options, &ck)? {
                let is_cast = need_cast.to_boolean() && includes(rt, &need_cast, &Value::String(JsStr::from_str(&ck)))?;
                if !is_cast {
                    rt.set_strict(props, &PropKey::from_str(&ck), value)?;
                } else {
                    let obj = match raw_cast {
                        Some(o) => o,
                        None => {
                            let o = rt.new_object();
                            raw_cast = Some(o);
                            o
                        }
                    };
                    rt.define_data(obj, PropKey::from_str(&ck), value, Flags::ALL)?;
                }
            } else if !is_emit_listener(rt, &emits, &k)? {
                let pk = PropKey::from_str(&k);
                let in_attrs = rt.has_property(attrs, &pk)?;
                let differs = if in_attrs {
                    let cur = rt.get_v(&Value::Object(attrs), &pk)?;
                    !vn_value::strict_eq(&value, &cur)
                } else {
                    true
                };
                if !in_attrs || differs {
                    rt.set_strict(attrs, &pk, value)?;
                    changed = true;
                }
            }
        }
    }
    if need_cast.to_boolean() {
        let raw_current = to_raw(rt, &Value::Object(props))?;
        let cast_values = match raw_cast {
            Some(o) => Value::Object(o),
            None => empty_obj(rt),
        };
        for i in 0..length_of(rt, &need_cast)? as usize {
            let key = rt.to_string(&array_get(rt, &need_cast, i)?)?.to_rust_string_lossy();
            let cur = rt.get_v(&cast_values, &PropKey::from_str(&key))?;
            let absent = !has_own(rt, &cast_values, &key)?;
            let v = resolve_prop_value(rt, inst, &options, &raw_current, &key, cur, absent)?;
            rt.set_strict(props, &PropKey::from_str(&key), v)?;
        }
    }
    Ok(changed)
}

fn includes(rt: &Rt, arr: &Value, v: &Value) -> Res<bool> {
    for i in 0..length_of(rt, arr)? as usize {
        if same_value_zero(&array_get(rt, arr, i)?, v) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `resolvePropValue(options, props, key, value, instance, isAbsent)`: defaults (factories run with the instance
/// current) and boolean casting.
pub(crate) fn resolve_prop_value(
    rt: &Rt,
    inst: ObjRef,
    options: &Value,
    props: &Value,
    key: &str,
    value: Value,
    is_absent: bool,
) -> Res<Value> {
    let opt = rt.get_v(options, &PropKey::from_str(key))?;
    if opt.is_nullish() {
        return Ok(value);
    }
    let mut value = value;
    let has_default = has_own(rt, &opt, "default")?;
    if has_default && matches!(value, Value::Undefined) {
        let default_value = rt.get_str(&opt, "default")?;
        let function_ctor = {
            let g = Value::Object(rt.intrinsic(Intrinsic::Global));
            rt.get_str(&g, "Function")?
        };
        let ty = rt.get_str(&opt, "type")?;
        let skip = rt.get_str(&opt, "skipFactory")?;
        if !vn_value::strict_eq(&ty, &function_ctor) && !skip.to_boolean() && rt.is_callable(&default_value) {
            let pd = instance_of(rt, inst).props_defaults.borrow().clone();
            if rt.has_property(obj_of(rt, &pd)?, &PropKey::from_str(key))? {
                value = rt.get_str(&pd, key)?;
            } else {
                let props_v = props.clone();
                value = with_instance(rt, inst, |rt| {
                    let v = rt.call(&default_value, &Value::Null, std::slice::from_ref(&props_v))?;
                    put(rt, &pd, key, v.clone())?;
                    Ok(v)
                })?;
            }
        } else {
            value = default_value;
        }
    }
    let should_cast = rt.get_v(&opt, &PropKey::Index(0))?;
    if should_cast.to_boolean() {
        let should_cast_true = rt.get_v(&opt, &PropKey::Index(1))?;
        if is_absent && !has_default {
            value = Value::Bool(false);
        } else if should_cast_true.to_boolean() {
            let is_empty = matches!(&value, Value::String(s) if s.is_empty());
            let is_hyph = matches!(&value, Value::String(s) if s.to_rust_string_lossy() == hyph(key));
            if is_empty || is_hyph {
                value = Value::Bool(true);
            }
        }
    }
    Ok(value)
}

fn obj_of(rt: &Rt, v: &Value) -> Res<ObjRef> {
    match v {
        Value::Object(o) => Ok(*o),
        _ => Err(rt.throw_type_error("renderer: props defaults is not an object (framework bug)")),
    }
}

/// `updateProps(instance, rawProps, rawPrevProps, optimized)`.
pub(crate) fn update_props(rt: &Rt, inst: ObjRef, raw_props: &Value, raw_prev: &Value, optimized: bool) -> Res<()> {
    let rec = instance_of(rt, inst);
    let props = rec.props.borrow().clone();
    let attrs = rec.attrs.borrow().clone();
    let vnode = rec.vnode.borrow().clone();
    let vnode_obj = match vnode {
        Value::Object(o) => o,
        _ => return Err(rt.throw_type_error("renderer: instance without a vnode (framework bug)")),
    };
    let patch = patch_of(rt, vnode_obj);
    let raw_current = to_raw(rt, &props)?;
    let (options, _) = options_of(rt, inst)?;
    let emits = rec.emits_options.borrow().clone();
    let props_o = obj_of(rt, &props)?;
    let attrs_o = obj_of(rt, &attrs)?;
    let mut changed = false;
    if (optimized || patch > 0) && patch & crate::shapes::PatchFlags::FULL_PROPS == 0 {
        if patch & crate::shapes::PatchFlags::PROPS != 0 {
            let dyn_props = dyn_props_of(rt, vnode_obj);
            for i in 0..length_of(rt, &dyn_props)? as usize {
                let k = rt.to_string(&array_get(rt, &dyn_props, i)?)?.to_rust_string_lossy();
                if is_emit_listener(rt, &emits, &k)? {
                    continue;
                }
                let pk = PropKey::from_str(&k);
                let value = rt.get_v(raw_props, &pk)?;
                if options.to_boolean() {
                    if has_own(rt, &attrs, &k)? {
                        let cur = rt.get_v(&attrs, &pk)?;
                        if !vn_value::strict_eq(&value, &cur) {
                            rt.set_strict(attrs_o, &pk, value)?;
                            changed = true;
                        }
                    } else {
                        let ck = camel(&k);
                        let v = resolve_prop_value(rt, inst, &options, &raw_current, &ck, value, false)?;
                        rt.set_strict(props_o, &PropKey::from_str(&ck), v)?;
                    }
                } else {
                    let cur = rt.get_v(&attrs, &pk)?;
                    if !vn_value::strict_eq(&value, &cur) {
                        rt.set_strict(attrs_o, &pk, value)?;
                        changed = true;
                    }
                }
            }
        }
    } else {
        if set_full_props(rt, inst, raw_props, props_o, attrs_o)? {
            changed = true;
        }
        for key in for_in_keys(rt, &raw_current)? {
            let k = key_string(&key);
            let kebab = hyph(&k);
            let raw_has = has_own(rt, raw_props, &k)?;
            let kebab_has = has_own(rt, raw_props, &kebab)?;
            let stale = !raw_props.to_boolean() || (!raw_has && (kebab == k || !kebab_has));
            if stale {
                if options.to_boolean() {
                    let prev_k = rt.get_v(raw_prev, &PropKey::from_str(&k))?;
                    let prev_kebab = rt.get_v(raw_prev, &PropKey::from_str(&kebab))?;
                    if raw_prev.to_boolean() && (!matches!(prev_k, Value::Undefined) || !matches!(prev_kebab, Value::Undefined)) {
                        let v = resolve_prop_value(rt, inst, &options, &raw_current, &k, Value::Undefined, true)?;
                        rt.set_strict(props_o, &key, v)?;
                    }
                } else {
                    rt.delete(props_o, &key)?;
                }
            }
        }
        if !vn_value::strict_eq(&attrs, &raw_current) {
            for key in for_in_keys(rt, &attrs)? {
                let k = key_string(&key);
                if !raw_props.to_boolean() || !has_own(rt, raw_props, &k)? {
                    rt.delete(attrs_o, &key)?;
                    changed = true;
                }
            }
        }
    }
    if changed {
        vn_reactivity::trigger(rt, attrs_o, vn_reactivity::TriggerOp::Set, &Value::String(JsStr::from_str("")), &Value::Undefined)?;
    }
    Ok(())
}

/// `hasPropsChanged(prev, next, emits)`.
pub(crate) fn has_props_changed(rt: &Rt, prev: &Value, next: &Value, emits: &Value) -> Res<bool> {
    let next_keys = enumerable_keys(rt, next)?;
    if next_keys.len() != enumerable_keys(rt, prev)?.len() {
        return Ok(true);
    }
    for k in next_keys {
        if has_prop_value_changed(rt, next, prev, &k)? && !is_emit_listener(rt, emits, &k)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `hasPropValueChanged(next, prev, key)`: a style object is compared loosely (deep), anything else by identity.
pub(crate) fn has_prop_value_changed(rt: &Rt, next: &Value, prev: &Value, key: &str) -> Res<bool> {
    let np = rt.get_str(next, key)?;
    let pp = rt.get_str(prev, key)?;
    let is_obj = |v: &Value| matches!(v, Value::Object(_)) && !rt.is_callable(v);
    if key == "style" && is_obj(&np) && is_obj(&pp) {
        return Ok(!crate::normalize::loose_equal(rt, &np, &pp)?);
    }
    Ok(!vn_value::strict_eq(&np, &pp))
}

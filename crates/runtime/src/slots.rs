//! vn-runtime::slots: `initSlots`, `updateSlots` and the slot normalization of @vue/runtime-core 3.5.43 (production build),
//! and `withCtx` (the rendering-instance and block-tracking wrapper of compiled slot functions). Slot flags: STABLE (1),
//! DYNAMIC (2) and FORWARDED (3) are read from the children's `_` field as in runtime-core.

use vn_value::{Call, Flags, ObjRef, PropKey, Res, Rt, Value};

use crate::instance::{empty_obj, inst_state, instance_of};
use crate::normalize::{array_get, for_in_keys, length_of};
use crate::props::{internal_object, put};
use crate::renderer::{renderer, shape_of};
use crate::shapes::ShapeFlags;

/// `isInternalKey`: `_`, `_ctx` and `$stable` are not slots.
fn is_internal_key(k: &str) -> bool {
    k == "_" || k == "_ctx" || k == "$stable"
}

fn obj(rt: &Rt, v: &Value) -> Res<ObjRef> {
    match v {
        Value::Object(o) => Ok(*o),
        _ => Err(rt.throw_type_error("renderer: slots object expected (framework bug)")),
    }
}

/// The rendering instance (`currentRenderingInstance`).
pub(crate) fn current_rendering(rt: &Rt) -> Option<ObjRef> {
    inst_state(rt).rendering.get()
}

/// `setBlockTracking(delta)` of the vnode layer (the export registered by `renderer::vnode_state`).
pub(crate) fn set_block_tracking(rt: &Rt, delta: f64) -> Res<()> {
    let f = crate::renderer::block_tracking_fn(rt);
    rt.call(&Value::Object(f), &Value::Undefined, &[Value::Number(delta)]).map(|_| ())
}

/// `withCtx(fn, ctx)`. `ctx` undefined means the current rendering instance; null means no context (fn itself).
pub(crate) fn with_ctx(rt: &Rt, f: Value, ctx: Value) -> Res<Value> {
    let ctx_obj = match ctx {
        Value::Object(o) => Some(o),
        Value::Undefined => current_rendering(rt),
        _ => None,
    };
    let Some(ctx_obj) = ctx_obj else { return Ok(f) };
    if rt.get_str(&f, "_n")?.to_boolean() {
        return Ok(f);
    }
    let wrapper = rt.new_native_function(with_ctx_code, "", 0, vec![f, Value::Object(ctx_obj)]);
    let w = Value::Object(wrapper);
    put(rt, &w, "_n", Value::Bool(true))?;
    put(rt, &w, "_c", Value::Bool(true))?;
    put(rt, &w, "_d", Value::Bool(true))?;
    Ok(w)
}

/// `renderFnWithContext(...args)`: runs `fn` with `ctx` as the rendering instance, block tracking off when `_d` is set.
fn with_ctx_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let f = data.first().cloned().unwrap_or(Value::Undefined);
    let ctx = match data.get(1) {
        Some(Value::Object(o)) => *o,
        _ => panic!("withCtx wrapper without its context (framework bug)"),
    };
    let me = Value::Object(c.callee);
    let disable = rt.get_str(&me, "_d")?.to_boolean();
    if disable {
        set_block_tracking(rt, -1.0)?;
    }
    let prev = crate::instance::set_current_rendering_instance(rt, Some(ctx));
    let res = rt.call(&f, &Value::Undefined, c.args);
    crate::instance::set_current_rendering_instance(rt, prev);
    if rt.get_str(&me, "_d")?.to_boolean() {
        set_block_tracking(rt, 1.0)?;
    }
    res
}

/// `normalizeSlotValue(value)`: an array of normalized vnodes.
pub(crate) fn normalize_slot_value(rt: &Rt, v: &Value) -> Res<Value> {
    let r = renderer(rt)?;
    let mut out = Vec::new();
    if rt.is_array(v) {
        for i in 0..length_of(rt, v)? as usize {
            let item = array_get(rt, v, i)?;
            let n = r.with_vst(|st| crate::vnode::normalize_vnode(rt, st, &item))?;
            out.push(Value::Object(n));
        }
    } else {
        let n = r.with_vst(|st| crate::vnode::normalize_vnode(rt, st, v))?;
        out.push(Value::Object(n));
    }
    Ok(Value::Object(rt.new_array(out)))
}

/// `normalizeSlot(key, rawSlot, ctx)`.
fn normalize_slot(rt: &Rt, raw: &Value, ctx: &Value) -> Res<Value> {
    if rt.get_str(raw, "_n")?.to_boolean() {
        return Ok(raw.clone());
    }
    let f = Value::Object(rt.new_native_function(normalized_slot_code, "", 0, vec![raw.clone()]));
    let normalized = with_ctx(rt, f, ctx.clone())?;
    put(rt, &normalized, "_c", Value::Bool(false))?;
    Ok(normalized)
}

fn normalized_slot_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let raw = rt.fn_data(c.callee).first().cloned().unwrap_or(Value::Undefined);
    let v = rt.call(&raw, &Value::Undefined, c.args)?;
    normalize_slot_value(rt, &v)
}

/// A function returning a fixed value (`() => normalized`).
fn constant_fn(rt: &Rt, v: Value) -> Value {
    Value::Object(rt.new_native_function(constant_code, "", 0, vec![v]))
}

fn constant_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(rt.fn_data(c.callee).first().cloned().unwrap_or(Value::Undefined))
}

/// `normalizeObjectSlots(rawSlots, slots)`.
fn normalize_object_slots(rt: &Rt, raw: &Value, slots: ObjRef) -> Res<()> {
    let ctx = rt.get_str(raw, "_ctx")?;
    for key in for_in_keys(rt, raw)? {
        let k = crate::renderer::key_string(&key);
        if is_internal_key(&k) {
            continue;
        }
        let value = rt.get_v(raw, &key)?;
        if rt.is_callable(&value) {
            let n = normalize_slot(rt, &value, &ctx)?;
            put(rt, &Value::Object(slots), &k, n)?;
        } else if !value.is_nullish() {
            let normalized = normalize_slot_value(rt, &value)?;
            put(rt, &Value::Object(slots), &k, constant_fn(rt, normalized))?;
        }
    }
    Ok(())
}

/// `normalizeVNodeSlots(instance, children)`: a single slot content becomes the default slot.
fn normalize_vnode_slots(rt: &Rt, inst: ObjRef, children: &Value) -> Res<()> {
    let normalized = normalize_slot_value(rt, children)?;
    let slots = instance_of(rt, inst).slots.borrow().clone();
    put(rt, &slots, "default", constant_fn(rt, normalized))
}

/// `assignSlots(slots, children, optimized)`.
fn assign_slots(rt: &Rt, slots: ObjRef, children: &Value, optimized: bool) -> Res<()> {
    for key in for_in_keys(rt, children)? {
        let k = crate::renderer::key_string(&key);
        if optimized || !is_internal_key(&k) {
            let v = rt.get_v(children, &key)?;
            rt.set_strict(slots, &key, v)?;
        }
    }
    Ok(())
}

/// `initSlots(instance, children, optimized)`.
pub(crate) fn init_slots(rt: &Rt, inst: ObjRef, children: &Value, optimized: bool) -> Res<()> {
    let rec = instance_of(rt, inst);
    let slots = internal_object(rt);
    *rec.slots.borrow_mut() = Value::Object(slots);
    let vnode = rec.vnode.borrow().clone();
    let shape = shape_of(rt, obj(rt, &vnode)?);
    if shape & ShapeFlags::SLOTS_CHILDREN != 0 {
        let ty = rt.get_str(children, "_")?;
        if ty.to_boolean() {
            assign_slots(rt, slots, children, optimized)?;
            if optimized {
                rt.define_data(slots, PropKey::lit("_"), ty, Flags::new(true, false, true))?;
            }
        } else {
            normalize_object_slots(rt, children, slots)?;
        }
    } else if children.to_boolean() {
        normalize_vnode_slots(rt, inst, children)?;
    }
    Ok(())
}

/// `updateSlots(instance, children, optimized)`.
pub(crate) fn update_slots(rt: &Rt, inst: ObjRef, children: &Value, optimized: bool) -> Res<()> {
    let rec = instance_of(rt, inst);
    let vnode = rec.vnode.borrow().clone();
    let slots = rec.slots.borrow().clone();
    let slots_o = obj(rt, &slots)?;
    let mut need_deletion_check = true;
    let mut target = empty_obj(rt);
    let shape = shape_of(rt, obj(rt, &vnode)?);
    if shape & ShapeFlags::SLOTS_CHILDREN != 0 {
        let ty = rt.get_str(children, "_")?;
        if ty.to_boolean() {
            if optimized && matches!(ty, Value::Number(n) if n == 1.0) {
                need_deletion_check = false;
            } else {
                assign_slots(rt, slots_o, children, optimized)?;
            }
        } else {
            need_deletion_check = !rt.get_str(children, "$stable")?.to_boolean();
            normalize_object_slots(rt, children, slots_o)?;
        }
        target = children.clone();
    } else if children.to_boolean() {
        normalize_vnode_slots(rt, inst, children)?;
        let t = rt.new_object();
        rt.define_data(t, PropKey::lit("default"), Value::Number(1.0), Flags::ALL)?;
        target = Value::Object(t);
    }
    if need_deletion_check {
        for key in for_in_keys(rt, &slots)? {
            let k = crate::renderer::key_string(&key);
            if !is_internal_key(&k) && rt.get_v(&target, &key)?.is_nullish() {
                rt.delete(slots_o, &key)?;
            }
        }
    }
    Ok(())
}

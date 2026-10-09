//! vn-runtime::refs: template refs (`setRef` and the binding of `useTemplateRef`) of @vue/runtime-core 3.5.43
//! (production build): string refs, ref objects, function refs, `ref_for` arrays, the deferred set of the post-flush
//! queue (job id -1) and the clearing on unmount.
//! Deviation: a vnode's ref object carries no owner (`i` is null, vnode.rs), so the owner is the component that patches
//! the vnode (`parent`). Refs inside slot content therefore belong to the child that renders the slot, not the parent.

use vn_value::{Call, NativeObject, ObjRef, Prop, PropKey, Res, Rt, Tracer, Value};

use crate::errors::{call_with_error_handling, ErrType};
use crate::instance::{empty_obj, instance_of};
use crate::props::{has_own, put, reactivity_fn};
use crate::renderer::{component_of, el_of, renderer, shape_of, Renderer};
use crate::scheduler::{job_meta, queue_post_render_effect, DISPOSED};
use crate::shapes::ShapeFlags;

/// `isTemplateRefKey(refs, key)`: a non-configurable own accessor of the refs object (defined by useTemplateRef).
fn is_template_ref_key(rt: &Rt, refs: &Value, key: &str) -> Res<bool> {
    let Value::Object(o) = refs else { return Ok(false) };
    Ok(matches!(rt.get_own_property(*o, &PropKey::from_str(key))?, Some(Prop::Accessor { flags, .. }) if !flags.configurable))
}

fn is_string(v: &Value) -> bool {
    matches!(v, Value::String(_))
}

fn is_ref(rt: &Rt, v: &Value) -> Res<bool> {
    let f = reactivity_fn(rt, "isRef");
    Ok(rt.call(&f, &Value::Undefined, std::slice::from_ref(v))?.to_boolean())
}

fn str_of(rt: &Rt, v: &Value) -> Res<String> {
    Ok(rt.to_string(v)?.to_rust_string_lossy())
}

/// The fields of a normalized ref object `{ i, r, k, f }`.
struct RawRef {
    owner: Value,
    r: Value,
    k: Value,
    in_for: bool,
}

fn raw_ref(rt: &Rt, raw: &Value, fallback: Option<ObjRef>) -> Res<RawRef> {
    let owner = rt.get_str(raw, "i")?;
    let owner = if owner.is_nullish() {
        match fallback {
            Some(p) => Value::Object(p),
            None => Value::Null,
        }
    } else {
        owner
    };
    Ok(RawRef {
        owner,
        r: rt.get_str(raw, "r")?,
        k: rt.get_str(raw, "k")?,
        in_for: rt.get_str(raw, "f")?.to_boolean(),
    })
}

/// The pending set of a ref (the `pendingSetRefMap` of runtime-core, keyed by the raw ref object).
pub(crate) struct PendingRefs(pub(crate) Vec<(ObjRef, ObjRef)>);

impl NativeObject for PendingRefs {
    fn trace(&self, t: &mut Tracer) {
        for (a, b) in &self.0 {
            t.mark(*a);
            t.mark(*b);
        }
    }
}

fn owner_refs(rt: &Rt, owner: &Value) -> Res<Value> {
    let o = instance_of(rt, match owner {
        Value::Object(o) => *o,
        _ => return Err(rt.throw_type_error("renderer: ref owner is not a component instance (framework bug)")),
    });
    let cur = o.refs.borrow().clone();
    if matches!(&cur, Value::Object(c) if *c == empty_obj_obj(rt)) {
        let fresh = Value::Object(rt.new_object());
        *o.refs.borrow_mut() = fresh.clone();
        return Ok(fresh);
    }
    Ok(cur)
}

fn empty_obj_obj(rt: &Rt) -> ObjRef {
    match empty_obj(rt) {
        Value::Object(o) => o,
        _ => unreachable!("EMPTY_OBJ is an object"),
    }
}

fn setup_state_of(rt: &Rt, owner: &Value) -> Res<Value> {
    match owner {
        Value::Object(o) => Ok(instance_of(rt, *o).setup_state.borrow().clone()),
        _ => Ok(Value::Null),
    }
}

/// `canSetSetupRef(key)`: a setup binding that is not a template ref.
fn can_set_setup_ref(rt: &Rt, refs: &Value, setup: &Value, key: &str) -> Res<bool> {
    if matches!(setup, Value::Object(s) if *s == empty_obj_obj(rt)) {
        return Ok(false);
    }
    if is_template_ref_key(rt, refs, key)? {
        return Ok(false);
    }
    let raw = to_raw(rt, setup)?;
    has_own(rt, &raw, key)
}

fn to_raw(rt: &Rt, v: &Value) -> Res<Value> {
    crate::props::to_raw(rt, v)
}

/// `canSetRef(ref, key)`: a ref value can be written unless it is a template ref key.
fn can_set_ref(rt: &Rt, refs: &Value, key: &Value) -> Res<bool> {
    if key.to_boolean() {
        let k = str_of(rt, key)?;
        if is_template_ref_key(rt, refs, &k)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `shared.remove(arr, i)`: drops element `i`, shifting the rest down.
fn remove_index(rt: &Rt, arr: &Value, i: usize) -> Res<()> {
    let n = crate::normalize::length_of(rt, arr)? as usize;
    let mut items = Vec::with_capacity(n);
    for j in 0..n {
        items.push(crate::normalize::array_get(rt, arr, j)?);
    }
    items.remove(i);
    for (j, v) in items.iter().enumerate() {
        rt.set_v_strict(arr, &PropKey::Index(j as u32), v.clone())?;
    }
    rt.set_str(arr, "length", Value::Number(items.len() as f64))?;
    Ok(())
}

fn array_index_of(rt: &Rt, arr: &Value, v: &Value) -> Res<Option<usize>> {
    let n = crate::normalize::length_of(rt, arr)? as usize;
    for i in 0..n {
        if vn_value::strict_eq(&crate::normalize::array_get(rt, arr, i)?, v) {
            return Ok(Some(i));
        }
    }
    Ok(None)
}

/// `doSet` of setRef: writes the ref value (once the post-flush queue reaches it, or immediately for a null value).
fn do_set(rt: &Rt, raw: &Value, ref_value: &Value, is_unmount: bool, fallback: Option<ObjRef>) -> Res<()> {
    let rr = raw_ref(rt, raw, fallback)?;
    let refs = owner_refs(rt, &rr.owner)?;
    let setup = setup_state_of(rt, &rr.owner)?;
    if rr.in_for {
        let existing = if is_string(&rr.r) {
            let rk = str_of(rt, &rr.r)?;
            if can_set_setup_ref(rt, &refs, &setup, &rk)? {
                rt.get_v(&setup, &PropKey::from_str(&rk))?
            } else {
                rt.get_v(&refs, &PropKey::from_str(&rk))?
            }
        } else if can_set_ref(rt, &refs, &Value::Undefined)? || !rr.k.to_boolean() {
            rt.get_str(&rr.r, "value")?
        } else {
            Value::Undefined
        };
        if is_unmount {
            if rt.is_array(&existing) {
                if let Some(i) = array_index_of(rt, &existing, ref_value)? {
                    remove_index(rt, &existing, i)?;
                }
            }
        } else if !rt.is_array(&existing) {
            if is_string(&rr.r) {
                let rk = str_of(rt, &rr.r)?;
                let list = Value::Object(rt.new_array(vec![ref_value.clone()]));
                put(rt, &refs, &rk, list.clone())?;
                if can_set_setup_ref(rt, &refs, &setup, &rk)? {
                    put(rt, &setup, &rk, list)?;
                }
            } else {
                let list = Value::Object(rt.new_array(vec![ref_value.clone()]));
                if can_set_ref(rt, &refs, &rr.k)? {
                    rt.set_str(&rr.r, "value", list.clone())?;
                }
                if rr.k.to_boolean() {
                    put(rt, &refs, &str_of(rt, &rr.k)?, list)?;
                }
            }
        } else if array_index_of(rt, &existing, ref_value)?.is_none() {
            let n = crate::normalize::length_of(rt, &existing)? as u32;
            rt.set_v_strict(&existing, &PropKey::Index(n), ref_value.clone())?;
        }
    } else if is_string(&rr.r) {
        let rk = str_of(rt, &rr.r)?;
        put(rt, &refs, &rk, ref_value.clone())?;
        if can_set_setup_ref(rt, &refs, &setup, &rk)? {
            put(rt, &setup, &rk, ref_value.clone())?;
        }
    } else if is_ref(rt, &rr.r)? {
        if can_set_ref(rt, &refs, &rr.k)? {
            rt.set_str(&rr.r, "value", ref_value.clone())?;
        }
        if rr.k.to_boolean() {
            put(rt, &refs, &str_of(rt, &rr.k)?, ref_value.clone())?;
        }
    }
    Ok(())
}

/// The deferred `doSet` job (`job.id = -1`): runs once, then forgets its pending entry.
fn set_ref_job_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let raw = data.first().cloned().unwrap_or(Value::Undefined);
    let value = data.get(1).cloned().unwrap_or(Value::Undefined);
    let fallback = match data.get(3) {
        Some(Value::Object(o)) => Some(*o),
        _ => None,
    };
    do_set(rt, &raw, &value, false, fallback)?;
    if let Value::Object(r) = raw {
        renderer(rt)?.forget_pending(r);
    }
    Ok(Value::Undefined)
}

impl Renderer {
    fn forget_pending(&self, raw: ObjRef) {
        let mut p = self.pending_refs.borrow_mut();
        p.0.retain(|(a, _)| *a != raw);
    }

    /// `invalidatePendingSetRef(rawRef)`: a pending set of this ref does not run any more.
    fn invalidate_pending(&self, raw: Value) -> Res<()> {
        let Value::Object(r) = raw else { return Ok(()) };
        let job = {
            let p = self.pending_refs.borrow();
            p.0.iter().find(|(a, _)| *a == r).map(|(_, j)| *j)
        };
        if let Some(j) = job {
            let meta = job_meta(self.rt(), j);
            meta.flags.set(meta.flags.get() | DISPOSED);
            self.forget_pending(r);
        }
        Ok(())
    }

    /// `setRef(rawRef, oldRawRef, parentSuspense, vnode, isUnmount)`. `parent` is the component that patches `vnode`.
    pub(crate) fn set_ref(&self, raw: &Value, old: Option<&Value>, vnode: ObjRef, is_unmount: bool, parent: Option<ObjRef>) -> Res<()> {
        let rt = self.rt();
        if rt.is_array(raw) {
            let n = crate::normalize::length_of(rt, raw)? as usize;
            for i in 0..n {
                let r = crate::normalize::array_get(rt, raw, i)?;
                let old_i = match old {
                    Some(o) if rt.is_array(o) => crate::normalize::array_get(rt, o, i)?,
                    Some(o) => o.clone(),
                    None => Value::Undefined,
                };
                let old_arg = if old.is_some() { Some(&old_i) } else { None };
                self.set_ref(&r, old_arg, vnode, is_unmount, parent)?;
            }
            return Ok(());
        }
        let shape = shape_of(rt, vnode);
        let ref_value = if is_unmount {
            Value::Null
        } else if shape & ShapeFlags::STATEFUL_COMPONENT != 0 {
            let comp = component_of(rt, vnode);
            match comp {
                Value::Object(i) => crate::public_instance::public_instance_of(rt, Some(i))?,
                _ => Value::Null,
            }
        } else {
            match el_of(rt, vnode) {
                Some(n) => self.ctx.handle(n),
                None => Value::Null,
            }
        };
        let value = if is_unmount { Value::Null } else { ref_value.clone() };
        let rr = raw_ref(rt, raw, parent)?;
        let refs = owner_refs(rt, &rr.owner)?;
        let setup = setup_state_of(rt, &rr.owner)?;
        let old_ref = match old {
            Some(o) => rt.get_str(o, "r")?,
            None => Value::Null,
        };
        let new_ref = rr.r.clone();
        if !old_ref.is_nullish() && !vn_value::strict_eq(&old_ref, &new_ref) {
            if let Some(o) = old {
                self.invalidate_pending(o.clone())?;
            }
            if is_string(&old_ref) {
                let k = str_of(rt, &old_ref)?;
                put(rt, &refs, &k, Value::Null)?;
                if can_set_setup_ref(rt, &refs, &setup, &k)? {
                    put(rt, &setup, &k, Value::Null)?;
                }
            } else if is_ref(rt, &old_ref)? {
                let old_k = match old {
                    Some(o) => rt.get_str(o, "k")?,
                    None => Value::Null,
                };
                if can_set_ref(rt, &refs, &old_k)? {
                    rt.set_str(&old_ref, "value", Value::Null)?;
                }
                if old_k.to_boolean() {
                    put(rt, &refs, &str_of(rt, &old_k)?, Value::Null)?;
                }
            }
        }
        if rt.is_callable(&new_ref) {
            call_with_error_handling(rt, &new_ref, match rr.owner {
                Value::Object(o) => Some(o),
                _ => None,
            }, ErrType::Code(12), Some(&[value, refs]))?;
        } else if is_string(&new_ref) || is_ref(rt, &new_ref)? {
            if value.to_boolean() {
                let job = rt.new_native_function(set_ref_job_code, "", 0, vec![
                    raw.clone(),
                    ref_value,
                    rr.owner.clone(),
                    parent.map_or(Value::Null, Value::Object),
                ]);
                let meta = job_meta(rt, job);
                meta.id.set(Some(-1.0));
                if let Value::Object(r) = raw {
                    self.invalidate_pending(raw.clone())?;
                    self.pending_refs.borrow_mut().0.push((*r, job));
                }
                queue_post_render_effect(rt, &Value::Object(job), &Value::Null)?;
            } else {
                self.invalidate_pending(raw.clone())?;
                do_set(rt, raw, &value, is_unmount, parent)?;
            }
        }
        Ok(())
    }
}

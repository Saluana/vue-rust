//! The Proxy handlers of @vue/reactivity 3.5.43 as exotic-object hooks. `BaseHandler` is `MutableReactiveHandler` (and its
//! shallow variant) and `ReadonlyReactiveHandler`; `CollectionHandler` is the Map/Set handler, which instruments `get` only.
//! A hook Vue does not define is forwarded to the target, as a JS Proxy without that trap would do. Behaviour per
//! @vue/reactivity 3.5.43 (production build).

use vn_value::{Exotic, Hook, JsStr, ObjRef, Prop, PropKey, Res, Rt, Tracer, Value, WellKnown};

use crate::array_inst;
use crate::reactive::{cached, reactive_value, readonly_value, slot, tables};
use crate::shared::{has_changed, is_integer_key, is_object, is_readonly, is_ref, is_shallow, key_value, to_raw};
use crate::state::state;
use crate::target::{iterate_keys, track, trigger, TriggerOp};

/// `key === name` for a string key.
pub(crate) fn key_is(key: &PropKey, name: &str) -> bool {
    matches!(key, PropKey::Str(s) if *s == JsStr::from_str(name))
}

/// `track(target, "...", key)` for a target that may be a primitive: a non-object target throws while tracking (the
/// WeakMap key of Vue's `track`) and is ignored otherwise.
pub(crate) fn track_on(rt: &Rt, target: &Value, key: &Value) -> Res<()> {
    match target {
        Value::Object(o) => track(rt, *o, key),
        _ => {
            let st = state(rt);
            let active = st.should_track.get() && st.active_sub.get().is_some();
            drop(st);
            if active {
                Err(rt.throw_type_error("Invalid value used as weak map key"))
            } else {
                Ok(())
            }
        }
    }
}

/// `trigger(target, ...)`: a non-object target has no dependencies, so only the global version moves.
pub(crate) fn trigger_on(rt: &Rt, target: &Value, op: TriggerOp, key: &Value, new_value: &Value) -> Res<()> {
    match target {
        Value::Object(o) => trigger(rt, *o, op, key, new_value),
        _ => {
            let st = state(rt);
            st.global_version.set(st.global_version.get() + 1);
            Ok(())
        }
    }
}

/// `builtInSymbols.has(key)` of @vue/reactivity, over the well-known symbols of this kernel.
pub(crate) fn is_builtin_symbol(rt: &Rt, key: &PropKey) -> bool {
    let PropKey::Sym(s) = key else {
        return false;
    };
    for w in [WellKnown::Iterator, WellKnown::AsyncIterator, WellKnown::ToPrimitive, WellKnown::ToStringTag, WellKnown::HasInstance] {
        if rt.well_known(w) == *s {
            return true;
        }
    }
    false
}

/// Reads that are not tracked: built-in symbols, `__proto__`, `__v_isRef` and `__isVue`.
fn is_untracked_read(rt: &Rt, key: &PropKey) -> bool {
    match key {
        PropKey::Sym(_) => is_builtin_symbol(rt, key),
        _ => key_is(key, "__proto__") || key_is(key, "__v_isRef") || key_is(key, "__isVue"),
    }
}

/// `Number(key)` for an integer key.
fn number_of_key(rt: &Rt, key: &PropKey) -> Res<f64> {
    match key {
        PropKey::Index(i) => Ok(*i as f64),
        other => rt.to_number(&key_value(other)),
    }
}

/// The handler of a mutable, shallow or readonly object proxy over `target`.
pub(crate) struct BaseHandler {
    pub(crate) target: ObjRef,
    pub(crate) readonly: bool,
    pub(crate) shallow: bool,
    pub(crate) is_arr: bool,
}

impl BaseHandler {
    fn get_impl(&self, rt: &Rt, key: &PropKey, receiver: &Value) -> Res<Value> {
        let target = Value::Object(self.target);
        if key_is(key, "__v_skip") {
            return rt.get(self.target, key, &target);
        }
        if key_is(key, "__v_isReactive") {
            return Ok(Value::Bool(!self.readonly));
        }
        if key_is(key, "__v_isReadonly") {
            return Ok(Value::Bool(self.readonly));
        }
        if key_is(key, "__v_isShallow") {
            return Ok(Value::Bool(self.shallow));
        }
        if key_is(key, "__v_raw") {
            let by_proxy = matches!(receiver, Value::Object(r) if Some(*r) == cached(rt, self.target, slot(self.readonly, self.shallow)));
            let by_proto = match receiver {
                Value::Object(r) => rt.get_prototype_of(*r) == rt.get_prototype_of(self.target),
                _ => false,
            };
            return Ok(if by_proxy || by_proto { target } else { Value::Undefined });
        }
        let target_is_array = rt.is_array(&target);
        if !self.readonly {
            if target_is_array {
                if let Some(f) = array_inst::instrumentation(rt, key) {
                    return Ok(f);
                }
            }
            if key_is(key, "hasOwnProperty") {
                return Ok(Value::Object(tables(rt).has_own));
            }
        }
        let recv = if is_ref(rt, &target)? { target.clone() } else { receiver.clone() };
        let res = rt.get(self.target, key, &recv)?;
        if is_untracked_read(rt, key) {
            return Ok(res);
        }
        if !self.readonly {
            track(rt, self.target, &key_value(key))?;
        }
        if self.shallow {
            return Ok(res);
        }
        if is_ref(rt, &res)? {
            let value = if target_is_array && is_integer_key(rt, &key_value(key)) {
                res
            } else {
                rt.get_str(&res, "value")?
            };
            return if self.readonly && is_object(rt, &value) { readonly_value(rt, &value) } else { Ok(value) };
        }
        if is_object(rt, &res) {
            return if self.readonly { readonly_value(rt, &res) } else { reactive_value(rt, &res) };
        }
        Ok(res)
    }

    fn set_impl(&self, rt: &Rt, key: &PropKey, value: &Value, receiver: &Value) -> Res<bool> {
        if self.readonly {
            return Ok(true);
        }
        let target = Value::Object(self.target);
        let mut old = rt.get(self.target, key, &target)?;
        let is_arr_int = rt.is_array(&target) && is_integer_key(rt, &key_value(key));
        let mut value = value.clone();
        if !self.shallow {
            let old_readonly = is_readonly(rt, &old)?;
            if !is_shallow(rt, &value)? && !is_readonly(rt, &value)? {
                old = to_raw(rt, &old)?;
                value = to_raw(rt, &value)?;
            }
            if !is_arr_int && is_ref(rt, &old)? && !is_ref(rt, &value)? {
                if old_readonly {
                    return Ok(true);
                }
                rt.set_v_strict(&old, &PropKey::lit("value"), value)?;
                return Ok(true);
            }
        }
        let had_key = if is_arr_int {
            number_of_key(rt, key)? < rt.to_number(&rt.get_str(&target, "length")?)?
        } else {
            rt.has_own_property(self.target, key)?
        };
        let recv = if is_ref(rt, &target)? { target.clone() } else { receiver.clone() };
        let result = rt.set(self.target, key, value.clone(), &recv)?;
        let raw_receiver_is_target = matches!(to_raw(rt, receiver)?, Value::Object(r) if r == self.target);
        if raw_receiver_is_target && result {
            if !had_key {
                trigger(rt, self.target, TriggerOp::Add, &key_value(key), &value)?;
            } else if has_changed(&value, &old) {
                trigger(rt, self.target, TriggerOp::Set, &key_value(key), &value)?;
            }
        }
        Ok(result)
    }

    fn delete_impl(&self, rt: &Rt, key: &PropKey) -> Res<bool> {
        if self.readonly {
            return Ok(true);
        }
        let had_key = rt.has_own_property(self.target, key)?;
        rt.get(self.target, key, &Value::Object(self.target))?;
        let result = rt.delete(self.target, key)?;
        if result && had_key {
            trigger(rt, self.target, TriggerOp::Delete, &key_value(key), &Value::Undefined)?;
        }
        Ok(result)
    }

    fn has_impl(&self, rt: &Rt, key: &PropKey) -> Res<bool> {
        let result = rt.has_property(self.target, key)?;
        if !self.readonly && !is_builtin_symbol(rt, key) {
            track(rt, self.target, &key_value(key))?;
        }
        Ok(result)
    }

    fn own_keys_impl(&self, rt: &Rt) -> Res<Vec<PropKey>> {
        if !self.readonly {
            let target = Value::Object(self.target);
            let k = if rt.is_array(&target) { Value::String(JsStr::from_str("length")) } else { iterate_keys(rt)[0].clone() };
            track(rt, self.target, &k)?;
        }
        rt.own_keys(self.target)
    }
}

impl Exotic for BaseHandler {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.target);
    }

    fn get(&self, rt: &Rt, _obj: ObjRef, key: &PropKey, receiver: &Value) -> Res<Hook<Value>> {
        Ok(Hook::Handled(self.get_impl(rt, key, receiver)?))
    }

    fn set(&self, rt: &Rt, _obj: ObjRef, key: &PropKey, v: &Value, receiver: &Value) -> Res<Hook<bool>> {
        Ok(Hook::Handled(self.set_impl(rt, key, v, receiver)?))
    }

    fn has(&self, rt: &Rt, _obj: ObjRef, key: &PropKey) -> Res<Hook<bool>> {
        Ok(Hook::Handled(self.has_impl(rt, key)?))
    }

    fn delete(&self, rt: &Rt, _obj: ObjRef, key: &PropKey) -> Res<Hook<bool>> {
        Ok(Hook::Handled(self.delete_impl(rt, key)?))
    }

    fn own_keys(&self, rt: &Rt, _obj: ObjRef) -> Res<Hook<Vec<PropKey>>> {
        Ok(Hook::Handled(self.own_keys_impl(rt)?))
    }

    fn get_own_property(&self, rt: &Rt, _obj: ObjRef, key: &PropKey) -> Res<Hook<Option<Prop>>> {
        Ok(Hook::Handled(rt.get_own_property(self.target, key)?))
    }

    fn define(&self, rt: &Rt, _obj: ObjRef, key: &PropKey, prop: &Prop) -> Res<Hook<bool>> {
        Ok(Hook::Handled(rt.define_own(self.target, key, prop.clone())?))
    }

    fn prevent_extensions(&self, rt: &Rt, _obj: ObjRef) -> Res<Hook<bool>> {
        Ok(Hook::Handled(rt.prevent_extensions(self.target)?))
    }

    fn is_extensible(&self, rt: &Rt, _obj: ObjRef) -> Res<Hook<bool>> {
        Ok(Hook::Handled(rt.is_extensible(self.target)?))
    }

    fn is_array(&self) -> bool {
        self.is_arr
    }
}

/// The handler of a Map, Set (or Weak) collection proxy: `get` reads the instrumentation tables, every other operation
/// goes to the target.
pub(crate) struct CollectionHandler {
    pub(crate) target: ObjRef,
    pub(crate) readonly: bool,
    pub(crate) shallow: bool,
}

impl Exotic for CollectionHandler {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.target);
    }

    fn get(&self, rt: &Rt, _obj: ObjRef, key: &PropKey, receiver: &Value) -> Res<Hook<Value>> {
        if key_is(key, "__v_isReactive") {
            return Ok(Hook::Handled(Value::Bool(!self.readonly)));
        }
        if key_is(key, "__v_isReadonly") {
            return Ok(Hook::Handled(Value::Bool(self.readonly)));
        }
        if key_is(key, "__v_raw") {
            return Ok(Hook::Handled(Value::Object(self.target)));
        }
        let instr = tables(rt).coll[slot(self.readonly, self.shallow)];
        let from_instr = rt.has_own_property(instr, key)? && rt.has_property(self.target, key)?;
        let src = if from_instr { instr } else { self.target };
        Ok(Hook::Handled(rt.get(src, key, receiver)?))
    }

    fn prevent_extensions(&self, rt: &Rt, _obj: ObjRef) -> Res<Hook<bool>> {
        Ok(Hook::Handled(rt.prevent_extensions(self.target)?))
    }

    fn is_extensible(&self, rt: &Rt, _obj: ObjRef) -> Res<Hook<bool>> {
        Ok(Hook::Handled(rt.is_extensible(self.target)?))
    }

    fn set(&self, rt: &Rt, _obj: ObjRef, key: &PropKey, v: &Value, receiver: &Value) -> Res<Hook<bool>> {
        Ok(Hook::Handled(rt.set(self.target, key, v.clone(), receiver)?))
    }

    fn has(&self, rt: &Rt, _obj: ObjRef, key: &PropKey) -> Res<Hook<bool>> {
        Ok(Hook::Handled(rt.has_property(self.target, key)?))
    }

    fn delete(&self, rt: &Rt, _obj: ObjRef, key: &PropKey) -> Res<Hook<bool>> {
        Ok(Hook::Handled(rt.delete(self.target, key)?))
    }

    fn own_keys(&self, rt: &Rt, _obj: ObjRef) -> Res<Hook<Vec<PropKey>>> {
        Ok(Hook::Handled(rt.own_keys(self.target)?))
    }

    fn get_own_property(&self, rt: &Rt, _obj: ObjRef, key: &PropKey) -> Res<Hook<Option<Prop>>> {
        Ok(Hook::Handled(rt.get_own_property(self.target, key)?))
    }

    fn define(&self, rt: &Rt, _obj: ObjRef, key: &PropKey, prop: &Prop) -> Res<Hook<bool>> {
        Ok(Hook::Handled(rt.define_own(self.target, key, prop.clone())?))
    }
}

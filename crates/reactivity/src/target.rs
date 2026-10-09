//! The targetMap of @vue/reactivity 3.5.43: `track`, `trigger` and `getDepFromReactive`. A target's depsMap lives in the
//! target's extension slot (ABI.md section 9), so no weak table is needed; that slot is reserved for this map. Keys
//! follow SameValueZero and entries keep insertion order, which is the order of `depsMap.forEach` and hence of
//! notification.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use vn_value::{JsStr, NativeObject, ObjRef, Res, Rt, Sym, Tracer, Value};

use crate::graph::{dep_track, dep_trigger, end_batch, new_dep, start_batch};
use crate::shared::{has_tag, is_integer_key};
use crate::state::state;

/// A depsMap key under SameValueZero (NaN equals NaN, +0 equals -0).
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) enum Key {
    Undef,
    Null,
    Bool(bool),
    Num(u64),
    Str(JsStr),
    Sym(Sym),
    Obj(ObjRef),
}

pub(crate) fn key_of(v: &Value) -> Key {
    match v {
        Value::Undefined | Value::Uninit | Value::Hole => Key::Undef,
        Value::Null => Key::Null,
        Value::Bool(b) => Key::Bool(*b),
        Value::Number(n) => Key::Num(if n.is_nan() {
            f64::NAN.to_bits()
        } else if *n == 0.0 {
            0
        } else {
            n.to_bits()
        }),
        Value::String(s) => Key::Str(s.clone()),
        Value::Symbol(s) => Key::Sym(*s),
        Value::Object(o) => Key::Obj(*o),
    }
}

pub(crate) struct Entry {
    pub(crate) seq: u64,
    pub(crate) key: Value,
    pub(crate) dep: ObjRef,
}

/// The depsMap of one target: key -> Dep, in insertion order.
#[derive(Default)]
pub(crate) struct TargetMap {
    entries: RefCell<HashMap<Key, Entry>>,
    seq: Cell<u64>,
}

impl TargetMap {
    pub(crate) fn get(&self, key: &Value) -> Option<ObjRef> {
        self.entries.borrow().get(&key_of(key)).map(|e| e.dep)
    }

    pub(crate) fn has(&self, key: &Value) -> bool {
        self.entries.borrow().contains_key(&key_of(key))
    }

    /// Adds a key that is not present yet (`Map.prototype.set` of a new key).
    pub(crate) fn insert(&self, key: &Value, dep: ObjRef) {
        let seq = self.seq.get();
        self.seq.set(seq + 1);
        self.entries.borrow_mut().insert(key_of(key), Entry { seq, key: key.clone(), dep });
    }

    /// `Map.prototype.delete` by an already normalised key.
    pub(crate) fn remove(&self, k: &Key) {
        self.entries.borrow_mut().remove(k);
    }

    /// Entries in insertion order (the order of `Map.prototype.forEach`).
    pub(crate) fn in_order(&self) -> Vec<(Value, ObjRef)> {
        let mut v: Vec<(u64, Value, ObjRef)> =
            self.entries.borrow().values().map(|e| (e.seq, e.key.clone(), e.dep)).collect();
        v.sort_by_key(|x| x.0);
        v.into_iter().map(|(_, k, d)| (k, d)).collect()
    }
}

impl NativeObject for TargetMap {
    fn trace(&self, t: &mut Tracer) {
        for e in self.entries.borrow().values() {
            t.mark(e.dep);
            t.mark_value(&e.key);
        }
    }
}

/// The depsMap of a target, if it has one.
pub(crate) fn target_map(rt: &Rt, target: ObjRef) -> Option<Rc<TargetMap>> {
    let ext = rt.ext(target)?;
    let any: Rc<dyn Any> = ext;
    match any.downcast::<TargetMap>() {
        Ok(m) => Some(m),
        Err(_) => panic!("the extension slot of a reactive target is reserved for the targetMap (framework bug)"),
    }
}

fn target_map_or_create(rt: &Rt, target: ObjRef) -> Rc<TargetMap> {
    if let Some(m) = target_map(rt, target) {
        return m;
    }
    let m = Rc::new(TargetMap::default());
    let payload: Rc<dyn NativeObject> = m.clone();
    rt.set_ext(target, payload);
    m
}

/// `getDepFromReactive(target, key)`: the dep of a key, if one exists.
pub(crate) fn dep_of(rt: &Rt, target: &Value, key: &Value) -> Option<ObjRef> {
    match target {
        Value::Object(o) => target_map(rt, *o)?.get(key),
        _ => None,
    }
}

/// TrackOpTypes are not kept: `track` ignores its type in the production build.
pub fn track(rt: &Rt, target: ObjRef, key: &Value) -> Res<()> {
    let st = state(rt);
    if !st.should_track.get() || st.active_sub.get().is_none() {
        return Ok(());
    }
    drop(st);
    let map = target_map_or_create(rt, target);
    let dep = match map.get(key) {
        Some(d) => d,
        None => {
            let d = new_dep(rt, Some((map.clone(), key_of(key))), None);
            map.insert(key, d);
            d
        }
    };
    dep_track(rt, dep);
    Ok(())
}

/// TriggerOpTypes. `Other` is an unknown type string: it runs no operation-specific triggers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TriggerOp {
    Set,
    Add,
    Delete,
    Clear,
    Other,
}

/// `trigger(target, type, key, newValue)` of @vue/reactivity 3.5.43. `oldValue` and `oldTarget` only feed the
/// development debugger hooks, so the production algorithm does not take them.
pub fn trigger(rt: &Rt, target: ObjRef, op: TriggerOp, key: &Value, new_value: &Value) -> Res<()> {
    let Some(map) = target_map(rt, target) else {
        let st = state(rt);
        st.global_version.set(st.global_version.get() + 1);
        return Ok(());
    };
    start_batch(rt);
    let r = trigger_deps(rt, target, &map, op, key, new_value);
    let e = end_batch(rt);
    r?;
    e
}

fn is_str(v: &Value, s: &str) -> bool {
    matches!(v, Value::String(x) if *x == JsStr::from_str(s))
}

fn trigger_deps(
    rt: &Rt,
    target: ObjRef,
    map: &TargetMap,
    op: TriggerOp,
    key: &Value,
    new_value: &Value,
) -> Res<()> {
    let syms = state(rt).syms;
    let run = |dep: Option<ObjRef>| -> Res<()> {
        match dep {
            Some(d) => dep_trigger(rt, d),
            None => Ok(()),
        }
    };
    if op == TriggerOp::Clear {
        for (_, d) in map.in_order() {
            dep_trigger(rt, d)?;
        }
        return Ok(());
    }
    let target_v = Value::Object(target);
    let target_is_array = rt.is_array(&target_v);
    let is_index = target_is_array && is_integer_key(rt, key);
    if target_is_array && is_str(key, "length") {
        let new_length = rt.to_number(new_value)?;
        for (k, d) in map.in_order() {
            let hit = is_str(&k, "length")
                || matches!(&k, Value::Symbol(s) if *s == syms.array_iterate)
                || match &k {
                    Value::Symbol(_) => false,
                    other => rt.to_number(other)? >= new_length,
                };
            if hit {
                dep_trigger(rt, d)?;
            }
        }
        return Ok(());
    }
    if !key.is_undefined() || map.has(&Value::Undefined) {
        run(map.get(key))?;
    }
    if is_index {
        run(map.get(&Value::Symbol(syms.array_iterate)))?;
    }
    match op {
        TriggerOp::Add => {
            if !target_is_array {
                run(map.get(&Value::Symbol(syms.iterate)))?;
                if has_tag(rt, &target_v, "Map")? {
                    run(map.get(&Value::Symbol(syms.map_iterate)))?;
                }
            } else if is_index {
                run(map.get(&Value::String(JsStr::from_str("length"))))?;
            }
        }
        TriggerOp::Delete => {
            if !target_is_array {
                run(map.get(&Value::Symbol(syms.iterate)))?;
                if has_tag(rt, &target_v, "Map")? {
                    run(map.get(&Value::Symbol(syms.map_iterate)))?;
                }
            }
        }
        TriggerOp::Set => {
            if has_tag(rt, &target_v, "Map")? {
                run(map.get(&Value::Symbol(syms.iterate)))?;
            }
        }
        TriggerOp::Clear | TriggerOp::Other => {}
    }
    Ok(())
}

/// The internal keys of the target map (`ITERATE_KEY`, `MAP_KEY_ITERATE_KEY`, `ARRAY_ITERATE_KEY`).
pub fn iterate_keys(rt: &Rt) -> [Value; 3] {
    let s = state(rt).syms;
    [Value::Symbol(s.iterate), Value::Symbol(s.map_iterate), Value::Symbol(s.array_iterate)]
}

//! Reactive proxies of @vue/reactivity 3.5.43: `reactive`, `shallowReactive`, `readonly`, `shallowReadonly`, `toRaw`,
//! `markRaw`, the flag predicates and the internal `toReactive`/`toReadonly`. A proxy is an exotic object whose hooks
//! live in `handlers.rs`. The four raw -> proxy caches (Vue's reactiveMap, shallowReactiveMap, readonlyMap and
//! shallowReadonlyMap) sit in the raw object's ext slot. Behaviour per @vue/reactivity 3.5.43 (production build).

use std::rc::Rc;

use vn_value::{payload_ref, Call, CodeFn, Exotic, Flags, NativeObject, ObjRef, PropKey, Res, Rt, Tracer, Value};

use crate::array_inst;
use crate::collection_inst;
use crate::handlers::{BaseHandler, CollectionHandler};
use crate::shared::{has_tag, is_object, is_proxy, is_readonly, is_reactive, is_shallow, native, to_raw};
use crate::state::{set_reactive_hooks, ReactiveHooks};

const INVALID: u8 = 0;
const COMMON: u8 = 1;
const COLLECTION: u8 = 2;

/// Cache slot of a mode: 0 reactive, 1 shallowReactive, 2 readonly, 3 shallowReadonly (the order of Vue's four maps).
pub(crate) fn slot(readonly: bool, shallow: bool) -> usize {
    (readonly as usize) * 2 + shallow as usize
}

/// The cached reactive proxy of `target` for a mode (kept in the object's extension payload, see `target::ObjExt`).
pub(crate) fn cached(rt: &Rt, target: ObjRef, slot: usize) -> Option<ObjRef> {
    crate::target::cached_proxy(rt, target, slot)
}

fn cache_put(rt: &Rt, target: ObjRef, slot: usize, proxy: ObjRef) -> Res<()> {
    crate::target::set_cached_proxy(rt, target, slot, proxy).map_err(|_| {
        rt.throw_type_error("reactive: the object's extension slot is owned by another subsystem (unsupported)")
    })
}

/// `targetTypeMap(toRawType(value))`: 1 for Object and Array, 2 for Map, Set, WeakMap and WeakSet, 0 otherwise.
fn target_type(rt: &Rt, v: &Value) -> Res<u8> {
    for tag in ["Object", "Array"] {
        if has_tag(rt, v, tag)? {
            return Ok(COMMON);
        }
    }
    for tag in ["Map", "Set", "WeakMap", "WeakSet"] {
        if has_tag(rt, v, tag)? {
            return Ok(COLLECTION);
        }
    }
    Ok(INVALID)
}

/// `createReactiveObject`: the proxy of `target` in the given mode, or `target` itself when it cannot be proxied.
pub(crate) fn create_reactive_object(rt: &Rt, target: &Value, readonly: bool, shallow: bool) -> Res<Value> {
    if !is_object(rt, target) {
        return Ok(target.clone());
    }
    let t = target.as_object().expect("object value (framework bug)");
    if rt.get_str(target, "__v_raw")?.to_boolean()
        && !(readonly && rt.get_str(target, "__v_isReactive")?.to_boolean())
    {
        return Ok(target.clone());
    }
    if rt.get_str(target, "__v_skip")?.to_boolean() || !rt.is_extensible(t)? {
        return Ok(target.clone());
    }
    let s = slot(readonly, shallow);
    if let Some(p) = cached(rt, t, s) {
        return Ok(Value::Object(p));
    }
    let ty = target_type(rt, target)?;
    if ty == INVALID {
        return Ok(target.clone());
    }
    let hooks: Rc<dyn Exotic> = if ty == COLLECTION {
        Rc::new(CollectionHandler { target: t, readonly, shallow })
    } else {
        Rc::new(BaseHandler { target: t, readonly, shallow, is_arr: rt.is_array(target) })
    };
    let proto = rt.get_prototype_of(t);
    let proxy = rt.new_exotic_object(proto, hooks);
    cache_put(rt, t, s, proxy)?;
    Ok(Value::Object(proxy))
}

/// `reactive(target)`: a readonly proxy is returned as it is.
pub(crate) fn reactive_value(rt: &Rt, v: &Value) -> Res<Value> {
    if is_readonly(rt, v)? {
        return Ok(v.clone());
    }
    create_reactive_object(rt, v, false, false)
}

/// `readonly(target)`.
pub(crate) fn readonly_value(rt: &Rt, v: &Value) -> Res<Value> {
    create_reactive_object(rt, v, true, false)
}

/// `toReactive(value)` / `toReadonly(value)`: the wrapper of an object, the value itself otherwise.
pub(crate) fn to_reactive_value(rt: &Rt, v: &Value) -> Res<Value> {
    if is_object(rt, v) {
        reactive_value(rt, v)
    } else {
        Ok(v.clone())
    }
}

pub(crate) fn to_readonly_value(rt: &Rt, v: &Value) -> Res<Value> {
    if is_object(rt, v) {
        readonly_value(rt, v)
    } else {
        Ok(v.clone())
    }
}

/// `markRaw(value)`: defines the non-enumerable `__v_skip` flag on an extensible object.
fn mark_raw(rt: &Rt, v: &Value) -> Res<Value> {
    let o = match v {
        Value::Object(o) => *o,
        Value::Undefined | Value::Null => {
            return Err(rt.throw_type_error("Cannot convert undefined or null to object"));
        }
        _ => return Ok(v.clone()),
    };
    let key = PropKey::from_str("__v_skip");
    if !rt.has_own_property(o, &key)? && rt.is_extensible(o)? {
        rt.define_data(o, key, Value::Bool(true), Flags::new(false, false, true))?;
    }
    Ok(v.clone())
}

// ----- per-realm tables -----

/// The per-realm tables the proxies read: the array instrumentations, the four collection instrumentation tables (slot
/// order) and the `hasOwnProperty` function. They are reachable from a registered builtin, so the collector keeps them.
#[derive(Clone, Copy)]
pub(crate) struct Tables {
    pub(crate) arr: ObjRef,
    pub(crate) coll: [ObjRef; 4],
    pub(crate) has_own: ObjRef,
}

struct RealmTables(Tables);

impl NativeObject for RealmTables {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.0.arr);
        t.mark(self.0.has_own);
        for c in self.0.coll {
            t.mark(c);
        }
    }
}

const TABLES: &str = "vn-reactivity/r1b";

pub(crate) fn tables(rt: &Rt) -> Tables {
    let obj = match rt.builtin(TABLES) {
        Some(o) => o,
        None => {
            let t = Tables {
                arr: array_inst::build(rt),
                coll: [0usize, 1, 2, 3].map(|s| collection_inst::build(rt, s >> 1 == 1, s & 1 == 1)),
                has_own: collection_inst::has_own_function(rt),
            };
            let payload: Rc<dyn NativeObject> = Rc::new(RealmTables(t));
            let o = rt.new_native_object(None, payload);
            rt.register_builtin(TABLES, o);
            o
        }
    };
    let payload = rt.native_payload(obj).expect("reactivity tables (framework bug)");
    payload_ref::<RealmTables>(&*payload).expect("reactivity tables (framework bug)").0
}

/// Installs the realm's `toReactive`/`toReadonly` hooks and builds its tables. Called from `api::install_protos`.
pub(crate) fn install(rt: &Rt) {
    set_reactive_hooks(rt, ReactiveHooks { to_reactive: to_reactive_value, to_readonly: to_readonly_value });
    tables(rt);
}

// ----- JS surface -----

fn export(rt: &Rt, name: &'static str, len: u32, code: CodeFn) -> (&'static str, Value) {
    (name, Value::Object(native(rt, name, len, code, Vec::new())))
}

/// The `vue` exports this module owns (`reactive`, `shallowReactive`, `readonly`, `shallowReadonly`, `toRaw`, `markRaw`,
/// `isReactive`, `isReadonly`, `isProxy`, `isShallow`).
pub(crate) fn exported(rt: &Rt) -> Vec<(&'static str, Value)> {
    vec![
        export(rt, "reactive", 1, js_reactive),
        export(rt, "shallowReactive", 1, js_shallow_reactive),
        export(rt, "readonly", 1, js_readonly),
        export(rt, "shallowReadonly", 1, js_shallow_readonly),
        export(rt, "toRaw", 1, js_to_raw),
        export(rt, "markRaw", 1, js_mark_raw),
        export(rt, "isReactive", 1, js_is_reactive),
        export(rt, "isReadonly", 1, js_is_readonly),
        export(rt, "isProxy", 1, js_is_proxy),
        export(rt, "isShallow", 1, js_is_shallow),
    ]
}

/// The internal helpers (tests and the runtime): `toReactive` and `toReadonly`.
pub(crate) fn internal(rt: &Rt) -> Vec<(&'static str, Value)> {
    vec![export(rt, "toReactive", 1, js_to_reactive), export(rt, "toReadonly", 1, js_to_readonly)]
}

fn js_reactive(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    reactive_value(rt, &c.arg(0))
}

fn js_shallow_reactive(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    create_reactive_object(rt, &c.arg(0), false, true)
}

fn js_readonly(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    create_reactive_object(rt, &c.arg(0), true, false)
}

fn js_shallow_readonly(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    create_reactive_object(rt, &c.arg(0), true, true)
}

fn js_to_raw(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    to_raw(rt, &c.arg(0))
}

fn js_mark_raw(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    mark_raw(rt, &c.arg(0))
}

fn js_is_reactive(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(is_reactive(rt, &c.arg(0))?))
}

fn js_is_readonly(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(is_readonly(rt, &c.arg(0))?))
}

fn js_is_shallow(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(is_shallow(rt, &c.arg(0))?))
}

fn js_is_proxy(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(is_proxy(rt, &c.arg(0))?))
}

fn js_to_reactive(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    to_reactive_value(rt, &c.arg(0))
}

fn js_to_readonly(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    to_readonly_value(rt, &c.arg(0))
}

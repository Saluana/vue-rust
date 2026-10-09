//! Map and Set instrumentation of @vue/reactivity 3.5.43: the four instrumentation tables read by the collection handler's
//! `get` hook (mutable, shallow, readonly and shallow readonly), the iterators they return, and the shared
//! `hasOwnProperty` function. Behaviour per @vue/reactivity 3.5.43 (production build).

use vn_value::{strict_eq, Call, Flags, ObjRef, PropKey, Res, Rt, Value, WellKnown};

use crate::handlers::{track_on, trigger_on};
use crate::reactive::{to_readonly_value, to_reactive_value};
use crate::shared::{has_changed, has_tag, is_readonly, is_shallow, native, to_raw};
use crate::target::{iterate_keys, TriggerOp};

/// The instrumented methods (`size` is an accessor, added in `build`). `@@iterator` is `Symbol.iterator`.
const METHODS: &[(&str, u32)] = &[
    ("get", 1),
    ("has", 1),
    ("add", 1),
    ("set", 2),
    ("delete", 1),
    ("clear", 0),
    ("forEach", 1),
    ("keys", 0),
    ("values", 0),
    ("entries", 0),
    ("@@iterator", 0),
];

/// `createInstrumentations(readonly, shallow)`, with the functions carrying their method index and mode as data.
pub(crate) fn build(rt: &Rt, readonly: bool, shallow: bool) -> ObjRef {
    let obj = rt.new_object();
    for (idx, (name, len)) in METHODS.iter().enumerate() {
        let (key, fname) = if *name == "@@iterator" {
            (PropKey::Sym(rt.well_known(WellKnown::Iterator)), "[Symbol.iterator]")
        } else {
            (PropKey::from_str(name), *name)
        };
        let data = vec![Value::Number(idx as f64), Value::Bool(readonly), Value::Bool(shallow)];
        let f = native(rt, fname, *len, coll_dispatch, data);
        rt.define_data(obj, key, Value::Object(f), Flags::ALL).expect("collection instrumentation (framework bug)");
    }
    let getter = native(rt, "get size", 0, size_dispatch, vec![Value::Bool(readonly)]);
    rt.define_accessor(obj, PropKey::from_str("size"), Some(getter), None, Flags::ALL)
        .expect("collection instrumentation (framework bug)");
    obj
}

/// The shared `hasOwnProperty` of the mutable object proxies.
pub(crate) fn has_own_function(rt: &Rt) -> ObjRef {
    native(rt, "hasOwnProperty", 1, has_own_dispatch, Vec::new())
}

fn index_data(v: &Value) -> usize {
    match v {
        Value::Number(n) => *n as usize,
        _ => 0,
    }
}

/// How a value read through a collection proxy is wrapped (`toShallow`, `toReadonly` or `toReactive`).
#[derive(Clone, Copy)]
enum Wrap {
    Shallow,
    Readonly,
    Reactive,
}

fn wrap_of(readonly: bool, shallow: bool) -> Wrap {
    if shallow {
        Wrap::Shallow
    } else if readonly {
        Wrap::Readonly
    } else {
        Wrap::Reactive
    }
}

fn wrap(rt: &Rt, w: Wrap, v: &Value) -> Res<Value> {
    match w {
        Wrap::Shallow => Ok(v.clone()),
        Wrap::Readonly => to_readonly_value(rt, v),
        Wrap::Reactive => to_reactive_value(rt, v),
    }
}

/// `obj[name](...args)`.
fn call_named(rt: &Rt, obj: &Value, name: &str, args: &[Value]) -> Res<Value> {
    let f = rt.get_str(obj, name)?;
    rt.call(&f, obj, args)
}

/// `getProto(obj)`: `Reflect.getPrototypeOf`, which throws for a non-object.
fn proto_value(rt: &Rt, obj: &Value) -> Res<Value> {
    let Some(o) = obj.as_object() else {
        return Err(rt.throw_type_error("Reflect.getPrototypeOf called on non-object"));
    };
    Ok(match rt.get_prototype_of(o) {
        Some(p) => Value::Object(p),
        None => Value::Null,
    })
}

fn coll_dispatch(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let name = METHODS[index_data(&d[0])].0;
    let readonly = matches!(d[1], Value::Bool(true));
    let shallow = matches!(d[2], Value::Bool(true));
    let this = c.this;
    if readonly {
        match name {
            "delete" => return Ok(Value::Bool(false)),
            "clear" => return Ok(Value::Undefined),
            "add" | "set" => return Ok(this.clone()),
            _ => {}
        }
    }
    match name {
        "get" => coll_get(rt, this, &c.arg(0), readonly, shallow),
        "has" => coll_has(rt, this, &c.arg(0), readonly),
        "add" => coll_add(rt, this, &c.arg(0), shallow),
        "set" => coll_set(rt, this, &c.arg(0), &c.arg(1), shallow),
        "delete" => coll_delete(rt, this, &c.arg(0)),
        "clear" => coll_clear(rt, this),
        "forEach" => coll_for_each(rt, this, &c.arg(0), &c.arg(1), readonly, shallow),
        "keys" => coll_iterable(rt, this, Which::Keys, c.args, readonly, shallow),
        "values" => coll_iterable(rt, this, Which::Values, c.args, readonly, shallow),
        "entries" => coll_iterable(rt, this, Which::Entries, c.args, readonly, shallow),
        "@@iterator" => coll_iterable(rt, this, Which::Iterator, c.args, readonly, shallow),
        _ => Err(rt.throw_type_error("collection instrumentation (framework bug)")),
    }
}

fn size_dispatch(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let readonly = matches!(rt.fn_data(c.callee)[0], Value::Bool(true));
    coll_size(rt, c.this, readonly)
}

/// `get(key)`: tracks the key and its raw form, then reads the target through whichever of them is present.
fn coll_get(rt: &Rt, this: &Value, key: &Value, readonly: bool, shallow: bool) -> Res<Value> {
    let target = rt.get_str(this, "__v_raw")?;
    let raw_target = to_raw(rt, &target)?;
    let raw_key = to_raw(rt, key)?;
    if !readonly {
        if has_changed(key, &raw_key) {
            track_on(rt, &raw_target, key)?;
        }
        track_on(rt, &raw_target, &raw_key)?;
    }
    let has = proto_value(rt, &raw_target)?;
    let has = rt.get_str(&has, "has")?;
    let w = wrap_of(readonly, shallow);
    if rt.call(&has, &raw_target, &[key.clone()])?.to_boolean() {
        let v = call_named(rt, &target, "get", &[key.clone()])?;
        return wrap(rt, w, &v);
    }
    if rt.call(&has, &raw_target, &[raw_key.clone()])?.to_boolean() {
        let v = call_named(rt, &target, "get", &[raw_key])?;
        return wrap(rt, w, &v);
    }
    if !strict_eq(&target, &raw_target) {
        call_named(rt, &target, "get", &[key.clone()])?;
    }
    Ok(Value::Undefined)
}

/// `size`: tracks ITERATE_KEY on the raw collection, then reads the target's size.
fn coll_size(rt: &Rt, this: &Value, readonly: bool) -> Res<Value> {
    let target = rt.get_str(this, "__v_raw")?;
    if !readonly {
        let raw = to_raw(rt, &target)?;
        track_on(rt, &raw, &iterate_keys(rt)[0])?;
    }
    rt.get_str(&target, "size")
}

fn coll_has(rt: &Rt, this: &Value, key: &Value, readonly: bool) -> Res<Value> {
    let target = rt.get_str(this, "__v_raw")?;
    let raw_target = to_raw(rt, &target)?;
    let raw_key = to_raw(rt, key)?;
    if !readonly {
        if has_changed(key, &raw_key) {
            track_on(rt, &raw_target, key)?;
        }
        track_on(rt, &raw_target, &raw_key)?;
    }
    if strict_eq(key, &raw_key) {
        return call_named(rt, &target, "has", &[key.clone()]);
    }
    let found = call_named(rt, &target, "has", &[key.clone()])?;
    if found.to_boolean() {
        return Ok(found);
    }
    call_named(rt, &target, "has", &[raw_key])
}

fn coll_add(rt: &Rt, this: &Value, value: &Value, shallow: bool) -> Res<Value> {
    let target = to_raw(rt, this)?;
    let proto = proto_value(rt, &target)?;
    let has = rt.get_str(&proto, "has")?;
    let raw_value = to_raw(rt, value)?;
    let value_to_add = if !shallow && !is_shallow(rt, value)? && !is_readonly(rt, value)? {
        raw_value.clone()
    } else {
        value.clone()
    };
    let mut had = rt.call(&has, &target, &[value_to_add.clone()])?.to_boolean();
    if !had && has_changed(value, &value_to_add) {
        had = rt.call(&has, &target, &[value.clone()])?.to_boolean();
    }
    if !had && has_changed(&raw_value, &value_to_add) {
        had = rt.call(&has, &target, &[raw_value.clone()])?.to_boolean();
    }
    if !had {
        call_named(rt, &target, "add", &[value_to_add.clone()])?;
        trigger_on(rt, &target, TriggerOp::Add, &value_to_add, &value_to_add)?;
    }
    Ok(this.clone())
}

fn coll_set(rt: &Rt, this: &Value, key: &Value, value: &Value, shallow: bool) -> Res<Value> {
    let mut value = value.clone();
    if !shallow && !is_shallow(rt, &value)? && !is_readonly(rt, &value)? {
        value = to_raw(rt, &value)?;
    }
    let target = to_raw(rt, this)?;
    let proto = proto_value(rt, &target)?;
    let has = rt.get_str(&proto, "has")?;
    let get = rt.get_str(&proto, "get")?;
    let mut key = key.clone();
    let mut had = rt.call(&has, &target, &[key.clone()])?.to_boolean();
    if !had {
        key = to_raw(rt, &key)?;
        had = rt.call(&has, &target, &[key.clone()])?.to_boolean();
    }
    let old = rt.call(&get, &target, &[key.clone()])?;
    call_named(rt, &target, "set", &[key.clone(), value.clone()])?;
    if !had {
        trigger_on(rt, &target, TriggerOp::Add, &key, &value)?;
    } else if has_changed(&value, &old) {
        trigger_on(rt, &target, TriggerOp::Set, &key, &value)?;
    }
    Ok(this.clone())
}

fn coll_delete(rt: &Rt, this: &Value, key: &Value) -> Res<Value> {
    let target = to_raw(rt, this)?;
    let proto = proto_value(rt, &target)?;
    let has = rt.get_str(&proto, "has")?;
    let get = rt.get_str(&proto, "get")?;
    let mut key = key.clone();
    let mut had = rt.call(&has, &target, &[key.clone()])?.to_boolean();
    if !had {
        key = to_raw(rt, &key)?;
        had = rt.call(&has, &target, &[key.clone()])?.to_boolean();
    }
    if get.to_boolean() {
        rt.call(&get, &target, &[key.clone()])?;
    }
    let result = call_named(rt, &target, "delete", &[key.clone()])?;
    if had {
        trigger_on(rt, &target, TriggerOp::Delete, &key, &Value::Undefined)?;
    }
    Ok(result)
}

fn coll_clear(rt: &Rt, this: &Value) -> Res<Value> {
    let target = to_raw(rt, this)?;
    let had_items = !matches!(rt.get_str(&target, "size")?, Value::Number(n) if n == 0.0);
    let result = call_named(rt, &target, "clear", &[])?;
    if had_items {
        trigger_on(rt, &target, TriggerOp::Clear, &Value::Undefined, &Value::Undefined)?;
    }
    Ok(result)
}

/// `forEach(callback, thisArg)`: the callback receives the wrapped value and key, and the proxy as third argument.
fn coll_for_each(rt: &Rt, this: &Value, callback: &Value, this_arg: &Value, readonly: bool, shallow: bool) -> Res<Value> {
    let target = rt.get_str(this, "__v_raw")?;
    let raw_target = to_raw(rt, &target)?;
    if !readonly {
        track_on(rt, &raw_target, &iterate_keys(rt)[0])?;
    }
    let data = vec![
        callback.clone(),
        this_arg.clone(),
        this.clone(),
        Value::Bool(readonly),
        Value::Bool(shallow),
    ];
    let cb = native(rt, "", 2, for_each_callback, data);
    call_named(rt, &target, "forEach", &[Value::Object(cb)])
}

fn for_each_callback(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let w = wrap_of(matches!(d[3], Value::Bool(true)), matches!(d[4], Value::Bool(true)));
    let v = wrap(rt, w, &c.arg(0))?;
    let k = wrap(rt, w, &c.arg(1))?;
    rt.call(&d[0], &d[1], &[v, k, d[2].clone()])
}

#[derive(Clone, Copy, PartialEq)]
enum Which {
    Keys,
    Values,
    Entries,
    Iterator,
}

/// `keys`, `values`, `entries` and `Symbol.iterator`: the target's iterator, wrapped result by result.
fn coll_iterable(rt: &Rt, this: &Value, which: Which, args: &[Value], readonly: bool, shallow: bool) -> Res<Value> {
    let target = rt.get_str(this, "__v_raw")?;
    let raw_target = to_raw(rt, &target)?;
    let target_is_map = has_tag(rt, &raw_target, "Map")?;
    let is_pair = which == Which::Entries || (which == Which::Iterator && target_is_map);
    let is_key_only = which == Which::Keys && target_is_map;
    let method = match which {
        Which::Keys => PropKey::from_str("keys"),
        Which::Values => PropKey::from_str("values"),
        Which::Entries => PropKey::from_str("entries"),
        Which::Iterator => PropKey::Sym(rt.well_known(WellKnown::Iterator)),
    };
    let f = rt.get_v(&target, &method)?;
    let inner = rt.call(&f, &target, args)?;
    if !readonly {
        let keys = iterate_keys(rt);
        track_on(rt, &raw_target, if is_key_only { &keys[1] } else { &keys[0] })?;
    }
    let Some(inner_obj) = inner.as_object() else {
        return Err(rt.throw_type_error("Object prototype may only be an Object or null"));
    };
    let it = rt.new_object_with_proto(Some(inner_obj));
    let data = vec![
        inner,
        Value::Bool(is_pair),
        Value::Bool(readonly),
        Value::Bool(shallow),
    ];
    let next = native(rt, "next", 0, iterator_next, data);
    rt.define_data(it, PropKey::from_str("next"), Value::Object(next), Flags::ALL)?;
    Ok(Value::Object(it))
}

/// The `next` of a collection iterator: `{ value, done }` with the value (or the pair) wrapped.
fn iterator_next(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let inner = d[0].clone();
    let is_pair = matches!(d[1], Value::Bool(true));
    let w = wrap_of(matches!(d[2], Value::Bool(true)), matches!(d[3], Value::Bool(true)));
    let r = call_named(rt, &inner, "next", &[])?;
    let value = rt.get_str(&r, "value")?;
    let done = rt.get_str(&r, "done")?;
    let out = if done.to_boolean() {
        value
    } else if is_pair {
        let k = rt.get_v(&value, &PropKey::Index(0))?;
        let wk = wrap(rt, w, &k)?;
        let v1 = rt.get_v(&value, &PropKey::Index(1))?;
        let wv = wrap(rt, w, &v1)?;
        Value::Object(rt.new_array(vec![wk, wv]))
    } else {
        wrap(rt, w, &value)?
    };
    let o = rt.new_object();
    rt.define_data(o, PropKey::from_str("value"), out, Flags::ALL)?;
    rt.define_data(o, PropKey::from_str("done"), done, Flags::ALL)?;
    Ok(Value::Object(o))
}

/// The shared `hasOwnProperty`: tracks HAS on the raw receiver, then asks the raw object.
fn has_own_dispatch(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let key = match c.arg(0) {
        k @ Value::Symbol(_) => k,
        k => Value::String(rt.to_string(&k)?),
    };
    let obj = to_raw(rt, c.this)?;
    track_on(rt, &obj, &key)?;
    call_named(rt, &obj, "hasOwnProperty", &[key])
}

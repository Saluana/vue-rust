//! Builtin family: Object (WP-B1). The Object function with its statics (keys, values, entries, assign, freeze and
//! the integrity tests, preventExtensions, isExtensible, hasOwn, is, getPrototypeOf, create, the own-property
//! listings and descriptor reads), and Object.prototype (hasOwnProperty, isPrototypeOf, propertyIsEnumerable,
//! valueOf, toLocaleString). Object.prototype.toString is the kernel's (coerce.rs). defineProperty,
//! defineProperties and setPrototypeOf are absent (manifest X).

use crate::builtins::iterator::iterate;
use crate::function::{CodeFn, Call};
use crate::jsstr::JsStr;
use crate::object::{Flags, Prop, MAX_DENSE_LEN};
use crate::rt::{Intrinsic, Rt};
use crate::value::{same_value, ObjRef, PropKey, Res, Value};

/// The key as the JavaScript value it names: an index becomes its decimal string, a symbol stays a symbol.
pub(crate) fn key_value(k: &PropKey) -> Value {
    match k {
        PropKey::Index(i) => Value::String(JsStr::from_str(&i.to_string())),
        PropKey::Str(s) => Value::String(s.clone()),
        PropKey::Sym(s) => Value::Symbol(*s),
    }
}

/// A fresh array of `items`. A list longer than the dense limit is a RangeError (not a panic).
pub(crate) fn array_of(rt: &Rt, items: Vec<Value>) -> Res<ObjRef> {
    if items.len() > MAX_DENSE_LEN {
        return Err(rt.throw_range_error("Invalid array length"));
    }
    Ok(rt.new_array(items))
}

/// GetPrototypeFromConstructor (ECMA-262 10.1.14): `newTarget.prototype` when it is an object, else the realm's
/// intrinsic. The builtins call it only with `newTarget` set to the constructor itself, so no subclass path exists.
pub(crate) fn proto_from_ctor(rt: &Rt, new_target: &Value, fallback: Intrinsic) -> Res<ObjRef> {
    if let Value::Object(nt) = new_target {
        if let Value::Object(p) = rt.get(*nt, &PropKey::from_str("prototype"), new_target)? {
            return Ok(p);
        }
    }
    Ok(rt.intrinsic(fallback))
}

fn object_call(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    object_new(rt, c)
}

fn object_construct(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    object_new(rt, c)
}

/// `Object(value)` (ECMA-262 20.1.1.1): ToObject for a non-nullish value, else a new ordinary object.
fn object_new(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    if let Value::Object(nt) = c.new_target {
        if *nt != c.callee {
            let proto = proto_from_ctor(rt, c.new_target, Intrinsic::ObjectPrototype)?;
            return Ok(Value::Object(rt.new_object_with_proto(Some(proto))));
        }
    }
    let v = c.arg(0);
    if v.is_nullish() {
        return Ok(Value::Object(rt.new_object()));
    }
    Ok(Value::Object(rt.to_object(&v)?))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Listing {
    Keys,
    Values,
    Entries,
}

/// EnumerableOwnProperties (ECMA-262 7.3.23). Symbol keys are skipped. Each key's enumerability is checked just
/// before its value is read, so a getter that changes later keys is seen (V8 order).
fn enumerable_own(rt: &Rt, o: ObjRef, listing: Listing) -> Res<Value> {
    let mut out = Vec::new();
    for k in rt.own_keys(o)? {
        if matches!(k, PropKey::Sym(_)) {
            continue;
        }
        let Some(p) = rt.get_own_property(o, &k)? else { continue };
        if !p.flags().enumerable {
            continue;
        }
        let item = match listing {
            Listing::Keys => key_value(&k),
            Listing::Values => rt.get(o, &k, &Value::Object(o))?,
            Listing::Entries => {
                let v = rt.get(o, &k, &Value::Object(o))?;
                Value::Object(array_of(rt, vec![key_value(&k), v])?)
            }
        };
        out.push(item);
    }
    Ok(Value::Object(array_of(rt, out)?))
}

fn object_keys(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = rt.to_object(&c.arg(0))?;
    enumerable_own(rt, o, Listing::Keys)
}

fn object_values(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = rt.to_object(&c.arg(0))?;
    enumerable_own(rt, o, Listing::Values)
}

fn object_entries(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = rt.to_object(&c.arg(0))?;
    enumerable_own(rt, o, Listing::Entries)
}

/// Object.assign (ECMA-262 20.1.2.1): copies enumerable own properties (symbols too) with [[Set]] that throws on
/// refusal. Nullish sources are skipped.
fn object_assign(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let to = rt.to_object(&c.arg(0))?;
    for src in c.args.get(1..).unwrap_or(&[]) {
        if src.is_nullish() {
            continue;
        }
        let from = rt.to_object(src)?;
        for k in rt.own_keys(from)? {
            let Some(p) = rt.get_own_property(from, &k)? else { continue };
            if !p.flags().enumerable {
                continue;
            }
            let v = rt.get(from, &k, &Value::Object(from))?;
            rt.set_strict(to, &k, v)?;
        }
    }
    Ok(Value::Object(to))
}

fn object_freeze(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let v = c.arg(0);
    if let Value::Object(o) = v {
        if !rt.freeze(o)? {
            return Err(rt.throw_type_error("Cannot freeze"));
        }
    }
    Ok(v)
}

fn object_seal(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let v = c.arg(0);
    if let Value::Object(o) = v {
        if !rt.seal(o)? {
            return Err(rt.throw_type_error("Cannot seal"));
        }
    }
    Ok(v)
}

fn object_is_frozen(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    match c.arg(0) {
        Value::Object(o) => Ok(Value::Bool(rt.is_frozen(o)?)),
        _ => Ok(Value::Bool(true)),
    }
}

fn object_is_sealed(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    match c.arg(0) {
        Value::Object(o) => Ok(Value::Bool(rt.is_sealed(o)?)),
        _ => Ok(Value::Bool(true)),
    }
}

fn object_prevent_extensions(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let v = c.arg(0);
    if let Value::Object(o) = v {
        if !rt.prevent_extensions(o)? {
            return Err(rt.throw_type_error("Cannot prevent extensions"));
        }
    }
    Ok(v)
}

fn object_is_extensible(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    match c.arg(0) {
        Value::Object(o) => Ok(Value::Bool(rt.is_extensible(o)?)),
        _ => Ok(Value::Bool(false)),
    }
}

/// Object.hasOwn (ECMA-262 20.1.2.11): ToObject of the target first, then ToPropertyKey of the key.
fn object_has_own(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = rt.to_object(&c.arg(0))?;
    let k = rt.to_property_key(&c.arg(1))?;
    Ok(Value::Bool(rt.has_own_property(o, &k)?))
}

fn object_is(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(same_value(&c.arg(0), &c.arg(1))))
}

fn object_get_prototype_of(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = rt.to_object(&c.arg(0))?;
    Ok(match rt.get_prototype_of(o) {
        Some(p) => Value::Object(p),
        None => Value::Null,
    })
}

/// Object.create (ECMA-262 20.1.2.2). The properties argument is supported only when it is undefined; any other
/// value throws TypeError (conservative: defineProperties is not implemented).
fn object_create(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let proto = match c.arg(0) {
        Value::Object(p) => Some(p),
        Value::Null => None,
        other => {
            return Err(rt.throw_type_error(&format!(
                "Object prototype may only be an Object or null: {}",
                rt.describe(&other)
            )))
        }
    };
    if !c.arg(1).is_undefined() {
        return Err(rt.throw_type_error("unsupported: Object.create with a properties argument"));
    }
    Ok(Value::Object(rt.new_object_with_proto(proto)))
}

/// Object.fromEntries (ECMA-262 20.1.2.7): each item must be an object whose "0" and "1" give the key and value. A
/// non-object item, or a throw from the key or value conversion, closes the iterator (through `iterate`).
fn object_from_entries(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let iterable = c.arg(0);
    if iterable.is_nullish() {
        return Err(rt.throw_type_error(&format!("{} is not iterable", rt.describe(&iterable))));
    }
    let out = rt.new_object();
    iterate(rt, &iterable, &mut |entry| {
        let e = match entry {
            Value::Object(e) => e,
            other => {
                return Err(rt.throw_type_error(&format!("Iterator value {} is not an entry object", rt.describe(&other))))
            }
        };
        let k = rt.get(e, &PropKey::Index(0), &Value::Object(e))?;
        let v = rt.get(e, &PropKey::Index(1), &Value::Object(e))?;
        let key = rt.to_property_key(&k)?;
        rt.create_data_property(out, key, v)?;
        Ok(true)
    })?;
    Ok(Value::Object(out))
}

/// Object.groupBy (ECMA-262 20.1.2.12): groups in first-seen order, keyed by ToPropertyKey of the callback result. The
/// callback gets (value, index) with `this` undefined. The result has a null prototype.
fn object_group_by(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let items = c.arg(0);
    if items.is_nullish() {
        return Err(rt.throw_type_error(&format!("{} is not iterable", rt.describe(&items))));
    }
    let callback = c.arg(1);
    if !rt.is_callable(&callback) {
        return Err(rt.throw_type_error(&format!("{} is not a function", rt.describe(&callback))));
    }
    let mut groups: Vec<(PropKey, Vec<Value>)> = Vec::new();
    let mut index = 0.0;
    iterate(rt, &items, &mut |value| {
        let key = rt.call(&callback, &Value::Undefined, &[value.clone(), Value::Number(index)])?;
        let key = rt.to_property_key(&key)?;
        match groups.iter().position(|(g, _)| *g == key) {
            Some(i) => groups[i].1.push(value),
            None => groups.push((key, vec![value])),
        }
        index += 1.0;
        Ok(true)
    })?;
    let out = rt.new_object_with_proto(None);
    for (key, members) in groups {
        let list = array_of(rt, members)?;
        rt.create_data_property(out, key, Value::Object(list))?;
    }
    Ok(Value::Object(out))
}

fn object_get_own_property_names(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = rt.to_object(&c.arg(0))?;
    let names: Vec<Value> = rt
        .own_keys(o)?
        .iter()
        .filter(|k| !matches!(k, PropKey::Sym(_)))
        .map(key_value)
        .collect();
    Ok(Value::Object(array_of(rt, names)?))
}

fn object_get_own_property_symbols(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = rt.to_object(&c.arg(0))?;
    let syms: Vec<Value> = rt
        .own_keys(o)?
        .into_iter()
        .filter_map(|k| match k {
            PropKey::Sym(s) => Some(Value::Symbol(s)),
            _ => None,
        })
        .collect();
    Ok(Value::Object(array_of(rt, syms)?))
}

/// FromPropertyDescriptor (ECMA-262 6.2.6.4): `value, writable` for a data property or `get, set` for an accessor,
/// then `enumerable, configurable`.
fn descriptor_object(rt: &Rt, p: &Prop) -> Res<ObjRef> {
    let d = rt.new_object();
    match p {
        Prop::Data { value, flags } => {
            rt.create_data_property(d, PropKey::from_str("value"), value.clone())?;
            rt.create_data_property(d, PropKey::from_str("writable"), Value::Bool(flags.writable))?;
        }
        Prop::Accessor { get, set, .. } => {
            let g = get.map_or(Value::Undefined, Value::Object);
            let s = set.map_or(Value::Undefined, Value::Object);
            rt.create_data_property(d, PropKey::from_str("get"), g)?;
            rt.create_data_property(d, PropKey::from_str("set"), s)?;
        }
    }
    let flags = p.flags();
    rt.create_data_property(d, PropKey::from_str("enumerable"), Value::Bool(flags.enumerable))?;
    rt.create_data_property(d, PropKey::from_str("configurable"), Value::Bool(flags.configurable))?;
    Ok(d)
}

fn object_get_own_property_descriptor(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = rt.to_object(&c.arg(0))?;
    let k = rt.to_property_key(&c.arg(1))?;
    match rt.get_own_property(o, &k)? {
        Some(p) => Ok(Value::Object(descriptor_object(rt, &p)?)),
        None => Ok(Value::Undefined),
    }
}

fn object_get_own_property_descriptors(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = rt.to_object(&c.arg(0))?;
    let out = rt.new_object();
    for k in rt.own_keys(o)? {
        if let Some(p) = rt.get_own_property(o, &k)? {
            let d = descriptor_object(rt, &p)?;
            rt.create_data_property(out, k, Value::Object(d))?;
        }
    }
    Ok(Value::Object(out))
}

// ----- Object.prototype -----

/// `hasOwnProperty` (ECMA-262 20.1.3.2): ToPropertyKey of the argument, then ToObject of `this`.
fn proto_has_own_property(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let k = rt.to_property_key(&c.arg(0))?;
    let o = rt.to_object(c.this)?;
    Ok(Value::Bool(rt.has_own_property(o, &k)?))
}

fn proto_is_prototype_of(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let Value::Object(v) = c.arg(0) else { return Ok(Value::Bool(false)) };
    let o = rt.to_object(c.this)?;
    let mut cur = rt.get_prototype_of(v);
    while let Some(p) = cur {
        if p == o {
            return Ok(Value::Bool(true));
        }
        cur = rt.get_prototype_of(p);
    }
    Ok(Value::Bool(false))
}

fn proto_property_is_enumerable(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let k = rt.to_property_key(&c.arg(0))?;
    let o = rt.to_object(c.this)?;
    Ok(Value::Bool(match rt.get_own_property(o, &k)? {
        Some(p) => p.flags().enumerable,
        None => false,
    }))
}

fn proto_value_of(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Object(rt.to_object(c.this)?))
}

/// `toLocaleString` (ECMA-262 20.1.3.5): Invoke(this, "toString").
fn proto_to_locale_string(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let f = rt.get_v(c.this, &PropKey::from_str("toString"))?;
    rt.call(&f, c.this, &[])
}

pub fn install(rt: &Rt) {
    let op = rt.intrinsic(Intrinsic::ObjectPrototype);
    let ctor = rt.new_native_constructor(object_call, object_construct, "Object", 1, Vec::new());
    rt.define_builtin(ctor, PropKey::from_str("prototype"), Prop::Data { value: Value::Object(op), flags: Flags::NONE });
    rt.define_builtin(op, PropKey::from_str("constructor"), Prop::Data { value: Value::Object(ctor), flags: Flags::HIDDEN });

    let statics: &[(&str, CodeFn, u32)] = &[
        ("keys", object_keys, 1),
        ("values", object_values, 1),
        ("entries", object_entries, 1),
        ("fromEntries", object_from_entries, 1),
        ("groupBy", object_group_by, 2),
        ("assign", object_assign, 2),
        ("freeze", object_freeze, 1),
        ("isFrozen", object_is_frozen, 1),
        ("seal", object_seal, 1),
        ("isSealed", object_is_sealed, 1),
        ("preventExtensions", object_prevent_extensions, 1),
        ("isExtensible", object_is_extensible, 1),
        ("hasOwn", object_has_own, 2),
        ("is", object_is, 2),
        ("getPrototypeOf", object_get_prototype_of, 1),
        ("create", object_create, 2),
        ("getOwnPropertyNames", object_get_own_property_names, 1),
        ("getOwnPropertySymbols", object_get_own_property_symbols, 1),
        ("getOwnPropertyDescriptor", object_get_own_property_descriptor, 2),
        ("getOwnPropertyDescriptors", object_get_own_property_descriptors, 1),
    ];
    for &(name, code, len) in statics {
        rt.define_native_method(ctor, name, code, len);
    }

    rt.define_native_method(op, "hasOwnProperty", proto_has_own_property, 1);
    rt.define_native_method(op, "isPrototypeOf", proto_is_prototype_of, 1);
    rt.define_native_method(op, "propertyIsEnumerable", proto_property_is_enumerable, 1);
    rt.define_native_method(op, "valueOf", proto_value_of, 0);
    rt.define_native_method(op, "toLocaleString", proto_to_locale_string, 0);

    rt.install_global("Object", Value::Object(ctor));
    rt.register_builtin("Object", ctor);
}

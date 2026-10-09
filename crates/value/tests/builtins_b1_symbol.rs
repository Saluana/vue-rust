//! WP-B1: Symbol (ECMA-262 20.4) and the symbol operators of the kernel (typeof, instanceof, ToString, ToPrimitive).
//! Symbol is a function and not a constructor. Expected values follow the spec and were checked against Node 26 (V8):
//! descriptions, the registry, "[object Symbol]" through @@toStringTag, and the TypeErrors.

use vn_value::{Call, Flags, Hint, Intrinsic, Prop, PropKey, Res, Rt, Sym, Throw, Value, WellKnown};

fn ok<T>(r: Res<T>) -> T {
    match r {
        Ok(v) => v,
        Err(Throw(v)) => panic!("unexpected JS throw: {:?}", v),
    }
}

fn thrown_name(rt: &Rt, r: Res<Value>) -> String {
    match r {
        Err(Throw(e)) => match rt.get_str(&e, "name") {
            Ok(Value::String(s)) => s.to_rust_string_lossy(),
            _ => String::from("<none>"),
        },
        Ok(v) => panic!("expected a throw, got {:?}", v),
    }
}

fn global(rt: &Rt) -> Value {
    Value::Object(rt.intrinsic(Intrinsic::Global))
}

fn gl(rt: &Rt, name: &str) -> Value {
    ok(rt.get_str(&global(rt), name))
}

fn getp(rt: &Rt, base: &Value, name: &str) -> Value {
    ok(rt.get_str(base, name))
}

fn s(x: &str) -> Value {
    Value::str(x)
}

fn n(x: f64) -> Value {
    Value::Number(x)
}

fn sym_of(v: &Value) -> Sym {
    match v {
        Value::Symbol(s) => *s,
        other => panic!("expected a symbol, got {other:?}"),
    }
}

fn string_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.to_rust_string_lossy(),
        other => panic!("expected a string, got {other:?}"),
    }
}

fn symbol_ctor(rt: &Rt) -> Value {
    gl(rt, "Symbol")
}

/// `Symbol.prototype[name].call(this, ...args)`.
fn sp_call(rt: &Rt, name: &str, this: &Value, args: &[Value]) -> Res<Value> {
    let sp = Value::Object(rt.intrinsic(Intrinsic::SymbolPrototype));
    let f = getp(rt, &sp, name);
    rt.call(&f, this, args)
}

/// `Symbol[name](...args)`.
fn static_call(rt: &Rt, name: &str, args: &[Value]) -> Res<Value> {
    let ctor = symbol_ctor(rt);
    let f = getp(rt, &ctor, name);
    rt.call(&f, &ctor, args)
}

/// Calls `Symbol(...args)` through the global.
fn make(rt: &Rt, args: &[Value]) -> Res<Value> {
    let ctor = symbol_ctor(rt);
    rt.call(&ctor, &Value::Undefined, args)
}

fn description(rt: &Rt, sym: &Value) -> Value {
    let sp = Value::Object(rt.intrinsic(Intrinsic::SymbolPrototype));
    let slot = rt.get_own_property(sp.as_object().unwrap(), &PropKey::from_str("description")).unwrap().unwrap();
    let Prop::Accessor { get: Some(g), .. } = slot else { panic!("description is an accessor with a getter") };
    ok(rt.call(&Value::Object(g), sym, &[]))
}

fn to_q(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::str("q"))
}

#[test]
fn symbol_is_a_function_and_not_a_constructor() {
    let rt = Rt::new();
    let ctor = symbol_ctor(&rt);
    assert!(rt.is_callable(&ctor));
    assert!(!rt.is_constructor(&ctor), "Symbol has no [[Construct]]");
    assert_eq!(thrown_name(&rt, rt.construct(&ctor, &[], &ctor)), "TypeError");
    assert_eq!(getp(&rt, &ctor, "length"), n(0.0));
    assert_eq!(getp(&rt, &ctor, "name"), s("Symbol"));
    let sp = rt.intrinsic(Intrinsic::SymbolPrototype);
    let proto_slot = rt.get_own_property(ctor.as_object().unwrap(), &PropKey::from_str("prototype")).unwrap().unwrap();
    assert_eq!(proto_slot.flags(), Flags::NONE);
    assert_eq!(getp(&rt, &ctor, "prototype"), Value::Object(sp));
    let back = rt.get_own_property(sp, &PropKey::from_str("constructor")).unwrap().unwrap();
    assert_eq!(back.flags(), Flags::HIDDEN);
    let Prop::Data { value, .. } = back else { panic!("data") };
    assert_eq!(value, ctor);
}

#[test]
fn symbol_creates_fresh_symbols_with_optional_descriptions() {
    let rt = Rt::new();
    let none = ok(make(&rt, &[]));
    assert!(matches!(none, Value::Symbol(_)));
    assert_eq!(description(&rt, &none), Value::Undefined);
    let explicit_undefined = ok(make(&rt, &[Value::Undefined]));
    assert_eq!(description(&rt, &explicit_undefined), Value::Undefined);
    let x = ok(make(&rt, &[s("x")]));
    assert_eq!(description(&rt, &x), s("x"));
    let null_desc = ok(make(&rt, &[Value::Null]));
    assert_eq!(description(&rt, &null_desc), s("null"), "null is ToString'd");
    let one = ok(make(&rt, &[n(1.0)]));
    assert_eq!(description(&rt, &one), s("1"));
    let obj = rt.new_object();
    let f = rt.new_native_function(to_q, "toString", 0, Vec::new());
    ok(rt.define_data(obj, PropKey::from_str("toString"), Value::Object(f), Flags::HIDDEN));
    let from_obj = ok(make(&rt, &[Value::Object(obj)]));
    assert_eq!(description(&rt, &from_obj), s("q"), "an object description is ToString'd");
    // each call makes a new symbol, even with the same description
    let a1 = ok(make(&rt, &[s("a")]));
    let a2 = ok(make(&rt, &[s("a")]));
    assert_ne!(a1, a2);
    assert_ne!(sym_of(&a1), sym_of(&a2));
}

#[test]
fn symbol_rejects_a_symbol_description() {
    let rt = Rt::new();
    let inner = ok(make(&rt, &[]));
    assert_eq!(thrown_name(&rt, make(&rt, &[inner])), "TypeError", "ToString of a symbol throws");
}

#[test]
fn symbol_for_and_key_for_form_the_global_registry() {
    let rt = Rt::new();
    let a = ok(static_call(&rt, "for", &[s("k")]));
    let b = ok(static_call(&rt, "for", &[s("k")]));
    assert_eq!(a, b, "Symbol.for returns the same symbol for the same key");
    assert_ne!(a, ok(make(&rt, &[s("k")])), "Symbol() is never registered");
    assert_eq!(ok(static_call(&rt, "keyFor", &[a.clone()])), s("k"));
    let undef_key = ok(static_call(&rt, "for", &[Value::Undefined]));
    assert_eq!(ok(static_call(&rt, "keyFor", &[undef_key])), s("undefined"), "the key is ToString(undefined)");
    let plain = ok(make(&rt, &[s("k")]));
    assert_eq!(ok(static_call(&rt, "keyFor", &[plain])), Value::Undefined);
    let iter = Value::Symbol(rt.well_known(WellKnown::Iterator));
    assert_eq!(ok(static_call(&rt, "keyFor", &[iter])), Value::Undefined, "well-known symbols are not registered");
    assert_eq!(thrown_name(&rt, static_call(&rt, "keyFor", &[s("k")])), "TypeError");
    assert_eq!(thrown_name(&rt, static_call(&rt, "keyFor", &[])), "TypeError");
    assert_eq!(thrown_name(&rt, static_call(&rt, "keyFor", &[n(1.0)])), "TypeError");
    // Symbol.for("") is a registry key too
    let empty = ok(static_call(&rt, "for", &[s("")]));
    assert_eq!(ok(static_call(&rt, "keyFor", &[empty])), s(""));
}

#[test]
fn well_known_symbols_are_the_five_listed_and_nothing_else() {
    let rt = Rt::new();
    let ctor = symbol_ctor(&rt);
    let pairs = [
        ("iterator", WellKnown::Iterator, "Symbol.iterator"),
        ("asyncIterator", WellKnown::AsyncIterator, "Symbol.asyncIterator"),
        ("toPrimitive", WellKnown::ToPrimitive, "Symbol.toPrimitive"),
        ("toStringTag", WellKnown::ToStringTag, "Symbol.toStringTag"),
        ("hasInstance", WellKnown::HasInstance, "Symbol.hasInstance"),
    ];
    for (name, w, text) in pairs {
        let v = getp(&rt, &ctor, name);
        assert_eq!(sym_of(&v), rt.well_known(w), "Symbol.{name}");
        let slot = rt.get_own_property(ctor.as_object().unwrap(), &PropKey::from_str(name)).unwrap().unwrap();
        assert_eq!(slot.flags(), Flags::NONE, "Symbol.{name} is read-only and permanent");
        assert_eq!(description(&rt, &v), s(text), "description of Symbol.{name}");
    }
    for absent in ["species", "match", "matchAll", "replace", "search", "split", "unscopables", "isConcatSpreadable", "dispose"] {
        assert_eq!(getp(&rt, &ctor, absent), Value::Undefined, "Symbol.{absent} is absent");
    }
}

#[test]
fn prototype_to_string_and_value_of() {
    let rt = Rt::new();
    let sym = ok(make(&rt, &[s("desc")]));
    assert_eq!(string_of(&ok(sp_call(&rt, "toString", &sym, &[]))), "Symbol(desc)");
    let bare = ok(make(&rt, &[]));
    assert_eq!(string_of(&ok(sp_call(&rt, "toString", &bare, &[]))), "Symbol()");
    assert_eq!(ok(sp_call(&rt, "valueOf", &sym, &[])), sym);
    // through a wrapper: Object(sym) is a symbol wrapper
    let wrapper = ok(rt.to_object(&sym).map(Value::Object));
    assert_eq!(string_of(&ok(sp_call(&rt, "toString", &wrapper, &[]))), "Symbol(desc)");
    assert_eq!(ok(sp_call(&rt, "valueOf", &wrapper, &[])), sym);
    assert_eq!(rt.get_prototype_of(wrapper.as_object().unwrap()), Some(rt.intrinsic(Intrinsic::SymbolPrototype)));
    // Symbol.iterator prints its own description
    let it = Value::Symbol(rt.well_known(WellKnown::Iterator));
    assert_eq!(string_of(&ok(sp_call(&rt, "toString", &it, &[]))), "Symbol(Symbol.iterator)");
    // non-symbol receivers are TypeErrors, including Symbol.prototype itself (it is an ordinary object)
    let sp = Value::Object(rt.intrinsic(Intrinsic::SymbolPrototype));
    assert_eq!(thrown_name(&rt, sp_call(&rt, "toString", &sp, &[])), "TypeError");
    assert_eq!(thrown_name(&rt, sp_call(&rt, "valueOf", &sp, &[])), "TypeError");
    assert_eq!(thrown_name(&rt, sp_call(&rt, "toString", &s("x"), &[])), "TypeError");
    assert_eq!(thrown_name(&rt, sp_call(&rt, "valueOf", &n(1.0), &[])), "TypeError");
    assert_eq!(thrown_name(&rt, sp_call(&rt, "toString", &Value::Undefined, &[])), "TypeError");
}

#[test]
fn description_getter_reads_the_description_and_is_configurable_only() {
    let rt = Rt::new();
    let sym = ok(make(&rt, &[s("d")]));
    assert_eq!(description(&rt, &sym), s("d"));
    let bare = ok(make(&rt, &[]));
    assert_eq!(description(&rt, &bare), Value::Undefined);
    assert_eq!(description(&rt, &Value::Symbol(rt.well_known(WellKnown::AsyncIterator))), s("Symbol.asyncIterator"));
    let sp = rt.intrinsic(Intrinsic::SymbolPrototype);
    let slot = rt.get_own_property(sp, &PropKey::from_str("description")).unwrap().unwrap();
    assert_eq!(slot.flags(), Flags::new(false, false, true));
    assert!(slot.is_accessor());
    // the getter rejects other receivers, including Symbol.prototype
    let Prop::Accessor { get: Some(g), .. } = slot else { panic!("accessor") };
    assert_eq!(thrown_name(&rt, rt.call(&Value::Object(g), &Value::Object(sp), &[])), "TypeError");
    assert_eq!(thrown_name(&rt, rt.call(&Value::Object(g), &n(1.0), &[])), "TypeError");
    // the getter is named "get description"
    assert_eq!(getp(&rt, &Value::Object(g), "name"), s("get description"));
}

#[test]
fn to_primitive_method_returns_the_symbol() {
    let rt = Rt::new();
    let sp = rt.intrinsic(Intrinsic::SymbolPrototype);
    let key = PropKey::Sym(rt.well_known(WellKnown::ToPrimitive));
    let slot = rt.get_own_property(sp, &key).unwrap().unwrap();
    assert_eq!(slot.flags(), Flags::new(false, false, true), "writable false, enumerable false, configurable true");
    let Prop::Data { value: f, .. } = slot else { panic!("data") };
    assert_eq!(getp(&rt, &f, "name"), s("[Symbol.toPrimitive]"));
    assert_eq!(getp(&rt, &f, "length"), n(1.0));
    let sym = ok(make(&rt, &[s("p")]));
    assert_eq!(ok(rt.call(&f, &sym, &[s("string")])), sym);
    let wrapper = Value::Object(rt.to_object(&sym).unwrap());
    assert_eq!(ok(rt.call(&f, &wrapper, &[s("number")])), sym);
    assert_eq!(thrown_name(&rt, rt.call(&f, &s("x"), &[])), "TypeError");
    // ToPrimitive of a symbol wrapper gives the symbol itself, for every hint
    assert_eq!(ok(rt.to_primitive(&wrapper, Hint::String)), sym);
    assert_eq!(ok(rt.to_primitive(&wrapper, Hint::Number)), sym);
}

#[test]
fn to_string_tag_and_object_to_string() {
    let rt = Rt::new();
    let sp = rt.intrinsic(Intrinsic::SymbolPrototype);
    let key = PropKey::Sym(rt.well_known(WellKnown::ToStringTag));
    let slot = rt.get_own_property(sp, &key).unwrap().unwrap();
    assert_eq!(slot.flags(), Flags::new(false, false, true));
    let Prop::Data { value, .. } = slot else { panic!("data") };
    assert_eq!(value, s("Symbol"));

    let object_proto = Value::Object(rt.intrinsic(Intrinsic::ObjectPrototype));
    let to_string = getp(&rt, &object_proto, "toString");
    let sym = ok(make(&rt, &[]));
    assert_eq!(string_of(&ok(rt.call(&to_string, &sym, &[]))), "[object Symbol]");
    let wrapper = Value::Object(rt.to_object(&sym).unwrap());
    assert_eq!(string_of(&ok(rt.call(&to_string, &wrapper, &[]))), "[object Symbol]");
}

#[test]
fn operators_typeof_instanceof_and_conversions() {
    let rt = Rt::new();
    let sym = ok(make(&rt, &[s("o")]));
    let symbol_ctor = symbol_ctor(&rt);
    assert_eq!(rt.typeof_(&sym), "symbol");
    let wrapper = Value::Object(rt.to_object(&sym).unwrap());
    assert_eq!(rt.typeof_(&wrapper), "object");
    // primitives are not instances; wrappers are
    assert!(!ok(rt.instance_of(&sym, &symbol_ctor)));
    assert!(ok(rt.instance_of(&wrapper, &symbol_ctor)));
    // ToString, ToNumber and the string concatenation all throw for a symbol
    assert_eq!(thrown_name(&rt, rt.to_string(&sym).map(|_| Value::Undefined)), "TypeError");
    assert_eq!(thrown_name(&rt, rt.to_number(&sym).map(Value::Number)), "TypeError");
    assert_eq!(thrown_name(&rt, rt.add(&sym, &s("")).map(|_| Value::Undefined)), "TypeError");
    assert_eq!(thrown_name(&rt, rt.add(&wrapper, &s("")).map(|_| Value::Undefined)), "TypeError");
    // a template literal calls ToString on the value: that is a TypeError too
    assert_eq!(thrown_name(&rt, rt.to_string(&wrapper).map(|_| Value::Undefined)), "TypeError");
    // equality: identity, and a wrapper equals its symbol after ToPrimitive
    assert!(vn_value::strict_eq(&sym, &sym));
    assert_eq!(ok(rt.loose_eq(&wrapper, &sym)), Value::Bool(true));
    assert_eq!(ok(rt.loose_eq(&sym, &s("x"))), Value::Bool(false));
    // a symbol is truthy
    assert!(sym.to_boolean());
    assert!(wrapper.to_boolean());
    // the wrapper's String conversion is not reachable here; ToPrimitive keeps the symbol
    assert_eq!(ok(rt.to_primitive(&wrapper, Hint::Default)), sym);
}

#[test]
fn the_registry_survives_collection() {
    // Symbols are immortal in this kernel, so a registry key still maps to its symbol after a collection.
    let rt = Rt::new();
    let a = ok(static_call(&rt, "for", &[s("persist")]));
    rt.collect();
    let b = ok(static_call(&rt, "for", &[s("persist")]));
    assert_eq!(a, b);
    assert_eq!(ok(static_call(&rt, "keyFor", &[a])), s("persist"));
}

#[test]
fn symbol_description_matches_the_host_text() {
    let rt = Rt::new();
    let sym = ok(make(&rt, &[s("café")]));
    assert_eq!(string_of(&ok(sp_call(&rt, "toString", &sym, &[]))), "Symbol(café)");
}

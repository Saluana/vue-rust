//! WP-L2b kernel amendments: `Exotic::is_array` (Array.isArray of an exotic array), the `Wrapper` object kind
//! made by ToObject, `class_of`, and the own properties of a String wrapper. Expected values come from ECMA-262
//! (7.1.18 ToObject, 10.4.3 String exotic objects) and were cross-checked with Node 26 (V8).

use std::rc::Rc;

use vn_value::{
    Call, ErrorKind, Exotic, Flags, Intrinsic, NativeObject, ObjClass, ObjRef, Prop, PropKey, Res, Rt, Tracer, Value,
};

fn ok<T>(r: Res<T>) -> T {
    match r {
        Ok(v) => v,
        Err(t) => panic!("unexpected JS throw: {:?}", t.0),
    }
}

fn message(rt: &Rt, v: &Value) -> String {
    match rt.get_str(v, "message") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => String::from("<none>"),
    }
}

fn keys_text(rt: &Rt, o: ObjRef) -> Vec<String> {
    ok(rt.own_keys(o))
        .into_iter()
        .map(|k| match k {
            PropKey::Index(i) => i.to_string(),
            PropKey::Str(s) => s.to_rust_string_lossy(),
            PropKey::Sym(_) => String::from("<symbol>"),
        })
        .collect()
}

/// An exotic object whose only hook is `is_array`, as the reactivity port's array proxies use it.
struct ArrayProxy(bool);

impl Exotic for ArrayProxy {
    fn trace(&self, _t: &mut Tracer) {}

    fn is_array(&self) -> bool {
        self.0
    }
}

/// An exotic that overrides no hook: `is_array` keeps its default.
struct Plain;

impl Exotic for Plain {
    fn trace(&self, _t: &mut Tracer) {}
}

struct Payload;

impl NativeObject for Payload {
    fn trace(&self, _t: &mut Tracer) {}
}

fn noop(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Undefined)
}

#[test]
fn exotic_is_array_hook_answers_is_array_and_nothing_else() {
    let rt = Rt::new();
    let proto = Some(rt.intrinsic(Intrinsic::ObjectPrototype));
    let array_proxy = rt.new_exotic_object(proto, Rc::new(ArrayProxy(true)));
    let object_proxy = rt.new_exotic_object(proto, Rc::new(ArrayProxy(false)));
    // Array.isArray(new Proxy([], {})) is true in JavaScript; a proxy of a plain object is not an array (V8 agrees).
    assert!(rt.is_array(&Value::Object(array_proxy)));
    assert!(!rt.is_array(&Value::Object(object_proxy)));
    assert!(rt.is_array(&Value::Object(rt.new_array(vec![]))));
    assert!(!rt.is_array(&Value::Object(rt.new_object())));
    assert!(!rt.is_array(&Value::Number(1.0)));
    // The hook does not create element storage: the exotic has no own keys of its own.
    assert!(ok(rt.own_keys(array_proxy)).is_empty());
    assert!(!Plain.is_array(), "the default Exotic::is_array is false");
}

#[test]
fn class_of_names_every_object_kind() {
    let rt = Rt::new();
    let f = rt.new_native_function(noop, "f", 0, vec![]);
    let err = rt.make_error(ErrorKind::Type, "x");
    let wrapper = ok(rt.to_object(&Value::Bool(false)));
    assert_eq!(rt.class_of(rt.new_object()), ObjClass::Ordinary);
    assert_eq!(rt.class_of(rt.new_array(vec![])), ObjClass::Array);
    assert_eq!(rt.class_of(f), ObjClass::Function);
    assert_eq!(rt.class_of(err.as_object().unwrap()), ObjClass::Error);
    assert_eq!(rt.class_of(wrapper), ObjClass::Wrapper);
    assert_eq!(rt.class_of(rt.new_cell(Value::Undefined)), ObjClass::Cell);
    assert_eq!(rt.class_of(rt.new_native_object(None, Rc::new(Payload))), ObjClass::Native);
    assert_eq!(rt.class_of(rt.new_exotic_object(None, Rc::new(Plain))), ObjClass::Exotic);
}

#[test]
fn to_object_wraps_each_primitive_with_its_intrinsic_prototype() {
    let rt = Rt::new();
    let sym = rt.new_symbol(None);
    let cases = [
        (Value::Bool(true), Intrinsic::BooleanPrototype),
        (Value::Number(2.0), Intrinsic::NumberPrototype),
        (Value::str("ab"), Intrinsic::StringPrototype),
        (Value::Symbol(sym), Intrinsic::SymbolPrototype),
    ];
    for (v, proto) in cases {
        let w = ok(rt.to_object(&v));
        assert_eq!(rt.get_prototype_of(w), Some(rt.intrinsic(proto)), "prototype of {v:?}");
        assert_eq!(rt.class_of(w), ObjClass::Wrapper);
    }
    // ToObject of an object is that object.
    let o = rt.new_object();
    assert_eq!(ok(rt.to_object(&Value::Object(o))), o);
    // undefined and null throw; V8 uses the same message.
    for v in [Value::Undefined, Value::Null] {
        let t = rt.to_object(&v).unwrap_err();
        assert_eq!(message(&rt, &t.0), "Cannot convert undefined or null to object");
    }
    // The wrapped Number is visible through Object.prototype.toString, as in V8: "[object Number]".
    let w = ok(rt.to_object(&Value::Number(2.0)));
    let object_proto = Value::Object(rt.intrinsic(Intrinsic::ObjectPrototype));
    let otos = ok(rt.get_str(&object_proto, "toString"));
    assert_eq!(ok(rt.call(&otos, &Value::Object(w), &[])), Value::str("[object Number]"));
}

#[test]
fn a_string_wrapper_has_read_only_length_and_character_properties() {
    let rt = Rt::new();
    let s = ok(rt.to_object(&Value::str("ab")));
    let w = Value::Object(s);
    // Object.getOwnPropertyNames(new String("ab")) is ["0", "1", "length"] in V8.
    assert_eq!(keys_text(&rt, s), ["0", "1", "length"]);
    // length: 2, neither writable, enumerable nor configurable (the V8 descriptor).
    assert_eq!(
        ok(rt.get_own_property(s, &PropKey::from_str("length"))),
        Some(Prop::Data { value: Value::Number(2.0), flags: Flags::NONE })
    );
    // index 0: "a", read-only, enumerable, non-configurable.
    assert_eq!(
        ok(rt.get_own_property(s, &PropKey::Index(0))),
        Some(Prop::Data { value: Value::str("a"), flags: Flags::new(false, true, false) })
    );
    assert_eq!(ok(rt.get_own_property(s, &PropKey::Index(2))), None);
    // Reads go through the ordinary path (V8: new String("ab")[1] is "b", [2] is undefined).
    assert_eq!(ok(rt.get_str(&w, "length")), Value::Number(2.0));
    assert_eq!(ok(rt.get_v(&w, &PropKey::Index(1))), Value::str("b"));
    assert_eq!(ok(rt.get_v(&w, &PropKey::Index(2))), Value::Undefined);
    // Writes and deletes of the protected properties are refused (Reflect.set and Reflect.deleteProperty give false).
    assert!(!ok(rt.set_v(&w, &PropKey::Index(0), Value::str("z"))));
    assert!(!ok(rt.set_v(&w, &PropKey::from_str("length"), Value::Number(9.0))));
    assert!(!ok(rt.delete(s, &PropKey::Index(0))));
    assert!(!ok(rt.delete(s, &PropKey::from_str("length"))));
    assert_eq!(ok(rt.get_v(&w, &PropKey::Index(0))), Value::str("a"));
    // Redefinition: the same value and flags are accepted, a different value or a writable length is refused
    // (Reflect.defineProperty: false, false, true, false in V8).
    let ro_enum = Flags::new(false, true, false);
    assert!(!ok(rt.define_data(s, PropKey::Index(0), Value::str("z"), ro_enum)));
    assert!(ok(rt.define_data(s, PropKey::Index(0), Value::str("a"), ro_enum)));
    assert!(!ok(rt.define_data(s, PropKey::from_str("length"), Value::Number(2.0), Flags::ALL)));
    assert!(ok(rt.define_data(s, PropKey::from_str("length"), Value::Number(2.0), Flags::NONE)));
    // An index past the end is an ordinary property, listed in index order before `length`.
    assert!(ok(rt.define_data(s, PropKey::Index(5), Value::Number(9.0), Flags::ALL)));
    assert_eq!(keys_text(&rt, s), ["0", "1", "5", "length"]);
    assert_eq!(ok(rt.get_v(&w, &PropKey::Index(5))), Value::Number(9.0));
}

//! WP-B1: Boolean (ECMA-262 20.3). Boolean is a function and a constructor; new Boolean(v) is a Boolean wrapper;
//! Boolean.prototype is a Boolean wrapper holding false. Expected values follow the spec and were checked against
//! Node 26 (V8).

use vn_value::{Call, ObjClass, Flags, Hint, Intrinsic, Prop, PropKey, Res, Rt, Throw, Value};

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

fn bp(rt: &Rt) -> Value {
    Value::Object(rt.intrinsic(Intrinsic::BooleanPrototype))
}

/// `Boolean(...args)` (a plain call).
fn call_boolean(rt: &Rt, args: &[Value]) -> Res<Value> {
    let b = gl(rt, "Boolean");
    rt.call(&b, &Value::Undefined, args)
}

/// `new Boolean(...args)`.
fn new_boolean(rt: &Rt, args: &[Value]) -> Value {
    let b = gl(rt, "Boolean");
    ok(rt.construct(&b, args, &b))
}

/// `Boolean.prototype[name].call(this, ...args)`.
fn bp_call(rt: &Rt, name: &str, this: &Value, args: &[Value]) -> Res<Value> {
    let f = getp(rt, &bp(rt), name);
    rt.call(&f, this, args)
}

fn plain(rt: &Rt) -> Value {
    Value::Object(rt.new_object())
}

fn object_with_to_string(rt: &Rt) -> Value {
    fn to_q(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
        Ok(Value::str("q"))
    }
    let o = rt.new_object();
    let f = rt.new_native_function(to_q, "toString", 0, Vec::new());
    ok(rt.define_data(o, PropKey::from_str("toString"), Value::Object(f), Flags::HIDDEN));
    Value::Object(o)
}

#[test]
fn boolean_call_is_truthiness() {
    let rt = Rt::new();
    for falsy in [Value::Undefined, Value::Null, Value::Bool(false), n(0.0), n(-0.0), n(f64::NAN), s("")] {
        assert_eq!(ok(call_boolean(&rt, std::slice::from_ref(&falsy))), Value::Bool(false), "Boolean({falsy:?})");
    }
    assert_eq!(ok(call_boolean(&rt, &[])), Value::Bool(false), "Boolean() is false");
    for truthy in [Value::Bool(true), n(1.0), n(-1.0), n(f64::INFINITY), s("0"), s("false"), s("x"), plain(&rt), object_with_to_string(&rt)] {
        assert_eq!(ok(call_boolean(&rt, std::slice::from_ref(&truthy))), Value::Bool(true), "Boolean({truthy:?})");
    }
    // a symbol is truthy
    let sym = Value::Symbol(rt.new_symbol(None));
    assert_eq!(ok(call_boolean(&rt, &[sym])), Value::Bool(true));
}

#[test]
fn new_boolean_makes_a_wrapper_with_boolean_prototype() {
    let rt = Rt::new();
    let w = new_boolean(&rt, &[n(0.0)]);
    let o = w.as_object().expect("an object");
    assert_eq!(rt.class_of(o), ObjClass::Wrapper);
    assert_eq!(rt.get_prototype_of(o), Some(rt.intrinsic(Intrinsic::BooleanPrototype)));
    // the wrapper is an object, so it is truthy even when it holds false
    assert_eq!(rt.typeof_(&w), "object");
    assert!(w.to_boolean());
    assert_eq!(ok(bp_call(&rt, "valueOf", &w, &[])), Value::Bool(false));
    assert_eq!(ok(bp_call(&rt, "toString", &w, &[])), s("false"));
    let t = new_boolean(&rt, &[s("nonempty")]);
    assert_eq!(ok(bp_call(&rt, "valueOf", &t, &[])), Value::Bool(true));
    // no argument: false
    let none = new_boolean(&rt, &[]);
    assert_eq!(ok(bp_call(&rt, "valueOf", &none, &[])), Value::Bool(false));
    // two wrappers are distinct objects
    assert_ne!(new_boolean(&rt, &[]), new_boolean(&rt, &[]));
}

#[test]
fn boolean_prototype_is_a_false_boolean_wrapper() {
    let rt = Rt::new();
    assert_eq!(rt.class_of(rt.intrinsic(Intrinsic::BooleanPrototype)), ObjClass::Wrapper);
    assert_eq!(ok(bp_call(&rt, "valueOf", &bp(&rt), &[])), Value::Bool(false));
    assert_eq!(ok(bp_call(&rt, "toString", &bp(&rt), &[])), s("false"));
    assert_eq!(rt.get_prototype_of(rt.intrinsic(Intrinsic::BooleanPrototype)), Some(rt.intrinsic(Intrinsic::ObjectPrototype)));
    // Object.prototype.toString sees the Boolean wrapper
    let to_string = getp(&rt, &Value::Object(rt.intrinsic(Intrinsic::ObjectPrototype)), "toString");
    assert_eq!(ok(rt.call(&to_string, &bp(&rt), &[])), s("[object Boolean]"));
}

#[test]
fn to_string_and_value_of_accept_primitives_and_wrappers() {
    let rt = Rt::new();
    assert_eq!(ok(bp_call(&rt, "toString", &Value::Bool(true), &[])), s("true"));
    assert_eq!(ok(bp_call(&rt, "toString", &Value::Bool(false), &[])), s("false"));
    assert_eq!(ok(bp_call(&rt, "valueOf", &Value::Bool(true), &[])), Value::Bool(true));
    let wrapped_true = new_boolean(&rt, &[Value::Bool(true)]);
    assert_eq!(ok(bp_call(&rt, "toString", &wrapped_true, &[])), s("true"));
    assert_eq!(ok(bp_call(&rt, "valueOf", &wrapped_true, &[])), Value::Bool(true));
}

#[test]
fn to_string_and_value_of_reject_other_receivers() {
    let rt = Rt::new();
    for bad in [n(1.0), s("true"), Value::Undefined, Value::Null, plain(&rt), n(0.0)] {
        assert_eq!(thrown_name(&rt, bp_call(&rt, "toString", &bad, &[])), "TypeError", "toString on {bad:?}");
        assert_eq!(thrown_name(&rt, bp_call(&rt, "valueOf", &bad, &[])), "TypeError", "valueOf on {bad:?}");
    }
}

#[test]
fn boolean_constructor_attributes() {
    let rt = Rt::new();
    let b = gl(&rt, "Boolean");
    assert!(rt.is_callable(&b));
    assert!(rt.is_constructor(&b));
    assert_eq!(getp(&rt, &b, "length"), n(1.0));
    assert_eq!(getp(&rt, &b, "name"), s("Boolean"));
    let proto = rt.get_own_property(b.as_object().unwrap(), &PropKey::from_str("prototype")).unwrap().unwrap();
    assert_eq!(proto.flags(), Flags::NONE);
    assert_eq!(getp(&rt, &b, "prototype"), bp(&rt));
    let back = rt.get_own_property(rt.intrinsic(Intrinsic::BooleanPrototype), &PropKey::from_str("constructor")).unwrap().unwrap();
    assert_eq!(back.flags(), Flags::HIDDEN);
    let Prop::Data { value, .. } = back else { panic!("data") };
    assert_eq!(value, b);
    for name in ["toString", "valueOf"] {
        let slot = rt.get_own_property(rt.intrinsic(Intrinsic::BooleanPrototype), &PropKey::from_str(name)).unwrap().unwrap();
        assert_eq!(slot.flags(), Flags::HIDDEN, "Boolean.prototype.{name}");
        let f = getp(&rt, &bp(&rt), name);
        assert_eq!(getp(&rt, &f, "length"), n(0.0));
        assert_eq!(getp(&rt, &f, "name"), s(name));
    }
}

#[test]
fn boolean_construct_defaults_to_the_boolean_prototype() {
    // new.target is Boolean itself here, so GetPrototypeFromConstructor reads Boolean.prototype.
    let rt = Rt::new();
    let w = new_boolean(&rt, &[]);
    assert_eq!(rt.get_prototype_of(w.as_object().unwrap()), Some(rt.intrinsic(Intrinsic::BooleanPrototype)));
}

#[test]
fn kernel_operators_see_boolean_wrappers_correctly() {
    let rt = Rt::new();
    let boolean = gl(&rt, "Boolean");
    let w = new_boolean(&rt, &[n(0.0)]);
    assert!(ok(rt.instance_of(&w, &boolean)));
    assert!(!ok(rt.instance_of(&Value::Bool(false), &boolean)));
    // ToPrimitive(wrapper, number) uses valueOf (false), so ToNumber is 0; the string form uses toString
    assert_eq!(ok(rt.to_number(&w)), 0.0);
    assert_eq!(ok(rt.to_string(&new_boolean(&rt, &[Value::Bool(true)]))).to_rust_string_lossy(), "true");
    assert_eq!(ok(rt.to_string(&w)).to_rust_string_lossy(), "false");
    assert_eq!(ok(rt.add(&new_boolean(&rt, &[Value::Bool(true)]), &n(1.0))), n(2.0), "valueOf gives true, then 1 + 1");
    assert_eq!(ok(rt.to_primitive(&w, Hint::Default)), Value::Bool(false));
    assert!(w.to_boolean());
    assert_eq!(rt.typeof_(&w), "object");
    assert_eq!(rt.typeof_(&Value::Bool(false)), "boolean");
    // ToObject of a boolean primitive: a wrapper with Boolean.prototype
    let boxed = rt.to_object(&Value::Bool(true)).unwrap();
    assert_eq!(rt.get_prototype_of(boxed), Some(rt.intrinsic(Intrinsic::BooleanPrototype)));
    assert_eq!(ok(bp_call(&rt, "valueOf", &Value::Object(boxed), &[])), Value::Bool(true));
    // boolean equality with loose_eq: true == 1 and false == 0 through ToNumber
    assert_eq!(ok(rt.loose_eq(&Value::Bool(true), &n(1.0))), Value::Bool(true));
    assert_eq!(ok(rt.loose_eq(&Value::Bool(false), &s(""))), Value::Bool(true));
}

//! WP-B1: global value properties (undefined, NaN, Infinity), globalThis and the global functions parseInt, parseFloat,
//! isNaN and isFinite (ECMA-262 19.1, 19.2). Expected values follow the spec and were checked against Node 26 (V8).

use vn_value::{Call, Flags, Intrinsic, Prop, PropKey, Res, Rt, Throw, Value};

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

fn call_global(rt: &Rt, name: &str, args: &[Value]) -> Res<Value> {
    let f = gl(rt, name);
    rt.call(&f, &Value::Undefined, args)
}

fn num(rt: &Rt, name: &str, args: &[Value]) -> f64 {
    match ok(call_global(rt, name, args)) {
        Value::Number(x) => x,
        other => panic!("{name} returned {other:?}, expected a number"),
    }
}

fn bits_eq(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits()
}

fn named_fn_text(rt: &Rt, v: &Value) -> String {
    match getp(rt, v, "name") {
        Value::String(s) => s.to_rust_string_lossy(),
        _ => String::from("<none>"),
    }
}

#[test]
fn value_properties_are_read_only_permanent_and_correct() {
    let rt = Rt::new();
    let g = rt.intrinsic(Intrinsic::Global);
    for name in ["undefined", "NaN", "Infinity"] {
        let slot = rt.get_own_property(g, &PropKey::from_str(name)).unwrap().unwrap();
        assert_eq!(slot.flags(), Flags::NONE, "{name} is non-writable, non-enumerable, non-configurable");
    }
    assert_eq!(gl(&rt, "undefined"), Value::Undefined);
    assert!(matches!(gl(&rt, "NaN"), Value::Number(x) if x.is_nan()));
    assert_eq!(gl(&rt, "Infinity"), n(f64::INFINITY));
    // reached through globalThis as well
    assert_eq!(getp(&rt, &gl(&rt, "globalThis"), "Infinity"), n(f64::INFINITY));
}

#[test]
fn global_this_is_the_global_object_with_hidden_attributes() {
    let rt = Rt::new();
    let g = rt.intrinsic(Intrinsic::Global);
    assert_eq!(gl(&rt, "globalThis"), Value::Object(g));
    let slot = rt.get_own_property(g, &PropKey::from_str("globalThis")).unwrap().unwrap();
    assert_eq!(slot.flags(), Flags::HIDDEN, "writable, non-enumerable, configurable");
    assert!(matches!(slot, Prop::Data { .. }));
}

#[test]
fn parse_int_follows_the_spec_and_v8() {
    let rt = Rt::new();
    assert_eq!(num(&rt, "parseInt", &[s("  0x1F")]), 31.0);
    assert_eq!(num(&rt, "parseInt", &[s("0x1F"), n(16.0)]), 31.0);
    assert_eq!(num(&rt, "parseInt", &[s("0x1F"), n(10.0)]), 0.0, "radix 10 stops at x");
    assert_eq!(num(&rt, "parseInt", &[s("08")]), 8.0, "no octal in ES5 and later");
    assert_eq!(num(&rt, "parseInt", &[s("1e3")]), 1.0);
    assert!(bits_eq(num(&rt, "parseInt", &[s("-0")]), -0.0));
    assert!(num(&rt, "parseInt", &[s("123"), n(1.0)]).is_nan(), "radix 1 is out of range");
    assert!(num(&rt, "parseInt", &[s("123"), n(37.0)]).is_nan(), "radix 37 is out of range");
    assert_eq!(num(&rt, "parseInt", &[s("11"), n(2.0)]), 3.0);
    assert_eq!(num(&rt, "parseInt", &[s("z"), n(36.0)]), 35.0);
    assert_eq!(num(&rt, "parseInt", &[s(" +12abc")]), 12.0);
    assert_eq!(num(&rt, "parseInt", &[s("-17")]), -17.0);
    assert!(num(&rt, "parseInt", &[s("")]).is_nan());
    assert!(num(&rt, "parseInt", &[s("0x")]).is_nan());
    assert!(num(&rt, "parseInt", &[]).is_nan(), "parseInt() is parseInt(undefined)");
    assert_eq!(num(&rt, "parseInt", &[Value::Null, n(36.0)]), 1112745.0, "\"null\" in base 36");
    assert_eq!(num(&rt, "parseInt", &[s("12"), n(0.0)]), 12.0, "radix 0 means 10");
    assert_eq!(num(&rt, "parseInt", &[s("12"), Value::Undefined]), 12.0, "undefined radix is 0");
    assert_eq!(num(&rt, "parseInt", &[s("0x10"), n(0.0)]), 16.0, "radix 0 allows the 0x prefix");
    assert_eq!(num(&rt, "parseInt", &[s("1010"), n(2.0)]), 10.0);
    assert_eq!(num(&rt, "parseInt", &[s("ff"), n(16.0)]), 255.0);
    assert!(num(&rt, "parseInt", &[s("-")]).is_nan());
    // a radix that is not an integer is truncated by ToInt32
    assert_eq!(num(&rt, "parseInt", &[s("11"), n(2.9)]), 3.0);
}

#[test]
fn parse_int_converts_the_string_before_the_radix() {
    // ToString(string) happens before ToInt32(radix): the first thrown error wins.
    let rt = Rt::new();
    fn throw_a(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
        Err(rt.throw_type_error("A"))
    }
    fn throw_b(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
        Err(rt.throw_type_error("B"))
    }
    let bad_string = rt.new_object();
    let f = rt.new_native_function(throw_a, "toString", 0, Vec::new());
    ok(rt.define_data(bad_string, PropKey::from_str("toString"), Value::Object(f), Flags::HIDDEN));
    ok(rt.define_data(bad_string, PropKey::from_str("valueOf"), Value::Object(f), Flags::HIDDEN));
    let bad_radix = rt.new_object();
    let g = rt.new_native_function(throw_b, "valueOf", 0, Vec::new());
    ok(rt.define_data(bad_radix, PropKey::from_str("valueOf"), Value::Object(g), Flags::HIDDEN));
    ok(rt.define_data(bad_radix, PropKey::from_str("toString"), Value::Object(g), Flags::HIDDEN));
    let r = call_global(&rt, "parseInt", &[Value::Object(bad_string), Value::Object(bad_radix)]);
    assert_eq!(ok_error_message(&rt, r), "A");
}

/// The `message` of the thrown value (the test helper errors carry their text as the message).
fn ok_error_message(rt: &Rt, r: Res<Value>) -> String {
    match r {
        Err(Throw(e)) => match rt.get_str(&e, "message") {
            Ok(Value::String(s)) => s.to_rust_string_lossy(),
            _ => String::from("<none>"),
        },
        Ok(v) => panic!("expected a throw, got {v:?}"),
    }
}

#[test]
fn parse_float_follows_the_spec_and_v8() {
    let rt = Rt::new();
    assert_eq!(num(&rt, "parseFloat", &[s("  3.14abc")]), 3.14);
    assert_eq!(num(&rt, "parseFloat", &[s(".5")]), 0.5);
    assert_eq!(num(&rt, "parseFloat", &[s("-.5e2x")]), -50.0);
    assert_eq!(num(&rt, "parseFloat", &[s("Infinityx")]), f64::INFINITY);
    assert_eq!(num(&rt, "parseFloat", &[s("-Infinity")]), f64::NEG_INFINITY);
    assert!(bits_eq(num(&rt, "parseFloat", &[s("-0")]), -0.0));
    assert!(num(&rt, "parseFloat", &[s("e5")]).is_nan());
    assert!(num(&rt, "parseFloat", &[s("")]).is_nan());
    assert_eq!(num(&rt, "parseFloat", &[s("1e")]), 1.0, "an incomplete exponent is ignored");
    assert_eq!(num(&rt, "parseFloat", &[s("0x10")]), 0.0, "parseFloat has no hex form");
    assert_eq!(num(&rt, "parseFloat", &[s("+1.5")]), 1.5);
    assert!(num(&rt, "parseFloat", &[Value::Null]).is_nan(), "\"null\" is not a decimal literal");
    assert!(num(&rt, "parseFloat", &[]).is_nan());
    assert_eq!(num(&rt, "parseFloat", &[n(12.5)]), 12.5, "numbers go through ToString");
    assert_eq!(num(&rt, "parseFloat", &[s("1.7976931348623157e309")]), f64::INFINITY);
}

#[test]
fn is_nan_coerces_with_to_number() {
    let rt = Rt::new();
    let check = |rt: &Rt, v: Value| ok(call_global(rt, "isNaN", &[v]));
    assert_eq!(check(&rt, s("abc")), Value::Bool(true));
    assert_eq!(check(&rt, s("")), Value::Bool(false), "the empty string is 0");
    assert_eq!(check(&rt, Value::Undefined), Value::Bool(true));
    assert_eq!(check(&rt, Value::Null), Value::Bool(false));
    assert_eq!(check(&rt, s(" 12 ")), Value::Bool(false));
    assert_eq!(check(&rt, Value::Bool(true)), Value::Bool(false));
    assert_eq!(check(&rt, n(f64::NAN)), Value::Bool(true));
    assert_eq!(check(&rt, n(f64::INFINITY)), Value::Bool(false));
    assert_eq!(check(&rt, s("Infinity")), Value::Bool(false));
    // an object goes through valueOf
    let o = rt.new_object();
    fn value_one(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
        Ok(Value::Number(1.0))
    }
    let f = rt.new_native_function(value_one, "valueOf", 0, Vec::new());
    ok(rt.define_data(o, PropKey::from_str("valueOf"), Value::Object(f), Flags::HIDDEN));
    assert_eq!(check(&rt, Value::Object(o)), Value::Bool(false));
    // a symbol cannot be converted: TypeError
    let sym = Value::Symbol(rt.new_symbol(None));
    assert_eq!(thrown_name(&rt, call_global(&rt, "isNaN", &[sym])), "TypeError");
}

#[test]
fn is_finite_coerces_with_to_number() {
    let rt = Rt::new();
    let check = |rt: &Rt, v: Value| ok(call_global(rt, "isFinite", &[v]));
    assert_eq!(check(&rt, s("12")), Value::Bool(true));
    assert_eq!(check(&rt, n(f64::INFINITY)), Value::Bool(false));
    assert_eq!(check(&rt, n(f64::NEG_INFINITY)), Value::Bool(false));
    assert_eq!(check(&rt, n(f64::NAN)), Value::Bool(false));
    assert_eq!(check(&rt, s("Infinity")), Value::Bool(false));
    assert_eq!(check(&rt, Value::Null), Value::Bool(true), "null is 0");
    assert_eq!(check(&rt, Value::Undefined), Value::Bool(false));
    assert_eq!(check(&rt, Value::Bool(false)), Value::Bool(true));
    assert_eq!(check(&rt, s("")), Value::Bool(true));
    assert_eq!(check(&rt, n(1e308)), Value::Bool(true));
    assert_eq!(thrown_name(&rt, call_global(&rt, "isFinite", &[Value::Symbol(rt.new_symbol(None))])), "TypeError");
}

#[test]
fn global_functions_have_spec_lengths_names_and_attributes() {
    let rt = Rt::new();
    let g = rt.intrinsic(Intrinsic::Global);
    for (name, len) in [("parseInt", 2.0), ("parseFloat", 1.0), ("isNaN", 1.0), ("isFinite", 1.0)] {
        let f = gl(&rt, name);
        assert!(rt.is_callable(&f));
        assert!(!rt.is_constructor(&f), "{name} is not a constructor");
        assert_eq!(getp(&rt, &f, "length"), n(len), "{name}.length");
        assert_eq!(named_fn_text(&rt, &f), name, "{name}.name");
        let slot = rt.get_own_property(g, &PropKey::from_str(name)).unwrap().unwrap();
        assert_eq!(slot.flags(), Flags::HIDDEN, "{name} is writable, non-enumerable, configurable");
    }
}

#[test]
fn parse_functions_are_registered_for_number() {
    // WP-B3 makes Number.parseInt and Number.parseFloat the same objects as the globals.
    let rt = Rt::new();
    assert_eq!(rt.builtin("parseInt"), gl(&rt, "parseInt").as_object());
    assert_eq!(rt.builtin("parseFloat"), gl(&rt, "parseFloat").as_object());
}

#[test]
fn only_the_listed_global_functions_exist() {
    let rt = Rt::new();
    let g = rt.intrinsic(Intrinsic::Global);
    for absent in ["eval", "encodeURI", "encodeURIComponent", "decodeURI", "decodeURIComponent", "escape", "unescape", "Function", "setTimeout"] {
        assert!(rt.get_own_property(g, &PropKey::from_str(absent)).unwrap().is_none(), "{absent} must be absent");
    }
    for present in ["Object", "Symbol", "Boolean", "globalThis", "undefined", "NaN", "Infinity"] {
        assert!(rt.get_own_property(g, &PropKey::from_str(present)).unwrap().is_some(), "{present} must exist");
    }
}

#[test]
fn parse_functions_still_work_after_a_collection() {
    let rt = Rt::new();
    rt.collect();
    assert_eq!(num(&rt, "parseInt", &[s("42")]), 42.0);
    assert_eq!(num(&rt, "parseFloat", &[s("4.5")]), 4.5);
}

//! WP-L2b conversions (ECMA-262 7.1, 13.5.3): ToBoolean, ToNumeric, ToString of objects, ToPropertyKey,
//! ToIntegerOrInfinity, ToLength, ToIndex, ToInt32, ToUint32, `typeof`, and the Object.prototype.toString the
//! conversions reach for. Expected values are written from the spec and cross-checked with Node 26 (V8). The
//! number and string tables are covered by ops_corpus.rs.

use std::cell::RefCell;

use vn_value::{same_value, Call, Flags, PropKey, Res, Rt, Value};

const MAX_SAFE: f64 = 9_007_199_254_740_991.0;

fn ok<T>(r: Res<T>) -> T {
    match r {
        Ok(v) => v,
        Err(t) => panic!("unexpected JS throw: {:?}", t.0),
    }
}

/// The name of a thrown error (`TypeError`, ...) and its message.
fn thrown(rt: &Rt, t: &vn_value::Throw) -> (String, String) {
    let name = match rt.get_str(&t.0, "name") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => String::from("<none>"),
    };
    let msg = match rt.get_str(&t.0, "message") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => String::from("<none>"),
    };
    (name, msg)
}

fn assert_throws<T: std::fmt::Debug>(rt: &Rt, r: Res<T>, name: &str, msg: &str) {
    match r {
        Err(t) => assert_eq!(thrown(rt, &t), (name.to_string(), msg.to_string())),
        Ok(v) => panic!("expected {name}, got {v:?}"),
    }
}

// Objects whose valueOf and toString record their calls in LOG, so tests can check the conversion order.
thread_local! {
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn take_log() -> Vec<String> {
    LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

fn name_of(rt: &Rt, this: &Value) -> String {
    match rt.get_str(this, "name") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => String::from("?"),
    }
}

fn value_of(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    LOG.with(|l| l.borrow_mut().push(format!("{}.valueOf", name_of(rt, c.this))));
    if rt.get_str(c.this, "throws")?.to_boolean() {
        return Err(rt.throw_range_error("boom"));
    }
    rt.get_str(c.this, "value")
}

fn to_string_method(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    LOG.with(|l| l.borrow_mut().push(format!("{}.toString", name_of(rt, c.this))));
    rt.get_str(c.this, "text")
}

/// An object with its own valueOf (returns `value`) and toString (returns `text`). `throws` makes valueOf throw.
fn conv_obj(rt: &Rt, name: &str, value: Value, text: &str, throws: bool) -> Value {
    let o = rt.new_object();
    let f_value_of = rt.new_native_function(value_of, "valueOf", 0, vec![]);
    let f_to_string = rt.new_native_function(to_string_method, "toString", 0, vec![]);
    let fields = [
        ("name", Value::str(name)),
        ("value", value),
        ("text", Value::str(text)),
        ("throws", Value::Bool(throws)),
        ("valueOf", Value::Object(f_value_of)),
        ("toString", Value::Object(f_to_string)),
    ];
    for (k, v) in fields {
        assert!(ok(rt.define_data(o, PropKey::from_str(k), v, Flags::ALL)));
    }
    Value::Object(o)
}

#[test]
fn to_boolean_follows_the_spec_table() {
    let rt = Rt::new();
    let obj = Value::Object(rt.new_object());
    let sym = Value::Symbol(rt.new_symbol(None));
    let table = [
        (Value::Undefined, false),
        (Value::Null, false),
        (Value::Bool(false), false),
        (Value::Number(0.0), false),
        (Value::Number(-0.0), false),
        (Value::Number(f64::NAN), false),
        (Value::str(""), false),
        (Value::Number(1.0), true),
        (Value::str("0"), true),
        (Value::str("false"), true),
        (sym, true),
        (obj, true),
    ];
    for (v, want) in table {
        assert_eq!(v.to_boolean(), want, "ToBoolean({v:?})");
    }
}

#[test]
fn to_numeric_is_to_number_and_a_symbol_throws() {
    let rt = Rt::new();
    let o = conv_obj(&rt, "a", Value::Number(3.0), "t", false);
    assert!(same_value(&Value::Number(ok(rt.to_numeric(&o))), &Value::Number(3.0)));
    assert!(same_value(&Value::Number(ok(rt.to_numeric(&Value::str(" 0x1F ")))), &Value::Number(31.0)));
    let sym = Value::Symbol(rt.new_symbol(None));
    assert_throws(&rt, rt.to_numeric(&sym), "TypeError", "Cannot convert a Symbol value to a number");
}

#[test]
fn to_string_of_primitives_matches_number_to_string_and_the_keywords() {
    let rt = Rt::new();
    let table = [
        (Value::Number(1e21), "1e+21"),
        (Value::Number(-0.0), "0"),
        (Value::Number(f64::NAN), "NaN"),
        (Value::Number(f64::NEG_INFINITY), "-Infinity"),
        (Value::Bool(true), "true"),
        (Value::Undefined, "undefined"),
        (Value::Null, "null"),
    ];
    for (v, want) in table {
        assert_eq!(ok(rt.to_string(&v)).to_rust_string_lossy(), want, "ToString({v:?})");
    }
}

#[test]
fn to_string_of_a_symbol_is_a_type_error() {
    let rt = Rt::new();
    let sym = Value::Symbol(rt.new_symbol(None));
    assert_throws(&rt, rt.to_string(&sym), "TypeError", "Cannot convert a Symbol value to a string");
}

#[test]
fn to_string_of_an_object_calls_to_string_first_and_stops_at_a_primitive() {
    let rt = Rt::new();
    let o = conv_obj(&rt, "a", Value::Number(1.0), "text", false);
    assert_eq!(ok(rt.to_string(&o)).to_rust_string_lossy(), "text");
    assert_eq!(take_log(), ["a.toString"]);
}

#[test]
fn to_string_of_an_error_goes_through_method_lookup() {
    // Error.prototype.toString is found by the normal lookup, so ToString works without any Error WP code.
    let rt = Rt::new();
    let e = rt.make_error(vn_value::ErrorKind::Type, "m");
    assert_eq!(ok(rt.to_string(&e)).to_rust_string_lossy(), "TypeError: m");
    let empty = rt.make_error(vn_value::ErrorKind::Type, "");
    assert_eq!(ok(rt.to_string(&empty)).to_rust_string_lossy(), "TypeError");
}

#[test]
fn to_string_of_an_object_with_no_conversion_methods_is_a_type_error() {
    let rt = Rt::new();
    let bare = Value::Object(rt.new_object_with_proto(None));
    assert_throws(&rt, rt.to_string(&bare), "TypeError", "Cannot convert object to primitive value");
}

#[test]
fn to_property_key_canonicalises_numbers_symbols_and_objects() {
    let rt = Rt::new();
    let sym = rt.new_symbol(None);
    let key = |v: &Value| ok(rt.to_property_key(v));
    assert_eq!(key(&Value::Number(1.0)), PropKey::Index(1));
    assert_eq!(key(&Value::Number(-0.0)), PropKey::Index(0), "-0 is the key \"0\"");
    assert_eq!(key(&Value::str("01")), PropKey::from_str("01"), "\"01\" is not an array index");
    assert_eq!(key(&Value::Number(1.5)), PropKey::from_str("1.5"));
    assert_eq!(key(&Value::Bool(true)), PropKey::from_str("true"));
    assert_eq!(key(&Value::Undefined), PropKey::from_str("undefined"));
    assert_eq!(key(&Value::Symbol(sym)), PropKey::Sym(sym));
    let o = conv_obj(&rt, "k", Value::Number(9.0), "k", false);
    assert_eq!(key(&o), PropKey::from_str("k"));
}

#[test]
fn to_integer_or_infinity_truncates_and_never_returns_negative_zero() {
    let rt = Rt::new();
    let table: [(Value, f64); 9] = [
        (Value::Number(f64::NAN), 0.0),
        (Value::Number(-0.5), 0.0),
        (Value::Number(1.9), 1.0),
        (Value::Number(-1.9), -1.0),
        (Value::Number(f64::INFINITY), f64::INFINITY),
        (Value::Number(f64::NEG_INFINITY), f64::NEG_INFINITY),
        (Value::str("3"), 3.0),
        (Value::Null, 0.0),
        (Value::Undefined, 0.0),
    ];
    for (v, want) in table {
        let got = ok(rt.to_integer_or_infinity(&v));
        assert!(same_value(&Value::Number(got), &Value::Number(want)), "ToIntegerOrInfinity({v:?}) = {got:?}");
    }
}

#[test]
fn to_length_clamps_into_zero_to_two_to_the_53_minus_one() {
    let rt = Rt::new();
    assert_eq!(ok(rt.to_length(&Value::Number(-5.0))), 0.0);
    assert_eq!(ok(rt.to_length(&Value::Number(f64::NAN))), 0.0);
    assert_eq!(ok(rt.to_length(&Value::Number(9_007_199_254_740_994.0))), MAX_SAFE);
    assert_eq!(ok(rt.to_length(&Value::Number(f64::INFINITY))), MAX_SAFE);
    assert_eq!(ok(rt.to_length(&Value::str("7.9"))), 7.0);
}

#[test]
fn to_index_accepts_zero_to_two_to_the_53_minus_one_and_rejects_the_rest() {
    let rt = Rt::new();
    assert_eq!(ok(rt.to_index(&Value::Undefined)), 0.0);
    assert_eq!(ok(rt.to_index(&Value::Number(-0.0))).to_bits(), 0.0f64.to_bits(), "-0 is index 0");
    assert_eq!(ok(rt.to_index(&Value::str("1.9"))), 1.0);
    assert_eq!(ok(rt.to_index(&Value::Number(MAX_SAFE))), MAX_SAFE);
    assert_throws(&rt, rt.to_index(&Value::Number(-1.0)), "RangeError", "Invalid index");
    assert_throws(&rt, rt.to_index(&Value::Number(MAX_SAFE + 1.0)), "RangeError", "Invalid index");
}

#[test]
fn to_int32_and_to_uint32_wrap_modulo_two_to_the_32() {
    let rt = Rt::new();
    let int32 = [
        (Value::Number(2147483648.0), -2147483648),
        (Value::Number(-1.0), -1),
        (Value::Number(4294967301.0), 5),
        (Value::Number(f64::NAN), 0),
        (Value::Number(f64::INFINITY), 0),
        (Value::str("0x10"), 16),
        (Value::Number(-1.9), -1),
    ];
    for (v, want) in int32 {
        assert_eq!(ok(rt.to_int32(&v)), want, "ToInt32({v:?})");
    }
    let uint32 = [
        (Value::Number(-1.0), 4294967295),
        (Value::Number(4294967296.0), 0),
        (Value::Number(3.7), 3),
    ];
    for (v, want) in uint32 {
        assert_eq!(ok(rt.to_uint32(&v)), want, "ToUint32({v:?})");
    }
}

#[test]
fn typeof_names_each_kind_of_value() {
    let rt = Rt::new();
    let f = Value::Object(rt.new_native_function(|_rt: &Rt, _c: &Call<'_>| Ok(Value::Undefined), "f", 0, vec![]));
    let table = [
        (Value::Undefined, "undefined"),
        (Value::Null, "object"),
        (Value::Bool(true), "boolean"),
        (Value::Number(1.0), "number"),
        (Value::str("s"), "string"),
        (Value::Symbol(rt.new_symbol(None)), "symbol"),
        (Value::Object(rt.new_object()), "object"),
        (f, "function"),
    ];
    for (v, want) in table {
        assert_eq!(rt.typeof_(&v), want, "typeof {v:?}");
    }
}

#[test]
fn object_prototype_to_string_reports_builtin_and_to_string_tags() {
    let rt = Rt::new();
    let otos = ok(rt.get_str(&Value::Object(rt.intrinsic(vn_value::Intrinsic::ObjectPrototype)), "toString"));
    let check = |this: Value, want: &str| {
        assert_eq!(ok(rt.call(&otos, &this, &[])), Value::str(want), "toString of {this:?}");
    };
    check(Value::Undefined, "[object Undefined]");
    check(Value::Null, "[object Null]");
    check(Value::Number(1.0), "[object Number]");
    check(Value::str("a"), "[object String]");
    check(Value::Bool(true), "[object Boolean]");
    check(Value::Object(rt.new_array(vec![])), "[object Array]");
    check(Value::Object(rt.new_object()), "[object Object]");
    check(Value::Object(rt.make_error(vn_value::ErrorKind::Error, "x").as_object().unwrap()), "[object Error]");
    let f = rt.new_native_function(|_rt: &Rt, _c: &Call<'_>| Ok(Value::Undefined), "f", 0, vec![]);
    check(Value::Object(f), "[object Function]");
    // A string-valued @@toStringTag wins; a non-string one is ignored (both as in V8).
    let tag = rt.well_known(vn_value::WellKnown::ToStringTag);
    let tagged = rt.new_object();
    assert!(ok(rt.define_data(tagged, PropKey::Sym(tag), Value::str("Custom"), Flags::HIDDEN)));
    check(Value::Object(tagged), "[object Custom]");
    let ignored = rt.new_object();
    assert!(ok(rt.define_data(ignored, PropKey::Sym(tag), Value::Number(5.0), Flags::HIDDEN)));
    check(Value::Object(ignored), "[object Object]");
}

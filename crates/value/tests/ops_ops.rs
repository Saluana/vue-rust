//! WP-L2b operators (ECMA-262 13.5 to 13.15, 7.2): arithmetic, bitwise, relational, equality, `instanceof` and
//! `in`, with objects and symbols (which the V8 corpus cannot express). Expected values are written from the spec
//! and were cross-checked with Node 26 (V8), including the valueOf/toString call order.

use std::cell::RefCell;

use vn_value::{same_value, same_value_zero, strict_eq, Call, Flags, Intrinsic, JsStr, PropKey, Res, Rt, Value};

fn ok<T>(r: Res<T>) -> T {
    match r {
        Ok(v) => v,
        Err(t) => panic!("unexpected JS throw: {:?}", t.0),
    }
}

/// Checks a result exactly: NaN equals NaN, and -0 differs from +0 (SameValue).
fn expect(label: &str, got: Res<Value>, want: Value) {
    match got {
        Ok(v) => assert!(same_value(&v, &want), "{label}: got {v:?}, want {want:?}"),
        Err(t) => panic!("{label}: unexpected throw {:?}", t.0),
    }
}

fn thrown(rt: &Rt, t: &vn_value::Throw) -> (String, String) {
    let text = |k: &str| match rt.get_str(&t.0, k) {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => String::from("<none>"),
    };
    (text("name"), text("message"))
}

fn expect_throw<T: std::fmt::Debug>(rt: &Rt, label: &str, got: Res<T>, name: &str, msg: &str) {
    match got {
        Err(t) => assert_eq!(thrown(rt, &t), (name.to_string(), msg.to_string()), "{label}"),
        Ok(v) => panic!("{label}: expected {name}, got {v:?}"),
    }
}

// Objects whose valueOf and toString record their calls, so tests can check the order of conversions.
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

fn noop(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Undefined)
}

fn s(text: &str) -> Value {
    Value::str(text)
}

fn n(x: f64) -> Value {
    Value::Number(x)
}

#[test]
fn add_concatenates_when_a_primitive_is_a_string_and_adds_otherwise() {
    let rt = Rt::new();
    let cases = [
        (n(1.0), s("2"), s("12")),
        (s("1"), n(2.0), s("12")),
        (Value::Null, n(1.0), n(1.0)),
        (Value::Undefined, n(1.0), n(f64::NAN)),
        (Value::Bool(true), Value::Bool(true), n(2.0)),
        (s("a"), Value::Undefined, s("aundefined")),
        (n(1.0), n(2.0), n(3.0)),
        (s("x"), s("y"), s("xy")),
    ];
    for (a, b, want) in cases {
        expect(&format!("{a:?} + {b:?}"), rt.add(&a, &b), want);
    }
}

#[test]
fn add_uses_the_default_hint_so_value_of_wins_for_objects() {
    let rt = Rt::new();
    let a = conv_obj(&rt, "a", n(5.0), "t", false);
    expect("obj + 1", rt.add(&a, &n(1.0)), n(6.0));
    assert_eq!(take_log(), ["a.valueOf"]);
    expect("obj + ''", rt.add(&a, &s("")), s("5"));
    assert_eq!(take_log(), ["a.valueOf"]);
}

#[test]
fn arithmetic_operators_use_the_number_hint_for_objects() {
    let rt = Rt::new();
    let a = conv_obj(&rt, "a", n(5.0), "t", false);
    expect("obj * 2", rt.mul(&a, &n(2.0)), n(10.0));
    expect("obj - 1", rt.sub(&a, &n(1.0)), n(4.0));
    expect("obj / 1", rt.div(&a, &n(1.0)), n(5.0));
    expect("obj % 3", rt.rem(&a, &n(3.0)), n(2.0));
    expect("obj ** 2", rt.pow(&a, &n(2.0)), n(25.0));
    assert_eq!(take_log(), ["a.valueOf"; 5]);
}

#[test]
fn operands_are_converted_left_to_right_for_every_operator_family() {
    let rt = Rt::new();
    let a = conv_obj(&rt, "a", n(2.0), "ta", false);
    let b = conv_obj(&rt, "b", n(3.0), "tb", false);
    expect("a + b", rt.add(&a, &b), n(5.0));
    assert_eq!(take_log(), ["a.valueOf", "b.valueOf"]);
    expect("a - b", rt.sub(&a, &b), n(-1.0));
    assert_eq!(take_log(), ["a.valueOf", "b.valueOf"]);
    expect("a < b", rt.lt(&a, &b), Value::Bool(true));
    assert_eq!(take_log(), ["a.valueOf", "b.valueOf"]);
    // `a > b` is evaluated as `b < a` with LeftFirst false, which still converts `a` first.
    expect("a > b", rt.gt(&a, &b), Value::Bool(false));
    assert_eq!(take_log(), ["a.valueOf", "b.valueOf"]);
    expect("a <= b", rt.le(&a, &b), Value::Bool(true));
    assert_eq!(take_log(), ["a.valueOf", "b.valueOf"]);
    expect("a >= b", rt.ge(&a, &b), Value::Bool(false));
    assert_eq!(take_log(), ["a.valueOf", "b.valueOf"]);
}

#[test]
fn a_throwing_value_of_stops_the_operator_before_the_right_operand() {
    let rt = Rt::new();
    let a = conv_obj(&rt, "a", n(1.0), "ta", true);
    let b = conv_obj(&rt, "b", n(3.0), "tb", false);
    expect_throw(&rt, "a + b", rt.add(&a, &b), "RangeError", "boom");
    assert_eq!(take_log(), ["a.valueOf"], "the right operand is not converted");
}

#[test]
fn symbol_operands_throw_type_errors_except_in_equality_and_logic() {
    let rt = Rt::new();
    let sym = Value::Symbol(rt.new_symbol(Some(JsStr::from_str("q"))));
    let number_msg = "Cannot convert a Symbol value to a number";
    let string_msg = "Cannot convert a Symbol value to a string";
    expect_throw(&rt, "sym + 1", rt.add(&sym, &n(1.0)), "TypeError", number_msg);
    expect_throw(&rt, "1 + sym", rt.add(&n(1.0), &sym), "TypeError", number_msg);
    expect_throw(&rt, "sym + ''", rt.add(&sym, &s("")), "TypeError", string_msg);
    expect_throw(&rt, "'' + sym", rt.add(&s(""), &sym), "TypeError", string_msg);
    expect_throw(&rt, "sym - 1", rt.sub(&sym, &n(1.0)), "TypeError", number_msg);
    expect_throw(&rt, "sym < 1", rt.lt(&sym, &n(1.0)), "TypeError", number_msg);
    expect_throw(&rt, "sym & 1", rt.bit_and(&sym, &n(1.0)), "TypeError", number_msg);
    expect_throw(&rt, "-sym", rt.neg(&sym), "TypeError", number_msg);
    expect_throw(&rt, "+sym", rt.pos(&sym), "TypeError", number_msg);
    expect_throw(&rt, "~sym", rt.bit_not(&sym), "TypeError", number_msg);
    expect_throw(&rt, "++sym", rt.inc(&sym), "TypeError", number_msg);
    // No conversion is needed for these, so they do not throw.
    assert!(!rt.not_(&sym));
    expect("sym == 1", rt.loose_eq(&sym, &n(1.0)), Value::Bool(false));
    expect("sym == 'q'", rt.loose_eq(&sym, &s("q")), Value::Bool(false));
    expect("sym == sym", rt.loose_eq(&sym, &sym), Value::Bool(true));
    assert!(strict_eq(&sym, &sym));
    assert!(!strict_eq(&sym, &Value::Symbol(rt.new_symbol(None))));
}

#[test]
fn loose_equality_converts_an_object_with_the_default_hint() {
    let rt = Rt::new();
    let a = conv_obj(&rt, "a", n(5.0), "t", false);
    expect("obj == 5", rt.loose_eq(&a, &n(5.0)), Value::Bool(true));
    assert_eq!(take_log(), ["a.valueOf"]);
    expect("obj == '5'", rt.loose_eq(&a, &s("5")), Value::Bool(true));
    assert_eq!(take_log(), ["a.valueOf"]);
    // true becomes 1 first, and the object then converts to 5.
    expect("true == obj", rt.loose_eq(&Value::Bool(true), &a), Value::Bool(false));
    assert_eq!(take_log(), ["a.valueOf"]);
    // No conversion for null, and for the same object.
    expect("obj == null", rt.loose_eq(&a, &Value::Null), Value::Bool(false));
    expect("obj == obj", rt.loose_eq(&a, &a), Value::Bool(true));
    assert!(take_log().is_empty());
    // valueOf returns an object, so toString supplies the primitive.
    let t = conv_obj(&rt, "t", Value::Object(rt.new_object()), "x", false);
    expect("obj == 'x' via toString", rt.loose_eq(&t, &s("x")), Value::Bool(true));
    assert_eq!(take_log(), ["t.valueOf", "t.toString"]);
    // Two different objects are never loosely equal.
    let other = conv_obj(&rt, "a", n(5.0), "t", false);
    expect("obj == other", rt.loose_eq(&a, &other), Value::Bool(false));
    assert!(take_log().is_empty());
}

#[test]
fn strict_equality_and_the_two_same_value_relations_differ_on_zero_and_nan() {
    let rt = Rt::new();
    let obj = rt.new_object();
    assert!(strict_eq(&n(0.0), &n(-0.0)));
    assert!(!strict_eq(&n(f64::NAN), &n(f64::NAN)));
    assert!(strict_eq(&s("a"), &s("a")), "equal content, different allocations");
    assert!(strict_eq(&Value::Object(obj), &Value::Object(obj)));
    assert!(!strict_eq(&Value::Object(obj), &Value::Object(rt.new_object())));
    assert!(!strict_eq(&Value::Null, &Value::Undefined));
    assert!(same_value(&n(f64::NAN), &n(f64::NAN)));
    assert!(!same_value(&n(0.0), &n(-0.0)));
    assert!(same_value_zero(&n(0.0), &n(-0.0)));
    assert!(same_value_zero(&n(f64::NAN), &n(f64::NAN)));
    assert!(!same_value_zero(&n(1.0), &s("1")));
}

#[test]
fn relational_comparison_of_strings_uses_code_units() {
    let rt = Rt::new();
    let lone = Value::String(JsStr::from_units(&[0xD800]));
    let top = Value::String(JsStr::from_units(&[0xFFFF]));
    expect("'B' < 'a'", rt.lt(&s("B"), &s("a")), Value::Bool(true));
    expect("'10' < '9'", rt.lt(&s("10"), &s("9")), Value::Bool(true));
    expect("'10' < 9", rt.lt(&s("10"), &n(9.0)), Value::Bool(false));
    expect("'' < 'a'", rt.lt(&s(""), &s("a")), Value::Bool(true));
    expect("lone surrogate < U+FFFF", rt.lt(&lone, &top), Value::Bool(true));
    expect("'a' <= 'a'", rt.le(&s("a"), &s("a")), Value::Bool(true));
    expect("'b' >= 'a'", rt.ge(&s("b"), &s("a")), Value::Bool(true));
}

#[test]
fn relational_operators_are_false_when_a_nan_is_involved() {
    let rt = Rt::new();
    let nan = n(f64::NAN);
    expect("NaN < 1", rt.lt(&nan, &n(1.0)), Value::Bool(false));
    expect("NaN > 1", rt.gt(&nan, &n(1.0)), Value::Bool(false));
    expect("NaN <= NaN", rt.le(&nan, &nan), Value::Bool(false));
    expect("NaN >= NaN", rt.ge(&nan, &nan), Value::Bool(false));
    expect("'x' < NaN", rt.lt(&s("x"), &nan), Value::Bool(false));
}

#[test]
fn instance_of_walks_the_prototype_chain_of_a_constructor() {
    let rt = Rt::new();
    let f = rt.new_native_function(noop, "F", 0, vec![]);
    let arr_proto = rt.intrinsic(Intrinsic::ArrayPrototype);
    assert!(ok(rt.define_data(f, PropKey::from_str("prototype"), Value::Object(arr_proto), Flags::NONE)));
    let array = Value::Object(rt.new_array(vec![]));
    assert!(ok(rt.instance_of(&array, &Value::Object(f))), "[] instanceof F");
    assert!(!ok(rt.instance_of(&Value::Object(rt.new_object()), &Value::Object(f))));
    assert!(!ok(rt.instance_of(&n(1.0), &Value::Object(f))), "a primitive is never an instance");
    // Errors: TypeError is an Error, but not the other way round (V8 agrees).
    let type_error = rt.make_error(vn_value::ErrorKind::Type, "x");
    let error = rt.make_error(vn_value::ErrorKind::Error, "x");
    let error_ctor = Value::Object(rt.intrinsic(Intrinsic::ErrorCtor));
    let type_error_ctor = Value::Object(rt.intrinsic(Intrinsic::TypeErrorCtor));
    assert!(ok(rt.instance_of(&type_error, &error_ctor)));
    assert!(ok(rt.instance_of(&type_error, &type_error_ctor)));
    assert!(!ok(rt.instance_of(&error, &type_error_ctor)));
}

#[test]
fn instance_of_rejects_a_right_side_that_is_not_a_function_object() {
    let rt = Rt::new();
    let obj = Value::Object(rt.new_object());
    expect_throw(
        &rt,
        "{} instanceof {}",
        rt.instance_of(&obj, &obj),
        "TypeError",
        "Right-hand side of 'instanceof' is not callable",
    );
    expect_throw(
        &rt,
        "{} instanceof 1",
        rt.instance_of(&obj, &n(1.0)),
        "TypeError",
        "Right-hand side of 'instanceof' is not an object",
    );
    // A non-object `prototype` throws only when the left side is an object (V8 does the same).
    let f = rt.new_native_function(noop, "F", 0, vec![]);
    assert!(ok(rt.define_data(f, PropKey::from_str("prototype"), n(1.0), Flags::NONE)));
    assert!(!ok(rt.instance_of(&n(1.0), &Value::Object(f))));
    expect_throw(
        &rt,
        "{} instanceof F (prototype 1)",
        rt.instance_of(&obj, &Value::Object(f)),
        "TypeError",
        "Function has non-object prototype '1' in instanceof check",
    );
    let g = rt.new_native_function(noop, "G", 0, vec![]);
    expect_throw(
        &rt,
        "{} instanceof G (no prototype)",
        rt.instance_of(&obj, &Value::Object(g)),
        "TypeError",
        "Function has non-object prototype 'undefined' in instanceof check",
    );
}

#[test]
fn in_operator_looks_up_own_and_inherited_keys() {
    let rt = Rt::new();
    let array = Value::Object(rt.new_array(vec![s("a")]));
    assert!(ok(rt.in_operator(&s("length"), &array)), "'length' in []");
    assert!(ok(rt.in_operator(&s("0"), &array)));
    assert!(!ok(rt.in_operator(&n(1.0), &array)));
    let obj = Value::Object(rt.new_object());
    assert!(ok(rt.in_operator(&s("toString"), &obj)), "inherited from Object.prototype");
    assert!(!ok(rt.in_operator(&s("x"), &obj)));
    let sym = rt.new_symbol(None);
    assert!(ok(rt.define_data(obj.as_object().unwrap(), PropKey::Sym(sym), n(1.0), Flags::ALL)));
    assert!(ok(rt.in_operator(&Value::Symbol(sym), &obj)), "a symbol key");
}

#[test]
fn in_operator_throws_for_a_primitive_right_side_with_v8_messages() {
    let rt = Rt::new();
    let msg = |key: &Value, rhs: &Value| -> Res<bool> { rt.in_operator(key, rhs) };
    expect_throw(&rt, "'a' in 1", msg(&s("a"), &n(1.0)), "TypeError", "Cannot use 'in' operator to search for 'a' in 1");
    expect_throw(
        &rt,
        "'a' in undefined",
        msg(&s("a"), &Value::Undefined),
        "TypeError",
        "Cannot use 'in' operator to search for 'a' in undefined",
    );
    expect_throw(&rt, "'a' in null", msg(&s("a"), &Value::Null), "TypeError", "Cannot use 'in' operator to search for 'a' in null");
    expect_throw(&rt, "1 in 2", msg(&n(1.0), &n(2.0)), "TypeError", "Cannot use 'in' operator to search for '1' in 2");
    let sym = Value::Symbol(rt.new_symbol(Some(JsStr::from_str("x"))));
    expect_throw(
        &rt,
        "Symbol(x) in 1",
        msg(&sym, &n(1.0)),
        "TypeError",
        "Cannot use 'in' operator to search for 'Symbol(x)' in 1",
    );
    let plain = Value::Object(rt.new_object());
    expect_throw(
        &rt,
        "{} in 1",
        msg(&plain, &n(1.0)),
        "TypeError",
        "Cannot use 'in' operator to search for '#<Object>' in 1",
    );
}

#[test]
fn in_operator_checks_the_right_side_before_converting_the_key() {
    let rt = Rt::new();
    let key = conv_obj(&rt, "k", n(9.0), "kk", false);
    // The primitive-right-side TypeError comes first, so the key's toString never runs (V8: same order).
    let err = rt.in_operator(&key, &n(1.0)).unwrap_err();
    assert_eq!(thrown(&rt, &err).0, "TypeError");
    assert!(take_log().is_empty(), "the key was not converted");
    let target = Value::Object(rt.new_object());
    assert!(ok(rt.define_data(target.as_object().unwrap(), PropKey::from_str("kk"), n(1.0), Flags::ALL)));
    assert!(ok(rt.in_operator(&key, &target)), "the key converts through toString");
    assert_eq!(take_log(), ["k.toString"]);
}

#[test]
fn unary_operators_convert_their_operand_once() {
    let rt = Rt::new();
    let a = conv_obj(&rt, "a", n(5.0), "t", false);
    expect("-obj", rt.neg(&a), n(-5.0));
    assert_eq!(take_log(), ["a.valueOf"]);
    expect("+' 12 '", rt.pos(&s(" 12 ")), n(12.0));
    expect("~'7'", rt.bit_not(&s("7")), n(-8.0));
    expect("-'-0' is +0", rt.neg(&s("-0")), n(0.0));
    assert!(!rt.not_(&a));
    assert!(take_log().is_empty(), "! uses ToBoolean and never converts");
    assert!(rt.not_(&s("")));
    assert!(rt.not_(&n(f64::NAN)));
    assert!(!rt.not_(&Value::Object(rt.new_object())));
}

#[test]
fn bitwise_and_shift_operators_mask_their_counts_and_wrap() {
    let rt = Rt::new();
    expect("1 << 33", rt.shl(&n(1.0), &n(33.0)), n(2.0));
    expect("1 << 31", rt.shl(&n(1.0), &n(31.0)), n(-2147483648.0));
    expect("1 >> 32", rt.shr(&n(1.0), &n(32.0)), n(1.0));
    expect("-16 >> 2", rt.shr(&n(-16.0), &n(2.0)), n(-4.0));
    expect("-1 >>> 0", rt.ushr(&n(-1.0), &n(0.0)), n(4294967295.0));
    expect("-1 >>> 32", rt.ushr(&n(-1.0), &n(32.0)), n(4294967295.0));
    expect("1 << '1'", rt.shl(&n(1.0), &s("1")), n(2.0));
    expect("'3' & '1'", rt.bit_and(&s("3"), &s("1")), n(1.0));
    expect("5 | 2", rt.bit_or(&n(5.0), &n(2.0)), n(7.0));
    expect("5 ^ 3", rt.bit_xor(&n(5.0), &n(3.0)), n(6.0));
    expect("~5", rt.bit_not(&n(5.0)), n(-6.0));
    expect("(2^32 + 1) & 1", rt.bit_and(&n(4294967297.0), &n(1.0)), n(1.0));
}

#[test]
fn arithmetic_special_values_follow_ieee_and_ecmascript() {
    let rt = Rt::new();
    expect("0 / 0", rt.div(&n(0.0), &n(0.0)), n(f64::NAN));
    expect("1 / -0", rt.div(&n(1.0), &n(-0.0)), n(f64::NEG_INFINITY));
    expect("-0 + 0", rt.add(&n(-0.0), &n(0.0)), n(0.0));
    expect("0 * -1", rt.mul(&n(0.0), &n(-1.0)), n(-0.0));
    expect("5 % 0", rt.rem(&n(5.0), &n(0.0)), n(f64::NAN));
    expect("-5 % 2", rt.rem(&n(-5.0), &n(2.0)), n(-1.0));
    expect("2 ** -1", rt.pow(&n(2.0), &n(-1.0)), n(0.5));
    expect("(-8) ** (1/3)", rt.pow(&n(-8.0), &n(1.0 / 3.0)), n(f64::NAN));
    expect("Infinity - Infinity", rt.sub(&n(f64::INFINITY), &n(f64::INFINITY)), n(f64::NAN));
}

#[test]
fn inc_and_dec_return_the_new_numeric_value() {
    let rt = Rt::new();
    expect("++'1'", rt.inc(&s("1")), n(2.0));
    expect("--null", rt.dec(&Value::Null), n(-1.0));
    expect("++undefined", rt.inc(&Value::Undefined), n(f64::NAN));
    expect("--true", rt.dec(&Value::Bool(true)), n(0.0));
    let a = conv_obj(&rt, "a", n(5.0), "t", false);
    expect("++obj", rt.inc(&a), n(6.0));
    assert_eq!(take_log(), ["a.valueOf"]);
}

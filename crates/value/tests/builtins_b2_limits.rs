//! WP-B2 limits: array-likes near 2^53 - 1 (string keys above the index range), the dense-limit RangeErrors, cyclic
//! arrays (join re-entry gives the empty string; flat of a cycle is a stack overflow RangeError), and sort's write-back
//! rule. Expected values are V8's (Node 26).

use vn_value::{Flags, Intrinsic, PropKey, Res, Rt, Value};

/// Calls `Array.prototype[name]` with `this` (an array-like needs the prototype method, not its own property).
fn proto_call(rt: &Rt, this: &Value, name: &str, args: &[Value]) -> Res<Value> {
    let f = rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::ArrayPrototype)), name)?;
    rt.call(&f, this, args)
}

const TWO53_M1: f64 = 9_007_199_254_740_991.0;

fn method(rt: &Rt, this: &Value, name: &str, args: &[Value]) -> Res<Value> {
    let f = rt.get_str(this, name)?;
    rt.call(&f, this, args)
}

fn error_name(rt: &Rt, e: &vn_value::Throw) -> String {
    match rt.get_str(&e.0, "name") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => "?".into(),
    }
}

fn array_like(rt: &Rt, length: f64, entries: &[(&str, Value)]) -> Value {
    let o = rt.new_object();
    rt.define_data(o, PropKey::from_str("length"), Value::Number(length), Flags::ALL).expect("length");
    for (k, v) in entries {
        rt.define_data(o, PropKey::from_str(k), v.clone(), Flags::ALL).expect("entry");
    }
    Value::Object(o)
}

fn arr(rt: &Rt, elems: Vec<Value>) -> Value {
    Value::Object(rt.new_array(elems))
}

#[test]
fn push_past_two_to_the_53_minus_one_is_a_type_error() {
    // V8: "Pushing 1 elements on an array-like of length 9007199254740991 is disallowed ..."
    let rt = Rt::new();
    let o = array_like(&rt, TWO53_M1, &[]);
    let err = proto_call(&rt, &o, "push", &[Value::Number(1.0)]).expect_err("push");
    assert_eq!(error_name(&rt, &err), "TypeError");
}

#[test]
fn push_at_the_limit_minus_one_writes_a_string_key() {
    // V8: [push] on {length: 2^53-2} returns 2^53-1, sets the length, and stores the element under "9007199254740990".
    let rt = Rt::new();
    let o = array_like(&rt, TWO53_M1 - 1.0, &[]);
    let n = proto_call(&rt, &o, "push", &[Value::Number(1.0)]).expect("push");
    assert_eq!(n, Value::Number(TWO53_M1));
    assert_eq!(rt.get_str(&o, "length").expect("length"), Value::Number(TWO53_M1));
    assert_eq!(rt.get_str(&o, "9007199254740990").expect("key"), Value::Number(1.0));
}

#[test]
fn pop_and_at_read_string_keys_above_the_index_range() {
    // V8: pop on {length: 2^53-1, "9007199254740990": "x"} returns "x" and leaves length 2^53-2; at(-1) reads the same key.
    let rt = Rt::new();
    let o = array_like(&rt, TWO53_M1, &[("9007199254740990", Value::str("x"))]);
    assert_eq!(proto_call(&rt, &o, "at", &[Value::Number(-1.0)]).expect("at"), Value::str("x"));
    assert_eq!(proto_call(&rt, &o, "pop", &[]).expect("pop"), Value::str("x"));
    assert_eq!(rt.get_str(&o, "length").expect("length"), Value::Number(TWO53_M1 - 1.0));
}

#[test]
fn unshift_past_the_limit_is_a_type_error() {
    let rt = Rt::new();
    let o = array_like(&rt, TWO53_M1, &[]);
    let err = proto_call(&rt, &o, "unshift", &[Value::Number(1.0)]).expect_err("unshift");
    assert_eq!(error_name(&rt, &err), "TypeError");
}

#[test]
fn slice_of_a_huge_array_like_is_a_range_error_beyond_the_dense_limit() {
    // V8: slice of {length: 2^53-1} from 0 would need an array of that length: RangeError (here too, the dense limit).
    let rt = Rt::new();
    let o = array_like(&rt, TWO53_M1, &[]);
    let err = proto_call(&rt, &o, "slice", &[Value::Number(0.0)]).expect_err("slice");
    assert_eq!(error_name(&rt, &err), "RangeError");
}

#[test]
fn growing_an_array_past_the_dense_limit_is_a_range_error() {
    // Deviation (ABI.md section 16): V8 allows sparse arrays of this length; this runtime refuses the growth.
    let rt = Rt::new();
    let a = arr(&rt, vec![]);
    let err = rt.set_str(&a, "length", Value::Number(4_194_305.0)).expect_err("length");
    assert_eq!(error_name(&rt, &err), "RangeError");
}

#[test]
fn join_of_a_cyclic_element_is_empty_for_the_cycle() {
    // V8: a = [1]; a[1] = a; a.join() is "1," (the inner join of a is the empty string).
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0)]);
    rt.set_str(&a, "1", a.clone()).expect("cycle");
    assert_eq!(method(&rt, &a, "join", &[]).expect("join"), Value::str("1,"));
    assert_eq!(method(&rt, &a, "toString", &[]).expect("toString"), Value::str("1,"));
}

#[test]
fn flat_of_a_cyclic_array_is_a_stack_overflow_range_error() {
    // V8: a = [1]; a.push(a); a.flat(Infinity) throws RangeError: Maximum call stack size exceeded.
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0)]);
    method(&rt, &a, "push", &[a.clone()]).expect("push");
    let err = method(&rt, &a, "flat", &[Value::Number(f64::INFINITY)]).expect_err("flat");
    assert_eq!(error_name(&rt, &err), "RangeError");
}

#[test]
fn last_index_of_with_an_explicit_undefined_from_index_starts_at_zero() {
    // V8: [1,2,1].lastIndexOf(1, undefined) is 0 (an explicit fromIndex counts as present).
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0), Value::Number(2.0), Value::Number(1.0)]);
    assert_eq!(method(&rt, &a, "lastIndexOf", &[Value::Number(1.0), Value::Undefined]).expect("lastIndexOf"), Value::Number(0.0));
    assert_eq!(method(&rt, &a, "lastIndexOf", &[Value::Number(1.0)]).expect("lastIndexOf"), Value::Number(2.0));
}

#[test]
fn sort_of_a_single_element_frozen_array_writes_nothing() {
    // V8: Object.freeze([2]).sort() returns the array without a write (no error), while a frozen [2,1] throws.
    let rt = Rt::new();
    let one = arr(&rt, vec![Value::Number(2.0)]);
    rt.freeze(one.as_object().expect("object")).expect("freeze");
    assert_eq!(method(&rt, &one, "sort", &[]).expect("sort"), one);
    let two = arr(&rt, vec![Value::Number(2.0), Value::Number(1.0)]);
    rt.freeze(two.as_object().expect("object")).expect("freeze");
    let err = method(&rt, &two, "sort", &[]).expect_err("sort of frozen [2,1]");
    assert_eq!(error_name(&rt, &err), "TypeError");
}

//! WP-B2 array iterators (%ArrayIteratorPrototype%, ECMA-262 23.1.5) and the iteration helpers: symbol keys, live length,
//! sticky exhaustion, the toStringTag, receiver checks and collection safety. Expected values are V8's (Node 26).

use vn_value::builtins::array::array_from_iterable;
use vn_value::{Intrinsic, PropKey, Res, Rt, Value, WellKnown};

fn arr(rt: &Rt, elems: Vec<Value>) -> Value {
    Value::Object(rt.new_array(elems))
}

fn method(rt: &Rt, this: &Value, name: &str, args: &[Value]) -> Res<Value> {
    let f = rt.get_str(this, name)?;
    rt.call(&f, this, args)
}

fn text(rt: &Rt, v: &Value) -> String {
    match v {
        Value::Undefined => "undefined".into(),
        Value::Object(_) => "object".into(),
        _ => rt.to_string(v).map(|s| s.to_rust_string_lossy()).unwrap_or_else(|_| "?".into()),
    }
}

fn render(rt: &Rt, v: &Value) -> String {
    match v {
        Value::Object(o) if rt.is_array(v) => {
            let len = match rt.get_str(v, "length") {
                Ok(Value::Number(n)) => n as u32,
                _ => 0,
            };
            let parts: Vec<String> = (0..len)
                .map(|i| {
                    let key = PropKey::Index(i);
                    if rt.has_property(*o, &key).unwrap_or(false) {
                        render(rt, &rt.get_v(v, &key).unwrap_or(Value::Undefined))
                    } else {
                        "<hole>".to_string()
                    }
                })
                .collect();
            format!("[{}]", parts.join(","))
        }
        other => text(rt, other),
    }
}

fn global_array(rt: &Rt) -> Value {
    rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), "Array").expect("Array")
}

/// The value and done flag of an iterator result.
fn step(rt: &Rt, it: &Value) -> (String, bool) {
    let r = method(rt, it, "next", &[]).expect("next");
    let done = matches!(rt.get_str(&r, "done"), Ok(Value::Bool(true)));
    (render(rt, &rt.get_str(&r, "value").unwrap_or(Value::Undefined)), done)
}

#[test]
fn symbol_iterator_is_values_with_its_own_name_and_length() {
    let rt = Rt::new();
    let ap = Value::Object(rt.intrinsic(Intrinsic::ArrayPrototype));
    let values = rt.get_str(&ap, "values").expect("values");
    let sym_iter = rt.get_v(&ap, &PropKey::Sym(rt.well_known(WellKnown::Iterator))).expect("@@iterator");
    assert_eq!(sym_iter, values, "Array.prototype[Symbol.iterator] is Array.prototype.values");
    assert_eq!(rt.get_str(&values, "name").expect("name"), Value::str("values"));
    assert_eq!(rt.get_str(&values, "length").expect("length"), Value::Number(0.0));
}

#[test]
fn values_iterator_steps_and_stays_done_after_growth() {
    // V8: [1].values(); next gives 1, then done; a later push does not revive the iterator.
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0)]);
    let it = method(&rt, &a, "values", &[]).expect("values");
    assert_eq!(step(&rt, &it), ("1".to_string(), false));
    assert_eq!(step(&rt, &it).1, true);
    method(&rt, &a, "push", &[Value::Number(2.0)]).expect("push");
    assert_eq!(step(&rt, &it).1, true);
}

#[test]
fn values_iterator_sees_elements_pushed_before_it_ends() {
    // V8: the length is read on every step, so the element pushed while iterating is yielded.
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0)]);
    let it = method(&rt, &a, "values", &[]).expect("values");
    assert_eq!(step(&rt, &it), ("1".to_string(), false));
    method(&rt, &a, "push", &[Value::Number(2.0)]).expect("push");
    assert_eq!(step(&rt, &it), ("2".to_string(), false));
    assert_eq!(step(&rt, &it).1, true);
}

#[test]
fn keys_iterator_after_shrink_is_not_done_at_index_zero() {
    // V8: a = [1,2,3]; keys(); a.length = 1; next() gives index 0, not done.
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]);
    let it = method(&rt, &a, "keys", &[]).expect("keys");
    rt.set_str(&a, "length", Value::Number(1.0)).expect("length");
    assert_eq!(step(&rt, &it), ("0".to_string(), false));
    assert_eq!(step(&rt, &it).1, true);
}

#[test]
fn entries_yield_index_value_pairs() {
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::str("a"), Value::str("b")]);
    let it = method(&rt, &a, "entries", &[]).expect("entries");
    assert_eq!(step(&rt, &it), ("[0,a]".to_string(), false));
    assert_eq!(step(&rt, &it), ("[1,b]".to_string(), false));
    assert_eq!(step(&rt, &it).1, true);
}

#[test]
fn array_iterator_has_the_array_iterator_tag() {
    // V8: Object.prototype.toString.call([].keys()) is "[object Array Iterator]".
    let rt = Rt::new();
    let a = arr(&rt, vec![]);
    let it = method(&rt, &a, "keys", &[]).expect("keys");
    let object_to_string = rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::ObjectPrototype)), "toString").expect("toString");
    let tag = rt.call(&object_to_string, &it, &[]).expect("toString call");
    assert_eq!(tag, Value::str("[object Array Iterator]"));
}

#[test]
fn next_on_a_foreign_receiver_is_a_type_error() {
    let rt = Rt::new();
    let proto = rt.builtin("ArrayIteratorPrototype").expect("ArrayIteratorPrototype");
    let next = rt.get_str(&Value::Object(proto), "next").expect("next");
    let err = rt.call(&next, &Value::Object(rt.new_object()), &[]).expect_err("next on a plain object");
    assert_eq!(text(&rt, &rt.get_str(&err.0, "name").expect("name")), "TypeError");
}

#[test]
fn a_rooted_iterator_keeps_its_array_alive_across_a_collection() {
    // The iterator is the only root of the array: its payload must trace the array.
    let rt = Rt::new();
    let it = {
        let a = arr(&rt, vec![Value::Number(7.0), Value::Number(8.0)]);
        method(&rt, &a, "values", &[]).expect("values")
    };
    let id = rt.root(&it);
    rt.collect();
    let it = rt.root_value(id);
    assert_eq!(step(&rt, &it), ("7".to_string(), false));
    assert_eq!(step(&rt, &it), ("8".to_string(), false));
    rt.unroot(id);
}

#[test]
fn array_from_iterable_fills_holes_with_undefined() {
    // V8: [...[1,,3]] is [1, undefined, 3]: holes are read through the iterator.
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0), Value::Hole, Value::Number(3.0)]);
    let out = array_from_iterable(&rt, &a).expect("array_from_iterable");
    assert_eq!(render(&rt, &Value::Object(out)), "[1,undefined,3]");
}

#[test]
fn array_from_string_iterates_by_code_point() {
    // V8: Array.from("a\u{1F600}b") has three elements; the emoji is one element (a surrogate pair).
    let rt = Rt::new();
    let out = method(&rt, &global_array(&rt), "from", &[Value::str("a\u{1F600}b")]).expect("from");
    assert_eq!(rt.get_str(&out, "length").expect("length"), Value::Number(3.0));
    assert_eq!(render(&rt, &out), "[a,\u{1F600},b]");
}

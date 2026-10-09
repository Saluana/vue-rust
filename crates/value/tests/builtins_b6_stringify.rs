//! WP-B6: JSON.stringify (ECMA-262 25.5.2) with values built on the kernel API: primitives, strings, objects and arrays,
//! the gap, the property list of an array replacer, toJSON, replacer functions, boxed booleans, cycles, getter order,
//! and very deep nesting. Expected texts are the ones V8 produces for the same values (checked with Node 26).

use std::cell::RefCell;

use vn_value::{Call, Flags, Intrinsic, JsStr, PropKey, Res, Rt, Throw, Value};

thread_local! {
    /// Records the calls of the native callbacks below, in order.
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn log_take() -> Vec<String> {
    LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

fn ok<T>(r: Res<T>) -> T {
    match r {
        Ok(v) => v,
        Err(t) => panic!("unexpected JS throw: {:?}", t.0),
    }
}

fn error_name(rt: &Rt, t: &Throw) -> String {
    match rt.get_str(&t.0, "name") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => String::from("<no name>"),
    }
}

fn stringify_fn(rt: &Rt) -> Value {
    let g = rt.intrinsic(Intrinsic::Global);
    let json = rt.get(g, &PropKey::from_str("JSON"), &Value::Object(g)).expect("JSON");
    rt.get_str(&json, "stringify").expect("JSON.stringify")
}

/// `JSON.stringify(args...)`.
fn stringify(rt: &Rt, args: &[Value]) -> Res<Value> {
    let g = rt.intrinsic(Intrinsic::Global);
    let json = rt.get(g, &PropKey::from_str("JSON"), &Value::Object(g)).expect("JSON");
    rt.call(&stringify_fn(rt), &json, args)
}

/// The text of `JSON.stringify(v)`, which must be a string.
fn text(rt: &Rt, v: Value) -> String {
    match ok(stringify(rt, &[v])) {
        Value::String(s) => s.to_rust_string_lossy(),
        other => panic!("expected a string, got {other:?}"),
    }
}

/// A plain object with the given enumerable data members, in order.
fn obj(rt: &Rt, members: &[(&str, Value)]) -> Value {
    let o = rt.new_object();
    for (k, v) in members {
        ok(rt.define_data(o, PropKey::from_str(k), v.clone(), Flags::ALL));
    }
    Value::Object(o)
}

fn arr(rt: &Rt, items: Vec<Value>) -> Value {
    Value::Object(rt.new_array(items))
}

fn s(text: &str) -> Value {
    Value::str(text)
}

fn num(n: f64) -> Value {
    Value::Number(n)
}

/// A native function that returns `result` for every call.
fn returns(rt: &Rt, result: Value) -> Value {
    let f = rt.new_native_function(constant, "f", 1, vec![result]);
    Value::Object(f)
}

fn constant(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    Ok(data[0].clone())
}

#[test]
fn primitives_serialize_as_json_text() {
    let rt = Rt::new();
    let cases: &[(Value, &str)] = &[
        (Value::Null, "null"),
        (Value::Bool(true), "true"),
        (Value::Bool(false), "false"),
        (num(0.0), "0"),
        (num(-0.0), "0"),
        (num(1.5), "1.5"),
        (num(-1.5), "-1.5"),
        (num(1e21), "1e+21"),
        (num(1e-7), "1e-7"),
        (num(5e-324), "5e-324"),
        (num(f64::NAN), "null"),
        (num(f64::INFINITY), "null"),
        (num(f64::NEG_INFINITY), "null"),
        (num(9007199254740992.0), "9007199254740992"),
    ];
    for (v, want) in cases {
        assert_eq!(text(&rt, v.clone()), *want, "JSON.stringify({v:?})");
    }
}

#[test]
fn undefined_functions_and_symbols_at_the_top_level_give_undefined() {
    let rt = Rt::new();
    assert_eq!(ok(stringify(&rt, &[Value::Undefined])), Value::Undefined);
    let f = returns(&rt, Value::Undefined);
    assert_eq!(ok(stringify(&rt, &[f])), Value::Undefined);
    let sym = Value::Symbol(rt.new_symbol(Some(JsStr::from_str("s"))));
    assert_eq!(ok(stringify(&rt, &[sym])), Value::Undefined);
}

#[test]
fn strings_escape_quotes_controls_and_lone_surrogates() {
    let rt = Rt::new();
    assert_eq!(text(&rt, s("\"\\")), "\"\\\"\\\\\"");
    assert_eq!(text(&rt, s("\u{8}\u{c}\n\r\t")), "\"\\b\\f\\n\\r\\t\"");
    // Other controls are \u00xx; DEL and the line and paragraph separators are written as they are.
    assert_eq!(text(&rt, s("\u{0}\u{1f}\u{7f}")), "\"\\u0000\\u001f\u{7f}\"");
    assert_eq!(text(&rt, s("\u{2028}\u{2029}")), "\"\u{2028}\u{2029}\"");
    // A surrogate pair is written as the character; lone surrogates as \udxxx in lowercase.
    let pair = Value::String(JsStr::from_units(&[0xD83D, 0xDE00]));
    assert_eq!(text(&rt, pair), "\"\u{1F600}\"");
    let lone_high = Value::String(JsStr::from_units(&[0xD800]));
    assert_eq!(text(&rt, lone_high), "\"\\ud800\"");
    let lone_low = Value::String(JsStr::from_units(&[0xDC00, 0x61]));
    assert_eq!(text(&rt, lone_low), "\"\\udc00a\"");
    let reversed = Value::String(JsStr::from_units(&[0xDE00, 0xD83D]));
    assert_eq!(text(&rt, reversed), "\"\\ude00\\ud83d\"");
}

#[test]
fn objects_leave_out_undefined_functions_symbols_and_hidden_members() {
    let rt = Rt::new();
    let f = returns(&rt, Value::Undefined);
    let sym = Value::Symbol(rt.new_symbol(None));
    let o = rt.new_object();
    ok(rt.define_data(o, PropKey::from_str("a"), Value::Undefined, Flags::ALL));
    ok(rt.define_data(o, PropKey::from_str("b"), num(1.0), Flags::ALL));
    ok(rt.define_data(o, PropKey::from_str("c"), f, Flags::ALL));
    ok(rt.define_data(o, PropKey::from_str("d"), sym.clone(), Flags::ALL));
    ok(rt.define_data(o, PropKey::from_str("e"), Value::Null, Flags::ALL));
    ok(rt.define_data(o, PropKey::from_str("hidden"), num(9.0), Flags::NONE));
    ok(rt.define_data(o, PropKey::Sym(rt.new_symbol(None)), num(8.0), Flags::ALL));
    assert_eq!(text(&rt, Value::Object(o)), "{\"b\":1,\"e\":null}");
    // Own-key order: integer keys ascending, then strings in insertion order.
    let v = obj(&rt, &[("b", num(1.0)), ("a", num(2.0))]);
    assert_eq!(text(&rt, v), "{\"b\":1,\"a\":2}");
    let v = obj(&rt, &[("x", num(3.0)), ("2", num(1.0)), ("1", num(2.0))]);
    assert_eq!(text(&rt, v), "{\"1\":2,\"2\":1,\"x\":3}");
    assert_eq!(text(&rt, obj(&rt, &[])), "{}");
}

#[test]
fn arrays_write_null_for_undefined_functions_symbols_and_holes() {
    let rt = Rt::new();
    let f = returns(&rt, Value::Undefined);
    let sym = Value::Symbol(rt.new_symbol(None));
    let a = arr(&rt, vec![Value::Undefined, f, sym, Value::Null]);
    assert_eq!(text(&rt, a), "[null,null,null,null]");
    let holes = arr(&rt, vec![Value::Hole, num(1.0), Value::Hole]);
    assert_eq!(text(&rt, holes), "[null,1,null]");
    assert_eq!(text(&rt, arr(&rt, vec![])), "[]");
    // A named property of an array is not serialized.
    let named = rt.new_array(vec![num(1.0)]);
    ok(rt.define_data(named, PropKey::from_str("x"), num(2.0), Flags::ALL));
    assert_eq!(text(&rt, Value::Object(named)), "[1]");
}

#[test]
fn space_gives_indentation_with_the_spec_clamping() {
    let rt = Rt::new();
    let value = || obj(&rt, &[("a", arr(&rt, vec![num(1.0), obj(&rt, &[("b", num(2.0))])])), ("c", obj(&rt, &[])), ("d", arr(&rt, vec![]))]);
    let stringify_with = |space: Value| text_args(&rt, &[value(), Value::Undefined, space]);
    let two = "{\n  \"a\": [\n    1,\n    {\n      \"b\": 2\n    }\n  ],\n  \"c\": {},\n  \"d\": []\n}";
    assert_eq!(stringify_with(num(2.0)), two);
    assert_eq!(stringify_with(num(2.9)), two);
    // More than ten spaces are clamped to ten; the gap of a string is its first ten units.
    let ten = "{\n          \"a\": 1\n}";
    assert_eq!(text_args(&rt, &[obj(&rt, &[("a", num(1.0))]), Value::Undefined, num(20.0)]), ten);
    let long = text_args(&rt, &[obj(&rt, &[("a", num(1.0))]), Value::Undefined, s("abcdefghijklmnop")]);
    assert_eq!(long, "{\nabcdefghij\"a\": 1\n}");
    // Less than one space, NaN, the empty string and non-number, non-string spaces give the compact form.
    for space in [num(0.0), num(-1.0), num(f64::NAN), s(""), Value::Bool(true), Value::Null] {
        assert_eq!(text_args(&rt, &[obj(&rt, &[("a", num(1.0))]), Value::Undefined, space]), "{\"a\":1}");
    }
    assert_eq!(text_args(&rt, &[obj(&rt, &[("a", num(1.0))]), Value::Undefined, num(f64::INFINITY)]), ten);
    assert_eq!(text_args(&rt, &[arr(&rt, vec![num(1.0)]), Value::Undefined, num(1.0)]), "[\n 1\n]");
}

fn text_args(rt: &Rt, args: &[Value]) -> String {
    match ok(stringify(rt, args)) {
        Value::String(s) => s.to_rust_string_lossy(),
        other => panic!("expected a string, got {other:?}"),
    }
}

#[test]
fn an_array_replacer_is_the_property_list_of_every_object_below() {
    let rt = Rt::new();
    let v = obj(&rt, &[("a", num(1.0)), ("b", num(2.0)), ("c", obj(&rt, &[("a", num(3.0)), ("b", num(4.0))]))]);
    let list = arr(&rt, vec![s("b"), s("a")]);
    // `c` is not listed, so it is left out. The listed members come in list order.
    assert_eq!(text_args(&rt, &[v.clone(), list]), "{\"b\":2,\"a\":1}");
    // Numbers become strings, duplicates go, and other items are ignored.
    let list = arr(&rt, vec![s("a"), s("a"), num(1.0), s("1"), num(1.5), Value::Null, Value::Bool(true)]);
    let v = obj(&rt, &[("a", num(1.0)), ("1", num(2.0)), ("b", num(3.0))]);
    assert_eq!(text_args(&rt, &[v, list]), "{\"a\":1,\"1\":2}");
    let v = obj(&rt, &[("0", num(4.0)), ("1.5", num(5.0))]);
    assert_eq!(text_args(&rt, &[v, arr(&rt, vec![num(1.5), num(-0.0)])]), "{\"1.5\":5,\"0\":4}");
    // An empty list writes every object as {}.
    let v = obj(&rt, &[("a", num(1.0))]);
    assert_eq!(text_args(&rt, &[v, arr(&rt, vec![])]), "{}");
    // Arrays are not filtered by the list.
    let v = arr(&rt, vec![obj(&rt, &[("a", num(1.0)), ("b", num(2.0))])]);
    assert_eq!(text_args(&rt, &[v, arr(&rt, vec![s("a")])]), "[{\"a\":1}]");
}

#[test]
fn a_non_callable_replacer_is_ignored() {
    let rt = Rt::new();
    let v = obj(&rt, &[("a", num(1.0))]);
    assert_eq!(text_args(&rt, &[v.clone(), num(5.0)]), "{\"a\":1}");
    assert_eq!(text_args(&rt, &[v.clone(), s("a")]), "{\"a\":1}");
    assert_eq!(text_args(&rt, &[v, obj(&rt, &[])]), "{\"a\":1}");
}

#[test]
fn to_json_is_called_with_the_key_and_the_object_as_this() {
    let rt = Rt::new();
    let to_json = rt.new_native_function(key_marker, "toJSON", 1, Vec::new());
    let inner = rt.new_object();
    ok(rt.define_data(inner, PropKey::from_str("toJSON"), Value::Object(to_json), Flags::ALL));
    let v = obj(&rt, &[("a", Value::Object(inner))]);
    assert_eq!(text(&rt, v), "{\"a\":\"K:a\"}");
    // A toJSON that returns undefined leaves the member out, and at the top level gives undefined.
    let gone = rt.new_object();
    ok(rt.define_data(gone, PropKey::from_str("toJSON"), returns(&rt, Value::Undefined), Flags::ALL));
    let v = obj(&rt, &[("x", Value::Object(gone)), ("y", num(1.0))]);
    assert_eq!(text(&rt, v), "{\"y\":1}");
    assert_eq!(ok(stringify(&rt, &[Value::Object(gone)])), Value::Undefined);
    // A toJSON that returns its receiver is serialized once, as an object without its function member.
    let this_obj = rt.new_object();
    let this_fn = rt.new_native_function(return_this, "toJSON", 0, Vec::new());
    ok(rt.define_data(this_obj, PropKey::from_str("toJSON"), Value::Object(this_fn), Flags::ALL));
    assert_eq!(text(&rt, Value::Object(this_obj)), "{}");
    // toJSON is looked up on arrays too.
    let a = rt.new_array(vec![num(1.0)]);
    ok(rt.define_data(a, PropKey::from_str("toJSON"), returns(&rt, s("x")), Flags::ALL));
    assert_eq!(text(&rt, Value::Object(a)), "\"x\"");
}

/// `toJSON(key)` returns "K:" followed by the key.
fn key_marker(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let key = match c.arg(0) {
        Value::String(s) => s,
        other => panic!("toJSON key is not a string: {other:?}"),
    };
    Ok(Value::String(JsStr::from_str("K:").concat(&key)))
}

fn return_this(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(c.this.clone())
}

#[test]
fn a_replacer_function_is_called_for_every_value_in_order_with_the_holder_as_this() {
    let rt = Rt::new();
    let repl = rt.new_native_function(log_replacer, "replacer", 2, Vec::new());
    let v = obj(&rt, &[("a", num(1.0)), ("b", obj(&rt, &[("c", num(2.0))]))]);
    let out = ok(stringify(&rt, &[v, Value::Object(repl)]));
    assert_eq!(out, s("{\"a\":1,\"b\":{\"c\":2}}"));
    assert_eq!(log_take(), vec!["\"\"", "\"a\"", "\"b\"", "\"c\""]);
}

/// Logs the key, then returns the value unchanged; the holder of the top-level call is the wrapper object, not undefined.
fn log_replacer(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let key = match c.arg(0) {
        Value::String(s) => s.to_rust_string_lossy(),
        other => panic!("replacer key is not a string: {other:?}"),
    };
    assert!(matches!(c.this, Value::Object(_)), "the replacer's this is the holder object");
    LOG.with(|l| l.borrow_mut().push(format!("\"{key}\"")));
    Ok(c.arg(1))
}

#[test]
fn a_replacer_can_omit_members_replace_values_and_throw() {
    let rt = Rt::new();
    let drop_numbers = rt.new_native_function(drop_numbers_fn, "replacer", 2, Vec::new());
    let v = obj(&rt, &[("a", num(1.0)), ("b", s("x")), ("c", num(2.0))]);
    assert_eq!(ok(stringify(&rt, &[v, Value::Object(drop_numbers)])), s("{\"b\":\"x\"}"));
    // At the top level, undefined from the replacer gives undefined.
    assert_eq!(ok(stringify(&rt, &[num(1.0), Value::Object(drop_numbers)])), Value::Undefined);
    let thrower = rt.new_native_function(throw_fn, "replacer", 2, Vec::new());
    let err = stringify(&rt, &[obj(&rt, &[("a", num(1.0))]), Value::Object(thrower)]).expect_err("the replacer throws");
    assert_eq!(error_name(&rt, &err), "TypeError");
}

fn drop_numbers_fn(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    match c.arg(1) {
        Value::Number(_) => Ok(Value::Undefined),
        other => Ok(other),
    }
}

fn throw_fn(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_type_error("replacer refuses"))
}

#[test]
fn a_boxed_boolean_serializes_as_its_primitive() {
    let rt = Rt::new();
    let no = Value::Object(ok(rt.to_object(&Value::Bool(false))));
    assert_eq!(text(&rt, no.clone()), "false");
    assert_eq!(text(&rt, arr(&rt, vec![no, Value::Object(ok(rt.to_object(&Value::Bool(true))))])), "[false,true]");
}

#[test]
fn cycles_throw_a_type_error_and_shared_references_do_not() {
    let rt = Rt::new();
    let o = rt.new_object();
    ok(rt.define_data(o, PropKey::from_str("self"), Value::Object(o), Flags::ALL));
    let err = stringify(&rt, &[Value::Object(o)]).expect_err("object cycle");
    assert_eq!(error_name(&rt, &err), "TypeError");

    let a = rt.new_array(vec![]);
    ok(rt.define_data(a, PropKey::Index(0), Value::Object(a), Flags::ALL));
    let err = stringify(&rt, &[Value::Object(a)]).expect_err("array cycle");
    assert_eq!(error_name(&rt, &err), "TypeError");

    // A cycle through a nested array and a toJSON that returns a fresh object still throws.
    let top = rt.new_object();
    let nested = rt.new_array(vec![Value::Object(top)]);
    ok(rt.define_data(top, PropKey::from_str("x"), Value::Object(nested), Flags::ALL));
    let err = stringify(&rt, &[Value::Object(top)]).expect_err("nested cycle");
    assert_eq!(error_name(&rt, &err), "TypeError");

    // The same object twice is not a cycle.
    let shared = obj(&rt, &[("v", num(1.0))]);
    assert_eq!(text(&rt, arr(&rt, vec![shared.clone(), shared])), "[{\"v\":1},{\"v\":1}]");
}

#[test]
fn getters_run_in_order_once_and_a_member_deleted_by_a_getter_is_skipped() {
    let rt = Rt::new();
    let o = rt.new_object();
    let getter = rt.new_native_function(deleting_getter, "get a", 0, vec![Value::Object(o)]);
    ok(rt.define_accessor(o, PropKey::from_str("a"), Some(getter), None, Flags::ALL));
    ok(rt.define_data(o, PropKey::from_str("b"), num(2.0), Flags::ALL));
    ok(rt.define_data(o, PropKey::from_str("c"), num(3.0), Flags::ALL));
    assert_eq!(text(&rt, Value::Object(o)), "{\"a\":1,\"c\":3}");
    assert_eq!(log_take(), vec!["a"]);
}

/// The getter of `a`: logs its call, deletes `b` from the object it was read from, and returns 1.
fn deleting_getter(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    LOG.with(|l| l.borrow_mut().push("a".into()));
    let data = rt.fn_data(c.callee);
    let Value::Object(target) = data[0] else { panic!("getter data is the object") };
    ok(rt.delete(target, &PropKey::from_str("b")));
    Ok(num(1.0))
}

#[test]
fn a_very_deep_array_is_written_without_recursion_limits() {
    let rt = Rt::new();
    let depth = 100_000;
    let mut v = rt.new_array(vec![]);
    for _ in 1..depth {
        v = rt.new_array(vec![Value::Object(v)]);
    }
    let out = text(&rt, Value::Object(v));
    assert_eq!(out.len(), 2 * depth);
    assert!(out.starts_with("[[") && out.ends_with("]]"));
    assert!(out.chars().all(|c| c == '[' || c == ']'));
}

#[test]
fn object_keys_that_are_lone_surrogates_are_escaped() {
    let rt = Rt::new();
    let key = JsStr::from_units(&[0xD800]);
    let o = rt.new_object();
    ok(rt.define_data(o, PropKey::from_jsstr(key), num(1.0), Flags::ALL));
    assert_eq!(text(&rt, Value::Object(o)), "{\"\\ud800\":1}");
}

#[test]
fn a_stringified_value_survives_a_collection_while_it_is_rooted() {
    let rt = Rt::new();
    let v = obj(&rt, &[("a", arr(&rt, vec![num(1.0), s("x")]))]);
    let id = rt.root(&v);
    rt.collect();
    assert_eq!(text(&rt, rt.root_value(id)), "{\"a\":[1,\"x\"]}");
    rt.unroot(id);
}

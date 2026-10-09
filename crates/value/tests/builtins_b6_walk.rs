//! WP-B6: the reviver walk of JSON.parse (ECMA-262 25.5.1.1): children before their parent, members in own-key order, the
//! holder as `this`, undefined deletes a member (leaving a hole in an array), a result replaces the value and is not walked
//! again, the root is revived with the empty key, a throwing reviver propagates, and a reviver may change its siblings.
//! Expected results are V8's (Node 26). `show` is the canonical text used by builtins_b6_parse.rs.

use std::cell::RefCell;

use vn_value::{Call, CodeFn, Flags, Intrinsic, JsStr, PropKey, Res, Rt, Throw, Value};

thread_local! {
    /// The calls of the native revivers below, in order.
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn log_push(entry: String) {
    LOG.with(|l| l.borrow_mut().push(entry));
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

/// A string as the canonical form: printable ASCII except `"` and `\` as is, every other unit as `\uxxxx`.
fn quoted(units: &[u16]) -> String {
    let mut out = String::from("\"");
    for &u in units {
        if (0x20..=0x7E).contains(&u) && u != 0x22 && u != 0x5C {
            out.push(char::from(u as u8));
        } else {
            out.push_str(&format!("\\u{u:04x}"));
        }
    }
    out.push('"');
    out
}

fn show(rt: &Rt, v: &Value) -> String {
    match v {
        Value::Undefined => "undefined".into(),
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) if *n == 0.0 && n.is_sign_negative() => "-0".into(),
        Value::Number(n) => vn_value::number::number_to_string(*n),
        Value::String(s) => quoted(s.units()),
        Value::Object(_) if rt.is_callable(v) => "fn".into(),
        Value::Object(o) if rt.is_array(v) => {
            let len = rt.get_str(v, "length").expect("length");
            let len = rt.to_length(&len).expect("length") as u32;
            let items: Vec<String> = (0..len)
                .map(|i| {
                    let k = PropKey::Index(i);
                    if rt.has_own_property(*o, &k).expect("own") {
                        show(rt, &rt.get(*o, &k, v).expect("get"))
                    } else {
                        "<hole>".into()
                    }
                })
                .collect();
            format!("[{}]", items.join(","))
        }
        Value::Object(o) => {
            let mut parts = Vec::new();
            for k in rt.own_keys(*o).expect("own keys") {
                let name = match &k {
                    PropKey::Index(i) => quoted(JsStr::from_str(&i.to_string()).units()),
                    PropKey::Str(s) => quoted(s.units()),
                    PropKey::Sym(_) => continue,
                };
                let val = rt.get(*o, &k, v).expect("get");
                parts.push(format!("{name}:{}", show(rt, &val)));
            }
            format!("{{{}}}", parts.join(","))
        }
        other => format!("<internal {other:?}>"),
    }
}

/// `JSON.parse(text, reviver)`.
fn parse_with(rt: &Rt, text: &str, reviver: Value) -> Res<Value> {
    let g = rt.intrinsic(Intrinsic::Global);
    let json = rt.get(g, &PropKey::from_str("JSON"), &Value::Object(g)).expect("JSON");
    let parse = rt.get_str(&json, "parse").expect("JSON.parse");
    rt.call(&parse, &json, &[Value::str(text), reviver])
}

fn native(rt: &Rt, code: CodeFn) -> Value {
    Value::Object(rt.new_native_function(code, "reviver", 2, Vec::new()))
}

/// The key argument of a reviver call, as text.
fn key_of(c: &Call<'_>) -> String {
    match c.arg(0) {
        Value::String(s) => s.to_rust_string_lossy(),
        other => panic!("reviver key is not a string: {other:?}"),
    }
}

fn log_keys(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log_push(key_of(c));
    Ok(c.arg(1))
}

fn log_quoted_keys_then_undefined(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    match c.arg(0) {
        Value::String(s) => log_push(quoted(s.units())),
        other => panic!("reviver key is not a string: {other:?}"),
    }
    Ok(Value::Undefined)
}

fn drop_even_numbers(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    match c.arg(1) {
        Value::Number(n) if n % 2.0 == 0.0 => Ok(Value::Undefined),
        v => Ok(v),
    }
}

fn wrap_numbers(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    match c.arg(1) {
        Value::Number(n) => {
            let o = rt.new_object();
            rt.define_data(o, PropKey::from_str("n"), Value::Number(n), Flags::ALL)?;
            Ok(Value::Object(o))
        }
        v => Ok(v),
    }
}

fn root_word(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    if key_of(c).is_empty() { Ok(Value::str("root")) } else { Ok(c.arg(1)) }
}

fn root_scalar(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    if key_of(c).is_empty() { Ok(Value::str("scalar-root")) } else { Ok(c.arg(1)) }
}

fn throw_at_b(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    if key_of(c) == "b" {
        return Err(rt.throw_type_error("stop"));
    }
    Ok(c.arg(1))
}

/// Logs the key and the own keys of `this` (the holder), each quoted.
fn log_holder(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let holder = match c.this {
        Value::Object(o) => {
            let names: Vec<String> = rt
                .own_keys(*o)
                .expect("own keys")
                .iter()
                .map(|k| match k {
                    PropKey::Index(i) => quoted(JsStr::from_str(&i.to_string()).units()),
                    PropKey::Str(s) => quoted(s.units()),
                    PropKey::Sym(_) => String::new(),
                })
                .collect();
            names.join("|")
        }
        _ => String::from("<not an object>"),
    };
    log_push(format!("{}@{holder}", key_of(c)));
    Ok(c.arg(1))
}

fn log_key_types(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let entry = match c.arg(0) {
        Value::String(s) => format!("string:{}", quoted(s.units())),
        other => format!("other:{other:?}"),
    };
    log_push(entry);
    Ok(c.arg(1))
}

/// Logs each key and value (objects as `obj`), and deletes index 1 of the holder while visiting index 0.
fn log_and_delete_sibling(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let v = c.arg(1);
    let shown = match &v {
        Value::Object(_) => String::from("obj"),
        _ => show(rt, &v),
    };
    log_push(format!("{}={shown}", key_of(c)));
    if key_of(c) == "0" {
        if let Value::Object(holder) = c.this {
            rt.delete(*holder, &PropKey::Index(1))?;
        }
    }
    Ok(v)
}

fn array_becomes_its_length(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    match c.arg(1) {
        Value::Object(o) if rt.is_array(&Value::Object(o)) => rt.get_str(&Value::Object(o), "length"),
        v => Ok(v),
    }
}

#[test]
fn children_are_revived_before_their_parent_in_key_order() {
    let rt = Rt::new();
    let out = ok(parse_with(&rt, r#"{"a":{"b":1},"c":2}"#, native(&rt, log_keys)));
    assert_eq!(show(&rt, &out), r#"{"a":{"b":1},"c":2}"#);
    assert_eq!(log_take(), vec!["b", "a", "c", ""]);
}

#[test]
fn undefined_deletes_a_member_and_leaves_a_hole_in_an_array() {
    let rt = Rt::new();
    let out = ok(parse_with(&rt, r#"{"a":1,"b":[2,3],"c":4}"#, native(&rt, drop_even_numbers)));
    assert_eq!(show(&rt, &out), r#"{"a":1,"b":[<hole>,3]}"#);
}

#[test]
fn a_revived_value_replaces_the_member_and_is_not_walked_again() {
    let rt = Rt::new();
    let out = ok(parse_with(&rt, r#"[1,[2]]"#, native(&rt, wrap_numbers)));
    assert_eq!(show(&rt, &out), r#"[{"n":1},[{"n":2}]]"#);
}

#[test]
fn the_root_is_revived_last_with_the_empty_key() {
    let rt = Rt::new();
    assert_eq!(show(&rt, &ok(parse_with(&rt, "[1]", native(&rt, root_word)))), r#""root""#);
    assert_eq!(show(&rt, &ok(parse_with(&rt, "1", native(&rt, root_scalar)))), r#""scalar-root""#);
}

#[test]
fn a_throwing_reviver_propagates_its_error() {
    let rt = Rt::new();
    let err = parse_with(&rt, r#"{"a":1,"b":2}"#, native(&rt, throw_at_b)).expect_err("reviver throws");
    assert_eq!(error_name(&rt, &err), "TypeError");
}

#[test]
fn the_holder_is_this_and_the_root_holder_owns_the_empty_key() {
    let rt = Rt::new();
    ok(parse_with(&rt, r#"{"a":1}"#, native(&rt, log_holder)));
    assert_eq!(log_take(), vec![r#"a@"a""#, r#"@"""#]);
}

#[test]
fn index_keys_reach_the_reviver_as_strings() {
    let rt = Rt::new();
    ok(parse_with(&rt, "[7,8]", native(&rt, log_key_types)));
    assert_eq!(log_take(), vec![r#"string:"0""#, r#"string:"1""#, r#"string:"""#]);
}

#[test]
fn members_are_visited_in_own_key_order_with_index_keys_first() {
    let rt = Rt::new();
    let out = ok(parse_with(&rt, r#"{"b":1,"2":2,"1":3}"#, native(&rt, log_quoted_keys_then_undefined)));
    assert_eq!(log_take(), vec![r#""1""#, r#""2""#, r#""b""#, r#""""#]);
    assert_eq!(out, Value::Undefined);
    // The root alone: an undefined result is the result of JSON.parse.
    let root = ok(parse_with(&rt, "5", native(&rt, log_quoted_keys_then_undefined)));
    assert_eq!(root, Value::Undefined);
}

#[test]
fn a_reviver_may_delete_a_later_sibling_and_the_walk_sees_undefined() {
    let rt = Rt::new();
    let out = ok(parse_with(&rt, "[1,2,3]", native(&rt, log_and_delete_sibling)));
    assert_eq!(log_take().join(" "), "0=1 1=undefined 2=3 =obj");
    assert_eq!(show(&rt, &out), "[1,<hole>,3]");
}

#[test]
fn a_reviver_sees_children_already_revived() {
    let rt = Rt::new();
    let out = ok(parse_with(&rt, r#"{"a":[1,2]}"#, native(&rt, array_becomes_its_length)));
    assert_eq!(show(&rt, &out), r#"{"a":2}"#);
}

#[test]
fn a_reviver_that_is_not_callable_is_ignored() {
    let rt = Rt::new();
    let out = ok(parse_with(&rt, "[1]", Value::Number(5.0)));
    assert_eq!(show(&rt, &out), "[1]");
    assert!(log_take().is_empty());
}

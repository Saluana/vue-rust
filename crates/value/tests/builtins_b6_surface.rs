//! WP-B6: the JSON object as installed on the realm. Attributes, lengths and names of the two methods, the toStringTag,
//! the absence of other members, the constructor behaviour, and that parse results survive a collection. Expected values
//! follow ECMA-262 25.5 and were checked against Node 26 (V8).

use vn_value::{Flags, Intrinsic, ObjRef, Prop, PropKey, Rt, Throw, Value, WellKnown};

fn json_obj(rt: &Rt) -> ObjRef {
    let g = rt.intrinsic(Intrinsic::Global);
    match rt.get(g, &PropKey::from_str("JSON"), &Value::Object(g)) {
        Ok(Value::Object(o)) => o,
        other => panic!("JSON is not an object: {other:?}"),
    }
}

fn method(rt: &Rt, name: &str) -> Value {
    let j = json_obj(rt);
    rt.get(j, &PropKey::from_str(name), &Value::Object(j)).expect("method read")
}

fn error_name(rt: &Rt, t: &Throw) -> String {
    match rt.get_str(&t.0, "name") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => String::from("<no name>"),
    }
}

fn own_prop(rt: &Rt, o: ObjRef, name: &str) -> Prop {
    rt.get_own_property(o, &PropKey::from_str(name)).expect("own read").expect("property present")
}

#[test]
fn json_is_a_plain_object_on_the_global_object() {
    let rt = Rt::new();
    let j = json_obj(&rt);
    assert_eq!(rt.type_of(&Value::Object(j)), "object");
    assert!(!rt.is_callable(&Value::Object(j)));
    assert!(!rt.is_constructor(&Value::Object(j)));
    assert_eq!(rt.get_prototype_of(j), Some(rt.intrinsic(Intrinsic::ObjectPrototype)));
    let g = rt.intrinsic(Intrinsic::Global);
    // A builtin global: writable, not enumerable, configurable.
    assert_eq!(own_prop(&rt, g, "JSON"), Prop::Data { value: Value::Object(j), flags: Flags::HIDDEN });
}

#[test]
fn parse_and_stringify_have_spec_lengths_names_and_attributes() {
    let rt = Rt::new();
    let j = json_obj(&rt);
    for (name, len) in [("parse", 2.0), ("stringify", 3.0)] {
        let f = method(&rt, name).as_object().expect("a function object");
        assert!(rt.is_callable(&Value::Object(f)), "{name} is callable");
        assert_eq!(own_prop(&rt, f, "length"), Prop::Data { value: Value::Number(len), flags: Flags::CONFIG });
        assert_eq!(own_prop(&rt, f, "name"), Prop::Data { value: Value::str(name), flags: Flags::CONFIG });
        // A builtin method: writable, not enumerable, configurable.
        assert_eq!(own_prop(&rt, j, name), Prop::Data { value: Value::Object(f), flags: Flags::HIDDEN });
    }
}

#[test]
fn json_has_only_parse_stringify_and_the_tag() {
    let rt = Rt::new();
    let j = json_obj(&rt);
    let keys = rt.own_keys(j).expect("own keys");
    assert_eq!(
        keys,
        vec![
            PropKey::from_str("parse"),
            PropKey::from_str("stringify"),
            PropKey::Sym(rt.well_known(WellKnown::ToStringTag)),
        ]
    );
}

#[test]
fn to_string_tag_is_json_and_read_only() {
    let rt = Rt::new();
    let j = json_obj(&rt);
    let tag = PropKey::Sym(rt.well_known(WellKnown::ToStringTag));
    let got = rt.get_own_property(j, &tag).expect("own read");
    // ECMA-262 25.5.3: not writable, not enumerable, configurable.
    assert_eq!(got, Some(Prop::Data { value: Value::str("JSON"), flags: Flags::CONFIG }));
    let to_string = rt
        .get_str(&Value::Object(rt.intrinsic(Intrinsic::ObjectPrototype)), "toString")
        .expect("Object.prototype.toString");
    assert_eq!(rt.call(&to_string, &Value::Object(j), &[]).expect("call"), Value::str("[object JSON]"));
}

#[test]
fn parse_and_stringify_are_not_constructors() {
    let rt = Rt::new();
    for name in ["parse", "stringify"] {
        let f = method(&rt, name);
        let err = rt.construct(&f, &[], &f).expect_err("new JSON.method must throw");
        assert_eq!(error_name(&rt, &err), "TypeError", "new JSON.{name}");
    }
}

#[test]
fn the_this_value_is_ignored_and_missing_arguments_behave_as_undefined() {
    let rt = Rt::new();
    let parse = method(&rt, "parse");
    let stringify = method(&rt, "stringify");
    // stringify() with no value: undefined result. parse() with no text: ToString(undefined) is "undefined".
    assert_eq!(rt.call(&stringify, &Value::Undefined, &[]).expect("stringify"), Value::Undefined);
    let err = rt.call(&parse, &Value::Undefined, &[]).expect_err("parse() must throw");
    assert_eq!(error_name(&rt, &err), "SyntaxError");
    // The receiver is never read.
    let text = [Value::str("[1]")];
    assert_eq!(rt.call(&parse, &Value::Number(3.0), &text).expect("parse").as_object().is_some(), true);
}

#[test]
fn parse_results_survive_a_collection_between_calls() {
    let rt = Rt::new();
    let j = Value::Object(json_obj(&rt));
    let parse = method(&rt, "parse");
    let stringify = method(&rt, "stringify");
    let text = r#"{"a":[1,{"b":"x"}],"c":null}"#;
    let v = rt.call(&parse, &j, &[Value::str(text)]).expect("parse");
    let id = rt.root(&v);
    rt.collect();
    let kept = rt.root_value(id);
    assert_eq!(rt.call(&stringify, &j, &[kept]).expect("stringify"), Value::str(text));
    rt.unroot(id);
}

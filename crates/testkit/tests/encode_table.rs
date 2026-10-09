//! Hand-written expectations for every row of the encoding table in docs/dev/trace-format.md (v1.1), plus the
//! `integrity` field. The expected text was written from the format document, not copied from the encoder's
//! output. A negative control at the end shows that a doctored expectation is reported.

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use support::{first_difference, key, noop, num, ov, s, units};
use vn_testkit::json::Json;
use vn_testkit::{encode_standalone, trace_call, Recorder};
use vn_value::{Flags, FnInfo, FnKind, Intrinsic, JsStr, NativeObject, Rt, Throw, Tracer, Value, WellKnown};

/// A host payload of a kind the encoder does not map yet (a stand-in for a Date).
struct Opaque;

impl NativeObject for Opaque {
    fn trace(&self, _t: &mut Tracer) {}
}

/// The entries text of the given `(label, values)` calls, traced in order into one recorder.
fn trace_text(rt: &Rt, calls: &[(&str, Vec<Value>)]) -> String {
    let rec = RefCell::new(Recorder::new());
    for (label, values) in calls {
        trace_call(rt, &rec, &JsStr::from_str(label), values).expect("the values encode");
    }
    let entries = rec.borrow().entries().to_vec();
    Json::Arr(entries).to_json()
}

/// The entries text of one `trace("t", ...values)` call, with `body` as the values array.
fn one_call(body: &str) -> String {
    format!(r#"[{{"label":"t","values":{body}}}]"#)
}

fn values_of(rt: &Rt, values: Vec<Value>) -> String {
    trace_text(rt, &[("t", values)])
}

fn construct(rt: &Rt, kind: Intrinsic, args: &[Value]) -> Value {
    let ctor = Value::Object(rt.intrinsic(kind));
    rt.construct(&ctor, args, &ctor).expect("builtin constructor")
}

fn object(rt: &Rt, pairs: &[(&str, Value)]) -> vn_value::ObjRef {
    let o = rt.new_object();
    for (k, v) in pairs {
        rt.define_data(o, key(k), v.clone(), Flags::ALL).expect("define");
    }
    o
}

#[test]
fn undefined_null_and_booleans() {
    let rt = Rt::new();
    let got = values_of(&rt, vec![Value::Undefined, Value::Null, Value::Bool(true), Value::Bool(false)]);
    assert_eq!(got, one_call(r#"[{"$":"undef"},null,true,false]"#));
}

#[test]
fn finite_numbers_are_written_as_string_of_n() {
    let rt = Rt::new();
    let got = values_of(
        &rt,
        vec![num(0.0), num(1.0), num(-5.0), num(0.1), num(1e21), num(1e-7), num(9007199254740992.0)],
    );
    assert_eq!(got, one_call(r#"[0,1,-5,0.1,1e+21,1e-7,9007199254740992]"#));
}

#[test]
fn negative_zero_nan_and_infinities_have_tagged_forms() {
    let rt = Rt::new();
    let got = values_of(&rt, vec![num(-0.0), num(f64::NAN), num(f64::INFINITY), num(f64::NEG_INFINITY)]);
    assert_eq!(
        got,
        one_call(r#"[{"$":"num","v":"-0"},{"$":"num","v":"NaN"},{"$":"num","v":"Infinity"},{"$":"num","v":"-Infinity"}]"#)
    );
}

#[test]
fn strings_escape_like_json_stringify() {
    let rt = Rt::new();
    let got = values_of(
        &rt,
        vec![
            s("a\"b\\c"),
            s("\u{8}\u{c}\n\r\t"),
            s("\u{0}\u{1}\u{1f}"),
            units(&[0xd83d]),
            units(&[0xde00]),
            units(&[0x78, 0xd83d, 0x79]),
            s("\u{1F600}"),
        ],
    );
    assert_eq!(got, one_call(r#"["a\"b\\c","\b\f\n\r\t","\u0000\u0001\u001f","\ud83d","\ude00","x\ud83dy","😀"]"#));
}

#[test]
fn delete_and_line_separator_are_written_raw_like_json_stringify() {
    let rt = Rt::new();
    let got = values_of(&rt, vec![s("\u{7f}\u{2028}")]);
    assert_eq!(got, one_call(&format!("[\"{}{}\"]", '\u{7f}', '\u{2028}')));
}

#[test]
fn symbols_are_numbered_in_first_encounter_order() {
    let rt = Rt::new();
    let a = rt.new_symbol(Some(JsStr::from_str("a")));
    let empty = rt.new_symbol(Some(JsStr::from_str("")));
    let none = rt.new_symbol(None);
    let got = values_of(
        &rt,
        vec![
            Value::Symbol(a),
            Value::Symbol(empty),
            Value::Symbol(a),
            Value::Symbol(none),
            Value::Symbol(rt.well_known(WellKnown::Iterator)),
        ],
    );
    assert_eq!(
        got,
        one_call(
            r#"[{"$":"sym","id":1,"desc":"a"},{"$":"sym","id":2,"desc":""},{"$":"sym","id":1,"desc":"a"},{"$":"sym","id":3,"desc":null},{"$":"sym","id":4,"desc":"Symbol.iterator"}]"#
        )
    );
}

#[test]
fn functions_are_fn_entries_and_take_no_ordinal() {
    let rt = Rt::new();
    let named = rt.new_function(noop, FnInfo::new("named", 2, FnKind::Normal), &[]);
    let anon = rt.new_function(noop, FnInfo::new("", 0, FnKind::Normal), &[]);
    let o = object(&rt, &[("x", num(1.0))]);
    let got = values_of(&rt, vec![ov(named), ov(anon), ov(o)]);
    assert_eq!(
        got,
        one_call(
            r#"[{"$":"fn","name":"named","length":2},{"$":"fn","name":"","length":0},{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[["x",1]]}]"#
        )
    );
}

#[test]
fn plain_object_props_follow_own_key_order() {
    let rt = Rt::new();
    let o = rt.new_object();
    for (k, v) in [("b", num(1.0)), ("a", s("x")), ("2", s("two")), ("1", s("one"))] {
        rt.define_data(o, key(k), v, Flags::ALL).expect("define");
    }
    let got = values_of(&rt, vec![ov(o)]);
    assert_eq!(
        got,
        one_call(r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[["1","one"],["2","two"],["b",1],["a","x"]]}]"#)
    );
}

#[test]
fn proto_is_null_object_prototype_or_other() {
    let rt = Rt::new();
    let inherited = rt.new_object();
    let p1 = rt.new_object_with_proto(None);
    let p2 = rt.new_object_with_proto(Some(inherited));
    let p3 = rt.new_object();
    let got = values_of(&rt, vec![ov(p1), ov(p2), ov(p3)]);
    assert_eq!(
        got,
        one_call(
            r#"[{"$":"obj","id":1,"cls":"Object","proto":"null","props":[]},{"$":"obj","id":2,"cls":"Object","proto":"other","props":[]},{"$":"obj","id":3,"cls":"Object","proto":"Object.prototype","props":[]}]"#
        )
    );
}

#[test]
fn arrays_list_props_then_elems_and_len_with_holes() {
    let rt = Rt::new();
    let a = rt.new_array(vec![num(1.0), Value::Hole, s("x")]);
    rt.define_data(a, key("tag"), s("T"), Flags::ALL).expect("define");
    let got = values_of(&rt, vec![ov(a)]);
    assert_eq!(
        got,
        one_call(r#"[{"$":"obj","id":1,"cls":"Array","proto":"other","props":[["tag","T"]],"elems":[1,{"$":"hole"},"x"],"len":3}]"#)
    );
}

#[test]
fn errors_carry_name_and_message() {
    let rt = Rt::new();
    let e1 = construct(&rt, Intrinsic::TypeErrorCtor, &[s("bad")]);
    let e2 = construct(&rt, Intrinsic::ErrorCtor, &[]);
    let got = values_of(&rt, vec![e1, e2]);
    assert_eq!(
        got,
        one_call(
            r#"[{"$":"obj","id":1,"cls":"Error","proto":"other","props":[],"name":"TypeError","message":"bad"},{"$":"obj","id":2,"cls":"Error","proto":"other","props":[],"name":"Error","message":""}]"#
        )
    );
}

/// A getter that would throw if the encoder called it.
fn throwing_getter(rt: &Rt, _c: &vn_value::Call<'_>) -> vn_value::Res<Value> {
    Err(rt.throw_type_error("getter ran"))
}

#[test]
fn accessors_are_markers_and_getters_never_run() {
    let rt = Rt::new();
    let o = rt.new_object();
    let get_x = rt.new_native_function(throwing_getter, "get x", 0, Vec::new());
    let set_y = rt.new_native_function(noop, "set y", 1, Vec::new());
    let get_z = rt.new_native_function(throwing_getter, "get z", 0, Vec::new());
    let enumerable = Flags::new(false, true, true);
    rt.define_accessor(o, key("x"), Some(get_x), None, enumerable).expect("define");
    rt.define_accessor(o, key("y"), None, Some(set_y), enumerable).expect("define");
    rt.define_accessor(o, key("z"), Some(get_z), Some(set_y), enumerable).expect("define");
    let got = values_of(&rt, vec![ov(o)]);
    assert_eq!(
        got,
        one_call(
            r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[["x",{"$":"accessor","get":true,"set":false}],["y",{"$":"accessor","get":false,"set":true}],["z",{"$":"accessor","get":true,"set":true}]]}]"#
        )
    );
}

#[test]
fn repeats_in_one_call_are_refs_and_the_next_call_expands_again() {
    let rt = Rt::new();
    let o = object(&rt, &[("n", num(1.0))]);
    let got = trace_text(&rt, &[("a", vec![ov(o), ov(o)]), ("b", vec![ov(o)])]);
    let expanded = r#"{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[["n",1]]}"#;
    assert_eq!(got, format!(r#"[{{"label":"a","values":[{expanded},{{"$":"ref","id":1}}]}},{{"label":"b","values":[{expanded}]}}]"#));
}

#[test]
fn cycles_are_refs_to_the_enclosing_object() {
    let rt = Rt::new();
    let o = rt.new_object();
    rt.define_data(o, key("self"), ov(o), Flags::ALL).expect("define");
    let got = values_of(&rt, vec![ov(o)]);
    assert_eq!(
        got,
        one_call(r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[["self",{"$":"ref","id":1}]]}]"#)
    );
}

#[test]
fn ordinals_are_global_and_a_rejected_call_keeps_the_ordinals_it_took() {
    let rt = Rt::new();
    let rec = RefCell::new(Recorder::new());
    let first = object(&rt, &[]);
    let opaque = rt.new_native_object(None, Rc::new(Opaque));
    let rejected = trace_call(&rt, &rec, &JsStr::from_str("bad"), &[ov(first), ov(opaque)]);
    assert!(rejected.is_err(), "a host payload is rejected");
    assert!(rec.borrow().entries().is_empty(), "a rejected call records no entry");
    let second = object(&rt, &[]);
    trace_call(&rt, &rec, &JsStr::from_str("ok"), &[ov(second)]).expect("the second call encodes");
    let got = Json::Arr(rec.borrow().entries().to_vec()).to_json();
    assert_eq!(
        got,
        r#"[{"label":"ok","values":[{"$":"obj","id":2,"cls":"Object","proto":"Object.prototype","props":[]}]}]"#
    );
}

#[test]
fn host_payloads_are_untraceable_type_errors() {
    let rt = Rt::new();
    let opaque = rt.new_native_object(None, Rc::new(Opaque));
    let rec = RefCell::new(Recorder::new());
    let Err(Throw(e)) = trace_call(&rt, &rec, &JsStr::from_str("t"), &[ov(opaque)]) else {
        panic!("a host payload must be rejected");
    };
    assert_eq!(rt.get_str(&e, "name").expect("name"), Value::str("TypeError"));
    assert_eq!(rt.get_str(&e, "message").expect("message"), Value::str("untraceable value"));
}

#[test]
fn standalone_encodings_use_a_fresh_expansion_set() {
    let rt = Rt::new();
    let rec = RefCell::new(Recorder::new());
    let o = object(&rt, &[]);
    let first = encode_standalone(&rt, &rec, &ov(o)).expect("encodes").to_json();
    let second = encode_standalone(&rt, &rec, &ov(o)).expect("encodes").to_json();
    let expected = r#"{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[]}"#;
    assert_eq!(first, expected);
    assert_eq!(second, expected, "the second standalone encoding is not a ref");
}

#[test]
fn frozen_object_has_integrity_after_proto_and_before_props() {
    let rt = Rt::new();
    let o = object(&rt, &[("a", num(1.0))]);
    rt.freeze(o).expect("freeze");
    let got = values_of(&rt, vec![ov(o)]);
    assert_eq!(
        got,
        one_call(r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","integrity":"frozen","props":[["a",1]]}]"#)
    );
}

#[test]
fn sealed_object_has_integrity_sealed() {
    let rt = Rt::new();
    let o = object(&rt, &[("a", num(1.0))]);
    rt.seal(o).expect("seal");
    let got = values_of(&rt, vec![ov(o)]);
    assert_eq!(
        got,
        one_call(r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","integrity":"sealed","props":[["a",1]]}]"#)
    );
}

#[test]
fn preventextensions_object_has_integrity_nonextensible() {
    let rt = Rt::new();
    let o = object(&rt, &[("a", num(1.0))]);
    rt.prevent_extensions(o);
    let got = values_of(&rt, vec![ov(o)]);
    assert_eq!(
        got,
        one_call(r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","integrity":"nonextensible","props":[["a",1]]}]"#)
    );
}

#[test]
fn empty_object_after_preventextensions_is_frozen() {
    let rt = Rt::new();
    let o = rt.new_object();
    rt.prevent_extensions(o);
    let got = values_of(&rt, vec![ov(o)]);
    assert_eq!(got, one_call(r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","integrity":"frozen","props":[]}]"#));
}

#[test]
fn extensible_object_has_no_integrity_field() {
    let rt = Rt::new();
    let o = object(&rt, &[("a", num(1.0))]);
    let got = values_of(&rt, vec![ov(o)]);
    assert_eq!(got, one_call(r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[["a",1]]}]"#));
}

#[test]
fn frozen_array_has_integrity_before_props_elems_and_len() {
    let rt = Rt::new();
    let a = rt.new_array(vec![num(1.0), num(2.0)]);
    rt.freeze(a).expect("freeze");
    let got = values_of(&rt, vec![ov(a)]);
    assert_eq!(
        got,
        one_call(r#"[{"$":"obj","id":1,"cls":"Array","proto":"other","integrity":"frozen","props":[],"elems":[1,2],"len":2}]"#)
    );
}

#[test]
fn sealed_and_nonextensible_arrays() {
    let rt = Rt::new();
    let sealed = rt.new_array(vec![num(1.0)]);
    rt.seal(sealed).expect("seal");
    let open = rt.new_array(vec![num(7.0), num(8.0)]);
    rt.prevent_extensions(open);
    let got = values_of(&rt, vec![ov(sealed), ov(open)]);
    assert_eq!(
        got,
        one_call(
            r#"[{"$":"obj","id":1,"cls":"Array","proto":"other","integrity":"sealed","props":[],"elems":[1],"len":1},{"$":"obj","id":2,"cls":"Array","proto":"other","integrity":"nonextensible","props":[],"elems":[7,8],"len":2}]"#
        )
    );
}

#[test]
fn frozen_error_keeps_name_and_message() {
    let rt = Rt::new();
    let e = construct(&rt, Intrinsic::TypeErrorCtor, &[s("x")]);
    rt.freeze(e.as_object().expect("object")).expect("freeze");
    let got = values_of(&rt, vec![e]);
    assert_eq!(
        got,
        one_call(
            r#"[{"$":"obj","id":1,"cls":"Error","proto":"other","integrity":"frozen","props":[],"name":"TypeError","message":"x"}]"#
        )
    );
}

#[test]
fn a_ref_carries_no_integrity_and_functions_carry_none_either() {
    let rt = Rt::new();
    let o = object(&rt, &[]);
    rt.freeze(o).expect("freeze");
    let f = rt.new_function(noop, FnInfo::new("named", 0, FnKind::Normal), &[]);
    rt.freeze(f).expect("freeze");
    let got = values_of(&rt, vec![ov(o), ov(o), ov(f)]);
    assert_eq!(
        got,
        one_call(
            r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","integrity":"frozen","props":[]},{"$":"ref","id":1},{"$":"fn","name":"named","length":0}]"#
        )
    );
}

#[test]
fn integrity_is_read_at_encode_time() {
    let rt = Rt::new();
    let o = object(&rt, &[("a", num(1.0))]);
    let before = trace_text(&rt, &[("a", vec![ov(o)])]);
    rt.freeze(o).expect("freeze");
    let after = trace_text(&rt, &[("b", vec![ov(o)])]);
    assert_eq!(before, r#"[{"label":"a","values":[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[["a",1]]}]}]"#);
    assert_eq!(
        after,
        r#"[{"label":"b","values":[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","integrity":"frozen","props":[["a",1]]}]}]"#
    );
}

#[test]
fn negative_control_a_doctored_expectation_is_reported() {
    let rt = Rt::new();
    let o = object(&rt, &[("a", num(1.0))]);
    rt.seal(o).expect("seal");
    let actual = values_of(&rt, vec![ov(o)]);
    let expected = one_call(r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","integrity":"sealed","props":[["a",1]]}]"#);
    assert_eq!(actual, expected, "the real expectation holds");
    let doctored = expected.replacen("\"sealed\"", "\"frozen\"", 1);
    let report = first_difference(&doctored, &actual).expect("a doctored expectation must be reported");
    assert!(report.contains("frozen") && report.contains("sealed"), "the report shows both texts: {report}");
}

#[test]
fn negative_control_a_missing_field_is_reported() {
    let rt = Rt::new();
    let o = object(&rt, &[("a", num(1.0))]);
    rt.prevent_extensions(o);
    let actual = values_of(&rt, vec![ov(o)]);
    let without_field = one_call(r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[["a",1]]}]"#);
    let report = first_difference(&without_field, &actual).expect("a missing field must be reported");
    assert!(report.contains("nonextensible"), "the report names the field the encoder wrote: {report}");
}

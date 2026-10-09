//! The corpus replay harness (WP-T1, docs/dev/builtin-corpus.md), unit-tested against a hand-written mini suite
//! whose targets are native functions defined in `setup`. The expected values were derived by hand from the
//! format and the preset bodies, not from the harness. Includes negative controls and the missing-target report.

mod support;

use vn_testkit::{replay_suite, ReplayOptions};
use vn_value::{Call, Flags, PropKey, Res, Rt, Value};

const LOG_EMPTY: &str = r#"{"$":"obj","id":1,"cls":"Array","proto":"other","props":[],"elems":[],"len":0}"#;

fn mini_add(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let a = rt.to_number(&c.arg(0))?;
    let b = rt.to_number(&c.arg(1))?;
    Ok(Value::Number(a + b))
}

/// Array.prototype.map without the thisArg: calls the callback with (element, index).
fn mini_map(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let arr = c.arg(0);
    let cb = c.arg(1);
    let len = match rt.get_str(&arr, "length")? {
        Value::Number(n) => n as u32,
        _ => 0,
    };
    let mut out = Vec::new();
    for i in 0..len {
        let x = rt.get_v(&arr, &PropKey::Index(i))?;
        out.push(rt.call(&cb, &Value::Undefined, &[x, Value::Number(f64::from(i))])?);
    }
    Ok(Value::Object(rt.new_array(out)))
}

/// A method: `this` is the first decoded value of pre (the replay passes it through unchanged).
fn mini_twice(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Number(rt.to_number(&c.this.clone())? * 2.0))
}

fn mini_boom(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_type_error("boom"))
}

fn mini_sqrt2(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Number(2f64.sqrt()))
}

fn mini_is_frozen(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    match c.arg(0) {
        Value::Object(o) => Ok(Value::Bool(rt.is_frozen(o)?)),
        _ => Ok(Value::Bool(false)),
    }
}

fn mini_box_call(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_type_error("Box must be constructed"))
}

fn mini_box_construct(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = rt.new_object();
    rt.define_data(o, PropKey::from_str("value"), c.arg(0), Flags::ALL)?;
    Ok(Value::Object(o))
}

/// The mini realm: `Mini.add`, `Mini.map`, `Mini.boom`, `Mini.sqrt2`, `Mini.isFrozen`, `Mini.PI` and `new Mini.Box`.
fn setup_mini(rt: &Rt) {
    let mini = rt.new_object();
    let method = |name: &str, code: vn_value::CodeFn, length: u32| {
        let f = rt.new_native_function(code, name, length, Vec::new());
        rt.define_data(mini, PropKey::from_str(name), Value::Object(f), Flags::ALL).expect("define");
    };
    method("add", mini_add, 2);
    method("map", mini_map, 2);
    method("boom", mini_boom, 0);
    method("twice", mini_twice, 0);
    method("sqrt2", mini_sqrt2, 0);
    method("isFrozen", mini_is_frozen, 1);
    rt.define_data(mini, PropKey::from_str("PI"), Value::Number(3.14), Flags::ALL).expect("define");
    let box_ctor = rt.new_native_constructor(mini_box_call, mini_box_construct, "Box", 1, Vec::new());
    rt.define_data(mini, PropKey::from_str("Box"), Value::Object(box_ctor), Flags::ALL).expect("define");
    rt.install_global("Mini", Value::Object(mini));
}

fn case(id: &str, body: &str) -> String {
    format!(r#"{{"id":"{id}",{body}}}"#)
}

fn suite(name: &str, cases: &[String]) -> String {
    format!(r#"{{"suite":"{name}","cases":[{}]}}"#, cases.join(","))
}

fn options() -> ReplayOptions {
    ReplayOptions { setup: Some(setup_mini) }
}

/// The exact one-ulp neighbour of sqrt(2), as the oracle would write it.
fn sqrt2_next() -> String {
    vn_value::number::number_to_string(2f64.sqrt().next_up())
}

#[test]
fn a_correct_mini_suite_replays_clean() {
    let cases = [
        case("add/numbers", r#""target":"Mini.add","static":true,"pre":[1,2],"post":{"ret":3,"throw":null,"state":[1,2,LOG]}"#),
        case("add/coerces", r#""target":"Mini.add","static":true,"pre":["2",null],"post":{"ret":2,"throw":null,"state":["2",null,LOG]}"#),
        case(
            "map/index-callback",
            r#""target":"Mini.map","static":true,"cbs":[null,"index"],"pre":[{"$":"obj","id":1,"cls":"Array","proto":"other","props":[],"elems":[10,20,30],"len":3},{"$":"fn","name":"index","length":2}],"post":{"ret":{"$":"obj","id":1,"cls":"Array","proto":"other","props":[],"elems":[0,1,2],"len":3},"throw":null,"state":[{"$":"obj","id":2,"cls":"Array","proto":"other","props":[],"elems":[10,20,30],"len":3},{"$":"fn","name":"index","length":2},{"$":"obj","id":3,"cls":"Array","proto":"other","props":[],"elems":[{"$":"obj","id":4,"cls":"Array","proto":"other","props":[],"elems":["index",10,0],"len":3},{"$":"obj","id":5,"cls":"Array","proto":"other","props":[],"elems":["index",20,1],"len":3},{"$":"obj","id":6,"cls":"Array","proto":"other","props":[],"elems":["index",30,2],"len":3}],"len":3}]}"#,
        ),
        case("method/this", r#""target":"Mini.twice","pre":[21],"post":{"ret":42,"throw":null,"state":[21,LOG]}"#),
        case("boom/throws", r#""target":"Mini.boom","static":true,"pre":[],"post":{"ret":null,"throw":{"name":"TypeError"},"state":[LOG]}"#),
        case("get/PI", r#""target":"Mini.PI","get":1,"pre":[],"post":{"ret":3.14,"throw":null,"state":[LOG]}"#),
        case(
            "new/box",
            r#""target":"new:Mini.Box","pre":[5],"post":{"ret":{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[["value",5]]},"throw":null,"state":[5,{"$":"obj","id":2,"cls":"Array","proto":"other","props":[],"elems":[],"len":0}]}"#,
        ),
        case(
            "frozen/input",
            r#""target":"Mini.isFrozen","static":true,"pre":[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","integrity":"frozen","props":[["a",1]]}],"post":{"ret":true,"throw":null,"state":[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","integrity":"frozen","props":[["a",1]]},{"$":"obj","id":2,"cls":"Array","proto":"other","props":[],"elems":[],"len":0}]}"#,
        ),
        case(
            "approx/one-ulp",
            &format!(r#""target":"Mini.sqrt2","static":true,"approx":1,"pre":[],"post":{{"ret":{},"throw":null,"state":[LOG]}}"#, sqrt2_next()),
        ),
    ]
    .map(|c| c.replace("LOG", LOG_EMPTY));
    let report = replay_suite(&suite("mini-clean", &cases), &options());
    assert_eq!(report.failures, Vec::new(), "no case fails: {:?}", report.failures);
    assert!(report.missing.is_empty(), "every target exists");
    assert_eq!(report.total, cases.len());
    assert_eq!(report.passed, cases.len());
    assert!(report.is_clean());
}

#[test]
fn negative_controls_are_reported_as_failures_with_both_texts() {
    let cases = [
        case("neg/doctored-ret", r#""target":"Mini.add","static":true,"pre":[1,2],"post":{"ret":4,"throw":null,"state":[1,2,LOG]}"#),
        case(
            "neg/no-approx",
            &format!(r#""target":"Mini.sqrt2","static":true,"pre":[],"post":{{"ret":{},"throw":null,"state":[LOG]}}"#, sqrt2_next()),
        ),
        case("neg/unknown-callback", r#""target":"Mini.map","static":true,"cbs":[null,"nosuch"],"pre":[{"$":"obj","id":1,"cls":"Array","proto":"other","props":[],"elems":[],"len":0},{"$":"fn","name":"nosuch","length":2}],"post":{"ret":null,"throw":null,"state":[]}"#),
    ]
    .map(|c| c.replace("LOG", LOG_EMPTY));
    let report = replay_suite(&suite("mini-negative", &cases), &options());
    assert_eq!(report.total, 3);
    assert_eq!(report.passed, 0, "every negative control must fail");
    let ids: Vec<&str> = report.failures.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(ids, ["neg/doctored-ret", "neg/no-approx", "neg/unknown-callback"]);
    let doctored = &report.failures[0];
    assert!(doctored.reason.contains("post.ret"), "the reason names the path: {}", doctored.reason);
    assert_eq!(doctored.expected, "4");
    assert_eq!(doctored.actual, "3");
    assert!(report.failures[1].reason.contains("post.ret"), "without approx, one ulp is a difference");
    assert!(report.failures[2].reason.contains("unknown callback"), "{}", report.failures[2].reason);
}

#[test]
fn a_missing_target_is_reported_and_is_not_a_failure() {
    let cases = [case("missing/nope", r#""target":"Nope.thing","static":true,"pre":[1],"post":{"ret":null,"throw":null,"state":[1]}"#)];
    let report = replay_suite(&suite("mini-missing", &cases), &options());
    assert!(report.failures.is_empty(), "a missing target is not a failure");
    assert_eq!(report.missing.len(), 1);
    assert_eq!(report.missing[0].target, "Nope.thing");
    assert!(!report.is_clean(), "but the suite is not clean, so the test can require 'no missing'");
    assert_eq!(report.total, 1);
    assert_eq!(report.passed, 0);
}

#[test]
fn a_malformed_suite_is_one_failure_with_no_cases() {
    let report = replay_suite("{\"suite\":\"x\",\"cases\":[", &options());
    assert_eq!(report.total, 0);
    assert_eq!(report.failures.len(), 1);
    assert!(!report.is_clean());
}

#[test]
fn the_report_is_one_line_of_json() {
    let cases = [case("missing/nope", r#""target":"Nope.thing","static":true,"pre":[],"post":{"ret":null,"throw":null,"state":[]}"#)];
    let report = replay_suite(&suite("mini-missing", &cases), &options());
    let text = report.to_json();
    assert!(!text.contains('\n'));
    assert!(text.starts_with(r#"{"suite":"mini-missing","total":1,"passed":0,"failures":[],"missing":[{"id":"missing/nope","target":"Nope.thing"}]}"#), "{text}");
}

//! WP-RT-A vnode scenario replay (tests/ui/runtime-a/corpora/vnodes.json, written by gen-vnodes.mjs from the real
//! vue@3.5.43 package). Each scenario runs in a fresh realm whose global `Vue` is `exports::all`; every step calls an
//! export, and the canonical description of the result (README.md) must equal Node's text. The flag values of
//! flags.json (read from @vue/shared) must equal `shapes`. A negative control checks that a doctored text is reported.

use std::path::PathBuf;

use vn_runtime::exports::install_vue_global;
use vn_runtime::shapes::{PatchFlags, ShapeFlags, SlotFlags};
use vn_runtime::vnode::testing::describe;
use vn_testkit::json::Json;
use vn_testkit::json_in;
use vn_value::{Call, Intrinsic, JsStr, PropKey, Res, Rt, Throw, Value};

fn corpus(name: &str) -> String {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tests", "ui", "runtime-a", "corpora", name].iter().collect();
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn text(j: &Json) -> String {
    j.units().map(String::from_utf16_lossy).unwrap_or_default()
}

fn tag(j: &Json) -> Option<String> {
    j.field("$").and_then(Json::units).map(String::from_utf16_lossy)
}

/// The value of a scenario argument (the language of gen-vnodes.mjs).
fn arg(rt: &Rt, j: &Json, results: &[Value]) -> Res<Value> {
    Ok(match j {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Num(t) => Value::Number(t.parse::<f64>().unwrap_or_else(|_| panic!("bad number {t}"))),
        Json::Str(u) => Value::String(JsStr::from_units(u)),
        Json::Arr(items) => {
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                out.push(if tag(it).as_deref() == Some("hole") { Value::Hole } else { arg(rt, it, results)? });
            }
            Value::Object(rt.new_array(out))
        }
        Json::Obj(_) => match tag(j).as_deref() {
            Some("undef") => Value::Undefined,
            Some("num") => Value::Number(match text(j.field("v").unwrap_or(&Json::Null)).as_str() {
                "-0" => -0.0,
                "NaN" => f64::NAN,
                "Infinity" => f64::INFINITY,
                "-Infinity" => f64::NEG_INFINITY,
                other => panic!("bad num tag {other}"),
            }),
            Some("ref") => {
                let step = j.field("step").and_then(Json::num_text).and_then(|t| t.parse::<usize>().ok()).expect("ref step");
                results[step].clone()
            }
            Some("export") => {
                let vue = rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), "Vue")?;
                rt.get_str(&vue, &text(j.field("name").unwrap_or(&Json::Null)))?
            }
            Some("sym") => Value::Symbol(rt.symbol_for(&JsStr::from_str(&text(j.field("name").unwrap_or(&Json::Null))))),
            Some("obj") => {
                let o = rt.new_object();
                for pair in j.field("props").and_then(Json::items).unwrap_or(&[]) {
                    let items = pair.items().expect("prop pair");
                    let key = PropKey::from_jsstr(JsStr::from_units(items[0].units().expect("key")));
                    let v = arg(rt, &items[1], results)?;
                    rt.define_data(o, key, v, vn_value::Flags::ALL)?;
                }
                Value::Object(o)
            }
            Some("fn") => {
                let ret = match j.field("returns") {
                    Some(r) => arg(rt, r, results)?,
                    None => Value::Null,
                };
                let name = text(j.field("name").unwrap_or(&Json::Null));
                let length = j.field("length").and_then(Json::num_text).and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                let f = rt.new_native_function(return_data, &name, length, vec![ret]);
                for pair in j.field("props").and_then(Json::items).unwrap_or(&[]) {
                    let items = pair.items().expect("prop pair");
                    let key = PropKey::from_jsstr(JsStr::from_units(items[0].units().expect("key")));
                    let v = arg(rt, &items[1], results)?;
                    rt.define_data(f, key, v, vn_value::Flags::ALL)?;
                }
                Value::Object(f)
            }
            other => panic!("unknown argument tag {other:?}"),
        },
    })
}

/// The code of the `fn` arguments: returns the value given in the scenario.
fn return_data(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(rt.fn_data(c.callee).first().cloned().unwrap_or(Value::Undefined))
}

/// `throw:<name>` for a thrown error, as the generator writes it.
fn thrown_name(rt: &Rt, e: &Throw) -> String {
    match rt.get_str(&e.0, "name") {
        Ok(Value::String(s)) => format!("throw:{}", s.to_rust_string_lossy()),
        _ => "throw:<unnamed>".to_string(),
    }
}

/// Runs one scenario and returns the texts of its steps.
fn run_steps(rt: &Rt, steps: &[Json]) -> Vec<String> {
    let mut results: Vec<Value> = Vec::new();
    let mut out = Vec::new();
    let vue = rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), "Vue").expect("global Vue");
    for step in steps {
        let name = text(step.field("call").expect("call"));
        let args: Vec<Value> = match step.field("args").and_then(Json::items) {
            Some(items) => items.iter().map(|a| arg(rt, a, &results)).collect::<Res<Vec<Value>>>().expect("argument decodes"),
            None => Vec::new(),
        };
        let f = rt.get_str(&vue, &name).expect("export");
        match rt.call(&f, &Value::Undefined, &args) {
            Ok(v) => {
                out.push(describe(rt, &v));
                results.push(v);
            }
            Err(e) => {
                out.push(thrown_name(rt, &e));
                results.push(Value::Undefined);
            }
        }
    }
    out
}

/// Replays every scenario of a document; returns (scenarios, steps, mismatch descriptions). A scenario marked
/// `unsupported` (a documented deviation) must make the port throw a TypeError at the marked step; it is not a mismatch.
fn replay(text_doc: &str) -> (usize, usize, Vec<String>) {
    let doc = json_in::parse(text_doc).expect("scenario JSON");
    let scenarios = doc.field("scenarios").and_then(Json::items).expect("scenarios");
    let (mut steps_total, mut mismatches) = (0usize, Vec::new());
    for sc in scenarios {
        let id = text(sc.field("id").expect("id"));
        let deviation = sc.field("unsupported").is_some();
        let steps = sc.field("steps").and_then(Json::items).expect("steps");
        let expect: Vec<String> = sc.field("expect").and_then(Json::items).expect("expect").iter().map(text).collect();
        let rt = Rt::new();
        install_vue_global(&rt);
        let got = run_steps(&rt, steps);
        steps_total += steps.len();
        if got.len() != expect.len() {
            mismatches.push(format!("{id}: {} steps, {} expectations", got.len(), expect.len()));
            continue;
        }
        for (i, (g, e)) in got.iter().zip(&expect).enumerate() {
            if deviation && g == "throw:TypeError" && e.starts_with("{\"$\"") {
                continue;
            }
            if g != e {
                mismatches.push(format!("{id} step {i}:\n  expected {e}\n  actual   {g}"));
            }
        }
    }
    (scenarios.len(), steps_total, mismatches)
}

#[test]
fn vnode_scenarios_replay_clean() {
    let (scenarios, steps, mismatches) = replay(&corpus("vnodes.json"));
    assert!(scenarios >= 250, "only {scenarios} scenarios");
    assert!(mismatches.is_empty(), "{} of {scenarios} scenarios ({steps} steps) differ:\n{}", mismatches.len(), mismatches.iter().take(8).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn negative_control_detects_a_doctored_expectation() {
    let text_doc = corpus("vnodes.json");
    let from = "\\\"shapeFlag\\\":9,";
    assert!(text_doc.contains(from), "the control needs a shapeFlag 9 expectation");
    let doctored = text_doc.replacen(from, "\\\"shapeFlag\\\":8,", 1);
    let (_, _, mismatches) = replay(&doctored);
    assert!(!mismatches.is_empty(), "a doctored expectation must be reported");
}

fn number_field(doc: &Json, group: &str, name: &str) -> Option<i64> {
    doc.field(group)?.field(name).and_then(Json::num_text).and_then(|t| t.parse::<i64>().ok())
}

#[test]
fn flags_match_vue_shared() {
    let doc = json_in::parse(&corpus("flags.json")).expect("flags JSON");
    let checks: [(&str, &str, i64); 28] = [
        ("PatchFlags", "TEXT", PatchFlags::TEXT as i64), ("PatchFlags", "CLASS", PatchFlags::CLASS as i64),
        ("PatchFlags", "STYLE", PatchFlags::STYLE as i64), ("PatchFlags", "PROPS", PatchFlags::PROPS as i64),
        ("PatchFlags", "FULL_PROPS", PatchFlags::FULL_PROPS as i64), ("PatchFlags", "NEED_HYDRATION", PatchFlags::NEED_HYDRATION as i64),
        ("PatchFlags", "STABLE_FRAGMENT", PatchFlags::STABLE_FRAGMENT as i64), ("PatchFlags", "KEYED_FRAGMENT", PatchFlags::KEYED_FRAGMENT as i64),
        ("PatchFlags", "UNKEYED_FRAGMENT", PatchFlags::UNKEYED_FRAGMENT as i64), ("PatchFlags", "NEED_PATCH", PatchFlags::NEED_PATCH as i64),
        ("PatchFlags", "DYNAMIC_SLOTS", PatchFlags::DYNAMIC_SLOTS as i64), ("PatchFlags", "DEV_ROOT_FRAGMENT", PatchFlags::DEV_ROOT_FRAGMENT as i64),
        ("PatchFlags", "CACHED", PatchFlags::CACHED as i64), ("PatchFlags", "BAIL", PatchFlags::BAIL as i64),
        ("ShapeFlags", "ELEMENT", ShapeFlags::ELEMENT as i64), ("ShapeFlags", "FUNCTIONAL_COMPONENT", ShapeFlags::FUNCTIONAL_COMPONENT as i64),
        ("ShapeFlags", "STATEFUL_COMPONENT", ShapeFlags::STATEFUL_COMPONENT as i64), ("ShapeFlags", "TEXT_CHILDREN", ShapeFlags::TEXT_CHILDREN as i64),
        ("ShapeFlags", "ARRAY_CHILDREN", ShapeFlags::ARRAY_CHILDREN as i64), ("ShapeFlags", "SLOTS_CHILDREN", ShapeFlags::SLOTS_CHILDREN as i64),
        ("ShapeFlags", "TELEPORT", ShapeFlags::TELEPORT as i64), ("ShapeFlags", "SUSPENSE", ShapeFlags::SUSPENSE as i64),
        ("ShapeFlags", "COMPONENT_SHOULD_KEEP_ALIVE", ShapeFlags::COMPONENT_SHOULD_KEEP_ALIVE as i64),
        ("ShapeFlags", "COMPONENT_KEPT_ALIVE", ShapeFlags::COMPONENT_KEPT_ALIVE as i64), ("ShapeFlags", "COMPONENT", ShapeFlags::COMPONENT as i64),
        ("SlotFlags", "STABLE", SlotFlags::STABLE as i64), ("SlotFlags", "DYNAMIC", SlotFlags::DYNAMIC as i64),
        ("SlotFlags", "FORWARDED", SlotFlags::FORWARDED as i64),
    ];
    let mut seen = 0;
    for (group, name, value) in checks {
        assert_eq!(number_field(&doc, group, name), Some(value), "{group}.{name}");
        seen += 1;
    }
    assert_eq!(seen, 28);
}

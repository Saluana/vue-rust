//! The wasm entry (`vn_testkit::wasm`, task 4.2) on the native target. Each document it produces is the document the isolated
//! runner produces for the same entry, which is the text the native scenario binary prints. The wasm build therefore
//! reports what the native build reports. The wasm32 build itself is exercised by tests/language/wasm.test.mjs.

use vn_testkit::wasm::{self, NO_SUCH_SCENARIO};
use vn_testkit::{run_scenario_isolated_with_setup, run_scenario_with_gc_threshold, ScenarioEntry, ScenarioFn};
use vn_value::rt::DEFAULT_GC_THRESHOLD;
use vn_value::{Call, FnInfo, FnKind, Intrinsic, Res, Rt, Throw, Value};

fn no_setup(_rt: &Rt) {}

/// `trace(label, value)` through the realm's global, as a scenario module calls it.
fn trace_one(rt: &Rt, label: &str, v: Value) -> Res<()> {
    let trace = rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), "trace")?;
    rt.call(&trace, &Value::Undefined, &[Value::str(label), v])?;
    Ok(())
}

fn entry_ok(rt: &Rt) -> Res<ScenarioEntry> {
    trace_one(rt, "module-init", Value::Bool(true))?;
    let run = rt.new_function(run_body, FnInfo::new("run", 0, FnKind::Normal), &[]);
    Ok(ScenarioEntry { run: Value::Object(run) })
}

fn run_body(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    trace_one(rt, "in-run", Value::Number(2.0))?;
    Ok(Value::Undefined)
}

fn entry_throws(_rt: &Rt) -> Res<ScenarioEntry> {
    Err(Throw(Value::str("boom")))
}

fn entry_panics(_rt: &Rt) -> Res<ScenarioEntry> {
    panic!("deliberate panic (native test)")
}

const TABLE: &[(&str, ScenarioFn)] = &[
    ("ok", entry_ok as ScenarioFn),
    ("module-throws", entry_throws as ScenarioFn),
    ("panics", entry_panics as ScenarioFn),
];

fn output_text() -> String {
    String::from_utf8(wasm::output()).expect("the document is UTF-8")
}

#[test]
fn documents_equal_the_isolated_runner() {
    for (i, (name, entry)) in TABLE.iter().enumerate() {
        let len = wasm::run_scenario(TABLE, i as u32, no_setup);
        let got = output_text();
        assert_eq!(len as usize, got.len(), "{name}: the returned length is the document's length");
        let expected = run_scenario_isolated_with_setup(name, *entry, no_setup);
        assert_eq!(got, expected, "{name}");
    }
}

#[test]
fn documents_equal_the_runner_at_threshold_one() {
    // The stress run (a collection at every safepoint) gives the document the runner gives at that threshold.
    for (i, (name, entry)) in TABLE.iter().enumerate().take(2) {
        wasm::run_scenario_with_gc_threshold(TABLE, i as u32, 1, no_setup);
        assert_eq!(output_text(), run_scenario_with_gc_threshold(name, *entry, 1), "{name} at threshold 1");
    }
}

#[test]
fn panic_is_a_panic_document_on_native() {
    // Natively a panic unwinds to the entry (on wasm32 it is a trap, reported by the host instead).
    wasm::run_scenario(TABLE, 2, no_setup);
    assert!(output_text().contains("\"result\":\"panic\""), "{}", output_text());
}

#[test]
fn names_count_and_bounds() {
    assert_eq!(wasm::scenario_count(TABLE), 3);
    let len = wasm::scenario_name(TABLE, 1);
    assert_eq!(output_text(), "module-throws");
    assert_eq!(len as usize, "module-throws".len());
    assert_eq!(wasm::scenario_name(TABLE, 3), NO_SUCH_SCENARIO);
    assert_eq!(wasm::run_scenario(TABLE, 3, no_setup), NO_SUCH_SCENARIO);
}

// Negative control: the document of one entry is not the document of another, so a wrong expectation is detected.
#[test]
fn negative_control_a_different_entry_is_detected() {
    wasm::run_scenario(TABLE, 0, no_setup);
    let got = output_text();
    assert!(got.contains("\"result\":\"ok\""), "{got}");
    let other = run_scenario_isolated_with_setup("module-throws", entry_throws, no_setup);
    assert_ne!(got, other);
}

// Negative control of the stress threshold: the run body records how many collections the realm has made. With the default
// threshold this small scenario makes none; with 1 the drain between module evaluation and `run` collects first.
const GC_TABLE: &[(&str, ScenarioFn)] = &[("collections", entry_collections as ScenarioFn)];

fn entry_collections(rt: &Rt) -> Res<ScenarioEntry> {
    let run = rt.new_function(collections_body, FnInfo::new("run", 0, FnKind::Normal), &[]);
    Ok(ScenarioEntry { run: Value::Object(run) })
}

fn collections_body(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    trace_one(rt, "collections", Value::Number(rt.heap_stats().collections as f64))?;
    Ok(Value::Undefined)
}

#[test]
fn the_threshold_reaches_the_collector() {
    wasm::run_scenario_with_gc_threshold(GC_TABLE, 0, DEFAULT_GC_THRESHOLD as u32, no_setup);
    let default = output_text();
    wasm::run_scenario_with_gc_threshold(GC_TABLE, 0, 1, no_setup);
    let stressed = output_text();
    assert!(default.contains(r#""label":"collections","values":[0]"#), "{default}");
    assert!(!stressed.contains(r#""values":[0]"#), "a threshold of 1 collects before run: {stressed}");
}

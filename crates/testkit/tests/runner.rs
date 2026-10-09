//! The scenario runner (WP-T1): the documents of steps 1-5, the isolated runner (panics become documents, deep
//! recursion stays a RangeError), and the GC contract (the document does not depend on the collection schedule).

mod support;

use std::sync::atomic::{AtomicU64, Ordering};

use support::{key, noop, num, ov, s};
use vn_testkit::{run_scenario, run_scenario_isolated, run_scenario_with_gc_threshold, ScenarioEntry};
use vn_value::builtins::promise::new_promise;
use vn_value::{Call, Flags, FnInfo, FnKind, Intrinsic, Res, Rt, Throw, Value};

fn global_trace(rt: &Rt) -> Value {
    rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), "trace").expect("trace is installed")
}

/// `trace(label, ...values)` through the realm's global, as the scenario module calls it.
fn trace_in(rt: &Rt, label: &str, values: &[Value]) -> Res<()> {
    let mut args = vec![s(label)];
    args.extend_from_slice(values);
    rt.call(&global_trace(rt), &Value::Undefined, &args)?;
    Ok(())
}

fn make_error(rt: &Rt, kind: Intrinsic, message: &str) -> Throw {
    let ctor = Value::Object(rt.intrinsic(kind));
    Throw(rt.construct(&ctor, &[s(message)], &ctor).expect("builtin error constructor"))
}

fn entry_ok(rt: &Rt) -> Res<ScenarioEntry> {
    trace_in(rt, "module-init", &[s("ran")])?;
    let run = rt.new_function(run_ok_body, FnInfo::new("run", 0, FnKind::Normal), &[]);
    Ok(ScenarioEntry { run: ov(run) })
}

fn run_ok_body(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    trace_in(rt, "in-run", &[num(2.0)])?;
    Ok(Value::Undefined)
}

fn entry_module_throws(rt: &Rt) -> Res<ScenarioEntry> {
    trace_in(rt, "before", &[])?;
    Err(make_error(rt, Intrinsic::TypeErrorCtor, "boom"))
}

fn entry_run_throws(rt: &Rt) -> Res<ScenarioEntry> {
    trace_in(rt, "module-init", &[s("ran")])?;
    let run = rt.new_function(run_throws_body, FnInfo::new("run", 0, FnKind::Normal), &[]);
    Ok(ScenarioEntry { run: ov(run) })
}

fn run_throws_body(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    trace_in(rt, "in-run", &[num(2.0)])?;
    Err(make_error(rt, Intrinsic::RangeErrorCtor, "r"))
}

fn entry_bad_run(_rt: &Rt) -> Res<ScenarioEntry> {
    Ok(ScenarioEntry { run: num(5.0) })
}

fn entry_throws_untraceable(rt: &Rt) -> Res<ScenarioEntry> {
    let payload = rt.new_native_object(None, std::rc::Rc::new(Opaque));
    Err(Throw(ov(payload)))
}

struct Opaque;

impl vn_value::NativeObject for Opaque {
    fn trace(&self, _t: &mut vn_value::Tracer) {}
}

fn entry_panics(rt: &Rt) -> Res<ScenarioEntry> {
    trace_in(rt, "before-panic", &[])?;
    panic!("framework bug marker");
}

fn recurse(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    rt.call(&Value::Object(c.callee), &Value::Undefined, &[])
}

fn entry_deep(rt: &Rt) -> Res<ScenarioEntry> {
    let f = rt.new_native_function(recurse, "recurse", 0, Vec::new());
    rt.call(&ov(f), &Value::Undefined, &[])?;
    Ok(ScenarioEntry { run: Value::Undefined })
}

/// The run export keeps an object alive only through its closure data, and traces it after garbage.
fn entry_gc(rt: &Rt) -> Res<ScenarioEntry> {
    let held = rt.new_object();
    rt.define_data(held, key("k"), num(42.0), Flags::ALL)?;
    trace_in(rt, "module-init", &[ov(held)])?;
    let run = rt.new_native_function(run_gc_body, "run", 0, vec![ov(held)]);
    Ok(ScenarioEntry { run: ov(run) })
}

static COLLECTIONS_SEEN_BY_RUN: AtomicU64 = AtomicU64::new(0);

fn run_gc_body(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    COLLECTIONS_SEEN_BY_RUN.store(rt.heap_stats().collections, Ordering::SeqCst);
    let held = rt.fn_data(c.callee)[0].clone();
    for _ in 0..50 {
        let _ = rt.new_object();
    }
    trace_in(rt, "in-run", &[held])?;
    Ok(Value::Undefined)
}

#[test]
fn an_ok_scenario_records_module_and_run_entries_in_order() {
    assert_eq!(
        run_scenario("ok", entry_ok),
        r#"{"scenario":"ok","result":"ok","error":null,"entries":[{"label":"module-init","values":["ran"]},{"label":"in-run","values":[2]}],"unhandledRejections":[]}"#
    );
}

#[test]
fn a_module_throw_is_the_result_and_run_is_not_called() {
    assert_eq!(
        run_scenario("module-throws", entry_module_throws),
        concat!(
            r#"{"scenario":"module-throws","result":"throw","#,
            r#""error":{"$":"obj","id":1,"cls":"Error","proto":"other","props":[],"name":"TypeError","message":"boom"},"#,
            r#""entries":[{"label":"before","values":[]}],"unhandledRejections":[]}"#
        )
    );
}

#[test]
fn a_run_throw_is_the_result_after_the_earlier_entries() {
    assert_eq!(
        run_scenario("run-throws", entry_run_throws),
        concat!(
            r#"{"scenario":"run-throws","result":"throw","#,
            r#""error":{"$":"obj","id":1,"cls":"Error","proto":"other","props":[],"name":"RangeError","message":"r"},"#,
            r#""entries":[{"label":"module-init","values":["ran"]},{"label":"in-run","values":[2]}],"unhandledRejections":[]}"#
        )
    );
}

#[test]
fn a_run_export_that_is_not_a_function_is_a_harness_panic() {
    assert_eq!(
        run_scenario("bad-run", entry_bad_run),
        r#"{"scenario":"bad-run","result":"panic","error":"the scenario export \"run\" is not a function","entries":[],"unhandledRejections":[]}"#
    );
}

#[test]
fn a_thrown_value_that_cannot_be_encoded_is_a_panic_document_not_a_crash() {
    assert_eq!(
        run_scenario("untraceable-throw", entry_throws_untraceable),
        r#"{"scenario":"untraceable-throw","result":"panic","error":"TypeError: untraceable value","entries":[],"unhandledRejections":[]}"#
    );
}

#[test]
fn an_isolated_panic_is_a_document_with_the_message_and_the_earlier_entries() {
    assert_eq!(
        run_scenario_isolated("panics", entry_panics),
        r#"{"scenario":"panics","result":"panic","error":"framework bug marker","entries":[{"label":"before-panic","values":[]}],"unhandledRejections":[]}"#
    );
}

#[test]
fn deep_recursion_is_a_range_error_on_the_isolated_thread() {
    assert_eq!(
        run_scenario_isolated("deep", entry_deep),
        r#"{"scenario":"deep","result":"throw","error":{"$":"obj","id":1,"cls":"Error","proto":"other","props":[],"name":"RangeError","message":"Maximum call stack size exceeded"},"entries":[],"unhandledRejections":[]}"#
    );
}

#[test]
fn the_isolated_runner_and_the_plain_runner_give_the_same_document() {
    assert_eq!(run_scenario_isolated("ok", entry_ok), run_scenario("ok", entry_ok));
}

#[test]
fn collecting_at_every_safepoint_does_not_change_the_document() {
    let expected = concat!(
        r#"{"scenario":"gc","result":"ok","error":null,"entries":["#,
        r#"{"label":"module-init","values":[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[["k",42]]}]},"#,
        r#"{"label":"in-run","values":[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[["k",42]]}]}"#,
        r#"],"unhandledRejections":[]}"#
    );
    let default_schedule = run_scenario("gc", entry_gc);
    let at_every_safepoint = run_scenario_with_gc_threshold("gc", entry_gc, 1);
    let seen_with_stress = COLLECTIONS_SEEN_BY_RUN.load(Ordering::SeqCst);
    assert_eq!(default_schedule, expected);
    assert_eq!(at_every_safepoint, expected);
    assert!(seen_with_stress > 0, "the stressed schedule collected before run (saw {seen_with_stress})");
}

// ----- promises at the scenario level (trace-format.md steps 4-5). The expected documents are the Node oracle's
// output for the same scenarios (node tooling/oracle/run.mjs <file>), recorded in each test. -----

fn global_value(rt: &Rt, name: &str) -> Value {
    rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), name).expect("a global of the realm")
}

/// `Promise.<name>(args)` through the realm's global.
fn promise_static(rt: &Rt, name: &str, args: &[Value]) -> Res<Value> {
    let ctor = global_value(rt, "Promise");
    let f = rt.get_str(&ctor, name)?;
    rt.call(&f, &ctor, args)
}

/// `this[name](args)`.
fn invoke(rt: &Rt, this: &Value, name: &str, args: &[Value]) -> Res<Value> {
    let f = rt.get_str(this, name)?;
    rt.call(&f, this, args)
}

/// Traces its closure data: the label and one value.
fn trace_data(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let label = rt.to_string(&d[0])?.to_rust_string_lossy();
    trace_in(rt, &label, &[d[1].clone()])?;
    Ok(Value::Undefined)
}

/// A native function that traces `label` with `value` when called.
fn tracer(rt: &Rt, label: &str, value: Value) -> Value {
    Value::Object(rt.new_native_function(trace_data, label, 0, vec![s(label), value]))
}

fn run_fn(rt: &Rt, body: vn_value::CodeFn) -> ScenarioEntry {
    ScenarioEntry { run: ov(rt.new_function(body, FnInfo::new("run", 0, FnKind::Normal), &[])) }
}

/// `testHost.tick()`.
fn tick(rt: &Rt) -> Res<Value> {
    let host = global_value(rt, "testHost");
    invoke(rt, &host, "tick", &[])
}

fn run_rejects_after_a_trace(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    trace_in(rt, "inside", &[num(1.0)])?;
    promise_static(rt, "reject", &[s("boom")])
}

#[test]
fn a_rejection_returned_by_run_is_the_result_after_the_earlier_entries() {
    // Node: {"scenario":"rejected-run","result":"throw","error":"boom","entries":[{"label":"inside","values":[1]}],"unhandledRejections":[]}
    let doc = run_scenario("rejected-run", |rt| Ok(run_fn(rt, run_rejects_after_a_trace)));
    assert_eq!(
        doc,
        r#"{"scenario":"rejected-run","result":"throw","error":"boom","entries":[{"label":"inside","values":[1]}],"unhandledRejections":[]}"#
    );
}

fn run_micro_then_tick(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    let resolved = promise_static(rt, "resolve", &[Value::Undefined])?;
    invoke(rt, &resolved, "then", &[tracer(rt, "micro", num(2.0))])?;
    let ticked = tick(rt)?;
    invoke(rt, &ticked, "then", &[tracer(rt, "after-tick", num(1.0))])?;
    Ok(Value::Undefined)
}

#[test]
fn run_waits_for_the_queue_so_the_tick_continuation_is_in_the_document() {
    // Node: micro (the job queued first), then after-tick (the continuation of testHost.tick()).
    let doc = run_scenario("tick-run", |rt| Ok(run_fn(rt, run_micro_then_tick)));
    assert_eq!(
        doc,
        r#"{"scenario":"tick-run","result":"ok","error":null,"entries":[{"label":"micro","values":[2]},{"label":"after-tick","values":[1]}],"unhandledRejections":[]}"#
    );
}

fn module_rejects_first(rt: &Rt) -> Res<ScenarioEntry> {
    promise_static(rt, "reject", &[s("first")])?;
    Ok(run_fn(rt, run_rejects_second_then_ticks))
}

fn run_rejects_second_then_ticks(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    promise_static(rt, "reject", &[s("second")])?;
    let ticked = tick(rt)?;
    invoke(rt, &ticked, "then", &[tracer(rt, "done", Value::Bool(true))])?;
    Ok(Value::Undefined)
}

#[test]
fn unhandled_rejections_are_encoded_in_rejection_order() {
    // Node: "unhandledRejections":["first","second"] (both rejected, never handled).
    let doc = run_scenario("unhandled-order", module_rejects_first);
    assert_eq!(
        doc,
        r#"{"scenario":"unhandled-order","result":"ok","error":null,"entries":[{"label":"done","values":[true]}],"unhandledRejections":["first","second"]}"#
    );
}

fn module_handles_its_rejection(rt: &Rt) -> Res<ScenarioEntry> {
    let rejected = promise_static(rt, "reject", &[s("caught")])?;
    let handler = Value::Object(rt.new_native_function(noop, "noop", 0, Vec::new()));
    invoke(rt, &rejected, "catch", &[handler])?;
    Ok(run_fn(rt, run_traces_x))
}

fn run_traces_x(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    trace_in(rt, "x", &[num(1.0)])?;
    Ok(Value::Undefined)
}

#[test]
fn a_rejection_with_a_handler_is_not_listed() {
    // Node: "unhandledRejections":[] (the catch handles it before the end of the run).
    let doc = run_scenario("handled-run", module_handles_its_rejection);
    assert_eq!(doc, r#"{"scenario":"handled-run","result":"ok","error":null,"entries":[{"label":"x","values":[1]}],"unhandledRejections":[]}"#);
}

fn run_traces_a_pending_promise(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    let p = new_promise(rt);
    trace_in(rt, "p", &[ov(p)])?;
    Ok(Value::Undefined)
}

#[test]
fn a_promise_is_traced_as_an_opaque_object() {
    // Node: the entry is {"$":"obj","id":1,"cls":"Promise","proto":"other","props":[]}.
    let doc = run_scenario("promise-entry", |rt| Ok(run_fn(rt, run_traces_a_pending_promise)));
    assert_eq!(
        doc,
        r#"{"scenario":"promise-entry","result":"ok","error":null,"entries":[{"label":"p","values":[{"$":"obj","id":1,"cls":"Promise","proto":"other","props":[]}]}],"unhandledRejections":[]}"#
    );
}

fn run_returns_a_pending_promise(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Object(new_promise(rt)))
}

#[test]
fn a_value_returned_by_run_that_never_settles_is_a_harness_panic() {
    // The oracle cannot finish this either (the process exits with "run() never settled"), so the runner reports a
    // panic document rather than a result.
    let doc = run_scenario("never-settles", |rt| Ok(run_fn(rt, run_returns_a_pending_promise)));
    assert!(doc.contains(r#""result":"panic""#) && doc.contains("never settled"), "{doc}");
}

//! The scenario host globals: the fake clock, `trace` through the realm, the oracle's shape of `testHost`, the
//! loud stubs for the promise-returning functions, and the GC contract between steps (WP-T1).

mod support;

use support::{num, ov, s, Realm};
use vn_value::builtins::promise::{promise_state, PromiseState};
use vn_value::{Call, Intrinsic, Res, Rt, Value};

/// `testHost.timers[name](...args)`, called with `this = testHost.timers` as scenario code does.
fn timers_call(rt: &Rt, name: &str, args: &[Value]) -> Res<Value> {
    let host = rt.get_str(&global(rt), "testHost")?;
    let timers = rt.get_str(&host, "timers")?;
    let f = rt.get_str(&timers, name)?;
    rt.call(&f, &timers, args)
}

fn global(rt: &Rt) -> Value {
    Value::Object(rt.intrinsic(Intrinsic::Global))
}

fn global_trace(rt: &Rt) -> Value {
    rt.get_str(&global(rt), "trace").expect("trace is installed")
}

/// A timer callback: it traces its label (its closure data), so the firing order is visible.
fn fire(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let label = rt.fn_data(c.callee)[0].clone();
    rt.call(&global_trace(rt), &Value::Undefined, &[label])
}

fn timer(r: &Realm, label: &str) -> Value {
    ov(r.rt.new_native_function(fire, "fire", 0, vec![s(label)]))
}

/// A timer callback that traces its label and the object in its closure data.
fn fire_object(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let args = [data[0].clone(), data[1].clone()];
    rt.call(&global_trace(rt), &Value::Undefined, &args)
}

/// A timer callback that schedules another timer 3 ms later.
fn schedule_nested(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    let nested = ov(rt.new_native_function(fire, "fire", 0, vec![s("nested")]));
    timers_call(rt, "setTimeout", &[nested, num(3.0)])
}

fn throws(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_type_error("boom"))
}

#[test]
fn timers_fire_in_due_time_then_creation_order() {
    let r = Realm::new();
    let ids: Vec<Value> = [("a", 10.0), ("b", 5.0), ("c", 10.0), ("d", 0.0)]
        .iter()
        .map(|(label, ms)| timers_call(&r.rt, "setTimeout", &[timer(&r, label), num(*ms)]).expect("setTimeout"))
        .collect();
    assert_eq!(ids, vec![num(1.0), num(2.0), num(3.0), num(4.0)], "ids count up from 1");
    assert_eq!(r.timers.pending_count(), 4);
    r.timers.advance_sync(&r.rt, 10.0).expect("advance");
    assert_eq!(
        r.entries_text(),
        r#"[{"label":"d","values":[]},{"label":"b","values":[]},{"label":"a","values":[]},{"label":"c","values":[]}]"#
    );
    assert_eq!(r.timers.now(), 10.0);
    assert_eq!(r.timers.pending_count(), 0);
}

#[test]
fn clear_timeout_cancels_numbers_and_ignores_other_values() {
    let r = Realm::new();
    let x = timers_call(&r.rt, "setTimeout", &[timer(&r, "x"), num(1.0)]).expect("setTimeout");
    timers_call(&r.rt, "setTimeout", &[timer(&r, "y"), num(2.0)]).expect("setTimeout");
    timers_call(&r.rt, "clearTimeout", &[x]).expect("clearTimeout");
    timers_call(&r.rt, "clearTimeout", &[s("2")]).expect("a string names no timer");
    timers_call(&r.rt, "clearTimeout", &[num(99.0)]).expect("an unknown id is a no-op");
    r.timers.advance_sync(&r.rt, 5.0).expect("advance");
    assert_eq!(r.entries_text(), r#"[{"label":"y","values":[]}]"#);
}

#[test]
fn a_callback_can_schedule_a_timer_that_fires_within_the_same_advance() {
    let r = Realm::new();
    let scheduler = ov(r.rt.new_native_function(schedule_nested, "scheduler", 0, Vec::new()));
    timers_call(&r.rt, "setTimeout", &[scheduler, num(5.0)]).expect("setTimeout");
    timers_call(&r.rt, "setTimeout", &[timer(&r, "late"), num(11.0)]).expect("setTimeout");
    r.timers.advance_sync(&r.rt, 10.0).expect("advance");
    assert_eq!(r.entries_text(), r#"[{"label":"nested","values":[]}]"#, "due at 8, inside the advance");
    assert_eq!(r.timers.now(), 10.0);
    assert_eq!(r.timers.pending_count(), 1, "the timer due at 11 is still pending");
}

#[test]
fn a_throwing_callback_ends_the_advance_and_its_error_is_returned() {
    let r = Realm::new();
    let bad = ov(r.rt.new_native_function(throws, "bad", 0, Vec::new()));
    timers_call(&r.rt, "setTimeout", &[bad, num(2.0)]).expect("setTimeout");
    timers_call(&r.rt, "setTimeout", &[timer(&r, "after"), num(3.0)]).expect("setTimeout");
    let err = r.timers.advance_sync(&r.rt, 10.0).expect_err("the callback throws");
    assert_eq!(r.rt.get_str(&err.0, "message").expect("message"), s("boom"));
    assert_eq!(r.timers.now(), 2.0, "the clock stays at the failing timer's due time");
    assert_eq!(r.timers.pending_count(), 1, "the later timer does not fire");
}

#[test]
fn set_timeout_checks_its_callback_and_converts_delays_with_number() {
    let r = Realm::new();
    let err = timers_call(&r.rt, "setTimeout", &[s("not a function"), num(1.0)]).expect_err("a string is refused");
    assert_eq!(r.rt.get_str(&err.0, "message").expect("message"), s("setTimeout callback must be a function"));
    // Delays: undefined and NaN count as 0, "7" is 7, and negative counts as 0.
    timers_call(&r.rt, "setTimeout", &[timer(&r, "u"), Value::Undefined]).expect("setTimeout");
    timers_call(&r.rt, "setTimeout", &[timer(&r, "n"), num(f64::NAN)]).expect("setTimeout");
    timers_call(&r.rt, "setTimeout", &[timer(&r, "s7"), s("7")]).expect("setTimeout");
    timers_call(&r.rt, "setTimeout", &[timer(&r, "neg"), num(-3.0)]).expect("setTimeout");
    r.timers.advance_sync(&r.rt, 0.0).expect("advance by zero");
    assert_eq!(
        r.entries_text(),
        r#"[{"label":"u","values":[]},{"label":"n","values":[]},{"label":"neg","values":[]}]"#
    );
    assert_eq!(r.timers.pending_count(), 1, "the timer with delay 7 is still pending");
}

#[test]
fn testhost_has_the_oracle_shape() {
    // Captured from tooling/oracle/run.mjs on a scratch scenario that traces testHost, its timers and trace.
    let r = Realm::new();
    r.t("host", &[r.rt.get_str(&r.global(), "testHost").expect("testHost")]).expect("trace");
    r.t("trace-fn", &[r.rt.get_str(&r.global(), "trace").expect("trace")]).expect("trace");
    assert_eq!(
        r.entries_text(),
        concat!(
            r#"[{"label":"host","values":[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[["timers",{"$":"obj","id":2,"cls":"Object","proto":"Object.prototype","props":[["setTimeout",{"$":"fn","name":"setTimeout","length":2}],["clearTimeout",{"$":"fn","name":"clearTimeout","length":1}],["now",{"$":"fn","name":"now","length":0}],["advance",{"$":"fn","name":"advance","length":1}]]}],["tick",{"$":"fn","name":"drain","length":0}]]}]},"#,
            r#"{"label":"trace-fn","values":[{"$":"fn","name":"trace","length":1}]}]"#
        )
    );
}

#[test]
fn advance_and_tick_return_promises_that_settle_once_the_queue_drains() {
    let r = Realm::new();
    let p = timers_call(&r.rt, "advance", &[num(1.0)]).expect("advance returns a promise");
    assert_eq!(promise_state(&r.rt, &p), Some(PromiseState::Pending), "pending until the queue drains");
    r.rt.run_jobs();
    assert_eq!(promise_state(&r.rt, &p), Some(PromiseState::Fulfilled(Value::Undefined)));
    assert_eq!(r.timers.now(), 1.0, "the clock moved to the target");
    let host = r.rt.get_str(&r.global(), "testHost").expect("testHost");
    let tick = r.rt.get_str(&host, "tick").expect("tick");
    let t = r.rt.call(&tick, &Value::Undefined, &[]).expect("tick returns a promise");
    assert_eq!(promise_state(&r.rt, &t), Some(PromiseState::Pending));
    r.rt.run_jobs();
    assert_eq!(promise_state(&r.rt, &t), Some(PromiseState::Fulfilled(Value::Undefined)));
}

#[test]
fn advance_rejects_when_a_callback_throws_and_the_clock_stays_at_its_due_time() {
    let r = Realm::new();
    let bad = ov(r.rt.new_native_function(throws, "bad", 0, Vec::new()));
    timers_call(&r.rt, "setTimeout", &[bad, num(2.0)]).expect("setTimeout");
    let p = timers_call(&r.rt, "advance", &[num(10.0)]).expect("advance returns a promise");
    r.rt.run_jobs();
    let Some(PromiseState::Rejected(err)) = promise_state(&r.rt, &p) else { panic!("the advance promise rejects") };
    assert_eq!(r.rt.get_str(&err, "message").expect("message"), s("boom"));
    assert_eq!(r.timers.now(), 2.0, "the clock stays at the failing timer's due time");
}

#[test]
fn trace_requires_a_string_label_at_the_global() {
    let r = Realm::new();
    let err = r.call_trace(&[num(1.0), num(2.0)]).expect_err("a number label is refused");
    assert_eq!(r.rt.get_str(&err.0, "message").expect("message"), s("trace label must be a string"));
    assert_eq!(r.entries_text(), "[]", "a refused call records nothing");
}

#[test]
fn timer_callbacks_and_their_data_survive_collections_between_steps() {
    let r = Realm::new();
    let scheduled = {
        let held = r.obj(&[("k", num(42.0))]);
        ov(r.rt.new_native_function(fire_object, "held", 0, vec![s("held"), ov(held)]))
    };
    timers_call(&r.rt, "setTimeout", &[scheduled, num(1.0)]).expect("setTimeout");
    let before = r.rt.heap_stats();
    for i in 0..100 {
        let _ = r.obj(&[("g", num(f64::from(i)))]); // garbage: nothing refers to these
    }
    for _ in 0..3 {
        r.rt.collect();
    }
    let after = r.rt.heap_stats();
    assert!(after.freed_total > before.freed_total, "the collector really freed the garbage");
    r.timers.advance_sync(&r.rt, 1.0).expect("advance");
    assert_eq!(
        r.entries_text(),
        r#"[{"label":"held","values":[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[["k",42]]}]}]"#
    );
    r.t("after", &[num(1.0)]).expect("the recorder and the globals survive collection");
}

#[test]
fn the_host_survives_a_collection_with_no_scenario_references() {
    let r = Realm::new();
    r.rt.collect();
    r.t("alive", &[ov(r.obj(&[("v", num(1.0))]))]).expect("trace still works");
    let host = r.rt.get_str(&r.global(), "testHost").expect("testHost");
    let timers = r.rt.get_str(&host, "timers").expect("timers");
    let now = r.rt.get_str(&timers, "now").expect("now");
    assert_eq!(r.rt.call(&now, &timers, &[]).expect("now()"), num(0.0));
}

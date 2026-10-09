//! Kernel job queue and watchdog: FIFO draining, jobs queued while draining, error collection that does not
//! stop the drain, queued values as roots, the heap limit and the execution budget (`tick`).

use vn_value::{Call, Flags, PropKey, Res, Rt, Value};

fn key(s: &str) -> PropKey {
    PropKey::from_str(s)
}

fn num(n: f64) -> Value {
    Value::Number(n)
}

fn ok<T>(r: Res<T>) -> T {
    match r {
        Ok(v) => v,
        Err(_) => panic!("unexpected JS throw"),
    }
}

fn text(rt: &Rt, v: &Value, name: &str) -> String {
    match rt.get_str(v, name) {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => String::from("<none>"),
    }
}

/// Appends `arg(1)` to the array `arg(0)`.
fn append(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let log = c.arg(0);
    let len = match rt.get_str(&log, "length")? {
        Value::Number(n) => n as u32,
        _ => 0,
    };
    rt.set_v_strict(&log, &PropKey::Index(len), c.arg(1))?;
    Ok(Value::Undefined)
}

fn fail(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_type_error("boom"))
}

/// Appends a new job to the queue (`arg(0)` is the log, `arg(1)` the value to log for the new job).
fn enqueue_follow_up(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let append_fn = rt.fn_data(c.callee)[0].clone();
    rt.enqueue_job(append_fn, vec![c.arg(0), c.arg(1)]);
    Ok(Value::Undefined)
}

/// Runs `tick` until it fails, stores the number of successful ticks in the cell in its data, then rethrows.
fn tick_until_refused(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let cell = match rt.fn_data(c.callee)[0] {
        Value::Object(o) => o,
        _ => panic!("cell missing"),
    };
    let mut n = 0.0;
    loop {
        match rt.tick() {
            Ok(()) => n += 1.0,
            Err(e) => {
                rt.cell_set(cell, num(n));
                return Err(e);
            }
        }
    }
}

/// Ticks three times and returns normally (used to show the budget resets between jobs).
fn three_ticks(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    for _ in 0..3 {
        rt.tick()?;
    }
    Ok(Value::Undefined)
}

/// Records whether its argument is still a live object when it runs.
fn record_liveness(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let Value::Object(o) = c.arg(0) else { return Ok(Value::Bool(false)) };
    let cell = match rt.fn_data(c.callee)[0] {
        Value::Object(o) => o,
        _ => panic!("cell missing"),
    };
    rt.cell_set(cell, Value::Bool(rt.is_live(o)));
    Ok(Value::Undefined)
}

fn log_array(rt: &Rt) -> Value {
    Value::Object(rt.new_array(Vec::new()))
}

fn log_items(rt: &Rt, log: &Value) -> Vec<Value> {
    let len = match rt.get_str(log, "length").unwrap() {
        Value::Number(n) => n as u32,
        _ => 0,
    };
    (0..len).map(|i| rt.get_v(log, &PropKey::Index(i)).unwrap()).collect()
}

#[test]
fn jobs_run_in_fifo_order() {
    let rt = Rt::new();
    let log = log_array(&rt);
    let f = Value::Object(rt.new_native_function(append, "append", 2, vec![]));
    for v in [1.0, 2.0, 3.0] {
        rt.enqueue_job(f.clone(), vec![log.clone(), num(v)]);
    }
    assert_eq!(rt.run_jobs(), 3);
    assert_eq!(log_items(&rt, &log), vec![num(1.0), num(2.0), num(3.0)]);
}

#[test]
fn run_jobs_on_an_empty_queue_runs_nothing() {
    let rt = Rt::new();
    assert_eq!(rt.run_jobs(), 0);
    assert!(rt.take_uncaught_job_errors().is_empty());
}

#[test]
fn jobs_queued_while_draining_run_in_the_same_drain() {
    let rt = Rt::new();
    let log = log_array(&rt);
    let append_fn = Value::Object(rt.new_native_function(append, "append", 2, vec![]));
    let follow = rt.new_native_function(enqueue_follow_up, "follow", 2, vec![append_fn]);
    rt.enqueue_job(Value::Object(follow), vec![log.clone(), num(10.0)]);
    assert_eq!(rt.run_jobs(), 2, "the job queued by the first one runs in the same drain");
    assert_eq!(log_items(&rt, &log), vec![num(10.0)]);
}

#[test]
fn a_throwing_job_does_not_stop_the_drain_and_its_error_is_kept() {
    let rt = Rt::new();
    let log = log_array(&rt);
    let bad = Value::Object(rt.new_native_function(fail, "fail", 0, vec![]));
    let good = Value::Object(rt.new_native_function(append, "append", 2, vec![]));
    rt.enqueue_job(bad, vec![]);
    rt.enqueue_job(good, vec![log.clone(), num(7.0)]);
    assert_eq!(rt.run_jobs(), 2);
    assert_eq!(log_items(&rt, &log), vec![num(7.0)]);
    let errors = rt.take_uncaught_job_errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(text(&rt, &errors[0].0, "name"), "TypeError");
    assert_eq!(text(&rt, &errors[0].0, "message"), "boom");
}

#[test]
fn taking_uncaught_errors_empties_the_list() {
    let rt = Rt::new();
    rt.enqueue_job(Value::Object(rt.new_native_function(fail, "fail", 0, vec![])), vec![]);
    rt.run_jobs();
    assert_eq!(rt.take_uncaught_job_errors().len(), 1);
    assert!(rt.take_uncaught_job_errors().is_empty());
}

#[test]
fn a_job_that_is_not_callable_is_recorded_as_a_type_error() {
    let rt = Rt::new();
    rt.enqueue_job(num(1.0), vec![]);
    rt.run_jobs();
    let errors = rt.take_uncaught_job_errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(text(&rt, &errors[0].0, "message"), "1 is not a function");
}

#[test]
fn queued_values_are_roots_until_their_job_runs() {
    let rt = Rt::new();
    let seen = rt.new_cell(Value::Bool(false));
    let probe = rt.new_native_function(record_liveness, "probe", 1, vec![Value::Object(seen)]);
    let target = rt.new_object();
    rt.enqueue_job(Value::Object(probe), vec![Value::Object(target)]);
    rt.collect();
    assert!(rt.is_live(target), "the queued argument is a root");
    rt.run_jobs();
    assert_eq!(rt.cell_get(seen), Value::Bool(true));
}

#[test]
fn run_jobs_never_collects_so_objects_survive_until_the_host_collects() {
    let rt = Rt::new();
    rt.collect();
    let seen = rt.new_cell(Value::Bool(false));
    let probe = rt.new_native_function(record_liveness, "probe", 1, vec![Value::Object(seen)]);
    let target = rt.new_object();
    rt.enqueue_job(Value::Object(probe), vec![Value::Object(target)]);
    let before = rt.heap_stats().collections;
    rt.run_jobs();
    assert_eq!(rt.heap_stats().collections, before, "run_jobs does not collect");
    assert_eq!(rt.cell_get(seen), Value::Bool(true));
    assert!(rt.try_collect());
    assert!(!rt.is_live(target), "after the job the host may collect it");
}

#[test]
#[should_panic(expected = "run_jobs inside running code")]
fn run_jobs_inside_running_code_is_a_framework_bug() {
    fn drain(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
        rt.run_jobs();
        Ok(Value::Undefined)
    }
    let rt = Rt::new();
    let f = rt.new_native_function(drain, "drain", 0, vec![]);
    let _ = rt.call(&Value::Object(f), &Value::Undefined, &[]);
}

#[test]
fn heap_limit_refuses_ticks_while_the_estimate_is_above_it() {
    let rt = Rt::new();
    rt.collect();
    rt.set_heap_limit(0);
    let e = rt.tick().expect_err("tick must fail above the limit");
    assert_eq!(text(&rt, &e.0, "name"), "RangeError");
    assert_eq!(text(&rt, &e.0, "message"), "heap limit exceeded");
    rt.clear_heap_limit();
    assert!(rt.tick().is_ok());
}

#[test]
fn heap_limit_is_exact_at_the_estimate_boundary() {
    let rt = Rt::new();
    rt.collect();
    let base = rt.heap_stats().bytes_estimate;
    rt.set_heap_limit(base + 1);
    assert!(rt.tick().is_ok(), "an estimate equal to the base is within the limit");
    let o = rt.new_object();
    let id = rt.root(&Value::Object(o));
    let e = rt.tick().expect_err("one allocation pushes the estimate past the limit");
    assert_eq!(text(&rt, &e.0, "message"), "heap limit exceeded");
    rt.unroot(id);
}

#[test]
fn execution_budget_stops_a_job_after_the_allowed_ticks() {
    let rt = Rt::new();
    let cell = rt.new_cell(num(-1.0));
    let job = rt.new_native_function(tick_until_refused, "spin", 0, vec![Value::Object(cell)]);
    rt.set_execution_budget(3);
    rt.enqueue_job(Value::Object(job), vec![]);
    rt.run_jobs();
    assert_eq!(rt.cell_get(cell), num(3.0), "three ticks succeed, the fourth is refused");
    let errors = rt.take_uncaught_job_errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(text(&rt, &errors[0].0, "name"), "RangeError");
    assert_eq!(text(&rt, &errors[0].0, "message"), "execution budget exceeded");
}

#[test]
fn the_execution_budget_resets_before_each_job() {
    let rt = Rt::new();
    rt.set_execution_budget(3);
    let f = Value::Object(rt.new_native_function(three_ticks, "three", 0, vec![]));
    rt.enqueue_job(f.clone(), vec![]);
    rt.enqueue_job(f, vec![]);
    rt.run_jobs();
    assert!(rt.take_uncaught_job_errors().is_empty(), "each job gets its own three ticks");
}

#[test]
fn ticks_outside_jobs_accumulate_until_the_next_job_resets_them() {
    let rt = Rt::new();
    rt.set_execution_budget(2);
    assert!(rt.tick().is_ok());
    assert!(rt.tick().is_ok());
    let e = rt.tick().expect_err("the third tick exceeds the budget");
    assert_eq!(text(&rt, &e.0, "message"), "execution budget exceeded");
    rt.clear_execution_budget();
    assert!(rt.tick().is_ok());
}

#[test]
fn clearing_the_budget_removes_the_limit() {
    let rt = Rt::new();
    rt.set_execution_budget(1);
    rt.clear_execution_budget();
    for _ in 0..100 {
        assert!(rt.tick().is_ok());
    }
}

#[test]
fn a_job_can_read_and_write_properties_of_objects_it_was_given() {
    let rt = Rt::new();
    let o = rt.new_object();
    let f = Value::Object(rt.new_native_function(append, "append", 2, vec![]));
    let arr = Value::Object(rt.new_array(vec![]));
    rt.enqueue_job(f, vec![arr.clone(), Value::Object(o)]);
    rt.run_jobs();
    let items = log_items(&rt, &arr);
    assert_eq!(items, vec![Value::Object(o)]);
    assert!(ok(rt.define_data(o, key("x"), num(1.0), Flags::ALL)));
}

//! Async-function runtime (WP-B5 part 2): `async_start`, `await_value` and heap frames. The bodies are written the way
//! the compiler lowers them: locals live in frame slots and `.await` is `await_value(..).await`. Each expected order is
//! the output of V8 (Node 26) for the same program, recorded above the test.

use std::future::Future;
use std::pin::Pin;

use vn_value::builtins::promise::{async_start, await_value, frame_get, frame_set, new_frame, new_promise, promise_state, PromiseState};
use vn_value::{Call, Intrinsic, ObjRef, PropKey, Res, Rt, Throw, Value};

type Body = Pin<Box<dyn Future<Output = Res<Value>>>>;

fn push(rt: &Rt, arr: &Value, v: Value) -> Res<Value> {
    let len = match rt.get_str(arr, "length")? {
        Value::Number(n) => n as u32,
        _ => 0,
    };
    rt.set_v_strict(arr, &PropKey::Index(len), v)?;
    Ok(Value::Undefined)
}

/// Frame slot 0 holds the log array in every test.
fn log_push(rt: &Rt, frame: ObjRef, label: &str) -> Res<Value> {
    let log = frame_get(rt, frame, 0);
    push(rt, &log, Value::str(label))
}

fn global(rt: &Rt) -> Value {
    Value::Object(rt.intrinsic(Intrinsic::Global))
}

/// `Promise.<name>(args)` through the JavaScript-visible global.
fn promise_static(rt: &Rt, name: &str, args: &[Value]) -> Res<Value> {
    let ctor = rt.get_str(&global(rt), "Promise")?;
    let f = rt.get_str(&ctor, name)?;
    rt.call(&f, &ctor, args)
}

/// `p.then(f)` for a native `f` (closure data: its kind and the log).
fn then(rt: &Rt, p: &Value, f: Value) -> Res<Value> {
    let then = rt.get_str(p, "then")?;
    rt.call(&then, p, &[f])
}

/// Logs `label` when called.
fn label_value(rt: &Rt, log: &Value, label: &str) -> Value {
    Value::Object(rt.new_native_function(label_fn, "label", 0, vec![log.clone(), Value::str(label)]))
}

fn label_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    push(rt, &d[0], d[1].clone())
}

/// Logs its argument when called.
fn log_arg_value(rt: &Rt, log: &Value) -> Value {
    Value::Object(rt.new_native_function(log_arg_fn, "logArg", 1, vec![log.clone()]))
}

fn log_arg_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let log = rt.fn_data(c.callee)[0].clone();
    push(rt, &log, c.arg(0))
}

/// `Promise.resolve().then(l1).then(l2)...` with each step logging its label. Returns the last promise.
fn chain(rt: &Rt, log: &Value, labels: &[&str]) -> Res<Value> {
    let mut p = promise_static(rt, "resolve", &[Value::Undefined])?;
    for l in labels {
        let f = label_value(rt, log, l);
        p = then(rt, &p, f)?;
    }
    Ok(p)
}

/// Executor that keeps its `resolve` argument in the slot of the holder array (closure data).
fn capture_resolve(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let holder = rt.fn_data(c.callee)[0].clone();
    rt.set_v_strict(&holder, &PropKey::Index(0), c.arg(0))?;
    Ok(Value::Undefined)
}

/// A pending promise and its resolve function.
fn pending(rt: &Rt) -> Res<(Value, Value)> {
    let holder = Value::Object(rt.new_array(vec![Value::Undefined]));
    let exec = Value::Object(rt.new_native_function(capture_resolve, "executor", 2, vec![holder.clone()]));
    let ctor = rt.get_str(&global(rt), "Promise")?;
    let p = rt.construct(&ctor, &[exec], &ctor)?;
    let resolve = rt.get_v(&holder, &PropKey::Index(0))?;
    Ok((p, resolve))
}

/// A log array rooted for the whole test, and a frame that holds it in slot 0.
struct Harness {
    rt: Rt,
    log: Value,
}

impl Harness {
    fn new() -> Harness {
        let rt = Rt::new();
        let log = Value::Object(rt.new_array(Vec::new()));
        rt.root(&log);
        Harness { rt, log }
    }

    fn frame(&self, slots: usize) -> ObjRef {
        let f = new_frame(&self.rt, slots);
        frame_set(&self.rt, f, 0, self.log.clone());
        f
    }

    fn entries(&self) -> Vec<String> {
        let n = match self.rt.get_str(&self.log, "length").expect("length") {
            Value::Number(n) => n as u32,
            _ => 0,
        };
        (0..n)
            .map(|i| match self.rt.get_v(&self.log, &PropKey::Index(i)).expect("entry") {
                Value::String(s) => s.to_rust_string_lossy(),
                Value::Number(x) => format!("{x}"),
                other => format!("{other:?}"),
            })
            .collect()
    }

    fn sync(&self, label: &str) {
        push(&self.rt, &self.log, Value::str(label)).expect("log");
    }

    fn drain(&self) {
        self.rt.run_jobs();
        assert!(self.rt.take_uncaught_job_errors().is_empty(), "no job may throw");
    }
}

fn s(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|p| p.to_string()).collect()
}

#[test]
fn an_async_call_runs_up_to_its_first_await_synchronously() {
    // Node 26 for `log('before'); await undefined; log('after')` in an async call, then log('sync'): before sync after.
    let h = Harness::new();
    let frame = h.frame(4);
    let rt = h.rt.clone();
    let body: Body = Box::pin(async move {
        log_push(&rt, frame, "before")?;
        await_value(&rt, Value::Undefined).await?;
        log_push(&rt, frame, "after")
    });
    async_start(&h.rt, frame, body);
    h.sync("sync");
    h.drain();
    assert_eq!(h.entries(), s(&["before", "sync", "after"]));
}

#[test]
fn await_of_a_native_promise_costs_one_tick_and_return_completes_the_result() {
    // Node 26: start a1 m1 a2 m2 R m3, for the program in the comment of the body (the .then logs the resolved value).
    let h = Harness::new();
    let frame = h.frame(4);
    let rt = h.rt.clone();
    let body: Body = Box::pin(async move {
        log_push(&rt, frame, "start")?;
        let p = promise_static(&rt, "resolve", &[Value::Undefined])?;
        await_value(&rt, p).await?;
        log_push(&rt, frame, "a1")?;
        await_value(&rt, Value::Null).await?;
        log_push(&rt, frame, "a2")?;
        Ok(Value::str("R"))
    });
    let result = async_start(&h.rt, frame, body);
    h.rt.root(&result);
    let logger = log_arg_value(&h.rt, &h.log);
    then(&h.rt, &result, logger).expect("then");
    chain(&h.rt, &h.log, &["m1", "m2", "m3"]).expect("chain");
    h.drain();
    assert_eq!(h.entries(), s(&["start", "a1", "m1", "a2", "m2", "R", "m3"]));
}

#[test]
fn await_of_a_non_promise_costs_one_tick() {
    // Node 26: `(async () => { await 5; log('a') })(); Promise.resolve().then(m1).then(m2)` prints a, m1, m2.
    let h = Harness::new();
    let frame = h.frame(4);
    let rt = h.rt.clone();
    let body: Body = Box::pin(async move {
        await_value(&rt, Value::Number(5.0)).await?;
        log_push(&rt, frame, "a")
    });
    async_start(&h.rt, frame, body);
    chain(&h.rt, &h.log, &["m1", "m2"]).expect("chain");
    h.drain();
    assert_eq!(h.entries(), s(&["a", "m1", "m2"]));
}

#[test]
fn await_of_a_thenable_adopts_it_in_a_job() {
    // Node 26: `await th` (th.then logs "then" and resolves with 'tv') with a chain prints sync, then, m1, got:tv, m2, m3.
    let h = Harness::new();
    let frame = h.frame(4);
    let thenable = {
        let o = h.rt.new_object();
        let f = h.rt.new_native_function(thenable_then, "then", 2, vec![h.log.clone()]);
        h.rt.define_data(o, PropKey::from_str("then"), Value::Object(f), vn_value::Flags::ALL).expect("define");
        Value::Object(o)
    };
    let rt = h.rt.clone();
    let body: Body = Box::pin(async move {
        let v = await_value(&rt, thenable).await?;
        let text = match v {
            Value::String(t) => t.to_rust_string_lossy(),
            _ => String::from("?"),
        };
        log_push(&rt, frame, &format!("got:{text}"))
    });
    async_start(&h.rt, frame, body);
    chain(&h.rt, &h.log, &["m1", "m2", "m3"]).expect("chain");
    h.sync("sync");
    h.drain();
    assert_eq!(h.entries(), s(&["sync", "then", "m1", "got:tv", "m2", "m3"]));
}

fn thenable_then(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let log = rt.fn_data(c.callee)[0].clone();
    push(rt, &log, Value::str("then"))?;
    rt.call(&c.arg(0), &Value::Undefined, &[Value::str("tv")])
}

#[test]
fn a_rejected_await_is_caught_by_try_catch_and_finally_runs() {
    // Node 26: try { await Promise.reject(..) } catch (e) { log(caught) } finally { log(fin) }; chain m1, m2
    // prints caught fin m1 m2.
    let h = Harness::new();
    let frame = h.frame(4);
    let rt = h.rt.clone();
    let body: Body = Box::pin(async move {
        let p = promise_static(&rt, "reject", &[Value::str("bad")])?;
        match await_value(&rt, p).await {
            Err(Throw(_)) => log_push(&rt, frame, "caught")?,
            Ok(_) => log_push(&rt, frame, "not caught")?,
        };
        log_push(&rt, frame, "fin")
    });
    async_start(&h.rt, frame, body);
    chain(&h.rt, &h.log, &["m1", "m2"]).expect("chain");
    h.drain();
    assert_eq!(h.entries(), s(&["caught", "fin", "m1", "m2"]));
}

#[test]
fn returning_a_promise_adopts_it_with_two_extra_ticks() {
    // Node 26: `(async () => Promise.resolve('P'))().then(v => log(v))` against a four-step chain prints a, b, P, c, d.
    let h = Harness::new();
    let frame = h.frame(4);
    let rt = h.rt.clone();
    let body: Body = Box::pin(async move {
        let p = promise_static(&rt, "resolve", &[Value::str("P")])?;
        Ok(p)
    });
    let result = async_start(&h.rt, frame, body);
    h.rt.root(&result);
    then(&h.rt, &result, log_arg_value(&h.rt, &h.log)).expect("then");
    chain(&h.rt, &h.log, &["a", "b", "c", "d"]).expect("chain");
    h.drain();
    assert_eq!(h.entries(), s(&["a", "b", "P", "c", "d"]));
}

#[test]
fn a_value_stored_in_a_frame_survives_suspension() {
    let h = Harness::new();
    let frame = h.frame(4);
    let rt = h.rt.clone();
    let body: Body = Box::pin(async move {
        frame_set(&rt, frame, 1, Value::Number(41.0));
        await_value(&rt, Value::Undefined).await?;
        let v = match frame_get(&rt, frame, 1) {
            Value::Number(n) => n + 1.0,
            _ => f64::NAN,
        };
        push(&rt, &frame_get(&rt, frame, 0), Value::Number(v))
    });
    async_start(&h.rt, frame, body);
    h.drain();
    assert_eq!(h.entries(), s(&["42"]));
}

#[test]
fn a_throw_rejects_the_result_and_an_unhandled_result_is_listed() {
    let h = Harness::new();
    let frame = h.frame(4);
    let rt = h.rt.clone();
    let body: Body = Box::pin(async move {
        await_value(&rt, Value::Undefined).await?;
        Err(Throw(Value::str("boom")))
    });
    let result = async_start(&h.rt, frame, body);
    h.rt.root(&result);
    h.drain();
    let Value::Object(r) = result.clone() else { panic!("async call returns a promise") };
    assert_eq!(promise_state(&h.rt, &result), Some(PromiseState::Rejected(Value::str("boom"))));
    assert_eq!(h.rt.take_unhandled_rejections(), vec![Value::Object(r)]);
}

#[test]
fn a_rejected_await_without_try_rejects_the_result() {
    let h = Harness::new();
    let frame = h.frame(4);
    let rt = h.rt.clone();
    let body: Body = Box::pin(async move {
        let p = promise_static(&rt, "reject", &[Value::str("x")])?;
        await_value(&rt, p).await?;
        Ok(Value::str("not reached"))
    });
    let result = async_start(&h.rt, frame, body);
    h.rt.root(&result);
    h.drain();
    assert_eq!(promise_state(&h.rt, &result), Some(PromiseState::Rejected(Value::str("x"))));
    assert_eq!(h.rt.take_unhandled_rejections().len(), 1, "the result is unhandled; the awaited promise was handled");
}

#[test]
fn a_task_stays_alive_across_collections_between_jobs() {
    // A body that awaits 300 promises that the test resolves one at a time, with a collection between the steps.
    let h = Harness::new();
    let frame = h.frame(4);
    let rt = h.rt.clone();
    let body: Body = Box::pin(async move {
        for _ in 0..300 {
            let p = promise_static(&rt, "resolve", &[Value::Undefined])?;
            await_value(&rt, p).await?;
        }
        log_push(&rt, frame, "finished")
    });
    let result = async_start(&h.rt, frame, body);
    h.rt.root(&result);
    for _ in 0..400 {
        h.rt.run_jobs();
        h.rt.collect();
    }
    assert_eq!(h.entries(), s(&["finished"]));
    assert_eq!(promise_state(&h.rt, &result), Some(PromiseState::Fulfilled(Value::Undefined)));
}

#[test]
fn a_task_awaiting_a_pending_promise_that_is_unreachable_is_collected() {
    let h = Harness::new();
    let before = h.rt.heap_stats().live_objects;
    let (result_obj, frame_obj, never_obj) = {
        let frame = h.frame(4);
        let never = new_promise(&h.rt);
        frame_set(&h.rt, frame, 1, Value::Object(never));
        let rt = h.rt.clone();
        let body: Body = Box::pin(async move {
            let p = frame_get(&rt, frame, 1);
            await_value(&rt, p).await?;
            log_push(&rt, frame, "never")
        });
        let result = async_start(&h.rt, frame, body);
        let Value::Object(r) = result else { panic!("async call returns a promise") };
        (r, frame, never)
    };
    h.rt.run_jobs();
    h.rt.collect();
    assert!(!h.rt.is_live(result_obj), "the result promise is garbage");
    assert!(!h.rt.is_live(frame_obj), "the frame is garbage");
    assert!(!h.rt.is_live(never_obj), "the awaited promise is garbage");
    assert!(h.rt.heap_stats().live_objects <= before + 4, "the task itself is garbage too");
    assert!(h.entries().is_empty());
}

#[test]
fn a_long_chain_of_awaits_runs_without_deep_recursion() {
    // 10 000 awaits in one body: resumption happens in reaction jobs, so the Rust stack does not grow.
    let h = Harness::new();
    let frame = h.frame(4);
    let rt = h.rt.clone();
    let body: Body = Box::pin(async move {
        for _ in 0..10_000 {
            await_value(&rt, Value::Number(1.0)).await?;
        }
        log_push(&rt, frame, "done")
    });
    async_start(&h.rt, frame, body);
    h.drain();
    assert_eq!(h.entries(), s(&["done"]));
}

#[test]
fn a_task_awaiting_a_pending_promise_survives_collections_until_it_is_resolved() {
    let h = Harness::new();
    let frame = h.frame(4);
    let (p, resolve) = pending(&h.rt).expect("pending promise");
    h.rt.root(&p);
    h.rt.root(&resolve);
    frame_set(&h.rt, frame, 1, p.clone());
    let rt = h.rt.clone();
    let body: Body = Box::pin(async move {
        let awaited = frame_get(&rt, frame, 1);
        await_value(&rt, awaited).await?;
        log_push(&rt, frame, "resumed")
    });
    let result = async_start(&h.rt, frame, body);
    h.rt.root(&result);
    for _ in 0..3 {
        h.rt.collect();
    }
    assert!(h.entries().is_empty(), "still suspended");
    h.rt.call(&resolve, &Value::Undefined, &[Value::Undefined]).expect("resolve");
    h.drain();
    h.rt.collect();
    assert_eq!(h.entries(), s(&["resumed"]));
    assert_eq!(promise_state(&h.rt, &result), Some(PromiseState::Fulfilled(Value::Undefined)));
}

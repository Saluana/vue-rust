//! WP-RT-B watch and watchEffect tests through the vue exports (crates/runtime/src/watch.rs over vn-reactivity). Expected
//! results follow runtime-core 3.5.43 (production): 'pre' watchers run in the flush, 'sync' inside the write, 'post'
//! after the pre jobs of the same flush, and callback errors use the WATCH_CALLBACK type (3).

mod rtb_common;

use rtb_common::{append, object, throw_message, Env};
use vn_runtime::instance::{create_component_instance, with_instance};
use vn_value::{Call, Res, Rt, Value};

const INFO_CALLBACK: &str = "https://vuejs.org/error-reference/#runtime-3";

/// Logs its first two arguments (new and old value) as separate lines. Data: [log].
fn log_two(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    for i in 0..2 {
        let text = rt.to_string(&c.arg(i))?;
        append(rt, &d[0], Value::String(text))?;
    }
    Ok(Value::Undefined)
}

/// Logs the current value of the ref in data[0] (a watchEffect body). Data: [ref, log].
fn log_value(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let v = rt.get_str(&d[0], "value")?;
    let text = rt.to_string(&v)?;
    append(rt, &d[1], Value::String(text))?;
    Ok(Value::Undefined)
}

fn new_ref(env: &Env, v: Value) -> Value {
    env.rt.call(&env.export("ref"), &Value::Undefined, &[v]).expect("ref")
}

fn set(env: &Env, r: &Value, v: Value) {
    env.rt.set_str(r, "value", v).expect("set value");
}

fn cb(env: &Env, label: &str) -> Value {
    Value::Object(env.rt.new_native_function(log_two, label, 2, vec![env.log_value()]))
}

fn watch(env: &Env, source: &Value, cb: &Value, options: Value) -> Value {
    env.rt.call(&env.export("watch"), &Value::Undefined, &[source.clone(), cb.clone(), options]).expect("watch")
}

fn opts(env: &Env, pairs: Vec<(&str, Value)>) -> Value {
    object(&env.rt, pairs)
}

fn stop(env: &Env, handle: &Value) {
    let f = env.rt.get_str(handle, "stop").expect("stop");
    env.rt.call(&f, handle, &[]).expect("stop call");
}

#[test]
fn a_pre_watcher_runs_in_the_flush_with_the_new_and_old_values() {
    let env = Env::new();
    let r = new_ref(&env, Value::Number(1.0));
    let _handle = watch(&env, &r, &cb(&env, "cb"), Value::Undefined);
    set(&env, &r, Value::Number(2.0));
    assert!(env.log_lines().is_empty(), "nothing runs before the flush");
    env.rt.run_jobs();
    assert_eq!(env.log_lines(), ["2", "1"]);
}

#[test]
fn stopping_a_watcher_drops_its_queued_delivery() {
    let env = Env::new();
    let r = new_ref(&env, Value::Number(1.0));
    let handle = watch(&env, &r, &cb(&env, "cb"), Value::Undefined);
    set(&env, &r, Value::Number(2.0));
    stop(&env, &handle);
    env.rt.run_jobs();
    assert!(env.log_lines().is_empty());
}

#[test]
fn a_sync_watcher_runs_inside_the_write() {
    let env = Env::new();
    let r = new_ref(&env, Value::Number(1.0));
    let _handle = watch(&env, &r, &cb(&env, "cb"), opts(&env, vec![("flush", env.s("sync"))]));
    set(&env, &r, Value::Number(2.0));
    assert_eq!(env.log_lines(), ["2", "1"], "no flush is needed");
}

#[test]
fn post_watchers_run_after_the_pre_jobs_of_the_same_flush() {
    let env = Env::new();
    let r = new_ref(&env, Value::Number(1.0));
    let _post = watch(&env, &r, &cb(&env, "post"), opts(&env, vec![("flush", env.s("post"))]));
    let _pre = watch(&env, &r, &cb(&env, "pre"), Value::Undefined);
    set(&env, &r, Value::Number(2.0));
    env.rt.run_jobs();
    assert_eq!(env.log_lines(), ["2", "1", "2", "1"], "the pre watcher is created second but runs first");
}

#[test]
fn watch_effect_runs_once_at_creation_and_again_on_change() {
    let env = Env::new();
    let rt = &env.rt;
    let r = new_ref(&env, Value::Number(1.0));
    let effect = Value::Object(rt.new_native_function(log_value, "effect", 0, vec![r.clone(), env.log_value()]));
    let _handle = rt.call(&env.export("watchEffect"), &Value::Undefined, &[effect]).unwrap();
    assert_eq!(env.log_lines(), ["1"], "the first run of watchEffect is synchronous");
    set(&env, &r, Value::Number(2.0));
    rt.run_jobs();
    assert_eq!(env.log_lines(), ["1", "2"]);
}

#[test]
fn immediate_and_once_run_the_callback_once_at_creation() {
    let env = Env::new();
    let r = new_ref(&env, Value::Number(1.0));
    let _h = watch(&env, &r, &cb(&env, "cb"), opts(&env, vec![("immediate", Value::Bool(true)), ("once", Value::Bool(true))]));
    assert_eq!(env.log_lines(), ["1", "undefined"]);
    set(&env, &r, Value::Number(2.0));
    env.rt.run_jobs();
    assert_eq!(env.log_lines(), ["1", "undefined"], "once stops the watcher after its first call");
}

#[test]
fn a_callback_error_goes_to_the_error_handler_with_the_watch_callback_type() {
    let env = Env::new();
    let rt = &env.rt;
    let handler = Value::Object(rt.new_native_function(rtb_common::log_args, "errorHandler", 3, vec![env.log_value()]));
    let app = object(rt, vec![("config", object(rt, vec![("errorHandler", handler)])), ("provides", Value::Null)]);
    let vnode = object(rt, vec![("type", object(rt, vec![])), ("appContext", app.clone())]);
    let inst = create_component_instance(rt, &vnode, None, &app).unwrap();
    let thrower = Value::Object(rt.new_native_function(throw_message, "cb", 0, vec![env.s("cb failed")]));
    let r = new_ref(&env, Value::Number(1.0));
    with_instance(rt, inst, |rt| {
        rt.call(&env.export("watch"), &Value::Undefined, &[r.clone(), thrower.clone(), Value::Undefined]).map(|_| ())
    })
    .unwrap();
    set(&env, &r, Value::Number(2.0));
    rt.run_jobs();
    assert_eq!(env.log_lines(), ["TypeError: cb failed", "null", INFO_CALLBACK]);
}

#[test]
fn stopping_the_instance_scope_stops_its_watchers() {
    let env = Env::new();
    let rt = &env.rt;
    let app = object(rt, vec![("config", object(rt, vec![])), ("provides", Value::Null)]);
    let vnode = object(rt, vec![("type", object(rt, vec![])), ("appContext", app.clone())]);
    let inst = create_component_instance(rt, &vnode, None, &app).unwrap();
    let r = new_ref(&env, Value::Number(1.0));
    with_instance(rt, inst, |rt| {
        rt.call(&env.export("watch"), &Value::Undefined, &[r.clone(), cb(&env, "cb"), Value::Undefined]).map(|_| ())
    })
    .unwrap();
    let instance_scope = vn_runtime::instance::instance_of(rt, inst).scope.borrow().clone();
    let stop_scope = rt.get_str(&instance_scope, "stop").unwrap();
    rt.call(&stop_scope, &instance_scope, &[]).unwrap();
    set(&env, &r, Value::Number(2.0));
    rt.run_jobs();
    assert!(env.log_lines().is_empty(), "the scope stop drops the watcher");
}

#[test]
fn a_null_options_argument_is_a_type_error() {
    let env = Env::new();
    let r = new_ref(&env, Value::Number(1.0));
    let res = env.rt.call(&env.export("watch"), &Value::Undefined, &[r, cb(&env, "cb"), Value::Null]);
    assert!(res.is_err());
}

#[test]
fn a_queued_watch_job_survives_a_collection_before_the_flush() {
    let env = Env::new();
    let r = new_ref(&env, Value::Number(1.0));
    let _h = watch(&env, &r, &cb(&env, "cb"), Value::Undefined);
    set(&env, &r, Value::Number(2.0));
    env.rt.collect();
    env.rt.run_jobs();
    assert_eq!(env.log_lines(), ["2", "1"]);
}

#[test]
fn a_stopped_watcher_and_its_job_are_released_by_the_collector() {
    let env = Env::new();
    let rt = &env.rt;
    rt.collect();
    let baseline = rt.heap_stats().live_objects;
    let r = new_ref(&env, Value::Number(1.0));
    // Rust locals are not GC roots (ABI section 11): the ref and the handle are rooted while the test holds them.
    let r_root = rt.root(&r);
    let handle = watch(&env, &r, &cb(&env, "cb"), Value::Undefined);
    let h_root = rt.root(&handle);
    set(&env, &r, Value::Number(2.0));
    rt.run_jobs();
    rt.collect();
    let alive = rt.heap_stats().live_objects;
    assert!(alive > baseline, "a live watcher keeps its nodes and job");
    stop(&env, &handle);
    rt.unroot(h_root);
    rt.collect();
    let released = rt.heap_stats().live_objects;
    assert!(released < alive, "a stopped watcher, and its job, are collected once nothing refers to them");
    rt.unroot(r_root);
}

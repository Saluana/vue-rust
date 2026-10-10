//! Dynamic import (WP-C1c.5.5, `Rt::dynamic_import`). An import settles in host turns, one per round trip of Node's
//! loader with loader hooks, which is the project oracle (tests/language/projects, run by
//! tooling/harness/run-projects.mjs). A turn runs only when the job queue is empty. An import of a module that is not
//! loaded takes one turn to resolve it and one to load it, plus a resolve turn per request (and the loads of the
//! requests that are not loaded). An import of a loaded module takes one turn. The orders checked here are the ones
//! the oracle shows for these graphs; the probes are recorded in docs/evidence/dynamic-import.md.

mod support;
#[macro_use]
#[path = "modules_common.rs"]
mod common;

use common::*;
use vn_value::builtins::promise::{promise_state, PromiseState};
use vn_value::{same_value, Call, CodeFn, ObjRef, Res, Rt, Value};

fn eval_leaf(_rt: &Rt, _m: ObjRef) -> Res<()> {
    log("leaf-eval");
    Ok(())
}

fn eval_loaded(_rt: &Rt, _m: ObjRef) -> Res<()> {
    log("loaded-eval");
    Ok(())
}

fn eval_lwd(_rt: &Rt, _m: ObjRef) -> Res<()> {
    log("lwd-eval");
    Ok(())
}

fn eval_dep_a(_rt: &Rt, _m: ObjRef) -> Res<()> {
    log("dep-a-eval");
    Ok(())
}

fn eval_deps(_rt: &Rt, _m: ObjRef) -> Res<()> {
    log("deps-eval");
    Ok(())
}

fn eval_bad(rt: &Rt, _m: ObjRef) -> Res<()> {
    log("bad-eval");
    Err(rt.throw_error(vn_value::ErrorKind::Error, "boom"))
}

fn eval_bad_parent(_rt: &Rt, _m: ObjRef) -> Res<()> {
    log("bad-parent-eval");
    Ok(())
}

fn eval_main(_rt: &Rt, _m: ObjRef) -> Res<()> {
    log("main-eval");
    Ok(())
}

/// Registers the definitions of this file's modules. None of them is loaded yet.
fn register(rt: &Rt) {
    rt.register_module_def(def("leaf.mjs", &[], nothing, nothing, eval_leaf));
    rt.register_module_def(def("loaded.mjs", &[], nothing, nothing, eval_loaded));
    rt.register_module_def(def("lwd.mjs", &["loaded.mjs"], nothing, nothing, eval_lwd));
    rt.register_module_def(def("dep-a.mjs", &[], nothing, nothing, eval_dep_a));
    rt.register_module_def(def("deps.mjs", &["dep-a.mjs"], nothing, nothing, eval_deps));
    rt.register_module_def(def("bad.mjs", &[], nothing, nothing, eval_bad));
    rt.register_module_def(def("bad-parent.mjs", &["bad.mjs"], nothing, nothing, eval_bad_parent));
    rt.register_module_def(def("main.mjs", &["loaded.mjs"], nothing, nothing, eval_main));
}

/// A promise job that logs `then<i>`: one step of a `.then()` chain. Jobs queued before a host turn all run first.
fn chain_step(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let Value::Number(i) = c.arg(0) else {
        panic!("chain step without its number (test bug)")
    };
    log(format!("then{}", i as u32));
    Ok(Value::Undefined)
}

fn queue_chain(rt: &Rt, n: u32) {
    let f = rt.new_native_function(chain_step, "", 1, Vec::new());
    for i in 1..=n {
        rt.enqueue_job(Value::Object(f), vec![Value::Number(f64::from(i))]);
    }
}

/// Reactions that log the settlement of one import.
macro_rules! settled {
    ($f:ident, $label:literal) => {
        fn $f(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
            log(concat!("settled-", $label));
            Ok(Value::Undefined)
        }
    };
}
settled!(settled_v, "V");
settled!(settled_l, "L");
settled!(settled_w, "W");
settled!(settled_d, "D");

/// Attaches `code` to promise `p` as `p.then(code)`.
fn log_settle(rt: &Rt, p: &Value, code: CodeFn) {
    let then = rt.get_str(p, "then").expect("a promise has then");
    let f = rt.new_native_function(code, "", 1, Vec::new());
    rt.call(&then, p, &[Value::Object(f)]).expect("then does not throw");
}

fn import(rt: &Rt, spec: &str) -> Value {
    Value::Object(rt.dynamic_import(spec))
}

fn settled_value(rt: &Rt, p: &Value) -> Value {
    match promise_state(rt, p) {
        Some(PromiseState::Fulfilled(v)) => v,
        other => panic!("the import is not fulfilled: {other:?}"),
    }
}

fn rejected_reason(rt: &Rt, p: &Value) -> Value {
    match promise_state(rt, p) {
        Some(PromiseState::Rejected(v)) => v,
        other => panic!("the import is not rejected: {other:?}"),
    }
}

/// Loads each module of `warm` (no reactions), then starts the imports in `order` with their reactions, drains the
/// queue, and returns the log of the run.
fn run_imports(warm: &[&str], order: &[(&str, CodeFn)]) -> Vec<String> {
    let rt = Rt::new();
    register(&rt);
    for spec in warm {
        let p = import(&rt, spec);
        rt.run_jobs();
        assert!(matches!(promise_state(&rt, &p), Some(PromiseState::Fulfilled(_))), "warm-up of {spec} failed");
    }
    take_log();
    for (spec, code) in order {
        let p = import(&rt, spec);
        log_settle(&rt, &p, *code);
    }
    rt.run_jobs();
    take_log()
}

#[test]
fn an_import_of_a_module_not_loaded_waits_for_the_promise_jobs_queued_before_it() {
    let rt = Rt::new();
    register(&rt);
    take_log();
    let p = import(&rt, "leaf.mjs");
    queue_chain(&rt, 24);
    rt.run_jobs();
    let mut want: Vec<String> = (1..=24).map(|i| format!("then{i}")).collect();
    want.push("leaf-eval".to_string());
    assert_eq!(take_log(), want);
    let m = rt.module_record("leaf.mjs").expect("leaf is loaded");
    assert!(same_value(&settled_value(&rt, &p), &Value::Object(rt.module_namespace(m))));
}

#[test]
fn two_imports_settle_in_the_order_of_their_turns_whichever_starts_first() {
    let want = ["leaf-eval", "settled-L", "dep-a-eval", "deps-eval", "settled-D"];
    assert_eq!(run_imports(&[], &[("leaf.mjs", settled_l), ("deps.mjs", settled_d)]), want);
    assert_eq!(run_imports(&[], &[("deps.mjs", settled_d), ("leaf.mjs", settled_l)]), want);
}

#[test]
fn a_loaded_module_settles_in_one_turn_before_a_module_that_is_not_loaded() {
    let want = ["settled-V", "leaf-eval", "settled-L"];
    assert_eq!(run_imports(&["loaded.mjs"], &[("loaded.mjs", settled_v), ("leaf.mjs", settled_l)]), want);
    assert_eq!(run_imports(&["loaded.mjs"], &[("leaf.mjs", settled_l), ("loaded.mjs", settled_v)]), want);
}

#[test]
fn a_request_of_a_module_costs_a_turn_even_when_the_requested_module_is_loaded() {
    // lwd.mjs requests loaded.mjs, which is loaded: three turns against the leaf's two.
    let want = ["leaf-eval", "settled-L", "lwd-eval", "settled-W"];
    assert_eq!(run_imports(&["loaded.mjs"], &[("lwd.mjs", settled_w), ("leaf.mjs", settled_l)]), want);
}

#[test]
fn imports_share_one_namespace_and_evaluate_each_module_once() {
    let rt = Rt::new();
    register(&rt);
    take_log();
    let a = import(&rt, "leaf.mjs");
    let b = import(&rt, "leaf.mjs");
    rt.run_jobs();
    let c = import(&rt, "leaf.mjs");
    rt.run_jobs();
    assert_eq!(take_log(), ["leaf-eval"]);
    let (na, nb, nc) = (settled_value(&rt, &a), settled_value(&rt, &b), settled_value(&rt, &c));
    assert!(same_value(&na, &nb) && same_value(&nb, &nc), "imports of one module give one namespace");
}

#[test]
fn a_dynamic_import_of_a_static_import_gives_the_static_namespace() {
    let rt = Rt::new();
    register(&rt);
    take_log();
    let main = rt.load_module("main.mjs").expect("main loads");
    assert_eq!(take_log(), ["loaded-eval", "main-eval"]);
    let p = import(&rt, "loaded.mjs");
    rt.run_jobs();
    let ns = rt.module_namespace(rt.module_record("loaded.mjs").expect("loaded is linked"));
    assert!(same_value(&settled_value(&rt, &p), &Value::Object(ns)));
    assert!(take_log().is_empty(), "the static import is not evaluated again");
    let _ = main;
}

#[test]
fn a_module_that_throws_rejects_every_import_with_the_same_error() {
    let rt = Rt::new();
    register(&rt);
    take_log();
    let first = import(&rt, "bad.mjs");
    rt.run_jobs();
    let second = import(&rt, "bad.mjs");
    let parent = import(&rt, "bad-parent.mjs");
    rt.run_jobs();
    let (r1, r2, r3) = (rejected_reason(&rt, &first), rejected_reason(&rt, &second), rejected_reason(&rt, &parent));
    assert!(same_value(&r1, &r2), "a later import rejects with the cached error");
    assert!(same_value(&r1, &r3), "a module that imports the failed one rejects with the same error");
    assert_eq!(error_name(&rt, &r1), "Error");
    assert_eq!(error_message(&rt, &r1), "boom");
    assert_eq!(take_log(), ["bad-eval"], "the failed module is evaluated once, and its parent is never evaluated");
}

#[test]
fn a_pending_import_survives_a_collection_with_its_reactions() {
    let rt = Rt::new();
    register(&rt);
    take_log();
    let p = import(&rt, "lwd.mjs");
    log_settle(&rt, &p, settled_w);
    // Only the host turn holds the promise now. The collection must keep it, its reactions and the modules it loads.
    rt.collect();
    rt.run_jobs();
    assert_eq!(take_log(), ["loaded-eval", "lwd-eval", "settled-W"]);
}

#[test]
fn an_import_of_an_unregistered_module_rejects_without_evaluating_anything() {
    let rt = Rt::new();
    register(&rt);
    take_log();
    let p = import(&rt, "missing.mjs");
    rt.run_jobs();
    let _ = rejected_reason(&rt, &p);
    assert!(take_log().is_empty());
}

/// Negative control. Plain Node (no loader hooks) evaluates a leaf after the seventh job of a chain. The runtime must
/// not reproduce that order: it is the one the project oracle does not show, so a test that accepted it would pass for
/// the wrong model.
#[test]
fn negative_control_the_plain_node_job_order_is_not_reproduced() {
    let rt = Rt::new();
    register(&rt);
    take_log();
    let p = import(&rt, "leaf.mjs");
    queue_chain(&rt, 24);
    rt.run_jobs();
    let log = take_log();
    let at = log.iter().position(|s| s == "leaf-eval").expect("the leaf is evaluated");
    assert_eq!(at, 24, "the leaf runs after the whole chain, as under the loader hooks");
    assert_ne!(at, 7, "plain Node evaluates the leaf after job 7; this runtime must not");
    let _ = p;
}

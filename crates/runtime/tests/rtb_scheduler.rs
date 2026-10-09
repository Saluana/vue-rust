//! WP-RT-B scheduler tests (crates/runtime/src/scheduler.rs). Expected orders follow runtime-core 3.5.43 (production):
//! queueJob / findInsertionIndex, flushJobs, flushPostFlushCbs and nextTick. The differential scenarios in
//! tests/vue/runtime-b/run.mjs cover the same ground against the real Vue build.

mod rtb_common;

use rtb_common::{append, logger, logging_job, Env};
use vn_runtime::scheduler::{self, ALLOW_RECURSE, DISPOSED, PRE, QUEUED};
use vn_value::{Call, ObjRef, Res, Rt, Throw, Value};

fn set_id(rt: &Rt, job: ObjRef, id: f64, flags: u32) {
    let m = scheduler::job_meta(rt, job);
    m.id.set(Some(id));
    m.flags.set(m.flags.get() | flags);
}

#[test]
fn pre_jobs_come_first_within_an_id_and_ids_sort_the_queue() {
    let env = Env::new();
    let a = logging_job(&env, "A");
    let b = logging_job(&env, "B");
    let c = logging_job(&env, "C");
    set_id(&env.rt, a, 2.0, 0);
    set_id(&env.rt, b, 1.0, PRE);
    set_id(&env.rt, c, 2.0, PRE);
    for j in [a, b, c] {
        scheduler::queue_job(&env.rt, j).unwrap();
    }
    env.rt.run_jobs();
    assert_eq!(env.log_lines(), ["B", "C", "A"]);
}

#[test]
fn a_queued_job_is_not_queued_twice() {
    let env = Env::new();
    let a = logging_job(&env, "A");
    scheduler::queue_job(&env.rt, a).unwrap();
    scheduler::queue_job(&env.rt, a).unwrap();
    env.rt.run_jobs();
    assert_eq!(env.log_lines(), ["A"]);
    assert_eq!(scheduler::job_meta(&env.rt, a).flags.get() & QUEUED, 0, "the flush clears QUEUED");
}

/// Data: [log, label, next job or undefined]. Logs its label, then queues the next job.
fn chain_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    append(rt, &d[0], d[1].clone())?;
    if let Value::Object(next) = d[2] {
        scheduler::queue_job(rt, next)?;
    }
    Ok(Value::Undefined)
}

#[test]
fn jobs_queued_during_a_flush_run_in_the_same_flush_after_the_running_one() {
    let env = Env::new();
    let b = logging_job(&env, "B");
    let a = env.rt.new_native_function(chain_code, "A", 0, vec![env.log_value(), env.s("A"), Value::Object(b)]);
    scheduler::queue_job(&env.rt, a).unwrap();
    env.rt.run_jobs();
    assert_eq!(env.log_lines(), ["A", "B"]);
}

#[test]
fn post_callbacks_run_after_the_jobs_and_are_deduplicated() {
    let env = Env::new();
    let job = logging_job(&env, "J");
    let post = logging_job(&env, "P");
    scheduler::queue_post_flush_cb(&env.rt, &Value::Object(post)).unwrap();
    scheduler::queue_job(&env.rt, job).unwrap();
    scheduler::queue_post_flush_cb(&env.rt, &Value::Object(post)).unwrap();
    env.rt.run_jobs();
    assert_eq!(env.log_lines(), ["J", "P"]);
}

/// Data: [log, label, callback to queue]. Logs its label, then queues the callback as a post flush callback.
fn post_queue_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    append(rt, &d[0], d[1].clone())?;
    scheduler::queue_post_flush_cb(rt, &d[2])?;
    Ok(Value::Undefined)
}

#[test]
fn a_post_callback_with_id_minus_one_queued_while_draining_runs_next() {
    let env = Env::new();
    let rt = &env.rt;
    let p2 = logging_job(&env, "P2");
    let p3 = logging_job(&env, "P3");
    scheduler::job_meta(rt, p3).id.set(Some(-1.0));
    let p1 = rt.new_native_function(post_queue_code, "P1", 0, vec![env.log_value(), env.s("P1"), Value::Object(p3)]);
    scheduler::queue_post_flush_cb(rt, &Value::Object(p1)).unwrap();
    scheduler::queue_post_flush_cb(rt, &Value::Object(p2)).unwrap();
    rt.run_jobs();
    assert_eq!(env.log_lines(), ["P1", "P3", "P2"], "an id -1 callback is inserted right after the running one");
}

#[test]
fn next_tick_returns_the_same_promise_when_idle_and_runs_callbacks_after_the_flush() {
    let env = Env::new();
    let rt = &env.rt;
    let p1 = scheduler::next_tick(rt, &Value::Undefined, &Value::Undefined).unwrap();
    let p2 = scheduler::next_tick(rt, &Value::Undefined, &Value::Undefined).unwrap();
    assert!(vn_value::strict_eq(&p1, &p2), "idle nextTick returns resolvedPromise each time");
    let job = logging_job(&env, "J");
    scheduler::queue_job(rt, job).unwrap();
    let tick = env.native("tick", logger, vec![env.log_value(), env.s("tick")]);
    let p3 = scheduler::next_tick(rt, &Value::Undefined, &tick).unwrap();
    assert!(!vn_value::strict_eq(&p1, &p3), "a pending flush makes nextTick chain on currentFlushPromise");
    rt.run_jobs();
    assert_eq!(env.log_lines(), ["J", "tick"]);
}

/// Logs and re-queues itself; with ALLOW_RECURSE set, the flush keeps running it until the watchdog stops it.
fn requeue_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    append(rt, &d[0], Value::Number(1.0))?;
    scheduler::queue_job(rt, c.callee)?;
    Ok(Value::Undefined)
}

#[test]
fn a_job_that_keeps_requeuing_itself_fails_the_flush_at_the_recursion_limit() {
    let env = Env::new();
    let rt = &env.rt;
    let job = rt.new_native_function(requeue_code, "requeue", 0, vec![env.log_value()]);
    scheduler::job_meta(rt, job).flags.set(ALLOW_RECURSE);
    scheduler::queue_job(rt, job).unwrap();
    let err = match scheduler::flush_jobs(rt) {
        Err(Throw(e)) => e,
        Ok(()) => panic!("the watchdog must fail the flush"),
    };
    let message = rt.get_str(&err, "message").unwrap();
    assert!(rt.to_string(&message).unwrap().to_rust_string_lossy().starts_with("Maximum recursive updates exceeded"));
    assert_eq!(env.log_lines().len(), scheduler::RECURSION_LIMIT as usize + 1, "101 runs are allowed, the 102nd fails");
    assert!(rt.is_live(job));
}

#[test]
fn a_disposed_job_is_skipped_by_the_flush() {
    let env = Env::new();
    let a = logging_job(&env, "A");
    let b = logging_job(&env, "B");
    scheduler::job_meta(&env.rt, a).flags.set(DISPOSED);
    scheduler::queue_job(&env.rt, a).unwrap();
    scheduler::queue_job(&env.rt, b).unwrap();
    env.rt.run_jobs();
    assert_eq!(env.log_lines(), ["B"]);
}

#[test]
fn queued_jobs_survive_a_collection_before_the_flush() {
    let env = Env::new();
    let a = logging_job(&env, "A");
    let b = logging_job(&env, "B");
    scheduler::queue_job(&env.rt, a).unwrap();
    scheduler::queue_job(&env.rt, b).unwrap();
    env.rt.collect();
    env.rt.run_jobs();
    assert_eq!(env.log_lines(), ["A", "B"]);
}

/// Data: [log, job]. Logs "P" and queues the job (the post flush then runs a second round of jobs).
fn post_queues_job_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    append(rt, &d[0], Value::String(vn_value::JsStr::from_str("P")))?;
    if let Value::Object(job) = d[1] {
        scheduler::queue_job(rt, job)?;
    }
    Ok(Value::Undefined)
}

#[test]
fn a_job_queued_by_a_post_callback_runs_in_a_following_round_of_the_same_flush() {
    let env = Env::new();
    let rt = &env.rt;
    let job = logging_job(&env, "J");
    let post = rt.new_native_function(post_queues_job_code, "post", 0, vec![env.log_value(), Value::Object(job)]);
    scheduler::queue_post_flush_cb(rt, &Value::Object(post)).unwrap();
    rt.run_jobs();
    assert_eq!(env.log_lines(), ["P", "J"]);
}

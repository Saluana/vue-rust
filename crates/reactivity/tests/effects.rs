//! Effects, batching and manual tracking. Each test is the Rust translation of one snippet in tests/vue/rust/snippets.json
//! (the id is in the doc comment); the expected lines are the log that real @vue/reactivity 3.5.43 produced for it.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::{check, holder, js, make_fn, put, take_from, Env};
use vn_value::{Flags, PropKey, Value};

fn num(x: f64) -> Value {
    Value::Number(x)
}

fn undef() -> Value {
    Value::Undefined
}

fn counter() -> Rc<Cell<i64>> {
    Rc::new(Cell::new(0))
}

/// A plain object with one data property (an option bag such as `{ scheduler }`).
fn bag(e: &Env, pairs: &[(&str, Value)]) -> Value {
    let o = e.rt.new_object();
    for (k, v) in pairs {
        e.rt.define_data(o, PropKey::from_str(k), v.clone(), Flags::ALL).expect("bag");
    }
    Value::Object(o)
}

/// `effect-runs-and-skips-equal-writes`
#[test]
fn effect_runs_and_skips_equal_writes() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let runs = counter();
    let (log, r2) = (e.log.clone(), runs.clone());
    let body = e.func("body", 0, vec![n.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        log.borrow_mut().push(format!("run {}", js(rt, &rt.get_str(&d[0], "value")?)));
        Ok(undef())
    });
    e.invoke("effect", &[body]);
    e.log.borrow_mut().push(format!("after-create {}", runs.get()));
    e.set(&n, "value", num(1.0));
    e.set(&n, "value", num(1.0));
    e.log.borrow_mut().push(format!("after-equal-write {}", runs.get()));
    e.set(&n, "value", num(2.0));
    e.log.borrow_mut().push(format!("after-change {}", runs.get()));
    check(&e, "effect-runs-and-skips-equal-writes");
}

/// `effect-nan-and-negative-zero`: Object.is semantics. NaN equals NaN; -0 differs from +0.
#[test]
fn effect_nan_and_negative_zero() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(f64::NAN)]);
    let runs = counter();
    let (log, r2) = (e.log.clone(), runs.clone());
    let body = e.func("body", 0, vec![n.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        log.borrow_mut().push(format!("nan-run {}", js(rt, &rt.get_str(&d[0], "value")?)));
        Ok(undef())
    });
    e.invoke("effect", &[body]);
    e.set(&n, "value", num(f64::NAN));
    e.log.borrow_mut().push(format!("nan-runs {}", runs.get()));
    let z = e.invoke("ref", &[num(0.0)]);
    let zruns = counter();
    let (log2, z2) = (e.log.clone(), zruns.clone());
    let zbody = e.func("zbody", 0, vec![z.clone()], move |rt, _, _, d| {
        z2.set(z2.get() + 1);
        let v = rt.to_number(&rt.get_str(&d[0], "value")?)?;
        log2.borrow_mut().push(format!("zero-run {}", js(rt, &Value::Number(1.0 / v))));
        Ok(undef())
    });
    e.invoke("effect", &[zbody]);
    e.set(&z, "value", num(-0.0));
    e.log.borrow_mut().push(format!("zero-runs {}", zruns.get()));
    check(&e, "effect-nan-and-negative-zero");
}

/// `effect-cleanup-before-rerun-and-stop`: each run registers a cleanup that logs the value of its own run.
#[test]
fn effect_cleanup_before_rerun_and_stop() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let (table, log) = (e.table(), e.log.clone());
    // makeCleanup(v): a fresh function that logs `cleanup <v>` (v is in its data).
    let maker = make_fn(&e.rt, &table, "makeCleanup", 1, vec![], {
        let log = log.clone();
        let table = table.clone();
        move |rt, _, args, _| {
            let log = log.clone();
            Ok(make_fn(rt, &table, "cleanup", 0, vec![args[0].clone()], move |rt, _, _, d| {
                log.borrow_mut().push(format!("cleanup {}", js(rt, &d[0])));
                Ok(undef())
            }))
        }
    });
    let oec = e.api("onEffectCleanup");
    let body = e.func("body", 0, vec![n.clone(), oec, maker], move |rt, _, _, d| {
        let v = rt.get_str(&d[0], "value")?;
        log.borrow_mut().push(format!("run {}", js(rt, &v)));
        let cleanup = rt.call(&d[2], &undef(), &[v])?;
        rt.call(&d[1], &undef(), &[cleanup])?;
        Ok(undef())
    });
    let runner = e.invoke("effect", &[body]);
    e.set(&n, "value", num(1.0));
    e.invoke("stop", &[runner]);
    e.set(&n, "value", num(2.0));
    e.log.borrow_mut().push("end".to_string());
    check(&e, "effect-cleanup-before-rerun-and-stop");
}

/// `effect-conditional-dependencies`
#[test]
fn effect_conditional_dependencies() {
    let e = Env::new();
    let flag = e.invoke("ref", &[Value::Bool(true)]);
    let a = e.invoke("ref", &[Env::text("a1")]);
    let b = e.invoke("ref", &[Env::text("b1")]);
    let runs = counter();
    let (log, r2) = (e.log.clone(), runs.clone());
    let body = e.func("body", 0, vec![flag.clone(), a.clone(), b.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        let pick = if rt.get_str(&d[0], "value")?.to_boolean() { &d[1] } else { &d[2] };
        log.borrow_mut().push(format!("run {}", js(rt, &rt.get_str(pick, "value")?)));
        Ok(undef())
    });
    e.invoke("effect", &[body]);
    e.set(&b, "value", Env::text("b2"));
    e.log.borrow_mut().push(format!("runs-1 {}", runs.get()));
    e.set(&flag, "value", Value::Bool(false));
    e.log.borrow_mut().push(format!("runs-2 {}", runs.get()));
    e.set(&a, "value", Env::text("a2"));
    e.log.borrow_mut().push(format!("runs-3 {}", runs.get()));
    e.set(&b, "value", Env::text("b3"));
    e.log.borrow_mut().push(format!("runs-4 {}", runs.get()));
    check(&e, "effect-conditional-dependencies");
}

/// `effect-nested-effects`: an effect created inside an effect is not stopped when the outer one re-runs.
#[test]
fn effect_nested_effects() {
    let e = Env::new();
    let outer = e.invoke("ref", &[num(0.0)]);
    let inner = e.invoke("ref", &[num(10.0)]);
    let inner_runs = counter();
    let (log, ir) = (e.log.clone(), inner_runs.clone());
    let inner_body = e.func("innerBody", 0, vec![inner.clone()], move |rt, _, _, d| {
        ir.set(ir.get() + 1);
        log.borrow_mut().push(format!("inner {}", js(rt, &rt.get_str(&d[0], "value")?)));
        Ok(undef())
    });
    let log2 = e.log.clone();
    let outer_body = e.func("outerBody", 0, vec![outer.clone(), e.api("effect"), inner_body], move |rt, _, _, d| {
        log2.borrow_mut().push(format!("outer {}", js(rt, &rt.get_str(&d[0], "value")?)));
        rt.call(&d[1], &undef(), &[d[2].clone()])?;
        Ok(undef())
    });
    e.invoke("effect", &[outer_body]);
    e.log.borrow_mut().push(format!("count {}", inner_runs.get()));
    e.set(&inner, "value", num(11.0));
    e.log.borrow_mut().push(format!("count2 {}", inner_runs.get()));
    e.set(&outer, "value", num(1.0));
    e.log.borrow_mut().push(format!("count3 {}", inner_runs.get()));
    e.set(&inner, "value", num(12.0));
    e.log.borrow_mut().push(format!("count4 {}", inner_runs.get()));
    check(&e, "effect-nested-effects");
}

/// `effect-stop-releases-links`: a stopped effect is not re-run by writes; a runner still runs its function.
#[test]
fn effect_stop_releases_links() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(1.0)]);
    let runs = counter();
    let (log, r2) = (e.log.clone(), runs.clone());
    let body = e.func("body", 0, vec![n.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        log.borrow_mut().push(format!("run {}", js(rt, &rt.get_str(&d[0], "value")?)));
        Ok(undef())
    });
    let runner = e.invoke("effect", &[body]);
    e.invoke("stop", &[runner.clone()]);
    e.set(&n, "value", num(2.0));
    e.log.borrow_mut().push(format!("runs-after-stop {}", runs.get()));
    e.invoke("stop", &[runner.clone()]);
    e.set(&n, "value", num(3.0));
    e.call(&runner, &undef(), &[]);
    e.log.borrow_mut().push(format!("runs-manual {}", runs.get()));
    e.set(&n, "value", num(4.0));
    e.log.borrow_mut().push(format!("runs-final {}", runs.get()));
    check(&e, "effect-stop-releases-links");
}

/// `effect-pause-coalesces-writes`: writes while paused queue one run, executed on resume.
#[test]
fn effect_pause_coalesces_writes() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let runs = counter();
    let (log, r2) = (e.log.clone(), runs.clone());
    let body = e.func("body", 0, vec![n.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        log.borrow_mut().push(format!("run {}", js(rt, &rt.get_str(&d[0], "value")?)));
        Ok(undef())
    });
    let runner = e.invoke("effect", &[body]);
    let eff = e.get(&runner, "effect");
    let pause = e.get(&eff, "pause");
    let resume = e.get(&eff, "resume");
    e.call(&pause, &eff, &[]);
    e.set(&n, "value", num(1.0));
    e.set(&n, "value", num(2.0));
    e.set(&n, "value", num(3.0));
    e.log.borrow_mut().push(format!("paused-runs {}", runs.get()));
    e.call(&resume, &eff, &[]);
    e.log.borrow_mut().push(format!("resumed-runs {}", runs.get()));
    e.set(&n, "value", num(4.0));
    e.log.borrow_mut().push(format!("final-runs {}", runs.get()));
    check(&e, "effect-pause-coalesces-writes");
}

/// `effect-scheduler-option`: with a scheduler, a trigger calls the scheduler and the runner decides when to run.
#[test]
fn effect_scheduler_option() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let slot = holder(&e.rt);
    let log = e.log.clone();
    let sched = e.func("scheduler", 0, vec![slot.clone()], move |rt, _, _, d| {
        log.borrow_mut().push("scheduled".to_string());
        let _ = (rt, &d[0]);
        Ok(undef())
    });
    let runs = counter();
    let (log2, r2) = (e.log.clone(), runs.clone());
    let body = e.func("body", 0, vec![n.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        log2.borrow_mut().push(format!("run {}", js(rt, &rt.get_str(&d[0], "value")?)));
        Ok(undef())
    });
    let opts = bag(&e, &[("scheduler", sched)]);
    let runner = e.invoke("effect", &[body, opts]);
    put(&e.rt, &slot, "r", runner.clone());
    e.set(&n, "value", num(1.0));
    e.set(&n, "value", num(2.0));
    e.log.borrow_mut().push(format!("runs-before-flush {}", runs.get()));
    let queued = take_from(&e.rt, &slot, "r");
    e.call(&queued, &undef(), &[]);
    e.log.borrow_mut().push(format!("runs-after-flush {}", runs.get()));
    check(&e, "effect-scheduler-option");
}

/// `effect-on-stop-option`: onStop runs once, when the effect is first stopped.
#[test]
fn effect_on_stop_option() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let log = e.log.clone();
    let body = e.func("body", 0, vec![n.clone()], {
        let log = log.clone();
        move |rt, _, _, d| {
            log.borrow_mut().push(format!("run {}", js(rt, &rt.get_str(&d[0], "value")?)));
            Ok(undef())
        }
    });
    let on_stop = e.func("onStop", 0, vec![], move |_, _, _, _| {
        log.borrow_mut().push("on-stop".to_string());
        Ok(undef())
    });
    let opts = bag(&e, &[("onStop", on_stop)]);
    let runner = e.invoke("effect", &[body, opts]);
    e.invoke("stop", &[runner.clone()]);
    e.invoke("stop", &[runner]);
    e.log.borrow_mut().push("done".to_string());
    check(&e, "effect-on-stop-option");
}

/// `effect-self-write-no-recursion`: a write to the effect's own dep while it runs does not re-run it.
#[test]
fn effect_self_write_no_recursion() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let runs = counter();
    let (log, r2) = (e.log.clone(), runs.clone());
    let body = e.func("body", 0, vec![n.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        let v = rt.get_str(&d[0], "value")?;
        log.borrow_mut().push(format!("run {}", js(rt, &v)));
        if rt.to_number(&v)? < 5.0 {
            let next = rt.to_number(&v)? + 1.0;
            rt.set_v_strict(&d[0], &PropKey::from_str("value"), num(next))?;
        }
        Ok(undef())
    });
    e.invoke("effect", &[body]);
    e.log.borrow_mut().push(format!("runs {} value {}", runs.get(), js(&e.rt, &e.get(&n, "value"))));
    check(&e, "effect-self-write-no-recursion");
}

/// `effect-allow-recurse-reruns-itself`: with the ALLOW_RECURSE flag (32) set on the effect, its own writes re-run it.
#[test]
fn effect_allow_recurse_reruns_itself() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let runs = counter();
    let (log, r2) = (e.log.clone(), runs.clone());
    let body = e.func("body", 0, vec![n.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        let v = rt.get_str(&d[0], "value")?;
        log.borrow_mut().push(format!("run {}", js(rt, &v)));
        if rt.to_number(&v)? < 3.0 {
            let next = rt.to_number(&v)? + 1.0;
            rt.set_v_strict(&d[0], &PropKey::from_str("value"), num(next))?;
        }
        Ok(undef())
    });
    let runner = e.invoke("effect", &[body]);
    e.log.borrow_mut().push(format!("after-create {} {}", runs.get(), js(&e.rt, &e.get(&n, "value"))));
    let eff = e.get(&runner, "effect");
    let flags = e.get(&eff, "flags");
    let flags_v = e.rt.to_uint32(&flags).expect("flags");
    e.set(&eff, "flags", num((flags_v | 32) as f64));
    e.set(&n, "value", num(0.0));
    e.log.borrow_mut().push(format!("after-flagged {} {}", runs.get(), js(&e.rt, &e.get(&n, "value"))));
    check(&e, "effect-allow-recurse-reruns-itself");
}

/// `effect-dirty-getter`: `dirty` is true after a dep changed and false after the run.
#[test]
fn effect_dirty_getter() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let log = e.log.clone();
    let runs = counter();
    let r2 = runs.clone();
    let body = e.func("body", 0, vec![n.clone()], {
        let log = log.clone();
        move |rt, _, _, d| {
            r2.set(r2.get() + 1);
            log.borrow_mut().push(format!("run {}", js(rt, &rt.get_str(&d[0], "value")?)));
            Ok(undef())
        }
    });
    let sched = e.func("sched", 0, vec![], move |_, _, _, _| {
        log.borrow_mut().push("sched".to_string());
        Ok(undef())
    });
    let opts = bag(&e, &[("scheduler", sched)]);
    let runner = e.invoke("effect", &[body, opts]);
    let eff = e.get(&runner, "effect");
    e.log.borrow_mut().push(format!("dirty-before {}", js(&e.rt, &e.get(&eff, "dirty"))));
    e.set(&n, "value", num(1.0));
    e.log.borrow_mut().push(format!("dirty-after-write {}", js(&e.rt, &e.get(&eff, "dirty"))));
    e.log.borrow_mut().push(format!("runs {}", runs.get()));
    e.call(&runner, &undef(), &[]);
    e.log.borrow_mut().push(format!("dirty-after-run {}", js(&e.rt, &e.get(&eff, "dirty"))));
    check(&e, "effect-dirty-getter");
}

/// `effect-return-and-pause-tracking`: a read inside pauseTracking/resetTracking is not a dependency.
#[test]
fn effect_return_and_pause_tracking() {
    let e = Env::new();
    let a = e.invoke("ref", &[num(1.0)]);
    let b = e.invoke("ref", &[num(2.0)]);
    let runs = counter();
    let (r2, pause, reset) = (runs.clone(), e.api("pauseTracking"), e.api("resetTracking"));
    let body = e.func("body", 0, vec![a.clone(), b.clone(), pause, reset], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        let x = rt.to_number(&rt.get_str(&d[0], "value")?)?;
        rt.call(&d[2], &undef(), &[])?;
        let y = rt.to_number(&rt.get_str(&d[1], "value")?)?;
        rt.call(&d[3], &undef(), &[])?;
        Ok(num(x + y))
    });
    let runner = e.invoke("effect", &[body]);
    let returned = e.call(&runner, &undef(), &[]);
    e.log.borrow_mut().push(format!("returned {}", js(&e.rt, &returned)));
    e.set(&b, "value", num(5.0));
    e.log.borrow_mut().push(format!("runs-after-b {}", runs.get()));
    e.set(&a, "value", num(3.0));
    e.log.borrow_mut().push(format!("runs-after-a {}", runs.get()));
    check(&e, "effect-return-and-pause-tracking");
}

/// `effect-throw-stops-effect`: an effect whose first run throws is stopped by `effect()` and not re-run.
#[test]
fn effect_throw_stops_effect() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let runs = counter();
    let (log, r2) = (e.log.clone(), runs.clone());
    let body = e.func("body", 0, vec![n.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        let v = rt.get_str(&d[0], "value")?;
        if rt.to_number(&v)? == 0.0 {
            return Err(rt.throw_type_error("boom"));
        }
        log.borrow_mut().push(format!("ok {}", js(rt, &v)));
        Ok(undef())
    });
    let mut caught = "none".to_string();
    if let Err(t) = e.try_invoke("effect", &[body]) {
        caught = js(&e.rt, &e.get(&t.0, "name"));
    }
    e.log.borrow_mut().push(format!("caught {caught} runs {}", runs.get()));
    e.set(&n, "value", num(1.0));
    e.log.borrow_mut().push(format!("runs-after-write {}", runs.get()));
    check(&e, "effect-throw-stops-effect");
}

/// `effect-stop-in-own-run`: an effect may stop itself while it runs.
#[test]
fn effect_stop_in_own_run() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let slot = holder(&e.rt);
    let runs = counter();
    let (log, r2, stop) = (e.log.clone(), runs.clone(), e.api("stop"));
    let body = e.func("body", 0, vec![n.clone(), slot.clone(), stop], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        let v = rt.get_str(&d[0], "value")?;
        log.borrow_mut().push(format!("run {}", js(rt, &v)));
        if rt.to_number(&v)? == 1.0 {
            let me = rt.get_str(&d[1], "r")?;
            rt.call(&d[2], &undef(), &[me])?;
        }
        Ok(undef())
    });
    let runner = e.invoke("effect", &[body]);
    put(&e.rt, &slot, "r", runner);
    e.set(&n, "value", num(1.0));
    e.set(&n, "value", num(2.0));
    e.log.borrow_mut().push(format!("runs {}", runs.get()));
    check(&e, "effect-stop-in-own-run");
}

/// `effect-two-runners-one-stopped`
#[test]
fn effect_two_runners_one_stopped() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let make = |tag: &'static str| {
        let log = e.log.clone();
        e.func("body", 0, vec![n.clone()], move |rt, _, _, d| {
            log.borrow_mut().push(format!("{tag} {}", js(rt, &rt.get_str(&d[0], "value")?)));
            Ok(undef())
        })
    };
    let r1 = e.invoke("effect", &[make("r1")]);
    let r2 = e.invoke("effect", &[make("r2")]);
    e.invoke("stop", &[r1.clone()]);
    e.set(&n, "value", num(1.0));
    e.log.borrow_mut().push(format!("end {} {}", e.rt.typeof_(&r1), e.rt.typeof_(&r2)));
    check(&e, "effect-two-runners-one-stopped");
}

/// `batch-subscription-order`: a write runs the effects in subscription order.
#[test]
fn batch_subscription_order() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let effect = e.api("effect");
    for tag in ["A", "B", "C"] {
        let log = e.log.clone();
        let body = e.func("body", 0, vec![n.clone()], move |rt, _, _, d| {
            log.borrow_mut().push(format!("{tag} {}", js(rt, &rt.get_str(&d[0], "value")?)));
            Ok(undef())
        });
        e.call(&effect, &undef(), &[body]);
    }
    e.log.borrow_mut().push("-- write".to_string());
    e.set(&n, "value", num(1.0));
    check(&e, "batch-subscription-order");
}

/// `batch-computed-and-effects-share-dep`: a computed is read in the effects; the write recomputes it once.
#[test]
fn batch_computed_and_effects_share_dep() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let log = e.log.clone();
    let getter = e.func("getter", 0, vec![n.clone()], move |rt, _, _, d| {
        let v = rt.get_str(&d[0], "value")?;
        log.borrow_mut().push(format!("compute {}", js(rt, &v)));
        Ok(num(rt.to_number(&v)? * 10.0))
    });
    let c = e.invoke("computed", &[getter]);
    let log1 = e.log.clone();
    let e1 = e.func("E1", 0, vec![c.clone()], move |rt, _, _, d| {
        let v = rt.get_str(&d[0], "value")?;
        log1.borrow_mut().push(format!("E1 {}", js(rt, &v)));
        Ok(undef())
    });
    let log2 = e.log.clone();
    let e2 = e.func("E2", 0, vec![n.clone()], move |rt, _, _, d| {
        log2.borrow_mut().push(format!("E2 {}", js(rt, &rt.get_str(&d[0], "value")?)));
        Ok(undef())
    });
    e.invoke("effect", &[e1]);
    e.invoke("effect", &[e2]);
    e.log.borrow_mut().push("-- write".to_string());
    e.set(&n, "value", num(1.0));
    check(&e, "batch-computed-and-effects-share-dep");
}

/// `batch-nested-writes-propagate`: a write made by an effect runs the effects that read it before the writer ends.
#[test]
fn batch_nested_writes_propagate() {
    let e = Env::new();
    let src = e.invoke("ref", &[num(0.0)]);
    let mid = e.invoke("ref", &[num(0.0)]);
    let log = e.log.clone();
    let reader = e.func("reader", 0, vec![mid.clone()], move |rt, _, _, d| {
        log.borrow_mut().push(format!("mid-reader {}", js(rt, &rt.get_str(&d[0], "value")?)));
        Ok(undef())
    });
    let log2 = e.log.clone();
    let writer = e.func("writer", 0, vec![src.clone(), mid.clone()], move |rt, _, _, d| {
        let s = rt.get_str(&d[0], "value")?;
        rt.set_v_strict(&d[1], &PropKey::from_str("value"), num(rt.to_number(&s)? * 2.0))?;
        log2.borrow_mut().push(format!("writer {}", js(rt, &s)));
        Ok(undef())
    });
    e.invoke("effect", &[reader]);
    e.invoke("effect", &[writer]);
    e.log.borrow_mut().push("-- write".to_string());
    e.set(&src, "value", num(1.0));
    e.log.borrow_mut().push(format!("end {}", js(&e.rt, &e.get(&mid, "value"))));
    check(&e, "batch-nested-writes-propagate");
}

/// `track-manual-array-length`: trigger(arr, 'set', 'length', 2) reaches the length dep and the index dep >= 2.
#[test]
fn track_manual_array_length() {
    let e = Env::new();
    let arr = Value::Object(e.rt.new_array(vec![num(1.0), num(2.0), num(3.0)]));
    let (track, trigger) = (e.api("track"), e.api("trigger"));
    let len_runs = counter();
    let (log, l2) = (e.log.clone(), len_runs.clone());
    let b1 = e.func("lenBody", 0, vec![arr.clone(), track.clone()], move |rt, _, _, d| {
        l2.set(l2.get() + 1);
        rt.call(&d[1], &undef(), &[d[0].clone(), Env::text("get"), Env::text("length")])?;
        log.borrow_mut().push(format!("len-run {}", l2.get()));
        Ok(undef())
    });
    let idx_runs = counter();
    let (log2, i2) = (e.log.clone(), idx_runs.clone());
    let b2 = e.func("idxBody", 0, vec![arr.clone(), track.clone()], move |rt, _, _, d| {
        i2.set(i2.get() + 1);
        rt.call(&d[1], &undef(), &[d[0].clone(), Env::text("get"), Env::text("2")])?;
        log2.borrow_mut().push(format!("idx-run {}", i2.get()));
        Ok(undef())
    });
    e.invoke("effect", &[b1]);
    e.invoke("effect", &[b2]);
    e.call(&trigger, &undef(), &[arr.clone(), Env::text("set"), Env::text("length"), num(2.0)]);
    e.log.borrow_mut().push(format!("after-shrink {} {}", len_runs.get(), idx_runs.get()));
    check(&e, "track-manual-array-length");
}

/// `track-add-iterate-and-delete`: 'add' and 'delete' on a plain object trigger ITERATE_KEY; 'set' does not.
#[test]
fn track_add_iterate_and_delete() {
    let e = Env::new();
    let obj = e.rt.new_object();
    e.rt.define_data(obj, PropKey::from_str("a"), num(1.0), Flags::ALL).expect("a");
    let obj = Value::Object(obj);
    let (track, trigger, iterate) = (e.api("track"), e.api("trigger"), e.api("ITERATE_KEY"));
    let runs = counter();
    let (log, r2) = (e.log.clone(), runs.clone());
    let body = e.func("body", 0, vec![obj.clone(), track, iterate.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        rt.call(&d[1], &undef(), &[d[0].clone(), Env::text("iterate"), d[2].clone()])?;
        log.borrow_mut().push(format!("iter {}", r2.get()));
        Ok(undef())
    });
    e.invoke("effect", &[body]);
    e.call(&trigger, &undef(), &[obj.clone(), Env::text("add"), Env::text("b"), num(2.0)]);
    e.call(&trigger, &undef(), &[obj.clone(), Env::text("set"), Env::text("a"), num(3.0)]);
    e.call(&trigger, &undef(), &[obj.clone(), Env::text("delete"), Env::text("a"), undef()]);
    e.log.borrow_mut().push(format!("total {}", runs.get()));
    check(&e, "track-add-iterate-and-delete");
}

/// `track-clear-triggers-all`: 'clear' runs every dependent once.
#[test]
fn track_clear_triggers_all() {
    let e = Env::new();
    let obj = e.rt.new_object();
    e.rt.define_data(obj, PropKey::from_str("x"), num(1.0), Flags::ALL).expect("x");
    e.rt.define_data(obj, PropKey::from_str("y"), num(2.0), Flags::ALL).expect("y");
    let obj = Value::Object(obj);
    let (track, trigger) = (e.api("track"), e.api("trigger"));
    let runs = counter();
    let (log, r2) = (e.log.clone(), runs.clone());
    let body = e.func("body", 0, vec![obj.clone(), track], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        rt.call(&d[1], &undef(), &[d[0].clone(), Env::text("get"), Env::text("x")])?;
        rt.call(&d[1], &undef(), &[d[0].clone(), Env::text("get"), Env::text("y")])?;
        log.borrow_mut().push(format!("r {}", r2.get()));
        Ok(undef())
    });
    e.invoke("effect", &[body]);
    e.call(&trigger, &undef(), &[obj.clone(), Env::text("clear"), undef(), undef()]);
    e.log.borrow_mut().push(format!("total {}", runs.get()));
    check(&e, "track-clear-triggers-all");
}

/// `trigger-without-dependents`: triggering a target nobody tracks does nothing observable.
#[test]
fn trigger_without_dependents() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let trigger = e.api("trigger");
    let target = Value::Object(e.rt.new_object());
    e.call(&trigger, &undef(), &[target, Env::text("set"), Env::text("nope"), num(1.0)]);
    e.log.borrow_mut().push("ok".to_string());
    e.set(&n, "value", num(1.0));
    e.log.borrow_mut().push(format!("end {}", js(&e.rt, &e.get(&n, "value"))));
    check(&e, "trigger-without-dependents");
}

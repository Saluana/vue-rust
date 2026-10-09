//! Effect scopes. Each test is the Rust translation of one snippet in tests/vue/rust/snippets.json (id in the doc comment).

mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use common::{check, holder, js, put, take_from, Env};
use vn_value::{same_value, Value};

fn undef() -> Value {
    Value::Undefined
}

fn num(x: f64) -> Value {
    Value::Number(x)
}

fn is_undef(v: &Value) -> bool {
    matches!(v, Value::Undefined)
}

/// `scope-run-returns-and-current`
#[test]
fn scope_run_returns_and_current() {
    let e = Env::new();
    let scope = e.invoke("effectScope", &[]);
    let gcs = e.api("getCurrentScope");
    let outside = e.invoke("getCurrentScope", &[]);
    e.log.borrow_mut().push(format!("outside {}", is_undef(&outside)));
    let run = e.get(&scope, "run");
    let log = e.log.clone();
    let body = e.func("inside", 0, vec![gcs.clone(), scope.clone()], move |rt, _, _, d| {
        let cur = rt.call(&d[0], &undef(), &[])?;
        log.borrow_mut().push(format!("inside {}", same_value(&cur, &d[1])));
        Ok(num(42.0))
    });
    let got = e.call(&run, &scope, &[body]);
    let after = e.invoke("getCurrentScope", &[]);
    e.log.borrow_mut().push(format!("returned {} {}", js(&e.rt, &got), is_undef(&after)));
    e.call(&e.get(&scope, "stop"), &scope, &[]);
    let one = e.func("one", 0, vec![], |_, _, _, _| Ok(num(1.0)));
    let r = e.call(&run, &scope, &[one]);
    e.log.borrow_mut().push(format!("stopped-run {}", is_undef(&r)));
    check(&e, "scope-run-returns-and-current");
}

/// `scope-nested-stop-cascade`: parent effects, then parent cleanups, then the child's cleanups.
#[test]
fn scope_nested_stop_cascade() {
    let e = Env::new();
    let order = Rc::new(RefCell::new(Vec::<String>::new()));
    let osd = e.api("onScopeDispose");
    let mark = |name: &'static str| {
        let order = order.clone();
        e.func("cb", 0, vec![], move |_, _, _, _| {
            order.borrow_mut().push(name.to_string());
            Ok(undef())
        })
    };
    let (c1, c2, p1, p2) = (mark("child-1"), mark("child-2"), mark("parent-1"), mark("parent-2"));
    let child_body = e.func("childBody", 0, vec![osd.clone(), c1, c2], move |rt, _, _, d| {
        rt.call(&d[0], &undef(), &[d[1].clone()])?;
        rt.call(&d[0], &undef(), &[d[2].clone()])?;
        Ok(undef())
    });
    let es = e.api("effectScope");
    let parent_body = e.func("parentBody", 0, vec![osd.clone(), p1, p2, es.clone(), child_body], move |rt, _, _, d| {
        rt.call(&d[0], &undef(), &[d[1].clone()])?;
        let child = rt.call(&d[3], &undef(), &[])?;
        let run = rt.get_str(&child, "run")?;
        rt.call(&run, &child, &[d[4].clone()])?;
        rt.call(&d[0], &undef(), &[d[2].clone()])?;
        Ok(undef())
    });
    let parent = e.invoke("effectScope", &[]);
    let run = e.get(&parent, "run");
    e.call(&run, &parent, &[parent_body]);
    e.call(&e.get(&parent, "stop"), &parent, &[]);
    e.log.borrow_mut().push(format!("order {}", order.borrow().join(",")));
    check(&e, "scope-nested-stop-cascade");
}

/// `scope-effects-before-cleanups`: stopping a scope stops its effects silently, then runs cleanups in order.
#[test]
fn scope_effects_before_cleanups() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let log = e.log.clone();
    let effect_body = e.func("effectBody", 0, vec![n.clone()], {
        let log = log.clone();
        move |rt, _, _, d| {
            let v = rt.get_str(&d[0], "value")?;
            log.borrow_mut().push(format!("effect {}", js(rt, &v)));
            Ok(undef())
        }
    });
    let cb = |name: &'static str| {
        let log = log.clone();
        e.func("cb", 0, vec![], move |_, _, _, _| {
            log.borrow_mut().push(name.to_string());
            Ok(undef())
        })
    };
    let (cleanup1, cleanup2, child_cleanup) = (cb("cleanup-1"), cb("cleanup-2"), cb("child-cleanup"));
    let osd = e.api("onScopeDispose");
    let child_body = e.func("childBody", 0, vec![osd.clone(), child_cleanup], move |rt, _, _, d| {
        rt.call(&d[0], &undef(), &[d[1].clone()])?;
        Ok(undef())
    });
    let outer_body = e.func(
        "outerBody",
        0,
        vec![osd, e.api("effect"), effect_body, cleanup1, cleanup2, e.api("effectScope"), child_body],
        move |rt, _, _, d| {
            rt.call(&d[1], &undef(), &[d[2].clone()])?;
            rt.call(&d[0], &undef(), &[d[3].clone()])?;
            let child = rt.call(&d[5], &undef(), &[])?;
            let run = rt.get_str(&child, "run")?;
            rt.call(&run, &child, &[d[6].clone()])?;
            rt.call(&d[0], &undef(), &[d[4].clone()])?;
            Ok(undef())
        },
    );
    let scope = e.invoke("effectScope", &[]);
    e.call(&e.get(&scope, "run"), &scope, &[outer_body]);
    e.call(&e.get(&scope, "stop"), &scope, &[]);
    e.set(&n, "value", num(1.0));
    e.log.borrow_mut().push("done".to_string());
    check(&e, "scope-effects-before-cleanups");
}

/// `scope-detached-survives-parent`: a detached child scope keeps running after its parent stops.
#[test]
fn scope_detached_survives_parent() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let slot = holder(&e.rt);
    let log = e.log.clone();
    let effect_body = e.func("effectBody", 0, vec![n.clone()], {
        let log = log.clone();
        move |rt, _, _, d| {
            let v = rt.get_str(&d[0], "value")?;
            log.borrow_mut().push(format!("detached-effect {}", js(rt, &v)));
            Ok(undef())
        }
    });
    let cleanup = e.func("cleanup", 0, vec![], {
        let log = log.clone();
        move |_, _, _, _| {
            log.borrow_mut().push("detached-cleanup".to_string());
            Ok(undef())
        }
    });
    let osd = e.api("onScopeDispose");
    let detached_body = e.func("detachedBody", 0, vec![e.api("effect"), effect_body, osd.clone(), cleanup], move |rt, _, _, d| {
        rt.call(&d[0], &undef(), &[d[1].clone()])?;
        rt.call(&d[2], &undef(), &[d[3].clone()])?;
        Ok(undef())
    });
    let outer_body = e.func("outerBody", 0, vec![e.api("effectScope"), slot.clone(), detached_body], move |rt, _, _, d| {
        let det = rt.call(&d[0], &undef(), &[Value::Bool(true)])?;
        put(rt, &d[1], "d", det.clone());
        let run = rt.get_str(&det, "run")?;
        rt.call(&run, &det, &[d[2].clone()])?;
        Ok(undef())
    });
    let parent = e.invoke("effectScope", &[]);
    e.call(&e.get(&parent, "run"), &parent, &[outer_body]);
    put(&e.rt, &slot, "parent", parent.clone());
    e.call(&e.get(&parent, "stop"), &parent, &[]);
    e.set(&n, "value", num(1.0));
    e.log.borrow_mut().push("after-parent-stop".to_string());
    let detached = take_from(&e.rt, &slot, "d");
    e.call(&e.get(&detached, "stop"), &detached, &[]);
    e.log.borrow_mut().push("detached-stopped".to_string());
    check(&e, "scope-detached-survives-parent");
}

/// `scope-nested-run-restores-current`: a nested run restores the outer scope as current.
#[test]
fn scope_nested_run_restores_current() {
    let e = Env::new();
    let outer = e.invoke("effectScope", &[]);
    let inner = e.invoke("effectScope", &[]);
    let gcs = e.api("getCurrentScope");
    let is_current = |rt: &vn_value::Rt, gcs: &Value, s: &Value| -> bool {
        let cur = rt.call(gcs, &undef(), &[]).expect("getCurrentScope");
        same_value(&cur, s)
    };
    let cur0 = e.invoke("getCurrentScope", &[]);
    e.log.borrow_mut().push(format!("outside {}", is_undef(&cur0)));
    let inner_fn = e.func("innerFn", 0, vec![gcs.clone(), inner.clone()], move |rt, _, _, d| {
        let ok = is_current(rt, &d[0], &d[1]);
        Ok(Value::Bool(ok))
    });
    let log = e.log.clone();
    let outer_fn = e.func("outerFn", 0, vec![gcs.clone(), outer.clone(), inner.clone(), inner_fn], move |rt, _, _, d| {
        log.borrow_mut().push(format!("in-outer {}", is_current(rt, &d[0], &d[1])));
        let run = rt.get_str(&d[2], "run")?;
        let inner_ok = rt.call(&run, &d[2], &[d[3].clone()])?;
        log.borrow_mut().push(format!("in-inner {}", js(rt, &inner_ok)));
        log.borrow_mut().push(format!("back-in-outer {}", is_current(rt, &d[0], &d[1])));
        Ok(undef())
    });
    let run = e.get(&outer, "run");
    e.call(&run, &outer, &[outer_fn]);
    let after = e.invoke("getCurrentScope", &[]);
    e.log.borrow_mut().push(format!("after {}", is_undef(&after)));
    check(&e, "scope-nested-run-restores-current");
}

/// `scope-stop-idempotent`: a second stop does nothing; run after stop returns undefined.
#[test]
fn scope_stop_idempotent() {
    let e = Env::new();
    let s = e.invoke("effectScope", &[]);
    let cb = e.func("dispose", 0, vec![], {
        let log = e.log.clone();
        move |_, _, _, _| {
            log.borrow_mut().push("dispose".to_string());
            Ok(undef())
        }
    });
    let body = e.func("body", 0, vec![e.api("onScopeDispose"), cb], move |rt, _, _, d| {
        rt.call(&d[0], &undef(), &[d[1].clone()])?;
        Ok(undef())
    });
    e.call(&e.get(&s, "run"), &s, &[body]);
    e.call(&e.get(&s, "stop"), &s, &[]);
    e.call(&e.get(&s, "stop"), &s, &[]);
    e.log.borrow_mut().push(format!("active {}", js(&e.rt, &e.get(&s, "active"))));
    let one = e.func("one", 0, vec![], |_, _, _, _| Ok(num(1.0)));
    let r = e.call(&e.get(&s, "run"), &s, &[one]);
    e.log.borrow_mut().push(format!("run-after-stop {}", js(&e.rt, &r)));
    check(&e, "scope-stop-idempotent");
}

/// `scope-pause-resume-effects`: a paused scope queues its effects' runs until resume.
#[test]
fn scope_pause_resume_effects() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let runs = Rc::new(Cell::new(0));
    let log = e.log.clone();
    let r2 = runs.clone();
    let effect_body = e.func("effectBody", 0, vec![n.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        let v = rt.get_str(&d[0], "value")?;
        log.borrow_mut().push(format!("run {}", js(rt, &v)));
        Ok(undef())
    });
    let s = e.invoke("effectScope", &[]);
    let body = e.func("body", 0, vec![e.api("effect"), effect_body], move |rt, _, _, d| {
        rt.call(&d[0], &undef(), &[d[1].clone()])?;
        Ok(undef())
    });
    e.call(&e.get(&s, "run"), &s, &[body]);
    e.call(&e.get(&s, "pause"), &s, &[]);
    e.set(&n, "value", num(1.0));
    e.set(&n, "value", num(2.0));
    e.log.borrow_mut().push(format!("paused {}", runs.get()));
    e.call(&e.get(&s, "resume"), &s, &[]);
    e.log.borrow_mut().push(format!("resumed {}", runs.get()));
    check(&e, "scope-pause-resume-effects");
}

/// `scope-on-off-current`: on() makes the scope current without running a function; off() restores.
#[test]
fn scope_on_off_current() {
    let e = Env::new();
    let s = e.invoke("effectScope", &[]);
    e.call(&e.get(&s, "on"), &s, &[]);
    let cur = e.invoke("getCurrentScope", &[]);
    e.log.borrow_mut().push(format!("on {}", same_value(&cur, &s)));
    e.call(&e.get(&s, "off"), &s, &[]);
    let cur2 = e.invoke("getCurrentScope", &[]);
    e.log.borrow_mut().push(format!("off {}", is_undef(&cur2)));
    check(&e, "scope-on-off-current");
}

/// `scope-onscopedispose-without-scope`: with no active scope the callback is never registered.
#[test]
fn scope_onscopedispose_without_scope() {
    let e = Env::new();
    let never = e.func("never", 0, vec![], {
        let log = e.log.clone();
        move |_, _, _, _| {
            log.borrow_mut().push("never".to_string());
            Ok(undef())
        }
    });
    e.invoke("onScopeDispose", &[never]);
    let cur = e.invoke("getCurrentScope", &[]);
    e.log.borrow_mut().push(format!("ok {}", is_undef(&cur)));
    check(&e, "scope-onscopedispose-without-scope");
}

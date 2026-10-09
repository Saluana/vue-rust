//! The watch base (`watch` of @vue/reactivity 3.5.43). Each test is the Rust translation of one snippet in
//! tests/vue/rust/snippets.json (id in the doc comment); the expected lines come from the real package.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::{check, holder, js, make_fn, put, take_from, Env};
use vn_value::{same_value, Flags, PropKey, Res, Rt, Value};

fn num(x: f64) -> Value {
    Value::Number(x)
}

fn undef() -> Value {
    Value::Undefined
}

fn counter() -> Rc<Cell<i64>> {
    Rc::new(Cell::new(0))
}

fn bag(e: &Env, pairs: &[(&str, Value)]) -> Value {
    let o = e.rt.new_object();
    for (k, v) in pairs {
        e.rt.define_data(o, PropKey::from_str(k), v.clone(), Flags::ALL).expect("bag");
    }
    Value::Object(o)
}

/// `js` of every element joined by `|` (holes skipped), as the snippets join arrays.
fn join_pipe(rt: &Rt, arr: &Value) -> Res<String> {
    let len = rt.to_number(&rt.get_str(arr, "length")?)? as u32;
    let mut parts = Vec::new();
    for i in 0..len {
        parts.push(js(rt, &rt.get_v(arr, &PropKey::Index(i))?));
    }
    Ok(parts.join("|"))
}

/// `watch-getter-changes-only`
#[test]
fn watch_getter_changes_only() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(1.0)]);
    let getter = e.func("getter", 0, vec![n.clone()], |rt, _, _, d| {
        Ok(Value::Bool(rt.to_number(&rt.get_str(&d[0], "value")?)? > 0.0))
    });
    let cb = e.func("cb", 2, vec![], {
        let log = e.log.clone();
        move |rt, _, args, _| {
            log.borrow_mut().push(format!("cb {} {}", js(rt, &args[0]), js(rt, &args[1])));
            Ok(undef())
        }
    });
    e.invoke("watch", &[getter, cb]);
    e.set(&n, "value", num(2.0));
    e.set(&n, "value", num(-1.0));
    e.set(&n, "value", num(0.0));
    e.set(&n, "value", num(5.0));
    e.log.borrow_mut().push("end".to_string());
    check(&e, "watch-getter-changes-only");
}

/// `watch-ref-old-new`
#[test]
fn watch_ref_old_new() {
    let e = Env::new();
    let n = e.invoke("ref", &[Env::text("a")]);
    let cb = e.func("cb", 2, vec![], {
        let log = e.log.clone();
        move |rt, _, args, _| {
            log.borrow_mut().push(format!("cb {} {}", js(rt, &args[0]), js(rt, &args[1])));
            Ok(undef())
        }
    });
    e.invoke("watch", &[n.clone(), cb]);
    e.set(&n, "value", Env::text("b"));
    e.set(&n, "value", Env::text("b"));
    e.set(&n, "value", Env::text("c"));
    e.log.borrow_mut().push("end".to_string());
    check(&e, "watch-ref-old-new");
}

/// `watch-immediate`: with `immediate`, the callback runs during creation with an undefined old value.
#[test]
fn watch_immediate() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(1.0)]);
    let cb = e.func("cb", 2, vec![], {
        let log = e.log.clone();
        move |rt, _, args, _| {
            let old = if args[1].is_undefined() { "undef".to_string() } else { js(rt, &args[1]) };
            log.borrow_mut().push(format!("cb {} {old}", js(rt, &args[0])));
            Ok(undef())
        }
    });
    let opts = bag(&e, &[("immediate", Value::Bool(true))]);
    e.invoke("watch", &[n.clone(), cb, opts]);
    e.log.borrow_mut().push("after-create".to_string());
    e.set(&n, "value", num(2.0));
    e.log.borrow_mut().push("end".to_string());
    check(&e, "watch-immediate");
}

/// `watch-once`: the callback runs once and the watcher stops; the handle has a `stop` function.
#[test]
fn watch_once() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let cb = e.func("cb", 1, vec![], {
        let log = e.log.clone();
        move |rt, _, args, _| {
            log.borrow_mut().push(format!("cb {}", js(rt, &args[0])));
            Ok(undef())
        }
    });
    let opts = bag(&e, &[("once", Value::Bool(true))]);
    let h = e.invoke("watch", &[n.clone(), cb, opts]);
    e.set(&n, "value", num(1.0));
    e.set(&n, "value", num(2.0));
    e.log.borrow_mut().push(format!("end {}", e.rt.typeof_(&e.get(&h, "stop"))));
    check(&e, "watch-once");
}

/// `watch-deep-numeric-depth`: deep: N walks N levels below the source; a ref three levels down is seen at depth 4 only.
#[test]
fn watch_deep_numeric_depth() {
    let e = Env::new();
    let leaf = e.invoke("ref", &[num(0.0)]);
    let level2 = e.rt.new_object();
    e.rt.define_data(level2, PropKey::from_str("leaf"), leaf.clone(), Flags::ALL).expect("leaf");
    let level1 = e.rt.new_object();
    e.rt.define_data(level1, PropKey::from_str("level2"), Value::Object(level2), Flags::ALL).expect("level2");
    let root = e.rt.new_object();
    e.rt.define_data(root, PropKey::from_str("level1"), Value::Object(level1), Flags::ALL).expect("level1");
    let root = Value::Object(root);
    let shallow = counter();
    let deep = counter();
    let getter = e.func("root", 0, vec![root.clone()], |_, _, _, d| Ok(d[0].clone()));
    let s2 = shallow.clone();
    let cb_shallow = e.func("cbShallow", 0, vec![], move |_, _, _, _| {
        s2.set(s2.get() + 1);
        Ok(undef())
    });
    let d2 = deep.clone();
    let cb_deep = e.func("cbDeep", 0, vec![], move |_, _, _, _| {
        d2.set(d2.get() + 1);
        Ok(undef())
    });
    e.invoke("watch", &[getter.clone(), cb_shallow, bag(&e, &[("deep", num(3.0))])]);
    e.invoke("watch", &[getter, cb_deep, bag(&e, &[("deep", num(4.0))])]);
    e.set(&leaf, "value", num(1.0));
    e.log.borrow_mut().push(format!("calls {} {}", shallow.get(), deep.get()));
    check(&e, "watch-deep-numeric-depth");
}

/// `watch-deep-cycle-terminates`: a cyclic object walked deep terminates; the callback runs once per change.
#[test]
fn watch_deep_cycle_terminates() {
    let e = Env::new();
    let a = e.rt.new_object();
    let b = e.rt.new_object();
    e.rt.define_data(a, PropKey::from_str("name"), Env::text("a"), Flags::ALL).expect("name");
    e.rt.define_data(b, PropKey::from_str("name"), Env::text("b"), Flags::ALL).expect("name");
    e.rt.define_data(b, PropKey::from_str("peer"), Value::Object(a), Flags::ALL).expect("peer");
    e.rt.define_data(a, PropKey::from_str("peer"), Value::Object(b), Flags::ALL).expect("peer");
    let a = Value::Object(a);
    let n = e.invoke("ref", &[num(0.0)]);
    let getter = e.func("getter", 0, vec![n.clone(), a.clone()], |rt, _, _, d| {
        rt.get_str(&d[0], "value")?;
        Ok(d[1].clone())
    });
    let calls = counter();
    let k = calls.clone();
    let cb = e.func("cb", 0, vec![], {
        let log = e.log.clone();
        move |_, _, _, _| {
            k.set(k.get() + 1);
            log.borrow_mut().push(format!("cb {}", k.get()));
            Ok(undef())
        }
    });
    let opts = bag(&e, &[("deep", Value::Bool(true))]);
    e.invoke("watch", &[getter, cb, opts]);
    e.set(&n, "value", num(1.0));
    e.log.borrow_mut().push(format!("end {}", calls.get()));
    check(&e, "watch-deep-cycle-terminates");
}

/// `watch-cleanup-before-rerun`: the cleanup registered by a callback runs before the next callback.
#[test]
fn watch_cleanup_before_rerun() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let table = e.table();
    let log = e.log.clone();
    let cb = e.func("cb", 3, vec![], move |rt, _, args, _| {
        let v = args[0].clone();
        log.borrow_mut().push(format!("cb {}", js(rt, &v)));
        let log2 = log.clone();
        let cleanup = make_fn(rt, &table, "cleanup", 0, vec![v], move |rt, _, _, d| {
            log2.borrow_mut().push(format!("cleanup {}", js(rt, &d[0])));
            Ok(undef())
        });
        rt.call(&args[2], &undef(), &[cleanup])?;
        Ok(undef())
    });
    e.invoke("watch", &[n.clone(), cb]);
    e.set(&n, "value", num(1.0));
    e.set(&n, "value", num(2.0));
    e.log.borrow_mut().push("end".to_string());
    check(&e, "watch-cleanup-before-rerun");
}

/// `watch-onwatchercleanup-outside-noop`: outside a watcher, onWatcherCleanup registers nothing and does not throw.
#[test]
fn watch_onwatchercleanup_outside_noop() {
    let e = Env::new();
    let never = e.func("never", 0, vec![], {
        let log = e.log.clone();
        move |_, _, _, _| {
            log.borrow_mut().push("never".to_string());
            Ok(undef())
        }
    });
    e.invoke("onWatcherCleanup", &[never]);
    let cur = e.invoke("getCurrentWatcher", &[]);
    e.log.borrow_mut().push(format!("current {}", cur.is_undefined()));
    check(&e, "watch-onwatchercleanup-outside-noop");
}

/// `watch-stop-inside-callback`: the callback stops its own watcher through the handle.
#[test]
fn watch_stop_inside_callback() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let slot = holder(&e.rt);
    let cb = e.func("cb", 1, vec![slot.clone()], {
        let log = e.log.clone();
        move |rt, _, args, d| {
            log.borrow_mut().push(format!("cb {}", js(rt, &args[0])));
            if matches!(args[0], Value::Number(x) if x == 2.0) {
                let h = rt.get_str(&d[0], "h")?;
                let stop = rt.get_str(&h, "stop")?;
                rt.call(&stop, &undef(), &[])?;
            }
            Ok(undef())
        }
    });
    let h = e.invoke("watch", &[n.clone(), cb]);
    put(&e.rt, &slot, "h", h);
    e.set(&n, "value", num(1.0));
    e.set(&n, "value", num(2.0));
    e.set(&n, "value", num(3.0));
    e.log.borrow_mut().push("end".to_string());
    check(&e, "watch-stop-inside-callback");
}

/// `watch-pause-resume-coalesces`: writes while paused are delivered once on resume.
#[test]
fn watch_pause_resume_coalesces() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let cb = e.func("cb", 2, vec![], {
        let log = e.log.clone();
        move |rt, _, args, _| {
            log.borrow_mut().push(format!("cb {} {}", js(rt, &args[0]), js(rt, &args[1])));
            Ok(undef())
        }
    });
    let h = e.invoke("watch", &[n.clone(), cb]);
    e.call(&e.get(&h, "pause"), &h, &[]);
    e.set(&n, "value", num(1.0));
    e.set(&n, "value", num(2.0));
    e.log.borrow_mut().push("paused".to_string());
    e.call(&e.get(&h, "resume"), &h, &[]);
    e.log.borrow_mut().push("resumed".to_string());
    e.set(&n, "value", num(3.0));
    e.log.borrow_mut().push("end".to_string());
    check(&e, "watch-pause-resume-coalesces");
}

/// `watch-array-of-sources`: a multi-source watch passes arrays of new and old values.
#[test]
fn watch_array_of_sources() {
    let e = Env::new();
    let a = e.invoke("ref", &[num(1.0)]);
    let b = e.invoke("ref", &[Env::text("x")]);
    let sources = Value::Object(e.rt.new_array(vec![a.clone(), b.clone()]));
    let cb = e.func("cb", 2, vec![], {
        let log = e.log.clone();
        move |rt, _, args, _| {
            let vals = join_pipe(rt, &args[0])?;
            let old_len = rt.to_number(&rt.get_str(&args[1], "length")?)?;
            let old = if old_len == 0.0 { "none".to_string() } else { join_pipe(rt, &args[1])? };
            log.borrow_mut().push(format!("cb {vals} {old}"));
            Ok(undef())
        }
    });
    e.invoke("watch", &[sources, cb]);
    e.set(&a, "value", num(2.0));
    e.set(&b, "value", Env::text("y"));
    e.log.borrow_mut().push("end".to_string());
    check(&e, "watch-array-of-sources");
}

/// `watch-throw-in-getter-then-change`: a throwing getter rejects the watch creation, but the watcher stays subscribed.
#[test]
fn watch_throw_in_getter_then_change() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let getter = e.func("getter", 0, vec![n.clone()], |rt, _, _, d| {
        let v = rt.get_str(&d[0], "value")?;
        if rt.to_number(&v)? == 0.0 {
            return Err(rt.throw_range_error("x"));
        }
        Ok(v)
    });
    let cb = e.func("cb", 0, vec![], {
        let log = e.log.clone();
        move |_, _, _, _| {
            log.borrow_mut().push("cb".to_string());
            Ok(undef())
        }
    });
    let got = match e.try_invoke("watch", &[getter, cb]) {
        Ok(_) => "none".to_string(),
        Err(t) => js(&e.rt, &e.get(&t.0, "name")),
    };
    e.log.borrow_mut().push(format!("created {got}"));
    e.set(&n, "value", num(1.0));
    e.log.borrow_mut().push("end".to_string());
    check(&e, "watch-throw-in-getter-then-change");
}

/// `watch-throw-in-callback-reaches-writer`: an error thrown by a callback surfaces at the write that caused it.
#[test]
fn watch_throw_in_callback_reaches_writer() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let cb = e.func("cb", 0, vec![], |rt, _, _, _| Err(rt.throw_range_error("cb-failed")));
    e.invoke("watch", &[n.clone(), cb]);
    let got = match e.try_set(&n, "value", num(1.0)) {
        Ok(()) => "none".to_string(),
        Err(t) => js(&e.rt, &e.get(&t.0, "name")),
    };
    e.log.borrow_mut().push(format!("writer-got {got}"));
    e.log.borrow_mut().push(format!("still-live {}", js(&e.rt, &e.get(&n, "value"))));
    check(&e, "watch-throw-in-callback-reaches-writer");
}

/// `watch-augment-job-and-call`: augmentJob receives the job (a function); the call option wraps the callback call.
#[test]
fn watch_augment_job_and_call() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let job_seen = Rc::new(Cell::new(false));
    let js2 = job_seen.clone();
    let augment = e.func("augment", 1, vec![], move |rt, _, args, _| {
        let job = args[0].clone();
        js2.set(rt.typeof_(&job) == "function");
        rt.set_v_strict(&job, &PropKey::from_str("tag"), Env::text("tagged"))?;
        Ok(undef())
    });
    let call = e.func("call", 3, vec![], {
        let log = e.log.clone();
        move |rt, _, args, _| {
            log.borrow_mut().push(format!("call {}", js(rt, &args[1])));
            let fn_args = args[2].clone();
            let len = rt.to_number(&rt.get_str(&fn_args, "length")?)? as u32;
            let mut items = Vec::new();
            for i in 0..len {
                items.push(rt.get_v(&fn_args, &PropKey::Index(i))?);
            }
            rt.call(&args[0], &undef(), &items)
        }
    });
    let cb = e.func("cb", 1, vec![], {
        let log = e.log.clone();
        move |rt, _, args, _| {
            log.borrow_mut().push(format!("cb {}", js(rt, &args[0])));
            Ok(undef())
        }
    });
    let opts = bag(&e, &[("augmentJob", augment), ("call", call)]);
    e.invoke("watch", &[n.clone(), cb, opts]);
    e.set(&n, "value", num(1.0));
    e.log.borrow_mut().push(format!("job {}", job_seen.get()));
    check(&e, "watch-augment-job-and-call");
}

/// `watch-triggers-watch`: a callback that writes a ref triggers a second watcher.
#[test]
fn watch_triggers_watch() {
    let e = Env::new();
    let a = e.invoke("ref", &[num(0.0)]);
    let b = e.invoke("ref", &[num(0.0)]);
    let cb_a = e.func("aCb", 1, vec![b.clone()], {
        let log = e.log.clone();
        move |rt, _, args, d| {
            log.borrow_mut().push(format!("a-cb {}", js(rt, &args[0])));
            let doubled = rt.to_number(&args[0])? * 2.0;
            rt.set_v_strict(&d[0], &PropKey::from_str("value"), num(doubled))?;
            Ok(undef())
        }
    });
    let cb_b = e.func("bCb", 1, vec![], {
        let log = e.log.clone();
        move |rt, _, args, _| {
            log.borrow_mut().push(format!("b-cb {}", js(rt, &args[0])));
            Ok(undef())
        }
    });
    e.invoke("watch", &[a.clone(), cb_a]);
    e.invoke("watch", &[b.clone(), cb_b]);
    e.set(&a, "value", num(1.0));
    e.log.borrow_mut().push(format!("end {}", js(&e.rt, &e.get(&b, "value"))));
    check(&e, "watch-triggers-watch");
}

/// `watch-no-callback-cleanup-and-stop`: a getter-only watch runs at creation; its cleanup runs before each rerun and on stop.
#[test]
fn watch_no_callback_cleanup_and_stop() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let table = e.table();
    let log = e.log.clone();
    let source = e.func("source", 1, vec![n.clone()], move |rt, _, args, d| {
        let v = rt.get_str(&d[0], "value")?;
        log.borrow_mut().push(format!("run {}", js(rt, &v)));
        let log2 = log.clone();
        let cleanup = make_fn(rt, &table, "cleanup", 0, vec![v], move |rt, _, _, d| {
            log2.borrow_mut().push(format!("cleanup {}", js(rt, &d[0])));
            Ok(undef())
        });
        rt.call(&args[0], &undef(), &[cleanup])?;
        Ok(undef())
    });
    let h = e.invoke("watch", &[source]);
    e.set(&n, "value", num(1.0));
    e.call(&e.get(&h, "stop"), &h, &[]);
    e.set(&n, "value", num(2.0));
    e.log.borrow_mut().push("end".to_string());
    check(&e, "watch-no-callback-cleanup-and-stop");
}

/// `watch-scheduler-option`: a scheduler receives the job and decides when it runs.
#[test]
fn watch_scheduler_option() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let slot = holder(&e.rt);
    let sched = e.func("sched", 2, vec![slot.clone()], {
        let log = e.log.clone();
        move |rt, _, args, d| {
            log.borrow_mut().push(format!("sched {}", js(rt, &args[1])));
            put(rt, &d[0], "job", args[0].clone());
            Ok(undef())
        }
    });
    let cb = e.func("cb", 2, vec![], {
        let log = e.log.clone();
        move |rt, _, args, _| {
            log.borrow_mut().push(format!("cb {} {}", js(rt, &args[0]), js(rt, &args[1])));
            Ok(undef())
        }
    });
    let opts = bag(&e, &[("scheduler", sched)]);
    e.invoke("watch", &[n.clone(), cb, opts]);
    e.log.borrow_mut().push("created".to_string());
    e.set(&n, "value", num(1.0));
    e.log.borrow_mut().push("pending 1".to_string());
    let job = take_from(&e.rt, &slot, "job");
    e.call(&job, &undef(), &[Value::Bool(false)]);
    e.log.borrow_mut().push("end".to_string());
    check(&e, "watch-scheduler-option");
}

/// `watch-shallow-ref-set-only`: a shallow ref source fires on replacement only, not on nested writes.
#[test]
fn watch_shallow_ref_set_only() {
    let e = Env::new();
    let obj = e.rt.new_object();
    e.rt.define_data(obj, PropKey::from_str("v"), num(0.0), Flags::ALL).expect("v");
    let obj = Value::Object(obj);
    let s = e.invoke("shallowRef", &[obj.clone()]);
    let cb = e.func("cb", 2, vec![], {
        let log = e.log.clone();
        move |rt, _, args, _| {
            let v = rt.get_str(&args[0], "v")?;
            log.borrow_mut().push(format!("cb {} {}", js(rt, &v), same_value(&args[1], &args[0])));
            Ok(undef())
        }
    });
    e.invoke("watch", &[s.clone(), cb]);
    e.set(&obj, "v", num(1.0));
    e.set(&s, "value", obj.clone());
    let next = e.rt.new_object();
    e.rt.define_data(next, PropKey::from_str("v"), num(2.0), Flags::ALL).expect("v");
    e.set(&s, "value", Value::Object(next));
    e.log.borrow_mut().push("end".to_string());
    check(&e, "watch-shallow-ref-set-only");
}

/// `watch-effect-self-write-no-rerun`: a getter-only watch that writes its own dep does not re-run while it runs.
#[test]
fn watch_effect_self_write_no_rerun() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let runs = counter();
    let r2 = runs.clone();
    let source = e.func("source", 0, vec![n.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        let v = rt.to_number(&rt.get_str(&d[0], "value")?)?;
        if v < 3.0 {
            rt.set_v_strict(&d[0], &PropKey::from_str("value"), num(v + 1.0))?;
        }
        Ok(undef())
    });
    e.invoke("watch", &[source]);
    e.log.borrow_mut().push(format!("runs {} n {}", runs.get(), js(&e.rt, &e.get(&n, "value"))));
    check(&e, "watch-effect-self-write-no-rerun");
}

/// `scope-watch-stopped-with-scope`: a watcher created in a scope stops when the scope stops.
#[test]
fn scope_watch_stopped_with_scope() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let cb = e.func("cb", 1, vec![], {
        let log = e.log.clone();
        move |rt, _, args, _| {
            log.borrow_mut().push(format!("cb {}", js(rt, &args[0])));
            Ok(undef())
        }
    });
    let scope = e.invoke("effectScope", &[]);
    let body = e.func("body", 0, vec![e.api("watch"), n.clone(), cb], |rt, _, _, d| {
        rt.call(&d[0], &undef(), &[d[1].clone(), d[2].clone()])
    });
    e.call(&e.get(&scope, "run"), &scope, &[body]);
    e.set(&n, "value", num(1.0));
    e.call(&e.get(&scope, "stop"), &scope, &[]);
    e.set(&n, "value", num(2.0));
    e.log.borrow_mut().push("done".to_string());
    check(&e, "scope-watch-stopped-with-scope");
}

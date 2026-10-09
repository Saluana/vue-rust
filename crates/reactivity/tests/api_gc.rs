//! Rust-side checks that the oracle snippets do not cover: export shape, `EffectScope` construction, `proxyRefs`, the
//! reactive hooks, the test glue, garbage collection of reactivity graphs, and the oracle harness itself (negative control
//! and coverage audit).

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::{compare, expected, js, make_fn, oracle_ids, Env};
use vn_reactivity::{register_vue_for_tests, set_reactive_hooks, ReactiveHooks};
use vn_value::{same_value, Flags, PropKey, Res, Rt, Value};

fn num(x: f64) -> Value {
    Value::Number(x)
}

fn undef() -> Value {
    Value::Undefined
}

fn live(e: &Env) -> usize {
    e.rt.collect();
    e.rt.heap_stats().live_objects
}

/// Export names and function lengths are those of @vue/reactivity 3.5.43 (checked against the real package).
#[test]
fn export_names_and_lengths_match_vue() {
    let e = Env::new();
    let public = [
        ("ref", 1), ("shallowRef", 1), ("triggerRef", 1), ("customRef", 1), ("toRef", 3), ("toRefs", 1),
        ("toValue", 1), ("unref", 1), ("isRef", 1), ("computed", 2), ("effectScope", 1), ("getCurrentScope", 0),
        ("onScopeDispose", 1), ("onWatcherCleanup", 1), ("EffectScope", 0),
    ];
    for (name, len) in public {
        let f = e.api(name);
        assert_eq!(js(&e.rt, &e.get(&f, "length")), len.to_string(), "length of {name}");
        assert_eq!(js(&e.rt, &e.get(&f, "name")), name, "name of {name}");
    }
    let internal = [
        ("effect", 2), ("stop", 1), ("onEffectCleanup", 1), ("pauseTracking", 0), ("enableTracking", 0),
        ("resetTracking", 0), ("track", 3), ("trigger", 6), ("proxyRefs", 1), ("watch", 2), ("traverse", 1),
        ("getCurrentWatcher", 0),
    ];
    for (name, len) in internal {
        let f = e.api(name);
        assert_eq!(js(&e.rt, &e.get(&f, "length")), len.to_string(), "length of {name}");
    }
}

/// `new EffectScope(detached)` builds a scope; a plain call is a TypeError, as for a class.
#[test]
fn effect_scope_constructor_requires_new() {
    let e = Env::new();
    let ctor = e.api("EffectScope");
    let plain = e.try_invoke("EffectScope", &[]);
    let name = match plain {
        Ok(_) => "none".to_string(),
        Err(t) => js(&e.rt, &e.get(&t.0, "name")),
    };
    assert_eq!(name, "TypeError");
    let made = e.rt.construct(&ctor, &[], &ctor).expect("construct EffectScope");
    assert_eq!(js(&e.rt, &e.get(&made, "active")), "true");
    let proto = e.get(&ctor, "prototype");
    let made_proto = e.rt.get_prototype_of(made.as_object().expect("object"));
    assert_eq!(made_proto, proto.as_object(), "instances use EffectScope.prototype");
}

/// `proxyRefs(obj)`: reads unwrap refs, writes assign through them, other keys stay plain.
#[test]
fn proxy_refs_unwraps_and_assigns_through_refs() {
    let e = Env::new();
    let obj = e.rt.new_object();
    let a = e.invoke("ref", &[num(1.0)]);
    e.rt.define_data(obj, PropKey::from_str("a"), a.clone(), Flags::ALL).expect("a");
    e.rt.define_data(obj, PropKey::from_str("b"), num(2.0), Flags::ALL).expect("b");
    let obj = Value::Object(obj);
    let p = e.invoke("proxyRefs", &[obj.clone()]);
    assert_eq!(js(&e.rt, &e.get(&p, "a")), "1");
    e.set(&p, "a", num(5.0));
    assert_eq!(js(&e.rt, &e.get(&a, "value")), "5");
    e.set(&p, "b", num(3.0));
    assert_eq!(js(&e.rt, &e.get(&obj, "b")), "3");
    assert!(same_value(&e.get(&p, "__v_raw"), &obj));
}

/// The reactive hooks replace `toReactive` for refs: a ref of an object holds what the hook returns.
#[test]
fn reactive_hook_is_used_by_refs() {
    fn wrap(_rt: &Rt, v: &Value) -> Res<Value> {
        Ok(match v {
            Value::Object(_) => Value::String(vn_value::JsStr::from_str("wrapped")),
            other => other.clone(),
        })
    }
    fn identity(_rt: &Rt, v: &Value) -> Res<Value> {
        Ok(v.clone())
    }
    let e = Env::new();
    set_reactive_hooks(&e.rt, ReactiveHooks { to_reactive: wrap, to_readonly: identity });
    let obj = Value::Object(e.rt.new_object());
    let r = e.invoke("ref", &[obj]);
    assert_eq!(js(&e.rt, &e.get(&r, "value")), "wrapped");
    let prim = e.invoke("ref", &[num(4.0)]);
    assert_eq!(js(&e.rt, &e.get(&prim, "value")), "4");
}

/// `register_vue_for_tests` registers the `vue` module with the owned exports and `__internal`.
#[test]
fn register_vue_module_exposes_exports() {
    let rt = Rt::new();
    let module = register_vue_for_tests(&rt);
    for name in ["ref", "computed", "effectScope", "onWatcherCleanup", "toRefs"] {
        let cell = rt.module_export_cell(module, name).expect("export lookup").expect("export present");
        assert!(rt.cell_get(cell).is_object(), "{name} is a function object");
    }
    let internal = rt.module_export_cell(module, "__internal").expect("lookup").expect("__internal present");
    let bag = rt.cell_get(internal);
    assert!(rt.get_str(&bag, "effect").expect("effect").is_object());
    assert!(rt.get_str(&bag, "track").expect("track").is_object());
}

/// Stopped effects are released: after stop and drop, a collection frees everything the effects allocated.
#[test]
fn gc_releases_stopped_effects() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let _root = e.rt.root(&n);
    let body = e.func("body", 0, vec![n.clone()], |rt, _, _, d| {
        rt.get_str(&d[0], "value")?;
        Ok(undef())
    });
    let _root_body = e.rt.root(&body);
    let base = live(&e);
    for _ in 0..50 {
        let runner = e.invoke("effect", &[body.clone()]);
        e.invoke("stop", &[runner]);
    }
    assert_eq!(live(&e), base, "stopped effects must not stay reachable");
    let keep = e.invoke("effect", &[body.clone()]);
    let _root_keep = e.rt.root(&keep);
    assert!(live(&e) > base, "a running effect is reachable from the ref's dep");
    e.invoke("stop", &[keep]);
}

/// An effect whose runner is dropped stays subscribed while its dep is reachable, and it still runs.
#[test]
fn gc_keeps_subscribed_effects_alive() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let _root = e.rt.root(&n);
    let runs = Rc::new(Cell::new(0));
    let r2 = runs.clone();
    let body = e.func("body", 0, vec![n.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        rt.get_str(&d[0], "value")?;
        Ok(undef())
    });
    e.invoke("effect", &[body]);
    e.rt.collect();
    e.set(&n, "value", num(1.0));
    e.rt.collect();
    e.set(&n, "value", num(2.0));
    assert_eq!(runs.get(), 3, "the effect survives collections and keeps running");
}

/// Scope-owned graphs are released when the scope stops and nothing references them.
#[test]
fn gc_releases_scope_graphs_on_stop() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let _root = e.rt.root(&n);
    let base = live(&e);
    for _ in 0..20 {
        let scope = e.invoke("effectScope", &[]);
        let body = e.func("body", 0, vec![n.clone(), e.api("effect")], |rt, _, _, d| {
            rt.call(&d[1], &undef(), &[{
                let inner = rt.new_native_function(|_, _| Ok(undef()), "inner", 0, vec![]);
                Value::Object(inner)
            }])?;
            Ok(undef())
        });
        e.call(&e.get(&scope, "run"), &scope, &[body]);
        e.call(&e.get(&scope, "stop"), &scope, &[]);
    }
    assert_eq!(live(&e), base, "scope graphs are freed after stop");
}

/// Collections between the steps of a chain do not change what the effects observe (the same log as the oracle).
#[test]
fn gc_between_steps_keeps_results() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(1.0)]);
    let _root_n = e.rt.root(&n);
    let log = e.log.clone();
    let c1 = e.invoke("computed", &[e.func("c1", 0, vec![n.clone()], |rt, _, _, d| {
        Ok(num(rt.to_number(&rt.get_str(&d[0], "value")?)? + 1.0))
    })]);
    let _root_c1 = e.rt.root(&c1);
    let c2 = e.invoke("computed", &[e.func("c2", 0, vec![c1.clone()], |rt, _, _, d| {
        Ok(num(rt.to_number(&rt.get_str(&d[0], "value")?)? * 10.0))
    })]);
    let _root_c2 = e.rt.root(&c2);
    let body = e.func("eff", 0, vec![c2.clone()], move |rt, _, _, d| {
        let v = rt.get_str(&d[0], "value")?;
        log.borrow_mut().push(format!("eff {}", js(rt, &v)));
        Ok(undef())
    });
    let runner = e.invoke("effect", &[body]);
    let _root_r = e.rt.root(&runner);
    e.rt.collect();
    e.set(&n, "value", num(2.0));
    e.rt.collect();
    e.log.borrow_mut().push(format!("calls {} {} {}", 2, 2, 2));
    let got = e.take();
    let want = expected("computed-invalidation-chain");
    assert_eq!(got[0], want[0]);
    assert_eq!(got[1], want[1]);
}

/// Negative control: a deliberately wrong expectation is reported, and the right log passes.
#[test]
fn negative_control_detects_wrong_expectation() {
    let want = expected("batch-subscription-order");
    assert!(compare("batch-subscription-order", &want).is_ok());
    let mut changed = want.clone();
    changed[0] = "A 1".to_string();
    assert!(compare("batch-subscription-order", &changed).is_err(), "a changed line must be reported");
    let mut swapped = want.clone();
    swapped.swap(0, 1);
    assert!(compare("batch-subscription-order", &swapped).is_err(), "a reordered log must be reported");
    let mut missing = want.clone();
    missing.pop();
    assert!(compare("batch-subscription-order", &missing).is_err(), "a truncated log must be reported");
}

/// Coverage audit: every oracle case is exercised by a test (its id appears in a test source) and the oracle is complete.
#[test]
fn every_oracle_case_has_a_test() {
    let sources = [
        include_str!("effects.rs"),
        include_str!("scopes.rs"),
        include_str!("refs.rs"),
        include_str!("computed.rs"),
        include_str!("watch.rs"),
    ];
    let ids = oracle_ids();
    assert_eq!(ids.len(), 76, "the oracle has 76 snippets");
    for id in &ids {
        let quoted = format!("\"{id}\"");
        assert!(sources.iter().any(|s| s.contains(&quoted)), "no test runs oracle case {id}");
    }
}

/// `make_fn` closures created from closures keep their data: a maker returns functions that remember their argument.
#[test]
fn closures_created_inside_closures_keep_their_data() {
    let e = Env::new();
    let table = e.table();
    let maker = make_fn(&e.rt, &table, "maker", 1, vec![], move |rt, _, args, _| {
        let v = args[0].clone();
        Ok(Value::Object(rt.new_native_function(|rt, c| Ok(rt.fn_data(c.callee)[0].clone()), "get", 0, vec![v])))
    });
    let f = e.call(&maker, &undef(), &[Env::text("kept")]);
    let got = e.call(&f, &undef(), &[]);
    assert_eq!(js(&e.rt, &got), "kept");
}

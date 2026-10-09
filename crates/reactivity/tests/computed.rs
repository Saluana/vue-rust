//! Computed values: laziness, caching, invalidation, equal-result suppression, writable computeds, errors. Each test is the
//! Rust translation of one snippet in tests/vue/rust/snippets.json (id in the doc comment).

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::{check, js, Env};
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

fn line(e: &Env, text: String) {
    e.log.borrow_mut().push(text);
}

/// `computed-lazy-cached`: the getter runs on the first read, then only after a change.
#[test]
fn computed_lazy_cached() {
    let e = Env::new();
    let calls = counter();
    let n = e.invoke("ref", &[num(1.0)]);
    let c2 = calls.clone();
    let getter = e.func("getter", 0, vec![n.clone()], move |rt, _, _, d| {
        c2.set(c2.get() + 1);
        Ok(num(rt.to_number(&rt.get_str(&d[0], "value")?)? * 2.0))
    });
    let doubled = e.invoke("computed", &[getter]);
    line(&e, format!("before-read {}", calls.get()));
    let v1 = e.get(&doubled, "value");
    let v2 = e.get(&doubled, "value");
    let v3 = e.get(&doubled, "value");
    line(&e, format!("reads {} {} {} {}", js(&e.rt, &v1), js(&e.rt, &v2), js(&e.rt, &v3), calls.get()));
    e.set(&n, "value", num(2.0));
    line(&e, format!("stale-read-not-yet {}", calls.get()));
    let v4 = e.get(&doubled, "value");
    line(&e, format!("after-change {} {}", js(&e.rt, &v4), calls.get()));
    check(&e, "computed-lazy-cached");
}

/// `computed-invalidation-chain`: a change reaches an effect through two computeds, each recomputed once.
#[test]
fn computed_invalidation_chain() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(1.0)]);
    let (c1_calls, c2_calls) = (counter(), counter());
    let k1 = c1_calls.clone();
    let c1_getter = e.func("c1", 0, vec![n.clone()], move |rt, _, _, d| {
        k1.set(k1.get() + 1);
        Ok(num(rt.to_number(&rt.get_str(&d[0], "value")?)? + 1.0))
    });
    let c1 = e.invoke("computed", &[c1_getter]);
    let k2 = c2_calls.clone();
    let c2_getter = e.func("c2", 0, vec![c1.clone()], move |rt, _, _, d| {
        k2.set(k2.get() + 1);
        Ok(num(rt.to_number(&rt.get_str(&d[0], "value")?)? * 10.0))
    });
    let c2 = e.invoke("computed", &[c2_getter]);
    let eff_runs = counter();
    let (er, log) = (eff_runs.clone(), e.log.clone());
    let body = e.func("body", 0, vec![c2.clone()], move |rt, _, _, d| {
        er.set(er.get() + 1);
        log.borrow_mut().push(format!("eff {}", js(rt, &rt.get_str(&d[0], "value")?)));
        Ok(undef())
    });
    e.invoke("effect", &[body]);
    e.set(&n, "value", num(2.0));
    line(&e, format!("calls {} {} {}", c1_calls.get(), c2_calls.get(), eff_runs.get()));
    check(&e, "computed-invalidation-chain");
}

/// `computed-diamond-single-recompute`: a diamond recomputes the bottom node once per write.
#[test]
fn computed_diamond_single_recompute() {
    let e = Env::new();
    let a = e.invoke("ref", &[num(1.0)]);
    let d_calls = counter();
    let b_getter = e.func("b", 0, vec![a.clone()], |rt, _, _, d| {
        Ok(num(rt.to_number(&rt.get_str(&d[0], "value")?)? + 1.0))
    });
    let c_getter = e.func("c", 0, vec![a.clone()], |rt, _, _, d| {
        Ok(num(rt.to_number(&rt.get_str(&d[0], "value")?)? * 2.0))
    });
    let b = e.invoke("computed", &[b_getter]);
    let c = e.invoke("computed", &[c_getter]);
    let dk = d_calls.clone();
    let d_getter = e.func("d", 0, vec![b.clone(), c.clone()], move |rt, _, _, d| {
        dk.set(dk.get() + 1);
        let s = rt.to_number(&rt.get_str(&d[0], "value")?)? + rt.to_number(&rt.get_str(&d[1], "value")?)?;
        Ok(num(s))
    });
    let dn = e.invoke("computed", &[d_getter]);
    let log = e.log.clone();
    let eff = e.func("eff", 0, vec![dn.clone()], move |rt, _, _, d| {
        log.borrow_mut().push(format!("d {}", js(rt, &rt.get_str(&d[0], "value")?)));
        Ok(undef())
    });
    e.invoke("effect", &[eff]);
    e.set(&a, "value", num(2.0));
    line(&e, format!("d-calls {}", d_calls.get()));
    check(&e, "computed-diamond-single-recompute");
}

/// `computed-unchanged-result-no-retrigger`: a dependent is not re-run when the computed's value is unchanged.
#[test]
fn computed_unchanged_result_no_retrigger() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(3.0)]);
    let getter = e.func("isEven", 0, vec![n.clone()], |rt, _, _, d| {
        let v = rt.to_number(&rt.get_str(&d[0], "value")?)?;
        Ok(Value::Bool(v % 2.0 == 0.0))
    });
    let is_even = e.invoke("computed", &[getter]);
    let runs = counter();
    let (r2, log) = (runs.clone(), e.log.clone());
    let body = e.func("body", 0, vec![is_even.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        log.borrow_mut().push(format!("even {}", js(rt, &rt.get_str(&d[0], "value")?)));
        Ok(undef())
    });
    e.invoke("effect", &[body]);
    e.set(&n, "value", num(5.0));
    e.set(&n, "value", num(6.0));
    line(&e, format!("runs {}", runs.get()));
    check(&e, "computed-unchanged-result-no-retrigger");
}

/// `computed-writable-roundtrip`: a writable computed forwards writes to its setter.
#[test]
fn computed_writable_roundtrip() {
    let e = Env::new();
    let first = e.invoke("ref", &[Env::text("Ada")]);
    let last = e.invoke("ref", &[Env::text("Lovelace")]);
    let get = e.func("get", 0, vec![first.clone(), last.clone()], |rt, _, _, d| {
        let f = js(rt, &rt.get_str(&d[0], "value")?);
        let l = js(rt, &rt.get_str(&d[1], "value")?);
        Ok(Env::text(&format!("{f} {l}")))
    });
    let set = e.func("set", 1, vec![first.clone(), last.clone()], |rt, _, args, d| {
        let text = js(rt, &args[0]);
        let mut parts = text.split(' ');
        let f = parts.next().unwrap_or("").to_string();
        let l = parts.next().unwrap_or("").to_string();
        rt.set_v_strict(&d[0], &PropKey::from_str("value"), Env::text(&f))?;
        rt.set_v_strict(&d[1], &PropKey::from_str("value"), Env::text(&l))?;
        Ok(undef())
    });
    let opts = e.rt.new_object();
    e.rt.define_data(opts, PropKey::from_str("get"), get, Flags::ALL).expect("get");
    e.rt.define_data(opts, PropKey::from_str("set"), set, Flags::ALL).expect("set");
    let full = e.invoke("computed", &[Value::Object(opts)]);
    line(&e, format!("full {}", js(&e.rt, &e.get(&full, "value"))));
    e.set(&full, "value", Env::text("Grace Hopper"));
    line(
        &e,
        format!(
            "first {} last {} full {}",
            js(&e.rt, &e.get(&first, "value")),
            js(&e.rt, &e.get(&last, "value")),
            js(&e.rt, &e.get(&full, "value"))
        ),
    );
    check(&e, "computed-writable-roundtrip");
}

/// `computed-readonly-set-ignored`: writing a computed without a setter does nothing (no throw in production).
#[test]
fn computed_readonly_set_ignored() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(1.0)]);
    let getter = e.func("getter", 0, vec![n.clone()], |rt, _, _, d| {
        Ok(num(rt.to_number(&rt.get_str(&d[0], "value")?)? + 1.0))
    });
    let c = e.invoke("computed", &[getter]);
    e.set(&c, "value", num(100.0));
    let ro = js(&e.rt, &e.get(&c, "__v_isReadonly"));
    line(&e, format!("value {} readonly {}", js(&e.rt, &e.get(&c, "value")), ro));
    check(&e, "computed-readonly-set-ignored");
}

/// `computed-oldvalue-param`: the getter receives the previous value (undefined on the first run).
#[test]
fn computed_oldvalue_param() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(1.0)]);
    let log = e.log.clone();
    let getter = e.func("getter", 1, vec![n.clone()], move |rt, _, args, d| {
        log.borrow_mut().push(format!("getter-old {}", js(rt, &args[0])));
        Ok(num(rt.to_number(&rt.get_str(&d[0], "value")?)? * 10.0))
    });
    let c = e.invoke("computed", &[getter]);
    let v1 = e.get(&c, "value");
    line(&e, format!("v1 {}", js(&e.rt, &v1)));
    e.set(&n, "value", num(2.0));
    let v2 = e.get(&c, "value");
    line(&e, format!("v2 {}", js(&e.rt, &v2)));
    check(&e, "computed-oldvalue-param");
}

/// `computed-branch-switching`: a computed only depends on what its last run read.
#[test]
fn computed_branch_switching() {
    let e = Env::new();
    let use_a = e.invoke("ref", &[Value::Bool(true)]);
    let a = e.invoke("ref", &[Env::text("A0")]);
    let b = e.invoke("ref", &[Env::text("B0")]);
    let calls = counter();
    let k = calls.clone();
    let getter = e.func("pick", 0, vec![use_a.clone(), a.clone(), b.clone()], move |rt, _, _, d| {
        k.set(k.get() + 1);
        let pick = if rt.get_str(&d[0], "value")?.to_boolean() { &d[1] } else { &d[2] };
        rt.get_str(pick, "value")
    });
    let pick = e.invoke("computed", &[getter]);
    let log = e.log.clone();
    let eff = e.func("eff", 0, vec![pick.clone()], move |rt, _, _, d| {
        log.borrow_mut().push(format!("pick {}", js(rt, &rt.get_str(&d[0], "value")?)));
        Ok(undef())
    });
    e.invoke("effect", &[eff]);
    e.set(&b, "value", Env::text("B1"));
    line(&e, format!("calls-1 {}", calls.get()));
    e.set(&use_a, "value", Value::Bool(false));
    line(&e, format!("calls-2 {}", calls.get()));
    e.set(&a, "value", Env::text("A1"));
    line(&e, format!("calls-3 {}", calls.get()));
    e.set(&b, "value", Env::text("B2"));
    line(&e, format!("calls-4 {}", calls.get()));
    check(&e, "computed-branch-switching");
}

/// `computed-error-then-recover`: a throwing getter rejects the read; a later write recovers it.
#[test]
fn computed_error_then_recover() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let getter = e.func("inv", 0, vec![n.clone()], |rt, _, _, d| {
        let v = rt.to_number(&rt.get_str(&d[0], "value")?)?;
        if v == 0.0 {
            return Err(rt.throw_range_error("zero"));
        }
        Ok(num(10.0 / v))
    });
    let inv = e.invoke("computed", &[getter]);
    let first = match e.rt.get_str(&inv, "value") {
        Ok(_) => "none".to_string(),
        Err(t) => js(&e.rt, &e.get(&t.0, "name")),
    };
    line(&e, format!("first {first}"));
    e.set(&n, "value", num(5.0));
    line(&e, format!("recovered {}", js(&e.rt, &e.get(&inv, "value"))));
    check(&e, "computed-error-then-recover");
}

/// `computed-self-read-previous-value`: a computed that reads itself gets its previous value, without recursion.
#[test]
fn computed_self_read_previous_value() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let calls = counter();
    let slot = common::holder(&e.rt);
    let k = calls.clone();
    let getter = e.func("acc", 0, vec![n.clone(), slot.clone()], move |rt, _, _, d| {
        k.set(k.get() + 1);
        rt.get_str(&d[0], "value")?;
        let acc = rt.get_str(&d[1], "acc")?;
        let prev = rt.get_str(&acc, "value")?;
        let base = if prev.to_boolean() { rt.to_number(&prev)? } else { 0.0 };
        Ok(num(base + 1.0))
    });
    let acc = e.invoke("computed", &[getter]);
    common::put(&e.rt, &slot, "acc", acc.clone());
    line(&e, format!("first {} {}", js(&e.rt, &e.get(&acc, "value")), calls.get()));
    e.set(&n, "value", num(1.0));
    line(&e, format!("second {} {}", js(&e.rt, &e.get(&acc, "value")), calls.get()));
    check(&e, "computed-self-read-previous-value");
}

/// `computed-no-deps-evaluated-once`: a computed without dependencies is evaluated once and cached.
#[test]
fn computed_no_deps_evaluated_once() {
    let e = Env::new();
    let calls = counter();
    let k = calls.clone();
    let getter = e.func("k", 0, vec![], move |_, _, _, _| {
        k.set(k.get() + 1);
        Ok(num(5.0))
    });
    let c = e.invoke("computed", &[getter]);
    let a = e.get(&c, "value");
    let b = e.get(&c, "value");
    line(&e, format!("a {} {} {}", js(&e.rt, &a), js(&e.rt, &b), calls.get()));
    check(&e, "computed-no-deps-evaluated-once");
}

/// `computed-new-object-retrigger`: a computed returning a new object each time retriggers its dependents.
#[test]
fn computed_new_object_retrigger() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let getter = e.func("obj", 0, vec![n.clone()], |rt, _, _, d| {
        let v = rt.to_number(&rt.get_str(&d[0], "value")?)?;
        let o = rt.new_object();
        rt.define_data(o, PropKey::from_str("parity"), num(v % 2.0), Flags::ALL)?;
        Ok(Value::Object(o))
    });
    let obj = e.invoke("computed", &[getter]);
    let runs = counter();
    let (r2, log) = (runs.clone(), e.log.clone());
    let body = e.func("body", 0, vec![obj.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        let v = rt.get_str(&d[0], "value")?;
        log.borrow_mut().push(format!("p {}", js(rt, &rt.get_str(&v, "parity")?)));
        Ok(undef())
    });
    e.invoke("effect", &[body]);
    e.set(&n, "value", num(2.0));
    line(&e, format!("runs {}", runs.get()));
    check(&e, "computed-new-object-retrigger");
}

/// `computed-throw-cache-after-error`: after a throw, the next read with no change returns the cached value silently.
#[test]
fn computed_throw_cache_after_error() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let calls = counter();
    let k = calls.clone();
    let getter = e.func("c", 0, vec![n.clone()], move |rt, _, _, d| {
        k.set(k.get() + 1);
        let v = rt.get_str(&d[0], "value")?;
        if rt.to_number(&v)? == 0.0 {
            return Err(rt.throw_range_error("z"));
        }
        Ok(v)
    });
    let c = e.invoke("computed", &[getter]);
    let mut results = Vec::new();
    for _ in 0..2 {
        match e.rt.get_str(&c, "value") {
            Ok(_) => results.push("ok".to_string()),
            Err(t) => results.push(js(&e.rt, &e.get(&t.0, "name"))),
        }
    }
    line(&e, format!("results {} {} {}", results[0], results[1], calls.get()));
    check(&e, "computed-throw-cache-after-error");
}

/// `computed-scope-stop-keeps-cache`: a computed created in a scope is not stopped with it; its cache survives.
#[test]
fn computed_scope_stop_keeps_cache() {
    let e = Env::new();
    let n = e.invoke("ref", &[num(0.0)]);
    let calls = counter();
    let k = calls.clone();
    let getter = e.func("g", 0, vec![n.clone()], move |rt, _, _, d| {
        k.set(k.get() + 1);
        Ok(num(rt.to_number(&rt.get_str(&d[0], "value")?)? + 1.0))
    });
    let scope = e.invoke("effectScope", &[]);
    let make = e.func("make", 0, vec![e.api("computed"), getter], |rt, _, _, d| rt.call(&d[0], &undef(), &[d[1].clone()]));
    let c = e.call(&e.get(&scope, "run"), &scope, &[make]);
    let a = e.get(&c, "value");
    line(&e, format!("a {} {}", js(&e.rt, &a), calls.get()));
    e.call(&e.get(&scope, "stop"), &scope, &[]);
    let b = e.get(&c, "value");
    line(&e, format!("b {} {}", js(&e.rt, &b), calls.get()));
    e.set(&n, "value", num(5.0));
    let cv = e.get(&c, "value");
    line(&e, format!("c {} {}", js(&e.rt, &cv), calls.get()));
    check(&e, "computed-scope-stop-keeps-cache");
}

//! Effects, computed values and effect scopes over proxies, method receivers, JSON, key order, readonly and shallow keys,
//! getters with receiver semantics and null-prototype objects. Each test is the Rust translation of one snippet in
//! tests/vue/rust/reactive/snippets.mjs (id in the doc comment).

#[macro_use]
mod r1b_support;
mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::{js, Env};
use r1b_support::{bo, check, flag, func, global, keys_joined, method, num, object, run_effect, st, sv, undef};
use vn_value::{strict_eq, Flags, PropKey, Value};

fn number_of(v: &Value) -> f64 {
    match v {
        Value::Number(n) => *n,
        _ => f64::NAN,
    }
}

/// `r1b-effect-stop-stops-proxy-tracking`
#[test]
fn effect_stop_stops_proxy_tracking() {
    let e = Env::new();
    let p = e.invoke("reactive", &[object(&e, &[("n", num(1.0))])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    let runner = run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let n = rt.get_str(&d[0], "n")?;
        log.borrow_mut().push(format!("n {}", js(rt, &n)));
        Ok(undef())
    });
    e.invoke("stop", &[runner]);
    e.set(&p, "n", num(2.0));
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&p, "n")));
    check(&e, "r1b-effect-stop-stops-proxy-tracking");
}

/// `r1b-computed-over-proxy-fields`
#[test]
fn computed_over_proxy_fields() {
    let e = Env::new();
    let p = e.invoke("reactive", &[object(&e, &[("a", num(1.0)), ("b", num(2.0))])]);
    let sum_getter = func(&e, "", 0, vec![p.clone()], |rt, _this, _args, d| {
        let a = number_of(&rt.get_str(&d[0], "a")?);
        let b = number_of(&rt.get_str(&d[0], "b")?);
        Ok(num(a + b))
    });
    let sum = e.invoke("computed", &[sum_getter]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    let sum_for_effect = sum.clone();
    run_effect(&e, vec![sum_for_effect], move |rt, d| {
        r2.set(r2.get() + 1);
        let v = rt.get_str(&d[0], "value")?;
        log.borrow_mut().push(format!("sum {}", js(rt, &v)));
        Ok(undef())
    });
    e.set(&p, "a", num(10.0));
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&sum, "value")));
    check(&e, "r1b-computed-over-proxy-fields");
}

/// `r1b-computed-cache-unrelated-key`: a computed value is not recomputed when a key it did not read changes.
#[test]
fn computed_cache_unrelated_key() {
    let e = Env::new();
    let p = e.invoke("reactive", &[object(&e, &[("a", num(1.0)), ("b", num(2.0))])]);
    let evals = Rc::new(Cell::new(0i64));
    let counter = evals.clone();
    let getter = func(&e, "", 0, vec![p.clone()], move |rt, _this, _args, d| {
        counter.set(counter.get() + 1);
        let a = number_of(&rt.get_str(&d[0], "a")?);
        Ok(num(a * 2.0))
    });
    let c = e.invoke("computed", &[getter]);
    log_line!(e, "first", sv(&e, &e.get(&c, "value")), evals.get());
    e.set(&p, "b", num(5.0));
    log_line!(e, "after-b", sv(&e, &e.get(&c, "value")), evals.get());
    e.set(&p, "a", num(3.0));
    log_line!(e, "after-a", sv(&e, &e.get(&c, "value")), evals.get());
    check(&e, "r1b-computed-cache-unrelated-key");
}

/// `r1b-effect-scope-disposes-proxy-effect`
#[test]
fn effect_scope_disposes_proxy_effect() {
    let e = Env::new();
    let p = e.invoke("reactive", &[object(&e, &[("n", num(1.0))])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    let body = func(&e, "", 0, vec![p.clone()], move |rt, _this, _args, d| {
        r2.set(r2.get() + 1);
        let n = rt.get_str(&d[0], "n")?;
        log.borrow_mut().push(format!("n {}", js(rt, &n)));
        Ok(undef())
    });
    let effect_fn = e.api("effect");
    let run_fn = func(&e, "", 0, vec![effect_fn, body], |rt, _this, _args, d| rt.call(&d[0], &Value::Undefined, &[d[1].clone()]));
    let scope = e.invoke("effectScope", &[]);
    method(&e, &scope, "run", &[run_fn]);
    e.set(&p, "n", num(2.0));
    method(&e, &scope, "stop", &[]);
    e.set(&p, "n", num(3.0));
    log_line!(e, "runs", runs.get());
    check(&e, "r1b-effect-scope-disposes-proxy-effect");
}

/// `r1b-method-this-is-proxy`: a method called through the proxy receives the proxy as `this`.
#[test]
fn method_this_is_proxy() {
    let e = Env::new();
    let raw = object(&e, &[("n", num(1.0))]);
    let self_fn = func(&e, "self", 0, vec![], |_rt, this, _args, _d| Ok(this.clone()));
    e.rt.define_data(raw.as_object().expect("object"), PropKey::from_str("self"), self_fn, Flags::ALL).expect("method");
    let p = e.invoke("reactive", &[raw]);
    let returned = method(&e, &p, "self", &[]);
    log_line!(e, "method-this", bo(strict_eq(&returned, &p)));
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let me = rt.call(&rt.get_str(&d[0], "self")?, &d[0], &[])?;
        let n = rt.get_str(&me, "n")?;
        log.borrow_mut().push(format!("via-method {}", js(rt, &n)));
        Ok(undef())
    });
    e.set(&p, "n", num(2.0));
    log_line!(e, "runs", runs.get());
    check(&e, "r1b-method-this-is-proxy");
}

/// `r1b-json-stringify-proxy`
#[test]
fn json_stringify_proxy() {
    let e = Env::new();
    let inner = object(&e, &[("d", st("x"))]);
    let list = e.rt.new_array(vec![num(2.0), num(3.0)]);
    let raw = object(&e, &[("a", num(1.0)), ("b", Value::Object(list)), ("c", inner)]);
    let p = e.invoke("reactive", &[raw]);
    let text = method(&e, &global(&e, "JSON"), "stringify", &[p]);
    log_line!(e, "json", sv(&e, &text));
    check(&e, "r1b-json-stringify-proxy");
}

/// `r1b-keys-order-after-delete-readd`
#[test]
fn keys_order_after_delete_readd() {
    let e = Env::new();
    let p = e.invoke("reactive", &[object(&e, &[("a", num(1.0)), ("b", num(2.0))])]);
    e.rt.delete(p.as_object().expect("proxy"), &PropKey::from_str("a")).expect("delete");
    e.set(&p, "a", num(3.0));
    log_line!(e, "keys", keys_joined(&e, &p, ","));
    check(&e, "r1b-keys-order-after-delete-readd");
}

/// `r1b-readonly-keys-and-shallow-readonly-set`: a shallow readonly write is silently refused.
#[test]
fn readonly_keys_and_shallow_readonly_set() {
    let e = Env::new();
    let r = e.invoke("readonly", &[object(&e, &[("a", num(1.0)), ("b", num(2.0))])]);
    let sr = e.invoke("shallowReadonly", &[object(&e, &[("a", num(1.0))])]);
    e.set(&sr, "a", num(2.0));
    log_line!(e, "ro-keys", keys_joined(&e, &r, ","), sv(&e, &e.get(&sr, "a")));
    check(&e, "r1b-readonly-keys-and-shallow-readonly-set");
}

/// `r1b-class-instance-getter-receiver`: a getter reads `this.x` through the proxy it was called on.
#[test]
fn class_instance_getter_receiver() {
    let e = Env::new();
    let raw = object(&e, &[("x", num(1.0))]);
    let dbl = func(&e, "get dbl", 0, vec![], |rt, this, _args, _d| {
        let x = number_of(&rt.get_str(this, "x")?);
        Ok(num(x * 2.0))
    });
    e.rt.define_accessor(raw.as_object().expect("object"), PropKey::from_str("dbl"), Some(dbl.as_object().expect("fn")), None, Flags::ALL)
        .expect("getter");
    let p = e.invoke("reactive", &[raw]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let v = rt.get_str(&d[0], "dbl")?;
        log.borrow_mut().push(format!("dbl {}", js(rt, &v)));
        Ok(undef())
    });
    e.set(&p, "x", num(5.0));
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&p, "dbl")));
    check(&e, "r1b-class-instance-getter-receiver");
}

/// `r1b-two-proxies-independent-tracking`
#[test]
fn two_proxies_independent_tracking() {
    let e = Env::new();
    let a = e.invoke("reactive", &[object(&e, &[("v", num(1.0))])]);
    let b = e.invoke("reactive", &[object(&e, &[("v", num(1.0))])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![a.clone(), b.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let av = number_of(&rt.get_str(&d[0], "v")?);
        let bv = number_of(&rt.get_str(&d[1], "v")?);
        log.borrow_mut().push(format!("sum {}", av + bv));
        Ok(undef())
    });
    e.set(&b, "v", num(5.0));
    log_line!(e, "runs", runs.get());
    e.set(&a, "v", num(2.0));
    log_line!(e, "runs2", runs.get());
    check(&e, "r1b-two-proxies-independent-tracking");
}

/// `r1b-null-prototype-object-proxied`
#[test]
fn null_prototype_object_proxied() {
    let e = Env::new();
    let raw = method(&e, &global(&e, "Object"), "create", &[Value::Null]);
    let p = e.invoke("reactive", &[raw]);
    e.set(&p, "x", num(1.0));
    let proto = method(&e, &global(&e, "Object"), "getPrototypeOf", &[p.clone()]);
    log_line!(e, "null-proto", flag(&e, "isReactive", &p), sv(&e, &e.get(&p, "x")), bo(strict_eq(&proto, &Value::Null)));
    check(&e, "r1b-null-prototype-object-proxied");
}

/// `r1b-proxy-function-property-not-proxied`: functions are never wrapped, so the property is the function itself.
#[test]
fn proxy_function_property_not_proxied() {
    let e = Env::new();
    let fn_value = func(&e, "", 0, vec![], |_rt, _this, _args, _d| Ok(undef()));
    let p = e.invoke("reactive", &[object(&e, &[("fn", fn_value.clone())])]);
    let got = e.get(&p, "fn");
    log_line!(e, "fn-prop", bo(strict_eq(&got, &fn_value)), flag(&e, "isReactive", &got), "function");
    assert!(e.rt.is_callable(&got), "a function property stays callable");
    check(&e, "r1b-proxy-function-property-not-proxied");
}

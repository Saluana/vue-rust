//! Refs inside reactive objects and reactive values inside refs: unwrapping rules, identity, write-through, toRef/toRefs over
//! proxies and readonly values. Each test is the Rust translation of one snippet in tests/vue/rust/reactive/snippets.mjs
//! (id in the doc comment).

#[macro_use]
mod r1b_support;
mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::{js, Env};
use r1b_support::{array, bo, check, flag, method, new_map, num, object, run_effect, st, sv, undef};
use vn_value::strict_eq;

/// `r1b-ref-value-is-reactive-object`
#[test]
fn ref_value_is_reactive_object() {
    let e = Env::new();
    let obj = object(&e, &[("n", num(1.0))]);
    let r = e.invoke("ref", &[obj.clone()]);
    let value = e.get(&r, "value");
    let raw = e.invoke("toRaw", &[e.get(&r, "value")]);
    log_line!(e, "ref-obj", flag(&e, "isReactive", &value), bo(strict_eq(&raw, &obj)), bo(strict_eq(&e.get(&r, "value"), &e.get(&r, "value"))));
    check(&e, "r1b-ref-value-is-reactive-object");
}

/// `r1b-shallowref-value-raw`
#[test]
fn shallowref_value_raw() {
    let e = Env::new();
    let obj = object(&e, &[("n", num(1.0))]);
    let r = e.invoke("shallowRef", &[obj.clone()]);
    let value = e.get(&r, "value");
    log_line!(e, "shallow-ref", bo(strict_eq(&value, &obj)), flag(&e, "isReactive", &value));
    check(&e, "r1b-shallowref-value-raw");
}

/// `r1b-ref-assign-same-raw-no-trigger`: assigning the proxy of the current raw value is not a change.
#[test]
fn ref_assign_same_raw_no_trigger() {
    let e = Env::new();
    let obj = object(&e, &[("n", num(1.0))]);
    let r = e.invoke("ref", &[obj.clone()]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![r.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let value = rt.get_str(&d[0], "value")?;
        let n = rt.get_str(&value, "n")?;
        log.borrow_mut().push(format!("n {}", js(rt, &n)));
        Ok(undef())
    });
    let same = e.invoke("reactive", &[obj.clone()]);
    e.set(&r, "value", same);
    log_line!(e, "runs-same", runs.get());
    e.set(&r, "value", object(&e, &[("n", num(2.0))]));
    log_line!(e, "runs-new", runs.get());
    check(&e, "r1b-ref-assign-same-raw-no-trigger");
}

/// `r1b-reactive-unwraps-nested-ref-writes`: a ref inside a nested reactive object is unwrapped on read and written through.
#[test]
fn reactive_unwraps_nested_ref_writes() {
    let e = Env::new();
    let count = e.invoke("ref", &[num(1.0)]);
    let nested = object(&e, &[("count", count.clone())]);
    let p = e.invoke("reactive", &[object(&e, &[("nested", nested)])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let nested = rt.get_str(&d[0], "nested")?;
        let c = rt.get_str(&nested, "count")?;
        log.borrow_mut().push(format!("count {}", js(rt, &c)));
        Ok(undef())
    });
    e.set(&count, "value", num(2.0));
    log_line!(e, "runs", runs.get());
    let nested_now = e.get(&p, "nested");
    e.set(&nested_now, "count", num(3.0));
    log_line!(e, "runs2", runs.get(), sv(&e, &e.get(&count, "value")));
    check(&e, "r1b-reactive-unwraps-nested-ref-writes");
}

/// `r1b-ref-in-array-not-unwrapped`: integer keys of arrays return the ref itself; writing an index replaces the ref.
#[test]
fn ref_in_array_not_unwrapped() {
    let e = Env::new();
    let r = e.invoke("ref", &[num(7.0)]);
    let arr = e.invoke("reactive", &[array(&e, vec![r.clone()])]);
    let first = e.get(&arr, "0");
    log_line!(e, "arr-ref", flag(&e, "isRef", &first), bo(strict_eq(&first, &r)), sv(&e, &e.get(&first, "value")));
    e.set(&arr, "0", num(8.0));
    let after = e.get(&arr, "0");
    log_line!(e, "arr-set", flag(&e, "isRef", &after), sv(&e, &after));
    check(&e, "r1b-ref-in-array-not-unwrapped");
}

/// `r1b-ref-in-map-value-wrapped`: a Map returns its values wrapped, so a ref stored in a Map comes back as a proxy of it.
#[test]
fn ref_in_map_value_wrapped() {
    let e = Env::new();
    let r = e.invoke("ref", &[st("v")]);
    let m = e.invoke("reactive", &[new_map(&e, &[(st("k"), r.clone())])]);
    let got = method(&e, &m, "get", &[st("k")]);
    log_line!(e, "map-ref", flag(&e, "isRef", &got), bo(strict_eq(&got, &r)));
    check(&e, "r1b-ref-in-map-value-wrapped");
}

/// `r1b-ref-of-proxy-reads-through`
#[test]
fn ref_of_proxy_reads_through() {
    let e = Env::new();
    let p = e.invoke("reactive", &[object(&e, &[("n", num(1.0))])]);
    let r = e.invoke("ref", &[p.clone()]);
    let value = e.get(&r, "value");
    log_line!(e, "ref-of-proxy", bo(strict_eq(&value, &p)), flag(&e, "isReactive", &value));
    check(&e, "r1b-ref-of-proxy-reads-through");
}

/// `r1b-torefs-over-reactive`: `toRefs` of a reactive object reads and writes through the proxy.
#[test]
fn torefs_over_reactive() {
    let e = Env::new();
    let p = e.invoke("reactive", &[object(&e, &[("a", num(1.0)), ("b", num(2.0))])]);
    let refs = e.invoke("toRefs", &[p.clone()]);
    let a = e.get(&refs, "a");
    let b = e.get(&refs, "b");
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![a.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let v = rt.get_str(&d[0], "value")?;
        log.borrow_mut().push(format!("a {}", js(rt, &v)));
        Ok(undef())
    });
    e.set(&p, "a", num(5.0));
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&b, "value")));
    e.set(&a, "value", num(7.0));
    log_line!(e, "runs2", runs.get(), sv(&e, &e.get(&p, "a")));
    check(&e, "r1b-torefs-over-reactive");
}

/// `r1b-toref-over-readonly`
#[test]
fn toref_over_readonly() {
    let e = Env::new();
    let r = e.invoke("readonly", &[object(&e, &[("n", num(1.0))])]);
    let tr = e.invoke("toRef", &[r.clone(), st("n")]);
    log_line!(e, "toref-ro", sv(&e, &e.get(&tr, "value")));
    check(&e, "r1b-toref-over-readonly");
}

/// `r1b-ref-deep-object-value-tracking`: the value of a ref is reactive, so its fields are tracked.
#[test]
fn ref_deep_object_value_tracking() {
    let e = Env::new();
    let r = e.invoke("ref", &[object(&e, &[("n", num(1.0))])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![r.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let value = rt.get_str(&d[0], "value")?;
        let n = rt.get_str(&value, "n")?;
        log.borrow_mut().push(format!("deep {}", js(rt, &n)));
        Ok(undef())
    });
    let value = e.get(&r, "value");
    e.set(&value, "n", num(2.0));
    log_line!(e, "runs", runs.get());
    e.set(&r, "value", object(&e, &[("n", num(3.0))]));
    log_line!(e, "runs2", runs.get());
    check(&e, "r1b-ref-deep-object-value-tracking");
}

/// `r1b-unref-toValue-on-reactive`
#[test]
fn unref_tovalue_on_reactive() {
    let e = Env::new();
    let r = e.invoke("ref", &[num(4.0)]);
    let p = e.invoke("reactive", &[object(&e, &[("r", r.clone())])]);
    let field = e.get(&p, "r");
    let unwrapped = e.invoke("unref", &[field.clone()]);
    let value = e.invoke("toValue", &[field.clone()]);
    log_line!(e, "unref", sv(&e, &unwrapped), sv(&e, &value), flag(&e, "isRef", &field), sv(&e, &e.invoke("unref", &[p.clone()])));
    check(&e, "r1b-unref-toValue-on-reactive");
}

/// `r1b-ref-deep-array-push-tracking`
#[test]
fn ref_deep_array_push_tracking() {
    let e = Env::new();
    let r = e.invoke("ref", &[array(&e, vec![num(1.0), num(2.0)])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![r.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let value = rt.get_str(&d[0], "value")?;
        let len = rt.get_str(&value, "length")?;
        log.borrow_mut().push(format!("len {}", js(rt, &len)));
        Ok(undef())
    });
    let value = e.get(&r, "value");
    method(&e, &value, "push", &[num(3.0)]);
    log_line!(e, "runs", runs.get(), flag(&e, "isReactive", &e.get(&r, "value")));
    check(&e, "r1b-ref-deep-array-push-tracking");
}

/// `r1b-ref-of-readonly-value`: a readonly value stays readonly when it is stored in a ref.
#[test]
fn ref_of_readonly_value() {
    let e = Env::new();
    let ro = e.invoke("readonly", &[object(&e, &[("n", num(1.0))])]);
    let r = e.invoke("ref", &[ro]);
    let value = e.get(&r, "value");
    log_line!(e, "ref-ro", flag(&e, "isReadonly", &value), flag(&e, "isReactive", &value));
    check(&e, "r1b-ref-of-readonly-value");
}

/// `r1b-shallowref-in-reactive-not-rewrapped`: a shallow ref inside a reactive object unwraps to the raw value, unwrapped.
#[test]
fn shallowref_in_reactive_not_rewrapped() {
    let e = Env::new();
    let sr = e.invoke("shallowRef", &[object(&e, &[("n", num(1.0))])]);
    let p = e.invoke("reactive", &[object(&e, &[("sr", sr.clone())])]);
    let got = e.get(&p, "sr");
    log_line!(e, "sr-unwrapped", flag(&e, "isRef", &got), sv(&e, &e.get(&got, "n")), bo(strict_eq(&got, &e.get(&sr, "value"))));
    check(&e, "r1b-shallowref-in-reactive-not-rewrapped");
}

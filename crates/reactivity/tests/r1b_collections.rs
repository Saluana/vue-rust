//! Map and Set proxies: flags and identity, instrumented methods with effects, iterators and forEach wrapping, readonly and
//! shallow collections, and mutation during iteration. Each test is the Rust translation of one snippet in
//! tests/vue/rust/reactive/snippets.mjs (id in the doc comment).

#[macro_use]
mod r1b_support;
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use common::{js, Env};
use r1b_support::{
    array, bo, check, flag, func, global, iterate, iterate_rt, method, new_map, new_set, num, object, run_effect, st, sv,
    tag_of, undef,
};
use vn_value::{strict_eq, Value};

/// `r1b-map-flags-and-identity`
#[test]
fn map_flags_and_identity() {
    let e = Env::new();
    let raw = new_map(&e, &[]);
    let p = e.invoke("reactive", &[raw.clone()]);
    let p_again = e.invoke("reactive", &[raw.clone()]);
    let p_of_p = e.invoke("reactive", &[p.clone()]);
    let map_proto = e.get(&global(&e, "Map"), "prototype");
    let is_map = e.rt.get_prototype_of(p.as_object().expect("proxy")).map(Value::Object) == Some(map_proto.clone());
    log_line!(e, "same", bo(strict_eq(&p, &p_again)), bo(strict_eq(&p, &raw)), bo(strict_eq(&p_of_p, &p)), bo(is_map));
    let p_raw = e.invoke("toRaw", &[p.clone()]);
    log_line!(e, "flags", flag(&e, "isReactive", &p), flag(&e, "isReadonly", &p), flag(&e, "isShallow", &p), flag(&e, "isProxy", &p), bo(strict_eq(&p_raw, &raw)));
    let r = e.invoke("readonly", &[raw.clone()]);
    let r_again = e.invoke("readonly", &[raw.clone()]);
    let r_of_r = e.invoke("reactive", &[r.clone()]);
    log_line!(e, "readonly", flag(&e, "isReactive", &r), flag(&e, "isReadonly", &r), flag(&e, "isShallow", &r), bo(strict_eq(&r_again, &r)), bo(strict_eq(&r_of_r, &r)));
    let s = e.invoke("shallowReactive", &[raw.clone()]);
    let s_raw = e.invoke("toRaw", &[s.clone()]);
    log_line!(e, "shallow", flag(&e, "isReactive", &s), flag(&e, "isShallow", &s), bo(strict_eq(&s, &p)), bo(strict_eq(&s_raw, &raw)));
    let ts = tag_of(&e, &p);
    log_line!(e, "proto", bo(e.rt.get_prototype_of(p.as_object().expect("proxy")).map(Value::Object) == Some(map_proto)), ts);
    check(&e, "r1b-map-flags-and-identity");
}

/// `r1b-map-get-set-has-size-effects`
#[test]
fn map_get_set_has_size_effects() {
    let e = Env::new();
    let m = e.invoke("reactive", &[new_map(&e, &[])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![m.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let size = rt.get_str(&d[0], "size")?;
        let has = rt.call(&rt.get_str(&d[0], "has")?, &d[0], &[st("a")])?;
        let get = rt.call(&rt.get_str(&d[0], "get")?, &d[0], &[st("a")])?;
        log.borrow_mut().push(format!("size {} {} {}", js(rt, &size), js(rt, &has), js(rt, &get)));
        Ok(undef())
    });
    method(&e, &m, "set", &[st("a"), num(1.0)]);
    log_line!(e, "runs", runs.get());
    method(&e, &m, "set", &[st("a"), num(1.0)]);
    log_line!(e, "runs-same", runs.get());
    method(&e, &m, "set", &[st("a"), num(2.0)]);
    log_line!(e, "runs-change", runs.get());
    method(&e, &m, "delete", &[st("a")]);
    log_line!(e, "runs-delete", runs.get());
    method(&e, &m, "delete", &[st("missing")]);
    log_line!(e, "runs-missing", runs.get());
    method(&e, &m, "clear", &[]);
    log_line!(e, "runs-clear-empty", runs.get());
    check(&e, "r1b-map-get-set-has-size-effects");
}

/// `r1b-map-clear-triggers`
#[test]
fn map_clear_triggers() {
    let e = Env::new();
    let m = e.invoke("reactive", &[new_map(&e, &[(st("x"), num(1.0)), (st("y"), num(2.0))])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![m.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let size = rt.get_str(&d[0], "size")?;
        log.borrow_mut().push(format!("size {}", js(rt, &size)));
        Ok(undef())
    });
    method(&e, &m, "clear", &[]);
    log_line!(e, "after-clear", runs.get(), sv(&e, &e.get(&m, "size")));
    check(&e, "r1b-map-clear-triggers");
}

/// `r1b-set-add-delete-has-tracking`
#[test]
fn set_add_delete_has_tracking() {
    let e = Env::new();
    let s = e.invoke("reactive", &[new_set(&e, &[num(1.0)])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![s.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let has = rt.call(&rt.get_str(&d[0], "has")?, &d[0], &[num(2.0)])?;
        let size = rt.get_str(&d[0], "size")?;
        log.borrow_mut().push(format!("has2 {} {}", js(rt, &has), js(rt, &size)));
        Ok(undef())
    });
    let add_ret = method(&e, &s, "add", &[num(2.0)]);
    log_line!(e, "add-ret", bo(strict_eq(&add_ret, &s)), runs.get());
    let add_dup = method(&e, &s, "add", &[num(2.0)]);
    log_line!(e, "add-dup", bo(strict_eq(&add_dup, &s)), runs.get());
    method(&e, &s, "delete", &[num(1.0)]);
    let delete_ret = method(&e, &s, "delete", &[num(9.0)]);
    log_line!(e, "delete-ret", bo(delete_ret.to_boolean()), runs.get());
    let has1 = method(&e, &s, "has", &[num(1.0)]);
    log_line!(e, "final", sv(&e, &e.get(&s, "size")), bo(has1.to_boolean()), runs.get());
    check(&e, "r1b-set-add-delete-has-tracking");
}

/// `r1b-map-foreach-wraps-values`
#[test]
fn map_foreach_wraps_values() {
    let e = Env::new();
    let inner = object(&e, &[("v", num(1.0))]);
    let m = e.invoke("reactive", &[new_map(&e, &[(st("k"), inner.clone())])]);
    let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    let cb = func(&e, "", 3, vec![m.clone()], move |rt, _this, args, d| {
        let v_reactive = rt.get_str(&args[0], "__v_isReactive")?.to_boolean();
        sink.borrow_mut().push(v_reactive.to_string());
        sink.borrow_mut().push(js(rt, &args[1]));
        sink.borrow_mut().push(strict_eq(&args[2], &d[0]).to_string());
        Ok(undef())
    });
    method(&e, &m, "forEach", &[cb]);
    log_line!(e, "forEach", seen.borrow().join(","));
    let got = method(&e, &m, "get", &[st("k")]);
    log_line!(e, "get-wrapped", flag(&e, "isReactive", &got), bo(strict_eq(&e.invoke("toRaw", &[got.clone()]), &inner)));
    check(&e, "r1b-map-foreach-wraps-values");
}

/// `r1b-map-iterators-wrap-entries`
#[test]
fn map_iterators_wrap_entries() {
    let e = Env::new();
    let first = object(&e, &[("n", num(1.0))]);
    let m = e.invoke("reactive", &[new_map(&e, &[(st("a"), first), (st("b"), num(2.0))])]);
    let mut out = Vec::new();
    for pair in iterate(&e, &m) {
        let k = e.get(&pair, "0");
        let v = e.get(&pair, "1");
        out.push(sv(&e, &k));
        out.push(flag(&e, "isReactive", &v));
    }
    log_line!(e, "iter", out.join(","));
    let keys = method(&e, &m, "keys", &[]);
    let key_list: Vec<String> = iterate(&e, &keys).iter().map(|k| sv(&e, k)).collect();
    log_line!(e, "keys", key_list.join(","));
    let values = method(&e, &m, "values", &[]);
    let value_flags: Vec<String> = iterate(&e, &values).iter().map(|v| flag(&e, "isReactive", v)).collect();
    log_line!(e, "values", value_flags.join(","));
    let entries = method(&e, &m, "entries", &[]);
    log_line!(e, "entries", iterate(&e, &entries).len());
    check(&e, "r1b-map-iterators-wrap-entries");
}

/// `r1b-map-keys-tracking-iterate-key`
#[test]
fn map_keys_tracking_iterate_key() {
    let e = Env::new();
    let m = e.invoke("reactive", &[new_map(&e, &[(st("a"), num(1.0))])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![m.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let keys = rt.call(&rt.get_str(&d[0], "keys")?, &d[0], &[])?;
        let list = iterate_rt(rt, &keys)?;
        let parts: Vec<String> = list.iter().map(|k| js(rt, k)).collect();
        log.borrow_mut().push(format!("keys {}", parts.join("|")));
        Ok(undef())
    });
    method(&e, &m, "set", &[st("a"), num(5.0)]);
    log_line!(e, "runs-value-change", runs.get());
    method(&e, &m, "set", &[st("b"), num(2.0)]);
    log_line!(e, "runs-add", runs.get());
    check(&e, "r1b-map-keys-tracking-iterate-key");
}

/// `r1b-map-values-track-on-value-change`
#[test]
fn map_values_track_on_value_change() {
    let e = Env::new();
    let m = e.invoke("reactive", &[new_map(&e, &[(st("a"), num(1.0))])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![m.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let values = rt.call(&rt.get_str(&d[0], "values")?, &d[0], &[])?;
        let parts: Vec<String> = iterate_rt(rt, &values)?.iter().map(|v| js(rt, v)).collect();
        log.borrow_mut().push(format!("values {}", parts.join("|")));
        Ok(undef())
    });
    method(&e, &m, "set", &[st("a"), num(5.0)]);
    log_line!(e, "runs", runs.get());
    check(&e, "r1b-map-values-track-on-value-change");
}

/// `r1b-map-object-keys-raw-and-proxy`
#[test]
fn map_object_keys_raw_and_proxy() {
    let e = Env::new();
    let k = object(&e, &[]);
    let m = e.invoke("reactive", &[new_map(&e, &[])]);
    method(&e, &m, "set", &[k.clone(), st("raw")]);
    let kp = e.invoke("reactive", &[k.clone()]);
    let got_raw = method(&e, &m, "get", &[k.clone()]);
    let got_proxy = method(&e, &m, "get", &[kp.clone()]);
    let has_proxy = method(&e, &m, "has", &[kp.clone()]);
    let has_raw = method(&e, &m, "has", &[k.clone()]);
    log_line!(e, "get-raw", sv(&e, &got_raw), sv(&e, &got_proxy), bo(has_proxy.to_boolean()), bo(has_raw.to_boolean()));
    let raw = e.invoke("toRaw", &[m.clone()]);
    let stored_raw = method(&e, &raw, "has", &[k.clone()]);
    let stored_proxy = method(&e, &raw, "has", &[kp.clone()]);
    log_line!(e, "stored-raw-key", bo(stored_raw.to_boolean()), bo(stored_proxy.to_boolean()));
    check(&e, "r1b-map-object-keys-raw-and-proxy");
}

/// `r1b-map-set-stores-raw-value`
#[test]
fn map_set_stores_raw_value() {
    let e = Env::new();
    let v = object(&e, &[("n", num(1.0))]);
    let m = e.invoke("reactive", &[new_map(&e, &[])]);
    let proxied = e.invoke("reactive", &[v.clone()]);
    method(&e, &m, "set", &[st("p"), proxied]);
    let raw = e.invoke("toRaw", &[m.clone()]);
    let stored = method(&e, &raw, "get", &[st("p")]);
    let got = method(&e, &m, "get", &[st("p")]);
    log_line!(e, "stored", bo(strict_eq(&stored, &v)), flag(&e, "isReactive", &got), sv(&e, &e.get(&got, "n")));
    check(&e, "r1b-map-set-stores-raw-value");
}

/// `r1b-map-readonly-methods`
#[test]
fn map_readonly_methods() {
    let e = Env::new();
    let inner = object(&e, &[("x", num(1.0))]);
    let raw = new_map(&e, &[(st("a"), inner)]);
    let r = e.invoke("readonly", &[raw.clone()]);
    let set_ret = method(&e, &r, "set", &[st("b"), num(2.0)]);
    let raw_has_b = method(&e, &raw, "has", &[st("b")]);
    log_line!(e, "ro-set", bo(strict_eq(&set_ret, &r)), bo(raw_has_b.to_boolean()), sv(&e, &e.get(&r, "size")));
    let del = method(&e, &r, "delete", &[st("a")]);
    let has_a = method(&e, &r, "has", &[st("a")]);
    log_line!(e, "ro-delete", bo(del.to_boolean()), bo(has_a.to_boolean()), sv(&e, &e.get(&raw, "size")));
    method(&e, &r, "clear", &[]);
    log_line!(e, "ro-clear", sv(&e, &e.get(&raw, "size")));
    let got = method(&e, &r, "get", &[st("a")]);
    log_line!(e, "ro-get", flag(&e, "isReadonly", &got), sv(&e, &e.get(&got, "x")));
    check(&e, "r1b-map-readonly-methods");
}

/// `r1b-shallow-map-values-not-wrapped`
#[test]
fn shallow_map_values_not_wrapped() {
    let e = Env::new();
    let inner = object(&e, &[("n", num(1.0))]);
    let m = e.invoke("shallowReactive", &[new_map(&e, &[(st("k"), inner.clone())])]);
    let got = method(&e, &m, "get", &[st("k")]);
    log_line!(e, "shallow-get", bo(strict_eq(&got, &inner)), flag(&e, "isReactive", &got));
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![m.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let size = rt.get_str(&d[0], "size")?;
        log.borrow_mut().push(format!("size {}", js(rt, &size)));
        Ok(undef())
    });
    method(&e, &m, "set", &[st("j"), object(&e, &[])]);
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&m, "size")));
    check(&e, "r1b-shallow-map-values-not-wrapped");
}

/// `r1b-readonly-map-values-wrapped`
#[test]
fn readonly_map_values_wrapped() {
    let e = Env::new();
    let inner = object(&e, &[("n", num(1.0))]);
    let raw = new_map(&e, &[(st("k"), inner)]);
    let reactive_map = e.invoke("reactive", &[raw.clone()]);
    let r = e.invoke("readonly", &[reactive_map.clone()]);
    let v = method(&e, &r, "get", &[st("k")]);
    log_line!(e, "ro-of-reactive", flag(&e, "isReadonly", &v), flag(&e, "isReactive", &v), sv(&e, &e.get(&v, "n")));
    let first = iterate(&e, &method(&e, &r, "values", &[]))[0].clone();
    log_line!(e, "ro-values", flag(&e, "isReadonly", &first), flag(&e, "isReactive", &first));
    check(&e, "r1b-readonly-map-values-wrapped");
}

/// `r1b-set-foreach-and-iteration`
#[test]
fn set_foreach_and_iteration() {
    let e = Env::new();
    let inner = object(&e, &[("a", num(1.0))]);
    let s = e.invoke("reactive", &[new_set(&e, &[inner, num(2.0)])]);
    let out: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = out.clone();
    let cb = func(&e, "", 3, vec![s.clone()], move |rt, _this, args, d| {
        sink.borrow_mut().push(rt.get_str(&args[0], "__v_isReactive")?.to_boolean().to_string());
        sink.borrow_mut().push(strict_eq(&args[1], &args[0]).to_string());
        sink.borrow_mut().push(strict_eq(&args[2], &d[0]).to_string());
        Ok(undef())
    });
    method(&e, &s, "forEach", &[cb]);
    log_line!(e, "foreach", out.borrow().join(","));
    let flags: Vec<String> = iterate(&e, &s).iter().map(|v| flag(&e, "isReactive", v)).collect();
    log_line!(e, "iter", flags.join(","));
    let keys = iterate(&e, &method(&e, &s, "keys", &[])).len();
    let entries_first = iterate(&e, &method(&e, &s, "entries", &[]))[0].clone();
    let values_first = iterate(&e, &method(&e, &s, "values", &[]))[0].clone();
    let same = strict_eq(&e.get(&entries_first, "0"), &values_first);
    log_line!(e, "keys-same", keys, bo(same));
    check(&e, "r1b-set-foreach-and-iteration");
}

/// `r1b-set-tracks-iteration-on-add`
#[test]
fn set_tracks_iteration_on_add() {
    let e = Env::new();
    let s = e.invoke("reactive", &[new_set(&e, &[])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![s.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let n = iterate_rt(rt, &d[0])?.len();
        log.borrow_mut().push(format!("count {n}"));
        Ok(undef())
    });
    method(&e, &s, "add", &[num(1.0)]);
    method(&e, &s, "add", &[num(1.0)]);
    method(&e, &s, "delete", &[num(1.0)]);
    log_line!(e, "runs", runs.get());
    check(&e, "r1b-set-tracks-iteration-on-add");
}

/// `r1b-collection-mutate-during-iteration`: the live iterators see the entries added while they run.
#[test]
fn collection_mutate_during_iteration() {
    let e = Env::new();
    let m = e.invoke("reactive", &[new_map(&e, &[(st("a"), num(1.0))])]);
    let mut seen = Vec::new();
    let iter_fn = e.get(&m, "entries");
    let iter = e.call(&iter_fn, &m, &[]);
    let next = e.get(&iter, "next");
    loop {
        let r = e.call(&next, &iter, &[]);
        if e.get(&r, "done").to_boolean() {
            break;
        }
        let k = sv(&e, &e.get(&e.get(&r, "value"), "0"));
        if k == "a" {
            method(&e, &m, "set", &[st("b"), num(2.0)]);
        }
        seen.push(k);
    }
    log_line!(e, "seen", seen.join(","), sv(&e, &e.get(&m, "size")));
    let s = e.invoke("reactive", &[new_set(&e, &[num(1.0)])]);
    let mut sv_list = Vec::new();
    let s_iter = e.call(&e.get(&s, "values"), &s, &[]);
    let s_next = e.get(&s_iter, "next");
    loop {
        let r = e.call(&s_next, &s_iter, &[]);
        if e.get(&r, "done").to_boolean() {
            break;
        }
        let v = e.get(&r, "value");
        sv_list.push(sv(&e, &v));
        if e.rt.to_number(&v).expect("number") < 3.0 {
            method(&e, &s, "add", &[num(e.rt.to_number(&v).expect("number") + 1.0)]);
        }
    }
    log_line!(e, "set-seen", sv_list.join(","));
    check(&e, "r1b-collection-mutate-during-iteration");
}

/// `r1b-map-chainable-and-size`
#[test]
fn map_chainable_and_size() {
    let e = Env::new();
    let m = e.invoke("reactive", &[new_map(&e, &[])]);
    let first = method(&e, &m, "set", &[st("a"), num(1.0)]);
    let chained = method(&e, &first, "set", &[st("b"), num(2.0)]);
    log_line!(e, "chain", bo(strict_eq(&chained, &m)), sv(&e, &e.get(&m, "size")));
    check(&e, "r1b-map-chainable-and-size");
}

/// `r1b-map-nan-zero-keys`
#[test]
fn map_nan_zero_keys() {
    let e = Env::new();
    let m = e.invoke("reactive", &[new_map(&e, &[])]);
    method(&e, &m, "set", &[num(f64::NAN), st("n")]);
    method(&e, &m, "set", &[num(-0.0), st("z")]);
    let got_nan = method(&e, &m, "get", &[num(f64::NAN)]);
    let has_nan = method(&e, &m, "has", &[num(f64::NAN)]);
    let got_zero = method(&e, &m, "get", &[num(0.0)]);
    let has_neg_zero = method(&e, &m, "has", &[num(-0.0)]);
    log_line!(e, "nan", sv(&e, &got_nan), bo(has_nan.to_boolean()), sv(&e, &got_zero), bo(has_neg_zero.to_boolean()), sv(&e, &e.get(&m, "size")));
    check(&e, "r1b-map-nan-zero-keys");
}

/// `r1b-collection-get-tracks-key`
#[test]
fn collection_get_tracks_key() {
    let e = Env::new();
    let m = e.invoke("reactive", &[new_map(&e, &[])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![m.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let v = rt.call(&rt.get_str(&d[0], "get")?, &d[0], &[st("b")])?;
        log.borrow_mut().push(format!("get-b {}", js(rt, &v)));
        Ok(undef())
    });
    method(&e, &m, "set", &[st("a"), num(1.0)]);
    log_line!(e, "runs-other", runs.get());
    method(&e, &m, "set", &[st("b"), num(2.0)]);
    log_line!(e, "runs-b", runs.get());
    check(&e, "r1b-collection-get-tracks-key");
}

/// `r1b-readonly-set-clear-silent`
#[test]
fn readonly_set_clear_silent() {
    let e = Env::new();
    let s = new_set(&e, &[num(1.0)]);
    let r = e.invoke("readonly", &[s.clone()]);
    let cleared = method(&e, &r, "clear", &[]);
    let added = method(&e, &r, "add", &[num(2.0)]);
    let deleted = method(&e, &r, "delete", &[num(1.0)]);
    let has = method(&e, &r, "has", &[num(1.0)]);
    log_line!(e, "clear", sv(&e, &cleared), bo(strict_eq(&added, &r)), bo(deleted.to_boolean()), sv(&e, &e.get(&s, "size")), bo(has.to_boolean()));
    check(&e, "r1b-readonly-set-clear-silent");
}

/// `r1b-map-entry-object-deep-tracking`
#[test]
fn map_entry_object_deep_tracking() {
    let e = Env::new();
    let inner = object(&e, &[("n", num(1.0))]);
    let m = e.invoke("reactive", &[new_map(&e, &[(st("a"), inner)])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![m.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let mut sum = 0.0;
        for pair in iterate_rt(rt, &d[0])? {
            let v = rt.get_v(&pair, &vn_value::PropKey::Index(1))?;
            if let Value::Number(n) = rt.get_str(&v, "n")? {
                sum += n;
            }
        }
        log.borrow_mut().push(format!("sum {sum}"));
        Ok(undef())
    });
    let a = method(&e, &m, "get", &[st("a")]);
    e.set(&a, "n", num(4.0));
    log_line!(e, "runs", runs.get());
    check(&e, "r1b-map-entry-object-deep-tracking");
}

/// `r1b-set-of-proxies-stores-raw`
#[test]
fn set_of_proxies_stores_raw() {
    let e = Env::new();
    let obj = object(&e, &[]);
    let s = e.invoke("reactive", &[new_set(&e, &[])]);
    let proxied = e.invoke("reactive", &[obj.clone()]);
    method(&e, &s, "add", &[proxied.clone()]);
    let raw = e.invoke("toRaw", &[s.clone()]);
    log_line!(
        e,
        "stored",
        bo(method(&e, &raw, "has", &[obj.clone()]).to_boolean()),
        bo(method(&e, &s, "has", &[obj.clone()]).to_boolean()),
        bo(method(&e, &s, "has", &[proxied.clone()]).to_boolean()),
        sv(&e, &e.get(&s, "size"))
    );
    check(&e, "r1b-set-of-proxies-stores-raw");
}

/// `r1b-map-delete-during-iteration`
#[test]
fn map_delete_during_iteration() {
    let e = Env::new();
    let m = e.invoke("reactive", &[new_map(&e, &[(st("a"), num(1.0)), (st("b"), num(2.0)), (st("c"), num(3.0))])]);
    let mut seen = Vec::new();
    let iter = e.call(&e.get(&m, "entries"), &m, &[]);
    let next = e.get(&iter, "next");
    loop {
        let r = e.call(&next, &iter, &[]);
        if e.get(&r, "done").to_boolean() {
            break;
        }
        let k = sv(&e, &e.get(&e.get(&r, "value"), "0"));
        if k == "a" {
            method(&e, &m, "delete", &[st("b")]);
        }
        seen.push(k);
    }
    log_line!(e, "seen", seen.join(","), sv(&e, &e.get(&m, "size")));
    check(&e, "r1b-map-delete-during-iteration");
}

/// `r1b-shallow-set-values-not-wrapped`
#[test]
fn shallow_set_values_not_wrapped() {
    let e = Env::new();
    let inner = object(&e, &[]);
    let s = e.invoke("shallowReactive", &[new_set(&e, &[inner.clone()])]);
    let first = iterate(&e, &method(&e, &s, "values", &[]))[0].clone();
    let first_by_iter = iterate(&e, &s)[0].clone();
    log_line!(
        e,
        "shallow-set",
        bo(strict_eq(&first, &inner)),
        flag(&e, "isReactive", &first),
        flag(&e, "isReactive", &first_by_iter)
    );
    check(&e, "r1b-shallow-set-values-not-wrapped");
}

/// `r1b-readonly-raw-map-no-tracking`: a readonly view of a raw map does not track writes made to the raw map.
#[test]
fn readonly_raw_map_no_tracking() {
    let e = Env::new();
    let raw = new_map(&e, &[]);
    let r = e.invoke("readonly", &[raw.clone()]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![r.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let size = rt.get_str(&d[0], "size")?;
        log.borrow_mut().push(format!("ro-size {}", js(rt, &size)));
        Ok(undef())
    });
    method(&e, &raw, "set", &[st("a"), num(1.0)]);
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&r, "size")));
    check(&e, "r1b-readonly-raw-map-no-tracking");
}

/// `r1b-collection-proto-and-tag`
#[test]
fn collection_proto_and_tag() {
    let e = Env::new();
    let p = e.invoke("reactive", &[new_set(&e, &[])]);
    let set_proto = e.get(&global(&e, "Set"), "prototype");
    let is_set = e.rt.get_prototype_of(p.as_object().expect("proxy")).map(Value::Object) == Some(set_proto.clone());
    log_line!(e, "set-tag", bo(is_set), tag_of(&e, &p), bo(is_set));
    let q = e.invoke("readonly", &[new_map(&e, &[])]);
    let map_proto = e.get(&global(&e, "Map"), "prototype");
    let is_map = e.rt.get_prototype_of(q.as_object().expect("proxy")).map(Value::Object) == Some(map_proto);
    let shallow = e.invoke("shallowReactive", &[new_map(&e, &[])]);
    log_line!(e, "map-tag", bo(is_map), tag_of(&e, &q), flag(&e, "isReactive", &shallow));
    check(&e, "r1b-collection-proto-and-tag");
}

/// `r1b-readonly-set-iteration-wraps`
#[test]
fn readonly_set_iteration_wraps() {
    let e = Env::new();
    let r = e.invoke("readonly", &[new_set(&e, &[object(&e, &[]), num(1.0)])]);
    let flags: Vec<String> = iterate(&e, &r)
        .iter()
        .map(|v| match v {
            Value::Object(_) => flag(&e, "isReadonly", v),
            _ => "number".to_string(),
        })
        .collect();
    log_line!(e, "ro-iter", flags.join(","));
    check(&e, "r1b-readonly-set-iteration-wraps");
}

/// `r1b-map-has-tracks-object-key`
#[test]
fn map_has_tracks_object_key() {
    let e = Env::new();
    let k = object(&e, &[]);
    let m = e.invoke("reactive", &[new_map(&e, &[])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![m.clone(), k.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let has = rt.call(&rt.get_str(&d[0], "has")?, &d[0], &[d[1].clone()])?;
        log.borrow_mut().push(format!("has {}", js(rt, &has)));
        Ok(undef())
    });
    method(&e, &m, "set", &[k.clone(), num(1.0)]);
    log_line!(e, "runs", runs.get());
    method(&e, &m, "delete", &[k.clone()]);
    log_line!(e, "runs2", runs.get());
    check(&e, "r1b-map-has-tracks-object-key");
}

/// `r1b-map-nested-map-values`: a nested Map is reached through the proxy, so writes to it through the proxy run effects.
#[test]
fn map_nested_map_values() {
    let e = Env::new();
    let inner = new_map(&e, &[(st("x"), num(1.0))]);
    let m = e.invoke("reactive", &[new_map(&e, &[(st("inner"), inner.clone())])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![m.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let nested = rt.call(&rt.get_str(&d[0], "get")?, &d[0], &[st("inner")])?;
        let x = rt.call(&rt.get_str(&nested, "get")?, &nested, &[st("x")])?;
        log.borrow_mut().push(format!("x {}", js(rt, &x)));
        Ok(undef())
    });
    method(&e, &inner, "set", &[st("x"), num(2.0)]);
    let nested = method(&e, &m, "get", &[st("inner")]);
    log_line!(e, "runs", runs.get(), flag(&e, "isReactive", &nested));
    method(&e, &nested, "set", &[st("x"), num(3.0)]);
    log_line!(e, "runs2", runs.get(), sv(&e, &method(&e, &inner, "get", &[st("x")])));
    check(&e, "r1b-map-nested-map-values");
}

/// `r1b-set-of-arrays-identity`
#[test]
fn set_of_arrays_identity() {
    let e = Env::new();
    let arr = array(&e, vec![num(1.0)]);
    let s = e.invoke("reactive", &[new_set(&e, &[])]);
    method(&e, &s, "add", &[arr.clone()]);
    let got = iterate(&e, &s)[0].clone();
    let proxy = e.invoke("reactive", &[arr.clone()]);
    log_line!(e, "set-arr", flag(&e, "isReactive", &got), bo(strict_eq(&got, &proxy)));
    check(&e, "r1b-set-of-arrays-identity");
}

/// `r1b-set-clear-triggers-size`
#[test]
fn set_clear_triggers_size() {
    let e = Env::new();
    let s = e.invoke("reactive", &[new_set(&e, &[num(1.0), num(2.0)])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![s.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let size = rt.get_str(&d[0], "size")?;
        log.borrow_mut().push(format!("size {}", js(rt, &size)));
        Ok(undef())
    });
    method(&e, &s, "clear", &[]);
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&s, "size")));
    check(&e, "r1b-set-clear-triggers-size");
}

/// `r1b-array-of-maps-proxied`
#[test]
fn array_of_maps_proxied() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![new_map(&e, &[])])]);
    let first = e.get(&a, "0");
    log_line!(e, "map-in-arr", flag(&e, "isReactive", &first), sv(&e, &e.get(&first, "size")), tag_of(&e, &first));
    check(&e, "r1b-array-of-maps-proxied");
}

/// `r1b-object-with-map-field-size`
#[test]
fn object_with_map_field_size() {
    let e = Env::new();
    let p = e.invoke("reactive", &[object(&e, &[("m", new_map(&e, &[]))])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let m = rt.get_str(&d[0], "m")?;
        let size = rt.get_str(&m, "size")?;
        log.borrow_mut().push(format!("size {}", js(rt, &size)));
        Ok(undef())
    });
    let m = e.get(&p, "m");
    method(&e, &m, "set", &[st("k"), num(1.0)]);
    log_line!(e, "runs", runs.get());
    check(&e, "r1b-object-with-map-field-size");
}

/// `r1b-map-entries-tracking-add-in-effect`
#[test]
fn map_entries_tracking_add_in_effect() {
    let e = Env::new();
    let m = e.invoke("reactive", &[new_map(&e, &[])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![m.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let entries = rt.call(&rt.get_str(&d[0], "entries")?, &d[0], &[])?;
        let n = iterate_rt(rt, &entries)?.len();
        log.borrow_mut().push(format!("entries {n}"));
        Ok(undef())
    });
    method(&e, &m, "set", &[st("a"), num(1.0)]);
    method(&e, &m, "set", &[st("b"), num(2.0)]);
    log_line!(e, "runs", runs.get());
    check(&e, "r1b-map-entries-tracking-add-in-effect");
}

/// `r1b-map-foreach-add-during-visits-new`: `forEach` visits the entries added while it runs.
#[test]
fn map_foreach_add_during_visits_new() {
    let e = Env::new();
    let m = e.invoke("reactive", &[new_map(&e, &[(st("a"), num(1.0))])]);
    let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    let cb = func(&e, "", 2, vec![m.clone()], move |rt, _this, args, d| {
        sink.borrow_mut().push(js(rt, &args[1]));
        if js(rt, &args[1]) == "a" {
            let set = rt.get_str(&d[0], "set")?;
            rt.call(&set, &d[0], &[st("b"), num(2.0)])?;
        }
        Ok(undef())
    });
    method(&e, &m, "forEach", &[cb]);
    log_line!(e, "seen", seen.borrow().join(","), sv(&e, &e.get(&m, "size")));
    check(&e, "r1b-map-foreach-add-during-visits-new");
}

/// `r1b-set-delete-during-foreach`: a value deleted before it is visited is skipped.
#[test]
fn set_delete_during_foreach() {
    let e = Env::new();
    let s = e.invoke("reactive", &[new_set(&e, &[num(1.0), num(2.0), num(3.0)])]);
    let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    let cb = func(&e, "", 1, vec![s.clone()], move |rt, _this, args, d| {
        sink.borrow_mut().push(js(rt, &args[0]));
        if matches!(args[0], Value::Number(n) if n == 1.0) {
            let delete = rt.get_str(&d[0], "delete")?;
            rt.call(&delete, &d[0], &[num(2.0)])?;
        }
        Ok(undef())
    });
    method(&e, &s, "forEach", &[cb]);
    log_line!(e, "seen", seen.borrow().join(","), sv(&e, &e.get(&s, "size")));
    check(&e, "r1b-set-delete-during-foreach");
}

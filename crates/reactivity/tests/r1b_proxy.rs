//! Reactive object proxies: flags, identity and caching, property tracking, readonly and shallow variants, markRaw, frozen
//! bypass and refs inside reactive objects. Each test is the Rust translation of one snippet in
//! tests/vue/rust/reactive/snippets.mjs (id in the doc comment).

#[macro_use]
mod r1b_support;
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use common::{js, Env};
use r1b_support::{array, bo, check, flag, func, global, method, num, object, run_effect, st, sv, undef};
use vn_value::{strict_eq, Flags, Intrinsic, JsStr, PropKey, Res, Rt, Value, WellKnown};

/// Reads `obj[key]` for a Rust-side key.
fn get_key(rt: &Rt, obj: &Value, key: &str) -> Res<Value> {
    rt.get_str(obj, key)
}

/// `r1b-flags-reactive`
#[test]
fn flags_reactive() {
    let e = Env::new();
    let raw = object(&e, &[("a", num(1.0))]);
    let p = e.invoke("reactive", &[raw.clone()]);
    let again = e.invoke("reactive", &[raw.clone()]);
    let p_again = e.invoke("reactive", &[p.clone()]);
    log_line!(e, "same", bo(strict_eq(&p, &again)), bo(strict_eq(&p, &raw)), bo(strict_eq(&p_again, &p)));
    log_line!(e, "flags", flag(&e, "isReactive", &p), flag(&e, "isReadonly", &p), flag(&e, "isShallow", &p), flag(&e, "isProxy", &p));
    log_line!(e, "raw", flag(&e, "isReactive", &raw), flag(&e, "isReadonly", &raw), flag(&e, "isShallow", &raw), flag(&e, "isProxy", &raw));
    let p_raw = e.invoke("toRaw", &[p.clone()]);
    let raw_raw = e.invoke("toRaw", &[raw.clone()]);
    log_line!(e, "toRaw", bo(strict_eq(&p_raw, &raw)), bo(strict_eq(&raw_raw, &raw)), sv(&e, &e.invoke("toRaw", &[num(1.0)])));
    check(&e, "r1b-flags-reactive");
}

/// `r1b-readonly-flags`
#[test]
fn readonly_flags() {
    let e = Env::new();
    let raw = object(&e, &[("a", num(1.0))]);
    let r = e.invoke("readonly", &[raw.clone()]);
    let r_raw = e.invoke("toRaw", &[r.clone()]);
    log_line!(e, "flags", flag(&e, "isReactive", &r), flag(&e, "isReadonly", &r), flag(&e, "isShallow", &r), flag(&e, "isProxy", &r), bo(strict_eq(&r_raw, &raw)));
    let r_again = e.invoke("readonly", &[raw.clone()]);
    let r_of_r = e.invoke("readonly", &[r.clone()]);
    let reactive_of_r = e.invoke("reactive", &[r.clone()]);
    log_line!(e, "same", bo(strict_eq(&r_again, &r)), bo(strict_eq(&r_of_r, &r)), bo(strict_eq(&reactive_of_r, &r)));
    let p = e.invoke("reactive", &[raw.clone()]);
    let rp = e.invoke("readonly", &[p.clone()]);
    let rp_raw = e.invoke("toRaw", &[rp.clone()]);
    log_line!(e, "nested", flag(&e, "isReactive", &rp), flag(&e, "isReadonly", &rp), bo(strict_eq(&rp_raw, &raw)), bo(strict_eq(&rp, &r)));
    let reactive_of_rp = e.invoke("reactive", &[rp.clone()]);
    log_line!(e, "reactive-of-nested", bo(strict_eq(&reactive_of_rp, &rp)));
    check(&e, "r1b-readonly-flags");
}

/// `r1b-shallow-flags`
#[test]
fn shallow_flags() {
    let e = Env::new();
    let inner = object(&e, &[("b", num(1.0))]);
    let raw = object(&e, &[("a", inner)]);
    let s = e.invoke("shallowReactive", &[raw.clone()]);
    let deep = e.invoke("reactive", &[raw.clone()]);
    let s_raw = e.invoke("toRaw", &[s.clone()]);
    log_line!(e, "sr", flag(&e, "isReactive", &s), flag(&e, "isReadonly", &s), flag(&e, "isShallow", &s), bo(strict_eq(&s, &deep)), bo(strict_eq(&s_raw, &raw)));
    let sro = e.invoke("shallowReadonly", &[raw.clone()]);
    let ro = e.invoke("readonly", &[raw.clone()]);
    log_line!(e, "sro", flag(&e, "isReactive", &sro), flag(&e, "isReadonly", &sro), flag(&e, "isShallow", &sro), bo(strict_eq(&sro, &ro)));
    let s_a = e.get(&s, "a");
    let sro_a = e.get(&sro, "a");
    log_line!(e, "nested-shallow", flag(&e, "isReactive", &s_a), flag(&e, "isReactive", &sro_a), flag(&e, "isReadonly", &sro_a));
    check(&e, "r1b-shallow-flags");
}

/// `r1b-get-tracks-property`
#[test]
fn get_tracks_property() {
    let e = Env::new();
    let raw = object(&e, &[("a", num(1.0))]);
    let p = e.invoke("reactive", &[raw]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let a = get_key(rt, &d[0], "a")?;
        log.borrow_mut().push(format!("read {}", js(rt, &a)));
        Ok(undef())
    });
    e.set(&p, "a", num(2.0));
    log_line!(e, "runs", runs.get());
    e.set(&p, "a", num(2.0));
    log_line!(e, "runs-same", runs.get());
    e.set(&p, "b", num(3.0));
    log_line!(e, "runs-new-key", runs.get());
    check(&e, "r1b-get-tracks-property");
}

/// `r1b-has-tracks-key`
#[test]
fn has_tracks_key() {
    let e = Env::new();
    let p = e.invoke("reactive", &[object(&e, &[])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let has = rt.has_property(d[0].as_object().expect("proxy"), &PropKey::from_str("x"))?;
        log.borrow_mut().push(format!("has {has}"));
        Ok(undef())
    });
    e.set(&p, "x", num(1.0));
    log_line!(e, "runs", runs.get());
    e.rt.delete(p.as_object().expect("proxy"), &PropKey::from_str("x")).expect("delete");
    log_line!(e, "runs-after-delete", runs.get());
    check(&e, "r1b-has-tracks-key");
}

/// `r1b-delete-missing-no-trigger`
#[test]
fn delete_missing_no_trigger() {
    let e = Env::new();
    let raw = object(&e, &[("a", num(1.0))]);
    let p = e.invoke("reactive", &[raw]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let a = get_key(rt, &d[0], "a")?;
        log.borrow_mut().push(format!("ok {}", js(rt, &a)));
        Ok(undef())
    });
    let obj = p.as_object().expect("proxy");
    let missing = e.rt.delete(obj, &PropKey::from_str("zz")).expect("delete");
    log_line!(e, "del-missing", bo(missing), runs.get());
    let present = e.rt.delete(obj, &PropKey::from_str("a")).expect("delete");
    log_line!(e, "del", bo(present), runs.get());
    check(&e, "r1b-delete-missing-no-trigger");
}

/// `r1b-ownkeys-tracks-iterate`
#[test]
fn ownkeys_tracks_iterate() {
    let e = Env::new();
    let raw = object(&e, &[("a", num(1.0))]);
    let p = e.invoke("reactive", &[raw]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let object_ctor = rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), "Object")?;
        let keys = rt.get_str(&object_ctor, "keys")?;
        let list = rt.call(&keys, &object_ctor, &[d[0].clone()])?;
        let joined = rt.call(&rt.get_str(&list, "join")?, &list, &[st(",")])?;
        log.borrow_mut().push(format!("keys {}", js(rt, &joined)));
        Ok(undef())
    });
    e.set(&p, "b", num(2.0));
    log_line!(e, "after-add", runs.get());
    e.set(&p, "a", num(5.0));
    log_line!(e, "after-set", runs.get());
    e.rt.delete(p.as_object().expect("proxy"), &PropKey::from_str("a")).expect("delete");
    log_line!(e, "after-delete", runs.get());
    check(&e, "r1b-ownkeys-tracks-iterate");
}

/// `r1b-for-in-tracks` (for-in over own enumerable keys, as Object.keys; this snippet has no inherited keys)
#[test]
fn for_in_tracks() {
    let e = Env::new();
    let raw = object(&e, &[("a", num(1.0)), ("b", num(2.0))]);
    let p = e.invoke("reactive", &[raw]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let object_ctor = rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), "Object")?;
        let keys = rt.call(&rt.get_str(&object_ctor, "keys")?, &object_ctor, &[d[0].clone()])?;
        let joined = rt.call(&rt.get_str(&keys, "join")?, &keys, &[st("")])?;
        log.borrow_mut().push(format!("forin {}", js(rt, &joined)));
        Ok(undef())
    });
    e.set(&p, "c", num(3.0));
    log_line!(e, "runs", runs.get());
    e.set(&p, "a", num(10.0));
    log_line!(e, "runs-set", runs.get());
    check(&e, "r1b-for-in-tracks");
}

/// `r1b-set-same-value-no-trigger`
#[test]
fn set_same_value_no_trigger() {
    let e = Env::new();
    let raw = object(&e, &[("n", num(f64::NAN)), ("z", num(0.0))]);
    let p = e.invoke("reactive", &[raw]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let n = get_key(rt, &d[0], "n")?;
        let z = get_key(rt, &d[0], "z")?;
        log.borrow_mut().push(format!("read {} {}", js(rt, &n), js(rt, &z)));
        Ok(undef())
    });
    e.set(&p, "n", num(f64::NAN));
    e.set(&p, "z", num(-0.0));
    log_line!(e, "runs", runs.get());
    e.set(&p, "z", num(1.0));
    log_line!(e, "runs2", runs.get());
    check(&e, "r1b-set-same-value-no-trigger");
}

/// `r1b-getter-receiver-tracks`
#[test]
fn getter_receiver_tracks() {
    let e = Env::new();
    let raw = object(&e, &[("_v", num(1.0))]);
    let getter = func(&e, "get v", 0, vec![], |rt, this, _args, _d| {
        let v = rt.get_str(this, "_v")?;
        Ok(num(match v {
            Value::Number(n) => n * 10.0,
            _ => f64::NAN,
        }))
    });
    let raw_obj = raw.as_object().expect("object");
    e.rt.define_accessor(raw_obj, PropKey::from_str("v"), Some(getter.as_object().expect("fn")), None, Flags::ALL)
        .expect("accessor");
    let p = e.invoke("reactive", &[raw]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let v = get_key(rt, &d[0], "v")?;
        log.borrow_mut().push(format!("v {}", js(rt, &v)));
        Ok(undef())
    });
    e.set(&p, "_v", num(2.0));
    log_line!(e, "runs", runs.get());
    check(&e, "r1b-getter-receiver-tracks");
}

/// `r1b-setter-receiver-is-proxy`
#[test]
fn setter_receiver_is_proxy() {
    let e = Env::new();
    let raw = object(&e, &[("_x", num(0.0))]);
    let p = e.invoke("reactive", &[raw.clone()]);
    let seen: Rc<RefCell<Vec<bool>>> = Rc::new(RefCell::new(Vec::new()));
    let seen_set = seen.clone();
    let setter = func(&e, "set x", 1, vec![p.clone()], move |rt, this, args, d| {
        seen_set.borrow_mut().push(strict_eq(this, &d[0]));
        rt.set_v_strict(this, &PropKey::from_str("_x"), args[0].clone())?;
        Ok(undef())
    });
    let getter = func(&e, "get x", 0, vec![], |rt, this, _args, _d| rt.get_str(this, "_x"));
    e.rt.define_accessor(
        raw.as_object().expect("object"),
        PropKey::from_str("x"),
        Some(getter.as_object().expect("fn")),
        Some(setter.as_object().expect("fn")),
        Flags::ALL,
    )
    .expect("accessor");
    e.set(&p, "x", num(5.0));
    let seen_len = seen.borrow().len();
    let first = seen.borrow()[0];
    log_line!(e, "setter", seen_len, bo(first), sv(&e, &e.get(&p, "_x")), sv(&e, &e.get(&raw, "_x")));
    check(&e, "r1b-setter-receiver-is-proxy");
}

/// `r1b-symbol-keys`
#[test]
fn symbol_keys() {
    let e = Env::new();
    let sym = e.rt.new_symbol(Some(JsStr::from_str("s")));
    let raw = object(&e, &[]);
    e.rt.define_data(raw.as_object().expect("object"), PropKey::Sym(sym), num(1.0), Flags::ALL).expect("symbol key");
    let p = e.invoke("reactive", &[raw]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let v = rt.get(d[0].as_object().expect("proxy"), &PropKey::Sym(sym), &d[0])?;
        log.borrow_mut().push(format!("sym {}", js(rt, &v)));
        Ok(undef())
    });
    e.rt.set_v_strict(&p, &PropKey::Sym(sym), num(2.0)).expect("symbol set");
    log_line!(e, "runs", runs.get());
    let arr = e.invoke("reactive", &[array(&e, vec![])]);
    let has_sym = e.rt.has_property(p.as_object().expect("proxy"), &PropKey::Sym(sym)).expect("has");
    let has_iter = e.rt.has_property(arr.as_object().expect("proxy"), &PropKey::Sym(e.rt.well_known(WellKnown::Iterator))).expect("has");
    log_line!(e, "has-sym", bo(has_sym), bo(has_iter));
    check(&e, "r1b-symbol-keys");
}

/// `r1b-nested-object-tracks`
#[test]
fn nested_object_tracks() {
    let e = Env::new();
    let inner = object(&e, &[("x", num(1.0))]);
    let raw = object(&e, &[("inner", inner)]);
    let p = e.invoke("reactive", &[raw]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let inner = get_key(rt, &d[0], "inner")?;
        let x = get_key(rt, &inner, "x")?;
        log.borrow_mut().push(format!("x {}", js(rt, &x)));
        Ok(undef())
    });
    let inner_proxy = e.get(&p, "inner");
    e.set(&inner_proxy, "x", num(2.0));
    log_line!(e, "runs", runs.get());
    let again = e.get(&p, "inner");
    let proto = e.get(&e.get(&p, "inner"), "__proto__");
    log_line!(e, "same-inner", bo(strict_eq(&inner_proxy, &again)), bo(!strict_eq(&e.get(&p, "inner"), &proto)));
    check(&e, "r1b-nested-object-tracks");
}

/// `r1b-proxy-cache-identity`
#[test]
fn proxy_cache_identity() {
    let e = Env::new();
    let raw = object(&e, &[]);
    let a = e.invoke("reactive", &[raw.clone()]);
    let b = e.invoke("reactive", &[raw.clone()]);
    let a_again = e.invoke("reactive", &[a.clone()]);
    log_line!(e, "reactive-cached", bo(strict_eq(&a, &b)), bo(strict_eq(&a_again, &a)));
    let r1 = e.invoke("readonly", &[raw.clone()]);
    let r2 = e.invoke("readonly", &[raw.clone()]);
    let readonly_of_a = e.invoke("readonly", &[a.clone()]);
    let reactive_of_r1 = e.invoke("reactive", &[r1.clone()]);
    log_line!(e, "readonly-cached", bo(strict_eq(&r1, &r2)), bo(strict_eq(&readonly_of_a, &r1)), bo(strict_eq(&reactive_of_r1, &r1)));
    let s1 = e.invoke("shallowReactive", &[raw.clone()]);
    let s1_again = e.invoke("shallowReactive", &[raw.clone()]);
    log_line!(e, "shallow-cached", bo(strict_eq(&s1, &s1_again)), bo(!strict_eq(&s1, &a)));
    check(&e, "r1b-proxy-cache-identity");
}

/// `r1b-readonly-set-and-delete-silent`
#[test]
fn readonly_set_and_delete_silent() {
    let e = Env::new();
    let raw = object(&e, &[("a", num(1.0))]);
    let r = e.invoke("readonly", &[raw.clone()]);
    e.set(&r, "a", num(2.0));
    log_line!(e, "after set", sv(&e, &e.get(&r, "a")), sv(&e, &e.get(&raw, "a")));
    let deleted = e.rt.delete(r.as_object().expect("proxy"), &PropKey::from_str("a")).expect("delete");
    log_line!(e, "delete", bo(deleted), sv(&e, &e.get(&r, "a")));
    check(&e, "r1b-readonly-set-and-delete-silent");
}

/// `r1b-readonly-nested-wrapped`
#[test]
fn readonly_nested_wrapped() {
    let e = Env::new();
    let inner = object(&e, &[("x", num(1.0))]);
    let raw = object(&e, &[("inner", inner.clone())]);
    let r = e.invoke("readonly", &[raw.clone()]);
    let r_inner = e.get(&r, "inner");
    let readonly_inner = e.invoke("readonly", &[inner.clone()]);
    log_line!(e, "inner-ro", sv(&e, &e.get(&r_inner, "x")), bo(strict_eq(&readonly_inner, &r_inner)));
    e.set(&r_inner, "x", num(9.0));
    log_line!(e, "inner-after", sv(&e, &e.get(&inner, "x")), sv(&e, &e.get(&r_inner, "x")));
    check(&e, "r1b-readonly-nested-wrapped");
}

/// `r1b-markraw-skips-wrapping`
#[test]
fn markraw_skips_wrapping() {
    let e = Env::new();
    let inner = object(&e, &[("b", num(1.0))]);
    let raw = object(&e, &[("a", inner.clone())]);
    let m = e.invoke("markRaw", &[raw.clone()]);
    log_line!(e, "marked", bo(strict_eq(&m, &raw)), sv(&e, &e.get(&raw, "__v_skip")));
    let p = e.invoke("reactive", &[raw.clone()]);
    let reactive_inner = e.invoke("reactive", &[inner.clone()]);
    log_line!(e, "skip", bo(strict_eq(&p, &raw)), flag(&e, "isReactive", &p), bo(strict_eq(&reactive_inner, &inner)));
    let mark_child = object(&e, &[("k", num(1.0))]);
    let outer_raw = object(&e, &[("m", e.invoke("markRaw", &[mark_child]))]);
    let outer = e.invoke("reactive", &[outer_raw]);
    let outer_m = e.get(&outer, "m");
    log_line!(e, "nested-marked", flag(&e, "isReactive", &outer_m), sv(&e, &e.get(&outer_m, "k")));
    check(&e, "r1b-markraw-skips-wrapping");
}

/// `r1b-frozen-and-nonextensible-bypass`
#[test]
fn frozen_and_nonextensible_bypass() {
    let e = Env::new();
    let object_ctor = global(&e, "Object");
    let inner = object(&e, &[("x", num(1.0))]);
    let f = method(&e, &object_ctor, "freeze", &[object(&e, &[("a", inner)])]);
    let p = e.invoke("reactive", &[f.clone()]);
    let readonly_f = e.invoke("readonly", &[f.clone()]);
    log_line!(e, "frozen", bo(strict_eq(&p, &f)), flag(&e, "isReactive", &p), bo(strict_eq(&readonly_f, &f)));
    let ne = method(&e, &object_ctor, "preventExtensions", &[object(&e, &[("q", num(1.0))])]);
    let reactive_ne = e.invoke("reactive", &[ne.clone()]);
    let readonly_ne = e.invoke("readonly", &[ne.clone()]);
    log_line!(e, "nonext", bo(strict_eq(&reactive_ne, &ne)), bo(strict_eq(&readonly_ne, &ne)));
    check(&e, "r1b-frozen-and-nonextensible-bypass");
}

/// `r1b-non-plain-and-primitives-bypass`
#[test]
fn non_plain_and_primitives_bypass() {
    let e = Env::new();
    let number_ctor = global(&e, "Number");
    let w = e.rt.construct(&number_ctor, &[num(7.0)], &number_ctor).expect("new Number");
    let pr = method(&e, &global(&e, "Promise"), "resolve", &[num(1.0)]);
    let reactive_w = e.invoke("reactive", &[w.clone()]);
    let reactive_pr = e.invoke("reactive", &[pr.clone()]);
    log_line!(e, "wrapper", bo(strict_eq(&reactive_w, &w)), flag(&e, "isReactive", &w), bo(strict_eq(&reactive_pr, &pr)));
    let f = func(&e, "", 0, vec![], |_rt, _this, _args, _d| Ok(undef()));
    let reactive_f = e.invoke("reactive", &[f.clone()]);
    log_line!(e, "fn", bo(strict_eq(&reactive_f, &f)));
    let prims: Vec<String> = [num(1.0), st("s"), Value::Null, undef()]
        .iter()
        .map(|v| sv(&e, &e.invoke("reactive", &[v.clone()])))
        .collect();
    log_line!(e, "prims", prims[0], prims[1], prims[2], prims[3]);
    check(&e, "r1b-non-plain-and-primitives-bypass");
}

/// `r1b-refs-unwrap-in-reactive`
#[test]
fn refs_unwrap_in_reactive() {
    let e = Env::new();
    let count = e.invoke("ref", &[num(1.0)]);
    let raw = object(&e, &[("count", count.clone()), ("list", array(&e, vec![num(1.0)]))]);
    let p = e.invoke("reactive", &[raw]);
    log_line!(e, "unwrap", sv(&e, &e.get(&p, "count")), flag(&e, "isRef", &e.get(&p, "count")));
    e.set(&p, "count", num(5.0));
    log_line!(e, "written", sv(&e, &e.get(&count, "value")), sv(&e, &e.get(&p, "count")));
    let other = e.invoke("ref", &[num(0.0)]);
    e.set(&p, "count", other.clone());
    log_line!(e, "replaced", sv(&e, &e.get(&p, "count")), sv(&e, &e.get(&count, "value")), sv(&e, &e.get(&other, "value")));
    check(&e, "r1b-refs-unwrap-in-reactive");
}

/// `r1b-shallow-reactive-keeps-refs`
#[test]
fn shallow_reactive_keeps_refs() {
    let e = Env::new();
    let r = e.invoke("ref", &[num(2.0)]);
    let raw = object(&e, &[("r", r.clone())]);
    let p = e.invoke("shallowReactive", &[raw.clone()]);
    let p_r = e.get(&p, "r");
    log_line!(e, "shallow-keep", bo(strict_eq(&p_r, &r)), sv(&e, &e.get(&p_r, "value")));
    let q = e.invoke("reactive", &[object(&e, &[("r", r.clone())])]);
    let q_r = e.get(&q, "r");
    log_line!(e, "deep-unwrap", bo(strict_eq(&q_r, &r)), sv(&e, &q_r));
    check(&e, "r1b-shallow-reactive-keeps-refs");
}

/// `r1b-tor-chain-and-isproxy`
#[test]
fn tor_chain_and_isproxy() {
    let e = Env::new();
    let raw = object(&e, &[]);
    let rr = e.invoke("reactive", &[raw.clone()]);
    let deep = e.invoke("readonly", &[rr.clone()]);
    let deep_raw = e.invoke("toRaw", &[deep.clone()]);
    log_line!(e, "chain", bo(strict_eq(&deep_raw, &raw)), flag(&e, "isProxy", &deep), flag(&e, "isProxy", &raw));
    let sr = e.invoke("shallowReactive", &[raw.clone()]);
    let sr_raw = e.invoke("toRaw", &[sr.clone()]);
    let ro_sr = e.invoke("readonly", &[sr.clone()]);
    let ro_sr_raw = e.invoke("toRaw", &[ro_sr.clone()]);
    log_line!(e, "sr", bo(strict_eq(&sr_raw, &raw)), bo(strict_eq(&ro_sr_raw, &raw)));
    check(&e, "r1b-tor-chain-and-isproxy");
}

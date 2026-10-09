//! Readonly and shallow variants, flags on primitives and prototypes, property descriptors and keys, freezing, cycles,
//! tagged objects, and GC safety of proxies with cross references. Each test is the Rust translation of one snippet in
//! tests/vue/rust/reactive/snippets.mjs (id in the doc comment). The audit at the end checks that every snippet has one.

#[macro_use]
mod r1b_support;
mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::{js, Env};
use r1b_support::{
    array, bo, check, flag, global, keys_joined, method, new_map, num, object, oracle_ids, run_effect, st, sv, tag_of, undef,
};
use vn_value::{strict_eq, Flags, Prop, PropKey, Value, WellKnown};

/// `r1b-readonly-tracks-underlying`: readonly over a reactive object tracks the writes made through the reactive object.
#[test]
fn readonly_tracks_underlying() {
    let e = Env::new();
    let raw = e.invoke("reactive", &[object(&e, &[("n", num(1.0))])]);
    let ro = e.invoke("readonly", &[raw.clone()]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![ro.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let n = rt.get_str(&d[0], "n")?;
        log.borrow_mut().push(format!("ro-read {}", js(rt, &n)));
        Ok(undef())
    });
    e.set(&raw, "n", num(2.0));
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&ro, "n")));
    check(&e, "r1b-readonly-tracks-underlying");
}

/// `r1b-readonly-of-raw-no-tracking`: readonly over a plain object tracks nothing.
#[test]
fn readonly_of_raw_no_tracking() {
    let e = Env::new();
    let raw = object(&e, &[("n", num(1.0))]);
    let ro = e.invoke("readonly", &[raw.clone()]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![ro.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let n = rt.get_str(&d[0], "n")?;
        log.borrow_mut().push(format!("ro-read {}", js(rt, &n)));
        Ok(undef())
    });
    e.set(&raw, "n", num(2.0));
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&ro, "n")));
    check(&e, "r1b-readonly-of-raw-no-tracking");
}

/// `r1b-shallow-nested-not-tracked`: a shallow root does not track its nested objects, but tracks its own keys.
#[test]
fn shallow_nested_not_tracked() {
    let e = Env::new();
    let inner = object(&e, &[("n", num(1.0))]);
    let s = e.invoke("shallowReactive", &[object(&e, &[("inner", inner)])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![s.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let inner = rt.get_str(&d[0], "inner")?;
        let n = rt.get_str(&inner, "n")?;
        log.borrow_mut().push(format!("shallow-n {}", js(rt, &n)));
        Ok(undef())
    });
    let inner_now = e.get(&s, "inner");
    e.set(&inner_now, "n", num(2.0));
    log_line!(e, "runs", runs.get());
    e.set(&s, "inner", object(&e, &[("n", num(3.0))]));
    log_line!(e, "runs2", runs.get());
    check(&e, "r1b-shallow-nested-not-tracked");
}

/// `r1b-shallow-readonly-root-only`
#[test]
fn shallow_readonly_root_only() {
    let e = Env::new();
    let inner = object(&e, &[("n", num(1.0))]);
    let raw = object(&e, &[("inner", inner.clone())]);
    let sr = e.invoke("shallowReadonly", &[raw.clone()]);
    let sr_inner = e.get(&sr, "inner");
    log_line!(e, "sro-root", flag(&e, "isReadonly", &sr_inner), flag(&e, "isReactive", &sr_inner), bo(strict_eq(&sr_inner, &inner)));
    e.set(&sr_inner, "n", num(5.0));
    log_line!(e, "sro-write", sv(&e, &e.get(&inner, "n")));
    check(&e, "r1b-shallow-readonly-root-only");
}

/// `r1b-readonly-vs-shallow-on-refs`
#[test]
fn readonly_vs_shallow_on_refs() {
    let e = Env::new();
    let r = e.invoke("ref", &[num(3.0)]);
    let d = e.invoke("readonly", &[object(&e, &[("r", r.clone())])]);
    let s = e.invoke("shallowReadonly", &[object(&e, &[("r", r.clone())])]);
    let d_r = e.get(&d, "r");
    let s_r = e.get(&s, "r");
    log_line!(e, "ro-unwrap", sv(&e, &d_r), flag(&e, "isRef", &d_r), bo(strict_eq(&s_r, &r)), flag(&e, "isRef", &s_r));
    check(&e, "r1b-readonly-vs-shallow-on-refs");
}

/// `r1b-is-flags-on-primitives`
#[test]
fn is_flags_on_primitives() {
    let e = Env::new();
    log_line!(
        e,
        "prims",
        flag(&e, "isReactive", &num(0.0)),
        flag(&e, "isReadonly", &st("")),
        flag(&e, "isShallow", &Value::Null),
        flag(&e, "isProxy", &undef()),
        flag(&e, "isRef", &Value::Bool(false)),
        sv(&e, &e.invoke("toRaw", &[Value::Null])),
        sv(&e, &e.invoke("toRaw", &[undef()])),
        sv(&e, &e.invoke("toRaw", &[num(7.0)]))
    );
    check(&e, "r1b-is-flags-on-primitives");
}

/// `r1b-proto-chain-get-and-flags`
#[test]
fn proto_chain_get_and_flags() {
    let e = Env::new();
    let base = object(&e, &[("b", num(1.0))]);
    let child = method(&e, &global(&e, "Object"), "create", &[base]);
    let p = e.invoke("reactive", &[child]);
    let proto_of_p = method(&e, &global(&e, "Object"), "getPrototypeOf", &[p.clone()]);
    log_line!(e, "proto-get", sv(&e, &e.get(&p, "b")), flag(&e, "isReactive", &e.get(&p, "__proto__")), flag(&e, "isReactive", &proto_of_p));
    check(&e, "r1b-proto-chain-get-and-flags");
}

/// `r1b-set-through-prototype-chain`: assigning an inherited key creates an own key on the proxy's target.
#[test]
fn set_through_prototype_chain() {
    let e = Env::new();
    let base = object(&e, &[("b", num(1.0))]);
    let child = method(&e, &global(&e, "Object"), "create", &[base.clone()]);
    let c = e.invoke("reactive", &[child]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![c.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let b = rt.get_str(&d[0], "b")?;
        log.borrow_mut().push(format!("b {}", js(rt, &b)));
        Ok(undef())
    });
    e.set(&c, "b", num(2.0));
    let keys = method(&e, &global(&e, "Object"), "keys", &[c.clone()]);
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&base, "b")), sv(&e, &e.get(&keys, "length")));
    check(&e, "r1b-set-through-prototype-chain");
}

/// `r1b-define-property-no-trigger`: a define has no trap, so it writes the target without tracking. The kernel has no
/// `Object.defineProperty`, so this calls its `[[DefineOwnProperty]]` (the same path) with complete data descriptors.
#[test]
fn define_property_no_trigger() {
    let e = Env::new();
    let p = e.invoke("reactive", &[object(&e, &[("a", num(1.0))])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let a = rt.get_str(&d[0], "a")?;
        log.borrow_mut().push(format!("a {}", js(rt, &a)));
        Ok(undef())
    });
    let target = p.as_object().expect("proxy");
    e.rt.define_own(target, &PropKey::from_str("a"), Prop::Data { value: num(9.0), flags: Flags::ALL })
        .expect("[[DefineOwnProperty]] of a");
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&p, "a")));
    e.rt.define_own(target, &PropKey::from_str("b"), Prop::Data { value: num(2.0), flags: Flags::ALL })
        .expect("[[DefineOwnProperty]] of b");
    log_line!(e, "runs2", runs.get(), sv(&e, &e.get(&p, "b")));
    check(&e, "r1b-define-property-no-trigger");
}

/// `r1b-gopd-returns-raw-value`: descriptors are not wrapped (there is no getOwnPropertyDescriptor trap).
#[test]
fn gopd_returns_raw_value() {
    let e = Env::new();
    let inner = object(&e, &[("n", num(1.0))]);
    let p = e.invoke("reactive", &[object(&e, &[("inner", inner.clone())])]);
    let object_ctor = global(&e, "Object");
    let d = method(&e, &object_ctor, "getOwnPropertyDescriptor", &[p.clone(), st("inner")]);
    let value = e.get(&d, "value");
    let again = method(&e, &object_ctor, "getOwnPropertyDescriptor", &[p.clone(), st("inner")]);
    log_line!(e, "gopd", bo(strict_eq(&value, &inner)), sv(&e, &e.get(&again, "enumerable")), sv(&e, &e.get(&d, "writable")), keys_joined(&e, &p, ","));
    check(&e, "r1b-gopd-returns-raw-value");
}

/// `r1b-object-entries-values-wrapped`
#[test]
fn object_entries_values_wrapped() {
    let e = Env::new();
    let a_obj = object(&e, &[("x", num(1.0))]);
    let p = e.invoke("reactive", &[object(&e, &[("a", a_obj), ("b", num(2.0))])]);
    let object_ctor = global(&e, "Object");
    let entries = method(&e, &object_ctor, "entries", &[p.clone()]);
    let first_entry = e.get(&entries, "0");
    log_line!(e, "entries", sv(&e, &e.get(&entries, "length")), flag(&e, "isReactive", &e.get(&first_entry, "1")), sv(&e, &e.get(&e.get(&entries, "1"), "1")));
    let values = method(&e, &object_ctor, "values", &[p.clone()]);
    log_line!(e, "values", flag(&e, "isReactive", &e.get(&values, "0")), sv(&e, &e.get(&values, "1")));
    let target = Value::Object(e.rt.new_object());
    let assigned = method(&e, &object_ctor, "assign", &[target, p.clone()]);
    let object_proto = Value::Object(e.rt.intrinsic(vn_value::Intrinsic::ObjectPrototype));
    let assigned_proto = method(&e, &object_ctor, "getPrototypeOf", &[assigned.clone()]);
    log_line!(e, "assign", flag(&e, "isReactive", &e.get(&assigned, "a")), sv(&e, &e.get(&assigned, "b")), bo(strict_eq(&assigned_proto, &object_proto)));
    check(&e, "r1b-object-entries-values-wrapped");
}

/// `r1b-in-operator-inherited-tracking`
#[test]
fn in_operator_inherited_tracking() {
    let e = Env::new();
    let base = object(&e, &[("inherited", Value::Bool(true))]);
    let child = method(&e, &global(&e, "Object"), "create", &[base.clone()]);
    let p = e.invoke("reactive", &[child]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let has = rt.has_property(d[0].as_object().expect("proxy"), &PropKey::from_str("inherited"))?;
        let v = rt.get_str(&d[0], "inherited")?;
        log.borrow_mut().push(format!("in {has} {}", js(rt, &v)));
        Ok(undef())
    });
    e.set(&p, "inherited", Value::Bool(false));
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&base, "inherited")));
    check(&e, "r1b-in-operator-inherited-tracking");
}

/// `r1b-proto-of-proxy-is-raw`
#[test]
fn proto_of_proxy_is_raw() {
    let e = Env::new();
    let base = object(&e, &[("x", num(1.0))]);
    let child = method(&e, &global(&e, "Object"), "create", &[base.clone()]);
    let p = e.invoke("reactive", &[child]);
    let proto = method(&e, &global(&e, "Object"), "getPrototypeOf", &[p.clone()]);
    log_line!(e, "proto", bo(strict_eq(&proto, &base)), flag(&e, "isReactive", &proto));
    check(&e, "r1b-proto-of-proxy-is-raw");
}

/// `r1b-cycles-through-proxies`
#[test]
fn cycles_through_proxies() {
    let e = Env::new();
    let node = object(&e, &[("name", st("n"))]);
    e.set(&node, "self", node.clone());
    let p = e.invoke("reactive", &[node.clone()]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let s1 = rt.get_str(&d[0], "self")?;
        let s2 = rt.get_str(&s1, "self")?;
        let name = rt.get_str(&s2, "name")?;
        log.borrow_mut().push(format!("name {}", js(rt, &name)));
        Ok(undef())
    });
    let p_self = e.get(&p, "self");
    e.set(&p_self, "name", st("m"));
    let p_self_again = e.get(&p, "self");
    let p_self_self = e.get(&p_self_again, "self");
    log_line!(
        e,
        "runs",
        runs.get(),
        bo(strict_eq(&e.invoke("toRaw", &[p_self.clone()]), &node)),
        bo(strict_eq(&p_self, &p_self_self)),
        bo(strict_eq(&e.get(&p_self_self, "self"), &p))
    );
    check(&e, "r1b-cycles-through-proxies");
}

/// `r1b-frozen-nested-not-proxied`
#[test]
fn frozen_nested_not_proxied() {
    let e = Env::new();
    let frozen_inner = method(&e, &global(&e, "Object"), "freeze", &[object(&e, &[("v", num(1.0))])]);
    let plain = object(&e, &[("v", num(2.0))]);
    let p = e.invoke("reactive", &[object(&e, &[("inner", frozen_inner.clone()), ("plain", plain)])]);
    let inner = e.get(&p, "inner");
    log_line!(e, "frozen-inner", bo(strict_eq(&inner, &frozen_inner)), flag(&e, "isReactive", &inner), flag(&e, "isReactive", &e.get(&p, "plain")));
    check(&e, "r1b-frozen-nested-not-proxied");
}

/// `r1b-freeze-readonly-proxy-freezes-target`: freezing a readonly proxy has no trap, so it freezes the target.
#[test]
fn freeze_readonly_proxy_freezes_target() {
    let e = Env::new();
    let raw = object(&e, &[("a", num(1.0))]);
    let r = e.invoke("readonly", &[raw.clone()]);
    let object_ctor = global(&e, "Object");
    let frozen = method(&e, &object_ctor, "freeze", &[r.clone()]);
    let raw_frozen = method(&e, &object_ctor, "isFrozen", &[raw.clone()]);
    log_line!(e, "ro-after-freeze", flag(&e, "isReadonly", &frozen), sv(&e, &raw_frozen), sv(&e, &e.get(&r, "a")));
    check(&e, "r1b-freeze-readonly-proxy-freezes-target");
}

/// `r1b-reactive-of-readonly-unchanged`
#[test]
fn reactive_of_readonly_unchanged() {
    let e = Env::new();
    let r = e.invoke("readonly", &[object(&e, &[("a", num(1.0))])]);
    let again = e.invoke("reactive", &[r.clone()]);
    log_line!(e, "rr", bo(strict_eq(&again, &r)), flag(&e, "isReactive", &again), flag(&e, "isReadonly", &again));
    check(&e, "r1b-reactive-of-readonly-unchanged");
}

/// `r1b-readonly-of-shallow-reactive`
#[test]
fn readonly_of_shallow_reactive() {
    let e = Env::new();
    let s = e.invoke("shallowReactive", &[object(&e, &[("inner", object(&e, &[("n", num(1.0))]))])]);
    let r = e.invoke("readonly", &[s.clone()]);
    let inner = e.get(&r, "inner");
    log_line!(e, "ro-of-shallow", flag(&e, "isReadonly", &inner), flag(&e, "isReactive", &inner));
    check(&e, "r1b-readonly-of-shallow-reactive");
}

/// `r1b-markraw-under-readonly`: a raw-marked object is not wrapped even by readonly.
#[test]
fn markraw_under_readonly() {
    let e = Env::new();
    let marked = object(&e, &[("k", num(1.0))]);
    e.invoke("markRaw", &[marked.clone()]);
    let r = e.invoke("readonly", &[object(&e, &[("m", marked)])]);
    let m = e.get(&r, "m");
    log_line!(e, "mark-ro", flag(&e, "isReadonly", &m), sv(&e, &e.get(&m, "k")));
    check(&e, "r1b-markraw-under-readonly");
}

/// `r1b-tostringtag-object-proxied-as-collection`: a plain object whose tag is "Map" takes the collection handler.
#[test]
fn tostringtag_object_proxied_as_collection() {
    let e = Env::new();
    let tagged = object(&e, &[("x", num(1.0))]);
    let tag_key = PropKey::Sym(e.rt.well_known(WellKnown::ToStringTag));
    e.rt.define_data(tagged.as_object().expect("object"), tag_key, st("Map"), Flags::new(false, false, true)).expect("tag");
    let p = e.invoke("reactive", &[tagged]);
    log_line!(e, "tag", flag(&e, "isReactive", &p), tag_of(&e, &p), sv(&e, &e.get(&p, "x")));
    check(&e, "r1b-tostringtag-object-proxied-as-collection");
}

/// `r1b-array-like-object-not-array`
#[test]
fn array_like_object_not_array() {
    let e = Env::new();
    let p = e.invoke("reactive", &[object(&e, &[("length", num(0.0)), ("0", st("a"))])]);
    log_line!(e, "arraylike", bo(e.rt.is_array(&p)), sv(&e, &e.get(&p, "length")), tag_of(&e, &p));
    check(&e, "r1b-array-like-object-not-array");
}

/// `r1b-numeric-and-string-keys-same`: `p[1]` and `p['1']` are the same property.
#[test]
fn numeric_and_string_keys_same() {
    let e = Env::new();
    let p = e.invoke("reactive", &[object(&e, &[])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let v = rt.get_str(&d[0], "1")?;
        log.borrow_mut().push(format!("k {}", js(rt, &v)));
        Ok(undef())
    });
    e.set(&p, "1", st("a"));
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&p, "1")));
    check(&e, "r1b-numeric-and-string-keys-same");
}

/// `r1b-gc-cross-references`: the proxies and raw objects of a cycle survive collections between the steps.
#[test]
fn gc_cross_references() {
    let e = Env::new();
    let a = object(&e, &[("name", st("a"))]);
    let b = object(&e, &[("name", st("b")), ("peer", a.clone())]);
    e.set(&a, "peer", b.clone());
    let pa = e.invoke("reactive", &[a.clone()]);
    let keep = [e.rt.root(&a), e.rt.root(&b), e.rt.root(&pa)];
    e.rt.collect();
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    let runner = run_effect(&e, vec![pa.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let peer = rt.get_str(&d[0], "peer")?;
        let peer_peer = rt.get_str(&peer, "peer")?;
        let name = rt.get_str(&peer_peer, "name")?;
        log.borrow_mut().push(format!("peer-name {}", js(rt, &name)));
        Ok(undef())
    });
    let keep_runner = e.rt.root(&runner);
    e.rt.collect();
    let peer = e.get(&pa, "peer");
    e.set(&peer, "name", st("B"));
    e.rt.collect();
    let peer_now = e.get(&pa, "peer");
    log_line!(e, "after", runs.get(), bo(strict_eq(&e.invoke("toRaw", &[peer_now.clone()]), &b)), sv(&e, &e.get(&peer_now, "name")));
    for id in keep {
        e.rt.unroot(id);
    }
    e.rt.unroot(keep_runner);
    check(&e, "r1b-gc-cross-references");
}

/// `r1b-gc-proxy-identity-across-collections`: the cached proxy of a raw object is the same value across collections.
#[test]
fn gc_proxy_identity_across_collections() {
    let e = Env::new();
    let raw = object(&e, &[("tag", num(1.0))]);
    let holder = e.invoke("reactive", &[new_map(&e, &[(st("k"), raw.clone())])]);
    let keep_holder = e.rt.root(&holder);
    let keep_raw = e.rt.root(&raw);
    e.rt.collect();
    let first = method(&e, &holder, "get", &[st("k")]);
    let list = e.invoke("reactive", &[array(&e, vec![raw.clone()])]);
    let keep_list = e.rt.root(&list);
    e.rt.collect();
    let in_list = e.get(&list, "0");
    let again = e.invoke("reactive", &[raw.clone()]);
    log_line!(e, "identity", bo(strict_eq(&first, &in_list)), bo(strict_eq(&e.invoke("toRaw", &[first.clone()]), &raw)), bo(strict_eq(&again, &first)));
    e.rt.unroot(keep_holder);
    e.rt.unroot(keep_raw);
    e.rt.unroot(keep_list);
    check(&e, "r1b-gc-proxy-identity-across-collections");
}

/// The coverage audit: every snippet of the R1b oracle has a Rust translation (its id appears in a test doc comment).
#[test]
fn every_snippet_has_a_translation() {
    let sources = [
        include_str!("r1b_proxy.rs"),
        include_str!("r1b_array.rs"),
        include_str!("r1b_collections.rs"),
        include_str!("r1b_misc.rs"),
        include_str!("r1b_refs.rs"),
        include_str!("r1b_effects.rs"),
    ];
    let ids = oracle_ids();
    assert!(ids.len() >= 150, "the R1b oracle has {} snippets (WP-R1b asks for at least 150)", ids.len());
    for id in ids {
        assert!(sources.iter().any(|s| s.contains(&format!("`{id}`"))), "snippet {id} has no Rust translation");
    }
}

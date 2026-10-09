//! Refs: ref, shallowRef, triggerRef, customRef, toRef, toRefs, toValue, unref, isRef. Each test is the Rust translation of
//! one snippet in tests/vue/rust/snippets.json (id in the doc comment).

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::{check, js, make_fn, Env};
use vn_value::{same_value, Flags, PropKey, Value};

fn num(x: f64) -> Value {
    Value::Number(x)
}

fn undef() -> Value {
    Value::Undefined
}

fn counter() -> Rc<Cell<i64>> {
    Rc::new(Cell::new(0))
}

/// `ref-primitive-get-set`
#[test]
fn ref_primitive_get_set() {
    let e = Env::new();
    let r = e.invoke("ref", &[num(1.0)]);
    e.log.borrow_mut().push(format!("init {}", js(&e.rt, &e.get(&r, "value"))));
    e.set(&r, "value", num(2.0));
    e.log.borrow_mut().push(format!("after {}", js(&e.rt, &e.get(&r, "value"))));
    e.set(&r, "value", num(2.0));
    e.log.borrow_mut().push(format!("again {}", js(&e.rt, &e.get(&r, "value"))));
    check(&e, "ref-primitive-get-set");
}

/// `ref-shallow-identity-nested-no-trigger`: a shallow ref tracks only its own value; nested writes are inert.
#[test]
fn ref_shallow_identity_nested_no_trigger() {
    let e = Env::new();
    let obj = e.rt.new_object();
    e.rt.define_data(obj, PropKey::from_str("count"), num(0.0), Flags::ALL).expect("count");
    let obj = Value::Object(obj);
    let s = e.invoke("shallowRef", &[obj.clone()]);
    e.log.borrow_mut().push(format!("identity {}", same_value(&e.get(&s, "value"), &obj)));
    let runs = counter();
    let (log, r2) = (e.log.clone(), runs.clone());
    let body = e.func("body", 0, vec![s.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        let v = rt.get_str(&rt.get_str(&d[0], "value")?, "count")?;
        log.borrow_mut().push(format!("read {}", js(rt, &v)));
        Ok(undef())
    });
    e.invoke("effect", &[body]);
    e.set(&obj, "count", num(1.0));
    e.log.borrow_mut().push(format!("runs-after-nested {}", runs.get()));
    e.invoke("triggerRef", &[s.clone()]);
    e.log.borrow_mut().push(format!("runs-after-trigger {}", runs.get()));
    e.set(&s, "value", obj.clone());
    e.log.borrow_mut().push(format!("runs-after-same {}", runs.get()));
    check(&e, "ref-shallow-identity-nested-no-trigger");
}

/// `ref-trigger-ref-same-value`: triggerRef reruns dependents even though the value did not change.
#[test]
fn ref_trigger_ref_same_value() {
    let e = Env::new();
    let r = e.invoke("ref", &[num(5.0)]);
    let runs = counter();
    let (log, r2) = (e.log.clone(), runs.clone());
    let body = e.func("body", 0, vec![r.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        log.borrow_mut().push(format!("read {}", js(rt, &rt.get_str(&d[0], "value")?)));
        Ok(undef())
    });
    e.invoke("effect", &[body]);
    e.invoke("triggerRef", &[r.clone()]);
    e.log.borrow_mut().push(format!("runs {}", runs.get()));
    check(&e, "ref-trigger-ref-same-value");
}

/// Builds the `customRef` factory of the snippets: the returned get/set count their calls and use track/trigger.
fn counting_factory(e: &Env, tracks: Rc<Cell<i64>>, triggers: Rc<Cell<i64>>, stored: Rc<Cell<f64>>) -> Value {
    let table = e.table();
    let (t2, g2, s2) = (tracks.clone(), triggers.clone(), stored.clone());
    let table2 = table.clone();
    make_fn(&e.rt, &table, "factory", 2, vec![], move |rt, _, args, _| {
        let track = args[0].clone();
        let trigger = args[1].clone();
        let t3 = t2.clone();
        let s3 = s2.clone();
        let getter = make_fn(rt, &table2, "get", 0, vec![track.clone()], move |rt, _, _, d| {
            rt.call(&d[0], &undef(), &[])?;
            t3.set(t3.get() + 1);
            Ok(num(s3.get()))
        });
        let g3 = g2.clone();
        let s4 = s2.clone();
        let setter = make_fn(rt, &table2, "set", 1, vec![trigger], move |rt, _, args, d| {
            s4.set(rt.to_number(&args[0])?);
            g3.set(g3.get() + 1);
            rt.call(&d[0], &undef(), &[])?;
            Ok(undef())
        });
        let out = rt.new_object();
        rt.define_data(out, PropKey::from_str("get"), getter, Flags::ALL)?;
        rt.define_data(out, PropKey::from_str("set"), setter, Flags::ALL)?;
        Ok(Value::Object(out))
    })
}

/// `ref-custom-track-trigger-counts`
#[test]
fn ref_custom_track_trigger_counts() {
    let e = Env::new();
    let (tracks, triggers, stored) = (counter(), counter(), Rc::new(Cell::new(0.0)));
    let factory = counting_factory(&e, tracks.clone(), triggers.clone(), stored.clone());
    let r = e.invoke("customRef", &[factory]);
    let runs = counter();
    let (log, r2) = (e.log.clone(), runs.clone());
    let body = e.func("body", 0, vec![r.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        log.borrow_mut().push(format!("read {}", js(rt, &rt.get_str(&d[0], "value")?)));
        Ok(undef())
    });
    e.invoke("effect", &[body]);
    e.set(&r, "value", num(3.0));
    e.log.borrow_mut().push(format!("counts {} {} {}", tracks.get(), triggers.get(), runs.get()));
    check(&e, "ref-custom-track-trigger-counts");
}

/// `ref-customref-set-always-triggers`: the custom setter triggers on every write, even of an equal value.
#[test]
fn ref_customref_set_always_triggers() {
    let e = Env::new();
    let writes = counter();
    let stored = Rc::new(std::cell::RefCell::new(Env::text("a")));
    let table = e.table();
    let (w2, st) = (writes.clone(), stored.clone());
    let factory = make_fn(&e.rt, &table.clone(), "factory", 2, vec![], move |rt, _, args, _| {
        let track = args[0].clone();
        let trigger = args[1].clone();
        let st1 = st.clone();
        let getter = make_fn(rt, &table, "get", 0, vec![track], move |rt, _, _, d| {
            rt.call(&d[0], &undef(), &[])?;
            Ok(st1.borrow().clone())
        });
        let st2 = st.clone();
        let w3 = w2.clone();
        let setter = make_fn(rt, &table, "set", 1, vec![trigger], move |rt, _, args, d| {
            w3.set(w3.get() + 1);
            *st2.borrow_mut() = args[0].clone();
            rt.call(&d[0], &undef(), &[])?;
            Ok(undef())
        });
        let out = rt.new_object();
        rt.define_data(out, PropKey::from_str("get"), getter, Flags::ALL)?;
        rt.define_data(out, PropKey::from_str("set"), setter, Flags::ALL)?;
        Ok(Value::Object(out))
    });
    let r = e.invoke("customRef", &[factory]);
    let runs = counter();
    let (log, r2) = (e.log.clone(), runs.clone());
    let body = e.func("body", 0, vec![r.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        log.borrow_mut().push(format!("read {}", js(rt, &rt.get_str(&d[0], "value")?)));
        Ok(undef())
    });
    e.invoke("effect", &[body]);
    e.set(&r, "value", Env::text("b"));
    e.set(&r, "value", Env::text("b"));
    e.log.borrow_mut().push(format!("writes {} runs {}", writes.get(), runs.get()));
    check(&e, "ref-customref-set-always-triggers");
}

/// `ref-toref-property-alias`: a property ref reads and writes the property of the object.
#[test]
fn ref_toref_property_alias() {
    let e = Env::new();
    let obj = e.rt.new_object();
    e.rt.define_data(obj, PropKey::from_str("x"), num(1.0), Flags::ALL).expect("x");
    let obj = Value::Object(obj);
    let rx = e.invoke("toRef", &[obj.clone(), Env::text("x")]);
    e.log.borrow_mut().push(format!("read {}", js(&e.rt, &e.get(&rx, "value"))));
    e.set(&rx, "value", num(5.0));
    e.log.borrow_mut().push(format!("obj {}", js(&e.rt, &e.get(&obj, "x"))));
    e.set(&obj, "x", num(9.0));
    e.log.borrow_mut().push(format!("via-ref {}", js(&e.rt, &e.get(&rx, "value"))));
    e.log.borrow_mut().push(format!("isref {}", js(&e.rt, &e.invoke("isRef", &[rx.clone()]))));
    check(&e, "ref-toref-property-alias");
}

/// `ref-toref-getter-readonly-and-tovalue`: a getter ref is read-only (strict write throws); toValue unwraps.
#[test]
fn ref_toref_getter_readonly_and_tovalue() {
    let e = Env::new();
    let base = Rc::new(Cell::new(2.0));
    let b2 = base.clone();
    let getter = e.func("getter", 0, vec![], move |_, _, _, _| Ok(num(b2.get() * 10.0)));
    let g = e.invoke("toRef", &[getter]);
    e.log.borrow_mut().push(format!("get {}", js(&e.rt, &e.get(&g, "value"))));
    base.set(3.0);
    e.log.borrow_mut().push(format!("get2 {}", js(&e.rt, &e.get(&g, "value"))));
    match e.try_set(&g, "value", num(1.0)) {
        Ok(()) => e.log.borrow_mut().push("wrote".to_string()),
        Err(t) => e.log.borrow_mut().push(format!("threw {}", js(&e.rt, &e.get(&t.0, "name")))),
    }
    let seven = e.func("seven", 0, vec![], |_, _, _, _| Ok(num(7.0)));
    let a = e.invoke("toValue", &[seven]);
    let b = e.invoke("toValue", &[g.clone()]);
    let c = e.invoke("toValue", &[num(4.0)]);
    e.log.borrow_mut().push(format!("toValue {} {} {}", js(&e.rt, &a), js(&e.rt, &b), js(&e.rt, &c)));
    check(&e, "ref-toref-getter-readonly-and-tovalue");
}

/// `ref-torefs-destructure`: toRefs gives one property ref per key, aliasing the object.
#[test]
fn ref_torefs_destructure() {
    let e = Env::new();
    let obj = e.rt.new_object();
    e.rt.define_data(obj, PropKey::from_str("a"), num(1.0), Flags::ALL).expect("a");
    e.rt.define_data(obj, PropKey::from_str("b"), Env::text("two"), Flags::ALL).expect("b");
    let obj = Value::Object(obj);
    let refs = e.invoke("toRefs", &[obj.clone()]);
    let mut keys = Vec::new();
    for k in e.rt.own_keys(refs.as_object().expect("refs")).expect("keys") {
        if let PropKey::Str(s) = k {
            keys.push(s.to_rust_string_lossy());
        }
    }
    e.log.borrow_mut().push(format!("keys {}", keys.join(",")));
    let (ra, rb) = (e.get(&refs, "a"), e.get(&refs, "b"));
    e.log.borrow_mut().push(format!(
        "isRef {} {}",
        js(&e.rt, &e.invoke("isRef", &[ra.clone()])),
        js(&e.rt, &e.invoke("isRef", &[rb.clone()]))
    ));
    e.set(&ra, "value", num(10.0));
    e.log.borrow_mut().push(format!("obj.a {}", js(&e.rt, &e.get(&obj, "a"))));
    e.set(&obj, "b", Env::text("three"));
    e.log.borrow_mut().push(format!("refs.b {}", js(&e.rt, &e.get(&rb, "value"))));
    e.log.borrow_mut().push(format!("plain {}", js(&e.rt, &e.invoke("isRef", &[e.get(&obj, "a")]))));
    check(&e, "ref-torefs-destructure");
}

/// `ref-isref-unref-same-ref`: isRef answers for refs only; ref(ref) returns the same ref.
#[test]
fn ref_isref_unref_same_ref() {
    let e = Env::new();
    let r = e.invoke("ref", &[num(1.0)]);
    let sh = e.invoke("shallowRef", &[num(2.0)]);
    let plain = Value::Object(e.rt.new_object());
    let is = |v: Value| js(&e.rt, &e.invoke("isRef", &[v]));
    let line = [
        is(r.clone()),
        is(num(1.0)),
        is(Value::Null),
        is(plain),
        is(sh.clone()),
    ];
    e.log.borrow_mut().push(format!("isRef {}", line.join(" ")));
    let shs = e.invoke("shallowRef", &[Env::text("s")]);
    let un = [
        js(&e.rt, &e.invoke("unref", &[r.clone()])),
        js(&e.rt, &e.invoke("unref", &[num(5.0)])),
        js(&e.rt, &e.invoke("unref", &[shs])),
    ];
    e.log.borrow_mut().push(format!("unref {}", un.join(" ")));
    let same = e.invoke("ref", &[r.clone()]);
    e.log.borrow_mut().push(format!("same-ref {}", same_value(&same, &r)));
    check(&e, "ref-isref-unref-same-ref");
}

/// `ref-dep-per-instance`: each ref has its own dep; writes to one do not run effects of another.
#[test]
fn ref_dep_per_instance() {
    let e = Env::new();
    let a = e.invoke("ref", &[num(0.0)]);
    let b = e.invoke("ref", &[num(0.0)]);
    let runs = counter();
    let (log, r2) = (e.log.clone(), runs.clone());
    let body = e.func("body", 0, vec![a.clone()], move |rt, _, _, d| {
        r2.set(r2.get() + 1);
        log.borrow_mut().push(format!("read-a {}", js(rt, &rt.get_str(&d[0], "value")?)));
        Ok(undef())
    });
    e.invoke("effect", &[body]);
    e.set(&b, "value", num(1.0));
    e.log.borrow_mut().push(format!("after-b {}", runs.get()));
    e.set(&a, "value", num(1.0));
    e.log.borrow_mut().push(format!("after-a {}", runs.get()));
    check(&e, "ref-dep-per-instance");
}

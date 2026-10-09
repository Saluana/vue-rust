//! Exotic-object hooks (the proxy-like extension point the reactivity port uses) and the
//! `Symbol.toPrimitive` coercion path. Each hook either handles a key or returns `Ordinary`, which must fall
//! back to the object's storage. Expected values are written from ECMA-262 10.5 and 7.1.1.

use std::cell::Cell;
use std::rc::Rc;

use vn_value::{Call, Exotic, Flags, Hook, ObjRef, Prop, PropKey, Res, Rt, Tracer, Value, WellKnown};

fn key(s: &str) -> PropKey {
    PropKey::from_str(s)
}

fn num(n: f64) -> Value {
    Value::Number(n)
}

fn ok<T>(r: Res<T>) -> T {
    match r {
        Ok(v) => v,
        Err(_) => panic!("unexpected JS throw"),
    }
}

fn throws<T: std::fmt::Debug>(r: Res<T>) -> vn_value::Throw {
    match r {
        Err(t) => t,
        Ok(v) => panic!("expected a JS throw, got {v:?}"),
    }
}

fn message(rt: &Rt, v: &Value) -> String {
    match rt.get_str(v, "message") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => String::from("<none>"),
    }
}

fn is_key(k: &PropKey, s: &str) -> bool {
    *k == key(s)
}

/// Handles a few keys and defers everything else to the ordinary algorithm.
struct Magic {
    hooks_hit: Cell<u32>,
}

impl Exotic for Magic {
    fn trace(&self, _t: &mut Tracer) {}

    fn get(&self, _rt: &Rt, _obj: ObjRef, k: &PropKey, _receiver: &Value) -> Res<Hook<Value>> {
        if is_key(k, "magic") {
            self.hooks_hit.set(self.hooks_hit.get() + 1);
            return Ok(Hook::Handled(num(42.0)));
        }
        Ok(Hook::Ordinary)
    }

    fn has(&self, _rt: &Rt, _obj: ObjRef, k: &PropKey) -> Res<Hook<bool>> {
        Ok(if is_key(k, "magic") { Hook::Handled(true) } else { Hook::Ordinary })
    }

    fn own_keys(&self, _rt: &Rt, _obj: ObjRef) -> Res<Hook<Vec<PropKey>>> {
        Ok(Hook::Handled(vec![key("virtual")]))
    }

    fn get_own_property(&self, _rt: &Rt, _obj: ObjRef, k: &PropKey) -> Res<Hook<Option<Prop>>> {
        if is_key(k, "virtual") {
            return Ok(Hook::Handled(Some(Prop::Data { value: num(1.0), flags: Flags::ALL })));
        }
        Ok(Hook::Ordinary)
    }

    fn delete(&self, _rt: &Rt, _obj: ObjRef, k: &PropKey) -> Res<Hook<bool>> {
        Ok(if is_key(k, "virtual") { Hook::Handled(false) } else { Hook::Ordinary })
    }

    fn define(&self, _rt: &Rt, _obj: ObjRef, k: &PropKey, _p: &Prop) -> Res<Hook<bool>> {
        Ok(if is_key(k, "blocked") { Hook::Handled(false) } else { Hook::Ordinary })
    }

    fn set(&self, _rt: &Rt, _obj: ObjRef, k: &PropKey, _v: &Value, _r: &Value) -> Res<Hook<bool>> {
        Ok(if is_key(k, "readonly") { Hook::Handled(false) } else { Hook::Ordinary })
    }
}

fn magic(rt: &Rt) -> ObjRef {
    let hooks: Rc<dyn Exotic> = Rc::new(Magic { hooks_hit: Cell::new(0) });
    rt.new_exotic_object(Some(rt.intrinsic(vn_value::Intrinsic::ObjectPrototype)), hooks)
}

#[test]
fn a_get_hook_overrides_stored_properties() {
    let rt = Rt::new();
    let o = magic(&rt);
    ok(rt.define_data(o, key("magic"), num(1.0), Flags::ALL));
    assert_eq!(ok(rt.get(o, &key("magic"), &Value::Object(o))), num(42.0));
}

#[test]
fn an_ordinary_result_falls_back_to_storage() {
    let rt = Rt::new();
    let o = magic(&rt);
    ok(rt.define_data(o, key("plain"), num(5.0), Flags::ALL));
    assert_eq!(ok(rt.get(o, &key("plain"), &Value::Object(o))), num(5.0));
    assert_eq!(ok(rt.get(o, &key("missing"), &Value::Object(o))), Value::Undefined);
}

#[test]
fn a_has_hook_answers_has_property_without_storage() {
    let rt = Rt::new();
    let o = magic(&rt);
    assert!(ok(rt.has_property(o, &key("magic"))));
    assert!(!ok(rt.has_property(o, &key("nothing"))));
}

#[test]
fn an_own_keys_hook_replaces_the_stored_keys() {
    let rt = Rt::new();
    let o = magic(&rt);
    ok(rt.define_data(o, key("stored"), num(1.0), Flags::ALL));
    assert_eq!(ok(rt.own_keys(o)), vec![key("virtual")]);
}

#[test]
fn get_own_property_hook_is_used_by_get_and_has_own_property() {
    let rt = Rt::new();
    let o = magic(&rt);
    assert_eq!(ok(rt.get(o, &key("virtual"), &Value::Object(o))), num(1.0));
    assert!(ok(rt.has_own_property(o, &key("virtual"))));
}

#[test]
fn a_delete_hook_can_refuse_deletion() {
    let rt = Rt::new();
    let o = magic(&rt);
    assert!(!ok(rt.delete(o, &key("virtual"))));
}

#[test]
fn a_define_hook_can_refuse_definitions_and_ordinary_keys_still_work() {
    let rt = Rt::new();
    let o = magic(&rt);
    assert!(!ok(rt.define_data(o, key("blocked"), num(1.0), Flags::ALL)));
    assert!(ok(rt.define_data(o, key("free"), num(2.0), Flags::ALL)));
    assert_eq!(ok(rt.get(o, &key("free"), &Value::Object(o))), num(2.0));
}

#[test]
fn a_set_hook_can_refuse_assignment_and_strict_mode_throws() {
    let rt = Rt::new();
    let o = magic(&rt);
    assert!(!ok(rt.set(o, &key("readonly"), num(3.0), &Value::Object(o))));
    let t = throws(rt.set_strict(o, &key("readonly"), num(3.0)));
    assert_eq!(message(&rt, &t.0), "Cannot assign to read only property 'readonly' of object");
}

struct Thrower;

impl Exotic for Thrower {
    fn trace(&self, _t: &mut Tracer) {}

    fn get(&self, rt: &Rt, _obj: ObjRef, _k: &PropKey, _r: &Value) -> Res<Hook<Value>> {
        Err(rt.throw_range_error("hook failed"))
    }
}

#[test]
fn an_error_from_a_hook_propagates_as_a_throw() {
    let rt = Rt::new();
    let hooks: Rc<dyn Exotic> = Rc::new(Thrower);
    let o = rt.new_exotic_object(None, hooks);
    let t = throws(rt.get(o, &key("anything"), &Value::Object(o)));
    assert_eq!(message(&rt, &t.0), "hook failed");
}

struct Echo;

impl Exotic for Echo {
    fn trace(&self, _t: &mut Tracer) {}

    fn get(&self, _rt: &Rt, _obj: ObjRef, _k: &PropKey, receiver: &Value) -> Res<Hook<Value>> {
        Ok(Hook::Handled(receiver.clone()))
    }
}

#[test]
fn a_get_hook_sees_the_receiver() {
    let rt = Rt::new();
    let hooks: Rc<dyn Exotic> = Rc::new(Echo);
    let o = rt.new_exotic_object(None, hooks);
    let child = rt.new_object_with_proto(Some(o));
    assert_eq!(ok(rt.get(child, &key("x"), &Value::Object(child))), Value::Object(child));
}

fn to_primitive_by_hint(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let _ = rt;
    match c.arg(0) {
        Value::String(s) if s.to_rust_string_lossy() == "number" => Ok(num(7.0)),
        Value::String(s) if s.to_rust_string_lossy() == "string" => Ok(Value::str("s")),
        _ => Ok(Value::str("d")),
    }
}

#[test]
fn symbol_to_primitive_receives_the_hint() {
    let rt = Rt::new();
    let o = rt.new_object();
    let f = rt.new_native_function(to_primitive_by_hint, "toPrimitive", 1, vec![]);
    let sym = rt.well_known(WellKnown::ToPrimitive);
    ok(rt.define_data(o, PropKey::Sym(sym), Value::Object(f), Flags::HIDDEN));
    assert_eq!(ok(rt.to_number(&Value::Object(o))), 7.0);
    assert_eq!(ok(rt.to_string(&Value::Object(o))).to_rust_string_lossy(), "s");
    assert_eq!(ok(rt.to_primitive(&Value::Object(o), vn_value::Hint::Default)), Value::str("d"));
}

fn returns_object(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Object(rt.new_object()))
}

#[test]
fn symbol_to_primitive_returning_an_object_is_a_type_error() {
    let rt = Rt::new();
    let o = rt.new_object();
    let f = rt.new_native_function(returns_object, "toPrimitive", 1, vec![]);
    ok(rt.define_data(o, PropKey::Sym(rt.well_known(WellKnown::ToPrimitive)), Value::Object(f), Flags::HIDDEN));
    let t = throws(rt.to_number(&Value::Object(o)));
    assert_eq!(message(&rt, &t.0), "Cannot convert object to primitive value");
}

#[test]
fn a_non_callable_symbol_to_primitive_is_a_type_error() {
    let rt = Rt::new();
    let o = rt.new_object();
    ok(rt.define_data(o, PropKey::Sym(rt.well_known(WellKnown::ToPrimitive)), num(1.0), Flags::HIDDEN));
    let t = throws(rt.to_primitive(&Value::Object(o), vn_value::Hint::Number));
    assert_eq!(message(&rt, &t.0), "Symbol.toPrimitive is not a function");
}

#[test]
fn an_object_with_no_conversion_methods_cannot_become_a_primitive() {
    let rt = Rt::new();
    let bare = rt.new_object_with_proto(None);
    let t = throws(rt.to_string(&Value::Object(bare)));
    assert_eq!(message(&rt, &t.0), "Cannot convert object to primitive value");
}

//! Integrity levels through exotic hooks (WP-R1b follow-up). An exotic object whose hooks forward to a target takes its
//! preventExtensions, extensibility, keys and properties from the target, so freezing or sealing it freezes or seals the
//! target. A refused preventExtensions refuses freeze, a throwing hook throws, and ordinary objects keep their behaviour.
//! Expected values follow ECMA-262 10.1.4 (preventExtensions, [[IsExtensible]]) and 7.3.14-16 (SetIntegrityLevel,
//! TestIntegrityLevel).

use std::rc::Rc;

use vn_value::{Exotic, Flags, Hook, ObjRef, Prop, PropKey, Res, Rt, Tracer, Value};

/// Forwards the integrity and property hooks to `target`.
struct Forward {
    target: ObjRef,
}

impl Exotic for Forward {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.target);
    }

    fn get_own_property(&self, rt: &Rt, _obj: ObjRef, key: &PropKey) -> Res<Hook<Option<Prop>>> {
        Ok(Hook::Handled(rt.get_own_property(self.target, key)?))
    }

    fn own_keys(&self, rt: &Rt, _obj: ObjRef) -> Res<Hook<Vec<PropKey>>> {
        Ok(Hook::Handled(rt.own_keys(self.target)?))
    }

    fn define(&self, rt: &Rt, _obj: ObjRef, key: &PropKey, prop: &Prop) -> Res<Hook<bool>> {
        Ok(Hook::Handled(rt.define_own(self.target, key, prop.clone())?))
    }

    fn prevent_extensions(&self, rt: &Rt, _obj: ObjRef) -> Res<Hook<bool>> {
        Ok(Hook::Handled(rt.prevent_extensions(self.target)?))
    }

    fn is_extensible(&self, rt: &Rt, _obj: ObjRef) -> Res<Hook<bool>> {
        Ok(Hook::Handled(rt.is_extensible(self.target)?))
    }
}

/// Refuses preventExtensions and leaves every other hook ordinary.
struct Refuse;

impl Exotic for Refuse {
    fn trace(&self, _t: &mut Tracer) {}

    fn prevent_extensions(&self, _rt: &Rt, _obj: ObjRef) -> Res<Hook<bool>> {
        Ok(Hook::Handled(false))
    }
}

/// Throws from `is_extensible`.
struct Throws;

impl Exotic for Throws {
    fn trace(&self, _t: &mut Tracer) {}

    fn is_extensible(&self, rt: &Rt, _obj: ObjRef) -> Res<Hook<bool>> {
        Err(rt.throw_type_error("is_extensible refused"))
    }
}

fn key(s: &str) -> PropKey {
    PropKey::from_str(s)
}

/// An ordinary object with one writable, configurable data property `a = 1`.
fn target_with_a(rt: &Rt) -> ObjRef {
    let t = rt.new_object();
    rt.define_data(t, key("a"), Value::Number(1.0), Flags::ALL).expect("data property");
    t
}

fn writable_of(rt: &Rt, o: ObjRef, k: &str) -> bool {
    match rt.get_own_property(o, &key(k)).expect("own property") {
        Some(p) => p.flags().writable,
        None => panic!("no own property {k}"),
    }
}

#[test]
fn freeze_through_exotic_freezes_target() {
    let rt = Rt::new();
    let target = target_with_a(&rt);
    let exotic = rt.new_exotic_object(None, Rc::new(Forward { target }));
    assert!(rt.freeze(exotic).expect("freeze"));
    assert!(!rt.is_extensible(target).expect("isExtensible"));
    assert!(rt.is_frozen(target).expect("isFrozen"));
    assert!(!writable_of(&rt, target, "a"));
    assert!(!rt.is_extensible(exotic).expect("isExtensible"));
    assert!(rt.is_frozen(exotic).expect("isFrozen"));
}

#[test]
fn seal_through_exotic_seals_target() {
    let rt = Rt::new();
    let target = target_with_a(&rt);
    let exotic = rt.new_exotic_object(None, Rc::new(Forward { target }));
    assert!(rt.seal(exotic).expect("seal"));
    assert!(rt.is_sealed(target).expect("isSealed"));
    assert!(!rt.is_frozen(target).expect("isFrozen"));
    assert!(writable_of(&rt, target, "a"));
}

#[test]
fn prevent_extensions_through_exotic_reaches_target() {
    let rt = Rt::new();
    let target = target_with_a(&rt);
    let exotic = rt.new_exotic_object(None, Rc::new(Forward { target }));
    assert!(rt.prevent_extensions(exotic).expect("preventExtensions"));
    assert!(!rt.is_extensible(target).expect("isExtensible"));
    assert!(!rt.is_extensible(exotic).expect("isExtensible"));
    let added = rt.define_own(exotic, &key("z"), Prop::Data { value: Value::Number(2.0), flags: Flags::ALL }).expect("define");
    assert!(!added, "a non-extensible target refuses a new property");
    assert!(!rt.has_own_property(target, &key("z")).expect("has own"));
}

#[test]
fn refused_prevent_extensions_refuses_freeze() {
    let rt = Rt::new();
    let refuse = rt.new_exotic_object(None, Rc::new(Refuse));
    assert!(!rt.prevent_extensions(refuse).expect("preventExtensions"));
    assert!(!rt.freeze(refuse).expect("freeze"), "SetIntegrityLevel stops when [[PreventExtensions]] is false");
    assert!(rt.is_extensible(refuse).expect("isExtensible"), "the refused object stays extensible");
}

#[test]
fn throwing_hook_propagates() {
    let rt = Rt::new();
    let throws = rt.new_exotic_object(None, Rc::new(Throws));
    assert!(rt.is_extensible(throws).is_err());
    assert!(rt.is_frozen(throws).is_err(), "TestIntegrityLevel reads [[IsExtensible]] first");
}

#[test]
fn ordinary_objects_unchanged() {
    let rt = Rt::new();
    let fresh = rt.new_object();
    assert!(rt.is_extensible(fresh).expect("isExtensible"));
    assert!(!rt.is_frozen(fresh).expect("isFrozen"), "an extensible object is not frozen");
    assert!(rt.prevent_extensions(fresh).expect("preventExtensions"));
    assert!(!rt.is_extensible(fresh).expect("isExtensible"));
    assert!(rt.is_frozen(fresh).expect("isFrozen"), "an empty non-extensible object is frozen vacuously");

    let p = target_with_a(&rt);
    assert!(rt.freeze(p).expect("freeze"));
    assert!(rt.is_frozen(p).expect("isFrozen"));
    assert!(!writable_of(&rt, p, "a"));
}

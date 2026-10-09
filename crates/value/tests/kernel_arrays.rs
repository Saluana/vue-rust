//! Kernel arrays: dense elements with holes, the exotic `length`, extension, integrity levels and the
//! documented dense limit. Expected values are V8's (checked with Node 26) or follow ECMA-262 10.4.2.
//! Where the kernel refuses a case the test says so (see ABI.md).

use vn_value::{Flags, Prop, PropKey, Res, Rt, Throw, Value, MAX_DENSE_LEN};

fn key(s: &str) -> PropKey {
    PropKey::from_str(s)
}

fn idx(i: u32) -> PropKey {
    PropKey::Index(i)
}

fn ok<T>(rt: &Rt, r: Res<T>) -> T {
    match r {
        Ok(v) => v,
        Err(t) => panic!("unexpected JS throw: {}", describe(rt, &t)),
    }
}

fn describe(rt: &Rt, t: &Throw) -> String {
    let get = |n: &str| match rt.get_str(&t.0, n) {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => String::from("<none>"),
    };
    format!("{}: {}", get("name"), get("message"))
}

fn throws<T: std::fmt::Debug>(r: Res<T>) -> Throw {
    match r {
        Err(t) => t,
        Ok(v) => panic!("expected a JS throw, got {v:?}"),
    }
}

fn message(rt: &Rt, t: &Throw) -> String {
    match rt.get_str(&t.0, "message") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => String::from("<none>"),
    }
}

fn num(n: f64) -> Value {
    Value::Number(n)
}

fn arr(rt: &Rt, items: &[f64]) -> (vn_value::ObjRef, Value) {
    let o = rt.new_array(items.iter().map(|&n| num(n)).collect());
    (o, Value::Object(o))
}

fn texts(rt: &Rt, keys: &[PropKey]) -> Vec<String> {
    let _ = rt;
    keys.iter()
        .map(|k| match k {
            PropKey::Index(i) => i.to_string(),
            PropKey::Str(s) => s.to_rust_string_lossy(),
            PropKey::Sym(_) => String::from("<sym>"),
        })
        .collect()
}

fn length_of(rt: &Rt, v: &Value) -> f64 {
    match ok(rt, rt.get_str(v, "length")) {
        Value::Number(n) => n,
        other => panic!("length is not a number: {other:?}"),
    }
}

#[test]
fn length_is_an_own_non_enumerable_non_configurable_data_property() {
    let rt = Rt::new();
    let (o, _) = arr(&rt, &[1.0, 2.0]);
    let p = ok(&rt, rt.get_own_property(o, &key("length"))).expect("length exists");
    assert_eq!(p, Prop::Data { value: num(2.0), flags: Flags::new(true, false, false) });
}

#[test]
fn own_keys_are_indices_then_length_then_named_properties() {
    // V8: Reflect.ownKeys(a) is ['0', '1', 'length', 'x'].
    let rt = Rt::new();
    let (o, _) = arr(&rt, &[1.0, 2.0]);
    ok(&rt, rt.define_data(o, key("x"), num(0.0), Flags::ALL));
    let keys = ok(&rt, rt.own_keys(o));
    assert_eq!(texts(&rt, &keys), ["0", "1", "length", "x"]);
}

#[test]
fn length_truncation_drops_trailing_elements() {
    // V8: a = [1,2,3,4]; a.length = 2 gives [1,2].
    let rt = Rt::new();
    let (_, v) = arr(&rt, &[1.0, 2.0, 3.0, 4.0]);
    assert!(ok(&rt, rt.set_str(&v, "length", num(2.0))));
    assert_eq!(length_of(&rt, &v), 2.0);
    assert_eq!(ok(&rt, rt.get_v(&v, &idx(1))), num(2.0));
    assert_eq!(ok(&rt, rt.get_v(&v, &idx(2))), Value::Undefined);
    let keys = ok(&rt, rt.own_keys(v.as_object().unwrap()));
    assert_eq!(texts(&rt, &keys), ["0", "1", "length"]);
}

#[test]
fn length_extension_creates_holes_that_are_not_own_properties() {
    // V8: b = [1]; b.length = 4 gives length 4, `2 in b` false, keys 0 length.
    let rt = Rt::new();
    let (o, v) = arr(&rt, &[1.0]);
    assert!(ok(&rt, rt.set_str(&v, "length", num(4.0))));
    assert_eq!(length_of(&rt, &v), 4.0);
    assert!(!ok(&rt, rt.has_own_property(o, &idx(2))));
    assert!(!ok(&rt, rt.has_property(o, &idx(2))));
    assert_eq!(texts(&rt, &ok(&rt, rt.own_keys(o))), ["0", "length"]);
}

#[test]
fn writing_past_the_end_extends_with_holes() {
    // V8: g = []; g[3] = 1 gives length 4 and keys 3 length.
    let rt = Rt::new();
    let (o, v) = arr(&rt, &[]);
    assert!(ok(&rt, rt.set_v(&v, &idx(3), num(1.0))));
    assert_eq!(length_of(&rt, &v), 4.0);
    assert_eq!(texts(&rt, &ok(&rt, rt.own_keys(o))), ["3", "length"]);
}

#[test]
fn a_hole_reads_through_to_the_prototype_chain() {
    // V8: Array.prototype[1] = "proto"; [1,,3][1] is "proto".
    let rt = Rt::new();
    let ap = rt.intrinsic(vn_value::Intrinsic::ArrayPrototype);
    ok(&rt, rt.define_data(ap, idx(1), Value::str("proto"), Flags::ALL));
    let (_, v) = arr(&rt, &[]);
    ok(&rt, rt.set_v(&v, &idx(2), num(3.0)));
    assert_eq!(ok(&rt, rt.get_v(&v, &idx(1))), Value::str("proto"));
    assert!(ok(&rt, rt.delete(ap, &idx(1))));
}

#[test]
fn length_rejects_negative_and_fractional_values_with_range_error() {
    // V8: [1].length = -1 and = 1.5 both throw RangeError "Invalid array length".
    let rt = Rt::new();
    let (_, v) = arr(&rt, &[1.0]);
    let t = throws(rt.set_v(&v, &key("length"), num(-1.0)));
    assert_eq!(message(&rt, &t), "Invalid array length");
    assert_eq!(rt.get_str(&t.0, "name").ok(), Some(Value::str("RangeError")));
    let t = throws(rt.set_v(&v, &key("length"), num(1.5)));
    assert_eq!(message(&rt, &t), "Invalid array length");
    assert_eq!(length_of(&rt, &v), 1.0, "a refused length leaves the array unchanged");
}

#[test]
fn length_coerces_strings_like_to_number() {
    // V8: a.length = "3" sets length 3.
    let rt = Rt::new();
    let (_, v) = arr(&rt, &[1.0]);
    assert!(ok(&rt, rt.set_str(&v, "length", Value::str("3"))));
    assert_eq!(length_of(&rt, &v), 3.0);
}

#[test]
fn frozen_array_refuses_element_writes_and_new_indices() {
    // V8: f = Object.freeze([1,2]); f[0] = 9 and f[5] = 1 change nothing.
    let rt = Rt::new();
    let (o, v) = arr(&rt, &[1.0, 2.0]);
    assert!(ok(&rt, rt.freeze(o)));
    assert!(ok(&rt, rt.is_frozen(o)));
    assert!(!ok(&rt, rt.set_v(&v, &idx(0), num(9.0))));
    assert!(!ok(&rt, rt.set_v(&v, &idx(5), num(1.0))));
    assert_eq!(ok(&rt, rt.get_v(&v, &idx(0))), num(1.0));
    assert_eq!(length_of(&rt, &v), 2.0);
    assert!(!ok(&rt, rt.has_own_property(o, &idx(5))));
}

#[test]
fn frozen_array_length_is_read_only() {
    let rt = Rt::new();
    let (o, v) = arr(&rt, &[1.0, 2.0]);
    ok(&rt, rt.freeze(o));
    assert!(!ok(&rt, rt.set_v(&v, &key("length"), num(0.0))));
    let t = throws(rt.set_v_strict(&v, &key("length"), num(0.0)));
    assert_eq!(message(&rt, &t), "Cannot assign to read only property 'length' of object");
    let p = ok(&rt, rt.get_own_property(o, &key("length"))).unwrap();
    assert_eq!(p.flags().writable, false);
}

#[test]
fn sealed_array_keeps_writable_values_and_refuses_truncation_and_delete() {
    // V8: s = Object.seal([1,2,3]); s.length = 0 leaves length 3; delete s[2] fails.
    let rt = Rt::new();
    let (o, v) = arr(&rt, &[1.0, 2.0, 3.0]);
    assert!(ok(&rt, rt.seal(o)));
    assert!(ok(&rt, rt.set_v(&v, &idx(0), num(7.0))), "sealed elements stay writable");
    assert_eq!(ok(&rt, rt.get_v(&v, &idx(0))), num(7.0));
    assert!(!ok(&rt, rt.delete(o, &idx(2))));
    assert!(!ok(&rt, rt.set_v(&v, &key("length"), num(0.0))));
    assert_eq!(length_of(&rt, &v), 3.0);
    assert!(!ok(&rt, rt.set_v(&v, &idx(3), num(4.0))), "sealed arrays cannot grow");
}

#[test]
fn delete_of_an_element_leaves_a_hole_and_keeps_length() {
    // V8: d = [1,2,3]; delete d[1] gives length 3, `1 in d` false.
    let rt = Rt::new();
    let (o, v) = arr(&rt, &[1.0, 2.0, 3.0]);
    assert!(ok(&rt, rt.delete(o, &idx(1))));
    assert_eq!(length_of(&rt, &v), 3.0);
    assert!(!ok(&rt, rt.has_own_property(o, &idx(1))));
    assert_eq!(texts(&rt, &ok(&rt, rt.own_keys(o))), ["0", "2", "length"]);
}

#[test]
fn non_writable_length_blocks_growth_but_not_reads() {
    let rt = Rt::new();
    let (o, v) = arr(&rt, &[1.0]);
    assert!(ok(&rt, rt.define_data(o, key("length"), num(1.0), Flags::NONE)));
    assert!(!ok(&rt, rt.set_v(&v, &idx(1), num(2.0))));
    assert_eq!(ok(&rt, rt.get_v(&v, &idx(0))), num(1.0));
}

#[test]
fn length_define_refuses_enumerable_or_configurable_attributes() {
    let rt = Rt::new();
    let (o, _) = arr(&rt, &[1.0]);
    assert!(!ok(&rt, rt.define_data(o, key("length"), num(1.0), Flags::ALL)));
}

#[test]
fn array_is_array_is_true_only_for_arrays() {
    let rt = Rt::new();
    let (_, v) = arr(&rt, &[]);
    assert!(rt.is_array(&v));
    assert!(!rt.is_array(&Value::Object(rt.new_object())));
    assert!(!rt.is_array(&Value::Undefined));
}

#[test]
fn writes_to_existing_elements_update_in_place() {
    let rt = Rt::new();
    let (o, v) = arr(&rt, &[1.0, 2.0, 3.0]);
    assert!(ok(&rt, rt.set_v(&v, &idx(1), num(20.0))));
    assert_eq!(ok(&rt, rt.get_v(&v, &idx(1))), num(20.0));
    assert_eq!(length_of(&rt, &v), 3.0);
    assert_eq!(texts(&rt, &ok(&rt, rt.own_keys(o))), ["0", "1", "2", "length"]);
}

#[test]
fn dense_limit_refuses_growth_with_a_range_error() {
    // Documented limit: MAX_DENSE_LEN elements. Beyond it the kernel throws instead of allocating.
    let rt = Rt::new();
    let (_, v) = arr(&rt, &[]);
    let t = throws(rt.set_v(&v, &idx(MAX_DENSE_LEN as u32), num(1.0)));
    assert_eq!(rt.get_str(&t.0, "name").ok(), Some(Value::str("RangeError")));
    let t = throws(rt.set_v(&v, &key("length"), num((MAX_DENSE_LEN + 1) as f64)));
    assert_eq!(rt.get_str(&t.0, "name").ok(), Some(Value::str("RangeError")));
    assert_eq!(length_of(&rt, &v), 0.0);
}

#[test]
fn truncating_a_sealed_array_stops_at_its_non_configurable_elements() {
    // Non-configurable elements stop truncation (ECMA-262 ArraySetLength): length stays put.
    let rt = Rt::new();
    let (o, v) = arr(&rt, &[1.0, 2.0]);
    ok(&rt, rt.seal(o));
    assert!(!ok(&rt, rt.define_data(o, key("length"), num(0.0), Flags::new(true, false, false))));
    assert_eq!(length_of(&rt, &v), 2.0);
}

#[test]
fn a_new_element_with_non_default_attributes_is_refused_as_unsupported() {
    // Documented limit (ABI.md): elements share one attribute set, so this throws TypeError.
    let rt = Rt::new();
    let (o, _) = arr(&rt, &[]);
    let t = throws(rt.define_data(o, idx(0), num(1.0), Flags::HIDDEN));
    assert!(message(&rt, &t).starts_with("unsupported:"), "got {:?}", message(&rt, &t));
}

#[test]
fn an_accessor_on_an_array_index_is_refused_as_unsupported() {
    let rt = Rt::new();
    let (o, _) = arr(&rt, &[1.0]);
    let g = rt.new_native_function(|_, _| Ok(Value::Undefined), "get", 0, vec![]);
    let t = throws(rt.define_accessor(o, idx(0), Some(g), None, Flags::ALL));
    assert!(message(&rt, &t).starts_with("unsupported:"));
}

#[test]
fn freezing_an_array_keeps_its_length_and_values_readable() {
    let rt = Rt::new();
    let (o, v) = arr(&rt, &[4.0, 5.0, 6.0]);
    ok(&rt, rt.freeze(o));
    assert_eq!(length_of(&rt, &v), 3.0);
    assert_eq!(ok(&rt, rt.get_v(&v, &idx(2))), num(6.0));
    assert!(ok(&rt, rt.is_sealed(o)));
}

// ----- integrity levels of arrays: V8 does not consult `length` (ABI.md section 12) -----
// Expected values below are Node 26 (V8 14.6) output, written by hand: (isFrozen, isSealed).

/// Applies `op` to a fresh array with `items` and returns (isFrozen, isSealed).
fn integrity_after(rt: &Rt, items: &[f64], op: &str) -> (bool, bool) {
    let (o, _) = arr(rt, items);
    match op {
        "preventExtensions" => assert!(rt.prevent_extensions(o)),
        "seal" => assert!(ok(rt, rt.seal(o))),
        "freeze" => assert!(ok(rt, rt.freeze(o))),
        other => panic!("unknown operation {other}"),
    }
    (ok(rt, rt.is_frozen(o)), ok(rt, rt.is_sealed(o)))
}

#[test]
fn v8_integrity_of_empty_arrays_ignores_the_writable_length() {
    // Object.isFrozen(Object.preventExtensions([])) is true although length is still writable there.
    let rt = Rt::new();
    assert_eq!(integrity_after(&rt, &[], "preventExtensions"), (true, true));
    assert_eq!(integrity_after(&rt, &[], "seal"), (true, true));
    assert_eq!(integrity_after(&rt, &[], "freeze"), (true, true));
}

#[test]
fn v8_integrity_of_one_element_arrays_follows_the_element() {
    // The element's attributes decide: a sealed element stays writable, so it is not frozen.
    let rt = Rt::new();
    assert_eq!(integrity_after(&rt, &[1.0], "preventExtensions"), (false, false));
    assert_eq!(integrity_after(&rt, &[1.0], "seal"), (false, true));
    assert_eq!(integrity_after(&rt, &[1.0], "freeze"), (true, true));
}

#[test]
fn v8_integrity_of_a_non_extensible_array_of_holes_is_frozen_and_sealed() {
    // Holes are not own properties (Object.isFrozen(Object.preventExtensions(new Array(3))) is true).
    let rt = Rt::new();
    let a = rt.new_array(vec![Value::Hole; 3]);
    assert!(rt.prevent_extensions(a));
    assert_eq!((ok(&rt, rt.is_frozen(a)), ok(&rt, rt.is_sealed(a))), (true, true));
}

#[test]
fn an_extensible_array_is_never_frozen_or_sealed_even_with_a_read_only_length() {
    // Object.isFrozen(x) is false for an extensible [] whose length was made read-only.
    let rt = Rt::new();
    let (o, _) = arr(&rt, &[]);
    ok(&rt, rt.define_data(o, key("length"), num(0.0), Flags::NONE));
    let p = ok(&rt, rt.get_own_property(o, &key("length"))).expect("length exists");
    assert!(!p.flags().writable, "the length really is read-only");
    assert!(!ok(&rt, rt.is_frozen(o)));
    assert!(!ok(&rt, rt.is_sealed(o)));
}

#[test]
fn a_read_only_length_on_a_non_extensible_empty_array_is_still_frozen() {
    // Object.isFrozen(Object.defineProperty(Object.preventExtensions([]), 'length', {writable: false})) is true.
    let rt = Rt::new();
    let (o, _) = arr(&rt, &[]);
    assert!(rt.prevent_extensions(o));
    ok(&rt, rt.define_data(o, key("length"), num(0.0), Flags::NONE));
    assert!(ok(&rt, rt.is_frozen(o)));
    assert!(ok(&rt, rt.is_sealed(o)));
}

#[test]
fn freeze_still_makes_length_read_only_and_seal_leaves_it_writable() {
    // Node 26: freeze([1]) has a read-only length; seal([1]) has a writable, non-configurable length.
    let rt = Rt::new();
    let (f, _) = arr(&rt, &[1.0]);
    ok(&rt, rt.freeze(f));
    assert!(!ok(&rt, rt.get_own_property(f, &key("length"))).expect("length").flags().writable);
    let (s, _) = arr(&rt, &[1.0]);
    ok(&rt, rt.seal(s));
    let len = ok(&rt, rt.get_own_property(s, &key("length"))).expect("length");
    assert!(len.flags().writable);
    assert!(!len.flags().configurable);
}

#[test]
fn named_properties_of_arrays_still_count_toward_integrity() {
    // Node 26: a non-extensible array with a writable, configurable named property is neither frozen nor
    // sealed; with a non-writable, non-configurable one it is both.
    let rt = Rt::new();
    let (a, _) = arr(&rt, &[]);
    ok(&rt, rt.define_data(a, key("n"), num(1.0), Flags::ALL));
    assert!(rt.prevent_extensions(a));
    assert_eq!((ok(&rt, rt.is_frozen(a)), ok(&rt, rt.is_sealed(a))), (false, false));
    let (b, _) = arr(&rt, &[]);
    ok(&rt, rt.define_data(b, key("n"), num(1.0), Flags::new(false, true, false)));
    assert!(rt.prevent_extensions(b));
    assert_eq!((ok(&rt, rt.is_frozen(b)), ok(&rt, rt.is_sealed(b))), (true, true));
}

#[test]
fn plain_objects_keep_the_spec_integrity_rule() {
    // Object.isFrozen(Object.preventExtensions({})) is true; a writable property breaks it.
    let rt = Rt::new();
    let o = rt.new_object();
    assert!(rt.prevent_extensions(o));
    assert!(ok(&rt, rt.is_frozen(o)));
    let p = rt.new_object();
    ok(&rt, rt.define_data(p, key("w"), num(1.0), Flags::ALL));
    assert!(rt.prevent_extensions(p));
    assert!(!ok(&rt, rt.is_frozen(p)));
    assert!(!ok(&rt, rt.is_sealed(p)));
}

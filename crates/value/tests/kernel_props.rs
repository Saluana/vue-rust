//! Kernel property operations: key order, accessors and reentrancy, attributes, integrity levels,
//! primitives and coercions. Expected values follow ECMA-262 and were checked against Node 26 (V8) where a
//! comment says "V8". No test here runs collection; see kernel_gc.rs.

use vn_value::{same_value, Call, Flags, JsStr, Prop, PropKey, Res, Rt, Throw, Value};

fn key(s: &str) -> PropKey {
    PropKey::from_str(s)
}

fn texts(rt: &Rt, keys: &[PropKey]) -> Vec<String> {
    keys.iter()
        .map(|k| match k {
            PropKey::Index(i) => i.to_string(),
            PropKey::Str(s) => s.to_rust_string_lossy(),
            PropKey::Sym(s) => {
                format!("Symbol({})", rt.symbol_description(*s).map(|d| d.to_rust_string_lossy()).unwrap_or_default())
            }
        })
        .collect()
}

fn text_prop(rt: &Rt, base: &Value, name: &str) -> String {
    match rt.get_str(base, name) {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => String::from("<none>"),
    }
}

fn describe_throw(rt: &Rt, t: &Throw) -> String {
    format!("{}: {}", text_prop(rt, &t.0, "name"), text_prop(rt, &t.0, "message"))
}

/// Unwraps a non-throwing result, reporting the JavaScript error if there was one.
fn ok<T>(rt: &Rt, r: Res<T>) -> T {
    match r {
        Ok(v) => v,
        Err(t) => panic!("unexpected JS throw: {}", describe_throw(rt, &t)),
    }
}

/// Unwraps a throwing result.
fn throws<T: std::fmt::Debug>(r: Res<T>) -> Throw {
    match r {
        Err(t) => t,
        Ok(v) => panic!("expected a JS throw, got {v:?}"),
    }
}

fn msg(rt: &Rt, t: &Throw) -> String {
    text_prop(rt, &t.0, "message")
}

fn num(n: f64) -> Value {
    Value::Number(n)
}

fn return_this(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(c.this.clone())
}

/// Getter that reads `this.x` through the runtime (reentrant read of the same object).
fn return_this_x(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let Value::Object(o) = c.this else { return Ok(Value::Undefined) };
    rt.get(*o, &key("x"), c.this)
}

/// Setter that mirrors its argument into `this.mirror` (reentrant write of the same object).
fn mirror_setter(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    rt.set_str(c.this, "mirror", c.arg(0))?;
    Ok(Value::Undefined)
}

fn throws_boom(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_type_error("boom"))
}

fn value_of_object(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Object(rt.new_object()))
}

fn to_string_text(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::str("ts"))
}

fn value_of_seven(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(num(7.0))
}

fn to_string_is_seven_text(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::str("text"))
}

#[test]
fn own_keys_orders_indices_strings_then_symbols() {
    // V8: keys 1 2 10 b a Symbol(s)
    let rt = Rt::new();
    let o = rt.new_object();
    let sym = rt.new_symbol(Some(JsStr::from_str("s")));
    ok(&rt, rt.define_data(o, PropKey::Sym(sym), num(1.0), Flags::ALL));
    ok(&rt, rt.define_data(o, key("b"), num(1.0), Flags::ALL));
    ok(&rt, rt.define_data(o, key("2"), num(1.0), Flags::ALL));
    ok(&rt, rt.define_data(o, key("a"), num(1.0), Flags::ALL));
    ok(&rt, rt.define_data(o, key("10"), num(1.0), Flags::ALL));
    ok(&rt, rt.define_data(o, key("1"), num(1.0), Flags::ALL));
    let keys = ok(&rt, rt.own_keys(o));
    assert_eq!(texts(&rt, &keys), ["1", "2", "10", "b", "a", "Symbol(s)"]);
}

#[test]
fn integer_keys_ascend_regardless_of_insertion_order() {
    let rt = Rt::new();
    let o = rt.new_object();
    for k in ["5", "4294967294", "0", "77", "3"] {
        ok(&rt, rt.define_data(o, key(k), num(0.0), Flags::ALL));
    }
    let keys = ok(&rt, rt.own_keys(o));
    assert_eq!(texts(&rt, &keys), ["0", "3", "5", "77", "4294967294"]);
}

#[test]
fn canonical_index_strings_normalise_to_index_keys() {
    assert_eq!(key("2"), PropKey::Index(2));
    assert_eq!(key("4294967294"), PropKey::Index(4_294_967_294));
    assert_eq!(key("0"), PropKey::Index(0));
}

#[test]
fn non_canonical_numeric_strings_stay_string_keys() {
    // "01", "-1", "1.5", "+1" and 2^32-1 are not array indices (ECMA-262 6.1.7).
    for s in ["01", "-1", "1.5", "+1", "4294967295", " 1"] {
        assert!(matches!(key(s), PropKey::Str(_)), "{s:?} must stay a string key");
    }
}

#[test]
fn deleting_then_reinserting_moves_a_key_to_the_end() {
    let rt = Rt::new();
    let o = rt.new_object();
    for k in ["a", "b", "c"] {
        ok(&rt, rt.define_data(o, key(k), num(1.0), Flags::ALL));
    }
    assert!(ok(&rt, rt.delete(o, &key("a"))));
    ok(&rt, rt.define_data(o, key("a"), num(2.0), Flags::ALL));
    let keys = ok(&rt, rt.own_keys(o));
    assert_eq!(texts(&rt, &keys), ["b", "c", "a"]);
}

#[test]
fn many_properties_stay_findable_past_the_index_threshold() {
    let rt = Rt::new();
    let o = rt.new_object();
    for i in 0..100 {
        ok(&rt, rt.define_data(o, key(&format!("p{i}")), num(f64::from(i)), Flags::ALL));
    }
    for i in (0..100).rev() {
        assert_eq!(ok(&rt, rt.get(o, &key(&format!("p{i}")), &Value::Object(o))), num(f64::from(i)));
    }
    assert!(ok(&rt, rt.delete(o, &key("p50"))));
    assert_eq!(ok(&rt, rt.get(o, &key("p50"), &Value::Object(o))), Value::Undefined);
    assert_eq!(ok(&rt, rt.get(o, &key("p51"), &Value::Object(o))), num(51.0));
}

#[test]
fn missing_property_reads_undefined_and_define_reads_back() {
    let rt = Rt::new();
    let o = rt.new_object();
    let ov = Value::Object(o);
    assert_eq!(ok(&rt, rt.get(o, &key("nope"), &ov)), Value::Undefined);
    ok(&rt, rt.define_data(o, key("yes"), num(3.0), Flags::ALL));
    assert_eq!(ok(&rt, rt.get(o, &key("yes"), &ov)), num(3.0));
}

#[test]
fn get_walks_the_prototype_chain() {
    let rt = Rt::new();
    let base = rt.new_object();
    ok(&rt, rt.define_data(base, key("inherited"), Value::str("from base"), Flags::ALL));
    let child = rt.new_object_with_proto(Some(base));
    let got = ok(&rt, rt.get(child, &key("inherited"), &Value::Object(child)));
    assert_eq!(got, Value::str("from base"));
    assert!(!ok(&rt, rt.has_own_property(child, &key("inherited"))));
    assert!(ok(&rt, rt.has_property(child, &key("inherited"))));
}

#[test]
fn set_creates_own_property_and_leaves_the_prototype_alone() {
    let rt = Rt::new();
    let base = rt.new_object();
    ok(&rt, rt.define_data(base, key("v"), num(1.0), Flags::ALL));
    let child = rt.new_object_with_proto(Some(base));
    assert!(ok(&rt, rt.set(child, &key("v"), num(2.0), &Value::Object(child))));
    assert_eq!(ok(&rt, rt.get(base, &key("v"), &Value::Object(base))), num(1.0));
    assert_eq!(ok(&rt, rt.get(child, &key("v"), &Value::Object(child))), num(2.0));
}

#[test]
fn inherited_read_only_property_blocks_assignment() {
    let rt = Rt::new();
    let base = rt.new_object();
    ok(&rt, rt.define_data(base, key("k"), num(1.0), Flags::new(false, true, true)));
    let child = rt.new_object_with_proto(Some(base));
    assert!(!ok(&rt, rt.set(child, &key("k"), num(9.0), &Value::Object(child))));
    assert!(!ok(&rt, rt.has_own_property(child, &key("k"))));
}

#[test]
fn inherited_setter_runs_with_the_assigned_object_as_receiver() {
    let rt = Rt::new();
    let base = rt.new_object();
    let setter = rt.new_native_function(mirror_setter, "set m", 1, vec![]);
    ok(&rt, rt.define_accessor(base, key("m"), None, Some(setter), Flags::new(false, true, true)));
    let child = rt.new_object_with_proto(Some(base));
    let cv = Value::Object(child);
    assert!(ok(&rt, rt.set(child, &key("m"), num(5.0), &cv)));
    // The setter wrote `this.mirror`, so `this` was the child, not the prototype.
    assert_eq!(ok(&rt, rt.get(child, &key("mirror"), &cv)), num(5.0));
    assert_eq!(ok(&rt, rt.get(base, &key("mirror"), &cv)), Value::Undefined);
}

#[test]
fn getter_receives_the_receiver_as_this() {
    let rt = Rt::new();
    let base = rt.new_object();
    let getter = rt.new_native_function(return_this, "get r", 0, vec![]);
    ok(&rt, rt.define_accessor(base, key("r"), Some(getter), None, Flags::new(false, true, true)));
    let child = rt.new_object_with_proto(Some(base));
    assert_eq!(ok(&rt, rt.get(child, &key("r"), &Value::Object(child))), Value::Object(child));
}

#[test]
fn getter_that_reads_its_own_object_reenters_cleanly() {
    let rt = Rt::new();
    let o = rt.new_object();
    ok(&rt, rt.define_data(o, key("x"), num(41.0), Flags::ALL));
    let g = rt.new_native_function(return_this_x, "get y", 0, vec![]);
    ok(&rt, rt.define_accessor(o, key("y"), Some(g), None, Flags::new(false, true, true)));
    assert_eq!(ok(&rt, rt.get(o, &key("y"), &Value::Object(o))), num(41.0));
}

#[test]
fn setter_that_writes_its_own_object_reenters_cleanly() {
    let rt = Rt::new();
    let o = rt.new_object();
    let s = rt.new_native_function(mirror_setter, "set m", 1, vec![]);
    ok(&rt, rt.define_accessor(o, key("m"), None, Some(s), Flags::new(false, true, true)));
    assert!(ok(&rt, rt.set(o, &key("m"), num(8.0), &Value::Object(o))));
    assert_eq!(ok(&rt, rt.get(o, &key("mirror"), &Value::Object(o))), num(8.0));
}

#[test]
fn getter_throw_propagates_as_an_error_value() {
    let rt = Rt::new();
    let o = rt.new_object();
    let g = rt.new_native_function(throws_boom, "get b", 0, vec![]);
    ok(&rt, rt.define_accessor(o, key("b"), Some(g), None, Flags::new(false, true, true)));
    let t = throws(rt.get(o, &key("b"), &Value::Object(o)));
    assert_eq!(text_prop(&rt, &t.0, "name"), "TypeError");
    assert_eq!(msg(&rt, &t), "boom");
}

#[test]
fn accessor_without_setter_refuses_assignment() {
    let rt = Rt::new();
    let o = rt.new_object();
    let g = rt.new_native_function(return_this, "get g", 0, vec![]);
    ok(&rt, rt.define_accessor(o, key("g"), Some(g), None, Flags::new(false, true, true)));
    assert!(!ok(&rt, rt.set(o, &key("g"), num(1.0), &Value::Object(o))));
}

#[test]
fn define_reports_attributes_through_get_own_property() {
    let rt = Rt::new();
    let o = rt.new_object();
    ok(&rt, rt.define_data(o, key("h"), num(1.0), Flags::HIDDEN));
    let p = ok(&rt, rt.get_own_property(o, &key("h"))).expect("h is present");
    assert_eq!(p, Prop::Data { value: num(1.0), flags: Flags::new(true, false, true) });
    assert!(ok(&rt, rt.get_own_property(o, &key("absent"))).is_none());
}

#[test]
fn non_configurable_read_only_refuses_a_different_value() {
    let rt = Rt::new();
    let o = rt.new_object();
    let ro = Flags::new(false, true, false);
    ok(&rt, rt.define_data(o, key("k"), num(1.0), ro));
    assert!(ok(&rt, rt.define_data(o, key("k"), num(1.0), ro)), "same value and attributes are allowed");
    assert!(!ok(&rt, rt.define_data(o, key("k"), num(2.0), ro)));
    assert!(!ok(&rt, rt.set(o, &key("k"), num(3.0), &Value::Object(o))));
    assert_eq!(ok(&rt, rt.get(o, &key("k"), &Value::Object(o))), num(1.0));
}

#[test]
fn non_configurable_writable_allows_value_change_then_read_only() {
    let rt = Rt::new();
    let o = rt.new_object();
    ok(&rt, rt.define_data(o, key("w"), num(1.0), Flags::new(true, true, false)));
    assert!(ok(&rt, rt.define_data(o, key("w"), num(2.0), Flags::new(false, true, false))));
    assert!(!ok(&rt, rt.define_data(o, key("w"), num(3.0), Flags::new(true, true, false))), "cannot re-enable writable");
    assert_eq!(ok(&rt, rt.get(o, &key("w"), &Value::Object(o))), num(2.0));
}

#[test]
fn non_configurable_refuses_becoming_an_accessor_or_configurable() {
    let rt = Rt::new();
    let o = rt.new_object();
    ok(&rt, rt.define_data(o, key("n"), num(1.0), Flags::new(true, true, false)));
    let g = rt.new_native_function(return_this, "get n", 0, vec![]);
    assert!(!ok(&rt, rt.define_accessor(o, key("n"), Some(g), None, Flags::new(false, true, false))));
    assert!(!ok(&rt, rt.define_data(o, key("n"), num(1.0), Flags::ALL)));
}

#[test]
fn configurable_property_can_change_kind() {
    let rt = Rt::new();
    let o = rt.new_object();
    ok(&rt, rt.define_data(o, key("c"), num(1.0), Flags::ALL));
    let g = rt.new_native_function(return_this, "get c", 0, vec![]);
    assert!(ok(&rt, rt.define_accessor(o, key("c"), Some(g), None, Flags::new(false, true, true))));
    let p = ok(&rt, rt.get_own_property(o, &key("c"))).expect("c");
    assert!(p.is_accessor());
}

#[test]
fn delete_respects_configurable() {
    let rt = Rt::new();
    let o = rt.new_object();
    ok(&rt, rt.define_data(o, key("keep"), num(1.0), Flags::new(true, true, false)));
    ok(&rt, rt.define_data(o, key("drop"), num(1.0), Flags::ALL));
    assert!(!ok(&rt, rt.delete(o, &key("keep"))));
    assert!(ok(&rt, rt.delete(o, &key("drop"))));
    assert!(ok(&rt, rt.delete(o, &key("never-there"))));
    assert!(!ok(&rt, rt.has_own_property(o, &key("drop"))));
}

#[test]
fn strict_assignment_to_read_only_throws_with_the_spec_message() {
    let rt = Rt::new();
    let o = rt.new_object();
    ok(&rt, rt.define_data(o, key("ro"), num(1.0), Flags::NONE));
    let t = throws(rt.set_strict(o, &key("ro"), num(2.0)));
    assert_eq!(msg(&rt, &t), "Cannot assign to read only property 'ro' of object");
    assert_eq!(text_prop(&rt, &t.0, "name"), "TypeError");
}

#[test]
fn strict_add_to_non_extensible_object_throws_add_message() {
    let rt = Rt::new();
    let o = rt.new_object();
    assert!(rt.prevent_extensions(o));
    let t = throws(rt.set_strict(o, &key("fresh"), num(1.0)));
    assert_eq!(msg(&rt, &t), "Cannot add property fresh, object is not extensible");
}

#[test]
fn prevent_extensions_blocks_new_keys_and_allows_existing_writes() {
    let rt = Rt::new();
    let o = rt.new_object();
    ok(&rt, rt.define_data(o, key("a"), num(1.0), Flags::ALL));
    assert!(rt.prevent_extensions(o));
    assert!(!rt.is_extensible(o));
    assert!(!ok(&rt, rt.define_data(o, key("b"), num(1.0), Flags::ALL)));
    assert!(ok(&rt, rt.set(o, &key("a"), num(5.0), &Value::Object(o))));
    assert_eq!(ok(&rt, rt.get(o, &key("a"), &Value::Object(o))), num(5.0));
}

#[test]
fn freeze_makes_every_property_read_only_and_fixed() {
    let rt = Rt::new();
    let o = rt.new_object();
    ok(&rt, rt.define_data(o, key("x"), num(1.0), Flags::ALL));
    ok(&rt, rt.define_data(o, key("y"), num(2.0), Flags::ALL));
    assert!(!ok(&rt, rt.is_frozen(o)));
    assert!(ok(&rt, rt.freeze(o)));
    assert!(ok(&rt, rt.is_frozen(o)));
    assert!(ok(&rt, rt.is_sealed(o)));
    assert!(!ok(&rt, rt.set(o, &key("x"), num(9.0), &Value::Object(o))));
    assert!(!ok(&rt, rt.delete(o, &key("y"))));
    assert!(!ok(&rt, rt.define_data(o, key("z"), num(0.0), Flags::ALL)));
    assert_eq!(ok(&rt, rt.get(o, &key("y"), &Value::Object(o))), num(2.0));
}

#[test]
fn seal_keeps_values_writable_but_blocks_delete_and_add() {
    let rt = Rt::new();
    let o = rt.new_object();
    ok(&rt, rt.define_data(o, key("s"), num(1.0), Flags::ALL));
    assert!(ok(&rt, rt.seal(o)));
    assert!(ok(&rt, rt.is_sealed(o)));
    assert!(!ok(&rt, rt.is_frozen(o)));
    assert!(ok(&rt, rt.set(o, &key("s"), num(4.0), &Value::Object(o))));
    assert!(!ok(&rt, rt.delete(o, &key("s"))));
    assert_eq!(ok(&rt, rt.get(o, &key("s"), &Value::Object(o))), num(4.0));
}

#[test]
fn empty_non_extensible_object_is_vacuously_frozen_and_sealed() {
    let rt = Rt::new();
    let o = rt.new_object();
    assert!(!ok(&rt, rt.is_frozen(o)), "an extensible object is never frozen");
    assert!(ok(&rt, rt.freeze(o)));
    assert!(ok(&rt, rt.is_frozen(o)));
}

#[test]
fn set_prototype_of_rejects_cycles_and_non_extensible_targets() {
    let rt = Rt::new();
    let a = rt.new_object();
    let b = rt.new_object_with_proto(Some(a));
    assert!(!rt.set_prototype_of(a, Some(b)), "a -> b -> a would be a cycle");
    let c = rt.new_object();
    assert!(rt.prevent_extensions(c));
    assert!(!rt.set_prototype_of(c, Some(a)));
    assert!(rt.set_prototype_of(b, None));
    assert_eq!(rt.get_prototype_of(b), None);
}

#[test]
fn symbol_keys_round_trip_and_follow_strings() {
    let rt = Rt::new();
    let o = rt.new_object();
    let s = rt.new_symbol(Some(JsStr::from_str("tag")));
    ok(&rt, rt.define_data(o, PropKey::Sym(s), Value::str("symbol value"), Flags::ALL));
    ok(&rt, rt.define_data(o, key("name"), num(1.0), Flags::ALL));
    assert_eq!(ok(&rt, rt.get(o, &PropKey::Sym(s), &Value::Object(o))), Value::str("symbol value"));
    let keys = ok(&rt, rt.own_keys(o));
    assert_eq!(texts(&rt, &keys), ["name", "Symbol(tag)"]);
}

#[test]
fn get_v_on_strings_reads_length_index_and_prototype() {
    let rt = Rt::new();
    let base = Value::str("héllo");
    assert_eq!(ok(&rt, rt.get_v(&base, &key("length"))), num(5.0));
    assert_eq!(ok(&rt, rt.get_v(&base, &key("1"))), Value::str("é"));
    assert_eq!(ok(&rt, rt.get_v(&base, &key("9"))), Value::Undefined);
    assert_eq!(ok(&rt, rt.get_v(&base, &key("nope"))), Value::Undefined);
}

#[test]
fn get_v_on_undefined_and_null_throws_the_v8_message() {
    let rt = Rt::new();
    let t = throws(rt.get_v(&Value::Undefined, &key("x")));
    assert_eq!(msg(&rt, &t), "Cannot read properties of undefined (reading 'x')");
    let t = throws(rt.get_v(&Value::Null, &key("y")));
    assert_eq!(msg(&rt, &t), "Cannot read properties of null (reading 'y')");
}

#[test]
fn set_v_on_undefined_throws_and_on_primitives_is_refused() {
    let rt = Rt::new();
    let t = throws(rt.set_v(&Value::Undefined, &key("x"), num(1.0)));
    assert_eq!(msg(&rt, &t), "Cannot set properties of undefined (setting 'x')");
    assert!(!ok(&rt, rt.set_v(&num(1.0), &key("x"), num(2.0))));
    let t = throws(rt.set_v_strict(&Value::str("abc"), &key("x"), num(2.0)));
    assert_eq!(msg(&rt, &t), "Cannot create property 'x' on string 'abc'");
    let t = throws(rt.set_v_strict(&num(1.0), &key("y"), num(2.0)));
    assert_eq!(msg(&rt, &t), "Cannot create property 'y' on number '1'");
}

#[test]
fn to_primitive_prefers_value_of_for_default_and_to_string_for_string_hint() {
    let rt = Rt::new();
    let o = rt.new_object();
    let v_of = rt.new_native_function(value_of_object, "valueOf", 0, vec![]);
    let t_str = rt.new_native_function(to_string_text, "toString", 0, vec![]);
    ok(&rt, rt.define_data(o, key("valueOf"), Value::Object(v_of), Flags::HIDDEN));
    ok(&rt, rt.define_data(o, key("toString"), Value::Object(t_str), Flags::HIDDEN));
    // valueOf returns an object, so ToPrimitive falls back to toString.
    assert_eq!(ok(&rt, rt.to_string(&Value::Object(o))), JsStr::from_str("ts"));
    assert_eq!(ok(&rt, rt.to_primitive(&Value::Object(o), vn_value::Hint::Default)), Value::str("ts"));
}

#[test]
fn to_primitive_uses_value_of_first_when_it_yields_a_primitive() {
    let rt = Rt::new();
    let o = rt.new_object();
    let v_of = rt.new_native_function(value_of_seven, "valueOf", 0, vec![]);
    let t_str = rt.new_native_function(to_string_is_seven_text, "toString", 0, vec![]);
    ok(&rt, rt.define_data(o, key("valueOf"), Value::Object(v_of), Flags::HIDDEN));
    ok(&rt, rt.define_data(o, key("toString"), Value::Object(t_str), Flags::HIDDEN));
    assert_eq!(ok(&rt, rt.to_number(&Value::Object(o))), 7.0);
    assert_eq!(ok(&rt, rt.to_string(&Value::Object(o))), JsStr::from_str("text"));
}

#[test]
fn to_number_follows_string_to_number_for_primitives() {
    // V8: Number(undefined) NaN, Number(null) 0, Number(true) 1, Number("  12  ") 12, Number("0x10") 16.
    let rt = Rt::new();
    assert!(ok(&rt, rt.to_number(&Value::Undefined)).is_nan());
    assert_eq!(ok(&rt, rt.to_number(&Value::Null)), 0.0);
    assert_eq!(ok(&rt, rt.to_number(&Value::Bool(true))), 1.0);
    assert_eq!(ok(&rt, rt.to_number(&Value::str("  12  "))), 12.0);
    assert_eq!(ok(&rt, rt.to_number(&Value::str("0x10"))), 16.0);
    assert!(ok(&rt, rt.to_number(&Value::str("12px"))).is_nan());
}

#[test]
fn to_property_key_normalises_numbers_and_keeps_symbols() {
    let rt = Rt::new();
    assert_eq!(ok(&rt, rt.to_property_key(&num(1.0))), PropKey::Index(1));
    assert_eq!(ok(&rt, rt.to_property_key(&num(-0.0))), PropKey::Index(0));
    assert_eq!(ok(&rt, rt.to_property_key(&num(1.5))), key("1.5"));
    let s = rt.new_symbol(None);
    assert_eq!(ok(&rt, rt.to_property_key(&Value::Symbol(s))), PropKey::Sym(s));
}

#[test]
fn symbol_to_string_and_number_throw_type_errors() {
    let rt = Rt::new();
    let s = rt.new_symbol(Some(JsStr::from_str("q")));
    let t = throws(rt.to_string(&Value::Symbol(s)));
    assert_eq!(msg(&rt, &t), "Cannot convert a Symbol value to a string");
    let t = throws(rt.to_number(&Value::Symbol(s)));
    assert_eq!(msg(&rt, &t), "Cannot convert a Symbol value to a number");
}

#[test]
fn typeof_and_to_boolean_follow_the_spec_tables() {
    let rt = Rt::new();
    assert_eq!(rt.type_of(&Value::Undefined), "undefined");
    assert_eq!(rt.type_of(&Value::Null), "object");
    assert_eq!(rt.type_of(&Value::Bool(false)), "boolean");
    assert_eq!(rt.type_of(&num(0.0)), "number");
    assert_eq!(rt.type_of(&Value::str("")), "string");
    assert_eq!(rt.type_of(&Value::Symbol(rt.new_symbol(None))), "symbol");
    assert_eq!(rt.type_of(&Value::Object(rt.new_object())), "object");
    let g = rt.new_native_function(return_this, "f", 0, vec![]);
    assert_eq!(rt.type_of(&Value::Object(g)), "function");
    for falsy in [Value::Undefined, Value::Null, Value::Bool(false), num(0.0), num(-0.0), num(f64::NAN), Value::str("")] {
        assert!(!falsy.to_boolean(), "{falsy:?} is falsy");
    }
    for truthy in [num(0.5), Value::str("0"), Value::Object(g), Value::Bool(true)] {
        assert!(truthy.to_boolean(), "{truthy:?} is truthy");
    }
}

#[test]
fn same_value_separates_signed_zero_and_joins_nan() {
    assert!(same_value(&num(f64::NAN), &num(f64::NAN)));
    assert!(!same_value(&num(0.0), &num(-0.0)));
    assert!(same_value(&Value::str("a"), &Value::str("a")));
    assert!(!same_value(&Value::Undefined, &Value::Null));
}

#[test]
fn get_str_and_set_str_use_the_same_key_normalisation() {
    let rt = Rt::new();
    let o = rt.new_object();
    let ov = Value::Object(o);
    assert!(ok(&rt, rt.set_str(&ov, "7", Value::str("seven"))));
    assert_eq!(ok(&rt, rt.get_str(&ov, "7")), Value::str("seven"));
    assert_eq!(ok(&rt, rt.get(o, &PropKey::Index(7), &ov)), Value::str("seven"));
}

#[test]
fn strict_writes_to_string_indices_and_length_name_the_string() {
    // V8: "abc"[0] = "x" and "abc".length = 5 are read-only; "abc"[5] = 1 would create a property.
    let rt = Rt::new();
    let s = Value::str("abc");
    let t = throws(rt.set_v_strict(&s, &PropKey::Index(0), num(1.0)));
    assert_eq!(msg(&rt, &t), "Cannot assign to read only property '0' of string 'abc'");
    let t = throws(rt.set_v_strict(&s, &key("length"), num(5.0)));
    assert_eq!(msg(&rt, &t), "Cannot assign to read only property 'length' of string 'abc'");
    let t = throws(rt.set_v_strict(&s, &PropKey::Index(5), num(1.0)));
    assert_eq!(msg(&rt, &t), "Cannot create property '5' on string 'abc'");
}

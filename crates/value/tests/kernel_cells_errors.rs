//! Kernel cells (closure variables and the TDZ), the Error hierarchy and symbols. Expected values are
//! written from ECMA-262 (20.5, 6.1.5, 9.1.1.1.1) or checked against V8 where a comment says "V8".

use vn_value::{Call, Flags, Intrinsic, JsStr, Prop, PropKey, Res, Rt, Throw, Value, WellKnown};

fn key(s: &str) -> PropKey {
    PropKey::from_str(s)
}

fn ok<T>(rt: &Rt, r: Res<T>) -> T {
    match r {
        Ok(v) => v,
        Err(t) => panic!("unexpected JS throw: {}", describe(rt, &t)),
    }
}

fn throws<T: std::fmt::Debug>(r: Res<T>) -> Throw {
    match r {
        Err(t) => t,
        Ok(v) => panic!("expected a JS throw, got {v:?}"),
    }
}

fn text_of(rt: &Rt, v: &Value, name: &str) -> String {
    match rt.get_str(v, name) {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        Ok(other) => format!("{other:?}"),
        Err(_) => String::from("<throw>"),
    }
}

fn describe(rt: &Rt, t: &Throw) -> String {
    format!("{}: {}", text_of(rt, &t.0, "name"), text_of(rt, &t.0, "message"))
}

fn names_of(rt: &Rt, keys: &[PropKey]) -> Vec<String> {
    keys.iter()
        .map(|k| match k {
            PropKey::Index(i) => i.to_string(),
            PropKey::Str(s) => s.to_rust_string_lossy(),
            PropKey::Sym(s) => format!("Symbol({})", rt.symbol_description(*s).map(|d| d.to_rust_string_lossy()).unwrap_or_default()),
        })
        .collect()
}

fn ctor_call(rt: &Rt, which: Intrinsic, args: &[Value]) -> Res<Value> {
    let c = rt.intrinsic(which);
    rt.call(&Value::Object(c), &Value::Undefined, args)
}

fn ctor_new(rt: &Rt, which: Intrinsic, args: &[Value]) -> Res<Value> {
    let c = rt.intrinsic(which);
    rt.construct(&Value::Object(c), args, &Value::Object(c))
}

// ----- cells and the temporal dead zone -----

#[test]
fn cell_set_then_get_round_trips() {
    let rt = Rt::new();
    let c = rt.new_cell(Value::Number(1.0));
    assert_eq!(rt.cell_get(c), Value::Number(1.0));
    rt.cell_set(c, Value::str("two"));
    assert_eq!(rt.cell_get(c), Value::str("two"));
}

#[test]
fn distinct_cells_do_not_alias() {
    let rt = Rt::new();
    let a = rt.new_cell(Value::Number(1.0));
    let b = rt.new_cell(Value::Number(1.0));
    assert_ne!(a, b);
    rt.cell_set(a, Value::Number(9.0));
    assert_eq!(rt.cell_get(b), Value::Number(1.0));
}

#[test]
fn checked_read_of_an_uninitialised_binding_is_a_reference_error() {
    // ECMA-262 9.1.1.1.6: reading a let/const binding in its TDZ throws ReferenceError.
    let rt = Rt::new();
    let c = rt.new_cell(Value::Uninit);
    let t = throws(rt.cell_get_checked(c, "x"));
    assert_eq!(describe(&rt, &t), "ReferenceError: Cannot access 'x' before initialization");
}

#[test]
fn initialising_then_reading_checked_succeeds() {
    let rt = Rt::new();
    let c = rt.new_cell(Value::Uninit);
    rt.cell_init(c, Value::Number(7.0));
    assert_eq!(ok(&rt, rt.cell_get_checked(c, "y")), Value::Number(7.0));
}

#[test]
fn assigning_before_initialisation_is_a_reference_error() {
    let rt = Rt::new();
    let c = rt.new_cell(Value::Uninit);
    let t = throws(rt.cell_set_checked(c, "z", Value::Number(1.0)));
    assert_eq!(describe(&rt, &t), "ReferenceError: Cannot access 'z' before initialization");
    assert_eq!(rt.cell_get(c), Value::Uninit, "a refused assignment leaves the TDZ marker");
}

#[test]
fn assigning_after_initialisation_writes_through_the_checked_form() {
    let rt = Rt::new();
    let c = rt.new_cell(Value::Uninit);
    rt.cell_init(c, Value::Number(1.0));
    ok(&rt, rt.cell_set_checked(c, "w", Value::Number(2.0)));
    assert_eq!(rt.cell_get(c), Value::Number(2.0));
}

// ----- errors -----

#[test]
fn make_error_builds_name_message_and_stack() {
    let rt = Rt::new();
    let e = rt.make_error(vn_value::ErrorKind::Type, "bad");
    assert_eq!(text_of(&rt, &e, "name"), "TypeError");
    assert_eq!(text_of(&rt, &e, "message"), "bad");
    assert_eq!(text_of(&rt, &e, "stack"), "TypeError: bad\n    at <anonymous>");
}

#[test]
fn the_throw_helpers_build_the_matching_error_kinds() {
    let rt = Rt::new();
    assert_eq!(describe(&rt, &rt.throw_range_error("r")), "RangeError: r");
    assert_eq!(describe(&rt, &rt.throw_reference_error("f")), "ReferenceError: f");
    assert_eq!(describe(&rt, &rt.throw_syntax_error("s")), "SyntaxError: s");
    assert_eq!(describe(&rt, &rt.throw_type_error("t")), "TypeError: t");
}

#[test]
fn an_error_without_a_message_has_only_a_stack_and_no_colon_in_it() {
    // V8 prints the first stack line as "Error" when the message is empty.
    let rt = Rt::new();
    let e = ok(&rt, ctor_call(&rt, Intrinsic::ErrorCtor, &[]));
    let o = e.as_object().unwrap();
    assert_eq!(names_of(&rt, &ok(&rt, rt.own_keys(o))), ["stack"]);
    assert_eq!(text_of(&rt, &e, "stack"), "Error\n    at <anonymous>");
}

#[test]
fn error_constructor_works_without_new() {
    let rt = Rt::new();
    let e = ok(&rt, ctor_call(&rt, Intrinsic::TypeErrorCtor, &[Value::str("plain")]));
    assert_eq!(text_of(&rt, &e, "message"), "plain");
    let proto = rt.get_prototype_of(e.as_object().unwrap()).unwrap();
    assert_eq!(proto, rt.intrinsic(Intrinsic::TypeErrorPrototype));
}

#[test]
fn error_constructor_works_with_new() {
    let rt = Rt::new();
    let e = ok(&rt, ctor_new(&rt, Intrinsic::RangeErrorCtor, &[Value::str("nw")]));
    assert_eq!(describe(&rt, &Throw(e.clone())), "RangeError: nw");
    assert_eq!(rt.get_prototype_of(e.as_object().unwrap()), Some(rt.intrinsic(Intrinsic::RangeErrorPrototype)));
}

#[test]
fn constructor_converts_a_non_string_message() {
    // V8: new Error(42).message is "42".
    let rt = Rt::new();
    let e = ok(&rt, ctor_new(&rt, Intrinsic::ErrorCtor, &[Value::Number(42.0)]));
    assert_eq!(text_of(&rt, &e, "message"), "42");
}

#[test]
fn undefined_message_creates_no_own_message_property() {
    let rt = Rt::new();
    let e = ok(&rt, ctor_new(&rt, Intrinsic::ErrorCtor, &[Value::Undefined]));
    let o = e.as_object().unwrap();
    assert!(!ok(&rt, rt.has_own_property(o, &key("message"))));
    assert_eq!(text_of(&rt, &e, "message"), "", "message is inherited from Error.prototype");
}

#[test]
fn cause_option_creates_an_own_non_enumerable_cause() {
    let rt = Rt::new();
    let opts = rt.new_object();
    ok(&rt, rt.define_data(opts, key("cause"), Value::Number(5.0), Flags::ALL));
    let e = ok(&rt, ctor_new(&rt, Intrinsic::ErrorCtor, &[Value::str("m"), Value::Object(opts)]));
    let o = e.as_object().unwrap();
    let p = ok(&rt, rt.get_own_property(o, &key("cause"))).expect("cause is own");
    assert_eq!(p, Prop::Data { value: Value::Number(5.0), flags: Flags::HIDDEN });
}

#[test]
fn cause_is_absent_when_the_options_do_not_have_it() {
    let rt = Rt::new();
    let opts = rt.new_object();
    let e = ok(&rt, ctor_new(&rt, Intrinsic::ErrorCtor, &[Value::str("m"), Value::Object(opts)]));
    assert!(!ok(&rt, rt.has_own_property(e.as_object().unwrap(), &key("cause"))));
}

#[test]
fn error_own_message_is_not_enumerable() {
    let rt = Rt::new();
    let e = rt.make_error(vn_value::ErrorKind::Error, "hidden");
    let p = ok(&rt, rt.get_own_property(e.as_object().unwrap(), &key("message"))).unwrap();
    assert!(!p.flags().enumerable);
    assert!(p.flags().writable && p.flags().configurable);
}

#[test]
fn prototype_chain_runs_from_each_kind_to_error_prototype() {
    // V8: Object.getPrototypeOf(TypeError.prototype) === Error.prototype, and TypeError's own prototype is Error.
    let rt = Rt::new();
    let ep = rt.intrinsic(Intrinsic::ErrorPrototype);
    let tp = rt.intrinsic(Intrinsic::TypeErrorPrototype);
    assert_eq!(rt.get_prototype_of(tp), Some(ep));
    assert_eq!(rt.get_prototype_of(ep), Some(rt.intrinsic(Intrinsic::ObjectPrototype)));
    assert_eq!(rt.get_prototype_of(rt.intrinsic(Intrinsic::TypeErrorCtor)), Some(rt.intrinsic(Intrinsic::ErrorCtor)));
    assert_eq!(rt.get_prototype_of(rt.intrinsic(Intrinsic::ErrorCtor)), Some(rt.intrinsic(Intrinsic::FunctionPrototype)));
}

#[test]
fn prototypes_carry_name_and_empty_message_and_link_back_to_constructors() {
    let rt = Rt::new();
    let kinds = [
        (Intrinsic::ErrorPrototype, Intrinsic::ErrorCtor, "Error"),
        (Intrinsic::TypeErrorPrototype, Intrinsic::TypeErrorCtor, "TypeError"),
        (Intrinsic::RangeErrorPrototype, Intrinsic::RangeErrorCtor, "RangeError"),
        (Intrinsic::ReferenceErrorPrototype, Intrinsic::ReferenceErrorCtor, "ReferenceError"),
        (Intrinsic::SyntaxErrorPrototype, Intrinsic::SyntaxErrorCtor, "SyntaxError"),
        (Intrinsic::EvalErrorPrototype, Intrinsic::EvalErrorCtor, "EvalError"),
        (Intrinsic::UriErrorPrototype, Intrinsic::UriErrorCtor, "URIError"),
    ];
    for (proto, ctor, name) in kinds {
        let p = rt.intrinsic(proto);
        assert_eq!(text_of(&rt, &Value::Object(p), "name"), name);
        assert_eq!(text_of(&rt, &Value::Object(p), "message"), "");
        let c = rt.intrinsic(ctor);
        assert_eq!(ok(&rt, rt.get(c, &key("prototype"), &Value::Object(c))), Value::Object(p));
        assert_eq!(ok(&rt, rt.get(p, &key("constructor"), &Value::Object(p))), Value::Object(c));
        assert_eq!(text_of(&rt, &Value::Object(c), "name"), name);
        assert_eq!(ok(&rt, rt.get(c, &key("length"), &Value::Object(c))), Value::Number(1.0));
    }
}

#[test]
fn error_prototype_to_string_formats_like_v8() {
    // V8: String(new TypeError("")) is "TypeError"; "TypeError: m" with a message; the message alone when
    // the name is empty.
    let rt = Rt::new();
    let s = |rt: &Rt, v: Value| ok(rt, rt.to_string(&v)).to_rust_string_lossy();
    let e_empty = ok(&rt, ctor_new(&rt, Intrinsic::TypeErrorCtor, &[Value::str("")]));
    assert_eq!(s(&rt, e_empty), "TypeError");
    let e = ok(&rt, ctor_new(&rt, Intrinsic::TypeErrorCtor, &[Value::str("m")]));
    assert_eq!(s(&rt, e.clone()), "TypeError: m");
    let anon = rt.new_object_with_proto(Some(rt.intrinsic(Intrinsic::ErrorPrototype)));
    ok(&rt, rt.define_data(anon, key("name"), Value::str(""), Flags::ALL));
    ok(&rt, rt.define_data(anon, key("message"), Value::str("q"), Flags::ALL));
    assert_eq!(s(&rt, Value::Object(anon)), "q");
}

#[test]
fn error_prototype_to_string_rejects_a_non_object_receiver() {
    let rt = Rt::new();
    let f = rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::ErrorPrototype)), "toString").unwrap();
    let t = throws(rt.call(&f, &Value::Undefined, &[]));
    assert_eq!(describe(&rt, &t), "TypeError: Error.prototype.toString requires that 'this' be an Object");
}

#[test]
fn error_constructor_length_is_one_and_name_matches() {
    // V8: Error.length === 1 and Error.name === "Error".
    let rt = Rt::new();
    let e = Value::Object(rt.intrinsic(Intrinsic::ErrorCtor));
    assert_eq!(text_of(&rt, &e, "name"), "Error");
    assert_eq!(ok(&rt, rt.get_str(&e, "length")), Value::Number(1.0));
}

#[test]
fn constructor_uses_new_target_prototype_when_present() {
    // Mirrors `class X extends Error`: new.target.prototype decides the instance's prototype.
    let rt = Rt::new();
    let noop: vn_value::CodeFn = |_, _| Ok(Value::Undefined);
    let sub = rt.new_native_constructor(noop, noop, "Sub", 0, vec![]);
    let p = rt.new_object_with_proto(Some(rt.intrinsic(Intrinsic::TypeErrorPrototype)));
    ok(&rt, rt.define_data(sub, key("prototype"), Value::Object(p), Flags::NONE));
    let e = ok(&rt, rt.construct(&Value::Object(rt.intrinsic(Intrinsic::ErrorCtor)), &[], &Value::Object(sub)));
    assert_eq!(rt.get_prototype_of(e.as_object().unwrap()), Some(p));
}

#[test]
fn the_error_constructors_are_installed_on_the_global_object() {
    let rt = Rt::new();
    let g = rt.intrinsic(Intrinsic::Global);
    for name in ["Error", "TypeError", "RangeError", "ReferenceError", "SyntaxError", "EvalError", "URIError"] {
        let v = ok(&rt, rt.get(g, &key(name), &Value::Object(g)));
        assert!(rt.is_constructor(&v), "{name} is a constructor");
    }
    assert_eq!(ok(&rt, rt.get(g, &key("globalThis"), &Value::Object(g))), Value::Object(g));
}

fn echo_message(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let _ = rt;
    Ok(c.arg(0))
}

#[test]
fn a_thrown_value_round_trips_through_a_native_call() {
    let rt = Rt::new();
    let f = rt.new_native_function(echo_message, "echo", 1, vec![]);
    let v = ok(&rt, rt.call(&Value::Object(f), &Value::Undefined, &[Value::str("payload")]));
    assert_eq!(v, Value::str("payload"));
}

// ----- symbols -----

#[test]
fn new_symbols_are_unique_even_with_the_same_description() {
    let rt = Rt::new();
    let a = rt.new_symbol(Some(JsStr::from_str("d")));
    let b = rt.new_symbol(Some(JsStr::from_str("d")));
    assert_ne!(a, b);
    let o = rt.new_object();
    ok(&rt, rt.define_data(o, PropKey::Sym(a), Value::Number(1.0), Flags::ALL));
    assert!(!ok(&rt, rt.has_own_property(o, &PropKey::Sym(b))));
}

#[test]
fn symbol_for_returns_one_symbol_per_key() {
    let rt = Rt::new();
    let a = rt.symbol_for(&JsStr::from_str("shared"));
    let b = rt.symbol_for(&JsStr::from_str("shared"));
    assert_eq!(a, b);
    let c = rt.symbol_for(&JsStr::from_str("other"));
    assert_ne!(a, c);
    assert_eq!(rt.symbol_description(a), Some(JsStr::from_str("shared")));
}

#[test]
fn symbol_for_is_separate_from_new_symbol() {
    let rt = Rt::new();
    let registered = rt.symbol_for(&JsStr::from_str("k"));
    let fresh = rt.new_symbol(Some(JsStr::from_str("k")));
    assert_ne!(registered, fresh);
}

#[test]
fn descriptions_round_trip_and_absent_descriptions_are_none() {
    let rt = Rt::new();
    let named = rt.new_symbol(Some(JsStr::from_str("name")));
    let anon = rt.new_symbol(None);
    assert_eq!(rt.symbol_description(named), Some(JsStr::from_str("name")));
    assert_eq!(rt.symbol_description(anon), None);
}

#[test]
fn well_known_symbols_are_distinct_and_carry_their_spec_names() {
    let rt = Rt::new();
    let all = [
        WellKnown::Iterator,
        WellKnown::AsyncIterator,
        WellKnown::ToPrimitive,
        WellKnown::ToStringTag,
        WellKnown::HasInstance,
    ];
    let names = ["Symbol.iterator", "Symbol.asyncIterator", "Symbol.toPrimitive", "Symbol.toStringTag", "Symbol.hasInstance"];
    for (i, w) in all.iter().enumerate() {
        let s = rt.well_known(*w);
        assert_eq!(rt.symbol_description(s), Some(JsStr::from_str(names[i])));
        for (j, other) in all.iter().enumerate() {
            if i != j {
                assert_ne!(s, rt.well_known(*other));
            }
        }
    }
}

#[test]
fn well_known_symbols_are_not_registered_under_symbol_for() {
    let rt = Rt::new();
    let reg = rt.symbol_for(&JsStr::from_str("Symbol.iterator"));
    assert_ne!(reg, rt.well_known(WellKnown::Iterator));
}

#[test]
fn a_symbol_key_addresses_only_its_own_property() {
    let rt = Rt::new();
    let o = rt.new_object();
    let s1 = rt.new_symbol(Some(JsStr::from_str("one")));
    let s2 = rt.new_symbol(Some(JsStr::from_str("two")));
    ok(&rt, rt.define_data(o, PropKey::Sym(s1), Value::str("v1"), Flags::ALL));
    assert_eq!(ok(&rt, rt.get(o, &PropKey::Sym(s2), &Value::Object(o))), Value::Undefined);
    assert_eq!(ok(&rt, rt.get(o, &PropKey::Sym(s1), &Value::Object(o))), Value::str("v1"));
}

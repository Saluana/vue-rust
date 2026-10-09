//! WP-B1: Function.prototype (ECMA-262 20.2.3): call, apply, bind, toString, [Symbol.hasInstance], and the bound
//! functions that bind creates. The Function constructor is absent. Expected values follow the spec; the toString
//! shapes and bound names were checked against Node 26 (V8).

use vn_value::{Call, Flags, Intrinsic, Prop, PropKey, Res, Rt, Throw, Value, WellKnown};

fn ok<T>(r: Res<T>) -> T {
    match r {
        Ok(v) => v,
        Err(Throw(v)) => panic!("unexpected JS throw: {:?}", v),
    }
}

fn thrown_name(rt: &Rt, r: Res<Value>) -> String {
    match r {
        Err(Throw(e)) => match rt.get_str(&e, "name") {
            Ok(Value::String(s)) => s.to_rust_string_lossy(),
            _ => String::from("<none>"),
        },
        Ok(v) => panic!("expected a throw, got {:?}", v),
    }
}

fn show(rt: &Rt, v: &Value) -> String {
    match v {
        Value::Undefined => "undefined".into(),
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => {
            if *n == 0.0 && n.is_sign_negative() {
                "-0".into()
            } else {
                vn_value::number::number_to_string(*n)
            }
        }
        Value::String(s) => format!("{:?}", s.to_rust_string_lossy()),
        Value::Symbol(s) => {
            let d = rt.symbol_description(*s).map(|d| d.to_rust_string_lossy()).unwrap_or_default();
            format!("Symbol({d})")
        }
        Value::Object(_) => {
            if rt.is_array(v) {
                let len = match ok(rt.get_str(v, "length")) {
                    Value::Number(n) => n as u32,
                    _ => 0,
                };
                let items: Vec<String> = (0..len).map(|i| show(rt, &ok(rt.get_v(v, &PropKey::Index(i))))).collect();
                format!("[{}]", items.join(","))
            } else {
                // plain objects are only identified by the tests (they compare them by identity)
                "obj".to_string()
            }
        }
        _ => "<internal>".into(),
    }
}

fn string_of(rt: &Rt, v: &Value) -> String {
    match v {
        Value::String(s) => s.to_rust_string_lossy(),
        other => panic!("expected a string, got {}", show(rt, other)),
    }
}

fn global(rt: &Rt) -> Value {
    Value::Object(rt.intrinsic(Intrinsic::Global))
}

fn gl(rt: &Rt, name: &str) -> Value {
    ok(rt.get_str(&global(rt), name))
}

fn getp(rt: &Rt, base: &Value, name: &str) -> Value {
    ok(rt.get_str(base, name))
}

fn n(x: f64) -> Value {
    Value::Number(x)
}

fn s(x: &str) -> Value {
    Value::str(x)
}

fn fp(rt: &Rt) -> Value {
    Value::Object(rt.intrinsic(Intrinsic::FunctionPrototype))
}

/// `Function.prototype[name].call(this, ...args)`.
fn fp_call(rt: &Rt, name: &str, this: &Value, args: &[Value]) -> Res<Value> {
    let f = getp(rt, &fp(rt), name);
    rt.call(&f, this, args)
}

/// `target.bind(thisArg, ...args)` through the prototype.
fn bind(rt: &Rt, target: &Value, this_arg: &Value, args: &[Value]) -> Res<Value> {
    fp_call(rt, "bind", target, &[&[this_arg.clone()][..], args].concat())
}

fn echo(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let mut items = vec![c.this.clone()];
    items.extend(c.args.iter().cloned());
    Ok(Value::Object(rt.new_array(items)))
}

fn three(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Undefined)
}

fn native(rt: &Rt, name: &str, len: u32) -> Value {
    Value::Object(rt.new_native_function(echo, name, len, Vec::new()))
}

fn plain(rt: &Rt, props: &[(&str, Value)]) -> Value {
    let o = rt.new_object();
    for (k, v) in props {
        ok(rt.define_data(o, PropKey::from_str(k), v.clone(), Flags::ALL));
    }
    Value::Object(o)
}

#[test]
fn function_prototype_is_callable_and_empty() {
    let rt = Rt::new();
    let f = fp(&rt);
    assert!(rt.is_callable(&f));
    assert!(!rt.is_constructor(&f));
    assert_eq!(ok(rt.call(&f, &Value::Undefined, &[])), Value::Undefined);
    assert_eq!(getp(&rt, &f, "name"), s(""));
    assert_eq!(getp(&rt, &f, "length"), n(0.0));
    assert_eq!(rt.get_prototype_of(f.as_object().unwrap()), Some(rt.intrinsic(Intrinsic::ObjectPrototype)));
}

#[test]
fn function_constructor_and_constructor_property_are_absent() {
    let rt = Rt::new();
    assert_eq!(gl(&rt, "Function"), Value::Undefined, "no Function global");
    // No own `constructor`. The lookup still finds Object.prototype.constructor through the prototype chain (V8 would
    // say Function); that is a consequence of the absent Function constructor and is reported as a deviation.
    assert!(rt
        .get_own_property(rt.intrinsic(Intrinsic::FunctionPrototype), &PropKey::from_str("constructor"))
        .unwrap()
        .is_none());
    assert!(rt.get_own_property(rt.intrinsic(Intrinsic::FunctionPrototype), &PropKey::from_str("caller")).unwrap().is_none());
    assert!(rt.get_own_property(rt.intrinsic(Intrinsic::FunctionPrototype), &PropKey::from_str("arguments")).unwrap().is_none());
}

#[test]
fn prototype_methods_have_spec_lengths_names_and_attributes() {
    let rt = Rt::new();
    let f = fp(&rt);
    for (name, len) in [("apply", 2.0), ("bind", 1.0), ("call", 1.0), ("toString", 0.0)] {
        let m = getp(&rt, &f, name);
        assert_eq!(getp(&rt, &m, "length"), n(len), "{name}.length");
        assert_eq!(getp(&rt, &m, "name"), s(name), "{name}.name");
        let slot = rt.get_own_property(f.as_object().unwrap(), &PropKey::from_str(name)).unwrap().unwrap();
        assert_eq!(slot.flags(), Flags::HIDDEN, "{name} is writable, non-enumerable, configurable");
    }
    let hi = getp(&rt, &f, "[Symbol.hasInstance]");
    assert_eq!(hi, Value::Undefined, "string key lookup does not find the symbol method");
    let sym_key = PropKey::Sym(rt.well_known(WellKnown::HasInstance));
    let slot = rt.get_own_property(f.as_object().unwrap(), &sym_key).unwrap().unwrap();
    assert_eq!(slot.flags(), Flags::NONE, "Function.prototype[@@hasInstance] is non-writable, non-enumerable, non-configurable");
    let Prop::Data { value, .. } = slot else { panic!("hasInstance is a data property") };
    assert_eq!(getp(&rt, &value, "name"), s("[Symbol.hasInstance]"));
    assert_eq!(getp(&rt, &value, "length"), n(1.0));
}

#[test]
fn call_forwards_this_and_the_remaining_arguments() {
    let rt = Rt::new();
    let target = native(&rt, "echo", 0);
    let this_arg = plain(&rt, &[("x", n(1.0))]);
    let r = fp_call(&rt, "call", &target, &[this_arg.clone(), n(2.0), n(3.0)]).unwrap();
    assert_eq!(show(&rt, &r), "[obj,2,3]");
    let items = ok(rt.get_v(&r, &PropKey::Index(0)));
    assert_eq!(items, this_arg, "the first element is the thisArg object");
    // no arguments at all: this is undefined and no arguments follow
    let r2 = fp_call(&rt, "call", &target, &[]).unwrap();
    assert_eq!(show(&rt, &r2), "[undefined]");
    // thisArg is passed unchanged (no boxing for primitives)
    let r3 = fp_call(&rt, "call", &target, &[s("p")]).unwrap();
    assert_eq!(show(&rt, &r3), r#"["p"]"#);
}

#[test]
fn apply_spreads_an_array_like_and_nullish_gives_no_arguments() {
    let rt = Rt::new();
    let target = native(&rt, "echo", 2);
    let array_like = plain(&rt, &[("length", n(2.0)), ("0", s("a")), ("1", s("b"))]);
    let r = fp_call(&rt, "apply", &target, &[Value::Null, array_like]).unwrap();
    assert_eq!(show(&rt, &r), r#"[null,"a","b"]"#);
    let r2 = fp_call(&rt, "apply", &target, &[Value::Undefined]).unwrap();
    assert_eq!(show(&rt, &r2), "[undefined]");
    let r3 = fp_call(&rt, "apply", &target, &[Value::Null, Value::Null]).unwrap();
    assert_eq!(show(&rt, &r3), "[null]");
    // a real array works too
    let arr = Value::Object(rt.new_array(vec![n(7.0)]));
    let r4 = fp_call(&rt, "apply", &target, &[Value::Null, arr]).unwrap();
    assert_eq!(show(&rt, &r4), "[null,7]");
}

#[test]
fn apply_rejects_non_object_argument_lists_and_huge_lengths() {
    let rt = Rt::new();
    let target = native(&rt, "echo", 0);
    assert_eq!(thrown_name(&rt, fp_call(&rt, "apply", &target, &[Value::Null, n(1.0)])), "TypeError");
    assert_eq!(thrown_name(&rt, fp_call(&rt, "apply", &target, &[Value::Null, s("ab")])), "TypeError");
    assert_eq!(thrown_name(&rt, fp_call(&rt, "apply", &target, &[Value::Null, Value::Bool(true)])), "TypeError");
    let huge = plain(&rt, &[("length", n(5_000_000.0))]);
    assert_eq!(thrown_name(&rt, fp_call(&rt, "apply", &target, &[Value::Null, huge])), "RangeError");
}

#[test]
fn call_and_apply_on_a_non_callable_this_throw_type_error() {
    let rt = Rt::new();
    let obj = plain(&rt, &[]);
    assert_eq!(thrown_name(&rt, fp_call(&rt, "call", &obj, &[])), "TypeError");
    assert_eq!(thrown_name(&rt, fp_call(&rt, "apply", &obj, &[])), "TypeError");
    assert_eq!(thrown_name(&rt, fp_call(&rt, "call", &n(1.0), &[Value::Null])), "TypeError");
    assert_eq!(thrown_name(&rt, fp_call(&rt, "apply", &Value::Undefined, &[])), "TypeError");
}

#[test]
fn bind_sets_name_and_length_from_the_target() {
    let rt = Rt::new();
    let echo_f = native(&rt, "echo", 3);
    let b0 = bind(&rt, &echo_f, &Value::Undefined, &[]).unwrap();
    assert_eq!(getp(&rt, &b0, "name"), s("bound echo"));
    assert_eq!(getp(&rt, &b0, "length"), n(3.0));
    let b1 = bind(&rt, &echo_f, &Value::Undefined, &[n(1.0)]).unwrap();
    assert_eq!(getp(&rt, &b1, "length"), n(2.0));
    let b4 = bind(&rt, &echo_f, &Value::Undefined, &[n(1.0), n(2.0), n(3.0), n(4.0)]).unwrap();
    assert_eq!(getp(&rt, &b4, "length"), n(0.0), "length never goes below zero");

    // bound functions have own length and name, not writable, not enumerable, configurable
    let slot = rt.get_own_property(b1.as_object().unwrap(), &PropKey::from_str("length")).unwrap().unwrap();
    assert_eq!(slot.flags(), Flags::CONFIG);
    let slot_name = rt.get_own_property(b1.as_object().unwrap(), &PropKey::from_str("name")).unwrap().unwrap();
    assert_eq!(slot_name.flags(), Flags::CONFIG);

    // bind of a bound function nests the names
    let nested = bind(&rt, &b1, &Value::Undefined, &[]).unwrap();
    assert_eq!(getp(&rt, &nested, "name"), s("bound bound echo"));
    assert_eq!(getp(&rt, &nested, "length"), n(2.0));
}

#[test]
fn bind_length_follows_the_target_length_property() {
    let rt = Rt::new();
    // no own length: 0
    let f = native(&rt, "f", 5);
    assert!(ok(rt.delete(f.as_object().unwrap(), &PropKey::from_str("length"))));
    let b = bind(&rt, &f, &Value::Undefined, &[]).unwrap();
    assert_eq!(getp(&rt, &b, "length"), n(0.0));

    // a non-number length: 0
    let g = native(&rt, "g", 5);
    ok(rt.define_data(g.as_object().unwrap(), PropKey::from_str("length"), s("3"), Flags::CONFIG));
    assert_eq!(getp(&rt, &bind(&rt, &g, &Value::Undefined, &[]).unwrap(), "length"), n(0.0));

    // fractional lengths are truncated (ToIntegerOrInfinity)
    let h = native(&rt, "h", 0);
    ok(rt.define_data(h.as_object().unwrap(), PropKey::from_str("length"), n(2.7), Flags::CONFIG));
    assert_eq!(getp(&rt, &bind(&rt, &h, &Value::Undefined, &[]).unwrap(), "length"), n(2.0));

    // +Infinity stays infinite; NaN is 0
    let inf = native(&rt, "inf", 0);
    ok(rt.define_data(inf.as_object().unwrap(), PropKey::from_str("length"), n(f64::INFINITY), Flags::CONFIG));
    assert_eq!(getp(&rt, &bind(&rt, &inf, &Value::Undefined, &[n(1.0)]).unwrap(), "length"), n(f64::INFINITY));
    let nan = native(&rt, "nan", 0);
    ok(rt.define_data(nan.as_object().unwrap(), PropKey::from_str("length"), n(f64::NAN), Flags::CONFIG));
    assert_eq!(getp(&rt, &bind(&rt, &nan, &Value::Undefined, &[]).unwrap(), "length"), n(0.0));
}

#[test]
fn bind_name_defaults_to_the_bound_prefix_when_the_target_name_is_not_a_string() {
    let rt = Rt::new();
    let f = native(&rt, "f", 0);
    ok(rt.define_data(f.as_object().unwrap(), PropKey::from_str("name"), n(5.0), Flags::CONFIG));
    assert_eq!(getp(&rt, &bind(&rt, &f, &Value::Undefined, &[]).unwrap(), "name"), s("bound "));
    let g = native(&rt, "g", 0);
    assert!(ok(rt.delete(g.as_object().unwrap(), &PropKey::from_str("name"))));
    assert_eq!(getp(&rt, &bind(&rt, &g, &Value::Undefined, &[]).unwrap(), "name"), s("bound "));
}

#[test]
fn bind_requires_a_callable_this() {
    let rt = Rt::new();
    assert_eq!(thrown_name(&rt, fp_call(&rt, "bind", &plain(&rt, &[]), &[])), "TypeError");
    assert_eq!(thrown_name(&rt, fp_call(&rt, "bind", &Value::Undefined, &[])), "TypeError");
    assert_eq!(thrown_name(&rt, fp_call(&rt, "bind", &s("f"), &[])), "TypeError");
}

fn record_this_and_args(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    echo(rt, c)
}

#[test]
fn bound_function_calls_the_target_with_bound_this_and_arguments() {
    let rt = Rt::new();
    let f = Value::Object(rt.new_native_function(record_this_and_args, "echo", 0, Vec::new()));
    let bound_this = plain(&rt, &[("tag", s("bound"))]);
    let b = bind(&rt, &f, &bound_this, &[n(1.0)]).unwrap();
    let r = rt.call(&b, &s("ignored this"), &[n(2.0), n(3.0)]).unwrap();
    assert_eq!(show(&rt, &r), "[obj,1,2,3]");
    assert_eq!(ok(rt.get_v(&r, &PropKey::Index(0))), bound_this, "the bound this wins over the call-time this");
    // the arguments are bound before the call: callers cannot change them
    let r2 = rt.call(&b, &Value::Undefined, &[]).unwrap();
    assert_eq!(show(&rt, &r2), "[obj,1]");
}

#[test]
fn bound_function_construct_uses_the_target_as_new_target() {
    let rt = Rt::new();
    let boolean = gl(&rt, "Boolean");
    let b = bind(&rt, &boolean, &Value::Null, &[n(0.0)]).unwrap();
    assert!(rt.is_constructor(&b), "a bound constructor is a constructor");
    // new b(1): Boolean(0, 1) constructs a wrapper of false with Boolean.prototype
    let made = ok(rt.construct(&b, &[n(1.0)], &b));
    let proto = getp(&rt, &boolean, "prototype");
    assert_eq!(rt.get_prototype_of(made.as_object().unwrap()), proto.as_object());
    let value_of = getp(&rt, &proto, "valueOf");
    assert_eq!(ok(rt.call(&value_of, &made, &[])), Value::Bool(false));
}

#[test]
fn bound_function_of_a_non_constructor_is_not_a_constructor() {
    let rt = Rt::new();
    let symbol = gl(&rt, "Symbol");
    assert!(!rt.is_constructor(&symbol));
    let b = bind(&rt, &symbol, &Value::Undefined, &[]).unwrap();
    assert!(!rt.is_constructor(&b));
    assert!(rt.is_callable(&b));
    assert_eq!(thrown_name(&rt, rt.construct(&b, &[], &b)), "TypeError");
    // but calling the bound symbol works: Symbol("q") through the bind
    let made = ok(rt.call(&b, &Value::Undefined, &[s("q")]));
    assert!(matches!(made, Value::Symbol(_)));
}

#[test]
fn bound_function_prototype_is_the_target_prototype() {
    let rt = Rt::new();
    let f = native(&rt, "f", 0);
    let proto = plain(&rt, &[("inherited", n(1.0))]);
    rt.set_prototype_of(f.as_object().unwrap(), proto.as_object());
    let b = bind(&rt, &f, &Value::Undefined, &[]).unwrap();
    assert_eq!(rt.get_prototype_of(b.as_object().unwrap()), proto.as_object());
    assert_eq!(getp(&rt, &b, "inherited"), n(1.0));
}

#[test]
fn to_string_prints_the_native_shape() {
    let rt = Rt::new();
    let keys = getp(&rt, &gl(&rt, "Object"), "keys");
    let text = |v: &Value| string_of(&rt, &fp_call(&rt, "toString", v, &[]).unwrap());
    assert_eq!(text(&keys), "function keys() { [native code] }");
    assert_eq!(text(&fp(&rt)), "function () { [native code] }");
    // a bound function prints without a name, as V8 does
    let b = bind(&rt, &keys, &Value::Undefined, &[]).unwrap();
    assert_eq!(text(&b), "function () { [native code] }");
    // a user-visible function that is only a native here: shape with its name
    let f = native(&rt, "named", 1);
    assert_eq!(text(&f), "function named() { [native code] }");
}

#[test]
fn to_string_rejects_non_functions() {
    let rt = Rt::new();
    assert_eq!(thrown_name(&rt, fp_call(&rt, "toString", &plain(&rt, &[]), &[])), "TypeError");
    assert_eq!(thrown_name(&rt, fp_call(&rt, "toString", &Value::Undefined, &[])), "TypeError");
    assert_eq!(thrown_name(&rt, fp_call(&rt, "toString", &n(1.0), &[])), "TypeError");
}

/// `Function.prototype[@@hasInstance].call(ctor, v)`.
fn ordinary_of(rt: &Rt, ctor: &Value, v: &Value) -> Res<Value> {
    let sym_key = PropKey::Sym(rt.well_known(WellKnown::HasInstance));
    let slot = rt.get_own_property(fp(rt).as_object().unwrap(), &sym_key).unwrap().unwrap();
    let Prop::Data { value: hi, .. } = slot else { panic!("data") };
    rt.call(&hi, ctor, std::slice::from_ref(v))
}

#[test]
fn has_instance_follows_the_prototype_chain() {
    let rt = Rt::new();
    let f = native(&rt, "F", 0);
    let proto = rt.new_object();
    ok(rt.define_data(f.as_object().unwrap(), PropKey::from_str("prototype"), Value::Object(proto), Flags::NONE));
    let inst = Value::Object(rt.new_object_with_proto(Some(proto)));
    let grand = Value::Object(rt.new_object_with_proto(inst.as_object()));
    assert_eq!(ordinary_of(&rt, &f, &inst).unwrap(), Value::Bool(true));
    assert_eq!(ordinary_of(&rt, &f, &grand).unwrap(), Value::Bool(true));
    assert_eq!(ordinary_of(&rt, &f, &plain(&rt, &[])).unwrap(), Value::Bool(false));
    // a primitive V is false, without reading the prototype
    assert_eq!(ordinary_of(&rt, &f, &n(1.0)).unwrap(), Value::Bool(false));
    assert_eq!(ordinary_of(&rt, &f, &Value::Undefined).unwrap(), Value::Bool(false));
    // a non-callable this is false
    assert_eq!(ordinary_of(&rt, &plain(&rt, &[]), &inst).unwrap(), Value::Bool(false));
    assert_eq!(ordinary_of(&rt, &Value::Undefined, &inst).unwrap(), Value::Bool(false));
}

#[test]
fn has_instance_throws_when_prototype_is_not_an_object() {
    let rt = Rt::new();
    let f = native(&rt, "F", 0);
    ok(rt.define_data(f.as_object().unwrap(), PropKey::from_str("prototype"), n(1.0), Flags::NONE));
    let inst = plain(&rt, &[]);
    assert_eq!(thrown_name(&rt, ordinary_of(&rt, &f, &inst)), "TypeError");
    // but only for an object V (the primitive check comes first)
    assert_eq!(ordinary_of(&rt, &f, &n(1.0)).unwrap(), Value::Bool(false));
}

#[test]
fn has_instance_of_a_bound_function_asks_its_target() {
    let rt = Rt::new();
    let f = native(&rt, "F", 0);
    let proto = rt.new_object();
    ok(rt.define_data(f.as_object().unwrap(), PropKey::from_str("prototype"), Value::Object(proto), Flags::NONE));
    let b = bind(&rt, &f, &Value::Undefined, &[]).unwrap();
    let inst = Value::Object(rt.new_object_with_proto(Some(proto)));
    assert_eq!(ordinary_of(&rt, &b, &inst).unwrap(), Value::Bool(true));
    assert_eq!(ordinary_of(&rt, &b, &plain(&rt, &[])).unwrap(), Value::Bool(false));
}

#[test]
fn instanceof_operator_for_ordinary_functions() {
    let rt = Rt::new();
    let f = native(&rt, "F", 0);
    let proto = rt.new_object();
    ok(rt.define_data(f.as_object().unwrap(), PropKey::from_str("prototype"), Value::Object(proto), Flags::NONE));
    let inst = Value::Object(rt.new_object_with_proto(Some(proto)));
    assert!(ok(rt.instance_of(&inst, &f)));
    assert!(!ok(rt.instance_of(&plain(&rt, &[]), &f)));
    assert!(!ok(rt.instance_of(&n(1.0), &f)));
    // Object and Function.prototype's hasInstance agree on a built-in: Boolean wrapper is a Boolean
    let boolean = gl(&rt, "Boolean");
    let wrapper = ok(rt.construct(&boolean, &[n(0.0)], &boolean));
    assert!(ok(rt.instance_of(&wrapper, &boolean)));
    assert!(!ok(rt.instance_of(&Value::Bool(false), &boolean)));
    // the right side must be callable
    assert_eq!(thrown_name(&rt, rt.instance_of(&inst, &plain(&rt, &[])).map(Value::Bool)), "TypeError");
}

#[test]
fn bound_function_record_survives_a_collection() {
    // The bound function's arguments are reachable only through its record (its extension slot). A collection
    // between the bind and the call must keep them alive: the record's trace marks the target and arguments.
    let rt = Rt::new();
    let keys = getp(&rt, &gl(&rt, "Object"), "keys");
    let arg = plain(&rt, &[("kept", n(1.0))]);
    let b = bind(&rt, &keys, &Value::Undefined, std::slice::from_ref(&arg)).unwrap();
    drop(arg);
    let root = rt.root(&b);
    rt.collect();
    let r = ok(rt.call(&b, &Value::Undefined, &[]));
    assert_eq!(string_of(&rt, &ok(rt.get_v(&r, &PropKey::Index(0)))), "kept");
    rt.unroot(root);
}

#[test]
fn bound_function_target_survives_a_collection() {
    // A target that only the bound function refers to: the record marks it.
    let rt = Rt::new();
    let target = native(&rt, "only", 0);
    let b = bind(&rt, &target, &Value::Undefined, &[]).unwrap();
    drop(target);
    let root = rt.root(&b);
    rt.collect();
    let r = ok(rt.call(&b, &Value::Undefined, &[n(4.0)]));
    assert_eq!(show(&rt, &r), "[undefined,4]");
    rt.unroot(root);
}

#[test]
fn bound_function_of_a_three_argument_native_is_a_plain_function() {
    let rt = Rt::new();
    let f = Value::Object(rt.new_native_function(three, "three", 3, Vec::new()));
    let b = bind(&rt, &f, &Value::Undefined, &[]).unwrap();
    assert_eq!(getp(&rt, &b, "name"), s("bound three"));
    assert_eq!(getp(&rt, &b, "length"), n(3.0));
    assert!(!rt.is_constructor(&b));
    assert_eq!(ok(rt.call(&b, &Value::Undefined, &[])), Value::Undefined);
}

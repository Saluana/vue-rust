//! WP-B2 constructor, statics and the shape of Array.prototype: call versus construct, new.target, constructor receivers of
//! Array.of and Array.from, the installed names, attributes and lengths, and the methods that must be absent.
//! Expected values are V8's (Node 26) where V8 has the behaviour; deliberate deviations are asserted as such.

use vn_value::{Call, Flags, Intrinsic, PropKey, Prop, Res, Rt, Value, WellKnown};

fn arr(rt: &Rt, elems: Vec<Value>) -> Value {
    Value::Object(rt.new_array(elems))
}

fn method(rt: &Rt, this: &Value, name: &str, args: &[Value]) -> Res<Value> {
    let f = rt.get_str(this, name)?;
    rt.call(&f, this, args)
}

fn text(rt: &Rt, v: &Value) -> String {
    match v {
        Value::Undefined => "undefined".into(),
        Value::Object(_) => "object".into(),
        _ => rt.to_string(v).map(|s| s.to_rust_string_lossy()).unwrap_or_else(|_| "?".into()),
    }
}

fn render(rt: &Rt, v: &Value) -> String {
    match v {
        Value::Object(o) if rt.is_array(v) => {
            let len = match rt.get_str(v, "length") {
                Ok(Value::Number(n)) => n as u32,
                _ => 0,
            };
            let parts: Vec<String> = (0..len)
                .map(|i| {
                    let key = PropKey::Index(i);
                    if rt.has_property(*o, &key).unwrap_or(false) {
                        render(rt, &rt.get_v(v, &key).unwrap_or(Value::Undefined))
                    } else {
                        "<hole>".to_string()
                    }
                })
                .collect();
            format!("[{}]", parts.join(","))
        }
        other => text(rt, other),
    }
}

fn error_name(rt: &Rt, e: &vn_value::Throw) -> String {
    text(rt, &rt.get_str(&e.0, "name").unwrap_or(Value::Undefined))
}

fn global(rt: &Rt, name: &str) -> Value {
    rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), name).expect("global")
}

fn array_ctor(rt: &Rt) -> Value {
    global(rt, "Array")
}

fn noop_call(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_type_error("Other must be constructed"))
}

fn noop_construct(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Object(rt.new_object()))
}

/// A constructor whose instances are ordinary objects, used as the `this` of Array.of and Array.from.
fn box_ctor(rt: &Rt) -> Value {
    let f = rt.new_native_constructor(noop_call, noop_construct, "Box", 1, Vec::new());
    let p = rt.new_object();
    rt.define_data(f, PropKey::from_str("prototype"), Value::Object(p), Flags::NONE).expect("prototype");
    Value::Object(f)
}

// ----- call, construct and length rules -----

#[test]
fn call_and_construct_build_the_same_holey_array() {
    // V8: Array(3) and new Array(3) both have length 3 and no elements.
    let rt = Rt::new();
    let ctor = array_ctor(&rt);
    let called = rt.call(&ctor, &Value::Undefined, &[Value::Number(3.0)]).expect("call");
    let constructed = rt.construct(&ctor, &[Value::Number(3.0)], &ctor).expect("construct");
    for v in [called, constructed] {
        assert_eq!(rt.get_str(&v, "length").expect("length"), Value::Number(3.0));
        assert_eq!(render(&rt, &v), "[<hole>,<hole>,<hole>]");
    }
    let two = rt.call(&ctor, &Value::Undefined, &[Value::Number(1.0), Value::Number(2.0)]).expect("call");
    assert_eq!(render(&rt, &two), "[1,2]");
    let s = rt.call(&ctor, &Value::Undefined, &[Value::str("3")]).expect("call");
    assert_eq!(render(&rt, &s), "[3]");
}

#[test]
fn invalid_length_arguments_are_range_errors() {
    // V8: 1.5, -1, NaN and 2^32 are not valid lengths. 4194305 is valid in V8 but beyond this runtime's dense limit
    // (ABI.md section 16), so it is a RangeError here as a deliberate deviation.
    let rt = Rt::new();
    let ctor = array_ctor(&rt);
    for bad in [1.5, -1.0, f64::NAN, 4_294_967_296.0, 4_194_305.0] {
        let err = rt.call(&ctor, &Value::Undefined, &[Value::Number(bad)]).expect_err("RangeError expected");
        assert_eq!(error_name(&rt, &err), "RangeError", "length {bad}");
    }
}

#[test]
fn new_target_chooses_the_prototype_of_the_new_array() {
    // The prototype of the constructed array is new.target.prototype (GetPrototypeFromConstructor).
    let rt = Rt::new();
    let other = box_ctor(&rt);
    let proto = match rt.get_str(&other, "prototype").expect("prototype") {
        Value::Object(p) => p,
        _ => panic!("prototype is an object"),
    };
    let out = rt.construct(&array_ctor(&rt), &[], &other).expect("construct with new.target");
    assert_eq!(rt.get_prototype_of(out.as_object().expect("object")), Some(proto));
}

#[test]
fn array_of_uses_the_constructor_it_is_called_on() {
    // V8: Array.of.call(Box, 1, 2) constructs Box with the length and defines the elements on the result.
    let rt = Rt::new();
    let box_c = box_ctor(&rt);
    let of = rt.get_str(&array_ctor(&rt), "of").expect("of");
    let out = rt.call(&of, &box_c, &[Value::Number(1.0), Value::Number(2.0)]).expect("of");
    assert!(!rt.is_array(&out), "a Box is not an array");
    assert_eq!(rt.get_str(&out, "length").expect("length"), Value::Number(2.0));
    assert_eq!(rt.get_str(&out, "0").expect("0"), Value::Number(1.0));
    assert_eq!(rt.get_str(&out, "1").expect("1"), Value::Number(2.0));
}

#[test]
fn array_from_array_like_uses_the_constructor_it_is_called_on() {
    let rt = Rt::new();
    let box_c = box_ctor(&rt);
    let from = rt.get_str(&array_ctor(&rt), "from").expect("from");
    let src = rt.new_object();
    rt.define_data(src, PropKey::from_str("length"), Value::Number(2.0), Flags::ALL).expect("length");
    rt.define_data(src, PropKey::from_str("0"), Value::str("x"), Flags::ALL).expect("0");
    let out = rt.call(&from, &box_c, &[Value::Object(src)]).expect("from");
    assert!(!rt.is_array(&out));
    assert_eq!(rt.get_str(&out, "length").expect("length"), Value::Number(2.0));
    assert_eq!(rt.get_str(&out, "0").expect("0"), Value::str("x"));
}

#[test]
fn is_array_is_true_for_the_array_prototype_and_false_for_objects() {
    // V8: Array.isArray(Array.prototype) is true (the prototype is itself an array).
    let rt = Rt::new();
    let is_array = rt.get_str(&array_ctor(&rt), "isArray").expect("isArray");
    let ap = Value::Object(rt.intrinsic(Intrinsic::ArrayPrototype));
    assert_eq!(rt.call(&is_array, &Value::Undefined, &[ap]).expect("isArray"), Value::Bool(true));
    let plain = Value::Object(rt.new_object());
    assert_eq!(rt.call(&is_array, &Value::Undefined, &[plain]).expect("isArray"), Value::Bool(false));
}

// ----- the installed shape -----

/// The string-keyed own properties of Array.prototype: the WP list (38 names: `length`, `constructor` and 36 methods).
const PROTO_NAMES: [&str; 38] = [
    "length", "constructor", "at", "concat", "entries", "every", "fill", "filter", "find", "findIndex", "findLast",
    "findLastIndex", "flat", "flatMap", "forEach", "includes", "indexOf", "join", "keys", "lastIndexOf", "map", "pop",
    "push", "reduce", "reduceRight", "reverse", "shift", "slice", "some", "sort", "splice", "toReversed", "toSorted",
    "toSpliced", "toString", "unshift", "values", "with",
];

#[test]
fn prototype_has_exactly_the_wp_names_and_one_symbol() {
    // V8 also has copyWithin, toLocaleString and Symbol.unscopables; the WP omits them, so they must be absent here.
    let rt = Rt::new();
    let ap = rt.intrinsic(Intrinsic::ArrayPrototype);
    let mut names: Vec<String> = rt
        .own_keys(ap)
        .expect("own keys")
        .into_iter()
        .filter_map(|k| match k {
            PropKey::Str(s) => Some(s.to_rust_string_lossy()),
            _ => None,
        })
        .collect();
    names.sort();
    let mut want: Vec<String> = PROTO_NAMES.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(names, want);
    let symbols = rt.own_keys(ap).expect("own keys").into_iter().filter(|k| matches!(k, PropKey::Sym(_))).count();
    assert_eq!(symbols, 1, "only Symbol.iterator");
    for absent in ["copyWithin", "toLocaleString"] {
        assert!(!rt.has_own_property(ap, &PropKey::from_str(absent)).expect("own"), "{absent} is absent");
    }
}

#[test]
fn methods_are_writable_non_enumerable_and_configurable() {
    let rt = Rt::new();
    let ap = rt.intrinsic(Intrinsic::ArrayPrototype);
    for name in PROTO_NAMES.iter().filter(|n| **n != "length" && **n != "constructor") {
        match rt.get_own_property(ap, &PropKey::from_str(name)).expect("own") {
            Some(Prop::Data { flags, .. }) => assert_eq!(flags, Flags::HIDDEN, "{name}"),
            other => panic!("{name} is a data property, found {other:?}"),
        }
    }
    let values_fn = rt.get_str(&Value::Object(ap), "values").expect("values");
    let sym = rt.get_v(&Value::Object(ap), &PropKey::Sym(rt.well_known(WellKnown::Iterator))).expect("@@iterator");
    assert_eq!(sym, values_fn);
}

#[test]
fn constructor_and_prototype_are_linked_with_their_attributes() {
    let rt = Rt::new();
    let ctor = array_ctor(&rt);
    let ap = rt.intrinsic(Intrinsic::ArrayPrototype);
    assert_eq!(rt.get_str(&ctor, "prototype").expect("prototype"), Value::Object(ap));
    assert_eq!(rt.get_str(&Value::Object(ap), "constructor").expect("constructor"), ctor);
    match rt.get_own_property(ctor.as_object().expect("object"), &PropKey::from_str("prototype")).expect("own") {
        Some(Prop::Data { flags, .. }) => assert_eq!(flags, Flags::NONE, "prototype is read-only"),
        other => panic!("prototype is a data property, found {other:?}"),
    }
    assert_eq!(rt.get_str(&ctor, "name").expect("name"), Value::str("Array"));
    assert_eq!(rt.get_str(&ctor, "length").expect("length"), Value::Number(1.0));
}

#[test]
fn method_names_and_lengths_match_the_spec() {
    // Lengths as in ECMA-262 (and V8).
    let rt = Rt::new();
    let ap = Value::Object(rt.intrinsic(Intrinsic::ArrayPrototype));
    let table: [(&str, f64); 36] = [
        ("at", 1.0), ("concat", 1.0), ("entries", 0.0), ("every", 1.0), ("fill", 1.0), ("filter", 1.0), ("find", 1.0),
        ("findIndex", 1.0), ("findLast", 1.0), ("findLastIndex", 1.0), ("flat", 0.0), ("flatMap", 1.0),
        ("forEach", 1.0), ("includes", 1.0), ("indexOf", 1.0), ("join", 1.0), ("keys", 0.0), ("lastIndexOf", 1.0),
        ("map", 1.0), ("pop", 0.0), ("push", 1.0), ("reduce", 1.0), ("reduceRight", 1.0), ("reverse", 0.0),
        ("shift", 0.0), ("slice", 2.0), ("some", 1.0), ("sort", 1.0), ("splice", 2.0), ("toReversed", 0.0),
        ("toSorted", 1.0), ("toSpliced", 2.0), ("toString", 0.0), ("unshift", 1.0), ("values", 0.0), ("with", 2.0),
    ];
    for (name, len) in table {
        let f = rt.get_str(&ap, name).expect(name);
        assert_eq!(rt.get_str(&f, "length").expect("length"), Value::Number(len), "{name}.length");
        assert_eq!(rt.get_str(&f, "name").expect("name"), Value::str(name), "{name}.name");
    }
    let ctor = array_ctor(&rt);
    for (name, len) in [("isArray", 1.0), ("of", 0.0), ("from", 1.0)] {
        let f = rt.get_str(&ctor, name).expect(name);
        assert_eq!(rt.get_str(&f, "length").expect("length"), Value::Number(len), "Array.{name}.length");
    }
}

#[test]
fn array_prototype_is_an_empty_array() {
    let rt = Rt::new();
    let ap = Value::Object(rt.intrinsic(Intrinsic::ArrayPrototype));
    assert_eq!(rt.get_str(&ap, "length").expect("length"), Value::Number(0.0));
    let _ = method(&rt, &arr(&rt, vec![]), "toString", &[]).expect("toString");
}

//! WP-B1: the Object function, its statics and Object.prototype (ECMA-262 20.1). Expected values are written from the
//! spec and checked against Node 26 (V8) where the text says so. `show` renders a value canonically: strings quoted,
//! arrays as `[a,b]`, own enumerable keys as `{k:v}`, functions as `fn:name/length`.

use vn_value::{Call, ObjClass, ObjRef, Flags, Intrinsic, Prop, PropKey, Res, Rt, Throw, Value, WellKnown};

fn ok<T>(r: Res<T>) -> T {
    match r {
        Ok(v) => v,
        Err(Throw(v)) => panic!("unexpected JS throw: {}", describe_error(&v)),
    }
}

fn describe_error(v: &Value) -> String {
    format!("{:?}", v)
}

/// The `name` of the thrown error, or panics when the call did not throw.
fn thrown_name(rt: &Rt, r: Res<Value>) -> String {
    match r {
        Err(Throw(e)) => match rt.get_str(&e, "name") {
            Ok(Value::String(s)) => s.to_rust_string_lossy(),
            _ => String::from("<none>"),
        },
        Ok(v) => panic!("expected a throw, got {}", show(rt, &v)),
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
        Value::Object(o) => show_object(rt, *o, v),
        _ => "<internal>".into(),
    }
}

fn show_object(rt: &Rt, o: ObjRef, v: &Value) -> String {
    if rt.is_callable(v) {
        let name = ok(rt.get_str(v, "name"));
        let len = ok(rt.get_str(v, "length"));
        return format!("fn:{}/{}", show(rt, &name).trim_matches('"'), show(rt, &len));
    }
    if rt.is_array(v) {
        let len = match ok(rt.get_str(v, "length")) {
            Value::Number(n) => n as u32,
            _ => 0,
        };
        let items: Vec<String> = (0..len).map(|i| show(rt, &ok(rt.get_v(v, &PropKey::Index(i))))).collect();
        return format!("[{}]", items.join(","));
    }
    let mut parts = Vec::new();
    for k in ok_keys(rt, o) {
        let Some(p) = ok_prop(rt, o, &k) else { continue };
        if !p.flags().enumerable {
            continue;
        }
        let key = match &k {
            PropKey::Index(i) => i.to_string(),
            PropKey::Str(s) => s.to_rust_string_lossy(),
            PropKey::Sym(_) => continue,
        };
        let text = match p {
            Prop::Data { value, .. } => show(rt, &value),
            Prop::Accessor { .. } => "<accessor>".into(),
        };
        parts.push(format!("{key}:{text}"));
    }
    format!("{{{}}}", parts.join(","))
}

fn ok_keys(rt: &Rt, o: ObjRef) -> Vec<PropKey> {
    match rt.own_keys(o) {
        Ok(k) => k,
        Err(Throw(v)) => panic!("own_keys threw: {}", describe_error(&v)),
    }
}

fn ok_prop(rt: &Rt, o: ObjRef, k: &PropKey) -> Option<Prop> {
    match rt.get_own_property(o, k) {
        Ok(p) => p,
        Err(Throw(v)) => panic!("get_own_property threw: {}", describe_error(&v)),
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

fn s(x: &str) -> Value {
    Value::str(x)
}

fn n(x: f64) -> Value {
    Value::Number(x)
}

fn arr(rt: &Rt, items: Vec<Value>) -> Value {
    Value::Object(rt.new_array(items))
}

/// A plain object with the given enumerable data properties, in order.
fn plain(rt: &Rt, props: &[(&str, Value)]) -> Value {
    let o = rt.new_object();
    for (k, v) in props {
        ok(rt.define_data(o, PropKey::from_str(k), v.clone(), Flags::ALL));
    }
    Value::Object(o)
}

/// `Object[name](...args)`.
fn static_call(rt: &Rt, name: &str, args: &[Value]) -> Res<Value> {
    let ctor = gl(rt, "Object");
    let f = getp(rt, &ctor, name);
    rt.call(&f, &ctor, args)
}

/// `Object.prototype[name].call(this, ...args)`.
fn proto_call(rt: &Rt, name: &str, this: &Value, args: &[Value]) -> Res<Value> {
    let p = Value::Object(rt.intrinsic(Intrinsic::ObjectPrototype));
    let f = getp(rt, &p, name);
    rt.call(&f, this, args)
}

fn echo(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let mut items = vec![c.this.clone()];
    items.extend(c.args.iter().cloned());
    Ok(Value::Object(rt.new_array(items)))
}

// ----- Object itself -----

#[test]
fn object_constructor_shape() {
    let rt = Rt::new();
    let ctor = gl(&rt, "Object");
    let Value::Object(c) = ctor.clone() else { panic!("Object is not an object") };
    assert!(rt.is_callable(&ctor));
    assert!(rt.is_constructor(&ctor));
    assert_eq!(show(&rt, &getp(&rt, &ctor, "length")), "1");
    assert_eq!(show(&rt, &getp(&rt, &ctor, "name")), "\"Object\"");
    let proto = getp(&rt, &ctor, "prototype");
    assert_eq!(proto, Value::Object(rt.intrinsic(Intrinsic::ObjectPrototype)));
    let back = getp(&rt, &proto, "constructor");
    assert_eq!(back, Value::Object(c));
    // prototype is non-writable, non-enumerable, non-configurable
    let p = rt.get_own_property(c, &PropKey::from_str("prototype")).unwrap().unwrap();
    assert_eq!(p.flags(), Flags::NONE);
    // the realm binds the global `Object`; no `defineProperty` etc.
    for absent in ["defineProperty", "defineProperties", "setPrototypeOf"] {
        assert!(ok(rt.get_str(&ctor, absent)) == Value::Undefined, "{absent} must be absent");
    }
}

#[test]
fn object_call_and_construct_coerce_like_the_spec() {
    let rt = Rt::new();
    let ctor = gl(&rt, "Object");
    let op = Value::Object(rt.intrinsic(Intrinsic::ObjectPrototype));

    // Object() and Object(undefined|null) are fresh ordinary objects
    let fresh = ok(rt.call(&ctor, &Value::Undefined, &[]));
    assert_eq!(rt.class_of(fresh.as_object().unwrap()), ObjClass::Ordinary);
    assert_eq!(rt.get_prototype_of(fresh.as_object().unwrap()), op.as_object());
    let from_null = ok(rt.call(&ctor, &Value::Undefined, &[Value::Null]));
    assert_eq!(rt.get_prototype_of(from_null.as_object().unwrap()), op.as_object());

    // Object(obj) is the same object
    let o = plain(&rt, &[("a", n(1.0))]);
    let same = ok(rt.call(&ctor, &Value::Undefined, std::slice::from_ref(&o)));
    assert_eq!(same, o);

    // Object(primitive) is a wrapper of that class
    let wrap = ok(rt.call(&ctor, &Value::Undefined, &[s("ab")]));
    assert_eq!(rt.class_of(wrap.as_object().unwrap()), ObjClass::Wrapper);
    assert_eq!(rt.get_str(&wrap, "length").unwrap(), n(2.0));

    // new Object(v) behaves like Object(v) when new.target is Object itself
    let made = ok(rt.construct(&ctor, &[n(5.0)], &ctor));
    assert_eq!(rt.class_of(made.as_object().unwrap()), ObjClass::Wrapper);
    let made_empty = ok(rt.construct(&ctor, &[], &ctor));
    assert_eq!(rt.get_prototype_of(made_empty.as_object().unwrap()), op.as_object());
    assert_eq!(rt.class_of(made_empty.as_object().unwrap()), ObjClass::Ordinary);
}

#[test]
fn keys_values_entries_follow_own_key_order() {
    let rt = Rt::new();
    // integer keys ascending, then strings in insertion order
    let o = plain(&rt, &[("b", n(1.0)), ("2", n(2.0)), ("a", n(3.0)), ("1", n(4.0))]);
    assert_eq!(show(&rt, &static_call(&rt, "keys", &[o.clone()]).unwrap()), r#"["1","2","b","a"]"#);
    assert_eq!(show(&rt, &static_call(&rt, "values", &[o.clone()]).unwrap()), "[4,2,1,3]");
    assert_eq!(
        show(&rt, &static_call(&rt, "entries", &[o]).unwrap()),
        r#"[["1",4],["2",2],["b",1],["a",3]]"#
    );
}

#[test]
fn keys_skip_symbols_and_hidden_properties() {
    let rt = Rt::new();
    let o = rt.new_object();
    ok(rt.define_data(o, PropKey::from_str("shown"), n(1.0), Flags::ALL));
    ok(rt.define_data(o, PropKey::from_str("hidden"), n(2.0), Flags::HIDDEN));
    let sym = rt.new_symbol(None);
    ok(rt.define_data(o, PropKey::Sym(sym), n(3.0), Flags::ALL));
    let keys = static_call(&rt, "keys", &[Value::Object(o)]).unwrap();
    assert_eq!(show(&rt, &keys), r#"["shown"]"#);
    let vals = static_call(&rt, "values", &[Value::Object(o)]).unwrap();
    assert_eq!(show(&rt, &vals), "[1]");
}

#[test]
fn keys_of_primitives_and_arrays() {
    let rt = Rt::new();
    let a = arr(&rt, vec![s("x"), s("y")]);
    assert_eq!(show(&rt, &static_call(&rt, "keys", &[a]).unwrap()), r#"["0","1"]"#);
    assert_eq!(show(&rt, &static_call(&rt, "keys", &[s("ab")]).unwrap()), r#"["0","1"]"#);
    assert_eq!(show(&rt, &static_call(&rt, "values", &[s("ab")]).unwrap()), r#"["a","b"]"#);
    assert_eq!(show(&rt, &static_call(&rt, "keys", &[n(5.0)]).unwrap()), "[]");
    assert_eq!(show(&rt, &static_call(&rt, "keys", &[Value::Bool(true)]).unwrap()), "[]");
    // array length is not enumerable
    let len_keys = static_call(&rt, "keys", &[arr(&rt, vec![n(1.0)])]).unwrap();
    assert_eq!(show(&rt, &len_keys), r#"["0"]"#);
}

#[test]
fn keys_values_entries_throw_on_nullish() {
    let rt = Rt::new();
    for name in ["keys", "values", "entries"] {
        assert_eq!(thrown_name(&rt, static_call(&rt, name, &[Value::Undefined])), "TypeError", "{name}(undefined)");
        assert_eq!(thrown_name(&rt, static_call(&rt, name, &[Value::Null])), "TypeError", "{name}(null)");
        assert_eq!(thrown_name(&rt, static_call(&rt, name, &[])), "TypeError", "{name}()");
    }
}

fn getter_deletes_b(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let this_obj = c.this.as_object().expect("receiver");
    rt.delete(this_obj, &PropKey::from_str("b"))?;
    Ok(n(1.0))
}

#[test]
fn values_recheck_enumerability_after_an_earlier_getter() {
    // V8: Object.values({ get a() { delete this.b; return 1 }, b: 2 }) is [1]. The key list is taken first, but
    // b is no longer an own property when its turn comes, so it is skipped.
    let rt = Rt::new();
    let o = rt.new_object();
    let getter = rt.new_native_function(getter_deletes_b, "a", 0, Vec::new());
    ok(rt.define_accessor(o, PropKey::from_str("a"), Some(getter), None, Flags::new(false, true, true)));
    ok(rt.define_data(o, PropKey::from_str("b"), n(2.0), Flags::ALL));
    let vals = static_call(&rt, "values", &[Value::Object(o)]).unwrap();
    assert_eq!(show(&rt, &vals), "[1]");
}

#[test]
fn assign_copies_enumerable_props_including_symbols_and_skips_nullish() {
    let rt = Rt::new();
    let target = plain(&rt, &[("a", n(0.0))]);
    let sym = rt.new_symbol(None);
    let src = rt.new_object();
    ok(rt.define_data(src, PropKey::from_str("hidden"), n(9.0), Flags::HIDDEN));
    ok(rt.define_data(src, PropKey::from_str("b"), n(2.0), Flags::ALL));
    ok(rt.define_data(src, PropKey::Sym(sym), s("sym"), Flags::ALL));
    let r = static_call(&rt, "assign", &[target.clone(), Value::Null, Value::Object(src), Value::Undefined, s("xy")]);
    let r = r.unwrap();
    assert_eq!(r, target, "assign returns the target object");
    // integer keys come first in own-key order, then strings in insertion order
    assert_eq!(show(&rt, &target), r#"{0:"x",1:"y",a:0,b:2}"#);
    let sym_val = ok(rt.get(target.as_object().unwrap(), &PropKey::Sym(sym), &target));
    assert_eq!(sym_val, s("sym"));
}

#[test]
fn assign_throws_when_a_target_property_is_read_only() {
    let rt = Rt::new();
    let target = rt.new_object();
    ok(rt.define_data(target, PropKey::from_str("a"), n(0.0), Flags::NONE));
    let src = plain(&rt, &[("a", n(1.0))]);
    assert_eq!(thrown_name(&rt, static_call(&rt, "assign", &[Value::Object(target), src])), "TypeError");
}

#[test]
fn assign_coerces_the_target_and_throws_on_nullish() {
    let rt = Rt::new();
    assert_eq!(thrown_name(&rt, static_call(&rt, "assign", &[])), "TypeError");
    assert_eq!(thrown_name(&rt, static_call(&rt, "assign", &[Value::Undefined])), "TypeError");
    assert_eq!(thrown_name(&rt, static_call(&rt, "assign", &[Value::Null, plain(&rt, &[])])), "TypeError");
    // a primitive target becomes a wrapper object, which is returned
    let r = static_call(&rt, "assign", &[n(1.0)]).unwrap();
    assert_eq!(rt.class_of(r.as_object().unwrap()), ObjClass::Wrapper);
}

#[test]
fn freeze_seal_and_prevent_extensions_round_trip() {
    let rt = Rt::new();
    let o = plain(&rt, &[("a", n(1.0))]);
    let ob = o.as_object().unwrap();
    assert_eq!(static_call(&rt, "isFrozen", &[o.clone()]).unwrap(), Value::Bool(false));
    assert_eq!(static_call(&rt, "isExtensible", &[o.clone()]).unwrap(), Value::Bool(true));
    let same = static_call(&rt, "freeze", &[o.clone()]).unwrap();
    assert_eq!(same, o, "freeze returns its argument");
    assert_eq!(static_call(&rt, "isFrozen", &[o.clone()]).unwrap(), Value::Bool(true));
    assert_eq!(static_call(&rt, "isSealed", &[o.clone()]).unwrap(), Value::Bool(true));
    assert_eq!(static_call(&rt, "isExtensible", &[o.clone()]).unwrap(), Value::Bool(false));
    assert!(!rt.is_extensible(ob).expect("is_extensible"));

    // seal: non-extensible and non-configurable, but writable data stays writable
    let p = plain(&rt, &[("a", n(1.0))]);
    static_call(&rt, "seal", &[p.clone()]).unwrap();
    assert_eq!(static_call(&rt, "isSealed", &[p.clone()]).unwrap(), Value::Bool(true));
    assert_eq!(static_call(&rt, "isFrozen", &[p.clone()]).unwrap(), Value::Bool(false));

    // preventExtensions: only extensibility changes
    let q = plain(&rt, &[]);
    let back = static_call(&rt, "preventExtensions", &[q.clone()]).unwrap();
    assert_eq!(back, q);
    assert_eq!(static_call(&rt, "isExtensible", &[q.clone()]).unwrap(), Value::Bool(false));
    // no own properties and not extensible: frozen
    assert_eq!(static_call(&rt, "isFrozen", &[q.clone()]).unwrap(), Value::Bool(true));
}

#[test]
fn integrity_functions_on_primitives_follow_the_spec() {
    let rt = Rt::new();
    assert_eq!(static_call(&rt, "freeze", &[n(1.0)]).unwrap(), n(1.0));
    assert_eq!(static_call(&rt, "seal", &[s("x")]).unwrap(), s("x"));
    assert_eq!(static_call(&rt, "preventExtensions", &[Value::Undefined]).unwrap(), Value::Undefined);
    assert_eq!(static_call(&rt, "isFrozen", &[n(1.0)]).unwrap(), Value::Bool(true));
    assert_eq!(static_call(&rt, "isFrozen", &[Value::Null]).unwrap(), Value::Bool(true));
    assert_eq!(static_call(&rt, "isSealed", &[s("x")]).unwrap(), Value::Bool(true));
    assert_eq!(static_call(&rt, "isExtensible", &[n(1.0)]).unwrap(), Value::Bool(false));
    assert_eq!(static_call(&rt, "isExtensible", &[Value::Undefined]).unwrap(), Value::Bool(false));
}

#[test]
fn freeze_fixes_array_elements_and_length() {
    let rt = Rt::new();
    let a = arr(&rt, vec![n(1.0), n(2.0)]);
    static_call(&rt, "freeze", &[a.clone()]).unwrap();
    assert_eq!(static_call(&rt, "isFrozen", &[a.clone()]).unwrap(), Value::Bool(true));
    let d = static_call(&rt, "getOwnPropertyDescriptor", &[a.clone(), s("0")]).unwrap();
    assert_eq!(show(&rt, &getp(&rt, &d, "writable")), "false");
    assert_eq!(show(&rt, &getp(&rt, &d, "configurable")), "false");
}

#[test]
fn is_has_own_and_get_prototype_of() {
    let rt = Rt::new();
    assert_eq!(static_call(&rt, "is", &[n(f64::NAN), n(f64::NAN)]).unwrap(), Value::Bool(true));
    assert_eq!(static_call(&rt, "is", &[n(0.0), n(-0.0)]).unwrap(), Value::Bool(false));
    assert_eq!(static_call(&rt, "is", &[n(-0.0), n(-0.0)]).unwrap(), Value::Bool(true));
    assert_eq!(static_call(&rt, "is", &[]).unwrap(), Value::Bool(true), "is(undefined, undefined)");
    assert_eq!(static_call(&rt, "is", &[s("a"), s("a")]).unwrap(), Value::Bool(true));

    let own = plain(&rt, &[("a", n(1.0))]);
    assert_eq!(static_call(&rt, "hasOwn", &[own.clone(), s("a")]).unwrap(), Value::Bool(true));
    assert_eq!(static_call(&rt, "hasOwn", &[own.clone(), s("toString")]).unwrap(), Value::Bool(false));
    let child = rt.new_object_with_proto(own.as_object());
    assert_eq!(static_call(&rt, "hasOwn", &[Value::Object(child), s("a")]).unwrap(), Value::Bool(false));
    assert_eq!(static_call(&rt, "hasOwn", &[arr(&rt, vec![n(7.0)]), n(0.0)]).unwrap(), Value::Bool(true));
    assert_eq!(static_call(&rt, "hasOwn", &[s("ab"), n(1.0)]).unwrap(), Value::Bool(true));
    assert_eq!(static_call(&rt, "hasOwn", &[s("ab"), n(2.0)]).unwrap(), Value::Bool(false));
    assert_eq!(thrown_name(&rt, static_call(&rt, "hasOwn", &[Value::Null, s("x")])), "TypeError");
    // a missing key is ToPropertyKey(undefined), i.e. the property "undefined", which is not there
    assert_eq!(static_call(&rt, "hasOwn", &[own.clone()]).unwrap(), Value::Bool(false));

    // getPrototypeOf
    let op = Value::Object(rt.intrinsic(Intrinsic::ObjectPrototype));
    assert_eq!(static_call(&rt, "getPrototypeOf", &[plain(&rt, &[])]).unwrap(), op);
    assert_eq!(static_call(&rt, "getPrototypeOf", &[n(1.0)]).unwrap(), Value::Object(rt.intrinsic(Intrinsic::NumberPrototype)));
    assert_eq!(static_call(&rt, "getPrototypeOf", &[Value::Object(child)]).unwrap(), own);
    assert_eq!(thrown_name(&rt, static_call(&rt, "getPrototypeOf", &[Value::Undefined])), "TypeError");
    assert_eq!(thrown_name(&rt, static_call(&rt, "getPrototypeOf", &[Value::Null])), "TypeError");
}

#[test]
fn create_with_null_or_object_prototype() {
    let rt = Rt::new();
    let null_proto = static_call(&rt, "create", &[Value::Null]).unwrap();
    assert_eq!(rt.get_prototype_of(null_proto.as_object().unwrap()), None);
    assert_eq!(show(&rt, &null_proto), "{}");

    let parent = plain(&rt, &[("inherited", n(1.0))]);
    let child = static_call(&rt, "create", &[parent.clone()]).unwrap();
    assert_eq!(rt.get_prototype_of(child.as_object().unwrap()), parent.as_object());
    assert_eq!(ok(rt.get_str(&child, "inherited")), n(1.0));

    // an undefined properties argument is accepted
    let with_undef = static_call(&rt, "create", &[Value::Null, Value::Undefined]).unwrap();
    assert_eq!(rt.get_prototype_of(with_undef.as_object().unwrap()), None);
}

#[test]
fn create_rejects_a_non_object_prototype_and_any_properties_argument() {
    let rt = Rt::new();
    assert_eq!(thrown_name(&rt, static_call(&rt, "create", &[])), "TypeError");
    assert_eq!(thrown_name(&rt, static_call(&rt, "create", &[n(1.0)])), "TypeError");
    assert_eq!(thrown_name(&rt, static_call(&rt, "create", &[s("x")])), "TypeError");
    assert_eq!(thrown_name(&rt, static_call(&rt, "create", &[Value::Bool(false)])), "TypeError");
    // defineProperties is not implemented: any other properties argument is refused, even an empty object
    let empty = plain(&rt, &[]);
    assert_eq!(thrown_name(&rt, static_call(&rt, "create", &[Value::Null, empty])), "TypeError");
    assert_eq!(thrown_name(&rt, static_call(&rt, "create", &[Value::Null, Value::Null])), "TypeError");
}

#[test]
fn own_property_names_include_hidden_properties_and_length() {
    let rt = Rt::new();
    let o = rt.new_object();
    ok(rt.define_data(o, PropKey::from_str("b"), n(1.0), Flags::ALL));
    ok(rt.define_data(o, PropKey::from_str("h"), n(2.0), Flags::HIDDEN));
    ok(rt.define_data(o, PropKey::Sym(rt.new_symbol(None)), n(3.0), Flags::ALL));
    ok(rt.define_data(o, PropKey::Index(0), n(4.0), Flags::ALL));
    assert_eq!(
        show(&rt, &static_call(&rt, "getOwnPropertyNames", &[Value::Object(o)]).unwrap()),
        r#"["0","b","h"]"#
    );
    assert_eq!(
        show(&rt, &static_call(&rt, "getOwnPropertyNames", &[arr(&rt, vec![n(1.0)])]).unwrap()),
        r#"["0","length"]"#
    );
    assert_eq!(
        show(&rt, &static_call(&rt, "getOwnPropertyNames", &[s("ab")]).unwrap()),
        r#"["0","1","length"]"#
    );
    assert_eq!(thrown_name(&rt, static_call(&rt, "getOwnPropertyNames", &[Value::Null])), "TypeError");
}

#[test]
fn own_property_symbols_list_only_symbol_keys() {
    let rt = Rt::new();
    let o = rt.new_object();
    let a = rt.new_symbol(Some(vn_value::JsStr::from_str("a")));
    let b = rt.new_symbol(None);
    ok(rt.define_data(o, PropKey::from_str("name"), n(1.0), Flags::ALL));
    ok(rt.define_data(o, PropKey::Sym(a), n(2.0), Flags::ALL));
    ok(rt.define_data(o, PropKey::Sym(b), n(3.0), Flags::HIDDEN));
    let got = static_call(&rt, "getOwnPropertySymbols", &[Value::Object(o)]).unwrap();
    assert_eq!(show(&rt, &got), "[Symbol(a),Symbol()]");
    assert_eq!(show(&rt, &static_call(&rt, "getOwnPropertySymbols", &[plain(&rt, &[])]).unwrap()), "[]");
}

#[test]
fn descriptor_reads_data_accessor_missing_and_array_length() {
    let rt = Rt::new();
    let o = plain(&rt, &[("a", n(1.0))]);
    let d = static_call(&rt, "getOwnPropertyDescriptor", &[o.clone(), s("a")]).unwrap();
    assert_eq!(show(&rt, &d), "{value:1,writable:true,enumerable:true,configurable:true}");
    let keys: Vec<String> = ok_keys(&rt, d.as_object().unwrap()).iter().map(|k| format!("{k:?}")).collect();
    assert_eq!(keys.len(), 4);

    // a data property with flags NONE
    let h = rt.new_object();
    ok(rt.define_data(h, PropKey::from_str("v"), n(5.0), Flags::NONE));
    let dh = static_call(&rt, "getOwnPropertyDescriptor", &[Value::Object(h), s("v")]).unwrap();
    assert_eq!(show(&rt, &dh), "{value:5,writable:false,enumerable:false,configurable:false}");

    // an accessor: get and set (undefined when absent), then enumerable and configurable
    let getter = rt.new_native_function(echo, "g", 0, Vec::new());
    let acc = rt.new_object();
    ok(rt.define_accessor(acc, PropKey::from_str("x"), Some(getter), None, Flags::new(false, false, true)));
    let da = static_call(&rt, "getOwnPropertyDescriptor", &[Value::Object(acc), s("x")]).unwrap();
    assert_eq!(getp(&rt, &da, "get"), Value::Object(getter));
    assert_eq!(getp(&rt, &da, "set"), Value::Undefined);
    assert_eq!(getp(&rt, &da, "enumerable"), Value::Bool(false));
    assert_eq!(getp(&rt, &da, "configurable"), Value::Bool(true));
    // an accessor descriptor has no `writable` field
    assert!(rt.get_own_property(da.as_object().unwrap(), &PropKey::from_str("writable")).unwrap().is_none());

    // missing property is undefined
    assert_eq!(static_call(&rt, "getOwnPropertyDescriptor", &[o.clone(), s("zz")]).unwrap(), Value::Undefined);

    // array length: value, writable, not enumerable, not configurable
    let dl = static_call(&rt, "getOwnPropertyDescriptor", &[arr(&rt, vec![n(1.0)]), s("length")]).unwrap();
    assert_eq!(show(&rt, &dl), "{value:1,writable:true,enumerable:false,configurable:false}");

    // string wrapper character: read-only enumerable
    let dc = static_call(&rt, "getOwnPropertyDescriptor", &[s("ab"), s("0")]).unwrap();
    assert_eq!(show(&rt, &dc), r#"{value:"a",writable:false,enumerable:true,configurable:false}"#);

    // the key is coerced with ToPropertyKey; the object with ToObject first
    assert_eq!(thrown_name(&rt, static_call(&rt, "getOwnPropertyDescriptor", &[Value::Null, s("a")])), "TypeError");
    let sym = rt.new_symbol(None);
    assert_eq!(
        static_call(&rt, "getOwnPropertyDescriptor", &[plain(&rt, &[]), Value::Symbol(sym)]).unwrap(),
        Value::Undefined
    );
}

#[test]
fn descriptors_for_every_own_key() {
    let rt = Rt::new();
    let o = rt.new_object();
    ok(rt.define_data(o, PropKey::from_str("a"), n(1.0), Flags::ALL));
    ok(rt.define_data(o, PropKey::from_str("h"), n(2.0), Flags::HIDDEN));
    let sym = rt.new_symbol(None);
    ok(rt.define_data(o, PropKey::Sym(sym), n(3.0), Flags::ALL));
    let all = static_call(&rt, "getOwnPropertyDescriptors", &[Value::Object(o)]).unwrap();
    let ha = getp(&rt, &all, "a");
    assert_eq!(show(&rt, &ha), "{value:1,writable:true,enumerable:true,configurable:true}");
    let hh = getp(&rt, &all, "h");
    assert_eq!(getp(&rt, &hh, "enumerable"), Value::Bool(false));
    let hs = ok(rt.get(all.as_object().unwrap(), &PropKey::Sym(sym), &all));
    assert_eq!(getp(&rt, &hs, "value"), n(3.0));
}

#[test]
fn object_prototype_has_own_property_coerces_key_before_this() {
    let rt = Rt::new();
    let o = plain(&rt, &[("a", n(1.0)), ("2", n(2.0))]);
    assert_eq!(proto_call(&rt, "hasOwnProperty", &o, &[s("a")]).unwrap(), Value::Bool(true));
    assert_eq!(proto_call(&rt, "hasOwnProperty", &o, &[s("b")]).unwrap(), Value::Bool(false));
    assert_eq!(proto_call(&rt, "hasOwnProperty", &o, &[n(2.0)]).unwrap(), Value::Bool(true));
    assert_eq!(proto_call(&rt, "hasOwnProperty", &arr(&rt, vec![n(9.0)]), &[n(0.0)]).unwrap(), Value::Bool(true));
    assert_eq!(proto_call(&rt, "hasOwnProperty", &arr(&rt, vec![]), &[s("length")]).unwrap(), Value::Bool(true));
    // no argument: the key is "undefined"
    assert_eq!(proto_call(&rt, "hasOwnProperty", &o, &[]).unwrap(), Value::Bool(false));
    // this is coerced after the key: null this is a TypeError
    assert_eq!(thrown_name(&rt, proto_call(&rt, "hasOwnProperty", &Value::Null, &[s("a")])), "TypeError");
    assert_eq!(thrown_name(&rt, proto_call(&rt, "hasOwnProperty", &Value::Undefined, &[s("a")])), "TypeError");
    // primitive this is boxed
    assert_eq!(proto_call(&rt, "hasOwnProperty", &s("abc"), &[n(1.0)]).unwrap(), Value::Bool(true));
}

#[test]
fn object_prototype_is_prototype_of() {
    let rt = Rt::new();
    let parent = plain(&rt, &[]);
    let child = Value::Object(rt.new_object_with_proto(parent.as_object()));
    let grand = Value::Object(rt.new_object_with_proto(child.as_object()));
    assert_eq!(proto_call(&rt, "isPrototypeOf", &parent, &[child.clone()]).unwrap(), Value::Bool(true));
    assert_eq!(proto_call(&rt, "isPrototypeOf", &parent, &[grand.clone()]).unwrap(), Value::Bool(true));
    assert_eq!(proto_call(&rt, "isPrototypeOf", &child, &[parent.clone()]).unwrap(), Value::Bool(false));
    assert_eq!(proto_call(&rt, "isPrototypeOf", &parent, &[s("x")]).unwrap(), Value::Bool(false));
    // a primitive argument answers false before `this` is coerced (so undefined this is not a TypeError)
    assert_eq!(proto_call(&rt, "isPrototypeOf", &Value::Undefined, &[n(1.0)]).unwrap(), Value::Bool(false));
    // an object argument with an undefined this is a TypeError
    assert_eq!(thrown_name(&rt, proto_call(&rt, "isPrototypeOf", &Value::Undefined, &[child])), "TypeError");
}

#[test]
fn object_prototype_property_is_enumerable() {
    let rt = Rt::new();
    let o = plain(&rt, &[("a", n(1.0))]);
    ok(rt.define_data(o.as_object().unwrap(), PropKey::from_str("h"), n(2.0), Flags::HIDDEN));
    assert_eq!(proto_call(&rt, "propertyIsEnumerable", &o, &[s("a")]).unwrap(), Value::Bool(true));
    assert_eq!(proto_call(&rt, "propertyIsEnumerable", &o, &[s("h")]).unwrap(), Value::Bool(false));
    assert_eq!(proto_call(&rt, "propertyIsEnumerable", &o, &[s("missing")]).unwrap(), Value::Bool(false));
    // inherited properties are not own
    let child = Value::Object(rt.new_object_with_proto(o.as_object()));
    assert_eq!(proto_call(&rt, "propertyIsEnumerable", &child, &[s("a")]).unwrap(), Value::Bool(false));
    let a = arr(&rt, vec![n(1.0)]);
    assert_eq!(proto_call(&rt, "propertyIsEnumerable", &a, &[n(0.0)]).unwrap(), Value::Bool(true));
    assert_eq!(proto_call(&rt, "propertyIsEnumerable", &a, &[s("length")]).unwrap(), Value::Bool(false));
    assert_eq!(thrown_name(&rt, proto_call(&rt, "propertyIsEnumerable", &Value::Null, &[s("a")])), "TypeError");
}

#[test]
fn object_prototype_value_of_boxes_this() {
    let rt = Rt::new();
    let o = plain(&rt, &[("a", n(1.0))]);
    assert_eq!(proto_call(&rt, "valueOf", &o, &[]).unwrap(), o);
    let boxed = proto_call(&rt, "valueOf", &n(3.0), &[]).unwrap();
    assert_eq!(rt.class_of(boxed.as_object().unwrap()), ObjClass::Wrapper);
    assert_eq!(thrown_name(&rt, proto_call(&rt, "valueOf", &Value::Null, &[])), "TypeError");
    assert_eq!(thrown_name(&rt, proto_call(&rt, "valueOf", &Value::Undefined, &[])), "TypeError");
}

#[test]
fn object_prototype_to_locale_string_invokes_to_string() {
    let rt = Rt::new();
    // an object with its own toString; the result is whatever toString returns
    fn tag(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
        Ok(Value::str("LOCALE"))
    }
    let o = rt.new_object();
    let f = rt.new_native_function(tag, "toString", 0, Vec::new());
    ok(rt.define_data(o, PropKey::from_str("toString"), Value::Object(f), Flags::HIDDEN));
    assert_eq!(proto_call(&rt, "toLocaleString", &Value::Object(o), &[]).unwrap(), s("LOCALE"));
    // undefined and null this: GetV throws TypeError
    assert_eq!(thrown_name(&rt, proto_call(&rt, "toLocaleString", &Value::Undefined, &[])), "TypeError");
    assert_eq!(thrown_name(&rt, proto_call(&rt, "toLocaleString", &Value::Null, &[])), "TypeError");
    // a toString that is not callable is a TypeError
    let bad = rt.new_object();
    ok(rt.define_data(bad, PropKey::from_str("toString"), n(1.0), Flags::ALL));
    assert_eq!(thrown_name(&rt, proto_call(&rt, "toLocaleString", &Value::Object(bad), &[])), "TypeError");
}

#[test]
fn object_to_string_tags_follow_the_builtin_kinds_and_to_string_tag() {
    let rt = Rt::new();
    assert_eq!(show(&rt, &proto_call(&rt, "toString", &Value::Undefined, &[]).unwrap()), "\"[object Undefined]\"");
    assert_eq!(show(&rt, &proto_call(&rt, "toString", &Value::Null, &[]).unwrap()), "\"[object Null]\"");
    assert_eq!(show(&rt, &proto_call(&rt, "toString", &arr(&rt, vec![]), &[]).unwrap()), "\"[object Array]\"");
    assert_eq!(show(&rt, &proto_call(&rt, "toString", &plain(&rt, &[]), &[]).unwrap()), "\"[object Object]\"");
    assert_eq!(show(&rt, &proto_call(&rt, "toString", &n(1.0), &[]).unwrap()), "\"[object Number]\"");
    assert_eq!(show(&rt, &proto_call(&rt, "toString", &s("x"), &[]).unwrap()), "\"[object String]\"");
    assert_eq!(show(&rt, &proto_call(&rt, "toString", &Value::Bool(true), &[]).unwrap()), "\"[object Boolean]\"");
    let f = rt.new_native_function(echo, "f", 0, Vec::new());
    assert_eq!(show(&rt, &proto_call(&rt, "toString", &Value::Object(f), &[]).unwrap()), "\"[object Function]\"");

    // Symbol.toStringTag (string-valued) overrides the builtin tag
    let o = rt.new_object();
    let tag_sym = rt.well_known(WellKnown::ToStringTag);
    ok(rt.define_data(o, PropKey::Sym(tag_sym), s("Custom"), Flags::HIDDEN));
    assert_eq!(show(&rt, &proto_call(&rt, "toString", &Value::Object(o), &[]).unwrap()), "\"[object Custom]\"");
    // a non-string tag is ignored
    let p = rt.new_object();
    ok(rt.define_data(p, PropKey::Sym(tag_sym), n(1.0), Flags::HIDDEN));
    assert_eq!(show(&rt, &proto_call(&rt, "toString", &Value::Object(p), &[]).unwrap()), "\"[object Object]\"");
}

#[test]
fn object_prototype_methods_and_statics_have_spec_attributes() {
    let rt = Rt::new();
    let ctor = gl(&rt, "Object");
    let cobj = ctor.as_object().unwrap();
    let expect: [(&str, u32); 18] = [
        ("keys", 1),
        ("values", 1),
        ("entries", 1),
        ("assign", 2),
        ("freeze", 1),
        ("isFrozen", 1),
        ("seal", 1),
        ("isSealed", 1),
        ("preventExtensions", 1),
        ("isExtensible", 1),
        ("hasOwn", 2),
        ("is", 2),
        ("getPrototypeOf", 1),
        ("create", 2),
        ("getOwnPropertyNames", 1),
        ("getOwnPropertySymbols", 1),
        ("getOwnPropertyDescriptor", 2),
        ("getOwnPropertyDescriptors", 1),
    ];
    for (name, len) in expect {
        let f = getp(&rt, &ctor, name);
        assert_eq!(show(&rt, &getp(&rt, &f, "length")), len.to_string(), "{name}.length");
        assert_eq!(show(&rt, &getp(&rt, &f, "name")), format!("\"{name}\""), "{name}.name");
        let flags = rt.get_own_property(f.as_object().unwrap(), &PropKey::from_str("length")).unwrap();
        assert!(flags.is_some());
        let slot = rt.get_own_property(cobj, &PropKey::from_str(name)).unwrap().unwrap();
        assert_eq!(slot.flags(), Flags::HIDDEN, "{name} is writable, non-enumerable, configurable");
    }
    let op = rt.intrinsic(Intrinsic::ObjectPrototype);
    let to_string = rt.get_own_property(op, &PropKey::from_str("toString")).unwrap().unwrap();
    assert_eq!(to_string.flags(), Flags::HIDDEN);
    for name in ["__proto__", "__defineGetter__", "__lookupGetter__"] {
        assert_eq!(rt.get_own_property(op, &PropKey::from_str(name)).unwrap(), None, "{name} must be absent");
    }
}

// ----- Object.fromEntries and Object.groupBy (iteration through builtins::iterator::iterate) -----

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use vn_value::builtins::iterator::create_iter_result;
use vn_value::{payload_ref, NativeObject, Tracer};

/// The payload of a test iterable: its remaining items and how many times `return` ran.
struct ListIter {
    items: RefCell<VecDeque<Value>>,
    closes: Cell<u32>,
}

impl NativeObject for ListIter {
    fn trace(&self, t: &mut Tracer) {
        for v in self.items.borrow().iter() {
            t.mark_value(v);
        }
    }
}

fn payload_of(rt: &Rt, c: &Call<'_>) -> Rc<dyn NativeObject> {
    let o = c.this.as_object().expect("iterator receiver");
    rt.native_payload(o).expect("iterator payload")
}

fn iter_next(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = payload_of(rt, c);
    let li = payload_ref::<ListIter>(&*p).expect("ListIter");
    let next = li.items.borrow_mut().pop_front();
    Ok(match next {
        Some(v) => create_iter_result(rt, v, false),
        None => create_iter_result(rt, Value::Undefined, true),
    })
}

fn iter_return(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = payload_of(rt, c);
    let li = payload_ref::<ListIter>(&*p).expect("ListIter");
    li.closes.set(li.closes.get() + 1);
    Ok(create_iter_result(rt, Value::Undefined, true))
}

fn iter_self(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(c.this.clone())
}

/// An iterable whose iterator yields `items`. Returns the iterable and the shared record of its state.
fn list_iterable(rt: &Rt, items: Vec<Value>) -> (Value, Rc<ListIter>) {
    let li = Rc::new(ListIter { items: RefCell::new(VecDeque::from(items)), closes: Cell::new(0) });
    let o = rt.new_native_object(Some(rt.intrinsic(Intrinsic::ObjectPrototype)), li.clone());
    ok(rt.define_data(o, PropKey::from_str("next"), Value::Object(rt.new_native_function(iter_next, "next", 0, Vec::new())), Flags::HIDDEN));
    ok(rt.define_data(o, PropKey::from_str("return"), Value::Object(rt.new_native_function(iter_return, "return", 0, Vec::new())), Flags::HIDDEN));
    let it = rt.new_native_function(iter_self, "[Symbol.iterator]", 0, Vec::new());
    ok(rt.define_data(o, PropKey::Sym(rt.well_known(WellKnown::Iterator)), Value::Object(it), Flags::HIDDEN));
    (Value::Object(o), li)
}

fn pair(rt: &Rt, k: Value, v: Value) -> Value {
    arr(rt, vec![k, v])
}

fn from_entries(rt: &Rt, iterable: Value) -> Res<Value> {
    static_call(rt, "fromEntries", &[iterable])
}

fn group_by(rt: &Rt, items: Value, cb: Value) -> Res<Value> {
    static_call(rt, "groupBy", &[items, cb])
}

fn native_fn(rt: &Rt, name: &str, code: vn_value::CodeFn) -> Value {
    Value::Object(rt.new_native_function(code, name, 1, Vec::new()))
}

#[test]
fn from_entries_builds_an_object_from_key_value_pairs() {
    let rt = Rt::new();
    let (it, li) = list_iterable(&rt, vec![pair(&rt, s("a"), n(1.0)), pair(&rt, s("b"), n(2.0))]);
    let o = from_entries(&rt, it).unwrap();
    assert_eq!(show(&rt, &o), "{a:1,b:2}");
    assert_eq!(li.closes.get(), 0, "a normal end does not call return");
    assert_eq!(rt.get_prototype_of(o.as_object().unwrap()), Some(rt.intrinsic(Intrinsic::ObjectPrototype)));
}

#[test]
fn from_entries_later_duplicates_win_and_keys_are_property_keys() {
    let rt = Rt::new();
    let (it, _) = list_iterable(
        &rt,
        vec![pair(&rt, s("a"), n(1.0)), pair(&rt, s("a"), n(2.0)), pair(&rt, n(1.0), s("one")), pair(&rt, s("x"), n(9.0))],
    );
    let o = from_entries(&rt, it).unwrap();
    assert_eq!(show(&rt, &o), r#"{1:"one",a:2,x:9}"#, "the key 1 becomes the string \"1\" and integer keys come first");
}

#[test]
fn from_entries_accepts_symbol_keys() {
    let rt = Rt::new();
    let sym = rt.new_symbol(None);
    let (it, _) = list_iterable(&rt, vec![pair(&rt, Value::Symbol(sym), n(7.0))]);
    let o = from_entries(&rt, it).unwrap();
    assert_eq!(ok(rt.get(o.as_object().unwrap(), &PropKey::Sym(sym), &o)), n(7.0));
}

#[test]
fn from_entries_closes_the_iterator_when_an_item_is_not_an_object() {
    let rt = Rt::new();
    let (it, li) = list_iterable(&rt, vec![pair(&rt, s("a"), n(1.0)), n(5.0), pair(&rt, s("b"), n(2.0))]);
    assert_eq!(thrown_name(&rt, from_entries(&rt, it)), "TypeError");
    assert_eq!(li.closes.get(), 1, "return is called once, with a throw completion");
    assert_eq!(li.items.borrow().len(), 1, "the rest of the items are not read");
}

fn throwing_key_getter(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_type_error("key getter"))
}

#[test]
fn from_entries_closes_the_iterator_when_reading_an_entry_throws() {
    let rt = Rt::new();
    let entry = rt.new_object();
    let g = rt.new_native_function(throwing_key_getter, "get 0", 0, Vec::new());
    ok(rt.define_accessor(entry, PropKey::Index(0), Some(g), None, Flags::new(false, true, true)));
    let (it, li) = list_iterable(&rt, vec![Value::Object(entry)]);
    assert_eq!(thrown_name(&rt, from_entries(&rt, it)), "TypeError");
    assert_eq!(li.closes.get(), 1);
}

#[test]
fn from_entries_refuses_nullish_and_non_iterable_arguments() {
    let rt = Rt::new();
    assert_eq!(thrown_name(&rt, from_entries(&rt, Value::Undefined)), "TypeError");
    assert_eq!(thrown_name(&rt, from_entries(&rt, Value::Null)), "TypeError");
    assert_eq!(thrown_name(&rt, from_entries(&rt, n(1.0))), "TypeError");
    assert_eq!(thrown_name(&rt, from_entries(&rt, plain(&rt, &[]))), "TypeError", "an object without @@iterator");
    assert_eq!(thrown_name(&rt, static_call(&rt, "fromEntries", &[])), "TypeError");
}

#[test]
fn group_by_groups_in_first_seen_order_with_a_null_prototype() {
    let rt = Rt::new();
    let (items, li) = list_iterable(&rt, vec![n(1.0), n(2.0), n(3.0), n(4.0), n(5.0)]);
    let parity = native_fn(&rt, "parity", by_parity);
    let out = group_by(&rt, items, parity).unwrap();
    assert_eq!(show(&rt, &out), r#"{odd:[1,3,5],even:[2,4]}"#);
    assert_eq!(rt.get_prototype_of(out.as_object().unwrap()), None, "groupBy returns a null-prototype object");
    assert_eq!(li.closes.get(), 0);
}

fn by_parity(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    match c.arg(0) {
        Value::Number(x) if x % 2.0 == 0.0 => Ok(Value::str("even")),
        _ => Ok(Value::str("odd")),
    }
}

fn by_index(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(c.arg(1))
}

fn identity(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(c.arg(0))
}

#[test]
fn group_by_passes_the_index_and_coerces_keys_with_to_property_key() {
    let rt = Rt::new();
    let (items, _) = list_iterable(&rt, vec![s("a"), s("b"), s("c")]);
    let out = group_by(&rt, items, native_fn(&rt, "idx", by_index)).unwrap();
    assert_eq!(show(&rt, &out), r#"{0:["a"],1:["b"],2:["c"]}"#);

    // 1 and "1" are the same property key, so they share a group
    let (items2, _) = list_iterable(&rt, vec![n(1.0), s("1"), n(2.0)]);
    let out2 = group_by(&rt, items2, native_fn(&rt, "id", identity)).unwrap();
    assert_eq!(show(&rt, &out2), r#"{1:[1,"1"],2:[2]}"#);
}

#[test]
fn group_by_keys_may_be_symbols() {
    let rt = Rt::new();
    let sym = rt.new_symbol(None);
    // the callback returns the symbol it was built with (native closure data)
    fn return_data(rt: &Rt, c: &Call<'_>) -> Res<Value> {
        Ok(rt.fn_data(c.callee)[0].clone())
    }
    let cb = Value::Object(rt.new_native_function(return_data, "key", 1, vec![Value::Symbol(sym)]));
    let (items, _) = list_iterable(&rt, vec![n(1.0), n(2.0)]);
    let out = group_by(&rt, items, cb).unwrap();
    let group = ok(rt.get(out.as_object().unwrap(), &PropKey::Sym(sym), &out));
    assert_eq!(show(&rt, &group), "[1,2]");
    assert_eq!(rt.get_prototype_of(out.as_object().unwrap()), None);
}

#[test]
fn group_by_checks_the_callback_before_it_reads_items() {
    let rt = Rt::new();
    let (items, li) = list_iterable(&rt, vec![n(1.0), n(2.0)]);
    assert_eq!(thrown_name(&rt, group_by(&rt, items, n(1.0))), "TypeError");
    assert_eq!(li.items.borrow().len(), 2, "no item was read");
    assert_eq!(li.closes.get(), 0, "the iterator was never opened");
    assert_eq!(thrown_name(&rt, group_by(&rt, Value::Undefined, native_fn(&rt, "p", by_parity))), "TypeError");
    assert_eq!(thrown_name(&rt, group_by(&rt, Value::Null, native_fn(&rt, "p", by_parity))), "TypeError");
    assert_eq!(thrown_name(&rt, static_call(&rt, "groupBy", &[])), "TypeError");
}

fn throwing_callback(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_type_error("callback"))
}

#[test]
fn group_by_closes_the_iterator_when_the_callback_throws() {
    let rt = Rt::new();
    let (items, li) = list_iterable(&rt, vec![n(1.0), n(2.0), n(3.0)]);
    assert_eq!(thrown_name(&rt, group_by(&rt, items, native_fn(&rt, "t", throwing_callback))), "TypeError");
    assert_eq!(li.closes.get(), 1);
    assert_eq!(li.items.borrow().len(), 2, "only the first item was consumed");
}

#[test]
fn group_by_closes_the_iterator_when_the_key_conversion_throws() {
    let rt = Rt::new();
    // a key object whose toString throws: ToPropertyKey fails after the callback returned it
    fn throwing_to_string(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
        Err(rt.throw_type_error("toString"))
    }
    let bad = rt.new_object();
    let f = rt.new_native_function(throwing_to_string, "toString", 0, Vec::new());
    ok(rt.define_data(bad, PropKey::from_str("toString"), Value::Object(f), Flags::HIDDEN));
    let (items, li) = list_iterable(&rt, vec![Value::Object(bad)]);
    assert_eq!(thrown_name(&rt, group_by(&rt, items, native_fn(&rt, "id", identity))), "TypeError");
    assert_eq!(li.closes.get(), 1);
}

#[test]
fn group_by_of_nothing_is_an_empty_null_prototype_object() {
    let rt = Rt::new();
    let (items, _) = list_iterable(&rt, vec![]);
    let out = group_by(&rt, items, native_fn(&rt, "p", by_parity)).unwrap();
    assert_eq!(show(&rt, &out), "{}");
    assert_eq!(rt.get_prototype_of(out.as_object().unwrap()), None);
}

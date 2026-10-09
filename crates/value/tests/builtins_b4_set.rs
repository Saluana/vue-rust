//! Set (WP-B4): constructor, basic operations and the ES2025 set methods (Set.prototype.union ... isDisjointFrom). The
//! order of the reads (GetSetRecord: size, has, keys), the points where `keys` and `has` may mutate the sets, the iterator
//! closing rules and the argument errors follow Node 26 (V8 14) probes, quoted next to each test. Plain operands are also
//! in crates/value/tests/../../../../tests/language/builtins/set.json (V8-generated corpus). The set-like objects here have
//! native `has` and `keys` functions whose behaviour is chosen by a mode number held in their closure data.

use vn_value::builtins::iterator::create_iter_result;
use vn_value::{Call, Flags, Intrinsic, Prop, PropKey, Res, Rt, Throw, Value, WellKnown};

// ---------------------------------------------------------------- helpers

fn global(rt: &Rt, name: &str) -> Value {
    rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), name).expect("global binding")
}

fn invoke(rt: &Rt, this: &Value, name: &str, args: &[Value]) -> Res<Value> {
    let f = rt.get_str(this, name)?;
    rt.call(&f, this, args)
}

fn construct(rt: &Rt, name: &str, args: &[Value]) -> Res<Value> {
    let c = global(rt, name);
    rt.construct(&c, args, &c)
}

fn arr(rt: &Rt, items: Vec<Value>) -> Value {
    Value::Object(rt.new_array(items))
}

fn num(n: f64) -> Value {
    Value::Number(n)
}

fn s(text: &str) -> Value {
    Value::str(text)
}

fn native(rt: &Rt, code: fn(&Rt, &Call<'_>) -> Res<Value>, name: &str, data: Vec<Value>) -> Value {
    Value::Object(rt.new_native_function(code, name, 0, data))
}

fn len(rt: &Rt, v: &Value) -> usize {
    match rt.get_str(v, "length").expect("length") {
        Value::Number(n) => n as usize,
        other => panic!("length is {other:?}"),
    }
}

fn push(rt: &Rt, list: &Value, v: Value) {
    let n = len(rt, list);
    rt.set_v_strict(list, &PropKey::Index(n as u32), v).expect("push");
}

fn elements(rt: &Rt, list: &Value) -> Vec<Value> {
    (0..len(rt, list)).map(|i| rt.get_v(list, &PropKey::Index(i as u32)).expect("element")).collect()
}

fn numbers(vals: &[Value]) -> Vec<f64> {
    vals.iter().map(|v| match v {
        Value::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }).collect()
}

fn log_strings(rt: &Rt, log: &Value) -> Vec<String> {
    elements(rt, log).iter().map(|v| rt.to_string(v).expect("string").to_rust_string_lossy()).collect()
}

fn error_name(rt: &Rt, r: Res<Value>) -> String {
    match r {
        Ok(v) => panic!("expected a throw, got {v:?}"),
        Err(Throw(e)) => match rt.get_str(&e, "name").expect("name") {
            Value::String(text) => text.to_rust_string_lossy(),
            other => panic!("error name is {other:?}"),
        },
    }
}

/// The values yielded by an iterator object, in order.
fn drain(rt: &Rt, it: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    loop {
        let r = invoke(rt, it, "next", &[]).expect("next");
        if rt.get_str(&r, "done").expect("done").to_boolean() {
            return out;
        }
        out.push(rt.get_str(&r, "value").expect("value"));
    }
}

/// The values of a Set in insertion order.
fn set_values(rt: &Rt, set: &Value) -> Vec<Value> {
    let it = invoke(rt, set, "values", &[]).expect("values");
    drain(rt, &it)
}

/// An iterable over `values`. Arrays are only iterable once the Array family (WP-B2) is installed, so the tests use this.
fn iterable_values(rt: &Rt, values: Vec<Value>) -> Value {
    let log = arr(rt, vec![]);
    let obj = rt.new_object();
    let method = native(rt, iter_method, "[Symbol.iterator]", vec![arr(rt, values), log]);
    rt.define_data(obj, PropKey::Sym(rt.well_known(WellKnown::Iterator)), method, Flags::ALL).expect("define");
    Value::Object(obj)
}

fn iter_method(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    Ok(keyed_iterator(rt, d[0].clone(), arr(rt, vec![]), Value::Undefined, Value::Undefined, num(0.0)))
}

/// Iterator over `items` (any array-like). `next` logs "next" into `log`. On the first call it mutates `set`: mode 1 adds
/// `x`, mode 2 deletes `x`, mode 0 does nothing. `return` logs "return".
fn keyed_iterator(rt: &Rt, items: Value, log: Value, set: Value, x: Value, mode: Value) -> Value {
    let it = rt.new_object();
    let state = arr(rt, vec![num(0.0)]);
    let next = native(rt, keyed_next, "next", vec![items, state, log.clone(), set, x, mode]);
    let ret = native(rt, keyed_return, "return", vec![log]);
    rt.define_data(it, PropKey::from_str("next"), next, Flags::ALL).expect("next");
    rt.define_data(it, PropKey::from_str("return"), ret, Flags::ALL).expect("return");
    Value::Object(it)
}

fn cursor(rt: &Rt, state: &Value) -> usize {
    match rt.get_v(state, &PropKey::Index(0)).expect("cursor") {
        Value::Number(n) => n as usize,
        _ => 0,
    }
}

fn keyed_next(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    push(rt, &d[2], s("next"));
    let i = cursor(rt, &d[1]);
    if i == 0 {
        match mode(&d[5]) {
            1 => {
                invoke(rt, &d[3], "add", &[d[4].clone()])?;
            }
            2 => {
                invoke(rt, &d[3], "delete", &[d[4].clone()])?;
            }
            _ => {}
        }
    }
    if i < len(rt, &d[0]) {
        let v = rt.get_v(&d[0], &PropKey::Index(i as u32))?;
        rt.set_v_strict(&d[1], &PropKey::Index(0), num((i + 1) as f64))?;
        return Ok(create_iter_result(rt, v, false));
    }
    Ok(create_iter_result(rt, Value::Undefined, true))
}

fn keyed_return(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    push(rt, &d[0], s("return"));
    Ok(Value::Object(rt.new_object()))
}

fn mode(v: &Value) -> u32 {
    match v {
        Value::Number(n) => *n as u32,
        other => panic!("mode is {other:?}"),
    }
}

/// `keys` of a scripted set-like. Data: items, log, set, x, call mode, next mode. Call mode 1 adds `x` to `set` when
/// keys is called. It logs "keys" into the log.
fn scripted_keys(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    push(rt, &d[1], s("keys"));
    if mode(&d[4]) == 1 {
        invoke(rt, &d[2], "add", &[d[3].clone()])?;
    }
    Ok(keyed_iterator(rt, d[0].clone(), d[1].clone(), d[2].clone(), d[3].clone(), d[5].clone()))
}

/// `has` of a scripted set-like. Data: the set, the log, the mode, x. It logs its argument. Modes:
/// 0 return true; 1 return false; 2 delete x when called with 1, return true; 3 add x when called with 1, return true;
/// 4 add x when called with 1, return (argument is 2); 5 delete x when called with 1, return (argument is 1);
/// 6 add x when called with 1, return false.
fn scripted_has(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let v = c.arg(0);
    push(rt, &d[1], v.clone());
    let is = |n: f64| matches!(v, Value::Number(x) if x == n);
    let mutate_add = |rt: &Rt| invoke(rt, &d[0], "add", &[d[3].clone()]).map(|_| ());
    let mutate_delete = |rt: &Rt| invoke(rt, &d[0], "delete", &[d[3].clone()]).map(|_| ());
    let answer = match mode(&d[2]) {
        0 => true,
        1 => false,
        2 => {
            if is(1.0) {
                mutate_delete(rt)?;
            }
            true
        }
        3 => {
            if is(1.0) {
                mutate_add(rt)?;
            }
            true
        }
        4 => {
            if is(1.0) {
                mutate_add(rt)?;
            }
            is(2.0)
        }
        5 => {
            if is(1.0) {
                mutate_delete(rt)?;
            }
            is(1.0)
        }
        6 => {
            if is(1.0) {
                mutate_add(rt)?;
            }
            false
        }
        other => panic!("unknown has mode {other}"),
    };
    Ok(Value::Bool(answer))
}

/// A getter that logs its name into `data[0]` and returns `data[1]`.
fn logging_getter(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    push(rt, &d[0], d[1].clone());
    Ok(d[2].clone())
}

fn throw_range(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_range_error("next failed"))
}

fn returns_five(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(num(5.0))
}

fn has_fn(rt: &Rt, set: &Value, log: &Value, mode: f64, x: Value) -> Value {
    native(rt, scripted_has, "has", vec![set.clone(), log.clone(), num(mode), x])
}

fn keys_fn(rt: &Rt, items: Value, keys_log: &Value, set: &Value, x: Value, call_mode: f64, next_mode: f64) -> Value {
    native(
        rt,
        scripted_keys,
        "keys",
        vec![items, keys_log.clone(), set.clone(), x, num(call_mode), num(next_mode)],
    )
}

/// `has` that never mutates and answers true: GetSetRecord requires a callable `has` even for methods that do not call it.
fn passive_has(rt: &Rt) -> Value {
    has_fn(rt, &Value::Undefined, &arr(rt, vec![]), 0.0, Value::Undefined)
}

/// `keys` that yields nothing and makes no change.
fn no_keys(rt: &Rt) -> Value {
    empty_keys(rt, &Value::Undefined)
}

/// A set-like object with the given `size`, `has` and `keys` as data properties. An undefined `has` or `keys` gets the
/// passive function above, so every object passes GetSetRecord unless the test passes a bad value on purpose.
fn set_like(rt: &Rt, size: Value, has: Value, keys: Value) -> Value {
    let has = if has.is_undefined() { passive_has(rt) } else { has };
    let keys = if keys.is_undefined() { no_keys(rt) } else { keys };
    let o = rt.new_object();
    rt.define_data(o, PropKey::from_str("size"), size, Flags::ALL).expect("size");
    rt.define_data(o, PropKey::from_str("has"), has, Flags::ALL).expect("has");
    rt.define_data(o, PropKey::from_str("keys"), keys, Flags::ALL).expect("keys");
    Value::Object(o)
}

/// A set `{1, 2, 3}` or another list of numbers, built with the constructor.
fn set_of(rt: &Rt, values: &[f64]) -> Value {
    construct(rt, "Set", &[iterable_values(rt, values.iter().map(|n| num(*n)).collect())]).expect("set")
}

/// `keys` of a set-like with no side effects: an empty iterator.
fn empty_keys(rt: &Rt, _set: &Value) -> Value {
    let log = arr(rt, vec![]);
    native(rt, scripted_keys, "keys", vec![arr(rt, vec![]), log, Value::Undefined, Value::Undefined, num(0.0), num(0.0)])
}

// ---------------------------------------------------------------- basic operations

#[test]
fn add_has_delete_and_zero_normalisation() {
    let rt = Rt::new();
    let set = construct(&rt, "Set", &[]).expect("set");
    assert_eq!(invoke(&rt, &set, "add", &[num(1.0)]).expect("add"), set, "add returns the set");
    invoke(&rt, &set, "add", &[num(-0.0)]).expect("add -0");
    invoke(&rt, &set, "add", &[num(f64::NAN)]).expect("add NaN");
    invoke(&rt, &set, "add", &[num(f64::NAN)]).expect("add NaN again");
    assert_eq!(invoke(&rt, &set, "has", &[num(0.0)]).expect("has"), Value::Bool(true), "+0 finds -0");
    assert_eq!(invoke(&rt, &set, "has", &[num(f64::NAN)]).expect("has"), Value::Bool(true));
    let vals = set_values(&rt, &set);
    assert_eq!(vals.len(), 3);
    match vals[1] {
        Value::Number(n) => assert!(n.is_sign_positive() && n == 0.0, "stored as +0"),
        ref other => panic!("second value is {other:?}"),
    }
    assert_eq!(invoke(&rt, &set, "delete", &[num(1.0)]).expect("delete"), Value::Bool(true));
    assert_eq!(invoke(&rt, &set, "delete", &[num(1.0)]).expect("delete"), Value::Bool(false));
    invoke(&rt, &set, "add", &[num(1.0)]).expect("re-add");
    assert_eq!(numbers(&set_values(&rt, &set)).len(), 3, "re-added value moves to the end");
    assert!(matches!(set_values(&rt, &set).last(), Some(Value::Number(n)) if *n == 1.0));
    assert_eq!(invoke(&rt, &set, "clear", &[]).expect("clear"), Value::Undefined);
    assert_eq!(set_values(&rt, &set).len(), 0);
}

#[test]
fn set_constructor_and_iterable_protocol() {
    // Set(): TypeError without new; new Set(5) is a TypeError; holes become undefined members (V8: [1, undefined, 2]).
    let rt = Rt::new();
    let set = global(&rt, "Set");
    assert_eq!(error_name(&rt, rt.call(&set, &Value::Undefined, &[])), "TypeError");
    assert_eq!(error_name(&rt, construct(&rt, "Set", &[num(5.0)])), "TypeError");
    let holes = iterable_values(&rt, vec![num(1.0), Value::Undefined, num(2.0)]);
    let made = construct(&rt, "Set", &[holes]).expect("set");
    assert_eq!(set_values(&rt, &made), vec![num(1.0), Value::Undefined, num(2.0)]);
}

#[test]
fn set_prototype_shapes() {
    let rt = Rt::new();
    let set = global(&rt, "Set");
    let proto = rt.get_str(&set, "prototype").expect("prototype");
    assert_eq!(rt.get_str(&rt.get_str(&proto, "union").expect("union"), "length").expect("length"), num(1.0));
    assert_eq!(rt.get_str(&rt.get_str(&proto, "add").expect("add"), "length").expect("length"), num(1.0));
    assert_eq!(rt.get_str(&proto, "keys").expect("keys"), rt.get_str(&proto, "values").expect("values"), "keys is values");
    let iter = rt.get_v(&proto, &PropKey::Sym(rt.well_known(WellKnown::Iterator))).expect("iterator");
    assert_eq!(iter, rt.get_str(&proto, "values").expect("values"), "Symbol.iterator is values");
    assert_eq!(rt.get_str(&proto, "getOrInsert").expect("absent"), Value::Undefined);
    assert_eq!(rt.get_str(&proto, "constructor").expect("constructor"), set);
    match rt.get_own_property(set.as_object().expect("object"), &PropKey::from_str("prototype")).expect("prototype") {
        Some(Prop::Data { flags, .. }) => assert_eq!(flags, Flags::NONE),
        other => panic!("prototype is {other:?}"),
    }
    assert_eq!(rt.get_v(&proto, &PropKey::Sym(rt.well_known(WellKnown::ToStringTag))).expect("tag"), s("Set"));
    assert_eq!(rt.get_str(&global(&rt, "Set"), "groupBy").expect("groupBy"), Value::Undefined, "Set has no groupBy");
}

// ---------------------------------------------------------------- the methods on plain operands (V8 14)

#[test]
fn set_algebra_on_plain_sets_matches_v8() {
    // V8 14: A = {1,2,3}, B = {2,3,4}: union [1,2,3,4], intersection [2,3], difference [1], symmetricDifference [1,4];
    // isSubsetOf(B), isSupersetOf(B) and isDisjointFrom(B) are all false.
    let rt = Rt::new();
    let a = set_of(&rt, &[1.0, 2.0, 3.0]);
    let b = set_of(&rt, &[2.0, 3.0, 4.0]);
    assert_eq!(numbers(&set_values(&rt, &invoke(&rt, &a, "union", &[b.clone()]).expect("union"))), vec![1.0, 2.0, 3.0, 4.0]);
    assert_eq!(numbers(&set_values(&rt, &invoke(&rt, &a, "intersection", &[b.clone()]).expect("intersection"))), vec![2.0, 3.0]);
    assert_eq!(numbers(&set_values(&rt, &invoke(&rt, &a, "difference", &[b.clone()]).expect("difference"))), vec![1.0]);
    assert_eq!(numbers(&set_values(&rt, &invoke(&rt, &a, "symmetricDifference", &[b.clone()]).expect("symdiff"))), vec![1.0, 4.0]);
    assert_eq!(invoke(&rt, &a, "isSubsetOf", &[b.clone()]).expect("isSubsetOf"), Value::Bool(false));
    assert_eq!(invoke(&rt, &a, "isSupersetOf", &[b.clone()]).expect("isSupersetOf"), Value::Bool(false));
    assert_eq!(invoke(&rt, &a, "isDisjointFrom", &[b.clone()]).expect("isDisjointFrom"), Value::Bool(false));
    let two_three = set_of(&rt, &[2.0, 3.0]);
    assert_eq!(invoke(&rt, &two_three, "isSubsetOf", &[a.clone()]).expect("isSubsetOf"), Value::Bool(true));
    assert_eq!(invoke(&rt, &a, "isSupersetOf", &[two_three.clone()]).expect("isSupersetOf"), Value::Bool(true));
    // Results are new Sets with Set.prototype; the operands are unchanged.
    let union = invoke(&rt, &a, "union", &[b.clone()]).expect("union");
    assert_eq!(rt.get_prototype_of(union.as_object().expect("object")), rt.get_prototype_of(a.as_object().expect("object")));
    assert_eq!(numbers(&set_values(&rt, &a)), vec![1.0, 2.0, 3.0]);
    // A Map is set-like: its keys are the members.
    let map = construct(&rt, "Map", &[iterable_values(&rt, vec![arr(&rt, vec![num(2.0), s("x")])])]).expect("map");
    assert_eq!(numbers(&set_values(&rt, &invoke(&rt, &a, "intersection", &[map]).expect("intersection"))), vec![2.0]);
}

// ---------------------------------------------------------------- GetSetRecord and the argument errors

#[test]
fn getsetrecord_reads_size_has_keys_in_that_order() {
    // V8 14: a set-like whose size, has and keys are getters: the reads happen as size, has, keys.
    let rt = Rt::new();
    let log = arr(&rt, vec![]);
    let keys_log = arr(&rt, vec![]);
    let set = set_of(&rt, &[1.0]);
    let o = rt.new_object();
    let size_get = native(&rt, logging_getter, "get size", vec![log.clone(), s("size"), num(10.0)]);
    let has_get = native(&rt, logging_getter, "get has", vec![log.clone(), s("has"), has_fn(&rt, &set, &keys_log, 0.0, Value::Undefined)]);
    let keys_get = native(&rt, logging_getter, "get keys", vec![log.clone(), s("keys"), keys_fn(&rt, arr(&rt, vec![]), &keys_log, &set, Value::Undefined, 0.0, 0.0)]);
    for (name, getter) in [("size", size_get), ("has", has_get), ("keys", keys_get)] {
        let Value::Object(g) = getter else { unreachable!() };
        rt.define_accessor(o, PropKey::from_str(name), Some(g), None, Flags::ALL).expect("accessor");
    }
    invoke(&rt, &set, "union", &[Value::Object(o)]).expect("union");
    assert_eq!(log_strings(&rt, &log), vec!["size", "has", "keys"]);
}

#[test]
fn size_validation_happens_before_has_and_keys() {
    // V8 14: NaN and undefined sizes are TypeError, a negative size is RangeError (has and keys are not read).
    let rt = Rt::new();
    let set = set_of(&rt, &[1.0]);
    let no_has = Value::Object(rt.new_object());
    let set_with = |size: Value| set_like(&rt, size, Value::Undefined, Value::Undefined);
    assert_eq!(error_name(&rt, invoke(&rt, &set, "union", &[set_with(num(f64::NAN))])), "TypeError");
    assert_eq!(error_name(&rt, invoke(&rt, &set, "union", &[set_with(Value::Undefined)])), "TypeError");
    assert_eq!(error_name(&rt, invoke(&rt, &set, "union", &[set_with(num(-1.0))])), "RangeError");
    assert_eq!(error_name(&rt, invoke(&rt, &set, "union", &[no_has])), "TypeError");
    // Fractional and string sizes convert; a size of -0.5 is 0 after ToIntegerOrInfinity; Infinity is allowed.
    let keys = empty_keys(&rt, &set);
    let has = has_fn(&rt, &set, &arr(&rt, vec![]), 1.0, Value::Undefined);
    for size in [s("2"), num(-0.5), num(f64::INFINITY)] {
        let other = set_like(&rt, size, has.clone(), keys.clone());
        assert!(invoke(&rt, &set, "union", &[other]).is_ok(), "size accepted");
    }
}

#[test]
fn has_and_keys_must_be_callable_and_keys_must_return_an_object() {
    // V8 14: a non-callable has, a non-callable keys, and keys() returning a non-object are TypeErrors.
    let rt = Rt::new();
    let set = set_of(&rt, &[1.0]);
    let keys = empty_keys(&rt, &set);
    let has = has_fn(&rt, &set, &arr(&rt, vec![]), 1.0, Value::Undefined);
    assert_eq!(error_name(&rt, invoke(&rt, &set, "union", &[set_like(&rt, num(1.0), num(5.0), keys.clone())])), "TypeError");
    assert_eq!(error_name(&rt, invoke(&rt, &set, "union", &[set_like(&rt, num(1.0), has.clone(), num(5.0))])), "TypeError");
    let keys_five = native(&rt, returns_five, "keys", vec![]);
    assert_eq!(error_name(&rt, invoke(&rt, &set, "union", &[set_like(&rt, num(1.0), has.clone(), keys_five)])), "TypeError");
}

#[test]
fn other_must_be_an_object_and_this_must_be_a_set() {
    // V8 14: non-object arguments and a Map receiver are TypeErrors.
    let rt = Rt::new();
    let set = set_of(&rt, &[1.0]);
    assert_eq!(error_name(&rt, invoke(&rt, &set, "union", &[num(1.0)])), "TypeError");
    assert_eq!(error_name(&rt, invoke(&rt, &set, "union", &[])), "TypeError");
    let map = construct(&rt, "Map", &[]).expect("map");
    assert_eq!(error_name(&rt, invoke(&rt, &map, "union", &[set.clone()])), "TypeError");
    let union = rt.get_str(&global(&rt, "Set"), "prototype").and_then(|p| rt.get_str(&p, "union")).expect("union");
    assert_eq!(error_name(&rt, rt.call(&union, &map, &[set.clone()])), "TypeError");
}

// ---------------------------------------------------------------- mutation during the has and keys walks

#[test]
fn union_reads_keys_before_copying_this() {
    // V8 14: s = {1}; other.keys() adds 5 to s and returns an empty iterator; union sees the copy {1, 5}.
    let rt = Rt::new();
    let set = set_of(&rt, &[1.0]);
    let keys_log = arr(&rt, vec![]);
    let other = set_like(&rt, num(1.0), has_fn(&rt, &set, &keys_log, 1.0, Value::Undefined), keys_fn(&rt, arr(&rt, vec![]), &keys_log, &set, num(5.0), 1.0, 0.0));
    let union = invoke(&rt, &set, "union", &[other]).expect("union");
    assert_eq!(numbers(&set_values(&rt, &union)), vec![1.0, 5.0]);
}

#[test]
fn intersection_walks_this_live_when_it_is_no_larger_than_other() {
    // V8 14: s = {1,2}, other.size 10. has(1) adds 3 to s, so the walk visits 3 too: result [1,2,3], calls [1,2,3].
    let rt = Rt::new();
    let set = set_of(&rt, &[1.0, 2.0]);
    let calls = arr(&rt, vec![]);
    let other = set_like(&rt, num(10.0), has_fn(&rt, &set, &calls, 3.0, num(3.0)), Value::Undefined);
    let result = invoke(&rt, &set, "intersection", &[other]).expect("intersection");
    assert_eq!(numbers(&set_values(&rt, &result)), vec![1.0, 2.0, 3.0]);
    assert_eq!(numbers(&elements(&rt, &calls)), vec![1.0, 2.0, 3.0]);
    // s = {1,2,3}; has(1) deletes 2: the walk skips it: result [1,3], calls [1,3].
    let set = set_of(&rt, &[1.0, 2.0, 3.0]);
    let calls = arr(&rt, vec![]);
    let other = set_like(&rt, num(10.0), has_fn(&rt, &set, &calls, 2.0, num(2.0)), Value::Undefined);
    let result = invoke(&rt, &set, "intersection", &[other]).expect("intersection");
    assert_eq!(numbers(&set_values(&rt, &result)), vec![1.0, 3.0]);
    assert_eq!(numbers(&elements(&rt, &calls)), vec![1.0, 3.0]);
}

#[test]
fn difference_walks_a_copy_on_the_has_path() {
    // V8 14: s = {1,2,3}; has(1) deletes 3 and returns true. The copy is still walked: calls [1,2,3], result [2,3].
    let rt = Rt::new();
    let set = set_of(&rt, &[1.0, 2.0, 3.0]);
    let calls = arr(&rt, vec![]);
    let other = set_like(&rt, num(10.0), has_fn(&rt, &set, &calls, 5.0, num(3.0)), Value::Undefined);
    let result = invoke(&rt, &set, "difference", &[other]).expect("difference");
    assert_eq!(numbers(&elements(&rt, &calls)), vec![1.0, 2.0, 3.0]);
    assert_eq!(numbers(&set_values(&rt, &result)), vec![2.0, 3.0]);
    // s = {1,2}; has(1) adds 7: the copy is not extended, so only 1 and 2 are called; result [1].
    let set = set_of(&rt, &[1.0, 2.0]);
    let calls = arr(&rt, vec![]);
    let other = set_like(&rt, num(10.0), has_fn(&rt, &set, &calls, 4.0, num(7.0)), Value::Undefined);
    let result = invoke(&rt, &set, "difference", &[other]).expect("difference");
    assert_eq!(numbers(&elements(&rt, &calls)), vec![1.0, 2.0]);
    assert_eq!(numbers(&set_values(&rt, &result)), vec![1.0]);
}

#[test]
fn subset_and_disjoint_walk_the_live_set() {
    // V8 14: isSubsetOf with a delete ahead (calls [1,3], true); with an add (calls [1,4], true);
    // isDisjointFrom with an add (calls [1,2,9], true).
    let rt = Rt::new();
    let set = set_of(&rt, &[1.0, 2.0, 3.0]);
    let calls = arr(&rt, vec![]);
    let other = set_like(&rt, num(10.0), has_fn(&rt, &set, &calls, 2.0, num(2.0)), Value::Undefined);
    assert_eq!(invoke(&rt, &set, "isSubsetOf", &[other]).expect("isSubsetOf"), Value::Bool(true));
    assert_eq!(numbers(&elements(&rt, &calls)), vec![1.0, 3.0]);

    let set = set_of(&rt, &[1.0]);
    let calls = arr(&rt, vec![]);
    let other = set_like(&rt, num(10.0), has_fn(&rt, &set, &calls, 3.0, num(4.0)), Value::Undefined);
    assert_eq!(invoke(&rt, &set, "isSubsetOf", &[other]).expect("isSubsetOf"), Value::Bool(true));
    assert_eq!(numbers(&elements(&rt, &calls)), vec![1.0, 4.0]);

    let set = set_of(&rt, &[1.0, 2.0]);
    let calls = arr(&rt, vec![]);
    let other = set_like(&rt, num(10.0), has_fn(&rt, &set, &calls, 6.0, num(9.0)), Value::Undefined);
    assert_eq!(invoke(&rt, &set, "isDisjointFrom", &[other]).expect("isDisjointFrom"), Value::Bool(true));
    assert_eq!(numbers(&elements(&rt, &calls)), vec![1.0, 2.0, 9.0]);
}

#[test]
fn superset_closes_the_keys_iterator_at_the_first_missing_key() {
    // V8 14: s = {1,2}, other.size 1 -> keys walk: next yields 1 (present), next yields 9 (absent): return, then false.
    let rt = Rt::new();
    let set = set_of(&rt, &[1.0, 2.0]);
    let log = arr(&rt, vec![]);
    let other = set_like(&rt, num(1.0), Value::Undefined, keys_fn(&rt, arr(&rt, vec![num(1.0), num(9.0)]), &log, &set, Value::Undefined, 0.0, 0.0));
    let r = invoke(&rt, &set, "isSupersetOf", &[other]).expect("isSupersetOf");
    assert_eq!(r, Value::Bool(false));
    assert_eq!(log_strings(&rt, &log), vec!["keys", "next", "next", "return"]);
    // With a larger other (size 3 > 2) the answer is false before keys is called.
    let log = arr(&rt, vec![]);
    let other = set_like(&rt, num(3.0), Value::Undefined, keys_fn(&rt, arr(&rt, vec![]), &log, &set, Value::Undefined, 0.0, 0.0));
    assert_eq!(invoke(&rt, &set, "isSupersetOf", &[other]).expect("isSupersetOf"), Value::Bool(false));
    assert_eq!(elements(&rt, &log).len(), 0, "keys is not called");
}

#[test]
fn symmetric_difference_copies_this_after_keys() {
    // V8 14: s = {1}; other.keys() adds 4 to s; the copy includes 4, so the result is [1, 4].
    let rt = Rt::new();
    let set = set_of(&rt, &[1.0]);
    let log = arr(&rt, vec![]);
    let other = set_like(&rt, num(1.0), Value::Undefined, keys_fn(&rt, arr(&rt, vec![]), &log, &set, num(4.0), 1.0, 0.0));
    let result = invoke(&rt, &set, "symmetricDifference", &[other]).expect("symmetricDifference");
    assert_eq!(numbers(&set_values(&rt, &result)), vec![1.0, 4.0]);
}

#[test]
fn difference_copies_this_before_keys_on_the_keys_path() {
    // V8 14: s = {1,2,3}, other.size 1 -> keys walk. The first next adds 8 to s and yields 2: the copy has no 8: [1, 3].
    let rt = Rt::new();
    let set = set_of(&rt, &[1.0, 2.0, 3.0]);
    let log = arr(&rt, vec![]);
    let other = set_like(&rt, num(1.0), Value::Undefined, keys_fn(&rt, arr(&rt, vec![num(2.0)]), &log, &set, num(8.0), 0.0, 1.0));
    let result = invoke(&rt, &set, "difference", &[other]).expect("difference");
    assert_eq!(numbers(&set_values(&rt, &result)), vec![1.0, 3.0]);
}

#[test]
fn intersection_keys_path_sees_deletions_made_by_next() {
    // V8 14: s = {1,2,3}, other.size 1 -> keys walk. The first next deletes 2 and yields 2: 2 is no longer present: [].
    let rt = Rt::new();
    let set = set_of(&rt, &[1.0, 2.0, 3.0]);
    let log = arr(&rt, vec![]);
    let other = set_like(&rt, num(1.0), Value::Undefined, keys_fn(&rt, arr(&rt, vec![num(2.0)]), &log, &set, num(2.0), 0.0, 2.0));
    let result = invoke(&rt, &set, "intersection", &[other]).expect("intersection");
    assert_eq!(set_values(&rt, &result).len(), 0);
}

// ---------------------------------------------------------------- iterator closing and errors from next

#[test]
fn errors_from_next_do_not_close_the_iterator() {
    // V8 14: union and isSupersetOf propagate a throw from next without calling return (seq ["next"]).
    let rt = Rt::new();
    let set = set_of(&rt, &[1.0]);
    let log = arr(&rt, vec![]);
    let throwing = Value::Object({
        let it = rt.new_object();
        let next = native(&rt, throw_range, "next", vec![]);
        rt.define_data(it, PropKey::from_str("next"), next, Flags::ALL).expect("next");
        let ret = native(&rt, keyed_return, "return", vec![log.clone()]);
        rt.define_data(it, PropKey::from_str("return"), ret, Flags::ALL).expect("return");
        it
    });
    let keys = native(&rt, returns_iterator, "keys", vec![throwing]);
    let other = set_like(&rt, num(1.0), Value::Undefined, keys.clone());
    assert_eq!(error_name(&rt, invoke(&rt, &set, "union", &[other])), "RangeError");
    assert_eq!(elements(&rt, &log).len(), 0, "no return call");
    let other = set_like(&rt, num(1.0), Value::Undefined, keys);
    assert_eq!(error_name(&rt, invoke(&rt, &set, "isSupersetOf", &[other])), "RangeError");
    assert_eq!(elements(&rt, &log).len(), 0, "no return call");
}

/// A keys function that returns its closure data (an iterator object).
fn returns_iterator(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(rt.fn_data(c.callee)[0].clone())
}

#[test]
fn next_results_must_be_objects() {
    // V8 14: keys returning an iterator whose next returns 5 is a TypeError.
    let rt = Rt::new();
    let set = set_of(&rt, &[1.0]);
    let it = rt.new_object();
    let next = native(&rt, returns_five, "next", vec![]);
    rt.define_data(it, PropKey::from_str("next"), next, Flags::ALL).expect("next");
    let keys = native(&rt, returns_iterator, "keys", vec![Value::Object(it)]);
    let other = set_like(&rt, num(1.0), Value::Undefined, keys);
    assert_eq!(error_name(&rt, invoke(&rt, &set, "union", &[other])), "TypeError");
}

// ---------------------------------------------------------------- zero and results

#[test]
fn zero_is_normalised_in_results_and_has_arguments() {
    // V8 14: has receives +0 for a stored -0 (new Set([-0])); union with keys yielding -0 stores +0.
    let rt = Rt::new();
    let set = construct(&rt, "Set", &[iterable_values(&rt, vec![num(-0.0)])]).expect("set");
    let calls = arr(&rt, vec![]);
    let other = set_like(&rt, num(10.0), has_fn(&rt, &set, &calls, 0.0, Value::Undefined), Value::Undefined);
    invoke(&rt, &set, "isSubsetOf", &[other]).expect("isSubsetOf");
    match elements(&rt, &calls)[0] {
        Value::Number(n) => assert!(n.is_sign_positive() && n == 0.0, "has receives +0"),
        ref other => panic!("has argument is {other:?}"),
    }
    let empty = set_of(&rt, &[]);
    let keys_log = arr(&rt, vec![]);
    let yields_negzero = set_like(&rt, num(1.0), Value::Undefined, keys_fn(&rt, arr(&rt, vec![num(-0.0)]), &keys_log, &empty, Value::Undefined, 0.0, 0.0));
    let result = invoke(&rt, &empty, "union", &[yields_negzero]).expect("union");
    match set_values(&rt, &result).as_slice() {
        [Value::Number(n)] => assert!(n.is_sign_positive() && *n == 0.0, "union stores +0"),
        other => panic!("union result is {other:?}"),
    }
}

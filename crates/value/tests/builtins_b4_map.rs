//! Map (WP-B4): constructor and iterable protocol, SameValueZero keys, insertion order, forEach under mutation, Map.groupBy,
//! the builtin shapes, the enumeration helpers used by the trace encoder, and collection. Expected values come from Node 26
//! (V8 14) probes; the probe scripts are summarised next to each test. Callbacks are native functions whose closure data
//! holds the collection, a log array and a scenario number, because a `CodeFn` cannot capture Rust state.

use vn_value::builtins::iterator::create_iter_result;
use vn_value::builtins::iterator::iterate;
use vn_value::builtins::map_set::{map_entries, map_put, new_map, new_set, set_put, set_values};
use vn_value::{Call, Flags, Intrinsic, Prop, PropKey, Res, Rt, Throw, Value, WellKnown};

// ---------------------------------------------------------------- helpers

fn global(rt: &Rt, name: &str) -> Value {
    rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), name).expect("global binding")
}

/// `this[name](...args)`.
fn invoke(rt: &Rt, this: &Value, name: &str, args: &[Value]) -> Res<Value> {
    let f = rt.get_str(this, name)?;
    rt.call(&f, this, args)
}

/// `new C(...args)` for a global constructor.
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

fn map_keys(rt: &Rt, m: &Value) -> Vec<Value> {
    let it = invoke(rt, m, "keys", &[]).expect("keys");
    drain(rt, &it)
}

/// Iterator over `items` that logs "next" and "return" into `log`; `next` keeps its cursor in `state[0]`.
fn list_iterator(rt: &Rt, items: Value, log: Value) -> Value {
    let it = rt.new_object();
    let state = arr(rt, vec![num(0.0)]);
    let next = native(rt, list_next, "next", vec![items, state, log.clone()]);
    let ret = native(rt, list_return, "return", vec![log]);
    rt.define_data(it, PropKey::from_str("next"), next, Flags::ALL).expect("define next");
    rt.define_data(it, PropKey::from_str("return"), ret, Flags::ALL).expect("define return");
    Value::Object(it)
}

fn list_next(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    push(rt, &d[2], s("next"));
    let i = match rt.get_v(&d[1], &PropKey::Index(0))? {
        Value::Number(n) => n as usize,
        _ => 0,
    };
    if i < len(rt, &d[0]) {
        let v = rt.get_v(&d[0], &PropKey::Index(i as u32))?;
        rt.set_v_strict(&d[1], &PropKey::Index(0), num((i + 1) as f64))?;
        return Ok(create_iter_result(rt, v, false));
    }
    Ok(create_iter_result(rt, Value::Undefined, true))
}

fn list_return(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    push(rt, &d[0], s("return"));
    Ok(Value::Object(rt.new_object()))
}

fn iter_method(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    push(rt, &d[1], s("iter"));
    Ok(list_iterator(rt, d[0].clone(), d[1].clone()))
}

/// An iterable over `values` whose [Symbol.iterator] logs "iter", and whose iterator logs "next" and "return".
fn iterable(rt: &Rt, values: Vec<Value>, log: &Value) -> Value {
    let obj = rt.new_object();
    let method = native(rt, iter_method, "[Symbol.iterator]", vec![arr(rt, values), log.clone()]);
    rt.define_data(obj, PropKey::Sym(rt.well_known(WellKnown::Iterator)), method, Flags::ALL).expect("define");
    Value::Object(obj)
}

/// A getter that logs its name and returns `data[2]`.
fn log_getter(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    push(rt, &d[0], d[1].clone());
    Ok(d[2].clone())
}

fn scenario(v: &Value) -> u32 {
    match v {
        Value::Number(n) => *n as u32,
        other => panic!("scenario is {other:?}"),
    }
}

/// forEach callback. Data: the map, the log, the scenario. It logs each key, then mutates the map (V8 probe results in
/// the tests below).
fn each_mutate(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let key = c.arg(1);
    push(rt, &d[1], key.clone());
    let k = numbers(&[key])[0];
    let map = &d[0];
    match scenario(&d[2]) {
        0 => {
            if k == 1.0 {
                invoke(rt, map, "clear", &[])?;
                invoke(rt, map, "set", &[num(7.0), num(7.0)])?;
            }
        }
        1 => {
            if k == 1.0 {
                invoke(rt, map, "delete", &[num(2.0)])?;
            }
        }
        2 => {
            if k < 4.0 {
                invoke(rt, map, "set", &[num(k + 1.0), s("n")])?;
            }
        }
        3 => {
            if k == 1.0 && len(rt, &d[1]) == 1 {
                invoke(rt, map, "delete", &[num(1.0)])?;
                invoke(rt, map, "set", &[num(1.0), s("x")])?;
            }
        }
        other => panic!("unknown scenario {other}"),
    }
    Ok(Value::Undefined)
}

/// Set.prototype.forEach callback for the clear-then-add scenario (data: the set, the log).
fn set_each_clear_add(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let k = numbers(&[c.arg(0)])[0];
    push(rt, &d[1], c.arg(0));
    if k == 1.0 {
        invoke(rt, &d[0], "clear", &[])?;
        invoke(rt, &d[0], "add", &[num(5.0)])?;
    }
    Ok(Value::Undefined)
}

/// forEach callback that records (value, key, map, this) into the log.
fn record_args(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let same_map = match (c.arg(2), &d[0]) {
        (Value::Object(a), Value::Object(b)) => a == *b,
        _ => false,
    };
    let row = arr(rt, vec![c.arg(0), c.arg(1), Value::Bool(same_map), c.this.clone()]);
    push(rt, &d[1], row);
    Ok(Value::Undefined)
}

/// Map.groupBy callback: the parity of its first argument, as "even" or "odd".
fn parity(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let n = numbers(&[c.arg(0)])[0];
    Ok(s(if n % 2.0 == 0.0 { "even" } else { "odd" }))
}

/// Map.groupBy callback: 0 maps to -0, 1 and 3 to NaN, and anything else to "k".
fn zero_or_nan_key(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(match c.arg(0) {
        Value::Number(n) if n == 0.0 => num(-0.0),
        Value::Number(n) if n == 1.0 || n == 3.0 => num(f64::NAN),
        _ => s("k"),
    })
}

/// A callback that records its (value, index) arguments and returns the value's key "k".
fn record_index(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let row = arr(rt, vec![c.arg(0), c.arg(1)]);
    push(rt, &d[0], row);
    Ok(s("k"))
}

fn throw_range(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_range_error("callback failed"))
}

// ---------------------------------------------------------------- constructor and basic operations

#[test]
fn constructor_requires_new_and_nullish_iterables_are_empty() {
    let rt = Rt::new();
    let map = global(&rt, "Map");
    assert_eq!(error_name(&rt, rt.call(&map, &Value::Undefined, &[])), "TypeError", "Map() without new");
    for args in [vec![], vec![Value::Undefined], vec![Value::Null]] {
        let m = construct(&rt, "Map", &args).expect("empty map");
        assert_eq!(map_keys(&rt, &m).len(), 0);
    }
    // Non-iterable arguments are TypeErrors (V8: new Map(5), new Map({}), new Map([1])).
    assert_eq!(error_name(&rt, construct(&rt, "Map", &[num(5.0)])), "TypeError");
    assert_eq!(error_name(&rt, construct(&rt, "Map", &[iterable_values(&rt, vec![num(1.0)])])), "TypeError");
}

#[test]
fn same_value_zero_keys() {
    let rt = Rt::new();
    let o = rt.new_object();
    let entries = iterable_values(
        &rt,
        vec![
            arr(&rt, vec![num(f64::NAN), s("nan")]),
            arr(&rt, vec![num(-0.0), s("zero")]),
            arr(&rt, vec![Value::Object(o), s("obj")]),
            arr(&rt, vec![s("1"), s("str")]),
            arr(&rt, vec![num(1.0), s("num")]),
            arr(&rt, vec![Value::Undefined, s("undef")]),
            arr(&rt, vec![Value::Null, s("null")]),
        ],
    );
    let m = construct(&rt, "Map", &[entries]).expect("map");
    assert_eq!(invoke(&rt, &m, "get", &[num(f64::NAN)]).expect("get"), s("nan"));
    assert_eq!(invoke(&rt, &m, "get", &[num(0.0)]).expect("get"), s("zero"), "+0 finds the -0 entry");
    assert_eq!(invoke(&rt, &m, "get", &[Value::Object(o)]).expect("get"), s("obj"));
    assert_eq!(invoke(&rt, &m, "get", &[Value::Object(rt.new_object())]).expect("get"), Value::Undefined);
    assert_eq!(invoke(&rt, &m, "get", &[s("1")]).expect("get"), s("str"));
    assert_eq!(invoke(&rt, &m, "get", &[num(1.0)]).expect("get"), s("num"));
    assert_eq!(invoke(&rt, &m, "get", &[Value::Undefined]).expect("get"), s("undef"));
    assert_eq!(invoke(&rt, &m, "get", &[Value::Null]).expect("get"), s("null"));
    // The stored key of the zero entry is +0, as V8 stores it (Object.is(keys[1], 0)).
    let keys = map_keys(&rt, &m);
    match &keys[1] {
        Value::Number(n) => assert!(n.is_sign_positive() && *n == 0.0, "stored key is +0"),
        other => panic!("zero key is {other:?}"),
    }
    assert_eq!(invoke(&rt, &m, "has", &[num(f64::NAN)]).expect("has"), Value::Bool(true));
}

#[test]
fn insertion_order_and_overwrite_keeps_position() {
    // V8: m = new Map([['a',1],['b',2],['c',3]]); m.set('a', 9); m.delete('b'); m.set('b', 4) gives keys a,c,b.
    let rt = Rt::new();
    let m = construct(&rt, "Map", &[]).expect("map");
    for (k, v) in [("a", 1.0), ("b", 2.0), ("c", 3.0)] {
        invoke(&rt, &m, "set", &[s(k), num(v)]).expect("set");
    }
    assert_eq!(invoke(&rt, &m, "set", &[s("a"), num(9.0)]).expect("set"), m, "set returns the map");
    assert_eq!(invoke(&rt, &m, "delete", &[s("b")]).expect("delete"), Value::Bool(true));
    assert_eq!(invoke(&rt, &m, "delete", &[s("b")]).expect("delete"), Value::Bool(false));
    invoke(&rt, &m, "set", &[s("b"), num(4.0)]).expect("set");
    assert_eq!(map_keys(&rt, &m), vec![s("a"), s("c"), s("b")]);
    assert_eq!(invoke(&rt, &m, "clear", &[]).expect("clear"), Value::Undefined);
    assert_eq!(map_keys(&rt, &m).len(), 0);
}

// ---------------------------------------------------------------- the iterable protocol of the constructor

#[test]
fn constructor_closes_the_iterator_on_a_non_object_entry() {
    // V8: new Map(iterable of 7): next, return, then TypeError.
    let rt = Rt::new();
    let log = arr(&rt, vec![]);
    let it = iterable(&rt, vec![num(7.0)], &log);
    assert_eq!(error_name(&rt, construct(&rt, "Map", &[it])), "TypeError");
    assert_eq!(log_strings(&rt, &log), vec!["iter", "next", "return"]);
}

#[test]
fn constructor_adder_error_closes_the_iterator_and_propagates() {
    // V8: a throwing set gives next, return, then the adder's RangeError.
    let rt = Rt::new();
    let map_proto = rt.get_str(&global(&rt, "Map"), "prototype").expect("prototype");
    rt.set_v_strict(&map_proto, &PropKey::from_str("set"), native(&rt, throw_range, "set", vec![])).expect("replace set");
    let log = arr(&rt, vec![]);
    let pair = arr(&rt, vec![num(1.0), num(2.0)]);
    let it = iterable(&rt, vec![pair], &log);
    assert_eq!(error_name(&rt, construct(&rt, "Map", &[it])), "RangeError");
    assert_eq!(log_strings(&rt, &log), vec!["iter", "next", "return"]);
}

#[test]
fn constructor_reads_set_once_before_iterating() {
    // V8: a getter on Map.prototype.set runs before the iterable is touched: get set, iter, next, return.
    let rt = Rt::new();
    let log = arr(&rt, vec![]);
    let map_proto = rt.get_str(&global(&rt, "Map"), "prototype").expect("prototype");
    let original = rt.get_str(&map_proto, "set").expect("set");
    let getter = native(&rt, log_getter, "get set", vec![log.clone(), s("get set"), original]);
    let getter_ref = match getter {
        Value::Object(o) => o,
        _ => unreachable!(),
    };
    rt.define_accessor(
        match map_proto {
            Value::Object(o) => o,
            _ => unreachable!(),
        },
        PropKey::from_str("set"),
        Some(getter_ref),
        None,
        Flags::new(false, false, true),
    )
    .expect("define accessor");
    let it = iterable(&rt, vec![], &log);
    construct(&rt, "Map", &[it]).expect("empty iterable");
    assert_eq!(log_strings(&rt, &log), vec!["get set", "iter", "next"]);
}

#[test]
fn constructor_reads_entry_key_then_value() {
    // V8: getters on an entry object run in the order 0 then 1.
    let rt = Rt::new();
    let log = arr(&rt, vec![]);
    let entry = rt.new_object();
    let g0 = native(&rt, log_getter, "get 0", vec![log.clone(), s("k"), s("a")]);
    let g1 = native(&rt, log_getter, "get 1", vec![log.clone(), s("v"), s("b")]);
    let (Value::Object(g0), Value::Object(g1)) = (g0, g1) else { unreachable!() };
    rt.define_accessor(entry, PropKey::Index(0), Some(g0), None, Flags::ALL).expect("accessor 0");
    rt.define_accessor(entry, PropKey::Index(1), Some(g1), None, Flags::ALL).expect("accessor 1");
    let m = construct(&rt, "Map", &[iterable_values(&rt, vec![Value::Object(entry)])]).expect("map");
    assert_eq!(log_strings(&rt, &log), vec!["k", "v"]);
    assert_eq!(invoke(&rt, &m, "get", &[s("a")]).expect("get"), s("b"));
}

// ---------------------------------------------------------------- forEach under mutation (V8 14 probe results)

fn for_each_case(initial: &[f64], scenario_no: f64) -> Vec<f64> {
    let rt = Rt::new();
    let entries = iterable_values(&rt, initial.iter().map(|k| arr(&rt, vec![num(*k), num(*k)])).collect());
    let m = construct(&rt, "Map", &[entries]).expect("map");
    let log = arr(&rt, vec![]);
    let cb = native(&rt, each_mutate, "cb", vec![m.clone(), log.clone(), num(scenario_no)]);
    invoke(&rt, &m, "forEach", &[cb]).expect("forEach");
    numbers(&elements(&rt, &log))
}

#[test]
fn foreach_clear_then_add_visits_the_added_entry() {
    // V8 14: m = {1,2}; forEach(k => { push k; if k === 1 { m.clear(); m.set(7,7) } }) visits [1, 7].
    assert_eq!(for_each_case(&[1.0, 2.0], 0.0), vec![1.0, 7.0]);
}

#[test]
fn foreach_skips_an_entry_deleted_before_it_is_reached() {
    // V8 14: m = {1,2,3}; forEach deletes 2 while at 1 and visits [1, 3].
    assert_eq!(for_each_case(&[1.0, 2.0, 3.0], 1.0), vec![1.0, 3.0]);
}

#[test]
fn foreach_visits_entries_added_during_the_walk() {
    // V8 14: m = {1}; each k < 4 adds k+1, and the walk visits [1, 2, 3, 4].
    assert_eq!(for_each_case(&[1.0], 2.0), vec![1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn foreach_delete_then_readd_moves_the_entry_to_the_end() {
    // V8 14: m = {1,2,3}; at the first visit of 1, delete 1 and set 1 again; the walk visits [1, 2, 3, 1].
    assert_eq!(for_each_case(&[1.0, 2.0, 3.0], 3.0), vec![1.0, 2.0, 3.0, 1.0]);
}

#[test]
fn set_foreach_clear_then_add() {
    // V8 14: s = {1,2}; forEach(v => { if v === 1 { s.clear(); s.add(5) } }) visits [1, 5].
    let rt = Rt::new();
    let set = construct(&rt, "Set", &[iterable_values(&rt, vec![num(1.0), num(2.0)])]).expect("set");
    let log = arr(&rt, vec![]);
    let cb = native(&rt, set_each_clear_add, "cb", vec![set.clone(), log.clone()]);
    invoke(&rt, &set, "forEach", &[cb]).expect("forEach");
    assert_eq!(numbers(&elements(&rt, &log)), vec![1.0, 5.0]);
}

#[test]
fn foreach_passes_value_key_collection_and_this_arg() {
    // V8: forEach((v, k, m) => ...) with a thisArg; a Set passes (value, value, set).
    let rt = Rt::new();
    let m = construct(&rt, "Map", &[iterable_values(&rt, vec![arr(&rt, vec![s("a"), num(1.0)])])]).expect("map");
    let log = arr(&rt, vec![]);
    let cb = native(&rt, record_args, "cb", vec![m.clone(), log.clone()]);
    let this_arg = rt.new_object();
    invoke(&rt, &m, "forEach", &[cb.clone(), Value::Object(this_arg)]).expect("forEach");
    let rows = elements(&rt, &log);
    let row = elements(&rt, &rows[0]);
    assert_eq!(row[0], num(1.0));
    assert_eq!(row[1], s("a"));
    assert_eq!(row[2], Value::Bool(true));
    assert_eq!(row[3], Value::Object(this_arg));
    assert_eq!(error_name(&rt, invoke(&rt, &m, "forEach", &[num(5.0)])), "TypeError");
}

// ---------------------------------------------------------------- Map.groupBy

#[test]
fn groupby_groups_in_first_seen_order() {
    // V8: Map.groupBy([1,2,3,4], parity) gives keys odd, even with values [1,3] and [2,4].
    let rt = Rt::new();
    let map = global(&rt, "Map");
    let group_by = rt.get_str(&map, "groupBy").expect("groupBy");
    let cb = native(&rt, parity, "parity", vec![]);
    let items = iterable_values(&rt, vec![num(1.0), num(2.0), num(3.0), num(4.0)]);
    let grouped = rt.call(&group_by, &map, &[items, cb]).expect("groupBy");
    assert_eq!(map_keys(&rt, &grouped), vec![s("odd"), s("even")]);
    let odd = invoke(&rt, &grouped, "get", &[s("odd")]).expect("get");
    assert_eq!(numbers(&elements(&rt, &odd)), vec![1.0, 3.0]);
    let even = invoke(&rt, &grouped, "get", &[s("even")]).expect("get");
    assert_eq!(numbers(&elements(&rt, &even)), vec![2.0, 4.0]);
}

#[test]
fn groupby_normalises_negative_zero_and_groups_nan_together() {
    // V8: Map.groupBy([0,1,2,3], k) with -0 for 0, NaN for 1 and 3, "k" for 2: keys are +0, NaN, "k" in first-seen order.
    let rt = Rt::new();
    let map = global(&rt, "Map");
    let group_by = rt.get_str(&map, "groupBy").expect("groupBy");
    let cb = native(&rt, zero_or_nan_key, "key", vec![]);
    let items = iterable_values(&rt, vec![num(0.0), num(1.0), num(2.0), num(3.0)]);
    let grouped = rt.call(&group_by, &map, &[items, cb]).expect("groupBy");
    let keys = map_keys(&rt, &grouped);
    assert_eq!(keys.len(), 3);
    match &keys[0] {
        Value::Number(n) => assert!(n.is_sign_positive() && *n == 0.0, "zero key is +0"),
        other => panic!("first key is {other:?}"),
    }
    assert!(matches!(keys[1], Value::Number(n) if n.is_nan()));
    assert_eq!(keys[2], s("k"));
    let nan_group = invoke(&rt, &grouped, "get", &[num(f64::NAN)]).expect("get");
    assert_eq!(numbers(&elements(&rt, &nan_group)), vec![1.0, 3.0]);
}

#[test]
fn groupby_passes_value_and_index_and_closes_the_iterator_on_a_throw() {
    // V8: callbacks get (value, index); a callback that throws makes the iterator run return, then the error propagates.
    let rt = Rt::new();
    let map = global(&rt, "Map");
    let group_by = rt.get_str(&map, "groupBy").expect("groupBy");
    let rows = arr(&rt, vec![]);
    let cb = native(&rt, record_index, "cb", vec![rows.clone()]);
    let items = iterable_values(&rt, vec![s("a"), s("b")]);
    rt.call(&group_by, &map, &[items, cb]).expect("groupBy");
    assert_eq!(elements(&rt, &rows).len(), 2);
    assert_eq!(elements(&rt, &elements(&rt, &rows)[1]), vec![s("b"), num(1.0)]);

    let log = arr(&rt, vec![]);
    let source = iterable(&rt, vec![num(1.0)], &log);
    let thrower = native(&rt, throw_range, "cb", vec![]);
    let r = rt.call(&group_by, &map, &[source, thrower]);
    assert_eq!(error_name(&rt, r), "RangeError");
    assert_eq!(log_strings(&rt, &log), vec!["iter", "next", "return"]);
}

#[test]
fn groupby_argument_errors_are_type_errors() {
    // V8: null or undefined items, and a missing or non-callable callback, all throw TypeError.
    let rt = Rt::new();
    let map = global(&rt, "Map");
    let group_by = rt.get_str(&map, "groupBy").expect("groupBy");
    let cb = native(&rt, parity, "parity", vec![]);
    assert_eq!(error_name(&rt, rt.call(&group_by, &map, &[Value::Null, cb.clone()])), "TypeError");
    assert_eq!(error_name(&rt, rt.call(&group_by, &map, &[Value::Undefined, cb.clone()])), "TypeError");
    assert_eq!(error_name(&rt, rt.call(&group_by, &map, &[arr(&rt, vec![num(1.0)]), num(5.0)])), "TypeError");
    assert_eq!(error_name(&rt, rt.call(&group_by, &map, &[arr(&rt, vec![num(1.0)])])), "TypeError");
    assert_eq!(error_name(&rt, rt.call(&group_by, &map, &[num(5.0), num(5.0)])), "TypeError");
}

// ---------------------------------------------------------------- shapes and absent members

#[test]
fn builtin_shapes_and_absent_members() {
    let rt = Rt::new();
    let map = global(&rt, "Map");
    let proto = rt.get_str(&map, "prototype").expect("prototype");
    let (Value::Object(map_ref), Value::Object(proto_ref)) = (map.clone(), proto.clone()) else { unreachable!() };
    assert_eq!(rt.get_str(&map, "length").expect("length"), num(0.0));
    assert_eq!(rt.get_str(&rt.get_str(&proto, "set").expect("set"), "length").expect("length"), num(2.0));
    assert_eq!(rt.get_str(&rt.get_str(&proto, "forEach").expect("forEach"), "length").expect("length"), num(1.0));
    assert_eq!(rt.get_str(&rt.get_str(&map, "groupBy").expect("groupBy"), "length").expect("length"), num(2.0));
    assert_eq!(rt.get_str(&rt.get_str(&proto, "get").expect("get"), "name").expect("name"), s("get"));
    // size is an accessor named "get size" with no setter, on the prototype.
    match rt.get_own_property(proto_ref, &PropKey::from_str("size")).expect("size") {
        Some(Prop::Accessor { get: Some(g), set: None, flags }) => {
            assert_eq!(rt.get_str(&Value::Object(g), "name").expect("name"), s("get size"));
            assert!(!flags.enumerable && flags.configurable);
        }
        other => panic!("size is {other:?}"),
    }
    // Symbol.toStringTag: "Map", not writable, not enumerable, configurable.
    match rt.get_own_property(proto_ref, &PropKey::Sym(rt.well_known(WellKnown::ToStringTag))).expect("tag") {
        Some(Prop::Data { value, flags }) => {
            assert_eq!(value, s("Map"));
            assert_eq!(flags, Flags::CONFIG);
        }
        other => panic!("toStringTag is {other:?}"),
    }
    // Map.prototype[Symbol.iterator] is the same function object as entries.
    let iter_fn = rt.get_v(&proto, &PropKey::Sym(rt.well_known(WellKnown::Iterator))).expect("iterator");
    assert_eq!(iter_fn, rt.get_str(&proto, "entries").expect("entries"));
    // prototype is non-writable and non-enumerable; prototype.constructor is Map.
    match rt.get_own_property(map_ref, &PropKey::from_str("prototype")).expect("prototype") {
        Some(Prop::Data { flags, .. }) => assert_eq!(flags, Flags::NONE),
        other => panic!("prototype is {other:?}"),
    }
    assert_eq!(rt.get_str(&proto, "constructor").expect("constructor"), map);
    // Not in the WP's list, and not supported by the manifest: absent.
    assert_eq!(rt.get_str(&proto, "getOrInsert").expect("getOrInsert"), Value::Undefined);
    assert_eq!(rt.get_str(&proto, "getOrInsertComputed").expect("getOrInsertComputed"), Value::Undefined);
    assert_eq!(global(&rt, "WeakMap"), Value::Undefined);
    assert_eq!(global(&rt, "WeakSet"), Value::Undefined);
    assert_eq!(global(&rt, "WeakRef"), Value::Undefined);
    assert_eq!(global(&rt, "FinalizationRegistry"), Value::Undefined);
    // Iterator tags.
    let it = invoke(&rt, &proto, "keys", &[]);
    assert!(it.is_err(), "keys needs a receiver");
    let m = construct(&rt, "Map", &[]).expect("map");
    let keys_it = invoke(&rt, &m, "keys", &[]).expect("keys");
    let iter_proto = rt.get_prototype_of(keys_it.as_object().expect("object")).expect("prototype");
    assert_eq!(rt.get_v(&Value::Object(iter_proto), &PropKey::Sym(rt.well_known(WellKnown::ToStringTag))).expect("tag"), s("Map Iterator"));
    // The iterator prototype is %IteratorPrototype%: its [Symbol.iterator] returns the iterator itself.
    let f = rt.get_v(&keys_it, &PropKey::Sym(rt.well_known(WellKnown::Iterator))).expect("iterator method");
    assert_eq!(rt.call(&f, &keys_it, &[]).expect("self"), keys_it);
}

// ---------------------------------------------------------------- receivers

#[test]
fn size_and_methods_reject_other_receivers() {
    let rt = Rt::new();
    let set = construct(&rt, "Set", &[]).expect("set");
    let proto = rt.get_str(&global(&rt, "Map"), "prototype").expect("prototype");
    let size_getter = match rt.get_own_property(match proto {
        Value::Object(o) => o,
        _ => unreachable!(),
    }, &PropKey::from_str("size")).expect("size") {
        Some(Prop::Accessor { get: Some(g), .. }) => Value::Object(g),
        other => panic!("size is {other:?}"),
    };
    assert_eq!(error_name(&rt, rt.call(&size_getter, &set, &[])), "TypeError");
    assert_eq!(error_name(&rt, invoke(&rt, &set, "get", &[num(1.0)])), "TypeError");
    let plain = Value::Object(rt.new_object());
    assert_eq!(error_name(&rt, invoke(&rt, &plain, "has", &[num(1.0)])), "TypeError");
}

// ---------------------------------------------------------------- enumeration helpers and collection

#[test]
fn enumeration_helpers_follow_insertion_order() {
    let rt = Rt::new();
    let m = new_map(&rt);
    assert!(map_put(&rt, m, num(-0.0), s("zero")));
    assert!(map_put(&rt, m, s("b"), num(2.0)));
    assert!(!map_put(&rt, new_set(&rt), num(1.0), num(1.0)), "map_put refuses a Set");
    let entries = map_entries(&rt, m).expect("map entries");
    assert_eq!(entries.len(), 2);
    match &entries[0].0 {
        Value::Number(n) => assert!(n.is_sign_positive(), "-0 is stored as +0"),
        other => panic!("key is {other:?}"),
    }
    assert_eq!(entries[1], (s("b"), num(2.0)));
    assert!(map_entries(&rt, new_set(&rt)).is_none());
    let set = new_set(&rt);
    assert!(set_put(&rt, set, s("x")));
    assert!(set_put(&rt, set, s("x")));
    assert_eq!(set_values(&rt, set).expect("set values"), vec![s("x")]);
    assert!(set_values(&rt, m).is_none());
    // The helpers agree with the methods.
    assert_eq!(invoke(&rt, &Value::Object(m), "get", &[s("b")]).expect("get"), num(2.0));
}

#[test]
fn collection_keeps_its_object_keys_alive() {
    let rt = Rt::new();
    let m = new_map(&rt);
    let key = rt.new_object();
    map_put(&rt, m, Value::Object(key), num(1.0));
    let root = rt.root(&Value::Object(m));
    rt.collect();
    assert!(rt.is_live(key), "the map traces its keys");
    assert_eq!(invoke(&rt, &Value::Object(m), "get", &[Value::Object(key)]).expect("get"), num(1.0));
    rt.unroot(root);
    let garbage = new_map(&rt);
    let garbage_key = rt.new_object();
    map_put(&rt, garbage, Value::Object(garbage_key), num(2.0));
    rt.collect();
    assert!(!rt.is_live(garbage), "an unreachable map is freed");
    assert!(!rt.is_live(garbage_key));
}

#[test]
fn iterate_driver_walks_a_map_as_entries() {
    // B0's iterate() reads Map.prototype[Symbol.iterator] (entries) through the iteration protocol.
    let rt = Rt::new();
    let m = construct(&rt, "Map", &[iterable_values(&rt, vec![arr(&rt, vec![num(1.0), s("a")]), arr(&rt, vec![num(2.0), s("b")])])]).expect("map");
    let mut seen = Vec::new();
    iterate(&rt, &m, &mut |pair| {
        seen.push(elements(&rt, &pair));
        Ok(true)
    })
    .expect("iterate");
    assert_eq!(seen, vec![vec![num(1.0), s("a")], vec![num(2.0), s("b")]]);
}

/// An iterable over `values`. Arrays are only iterable once the Array family (WP-B2) is installed, so the tests use this.
fn iterable_values(rt: &Rt, values: Vec<Value>) -> Value {
    let log = arr(rt, vec![]);
    iterable(rt, values, &log)
}

//! Map and Set iterators (WP-B4): the walk under mutation, compaction with live cursors, the iterator prototypes, receiver
//! checks, GC of a walk in progress, and B0's iterate() driver over the collections. Expected values come from Node 26
//! (V8 14) probes; each test names the probe it repeats.

use vn_value::builtins::iterator::iterate;
use vn_value::{Call, Flags, Intrinsic, PropKey, Res, Rt, Throw, Value, WellKnown};

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

fn num(n: f64) -> Value {
    Value::Number(n)
}

fn s(text: &str) -> Value {
    Value::str(text)
}

fn arr(rt: &Rt, items: Vec<Value>) -> Value {
    Value::Object(rt.new_array(items))
}

fn numbers(vals: &[Value]) -> Vec<f64> {
    vals.iter().map(|v| match v {
        Value::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }).collect()
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

/// One `next()` call: the value, or None once done.
fn step(rt: &Rt, it: &Value) -> Option<Value> {
    let r = invoke(rt, it, "next", &[]).expect("next");
    if rt.get_str(&r, "done").expect("done").to_boolean() {
        None
    } else {
        Some(rt.get_str(&r, "value").expect("value"))
    }
}

/// The rest of an iterator, in order.
fn drain(rt: &Rt, it: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    while let Some(v) = step(rt, it) {
        out.push(v);
    }
    out
}

fn map_of(rt: &Rt, keys: &[f64]) -> Value {
    let entries: Vec<Value> = keys.iter().map(|k| arr(rt, vec![num(*k), num(*k)])).collect();
    let it = iterable(rt, entries);
    construct(rt, "Map", &[it]).expect("map")
}

fn set_of(rt: &Rt, values: &[f64]) -> Value {
    let it = iterable(rt, values.iter().map(|v| num(*v)).collect());
    construct(rt, "Set", &[it]).expect("set")
}

/// An iterable over `values` (arrays are only iterable once WP-B2 is installed). Its iterator is a native-function
/// object; `values` is captured in the closure data of `[Symbol.iterator]`.
fn iterable(rt: &Rt, values: Vec<Value>) -> Value {
    let obj = rt.new_object();
    let source = arr(rt, values);
    let method = Value::Object(rt.new_native_function(list_method, "[Symbol.iterator]", 0, vec![source]));
    rt.define_data(obj, PropKey::Sym(rt.well_known(WellKnown::Iterator)), method, Flags::ALL).expect("define");
    Value::Object(obj)
}

fn list_method(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let it = rt.new_object();
    let state = arr(rt, vec![num(0.0)]);
    let next = Value::Object(rt.new_native_function(list_next, "next", 0, vec![d[0].clone(), state]));
    rt.define_data(it, PropKey::from_str("next"), next, Flags::ALL)?;
    Ok(Value::Object(it))
}

fn list_next(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let i = match rt.get_v(&d[1], &PropKey::Index(0))? {
        Value::Number(n) => n as usize,
        _ => 0,
    };
    let n = match rt.get_str(&d[0], "length")? {
        Value::Number(n) => n as usize,
        _ => 0,
    };
    if i < n {
        let v = rt.get_v(&d[0], &PropKey::Index(i as u32))?;
        rt.set_v_strict(&d[1], &PropKey::Index(0), num((i + 1) as f64))?;
        return Ok(rt_result(rt, v, false));
    }
    Ok(rt_result(rt, Value::Undefined, true))
}

fn rt_result(rt: &Rt, value: Value, done: bool) -> Value {
    vn_value::builtins::iterator::create_iter_result(rt, value, done)
}

// ---------------------------------------------------------------- the walk under mutation (V8 14)

#[test]
fn map_walk_sees_entries_added_after_clear() {
    // V8 14: m = {1,2}; it.next() gives 1; m.clear(); m.set(9, 9): the walk then yields 9.
    let rt = Rt::new();
    let m = map_of(&rt, &[1.0, 2.0]);
    let it = invoke(&rt, &m, "keys", &[]).expect("keys");
    let mut seen = vec![step(&rt, &it).expect("first")];
    invoke(&rt, &m, "clear", &[]).expect("clear");
    invoke(&rt, &m, "set", &[num(9.0), num(9.0)]).expect("set");
    seen.extend(drain(&rt, &it));
    assert_eq!(numbers(&seen), vec![1.0, 9.0]);
}

#[test]
fn map_walk_ends_after_clear_when_nothing_is_added() {
    // V8 14: m = {1,2}; it.next() gives 1; m.clear(): the walk is done.
    let rt = Rt::new();
    let m = map_of(&rt, &[1.0, 2.0]);
    let it = invoke(&rt, &m, "keys", &[]).expect("keys");
    assert_eq!(step(&rt, &it), Some(num(1.0)));
    invoke(&rt, &m, "clear", &[]).expect("clear");
    assert_eq!(step(&rt, &it), None);
}

#[test]
fn map_walk_skips_an_entry_deleted_before_it_is_reached() {
    // V8 14: m = {1,2,3}; the walk yields 1, then 2 is deleted: it yields 3.
    let rt = Rt::new();
    let m = map_of(&rt, &[1.0, 2.0, 3.0]);
    let it = invoke(&rt, &m, "keys", &[]).expect("keys");
    assert_eq!(step(&rt, &it), Some(num(1.0)));
    invoke(&rt, &m, "delete", &[num(2.0)]).expect("delete");
    assert_eq!(drain(&rt, &it), vec![num(3.0)]);
}

#[test]
fn map_walk_visits_entries_added_during_the_walk() {
    // V8 14: m = {1}; the walk yields 1 and adds 2, 3, 4 one by one before each step: [1, 2, 3, 4].
    let rt = Rt::new();
    let m = map_of(&rt, &[1.0]);
    let it = invoke(&rt, &m, "keys", &[]).expect("keys");
    let mut seen = vec![step(&rt, &it).expect("first")];
    for k in [2.0, 3.0, 4.0] {
        invoke(&rt, &m, "set", &[num(k), s("n")]).expect("set");
        seen.push(step(&rt, &it).expect("next"));
    }
    assert_eq!(step(&rt, &it), None);
    assert_eq!(numbers(&seen), vec![1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn map_delete_then_readd_is_visited_again_at_the_end() {
    // V8 14: m = {1,2,3}; after the first 1, delete 1 and set 1 again: the walk visits 2, 3, 1.
    let rt = Rt::new();
    let m = map_of(&rt, &[1.0, 2.0, 3.0]);
    let it = invoke(&rt, &m, "keys", &[]).expect("keys");
    assert_eq!(step(&rt, &it), Some(num(1.0)));
    invoke(&rt, &m, "delete", &[num(1.0)]).expect("delete");
    invoke(&rt, &m, "set", &[num(1.0), s("x")]).expect("set");
    assert_eq!(numbers(&drain(&rt, &it)), vec![2.0, 3.0, 1.0]);
}

#[test]
fn exhausted_walk_stays_done_after_later_additions() {
    // V8 14: m = {1}; the walk is done after 1; m.set(5, 5): next is still done.
    let rt = Rt::new();
    let m = map_of(&rt, &[1.0]);
    let it = invoke(&rt, &m, "keys", &[]).expect("keys");
    assert_eq!(drain(&rt, &it), vec![num(1.0)]);
    invoke(&rt, &m, "set", &[num(5.0), num(5.0)]).expect("set");
    assert_eq!(step(&rt, &it), None);
    assert_eq!(step(&rt, &it), None);
}

#[test]
fn walk_cursor_survives_compaction() {
    // V8 14: m = {0..39}; take 0..4, delete 5..29 (more than half of the entries, so the table is compacted), then the
    // walk yields 30..39.
    let rt = Rt::new();
    let keys: Vec<f64> = (0..40).map(|i| i as f64).collect();
    let m = map_of(&rt, &keys);
    let it = invoke(&rt, &m, "keys", &[]).expect("keys");
    let mut seen: Vec<Value> = (0..5).map(|_| step(&rt, &it).expect("visited")).collect();
    for k in 5..30 {
        invoke(&rt, &m, "delete", &[num(k as f64)]).expect("delete");
    }
    seen.extend(drain(&rt, &it));
    let expected: Vec<f64> = (0..5).chain(30..40).map(|i| i as f64).collect();
    assert_eq!(numbers(&seen), expected);
}

#[test]
fn walk_started_before_deletions_continues_after_the_cursor() {
    // V8 14: m = {0..39}; take 0..19, then delete 0..19; the walk yields 20..39.
    let rt = Rt::new();
    let keys: Vec<f64> = (0..40).map(|i| i as f64).collect();
    let m = map_of(&rt, &keys);
    let it = invoke(&rt, &m, "keys", &[]).expect("keys");
    let mut seen: Vec<Value> = (0..20).map(|_| step(&rt, &it).expect("visited")).collect();
    for k in 0..20 {
        invoke(&rt, &m, "delete", &[num(k as f64)]).expect("delete");
    }
    seen.extend(drain(&rt, &it));
    assert_eq!(numbers(&seen), (0..40).map(|i| i as f64).collect::<Vec<_>>());
}

#[test]
fn set_walk_mutations_match_the_map_walk() {
    // V8 14: s = {1,2,3}; delete 2 while at 1: [1,3]. s = {1}; add 2..4 during the walk: [1,2,3,4].
    // s = {1,2}; clear and add 8 after the first value: the walk yields 8.
    let rt = Rt::new();
    let s1 = set_of(&rt, &[1.0, 2.0, 3.0]);
    let it = invoke(&rt, &s1, "values", &[]).expect("values");
    assert_eq!(step(&rt, &it), Some(num(1.0)));
    invoke(&rt, &s1, "delete", &[num(2.0)]).expect("delete");
    assert_eq!(drain(&rt, &it), vec![num(3.0)]);

    let s2 = set_of(&rt, &[1.0]);
    let it = invoke(&rt, &s2, "values", &[]).expect("values");
    let mut seen = vec![step(&rt, &it).expect("first")];
    for v in [2.0, 3.0, 4.0] {
        invoke(&rt, &s2, "add", &[num(v)]).expect("add");
        seen.push(step(&rt, &it).expect("next"));
    }
    assert_eq!(numbers(&seen), vec![1.0, 2.0, 3.0, 4.0]);

    let s3 = set_of(&rt, &[1.0, 2.0]);
    let it = invoke(&rt, &s3, "values", &[]).expect("values");
    assert_eq!(step(&rt, &it), Some(num(1.0)));
    invoke(&rt, &s3, "clear", &[]).expect("clear");
    invoke(&rt, &s3, "add", &[num(8.0)]).expect("add");
    assert_eq!(drain(&rt, &it), vec![num(8.0)]);
}

// ---------------------------------------------------------------- entries, prototypes and receivers

#[test]
fn entries_yield_pairs_and_sets_repeat_the_value() {
    // V8 14: [...new Map([['x',1]]).entries()] is [['x',1]]; [...new Set(['x','y']).entries()] is [['x','x'],['y','y']].
    let rt = Rt::new();
    let m = construct(&rt, "Map", &[iterable(&rt, vec![arr(&rt, vec![s("x"), num(1.0)])])]).expect("map");
    let it = invoke(&rt, &m, "entries", &[]).expect("entries");
    let pair = step(&rt, &it).expect("pair");
    assert_eq!(rt.get_v(&pair, &PropKey::Index(0)).expect("key"), s("x"));
    assert_eq!(rt.get_v(&pair, &PropKey::Index(1)).expect("value"), num(1.0));
    let set = set_of(&rt, &[]);
    invoke(&rt, &set, "add", &[s("x")]).expect("add");
    invoke(&rt, &set, "add", &[s("y")]).expect("add");
    let it = invoke(&rt, &set, "entries", &[]).expect("entries");
    let first = step(&rt, &it).expect("pair");
    assert_eq!(rt.get_v(&first, &PropKey::Index(0)).expect("key"), s("x"));
    assert_eq!(rt.get_v(&first, &PropKey::Index(1)).expect("value"), s("x"));
}

#[test]
fn iterator_prototypes_have_tags_and_inherit_iterator_prototype() {
    // V8 14: Object.prototype.toString gives "[object Map Iterator]" and "[object Set Iterator]"; both inherit from
    // %IteratorPrototype%, whose [Symbol.iterator] returns the iterator itself.
    let rt = Rt::new();
    let m = construct(&rt, "Map", &[]).expect("map");
    let set = construct(&rt, "Set", &[]).expect("set");
    let map_it = invoke(&rt, &m, "keys", &[]).expect("keys");
    let set_it = invoke(&rt, &set, "values", &[]).expect("values");
    let map_proto = rt.get_prototype_of(map_it.as_object().expect("object")).expect("prototype");
    let set_proto = rt.get_prototype_of(set_it.as_object().expect("object")).expect("prototype");
    assert_eq!(rt.get_v(&Value::Object(map_proto), &PropKey::Sym(rt.well_known(WellKnown::ToStringTag))).expect("tag"), s("Map Iterator"));
    assert_eq!(rt.get_v(&Value::Object(set_proto), &PropKey::Sym(rt.well_known(WellKnown::ToStringTag))).expect("tag"), s("Set Iterator"));
    assert_eq!(rt.get_prototype_of(map_proto), rt.builtin("IteratorPrototype"));
    assert_eq!(rt.get_prototype_of(set_proto), rt.builtin("IteratorPrototype"));
    let method = rt.get_v(&map_it, &PropKey::Sym(rt.well_known(WellKnown::Iterator))).expect("method");
    assert_eq!(rt.call(&method, &map_it, &[]).expect("self"), map_it);
}

#[test]
fn next_rejects_iterators_of_another_family_and_other_receivers() {
    // V8 14: Object.getPrototypeOf(new Map().keys()).next.call(new Set().values()) and .call({}) are TypeErrors.
    let rt = Rt::new();
    let m = construct(&rt, "Map", &[]).expect("map");
    let set = construct(&rt, "Set", &[]).expect("set");
    let map_it = invoke(&rt, &m, "keys", &[]).expect("keys");
    let set_it = invoke(&rt, &set, "values", &[]).expect("values");
    let map_proto = rt.get_prototype_of(map_it.as_object().expect("object")).expect("prototype");
    let next = rt.get_v(&Value::Object(map_proto), &PropKey::from_str("next")).expect("next");
    assert_eq!(error_name(&rt, rt.call(&next, &set_it, &[])), "TypeError");
    assert_eq!(error_name(&rt, rt.call(&next, &Value::Object(rt.new_object()), &[])), "TypeError");
    let done = rt.call(&next, &map_it, &[]).expect("a valid receiver");
    assert_eq!(rt.get_str(&done, "done").expect("done"), Value::Bool(true));
}

// ---------------------------------------------------------------- GC and the iterate() driver

#[test]
fn a_walk_in_progress_keeps_its_map_and_keys_alive() {
    // The iterator traces its collection, so the map and its keys survive a collection that happens mid-walk.
    let rt = Rt::new();
    let key = rt.new_object();
    let m = construct(&rt, "Map", &[iterable(&rt, vec![arr(&rt, vec![Value::Object(key), num(7.0)])])]).expect("map");
    let map_ref = m.as_object().expect("object");
    let it = invoke(&rt, &m, "keys", &[]).expect("keys");
    let root = rt.root(&it);
    drop(m);
    rt.collect();
    assert!(rt.is_live(map_ref), "the walk keeps its collection");
    let first = step(&rt, &rt.root_value(root)).expect("first key");
    assert_eq!(first, Value::Object(key));
    assert!(rt.is_live(key));
    rt.collect();
    assert_eq!(step(&rt, &rt.root_value(root)), None);
    rt.unroot(root);
}

#[test]
fn iterate_driver_walks_a_set_and_a_map() {
    // B0's iterate() drives the iteration protocol of both collections.
    let rt = Rt::new();
    let set = set_of(&rt, &[4.0, 5.0]);
    let mut members = Vec::new();
    iterate(&rt, &set, &mut |v| {
        members.push(v);
        Ok(true)
    })
    .expect("iterate set");
    assert_eq!(numbers(&members), vec![4.0, 5.0]);
    let m = map_of(&rt, &[6.0]);
    let mut pairs = Vec::new();
    iterate(&rt, &m, &mut |v| {
        pairs.push(v);
        Ok(true)
    })
    .expect("iterate map");
    assert_eq!(pairs.len(), 1);
}

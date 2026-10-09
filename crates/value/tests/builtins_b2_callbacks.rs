//! WP-B2 callback behaviour with native callbacks (ECMA-262 23.1.3): call order, holes, mutation during iteration,
//! thisArg, errors and collection safety. The corpus (tests/language/builtins/array-callbacks.json) covers the presets
//! of map, reduce and sort; these cases cover the other callback methods. Expected values are V8's (Node 26), recorded
//! from the same scenarios in a probe.

use std::cell::RefCell;

use vn_value::{Call, Intrinsic, PropKey, Res, Rt, Throw, Value, WellKnown};

thread_local! {
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn log(entry: String) {
    LOG.with(|l| l.borrow_mut().push(entry));
}

fn take_log() -> Vec<String> {
    LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

fn num_of(v: &Value) -> f64 {
    match v {
        Value::Number(n) => *n,
        _ => f64::NAN,
    }
}

fn text(rt: &Rt, v: &Value) -> String {
    match v {
        Value::Undefined => "undefined".into(),
        Value::Object(_) => "object".into(),
        _ => rt.to_string(v).map(|s| s.to_rust_string_lossy()).unwrap_or_else(|_| "?".into()),
    }
}

/// Renders arrays as `[a,<hole>,b]` (holes shown as `<hole>`) and primitives by their string form.
fn render(rt: &Rt, v: &Value) -> String {
    match v {
        Value::Object(o) if rt.is_array(v) => {
            let len = num_of(&rt.get_str(v, "length").unwrap_or(Value::Undefined)) as u32;
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

fn error_name(rt: &Rt, e: &Throw) -> String {
    text(rt, &rt.get_str(&e.0, "name").unwrap_or(Value::Undefined))
}

fn arr(rt: &Rt, elems: Vec<Value>) -> Value {
    Value::Object(rt.new_array(elems))
}

fn method(rt: &Rt, this: &Value, name: &str, args: &[Value]) -> Res<Value> {
    let f = rt.get_str(this, name)?;
    rt.call(&f, this, args)
}

fn native(rt: &Rt, code: vn_value::CodeFn) -> Value {
    Value::Object(rt.new_native_function(code, "cb", 3, Vec::new()))
}

// ----- native callbacks -----

/// Logs its index; returns undefined.
fn cb_log_only(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log(format!("i{}", num_of(&c.arg(1))));
    Ok(Value::Undefined)
}

/// Logs its index; returns x > 1.
fn cb_gt1(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log(format!("i{}", num_of(&c.arg(1))));
    Ok(Value::Bool(num_of(&c.arg(0)) > 1.0))
}

/// Logs its index; returns x == 2.
fn cb_eq2(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log(format!("i{}", num_of(&c.arg(1))));
    Ok(Value::Bool(num_of(&c.arg(0)) == 2.0))
}

/// Logs its index; returns x < 2.
fn cb_lt2(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log(format!("i{}", num_of(&c.arg(1))));
    Ok(Value::Bool(num_of(&c.arg(0)) < 2.0))
}

/// Logs its index; returns x < 3.
fn cb_lt3(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log(format!("i{}", num_of(&c.arg(1))));
    Ok(Value::Bool(num_of(&c.arg(0)) < 3.0))
}

/// Logs its index; returns whether x is undefined (find reads holes as undefined).
fn cb_is_undefined(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log(format!("i{}", num_of(&c.arg(1))));
    Ok(Value::Bool(c.arg(0).is_undefined()))
}

/// reduceRight callback: logs the index (third argument) and returns `acc|x` as a string.
fn cb_cat(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log(format!("i{}", num_of(&c.arg(2))));
    let acc = text(rt, &c.arg(0));
    let x = text(rt, &c.arg(1));
    Ok(Value::str(&format!("{acc}|{x}")))
}

/// Pushes its argument onto the array it is called on (thisArg) and logs "call".
fn cb_push_self(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log("call".into());
    method(rt, c.this, "push", &[c.arg(0)])?;
    Ok(Value::Undefined)
}

/// Sets `length` of the array it is called on to 1, logs its index and returns x.
fn cb_shrink_self(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log(format!("i{}", num_of(&c.arg(1))));
    rt.set_str(c.this, "length", Value::Number(1.0))?;
    Ok(c.arg(0))
}

/// Logs its index and throws a TypeError at index 1; otherwise returns x.
fn cb_throw_at_1(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let i = num_of(&c.arg(1));
    log(format!("i{i}"));
    if i == 1.0 {
        return Err(rt.throw_type_error("boom"));
    }
    Ok(c.arg(0))
}

/// Returns [x, [x]].
fn cb_pair(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let inner = arr(rt, vec![c.arg(0)]);
    Ok(arr(rt, vec![c.arg(0), inner]))
}

/// Returns [<hole>, 2] (a hole in a flatMap result is dropped).
fn cb_hole_pair(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(arr(rt, vec![Value::Hole, Value::Number(2.0)]))
}

/// Returns x * 10 + index.
fn cb_tens_plus_index(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Number(num_of(&c.arg(0)) * 10.0 + num_of(&c.arg(1))))
}

/// Returns the index.
fn cb_return_index(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(c.arg(1))
}

/// Logs whether `this` is an object.
fn cb_this_kind(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log(if c.this.is_object() { "this:object".into() } else { "this:other".into() });
    Ok(Value::Undefined)
}

/// A comparator that throws on its second call.
fn cmp_throw_second(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let n = LOG.with(|l| {
        let mut l = l.borrow_mut();
        l.push("cmp".into());
        l.iter().filter(|e| *e == "cmp").count()
    });
    if n == 2 {
        return Err(rt.throw_error(vn_value::ErrorKind::Error, "cmp"));
    }
    Ok(Value::Number(num_of(&c.arg(0)) - num_of(&c.arg(1))))
}

// ----- an iterable whose iterator records `return` -----

thread_local! {
    static ITER_NEXT: RefCell<f64> = const { RefCell::new(0.0) };
}

fn iter_next(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    let v = ITER_NEXT.with(|n| {
        let mut n = n.borrow_mut();
        let v = *n;
        *n += 1.0;
        v
    });
    Ok(vn_value::builtins::iterator::create_iter_result(rt, Value::Number(v), v > 2.0))
}

fn iter_return(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    log("return".into());
    Ok(Value::Object(rt.new_object()))
}

/// `[Symbol.iterator]()` of the iterable: a fresh iterator object with `next` and `return`.
fn make_iterator(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    let it = rt.new_object();
    rt.define_data(it, PropKey::from_str("next"), native_iter(rt, iter_next), vn_value::Flags::ALL)?;
    rt.define_data(it, PropKey::from_str("return"), native_iter(rt, iter_return), vn_value::Flags::ALL)?;
    Ok(Value::Object(it))
}

fn native_iter(rt: &Rt, code: vn_value::CodeFn) -> Value {
    Value::Object(rt.new_native_function(code, "f", 0, Vec::new()))
}

// ----- tests -----

#[test]
fn for_each_visits_present_indices_in_order_and_skips_holes() {
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0), Value::Hole, Value::Number(3.0)]);
    method(&rt, &a, "forEach", &[native(&rt, cb_log_only)]).expect("forEach");
    assert_eq!(take_log(), ["i0", "i2"]);
}

#[test]
fn for_each_ignores_elements_pushed_during_the_loop() {
    // V8: the length is fixed when the call starts, so two callbacks run and the array ends with four elements.
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0), Value::Number(2.0)]);
    method(&rt, &a, "forEach", &[native(&rt, cb_push_self), a.clone()]).expect("forEach");
    // thisArg is the array: the callback pushes onto it.
    assert_eq!(render(&rt, &a), "[1,2,1,2]");
    assert_eq!(take_log(), ["call", "call"]);
}

#[test]
fn map_with_a_shrinking_array_keeps_holes_and_stops_early() {
    // V8: after the first callback truncates the array, indices 1 and 2 are absent, so only index 0 is visited.
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]);
    let out = method(&rt, &a, "map", &[native(&rt, cb_shrink_self), a.clone()]).expect("map");
    assert_eq!(take_log(), ["i0"]);
    assert_eq!(render(&rt, &out), "[1,<hole>,<hole>]");
}

#[test]
fn map_result_keeps_the_holes_of_the_source() {
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0), Value::Hole, Value::Number(3.0)]);
    let out = method(&rt, &a, "map", &[native(&rt, cb_return_index)]).expect("map");
    assert_eq!(render(&rt, &out), "[0,<hole>,2]");
}

#[test]
fn filter_skips_holes_and_keeps_order() {
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0), Value::Hole, Value::Number(3.0), Value::Number(4.0)]);
    let out = method(&rt, &a, "filter", &[native(&rt, cb_gt1)]).expect("filter");
    assert_eq!(take_log(), ["i0", "i2", "i3"]);
    assert_eq!(render(&rt, &out), "[3,4]");
}

#[test]
fn some_and_every_stop_at_the_deciding_element() {
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]);
    assert_eq!(method(&rt, &a, "some", &[native(&rt, cb_eq2)]).expect("some"), Value::Bool(true));
    assert_eq!(take_log(), ["i0", "i1"]);
    assert_eq!(method(&rt, &a, "every", &[native(&rt, cb_lt2)]).expect("every"), Value::Bool(false));
    assert_eq!(take_log(), ["i0", "i1"]);
}

#[test]
fn find_index_reads_holes_as_undefined() {
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Hole, Value::Number(2.0)]);
    assert_eq!(method(&rt, &a, "findIndex", &[native(&rt, cb_is_undefined)]).expect("findIndex"), Value::Number(0.0));
    assert_eq!(take_log(), ["i0"]);
}

#[test]
fn find_last_and_find_last_index_scan_from_the_end() {
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]);
    assert_eq!(method(&rt, &a, "findLast", &[native(&rt, cb_lt3)]).expect("findLast"), Value::Number(2.0));
    assert_eq!(take_log(), ["i2", "i1"]);
    assert_eq!(method(&rt, &a, "findLastIndex", &[native(&rt, cb_lt3)]).expect("findLastIndex"), Value::Number(1.0));
    assert_eq!(take_log(), ["i2", "i1"]);
}

#[test]
fn reduce_right_skips_holes() {
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0), Value::Hole, Value::Number(3.0)]);
    let out = method(&rt, &a, "reduceRight", &[native(&rt, cb_cat)]).expect("reduceRight");
    assert_eq!(text(&rt, &out), "3|1");
    assert_eq!(take_log(), ["i0"]);
}

#[test]
fn reduce_without_initial_value_starts_at_the_first_present_element() {
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Hole, Value::Hole, Value::Number(5.0)]);
    assert_eq!(method(&rt, &a, "reduce", &[native(&rt, cb_cat)]).expect("reduce"), Value::Number(5.0));
    assert_eq!(take_log().len(), 0);
}

#[test]
fn reduce_over_only_holes_returns_the_initial_value() {
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Hole, Value::Hole]);
    let init = Value::str("init");
    assert_eq!(method(&rt, &a, "reduce", &[native(&rt, cb_cat), init]).expect("reduce"), Value::str("init"));
    assert_eq!(take_log().len(), 0);
}

#[test]
fn reduce_with_no_elements_and_no_initial_value_is_a_type_error() {
    let rt = Rt::new();
    let a = arr(&rt, vec![]);
    let err = method(&rt, &a, "reduce", &[native(&rt, cb_cat)]).expect_err("reduce");
    assert_eq!(error_name(&rt, &err), "TypeError");
}

#[test]
fn flat_map_flattens_one_level() {
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0), Value::Number(2.0)]);
    let out = method(&rt, &a, "flatMap", &[native(&rt, cb_pair)]).expect("flatMap");
    assert_eq!(render(&rt, &out), "[1,[1],2,[2]]");
}

#[test]
fn flat_map_drops_holes_of_the_mapper_result() {
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0)]);
    let out = method(&rt, &a, "flatMap", &[native(&rt, cb_hole_pair)]).expect("flatMap");
    assert_eq!(render(&rt, &out), "[2]");
}

#[test]
fn for_each_error_stops_the_loop_and_propagates() {
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]);
    let err = method(&rt, &a, "forEach", &[native(&rt, cb_throw_at_1)]).expect_err("forEach");
    assert_eq!(error_name(&rt, &err), "TypeError");
    assert_eq!(take_log(), ["i0", "i1"]);
}

#[test]
fn map_callbacks_receive_the_this_argument() {
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0)]);
    let this_arg = Value::Object(rt.new_object());
    method(&rt, &a, "map", &[native(&rt, cb_this_kind), this_arg]).expect("map");
    assert_eq!(take_log(), ["this:object"]);
}

#[test]
fn array_from_with_a_mapper_receives_the_index() {
    let rt = Rt::new();
    let array_ctor = rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), "Array").expect("Array");
    let src = arr(&rt, vec![Value::Number(1.0), Value::Number(2.0)]);
    let out = method(&rt, &array_ctor, "from", &[src, native(&rt, cb_tens_plus_index)]).expect("from");
    assert_eq!(render(&rt, &out), "[10,21]");
    let len3 = rt.new_object();
    rt.define_data(len3, PropKey::from_str("length"), Value::Number(3.0), vn_value::Flags::ALL).expect("length");
    let out = method(&rt, &array_ctor, "from", &[Value::Object(len3), native(&rt, cb_return_index)])
        .expect("from");
    assert_eq!(render(&rt, &out), "[0,1,2]");
}

#[test]
fn array_from_closes_the_iterator_when_the_mapper_throws() {
    // V8: the mapper throws on the second value, the iterator is closed once (return is called) and the error propagates.
    let rt = Rt::new();
    ITER_NEXT.with(|n| *n.borrow_mut() = 0.0);
    let iterable = rt.new_object();
    let f = native_iter(&rt, make_iterator);
    rt.define_data(iterable, PropKey::Sym(rt.well_known(WellKnown::Iterator)), f, vn_value::Flags::ALL)
        .expect("iterator method");
    let array_ctor = rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), "Array").expect("Array");
    let err = method(&rt, &array_ctor, "from", &[Value::Object(iterable), native(&rt, cb_throw_at_1)])
        .expect_err("from");
    assert_eq!(error_name(&rt, &err), "TypeError");
    let entries = take_log();
    assert_eq!(entries.iter().filter(|e| *e == "return").count(), 1, "log: {entries:?}");
}

#[test]
fn sort_leaves_the_array_unchanged_when_the_comparator_throws() {
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(3.0), Value::Number(1.0), Value::Number(2.0)]);
    let err = method(&rt, &a, "sort", &[native(&rt, cmp_throw_second)]).expect_err("sort");
    assert_eq!(error_name(&rt, &err), "Error");
    assert_eq!(render(&rt, &a), "[3,1,2]");
    take_log();
}

#[test]
fn a_collected_result_survives_a_collection_while_rooted() {
    // The result is rooted, the collector runs at depth 0 between calls, and the result still reads back.
    let rt = Rt::new();
    let a = arr(&rt, vec![Value::Number(1.0), Value::Number(2.0)]);
    let out = method(&rt, &a, "map", &[native(&rt, cb_return_index)]).expect("map");
    let id = rt.root(&out);
    rt.collect();
    assert_eq!(render(&rt, &rt.root_value(id)), "[0,1]");
    rt.unroot(id);
}

// ----- mutation during a callback (expectations from V8, the probe recorded with these scenarios) -----

fn num(n: f64) -> Value {
    Value::Number(n)
}

fn global_array(rt: &Rt) -> Value {
    rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), "Array").expect("Array")
}

/// A native callback whose closure data is one value (for callbacks that act on an array other than thisArg).
fn native_with_data(rt: &Rt, code: vn_value::CodeFn, data: Value) -> Value {
    Value::Object(rt.new_native_function(code, "cb", 3, vec![data]))
}

/// filter: shrinks the array it runs on (thisArg) to length 1 at index 0.
fn cb_m1_filter_shrink(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log(format!("i{}", num_of(&c.arg(1))));
    rt.set_str(c.this, "length", num(1.0))?;
    Ok(Value::Bool(true))
}

/// find: pushes 9 onto the array (thisArg) at index 0; find keeps the length it started with.
fn cb_m2_find_push(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log(format!("i{}", num_of(&c.arg(1))));
    if num_of(&c.arg(1)) == 0.0 {
        method(rt, c.this, "push", &[num(9.0)])?;
    }
    Ok(Value::Bool(num_of(&c.arg(0)) == 9.0))
}

/// some: pops the array (thisArg) at every call and answers false.
fn cb_m3_some_pop(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log(format!("i{}", num_of(&c.arg(1))));
    method(rt, c.this, "pop", &[])?;
    Ok(Value::Bool(false))
}

/// reduce: at index 0 writes 30 to element 2 of the array (closure data); the visit of index 2 reads it.
fn cb_m4_reduce_write(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log(format!("i{}", num_of(&c.arg(2))));
    if num_of(&c.arg(2)) == 0.0 {
        let target = rt.fn_data(c.callee)[0].clone();
        rt.set_str(&target, "2", num(30.0))?;
    }
    Ok(Value::Number(num_of(&c.arg(0)) + num_of(&c.arg(1))))
}

/// map: at index 0 writes 9 to element 2 of the array (thisArg); the visit of index 2 reads 9.
fn cb_m5_map_write(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    if num_of(&c.arg(1)) == 0.0 {
        rt.set_str(c.this, "2", num(9.0))?;
    }
    Ok(c.arg(0))
}

/// forEach: deletes element 1 of the array (thisArg) at index 0; the later visit of index 1 is skipped.
fn cb_m6_foreach_delete(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log(format!("i{}", num_of(&c.arg(1))));
    if num_of(&c.arg(1)) == 0.0 {
        if let Value::Object(o) = c.this {
            rt.delete(*o, &PropKey::Index(1))?;
        }
    }
    Ok(Value::Undefined)
}

/// sort comparator: pushes 9 onto the array (closure data) on every call.
fn cmp_push_data(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let target = rt.fn_data(c.callee)[0].clone();
    method(rt, &target, "push", &[num(9.0)])?;
    Ok(Value::Number(num_of(&c.arg(0)) - num_of(&c.arg(1))))
}

/// Array.from mapper: pushes 2 onto the source (closure data) at index 0; the live iterator yields it.
fn cb_m8_from_push(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    if num_of(&c.arg(1)) == 0.0 {
        let items = rt.fn_data(c.callee)[0].clone();
        method(rt, &items, "push", &[num(2.0)])?;
    }
    Ok(c.arg(0))
}

/// toSorted comparator: writes 7 to index 0 of the source array (closure data).
fn cmp_write_data(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let target = rt.fn_data(c.callee)[0].clone();
    rt.set_str(&target, "0", num(7.0))?;
    Ok(Value::Number(num_of(&c.arg(0)) - num_of(&c.arg(1))))
}

/// findLast: shrinks the array (thisArg) to length 2 at index 3; findLast reads every index without HasProperty.
fn cb_m10_findlast_shrink(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log(format!("i{}", num_of(&c.arg(1))));
    if num_of(&c.arg(1)) == 3.0 {
        rt.set_str(c.this, "length", num(2.0))?;
    }
    Ok(Value::Bool(false))
}

/// flatMap: pushes 5 onto the array (thisArg) at index 0; flatMap keeps the length it started with.
fn cb_m11_flatmap_push(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log(format!("i{}", num_of(&c.arg(1))));
    if num_of(&c.arg(1)) == 0.0 {
        method(rt, c.this, "push", &[num(5.0)])?;
    }
    Ok(Value::Object(rt.new_array(vec![c.arg(0)])))
}

/// reduceRight: pops the array (closure data) at index 2, the first visit.
fn cb_m12_reduceright_pop(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    log(format!("i{}", num_of(&c.arg(2))));
    if num_of(&c.arg(2)) == 2.0 {
        let target = rt.fn_data(c.callee)[0].clone();
        method(rt, &target, "pop", &[])?;
    }
    Ok(Value::Number(num_of(&c.arg(0)) + num_of(&c.arg(1))))
}

#[test]
fn filter_that_shrinks_the_array_skips_the_missing_indices() {
    // V8: filter visits index 0 only; the result is [1] and the array has length 1.
    let rt = Rt::new();
    let a = arr(&rt, vec![num(1.0), num(2.0), num(3.0)]);
    let out = method(&rt, &a, "filter", &[native(&rt, cb_m1_filter_shrink), a.clone()]).expect("filter");
    assert_eq!(take_log(), ["i0"]);
    assert_eq!(render(&rt, &out), "[1]");
    assert_eq!(render(&rt, &a), "[1]");
}

#[test]
fn find_keeps_the_length_it_started_with() {
    // V8: the callback pushes 9 at index 0 and index 1 is never visited, so find returns undefined and the array is [1,9].
    let rt = Rt::new();
    let a = arr(&rt, vec![num(1.0)]);
    let out = method(&rt, &a, "find", &[native(&rt, cb_m2_find_push), a.clone()]).expect("find");
    assert!(out.is_undefined());
    assert_eq!(take_log(), ["i0"]);
    assert_eq!(render(&rt, &a), "[1,9]");
}

#[test]
fn some_treats_an_element_removed_during_the_loop_as_absent() {
    // V8: the array pops at every call; index 2 is absent by its visit and skipped. Result false, array [1].
    let rt = Rt::new();
    let a = arr(&rt, vec![num(1.0), num(2.0), num(3.0)]);
    let out = method(&rt, &a, "some", &[native(&rt, cb_m3_some_pop), a.clone()]).expect("some");
    assert_eq!(out, Value::Bool(false));
    assert_eq!(take_log(), ["i0", "i1"]);
    assert_eq!(render(&rt, &a), "[1]");
}

#[test]
fn reduce_reads_each_element_when_it_is_visited() {
    // V8: index 0 writes 30 to element 2, so the visit of index 2 reads 30: 1 + 2 + 30 = 33.
    let rt = Rt::new();
    let a = arr(&rt, vec![num(1.0), num(2.0), num(3.0)]);
    let cb = native_with_data(&rt, cb_m4_reduce_write, a.clone());
    let out = method(&rt, &a, "reduce", &[cb, num(0.0)]).expect("reduce");
    assert_eq!(out, num(33.0));
    assert_eq!(take_log(), ["i0", "i1", "i2"]);
}

#[test]
fn map_reads_each_element_when_it_is_visited() {
    // V8: index 0 writes 9 to element 2, so the map of index 2 returns 9: [1,2,9].
    let rt = Rt::new();
    let a = arr(&rt, vec![num(1.0), num(2.0), num(3.0)]);
    let out = method(&rt, &a, "map", &[native(&rt, cb_m5_map_write), a.clone()]).expect("map");
    assert_eq!(render(&rt, &out), "[1,2,9]");
}

#[test]
fn for_each_skips_an_element_deleted_before_its_visit() {
    // V8: deleting element 1 at index 0 makes index 1 absent, so the visits are 0 and 2.
    let rt = Rt::new();
    let a = arr(&rt, vec![num(1.0), num(2.0), num(3.0)]);
    method(&rt, &a, "forEach", &[native(&rt, cb_m6_foreach_delete), a.clone()]).expect("forEach");
    assert_eq!(take_log(), ["i0", "i2"]);
}

#[test]
fn sort_writes_back_within_the_length_it_started_with() {
    // V8: each of the three comparator calls pushes 9; the sorted values go to indices 0..3 and the pushed elements stay.
    let rt = Rt::new();
    let a = arr(&rt, vec![num(3.0), num(1.0), num(2.0)]);
    let cmp = native_with_data(&rt, cmp_push_data, a.clone());
    let out = method(&rt, &a, "sort", &[cmp]).expect("sort");
    assert_eq!(render(&rt, &out), "[1,2,3,9,9,9]");
}

#[test]
fn array_from_reads_its_iterable_live() {
    // V8: the mapper pushes 2 onto the source at index 0 and the array iterator yields it: [1,2].
    let rt = Rt::new();
    let items = arr(&rt, vec![num(1.0)]);
    let mapper = native_with_data(&rt, cb_m8_from_push, items.clone());
    let out = method(&rt, &global_array(&rt), "from", &[items.clone(), mapper]).expect("from");
    assert_eq!(render(&rt, &out), "[1,2]");
}

#[test]
fn to_sorted_sorts_a_snapshot_of_the_source() {
    // V8: the comparator writes 7 into the source; toSorted sorted the values it read first: [1,2,3], and the source is [7,2,3].
    let rt = Rt::new();
    let a = arr(&rt, vec![num(1.0), num(2.0), num(3.0)]);
    let cmp = native_with_data(&rt, cmp_write_data, a.clone());
    let out = method(&rt, &a, "toSorted", &[cmp]).expect("toSorted");
    assert_eq!(render(&rt, &out), "[1,2,3]");
    assert_eq!(render(&rt, &a), "[7,2,3]");
}

#[test]
fn find_last_reads_removed_elements_as_undefined() {
    // V8: findLast shrinks the array at index 3; the later visits read the missing elements as undefined.
    let rt = Rt::new();
    let a = arr(&rt, vec![num(1.0), num(2.0), num(3.0), num(4.0)]);
    let out = method(&rt, &a, "findLast", &[native(&rt, cb_m10_findlast_shrink), a.clone()]).expect("findLast");
    assert!(out.is_undefined());
    assert_eq!(take_log(), ["i3", "i2", "i1", "i0"]);
}

#[test]
fn flat_map_keeps_the_length_it_started_with() {
    // V8: the callback pushes 5 at index 0; flatMap visits indices 0 and 1 only: [1,2].
    let rt = Rt::new();
    let a = arr(&rt, vec![num(1.0), num(2.0)]);
    let out = method(&rt, &a, "flatMap", &[native(&rt, cb_m11_flatmap_push), a.clone()]).expect("flatMap");
    assert_eq!(render(&rt, &out), "[1,2]");
    assert_eq!(take_log(), ["i0", "i1"]);
}

#[test]
fn reduce_right_follows_the_array_shortened_during_the_loop() {
    // V8: the callback pops at index 2, the first visit; the later visits read the remaining elements: 3 + 2 + 1 = 6.
    let rt = Rt::new();
    let a = arr(&rt, vec![num(1.0), num(2.0), num(3.0)]);
    let cb = native_with_data(&rt, cb_m12_reduceright_pop, a.clone());
    let out = method(&rt, &a, "reduceRight", &[cb, num(0.0)]).expect("reduceRight");
    assert_eq!(out, num(6.0));
    assert_eq!(take_log(), ["i2", "i1", "i0"]);
}

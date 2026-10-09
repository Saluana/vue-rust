//! Kernel functions: `name`/`length` properties, call and construct plumbing (`this`, arguments,
//! new.target), captured cells and closure data, the call-depth limit and error propagation. Expected values
//! are written from ECMA-262 15.2 / 10.2 or are V8's where a comment says so.

use vn_value::{Call, CodeFn, Flags, FnInfo, FnKind, JsStr, Prop, PropKey, Res, Rt, Throw, Value};

fn key(s: &str) -> PropKey {
    PropKey::from_str(s)
}

fn num(n: f64) -> Value {
    Value::Number(n)
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

fn text_of(rt: &Rt, v: &Value, name: &str) -> String {
    match rt.get_str(v, name) {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => String::from("<none>"),
    }
}

// ----- native code used by the tests -----

fn echo_this_and_first_arg(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let _ = rt;
    let arr = rt.new_array(vec![c.this.clone(), c.arg(0), c.arg(1)]);
    Ok(Value::Object(arr))
}

fn report_new_target(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(c.new_target.clone())
}

fn report_depth(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(num(f64::from(rt.call_depth())))
}

fn code_a(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::str("called"))
}

fn code_b(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::str("constructed"))
}

fn throw_range(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_range_error("from native"))
}

/// Recurses through `rt.call` on itself, forever. Used to reach the depth limit.
fn recurse_forever(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    rt.call(&Value::Object(c.callee), c.this, &[])
}

/// Counts how often it runs (in the cell in its closure data) and recurses.
fn count_and_recurse(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let Value::Object(cell) = data[0] else { panic!("cell missing from closure data") };
    let n = match rt.cell_get(cell) {
        Value::Number(n) => n,
        _ => 0.0,
    };
    rt.cell_set(cell, num(n + 1.0));
    rt.call(&Value::Object(c.callee), c.this, &[])
}

fn read_capture(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let caps = rt.captures(c.callee);
    Ok(rt.cell_get(caps[0]))
}

fn read_data(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(rt.fn_data(c.callee)[1].clone())
}

fn no_op(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Undefined)
}

// ----- name and length -----

#[test]
fn name_and_length_are_own_read_only_configurable_properties() {
    let rt = Rt::new();
    let f = rt.new_function(no_op, FnInfo::new("compiled", 2, FnKind::Normal), &[]);
    let fv = Value::Object(f);
    let name = ok(&rt, rt.get_own_property(f, &key("name"))).unwrap();
    assert_eq!(name, Prop::Data { value: Value::str("compiled"), flags: Flags::CONFIG });
    let length = ok(&rt, rt.get_own_property(f, &key("length"))).unwrap();
    assert_eq!(length, Prop::Data { value: num(2.0), flags: Flags::CONFIG });
    assert_eq!(text_of(&rt, &fv, "name"), "compiled");
}

#[test]
fn length_precedes_name_in_own_key_order() {
    // V8: Reflect.ownKeys(function f(){}) starts with length, name. Our functions have no `prototype`
    // object, so the list is exactly these two.
    let rt = Rt::new();
    let f = rt.new_native_function(no_op, "nat", 0, vec![]);
    let keys = ok(&rt, rt.own_keys(f));
    assert_eq!(keys, vec![key("length"), key("name")]);
}

#[test]
fn name_and_length_are_not_writable_but_can_be_redefined() {
    let rt = Rt::new();
    let f = rt.new_native_function(no_op, "nat", 1, vec![]);
    assert!(!ok(&rt, rt.set(f, &key("name"), Value::str("x"), &Value::Object(f))));
    assert!(ok(&rt, rt.define_data(f, key("name"), Value::str("renamed"), Flags::CONFIG)));
    assert_eq!(text_of(&rt, &Value::Object(f), "name"), "renamed");
}

#[test]
fn function_length_and_name_are_zero_and_empty_for_function_prototype() {
    let rt = Rt::new();
    let fp = rt.intrinsic(vn_value::Intrinsic::FunctionPrototype);
    assert!(rt.is_callable(&Value::Object(fp)));
    assert_eq!(ok(&rt, rt.get(fp, &key("length"), &Value::Object(fp))), num(0.0));
    assert_eq!(ok(&rt, rt.get(fp, &key("name"), &Value::Object(fp))), Value::str(""));
    assert_eq!(ok(&rt, rt.call(&Value::Object(fp), &Value::Undefined, &[])), Value::Undefined);
}

// ----- call plumbing -----

#[test]
fn call_passes_this_and_arguments_through() {
    let rt = Rt::new();
    let f = rt.new_native_function(echo_this_and_first_arg, "echo", 2, vec![]);
    let this = Value::str("receiver");
    let args = [num(1.0), Value::Bool(true)];
    let out = ok(&rt, rt.call(&Value::Object(f), &this, &args));
    let arr = out.as_object().expect("array result");
    assert_eq!(ok(&rt, rt.get(arr, &PropKey::Index(0), &out)), Value::str("receiver"));
    assert_eq!(ok(&rt, rt.get(arr, &PropKey::Index(1), &out)), num(1.0));
    assert_eq!(ok(&rt, rt.get(arr, &PropKey::Index(2), &out)), Value::Bool(true));
}

#[test]
fn missing_arguments_read_as_undefined() {
    let rt = Rt::new();
    let f = rt.new_native_function(echo_this_and_first_arg, "echo", 2, vec![]);
    let out = ok(&rt, rt.call(&Value::Object(f), &Value::Undefined, &[num(4.0)]));
    let arr = out.as_object().unwrap();
    assert_eq!(ok(&rt, rt.get(arr, &PropKey::Index(2), &out)), Value::Undefined);
}

#[test]
fn plain_call_has_undefined_new_target() {
    let rt = Rt::new();
    let f = rt.new_native_function(report_new_target, "nt", 0, vec![]);
    assert_eq!(ok(&rt, rt.call(&Value::Object(f), &Value::Undefined, &[])), Value::Undefined);
}

#[test]
fn construct_passes_new_target_and_calls_the_constructor_code() {
    let rt = Rt::new();
    let c = rt.new_native_constructor(code_a, report_new_target, "C", 0, vec![]);
    let cv = Value::Object(c);
    let made = ok(&rt, rt.construct(&cv, &[], &cv));
    assert_eq!(made, cv, "new.target is the constructor");
    // A plain call uses the call code, not the constructor code.
    let c2 = rt.new_native_constructor(code_a, code_b, "C2", 0, vec![]);
    assert_eq!(ok(&rt, rt.call(&Value::Object(c2), &Value::Undefined, &[])), Value::str("called"));
    assert_eq!(ok(&rt, rt.construct(&Value::Object(c2), &[], &Value::Object(c2))), Value::str("constructed"));
}

#[test]
fn user_functions_and_plain_natives_are_not_constructors() {
    let rt = Rt::new();
    let f = rt.new_function(no_op, FnInfo::new("u", 0, FnKind::Normal), &[]);
    let fv = Value::Object(f);
    assert!(rt.is_callable(&fv));
    assert!(!rt.is_constructor(&fv));
    let t = throws(rt.construct(&fv, &[], &fv));
    assert_eq!(describe(&rt, &t), "TypeError: function is not a constructor");
    let n = rt.new_native_function(no_op, "n", 0, vec![]);
    assert!(!rt.is_constructor(&Value::Object(n)));
}

#[test]
fn construct_rejects_a_non_constructor_new_target() {
    let rt = Rt::new();
    let c = rt.new_native_constructor(code_a, report_new_target, "C", 0, vec![]);
    let plain = rt.new_native_function(no_op, "p", 0, vec![]);
    let t = throws(rt.construct(&Value::Object(c), &[], &Value::Object(plain)));
    assert_eq!(describe(&rt, &t), "TypeError: new.target is not a constructor");
}

#[test]
fn calling_a_non_callable_throws_type_error() {
    let rt = Rt::new();
    for v in [num(1.0), Value::Undefined, Value::str("s"), Value::Object(rt.new_object())] {
        let t = throws(rt.call(&v, &Value::Undefined, &[]));
        assert!(describe(&rt, &t).starts_with("TypeError: "), "got {}", describe(&rt, &t));
        assert!(describe(&rt, &t).ends_with("is not a function"), "got {}", describe(&rt, &t));
    }
}

#[test]
fn a_native_error_propagates_as_a_value_with_its_message() {
    let rt = Rt::new();
    let f = rt.new_native_function(throw_range, "t", 0, vec![]);
    let t = throws(rt.call(&Value::Object(f), &Value::Undefined, &[]));
    assert_eq!(describe(&rt, &t), "RangeError: from native");
}

#[test]
fn call_depth_is_one_inside_a_top_level_call_and_zero_after() {
    let rt = Rt::new();
    let f = rt.new_native_function(report_depth, "d", 0, vec![]);
    assert_eq!(rt.call_depth(), 0);
    assert_eq!(ok(&rt, rt.call(&Value::Object(f), &Value::Undefined, &[])), num(1.0));
    assert_eq!(rt.call_depth(), 0);
}

#[test]
fn the_default_call_limit_is_1500_nested_calls() {
    // Runs on a thread with a large stack: 1500 nested native frames need more than the test default.
    let depth = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| {
            let rt = Rt::new();
            assert_eq!(rt.max_call_depth(), 1500);
            let cell = rt.new_cell(num(0.0));
            let f = rt.new_native_function(count_and_recurse, "rec", 0, vec![Value::Object(cell)]);
            let t = throws(rt.call(&Value::Object(f), &Value::Undefined, &[]));
            assert_eq!(describe(&rt, &t), "RangeError: Maximum call stack size exceeded");
            assert_eq!(rt.call_depth(), 0, "the guard restores the depth on unwinding");
            match rt.cell_get(cell) {
                Value::Number(n) => n,
                other => panic!("counter is {other:?}"),
            }
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(depth, 1500.0);
}

#[test]
fn a_lowered_call_limit_is_honoured_exactly() {
    let depth = std::thread::Builder::new()
        .stack_size(16 << 20)
        .spawn(|| {
            let rt = Rt::new();
            rt.set_max_call_depth(5);
            let cell = rt.new_cell(num(0.0));
            let f = rt.new_native_function(count_and_recurse, "rec", 0, vec![Value::Object(cell)]);
            let _ = rt.call(&Value::Object(f), &Value::Undefined, &[]);
            match rt.cell_get(cell) {
                Value::Number(n) => n,
                other => panic!("counter is {other:?}"),
            }
        })
        .unwrap()
        .join()
        .unwrap();
    // Entries 1..=5 run; the sixth nested call finds depth 5 and throws before running.
    assert_eq!(depth, 5.0);
}

#[test]
fn a_thrown_recursion_error_leaves_the_runtime_usable() {
    let rt = Rt::new();
    rt.set_max_call_depth(8);
    let f = rt.new_native_function(recurse_forever, "forever", 0, vec![]);
    let t = throws(rt.call(&Value::Object(f), &Value::Undefined, &[]));
    assert_eq!(describe(&rt, &t), "RangeError: Maximum call stack size exceeded");
    let g = rt.new_native_function(report_depth, "d", 0, vec![]);
    assert_eq!(ok(&rt, rt.call(&Value::Object(g), &Value::Undefined, &[])), num(1.0));
}

// ----- closure data and captures -----

#[test]
fn captured_cells_are_visible_inside_the_function() {
    let rt = Rt::new();
    let cell = rt.new_cell(Value::str("captured"));
    let f = rt.new_function(read_capture, FnInfo::new("reader", 0, FnKind::Normal), &[cell]);
    assert_eq!(&*rt.captures(f), &[cell]);
    assert_eq!(ok(&rt, rt.call(&Value::Object(f), &Value::Undefined, &[])), Value::str("captured"));
}

#[test]
fn native_closure_data_is_visible_inside_the_function() {
    let rt = Rt::new();
    let f = rt.new_native_function(read_data, "reader", 0, vec![Value::Undefined, Value::str("data")]);
    assert_eq!(rt.fn_data(f).len(), 2);
    assert_eq!(ok(&rt, rt.call(&Value::Object(f), &Value::Undefined, &[])), Value::str("data"));
}

#[test]
fn fn_info_name_and_kind_are_recorded() {
    let rt = Rt::new();
    let info = FnInfo { name: JsStr::from_str("arrowish"), length: 3, kind: FnKind::Arrow };
    let f = rt.new_function(no_op, info, &[]);
    assert_eq!(text_of(&rt, &Value::Object(f), "name"), "arrowish");
    assert_eq!(ok(&rt, rt.get(f, &key("length"), &Value::Object(f))), num(3.0));
}

#[test]
fn a_function_value_is_not_a_constructor_even_with_a_native_code_pointer() {
    // Only `ctor: Some` makes a constructor. A `CodeFn` pointer alone never does.
    let rt = Rt::new();
    let code: CodeFn = code_a;
    let f = rt.new_function(code, FnInfo::new("plain", 0, FnKind::Method), &[]);
    assert!(!rt.is_constructor(&Value::Object(f)));
    assert!(rt.is_callable(&Value::Object(f)));
}

#[test]
fn calling_a_function_returns_the_code_value_unchanged() {
    let rt = Rt::new();
    let f = rt.new_native_function(code_a, "a", 0, vec![]);
    assert_eq!(ok(&rt, rt.call(&Value::Object(f), &Value::Undefined, &[])), Value::str("called"));
    let _ = FnKind::Getter;
}

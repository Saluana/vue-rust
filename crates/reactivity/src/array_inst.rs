//! Array instrumentation of @vue/reactivity 3.5.43: `arrayInstrumentations` (the functions a reactive array returns from its
//! `get` hook, created once per realm), `reactiveReadArray` and `shallowReadArray`, the search, iteration, apply and reduce
//! helpers and `noTracking`. Behaviour per @vue/reactivity 3.5.43 (production build).

use vn_value::{strict_eq, Call, Flags, Intrinsic, ObjRef, Prop, PropKey, Res, Rt, Value, WellKnown};

use crate::effect::{pause_tracking, reset_tracking};
use crate::graph::{end_batch, start_batch};
use crate::handlers::track_on;
use crate::reactive::{tables, to_readonly_value, to_reactive_value};
use crate::shared::{is_proxy, is_readonly, is_reactive, is_shallow, native, to_raw};
use crate::target::iterate_keys;

/// The methods of `arrayInstrumentations` with their lengths. `@@iterator` is `Symbol.iterator`.
const METHODS: &[(&str, u32)] = &[
    ("@@iterator", 0),
    ("concat", 0),
    ("entries", 0),
    ("every", 1),
    ("filter", 1),
    ("find", 1),
    ("findIndex", 1),
    ("findLast", 1),
    ("findLastIndex", 1),
    ("forEach", 1),
    ("includes", 0),
    ("indexOf", 0),
    ("join", 1),
    ("lastIndexOf", 0),
    ("map", 1),
    ("pop", 0),
    ("push", 0),
    ("reduce", 1),
    ("reduceRight", 1),
    ("shift", 0),
    ("some", 1),
    ("splice", 0),
    ("toReversed", 0),
    ("toSorted", 1),
    ("toSpliced", 0),
    ("unshift", 0),
    ("values", 0),
];

/// Builds `arrayInstrumentations`: a null-prototype object whose own functions carry their method index as data.
pub(crate) fn build(rt: &Rt) -> ObjRef {
    let obj = rt.new_object_with_proto(None);
    for (idx, (name, len)) in METHODS.iter().enumerate() {
        let (key, fname) = if *name == "@@iterator" {
            (PropKey::Sym(rt.well_known(WellKnown::Iterator)), "[Symbol.iterator]")
        } else {
            (PropKey::from_str(name), *name)
        };
        let f = native(rt, fname, *len, dispatch, vec![Value::Number(idx as f64)]);
        rt.define_data(obj, key, Value::Object(f), Flags::ALL).expect("arrayInstrumentations (framework bug)");
    }
    obj
}

/// The instrumentation function of `key` for an array proxy, if the key has one.
pub(crate) fn instrumentation(rt: &Rt, key: &PropKey) -> Option<Value> {
    match rt.get_own_property(tables(rt).arr, key) {
        Ok(Some(Prop::Data { value, .. })) => Some(value),
        _ => None,
    }
}

fn native_fn(rt: &Rt, name: &str, len: u32, code: vn_value::CodeFn) -> Value {
    Value::Object(native(rt, name, len, code, Vec::new()))
}

fn index_data(v: &Value) -> usize {
    match v {
        Value::Number(n) => *n as usize,
        _ => 0,
    }
}

fn dispatch(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let idx = index_data(&rt.fn_data(c.callee)[0]);
    let name = METHODS[idx].0;
    let this = c.this;
    let a0 = c.arg(0);
    let a1 = c.arg(1);
    match name {
        "@@iterator" => iterator(rt, this, PropKey::Sym(rt.well_known(WellKnown::Iterator)), false),
        "values" => iterator(rt, this, PropKey::lit("values"), false),
        "entries" => iterator(rt, this, PropKey::lit("entries"), true),
        "concat" => concat(rt, this, c.args),
        "every" | "forEach" | "map" | "some" | "findIndex" | "findLastIndex" => {
            apply(rt, this, name, &a0, &a1, Ret::None, c.args)
        }
        "filter" => apply(rt, this, name, &a0, &a1, Ret::List, c.args),
        "find" | "findLast" => apply(rt, this, name, &a0, &a1, Ret::Item, c.args),
        "includes" | "indexOf" | "lastIndexOf" => search_proxy(rt, this, name, c.args),
        "join" => {
            let a = reactive_read_array(rt, this)?;
            call_named(rt, &a, "join", &[a0])
        }
        "pop" | "push" | "shift" | "unshift" | "splice" => no_tracking(rt, this, name, c.args),
        "reduce" | "reduceRight" => reduce(rt, this, name, &a0, c.args.get(1..).unwrap_or(&[])),
        "toReversed" => {
            let a = reactive_read_array(rt, this)?;
            call_named(rt, &a, name, &[])
        }
        "toSorted" => {
            let a = reactive_read_array(rt, this)?;
            call_named(rt, &a, name, &[a0])
        }
        "toSpliced" => {
            let a = reactive_read_array(rt, this)?;
            call_named(rt, &a, name, c.args)
        }
        _ => Err(rt.throw_type_error("arrayInstrumentations (framework bug)")),
    }
}

/// `obj[name](...args)`.
fn call_named(rt: &Rt, obj: &Value, name: &str, args: &[Value]) -> Res<Value> {
    let f = rt.get_str(obj, name)?;
    rt.call(&f, obj, args)
}

/// `fn.length` as a number (a non-number length counts as 0; `undefined` throws like the property read does).
fn fn_length(rt: &Rt, f: &Value) -> Res<f64> {
    Ok(match rt.get_v(f, &PropKey::lit("length"))? {
        Value::Number(n) => n,
        _ => 0.0,
    })
}

/// `shallowReadArray(arr)`: the raw array, tracked on ARRAY_ITERATE_KEY.
pub(crate) fn shallow_read_array(rt: &Rt, v: &Value) -> Res<Value> {
    let raw = to_raw(rt, v)?;
    track_on(rt, &raw, &iterate_keys(rt)[2])?;
    Ok(raw)
}

/// `reactiveReadArray(array)`: the raw array when `array` is not a proxy; otherwise a copy of the raw array whose
/// elements are wrapped in the mode of the proxy.
pub(crate) fn reactive_read_array(rt: &Rt, array: &Value) -> Res<Value> {
    let raw = to_raw(rt, array)?;
    if strict_eq(&raw, array) {
        return Ok(raw);
    }
    track_on(rt, &raw, &iterate_keys(rt)[2])?;
    if is_shallow(rt, array)? {
        return Ok(raw);
    }
    if !is_readonly(rt, array)? {
        return call_named(rt, &raw, "map", &[native_fn(rt, "toReactive", 1, to_reactive_item)]);
    }
    if is_reactive(rt, array)? {
        call_named(rt, &raw, "map", &[native_fn(rt, "", 1, readonly_of_reactive_item)])
    } else {
        call_named(rt, &raw, "map", &[native_fn(rt, "toReadonly", 1, to_readonly_item)])
    }
}

fn to_reactive_item(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    to_reactive_value(rt, &c.arg(0))
}

fn to_readonly_item(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    to_readonly_value(rt, &c.arg(0))
}

fn readonly_of_reactive_item(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let r = to_reactive_value(rt, &c.arg(0))?;
    to_readonly_value(rt, &r)
}

/// `toWrapped(target, item)`: the element of an array read through `target`.
pub(crate) fn to_wrapped(rt: &Rt, target: &Value, item: &Value) -> Res<Value> {
    if is_readonly(rt, target)? {
        if is_reactive(rt, target)? {
            let r = to_reactive_value(rt, item)?;
            to_readonly_value(rt, &r)
        } else {
            to_readonly_value(rt, item)
        }
    } else {
        to_reactive_value(rt, item)
    }
}

/// `arrayInstrumentations.includes/indexOf/lastIndexOf` (`searchProxy`): searches the raw array, and retries with the raw
/// argument when a proxy argument was not found.
fn search_proxy(rt: &Rt, this: &Value, method: &str, args: &[Value]) -> Res<Value> {
    let arr = to_raw(rt, this)?;
    track_on(rt, &arr, &iterate_keys(rt)[2])?;
    let f = rt.get_v(&arr, &PropKey::from_str(method))?;
    let mut args = args.to_vec();
    let res = rt.call(&f, &arr, &args)?;
    let missing = matches!(res, Value::Number(n) if n == -1.0) || matches!(res, Value::Bool(false));
    let first = args.first().cloned().unwrap_or(Value::Undefined);
    if missing && is_proxy(rt, &first)? {
        args[0] = to_raw(rt, &first)?;
        let f = rt.get_v(&arr, &PropKey::from_str(method))?;
        return rt.call(&f, &arr, &args);
    }
    Ok(res)
}

/// `noTracking(self, method, args)`: pauses tracking and batches the triggers of a raw array method run on `self`.
fn no_tracking(rt: &Rt, this: &Value, method: &str, args: &[Value]) -> Res<Value> {
    pause_tracking(rt);
    start_batch(rt);
    let raw = to_raw(rt, this)?;
    let f = rt.get_v(&raw, &PropKey::from_str(method))?;
    let res = rt.call(&f, this, args)?;
    end_batch(rt)?;
    reset_tracking(rt);
    Ok(res)
}

/// `arrayInstrumentations.concat(...args)`: spreads the reactive reads of the array arguments.
fn concat(rt: &Rt, this: &Value, args: &[Value]) -> Res<Value> {
    let copy = reactive_read_array(rt, this)?;
    let mut mapped = Vec::with_capacity(args.len());
    for x in args {
        mapped.push(if rt.is_array(x) { reactive_read_array(rt, x)? } else { x.clone() });
    }
    call_named(rt, &copy, "concat", &mapped)
}

#[derive(Clone, Copy, PartialEq)]
enum Ret {
    None,
    Item,
    List,
}

/// The per-item callback wrapper of `apply`: `function (item, index) { return fn.call(this, item', index, self) }`.
fn item_wrapper(rt: &Rt, f: &Value, this: &Value, wrap: bool) -> Value {
    Value::Object(native(rt, "", 2, item_wrapper_call, vec![f.clone(), this.clone(), Value::Bool(wrap)]))
}

fn item_wrapper_call(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let (f, this_v) = (d[0].clone(), d[1].clone());
    let item = if matches!(d[2], Value::Bool(true)) { to_wrapped(rt, &this_v, &c.arg(0))? } else { c.arg(0) };
    rt.call(&f, c.this, &[item, c.arg(1), this_v])
}

/// The element mapper of a `filter` result: `(v) => v.map((item) => toWrapped(this, item))`.
fn element_mapper(rt: &Rt, this: &Value) -> Value {
    Value::Object(native(rt, "", 1, element_mapper_call, vec![this.clone()]))
}

fn element_mapper_call(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let this_v = rt.fn_data(c.callee)[0].clone();
    to_wrapped(rt, &this_v, &c.arg(0))
}

/// `apply(self, method, fn, thisArg, wrappedRetFn, args)`: every/filter/find/forEach/map/some and the find family.
fn apply(rt: &Rt, this: &Value, method: &str, f: &Value, this_arg: &Value, ret: Ret, args: &[Value]) -> Res<Value> {
    let arr = shallow_read_array(rt, this)?;
    let needs_wrap = !strict_eq(&arr, this) && !is_shallow(rt, this)?;
    let method_fn = rt.get_v(&arr, &PropKey::from_str(method))?;
    let array_proto = Value::Object(rt.intrinsic(Intrinsic::ArrayPrototype));
    let builtin_fn = rt.get_v(&array_proto, &PropKey::from_str(method))?;
    if !strict_eq(&method_fn, &builtin_fn) {
        let r = rt.call(&method_fn, this, args)?;
        return if needs_wrap { to_reactive_value(rt, &r) } else { Ok(r) };
    }
    let mut wrapped = f.clone();
    if !strict_eq(&arr, this) {
        if needs_wrap {
            wrapped = item_wrapper(rt, f, this, true);
        } else if fn_length(rt, f)? > 2.0 {
            wrapped = item_wrapper(rt, f, this, false);
        }
    }
    let result = rt.call(&method_fn, &arr, &[wrapped, this_arg.clone()])?;
    if !needs_wrap {
        return Ok(result);
    }
    match ret {
        Ret::None => Ok(result),
        Ret::Item => to_wrapped(rt, this, &result),
        Ret::List => call_named(rt, &result, "map", &[element_mapper(rt, this)]),
    }
}

/// The accumulator wrapper of `reduce`. Data: callback, self, the holder of `wrapInitialAccumulator` (property `w`), and
/// whether items are wrapped.
fn reduce_wrapper(rt: &Rt, f: &Value, this: &Value, holder: ObjRef, wrap: bool) -> Value {
    Value::Object(native(
        rt,
        "",
        3,
        reduce_wrapper_call,
        vec![f.clone(), this.clone(), Value::Object(holder), Value::Bool(wrap)],
    ))
}

fn reduce_wrapper_call(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let (f, this_v, holder) = (d[0].clone(), d[1].clone(), d[2].clone());
    let mut acc = c.arg(0);
    if !matches!(d[3], Value::Bool(true)) {
        return rt.call(&f, c.this, &[acc, c.arg(1), c.arg(2), this_v]);
    }
    if matches!(rt.get_str(&holder, "w")?, Value::Bool(true)) {
        rt.set_str(&holder, "w", Value::Bool(false))?;
        acc = to_wrapped(rt, &this_v, &acc)?;
    }
    let item = to_wrapped(rt, &this_v, &c.arg(1))?;
    rt.call(&f, c.this, &[acc, item, c.arg(2), this_v])
}

/// `reduce(self, method, fn, args)`.
fn reduce(rt: &Rt, this: &Value, method: &str, f: &Value, args: &[Value]) -> Res<Value> {
    let arr = shallow_read_array(rt, this)?;
    let needs_wrap = !strict_eq(&arr, this) && !is_shallow(rt, this)?;
    let holder = rt.new_object();
    rt.define_data(holder, PropKey::lit("w"), Value::Bool(false), Flags::ALL)?;
    let mut wrapped = f.clone();
    if !strict_eq(&arr, this) {
        if needs_wrap {
            rt.define_data(holder, PropKey::lit("w"), Value::Bool(args.is_empty()), Flags::ALL)?;
            wrapped = reduce_wrapper(rt, f, this, holder, true);
        } else if fn_length(rt, f)? > 3.0 {
            wrapped = reduce_wrapper(rt, f, this, holder, false);
        }
    }
    let m = rt.get_v(&arr, &PropKey::from_str(method))?;
    let mut call_args = vec![wrapped];
    call_args.extend_from_slice(args);
    let result = rt.call(&m, &arr, &call_args)?;
    if matches!(rt.get_str(&Value::Object(holder), "w")?, Value::Bool(true)) {
        to_wrapped(rt, this, &result)
    } else {
        Ok(result)
    }
}

/// `iterator(self, method, wrapValue)`: `arr[method]()`, with `next` patched to wrap what it yields.
fn iterator(rt: &Rt, this: &Value, method: PropKey, entries: bool) -> Res<Value> {
    let arr = shallow_read_array(rt, this)?;
    let f = rt.get_v(&arr, &method)?;
    let iter = rt.call(&f, &arr, &[])?;
    if !strict_eq(&arr, this) && !is_shallow(rt, this)? {
        let Some(io) = iter.as_object() else {
            return Err(rt.throw_type_error("array iterator is not an object (framework bug)"));
        };
        let next = rt.get_str(&iter, "next")?;
        rt.define_data(io, PropKey::lit("_next"), next, Flags::ALL)?;
        let data = vec![iter.clone(), this.clone(), Value::Bool(entries)];
        let wrapper = native(rt, "next", 0, iter_next, data);
        rt.set_v_strict(&iter, &PropKey::lit("next"), Value::Object(wrapper))?;
    }
    Ok(iter)
}

/// The patched `next` of `iterator`: wraps `result.value` unless the iterator is done.
fn iter_next(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let (iter, this_v) = (d[0].clone(), d[1].clone());
    let entries = matches!(d[2], Value::Bool(true));
    let next = rt.get_str(&iter, "_next")?;
    let result = rt.call(&next, &iter, &[])?;
    if !rt.get_str(&result, "done")?.to_boolean() {
        let value = rt.get_str(&result, "value")?;
        let wrapped = if entries {
            let v1 = rt.get_v(&value, &PropKey::Index(1))?;
            let w = to_wrapped(rt, &this_v, &v1)?;
            rt.set_v_strict(&value, &PropKey::Index(1), w)?;
            value
        } else {
            to_wrapped(rt, &this_v, &value)?
        };
        rt.set_v_strict(&result, &PropKey::lit("value"), wrapped)?;
    }
    Ok(result)
}

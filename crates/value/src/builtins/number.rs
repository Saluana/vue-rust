//! Builtin family: Number (ECMA-262 21.1). Conversion, formatting and parsing are the finished `number.rs`
//! primitives. The argument checks follow the spec order, and the RangeError messages are V8's. Number.parseFloat
//! and Number.parseInt are the global functions themselves: they are looked up after the global family has
//! installed them (global::install runs before number::install). Number.prototype.toLocaleString is absent.

use crate::function::{Call, CodeFn};
use crate::jsstr::JsStr;
use crate::number;
use crate::object::{Flags, Obj, ObjectKind, Prop};
use crate::rt::{Intrinsic, Rt};
use crate::value::{ObjRef, PropKey, Res, Value};

/// 2^53 - 1, Number.MAX_SAFE_INTEGER.
const MAX_SAFE: f64 = 9_007_199_254_740_991.0;

pub fn install(rt: &Rt) {
    let proto = rt.intrinsic(Intrinsic::NumberPrototype);
    // Number.prototype is itself a Number wrapper whose value is +0 (ECMA-262 21.1.3).
    rt.with_obj_mut(proto, |o| o.kind = ObjectKind::Wrapper(Value::Number(0.0)));
    let ctor = rt.new_native_constructor(number_call, number_construct, "Number", 1, Vec::new());
    rt.define_builtin(ctor, PropKey::from_str("prototype"), Prop::Data { value: Value::Object(proto), flags: Flags::NONE });
    rt.define_builtin(proto, PropKey::from_str("constructor"), Prop::Data { value: Value::Object(ctor), flags: Flags::HIDDEN });

    let consts: &[(&str, f64)] = &[
        ("EPSILON", f64::EPSILON),
        ("MAX_SAFE_INTEGER", MAX_SAFE),
        ("MAX_VALUE", f64::MAX),
        ("MIN_SAFE_INTEGER", -MAX_SAFE),
        ("MIN_VALUE", f64::from_bits(1)),
        ("NaN", f64::NAN),
        ("NEGATIVE_INFINITY", f64::NEG_INFINITY),
        ("POSITIVE_INFINITY", f64::INFINITY),
    ];
    for (name, v) in consts {
        rt.define_builtin(ctor, PropKey::from_str(name), Prop::Data { value: Value::Number(*v), flags: Flags::NONE });
    }

    let statics: &[(&str, CodeFn, u32)] = &[
        ("isFinite", is_finite, 1),
        ("isInteger", is_integer, 1),
        ("isNaN", is_nan, 1),
        ("isSafeInteger", is_safe_integer, 1),
    ];
    for (name, code, len) in statics {
        rt.define_native_method(ctor, name, *code, *len);
    }
    // The global functions are reused, so Number.parseFloat === parseFloat. If the global family has not installed them,
    // this file's own functions are used instead; that only happens if install order changes.
    let parse_float = global_function(rt, "parseFloat")
        .unwrap_or_else(|| Value::Object(rt.new_native_function(parse_float_fallback, "parseFloat", 1, Vec::new())));
    let parse_int = global_function(rt, "parseInt")
        .unwrap_or_else(|| Value::Object(rt.new_native_function(parse_int_fallback, "parseInt", 2, Vec::new())));
    rt.define_builtin(ctor, PropKey::from_str("parseFloat"), Prop::Data { value: parse_float, flags: Flags::HIDDEN });
    rt.define_builtin(ctor, PropKey::from_str("parseInt"), Prop::Data { value: parse_int, flags: Flags::HIDDEN });

    let methods: &[(&str, CodeFn, u32)] = &[
        ("toExponential", to_exponential, 1),
        ("toFixed", to_fixed, 1),
        ("toPrecision", to_precision, 1),
        ("toString", to_string_method, 1),
        ("valueOf", value_of, 0),
    ];
    for (name, code, len) in methods {
        rt.define_native_method(proto, name, *code, *len);
    }
    rt.install_global("Number", Value::Object(ctor));
}

/// The own data property `name` of the global object, when it holds a function.
fn global_function(rt: &Rt, name: &str) -> Option<Value> {
    let global = rt.intrinsic(Intrinsic::Global);
    match rt.get_own_property(global, &PropKey::from_str(name)) {
        Ok(Some(Prop::Data { value, .. })) if rt.is_callable(&value) => Some(value),
        _ => None,
    }
}

/// GetPrototypeFromConstructor (ECMA-262 10.1.14): `new_target.prototype` when it is an object, else the intrinsic.
fn proto_from(rt: &Rt, new_target: &Value, fallback: Intrinsic) -> Res<ObjRef> {
    match rt.get_str(new_target, "prototype")? {
        Value::Object(p) => Ok(p),
        _ => Ok(rt.intrinsic(fallback)),
    }
}

/// thisNumberValue (ECMA-262 21.1.3): a Number primitive or a Number wrapper.
fn this_number(rt: &Rt, c: &Call<'_>, name: &str) -> Res<f64> {
    let found = match c.this {
        Value::Number(n) => Some(*n),
        Value::Object(o) => rt.with_obj(*o, |ob| match &ob.kind {
            ObjectKind::Wrapper(Value::Number(n)) => Some(*n),
            _ => None,
        }),
        _ => None,
    };
    found.ok_or_else(|| rt.throw_type_error(&format!("Number.prototype.{name} requires that 'this' be a Number")))
}

fn number_call(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    if c.args.is_empty() {
        return Ok(Value::Number(0.0));
    }
    Ok(Value::Number(rt.to_number(&c.args[0])?))
}

fn number_construct(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let n = if c.args.is_empty() { 0.0 } else { rt.to_number(&c.args[0])? };
    let proto = proto_from(rt, c.new_target, Intrinsic::NumberPrototype)?;
    Ok(Value::Object(rt.alloc(Obj::new(Some(proto), ObjectKind::Wrapper(Value::Number(n))))))
}

fn parse_float_fallback(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = rt.to_string(&c.arg(0))?;
    Ok(Value::Number(number::parse_float(&s)))
}

fn parse_int_fallback(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = rt.to_string(&c.arg(0))?;
    let radix = rt.to_int32(&c.arg(1))?;
    Ok(Value::Number(number::parse_int(&s, radix)))
}

// ----- statics: these do not coerce their argument -----

fn is_finite(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(matches!(c.arg(0), Value::Number(n) if n.is_finite())))
}

fn is_integer(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(matches!(c.arg(0), Value::Number(n) if number::is_integer_valued(n))))
}

fn is_nan(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(matches!(c.arg(0), Value::Number(n) if n.is_nan())))
}

fn is_safe_integer(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(matches!(c.arg(0), Value::Number(n) if number::is_integer_valued(n) && n.abs() <= MAX_SAFE)))
}

// ----- prototype -----

/// toString(radix). `this` is checked before the radix is converted (V8 order).
fn to_string_method(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let x = this_number(rt, c, "toString")?;
    let radix = if c.arg(0).is_undefined() { 10.0 } else { rt.to_integer_or_infinity(&c.arg(0))? };
    match number::number_to_string_radix_js(x, radix) {
        Ok(s) => Ok(Value::String(JsStr::from_str(&s))),
        Err(_) => Err(rt.throw_range_error("toString() radix must be between 2 and 36")),
    }
}

fn to_fixed(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let x = this_number(rt, c, "toFixed")?;
    let digits = rt.to_integer_or_infinity(&c.arg(0))?;
    match number::to_fixed_js(x, digits) {
        Ok(s) => Ok(Value::String(JsStr::from_str(&s))),
        Err(_) => Err(rt.throw_range_error("toFixed() digits argument must be between 0 and 100")),
    }
}

fn to_exponential(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let x = this_number(rt, c, "toExponential")?;
    let digits = if c.arg(0).is_undefined() { None } else { Some(rt.to_integer_or_infinity(&c.arg(0))?) };
    match number::to_exponential_js(x, digits) {
        Ok(s) => Ok(Value::String(JsStr::from_str(&s))),
        Err(_) => Err(rt.throw_range_error("toExponential() argument must be between 0 and 100")),
    }
}

fn to_precision(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let x = this_number(rt, c, "toPrecision")?;
    let precision = if c.arg(0).is_undefined() { None } else { Some(rt.to_integer_or_infinity(&c.arg(0))?) };
    match number::to_precision_js(x, precision) {
        Ok(s) => Ok(Value::String(JsStr::from_str(&s))),
        Err(_) => Err(rt.throw_range_error("toPrecision() argument must be between 1 and 100")),
    }
}

fn value_of(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Number(this_number(rt, c, "valueOf")?))
}

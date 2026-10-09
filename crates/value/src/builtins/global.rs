//! Builtin family: global functions and value properties (WP-B1). Value properties undefined, NaN and Infinity are
//! non-writable, non-enumerable and non-configurable. parseInt, parseFloat, isNaN and isFinite are writable and
//! configurable, not enumerable. parseInt and parseFloat are registered as "parseInt" and "parseFloat" so that
//! Number.parseInt and Number.parseFloat can be the same function objects (WP-B3). globalThis is set by the realm
//! bootstrap (rt.rs). Other global functions (encodeURI, eval, ...) are absent.

use crate::function::Call;
use crate::number;
use crate::object::{Flags, Prop};
use crate::rt::{Intrinsic, Rt};
use crate::value::{PropKey, Res, Value};

fn global_parse_int(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = rt.to_string(&c.arg(0))?;
    let radix = rt.to_int32(&c.arg(1))?;
    Ok(Value::Number(number::parse_int(&s, radix)))
}

fn global_parse_float(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = rt.to_string(&c.arg(0))?;
    Ok(Value::Number(number::parse_float(&s)))
}

fn global_is_nan(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(rt.to_number(&c.arg(0))?.is_nan()))
}

fn global_is_finite(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(rt.to_number(&c.arg(0))?.is_finite()))
}

pub fn install(rt: &Rt) {
    let g = rt.intrinsic(Intrinsic::Global);
    rt.define_builtin(g, PropKey::from_str("undefined"), Prop::Data { value: Value::Undefined, flags: Flags::NONE });
    rt.define_builtin(g, PropKey::from_str("NaN"), Prop::Data { value: Value::Number(f64::NAN), flags: Flags::NONE });
    rt.define_builtin(
        g,
        PropKey::from_str("Infinity"),
        Prop::Data { value: Value::Number(f64::INFINITY), flags: Flags::NONE },
    );

    let parse_int = rt.new_native_function(global_parse_int, "parseInt", 2, Vec::new());
    let parse_float = rt.new_native_function(global_parse_float, "parseFloat", 1, Vec::new());
    rt.install_global("parseInt", Value::Object(parse_int));
    rt.install_global("parseFloat", Value::Object(parse_float));
    rt.register_builtin("parseInt", parse_int);
    rt.register_builtin("parseFloat", parse_float);
    rt.define_native_method(g, "isNaN", global_is_nan, 1);
    rt.define_native_method(g, "isFinite", global_is_finite, 1);
}

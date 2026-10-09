//! Builtin family: Math (ECMA-262 21.3). Math.random and Math.f16round are absent: the platform service provides
//! randomness, and f16round is not in this runtime. Transcendental functions use the platform libm, while V8 uses its
//! own ieee754 port, so a few results may differ in the last ulp (the O2 corpus marks them approx). Math.hypot
//! follows V8's MathHypot, which is bit-exact.

use crate::function::{Call, CodeFn};
use crate::number;
use crate::object::{Flags, Prop};
use crate::rt::{Rt, WellKnown};
use crate::value::{PropKey, Res, Value};

pub fn install(rt: &Rt) {
    let m = rt.new_object();
    for (name, code) in UNARY {
        rt.define_native_method(m, name, *code, 1);
    }
    let binary: &[(&str, CodeFn, u32)] = &[
        ("atan2", atan2, 2),
        ("hypot", hypot, 2),
        ("imul", imul, 2),
        ("max", max, 2),
        ("min", min, 2),
        ("pow", pow, 2),
    ];
    for (name, code, len) in binary {
        rt.define_native_method(m, name, *code, *len);
    }
    let consts: &[(&str, f64)] = &[
        ("E", std::f64::consts::E),
        ("LN10", std::f64::consts::LN_10),
        ("LN2", std::f64::consts::LN_2),
        ("LOG10E", std::f64::consts::LOG10_E),
        ("LOG2E", std::f64::consts::LOG2_E),
        ("PI", std::f64::consts::PI),
        ("SQRT1_2", std::f64::consts::FRAC_1_SQRT_2),
        ("SQRT2", std::f64::consts::SQRT_2),
    ];
    for (name, v) in consts {
        rt.define_builtin(m, PropKey::from_str(name), Prop::Data { value: Value::Number(*v), flags: Flags::NONE });
    }
    // Math[Symbol.toStringTag] is "Math": not writable, not enumerable, configurable (ECMA-262 21.3.1).
    let tag = PropKey::Sym(rt.well_known(WellKnown::ToStringTag));
    rt.define_builtin(m, tag, Prop::Data { value: Value::str("Math"), flags: Flags::CONFIG });
    rt.install_global("Math", Value::Object(m));
}

/// ToNumber of the argument, then `f`.
fn apply1(rt: &Rt, c: &Call<'_>, f: fn(f64) -> f64) -> Res<Value> {
    Ok(Value::Number(f(rt.to_number(&c.arg(0))?)))
}

/// Math.sign: NaN and the zeros are returned unchanged.
fn sign_of(x: f64) -> f64 {
    if x.is_nan() || x == 0.0 {
        x
    } else if x > 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// Math.hypot after ToNumber of every argument. Matches V8's MathHypot bit for bit: an infinity wins over a NaN, the
/// values are scaled by their largest magnitude, and the scaled squares are summed with Kahan compensation.
fn hypot_of(vals: &[f64]) -> f64 {
    if vals.iter().any(|v| v.is_infinite()) {
        return f64::INFINITY;
    }
    if vals.iter().any(|v| v.is_nan()) {
        return f64::NAN;
    }
    let max = vals.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    if max == 0.0 {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut compensation = 0.0;
    for v in vals {
        let n = v.abs() / max;
        let summand = n * n - compensation;
        let preliminary = sum + summand;
        compensation = (preliminary - sum) - summand;
        sum = preliminary;
    }
    sum.sqrt() * max
}

/// ToNumber of every argument, in order, before any of them is used.
fn to_numbers(rt: &Rt, c: &Call<'_>) -> Res<Vec<f64>> {
    c.args.iter().map(|a| rt.to_number(a)).collect()
}

macro_rules! unary {
    ($($js:literal => $name:ident = $f:expr;)*) => {
        $(
            fn $name(rt: &Rt, c: &Call<'_>) -> Res<Value> {
                apply1(rt, c, $f)
            }
        )*
        /// The one-argument functions, by JavaScript name.
        const UNARY: &[(&str, CodeFn)] = &[$(($js, $name as CodeFn)),*];
    };
}

unary! {
    "abs" => abs = f64::abs;
    "acos" => acos = f64::acos;
    "acosh" => acosh = f64::acosh;
    "asin" => asin = f64::asin;
    "asinh" => asinh = f64::asinh;
    "atan" => atan = f64::atan;
    "atanh" => atanh = f64::atanh;
    "cbrt" => cbrt = f64::cbrt;
    "ceil" => ceil = f64::ceil;
    "clz32" => clz32 = |x| f64::from(number::to_uint32(x).leading_zeros());
    "cos" => cos = f64::cos;
    "cosh" => cosh = f64::cosh;
    "exp" => exp = f64::exp;
    "expm1" => expm1 = f64::exp_m1;
    "floor" => floor = f64::floor;
    "fround" => fround = |x| f64::from(x as f32);
    "log" => log = f64::ln;
    "log10" => log10 = f64::log10;
    "log1p" => log1p = f64::ln_1p;
    "log2" => log2 = f64::log2;
    "round" => round = number::js_round;
    "sign" => sign = sign_of;
    "sin" => sin = f64::sin;
    "sinh" => sinh = f64::sinh;
    "sqrt" => sqrt = f64::sqrt;
    "tan" => tan = f64::tan;
    "tanh" => tanh = f64::tanh;
    "trunc" => trunc = f64::trunc;
}

/// Math.atan2(y, x): both arguments are converted, y first.
fn atan2(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let y = rt.to_number(&c.arg(0))?;
    let x = rt.to_number(&c.arg(1))?;
    Ok(Value::Number(y.atan2(x)))
}

/// Math.pow: the `**` operator's Number::exponentiate.
fn pow(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let base = rt.to_number(&c.arg(0))?;
    let exp = rt.to_number(&c.arg(1))?;
    Ok(Value::Number(number::js_pow(base, exp)))
}

/// Math.imul: the low 32 bits of the product of the two ToUint32 values, as a signed integer.
fn imul(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let a = number::to_uint32(rt.to_number(&c.arg(0))?);
    let b = number::to_uint32(rt.to_number(&c.arg(1))?);
    Ok(Value::Number(f64::from(a.wrapping_mul(b) as i32)))
}

fn hypot(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let vals = to_numbers(rt, c)?;
    Ok(Value::Number(hypot_of(&vals)))
}

fn max(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let vals = to_numbers(rt, c)?;
    Ok(Value::Number(number::js_max(&vals)))
}

fn min(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let vals = to_numbers(rt, c)?;
    Ok(Value::Number(number::js_min(&vals)))
}

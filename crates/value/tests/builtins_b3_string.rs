//! WP-B3: String builtins against V8. The tables are V8's results (generated) for String.prototype methods, for
//! String.fromCharCode, String.fromCodePoint and String(...). The hand-written tests cover what a corpus cannot carry:
//! wrapper objects, symbols, the order of argument conversions (objects that log), callbacks, the String iterator,
//! property attributes and collection between calls.
//! // Generated from Node v26.5.1 (V8). Each expectation is V8's result for the same call (see the scratch
//! // generator notes in the WP-B3 report). Edge cases come first in the sample; nothing here is computed by the code under test.

use std::cell::RefCell;

use vn_value::number::number_to_string;
use vn_value::{Call, Flags, Intrinsic, JsStr, PropKey, Res, Rt, Throw, Value, WellKnown};

#[derive(Clone, Copy, Debug)]
enum A {
    U,
    Nl,
    B(bool),
    N(f64),
    S(&'static [u16]),
}

fn to_value(a: A) -> Value {
    match a {
        A::U => Value::Undefined,
        A::Nl => Value::Null,
        A::B(b) => Value::Bool(b),
        A::N(n) => Value::Number(n),
        A::S(u) => Value::String(JsStr::from_units(u)),
    }
}

/// Printable ASCII as is; every other code unit (and " and \) as \uXXXX.
fn escape(units: &[u16]) -> String {
    let mut out = String::new();
    for &c in units {
        if (0x20..=0x7e).contains(&c) && c != 0x22 && c != 0x5c {
            out.push(char::from(c as u8));
        } else {
            out.push_str(&format!("\\u{c:04x}"));
        }
    }
    out
}

/// undefined, null, booleans, numbers (-0 included), quoted strings, arrays, and throw:<name> for an exception.
fn show(rt: &Rt, r: &Res<Value>) -> String {
    match r {
        Ok(v) => show_val(rt, v),
        Err(Throw(e)) => format!("throw:{}", error_name(rt, e)),
    }
}

fn show_val(rt: &Rt, v: &Value) -> String {
    match v {
        Value::Undefined => "undefined".to_string(),
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) if *n == 0.0 && n.is_sign_negative() => "-0".to_string(),
        Value::Number(n) => number_to_string(*n),
        Value::String(s) => format!("\"{}\"", escape(s.units())),
        Value::Object(_) if rt.is_array(v) => {
            let len = match rt.get_str(v, "length") {
                Ok(Value::Number(n)) => n as u32,
                _ => 0,
            };
            let items: Vec<String> = (0..len).map(|i| show(rt, &rt.get_v(v, &PropKey::Index(i)))).collect();
            format!("[{}]", items.join(","))
        }
        _ => "?".to_string(),
    }
}

fn error_name(rt: &Rt, e: &Value) -> String {
    match rt.get_str(e, "name") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => "?".to_string(),
    }
}

fn global(rt: &Rt, name: &str) -> Value {
    rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), name).expect("global binding")
}

fn string_method(rt: &Rt, name: &str) -> Value {
    let proto = rt.get_str(&global(rt, "String"), "prototype").expect("String.prototype");
    rt.get_str(&proto, name).expect("String.prototype method")
}

fn call_method(rt: &Rt, name: &str, this: &Value, args: &[Value]) -> Res<Value> {
    let f = string_method(rt, name);
    rt.call(&f, this, args)
}

fn call_static(rt: &Rt, name: &str, args: &[Value]) -> Res<Value> {
    let f = rt.get_str(&global(rt, "String"), name).expect("String static");
    rt.call(&f, &Value::Undefined, args)
}

// ----- generated tables (V8) -----

const METHOD_CASES: &[(A, &str, &[A], &str)] = &[
    (A::U, "at", &[], r##"throw:TypeError"##),
    (A::Nl, "at", &[], r##"throw:TypeError"##),
    (A::N(12.0), "at", &[A::N(0.0)], r##""1""##),
    (A::N(-0.0), "at", &[A::N(0.0)], r##""0""##),
    (A::B(true), "at", &[A::N(0.0)], r##""t""##),
    (A::N(1.5), "at", &[A::N(0.0)], r##""1""##),
    (A::U, "charAt", &[], r##"throw:TypeError"##),
    (A::Nl, "charAt", &[], r##"throw:TypeError"##),
    (A::N(12.0), "charAt", &[A::N(0.0)], r##""1""##),
    (A::N(-0.0), "charAt", &[A::N(0.0)], r##""0""##),
    (A::B(true), "charAt", &[A::N(0.0)], r##""t""##),
    (A::N(1.5), "charAt", &[A::N(0.0)], r##""1""##),
    (A::U, "charCodeAt", &[], r##"throw:TypeError"##),
    (A::Nl, "charCodeAt", &[], r##"throw:TypeError"##),
    (A::N(12.0), "charCodeAt", &[A::N(0.0)], r##"49"##),
    (A::N(-0.0), "charCodeAt", &[A::N(0.0)], r##"48"##),
    (A::B(true), "charCodeAt", &[A::N(0.0)], r##"116"##),
    (A::N(1.5), "charCodeAt", &[A::N(0.0)], r##"49"##),
    (A::U, "codePointAt", &[], r##"throw:TypeError"##),
    (A::Nl, "codePointAt", &[], r##"throw:TypeError"##),
    (A::N(12.0), "codePointAt", &[A::N(0.0)], r##"49"##),
    (A::N(-0.0), "codePointAt", &[A::N(0.0)], r##"48"##),
    (A::B(true), "codePointAt", &[A::N(0.0)], r##"116"##),
    (A::N(1.5), "codePointAt", &[A::N(0.0)], r##"49"##),
    (A::U, "concat", &[], r##"throw:TypeError"##),
    (A::Nl, "concat", &[], r##"throw:TypeError"##),
    (A::N(12.0), "concat", &[A::N(0.0)], r##""120""##),
    (A::N(-0.0), "concat", &[A::N(0.0)], r##""00""##),
    (A::B(true), "concat", &[A::N(0.0)], r##""true0""##),
    (A::N(1.5), "concat", &[A::N(0.0)], r##""1.50""##),
    (A::U, "isWellFormed", &[], r##"throw:TypeError"##),
    (A::Nl, "isWellFormed", &[], r##"throw:TypeError"##),
    (A::N(12.0), "isWellFormed", &[A::N(0.0)], r##"true"##),
    (A::N(-0.0), "isWellFormed", &[A::N(0.0)], r##"true"##),
    (A::B(true), "isWellFormed", &[A::N(0.0)], r##"true"##),
    (A::N(1.5), "isWellFormed", &[A::N(0.0)], r##"true"##),
    (A::U, "repeat", &[], r##"throw:TypeError"##),
    (A::Nl, "repeat", &[], r##"throw:TypeError"##),
    (A::N(12.0), "repeat", &[A::N(0.0)], r##""""##),
    (A::N(-0.0), "repeat", &[A::N(0.0)], r##""""##),
    (A::B(true), "repeat", &[A::N(0.0)], r##""""##),
    (A::N(1.5), "repeat", &[A::N(0.0)], r##""""##),
    (A::U, "slice", &[], r##"throw:TypeError"##),
    (A::Nl, "slice", &[], r##"throw:TypeError"##),
    (A::N(12.0), "slice", &[A::N(0.0)], r##""12""##),
    (A::N(-0.0), "slice", &[A::N(0.0)], r##""0""##),
    (A::B(true), "slice", &[A::N(0.0)], r##""true""##),
    (A::N(1.5), "slice", &[A::N(0.0)], r##""1.5""##),
    (A::U, "substr", &[], r##"throw:TypeError"##),
    (A::Nl, "substr", &[], r##"throw:TypeError"##),
    (A::N(12.0), "substr", &[A::N(0.0)], r##""12""##),
    (A::N(-0.0), "substr", &[A::N(0.0)], r##""0""##),
    (A::B(true), "substr", &[A::N(0.0)], r##""true""##),
    (A::N(1.5), "substr", &[A::N(0.0)], r##""1.5""##),
    (A::U, "substring", &[], r##"throw:TypeError"##),
    (A::Nl, "substring", &[], r##"throw:TypeError"##),
    (A::N(12.0), "substring", &[A::N(0.0)], r##""12""##),
    (A::N(-0.0), "substring", &[A::N(0.0)], r##""0""##),
    (A::B(true), "substring", &[A::N(0.0)], r##""true""##),
    (A::N(1.5), "substring", &[A::N(0.0)], r##""1.5""##),
    (A::U, "toLowerCase", &[], r##"throw:TypeError"##),
    (A::Nl, "toLowerCase", &[], r##"throw:TypeError"##),
    (A::N(12.0), "toLowerCase", &[A::N(0.0)], r##""12""##),
    (A::N(-0.0), "toLowerCase", &[A::N(0.0)], r##""0""##),
    (A::B(true), "toLowerCase", &[A::N(0.0)], r##""true""##),
    (A::N(1.5), "toLowerCase", &[A::N(0.0)], r##""1.5""##),
    (A::U, "toString", &[], r##"throw:TypeError"##),
    (A::Nl, "toString", &[], r##"throw:TypeError"##),
    (A::N(12.0), "toString", &[A::N(0.0)], r##"throw:TypeError"##),
    (A::N(-0.0), "toString", &[A::N(0.0)], r##"throw:TypeError"##),
    (A::B(true), "toString", &[A::N(0.0)], r##"throw:TypeError"##),
    (A::N(1.5), "toString", &[A::N(0.0)], r##"throw:TypeError"##),
    (A::U, "toUpperCase", &[], r##"throw:TypeError"##),
    (A::Nl, "toUpperCase", &[], r##"throw:TypeError"##),
    (A::N(12.0), "toUpperCase", &[A::N(0.0)], r##""12""##),
    (A::N(-0.0), "toUpperCase", &[A::N(0.0)], r##""0""##),
    (A::B(true), "toUpperCase", &[A::N(0.0)], r##""TRUE""##),
    (A::N(1.5), "toUpperCase", &[A::N(0.0)], r##""1.5""##),
    (A::U, "toWellFormed", &[], r##"throw:TypeError"##),
    (A::Nl, "toWellFormed", &[], r##"throw:TypeError"##),
    (A::N(12.0), "toWellFormed", &[A::N(0.0)], r##""12""##),
    (A::N(-0.0), "toWellFormed", &[A::N(0.0)], r##""0""##),
    (A::B(true), "toWellFormed", &[A::N(0.0)], r##""true""##),
    (A::N(1.5), "toWellFormed", &[A::N(0.0)], r##""1.5""##),
    (A::U, "trim", &[], r##"throw:TypeError"##),
    (A::Nl, "trim", &[], r##"throw:TypeError"##),
    (A::N(12.0), "trim", &[A::N(0.0)], r##""12""##),
    (A::N(-0.0), "trim", &[A::N(0.0)], r##""0""##),
    (A::B(true), "trim", &[A::N(0.0)], r##""true""##),
    (A::N(1.5), "trim", &[A::N(0.0)], r##""1.5""##),
    (A::U, "trimEnd", &[], r##"throw:TypeError"##),
    (A::Nl, "trimEnd", &[], r##"throw:TypeError"##),
    (A::N(12.0), "trimEnd", &[A::N(0.0)], r##""12""##),
    (A::N(-0.0), "trimEnd", &[A::N(0.0)], r##""0""##),
    (A::B(true), "trimEnd", &[A::N(0.0)], r##""true""##),
    (A::N(1.5), "trimEnd", &[A::N(0.0)], r##""1.5""##),
    (A::U, "trimStart", &[], r##"throw:TypeError"##),
    (A::Nl, "trimStart", &[], r##"throw:TypeError"##),
    (A::N(12.0), "trimStart", &[A::N(0.0)], r##""12""##),
    (A::N(-0.0), "trimStart", &[A::N(0.0)], r##""0""##),
    (A::B(true), "trimStart", &[A::N(0.0)], r##""true""##),
    (A::N(1.5), "trimStart", &[A::N(0.0)], r##""1.5""##),
    (A::U, "valueOf", &[], r##"throw:TypeError"##),
    (A::Nl, "valueOf", &[], r##"throw:TypeError"##),
    (A::N(12.0), "valueOf", &[A::N(0.0)], r##"throw:TypeError"##),
    (A::N(-0.0), "valueOf", &[A::N(0.0)], r##"throw:TypeError"##),
    (A::B(true), "valueOf", &[A::N(0.0)], r##"throw:TypeError"##),
    (A::N(1.5), "valueOf", &[A::N(0.0)], r##"throw:TypeError"##),
    (A::N(12.0), "toString", &[], r##"throw:TypeError"##),
    (A::N(12.0), "valueOf", &[], r##"throw:TypeError"##),
    (A::B(true), "toString", &[], r##"throw:TypeError"##),
    (A::B(true), "valueOf", &[], r##"throw:TypeError"##),
    (A::U, "toString", &[], r##"throw:TypeError"##),
    (A::U, "valueOf", &[], r##"throw:TypeError"##),
    (A::Nl, "toString", &[], r##"throw:TypeError"##),
    (A::Nl, "valueOf", &[], r##"throw:TypeError"##),
    (A::S(&[]), "at", &[A::B(false)], r##"undefined"##),
    (A::S(&[]), "at", &[A::N(2.0)], r##"undefined"##),
    (A::S(&[]), "codePointAt", &[A::N(3.0)], r##"undefined"##),
    (A::S(&[]), "codePointAt", &[A::N(-1.0)], r##"undefined"##),
    (A::S(&[]), "charAt", &[A::N(1.5)], r##""""##),
    (A::S(&[]), "at", &[A::N(f64::NAN)], r##"undefined"##),
    (A::S(&[]), "charAt", &[A::N(2147483648.0)], r##""""##),
    (A::S(&[]), "codePointAt", &[A::S(&[0x0032])], r##"undefined"##),
    (A::S(&[]), "codePointAt", &[A::S(&[0x0078])], r##"undefined"##),
    (A::S(&[]), "charAt", &[A::S(&[0x0020, 0x0031, 0x0020])], r##""""##),
    (A::S(&[]), "charCodeAt", &[A::N(4294967296.0)], r##"NaN"##),
    (A::S(&[0x0061]), "charAt", &[A::U], r##""a""##),
    (A::S(&[0x0061]), "at", &[A::Nl], r##""a""##),
    (A::S(&[0x0061]), "charAt", &[A::N(0.0)], r##""a""##),
    (A::S(&[0x0061]), "charAt", &[A::N(3.0)], r##""""##),
    (A::S(&[0x0061]), "codePointAt", &[A::N(-3.0)], r##"undefined"##),
    (A::S(&[0x0061]), "charCodeAt", &[A::N(1.5)], r##"NaN"##),
    (A::S(&[0x0061]), "codePointAt", &[A::N(1.5)], r##"undefined"##),
    (A::S(&[0x0061]), "at", &[A::N(2147483648.0)], r##"undefined"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "at", &[A::B(true)], r##""b""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "charCodeAt", &[A::B(true)], r##"98"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "charAt", &[A::N(-0.0)], r##""a""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "at", &[A::N(1.0)], r##""b""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "charAt", &[A::N(-3.0)], r##""""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "at", &[A::N(10.0)], r##"undefined"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "charAt", &[A::N(10.0)], r##""""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "at", &[A::S(&[0x0032])], r##""c""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "at", &[A::S(&[0x002d, 0x0031])], r##""c""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "codePointAt", &[A::N(4294967296.0)], r##"undefined"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "at", &[A::U], r##""a""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "charAt", &[A::U], r##""a""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "at", &[A::B(true)], r##"",""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "codePointAt", &[A::B(true)], r##"44"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "charCodeAt", &[A::N(1.0)], r##"44"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "charAt", &[A::N(-0.5)], r##""a""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "charCodeAt", &[A::N(-0.5)], r##"97"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "charCodeAt", &[A::N(f64::NAN)], r##"97"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "at", &[A::N(2147483648.0)], r##"undefined"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "codePointAt", &[A::S(&[0x0032])], r##"98"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "codePointAt", &[A::S(&[0x002d, 0x0031])], r##"undefined"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "codePointAt", &[A::S(&[0x0020, 0x0031, 0x0020])], r##"44"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "charCodeAt", &[A::N(4294967296.0)], r##"NaN"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "codePointAt", &[A::Nl], r##"97"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "at", &[A::B(false)], r##""a""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "charAt", &[A::N(-0.0)], r##""a""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "charCodeAt", &[A::N(3.0)], r##"56832"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "codePointAt", &[A::N(3.0)], r##"56832"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "charAt", &[A::N(-3.0)], r##""""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "charAt", &[A::N(1.5)], r##""b""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "codePointAt", &[A::N(-0.5)], r##"97"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "charAt", &[A::N(f64::NAN)], r##""a""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "charCodeAt", &[A::N(f64::NAN)], r##"97"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "codePointAt", &[A::N(f64::NAN)], r##"97"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "charAt", &[A::N(f64::INFINITY)], r##""""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "codePointAt", &[A::N(f64::NEG_INFINITY)], r##"undefined"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "codePointAt", &[A::S(&[0x0032])], r##"128512"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "charCodeAt", &[A::N(4294967296.0)], r##"NaN"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "charCodeAt", &[A::Nl], r##"55296"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "charCodeAt", &[A::B(true)], r##"120"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "charAt", &[A::B(false)], r##""\ud800""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "codePointAt", &[A::N(1.0)], r##"120"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "charAt", &[A::N(1.5)], r##""x""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "at", &[A::N(-0.5)], r##""\ud800""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "charAt", &[A::S(&[0x002d, 0x0031])], r##""""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "includes", &[A::S(&[]), A::U], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "lastIndexOf", &[A::S(&[]), A::N(0.0)], r##"0"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "startsWith", &[A::S(&[]), A::N(-1.0)], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "endsWith", &[A::S(&[]), A::N(5.0)], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "startsWith", &[A::S(&[]), A::N(f64::INFINITY)], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "lastIndexOf", &[A::S(&[]), A::S(&[0x0031])], r##"1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "includes", &[A::S(&[0x0062]), A::U], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "startsWith", &[A::S(&[0x0062]), A::N(2.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "indexOf", &[A::S(&[0x0062]), A::N(-1.0)], r##"1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "lastIndexOf", &[A::S(&[0x0062]), A::N(5.0)], r##"1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "endsWith", &[A::S(&[0x0062]), A::N(f64::NAN)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "includes", &[A::S(&[0x0062]), A::N(1.5)], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "includes", &[A::S(&[0x0062, 0x0063]), A::N(0.0)], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "endsWith", &[A::S(&[0x0062, 0x0063]), A::N(1.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "includes", &[A::S(&[0x0062, 0x0063]), A::N(2.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "lastIndexOf", &[A::S(&[0x0062, 0x0063]), A::N(2.0)], r##"1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "endsWith", &[A::S(&[0x0062, 0x0063]), A::N(-1.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "startsWith", &[A::S(&[0x0062, 0x0063]), A::N(5.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "lastIndexOf", &[A::S(&[0x0062, 0x0063]), A::N(f64::NAN)], r##"1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "indexOf", &[A::S(&[0x0062, 0x0063]), A::N(f64::INFINITY)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "endsWith", &[A::S(&[0x0061]), A::U], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "includes", &[A::S(&[0x0061]), A::N(0.0)], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "includes", &[A::S(&[0x0061]), A::N(-1.0)], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "includes", &[A::S(&[0x0061]), A::N(5.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "lastIndexOf", &[A::S(&[0x0061]), A::N(1.5)], r##"0"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "includes", &[A::S(&[0x0061, 0x0062, 0x0063]), A::U], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0x0063]), A::N(2.0)], r##"0"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0x0063]), A::N(f64::NAN)], r##"0"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(0.0)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "startsWith", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(1.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(2.0)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(f64::NEG_INFINITY)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "includes", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "endsWith", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(1.5)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "startsWith", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(1.5)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "lastIndexOf", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(2.0)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "includes", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(1.5)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "indexOf", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(1.5)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "startsWith", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(1.5)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "endsWith", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(1.5)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "startsWith", &[A::U, A::U], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "endsWith", &[A::U, A::N(1.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "indexOf", &[A::U, A::N(2.0)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "includes", &[A::U, A::N(f64::NAN)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "endsWith", &[A::U, A::N(f64::NAN)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "includes", &[A::U, A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "indexOf", &[A::U, A::N(f64::INFINITY)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "indexOf", &[A::U, A::N(1.5)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "indexOf", &[A::Nl, A::U], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "indexOf", &[A::Nl, A::N(5.0)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "lastIndexOf", &[A::Nl, A::N(5.0)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "startsWith", &[A::Nl, A::N(5.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "endsWith", &[A::N(1.0), A::N(2.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "indexOf", &[A::N(1.0), A::N(f64::INFINITY)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "startsWith", &[A::N(1.0), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "lastIndexOf", &[A::S(&[0x0063]), A::N(2.0)], r##"2"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "includes", &[A::S(&[0x0063]), A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "endsWith", &[A::S(&[0x0063]), A::N(f64::NEG_INFINITY)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "endsWith", &[A::S(&[0x0063]), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "endsWith", &[A::S(&[0xd800]), A::N(1.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "startsWith", &[A::S(&[0xd800]), A::N(5.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "endsWith", &[A::S(&[0xd800]), A::N(5.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "includes", &[A::S(&[0xd800]), A::N(f64::NAN)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "indexOf", &[A::S(&[0xd800]), A::N(f64::INFINITY)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "indexOf", &[A::S(&[0xd800]), A::N(f64::NEG_INFINITY)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "lastIndexOf", &[A::S(&[0xd800]), A::N(1.5)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "endsWith", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(2.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "indexOf", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(f64::NAN)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "includes", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(f64::NEG_INFINITY)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::S(&[]), A::N(-1.0)], r##"true"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::S(&[]), A::N(5.0)], r##"true"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "includes", &[A::S(&[]), A::N(f64::NEG_INFINITY)], r##"true"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "lastIndexOf", &[A::S(&[0x0062]), A::N(0.0)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::S(&[0x0062]), A::N(5.0)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::S(&[0x0062]), A::N(f64::NAN)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "includes", &[A::S(&[0x0062]), A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::S(&[0x0062]), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::S(&[0x0062]), A::N(1.5)], r##"2"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "includes", &[A::S(&[0x0062, 0x0063]), A::N(0.0)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::S(&[0x0062, 0x0063]), A::N(1.0)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "startsWith", &[A::S(&[0x0062, 0x0063]), A::N(1.0)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "includes", &[A::S(&[0x0062, 0x0063]), A::N(2.0)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::S(&[0x0062, 0x0063]), A::N(f64::NAN)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "startsWith", &[A::S(&[0x0062, 0x0063]), A::N(f64::NAN)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::S(&[0x0062, 0x0063]), A::N(f64::NAN)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::S(&[0x0062, 0x0063]), A::N(f64::NEG_INFINITY)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::S(&[0x0061]), A::N(0.0)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "lastIndexOf", &[A::S(&[0x0061]), A::N(2.0)], r##"0"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "lastIndexOf", &[A::S(&[0x0061]), A::N(-1.0)], r##"0"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "startsWith", &[A::S(&[0x0061]), A::N(5.0)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "includes", &[A::S(&[0x0061]), A::N(1.5)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::S(&[0x0061]), A::N(1.5)], r##"true"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "startsWith", &[A::S(&[0x0061, 0x0062, 0x0063]), A::N(2.0)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::S(&[0x0061, 0x0062, 0x0063]), A::N(f64::NAN)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::S(&[0x0061, 0x0062, 0x0063]), A::N(1.5)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "includes", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(f64::NAN)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(1.5)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(1.5)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(1.5)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::U], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "lastIndexOf", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(-1.0)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(f64::NAN)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(f64::INFINITY)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::U, A::N(1.0)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::U, A::N(2.0)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "startsWith", &[A::U, A::N(-1.0)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::U, A::N(5.0)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "includes", &[A::U, A::N(f64::NAN)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "includes", &[A::U, A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::U, A::N(f64::INFINITY)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "includes", &[A::U, A::S(&[0x0031])], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::U, A::S(&[0x0031])], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::U, A::N(1.5)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "lastIndexOf", &[A::Nl, A::N(1.0)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::Nl, A::N(5.0)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "lastIndexOf", &[A::Nl, A::N(5.0)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::Nl, A::N(f64::NAN)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::Nl, A::S(&[0x0031])], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::Nl, A::N(1.5)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "lastIndexOf", &[A::N(1.0), A::N(f64::NAN)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::N(1.0), A::N(f64::NEG_INFINITY)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "startsWith", &[A::N(1.0), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "startsWith", &[A::N(1.0), A::N(1.5)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::S(&[0x0063]), A::N(0.0)], r##"5"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "lastIndexOf", &[A::S(&[0x0063]), A::N(-1.0)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "lastIndexOf", &[A::S(&[0x0063]), A::N(5.0)], r##"5"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "lastIndexOf", &[A::S(&[0x0063]), A::N(f64::INFINITY)], r##"5"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "lastIndexOf", &[A::S(&[0x0063]), A::S(&[0x0031])], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "lastIndexOf", &[A::S(&[0xd800]), A::U], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "indexOf", &[A::S(&[0xd800]), A::N(0.0)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "startsWith", &[A::S(&[0xd800]), A::N(-1.0)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::S(&[0xd800]), A::N(5.0)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::S(&[0xd800]), A::N(f64::NEG_INFINITY)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "includes", &[A::S(&[0xd800]), A::N(1.5)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "includes", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(0.0)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(1.0)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(f64::NAN)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(f64::NAN)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "endsWith", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "includes", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(1.5)], r##"false"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(1.5)], r##"-1"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "startsWith", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(1.5)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "indexOf", &[A::S(&[]), A::N(0.0)], r##"0"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::S(&[]), A::N(1.0)], r##"1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "startsWith", &[A::S(&[]), A::N(1.0)], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "startsWith", &[A::S(&[]), A::N(5.0)], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "startsWith", &[A::S(&[]), A::N(f64::NAN)], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "indexOf", &[A::S(&[]), A::N(f64::INFINITY)], r##"6"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[]), A::N(f64::NEG_INFINITY)], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "indexOf", &[A::S(&[0x0062]), A::U], r##"1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "startsWith", &[A::S(&[0x0062]), A::U], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::S(&[0x0062]), A::N(0.0)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "includes", &[A::S(&[0x0062]), A::N(1.0)], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::S(&[0x0062]), A::N(2.0)], r##"1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::S(&[0x0062]), A::N(f64::NAN)], r##"1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "indexOf", &[A::S(&[0x0062]), A::N(f64::INFINITY)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "indexOf", &[A::S(&[0x0062]), A::N(f64::NEG_INFINITY)], r##"1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[0x0062]), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[0x0062]), A::N(1.5)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[0x0062, 0x0063]), A::N(0.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "indexOf", &[A::S(&[0x0062, 0x0063]), A::N(f64::NAN)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[0x0062, 0x0063]), A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "startsWith", &[A::S(&[0x0062, 0x0063]), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "startsWith", &[A::S(&[0x0062, 0x0063]), A::N(1.5)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "indexOf", &[A::S(&[0x0061]), A::N(1.0)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[0x0061]), A::N(1.0)], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "indexOf", &[A::S(&[0x0061]), A::N(-1.0)], r##"0"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "includes", &[A::S(&[0x0061]), A::N(5.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[0x0061]), A::N(f64::NAN)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "includes", &[A::S(&[0x0061]), A::N(f64::NEG_INFINITY)], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "includes", &[A::S(&[0x0061, 0x0062, 0x0063]), A::U], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0x0063]), A::N(5.0)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0x0063]), A::N(f64::NAN)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "includes", &[A::S(&[0x0061, 0x0062, 0x0063]), A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "includes", &[A::S(&[0x0061, 0x0062, 0x0063]), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0x0063]), A::S(&[0x0031])], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "startsWith", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::U], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::U], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "includes", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(5.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(5.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(f64::INFINITY)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(1.5)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::U], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "indexOf", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(-1.0)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "startsWith", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "startsWith", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(f64::NEG_INFINITY)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "includes", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0031])], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "includes", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(1.5)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "startsWith", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(1.5)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::U, A::U], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::U, A::N(2.0)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::U, A::N(5.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "includes", &[A::U, A::N(f64::NAN)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "indexOf", &[A::U, A::N(1.5)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "indexOf", &[A::Nl, A::U], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::Nl, A::N(-1.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::Nl, A::N(f64::NEG_INFINITY)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::Nl, A::S(&[0x0031])], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "includes", &[A::N(1.0), A::U], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::N(1.0), A::N(2.0)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "indexOf", &[A::N(1.0), A::N(-1.0)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "includes", &[A::N(1.0), A::N(5.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::N(1.0), A::N(5.0)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "startsWith", &[A::N(1.0), A::N(5.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::N(1.0), A::N(f64::NEG_INFINITY)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::N(1.0), A::S(&[0x0031])], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::S(&[0x0063]), A::U], r##"4"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[0x0063]), A::U], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[0x0063]), A::N(1.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "includes", &[A::S(&[0x0063]), A::N(2.0)], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::S(&[0x0063]), A::N(2.0)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "includes", &[A::S(&[0x0063]), A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "indexOf", &[A::S(&[0x0063]), A::N(1.5)], r##"4"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "startsWith", &[A::S(&[0xd800]), A::N(-1.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "indexOf", &[A::S(&[0xd800]), A::N(f64::NAN)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[0xd800]), A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "indexOf", &[A::S(&[0xd800]), A::N(1.5)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::U], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "includes", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(0.0)], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(1.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "includes", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(2.0)], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "indexOf", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(2.0)], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(2.0)], r##"0"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "indexOf", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::S(&[0x0031])], r##"-1"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "endsWith", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "includes", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(1.5)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::S(&[]), A::N(1.0)], r##"1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "lastIndexOf", &[A::S(&[]), A::N(-1.0)], r##"0"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "lastIndexOf", &[A::S(&[]), A::N(5.0)], r##"3"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "startsWith", &[A::S(&[]), A::N(1.5)], r##"true"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "lastIndexOf", &[A::S(&[0x0062]), A::N(1.0)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "startsWith", &[A::S(&[0x0062]), A::N(1.0)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "startsWith", &[A::S(&[0x0062]), A::N(f64::NAN)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "endsWith", &[A::S(&[0x0062]), A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "endsWith", &[A::S(&[0x0062]), A::N(f64::NEG_INFINITY)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::S(&[0x0062]), A::S(&[0x0031])], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "lastIndexOf", &[A::S(&[0x0062]), A::S(&[0x0031])], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "lastIndexOf", &[A::S(&[0x0062]), A::N(1.5)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "startsWith", &[A::S(&[0x0062]), A::N(1.5)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "endsWith", &[A::S(&[0x0062]), A::N(1.5)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "lastIndexOf", &[A::S(&[0x0062, 0x0063]), A::U], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "startsWith", &[A::S(&[0x0062, 0x0063]), A::N(0.0)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::S(&[0x0062, 0x0063]), A::N(f64::NAN)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "startsWith", &[A::S(&[0x0062, 0x0063]), A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::S(&[0x0062, 0x0063]), A::N(f64::NEG_INFINITY)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "lastIndexOf", &[A::S(&[0x0062, 0x0063]), A::N(f64::NEG_INFINITY)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "endsWith", &[A::S(&[0x0061]), A::N(5.0)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::S(&[0x0061]), A::N(f64::INFINITY)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "startsWith", &[A::S(&[0x0061]), A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "includes", &[A::S(&[0x0061]), A::N(1.5)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "startsWith", &[A::S(&[0x0061, 0x0062, 0x0063]), A::N(1.0)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "endsWith", &[A::S(&[0x0061, 0x0062, 0x0063]), A::N(-1.0)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "endsWith", &[A::S(&[0x0061, 0x0062, 0x0063]), A::N(5.0)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "includes", &[A::S(&[0x0061, 0x0062, 0x0063]), A::N(f64::NEG_INFINITY)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::S(&[0x0061, 0x0062, 0x0063]), A::N(f64::NEG_INFINITY)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::U], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(0.0)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "includes", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(f64::NAN)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "lastIndexOf", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(f64::NAN)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "includes", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "startsWith", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "includes", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::S(&[0x0061, 0x0062, 0x0063, 0x0064]), A::N(1.5)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "includes", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(0.0)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "startsWith", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(2.0)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "startsWith", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(f64::NAN)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "includes", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "endsWith", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::U, A::U], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::U, A::N(2.0)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "includes", &[A::U, A::N(-1.0)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "includes", &[A::U, A::N(5.0)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::U, A::N(5.0)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::U, A::S(&[0x0031])], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "lastIndexOf", &[A::U, A::S(&[0x0031])], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "endsWith", &[A::U, A::N(1.5)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "includes", &[A::Nl, A::U], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "startsWith", &[A::Nl, A::N(5.0)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "startsWith", &[A::Nl, A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::Nl, A::S(&[0x0031])], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "endsWith", &[A::N(1.0), A::N(2.0)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "endsWith", &[A::N(1.0), A::N(f64::NAN)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "includes", &[A::N(1.0), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::N(1.0), A::S(&[0x0031])], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::N(1.0), A::N(1.5)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "lastIndexOf", &[A::S(&[0x0063]), A::N(0.0)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::S(&[0x0063]), A::N(2.0)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::S(&[0x0063]), A::N(-1.0)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "includes", &[A::S(&[0x0063]), A::N(5.0)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "lastIndexOf", &[A::S(&[0x0063]), A::N(f64::INFINITY)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "endsWith", &[A::S(&[0x0063]), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "includes", &[A::S(&[0xd800]), A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "startsWith", &[A::S(&[0xd800]), A::N(f64::NEG_INFINITY)], r##"true"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "includes", &[A::S(&[0xd800]), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "includes", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::U], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "endsWith", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::U], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "endsWith", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(2.0)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(-1.0)], r##"-1"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "startsWith", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(f64::NAN)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "endsWith", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(f64::INFINITY)], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "startsWith", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::S(&[0x0031])], r##"false"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "indexOf", &[A::S(&[0x0061, 0x0062, 0xd83d, 0xde00]), A::N(1.5)], r##"-1"##),
    (A::S(&[]), "substring", &[A::N(1.0), A::N(2.0)], r##""""##),
    (A::S(&[]), "slice", &[A::N(-2.0)], r##""""##),
    (A::S(&[]), "substr", &[A::N(-2.0)], r##""""##),
    (A::S(&[]), "substring", &[A::N(-3.0), A::N(-1.0)], r##""""##),
    (A::S(&[]), "substr", &[A::N(f64::NAN), A::N(3.0)], r##""""##),
    (A::S(&[]), "slice", &[A::N(1.0), A::N(f64::INFINITY)], r##""""##),
    (A::S(&[]), "substring", &[A::N(f64::NEG_INFINITY), A::N(2.0)], r##""""##),
    (A::S(&[]), "slice", &[A::N(0.0), A::N(0.0)], r##""""##),
    (A::S(&[0x0061]), "substring", &[A::N(0.0)], r##""a""##),
    (A::S(&[0x0061]), "slice", &[A::N(1.0), A::N(2.0)], r##""""##),
    (A::S(&[0x0061]), "substr", &[A::N(1.9), A::N(3.9)], r##""""##),
    (A::S(&[0x0061]), "slice", &[A::N(5.0), A::N(2.0)], r##""""##),
    (A::S(&[0x0061]), "slice", &[A::S(&[0x0078])], r##""a""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "substr", &[A::N(0.0)], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "slice", &[A::N(-2.0)], r##""bc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "substring", &[A::N(-2.0)], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "slice", &[A::N(-3.0), A::N(-1.0)], r##""ab""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "substring", &[A::N(f64::NEG_INFINITY), A::N(2.0)], r##""ab""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "substring", &[A::Nl, A::S(&[0x0032])], r##""ab""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "slice", &[A::N(1.9), A::N(3.9)], r##""bc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "substring", &[A::N(0.0), A::N(0.0)], r##""""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "substr", &[A::N(5.0), A::N(2.0)], r##""""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "substring", &[A::S(&[0x0078])], r##""abc""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "slice", &[A::N(2.0), A::N(1.0)], r##""""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "substr", &[A::N(2.0), A::N(1.0)], r##""b""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "substring", &[A::N(1.0), A::N(f64::INFINITY)], r##"",b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "substring", &[A::N(1.9), A::N(3.9)], r##"",b""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "substr", &[A::N(5.0), A::N(2.0)], r##""c""##),
    (A::S(&[0x0061, 0x0058, 0x0062, 0x0058, 0x0063]), "slice", &[], r##""aXbXc""##),
    (A::S(&[0x0061, 0x0058, 0x0062, 0x0058, 0x0063]), "slice", &[], r##""aXbXc""##),
    (A::S(&[0x0061, 0x0058, 0x0062, 0x0058, 0x0063]), "substr", &[], r##""aXbXc""##),
    (A::S(&[0x0061, 0x0058, 0x0062, 0x0058, 0x0063]), "slice", &[A::N(2.0), A::N(1.0)], r##""""##),
    (A::S(&[0x0061, 0x0058, 0x0062, 0x0058, 0x0063]), "substr", &[A::N(2.0), A::N(1.0)], r##""b""##),
    (A::S(&[0x0061, 0x0058, 0x0062, 0x0058, 0x0063]), "substring", &[A::N(-3.0), A::N(-1.0)], r##""""##),
    (A::S(&[0x0061, 0x0058, 0x0062, 0x0058, 0x0063]), "substring", &[A::N(f64::NAN), A::N(3.0)], r##""aXb""##),
    (A::S(&[0x0061, 0x0058, 0x0062, 0x0058, 0x0063]), "substr", &[A::N(3.0), A::N(-1.0)], r##""""##),
    (A::S(&[0x0061, 0x0058, 0x0062, 0x0058, 0x0063]), "slice", &[A::N(0.0), A::N(0.0)], r##""""##),
    (A::S(&[0x0061, 0x0058, 0x0062, 0x0058, 0x0063]), "substring", &[A::N(0.0), A::N(0.0)], r##""""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "slice", &[], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "slice", &[A::N(2.0), A::N(1.0)], r##""""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "substring", &[A::N(2.0), A::N(1.0)], r##""b""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "substr", &[A::N(2.0), A::N(1.0)], r##""\ud83d""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "substr", &[A::N(-2.0)], r##""cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "substr", &[A::N(1.0), A::N(f64::INFINITY)], r##""b\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "slice", &[A::Nl, A::S(&[0x0032])], r##""ab""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "substring", &[A::N(1.9), A::N(3.9)], r##""b\ud83d""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "substring", &[A::N(3.0), A::N(-1.0)], r##""ab\ud83d""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "substr", &[A::N(0.0), A::N(0.0)], r##""""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "substring", &[], r##""\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "substr", &[A::N(f64::NAN), A::N(3.0)], r##""\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "slice", &[A::N(1.0), A::N(f64::INFINITY)], r##""x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "slice", &[A::Nl, A::S(&[0x0032])], r##""\ud800x""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "substring", &[A::N(0.0), A::N(0.0)], r##""""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "substr", &[A::S(&[0x0078])], r##""\ud800x\udc00""##),
    (A::S(&[0x0020, 0x0020, 0x0009, 0x0020, 0x0078, 0x0020, 0x0079, 0x0020, 0x000a]), "slice", &[], r##""  \u0009 x y \u000a""##),
    (A::S(&[0x0020, 0x0020, 0x0009, 0x0020, 0x0078, 0x0020, 0x0079, 0x0020, 0x000a]), "substr", &[A::N(1.0), A::N(2.0)], r##"" \u0009""##),
    (A::S(&[0x0020, 0x0020, 0x0009, 0x0020, 0x0078, 0x0020, 0x0079, 0x0020, 0x000a]), "slice", &[A::N(2.0), A::N(1.0)], r##""""##),
    (A::S(&[0x0020, 0x0020, 0x0009, 0x0020, 0x0078, 0x0020, 0x0079, 0x0020, 0x000a]), "slice", &[A::N(-2.0)], r##"" \u000a""##),
    (A::S(&[0x0020, 0x0020, 0x0009, 0x0020, 0x0078, 0x0020, 0x0079, 0x0020, 0x000a]), "substring", &[A::N(f64::NAN), A::N(3.0)], r##""  \u0009""##),
    (A::S(&[0x0020, 0x0020, 0x0009, 0x0020, 0x0078, 0x0020, 0x0079, 0x0020, 0x000a]), "substring", &[A::N(1.0), A::N(f64::INFINITY)], r##"" \u0009 x y \u000a""##),
    (A::S(&[0x0020, 0x0020, 0x0009, 0x0020, 0x0078, 0x0020, 0x0079, 0x0020, 0x000a]), "substring", &[A::Nl, A::S(&[0x0032])], r##""  ""##),
    (A::S(&[0x00df, 0x0041, 0x0391, 0x03a3]), "slice", &[], r##""\u00dfA\u0391\u03a3""##),
    (A::S(&[0x00df, 0x0041, 0x0391, 0x03a3]), "slice", &[A::N(2.0), A::N(1.0)], r##""""##),
    (A::S(&[0x00df, 0x0041, 0x0391, 0x03a3]), "slice", &[A::N(f64::NAN), A::N(3.0)], r##""\u00dfA\u0391""##),
    (A::S(&[0x00df, 0x0041, 0x0391, 0x03a3]), "substr", &[A::N(f64::NAN), A::N(3.0)], r##""\u00dfA\u0391""##),
    (A::S(&[0x00df, 0x0041, 0x0391, 0x03a3]), "slice", &[A::N(5.0), A::N(2.0)], r##""""##),
    (A::S(&[0x0041, 0x0042, 0x0043, 0x0064, 0x0065, 0x0066]), "substr", &[], r##""ABCdef""##),
    (A::S(&[0x0041, 0x0042, 0x0043, 0x0064, 0x0065, 0x0066]), "substring", &[A::N(2.0), A::N(1.0)], r##""B""##),
    (A::S(&[0x0041, 0x0042, 0x0043, 0x0064, 0x0065, 0x0066]), "slice", &[A::N(1.9), A::N(3.9)], r##""BC""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "substring", &[], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "substr", &[A::N(0.0)], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "substring", &[A::N(-2.0)], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "substring", &[A::N(f64::NAN), A::N(3.0)], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "slice", &[A::N(1.9), A::N(3.9)], r##""$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "substr", &[A::N(3.0), A::N(-1.0)], r##""""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "substring", &[A::N(0.0), A::N(0.0)], r##""""##),
    (A::S(&[]), "padStart", &[], r##""""##),
    (A::S(&[]), "padStart", &[], r##""""##),
    (A::S(&[]), "padStart", &[], r##""""##),
    (A::S(&[]), "padEnd", &[], r##""""##),
    (A::S(&[]), "padEnd", &[A::N(0.0)], r##""""##),
    (A::S(&[]), "padStart", &[A::N(0.0), A::Nl], r##""""##),
    (A::S(&[]), "padEnd", &[A::N(0.0), A::Nl], r##""""##),
    (A::S(&[]), "padEnd", &[A::N(1.0)], r##"" ""##),
    (A::S(&[]), "padStart", &[A::N(1.0), A::S(&[0x0078])], r##""x""##),
    (A::S(&[]), "padEnd", &[A::N(2.0)], r##""  ""##),
    (A::S(&[]), "padEnd", &[A::N(2.0), A::N(0.0)], r##""00""##),
    (A::S(&[]), "padEnd", &[A::N(3.0), A::S(&[0x0078])], r##""xxx""##),
    (A::S(&[]), "padEnd", &[A::N(3.0), A::N(0.0)], r##""000""##),
    (A::S(&[]), "padEnd", &[A::N(5.0), A::Nl], r##""nulln""##),
    (A::S(&[]), "padStart", &[A::N(10.0), A::N(0.0)], r##""0000000000""##),
    (A::S(&[]), "padEnd", &[A::N(-1.0)], r##""""##),
    (A::S(&[]), "padStart", &[A::N(-1.0), A::N(0.0)], r##""""##),
    (A::S(&[]), "padEnd", &[A::N(f64::NAN)], r##""""##),
    (A::S(&[]), "padStart", &[A::N(f64::NAN), A::S(&[])], r##""""##),
    (A::S(&[]), "padStart", &[A::N(f64::NAN), A::S(&[0x0078, 0x0079])], r##""""##),
    (A::S(&[]), "padEnd", &[A::N(f64::NAN), A::S(&[0x0061, 0x0062, 0x0063])], r##""""##),
    (A::S(&[]), "padStart", &[A::S(&[0x0037]), A::N(0.0)], r##""0000000""##),
    (A::S(&[]), "padStart", &[A::N(2.7), A::N(0.0)], r##""00""##),
    (A::S(&[]), "padEnd", &[A::N(f64::INFINITY)], r##"throw:RangeError"##),
    (A::S(&[]), "padStart", &[A::N(f64::INFINITY), A::S(&[0x0078])], r##"throw:RangeError"##),
    (A::S(&[]), "padEnd", &[A::N(f64::INFINITY), A::S(&[0x0078])], r##"throw:RangeError"##),
    (A::S(&[]), "padEnd", &[A::N(f64::INFINITY), A::S(&[0x0078, 0x0079])], r##"throw:RangeError"##),
    (A::S(&[]), "padEnd", &[A::N(f64::INFINITY), A::S(&[0x0061, 0x0062, 0x0063])], r##"throw:RangeError"##),
    (A::S(&[]), "padEnd", &[A::N(f64::INFINITY), A::S(&[0xd800])], r##"throw:RangeError"##),
    (A::S(&[0x0061]), "padStart", &[], r##""a""##),
    (A::S(&[0x0061]), "padEnd", &[A::N(1.0), A::S(&[])], r##""a""##),
    (A::S(&[0x0061]), "padEnd", &[A::N(1.0), A::S(&[0x0078])], r##""a""##),
    (A::S(&[0x0061]), "padStart", &[A::N(1.0), A::S(&[0x0078, 0x0079])], r##""a""##),
    (A::S(&[0x0061]), "padEnd", &[A::N(1.0), A::S(&[0x0078, 0x0079])], r##""a""##),
    (A::S(&[0x0061]), "padStart", &[A::N(2.0)], r##"" a""##),
    (A::S(&[0x0061]), "padEnd", &[A::N(2.0), A::S(&[0x0078])], r##""ax""##),
    (A::S(&[0x0061]), "padStart", &[A::N(3.0), A::S(&[])], r##""a""##),
    (A::S(&[0x0061]), "padEnd", &[A::N(3.0), A::N(0.0)], r##""a00""##),
    (A::S(&[0x0061]), "padStart", &[A::N(5.0), A::S(&[])], r##""a""##),
    (A::S(&[0x0061]), "padEnd", &[A::N(10.0), A::N(0.0)], r##""a000000000""##),
    (A::S(&[0x0061]), "padEnd", &[A::N(-1.0), A::S(&[])], r##""a""##),
    (A::S(&[0x0061]), "padEnd", &[A::N(f64::NAN), A::S(&[0xd800])], r##""a""##),
    (A::S(&[0x0061]), "padEnd", &[A::S(&[0x0037]), A::S(&[0x0078])], r##""axxxxxx""##),
    (A::S(&[0x0061]), "padEnd", &[A::N(2.7), A::S(&[0x0078, 0x0079])], r##""ax""##),
    (A::S(&[0x0061]), "padStart", &[A::N(f64::INFINITY), A::Nl], r##"throw:RangeError"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padStart", &[], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padEnd", &[], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padStart", &[A::N(1.0), A::S(&[0x0061, 0x0062, 0x0063])], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padEnd", &[A::N(1.0), A::N(0.0)], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padStart", &[A::N(1.0), A::Nl], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padStart", &[A::N(2.0), A::S(&[])], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padStart", &[A::N(2.0), A::S(&[0xd800])], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padEnd", &[A::N(2.0), A::Nl], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padEnd", &[A::N(3.0), A::S(&[0x0078])], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padEnd", &[A::N(3.0), A::S(&[0x0078, 0x0079])], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padEnd", &[A::N(3.0), A::S(&[0x0061, 0x0062, 0x0063])], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padEnd", &[A::N(5.0)], r##""abc  ""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padEnd", &[A::N(5.0), A::S(&[])], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padStart", &[A::N(5.0), A::S(&[0x0061, 0x0062, 0x0063])], r##""ababc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padEnd", &[A::N(5.0), A::S(&[0xd800])], r##""abc\ud800\ud800""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padStart", &[A::N(5.0), A::N(0.0)], r##""00abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padStart", &[A::N(10.0)], r##""       abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padEnd", &[A::N(10.0), A::S(&[0x0061, 0x0062, 0x0063])], r##""abcabcabca""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padStart", &[A::N(10.0), A::S(&[0xd800])], r##""\ud800\ud800\ud800\ud800\ud800\ud800\ud800abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padEnd", &[A::N(-1.0), A::S(&[0x0078])], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padStart", &[A::N(-1.0), A::S(&[0xd800])], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padEnd", &[A::N(f64::NAN), A::S(&[0x0061, 0x0062, 0x0063])], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padEnd", &[A::N(f64::NAN), A::Nl], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padEnd", &[A::S(&[0x0037]), A::S(&[])], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padStart", &[A::S(&[0x0037]), A::S(&[0x0078])], r##""xxxxabc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padStart", &[A::S(&[0x0037]), A::S(&[0x0078, 0x0079])], r##""xyxyabc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padStart", &[A::S(&[0x0037]), A::S(&[0x0061, 0x0062, 0x0063])], r##""abcaabc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padStart", &[A::S(&[0x0037]), A::S(&[0xd800])], r##""\ud800\ud800\ud800\ud800abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padEnd", &[A::S(&[0x0037]), A::S(&[0xd800])], r##""abc\ud800\ud800\ud800\ud800""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padStart", &[A::N(2.7), A::S(&[0x0078])], r##""abc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "padEnd", &[A::N(f64::INFINITY), A::S(&[0x0061, 0x0062, 0x0063])], r##"throw:RangeError"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padEnd", &[], r##""a,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padEnd", &[], r##""a,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padStart", &[A::N(0.0), A::N(0.0)], r##""a,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padEnd", &[A::N(1.0), A::S(&[0x0078])], r##""a,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padStart", &[A::N(3.0)], r##""a,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padStart", &[A::N(3.0), A::S(&[0x0078, 0x0079])], r##""a,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padStart", &[A::N(5.0), A::S(&[0x0061, 0x0062, 0x0063])], r##""a,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padEnd", &[A::N(10.0), A::S(&[0x0078, 0x0079])], r##""a,b,,cxyxy""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padStart", &[A::N(10.0), A::N(0.0)], r##""0000a,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padEnd", &[A::N(-1.0), A::S(&[0x0078, 0x0079])], r##""a,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padEnd", &[A::N(-1.0), A::S(&[0x0061, 0x0062, 0x0063])], r##""a,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padEnd", &[A::N(f64::NAN)], r##""a,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padEnd", &[A::N(f64::NAN), A::S(&[0x0078, 0x0079])], r##""a,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padStart", &[A::N(f64::NAN), A::N(0.0)], r##""a,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padStart", &[A::S(&[0x0037]), A::N(0.0)], r##""0a,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padEnd", &[A::N(2.7), A::S(&[0x0078, 0x0079])], r##""a,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padEnd", &[A::N(2.7), A::S(&[0x0061, 0x0062, 0x0063])], r##""a,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padStart", &[A::N(f64::INFINITY), A::S(&[0x0078, 0x0079])], r##"throw:RangeError"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padStart", &[A::N(f64::INFINITY), A::S(&[0xd800])], r##"throw:RangeError"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "padStart", &[A::N(f64::INFINITY), A::N(0.0)], r##"throw:RangeError"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padStart", &[], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padEnd", &[], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padStart", &[A::N(1.0), A::S(&[])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padEnd", &[A::N(1.0), A::S(&[0x0061, 0x0062, 0x0063])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padEnd", &[A::N(3.0), A::S(&[])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padStart", &[A::N(3.0), A::S(&[0x0078, 0x0079])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padEnd", &[A::N(5.0)], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padEnd", &[A::N(5.0), A::S(&[0x0078, 0x0079])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padEnd", &[A::N(5.0), A::S(&[0x0061, 0x0062, 0x0063])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padEnd", &[A::N(5.0), A::S(&[0xd800])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padStart", &[A::N(10.0), A::S(&[0x0078, 0x0079])], r##""xyxyab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padEnd", &[A::N(10.0), A::S(&[0xd800])], r##""ab\ud83d\ude00cd\ud800\ud800\ud800\ud800""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padEnd", &[A::N(10.0), A::N(0.0)], r##""ab\ud83d\ude00cd0000""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padStart", &[A::N(-1.0), A::S(&[0xd800])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padEnd", &[A::N(f64::NAN)], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padEnd", &[A::N(f64::NAN), A::S(&[0x0078])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padStart", &[A::N(2.7)], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padEnd", &[A::N(f64::INFINITY), A::S(&[])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padEnd", &[A::N(f64::INFINITY), A::S(&[0x0078, 0x0079])], r##"throw:RangeError"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padEnd", &[A::N(f64::INFINITY), A::S(&[0x0061, 0x0062, 0x0063])], r##"throw:RangeError"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "padEnd", &[A::N(f64::INFINITY), A::S(&[0xd800])], r##"throw:RangeError"##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padStart", &[], r##""\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padStart", &[], r##""\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padStart", &[A::N(0.0), A::N(0.0)], r##""\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padStart", &[A::N(0.0), A::Nl], r##""\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padEnd", &[A::N(0.0), A::Nl], r##""\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padStart", &[A::N(1.0), A::S(&[0x0078, 0x0079])], r##""\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padEnd", &[A::N(1.0), A::Nl], r##""\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padStart", &[A::N(2.0)], r##""\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padStart", &[A::N(2.0), A::S(&[0x0061, 0x0062, 0x0063])], r##""\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padEnd", &[A::N(2.0), A::S(&[0x0061, 0x0062, 0x0063])], r##""\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padStart", &[A::N(3.0), A::Nl], r##""\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padStart", &[A::N(5.0), A::S(&[0xd800])], r##""\ud800\ud800\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padEnd", &[A::N(10.0), A::S(&[0x0078])], r##""\ud800x\udc00xxxxxxx""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padEnd", &[A::N(10.0), A::S(&[0x0078, 0x0079])], r##""\ud800x\udc00xyxyxyx""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padStart", &[A::N(-1.0), A::S(&[])], r##""\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padEnd", &[A::N(f64::NAN)], r##""\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padEnd", &[A::N(f64::NAN), A::S(&[0x0061, 0x0062, 0x0063])], r##""\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padEnd", &[A::N(f64::NAN), A::Nl], r##""\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padStart", &[A::S(&[0x0037])], r##""    \ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padEnd", &[A::S(&[0x0037]), A::N(0.0)], r##""\ud800x\udc000000""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "padStart", &[A::N(f64::INFINITY), A::S(&[0x0061, 0x0062, 0x0063])], r##"throw:RangeError"##),
    (A::S(&[]), "repeat", &[A::N(3.0)], r##""""##),
    (A::S(&[]), "repeat", &[A::N(0.5)], r##""""##),
    (A::S(&[]), "repeat", &[A::S(&[0x0032])], r##""""##),
    (A::S(&[0x0061]), "repeat", &[A::N(0.5)], r##""""##),
    (A::S(&[0x0061]), "repeat", &[A::N(536870912.0)], r##"throw:RangeError"##),
    (A::S(&[0x0061]), "repeat", &[A::B(true)], r##""a""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "repeat", &[A::N(3.0)], r##""abcabcabc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "repeat", &[A::N(0.5)], r##""""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "repeat", &[A::S(&[0x0032])], r##""abcabc""##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "repeat", &[A::N(9007199254740992.0)], r##"throw:RangeError"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "repeat", &[A::N(3.0)], r##""a,b,,ca,b,,ca,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "repeat", &[A::N(1.9)], r##""a,b,,c""##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "repeat", &[A::N(536870912.0)], r##"throw:RangeError"##),
    (A::S(&[0x0061, 0x002c, 0x0062, 0x002c, 0x002c, 0x0063]), "repeat", &[A::N(9007199254740992.0)], r##"throw:RangeError"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "repeat", &[], r##""""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "repeat", &[A::N(f64::NEG_INFINITY)], r##"throw:RangeError"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "repeat", &[A::B(true)], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "repeat", &[A::N(3.0)], r##""\ud800x\udc00\ud800x\udc00\ud800x\udc00""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "repeat", &[A::B(true)], r##""\ud800x\udc00""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replaceAll", &[A::S(&[]), A::S(&[0x0024, 0x0060])], r##""aababcabcbabcbbabcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replaceAll", &[A::S(&[]), A::S(&[0x0024, 0x0027])], r##""abcbbabcbbbcbbcbbbbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::S(&[0x0062]), A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""a$<a>cbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replaceAll", &[A::S(&[0x0062]), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""a[bb]c[bb][bb]""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::S(&[0x0062, 0x0062]), A::S(&[0x0058])], r##""abcX""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::S(&[0x0062, 0x0062]), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""abc[bbbb]""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::S(&[0x0062, 0x0062]), A::S(&[0x0059, 0x0024])], r##""abcY$""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replaceAll", &[A::S(&[0x0061]), A::S(&[0x0024, 0x0024])], r##""$bcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::S(&[0x0078]), A::S(&[])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::S(&[0x0078]), A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::S(&[0x0078]), A::S(&[0x0024])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replaceAll", &[A::S(&[0x0078]), A::S(&[0x0061, 0x0024])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::U, A::S(&[])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::U, A::S(&[0x0024, 0x0060])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replaceAll", &[A::U, A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replaceAll", &[A::U, A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replaceAll", &[A::U, A::S(&[0x0059, 0x0024])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replaceAll", &[A::Nl, A::S(&[0x0024, 0x0031])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::Nl, A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replaceAll", &[A::N(1.0), A::S(&[0x0024, 0x0031])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replaceAll", &[A::N(1.0), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::N(1.0), A::S(&[0x0024])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0024, 0x0060])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::S(&[0x0061, 0x0062]), A::S(&[])], r##""cbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::S(&[0x0061, 0x0062]), A::S(&[0x0024, 0x0026])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::S(&[0x0061, 0x0062]), A::S(&[0x0024, 0x0024])], r##""$cbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replaceAll", &[A::S(&[0x0061, 0x0062]), A::S(&[0x0024, 0x0060])], r##""cbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::S(&[0xd800]), A::S(&[0x0024, 0x0026])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replaceAll", &[A::S(&[0xd800]), A::S(&[0x0024, 0x0026])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::S(&[0xd800]), A::S(&[0x0024, 0x0060])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replaceAll", &[A::S(&[0xd800]), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::S(&[0xd800]), A::S(&[0x0024])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "replace", &[A::S(&[0xd800]), A::S(&[0x0059, 0x0024])], r##""abcbb""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::S(&[]), A::S(&[0x0058])], r##""Xaaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::S(&[]), A::S(&[0x0024, 0x0024])], r##""$aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replaceAll", &[A::S(&[]), A::S(&[0x0024, 0x0027])], r##""aaaaaaaaaaaaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::S(&[]), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""[]aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replaceAll", &[A::S(&[]), A::S(&[0x0061, 0x0024])], r##""a$aa$aa$aa$aa$""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::S(&[0x0062]), A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::S(&[0x0062]), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::S(&[0x0062]), A::S(&[0x0061, 0x0024])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::S(&[0x0062, 0x0062]), A::S(&[0x0058])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replaceAll", &[A::S(&[0x0062, 0x0062]), A::S(&[0x0024, 0x0024])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::S(&[0x0062, 0x0062]), A::S(&[0x0059, 0x0024])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replaceAll", &[A::S(&[0x0061]), A::S(&[])], r##""""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::S(&[0x0061]), A::S(&[0x0024, 0x0060])], r##""aaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replaceAll", &[A::S(&[0x0078]), A::S(&[0x0024, 0x0031])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replaceAll", &[A::U, A::S(&[])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replaceAll", &[A::U, A::S(&[0x0024, 0x0060])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::U, A::S(&[0x0024, 0x0027])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::U, A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::U, A::S(&[0x0059, 0x0024])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replaceAll", &[A::Nl, A::S(&[0x0024, 0x0026])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::Nl, A::S(&[0x0024, 0x0024])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replaceAll", &[A::Nl, A::S(&[0x0061, 0x0024])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replaceAll", &[A::N(1.0), A::S(&[0x0024, 0x0024])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replaceAll", &[A::N(1.0), A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replaceAll", &[A::N(1.0), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replaceAll", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0024, 0x0060])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0024, 0x0027])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::S(&[0x0061, 0x0062]), A::S(&[0x0058])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replaceAll", &[A::S(&[0x0061, 0x0062]), A::S(&[0x0024, 0x0027])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::S(&[0x0061, 0x0062]), A::S(&[0x0061, 0x0024])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replaceAll", &[A::S(&[0xd800]), A::S(&[])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::S(&[0xd800]), A::S(&[0x0024, 0x0024])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::S(&[0xd800]), A::S(&[0x0024])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "replace", &[A::S(&[0xd800]), A::S(&[0x0061, 0x0024])], r##""aaaa""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::S(&[]), A::S(&[])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::S(&[]), A::S(&[0x0024, 0x0031])], r##""$1a$1b$1""##),
    (A::S(&[0x0061, 0x0062]), "replace", &[A::S(&[]), A::S(&[0x0024])], r##""$ab""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::S(&[0x0062]), A::S(&[0x0058])], r##""aX""##),
    (A::S(&[0x0061, 0x0062]), "replace", &[A::S(&[0x0062]), A::S(&[0x0024, 0x0031])], r##""a$1""##),
    (A::S(&[0x0061, 0x0062]), "replace", &[A::S(&[0x0061]), A::S(&[0x0024, 0x0027])], r##""bb""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::S(&[0x0061]), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""[aa]b""##),
    (A::S(&[0x0061, 0x0062]), "replace", &[A::S(&[0x0061]), A::S(&[0x0024])], r##""$b""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::S(&[0x0061]), A::S(&[0x0059, 0x0024])], r##""Y$b""##),
    (A::S(&[0x0061, 0x0062]), "replace", &[A::S(&[0x0078]), A::S(&[0x0058])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::S(&[0x0078]), A::S(&[0x0024, 0x0060])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::U, A::S(&[0x0024, 0x0024])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replace", &[A::U, A::S(&[0x0024, 0x0031])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::U, A::S(&[0x0024, 0x0031])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::U, A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::U, A::S(&[0x0061, 0x0024])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::U, A::S(&[0x0059, 0x0024])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replace", &[A::Nl, A::S(&[0x0024, 0x0026])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replace", &[A::Nl, A::S(&[0x0024, 0x0060])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::Nl, A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replace", &[A::Nl, A::S(&[0x0059, 0x0024])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replace", &[A::N(1.0), A::S(&[0x0058])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0058])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0024, 0x0026])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replace", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0024, 0x0031])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replace", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0024])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replace", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0059, 0x0024])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0059, 0x0024])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::S(&[0x0061, 0x0062]), A::S(&[0x0024, 0x0026])], r##""ab""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::S(&[0x0061, 0x0062]), A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""$<a>""##),
    (A::S(&[0x0061, 0x0062]), "replace", &[A::S(&[0x0061, 0x0062]), A::S(&[0x0061, 0x0024])], r##""a$""##),
    (A::S(&[0x0061, 0x0062]), "replaceAll", &[A::S(&[0xd800]), A::S(&[0x0059, 0x0024])], r##""ab""##),
    (A::S(&[]), "replace", &[A::S(&[]), A::S(&[])], r##""""##),
    (A::S(&[]), "replaceAll", &[A::S(&[]), A::S(&[0x0061, 0x0024])], r##""a$""##),
    (A::S(&[]), "replaceAll", &[A::S(&[]), A::S(&[0x0059, 0x0024])], r##""Y$""##),
    (A::S(&[]), "replaceAll", &[A::S(&[0x0062]), A::S(&[0x0024, 0x0024])], r##""""##),
    (A::S(&[]), "replaceAll", &[A::S(&[0x0062]), A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""""##),
    (A::S(&[]), "replace", &[A::S(&[0x0062]), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""""##),
    (A::S(&[]), "replace", &[A::S(&[0x0062, 0x0062]), A::S(&[0x0024, 0x0031])], r##""""##),
    (A::S(&[]), "replaceAll", &[A::S(&[0x0062, 0x0062]), A::S(&[0x0024, 0x0031])], r##""""##),
    (A::S(&[]), "replace", &[A::S(&[0x0062, 0x0062]), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""""##),
    (A::S(&[]), "replaceAll", &[A::S(&[0x0062, 0x0062]), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""""##),
    (A::S(&[]), "replace", &[A::S(&[0x0061]), A::S(&[0x0024, 0x0031])], r##""""##),
    (A::S(&[]), "replaceAll", &[A::S(&[0x0061]), A::S(&[0x0024, 0x0031])], r##""""##),
    (A::S(&[]), "replace", &[A::S(&[0x0061]), A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""""##),
    (A::S(&[]), "replace", &[A::S(&[0x0078]), A::S(&[])], r##""""##),
    (A::S(&[]), "replace", &[A::S(&[0x0078]), A::S(&[0x0024, 0x0024])], r##""""##),
    (A::S(&[]), "replaceAll", &[A::S(&[0x0078]), A::S(&[0x0024, 0x0027])], r##""""##),
    (A::S(&[]), "replaceAll", &[A::U, A::S(&[0x0024, 0x0027])], r##""""##),
    (A::S(&[]), "replaceAll", &[A::U, A::S(&[0x0024])], r##""""##),
    (A::S(&[]), "replaceAll", &[A::U, A::S(&[0x0061, 0x0024])], r##""""##),
    (A::S(&[]), "replace", &[A::Nl, A::S(&[0x0058])], r##""""##),
    (A::S(&[]), "replaceAll", &[A::Nl, A::S(&[0x0024, 0x0026])], r##""""##),
    (A::S(&[]), "replaceAll", &[A::Nl, A::S(&[0x0024, 0x0024])], r##""""##),
    (A::S(&[]), "replace", &[A::Nl, A::S(&[0x0061, 0x0024])], r##""""##),
    (A::S(&[]), "replaceAll", &[A::N(1.0), A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""""##),
    (A::S(&[]), "replace", &[A::N(1.0), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""""##),
    (A::S(&[]), "replace", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[])], r##""""##),
    (A::S(&[]), "replace", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0058])], r##""""##),
    (A::S(&[]), "replace", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0024, 0x0026])], r##""""##),
    (A::S(&[]), "replaceAll", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0024, 0x0026])], r##""""##),
    (A::S(&[]), "replaceAll", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0024, 0x0024])], r##""""##),
    (A::S(&[]), "replace", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0024, 0x0027])], r##""""##),
    (A::S(&[]), "replaceAll", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""""##),
    (A::S(&[]), "replace", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0061, 0x0024])], r##""""##),
    (A::S(&[]), "replaceAll", &[A::S(&[0xd800]), A::S(&[])], r##""""##),
    (A::S(&[]), "replaceAll", &[A::S(&[0xd800]), A::S(&[0x0024, 0x0031])], r##""""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replace", &[A::S(&[]), A::S(&[0x0024, 0x0026])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replaceAll", &[A::S(&[]), A::S(&[0x0024, 0x0024])], r##""$a$$$b$""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replace", &[A::S(&[]), A::S(&[0x0059, 0x0024])], r##""Y$a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replaceAll", &[A::S(&[0x0062]), A::S(&[0x0024, 0x0027])], r##""a$""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replace", &[A::S(&[0x0062, 0x0062]), A::S(&[0x0024, 0x0027])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replace", &[A::S(&[0x0061]), A::S(&[0x0061, 0x0024])], r##""a$$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replaceAll", &[A::S(&[0x0078]), A::S(&[0x0024, 0x0027])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replace", &[A::S(&[0x0078]), A::S(&[0x0024, 0x0031])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replace", &[A::S(&[0x0078]), A::S(&[0x0024])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replaceAll", &[A::S(&[0x0078]), A::S(&[0x0061, 0x0024])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replaceAll", &[A::U, A::S(&[0x0024, 0x0026])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replaceAll", &[A::U, A::S(&[0x0024, 0x0024])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replaceAll", &[A::U, A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replace", &[A::U, A::S(&[0x0061, 0x0024])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replaceAll", &[A::U, A::S(&[0x0061, 0x0024])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replace", &[A::Nl, A::S(&[0x0024])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replaceAll", &[A::Nl, A::S(&[0x0024])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replace", &[A::N(1.0), A::S(&[0x0058])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replace", &[A::N(1.0), A::S(&[0x0024, 0x0027])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replace", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0058])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replaceAll", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0024, 0x0060])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replace", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0024, 0x0027])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replace", &[A::S(&[0x0061, 0x0062]), A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replaceAll", &[A::S(&[0x0061, 0x0062]), A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replace", &[A::S(&[0x0061, 0x0062]), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replace", &[A::S(&[0xd800]), A::S(&[0x0024, 0x0024])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replaceAll", &[A::S(&[0xd800]), A::S(&[0x0024, 0x0060])], r##""a$b""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "replace", &[A::S(&[0xd800]), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""a$b""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replaceAll", &[A::S(&[0x0062]), A::S(&[0x0058])], r##""xayXz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replaceAll", &[A::S(&[0x0062]), A::S(&[0x0024, 0x0031])], r##""xay$1z""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replace", &[A::S(&[0x0062]), A::S(&[0x0024])], r##""xay$z""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replace", &[A::S(&[0x0062, 0x0062]), A::S(&[0x0024, 0x0026])], r##""xaybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replace", &[A::S(&[0x0062, 0x0062]), A::S(&[0x0024])], r##""xaybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replace", &[A::S(&[0x0062, 0x0062]), A::S(&[0x0059, 0x0024])], r##""xaybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replaceAll", &[A::S(&[0x0061]), A::S(&[0x0058])], r##""xXybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replaceAll", &[A::S(&[0x0061]), A::S(&[0x0024, 0x0027])], r##""xybzybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replace", &[A::S(&[0x0061]), A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""x$<a>ybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replace", &[A::S(&[0x0061]), A::S(&[0x0059, 0x0024])], r##""xY$ybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replace", &[A::S(&[0x0078]), A::S(&[0x0024, 0x0060])], r##""aybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replaceAll", &[A::S(&[0x0078]), A::S(&[0x0024, 0x0027])], r##""aybzaybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replace", &[A::S(&[0x0078]), A::S(&[0x0061, 0x0024])], r##""a$aybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replace", &[A::U, A::S(&[0x0058])], r##""xaybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replaceAll", &[A::U, A::S(&[0x0024, 0x0024])], r##""xaybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replace", &[A::U, A::S(&[0x0024, 0x0031])], r##""xaybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replaceAll", &[A::Nl, A::S(&[0x0024, 0x0024])], r##""xaybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replaceAll", &[A::Nl, A::S(&[0x0024])], r##""xaybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replace", &[A::N(1.0), A::S(&[0x0024])], r##""xaybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replace", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0024, 0x0031])], r##""xaybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replace", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""xaybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replace", &[A::S(&[0x0061, 0x0062]), A::S(&[0x0024, 0x0031])], r##""xaybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replaceAll", &[A::S(&[0xd800]), A::S(&[0x0061, 0x0024])], r##""xaybz""##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "replaceAll", &[A::S(&[0xd800]), A::S(&[0x0059, 0x0024])], r##""xaybz""##),
    (A::S(&[0xd800, 0xd800]), "replace", &[A::S(&[]), A::S(&[0x0059, 0x0024])], r##""Y$\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replace", &[A::S(&[0x0062]), A::S(&[0x0024, 0x0024])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replaceAll", &[A::S(&[0x0062]), A::S(&[0x0024, 0x0027])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replaceAll", &[A::S(&[0x0062]), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replace", &[A::S(&[0x0062]), A::S(&[0x0059, 0x0024])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replace", &[A::S(&[0x0061]), A::S(&[])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replaceAll", &[A::S(&[0x0061]), A::S(&[0x0058])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replaceAll", &[A::S(&[0x0061]), A::S(&[0x0024, 0x0026])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replace", &[A::S(&[0x0061]), A::S(&[0x0061, 0x0024])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replaceAll", &[A::S(&[0x0078]), A::S(&[0x0058])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replaceAll", &[A::S(&[0x0078]), A::S(&[0x0024, 0x0024])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replace", &[A::S(&[0x0078]), A::S(&[0x0024, 0x0027])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replaceAll", &[A::S(&[0x0078]), A::S(&[0x0024, 0x0031])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replace", &[A::S(&[0x0078]), A::S(&[0x0059, 0x0024])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replace", &[A::U, A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replace", &[A::U, A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replace", &[A::U, A::S(&[0x0024])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replace", &[A::Nl, A::S(&[0x0024, 0x0026])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replaceAll", &[A::Nl, A::S(&[0x0059, 0x0024])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replaceAll", &[A::N(1.0), A::S(&[0x0058])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replace", &[A::N(1.0), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replaceAll", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0024, 0x0026])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replaceAll", &[A::S(&[0x0061, 0x0062]), A::S(&[0x0024, 0x0026])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replace", &[A::S(&[0x0061, 0x0062]), A::S(&[0x0024])], r##""\ud800\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replaceAll", &[A::S(&[0xd800]), A::S(&[0x0024, 0x0060])], r##""\ud800""##),
    (A::S(&[0xd800, 0xd800]), "replace", &[A::S(&[0xd800]), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""[\ud800\ud800]\ud800""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replace", &[A::S(&[]), A::S(&[0x0024, 0x0027])], r##""ab\ud83d\ude00cdab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replaceAll", &[A::S(&[]), A::S(&[0x0024, 0x0027])], r##""ab\ud83d\ude00cdab\ud83d\ude00cdb\ud83d\ude00cd\ud83d\ude00cd\ude00cdcdd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replace", &[A::S(&[]), A::S(&[0x0024, 0x0031])], r##""$1ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replace", &[A::S(&[0x0062]), A::S(&[0x0058])], r##""aX\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replaceAll", &[A::S(&[0x0062]), A::S(&[0x0024, 0x0060])], r##""aa\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replaceAll", &[A::S(&[0x0062, 0x0062]), A::S(&[0x0058])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replaceAll", &[A::S(&[0x0062, 0x0062]), A::S(&[0x0024, 0x0027])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replace", &[A::S(&[0x0062, 0x0062]), A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replaceAll", &[A::S(&[0x0062, 0x0062]), A::S(&[0x0059, 0x0024])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replaceAll", &[A::S(&[0x0061]), A::S(&[0x0024, 0x0024])], r##""$b\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replace", &[A::S(&[0x0061]), A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""$<a>b\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replaceAll", &[A::S(&[0x0061]), A::S(&[0x0061, 0x0024])], r##""a$b\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replaceAll", &[A::S(&[0x0078]), A::S(&[0x0058])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replace", &[A::S(&[0x0078]), A::S(&[0x0024, 0x0031])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replace", &[A::U, A::S(&[0x0024, 0x0026])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replaceAll", &[A::U, A::S(&[0x0024, 0x0027])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replace", &[A::Nl, A::S(&[0x0059, 0x0024])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replace", &[A::N(1.0), A::S(&[0x0058])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replace", &[A::N(1.0), A::S(&[0x0024, 0x0026])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replace", &[A::N(1.0), A::S(&[0x0024, 0x0060])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replace", &[A::N(1.0), A::S(&[0x0024, 0x0027])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replaceAll", &[A::N(1.0), A::S(&[0x0024, 0x0031])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replace", &[A::N(1.0), A::S(&[0x0024, 0x003c, 0x0061, 0x003e])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replaceAll", &[A::N(1.0), A::S(&[0x0024])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replace", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replaceAll", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0058])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replace", &[A::S(&[0x0061, 0x0062]), A::S(&[])], r##""\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replaceAll", &[A::S(&[0x0061, 0x0062]), A::S(&[0x0058])], r##""X\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replace", &[A::S(&[0xd800]), A::S(&[0x005b, 0x0024, 0x0026, 0x0024, 0x0026, 0x005d])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replace", &[A::S(&[0xd800]), A::S(&[0x0061, 0x0024])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "replaceAll", &[A::S(&[0xd800]), A::S(&[0x0061, 0x0024])], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "split", &[A::U, A::N(-1.0)], r##"["abcbb"]"##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "split", &[A::Nl, A::N(2.0)], r##"["abcbb"]"##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "split", &[A::S(&[]), A::N(4294967297.0)], r##"["a"]"##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "split", &[A::S(&[0x002c]), A::N(0.0)], r##"[]"##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "split", &[A::S(&[0x0062]), A::N(1.0)], r##"["a"]"##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "split", &[A::S(&[0x0062, 0x0063]), A::N(0.0)], r##"[]"##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "split", &[A::S(&[0x0062, 0x0063]), A::N(-0.5)], r##"[]"##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "split", &[A::S(&[0x0061, 0x0062])], r##"["","cbb"]"##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "split", &[A::N(1.0), A::N(1.0)], r##"["abcbb"]"##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "split", &[A::N(1.0), A::S(&[0x0032])], r##"["abcbb"]"##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "split", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(f64::NAN)], r##"[]"##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "split", &[A::S(&[0x0020]), A::N(f64::NAN)], r##"[]"##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "split", &[A::S(&[0x0020]), A::N(-0.5)], r##"[]"##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "split", &[A::S(&[0xd800]), A::S(&[0x0032])], r##"["abcbb"]"##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "split", &[A::S(&[0x002c, 0x002c])], r##"["abcbb"]"##),
    (A::S(&[0x0061, 0x0062, 0x0063, 0x0062, 0x0062]), "split", &[A::S(&[0x002c, 0x002c]), A::N(-1.0)], r##"["abcbb"]"##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "split", &[A::U, A::N(-0.5)], r##"[]"##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "split", &[A::Nl, A::S(&[0x0032])], r##"["aaaa"]"##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "split", &[A::S(&[]), A::N(3.9)], r##"["a","a","a"]"##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "split", &[A::S(&[0x002c])], r##"["aaaa"]"##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "split", &[A::S(&[0x002c]), A::N(1.0)], r##"["aaaa"]"##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "split", &[A::S(&[0x002c]), A::N(4294967297.0)], r##"["aaaa"]"##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "split", &[A::S(&[0x002c]), A::N(f64::NAN)], r##"[]"##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "split", &[A::S(&[0x002c]), A::S(&[0x0032])], r##"["aaaa"]"##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "split", &[A::S(&[0x0062]), A::N(4294967297.0)], r##"["aaaa"]"##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "split", &[A::S(&[0x0062, 0x0063]), A::S(&[0x0032])], r##"["aaaa"]"##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "split", &[A::S(&[0x0061, 0x0062]), A::N(0.0)], r##"[]"##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "split", &[A::S(&[0x0061, 0x0062]), A::N(1.0)], r##"["aaaa"]"##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "split", &[A::N(1.0), A::N(0.0)], r##"[]"##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "split", &[A::N(1.0), A::N(4294967297.0)], r##"["aaaa"]"##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "split", &[A::N(1.0), A::N(-0.5)], r##"[]"##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "split", &[A::S(&[0x002c, 0x002c])], r##"["aaaa"]"##),
    (A::S(&[0x0061, 0x0061, 0x0061, 0x0061]), "split", &[A::S(&[0x002c, 0x002c]), A::N(1.0)], r##"["aaaa"]"##),
    (A::S(&[0x0061, 0x0062]), "split", &[A::Nl, A::N(f64::NAN)], r##"[]"##),
    (A::S(&[0x0061, 0x0062]), "split", &[A::S(&[]), A::N(1.0)], r##"["a"]"##),
    (A::S(&[0x0061, 0x0062]), "split", &[A::S(&[]), A::N(3.9)], r##"["a","b"]"##),
    (A::S(&[0x0061, 0x0062]), "split", &[A::S(&[0x002c]), A::N(3.9)], r##"["ab"]"##),
    (A::S(&[0x0061, 0x0062]), "split", &[A::S(&[0x002c]), A::N(-0.5)], r##"[]"##),
    (A::S(&[0x0061, 0x0062]), "split", &[A::S(&[0x0062])], r##"["a",""]"##),
    (A::S(&[0x0061, 0x0062]), "split", &[A::S(&[0x0062]), A::N(f64::NAN)], r##"[]"##),
    (A::S(&[0x0061, 0x0062]), "split", &[A::S(&[0x0062, 0x0063]), A::N(1.0)], r##"["ab"]"##),
    (A::S(&[0x0061, 0x0062]), "split", &[A::S(&[0x0062, 0x0063]), A::S(&[0x0032])], r##"["ab"]"##),
    (A::S(&[0x0061, 0x0062]), "split", &[A::S(&[0x0061, 0x0062]), A::S(&[0x0032])], r##"["",""]"##),
    (A::S(&[0x0061, 0x0062]), "split", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(2.0)], r##"["ab"]"##),
    (A::S(&[0x0061, 0x0062]), "split", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0032])], r##"["ab"]"##),
    (A::S(&[0x0061, 0x0062]), "split", &[A::S(&[0xd800]), A::N(2.0)], r##"["ab"]"##),
    (A::S(&[0x0061, 0x0062]), "split", &[A::S(&[0xd800]), A::N(3.9)], r##"["ab"]"##),
    (A::S(&[0x0061, 0x0062]), "split", &[A::S(&[0x002c, 0x002c]), A::N(f64::NAN)], r##"[]"##),
    (A::S(&[0x0061, 0x0062]), "split", &[A::S(&[0x002c, 0x002c]), A::N(-0.5)], r##"[]"##),
    (A::S(&[]), "split", &[A::U, A::N(-0.5)], r##"[]"##),
    (A::S(&[]), "split", &[A::S(&[]), A::S(&[0x0032])], r##"[]"##),
    (A::S(&[]), "split", &[A::S(&[0x002c]), A::N(4294967297.0)], r##"[""]"##),
    (A::S(&[]), "split", &[A::S(&[0x0062]), A::N(f64::NAN)], r##"[]"##),
    (A::S(&[]), "split", &[A::S(&[0x0061, 0x0062]), A::N(3.9)], r##"[""]"##),
    (A::S(&[]), "split", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0032])], r##"[""]"##),
    (A::S(&[]), "split", &[A::S(&[0x0020])], r##"[""]"##),
    (A::S(&[]), "split", &[A::S(&[0x002c, 0x002c]), A::N(0.0)], r##"[]"##),
    (A::S(&[]), "split", &[A::S(&[0x002c, 0x002c]), A::N(2.0)], r##"[""]"##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "split", &[A::S(&[]), A::N(2.0)], r##"["a","$"]"##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "split", &[A::S(&[0x002c]), A::N(1.0)], r##"["a$b"]"##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "split", &[A::S(&[0x0062, 0x0063]), A::S(&[0x0032])], r##"["a$b"]"##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "split", &[A::N(1.0), A::N(3.9)], r##"["a$b"]"##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "split", &[A::N(1.0), A::N(-0.5)], r##"[]"##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "split", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(2.0)], r##"["a$b"]"##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "split", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(-1.0)], r##"["a$b"]"##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "split", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::S(&[0x0032])], r##"["a$b"]"##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "split", &[A::S(&[0x002c, 0x002c]), A::N(0.0)], r##"[]"##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "split", &[A::S(&[0x002c, 0x002c]), A::N(4294967297.0)], r##"["a$b"]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::U, A::N(2.0)], r##"["xaybz"]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::U, A::N(4294967297.0)], r##"["xaybz"]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::U, A::N(f64::NAN)], r##"[]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::Nl, A::N(4294967297.0)], r##"["xaybz"]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::S(&[])], r##"["x","a","y","b","z"]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::S(&[]), A::N(4294967297.0)], r##"["x"]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::S(&[0x002c]), A::N(0.0)], r##"[]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::S(&[0x002c]), A::N(f64::NAN)], r##"[]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::S(&[0x002c]), A::N(-0.5)], r##"[]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::S(&[0x0062]), A::N(1.0)], r##"["xay"]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::S(&[0x0062]), A::N(-1.0)], r##"["xay","z"]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::S(&[0x0062, 0x0063]), A::S(&[0x0032])], r##"["xaybz"]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(2.0)], r##"["xaybz"]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(-0.5)], r##"[]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::S(&[0x0020])], r##"["xaybz"]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::S(&[0x0020]), A::N(0.0)], r##"[]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::S(&[0x0020]), A::N(1.0)], r##"["xaybz"]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::S(&[0x0020]), A::N(f64::NAN)], r##"[]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::S(&[0xd800]), A::N(f64::NAN)], r##"[]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::S(&[0x002c, 0x002c])], r##"["xaybz"]"##),
    (A::S(&[0x0078, 0x0061, 0x0079, 0x0062, 0x007a]), "split", &[A::S(&[0x002c, 0x002c]), A::N(-1.0)], r##"["xaybz"]"##),
    (A::S(&[0xd800, 0xd800]), "split", &[A::U, A::N(0.0)], r##"[]"##),
    (A::S(&[0xd800, 0xd800]), "split", &[A::U, A::N(f64::NAN)], r##"[]"##),
    (A::S(&[0xd800, 0xd800]), "split", &[A::U, A::N(3.9)], r##"["\ud800\ud800"]"##),
    (A::S(&[0xd800, 0xd800]), "split", &[A::Nl, A::N(-1.0)], r##"["\ud800\ud800"]"##),
    (A::S(&[0xd800, 0xd800]), "split", &[A::Nl, A::N(3.9)], r##"["\ud800\ud800"]"##),
    (A::S(&[0xd800, 0xd800]), "split", &[A::S(&[])], r##"["\ud800","\ud800"]"##),
    (A::S(&[0xd800, 0xd800]), "split", &[A::S(&[]), A::N(0.0)], r##"[]"##),
    (A::S(&[0xd800, 0xd800]), "split", &[A::S(&[]), A::N(1.0)], r##"["\ud800"]"##),
    (A::S(&[0xd800, 0xd800]), "split", &[A::S(&[]), A::N(2.0)], r##"["\ud800","\ud800"]"##),
    (A::S(&[0xd800, 0xd800]), "split", &[A::S(&[]), A::S(&[0x0032])], r##"["\ud800","\ud800"]"##),
    (A::S(&[0xd800, 0xd800]), "split", &[A::S(&[0x0062]), A::N(-1.0)], r##"["\ud800\ud800"]"##),
    (A::S(&[0xd800, 0xd800]), "split", &[A::S(&[0x0062]), A::N(-0.5)], r##"[]"##),
    (A::S(&[0xd800, 0xd800]), "split", &[A::S(&[0x0062, 0x0063]), A::N(-0.5)], r##"[]"##),
    (A::S(&[0xd800, 0xd800]), "split", &[A::S(&[0x0061, 0x0062]), A::N(2.0)], r##"["\ud800\ud800"]"##),
    (A::S(&[0xd800, 0xd800]), "split", &[A::S(&[0x0061, 0x0062]), A::N(f64::NAN)], r##"[]"##),
    (A::S(&[0xd800, 0xd800]), "split", &[A::S(&[0x0061, 0x0062]), A::S(&[0x0032])], r##"["\ud800\ud800"]"##),
    (A::S(&[0xd800, 0xd800]), "split", &[A::S(&[0x0020]), A::S(&[0x0032])], r##"["\ud800\ud800"]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::U, A::N(-1.0)], r##"["ab\ud83d\ude00cd"]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::U, A::N(3.9)], r##"["ab\ud83d\ude00cd"]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::Nl], r##"["ab\ud83d\ude00cd"]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::Nl, A::N(-1.0)], r##"["ab\ud83d\ude00cd"]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::Nl, A::S(&[0x0032])], r##"["ab\ud83d\ude00cd"]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::S(&[]), A::N(1.0)], r##"["a"]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::S(&[]), A::N(3.9)], r##"["a","b","\ud83d"]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::S(&[]), A::N(-0.5)], r##"[]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::S(&[0x002c])], r##"["ab\ud83d\ude00cd"]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::S(&[0x0062]), A::N(1.0)], r##"["a"]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::S(&[0x0062]), A::N(4294967297.0)], r##"["a"]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::S(&[0x0062]), A::N(f64::NAN)], r##"[]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::S(&[0x0062, 0x0063]), A::N(3.9)], r##"["ab\ud83d\ude00cd"]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064])], r##"["ab\ud83d\ude00cd"]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::S(&[0x0075, 0x006e, 0x0064, 0x0065, 0x0066, 0x0069, 0x006e, 0x0065, 0x0064]), A::N(3.9)], r##"["ab\ud83d\ude00cd"]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::S(&[0x0020]), A::N(f64::NAN)], r##"[]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::S(&[0xd800])], r##"["ab\ud83d\ude00cd"]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::S(&[0xd800]), A::N(f64::NAN)], r##"[]"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "split", &[A::S(&[0x002c, 0x002c]), A::N(1.0)], r##"["ab\ud83d\ude00cd"]"##),
    (A::S(&[0x0061, 0x0062, 0x0063]), "concat", &[A::Nl], r##""abcnull""##),
    (A::S(&[0xd800, 0x0078, 0xdc00]), "concat", &[A::U], r##""\ud800x\udc00undefined""##),
    (A::S(&[0x0061]), "trimStart", &[], r##""a""##),
    (A::S(&[0x0061]), "trimEnd", &[], r##""a""##),
    (A::S(&[0x0061, 0x0058, 0x0062, 0x0058, 0x0063]), "isWellFormed", &[], r##"true"##),
    (A::S(&[0x0061, 0x0062, 0xd83d, 0xde00, 0x0063, 0x0064]), "trimEnd", &[], r##""ab\ud83d\ude00cd""##),
    (A::S(&[0x00df, 0x0041, 0x0391, 0x03a3]), "isWellFormed", &[], r##"true"##),
    (A::S(&[0x0041, 0x0042, 0x0043, 0x0064, 0x0065, 0x0066]), "trimEnd", &[], r##""ABCdef""##),
    (A::S(&[0x0061, 0x0024, 0x0062]), "toUpperCase", &[], r##""A$B""##),
    (A::N(-0.0), "codePointAt", &[A::N(0.0)], r##"48"##),
    (A::U, "concat", &[], r##"throw:TypeError"##),
    (A::N(-0.0), "concat", &[A::N(0.0)], r##""00""##),
    (A::N(12.0), "endsWith", &[A::N(0.0)], r##"false"##),
    (A::N(-0.0), "endsWith", &[A::N(0.0)], r##"true"##),
    (A::N(1.5), "endsWith", &[A::N(0.0)], r##"false"##),
    (A::N(12.0), "indexOf", &[A::N(0.0)], r##"-1"##),
    (A::N(1.5), "isWellFormed", &[A::N(0.0)], r##"true"##),
    (A::N(-0.0), "lastIndexOf", &[A::N(0.0)], r##"0"##),
    (A::N(-0.0), "padEnd", &[A::N(0.0)], r##""0""##),
    (A::N(-0.0), "replace", &[A::N(0.0)], r##""undefined""##),
    (A::N(-0.0), "replaceAll", &[A::N(0.0)], r##""undefined""##),
    (A::B(true), "replaceAll", &[A::N(0.0)], r##""true""##),
    (A::N(12.0), "slice", &[A::N(0.0)], r##""12""##),
    (A::Nl, "substr", &[], r##"throw:TypeError"##),
    (A::N(-0.0), "substr", &[A::N(0.0)], r##""0""##),
    (A::N(1.5), "substring", &[A::N(0.0)], r##""1.5""##),
    (A::N(-0.0), "toWellFormed", &[A::N(0.0)], r##""0""##),
    (A::Nl, "valueOf", &[], r##"throw:TypeError"##),
];

const STATIC_CASES: &[(&str, &[A], &str)] = &[
    ("fromCharCode", &[], r##""""##),
    ("fromCharCode", &[A::N(65.0)], r##""A""##),
    ("fromCharCode", &[A::N(65.0), A::N(66.0), A::N(67.0)], r##""ABC""##),
    ("fromCharCode", &[A::N(65601.0)], r##""A""##),
    ("fromCharCode", &[A::N(-1.0)], r##""\uffff""##),
    ("fromCharCode", &[A::N(f64::NAN)], r##""\u0000""##),
    ("fromCharCode", &[A::N(f64::INFINITY)], r##""\u0000""##),
    ("fromCharCode", &[A::S(&[0x0036, 0x0035])], r##""A""##),
    ("fromCharCode", &[A::B(true)], r##""\u0001""##),
    ("fromCharCode", &[A::Nl], r##""\u0000""##),
    ("fromCharCode", &[A::N(1.9)], r##""\u0001""##),
    ("fromCharCode", &[A::N(55296.0), A::N(56320.0)], r##""\ud800\udc00""##),
    ("fromCharCode", &[A::N(4294967361.0)], r##""A""##),
    ("fromCharCode", &[A::U], r##""\u0000""##),
    ("fromCharCode", &[A::N(-0.0)], r##""\u0000""##),
    ("fromCharCode", &[A::N(65601.0), A::S(&[0x0078])], r##""A\u0000""##),
    ("fromCodePoint", &[], r##""""##),
    ("fromCodePoint", &[A::N(65.0)], r##""A""##),
    ("fromCodePoint", &[A::N(128512.0)], r##""\ud83d\ude00""##),
    ("fromCodePoint", &[A::N(1114111.0)], r##""\udbff\udfff""##),
    ("fromCodePoint", &[A::N(1114112.0)], r##"throw:RangeError"##),
    ("fromCodePoint", &[A::N(-0.0)], r##""\u0000""##),
    ("fromCodePoint", &[A::N(f64::NAN)], r##"throw:RangeError"##),
    ("fromCodePoint", &[A::N(1.5)], r##"throw:RangeError"##),
    ("fromCodePoint", &[A::S(&[0x0036, 0x0036])], r##""B""##),
    ("fromCodePoint", &[A::N(55296.0)], r##""\ud800""##),
    ("fromCodePoint", &[A::N(65.0), A::N(1114112.0)], r##"throw:RangeError"##),
    ("fromCodePoint", &[A::N(-1.0)], r##"throw:RangeError"##),
    ("fromCodePoint", &[A::N(f64::INFINITY)], r##"throw:RangeError"##),
    ("fromCodePoint", &[A::B(true)], r##""\u0001""##),
    ("fromCodePoint", &[A::N(0.0)], r##""\u0000""##),
    ("fromCodePoint", &[A::N(65.0), A::N(66.0), A::N(128512.0)], r##""AB\ud83d\ude00""##),
];

const CTOR_CASES: &[(&[A], &str)] = &[
    (&[], r##""""##),
    (&[A::U], r##""undefined""##),
    (&[A::Nl], r##""null""##),
    (&[A::B(true)], r##""true""##),
    (&[A::N(1e21)], r##""1e+21""##),
    (&[A::N(-0.0)], r##""0""##),
    (&[A::N(f64::NAN)], r##""NaN""##),
    (&[A::S(&[0x0078])], r##""x""##),
    (&[A::N(1.5)], r##""1.5""##),
    (&[A::N(12.0), A::S(&[0x0078])], r##""12""##),
    (&[A::N(128512.0)], r##""128512""##),
];

const PROTO_LENGTHS: &[(&str, u32)] = &[
    ("at", 1),
    ("charAt", 1),
    ("charCodeAt", 1),
    ("codePointAt", 1),
    ("concat", 1),
    ("endsWith", 1),
    ("includes", 1),
    ("indexOf", 1),
    ("isWellFormed", 0),
    ("lastIndexOf", 1),
    ("padEnd", 1),
    ("padStart", 1),
    ("repeat", 1),
    ("replace", 2),
    ("replaceAll", 2),
    ("slice", 2),
    ("split", 2),
    ("startsWith", 1),
    ("substr", 2),
    ("substring", 2),
    ("toLowerCase", 0),
    ("toString", 0),
    ("toUpperCase", 0),
    ("toWellFormed", 0),
    ("trim", 0),
    ("trimEnd", 0),
    ("trimStart", 0),
    ("valueOf", 0),
];

const STATIC_LENGTHS: &[(&str, u32)] = &[
    ("fromCharCode", 1),
    ("fromCodePoint", 1),
];

// ----- hand-written: conversion order, logging objects, callbacks -----

thread_local! {
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn log(entry: String) {
    LOG.with(|l| l.borrow_mut().push(entry));
}

fn take_log() -> String {
    LOG.with(|l| l.borrow_mut().drain(..).collect::<Vec<_>>().join(","))
}

fn text_of(rt: &Rt, v: &Value) -> String {
    rt.to_string(v).map(|s| s.to_rust_string_lossy()).unwrap_or_default()
}

/// An object whose valueOf returns \`value\` and whose toString returns ToString(value); both log \`tag\`.
fn tagged(rt: &Rt, tag: &str, value: Value) -> Value {
    let o = rt.new_object();
    let data = vec![Value::str(tag), value];
    let vo = rt.new_native_function(tagged_value_of, "valueOf", 0, data.clone());
    let ts = rt.new_native_function(tagged_to_string, "toString", 0, data);
    rt.define_data(o, PropKey::from_str("valueOf"), Value::Object(vo), Flags::HIDDEN).expect("define valueOf");
    rt.define_data(o, PropKey::from_str("toString"), Value::Object(ts), Flags::HIDDEN).expect("define toString");
    Value::Object(o)
}

fn tagged_value_of(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    log(format!("{}.valueOf", text_of(rt, &data[0])));
    Ok(data[1].clone())
}

fn tagged_to_string(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    log(format!("{}.toString", text_of(rt, &data[0])));
    Ok(Value::String(rt.to_string(&data[1])?))
}

/// A callback that logs its arguments as (a;b;c) and returns \`ret\`.
fn recorder(rt: &Rt, ret: Value) -> Value {
    Value::Object(rt.new_native_function(record_call, "cb", 3, vec![ret]))
}

fn record_call(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let shown: Vec<String> = c.args.iter().map(|a| show_val(rt, a)).collect();
    log(format!("({})", shown.join(";")));
    Ok(rt.fn_data(c.callee)[0].clone())
}

fn throwing(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_range_error("boom"))
}

fn symbol_result(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Symbol(rt.new_symbol(None)))
}

fn check(label: &str, got: &str, want: &str, log: &str, want_log: &str, fails: &mut Vec<String>) {
    if got != want || log != want_log {
        fails.push(format!("{label}: result {got} (V8 {want}); log [{log}] (V8 [{want_log}])"));
    }
}

#[test]
fn method_table_matches_v8() {
    let rt = Rt::new();
    let mut bad = Vec::new();
    for (i, (this, name, args, want)) in METHOD_CASES.iter().enumerate() {
        let args: Vec<Value> = args.iter().map(|a| to_value(*a)).collect();
        let got = show(&rt, &call_method(&rt, name, &to_value(*this), &args));
        if got != *want {
            bad.push(format!("#{i} {name} this={this:?} args={args:?}: V8 {want}, got {got}"));
        }
    }
    assert!(bad.is_empty(), "{} of {} method cases differ from V8:\n{}", bad.len(), METHOD_CASES.len(), bad.join("\n"));
}

#[test]
fn static_and_constructor_calls_match_v8() {
    let rt = Rt::new();
    let mut bad = Vec::new();
    for (name, args, want) in STATIC_CASES {
        let args: Vec<Value> = args.iter().map(|a| to_value(*a)).collect();
        let got = show(&rt, &call_static(&rt, name, &args));
        if got != *want {
            bad.push(format!("String.{name} {args:?}: V8 {want}, got {got}"));
        }
    }
    for (args, want) in CTOR_CASES {
        let args: Vec<Value> = args.iter().map(|a| to_value(*a)).collect();
        let got = show(&rt, &rt.call(&global(&rt, "String"), &Value::Undefined, &args));
        if got != *want {
            bad.push(format!("String({args:?}): V8 {want}, got {got}"));
        }
    }
    assert!(bad.is_empty(), "{} static or constructor cases differ from V8:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn conversion_order_matches_v8() {
    let rt = Rt::new();
    let s = Value::str;
    let n = Value::Number;
    let mut fails = Vec::new();
    let r = call_method(&rt, "indexOf", &s("abcb"), &[tagged(&rt, "s", s("b")), tagged(&rt, "p", n(2.0))]);
    check("indexOf", &show(&rt, &r), "3", &take_log(), "s.toString,p.valueOf", &mut fails);
    let r = call_method(&rt, "lastIndexOf", &s("abcb"), &[tagged(&rt, "s", s("b")), tagged(&rt, "p", n(2.9))]);
    check("lastIndexOf", &show(&rt, &r), "1", &take_log(), "s.toString,p.valueOf", &mut fails);
    let r = call_method(&rt, "padStart", &s("abc"), &[n(2.0), tagged(&rt, "f", s("x"))]);
    check("padStart short target", &show(&rt, &r), r#""abc""#, &take_log(), "", &mut fails);
    let r = call_method(&rt, "padStart", &s("abc"), &[n(5.0), tagged(&rt, "f", s("x"))]);
    check("padStart fill", &show(&rt, &r), r#""xxabc""#, &take_log(), "f.toString", &mut fails);
    let r = call_method(&rt, "padStart", &s("abc"), &[tagged(&rt, "m", n(2.0)), s("")]);
    check("padStart maxLength", &show(&rt, &r), r#""abc""#, &take_log(), "m.valueOf", &mut fails);
    let r = call_method(&rt, "padEnd", &s("abc"), &[tagged(&rt, "m", n(6.0)), tagged(&rt, "f", s("xy"))]);
    check("padEnd", &show(&rt, &r), r#""abcxyx""#, &take_log(), "m.valueOf,f.toString", &mut fails);
    let r = call_method(&rt, "split", &s("a,b"), &[tagged(&rt, "sep", s(",")), n(0.0)]);
    check("split limit 0", &show(&rt, &r), "[]", &take_log(), "sep.toString", &mut fails);
    let r = call_method(&rt, "split", &tagged(&rt, "this", s("a,b")), &[tagged(&rt, "sep", s(",")), tagged(&rt, "lim", n(5.0))]);
    check("split order", &show(&rt, &r), r#"["a","b"]"#, &take_log(), "this.toString,lim.valueOf,sep.toString", &mut fails);
    let r = call_method(&rt, "replace", &s("abc"), &[tagged(&rt, "pat", s("b")), tagged(&rt, "rep", s("X"))]);
    check("replace", &show(&rt, &r), r#""aXc""#, &take_log(), "pat.toString,rep.toString", &mut fails);
    let r = call_method(&rt, "replace", &s("abc"), &[s("z"), tagged(&rt, "rep", s("X"))]);
    check("replace no match", &show(&rt, &r), r#""abc""#, &take_log(), "rep.toString", &mut fails);
    let r = call_method(&rt, "replaceAll", &s("abcb"), &[tagged(&rt, "pat", s("b")), tagged(&rt, "rep", s("X"))]);
    check("replaceAll", &show(&rt, &r), r#""aXcX""#, &take_log(), "pat.toString,rep.toString", &mut fails);
    let r = call_method(&rt, "concat", &s("a"), &[tagged(&rt, "x", n(1.0)), tagged(&rt, "y", Value::Null), Value::Undefined]);
    check("concat", &show(&rt, &r), r#""a1nullundefined""#, &take_log(), "x.toString,y.toString", &mut fails);
    let r = call_method(&rt, "repeat", &s("ab"), &[tagged(&rt, "c", n(2.0))]);
    check("repeat", &show(&rt, &r), r#""abab""#, &take_log(), "c.valueOf", &mut fails);
    let r = call_method(&rt, "at", &tagged(&rt, "this", s("abc")), &[tagged(&rt, "i", n(1.0))]);
    check("at on object this", &show(&rt, &r), r#""b""#, &take_log(), "this.toString,i.valueOf", &mut fails);
    let r = call_method(&rt, "slice", &s("abcdef"), &[tagged(&rt, "s", n(1.0)), tagged(&rt, "e", n(-1.0))]);
    check("slice", &show(&rt, &r), r#""bcde""#, &take_log(), "s.valueOf,e.valueOf", &mut fails);
    let r = call_method(&rt, "substring", &s("abcdef"), &[tagged(&rt, "s", n(4.0)), tagged(&rt, "e", n(1.0))]);
    check("substring", &show(&rt, &r), r#""bcd""#, &take_log(), "s.valueOf,e.valueOf", &mut fails);
    let r = call_method(&rt, "substr", &s("abcdef"), &[tagged(&rt, "s", n(-4.0)), tagged(&rt, "l", n(2.0))]);
    check("substr", &show(&rt, &r), r#""cd""#, &take_log(), "s.valueOf,l.valueOf", &mut fails);
    let r = call_method(&rt, "endsWith", &s("abc"), &[tagged(&rt, "s", s("c")), tagged(&rt, "e", n(2.0))]);
    check("endsWith", &show(&rt, &r), "false", &take_log(), "s.toString,e.valueOf", &mut fails);
    let r = call_method(&rt, "startsWith", &s("abc"), &[tagged(&rt, "s", s("b")), tagged(&rt, "p", n(1.0))]);
    check("startsWith", &show(&rt, &r), "true", &take_log(), "s.toString,p.valueOf", &mut fails);
    let r = call_method(&rt, "includes", &s("abc"), &[tagged(&rt, "s", s("b")), tagged(&rt, "p", n(1.0))]);
    check("includes", &show(&rt, &r), "true", &take_log(), "s.toString,p.valueOf", &mut fails);
    let r = call_method(&rt, "codePointAt", &s("abc"), &[tagged(&rt, "p", n(0.0))]);
    check("codePointAt", &show(&rt, &r), "97", &take_log(), "p.valueOf", &mut fails);
    let r = call_method(&rt, "charAt", &s("abc"), &[tagged(&rt, "p", n(1.0))]);
    check("charAt", &show(&rt, &r), r#""b""#, &take_log(), "p.valueOf", &mut fails);
    let r = call_static(&rt, "fromCharCode", &[tagged(&rt, "a", n(1.0)), tagged(&rt, "b", n(2.0))]);
    check("fromCharCode", &show(&rt, &r), r#""\u0001\u0002""#, &take_log(), "a.valueOf,b.valueOf", &mut fails);
    let r = call_static(&rt, "fromCodePoint", &[n(1.0), n(0x110000 as f64), tagged(&rt, "cp", n(3.0))]);
    check("fromCodePoint stops at the first bad code point", &show(&rt, &r), "throw:RangeError", &take_log(), "", &mut fails);
    let r = call_static(&rt, "fromCodePoint", &[tagged(&rt, "a", n(-1.0)), tagged(&rt, "b", n(3.0))]);
    check("fromCodePoint negative", &show(&rt, &r), "throw:RangeError", &take_log(), "a.valueOf", &mut fails);
    let r = rt.call(&global(&rt, "String"), &Value::Undefined, &[tagged(&rt, "o", s("z"))]);
    check("String(object)", &show(&rt, &r), r#""z""#, &take_log(), "o.toString", &mut fails);
    assert!(fails.is_empty(), "conversion order differs from V8:\n{}", fails.join("\n"));
}

#[test]
fn callbacks_match_v8() {
    let rt = Rt::new();
    let s = Value::str;
    let mut fails = Vec::new();
    let all = |rt: &Rt, ret: Value| recorder(rt, ret);
    let r = call_method(&rt, "replaceAll", &s("xaybz"), &[s(""), all(&rt, s(""))]);
    check("replaceAll empty pattern", &show(&rt, &r), r#""xaybz""#, &take_log(), r#"("";0;"xaybz"),("";1;"xaybz"),("";2;"xaybz"),("";3;"xaybz"),("";4;"xaybz"),("";5;"xaybz")"#, &mut fails);
    let r = call_method(&rt, "replace", &s("xaybz"), &[s("y"), all(&rt, s("Q"))]);
    check("replace callback", &show(&rt, &r), r#""xaQbz""#, &take_log(), r#"("y";2;"xaybz")"#, &mut fails);
    let r = call_method(&rt, "replaceAll", &s("abcb"), &[s("b"), all(&rt, s("Q"))]);
    check("replaceAll callback", &show(&rt, &r), r#""aQcQ""#, &take_log(), r#"("b";1;"abcb"),("b";3;"abcb")"#, &mut fails);
    let r = call_method(&rt, "replace", &s("abc"), &[s("z"), all(&rt, s("Q"))]);
    check("replace callback not called", &show(&rt, &r), r#""abc""#, &take_log(), "", &mut fails);
    let cb_obj = tagged(&rt, "t", s("T"));
    let r = call_method(&rt, "replace", &s("abc"), &[s("b"), all(&rt, cb_obj)]);
    check("replace callback result converted", &show(&rt, &r), r#""aTc""#, &take_log(), r#"("b";1;"abc"),t.toString"#, &mut fails);
    let thrower = Value::Object(rt.new_native_function(throwing, "cb", 0, Vec::new()));
    let r = call_method(&rt, "replace", &s("abc"), &[s("b"), thrower]);
    check("replace callback throws", &show(&rt, &r), "throw:RangeError", &take_log(), "", &mut fails);
    let sym = Value::Object(rt.new_native_function(symbol_result, "cb", 0, Vec::new()));
    let r = call_method(&rt, "replace", &s("abc"), &[s("b"), sym]);
    check("replace callback returns a symbol", &show(&rt, &r), "throw:TypeError", &take_log(), "", &mut fails);
    assert!(fails.is_empty(), "callbacks differ from V8:\n{}", fails.join("\n"));
}

#[test]
fn symbols_and_wrappers() {
    let rt = Rt::new();
    let string = global(&rt, "String");
    let desc = Value::Symbol(rt.new_symbol(Some(JsStr::from_str("desc"))));
    let nodesc = Value::Symbol(rt.new_symbol(None));
    let mut fails = Vec::new();
    check("String(symbol)", &show(&rt, &rt.call(&string, &Value::Undefined, &[desc.clone()])), r#""Symbol(desc)""#, "", "", &mut fails);
    check("String(symbol without description)", &show(&rt, &rt.call(&string, &Value::Undefined, &[nodesc])), r#""Symbol()""#, "", "", &mut fails);
    check("new String(symbol)", &show(&rt, &rt.construct(&string, &[desc.clone()], &string)), "throw:TypeError", "", "", &mut fails);
    check("at with a symbol this", &show(&rt, &call_method(&rt, "at", &desc, &[])), "throw:TypeError", "", "", &mut fails);
    let w = rt.construct(&string, &[Value::str("abc")], &string).expect("new String");
    check("wrapper is an object", &show(&rt, &Ok(Value::Bool(w.is_object()))), "true", "", "", &mut fails);
    check("wrapper length", &show(&rt, &rt.get_str(&w, "length")), "3", "", "", &mut fails);
    check("wrapper index", &show(&rt, &rt.get_v(&w, &PropKey::Index(1))), r#""b""#, "", "", &mut fails);
    check("wrapper valueOf", &show(&rt, &call_method(&rt, "valueOf", &w, &[])), r#""abc""#, "", "", &mut fails);
    check("wrapper toString", &show(&rt, &call_method(&rt, "toString", &w, &[])), r#""abc""#, "", "", &mut fails);
    check("at on wrapper", &show(&rt, &call_method(&rt, "at", &w, &[Value::Number(-1.0)])), r#""c""#, "", "", &mut fails);
    let empty = rt.construct(&string, &[], &string).expect("new String()");
    check("new String() is empty", &show(&rt, &call_method(&rt, "valueOf", &empty, &[])), r#""""#, "", "", &mut fails);
    let undef = rt.construct(&string, &[Value::Undefined], &string).expect("new String(undefined)");
    check("new String(undefined)", &show(&rt, &call_method(&rt, "valueOf", &undef, &[])), r#""undefined""#, "", "", &mut fails);
    let object_tostring = rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::ObjectPrototype)), "toString").expect("toString");
    check("Object.prototype.toString of a wrapper", &show(&rt, &rt.call(&object_tostring, &w, &[])), r#""[object String]""#, "", "", &mut fails);
    let proto = Value::Object(rt.intrinsic(Intrinsic::StringPrototype));
    check("String.prototype is a String wrapper", &show(&rt, &rt.call(&object_tostring, &proto, &[])), r#""[object String]""#, "", "", &mut fails);
    check("String.prototype.length", &show(&rt, &rt.get_str(&proto, "length")), "0", "", "", &mut fails);
    check("String.prototype.valueOf()", &show(&rt, &call_method(&rt, "valueOf", &proto, &[])), r#""""#, "", "", &mut fails);
    assert!(fails.is_empty(), "wrapper and symbol behaviour differs from V8:\n{}", fails.join("\n"));
}

#[test]
fn string_iterator_yields_code_points() {
    let rt = Rt::new();
    let proto = rt.get_str(&global(&rt, "String"), "prototype").expect("String.prototype");
    let iter_fn = rt.get_v(&proto, &PropKey::Sym(rt.well_known(WellKnown::Iterator))).expect("[Symbol.iterator]");
    let text = Value::String(JsStr::from_units(&[0x61, 0xD83D, 0xDE00, 0xD800]));
    let it = rt.call(&iter_fn, &text, &[]).expect("iterator");
    let next = rt.get_str(&it, "next").expect("next");
    let mut items = Vec::new();
    loop {
        let step = rt.call(&next, &it, &[]).expect("next()");
        if rt.get_str(&step, "done").expect("done").to_boolean() {
            break;
        }
        items.push(show(&rt, &rt.get_str(&step, "value")));
    }
    assert_eq!(items, [r#""a""#, r#""\ud83d\ude00""#, r#""\ud800""#]);
    let object_tostring = rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::ObjectPrototype)), "toString").expect("toString");
    assert_eq!(show(&rt, &rt.call(&object_tostring, &it, &[])), r#""[object String Iterator]""#);
}

#[test]
fn results_survive_collection_between_calls() {
    let rt = Rt::new();
    let parts = call_method(&rt, "split", &Value::str("a,b,c"), &[Value::str(",")]).expect("split");
    let root = rt.root(&parts);
    rt.collect();
    let parts = rt.root_value(root);
    rt.unroot(root);
    assert_eq!(show(&rt, &Ok(parts)), r#"["a","b","c"]"#);
}

#[test]
fn lengths_names_attributes_and_absent_members() {
    let rt = Rt::new();
    let string = global(&rt, "String");
    let proto_value = rt.get_str(&string, "prototype").expect("prototype");
    let proto = proto_value.as_object().expect("object");
    let mut fails = Vec::new();
    for (name, len) in STATIC_LENGTHS {
        let f = rt.get_str(&string, name).expect("static");
        let got_len = rt.get_str(&f, "length").expect("length");
        if got_len != Value::Number(f64::from(*len)) {
            fails.push(format!("String.{name}: length {got_len:?} (V8 {len})"));
        }
    }
    for (name, len) in PROTO_LENGTHS {
        let f = rt.get_str(&proto_value, name).expect("method");
        let got_len = rt.get_str(&f, "length").expect("length");
        let got_name = rt.get_str(&f, "name").expect("name");
        if got_len != Value::Number(f64::from(*len)) || got_name != Value::str(name) {
            fails.push(format!("{name}: length {got_len:?} (V8 {len}), name {got_name:?}"));
        }
        let flags = rt.get_own_property(proto, &PropKey::from_str(name)).expect("own").map(|p| p.flags());
        if flags != Some(Flags::HIDDEN) {
            fails.push(format!("{name}: flags {flags:?}, want HIDDEN"));
        }
    }
    for name in ["localeCompare", "normalize", "match", "matchAll", "search", "toLocaleLowerCase", "toLocaleUpperCase", "trimLeft", "trimRight", "anchor", "big"] {
        if !rt.get_str(&proto_value, name).expect("get").is_undefined() {
            fails.push(format!("String.prototype.{name} must be absent"));
        }
    }
    if !rt.get_str(&string, "raw").expect("get").is_undefined() {
        fails.push("String.raw must be absent".to_string());
    }
    let iter_key = PropKey::Sym(rt.well_known(WellKnown::Iterator));
    let iter_flags = rt.get_own_property(proto, &iter_key).expect("own").map(|p| p.flags());
    if iter_flags != Some(Flags::HIDDEN) {
        fails.push(format!("[Symbol.iterator] flags {iter_flags:?}"));
    }
    let proto_flags = rt.get_own_property(string.as_object().expect("ctor"), &PropKey::from_str("prototype")).expect("own");
    if proto_flags.map(|p| p.flags()) != Some(Flags::NONE) {
        fails.push("String.prototype must be non-writable, non-enumerable, non-configurable".to_string());
    }
    let global_obj = rt.intrinsic(Intrinsic::Global);
    let string_flags = rt.get_own_property(global_obj, &PropKey::from_str("String")).expect("own").map(|p| p.flags());
    if string_flags != Some(Flags::HIDDEN) {
        fails.push(format!("global String flags {string_flags:?}"));
    }
    assert!(fails.is_empty(), "shape differs from the spec or V8:\n{}", fails.join("\n"));
}

#[test]
fn surrogates_and_limits() {
    let rt = Rt::new();
    let mut fails = Vec::new();
    check("repeat past the V8 string limit", &show(&rt, &call_method(&rt, "repeat", &Value::str("ab"), &[Value::Number(268435456.0)])), "throw:RangeError", "", "", &mut fails);
    check("repeat of empty string with a huge count", &show(&rt, &call_method(&rt, "repeat", &Value::str(""), &[Value::Number(9007199254740992.0)])), r#""""#, "", "", &mut fails);
    check("padStart past the V8 string limit", &show(&rt, &call_method(&rt, "padStart", &Value::str("a"), &[Value::Number(536870889.0), Value::str("x")])), "throw:RangeError", "", "", &mut fails);
    check("padStart with an empty fill never throws", &show(&rt, &call_method(&rt, "padStart", &Value::str("a"), &[Value::Number(1073741824.0), Value::str("")])), r#""a""#, "", "", &mut fails);
    check("fromCodePoint at the top of the range", &show(&rt, &call_static(&rt, "fromCodePoint", &[Value::Number(1114111.0)])), r#""\udbff\udfff""#, "", "", &mut fails);
    assert!(fails.is_empty(), "limits differ from V8:\n{}", fails.join("\n"));
}

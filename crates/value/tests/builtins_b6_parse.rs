//! WP-B6: JSON.parse (ECMA-262 25.5.1) on its own: the strict grammar, number and string values, duplicate and
//! `__proto__` keys, key order, the nesting limit of WP-B6 (1000), the dense array limit, and ToString of the text.
//! Expected values are written from the spec and were checked against Node 26 (V8). `show` prints the canonical text
//! that the Node side prints too: numbers as String(n) with `-0` marked, strings with every unit outside printable
//! ASCII as `\uxxxx`, arrays as `[..]` and objects as `{"key":value}` in own-key order. Errors are compared by name.

use vn_value::{Flags, Intrinsic, JsStr, PropKey, Res, Rt, Throw, Value};

fn ok<T>(r: Res<T>) -> T {
    match r {
        Ok(v) => v,
        Err(t) => panic!("unexpected JS throw: {:?}", t.0),
    }
}

fn error_name(rt: &Rt, t: &Throw) -> String {
    match rt.get_str(&t.0, "name") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => String::from("<no name>"),
    }
}

/// A string as the canonical form: printable ASCII except `"` and `\` as is, every other unit as `\uxxxx`.
fn quoted(units: &[u16]) -> String {
    let mut out = String::from("\"");
    for &u in units {
        if (0x20..=0x7E).contains(&u) && u != 0x22 && u != 0x5C {
            out.push(char::from(u as u8));
        } else {
            out.push_str(&format!("\\u{u:04x}"));
        }
    }
    out.push('"');
    out
}

fn show(rt: &Rt, v: &Value) -> String {
    match v {
        Value::Undefined => "undefined".into(),
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) if *n == 0.0 && n.is_sign_negative() => "-0".into(),
        Value::Number(n) => vn_value::number::number_to_string(*n),
        Value::String(s) => quoted(s.units()),
        Value::Object(_) if rt.is_callable(v) => "fn".into(),
        Value::Object(o) if rt.is_array(v) => {
            let len = rt.get_str(v, "length").expect("length");
            let len = rt.to_length(&len).expect("length") as u32;
            let items: Vec<String> = (0..len)
                .map(|i| {
                    let k = PropKey::Index(i);
                    if rt.has_own_property(*o, &k).expect("own") {
                        show(rt, &rt.get(*o, &k, v).expect("get"))
                    } else {
                        "<hole>".into()
                    }
                })
                .collect();
            format!("[{}]", items.join(","))
        }
        Value::Object(o) => {
            let mut parts = Vec::new();
            for k in rt.own_keys(*o).expect("own keys") {
                let name = match &k {
                    PropKey::Index(i) => quoted(JsStr::from_str(&i.to_string()).units()),
                    PropKey::Str(s) => quoted(s.units()),
                    PropKey::Sym(_) => continue,
                };
                let val = rt.get(*o, &k, v).expect("get");
                parts.push(format!("{name}:{}", show(rt, &val)));
            }
            format!("{{{}}}", parts.join(","))
        }
        other => format!("<internal {other:?}>"),
    }
}

/// `JSON.parse` looked up on the realm's JSON object.
fn json_parse(rt: &Rt) -> Value {
    let g = rt.intrinsic(Intrinsic::Global);
    let json = rt.get(g, &PropKey::from_str("JSON"), &Value::Object(g)).expect("JSON");
    rt.get_str(&json, "parse").expect("JSON.parse")
}

/// Calls `JSON.parse(x)` with one argument.
fn parse_value(rt: &Rt, x: Value) -> Res<Value> {
    let g = Value::Object(rt.intrinsic(Intrinsic::Global));
    rt.call(&json_parse(rt), &g, &[x])
}

fn parse_units(rt: &Rt, units: &[u16]) -> Res<Value> {
    parse_value(rt, Value::String(JsStr::from_units(units)))
}

fn parse(rt: &Rt, text: &str) -> Res<Value> {
    let units: Vec<u16> = text.encode_utf16().collect();
    parse_units(rt, &units)
}

#[test]
fn grammar_accepts_strict_json_and_rejects_the_rest() {
    let rt = Rt::new();
    // (text, canonical result) for valid texts; `SyntaxError` marks an invalid one.
    let cases: &[(&str, &str)] = &[
        ("0", "0"),
        ("-0", "-0"),
        ("[]", "[]"),
        (" [1, 2 ,3] ", "[1,2,3]"),
        ("{}", "{}"),
        (r#"{"a":1,"a":2}"#, r#"{"a":2}"#),
        (r#"{"b":1,"a":2}"#, r#"{"b":1,"a":2}"#),
        (r#"{"2":1,"1":2,"0":3}"#, r#"{"0":3,"1":2,"2":1}"#),
        (r#"{"__proto__":{"x":1}}"#, r#"{"__proto__":{"x":1}}"#),
        ("\"\\ud83d\\ude00\"", "\"\\ud83d\\ude00\""),
        ("\"\u{1F600}\"", "\"\\ud83d\\ude00\""),
        ("\"\\u0041\\n\"", "\"A\\u000a\""),
        ("[1e400,-1e400,5e-324]", "[Infinity,-Infinity,5e-324]"),
        ("9007199254740993", "9007199254740992"),
        ("[0.1,1.5e3,-2E-2]", "[0.1,1500,-0.02]"),
        ("\"\u{e9}\"", "\"\\u00e9\""),
        ("null", "null"),
        ("true", "true"),
        ("false", "false"),
        ("\"\\uD800\"", "\"\\ud800\""),
        (r#"{"a":{"b":[true,null]}}"#, r#"{"a":{"b":[true,null]}}"#),
        ("[ ]", "[]"),
        // invalid texts
        ("01", "SyntaxError"),
        ("-", "SyntaxError"),
        ("1.", "SyntaxError"),
        ("1e", "SyntaxError"),
        ("+1", "SyntaxError"),
        ("NaN", "SyntaxError"),
        (".5", "SyntaxError"),
        ("[1,]", "SyntaxError"),
        ("[,1]", "SyntaxError"),
        (r#"{"a":1,}"#, "SyntaxError"),
        ("{a:1}", "SyntaxError"),
        ("'a'", "SyntaxError"),
        ("", "SyntaxError"),
        ("   ", "SyntaxError"),
        (r#""\x41""#, "SyntaxError"),
        ("\"\\u12\"", "SyntaxError"),
        ("\"a\nb\"", "SyntaxError"),
        ("\u{a0}1", "SyntaxError"),
        ("\u{feff}1", "SyntaxError"),
        ("[1]]", "SyntaxError"),
        ("tru", "SyntaxError"),
        ("nullx", "SyntaxError"),
        ("1 2", "SyntaxError"),
        ("1 // c", "SyntaxError"),
    ];
    for &(text, want) in cases {
        let got = match parse(&rt, text) {
            Ok(v) => show(&rt, &v),
            Err(t) => error_name(&rt, &t),
        };
        assert_eq!(got, want, "JSON.parse({text:?})");
    }
}

#[test]
fn numbers_are_correctly_rounded_and_keep_negative_zero() {
    let rt = Rt::new();
    let neg_zero = ok(parse(&rt, "-0"));
    assert!(matches!(neg_zero, Value::Number(n) if n == 0.0 && n.is_sign_negative()));
    let fraction = ok(parse(&rt, "-0.0"));
    assert!(matches!(fraction, Value::Number(n) if n == 0.0 && n.is_sign_negative()));
    assert_eq!(ok(parse(&rt, "5e-324")), Value::Number(f64::from_bits(1)));
    assert_eq!(ok(parse(&rt, "2e-324")), Value::Number(0.0));
    assert_eq!(ok(parse(&rt, "1.7976931348623157e308")), Value::Number(f64::MAX));
    assert_eq!(ok(parse(&rt, "1.7976931348623159e308")), Value::Number(f64::INFINITY));
    let long = ok(parse(&rt, "123456789012345678901234567890"));
    assert_eq!(long, Value::Number(123456789012345678901234567890.0));
}

#[test]
fn strings_keep_pairs_and_lone_surrogates_as_code_units() {
    let rt = Rt::new();
    let pair = ok(parse(&rt, "\"\\uD83D\\uDE00\""));
    assert_eq!(pair, Value::String(JsStr::from_units(&[0xD83D, 0xDE00])));
    let raw_pair = ok(parse(&rt, "\"\u{1F600}\""));
    assert_eq!(raw_pair, Value::String(JsStr::from_units(&[0xD83D, 0xDE00])));
    // A raw lone surrogate in the text is kept, as V8 keeps it.
    let raw = ok(parse_units(&rt, &[0x22, 0xD800, 0x22]));
    assert_eq!(raw, Value::String(JsStr::from_units(&[0xD800])));
    let escapes = ok(parse(&rt, "\"\\u0000\\/\\b\\f\\t\\r\""));
    assert_eq!(escapes, Value::String(JsStr::from_units(&[0, 0x2F, 8, 12, 9, 13])));
}

#[test]
fn a_repeated_key_keeps_its_first_position_and_the_last_value() {
    let rt = Rt::new();
    let v = ok(parse(&rt, r#"{"a":1,"b":2,"a":3}"#));
    assert_eq!(show(&rt, &v), r#"{"a":3,"b":2}"#);
}

#[test]
fn proto_is_an_own_key_and_the_prototype_is_unchanged() {
    let rt = Rt::new();
    let v = ok(parse(&rt, r#"{"__proto__":{"x":1}}"#));
    let Value::Object(o) = v else { panic!("object expected") };
    assert_eq!(rt.get_prototype_of(o), Some(rt.intrinsic(Intrinsic::ObjectPrototype)));
    assert!(rt.has_own_property(o, &PropKey::from_str("__proto__")).expect("own"));
    let inner = ok(rt.get(o, &PropKey::from_str("__proto__"), &v));
    assert_eq!(show(&rt, &inner), r#"{"x":1}"#);
}

#[test]
fn array_index_keys_come_first_in_ascending_order() {
    let rt = Rt::new();
    // 4294967294 is the largest array index; 4294967295 is an ordinary string key.
    let v = ok(parse(&rt, r#"{"b":1,"4294967295":2,"4294967294":3,"1":4,"-1":5}"#));
    let Value::Object(o) = v else { panic!("object expected") };
    let names: Vec<String> = rt
        .own_keys(o)
        .expect("own keys")
        .iter()
        .map(|k| match k {
            PropKey::Index(i) => i.to_string(),
            PropKey::Str(s) => s.to_rust_string_lossy(),
            PropKey::Sym(_) => String::from("<sym>"),
        })
        .collect();
    assert_eq!(names, vec!["1", "4294967294", "b", "4294967295", "-1"]);
}

#[test]
fn nesting_up_to_1000_levels_is_accepted_and_1001_is_a_range_error() {
    let rt = Rt::new();
    let ok_text = format!("{}{}", "[".repeat(1000), "]".repeat(1000));
    let mut depth = 0;
    let mut v = ok(parse(&rt, &ok_text));
    while let Value::Object(o) = v {
        depth += 1;
        let child = ok(rt.get(o, &PropKey::Index(0), &v));
        if !matches!(child, Value::Object(_)) {
            break;
        }
        v = child;
    }
    assert_eq!(depth, 1000);
    let deep = format!("{}{}", "[".repeat(1001), "]".repeat(1001));
    let err = parse(&rt, &deep).expect_err("1001 levels must be rejected");
    assert_eq!(error_name(&rt, &err), "RangeError");
    // Objects count as levels too: 1000 arrays around an object make 1001 levels.
    let mixed = format!("{}{{}}{}", "[".repeat(1000), "]".repeat(1000));
    let err = parse(&rt, &mixed).expect_err("1001 levels with an object must be rejected");
    assert_eq!(error_name(&rt, &err), "RangeError");
}

#[test]
fn an_array_longer_than_the_dense_limit_is_a_range_error() {
    let rt = Rt::new();
    let n = vn_value::MAX_DENSE_LEN;
    let mut text = String::with_capacity(2 * n + 2);
    text.push('[');
    for _ in 0..n {
        text.push_str("0,");
    }
    text.push_str("0]");
    let err = parse(&rt, &text).expect_err("MAX_DENSE_LEN + 1 elements must be rejected");
    assert_eq!(error_name(&rt, &err), "RangeError");
}

#[test]
fn the_text_argument_is_converted_with_tostring() {
    let rt = Rt::new();
    assert_eq!(show(&rt, &ok(parse_value(&rt, Value::Number(1.0)))), "1");
    // ToString(-0) is "0", so the result is +0.
    let zero = ok(parse_value(&rt, Value::Number(-0.0)));
    assert!(matches!(zero, Value::Number(n) if n == 0.0 && !n.is_sign_negative()));
    assert_eq!(show(&rt, &ok(parse_value(&rt, Value::Null))), "null");
    assert_eq!(show(&rt, &ok(parse_value(&rt, Value::Bool(true)))), "true");
    let err = parse_value(&rt, Value::Undefined).expect_err("'undefined' is not JSON");
    assert_eq!(error_name(&rt, &err), "SyntaxError");
    let err = parse_value(&rt, Value::Number(f64::NAN)).expect_err("'NaN' is not JSON");
    assert_eq!(error_name(&rt, &err), "SyntaxError");
    // A plain object converts to "[object Object]" through Object.prototype.toString.
    let obj = rt.new_object();
    let err = parse_value(&rt, Value::Object(obj)).expect_err("'[object Object]' is not JSON");
    assert_eq!(error_name(&rt, &err), "SyntaxError");
    // ToString of a symbol throws.
    let sym = Value::Symbol(rt.new_symbol(Some(JsStr::from_str("s"))));
    let err = parse_value(&rt, sym).expect_err("ToString of a symbol throws");
    assert_eq!(error_name(&rt, &err), "TypeError");
}

#[test]
fn a_to_string_method_decides_the_text() {
    let rt = Rt::new();
    let obj = rt.new_object();
    let to_string = rt.new_native_function(answer_1_2, "toString", 0, Vec::new());
    ok(rt.define_data(obj, PropKey::from_str("toString"), Value::Object(to_string), Flags::ALL));
    assert_eq!(show(&rt, &ok(parse_value(&rt, Value::Object(obj)))), "[1,2]");
}

fn answer_1_2(_rt: &Rt, _c: &vn_value::Call<'_>) -> Res<Value> {
    Ok(Value::str("[1,2]"))
}

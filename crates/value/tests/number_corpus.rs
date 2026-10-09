//! Compares vn-value's number functions with V8 (Node 26). Expected values come from
//! tests/language/corpora/number.json, written by tests/language/gen/gen-corpora.mjs. A JS number is 16
//! hex digits of IEEE-754 bits, a JS string is an array of UTF-16 code units, null is undefined, and
//! {"throw":"RangeError"} is a thrown error. Every case is compared; a failure prints the case and both
//! values.

mod support;

use support::{rows, Checker, Json};
use vn_value::number as n;
use vn_value::JsStr;

const CORPUS: &str = include_str!("../../../tests/language/corpora/number.json");

fn doc() -> Json {
    Json::parse(CORPUS)
}

/// The expected outcome of a call: its string, or the name of the thrown error.
fn expected_result(v: &Json) -> Result<String, String> {
    match v.thrown() {
        Some(name) => Err(name.to_string()),
        None => Ok(v.str().to_string()),
    }
}

fn finish(c: Checker) {
    let k = c.finish().unwrap_or_else(|e| panic!("{e}"));
    assert!(k > 0, "corpus table is empty");
}

fn x_desc(x: f64) -> String {
    format!("{x:?} [{:016x}]", x.to_bits())
}

#[test]
fn to_string_matches_v8() {
    let doc = doc();
    let mut c = Checker::new("toString");
    for (i, row) in rows(&doc, "toString").iter().enumerate() {
        let r = row.arr();
        let x = r[0].f64();
        c.check(i, &x_desc(x), n::number_to_string(x), r[1].str().to_string());
    }
    finish(c);
}

#[test]
fn to_string_with_radix_matches_v8() {
    let doc = doc();
    let mut c = Checker::new("toStringRadix");
    for (i, row) in rows(&doc, "toStringRadix").iter().enumerate() {
        let r = row.arr();
        let (x, radix) = (r[0].f64(), r[1].f64());
        let actual = n::number_to_string_radix_js(x, radix).map_err(|e| e.name().to_string());
        c.check(i, &format!("{} .toString({radix:?})", x_desc(x)), actual, expected_result(&r[2]));
    }
    finish(c);
}

#[test]
fn to_fixed_matches_v8() {
    let doc = doc();
    let mut c = Checker::new("toFixed");
    for (i, row) in rows(&doc, "toFixed").iter().enumerate() {
        let r = row.arr();
        let (x, digits) = (r[0].f64(), r[1].f64());
        let actual = n::to_fixed_js(x, digits).map_err(|e| e.name().to_string());
        c.check(i, &format!("{} .toFixed({digits:?})", x_desc(x)), actual, expected_result(&r[2]));
    }
    finish(c);
}

#[test]
fn to_exponential_matches_v8() {
    let doc = doc();
    let mut c = Checker::new("toExponential");
    for (i, row) in rows(&doc, "toExponential").iter().enumerate() {
        let r = row.arr();
        let x = r[0].f64();
        let digits = if r[1].is_null() { None } else { Some(r[1].f64()) };
        let actual = n::to_exponential_js(x, digits).map_err(|e| e.name().to_string());
        c.check(i, &format!("{} .toExponential({digits:?})", x_desc(x)), actual, expected_result(&r[2]));
    }
    finish(c);
}

#[test]
fn to_precision_matches_v8() {
    let doc = doc();
    let mut c = Checker::new("toPrecision");
    for (i, row) in rows(&doc, "toPrecision").iter().enumerate() {
        let r = row.arr();
        let x = r[0].f64();
        let p = if r[1].is_null() { None } else { Some(r[1].f64()) };
        let actual = n::to_precision_js(x, p).map_err(|e| e.name().to_string());
        c.check(i, &format!("{} .toPrecision({p:?})", x_desc(x)), actual, expected_result(&r[2]));
    }
    finish(c);
}

#[test]
fn string_to_number_matches_v8() {
    let doc = doc();
    let mut c = Checker::new("stringToNumber");
    for (i, row) in rows(&doc, "stringToNumber").iter().enumerate() {
        let r = row.arr();
        let s = JsStr::from_vec(r[0].units());
        c.check_bits(i, &format!("Number({s:?})"), n::string_to_number(&s), r[1].bits());
    }
    finish(c);
}

#[test]
fn parse_float_matches_v8() {
    let doc = doc();
    let mut c = Checker::new("parseFloat");
    for (i, row) in rows(&doc, "parseFloat").iter().enumerate() {
        let r = row.arr();
        let s = JsStr::from_vec(r[0].units());
        c.check_bits(i, &format!("parseFloat({s:?})"), n::parse_float(&s), r[1].bits());
    }
    finish(c);
}

#[test]
fn parse_int_matches_v8() {
    let doc = doc();
    let mut c = Checker::new("parseInt");
    for (i, row) in rows(&doc, "parseInt").iter().enumerate() {
        let r = row.arr();
        let s = JsStr::from_vec(r[0].units());
        let radix = r[1].int() as i32;
        c.check_bits(i, &format!("parseInt({s:?}, {radix})"), n::parse_int(&s, radix), r[2].bits());
    }
    finish(c);
}

#[test]
fn to_int_family_matches_v8() {
    let doc = doc();
    let mut c = Checker::new("toIntegers");
    for (i, row) in rows(&doc, "toIntegers").iter().enumerate() {
        let r = row.arr();
        let x = r[0].f64();
        let want: Vec<i64> = r[1].arr().iter().map(|v| v.int()).collect();
        let got = vec![
            n::to_int32(x) as i64,
            n::to_uint32(x) as i64,
            n::to_int16(x) as i64,
            n::to_uint16(x) as i64,
            n::to_int8(x) as i64,
            n::to_uint8(x) as i64,
        ];
        c.check(i, &format!("{} -> [i32,u32,i16,u16,i8,u8]", x_desc(x)), got, want);
    }
    finish(c);
}

#[test]
fn remainder_matches_v8() {
    let doc = doc();
    let mut c = Checker::new("rem");
    for (i, row) in rows(&doc, "rem").iter().enumerate() {
        let r = row.arr();
        let (a, b) = (r[0].f64(), r[1].f64());
        c.check_bits(i, &format!("{} % {}", x_desc(a), x_desc(b)), n::js_rem(a, b), r[2].bits());
    }
    finish(c);
}

#[test]
fn exponentiation_matches_v8() {
    let doc = doc();
    let mut c = Checker::new("pow");
    for (i, row) in rows(&doc, "pow").iter().enumerate() {
        let r = row.arr();
        let (a, b) = (r[0].f64(), r[1].f64());
        c.check_bits(i, &format!("{} ** {}", x_desc(a), x_desc(b)), n::js_pow(a, b), r[2].bits());
    }
    finish(c);
}

#[test]
fn math_round_matches_v8() {
    let doc = doc();
    let mut c = Checker::new("round");
    for (i, row) in rows(&doc, "round").iter().enumerate() {
        let r = row.arr();
        let x = r[0].f64();
        c.check_bits(i, &format!("Math.round({})", x_desc(x)), n::js_round(x), r[1].bits());
    }
    finish(c);
}

#[test]
fn math_min_matches_v8() {
    let doc = doc();
    let mut c = Checker::new("min");
    for (i, row) in rows(&doc, "min").iter().enumerate() {
        let r = row.arr();
        let args: Vec<f64> = r[0].arr().iter().map(|v| v.f64()).collect();
        c.check_bits(i, &format!("Math.min{args:?}"), n::js_min(&args), r[1].bits());
    }
    finish(c);
}

#[test]
fn math_max_matches_v8() {
    let doc = doc();
    let mut c = Checker::new("max");
    for (i, row) in rows(&doc, "max").iter().enumerate() {
        let r = row.arr();
        let args: Vec<f64> = r[0].arr().iter().map(|v| v.f64()).collect();
        c.check_bits(i, &format!("Math.max{args:?}"), n::js_max(&args), r[1].bits());
    }
    finish(c);
}

#[test]
fn same_value_and_same_value_zero_match_v8() {
    let doc = doc();
    let mut c = Checker::new("sameValue");
    for (i, row) in rows(&doc, "sameValue").iter().enumerate() {
        let r = row.arr();
        let (a, b) = (r[0].f64(), r[1].f64());
        c.check(i, &format!("Object.is({a:?}, {b:?})"), n::same_value(a, b), r[2].bool());
        c.check(i, &format!("SameValueZero({a:?}, {b:?})"), n::same_value_zero(a, b), r[3].bool());
    }
    finish(c);
}

#[test]
fn number_is_integer_matches_v8() {
    let doc = doc();
    let mut c = Checker::new("isInteger");
    for (i, row) in rows(&doc, "isInteger").iter().enumerate() {
        let r = row.arr();
        let x = r[0].f64();
        c.check(i, &format!("Number.isInteger({})", x_desc(x)), n::is_integer_valued(x), r[1].bool());
    }
    finish(c);
}

/// Negative control: the comparer must report a wrong expectation. Real corpus rows are mutated (the
/// expected string gets an extra digit, the expected bits get their sign flipped) and must be reported.
#[test]
fn negative_control_mutated_corpus_rows_are_reported() {
    let doc = doc();
    let first = rows(&doc, "toString")[0].arr().to_vec();
    let x = first[0].f64();
    let good = first[1].str().to_string();
    let mut c = Checker::new("negative control");
    c.check(0, "toString, correct expectation", n::number_to_string(x), good.clone());
    c.check(1, "toString, mutated expectation", n::number_to_string(x), format!("{good}0"));

    // A non-NaN row: a sign flip of a NaN expectation is not observable by design.
    let sample = rows(&doc, "rem")
        .iter()
        .map(|r| r.arr().to_vec())
        .find(|r| !r[2].f64().is_nan())
        .expect("corpus has a non-NaN remainder row");
    let (a, b) = (sample[0].f64(), sample[1].f64());
    let want = sample[2].bits();
    c.check_bits(2, "rem, correct expectation", n::js_rem(a, b), want);
    c.check_bits(3, "rem, sign-flipped expectation", n::js_rem(a, b), want ^ (1u64 << 63));
    // Hand-written wrong values, each checked against the spec.
    c.check_bits(4, "Math.round(2.5) is 3, not 2", n::js_round(2.5), 2.0f64.to_bits());
    c.check(5, "(1.005).toFixed(2) is \"1.00\"", n::to_fixed(1.005, 2).unwrap(), "1.01".to_string());
    c.check_bits(6, "SameValue(0, -0) is false", 0.0, (-0.0f64).to_bits());

    let err = c.finish().expect_err("mutated expectations must be reported");
    assert!(err.contains("5 of 7 cases mismatch"), "{err}");
    for label in [
        "toString, mutated expectation",
        "rem, sign-flipped expectation",
        "Math.round(2.5) is 3",
        "(1.005).toFixed(2)",
        "SameValue(0, -0)",
    ] {
        assert!(err.contains(label), "missing mismatch report for {label}: {err}");
    }
}

/// Positive control for the comparer: correct expectations pass, and NaN payloads are not observable.
#[test]
fn positive_control_passes_on_correct_expectations() {
    let mut c = Checker::new("positive control");
    c.check_bits(0, "NaN payload is not observable", f64::NAN, 0x7ff0_0000_0000_0001);
    c.check_bits(1, "Math.round(2.5)", n::js_round(2.5), 3.0f64.to_bits());
    c.check_bits(2, "-0 is observable", -0.0, (-0.0f64).to_bits());
    assert_eq!(c.finish(), Ok(3));
}

//! Compares vn-value's operators and conversions with V8 (Node 26). Expected values come from
//! tests/language/corpora/ops.json, written by tests/language/gen/gen-ops-corpus.mjs. Operands are the 48 primitives
//! of that table: every ordered pair is checked for the 20 binary operators, and every operand for the unary
//! operators and for ToString, ToNumber and ToInt32. A JS number is 16 hex digits of IEEE-754 bits, a string is an
//! array of UTF-16 code units, null is null, undefined is {"$":"undef"}, and {"throw":"TypeError"} is a thrown error.
//! The negative controls at the end show that a doctored expectation and a mutated operand are both reported.

mod support;

use support::{Checker, Json};
use vn_value::{Res, Rt, Value};

const CORPUS: &str = include_str!("../../../tests/language/corpora/ops.json");

type Binary = fn(&Rt, &Value, &Value) -> Res<Value>;
type Unary = fn(&Rt, &Value) -> Res<Value>;

/// The binary operators of the corpus, in corpus order.
fn binary_table() -> [(&'static str, Binary); 20] {
    [
        ("+", |rt, a, b| rt.add(a, b)),
        ("-", |rt, a, b| rt.sub(a, b)),
        ("*", |rt, a, b| rt.mul(a, b)),
        ("/", |rt, a, b| rt.div(a, b)),
        ("%", |rt, a, b| rt.rem(a, b)),
        ("**", |rt, a, b| rt.pow(a, b)),
        ("&", |rt, a, b| rt.bit_and(a, b)),
        ("|", |rt, a, b| rt.bit_or(a, b)),
        ("^", |rt, a, b| rt.bit_xor(a, b)),
        ("<<", |rt, a, b| rt.shl(a, b)),
        (">>", |rt, a, b| rt.shr(a, b)),
        (">>>", |rt, a, b| rt.ushr(a, b)),
        ("<", |rt, a, b| rt.lt(a, b)),
        (">", |rt, a, b| rt.gt(a, b)),
        ("<=", |rt, a, b| rt.le(a, b)),
        (">=", |rt, a, b| rt.ge(a, b)),
        ("==", |rt, a, b| rt.loose_eq(a, b)),
        ("!=", |rt, a, b| Ok(Value::Bool(!rt.loose_eq(a, b)?.to_boolean()))),
        ("===", |_rt, a, b| Ok(Value::Bool(vn_value::strict_eq(a, b)))),
        ("!==", |_rt, a, b| Ok(Value::Bool(!vn_value::strict_eq(a, b)))),
    ]
}

/// The unary operators of the corpus.
fn unary_table() -> [(&'static str, Unary); 5] {
    [
        ("-", |rt, a| rt.neg(a)),
        ("+", |rt, a| rt.pos(a)),
        ("~", |rt, a| rt.bit_not(a)),
        ("!", |rt, a| Ok(Value::Bool(rt.not_(a)))),
        ("typeof", |rt, a| Ok(Value::str(rt.typeof_(a)))),
    ]
}

/// The conversions of the corpus.
fn conversion_table() -> [(&'static str, Unary); 3] {
    [
        ("ToString", |rt, a| rt.to_string(a).map(Value::String)),
        ("ToNumber", |rt, a| rt.to_number(a).map(Value::Number)),
        ("ToInt32", |rt, a| rt.to_int32(a).map(|i| Value::Number(f64::from(i)))),
    ]
}

/// One comparable key per JavaScript value. Numbers keep their bits, so -0 differs from +0, but every NaN is one
/// key because JavaScript cannot observe NaN payloads.
fn num_key(x: f64) -> String {
    if x.is_nan() { String::from("num NaN") } else { format!("num {x:?} {:016x}", x.to_bits()) }
}

fn str_key(u: &[u16]) -> String {
    format!("str {:?} {:x?}", String::from_utf16_lossy(u), u)
}

fn value_key(v: &Value) -> String {
    match v {
        Value::Undefined => String::from("undefined"),
        Value::Null => String::from("null"),
        Value::Bool(b) => format!("bool {b}"),
        Value::Number(x) => num_key(*x),
        Value::String(s) => str_key(s.units()),
        other => panic!("symbols and objects never appear in the corpus: {other:?}"),
    }
}

fn is_undefined_json(j: &Json) -> bool {
    matches!(j, Json::Obj(kv) if kv.iter().any(|(k, v)| k == "$" && matches!(v, Json::Str(s) if s == "undef")))
}

/// The key of a corpus value, or of a thrown error (`throw TypeError`).
fn expected_key(j: &Json) -> String {
    if let Some(name) = j.thrown() {
        return format!("throw {name}");
    }
    if is_undefined_json(j) {
        return String::from("undefined");
    }
    match j {
        Json::Null => String::from("null"),
        Json::Bool(b) => format!("bool {b}"),
        Json::Str(_) => num_key(j.f64()),
        Json::Arr(_) => str_key(&j.units()),
        other => panic!("unknown corpus encoding: {other:?}"),
    }
}

/// The key of a computed result: a value, or `throw <name>` for a JavaScript throw.
fn result_key(rt: &Rt, r: Res<Value>) -> String {
    match r {
        Ok(v) => value_key(&v),
        Err(t) => {
            let name = match rt.get_str(&t.0, "name") {
                Ok(Value::String(s)) => s.to_rust_string_lossy(),
                _ => String::from("?"),
            };
            format!("throw {name}")
        }
    }
}

/// A corpus operand as a runtime value.
fn operand_value(j: &Json) -> Value {
    if is_undefined_json(j) {
        return Value::Undefined;
    }
    match j {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Str(_) => Value::Number(j.f64()),
        Json::Arr(_) => Value::String(vn_value::JsStr::from_units(&j.units())),
        other => panic!("unknown operand encoding: {other:?}"),
    }
}

fn doc() -> Json {
    Json::parse(CORPUS)
}

/// The operands (name and value) in table order.
fn operands(doc: &Json) -> Vec<(String, Value)> {
    doc.get("operands")
        .arr()
        .iter()
        .map(|o| (o.get("name").str().to_string(), operand_value(o.get("v"))))
        .collect()
}

fn finish(c: Checker, expected: usize) {
    let k = c.finish().unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(k, expected, "cases checked");
}

#[test]
fn binary_operators_match_v8_for_every_ordered_pair() {
    let rt = Rt::new();
    let doc = doc();
    let ops = operands(&doc);
    let n = ops.len();
    assert_eq!(n, 48, "the corpus has 48 operands");
    let mut c = Checker::new("binary operators");
    for (op, f) in binary_table() {
        let table = doc.get("binary").get(op).arr();
        assert_eq!(table.len(), n * n, "binary {op}: every ordered pair appears once");
        for (row, r) in table.iter().enumerate() {
            let r = r.arr();
            let (i, j) = (r[0].int() as usize, r[1].int() as usize);
            assert_eq!(i * n + j, row, "binary {op}: rows are in (left, right) order");
            let (name_a, a) = &ops[i];
            let (name_b, b) = &ops[j];
            c.check(row, &format!("{name_a} {op} {name_b}"), result_key(&rt, f(&rt, a, b)), expected_key(&r[2]));
        }
    }
    finish(c, 20 * n * n);
}

#[test]
fn unary_operators_and_conversions_match_v8_for_every_operand() {
    let rt = Rt::new();
    let doc = doc();
    let ops = operands(&doc);
    let mut c = Checker::new("unary operators and conversions");
    let unary = unary_table().map(|(op, f)| (op, f, "unary"));
    let conv = conversion_table().map(|(op, f)| (op, f, "conversion"));
    for (op, f, kind) in unary.into_iter().chain(conv) {
        let table = doc.get(if kind == "unary" { "unary" } else { "conversions" }).get(op).arr();
        assert_eq!(table.len(), ops.len(), "{kind} {op}: every operand appears once");
        for (row, r) in table.iter().enumerate() {
            let r = r.arr();
            let i = r[0].int() as usize;
            assert_eq!(i, row, "{kind} {op}: rows are in operand order");
            let (name, a) = &ops[i];
            c.check(row, &format!("{op} {name}"), result_key(&rt, f(&rt, a)), expected_key(&r[1]));
        }
    }
    finish(c, 8 * ops.len());
}

#[test]
fn negative_control_a_doctored_expectation_is_reported() {
    let rt = Rt::new();
    // undefined + undefined is NaN in V8; the doctored expectation +0 must be reported, not accepted.
    let mut c = Checker::new("negative control");
    let actual = result_key(&rt, rt.add(&Value::Undefined, &Value::Undefined));
    c.check(0, "undefined + undefined (doctored expectation 0)", actual, num_key(0.0));
    let err = c.finish().expect_err("a doctored expectation must be reported");
    assert!(err.contains("undefined + undefined (doctored expectation 0)"), "{err}");
}

#[test]
fn negative_control_a_mutated_operand_is_reported() {
    let rt = Rt::new();
    let doc = doc();
    let ops = operands(&doc);
    // V8: -0 + -0 is -0. The corpus row (operand 5 is "-0") holds that value; feeding +0 as the left operand
    // gives +0, which must be reported as a mismatch against the corpus.
    let row = &doc.get("binary").get("+").arr()[5 * ops.len() + 5];
    let expected = expected_key(&row.arr()[2]);
    assert_eq!(expected, num_key(-0.0), "the corpus says -0 + -0 is -0");
    let mut c = Checker::new("negative control");
    let mutated = result_key(&rt, rt.add(&Value::Number(0.0), &Value::Number(-0.0)));
    c.check(0, "-0 + -0 with +0 as the left operand", mutated, expected);
    let err = c.finish().expect_err("a mutated operand must be reported");
    assert!(err.contains("-0 + -0 with +0 as the left operand"), "{err}");
}

//! Cross-check against the Node oracle (WP-T1). `tests/language/rust-trace/cross-check.jsonl` was written by
//! cross-check.mjs from the same graphs, recorded by tooling/oracle/trace.mjs. Each graph below builds the same
//! object graph natively, calls the realm's own `trace` global (so the host surface is exercised too), and must
//! produce the same entries text, byte for byte.

mod support;

use std::rc::Rc;

use support::{key, noop, num, ov, s, units, Realm};
use vn_value::builtins::map_set::{map_put, new_map, new_set, set_put};
use vn_value::{Flags, FnKind, Intrinsic, JsStr, NativeObject, PropKey, Res, Rt, Throw, Tracer, Value, WellKnown};

type Graph = fn(&Realm) -> Res<()>;

const FIXTURE: &str = include_str!("../../../tests/language/rust-trace/cross-check.jsonl");

/// Calls a method by name on a value (the graphs use the builtins the way a scenario does).
fn call_method(r: &Realm, this: &Value, name: &str, args: &[Value]) -> Res<Value> {
    let f = r.rt.get_str(this, name)?;
    r.rt.call(&f, this, args)
}

fn map_entries_in_order(r: &Realm) -> Res<()> {
    let m = new_map(&r.rt);
    let inner = r.rt.new_array(vec![num(2.0)]);
    map_put(&r.rt, m, num(1.0), s("a"));
    map_put(&r.rt, m, s("k"), ov(inner));
    map_put(&r.rt, m, num(f64::NAN), Value::Undefined);
    r.t("m", &[ov(m)])
}

fn map_zero_and_identity_keys(r: &Realm) -> Res<()> {
    let o = r.rt.new_object();
    let m = new_map(&r.rt);
    map_put(&r.rt, m, num(-0.0), s("zero"));
    map_put(&r.rt, m, ov(o), num(1.0));
    map_put(&r.rt, m, ov(o), num(2.0));
    r.t("mz", &[ov(m), ov(o)])
}

fn set_values_in_order(r: &Realm) -> Res<()> {
    let set = new_set(&r.rt);
    for v in [num(3.0), s("a"), num(f64::NAN), num(-0.0), num(f64::NAN), num(3.0)] {
        set_put(&r.rt, set, v);
    }
    r.t("s", &[ov(set)])
}

fn collections_of_themselves(r: &Realm) -> Res<()> {
    let m = new_map(&r.rt);
    map_put(&r.rt, m, s("self"), ov(m));
    let set = new_set(&r.rt);
    set_put(&r.rt, set, ov(set));
    r.t("cm", &[ov(m), ov(set)])
}

fn map_after_delete_and_readd(r: &Realm) -> Res<()> {
    let m = new_map(&r.rt);
    map_put(&r.rt, m, num(1.0), s("a"));
    map_put(&r.rt, m, num(2.0), s("b"));
    call_method(r, &ov(m), "delete", &[num(1.0)])?;
    call_method(r, &ov(m), "set", &[num(1.0), s("c")])?;
    r.t("md", &[ov(m)])
}

fn collection_iterators_are_plain_objects(r: &Realm) -> Res<()> {
    let m = new_map(&r.rt);
    map_put(&r.rt, m, num(1.0), num(2.0));
    let keys = call_method(r, &ov(m), "keys", &[])?;
    let set = new_set(&r.rt);
    set_put(&r.rt, set, num(3.0));
    let values = call_method(r, &ov(set), "values", &[])?;
    r.t("it", &[keys, values])
}

fn set_with_own_property(r: &Realm) -> Res<()> {
    let set = new_set(&r.rt);
    set_put(&r.rt, set, num(1.0));
    r.rt.define_data(set, key("extra"), s("x"), Flags::ALL)?;
    r.t("sp", &[ov(set)])
}

/// The graphs, in the order of cross-check.mjs. Each name must match its fixture line.
const GRAPHS: &[(&str, Graph)] = &[
    ("primitives", primitives),
    ("string-escapes", string_escapes),
    ("symbols", symbols),
    ("functions", functions),
    ("function-name-redefined", function_name_redefined),
    ("plain-key-order", plain_key_order),
    ("integer-key-order", integer_key_order),
    ("key-escaping", key_escaping),
    ("accessors-not-invoked", accessors_not_invoked),
    ("arrays-holes", arrays_holes),
    ("array-length-and-extra", array_length_and_extra),
    ("aliases-one-call", aliases_one_call),
    ("cycles", cycles),
    ("shared-across-calls", shared_across_calls),
    ("mutation-between-calls", mutation_between_calls),
    ("ordinals-across-calls", ordinals_across_calls),
    ("proto-kinds", proto_kinds),
    ("errors", errors),
    ("frozen", frozen),
    ("sealed", sealed),
    ("nonextensible", nonextensible),
    ("empty-nonextensible-is-frozen", empty_nonextensible_is_frozen),
    ("extensible-has-no-integrity", extensible_has_no_integrity),
    ("frozen-array", frozen_array),
    ("sealed-array", sealed_array),
    ("nonextensible-array", nonextensible_array),
    ("nonextensible-empty-array", nonextensible_empty_array),
    ("empty-frozen-array", empty_frozen_array),
    ("integrity-nested", integrity_nested),
    ("frozen-error", frozen_error),
    ("numbers-in-arrays", numbers_in_arrays),
    ("untraceable-after-partial", untraceable_after_partial),
    ("label-must-be-string", label_must_be_string),
    ("deep-nesting", deep_nesting),
    ("symbol-keys-and-values", symbol_keys_and_values),
    ("many-values-one-call", many_values_one_call),
    ("map-entries-in-order", map_entries_in_order),
    ("map-zero-and-identity-keys", map_zero_and_identity_keys),
    ("set-values-in-order", set_values_in_order),
    ("collections-of-themselves", collections_of_themselves),
    ("map-after-delete-and-readd", map_after_delete_and_readd),
    ("collection-iterators-are-plain-objects", collection_iterators_are_plain_objects),
    ("set-with-own-property", set_with_own_property),
];

fn primitives(r: &Realm) -> Res<()> {
    r.t(
        "p",
        &[
            Value::Undefined,
            Value::Null,
            Value::Bool(true),
            Value::Bool(false),
            num(0.0),
            num(1.0),
            num(-5.0),
            num(0.1),
            num(1e21),
            num(1e-7),
            num(5e-324),
            num(9007199254740992.0),
            num(1.5e300),
            num(123456789.125),
            num(-0.0),
            num(f64::NAN),
            num(f64::INFINITY),
            num(f64::NEG_INFINITY),
            s("str"),
            s(""),
        ],
    )
}

fn string_escapes(r: &Realm) -> Res<()> {
    r.t(
        "s",
        &[
            s("a\"b\\c"),
            s("\u{8}\u{c}\n\r\t"),
            s("\u{0}\u{1}\u{1f}"),
            s("\u{7f}\u{e9}\u{2028}"),
            s("\u{1F600}"),
            units(&[0xd83d]),
            units(&[0xde00]),
            units(&[0x78, 0xd83d, 0x79]),
        ],
    )
}

fn symbols(r: &Realm) -> Res<()> {
    let a = r.rt.new_symbol(Some(JsStr::from_str("a")));
    let e = r.rt.new_symbol(Some(JsStr::from_str("")));
    let n = r.rt.new_symbol(None);
    let k = r.rt.symbol_for(&JsStr::from_str("k"));
    let iter = r.rt.well_known(WellKnown::Iterator);
    r.t(
        "y",
        &[
            Value::Symbol(a),
            Value::Symbol(e),
            Value::Symbol(a),
            Value::Symbol(n),
            Value::Symbol(iter),
            Value::Symbol(k),
            Value::Symbol(k),
        ],
    )
}

fn functions(r: &Realm) -> Res<()> {
    let named = r.func("named", 3, FnKind::Normal);
    let anon = r.func("anon", 0, FnKind::Arrow);
    let anon_expr = r.func("", 0, FnKind::Normal);
    let m = r.obj(&[("m", ov(named))]);
    let arr = r.rt.new_array(vec![ov(anon_expr)]);
    r.t("f", &[ov(named), ov(anon), ov(named), ov(m), ov(arr)])
}

fn function_name_redefined(r: &Realm) -> Res<()> {
    let f = r.func("f", 0, FnKind::Normal);
    r.rt.define_data(f, key("name"), s("renamed"), Flags::new(false, false, true))?;
    r.t("fn2", &[ov(f)])
}

fn plain_key_order(r: &Realm) -> Res<()> {
    let o = r.rt.new_object();
    let props = [
        ("b", num(1.0)),
        ("a", s("x")),
        ("2", s("two")),
        ("1", s("one")),
        ("-1", s("neg")),
        ("01", s("lead")),
        ("4294967294", s("maxidx")),
        ("4294967295", s("notidx")),
    ];
    for (k, v) in props {
        r.rt.define_data(o, key(k), v, Flags::ALL)?;
    }
    let hidden = r.rt.new_symbol(Some(JsStr::from_str("hidden")));
    r.rt.define_data(o, PropKey::Sym(hidden), num(1.0), Flags::ALL)?;
    r.t("o", &[ov(o)])
}

fn integer_key_order(r: &Realm) -> Res<()> {
    let o = r.obj(&[("10", s("a")), ("9", s("b")), ("1", s("c")), ("x", num(1.0)), ("0", s("z"))]);
    r.t("k", &[ov(o)])
}

fn key_escaping(r: &Realm) -> Res<()> {
    let o = r.rt.new_object();
    r.rt.define_data(o, key("a\"b"), num(1.0), Flags::ALL)?;
    r.rt.define_data(o, key("\\"), num(2.0), Flags::ALL)?;
    r.rt.define_data(o, PropKey::from_jsstr(JsStr::from_units(&[0xd800])), num(3.0), Flags::ALL)?;
    r.rt.define_data(o, key("\u{e9}"), num(4.0), Flags::ALL)?;
    r.t("ke", &[ov(o)])
}

/// A getter that must never run while tracing (it would throw).
fn throwing_getter(rt: &Rt, _c: &vn_value::Call<'_>) -> Res<Value> {
    Err(rt.throw_type_error("getter ran"))
}

fn returns_one(_rt: &Rt, _c: &vn_value::Call<'_>) -> Res<Value> {
    Ok(num(1.0))
}

fn accessors_not_invoked(r: &Realm) -> Res<()> {
    let o = r.rt.new_object();
    let get_x = r.rt.new_native_function(throwing_getter, "get x", 0, Vec::new());
    let set_noop = r.rt.new_native_function(noop, "set", 1, Vec::new());
    let get_z = r.rt.new_native_function(returns_one, "get z", 0, Vec::new());
    let enumerable = Flags::new(false, true, true);
    r.rt.define_accessor(o, key("x"), Some(get_x), None, enumerable)?;
    r.rt.define_accessor(o, key("y"), None, Some(set_noop), enumerable)?;
    r.rt.define_accessor(o, key("z"), Some(get_z), Some(set_noop), enumerable)?;
    r.rt.define_data(o, key("hidden"), num(1.0), Flags::NONE)?;
    r.t("acc", &[ov(o)])
}

fn arrays_holes(r: &Realm) -> Res<()> {
    let a = r.rt.new_array(vec![num(1.0), Value::Hole, s("x")]);
    r.rt.define_data(a, key("tag"), s("T"), Flags::ALL)?;
    let b = r.rt.new_array(vec![]);
    r.rt.define_data(b, PropKey::Index(3), Value::Bool(true), Flags::ALL)?;
    let empty = r.rt.new_array(vec![]);
    r.t("arr", &[ov(a), ov(b), ov(empty)])
}

fn array_length_and_extra(r: &Realm) -> Res<()> {
    let a = r.rt.new_array(vec![num(7.0), num(8.0), num(9.0)]);
    r.rt.set_str(&ov(a), "length", num(1.0))?;
    r.rt.define_data(a, key("-1"), s("neg"), Flags::ALL)?;
    r.rt.define_data(a, key("x"), s("y"), Flags::ALL)?;
    r.rt.define_data(a, PropKey::Index(2), s("two"), Flags::ALL)?;
    r.t("al", &[ov(a)])
}

fn aliases_one_call(r: &Realm) -> Res<()> {
    let o = r.rt.new_object();
    let pair = r.rt.new_array(vec![ov(o), ov(o)]);
    let inner = r.rt.new_array(vec![ov(o)]);
    let holder = r.obj(&[("x", ov(o)), ("y", ov(inner))]);
    r.t("al", &[ov(pair), ov(holder), ov(o)])
}

fn cycles(r: &Realm) -> Res<()> {
    let o = r.rt.new_object();
    r.rt.define_data(o, key("self"), ov(o), Flags::ALL)?;
    let arr = r.rt.new_array(vec![ov(o)]);
    r.rt.define_data(o, key("arr"), ov(arr), Flags::ALL)?;
    r.t("cy", &[ov(o)])
}

fn shared_across_calls(r: &Realm) -> Res<()> {
    let o = r.obj(&[("n", num(1.0))]);
    r.t("s1", &[ov(o)])?;
    r.rt.set_str(&ov(o), "n", num(2.0))?;
    let extra = r.rt.new_array(vec![ov(o)]);
    r.rt.define_data(o, key("extra"), ov(extra), Flags::ALL)?;
    r.t("s2", &[ov(o)])
}

fn mutation_between_calls(r: &Realm) -> Res<()> {
    let o = r.obj(&[("v", num(1.0)), ("w", num(2.0))]);
    r.t("m1", &[ov(o)])?;
    r.rt.set_str(&ov(o), "v", s("changed"))?;
    r.rt.delete(o, &key("w"))?;
    r.rt.set_str(&ov(o), "z", Value::Null)?;
    r.t("m2", &[ov(o)])
}

fn ordinals_across_calls(r: &Realm) -> Res<()> {
    let a = r.obj(&[]);
    let b = r.obj(&[]);
    r.t("one", &[ov(a)])?;
    r.t("two", &[ov(b), ov(a)])?;
    let c = r.rt.new_array(vec![ov(b)]);
    r.t("three", &[ov(c), ov(a)])
}

fn proto_kinds(r: &Realm) -> Res<()> {
    let inh = r.obj(&[("inh", num(1.0))]);
    let p1 = r.rt.new_object_with_proto(None);
    let p2 = r.rt.new_object_with_proto(Some(inh));
    let p3 = r.rt.new_object_with_proto(Some(r.rt.intrinsic(Intrinsic::ArrayPrototype)));
    let p4 = r.rt.new_object_with_proto(Some(r.rt.intrinsic(Intrinsic::ObjectPrototype)));
    let p5 = r.obj(&[("length", num(2.0))]);
    r.t("pk", &[ov(p1), ov(p2), ov(p3), ov(p4), ov(p5)])
}

fn errors(r: &Realm) -> Res<()> {
    let e1 = r.construct(Intrinsic::TypeErrorCtor, &[s("bad")])?;
    let e2 = r.construct(Intrinsic::ErrorCtor, &[])?;
    r.rt.set_str(&e2, "code", num(42.0))?;
    r.rt.set_str(&e2, "name", s("Custom"))?;
    let e3 = ov(r.rt.new_object_with_proto(Some(r.rt.intrinsic(Intrinsic::ErrorPrototype))));
    let e4 = r.construct(Intrinsic::RangeErrorCtor, &[s("r")])?;
    r.t("err", &[e1, e2, e3, e4])
}

fn frozen(r: &Realm) -> Res<()> {
    let o = r.obj(&[("a", num(1.0))]);
    r.rt.freeze(o)?;
    r.t("fz", &[ov(o)])
}

fn sealed(r: &Realm) -> Res<()> {
    let o = r.obj(&[("a", num(1.0))]);
    r.rt.seal(o)?;
    r.t("sl", &[ov(o)])
}

fn nonextensible(r: &Realm) -> Res<()> {
    let o = r.obj(&[("a", num(1.0))]);
    r.rt.prevent_extensions(o);
    r.t("ne", &[ov(o)])
}

fn empty_nonextensible_is_frozen(r: &Realm) -> Res<()> {
    let o = r.obj(&[]);
    r.rt.prevent_extensions(o);
    r.t("ep", &[ov(o)])
}

fn extensible_has_no_integrity(r: &Realm) -> Res<()> {
    let o = r.obj(&[("a", num(1.0))]);
    r.t("ex", &[ov(o)])
}

fn frozen_array(r: &Realm) -> Res<()> {
    let a = r.rt.new_array(vec![num(1.0), Value::Hole, num(3.0)]);
    r.rt.freeze(a)?;
    r.t("fa", &[ov(a)])
}

fn sealed_array(r: &Realm) -> Res<()> {
    let a = r.rt.new_array(vec![num(1.0), num(2.0)]);
    r.rt.seal(a)?;
    r.t("sa", &[ov(a)])
}

fn nonextensible_array(r: &Realm) -> Res<()> {
    let a = r.rt.new_array(vec![num(1.0)]);
    r.rt.prevent_extensions(a);
    r.t("na", &[ov(a)])
}

fn nonextensible_empty_array(r: &Realm) -> Res<()> {
    let a = r.rt.new_array(vec![]);
    r.rt.prevent_extensions(a);
    r.t("nea", &[ov(a)])
}

fn empty_frozen_array(r: &Realm) -> Res<()> {
    let a = r.rt.new_array(vec![]);
    r.rt.freeze(a)?;
    r.t("efa", &[ov(a)])
}

fn integrity_nested(r: &Realm) -> Res<()> {
    let inner = r.obj(&[("x", num(1.0))]);
    let list = r.rt.new_array(vec![]);
    r.rt.seal(list)?;
    let outer = r.obj(&[("inner", ov(inner)), ("list", ov(list))]);
    r.rt.freeze(outer)?;
    r.t("in", &[ov(outer)])
}

fn frozen_error(r: &Realm) -> Res<()> {
    let e = r.construct(Intrinsic::ErrorCtor, &[s("e")])?;
    let o = e.as_object().expect("an error object");
    r.rt.freeze(o)?;
    r.t("fe", &[e])
}

fn numbers_in_arrays(r: &Realm) -> Res<()> {
    let a = r.rt.new_array(vec![num(f64::NAN), num(-0.0), num(1e21), num(0.000001)]);
    r.t("n", &[ov(a)])
}

/// A host payload (a stand-in for a Date): the encoder must refuse it.
struct Opaque;

impl NativeObject for Opaque {
    fn trace(&self, _t: &mut Tracer) {}
}

fn untraceable_after_partial(r: &Realm) -> Res<()> {
    let a = r.obj(&[("a", num(1.0))]);
    let date = r.rt.new_native_object(None, Rc::new(Opaque));
    if let Err(Throw(e)) = r.t("bad", &[ov(a), ov(date)]) {
        let message = r.rt.get_str(&e, "message")?;
        r.t("caught", &[message])?;
    }
    let after = r.obj(&[]);
    r.t("after", &[ov(after)])
}

fn label_must_be_string(r: &Realm) -> Res<()> {
    if let Err(Throw(e)) = r.call_trace(&[num(1.0), num(2.0)]) {
        let message = r.rt.get_str(&e, "message")?;
        r.t("caught", &[message])?;
    }
    r.t("ok", &[s("x")])
}

fn deep_nesting(r: &Realm) -> Res<()> {
    let root = r.rt.new_object();
    let mut cur = root;
    for i in 0..6 {
        let item = r.obj(&[("i", num(f64::from(i)))]);
        let arr = r.rt.new_array(vec![ov(item)]);
        r.rt.define_data(cur, key("next"), ov(arr), Flags::ALL)?;
        cur = item;
    }
    r.t("deep", &[ov(root)])
}

fn symbol_keys_and_values(r: &Realm) -> Res<()> {
    let v = r.rt.new_symbol(Some(JsStr::from_str("v")));
    let o = r.rt.new_object();
    r.rt.define_data(o, PropKey::Sym(r.rt.well_known(WellKnown::Iterator)), num(1.0), Flags::ALL)?;
    r.rt.define_data(o, key("a"), Value::Symbol(v), Flags::ALL)?;
    r.rt.define_data(o, key("b"), Value::Symbol(v), Flags::ALL)?;
    r.t("sv", &[ov(o)])
}

fn many_values_one_call(r: &Realm) -> Res<()> {
    let xs: Vec<_> = (0..5).map(|i| r.obj(&[("i", num(f64::from(i)))])).collect();
    let arr = r.rt.new_array(xs.iter().map(|o| ov(*o)).collect());
    let mut values: Vec<Value> = xs.iter().map(|o| ov(*o)).collect();
    values.push(ov(arr));
    r.t("many", &values)
}

/// Builds one graph in a fresh realm and returns the entries text it recorded.
fn run_graph(build: Graph) -> String {
    let r = Realm::new();
    build(&r).expect("the graph builds and traces");
    r.entries_text()
}

fn graph(name: &str) -> Graph {
    GRAPHS.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("no Rust graph named {name}")).1
}

/// Splits each `{"name":"<name>","entries":<entries>}` line. Names are plain ASCII, so the fixed prefix and
/// suffix are cut off, and the entries text is returned exactly as the Node oracle wrote it.
fn parse_fixture(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let rest = line.strip_prefix(r#"{"name":""#).expect("fixture line starts with the name key");
            let (name, rest) = rest.split_once('"').expect("the name is terminated");
            let entries = rest.strip_prefix(r#","entries":"#).expect("the entries key follows the name");
            let entries = entries.strip_suffix('}').expect("the line ends with the object's closing brace");
            (name.to_string(), entries.to_string())
        })
        .collect()
}

#[test]
fn rust_encoder_matches_the_node_oracle_on_every_graph() {
    let fixture = parse_fixture(FIXTURE);
    assert!(fixture.len() >= 25, "the cross-check needs at least 25 graphs, the fixture has {}", fixture.len());
    assert_eq!(fixture.len(), GRAPHS.len(), "the fixture and the Rust graphs list different counts");
    let mut differing = Vec::new();
    for (name, expected) in &fixture {
        let actual = run_graph(graph(name));
        if let Some(report) = support::first_difference(expected, &actual) {
            differing.push(format!("{name}: {report}"));
        }
    }
    assert!(differing.is_empty(), "{} of {} graphs differ from the oracle:\n{}", differing.len(), fixture.len(), differing.join("\n"));
    println!("cross-check: {} graphs match the Node oracle", fixture.len());
}

#[test]
fn negative_control_a_doctored_expectation_is_reported() {
    let fixture = parse_fixture(FIXTURE);
    let (_, expected) = fixture.iter().find(|(n, _)| n == "frozen").expect("the frozen graph is in the fixture");
    let actual = run_graph(graph("frozen"));
    assert!(support::first_difference(expected, &actual).is_none(), "the real expectation must hold");
    let doctored = expected.replacen("\"frozen\"", "\"sealed\"", 1);
    assert_ne!(&doctored, expected, "the doctoring must change the text");
    let report = support::first_difference(&doctored, &actual).expect("a doctored expectation must be reported");
    assert!(report.contains("sealed") && report.contains("frozen"), "the report shows both texts: {report}");
}

#[test]
fn negative_control_a_mutated_input_is_reported() {
    let fixture = parse_fixture(FIXTURE);
    let (_, expected) = fixture.iter().find(|(n, _)| n == "frozen").expect("the frozen graph is in the fixture");
    // The same object without freeze: the encoder must not invent `integrity`.
    let actual = run_graph(graph("extensible-has-no-integrity"));
    assert!(support::first_difference(expected, &actual).is_some(), "a missing integrity field must be reported");
}

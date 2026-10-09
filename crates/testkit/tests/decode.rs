//! The decoder (WP-T1): `encode(decode(E)) == E` for the first trace call of every cross-check graph (E is the
//! Node oracle's text), for hand-written encodings, and the inputs it must refuse. A negative control shows that
//! dropping a field from an encoding is detected by the round trip.

mod support;

use std::cell::RefCell;

use vn_testkit::json::Json;
use vn_testkit::json_in;
use vn_testkit::{decode_pass, encode_pass, IdTable, Recorder};
use vn_value::{Rt, Throw};

const FIXTURE: &str = include_str!("../../../tests/language/rust-trace/cross-check.jsonl");

fn message(rt: &Rt, e: &Throw) -> String {
    match rt.get_str(&e.0, "message") {
        Ok(vn_value::Value::String(s)) => s.to_rust_string_lossy(),
        _ => "<error>".to_string(),
    }
}

/// Decodes one encoding pass and encodes it back in one pass.
fn round_trip(list: &[Json]) -> Result<Vec<Json>, String> {
    let rt = Rt::new();
    let mut ids = IdTable::new();
    let values = decode_pass(&rt, list, &mut ids).map_err(|e| message(&rt, &e))?;
    let rec = RefCell::new(Recorder::new());
    encode_pass(&rt, &rec, &values).map_err(|e| message(&rt, &e))
}

fn parse(text: &str) -> Vec<Json> {
    json_in::parse(text).expect("the encoding is JSON").items().expect("a list of values").to_vec()
}

/// Graphs the decoder refuses by design, with the reason it must give. An accessor marker cannot be decoded (the
/// getter is not part of the encoding), and a plain object whose prototype is neither Object.prototype nor null
/// cannot be rebuilt from `proto: "other"`.
const REFUSED: &[(&str, &str)] = &[
    ("accessors-not-invoked", "accessor markers"),
    ("proto-kinds", "unsupported prototype"),
    // Iterator objects encode as plain objects with another prototype; the decoder never rebuilds them.
    ("collection-iterators-are-plain-objects", "unsupported prototype"),
    // A promise's nonextensible integrity depends on its own (untraced) properties, so an empty rebuild cannot reproduce it.
    ("promise-nonextensible", "depends on its own properties"),
];

#[test]
fn the_first_call_of_every_cross_check_graph_round_trips() {
    let mut round_tripped = 0;
    let mut failing = Vec::new();
    for line in FIXTURE.lines().filter(|l| !l.is_empty()) {
        let case = json_in::parse(line).expect("fixture line parses");
        let name = case.field("name").and_then(Json::units).map(String::from_utf16_lossy).expect("a name");
        let entries = case.field("entries").and_then(Json::items).expect("entries");
        let first = entries[0].field("values").and_then(Json::items).expect("values of the first call").to_vec();
        if let Some((_, reason)) = REFUSED.iter().find(|(n, _)| *n == name) {
            match round_trip(&first) {
                Err(message) if message.contains(reason) => {}
                other => failing.push(format!("{name}: expected a refusal mentioning {reason}, got {other:?}")),
            }
            continue;
        }
        match round_trip(&first) {
            Ok(back) if Json::Arr(back.clone()) == Json::Arr(first.clone()) => round_tripped += 1,
            Ok(back) => failing.push(format!("{name}: re-encoded as {}", Json::Arr(back).to_json())),
            Err(reason) => failing.push(format!("{name}: {reason}")),
        }
    }
    assert!(round_tripped >= 30, "at least 30 graphs round-trip (found {round_tripped})");
    assert!(failing.is_empty(), "{} graphs do not round-trip:\n{}", failing.len(), failing.join("\n"));
}

#[test]
fn hand_written_encodings_round_trip() {
    let cases = [
        r#"[{"$":"obj","id":1,"cls":"Array","proto":"other","integrity":"sealed","props":[["tag","T"]],"elems":[1,{"$":"hole"},"x"],"len":3}]"#,
        r#"[{"$":"obj","id":1,"cls":"Object","proto":"null","props":[["1","one"],["b",{"$":"num","v":"-0"}]]}]"#,
        r#"[{"$":"sym","id":1,"desc":"d"},{"$":"sym","id":2,"desc":null},{"$":"sym","id":1,"desc":"d"}]"#,
        r#"[{"$":"fn","name":"named","length":3},{"$":"fn","name":"","length":0}]"#,
        r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[["self",{"$":"ref","id":1}]]}]"#,
        r#"[{"$":"obj","id":1,"cls":"Error","proto":"other","integrity":"frozen","props":[],"name":"RangeError","message":"r"}]"#,
        r#"["\ud83d","x\ud800y",1e+21,0.1,{"$":"num","v":"NaN"},{"$":"num","v":"-Infinity"},-5]"#,
        r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","integrity":"frozen","props":[]}]"#,
    ];
    for text in cases {
        let original = parse(text);
        let back = round_trip(&original).unwrap_or_else(|e| panic!("{text}: {e}"));
        assert_eq!(Json::Arr(back), Json::Arr(original), "{text}");
    }
}

#[test]
fn inputs_the_decoder_must_refuse_are_errors() {
    let refused = [
        r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[["x",{"$":"accessor","get":true,"set":false}]]}]"#,
        r#"[{"$":"obj","id":1,"cls":"Promise","proto":"other","integrity":"sealed","props":[]}]"#,
        r#"[{"$":"ref","id":1}]"#,
        r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[]},{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[]}]"#,
        r#"[{"$":"weird"}]"#,
        r#"[{"$":"obj","id":1,"cls":"Object","proto":"other","props":[]}]"#,
        r#"[{"$":"obj","id":1,"cls":"Array","proto":"other","props":[],"elems":[1],"len":2}]"#,
        r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","integrity":"melted","props":[]}]"#,
    ];
    for text in refused {
        let list = parse(text);
        assert!(round_trip(&list).is_err(), "must refuse: {text}");
    }
}

#[test]
fn negative_control_dropping_integrity_is_detected_by_the_round_trip() {
    let original = parse(r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","integrity":"frozen","props":[["a",1]]}]"#);
    let doctored = parse(r#"[{"$":"obj","id":1,"cls":"Object","proto":"Object.prototype","props":[["a",1]]}]"#);
    assert_eq!(Json::Arr(round_trip(&original).expect("round trips")), Json::Arr(original.clone()));
    let back = round_trip(&doctored).expect("the doctored encoding still decodes");
    assert_eq!(Json::Arr(back.clone()), Json::Arr(doctored), "the encoder reports what was decoded");
    assert_ne!(Json::Arr(back), Json::Arr(original), "a missing integrity field is a difference");
}

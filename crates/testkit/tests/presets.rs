//! Native callback presets against the Node oracle (WP-T1, builtin corpora). tests/language/rust-trace/presets.jsonl
//! was written by presets-check.mjs from the generator's own presets (tooling/oracle/builtin-corpus.mjs). Each line
//! is replayed here: the inputs are decoded from `pre`, the presets are called in order, and the state is encoded
//! and compared with the oracle's, including the callback log and the names of thrown errors.

mod support;

use std::cell::RefCell;

use vn_testkit::json::Json;
use vn_testkit::json_in;
use vn_testkit::presets::{install_presets, PRESETS};
use vn_testkit::{decode_pass, encode_pass, IdTable, Recorder};
use vn_value::{Rt, Throw, Value};

const FIXTURE: &str = include_str!("../../../tests/language/rust-trace/presets.jsonl");

fn text_of(v: Option<&Json>) -> String {
    v.and_then(Json::units).map(String::from_utf16_lossy).expect("a string field")
}

fn items_of(v: Option<&Json>) -> Vec<Json> {
    v.and_then(Json::items).expect("an array field").to_vec()
}

/// The `name` of a thrown value as the oracle records it: its string `name`, else null.
fn name_of(rt: &Rt, e: &Value) -> Json {
    match e {
        Value::Object(_) => match rt.get_str(e, "name") {
            Ok(Value::String(s)) => Json::Str(s.units().to_vec()),
            _ => Json::Null,
        },
        _ => Json::Null,
    }
}

/// Replays one fixture line. Returns the (state, throws) the native presets produce.
fn replay(line: &str) -> (Vec<Json>, Vec<Json>, Vec<Json>, String) {
    let case = json_in::parse(line).expect("the fixture line is JSON");
    let name = text_of(case.field("preset"));
    let arity: Vec<usize> = items_of(case.field("arity"))
        .iter()
        .map(|n| n.num_text().and_then(|t| t.parse().ok()).expect("an arity"))
        .collect();
    let pre = items_of(case.field("pre"));
    let post = case.field("post").expect("a post object");
    let want_throws = items_of(post.field("throws"));
    let want_state = items_of(post.field("state"));

    let rt = Rt::new();
    let log = rt.new_array(Vec::new());
    let presets = install_presets(&rt, log);
    let mut ids = IdTable::new();
    for (preset, f) in &presets {
        let length = PRESETS.iter().find(|(n, _)| n == preset).map(|(_, l)| *l).expect("a known preset");
        ids.register_function(preset, length, f.clone());
    }
    let flat = decode_pass(&rt, &pre, &mut ids).expect("the inputs decode");
    let f = presets.iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone()).expect("the preset exists");

    let mut rets = Vec::new();
    let mut throws = Vec::new();
    let mut at = 0;
    for n in arity {
        let args = flat[at..at + n].to_vec();
        at += n;
        match rt.call(&f, &Value::Undefined, &args) {
            Ok(v) => {
                rets.push(v);
                throws.push(Json::Null);
            }
            Err(Throw(e)) => {
                rets.push(Value::Null);
                throws.push(name_of(&rt, &e));
            }
        }
    }
    let mut list = rets;
    list.extend(flat.iter().cloned());
    list.push(Value::Object(log));
    let rec = RefCell::new(Recorder::new());
    let state = encode_pass(&rt, &rec, &list).expect("the state encodes");
    (state, throws, want_throws, format!("{name}: want {} state entries", want_state.len()))
}

#[test]
fn every_preset_sequence_matches_the_oracle() {
    let mut sequences = 0;
    let mut failures = Vec::new();
    for line in FIXTURE.lines().filter(|l| !l.is_empty()) {
        let case = json_in::parse(line).expect("fixture line parses");
        let name = text_of(case.field("preset"));
        let want_state = items_of(case.field("post").and_then(|p| p.field("state")));
        let (state, throws, want_throws, _) = replay(line);
        if Json::Arr(state.clone()) != Json::Arr(want_state.clone()) {
            failures.push(format!("{name}: state\n  oracle: {}\n  native: {}", Json::Arr(want_state).to_json(), Json::Arr(state).to_json()));
        }
        if Json::Arr(throws.clone()) != Json::Arr(want_throws.clone()) {
            failures.push(format!("{name}: throws {} vs oracle {}", Json::Arr(throws).to_json(), Json::Arr(want_throws).to_json()));
        }
        sequences += 1;
    }
    assert!(sequences >= 15, "every preset has a sequence (found {sequences})");
    assert!(failures.is_empty(), "{} preset sequences differ from the oracle:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn negative_control_a_changed_log_entry_is_detected() {
    let line = FIXTURE.lines().find(|l| l.contains("\"preset\":\"cmpStr\"")).expect("a cmpStr line");
    let (state, _, _, _) = replay(line);
    let want = items_of(json_in::parse(line).expect("parses").field("post").and_then(|p| p.field("state")));
    assert_eq!(Json::Arr(state.clone()), Json::Arr(want.clone()), "the real line matches");
    let mut doctored = want.clone();
    let last = doctored.len() - 1;
    doctored.swap(0, last);
    assert_ne!(Json::Arr(state), Json::Arr(doctored), "a reordered state is a difference");
}

//! Shared test code: the corpus reader, the JSON reader and the replay engine.
#![allow(dead_code)]

pub mod json;
pub mod replay;

use json::Json;
use replay::{parse_case, Case};

/// The corpus file for one group (tests/dom/corpora/<group>.json).
pub fn load_corpus(group: &str) -> Vec<Case> {
    let path = format!("{}/../../tests/dom/corpora/{group}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let root = Json::parse(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    root.as_arr()
        .unwrap_or_else(|| panic!("{path}: not an array"))
        .iter()
        .map(|c| parse_case(c).unwrap_or_else(|e| panic!("{path}: {e}")))
        .collect()
}

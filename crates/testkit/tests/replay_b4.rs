//! Corpus replay for WP-B4 (docs/dev/builtin-corpus.md): the Map, Set and Map.groupBy suites written by
//! tests/language/gen/gen-b4.mjs. Every expected value is produced by V8 (Node 26) when a suite is compiled. Each suite must
//! replay with zero failures and zero missing targets. The negative control corrupts one expectation of a copy of the Map
//! suite and requires exactly that case to be reported.

use vn_testkit::{replay_suite, ReplayOptions};

const MAP: &str = include_str!("../../../tests/language/builtins/map.json");
const SET: &str = include_str!("../../../tests/language/builtins/set.json");
const GROUPBY: &str = include_str!("../../../tests/language/builtins/groupby.json");

/// Replays one suite and prints every failure and missing target. Returns the number of cases.
fn replay(name: &str, text: &str) -> usize {
    let report = replay_suite(text, &ReplayOptions::default());
    for f in report.failures.iter().take(64) {
        eprintln!("{name}/{}: {}\n  expected {}\n  actual   {}", f.id, f.reason, f.expected, f.actual);
    }
    for m in report.missing.iter().take(64) {
        eprintln!("{name}/{}: missing target {}", m.id, m.target);
    }
    eprintln!(
        "{name}: {} cases, {} passed, {} failed, {} missing",
        report.total,
        report.passed,
        report.failures.len(),
        report.missing.len()
    );
    assert!(report.is_clean(), "{name}: the replay is not clean");
    report.total
}

#[test]
fn map_suite_replays_clean() {
    replay("map", MAP);
}

#[test]
fn set_suite_replays_clean() {
    replay("set", SET);
}

#[test]
fn groupby_suite_replays_clean() {
    replay("groupby", GROUPBY);
}

#[test]
fn suites_have_at_least_250_cases() {
    let total: usize = [MAP, SET, GROUPBY].iter().map(|text| replay_suite(text, &ReplayOptions::default()).total).sum();
    assert!(total >= 250, "only {total} cases");
}

#[test]
fn negative_control_reports_a_corrupted_expectation() {
    // get/key-one looks up the key 1 in a map whose value for it is 6 (its position in the key list): post.ret is 6.
    // Changing that expectation to 60 must add exactly one failure, get/key-one, to the failures of the untouched suite.
    let start = MAP.find("{\"id\":\"get/key-one\"").expect("case get/key-one is in map.json");
    let end = start + MAP[start..].find('\n').expect("the case ends its line");
    let line = &MAP[start..end];
    assert_eq!(line.matches("\"ret\":6,").count(), 1, "the expectation to corrupt is unique");
    let corrupted = format!("{}{}{}", &MAP[..start], line.replacen("\"ret\":6,", "\"ret\":60,", 1), &MAP[end..]);
    let baseline = replay_suite(MAP, &ReplayOptions::default());
    let report = replay_suite(&corrupted, &ReplayOptions::default());
    let ids = |r: &vn_testkit::ReplayReport| r.failures.iter().map(|f| f.id.clone()).collect::<Vec<_>>();
    let mut expected = ids(&baseline);
    expected.push("get/key-one".to_string());
    assert_eq!(ids(&report), expected, "the corruption is the only new failure");
    assert_eq!(report.missing.len(), baseline.missing.len(), "no new missing targets");
    assert_eq!(report.total, baseline.total);
}

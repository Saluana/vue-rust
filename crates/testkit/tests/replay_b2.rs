//! WP-B2 corpus replay: the Array suites in tests/language/builtins/array-*.json, generated from V8 by
//! tests/language/gen/gen-b2.mjs. Every case must replay with zero failures and zero missing targets. The negative
//! control corrupts one expectation in a copy of a suite and requires exactly that case to be reported.

use vn_testkit::{replay_suite, ReplayOptions};

const STATICS: &str = include_str!("../../../tests/language/builtins/array-statics.json");
const METHODS: &str = include_str!("../../../tests/language/builtins/array-methods.json");
const CALLBACKS: &str = include_str!("../../../tests/language/builtins/array-callbacks.json");

fn replay(name: &str, text: &str) -> vn_testkit::ReplayReport {
    let report = replay_suite(text, &ReplayOptions::default());
    for f in report.failures.iter().take(15) {
        eprintln!("{name} FAIL {}: {} expected={} actual={}", f.id, f.reason, f.expected, f.actual);
    }
    for m in report.missing.iter().take(15) {
        eprintln!("{name} MISSING {}: {}", m.id, m.target);
    }
    println!("{name}: total {} passed {} failures {} missing {}", report.total, report.passed, report.failures.len(), report.missing.len());
    report
}

#[test]
fn array_statics_replay_clean() {
    let r = replay("array-statics", STATICS);
    assert!(r.missing.is_empty(), "array-statics: {} missing targets", r.missing.len());
    assert!(r.failures.is_empty(), "array-statics: {} failures", r.failures.len());
    assert_eq!(r.passed, r.total);
}

#[test]
fn array_methods_replay_clean() {
    let r = replay("array-methods", METHODS);
    assert!(r.missing.is_empty(), "array-methods: {} missing targets", r.missing.len());
    assert!(r.failures.is_empty(), "array-methods: {} failures", r.failures.len());
    assert_eq!(r.passed, r.total);
}

#[test]
fn array_callbacks_replay_clean() {
    let r = replay("array-callbacks", CALLBACKS);
    assert!(r.missing.is_empty(), "array-callbacks: {} missing targets", r.missing.len());
    assert!(r.failures.is_empty(), "array-callbacks: {} failures", r.failures.len());
    assert_eq!(r.passed, r.total);
}

/// Negative control: flip the expected result of `is-array/01` (Array.isArray([1]) is true) in a copy of the statics
/// suite. The replay must report exactly that case and nothing else.
#[test]
fn negative_control_corrupted_expectation_is_reported() {
    let line = STATICS
        .lines()
        .find(|l| l.contains("\"id\":\"is-array/01\""))
        .expect("the case is-array/01 exists");
    assert!(line.contains("\"ret\":true"), "the expectation to corrupt is present");
    let corrupted_line = line.replacen("\"ret\":true", "\"ret\":false", 1);
    let corrupted = STATICS.replacen(line, &corrupted_line, 1);
    let r = replay_suite(&corrupted, &ReplayOptions::default());
    assert_eq!(r.failures.len(), 1, "exactly the corrupted case fails");
    assert_eq!(r.failures[0].id, "is-array/01");
    assert!(r.missing.is_empty());
}

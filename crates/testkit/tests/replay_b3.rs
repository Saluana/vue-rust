//! WP-B3 corpus replay: the String, Number and Math suites (tests/language/builtins/{string-*,number-proto,math-extra}.json)
//! and O2's math.json and number.json, which the B3 implementation must also pass. Every case runs against V8's result
//! (the suites were compiled by tests/language/gen/gen-b3.mjs and tooling/oracle/builtin-corpus.mjs).

use vn_testkit::replay::replay_suite_approx_ulps;
use vn_testkit::{ReplayOptions, ReplayReport};

/// Approx numbers may differ from V8 by this many ulps: one, the tolerance of `replay_suite`. The `tan` cases of
/// math.json are exact (see `tan_cases_match_v8_bit_for_bit`).
const APPROX_ULPS: u32 = 1;

/// The suites this WP generates (tests/language/gen/gen-b3.mjs).
const OWN_SUITES: &[(&str, &str)] = &[
    ("string-proto", include_str!("../../../tests/language/builtins/string-proto.json")),
    ("string-replace", include_str!("../../../tests/language/builtins/string-replace.json")),
    ("string-statics", include_str!("../../../tests/language/builtins/string-statics.json")),
    ("number-proto", include_str!("../../../tests/language/builtins/number-proto.json")),
    ("math-extra", include_str!("../../../tests/language/builtins/math-extra.json")),
];

/// O2's seed suites, which this WP must also pass (not generated here; see the report for their current status).
const O2_SUITES: &[(&str, &str)] = &[
    ("math", include_str!("../../../tests/language/builtins/math.json")),
    ("number", include_str!("../../../tests/language/builtins/number.json")),
];

fn report_text(report: &ReplayReport) -> String {
    let mut out = format!(
        "suite {}: {} of {} passed, {} failed, {} missing\n",
        report.suite,
        report.passed,
        report.total,
        report.failures.len(),
        report.missing.len()
    );
    for f in report.failures.iter().take(12) {
        out.push_str(&format!("  FAIL {}: {}\n    expected {}\n    actual   {}\n", f.id, f.reason, f.expected, f.actual));
    }
    for m in report.missing.iter().take(12) {
        out.push_str(&format!("  MISSING {} ({})\n", m.id, m.target));
    }
    out
}

fn replay_all(suites: &[(&str, &str)]) -> String {
    let mut problems = String::new();
    for (name, text) in suites {
        let report = replay_suite_approx_ulps(text, &ReplayOptions::default(), APPROX_ULPS);
        eprint!("{}", report_text(&report));
        if !report.is_clean() {
            problems.push_str(&format!("{name}: not clean\n"));
        }
        assert!(report.total > 0, "{name} has no cases");
    }
    problems
}

#[test]
fn b3_generated_suites_replay_with_no_failures_and_no_missing_targets() {
    let problems = replay_all(OWN_SUITES);
    assert!(problems.is_empty(), "replay problems:\n{problems}");
}

/// The O2 seed suites (math.json and number.json) must replay with no failures and no missing targets.
#[test]
fn o2_math_and_number_suites_replay_with_no_failures() {
    let problems = replay_all(O2_SUITES);
    assert!(problems.is_empty(), "O2 replay problems:\n{problems}");
}

/// Negative control: a copy of a suite with one expectation changed must be reported as a failure, so a clean
/// result above means something.
#[test]
fn a_corrupted_expectation_is_reported() {
    let text = OWN_SUITES[2].1;
    let marker = "\"post\":{\"ret\":";
    assert!(text.contains(marker), "the marker must exist in the suite text");
    let corrupted = text.replacen(marker, "\"post\":{\"ret\":\"CORRUPTED\",\"was\":", 1);
    let report = replay_suite_approx_ulps(&corrupted, &ReplayOptions::default(), APPROX_ULPS);
    assert_eq!(report.failures.len(), 1, "exactly the corrupted case must fail:\n{}", report_text(&report));
    assert!(!report.is_clean());
}

/// Negative control for a missing target: a case that names a method the realm does not have is reported missing.
#[test]
fn an_absent_method_is_reported_missing() {
    let text = OWN_SUITES[2].1;
    let renamed = text.replacen("\"target\":\"String.fromCharCode\"", "\"target\":\"String.raw\"", 1);
    let report = replay_suite_approx_ulps(&renamed, &ReplayOptions::default(), APPROX_ULPS);
    assert_eq!(report.missing.len(), 1, "{}", report_text(&report));
    assert!(report.failures.is_empty(), "{}", report_text(&report));
}

/// Math.tan is bit-exact with V8: every `tan` case of math.json passes with no tolerance. The two string-argument cases
/// (tan/arg-str5 and tan/arg-strws) were two ulps from V8 under the platform libm, which is why the tolerance was two.
#[test]
fn tan_cases_match_v8_bit_for_bit() {
    let text = O2_SUITES[0].1;
    for id in ["tan/arg-str5", "tan/arg-strws"] {
        assert!(text.contains(&format!("\"id\":\"{id}\"")), "{id} is not a case of math.json");
    }
    let report = replay_suite_approx_ulps(text, &ReplayOptions::default(), 0);
    let failing: Vec<&str> =
        report.failures.iter().map(|f| f.id.as_str()).filter(|id| id.starts_with("tan/")).collect();
    let missing: Vec<&str> =
        report.missing.iter().map(|m| m.id.as_str()).filter(|id| id.starts_with("tan/")).collect();
    assert!(
        failing.is_empty() && missing.is_empty(),
        "tan failures {failing:?}, missing {missing:?}\n{}",
        report_text(&report)
    );
}

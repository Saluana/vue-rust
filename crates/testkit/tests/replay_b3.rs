//! WP-B3 corpus replay: the String, Number and Math suites (tests/language/builtins/{string-*,number-proto,math-extra}.json)
//! and O2's math.json and number.json, which the B3 implementation must also pass. Every case runs against V8's result
//! (the suites were compiled by tests/language/gen/gen-b3.mjs and tooling/oracle/builtin-corpus.mjs).

use vn_testkit::replay::replay_suite_approx_ulps;
use vn_testkit::{ReplayOptions, ReplayReport};

/// Approx numbers may differ from V8 by this many ulps (libm versus V8's ieee754 port; the two `tan` cases of math.json
/// are two ulps away, see `tan_cases_need_the_two_ulp_tolerance`).
const APPROX_ULPS: u32 = 2;

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

/// Not green until the Array family provides Array.prototype.toString: 32 cases pass arrays, whose ToString needs it.
/// Left failing on purpose.
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

/// The two `tan` cases of math.json are about two ulps from V8: they fail with a one-ulp tolerance and pass with two.
#[test]
fn tan_cases_need_the_two_ulp_tolerance() {
    let failing = |ulps: u32| -> Vec<String> {
        replay_suite_approx_ulps(O2_SUITES[0].1, &ReplayOptions::default(), ulps).failures.into_iter().map(|f| f.id).collect()
    };
    let one = failing(1);
    assert!(one.iter().any(|id| id == "tan/arg-str5") && one.iter().any(|id| id == "tan/arg-strws"), "{one:?}");
    let two = failing(2);
    // tan/arg-arr (an array argument) fails at both tolerances for the Array reason; only the two scalar cases are checked.
    assert!(!two.iter().any(|id| id == "tan/arg-str5" || id == "tan/arg-strws"), "{two:?}");
}

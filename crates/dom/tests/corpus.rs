//! Replays the Chrome corpora (tests/dom/corpora) on vn-dom. Each group is one test; a failing case reports the op index
//! of the first divergence. The negative controls check that the comparator does detect a doctored recording.

mod support;

use support::load_corpus;
use support::replay::{replay, Action, Case};

fn run_group(group: &str) -> usize {
    let cases = load_corpus(group);
    assert!(!cases.is_empty(), "{group}: the corpus has no cases");
    let failures: Vec<_> = cases.iter().filter_map(|c| replay(c).err()).collect();
    if let Some(first) = failures.first() {
        panic!(
            "{} of {} {group} cases diverge from Chrome. First: case {} at op {} {}\n  expected {}\n  actual   {}",
            failures.len(),
            cases.len(),
            first.case,
            first.op_index,
            first.op,
            first.expected,
            first.actual
        );
    }
    cases.len()
}

#[test]
fn corpus_tree() {
    run_group("tree");
}

#[test]
fn corpus_attr() {
    run_group("attr");
}

#[test]
fn corpus_prop() {
    run_group("prop");
}

#[test]
fn corpus_form() {
    run_group("form");
}

#[test]
fn corpus_style() {
    run_group("style");
}

#[test]
fn corpus_class() {
    run_group("class");
}

#[test]
fn corpus_event() {
    run_group("event");
}

#[test]
fn corpus_focus() {
    run_group("focus");
}

#[test]
fn corpus_serial() {
    run_group("serial");
}

#[test]
fn corpus_query() {
    run_group("query");
}

#[test]
fn corpus_activation() {
    run_group("activation");
}

#[test]
fn corpus_total_is_about_250_cases() {
    let total: usize = ["tree", "attr", "prop", "form", "style", "class", "event", "focus", "serial", "query", "activation"]
        .iter()
        .map(|g| load_corpus(g).len())
        .sum();
    assert!(total >= 250, "only {total} cases in the corpora");
}

fn case_named(group: &str, name: &str) -> Case {
    load_corpus(group).into_iter().find(|c| c.name == name).unwrap_or_else(|| panic!("no case {name}"))
}

/// Negative control 1: a recorded result changed in the corpus must be reported at exactly that op.
#[test]
fn negative_control_doctored_expectation_is_reported_at_its_op() {
    let mut case = case_named("tree", "tree/create-html-lowercases");
    // Op 2 is tagName of the element: Chrome says "DIV". Doctor it to "div".
    assert_eq!(case.steps[2].get("r").and_then(|r| r.as_str()), Some("DIV"), "fixture changed");
    let doctored = {
        let mut steps = case.steps.clone();
        let mut kv = steps[2].clone();
        if let support::json::Json::Obj(fields) = &mut kv {
            for (k, v) in fields.iter_mut() {
                if k == "r" {
                    *v = support::json::Json::Str("div".into());
                }
            }
        }
        steps[2] = kv;
        steps
    };
    case.steps = doctored;
    let d = replay(&case).expect_err("a doctored expectation must not replay cleanly");
    assert_eq!(d.op_index, 2, "the divergence must be reported at the doctored op: {d:?}");
    assert!(d.expected.contains("div") && d.actual.contains("DIV"), "{d:?}");
}

/// Negative control 2: a doctored op argument changes the outcome, which must be reported at the first op it affects.
#[test]
fn negative_control_doctored_op_is_reported() {
    let mut case = case_named("tree", "tree/create-html-lowercases");
    // Create "P" instead of "DIV": localName (op 1) then no longer matches the recording "div".
    case.ops[0] = support::json::Json::Arr(vec![
        support::json::Json::Str("el".into()),
        support::json::Json::Str("html".into()),
        support::json::Json::Str("P".into()),
    ]);
    let d = replay(&case).expect_err("a doctored op must not replay cleanly");
    assert_eq!(d.op_index, 1, "{d:?}");
}

/// Negative control 3: an op the replay does not know is a harness error, reported as a divergence, never a pass.
#[test]
fn negative_control_unknown_op_is_not_silently_accepted() {
    let mut case = case_named("tree", "tree/create-html-lowercases");
    case.ops[1] = support::json::Json::Arr(vec![support::json::Json::Str("noSuchOp".into())]);
    let d = replay(&case).expect_err("an unknown op must be reported");
    assert_eq!(d.op_index, 1);
    assert!(d.actual.contains("harness error"), "{d:?}");
}

/// Negative control 4: a listener behaviour removed from the recording's case changes the event log at that op.
#[test]
fn negative_control_missing_listener_behaviour_is_reported() {
    let mut case = case_named("event", "event/path-capture-target-bubble");
    case.listeners.insert(3, vec![Action::Stop]);
    let d = replay(&case).expect_err("a changed listener must change the log");
    assert!(d.op_index > 0, "{d:?}");
}

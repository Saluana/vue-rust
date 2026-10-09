//! The patchProp corpus (tests/ui/runtime-d/corpora/patchprop.json, recorded from vue@3.5.43 in headless Chrome) replayed
//! through `DocHost` and `dom_props::patch_prop`, plus negative controls and the innerHTML error path (WP-RT-D).

mod rtd_common;

use rtd_common::{load_corpus, replay, Json};

fn count_field(header: &Json, field: &str) -> usize {
    match header.get("counts").and_then(|c| c.get(field)) {
        Some(Json::Num(n)) => *n as usize,
        other => panic!("header counts.{field} missing: {other:?}"),
    }
}

fn case_named<'a>(cases: &'a [Json], name: &str) -> &'a Json {
    cases.iter().find(|c| c.get("name").and_then(Json::as_str) == Some(name)).unwrap_or_else(|| panic!("no case {name}"))
}

/// Replaces field `key` of an object value in place.
fn set_field(j: &mut Json, key: &str, value: Json) {
    match j {
        Json::Obj(fields) => {
            for (k, v) in fields.iter_mut() {
                if k == key {
                    *v = value.clone();
                }
            }
        }
        _ => panic!("not an object"),
    }
}

fn result_mut(case: &mut Json, index: usize) -> &mut Json {
    match case {
        Json::Obj(fields) => {
            let (_, results) = fields.iter_mut().find(|(k, _)| k == "results").expect("results");
            match results {
                Json::Arr(items) => &mut items[index],
                _ => panic!("results is not an array"),
            }
        }
        _ => panic!("case is not an object"),
    }
}

fn step_mut(case: &mut Json, index: usize) -> &mut Json {
    match case {
        Json::Obj(fields) => {
            let (_, steps) = fields.iter_mut().find(|(k, _)| k == "steps").expect("steps");
            match steps {
                Json::Arr(items) => &mut items[index],
                _ => panic!("steps is not an array"),
            }
        }
        _ => panic!("case is not an object"),
    }
}

#[test]
fn patchprop_corpus_replays_identically_to_chrome() {
    let (header, cases) = load_corpus("patchprop.json");
    assert_eq!(cases.len(), count_field(&header, "cases"), "header case count");
    assert!(count_field(&header, "transitions") >= 500, "the corpus must hold at least 500 patchProp transitions");
    let mut checked = 0;
    let mut diverging = Vec::new();
    for case in &cases {
        match replay(case) {
            Ok(n) => checked += n,
            Err(m) => diverging.push(m),
        }
    }
    let listing: Vec<String> = diverging
        .iter()
        .take(12)
        .map(|m| format!("{} step {} ({}): expected {} got {}", m.case, m.step, m.op, m.expected, m.actual))
        .collect();
    assert!(diverging.is_empty(), "{} of {} cases diverge from Chrome:\n{}", diverging.len(), cases.len(), listing.join("\n"));
    eprintln!(
        "patchprop: {} cases, {} transitions, {} recorded results compared, 0 mismatches",
        cases.len(),
        count_field(&header, "transitions"),
        checked
    );
}

/// Negative control 1: a snapshot expectation doctored in the recorded results must be reported at exactly that step.
#[test]
fn negative_control_doctored_snapshot_is_reported_at_its_step() {
    let (_header, cases) = load_corpus("patchprop.json");
    let mut case = case_named(&cases, "class/div/null->\"a b\"").clone();
    // step 5 is the second snap: the class is "a b" there in Chrome; claim it is empty instead
    *result_mut(&mut case, 5) = Json::Str("{\"$\":\"el\",\"tag\":\"div\",\"attrs\":[],\"children\":[]}".to_string());
    let m = replay(&case).expect_err("a doctored expectation must not replay cleanly");
    assert_eq!(m.step, 5, "{m:?}");
    assert_eq!(m.op, "snap");
    assert!(m.actual.contains("a b"), "the replay reports its own snapshot: {m:?}");
}

/// Negative control 2: a doctored transition argument changes the outcome, reported at the first snap it affects.
#[test]
fn negative_control_mutated_transition_is_reported() {
    let (_header, cases) = load_corpus("patchprop.json");
    let mut case = case_named(&cases, "class/div/null->\"a b\"").clone();
    // patch step 4 moves "a b" to "x y": the snapshot at step 5 must now disagree with Chrome's "c d"-free record
    let mut step = step_mut(&mut case, 4).clone();
    set_field(&mut step, "next", Json::Str("x y".to_string()));
    *step_mut(&mut case, 4) = step;
    let m = replay(&case).expect_err("a changed transition must change the snapshot");
    assert_eq!(m.step, 5, "{m:?}");
    assert!(m.actual.contains("x y"), "{m:?}");
}

/// Negative control 3: an op the replay does not know is a harness error, reported as a divergence, never a pass.
#[test]
fn negative_control_unknown_op_is_not_silently_accepted() {
    let (_header, cases) = load_corpus("patchprop.json");
    let mut case = case_named(&cases, "class/div/null->\"a b\"").clone();
    *step_mut(&mut case, 1) = Json::Obj(vec![("op".to_string(), Json::Str("noSuchOp".to_string()))]);
    let m = replay(&case).expect_err("an unknown op must be reported");
    assert_eq!(m.step, 1);
    assert!(m.actual.contains("harness error"), "{m:?}");
}

/// Negative control 4: a recorded handler log that Chrome did not produce must be reported at the log step.
#[test]
fn negative_control_recorded_handler_log_is_checked() {
    let (_header, cases) = load_corpus("patchprop.json");
    let mut case = case_named(&cases, "event/once").clone();
    // steps: create, create, mount, mount, patch, dispatch, log (6), dispatch, log (8)
    let mut doctored = result_mut(&mut case, 6).clone();
    let Json::Obj(fields) = &mut doctored else { panic!("log result") };
    let (_, log) = fields.iter_mut().find(|(k, _)| k == "log").expect("log field");
    *log = Json::Arr(Vec::new());
    *result_mut(&mut case, 6) = doctored;
    let m = replay(&case).expect_err("a missing handler call must be reported");
    assert_eq!(m.step, 6, "{m:?}");
    assert_eq!(m.op, "log");
}

/// The innerHTML setter has no native implementation: patchDOMProp reports a TypeError with a clear message.
#[test]
fn innerhtml_assignment_is_a_type_error_on_the_native_host() {
    use vn_runtime::dom_events::DomCtx;
    use vn_runtime::dom_props::patch_prop;
    use vn_runtime::host::{Host, HostNs};
    use vn_runtime::host_doc::DocHost;
    use vn_value::{JsStr, Rt, Value};

    let rt = Rt::new();
    let mut host = DocHost::new();
    let div = host.create_element(HostNs::Html, "div", None).expect("div");
    let ctx = DomCtx::new(&rt, &host);
    let next = Value::String(JsStr::from_str("<b>x</b>"));
    let err = patch_prop(&ctx, div, "innerHTML", &Value::Null, &next, HostNs::Html).expect_err("innerHTML must throw");
    let message = match rt.get_str(&err.0, "message") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        other => panic!("no message: {other:?}"),
    };
    assert!(message.contains("innerHTML is not supported"), "{message}");
    assert_eq!(host.inner_html(div), Some(String::new()), "nothing was parsed or inserted");
    // null assigns nothing (patchDOMProp skips null innerHTML)
    patch_prop(&ctx, div, "innerHTML", &Value::Null, &Value::Null, HostNs::Html).expect("null innerHTML is a no-op");
}

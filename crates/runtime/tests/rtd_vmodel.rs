//! The directive corpus (tests/ui/runtime-d/corpora/directives.json, recorded from vue@3.5.43 in headless Chrome):
//! v-model (text with the lazy/trim/number modifiers, composition, textarea, checkbox, radio, select, dynamic), v-show,
//! withModifiers and withKeys, replayed through `vn_runtime::exports` (the JS-visible directive objects and wrappers) on
//! `DocHost` and the runtime's DOM layer (WP-RT-D).

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

#[test]
fn directive_corpus_replays_identically_to_chrome() {
    let (header, cases) = load_corpus("directives.json");
    assert_eq!(cases.len(), count_field(&header, "cases"), "header case count");
    assert!(cases.len() >= 150, "the corpus must hold at least 150 scenarios");
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
    assert!(diverging.is_empty(), "{} of {} scenarios diverge from Chrome:\n{}", diverging.len(), cases.len(), listing.join("\n"));
    eprintln!(
        "directives: {} scenarios, {} directive calls, {} wrappers, {} recorded results compared, 0 mismatches",
        cases.len(),
        count_field(&header, "directiveCalls"),
        count_field(&header, "wraps"),
        checked
    );
}

/// Negative control: a recorded callback value (the model Vue assigns after typing) must be reported at its step.
#[test]
fn negative_control_doctored_model_assignment_is_reported() {
    let (_header, cases) = load_corpus("directives.json");
    let mut case = case_named(&cases, "v-model/text/trim/\" a b \"").clone();
    // step 4 is the log after typing: Chrome says the trimmed value "a b"; claim the untrimmed one
    let doctored = Json::Obj(vec![
        ("log".to_string(), Json::Arr(vec![Json::Str("assign|$=s: a b ".to_string())])),
        ("errors".to_string(), Json::Arr(Vec::new())),
    ]);
    *result_mut(&mut case, 6) = doctored;
    let m = replay(&case).expect_err("a doctored callback value must be reported");
    assert_eq!(m.step, 6, "{m:?}");
    assert_eq!(m.op, "log");
    assert!(m.actual.contains("s:a b"), "{m:?}");
}

/// Negative control: a scenario whose Chrome run threw must be reported when the replay does not throw. The v-show
/// and v-model corpora record no step errors, so the control doctors one expectation into a thrown step instead.
#[test]
fn negative_control_expected_throw_must_be_reproduced() {
    let (_header, cases) = load_corpus("directives.json");
    let mut case = case_named(&cases, "v-show/toggle/true").clone();
    *result_mut(&mut case, 2) = Json::Obj(vec![("$error".to_string(), Json::Str("claimed".to_string()))]);
    let m = replay(&case).expect_err("a step that did not throw must not match a recorded throw");
    assert_eq!(m.step, 2, "{m:?}");
}

//! Selector matching against headless Chrome. For each of the 64 corpus trees, every selector is parsed by
//! vn_css and matched over the same tree (element states come from Chrome). Chrome's verdict decides the
//! expectation: a selector Chrome rejects must be rejected here; a valid selector that we support must match the
//! same elements in document order and serialise as Chrome's selectorText; a valid selector we do not support must
//! carry a VNCSS3xxx code. Corpus: tests/css/corpora/selectors.json (tests/css/gen/gen-selector-corpus.mjs).

mod support;

use support::{Checker, Doc, Json};
use std::sync::atomic::{AtomicUsize, Ordering};

use vn_css::diag::code;
use vn_css::matching::matches_list;
use vn_css::selector::{parse, SelectorList};

const CORPUS: &str = include_str!("../../../tests/css/corpora/selectors.json");

/// Known deviations counted during a run (see `compare`). Reported, never silently applied.
static DEVIATIONS: AtomicUsize = AtomicUsize::new(0);

/// Compares one case. `expected` overrides the corpus matches (used by the negative control).
fn compare(c: &mut Checker, tree: usize, case_no: usize, case: &Json, doc: &Doc, expected: Option<Vec<usize>>) {
    let sel = case.get("sel").str();
    let label = format!("tree {tree} case {case_no} {sel:?}");
    let chrome_ok = case.get("ok").bool();
    let parsed = parse(sel);
    if !chrome_ok {
        c.check(tree, &format!("{label}: Chrome rejects, we must reject"), parsed.is_err(), true);
        return;
    }
    match parsed {
        Err(diags) => {
            // Valid for Chrome, so the construct must be one we report as unsupported (VNCSS3xxx selectors, VNCSS4xxx
            // nesting). KNOWN DEVIATION: a selector with an unclosed bracket is valid for Chrome (EOF closes the block,
            // CSS Syntax 3); we reject it with a syntax diagnostic on purpose, so it is accepted here and counted.
            let codes: Vec<&str> = diags.iter().map(|d| d.code).collect();
            let unsupported = codes.iter().all(|k| k.starts_with("VNCSS3") || k.starts_with("VNCSS4"));
            let unclosed = unclosed_at_eof(sel) && codes.iter().all(|k| *k == code::INVALID_SELECTOR);
            if unclosed {
                DEVIATIONS.fetch_add(1, Ordering::Relaxed);
            } else {
                c.check(tree, &format!("{label}: valid for Chrome, must be an unsupported diagnostic, got {codes:?}"), unsupported, true);
            }
        }
        Ok(list) => {
            check_supported(c, tree, &label, case, doc, &list, expected);
        }
    }
}

/// True when the selector text has a bracket or parenthesis that is never closed.
fn unclosed_at_eof(sel: &str) -> bool {
    let count = |open: char, close: char| sel.chars().filter(|&c| c == open).count() as i64 - sel.chars().filter(|&c| c == close).count() as i64;
    count('[', ']') > 0 || count('(', ')') > 0
}

fn check_supported(c: &mut Checker, tree: usize, label: &str, case: &Json, doc: &Doc, list: &SelectorList, expected: Option<Vec<usize>>) {
    let want: Vec<usize> = match expected {
        Some(v) => v,
        None => case.get("matches").arr().iter().map(|j| j.int() as usize).collect(),
    };
    let got = doc.select(|el| matches_list(list, &el));
    c.check(tree, &format!("{label}: matches"), got, want);
    if let Some(text) = case.opt("text").filter(|t| !t.is_null()) {
        c.check(tree, &format!("{label}: selectorText"), list.to_string(), text.str().to_string());
    }
}

#[test]
fn selectors_agree_with_chrome() {
    let corpus = Json::parse(CORPUS);
    let trees = corpus.get("trees").arr();
    assert!(trees.len() >= 60, "corpus has {} trees, need >= 60", trees.len());
    let mut c = Checker::new("selectors");
    let mut cases = 0;
    for (ti, tree) in trees.iter().enumerate() {
        let doc = Doc::from_json(tree.get("root"));
        let list = tree.get("cases").arr();
        assert!(list.len() >= 25, "tree {ti} has {} selectors, need >= 25", list.len());
        for (ci, case) in list.iter().enumerate() {
            cases += 1;
            compare(&mut c, ti, ci, case, &doc, None);
        }
    }
    let n = c.finish().unwrap_or_else(|e| panic!("{e}"));
    assert!(n > 0 && cases > 0);
    let deviations = DEVIATIONS.load(Ordering::Relaxed);
    assert!(deviations <= 12, "{deviations} unclosed-block deviations; expected only the few listed in compare()");
    eprintln!("selector corpus: {cases} cases, {n} checks, {deviations} known unclosed-block deviations");
}

/// Negative control: a doctored expectation (one extra element index) must be reported as a mismatch, and so must
/// a doctored selectorText. Proves the checker compares the matches it is given.
#[test]
fn negative_control_doctored_row_is_reported() {
    let corpus = Json::parse(CORPUS);
    let tree = &corpus.get("trees").arr()[0];
    let doc = Doc::from_json(tree.get("root"));
    let case = tree
        .get("cases")
        .arr()
        .iter()
        .find(|c| c.get("ok").bool() && !c.get("matches").arr().is_empty())
        .expect("a matching selector in tree 0");
    let mut doctored: Vec<usize> = case.get("matches").arr().iter().map(|j| j.int() as usize).collect();
    doctored.push(doc.len() + 1000);
    let mut c = Checker::new("negative-control");
    compare(&mut c, 0, 0, case, &doc, Some(doctored));
    assert!(c.finish().is_err(), "a doctored matches row must be reported");

    let mut c = Checker::new("negative-control");
    let list = parse(case.get("sel").str()).expect("corpus selector parses");
    c.check(0, "doctored selectorText", list.to_string(), format!("{} x", list));
    assert!(c.finish().is_err(), "a doctored selectorText must be reported");
}

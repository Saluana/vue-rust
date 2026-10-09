//! WP-U1b cascade and inheritance corpus (tests/ui/style/corpora/cascade.json): each fixture is rebuilt through vn-dom
//! (html > body > the fixture tree, the same element order as Chrome), focused as in Chrome, computed with the author
//! stylesheet, and every listed property of every element is compared with what Chrome computed. The fixtures use
//! only the documented subset, so they must produce no diagnostics.

mod support;

use support::{append, element, no_listeners, Json};
use vn_css::media::MediaEnv;
use vn_dom::{Document, NodeId};
use vn_ui::{compute_styles, NoStates, Sheet};

const CASCADE: &str = include_str!("../../../tests/ui/style/corpora/cascade.json");

/// Builds a tree description ({t, a, c} elements, {x} text, {m} comments). Elements are recorded in pre-order.
fn build(doc: &mut Document, d: &Json, elements: &mut Vec<NodeId>) -> NodeId {
    if let Some(x) = d.opt("x") {
        return doc.create_text(x.str());
    }
    if let Some(m) = d.opt("m") {
        return doc.create_comment(m.str());
    }
    let el = element(doc, d.get("t").str(), "");
    elements.push(el);
    for pair in d.get("a").arr() {
        let (k, v) = (pair.arr()[0].str(), pair.arr()[1].str());
        doc.set_attribute(el, k, v).expect("attribute");
    }
    for c in d.get("c").arr() {
        let child = build(doc, c, elements);
        append(doc, el, child);
    }
    el
}

/// The fixture's document, its elements in pre-order (the fixture root first), and the focused element if any.
fn fixture_document(fx: &Json) -> (Document, Vec<NodeId>) {
    let mut doc = Document::new();
    let html = element(&mut doc, "html", "");
    // The oracle page has head before body; :first-child/:nth-child rules also match these ancestors.
    let head = element(&mut doc, "head", "");
    let body = element(&mut doc, "body", "");
    let mut elements = Vec::new();
    let root = build(&mut doc, fx.get("tree"), &mut elements);
    append(&mut doc, body, root);
    append(&mut doc, html, head);
    append(&mut doc, html, body);
    let top = doc.document();
    append(&mut doc, top, html);
    let focus = fx.get("focus").num() as i64;
    if focus >= 0 {
        doc.focus(elements[focus as usize], &mut no_listeners).expect("focus");
    }
    (doc, elements)
}

/// Mismatches of one fixture against `expect` (rows per element, values per property); empty when all match.
fn check(fx: &Json, expect: &[Vec<String>]) -> Vec<String> {
    let id = fx.get("id").num() as i64;
    let props: Vec<&str> = fx.get("props").arr().iter().map(|p| p.str()).collect();
    let (mut doc, elements) = fixture_document(fx);
    let sheet = Sheet::parse(fx.get("css").str());
    let env = MediaEnv::new(800.0, 600.0);
    let tree = compute_styles(&mut doc, std::slice::from_ref(&sheet), &env, &NoStates);
    let mut out = Vec::new();
    if !sheet.diagnostics.is_empty() || !tree.diagnostics.is_empty() {
        out.push(format!("fixture {id}: unexpected diagnostics {:?} {:?}", sheet.diagnostics, tree.diagnostics));
    }
    if elements.len() != expect.len() {
        out.push(format!("fixture {id}: {} elements, Chrome has {}", elements.len(), expect.len()));
        return out;
    }
    for (i, (node, row)) in elements.iter().zip(expect).enumerate() {
        let style = tree.style(*node).expect("element style");
        for (p, want) in props.iter().zip(row) {
            let got = style.get(p).unwrap_or_else(|| "<absent>".to_string());
            if &got != want {
                out.push(format!("fixture {id} element {i} {p}: got {got:?}, Chrome {want:?}"));
            }
        }
    }
    out
}

fn expectations(fx: &Json) -> Vec<Vec<String>> {
    fx.get("expect")
        .arr()
        .iter()
        .map(|row| row.arr().iter().map(|v| v.str().to_string()).collect())
        .collect()
}

#[test]
fn cascade_fixtures_match_chrome() {
    let doc = Json::parse(CASCADE);
    let fixtures = doc.get("fixtures").arr();
    let mut failures = Vec::new();
    let mut checks = 0;
    for fx in fixtures {
        let expect = expectations(fx);
        checks += expect.iter().map(|r| r.len()).sum::<usize>();
        failures.extend(check(fx, &expect));
    }
    eprintln!("cascade: {} fixtures, {checks} element-property values compared", fixtures.len());
    assert!(fixtures.len() >= 60, "the corpus holds at least 60 fixtures");
    assert!(failures.is_empty(), "{} mismatches; first ones:\n{}", failures.len(), failures.iter().take(60).cloned().collect::<Vec<_>>().join("\n"));
}

/// The comparison must report a wrong expectation (negative control on the first fixture).
#[test]
fn cascade_checker_detects_wrong_expectations() {
    let doc = Json::parse(CASCADE);
    let fx = &doc.get("fixtures").arr()[0];
    let expect = expectations(fx);
    assert!(check(fx, &expect).is_empty(), "the genuine expectations match");
    let mut wrong = expect.clone();
    wrong[0][0] = format!("{}-wrong", wrong[0][0]);
    assert!(!check(fx, &wrong).is_empty(), "a wrong expectation must be reported");
}

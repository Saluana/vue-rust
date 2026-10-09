//! WP-U1b value corpus: every row of tests/ui/style/corpora/values.json is cascaded and computed through
//! `compute_styles` in the fixed context of the Chrome page, and the serialised value is compared with what Chrome
//! computed. Rows of kind "ok" must match with no diagnostic; rows of kind "invalid" or "unsupported" must keep the
//! value of the page without the declaration and must produce a diagnostic on the element.

mod support;

use support::{value_context, Json};
use vn_css::media::MediaEnv;
use vn_ui::{compute_styles, NoStates};

const VALUES: &str = include_str!("../../../tests/ui/style/corpora/values.json");
const BASE: &str = "font-size: 16px; display: none";

/// One row as the test sees it: the declaration, the property read, the parent weight and the expectation.
struct Row {
    index: usize,
    category: String,
    decl: String,
    read: String,
    weight: Option<String>,
    kind: String,
    want: String,
}

fn rows() -> Vec<Row> {
    let doc = Json::parse(VALUES);
    doc.get("rows")
        .arr()
        .iter()
        .enumerate()
        .map(|(index, r)| {
            let weight = match r.get("w") {
                Json::Null => None,
                w => Some(format!("{}", w.num() as i64)),
            };
            Row {
                index,
                category: r.get("c").str().to_string(),
                decl: format!("{}: {}", r.get("s").str(), r.get("v").str()),
                read: r.get("r").str().to_string(),
                weight,
                kind: r.get("k").str().to_string(),
                want: r.get("e").str().to_string(),
            }
        })
        .collect()
}

/// The serialised value of the row's property and the number of diagnostics on the element; None when the cascade
/// has no value for the property.
fn compute(row: &Row, decl: &str) -> (Option<String>, usize) {
    let style_text = format!("{BASE}; {decl}");
    let (mut doc, t) = value_context(&style_text, row.weight.as_deref());
    let env = MediaEnv::new(800.0, 600.0);
    let tree = compute_styles(&mut doc, &[], &env, &NoStates);
    let got = tree.style(t).and_then(|s| s.get(&row.read));
    let diags = tree.diagnostics.iter().filter(|d| d.node == Some(t)).count();
    (got, diags)
}

/// None when the row matches; otherwise the description of the mismatch.
fn check(row: &Row, want: &str) -> Option<String> {
    let (got, diags) = compute(row, &row.decl);
    let got_text = got.clone().unwrap_or_else(|| "<absent>".to_string());
    if got.as_deref() != Some(want) {
        return Some(format!("row {} {:?} read {}: got {got_text:?}, Chrome {want:?}", row.index, row.decl, row.read));
    }
    match row.kind.as_str() {
        "ok" if diags > 0 => Some(format!("row {} {:?}: {diags} unexpected diagnostic(s)", row.index, row.decl)),
        "invalid" | "unsupported" if diags == 0 => Some(format!("row {} {:?}: no diagnostic for a refused value", row.index, row.decl)),
        _ => None,
    }
}

#[test]
fn values_match_chrome() {
    let rows = rows();
    let mut counts = std::collections::BTreeMap::<String, (usize, usize)>::new();
    let mut failures = Vec::new();
    for row in &rows {
        let entry = counts.entry(row.category.clone()).or_insert((0, 0));
        entry.0 += 1;
        if let Some(msg) = check(row, &row.want) {
            failures.push(msg);
        } else {
            entry.1 += 1;
        }
    }
    for (cat, (total, ok)) in &counts {
        eprintln!("values {cat:<12} {ok}/{total} rows match");
    }
    assert!(failures.is_empty(), "{} of {} value rows differ from Chrome; first ones:\n{}", failures.len(), rows.len(), failures.iter().take(400).cloned().collect::<Vec<_>>().join("\n"));
}

/// The checker must reject a wrong expectation and a changed declaration (negative control). The rows are built here,
/// so the mutation is known to change the value: 11px is not 10px.
#[test]
fn values_checker_detects_wrong_expectations() {
    let row = |decl: &str| Row {
        index: 0,
        category: "control".to_string(),
        decl: decl.to_string(),
        read: "width".to_string(),
        weight: None,
        kind: "ok".to_string(),
        want: "10px".to_string(),
    };
    assert!(check(&row("width: 10px"), "10px").is_none(), "the genuine row matches");
    assert!(check(&row("width: 10px"), "11px").is_some(), "a wrong expectation must be reported");
    assert!(check(&row("width: 11px"), "10px").is_some(), "a mutated declaration must be reported");
}

//! Cascade and custom-property tests with hand-written expectations (CSS Cascade 4 and CSS Variables 1 as
//! implemented by Chrome, which the corpus also checks). Covers the precedence keys, inline styles, media-conditional
//! rules, custom property inheritance and keywords, var() fallbacks, cycles, and serialisation.

mod support;

use support::{fake, Doc, Json};
use vn_css::cascade::{cascade, Origin};
use vn_css::custom::{computed_custom_properties, resolve_custom_property, CustomProperties};
use vn_css::matching::ElementState;
use vn_css::media::MediaEnv;
use vn_css::parse::{parse_declarations, parse_stylesheet, ParseOptions};
use vn_css::Stylesheet;

fn sheet(src: &str) -> Stylesheet {
    let r = parse_stylesheet(src, &ParseOptions::default());
    assert!(r.diagnostics.is_empty(), "{src:?}: {:?}", r.diagnostics);
    r.sheet
}

fn value_of(src: &str, name: &str) -> Option<String> {
    let s = sheet(src);
    let doc = fake("div", &[("id", "t")], ElementState::default());
    let env = MediaEnv::new(800.0, 600.0);
    let sheets = [(&s, Origin::Author)];
    let cas = cascade(&doc.el(0), &sheets, &env, &[]);
    let custom = computed_custom_properties(&[&cas]);
    custom.get(name).map(|t| t.text().to_string())
}

fn winner_text(src: &str, prop: &str) -> Option<String> {
    let s = sheet(src);
    let doc = fake("div", &[("id", "t"), ("class", "k")], ElementState::default());
    let env = MediaEnv::new(800.0, 600.0);
    let sheets = [(&s, Origin::Author)];
    let cas = cascade(&doc.el(0), &sheets, &env, &[]);
    cas.winner(prop).map(|c| c.decl.value.text().to_string())
}

#[test]
fn author_normal_beats_user_agent_normal_whatever_the_specificity() {
    let ua = sheet("#t { display: block }");
    let author = sheet("div { display: inline }");
    let doc = fake("div", &[("id", "t")], ElementState::default());
    let env = MediaEnv::new(800.0, 600.0);
    let cas = cascade(&doc.el(0), &[(&ua, Origin::UserAgent), (&author, Origin::Author)], &env, &[]);
    assert_eq!(cas.winner("display").expect("winner").decl.value.text(), "inline");
}

#[test]
fn important_user_agent_beats_important_author() {
    let ua = sheet("div { display: block !important }");
    let author = sheet("#t { display: inline !important }");
    let doc = fake("div", &[("id", "t")], ElementState::default());
    let env = MediaEnv::new(800.0, 600.0);
    let cas = cascade(&doc.el(0), &[(&ua, Origin::UserAgent), (&author, Origin::Author)], &env, &[]);
    let w = cas.winner("display").expect("winner");
    assert_eq!(w.decl.value.text(), "block");
    assert_eq!(w.origin, Origin::UserAgent);
}

#[test]
fn important_author_beats_normal_author_whatever_the_specificity() {
    assert_eq!(winner_text("div { display: block !important } #t { display: inline }", "display").as_deref(), Some("block"));
}

#[test]
fn specificity_then_source_order() {
    assert_eq!(winner_text(".k { display: block } div.k { display: inline }", "display").as_deref(), Some("inline"));
    assert_eq!(winner_text("div { display: block } span { display: inline } div { display: flex }", "display").as_deref(), Some("flex"));
    assert_eq!(winner_text("#t { display: block } .k { display: inline }", "display").as_deref(), Some("block"));
}

#[test]
fn later_sheets_win_ties() {
    let a = sheet("div { display: block }");
    let b = sheet("div { display: inline }");
    let doc = fake("div", &[], ElementState::default());
    let env = MediaEnv::new(800.0, 600.0);
    let cas = cascade(&doc.el(0), &[(&a, Origin::Author), (&b, Origin::Author)], &env, &[]);
    assert_eq!(cas.winner("display").expect("winner").decl.value.text(), "inline");
}

#[test]
fn inline_beats_author_normal_and_loses_to_important_author() {
    let s = sheet("#t { display: block } #t.k { visibility: hidden !important }");
    let doc = fake("div", &[("id", "t"), ("class", "k")], ElementState::default());
    let env = MediaEnv::new(800.0, 600.0);
    let (inline, _) = parse_declarations("display: inline");
    let cas = cascade(&doc.el(0), &[(&s, Origin::Author)], &env, &inline);
    assert_eq!(cas.winner("display").expect("winner").decl.value.text(), "inline");
    let (inline_imp, _) = parse_declarations("visibility: visible");
    let cas = cascade(&doc.el(0), &[(&s, Origin::Author)], &env, &inline_imp);
    assert_eq!(cas.winner("visibility").expect("winner").decl.value.text(), "hidden", "author !important beats inline normal");
}

#[test]
fn media_conditional_rules_only_apply_when_they_match() {
    let s = sheet("@media (min-width: 600px) { div { display: block } } @media print { div { display: none } } div { display: flex }");
    let doc = fake("div", &[], ElementState::default());
    let wide = MediaEnv::new(600.0, 100.0);
    assert_eq!(cascade(&doc.el(0), &[(&s, Origin::Author)], &wide, &[]).winner("display").expect("winner").decl.value.text(), "flex");
    let s2 = sheet("@media (min-width: 600px) { div { display: block } }");
    assert_eq!(cascade(&doc.el(0), &[(&s2, Origin::Author)], &wide, &[]).winner("display").expect("winner").decl.value.text(), "block");
    let narrow = MediaEnv::new(599.0, 100.0);
    assert!(cascade(&doc.el(0), &[(&s2, Origin::Author)], &narrow, &[]).winner("display").is_none());
}

#[test]
fn candidates_are_in_ascending_precedence() {
    let s = sheet("div { display: block } #t { display: inline } div { display: flex !important }");
    let doc = fake("div", &[("id", "t")], ElementState::default());
    let env = MediaEnv::new(800.0, 600.0);
    let cas = cascade(&doc.el(0), &[(&s, Origin::Author)], &env, &[]);
    let values: Vec<&str> = cas.candidates().iter().map(|c| c.decl.value.text()).collect();
    assert_eq!(values, vec!["block", "inline", "flex"], "normal by specificity, then important");
}

#[test]
fn custom_properties_inherit_and_children_override() {
    let root = sheet(":root { --c: red; --d: 2px }");
    let child = sheet("p { --c: blue }");
    let env = MediaEnv::new(800.0, 600.0);
    let doc = Doc::from_json(&Json::parse(r#"{"t":"html","c":[{"t":"head"},{"t":"body","c":[{"t":"p","c":[{"t":"span"}]}]}]}"#));
    let cas_root = cascade(&doc.el(0), &[(&root, Origin::Author)], &env, &[]);
    let cas_body = cascade(&doc.el(2), &[(&root, Origin::Author)], &env, &[]);
    let cas_p = cascade(&doc.el(3), &[(&root, Origin::Author), (&child, Origin::Author)], &env, &[]);
    let cas_span = cascade(&doc.el(4), &[(&root, Origin::Author), (&child, Origin::Author)], &env, &[]);
    let chain = |xs: &[&vn_css::CascadedDeclarations]| computed_custom_properties(xs);
    assert_eq!(chain(&[&cas_root, &cas_body]).get("--c").map(|t| t.text()), Some("red"));
    assert_eq!(chain(&[&cas_root, &cas_body, &cas_p]).get("--c").map(|t| t.text()), Some("blue"));
    assert_eq!(chain(&[&cas_root, &cas_body, &cas_p, &cas_span]).get("--c").map(|t| t.text()), Some("blue"), "inherited from p");
    assert_eq!(resolve_custom_property("--d", &[&cas_root, &cas_body, &cas_p]).map(|t| t.text().to_string()), Some("2px".into()));
    assert!(resolve_custom_property("--missing", &[&cas_root]).is_none());
}

#[test]
fn var_fallbacks_nest_and_undefined_without_fallback_is_invalid() {
    assert_eq!(value_of("#t { --x: var(--nope, 3px 4px) }", "--x").as_deref(), Some("3px 4px"));
    assert_eq!(value_of("#t { --p: 1px; --x: var(--nope, var(--p, 9px)) }", "--x").as_deref(), Some("1px"));
    assert_eq!(value_of("#t { --x: var(--nope, var(--nope2, 7px)) }", "--x").as_deref(), Some("7px"));
    assert_eq!(value_of("#t { --x: var(--nope) }", "--x"), None, "guaranteed-invalid");
    assert_eq!(value_of("#t { --x: a var(--nope) b }", "--x"), None);
}

#[test]
fn empty_values_are_valid_and_do_not_use_fallbacks() {
    assert_eq!(value_of("#t { --e: ; --x: var(--e, 5px) }", "--x").as_deref(), Some(""));
    assert_eq!(value_of("#t { --e: ; --x: a var(--e) b }", "--x").as_deref(), Some("a  b"));
    assert_eq!(value_of("#t { --x: ; }", "--x").as_deref(), Some(""));
}

#[test]
fn cycles_are_guaranteed_invalid() {
    assert_eq!(value_of("#t { --x: var(--x) }", "--x"), None);
    assert_eq!(value_of("#t { --a: var(--b); --b: var(--a); --x: var(--a) }", "--x"), None);
    assert_eq!(value_of("#t { --a: var(--b); --b: var(--a); --x: var(--a, 4px) }", "--x").as_deref(), Some("4px"), "fallback used for a cycle");
    assert_eq!(value_of("#t { --a: var(--b, 1px); --b: var(--a, 2px); --x: var(--a) }", "--x"), None, "both are in the cycle");
}

#[test]
fn css_wide_keywords_on_custom_properties() {
    let root = sheet(":root { --c: red }");
    let child = sheet("p { --c: inherit; --d: initial; --e: unset }");
    let env = MediaEnv::new(800.0, 600.0);
    let doc = Doc::from_json(&Json::parse(r#"{"t":"html","c":[{"t":"head"},{"t":"body","c":[{"t":"p"}]}]}"#));
    let a = cascade(&doc.el(0), &[(&root, Origin::Author)], &env, &[]);
    let p = cascade(&doc.el(3), &[(&root, Origin::Author), (&child, Origin::Author)], &env, &[]);
    let custom: CustomProperties = computed_custom_properties(&[&a, &p]);
    assert_eq!(custom.get("--c").map(|t| t.text()), Some("red"), "inherit takes the parent's value");
    assert!(custom.get("--d").is_none(), "initial is guaranteed-invalid");
    assert!(custom.get("--e").is_none(), "unset inherits, and the root has none");
}

#[test]
fn custom_values_keep_their_raw_text() {
    assert_eq!(value_of("#t { --x: a /* c */ b }", "--x").as_deref(), Some("a /* c */ b"));
    assert_eq!(value_of("#t { --x:  1px    2px  }", "--x").as_deref(), Some("1px    2px"));
    assert_eq!(value_of("#t { --x: \"a   b\" }", "--x").as_deref(), Some("\"a   b\""));
    assert_eq!(value_of("#t { --x: 1px !important }", "--x").as_deref(), Some("1px"));
}

#[test]
fn substitution_joins_raw_text_and_separates_tokens_that_would_merge() {
    assert_eq!(value_of("#t { --p: 1px   2px; --x: var(--p)  3px }", "--x").as_deref(), Some("1px   2px  3px"));
    assert_eq!(value_of("#t { --p: 1px; --x: var(--p)3px }", "--x").as_deref(), Some("1px/**/3px"));
    assert_eq!(value_of("#t { --p: 1px; --x: var(--p)-1 }", "--x").as_deref(), Some("1px/**/-1"));
    assert_eq!(value_of("#t { --p: y; --x: a+1 var(--p) }", "--x").as_deref(), Some("a/**/+1 y"), "literal pairs are separated once a var() is present");
    assert_eq!(value_of("#t { --x: a+1 }", "--x").as_deref(), Some("a+1"), "no var(): raw text");
}

#[test]
fn regular_properties_substitute_and_report_invalid_values() {
    let s = sheet("#t { --d: block; display: var(--d); visibility: var(--nope, hidden); position: var(--nope) }");
    let doc = fake("div", &[("id", "t")], ElementState::default());
    let env = MediaEnv::new(800.0, 600.0);
    let cas = cascade(&doc.el(0), &[(&s, Origin::Author)], &env, &[]);
    let custom = computed_custom_properties(&[&cas]);
    let display = cas.winner("display").expect("display winner");
    assert_eq!(custom.substitute(&display.decl.value).expect("valid").text(), "block");
    let vis = cas.winner("visibility").expect("visibility winner");
    assert_eq!(custom.substitute(&vis.decl.value).expect("fallback").text(), "hidden");
    let pos = cas.winner("position").expect("position winner");
    assert!(custom.substitute(&pos.decl.value).is_err());
}

#[test]
fn inline_declarations_from_a_style_attribute_are_author_origin() {
    let (decls, d) = parse_declarations("display: none; --x: 1px");
    assert!(d.is_empty());
    let s = sheet("#t { display: block }");
    let doc = fake("div", &[("id", "t")], ElementState::default());
    let env = MediaEnv::new(800.0, 600.0);
    let cas = cascade(&doc.el(0), &[(&s, Origin::Author)], &env, &decls);
    assert_eq!(cas.winner("display").expect("winner").decl.value.text(), "none");
    assert!(cas.winner("display").expect("winner").inline);
}

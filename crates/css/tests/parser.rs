//! Stylesheet parser tests with hand-written expectations: rule and declaration recovery (CSS Syntax 3 section 5),
//! every VNCSS code with its span, at-rules and nested media, nesting rejection, and the resource limits.

use vn_css::ast::Rule;
use vn_css::diag::{code, Diagnostic};
use vn_css::parse::{parse_declarations, parse_stylesheet, ParseOptions};
use vn_css::Stylesheet;

fn sheet(src: &str) -> (Stylesheet, Vec<Diagnostic>) {
    let r = parse_stylesheet(src, &ParseOptions::default());
    (r.sheet, r.diagnostics)
}

fn codes(ds: &[Diagnostic]) -> Vec<&'static str> {
    ds.iter().map(|d| d.code).collect()
}

fn span_text<'a>(src: &'a str, d: &Diagnostic) -> &'a str {
    &src[d.start..d.end]
}

fn style_names(rules: &[Rule]) -> Vec<Vec<String>> {
    rules
        .iter()
        .filter_map(|r| match r {
            Rule::Style(s) => Some(s.declarations.iter().map(|d| d.name.clone()).collect()),
            Rule::Media(_) => None,
        })
        .collect()
}

#[test]
fn style_rules_and_declarations() {
    let (s, d) = sheet("a { color: red; background : blue !important }\nb,\nc { width: 1px }");
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(s.rules.len(), 2);
    let Rule::Style(first) = &s.rules[0] else { panic!("style rule expected") };
    assert_eq!(first.selectors.selectors.len(), 1);
    assert_eq!(first.declarations.len(), 2);
    assert_eq!(first.declarations[1].name, "background");
    assert!(first.declarations[1].important);
    assert_eq!(first.declarations[1].value.text(), "blue");
    let Rule::Style(second) = &s.rules[1] else { panic!("style rule expected") };
    assert_eq!(second.selectors.selectors.len(), 2, "selector list members");
}

#[test]
fn property_names_lowercase_but_custom_names_keep_case() {
    let (s, d) = sheet("p { COLOR: red; --X: 1px    2px ; --x: y }");
    assert!(d.is_empty(), "{d:?}");
    let Rule::Style(r) = &s.rules[0] else { panic!("style rule") };
    assert_eq!(r.declarations[0].name, "color");
    assert_eq!(r.declarations[1].name, "--X");
    assert_eq!(r.declarations[1].value.text(), "1px    2px", "custom values keep their raw text");
    assert_eq!(r.declarations[2].name, "--x", "custom names are case-sensitive");
}

#[test]
fn important_is_case_and_space_insensitive() {
    let (s, _) = sheet("p { a: b ! IMPORTANT; c: d !important/**/; e: f ! /**/important }");
    let Rule::Style(r) = &s.rules[0] else { panic!("style rule") };
    assert!(r.declarations.iter().all(|x| x.important));
    assert_eq!(r.declarations.iter().map(|x| x.value.text()).collect::<Vec<_>>(), vec!["b", "d", "f"]);
}

#[test]
fn invalid_declaration_is_dropped_and_reported_with_its_span() {
    let src = "a { color red; width: 1px }";
    let (s, d) = sheet(src);
    assert_eq!(codes(&d), vec![code::INVALID_DECLARATION]);
    assert_eq!(span_text(src, &d[0]), "color red");
    assert_eq!(style_names(&s.rules), vec![vec!["width".to_string()]]);
}

#[test]
fn empty_value_is_invalid_for_properties_but_valid_for_custom_properties() {
    let src = "a { color: ; --x: ; }";
    let (s, d) = sheet(src);
    assert_eq!(codes(&d), vec![code::INVALID_DECLARATION]);
    assert_eq!(style_names(&s.rules), vec![vec!["--x".to_string()]]);
}

#[test]
fn bad_tokens_and_bang_are_invalid_values() {
    let src = "a { x: url(a b); --y: a) b; --z: a!; --w: a !important b; ok: 1 }";
    let (s, d) = sheet(src);
    assert_eq!(codes(&d), vec![code::INVALID_VALUE_TOKEN; 4]);
    assert_eq!(style_names(&s.rules), vec![vec!["ok".to_string()]]);
}

#[test]
fn function_names_starting_with_double_dash_are_invalid() {
    let (s, d) = sheet("a { --x: --f(a); --y: -f(a); }");
    assert_eq!(codes(&d), vec![code::INVALID_VALUE_TOKEN]);
    assert_eq!(style_names(&s.rules), vec![vec!["--y".to_string()]]);
}

#[test]
fn env_is_unsupported_and_dropped() {
    let src = "a { width: env(safe-area-inset-top); height: 1px }";
    let (s, d) = sheet(src);
    assert_eq!(codes(&d), vec![code::UNSUPPORTED_FUNCTION]);
    assert_eq!(span_text(src, &d[0]), "env(safe-area-inset-top)");
    assert_eq!(style_names(&s.rules), vec![vec!["height".to_string()]]);
}

#[test]
fn unknown_at_rules_each_have_their_code() {
    let src = "@import url(x); @font-face {} @keyframes k { } @supports (a) {} @container x {} @layer a; @namespace x; @page {} @charset 'x'; @foo bar {} @-webkit-keyframes q {}";
    let (s, d) = sheet(src);
    assert!(s.rules.is_empty());
    assert_eq!(
        codes(&d),
        vec![
            code::AT_IMPORT,
            code::AT_FONT_FACE,
            code::AT_KEYFRAMES,
            code::AT_SUPPORTS,
            code::AT_CONTAINER,
            code::AT_LAYER,
            code::AT_NAMESPACE,
            code::AT_PAGE,
            code::AT_CHARSET,
            code::AT_OTHER,
            code::AT_KEYFRAMES,
        ]
    );
    assert_eq!(span_text(src, &d[0]), "@import url(x);");
    assert_eq!(span_text(src, &d[9]), "@foo bar {}");
}

#[test]
fn media_rules_nest_and_keep_their_rules() {
    let (s, d) = sheet("@media (min-width: 1px) { @media print { a{} } b { c: d } }");
    assert!(d.is_empty(), "{d:?}");
    let Rule::Media(m) = &s.rules[0] else { panic!("media rule") };
    assert_eq!(m.rules.len(), 2);
    let Rule::Media(inner) = &m.rules[0] else { panic!("nested media") };
    assert_eq!(inner.rules.len(), 1);
    assert!(matches!(m.rules[1], Rule::Style(_)));
}

#[test]
fn media_without_block_and_bad_queries_are_diagnosed() {
    let (s, d) = sheet("@media screen; @media (min-width: 5) { a {} }");
    assert_eq!(codes(&d), vec![code::INVALID_RULE, code::INVALID_MEDIA_QUERY]);
    assert_eq!(s.rules.len(), 1, "the media rule with an invalid query is kept; it never matches");
}

#[test]
fn style_rule_without_a_block_is_dropped() {
    let src = "a b";
    let (s, d) = sheet(src);
    assert!(s.rules.is_empty());
    assert_eq!(codes(&d), vec![code::INVALID_RULE]);
    assert_eq!(span_text(src, &d[0]), "a b");
}

#[test]
fn rule_without_selector_is_dropped() {
    let (s, d) = sheet("{ color: red } p { a: b }");
    assert_eq!(codes(&d), vec![code::INVALID_SELECTOR]);
    assert_eq!(style_names(&s.rules), vec![vec!["a".to_string()]]);
}

#[test]
fn stray_close_brace_drops_its_rule_only() {
    let src = "a } b { c: d } e { f: g }";
    let (s, d) = sheet(src);
    assert_eq!(codes(&d), vec![code::UNEXPECTED_CLOSE_BRACE]);
    assert_eq!(span_text(src, &d[0]), "}");
    assert_eq!(style_names(&s.rules), vec![vec!["f".to_string()]]);
}

#[test]
fn unclosed_block_is_kept_and_reported() {
    let (s, d) = sheet("a { color: red");
    assert_eq!(codes(&d), vec![code::UNCLOSED_BLOCK]);
    assert_eq!(style_names(&s.rules), vec![vec!["color".to_string()]]);
}

#[test]
fn html_comment_tokens_are_ignored_with_a_note() {
    let (s, d) = sheet("<!-- a { } --> b { c: d }");
    assert_eq!(codes(&d), vec![code::HTML_COMMENT_TOKEN, code::HTML_COMMENT_TOKEN]);
    assert_eq!(s.rules.len(), 2);
}

#[test]
fn nested_style_rules_and_at_rules_are_rejected() {
    let (s, d) = sheet("a { color: red; b { color: blue } @media (x) { } width: 1px }");
    assert_eq!(codes(&d), vec![code::NESTED_RULE, code::NESTED_RULE]);
    assert_eq!(style_names(&s.rules), vec![vec!["color".to_string(), "width".to_string()]]);
}

#[test]
fn custom_property_may_hold_blocks_and_semicolons_inside_brackets() {
    let (s, d) = sheet("a { --x: [a;b] { c: d }; y: z }");
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(style_names(&s.rules), vec![vec!["--x".to_string(), "y".to_string()]]);
}

#[test]
fn standalone_declarations() {
    let (decls, d) = parse_declarations("color: red; --x: 1px !important;");
    assert!(d.is_empty());
    assert_eq!(decls.len(), 2);
    assert!(decls[1].important && decls[1].is_custom());
    assert_eq!(decls[0].value.text(), "red");
}

#[test]
fn diagnostic_positions_are_line_and_utf16_column() {
    // The emoji is 2 UTF-16 units, so "color" starts at column 5 of line 2 (" { " is 3 more units).
    let src = "a { b: c }\n\u{1F600} { color red }";
    let (_, d) = sheet(src);
    assert_eq!(codes(&d), vec![code::INVALID_DECLARATION]);
    assert_eq!(span_text(src, &d[0]), "color red");
    assert_eq!(d[0].position(src), (2, 5));
}

#[test]
fn limits_source_nesting_and_rule_count() {
    let big = "a{}".repeat(100);
    let r = parse_stylesheet(&big, &ParseOptions { max_source_bytes: 10, ..ParseOptions::default() });
    assert_eq!(codes(&r.diagnostics), vec![code::LIMIT_SOURCE]);
    assert!(r.sheet.rules.is_empty());

    let nested = "@media x { ".repeat(4) + "a{}" + &" }".repeat(4);
    let r = parse_stylesheet(&nested, &ParseOptions { max_nesting_depth: 2, ..ParseOptions::default() });
    assert!(codes(&r.diagnostics).contains(&code::LIMIT_NESTING), "{:?}", r.diagnostics);

    let many = "a{} ".repeat(10);
    let r = parse_stylesheet(&many, &ParseOptions { max_rules: 3, ..ParseOptions::default() });
    assert_eq!(r.sheet.rules.len(), 3);
    assert_eq!(codes(&r.diagnostics), vec![code::LIMIT_RULES], "reported once");
}

#[test]
fn deep_value_nesting_is_a_resource_diagnostic() {
    let src = format!("a {{ x: {}{} }}", "(".repeat(40), ")".repeat(40));
    let r = parse_stylesheet(&src, &ParseOptions::default());
    assert_eq!(codes(&r.diagnostics), vec![code::LIMIT_NESTING]);
    let src = format!("a {{ x: {}{} }}", "(".repeat(20), ")".repeat(20));
    let r = parse_stylesheet(&src, &ParseOptions::default());
    assert!(r.diagnostics.is_empty());
}

#[test]
fn deep_rule_nesting_and_large_input_do_not_overflow_the_stack() {
    let deep = "@media x { ".repeat(5_000) + &" }".repeat(5_000);
    let r = parse_stylesheet(&deep, &ParseOptions::default());
    assert!(codes(&r.diagnostics).contains(&code::LIMIT_NESTING));
    let many: String = (0..20_000).map(|i| format!(".c{i} {{ color: red }}\n")).collect();
    let r = parse_stylesheet(&many, &ParseOptions { max_rules: 100_000, ..ParseOptions::default() });
    assert_eq!(r.sheet.rules.len(), 20_000);
    assert!(r.diagnostics.is_empty());
}

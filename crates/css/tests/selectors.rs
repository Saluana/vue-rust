//! Selector tests with hand-written expectations: the specificity examples of Selectors Level 3 section 9,
//! CSSOM serialisation strings (Chrome prints these for the same input), and the exact diagnostic code for every
//! unsupported or invalid form.

use vn_css::diag::code;
use vn_css::selector::{parse, Specificity};

fn spec(src: &str) -> Specificity {
    let list = parse(src).unwrap_or_else(|d| panic!("{src:?} should parse: {d:?}"));
    assert_eq!(list.selectors.len(), 1, "one complex selector in {src:?}");
    list.selectors[0].specificity()
}

#[test]
fn specificity_examples_from_selectors_level_3() {
    assert_eq!(spec("*"), Specificity(0, 0, 0));
    assert_eq!(spec("li"), Specificity(0, 0, 1));
    assert_eq!(spec("ul li"), Specificity(0, 0, 2));
    assert_eq!(spec("ul ol+li"), Specificity(0, 0, 3));
    assert_eq!(spec("h1 + *[REL=up]"), Specificity(0, 1, 1));
    assert_eq!(spec("ul ol li.red"), Specificity(0, 1, 3));
    assert_eq!(spec("li.red.level"), Specificity(0, 2, 1));
    assert_eq!(spec("#x34y"), Specificity(1, 0, 0));
    assert_eq!(spec("#s12:not(FOO)"), Specificity(1, 0, 1));
    assert_eq!(spec("h1#chapter1"), Specificity(1, 0, 1));
}

#[test]
fn specificity_of_pseudo_classes_and_not() {
    assert_eq!(spec(":hover"), Specificity(0, 1, 0));
    assert_eq!(spec(":nth-child(2n+1)"), Specificity(0, 1, 0));
    assert_eq!(spec(":not(#a)"), Specificity(1, 0, 0));
    assert_eq!(spec(":not(.a, #b)"), Specificity(1, 0, 0), ":not() takes the most specific argument");
    assert_eq!(spec(".foo:not(#bar)"), Specificity(1, 1, 0));
    assert_eq!(spec("a:not(.b) c"), Specificity(0, 1, 2));
    assert_eq!(spec("[x=y]"), Specificity(0, 1, 0));
}

fn text(src: &str) -> String {
    parse(src).unwrap_or_else(|d| panic!("{src:?} should parse: {d:?}")).to_string()
}

#[test]
fn serialisation_matches_cssom() {
    let cases = [
        ("a>b", "a > b"),
        ("a+b", "a + b"),
        ("a~b", "a ~ b"),
        ("a  b", "a b"),
        ("a \n\t > \n b", "a > b"),
        ("[data-x=v]", "[data-x=\"v\"]"),
        ("[data-x=\"v\" i]", "[data-x=\"v\" i]"),
        ("[data-x]", "[data-x]"),
        ("[ data-x ~= \"a b\" ]", "[data-x~=\"a b\"]"),
        (":not(.a,#b)", ":not(.a, #b)"),
        (":not(.a)", ":not(.a)"),
        (":nth-child(2n+1)", ":nth-child(2n+1)"),
        (":nth-child( -n + 3 )", ":nth-child(-n+3)"),
        (":nth-child(odd)", ":nth-child(2n+1)"),
        (":nth-child(EVEN)", ":nth-child(2n)"),
        (":nth-child(+5)", ":nth-child(5)"),
        (":nth-child(-2n-1)", ":nth-child(-2n-1)"),
        (":nth-child(n)", ":nth-child(n)"),
        (":nth-child(0n+0)", ":nth-child(0)"),
        (":nth-last-child(3)", ":nth-last-child(3)"),
        ("DIV.Foo#bar", "div.Foo#bar"),
        ("*", "*"),
        ("*.a", ".a"),
        ("A:NOT(a)", "a:not(a)"),
        (":Hover", ":hover"),
        ("a:first-child:last-child", "a:first-child:last-child"),
        ("h1 + *[REL=up]", "h1 + [rel=\"up\"]"),
        ("UL OL+LI", "ul ol + li"),
        ("[href=\"a\\\"b\"]", "[href=\"a\\\"b\"]"),
        ("[a=\"\\\\\"]", "[a=\"\\\\\"]"),
        (".a\\:b", ".a\\:b"),
        ("#\\31 x", "#\\31 x"),
        ("[a|=b]", "[a|=\"b\"]"),
        ("[x=\"\" i]", "[x=\"\" i]"),
        (":not(:hover)", ":not(:hover)"),
        ("div > p span", "div > p span"),
        ("[lang|=en]", "[lang|=\"en\"]"),
    ];
    for (src, want) in cases {
        assert_eq!(text(src), want, "serialisation of {src:?}");
    }
}

#[test]
fn round_trip_is_stable() {
    for src in ["a > b c", ":nth-child(-2n+3)", "[x~=\"a b\" i]", ":not(a, b.c)", "h1 + *[rel=\"up\"]"] {
        let once = text(src);
        assert_eq!(text(&once), once, "second serialisation of {src:?}");
    }
}

fn code_of(src: &str) -> Vec<&'static str> {
    match parse(src) {
        Ok(l) => panic!("{src:?} should be rejected, parsed as {l:?}"),
        Err(ds) => ds.iter().map(|d| d.code).collect(),
    }
}

#[test]
fn unsupported_forms_have_their_codes() {
    let cases: &[(&str, &str)] = &[
        (":has(a)", code::SEL_HAS),
        (":is(a)", code::SEL_IS_WHERE),
        (":where(a, b)", code::SEL_IS_WHERE),
        ("a::before", code::SEL_PSEUDO_ELEMENT),
        ("a:before", code::SEL_PSEUDO_ELEMENT),
        ("p::first-line", code::SEL_PSEUDO_ELEMENT),
        ("p:first-letter", code::SEL_PSEUDO_ELEMENT),
        (":visited", code::SEL_LINK_HISTORY),
        ("a:link", code::SEL_LINK_HISTORY),
        ("a:any-link", code::SEL_LINK_HISTORY),
        (":target", code::SEL_LINK_HISTORY),
        (":lang(en)", code::SEL_LANG_DIR),
        (":dir(rtl)", code::SEL_LANG_DIR),
        (":nth-of-type(2)", code::SEL_OF_TYPE),
        (":first-of-type", code::SEL_OF_TYPE),
        ("ns|a", code::SEL_NAMESPACE),
        ("*|a", code::SEL_NAMESPACE),
        ("|a", code::SEL_NAMESPACE),
        ("[ns|a]", code::SEL_NAMESPACE),
        (":not(a b)", code::SEL_COMPLEX_NOT),
        (":not(a > b)", code::SEL_COMPLEX_NOT),
        ("a||b", code::SEL_COLUMN),
        (":nth-child(2n of .a)", code::SEL_NTH_OF),
        (":scope", code::SEL_PSEUDO_CLASS),
        (":placeholder-shown", code::SEL_PSEUDO_CLASS),
        ("a &", code::NESTING_SELECTOR),
        ("[a=b s]", code::INVALID_ATTRIBUTE_FLAG),
    ];
    for (src, want) in cases {
        let codes = code_of(src);
        assert!(codes.contains(want), "{src:?}: expected {want}, got {codes:?}");
    }
}

#[test]
fn invalid_forms_are_syntax_errors() {
    let invalid = [
        "", "a >", "> a", "a > > b", ".1a", "#1a", "[a=]", "[a", ":nth-child(2n+)", ":nth-child()", "a:", "a,,b", "a[x]y", ":not()",
        "[a=1]", "a,", ":nth-child(2n+ -1)", ":nth-child(- n)", "a b >",
    ];
    for src in invalid {
        let codes = code_of(src);
        assert!(codes.iter().all(|c| *c == code::INVALID_SELECTOR || *c == code::LIMIT_SELECTOR || c.starts_with("VNCSS3")), "{src:?}: {codes:?}");
    }
}

#[test]
fn one_bad_member_rejects_the_whole_list_and_reports_each_bad_member() {
    let ds = parse("a, b:has(c), d::before").expect_err("bad members make the list invalid");
    let codes: Vec<&str> = ds.iter().map(|d| d.code).collect();
    assert_eq!(codes, vec![code::SEL_HAS, code::SEL_PSEUDO_ELEMENT]);
}

#[test]
fn diagnostic_spans_point_at_the_construct() {
    let src = "div a::before";
    let ds = parse(src).expect_err("pseudo-element is unsupported");
    assert_eq!(ds[0].code, code::SEL_PSEUDO_ELEMENT);
    assert_eq!(&src[ds[0].start..ds[0].end], "::before");
    let src = "a , b:has(c)";
    let ds = parse(src).expect_err("has is unsupported");
    assert_eq!(&src[ds[0].start..ds[0].end], ":has(c)");
}

#[test]
fn a_deep_descendant_chain_is_within_the_non_greedy_limit() {
    // Pure descendant chains are matched greedily and have no limit; a child inside the chain counts.
    assert!(parse("a b c d e f g h i j k l").is_ok());
    let ds = parse("a > b c > d e > f g > h i").expect_err("too many ancestor searches");
    assert_eq!(ds[0].code, code::LIMIT_SELECTOR);
}

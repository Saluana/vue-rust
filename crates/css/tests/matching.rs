//! Matcher tests with hand-written expectations. Covers the element states a DOM supplies (hover and active
//! are only testable here, since headless Chrome cannot hover), the HTML and non-HTML case rules, An+B over sibling
//! lists, the backtracking case that greedy descendant matching must not get wrong, and deep-tree cost.

mod support;

use support::{fake, Doc, Json, Node};
use vn_css::matching::{matches, matches_list, Element, ElementState};
use vn_css::selector::parse;

fn one(src: &str) -> vn_css::selector::SelectorList {
    parse(src).unwrap_or_else(|d| panic!("{src:?}: {d:?}"))
}

fn fake_matches(src: &str, tag: &str, attrs: &[(&str, &str)], state: ElementState) -> bool {
    let doc = fake(tag, attrs, state);
    matches_list(&one(src), &doc.el(0))
}

#[test]
fn hover_and_active_come_from_the_element_state() {
    let hovered = ElementState { hover: true, ..ElementState::default() };
    let active = ElementState { active: true, ..ElementState::default() };
    assert!(fake_matches("a:hover", "a", &[], hovered));
    assert!(!fake_matches("a:active", "a", &[], hovered));
    assert!(fake_matches("a:active", "a", &[], active));
    assert!(!fake_matches("a:hover", "a", &[], ElementState::default()));
}

#[test]
fn form_and_focus_states() {
    let st = ElementState { disabled: true, checked: true, focus: true, focus_visible: true, focus_within: true, ..ElementState::default() };
    assert!(fake_matches("input:disabled:checked", "input", &[], st));
    assert!(!fake_matches("input:enabled", "input", &[], st));
    assert!(fake_matches(":focus-visible", "button", &[], st));
    assert!(fake_matches(":focus-within", "div", &[], st));
    let en = ElementState { enabled: true, ..ElementState::default() };
    assert!(fake_matches("button:enabled", "button", &[], en));
    assert!(!fake_matches("button:disabled", "button", &[], en));
}

#[test]
fn empty_comes_from_the_element_state_not_from_children() {
    assert!(fake_matches(":empty", "div", &[], ElementState { empty: true, ..ElementState::default() }));
    assert!(!fake_matches(":empty", "div", &[], ElementState::default()));
}

#[test]
fn ids_and_classes_are_case_sensitive() {
    assert!(fake_matches("#x", "div", &[("id", "x")], ElementState::default()));
    assert!(!fake_matches("#X", "div", &[("id", "x")], ElementState::default()));
    assert!(fake_matches(".Foo", "div", &[("class", "bar  Foo baz")], ElementState::default()));
    assert!(!fake_matches(".foo", "div", &[("class", "Foo")], ElementState::default()));
}

#[test]
fn html_type_names_are_ascii_case_insensitive_and_non_html_names_are_not() {
    assert!(fake_matches("DIV", "div", &[], ElementState::default()));
    assert!(fake_matches("div", "DIV", &[], ElementState::default()));
    let svg = SvgLike { tag: "linearGradient", attrs: vec![("viewBox", "0 0 1 1")] };
    assert!(matches(&one("linearGradient").selectors[0], &svg));
    assert!(!matches(&one("lineargradient").selectors[0], &svg));
    assert!(matches(&one("[viewBox]").selectors[0], &svg), "SVG attribute names keep their case");
    assert!(!matches(&one("[viewbox]").selectors[0], &svg));
}

/// A non-HTML element (namespace is not HTML): type and attribute names are case-sensitive.
#[derive(Clone)]
struct SvgLike {
    tag: &'static str,
    attrs: Vec<(&'static str, &'static str)>,
}

impl Element for SvgLike {
    fn local_name(&self) -> &str {
        self.tag
    }
    fn namespace_is_html(&self) -> bool {
        false
    }
    fn id(&self) -> Option<&str> {
        None
    }
    fn has_class(&self, _: &str) -> bool {
        false
    }
    fn attribute(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
    }
    fn parent_element(&self) -> Option<Self> {
        None
    }
    fn prev_sibling_element(&self) -> Option<Self> {
        None
    }
    fn next_sibling_element(&self) -> Option<Self> {
        None
    }
    fn first_child_element(&self) -> Option<Self> {
        None
    }
    fn last_child_element(&self) -> Option<Self> {
        None
    }
    fn is_root(&self) -> bool {
        false
    }
    fn state(&self) -> ElementState {
        ElementState::default()
    }
}

#[test]
fn html_listed_attributes_compare_case_insensitively_others_do_not() {
    assert!(fake_matches("[type=text]", "input", &[("type", "TEXT")], ElementState::default()), "type is in the HTML list");
    assert!(fake_matches("[lang=en]", "p", &[("lang", "EN")], ElementState::default()));
    assert!(!fake_matches("[data-x=v]", "p", &[("data-x", "V")], ElementState::default()), "data-x is case-sensitive");
    assert!(fake_matches("[data-x=v i]", "p", &[("data-x", "V")], ElementState::default()), "the i flag");
    assert!(fake_matches("[TYPE=text]", "input", &[("type", "text")], ElementState::default()), "attribute names are case-insensitive");
}

#[test]
fn attribute_word_and_prefix_rules() {
    let s = ElementState::default();
    assert!(fake_matches("[data-x~=b]", "p", &[("data-x", "a  b c")], s));
    assert!(!fake_matches("[data-x~=\"a b\"]", "p", &[("data-x", "a b")], s), "a value with a space never matches ~=");
    assert!(!fake_matches("[data-x~=\"\"]", "p", &[("data-x", "")], s));
    assert!(fake_matches("[data-x|=en]", "p", &[("data-x", "en-US")], s));
    assert!(fake_matches("[data-x|=en]", "p", &[("data-x", "en")], s));
    assert!(!fake_matches("[data-x|=en]", "p", &[("data-x", "eng")], s));
    assert!(!fake_matches("[data-x^=\"\"]", "p", &[("data-x", "a")], s), "empty prefix never matches");
    assert!(fake_matches("[data-x$=b]", "p", &[("data-x", "ab")], s));
    assert!(fake_matches("[data-x*=b]", "p", &[("data-x", "abc")], s));
    assert!(fake_matches("[data-x*=é]", "p", &[("data-x", "aéb")], s), "multi-byte substrings");
}

/// A flat sibling list: parent `ul`, children `li` 1..=n. Element index i is the i-th child (1-based).
fn siblings(n: usize) -> Doc {
    let mut nodes = vec![Node {
        tag: "ul".into(),
        attrs: vec![],
        parent: None,
        prev: None,
        next: None,
        first: Some(1),
        last: Some(n),
        state: ElementState::default(),
        root: true,
    }];
    for i in 1..=n {
        nodes.push(Node {
            tag: "li".into(),
            attrs: vec![],
            parent: Some(0),
            prev: if i > 1 { Some(i - 1) } else { None },
            next: if i < n { Some(i + 1) } else { None },
            first: None,
            last: None,
            state: ElementState::default(),
            root: false,
        });
    }
    Doc { nodes }
}

fn nth_hits(src: &str, n: usize) -> Vec<usize> {
    let doc = siblings(n);
    let list = one(src);
    doc.select(|el| el.idx > 0 && matches_list(&list, &el)).into_iter().map(|i| i).collect()
}

#[test]
fn an_plus_b_over_siblings() {
    // Indices are 1-based child positions; element index = position.
    assert_eq!(nth_hits(":nth-child(2n+1)", 7), vec![1, 3, 5, 7]);
    assert_eq!(nth_hits(":nth-child(odd)", 4), vec![1, 3]);
    assert_eq!(nth_hits(":nth-child(even)", 4), vec![2, 4]);
    assert_eq!(nth_hits(":nth-child(-n+3)", 6), vec![1, 2, 3]);
    assert_eq!(nth_hits(":nth-child(3)", 6), vec![3]);
    assert_eq!(nth_hits(":nth-child(n+5)", 6), vec![5, 6]);
    assert_eq!(nth_hits(":nth-child(0n+0)", 3), Vec::<usize>::new());
    assert_eq!(nth_hits(":nth-last-child(1)", 4), vec![4]);
    // Last-indices 4 and 2 are positions 2 and 4 of 5 (position = 6 - last-index).
    assert_eq!(nth_hits(":nth-last-child(-2n+4)", 5), vec![2, 4]);
    assert_eq!(nth_hits(":first-child", 3), vec![1]);
    assert_eq!(nth_hits(":last-child", 3), vec![3]);
    assert_eq!(nth_hits(":only-child", 1), vec![1]);
    assert_eq!(nth_hits(":only-child", 2), Vec::<usize>::new());
}

#[test]
fn backtracking_finds_the_outer_ancestor_when_the_nearest_fails() {
    // body > [.x, .y > .y > span]: the nearest .y has no .x sibling before it, the outer .y does.
    let nodes = vec![
        Node { tag: "body".into(), attrs: vec![], parent: None, prev: None, next: None, first: Some(1), last: Some(3), state: ElementState::default(), root: true },
        Node { tag: "i".into(), attrs: vec![("class".into(), "x".into())], parent: Some(0), prev: None, next: Some(2), first: None, last: None, state: ElementState::default(), root: false },
        Node { tag: "i".into(), attrs: vec![("class".into(), "y".into())], parent: Some(0), prev: Some(1), next: None, first: Some(3), last: Some(3), state: ElementState::default(), root: false },
        Node { tag: "i".into(), attrs: vec![("class".into(), "y".into())], parent: Some(2), prev: None, next: None, first: Some(4), last: Some(4), state: ElementState::default(), root: false },
        Node { tag: "span".into(), attrs: vec![], parent: Some(3), prev: None, next: None, first: None, last: None, state: ElementState::default(), root: false },
    ];
    let doc = Doc { nodes };
    let list = one(".x ~ .y span");
    assert_eq!(doc.select(|el| matches_list(&list, &el)), vec![4], "outer .y has .x before it");
    let list = one(".x ~ .y .y span");
    assert_eq!(doc.select(|el| matches_list(&list, &el)), vec![4], "inner .y has no .x before it, but the chain still holds");
    let list = one(".y > .y > span");
    assert_eq!(doc.select(|el| matches_list(&list, &el)), vec![4]);
    let list = one(".x > .y span");
    assert_eq!(doc.select(|el| matches_list(&list, &el)), Vec::<usize>::new(), "child combinator needs the direct parent");
}

#[test]
fn deep_descendant_chains_are_linear_and_correct() {
    // 3000 nested divs; the subject is the deepest span.
    let depth = 3000;
    let mut nodes = Vec::new();
    for i in 0..depth {
        nodes.push(Node {
            tag: "div".into(),
            attrs: vec![],
            parent: if i == 0 { None } else { Some(i - 1) },
            prev: None,
            next: None,
            first: Some(i + 1),
            last: Some(i + 1),
            state: ElementState::default(),
            root: i == 0,
        });
    }
    nodes.push(Node { tag: "span".into(), attrs: vec![], parent: Some(depth - 1), prev: None, next: None, first: None, last: None, state: ElementState::default(), root: false });
    let doc = Doc { nodes };
    let subject = doc.el(depth);
    let deep = one("div div div div div div div div div div div div span");
    assert!(matches_list(&deep, &subject));
    let fails = one("div div div div div p span");
    assert!(!matches_list(&fails, &subject));
    // The parent and the two ancestors above it are all divs in this chain.
    let child_chain = one("div > div > div > span");
    assert!(matches_list(&child_chain, &subject));
    let child_wrong = one("p > div > div > span");
    assert!(!matches_list(&child_wrong, &subject));
}

#[test]
fn mixed_combinators_with_the_non_greedy_bound() {
    // A selector with exactly MAX_NON_GREEDY ancestor searches parses; one more is rejected (VNCSS5004).
    assert!(parse("a > b c > d e > f g").is_ok());
    assert!(parse("a > b c > d e > f g > h i").is_err());
    let doc = Doc::from_json(&Json::parse(r#"{"t":"html","c":[{"t":"head"},{"t":"body","c":[{"t":"div","a":[["class","k"]],"c":[{"t":"p","c":[{"t":"span"}]}]}]}]}"#));
    let subject = doc.el(doc.len() - 1);
    assert!(matches_list(&one("html > body div > p span"), &subject));
    assert!(!matches_list(&one("html > head p span"), &subject));
}

#[test]
fn not_takes_compounds_and_matches_when_none_of_them_does() {
    let s = ElementState::default();
    assert!(fake_matches(":not(.a, #b)", "p", &[("class", "c")], s));
    assert!(!fake_matches(":not(.a, #b)", "p", &[("class", "a")], s));
    assert!(!fake_matches(":not(p)", "p", &[], s));
    assert!(fake_matches(":not(:hover)", "p", &[], s));
}

#[test]
fn root_is_the_document_element() {
    let doc = Doc::from_json(&Json::parse(r#"{"t":"html","c":[{"t":"head"},{"t":"body"}]}"#));
    assert!(matches_list(&one(":root"), &doc.el(0)));
    assert!(!matches_list(&one(":root"), &doc.el(2)));
    assert!(matches_list(&one("html:root > body"), &doc.el(2)));
}

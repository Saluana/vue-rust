//! Selector matching over a caller's element tree. The matcher is generic: a DOM implements `Element`,
//! and the tests use a small fixture tree. Matching is right to left. A descendant combinator whose
//! left-hand side is all descendants tries only the nearest matching ancestor (provably complete, because
//! the ancestors of a farther candidate are a subset of the nearer one's). Other combinators backtrack, and
//! `selector::MAX_NON_GREEDY` bounds how many of those a selector may contain.

use crate::selector::{Attr, AttrOp, Combinator, Compound, Nth, Pseudo, Selector, SelectorList, Simple, Specificity};

/// Element state that the tree owner computes (for example from the DOM). `empty` means no child node
/// other than comments and zero-length text; `enabled` and `disabled` are the form-control states.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ElementState {
    pub hover: bool,
    pub active: bool,
    pub focus: bool,
    pub focus_visible: bool,
    pub focus_within: bool,
    pub disabled: bool,
    pub enabled: bool,
    pub checked: bool,
    pub empty: bool,
}

/// What the matcher needs from an element. Sibling and parent navigation covers element children only.
pub trait Element: Clone {
    fn local_name(&self) -> &str;
    /// True for elements in the HTML namespace (type and attribute names then compare ASCII-insensitively).
    fn namespace_is_html(&self) -> bool;
    fn id(&self) -> Option<&str>;
    fn has_class(&self, class: &str) -> bool;
    /// Attribute value by name. Names are lowercase for HTML elements (the matcher lowercases lookups).
    fn attribute(&self, name: &str) -> Option<&str>;
    fn parent_element(&self) -> Option<Self>;
    fn prev_sibling_element(&self) -> Option<Self>;
    fn next_sibling_element(&self) -> Option<Self>;
    fn first_child_element(&self) -> Option<Self>;
    fn last_child_element(&self) -> Option<Self>;
    /// True for the document element (`:root`).
    fn is_root(&self) -> bool;
    fn state(&self) -> ElementState;
}

/// Attribute names whose values compare ASCII-case-insensitively on HTML elements (Selectors 4 section 6.3;
/// the list matches Chrome, checked by the corpus).
const HTML_CI_ATTRS: &[&str] = &[
    "accept", "accept-charset", "align", "alink", "axis", "bgcolor", "charset", "checked", "clear", "codetype", "color", "compact",
    "declare", "defer", "dir", "direction", "disabled", "enctype", "face", "frame", "hreflang", "http-equiv", "lang", "language",
    "link", "media", "method", "multiple", "nohref", "noresize", "noshade", "nowrap", "readonly", "rel", "rev", "rules", "scope",
    "scrolling", "selected", "shape", "target", "text", "type", "valign", "valuetype", "vlink",
];

/// True when the selector list matches `el`.
pub fn matches_list<E: Element>(list: &SelectorList, el: &E) -> bool {
    list.selectors.iter().any(|s| matches(s, el))
}

/// True when the complex selector matches `el`.
pub fn matches<E: Element>(sel: &Selector, el: &E) -> bool {
    match_at(sel, sel.parts.len() - 1, el)
}

/// The highest specificity among the members of `list` that match `el` (None when none match).
pub fn matching_specificity<E: Element>(list: &SelectorList, el: &E) -> Option<Specificity> {
    list.selectors.iter().filter(|s| matches(s, el)).map(Selector::specificity).max()
}

/// `parts[idx]` matches `el`, and the chain of combinators to its left holds.
fn match_at<E: Element>(sel: &Selector, idx: usize, el: &E) -> bool {
    if !compound_matches(&sel.parts[idx], el) {
        return false;
    }
    if idx == 0 {
        return true;
    }
    match sel.combinators[idx - 1] {
        Combinator::Child => el.parent_element().is_some_and(|p| match_at(sel, idx - 1, &p)),
        Combinator::NextSibling => el.prev_sibling_element().is_some_and(|p| match_at(sel, idx - 1, &p)),
        Combinator::SubsequentSibling => {
            let mut cur = el.prev_sibling_element();
            while let Some(s) = cur {
                if match_at(sel, idx - 1, &s) {
                    return true;
                }
                cur = s.prev_sibling_element();
            }
            false
        }
        Combinator::Descendant => {
            let greedy = sel.combinators[..idx - 1].iter().all(|c| *c == Combinator::Descendant);
            let mut cur = el.parent_element();
            while let Some(a) = cur {
                if compound_matches(&sel.parts[idx - 1], &a) {
                    if match_at(sel, idx - 1, &a) {
                        return true;
                    }
                    if greedy {
                        return false;
                    }
                }
                cur = a.parent_element();
            }
            false
        }
    }
}

fn compound_matches<E: Element>(c: &Compound, el: &E) -> bool {
    if let Some(t) = &c.type_name {
        let ok = if el.namespace_is_html() { t.eq_ignore_ascii_case(el.local_name()) } else { t == el.local_name() };
        if !ok {
            return false;
        }
    }
    c.simple.iter().all(|s| simple_matches(s, el))
}

fn simple_matches<E: Element>(s: &Simple, el: &E) -> bool {
    match s {
        Simple::Id(v) => el.id() == Some(v.as_str()),
        Simple::Class(v) => el.has_class(v),
        Simple::Attr(a) => attr_matches(a, el),
        Simple::Pseudo(p) => pseudo_matches(p, el),
    }
}

fn pseudo_matches<E: Element>(p: &Pseudo, el: &E) -> bool {
    let st = el.state();
    match p {
        Pseudo::Hover => st.hover,
        Pseudo::Active => st.active,
        Pseudo::Focus => st.focus,
        Pseudo::FocusVisible => st.focus_visible,
        Pseudo::FocusWithin => st.focus_within,
        Pseudo::Disabled => st.disabled,
        Pseudo::Enabled => st.enabled,
        Pseudo::Checked => st.checked,
        Pseudo::Empty => st.empty,
        Pseudo::Root => el.is_root(),
        Pseudo::FirstChild => el.prev_sibling_element().is_none(),
        Pseudo::LastChild => el.next_sibling_element().is_none(),
        Pseudo::OnlyChild => el.prev_sibling_element().is_none() && el.next_sibling_element().is_none(),
        Pseudo::NthChild(n) => nth_matches(*n, 1 + count_prev(el)),
        Pseudo::NthLastChild(n) => nth_matches(*n, 1 + count_next(el)),
        Pseudo::Not(args) => !args.iter().any(|c| compound_matches(c, el)),
    }
}

fn count_prev<E: Element>(el: &E) -> i64 {
    let mut n = 0;
    let mut cur = el.prev_sibling_element();
    while let Some(s) = cur {
        n += 1;
        cur = s.prev_sibling_element();
    }
    n
}

fn count_next<E: Element>(el: &E) -> i64 {
    let mut n = 0;
    let mut cur = el.next_sibling_element();
    while let Some(s) = cur {
        n += 1;
        cur = s.next_sibling_element();
    }
    n
}

/// Whether some n >= 0 gives `a*n + b == index` (1-based).
pub(crate) fn nth_matches(n: Nth, index: i64) -> bool {
    let d = index - n.b;
    if n.a == 0 {
        return d == 0;
    }
    d % n.a == 0 && d / n.a >= 0
}

/// Attribute selector against `el`. HTML names are looked up lowercase; values compare ASCII-insensitively
/// when the `i` flag is set, or for the HTML case-insensitive attribute list on HTML elements.
fn attr_matches<E: Element>(a: &Attr, el: &E) -> bool {
    let html = el.namespace_is_html();
    let lookup = if html { a.name.to_ascii_lowercase() } else { a.name.clone() };
    let Some(actual) = el.attribute(&lookup) else {
        return false;
    };
    let Some(test) = &a.test else {
        return true;
    };
    let ci = test.insensitive || (html && HTML_CI_ATTRS.contains(&lookup.as_str()));
    let want = test.value.as_str();
    match test.op {
        AttrOp::Equals => eq(actual, want, ci),
        AttrOp::Includes => {
            !want.is_empty() && !want.bytes().any(|c| c.is_ascii_whitespace()) && actual.split(|c: char| c.is_ascii_whitespace()).any(|w| eq(w, want, ci))
        }
        AttrOp::DashMatch => eq(actual, want, ci) || (starts(actual, want, ci) && actual.as_bytes().get(want.len()) == Some(&b'-')),
        AttrOp::Prefix => !want.is_empty() && starts(actual, want, ci),
        AttrOp::Suffix => !want.is_empty() && ends(actual, want, ci),
        AttrOp::Substring => !want.is_empty() && contains(actual, want, ci),
    }
}

fn eq(a: &str, b: &str, ci: bool) -> bool {
    if ci { a.eq_ignore_ascii_case(b) } else { a == b }
}

fn starts(a: &str, b: &str, ci: bool) -> bool {
    a.len() >= b.len() && a.is_char_boundary(b.len()) && eq(&a[..b.len()], b, ci)
}

fn ends(a: &str, b: &str, ci: bool) -> bool {
    a.len() >= b.len() && a.is_char_boundary(a.len() - b.len()) && eq(&a[a.len() - b.len()..], b, ci)
}

fn contains(a: &str, b: &str, ci: bool) -> bool {
    (0..=a.len()).any(|i| a.is_char_boundary(i) && starts(&a[i..], b, ci))
}

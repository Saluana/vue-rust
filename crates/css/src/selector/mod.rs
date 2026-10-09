//! Selector AST, token-based parser, specificity and CSSOM serialisation for the supported subset
//! (design.md 9.2 "Selectors"; the allowlist is data in `support`). Anything outside the subset is an
//! error with a VNCSS3xxx code, never silently dropped.

mod display;
mod parse;

pub use parse::{parse, parse_tokens};

/// Descendant combinators whose left-hand side contains a child or sibling combinator need a search over
/// ancestors; a selector may contain at most this many of them (VNCSS5004 otherwise). Bounds matching to
/// O(depth^MAX_NON_GREEDY) per element.
pub const MAX_NON_GREEDY: usize = 3;

/// A comma-separated list of complex selectors.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectorList {
    pub selectors: Vec<Selector>,
}

/// A complex selector: `parts[0] combinators[0] parts[1] ...`, so `combinators.len() == parts.len() - 1`.
#[derive(Debug, Clone, PartialEq)]
pub struct Selector {
    pub parts: Vec<Compound>,
    pub combinators: Vec<Combinator>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    Descendant,
    Child,
    NextSibling,
    SubsequentSibling,
}

/// A compound selector: an optional type (`type_name`, as written) or `*`, then simple selectors.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Compound {
    pub type_name: Option<String>,
    pub universal: bool,
    pub simple: Vec<Simple>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Simple {
    Id(String),
    Class(String),
    Attr(Attr),
    Pseudo(Pseudo),
}

/// An attribute selector. `name` is ASCII-lowercased.
#[derive(Debug, Clone, PartialEq)]
pub struct Attr {
    pub name: String,
    pub test: Option<AttrTest>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttrTest {
    pub op: AttrOp,
    pub value: String,
    /// The `i` flag: ASCII case-insensitive value comparison.
    pub insensitive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrOp {
    Equals,
    Includes,
    DashMatch,
    Prefix,
    Suffix,
    Substring,
}

/// `An+B` for `:nth-child` and `:nth-last-child`: the element index i matches when `a*n + b == i` for some n >= 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nth {
    pub a: i64,
    pub b: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pseudo {
    Hover,
    Active,
    Focus,
    FocusVisible,
    FocusWithin,
    Disabled,
    Enabled,
    Checked,
    FirstChild,
    LastChild,
    OnlyChild,
    Empty,
    Root,
    NthChild(Nth),
    NthLastChild(Nth),
    /// `:not(<compound list>)`; a complex argument is rejected at parse time (VNCSS3008).
    Not(Vec<Compound>),
}

/// Selectors Level 4 specificity (a, b, c): ids, classes/attributes/pseudo-classes, types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Specificity(pub u32, pub u32, pub u32);

impl Compound {
    pub fn specificity(&self) -> Specificity {
        let mut s = Specificity(0, 0, u32::from(self.type_name.is_some()));
        for simple in &self.simple {
            let add = match simple {
                Simple::Id(_) => Specificity(1, 0, 0),
                Simple::Class(_) | Simple::Attr(_) => Specificity(0, 1, 0),
                Simple::Pseudo(Pseudo::Not(args)) => args.iter().map(Compound::specificity).max().unwrap_or_default(),
                Simple::Pseudo(_) => Specificity(0, 1, 0),
            };
            s = Specificity(s.0 + add.0, s.1 + add.1, s.2 + add.2);
        }
        s
    }
}

impl Selector {
    pub fn specificity(&self) -> Specificity {
        self.parts.iter().fold(Specificity::default(), |acc, c| {
            let s = c.specificity();
            Specificity(acc.0 + s.0, acc.1 + s.1, acc.2 + s.2)
        })
    }

    /// Number of descendant combinators whose left-hand side contains a non-descendant combinator.
    pub(crate) fn non_greedy_descendants(&self) -> usize {
        let mut count = 0;
        for (i, c) in self.combinators.iter().enumerate() {
            if *c == Combinator::Descendant && self.combinators[..i].iter().any(|d| *d != Combinator::Descendant) {
                count += 1;
            }
        }
        count
    }
}

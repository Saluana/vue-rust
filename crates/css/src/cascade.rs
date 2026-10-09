//! The cascade (CSS Cascade 4, property level): candidates from matching style rules (with `@media`
//! applied) and inline declarations are ordered by origin and importance, then specificity, then source
//! order. The winner per property name is the last one. Shorthand expansion is not part of this module.

use std::collections::BTreeMap;

use crate::ast::{Declaration, Rule, Stylesheet};
use crate::matching::{matching_specificity, Element};
use crate::media::{matches as media_matches, MediaEnv};
use crate::selector::Specificity;

/// Stylesheet origin. Author declarations beat user-agent ones when normal; important ones reverse that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    UserAgent,
    Author,
}

/// A declaration that matched, with the keys that order it.
#[derive(Debug, Clone, Copy)]
pub struct Candidate<'a> {
    pub decl: &'a Declaration,
    pub origin: Origin,
    pub specificity: Specificity,
    /// Inline style attribute declarations (author origin, above every selector's specificity).
    pub inline: bool,
    /// (sheet index, rule index in document order, declaration index); inline uses `sheets.len()`.
    pub order: (usize, usize, usize),
}

impl Candidate<'_> {
    /// 0 normal user-agent, 1 normal author, 2 important author, 3 important user-agent.
    fn level(&self) -> u8 {
        match (self.origin, self.decl.important) {
            (Origin::UserAgent, false) => 0,
            (Origin::Author, false) => 1,
            (Origin::Author, true) => 2,
            (Origin::UserAgent, true) => 3,
        }
    }
}

/// The matched declarations of one element, in ascending precedence, and the winner per property name.
#[derive(Debug, Clone)]
pub struct CascadedDeclarations<'a> {
    candidates: Vec<Candidate<'a>>,
    winners: BTreeMap<&'a str, usize>,
}

impl<'a> CascadedDeclarations<'a> {
    /// Every matching declaration, lowest precedence first (for debugging).
    pub fn candidates(&self) -> &[Candidate<'a>] {
        &self.candidates
    }

    /// The winning declaration for a property name (custom property names are case-sensitive).
    pub fn winner(&self, name: &str) -> Option<&Candidate<'a>> {
        self.winners.get(name).map(|&i| &self.candidates[i])
    }

    /// Every winner, in property-name order.
    pub fn winners(&self) -> impl Iterator<Item = (&'a str, &Candidate<'a>)> + '_ {
        self.winners.iter().map(|(name, &i)| (*name, &self.candidates[i]))
    }
}

/// Specificity of an inline style: above any selector.
const INLINE: Specificity = Specificity(u32::MAX, u32::MAX, u32::MAX);

/// Cascades `sheets` (in order, each with its origin) and the element's inline declarations for `el`.
pub fn cascade<'a, E: Element>(el: &E, sheets: &[(&'a Stylesheet, Origin)], env: &MediaEnv, inline: &'a [Declaration]) -> CascadedDeclarations<'a> {
    let mut candidates = Vec::new();
    for (si, (sheet, origin)) in sheets.iter().enumerate() {
        let mut rule_index = 0;
        collect(&sheet.rules, el, env, *origin, si, &mut rule_index, &mut candidates);
    }
    for (di, decl) in inline.iter().enumerate() {
        candidates.push(Candidate { decl, origin: Origin::Author, specificity: INLINE, inline: true, order: (sheets.len(), 0, di) });
    }
    candidates.sort_by_key(|c| (c.level(), c.specificity, c.order));
    let mut winners = BTreeMap::new();
    for (i, c) in candidates.iter().enumerate() {
        winners.insert(c.decl.name.as_str(), i);
    }
    CascadedDeclarations { candidates, winners }
}

fn collect<'a, E: Element>(rules: &'a [Rule], el: &E, env: &MediaEnv, origin: Origin, si: usize, rule_index: &mut usize, out: &mut Vec<Candidate<'a>>) {
    for rule in rules {
        match rule {
            Rule::Style(sr) => {
                let ri = *rule_index;
                *rule_index += 1;
                if let Some(specificity) = matching_specificity(&sr.selectors, el) {
                    for (di, decl) in sr.declarations.iter().enumerate() {
                        out.push(Candidate { decl, origin, specificity, inline: false, order: (si, ri, di) });
                    }
                }
            }
            Rule::Media(mr) => {
                if media_matches(&mr.queries, env) {
                    collect(&mr.rules, el, env, origin, si, rule_index, out);
                }
            }
        }
    }
}

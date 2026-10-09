//! Queries (WP-U0): getElementById and querySelector for one compound selector per comma-separated item. Supported:
//! tag, #id, .class, [attr], [attr=value] (quoted or bare), `*`, combined without spaces, and comma lists. Other syntax
//! (combinators, pseudo-classes, attribute operators, escapes) is UnsupportedSelector; an empty selector is SyntaxError.

use crate::error::DomError;
use crate::node::{NodeId, NodeKind, Namespace};
use crate::value::is_ascii_ws;
use crate::Document;

#[derive(Debug, Clone, PartialEq)]
enum Simple {
    Universal,
    Tag(String),
    Id(String),
    Class(String),
    Attr(String, Option<String>),
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
}

/// An identifier starting at byte `start`: returns it with the index after it.
fn ident(s: &str, start: usize) -> Option<(String, usize)> {
    let b = s.as_bytes();
    if start >= b.len() || !is_ident_start(b[start]) {
        return None;
    }
    let mut end = start + 1;
    while end < b.len() && is_ident_char(b[end]) {
        end += 1;
    }
    Some((s[start..end].to_string(), end))
}

fn parse_compound(s: &str) -> Result<Vec<Simple>, DomError> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'*' => {
                out.push(Simple::Universal);
                i += 1;
            }
            b'#' | b'.' => {
                let (name, next) = ident(s, i + 1).ok_or(DomError::UnsupportedSelector)?;
                out.push(if b[i] == b'#' { Simple::Id(name) } else { Simple::Class(name) });
                i = next;
            }
            b'[' => {
                let close = s[i..].find(']').ok_or(DomError::UnsupportedSelector)? + i;
                out.push(parse_attr(&s[i + 1..close])?);
                i = close + 1;
            }
            c if is_ident_start(c) => {
                let (name, next) = ident(s, i).ok_or(DomError::UnsupportedSelector)?;
                out.push(Simple::Tag(name));
                i = next;
            }
            _ => return Err(DomError::UnsupportedSelector),
        }
    }
    if out.is_empty() {
        return Err(DomError::SyntaxError);
    }
    Ok(out)
}

fn parse_attr(inner: &str) -> Result<Simple, DomError> {
    let inner = inner.trim_matches(is_ascii_ws);
    match inner.split_once('=') {
        None => {
            let (name, end) = ident(inner, 0).ok_or(DomError::UnsupportedSelector)?;
            if end != inner.len() {
                return Err(DomError::UnsupportedSelector);
            }
            Ok(Simple::Attr(name, None))
        }
        Some((name, value)) => {
            let name = name.trim_matches(is_ascii_ws);
            let (attr, end) = ident(name, 0).ok_or(DomError::UnsupportedSelector)?;
            if end != name.len() {
                return Err(DomError::UnsupportedSelector);
            }
            let value = value.trim_matches(is_ascii_ws);
            let unquoted = match value.as_bytes().first() {
                Some(b'"') | Some(b'\'') => {
                    let q = value.as_bytes()[0] as char;
                    let body = value.strip_prefix(q).and_then(|v| v.strip_suffix(q)).ok_or(DomError::UnsupportedSelector)?;
                    if body.contains(q) {
                        return Err(DomError::UnsupportedSelector);
                    }
                    body.to_string()
                }
                _ => {
                    if value.is_empty() || !value.bytes().all(is_ident_char) {
                        return Err(DomError::UnsupportedSelector);
                    }
                    value.to_string()
                }
            };
            Ok(Simple::Attr(attr, Some(unquoted)))
        }
    }
}

/// The selector list: one compound per comma-separated item.
fn parse_selector(sel: &str) -> Result<Vec<Vec<Simple>>, DomError> {
    if sel.trim_matches(is_ascii_ws).is_empty() {
        return Err(DomError::SyntaxError);
    }
    sel.split(',')
        .map(|part| {
            let part = part.trim_matches(is_ascii_ws);
            if part.is_empty() {
                return Err(DomError::SyntaxError);
            }
            parse_compound(part)
        })
        .collect()
}

impl Document {
    /// The first element with this id among the descendants of `root`, in tree order. An empty id matches nothing.
    pub fn get_element_by_id(&self, root: NodeId, id: &str) -> Option<NodeId> {
        if id.is_empty() {
            return None;
        }
        self.descendants(root)
            .into_iter()
            .find(|&x| self.n(x).kind == NodeKind::Element && self.attr_raw(x, "id").as_deref() == Some(id))
    }

    /// The first descendant of `root` (not root itself) that matches the selector list, in tree order.
    pub fn query_selector(&self, root: NodeId, selector: &str) -> Result<Option<NodeId>, DomError> {
        self.check(root)?;
        let list = parse_selector(selector)?;
        Ok(self.descendants(root).into_iter().find(|&x| list.iter().any(|c| self.matches_compound(x, c))))
    }

    /// An attribute as selectors see it: a pending style attribute counts as present.
    fn attr_view(&self, id: NodeId, qname: &str) -> Option<String> {
        if qname == "style" && self.n(id).style_pending {
            return Some(crate::style::serialize_declarations(&self.n(id).style));
        }
        self.attr_raw(id, qname)
    }

    fn matches_compound(&self, id: NodeId, compound: &[Simple]) -> bool {
        let n = self.n(id);
        if n.kind != NodeKind::Element {
            return false;
        }
        let html = n.ns == Namespace::Html;
        compound.iter().all(|s| match s {
            Simple::Universal => true,
            Simple::Tag(t) => {
                if html {
                    n.local == t.to_ascii_lowercase()
                } else {
                    n.local == *t
                }
            }
            Simple::Id(v) => self.attr_view(id, "id").as_deref() == Some(v.as_str()),
            Simple::Class(c) => self
                .attr_view(id, "class")
                .is_some_and(|v| v.split(is_ascii_ws).any(|t| t == c.as_str())),
            Simple::Attr(name, value) => {
                let qname = if html { name.to_ascii_lowercase() } else { name.clone() };
                match (self.attr_view(id, &qname), value) {
                    (None, _) => false,
                    (Some(_), None) => true,
                    (Some(v), Some(expected)) => v == *expected,
                }
            }
        })
    }
}

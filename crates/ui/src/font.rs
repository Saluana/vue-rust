//! Font values: family lists (quoted names, unquoted identifier sequences, generic families) and the keyword tables
//! of font-size and font-weight. Serialisation follows Chrome (checked against the corpus): a single identifier is
//! unquoted, a multi-word name is quoted with the string escapes, and generic families are lower case.

use crate::tree::{nows, split_commas, Cv};
use crate::value::ParseErr;
use vn_css::token::Kind;

#[derive(Debug, Clone, PartialEq)]
pub enum Family {
    Generic(&'static str),
    Named(String),
}

pub const GENERIC_FAMILIES: &[&str] = &[
    "cursive", "emoji", "fangsong", "fantasy", "math", "monospace", "sans-serif", "serif", "system-ui", "ui-monospace",
    "ui-rounded", "ui-sans-serif", "ui-serif",
];

/// Identifiers that cannot be the unquoted name of a family (CSS-wide keywords and `default`).
const RESERVED: &[&str] = &["default", "inherit", "initial", "revert", "revert-layer", "unset"];

pub fn parse_families(cvs: &[Cv]) -> Result<Vec<Family>, ParseErr> {
    let mut out = Vec::new();
    for item in split_commas(cvs) {
        let parts = nows(&item);
        if let [Cv::Tok(t)] = parts.as_slice() {
            if let Kind::Str(s) = &t.kind {
                out.push(Family::Named(s.clone()));
                continue;
            }
        }
        if parts.is_empty() {
            return Err(ParseErr::Invalid);
        }
        let mut words = Vec::new();
        for p in parts {
            match p {
                Cv::Tok(t) => match &t.kind {
                    Kind::Ident(s) if !RESERVED.contains(&s.to_ascii_lowercase().as_str()) => words.push(s.clone()),
                    _ => return Err(ParseErr::Invalid),
                },
                _ => return Err(ParseErr::Invalid),
            }
        }
        if words.len() == 1 {
            let lower = words[0].to_ascii_lowercase();
            if let Some(g) = GENERIC_FAMILIES.iter().find(|g| **g == lower) {
                out.push(Family::Generic(g));
                continue;
            }
        } else if GENERIC_FAMILIES.contains(&words[0].to_ascii_lowercase().as_str()) {
            // A generic keyword cannot start a longer unquoted name: the whole declaration is invalid. (A generic word
            // later in the name is an ordinary word.)
            return Err(ParseErr::Invalid);
        }
        out.push(Family::Named(words.join(" ")));
    }
    Ok(out)
}

/// Chrome's serialisation of a computed font-family list.
pub fn serialize_families(list: &[Family]) -> String {
    let items: Vec<String> = list
        .iter()
        .map(|f| match f {
            Family::Generic(g) => (*g).to_string(),
            Family::Named(n) => serialize_name(n),
        })
        .collect();
    items.join(", ")
}

fn serialize_name(n: &str) -> String {
    let lower = n.to_ascii_lowercase();
    if is_bare_ident(n) && !GENERIC_FAMILIES.contains(&lower.as_str()) && !RESERVED.contains(&lower.as_str()) {
        return n.to_string();
    }
    let mut s = String::from("\"");
    for ch in n.chars() {
        if ch == '"' || ch == '\\' {
            s.push('\\');
        }
        s.push(ch);
    }
    s.push('"');
    s
}

/// A CSS identifier that is not `--` prefixed (those are quoted by Chrome).
fn is_bare_ident(n: &str) -> bool {
    let mut chars = n.chars().peekable();
    let starts_name = |c: char| c.is_ascii_alphabetic() || c == '_' || !c.is_ascii();
    match chars.next() {
        Some('-') => match chars.next() {
            Some(c) if starts_name(c) => {}
            _ => return false,
        },
        Some(c) if starts_name(c) => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || !c.is_ascii())
}

/// font-size keywords (medium is 16px; Chrome's table).
pub fn size_keyword(name: &str) -> Option<f64> {
    Some(match name {
        "xx-small" => 9.0,
        "x-small" => 10.0,
        "small" => 13.0,
        "medium" => 16.0,
        "large" => 18.0,
        "x-large" => 24.0,
        "xx-large" => 32.0,
        "xxx-large" => 48.0,
        _ => return None,
    })
}

/// `bolder` and `lighter` (font-weight relative to the parent weight, CSS Fonts 4 table).
pub fn relative_weight(keyword: &str, parent: f64) -> Option<f64> {
    Some(match keyword {
        "bolder" => {
            if parent < 350.0 {
                400.0
            } else if parent < 550.0 {
                700.0
            } else if parent < 900.0 {
                900.0
            } else {
                parent
            }
        }
        "lighter" => {
            if parent < 100.0 {
                parent
            } else if parent < 550.0 {
                100.0
            } else if parent < 750.0 {
                400.0
            } else {
                700.0
            }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_identifiers() {
        assert!(is_bare_ident("Arial"));
        assert!(is_bare_ident("-apple-system"));
        assert!(is_bare_ident("_foo"));
        assert!(is_bare_ident("Ümlaut"));
        assert!(!is_bare_ident("--foo"));
        assert!(!is_bare_ident("1foo"));
        assert!(!is_bare_ident("Arial Black"));
        assert!(!is_bare_ident("-"));
    }
}

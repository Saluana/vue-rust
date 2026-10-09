//! Parsed stylesheet types. Declaration values are token lists (value interpretation is a later WP).
//! A `TokenList` shares its source text through an `Arc`, so its spans and raw text stay valid.

use std::sync::Arc;

use crate::media::MediaQueryList;
use crate::selector::SelectorList;
use crate::token::{tokenize, Token};

/// A sequence of component tokens with no leading or trailing whitespace. Its text is the source slice
/// from the first to the last token, so comments and whitespace between tokens are kept verbatim.
#[derive(Debug, Clone)]
pub struct TokenList {
    pub src: Arc<str>,
    pub tokens: Vec<Token>,
}

impl TokenList {
    /// Tokenizes `text` as a declaration value (surrounding whitespace is dropped).
    pub fn from_text(text: &str) -> TokenList {
        let all = tokenize(text);
        let tokens = crate::token::trim_ws(&all).to_vec();
        TokenList { src: Arc::from(text), tokens }
    }

    /// The raw source text of the list (`""` when empty).
    pub fn text(&self) -> &str {
        match (self.tokens.first(), self.tokens.last()) {
            // A hex escape's terminating whitespace belongs to its token but is not part of the value.
            (Some(a), Some(b)) => self.src[a.start..b.end].trim_end(),
            _ => "",
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }
}

impl PartialEq for TokenList {
    fn eq(&self, other: &TokenList) -> bool {
        self.text() == other.text()
    }
}

/// A declaration `name: value [!important]`. Standard property names are ASCII-lowercased; custom
/// property names (`--x`) are case-sensitive and kept as written.
#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    pub name: String,
    pub value: TokenList,
    pub important: bool,
    pub start: usize,
    pub end: usize,
}

impl Declaration {
    pub fn is_custom(&self) -> bool {
        self.name.starts_with("--")
    }
}

#[derive(Debug, Clone)]
pub struct StyleRule {
    pub selectors: SelectorList,
    pub declarations: Vec<Declaration>,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct MediaRule {
    pub queries: MediaQueryList,
    pub rules: Vec<Rule>,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub enum Rule {
    Style(StyleRule),
    Media(MediaRule),
}

/// A parsed stylesheet. `src` is the text the spans and token lists refer to.
#[derive(Debug, Clone)]
pub struct Stylesheet {
    pub src: Arc<str>,
    pub rules: Vec<Rule>,
}

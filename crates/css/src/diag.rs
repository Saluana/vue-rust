//! Diagnostics for everything the parser recovers from or refuses. Codes are stable strings:
//! VNCSS1xxx syntax, VNCSS2xxx unsupported at-rule or media construct, VNCSS3xxx unsupported
//! selector, VNCSS4xxx nesting, VNCSS5xxx resource limits. The full table is in `support::CODES`.

/// A diagnostic with a byte span into the source it was produced from (`start..end`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub hint: &'static str,
    pub start: usize,
    pub end: usize,
}

impl Diagnostic {
    pub fn new(code: &'static str, message: impl Into<String>, hint: &'static str, start: usize, end: usize) -> Diagnostic {
        Diagnostic { code, message: message.into(), hint, start, end }
    }

    /// The 1-based line and 0-based UTF-16 column of the span start in `src`.
    pub fn position(&self, src: &str) -> (u32, u32) {
        let (mut line, mut col) = (1u32, 0u32);
        let mut prev_cr = false;
        for (i, c) in src.char_indices() {
            if i >= self.start {
                break;
            }
            match c {
                '\n' if prev_cr => {}
                '\n' | '\r' | '\x0C' => {
                    line += 1;
                    col = 0;
                }
                _ => col += c.len_utf16() as u32,
            }
            prev_cr = c == '\r';
        }
        (line, col)
    }
}

/// Diagnostic codes. Each constant documents the construct it reports; `support::CODES` lists them with
/// the design row they belong to.
pub mod code {
    /// VNCSS1001: a declaration is malformed (no colon, empty value, bad value tokens).
    pub const INVALID_DECLARATION: &str = "VNCSS1001";
    /// VNCSS1002: a selector is malformed.
    pub const INVALID_SELECTOR: &str = "VNCSS1002";
    /// VNCSS1003: a rule or at-rule is missing its block or has an invalid prelude.
    pub const INVALID_RULE: &str = "VNCSS1003";
    /// VNCSS1004: an unexpected `}` appeared in a rule prelude; the rule is dropped.
    pub const UNEXPECTED_CLOSE_BRACE: &str = "VNCSS1004";
    /// VNCSS1005: a block is not closed before the end of the input (the content is kept).
    pub const UNCLOSED_BLOCK: &str = "VNCSS1005";
    /// VNCSS1006: a value contains a token that no valid value can contain (bad string or URL,
    /// unmatched closer, a `!` that is not `!important`).
    pub const INVALID_VALUE_TOKEN: &str = "VNCSS1006";
    /// VNCSS1007: a media query is malformed; it never matches.
    pub const INVALID_MEDIA_QUERY: &str = "VNCSS1007";
    /// VNCSS1008: an HTML comment token (`<!--` or `-->`) at rule level; it is ignored with this note.
    pub const HTML_COMMENT_TOKEN: &str = "VNCSS1008";
    /// VNCSS1009: an attribute selector flag other than `i`.
    pub const INVALID_ATTRIBUTE_FLAG: &str = "VNCSS1009";
    /// VNCSS2001: `@import` is not supported.
    pub const AT_IMPORT: &str = "VNCSS2001";
    /// VNCSS2002: `@font-face` is not supported.
    pub const AT_FONT_FACE: &str = "VNCSS2002";
    /// VNCSS2003: `@keyframes` is not supported.
    pub const AT_KEYFRAMES: &str = "VNCSS2003";
    /// VNCSS2004: `@supports` is not supported.
    pub const AT_SUPPORTS: &str = "VNCSS2004";
    /// VNCSS2005: `@container` is not supported.
    pub const AT_CONTAINER: &str = "VNCSS2005";
    /// VNCSS2006: `@layer` is not supported.
    pub const AT_LAYER: &str = "VNCSS2006";
    /// VNCSS2007: `@namespace` is not supported.
    pub const AT_NAMESPACE: &str = "VNCSS2007";
    /// VNCSS2008: `@page` is not supported.
    pub const AT_PAGE: &str = "VNCSS2008";
    /// VNCSS2009: `@charset` is not supported.
    pub const AT_CHARSET: &str = "VNCSS2009";
    /// VNCSS2010: any other at-rule is not supported.
    pub const AT_OTHER: &str = "VNCSS2010";
    /// VNCSS2011: a function that the framework cannot evaluate (`env()`); the declaration is dropped.
    pub const UNSUPPORTED_FUNCTION: &str = "VNCSS2011";
    /// VNCSS2101: a media type other than all, screen or print.
    pub const MEDIA_TYPE: &str = "VNCSS2101";
    /// VNCSS2102: a media feature that is not supported (or unknown); it evaluates to unknown.
    pub const MEDIA_FEATURE: &str = "VNCSS2102";
    /// VNCSS3001: `:has()` is not supported.
    pub const SEL_HAS: &str = "VNCSS3001";
    /// VNCSS3002: `:is()`, `:where()` and other forgiving selector lists are not supported.
    pub const SEL_IS_WHERE: &str = "VNCSS3002";
    /// VNCSS3003: link and history pseudo-classes (`:visited`, `:link`, `:any-link`, `:target`).
    pub const SEL_LINK_HISTORY: &str = "VNCSS3003";
    /// VNCSS3004: any pseudo-element (`::before`, `::after`, `::first-line`, ...).
    pub const SEL_PSEUDO_ELEMENT: &str = "VNCSS3004";
    /// VNCSS3005: `:lang()` and `:dir()`.
    pub const SEL_LANG_DIR: &str = "VNCSS3005";
    /// VNCSS3006: `:nth-of-type()` and the other of-type pseudo-classes.
    pub const SEL_OF_TYPE: &str = "VNCSS3006";
    /// VNCSS3007: namespace prefixes (`ns|a`, `*|a`, `|a`).
    pub const SEL_NAMESPACE: &str = "VNCSS3007";
    /// VNCSS3008: a combinator inside `:not()` (complex argument).
    pub const SEL_COMPLEX_NOT: &str = "VNCSS3008";
    /// VNCSS3009: the column combinator `||`.
    pub const SEL_COLUMN: &str = "VNCSS3009";
    /// VNCSS3010: `:nth-child(An+B of S)`.
    pub const SEL_NTH_OF: &str = "VNCSS3010";
    /// VNCSS3011: any other pseudo-class that is not in the allowlist.
    pub const SEL_PSEUDO_CLASS: &str = "VNCSS3011";
    /// VNCSS4001: a nested style rule or at-rule inside a style rule (CSS nesting).
    pub const NESTED_RULE: &str = "VNCSS4001";
    /// VNCSS4002: the nesting selector `&`.
    pub const NESTING_SELECTOR: &str = "VNCSS4002";
    /// VNCSS5001: the source is longer than `ParseOptions::max_source_bytes`; nothing is parsed.
    pub const LIMIT_SOURCE: &str = "VNCSS5001";
    /// VNCSS5002: nesting is deeper than `ParseOptions::max_nesting_depth`; the block is skipped.
    pub const LIMIT_NESTING: &str = "VNCSS5002";
    /// VNCSS5003: more rules than `ParseOptions::max_rules`; the extra rules are dropped.
    pub const LIMIT_RULES: &str = "VNCSS5003";
    /// VNCSS5004: a selector has more than `selector::MAX_NON_GREEDY` ancestor searches.
    pub const LIMIT_SELECTOR: &str = "VNCSS5004";
}

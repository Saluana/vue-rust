//! Stylesheet parser: CSS Syntax Level 3 rule and declaration recovery (sections 5.3 and 5.4), restricted
//! to the subset. Style rules and nestable `@media` are kept. Every recovery, every unsupported at-rule,
//! selector or media construct, and every limit hit produces a `Diagnostic` with a span. Nothing is
//! dropped without one.

use std::sync::Arc;

use crate::ast::{Declaration, MediaRule, Rule, Stylesheet, StyleRule, TokenList};
use crate::diag::{code, Diagnostic};
use crate::media;
use crate::selector;
use crate::token::{block_end, find_top, max_block_depth, trim_ws, tokenize, Kind, Token};

/// Limits applied while parsing (see the VNCSS5xxx codes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseOptions {
    pub max_source_bytes: usize,
    pub max_nesting_depth: usize,
    pub max_rules: usize,
}

impl Default for ParseOptions {
    fn default() -> ParseOptions {
        ParseOptions { max_source_bytes: 1 << 20, max_nesting_depth: 32, max_rules: 1 << 16 }
    }
}

#[derive(Debug, Clone)]
pub struct ParseResult {
    pub sheet: Stylesheet,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses a stylesheet. Never fails: problems are diagnostics, and the rules that remain are valid.
pub fn parse_stylesheet(src: &str, opts: &ParseOptions) -> ParseResult {
    let arc: Arc<str> = Arc::from(src);
    let mut p = Parser { src, arc: Arc::clone(&arc), opts, diags: Vec::new(), rules_seen: 0, limit_reported: false };
    if src.len() > opts.max_source_bytes {
        p.diags.push(Diagnostic::new(
            code::LIMIT_SOURCE,
            format!("stylesheet is {} bytes; the limit is {}", src.len(), opts.max_source_bytes),
            "split the stylesheet or raise ParseOptions::max_source_bytes",
            0,
            src.len(),
        ));
        return ParseResult { sheet: Stylesheet { src: arc, rules: Vec::new() }, diagnostics: p.diags };
    }
    let toks = tokenize(src);
    let rules = p.rule_list(&toks, 0);
    ParseResult { sheet: Stylesheet { src: arc, rules }, diagnostics: p.diags }
}

struct Parser<'a> {
    src: &'a str,
    arc: Arc<str>,
    opts: &'a ParseOptions,
    diags: Vec<Diagnostic>,
    rules_seen: usize,
    limit_reported: bool,
}

fn is_ident_named(k: Option<&Kind>, word: &str) -> bool {
    matches!(k, Some(Kind::Ident(s)) if s.eq_ignore_ascii_case(word))
}

impl Parser<'_> {
    fn diag(&mut self, code: &'static str, msg: impl Into<String>, hint: &'static str, start: usize, end: usize) {
        self.diags.push(Diagnostic::new(code, msg, hint, start, end));
    }

    /// Counts one rule against `max_rules`. False (after one diagnostic) when the limit is reached.
    fn count_rule(&mut self, at: usize) -> bool {
        if self.rules_seen >= self.opts.max_rules {
            if !self.limit_reported {
                self.limit_reported = true;
                self.diag(code::LIMIT_RULES, format!("more than {} rules", self.opts.max_rules), "raise ParseOptions::max_rules", at, at);
            }
            return false;
        }
        self.rules_seen += 1;
        true
    }

    /// Rules of one level (the top level or the body of an `@media`).
    fn rule_list(&mut self, toks: &[Token], depth: usize) -> Vec<Rule> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < toks.len() {
            i = match &toks[i].kind {
                Kind::Whitespace => i + 1,
                Kind::Cdo | Kind::Cdc => {
                    let t = &toks[i];
                    self.diag(code::HTML_COMMENT_TOKEN, "HTML comment tokens are ignored in stylesheets", "remove '<!--' and '-->'", t.start, t.end);
                    i + 1
                }
                Kind::AtKeyword(_) => self.at_rule(toks, i, depth, &mut out),
                _ => self.style_rule(toks, i, depth, &mut out),
            };
        }
        out
    }

    /// A qualified rule starting at `i`. Returns the index after it.
    fn style_rule(&mut self, toks: &[Token], i: usize, _depth: usize, out: &mut Vec<Rule>) -> usize {
        let open = find_top(toks, i, |k| matches!(k, Kind::LCurly));
        let prelude = trim_ws(&toks[i..open]);
        if open >= toks.len() {
            let (s, e) = span(&toks[i..]);
            self.diag(code::INVALID_RULE, "style rule has no block", "add { ... } after the selector", s, e);
            return toks.len();
        }
        let (end, closed) = block_end(toks, open);
        if !closed {
            let at = toks[open].start;
            self.diag(code::UNCLOSED_BLOCK, "block is not closed before the end of the stylesheet", "add the missing '}'", at, toks[open].end);
        }
        let body = &toks[open + 1..if closed { end - 1 } else { end }];
        if let Some(stray) = prelude.iter().find(|t| matches!(t.kind, Kind::RCurly)) {
            let (s, e) = (stray.start, stray.end);
            self.diag(code::UNEXPECTED_CLOSE_BRACE, "unexpected '}' in a rule prelude", "remove the stray '}'", s, e);
            return end;
        }
        if prelude.is_empty() {
            let t = &toks[open];
            self.diag(code::INVALID_SELECTOR, "rule has no selector", "add a selector before '{'", t.start, t.end);
            return end;
        }
        let selectors = match selector::parse_tokens(prelude, self.src) {
            Ok(list) => Some(list),
            Err(ds) => {
                self.diags.extend(ds);
                None
            }
        };
        let declarations = self.declarations(body, 0);
        if let Some(selectors) = selectors {
            if self.count_rule(prelude[0].start) {
                let start = prelude[0].start;
                let stop = toks[end - 1].end;
                out.push(Rule::Style(StyleRule { selectors, declarations, start, end: stop }));
            }
        }
        end
    }

    /// An at-rule starting at `i` (an at-keyword). Returns the index after it, block included.
    fn at_rule(&mut self, toks: &[Token], i: usize, depth: usize, out: &mut Vec<Rule>) -> usize {
        let Kind::AtKeyword(name) = &toks[i].kind else {
            return i + 1;
        };
        let name = name.to_ascii_lowercase();
        let stop = find_top(toks, i + 1, |k| matches!(k, Kind::Semicolon | Kind::LCurly));
        let prelude = trim_ws(&toks[i + 1..stop]);
        let (block, next) = if stop < toks.len() && matches!(toks[stop].kind, Kind::LCurly) {
            let (end, closed) = block_end(toks, stop);
            if !closed {
                let t = &toks[stop];
                self.diag(code::UNCLOSED_BLOCK, "block is not closed before the end of the stylesheet", "add the missing '}'", t.start, t.end);
            }
            (Some((stop, end, closed)), end)
        } else if stop < toks.len() {
            (None, stop + 1)
        } else {
            (None, toks.len())
        };
        let start = toks[i].start;
        let end = toks[next - 1].end;
        let span_all = (start, end);
        match name.as_str() {
            "media" => {
                let Some((open, block_end_at, closed)) = block else {
                    self.diag(code::INVALID_RULE, "@media requires a block", "add { ... } after the media query", span_all.0, span_all.1);
                    return next;
                };
                if depth + 1 > self.opts.max_nesting_depth {
                    self.diag(code::LIMIT_NESTING, "rules are nested too deeply", "raise ParseOptions::max_nesting_depth", span_all.0, span_all.1);
                    return next;
                }
                let (queries, qd) = media::parse_tokens(prelude, self.src);
                self.diags.extend(qd);
                let body = &toks[open + 1..if closed { block_end_at - 1 } else { block_end_at }];
                let rules = self.rule_list(body, depth + 1);
                if self.count_rule(start) {
                    out.push(Rule::Media(MediaRule { queries, rules, start, end }));
                }
            }
            other => {
                let (code_, what) = match other {
                    "import" => (code::AT_IMPORT, "@import"),
                    "font-face" => (code::AT_FONT_FACE, "@font-face"),
                    "keyframes" | "-webkit-keyframes" | "-moz-keyframes" => (code::AT_KEYFRAMES, "@keyframes"),
                    "supports" => (code::AT_SUPPORTS, "@supports"),
                    "container" => (code::AT_CONTAINER, "@container"),
                    "layer" => (code::AT_LAYER, "@layer"),
                    "namespace" => (code::AT_NAMESPACE, "@namespace"),
                    "page" => (code::AT_PAGE, "@page"),
                    "charset" => (code::AT_CHARSET, "@charset"),
                    _ => (code::AT_OTHER, "this at-rule"),
                };
                self.diag(code_, format!("{what} is not supported"), "remove it or use a supported construct", span_all.0, span_all.1);
            }
        }
        next
    }

    /// Declarations of a style rule body. Nested rules and at-rules are rejected (CSS nesting).
    fn declarations(&mut self, body: &[Token], _depth: usize) -> Vec<Declaration> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < body.len() {
            match &body[i].kind {
                Kind::Whitespace | Kind::Semicolon => {
                    i += 1;
                    continue;
                }
                Kind::Cdo | Kind::Cdc => {
                    let t = &body[i];
                    self.diag(code::HTML_COMMENT_TOKEN, "HTML comment tokens are ignored in stylesheets", "remove '<!--' and '-->'", t.start, t.end);
                    i += 1;
                    continue;
                }
                Kind::AtKeyword(_) => {
                    let stop = find_top(body, i + 1, |k| matches!(k, Kind::Semicolon | Kind::LCurly));
                    let end = if stop < body.len() && matches!(body[stop].kind, Kind::LCurly) {
                        block_end(body, stop).0
                    } else {
                        (stop + 1).min(body.len())
                    };
                    let (s, e) = span(&body[i..end]);
                    self.diag(code::NESTED_RULE, "at-rules nested in a style rule are not supported", "move the at-rule to the top level", s, e);
                    i = end;
                    continue;
                }
                _ => {}
            }
            let custom = is_custom_start(body, i);
            let stop = if custom {
                find_top(body, i, |k| matches!(k, Kind::Semicolon))
            } else {
                find_top(body, i, |k| matches!(k, Kind::Semicolon | Kind::LCurly))
            };
            if !custom && stop < body.len() && matches!(body[stop].kind, Kind::LCurly) {
                // A nested style rule (`a { ... }`): its block is skipped.
                let end = block_end(body, stop).0;
                let (s, e) = span(&body[i..end]);
                self.diag(code::NESTED_RULE, "nested style rules are not supported", "flatten the selector", s, e);
                i = end;
                continue;
            }
            if let Some(d) = self.declaration(&body[i..stop]) {
                out.push(d);
            }
            i = stop;
        }
        out
    }

    /// One declaration (no trailing semicolon). Invalid ones are diagnosed and dropped.
    fn declaration(&mut self, seg: &[Token]) -> Option<Declaration> {
        let t = trim_ws(seg);
        if t.is_empty() {
            return None;
        }
        let name = match &t[0].kind {
            Kind::Ident(n) => n.clone(),
            _ => {
                let (s, e) = span(t);
                self.diag(code::INVALID_DECLARATION, "expected a property name", "write name: value", s, e);
                return None;
            }
        };
        let rest = trim_ws(&t[1..]);
        if !matches!(rest.first().map(|x| &x.kind), Some(Kind::Colon)) {
            let (s, e) = span(t);
            self.diag(code::INVALID_DECLARATION, "expected ':' after the property name", "write name: value", s, e);
            return None;
        }
        let mut value = trim_ws(&rest[1..]);
        let custom = name.starts_with("--");
        let mut important = false;
        if value.len() >= 2 && is_ident_named(value.last().map(|x| &x.kind), "important") {
            let mut b = value.len() - 1;
            while b > 0 && matches!(value[b - 1].kind, Kind::Whitespace) {
                b -= 1;
            }
            if b >= 1 && matches!(value[b - 1].kind, Kind::Delim('!')) {
                important = true;
                value = trim_ws(&value[..b - 1]);
            }
        }
        if !custom && value.is_empty() {
            let (s, e) = span(t);
            self.diag(code::INVALID_DECLARATION, format!("'{name}' has no value"), "give the property a value", s, e);
            return None;
        }
        if max_block_depth(value) > self.opts.max_nesting_depth {
            let (s, e) = span(t);
            return self.drop_with(code::LIMIT_NESTING, "declaration value is nested too deeply".to_string(), "raise ParseOptions::max_nesting_depth", s, e);
        }
        if let Some(reason) = self.value_problem(value) {
            let (s, e) = span(t);
            return self.drop_with(code::INVALID_VALUE_TOKEN, format!("invalid declaration value: {reason}"), "fix the value", s, e);
        }
        if let Some(f) = value.iter().find(|x| matches!(&x.kind, Kind::Function(n) if n.eq_ignore_ascii_case("env"))) {
            let (s, e) = (f.start, span(value).1);
            return self.drop_with(code::UNSUPPORTED_FUNCTION, "env() is not supported".to_string(), "use a fixed value", s, e);
        }
        let name = if custom { name } else { name.to_ascii_lowercase() };
        let (start, end) = span(t);
        Some(Declaration { name, value: TokenList { src: Arc::clone(&self.arc), tokens: value.to_vec() }, important, start, end })
    }

    fn drop_with(&mut self, code: &'static str, msg: String, hint: &'static str, s: usize, e: usize) -> Option<Declaration> {
        self.diag(code, msg, hint, s, e);
        None
    }

    /// Why a value cannot be a value at all (bad tokens, unbalanced closers, a stray `!`, too deep).
    fn value_problem(&self, value: &[Token]) -> Option<&'static str> {
        let mut stack: Vec<fn(&Kind) -> bool> = Vec::new();
        for t in value {
            match &t.kind {
                Kind::BadStr | Kind::BadUrl => return Some("bad string or URL"),
                // Chrome rejects a function whose name begins with -- inside a declaration value.
                Kind::Function(name) if name.starts_with("--") => return Some("a function name cannot start with --"),
                Kind::LParen | Kind::Function(_) => stack.push(|k| matches!(k, Kind::RParen)),
                Kind::LSquare => stack.push(|k| matches!(k, Kind::RSquare)),
                Kind::LCurly => stack.push(|k| matches!(k, Kind::RCurly)),
                Kind::RParen | Kind::RSquare | Kind::RCurly => {
                    if stack.last().is_some_and(|expect| expect(&t.kind)) {
                        stack.pop();
                    } else {
                        return Some("unmatched closing bracket");
                    }
                }
                Kind::Delim('!') if stack.is_empty() => return Some("'!' is only allowed as !important"),
                _ => {}
            }
        }
        if !stack.is_empty() {
            return Some("unclosed bracket");
        }
        None
    }
}

/// True when the segment at `i` is a custom property declaration (`--x: ...`), whose value may hold blocks.
fn is_custom_start(body: &[Token], i: usize) -> bool {
    matches!(&body[i].kind, Kind::Ident(n) if n.starts_with("--")) && {
        let rest = trim_ws(&body[i + 1..]);
        matches!(rest.first().map(|x| &x.kind), Some(Kind::Colon))
    }
}

/// Byte span of a token slice (empty slices give (0, 0)).
fn span(toks: &[Token]) -> (usize, usize) {
    match (toks.first(), toks.last()) {
        (Some(a), Some(b)) => (a.start, b.end),
        _ => (0, 0),
    }
}

/// Parses a declaration list such as a `style` attribute (no rule and no braces). Returns the declarations
/// and their diagnostics. The declarations keep their own source text, so they can be passed to the cascade.
pub fn parse_declarations(src: &str) -> (Vec<Declaration>, Vec<Diagnostic>) {
    let arc: Arc<str> = Arc::from(src);
    let opts = ParseOptions::default();
    let toks = tokenize(src);
    let mut p = Parser { src, arc: Arc::clone(&arc), opts: &opts, diags: Vec::new(), rules_seen: 0, limit_reported: false };
    let decls = p.declarations(&toks, 0);
    (decls, p.diags)
}

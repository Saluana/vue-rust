//! Token-based selector parser. A selector list is parsed comma by comma; a bad member is reported and
//! skipped to the next top-level comma, and the whole list is then an error (no partial rule survives).

use crate::diag::{code, Diagnostic};
use crate::token::{block_end, find_top, trim_ws, tokenize, Kind, Token};

use super::{Attr, AttrOp, AttrTest, Combinator, Compound, Nth, Pseudo, Selector, SelectorList, Simple};

/// Parses a selector list from source text.
pub fn parse(src: &str) -> Result<SelectorList, Vec<Diagnostic>> {
    let toks = tokenize(src);
    parse_tokens(&toks, src)
}

/// Parses a selector list from tokens of `src` (leading and trailing whitespace is ignored).
pub fn parse_tokens(toks: &[Token], src: &str) -> Result<SelectorList, Vec<Diagnostic>> {
    let toks = trim_ws(toks);
    if toks.is_empty() {
        return Err(vec![Diagnostic::new(code::INVALID_SELECTOR, "empty selector", "give the rule at least one selector", 0, 0)]);
    }
    let mut p = P { toks, i: 0, src, diags: Vec::new() };
    let mut selectors = Vec::new();
    loop {
        if let Some(s) = p.complex() {
            selectors.push(s);
        }
        if p.peek_is(&Kind::Comma) {
            p.i += 1;
        } else {
            break;
        }
    }
    if p.diags.is_empty() {
        Ok(SelectorList { selectors })
    } else {
        Err(p.diags)
    }
}

struct P<'a> {
    toks: &'a [Token],
    i: usize,
    src: &'a str,
    diags: Vec<Diagnostic>,
}

type Res<T> = Result<T, Diagnostic>;

fn eq_ci(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

impl P<'_> {
    fn peek(&self) -> Option<&Kind> {
        self.toks.get(self.i).map(|t| &t.kind)
    }

    fn peek_at(&self, k: usize) -> Option<&Kind> {
        self.toks.get(self.i + k).map(|t| &t.kind)
    }

    fn peek_is(&self, k: &Kind) -> bool {
        self.peek() == Some(k)
    }

    fn at_end(&self) -> bool {
        self.i >= self.toks.len()
    }

    /// Skips whitespace tokens; true when at least one was skipped.
    fn skip_ws(&mut self) -> bool {
        let start = self.i;
        while matches!(self.peek(), Some(Kind::Whitespace)) {
            self.i += 1;
        }
        self.i > start
    }

    /// Diagnostic over tokens `a..b` of this parser's slice. Spans are absolute source offsets.
    fn err(&self, code: &'static str, msg: &str, hint: &'static str, a: usize, b: usize) -> Diagnostic {
        let (s, e) = if a < self.toks.len() {
            (self.toks[a].start, self.toks[(b.max(a + 1)).min(self.toks.len()) - 1].end)
        } else {
            let at = self.toks.last().map_or(self.src.len(), |t| t.end);
            (at, at)
        };
        Diagnostic::new(code, msg, hint, s, e)
    }

    /// Records a diagnostic and skips to the next top-level comma. Returns None for the caller.
    fn fail(&mut self, d: Diagnostic) -> Option<Selector> {
        self.diags.push(d);
        self.i = find_top(self.toks, self.i, |k| matches!(k, Kind::Comma));
        None
    }

    /// Parses one complex selector, or records an error and returns None.
    fn complex(&mut self) -> Option<Selector> {
        self.skip_ws();
        let mut parts = Vec::new();
        let mut combinators = Vec::new();
        loop {
            match self.compound() {
                Ok(c) => parts.push(c),
                Err(d) => return self.fail(d),
            }
            let had_ws = self.skip_ws();
            if self.at_end() || self.peek_is(&Kind::Comma) {
                break;
            }
            let comb = match self.peek() {
                Some(Kind::Delim('>')) => Combinator::Child,
                Some(Kind::Delim('+')) => Combinator::NextSibling,
                Some(Kind::Delim('~')) => Combinator::SubsequentSibling,
                Some(Kind::Delim('|')) if self.peek_at(1) == Some(&Kind::Delim('|')) => {
                    let d = self.err(code::SEL_COLUMN, "the column combinator '||' is not supported", "remove '||'", self.i, self.i + 2);
                    return self.fail(d);
                }
                _ if had_ws => Combinator::Descendant,
                _ => {
                    let d = self.err(code::INVALID_SELECTOR, "unexpected token in selector", "check the selector syntax", self.i, self.i + 1);
                    return self.fail(d);
                }
            };
            if comb != Combinator::Descendant {
                self.i += 1;
                self.skip_ws();
                if self.at_end() || self.peek_is(&Kind::Comma) {
                    let d = self.err(code::INVALID_SELECTOR, "combinator without a following compound selector", "add a selector after the combinator", self.i - 1, self.i);
                    return self.fail(d);
                }
            }
            combinators.push(comb);
        }
        let sel = Selector { parts, combinators };
        if sel.non_greedy_descendants() > super::MAX_NON_GREEDY {
            let d = self.err(code::LIMIT_SELECTOR, "selector has too many ancestor searches", "split the selector", 0, self.toks.len());
            return self.fail(d);
        }
        Some(sel)
    }

    /// One compound selector, starting at the cursor. Errors are returned for the caller to record.
    fn compound(&mut self) -> Res<Compound> {
        let mut c = Compound::default();
        let start = self.i;
        let mut any = false;
        match self.peek().cloned() {
            Some(Kind::Ident(name)) => {
                c.type_name = Some(name);
                self.i += 1;
                any = true;
                if self.peek_is(&Kind::Delim('|')) {
                    if self.peek_at(1) == Some(&Kind::Delim('|')) {
                        return Err(self.err(code::SEL_COLUMN, "the column combinator '||' is not supported", "remove '||'", self.i, self.i + 2));
                    }
                    return Err(self.err(code::SEL_NAMESPACE, "namespace prefixes are not supported", "remove the namespace prefix", start, self.i + 1));
                }
            }
            Some(Kind::Delim('*')) => {
                if self.peek_at(1) == Some(&Kind::Delim('|')) {
                    return Err(self.err(code::SEL_NAMESPACE, "namespace prefixes are not supported", "remove the namespace prefix", start, self.i + 2));
                }
                c.universal = true;
                self.i += 1;
                any = true;
            }
            Some(Kind::Delim('|')) => {
                return Err(self.err(code::SEL_NAMESPACE, "namespace prefixes are not supported", "remove the namespace prefix", start, self.i + 1));
            }
            _ => {}
        }
        loop {
            let here = self.i;
            match self.peek().cloned() {
                Some(Kind::Hash { value, id: true }) => {
                    self.i += 1;
                    c.simple.push(Simple::Id(value));
                }
                Some(Kind::Hash { id: false, .. }) => {
                    return Err(self.err(code::INVALID_SELECTOR, "an id selector must be an identifier", "start the id with a letter", here, here + 1));
                }
                Some(Kind::Delim('.')) => match self.peek_at(1).cloned() {
                    Some(Kind::Ident(name)) => {
                        self.i += 2;
                        c.simple.push(Simple::Class(name));
                    }
                    _ => return Err(self.err(code::INVALID_SELECTOR, "expected a class name after '.'", "write .name", here, here + 1)),
                },
                Some(Kind::Delim('&')) => {
                    return Err(self.err(code::NESTING_SELECTOR, "the nesting selector '&' is not supported", "remove '&'", here, here + 1));
                }
                Some(Kind::LSquare) => {
                    let (end, closed) = block_end(self.toks, here);
                    if !closed {
                        return Err(self.err(code::INVALID_SELECTOR, "unclosed attribute selector", "add ']'", here, end));
                    }
                    let attr = self.attribute(here + 1, end - 1)?;
                    self.i = end;
                    c.simple.push(Simple::Attr(attr));
                }
                Some(Kind::Colon) => {
                    let p = self.pseudo()?;
                    c.simple.push(Simple::Pseudo(p));
                }
                _ => break,
            }
            any = true;
        }
        if !any {
            return Err(self.err(code::INVALID_SELECTOR, "expected a selector", "add a type, class, id, attribute or pseudo-class", start, start + 1));
        }
        Ok(c)
    }

    /// Attribute selector contents: tokens `a..b` between the brackets.
    fn attribute(&self, a: usize, b: usize) -> Res<Attr> {
        let inner = trim_ws(&self.toks[a..b]);
        let base = self.i;
        let at = |k: usize| inner.get(k).map(|t| &t.kind);
        let name = match at(0) {
            Some(Kind::Ident(n)) => n.clone(),
            Some(Kind::Delim('*' | '|')) => {
                return Err(self.err(code::SEL_NAMESPACE, "namespace prefixes are not supported", "remove the namespace prefix", base, base + 1));
            }
            _ => return Err(self.err(code::INVALID_SELECTOR, "expected an attribute name", "write [name] or [name=value]", base, base + 1)),
        };
        let mut k = 1;
        while matches!(at(k), Some(Kind::Whitespace)) {
            k += 1;
        }
        if k >= inner.len() {
            return Ok(Attr { name, test: None });
        }
        let op = match (at(k), at(k + 1)) {
            (Some(Kind::Delim('=')), _) => {
                k += 1;
                AttrOp::Equals
            }
            (Some(Kind::Delim(c @ ('~' | '|' | '^' | '$' | '*'))), Some(Kind::Delim('='))) => {
                k += 2;
                match c {
                    '~' => AttrOp::Includes,
                    '|' => AttrOp::DashMatch,
                    '^' => AttrOp::Prefix,
                    '$' => AttrOp::Suffix,
                    _ => AttrOp::Substring,
                }
            }
            (Some(Kind::Delim('|')), _) => {
                return Err(self.err(code::SEL_NAMESPACE, "namespace prefixes are not supported", "remove the namespace prefix", base, base + 1));
            }
            _ => return Err(self.err(code::INVALID_SELECTOR, "invalid attribute operator", "use =, ~=, |=, ^=, $= or *=", base, base + 1)),
        };
        while matches!(at(k), Some(Kind::Whitespace)) {
            k += 1;
        }
        let value = match at(k) {
            Some(Kind::Ident(v) | Kind::Str(v)) => v.clone(),
            _ => return Err(self.err(code::INVALID_SELECTOR, "expected an identifier or string as attribute value", "quote the value", base, base + 1)),
        };
        k += 1;
        while matches!(at(k), Some(Kind::Whitespace)) {
            k += 1;
        }
        let mut insensitive = false;
        if let Some(Kind::Ident(flag)) = at(k) {
            if !eq_ci(flag, "i") {
                return Err(self.err(code::INVALID_ATTRIBUTE_FLAG, "unsupported attribute selector flag", "only the 'i' flag is supported", base, base + 1));
            }
            insensitive = true;
            k += 1;
            while matches!(at(k), Some(Kind::Whitespace)) {
                k += 1;
            }
        }
        if k < inner.len() {
            return Err(self.err(code::INVALID_SELECTOR, "unexpected token in attribute selector", "check the attribute selector syntax", base, base + 1));
        }
        Ok(Attr { name, test: Some(AttrTest { op, value, insensitive }) })
    }

    /// A pseudo-class at the colon under the cursor. Pseudo-elements and unsupported names are errors.
    fn pseudo(&mut self) -> Res<Pseudo> {
        let colon = self.i;
        self.i += 1;
        if self.peek_is(&Kind::Colon) {
            self.i += 1;
            return Err(self.err(code::SEL_PSEUDO_ELEMENT, "pseudo-elements are not supported", "remove the pseudo-element", colon, self.i + 1));
        }
        match self.peek().cloned() {
            Some(Kind::Ident(name)) => {
                self.i += 1;
                let lower = name.to_ascii_lowercase();
                let p = match lower.as_str() {
                    "hover" => Pseudo::Hover,
                    "active" => Pseudo::Active,
                    "focus" => Pseudo::Focus,
                    "focus-visible" => Pseudo::FocusVisible,
                    "focus-within" => Pseudo::FocusWithin,
                    "disabled" => Pseudo::Disabled,
                    "enabled" => Pseudo::Enabled,
                    "checked" => Pseudo::Checked,
                    "first-child" => Pseudo::FirstChild,
                    "last-child" => Pseudo::LastChild,
                    "only-child" => Pseudo::OnlyChild,
                    "empty" => Pseudo::Empty,
                    "root" => Pseudo::Root,
                    "before" | "after" | "first-line" | "first-letter" => {
                        return Err(self.err(code::SEL_PSEUDO_ELEMENT, "pseudo-elements are not supported", "remove the pseudo-element", colon, self.i));
                    }
                    "visited" | "link" | "any-link" | "target" | "target-within" => {
                        return Err(self.err(code::SEL_LINK_HISTORY, &format!(":{lower} is not supported"), "remove the pseudo-class", colon, self.i));
                    }
                    "first-of-type" | "last-of-type" | "only-of-type" => {
                        return Err(self.err(code::SEL_OF_TYPE, &format!(":{lower} is not supported"), "use :first-child, :last-child or :only-child", colon, self.i));
                    }
                    _ => return Err(self.err(code::SEL_PSEUDO_CLASS, &format!(":{lower} is not supported"), "see the selector allowlist", colon, self.i)),
                };
                Ok(p)
            }
            Some(Kind::Function(name)) => {
                let open = self.i;
                let (end, closed) = block_end(self.toks, open);
                if !closed {
                    return Err(self.err(code::INVALID_SELECTOR, "unclosed pseudo-class arguments", "add ')'", colon, end));
                }
                let args = &self.toks[open + 1..end - 1];
                self.i = end;
                let lower = name.to_ascii_lowercase();
                match lower.as_str() {
                    "nth-child" | "nth-last-child" => {
                        if args.iter().any(|t| matches!(&t.kind, Kind::Ident(s) if eq_ci(s, "of"))) {
                            return Err(self.err(code::SEL_NTH_OF, ":nth-child(... of S) is not supported", "use :nth-child(An+B) without 'of'", colon, end));
                        }
                        let n = match parse_nth(args, self.src) {
                            Some(n) => n,
                            None => return Err(self.err(code::INVALID_SELECTOR, "invalid An+B expression", "write odd, even, or an+b", colon, end)),
                        };
                        Ok(if lower == "nth-child" { Pseudo::NthChild(n) } else { Pseudo::NthLastChild(n) })
                    }
                    "not" => self.not_args(args, colon, end),
                    "has" => Err(self.err(code::SEL_HAS, ":has() is not supported", "remove :has()", colon, end)),
                    "is" | "where" | "matches" | "-webkit-any" | "-moz-any" => {
                        Err(self.err(code::SEL_IS_WHERE, &format!(":{lower}() is not supported"), "list the selectors separately with commas", colon, end))
                    }
                    "lang" | "dir" => Err(self.err(code::SEL_LANG_DIR, &format!(":{lower}() is not supported"), "remove the pseudo-class", colon, end)),
                    "nth-of-type" | "nth-last-of-type" => Err(self.err(code::SEL_OF_TYPE, &format!(":{lower}() is not supported"), "use :nth-child", colon, end)),
                    _ => Err(self.err(code::SEL_PSEUDO_CLASS, &format!(":{lower}() is not supported"), "see the selector allowlist", colon, end)),
                }
            }
            _ => Err(self.err(code::INVALID_SELECTOR, "expected a pseudo-class name after ':'", "write :name", colon, colon + 1)),
        }
    }

    /// `:not(...)` argument: compound selectors separated by commas. A combinator is a complex argument.
    fn not_args(&self, args: &[Token], colon: usize, end: usize) -> Res<Pseudo> {
        let mut list = Vec::new();
        let mut i = 0;
        loop {
            let j = find_top(args, i, |k| matches!(k, Kind::Comma));
            let part = trim_ws(&args[i..j]);
            if part.is_empty() {
                return Err(self.err(code::INVALID_SELECTOR, "empty argument in :not()", "remove the empty argument", colon, end));
            }
            let mut sub = P { toks: part, i: 0, src: self.src, diags: Vec::new() };
            let compound = sub.compound()?;
            if !sub.at_end() {
                let complex = matches!(sub.peek(), Some(Kind::Whitespace | Kind::Delim('>' | '+' | '~')));
                let (c, msg) = if complex {
                    (code::SEL_COMPLEX_NOT, "complex selectors are not supported in :not()")
                } else {
                    (code::INVALID_SELECTOR, "invalid :not() argument")
                };
                return Err(self.err(c, msg, "use compound selectors in :not()", colon, end));
            }
            list.push(compound);
            if j >= args.len() {
                break;
            }
            i = j + 1;
        }
        Ok(Pseudo::Not(list))
    }
}

fn int_of(v: f64) -> Option<i64> {
    if v.fract() == 0.0 && v.abs() < 9.0e15 { Some(v as i64) } else { None }
}

/// Strips an ASCII-case-insensitive prefix (char-boundary safe).
fn strip_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    if head.eq_ignore_ascii_case(prefix) { Some(&s[prefix.len()..]) } else { None }
}

/// `n`, `-n`, `n-<digits>` or `-n-<digits>` as one identifier: (a, optional b).
fn ident_head(s: &str) -> Option<(i64, Option<i64>)> {
    if eq_ci(s, "n") {
        return Some((1, None));
    }
    if eq_ci(s, "-n") {
        return Some((-1, None));
    }
    let (a, digits) = match strip_ci(s, "-n-") {
        Some(d) => (-1, d),
        None => (1, strip_ci(s, "n-")?),
    };
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((a, Some(-digits.parse::<i64>().ok()?)))
}

/// The An+B microsyntax (CSS Syntax 3 section 6.2): odd, even, an+b with optional whitespace around the sign.
/// The tokenizer reads "2n-1" as one dimension (unit "n-1") and "2n- 1" as unit "n-" then a number, so both forms
/// are handled here.
fn parse_nth(toks: &[Token], src: &str) -> Option<Nth> {
    let t = trim_ws(toks);
    if t.len() == 1 {
        match &t[0].kind {
            Kind::Ident(s) if eq_ci(s, "odd") => return Some(Nth { a: 2, b: 1 }),
            Kind::Ident(s) if eq_ci(s, "even") => return Some(Nth { a: 2, b: 0 }),
            Kind::Number { value, int: true } => return Some(Nth { a: 0, b: int_of(*value)? }),
            _ => {}
        }
    }
    let first = t.first()?;
    let signed_at = |idx: usize| matches!(src[t[idx].start..].chars().next(), Some('+' | '-'));
    // The n-part. `dash` means the minus sign is already part of it ("2n-", "n-", "-n-"); the constant after it is unsigned.
    let (a, mut k, dash) = match &first.kind {
        Kind::Dimension { value, int: true, unit } if eq_ci(unit, "n") => (int_of(*value)?, 1, false),
        Kind::Dimension { value, int: true, unit } if eq_ci(unit, "n-") => (int_of(*value)?, 1, true),
        Kind::Dimension { value, int: true, unit } if t.len() == 1 && strip_ci(unit, "n-").is_some() => {
            // "2n-1" and "-2n-1" as one dimension whose unit is "n-1".
            let digits = strip_ci(unit, "n-")?;
            if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            return Some(Nth { a: int_of(*value)?, b: -digits.parse::<i64>().ok()? });
        }
        Kind::Ident(s) if eq_ci(s, "n-") => (1, 1, true),
        Kind::Ident(s) if eq_ci(s, "-n-") => (-1, 1, true),
        Kind::Ident(s) => match ident_head(s) {
            Some((a, Some(b))) if t.len() == 1 => return Some(Nth { a, b }),
            Some((a, None)) => (a, 1, false),
            _ => return None,
        },
        Kind::Delim('+') => match t.get(1).map(|x| &x.kind) {
            Some(Kind::Ident(s)) if eq_ci(s, "n-") => (1, 2, true),
            Some(Kind::Ident(s)) => match ident_head(s) {
                Some((1, Some(b))) if t.len() == 2 => return Some(Nth { a: 1, b }),
                Some((1, None)) => (1, 2, false),
                _ => return None,
            },
            _ => return None,
        },
        _ => return None,
    };
    while matches!(t.get(k).map(|x| &x.kind), Some(Kind::Whitespace)) {
        k += 1;
    }
    if dash {
        return match t.get(k).map(|x| &x.kind) {
            Some(Kind::Number { value, int: true }) if k + 1 == t.len() && !signed_at(k) => Some(Nth { a, b: -int_of(*value)? }),
            _ => None,
        };
    }
    if k == t.len() {
        return Some(Nth { a, b: 0 });
    }
    match &t.get(k)?.kind {
        // A signed integer token directly after the n-part: "2n-1" with the sign in the number, or "2n +1".
        Kind::Number { value, int: true } if signed_at(k) && k + 1 == t.len() => Some(Nth { a, b: int_of(*value)? }),
        Kind::Delim(sign @ ('+' | '-')) => {
            let s = if *sign == '-' { -1 } else { 1 };
            let mut j = k + 1;
            while matches!(t.get(j).map(|x| &x.kind), Some(Kind::Whitespace)) {
                j += 1;
            }
            match t.get(j).map(|x| &x.kind) {
                Some(Kind::Number { value, int: true }) if j + 1 == t.len() && !signed_at(j) => Some(Nth { a, b: s * int_of(*value)? }),
                _ => None,
            }
        }
        _ => None,
    }
}

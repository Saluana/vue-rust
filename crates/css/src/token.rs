//! CSS Syntax Level 3 tokenizer (section 4) over `&str`. Comments are skipped, not emitted. NUL is
//! read as U+FFFD, and CR, LF and FF are newlines (CRLF is one newline). Every token carries its byte
//! span, the 1-based line and the 0-based column in UTF-16 units. Malformed input yields Bad* tokens;
//! nothing here fails.

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Ident(String),
    /// Name of a function token; the opening parenthesis is part of the token.
    Function(String),
    AtKeyword(String),
    Hash { value: String, id: bool },
    Str(String),
    BadStr,
    Url(String),
    BadUrl,
    Delim(char),
    /// `int` is true for an integer (no fraction or exponent).
    Number { value: f64, int: bool },
    Percentage(f64),
    Dimension { value: f64, int: bool, unit: String },
    Whitespace,
    Cdo,
    Cdc,
    Colon,
    Semicolon,
    Comma,
    LSquare,
    RSquare,
    LParen,
    RParen,
    LCurly,
    RCurly,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: Kind,
    pub start: usize,
    pub end: usize,
    pub line: u32,
    pub col: u32,
}

impl Token {
    /// The source text of this token.
    pub fn text<'a>(&self, src: &'a str) -> &'a str {
        &src[self.start..self.end]
    }
}

pub(crate) const REPLACEMENT: char = '\u{FFFD}';

pub(crate) fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0C')
}

pub(crate) fn is_newline(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\x0C')
}

fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c as u32 >= 0x80
}

fn is_name(c: char) -> bool {
    is_name_start(c) || c.is_ascii_digit() || c == '-'
}

/// Tokenizes `src`. Never fails: every malformed construct becomes a Bad* token or a delimiter.
pub fn tokenize(src: &str) -> Vec<Token> {
    let mut t = Tokenizer { src, pos: 0, scan: 0, line: 1, col: 0, prev_cr: false };
    let mut out = Vec::new();
    while t.pos < src.len() {
        if src[t.pos..].starts_with("/*") {
            t.skip_comment();
            continue;
        }
        let start = t.pos;
        let (line, col) = t.locate(start);
        let kind = t.next_kind();
        out.push(Token { kind, start, end: t.pos, line, col });
    }
    out
}

struct Tokenizer<'a> {
    src: &'a str,
    pos: usize,
    // Position scanned so far for line/column bookkeeping (only moves forward).
    scan: usize,
    line: u32,
    col: u32,
    prev_cr: bool,
}

impl Tokenizer<'_> {
    /// The character at byte `p`, with NUL read as U+FFFD (CSS input preprocessing).
    fn ch_at(&self, p: usize) -> Option<char> {
        self.src[p..].chars().next().map(|c| if c == '\0' { REPLACEMENT } else { c })
    }

    fn ch(&self) -> Option<char> {
        self.ch_at(self.pos)
    }

    fn ch2(&self, p: usize) -> Option<char> {
        let c = self.src[p..].chars().next()?;
        let rest = &self.src[p + c.len_utf8()..];
        rest.chars().next().map(|c| if c == '\0' { REPLACEMENT } else { c })
    }

    fn bump(&mut self) -> char {
        let c = self.ch().expect("bump at end of input");
        self.pos += self.src[self.pos..].chars().next().map_or(1, char::len_utf8);
        c
    }

    /// Line and UTF-16 column of byte `at`. Scans forward only, so calls must be in source order.
    fn locate(&mut self, at: usize) -> (u32, u32) {
        while self.scan < at {
            let c = self.src[self.scan..].chars().next().expect("scan within source");
            self.scan += c.len_utf8();
            match c {
                '\n' if self.prev_cr => {}
                '\n' | '\r' | '\x0C' => {
                    self.line += 1;
                    self.col = 0;
                }
                _ => self.col += c.len_utf16() as u32,
            }
            self.prev_cr = c == '\r';
        }
        (self.line, self.col)
    }

    fn skip_comment(&mut self) {
        match self.src[self.pos + 2..].find("*/") {
            Some(i) => self.pos += 2 + i + 2,
            None => self.pos = self.src.len(),
        }
    }

    /// Byte length of a newline at byte `p` (2 for CRLF, 1 for CR/LF/FF, 0 otherwise).
    fn newline_len(&self, p: usize) -> usize {
        match self.src[p..].chars().next() {
            Some('\r') if self.src[p..].starts_with("\r\n") => 2,
            Some(c) if is_newline(c) => 1,
            _ => 0,
        }
    }

    /// Two code points are a valid escape when the first is `\` and the second is not a newline.
    /// At end of input the escape is valid (it yields U+FFFD).
    fn starts_escape(&self, p: usize) -> bool {
        if !self.src[p..].starts_with('\\') {
            return false;
        }
        match self.ch2(p) {
            None => true,
            Some(c) => !is_newline(c),
        }
    }

    fn starts_ident(&self, p: usize) -> bool {
        match self.ch_at(p) {
            Some('-') => match self.ch2(p) {
                Some(c) if is_name_start(c) || c == '-' => true,
                Some('\\') => self.starts_escape(p + 1),
                _ => false,
            },
            Some(c) if is_name_start(c) => true,
            Some('\\') => self.starts_escape(p),
            _ => false,
        }
    }

    fn starts_number(&self, p: usize) -> bool {
        match self.ch_at(p) {
            Some('+' | '-') => match self.ch2(p) {
                Some(c) if c.is_ascii_digit() => true,
                Some('.') => self.src[p..].chars().nth(2).is_some_and(|c| c.is_ascii_digit()),
                _ => false,
            },
            Some('.') => self.ch2(p).is_some_and(|c| c.is_ascii_digit()),
            Some(c) => c.is_ascii_digit(),
            None => false,
        }
    }

    /// Consumes an escape whose backslash was already consumed (the caller checked it is valid).
    fn consume_escape(&mut self) -> char {
        let Some(c) = self.ch() else {
            return REPLACEMENT;
        };
        if !c.is_ascii_hexdigit() {
            self.bump();
            return c;
        }
        let mut value: u32 = 0;
        let mut n = 0;
        while n < 6 {
            match self.ch() {
                Some(h) if h.is_ascii_hexdigit() => {
                    value = value * 16 + h.to_digit(16).expect("hex digit");
                    self.bump();
                    n += 1;
                }
                _ => break,
            }
        }
        let nl = self.newline_len(self.pos);
        if nl > 0 {
            self.pos += nl;
        } else if self.ch().is_some_and(is_ws) {
            self.bump();
        }
        if value == 0 || value > 0x10FFFF {
            return REPLACEMENT;
        }
        char::from_u32(value).unwrap_or(REPLACEMENT)
    }

    fn consume_name(&mut self) -> String {
        let mut s = String::new();
        loop {
            match self.ch() {
                Some(c) if is_name(c) => {
                    s.push(c);
                    self.bump();
                }
                Some('\\') if self.starts_escape(self.pos) => {
                    self.bump();
                    s.push(self.consume_escape());
                }
                _ => return s,
            }
        }
    }

    fn consume_number(&mut self) -> (f64, bool) {
        let mut repr = String::new();
        if matches!(self.ch(), Some('+' | '-')) {
            repr.push(self.bump());
        }
        let mut int = true;
        while let Some(c) = self.ch().filter(char::is_ascii_digit) {
            repr.push(c);
            self.bump();
        }
        if self.ch() == Some('.') && self.ch2(self.pos).is_some_and(|c| c.is_ascii_digit()) {
            int = false;
            repr.push(self.bump());
            while let Some(c) = self.ch().filter(char::is_ascii_digit) {
                repr.push(c);
                self.bump();
            }
        }
        if matches!(self.ch(), Some('e' | 'E')) {
            let sign_at = self.pos + 1;
            let digit_after = match self.ch2(self.pos) {
                Some(c) if c.is_ascii_digit() => true,
                Some('+' | '-') => self.src[sign_at..].chars().nth(1).is_some_and(|c| c.is_ascii_digit()),
                _ => false,
            };
            if digit_after {
                int = false;
                repr.push(self.bump());
                if matches!(self.ch(), Some('+' | '-')) {
                    repr.push(self.bump());
                }
                while let Some(c) = self.ch().filter(char::is_ascii_digit) {
                    repr.push(c);
                    self.bump();
                }
            }
        }
        let value = repr.trim_start_matches('+').parse::<f64>().unwrap_or(0.0);
        (value, int)
    }

    fn next_kind(&mut self) -> Kind {
        let c = self.ch().expect("next_kind at end of input");
        if is_ws(c) {
            while self.ch().is_some_and(is_ws) {
                self.bump();
            }
            return Kind::Whitespace;
        }
        match c {
            '"' | '\'' => self.consume_string(),
            '#' => {
                if self.ch2(self.pos).is_some_and(is_name) || self.starts_escape(self.pos + 1) {
                    self.bump();
                    let id = self.starts_ident(self.pos);
                    let value = self.consume_name();
                    Kind::Hash { value, id }
                } else {
                    self.bump();
                    Kind::Delim('#')
                }
            }
            '(' => self.single(Kind::LParen),
            ')' => self.single(Kind::RParen),
            ',' => self.single(Kind::Comma),
            ':' => self.single(Kind::Colon),
            ';' => self.single(Kind::Semicolon),
            '[' => self.single(Kind::LSquare),
            ']' => self.single(Kind::RSquare),
            '{' => self.single(Kind::LCurly),
            '}' => self.single(Kind::RCurly),
            '+' | '.' => {
                if self.starts_number(self.pos) {
                    self.number_or_dimension()
                } else {
                    self.single(Kind::Delim(c))
                }
            }
            '-' => {
                if self.starts_number(self.pos) {
                    self.number_or_dimension()
                } else if self.src[self.pos..].starts_with("-->") {
                    self.pos += 3;
                    Kind::Cdc
                } else if self.starts_ident(self.pos) {
                    self.ident_like()
                } else {
                    self.single(Kind::Delim('-'))
                }
            }
            '<' => {
                if self.src[self.pos..].starts_with("<!--") {
                    self.pos += 4;
                    Kind::Cdo
                } else {
                    self.single(Kind::Delim('<'))
                }
            }
            '@' => {
                if self.starts_ident(self.pos + 1) {
                    self.bump();
                    Kind::AtKeyword(self.consume_name())
                } else {
                    self.single(Kind::Delim('@'))
                }
            }
            '\\' => {
                if self.starts_escape(self.pos) {
                    self.ident_like()
                } else {
                    self.single(Kind::Delim('\\'))
                }
            }
            c if c.is_ascii_digit() => self.number_or_dimension(),
            c if is_name_start(c) => self.ident_like(),
            c => self.single(Kind::Delim(c)),
        }
    }

    fn single(&mut self, kind: Kind) -> Kind {
        self.bump();
        kind
    }

    fn number_or_dimension(&mut self) -> Kind {
        let (value, int) = self.consume_number();
        if self.starts_ident(self.pos) {
            let unit = self.consume_name();
            Kind::Dimension { value, int, unit }
        } else if self.ch() == Some('%') {
            self.bump();
            Kind::Percentage(value)
        } else {
            Kind::Number { value, int }
        }
    }

    fn ident_like(&mut self) -> Kind {
        let name = self.consume_name();
        if name.eq_ignore_ascii_case("url") && self.ch() == Some('(') {
            self.bump();
            let mut q = self.pos;
            while self.src[q..].chars().next().is_some_and(is_ws) {
                q += 1;
            }
            if matches!(self.ch_at(q), Some('"' | '\'')) {
                return Kind::Function(name);
            }
            while self.ch().is_some_and(is_ws) {
                self.bump();
            }
            return self.consume_url();
        }
        if self.ch() == Some('(') {
            self.bump();
            return Kind::Function(name);
        }
        Kind::Ident(name)
    }

    fn consume_string(&mut self) -> Kind {
        let quote = self.bump();
        let mut s = String::new();
        loop {
            match self.ch() {
                None => return Kind::Str(s),
                Some(c) if c == quote => {
                    self.bump();
                    return Kind::Str(s);
                }
                Some(c) if is_newline(c) => return Kind::BadStr,
                Some('\\') => {
                    if self.pos + 1 >= self.src.len() {
                        self.bump();
                    } else if self.newline_len(self.pos + 1) > 0 {
                        self.bump();
                        self.pos += self.newline_len(self.pos);
                    } else {
                        self.bump();
                        s.push(self.consume_escape());
                    }
                }
                Some(_) => s.push(self.bump()),
            }
        }
    }

    fn consume_url(&mut self) -> Kind {
        let mut s = String::new();
        loop {
            match self.ch() {
                None => return Kind::Url(s),
                Some(')') => {
                    self.bump();
                    return Kind::Url(s);
                }
                Some(c) if is_ws(c) => {
                    while self.ch().is_some_and(is_ws) {
                        self.bump();
                    }
                    return match self.ch() {
                        None => Kind::Url(s),
                        Some(')') => {
                            self.bump();
                            Kind::Url(s)
                        }
                        Some(_) => {
                            self.consume_bad_url_remnants();
                            Kind::BadUrl
                        }
                    };
                }
                Some('"' | '\'' | '(') => {
                    self.consume_bad_url_remnants();
                    return Kind::BadUrl;
                }
                Some(c) if (c as u32) <= 0x08 || c == '\x0B' || (0x0E..=0x1F).contains(&(c as u32)) || c == '\x7F' => {
                    self.consume_bad_url_remnants();
                    return Kind::BadUrl;
                }
                Some('\\') => {
                    if self.starts_escape(self.pos) {
                        self.bump();
                        s.push(self.consume_escape());
                    } else {
                        self.consume_bad_url_remnants();
                        return Kind::BadUrl;
                    }
                }
                Some(_) => s.push(self.bump()),
            }
        }
    }

    fn consume_bad_url_remnants(&mut self) {
        loop {
            match self.ch() {
                None => return,
                Some(')') => {
                    self.bump();
                    return;
                }
                Some('\\') if self.starts_escape(self.pos) => {
                    self.bump();
                    self.consume_escape();
                }
                Some(_) => {
                    self.bump();
                }
            }
        }
    }
}

/// Index just after the block that opens at `toks[i]` (`(`, `[`, `{` or a function token), and whether
/// its closing token was found. Nesting is tracked with an explicit stack, so deep input cannot overflow.
/// Only the innermost open block's closer ends a block; a mismatched closer is ordinary content here.
pub(crate) fn block_end(toks: &[Token], i: usize) -> (usize, bool) {
    let mut stack: Vec<fn(&Kind) -> bool> = Vec::new();
    let mut j = i;
    while j < toks.len() {
        match closer_for(&toks[j].kind) {
            Some(expect) => stack.push(expect),
            None => {
                if stack.last().is_some_and(|expect| expect(&toks[j].kind)) {
                    stack.pop();
                    if stack.is_empty() {
                        return (j + 1, true);
                    }
                }
            }
        }
        j += 1;
    }
    (toks.len(), false)
}

/// For a block opener, the predicate that recognises its closer.
fn closer_for(k: &Kind) -> Option<fn(&Kind) -> bool> {
    match k {
        Kind::LParen | Kind::Function(_) => Some(|k| matches!(k, Kind::RParen)),
        Kind::LSquare => Some(|k| matches!(k, Kind::RSquare)),
        Kind::LCurly => Some(|k| matches!(k, Kind::RCurly)),
        _ => None,
      }
}

/// True for `(`, `[`, `{` and function tokens: the tokens that open a block.
pub(crate) fn opens_block(k: &Kind) -> bool {
    matches!(k, Kind::LParen | Kind::LSquare | Kind::LCurly | Kind::Function(_))
}

/// Maximum block nesting depth inside `toks` (0 for no blocks).
pub(crate) fn max_block_depth(toks: &[Token]) -> usize {
    let mut stack: Vec<fn(&Kind) -> bool> = Vec::new();
    let mut max = 0;
    for t in toks {
        match closer_for(&t.kind) {
            Some(expect) => stack.push(expect),
            None => {
                if stack.last().is_some_and(|expect| expect(&t.kind)) {
                    stack.pop();
                }
            }
        }
        max = max.max(stack.len());
    }
    max
}

/// Slice of `toks` without leading and trailing whitespace tokens.
pub(crate) fn trim_ws(toks: &[Token]) -> &[Token] {
    let mut a = 0;
    let mut b = toks.len();
    while a < b && matches!(toks[a].kind, Kind::Whitespace) {
        a += 1;
    }
    while b > a && matches!(toks[b - 1].kind, Kind::Whitespace) {
        b -= 1;
    }
    &toks[a..b]
}

/// Index of the first top-level token at or after `i` for which `stop` holds, skipping whole blocks.
/// Returns `toks.len()` when there is none.
pub(crate) fn find_top(toks: &[Token], mut i: usize, stop: impl Fn(&Kind) -> bool) -> usize {
    while i < toks.len() {
        if stop(&toks[i].kind) {
            return i;
        }
        i = if opens_block(&toks[i].kind) { block_end(toks, i).0 } else { i + 1 };
    }
    toks.len()
}

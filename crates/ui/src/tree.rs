//! Component values: a declaration's tokens as a tree (functions and brackets nest). Whitespace is kept so that
//! `calc()` can apply its rule that `+` and `-` need whitespace on both sides.

use vn_css::token::{Kind, Token};

#[derive(Debug, Clone, PartialEq)]
pub enum Cv {
    Ws,
    Tok(Token),
    /// A function with its name (without the parenthesis) and its span in the source.
    Func { name: String, span: (usize, usize), args: Vec<Cv> },
    Paren(Vec<Cv>),
    Square(Vec<Cv>),
}

#[derive(Clone, Copy, PartialEq)]
enum Close {
    Top,
    Paren,
    Square,
}

/// Builds the tree. Err(()) for input no valid value can contain: bad strings or urls, unbalanced brackets,
/// `{}` blocks, `;` and `!`.
pub fn build(toks: &[Token]) -> Result<Vec<Cv>, ()> {
    let mut i = 0;
    seq(toks, &mut i, Close::Top)
}

fn seq(toks: &[Token], i: &mut usize, close: Close) -> Result<Vec<Cv>, ()> {
    let mut out = Vec::new();
    while *i < toks.len() {
        let t = &toks[*i];
        *i += 1;
        match &t.kind {
            Kind::Whitespace => out.push(Cv::Ws),
            Kind::Function(name) => {
                let args = seq(toks, i, Close::Paren)?;
                out.push(Cv::Func { name: name.clone(), span: (t.start, toks.get(*i - 1).map_or(t.end, |e| e.end)), args });
            }
            Kind::LParen => out.push(Cv::Paren(seq(toks, i, Close::Paren)?)),
            Kind::LSquare => out.push(Cv::Square(seq(toks, i, Close::Square)?)),
            Kind::RParen => {
                return if close == Close::Paren { Ok(out) } else { Err(()) };
            }
            Kind::RSquare => {
                return if close == Close::Square { Ok(out) } else { Err(()) };
            }
            Kind::BadStr | Kind::BadUrl | Kind::LCurly | Kind::RCurly | Kind::Semicolon => return Err(()),
            _ => out.push(Cv::Tok(t.clone())),
        }
    }
    if close == Close::Top { Ok(out) } else { Err(()) }
}

/// The components without whitespace.
pub fn nows(cvs: &[Cv]) -> Vec<&Cv> {
    cvs.iter().filter(|c| !matches!(c, Cv::Ws)).collect()
}

/// The identifier text of a single-token component.
pub fn ident(c: &Cv) -> Option<&str> {
    match c {
        Cv::Tok(Token { kind: Kind::Ident(s), .. }) => Some(s),
        _ => None,
    }
}

/// True when the components are exactly one identifier (ASCII case-insensitive comparison with `name`).
pub fn is_keyword(cvs: &[Cv], name: &str) -> bool {
    let v = nows(cvs);
    v.len() == 1 && ident(v[0]).is_some_and(|s| s.eq_ignore_ascii_case(name))
}

/// The lower-cased keyword when `c` is an identifier that is one of `options`.
pub fn keyword(c: &Cv, options: &'static [&'static str]) -> Option<&'static str> {
    let s = ident(c)?;
    options.iter().copied().find(|o| o.eq_ignore_ascii_case(s))
}

/// Splits the top-level components at commas (a list of values). Empty items are kept, so the caller can reject them.
pub fn split_commas(cvs: &[Cv]) -> Vec<Vec<Cv>> {
    let mut out = vec![Vec::new()];
    for c in cvs {
        if matches!(c, Cv::Tok(Token { kind: Kind::Comma, .. })) {
            out.push(Vec::new());
        } else {
            out.last_mut().map(|v| v.push(c.clone()));
        }
    }
    out
}

/// The source text of a component list (for error messages and for font-family names).
pub fn text_of(cvs: &[Cv], src: &str) -> String {
    fn walk(cvs: &[Cv], src: &str, out: &mut String) {
        for c in cvs {
            match c {
                Cv::Ws => out.push(' '),
                Cv::Tok(t) => out.push_str(t.text(src)),
                Cv::Func { name, args, .. } => {
                    out.push_str(name);
                    out.push('(');
                    walk(args, src, out);
                    out.push(')');
                }
                Cv::Paren(a) => {
                    out.push('(');
                    walk(a, src, out);
                    out.push(')');
                }
                Cv::Square(a) => {
                    out.push('[');
                    walk(a, src, out);
                    out.push(']');
                }
            }
        }
    }
    let mut s = String::new();
    walk(cvs, src, &mut s);
    s.trim().to_string()
}

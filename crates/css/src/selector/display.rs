//! CSSOM serialisation of selectors (what `CSSStyleRule.selectorText` prints in Chrome): lowercase
//! type names, `a > b` with spaces around the explicit combinators, attribute values always quoted,
//! `:nth-child(2n+1)` in canonical An+B form, and `, ` between the members of a list.

use std::fmt::{self, Write};

use super::{Attr, AttrOp, Combinator, Compound, Nth, Pseudo, Selector, SelectorList, Simple};

impl fmt::Display for SelectorList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, s) in self.selectors.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{s}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                f.write_str(match self.combinators[i - 1] {
                    Combinator::Descendant => " ",
                    Combinator::Child => " > ",
                    Combinator::NextSibling => " + ",
                    Combinator::SubsequentSibling => " ~ ",
                })?;
            }
            write!(f, "{part}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Compound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.type_name {
            Some(name) => f.write_str(&serialize_ident(&name.to_ascii_lowercase()))?,
            None if self.simple.is_empty() => f.write_str("*")?,
            None => {}
        }
        for s in &self.simple {
            match s {
                Simple::Id(v) => write!(f, "#{}", serialize_ident(v))?,
                Simple::Class(v) => write!(f, ".{}", serialize_ident(v))?,
                Simple::Attr(a) => write_attr(f, a)?,
                Simple::Pseudo(p) => write_pseudo(f, p)?,
            }
        }
        Ok(())
    }
}

fn write_attr(f: &mut fmt::Formatter<'_>, a: &Attr) -> fmt::Result {
    write!(f, "[{}", serialize_ident(&a.name.to_ascii_lowercase()))?;
    if let Some(t) = &a.test {
        let op = match t.op {
            AttrOp::Equals => "=",
            AttrOp::Includes => "~=",
            AttrOp::DashMatch => "|=",
            AttrOp::Prefix => "^=",
            AttrOp::Suffix => "$=",
            AttrOp::Substring => "*=",
        };
        write!(f, "{op}{}", serialize_string(&t.value))?;
        if t.insensitive {
            f.write_str(" i")?;
        }
    }
    f.write_str("]")
}

fn write_pseudo(f: &mut fmt::Formatter<'_>, p: &Pseudo) -> fmt::Result {
    let name = match p {
        Pseudo::Hover => "hover",
        Pseudo::Active => "active",
        Pseudo::Focus => "focus",
        Pseudo::FocusVisible => "focus-visible",
        Pseudo::FocusWithin => "focus-within",
        Pseudo::Disabled => "disabled",
        Pseudo::Enabled => "enabled",
        Pseudo::Checked => "checked",
        Pseudo::FirstChild => "first-child",
        Pseudo::LastChild => "last-child",
        Pseudo::OnlyChild => "only-child",
        Pseudo::Empty => "empty",
        Pseudo::Root => "root",
        Pseudo::NthChild(n) => return write!(f, ":nth-child({})", serialize_nth(*n)),
        Pseudo::NthLastChild(n) => return write!(f, ":nth-last-child({})", serialize_nth(*n)),
        Pseudo::Not(args) => {
            f.write_str(":not(")?;
            for (i, c) in args.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                write!(f, "{c}")?;
            }
            return f.write_str(")");
        }
    };
    write!(f, ":{name}")
}

/// CSSOM "serialize an An+B value".
pub(crate) fn serialize_nth(n: Nth) -> String {
    let (a, b) = (n.a, n.b);
    if a == 0 {
        return b.to_string();
    }
    let mut s = match a {
        1 => "n".to_string(),
        -1 => "-n".to_string(),
        _ => format!("{a}n"),
    };
    if b != 0 {
        if b > 0 {
            write!(s, "+{b}").expect("write to string");
        } else {
            write!(s, "{b}").expect("write to string");
        }
    }
    s
}

/// CSSOM "serialize an identifier".
pub(crate) fn serialize_ident(s: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        let c = if c == '\0' { '\u{FFFD}' } else { c };
        let code = c as u32;
        if (0x01..=0x1F).contains(&code) || code == 0x7F || (i == 0 && c.is_ascii_digit()) || (i == 1 && c.is_ascii_digit() && chars[0] == '-') {
            write!(out, "\\{code:x} ").expect("write to string");
        } else if i == 0 && c == '-' && chars.len() == 1 {
            out.push_str("\\-");
        } else if code >= 0x80 || c == '-' || c == '_' || c.is_ascii_alphanumeric() {
            out.push(c);
        } else {
            out.push('\\');
            out.push(c);
        }
    }
    out
}

/// CSSOM "serialize a string": double quotes, with `"` and `\` escaped.
pub(crate) fn serialize_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\a "),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

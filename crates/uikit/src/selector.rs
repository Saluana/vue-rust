//! The selector grammar of testHost.find and findAll (docs/dev/ui-scenario-host.md; tooling/oracle/ui/selector.mjs):
//! one compound selector, `tag? (#id | .class | [attr] | [attr=value] | [attr="value"])* (:nth-child(n))?`. Anything else
//! is rejected, so no scenario relies on a selector feature the Rust host lacks.

/// A parsed selector: an optional tag, the conditions in source order, and the `:nth-child(n)` position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    pub tag: Option<String>,
    pub conds: Vec<Cond>,
    pub nth_child: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cond {
    Id(String),
    Class(String),
    Attr { name: String, value: Option<String> },
}

/// Parses a selector. The error text is the TypeError message of the oracle.
pub fn parse(selector: &str) -> Result<Selector, String> {
    if selector.is_empty() {
        return Err("selector must be a non-empty string".to_string());
    }
    let b = selector.as_bytes();
    let mut pos = 0;
    let mut tag = None;
    if b[0].is_ascii_lowercase() {
        pos = scan(b, 0, |c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-');
        tag = Some(selector[..pos].to_string());
    }
    let mut conds = Vec::new();
    loop {
        if pos == b.len() {
            return Ok(Selector { tag, conds, nth_child: None });
        }
        if let Some((cond, next)) = take_cond(b, pos) {
            conds.push(cond);
            pos = next;
            continue;
        }
        if let Some((n, next)) = take_nth(b, pos) {
            if next == b.len() {
                return Ok(Selector { tag, conds, nth_child: Some(n) });
            }
        }
        break;
    }
    Err(format!(
        "selector \"{selector}\" is outside the oracle grammar (tag, #id, .class, [attr], [attr=value], :nth-child(n))"
    ))
}

/// The end of the run of bytes from `from` that satisfy `keep`.
fn scan(b: &[u8], from: usize, keep: impl Fn(u8) -> bool) -> usize {
    let mut i = from;
    while i < b.len() && keep(b[i]) {
        i += 1;
    }
    i
}

fn take_cond(b: &[u8], pos: usize) -> Option<(Cond, usize)> {
    match b.get(pos)? {
        b'#' => {
            let (name, next) = take_name(b, pos + 1)?;
            Some((Cond::Id(name), next))
        }
        b'.' => {
            let (name, next) = take_name(b, pos + 1)?;
            Some((Cond::Class(name), next))
        }
        b'[' => take_attr(b, pos),
        _ => None,
    }
}

/// `[A-Za-z_][\w-]*`: an id or a class name.
fn take_name(b: &[u8], pos: usize) -> Option<(String, usize)> {
    if !matches!(b.get(pos), Some(c) if c.is_ascii_alphabetic() || *c == b'_') {
        return None;
    }
    let end = scan(b, pos + 1, |c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-');
    Some((String::from_utf8_lossy(&b[pos..end]).into_owned(), end))
}

/// `\[[A-Za-z_][\w:-]*(?:=(?:"[^"\]]*"|'[^'\]]*'|[A-Za-z0-9_-]+))?\]`.
fn take_attr(b: &[u8], pos: usize) -> Option<(Cond, usize)> {
    let start = pos + 1;
    if !matches!(b.get(start), Some(c) if c.is_ascii_alphabetic() || *c == b'_') {
        return None;
    }
    let name_end = scan(b, start + 1, |c| c.is_ascii_alphanumeric() || c == b'_' || c == b':' || c == b'-');
    let name = String::from_utf8_lossy(&b[start..name_end]).into_owned();
    let mut i = name_end;
    let mut value = None;
    if b.get(i) == Some(&b'=') {
        i += 1;
        match b.get(i) {
            Some(&q) if q == b'"' || q == b'\'' => {
                let vstart = i + 1;
                let vend = scan(b, vstart, |c| c != q && c != b']');
                if b.get(vend) != Some(&q) {
                    return None;
                }
                value = Some(String::from_utf8_lossy(&b[vstart..vend]).into_owned());
                i = vend + 1;
            }
            _ => {
                let vend = scan(b, i, |c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-');
                if vend == i {
                    return None;
                }
                value = Some(String::from_utf8_lossy(&b[i..vend]).into_owned());
                i = vend;
            }
        }
    }
    if b.get(i) != Some(&b']') {
        return None;
    }
    Some((Cond::Attr { name, value }, i + 1))
}

/// `:nth-child\(\d+\)`. A number too large for u64 can match no element, so it saturates.
fn take_nth(b: &[u8], pos: usize) -> Option<(u64, usize)> {
    const PREFIX: &[u8] = b":nth-child(";
    if !b[pos..].starts_with(PREFIX) {
        return None;
    }
    let start = pos + PREFIX.len();
    let end = scan(b, start, |c| c.is_ascii_digit());
    if end == start || b.get(end) != Some(&b')') {
        return None;
    }
    let n = std::str::from_utf8(&b[start..end]).ok()?.parse::<u64>().unwrap_or(u64::MAX);
    Some((n, end + 1))
}

impl Selector {
    /// Whether an element matches. `tag` is the element's tag name (case-insensitive), `attr` reads an attribute, and
    /// `child_index` is the 1-based position among the parent's element children (None without a parent element).
    pub fn matches(&self, tag: &str, attr: &dyn Fn(&str) -> Option<String>, child_index: &dyn Fn() -> Option<u64>) -> bool {
        if let Some(t) = &self.tag {
            if !t.eq_ignore_ascii_case(tag) {
                return false;
            }
        }
        let all = self.conds.iter().all(|c| match c {
            Cond::Id(id) => attr("id").as_deref() == Some(id.as_str()),
            Cond::Class(class) => attr("class").is_some_and(|v| v.split_ascii_whitespace().any(|t| t == class)),
            Cond::Attr { name, value: None } => attr(name).is_some(),
            Cond::Attr { name, value: Some(v) } => attr(name).as_deref() == Some(v.as_str()),
        });
        all && self.nth_child.is_none_or(|n| child_index() == Some(n))
    }
}

//! Custom properties and `var()` (CSS Variables Level 1, behaviour checked against Chrome). A custom property
//! inherits down the caller's ancestor chain. A value is substituted from the element's own custom properties,
//! with fallbacks. A cycle makes every member guaranteed-invalid, and so does an undefined reference without a
//! fallback. A guaranteed-invalid value uses the fallback of a `var()` that refers to it. An empty value is valid
//! and is not replaced by a fallback. Substituted text is raw source text, and `/**/` is inserted where two
//! tokens would otherwise merge (the serialisation rule in `needs_comment`).

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{Declaration, TokenList};
use crate::cascade::CascadedDeclarations;
use crate::token::{block_end, tokenize, trim_ws, Kind, Token};

/// A value that is invalid at computed-value time (it becomes `unset` for the property it is applied to).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Invalid;

/// The custom properties of one element. A `None` value is guaranteed-invalid; a name that is absent is
/// also reported as `None`.
#[derive(Debug, Clone, Default)]
pub struct CustomProperties {
    values: BTreeMap<String, Option<TokenList>>,
}

impl CustomProperties {
    /// The computed value of a custom property, or None when it is absent or guaranteed-invalid.
    pub fn get(&self, name: &str) -> Option<&TokenList> {
        self.values.get(name).and_then(|v| v.as_ref())
    }

    /// Substitutes every `var()` in a declaration value of any property. Err(Invalid) means the value is
    /// invalid at computed-value time.
    pub fn substitute(&self, value: &TokenList) -> Result<TokenList, Invalid> {
        let mut src = self.clone();
        let text = subst_text(&value.src, &value.tokens, &mut src)?;
        Ok(TokenList::from_text(&text))
    }
}

/// Computes the custom properties of the last element of `chain`. The chain runs from the root down to the
/// element, one entry per element (so the caller cascades every ancestor). An empty chain gives no values.
pub fn computed_custom_properties(chain: &[&CascadedDeclarations]) -> CustomProperties {
    let mut inherited: BTreeMap<String, Option<TokenList>> = BTreeMap::new();
    for level in chain {
        inherited = compute_level(level, &inherited);
    }
    CustomProperties { values: inherited }
}

/// The computed value of one custom property for the element at the end of `chain`.
pub fn resolve_custom_property(name: &str, chain: &[&CascadedDeclarations]) -> Option<TokenList> {
    computed_custom_properties(chain).get(name).cloned()
}

fn compute_level(level: &CascadedDeclarations, parent: &BTreeMap<String, Option<TokenList>>) -> BTreeMap<String, Option<TokenList>> {
    let declared: BTreeMap<&str, &Declaration> = level.winners().filter(|(n, _)| n.starts_with("--")).map(|(n, c)| (n, c.decl)).collect();
    let mut r = Resolver { declared, parent, memo: BTreeMap::new(), stack: Vec::new(), cyclic: BTreeSet::new() };
    let mut out = parent.clone();
    let names: Vec<&str> = r.declared.keys().copied().collect();
    for name in names {
        let v = r.resolve(name);
        out.insert(name.to_string(), v);
    }
    out
}

/// Something `var()` can read custom property values from.
trait VarSource {
    fn custom(&mut self, name: &str) -> Option<TokenList>;
}

impl VarSource for CustomProperties {
    fn custom(&mut self, name: &str) -> Option<TokenList> {
        self.get(name).cloned()
    }
}

struct Resolver<'p> {
    declared: BTreeMap<&'p str, &'p Declaration>,
    parent: &'p BTreeMap<String, Option<TokenList>>,
    memo: BTreeMap<String, Option<TokenList>>,
    stack: Vec<String>,
    cyclic: BTreeSet<String>,
}

impl Resolver<'_> {
    fn resolve(&mut self, name: &str) -> Option<TokenList> {
        if let Some(v) = self.memo.get(name) {
            return v.clone();
        }
        let Some(decl) = self.declared.get(name).copied() else {
            return self.parent.get(name).cloned().flatten();
        };
        if let Some(pos) = self.stack.iter().position(|n| n == name) {
            let members: Vec<String> = self.stack[pos..].to_vec();
            self.cyclic.extend(members);
            return None;
        }
        self.stack.push(name.to_string());
        let value = self.value_of(decl);
        self.stack.pop();
        let value = if self.cyclic.contains(name) { None } else { value };
        self.memo.insert(name.to_string(), value.clone());
        value
    }

    fn value_of(&mut self, decl: &Declaration) -> Option<TokenList> {
        let t = trim_ws(&decl.value.tokens);
        if let [Token { kind: Kind::Ident(kw), .. }] = t {
            match kw.to_ascii_lowercase().as_str() {
                "initial" => return None,
                "inherit" | "unset" | "revert" => return self.parent.get(decl.name.as_str()).cloned().flatten(),
                _ => {}
            }
        }
        let text = subst_text(&decl.value.src, &decl.value.tokens, self).ok()?;
        Some(TokenList::from_text(&text))
    }
}

impl VarSource for Resolver<'_> {
    fn custom(&mut self, name: &str) -> Option<TokenList> {
        self.resolve(name)
    }
}

/// Serialises a value. Without a var() the raw source text is kept (Chrome does the same). With one, the tokens
/// are re-serialised: each token keeps the whitespace and comments between it and its neighbour, and a `/**/` is
/// inserted between directly adjacent tokens that would merge or lose their identity (see `needs_comment`).
fn subst_text<S: VarSource>(src: &str, toks: &[Token], s: &mut S) -> Result<String, Invalid> {
    let (Some(first), Some(last)) = (toks.first(), toks.last()) else {
        return Ok(String::new());
    };
    // A hex escape takes one whitespace character as its terminator; a value never ends in whitespace.
    if !toks.iter().any(is_var) {
        return Ok(src[first.start..last.end].trim_end().to_string());
    }
    let mut out = String::new();
    let mut cursor = first.start;
    // The last emitted token: its text, and its end in the source.
    let mut prev: Option<(String, usize)> = None;
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        let (text, end, next) = if is_var(t) {
            let (end, closed) = block_end(toks, i);
            if !closed {
                return Err(Invalid);
            }
            let rep = var_replacement(src, &toks[i + 1..end - 1], s)?;
            (rep, toks[end - 1].end, end)
        } else {
            (t.text(src).to_string(), t.end, i + 1)
        };
        out.push_str(&src[cursor..t.start]);
        if !text.is_empty() {
            let is_sub = is_var(t);
            let first_text = if is_sub { first_token_text(&text) } else { text.clone() };
            if let Some((left, left_end)) = &prev {
                if *left_end == t.start && needs_comment(left, &first_text) {
                    out.push_str("/**/");
                }
            }
            let last_text = if is_sub { last_token_text(&text) } else { text.clone() };
            out.push_str(&text);
            prev = Some((last_text, end));
        }
        cursor = end;
        i = next;
    }
    out.push_str(&src[cursor..last.end]);
    Ok(out.trim_end().to_string())
}

fn is_var(t: &Token) -> bool {
    matches!(&t.kind, Kind::Function(n) if n.eq_ignore_ascii_case("var"))
}

/// The value of `var(--name, fallback)`, from the tokens between the parentheses.
fn var_replacement<S: VarSource>(src: &str, inner: &[Token], s: &mut S) -> Result<String, Invalid> {
    let t = trim_ws(inner);
    let name = match t.first().map(|x| &x.kind) {
        Some(Kind::Ident(n)) if n.starts_with("--") => n.clone(),
        _ => return Err(Invalid),
    };
    let rest = trim_ws(&t[1..]);
    let fallback: Option<&[Token]> = match rest.first().map(|x| &x.kind) {
        None => None,
        Some(Kind::Comma) => Some(trim_ws(&rest[1..])),
        _ => return Err(Invalid),
    };
    if let Some(v) = s.custom(&name) {
        return Ok(v.text().to_string());
    }
    match fallback {
        Some(fb) => subst_text(src, fb, s),
        None => Err(Invalid),
    }
}

fn first_token_text(text: &str) -> String {
    tokenize(text).first().map(|t| t.text(text).to_string()).unwrap_or_default()
}

fn last_token_text(text: &str) -> String {
    tokenize(text).last().map(|t| t.text(text).to_string()).unwrap_or_default()
}

/// Whether two directly adjacent tokens need a `/**/` between them. Chrome (checked on a 2000-pair grid):
/// (1) the generic merge: the first token of `left + right` does not end at the boundary;
/// (2) a word-like token (ident, hash, at-keyword, number, dimension) before a signed or dotted number;
/// (3) a delimiter `+`, `-`, `.` or `#` before a signed or dotted number;
/// (4) `@` before `-`.
pub(crate) fn needs_comment(left: &str, right: &str) -> bool {
    if left.is_empty() || right.is_empty() {
        return false;
    }
    let joined = format!("{left}{right}");
    if tokenize(&joined).first().is_some_and(|t| t.end != left.len()) {
        return true;
    }
    let left_kind = tokenize(left).into_iter().next().map(|t| t.kind);
    let word_like = matches!(left_kind, Some(Kind::Ident(_) | Kind::Hash { .. } | Kind::AtKeyword(_) | Kind::Dimension { .. } | Kind::Number { .. }));
    let number_next = signed_or_dotted_number(right);
    if word_like && number_next {
        return true;
    }
    if matches!(left, "+" | "-" | "." | "#") && number_next {
        return true;
    }
    left == "@" && (right == "-" || right.starts_with("-->"))
}

/// `+1`, `-1`, `.5`: a number that starts with a sign or a dot and a digit.
fn signed_or_dotted_number(s: &str) -> bool {
    match s.as_bytes() {
        [b'+' | b'-', d, ..] | [b'.', d, ..] => d.is_ascii_digit(),
        _ => false,
    }
}

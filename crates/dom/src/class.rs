//! DOMTokenList for `classList`, backed by the `class` attribute (WP-U0). Behaviour checked against Chrome: writes
//! normalise the attribute to the serialized token set (single spaces, duplicates removed, first occurrence kept);
//! `add` always writes the attribute, even with no tokens; `remove` and a toggle-off on an element without the
//! attribute create nothing; `toggle` with force true on a present token and `contains` do not write.

use crate::error::DomError;
use crate::node::NodeId;
use crate::value::is_ascii_ws;
use crate::Document;

fn tokens_of(attr: Option<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in attr.unwrap_or_default().split(is_ascii_ws).filter(|t| !t.is_empty()) {
        if !out.iter().any(|x| x == t) {
            out.push(t.to_string());
        }
    }
    out
}

/// Empty tokens are a SyntaxError; tokens with ASCII whitespace are an InvalidCharacterError.
fn check_token(t: &str) -> Result<(), DomError> {
    if t.is_empty() {
        return Err(DomError::SyntaxError);
    }
    if t.chars().any(|c| c.is_ascii() && is_ascii_ws(c)) {
        return Err(DomError::InvalidCharacterError);
    }
    Ok(())
}

impl Document {
    fn token_set(&self, id: NodeId) -> Vec<String> {
        tokens_of(self.attr_raw(id, "class"))
    }

    fn write_class(&mut self, id: NodeId, tokens: &[String]) {
        let value = tokens.join(" ");
        self.write_attr(id, "class", &value);
    }

    /// The classes of an element in order (the DOMTokenList contents).
    pub fn class_list(&self, id: NodeId) -> Vec<String> {
        self.token_set(id)
    }

    pub fn class_list_add(&mut self, id: NodeId, tokens: &[&str]) -> Result<(), DomError> {
        self.check(id)?;
        tokens.iter().try_for_each(|t| check_token(t))?;
        let mut set = self.token_set(id);
        for t in tokens {
            if !set.iter().any(|x| x == t) {
                set.push((*t).to_string());
            }
        }
        self.write_class(id, &set);
        Ok(())
    }

    pub fn class_list_remove(&mut self, id: NodeId, tokens: &[&str]) -> Result<(), DomError> {
        self.check(id)?;
        tokens.iter().try_for_each(|t| check_token(t))?;
        if self.attr_raw(id, "class").is_none() {
            return Ok(());
        }
        let mut set = self.token_set(id);
        set.retain(|x| !tokens.contains(&x.as_str()));
        self.write_class(id, &set);
        Ok(())
    }

    /// Never an error for the token value: an empty or spaced token is simply not contained.
    pub fn class_list_contains(&self, id: NodeId, token: &str) -> bool {
        self.token_set(id).iter().any(|x| x == token)
    }

    /// toggle(token, force): returns whether the token is present afterwards.
    pub fn class_list_toggle(&mut self, id: NodeId, token: &str, force: Option<bool>) -> Result<bool, DomError> {
        self.check(id)?;
        check_token(token)?;
        let mut set = self.token_set(id);
        let present = set.iter().any(|x| x == token);
        if present {
            if force == Some(true) {
                return Ok(true);
            }
            set.retain(|x| x != token);
            self.write_class(id, &set);
            Ok(false)
        } else if force == Some(false) {
            Ok(false)
        } else {
            set.push(token.to_string());
            self.write_class(id, &set);
            Ok(true)
        }
    }
}

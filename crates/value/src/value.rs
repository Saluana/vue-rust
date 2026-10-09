//! Core value types: `Value`, `ObjRef`, `Sym`, `PropKey`, `Throw`, `Res`. No heap access here.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::jsstr::JsStr;
use crate::number;

/// Handle to a heap object. Generation-checked: a stale handle is detected, never silently reused.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ObjRef {
    pub(crate) slot: u32,
    pub(crate) r#gen: u32,
}

/// Runtime symbol id. Symbols are immortal; the description lives in the runtime's symbol table.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Sym(pub(crate) u32);

impl Sym {
    pub fn index(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    String(JsStr),
    Symbol(Sym),
    Object(ObjRef),
    /// Internal TDZ marker. Never visible to user code.
    Uninit,
    /// Internal array hole marker. Never visible to user code.
    Hole,
}

impl Default for Value {
    fn default() -> Self {
        Value::Undefined
    }
}

impl Value {
    pub fn str(s: &str) -> Value {
        Value::String(JsStr::from_str(s))
    }

    pub fn is_undefined(&self) -> bool {
        matches!(self, Value::Undefined)
    }

    pub fn is_nullish(&self) -> bool {
        matches!(self, Value::Undefined | Value::Null)
    }

    pub fn as_object(&self) -> Option<ObjRef> {
        match self {
            Value::Object(o) => Some(*o),
            _ => None,
        }
    }

    pub fn is_object(&self) -> bool {
        matches!(self, Value::Object(_))
    }
}

/// Property key. Invariant: `Str` never spells a canonical array index; build keys with
/// `PropKey::from_jsstr` (or `Rt::key`) so the invariant holds.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PropKey {
    Index(u32),
    Str(JsStr),
    Sym(Sym),
}

impl PropKey {
    pub fn from_jsstr(s: JsStr) -> PropKey {
        match s.array_index() {
            Some(i) => PropKey::Index(i),
            None => PropKey::Str(s),
        }
    }

    pub fn from_str(s: &str) -> PropKey {
        PropKey::from_jsstr(JsStr::from_str(s))
    }

    /// `PropKey::from_str(s)` for a string literal, encoded once per thread. The cache is keyed by the address and
    /// length of `s`: a `'static` string never changes its bytes, so equal keys mean equal text.
    pub fn lit(s: &'static str) -> PropKey {
        let id = (s.as_ptr() as usize, s.len());
        LIT_KEYS.with(|cache| cache.borrow_mut().entry(id).or_insert_with(|| PropKey::from_str(s)).clone())
    }

    pub fn is_index(&self) -> bool {
        matches!(self, PropKey::Index(_))
    }
}

thread_local! {
    static LIT_KEYS: RefCell<HashMap<(usize, usize), PropKey>> = RefCell::new(HashMap::new());
}

/// A thrown JavaScript value. `Err(Throw)` is JavaScript `throw`; Rust panics are framework bugs.
#[derive(Clone, Debug, PartialEq)]
pub struct Throw(pub Value);

pub type Res<T> = Result<T, Throw>;

impl Value {
    /// ToBoolean (ECMA-262 7.1.2).
    pub fn to_boolean(&self) -> bool {
        match self {
            Value::Undefined | Value::Null | Value::Uninit | Value::Hole => false,
            Value::Bool(b) => *b,
            Value::Number(n) => !(*n == 0.0 || n.is_nan()),
            Value::String(s) => !s.is_empty(),
            Value::Symbol(_) | Value::Object(_) => true,
        }
    }
}

/// SameValue (Object.is): numbers by `number::same_value`, everything else by identity or content.
pub fn same_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Undefined, Value::Undefined) | (Value::Null, Value::Null) => true,
        (Value::Uninit, Value::Uninit) | (Value::Hole, Value::Hole) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Number(x), Value::Number(y)) => number::same_value(*x, *y),
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Symbol(x), Value::Symbol(y)) => x == y,
        (Value::Object(x), Value::Object(y)) => x == y,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lit_is_from_str_for_every_kind_of_name() {
        let names: [&'static str; 14] = [
            "value", "length", "__v_skip", "", "0", "1", "42", "4294967294", "4294967295", "01", "-1", "\u{e9}",
            "a\u{1F600}b", "\u{FFFD}",
        ];
        for name in names {
            // The first call encodes the name; the second one reads the cache. Both must equal from_str.
            assert_eq!(PropKey::lit(name), PropKey::from_str(name), "key of {name:?}");
            assert_eq!(PropKey::lit(name), PropKey::from_str(name), "cached key of {name:?}");
        }
    }

    #[test]
    fn lit_turns_canonical_indices_into_index_keys() {
        assert_eq!(PropKey::lit("0"), PropKey::Index(0));
        assert_eq!(PropKey::lit("42"), PropKey::Index(42));
        assert_eq!(PropKey::lit("4294967294"), PropKey::Index(4294967294));
        assert_eq!(PropKey::lit("01"), PropKey::from_str("01"));
        assert!(matches!(PropKey::lit("4294967295"), PropKey::Str(_)));
        assert!(matches!(PropKey::lit("value"), PropKey::Str(_)));
    }

    #[test]
    fn lit_keys_of_equal_text_from_different_literals_are_equal() {
        // Two literals with the same text may have different addresses; the keys still compare equal.
        let a: &'static str = "shared";
        let b: &'static str = "shared";
        assert_eq!(PropKey::lit(a), PropKey::lit(b));
        assert_eq!(PropKey::lit(a), PropKey::from_str("shared"));
    }
}

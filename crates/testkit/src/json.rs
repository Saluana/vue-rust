//! Compact JSON writer for trace documents. The text is what `JSON.stringify` produces for the same data:
//! keys keep their insertion order, and strings are written from UTF-16 code units and escaped exactly as
//! `JSON.stringify` escapes them. (The replay reader lives in json_in.rs; this module only writes.)

use std::fmt::Write as _;

/// A JSON value. Keys and string contents are UTF-16 code units, so lone surrogates survive.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    /// A number already in JSON syntax (the encoder writes the text of `String(n)` here).
    Num(String),
    Str(Vec<u16>),
    Arr(Vec<Json>),
    Obj(Vec<(Vec<u16>, Json)>),
}

impl Json {
    /// A string value from Rust text.
    pub fn text(s: &str) -> Json {
        Json::Str(s.encode_utf16().collect())
    }

    /// An object with static keys, in the given order.
    pub fn obj<'a>(pairs: impl IntoIterator<Item = (&'a str, Json)>) -> Json {
        Json::Obj(pairs.into_iter().map(|(k, v)| (k.encode_utf16().collect(), v)).collect())
    }

    /// An unsigned integer as a JSON number.
    pub fn int(n: u64) -> Json {
        Json::Num(n.to_string())
    }

    /// The value of an object's key (the first match), when this is an object.
    pub fn field(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(pairs) => pairs
                .iter()
                .find(|(k, _)| k.iter().copied().eq(key.encode_utf16()))
                .map(|(_, v)| v),
            _ => None,
        }
    }

    /// The items of an array.
    pub fn items(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(items) => Some(items),
            _ => None,
        }
    }

    /// The UTF-16 code units of a string.
    pub fn units(&self) -> Option<&[u16]> {
        match self {
            Json::Str(units) => Some(units),
            _ => None,
        }
    }

    /// The text of a number, when this is a number.
    pub fn num_text(&self) -> Option<&str> {
        match self {
            Json::Num(text) => Some(text),
            _ => None,
        }
    }

    /// Compact JSON text (no whitespace, one line).
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Num(text) => out.push_str(text),
            Json::Str(units) => write_units(out, units),
            Json::Arr(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Json::Obj(pairs) => {
                out.push('{');
                for (i, (key, value)) in pairs.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_units(out, key);
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
        }
    }
}

/// Writes UTF-16 code units as a JSON string literal, escaped like `JSON.stringify`: `"` `\` and the controls
/// `\b \f \n \r \t` by name, other controls as `\u00xx`, lone surrogates as `\udxxx`, all lowercase hex.
/// Valid surrogate pairs and every other code unit are written as UTF-8.
pub fn write_units(out: &mut String, units: &[u16]) {
    out.push('"');
    let mut i = 0;
    while i < units.len() {
        let u = units[i];
        match u {
            0x22 => out.push_str("\\\""),
            0x5c => out.push_str("\\\\"),
            0x08 => out.push_str("\\b"),
            0x0c => out.push_str("\\f"),
            0x0a => out.push_str("\\n"),
            0x0d => out.push_str("\\r"),
            0x09 => out.push_str("\\t"),
            0x00..=0x1f => {
                let _ = write!(out, "\\u{u:04x}");
            }
            0xd800..=0xdbff if units.get(i + 1).is_some_and(|lo| (0xdc00..=0xdfff).contains(lo)) => {
                let lo = units[i + 1];
                let cp = 0x10000 + ((u32::from(u) - 0xd800) << 10) + (u32::from(lo) - 0xdc00);
                out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                i += 1;
            }
            0xd800..=0xdfff => {
                let _ = write!(out, "\\u{u:04x}");
            }
            _ => out.push(char::from_u32(u32::from(u)).unwrap_or('\u{fffd}')),
        }
        i += 1;
    }
    out.push('"');
}

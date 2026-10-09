//! Host values and the JavaScript conversions that the IDL setters apply (ToString, ToBoolean, ToNumber, ToInt32).
//! Number formatting covers integers and decimals in [1e-6, 1e21), which is all the corpora use; other magnitudes use
//! Rust's shortest form and are not JavaScript-exact.

#[derive(Clone, Debug, PartialEq)]
pub enum HostValue {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
}

impl HostValue {
    /// ToString for DOMString arguments: null becomes "null", undefined becomes "undefined".
    pub fn to_dom_string(&self) -> String {
        match self {
            HostValue::Undefined => "undefined".to_string(),
            HostValue::Null => "null".to_string(),
            HostValue::Bool(b) => b.to_string(),
            HostValue::Number(n) => number_to_string(*n),
            HostValue::Str(s) => s.clone(),
        }
    }

    /// `[LegacyNullToEmptyString]` (input and textarea value): null becomes "", undefined becomes "undefined".
    pub fn to_dom_string_null_empty(&self) -> String {
        match self {
            HostValue::Null => String::new(),
            other => other.to_dom_string(),
        }
    }

    /// A nullable DOMString (`DOMString?`: nodeValue, textContent): null and undefined both become "".
    pub fn to_nullable_string(&self) -> String {
        match self {
            HostValue::Null | HostValue::Undefined => String::new(),
            other => other.to_dom_string(),
        }
    }

    pub fn to_boolean(&self) -> bool {
        match self {
            HostValue::Undefined | HostValue::Null => false,
            HostValue::Bool(b) => *b,
            HostValue::Number(n) => *n != 0.0 && !n.is_nan(),
            HostValue::Str(s) => !s.is_empty(),
        }
    }

    pub fn to_number(&self) -> f64 {
        match self {
            HostValue::Undefined => f64::NAN,
            HostValue::Null => 0.0,
            HostValue::Bool(b) => f64::from(u8::from(*b)),
            HostValue::Number(n) => *n,
            HostValue::Str(s) => string_to_number(s),
        }
    }

    /// The `long` IDL conversion: ToInt32(ToNumber(v)).
    pub fn to_long(&self) -> i32 {
        to_int32(self.to_number())
    }
}

pub(crate) fn number_to_string(n: f64) -> String {
    if n.is_nan() {
        return "NaN".to_string();
    }
    if n.is_infinite() {
        return if n > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if n == 0.0 {
        return "0".to_string();
    }
    format!("{n}")
}

pub(crate) fn to_int32(n: f64) -> i32 {
    if !n.is_finite() {
        return 0;
    }
    let m = n.trunc().rem_euclid(4_294_967_296.0);
    if m >= 2_147_483_648.0 {
        (m - 4_294_967_296.0) as i32
    } else {
        m as i32
    }
}

/// JavaScript WhiteSpace and LineTerminator code points (the set trimmed by StringToNumber).
pub(crate) fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}'
            | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

/// ASCII whitespace as used by the HTML and DOM algorithms (tab, LF, FF, CR, space).
pub(crate) fn is_ascii_ws(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{c}' | '\r' | ' ')
}

/// StringToNumber for the forms in the corpora: decimal literals, Infinity, 0x/0o/0b, empty string is 0.
pub(crate) fn string_to_number(s: &str) -> f64 {
    let t = s.trim_matches(is_js_space);
    if t.is_empty() {
        return 0.0;
    }
    for (prefix, radix) in [("0x", 16), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
        if let Some(digits) = t.strip_prefix(prefix) {
            return parse_radix(digits, radix);
        }
    }
    let (sign, body) = match t.as_bytes()[0] {
        b'+' => (1.0, &t[1..]),
        b'-' => (-1.0, &t[1..]),
        _ => (1.0, t),
    };
    if body == "Infinity" {
        return sign * f64::INFINITY;
    }
    if is_decimal_literal(body) {
        return body.parse::<f64>().map(|v| sign * v).unwrap_or(f64::NAN);
    }
    f64::NAN
}

fn parse_radix(digits: &str, radix: u32) -> f64 {
    if digits.is_empty() {
        return f64::NAN;
    }
    let mut v = 0.0;
    for c in digits.chars() {
        match c.to_digit(radix) {
            Some(d) => v = v * f64::from(radix) + f64::from(d),
            None => return f64::NAN,
        }
    }
    v
}

// digits [. digits] [e [+-] digits] with at least one digit before or after the point; no sign (the caller strips it).
fn is_decimal_literal(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    let mut int_digits = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        int_digits += 1;
    }
    let mut frac_digits = 0;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            frac_digits += 1;
        }
    }
    if int_digits + frac_digits == 0 {
        return false;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    i == b.len()
}

/// "Rules for parsing integers" (HTML) with the signed 32-bit range; None is a parse error.
pub(crate) fn parse_html_integer(s: &str) -> Option<i32> {
    let t = s.trim_start_matches(is_ascii_ws);
    let (neg, rest) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let mut v: i64 = 0;
    let mut any = false;
    for d in rest.bytes().take_while(u8::is_ascii_digit) {
        any = true;
        v = v * 10 + i64::from(d - b'0');
        if v > 2_147_483_648 {
            return None;
        }
    }
    if !any {
        return None;
    }
    let v = if neg { -v } else { v };
    i32::try_from(v).ok()
}

/// Strip and collapse ASCII whitespace (the `text` of an option).
pub(crate) fn strip_collapse(s: &str) -> String {
    s.split(is_ascii_ws).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ")
}

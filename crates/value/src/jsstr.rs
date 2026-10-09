//! `JsStr`: an immutable JavaScript string stored as UTF-16 code units. Lone surrogates are legal.
//! Methods follow ECMA-262 `String.prototype.*` on code units. Index and count parameters are already
//! clamped `usize` values: the caller performs ToIntegerOrInfinity and clamping.

use std::cmp::Ordering;
use std::fmt;
use std::rc::Rc;

/// Longest string the runtime builds, in code units. `repeat` and `pad_*` fail above it.
pub const MAX_LEN: usize = 1 << 30;

/// The result would exceed `MAX_LEN` (JavaScript RangeError: Invalid string length).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StringTooLong;

impl fmt::Display for StringTooLong {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "string length exceeds {MAX_LEN} code units")
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct JsStr(Rc<[u16]>);

impl Default for JsStr {
    fn default() -> Self {
        JsStr::empty()
    }
}

impl fmt::Debug for JsStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"")?;
        for r in char::decode_utf16(self.0.iter().copied()) {
            match r {
                Ok(c) => {
                    for e in c.escape_debug() {
                        write!(f, "{e}")?;
                    }
                }
                Err(e) => write!(f, "\\u{{{:x}}}", e.unpaired_surrogate())?,
            }
        }
        write!(f, "\"")
    }
}

impl fmt::Display for JsStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_rust_string_lossy())
    }
}

/// WhiteSpace and LineTerminator (ECMA-262 12.2, 12.3): TAB VT FF SP NBSP ZWNBSP, the Unicode Zs
/// category, LF CR LS PS. U+180E and U+200B are not included (they are not Zs in current Unicode).
pub(crate) fn is_js_space(u: u16) -> bool {
    matches!(
        u,
        0x09..=0x0D | 0x20 | 0xA0 | 0x1680 | 0x2000..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000 | 0xFEFF
    )
}

pub(crate) fn trim_units(u: &[u16], start: bool, end: bool) -> &[u16] {
    let mut a = 0;
    let mut b = u.len();
    if start {
        while a < b && is_js_space(u[a]) {
            a += 1;
        }
    }
    if end {
        while b > a && is_js_space(u[b - 1]) {
            b -= 1;
        }
    }
    &u[a..b]
}

fn is_high(u: u16) -> bool {
    (0xD800..=0xDBFF).contains(&u)
}

fn is_low(u: u16) -> bool {
    (0xDC00..=0xDFFF).contains(&u)
}

fn push_mapped(run: &mut String, out: &mut Vec<u16>, upper: bool) {
    if run.is_empty() {
        return;
    }
    let mapped = if upper { run.to_uppercase() } else { run.to_lowercase() };
    out.extend(mapped.encode_utf16());
    run.clear();
}

/// GetSubstitution (ECMA-262 22.1.3.19.1) for a string pattern: there are no captures, so `$n` and
/// `$<name>` stay literal.
fn get_substitution(matched: &[u16], s: &[u16], position: usize, rep: &[u16], out: &mut Vec<u16>) {
    const DOLLAR: u16 = 0x24;
    let mut i = 0;
    while i < rep.len() {
        if rep[i] == DOLLAR && i + 1 < rep.len() {
            let handled = match rep[i + 1] {
                0x24 => {
                    out.push(DOLLAR);
                    true
                }
                0x26 => {
                    out.extend_from_slice(matched);
                    true
                }
                0x60 => {
                    out.extend_from_slice(&s[..position]);
                    true
                }
                0x27 => {
                    let tail = (position + matched.len()).min(s.len());
                    out.extend_from_slice(&s[tail..]);
                    true
                }
                _ => false,
            };
            if handled {
                i += 2;
                continue;
            }
        }
        out.push(rep[i]);
        i += 1;
    }
}

impl JsStr {
    pub fn empty() -> JsStr {
        JsStr::from_vec(Vec::new())
    }

    pub fn from_str(s: &str) -> JsStr {
        JsStr::from_vec(s.encode_utf16().collect())
    }

    pub fn from_units(u: &[u16]) -> JsStr {
        JsStr(Rc::from(u))
    }

    pub fn from_vec(v: Vec<u16>) -> JsStr {
        JsStr(Rc::from(v))
    }

    pub fn units(&self) -> &[u16] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Lossy conversion: each lone surrogate becomes U+FFFD.
    pub fn to_rust_string_lossy(&self) -> String {
        String::from_utf16_lossy(&self.0)
    }

    /// `None` when the string contains a lone surrogate.
    pub fn try_to_string(&self) -> Option<String> {
        String::from_utf16(&self.0).ok()
    }

    pub fn concat(&self, other: &JsStr) -> JsStr {
        let mut v = Vec::with_capacity(self.len() + other.len());
        v.extend_from_slice(&self.0);
        v.extend_from_slice(&other.0);
        JsStr::from_vec(v)
    }

    pub fn code_unit_at(&self, i: usize) -> Option<u16> {
        self.0.get(i).copied()
    }

    /// CodePointAt: the code point starting at `i` and the number of code units it spans (2 for a valid
    /// surrogate pair, else 1; a lone surrogate yields its own value).
    pub fn code_point_at(&self, i: usize) -> Option<(u32, usize)> {
        let u = *self.0.get(i)?;
        if is_high(u) {
            if let Some(&l) = self.0.get(i + 1) {
                if is_low(l) {
                    let cp = 0x10000 + ((u as u32 - 0xD800) << 10) + (l as u32 - 0xDC00);
                    return Some((cp, 2));
                }
            }
        }
        Some((u as u32, 1))
    }

    /// `substring`: clamps both ends to the length and swaps them when `start > end`.
    pub fn substring(&self, start: usize, end: usize) -> JsStr {
        let a = start.min(self.len());
        let b = end.min(self.len());
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        JsStr::from_units(&self.0[lo..hi])
    }

    /// `indexOf`: the first match at or after `from`. An empty search string matches at `from`.
    pub fn index_of(&self, search: &JsStr, from: usize) -> Option<usize> {
        let (h, n) = (&self.0[..], &search.0[..]);
        let from = from.min(h.len());
        if n.is_empty() {
            return Some(from);
        }
        if n.len() > h.len() {
            return None;
        }
        (from..=h.len() - n.len()).find(|&i| h[i..i + n.len()] == *n)
    }

    /// `lastIndexOf`: the last match starting at or before `from`.
    pub fn last_index_of(&self, search: &JsStr, from: usize) -> Option<usize> {
        let (h, n) = (&self.0[..], &search.0[..]);
        if n.len() > h.len() {
            return None;
        }
        let start = from.min(h.len() - n.len());
        (0..=start).rev().find(|&i| h[i..i + n.len()] == *n)
    }

    pub fn starts_with(&self, search: &JsStr, pos: usize) -> bool {
        let (h, n) = (&self.0[..], &search.0[..]);
        let pos = pos.min(h.len());
        pos + n.len() <= h.len() && h[pos..pos + n.len()] == *n
    }

    pub fn ends_with(&self, search: &JsStr, end_pos: usize) -> bool {
        let (h, n) = (&self.0[..], &search.0[..]);
        let end = end_pos.min(h.len());
        n.len() <= end && h[end - n.len()..end] == *n
    }

    pub fn includes(&self, search: &JsStr, pos: usize) -> bool {
        self.index_of(search, pos).is_some()
    }

    /// `toLowerCase`: full Unicode mapping including final sigma (Rust's `str::to_lowercase`). Lone
    /// surrogates pass through unchanged and separate the runs that are mapped.
    pub fn to_lower_case(&self) -> JsStr {
        self.map_case(false)
    }

    /// `toUpperCase`: full Unicode mapping, so `ß` becomes `SS`.
    pub fn to_upper_case(&self) -> JsStr {
        self.map_case(true)
    }

    fn map_case(&self, upper: bool) -> JsStr {
        let mut out = Vec::with_capacity(self.len());
        let mut run = String::new();
        for r in char::decode_utf16(self.0.iter().copied()) {
            match r {
                Ok(c) => run.push(c),
                Err(e) => {
                    push_mapped(&mut run, &mut out, upper);
                    out.push(e.unpaired_surrogate());
                }
            }
        }
        push_mapped(&mut run, &mut out, upper);
        JsStr::from_vec(out)
    }

    pub fn trim(&self) -> JsStr {
        JsStr::from_units(trim_units(&self.0, true, true))
    }

    pub fn trim_start(&self) -> JsStr {
        JsStr::from_units(trim_units(&self.0, true, false))
    }

    pub fn trim_end(&self) -> JsStr {
        JsStr::from_units(trim_units(&self.0, false, true))
    }

    /// `padStart`. The filler repeats `fill` and is truncated; an empty `fill` returns the string.
    pub fn pad_start(&self, target_len: usize, fill: &JsStr) -> Result<JsStr, StringTooLong> {
        self.pad(target_len, fill, true)
    }

    pub fn pad_end(&self, target_len: usize, fill: &JsStr) -> Result<JsStr, StringTooLong> {
        self.pad(target_len, fill, false)
    }

    fn pad(&self, target_len: usize, fill: &JsStr, at_start: bool) -> Result<JsStr, StringTooLong> {
        if target_len <= self.len() || fill.is_empty() {
            return Ok(self.clone());
        }
        if target_len > MAX_LEN {
            return Err(StringTooLong);
        }
        let need = target_len - self.len();
        let mut filler = Vec::with_capacity(need);
        while filler.len() < need {
            let take = (need - filler.len()).min(fill.len());
            filler.extend_from_slice(&fill.0[..take]);
        }
        let mut out = Vec::with_capacity(target_len);
        if at_start {
            out.extend(filler);
            out.extend_from_slice(&self.0);
        } else {
            out.extend_from_slice(&self.0);
            out.extend(filler);
        }
        Ok(JsStr::from_vec(out))
    }

    /// `repeat(n)`. The caller has already applied ToIntegerOrInfinity and rejected negative or
    /// infinite counts with a RangeError.
    pub fn repeat(&self, n: usize) -> Result<JsStr, StringTooLong> {
        if n == 0 || self.is_empty() {
            return Ok(JsStr::empty());
        }
        let total = self
            .len()
            .checked_mul(n)
            .filter(|&t| t <= MAX_LEN)
            .ok_or(StringTooLong)?;
        let mut out = Vec::with_capacity(total);
        for _ in 0..n {
            out.extend_from_slice(&self.0);
        }
        Ok(JsStr::from_vec(out))
    }

    /// `split(separator, limit)` with a string separator. `limit` is ToUint32 of the argument; pass
    /// `u32::MAX` when the argument is undefined. An empty separator splits into code units.
    pub fn split_by_string(&self, sep: &JsStr, limit: u32) -> Vec<JsStr> {
        let lim = limit as usize;
        if lim == 0 {
            return Vec::new();
        }
        let h = &self.0[..];
        if sep.is_empty() {
            return h.iter().take(lim).map(|&u| JsStr::from_units(&[u])).collect();
        }
        let mut out = Vec::new();
        let mut p = 0;
        loop {
            match self.index_of(sep, p) {
                Some(i) => {
                    out.push(JsStr::from_units(&h[p..i]));
                    if out.len() == lim {
                        return out;
                    }
                    p = i + sep.len();
                }
                None => {
                    out.push(JsStr::from_units(&h[p..]));
                    return out;
                }
            }
        }
    }

    /// `replace(pattern, replacement)` with a string pattern: replaces the first occurrence only.
    pub fn replace_first(&self, pattern: &JsStr, replacement: &JsStr) -> JsStr {
        let Some(i) = self.index_of(pattern, 0) else {
            return self.clone();
        };
        let mut out = Vec::with_capacity(self.len() + replacement.len());
        out.extend_from_slice(&self.0[..i]);
        get_substitution(&pattern.0, &self.0, i, &replacement.0, &mut out);
        out.extend_from_slice(&self.0[i + pattern.len()..]);
        JsStr::from_vec(out)
    }

    /// `replaceAll(pattern, replacement)` with a string pattern. Matches do not overlap; an empty
    /// pattern matches at every code unit boundary, including both ends.
    pub fn replace_all(&self, pattern: &JsStr, replacement: &JsStr) -> JsStr {
        let advance = pattern.len().max(1);
        let mut positions = Vec::new();
        let mut p = 0;
        while let Some(i) = self.index_of(pattern, p) {
            positions.push(i);
            p = i + advance;
            if p > self.len() {
                break;
            }
        }
        if positions.is_empty() {
            return self.clone();
        }
        let mut out = Vec::with_capacity(self.len());
        let mut end = 0;
        for &i in &positions {
            out.extend_from_slice(&self.0[end..i]);
            get_substitution(&pattern.0, &self.0, i, &replacement.0, &mut out);
            end = i + pattern.len();
        }
        out.extend_from_slice(&self.0[end..]);
        JsStr::from_vec(out)
    }

    /// Relational comparison: lexicographic by code unit.
    pub fn compare(&self, other: &JsStr) -> Ordering {
        self.0[..].cmp(&other.0[..])
    }

    /// String.fromCharCode for code units that are already ToUint16 values.
    pub fn from_char_code(units: &[u16]) -> JsStr {
        JsStr::from_units(units)
    }

    /// String.fromCodePoint. `None` for values above U+10FFFF (JavaScript RangeError). Surrogate code
    /// points are accepted and become lone surrogates.
    pub fn from_code_point(cp: u32) -> Option<JsStr> {
        if cp > 0x10FFFF {
            return None;
        }
        if cp < 0x10000 {
            return Some(JsStr::from_vec(vec![cp as u16]));
        }
        let c = cp - 0x10000;
        Some(JsStr::from_vec(vec![
            (0xD800 + (c >> 10)) as u16,
            (0xDC00 + (c & 0x3FF)) as u16,
        ]))
    }

    pub fn is_well_formed(&self) -> bool {
        char::decode_utf16(self.0.iter().copied()).all(|r| r.is_ok())
    }

    /// Replaces each lone surrogate with U+FFFD.
    pub fn to_well_formed(&self) -> JsStr {
        if self.is_well_formed() {
            return self.clone();
        }
        let mut out = Vec::with_capacity(self.len());
        for r in char::decode_utf16(self.0.iter().copied()) {
            match r {
                Ok(c) => {
                    let mut buf = [0u16; 2];
                    out.extend_from_slice(c.encode_utf16(&mut buf));
                }
                Err(_) => out.push(0xFFFD),
            }
        }
        JsStr::from_vec(out)
    }

    /// The canonical array index this string spells: no sign, no leading zeros, value below 2^32 - 1.
    pub fn array_index(&self) -> Option<u32> {
        let u = &self.0[..];
        if u.is_empty() || u.len() > 10 || (u.len() > 1 && u[0] == 0x30) {
            return None;
        }
        let mut v: u64 = 0;
        for &c in u {
            if !(0x30..=0x39).contains(&c) {
                return None;
            }
            v = v * 10 + (c - 0x30) as u64;
        }
        if v < 4_294_967_295 { Some(v as u32) } else { None }
    }

    /// String iterator: each item is one code point (a valid surrogate pair is one item; a lone
    /// surrogate is its own item).
    pub fn code_points(&self) -> CodePoints<'_> {
        CodePoints { units: &self.0, pos: 0 }
    }
}

/// Iterator returned by [`JsStr::code_points`].
pub struct CodePoints<'a> {
    units: &'a [u16],
    pos: usize,
}

impl Iterator for CodePoints<'_> {
    type Item = JsStr;

    fn next(&mut self) -> Option<JsStr> {
        let u = self.units;
        let i = self.pos;
        if i >= u.len() {
            return None;
        }
        let n = if is_high(u[i]) && i + 1 < u.len() && is_low(u[i + 1]) { 2 } else { 1 };
        self.pos += n;
        Some(JsStr::from_units(&u[i..i + n]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> JsStr {
        JsStr::from_str(text)
    }

    fn units(u: &[u16]) -> JsStr {
        JsStr::from_units(u)
    }

    #[test]
    fn lone_surrogates_are_kept_and_detected() {
        let j = units(&[0xD800, 0x61]);
        assert_eq!(j.units(), &[0xD800, 0x61]);
        assert!(!j.is_well_formed());
        assert_eq!(j.to_well_formed().units(), &[0xFFFD, 0x61]);
        assert_eq!(format!("{j:?}"), "\"\\u{d800}a\"");
    }

    #[test]
    fn code_point_at_pairs_and_bounds() {
        let j = s("\u{1F600}x");
        assert_eq!(j.len(), 3);
        assert_eq!(j.code_point_at(0), Some((0x1F600, 2)));
        assert_eq!(j.code_point_at(1), Some((0xDE00, 1)));
        assert_eq!(j.code_point_at(3), None);
        assert_eq!(units(&[0xDE00]).code_point_at(0), Some((0xDE00, 1)));
    }

    #[test]
    fn trim_uses_js_whitespace_only() {
        assert_eq!(s("\u{feff}\u{3000} a \u{2028}").trim().units(), s("a").units());
        // U+180E is not whitespace for JavaScript since ES2016.
        assert_eq!(s("\u{180e}a").trim().units(), s("\u{180e}a").units());
        assert_eq!(s("  a  ").trim_start().units(), s("a  ").units());
        assert_eq!(s("  a  ").trim_end().units(), s("  a").units());
    }

    #[test]
    fn case_mapping_special_casing_and_lone_surrogates() {
        assert_eq!(s("\u{df}").to_upper_case().units(), s("SS").units());
        assert_eq!(s("\u{130}").to_lower_case().units(), s("i\u{307}").units());
        // Final sigma: capital sigma at the end of a word lowers to U+03C2.
        assert_eq!(s("\u{391}\u{3a3}").to_lower_case().units(), s("\u{3b1}\u{3c2}").units());
        assert_eq!(units(&[0xD800, 0x41]).to_lower_case().units(), &[0xD800, 0x61]);
    }

    #[test]
    fn replace_applies_get_substitution_without_captures() {
        let r = s("abc").replace_first(&s("b"), &s("[$&|$`|$'|$$|$1|$<a>]"));
        assert_eq!(r.units(), s("a[b|a|c|$|$1|$<a>]c").units());
    }

    #[test]
    fn replace_all_with_empty_pattern_matches_every_boundary() {
        assert_eq!(s("ab").replace_all(&s(""), &s("-")).units(), s("-a-b-").units());
        assert_eq!(s("").replace_all(&s(""), &s("X")).units(), s("X").units());
        assert_eq!(s("aaaa").replace_all(&s("aa"), &s("b")).units(), s("bb").units());
    }

    #[test]
    fn split_limits_and_empty_cases() {
        let got: Vec<JsStr> = s("a,b,c").split_by_string(&s(","), 2);
        assert_eq!(got, vec![s("a"), s("b")]);
        assert_eq!(s("").split_by_string(&s(","), u32::MAX), vec![s("")]);
        assert!(s("").split_by_string(&s(""), u32::MAX).is_empty());
        assert_eq!(s("abc").split_by_string(&s(""), u32::MAX), vec![s("a"), s("b"), s("c")]);
        assert_eq!(s("a,").split_by_string(&s(","), u32::MAX), vec![s("a"), s("")]);
        assert!(s("a,b").split_by_string(&s(","), 0).is_empty());
    }

    #[test]
    fn pad_repeat_and_limits() {
        assert_eq!(s("abc").pad_start(7, &s("xy")).unwrap().units(), s("xyxyabc").units());
        assert_eq!(s("abc").pad_end(7, &s("xy")).unwrap().units(), s("abcxyxy").units());
        assert_eq!(s("abc").pad_start(9, &s("")).unwrap(), s("abc"));
        assert_eq!(s("abc").pad_start(MAX_LEN + 1, &s("x")), Err(StringTooLong));
        assert_eq!(s("ab").repeat(3).unwrap().units(), s("ababab").units());
        assert_eq!(s("").repeat(usize::MAX).unwrap(), s(""));
        assert_eq!(s("abc").repeat(1 << 29), Err(StringTooLong));
    }

    #[test]
    fn search_with_empty_and_out_of_range_positions() {
        assert_eq!(s("abc").index_of(&s(""), 10), Some(3));
        assert_eq!(s("aXa").last_index_of(&s("a"), 99), Some(2));
        assert_eq!(s("abc").last_index_of(&s("abcd"), 3), None);
        assert!(s("abc").starts_with(&s(""), 10));
        assert!(s("abc").starts_with(&s("c"), 2));
        assert!(!s("abc").starts_with(&s("b"), 2));
        assert!(s("abc").ends_with(&s("bc"), 3));
        assert!(!s("abc").ends_with(&s("bc"), 2));
    }

    #[test]
    fn array_index_is_canonical_only() {
        assert_eq!(s("0").array_index(), Some(0));
        assert_eq!(s("4294967294").array_index(), Some(4294967294));
        assert_eq!(s("4294967295").array_index(), None);
        assert_eq!(s("01").array_index(), None);
        assert_eq!(s("-1").array_index(), None);
        assert_eq!(s("").array_index(), None);
    }

    #[test]
    fn from_code_point_bounds() {
        assert_eq!(JsStr::from_code_point(0x110000), None);
        assert_eq!(JsStr::from_code_point(0x1F600).unwrap().units(), &[0xD83D, 0xDE00]);
        assert_eq!(JsStr::from_code_point(0xD800).unwrap().units(), &[0xD800]);
    }

    #[test]
    fn code_points_yield_pairs_and_lone_surrogates() {
        let j = units(&[0x61, 0xD83D, 0xDE00, 0xDC00]);
        let items: Vec<JsStr> = j.code_points().collect();
        assert_eq!(items, vec![units(&[0x61]), units(&[0xD83D, 0xDE00]), units(&[0xDC00])]);
    }

    #[test]
    fn conversions_to_rust_strings() {
        assert_eq!(s("h\u{e9}").try_to_string().as_deref(), Some("h\u{e9}"));
        assert_eq!(units(&[0xD800]).try_to_string(), None);
        assert_eq!(units(&[0x61, 0xD800]).to_rust_string_lossy(), "a\u{fffd}");
        assert_eq!(format!("{}", units(&[0x61, 0xDC00])), "a\u{fffd}");
    }

    #[test]
    fn default_and_code_unit_ordering() {
        assert_eq!(JsStr::default(), JsStr::empty());
        assert_eq!(s("ab").compare(&s("b")), Ordering::Less);
        assert_eq!(s("b").compare(&s("ab")), Ordering::Greater);
        // Code unit order: U+FFFF sorts after a surrogate pair's high unit.
        assert_eq!(s("\u{ffff}").compare(&s("\u{1F600}")), Ordering::Greater);
    }
}

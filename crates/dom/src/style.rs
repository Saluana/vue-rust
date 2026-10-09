//! CSSOM declarations and the `style` attribute (WP-U0).
//!
//! Supported properties: the table PROPS (about 60 longhands, custom properties `--*`). A name outside the table is an
//! error (UnsupportedStyleProperty). Values are checked against a small grammar per property: an invalid value leaves
//! the declarations unchanged (CSSOM behaviour). Chrome normalises more than this model does, so the corpora avoid:
//! hex and named colours other than the subset in NAMED_COLORS (Chrome rewrites hex as rgb()); shorthands, which
//! Chrome merges (margin, padding, inset, border, background...); and out-of-range numbers. A unitless `0` becomes
//! `0px`, and keyword values and named colours are lowercased, as in Chrome.
//!
//! Ordering: setProperty keeps insertion order, and an updated declaration keeps its position. cssText parsing adds
//! the normal declarations first and then the !important ones, each group in source order, so the serialization
//! matches Chrome (`style_changed` and parse_declarations below).
//!
//! style_set_camel takes the camelCase CSSOM name (`backgroundColor`), converted by camel_to_kebab.
//!
//! The `style` attribute: CSSOM changes update an existing attribute in place. When the attribute is absent, Chrome
//! creates it lazily: it is appended to the attribute list the next time the attributes are read. Here the
//! `style_pending` flag does the same (see sync_style_attribute).

use crate::error::DomError;
use crate::node::{Attr, NodeId};
use crate::Document;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Decl {
    pub name: String,
    pub value: String,
    pub important: bool,
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    /// Length, percentage or unitless zero.
    Len,
    /// Length, percentage, or `auto`.
    LenAuto,
    /// Length, percentage, or `none`.
    LenMax,
    /// Length, or one of the listed keywords.
    LenKw(&'static [&'static str]),
    Color,
    Kw(&'static [&'static str]),
    /// Real number (opacity, flex factors).
    Num,
    Int,
    IntAuto,
}

const KW_DISPLAY: &[&str] = &["block", "inline", "inline-block", "flex", "inline-flex", "grid", "inline-grid", "none", "contents", "table", "table-cell", "list-item", "flow-root"];
const KW_POSITION: &[&str] = &["static", "relative", "absolute", "fixed", "sticky"];
const KW_VISIBILITY: &[&str] = &["visible", "hidden", "collapse"];
const KW_OVERFLOW: &[&str] = &["visible", "hidden", "scroll", "auto", "clip"];
const KW_POINTER: &[&str] = &["auto", "none"];
const KW_BOX_SIZING: &[&str] = &["content-box", "border-box"];
const KW_TEXT_ALIGN: &[&str] = &["left", "right", "center", "justify", "start", "end"];
const KW_TEXT_TRANSFORM: &[&str] = &["none", "capitalize", "uppercase", "lowercase"];
const KW_FLOAT: &[&str] = &["left", "right", "none"];
const KW_FLEX_DIRECTION: &[&str] = &["row", "row-reverse", "column", "column-reverse"];
const KW_WHITE_SPACE: &[&str] = &["normal", "nowrap", "pre", "pre-wrap", "pre-line", "break-spaces"];
const KW_FONT_STYLE: &[&str] = &["normal", "italic", "oblique"];
const KW_FONT_SIZE: &[&str] = &["xx-small", "x-small", "small", "medium", "large", "x-large", "xx-large", "xxx-large", "smaller", "larger"];
const KW_BORDER_WIDTH: &[&str] = &["thin", "medium", "thick"];
const KW_NORMAL: &[&str] = &["normal"];

/// Named colours accepted by the colour grammar (a subset of the CSS list; the rest are unsupported values).
const NAMED_COLORS: &[&str] = &[
    "black", "white", "red", "green", "blue", "yellow", "orange", "purple", "pink", "brown", "gray", "grey", "silver",
    "navy", "teal", "maroon", "olive", "lime", "aqua", "cyan", "fuchsia", "magenta", "indigo", "violet", "gold",
    "transparent", "currentcolor",
];

const PROPS: &[(&str, Kind)] = &[
    ("width", Kind::LenAuto), ("height", Kind::LenAuto), ("min-width", Kind::LenAuto), ("min-height", Kind::LenAuto),
    ("max-width", Kind::LenMax), ("max-height", Kind::LenMax), ("flex-basis", Kind::LenAuto),
    ("top", Kind::LenAuto), ("right", Kind::LenAuto), ("bottom", Kind::LenAuto), ("left", Kind::LenAuto),
    ("margin-top", Kind::LenAuto), ("margin-right", Kind::LenAuto), ("margin-bottom", Kind::LenAuto), ("margin-left", Kind::LenAuto),
    ("padding-top", Kind::Len), ("padding-right", Kind::Len), ("padding-bottom", Kind::Len), ("padding-left", Kind::Len),
    ("border-top-width", Kind::LenKw(KW_BORDER_WIDTH)), ("border-right-width", Kind::LenKw(KW_BORDER_WIDTH)),
    ("border-bottom-width", Kind::LenKw(KW_BORDER_WIDTH)), ("border-left-width", Kind::LenKw(KW_BORDER_WIDTH)),
    ("font-size", Kind::LenKw(KW_FONT_SIZE)), ("text-indent", Kind::Len),
    ("row-gap", Kind::LenKw(KW_NORMAL)), ("column-gap", Kind::LenKw(KW_NORMAL)), ("letter-spacing", Kind::LenKw(KW_NORMAL)),
    ("color", Kind::Color), ("background-color", Kind::Color), ("border-top-color", Kind::Color),
    ("border-right-color", Kind::Color), ("border-bottom-color", Kind::Color), ("border-left-color", Kind::Color),
    ("display", Kind::Kw(KW_DISPLAY)), ("position", Kind::Kw(KW_POSITION)), ("visibility", Kind::Kw(KW_VISIBILITY)),
    ("overflow", Kind::Kw(KW_OVERFLOW)), ("overflow-x", Kind::Kw(KW_OVERFLOW)), ("overflow-y", Kind::Kw(KW_OVERFLOW)),
    ("pointer-events", Kind::Kw(KW_POINTER)), ("box-sizing", Kind::Kw(KW_BOX_SIZING)), ("text-align", Kind::Kw(KW_TEXT_ALIGN)),
    ("text-transform", Kind::Kw(KW_TEXT_TRANSFORM)), ("float", Kind::Kw(KW_FLOAT)), ("flex-direction", Kind::Kw(KW_FLEX_DIRECTION)),
    ("white-space", Kind::Kw(KW_WHITE_SPACE)), ("font-style", Kind::Kw(KW_FONT_STYLE)),
    ("opacity", Kind::Num), ("flex-grow", Kind::Num), ("flex-shrink", Kind::Num), ("z-index", Kind::IntAuto), ("order", Kind::Int),
];

const LENGTH_UNITS: &[&str] = &["px", "em", "rem", "%", "vw", "vh", "vmin", "vmax", "pt", "pc", "cm", "mm", "in", "ex", "ch"];
const CSS_WIDE: &[&str] = &["inherit", "initial", "unset", "revert", "revert-layer"];

fn is_css_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{c}' | '\r')
}

fn is_custom(name: &str) -> bool {
    name.starts_with("--")
}

/// Lowercases ASCII for standard names; custom property names are case-sensitive.
pub(crate) fn normalize_name(name: &str) -> String {
    if is_custom(name) {
        name.to_string()
    } else {
        name.to_ascii_lowercase()
    }
}

fn kind_of(name: &str) -> Option<Kind> {
    PROPS.iter().find(|(n, _)| *n == name).map(|(_, k)| *k)
}

/// Splits a number from the front of `s`: sign, digits, optional fraction and exponent. Returns the rest.
fn split_number(s: &str) -> Option<(&str, &str)> {
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let mut digits = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let start = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > start {
            i = j;
        }
    }
    Some((&s[..i], &s[i..]))
}

fn length(v: &str) -> Option<String> {
    if v == "0" {
        return Some("0px".to_string());
    }
    let (num, unit) = split_number(v)?;
    let unit = unit.to_ascii_lowercase();
    LENGTH_UNITS.contains(&unit.as_str()).then(|| format!("{num}{unit}"))
}

fn number(v: &str) -> Option<String> {
    let (num, rest) = split_number(v)?;
    rest.is_empty().then(|| num.to_string())
}

fn integer(v: &str) -> Option<String> {
    let body = v.strip_prefix(['+', '-']).unwrap_or(v);
    (!body.is_empty() && body.bytes().all(|c| c.is_ascii_digit())).then(|| v.to_string())
}

fn color(v: &str) -> Option<String> {
    let lower = v.to_ascii_lowercase();
    if NAMED_COLORS.contains(&lower.as_str()) {
        return Some(lower);
    }
    if let Some(hex) = v.strip_prefix('#') {
        let ok = matches!(hex.len(), 3 | 4 | 6 | 8) && hex.bytes().all(|c| c.is_ascii_hexdigit());
        return ok.then(|| v.to_string());
    }
    let func = ["rgb(", "rgba(", "hsl(", "hsla("].iter().any(|f| lower.starts_with(f));
    (func && lower.ends_with(')')).then(|| v.to_string())
}

/// The normalised value for a property of `kind`, or None when the value is invalid.
fn check_value(kind: Kind, v: &str) -> Option<String> {
    let lower = v.to_ascii_lowercase();
    if CSS_WIDE.contains(&lower.as_str()) {
        return Some(lower);
    }
    if v.contains("var(") {
        return Some(v.to_string());
    }
    match kind {
        Kind::Len => length(v),
        Kind::LenAuto => (lower == "auto").then_some(lower).or_else(|| length(v)),
        Kind::LenMax => (lower == "none").then_some(lower).or_else(|| length(v)),
        Kind::LenKw(kws) => kws.contains(&lower.as_str()).then_some(lower.clone()).or_else(|| length(v)),
        Kind::Color => color(v),
        Kind::Kw(kws) => kws.contains(&lower.as_str()).then_some(lower),
        Kind::Num => number(v),
        Kind::Int => integer(v),
        Kind::IntAuto => (lower == "auto").then_some(lower).or_else(|| integer(v)),
    }
}

/// CSSOM attribute name to property name: camelCase to kebab-case (`backgroundColor` to `background-color`,
/// `WebkitTransform` to `-webkit-transform`, `cssFloat` to `float`). Kebab-case names are unchanged, and so are custom
/// properties (`--*`), which have no camelCase form.
pub(crate) fn camel_to_kebab(name: &str) -> String {
    if name == "cssFloat" {
        return "float".to_string();
    }
    if is_custom(name) {
        return name.to_string();
    }
    let mut out = String::with_capacity(name.len() + 4);
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The normalised value to store for `name`, or None if the value is invalid. Custom properties accept any value.
fn valid_for(name: &str, v: &str) -> Option<String> {
    if is_custom(name) {
        return Some(v.to_string());
    }
    kind_of(name).and_then(|k| check_value(k, v))
}

/// Splits a declaration list at top-level semicolons (not inside parentheses or quotes).
fn split_declarations(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut quote, mut start) = (0i32, None::<char>, 0usize);
    for (i, c) in text.char_indices() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '(' => depth += 1,
                ')' => depth -= 1,
                ';' if depth <= 0 => {
                    out.push(&text[start..i]);
                    start = i + 1;
                }
                _ => {}
            },
        }
    }
    out.push(&text[start..]);
    out
}

/// One declaration: name, value and importance, before validation. None when the syntax is wrong.
fn parse_one(chunk: &str) -> Option<(String, String, bool)> {
    let (name, value) = chunk.split_once(':')?;
    let name = name.trim_matches(is_css_ws);
    if name.is_empty() {
        return None;
    }
    let mut value = value.trim_matches(is_css_ws);
    let mut important = false;
    if let Some(idx) = value.rfind('!') {
        if value[idx + 1..].trim_matches(is_css_ws).eq_ignore_ascii_case("important") {
            important = true;
            value = value[..idx].trim_end_matches(is_css_ws);
        }
    }
    if value.is_empty() {
        return None;
    }
    Some((normalize_name(name), value.to_string(), important))
}

fn upsert(list: &mut Vec<Decl>, name: String, value: String, important: bool) {
    match list.iter_mut().find(|d| d.name == name) {
        Some(d) => {
            d.value = value;
            d.important = important;
        }
        None => list.push(Decl { name, value, important }),
    }
}

/// cssText parsing: normal declarations first, then the !important ones, each in source order.
pub(crate) fn parse_declarations(text: &str) -> Vec<Decl> {
    let items: Vec<(String, String, bool)> = split_declarations(text)
        .into_iter()
        .filter_map(parse_one)
        .filter_map(|(n, v, imp)| valid_for(&n, &v).map(|nv| (n, nv, imp)))
        .collect();
    let mut list = Vec::new();
    for group in [false, true] {
        for (n, v, imp) in items.iter().filter(|x| x.2 == group) {
            upsert(&mut list, n.clone(), v.clone(), *imp);
        }
    }
    list
}

/// The serialization used for cssText and for the style attribute: "name: value[ !important];" items joined by spaces.
pub(crate) fn serialize_declarations(list: &[Decl]) -> String {
    list.iter()
        .map(|d| format!("{}: {}{};", d.name, d.value, if d.important { " !important" } else { "" }))
        .collect::<Vec<_>>()
        .join(" ")
}

impl Document {
    fn style_target(&self, id: NodeId) -> Result<(), DomError> {
        self.check(id)?;
        if self.n(id).kind != crate::node::NodeKind::Element {
            return Err(DomError::InvalidNodeType);
        }
        Ok(())
    }

    /// CSSOM setProperty(name, value, important). An empty value removes the declaration; an invalid value is ignored.
    pub fn style_set_property(&mut self, id: NodeId, name: &str, value: &str, important: bool) -> Result<(), DomError> {
        self.style_target(id)?;
        let name = normalize_name(name);
        if !is_custom(&name) && kind_of(&name).is_none() {
            return Err(DomError::UnsupportedStyleProperty);
        }
        let v = value.trim_matches(is_css_ws);
        if v.is_empty() {
            self.style_remove_property(id, &name)?;
            return Ok(());
        }
        let Some(stored) = valid_for(&name, v) else { return Ok(()) };
        upsert(&mut self.nm(id).style, name, stored, important);
        self.style_changed(id);
        Ok(())
    }

    /// The camelCase form used by `style.backgroundColor = …` (the CSSOM attribute): converts to the property name and
    /// then behaves exactly like style_set_property. An unknown name is UnsupportedStyleProperty.
    pub fn style_set_camel(&mut self, id: NodeId, name: &str, value: &str, important: bool) -> Result<(), DomError> {
        self.style_set_property(id, &camel_to_kebab(name), value, important)
    }

    /// CSSOM removeProperty: returns the old value ("" when absent, and then nothing changes).
    pub fn style_remove_property(&mut self, id: NodeId, name: &str) -> Result<String, DomError> {
        self.style_target(id)?;
        let name = normalize_name(name);
        let Some(i) = self.n(id).style.iter().position(|d| d.name == name) else { return Ok(String::new()) };
        let old = self.nm(id).style.remove(i).value;
        self.style_changed(id);
        Ok(old)
    }

    pub fn style_get_property(&self, id: NodeId, name: &str) -> Result<String, DomError> {
        self.style_target(id)?;
        let name = normalize_name(name);
        Ok(self.n(id).style.iter().find(|d| d.name == name).map(|d| d.value.clone()).unwrap_or_default())
    }

    /// "important" or "".
    pub fn style_get_priority(&self, id: NodeId, name: &str) -> Result<String, DomError> {
        self.style_target(id)?;
        let name = normalize_name(name);
        Ok(match self.n(id).style.iter().find(|d| d.name == name) {
            Some(d) if d.important => "important".to_string(),
            _ => String::new(),
        })
    }

    pub fn css_text(&self, id: NodeId) -> Result<String, DomError> {
        self.style_target(id)?;
        Ok(serialize_declarations(&self.n(id).style))
    }

    pub fn set_css_text(&mut self, id: NodeId, text: &str) -> Result<(), DomError> {
        self.style_target(id)?;
        self.nm(id).style = parse_declarations(text);
        self.style_changed(id);
        Ok(())
    }

    /// After a CSSOM change: an existing `style` attribute takes the new serialization in place; otherwise the
    /// attribute is created lazily on the next read.
    fn style_changed(&mut self, id: NodeId) {
        let text = serialize_declarations(&self.n(id).style);
        match self.attr_index(id, "style") {
            Some(i) => {
                self.nm(id).attrs[i].value = text;
                self.nm(id).style_pending = false;
            }
            None => self.nm(id).style_pending = true,
        }
    }

    /// Materialises a pending `style` attribute (appended at the end of the attribute list).
    pub(crate) fn sync_style_attribute(&mut self, id: NodeId) {
        if !self.n(id).style_pending {
            return;
        }
        let text = serialize_declarations(&self.n(id).style);
        let n = self.nm(id);
        n.style_pending = false;
        n.attrs.push(Attr { ns: None, prefix: None, local: "style".to_string(), value: text });
    }

    /// Hook: the `style` attribute was added or replaced, so the declarations are re-parsed from it.
    pub(crate) fn style_attribute_changed(&mut self, id: NodeId) {
        let text = self.attr_raw(id, "style").unwrap_or_default();
        let n = self.nm(id);
        n.style = parse_declarations(&text);
        n.style_pending = false;
    }

    /// Hook: the `style` attribute was removed (or was absent and removeAttribute was called): clear the declarations.
    pub(crate) fn style_attribute_removed(&mut self, id: NodeId) {
        let n = self.nm(id);
        n.style.clear();
        n.style_pending = false;
    }
}

//! Media queries: parsing (Media Queries Level 4 grammar, supported features only) and evaluation
//! against a `MediaEnv`. Unknown or unsupported features evaluate to *unknown* (Kleene logic, as in
//! MQ4: `(min-width: 1px) or (foo)` matches; `not (foo)` does not). Unknown at the top level is no match.
//! Negative lengths and resolutions are invalid here (MQ3 forbids them) even though Chrome accepts
//! `(min-width: -1px)`; see the report's open questions.

use crate::diag::{code, Diagnostic};
use crate::token::{block_end, find_top, trim_ws, tokenize, Kind, Token};

/// A media query list. An empty list matches everything (`@media { ... }`).
#[derive(Debug, Clone, PartialEq)]
pub struct MediaQueryList {
    pub queries: Vec<MediaQuery>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaQuery {
    /// `not` before the media type (negates the whole query).
    pub negated: bool,
    pub media_type: MediaType,
    pub condition: Option<Condition>,
    /// False for a syntactically invalid query: it never matches, and a diagnostic says why.
    pub valid: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MediaType {
    All,
    Screen,
    /// There is no printing, so `print` never matches.
    Print,
    /// Any other media type never matches.
    Other(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    Not(Box<Condition>),
    And(Vec<Condition>),
    Or(Vec<Condition>),
    Feature(Feature),
    /// An unsupported, unknown or invalid feature: evaluates to unknown.
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Feature {
    /// Viewport width in CSS px. An empty constraint list is the boolean context `(width)`.
    Width(Vec<(Op, f64)>),
    Height(Vec<(Op, f64)>),
    /// Device pixel ratio in dppx. An empty constraint list is the boolean context.
    Resolution(Vec<(Op, f64)>),
    Orientation(Option<Orientation>),
    ColorScheme(Option<ColorScheme>),
    /// `Some(true)` is `reduce`; `None` is the boolean context.
    ReducedMotion(Option<bool>),
    Hover(Option<HoverCap>),
    AnyHover(Option<HoverCap>),
    Pointer(Option<PointerCap>),
    AnyPointer(Option<PointerCap>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverCap {
    None,
    Hover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerCap {
    None,
    Coarse,
    Fine,
}

/// The device the styles are evaluated for. `dppx` is the device pixel ratio; `em` and `rem` are 16 px.
/// `any-hover` and `any-pointer` take the same values as `hover` and `pointer` (one input device).
#[derive(Debug, Clone, PartialEq)]
pub struct MediaEnv {
    pub width: f64,
    pub height: f64,
    pub color_scheme: ColorScheme,
    pub reduced_motion: bool,
    pub hover: HoverCap,
    pub pointer: PointerCap,
    pub dppx: f64,
}

impl MediaEnv {
    /// A light, full-motion desktop with a mouse at 1 dppx.
    pub fn new(width: f64, height: f64) -> MediaEnv {
        MediaEnv {
            width,
            height,
            color_scheme: ColorScheme::Light,
            reduced_motion: false,
            hover: HoverCap::Hover,
            pointer: PointerCap::Fine,
            dppx: 1.0,
        }
    }
}

/// Parses a media query list from source text.
pub fn parse(src: &str) -> (MediaQueryList, Vec<Diagnostic>) {
    let toks = tokenize(src);
    parse_tokens(&toks, src)
}

/// Parses a media query list from tokens of `src`. Diagnostics carry absolute source spans.
pub fn parse_tokens(toks: &[Token], src: &str) -> (MediaQueryList, Vec<Diagnostic>) {
    let toks = trim_ws(toks);
    let mut diags = Vec::new();
    if toks.is_empty() {
        return (MediaQueryList { queries: Vec::new() }, diags);
    }
    let mut queries = Vec::new();
    let mut i = 0;
    loop {
        let j = find_top(toks, i, |k| matches!(k, Kind::Comma));
        queries.push(parse_query(&toks[i..j], src, &mut diags));
        if j >= toks.len() {
            break;
        }
        i = j + 1;
    }
    (MediaQueryList { queries }, diags)
}

/// True when the list matches `env` (an empty list matches).
pub fn matches(list: &MediaQueryList, env: &MediaEnv) -> bool {
    list.queries.is_empty() || list.queries.iter().any(|q| eval_query(q, env) == Tri::True)
}

fn span_of(toks: &[Token], src: &str) -> (usize, usize) {
    match (toks.first(), toks.last()) {
        (Some(a), Some(b)) => (a.start, b.end),
        _ => (src.len(), src.len()),
    }
}

fn is_ident(k: Option<&Kind>, word: &str) -> bool {
    matches!(k, Some(Kind::Ident(s)) if s.eq_ignore_ascii_case(word))
}

const RESERVED: [&str; 4] = ["and", "or", "not", "only"];

fn invalid(toks: &[Token], src: &str, diags: &mut Vec<Diagnostic>, msg: &str) -> MediaQuery {
    let (s, e) = span_of(toks, src);
    diags.push(Diagnostic::new(code::INVALID_MEDIA_QUERY, msg, "write a valid media query", s, e));
    MediaQuery { negated: false, media_type: MediaType::All, condition: None, valid: false }
}

fn parse_query(part: &[Token], src: &str, diags: &mut Vec<Diagnostic>) -> MediaQuery {
    // Whitespace carries no meaning between media query tokens; spans stay absolute.
    let significant: Vec<Token> = part.iter().filter(|x| !matches!(x.kind, Kind::Whitespace)).cloned().collect();
    let t = &significant[..];
    if t.is_empty() {
        return invalid(part, src, diags, "empty media query");
    }
    // `not` or `only` in front of a media type.
    let (negated, modifier_len) = match &t[0].kind {
        Kind::Ident(s) if s.eq_ignore_ascii_case("not") && matches!(t.get(1).map(|x| &x.kind), Some(Kind::Ident(n)) if !RESERVED.iter().any(|r| n.eq_ignore_ascii_case(r))) => (true, 1),
        Kind::Ident(s) if s.eq_ignore_ascii_case("only") && matches!(t.get(1).map(|x| &x.kind), Some(Kind::Ident(n)) if !RESERVED.iter().any(|r| n.eq_ignore_ascii_case(r))) => (false, 1),
        _ => (false, 0),
    };
    let rest = &t[modifier_len..];
    if let Some(Kind::Ident(name)) = rest.first().map(|x| &x.kind).filter(|_| !is_ident(rest.first().map(|x| &x.kind), "not")) {
        let lower = name.to_ascii_lowercase();
        if RESERVED.contains(&lower.as_str()) {
            return invalid(t, src, diags, "a media type cannot be 'and', 'or', 'not' or 'only'");
        }
        let media_type = match lower.as_str() {
            "all" => MediaType::All,
            "screen" => MediaType::Screen,
            "print" => MediaType::Print,
            _ => {
                let (s, e) = span_of(&rest[..1], src);
                diags.push(Diagnostic::new(code::MEDIA_TYPE, format!("media type '{lower}' is not supported"), "use all, screen or print", s, e));
                MediaType::Other(lower)
            }
        };
        let after = trim_ws(&rest[1..]);
        if after.is_empty() {
            return MediaQuery { negated, media_type, condition: None, valid: true };
        }
        if !is_ident(after.first().map(|x| &x.kind), "and") {
            return invalid(t, src, diags, "expected 'and' after the media type");
        }
        return match parse_condition(trim_ws(&after[1..]), src, diags, false) {
            Some(c) => MediaQuery { negated, media_type, condition: Some(c), valid: true },
            None => invalid(t, src, diags, "invalid media condition"),
        };
    }
    // No media type: a media condition (which may be `not (...)`).
    match parse_condition(t, src, diags, true) {
        Some(c) => MediaQuery { negated: false, media_type: MediaType::All, condition: Some(c), valid: true },
        None => invalid(t, src, diags, "invalid media condition"),
    }
}

/// A media condition. `allow_or` is false after a media type (`screen and ...`), where only `and` may join.
fn parse_condition(t: &[Token], src: &str, diags: &mut Vec<Diagnostic>, allow_or: bool) -> Option<Condition> {
    let t = trim_ws(t);
    if t.is_empty() {
        return None;
    }
    if is_ident(t.first().map(|x| &x.kind), "not") {
        let inner = trim_ws(&t[1..]);
        let (end, _) = group_end(inner)?;
        if end != inner.len() {
            return None;
        }
        return Some(Condition::Not(Box::new(parse_in_parens(inner, src, diags)?)));
    }
    let mut items = Vec::new();
    let mut joiner: Option<&str> = None;
    let mut i = 0;
    loop {
        let (end, _) = group_end(&t[i..])?;
        items.push(parse_in_parens(&t[i..i + end], src, diags)?);
        let rest = trim_ws(&t[i + end..]);
        if rest.is_empty() {
            break;
        }
        let word = match rest.first().map(|x| &x.kind) {
            Some(Kind::Ident(s)) if s.eq_ignore_ascii_case("and") => "and",
            Some(Kind::Ident(s)) if s.eq_ignore_ascii_case("or") && allow_or => "or",
            _ => return None,
        };
        if joiner.is_some_and(|j| j != word) {
            return None;
        }
        joiner = Some(word);
        i = t.len() - rest.len() + 1;
        while matches!(t.get(i).map(|x| &x.kind), Some(Kind::Whitespace)) {
            i += 1;
        }
    }
    Some(match (items.len(), joiner) {
        (1, _) => items.pop().expect("one item"),
        (_, Some("or")) => Condition::Or(items),
        _ => Condition::And(items),
    })
}

/// Length of the first parenthesised or function group in `t` (`(...)` or `name(...)`), and whether it closed.
fn group_end(t: &[Token]) -> Option<(usize, bool)> {
    match t.first()?.kind {
        Kind::LParen | Kind::Function(_) => {
            let (end, closed) = block_end(t, 0);
            closed.then_some((end, true))
        }
        _ => None,
    }
}

/// `( condition )`, `( feature )` or a general-enclosed `name( ... )` (unknown).
fn parse_in_parens(t: &[Token], src: &str, diags: &mut Vec<Diagnostic>) -> Option<Condition> {
    match t.first()?.kind {
        Kind::Function(_) => {
            let (s, e) = span_of(t, src);
            diags.push(Diagnostic::new(code::MEDIA_FEATURE, "unsupported media query function", "remove the function", s, e));
            Some(Condition::Unknown)
        }
        Kind::LParen => {
            let inner = trim_ws(&t[1..t.len() - 1]);
            if inner.is_empty() {
                return None;
            }
            if matches!(inner[0].kind, Kind::LParen | Kind::Function(_)) || is_ident(Some(&inner[0].kind), "not") {
                return parse_condition(inner, src, diags, true);
            }
            Some(parse_feature(inner, src, diags))
        }
        _ => None,
    }
}

fn parse_feature(inner: &[Token], src: &str, diags: &mut Vec<Diagnostic>) -> Condition {
    let colon = find_top(inner, 0, |k| matches!(k, Kind::Colon));
    if colon < inner.len() {
        return feature_with_value(&inner[..colon], trim_ws(&inner[colon + 1..]), src, diags);
    }
    let has_cmp = inner.iter().any(|t| matches!(t.kind, Kind::Delim('<' | '>' | '=')));
    if has_cmp {
        return range_feature(inner, src, diags);
    }
    match inner {
        [only] => match &only.kind {
            Kind::Ident(name) => match name.to_ascii_lowercase().as_str() {
                "width" => Condition::Feature(Feature::Width(Vec::new())),
                "height" => Condition::Feature(Feature::Height(Vec::new())),
                "resolution" => Condition::Feature(Feature::Resolution(Vec::new())),
                "orientation" => Condition::Feature(Feature::Orientation(None)),
                "prefers-color-scheme" => Condition::Feature(Feature::ColorScheme(None)),
                "prefers-reduced-motion" => Condition::Feature(Feature::ReducedMotion(None)),
                "hover" => Condition::Feature(Feature::Hover(None)),
                "any-hover" => Condition::Feature(Feature::AnyHover(None)),
                "pointer" => Condition::Feature(Feature::Pointer(None)),
                "any-pointer" => Condition::Feature(Feature::AnyPointer(None)),
                // min- and max- need a value: (min-height) is invalid, not an unsupported feature.
                n if n.starts_with("min-") || n.starts_with("max-") => invalid_feature(inner, src, diags),
                _ => unsupported_feature(inner, src, diags, name),
            },
            _ => invalid_feature(inner, src, diags),
        },
        _ => invalid_feature(inner, src, diags),
    }
}

fn unsupported_feature(toks: &[Token], src: &str, diags: &mut Vec<Diagnostic>, name: &str) -> Condition {
    let (s, e) = span_of(toks, src);
    diags.push(Diagnostic::new(code::MEDIA_FEATURE, format!("media feature '{}' is not supported", name.to_ascii_lowercase()), "see the media feature allowlist", s, e));
    Condition::Unknown
}

fn invalid_feature(toks: &[Token], src: &str, diags: &mut Vec<Diagnostic>) -> Condition {
    let (s, e) = span_of(toks, src);
    diags.push(Diagnostic::new(code::INVALID_MEDIA_QUERY, "invalid media feature", "write (feature: value) or a range", s, e));
    Condition::Unknown
}

/// `(name: value)` where `name` may carry a min- or max- prefix.
fn feature_with_value(name_toks: &[Token], value: &[Token], src: &str, diags: &mut Vec<Diagnostic>) -> Condition {
    let name = match name_toks {
        [Token { kind: Kind::Ident(n), .. }] => n.to_ascii_lowercase(),
        _ => return invalid_feature(name_toks, src, diags),
    };
    let (prefix, base) = if let Some(b) = name.strip_prefix("min-") {
        (Some(Op::Ge), b)
    } else if let Some(b) = name.strip_prefix("max-") {
        (Some(Op::Le), b)
    } else {
        (None, name.as_str())
    };
    let numeric = matches!(base, "width" | "height" | "resolution");
    if !numeric && !matches!(base, "orientation" | "prefers-color-scheme" | "prefers-reduced-motion" | "hover" | "any-hover" | "pointer" | "any-pointer") {
        return unsupported_feature(name_toks, src, diags, &name);
    }
    let whole = span_of(name_toks, src);
    let invalid_value = |diags: &mut Vec<Diagnostic>| {
        let (s, e) = span_of(value, src);
        diags.push(Diagnostic::new(code::INVALID_MEDIA_QUERY, format!("invalid value for media feature '{name}'"), "check the feature value", s.min(whole.0), e.max(whole.1)));
        Condition::Unknown
    };
    if numeric {
        let Some(v) = (match base {
            "resolution" => value_resolution(value),
            _ => value_length(value),
        }) else {
            return invalid_value(diags);
        };
        let cons = vec![(prefix.unwrap_or(Op::Eq), v)];
        return Condition::Feature(match base {
            "width" => Feature::Width(cons),
            "height" => Feature::Height(cons),
            _ => Feature::Resolution(cons),
        });
    }
    if prefix.is_some() {
        return invalid_value(diags);
    }
    let single = match value {
        [one] => match &one.kind {
            Kind::Ident(v) => v.to_ascii_lowercase(),
            _ => return invalid_value(diags),
        },
        _ => return invalid_value(diags),
    };
    let feature = match base {
        "orientation" => match single.as_str() {
            "portrait" => Some(Feature::Orientation(Some(Orientation::Portrait))),
            "landscape" => Some(Feature::Orientation(Some(Orientation::Landscape))),
            _ => None,
        },
        "prefers-color-scheme" => match single.as_str() {
            "light" => Some(Feature::ColorScheme(Some(ColorScheme::Light))),
            "dark" => Some(Feature::ColorScheme(Some(ColorScheme::Dark))),
            _ => None,
        },
        "prefers-reduced-motion" => match single.as_str() {
            "no-preference" => Some(Feature::ReducedMotion(Some(false))),
            "reduce" => Some(Feature::ReducedMotion(Some(true))),
            _ => None,
        },
        "hover" | "any-hover" => {
            let cap = match single.as_str() {
                "none" => Some(HoverCap::None),
                "hover" => Some(HoverCap::Hover),
                _ => None,
            };
            cap.map(|c| if base == "hover" { Feature::Hover(Some(c)) } else { Feature::AnyHover(Some(c)) })
        }
        _ => {
            let cap = match single.as_str() {
                "none" => Some(PointerCap::None),
                "coarse" => Some(PointerCap::Coarse),
                "fine" => Some(PointerCap::Fine),
                _ => None,
            };
            cap.map(|c| if base == "pointer" { Feature::Pointer(Some(c)) } else { Feature::AnyPointer(Some(c)) })
        }
    };
    match feature {
        Some(f) => Condition::Feature(f),
        None => invalid_value(diags),
    }
}

/// A range such as `(width >= 600px)`, `(600px <= width)` or `(400px < width < 800px)`.
fn range_feature(inner: &[Token], src: &str, diags: &mut Vec<Diagnostic>) -> Condition {
    let (name_at, parsed) = match range_parts(inner) {
        Some(p) => p,
        None => return invalid_feature(inner, src, diags),
    };
    let name = match &inner[name_at].kind {
        Kind::Ident(n) => n.to_ascii_lowercase(),
        _ => return invalid_feature(inner, src, diags),
    };
    if !matches!(name.as_str(), "width" | "height" | "resolution") {
        return if matches!(name.as_str(), "orientation" | "prefers-color-scheme" | "prefers-reduced-motion" | "hover" | "any-hover" | "pointer" | "any-pointer") {
            invalid_feature(inner, src, diags)
        } else {
            unsupported_feature(inner, src, diags, &name)
        };
    }
    let mut cons = Vec::new();
    for (op, tok) in parsed {
        let v = if name == "resolution" { value_resolution(std::slice::from_ref(tok)) } else { value_length(std::slice::from_ref(tok)) };
        match v {
            Some(v) => cons.push((op, v)),
            None => return invalid_feature(inner, src, diags),
        }
    }
    Condition::Feature(match name.as_str() {
        "width" => Feature::Width(cons),
        "height" => Feature::Height(cons),
        _ => Feature::Resolution(cons),
    })
}

/// Splits a range into the feature name and its comparisons, normalised so that the feature is on the left:
/// `value op name` becomes `name reverse(op) value`. Operators are <, <=, >, >= and =, where
/// <= and >= are a < or > immediately followed by =. Returns (index of the name, comparisons).
fn range_parts(inner: &[Token]) -> Option<(usize, Vec<(Op, &Token)>)> {
    let sig: Vec<usize> = (0..inner.len()).filter(|&i| !matches!(inner[i].kind, Kind::Whitespace)).collect();
    // The operator at significant position p, and how many significant tokens it takes.
    let op_at = |p: usize| -> Option<(Op, usize)> {
        let i = *sig.get(p)?;
        match inner[i].kind {
            Kind::Delim('=') => Some((Op::Eq, 1)),
            Kind::Delim(c @ ('<' | '>')) => {
                let joined = sig.get(p + 1).is_some_and(|&j| j == i + 1 && matches!(inner[j].kind, Kind::Delim('=')));
                Some(match (c, joined) {
                    ('<', true) => (Op::Le, 2),
                    ('<', false) => (Op::Lt, 1),
                    (_, true) => (Op::Ge, 2),
                    _ => (Op::Gt, 1),
                })
            }
            _ => None,
        }
    };
    let is_name = |p: usize| sig.get(p).is_some_and(|&i| matches!(inner[i].kind, Kind::Ident(_)));
    let is_value = |p: usize| sig.get(p).is_some_and(|&i| matches!(inner[i].kind, Kind::Dimension { .. } | Kind::Number { .. } | Kind::Percentage(_)));
    let reverse = |op: Op| match op {
        Op::Lt => Op::Gt,
        Op::Le => Op::Ge,
        Op::Gt => Op::Lt,
        Op::Ge => Op::Le,
        Op::Eq => Op::Eq,
    };
    let forward = |o: Op| matches!(o, Op::Lt | Op::Le);
    // name op value
    if is_name(0) {
        let (op, n) = op_at(1)?;
        let v = 1 + n;
        if !is_value(v) || v + 1 != sig.len() {
            return None;
        }
        return Some((sig[0], vec![(op, &inner[sig[v]])]));
    }
    // value op name [op value]
    if !is_value(0) {
        return None;
    }
    let (op1, n1) = op_at(1)?;
    let name_at = 1 + n1;
    if !is_name(name_at) {
        return None;
    }
    if name_at + 1 == sig.len() {
        return Some((sig[name_at], vec![(reverse(op1), &inner[sig[0]])]));
    }
    let (op2, n2) = op_at(name_at + 1)?;
    let v2 = name_at + 1 + n2;
    if !is_value(v2) || v2 + 1 != sig.len() || matches!(op1, Op::Eq) || matches!(op2, Op::Eq) || forward(op1) != forward(op2) {
        return None;
    }
    Some((sig[name_at], vec![(reverse(op1), &inner[sig[0]]), (op2, &inner[sig[v2]])]))
}

fn value_length(v: &[Token]) -> Option<f64> {
    let [t] = v else { return None };
    let x = match &t.kind {
        Kind::Dimension { value, unit, .. } => match unit.to_ascii_lowercase().as_str() {
            "px" => *value,
            "em" | "rem" => *value * 16.0,
            _ => return None,
        },
        Kind::Number { value, .. } if *value == 0.0 => 0.0,
        _ => return None,
    };
    (x >= 0.0).then_some(x)
}

fn value_resolution(v: &[Token]) -> Option<f64> {
    let [t] = v else { return None };
    let x = match &t.kind {
        Kind::Dimension { value, unit, .. } => match unit.to_ascii_lowercase().as_str() {
            "dppx" | "x" => *value,
            "dpi" => *value / 96.0,
            "dpcm" => *value * 2.54 / 96.0,
            _ => return None,
        },
        _ => return None,
    };
    (x >= 0.0).then_some(x)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tri {
    True,
    False,
    Unknown,
}

fn not3(a: Tri) -> Tri {
    match a {
        Tri::True => Tri::False,
        Tri::False => Tri::True,
        Tri::Unknown => Tri::Unknown,
    }
}

fn and3(a: Tri, b: Tri) -> Tri {
    match (a, b) {
        (Tri::False, _) | (_, Tri::False) => Tri::False,
        (Tri::True, Tri::True) => Tri::True,
        _ => Tri::Unknown,
    }
}

fn or3(a: Tri, b: Tri) -> Tri {
    match (a, b) {
        (Tri::True, _) | (_, Tri::True) => Tri::True,
        (Tri::False, Tri::False) => Tri::False,
        _ => Tri::Unknown,
    }
}

fn cmp(op: Op, x: f64, v: f64) -> bool {
    match op {
        Op::Lt => x < v,
        Op::Le => x <= v,
        Op::Gt => x > v,
        Op::Ge => x >= v,
        Op::Eq => x == v,
    }
}

fn eval_feature(f: &Feature, env: &MediaEnv) -> bool {
    fn numeric(c: &[(Op, f64)], x: f64) -> bool {
        if c.is_empty() {
            x != 0.0
        } else {
            c.iter().all(|(op, v)| cmp(*op, x, *v))
        }
    }
    match f {
        Feature::Width(c) => numeric(c, env.width),
        Feature::Height(c) => numeric(c, env.height),
        Feature::Resolution(c) => numeric(c, env.dppx),
        Feature::Orientation(o) => {
            let actual = if env.width > env.height { Orientation::Landscape } else { Orientation::Portrait };
            o.is_none_or(|o| o == actual)
        }
        Feature::ColorScheme(c) => c.is_none_or(|c| c == env.color_scheme),
        Feature::ReducedMotion(r) => r.map_or(env.reduced_motion, |r| r == env.reduced_motion),
        Feature::Hover(h) | Feature::AnyHover(h) => h.map_or(env.hover != HoverCap::None, |h| h == env.hover),
        Feature::Pointer(p) | Feature::AnyPointer(p) => p.map_or(env.pointer != PointerCap::None, |p| p == env.pointer),
    }
}

fn eval_condition(c: &Condition, env: &MediaEnv) -> Tri {
    match c {
        Condition::Not(inner) => not3(eval_condition(inner, env)),
        Condition::And(items) => items.iter().fold(Tri::True, |acc, i| and3(acc, eval_condition(i, env))),
        Condition::Or(items) => items.iter().fold(Tri::False, |acc, i| or3(acc, eval_condition(i, env))),
        Condition::Feature(f) => {
            if eval_feature(f, env) {
                Tri::True
            } else {
                Tri::False
            }
        }
        Condition::Unknown => Tri::Unknown,
    }
}

fn eval_query(q: &MediaQuery, env: &MediaEnv) -> Tri {
    if !q.valid {
        return Tri::False;
    }
    let ty = match q.media_type {
        MediaType::All | MediaType::Screen => Tri::True,
        MediaType::Print | MediaType::Other(_) => Tri::False,
    };
    let cond = q.condition.as_ref().map_or(Tri::True, |c| eval_condition(c, env));
    let r = and3(ty, cond);
    if q.negated { not3(r) } else { r }
}

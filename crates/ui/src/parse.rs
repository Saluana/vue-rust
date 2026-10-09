//! Grammar dispatch: parses a component list against a property grammar (`props::G`) into a `Value`. Keywords are
//! matched ASCII case-insensitively and stored as the table's static text. Every refusal is `ParseErr::Invalid`
//! (not valid CSS) or `ParseErr::Unsupported` (valid CSS outside the subset, with its diagnostic code).

use crate::calc::{self, Calc};
use crate::color;
use crate::diag::code;
use crate::font;
use crate::grid;
use crate::length::{Lp, Unit};
use crate::props::{G, SIZE_KWS, WEIGHT_KWS};
use crate::transform;
use crate::tree::{ident, keyword, nows, Cv};
use crate::value::{ParseErr, Shadow, Value};
use vn_css::token::Kind;

/// Units that CSS defines and that the subset does not support (Chrome accepts them).
const UNSUPPORTED_UNITS: &[&str] = &[
    "cm", "mm", "q", "in", "pt", "pc", "ch", "ex", "ic", "lh", "rlh", "cap", "rcap", "rex", "ric", "vi", "vb", "dvw", "dvh",
    "dvi", "dvb", "dvmin", "dvmax", "svw", "svh", "svi", "svb", "svmin", "svmax", "lvw", "lvh", "lvi", "lvb", "lvmin", "lvmax",
    "cqw", "cqh", "cqi", "cqb", "cqmin", "cqmax",
];

pub fn parse(g: G, cvs: &[Cv]) -> Result<Value, ParseErr> {
    match g {
        G::Kw(list) => keyword_only(cvs, list).map(Value::Kw),
        G::Lp { kws, nonneg, pct } => match single_keyword(cvs, kws) {
            Some(k) => Ok(Value::Kw(k)),
            None => Ok(Value::Lp(lp(cvs, nonneg, pct)?)),
        },
        G::Len { kws, nonneg } => match single_keyword(cvs, kws) {
            Some(k) => Ok(Value::Kw(k)),
            None => Ok(Value::Lp(lp(cvs, nonneg, false)?)),
        },
        G::Num { nonneg, int, pct, kws } => match single_keyword(cvs, kws) {
            Some(k) => Ok(Value::Kw(k)),
            None => number(cvs, nonneg, int, pct).map(Value::Num),
        },
        G::Color => {
            if let [c] = nows(cvs).as_slice() {
                if ident(c).is_some_and(|s| s.eq_ignore_ascii_case("currentcolor")) {
                    return Ok(Value::CurrentColor);
                }
            }
            color::parse(cvs).map(Value::Color)
        }
        G::Families => font::parse_families(cvs).map(Value::Families),
        G::FontSize => match single_keyword(cvs, SIZE_KWS) {
            Some(k) => Ok(Value::Kw(k)),
            None => Ok(Value::Lp(lp(cvs, true, true)?)),
        },
        G::FontWeight => font_weight(cvs),
        G::LineHeight => line_height(cvs),
        G::Transform => match single_keyword(cvs, &["none"]) {
            Some(k) => Ok(Value::Kw(k)),
            None => transform::parse(cvs).map(Value::Transform),
        },
        G::Radius => radius(cvs),
        G::Tracks => match single_keyword(cvs, &["none"]) {
            Some(k) => Ok(Value::Kw(k)),
            None => grid::track_list(cvs).map(Value::Tracks),
        },
        G::TrackSizes => grid::track_sizes(cvs).map(Value::Tracks),
        G::Areas => match single_keyword(cvs, &["none"]) {
            Some(k) => Ok(Value::Kw(k)),
            None => grid::template_areas(cvs).map(Value::Areas),
        },
        G::GridLine => grid::grid_line(cvs).map(Value::Line),
        G::AutoFlow => grid::auto_flow(cvs).map(Value::Kw),
        G::Ratio => ratio(cvs),
        G::Shadows => match single_keyword(cvs, &["none"]) {
            Some(k) => Ok(Value::Kw(k)),
            None => shadows(cvs).map(Value::Shadows),
        },
        G::DecorationLine => decoration_line(cvs),
    }
}

/// The keyword when the components are exactly one identifier from `kws`.
fn single_keyword(cvs: &[Cv], kws: &'static [&'static str]) -> Option<&'static str> {
    match nows(cvs).as_slice() {
        [c] => keyword(c, kws),
        _ => None,
    }
}

fn keyword_only(cvs: &[Cv], kws: &'static [&'static str]) -> Result<&'static str, ParseErr> {
    single_keyword(cvs, kws).ok_or(ParseErr::Invalid)
}

/// A <length-percentage> (or <length> when `pct` is false) from one component: a dimension, a percentage, unitless
/// zero or calc(). Negative values are refused when `nonneg`; calc() results are clamped at computed-value time.
pub fn lp(cvs: &[Cv], nonneg: bool, pct: bool) -> Result<Lp, ParseErr> {
    match nows(cvs).as_slice() {
        [c] => lp_one(c, nonneg, pct),
        _ => Err(ParseErr::Invalid),
    }
}

pub fn lp_one(c: &Cv, nonneg: bool, pct: bool) -> Result<Lp, ParseErr> {
    match c {
        Cv::Tok(t) => match &t.kind {
            Kind::Dimension { value, unit, .. } => match Unit::from_name(unit) {
                Some(u) if !(nonneg && *value < 0.0) => Ok(Lp::one(u, *value)),
                Some(_) => Err(ParseErr::Invalid),
                None if UNSUPPORTED_UNITS.contains(&unit.to_ascii_lowercase().as_str()) => {
                    Err(ParseErr::unsupported(code::UNSUPPORTED_VALUE, format!("unit {unit}")))
                }
                None => Err(ParseErr::Invalid),
            },
            Kind::Percentage(v) if pct && !(nonneg && *v < 0.0) => Ok(Lp::one(Unit::Pct, *v)),
            Kind::Number { value, .. } if *value == 0.0 => Ok(Lp::px(0.0)),
            _ => Err(ParseErr::Invalid),
        },
        Cv::Func { name, args, .. } if name.eq_ignore_ascii_case("calc") => match calc::eval(args)? {
            Calc::Len(l) if pct || !l.has_pct() => Ok(l),
            _ => Err(ParseErr::Invalid),
        },
        Cv::Func { name, .. } => Err(ParseErr::unsupported(code::UNSUPPORTED_FUNCTION, format!("{name}()"))),
        _ => Err(ParseErr::Invalid),
    }
}

fn number(cvs: &[Cv], nonneg: bool, int: bool, pct: bool) -> Result<f64, ParseErr> {
    let items = nows(cvs);
    let v = match items.as_slice() {
        [Cv::Tok(t)] => match t.kind {
            Kind::Number { value, int: is_int } => {
                if int && !is_int {
                    return Err(ParseErr::Invalid);
                }
                value
            }
            Kind::Percentage(p) if pct => p / 100.0,
            _ => return Err(ParseErr::Invalid),
        },
        [Cv::Func { name, args, .. }] if name.eq_ignore_ascii_case("calc") => match calc::eval(args)? {
            Calc::Num(v) if !(int && v.fract() != 0.0) => v,
            _ => return Err(ParseErr::Invalid),
        },
        [Cv::Func { name, .. }] => return Err(ParseErr::unsupported(code::UNSUPPORTED_FUNCTION, format!("{name}()"))),
        _ => return Err(ParseErr::Invalid),
    };
    if nonneg && v < 0.0 { Err(ParseErr::Invalid) } else { Ok(v) }
}

fn font_weight(cvs: &[Cv]) -> Result<Value, ParseErr> {
    if let Some(k) = single_keyword(cvs, WEIGHT_KWS) {
        return Ok(Value::Kw(k));
    }
    let v = number(cvs, true, false, false)?;
    if (1.0..=1000.0).contains(&v) { Ok(Value::Num(v)) } else { Err(ParseErr::Invalid) }
}

fn line_height(cvs: &[Cv]) -> Result<Value, ParseErr> {
    if let Some(k) = single_keyword(cvs, &["normal"]) {
        return Ok(Value::Kw(k));
    }
    if let [Cv::Tok(t)] = nows(cvs).as_slice() {
        if let Kind::Number { value, .. } = t.kind {
            return if value < 0.0 { Err(ParseErr::Invalid) } else { Ok(Value::Num(value)) };
        }
    }
    Ok(Value::Lp(lp(cvs, true, true)?))
}

fn radius(cvs: &[Cv]) -> Result<Value, ParseErr> {
    let items = nows(cvs);
    match items.as_slice() {
        [a] => {
            let v = lp_one(a, true, true)?;
            Ok(Value::Radius(v.clone(), v))
        }
        [a, b] => Ok(Value::Radius(lp_one(a, true, true)?, lp_one(b, true, true)?)),
        _ => Err(ParseErr::Invalid),
    }
}

fn ratio(cvs: &[Cv]) -> Result<Value, ParseErr> {
    if let Some(k) = single_keyword(cvs, &["auto"]) {
        return Ok(Value::Kw(k));
    }
    let items = nows(cvs);
    let num = |c: &Cv| -> Result<f64, ParseErr> {
        match c {
            Cv::Tok(t) => match t.kind {
                Kind::Number { value, .. } if value >= 0.0 => Ok(value),
                _ => Err(ParseErr::Invalid),
            },
            _ => Err(ParseErr::Invalid),
        }
    };
    match items.as_slice() {
        [c] => Ok(Value::Ratio(num(c)?, 1.0)),
        [a, Cv::Tok(t), b] if matches!(t.kind, Kind::Delim('/')) => Ok(Value::Ratio(num(a)?, num(b)?)),
        [c, Cv::Tok(t), _] if ident(c).is_some_and(|s| s.eq_ignore_ascii_case("auto")) && matches!(t.kind, Kind::Delim('/')) => {
            Err(ParseErr::unsupported(code::UNSUPPORTED_VALUE, "aspect-ratio: auto <ratio>"))
        }
        _ => Err(ParseErr::Invalid),
    }
}

/// A list of shadows (E): `[inset] x y [blur [spread]] [color]`, with the colour first or last.
fn shadows(cvs: &[Cv]) -> Result<Vec<Shadow>, ParseErr> {
    let mut out = Vec::new();
    for item in crate::tree::split_commas(cvs) {
        let parts = nows(&item);
        let mut inset = false;
        let mut lens: Vec<Lp> = Vec::new();
        let mut color = None;
        let mut color_seen = false;
        for c in parts {
            if ident(c).is_some_and(|s| s.eq_ignore_ascii_case("inset")) && !inset {
                inset = true;
            } else if ident(c).is_some_and(|s| s.eq_ignore_ascii_case("currentcolor")) && !color_seen {
                color_seen = true;
            } else if let Ok(v) = lp_one(c, false, false) {
                if color_seen || lens.len() >= 4 {
                    return Err(ParseErr::Invalid);
                }
                lens.push(v);
            } else if !color_seen {
                color = Some(color::parse(std::slice::from_ref(c))?);
                color_seen = true;
            } else {
                return Err(ParseErr::Invalid);
            }
        }
        if lens.len() < 2 {
            return Err(ParseErr::Invalid);
        }
        let mut it = lens.into_iter();
        let (x, y) = (it.next().unwrap_or(Lp::px(0.0)), it.next().unwrap_or(Lp::px(0.0)));
        let blur = it.next();
        let spread = it.next();
        out.push(Shadow { inset, x, y, blur, spread, color });
    }
    Ok(out)
}

const DECORATION: &[&str] = &[
    "none",
    "underline",
    "overline",
    "underline overline",
    "line-through",
    "underline line-through",
    "overline line-through",
    "underline overline line-through",
    "blink",
    "underline blink",
    "overline blink",
    "underline overline blink",
    "line-through blink",
    "underline line-through blink",
    "overline line-through blink",
    "underline overline line-through blink",
];

fn decoration_line(cvs: &[Cv]) -> Result<Value, ParseErr> {
    let items = nows(cvs);
    if let [c] = items.as_slice() {
        if let Some(k) = keyword(c, &["none"]) {
            return Ok(Value::Kw(k));
        }
    }
    let mut flags = [false; 4];
    for c in items {
        let idx = match ident(c).map(|s| s.to_ascii_lowercase()).as_deref() {
            Some("underline") => 0,
            Some("overline") => 1,
            Some("line-through") => 2,
            Some("blink") => 3,
            _ => return Err(ParseErr::Invalid),
        };
        if flags[idx] {
            return Err(ParseErr::Invalid);
        }
        flags[idx] = true;
    }
    if !flags.iter().any(|f| *f) {
        return Err(ParseErr::Invalid);
    }
    let names = ["underline", "overline", "line-through", "blink"];
    let text: Vec<&str> = names.iter().zip(flags).filter(|(_, on)| *on).map(|(n, _)| *n).collect();
    let joined = text.join(" ");
    match DECORATION.iter().copied().find(|d| *d == joined.as_str()) {
        Some(d) => Ok(Value::Kw(d)),
        None => Err(ParseErr::Invalid),
    }
}

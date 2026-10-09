//! Shorthand expansion into longhands with the CSS rules: 1-4 value boxes (margin, padding, border-width, inset...),
//! the side and corner orders, flex's grow/shrink/basis defaults, border-radius with `/`, grid-column and grid-row
//! with `/`, and the align/justify pairs of place-*. Each longhand value is parsed with that longhand's own grammar
//! from the table, so a shorthand accepts exactly what its longhands accept. Unsupported forms (for example text
//! decoration style or colour, font-variant, background images) are refused with VNCSS5102.

use crate::diag::code;
use crate::font;
use crate::grid;
use crate::parse::{self, lp_one};
use crate::props::{self, G};
use crate::tree::{ident, nows, Cv};
use crate::value::{ParseErr, Value};
use crate::color;
use crate::length::Lp;
use vn_css::token::Kind;

pub type Longhands = Vec<(&'static str, Value)>;

/// The shorthand names this module expands.
pub const SHORTHANDS: &[&str] = &[
    "background", "border", "border-bottom", "border-color", "border-left", "border-radius", "border-right", "border-style",
    "border-top", "border-width", "flex", "flex-flow", "font", "gap", "grid-column", "grid-row", "inset", "margin", "outline",
    "overflow", "padding", "place-content", "place-items", "place-self", "text-decoration",
];

pub fn is_shorthand(name: &str) -> bool {
    SHORTHANDS.contains(&name)
}

/// Expands a shorthand. None when `name` is not a shorthand of the subset.
pub fn expand(name: &str, cvs: &[Cv]) -> Option<Result<Longhands, ParseErr>> {
    if !is_shorthand(name) {
        return None;
    }
    Some(match name {
        "margin" => four(cvs, ["margin-top", "margin-right", "margin-bottom", "margin-left"]),
        "padding" => four(cvs, ["padding-top", "padding-right", "padding-bottom", "padding-left"]),
        "inset" => four(cvs, ["top", "right", "bottom", "left"]),
        "border-width" => four(cvs, ["border-top-width", "border-right-width", "border-bottom-width", "border-left-width"]),
        "border-style" => four(cvs, ["border-top-style", "border-right-style", "border-bottom-style", "border-left-style"]),
        "border-color" => four(cvs, ["border-top-color", "border-right-color", "border-bottom-color", "border-left-color"]),
        "border-top" => side(cvs, "top"),
        "border-right" => side(cvs, "right"),
        "border-bottom" => side(cvs, "bottom"),
        "border-left" => side(cvs, "left"),
        "border" => border(cvs),
        "border-radius" => radius(cvs),
        "flex" => flex(cvs),
        "flex-flow" => flex_flow(cvs),
        "gap" => gap(cvs),
        "place-items" => pair(cvs, "align-items", "justify-items"),
        "place-self" => pair(cvs, "align-self", "justify-self"),
        "place-content" => place_content(cvs),
        "overflow" => pair(cvs, "overflow-x", "overflow-y"),
        "outline" => outline(cvs),
        "text-decoration" => text_decoration(cvs),
        "font" => font_shorthand(cvs),
        "background" => background(cvs),
        "grid-column" => grid_pair(cvs, "grid-column-start", "grid-column-end"),
        "grid-row" => grid_pair(cvs, "grid-row-start", "grid-row-end"),
        _ => return None,
    })
}

/// The grammar of a longhand of the table.
fn grammar(name: &str) -> G {
    props::lookup(name).map_or(G::Kw(&[]), |p| p.grammar)
}

/// One component parsed with a longhand's grammar.
fn one(name: &str, c: &Cv) -> Result<Value, ParseErr> {
    parse::parse(grammar(name), std::slice::from_ref(c))
}

/// The four-value box: 1 to 4 components assigned to top, right, bottom, left (CSS box order). The four sides share
/// one grammar in the table, so the components are parsed with the first side's grammar.
fn four(cvs: &[Cv], names: [&'static str; 4]) -> Result<Longhands, ParseErr> {
    let items = nows(cvs);
    let v: Vec<Value> = items.iter().map(|c| one(names[0], c)).collect::<Result<_, _>>()?;
    let sides: Vec<Value> = match v.len() {
        1 => vec![v[0].clone(), v[0].clone(), v[0].clone(), v[0].clone()],
        2 => vec![v[0].clone(), v[1].clone(), v[0].clone(), v[1].clone()],
        3 => vec![v[0].clone(), v[1].clone(), v[2].clone(), v[1].clone()],
        4 => v,
        _ => return Err(ParseErr::Invalid),
    };
    Ok(names.into_iter().zip(sides).collect())
}

/// A side shorthand's parts: width, style and colour, in any order, each at most once.
struct SidePart {
    width: Option<Value>,
    style: Option<Value>,
    color: Option<Value>,
}

/// Each component goes to the first of width, style and colour that accepts it. A refusal by one category (a colour
/// function is not a width) is not final: the component may belong to another category. An unsupported component
/// that no category accepts is reported as unsupported.
fn side_parts(cvs: &[Cv], width: &str, style: &str, color_name: &str) -> Result<SidePart, ParseErr> {
    let mut part = SidePart { width: None, style: None, color: None };
    for c in nows(cvs) {
        let mut unsupported = None;
        let mut placed = false;
        if part.width.is_none() {
            match one(width, c) {
                Ok(v) => {
                    part.width = Some(v);
                    placed = true;
                }
                Err(e @ ParseErr::Unsupported(..)) => unsupported = Some(e),
                Err(ParseErr::Invalid) => {}
            }
        }
        if !placed && part.style.is_none() {
            match one(style, c) {
                Ok(v) => {
                    part.style = Some(v);
                    placed = true;
                }
                Err(e @ ParseErr::Unsupported(..)) => unsupported = Some(e),
                Err(ParseErr::Invalid) => {}
            }
        }
        if !placed && part.color.is_none() {
            match one(color_name, c) {
                Ok(v) => {
                    part.color = Some(v);
                    placed = true;
                }
                Err(e @ ParseErr::Unsupported(..)) => unsupported = Some(e),
                Err(ParseErr::Invalid) => {}
            }
        }
        if !placed {
            return Err(unsupported.unwrap_or(ParseErr::Invalid));
        }
    }
    Ok(part)
}

fn side(cvs: &[Cv], side: &str) -> Result<Longhands, ParseErr> {
    let (w, st, c) = (side_name(side, "width"), side_name(side, "style"), side_name(side, "color"));
    let part = side_parts(cvs, w, st, c)?;
    Ok(vec![
        (w, part.width.unwrap_or(Value::Kw("medium"))),
        (st, part.style.unwrap_or(Value::Kw("none"))),
        (c, part.color.unwrap_or(Value::CurrentColor)),
    ])
}

fn border(cvs: &[Cv]) -> Result<Longhands, ParseErr> {
    let part = side_parts(cvs, "border-top-width", "border-top-style", "border-top-color")?;
    let w = part.width.unwrap_or(Value::Kw("medium"));
    let s = part.style.unwrap_or(Value::Kw("none"));
    let c = part.color.unwrap_or(Value::CurrentColor);
    let mut out = Vec::new();
    for side in ["top", "right", "bottom", "left"] {
        out.push((side_name(side, "width"), w.clone()));
        out.push((side_name(side, "style"), s.clone()));
        out.push((side_name(side, "color"), c.clone()));
    }
    Ok(out)
}

fn outline(cvs: &[Cv]) -> Result<Longhands, ParseErr> {
    let part = side_parts(cvs, "outline-width", "outline-style", "outline-color")?;
    Ok(vec![
        ("outline-width", part.width.unwrap_or(Value::Kw("medium"))),
        ("outline-style", part.style.unwrap_or(Value::Kw("none"))),
        ("outline-color", part.color.unwrap_or(Value::CurrentColor)),
    ])
}

fn side_name(side: &str, what: &str) -> &'static str {
    match (side, what) {
        ("top", "width") => "border-top-width",
        ("top", "style") => "border-top-style",
        ("top", _) => "border-top-color",
        ("right", "width") => "border-right-width",
        ("right", "style") => "border-right-style",
        ("right", _) => "border-right-color",
        ("bottom", "width") => "border-bottom-width",
        ("bottom", "style") => "border-bottom-style",
        ("bottom", _) => "border-bottom-color",
        ("left", "width") => "border-left-width",
        ("left", "style") => "border-left-style",
        _ => "border-left-color",
    }
}

/// border-radius: one to four horizontal radii, optionally `/` and one to four vertical radii.
fn radius(cvs: &[Cv]) -> Result<Longhands, ParseErr> {
    let parts = crate::tree::split_commas(cvs);
    if parts.len() != 1 {
        return Err(ParseErr::Invalid);
    }
    let all = nows(cvs);
    let slash = all.iter().position(|c| matches!(c, Cv::Tok(t) if matches!(t.kind, Kind::Delim('/'))));
    let (h_part, v_part): (Vec<&Cv>, Option<Vec<&Cv>>) = match slash {
        Some(i) => (all[..i].to_vec(), Some(all[i + 1..].to_vec())),
        None => (all.clone(), None),
    };
    let h = radius_list(&h_part)?;
    let v = match &v_part {
        Some(v) => radius_list(v)?,
        None => h.clone(),
    };
    let corners = ["border-top-left-radius", "border-top-right-radius", "border-bottom-right-radius", "border-bottom-left-radius"];
    Ok((0..4)
        .map(|k| (corners[k], Value::Radius(h[k].clone(), v[k].clone())))
        .collect())
}

/// Four corner values in TL, TR, BR, BL order from one to four components.
fn radius_list(items: &[&Cv]) -> Result<Vec<Lp>, ParseErr> {
    let v: Vec<Lp> = items.iter().map(|c| lp_one(c, true, true)).collect::<Result<_, _>>()?;
    Ok(match v.len() {
        1 => vec![v[0].clone(); 4],
        2 => vec![v[0].clone(), v[1].clone(), v[0].clone(), v[1].clone()],
        3 => vec![v[0].clone(), v[1].clone(), v[2].clone(), v[1].clone()],
        4 => v,
        _ => return Err(ParseErr::Invalid),
    })
}

/// flex: `none`, `auto`, `initial` forms, or a grow factor with an optional shrink factor and basis.
fn flex(cvs: &[Cv]) -> Result<Longhands, ParseErr> {
    let items = nows(cvs);
    if let [c] = items.as_slice() {
        if let Some(k) = ident(c) {
            match k.to_ascii_lowercase().as_str() {
                "none" => return Ok(flex_longhands(Value::Num(0.0), Value::Num(0.0), Value::Kw("auto"))),
                "auto" => return Ok(flex_longhands(Value::Num(1.0), Value::Num(1.0), Value::Kw("auto"))),
                _ => {}
            }
        }
    }
    let is_factor = |c: &Cv| one("flex-grow", c).is_ok();
    let (grow, shrink, basis): (Value, Value, Value) = match items.as_slice() {
        [a] if is_factor(a) => (one("flex-grow", a)?, Value::Num(1.0), Value::Lp(Lp::one(crate::length::Unit::Pct, 0.0))),
        [a] => (Value::Num(1.0), Value::Num(1.0), one("flex-basis", a)?),
        [a, b] if is_factor(a) && is_factor(b) => (one("flex-grow", a)?, one("flex-shrink", b)?, Value::Lp(Lp::one(crate::length::Unit::Pct, 0.0))),
        [a, b] if is_factor(a) => (one("flex-grow", a)?, Value::Num(1.0), one("flex-basis", b)?),
        [a, b] if is_factor(b) => (one("flex-grow", b)?, Value::Num(1.0), one("flex-basis", a)?),
        [a, b, c] if is_factor(a) && is_factor(b) => (one("flex-grow", a)?, one("flex-shrink", b)?, one("flex-basis", c)?),
        _ => return Err(ParseErr::Invalid),
    };
    Ok(flex_longhands(grow, shrink, basis))
}

fn flex_longhands(grow: Value, shrink: Value, basis: Value) -> Longhands {
    vec![("flex-grow", grow), ("flex-shrink", shrink), ("flex-basis", basis)]
}

fn flex_flow(cvs: &[Cv]) -> Result<Longhands, ParseErr> {
    let mut dir = None;
    let mut wrap = None;
    for c in nows(cvs) {
        if dir.is_none() {
            if let Ok(v) = one("flex-direction", c) {
                dir = Some(v);
                continue;
            }
        }
        if wrap.is_none() {
            if let Ok(v) = one("flex-wrap", c) {
                wrap = Some(v);
                continue;
            }
        }
        return Err(ParseErr::Invalid);
    }
    Ok(vec![
        ("flex-direction", dir.unwrap_or(Value::Kw("row"))),
        ("flex-wrap", wrap.unwrap_or(Value::Kw("nowrap"))),
    ])
}

/// `row-gap [column-gap]`: one value sets both.
fn gap(cvs: &[Cv]) -> Result<Longhands, ParseErr> {
    let items = nows(cvs);
    match items.as_slice() {
        [a] => {
            let v = one("row-gap", a)?;
            Ok(vec![("row-gap", v.clone()), ("column-gap", v)])
        }
        [a, b] => Ok(vec![("row-gap", one("row-gap", a)?), ("column-gap", one("column-gap", b)?)]),
        _ => Err(ParseErr::Invalid),
    }
}

/// place-content: one value sets align-content and justify-content, except that `baseline` gives justify-content start.
fn place_content(cvs: &[Cv]) -> Result<Longhands, ParseErr> {
    let items = nows(cvs);
    match items.as_slice() {
        [a] => {
            let align = one("align-content", a)?;
            let justify = if matches!(align, Value::Kw("baseline")) { Value::Kw("start") } else { one("justify-content", a)? };
            Ok(vec![("align-content", align), ("justify-content", justify)])
        }
        _ => pair(cvs, "align-content", "justify-content"),
    }
}

/// Two longhands from one or two components; one component sets both (place-*, overflow).
fn pair(cvs: &[Cv], first: &'static str, second: &'static str) -> Result<Longhands, ParseErr> {
    let items = nows(cvs);
    match items.as_slice() {
        [a] => Ok(vec![(first, one(first, a)?), (second, one(second, a)?)]),
        [a, b] => Ok(vec![(first, one(first, a)?), (second, one(second, b)?)]),
        _ => Err(ParseErr::Invalid),
    }
}

fn grid_pair(cvs: &[Cv], start: &'static str, end: &'static str) -> Result<Longhands, ParseErr> {
    let all = nows(cvs);
    let slash = all.iter().position(|c| matches!(c, Cv::Tok(t) if matches!(t.kind, Kind::Delim('/'))));
    let (a, b): (Vec<&Cv>, Vec<&Cv>) = match slash {
        Some(i) => (all[..i].to_vec(), all[i + 1..].to_vec()),
        None => (all.clone(), Vec::new()),
    };
    let start_value = grid_value(&a)?;
    let end_value = if b.is_empty() && slash.is_none() { Value::Line(grid::GridLine::Auto) } else { grid_value(&b)? };
    Ok(vec![(start, start_value), (end, end_value)])
}

fn grid_value(items: &[&Cv]) -> Result<Value, ParseErr> {
    let owned: Vec<Cv> = items.iter().map(|c| (*c).clone()).collect();
    grid::grid_line(&owned).map(Value::Line)
}

/// text-decoration: the line part only (style, colour and thickness are not supported).
fn text_decoration(cvs: &[Cv]) -> Result<Longhands, ParseErr> {
    match parse::parse(grammar("text-decoration-line"), cvs) {
        Ok(v) => Ok(vec![("text-decoration-line", v)]),
        Err(ParseErr::Unsupported(c, w)) => Err(ParseErr::Unsupported(c, w)),
        Err(ParseErr::Invalid) => Err(ParseErr::unsupported(code::UNSUPPORTED_VALUE, "text-decoration style, colour or thickness")),
    }
}

/// background: a colour (or none, which resets the colour). Images, positions and layers are unsupported.
fn background(cvs: &[Cv]) -> Result<Longhands, ParseErr> {
    let items = nows(cvs);
    if let [c] = items.as_slice() {
        if ident(c).is_some_and(|s| s.eq_ignore_ascii_case("none")) {
            return Ok(vec![("background-color", Value::Color(color::Color::TRANSPARENT))]);
        }
    }
    match parse::parse(grammar("background-color"), cvs) {
        Ok(v) => Ok(vec![("background-color", v)]),
        Err(ParseErr::Unsupported(c, w)) => Err(ParseErr::Unsupported(c, w)),
        Err(ParseErr::Invalid) => Err(ParseErr::unsupported(code::UNSUPPORTED_VALUE, "background image, position or repeat")),
    }
}

/// font: [style] [weight] size [/ line-height] family. Font-variant and font-stretch are not modelled (only `normal`).
fn font_shorthand(cvs: &[Cv]) -> Result<Longhands, ParseErr> {
    let all: Vec<(usize, &Cv)> = cvs.iter().enumerate().filter(|(_, c)| !matches!(c, Cv::Ws)).collect();
    let mut style = None;
    let mut weight = None;
    let mut i = 0;
    while i < all.len() {
        let c = all[i].1;
        let word = ident(c).map(|s| s.to_ascii_lowercase());
        match word.as_deref() {
            Some("normal") => {}
            Some("italic" | "oblique") if style.is_none() => style = Some(one("font-style", c)?),
            Some("bold" | "bolder" | "lighter") if weight.is_none() => weight = Some(one("font-weight", c)?),
            _ => {
                let is_weight_number = matches!(c, Cv::Tok(t) if matches!(t.kind, Kind::Number { int: true, .. }));
                if is_weight_number && weight.is_none() {
                    weight = Some(one("font-weight", c)?);
                } else {
                    break;
                }
            }
        }
        i += 1;
    }
    let size_index = i;
    let Some(&(_, size_cv)) = all.get(size_index) else {
        return Err(ParseErr::Invalid);
    };
    let size = one("font-size", size_cv)?;
    let mut next = size_index + 1;
    let mut line_height = Value::Kw("normal");
    if let Some(&(_, slash)) = all.get(next) {
        if matches!(slash, Cv::Tok(t) if matches!(t.kind, Kind::Delim('/'))) {
            let Some(&(_, lh)) = all.get(next + 1) else {
                return Err(ParseErr::Invalid);
            };
            line_height = one("line-height", lh)?;
            next += 2;
        }
    }
    let Some(&(family_start, _)) = all.get(next) else {
        return Err(ParseErr::Invalid);
    };
    let families = font::parse_families(&cvs[family_start..])?;
    Ok(vec![
        ("font-style", style.unwrap_or(Value::Kw("normal"))),
        ("font-weight", weight.unwrap_or(Value::Kw("normal"))),
        ("font-size", size),
        ("line-height", line_height),
        ("font-family", Value::Families(families)),
    ])
}

/// The longhands a shorthand sets (used when its value is invalid at computed-value time: all of them become unset).
pub fn longhands_of(name: &str) -> &'static [&'static str] {
    match name {
        "margin" => &["margin-top", "margin-right", "margin-bottom", "margin-left"],
        "padding" => &["padding-top", "padding-right", "padding-bottom", "padding-left"],
        "inset" => &["top", "right", "bottom", "left"],
        "border-width" => &["border-top-width", "border-right-width", "border-bottom-width", "border-left-width"],
        "border-style" => &["border-top-style", "border-right-style", "border-bottom-style", "border-left-style"],
        "border-color" => &["border-top-color", "border-right-color", "border-bottom-color", "border-left-color"],
        "border-top" => &["border-top-width", "border-top-style", "border-top-color"],
        "border-right" => &["border-right-width", "border-right-style", "border-right-color"],
        "border-bottom" => &["border-bottom-width", "border-bottom-style", "border-bottom-color"],
        "border-left" => &["border-left-width", "border-left-style", "border-left-color"],
        "border" => &[
            "border-top-width", "border-top-style", "border-top-color", "border-right-width", "border-right-style",
            "border-right-color", "border-bottom-width", "border-bottom-style", "border-bottom-color", "border-left-width",
            "border-left-style", "border-left-color",
        ],
        "border-radius" => &["border-top-left-radius", "border-top-right-radius", "border-bottom-right-radius", "border-bottom-left-radius"],
        "flex" => &["flex-grow", "flex-shrink", "flex-basis"],
        "flex-flow" => &["flex-direction", "flex-wrap"],
        "gap" => &["row-gap", "column-gap"],
        "place-items" => &["align-items", "justify-items"],
        "place-self" => &["align-self", "justify-self"],
        "place-content" => &["align-content", "justify-content"],
        "overflow" => &["overflow-x", "overflow-y"],
        "outline" => &["outline-width", "outline-style", "outline-color"],
        "text-decoration" => &["text-decoration-line"],
        "font" => &["font-style", "font-weight", "font-size", "line-height", "font-family"],
        "background" => &["background-color"],
        "grid-column" => &["grid-column-start", "grid-column-end"],
        "grid-row" => &["grid-row-start", "grid-row-end"],
        _ => &[],
    }
}

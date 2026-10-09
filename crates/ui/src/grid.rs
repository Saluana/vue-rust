//! Grid values: track lists (px, %, fr, auto, min-content, max-content, minmax(), repeat() with an integer or
//! auto-fill/auto-fit, and line names), grid lines (auto, integers, names, span), grid-template-areas and
//! grid-auto-flow. These are parsed and represented; the layout work package consumes them.

use crate::calc::{self, Calc};
use crate::diag::code;
use crate::length::{Lp, Unit};
use crate::tree::{ident, nows, split_commas, Cv};
use crate::value::ParseErr;
use vn_css::token::Kind;

#[derive(Debug, Clone, PartialEq)]
pub enum TrackItem {
    Size(TrackSize),
    Repeat(Repeat, Vec<TrackItem>),
    Names(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TrackSize {
    Auto,
    MinContent,
    MaxContent,
    Lp(Lp),
    Fr(f64),
    MinMax(Box<TrackSize>, Box<TrackSize>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Repeat {
    Count(u32),
    AutoFill,
    AutoFit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GridLine {
    Auto,
    Line { index: Option<i64>, name: Option<String>, span: bool },
}

/// `none` or a track list. Subgrid and masonry are unsupported.
pub fn track_list(cvs: &[Cv]) -> Result<Vec<TrackItem>, ParseErr> {
    let items = nows(cvs);
    if items.is_empty() {
        return Err(ParseErr::Invalid);
    }
    let mut out = Vec::new();
    let mut has_track = false;
    for c in items {
        match c {
            Cv::Square(names) => {
                let mut v = Vec::new();
                for n in nows(names) {
                    match ident(n) {
                        Some(s) if !is_reserved_name(s) => v.push(s.to_string()),
                        _ => return Err(ParseErr::Invalid),
                    }
                }
                out.push(TrackItem::Names(v));
            }
            Cv::Func { name, args, .. } if name.eq_ignore_ascii_case("repeat") => {
                has_track = true;
                out.push(repeat(args)?);
            }
            other => {
                has_track = true;
                out.push(TrackItem::Size(track_size(other)?));
            }
        }
    }
    if !has_track {
        return Err(ParseErr::Invalid);
    }
    Ok(out)
}

/// The sizes of grid-auto-columns and grid-auto-rows: one or more track sizes (no repeat, no names).
pub fn track_sizes(cvs: &[Cv]) -> Result<Vec<TrackItem>, ParseErr> {
    let items = nows(cvs);
    if items.is_empty() {
        return Err(ParseErr::Invalid);
    }
    items.into_iter().map(|c| track_size(c).map(TrackItem::Size)).collect()
}

fn is_reserved_name(s: &str) -> bool {
    s.eq_ignore_ascii_case("span") || s.eq_ignore_ascii_case("auto")
}

fn repeat(args: &[Cv]) -> Result<TrackItem, ParseErr> {
    let parts = split_commas(args);
    if parts.len() != 2 {
        return Err(ParseErr::Invalid);
    }
    let count = match nows(&parts[0]).as_slice() {
        [Cv::Tok(t)] => match t.kind {
            Kind::Ident(ref s) if s.eq_ignore_ascii_case("auto-fill") => Repeat::AutoFill,
            Kind::Ident(ref s) if s.eq_ignore_ascii_case("auto-fit") => Repeat::AutoFit,
            Kind::Number { value, int: true } if value >= 1.0 && value.fract() == 0.0 => Repeat::Count(value as u32),
            _ => return Err(ParseErr::Invalid),
        },
        _ => return Err(ParseErr::Invalid),
    };
    let sizes = nows(&parts[1]);
    if sizes.is_empty() {
        return Err(ParseErr::Invalid);
    }
    let mut inner = Vec::new();
    for c in sizes {
        match c {
            Cv::Square(names) => {
                let mut v = Vec::new();
                for n in nows(names) {
                    match ident(n) {
                        Some(s) if !is_reserved_name(s) => v.push(s.to_string()),
                        _ => return Err(ParseErr::Invalid),
                    }
                }
                inner.push(TrackItem::Names(v));
            }
            other => inner.push(TrackItem::Size(track_size(other)?)),
        }
    }
    // auto-fill and auto-fit take fixed sizes only (CSS Grid 1).
    if !matches!(count, Repeat::Count(_)) && inner.iter().any(|t| matches!(t, TrackItem::Size(TrackSize::Fr(_) | TrackSize::Auto | TrackSize::MinContent | TrackSize::MaxContent))) {
        return Err(ParseErr::Invalid);
    }
    Ok(TrackItem::Repeat(count, inner))
}

fn track_size(c: &Cv) -> Result<TrackSize, ParseErr> {
    match c {
        Cv::Tok(t) => match &t.kind {
            Kind::Ident(s) => match s.to_ascii_lowercase().as_str() {
                "auto" => Ok(TrackSize::Auto),
                "min-content" => Ok(TrackSize::MinContent),
                "max-content" => Ok(TrackSize::MaxContent),
                _ => Err(ParseErr::Invalid),
            },
            Kind::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("fr") => {
                if *value < 0.0 {
                    Err(ParseErr::Invalid)
                } else {
                    Ok(TrackSize::Fr(*value))
                }
            }
            Kind::Dimension { value, unit, .. } => match Unit::from_name(unit) {
                Some(u) if *value >= 0.0 => Ok(TrackSize::Lp(Lp::one(u, *value))),
                Some(_) => Err(ParseErr::Invalid),
                None => Err(ParseErr::unsupported(code::UNSUPPORTED_VALUE, format!("unit {unit}"))),
            },
            Kind::Percentage(v) if *v >= 0.0 => Ok(TrackSize::Lp(Lp::one(Unit::Pct, *v))),
            Kind::Number { value, .. } if *value == 0.0 => Ok(TrackSize::Lp(Lp::px(0.0))),
            _ => Err(ParseErr::Invalid),
        },
        Cv::Func { name, args, .. } if name.eq_ignore_ascii_case("minmax") => {
            let parts = split_commas(args);
            if parts.len() != 2 {
                return Err(ParseErr::Invalid);
            }
            let min = one_size(&parts[0])?;
            let max = one_size(&parts[1])?;
            Ok(TrackSize::MinMax(Box::new(min), Box::new(max)))
        }
        Cv::Func { name, args, .. } if name.eq_ignore_ascii_case("calc") => match calc::eval(args)? {
            Calc::Len(l) if !l.0.iter().any(|(_, v)| *v < 0.0) => Ok(TrackSize::Lp(l)),
            _ => Err(ParseErr::Invalid),
        },
        Cv::Func { name, .. } => Err(ParseErr::unsupported(code::UNSUPPORTED_FUNCTION, format!("{name}()"))),
        _ => Err(ParseErr::Invalid),
    }
}

fn one_size(part: &[Cv]) -> Result<TrackSize, ParseErr> {
    match nows(part).as_slice() {
        [c] => track_size(c),
        _ => Err(ParseErr::Invalid),
    }
}

/// `auto`, an integer, a name, or `span` followed by an integer and/or a name.
pub fn grid_line(cvs: &[Cv]) -> Result<GridLine, ParseErr> {
    let items = nows(cvs);
    if let [Cv::Tok(t)] = items.as_slice() {
        if let Kind::Ident(s) = &t.kind {
            if s.eq_ignore_ascii_case("auto") {
                return Ok(GridLine::Auto);
            }
        }
    }
    let mut span = false;
    let mut index = None;
    let mut name = None;
    for c in items {
        match c {
            Cv::Tok(t) => match &t.kind {
                Kind::Ident(s) if s.eq_ignore_ascii_case("span") && !span && index.is_none() && name.is_none() => span = true,
                Kind::Ident(s) if !is_reserved_name(s) && name.is_none() => name = Some(s.clone()),
                Kind::Number { value, int: true } if index.is_none() && value.fract() == 0.0 && *value != 0.0 => {
                    index = Some(*value as i64)
                }
                _ => return Err(ParseErr::Invalid),
            },
            _ => return Err(ParseErr::Invalid),
        }
    }
    if span && index.is_none() && name.is_none() {
        return Err(ParseErr::Invalid);
    }
    if let (true, Some(i)) = (span, index) {
        if i < 0 {
            return Err(ParseErr::Invalid);
        }
    }
    Ok(GridLine::Line { index, name, span })
}

/// `none` or one or more quoted rows of named cells and `.`; all rows must have the same number of cells.
pub fn template_areas(cvs: &[Cv]) -> Result<Vec<String>, ParseErr> {
    let mut rows = Vec::new();
    for c in nows(cvs) {
        match c {
            Cv::Tok(t) => match &t.kind {
                Kind::Str(s) => rows.push(s.clone()),
                _ => return Err(ParseErr::Invalid),
            },
            _ => return Err(ParseErr::Invalid),
        }
    }
    if rows.is_empty() {
        return Err(ParseErr::Invalid);
    }
    let mut cols = None;
    for r in &rows {
        let cells: Vec<&str> = r.split_ascii_whitespace().collect();
        if cells.is_empty() || cols.is_some_and(|c| c != cells.len()) {
            return Err(ParseErr::Invalid);
        }
        if !cells.iter().all(|c| *c == "." || c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')) {
            return Err(ParseErr::Invalid);
        }
        cols = Some(cells.len());
    }
    Ok(rows)
}

/// grid-auto-flow: `row` or `column` with an optional `dense`, or `dense` alone (row). Returns the canonical text.
pub fn auto_flow(cvs: &[Cv]) -> Result<&'static str, ParseErr> {
    let mut row = false;
    let mut column = false;
    let mut dense = false;
    for c in nows(cvs) {
        match ident(c).map(|s| s.to_ascii_lowercase()) {
            Some(s) if s == "row" && !row && !column => row = true,
            Some(s) if s == "column" && !row && !column => column = true,
            Some(s) if s == "dense" && !dense => dense = true,
            _ => return Err(ParseErr::Invalid),
        }
    }
    Ok(match (column, dense) {
        (false, false) => "row",
        (true, false) => "column",
        (false, true) => "dense",
        (true, true) => "column dense",
    })
}

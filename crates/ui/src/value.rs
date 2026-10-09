//! Typed values: what a parsed declaration (a specified value) and a computed value of the documented subset are.
//! Keywords are `&'static str` from each property's table, so a value never owns text it does not need.

use crate::color::Color;
use crate::font::Family;
use crate::grid::{GridLine, TrackItem};
use crate::length::Lp;
use crate::transform::TfOp;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A keyword of the property's table (lower case). `none`, `auto` and `normal` are keywords too.
    Kw(&'static str),
    /// <length-percentage>. Computed values hold px and percentage terms only, except transform, which keeps units.
    Lp(Lp),
    /// <number> or <integer>; opacity percentages become numbers too.
    Num(f64),
    /// A colour. Computed values are always resolved colours.
    Color(Color),
    /// `currentcolor`. Only specified values have it; the computed value is the element's colour.
    CurrentColor,
    Families(Vec<Family>),
    Transform(Vec<TfOp>),
    /// Corner radius of border-*-radius: horizontal and vertical radius.
    Radius(Lp, Lp),
    /// A grid track list (grid-template-columns/rows, grid-auto-columns/rows).
    Tracks(Vec<TrackItem>),
    /// grid-template-areas rows (parsed, not rendered yet).
    Areas(Vec<String>),
    /// grid-column-start and the other grid line values.
    Line(GridLine),
    /// aspect-ratio: width / height.
    Ratio(f64, f64),
    /// box-shadow list (parsed, not rendered yet). A `None` colour is `currentcolor`.
    Shadows(Vec<Shadow>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shadow {
    pub inset: bool,
    pub x: Lp,
    pub y: Lp,
    pub blur: Option<Lp>,
    pub spread: Option<Lp>,
    pub color: Option<Color>,
}

/// Why a value was refused: not valid CSS (`Invalid`), or valid CSS outside the documented subset.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseErr {
    Invalid,
    /// The diagnostic code and a short description of the unsupported construct.
    Unsupported(&'static str, String),
}

impl ParseErr {
    pub fn unsupported(code: &'static str, what: impl Into<String>) -> ParseErr {
        ParseErr::Unsupported(code, what.into())
    }
}

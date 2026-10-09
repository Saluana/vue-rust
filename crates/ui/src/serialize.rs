//! Chrome-style serialisation of computed values (what getComputedStyle(el).getPropertyValue returns for the
//! documented subset). Lengths are px and percentages; a px and a percentage together are `calc(P% + Qpx)` with the
//! percentage first; numbers use `%g` with six digits; colours and font families follow `color` and `font` modules.
//! Transform arguments are resolved lengths, angles in degrees (the computed value normalised as Chrome does).

use crate::color::Color;
use crate::font;
use crate::grid::{GridLine, Repeat, TrackItem, TrackSize};
use crate::length::{Lp, Unit};
use crate::num;
use crate::transform::{Arg, TfOp};
use crate::value::{Shadow, Value};

/// A resolved length: its px total and percentage total, with the percentage present in the source.
pub fn lp_resolved_text(px: f64, pct: f64, has_pct: bool) -> String {
    let px_zero = px == 0.0;
    let pct_zero = pct == 0.0;
    if has_pct && !pct_zero && !px_zero {
        let sign = if px < 0.0 { '-' } else { '+' };
        format!("calc({}% {sign} {}px)", num::fmt(pct), num::fmt(px.abs()))
    } else if has_pct && !pct_zero {
        format!("{}%", num::fmt(pct))
    } else if !px_zero || !has_pct {
        format!("{}px", num::fmt(px))
    } else {
        "0%".to_string()
    }
}

/// A computed length-percentage (px and percentage terms only).
pub fn lp_text(l: &Lp) -> String {
    let (mut px, mut pct) = (0.0, 0.0);
    for &(u, v) in &l.0 {
        match u {
            Unit::Pct => pct += v,
            _ => px += v,
        }
    }
    lp_resolved_text(px, pct, l.has_pct())
}

pub fn color_text(c: Color) -> String {
    c.serialize()
}

/// The text of a value in a property's computed form. `font_size` is needed for a numeric line-height.
pub fn value_text(v: &Value) -> String {
    match v {
        Value::Kw(k) => (*k).to_string(),
        Value::Lp(l) => lp_text(l),
        Value::Num(n) => num::fmt(*n),
        Value::Color(c) => color_text(*c),
        Value::CurrentColor => "currentcolor".to_string(),
        Value::Families(f) => font::serialize_families(f),
        Value::Transform(ops) => transform_text(ops),
        Value::Radius(h, v) => {
            if h == v {
                lp_text(h)
            } else {
                format!("{} {}", lp_text(h), lp_text(v))
            }
        }
        Value::Tracks(items) => items.iter().map(track_text).collect::<Vec<_>>().join(" "),
        Value::Areas(rows) => rows.iter().map(|r| format!("\"{r}\"")).collect::<Vec<_>>().join(" "),
        Value::Line(l) => line_text(l),
        Value::Ratio(w, h) => format!("{} / {}", num::fmt(*w), num::fmt(*h)),
        Value::Shadows(list) => list.iter().map(shadow_text).collect::<Vec<_>>().join(", "),
    }
}

pub fn transform_text(ops: &[TfOp]) -> String {
    ops.iter()
        .map(|op| {
            let args: Vec<String> = op
                .args
                .iter()
                .map(|a| match a {
                    Arg::Num(n) => num::fmt(*n),
                    Arg::Lp(l) => lp_text(l),
                    Arg::Angle(v) => format!("{}deg", num::fmt(*v)),
                })
                .collect();
            format!("{}({})", op.name, args.join(", "))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn track_text(t: &TrackItem) -> String {
    match t {
        TrackItem::Size(s) => size_text(s),
        TrackItem::Repeat(r, items) => {
            let count = match r {
                Repeat::Count(n) => n.to_string(),
                Repeat::AutoFill => "auto-fill".to_string(),
                Repeat::AutoFit => "auto-fit".to_string(),
            };
            let inner: Vec<String> = items.iter().map(track_text).collect();
            format!("repeat({count}, {})", inner.join(" "))
        }
        TrackItem::Names(names) => format!("[{}]", names.join(" ")),
    }
}

fn size_text(s: &TrackSize) -> String {
    match s {
        TrackSize::Auto => "auto".to_string(),
        TrackSize::MinContent => "min-content".to_string(),
        TrackSize::MaxContent => "max-content".to_string(),
        TrackSize::Lp(l) => lp_text(l),
        TrackSize::Fr(f) => format!("{}fr", num::fmt(*f)),
        TrackSize::MinMax(a, b) => format!("minmax({}, {})", size_text(a), size_text(b)),
    }
}

fn line_text(l: &GridLine) -> String {
    match l {
        GridLine::Auto => "auto".to_string(),
        GridLine::Line { index, name, span } => {
            let mut parts = Vec::new();
            if *span {
                parts.push("span".to_string());
            }
            if let Some(i) = index {
                parts.push(i.to_string());
            }
            if let Some(n) = name {
                parts.push(n.clone());
            }
            parts.join(" ")
        }
    }
}

fn shadow_text(s: &Shadow) -> String {
    let mut parts = Vec::new();
    if s.inset {
        parts.push("inset".to_string());
    }
    parts.push(lp_text(&s.x));
    parts.push(lp_text(&s.y));
    if let Some(b) = &s.blur {
        parts.push(lp_text(b));
    }
    if let Some(sp) = &s.spread {
        parts.push(lp_text(sp));
    }
    if let Some(c) = s.color {
        parts.push(c.serialize());
    }
    parts.join(" ")
}

//! The property table: every longhand of the documented subset (design.md section 9.2) with its inheritance, its
//! grammar and its initial value. Shorthands are not here: `shorthand` maps them onto these longhands. Anything else
//! is unsupported (VNCSS5101) and the declaration has no effect.

use crate::color::Color;
use crate::value::Value;

/// A grammar: how a specified value is parsed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum G {
    /// One keyword from the list.
    Kw(&'static [&'static str]),
    /// <length-percentage> plus keywords. `pct` allows percentages; `nonneg` rejects negative lengths.
    Lp { kws: &'static [&'static str], nonneg: bool, pct: bool },
    /// <length> only (no percentages) plus keywords.
    Len { kws: &'static [&'static str], nonneg: bool },
    /// <number> or <integer>; `pct` allows a percentage (divided by 100); `kws` are extra keywords.
    Num { nonneg: bool, int: bool, pct: bool, kws: &'static [&'static str] },
    Color,
    Families,
    FontSize,
    FontWeight,
    LineHeight,
    Transform,
    /// border-*-radius: one or two <length-percentage>, non-negative.
    Radius,
    /// grid-template-rows and -columns: none or a track list.
    Tracks,
    /// grid-auto-rows and -columns: one or more track sizes.
    TrackSizes,
    /// grid-template-areas (parsed, not rendered).
    Areas,
    /// grid-row-start and the other line values.
    GridLine,
    /// grid-auto-flow.
    AutoFlow,
    /// aspect-ratio: auto or a ratio.
    Ratio,
    /// box-shadow (parsed, not rendered).
    Shadows,
    /// text-decoration-line: none or a combination of the four line keywords.
    DecorationLine,
}

/// An initial value. Kept as data so the table is `static`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum I {
    Kw(&'static str),
    Px(f64),
    Num(f64),
    Color(u8, u8, u8, u8),
    CurrentColor,
    /// The default font family of Chrome on the oracle machine (serialised as `Times`).
    DefaultFamily,
    /// Zero corner radius.
    Radius0,
    GridLineAuto,
    TracksAuto,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prop {
    pub name: &'static str,
    pub inherited: bool,
    pub grammar: G,
    pub initial: I,
}

pub const DISPLAY: &[&str] = &[
    "block", "inline", "inline-block", "flex", "inline-flex", "grid", "inline-grid", "none", "contents", "table",
    "inline-table", "table-row", "table-cell", "list-item", "flow-root",
];
pub const POSITION: &[&str] = &["static", "relative", "absolute", "fixed", "sticky"];
pub const VISIBILITY: &[&str] = &["visible", "hidden", "collapse"];
pub const BOX_SIZING: &[&str] = &["content-box", "border-box"];
pub const OVERFLOW: &[&str] = &["visible", "hidden", "clip", "scroll", "auto"];
pub const FLEX_DIRECTION: &[&str] = &["row", "row-reverse", "column", "column-reverse"];
pub const FLEX_WRAP: &[&str] = &["nowrap", "wrap", "wrap-reverse"];
pub const CONTENT_DIST: &[&str] = &[
    "normal", "stretch", "center", "start", "end", "flex-start", "flex-end", "left", "right", "space-between",
    "space-around", "space-evenly", "baseline",
];
pub const SELF_POS: &[&str] = &["stretch", "center", "start", "end", "flex-start", "flex-end", "self-start", "self-end", "baseline", "left", "right"];
pub const FONT_STYLE: &[&str] = &["normal", "italic", "oblique"];
pub const TEXT_ALIGN: &[&str] = &["start", "end", "left", "right", "center", "justify"];
pub const TEXT_TRANSFORM: &[&str] = &["none", "capitalize", "uppercase", "lowercase"];
pub const WHITE_SPACE: &[&str] = &["normal", "nowrap", "pre", "pre-wrap", "pre-line", "break-spaces"];
pub const TEXT_OVERFLOW: &[&str] = &["clip", "ellipsis"];
pub const OVERFLOW_WRAP: &[&str] = &["normal", "break-word", "anywhere"];
pub const WORD_BREAK: &[&str] = &["normal", "break-all", "keep-all", "break-word"];
pub const VERTICAL_ALIGN: &[&str] = &["baseline", "sub", "super", "text-top", "text-bottom", "middle", "top", "bottom"];
pub const POINTER_EVENTS: &[&str] = &["auto", "none"];
pub const USER_SELECT: &[&str] = &["auto", "none", "text", "all"];
pub const OBJECT_FIT: &[&str] = &["fill", "contain", "cover", "none", "scale-down"];
pub const BORDER_STYLE: &[&str] = &["none", "hidden", "dotted", "dashed", "solid", "double", "groove", "ridge", "inset", "outset"];
pub const OUTLINE_STYLE: &[&str] = &["auto", "none", "dotted", "dashed", "solid", "double", "groove", "ridge", "inset", "outset"];
pub const LINE_WIDTH: &[&str] = &["thin", "medium", "thick"];
pub const CURSOR: &[&str] = &[
    "auto", "default", "none", "context-menu", "help", "pointer", "progress", "wait", "cell", "crosshair", "text",
    "vertical-text", "alias", "copy", "move", "no-drop", "not-allowed", "grab", "grabbing", "all-scroll", "col-resize",
    "row-resize", "n-resize", "e-resize", "s-resize", "w-resize", "ne-resize", "nw-resize", "se-resize", "sw-resize",
    "ew-resize", "ns-resize", "nesw-resize", "nwse-resize", "zoom-in", "zoom-out",
];
pub const SIZE_KWS: &[&str] = &["xx-small", "x-small", "small", "medium", "large", "x-large", "xx-large", "xxx-large", "larger", "smaller"];
pub const WEIGHT_KWS: &[&str] = &["normal", "bold", "bolder", "lighter"];
const AUTO_KWS: &[&str] = &["auto"];
const AUTO_SIZE_KWS: &[&str] = &["auto", "min-content", "max-content"];
const MAX_SIZE_KWS: &[&str] = &["none", "min-content", "max-content"];
const NORMAL_KWS: &[&str] = &["normal"];
const NO_KWS: &[&str] = &[];
const FLEX_BASIS_KWS: &[&str] = &["auto", "content", "max-content", "min-content"];

const fn p(name: &'static str, inherited: bool, grammar: G, initial: I) -> Prop {
    Prop { name, inherited, grammar, initial }
}

const SIZE: G = G::Lp { kws: AUTO_SIZE_KWS, nonneg: true, pct: true };
const MAX_SIZE: G = G::Lp { kws: MAX_SIZE_KWS, nonneg: true, pct: true };
const OFFSET: G = G::Lp { kws: AUTO_KWS, nonneg: false, pct: true };
const PADDING: G = G::Lp { kws: NO_KWS, nonneg: true, pct: true };
const MARGIN: G = G::Lp { kws: AUTO_KWS, nonneg: false, pct: true };
const BORDER_WIDTH: G = G::Len { kws: LINE_WIDTH, nonneg: true };
const BORDER_COLOR: G = G::Color;
const GAP: G = G::Lp { kws: NORMAL_KWS, nonneg: true, pct: true };

/// The table. Sorted by name (lookup is a binary search; the test checks the order).
pub static PROPS: &[Prop] = &[
    p("align-content", false, G::Kw(CONTENT_DIST), I::Kw("normal")),
    p("align-items", false, G::Kw(&["normal", "stretch", "center", "start", "end", "flex-start", "flex-end", "self-start", "self-end", "baseline", "left", "right"]), I::Kw("normal")),
    p("align-self", false, G::Kw(&["auto", "normal", "stretch", "center", "start", "end", "flex-start", "flex-end", "self-start", "self-end", "baseline", "left", "right"]), I::Kw("auto")),
    p("aspect-ratio", false, G::Ratio, I::Kw("auto")),
    p("background-color", false, G::Color, I::Color(0, 0, 0, 0)),
    p("border-bottom-color", false, BORDER_COLOR, I::CurrentColor),
    p("border-bottom-left-radius", false, G::Radius, I::Radius0),
    p("border-bottom-right-radius", false, G::Radius, I::Radius0),
    p("border-bottom-style", false, G::Kw(BORDER_STYLE), I::Kw("none")),
    p("border-bottom-width", false, BORDER_WIDTH, I::Kw("medium")),
    p("border-left-color", false, BORDER_COLOR, I::CurrentColor),
    p("border-left-style", false, G::Kw(BORDER_STYLE), I::Kw("none")),
    p("border-left-width", false, BORDER_WIDTH, I::Kw("medium")),
    p("border-right-color", false, BORDER_COLOR, I::CurrentColor),
    p("border-right-style", false, G::Kw(BORDER_STYLE), I::Kw("none")),
    p("border-right-width", false, BORDER_WIDTH, I::Kw("medium")),
    p("border-top-color", false, BORDER_COLOR, I::CurrentColor),
    p("border-top-left-radius", false, G::Radius, I::Radius0),
    p("border-top-right-radius", false, G::Radius, I::Radius0),
    p("border-top-style", false, G::Kw(BORDER_STYLE), I::Kw("none")),
    p("border-top-width", false, BORDER_WIDTH, I::Kw("medium")),
    p("bottom", false, OFFSET, I::Kw("auto")),
    p("box-shadow", false, G::Shadows, I::Kw("none")),
    p("box-sizing", false, G::Kw(BOX_SIZING), I::Kw("content-box")),
    p("color", true, G::Color, I::Color(0, 0, 0, 255)),
    p("column-gap", false, GAP, I::Kw("normal")),
    p("cursor", true, G::Kw(CURSOR), I::Kw("auto")),
    p("display", false, G::Kw(DISPLAY), I::Kw("inline")),
    p("flex-basis", false, G::Lp { kws: FLEX_BASIS_KWS, nonneg: true, pct: true }, I::Kw("auto")),
    p("flex-direction", false, G::Kw(FLEX_DIRECTION), I::Kw("row")),
    p("flex-grow", false, G::Num { nonneg: true, int: false, pct: false, kws: NO_KWS }, I::Num(0.0)),
    p("flex-shrink", false, G::Num { nonneg: true, int: false, pct: false, kws: NO_KWS }, I::Num(1.0)),
    p("flex-wrap", false, G::Kw(FLEX_WRAP), I::Kw("nowrap")),
    p("font-family", true, G::Families, I::DefaultFamily),
    p("font-size", true, G::FontSize, I::Px(16.0)),
    p("font-style", true, G::Kw(FONT_STYLE), I::Kw("normal")),
    p("font-weight", true, G::FontWeight, I::Num(400.0)),
    p("grid-auto-columns", false, G::TrackSizes, I::TracksAuto),
    p("grid-auto-flow", false, G::AutoFlow, I::Kw("row")),
    p("grid-auto-rows", false, G::TrackSizes, I::TracksAuto),
    p("grid-column-end", false, G::GridLine, I::GridLineAuto),
    p("grid-column-start", false, G::GridLine, I::GridLineAuto),
    p("grid-row-end", false, G::GridLine, I::GridLineAuto),
    p("grid-row-start", false, G::GridLine, I::GridLineAuto),
    p("grid-template-areas", false, G::Areas, I::Kw("none")),
    p("grid-template-columns", false, G::Tracks, I::Kw("none")),
    p("grid-template-rows", false, G::Tracks, I::Kw("none")),
    p("height", false, SIZE, I::Kw("auto")),
    p("justify-content", false, G::Kw(CONTENT_DIST), I::Kw("normal")),
    p("justify-items", false, G::Kw(&["normal", "stretch", "center", "start", "end", "flex-start", "flex-end", "self-start", "self-end", "baseline", "left", "right"]), I::Kw("normal")),
    p("justify-self", false, G::Kw(&["auto", "normal", "stretch", "center", "start", "end", "flex-start", "flex-end", "self-start", "self-end", "baseline", "left", "right"]), I::Kw("auto")),
    p("left", false, OFFSET, I::Kw("auto")),
    p("letter-spacing", true, G::Lp { kws: NORMAL_KWS, nonneg: false, pct: true }, I::Kw("normal")),
    p("line-height", true, G::LineHeight, I::Kw("normal")),
    p("margin-bottom", false, MARGIN, I::Px(0.0)),
    p("margin-left", false, MARGIN, I::Px(0.0)),
    p("margin-right", false, MARGIN, I::Px(0.0)),
    p("margin-top", false, MARGIN, I::Px(0.0)),
    p("max-height", false, MAX_SIZE, I::Kw("none")),
    p("max-width", false, MAX_SIZE, I::Kw("none")),
    p("min-height", false, SIZE, I::Kw("auto")),
    p("min-width", false, SIZE, I::Kw("auto")),
    p("object-fit", false, G::Kw(OBJECT_FIT), I::Kw("fill")),
    p("opacity", false, G::Num { nonneg: false, int: false, pct: true, kws: NO_KWS }, I::Num(1.0)),
    p("order", false, G::Num { nonneg: false, int: true, pct: false, kws: NO_KWS }, I::Num(0.0)),
    p("outline-color", false, BORDER_COLOR, I::CurrentColor),
    p("outline-offset", false, G::Len { kws: NO_KWS, nonneg: false }, I::Px(0.0)),
    p("outline-style", false, G::Kw(OUTLINE_STYLE), I::Kw("none")),
    p("outline-width", false, BORDER_WIDTH, I::Kw("medium")),
    p("overflow-wrap", true, G::Kw(OVERFLOW_WRAP), I::Kw("normal")),
    p("overflow-x", false, G::Kw(OVERFLOW), I::Kw("visible")),
    p("overflow-y", false, G::Kw(OVERFLOW), I::Kw("visible")),
    p("padding-bottom", false, PADDING, I::Px(0.0)),
    p("padding-left", false, PADDING, I::Px(0.0)),
    p("padding-right", false, PADDING, I::Px(0.0)),
    p("padding-top", false, PADDING, I::Px(0.0)),
    p("pointer-events", true, G::Kw(POINTER_EVENTS), I::Kw("auto")),
    p("position", false, G::Kw(POSITION), I::Kw("static")),
    p("right", false, OFFSET, I::Kw("auto")),
    p("row-gap", false, GAP, I::Kw("normal")),
    p("text-align", true, G::Kw(TEXT_ALIGN), I::Kw("start")),
    p("text-decoration-line", false, G::DecorationLine, I::Kw("none")),
    p("text-overflow", false, G::Kw(TEXT_OVERFLOW), I::Kw("clip")),
    p("text-transform", true, G::Kw(TEXT_TRANSFORM), I::Kw("none")),
    p("top", false, OFFSET, I::Kw("auto")),
    p("transform", false, G::Transform, I::Kw("none")),
    p("user-select", false, G::Kw(USER_SELECT), I::Kw("auto")),
    p("vertical-align", false, G::Lp { kws: VERTICAL_ALIGN, nonneg: false, pct: true }, I::Kw("baseline")),
    p("visibility", true, G::Kw(VISIBILITY), I::Kw("visible")),
    p("white-space", true, G::Kw(WHITE_SPACE), I::Kw("normal")),
    p("width", false, SIZE, I::Kw("auto")),
    p("word-break", true, G::Kw(WORD_BREAK), I::Kw("normal")),
    p("z-index", false, G::Num { nonneg: false, int: true, pct: false, kws: AUTO_KWS }, I::Kw("auto")),
];

/// Looks up a longhand by its lower-case name.
pub fn lookup(name: &str) -> Option<&'static Prop> {
    PROPS.binary_search_by(|q| q.name.cmp(name)).ok().map(|i| &PROPS[i])
}

/// The index of a longhand in `PROPS` (the position of its value in a `ComputedStyle`).
pub fn index_of(name: &str) -> Option<usize> {
    PROPS.binary_search_by(|q| q.name.cmp(name)).ok()
}

/// The initial value as a typed value.
pub fn initial(prop: &Prop) -> Value {
    use crate::font::Family;
    use crate::grid::{GridLine, TrackItem, TrackSize};
    use crate::length::Lp;
    match prop.initial {
        I::Kw(k) => Value::Kw(k),
        I::Px(v) => Value::Lp(Lp::px(v)),
        I::Num(v) => Value::Num(v),
        I::Color(r, g, b, a) => Value::Color(Color { r, g, b, a, translucent: false }),
        I::CurrentColor => Value::CurrentColor,
        I::DefaultFamily => Value::Families(vec![Family::Named("Times".to_string())]),
        I::Radius0 => Value::Radius(Lp::px(0.0), Lp::px(0.0)),
        I::GridLineAuto => Value::Line(GridLine::Auto),
        I::TracksAuto => Value::Tracks(vec![TrackItem::Size(TrackSize::Auto)]),
    }
}

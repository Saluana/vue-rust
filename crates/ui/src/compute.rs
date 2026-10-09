//! The cascade and the computed values (CSS Cascade 4 and CSS Values 4, for the documented subset).
//! For each element, vn-css cascades the user-agent sheet, the author sheets and the inline style attribute. Each
//! winning declaration is substituted (var()), expanded (shorthands) and parsed with its longhand grammar; a value
//! that is not valid CSS is dropped with a diagnostic, as browsers drop it, so the cascade falls through to the next
//! declaration. Then the computed values follow: CSS-wide keywords, inheritance, initial values, relative units
//! (em, rem, viewport, percentages kept), font-size and font-weight keywords, currentcolor, and display blockification.

use std::collections::{HashMap, HashSet};

use vn_css::cascade::{cascade, CascadedDeclarations, Origin};
use vn_css::custom::{computed_custom_properties, CustomProperties};
use vn_css::media::MediaEnv;
use vn_css::parse::{parse_declarations, parse_stylesheet, ParseOptions};
use vn_css::token::Kind;
use vn_css::{Declaration, Stylesheet};
use vn_dom::{Document, NodeId};

use crate::color::Color;
use crate::diag::{code, StyleDiagnostic};
use crate::dom::{self, El, StateProvider};
use crate::font::{relative_weight, size_keyword};
use crate::length::{Ctx, Lp, Unit};
use crate::parse;
use crate::props::{self, Prop, G};
use crate::shorthand;
use crate::style::ComputedStyle;
use crate::tree::{self, Cv};
use crate::ua;
use crate::transform::{Arg, TfOp};
use crate::value::{ParseErr, Value};
use crate::grid::{TrackItem, TrackSize};

/// An author stylesheet with the diagnostics of its parse (VNCSS1xxx-2xxx from vn-css, reported once per sheet).
pub struct Sheet {
    pub sheet: Stylesheet,
    pub diagnostics: Vec<StyleDiagnostic>,
}

impl Sheet {
    pub fn parse(src: &str) -> Sheet {
        let r = parse_stylesheet(src, &ParseOptions::default());
        let diagnostics = r
            .diagnostics
            .into_iter()
            .map(|d| StyleDiagnostic {
                code: d.code,
                property: String::new(),
                message: d.message,
                hint: d.hint,
                node: None,
                sheet: None,
                span: Some((d.start, d.end)),
            })
            .collect();
        Sheet { sheet: r.sheet, diagnostics }
    }
}

#[derive(Debug, Clone)]
pub struct ElementStyle {
    pub node: NodeId,
    /// Index of the parent element in `StyleTree::elements` (None for the root element).
    pub parent: Option<usize>,
    pub style: ComputedStyle,
}

/// The computed styles of every element, in document order (parents first).
#[derive(Debug, Clone, Default)]
pub struct StyleTree {
    pub elements: Vec<ElementStyle>,
    pub diagnostics: Vec<StyleDiagnostic>,
    index: HashMap<NodeId, usize>,
}

impl StyleTree {
    pub fn style(&self, node: NodeId) -> Option<&ComputedStyle> {
        self.index.get(&node).map(|&i| &self.elements[i].style)
    }

    pub fn root(&self) -> Option<&ElementStyle> {
        self.elements.first()
    }
}

/// Computes the styles of every element of `doc`. The document is mutable only because vn-dom reads attributes and
/// form state through `&mut self`; nothing is changed. The viewport for vw/vh/vmin/vmax and media queries is `env`'s
/// width and height.
pub fn compute_styles(doc: &mut Document, sheets: &[Sheet], env: &MediaEnv, states: &dyn StateProvider) -> StyleTree {
    let snap = dom::snapshot(doc, states);
    let mut sheet_refs: Vec<(&Stylesheet, Origin)> = vec![(ua::sheet(), Origin::UserAgent)];
    sheet_refs.extend(sheets.iter().map(|s| (&s.sheet, Origin::Author)));
    let inline_count = sheet_refs.len();

    let mut sink = Sink::default();
    for (i, s) in sheets.iter().enumerate() {
        for d in &s.diagnostics {
            let mut d = d.clone();
            d.sheet = Some(i);
            sink.out.push(d);
        }
    }

    // Inline style attributes, parsed once; their diagnostics are per element (the text differs by element).
    let mut inline: Vec<Vec<Declaration>> = Vec::with_capacity(snap.els.len());
    for (i, e) in snap.els.iter().enumerate() {
        let text = e.attrs.iter().find(|(k, _)| k == "style").map(|(_, v)| v.as_str()).unwrap_or("");
        let (decls, diags) = parse_declarations(text);
        for d in diags {
            sink.push(StyleDiagnostic {
                code: d.code,
                property: String::new(),
                message: d.message,
                hint: d.hint,
                node: Some(snap.els[i].node),
                sheet: None,
                span: Some((d.start, d.end)),
            });
        }
        inline.push(decls);
    }

    let by_node: HashMap<NodeId, usize> = snap.els.iter().enumerate().map(|(i, e)| (e.node, i)).collect();
    let mut tree = StyleTree::default();
    let mut styles: Vec<ComputedStyle> = Vec::with_capacity(snap.els.len());
    // The cascade of each ancestor, root first; a stack entry is (element index, cascade).
    let mut stack: Vec<(usize, CascadedDeclarations)> = Vec::new();
    let vp = (env.width, env.height);
    for i in 0..snap.els.len() {
        let parent = snap.els[i].parent;
        while stack.last().is_some_and(|(top, _)| Some(*top) != parent) {
            stack.pop();
        }
        let el = El { snap: &snap, i };
        let cas = cascade(&el, &sheet_refs, env, &inline[i]);
        let chain: Vec<&CascadedDeclarations> = stack.iter().map(|(_, c)| c).chain(std::iter::once(&cas)).collect();
        let custom = computed_custom_properties(&chain);
        drop(chain);
        let node = snap.els[i].node;
        let win = winners(&cas, &custom, inline_count, node, &mut sink);
        let parent_style = parent.map(|p| &styles[p]);
        let root_fs = styles.first().map_or(16.0, |s| s.font_size());
        let style = compute_element(&win, custom, parent_style, root_fs, vp);
        styles.push(style.clone());
        tree.elements.push(ElementStyle { node, parent, style });
        stack.push((i, cas));
    }
    tree.index = by_node;
    tree.diagnostics = sink.out;
    tree
}

/// Diagnostics are reported once per source declaration (and once per element for inline styles).
#[derive(Default)]
struct Sink {
    out: Vec<StyleDiagnostic>,
    seen: HashSet<(Option<usize>, Option<NodeId>, Option<usize>, &'static str, String)>,
}

impl Sink {
    fn push(&mut self, d: StyleDiagnostic) {
        let key = (d.sheet, if d.sheet.is_none() { d.node } else { None }, d.span.map(|s| s.0), d.code, d.property.clone());
        if self.seen.insert(key) {
            self.out.push(d);
        }
    }
}

/// What a declaration does to one longhand before computation.
#[derive(Debug, Clone, PartialEq)]
enum Assign {
    Wide(Wide),
    Val(Value),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Wide {
    Initial,
    Inherit,
    Unset,
}

fn index(name: &str) -> usize {
    props::index_of(name).unwrap_or(usize::MAX)
}

fn contains_var(toks: &[vn_css::token::Token]) -> bool {
    toks.iter().any(|t| matches!(&t.kind, Kind::Function(n) if n.eq_ignore_ascii_case("var")))
}

/// The CSS-wide keyword that a value consists of (lower case), if any.
fn css_wide(cvs: &[Cv]) -> Option<&'static str> {
    const WIDE: &[&str] = &["inherit", "initial", "unset", "revert", "revert-layer"];
    let items = tree::nows(cvs);
    match items.as_slice() {
        [c] => tree::keyword(c, WIDE),
        _ => None,
    }
}

/// The per-property assignments of one element, in the order of the cascade (later candidates overwrite earlier).
fn winners(cas: &CascadedDeclarations, custom: &CustomProperties, inline_count: usize, node: NodeId, sink: &mut Sink) -> Vec<Option<Assign>> {
    let mut win: Vec<Option<Assign>> = vec![None; props::PROPS.len()];
    for c in cas.candidates() {
        let decl = c.decl;
        if decl.is_custom() {
            continue;
        }
        let (sheet, node_key) = source_of(c.order.0, inline_count, node);
        let name = decl.name.as_str();
        let short = shorthand::is_shorthand(name);
        let prop = props::lookup(name);
        if prop.is_none() && !short {
            sink.push(diag(code::UNSUPPORTED_PROPERTY, name, format!("property `{name}` is not supported"), "the declaration has no effect; see crates/ui/src/props.rs for the subset", sheet, node_key, decl));
            continue;
        }
        let targets: Vec<&'static str> = if short { shorthand::longhands_of(name).to_vec() } else { vec![prop.map_or("", |p| p.name)] };
        let var = contains_var(&decl.value.tokens);
        let cvs = if var {
            match custom.substitute(&decl.value) {
                Ok(text) => tree::build(&text.tokens).ok(),
                Err(_) => {
                    // Invalid at computed-value time: every longhand the declaration sets becomes unset.
                    for t in &targets {
                        win[index(t)] = Some(Assign::Wide(Wide::Unset));
                    }
                    continue;
                }
            }
        } else {
            tree::build(&decl.value.tokens).ok()
        };
        let Some(cvs) = cvs else {
            sink.push(diag(code::INVALID_VALUE, name, format!("invalid value for `{name}`"), "the declaration is ignored, as browsers ignore it", sheet, node_key, decl));
            continue;
        };
        if !var {
            if let Some(kw) = css_wide(&cvs) {
                let assign = match kw {
                    "inherit" => Wide::Inherit,
                    "initial" => Wide::Initial,
                    _ => Wide::Unset,
                };
                if kw == "revert" || kw == "revert-layer" {
                    sink.push(diag(code::REVERT_AS_UNSET, name, format!("`{kw}` is treated as `unset`"), "the user-agent rollback is not modelled", sheet, node_key, decl));
                }
                for t in &targets {
                    win[index(t)] = Some(Assign::Wide(assign));
                }
                continue;
            }
        }
        let parsed: Result<Vec<(&'static str, Value)>, ParseErr> = if short {
            match shorthand::expand(name, &cvs) {
                Some(r) => r,
                None => Err(ParseErr::Invalid),
            }
        } else {
            parse::parse(prop.map_or(G::Kw(&[]), |p| p.grammar), &cvs).map(|v| vec![(prop.map_or("", |p| p.name), v)])
        };
        match parsed {
            Ok(list) => {
                for (longhand, v) in list {
                    win[index(longhand)] = Some(Assign::Val(v));
                }
            }
            Err(ParseErr::Invalid) if var => {
                // Substituted text that is not valid for the property: invalid at computed-value time.
                for t in &targets {
                    win[index(t)] = Some(Assign::Wide(Wide::Unset));
                }
            }
            Err(ParseErr::Invalid) => {
                sink.push(diag(code::INVALID_VALUE, name, format!("invalid value for `{name}`"), "the declaration is ignored, as browsers ignore it", sheet, node_key, decl));
            }
            Err(ParseErr::Unsupported(c, what)) => {
                sink.push(diag(c, name, format!("`{what}` is not supported for `{name}`"), "the declaration has no effect; see design.md section 9.2 for the subset", sheet, node_key, decl));
            }
        }
    }
    win
}

/// Which source a candidate came from: an author sheet index (None for the inline attribute) and the element for
/// inline styles. The user-agent sheet (index 0) is reported as sheet index usize::MAX (it must not produce any).
fn source_of(order_sheet: usize, inline_count: usize, node: NodeId) -> (Option<usize>, Option<NodeId>) {
    if order_sheet == inline_count {
        (None, Some(node))
    } else if order_sheet == 0 {
        (Some(usize::MAX), None)
    } else {
        (Some(order_sheet - 1), None)
    }
}

fn diag(c: &'static str, property: &str, message: String, hint: &'static str, sheet: Option<usize>, node: Option<NodeId>, decl: &Declaration) -> StyleDiagnostic {
    StyleDiagnostic {
        code: c,
        property: property.to_string(),
        message,
        hint,
        node,
        sheet,
        span: Some((decl.start, decl.end)),
    }
}

/// Computes one element from its winning assignments.
fn compute_element(win: &[Option<Assign>], custom: CustomProperties, parent: Option<&ComputedStyle>, root_fs: f64, vp: (f64, f64)) -> ComputedStyle {
    let mut vals: Vec<Value> = props::PROPS.iter().map(props::initial).collect();
    let parent_fs = parent.map_or(16.0, |p| p.font_size());
    // `rem` is the root element's font size; the root element itself uses the initial 16px for its font-size.
    let rem_base = if parent.is_none() { 16.0 } else { root_fs };
    let fs_idx = index("font-size");
    let fs = match &win[fs_idx] {
        None => parent_fs,
        Some(Assign::Wide(Wide::Initial)) => 16.0,
        Some(Assign::Wide(_)) => parent_fs,
        Some(Assign::Val(Value::Kw(k))) => match *k {
            "larger" => parent_fs * 1.2,
            "smaller" => parent_fs / 1.2,
            other => size_keyword(other).unwrap_or(16.0),
        },
        Some(Assign::Val(Value::Lp(l))) => {
            let r = l.resolve(&Ctx { font_size: parent_fs, root_font_size: rem_base, viewport_w: vp.0, viewport_h: vp.1 });
            (r.px + r.pct / 100.0 * parent_fs).max(0.0)
        }
        Some(Assign::Val(_)) => 16.0,
    };
    let root_for_rem = if parent.is_none() { fs } else { root_fs };
    let ctx = Ctx { font_size: fs, root_font_size: root_for_rem, viewport_w: vp.0, viewport_h: vp.1 };
    vals[fs_idx] = Value::Lp(Lp::px(fs));

    // color first: currentcolor in every other property refers to it.
    let color_idx = index("color");
    let parent_color = parent.map_or(Color::BLACK, |p| match p.at(color_idx) {
        Value::Color(c) => *c,
        _ => Color::BLACK,
    });
    let color = match &win[color_idx] {
        None | Some(Assign::Wide(Wide::Inherit | Wide::Unset)) => parent_color,
        Some(Assign::Wide(Wide::Initial)) => Color::BLACK,
        Some(Assign::Val(Value::Color(c))) => *c,
        Some(Assign::Val(_)) => parent_color,
    };
    vals[color_idx] = Value::Color(color);
    let parent_weight = parent.map_or(400.0, |p| match p.at(index("font-weight")) {
        Value::Num(n) => *n,
        _ => 400.0,
    });
    let env = Env { ctx, color, parent_color, parent_weight };

    for (idx, p) in props::PROPS.iter().enumerate() {
        if idx == fs_idx || idx == color_idx || is_border_width(p) {
            continue;
        }
        let v = resolve_one(p, idx, win, parent);
        vals[idx] = finalize(p, v, &env);
    }
    // Border widths depend on the border style of the same side: computed after the styles, zero when none or hidden.
    for side in ["top", "right", "bottom", "left"] {
        let w = index(&format!("border-{side}-width"));
        let s = index(&format!("border-{side}-style"));
        let style_none = matches!(vals[s], Value::Kw("none" | "hidden"));
        let p = &props::PROPS[w];
        let v = resolve_one(p, w, win, parent);
        let width = finalize(p, v, &env);
        vals[w] = if style_none { Value::Lp(Lp::px(0.0)) } else { width };
    }
    // CSS Overflow 3: when one axis is neither visible nor clip, visible becomes auto and clip becomes hidden.
    let (ox, oy) = (index("overflow-x"), index("overflow-y"));
    let kw = |v: &Value| if let Value::Kw(k) = v { *k } else { "visible" };
    let (x, y) = (kw(&vals[ox]), kw(&vals[oy]));
    if !matches!(x, "visible" | "clip") || !matches!(y, "visible" | "clip") {
        for idx in [ox, oy] {
            vals[idx] = match kw(&vals[idx]) {
                "visible" => Value::Kw("auto"),
                "clip" => Value::Kw("hidden"),
                _ => vals[idx].clone(),
            };
        }
    }
    blockify(&mut vals, parent);
    ComputedStyle::new(vals, custom, fs)
}

fn is_border_width(p: &Prop) -> bool {
    p.name.starts_with("border-") && p.name.ends_with("-width")
}

/// The specified-or-inherited value of one longhand, before the computed-value rules run.
fn resolve_one(p: &Prop, idx: usize, win: &[Option<Assign>], parent: Option<&ComputedStyle>) -> Value {
    let inherited = || parent.map_or_else(|| props::initial(p), |ps| ps.at(idx).clone());
    let initial = || props::initial(p);
    match &win[idx] {
        None => {
            if p.inherited {
                inherited()
            } else {
                initial()
            }
        }
        Some(Assign::Wide(Wide::Initial)) => initial(),
        Some(Assign::Wide(Wide::Inherit)) => inherited(),
        Some(Assign::Wide(Wide::Unset)) => {
            if p.inherited {
                inherited()
            } else {
                initial()
            }
        }
        Some(Assign::Val(v)) => v.clone(),
    }
}

/// Per-element inputs of the computed-value rules.
struct Env {
    ctx: Ctx,
    color: Color,
    parent_color: Color,
    parent_weight: f64,
}

fn nonneg(g: G) -> bool {
    matches!(g, G::Lp { nonneg: true, .. } | G::Len { nonneg: true, .. } | G::Radius)
}

/// Chrome clamps lengths to the layout range of +-2^25 px (a LayoutUnit), so an infinite calc() is that value.
const LAYOUT_LIMIT: f64 = 33_554_432.0;

/// A length-percentage at computed-value time: em, rem and viewport units become px; percentages stay.
fn lp_computed(l: &Lp, ctx: &Ctx, non_negative: bool) -> Lp {
    let r = l.resolve(ctx);
    let (mut px, mut pct) = (r.px.clamp(-LAYOUT_LIMIT, LAYOUT_LIMIT), r.pct.clamp(-LAYOUT_LIMIT, LAYOUT_LIMIT));
    if non_negative {
        if r.has_pct {
            if px == 0.0 && pct < 0.0 {
                pct = 0.0;
            }
        } else if px < 0.0 {
            px = 0.0;
        }
    }
    if r.has_pct { Lp(vec![(Unit::Px, px), (Unit::Pct, pct)]) } else { Lp::px(px) }
}

/// The computed-value rules for one value.
fn finalize(p: &Prop, v: Value, env: &Env) -> Value {
    let nn = nonneg(p.grammar);
    match v {
        Value::Kw(k) if p.name == "font-weight" => match k {
            "normal" => Value::Num(400.0),
            "bold" => Value::Num(700.0),
            "bolder" | "lighter" => Value::Num(relative_weight(k, env.parent_weight).unwrap_or(400.0)),
            other => Value::Kw(other),
        },
        Value::Num(n) if p.name == "opacity" => Value::Num(n.clamp(0.0, 1.0)),
        Value::Lp(l) if p.name == "line-height" => {
            let r = l.resolve(&env.ctx);
            Value::Lp(Lp::px((r.px + r.pct / 100.0 * env.ctx.font_size).max(0.0)))
        }
        Value::Lp(l) if p.name == "outline-offset" => {
            // Chrome keeps outline-offset in whole pixels: truncated, but a non-zero offset is at least one pixel.
            let px = l.resolve(&env.ctx).px.clamp(-LAYOUT_LIMIT, LAYOUT_LIMIT);
            let whole = if px == 0.0 { 0.0 } else { px.signum() * px.abs().floor().max(1.0) };
            Value::Lp(Lp::px(whole))
        }
        // The thin, medium and thick keywords are 1px, 3px and 5px (Chrome's border and outline widths).
        Value::Kw(k) if p.name.ends_with("-width") && matches!(k, "thin" | "medium" | "thick") => Value::Lp(Lp::px(match k {
            "thin" => 1.0,
            "thick" => 5.0,
            _ => 3.0,
        })),
        Value::Lp(l) => Value::Lp(lp_computed(&l, &env.ctx, nn)),
        Value::CurrentColor if p.name == "color" => Value::Color(env.parent_color),
        Value::CurrentColor => Value::Color(env.color),
        Value::Radius(h, v) => Value::Radius(lp_computed(&h, &env.ctx, true), lp_computed(&v, &env.ctx, true)),
        Value::Transform(ops) => Value::Transform(
            ops.into_iter()
                .map(|op| TfOp {
                    name: op.name,
                    args: op
                        .args
                        .into_iter()
                        .map(|a| match a {
                            Arg::Lp(l) => Arg::Lp(lp_computed(&l, &env.ctx, false)),
                            other => other,
                        })
                        .collect(),
                })
                .collect(),
        ),
        Value::Tracks(items) => Value::Tracks(items.into_iter().map(|t| track_computed(t, &env.ctx)).collect()),
        Value::Shadows(list) => Value::Shadows(
            list.into_iter()
                .map(|mut s| {
                    s.x = lp_computed(&s.x, &env.ctx, false);
                    s.y = lp_computed(&s.y, &env.ctx, false);
                    s.blur = s.blur.map(|b| lp_computed(&b, &env.ctx, true));
                    s.spread = s.spread.map(|b| lp_computed(&b, &env.ctx, false));
                    s.color = Some(s.color.unwrap_or(env.color));
                    s
                })
                .collect(),
        ),
        other => other,
    }
}

/// Track sizes at computed-value time: lengths resolved, percentages kept.
fn track_computed(t: TrackItem, ctx: &Ctx) -> TrackItem {
    fn size(s: TrackSize, ctx: &Ctx) -> TrackSize {
        match s {
            TrackSize::Lp(l) => TrackSize::Lp(lp_computed(&l, ctx, true)),
            TrackSize::MinMax(a, b) => TrackSize::MinMax(Box::new(size(*a, ctx)), Box::new(size(*b, ctx))),
            other => other,
        }
    }
    match t {
        TrackItem::Size(s) => TrackItem::Size(size(s, ctx)),
        TrackItem::Repeat(r, items) => TrackItem::Repeat(r, items.into_iter().map(|i| track_computed(i, ctx)).collect()),
        other => other,
    }
}

/// CSS Display 3 blockification: absolutely positioned elements, the root element and the children of flex and grid
/// containers are blockified.
fn blockify(vals: &mut [Value], parent: Option<&ComputedStyle>) {
    let display_idx = index("display");
    let position = match &vals[index("position")] {
        Value::Kw(k) => *k,
        _ => "static",
    };
    let parent_display = parent.map(|p| match p.at(display_idx) {
        Value::Kw(k) => *k,
        _ => "inline",
    });
    let container = matches!(parent_display, Some("flex" | "inline-flex" | "grid" | "inline-grid"));
    let needs = parent.is_none() || matches!(position, "absolute" | "fixed") || container;
    if !needs {
        return;
    }
    if let Value::Kw(d) = vals[display_idx] {
        let new = match d {
            "inline" | "inline-block" => "block",
            "inline-table" => "table",
            "inline-flex" => "flex",
            "inline-grid" => "grid",
            other => other,
        };
        vals[display_idx] = Value::Kw(new);
    }
}

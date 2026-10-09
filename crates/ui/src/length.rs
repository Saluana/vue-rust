//! Length-percentage values as linear combinations of units. A `calc()` of lengths and percentages is always linear,
//! because CSS type-checks products and quotients so that one factor is a plain number. A combination is therefore
//! a list of (unit, coefficient) terms, and resolution against a context gives an absolute part (px) and a
//! percentage part, which layout resolves against the containing block.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unit {
    Px,
    Em,
    Rem,
    Vw,
    Vh,
    Vmin,
    Vmax,
    Pct,
}

impl Unit {
    /// Dimension unit names are ASCII case-insensitive. `%` is handled by the caller.
    pub fn from_name(name: &str) -> Option<Unit> {
        Some(match name.to_ascii_lowercase().as_str() {
            "px" => Unit::Px,
            "em" => Unit::Em,
            "rem" => Unit::Rem,
            "vw" => Unit::Vw,
            "vh" => Unit::Vh,
            "vmin" => Unit::Vmin,
            "vmax" => Unit::Vmax,
            _ => return None,
        })
    }

    pub fn css(self) -> &'static str {
        match self {
            Unit::Px => "px",
            Unit::Em => "em",
            Unit::Rem => "rem",
            Unit::Vw => "vw",
            Unit::Vh => "vh",
            Unit::Vmin => "vmin",
            Unit::Vmax => "vmax",
            Unit::Pct => "%",
        }
    }
}

/// A linear combination of length terms, in source order with one term per unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Lp(pub Vec<(Unit, f64)>);

impl Lp {
    pub fn one(unit: Unit, v: f64) -> Lp {
        Lp(vec![(unit, v)])
    }

    pub fn px(v: f64) -> Lp {
        Lp::one(Unit::Px, v)
    }

    /// `self + k * other`, merging terms that share a unit. A term that cancels stays (as `calc(50% - 50%)` does).
    pub fn add_scaled(&self, other: &Lp, k: f64) -> Lp {
        let mut out = self.0.clone();
        for &(u, v) in &other.0 {
            match out.iter_mut().find(|(w, _)| *w == u) {
                Some(t) => t.1 += k * v,
                None => out.push((u, k * v)),
            }
        }
        Lp(out)
    }

    pub fn scale(&self, k: f64) -> Lp {
        Lp(self.0.iter().map(|&(u, v)| (u, k * v)).collect())
    }

    /// True when the combination has a percentage term (even one whose coefficient is zero).
    pub fn has_pct(&self) -> bool {
        self.0.iter().any(|(u, _)| *u == Unit::Pct)
    }

    /// Resolves the font, root and viewport relative units to px; percentages are kept as a percentage number.
    pub fn resolve(&self, ctx: &Ctx) -> Resolved {
        let mut px = 0.0;
        let mut pct = 0.0;
        for &(u, v) in &self.0 {
            px += match u {
                Unit::Px => v,
                Unit::Em => v * ctx.font_size,
                Unit::Rem => v * ctx.root_font_size,
                Unit::Vw => v * ctx.viewport_w / 100.0,
                Unit::Vh => v * ctx.viewport_h / 100.0,
                Unit::Vmin => v * ctx.viewport_w.min(ctx.viewport_h) / 100.0,
                Unit::Vmax => v * ctx.viewport_w.max(ctx.viewport_h) / 100.0,
                Unit::Pct => {
                    pct += v;
                    0.0
                }
            };
        }
        Resolved { px, pct, has_pct: self.has_pct() }
    }
}

/// The context of a resolution: `font_size` is the element's own font size (used by `em`), `root_font_size` the root
/// element's (used by `rem`), and the viewport size (CSS px) for the viewport units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ctx {
    pub font_size: f64,
    pub root_font_size: f64,
    pub viewport_w: f64,
    pub viewport_h: f64,
}

/// An absolute part and a percentage part (percentage number, so 50 for 50%).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resolved {
    pub px: f64,
    pub pct: f64,
    pub has_pct: bool,
}

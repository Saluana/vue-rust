//! 2D transform functions (CSS Transforms 1): translate, translateX/Y, scale, scaleX/Y, rotate, skew, skewX/Y and
//! matrix. The functions are normalised as Chrome's computed value does (checked with the CSS Typed OM): translate
//! always has two arguments, scale has two, scaleX(x) is scale(x, 1), scaleY(y) is scale(1, y), translateX(x) is
//! translate(x, 0px), translateY(y) is translate(0px, y), angles are degrees, and skew(x, 0) is skew(x). Lengths are
//! resolved at computed-value time (percentages kept). 3D functions and perspective() are unsupported (VNCSS5103).

use crate::calc::{self, Calc};
use crate::diag::code;
use crate::length::Lp;
use crate::tree::{nows, Cv};
use crate::value::ParseErr;
use vn_css::token::Kind;

#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Num(f64),
    Lp(Lp),
    /// An angle in degrees.
    Angle(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TfOp {
    pub name: &'static str,
    pub args: Vec<Arg>,
}

/// Parses a transform list (not `none`). Components separated by whitespace, each a known function.
pub fn parse(cvs: &[Cv]) -> Result<Vec<TfOp>, ParseErr> {
    let mut out = Vec::new();
    for c in nows(cvs) {
        match c {
            Cv::Func { name, args, .. } => out.push(normalise(function(&name.to_ascii_lowercase(), args)?)),
            _ => return Err(ParseErr::Invalid),
        }
    }
    if out.is_empty() { Err(ParseErr::Invalid) } else { Ok(out) }
}

/// The computed form of one function (see the module comment).
fn normalise(op: TfOp) -> TfOp {
    let zero = |a: &Arg| matches!(a, Arg::Angle(v) if *v == 0.0);
    let one = Arg::Num(1.0);
    let zero_len = Arg::Lp(Lp::px(0.0));
    let (name, args) = match (op.name, op.args.as_slice()) {
        ("translate", [x]) => ("translate", vec![x.clone(), zero_len]),
        ("translateX", [x]) => ("translate", vec![x.clone(), zero_len]),
        ("translateY", [y]) => ("translate", vec![zero_len, y.clone()]),
        ("scale", [x]) => ("scale", vec![x.clone(), x.clone()]),
        ("scaleX", [x]) => ("scale", vec![x.clone(), one]),
        ("scaleY", [y]) => ("scale", vec![one, y.clone()]),
        ("skew", [x, y]) if zero(y) => ("skew", vec![x.clone()]),
        (n, _) => (n, op.args.clone()),
    };
    TfOp { name, args }
}

fn function(name: &str, args: &[Cv]) -> Result<TfOp, ParseErr> {
    let parts: Vec<Vec<Cv>> = split(args);
    let n = parts.len();
    let (name_static, args): (&'static str, Vec<Arg>) = match name {
        "translate" if (1..=2).contains(&n) => ("translate", lps(&parts)?),
        "translatex" if n == 1 => ("translateX", lps(&parts)?),
        "translatey" if n == 1 => ("translateY", lps(&parts)?),
        "scale" if (1..=2).contains(&n) => ("scale", nums(&parts)?),
        "scalex" if n == 1 => ("scaleX", nums(&parts)?),
        "scaley" if n == 1 => ("scaleY", nums(&parts)?),
        "rotate" if n == 1 => ("rotate", angles(&parts)?),
        "skew" if (1..=2).contains(&n) => ("skew", angles(&parts)?),
        "skewx" if n == 1 => ("skewX", angles(&parts)?),
        "skewy" if n == 1 => ("skewY", angles(&parts)?),
        "matrix" if n == 6 => ("matrix", nums(&parts)?),
        "translate" | "translatex" | "translatey" | "scale" | "scalex" | "scaley" | "rotate" | "skew" | "skewx" | "skewy"
        | "matrix" => return Err(ParseErr::Invalid),
        _ => return Err(ParseErr::unsupported(code::UNSUPPORTED_FUNCTION, format!("{name}()"))),
    };
    Ok(TfOp { name: name_static, args })
}

fn split(args: &[Cv]) -> Vec<Vec<Cv>> {
    crate::tree::split_commas(args)
}

fn one(part: &[Cv]) -> Result<&Cv, ParseErr> {
    match nows(part).as_slice() {
        [c] => Ok(c),
        _ => Err(ParseErr::Invalid),
    }
}

fn lps(parts: &[Vec<Cv>]) -> Result<Vec<Arg>, ParseErr> {
    parts
        .iter()
        .map(|p| {
            let items = nows(p);
            // A calc() or a single length-percentage; unitless 0 is a zero length.
            if let [Cv::Tok(t)] = items.as_slice() {
                if let Kind::Number { value, .. } = t.kind {
                    return if value == 0.0 { Ok(Arg::Lp(Lp::px(0.0))) } else { Err(ParseErr::Invalid) };
                }
            }
            let c = one(p)?;
            match c {
                Cv::Tok(t) => match &t.kind {
                    Kind::Dimension { value, unit, .. } => match crate::length::Unit::from_name(unit) {
                        Some(u) => Ok(Arg::Lp(Lp::one(u, *value))),
                        None => Err(ParseErr::unsupported(code::UNSUPPORTED_VALUE, format!("unit {unit}"))),
                    },
                    Kind::Percentage(v) => Ok(Arg::Lp(Lp::one(crate::length::Unit::Pct, *v))),
                    _ => Err(ParseErr::Invalid),
                },
                Cv::Func { name, args, .. } if name.eq_ignore_ascii_case("calc") => match calc::eval(args)? {
                    Calc::Len(l) => Ok(Arg::Lp(l)),
                    Calc::Num(_) => Err(ParseErr::Invalid),
                },
                Cv::Func { name, .. } => Err(ParseErr::unsupported(code::UNSUPPORTED_FUNCTION, format!("{name}()"))),
                _ => Err(ParseErr::Invalid),
            }
        })
        .collect()
}

fn nums(parts: &[Vec<Cv>]) -> Result<Vec<Arg>, ParseErr> {
    parts
        .iter()
        .map(|p| match one(p)? {
            Cv::Tok(t) => match t.kind {
                Kind::Number { value, .. } => Ok(Arg::Num(value)),
                _ => Err(ParseErr::Invalid),
            },
            Cv::Func { name, args, .. } if name.eq_ignore_ascii_case("calc") => match calc::eval(args)? {
                Calc::Num(v) => Ok(Arg::Num(v)),
                Calc::Len(_) => Err(ParseErr::Invalid),
            },
            _ => Err(ParseErr::Invalid),
        })
        .collect()
}

/// Angles as degrees. Unitless zero is an angle too (Chrome accepts `rotate(0)`).
fn angles(parts: &[Vec<Cv>]) -> Result<Vec<Arg>, ParseErr> {
    parts
        .iter()
        .map(|p| match one(p)? {
            Cv::Tok(t) => match &t.kind {
                Kind::Dimension { value, unit, .. } => {
                    let deg = match unit.to_ascii_lowercase().as_str() {
                        "deg" => *value,
                        "grad" => value * 0.9,
                        "rad" => value * 180.0 / std::f64::consts::PI,
                        "turn" => value * 360.0,
                        _ => return Err(ParseErr::Invalid),
                    };
                    Ok(Arg::Angle(deg))
                }
                Kind::Number { value, .. } if *value == 0.0 => Ok(Arg::Angle(0.0)),
                _ => Err(ParseErr::Invalid),
            },
            _ => Err(ParseErr::Invalid),
        })
        .collect()
}

//! calc() over the documented units: + - * / with parentheses and nesting, numbers, percentages and the length
//! units. Other math functions are unsupported (VNCSS5103). Type rules as CSS Values 4: + and - need equal types,
//! * needs a number on one side, / needs a number divisor, and a zero divisor is invalid.

use crate::diag::code;
use crate::length::{Lp, Unit};
use crate::tree::Cv;
use crate::value::ParseErr;
use vn_css::token::Kind;

#[derive(Debug, Clone, PartialEq)]
pub enum Calc {
    Num(f64),
    Len(Lp),
}

/// One item of the expression: an operand (a component) or an operator with its whitespace on each side.
enum Item<'a> {
    Operand(&'a Cv),
    Op { c: char, spaced: bool },
}

/// Evaluates the arguments of a calc() function (components with whitespace).
pub fn eval(args: &[Cv]) -> Result<Calc, ParseErr> {
    let mut items = Vec::new();
    for (k, c) in args.iter().enumerate() {
        match c {
            Cv::Ws => {}
            Cv::Tok(t) if matches!(t.kind, Kind::Delim('+' | '-' | '*' | '/')) => {
                let Kind::Delim(ch) = t.kind else { unreachable!() };
                let before = k > 0 && matches!(args[k - 1], Cv::Ws);
                let after = matches!(args.get(k + 1), Some(Cv::Ws));
                items.push(Item::Op { c: ch, spaced: before && after });
            }
            _ => items.push(Item::Operand(c)),
        }
    }
    // Operators must be preceded by an operand and followed by one, and + and - need the whitespace on both sides.
    let mut p = P { items: &items, i: 0 };
    let v = p.sum()?;
    if p.i != items.len() {
        return Err(ParseErr::Invalid);
    }
    finite(v)
}

/// NaN is invalid; an infinity (from a division by zero) is a valid value that clamps at computed-value time.
fn finite(v: Calc) -> Result<Calc, ParseErr> {
    let ok = match &v {
        Calc::Num(n) => !n.is_nan(),
        Calc::Len(l) => l.0.iter().all(|(_, x)| !x.is_nan()),
    };
    if ok { Ok(v) } else { Err(ParseErr::Invalid) }
}

struct P<'a, 'b> {
    items: &'b [Item<'a>],
    i: usize,
}

impl P<'_, '_> {
    fn sum(&mut self) -> Result<Calc, ParseErr> {
        let mut v = self.product()?;
        while let Some(Item::Op { c, spaced }) = self.items.get(self.i) {
            if !(*c == '+' || *c == '-') {
                break;
            }
            if !spaced {
                return Err(ParseErr::Invalid);
            }
            let k = if *c == '+' { 1.0 } else { -1.0 };
            self.i += 1;
            let rhs = self.product()?;
            v = match (v, rhs) {
                (Calc::Num(a), Calc::Num(b)) => Calc::Num(a + k * b),
                (Calc::Len(a), Calc::Len(b)) => Calc::Len(a.add_scaled(&b, k)),
                _ => return Err(ParseErr::Invalid),
            };
        }
        Ok(v)
    }

    fn product(&mut self) -> Result<Calc, ParseErr> {
        let mut v = self.value()?;
        while let Some(Item::Op { c, .. }) = self.items.get(self.i) {
            if !(*c == '*' || *c == '/') {
                break;
            }
            let op = *c;
            self.i += 1;
            let rhs = self.value()?;
            v = match (op, v, rhs) {
                ('*', Calc::Num(a), Calc::Num(b)) => Calc::Num(a * b),
                ('*', Calc::Num(k), Calc::Len(l)) | ('*', Calc::Len(l), Calc::Num(k)) => Calc::Len(l.scale(k)),
                ('/', Calc::Num(a), Calc::Num(b)) => Calc::Num(a / b),
                ('/', Calc::Len(l), Calc::Num(k)) => Calc::Len(l.scale(1.0 / k)),
                _ => return Err(ParseErr::Invalid),
            };
            v = finite(v)?;
        }
        Ok(v)
    }

    fn value(&mut self) -> Result<Calc, ParseErr> {
        let Some(item) = self.items.get(self.i) else {
            return Err(ParseErr::Invalid);
        };
        self.i += 1;
        let c = match item {
            Item::Op { .. } => return Err(ParseErr::Invalid),
            Item::Operand(c) => *c,
        };
        match c {
            Cv::Tok(t) => match &t.kind {
                Kind::Number { value, .. } => Ok(Calc::Num(*value)),
                Kind::Percentage(v) => Ok(Calc::Len(Lp::one(Unit::Pct, *v))),
                Kind::Dimension { value, unit, .. } => match Unit::from_name(unit) {
                    Some(u) => Ok(Calc::Len(Lp::one(u, *value))),
                    None => Err(ParseErr::unsupported(code::UNSUPPORTED_VALUE, format!("unit {unit}"))),
                },
                _ => Err(ParseErr::Invalid),
            },
            Cv::Paren(args) => eval(args),
            Cv::Func { name, args, .. } => {
                if name.eq_ignore_ascii_case("calc") {
                    eval(args)
                } else {
                    Err(ParseErr::unsupported(code::UNSUPPORTED_FUNCTION, format!("{name}()")))
                }
            }
            Cv::Square(_) | Cv::Ws => Err(ParseErr::Invalid),
        }
    }
}

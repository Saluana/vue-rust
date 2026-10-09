//! Colours: the 148 CSS named colours, transparent, #rgb/#rgba/#rrggbb/#rrggbbaa, rgb()/rgba() and hsl()/hsla() in the
//! legacy comma and the space-with-slash syntaxes. `currentcolor` is handled by the caller. Chrome stores a colour as
//! four bytes (blink::Color is RGBA32), so the alpha is quantised to a byte here as well.

use crate::diag::code;
use crate::num::{alpha_text, round_clamped};
use crate::tree::{ident, nows, Cv};
use crate::value::ParseErr;
use vn_css::token::Kind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
    /// A modern-syntax alpha below 1 that quantises to 255: Chrome keeps it translucent (rgba(..., 1)), while a legacy
    /// alpha of the same byte is opaque (rgb()).
    pub translucent: bool,
}

impl Color {
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0, a: 255, translucent: false };
    pub const TRANSPARENT: Color = Color { r: 0, g: 0, b: 0, a: 0, translucent: false };

    pub fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b, a: 255, translucent: false }
    }

    /// The Chrome serialisation: `rgb(r, g, b)` when opaque, otherwise `rgba(r, g, b, alpha)`.
    pub fn serialize(self) -> String {
        if self.a == 255 && !self.translucent {
            format!("rgb({}, {}, {})", self.r, self.g, self.b)
        } else {
            format!("rgba({}, {}, {}, {})", self.r, self.g, self.b, alpha_text(self.a))
        }
    }
}

/// Parses one colour (not `currentcolor`) from the components of a declaration value.
pub fn parse(cvs: &[Cv]) -> Result<Color, ParseErr> {
    let v = nows(cvs);
    let [c] = v.as_slice() else {
        return Err(ParseErr::Invalid);
    };
    match c {
        Cv::Tok(t) => match &t.kind {
            Kind::Ident(name) => {
                let lower = name.to_ascii_lowercase();
                if lower == "transparent" {
                    return Ok(Color::TRANSPARENT);
                }
                match named(&lower) {
                    Some(rgb) => Ok(Color::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)),
                    None => Err(ParseErr::unsupported(code::UNSUPPORTED_VALUE, format!("color {name}"))),
                }
            }
            Kind::Hash { value, .. } => hex(value).ok_or(ParseErr::Invalid),
            _ => Err(ParseErr::Invalid),
        },
        Cv::Func { name, args, .. } => function(&name.to_ascii_lowercase(), args),
        _ => Err(ParseErr::Invalid),
    }
}

/// #rgb, #rgba, #rrggbb, #rrggbbaa (the hash token text without the `#`).
fn hex(s: &str) -> Option<Color> {
    if !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let d: Vec<u8> = s.bytes().map(|b| (b as char).to_digit(16).unwrap_or(0) as u8).collect();
    let (r, g, b, a) = match d.len() {
        3 => (d[0] * 17, d[1] * 17, d[2] * 17, 255),
        4 => (d[0] * 17, d[1] * 17, d[2] * 17, d[3] * 17),
        6 => (d[0] << 4 | d[1], d[2] << 4 | d[3], d[4] << 4 | d[5], 255),
        8 => (d[0] << 4 | d[1], d[2] << 4 | d[3], d[4] << 4 | d[5], d[6] << 4 | d[7]),
        _ => return None,
    };
    Some(Color { r, g, b, a, translucent: false })
}

fn function(name: &str, args: &[Cv]) -> Result<Color, ParseErr> {
    match name {
        "rgb" | "rgba" => rgb_function(args),
        "hsl" | "hsla" => hsl_function(args),
        "hwb" | "lab" | "lch" | "oklab" | "oklch" | "color" | "color-mix" => {
            Err(ParseErr::unsupported(code::UNSUPPORTED_FUNCTION, format!("{name}()")))
        }
        _ => Err(ParseErr::Invalid),
    }
}

/// The components of a colour function: three channels, an optional alpha, and whether the legacy comma syntax was used.
struct Args<'a> {
    ch: [&'a Cv; 3],
    alpha: Option<&'a Cv>,
    legacy: bool,
}

fn is_comma(c: &Cv) -> bool {
    matches!(c, Cv::Tok(t) if matches!(t.kind, Kind::Comma))
}

fn is_slash(c: &Cv) -> bool {
    matches!(c, Cv::Tok(t) if matches!(t.kind, Kind::Delim('/')))
}

/// Legacy syntax separates with commas (three or four parts, no slash); modern syntax separates with whitespace and
/// one slash before the alpha.
fn split_args(args: &[Cv]) -> Result<Args<'_>, ParseErr> {
    let items = nows(args);
    let commas = items.iter().filter(|c| is_comma(c)).count();
    let slashes = items.iter().filter(|c| is_slash(c)).count();
    if commas > 0 {
        let parts: Vec<&Cv> = items.into_iter().filter(|c| !is_comma(c)).collect();
        if slashes > 0 || !(commas == 2 || commas == 3) || parts.len() != commas + 1 {
            return Err(ParseErr::Invalid);
        }
        return Ok(Args { ch: [parts[0], parts[1], parts[2]], alpha: parts.get(3).copied(), legacy: true });
    }
    match slashes {
        0 if items.len() == 3 => Ok(Args { ch: [items[0], items[1], items[2]], alpha: None, legacy: false }),
        1 if items.len() == 5 && is_slash(items[3]) => Ok(Args { ch: [items[0], items[1], items[2]], alpha: Some(items[4]), legacy: false }),
        _ => Err(ParseErr::Invalid),
    }
}

/// `none` (modern syntax only) is zero.
fn is_none(c: &Cv) -> bool {
    ident(c).is_some_and(|s| s.eq_ignore_ascii_case("none"))
}

/// An rgb() channel: a number (0..255) or a percentage of 255, clamped and rounded by the caller.
fn rgb_channel(c: &Cv, legacy: bool) -> Result<(f64, bool), ParseErr> {
    match c {
        Cv::Tok(t) => match t.kind {
            Kind::Number { value, .. } => Ok((value, false)),
            // Exact arithmetic (p * 255 / 100): 50% is 127.5 and rounds up, as Chrome rounds it.
            Kind::Percentage(p) => Ok((p * 255.0 / 100.0, true)),
            _ if !legacy && is_none(c) => Ok((0.0, false)),
            _ => Err(ParseErr::Invalid),
        },
        _ => Err(ParseErr::Invalid),
    }
}

/// An hsl() saturation or lightness as a fraction. Chrome clamps a negative saturation to zero and leaves the rest
/// unclamped; a bare number means a percentage in modern syntax only.
fn hsl_fraction(c: &Cv, legacy: bool) -> Result<f64, ParseErr> {
    match c {
        Cv::Tok(t) => match t.kind {
            Kind::Percentage(p) => Ok(p / 100.0),
            Kind::Number { value, .. } if !legacy => Ok(value / 100.0),
            _ if !legacy && is_none(c) => Ok(0.0),
            _ => Err(ParseErr::Invalid),
        },
        _ => Err(ParseErr::Invalid),
    }
}

/// An alpha: a number or a percentage (`none` is zero in modern syntax), clamped to 0..1 and quantised to a byte
/// (round half up). `float_form` says the colour keeps its alpha as a float: then an alpha below one that quantises
/// to 255 stays translucent (see `Color::translucent`).
fn alpha(c: &Cv, float_form: bool, legacy: bool) -> Result<(u8, bool), ParseErr> {
    let v = match c {
        Cv::Tok(t) => match t.kind {
            Kind::Number { value, .. } => value,
            Kind::Percentage(p) => p / 100.0,
            _ if !legacy && is_none(c) => 0.0,
            _ => return Err(ParseErr::Invalid),
        },
        _ => return Err(ParseErr::Invalid),
    };
    let clamped = v.clamp(0.0, 1.0);
    let byte = round_clamped(clamped * 255.0, 0.0, 255.0) as u8;
    Ok((byte, float_form && clamped < 1.0 && byte == 255))
}

fn rgb_function(args: &[Cv]) -> Result<Color, ParseErr> {
    let parts = split_args(args)?;
    let mut ch = [(0.0, false); 3];
    for (k, c) in parts.ch.iter().enumerate() {
        ch[k] = rgb_channel(c, parts.legacy)?;
    }
    // Legacy syntax needs three numbers or three percentages.
    if parts.legacy && (ch[0].1 != ch[1].1 || ch[1].1 != ch[2].1) {
        return Err(ParseErr::Invalid);
    }
    let (a, translucent) = match parts.alpha {
        Some(c) => alpha(c, !parts.legacy, parts.legacy)?,
        None => (255, false),
    };
    let q = |v: f64| round_clamped(v, 0.0, 255.0) as u8;
    Ok(Color { r: q(ch[0].0), g: q(ch[1].0), b: q(ch[2].0), a, translucent })
}

fn hsl_function(args: &[Cv]) -> Result<Color, ParseErr> {
    let parts = split_args(args)?;
    let hue = match parts.ch[0] {
        Cv::Tok(t) => match &t.kind {
            Kind::Number { value, .. } => *value,
            Kind::Dimension { value, unit, .. } => match unit.to_ascii_lowercase().as_str() {
                "deg" => *value,
                "grad" => value * 0.9,
                "rad" => value * 180.0 / std::f64::consts::PI,
                "turn" => value * 360.0,
                _ => return Err(ParseErr::Invalid),
            },
            _ if !parts.legacy && is_none(parts.ch[0]) => 0.0,
            _ => return Err(ParseErr::Invalid),
        },
        _ => return Err(ParseErr::Invalid),
    };
    let s = hsl_fraction(parts.ch[1], parts.legacy)?.max(0.0);
    let l = hsl_fraction(parts.ch[2], parts.legacy)?;
    let (r, g, b) = hsl_to_rgb(hue, s, l);
    let (a, translucent) = match parts.alpha {
        Some(c) => alpha(c, true, parts.legacy)?,
        None => (255, false),
    };
    let q = |v: f64| round_clamped(v * 255.0, 0.0, 255.0) as u8;
    Ok(Color { r: q(r), g: q(g), b: q(b), a, translucent })
}

/// hsl to rgb in 0..1, as Chrome computes it: the lightness and saturation are clamped from below only, the HSL
/// value is taken as an HSV value clamped to 1 (with its saturation recomputed), and the HSV conversion runs
/// unclamped, so the channels may leave 0..1 (the caller clamps them). Checked against Chrome 155 for saturations and
/// lightnesses above 100%.
fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    let s = s.max(0.0);
    let l = l.max(0.0);
    let v = (l + s * l.min(1.0 - l)).min(1.0);
    let sv = if v == 0.0 { 0.0 } else { 2.0 * (1.0 - l / v) };
    let h6 = h.rem_euclid(360.0) / 60.0;
    let i = h6.floor();
    let f = h6 - i;
    let p = v * (1.0 - sv);
    let q = v * (1.0 - sv * f);
    let t = v * (1.0 - sv * (1.0 - f));
    match i as u32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

/// The CSS named colours (148) as 0xRRGGBB, lower-case names.
pub fn named(name: &str) -> Option<u32> {
    NAMED.binary_search_by(|(n, _)| n.cmp(&name)).ok().map(|i| NAMED[i].1)
}

pub const NAMED_COUNT: usize = 148;

static NAMED: &[(&str, u32)] = &[
    ("aliceblue", 0xf0f8ff), ("antiquewhite", 0xfaebd7), ("aqua", 0x00ffff), ("aquamarine", 0x7fffd4), ("azure", 0xf0ffff),
    ("beige", 0xf5f5dc), ("bisque", 0xffe4c4), ("black", 0x000000), ("blanchedalmond", 0xffebcd), ("blue", 0x0000ff),
    ("blueviolet", 0x8a2be2), ("brown", 0xa52a2a), ("burlywood", 0xdeb887), ("cadetblue", 0x5f9ea0), ("chartreuse", 0x7fff00),
    ("chocolate", 0xd2691e), ("coral", 0xff7f50), ("cornflowerblue", 0x6495ed), ("cornsilk", 0xfff8dc), ("crimson", 0xdc143c),
    ("cyan", 0x00ffff), ("darkblue", 0x00008b), ("darkcyan", 0x008b8b), ("darkgoldenrod", 0xb8860b), ("darkgray", 0xa9a9a9),
    ("darkgreen", 0x006400), ("darkgrey", 0xa9a9a9), ("darkkhaki", 0xbdb76b), ("darkmagenta", 0x8b008b), ("darkolivegreen", 0x556b2f),
    ("darkorange", 0xff8c00), ("darkorchid", 0x9932cc), ("darkred", 0x8b0000), ("darksalmon", 0xe9967a), ("darkseagreen", 0x8fbc8f),
    ("darkslateblue", 0x483d8b), ("darkslategray", 0x2f4f4f), ("darkslategrey", 0x2f4f4f), ("darkturquoise", 0x00ced1), ("darkviolet", 0x9400d3),
    ("deeppink", 0xff1493), ("deepskyblue", 0x00bfff), ("dimgray", 0x696969), ("dimgrey", 0x696969), ("dodgerblue", 0x1e90ff),
    ("firebrick", 0xb22222), ("floralwhite", 0xfffaf0), ("forestgreen", 0x228b22), ("fuchsia", 0xff00ff), ("gainsboro", 0xdcdcdc),
    ("ghostwhite", 0xf8f8ff), ("gold", 0xffd700), ("goldenrod", 0xdaa520), ("gray", 0x808080), ("green", 0x008000),
    ("greenyellow", 0xadff2f), ("grey", 0x808080), ("honeydew", 0xf0fff0), ("hotpink", 0xff69b4), ("indianred", 0xcd5c5c),
    ("indigo", 0x4b0082), ("ivory", 0xfffff0), ("khaki", 0xf0e68c), ("lavender", 0xe6e6fa), ("lavenderblush", 0xfff0f5),
    ("lawngreen", 0x7cfc00), ("lemonchiffon", 0xfffacd), ("lightblue", 0xadd8e6), ("lightcoral", 0xf08080), ("lightcyan", 0xe0ffff),
    ("lightgoldenrodyellow", 0xfafad2), ("lightgray", 0xd3d3d3), ("lightgreen", 0x90ee90), ("lightgrey", 0xd3d3d3), ("lightpink", 0xffb6c1),
    ("lightsalmon", 0xffa07a), ("lightseagreen", 0x20b2aa), ("lightskyblue", 0x87cefa), ("lightslategray", 0x778899), ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xb0c4de), ("lightyellow", 0xffffe0), ("lime", 0x00ff00), ("limegreen", 0x32cd32), ("linen", 0xfaf0e6),
    ("magenta", 0xff00ff), ("maroon", 0x800000), ("mediumaquamarine", 0x66cdaa), ("mediumblue", 0x0000cd), ("mediumorchid", 0xba55d3),
    ("mediumpurple", 0x9370db), ("mediumseagreen", 0x3cb371), ("mediumslateblue", 0x7b68ee), ("mediumspringgreen", 0x00fa9a), ("mediumturquoise", 0x48d1cc),
    ("mediumvioletred", 0xc71585), ("midnightblue", 0x191970), ("mintcream", 0xf5fffa), ("mistyrose", 0xffe4e1), ("moccasin", 0xffe4b5),
    ("navajowhite", 0xffdead), ("navy", 0x000080), ("oldlace", 0xfdf5e6), ("olive", 0x808000), ("olivedrab", 0x6b8e23),
    ("orange", 0xffa500), ("orangered", 0xff4500), ("orchid", 0xda70d6), ("palegoldenrod", 0xeee8aa), ("palegreen", 0x98fb98),
    ("paleturquoise", 0xafeeee), ("palevioletred", 0xdb7093), ("papayawhip", 0xffefd5), ("peachpuff", 0xffdab9), ("peru", 0xcd853f),
    ("pink", 0xffc0cb), ("plum", 0xdda0dd), ("powderblue", 0xb0e0e6), ("purple", 0x800080), ("rebeccapurple", 0x663399),
    ("red", 0xff0000), ("rosybrown", 0xbc8f8f), ("royalblue", 0x4169e1), ("saddlebrown", 0x8b4513), ("salmon", 0xfa8072),
    ("sandybrown", 0xf4a460), ("seagreen", 0x2e8b57), ("seashell", 0xfff5ee), ("sienna", 0xa0522d), ("silver", 0xc0c0c0),
    ("skyblue", 0x87ceeb), ("slateblue", 0x6a5acd), ("slategray", 0x708090), ("slategrey", 0x708090), ("snow", 0xfffafa),
    ("springgreen", 0x00ff7f), ("steelblue", 0x4682b4), ("tan", 0xd2b48c), ("teal", 0x008080), ("thistle", 0xd8bfd8),
    ("tomato", 0xff6347), ("turquoise", 0x40e0d0), ("violet", 0xee82ee), ("wheat", 0xf5deb3), ("white", 0xffffff),
    ("whitesmoke", 0xf5f5f5), ("yellow", 0xffff00), ("yellowgreen", 0x9acd32),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_table_is_sorted_and_complete() {
        assert_eq!(NAMED.len(), NAMED_COUNT);
        assert!(NAMED.windows(2).all(|w| w[0].0 < w[1].0), "table must be sorted for binary search");
        assert_eq!(named("rebeccapurple"), Some(0x663399));
    }
}

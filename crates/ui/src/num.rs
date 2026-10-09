//! Number text as Chrome serialises it: C `%g` with six significant digits ("0.123457", "1e-07", "1e+06"), and the
//! alpha of a colour, which Chrome stores as one byte (blink::Color is RGBA32): the shortest text of at most three
//! decimals that quantises back to that byte (checked for all 256 values against Chrome 155).

/// `%g` with precision 6.
pub fn fmt(x: f64) -> String {
    if !x.is_finite() {
        return format!("{x}");
    }
    if x == 0.0 {
        return "0".to_string();
    }
    let a = x.abs();
    let sci = format!("{a:.5e}");
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let body = if (-4..6).contains(&exp) {
        trim_fraction(&format!("{a:.*}", (5 - exp) as usize))
    } else {
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", trim_fraction(mantissa), exp.abs())
    };
    if x < 0.0 { format!("-{body}") } else { body }
}

fn trim_fraction(s: &str) -> String {
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s.to_string() }
}

/// Alpha text for an 8-bit alpha below 255 (255 is opaque and is serialised as rgb()).
pub fn alpha_text(a: u8) -> String {
    if a == 255 {
        return "1".to_string();
    }
    for d in 0..=3usize {
        let s = format!("{:.*}", d, f64::from(a) / 255.0);
        if let Ok(v) = s.parse::<f64>() {
            if (v * 255.0).round() as u32 == u32::from(a) {
                return s;
            }
        }
    }
    fmt(f64::from(a) / 255.0)
}

/// Rounds half up (Chrome's rounding of colour components) after clamping to `lo..=hi`.
pub fn round_clamped(x: f64, lo: f64, hi: f64) -> f64 {
    (x.clamp(lo, hi) + 0.5).floor()
}

//! ECMAScript number semantics: Number::toString (radix 10 and radix 2..36), toFixed, toExponential,
//! toPrecision, StringToNumber, parseFloat, parseInt, the ToInt32 family, `%`, `**`, Math.round,
//! Math.min/max, SameValue. Expected results are V8's (tests/language/corpora/number.json).
//! The radix-fraction algorithm is V8's DoubleToRadixCString and the parseInt loop for non-power-of-two
//! radixes is V8's NumberParseIntHelper::HandleGenericCase; both are reimplemented here.

use crate::jsstr::{trim_units, JsStr};

/// A JavaScript RangeError raised by a number conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumErr {
    Range,
}

impl NumErr {
    pub fn name(self) -> &'static str {
        match self {
            NumErr::Range => "RangeError",
        }
    }
}

const TWO53: f64 = 9_007_199_254_740_992.0;

/// Shortest decimal digits of a value: value = 0.d1d2...dk * 10^n (ECMA-262 `n`). ASCII digits, no
/// trailing zeros, at least one digit.
struct Dec {
    digits: Vec<u8>,
    n: i32,
}

/// Parses Rust's `{:e}` / `{:.Ne}` output ("d.ddde[-]N") of a positive finite value.
fn parse_sci(s: &str) -> Dec {
    let b = s.as_bytes();
    let e = b.iter().position(|&c| c == b'e').unwrap_or(b.len());
    let mut digits: Vec<u8> = b[..e].iter().copied().filter(|&c| c != b'.').collect();
    let mut exp: i32 = 0;
    let mut neg = false;
    for &c in &b[(e + 1).min(b.len())..] {
        match c {
            b'-' => neg = true,
            b'0'..=b'9' => exp = exp * 10 + (c - b'0') as i32,
            _ => {}
        }
    }
    if neg {
        exp = -exp;
    }
    while digits.len() > 1 && digits.last() == Some(&b'0') {
        digits.pop();
    }
    if digits.is_empty() {
        digits.push(b'0');
    }
    Dec { digits, n: exp + 1 }
}

/// Shortest round-trip digits. Rust's `{:e}` is the shortest representation, closest to the value.
fn shortest(x: f64) -> Dec {
    parse_sci(&format!("{x:e}"))
}

/// Exact decimal expansion. A double has at most 767 significant digits, so 1100 places are exact.
fn exact(x: f64) -> Dec {
    parse_sci(&format!("{x:.1100e}"))
}

fn push_digits(out: &mut String, d: &[u8]) {
    for &c in d {
        out.push(char::from(c));
    }
}

fn push_zeros(out: &mut String, n: i32) {
    for _ in 0..n.max(0) {
        out.push('0');
    }
}

/// `d[0].d[1..]e±E` with E = e (the exponent form used by toString, toExponential and toPrecision).
fn push_exp_form(out: &mut String, d: &[u8], e: i32) {
    push_digits(out, &d[..1]);
    if d.len() > 1 {
        out.push('.');
        push_digits(out, &d[1..]);
    }
    out.push('e');
    out.push(if e < 0 { '-' } else { '+' });
    out.push_str(&e.unsigned_abs().to_string());
}

/// Adds one to a decimal digit string (possibly growing it).
fn increment(digits: &mut Vec<u8>) {
    for c in digits.iter_mut().rev() {
        if *c == b'9' {
            *c = b'0';
        } else {
            *c += 1;
            return;
        }
    }
    digits.insert(0, b'1');
}

/// round_half_up(0.D * 10^m) as decimal digits without leading zeros ("" is zero), where D are the
/// digits of `d`. Ties round up: ECMA-262 picks the larger n when two candidates are equally close.
fn round_scaled(d: &Dec, m: i32) -> Vec<u8> {
    if m < 0 {
        return Vec::new();
    }
    let m = m as usize;
    let mut r: Vec<u8> = (0..m).map(|i| d.digits.get(i).copied().unwrap_or(b'0')).collect();
    if d.digits.get(m).is_some_and(|&c| c >= b'5') {
        increment(&mut r);
    }
    let lead = r.iter().take_while(|&&c| c == b'0').count();
    r.drain(..lead);
    r
}

/// Exactly `p` significant digits of `x` (finite, >= 0) rounded half up, and the exponent e with
/// x ~ d.ddd * 10^e.
fn sig_digits(x: f64, p: usize) -> (Vec<u8>, i32) {
    if x == 0.0 {
        return (vec![b'0'; p], 0);
    }
    let d = exact(x);
    let mut r = round_scaled(&d, p as i32);
    let mut e = d.n - 1;
    if r.len() > p {
        // The rounding carried into a new leading digit (for example 9.99 -> 10.0).
        r.truncate(p);
        e += 1;
    }
    (r, e)
}

/// ToIntegerOrInfinity on a number.
pub fn to_integer_or_infinity(v: f64) -> f64 {
    if v.is_nan() { 0.0 } else { v.trunc() }
}

/// Number::toString(x, 10) (ECMA-262 6.1.6.1.20).
pub fn number_to_string(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_string();
    }
    if x == 0.0 {
        return "0".to_string();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-Infinity" } else { "Infinity" }.to_string();
    }
    let mut out = String::new();
    if x < 0.0 {
        out.push('-');
    }
    let d = shortest(x.abs());
    let k = d.digits.len() as i32;
    let n = d.n;
    if k <= n && n <= 21 {
        push_digits(&mut out, &d.digits);
        push_zeros(&mut out, n - k);
    } else if 0 < n && n <= 21 {
        let split = n as usize;
        push_digits(&mut out, &d.digits[..split]);
        out.push('.');
        push_digits(&mut out, &d.digits[split..]);
    } else if -6 < n && n <= 0 {
        out.push_str("0.");
        push_zeros(&mut out, -n);
        push_digits(&mut out, &d.digits);
    } else {
        push_exp_form(&mut out, &d.digits, n - 1);
    }
    out
}

/// Number.prototype.toFixed after argument checks: `digits` is ToIntegerOrInfinity already applied.
/// Throws RangeError for digits > 100; non-finite values and |x| >= 1e21 print as ToString.
pub fn to_fixed(x: f64, digits: u32) -> Result<String, NumErr> {
    if digits > 100 {
        return Err(NumErr::Range);
    }
    if !x.is_finite() || x.abs() >= 1e21 {
        return Ok(number_to_string(x));
    }
    let mut out = String::new();
    if x < 0.0 {
        out.push('-');
    }
    let f = digits as usize;
    let mut int = if x == 0.0 {
        Vec::new()
    } else {
        let d = exact(x.abs());
        round_scaled(&d, d.n + digits as i32)
    };
    if int.is_empty() {
        int.push(b'0');
    }
    if f > 0 {
        while int.len() <= f {
            int.insert(0, b'0');
        }
        let point = int.len() - f;
        int.insert(point, b'.');
    }
    push_digits(&mut out, &int);
    Ok(out)
}

/// Number.prototype.toFixed with the raw JavaScript argument (any number, including NaN and
/// Infinity). Performs ToIntegerOrInfinity and the RangeError checks of ECMA-262 21.1.3.3.
pub fn to_fixed_js(x: f64, fraction_digits: f64) -> Result<String, NumErr> {
    let f = to_integer_or_infinity(fraction_digits);
    if !(0.0..=100.0).contains(&f) {
        return Err(NumErr::Range);
    }
    to_fixed(x, f as u32)
}

/// Number.prototype.toExponential. `None` is the undefined argument (shortest digits).
pub fn to_exponential(x: f64, digits: Option<u32>) -> Result<String, NumErr> {
    if !x.is_finite() {
        return Ok(number_to_string(x));
    }
    let (ds, e) = match digits {
        None => {
            if x == 0.0 {
                (vec![b'0'], 0)
            } else {
                let d = shortest(x.abs());
                (d.digits, d.n - 1)
            }
        }
        Some(f) if f > 100 => return Err(NumErr::Range),
        Some(f) => sig_digits(x.abs(), f as usize + 1),
    };
    let mut out = String::new();
    if x < 0.0 {
        out.push('-');
    }
    push_exp_form(&mut out, &ds, e);
    Ok(out)
}

/// Number.prototype.toExponential with the raw JavaScript argument.
pub fn to_exponential_js(x: f64, fraction_digits: Option<f64>) -> Result<String, NumErr> {
    match fraction_digits {
        None => to_exponential(x, None),
        Some(v) => {
            let f = to_integer_or_infinity(v);
            if !x.is_finite() {
                return Ok(number_to_string(x));
            }
            if !(0.0..=100.0).contains(&f) {
                return Err(NumErr::Range);
            }
            to_exponential(x, Some(f as u32))
        }
    }
}

/// Number.prototype.toPrecision. `None` is the undefined argument (ToString).
pub fn to_precision(x: f64, precision: Option<u32>) -> Result<String, NumErr> {
    let Some(p) = precision else {
        return Ok(number_to_string(x));
    };
    if !x.is_finite() {
        return Ok(number_to_string(x));
    }
    if !(1..=100).contains(&p) {
        return Err(NumErr::Range);
    }
    let p = p as usize;
    let (d, e) = sig_digits(x.abs(), p);
    let mut out = String::new();
    if x < 0.0 {
        out.push('-');
    }
    let pi = p as i32;
    if e < -6 || e >= pi {
        push_exp_form(&mut out, &d, e);
    } else if e == pi - 1 {
        push_digits(&mut out, &d);
    } else if e >= 0 {
        let k = (e + 1) as usize;
        push_digits(&mut out, &d[..k]);
        out.push('.');
        push_digits(&mut out, &d[k..]);
    } else {
        out.push_str("0.");
        push_zeros(&mut out, -(e + 1));
        push_digits(&mut out, &d);
    }
    Ok(out)
}

/// Number.prototype.toPrecision with the raw JavaScript argument.
pub fn to_precision_js(x: f64, precision: Option<f64>) -> Result<String, NumErr> {
    let Some(v) = precision else {
        return Ok(number_to_string(x));
    };
    let p = to_integer_or_infinity(v);
    if !x.is_finite() {
        return Ok(number_to_string(x));
    }
    if !(1.0..=100.0).contains(&p) {
        return Err(NumErr::Range);
    }
    to_precision(x, Some(p as u32))
}

/// Number::toString(x, radix) for 2..=36 (V8 DoubleToRadixCString). Callers must pass a radix in
/// 2..=36; `number_to_string_radix_js` performs the JavaScript check.
pub fn number_to_string_radix(x: f64, radix: u32) -> String {
    assert!((2..=36).contains(&radix), "radix must be 2..=36");
    if x.is_nan() {
        return "NaN".to_string();
    }
    if x == 0.0 {
        return "0".to_string();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-Infinity" } else { "Infinity" }.to_string();
    }
    const CHARS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let r = radix as f64;
    let value = x.abs();
    let mut integer = value.floor();
    let mut fraction = value - integer;
    // Fraction digits are produced only down to the precision of the input double.
    let next_up = |v: f64| f64::from_bits(v.to_bits() + 1);
    let mut delta = (0.5 * (next_up(value) - value)).max(next_up(0.0));
    let mut frac: Vec<u32> = Vec::new();
    if fraction >= delta {
        loop {
            fraction *= r;
            delta *= r;
            let digit = fraction as u32;
            frac.push(digit);
            fraction -= digit as f64;
            if (fraction > 0.5 || (fraction == 0.5 && digit & 1 == 1)) && fraction + delta > 1.0 {
                // Round up and carry through the digits already written.
                loop {
                    match frac.pop() {
                        None => {
                            integer += 1.0;
                            break;
                        }
                        Some(d) => {
                            if d + 1 < radix {
                                frac.push(d + 1);
                                break;
                            }
                        }
                    }
                }
                break;
            }
            if fraction < delta {
                break;
            }
        }
    }
    // Integer digits, least significant first. Digits below double precision are written as zeros.
    let mut int_digits: Vec<u8> = Vec::new();
    while integer / r >= TWO53 {
        integer /= r;
        int_digits.push(b'0');
    }
    loop {
        let rem = integer % r;
        int_digits.push(CHARS[rem as usize]);
        integer = (integer - rem) / r;
        if integer <= 0.0 {
            break;
        }
    }
    let mut out = String::new();
    if x < 0.0 {
        out.push('-');
    }
    for &c in int_digits.iter().rev() {
        out.push(char::from(c));
    }
    if !frac.is_empty() {
        out.push('.');
        for &d in &frac {
            out.push(char::from(CHARS[d as usize]));
        }
    }
    out
}

/// Number.prototype.toString(radix) with the raw JavaScript argument.
pub fn number_to_string_radix_js(x: f64, radix: f64) -> Result<String, NumErr> {
    let r = to_integer_or_infinity(radix);
    if !(2.0..=36.0).contains(&r) {
        return Err(NumErr::Range);
    }
    if r == 10.0 {
        return Ok(number_to_string(x));
    }
    Ok(number_to_string_radix(x, r as u32))
}

fn is_dec(c: u16) -> bool {
    (0x30..=0x39).contains(&c)
}

fn is_unit(c: u16, ascii: u8) -> bool {
    c == ascii as u16
}

fn ascii_string(u: &[u16]) -> String {
    u.iter().map(|&c| char::from(c as u8)).collect()
}

fn starts_with_ascii(u: &[u16], pat: &str) -> bool {
    u.len() >= pat.len() && pat.bytes().zip(u).all(|(p, &c)| is_unit(c, p))
}

/// Longest StrDecimalLiteral (optional sign, digits with optional fraction and exponent, or Infinity)
/// at the start of `u`. Returns its value and the number of code units it spans.
fn scan_decimal(u: &[u16]) -> Option<(f64, usize)> {
    let mut i = 0;
    let mut neg = false;
    if let Some(&c) = u.first() {
        if is_unit(c, b'+') || is_unit(c, b'-') {
            neg = is_unit(c, b'-');
            i = 1;
        }
    }
    if starts_with_ascii(&u[i..], "Infinity") {
        let v = if neg { f64::NEG_INFINITY } else { f64::INFINITY };
        return Some((v, i + 8));
    }
    let int_start = i;
    while i < u.len() && is_dec(u[i]) {
        i += 1;
    }
    let int_part = &u[int_start..i];
    let mut frac_part: &[u16] = &[];
    if i < u.len() && is_unit(u[i], b'.') {
        let fs = i + 1;
        let mut j = fs;
        while j < u.len() && is_dec(u[j]) {
            j += 1;
        }
        frac_part = &u[fs..j];
        if int_part.is_empty() && frac_part.is_empty() {
            return None;
        }
        i = j;
    } else if int_part.is_empty() {
        return None;
    }
    let mut exp_text = String::new();
    if i < u.len() && (is_unit(u[i], b'e') || is_unit(u[i], b'E')) {
        let mut j = i + 1;
        let mut sign = "";
        if j < u.len() && (is_unit(u[j], b'+') || is_unit(u[j], b'-')) {
            sign = if is_unit(u[j], b'-') { "-" } else { "+" };
            j += 1;
        }
        let ds = j;
        while j < u.len() && is_dec(u[j]) {
            j += 1;
        }
        if j > ds {
            exp_text = format!("e{sign}{}", ascii_string(&u[ds..j]));
            i = j;
        }
    }
    let mut text = String::new();
    if neg {
        text.push('-');
    }
    let int_text = if int_part.is_empty() { "0".to_string() } else { ascii_string(int_part) };
    let frac_text = if frac_part.is_empty() { "0".to_string() } else { ascii_string(frac_part) };
    text.push_str(&int_text);
    text.push('.');
    text.push_str(&frac_text);
    text.push_str(&exp_text);
    // Rust's parser rounds correctly; the grammar was checked above, so only ASCII digits remain.
    let v: f64 = text.parse().ok()?;
    Some((v, i))
}

/// Value of one digit in radixes up to 36 (case-insensitive letters).
fn digit_val(c: u16) -> Option<u32> {
    match c {
        0x30..=0x39 => Some((c - 0x30) as u32),
        0x61..=0x7A => Some((c - 0x61) as u32 + 10),
        0x41..=0x5A => Some((c - 0x41) as u32 + 10),
        _ => None,
    }
}

/// 2^e for 0 <= e; +Infinity above the largest finite power.
fn pow2(e: usize) -> f64 {
    if e > 1023 { f64::INFINITY } else { f64::from_bits(((e as u64) + 1023) << 52) }
}

/// Correctly rounded (ties to even) value of digits in base 2^bits, most significant first.
fn pow2_value(digits: &[u32], bits: u32) -> f64 {
    let mut bv: Vec<bool> = Vec::with_capacity(digits.len() * bits as usize);
    for &d in digits {
        for b in (0..bits).rev() {
            bv.push((d >> b) & 1 == 1);
        }
    }
    let Some(first) = bv.iter().position(|&b| b) else {
        return 0.0;
    };
    let sig = &bv[first..];
    if sig.len() <= 53 {
        return sig.iter().fold(0u64, |m, &b| (m << 1) | (b as u64)) as f64;
    }
    let mut m: u64 = sig[..53].iter().fold(0u64, |m, &b| (m << 1) | (b as u64));
    let round = sig[53];
    let sticky = sig[54..].iter().any(|&b| b);
    if round && (sticky || m & 1 == 1) {
        m += 1;
    }
    // m <= 2^53 is exact as f64; scaling by 2^shift is exact unless it overflows.
    let shift = sig.len() - 53;
    m as f64 * pow2(shift)
}

/// Value of a power-of-two radix literal after its 0x/0o/0b prefix; NaN for any invalid digit.
fn radix_literal(digits: &[u16], bits: u32) -> f64 {
    let mut vals = Vec::with_capacity(digits.len());
    for &c in digits {
        match digit_val(c) {
            Some(v) if v < (1 << bits) => vals.push(v),
            _ => return f64::NAN,
        }
    }
    if vals.is_empty() {
        return f64::NAN;
    }
    pow2_value(&vals, bits)
}

/// StringToNumber (ECMA-262 7.1.4.1.1). Whitespace is trimmed; the empty string is 0; 0x, 0o and 0b
/// literals take no sign; Infinity takes an optional sign; numeric separators are rejected.
pub fn string_to_number(s: &JsStr) -> f64 {
    let u = trim_units(s.units(), true, true);
    if u.is_empty() {
        return 0.0;
    }
    if u.len() > 2 && is_unit(u[0], b'0') {
        let bits = match u[1] {
            c if is_unit(c, b'x') || is_unit(c, b'X') => 4,
            c if is_unit(c, b'o') || is_unit(c, b'O') => 3,
            c if is_unit(c, b'b') || is_unit(c, b'B') => 1,
            _ => 0,
        };
        if bits > 0 {
            return radix_literal(&u[2..], bits);
        }
    }
    match scan_decimal(u) {
        Some((v, used)) if used == u.len() => v,
        _ => f64::NAN,
    }
}

/// parseFloat: leading whitespace is skipped and the longest decimal literal prefix is parsed.
pub fn parse_float(s: &JsStr) -> f64 {
    let u = trim_units(s.units(), true, false);
    match scan_decimal(u) {
        Some((v, _)) => v,
        None => f64::NAN,
    }
}

/// Generic-radix accumulation as V8 does it (NumberParseIntHelper::HandleGenericCase). Digits are taken
/// in parts whose multiplier stays within 32 bits, and each part is folded in with `number * mult + part`.
/// V8 on arm64 evaluates that expression fused (one rounding), which `mul_add` reproduces. The digits
/// must not start with zero (`parse_int` strips leading zeros first, as V8 does). ECMA-262 permits this
/// approximation for radixes other than 2, 4, 8, 10, 16 and 32.
fn generic_radix_value(digits: &[u32], radix: u32) -> f64 {
    const MAX_MULT: u32 = u32::MAX / 36;
    let mut number: f64 = 0.0;
    let mut i = 0;
    while i < digits.len() {
        let mut part: u32 = 0;
        let mut mult: u32 = 1;
        while i < digits.len() {
            // mult <= MAX_MULT, so mult * radix cannot overflow u32.
            let m = mult * radix;
            if m > MAX_MULT {
                break;
            }
            part = part * radix + digits[i];
            mult = m;
            i += 1;
        }
        number = number.mul_add(mult as f64, part as f64);
    }
    number
}

/// parseInt(string, radix). The caller applies ToInt32 to the radix argument (0 when absent).
pub fn parse_int(s: &JsStr, radix: i32) -> f64 {
    let u = trim_units(s.units(), true, false);
    let mut i = 0;
    let mut sign = 1.0;
    if i < u.len() && is_unit(u[i], b'-') {
        sign = -1.0;
        i += 1;
    } else if i < u.len() && is_unit(u[i], b'+') {
        i += 1;
    }
    let mut r = radix;
    let mut strip_prefix = true;
    if r != 0 {
        if !(2..=36).contains(&r) {
            return f64::NAN;
        }
        if r != 16 {
            strip_prefix = false;
        }
    } else {
        r = 10;
    }
    if strip_prefix && i + 1 < u.len() && is_unit(u[i], b'0') && (is_unit(u[i + 1], b'x') || is_unit(u[i + 1], b'X')) {
        i += 2;
        r = 16;
    }
    let radix = r as u32;
    let start = i;
    while i < u.len() && digit_val(u[i]).is_some_and(|d| d < radix) {
        i += 1;
    }
    if i == start {
        return f64::NAN;
    }
    // Leading zeros do not change the value, but V8 drops them before chunking digits, and the chunk
    // boundaries decide the rounding for long strings in generic radixes.
    let mut digits: Vec<u32> = u[start..i].iter().filter_map(|&c| digit_val(c)).collect();
    let lead = digits.iter().take_while(|&&d| d == 0).count();
    digits.drain(..lead);
    if digits.is_empty() {
        return sign * 0.0;
    }
    let v = match radix {
        10 => {
            let text: String = digits.iter().map(|&d| char::from(b'0' + d as u8)).collect();
            text.parse::<f64>().unwrap_or(f64::NAN)
        }
        2 => pow2_value(&digits, 1),
        4 => pow2_value(&digits, 2),
        8 => pow2_value(&digits, 3),
        16 => pow2_value(&digits, 4),
        32 => pow2_value(&digits, 5),
        _ => generic_radix_value(&digits, radix),
    };
    sign * v
}

/// ToUint32 (ECMA-262 7.1.7): truncate, then modulo 2^32.
pub fn to_uint32(x: f64) -> u32 {
    if !x.is_finite() {
        return 0;
    }
    x.trunc().rem_euclid(4_294_967_296.0) as u32
}

pub fn to_int32(x: f64) -> i32 {
    to_uint32(x) as i32
}

pub fn to_uint16(x: f64) -> u16 {
    to_uint32(x) as u16
}

pub fn to_int16(x: f64) -> i16 {
    to_uint16(x) as i16
}

pub fn to_uint8(x: f64) -> u8 {
    to_uint32(x) as u8
}

pub fn to_int8(x: f64) -> i8 {
    to_uint8(x) as i8
}

/// `%` (ECMA-262 6.1.6.1.6): fmod semantics; the result takes the sign of the dividend.
pub fn js_rem(a: f64, b: f64) -> f64 {
    a % b
}

/// `**` and Math.pow (ECMA-262 6.1.6.1.3 Number::exponentiate). Differs from C `pow` for NaN exponents
/// and for |base| = 1 with an infinite exponent. The exponents 1, 2 and 0.5 (non-negative base) are computed as
/// x, x * x and sqrt(x), which matched V8 14.6 on every sampled input. Every other case uses the platform `powf`:
/// V8 has its own pow, and a pure-Rust fdlibm pow disagreed with V8 14.6 on about 3% of random inputs, so it is not
/// used (see `exponentiation_matches_v8` in crates/value/tests/number_corpus.rs). x ** -1 is not 1 / x: V8 returns
/// 1 for 0.9999999999999999 ** -1, where 1 / x rounds up.
pub fn js_pow(base: f64, exp: f64) -> f64 {
    if exp.is_nan() {
        return f64::NAN;
    }
    if exp == 0.0 {
        return 1.0;
    }
    if base.abs() == 1.0 && exp.is_infinite() {
        return f64::NAN;
    }
    if exp == 1.0 {
        return base;
    }
    if exp == 2.0 {
        return base * base;
    }
    if exp == 0.5 && base.is_sign_positive() {
        return base.sqrt();
    }
    base.powf(exp)
}

/// Math.round: halves go toward +Infinity and -0 is preserved (Math.round(-0.4) is -0).
pub fn js_round(x: f64) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let f = x.floor();
    let r = if x - f >= 0.5 { f + 1.0 } else { f };
    if r == 0.0 && x < 0.0 { -0.0 } else { r }
}

/// Math.min: NaN if any argument is NaN; -0 is less than +0; no arguments give +Infinity.
pub fn js_min(values: &[f64]) -> f64 {
    let mut r = f64::INFINITY;
    for &v in values {
        if v.is_nan() {
            return f64::NAN;
        }
        if v < r || (v == 0.0 && r == 0.0 && v.is_sign_negative()) {
            r = v;
        }
    }
    r
}

/// Math.max: NaN if any argument is NaN; +0 is greater than -0; no arguments give -Infinity.
pub fn js_max(values: &[f64]) -> f64 {
    let mut r = f64::NEG_INFINITY;
    for &v in values {
        if v.is_nan() {
            return f64::NAN;
        }
        if v > r || (v == 0.0 && r == 0.0 && v.is_sign_positive()) {
            r = v;
        }
    }
    r
}

/// SameValue (Object.is): NaN equals NaN, and +0 differs from -0.
pub fn same_value(x: f64, y: f64) -> bool {
    (x.is_nan() && y.is_nan()) || x.to_bits() == y.to_bits()
}

/// SameValueZero: NaN equals NaN, and +0 equals -0.
pub fn same_value_zero(x: f64, y: f64) -> bool {
    (x.is_nan() && y.is_nan()) || x == y
}

/// Number.isInteger: finite and mathematically integral.
pub fn is_integer_valued(x: f64) -> bool {
    x.is_finite() && x.trunc() == x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> JsStr {
        JsStr::from_str(text)
    }

    #[test]
    fn to_string_formats_match_ecma_thresholds() {
        assert_eq!(number_to_string(1e21), "1e+21");
        assert_eq!(number_to_string(1e20), "100000000000000000000");
        assert_eq!(number_to_string(1e-7), "1e-7");
        assert_eq!(number_to_string(1e-6), "0.000001");
        assert_eq!(number_to_string(123e-20), "1.23e-18");
        assert_eq!(number_to_string(-0.0), "0");
        assert_eq!(number_to_string(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(number_to_string(5e-324), "5e-324");
        assert_eq!(number_to_string(f64::MAX), "1.7976931348623157e+308");
        assert_eq!(number_to_string(9_007_199_254_740_992.0), "9007199254740992");
        assert_eq!(number_to_string(-42.5), "-42.5");
    }

    #[test]
    fn to_fixed_rounds_on_the_exact_binary_value() {
        assert_eq!(to_fixed(1.005, 2).unwrap(), "1.00");
        assert_eq!(to_fixed(1.45, 1).unwrap(), "1.4");
        assert_eq!(to_fixed(0.5, 0).unwrap(), "1");
        assert_eq!(to_fixed(2.5, 0).unwrap(), "3");
        assert_eq!(to_fixed(-2.5, 0).unwrap(), "-3");
        assert_eq!(to_fixed(-0.0001, 2).unwrap(), "-0.00");
        assert_eq!(to_fixed(-0.0, 2).unwrap(), "0.00");
        assert_eq!(to_fixed(9.999, 2).unwrap(), "10.00");
        assert_eq!(to_fixed(1e21, 2).unwrap(), "1e+21");
        assert_eq!(to_fixed(f64::NAN, 2).unwrap(), "NaN");
        assert_eq!(to_fixed(1.0, 101), Err(NumErr::Range));
    }

    #[test]
    fn js_argument_checks_follow_the_spec_order() {
        assert_eq!(to_fixed_js(1.5, f64::NAN).unwrap(), "2");
        assert_eq!(to_fixed_js(1.5, -0.5).unwrap(), "2");
        assert_eq!(to_fixed_js(1.5, 2.9).unwrap(), "1.50");
        assert_eq!(to_fixed_js(1.5, f64::INFINITY), Err(NumErr::Range));
        assert_eq!(to_fixed_js(f64::NAN, -1.0), Err(NumErr::Range));
        // toExponential and toPrecision return the non-finite string before checking the argument.
        assert_eq!(to_exponential_js(f64::NAN, Some(101.0)).unwrap(), "NaN");
        assert_eq!(to_precision_js(f64::NAN, Some(0.0)).unwrap(), "NaN");
        assert_eq!(to_precision_js(1.0, Some(0.0)), Err(NumErr::Range));
        assert_eq!(number_to_string_radix_js(1.0, 1.0), Err(NumErr::Range));
        assert_eq!(number_to_string_radix_js(1.0, f64::NAN), Err(NumErr::Range));
    }

    #[test]
    fn exponential_and_precision_rounding() {
        assert_eq!(to_exponential(123456.0, Some(2)).unwrap(), "1.23e+5");
        assert_eq!(to_exponential(0.0, Some(2)).unwrap(), "0.00e+0");
        assert_eq!(to_exponential(0.0, None).unwrap(), "0e+0");
        assert_eq!(to_exponential(1.5, Some(0)).unwrap(), "2e+0");
        assert_eq!(to_exponential(2.5, Some(0)).unwrap(), "3e+0");
        assert_eq!(to_exponential(9.5, Some(0)).unwrap(), "1e+1");
        assert_eq!(to_precision(123.456, Some(4)).unwrap(), "123.5");
        assert_eq!(to_precision(0.000001234, Some(2)).unwrap(), "0.0000012");
        assert_eq!(to_precision(1e-7, Some(1)).unwrap(), "1e-7");
        assert_eq!(to_precision(1e21, Some(3)).unwrap(), "1.00e+21");
        assert_eq!(to_precision(123456.0, Some(2)).unwrap(), "1.2e+5");
        assert_eq!(to_precision(0.0, Some(3)).unwrap(), "0.00");
        assert_eq!(to_precision(-1.5, Some(1)).unwrap(), "-2");
        assert_eq!(to_precision(1.5, None).unwrap(), "1.5");
    }

    #[test]
    fn radix_conversion_matches_v8_samples() {
        assert_eq!(number_to_string_radix(255.0, 16), "ff");
        assert_eq!(number_to_string_radix(-255.0, 2), "-11111111");
        assert_eq!(number_to_string_radix(35.0, 36), "z");
        assert_eq!(number_to_string_radix(0.5, 2), "0.1");
        assert_eq!(number_to_string_radix(-0.0, 2), "0");
    }

    #[test]
    fn string_to_number_literals() {
        assert_eq!(string_to_number(&s("  0x1F \n")), 31.0);
        assert_eq!(string_to_number(&s("0o17")), 15.0);
        assert_eq!(string_to_number(&s("0b101")), 5.0);
        assert!(string_to_number(&s("0b102")).is_nan());
        assert!(string_to_number(&s("-0x1F")).is_nan());
        assert!(string_to_number(&s("0x")).is_nan());
        assert!(string_to_number(&s("1_0")).is_nan());
        assert_eq!(string_to_number(&s("")), 0.0);
        assert_eq!(string_to_number(&s(" \u{3000} ")), 0.0);
        assert_eq!(string_to_number(&s("5.")), 5.0);
        assert_eq!(string_to_number(&s(".5")), 0.5);
        assert!(string_to_number(&s(".")).is_nan());
        assert!(string_to_number(&s("1e")).is_nan());
        assert_eq!(string_to_number(&s("-Infinity")), f64::NEG_INFINITY);
        assert!(string_to_number(&s("infinity")).is_nan());
        assert_eq!(string_to_number(&s("1e400")), f64::INFINITY);
        assert_eq!(string_to_number(&s("1e-400")).to_bits(), 0);
        assert_eq!(string_to_number(&s("-0")).to_bits(), (-0.0f64).to_bits());
        // 0x followed by 20 f's is 2^80 - 1, which rounds to 2^80.
        assert_eq!(string_to_number(&s("0xffffffffffffffffffff")), 2f64.powi(80));
        assert_eq!(string_to_number(&s("1e99999999999999999999")), f64::INFINITY);
    }

    #[test]
    fn parse_float_takes_the_longest_prefix() {
        assert_eq!(parse_float(&s("3.14abc")), 3.14);
        assert_eq!(parse_float(&s("1e")), 1.0);
        assert_eq!(parse_float(&s("-.5e-1x")), -0.05);
        assert_eq!(parse_float(&s("Infinityx")), f64::INFINITY);
        assert_eq!(parse_float(&s("0x10")), 0.0);
        assert!(parse_float(&s("-.e1")).is_nan());
    }

    #[test]
    fn parse_int_radixes_and_prefixes() {
        assert_eq!(parse_int(&s("0x1F"), 0), 31.0);
        assert_eq!(parse_int(&s("0x1F"), 10), 0.0);
        assert_eq!(parse_int(&s("-0x10"), 16), -16.0);
        assert_eq!(parse_int(&s("z"), 36), 35.0);
        assert!(parse_int(&s("12"), 37).is_nan());
        assert!(parse_int(&s("12"), 1).is_nan());
        assert!(parse_int(&s(""), 0).is_nan());
        assert_eq!(parse_int(&s("-0"), 0).to_bits(), (-0.0f64).to_bits());
        assert_eq!(parse_int(&s("1010"), 2), 10.0);
    }

    #[test]
    fn to_integer_family_wraps_modulo() {
        assert_eq!(to_int32(2147483648.0), -2147483648);
        assert_eq!(to_int32(-1.5), -1);
        assert_eq!(to_int32(f64::NAN), 0);
        assert_eq!(to_uint32(-1.0), 4294967295);
        assert_eq!(to_int8(128.0), -128);
        assert_eq!(to_uint8(-1.0), 255);
        assert_eq!(to_int16(32768.0), -32768);
        assert_eq!(to_uint16(65537.0), 1);
    }

    #[test]
    fn remainder_power_round_min_max() {
        assert_eq!(js_rem(-1.0, 1.0).to_bits(), (-0.0f64).to_bits());
        assert_eq!(js_rem(5.5, 2.0), 1.5);
        assert!(js_rem(1.0, 0.0).is_nan());
        assert!(js_pow(1.0, f64::INFINITY).is_nan());
        assert!(js_pow(-1.0, f64::NEG_INFINITY).is_nan());
        assert_eq!(js_pow(f64::NAN, 0.0), 1.0);
        assert!(js_pow(f64::NAN, 2.0).is_nan());
        assert!(js_pow(-8.0, 1.0 / 3.0).is_nan());
        assert_eq!(js_pow(-0.0, -3.0), f64::NEG_INFINITY);
        assert_eq!(js_round(-0.5).to_bits(), (-0.0f64).to_bits());
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(0.49999999999999994), 0.0);
        assert_eq!(js_min(&[]), f64::INFINITY);
        assert_eq!(js_max(&[]), f64::NEG_INFINITY);
        assert_eq!(js_min(&[0.0, -0.0]).to_bits(), (-0.0f64).to_bits());
        assert_eq!(js_max(&[-0.0, 0.0]).to_bits(), 0.0f64.to_bits());
        assert!(js_min(&[1.0, f64::NAN]).is_nan());
    }

    #[test]
    fn same_value_and_integers() {
        assert!(same_value(f64::NAN, f64::NAN));
        assert!(!same_value(0.0, -0.0));
        assert!(same_value_zero(0.0, -0.0));
        assert!(is_integer_valued(1e300));
        assert!(!is_integer_valued(1.5));
        assert!(!is_integer_valued(f64::INFINITY));
    }
}

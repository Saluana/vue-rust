// The structure of V8's ieee754::atanh (src/base/ieee754.cc), itself fdlibm's e_atanh.c as adapted by V8. The routine
// it calls (log1p) is this module's. Original notice:
//
// ====================================================
// Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//
// Developed at SunSoft, a Sun Microsystems, Inc. business.
// Permission to use, copy, modify, and distribute this
// software is freely granted, provided that this notice
// is preserved.
// ====================================================
//
// The original source code covered by the above license has been modified significantly by Google Inc.
// Copyright 2016 the V8 project authors. All rights reserved.

use super::{log1p, with_set_high_word};

/// Inverse hyperbolic tangent: 0.5 * log1p(2x + 2x^2 / (1 - x)) for x < 0.5 and 0.5 * log1p(2x / (1 - x)) otherwise,
/// for |x| < 1; NaN for |x| > 1 and the infinities at x = +-1.
pub(crate) fn atanh(x: f64) -> f64 {
    const ONE: f64 = 1.0;
    const HUGE: f64 = 1e300;
    const ZERO: f64 = 0.0;

    let bits = x.to_bits();
    let hx = (bits >> 32) as i32;
    let lx = bits as u32;
    let ix = hx & 0x7FFFFFFF;
    // (lx | -lx) has its top bit set exactly when lx != 0, so this tests |x| > 1 (including NaN).
    let lx_nonzero = ((lx | lx.wrapping_neg()) >> 31) as i32;
    if (ix | lx_nonzero) > 0x3FF00000 {
        // |x| > 1
        return f64::NAN;
    }
    if ix == 0x3FF00000 {
        return if x > 0.0 { f64::INFINITY } else { f64::NEG_INFINITY };
    }
    if ix < 0x3E300000 && (HUGE + x) > ZERO {
        // x < 2**-28
        return x;
    }
    let x = with_set_high_word(x, ix as u32);
    let t = if ix < 0x3FE00000 {
        // x < 0.5
        let t = x + x;
        0.5 * log1p(t + t * x / (ONE - x))
    } else {
        0.5 * log1p((x + x) / (ONE - x))
    };
    if hx >= 0 { t } else { -t }
}

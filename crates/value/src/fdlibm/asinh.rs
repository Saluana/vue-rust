// The structure of V8's ieee754::asinh (src/base/ieee754.cc), itself fdlibm's s_asinh.c as adapted by V8. The routines
// it calls (log, log1p and sqrt) are this module's. Original notice:
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

use super::{fabs, log, log1p, sqrt};

/// Inverse hyperbolic sine: sign(x) * log(|x| + sqrt(x * x + 1)), evaluated in the form that avoids overflow and
/// cancellation for each range of |x|.
pub(crate) fn asinh(x: f64) -> f64 {
    const ONE: f64 = 1.00000000000000000000e+00; // 0x3FF00000, 0x00000000
    const LN2: f64 = 6.93147180559945286227e-01; // 0x3FE62E42, 0xFEFA39EF
    const HUGE: f64 = 1.00000000000000000000e+300;

    let hx = get_high_word_signed(x);
    let ix = hx & 0x7FFFFFFF;
    if ix >= 0x7FF00000 {
        // x is inf or NaN
        return x + x;
    }
    if ix < 0x3E300000 {
        // |x| < 2**-28
        if HUGE + x > ONE {
            return x; // return x inexact except 0
        }
    }
    let w = if ix > 0x41B00000 {
        // |x| > 2**28
        log(fabs(x)) + LN2
    } else if ix > 0x40000000 {
        // 2**28 > |x| > 2.0
        let t = fabs(x);
        log(2.0 * t + ONE / (sqrt(x * x + ONE) + t))
    } else {
        // 2.0 > |x| > 2**-28
        let t = x * x;
        log1p(fabs(x) + t / (ONE + sqrt(ONE + t)))
    };
    if hx > 0 { w } else { -w }
}

/// The signed high word of `x`, as V8's GET_HIGH_WORD gives it for an int32_t.
fn get_high_word_signed(x: f64) -> i32 {
    (x.to_bits() >> 32) as i32
}

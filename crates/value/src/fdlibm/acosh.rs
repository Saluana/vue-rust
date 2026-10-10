// The structure of V8's ieee754::acosh (src/base/ieee754.cc), itself fdlibm's e_acosh.c as adapted by V8. The routines
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

use super::{get_low_word, log, log1p, sqrt};

/// Inverse hyperbolic cosine. NaN for x < 1 (V8 returns a signaling NaN there, which is NaN in JavaScript).
pub(crate) fn acosh(x: f64) -> f64 {
    const ONE: f64 = 1.0;
    const LN2: f64 = 6.93147180559945286227e-01; // 0x3FE62E42, 0xFEFA39EF

    let hx = (x.to_bits() >> 32) as i32;
    let lx = get_low_word(x);
    if hx < 0x3FF00000 {
        // x < 1
        return f64::NAN;
    } else if hx >= 0x41B00000 {
        // x > 2**28
        if hx >= 0x7FF00000 {
            // x is inf of NaN
            return x + x;
        } else {
            // acosh(huge) = log(2x)
            return log(x) + LN2;
        }
    } else if (((hx - 0x3FF00000) as u32) | lx) == 0 {
        // acosh(1) = 0
        return 0.0;
    } else if hx > 0x40000000 {
        // 2**28 > x > 2
        let t = x * x;
        return log(2.0 * x - ONE / (x + sqrt(t - ONE)));
    } else {
        // 1 < x < 2
        let t = x - ONE;
        return log1p(t + sqrt(2.0 * t + t * t));
    }
}

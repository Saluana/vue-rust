// origin: FreeBSD /usr/src/lib/msun/src/s_tanh.c
//
// ====================================================
// Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//
// Developed at SunPro, a Sun Microsystems, Inc. business.
// Permission to use, copy, modify, and distribute this
// software is freely granted, provided that this notice
// is preserved.
// ====================================================

use super::{expm1, fabs, get_high_word};

const TINY: f64 = 1.0e-300;
const HUGE: f64 = 1.0e300;

/// Hyperbolic tangent. tanh(x) = -t / (t + 2) with t = expm1(-2|x|) for |x| < 1, and 1 - 2 / (t + 2) with
/// t = expm1(2|x|) for 1 <= |x| < 22; the result is 1 beyond 22.
pub(crate) fn tanh(x: f64) -> f64 {
    let jx = get_high_word(x) as i32;
    let ix = jx & 0x7fffffff;

    // x is INF or NaN
    if ix >= 0x7ff00000 {
        if jx >= 0 {
            // tanh(+-inf) = +-1 (tanh(+inf) = 1)
            return 1.0 / x + 1.0;
        } else {
            return 1.0 / x - 1.0;
        }
    }

    // |x| < 22
    let z = if ix < 0x40360000 {
        // |x| < 2**-28
        if ix < 0x3e300000 && HUGE + x > 1.0 {
            // tanh(tiny) = tiny with inexact
            return x;
        }
        if ix >= 0x3ff00000 {
            // |x| >= 1
            let t = expm1(2.0 * fabs(x));
            1.0 - 2.0 / (t + 2.0)
        } else {
            let t = expm1(-2.0 * fabs(x));
            -t / (t + 2.0)
        }
    } else {
        // |x| >= 22, return +-1
        1.0 - TINY
    };
    if jx >= 0 { z } else { -z }
}

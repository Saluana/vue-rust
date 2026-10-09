// The structure of V8's ieee754::cosh (src/base/ieee754.cc), itself fdlibm's e_cosh.c as adapted by V8. The routines it
// calls (expm1 and exp) are the fdlibm ports of this module. Original notice:
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

use super::{exp, expm1, fabs, get_high_word};

/// Hyperbolic cosine: 1 + expm1(|x|)^2 / (2 * exp(|x|)) for |x| < 0.5 * ln 2, (exp(|x|) + 1 / exp(|x|)) / 2 up to 22,
/// exp(|x|) / 2 up to log(DBL_MAX), and a scaled product of two exponentials up to the overflow threshold.
pub(crate) fn cosh(x: f64) -> f64 {
    const KCOSH_OVERFLOW: f64 = 710.4758600739439;

    // High word of |x|.
    let ix = get_high_word(x) & 0x7fffffff;

    // |x| in [0, 0.5*log2], return 1+expm1(|x|)^2/(2*exp(|x|))
    if ix < 0x3FD62E43 {
        let t = expm1(fabs(x));
        let w = 1.0 + t;
        // For |x| < 2^-55, cosh(x) = 1
        if ix < 0x3C800000 {
            return w;
        }
        return 1.0 + (t * t) / (w + w);
    }

    // |x| in [0.5*log2, 22], return (exp(|x|)+1/exp(|x|)/2
    if ix < 0x40360000 {
        let t = exp(fabs(x));
        return 0.5 * t + 0.5 / t;
    }

    // |x| in [22, log(maxdouble)], return half*exp(|x|)
    if ix < 0x40862E42 {
        return 0.5 * exp(fabs(x));
    }

    // |x| in [log(maxdouble), overflowthreshold]
    if fabs(x) <= KCOSH_OVERFLOW {
        let w = exp(0.5 * fabs(x));
        let t = 0.5 * w;
        return t * w;
    }

    // x is INF or NaN
    if ix >= 0x7FF00000 {
        return x * x;
    }

    // |x| > overflowthreshold.
    f64::INFINITY
}

// The structure of V8's ieee754::sinh (src/base/ieee754.cc), itself fdlibm's e_sinh.c as adapted by V8. The routines
// it calls (expm1 and exp) are the fdlibm ports of this module. Original notice:
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

use super::{exp, expm1, fabs};

/// Hyperbolic sine. sinh(x) = sign(x) * 0.5 * (E + E / (E + 1)) with E = expm1(|x|) for |x| < 22, sign(x) * 0.5 *
/// exp(|x|) up to log(DBL_MAX), and a scaled product of two exponentials up to the overflow threshold.
pub(crate) fn sinh(x: f64) -> f64 {
    const KSINH_OVERFLOW: f64 = 710.4758600739439;
    const TWO_M28: f64 = 3.725290298461914e-9; // 2^-28, empty lower half
    const LOG_MAXD: f64 = 709.7822265625; // 0x40862E42 00000000, empty lower half
    const SHUGE: f64 = 1.0e307;

    let h = if x < 0.0 { -0.5 } else { 0.5 };
    // |x| in [0, 22]. return sign(x)*0.5*(E+E/(E+1))
    let ax = fabs(x);
    if ax < 22.0 {
        // For |x| < 2^-28, sinh(x) = x
        if ax < TWO_M28 {
            return x;
        }
        let t = expm1(ax);
        if ax < 1.0 {
            return h * (2.0 * t - t * t / (t + 1.0));
        }
        return h * (t + t / (t + 1.0));
    }
    // |x| in [22, log(maxdouble)], return 0.5 * exp(|x|)
    if ax < LOG_MAXD {
        return h * exp(ax);
    }
    // |x| in [log(maxdouble), overflowthreshold]
    if ax <= KSINH_OVERFLOW {
        let w = exp(0.5 * ax);
        let t = h * w;
        return t * w;
    }
    // |x| > overflowthreshold or is NaN: the product overflows to infinity with the sign of x (or NaN).
    x * SHUGE
}

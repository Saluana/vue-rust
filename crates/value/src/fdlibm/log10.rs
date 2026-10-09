// The classic fdlibm e_log10.c (Sun Microsystems), in the form V8 used before its move to LLVM libc. Original notice:
//
// ====================================================
// Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//
// Developed at SunSoft, a Sun Microsystems, Inc. business.
// Permission to use, copy, modify, and distribute this
// software is freely granted, provided that this notice
// is preserved.
// ====================================================

use super::{get_high_word, log, with_set_high_word};

const TWO54: f64 = 1.80143985094819840000e+16; // 0x43500000, 0x00000000
const IVLN10: f64 = 4.34294481903251816668e-01; // 0x3FDBCB7B, 0x1526E50E
const LOG10_2HI: f64 = 3.01029995663611771306e-01; // 0x3FD34413, 0x509F6000
const LOG10_2LO: f64 = 3.69423907715893078616e-13; // 0x3D59FEF3, 0x11F12B36

/// Base 10 logarithm. With n = ilogb(x) (adjusted so that the mantissa lies in [1/2, 1) after scaling),
/// log10(x) = n * log10_2hi + (n * log10_2lo + ivln10 * log(x / 2^n)).
pub(crate) fn log10(x: f64) -> f64 {
    let mut x = x;
    let lx = x.to_bits() as u32;
    let mut hx = get_high_word(x) as i32;
    let mut k: i32 = 0;
    if hx < 0x00100000 {
        // x < 2**-1022
        if (((hx & 0x7fffffff) as u32) | lx) == 0 {
            // log(+-0) = -inf
            return f64::NEG_INFINITY;
        }
        if hx < 0 {
            // log(-#) = NaN
            return f64::NAN;
        }
        // subnormal number, scale up x
        k -= 54;
        x *= TWO54;
        hx = get_high_word(x) as i32;
    }
    if hx >= 0x7ff00000 {
        return x + x;
    }
    k += (hx >> 20) - 1023;
    let i = (((k as u32) & 0x8000_0000) >> 31) as i32;
    let hx = (hx & 0x000fffff) | ((0x3ff - i) << 20);
    let y = (k + i) as f64;
    let x = with_set_high_word(x, hx as u32);
    let z = y * LOG10_2LO + IVLN10 * log(x);
    z + y * LOG10_2HI
}

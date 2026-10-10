// The routines in this directory are adapted from rust-lang/libm 0.2.11, which ports the FreeBSD msun library
// (lib/msun/src, the fdlibm lineage). Each file keeps its origin line and the Sun Microsystems notice of the routine it
// ports. Changes from the libm source: safe indexing replaces its unchecked indexing, the `force_eval!` statements that
// only raise floating-point flags are removed, the x87 and pointer-width specific code is removed (the 690-entry 2/pi
// table of the 64-bit variant is used on every target), the `no_panic` attributes and the test modules are removed, and
// the precision arm of rem_pio2_large is `unreachable!()` in every build (its only caller passes 1). The arithmetic is
// otherwise unchanged. sinh, cosh, asinh, acosh and atanh follow V8's own ieee754.cc, which is written from the same
// fdlibm routines.
//
// The MIT license of rust-lang/libm, from its LICENSE file:
//
// rust-lang/libm as a whole is available for use under the MIT license:
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! Deterministic transcendental functions for the Math builtins. They use only IEEE-754 basic operations and bit
//! manipulation, so every platform returns the same bits. They are fdlibm's algorithms, not the host C library's, and
//! the corpora under tests/language (V8's results) are the reference they are measured against.

/// Indexing with the bounds of the C original. The algorithms never index out of range, so this is the same code
/// as the unchecked accesses of libm, with the checks kept.
macro_rules! i {
    ($array:expr, $index:expr) => {
        $array[$index]
    };
    ($array:expr, $index:expr, = , $rhs:expr) => {
        $array[$index] = $rhs
    };
    ($array:expr, $index:expr, += , $rhs:expr) => {
        $array[$index] += $rhs
    };
    ($array:expr, $index:expr, -= , $rhs:expr) => {
        $array[$index] -= $rhs
    };
    ($array:expr, $index:expr, &= , $rhs:expr) => {
        $array[$index] &= $rhs
    };
    ($array:expr, $index:expr, == , $rhs:expr) => {
        $array[$index] == $rhs
    };
}

/// Integer or floating division as written in the C original.
macro_rules! div {
    ($a:expr, $b:expr) => {
        $a / $b
    };
}

mod acos;
mod acosh;
mod asin;
mod asinh;
mod atan;
mod atan2;
mod atanh;
mod cbrt;
mod cos;
mod cosh;
mod exp;
mod expm1;
mod k_cos;
mod k_sin;
mod k_tan;
mod log;
mod log1p;
mod log10;
mod log2;
mod rem_pio2;
mod rem_pio2_large;
mod scalbn;
mod sin;
mod sinh;
mod tan;
mod tanh;

pub(crate) use self::acos::acos;
pub(crate) use self::acosh::acosh;
pub(crate) use self::asin::asin;
pub(crate) use self::asinh::asinh;
pub(crate) use self::atan::atan;
pub(crate) use self::atan2::atan2;
pub(crate) use self::atanh::atanh;
pub(crate) use self::cbrt::cbrt;
pub(crate) use self::cos::cos;
pub(crate) use self::cosh::cosh;
pub(crate) use self::exp::exp;
pub(crate) use self::expm1::expm1;
pub(crate) use self::log::log;
pub(crate) use self::log1p::log1p;
pub(crate) use self::log10::log10;
pub(crate) use self::log2::log2;
pub(crate) use self::sin::sin;
pub(crate) use self::sinh::sinh;
pub(crate) use self::tan::tan;
pub(crate) use self::tanh::tanh;

use self::k_cos::k_cos;
use self::k_sin::k_sin;
use self::k_tan::k_tan;
use self::rem_pio2::rem_pio2;
use self::rem_pio2_large::rem_pio2_large;
use self::scalbn::scalbn;

#[inline]
fn fabs(x: f64) -> f64 {
    x.abs()
}

#[inline]
fn sqrt(x: f64) -> f64 {
    x.sqrt()
}

#[inline]
fn floor(x: f64) -> f64 {
    x.floor()
}

#[inline]
fn get_high_word(x: f64) -> u32 {
    (x.to_bits() >> 32) as u32
}

#[inline]
fn get_low_word(x: f64) -> u32 {
    x.to_bits() as u32
}

#[inline]
fn with_set_high_word(f: f64, hi: u32) -> f64 {
    let mut tmp = f.to_bits();
    tmp &= 0x00000000_ffffffff;
    tmp |= (hi as u64) << 32;
    f64::from_bits(tmp)
}

#[inline]
fn with_set_low_word(f: f64, lo: u32) -> f64 {
    let mut tmp = f.to_bits();
    tmp &= 0xffffffff_00000000;
    tmp |= lo as u64;
    f64::from_bits(tmp)
}

/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Waldemar Horwat
 *   Roger Lawrence
 *   Attila Szegedi
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
/****************************************************************
 *
 * The author of this software is David M. Gay.
 *
 * Copyright (c) 1991, 2000, 2001 by Lucent Technologies.
 *
 * Permission to use, copy, modify, and distribute this software for any
 * purpose without fee is hereby granted, provided that this entire notice
 * is included in all copies of any software which is or includes a copy
 * or modification of this software and in all copies of the supporting
 * documentation for such software.
 *
 * THIS SOFTWARE IS BEING PROVIDED "AS IS", WITHOUT ANY EXPRESS OR IMPLIED
 * WARRANTY.  IN PARTICULAR, NEITHER THE AUTHOR NOR LUCENT MAKES ANY
 * REPRESENTATION OR WARRANTY OF ANY KIND CONCERNING THE MERCHANTABILITY
 * OF THIS SOFTWARE OR ITS FITNESS FOR ANY PARTICULAR PURPOSE.
 *
 ***************************************************************/
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/rhino/dtoa/DToA.java.

//! Faithful port of com.google.javascript.rhino.dtoa.DToA.

// Preserve the Java predicates and its deliberately rounded logarithm estimate.
#![allow(
    clippy::manual_range_contains,
    clippy::approx_constant,
    clippy::int_plus_one
)]

use num_bigint::{BigInt, Sign};
use num_integer::Integer;

// Java-private constants and JS_dtostr are public to exercise the formatting modes.
pub const DTOSTR_STANDARD: i32 = 0;
pub const DTOSTR_STANDARD_EXPONENTIAL: i32 = 1;
pub const DTOSTR_FIXED: i32 = 2;
pub const DTOSTR_EXPONENTIAL: i32 = 3;
pub const DTOSTR_PRECISION: i32 = 4;
const FRAC_MASK: i32 = 0xfffff;
const EXP_SHIFT: i32 = 20;
const EXP_MSK1: i32 = 0x100000;
const BIAS: i32 = 1023;
const P: i32 = 53;
const EXP_SHIFT1: i32 = 20;
const EXP_MASK: i32 = 0x7ff00000;
const BNDRY_MASK: i32 = 0xfffff;
const LOG2_P: i32 = 1;
const SIGN_BIT: i32 = i32::MIN;
const EXP_11: i32 = 0x3ff00000;
const TEN_PMAX: i32 = 22;
const QUICK_MAX: i32 = 14;
const BLETCH: i32 = 0x10;
const FRAC_MASK1: i32 = 0xfffff;
const INT_MAX: i32 = 14;
const N_BIGTENS: usize = 5;
const TENS: [f64; 23] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16,
    1e17, 1e18, 1e19, 1e20, 1e21, 1e22,
];
const BIGTENS: [f64; 5] = [1e16, 1e32, 1e64, 1e128, 1e256];

/// Java exceptions raised by DToA and its BigInteger operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DToAError {
    NegativeExponent,
    BigIntegerOverflow,
    ArrayIndexOutOfBounds { index: i32, length: usize },
}

impl DToAError {
    pub fn java_class(self) -> &'static str {
        match self {
            Self::NegativeExponent | Self::BigIntegerOverflow => "java.lang.ArithmeticException",
            Self::ArrayIndexOutOfBounds { .. } => "java.lang.ArrayIndexOutOfBoundsException",
        }
    }
}
impl std::fmt::Display for DToAError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NegativeExponent => f.write_str("Negative exponent"),
            Self::BigIntegerOverflow => f.write_str("BigInteger would overflow supported range"),
            Self::ArrayIndexOutOfBounds { index, length } => {
                write!(f, "Index {index} out of bounds for length {length}")
            }
        }
    }
}
impl std::error::Error for DToAError {}

// port: DToA#lo0bits
fn lo0bits(y: i32) -> i32 {
    let mut x = y;
    if (x & 7) != 0 {
        if (x & 1) != 0 {
            return 0;
        }
        if (x & 2) != 0 {
            return 1;
        }
        return 2;
    }
    let mut k = 0i32;
    if (x & 0xffff) == 0 {
        k = 16;
        x = (x as u32 >> 16) as i32;
    }
    if (x & 0xff) == 0 {
        k = k.wrapping_add(8);
        x = (x as u32 >> 8) as i32;
    }
    if (x & 0xf) == 0 {
        k = k.wrapping_add(4);
        x = (x as u32 >> 4) as i32;
    }
    if (x & 3) == 0 {
        k = k.wrapping_add(2);
        x = (x as u32 >> 2) as i32;
    }
    if (x & 1) == 0 {
        k = k.wrapping_add(1);
        x = (x as u32 >> 1) as i32;
        if (x & 1) == 0 {
            return 32;
        }
    }
    k
}

// port: DToA#hi0bits
fn hi0bits(mut x: i32) -> i32 {
    let mut k = 0i32;
    if (x & 0xffff0000u32 as i32) == 0 {
        k = 16;
        x = x.wrapping_shl(16);
    }
    if (x & 0xff000000u32 as i32) == 0 {
        k = k.wrapping_add(8);
        x = x.wrapping_shl(8);
    }
    if (x & 0xf0000000u32 as i32) == 0 {
        k = k.wrapping_add(4);
        x = x.wrapping_shl(4);
    }
    if (x & 0xc0000000u32 as i32) == 0 {
        k = k.wrapping_add(2);
        x = x.wrapping_shl(2);
    }
    if (x & SIGN_BIT) == 0 {
        k = k.wrapping_add(1);
        if (x & 0x40000000) == 0 {
            return 32;
        }
    }
    k
}

// port: DToA#stuffBits
fn stuff_bits(bits: &mut [u8], offset: usize, val: i32) {
    bits[offset] = (val >> 24) as u8;
    bits[offset + 1] = (val >> 16) as u8;
    bits[offset + 2] = (val >> 8) as u8;
    bits[offset + 3] = val as u8;
}

// port: Double#doubleToLongBits
fn double_to_long_bits(d: f64) -> u64 {
    if d.is_nan() {
        0x7ff8000000000000
    } else {
        d.to_bits()
    }
}

// port: DToA#d2b
fn d2b(d: f64, e: &mut i32, bits: &mut i32) -> BigInt {
    let d_bits = double_to_long_bits(d);
    let mut d0 = (d_bits >> 32) as i32;
    let d1 = d_bits as i32;
    let mut z = d0 & FRAC_MASK;
    d0 &= 0x7fffffff;
    let de = (d0 as u32 >> EXP_SHIFT) as i32;
    if de != 0 {
        z |= EXP_MSK1;
    }
    let mut y = d1;
    let mut k;
    let i;
    let mut dbl_bits;
    if y != 0 {
        dbl_bits = vec![0; 8];
        k = lo0bits(y);
        y = (y as u32).wrapping_shr(k as u32) as i32;
        if k != 0 {
            stuff_bits(&mut dbl_bits, 4, y | z.wrapping_shl((32 - k) as u32));
            z >>= k;
        } else {
            stuff_bits(&mut dbl_bits, 4, y);
        }
        stuff_bits(&mut dbl_bits, 0, z);
        i = if z != 0 { 2 } else { 1 };
    } else {
        dbl_bits = vec![0; 4];
        k = lo0bits(z);
        z = (z as u32).wrapping_shr(k as u32) as i32;
        stuff_bits(&mut dbl_bits, 0, z);
        k = k.wrapping_add(32);
        i = 1;
    }
    if de != 0 {
        *e = de - BIAS - (P - 1) + k;
        *bits = P - k;
    } else {
        *e = de - BIAS - (P - 1) + 1 + k;
        *bits = 32 * i - hi0bits(z);
    }
    BigInt::from_signed_bytes_be(&dbl_bits)
}

// port: DToA#word0
fn word0(d: f64) -> i32 {
    (double_to_long_bits(d) >> 32) as i32
}
// port: DToA#setWord0
fn set_word0(d: f64, i: i32) -> f64 {
    let d_bits = ((i as i64 as u64) << 32) | (double_to_long_bits(d) & 0xffffffff);
    f64::from_bits(d_bits)
}
// port: DToA#word1
fn word1(d: f64) -> i32 {
    double_to_long_bits(d) as i32
}
// The java.math.BigInteger arithmetic with its overflow checks, ported from OpenJDK (GPL-2.0 with
// the Classpath exception), is in its own file.
#[path = "d_to_a_jdk.rs"]
mod jdk;
use jdk::{check_initial_ranges, int_value, multiply, pow5, shift_left, subtract};

// port: DToA#pow5mult
fn pow5mult(b: BigInt, k: i32) -> Result<BigInt, DToAError> {
    multiply(b, &pow5(k)?)
}

// port: DToA#roundOff
fn round_off(buf: &mut Vec<u16>) -> bool {
    let mut i = buf.len();
    while i != 0 {
        i = i.wrapping_sub(1);
        let c = buf[i];
        if c != b'9' as u16 {
            buf[i] = c.wrapping_add(1);
            buf.truncate(i + 1);
            return false;
        }
    }
    buf.clear();
    true
}

// port: DToA#JS_dtoa
// Public for differential testing of all ten internal modes, including debug modes 4..9.
#[doc(hidden)]
#[allow(unused_assignments)] // Keep Java's otherwise-unused digit/allocation sizing assignments.
pub fn js_dtoa(
    mut d: f64,
    mut mode: i32,
    bias_up: bool,
    mut ndigits: i32,
    sign: &mut bool,
    buf: &mut Vec<u16>,
) -> Result<i32, DToAError> {
    let (mut be, mut bbits) = (0, 0);
    if (word0(d) & SIGN_BIT) != 0 {
        *sign = true;
        d = set_word0(d, word0(d) & !SIGN_BIT);
    } else {
        *sign = false;
    }
    if (word0(d) & EXP_MASK) == EXP_MASK {
        buf.extend(
            (if word1(d) == 0 && (word0(d) & FRAC_MASK) == 0 {
                "Infinity"
            } else {
                "NaN"
            })
            .encode_utf16(),
        );
        return Ok(9999);
    }
    if d == 0.0 {
        buf.clear();
        buf.push(b'0' as u16);
        return Ok(1);
    }
    let mut b = d2b(d, &mut be, &mut bbits);
    let mut i = ((word0(d) as u32 >> EXP_SHIFT1) as i32) & (EXP_MASK >> EXP_SHIFT1);
    let mut d2;
    let denorm;
    if i != 0 {
        d2 = set_word0(d, (word0(d) & FRAC_MASK1) | EXP_11);
        i = i.wrapping_sub(BIAS);
        denorm = false;
    } else {
        i = bbits + be + (BIAS + (P - 1) - 1);
        let x = if i > 32 {
            // Java's >>> operates on int before sign extension to long in the OR.
            (word0(d) as i64).wrapping_shl((64 - i) as u32)
                | ((word1(d) as u32).wrapping_shr((i - 32) as u32) as i32 as i64)
        } else {
            (word1(d) as i64).wrapping_shl((32 - i) as u32)
        };
        d2 = set_word0(x as f64, word0(x as f64).wrapping_sub(31 * EXP_MSK1));
        i = i.wrapping_sub((BIAS + (P - 1) - 1) + 1);
        denorm = true;
    }
    let mut ds = (d2 - 1.5) * 0.289529654602168 + 0.1760912590558 + i as f64 * 0.301029995663981;
    let mut k = ds as i32;
    if ds < 0.0 && ds != k as f64 {
        k = k.wrapping_sub(1);
    }
    let mut k_check = true;
    if k >= 0 && k <= TEN_PMAX {
        if d < TENS[k as usize] {
            k = k.wrapping_sub(1);
        }
        k_check = false;
    }
    let mut j = bbits - i - 1;
    let (mut b2, mut s2) = if j >= 0 {
        (0, j)
    } else {
        (j.wrapping_neg(), 0)
    };
    let (mut b5, mut s5);
    if k >= 0 {
        b5 = 0;
        s5 = k;
        s2 = s2.wrapping_add(k);
    } else {
        b2 = b2.wrapping_sub(k);
        b5 = k.wrapping_neg();
        s5 = 0;
    }
    if !(0..=9).contains(&mode) {
        mode = 0;
    }
    let mut try_quick = true;
    if mode > 5 {
        mode -= 4;
        try_quick = false;
    }
    let mut leftright = true;
    let (mut ilim, mut ilim1) = (0i32, 0i32);
    match mode {
        0 | 1 => {
            ilim = -1;
            ilim1 = -1;
            i = 18;
            ndigits = 0;
        }
        2 | 4 => {
            if mode == 2 {
                leftright = false;
            }
            if ndigits <= 0 {
                ndigits = 1;
            }
            ilim = ndigits;
            ilim1 = ndigits;
            i = ndigits;
        }
        3 | 5 => {
            if mode == 3 {
                leftright = false;
            }
            i = ndigits.wrapping_add(k).wrapping_add(1);
            ilim = i;
            ilim1 = i.wrapping_sub(1);
            if i <= 0 {
                i = 1;
            }
        }
        _ => {}
    }
    let mut fast_failed = false;
    if ilim >= 0 && ilim <= QUICK_MAX && try_quick {
        i = 0;
        d2 = d;
        let k0 = k;
        let ilim0 = ilim;
        let mut ieps = 2i32;
        if k > 0 {
            ds = TENS[(k & 0xf) as usize];
            j = k >> 4;
            if (j & BLETCH) != 0 {
                j &= BLETCH - 1;
                d /= BIGTENS[N_BIGTENS - 1];
                ieps = ieps.wrapping_add(1);
            }
            while j != 0 {
                if (j & 1) != 0 {
                    ieps = ieps.wrapping_add(1);
                    ds *= *BIGTENS
                        .get(i as usize)
                        .ok_or(DToAError::ArrayIndexOutOfBounds {
                            index: i,
                            length: BIGTENS.len(),
                        })?;
                }
                j >>= 1;
                i = i.wrapping_add(1);
            }
            d /= ds;
        } else {
            let j1 = k.wrapping_neg();
            if j1 != 0 {
                d *= TENS[(j1 & 0xf) as usize];
                j = j1 >> 4;
                while j != 0 {
                    if (j & 1) != 0 {
                        ieps = ieps.wrapping_add(1);
                        d *= *BIGTENS
                            .get(i as usize)
                            .ok_or(DToAError::ArrayIndexOutOfBounds {
                                index: i,
                                length: BIGTENS.len(),
                            })?;
                    }
                    j >>= 1;
                    i = i.wrapping_add(1);
                }
            }
        }
        if k_check && d < 1.0 && ilim > 0 {
            if ilim1 <= 0 {
                fast_failed = true;
            } else {
                ilim = ilim1;
                k = k.wrapping_sub(1);
                d *= 10.0;
                ieps = ieps.wrapping_add(1);
            }
        }
        let mut eps = ieps as f64 * d + 7.0;
        eps = set_word0(eps, word0(eps).wrapping_sub((P - 1) * EXP_MSK1));
        if ilim == 0 {
            d -= 5.0;
            if d > eps {
                buf.push(b'1' as u16);
                k = k.wrapping_add(1);
                return Ok(k.wrapping_add(1));
            }
            if d < -eps {
                buf.clear();
                buf.push(b'0' as u16);
                return Ok(1);
            }
            fast_failed = true;
        }
        if !fast_failed {
            fast_failed = true;
            if leftright {
                eps = 0.5 / TENS[(ilim - 1) as usize] - eps;
                i = 0;
                loop {
                    let l = d as i64;
                    d -= l as f64;
                    buf.push((b'0' as i64 + l) as u16);
                    if d < eps {
                        return Ok(k.wrapping_add(1));
                    }
                    if 1.0 - d < eps {
                        let mut last_ch;
                        loop {
                            last_ch = buf.pop().unwrap_or(b'0' as u16);
                            if last_ch != b'9' as u16 {
                                break;
                            }
                            if buf.is_empty() {
                                k = k.wrapping_add(1);
                                last_ch = b'0' as u16;
                                break;
                            }
                        }
                        buf.push(last_ch.wrapping_add(1));
                        return Ok(k.wrapping_add(1));
                    }
                    i = i.wrapping_add(1);
                    if i >= ilim {
                        break;
                    }
                    eps *= 10.0;
                    d *= 10.0;
                }
            } else {
                eps *= TENS[(ilim - 1) as usize];
                i = 1;
                loop {
                    let l = d as i64;
                    d -= l as f64;
                    buf.push((b'0' as i64 + l) as u16);
                    if i == ilim {
                        if d > 0.5 + eps {
                            let mut last_ch;
                            loop {
                                last_ch = buf.pop().unwrap_or(b'0' as u16);
                                if last_ch != b'9' as u16 {
                                    break;
                                }
                                if buf.is_empty() {
                                    k = k.wrapping_add(1);
                                    last_ch = b'0' as u16;
                                    break;
                                }
                            }
                            buf.push(last_ch.wrapping_add(1));
                            return Ok(k.wrapping_add(1));
                        } else if d < 0.5 - eps {
                            strip_trailing_zeroes(buf);
                            return Ok(k.wrapping_add(1));
                        }
                        break;
                    }
                    i = i.wrapping_add(1);
                    d *= 10.0;
                }
            }
        }
        if fast_failed {
            buf.clear();
            d = d2;
            k = k0;
            ilim = ilim0;
        }
    }
    if be >= 0 && k <= INT_MAX {
        ds = TENS[k as usize];
        if ndigits < 0 && ilim <= 0 {
            if ilim < 0 || d < 5.0 * ds || (!bias_up && d == 5.0 * ds) {
                buf.clear();
                buf.push(b'0' as u16);
                return Ok(1);
            }
            buf.push(b'1' as u16);
            k = k.wrapping_add(1);
            return Ok(k.wrapping_add(1));
        }
        i = 1;
        loop {
            let l = (d / ds) as i64;
            d -= l as f64 * ds;
            buf.push((b'0' as i64 + l) as u16);
            if i == ilim {
                d += d;
                if d > ds || (d == ds && ((l & 1) != 0 || bias_up)) {
                    let mut last_ch;
                    loop {
                        last_ch = buf.pop().unwrap_or(b'0' as u16);
                        if last_ch != b'9' as u16 {
                            break;
                        }
                        if buf.is_empty() {
                            k = k.wrapping_add(1);
                            last_ch = b'0' as u16;
                            break;
                        }
                    }
                    buf.push(last_ch.wrapping_add(1));
                }
                break;
            }
            d *= 10.0;
            if d == 0.0 {
                break;
            }
            i = i.wrapping_add(1);
        }
        return Ok(k.wrapping_add(1));
    }
    let mut m2 = b2;
    let mut m5 = b5;
    // Java null sentinels are never read; BigInt zero represents the unused slots.
    let mut mhi = BigInt::from(0);
    if leftright {
        if mode < 2 {
            i = if denorm {
                be + (BIAS + (P - 1) - 1 + 1)
            } else {
                1 + P - bbits
            };
        } else {
            j = ilim.wrapping_sub(1);
            if m5 >= j {
                m5 = m5.wrapping_sub(j);
            } else {
                j = j.wrapping_sub(m5);
                s5 = s5.wrapping_add(j);
                b5 = b5.wrapping_add(j);
                m5 = 0;
            }
            i = ilim;
            if i < 0 {
                m2 = m2.wrapping_sub(i);
                i = 0;
            }
        }
        b2 = b2.wrapping_add(i);
        s2 = s2.wrapping_add(i);
        mhi = BigInt::from(1);
    }
    if m2 > 0 && s2 > 0 {
        i = if m2 < s2 { m2 } else { s2 };
        b2 = b2.wrapping_sub(i);
        m2 = m2.wrapping_sub(i);
        s2 = s2.wrapping_sub(i);
    }
    check_initial_ranges(
        &b,
        b5,
        s5,
        m5,
        b2,
        s2,
        m2,
        leftright,
        mode < 2
            && word1(d) == 0
            && (word0(d) & BNDRY_MASK) == 0
            && (word0(d) & (EXP_MASK & EXP_MASK.wrapping_shl(1))) != 0,
        leftright && !(ilim <= 0 && mode > 2),
    )?;
    if b5 > 0 {
        if leftright {
            if m5 > 0 {
                mhi = pow5mult(mhi, m5)?;
                let b1 = multiply(mhi.clone(), &b)?;
                b = b1;
            }
            j = b5.wrapping_sub(m5);
            if j != 0 {
                b = pow5mult(b, j)?;
            }
        } else {
            b = pow5mult(b, b5)?;
        }
    }
    let mut s = BigInt::from(1);
    if s5 > 0 {
        s = pow5mult(s, s5)?;
    }
    let mut spec_case = false;
    if mode < 2
        && word1(d) == 0
        && (word0(d) & BNDRY_MASK) == 0
        && (word0(d) & (EXP_MASK & EXP_MASK.wrapping_shl(1))) != 0
    {
        b2 = b2.wrapping_add(LOG2_P);
        s2 = s2.wrapping_add(LOG2_P);
        spec_case = true;
    }
    let s_bytes = s.to_signed_bytes_be();
    let mut s_hi_word = 0i32;
    for idx in 0..4 {
        s_hi_word = s_hi_word.wrapping_shl(8);
        if idx < s_bytes.len() {
            s_hi_word |= s_bytes[idx] as i32 & 0xff;
        }
    }
    i = (if s5 != 0 { 32 - hi0bits(s_hi_word) } else { 1 }).wrapping_add(s2) & 0x1f;
    if i != 0 {
        i = 32 - i;
    }
    if i > 4 {
        i = i.wrapping_sub(4);
        b2 = b2.wrapping_add(i);
        m2 = m2.wrapping_add(i);
        s2 = s2.wrapping_add(i);
    } else if i < 4 {
        i = i.wrapping_add(28);
        b2 = b2.wrapping_add(i);
        m2 = m2.wrapping_add(i);
        s2 = s2.wrapping_add(i);
    }
    if b2 > 0 {
        b = shift_left(b, b2)?;
    }
    if s2 > 0 {
        s = shift_left(s, s2)?;
    }
    if k_check && b < s {
        k = k.wrapping_sub(1);
        b = multiply(b, &BigInt::from(10))?;
        if leftright {
            mhi = multiply(mhi, &BigInt::from(10))?;
        }
        ilim = ilim1;
    }
    if ilim <= 0 && mode > 2 {
        if ilim < 0
            || {
                s = multiply(s, &BigInt::from(5))?;
                i = b.cmp(&s) as i32;
                i < 0
            }
            || (i == 0 && !bias_up)
        {
            buf.clear();
            buf.push(b'0' as u16);
            return Ok(1);
        }
        buf.push(b'1' as u16);
        k = k.wrapping_add(1);
        return Ok(k.wrapping_add(1));
    }
    let mut dig;
    if leftright {
        if m2 > 0 {
            mhi = shift_left(mhi, m2)?;
        }
        let mut mlo = mhi.clone();
        // mlo = mhi aliases the Java object. shiftLeft(LOG2_P) creates a distinct object.
        let mlo_is_mhi = !spec_case;
        if spec_case {
            mhi = mlo.clone();
            mhi = shift_left(mhi, LOG2_P)?;
        }
        i = 1;
        loop {
            let div_result = b.div_rem(&s);
            b = div_result.1;
            dig = int_value(&div_result.0).wrapping_add(b'0' as i32) as u16;
            j = b.cmp(&mlo) as i32;
            let delta = subtract(&s, &mhi);
            let mut j1 = if delta.sign() != Sign::Plus {
                1
            } else {
                b.cmp(&delta) as i32
            };
            if j1 == 0 && mode == 0 && (word1(d) & 1) == 0 {
                if dig == b'9' as u16 {
                    buf.push(b'9' as u16);
                    if round_off(buf) {
                        k = k.wrapping_add(1);
                        buf.push(b'1' as u16);
                    }
                    return Ok(k.wrapping_add(1));
                }
                if j > 0 {
                    dig = dig.wrapping_add(1);
                }
                buf.push(dig);
                return Ok(k.wrapping_add(1));
            }
            if j < 0 || (j == 0 && mode == 0 && (word1(d) & 1) == 0) {
                if j1 > 0 {
                    b = shift_left(b, 1)?;
                    j1 = b.cmp(&s) as i32;
                    if j1 > 0 || (j1 == 0 && ((dig & 1) == 1 || bias_up)) {
                        let old_dig = dig;
                        dig = dig.wrapping_add(1);
                        if old_dig == b'9' as u16 {
                            buf.push(b'9' as u16);
                            if round_off(buf) {
                                k = k.wrapping_add(1);
                                buf.push(b'1' as u16);
                            }
                            return Ok(k.wrapping_add(1));
                        }
                    }
                }
                buf.push(dig);
                return Ok(k.wrapping_add(1));
            }
            if j1 > 0 {
                if dig == b'9' as u16 {
                    buf.push(b'9' as u16);
                    if round_off(buf) {
                        k = k.wrapping_add(1);
                        buf.push(b'1' as u16);
                    }
                    return Ok(k.wrapping_add(1));
                }
                buf.push(dig.wrapping_add(1));
                return Ok(k.wrapping_add(1));
            }
            buf.push(dig);
            if i == ilim {
                break;
            }
            b = multiply(b, &BigInt::from(10))?;
            if mlo_is_mhi {
                mhi = multiply(mhi, &BigInt::from(10))?;
                mlo = mhi.clone();
            } else {
                mlo = multiply(mlo, &BigInt::from(10))?;
                mhi = multiply(mhi, &BigInt::from(10))?;
            }
            i = i.wrapping_add(1);
        }
    } else {
        i = 1;
        loop {
            let div_result = b.div_rem(&s);
            b = div_result.1;
            dig = int_value(&div_result.0).wrapping_add(b'0' as i32) as u16;
            buf.push(dig);
            if i >= ilim {
                break;
            }
            b = multiply(b, &BigInt::from(10))?;
            i = i.wrapping_add(1);
        }
    }
    b = shift_left(b, 1)?;
    j = b.cmp(&s) as i32;
    if j > 0 || (j == 0 && ((dig & 1) == 1 || bias_up)) {
        if round_off(buf) {
            k = k.wrapping_add(1);
            buf.push(b'1' as u16);
            return Ok(k.wrapping_add(1));
        }
    } else {
        strip_trailing_zeroes(buf);
    }
    Ok(k.wrapping_add(1))
}

// port: DToA#stripTrailingZeroes
fn strip_trailing_zeroes(buf: &mut Vec<u16>) {
    let mut bl = buf.len() as i32;
    loop {
        let old_bl = bl;
        bl -= 1;
        if old_bl <= 0 || buf[bl as usize] != b'0' as u16 {
            break;
        }
    }
    buf.truncate((bl + 1) as usize);
}

const DTOA_MODES: [i32; 5] = [0, 0, 3, 2, 2];

// port: DToA#JS_dtostr
// Java-private; public only for the harness and formatting modes.
pub fn js_dtostr(
    buffer: &mut Vec<u16>,
    mut mode: i32,
    precision: i32,
    d: f64,
) -> Result<(), DToAError> {
    let mut sign = false;
    if mode == DTOSTR_FIXED && (d >= 1e21 || d <= -1e21) {
        mode = DTOSTR_STANDARD;
    }
    let dec_pt = js_dtoa(
        d,
        *DTOA_MODES
            .get(mode as usize)
            .ok_or(DToAError::ArrayIndexOutOfBounds {
                index: mode,
                length: DTOA_MODES.len(),
            })?,
        mode >= DTOSTR_FIXED,
        precision,
        &mut sign,
        buffer,
    )?;
    let mut n_digits = buffer.len() as i32;
    if dec_pt != 9999 {
        let mut exponential_notation = false;
        let mut min_n_digits = 0;
        match mode {
            DTOSTR_STANDARD => {
                if dec_pt < -5 || dec_pt > 21 {
                    exponential_notation = true;
                } else {
                    min_n_digits = dec_pt;
                }
            }
            DTOSTR_FIXED => {
                min_n_digits = if precision >= 0 {
                    dec_pt.wrapping_add(precision)
                } else {
                    dec_pt
                };
            }
            DTOSTR_EXPONENTIAL => {
                min_n_digits = precision;
                exponential_notation = true;
            }
            DTOSTR_STANDARD_EXPONENTIAL => {
                exponential_notation = true;
            }
            DTOSTR_PRECISION => {
                min_n_digits = precision;
                if dec_pt < -5 || dec_pt > precision {
                    exponential_notation = true;
                }
            }
            _ => {}
        }
        if n_digits < min_n_digits {
            let p = min_n_digits;
            n_digits = min_n_digits;
            loop {
                buffer.push(b'0' as u16);
                if buffer.len() as i32 == p {
                    break;
                }
            }
        }
        if exponential_notation {
            if n_digits != 1 {
                buffer.insert(1, b'.' as u16);
            }
            buffer.push(b'e' as u16);
            if dec_pt.wrapping_sub(1) >= 0 {
                buffer.push(b'+' as u16);
            }
            buffer.extend((dec_pt.wrapping_sub(1)).to_string().encode_utf16());
        } else if dec_pt != n_digits {
            if dec_pt > 0 {
                buffer.insert(dec_pt as usize, b'.' as u16);
            } else {
                for _ in 0..1i32.wrapping_sub(dec_pt) {
                    buffer.insert(0, b'0' as u16);
                }
                buffer.insert(1, b'.' as u16);
            }
        }
    }
    if sign
        && !(word0(d) == SIGN_BIT && word1(d) == 0)
        && !((word0(d) & EXP_MASK) == EXP_MASK && (word1(d) != 0 || (word0(d) & FRAC_MASK) != 0))
    {
        buffer.insert(0, b'-' as u16);
    }
    Ok(())
}

// port: DToA#numberToString
pub fn number_to_string(value: f64) -> Result<Vec<u16>, DToAError> {
    if value.is_nan() {
        return Ok("NaN".encode_utf16().collect());
    }
    if value == 0.0 {
        return Ok("0".encode_utf16().collect());
    }
    if value < 0.0 {
        let mut result = vec![b'-' as u16];
        result.extend(number_to_string(-value)?);
        return Ok(result);
    }
    if value.is_infinite() {
        return Ok("Infinity".encode_utf16().collect());
    }
    let mut buffer = Vec::new();
    js_dtostr(&mut buffer, DTOSTR_STANDARD, 0, value)?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected strings obtained from the pinned Java harness (picked.tsv.gz).
    #[test]
    fn java_hand_picked_cases() {
        for (bits, expected) in [
            (0x0000000000000000u64, "0"),
            (0x8000000000000000u64, "0"),
            (0x7ff8000000000000u64, "NaN"),
            (0x7ff0000000000000u64, "Infinity"),
            (0xfff0000000000000u64, "-Infinity"),
            (0x444b1ae4d6e2ef50u64, "1e+21"),
            (0x3e7ad7f29abcaf48u64, "1e-7"),
            (0x441ac53a7e04bcdau64, "123456789012345680000"),
            (0x0000000000000001u64, "5e-324"),
            (0x0000000000000001u64, "5e-324"),
            (0x416312d000000000u64, "10000000"),
            (0x3f50624dd2f1a9fcu64, "0.001"),
            (0x3f1a36e2eb1c432du64, "0.0001"),
            (0x44c52d02c7e14af6u64, "2e+23"),
            (0x4340000000000000u64, "9007199254740992"),
            (0x0010000000000000u64, "2.2250738585072014e-308"),
            (0x7fefffffffffffffu64, "1.7976931348623157e+308"),
            (0x44b52d02c7e14af6u64, "1e+23"),
            (0x3fb999999999999au64, "0.1"),
            (0x4004000000000000u64, "2.5"),
        ] {
            assert_eq!(
                number_to_string(f64::from_bits(bits)),
                Ok(expected.encode_utf16().collect()),
                "bits={bits:016x}"
            );
        }
    }
}

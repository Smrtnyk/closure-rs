/*
 * Copyright (c) 1994, 2023, Oracle and/or its affiliates. All rights reserved.
 * DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
 *
 * This code is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 2 only, as
 * published by the Free Software Foundation.  Oracle designates this
 * particular file as subject to the "Classpath" exception as provided
 * by Oracle in the LICENSE file that accompanied this code.
 *
 * This code is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
 * version 2 for more details (a copy is included in the LICENSE file that
 * accompanied this code).
 *
 * You should have received a copy of the GNU General Public License version
 * 2 along with this work; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
 *
 * Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
 * or visit www.oracle.com if you need additional information or have any
 * questions.
 */
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/lang/Math.java.

//! JDK 21 `java.lang.Math` methods whose results reach compiler output.

/// Java's `Math.pow(double, double)`.
///
/// The special cases of Java's specification that differ from C99 `pow` are handled first: a
/// NaN exponent gives NaN (C99: `pow(1, NaN) == 1`), and a base of absolute value 1 with an
/// infinite exponent gives NaN (C99: 1). Every other special case is the same in both. The
/// remaining values come from the platform `pow`, which, like HotSpot's `Math.pow` intrinsic
/// that the Java reference runs with, is exact whenever the result is representable.
// port: Math#pow
pub fn pow(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        return 1.0;
    }
    if b.is_nan() {
        return f64::NAN;
    }
    if a.abs() == 1.0 && b.is_infinite() {
        return f64::NAN;
    }
    a.powf(b)
}

// port: Math#max(double, double)
pub fn max(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        return a; // a is NaN
    }
    if a == 0.0 && b == 0.0 && a.to_bits() == (-0.0f64).to_bits() {
        // Raw conversion ok since NaN can't map to -0.0.
        return b;
    }
    if a >= b { a } else { b }
}

// port: Math#min(double, double)
pub fn min(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        return a; // a is NaN
    }
    if a == 0.0 && b == 0.0 && b.to_bits() == (-0.0f64).to_bits() {
        // Raw conversion ok since NaN can't map to -0.0.
        return b;
    }
    if a <= b { a } else { b }
}

// port: Math#round(double)
pub fn round(a: f64) -> i64 {
    const SIGNIFICAND_WIDTH: i64 = 53;
    const EXP_BIAS: i64 = 1023;
    const EXP_BIT_MASK: i64 = 0x7FF0_0000_0000_0000;
    const SIGNIF_BIT_MASK: i64 = 0x000F_FFFF_FFFF_FFFF;
    let long_bits = a.to_bits() as i64;
    let biased_exp = (long_bits & EXP_BIT_MASK) >> (SIGNIFICAND_WIDTH - 1);
    let shift = (SIGNIFICAND_WIDTH - 2 + EXP_BIAS) - biased_exp;
    if (shift & -64) == 0 {
        // shift >= 0 && shift < 64
        // a is a finite number such that pow(2,-64) <= ulp(a) < 1
        let mut r = (long_bits & SIGNIF_BIT_MASK) | (SIGNIF_BIT_MASK + 1);
        if long_bits < 0 {
            r = -r;
        }
        ((r >> shift) + 1) >> 1
    } else {
        // a is either a finite number with abs(a) < 1/2, or a finite number with ulp(a) >= 1
        // and hence a is a mathematical integer, or an infinity or NaN.
        a as i64
    }
}

// port: Math#signum(double)
pub fn signum(d: f64) -> f64 {
    if d == 0.0 || d.is_nan() {
        d
    } else {
        1.0f64.copysign(d)
    }
}

#[cfg(test)]
mod tests {
    use super::{pow, round, signum};

    #[test]
    fn java_special_cases() {
        assert_eq!(pow(f64::NAN, 0.0), 1.0);
        assert_eq!(pow(f64::NAN, -0.0), 1.0);
        assert!(pow(1.0, f64::NAN).is_nan());
        assert!(pow(1.0, f64::INFINITY).is_nan());
        assert!(pow(-1.0, f64::NEG_INFINITY).is_nan());
        assert!(pow(f64::NAN, 1.0).is_nan());
        assert_eq!(pow(2.0, 3.0), 8.0);
        assert_eq!(pow(2.0, -1.0), 0.5);
        assert_eq!(pow(-2.0, 3.0), -8.0);
        assert_eq!(pow(0.0, -1.0), f64::INFINITY);
        assert_eq!(pow(-0.0, -1.0), f64::NEG_INFINITY);
        assert_eq!(pow(2.0, f64::INFINITY), f64::INFINITY);
        assert_eq!(pow(0.5, f64::INFINITY), 0.0);
    }

    #[test]
    fn round_and_signum_match_java() {
        assert_eq!(round(0.49999999999999994), 0);
        assert_eq!(round(2.5), 3);
        assert_eq!(round(-2.5), -2);
        assert_eq!(round(-0.5), 0);
        assert_eq!(round(f64::NAN), 0);
        assert_eq!(round(1e300), i64::MAX);
        assert_eq!(round(-1e300), i64::MIN);
        assert_eq!(signum(-3.0), -1.0);
        assert!(signum(-0.0).is_sign_negative());
    }
}

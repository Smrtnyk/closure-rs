/*
 * Copyright (c) 1996, 2023, Oracle and/or its affiliates. All rights reserved.
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
/*
 * Portions Copyright IBM Corporation, 2001. All Rights Reserved.
 */
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/math/BigDecimal.java.

use closure_rhino::{java_lang::digit, js_string::JsString};
use num_bigint::{BigInt, Sign};

const INFLATED: i64 = i64::MIN;
const MAX_COMPACT_DIGITS: usize = 18;
const LONG_MASK: i64 = 0xffff_ffff;

// The BigDecimal(String) call in LazilyParsedNumber always uses
// MathContext.UNLIMITED. This subset ports that constructor path; no public
// MathContext API or precision rounding is introduced.
pub(crate) struct BigDecimal {
    scale: i32,
    precision: i32,
    int_compact: i64,
    int_val: Option<BigInt>,
}
impl BigDecimal {
    // port: java.math.BigDecimal#BigDecimal(String)
    pub(crate) fn new(val: &JsString) -> Self {
        Self::new_with_chars(val.as_units(), 0, val.length())
    }

    // port: java.math.BigDecimal#BigDecimal(char[],int,int)
    fn new_with_chars(input: &[u16], mut offset: usize, mut len: usize) -> Self {
        if offset > input.len() || len > input.len() - offset {
            panic!(
                "java.lang.NumberFormatException: Bad offset or len arguments for char[] input."
            );
        }
        let mut prec = 0;
        let mut scl = 0i64;
        let mut rs = 0i64;
        let mut rb = None;
        let mut isneg = false;
        if input[offset] == b'-' as u16 {
            isneg = true;
            offset += 1;
            len -= 1;
        } else if input[offset] == b'+' as u16 {
            offset += 1;
            len -= 1;
        }
        let mut dot = false;
        let is_compact = len <= MAX_COMPACT_DIGITS;
        let mut idx = 0;
        if is_compact {
            while len > 0 {
                let c = input[offset];
                if c == b'0' as u16 {
                    if prec == 0 {
                        prec = 1;
                    } else if rs != 0 {
                        rs *= 10;
                        prec += 1;
                    }
                    if dot {
                        scl += 1;
                    }
                } else if c >= b'1' as u16 && c <= b'9' as u16 {
                    let digit = c - b'0' as u16;
                    if prec != 1 || rs != 0 {
                        prec += 1;
                    }
                    rs = rs * 10 + digit as i64;
                    if dot {
                        scl += 1;
                    }
                } else if c == b'.' as u16 {
                    if dot {
                        panic!(
                            "java.lang.NumberFormatException: Character array contains more than one decimal point."
                        );
                    }
                    dot = true;
                } else if digit(c, 10) >= 0 {
                    let digit = digit(c, 10);
                    if digit == 0 {
                        if prec == 0 {
                            prec = 1;
                        } else if rs != 0 {
                            rs *= 10;
                            prec += 1;
                        }
                    } else {
                        if prec != 1 || rs != 0 {
                            prec += 1;
                        }
                        rs = rs * 10 + digit as i64;
                    }
                    if dot {
                        scl += 1;
                    }
                } else if c == b'e' as u16 || c == b'E' as u16 {
                    scl -= Self::parse_exp(input, offset, len);
                    break;
                } else {
                    crate::JavaException::throw(
                        "java.lang.NumberFormatException: Character ",
                        &JsString::from_units(vec![c]),
                        " is neither a decimal digit number, decimal point, nor \"e\" notation exponential mark.",
                    );
                }
                offset += 1;
                len -= 1;
            }
            if prec == 0 {
                panic!("java.lang.NumberFormatException: No digits found.");
            }
            rs = if isneg { -rs } else { rs };
            // MathContext.UNLIMITED.precision == 0: no rounding/drop branch.
        } else {
            let mut coeff = vec![0u16; len];
            while len > 0 {
                let c = input[offset];
                if (c >= b'0' as u16 && c <= b'9' as u16) || digit(c, 10) >= 0 {
                    if c == b'0' as u16 || digit(c, 10) == 0 {
                        if prec == 0 {
                            coeff[idx] = c;
                            prec = 1;
                        } else if idx != 0 {
                            coeff[idx] = c;
                            idx += 1;
                            prec += 1;
                        }
                    } else {
                        if prec != 1 || idx != 0 {
                            prec += 1;
                        }
                        coeff[idx] = c;
                        idx += 1;
                    }
                    if dot {
                        scl += 1;
                    }
                    offset += 1;
                    len -= 1;
                    continue;
                }
                if c == b'.' as u16 {
                    if dot {
                        panic!(
                            "java.lang.NumberFormatException: Character array contains more than one decimal point."
                        );
                    }
                    dot = true;
                    offset += 1;
                    len -= 1;
                    continue;
                }
                if c != b'e' as u16 && c != b'E' as u16 {
                    panic!(
                        "java.lang.NumberFormatException: Character array is missing \"e\" notation exponential mark."
                    );
                }
                scl -= Self::parse_exp(input, offset, len);
                break;
            }
            if prec == 0 {
                panic!("java.lang.NumberFormatException: No digits found.");
            }
            // BigInteger(char[], sign, precision): num-bigint supplies the
            // arbitrary precision value, with Character.digit on every char.
            let mut unscaled = BigInt::from(0);
            for &c in &coeff[..prec as usize] {
                unscaled = unscaled * 10 + digit(c, 10);
            }
            rb = Some(if isneg { -unscaled } else { unscaled });
            rs = Self::compact_val_for(rb.as_ref().unwrap());
            // MathContext.UNLIMITED.precision == 0: no rounding/drop branch.
        }
        if scl as i32 as i64 != scl {
            panic!("java.lang.NumberFormatException: Exponent overflow.");
        }
        Self {
            scale: scl as i32,
            precision: prec,
            int_compact: rs,
            int_val: rb,
        }
    }

    // port: java.math.BigDecimal#parseExp
    fn parse_exp(input: &[u16], mut offset: usize, mut len: usize) -> i64 {
        let mut exp = 0;
        offset += 1;
        let mut c = input[offset];
        len -= 1;
        let negexp = c == b'-' as u16;
        if negexp || c == b'+' as u16 {
            offset += 1;
            c = input[offset];
            len -= 1;
        }
        if len == 0 {
            panic!("java.lang.NumberFormatException: No exponent digits.");
        }
        while len > 10 && (c == b'0' as u16 || digit(c, 10) == 0) {
            offset += 1;
            c = input[offset];
            len -= 1;
        }
        if len > 10 {
            panic!("java.lang.NumberFormatException: Too many nonzero exponent digits.");
        }
        loop {
            let v;
            if c >= b'0' as u16 && c <= b'9' as u16 {
                v = (c - b'0' as u16) as i32;
            } else {
                v = digit(c, 10);
                if v < 0 {
                    panic!("java.lang.NumberFormatException: Not a digit.");
                }
            }
            exp = exp * 10 + v as i64;
            if len == 1 {
                break;
            }
            offset += 1;
            c = input[offset];
            len -= 1;
        }
        if negexp {
            exp = -exp;
        }
        exp
    }

    // port: java.math.BigDecimal#compactValFor
    fn compact_val_for(b: &BigInt) -> i64 {
        let (sign, mut m) = b.to_u32_digits();
        m.reverse(); // BigInteger.mag is big-endian.
        let len = m.len();
        if len == 0 {
            return 0;
        }
        let d = m[0] as i32;
        if len > 2 || (len == 2 && d < 0) {
            return INFLATED;
        }
        let u = if len == 2 {
            ((m[1] as i64) & LONG_MASK) + ((d as i64) << 32)
        } else {
            (d as i64) & LONG_MASK
        };
        if sign == Sign::Minus { -u } else { u }
    }

    // port: java.math.BigDecimal#signum
    fn signum(&self) -> i32 {
        if self.int_compact != INFLATED {
            self.int_compact.signum() as i32
        } else {
            match self.int_val.as_ref().unwrap().sign() {
                Sign::Minus => -1,
                Sign::NoSign => 0,
                Sign::Plus => 1,
            }
        }
    }

    // port: java.math.BigDecimal#fractionOnly
    fn fraction_only(&self) -> bool {
        // assert this.signum() != 0;
        self.precision <= self.scale
    }

    // port: java.math.BigDecimal#longValue
    fn long_value(&self) -> i64 {
        if self.int_compact != INFLATED && self.scale == 0 {
            self.int_compact
        } else if self.signum() == 0 || self.fraction_only() || self.scale <= -64 {
            0
        } else {
            // setScale(0, ROUND_DOWN).inflated().longValue(), with num-bigint
            // implementing BigInteger arithmetic and two's-complement narrowing.
            let mut value = self
                .int_val
                .clone()
                .unwrap_or_else(|| self.int_compact.into());
            if self.scale > 0 {
                value /= BigInt::from(10).pow(self.scale as u32);
            } else if self.scale < 0 {
                value *= BigInt::from(10).pow(-self.scale as u32);
            }
            let (sign, words) = value.to_u32_digits();
            let low = words.first().copied().unwrap_or(0) as u64
                | ((words.get(1).copied().unwrap_or(0) as u64) << 32);
            if sign == Sign::Minus {
                (low as i64).wrapping_neg()
            } else {
                low as i64
            }
        }
    }

    // port: java.math.BigDecimal#intValue
    pub(crate) fn int_value(&self) -> i32 {
        if self.int_compact != INFLATED && self.scale == 0 {
            self.int_compact as i32
        } else {
            self.long_value() as i32
        }
    }
}

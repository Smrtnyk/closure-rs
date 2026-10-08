/*
 * Copyright (c) 1996, 2022, Oracle and/or its affiliates. All rights reserved.
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
 * Portions Copyright (c) 1995  Colin Plumb.  All rights reserved.
 */
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/math/BigInteger.java.

//! JDK 21 BigInteger(String, int), with num_bigint as the magnitude storage.
use super::{NumberFormatException, digit, parse_int};
use crate::js_string::JsString;
use num_bigint::{BigInt, Sign};
const MAX_MAG_LENGTH: usize = i32::MAX as usize / 32 + 1;
const BITS_PER_DIGIT: [u64; 37] = [
    0, 0, 1024, 1624, 2048, 2378, 2648, 2875, 3072, 3247, 3402, 3543, 3672, 3790, 3899, 4001, 4096,
    4186, 4271, 4350, 4426, 4498, 4567, 4633, 4696, 4756, 4814, 4870, 4923, 4975, 5025, 5074, 5120,
    5166, 5210, 5253, 5295,
];
const DIGITS_PER_INT: [usize; 37] = [
    0, 0, 30, 19, 15, 13, 11, 11, 10, 9, 9, 8, 8, 8, 8, 7, 7, 7, 7, 7, 7, 7, 6, 6, 6, 6, 6, 6, 6,
    6, 6, 6, 6, 6, 6, 6, 5,
];
const INT_RADIX: [i32; 37] = [
    0, 0, 0x40000000, 0x4546b3db, 0x40000000, 0x48c27395, 0x159fd800, 0x75db9c97, 0x40000000,
    0x17179149, 0x3b9aca00, 0xcc6db61, 0x19a10000, 0x309f1021, 0x57f6c100, 0xa2f1b6f, 0x10000000,
    0x18754571, 0x247dbc80, 0x3547667b, 0x4c4b4000, 0x6b5a6e1d, 0x6c20a40, 0x8d2d931, 0xb640000,
    0xe8d4a51, 0x1269ae40, 0x17179149, 0x1cb91000, 0x23744899, 0x2b73a840, 0x34e63b41, 0x40000000,
    0x4cfa3cc1, 0x5c13d840, 0x6d91b519, 0x39aa400,
];
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BigIntegerParseError {
    NumberFormatException(NumberFormatException),
    ArithmeticException(&'static str),
}
impl From<NumberFormatException> for BigIntegerParseError {
    fn from(error: NumberFormatException) -> Self {
        Self::NumberFormatException(error)
    }
}
impl BigIntegerParseError {
    pub fn class(&self) -> &'static str {
        match self {
            Self::NumberFormatException(_) => "java.lang.NumberFormatException",
            Self::ArithmeticException(_) => "java.lang.ArithmeticException",
        }
    }
    pub fn message(self) -> JsString {
        match self {
            Self::NumberFormatException(error) => JsString::from_units(error.message),
            Self::ArithmeticException(message) => JsString::from(message),
        }
    }
}
fn failure(message: &str) -> BigIntegerParseError {
    NumberFormatException {
        message: message.encode_utf16().collect(),
    }
    .into()
}
// port: BigInteger#BigInteger(String, int)
pub fn parse_big_integer(val: &JsString, radix: i32) -> Result<BigInt, BigIntegerParseError> {
    let mut cursor = 0;
    let len = val.length();
    if !(2..=36).contains(&radix) {
        return Err(failure("Radix out of range"));
    }
    if len == 0 {
        return Err(failure("Zero length BigInteger"));
    }
    let mut sign = 1;
    let index1 = val.last_index_of_char(45);
    let index2 = val.last_index_of_char(43);
    if index1 >= 0 {
        if index1 != 0 || index2 >= 0 {
            return Err(failure("Illegal embedded sign character"));
        }
        sign = -1;
        cursor = 1;
    } else if index2 >= 0 {
        if index2 != 0 {
            return Err(failure("Illegal embedded sign character"));
        }
        cursor = 1;
    }
    if cursor == len {
        return Err(failure("Zero length BigInteger"));
    }
    while cursor < len && digit(val.char_at(cursor), radix) == 0 {
        cursor += 1;
    }
    if cursor == len {
        return Ok(BigInt::from(0));
    }
    let num_digits = len - cursor;
    let radix_index = radix as usize;
    let num_bits = ((num_digits as u64 * BITS_PER_DIGIT[radix_index]) >> 10) + 1;
    if num_bits + 31 >= 1 << 32 {
        return Err(report_overflow());
    }
    let num_words = ((num_bits + 31) >> 5) as usize;
    let mut magnitude = vec![0; num_words];
    let mut first_group_len = num_digits % DIGITS_PER_INT[radix_index];
    if first_group_len == 0 {
        first_group_len = DIGITS_PER_INT[radix_index];
    }
    let group = &val.as_units()[cursor..cursor + first_group_len];
    cursor += first_group_len;
    let first = parse_int(group, radix)?;
    if first < 0 {
        return Err(failure("Illegal digit"));
    }
    magnitude[num_words - 1] = first as u32;
    let super_radix = INT_RADIX[radix_index];
    while cursor < len {
        let group = &val.as_units()[cursor..cursor + DIGITS_PER_INT[radix_index]];
        cursor += DIGITS_PER_INT[radix_index];
        let group_val = parse_int(group, radix)?;
        if group_val < 0 {
            return Err(failure("Illegal digit"));
        }
        destructive_mul_add(&mut magnitude, super_radix, group_val);
    }
    let magnitude = trusted_strip_leading_zero_ints(magnitude);
    if magnitude.len() >= MAX_MAG_LENGTH {
        check_range(&magnitude)?;
    }
    let sign = if sign < 0 { Sign::Minus } else { Sign::Plus };
    Ok(BigInt::new(sign, magnitude.into_iter().rev().collect()))
}
// port: BigInteger#destructiveMulAdd
fn destructive_mul_add(x: &mut [u32], y: i32, z: i32) {
    let ylong = y as u32 as u64;
    let zlong = z as u32 as u64;
    let len = x.len();
    let mut carry = 0;
    for i in (0..len).rev() {
        let product = ylong * u64::from(x[i]) + carry;
        x[i] = product as u32;
        carry = product >> 32;
    }
    let sum = u64::from(x[len - 1]) + zlong;
    x[len - 1] = sum as u32;
    carry = sum >> 32;
    for i in (0..len - 1).rev() {
        let sum = u64::from(x[i]) + carry;
        x[i] = sum as u32;
        carry = sum >> 32;
    }
}
// port: BigInteger#trustedStripLeadingZeroInts
fn trusted_strip_leading_zero_ints(val: Vec<u32>) -> Vec<u32> {
    let mut keep = 0;
    while keep < val.len() && val[keep] == 0 {
        keep += 1;
    }
    if keep == 0 { val } else { val[keep..].to_vec() }
}
// port: BigInteger#checkRange
fn check_range(mag: &[u32]) -> Result<(), BigIntegerParseError> {
    if mag.len() > MAX_MAG_LENGTH || (mag.len() == MAX_MAG_LENGTH && (mag[0] as i32) < 0) {
        return Err(report_overflow());
    }
    Ok(())
}
// port: BigInteger#reportOverflow
fn report_overflow() -> BigIntegerParseError {
    BigIntegerParseError::ArithmeticException("BigInteger would overflow supported range")
}
/// Java's `ArithmeticException` thrown by a BigInteger operation, with its message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArithmeticException(pub &'static str);

/// `BigInteger#mod(BigInteger)`: the non-negative residue; throws for a modulus that is not
/// positive.
// port: BigInteger#mod
pub fn r#mod(this: &BigInt, m: &BigInt) -> Result<BigInt, ArithmeticException> {
    if m.sign() != Sign::Plus {
        return Err(ArithmeticException("BigInteger: modulus not positive"));
    }

    let result = this % m;
    Ok(if result.sign() != Sign::Minus {
        result
    } else {
        result + m
    })
}

/// `BigInteger#intValueExact()`.
// port: BigInteger#intValueExact
pub fn int_value_exact(this: &BigInt) -> Result<i32, ArithmeticException> {
    i32::try_from(this).map_err(|_| ArithmeticException("BigInteger out of int range"))
}

/// `BigInteger#pow(int)`, with Java's exceptions: a negative exponent, and a result outside the
/// supported range (checked where Java checks it: the shifted powers of two, the size estimate
/// of the large-number algorithm, and the magnitude of the result).
// port: BigInteger#pow
pub fn pow(this: &BigInt, exponent: i32) -> Result<BigInt, ArithmeticException> {
    if exponent < 0 {
        return Err(ArithmeticException("Negative exponent"));
    }
    if this.sign() == Sign::NoSign {
        return Ok(if exponent == 0 {
            BigInt::from(1)
        } else {
            this.clone()
        });
    }

    let part_to_square = this.magnitude();
    let powers_of_two = part_to_square.trailing_zeros().unwrap_or(0);
    let bits_to_shift_long = powers_of_two as i64 * exponent as i64;
    if bits_to_shift_long > i32::MAX as i64 {
        return Err(ARITHMETIC_OVERFLOW);
    }
    let shifted = part_to_square >> powers_of_two;
    let remaining_bits = shifted.bits();
    let scale_factor = remaining_bits as i64 * exponent as i64;
    if remaining_bits != 1 && !(shifted.bits() <= 32 && scale_factor <= 62) {
        // The large-number algorithm.
        if this.bits() as i64 * exponent as i64 / 32 > MAX_MAG_LENGTH as i64 {
            return Err(ARITHMETIC_OVERFLOW);
        }
    }
    let result = this.pow(exponent as u32);
    let mag_len = result.magnitude().bits().div_ceil(32) as usize;
    if mag_len > MAX_MAG_LENGTH
        || (mag_len == MAX_MAG_LENGTH && result.magnitude().bits().is_multiple_of(32))
    {
        return Err(ARITHMETIC_OVERFLOW);
    }
    Ok(result)
}
const REPORT_OVERFLOW_MESSAGE: &str = "BigInteger would overflow supported range";
const ARITHMETIC_OVERFLOW: ArithmeticException = ArithmeticException(REPORT_OVERFLOW_MESSAGE);
/// JDK 21 `BigInteger.doubleValue()`: the value rounded half-even to the nearest double.
// port: java.math.BigInteger#doubleValue
pub fn double_value(this: &BigInt) -> f64 {
    let (signum, le_digits) = this.to_u32_digits();
    if signum == Sign::NoSign {
        return 0.0;
    }
    // Java's big-endian `mag`.
    let mag: Vec<u32> = le_digits.iter().rev().copied().collect();

    let exponent = (((mag.len() - 1) << 5) as i32) + (32 - mag[0].leading_zeros() as i32) - 1;

    // exponent == floor(log2(abs(this))Double)
    if exponent < 64 - 1 {
        // longValue(): the magnitude fits in 63 bits.
        let mut magnitude: u64 = 0;
        for &d in mag.iter() {
            magnitude = (magnitude << 32) | u64::from(d);
        }
        let long_value = if signum == Sign::Minus {
            (magnitude as i64).wrapping_neg()
        } else {
            magnitude as i64
        };
        return long_value as f64;
    } else if exponent > 1023 {
        return if signum == Sign::Plus {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };
    }

    // DoubleConsts.SIGNIFICAND_WIDTH
    const SIGNIFICAND_WIDTH: i32 = 53;
    const SIGNIF_BIT_MASK: i64 = 0x000F_FFFF_FFFF_FFFF;
    const EXP_BIAS: i32 = 1023;
    let shift = exponent - SIGNIFICAND_WIDTH;

    let n_bits = shift & 0x1f;
    let n_bits2 = 32 - n_bits;

    let mut high_bits: u32;
    let mut low_bits: u32;
    if n_bits == 0 {
        high_bits = mag[0];
        low_bits = mag[1];
    } else {
        high_bits = mag[0] >> n_bits;
        low_bits = (mag[0] << n_bits2) | (mag[1] >> n_bits);
        if high_bits == 0 {
            high_bits = low_bits;
            low_bits = (mag[1] << n_bits2) | (mag[2] >> n_bits);
        }
    }

    let twice_signif_floor: i64 = ((i64::from(high_bits)) << 32) | i64::from(low_bits);

    let mut signif_floor = twice_signif_floor >> 1;
    signif_floor &= SIGNIF_BIT_MASK; // remove the implied bit

    // abs().getLowestSetBit()
    let lowest_set_bit = this.trailing_zeros().unwrap() as i64;
    let increment = (twice_signif_floor & 1) != 0
        && ((signif_floor & 1) != 0 || lowest_set_bit < i64::from(shift));
    let signif_rounded = if increment {
        signif_floor + 1
    } else {
        signif_floor
    };
    let mut bits: i64 = i64::from(exponent + EXP_BIAS) << (SIGNIFICAND_WIDTH - 1);
    bits += signif_rounded;
    if signum == Sign::Minus {
        bits |= i64::MIN; // signum & DoubleConsts.SIGN_BIT_MASK
    }
    f64::from_bits(bits as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn string_constructor() {
        for (s, radix, expected) in [
            ("+000", 10, "0"),
            ("-ff", 16, "-255"),
            ("18446744073709551616", 10, "18446744073709551616"),
            ("１２３", 10, "123"),
            ("101011", 2, "43"),
        ] {
            assert_eq!(
                parse_big_integer(&s.into(), radix).unwrap().to_string(),
                expected
            );
        }
        for (s, radix, message) in [
            ("", 10, "Zero length BigInteger"),
            ("-", 10, "Zero length BigInteger"),
            ("1-1", 10, "Illegal embedded sign character"),
            ("1", 1, "Radix out of range"),
            ("2", 2, "For input string: \"2\" under radix 2"),
        ] {
            assert_eq!(
                parse_big_integer(&s.into(), radix).unwrap_err().message(),
                message
            );
        }
    }

    #[test]
    fn double_value_rounds_half_even() {
        for (s, radix, expected) in [
            ("0", 10, 0.0),
            ("123", 10, 123.0),
            ("9007199254740993", 10, 9007199254740992.0),
            ("9007199254740995", 10, 9007199254740996.0),
            ("18446744073709551617", 10, 1.8446744073709552e19),
            ("1fffffffffffff9", 16, 1.4411518807585587e17),
            ("ffffffffffffffffffffffff", 16, 7.922816251426434e28),
        ] {
            assert_eq!(
                double_value(&parse_big_integer(&s.into(), radix).unwrap()),
                expected
            );
        }
        let huge = "1".repeat(400);
        assert_eq!(
            double_value(&parse_big_integer(&huge.as_str().into(), 10).unwrap()),
            f64::INFINITY
        );
    }
}

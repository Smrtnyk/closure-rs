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

//! `java.math.BigInteger` arithmetic for `DToA`: the overflow checks Java's
//! `BigInteger` makes (`checkRange`, `multiply`, `pow`, `shiftLeft`) and `intValue`.

use super::DToAError;
use num_bigint::{BigInt, Sign};

const MAX_MAG_LENGTH: u64 = i32::MAX as u64 / 32 + 1;
const MAX_BIT_LENGTH: u64 = 32 * MAX_MAG_LENGTH - 1;

// port: BigInteger#reportOverflow
fn report_overflow<T>() -> Result<T, DToAError> {
    Err(DToAError::BigIntegerOverflow)
}

// port: BigInteger#checkRange
fn check_range(bits: u64) -> Result<(), DToAError> {
    // mag.length > MAX_MAG_LENGTH || (mag.length == MAX_MAG_LENGTH && mag[0] < 0).
    if bits > MAX_BIT_LENGTH {
        report_overflow()
    } else {
        Ok(())
    }
}

// port: BigInteger#shiftLeft
pub(super) fn shift_left(b: BigInt, n: i32) -> Result<BigInt, DToAError> {
    if b.sign() == Sign::NoSign || n == 0 {
        return Ok(b);
    }
    if n > 0 {
        check_range(b.bits() + n as u64)?;
        Ok(b << n as usize)
    } else {
        Ok(b >> n.unsigned_abs() as usize)
    }
}

// port: BigInteger#multiply
fn check_multiply(xbits: u64, ybits: u64) -> Result<(), DToAError> {
    let xlen = xbits.div_ceil(32);
    let ylen = ybits.div_ceil(32);
    // Java's top-level Toom-Cook pre-check; small operands use multiplyByInt/multiplyToLen.
    if xlen >= 80
        && ylen >= 80
        && (xlen >= 240 || ylen >= 240)
        && xbits + ybits > 32 * MAX_MAG_LENGTH
    {
        report_overflow()
    } else {
        Ok(())
    }
}

// port: BigInteger#multiplyToLen
fn product_bit_length(x: &BigInt, y: &BigInt) -> u64 {
    let (xbits, ybits) = (x.bits(), y.bits());
    if xbits == 0 || ybits == 0 {
        return 0;
    }
    let bits = xbits + ybits;
    // A product has bits or bits-1 magnitude bits. Compare exact leading-bit intervals
    // against 2^(bits-1), extending the prefixes only if the interval straddles it.
    let mut precision = 64u64;
    loop {
        let xs = xbits.saturating_sub(precision);
        let ys = ybits.saturating_sub(precision);
        let xp = x.magnitude() >> xs as usize;
        let yp = y.magnitude() >> ys as usize;
        let threshold = num_bigint::BigUint::from(1u32) << (bits - 1 - xs - ys) as usize;
        let lower = &xp * &yp;
        if lower >= threshold {
            return bits;
        }
        let upper = (xp + u32::from(xs != 0)) * (yp + u32::from(ys != 0));
        if upper <= threshold {
            return bits - 1;
        }
        precision *= 2;
    }
}

// port: BigInteger#multiply
pub(super) fn multiply(x: BigInt, y: &BigInt) -> Result<BigInt, DToAError> {
    if x.sign() == Sign::NoSign || y.sign() == Sign::NoSign {
        return Ok(BigInt::from(0));
    }
    check_multiply(x.bits(), y.bits())?;
    if x.bits() + y.bits() > MAX_BIT_LENGTH {
        check_range(product_bit_length(&x, y))?;
    }
    Ok(x * y)
}

// port: BigInteger#square
fn square(x: BigInt) -> Result<BigInt, DToAError> {
    if x.bits().div_ceil(32) >= 216 && x.bits() > 16 * MAX_MAG_LENGTH {
        return report_overflow();
    }
    if 2 * x.bits() > MAX_BIT_LENGTH {
        check_range(product_bit_length(&x, &x))?;
    }
    Ok(&x * &x)
}

// port: BigInteger#subtract
pub(super) fn subtract(x: &BigInt, y: &BigInt) -> BigInt {
    // JS_dtoa only subtracts the positive margins S - mhi. Magnitude subtraction
    // cannot grow either operand; Java's constructor check therefore cannot overflow.
    x - y
}

// port: BigInteger#pow
fn check_pow5(exponent: i32) -> Result<(), DToAError> {
    if exponent < 0 {
        return Err(DToAError::NegativeExponent);
    }
    // 5 has no powers of two, remainingBits=3, and mag.length=1.
    if 3 * exponent as u64 / 32 > MAX_MAG_LENGTH {
        report_overflow()
    } else {
        Ok(())
    }
}

// port: BigInteger#pow
pub(super) fn pow5(exponent: i32) -> Result<BigInt, DToAError> {
    check_pow5(exponent)?;
    let mut answer = BigInt::from(1);
    let mut part_to_square = BigInt::from(5);
    let mut working_exponent = exponent as u32;
    while working_exponent != 0 {
        if working_exponent & 1 != 0 {
            answer = multiply(answer, &part_to_square)?;
        }
        working_exponent >>= 1;
        if working_exponent != 0 {
            part_to_square = square(part_to_square)?;
        }
    }
    Ok(answer)
}

// port: BigInteger#bitLength
fn log2_interval(value: &BigInt, fractional_bits: usize) -> (BigInt, BigInt) {
    let integer = value.bits() - 1;
    if value.trailing_zeros() == Some(integer) {
        let exact = BigInt::from(integer) << fractional_bits;
        return (exact.clone(), exact);
    }
    let mut precision = fractional_bits + 128;
    loop {
        let unit = BigInt::from(1) << precision;
        let mut lo = value << (precision - integer as usize);
        let mut hi = lo.clone();
        let mut fraction = BigInt::from(0);
        let mut ambiguous = false;
        for _ in 0..fractional_bits {
            lo = (&lo * &lo) >> precision;
            hi = ((&hi * &hi) + &unit - 1u32) >> precision;
            fraction <<= 1;
            if lo >= (&unit << 1) {
                lo >>= 1;
                hi = (hi + 1u32) >> 1;
                fraction += 1u32;
            } else if hi >= (&unit << 1) {
                ambiguous = true;
                break;
            }
        }
        if !ambiguous {
            let lower = (BigInt::from(integer) << fractional_bits) + fraction;
            return (lower.clone(), lower + 1u32);
        }
        precision += 128;
    }
}

// port: BigInteger#bitLength
fn scaled_bit_length(seed: &BigInt, exponent: i32) -> u64 {
    if exponent == 0 {
        return seed.bits();
    }
    // Compute bitLength(seed * 5^exponent) without constructing the power. Exact
    // rational bounds for log2 are refined until both endpoints have the same floor.
    let mut fractional_bits = 96;
    loop {
        let (seed_lo, seed_hi) = log2_interval(seed, fractional_bits);
        let (five_lo, five_hi) = log2_interval(&BigInt::from(5), fractional_bits);
        let lo = (seed_lo + five_lo * exponent) >> fractional_bits;
        let hi = (seed_hi + five_hi * exponent) >> fractional_bits;
        if lo == hi {
            return lo.iter_u32_digits().next().unwrap_or(0) as u64 + 1;
        }
        fractional_bits += 64;
    }
}

// port: BigInteger#checkRange
#[allow(clippy::too_many_arguments)]
pub(super) fn check_initial_ranges(
    b: &BigInt,
    b5: i32,
    s5: i32,
    m5: i32,
    b2: i32,
    s2: i32,
    m2: i32,
    leftright: bool,
    spec_case: bool,
    use_margins: bool,
) -> Result<(), DToAError> {
    // Anticipate the constructor checks before allocating powers which would fit
    // by themselves but overflow at the following shifts. Preserve pow-call order.
    if b5 > 0 {
        if leftright {
            if m5 > 0 {
                check_pow5(m5)?;
            }
            let j = b5.wrapping_sub(m5);
            if j != 0 {
                check_pow5(j)?;
            }
        } else {
            check_pow5(b5)?;
        }
    }
    if s5 > 0 {
        check_pow5(s5)?;
    }
    let b_upper = b.bits() + 3 * b5.max(0) as u64;
    let s_upper = 1 + 3 * s5.max(0) as u64;
    let m_exponent = if leftright && b5 > 0 { m5.max(0) } else { 0 };
    let m_upper = 1 + 3 * m_exponent as u64;
    if b_upper + b2.max(0) as u64 + 33 <= MAX_BIT_LENGTH
        && s_upper + s2.max(0) as u64 + 33 <= MAX_BIT_LENGTH
        && (!use_margins || m_upper + m2.max(0) as u64 + 34 <= MAX_BIT_LENGTH)
    {
        return Ok(());
    }
    let one = BigInt::from(1);
    let b_bits = scaled_bit_length(b, b5.max(0));
    let s_bits = scaled_bit_length(&one, s5.max(0));
    let m_bits = if leftright {
        scaled_bit_length(&one, m_exponent)
    } else {
        0
    };
    check_range(b_bits)?;
    check_range(s_bits)?;
    check_range(m_bits)?;
    if b5 > 0 && leftright && b5.wrapping_sub(m5) > 0 {
        let first_bits = scaled_bit_length(b, m5.max(0));
        let power_bits = scaled_bit_length(&one, b5.wrapping_sub(m5));
        check_multiply(first_bits, power_bits)?;
    }
    let extra = i32::from(spec_case);
    // S.toByteArray's first four bytes (including a possible sign byte).
    let hi_bits = if s5 != 0 { 24 + (s_bits % 8) as i32 } else { 1 };
    let mut i = hi_bits.wrapping_add(s2).wrapping_add(extra) & 31;
    if i != 0 {
        i = 32 - i;
    }
    let adjustment = if i > 4 {
        i - 4
    } else if i < 4 {
        i + 28
    } else {
        0
    };
    let b_shift = b2.wrapping_add(extra).wrapping_add(adjustment);
    let s_shift = s2.wrapping_add(extra).wrapping_add(adjustment);
    let m_shift = m2.wrapping_add(adjustment);
    if b_shift > 0 {
        check_range(b_bits + b_shift as u64)?;
    }
    if s_shift > 0 {
        check_range(s_bits + s_shift as u64)?;
    }
    if use_margins && m_shift > 0 {
        check_range(m_bits + m_shift as u64)?;
    }
    if use_margins && spec_case {
        check_range(m_bits + m_shift.max(0) as u64 + 1)?;
    }
    Ok(())
}

// port: BigInteger#intValue
pub(super) fn int_value(b: &BigInt) -> i32 {
    let (sign, digits) = b.to_u32_digits();
    let low = digits.first().copied().unwrap_or(0) as i32;
    if sign == Sign::Minus {
        low.wrapping_neg()
    } else {
        low
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dtoa::d_to_a::number_to_string;

    #[test]
    fn java_big_integer_range_boundaries() {
        assert_eq!(check_range(2_147_483_647), Ok(()));
        assert_eq!(
            check_range(2_147_483_648),
            Err(DToAError::BigIntegerOverflow)
        );
        assert_eq!(check_pow5(715_827_893), Ok(()));
        assert_eq!(check_pow5(715_827_894), Err(DToAError::BigIntegerOverflow));
        assert_eq!(pow5(-1), Err(DToAError::NegativeExponent));
        assert_eq!(check_multiply(1_073_741_824, 1_073_741_824), Ok(()));
        assert_eq!(
            check_multiply(1_073_741_824, 1_073_741_825),
            Err(DToAError::BigIntegerOverflow)
        );
        // The pinned estimate is k=650014105 for this low-word subnormal. pow
        // fits, but S.shiftLeft exceeds MAX_BIT_LENGTH; no giant power is allocated.
        assert_eq!(
            number_to_string(f64::from_bits(0x0000000042e886ef)),
            Err(DToAError::BigIntegerOverflow)
        );
        // pow(650000000) fits; the following S.shiftLeft(s2) exceeds checkRange.
        assert_eq!(
            check_initial_ranges(
                &BigInt::from(1),
                0,
                650_000_000,
                0,
                32,
                650_000_000,
                0,
                false,
                false,
                false
            ),
            Err(DToAError::BigIntegerOverflow)
        );
    }

    #[test]
    fn range_preflight_bit_lengths_are_exact() {
        for seed in [1u64, 3, 5, 255, 65535, (1 << 53) - 1] {
            for exponent in [0, 1, 2, 13, 31, 100, 1000, 20000] {
                let seed = BigInt::from(seed);
                let value = &seed * BigInt::from(5).pow(exponent as u32);
                assert_eq!(scaled_bit_length(&seed, exponent), value.bits());
                assert_eq!(product_bit_length(&seed, &value), (&seed * &value).bits());
            }
        }
    }
}

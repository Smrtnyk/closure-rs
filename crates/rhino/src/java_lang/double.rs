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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/lang/Double.java,
//   java.base/jdk/internal/math/FloatingDecimal.java.

//! JDK 21 Double.parseDouble lexical rules and IEEE round-to-nearest conversions.
use super::{NumberFormatException, trim};
use crate::js_string::JsString;
use num_bigint::BigUint;
fn failure(message: JsString) -> NumberFormatException {
    NumberFormatException {
        message: message.as_units().to_vec(),
    }
}
// port: Double#parseDouble
pub fn parse_double(s: &JsString) -> Result<f64, NumberFormatException> {
    read_java_format_string(s)
}
// port: FloatingDecimal#readJavaFormatString
fn read_java_format_string(s: &JsString) -> Result<f64, NumberFormatException> {
    let input = trim(s);
    let len = input.length();
    if len == 0 {
        return Err(failure("empty String".into()));
    }
    let bad = || {
        failure(
            JsString::from("For input string: \"")
                .concat(&input)
                .concat(&JsString::from("\"")),
        )
    };
    let mut i = 0;
    let negative = input.char_at(0) == 45;
    if matches!(input.char_at(0), 43 | 45) {
        i += 1;
    }
    if i == len {
        return Err(bad());
    }
    let rest = input.substring_from(i);
    if rest == "NaN" {
        return Ok(f64::NAN);
    }
    if rest == "Infinity" {
        return Ok(if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }
    if rest.length() > 1 && rest.char_at(0) == 48 && matches!(rest.char_at(1), 120 | 88) {
        return parse_hex_string(&input).ok_or_else(bad);
    }
    let mut dec_seen = false;
    let mut digits = 0;
    while i < len {
        let c = input.char_at(i);
        if (48..=57).contains(&c) {
            digits += 1;
        } else if c == 46 {
            if dec_seen {
                return Err(failure("multiple points".into()));
            }
            dec_seen = true;
        } else {
            break;
        }
        i += 1;
    }
    if digits == 0 {
        return Err(bad());
    }
    if i < len && matches!(input.char_at(i), 101 | 69) {
        i += 1;
        if i < len && matches!(input.char_at(i), 43 | 45) {
            i += 1;
        }
        let exp_at = i;
        while i < len && (48..=57).contains(&input.char_at(i)) {
            i += 1;
        }
        if i == exp_at {
            return Err(bad());
        }
    }
    let end = i;
    if i < len && (i != len - 1 || !matches!(input.char_at(i), 102 | 70 | 100 | 68)) {
        return Err(bad());
    }
    // Both Java's ASCIIToBinaryBuffer and Rust's decimal primitive implement correctly rounded
    // IEEE conversion. The JDK lexical validation above excludes Rust-only spellings.
    // The lexical checks above admit only ASCII signs, digits, a dot and exponent letters.
    input
        .substring(0, end)
        .to_string_lossy()
        .parse::<f64>()
        .map_err(|_| bad())
}
// port: FloatingDecimal#parseHexString
fn parse_hex_string(input: &JsString) -> Option<f64> {
    let mut s = String::from_utf16(input.as_units()).ok()?;
    if s.ends_with(['f', 'F', 'd', 'D']) {
        s.pop();
    }
    let negative = s.starts_with('-');
    let signless = s.strip_prefix(['+', '-']).unwrap_or(&s);
    let rest = signless
        .strip_prefix("0x")
        .or_else(|| signless.strip_prefix("0X"))?;
    let p = rest.find(['p', 'P'])?;
    let significand = &rest[..p];
    let exp_text = &rest[p + 1..];
    let exp_signless = exp_text.strip_prefix(['+', '-']).unwrap_or(exp_text);
    if exp_signless.is_empty() || !exp_signless.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let exp = exp_text
        .parse::<i64>()
        .unwrap_or(if exp_text.starts_with('-') {
            i64::MIN / 2
        } else {
            i64::MAX / 2
        });
    let mut digits = String::new();
    let mut fraction_digits = 0i64;
    let mut point = false;
    for c in significand.chars() {
        if c == '.' {
            if point {
                return None;
            }
            point = true;
        } else if c.is_ascii_hexdigit() {
            digits.push(c);
            if point {
                fraction_digits += 1;
            }
        } else {
            return None;
        }
    }
    if digits.is_empty() {
        return None;
    }
    let magnitude = BigUint::parse_bytes(digits.as_bytes(), 16)?;
    let value = hex_double(
        magnitude,
        exp.saturating_sub(fraction_digits.saturating_mul(4)),
    );
    Some(if negative { -value } else { value })
}
// port: FloatingDecimal#parseHexString (rounding and exponent assembly)
fn hex_double(magnitude: BigUint, exponent: i64) -> f64 {
    let bits = magnitude.bits() as i64;
    if bits == 0 {
        return 0.0;
    }
    let mut top = bits - 1 + exponent;
    if top > 1023 {
        return f64::INFINITY;
    }
    if top < -1075 {
        return 0.0;
    }
    let subnormal = top < -1022;
    let shift = if subnormal {
        -1074 - exponent
    } else {
        bits - 53
    };
    let mut rounded = if shift > 0 {
        let mut q = &magnitude >> shift as usize;
        let remainder = &magnitude - (&q << shift as usize);
        let half = BigUint::from(1u8) << (shift - 1) as usize;
        if remainder > half || (remainder == half && q.bit(0)) {
            q += 1u8;
        }
        q
    } else {
        magnitude << (-shift) as usize
    };
    if subnormal {
        return f64::from_bits(rounded.to_u64_digits().first().copied().unwrap_or(0));
    }
    if rounded.bits() > 53 {
        rounded >>= 1usize;
        top += 1;
    }
    if top > 1023 {
        return f64::INFINITY;
    }
    let significand = rounded.to_u64_digits()[0];
    f64::from_bits(((top + 1023) as u64) << 52 | (significand & ((1u64 << 52) - 1)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parsing() {
        for (s, value) in [
            ("\t -0.0 \r", -0.0),
            (".1", 0.1),
            ("1e309", f64::INFINITY),
            ("0x1.fffffffffffffp1023", f64::MAX),
            ("0x1p-1074", f64::from_bits(1)),
            ("0x1p-1075", 0.0),
            ("0x3p-1075", f64::from_bits(2)),
            ("1.5f", 1.5),
        ] {
            assert_eq!(
                parse_double(&s.into()).unwrap().to_bits(),
                value.to_bits(),
                "{s}"
            );
        }
        assert!(parse_double(&"+NaN".into()).unwrap().is_nan());
        for s in ["inf", "infinity", "nan", "1_0", "0x1", "0x1p", "1e+", "  "] {
            assert!(parse_double(&s.into()).is_err(), "{s}");
        }
    }
}

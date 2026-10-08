/*
 * Copyright (c) 2003, 2023, Oracle and/or its affiliates. All rights reserved.
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
 * Copyright (c) 2023, Oracle and/or its affiliates. All rights reserved.
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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/util/Formatter.java,
//   java.base/jdk/internal/math/FormattedFPDecimal.java.

//! JDK 21 Formatter's %.1f conversion, using the shared Schubfach renderer.
// port: Formatter.FormatSpecifier#print(double,Locale)
pub fn format_one_decimal(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    if value.is_infinite() {
        return format!("{sign}Infinity");
    }
    let decimal = super::double_to_string(value.abs());
    let (mantissa, exponent) = decimal
        .split_once('E')
        .map_or((decimal.as_str(), 0), |(m, e)| {
            (m, e.parse::<i32>().unwrap())
        });
    let fraction = mantissa.split_once('.').map_or(0, |(_, f)| f.len() as i32);
    let mut digits: Vec<u8> = mantissa
        .bytes()
        .filter(|&c| c != b'.')
        .map(|c| c - b'0')
        .collect();
    let mut e = exponent - fraction;
    while digits.len() > 1 && digits.last() == Some(&0) {
        digits.pop();
        e += 1;
    }
    // FormattedFPDecimal.plain(1) rounds the decimal selected by DoubleToDecimal,
    // rather than the exact binary double. HALF_UP retains ties away from zero.
    let p = digits.len() as i32 + e + 1;
    round(&mut digits, p);
    let mut scaled = if p <= 0 {
        digits
    } else {
        while digits.len() < p as usize {
            digits.push(0);
        }
        digits
    };
    while scaled.len() < 2 {
        scaled.insert(0, 0);
    }
    let last = scaled.pop().unwrap();
    let integer: String = scaled.iter().map(|d| (b'0' + d) as char).collect();
    format!("{sign}{integer}.{last}")
}
// port: FormattedFPDecimal#round
fn round(digits: &mut Vec<u8>, p: i32) {
    if p < 0 {
        *digits = vec![0];
        return;
    }
    if p as usize >= digits.len() {
        return;
    }
    let up = digits[p as usize] >= 5;
    digits.truncate(p as usize);
    if up {
        let mut i = digits.len();
        while i > 0 && digits[i - 1] == 9 {
            digits[i - 1] = 0;
            i -= 1;
        }
        if i == 0 {
            digits.insert(0, 1);
        } else {
            digits[i - 1] += 1;
        }
    }
    if digits.is_empty() {
        digits.push(0);
    }
}

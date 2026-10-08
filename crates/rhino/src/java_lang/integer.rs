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
/*
 * Copyright (c) 1994, 2020, Oracle and/or its affiliates. All rights reserved.
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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/lang/Integer.java,
//   java.base/java/lang/NumberFormatException.java.

//! JDK 21 `java.lang.Integer` string conversions whose results reach output.

use super::character_bmp::{MAX_RADIX, MIN_RADIX, digit};

/// `java.lang.NumberFormatException`; the message is the JDK text, UTF-16 because it embeds the
/// input string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumberFormatException {
    pub message: Vec<u16>,
}

impl NumberFormatException {
    // port: NumberFormatException#forInputString
    fn for_input_string(s: &[u16], radix: i32) -> NumberFormatException {
        let mut message: Vec<u16> = "For input string: \"".encode_utf16().collect();
        message.extend_from_slice(s);
        message.push(b'"' as u16);
        if radix != 10 {
            message.extend(format!(" under radix {radix}").encode_utf16());
        }
        NumberFormatException { message }
    }
}

// port: Integer#parseInt(String, int)
pub fn parse_int(s: &[u16], radix: i32) -> Result<i32, NumberFormatException> {
    if radix < MIN_RADIX {
        return Err(NumberFormatException {
            message: format!("radix {radix} less than Character.MIN_RADIX")
                .encode_utf16()
                .collect(),
        });
    }

    if radix > MAX_RADIX {
        return Err(NumberFormatException {
            message: format!("radix {radix} greater than Character.MAX_RADIX")
                .encode_utf16()
                .collect(),
        });
    }

    let mut negative = false;
    let mut i = 0;
    let len = s.len();
    let mut limit = -i32::MAX;

    if len > 0 {
        let first_char = s[0];
        if first_char < b'0' as u16 {
            // Possible leading "+" or "-"
            if first_char == b'-' as u16 {
                negative = true;
                limit = i32::MIN;
            } else if first_char != b'+' as u16 {
                return Err(NumberFormatException::for_input_string(s, radix));
            }

            if len == 1 {
                // Cannot have lone "+" or "-"
                return Err(NumberFormatException::for_input_string(s, radix));
            }
            i += 1;
        }
        let multmin = limit / radix;
        let mut result: i32 = 0;
        while i < len {
            // Accumulating negatively avoids surprises near MAX_VALUE
            let digit = digit(s[i], radix);
            i += 1;
            if digit < 0 || result < multmin {
                return Err(NumberFormatException::for_input_string(s, radix));
            }
            result *= radix;
            if result < limit + digit {
                return Err(NumberFormatException::for_input_string(s, radix));
            }
            result -= digit;
        }
        Ok(if negative { result } else { -result })
    } else {
        Err(NumberFormatException::for_input_string(s, radix))
    }
}

const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

// port: Integer#toString(int, int)
pub fn to_string_radix(i: i32, radix: i32) -> String {
    let radix = if !(MIN_RADIX..=MAX_RADIX).contains(&radix) {
        10
    } else {
        radix
    };
    let mut i = i;
    let mut buf = [0u8; 33];
    let negative = i < 0;
    let mut char_pos = 32;

    if !negative {
        i = -i;
    }

    while i <= -radix {
        buf[char_pos] = DIGITS[(-(i % radix)) as usize];
        char_pos -= 1;
        i /= radix;
    }
    buf[char_pos] = DIGITS[(-i) as usize];

    if negative {
        char_pos -= 1;
        buf[char_pos] = b'-';
    }

    String::from_utf8(buf[char_pos..].to_vec()).unwrap()
}

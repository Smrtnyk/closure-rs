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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/lang/Long.java.

//! `java.lang.Long#parseLong(String, int)`, with its `NumberFormatException` messages, for the
//! integer tokens of `TextFormat`.

use closure_rhino::java_lang::{self, NumberFormatException};

// port: Long#parseLong(String,int)
pub(super) fn parse_long(s: &[u16], radix: u32) -> Result<i64, NumberFormatException> {
    let for_input_string = || {
        let mut message: Vec<u16> = "For input string: \"".encode_utf16().collect();
        message.extend_from_slice(s);
        message.push(b'"' as u16);
        if radix != 10 {
            message.extend(format!(" under radix {radix}").encode_utf16());
        }
        NumberFormatException { message }
    };
    let len = s.len();
    if len == 0 {
        return Err(for_input_string());
    }
    let digit = |c: u16| -> Option<i64> {
        let d = java_lang::digit(c, radix as i32);
        if d < 0 { None } else { Some(d as i64) }
    };
    let mut negative = false;
    let mut i = 0;
    let mut limit = -i64::MAX;
    let first_char = s[0];
    if first_char < b'0' as u16 {
        // Possible leading "+" or "-"
        if first_char == b'-' as u16 {
            negative = true;
            limit = i64::MIN;
        } else if first_char != b'+' as u16 {
            return Err(for_input_string());
        }
        if len == 1 {
            // Cannot have lone "+" or "-"
            return Err(for_input_string());
        }
        i += 1;
    }
    let multmin = limit / radix as i64;
    let mut result: i64 = 0;
    while i < len {
        // Accumulating negatively avoids surprises near MAX_VALUE
        let Some(d) = digit(s[i]) else {
            return Err(for_input_string());
        };
        i += 1;
        if result < multmin {
            return Err(for_input_string());
        }
        result *= radix as i64;
        if result < limit + d {
            return Err(for_input_string());
        }
        result -= d;
    }
    Ok(if negative { result } else { -result })
}

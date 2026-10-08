/*
 * Copyright (c) 1994, 2024, Oracle and/or its affiliates. All rights reserved.
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
 * Copyright (c) 2012, 2023, Oracle and/or its affiliates. All rights reserved.
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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/lang/String.java,
//   java.base/java/util/Base64.java.

//! `new String(bytes, UTF_8)` (the JDK 21 UTF-8 decoder with its replacement of malformed
//! input) and `Base64.Encoder#encodeToString`, for the neutral encoding of string and bytes
//! fields.

use crate::json::JsString;

/// `new String(bytes, UTF_8)` (what `ByteString#toStringUtf8` returns): the JDK 21 decoder,
/// whose replacement of malformed input differs from `String::from_utf8_lossy` (an encoded
/// surrogate, as WTF-8 writes a lone one, becomes one U+FFFD in Java, three in Rust).
// port: java.lang.String#String(Charset,byte[],int,int) (UTF_8, !COMPACT_STRINGS branch)
pub(super) fn new_string_utf8(bytes: &[u8]) -> JsString {
    let mut dst = Vec::with_capacity(bytes.len());
    decode_utf8_utf16(bytes, 0, bytes.len(), &mut dst);
    JsString(dst)
}

const REPL: u16 = 0xfffd;

// port: java.lang.String#isNotContinuation
fn is_not_continuation(b: i32) -> bool {
    (b & 0xc0) != 0x80
}

// port: java.lang.String#isMalformed3
fn is_malformed3(b1: i32, b2: i32, b3: i32) -> bool {
    (b1 == i32::from(0xe0u8 as i8) && (b2 & 0xe0) == 0x80)
        || (b2 & 0xc0) != 0x80
        || (b3 & 0xc0) != 0x80
}

// port: java.lang.String#isMalformed3_2
fn is_malformed3_2(b1: i32, b2: i32) -> bool {
    (b1 == i32::from(0xe0u8 as i8) && (b2 & 0xe0) == 0x80) || (b2 & 0xc0) != 0x80
}

// port: java.lang.String#isMalformed4
fn is_malformed4(b2: i32, b3: i32, b4: i32) -> bool {
    (b2 & 0xc0) != 0x80 || (b3 & 0xc0) != 0x80 || (b4 & 0xc0) != 0x80
}

// port: java.lang.String#isMalformed4_2
fn is_malformed4_2(b1: i32, b2: i32) -> bool {
    (b1 == 0xf0 && !(0x90..=0xbf).contains(&b2))
        || (b1 == 0xf4 && (b2 & 0xf0) != 0x80)
        || (b2 & 0xc0) != 0x80
}

// port: java.lang.String#isMalformed4_3
fn is_malformed4_3(b3: i32) -> bool {
    (b3 & 0xc0) != 0x80
}

/// A Java `byte` read as the sign-extended `int` the JDK code works with.
fn jbyte(src: &[u8], i: usize) -> i32 {
    i32::from(src[i] as i8)
}

// port: java.lang.String#decode2
fn decode2(b1: i32, b2: i32) -> u16 {
    (((b1 << 6) ^ b2) ^ ((i32::from(0xC0u8 as i8) << 6) ^ i32::from(0x80u8 as i8))) as u16
}

// port: java.lang.String#decode3
fn decode3(b1: i32, b2: i32, b3: i32) -> u16 {
    ((b1 << 12)
        ^ (b2 << 6)
        ^ (b3
            ^ ((i32::from(0xE0u8 as i8) << 12)
                ^ (i32::from(0x80u8 as i8) << 6)
                ^ i32::from(0x80u8 as i8)))) as u16
}

// port: java.lang.String#decode4
fn decode4(b1: i32, b2: i32, b3: i32, b4: i32) -> i32 {
    (b1 << 18)
        ^ (b2 << 12)
        ^ (b3 << 6)
        ^ (b4
            ^ ((i32::from(0xF0u8 as i8) << 18)
                ^ (i32::from(0x80u8 as i8) << 12)
                ^ (i32::from(0x80u8 as i8) << 6)
                ^ i32::from(0x80u8 as i8)))
}

// port: java.lang.String#malformed3
fn malformed3(src: &[u8], mut sp: usize) -> usize {
    let b1 = jbyte(src, sp);
    sp += 1;
    let b2 = jbyte(src, sp); // no need to lookup b3
    if (b1 == i32::from(0xe0u8 as i8) && (b2 & 0xe0) == 0x80) || is_not_continuation(b2) {
        1
    } else {
        2
    }
}

// port: java.lang.String#malformed4
fn malformed4(src: &[u8], mut sp: usize) -> usize {
    // we don't care the speed here
    let b1 = jbyte(src, sp) & 0xff;
    sp += 1;
    let b2 = jbyte(src, sp) & 0xff;
    sp += 1;
    if b1 > 0xf4
        || (b1 == 0xf0 && !(0x90..=0xbf).contains(&b2))
        || (b1 == 0xf4 && (b2 & 0xf0) != 0x80)
        || is_not_continuation(b2)
    {
        return 1;
    }
    if is_not_continuation(jbyte(src, sp)) {
        return 2;
    }
    3
}

// port: java.lang.String#decodeUTF8_UTF16 (doReplace = true)
fn decode_utf8_utf16(src: &[u8], mut sp: usize, sl: usize, dst: &mut Vec<u16>) {
    while sp < sl {
        let mut b1 = jbyte(src, sp);
        sp += 1;
        if b1 >= 0 {
            dst.push(b1 as u16);
        } else if (b1 >> 5) == -2 && (b1 & 0x1e) != 0 {
            if sp < sl {
                let b2 = jbyte(src, sp);
                sp += 1;
                if is_not_continuation(b2) {
                    dst.push(REPL);
                    sp -= 1;
                } else {
                    dst.push(decode2(b1, b2));
                }
                continue;
            }
            dst.push(REPL);
            break;
        } else if (b1 >> 4) == -2 {
            if sp + 1 < sl {
                let b2 = jbyte(src, sp);
                let b3 = jbyte(src, sp + 1);
                sp += 2;
                if is_malformed3(b1, b2, b3) {
                    dst.push(REPL);
                    sp -= 3;
                    sp += malformed3(src, sp);
                } else {
                    let c = decode3(b1, b2, b3);
                    if (0xd800..=0xdfff).contains(&c) {
                        // Character.isSurrogate
                        dst.push(REPL);
                    } else {
                        dst.push(c);
                    }
                }
                continue;
            }
            if sp < sl && is_malformed3_2(b1, jbyte(src, sp)) {
                dst.push(REPL);
                continue;
            }
            dst.push(REPL);
            break;
        } else if (b1 >> 3) == -2 {
            if sp + 2 < sl {
                let b2 = jbyte(src, sp);
                let b3 = jbyte(src, sp + 1);
                let b4 = jbyte(src, sp + 2);
                sp += 3;
                let uc = decode4(b1, b2, b3, b4);
                // shortest form check (Character.isSupplementaryCodePoint)
                if is_malformed4(b2, b3, b4) || !(0x10000..0x110000).contains(&uc) {
                    dst.push(REPL);
                    sp -= 4;
                    sp += malformed4(src, sp);
                } else {
                    // Character.highSurrogate / Character.lowSurrogate
                    dst.push((((uc as u32) >> 10) + (0xd800 - (0x10000 >> 10))) as u16);
                    dst.push((((uc as u32) & 0x3ff) + 0xdc00) as u16);
                }
                continue;
            }
            b1 &= 0xff;
            if b1 > 0xf4 || sp < sl && is_malformed4_2(b1, jbyte(src, sp) & 0xff) {
                dst.push(REPL);
                continue;
            }
            sp += 1;
            dst.push(REPL);
            if sp < sl && is_malformed4_3(jbyte(src, sp)) {
                continue;
            }
            break;
        } else {
            dst.push(REPL);
        }
    }
}

const BASE64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

// port: java.util.Base64.Encoder#encodeToString (RFC 4648 basic encoder, with padding)
pub(super) fn base64_encode(src: &[u8]) -> String {
    let mut out = String::with_capacity(src.len().div_ceil(3) * 4);
    for chunk in src.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(*chunk.get(1).unwrap_or(&0));
        let b2 = u32::from(*chunk.get(2).unwrap_or(&0));
        let bits = (b0 << 16) | (b1 << 8) | b2;
        out.push(BASE64_TABLE[((bits >> 18) & 0x3f) as usize] as char);
        out.push(BASE64_TABLE[((bits >> 12) & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 {
            BASE64_TABLE[((bits >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            BASE64_TABLE[(bits & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::new_string_utf8;

    #[test]
    fn decodes_like_java_new_string_utf8() {
        // An encoded surrogate (WTF-8 of a lone U+D800) is one replacement character in Java.
        assert_eq!(new_string_utf8(&[0xed, 0xa0, 0x80]).0, vec![0xfffd]);
        // Well-formed text, including a supplementary character, decodes unchanged.
        assert_eq!(
            new_string_utf8("a\u{e9}\u{20ac}\u{1f600}".as_bytes()).0,
            "a\u{e9}\u{20ac}\u{1f600}"
                .encode_utf16()
                .collect::<Vec<_>>()
        );
        // Overlong and truncated sequences (values from JDK 21's `new String(bytes, UTF_8)`).
        assert_eq!(new_string_utf8(&[0xc0, 0x80]).0, vec![0xfffd, 0xfffd]);
        assert_eq!(
            new_string_utf8(&[0xe0, 0x80, 0x80]).0,
            vec![0xfffd, 0xfffd, 0xfffd]
        );
        assert_eq!(new_string_utf8(&[0xe2, 0x82]).0, vec![0xfffd]);
        assert_eq!(new_string_utf8(&[0xf0, 0x9f, 0x98]).0, vec![0xfffd]);
    }
}

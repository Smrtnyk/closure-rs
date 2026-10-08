/*
 * Copyright (c) 2000, 2023, Oracle and/or its affiliates. All rights reserved.
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
 * Copyright (c) 2000, 2021, Oracle and/or its affiliates. All rights reserved.
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
 * Copyright (c) 2000, 2019, Oracle and/or its affiliates. All rights reserved.
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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1):
//   java.base/java/nio/charset/Charset.java, java.base/java/nio/charset/CharsetDecoder.java,
//   java.base/java/nio/charset/CharsetEncoder.java, java.base/sun/nio/cs/UTF_8.java,
//   java.base/sun/nio/cs/UnicodeDecoder.java.

use super::io_exception::IOException;
use crate::js_string::JsString;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charset {
    UTF_8,
    US_ASCII,
    ISO_8859_1,
    UTF_16,
    UTF_16BE,
    UTF_16LE,
}
impl Charset {
    // port: Charset#forName
    pub fn for_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "utf-8" | "utf8" | "unicode-1-1-utf-8" => Self::UTF_8,
            "us-ascii" | "iso-ir-6" | "ansi_x3.4-1986" | "iso_646.irv:1991" | "ascii"
            | "iso646-us" | "us" | "ibm367" | "cp367" | "csascii" | "646" | "iso_646.irv:1983"
            | "ansi_x3.4-1968" | "ascii7" => Self::US_ASCII,
            "iso-8859-1" | "iso-ir-100" | "iso_8859-1" | "latin1" | "l1" | "ibm819" | "cp819"
            | "csisolatin1" | "819" | "ibm-819" | "iso8859_1" | "iso_8859-1:1987"
            | "iso_8859_1" | "8859_1" | "iso8859-1" => Self::ISO_8859_1,
            "utf-16" | "utf_16" | "utf16" | "unicode" | "unicodebig" => Self::UTF_16,
            "utf-16be" | "utf_16be" | "iso-10646-ucs-2" | "x-utf-16be" | "unicodebigunmarked" => {
                Self::UTF_16BE
            }
            "utf-16le" | "utf_16le" | "x-utf-16le" | "unicodelittleunmarked" => Self::UTF_16LE,
            _ => panic!("{name}"),
        }
    }
    // port: Charset#name
    pub fn name(self) -> &'static str {
        match self {
            Self::UTF_8 => "UTF-8",
            Self::US_ASCII => "US-ASCII",
            Self::ISO_8859_1 => "ISO-8859-1",
            Self::UTF_16 => "UTF-16",
            Self::UTF_16BE => "UTF-16BE",
            Self::UTF_16LE => "UTF-16LE",
        }
    }
    // port: CharsetDecoder#decode
    pub fn decode(self, bytes: &[u8], replace: bool) -> Result<JsString, IOException> {
        let mut out = Vec::new();
        let mut i = 0;
        let mut little = self == Self::UTF_16LE;
        if self == Self::UTF_16 && bytes.len() >= 2 {
            if bytes[..2] == [0xfe, 0xff] {
                i = 2;
            } else if bytes[..2] == [0xff, 0xfe] {
                i = 2;
                little = true;
            }
        }
        while i < bytes.len() {
            let (units, consumed, malformed) = match self {
                Self::ISO_8859_1 => (vec![bytes[i] as u16], 1, false),
                Self::US_ASCII => (vec![bytes[i] as u16], 1, bytes[i] >= 128),
                Self::UTF_8 => decode_utf8(bytes, i),
                _ => decode_utf16(bytes, i, little),
            };
            if malformed {
                if !replace {
                    return Err(IOException::new(format!("Input length = {consumed}")));
                }
                out.push(0xfffd);
            } else {
                out.extend(units);
            }
            i += consumed;
        }
        Ok(JsString::from_units(out))
    }
}
// port: UTF_8.Encoder#encodeArrayLoop (CodingErrorAction.REPLACE, as PrintStream,
// OutputStreamWriter and String#getBytes(UTF_8) use it)
/// The text Java writes for a WTF-16 string through a UTF-8 encoder: every unpaired
/// surrogate becomes the encoder's replacement `'?'`. Use this, not a lossy U+FFFD
/// conversion, wherever a Java `String` that may hold source text (excerpts, diagnostic
/// arguments) becomes a Rust `String` bound for output; lengths in UTF-16 units are kept.
pub fn utf8_encoded_text(units: &[u16]) -> String {
    char::decode_utf16(units.iter().copied())
        .map(|c| c.unwrap_or('?'))
        .collect()
}
// port: UTF_8.Decoder#decodeBufferLoop
fn decode_utf8(bytes: &[u8], i: usize) -> (Vec<u16>, usize, bool) {
    let b = bytes[i];
    if b < 128 {
        return (vec![b as u16], 1, false);
    }
    let n = match b {
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => return (vec![], 1, true),
    };
    let mut value = (b & (0x7f >> n)) as u32;
    for k in 1..n {
        if i + k >= bytes.len() {
            return (vec![], k, true);
        }
        let next = bytes[i + k];
        if next & 0xc0 != 0x80
            || k == 1
                && (b == 0xe0 && next < 0xa0
                    || b == 0xf0 && next < 0x90
                    || b == 0xf4 && next >= 0x90)
        {
            return (vec![], k, true);
        }
        value = value << 6 | (next & 0x3f) as u32;
    }
    if (0xd800..=0xdfff).contains(&value) {
        return (vec![], n, true);
    }
    if value >= 0x10000 {
        let v = value - 0x10000;
        (
            vec![0xd800 + (v >> 10) as u16, 0xdc00 + (v & 0x3ff) as u16],
            n,
            false,
        )
    } else {
        (vec![value as u16], n, false)
    }
}
// port: UnicodeDecoder#decodeLoop
fn decode_utf16(bytes: &[u8], i: usize, little: bool) -> (Vec<u16>, usize, bool) {
    if bytes.len() - i < 2 {
        return (vec![], 1, true);
    }
    let unit = |at| {
        if little {
            u16::from_le_bytes([bytes[at], bytes[at + 1]])
        } else {
            u16::from_be_bytes([bytes[at], bytes[at + 1]])
        }
    };
    let c = unit(i);
    if (0xd800..=0xdbff).contains(&c) {
        if bytes.len() - i < 4 {
            return (vec![], bytes.len() - i, true);
        }
        let c2 = unit(i + 2);
        (vec![c, c2], 4, !(0xdc00..=0xdfff).contains(&c2))
    } else {
        (vec![c], 2, (0xdc00..=0xdfff).contains(&c))
    }
}

impl Charset {
    // port: Charset#newEncoder
    pub fn new_encoder(self) -> CharsetEncoder {
        CharsetEncoder(self)
    }
}
pub struct CharsetEncoder(Charset);
impl CharsetEncoder {
    // port: CharsetEncoder#canEncode
    pub fn can_encode(&self, c: u16) -> bool {
        match self.0 {
            Charset::US_ASCII => c <= 0x7f,
            Charset::ISO_8859_1 => c <= 0xff,
            Charset::UTF_8 | Charset::UTF_16 | Charset::UTF_16BE | Charset::UTF_16LE => {
                !(0xd800..=0xdfff).contains(&c)
            }
        }
    }
}

#[cfg(test)]
mod encoder_tests {
    use super::Charset;

    #[test]
    fn can_encode_every_utf16_code_unit() {
        for charset in [
            Charset::US_ASCII,
            Charset::ISO_8859_1,
            Charset::UTF_8,
            Charset::UTF_16,
            Charset::UTF_16BE,
            Charset::UTF_16LE,
        ] {
            let encoder = charset.new_encoder();
            for c in 0..=u16::MAX {
                let expected = match charset {
                    Charset::US_ASCII => c <= 0x7f,
                    Charset::ISO_8859_1 => c <= 0xff,
                    _ => !(0xd800..=0xdfff).contains(&c),
                };
                assert_eq!(encoder.can_encode(c), expected, "{charset:?} U+{c:04X}");
            }
        }
    }
}

/*
 * Copyright 2021 The Closure Compiler Authors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/serialization/Wtf8.java.

//! Port of serialization/Wtf8.java: WTF-8 encoding of Java strings (UTF-8 that keeps unpaired
//! surrogates as 3-byte sequences) for the TypedAST string pool.
use closure_rhino::js_string::JsString;

/// port: Wtf8
pub struct Wtf8;

const CONTINUATION_MASK: i32 = 0x3F;

impl Wtf8 {
    // port: Wtf8#encodeToWtf8
    pub fn encode_to_wtf8(s: &JsString) -> Vec<u8> {
        let length = s.length();
        let mut output: Vec<u8> = Vec::with_capacity(length);
        let mut i = 0;
        while i < length {
            let codepoint = s.code_point_at(i) as i32;

            if codepoint < 0x80 {
                output.push(codepoint as u8);
            } else if codepoint < 0x800 {
                output.push((0xC0 | (0x1f & (codepoint >> 6))) as u8);
                output.push((0x80 | (CONTINUATION_MASK & codepoint)) as u8);
            } else if codepoint < 0x10000 {
                output.push((0xE0 | (0xf & (codepoint >> 12))) as u8);
                output.push((0x80 | (CONTINUATION_MASK & (codepoint >> 6))) as u8);
                output.push((0x80 | (CONTINUATION_MASK & codepoint)) as u8);
            } else {
                // This codepoints takes two UTF-16 code units, so we need an extra increment.
                i += 1;
                output.push((0xF0 | (0x7 & (codepoint >> 18))) as u8);
                output.push((0x80 | (CONTINUATION_MASK & (codepoint >> 12))) as u8);
                output.push((0x80 | (CONTINUATION_MASK & (codepoint >> 6))) as u8);
                output.push((0x80 | (CONTINUATION_MASK & codepoint)) as u8);
            }
            i += 1;
        }
        output
    }

    // port: Wtf8#decoder
    pub fn decoder(max_length: i32) -> Decoder {
        Decoder::new(max_length)
    }
}

/// port: Wtf8.Decoder
///
/// Decodes strings from WTF8 bytes. It reuses its buffer between decodings.
pub struct Decoder {
    codepoint_buffer: Vec<i32>,
}

impl Decoder {
    // port: Wtf8.Decoder#<init>
    fn new(max_length: i32) -> Self {
        Self {
            codepoint_buffer: vec![0; max_length as usize],
        }
    }

    // port: Wtf8.Decoder#decode
    pub fn decode(&mut self, encoded: &[u8]) -> JsString {
        let encoded_byte_count = encoded.len();
        let mut codepoint_count = 0;

        let mut i = 0;
        while i < encoded_byte_count {
            let b = encoded[i] as i8 as i32;
            let codepoint;

            if (b & 0x80) == 0 {
                // 0xxx xxxx: 1 byte
                codepoint = b;
            } else if (b & 0xE0) == 0xC0 {
                // 110x xxxx: 2 bytes
                let first_byte = 0x1F & b;
                i += 1;
                let second_byte = (encoded[i] as i8 as i32) & CONTINUATION_MASK;
                codepoint = (first_byte << 6) | second_byte;
            } else if (b & 0xF0) == 0xE0 {
                // 1110 xxxx: 3 bytes
                let first_byte = 0xF & b;
                i += 1;
                let second_byte = (encoded[i] as i8 as i32) & CONTINUATION_MASK;
                i += 1;
                let third_byte = (encoded[i] as i8 as i32) & CONTINUATION_MASK;
                codepoint = (first_byte << 12) | (second_byte << 6) | third_byte;
            } else if (b & 0xF8) == 0xF0 {
                // 1111 0xxx: 4 bytes
                let first_byte = 0x7 & b;
                i += 1;
                let second_byte = (encoded[i] as i8 as i32) & CONTINUATION_MASK;
                i += 1;
                let third_byte = (encoded[i] as i8 as i32) & CONTINUATION_MASK;
                i += 1;
                let fourth_byte = (encoded[i] as i8 as i32) & CONTINUATION_MASK;
                codepoint =
                    (first_byte << 18) | (second_byte << 12) | (third_byte << 6) | fourth_byte;
            } else {
                panic!("java.lang.AssertionError");
            }

            self.codepoint_buffer[codepoint_count] = codepoint;
            codepoint_count += 1;
            i += 1;
        }

        // new String(int[] codePoints, int offset, int count)
        let mut units: Vec<u16> = Vec::with_capacity(codepoint_count);
        for &codepoint in &self.codepoint_buffer[..codepoint_count] {
            if codepoint < 0x10000 {
                units.push(codepoint as u16);
            } else {
                let c = codepoint - 0x10000;
                units.push((0xD800 + (c >> 10)) as u16);
                units.push((0xDC00 + (c & 0x3FF)) as u16);
            }
        }
        JsString::from_units(units)
    }
}

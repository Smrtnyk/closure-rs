/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/debugging/sourcemap/Base64.java.

use closure_rhino::js_string::JsString;

pub struct Base64;
impl Base64 {
    const BASE64_MAP: &'static [u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    const BASE64_DECODE_MAP: [i32; 256] = Self::init_decode_map();

    // port: Base64#<clinit>
    const fn init_decode_map() -> [i32; 256] {
        let mut base64_decode_map = [-1; 256];
        let mut i = 0;
        while i < Self::BASE64_MAP.len() {
            base64_decode_map[Self::BASE64_MAP[i] as usize] = i as i32;
            i += 1;
        }
        base64_decode_map
    }

    // port: Base64#Base64
    #[allow(dead_code)]
    fn new() -> Self {
        Self
    }

    // port: Base64#toBase64
    pub fn to_base64(value: i32) -> u16 {
        // assert (value <= 63 && value >= 0) : "value out of range:" + value;
        *Self::BASE64_MAP.get(value as usize).unwrap_or_else(|| {
            panic!("java.lang.StringIndexOutOfBoundsException: Index {value} out of bounds for length 64")
        }) as u16
    }

    // port: Base64#fromBase64
    pub fn from_base64(c: u16) -> i32 {
        if c >= 256 {
            panic!(
                "java.lang.ArrayIndexOutOfBoundsException: Index {c} out of bounds for length 256"
            );
        }
        let result = Self::BASE64_DECODE_MAP[c as usize];
        // assert (result != -1) : "invalid char";
        result
    }

    // Preserve the Java statement order.
    #[allow(clippy::needless_range_loop)]
    // port: Base64#base64EncodeInt
    pub fn base64_encode_int(value: i32) -> JsString {
        let mut c = [0; 6];
        for i in 0..5 {
            c[i] = Self::to_base64((value >> (26 - i * 6)) & 0x3f);
        }
        c[5] = Self::to_base64(value.wrapping_shl(4) & 0x3f);
        JsString::from_units(c.to_vec())
    }
}

/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/GoogleJsMessageIdGenerator.java.

//! Port of `com.google.javascript.jscomp.GoogleJsMessageIdGenerator`: generates fingerprint ids
//! for messages (the ids used by Google's translation console).

use closure_rhino::java_lang::utf_8;
use closure_rhino::js_string::JsString;

use crate::js_message::{IdGenerator, Part};

/// An `IdGenerator` designed to play nicely with Google's Translation systems.
#[derive(Clone, Debug)]
pub struct GoogleJsMessageIdGenerator {
    project_id: Option<String>,
}

impl GoogleJsMessageIdGenerator {
    /// Creates an instance.
    // port: GoogleJsMessageIdGenerator#GoogleJsMessageIdGenerator
    pub fn new(project_id: Option<String>) -> Self {
        Self { project_id }
    }
}

impl IdGenerator for GoogleJsMessageIdGenerator {
    // port: GoogleJsMessageIdGenerator#generateId
    fn generate_id(&self, meaning: &JsString, message_parts: &[Part]) -> JsString {
        // checkState(meaning != null): the Rust signature makes `meaning` non-null.

        let mut sb: Vec<u16> = Vec::new();
        for part in message_parts {
            if part.is_placeholder() {
                sb.extend_from_slice(part.get_canonical_placeholder_name().as_units());
            } else {
                sb.extend_from_slice(part.get_string().as_units());
            }
        }
        let tc_value = JsString::from_units(sb);

        // Strings.isNullOrEmpty(projectId)
        let project_scoped_meaning = match &self.project_id {
            Some(project_id) if !project_id.is_empty() => {
                JsString::from(format!("{project_id}: ")).concat(meaning)
            }
            _ => meaning.clone(),
        };
        JsString::from(message_id::generate_id(&tc_value, &project_scoped_meaning).to_string())
    }
}

/// 64-bit fingerprint support.
mod fp {
    use super::*;

    /// Java's `>>>` on `int`.
    fn urs32(x: i32, n: u32) -> i32 {
        ((x as u32) >> n) as i32
    }

    // port: GoogleJsMessageIdGenerator.FP#fingerprint(byte[],int,int)
    fn fingerprint_bytes(str: &[u8], start: usize, limit: usize) -> i64 {
        let mut hi = hash32(str, start, limit, 0);
        let mut lo = hash32(str, start, limit, 102072);
        if (hi == 0) && (lo == 0 || lo == 1) {
            // Turn 0/1 into another fingerprint
            hi ^= 0x130f9bef;
            lo ^= 0x94a0a928_u32 as i32;
        }
        (i64::from(hi) << 32) | (i64::from(lo) & 0xffffffff_i64)
    }

    // port: GoogleJsMessageIdGenerator.FP#fingerprint(String)
    pub(super) fn fingerprint(str: &JsString) -> i64 {
        let tmp = utf_8::encode(str);
        fingerprint_bytes(&tmp, 0, tmp.len())
    }

    // port: GoogleJsMessageIdGenerator.FP#hash32
    fn hash32(str: &[u8], start: usize, limit: usize, c: i32) -> i32 {
        let mut c = c;
        let mut a: i32 = 0x9e3779b9_u32 as i32;
        let mut b: i32 = 0x9e3779b9_u32 as i32;
        // Java's (str[i] & 0xff)
        let ub = |i: usize| -> i32 { i32::from(str[i]) };
        let mut i = start;
        while i + 12 <= limit {
            a = a.wrapping_add(ub(i) | (ub(i + 1) << 8) | (ub(i + 2) << 16) | (ub(i + 3) << 24));
            b = b
                .wrapping_add(ub(i + 4) | (ub(i + 5) << 8) | (ub(i + 6) << 16) | (ub(i + 7) << 24));
            c = c.wrapping_add(
                ub(i + 8) | (ub(i + 9) << 8) | (ub(i + 10) << 16) | (ub(i + 11) << 24),
            );

            // Mix
            a = a.wrapping_sub(b);
            a = a.wrapping_sub(c);
            a ^= urs32(c, 13);
            b = b.wrapping_sub(c);
            b = b.wrapping_sub(a);
            b ^= a << 8;
            c = c.wrapping_sub(a);
            c = c.wrapping_sub(b);
            c ^= urs32(b, 13);
            a = a.wrapping_sub(b);
            a = a.wrapping_sub(c);
            a ^= urs32(c, 12);
            b = b.wrapping_sub(c);
            b = b.wrapping_sub(a);
            b ^= a << 16;
            c = c.wrapping_sub(a);
            c = c.wrapping_sub(b);
            c ^= urs32(b, 5);
            a = a.wrapping_sub(b);
            a = a.wrapping_sub(c);
            a ^= urs32(c, 3);
            b = b.wrapping_sub(c);
            b = b.wrapping_sub(a);
            b ^= a << 10;
            c = c.wrapping_sub(a);
            c = c.wrapping_sub(b);
            c ^= urs32(b, 15);

            i += 12;
        }

        c = c.wrapping_add((limit - start) as i32);
        let tmp = limit - i;
        if tmp == 11 {
            c = c.wrapping_add(ub(i + 10) << 24);
        }
        if tmp >= 10 {
            c = c.wrapping_add(ub(i + 9) << 16);
        }
        if tmp >= 9 {
            c = c.wrapping_add(ub(i + 8) << 8);
            // the first byte of c is reserved for the length
        }
        if tmp >= 8 {
            b = b.wrapping_add(ub(i + 7) << 24);
        }
        if tmp >= 7 {
            b = b.wrapping_add(ub(i + 6) << 16);
        }
        if tmp >= 6 {
            b = b.wrapping_add(ub(i + 5) << 8);
        }
        if tmp >= 5 {
            b = b.wrapping_add(ub(i + 4));
        }
        if tmp >= 4 {
            a = a.wrapping_add(ub(i + 3) << 24);
        }
        if tmp >= 3 {
            a = a.wrapping_add(ub(i + 2) << 16);
        }
        if tmp >= 2 {
            a = a.wrapping_add(ub(i + 1) << 8);
        }
        if tmp >= 1 {
            a = a.wrapping_add(ub(i));
            // case 0 : nothing left to add
        }

        // Mix
        a = a.wrapping_sub(b);
        a = a.wrapping_sub(c);
        a ^= urs32(c, 13);
        b = b.wrapping_sub(c);
        b = b.wrapping_sub(a);
        b ^= a << 8;
        c = c.wrapping_sub(a);
        c = c.wrapping_sub(b);
        c ^= urs32(b, 13);
        a = a.wrapping_sub(b);
        a = a.wrapping_sub(c);
        a ^= urs32(c, 12);
        b = b.wrapping_sub(c);
        b = b.wrapping_sub(a);
        b ^= a << 16;
        c = c.wrapping_sub(a);
        c = c.wrapping_sub(b);
        c ^= urs32(b, 5);
        a = a.wrapping_sub(b);
        a = a.wrapping_sub(c);
        a ^= urs32(c, 3);
        b = b.wrapping_sub(c);
        b = b.wrapping_sub(a);
        b ^= a << 10;
        c = c.wrapping_sub(a);
        c = c.wrapping_sub(b);
        c ^= urs32(b, 15);
        c
    }
}

/// Generates fingerprint for an English message using the FP package.
mod message_id {
    use super::*;

    // port: GoogleJsMessageIdGenerator.MessageId#generateId
    pub(super) fn generate_id(message: &JsString, meaning: &JsString) -> i64 {
        let mut fp = fp::fingerprint(message);
        if meaning.length() > 0 {
            // combine the fingerprints of message and meaning
            let fp2 = fp::fingerprint(meaning);
            fp = fp2
                .wrapping_add(fp.wrapping_shl(1))
                .wrapping_add(if fp < 0 { 1 } else { 0 });
        }
        // To avoid negative ids we strip the high-order bit
        fp & 0x7fffffffffffffff_i64
    }
}

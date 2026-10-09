/*
 * Copyright 2020 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/colors/ColorId.java.

use crate::fast_hash::IndexSet;
use crate::{check_state, common_hash::farm_hash_fingerprint64, js_string::JsString};
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColorId {
    right_aligned: i64,
}

// port: ColorId#ASCII_AT
const ASCII_AT: fn(&JsString, usize) -> i8 = |s, i| {
    let c = s.code_point_at(i);
    check_state!(c < 128, "%s", c);
    c as i8
};

impl ColorId {
    /// Rust-only: the private `rightAligned` field, read by the unit recorder's object dump.
    pub const fn right_aligned(&self) -> i64 {
        self.right_aligned
    }
    // port: ColorId#fromUnsigned(long)
    pub const fn from_unsigned(x: i64) -> Self {
        Self { right_aligned: x }
    }
    // port: ColorId#fromUnsigned(int)
    pub const fn from_unsigned_int(x: i32) -> Self {
        Self {
            right_aligned: x as i64 & 0xffffffff,
        }
    }
    // port: ColorId#fromUnsigned(byte)
    pub const fn from_unsigned_byte(x: i8) -> Self {
        Self {
            right_aligned: x as i64 & 0xff,
        }
    }
    // port: ColorId#fromBytes(ByteString)
    pub fn from_byte_string(bytes: &[u8]) -> Self {
        Self::from_bytes_at(bytes, bytes.len(), |s, i| s[i] as i8)
    }
    // port: ColorId#fromBytes(byte[])
    pub fn from_bytes(bytes: &[i8]) -> Self {
        Self::from_bytes_at(bytes, bytes.len(), |s, i| s[i])
    }
    // port: ColorId#fromAscii
    pub fn from_ascii(s: &str) -> Self {
        let s = JsString::from(s);
        Self::from_bytes_at(&s, s.length(), ASCII_AT)
    }
    // port: ColorId#fromBytesAt
    fn from_bytes_at<T: ?Sized>(
        source: &T,
        length: usize,
        byte_at: impl Fn(&T, usize) -> i8,
    ) -> Self {
        check_state!(length <= 8, "%s", length);
        let mut right_aligned = 0i64;
        for i in 0..length {
            right_aligned <<= 8;
            right_aligned |= byte_at(source, i) as i64 & 0xff;
        }
        Self { right_aligned }
    }
    // port: ColorId#union
    pub fn union(ids: &IndexSet<ColorId>) -> Self {
        if ids.len() <= 1 {
            return *ids.first().expect("");
        }
        let mut sorted: Vec<i64> = ids.iter().map(|id| id.right_aligned).collect();
        sorted.sort();
        let mut bytes = Vec::with_capacity(sorted.len() * 8);
        for id in sorted {
            bytes.extend_from_slice(&id.to_le_bytes());
        }
        Self {
            right_aligned: farm_hash_fingerprint64::hash_bytes(&bytes),
        }
    }
    // port: ColorId#hashCode
    pub fn hash_code(&self) -> i32 {
        self.right_aligned as i32
    }
    // port: ColorId#asByteString
    pub fn as_byte_string(&self) -> Vec<u8> {
        let mut copy = self.right_aligned as u64;
        let mut out = vec![0; 8];
        for i in (0..8).rev() {
            out[i] = copy as u8;
            copy >>= 8;
        }
        out
    }
    // port: ColorId#toLogsGson
    pub fn to_logs_gson(&self) -> String {
        self.to_string()
    }
}
impl fmt::Display for ColorId {
    // port: ColorId#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:x}", self.right_aligned)
    }
}
impl fmt::Debug for ColorId {
    // port: ColorId#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

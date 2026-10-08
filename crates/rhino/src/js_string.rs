/*
 * Copyright 2026 The closure-rs Authors.
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

//! Java String's UTF-16 storage, including unpaired surrogate code units.
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct JsString(pub(crate) Arc<[u16]>);
impl JsString {
    pub fn from_units(units: impl Into<Vec<u16>>) -> Self {
        Self(Arc::from(units.into()))
    }
    pub fn as_units(&self) -> &[u16] {
        &self.0
    }
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    // port: String#length
    pub fn length(&self) -> usize {
        self.0.len()
    }
    // port: String#charAt
    pub fn char_at(&self, i: usize) -> u16 {
        self.0[i]
    }
    // port: String#codePointAt
    pub fn code_point_at(&self, i: usize) -> u32 {
        let c = self.char_at(i);
        if (0xd800..=0xdbff).contains(&c) && i + 1 < self.length() {
            let d = self.char_at(i + 1);
            if (0xdc00..=0xdfff).contains(&d) {
                return 0x10000 + ((c as u32 - 0xd800) << 10) + (d as u32 - 0xdc00);
            }
        }
        c as u32
    }
    // port: String#substring(int, int)
    pub fn substring(&self, b: usize, e: usize) -> Self {
        assert!(b <= e);
        if b == 0 && e == self.length() {
            self.clone()
        } else {
            Self::from_units(self.0[b..e].to_vec())
        }
    }
    // port: String#substring(int)
    pub fn substring_from(&self, b: usize) -> Self {
        self.substring(b, self.length())
    }
    // port: String#indexOf(String)
    pub fn index_of(&self, needle: &Self) -> i32 {
        self.index_of_from(needle, 0)
    }
    // port: String#indexOf(String, int)
    pub fn index_of_from(&self, needle: &Self, from: i32) -> i32 {
        let b = (from.max(0) as usize).min(self.length());
        if needle.is_empty() {
            return b as i32;
        }
        if needle.length() > self.length() - b {
            return -1;
        }
        self.0[b..]
            .windows(needle.length())
            .position(|v| v == needle.as_units())
            .map_or(-1, |i| (b + i) as i32)
    }
    // port: String#indexOf(int)
    pub fn index_of_char(&self, c: u16) -> i32 {
        self.0.iter().position(|v| *v == c).map_or(-1, |i| i as i32)
    }
    // port: String#lastIndexOf(int)
    pub fn last_index_of_char(&self, c: u16) -> i32 {
        self.0
            .iter()
            .rposition(|v| *v == c)
            .map_or(-1, |i| i as i32)
    }
    // port: String#lastIndexOf(String)
    pub fn last_index_of(&self, needle: &Self) -> i32 {
        if needle.is_empty() {
            return self.length() as i32;
        }
        if needle.length() > self.length() {
            return -1;
        }
        self.as_units()
            .windows(needle.length())
            .rposition(|value| value == needle.as_units())
            .map_or(-1, |index| index as i32)
    }
    // port: String#startsWith
    pub fn starts_with(&self, s: &Self) -> bool {
        self.0.starts_with(&s.0)
    }
    // port: String#endsWith
    pub fn ends_with(&self, s: &Self) -> bool {
        self.0.ends_with(&s.0)
    }
    // port: String#isEmpty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    // port: String#hashCode
    pub fn hash_code(&self) -> i32 {
        self.0
            .iter()
            .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(*c as i32))
    }
    // port: String#compareTo
    pub fn compare_to(&self, s: &Self) -> i32 {
        for (a, b) in self.0.iter().zip(s.0.iter()) {
            if a != b {
                return *a as i32 - *b as i32;
            }
        }
        (self.length() as i32).wrapping_sub(s.length() as i32)
    }
    // port: String#concat
    pub fn concat(&self, s: &Self) -> Self {
        if s.is_empty() {
            return self.clone();
        }
        let mut units = self.0.to_vec();
        units.extend_from_slice(&s.0);
        Self::from_units(units)
    }
    // port: String#replace(CharSequence,CharSequence)
    pub fn replace(&self, target: &Self, replacement: &Self) -> Self {
        let mut result = Vec::new();
        if target.is_empty() {
            result.extend_from_slice(replacement.as_units());
            for &unit in self.as_units() {
                result.push(unit);
                result.extend_from_slice(replacement.as_units());
            }
            return Self::from_units(result);
        }
        let mut position = self.index_of(target);
        if position < 0 {
            return self.clone();
        }
        let mut cursor = 0;
        while position >= 0 {
            let at = position as usize;
            result.extend_from_slice(&self.as_units()[cursor..at]);
            result.extend_from_slice(replacement.as_units());
            cursor = at + target.length();
            position = self.index_of_from(target, cursor as i32);
        }
        result.extend_from_slice(&self.as_units()[cursor..]);
        Self::from_units(result)
    }
    /// Explicitly lossy UTF-8 convenience conversion; Java strings stay in `as_units()`.
    pub fn to_string_lossy(&self) -> String {
        String::from_utf16_lossy(&self.0)
    }
}
impl From<&str> for JsString {
    fn from(s: &str) -> Self {
        Self::from_units(s.encode_utf16().collect::<Vec<_>>())
    }
}
impl From<String> for JsString {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}
impl From<&JsString> for JsString {
    fn from(s: &JsString) -> Self {
        s.clone()
    }
}
impl PartialEq for JsString {
    fn eq(&self, s: &Self) -> bool {
        self.ptr_eq(s) || self.0 == s.0
    }
}
impl Eq for JsString {}
impl Hash for JsString {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.0.hash(h);
    }
}
impl Ord for JsString {
    fn cmp(&self, s: &Self) -> Ordering {
        self.0.cmp(&s.0)
    }
}
impl PartialOrd for JsString {
    fn partial_cmp(&self, s: &Self) -> Option<Ordering> {
        Some(self.cmp(s))
    }
}
impl PartialEq<str> for JsString {
    fn eq(&self, s: &str) -> bool {
        self.0.iter().copied().eq(s.encode_utf16())
    }
}
impl PartialEq<&str> for JsString {
    fn eq(&self, s: &&str) -> bool {
        self == *s
    }
}
impl fmt::Display for JsString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Display is UTF-8 output: use Java's encoder replacement, not a source-string conversion.
        f.write_str(&crate::java_lang::charset::utf8_encoded_text(
            self.as_units(),
        ))
    }
}
impl fmt::Debug for JsString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("JsString").field(&self.0).finish()
    }
}

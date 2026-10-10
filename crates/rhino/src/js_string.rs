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

/// Rust-only layout (D-025): a string carries its `hashCode()` next to the reference to its
/// code units, computed once when the string is made (Java's String caches it in the object).
/// Hashing and `hashCode` read it without touching the code units, and strings with different
/// hashes compare unequal without reading them.
///
/// A string interned by `RhinoStringPool` is `Interned`: the pool keeps its code units for the
/// rest of the process, so copies of it share them without reference counting, and two interned
/// strings are equal exactly when they share them (Java: the interner is weak and the garbage
/// collector frees strings no node uses any more). Every other string is `Shared`, a
/// reference-counted array.
#[derive(Clone)]
pub struct JsString(Repr);
#[derive(Clone)]
enum Repr {
    Interned(u32, &'static [u16]),
    Shared(u32, Arc<[u16]>),
}
impl Default for JsString {
    fn default() -> Self {
        Self::from_slice(&[])
    }
}
impl JsString {
    pub fn from_units(units: impl Into<Vec<u16>>) -> Self {
        let units: Vec<u16> = units.into();
        Self(Repr::Shared(Self::java_hash(&units), units.into()))
    }
    /// Rust-only: a string of the code units `units`.
    pub fn from_slice(units: &[u16]) -> Self {
        Self(Repr::Shared(Self::java_hash(units), units.into()))
    }
    /// Rust-only: `String#hashCode` of `units`.
    fn java_hash(units: &[u16]) -> u32 {
        units
            .iter()
            .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(i32::from(*c))) as u32
    }
    /// Rust-only: whether the string pool made this string.
    pub(crate) fn is_interned(&self) -> bool {
        matches!(self.0, Repr::Interned(..))
    }
    /// Rust-only: an interned copy of this string whose code units live for the rest of the
    /// process, for the string pool.
    pub(crate) fn leak_interned(&self) -> Self {
        let units: &'static [u16] = Box::leak(self.as_units().into());
        Self(Repr::Interned(self.hash_bits(), units))
    }
    /// Rust-only: the code units of an interned string, which live for the rest of the process.
    pub(crate) fn static_units(&self) -> &'static [u16] {
        match self.0 {
            Repr::Interned(_, units) => units,
            Repr::Shared(..) => panic!("not an interned string"),
        }
    }
    fn hash_bits(&self) -> u32 {
        match &self.0 {
            Repr::Interned(hash, _) | Repr::Shared(hash, _) => *hash,
        }
    }
    pub fn as_units(&self) -> &[u16] {
        match &self.0 {
            Repr::Interned(_, units) => units,
            Repr::Shared(_, units) => units,
        }
    }
    pub fn ptr_eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Repr::Interned(_, a), Repr::Interned(_, b)) => std::ptr::eq(*a, *b),
            (Repr::Shared(_, a), Repr::Shared(_, b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
    // port: String#length
    pub fn length(&self) -> usize {
        self.as_units().len()
    }
    // port: String#charAt
    pub fn char_at(&self, i: usize) -> u16 {
        self.as_units()[i]
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
            Self::from_slice(&self.as_units()[b..e])
        }
    }
    // port: String#substring(int)
    pub fn substring_from(&self, b: usize) -> Self {
        self.substring(b, self.length())
    }
    // port: String#indexOf(String)
    pub fn index_of(&self, needle: impl JsStrLike) -> i32 {
        self.index_of_from(needle, 0)
    }
    // port: String#indexOf(String, int)
    pub fn index_of_from(&self, needle: impl JsStrLike, from: i32) -> i32 {
        needle.with_units(|needle| {
            let b = (from.max(0) as usize).min(self.length());
            if needle.is_empty() {
                return b as i32;
            }
            if needle.len() > self.length() - b {
                return -1;
            }
            find_units(&self.as_units()[b..], needle).map_or(-1, |i| (b + i) as i32)
        })
    }
    // port: String#indexOf(int)
    pub fn index_of_char(&self, c: u16) -> i32 {
        self.as_units()
            .iter()
            .position(|v| *v == c)
            .map_or(-1, |i| i as i32)
    }
    // port: String#lastIndexOf(int)
    pub fn last_index_of_char(&self, c: u16) -> i32 {
        self.as_units()
            .iter()
            .rposition(|v| *v == c)
            .map_or(-1, |i| i as i32)
    }
    // port: String#lastIndexOf(String)
    pub fn last_index_of(&self, needle: impl JsStrLike) -> i32 {
        needle.with_units(|needle| {
            if needle.is_empty() {
                return self.length() as i32;
            }
            if needle.len() > self.length() {
                return -1;
            }
            self.as_units()
                .windows(needle.len())
                .rposition(|value| value == needle)
                .map_or(-1, |index| index as i32)
        })
    }
    // port: String#startsWith
    pub fn starts_with(&self, s: impl JsStrLike) -> bool {
        // (Any string form: callers need not allocate a JsString for a literal.)
        s.is_prefix_of(self.as_units())
    }
    // port: String#endsWith
    pub fn ends_with(&self, s: impl JsStrLike) -> bool {
        s.is_suffix_of(self.as_units())
    }
    // port: String#isEmpty
    pub fn is_empty(&self) -> bool {
        self.as_units().is_empty()
    }
    // port: String#hashCode
    pub fn hash_code(&self) -> i32 {
        self.hash_bits() as i32
    }
    // port: String#compareTo
    pub fn compare_to(&self, s: &Self) -> i32 {
        for (a, b) in self.as_units().iter().zip(s.as_units().iter()) {
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
        let mut units = self.as_units().to_vec();
        units.extend_from_slice(s.as_units());
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
        String::from_utf16_lossy(self.as_units())
    }
}
/// Rust-only (D-025): the first index of `needle` (non-empty) in `hay`, as `String#indexOf`
/// finds it: scans for the first unit, then compares the rest (Java's loop does the same; a
/// window-by-window slice compare costs a call per position).
fn find_units(hay: &[u16], needle: &[u16]) -> Option<usize> {
    let (&first, rest) = needle.split_first()?;
    let last_start = hay.len().checked_sub(needle.len())?;
    let mut i = 0;
    while i <= last_start {
        match hay[i..=last_start].iter().position(|u| *u == first) {
            None => return None,
            Some(off) => {
                let at = i + off;
                if hay[at + 1..at + needle.len()] == *rest {
                    return Some(at);
                }
                i = at + 1;
            }
        }
    }
    None
}
/// Rust-only: a string argument compared against JS strings without allocating a `JsString`
/// (Java passes `String`s, which need no conversion).
pub trait JsStrLike {
    /// Calls `f` with the UTF-16 code units of the string.
    fn with_units<R>(&self, f: impl FnOnce(&[u16]) -> R) -> R;
    /// Whether `units` begin with the code units of the string.
    fn is_prefix_of(&self, units: &[u16]) -> bool {
        self.with_units(|s| units.starts_with(s))
    }
    /// Whether `units` end with the code units of the string.
    fn is_suffix_of(&self, units: &[u16]) -> bool {
        self.with_units(|s| units.ends_with(s))
    }
}
/// Rust-only fast path (D-025): an ASCII str has one code unit per byte, so it is compared with
/// code units without converting it.
fn ascii_units_eq(units: &[u16], s: &str) -> bool {
    units.len() == s.len() && units.iter().zip(s.bytes()).all(|(a, b)| *a == u16::from(b))
}
impl JsStrLike for str {
    fn is_prefix_of(&self, units: &[u16]) -> bool {
        if self.is_ascii() {
            return units.len() >= self.len() && ascii_units_eq(&units[..self.len()], self);
        }
        self.with_units(|s| units.starts_with(s))
    }
    fn is_suffix_of(&self, units: &[u16]) -> bool {
        if self.is_ascii() {
            return units.len() >= self.len()
                && ascii_units_eq(&units[units.len() - self.len()..], self);
        }
        self.with_units(|s| units.ends_with(s))
    }
    fn with_units<R>(&self, f: impl FnOnce(&[u16]) -> R) -> R {
        // A str of n bytes has at most n UTF-16 code units.
        let mut buf = [0u16; 128];
        if self.len() <= buf.len() {
            let mut n = 0;
            for unit in self.encode_utf16() {
                buf[n] = unit;
                n += 1;
            }
            f(&buf[..n])
        } else {
            f(&self.encode_utf16().collect::<Vec<_>>())
        }
    }
}
impl JsStrLike for String {
    fn with_units<R>(&self, f: impl FnOnce(&[u16]) -> R) -> R {
        self.as_str().with_units(f)
    }
    fn is_prefix_of(&self, units: &[u16]) -> bool {
        self.as_str().is_prefix_of(units)
    }
    fn is_suffix_of(&self, units: &[u16]) -> bool {
        self.as_str().is_suffix_of(units)
    }
}
impl JsStrLike for JsString {
    fn with_units<R>(&self, f: impl FnOnce(&[u16]) -> R) -> R {
        f(self.as_units())
    }
}
impl<T: JsStrLike + ?Sized> JsStrLike for &T {
    fn with_units<R>(&self, f: impl FnOnce(&[u16]) -> R) -> R {
        (**self).with_units(f)
    }
    fn is_prefix_of(&self, units: &[u16]) -> bool {
        (**self).is_prefix_of(units)
    }
    fn is_suffix_of(&self, units: &[u16]) -> bool {
        (**self).is_suffix_of(units)
    }
}
impl From<&str> for JsString {
    fn from(s: &str) -> Self {
        // Rust-only (D-025): Java's "" literal is one interned object; share one empty string
        // instead of allocating one per conversion.
        static EMPTY: std::sync::LazyLock<JsString> =
            std::sync::LazyLock::new(|| JsString::from_units(Vec::new()));
        if s.is_empty() {
            return EMPTY.clone();
        }
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
        // The cached hashes are compared first (equal strings have equal hashes); two interned
        // strings are equal only if they are the same pool entry.
        if self.hash_bits() != s.hash_bits() {
            return false;
        }
        match (&self.0, &s.0) {
            (Repr::Interned(_, a), Repr::Interned(_, b)) => std::ptr::eq(*a, *b),
            _ => self.ptr_eq(s) || self.as_units() == s.as_units(),
        }
    }
}
impl Eq for JsString {}
impl Hash for JsString {
    fn hash<H: Hasher>(&self, h: &mut H) {
        h.write_u32(self.hash_code() as u32);
    }
}
impl Ord for JsString {
    fn cmp(&self, s: &Self) -> Ordering {
        if self.ptr_eq(s) {
            return Ordering::Equal;
        }
        self.as_units().cmp(s.as_units())
    }
}
impl PartialOrd for JsString {
    fn partial_cmp(&self, s: &Self) -> Option<Ordering> {
        Some(self.cmp(s))
    }
}
impl PartialEq<str> for JsString {
    fn eq(&self, s: &str) -> bool {
        // A str of n bytes has at most n UTF-16 code units (cheap early exit).
        if self.as_units().len() > s.len() {
            return false;
        }
        // Rust-only fast path: an ASCII str has one code unit per byte.
        if self.as_units().len() == s.len() && s.is_ascii() {
            return ascii_units_eq(self.as_units(), s);
        }
        self.as_units().iter().copied().eq(s.encode_utf16())
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
        f.debug_tuple("JsString").field(&self.as_units()).finish()
    }
}

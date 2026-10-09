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

//! Rust-only, not in Java (DECISIONS.md D-025): a fast non-cryptographic hasher for hash tables
//! on the compiler's hot paths. The std default (SipHash with a random key) resists hash
//! flooding, which a compiler reading its own inputs does not need, and costs several times
//! more per key. Insertion-ordered maps (`IndexMap`, `IndexSet`) iterate in insertion order
//! whatever the hasher, so switching their hasher never changes output.
//!
//! Each word is mixed in by rotate, xor and a multiply by the 64-bit golden-ratio constant
//! (Knuth's multiplicative hashing); `finish` rotates the well-mixed high bits down to the low
//! bits that pick the hash table bucket.

use std::hash::{BuildHasherDefault, Hasher};

/// The port's `IndexMap` (Java `LinkedHashMap`): `indexmap::IndexMap` with `FastHasher` by
/// default. Construct with `default()` / `from_iter`.
pub type IndexMap<K, V, S = FastBuildHasher> = indexmap::IndexMap<K, V, S>;
/// The port's `IndexSet` (Java `LinkedHashSet`), see `IndexMap`.
pub type IndexSet<T, S = FastBuildHasher> = indexmap::IndexSet<T, S>;
pub type FastBuildHasher = BuildHasherDefault<FastHasher>;

const SEED: u64 = 0x9e37_79b9_7f4a_7c15;

#[derive(Default, Clone, Copy)]
pub struct FastHasher {
    hash: u64,
}

impl FastHasher {
    #[inline]
    fn add_to_hash(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FastHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            self.add_to_hash(u64::from_le_bytes(chunk.try_into().unwrap()));
        }
        let mut rest = chunks.remainder();
        if rest.len() >= 4 {
            self.add_to_hash(u64::from(u32::from_le_bytes(rest[..4].try_into().unwrap())));
            rest = &rest[4..];
        }
        if rest.len() >= 2 {
            self.add_to_hash(u64::from(u16::from_le_bytes(rest[..2].try_into().unwrap())));
            rest = &rest[2..];
        }
        if let Some(&byte) = rest.first() {
            self.add_to_hash(u64::from(byte));
        }
    }
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add_to_hash(u64::from(i));
    }
    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add_to_hash(u64::from(i));
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add_to_hash(u64::from(i));
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add_to_hash(i as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.hash.rotate_left(26)
    }
}

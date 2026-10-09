//! Rust-only, not in Java (DECISIONS.md D-025): a fast non-cryptographic hasher for hash tables
//! on the compiler's hot paths. The std default (SipHash with a random key) resists hash
//! flooding, which a compiler reading its own inputs does not need, and costs several times
//! more per key. Insertion-ordered maps (`IndexMap`, `IndexSet`) iterate in insertion order
//! whatever the hasher, so switching their hasher never changes output.
//!
//! The algorithm is the "Fx" hash of the Rust compiler (rotate, xor, multiply per word).

use std::hash::{BuildHasherDefault, Hasher};

/// An `IndexMap` with the Fx hasher; construct with `default()`.
pub type FxIndexMap<K, V> = indexmap::IndexMap<K, V, FxBuildHasher>;
/// An `IndexSet` with the Fx hasher; construct with `default()`.
pub type FxIndexSet<T> = indexmap::IndexSet<T, FxBuildHasher>;
pub type FxBuildHasher = BuildHasherDefault<FxHasher>;

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

#[derive(Default, Clone, Copy)]
pub struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn add_to_hash(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
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
        self.hash
    }
}

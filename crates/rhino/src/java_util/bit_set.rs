/*
 * Copyright (c) 1995, 2020, Oracle and/or its affiliates. All rights reserved.
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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/util/BitSet.java.

//! The java.util.BitSet subset used by compiler analyses (JDK 21).
use std::{
    fmt,
    sync::atomic::{AtomicUsize, Ordering},
};

// Rust-only (performance divergence): store up to 128 bits inline to avoid heap allocations
// for small compiler analysis bitsets (liveness, reaching definitions, etc.).
const INLINE_WORDS: usize = 2;

#[derive(Debug)]
enum BitSetWords {
    Inline([u64; INLINE_WORDS]),
    Heap(Vec<u64>),
}

impl BitSetWords {
    #[inline]
    fn as_slice(&self) -> &[u64] {
        match self {
            Self::Inline(arr) => arr,
            Self::Heap(vec) => vec,
        }
    }

    #[inline]
    fn as_mut_slice(&mut self) -> &mut [u64] {
        match self {
            Self::Inline(arr) => arr,
            Self::Heap(vec) => vec,
        }
    }
}

#[derive(Debug)]
pub struct BitSet {
    words: BitSetWords,
    words_in_use: usize,
    size_is_sticky: bool,
    capacity: AtomicUsize,
}
const ADDRESS_BITS_PER_WORD: i32 = 6;
const BITS_PER_WORD: i32 = 1 << ADDRESS_BITS_PER_WORD;

impl BitSet {
    // port: BitSet#wordIndex
    fn word_index(bit_index: i32) -> usize {
        (bit_index >> ADDRESS_BITS_PER_WORD) as usize
    }
    // port: BitSet#BitSet()
    pub fn new_default() -> Self {
        // initWords(BITS_PER_WORD)
        let words = Self::word_index(BITS_PER_WORD - 1) + 1;
        Self {
            words: BitSetWords::Inline([0; INLINE_WORDS]),
            words_in_use: 0,
            size_is_sticky: false,
            capacity: AtomicUsize::new(words),
        }
    }
    // port: BitSet#BitSet(int)
    pub fn new(nbits: i32) -> Self {
        assert!(nbits >= 0, "nbits < 0: {nbits}");
        let words_len = (nbits as usize).div_ceil(64);
        let words = if words_len <= INLINE_WORDS {
            BitSetWords::Inline([0; INLINE_WORDS])
        } else {
            BitSetWords::Heap(vec![0; words_len])
        };
        Self {
            words,
            words_in_use: 0,
            size_is_sticky: true,
            capacity: AtomicUsize::new(words_len),
        }
    }
    // port: BitSet#size
    pub fn size(&self) -> i32 {
        (self.capacity.load(Ordering::Relaxed) as i32).wrapping_mul(64)
    }
    // port: BitSet#get(int)
    pub fn get(&self, bit_index: i32) -> bool {
        assert!(bit_index >= 0, "bitIndex < 0: {bit_index}");
        let word_index = bit_index as usize >> 6;
        word_index < self.words_in_use
            && (self.words.as_slice()[word_index] & (1u64 << (bit_index & 63))) != 0
    }
    // port: BitSet#ensureCapacity
    fn ensure_capacity(&mut self, words_required: usize) {
        let capacity = self.capacity.load(Ordering::Relaxed);
        if capacity < words_required {
            let new_capacity = (2 * capacity).max(words_required);
            match &mut self.words {
                BitSetWords::Inline(arr) => {
                    if new_capacity > INLINE_WORDS {
                        let mut vec = vec![0; new_capacity];
                        vec[..self.words_in_use].copy_from_slice(&arr[..self.words_in_use]);
                        self.words = BitSetWords::Heap(vec);
                    }
                }
                BitSetWords::Heap(vec) => {
                    vec.resize(new_capacity, 0);
                }
            }
            self.capacity.store(new_capacity, Ordering::Relaxed);
            self.size_is_sticky = false;
        }
    }
    // port: BitSet#expandTo
    fn expand_to(&mut self, word_index: usize) {
        let words_required = word_index + 1;
        if self.words_in_use < words_required {
            self.ensure_capacity(words_required);
            self.words_in_use = words_required;
        }
    }
    // port: BitSet#recalculateWordsInUse
    fn recalculate_words_in_use(&mut self) {
        let slice = self.words.as_slice();
        while self.words_in_use > 0 && slice[self.words_in_use - 1] == 0 {
            self.words_in_use -= 1;
        }
    }
    // port: BitSet#set(int)
    pub fn set(&mut self, bit_index: i32) {
        assert!(bit_index >= 0, "bitIndex < 0: {bit_index}");
        let word_index = bit_index as usize >> 6;
        self.expand_to(word_index);
        self.words.as_mut_slice()[word_index] |= 1u64 << (bit_index & 63);
    }
    // port: BitSet#or
    pub fn or(&mut self, set: &Self) {
        let words_in_common = self.words_in_use.min(set.words_in_use);
        if self.words_in_use < set.words_in_use {
            self.ensure_capacity(set.words_in_use);
            self.words_in_use = set.words_in_use;
        }
        let dst = self.words.as_mut_slice();
        let src = set.words.as_slice();
        for i in 0..words_in_common {
            dst[i] |= src[i];
        }
        dst[words_in_common..set.words_in_use]
            .copy_from_slice(&src[words_in_common..set.words_in_use]);
    }
    // port: BitSet#andNot
    pub fn and_not(&mut self, set: &Self) {
        let n = self.words_in_use.min(set.words_in_use);
        let dst = self.words.as_mut_slice();
        let src = set.words.as_slice();
        for i in (0..n).rev() {
            dst[i] &= !src[i];
        }
        self.recalculate_words_in_use();
    }
    // port: BitSet#nextSetBit
    pub fn next_set_bit(&self, from_index: i32) -> i32 {
        assert!(from_index >= 0, "fromIndex < 0: {from_index}");
        let mut u = from_index as usize >> 6;
        if u >= self.words_in_use {
            return -1;
        }
        let slice = self.words.as_slice();
        let mut word = slice[u] & (u64::MAX << (from_index & 63));
        loop {
            if word != 0 {
                return (u as i32 * 64).wrapping_add(word.trailing_zeros() as i32);
            }
            u += 1;
            if u == self.words_in_use {
                return -1;
            }
            word = slice[u];
        }
    }
    // port: BitSet#intersects
    pub fn intersects(&self, set: &Self) -> bool {
        let a = self.words.as_slice();
        let b = set.words.as_slice();
        for i in (0..self.words_in_use.min(set.words_in_use)).rev() {
            if (a[i] & b[i]) != 0 {
                return true;
            }
        }
        false
    }
    // port: BitSet#cardinality
    pub fn cardinality(&self) -> i32 {
        self.words.as_slice()[..self.words_in_use]
            .iter()
            .map(|w| w.count_ones() as i32)
            .sum()
    }
    // port: BitSet#isEmpty
    pub fn is_empty(&self) -> bool {
        self.words_in_use == 0
    }
    // port: BitSet#clear()
    pub fn clear(&mut self) {
        let slice = self.words.as_mut_slice();
        while self.words_in_use > 0 {
            self.words_in_use -= 1;
            slice[self.words_in_use] = 0;
        }
    }
    // port: BitSet#clear(int)
    pub fn clear_bit(&mut self, bit_index: i32) {
        assert!(bit_index >= 0, "bitIndex < 0: {bit_index}");
        let word_index = bit_index as usize >> 6;
        if word_index >= self.words_in_use {
            return;
        }
        self.words.as_mut_slice()[word_index] &= !(1u64 << (bit_index & 63));
        self.recalculate_words_in_use();
    }
    // port: BitSet#hashCode
    pub fn hash_code(&self) -> i32 {
        let slice = self.words.as_slice();
        let mut h = 1234u64;
        for i in (0..self.words_in_use).rev() {
            h ^= slice[i].wrapping_mul(i as u64 + 1);
        }
        ((h >> 32) ^ h) as i32
    }
}
impl Clone for BitSet {
    // port: BitSet#clone
    fn clone(&self) -> Self {
        // JDK clone trims the source's non-sticky capacity as well. The atomic
        // capacity preserves that observable side effect through a shared reference.
        let capacity = if self.size_is_sticky {
            self.capacity.load(Ordering::Relaxed)
        } else {
            self.capacity.store(self.words_in_use, Ordering::Relaxed);
            self.words_in_use
        };
        let src = self.words.as_slice();
        let words = if capacity <= INLINE_WORDS {
            let mut arr = [0u64; INLINE_WORDS];
            arr[..self.words_in_use].copy_from_slice(&src[..self.words_in_use]);
            BitSetWords::Inline(arr)
        } else {
            let mut vec = vec![0; capacity];
            vec[..self.words_in_use].copy_from_slice(&src[..self.words_in_use]);
            BitSetWords::Heap(vec)
        };
        Self {
            words,
            words_in_use: self.words_in_use,
            size_is_sticky: self.size_is_sticky,
            capacity: AtomicUsize::new(capacity),
        }
    }
}
impl PartialEq for BitSet {
    // port: BitSet#equals
    fn eq(&self, other: &Self) -> bool {
        self.words_in_use == other.words_in_use
            && self.words.as_slice()[..self.words_in_use]
                == other.words.as_slice()[..other.words_in_use]
    }
}
impl Eq for BitSet {}
impl std::hash::Hash for BitSet {
    // port: BitSet#hashCode
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hash_code().hash(state);
    }
}
impl fmt::Display for BitSet {
    // port: BitSet#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{")?;
        let mut i = self.next_set_bit(0);
        let mut first = true;
        while i >= 0 {
            if !first {
                f.write_str(", ")?;
            }
            write!(f, "{i}")?;
            first = false;
            if i == i32::MAX {
                break;
            }
            i = self.next_set_bit(i + 1);
        }
        f.write_str("}")
    }
}

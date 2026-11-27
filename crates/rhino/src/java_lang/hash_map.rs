/*
 * Copyright (c) 1997, 2023, Oracle and/or its affiliates. All rights reserved.
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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/util/HashMap.java.

//! Java `java.util.HashMap` iteration order (docs/PORTING.md §8: output that depends on the
//! iteration order of a `HashMap` keyed by `String` ports Java's exact order).
//!
//! For a map that only ever receives insertions, Java iterates its table bucket by bucket; a
//! bucket's entries keep insertion order (new entries are appended to the bin, and `resize`
//! splits each bin into its low and high halves preserving relative order). The final order is
//! therefore a stable sort of the entries, in insertion order, by their bucket index in the final
//! table. Tree bins (a bin of more than `TREEIFY_THRESHOLD` entries in a table of at least
//! `MIN_TREEIFY_CAPACITY` buckets) link later insertions after their tree parent instead;
//! `iteration_order` reports such a table with a panic rather than guessing.

/// `HashMap.DEFAULT_LOAD_FACTOR`.
const DEFAULT_LOAD_FACTOR: f32 = 0.75;
/// `HashMap.MAXIMUM_CAPACITY`.
const MAXIMUM_CAPACITY: i32 = 1 << 30;
/// `HashMap.TREEIFY_THRESHOLD`.
const TREEIFY_THRESHOLD: usize = 8;
/// `HashMap.MIN_TREEIFY_CAPACITY`.
const MIN_TREEIFY_CAPACITY: i32 = 64;

// port: HashMap#hash
pub fn hash(hash_code: i32) -> i32 {
    hash_code ^ ((hash_code as u32) >> 16) as i32
}

// port: HashMap#tableSizeFor
pub fn table_size_for(cap: i32) -> i32 {
    // Java masks the shift distance of `-1 >>> numberOfLeadingZeros(cap - 1)` to 5 bits.
    let n = ((-1_i32 as u32) >> (cap.wrapping_sub(1).leading_zeros() & 31)) as i32;
    if n < 0 {
        1
    } else if n >= MAXIMUM_CAPACITY {
        MAXIMUM_CAPACITY
    } else {
        n + 1
    }
}

/// The table length of `new HashMap<>(initial_capacity)` after `size` insertions of distinct
/// keys (`HashMap#putVal` resizes when `++size > threshold`; `HashMap#resize` doubles the table
/// and its threshold).
// port: HashMap#resize
pub fn table_length_after(initial_capacity: i32, size: usize) -> i32 {
    // The constructor stores tableSizeFor(initialCapacity) as the initial threshold, which the
    // first resize uses as the capacity.
    let mut cap = table_size_for(initial_capacity);
    let mut threshold = (cap as f32 * DEFAULT_LOAD_FACTOR) as i32;
    for inserted in 1..=size {
        if inserted as i64 > threshold as i64 && cap < MAXIMUM_CAPACITY {
            let old_cap = cap;
            cap <<= 1;
            threshold = if cap < MAXIMUM_CAPACITY && old_cap >= 16 {
                threshold << 1 // double threshold
            } else {
                (cap as f32 * DEFAULT_LOAD_FACTOR) as i32
            };
        }
    }
    cap
}

/// The iteration order of `new HashMap<>(initial_capacity)` (or of a Guava multimap backed by
/// one) whose distinct keys were inserted in `entries` order, never removed. `hash_code` is the
/// key's Java `hashCode()`.
// port: HashMap.HashIterator#nextNode
pub fn iteration_order<T>(
    entries: Vec<T>,
    hash_code: impl Fn(&T) -> i32,
    initial_capacity: i32,
) -> Vec<T> {
    let n = table_length_after(initial_capacity, entries.len());
    // Rust-only (performance divergence): a counting sort by bucket, stable as the sort below,
    // when the table is not much larger than the entries (the usual case: the table grows with
    // them). It counts the bins for the tree-bin check without a map.
    if (n as usize) <= entries.len().saturating_mul(4).saturating_add(64) {
        let buckets: Vec<u32> = entries
            .iter()
            .map(|entry| (hash(hash_code(entry)) & (n - 1)) as u32)
            .collect();
        let mut starts = vec![0usize; n as usize + 1];
        for &bucket in &buckets {
            starts[bucket as usize + 1] += 1;
        }
        if n >= MIN_TREEIFY_CAPACITY {
            assert!(
                starts.iter().all(|count| *count <= TREEIFY_THRESHOLD),
                "HashMap tree-bin iteration order is not ported"
            );
        }
        for i in 1..starts.len() {
            starts[i] += starts[i - 1];
        }
        let mut placed: Vec<Option<T>> = (0..entries.len()).map(|_| None).collect();
        for (entry, &bucket) in entries.into_iter().zip(&buckets) {
            placed[starts[bucket as usize]] = Some(entry);
            starts[bucket as usize] += 1;
        }
        return placed
            .into_iter()
            .map(|entry| entry.expect("every slot is filled"))
            .collect();
    }
    let mut keyed: Vec<(i32, T)> = entries
        .into_iter()
        .map(|entry| (hash(hash_code(&entry)) & (n - 1), entry))
        .collect();
    if n >= MIN_TREEIFY_CAPACITY {
        let mut counts = std::collections::BTreeMap::new();
        for (index, _) in &keyed {
            *counts.entry(*index).or_insert(0_usize) += 1;
        }
        assert!(
            counts.values().all(|count| *count <= TREEIFY_THRESHOLD),
            "HashMap tree-bin iteration order is not ported"
        );
    }
    keyed.sort_by_key(|(index, _)| *index);
    keyed.into_iter().map(|(_, entry)| entry).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stable sort by bucket that `iteration_order` stands for.
    fn by_sort(entries: &[i32], initial_capacity: i32) -> Vec<i32> {
        let n = table_length_after(initial_capacity, entries.len());
        let mut keyed: Vec<(i32, i32)> = entries.iter().map(|&e| (hash(e) & (n - 1), e)).collect();
        keyed.sort_by_key(|(index, _)| *index);
        keyed.into_iter().map(|(_, e)| e).collect()
    }

    #[test]
    fn counting_sort_keeps_the_stable_bucket_order() {
        let mut seed: u32 = 12345;
        for len in [0usize, 1, 5, 12, 13, 100, 1000, 5000] {
            let entries: Vec<i32> = (0..len)
                .map(|_| {
                    seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                    (seed >> 3) as i32
                })
                .collect();
            for capacity in [16, 64, 1 << 12] {
                assert_eq!(
                    iteration_order(entries.clone(), |&e| e, capacity),
                    by_sort(&entries, capacity),
                    "{len} entries, initial capacity {capacity}"
                );
            }
        }
    }
}

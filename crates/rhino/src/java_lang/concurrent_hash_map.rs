/*
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
/*
 * This file is available under and governed by the GNU General Public
 * License version 2 only, as published by the Free Software Foundation.
 * However, the following notice accompanied the original version of this
 * file:
 *
 * Written by Doug Lea with assistance from members of JCP JSR-166
 * Expert Group and released to the public domain, as explained at
 * http://creativecommons.org/publicdomain/zero/1.0/
 */
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1):
//   java.base/java/util/concurrent/ConcurrentHashMap.java.

//! Java `java.util.concurrent.ConcurrentHashMap` iteration order (docs/PORTING.md §8: output
//! that depends on the iteration order of a hash map keyed by `String` ports Java's exact order).
//!
//! `ConcurrentHashMap` orders its entries differently from `java.util.HashMap`: `spread` masks
//! the hash to 31 bits, the table grows when the count *reaches* `sizeCtl`, `putAll` presizes the
//! table first, `treeifyBin` on a small table grows it (possibly several doublings) instead of
//! building a tree, `transfer` rebuilds each bin with the `lastRun` optimisation (the nodes before
//! the bin's last same-half run are prepended, so they end up reversed in front of it), and a tree
//! bin links each new node at the front of its `first` list. [`ConcurrentHashMap`] replays the
//! table of a map used by one thread that only receives `put` and `putAll` (no removals), and
//! iterates it bucket by bucket like `ConcurrentHashMap.Traverser`.
//!
//! Lookups go through an insertion-ordered [`IndexMap`]; the bins hold the entries' positions in
//! it, so only iteration follows the replayed table.

use super::JavaHashCode;
use crate::fast_hash::IndexMap;
use std::borrow::Borrow;
use std::hash::Hash;

/// `ConcurrentHashMap.MAXIMUM_CAPACITY`.
const MAXIMUM_CAPACITY: i32 = 1 << 30;
/// `ConcurrentHashMap.DEFAULT_CAPACITY`.
const DEFAULT_CAPACITY: i32 = 16;
/// `ConcurrentHashMap.TREEIFY_THRESHOLD`.
const TREEIFY_THRESHOLD: usize = 8;
/// `ConcurrentHashMap.UNTREEIFY_THRESHOLD`.
const UNTREEIFY_THRESHOLD: usize = 6;
/// `ConcurrentHashMap.MIN_TREEIFY_CAPACITY`.
const MIN_TREEIFY_CAPACITY: usize = 64;
/// `ConcurrentHashMap.HASH_BITS`.
const HASH_BITS: i32 = 0x7fff_ffff;

// port: ConcurrentHashMap#spread
pub fn spread(h: i32) -> i32 {
    (h ^ ((h as u32) >> 16) as i32) & HASH_BITS
}

// port: ConcurrentHashMap#tableSizeFor
fn table_size_for(c: i32) -> i32 {
    // Java masks the shift distance of `-1 >>> numberOfLeadingZeros(c - 1)` to 5 bits.
    let n = ((-1_i32 as u32) >> (c.wrapping_sub(1).leading_zeros() & 31)) as i32;
    if n < 0 {
        1
    } else if n >= MAXIMUM_CAPACITY {
        MAXIMUM_CAPACITY
    } else {
        n + 1
    }
}

/// One table slot: the nodes of its bin in `next` order (a `TreeBin` in `first` order).
#[derive(Clone, Debug, Default)]
struct Bin {
    nodes: Vec<usize>,
    tree: bool,
}

/// A `new ConcurrentHashMap<>()` that only receives `put` and `putAll` from one thread.
#[derive(Clone, Debug)]
pub struct ConcurrentHashMap<K, V> {
    /// The mappings in insertion order; a node is its entry's index here.
    entries: IndexMap<K, V>,
    /// `Node#hash` (the spread hash) of every node.
    hashes: Vec<i32>,
    table: Vec<Bin>,
    size_ctl: i32,
}

impl<K, V> Default for ConcurrentHashMap<K, V> {
    // port: ConcurrentHashMap#ConcurrentHashMap()
    fn default() -> Self {
        Self {
            entries: IndexMap::default(),
            hashes: Vec::new(),
            table: Vec::new(),
            size_ctl: 0,
        }
    }
}

impl<K: Hash + Eq + JavaHashCode, V> ConcurrentHashMap<K, V> {
    // port: ConcurrentHashMap#ConcurrentHashMap()
    pub fn new() -> Self {
        Self::default()
    }

    // port: ConcurrentHashMap#size
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    // port: ConcurrentHashMap#isEmpty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    // port: ConcurrentHashMap#get
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.entries.get(key)
    }

    // port: ConcurrentHashMap#containsKey
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.entries.contains_key(key)
    }

    // port: ConcurrentHashMap#put
    pub fn put(&mut self, key: K, value: V) -> Option<V> {
        self.put_val(key, value)
    }

    /// `putAll` of a map whose entries iterate in `m`'s order (an `ImmutableMap`, a
    /// `LinkedHashMap`).
    // port: ConcurrentHashMap#putAll
    pub fn put_all<I>(&mut self, m: I)
    where
        I: IntoIterator<Item = (K, V)>,
        I::IntoIter: ExactSizeIterator,
    {
        let m = m.into_iter();
        self.try_presize(m.len() as i32);
        for (key, value) in m {
            self.put_val(key, value);
        }
    }

    /// The mappings in Java's iteration order.
    // port: ConcurrentHashMap.Traverser#advance
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.table
            .iter()
            .flat_map(|bin| bin.nodes.iter())
            .map(|&node| self.entries.get_index(node).expect("node of an entry"))
    }

    // port: ConcurrentHashMap#keySet
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(key, _)| key)
    }

    // port: ConcurrentHashMap#values
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, value)| value)
    }

    fn bin_index(&self, hash: i32) -> usize {
        hash as usize & (self.table.len() - 1)
    }

    // port: ConcurrentHashMap#putVal
    fn put_val(&mut self, key: K, value: V) -> Option<V> {
        let hash = spread(key.hash_code());
        if self.table.is_empty() {
            self.init_table();
        }
        let i = self.bin_index(hash);
        let existing = self.entries.get_index_of(&key);
        let bin_count;
        let mut old_val = None;
        let bin = &self.table[i];
        if bin.nodes.is_empty() {
            // no lock when adding to empty bin
            let node = self.new_node(hash, key, value);
            self.table[i].nodes.push(node);
            bin_count = 0;
        } else if !bin.tree {
            match existing {
                Some(node) => {
                    let position = bin.nodes.iter().position(|&n| n == node);
                    bin_count = position.expect("existing key in its bin") + 1;
                    old_val = Some(self.replace_value(node, value));
                }
                None => {
                    bin_count = bin.nodes.len();
                    let node = self.new_node(hash, key, value);
                    self.table[i].nodes.push(node);
                }
            }
        } else {
            // TreeBin#putTreeVal links a new node at the front of `first`.
            bin_count = 2;
            match existing {
                Some(node) => old_val = Some(self.replace_value(node, value)),
                None => {
                    let node = self.new_node(hash, key, value);
                    self.table[i].nodes.insert(0, node);
                }
            }
        }
        if bin_count != 0 {
            if bin_count >= TREEIFY_THRESHOLD {
                self.treeify_bin(i);
            }
            if old_val.is_some() {
                return old_val;
            }
        }
        self.add_count(bin_count as i32);
        None
    }

    fn new_node(&mut self, hash: i32, key: K, value: V) -> usize {
        let node = self.entries.len();
        self.entries.insert(key, value);
        self.hashes.push(hash);
        node
    }

    fn replace_value(&mut self, node: usize, value: V) -> V {
        let (_, slot) = self.entries.get_index_mut(node).expect("node of an entry");
        std::mem::replace(slot, value)
    }

    // port: ConcurrentHashMap#initTable
    fn init_table(&mut self) {
        let sc = self.size_ctl;
        let n = if sc > 0 { sc } else { DEFAULT_CAPACITY };
        self.table = vec![Bin::default(); n as usize];
        self.size_ctl = n - ((n as u32) >> 2) as i32;
    }

    /// `addCount(1L, check)`: with one thread the count cell is never contended.
    // port: ConcurrentHashMap#addCount
    fn add_count(&mut self, check: i32) {
        let s = self.entries.len() as i64;
        if check >= 0 {
            while s >= self.size_ctl as i64 && (self.table.len() as i32) < MAXIMUM_CAPACITY {
                self.transfer();
            }
        }
    }

    // port: ConcurrentHashMap#tryPresize
    fn try_presize(&mut self, size: i32) {
        let c = if size >= ((MAXIMUM_CAPACITY as u32) >> 1) as i32 {
            MAXIMUM_CAPACITY
        } else {
            table_size_for(size + ((size as u32) >> 1) as i32 + 1)
        };
        while self.size_ctl >= 0 {
            let sc = self.size_ctl;
            if self.table.is_empty() {
                let n = if sc > c { sc } else { c };
                self.table = vec![Bin::default(); n as usize];
                self.size_ctl = n - ((n as u32) >> 2) as i32;
            } else {
                let n = self.table.len() as i32;
                if c <= sc || n >= MAXIMUM_CAPACITY {
                    break;
                }
                self.transfer();
            }
        }
    }

    /// Doubles the table. Bins are independent, so the order Java's (possibly several) threads
    /// process them in does not matter.
    // port: ConcurrentHashMap#transfer
    fn transfer(&mut self) {
        let n = self.table.len();
        let mut next_tab = vec![Bin::default(); n << 1];
        let tab = std::mem::take(&mut self.table);
        for (i, f) in tab.into_iter().enumerate() {
            if f.nodes.is_empty() {
                continue;
            }
            let high = |node: usize| self.hashes[node] as usize & n != 0;
            let (ln, hn) = if !f.tree {
                let mut run_bit = high(f.nodes[0]);
                let mut last_run = 0;
                for (p, &node) in f.nodes.iter().enumerate().skip(1) {
                    let b = high(node);
                    if b != run_bit {
                        run_bit = b;
                        last_run = p;
                    }
                }
                // `new Node<K,V>(ph, pk, pv, ln)` prepends: the nodes before lastRun end up
                // reversed in front of the lastRun chain.
                let mut ln = Vec::new();
                let mut hn = Vec::new();
                for &node in f.nodes[..last_run].iter().rev() {
                    if high(node) {
                        hn.push(node);
                    } else {
                        ln.push(node);
                    }
                }
                if run_bit {
                    hn.extend_from_slice(&f.nodes[last_run..]);
                } else {
                    ln.extend_from_slice(&f.nodes[last_run..]);
                }
                (
                    Bin {
                        nodes: ln,
                        tree: false,
                    },
                    Bin {
                        nodes: hn,
                        tree: false,
                    },
                )
            } else {
                let (hi, lo): (Vec<usize>, Vec<usize>) =
                    f.nodes.iter().partition(|&&node| high(node));
                // untreeify(lo) when lc <= UNTREEIFY_THRESHOLD, else a TreeBin.
                let lo_tree = lo.len() > UNTREEIFY_THRESHOLD;
                let hi_tree = hi.len() > UNTREEIFY_THRESHOLD;
                (
                    Bin {
                        nodes: lo,
                        tree: lo_tree,
                    },
                    Bin {
                        nodes: hi,
                        tree: hi_tree,
                    },
                )
            };
            next_tab[i] = ln;
            next_tab[i + n] = hn;
        }
        self.table = next_tab;
        self.size_ctl = ((n << 1) - (n >> 1)) as i32;
    }

    // port: ConcurrentHashMap#treeifyBin
    fn treeify_bin(&mut self, index: usize) {
        let n = self.table.len();
        if n < MIN_TREEIFY_CAPACITY {
            self.try_presize((n << 1) as i32);
        } else {
            let bin = &mut self.table[index];
            if !bin.nodes.is_empty() && !bin.tree {
                bin.tree = true;
            }
        }
    }
}

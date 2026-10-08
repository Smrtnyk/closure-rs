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

//! The `java.util.HashMap` table behind [`JavaHashMultimap`]: hashing, resizing and the bins
//! that give its keys Java's iteration order.

use super::JavaHashMultimap;
use closure_rhino::js_string::JsString;

/// `HashMap.DEFAULT_LOAD_FACTOR`.
const DEFAULT_LOAD_FACTOR: f32 = 0.75;
/// `HashMap.MAXIMUM_CAPACITY`.
const MAXIMUM_CAPACITY: usize = 1 << 30;
/// `HashMap.TREEIFY_THRESHOLD`.
const TREEIFY_THRESHOLD: usize = 8;
/// `HashMap.MIN_TREEIFY_CAPACITY`.
const MIN_TREEIFY_CAPACITY: usize = 64;
/// The table length of the `HashMap` behind `HashMultimap.create()` (Guava sizes it for 12
/// expected keys; the reference jar allocates a table of 16 on the first put).
const HASH_MULTIMAP_INITIAL_TABLE_LENGTH: usize = 16;

impl<V: Copy + Eq> JavaHashMultimap<V> {
    // port: HashMap#hash
    fn hash(key: &JsString) -> i32 {
        let h = key.hash_code();
        h ^ ((h as u32) >> 16) as i32
    }

    fn index_for(key: &JsString, n: usize) -> usize {
        (Self::hash(key) as u32 as usize) & (n - 1)
    }

    // port: HashMap#resize
    fn resize(&mut self) {
        let old_cap = self.table.len();
        let (new_cap, new_thr);
        if old_cap > 0 {
            if old_cap >= MAXIMUM_CAPACITY {
                self.threshold = i32::MAX as usize;
                return;
            }
            new_cap = old_cap << 1;
            new_thr = if new_cap < MAXIMUM_CAPACITY && old_cap >= 16 {
                self.threshold << 1 // double threshold
            } else {
                (new_cap as f32 * DEFAULT_LOAD_FACTOR) as usize
            };
        } else {
            // initial capacity was placed in threshold
            new_cap = HASH_MULTIMAP_INITIAL_TABLE_LENGTH;
            new_thr = (new_cap as f32 * DEFAULT_LOAD_FACTOR) as usize;
        }
        self.threshold = new_thr;
        let old_tab = std::mem::take(&mut self.table);
        self.table = vec![Vec::new(); new_cap];
        // Each bin splits into its low and high halves, preserving relative order.
        for bin in old_tab {
            for key in bin {
                let index = Self::index_for(&key, new_cap);
                self.table[index].push(key);
            }
        }
    }

    // port: HashMap#putVal (absent key)
    pub(super) fn put_key(&mut self, key: JsString) {
        if self.table.is_empty() {
            self.resize();
        }
        let n = self.table.len();
        let index = Self::index_for(&key, n);
        let bin_count = self.table[index].len();
        self.table[index].push(key);
        if bin_count > TREEIFY_THRESHOLD - 1 {
            // port: HashMap#treeifyBin
            if n < MIN_TREEIFY_CAPACITY {
                self.resize();
            } else {
                panic!("HashMap tree-bin iteration order is not ported");
            }
        }
        if self.values.len() > self.threshold {
            self.resize();
        }
    }

    /// Unlinks an existing key from its bin.
    // port: HashMap#removeNode
    pub(super) fn remove_node(&mut self, key: &JsString) {
        let index = Self::index_for(key, self.table.len());
        let bin = &mut self.table[index];
        let position = bin.iter().position(|k| k == key).unwrap();
        bin.remove(position);
    }
}

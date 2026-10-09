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

//! The `java.util.HashMap` behaviour of [`MutatedNames`]: its table size and key iteration order.

use super::MutatedNames;
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::js_string::JsString;

impl MutatedNames {
    // port: HashMap#HashMap(int) via Maps#newHashMapWithExpectedSize(8)
    pub(super) fn new() -> Self {
        Self {
            entries: IndexMap::<_, _>::default(),
            table_capacity: 16,
        }
    }

    /// `HashMap#putVal` after inserting a new key: `if (++size > threshold) resize();`.
    // port: HashMap#putVal
    pub(super) fn grow_table_after_new_key(&mut self) {
        if self.entries.len() > self.table_capacity * 3 / 4 {
            self.table_capacity *= 2;
        }
    }

    // port: HashMap#hash
    fn spread(key: &JsString) -> i32 {
        let h = key.hash_code();
        h ^ ((h as u32) >> 16) as i32
    }

    // port: HashMap.KeySet#iterator
    pub(super) fn key_set(&self) -> Vec<JsString> {
        let mask = self.table_capacity - 1;
        let mut keys: Vec<(usize, usize, &JsString)> = self
            .entries
            .keys()
            .enumerate()
            .map(|(i, k)| ((Self::spread(k) as u32 as usize) & mask, i, k))
            .collect();
        keys.sort();
        keys.into_iter().map(|(_, _, k)| k.clone()).collect()
    }
}

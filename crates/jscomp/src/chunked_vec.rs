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

//! Rust-only, not in Java (DECISIONS.md D-025): an append-only vector stored in fixed-size
//! chunks. The scope arenas grow by hundreds of thousands of entries over a compilation (every
//! pass creates its scopes anew, Java's garbage collector frees them); a chunked vector grows
//! without moving the entries it already holds, where a `Vec` copies them all at each doubling.

use std::ops::{Index, IndexMut};

const CHUNK_BITS: u32 = 12;
const CHUNK_LEN: usize = 1 << CHUNK_BITS;

pub(crate) struct ChunkedVec<T> {
    chunks: Vec<Vec<T>>,
    len: usize,
}

impl<T> Default for ChunkedVec<T> {
    fn default() -> Self {
        Self {
            chunks: Vec::new(),
            len: 0,
        }
    }
}

impl<T> ChunkedVec<T> {
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn push(&mut self, value: T) {
        if self.len % CHUNK_LEN == 0 {
            self.chunks.push(Vec::with_capacity(CHUNK_LEN));
        }
        self.chunks.last_mut().unwrap().push(value);
        self.len += 1;
    }

    pub(crate) fn get(&self, index: usize) -> Option<&T> {
        self.chunks
            .get(index >> CHUNK_BITS)?
            .get(index & (CHUNK_LEN - 1))
    }

    /// The entries from `start` on, in order.
    pub(crate) fn iter_from(&self, start: usize) -> impl Iterator<Item = &T> {
        (start..self.len).map(move |i| &self[i])
    }
}

impl<T> Index<usize> for ChunkedVec<T> {
    type Output = T;
    #[inline]
    fn index(&self, index: usize) -> &T {
        &self.chunks[index >> CHUNK_BITS][index & (CHUNK_LEN - 1)]
    }
}

impl<T> IndexMut<usize> for ChunkedVec<T> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.chunks[index >> CHUNK_BITS][index & (CHUNK_LEN - 1)]
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for ChunkedVec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter_from(0)).finish()
    }
}

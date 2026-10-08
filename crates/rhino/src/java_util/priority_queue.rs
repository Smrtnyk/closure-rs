/*
 * Copyright (c) 2003, 2019, Oracle and/or its affiliates. All rights reserved.
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
/*
 * Copyright (c) 1997, 2022, Oracle and/or its affiliates. All rights reserved.
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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1):
//   java.base/java/util/AbstractCollection.java, java.base/java/util/AbstractQueue.java,
//   java.base/java/util/PriorityQueue.java.

//! The java.util.PriorityQueue subset (comparator-ordered binary heap) used by compiler analyses
//! (JDK 21).
//!
//! Java calls the comparator only when two queued elements are compared, so a comparator that
//! throws for some element (ControlFlowAnalysis#priorityComparator's checkNotNull) fails only
//! where Java fails. Rust's `BinaryHeap` would need every key up front. The comparator borrows
//! state its owner mutates between calls, so it is passed to each operation instead of being
//! stored.
use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub struct PriorityQueue<E> {
    queue: Vec<E>,
}
impl<E: Clone> PriorityQueue<E> {
    // port: PriorityQueue#PriorityQueue(int,Comparator)
    pub fn new(initial_capacity: i32) -> Self {
        // Note: This restriction of at least one is not actually needed,
        // but continues for 1.5 compatibility
        if initial_capacity < 1 {
            panic!("IllegalArgumentException");
        }
        Self {
            queue: Vec::with_capacity(initial_capacity as usize),
        }
    }
    // port: PriorityQueue#add
    pub fn add(&mut self, e: E, cmp: &mut dyn FnMut(&E, &E) -> Ordering) -> bool {
        self.offer(e, cmp)
    }
    // port: AbstractQueue#addAll
    pub fn add_all(
        &mut self,
        c: impl IntoIterator<Item = E>,
        cmp: &mut dyn FnMut(&E, &E) -> Ordering,
    ) -> bool {
        let mut modified = false;
        for e in c {
            if self.add(e, cmp) {
                modified = true;
            }
        }
        modified
    }
    // port: PriorityQueue#offer
    pub fn offer(&mut self, e: E, cmp: &mut dyn FnMut(&E, &E) -> Ordering) -> bool {
        let i = self.queue.len();
        self.queue.push(e.clone());
        Self::sift_up_using_comparator(i, e, &mut self.queue, cmp);
        true
    }
    // port: PriorityQueue#poll
    pub fn poll(&mut self, cmp: &mut dyn FnMut(&E, &E) -> Ordering) -> Option<E> {
        if self.queue.is_empty() {
            return None;
        }
        let result = self.queue[0].clone();
        let x = self.queue.pop().unwrap();
        let n = self.queue.len();
        if n > 0 {
            Self::sift_down_using_comparator(0, x, &mut self.queue, n, cmp);
        }
        Some(result)
    }
    // port: AbstractQueue#remove
    pub fn remove(&mut self, cmp: &mut dyn FnMut(&E, &E) -> Ordering) -> E {
        match self.poll(cmp) {
            Some(x) => x,
            None => panic!("NoSuchElementException"),
        }
    }
    // port: PriorityQueue#size
    pub fn size(&self) -> i32 {
        self.queue.len() as i32
    }
    // port: AbstractCollection#isEmpty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
    // port: PriorityQueue#siftUpUsingComparator
    fn sift_up_using_comparator(
        mut k: usize,
        x: E,
        es: &mut [E],
        cmp: &mut dyn FnMut(&E, &E) -> Ordering,
    ) {
        while k > 0 {
            let parent = (k - 1) >> 1;
            let e = es[parent].clone();
            if cmp(&x, &e) != Ordering::Less {
                break;
            }
            es[k] = e;
            k = parent;
        }
        es[k] = x;
    }
    // port: PriorityQueue#siftDownUsingComparator
    fn sift_down_using_comparator(
        mut k: usize,
        x: E,
        es: &mut [E],
        n: usize,
        cmp: &mut dyn FnMut(&E, &E) -> Ordering,
    ) {
        // assert n > 0;
        let half = n >> 1;
        while k < half {
            let mut child = (k << 1) + 1;
            let mut c = es[child].clone();
            let right = child + 1;
            if right < n && cmp(&c, &es[right]) == Ordering::Greater {
                child = right;
                c = es[child].clone();
            }
            if cmp(&x, &c) != Ordering::Greater {
                break;
            }
            es[k] = c;
            k = child;
        }
        es[k] = x;
    }
}

#[cfg(test)]
mod tests {
    use super::PriorityQueue;

    #[test]
    fn polls_in_comparator_order() {
        let mut cmp = |a: &i32, b: &i32| a.cmp(b);
        let mut q = PriorityQueue::new(10);
        q.add_all([5, 1, 4, 1, 3, 9, 2], &mut cmp);
        let mut out = Vec::new();
        while !q.is_empty() {
            out.push(q.remove(&mut cmp));
        }
        assert_eq!(out, vec![1, 1, 2, 3, 4, 5, 9]);
    }

    #[test]
    fn single_element_never_calls_the_comparator() {
        let mut cmp = |_: &i32, _: &i32| -> std::cmp::Ordering { panic!("compared") };
        let mut q = PriorityQueue::new(10);
        q.add(7, &mut cmp);
        assert_eq!(q.remove(&mut cmp), 7);
        q.add_all([8], &mut cmp);
        assert_eq!(q.poll(&mut cmp), Some(8));
        assert_eq!(q.poll(&mut cmp), None);
    }
}

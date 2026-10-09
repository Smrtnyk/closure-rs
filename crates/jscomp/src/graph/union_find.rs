/*
 * Copyright 2008 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/graph/UnionFind.java.

use closure_rhino::fast_hash::IndexSet;
pub trait UnionFind<E> {
    type Set;
    // port: UnionFind#add
    fn add(&mut self, e: E);
    // port: UnionFind#union
    fn union(&mut self, a: E, b: E) -> E;
    // port: UnionFind#find
    fn find(&mut self, e: &E) -> E;
    // port: UnionFind#areEquivalent
    fn are_equivalent(&mut self, a: &E, b: &E) -> bool;
    // port: UnionFind#elements
    fn elements(&self) -> IndexSet<E>;
    // port: UnionFind#allEquivalenceClasses
    fn all_equivalence_classes(&mut self) -> Vec<IndexSet<E>>;
    // port: UnionFind#findAll
    fn find_all(&self, value: E) -> Self::Set;
}

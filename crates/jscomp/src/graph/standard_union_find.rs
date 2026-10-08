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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/graph/StandardUnionFind.java.

use super::union_find::UnionFind;
use indexmap::{IndexMap, IndexSet};
use std::{cell::Cell, fmt::Display, hash::Hash};
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct NodeId(usize);
struct Node<E> {
    parent: NodeId,
    element: E,
    rank: i32,
    size: i32,
}
impl<E> Node<E> {
    // port: StandardUnionFind.Node#Node
    fn new(id: NodeId, element: E) -> Self {
        Self {
            parent: id,
            element,
            rank: 0,
            size: 1,
        }
    }
}
pub struct StandardUnionFind<E> {
    value_to_string: fn(&E) -> String,
    elmap: IndexMap<E, NodeId>,
    nodes: Vec<Node<E>>,
}
impl<E: Clone + Eq + Hash + Display> Default for StandardUnionFind<E> {
    fn default() -> Self {
        Self::new()
    }
}
impl<E: Clone + Eq + Hash> StandardUnionFind<E> {
    // port: StandardUnionFind#StandardUnionFind()
    pub fn new() -> Self
    where
        E: Display,
    {
        Self::new_with_value_to_string(|e| e.to_string())
    }
    // port: StandardUnionFind#StandardUnionFind()
    pub fn new_with_value_to_string(value_to_string: fn(&E) -> String) -> Self {
        Self {
            value_to_string,
            elmap: IndexMap::new(),
            nodes: Vec::new(),
        }
    }
    // port: StandardUnionFind#StandardUnionFind(UnionFind)
    pub fn from_union_find<U: UnionFind<E>>(other: &mut U) -> Self
    where
        E: Display,
    {
        let mut result = Self::new();
        for elem in other.elements() {
            result.union(other.find(&elem), elem);
        }
        result
    }
    // port: StandardUnionFind#addAll
    pub fn add_all(&mut self, es: impl IntoIterator<Item = E>) {
        for e in es {
            self.add(e);
        }
    }
    // port: StandardUnionFind#allRepresentatives
    pub fn all_representatives(&self) -> IndexSet<E> {
        self.elmap
            .values()
            .filter(|id| self.nodes[id.0].parent == **id)
            .map(|id| self.nodes[id.0].element.clone())
            .collect()
    }
    // port: StandardUnionFind#findRootOrCreateNode
    fn find_root_or_create_node(&mut self, e: E) -> NodeId {
        if let Some(id) = self.elmap.get(&e).copied() {
            return self.find_root(id);
        }
        let id = NodeId(self.nodes.len());
        self.nodes.push(Node::new(id, e.clone()));
        self.elmap.insert(e, id);
        id
    }
    // port: StandardUnionFind#findRoot
    fn find_root(&mut self, id: NodeId) -> NodeId {
        if self.nodes[id.0].parent != id {
            self.nodes[id.0].parent = self.find_root(self.nodes[id.0].parent);
        }
        self.nodes[id.0].parent
    }
}
impl<E: Clone + Eq + Hash> UnionFind<E> for StandardUnionFind<E> {
    type Set = FindAll<E>;
    // port: StandardUnionFind#add
    fn add(&mut self, e: E) {
        self.union(e.clone(), e);
    }
    // port: StandardUnionFind#union
    fn union(&mut self, a: E, b: E) -> E {
        let na = self.find_root_or_create_node(a);
        let nb = self.find_root_or_create_node(b);
        if na == nb {
            return self.nodes[na.0].element.clone();
        }
        if self.nodes[na.0].rank >= self.nodes[nb.0].rank {
            self.nodes[nb.0].parent = na;
            self.nodes[na.0].size = self.nodes[na.0].size.wrapping_add(self.nodes[nb.0].size);
            if self.nodes[na.0].rank == self.nodes[nb.0].rank {
                self.nodes[na.0].rank = self.nodes[na.0].rank.wrapping_add(1);
            }
            return self.nodes[na.0].element.clone();
        }
        self.nodes[na.0].parent = nb;
        self.nodes[nb.0].size = self.nodes[nb.0].size.wrapping_add(self.nodes[na.0].size);
        let temp = self.nodes[nb.0].element.clone();
        self.nodes[nb.0].element = self.nodes[na.0].element.clone();
        self.nodes[na.0].element = temp;
        self.nodes[nb.0].element.clone()
    }
    // port: StandardUnionFind#find
    fn find(&mut self, e: &E) -> E {
        closure_rhino::check_argument!(
            self.elmap.contains_key(e),
            "Element does not exist: %s",
            (self.value_to_string)(e)
        );
        let root = self.find_root(self.elmap[e]);
        self.nodes[root.0].element.clone()
    }
    // port: StandardUnionFind#areEquivalent
    fn are_equivalent(&mut self, a: &E, b: &E) -> bool {
        let a_rep = self.find(a);
        let b_rep = self.find(b);
        a_rep == b_rep
    }
    // port: StandardUnionFind#elements
    fn elements(&self) -> IndexSet<E> {
        self.elmap.keys().cloned().collect()
    }
    // port: StandardUnionFind#allEquivalenceClasses
    fn all_equivalence_classes(&mut self) -> Vec<IndexSet<E>> {
        let mut groups: IndexMap<NodeId, IndexSet<E>> = IndexMap::new();
        for elem in self.elmap.values().copied().collect::<Vec<_>>() {
            let root = self.find_root(elem);
            groups
                .entry(root)
                .or_default()
                .insert(self.nodes[elem.0].element.clone());
        }
        groups.into_values().collect()
    }
    // port: StandardUnionFind#findAll
    fn find_all(&self, value: E) -> FindAll<E> {
        closure_rhino::check_argument!(
            self.elmap.contains_key(&value),
            "Element does not exist: %s",
            (self.value_to_string)(&value)
        );
        let node_for_value = Cell::new(self.elmap[&value]);
        FindAll {
            value,
            node_for_value,
        }
    }
}
/// Live Java AbstractSet view. The owning union-find is supplied as its arena,
/// allowing this handle to stay alive across subsequent union operations.
pub struct FindAll<E> {
    value: E,
    node_for_value: Cell<NodeId>,
}
impl<E: Clone + Eq + Hash> FindAll<E> {
    // port: StandardUnionFind.findAll.AbstractSet#contains
    pub fn contains(&self, uf: &mut StandardUnionFind<E>, o: &E) -> bool {
        self.is_same_root(uf, o)
    }
    // port: StandardUnionFind.findAll.Predicate#apply
    fn is_same_root(&self, uf: &mut StandardUnionFind<E>, b: &E) -> bool {
        if &self.value == b {
            return true;
        }
        let Some(node_b) = uf.elmap.get(b).copied() else {
            return false;
        };
        let node_value = uf.find_root(self.node_for_value.get());
        self.node_for_value.set(node_value);
        uf.find_root(node_b) == node_value
    }
    // port: StandardUnionFind.findAll.AbstractSet#iterator
    pub fn iterator<'a>(&'a self, uf: &'a mut StandardUnionFind<E>) -> FindAllIterator<'a, E> {
        FindAllIterator {
            view: self,
            uf,
            index: 0,
        }
    }
    // port: StandardUnionFind.findAll.AbstractSet#size
    pub fn size(&self, uf: &mut StandardUnionFind<E>) -> i32 {
        let root = uf.find_root(uf.elmap[&self.value]);
        uf.nodes[root.0].size
    }
}

/// Guava filtering iterator: membership is evaluated when advancing, not at construction.
pub struct FindAllIterator<'a, E> {
    view: &'a FindAll<E>,
    uf: &'a mut StandardUnionFind<E>,
    index: usize,
}
impl<E: Clone + Eq + Hash> Iterator for FindAllIterator<'_, E> {
    type Item = E;
    // port: StandardUnionFind.findAll.AbstractSet#iterator
    fn next(&mut self) -> Option<E> {
        while self.index < self.uf.elmap.len() {
            let value = self.uf.elmap.get_index(self.index).unwrap().0.clone();
            self.index += 1;
            if self.view.is_same_root(self.uf, &value) {
                return Some(value);
            }
        }
        None
    }
}

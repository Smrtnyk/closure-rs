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
//   src/com/google/javascript/jscomp/graph/LowestCommonAncestorFinder.java.

use super::{
    di_graph::{DiGraph, DiGraphNode},
    graph_node::GraphNode,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use std::{
    collections::VecDeque,
    hash::{Hash, Hasher},
    marker::PhantomData,
};
pub trait Factory<N, E, G> {
    fn create(&self, graph: &G) -> LowestCommonAncestorFinder<N, E>;
}
#[derive(Clone, Copy, Eq, Debug)]
struct Color {
    bitset: i32,
}
impl Color {
    const COMMON_COLOR_CACHE: [Self; 2 << 5] = {
        let mut colors = [Self { bitset: 0 }; 2 << 5];
        let mut i = 0;
        while i < colors.len() {
            colors[i] = Self { bitset: i as i32 };
            i += 1;
        }
        colors
    };
    const BLANK: Self = Self::COMMON_COLOR_CACHE[0];
    const NOT_LOWEST: Self = Self { bitset: -1 };
    // port: LowestCommonAncestorFinder.Color#Color
    fn new(bitset: i32) -> Self {
        Self { bitset }
    }
    // port: LowestCommonAncestorFinder.Color#create
    fn create(bitset: i32) -> Self {
        if bitset < 0 {
            closure_rhino::check_argument!(bitset == -1);
            Self::NOT_LOWEST
        } else if (bitset as usize) < Self::COMMON_COLOR_CACHE.len() {
            Self::COMMON_COLOR_CACHE[bitset as usize]
        } else {
            Self::new(bitset)
        }
    }
    // port: LowestCommonAncestorFinder.Color#mix
    fn mix(self, other: Self) -> Self {
        if self.bitset == other.bitset {
            self
        } else {
            Self::create(self.bitset | other.bitset)
        }
    }
    // port: LowestCommonAncestorFinder.Color#contains
    fn contains(self, other: Self) -> bool {
        (self.bitset & other.bitset) == other.bitset
    }
}
impl PartialEq for Color {
    // port: LowestCommonAncestorFinder.Color#equals
    fn eq(&self, other: &Self) -> bool {
        self.bitset == other.bitset
    }
}
impl Hash for Color {
    // port: LowestCommonAncestorFinder.Color#hashCode
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.bitset.hash(state);
    }
}
pub struct LowestCommonAncestorFinder<N, E> {
    search_coloring: IndexMap<DiGraphNode, Color>,
    search_queue: VecDeque<DiGraphNode>,
    marker: PhantomData<(N, E)>,
}
impl<N: Clone + Eq + Hash, E> LowestCommonAncestorFinder<N, E> {
    // port: LowestCommonAncestorFinder#LowestCommonAncestorFinder
    pub fn new() -> Self {
        Self {
            search_coloring: IndexMap::<_, _>::default(),
            search_queue: VecDeque::new(),
            marker: PhantomData,
        }
    }
    // port: LowestCommonAncestorFinder#findAll
    pub fn find_all<G: DiGraph<N, E>>(&mut self, graph: &G, roots: &IndexSet<N>) -> IndexSet<N> {
        closure_rhino::check_argument!(roots.len() <= 31, "Too many roots.");
        closure_rhino::check_state!(self.search_coloring.is_empty());
        let all_color = Color::create(1_i32.wrapping_shl(roots.len() as u32).wrapping_sub(1));
        let mut bit_for_root = 1_i32;
        for root in roots {
            let root_node = closure_rhino::check_not_null!(
                graph.get_node(root),
                "Root not present in graph: %s",
                graph.value_to_string(root)
            );
            let color = Color::create(bit_for_root);
            self.search_coloring
                .entry(root_node)
                .and_modify(|c| *c = c.mix(color))
                .or_insert(color);
            self.paint_ancestors(graph, root_node, color);
            bit_for_root = bit_for_root.wrapping_shl(1);
        }
        for node in self.search_coloring.keys().copied().collect::<Vec<_>>() {
            if self.search_coloring[&node] == all_color {
                self.paint_ancestors(graph, node, Color::NOT_LOWEST);
            }
        }
        let mut results = IndexSet::<_>::default();
        for (node, color) in &self.search_coloring {
            if *color == all_color {
                results.insert(node.get_value(graph).clone());
            }
        }
        self.search_coloring.clear();
        self.search_queue.clear();
        results
    }
    // port: LowestCommonAncestorFinder#paintAncestors
    fn paint_ancestors<G: DiGraph<N, E>>(&mut self, graph: &G, root: DiGraphNode, color: Color) {
        closure_rhino::check_state!(self.search_queue.is_empty());
        self.search_queue.push_back(root);
        while let Some(curr) = self.search_queue.pop_front() {
            for parent_edge in curr.get_in_edges(graph) {
                let parent = parent_edge.get_source(graph);
                if parent == root {
                    continue;
                }
                let old_color = self
                    .search_coloring
                    .get(&parent)
                    .copied()
                    .unwrap_or(Color::BLANK);
                if !old_color.contains(color) {
                    self.search_queue.push_back(parent);
                    self.search_coloring.insert(parent, old_color.mix(color));
                }
            }
        }
    }
}
impl<N: Clone + Eq + Hash, E> Default for LowestCommonAncestorFinder<N, E> {
    fn default() -> Self {
        Self::new()
    }
}

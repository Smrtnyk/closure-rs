/*
 * Copyright 2026 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/graph/DominatorTree.java.

use super::{di_graph::DiGraph, graph_node::GraphNode};
use closure_rhino::fast_hash::IndexMap;
use std::hash::Hash;
pub struct DominatorTree<N> {
    idoms: IndexMap<N, N>,
    subtree_sizes: IndexMap<N, i32>,
}
impl<N: Clone + Eq + Hash> DominatorTree<N> {
    // port: DominatorTree#DominatorTree
    fn new(idoms: IndexMap<N, N>, subtree_sizes: IndexMap<N, i32>) -> Self {
        Self {
            idoms,
            subtree_sizes,
        }
    }
    // port: DominatorTree#compute
    pub fn compute<E, G: DiGraph<N, E>>(graph: &G, entry: N) -> Self {
        closure_rhino::check_not_null!(graph.get_node(&entry), "Entry node not in graph");
        let mut post_order = Vec::new();
        let mut post_order_index = IndexMap::<_, _>::default();
        Self::build_post_order(
            graph,
            entry.clone(),
            &mut post_order,
            &mut IndexMap::<_, _>::default(),
        );
        let num_nodes = post_order.len();
        let reverse_post_order = Self::to_reversed_array(&post_order);
        for (i, node) in post_order.iter().enumerate() {
            post_order_index.insert(node.clone(), i);
        }
        let mut idom_indexes = vec![-1; num_nodes];
        let start_node_index = post_order_index[&entry];
        idom_indexes[start_node_index] = start_node_index as i32;
        let mut changed = true;
        while changed {
            changed = false;
            for node in &reverse_post_order {
                if node == &entry {
                    continue;
                }
                let node_idx = post_order_index[node];
                let mut new_idom_idx = -1;
                for pred in graph.get_directed_pred_nodes(node) {
                    if let Some(&pred_idx) = post_order_index.get(pred.get_value(graph))
                        && idom_indexes[pred_idx] != -1
                    {
                        new_idom_idx = pred_idx as i32;
                        break;
                    }
                }
                if new_idom_idx != -1 {
                    for pred in graph.get_directed_pred_nodes(node) {
                        if let Some(&pred_idx) = post_order_index.get(pred.get_value(graph))
                            && idom_indexes[pred_idx] != -1
                        {
                            new_idom_idx =
                                Self::intersect(&idom_indexes, pred_idx as i32, new_idom_idx);
                        }
                    }
                    if idom_indexes[node_idx] != new_idom_idx {
                        idom_indexes[node_idx] = new_idom_idx;
                        changed = true;
                    }
                }
            }
        }
        let mut idoms_map = IndexMap::<_, _>::default();
        let mut children: IndexMap<N, Vec<N>> = IndexMap::<_, _>::default();
        for (i, node) in post_order.iter().enumerate() {
            let idom_idx = idom_indexes[i];
            if idom_idx != -1 && i != start_node_index {
                let idom = post_order[idom_idx as usize].clone();
                idoms_map.insert(node.clone(), idom.clone());
                children.entry(idom).or_default().push(node.clone());
            }
        }
        let mut sizes_map = IndexMap::<_, _>::default();
        Self::compute_subtree_sizes(entry, &children, &mut sizes_map);
        Self::new(idoms_map, sizes_map)
    }
    // port: DominatorTree#intersect
    fn intersect(idom_indexes: &[i32], i: i32, j: i32) -> i32 {
        let mut finger1 = i;
        let mut finger2 = j;
        while finger1 != finger2 {
            while finger1 < finger2 {
                finger1 = idom_indexes[finger1 as usize];
            }
            while finger2 < finger1 {
                finger2 = idom_indexes[finger2 as usize];
            }
        }
        finger1
    }
    // port: DominatorTree#buildPostOrder
    fn build_post_order<E, G: DiGraph<N, E>>(
        graph: &G,
        node: N,
        post_order: &mut Vec<N>,
        visited: &mut IndexMap<N, bool>,
    ) {
        visited.insert(node.clone(), true);
        for succ in graph.get_directed_succ_nodes(&node) {
            let value = succ.get_value(graph);
            if !visited.contains_key(value) {
                Self::build_post_order(graph, value.clone(), post_order, visited);
            }
        }
        post_order.push(node);
    }
    // port: DominatorTree#computeSubtreeSizes
    fn compute_subtree_sizes(
        node: N,
        children: &IndexMap<N, Vec<N>>,
        sizes: &mut IndexMap<N, i32>,
    ) -> i32 {
        let mut size = 1_i32;
        if let Some(node_children) = children.get(&node) {
            for child in node_children {
                size =
                    size.wrapping_add(Self::compute_subtree_sizes(child.clone(), children, sizes));
            }
        }
        sizes.insert(node, size);
        size
    }
    // port: DominatorTree#getImmediateDominator
    pub fn get_immediate_dominator(&self, node: &N) -> Option<&N> {
        self.idoms.get(node)
    }
    // port: DominatorTree#getDominatedSubtreeSize
    pub fn get_dominated_subtree_size(&self, node: &N) -> i32 {
        self.subtree_sizes.get(node).copied().unwrap_or(0)
    }
    // port: DominatorTree#getAllSubtreeSizes
    pub fn get_all_subtree_sizes(&self) -> &IndexMap<N, i32> {
        &self.subtree_sizes
    }
    // port: DominatorTree#toReversedArray
    fn to_reversed_array(list: &[N]) -> Vec<N> {
        let size = list.len();
        let mut reversed = Vec::with_capacity(size);
        for i in 0..size {
            reversed.push(list[size - 1 - i].clone());
        }
        reversed
    }
}

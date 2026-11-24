/*
 * Copyright 2019 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/disambiguate/ClusterPropagator.java.

//! Port of `disambiguate/ClusterPropagator.java`.

use super::{
    color_graph_builder::{ColorGraph, EdgeReason},
    color_graph_node::{
        ColorGraphNodeId, DisambiguateArena, PropAssociation, PropertyClusteringId,
    },
};
use crate::{
    colors::standard_colors,
    graph::{adjacency_graph::AdjacencyGraph, graph_node::GraphNode, union_find::UnionFind},
};
use closure_rhino::fast_hash::IndexSet;
use std::collections::VecDeque;

/// A callback to propagate clusterings across a type graph.
///
/// Java implements `FixedPointGraphTraversal.EdgeCallback<ColorGraphNode, Object>`; the arena
/// holding the nodes and properties is passed to [`ClusterPropagator::traverse_edge`], and the
/// traversal calls it through a closure.
#[derive(Default)]
pub struct ClusterPropagator {}

impl ClusterPropagator {
    // port: ClusterPropagator#ClusterPropagator
    pub fn new() -> Self {
        Self {}
    }

    // port: ClusterPropagator#traverseEdge
    pub fn traverse_edge(
        &mut self,
        arena: &mut DisambiguateArena,
        src: ColorGraphNodeId,
        _unused: Option<EdgeReason>,
        dest: ColorGraphNodeId,
    ) -> bool {
        if src.get_color(arena).ptr_eq(&standard_colors::TOP_OBJECT) {
            // Performance optimization. The for loop below already skips invalidated properties.
            // Colors that are inherently invalidating will have all their properties invalidated
            // already.
            // For example - in a sample large project, TOP_OBJECT had ~4.7k properties and it had
            // ~700k direct subtypes. So the for loop below had to iterate over 4.7k * 700k =
            // ~3.3B properties, just for the TOP_OBJECT node, that were all already invalidated.
            return false;
        }

        let start_dest_prop_count = dest.get_associated_props(arena).len();

        let src_props = src
            .get_associated_props(arena)
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for prop in src_props {
            if prop.is_invalidated(arena) {
                continue;
            }

            dest.get_associated_props_mut(arena)
                .entry(prop)
                .or_insert(PropAssociation::SUPERTYPE);
            // Even an extern-only property must reach dest and enqueue it in the original
            // order. That order determines which representative names a source property.
            if !prop.is_extern_only(arena) {
                prop.get_clusters_mut(arena).union(src, dest);
            }
        }

        // Were any properties added to dest?
        start_dest_prop_count < dest.get_associated_props(arena).len()
    }
}

/// After invalidation the ordinary renamer only needs the clusters of source properties.
/// Extern-only properties still schedule the same graph edges. Store their membership in
/// sorted `(word_index, bits)` pairs so a single edge can merge them linearly without per-node
/// hash maps.
#[derive(Default)]
struct PropagatedProperties {
    source: IndexSet<PropertyClusteringId>,
    externs: Vec<(u32, u64)>,
}

pub(super) struct RenamePropagation {
    nodes: Vec<PropagatedProperties>,
    /// Rust-only (performance divergence): when present, marks the backward closure of nodes
    /// reachable from any non-invalidated source property. Nodes outside this set can never carry
    /// a source property or enqueue one that can.
    active: Option<Vec<bool>>,
    merge_scratch: Vec<(u32, u64)>,
}

impl RenamePropagation {
    pub(super) fn new(arena: &mut DisambiguateArena) -> Self {
        let nodes = (0..arena.nodes.len())
            .map(|i| {
                let id = ColorGraphNodeId(i as u32);
                let mut properties = PropagatedProperties::default();
                let associations = std::mem::take(id.get_associated_props_mut(arena));
                for (prop, _) in associations {
                    if prop.is_invalidated(arena) {
                        continue;
                    }
                    if prop.is_extern_only(arena) {
                        let word = prop.0 / 64;
                        let bit = 1u64 << (prop.0 % 64);
                        match properties.externs.binary_search_by_key(&word, |&(w, _)| w) {
                            Ok(pos) => properties.externs[pos].1 |= bit,
                            Err(pos) => properties.externs.insert(pos, (word, bit)),
                        }
                    } else {
                        properties.source.insert(prop);
                    }
                }
                properties
            })
            .collect();
        Self {
            nodes,
            active: None,
            merge_scratch: Vec::new(),
        }
    }

    /// Rust-only (performance divergence): builds `RenamePropagation` and prunes the fixed-point
    /// entry set to `Ancestors(Reachable(nodes_with_source_props))` in original graph node order.
    pub(super) fn new_for_graph(
        arena: &mut DisambiguateArena,
        graph: &ColorGraph,
    ) -> (Self, Vec<ColorGraphNodeId>) {
        let mut this = Self::new(arena);
        let n = this.nodes.len();
        let mut reach_src = vec![false; n];
        let mut queue = VecDeque::new();
        for (i, props) in this.nodes.iter().enumerate() {
            if !props.source.is_empty() {
                let id = ColorGraphNodeId(i as u32);
                if let Some(di_node) = graph.get_node(&id) {
                    reach_src[i] = true;
                    queue.push_back((id, di_node));
                }
            }
        }
        if queue.is_empty() {
            this.active = Some(vec![false; n]);
            return (this, Vec::new());
        }
        while let Some((id, di_node)) = queue.pop_front() {
            if id.get_color(arena).ptr_eq(&standard_colors::TOP_OBJECT) {
                continue;
            }
            for edge in di_node.get_out_edges(graph) {
                let dest_node = edge.get_destination(graph);
                let dest_id = *dest_node.get_value(graph);
                if !reach_src[dest_id.0 as usize] {
                    reach_src[dest_id.0 as usize] = true;
                    queue.push_back((dest_id, dest_node));
                }
            }
        }

        let mut active = reach_src;
        for (i, &is_active) in active.iter().enumerate() {
            if is_active {
                let id = ColorGraphNodeId(i as u32);
                if let Some(di_node) = graph.get_node(&id) {
                    queue.push_back((id, di_node));
                }
            }
        }
        while let Some((_, di_node)) = queue.pop_front() {
            for edge in di_node.get_in_edges(graph) {
                let src_node = edge.get_source(graph);
                let src_id = *src_node.get_value(graph);
                if !active[src_id.0 as usize]
                    && !src_id.get_color(arena).ptr_eq(&standard_colors::TOP_OBJECT)
                {
                    active[src_id.0 as usize] = true;
                    queue.push_back((src_id, src_node));
                }
            }
        }

        let entries = graph
            .get_nodes()
            .into_iter()
            .map(|di_node| *di_node.get_value(graph))
            .filter(|id| active[id.0 as usize])
            .collect();
        this.active = Some(active);
        (this, entries)
    }

    pub(super) fn traverse_edge(
        &mut self,
        arena: &mut DisambiguateArena,
        src: ColorGraphNodeId,
        dest: ColorGraphNodeId,
    ) -> bool {
        if src.get_color(arena).ptr_eq(&standard_colors::TOP_OBJECT) {
            return false;
        }
        if src == dest {
            // An associated source property already contains this node in its cluster.
            return false;
        }
        if self
            .active
            .as_ref()
            .is_some_and(|active| !active[dest.0 as usize])
        {
            return false;
        }
        let [from, to] = self
            .nodes
            .get_disjoint_mut([src.0 as usize, dest.0 as usize])
            .unwrap();
        let mut changed = false;
        for &prop in &from.source {
            changed |= to.source.insert(prop);
            prop.get_clusters_mut(arena).union(src, dest);
        }
        if !from.externs.is_empty() {
            if to.externs.is_empty() {
                to.externs.extend_from_slice(&from.externs);
                changed = true;
            } else {
                // Fast scan in place: update overlapping words and detect whether any new word
                // must be inserted.
                let mut i = 0;
                let mut j = 0;
                let mut needs_insert = false;
                while i < from.externs.len() && j < to.externs.len() {
                    let (fw, fb) = from.externs[i];
                    let (tw, tb) = to.externs[j];
                    if fw == tw {
                        if fb & !tb != 0 {
                            to.externs[j].1 = tb | fb;
                            changed = true;
                        }
                        i += 1;
                        j += 1;
                    } else if fw < tw {
                        needs_insert = true;
                        break;
                    } else {
                        j += 1;
                    }
                }
                if i < from.externs.len() {
                    needs_insert = true;
                }
                if needs_insert {
                    changed = true;
                    self.merge_scratch.clear();
                    self.merge_scratch
                        .reserve(from.externs.len() + to.externs.len());
                    let mut fi = 0;
                    let mut ti = 0;
                    while fi < from.externs.len() && ti < to.externs.len() {
                        let (fw, fb) = from.externs[fi];
                        let (tw, tb) = to.externs[ti];
                        if fw == tw {
                            self.merge_scratch.push((fw, fb | tb));
                            fi += 1;
                            ti += 1;
                        } else if fw < tw {
                            self.merge_scratch.push((fw, fb));
                            fi += 1;
                        } else {
                            self.merge_scratch.push((tw, tb));
                            ti += 1;
                        }
                    }
                    self.merge_scratch.extend_from_slice(&from.externs[fi..]);
                    self.merge_scratch.extend_from_slice(&to.externs[ti..]);
                    std::mem::swap(&mut to.externs, &mut self.merge_scratch);
                }
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disambiguate::{
        color_graph_node::ColorGraphNode, property_clustering::PropertyClustering,
    };

    #[test]
    fn compact_edges_preserve_queue_updates_and_cluster_representatives() {
        fn fixture() -> (
            DisambiguateArena,
            Vec<ColorGraphNodeId>,
            Vec<PropertyClusteringId>,
        ) {
            let mut arena = DisambiguateArena::new();
            let nodes: Vec<_> = (0..6)
                .map(|i| ColorGraphNode::create_for_testing(&mut arena, -1 - i))
                .collect();
            let mut source_properties = Vec::new();
            for i in 0..140 {
                let prop = PropertyClustering::new(&mut arena, format!("p{i}").into());
                let extern_only = i % 31 != 0;
                prop.set_extern_only(&mut arena, extern_only);
                if !extern_only {
                    source_properties.push(prop);
                }
                for &node in &[nodes[i % 3], nodes[(i / 4) % 5]] {
                    node.get_associated_props_mut(&mut arena)
                        .insert(prop, PropAssociation::AST);
                    if !extern_only {
                        prop.get_clusters_mut(&mut arena).add(node);
                    }
                }
            }
            (arena, nodes, source_properties)
        }
        let (mut original, nodes, properties) = fixture();
        let (mut compact, _, _) = fixture();
        let mut ordinary = ClusterPropagator::new();
        let mut packed = RenamePropagation::new(&mut compact);
        // Across a cycle a new extern bit can schedule a revisit even when no source
        // property was added. Compare the decision on each edge, including self-edges.
        let edges = [(0, 3), (1, 4), (4, 2), (3, 5), (5, 1), (3, 3), (2, 0)];
        loop {
            let mut changed = false;
            for &(from, to) in &edges {
                let expected = ordinary.traverse_edge(&mut original, nodes[from], None, nodes[to]);
                assert_eq!(
                    packed.traverse_edge(&mut compact, nodes[from], nodes[to]),
                    expected
                );
                changed |= expected;
            }
            if !changed {
                break;
            }
        }
        for prop in properties {
            let expected = prop.get_clusters_mut(&mut original);
            let actual = prop.get_clusters_mut(&mut compact);
            assert_eq!(
                actual.all_equivalence_classes(),
                expected.all_equivalence_classes()
            );
            for node in expected.elements() {
                assert_eq!(actual.find(&node), expected.find(&node));
            }
        }
    }
}

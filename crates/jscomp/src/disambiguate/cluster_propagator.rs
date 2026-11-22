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
    color_graph_builder::EdgeReason,
    color_graph_node::{
        ColorGraphNodeId, DisambiguateArena, PropAssociation, PropertyClusteringId,
    },
};
use crate::{colors::standard_colors, graph::union_find::UnionFind};
use closure_rhino::fast_hash::{IndexMap, IndexSet};

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
/// sparse 64-property words so a single edge can propagate many of them at once.
#[derive(Default)]
struct PropagatedProperties {
    source: IndexSet<PropertyClusteringId>,
    externs: IndexMap<u32, u64>,
}

pub(super) struct RenamePropagation {
    nodes: Vec<PropagatedProperties>,
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
                        *properties.externs.entry(prop.0 / 64).or_default() |= 1 << (prop.0 % 64);
                    } else {
                        properties.source.insert(prop);
                    }
                }
                properties
            })
            .collect();
        Self { nodes }
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
        let [from, to] = self
            .nodes
            .get_disjoint_mut([src.0 as usize, dest.0 as usize])
            .unwrap();
        let mut changed = false;
        for &prop in &from.source {
            changed |= to.source.insert(prop);
            prop.get_clusters_mut(arena).union(src, dest);
        }
        for (&word, &bits) in &from.externs {
            let old = to.externs.entry(word).or_default();
            changed |= bits & !*old != 0;
            *old |= bits;
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

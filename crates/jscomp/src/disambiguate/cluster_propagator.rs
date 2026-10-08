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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/disambiguate/ClusterPropagator.java.

//! Port of `disambiguate/ClusterPropagator.java`.

use super::{
    color_graph_builder::EdgeReason,
    color_graph_node::{ColorGraphNodeId, DisambiguateArena, PropAssociation},
};
use crate::{colors::standard_colors, graph::union_find::UnionFind};

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
            prop.get_clusters_mut(arena).union(src, dest);
        }

        // Were any properties added to dest?
        start_dest_prop_count < dest.get_associated_props(arena).len()
    }
}

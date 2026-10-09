/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/disambiguate/ColorGraphBuilder.java.

//! Port of `disambiguate/ColorGraphBuilder.java`.

use super::{
    color_graph_node::{ColorGraphNodeId, DisambiguateArena},
    color_graph_node_factory::ColorGraphNodeFactory,
};
use crate::{
    colors::{Color, color_registry::ColorRegistry, standard_colors},
    graph::{
        adjacency_graph::AdjacencyGraph, di_graph::DiGraphNode, graph::Graph,
        graph_node::GraphNode, graphviz_graph::GraphvizValue,
        linked_directed_graph::LinkedDirectedGraph,
        lowest_common_ancestor_finder::LowestCommonAncestorFinder,
    },
};
use closure_rhino::fx_hash::IndexSet;
use closure_rhino::{check_not_null, check_state};
use std::{fmt, sync::Arc};

/// The graph of colors `ColorGraphBuilder` builds. Java declares the edge values as `Object`
/// ("only meant for debugging"); every edge it creates holds an [`EdgeReason`].
pub type ColorGraph = LinkedDirectedGraph<ColorGraphNodeId, EdgeReason>;

/// The relationship that caused an edge to be created.
///
/// This information is only retained for diagnostics, not correctness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EdgeReason {
    ALGEBRAIC,
    CAN_HOLD,
}

impl EdgeReason {
    // port: ColorGraphBuilder.EdgeReason#name
    pub fn name(self) -> &'static str {
        match self {
            EdgeReason::ALGEBRAIC => "ALGEBRAIC",
            EdgeReason::CAN_HOLD => "CAN_HOLD",
        }
    }
}

impl fmt::Display for EdgeReason {
    // port: ColorGraphBuilder.EdgeReason#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl GraphvizValue for EdgeReason {
    // port: String#valueOf(Object)
    fn to_graphviz_string(&self) -> String {
        self.name().to_owned()
    }
}

/// Builds a graph of the {@link Color}s on the AST from a specified set of seed colors.
///
/// Java keeps the node factory as a field; here the factory and the arena holding the
/// `ColorGraphNode`s are passed to each method (they are shared with other parts of the pass).
pub struct ColorGraphBuilder {
    registry: Arc<ColorRegistry>,
    lca_finder: LowestCommonAncestorFinder<ColorGraphNodeId, EdgeReason>,
    top_node: DiGraphNode,

    /// The graph of colors as defined by `holdsInstanceOf`.
    ///
    /// We use `holdsInstanceOf` rather than `isSupertypeOf` because we only actually care about
    /// edges that were "used" in the program. If instances never flow over an edge at runtime,
    /// then properties also don't need to be tracked across that edge either. Of course, since we
    /// don't track the types of all assignments in a program, many of the edges are
    /// `isSupertypeOf` edges that we include to be conservative.
    ///
    /// This graph, when fully constructed, is still only an approximation. This is the due to
    /// both memory and time constraints. The following quirks are expected:
    ///
    /// - Some colors, such as primitives, will not have nodes.
    /// - Transitive edges (shortcut edges for which there exist alternate paths) are kept minimal.
    ///
    /// In practice, this could be declared as taking an {@link EdgeReason} instead of an Object,
    /// but Object is used to indicate that EdgeReasons are only meant for debugging and not any
    /// actual logic in (dis)ambiguation.
    color_holds_instance_graph: Option<ColorGraph>,
}

impl ColorGraphBuilder {
    // port: ColorGraphBuilder#ColorGraphBuilder
    pub fn new(
        node_factory: &mut ColorGraphNodeFactory,
        arena: &mut DisambiguateArena,
        lca_finder_factory: impl FnOnce(
            &ColorGraph,
        )
            -> LowestCommonAncestorFinder<ColorGraphNodeId, EdgeReason>,
        registry: Arc<ColorRegistry>,
    ) -> Self {
        let mut color_holds_instance_graph =
            ColorGraph::new_with_value_to_string(false, false, ColorGraphNodeId::to_string);
        let lca_finder = lca_finder_factory(&color_holds_instance_graph);

        let top_node = color_holds_instance_graph
            .create_node(node_factory.create_node(arena, Some(&standard_colors::UNKNOWN)));
        Self {
            registry,
            lca_finder,
            top_node,
            color_holds_instance_graph: Some(color_holds_instance_graph),
        }
    }

    fn graph(&self) -> &ColorGraph {
        self.color_holds_instance_graph.as_ref().unwrap()
    }

    fn graph_mut(&mut self) -> &mut ColorGraph {
        self.color_holds_instance_graph.as_mut().unwrap()
    }

    // port: ColorGraphBuilder#add
    pub fn add(
        &mut self,
        node_factory: &mut ColorGraphNodeFactory,
        arena: &mut DisambiguateArena,
        flat: ColorGraphNodeId,
    ) {
        self.add_internal(node_factory, arena, flat);
    }

    // port: ColorGraphBuilder#addAll
    pub fn add_all(
        &mut self,
        node_factory: &mut ColorGraphNodeFactory,
        arena: &mut DisambiguateArena,
        flats: impl IntoIterator<Item = ColorGraphNodeId>,
    ) {
        for flat in flats {
            self.add(node_factory, arena, flat);
        }
    }

    // port: ColorGraphBuilder#build
    pub fn build(
        &mut self,
        node_factory: &mut ColorGraphNodeFactory,
        arena: &mut DisambiguateArena,
    ) -> ColorGraph {
        for node in self.graph().get_nodes() {
            self.connect_union_with_ancestors(node_factory, arena, node);
        }

        self.color_holds_instance_graph.take().unwrap()
    }

    /// During initial lattice construction unions were only given outbound edges. Here we add any
    /// necessary inbound ones.
    ///
    /// We defer this operation because adding union-to-union and common-supertype-to-union
    /// edges, is a hard problem. Solving it after all other colors are in place makes it easier.
    // port: ColorGraphBuilder#connectUnionWithAncestors
    fn connect_union_with_ancestors(
        &mut self,
        node_factory: &mut ColorGraphNodeFactory,
        arena: &mut DisambiguateArena,
        union_node: DiGraphNode,
    ) {
        let flat_union = *union_node.get_value(self.graph());
        if !flat_union.get_color(arena).is_union() {
            return;
        }

        /*
         * Connect the LCAs to the union.
         *
         * <p>The union itself will be found in most cases, but since we don't add self-edges, that
         * won't matter.
         *
         * <p>Some of these edges may pollute the "lattice-ness" of the graph, but all the
         * invariants we actually care about will be maintained. If disambiguation is too slow and
         * stricter invariants would help, we could be more careful.
         */
        check_state!(!union_node.get_out_edges(self.graph()).is_empty());
        let union_elements = flat_union.get_color(arena).get_union_elements().clone();
        let graph_nodes = union_elements
            .iter()
            .map(|e| node_factory.create_node(arena, Some(e)))
            .collect::<IndexSet<_>>();
        let graph = self.color_holds_instance_graph.as_ref().unwrap();
        for lca in self.lca_finder.find_all(graph, &graph_nodes) {
            let lca_node = check_not_null!(self.graph().get_node(&lca));
            self.connect_source_to_dest(lca_node, EdgeReason::ALGEBRAIC, union_node);
        }
    }

    /// Insert {@code color} and all necessary related colors into the datastructures of this
    /// pass.
    // port: ColorGraphBuilder#addInternal(Color)
    fn add_internal_color(
        &mut self,
        node_factory: &mut ColorGraphNodeFactory,
        arena: &mut DisambiguateArena,
        color: &Color,
    ) -> DiGraphNode {
        let node = node_factory.create_node(arena, Some(color));
        self.add_internal(node_factory, arena, node)
    }

    /// Insert {@code node} and all necessary related colors into the datastructures of this pass.
    // port: ColorGraphBuilder#addInternal(ColorGraphNode)
    fn add_internal(
        &mut self,
        node_factory: &mut ColorGraphNodeFactory,
        arena: &mut DisambiguateArena,
        node: ColorGraphNodeId,
    ) -> DiGraphNode {
        if let Some(flat_node) = self.graph().get_node(&node) {
            return flat_node;
        }
        let flat_node = self.graph_mut().create_node(node);

        if node.get_color(arena).is_union() {
            let union_elements = node.get_color(arena).get_union_elements().clone();
            for alt in &union_elements {
                let alt_node = self.add_internal_color(node_factory, arena, alt);
                self.connect_source_to_dest(flat_node, EdgeReason::ALGEBRAIC, alt_node);
            }
            return flat_node;
        }

        let color = node.get_color(arena).clone();
        let registry = Arc::clone(&self.registry);
        let supertypes = registry.get_disambiguation_supertypes(&color);
        if supertypes.is_empty() {
            self.connect_source_to_dest(self.top_node, EdgeReason::ALGEBRAIC, flat_node);
        } else {
            for supertype in supertypes {
                let supertype_node = self.add_internal_color(node_factory, arena, supertype);
                self.connect_source_to_dest(supertype_node, EdgeReason::CAN_HOLD, flat_node);
            }
        }

        /*
         * Add all instance and prototype colors when visiting a constructor. We won't necessarily
         * see all possible instance colors that exist at runtime during an AST traversal.
         *
         * <p>For example, a subclass constructor may never be explicitly initialized but instead
         * passed to some function expecting `function(new:Parent)`. See {@link
         * AmbiguatePropertiesTest#testImplementsAndExtends_respectsUndeclaredProperties()}
         */
        for prototype in color.get_prototypes() {
            self.add_internal_color(node_factory, arena, prototype);
        }
        for instance_color in color.get_instance_colors() {
            self.add_internal_color(node_factory, arena, instance_color);
        }
        flat_node
    }

    // port: ColorGraphBuilder#connectSourceToDest
    fn connect_source_to_dest(
        &mut self,
        source: DiGraphNode,
        reason: EdgeReason,
        dest: DiGraphNode,
    ) {
        if source == dest
            || self
                .graph()
                .is_connected_in_direction_nodes(source, |_t| true, dest)
        {
            return;
        }

        self.graph_mut().connect_nodes(source, reason, dest);
    }
}

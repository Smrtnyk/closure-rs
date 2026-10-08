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

//! Java node/edge interfaces are Copy arena handles; concrete Linked* classes
//! name the stored data. Graph wrappers delegate arena accessors to preserve
//! identity. Node data carry N, edge data carry E and endpoint handles (so the
//! unused Java type parameters disappear). Annotated subclasses share that data
//! representation; the graph flags select their get/setAnnotation implementation.
//! Display constructors cover ordinary Java values; new_with_value_to_string
//! supplies Java toString for arena handles without constraining N beyond
//! Clone + Eq + Hash. Graphviz and algorithms use the same registered formatter.
//! new_with_value_to_strings also supplies rendering for arbitrary edge values;
//! standard String.valueOf edge rendering (including Option nulls) is provided by
//! GraphvizValue. All graph traits and algorithms retain only the Java value bounds.
//! Type-erasure hooks, arena accessors, and Rust Default delegates are adapters,
//! rather than extra Java methods.
//!
//! SubGraph and algorithm methods receive the graph as an arena argument.
pub mod adjacency_graph;
pub mod annotatable;
pub mod annotation;
pub mod di_graph;
#[allow(clippy::module_inception)] // Java Graph.java in the graph package.
pub mod graph;
pub mod graph_node;
pub mod graphviz_graph;
pub mod lattice_element;
pub mod linked_directed_graph;
pub mod linked_undirected_graph;
pub mod sub_graph;
pub mod undi_graph;

pub use adjacency_graph::AdjacencyGraph;
pub use annotatable::Annotatable;
pub use annotation::Annotation;
pub use di_graph::{DiGraph, DiGraphEdge, DiGraphNode};
pub use graph::{Graph, GraphEdge};
pub use graph_node::GraphNode;
pub use linked_directed_graph::LinkedDirectedGraph;
pub use linked_undirected_graph::LinkedUndirectedGraph;
pub use undi_graph::{UndiGraph, UndiGraphEdge, UndiGraphNode};
pub mod check_paths_between_nodes;
pub mod dominator_tree;
pub mod fixed_point_graph_traversal;
pub mod graph_coloring;
pub mod graph_reachability;
pub mod lowest_common_ancestor_finder;
pub mod standard_union_find;
pub mod union_find;

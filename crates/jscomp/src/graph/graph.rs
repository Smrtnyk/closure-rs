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
//   src/com/google/javascript/jscomp/graph/DiGraph.java,
//   src/com/google/javascript/jscomp/graph/Graph.java,
//   src/com/google/javascript/jscomp/graph/LinkedDirectedGraph.java.

use super::{
    adjacency_graph::AdjacencyGraph, annotatable::Annotatable, annotation::Annotation,
    graph_node::GraphNode, sub_graph::SubGraph,
};
use closure_rhino::check_not_null;
use std::hash::Hash;
pub trait GraphEdge<N, E, G: AdjacencyGraph<N, E>>:
    Annotatable<N, E, G> + Copy + Eq + Hash + 'static
{
    // port: Graph.GraphEdge#getValue
    fn get_value(self, graph: &G) -> &E;
    // port: Graph.GraphEdge#getNodeA
    fn get_node_a(self, graph: &G) -> G::Node;
    // port: Graph.GraphEdge#getNodeB
    fn get_node_b(self, graph: &G) -> G::Node;
}
pub struct AnnotationState<H> {
    first: H,
    second: Option<Box<dyn Annotation>>,
}
impl<H> AnnotationState<H> {
    // port: Graph.AnnotationState#AnnotationState
    fn new(annotatable: H, annotation: Option<Box<dyn Annotation>>) -> Self {
        Self {
            first: annotatable,
            second: annotation,
        }
    }
}
#[derive(Default)]
pub struct GraphAnnotationState<H>(pub Vec<AnnotationState<H>>);
impl<H> GraphAnnotationState<H> {
    // port: Graph.GraphAnnotationState#GraphAnnotationState
    fn new(size: usize) -> Self {
        Self(Vec::with_capacity(size))
    }
}
pub struct GraphAnnotationStacks<N, E> {
    pub node_annotation_stack: Option<Vec<GraphAnnotationState<N>>>,
    pub edge_annotation_stack: Option<Vec<GraphAnnotationState<E>>>,
}
impl<N, E> Default for GraphAnnotationStacks<N, E> {
    // port: Graph#Graph
    fn default() -> Self {
        Self {
            node_annotation_stack: None,
            edge_annotation_stack: None,
        }
    }
}
pub trait Graph<N, E>: AdjacencyGraph<N, E> {
    type Edge: GraphEdge<N, E, Self>;
    // port: Graph#connect
    fn connect(&mut self, n1: N, edge: E, n2: N);
    // port: Graph#disconnect
    fn disconnect(&mut self, n1: &N, n2: &N);
    // port: Graph#connectIfNotFound
    fn connect_if_not_found(&mut self, n1: N, edge: E, n2: N) {
        if !self.is_connected_with_edge(&n1, &edge, &n2) {
            self.connect(n1, edge, n2);
        }
    }
    // port: DiGraph#createNode
    // port: Graph#createNode
    fn create_node(&mut self, value: N) -> Self::Node;
    // port: DiGraph#getEdges()
    // port: Graph#getEdges()
    fn get_edges(&self) -> Vec<Self::Edge>;
    // port: DiGraph#getEdges(N,N)
    // port: Graph#getEdges(N,N)
    fn get_edges_between(&self, n1: &N, n2: &N) -> Vec<Self::Edge>;
    // port: Graph#getNodeDegree
    fn get_node_degree(&self, value: &N) -> i32;
    // port: Graph#getNeighborNodes
    fn get_neighbor_nodes(&self, value: &N) -> Vec<Self::Node>;
    // port: Graph#getFirstEdge
    fn get_first_edge(&self, n1: &N, n2: &N) -> Option<Self::Edge>;
    // port: Graph#hasNode
    fn has_node(&self, n: &N) -> bool {
        self.get_node(n).is_some()
    }
    // port: Graph#isConnected(N,N)
    fn is_connected(&self, n1: &N, n2: &N) -> bool;
    // port: Graph#isConnected(N,E,N)
    fn is_connected_with_edge(&self, n1: &N, e: &E, n2: &N) -> bool;
    // port: Graph#getNodeOrFail
    fn get_node_or_fail(&self, val: &N) -> Self::Node {
        self.get_node(val)
            .unwrap_or_else(|| panic!("{} does not exist in graph", self.value_to_string(val)))
    }
    // port: LinkedDiGraphNode#toString
    fn node_to_string(&self, node: Self::Node) -> String {
        self.value_to_string(self.node_value(node))
    }
    /// Java Object.toString for precondition messages, supplied for arena values.
    fn value_to_string(&self, value: &N) -> String;
    // port: Graph#clearEdgeAnnotations
    fn clear_edge_annotations(&mut self) {
        for e in self.get_edges() {
            e.set_annotation(self, None);
        }
    }
    fn annotation_stacks(&mut self) -> &mut GraphAnnotationStacks<Self::Node, Self::Edge>;
    fn edge_value(&self, edge: Self::Edge) -> &E;
    fn edge_node_a(&self, edge: Self::Edge) -> Self::Node;
    fn edge_node_b(&self, edge: Self::Edge) -> Self::Node;
    fn edge_annotation(&self, edge: Self::Edge) -> &Option<Box<dyn Annotation>>;
    fn edge_annotation_mut(&mut self, edge: Self::Edge) -> &mut Option<Box<dyn Annotation>>;
    // port: Graph#pushNodeAnnotations
    fn push_node_annotations(&mut self) {
        if self.annotation_stacks().node_annotation_stack.is_none() {
            self.annotation_stacks().node_annotation_stack = Some(Vec::new());
        }
        let nodes = self.get_nodes();
        push_annotations(self, nodes, |graph| {
            graph
                .annotation_stacks()
                .node_annotation_stack
                .as_mut()
                .unwrap()
        });
    }
    // port: Graph#popNodeAnnotations
    fn pop_node_annotations(&mut self) {
        let stack = check_not_null!(
            self.annotation_stacks().node_annotation_stack.as_mut(),
            "Popping node annotations without pushing."
        );
        let state = check_not_null!(stack.pop());
        pop_annotations(self, state);
    }
    // port: Graph#pushEdgeAnnotations
    fn push_edge_annotations(&mut self) {
        if self.annotation_stacks().edge_annotation_stack.is_none() {
            self.annotation_stacks().edge_annotation_stack = Some(Vec::new());
        }
        let edges = self.get_edges();
        push_annotations(self, edges, |graph| {
            graph
                .annotation_stacks()
                .edge_annotation_stack
                .as_mut()
                .unwrap()
        });
    }
    // port: Graph#popEdgeAnnotations
    fn pop_edge_annotations(&mut self) {
        let stack = check_not_null!(
            self.annotation_stacks().edge_annotation_stack.as_mut(),
            "Popping edge annotations without pushing."
        );
        let state = check_not_null!(stack.pop());
        pop_annotations(self, state);
    }
}
// port: Graph#pushAnnotations
fn push_annotations<N, E, G, H: Annotatable<N, E, G>>(
    graph: &mut G,
    have_annotations: Vec<H>,
    stack: impl Fn(&mut G) -> &mut Vec<GraphAnnotationState<H>>,
) {
    stack(graph).push(GraphAnnotationState::new(have_annotations.len()));
    for h in have_annotations {
        let annotation = h.take_annotation(graph);
        stack(graph)
            .last_mut()
            .unwrap()
            .0
            .push(AnnotationState::new(h, annotation));
    }
}
// port: Graph#popAnnotations
fn pop_annotations<N, E, G, H: Annotatable<N, E, G>>(
    graph: &mut G,
    state: GraphAnnotationState<H>,
) {
    for state in state.0 {
        state.first.set_annotation(graph, state.second);
    }
}
pub struct SimpleSubGraph<H> {
    nodes: Vec<H>,
}
impl<H> SimpleSubGraph<H> {
    // port: Graph.SimpleSubGraph#SimpleSubGraph
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }
}
impl<H> Default for SimpleSubGraph<H> {
    fn default() -> Self {
        Self::new()
    }
}
impl<N, E, G: Graph<N, E>> SubGraph<N, E, G> for SimpleSubGraph<G::Node> {
    // port: Graph.SimpleSubGraph#isIndependentOf
    fn is_independent_of(&self, graph: &G, value: &N) -> bool {
        let node = graph.get_node(value);
        for n in &self.nodes {
            if node.is_some_and(|node| graph.get_neighbor_nodes(n.get_value(graph)).contains(&node))
            {
                return false;
            }
        }
        true
    }
    // port: Graph.SimpleSubGraph#addNode
    fn add_node(&mut self, graph: &G, value: &N) {
        self.nodes.push(graph.get_node_or_fail(value));
    }
}

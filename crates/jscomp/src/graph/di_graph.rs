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
//   src/com/google/javascript/jscomp/graph/Annotatable.java,
//   src/com/google/javascript/jscomp/graph/DiGraph.java,
//   src/com/google/javascript/jscomp/graph/Graph.java,
//   src/com/google/javascript/jscomp/graph/LinkedDirectedGraph.java.

use super::{
    adjacency_graph::AdjacencyGraph,
    annotatable::Annotatable,
    annotation::Annotation,
    graph::{Graph, GraphEdge},
    graph_node::GraphNode,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DiGraphNode(pub usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DiGraphEdge(pub usize);
impl<N, E, G: AdjacencyGraph<N, E, Node = DiGraphNode>> Annotatable<N, E, G> for DiGraphNode {
    // port: LinkedDiGraphNode#getAnnotation
    fn get_annotation(self, graph: &G) -> Option<&dyn Annotation> {
        graph.node_annotation(self).as_deref()
    }
    // port: Annotatable#setAnnotation
    fn set_annotation(self, graph: &mut G, data: Option<Box<dyn Annotation>>) {
        *graph.node_annotation_mut(self) = data;
    }
    fn take_annotation(self, graph: &mut G) -> Option<Box<dyn Annotation>> {
        graph.node_annotation_mut(self).take()
    }
    // port: Annotatable#getAnnotation
    fn get_annotation_as_mut<A: Annotation>(self, graph: &mut G) -> Option<&mut A> {
        graph.node_annotation_mut(self).as_deref_mut().map(|a| {
            a.as_any_mut()
                .downcast_mut::<A>()
                .expect("ClassCastException")
        })
    }
}
impl<N, E, G: Graph<N, E, Edge = DiGraphEdge>> Annotatable<N, E, G> for DiGraphEdge {
    // port: LinkedDiGraphEdge#getAnnotation
    fn get_annotation(self, graph: &G) -> Option<&dyn Annotation> {
        graph.edge_annotation(self).as_deref()
    }
    // port: Annotatable#setAnnotation
    fn set_annotation(self, graph: &mut G, data: Option<Box<dyn Annotation>>) {
        *graph.edge_annotation_mut(self) = data;
    }
    fn take_annotation(self, graph: &mut G) -> Option<Box<dyn Annotation>> {
        graph.edge_annotation_mut(self).take()
    }
    // port: Annotatable#getAnnotation
    fn get_annotation_as_mut<A: Annotation>(self, graph: &mut G) -> Option<&mut A> {
        graph.edge_annotation_mut(self).as_deref_mut().map(|a| {
            a.as_any_mut()
                .downcast_mut::<A>()
                .expect("ClassCastException")
        })
    }
}
impl<N, E, G: AdjacencyGraph<N, E, Node = DiGraphNode>> GraphNode<N, E, G> for DiGraphNode {
    // port: LinkedDiGraphNode#getValue
    // port: DiGraph.DiGraphNode#getValue
    fn get_value(self, graph: &G) -> &N {
        graph.node_value(self)
    }
}
impl<N, E, G: Graph<N, E, Edge = DiGraphEdge>> GraphEdge<N, E, G> for DiGraphEdge {
    // port: LinkedDiGraphEdge#getValue
    // port: DiGraph.DiGraphEdge#getValue
    fn get_value(self, graph: &G) -> &E {
        graph.edge_value(self)
    }
    // port: Graph.GraphEdge#getNodeA
    fn get_node_a(self, graph: &G) -> G::Node {
        graph.edge_node_a(self)
    }
    // port: Graph.GraphEdge#getNodeB
    fn get_node_b(self, graph: &G) -> G::Node {
        graph.edge_node_b(self)
    }
}
pub trait DiGraph<N, E>: Graph<N, E, Node = DiGraphNode, Edge = DiGraphEdge> {
    // port: DiGraph#getEdgesInDirection
    fn get_edges_in_direction(&self, n1: &N, n2: &N) -> Vec<DiGraphEdge>;
    // port: DiGraph#getOutEdges
    fn get_out_edges(&self, value: &N) -> &[DiGraphEdge];
    // port: DiGraph#getInEdges
    fn get_in_edges(&self, value: &N) -> &[DiGraphEdge];
    // port: DiGraph#getDirectedPredNodes(N)
    fn get_directed_pred_nodes(&self, value: &N) -> Vec<DiGraphNode>;
    // port: DiGraph#getDirectedSuccNodes(N)
    fn get_directed_succ_nodes(&self, value: &N) -> Vec<DiGraphNode>;
    // port: LinkedDirectedGraph#getDirectedPredNodes(DiGraphNode)
    // port: DiGraph#getDirectedPredNodes(DiGraphNode)
    fn get_directed_pred_nodes_of(&self, node: DiGraphNode) -> Vec<DiGraphNode> {
        self.node_in_edges(node)
            .iter()
            .map(|e| self.edge_node_a(*e))
            .collect()
    }
    // port: LinkedDirectedGraph#getDirectedSuccNodes(DiGraphNode)
    // port: DiGraph#getDirectedSuccNodes(DiGraphNode)
    fn get_directed_succ_nodes_of(&self, node: DiGraphNode) -> Vec<DiGraphNode> {
        self.node_out_edges(node)
            .iter()
            .map(|e| self.edge_node_b(*e))
            .collect()
    }
    // port: DiGraph#disconnectInDirection
    fn disconnect_in_direction(&mut self, n1: &N, n2: &N);
    // port: DiGraph#isConnectedInDirection(N,N)
    fn is_connected_in_direction(&self, n1: &N, n2: &N) -> bool;
    // port: DiGraph#isConnectedInDirection(N,E,N)
    fn is_connected_in_direction_with_edge(&self, n1: &N, e: &E, n2: &N) -> bool;
    fn node_out_edges(&self, node: DiGraphNode) -> &[DiGraphEdge];
    fn node_in_edges(&self, node: DiGraphNode) -> &[DiGraphEdge];
    fn node_out_edges_mut(&mut self, node: DiGraphNode) -> &mut Vec<DiGraphEdge>;
    fn node_in_edges_mut(&mut self, node: DiGraphNode) -> &mut Vec<DiGraphEdge>;
    fn node_priority(&self, node: DiGraphNode) -> i32;
    fn node_priority_mut(&mut self, node: DiGraphNode) -> &mut i32;
}
impl DiGraphNode {
    // port: LinkedDiGraphNode#getOutEdges
    pub fn get_out_edges_mut<N, E, G: DiGraph<N, E>>(self, graph: &mut G) -> &mut Vec<DiGraphEdge> {
        graph.node_out_edges_mut(self)
    }
    // port: LinkedDiGraphNode#getInEdges
    pub fn get_in_edges_mut<N, E, G: DiGraph<N, E>>(self, graph: &mut G) -> &mut Vec<DiGraphEdge> {
        graph.node_in_edges_mut(self)
    }
    // port: LinkedDiGraphNode#getOutEdges
    pub fn get_out_edges<N, E, G: DiGraph<N, E>>(self, graph: &G) -> &[DiGraphEdge] {
        graph.node_out_edges(self)
    }
    // port: LinkedDiGraphNode#getInEdges
    pub fn get_in_edges<N, E, G: DiGraph<N, E>>(self, graph: &G) -> &[DiGraphEdge] {
        graph.node_in_edges(self)
    }
    // port: LinkedDiGraphNode#hasPriority
    pub fn has_priority<N, E, G: DiGraph<N, E>>(self, graph: &G) -> bool {
        graph.node_priority(self) >= 0
    }
    // port: LinkedDiGraphNode#getPriority
    pub fn get_priority<N, E, G: DiGraph<N, E>>(self, graph: &G) -> i32 {
        let p = graph.node_priority(self);
        closure_rhino::check_state!(p >= 0, "priority not set");
        p
    }
    // port: LinkedDiGraphNode#setPriority
    pub fn set_priority<N, E, G: DiGraph<N, E>>(self, graph: &mut G, priority: i32) {
        closure_rhino::check_argument!(priority >= 0, "priorities must be non-negative");
        *graph.node_priority_mut(self) = priority;
    }
}
impl DiGraphEdge {
    // port: LinkedDiGraphEdge#getSource
    pub fn get_source<N, E, G: DiGraph<N, E>>(self, graph: &G) -> DiGraphNode {
        graph.edge_node_a(self)
    }
    // port: LinkedDiGraphEdge#getDestination
    pub fn get_destination<N, E, G: DiGraph<N, E>>(self, graph: &G) -> DiGraphNode {
        graph.edge_node_b(self)
    }
}

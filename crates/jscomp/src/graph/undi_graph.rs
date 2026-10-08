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
//   src/com/google/javascript/jscomp/graph/Annotatable.java,
//   src/com/google/javascript/jscomp/graph/Graph.java,
//   src/com/google/javascript/jscomp/graph/LinkedUndirectedGraph.java,
//   src/com/google/javascript/jscomp/graph/UndiGraph.java.

use super::{
    adjacency_graph::AdjacencyGraph,
    annotatable::Annotatable,
    annotation::Annotation,
    graph::{Graph, GraphEdge},
    graph_node::GraphNode,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UndiGraphNode(pub usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UndiGraphEdge(pub usize);
impl<N, E, G: AdjacencyGraph<N, E, Node = UndiGraphNode>> Annotatable<N, E, G> for UndiGraphNode {
    // port: LinkedUndirectedGraphNode#getAnnotation
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
impl<N, E, G: Graph<N, E, Edge = UndiGraphEdge>> Annotatable<N, E, G> for UndiGraphEdge {
    // port: LinkedUndirectedGraphEdge#getAnnotation
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
impl<N, E, G: AdjacencyGraph<N, E, Node = UndiGraphNode>> GraphNode<N, E, G> for UndiGraphNode {
    // port: LinkedUndirectedGraphNode#getValue
    // port: UndiGraph.UndiGraphNode#getValue
    fn get_value(self, graph: &G) -> &N {
        graph.node_value(self)
    }
}
impl<N, E, G: Graph<N, E, Edge = UndiGraphEdge>> GraphEdge<N, E, G> for UndiGraphEdge {
    // port: LinkedUndirectedGraphEdge#getValue
    // port: UndiGraph.UndiGraphEdge#getValue
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
pub trait UndiGraph<N, E>: Graph<N, E, Node = UndiGraphNode, Edge = UndiGraphEdge> {
    // port: UndiGraph#getUndirectedGraphNodes
    fn get_undirected_graph_nodes(&self) -> Vec<UndiGraphNode>;
    // port: UndiGraph#createUndirectedGraphNode
    fn create_undirected_graph_node(&mut self, value: N) -> UndiGraphNode;
    // port: UndiGraph#getUndirectedGraphNode
    fn get_undirected_graph_node(&self, value: &N) -> Option<UndiGraphNode>;
    // port: UndiGraph#getUndirectedGraphEdges
    fn get_undirected_graph_edges(&self, n1: &N, n2: &N) -> Option<Vec<UndiGraphEdge>>;
    fn node_neighbor_edges(&self, node: UndiGraphNode) -> &[UndiGraphEdge];
    fn node_neighbor_edges_mut(&mut self, node: UndiGraphNode) -> &mut Vec<UndiGraphEdge>;
}
impl UndiGraphNode {
    // port: LinkedUndirectedGraphNode#getNeighborEdges
    pub fn get_neighbor_edges_mut<N, E, G: UndiGraph<N, E>>(
        self,
        graph: &mut G,
    ) -> &mut Vec<UndiGraphEdge> {
        graph.node_neighbor_edges_mut(self)
    }
    // port: LinkedUndirectedGraphNode#getNeighborEdges
    pub fn get_neighbor_edges<N, E, G: UndiGraph<N, E>>(self, graph: &G) -> &[UndiGraphEdge] {
        graph.node_neighbor_edges(self)
    }
    // port: LinkedUndirectedGraphNode#getNeighborEdgesIterator
    pub fn get_neighbor_edges_iterator<N, E, G: UndiGraph<N, E>>(
        self,
        graph: &G,
    ) -> impl Iterator<Item = UndiGraphEdge> {
        graph.node_neighbor_edges(self).iter().copied()
    }
}

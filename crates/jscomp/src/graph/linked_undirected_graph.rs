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
//   src/com/google/javascript/jscomp/graph/Graph.java,
//   src/com/google/javascript/jscomp/graph/LinkedUndirectedGraph.java.

use super::{
    adjacency_graph::AdjacencyGraph,
    annotation::Annotation,
    graph::{Graph, GraphAnnotationStacks, SimpleSubGraph},
    graphviz_graph::{GraphvizEdge, GraphvizGraph, GraphvizNode, GraphvizValue},
    sub_graph::SubGraph,
    undi_graph::{UndiGraph, UndiGraphEdge, UndiGraphNode},
};
use closure_rhino::fast_hash::IndexMap;
use std::{fmt::Display, hash::Hash};
pub struct LinkedUndirectedGraph<N, E> {
    pub nodes: IndexMap<N, UndiGraphNode>,
    node_arena: Vec<LinkedUndirectedGraphNode<N>>,
    edge_arena: Vec<LinkedUndirectedGraphEdge<E>>,
    value_to_string: fn(&N) -> String,
    edge_value_to_string: fn(&E) -> String,
    use_node_annotations: bool,
    use_edge_annotations: bool,
    annotation_stacks: GraphAnnotationStacks<UndiGraphNode, UndiGraphEdge>,
}
/// Java annotated subclasses are represented by the optional annotation field;
/// the graph's flags choose the same get/set behaviour as the Java subclass.
pub struct LinkedUndirectedGraphNode<N> {
    pub value: N,
    annotation: Option<Box<dyn Annotation>>,
    neighbor_edges: Vec<UndiGraphEdge>,
}
impl<N> LinkedUndirectedGraphNode<N> {
    // port: LinkedUndirectedGraphNode#LinkedUndirectedGraphNode
    // port: AnnotatedLinkedUndirectedGraphNode#AnnotatedLinkedUndirectedGraphNode
    fn new(value: N) -> Self {
        Self {
            value,
            annotation: None,
            neighbor_edges: Vec::new(),
        }
    }
}
pub struct LinkedUndirectedGraphEdge<E> {
    value: E,
    node_a: UndiGraphNode,
    node_b: UndiGraphNode,
    annotation: Option<Box<dyn Annotation>>,
}
impl<E> LinkedUndirectedGraphEdge<E> {
    // port: LinkedUndirectedGraphEdge#LinkedUndirectedGraphEdge
    // port: AnnotatedLinkedUndirectedGraphEdge#AnnotatedLinkedUndirectedGraphEdge
    fn new(node_a: UndiGraphNode, value: E, node_b: UndiGraphNode) -> Self {
        Self {
            node_a,
            value,
            node_b,
            annotation: None,
        }
    }
}
impl<N: Clone + Eq + Hash, E: Clone + PartialEq> LinkedUndirectedGraph<N, E> {
    // port: LinkedUndirectedGraph#create
    pub fn create() -> Self
    where
        N: Display,
        E: GraphvizValue,
    {
        Self::new(true, true)
    }
    // port: LinkedUndirectedGraph#LinkedUndirectedGraph
    pub fn new(use_node_annotations: bool, use_edge_annotations: bool) -> Self
    where
        N: Display,
        E: GraphvizValue,
    {
        Self::new_with_value_to_string(use_node_annotations, use_edge_annotations, |n| {
            n.to_string()
        })
    }
    /// Arena values such as NodeId supply Java toString separately from equality.
    // port: LinkedUndirectedGraph#LinkedUndirectedGraph
    pub fn new_with_value_to_string(
        use_node_annotations: bool,
        use_edge_annotations: bool,
        value_to_string: fn(&N) -> String,
    ) -> Self
    where
        E: GraphvizValue,
    {
        Self::new_with_value_to_strings(
            use_node_annotations,
            use_edge_annotations,
            value_to_string,
            GraphvizValue::to_graphviz_string,
        )
    }
    // port: LinkedUndirectedGraph#LinkedUndirectedGraph
    pub fn new_with_value_to_strings(
        use_node_annotations: bool,
        use_edge_annotations: bool,
        value_to_string: fn(&N) -> String,
        edge_value_to_string: fn(&E) -> String,
    ) -> Self {
        Self {
            nodes: IndexMap::<_, _>::default(),
            value_to_string,
            edge_value_to_string,
            node_arena: Vec::new(),
            edge_arena: Vec::new(),
            use_node_annotations,
            use_edge_annotations,
            annotation_stacks: GraphAnnotationStacks::default(),
        }
    }
    // port: LinkedUndirectedGraph#isConnected(N,Predicate,N)
    pub fn is_connected_matching(
        &self,
        n1: &N,
        edge_predicate: impl Fn(&E) -> bool,
        n2: &N,
    ) -> bool {
        let Some(d1) = self.get_node(n1) else {
            return false;
        };
        let Some(d2) = self.get_node(n2) else {
            return false;
        };
        for edge in &self.node_arena[d1.0].neighbor_edges {
            let e = &self.edge_arena[edge.0];
            if ((e.node_a == d1 && e.node_b == d2) || (e.node_a == d2 && e.node_b == d1))
                && edge_predicate(&e.value)
            {
                return true;
            }
        }
        false
    }
    // port: LinkedUndirectedGraphNode#neighborList
    fn neighbor_list(&self, node: UndiGraphNode) -> Vec<UndiGraphNode> {
        self.node_arena[node.0]
            .neighbor_edges
            .iter()
            .map(|edge| {
                let e = &self.edge_arena[edge.0];
                if e.node_a == node { e.node_b } else { e.node_a }
            })
            .collect()
    }
}
impl<N: Clone + Eq + Hash, E: Clone + PartialEq> AdjacencyGraph<N, E>
    for LinkedUndirectedGraph<N, E>
{
    type Node = UndiGraphNode;
    // port: LinkedUndirectedGraph#getNodes
    fn get_nodes(&self) -> Vec<UndiGraphNode> {
        self.nodes.values().copied().collect()
    }
    // port: LinkedUndirectedGraph#getNodeCount
    fn get_node_count(&self) -> usize {
        self.nodes.len()
    }
    // port: LinkedUndirectedGraph#getNode
    fn get_node(&self, value: &N) -> Option<UndiGraphNode> {
        self.nodes.get(value).copied()
    }
    // port: LinkedUndirectedGraph#newSubGraph
    fn new_sub_graph(&self) -> Box<dyn SubGraph<N, E, Self>> {
        Box::new(SimpleSubGraph::<UndiGraphNode>::new())
    }
    // port: Graph#getWeight
    fn get_weight(&self, value: &N) -> i32 {
        self.get_node_degree(value)
    }
    // port: LinkedUndirectedGraphNode#getValue
    fn node_value(&self, node: UndiGraphNode) -> &N {
        &self.node_arena[node.0].value
    }
    // port: LinkedUndirectedGraphNode#getAnnotation
    // port: AnnotatedLinkedUndirectedGraphNode#getAnnotation
    fn node_annotation(&self, node: UndiGraphNode) -> &Option<Box<dyn Annotation>> {
        if !self.use_node_annotations {
            panic!("Graph initialized with node annotations turned off");
        }
        &self.node_arena[node.0].annotation
    }
    // port: LinkedUndirectedGraphNode#setAnnotation
    // port: AnnotatedLinkedUndirectedGraphNode#setAnnotation
    fn node_annotation_mut(&mut self, node: UndiGraphNode) -> &mut Option<Box<dyn Annotation>> {
        if !self.use_node_annotations {
            panic!("Graph initialized with node annotations turned off");
        }
        &mut self.node_arena[node.0].annotation
    }
}
impl<N: Clone + Eq + Hash, E: Clone + PartialEq> Graph<N, E> for LinkedUndirectedGraph<N, E> {
    type Edge = UndiGraphEdge;
    // port: LinkedUndirectedGraph#connect(N,E,N)
    fn connect(&mut self, n1: N, edge_value: E, n2: N) {
        let src = self.get_node_or_fail(&n1);
        let dest = self.get_node_or_fail(&n2);
        let edge = UndiGraphEdge(self.edge_arena.len());
        self.edge_arena
            .push(LinkedUndirectedGraphEdge::new(src, edge_value, dest));
        self.node_arena[src.0].neighbor_edges.push(edge);
        self.node_arena[dest.0].neighbor_edges.push(edge);
    }
    // port: LinkedUndirectedGraph#disconnect
    fn disconnect(&mut self, n1: &N, n2: &N) {
        let src = self.get_node_or_fail(n1);
        let dest = self.get_node_or_fail(n2);
        for edge in self.get_undirected_graph_edges(n1, n2).unwrap() {
            for node in [src, dest] {
                let list = &mut self.node_arena[node.0].neighbor_edges;
                if let Some(i) = list.iter().position(|e| *e == edge) {
                    list.remove(i);
                }
            }
        }
    }
    // port: LinkedUndirectedGraph#createNode
    fn create_node(&mut self, value: N) -> UndiGraphNode {
        self.create_undirected_graph_node(value)
    }
    // port: LinkedUndirectedGraph#getEdges()
    fn get_edges(&self) -> Vec<UndiGraphEdge> {
        let mut result = Vec::new();
        for node in self.nodes.values() {
            for edge in &self.node_arena[node.0].neighbor_edges {
                if self.edge_arena[edge.0].node_a == *node {
                    result.push(*edge);
                }
            }
        }
        result
    }
    // port: LinkedUndirectedGraph#getEdges(N,N)
    fn get_edges_between(&self, n1: &N, n2: &N) -> Vec<UndiGraphEdge> {
        self.get_undirected_graph_edges(n1, n2).expect("")
    }
    // port: LinkedUndirectedGraph#getFirstEdge
    fn get_first_edge(&self, n1: &N, n2: &N) -> Option<UndiGraphEdge> {
        let d1 = self.get_node_or_fail(n1);
        let d2 = self.get_node_or_fail(n2);
        for edge in &self.node_arena[d1.0].neighbor_edges {
            let e = &self.edge_arena[edge.0];
            if e.node_a == d2 || e.node_b == d2 {
                return Some(*edge);
            }
        }
        None
    }
    // port: LinkedUndirectedGraph#getNodeDegree
    fn get_node_degree(&self, value: &N) -> i32 {
        let node = self
            .get_node(value)
            .unwrap_or_else(|| panic!("{} not found in graph", self.value_to_string(value)));
        self.node_arena[node.0].neighbor_edges.len() as i32
    }
    // port: LinkedUndirectedGraph#getNeighborNodes
    fn get_neighbor_nodes(&self, value: &N) -> Vec<UndiGraphNode> {
        self.neighbor_list(self.get_node(value).expect(""))
    }
    // port: LinkedUndirectedGraph#isConnected(N,N)
    fn is_connected(&self, n1: &N, n2: &N) -> bool {
        self.is_connected_matching(n1, |_| true, n2)
    }
    // port: LinkedUndirectedGraph#isConnected(N,E,N)
    fn is_connected_with_edge(&self, n1: &N, edge: &E, n2: &N) -> bool {
        self.is_connected_matching(n1, |e| e == edge, n2)
    }
    // port: Object#toString (LinkedUndirectedGraphNode inherits it)
    fn node_to_string(&self, node: UndiGraphNode) -> String {
        let class = if self.use_node_annotations {
            "AnnotatedLinkedUndirectedGraphNode"
        } else {
            "LinkedUndirectedGraphNode"
        };
        // Deviation: inherited Object.toString uses the deterministic identity hash (arena index).
        format!(
            "com.google.javascript.jscomp.graph.LinkedUndirectedGraph${class}@{:x}",
            node.0
        )
    }
    fn value_to_string(&self, value: &N) -> String {
        (self.value_to_string)(value)
    }
    fn annotation_stacks(&mut self) -> &mut GraphAnnotationStacks<UndiGraphNode, UndiGraphEdge> {
        &mut self.annotation_stacks
    }
    // port: LinkedUndirectedGraphEdge#getValue
    fn edge_value(&self, edge: UndiGraphEdge) -> &E {
        &self.edge_arena[edge.0].value
    }
    // port: LinkedUndirectedGraphEdge#getNodeA
    fn edge_node_a(&self, edge: UndiGraphEdge) -> UndiGraphNode {
        self.edge_arena[edge.0].node_a
    }
    // port: LinkedUndirectedGraphEdge#getNodeB
    fn edge_node_b(&self, edge: UndiGraphEdge) -> UndiGraphNode {
        self.edge_arena[edge.0].node_b
    }
    // port: LinkedUndirectedGraphEdge#getAnnotation
    // port: AnnotatedLinkedUndirectedGraphEdge#getAnnotation
    fn edge_annotation(&self, edge: UndiGraphEdge) -> &Option<Box<dyn Annotation>> {
        if !self.use_edge_annotations {
            panic!("Graph initialized with edge annotations turned off");
        }
        &self.edge_arena[edge.0].annotation
    }
    // port: LinkedUndirectedGraphEdge#setAnnotation
    // port: AnnotatedLinkedUndirectedGraphEdge#setAnnotation
    fn edge_annotation_mut(&mut self, edge: UndiGraphEdge) -> &mut Option<Box<dyn Annotation>> {
        if !self.use_edge_annotations {
            panic!("Graph initialized with edge annotations turned off");
        }
        &mut self.edge_arena[edge.0].annotation
    }
}
impl<N: Clone + Eq + Hash, E: Clone + PartialEq> UndiGraph<N, E> for LinkedUndirectedGraph<N, E> {
    // port: LinkedUndirectedGraph#createUndirectedGraphNode
    fn create_undirected_graph_node(&mut self, value: N) -> UndiGraphNode {
        if let Some(node) = self.nodes.get(&value) {
            return *node;
        }
        let node = UndiGraphNode(self.node_arena.len());
        self.node_arena
            .push(LinkedUndirectedGraphNode::new(value.clone()));
        self.nodes.insert(value, node);
        node
    }
    // port: LinkedUndirectedGraph#getUndirectedGraphNodes
    fn get_undirected_graph_nodes(&self) -> Vec<UndiGraphNode> {
        self.nodes.values().copied().collect()
    }
    // port: LinkedUndirectedGraph#getUndirectedGraphNode
    fn get_undirected_graph_node(&self, value: &N) -> Option<UndiGraphNode> {
        self.nodes.get(value).copied()
    }
    // port: LinkedUndirectedGraph#getUndirectedGraphEdges
    fn get_undirected_graph_edges(&self, n1: &N, n2: &N) -> Option<Vec<UndiGraphEdge>> {
        let d1 = self.get_node(n1)?;
        let d2 = self.get_node(n2)?;
        Some(
            self.node_arena[d1.0]
                .neighbor_edges
                .iter()
                .copied()
                .filter(|edge| {
                    let e = &self.edge_arena[edge.0];
                    e.node_a == d2 || e.node_b == d2
                })
                .collect(),
        )
    }
    // port: LinkedUndirectedGraphNode#getNeighborEdges
    fn node_neighbor_edges_mut(&mut self, node: UndiGraphNode) -> &mut Vec<UndiGraphEdge> {
        &mut self.node_arena[node.0].neighbor_edges
    }
    // port: LinkedUndirectedGraphNode#getNeighborEdges
    fn node_neighbor_edges(&self, node: UndiGraphNode) -> &[UndiGraphEdge] {
        &self.node_arena[node.0].neighbor_edges
    }
}
impl<N: Clone + Eq + Hash, E: Clone + PartialEq> GraphvizGraph for LinkedUndirectedGraph<N, E> {
    type Node = UndiGraphNode;
    type Edge = UndiGraphEdge;
    // port: LinkedUndirectedGraph#getName
    fn get_name(&self) -> &str {
        "LinkedUndirectedGraph"
    }
    // port: LinkedUndirectedGraph#isDirected
    fn is_directed(&self) -> bool {
        false
    }
    // port: LinkedUndirectedGraph#getGraphvizNodes
    fn get_graphviz_nodes(&self) -> Vec<UndiGraphNode> {
        self.get_nodes()
    }
    // port: LinkedUndirectedGraph#getGraphvizEdges
    fn get_graphviz_edges(&self) -> Vec<UndiGraphEdge> {
        self.get_edges()
    }
}
impl<N: Clone + Eq + Hash, E: Clone + PartialEq> GraphvizNode<LinkedUndirectedGraph<N, E>>
    for UndiGraphNode
{
    // port: LinkedUndirectedGraphNode#getId
    fn get_id(&self, _graph: &LinkedUndirectedGraph<N, E>) -> String {
        // Deviation: Java's identity hash is replaced by the deterministic arena index.
        format!("LDN{}", self.0)
    }
    // port: LinkedUndirectedGraphNode#getColor
    fn get_color(&self, _graph: &LinkedUndirectedGraph<N, E>) -> &str {
        "white"
    }
    // port: LinkedUndirectedGraphNode#getLabel
    fn get_label(&self, graph: &LinkedUndirectedGraph<N, E>) -> String {
        graph.value_to_string(&graph.node_arena[self.0].value)
    }
}
impl<N: Clone + Eq + Hash, E: Clone + PartialEq> GraphvizEdge<LinkedUndirectedGraph<N, E>>
    for UndiGraphEdge
{
    // port: LinkedUndirectedGraphEdge#getNode1Id
    fn get_node1_id(&self, graph: &LinkedUndirectedGraph<N, E>) -> String {
        graph.edge_arena[self.0].node_a.get_id(graph)
    }
    // port: LinkedUndirectedGraphEdge#getNode2Id
    fn get_node2_id(&self, graph: &LinkedUndirectedGraph<N, E>) -> String {
        graph.edge_arena[self.0].node_b.get_id(graph)
    }
    // port: LinkedUndirectedGraphEdge#getColor
    fn get_color(&self, _graph: &LinkedUndirectedGraph<N, E>) -> &str {
        "black"
    }
    // port: LinkedUndirectedGraphEdge#getLabel
    fn get_label(&self, graph: &LinkedUndirectedGraph<N, E>) -> String {
        (graph.edge_value_to_string)(&graph.edge_arena[self.0].value)
    }
}
impl UndiGraphNode {
    // port: Object#toString (LinkedUndirectedGraphNode inherits it)
    pub fn to_string<N, E, G: Graph<N, E, Node = Self>>(self, graph: &G) -> String {
        graph.node_to_string(self)
    }
}
impl UndiGraphEdge {
    // port: LinkedUndirectedGraphEdge#toString
    pub fn to_string<N, E, G: Graph<N, E, Edge = Self>>(self, graph: &G) -> String {
        format!(
            "{} -- {}",
            graph.node_to_string(graph.edge_node_a(self)),
            graph.node_to_string(graph.edge_node_b(self))
        )
    }
}

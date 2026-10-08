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
//   src/com/google/javascript/jscomp/graph/DiGraph.java,
//   src/com/google/javascript/jscomp/graph/Graph.java,
//   src/com/google/javascript/jscomp/graph/LinkedDirectedGraph.java.

use super::{
    adjacency_graph::AdjacencyGraph,
    annotation::Annotation,
    di_graph::{DiGraph, DiGraphEdge, DiGraphNode},
    graph::{Graph, GraphAnnotationStacks, SimpleSubGraph},
    graphviz_graph::{GraphvizEdge, GraphvizGraph, GraphvizNode, GraphvizValue},
    sub_graph::SubGraph,
};
use indexmap::IndexMap;
use std::{fmt::Display, hash::Hash};
pub struct LinkedDirectedGraph<N, E> {
    pub nodes: IndexMap<N, DiGraphNode>,
    node_arena: Vec<LinkedDiGraphNode<N>>,
    edge_arena: Vec<LinkedDiGraphEdge<E>>,
    value_to_string: fn(&N) -> String,
    edge_value_to_string: fn(&E) -> String,
    use_node_annotations: bool,
    use_edge_annotations: bool,
    annotation_stacks: GraphAnnotationStacks<DiGraphNode, DiGraphEdge>,
}
/// Java annotated subclasses are represented by the optional annotation field;
/// the graph's flags choose the same get/set behaviour as the Java subclass.
pub struct LinkedDiGraphNode<N> {
    pub value: N,
    annotation: Option<Box<dyn Annotation>>,
    in_edge_list: Vec<DiGraphEdge>,
    out_edge_list: Vec<DiGraphEdge>,
    priority: i32,
}
impl<N> LinkedDiGraphNode<N> {
    // port: LinkedDiGraphNode#LinkedDiGraphNode
    // port: AnnotatedLinkedDiGraphNode#AnnotatedLinkedDiGraphNode
    fn new(value: N) -> Self {
        Self {
            value,
            annotation: None,
            in_edge_list: Vec::with_capacity(1),
            out_edge_list: Vec::with_capacity(1),
            priority: -1,
        }
    }
}
pub struct LinkedDiGraphEdge<E> {
    value: E,
    node_a: DiGraphNode,
    node_b: DiGraphNode,
    annotation: Option<Box<dyn Annotation>>,
}
impl<E> LinkedDiGraphEdge<E> {
    // port: LinkedDiGraphEdge#LinkedDiGraphEdge
    // port: AnnotatedLinkedDiGraphEdge#AnnotatedLinkedDiGraphEdge
    fn new(node_a: DiGraphNode, value: E, node_b: DiGraphNode) -> Self {
        Self {
            node_a,
            value,
            node_b,
            annotation: None,
        }
    }
}
impl<N: Clone + Eq + Hash, E: Clone + PartialEq> LinkedDirectedGraph<N, E> {
    // port: LinkedDirectedGraph#create
    pub fn create() -> Self
    where
        N: Display,
        E: GraphvizValue,
    {
        Self::new(true, true)
    }
    // port: LinkedDirectedGraph#LinkedDirectedGraph
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
    // port: LinkedDirectedGraph#LinkedDirectedGraph
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
    // port: LinkedDirectedGraph#LinkedDirectedGraph
    pub fn new_with_value_to_strings(
        use_node_annotations: bool,
        use_edge_annotations: bool,
        value_to_string: fn(&N) -> String,
        edge_value_to_string: fn(&E) -> String,
    ) -> Self {
        Self {
            nodes: IndexMap::new(),
            value_to_string,
            edge_value_to_string,
            node_arena: Vec::new(),
            edge_arena: Vec::new(),
            use_node_annotations,
            use_edge_annotations,
            annotation_stacks: GraphAnnotationStacks::default(),
        }
    }
    // port: LinkedDirectedGraph#createWithoutAnnotations
    pub fn create_without_annotations() -> Self
    where
        N: Display,
        E: GraphvizValue,
    {
        Self::new(false, false)
    }
    // port: LinkedDirectedGraph#connect(DiGraphNode,E,DiGraphNode)
    pub fn connect_nodes(&mut self, src: DiGraphNode, edge_value: E, dest: DiGraphNode) {
        let edge = DiGraphEdge(self.edge_arena.len());
        self.edge_arena
            .push(LinkedDiGraphEdge::new(src, edge_value, dest));
        self.node_arena[src.0].out_edge_list.push(edge);
        self.node_arena[dest.0].in_edge_list.push(edge);
    }
    // port: LinkedDirectedGraph#connectIfNotConnectedInDirection
    pub fn connect_if_not_connected_in_direction(
        &mut self,
        src_value: N,
        edge_value: E,
        dest_value: N,
    ) {
        let src = self.create_node(src_value);
        let dest = self.create_node(dest_value);
        if !self.is_connected_in_direction_nodes(src, |e| e == &edge_value, dest) {
            self.connect_nodes(src, edge_value, dest);
        }
    }
    // port: LinkedDirectedGraph#isConnectedInDirection(LinkedDiGraphNode,Predicate,LinkedDiGraphNode)
    pub fn is_connected_in_direction_nodes(
        &self,
        source: DiGraphNode,
        edge_filter: impl Fn(&E) -> bool,
        dest: DiGraphNode,
    ) -> bool {
        let out_edges = &self.node_arena[source.0].out_edge_list;
        let in_edges = &self.node_arena[dest.0].in_edge_list;
        if out_edges.len() < in_edges.len() {
            for edge in out_edges {
                let e = &self.edge_arena[edge.0];
                if e.node_b == dest && edge_filter(&e.value) {
                    return true;
                }
            }
        } else {
            for edge in in_edges {
                let e = &self.edge_arena[edge.0];
                if e.node_a == source && edge_filter(&e.value) {
                    return true;
                }
            }
        }
        false
    }
    // port: LinkedDirectedGraph#isConnectedInDirection(N,Predicate,N)
    pub fn is_connected_in_direction_matching(
        &self,
        n1: &N,
        edge_matcher: impl Fn(&E) -> bool,
        n2: &N,
    ) -> bool {
        self.is_connected_in_direction_nodes(
            self.get_node_or_fail(n1),
            edge_matcher,
            self.get_node_or_fail(n2),
        )
    }
}
impl<N: Clone + Eq + Hash, E: Clone + PartialEq> AdjacencyGraph<N, E>
    for LinkedDirectedGraph<N, E>
{
    type Node = DiGraphNode;
    // port: LinkedDirectedGraph#getNodes
    fn get_nodes(&self) -> Vec<DiGraphNode> {
        self.nodes.values().copied().collect()
    }
    // port: LinkedDirectedGraph#getNodeCount
    fn get_node_count(&self) -> usize {
        self.nodes.len()
    }
    // port: LinkedDirectedGraph#getNode
    fn get_node(&self, value: &N) -> Option<DiGraphNode> {
        self.nodes.get(value).copied()
    }
    // port: LinkedDirectedGraph#newSubGraph
    fn new_sub_graph(&self) -> Box<dyn SubGraph<N, E, Self>> {
        Box::new(SimpleSubGraph::<DiGraphNode>::new())
    }
    // port: Graph#getWeight
    fn get_weight(&self, value: &N) -> i32 {
        self.get_node_degree(value)
    }
    // port: LinkedDiGraphNode#getValue
    fn node_value(&self, node: DiGraphNode) -> &N {
        &self.node_arena[node.0].value
    }
    // port: LinkedDiGraphNode#getAnnotation
    // port: AnnotatedLinkedDiGraphNode#getAnnotation
    fn node_annotation(&self, node: DiGraphNode) -> &Option<Box<dyn Annotation>> {
        if !self.use_node_annotations {
            panic!("Graph initialized with node annotations turned off");
        }
        &self.node_arena[node.0].annotation
    }
    // port: LinkedDiGraphNode#setAnnotation
    // port: AnnotatedLinkedDiGraphNode#setAnnotation
    fn node_annotation_mut(&mut self, node: DiGraphNode) -> &mut Option<Box<dyn Annotation>> {
        if !self.use_node_annotations {
            panic!("Graph initialized with node annotations turned off");
        }
        &mut self.node_arena[node.0].annotation
    }
}
impl<N: Clone + Eq + Hash, E: Clone + PartialEq> Graph<N, E> for LinkedDirectedGraph<N, E> {
    type Edge = DiGraphEdge;
    // port: LinkedDirectedGraph#connect(N,E,N)
    fn connect(&mut self, n1: N, edge_value: E, n2: N) {
        let src = self.get_node_or_fail(&n1);
        let dest = self.get_node_or_fail(&n2);
        self.connect_nodes(src, edge_value, dest);
    }
    // port: LinkedDirectedGraph#disconnect
    fn disconnect(&mut self, n1: &N, n2: &N) {
        self.disconnect_in_direction(n1, n2);
        self.disconnect_in_direction(n2, n1);
    }
    // port: LinkedDirectedGraph#createNode
    fn create_node(&mut self, value: N) -> DiGraphNode {
        if let Some(node) = self.nodes.get(&value) {
            return *node;
        }
        let node = DiGraphNode(self.node_arena.len());
        self.node_arena.push(LinkedDiGraphNode::new(value.clone()));
        self.nodes.insert(value, node);
        node
    }
    // port: LinkedDirectedGraph#getEdges()
    fn get_edges(&self) -> Vec<DiGraphEdge> {
        let mut result = Vec::new();
        for node in self.nodes.values() {
            result.extend(&self.node_arena[node.0].out_edge_list);
        }
        result
    }
    // port: LinkedDirectedGraph#getEdges(N,N)
    fn get_edges_between(&self, n1: &N, n2: &N) -> Vec<DiGraphEdge> {
        let mut edges = self.get_edges_in_direction(n1, n2);
        edges.extend(self.get_edges_in_direction(n2, n1));
        edges
    }
    // port: LinkedDirectedGraph#getFirstEdge
    fn get_first_edge(&self, n1: &N, n2: &N) -> Option<DiGraphEdge> {
        let d1 = self.get_node_or_fail(n1);
        let d2 = self.get_node_or_fail(n2);
        for edge in &self.node_arena[d1.0].out_edge_list {
            if self.edge_arena[edge.0].node_b == d2 {
                return Some(*edge);
            }
        }
        for edge in &self.node_arena[d2.0].out_edge_list {
            if self.edge_arena[edge.0].node_b == d1 {
                return Some(*edge);
            }
        }
        None
    }
    // port: LinkedDirectedGraph#getNodeDegree
    fn get_node_degree(&self, value: &N) -> i32 {
        let node = &self.node_arena[self.get_node_or_fail(value).0];
        (node.in_edge_list.len() + node.out_edge_list.len()) as i32
    }
    // port: LinkedDirectedGraph#getNeighborNodes
    fn get_neighbor_nodes(&self, value: &N) -> Vec<DiGraphNode> {
        let node = self.get_node(value).expect("");
        let data = &self.node_arena[node.0];
        let mut result = Vec::with_capacity(data.in_edge_list.len() + data.out_edge_list.len());
        for e in &data.in_edge_list {
            result.push(self.edge_arena[e.0].node_a);
        }
        for e in &data.out_edge_list {
            result.push(self.edge_arena[e.0].node_b);
        }
        result
    }
    // port: DiGraph#isConnected(N,N)
    fn is_connected(&self, n1: &N, n2: &N) -> bool {
        self.is_connected_in_direction(n1, n2) || self.is_connected_in_direction(n2, n1)
    }
    // port: DiGraph#isConnected(N,E,N)
    fn is_connected_with_edge(&self, n1: &N, edge: &E, n2: &N) -> bool {
        self.is_connected_in_direction_with_edge(n1, edge, n2)
            || self.is_connected_in_direction_with_edge(n2, edge, n1)
    }
    fn value_to_string(&self, value: &N) -> String {
        (self.value_to_string)(value)
    }
    fn annotation_stacks(&mut self) -> &mut GraphAnnotationStacks<DiGraphNode, DiGraphEdge> {
        &mut self.annotation_stacks
    }
    // port: LinkedDiGraphEdge#getValue
    fn edge_value(&self, edge: DiGraphEdge) -> &E {
        &self.edge_arena[edge.0].value
    }
    // port: LinkedDiGraphEdge#getNodeA
    fn edge_node_a(&self, edge: DiGraphEdge) -> DiGraphNode {
        self.edge_arena[edge.0].node_a
    }
    // port: LinkedDiGraphEdge#getNodeB
    fn edge_node_b(&self, edge: DiGraphEdge) -> DiGraphNode {
        self.edge_arena[edge.0].node_b
    }
    // port: LinkedDiGraphEdge#getAnnotation
    // port: AnnotatedLinkedDiGraphEdge#getAnnotation
    fn edge_annotation(&self, edge: DiGraphEdge) -> &Option<Box<dyn Annotation>> {
        if !self.use_edge_annotations {
            panic!("Graph initialized with edge annotations turned off");
        }
        &self.edge_arena[edge.0].annotation
    }
    // port: LinkedDiGraphEdge#setAnnotation
    // port: AnnotatedLinkedDiGraphEdge#setAnnotation
    fn edge_annotation_mut(&mut self, edge: DiGraphEdge) -> &mut Option<Box<dyn Annotation>> {
        if !self.use_edge_annotations {
            panic!("Graph initialized with edge annotations turned off");
        }
        &mut self.edge_arena[edge.0].annotation
    }
}
impl<N: Clone + Eq + Hash, E: Clone + PartialEq> DiGraph<N, E> for LinkedDirectedGraph<N, E> {
    // port: LinkedDirectedGraph#getEdgesInDirection
    fn get_edges_in_direction(&self, n1: &N, n2: &N) -> Vec<DiGraphEdge> {
        let d1 = self.get_node_or_fail(n1);
        let d2 = self.get_node_or_fail(n2);
        self.node_arena[d1.0]
            .out_edge_list
            .iter()
            .copied()
            .filter(|e| self.edge_arena[e.0].node_b == d2)
            .collect()
    }
    // port: LinkedDirectedGraph#getOutEdges
    fn get_out_edges(&self, value: &N) -> &[DiGraphEdge] {
        &self.node_arena[self.get_node_or_fail(value).0].out_edge_list
    }
    // port: LinkedDirectedGraph#getInEdges
    fn get_in_edges(&self, value: &N) -> &[DiGraphEdge] {
        &self.node_arena[self.get_node_or_fail(value).0].in_edge_list
    }
    // port: LinkedDirectedGraph#getDirectedPredNodes(N)
    fn get_directed_pred_nodes(&self, value: &N) -> Vec<DiGraphNode> {
        self.get_directed_pred_nodes_of(self.get_node(value).expect(""))
    }
    // port: LinkedDirectedGraph#getDirectedSuccNodes(N)
    fn get_directed_succ_nodes(&self, value: &N) -> Vec<DiGraphNode> {
        self.get_directed_succ_nodes_of(self.get_node(value).expect(""))
    }
    // port: LinkedDirectedGraph#disconnectInDirection
    fn disconnect_in_direction(&mut self, n1: &N, n2: &N) {
        let src = self.get_node_or_fail(n1);
        let dest = self.get_node_or_fail(n2);
        for edge in self.get_edges_in_direction(n1, n2) {
            let out = &mut self.node_arena[src.0].out_edge_list;
            if let Some(i) = out.iter().position(|e| *e == edge) {
                out.remove(i);
            }
            let ins = &mut self.node_arena[dest.0].in_edge_list;
            if let Some(i) = ins.iter().position(|e| *e == edge) {
                ins.remove(i);
            }
        }
    }
    // port: LinkedDirectedGraph#isConnectedInDirection(N,N)
    fn is_connected_in_direction(&self, n1: &N, n2: &N) -> bool {
        self.is_connected_in_direction_matching(n1, |_| true, n2)
    }
    // port: LinkedDirectedGraph#isConnectedInDirection(N,E,N)
    fn is_connected_in_direction_with_edge(&self, n1: &N, e: &E, n2: &N) -> bool {
        self.is_connected_in_direction_matching(n1, |v| v == e, n2)
    }
    // port: LinkedDiGraphNode#getOutEdges
    fn node_out_edges(&self, node: DiGraphNode) -> &[DiGraphEdge] {
        &self.node_arena[node.0].out_edge_list
    }
    // port: LinkedDiGraphNode#getInEdges
    fn node_in_edges(&self, node: DiGraphNode) -> &[DiGraphEdge] {
        &self.node_arena[node.0].in_edge_list
    }
    // port: LinkedDiGraphNode#getOutEdges
    fn node_out_edges_mut(&mut self, node: DiGraphNode) -> &mut Vec<DiGraphEdge> {
        &mut self.node_arena[node.0].out_edge_list
    }
    // port: LinkedDiGraphNode#getInEdges
    fn node_in_edges_mut(&mut self, node: DiGraphNode) -> &mut Vec<DiGraphEdge> {
        &mut self.node_arena[node.0].in_edge_list
    }
    // port: LinkedDiGraphNode#getPriority
    fn node_priority(&self, node: DiGraphNode) -> i32 {
        self.node_arena[node.0].priority
    }
    // port: LinkedDiGraphNode#setPriority
    fn node_priority_mut(&mut self, node: DiGraphNode) -> &mut i32 {
        &mut self.node_arena[node.0].priority
    }
}
impl<N: Clone + Eq + Hash, E: Clone + PartialEq> GraphvizGraph for LinkedDirectedGraph<N, E> {
    type Node = DiGraphNode;
    type Edge = DiGraphEdge;
    // port: LinkedDirectedGraph#getName
    fn get_name(&self) -> &str {
        "LinkedGraph"
    }
    // port: LinkedDirectedGraph#isDirected
    fn is_directed(&self) -> bool {
        true
    }
    // port: LinkedDirectedGraph#getGraphvizNodes
    fn get_graphviz_nodes(&self) -> Vec<DiGraphNode> {
        self.get_nodes()
    }
    // port: LinkedDirectedGraph#getGraphvizEdges
    fn get_graphviz_edges(&self) -> Vec<DiGraphEdge> {
        self.get_edges()
    }
}
impl<N: Clone + Eq + Hash, E: Clone + PartialEq> GraphvizNode<LinkedDirectedGraph<N, E>>
    for DiGraphNode
{
    // port: LinkedDiGraphNode#getId
    fn get_id(&self, _graph: &LinkedDirectedGraph<N, E>) -> String {
        // Deviation: Java's identity hash is replaced by the deterministic arena index.
        format!("LDN{}", self.0)
    }
    // port: LinkedDiGraphNode#getColor
    fn get_color(&self, _graph: &LinkedDirectedGraph<N, E>) -> &str {
        "white"
    }
    // port: LinkedDiGraphNode#getLabel
    // port: AnnotatedLinkedDiGraphNode#getLabel
    fn get_label(&self, graph: &LinkedDirectedGraph<N, E>) -> String {
        let mut result = graph.value_to_string(&graph.node_arena[self.0].value);
        if let Some(a) = &graph.node_arena[self.0].annotation {
            result.push('\n');
            result.push_str(&a.to_string());
        }
        result
    }
}
impl<N: Clone + Eq + Hash, E: Clone + PartialEq> GraphvizEdge<LinkedDirectedGraph<N, E>>
    for DiGraphEdge
{
    // port: LinkedDiGraphEdge#getNode1Id
    fn get_node1_id(&self, graph: &LinkedDirectedGraph<N, E>) -> String {
        graph.edge_arena[self.0].node_a.get_id(graph)
    }
    // port: LinkedDiGraphEdge#getNode2Id
    fn get_node2_id(&self, graph: &LinkedDirectedGraph<N, E>) -> String {
        graph.edge_arena[self.0].node_b.get_id(graph)
    }
    // port: LinkedDiGraphEdge#getColor
    fn get_color(&self, _graph: &LinkedDirectedGraph<N, E>) -> &str {
        "black"
    }
    // port: LinkedDiGraphEdge#getLabel
    fn get_label(&self, graph: &LinkedDirectedGraph<N, E>) -> String {
        (graph.edge_value_to_string)(&graph.edge_arena[self.0].value)
    }
}
impl DiGraphNode {
    // port: LinkedDiGraphNode#toString
    pub fn to_string<N, E, G: Graph<N, E, Node = Self>>(self, graph: &G) -> String {
        graph.node_to_string(self)
    }
}
impl DiGraphEdge {
    // port: LinkedDiGraphEdge#toString
    pub fn to_string<N, E, G: Graph<N, E, Edge = Self>>(self, graph: &G) -> String {
        format!(
            "{} -> {}",
            graph.node_to_string(graph.edge_node_a(self)),
            graph.node_to_string(graph.edge_node_b(self))
        )
    }
}

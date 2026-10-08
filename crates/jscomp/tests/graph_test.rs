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
//   test/com/google/javascript/jscomp/graph/GraphTest.java.

use closure_jscomp::graph::graphviz_graph::{GraphvizEdge, GraphvizGraph, GraphvizNode};
use closure_jscomp::graph::*;
use indexmap::IndexSet;
use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
};
// port: GraphTest#testDirectedSimple
#[test]
fn test_directed_simple() {
    let mut graph = LinkedDirectedGraph::<&str, Option<&str>>::create();
    graph.create_node("a");
    graph.create_node("b");
    graph.create_node("c");
    graph.connect("a", Some("->"), "b");
    assert!(graph.has_node(&"a"));
    assert!(graph.has_node(&"b"));
    assert!(graph.has_node(&"c"));
    assert!(!graph.has_node(&"d"));
    assert!(graph.is_connected(&"a", &"b"));
    assert!(graph.is_connected(&"b", &"a"));
    assert!(!graph.is_connected(&"a", &"c"));
    assert!(!graph.is_connected(&"b", &"c"));
    assert!(!graph.is_connected(&"c", &"a"));
    assert!(!graph.is_connected(&"c", &"b"));
    assert!(!graph.is_connected(&"a", &"a"));
    assert!(!graph.is_connected(&"b", &"b"));
    assert!(!graph.is_connected(&"b", &"c"));
    assert!(graph.is_connected_in_direction(&"a", &"b"));
    assert!(!graph.is_connected_in_direction(&"b", &"a"));
    assert!(!graph.is_connected_in_direction(&"a", &"c"));
    assert!(!graph.is_connected_in_direction(&"b", &"c"));
    assert!(!graph.is_connected_in_direction(&"c", &"a"));
    assert!(!graph.is_connected_in_direction(&"c", &"b"));

    // Removal.
    graph.disconnect(&"a", &"b");
    assert!(!graph.is_connected(&"a", &"b"));
    assert!(!graph.is_connected(&"b", &"a"));

    // Disconnect both ways.
    graph.connect("a", Some("->"), "b");
    graph.connect("b", Some("->"), "a");
    graph.disconnect(&"a", &"b");
    assert!(!graph.is_connected(&"a", &"b"));
    assert!(!graph.is_connected(&"b", &"a"));

    // Disconnect one way.
    graph.connect("a", Some("->"), "b");
    graph.connect("b", Some("->"), "a");
    graph.disconnect_in_direction(&"a", &"b");
    assert!(graph.is_connected(&"b", &"a"));
    assert!(graph.is_connected(&"a", &"b"));
    assert!(!graph.is_connected_in_direction(&"a", &"b"));
    assert!(graph.is_connected_in_direction(&"b", &"a"));
}
// port: GraphTest#testUndirectedSimple
#[test]
fn test_undirected_simple() {
    let mut graph = LinkedUndirectedGraph::<&str, Option<&str>>::create();
    graph.create_node("a");
    graph.create_node("b");
    graph.create_node("c");
    graph.connect("a", Some("--"), "b");
    assert!(graph.has_node(&"a"));
    assert!(graph.has_node(&"b"));
    assert!(graph.has_node(&"c"));
    assert!(!graph.has_node(&"d"));
    assert!(graph.is_connected(&"a", &"b"));
    assert!(graph.is_connected(&"b", &"a"));
    assert!(!graph.is_connected(&"a", &"c"));
    assert!(!graph.is_connected(&"b", &"c"));
    assert!(!graph.is_connected(&"c", &"a"));
    assert!(!graph.is_connected(&"c", &"b"));
    assert!(!graph.is_connected(&"a", &"a"));
    assert!(!graph.is_connected(&"b", &"b"));
    assert!(!graph.is_connected(&"b", &"c"));

    // Removal.
    graph.disconnect(&"a", &"b");
    assert!(!graph.is_connected(&"a", &"b"));
    assert!(!graph.is_connected(&"b", &"a"));
}
// port: GraphTest#testDirectedSelfLoop
#[test]
fn test_directed_self_loop() {
    let mut graph = LinkedDirectedGraph::<&str, Option<&str>>::create();
    graph.create_node("a");
    graph.create_node("b");
    graph.connect("a", Some("->"), "a");
    assert!(graph.is_connected(&"a", &"a"));
    assert!(!graph.is_connected(&"a", &"b"));
    assert!(!graph.is_connected(&"b", &"a"));
    assert!(graph.is_connected_in_direction(&"a", &"a"));
    assert!(!graph.is_connected_in_direction(&"a", &"b"));
    assert!(!graph.is_connected_in_direction(&"b", &"a"));

    // Removal.
    graph.disconnect(&"a", &"a");
    assert!(!graph.is_connected(&"a", &"a"));

    // Disconnect both ways.
    graph.connect("a", Some("->"), "a");
    graph.disconnect(&"a", &"a");
    assert!(!graph.is_connected(&"a", &"a"));
    assert!(!graph.is_connected(&"a", &"a"));

    // Disconnect one way.
    graph.connect("a", Some("->"), "a");
    graph.disconnect_in_direction(&"a", &"a");
    assert!(!graph.is_connected(&"a", &"a"));
}
// port: GraphTest#testUndirectedSelfLoop
#[test]
fn test_undirected_self_loop() {
    let mut graph = LinkedUndirectedGraph::<&str, Option<&str>>::create();
    graph.create_node("a");
    graph.create_node("b");
    graph.connect("a", Some("--"), "a");
    assert!(graph.is_connected(&"a", &"a"));
    assert!(!graph.is_connected(&"a", &"b"));
    assert!(!graph.is_connected(&"b", &"a"));

    // Removal.
    graph.disconnect(&"a", &"a");
    assert!(!graph.is_connected(&"a", &"a"));
}
// port: GraphTest#testDirectedInAndOutEdges
#[test]
fn test_directed_in_and_out_edges() {
    let mut graph = LinkedDirectedGraph::<&str, Option<&str>>::create();
    graph.create_node("a");
    graph.create_node("b");
    graph.create_node("c");
    graph.create_node("d");
    graph.connect("a", Some("->"), "b");
    graph.connect("a", Some("-->"), "b");
    graph.connect("a", Some("--->"), "b");
    graph.connect("a", Some("->"), "c");
    graph.connect("c", Some("->"), "d");
    assert_set_equals(&graph, graph.get_directed_succ_nodes(&"a"), &["b", "c"]);
    assert_set_equals(&graph, graph.get_directed_pred_nodes(&"b"), &["a"]);
    assert_set_equals(&graph, graph.get_directed_pred_nodes(&"c"), &["a"]);
    assert_list_count(&graph, graph.get_directed_succ_nodes(&"a"), "b", 3);

    // Removal.
    graph.disconnect(&"a", &"b");
    assert!(!graph.is_connected(&"a", &"b"));
}
// port: GraphTest#testUndirectedNeighbors
#[test]
fn test_undirected_neighbors() {
    let mut graph = LinkedUndirectedGraph::<&str, Option<&str>>::create();
    graph.create_node("a");
    graph.create_node("b");
    graph.create_node("c");
    graph.create_node("d");
    graph.connect("a", Some("-"), "b");
    graph.connect("a", Some("--"), "b");
    graph.connect("a", Some("---"), "b");
    graph.connect("a", Some("-"), "c");
    graph.connect("c", Some("-"), "d");
    assert_set_equals(&graph, graph.get_neighbor_nodes(&"a"), &["b", "c"]);
    assert_set_equals(&graph, graph.get_neighbor_nodes(&"b"), &["a"]);
    assert_set_equals(&graph, graph.get_neighbor_nodes(&"c"), &["a", "d"]);
    assert_list_count(&graph, graph.get_neighbor_nodes(&"a"), "b", 3);

    // Removal.
    graph.disconnect(&"a", &"b");
    assert!(!graph.is_connected(&"a", &"b"));
}
// port: GraphTest#testDirectedGetFirstEdge
#[test]
fn test_directed_get_first_edge() {
    let mut graph = LinkedDirectedGraph::<&str, Option<&str>>::create();
    graph.create_node("a");
    graph.create_node("b");
    graph.create_node("c");
    graph.connect("a", Some("-"), "b");
    assert_eq!(
        *graph.get_first_edge(&"a", &"b").unwrap().get_value(&graph),
        Some("-")
    );
    assert_eq!(
        *graph.get_first_edge(&"b", &"a").unwrap().get_value(&graph),
        Some("-")
    );
    assert_eq!(graph.get_first_edge(&"a", &"c"), None);
}
// port: GraphTest#testUndirectedGetFirstEdge
#[test]
fn test_undirected_get_first_edge() {
    let mut graph = LinkedUndirectedGraph::<&str, Option<&str>>::create();
    graph.create_node("a");
    graph.create_node("b");
    graph.create_node("c");
    graph.connect("a", Some("-"), "b");
    assert_eq!(
        *graph.get_first_edge(&"a", &"b").unwrap().get_value(&graph),
        Some("-")
    );
    assert_eq!(
        *graph.get_first_edge(&"b", &"a").unwrap().get_value(&graph),
        Some("-")
    );
    assert_eq!(graph.get_first_edge(&"a", &"c"), None);
}
// Rust-only (no Java counterpart): Graphviz, priority and disabled-annotation
// behaviour of LinkedDirectedGraph, checked against LinkedDirectedGraph.java.
#[test]
fn rust_only_linked_directed_graph_graphviz_and_priority() {
    let mut graph = LinkedDirectedGraph::<&str, Option<&str>>::create();
    graph.create_node("a");
    graph.create_node("b");
    graph.create_node("c");
    graph.connect("a", Some("-"), "b");
    assert_eq!(graph.get_name(), "LinkedGraph");
    assert!(graph.is_directed());
    let node = graph.get_node(&"a").unwrap();
    assert!(!node.has_priority(&graph));
    assert!(catch_unwind(AssertUnwindSafe(|| node.get_priority(&graph))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| node.set_priority(&mut graph, -1))).is_err());
    node.set_priority(&mut graph, 3);
    assert!(node.has_priority(&graph));
    assert_eq!(node.get_priority(&graph), 3);
    assert_eq!(node.get_id(&graph), "LDN0");
    assert_eq!(GraphvizNode::get_color(&node, &graph), "white");
    assert_eq!(node.get_label(&graph), "a");
    let edge = graph.get_edges()[0];
    assert_eq!(edge.to_string(&graph), "a -> b");
    assert_eq!(edge.get_label(&graph), "-");
    graph.connect("a", None, "c");
    assert_eq!(graph.get_edges()[1].get_label(&graph), "null");
    let text_graph = LinkedDirectedGraph::<&str, &str>::create();
    assert_eq!(text_graph.get_graphviz_nodes().len(), 0);
    assert_eq!(text_graph.get_graphviz_edges().len(), 0);
    let plain = LinkedDirectedGraph::<&str, &str>::create_without_annotations();
    let mut plain = plain;
    let node = plain.create_node("x");
    assert!(catch_unwind(AssertUnwindSafe(|| node.get_annotation(&plain))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| node.set_annotation(&mut plain, None))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| plain.push_node_annotations())).is_err());
    plain.pop_node_annotations();
}
// Rust-only (no Java counterpart): Graphviz behaviour and inherited Object#toString
// text of LinkedUndirectedGraph, checked against LinkedUndirectedGraph.java.
#[test]
fn rust_only_linked_undirected_graph_graphviz() {
    let mut dot = LinkedUndirectedGraph::<&str, &str>::create();
    let a = dot.create_node("a");
    let b = dot.create_node("b");
    dot.connect("a", "-", "b");
    assert_eq!(dot.get_name(), "LinkedUndirectedGraph");
    assert!(!dot.is_directed());
    assert_eq!(dot.get_graphviz_nodes(), [a, b]);
    assert_eq!(a.get_label(&dot), "a");
    let edge = dot.get_graphviz_edges()[0];
    assert_eq!(edge.get_node1_id(&dot), "LDN0");
    assert_eq!(edge.get_node2_id(&dot), "LDN1");
    assert_eq!(GraphvizEdge::get_color(&edge, &dot), "black");
    assert_eq!(edge.get_label(&dot), "-");
    assert_eq!(
        edge.to_string(&dot),
        "com.google.javascript.jscomp.graph.LinkedUndirectedGraph$AnnotatedLinkedUndirectedGraphNode@0 -- com.google.javascript.jscomp.graph.LinkedUndirectedGraph$AnnotatedLinkedUndirectedGraphNode@1"
    );
}
// port: GraphTest#assertListCount
fn assert_list_count<G: Graph<&'static str, Option<&'static str>>>(
    graph: &G,
    list: Vec<G::Node>,
    target: &str,
    mut count: i32,
) {
    for node in list {
        if *node.get_value(graph) == target {
            count -= 1;
        }
    }
    assert_eq!(count, 0);
}
// port: GraphTest#assertSetEquals
fn assert_set_equals<G: Graph<&'static str, Option<&'static str>>>(
    graph: &G,
    actual: Vec<G::Node>,
    expected: &[&str],
) {
    let actual: IndexSet<_> = actual.into_iter().map(|n| *n.get_value(graph)).collect();
    assert_eq!(actual, expected.iter().copied().collect::<IndexSet<_>>());
}
struct A;
struct B;
impl Annotation for A {
    fn to_string(&self) -> String {
        "com.google.javascript.jscomp.graph.GraphTest$1@0".to_owned()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
impl Annotation for B {
    fn to_string(&self) -> String {
        "com.google.javascript.jscomp.graph.GraphTest$2@0".to_owned()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
// port: GraphTest#checkAnnotations
fn check_annotations<
    G: Graph<&'static str, Option<&'static str>>,
    H: Annotatable<&'static str, Option<&'static str>, G>,
>(
    graph: &mut G,
    a: H,
    b: H,
) {
    assert!(a.get_annotation(graph).is_none());
    assert!(b.get_annotation(graph).is_none());
    a.set_annotation(graph, Some(Box::new(A)));
    b.set_annotation(graph, Some(Box::new(B)));
    assert!(a.get_annotation(graph).unwrap().as_any().is::<A>());
    assert!(b.get_annotation(graph).unwrap().as_any().is::<B>());
    graph.clear_edge_annotations();
    graph.clear_node_annotations();
    assert!(a.get_annotation(graph).is_none());
    assert!(b.get_annotation(graph).is_none());
    a.set_annotation(graph, Some(Box::new(A)));
    b.set_annotation(graph, Some(Box::new(B)));
    graph.push_edge_annotations();
    graph.push_node_annotations();
    assert!(a.get_annotation(graph).is_none());
    assert!(b.get_annotation(graph).is_none());
    a.set_annotation(graph, Some(Box::new(B)));
    b.set_annotation(graph, Some(Box::new(B)));
    graph.push_edge_annotations();
    graph.push_node_annotations();
    a.set_annotation(graph, Some(Box::new(B)));
    b.set_annotation(graph, Some(Box::new(A)));
    assert!(a.get_annotation(graph).unwrap().as_any().is::<B>());
    assert!(b.get_annotation(graph).unwrap().as_any().is::<A>());
    graph.pop_edge_annotations();
    graph.pop_node_annotations();
    assert!(a.get_annotation(graph).unwrap().as_any().is::<B>());
    assert!(b.get_annotation(graph).unwrap().as_any().is::<B>());
    graph.pop_edge_annotations();
    graph.pop_node_annotations();
    assert!(a.get_annotation(graph).unwrap().as_any().is::<A>());
    assert!(b.get_annotation(graph).unwrap().as_any().is::<B>());
}
// port: GraphTest#testNodeAnnotations
#[test]
fn test_node_annotations() {
    let mut graph = LinkedUndirectedGraph::<&str, Option<&str>>::create();
    let a = graph.create_node("a");
    let b = graph.create_node("b");
    check_annotations(&mut graph, a, b);
}
// port: GraphTest#testEdgeAnnotations
#[test]
fn test_edge_annotations() {
    let mut graph = LinkedUndirectedGraph::<&str, Option<&str>>::create();
    for n in ["1", "2", "3"] {
        graph.create_node(n);
    }
    graph.connect("1", Some("a"), "2");
    graph.connect("2", Some("b"), "3");
    let a = graph.get_edges_between(&"1", &"2")[0];
    let b = graph.get_edges_between(&"2", &"3")[0];
    check_annotations(&mut graph, a, b);
}
// port: GraphTest#testDegree
#[test]
fn test_degree() {
    test_directed_degree(LinkedDirectedGraph::create());
    test_directed_degree(LinkedUndirectedGraph::create());
}
// port: GraphTest#testDirectedDegree
fn test_directed_degree<G: Graph<&'static str, Option<&'static str>>>(mut graph: G) {
    for n in ["a", "b", "c", "d"] {
        graph.create_node(n);
    }
    assert_eq!(graph.get_node_degree(&"a"), 0);
    graph.connect("a", Some("-"), "b");
    assert_eq!(graph.get_node_degree(&"a"), 1);
    graph.connect("b", Some("-"), "c");
    assert_eq!(graph.get_node_degree(&"a"), 1);
    graph.connect("a", Some("-"), "c");
    assert_eq!(graph.get_node_degree(&"a"), 2);
    graph.connect("d", Some("-"), "a");
    assert_eq!(graph.get_node_degree(&"a"), 3);
}
// port: GraphTest#testDirectedConnectIfNotFound
#[test]
fn test_directed_connect_if_not_found() {
    check_connect_if_not_found(LinkedDirectedGraph::create());
    check_connect_if_not_found(LinkedUndirectedGraph::create());
}
// port: GraphTest#testDirectedConnectIfNotFound(Graph)
fn check_connect_if_not_found<G: Graph<&'static str, Option<&'static str>>>(mut graph: G) {
    graph.create_node("a");
    graph.create_node("b");
    graph.connect_if_not_found("a", Some("-"), "b");
    assert_eq!(graph.get_node_degree(&"a"), 1);
    graph.connect_if_not_found("a", Some("-"), "b");
    assert_eq!(graph.get_node_degree(&"a"), 1);
    graph.connect_if_not_found("a", None, "b");
    assert_eq!(graph.get_node_degree(&"a"), 2);
    graph.connect_if_not_found("a", None, "b");
    assert_eq!(graph.get_node_degree(&"a"), 2);
}
// port: GraphTest#testSimpleSubGraph
#[test]
fn test_simple_sub_graph() {
    let mut graph = LinkedUndirectedGraph::<&str, Option<&str>>::create();
    for n in ["a", "b", "c"] {
        graph.create_node(n);
    }
    graph.connect("a", Some("--"), "b");
    let mut subgraph = graph.new_sub_graph();
    subgraph.add_node(&graph, &"a");
    subgraph.add_node(&graph, &"b");
    assert!(
        catch_unwind(AssertUnwindSafe(|| subgraph.add_node(&graph, &"d"))).is_err(),
        "SubGraph should not allow add for node that is not in graph."
    );
    assert!(!subgraph.is_independent_of(&graph, &"a"));
    assert!(!subgraph.is_independent_of(&graph, &"b"));
    assert!(subgraph.is_independent_of(&graph, &"c"));
}

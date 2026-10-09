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
//   test/com/google/javascript/jscomp/graph/GraphColoringTest.java.

use closure_jscomp::graph::graph_coloring::{Color, GraphColoring, GreedyGraphColoring};
use closure_jscomp::graph::*;
type TestGraph = LinkedUndirectedGraph<String, Option<&'static str>>;
// port: GraphColoringTest#validateColoring
fn validate_coloring<G: Graph<N, E>, N, E>(graph: &G) {
    for node in graph.get_nodes() {
        assert!(node.get_annotation(graph).is_some());
    }
    for edge in graph.get_edges() {
        let c1 = edge.get_node_a(graph).get_annotation_as::<Color>(graph);
        let c2 = edge.get_node_b(graph).get_annotation_as::<Color>(graph);
        assert!(c1.is_some());
        assert!(c2.is_some());
        assert_ne!(c1, c2);
    }
}
// port: GraphColoringTest#testNoEdge
#[test]
fn test_no_edge() {
    let mut graph = TestGraph::create();
    for i in 0..5 {
        graph.create_node(format!("Node {i}"));
        let mut coloring = GreedyGraphColoring::new();
        assert_eq!(coloring.color(&mut graph), 1);
        validate_coloring(&graph);
        for _j in 0..i {
            assert_eq!(
                coloring.get_partition_super_node(&graph, "Node 0".into()),
                "Node 0"
            );
        }
    }
}
// port: GraphColoringTest#testTwoNodesConnected
#[test]
fn test_two_nodes_connected() {
    let mut graph = TestGraph::create();
    graph.create_node("A".into());
    graph.create_node("B".into());
    graph.connect("A".into(), Some("--"), "B".into());
    let mut coloring = GreedyGraphColoring::new();
    assert_eq!(coloring.color(&mut graph), 2);
    validate_coloring(&graph);
    assert_eq!(coloring.get_partition_super_node(&graph, "A".into()), "A");
    assert_eq!(coloring.get_partition_super_node(&graph, "B".into()), "B");
}
// port: GraphColoringTest#testGreedy
#[test]
fn test_greedy() {
    let mut graph = TestGraph::create();
    for n in ["A", "B", "C", "D"] {
        graph.create_node(n.into());
    }
    for (a, b) in [("A", "C"), ("B", "C"), ("B", "D")] {
        graph.connect(a.into(), Some("--"), b.into());
    }
    let mut coloring = GreedyGraphColoring::new();
    assert_eq!(coloring.color(&mut graph), 2);
    validate_coloring(&graph);
    assert_eq!(coloring.get_partition_super_node(&graph, "A".into()), "A");
    assert_eq!(coloring.get_partition_super_node(&graph, "B".into()), "A");
    assert_eq!(coloring.get_partition_super_node(&graph, "C".into()), "C");
}
// port: GraphColoringTest#testFullyConnected
#[test]
fn test_fully_connected() {
    let count = 100;
    let mut graph = TestGraph::create();
    for i in 0..count {
        graph.create_node(format!("Node {i}"));
        for j in 0..count {
            graph.create_node(format!("Node {j}"));
            if i != j {
                graph.connect(format!("Node {i}"), None, format!("Node {j}"));
            }
        }
    }
    let mut coloring = GreedyGraphColoring::new();
    assert_eq!(coloring.color(&mut graph), count);
    validate_coloring(&graph);
    for i in 0..count {
        assert_eq!(
            coloring.get_partition_super_node(&graph, format!("Node {i}")),
            format!("Node {i}")
        );
    }
}
// port: GraphColoringTest#testAllConnectedToOneNode
#[test]
fn test_all_connected_to_one_node() {
    let count = 10;
    let mut graph = TestGraph::create();
    graph.create_node("Center".into());
    for i in 0..count {
        graph.create_node(format!("Node {i}"));
        graph.connect("Center".into(), None, format!("Node {i}"));
    }
    let mut coloring = GreedyGraphColoring::new();
    assert_eq!(coloring.color(&mut graph), 2);
    validate_coloring(&graph);
    assert_eq!(
        coloring.get_partition_super_node(&graph, "Center".into()),
        "Center"
    );
    for i in 0..count {
        assert_eq!(
            coloring.get_partition_super_node(&graph, format!("Node {i}")),
            "Node 0"
        );
    }
}
// port: GraphColoringTest#testTwoFullyConnected
#[test]
fn test_two_fully_connected() {
    let count = 100;
    let mut graph = TestGraph::create();
    for i in 0..count {
        graph.create_node(format!("Node Left {i}"));
        graph.create_node(format!("Node Right {i}"));
        for j in 0..count {
            graph.create_node(format!("Node Left {j}"));
            graph.create_node(format!("Node Right {j}"));
            if i != j {
                graph.connect(format!("Node Left {i}"), None, format!("Node Left {j}"));
                graph.connect(format!("Node Right {i}"), None, format!("Node Right {j}"));
            }
        }
    }
    assert_eq!(GreedyGraphColoring::new().color(&mut graph), count);
    validate_coloring(&graph);
    for i in 0..count {
        graph.connect(format!("Node Left {i}"), None, format!("Node Right {i}"));
    }
    assert_eq!(GreedyGraphColoring::new().color(&mut graph), count);
    validate_coloring(&graph);
}
// port: GraphColoringTest#testDeterministic
#[test]
fn test_deterministic() {
    let mut graph = TestGraph::create();
    for n in ["A", "B", "C", "D", "E"] {
        graph.create_node(n.into());
    }
    for (a, b) in [("A", "B"), ("B", "C"), ("C", "D"), ("D", "E"), ("E", "A")] {
        graph.connect(a.into(), Some("-->"), b.into());
    }
    let mut coloring = GreedyGraphColoring::with_tie_breaker(Some(Box::new(String::cmp)));
    assert_eq!(coloring.color(&mut graph), 3);
    validate_coloring(&graph);
    assert_eq!(coloring.get_partition_super_node(&graph, "A".into()), "A");
    assert_eq!(coloring.get_partition_super_node(&graph, "C".into()), "A");
    let mut coloring =
        GreedyGraphColoring::with_tie_breaker(Some(Box::new(|a: &String, b: &String| {
            a.replace('D', "@").cmp(&b.replace('D', "@"))
        })));
    assert_eq!(coloring.color(&mut graph), 3);
    validate_coloring(&graph);
    assert_eq!(coloring.get_partition_super_node(&graph, "A".into()), "A");
    assert_ne!(coloring.get_partition_super_node(&graph, "C".into()), "A");
}
// Rust-only (no Java counterpart): Annotation::hash_code on a Color trait object is
// Color#hashCode, which AmbiguateProperties reads through getAnnotation().hashCode().
#[test]
fn rust_only_color_hash_code_through_annotation() {
    let mut graph = TestGraph::create();
    graph.create_node("A".into());
    graph.create_node("B".into());
    graph.connect("A".into(), Some("--"), "B".into());
    assert_eq!(GreedyGraphColoring::new().color(&mut graph), 2);
    for node in graph.get_nodes() {
        assert_eq!(
            node.get_annotation(&graph).unwrap().hash_code(),
            node.get_annotation_as::<Color>(&graph).unwrap().value
        );
    }
}

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
//   test/com/google/javascript/jscomp/graph/GraphReachabilityTest.java.

use closure_jscomp::graph::graph_reachability::{GraphReachability, Reachable};
use closure_jscomp::graph::*;
// port: GraphReachabilityTest#testSimple
#[test]
fn test_simple() {
    let mut graph = LinkedDirectedGraph::<&str, &str>::create();
    graph.create_node("A");
    let mut reachability = GraphReachability::new();
    reachability.compute(&mut graph, "A");
    assert_reachable(&graph, "A");

    graph.create_node("B");
    let mut reachability = GraphReachability::new();
    reachability.compute(&mut graph, "A");
    assert_reachable(&graph, "A");
    assert_not_reachable(&graph, "B");

    graph.connect("A", "--->", "B");
    let mut reachability = GraphReachability::new();
    reachability.compute(&mut graph, "B");
    assert_not_reachable(&graph, "A");
    assert_reachable(&graph, "B");

    graph.connect("B", "--->", "A");
    let mut reachability = GraphReachability::new();
    reachability.compute(&mut graph, "B");
    assert_reachable(&graph, "A");
    assert_reachable(&graph, "B");

    graph.create_node("C");
    let mut reachability = GraphReachability::new();
    reachability.compute(&mut graph, "A");
    assert_reachable(&graph, "A");
    assert_reachable(&graph, "B");
    assert_not_reachable(&graph, "C");

    graph.create_node("D");
    graph.connect("C", "--->", "D");
    let mut reachability = GraphReachability::new();
    reachability.compute(&mut graph, "A");
    assert_reachable(&graph, "A");
    assert_reachable(&graph, "B");
    assert_not_reachable(&graph, "C");
    assert_not_reachable(&graph, "D");
    reachability.recompute(&mut graph, "C");
    assert_reachable(&graph, "C");
    assert_reachable(&graph, "D");
}
// port: GraphReachabilityTest#assertReachable
fn assert_reachable<'a>(graph: &LinkedDirectedGraph<&'a str, &str>, s: &'a str) {
    assert!(
        graph
            .get_node(&s)
            .unwrap()
            .get_annotation(graph)
            .is_some_and(|a| a.as_any().is::<Reachable>()),
        "{s} should be reachable"
    );
}
// port: GraphReachabilityTest#assertNotReachable
fn assert_not_reachable<'a>(graph: &LinkedDirectedGraph<&'a str, &str>, s: &'a str) {
    assert!(
        !graph
            .get_node(&s)
            .unwrap()
            .get_annotation(graph)
            .is_some_and(|a| a.as_any().is::<Reachable>()),
        "{s} should not be reachable"
    );
}

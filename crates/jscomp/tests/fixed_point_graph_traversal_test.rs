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
//   test/com/google/javascript/jscomp/FixedPointGraphTraversalTest.java.

use closure_jscomp::graph::fixed_point_graph_traversal::{EdgeCallback, FixedPointGraphTraversal};
use closure_jscomp::graph::*;
use std::{
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};
// Java Counter objects use arena identity and store their mutable values separately.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Counter(usize);
impl fmt::Display for Counter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Counter{}", self.0)
    }
}
struct CounterData {
    value: i32,
}
struct CounterIncrementer {
    max_change: i32,
    counters: Vec<CounterData>,
}
type G = LinkedDirectedGraph<Counter, &'static str>;
impl EdgeCallback<Counter, &'static str, G> for &mut CounterIncrementer {
    // port: FixedPointGraphTraversalTest.CounterIncrementer#traverseEdge
    fn traverse_edge(&mut self, _graph: &mut G, _source: Counter, _e: &str, dest: Counter) -> bool {
        self.counters[dest.0].value += 1;
        self.counters[dest.0].value <= self.max_change
    }
}
// port: FixedPointGraphTraversalTest#setUp
fn set_up() -> (G, CounterIncrementer) {
    let mut graph = G::create();
    for i in 0..5 {
        graph.create_node(Counter(i));
    }
    for (a, b) in [(0, 1), (0, 2), (0, 3), (1, 3), (2, 4), (3, 4), (4, 3)] {
        graph.connect(Counter(a), "->", Counter(b));
    }
    (
        graph,
        CounterIncrementer {
            max_change: 0,
            counters: (0..5).map(|_| CounterData { value: 0 }).collect(),
        },
    )
}
// port: FixedPointGraphTraversalTest#testGraph1
#[test]
fn test_graph1() {
    let (mut graph, mut callback) = set_up();
    callback.max_change = 0;
    FixedPointGraphTraversal::new_traversal(&mut callback)
        .compute_fixed_point_from(&mut graph, Counter(0));
    assert_eq!(callback.counters[0].value, 0);
    assert_eq!(callback.counters[1].value, 1);
    assert_eq!(callback.counters[2].value, 1);
    assert_eq!(callback.counters[3].value, 1);
    assert_eq!(callback.counters[4].value, 0);
}
// port: FixedPointGraphTraversalTest#testGraph2
#[test]
fn test_graph2() {
    let (mut graph, mut callback) = set_up();
    callback.max_change = 0;
    FixedPointGraphTraversal::new_traversal(&mut callback)
        .compute_fixed_point_from(&mut graph, Counter(3));
    assert_eq!(callback.counters[0].value, 0);
    assert_eq!(callback.counters[1].value, 0);
    assert_eq!(callback.counters[2].value, 0);
    assert_eq!(callback.counters[3].value, 0);
    assert_eq!(callback.counters[4].value, 1);
}
// port: FixedPointGraphTraversalTest#testGraph3
#[test]
fn test_graph3() {
    let (mut graph, mut callback) = set_up();
    callback.max_change = 1;
    FixedPointGraphTraversal::new_traversal(&mut callback)
        .compute_fixed_point_from(&mut graph, Counter(0));
    assert_eq!(callback.counters[0].value, 0);
    assert_eq!(callback.counters[1].value, 1);
    assert_eq!(callback.counters[2].value, 1);
    assert_eq!(callback.counters[3].value, 3);
    assert_eq!(callback.counters[4].value, 2);
}
// port: FixedPointGraphTraversalTest#testGraph4
#[test]
fn test_graph4() {
    let (mut graph, mut callback) = set_up();
    callback.max_change = 1;
    FixedPointGraphTraversal::new_traversal(&mut callback)
        .compute_fixed_point_from(&mut graph, Counter(3));
    assert_eq!(callback.counters[0].value, 0);
    assert_eq!(callback.counters[1].value, 0);
    assert_eq!(callback.counters[2].value, 0);
    assert_eq!(callback.counters[3].value, 1);
    assert_eq!(callback.counters[4].value, 2);
}
// port: FixedPointGraphTraversalTest#testGraph5
#[test]
fn test_graph5() {
    let (mut graph, mut callback) = set_up();
    callback.max_change = 5;
    FixedPointGraphTraversal::new_traversal(&mut callback)
        .compute_fixed_point_from(&mut graph, Counter(0));
    assert_eq!(callback.counters[0].value, 0);
    assert_eq!(callback.counters[1].value, 1);
    assert_eq!(callback.counters[2].value, 1);
    assert_eq!(callback.counters[3].value, 6);
    assert_eq!(callback.counters[4].value, 5);
}
// port: FixedPointGraphTraversalTest#testGraph6
#[test]
fn test_graph6() {
    let (mut graph, mut callback) = set_up();
    callback.max_change = 5;
    FixedPointGraphTraversal::new_traversal(&mut callback)
        .compute_fixed_point_from(&mut graph, Counter(1));
    assert_eq!(callback.counters[0].value, 0);
    assert_eq!(callback.counters[1].value, 0);
    assert_eq!(callback.counters[2].value, 0);
    assert_eq!(callback.counters[3].value, 6);
    assert_eq!(callback.counters[4].value, 5);
}
// port: FixedPointGraphTraversalTest#testGraph8
#[test]
#[allow(clippy::err_expect)] // Preserve the imported Java test's explicit error extraction.
fn test_graph8() {
    let (mut graph, mut callback) = set_up();
    callback.max_change = 2;
    FixedPointGraphTraversal::new_traversal(&mut callback)
        .compute_fixed_point_from(&mut graph, Counter(0));
    // port: FixedPointGraphTraversalTest.EdgeCallback#traverseEdge
    let mut traversal = FixedPointGraphTraversal::new_traversal(
        |_graph: &mut G, _source: Counter, _e: &str, _dest: Counter| true,
    );
    let error = catch_unwind(AssertUnwindSafe(|| {
        traversal.compute_fixed_point_from(&mut graph, Counter(0))
    }))
    .expect_err("Expecting Error: Fixed point computation not halting");
    let message = error
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| error.downcast_ref::<&str>().copied())
        .unwrap();
    assert_eq!(message, "Fixed point computation not halting");
}
// port: FixedPointGraphTraversalTest#testGraph9
#[test]
fn test_graph9() {
    let (mut graph, mut callback) = set_up();
    callback.max_change = 0;
    FixedPointGraphTraversal::new_traversal(&mut callback).compute_fixed_point(&mut graph);
    assert_eq!(callback.counters[0].value, 0);
    assert_eq!(callback.counters[1].value, 1);
    assert_eq!(callback.counters[2].value, 1);
    assert_eq!(callback.counters[3].value, 3);
    assert_eq!(callback.counters[4].value, 2);
}
// port: FixedPointGraphTraversalTest#testGraph10
#[test]
fn test_graph10() {
    let mut graph = G::create();
    let mut callback = CounterIncrementer {
        max_change: 5,
        counters: (0..2).map(|_| CounterData { value: 0 }).collect(),
    };
    graph.create_node(Counter(0));
    graph.create_node(Counter(1));
    graph.connect(Counter(0), "->", Counter(0));
    graph.connect(Counter(0), "->", Counter(1));
    FixedPointGraphTraversal::new_traversal(&mut callback).compute_fixed_point(&mut graph);
    assert_eq!(callback.counters[0].value, 6);
    assert_eq!(callback.counters[1].value, 6);
}
// port: FixedPointGraphTraversalTest#testReversedTraversal
#[test]
fn test_reversed_traversal() {
    let (mut graph, mut callback) = set_up();
    callback.max_change = 1;
    FixedPointGraphTraversal::new_reverse_traversal(&mut callback)
        .compute_fixed_point_from(&mut graph, Counter(4));
    assert_eq!(callback.counters[0].value, 3);
    assert_eq!(callback.counters[1].value, 1);
    assert_eq!(callback.counters[2].value, 2);
    assert_eq!(callback.counters[3].value, 2);
    assert_eq!(callback.counters[4].value, 1);
}

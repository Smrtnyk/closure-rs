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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 8ac03b6:
//   test/com/google/javascript/jscomp/graph/CheckPathsBetweenNodesTest.java.

use closure_jscomp::graph::check_paths_between_nodes::CheckPathsBetweenNodes;
use closure_jscomp::graph::*;
use std::cell::Cell;
type G = LinkedDirectedGraph<&'static str, &'static str>;
const FALSE: fn(&&str) -> bool = |_| false;
const ALL_EDGE: fn(&G, DiGraphEdge) -> bool = |_, _| true;
const NO_EDGE: fn(&G, DiGraphEdge) -> bool = |_, _| false;
struct PrefixPredicate {
    prefix: &'static str,
}
impl PrefixPredicate {
    // port: CheckPathsBetweenNodesTest.PrefixPredicate#PrefixPredicate
    fn new(prefix: &'static str) -> Self {
        Self { prefix }
    }
    // port: CheckPathsBetweenNodesTest.PrefixPredicate#apply
    fn apply(&self, input: &&str) -> bool {
        input.starts_with(self.prefix)
    }
}
struct CountingPredicate<P> {
    count: Cell<i32>,
    delegate: P,
}
impl<P: Fn(&&str) -> bool> CountingPredicate<P> {
    // port: CheckPathsBetweenNodesTest.CountingPredicate#CountingPredicate
    fn new(delegate: P) -> Self {
        Self {
            count: Cell::new(0),
            delegate,
        }
    }
    // port: CheckPathsBetweenNodesTest.CountingPredicate#apply
    fn apply(&self, input: &&str) -> bool {
        self.count.set(self.count.get() + 1);
        (self.delegate)(input)
    }
}
struct NodeCountingPredicate {
    count: Cell<i32>,
    target: &'static str,
    return_value: bool,
}
impl NodeCountingPredicate {
    // port: CheckPathsBetweenNodesTest.NodeCountingPredicate#NodeCountingPredicate
    fn new(target: &'static str, return_value: bool) -> Self {
        Self {
            count: Cell::new(0),
            target,
            return_value,
        }
    }
    // port: CheckPathsBetweenNodesTest.NodeCountingPredicate#apply
    fn apply(&self, input: &&str) -> bool {
        if *input == self.target {
            self.count.set(self.count.get() + 1);
            return self.return_value;
        }
        false
    }
}
// port: CheckPathsBetweenNodesTest#createTest
fn create_test<NP: Fn(&&str) -> bool, EP: Fn(&G, DiGraphEdge) -> bool>(
    graph: &G,
    entry: &'static str,
    exit: &'static str,
    node_predicate: NP,
    edge_predicate: EP,
) -> CheckPathsBetweenNodes<&'static str, &'static str, NP, EP> {
    CheckPathsBetweenNodes::new(
        graph.get_node(&entry).unwrap(),
        graph.get_node(&exit).unwrap(),
        node_predicate,
        edge_predicate,
    )
}
// port: CheckPathsBetweenNodesTest#createNonInclusiveTest
fn create_non_inclusive_test<NP: Fn(&&str) -> bool, EP: Fn(&G, DiGraphEdge) -> bool>(
    graph: &G,
    entry: &'static str,
    exit: &'static str,
    node_predicate: NP,
    edge_predicate: EP,
) -> CheckPathsBetweenNodes<&'static str, &'static str, NP, EP> {
    CheckPathsBetweenNodes::with_inclusive(
        graph.get_node(&entry).unwrap(),
        graph.get_node(&exit).unwrap(),
        node_predicate,
        edge_predicate,
        false,
    )
}
// port: CheckPathsBetweenNodesTest#assertGood
fn assert_good<NP: Fn(&&str) -> bool, EP: Fn(&G, DiGraphEdge) -> bool>(
    test: CheckPathsBetweenNodes<&'static str, &'static str, NP, EP>,
    graph: &mut G,
) {
    assert!(test.all_paths_satisfy_predicate(graph));
}
// port: CheckPathsBetweenNodesTest#assertBad
fn assert_bad<NP: Fn(&&str) -> bool, EP: Fn(&G, DiGraphEdge) -> bool>(
    test: CheckPathsBetweenNodes<&'static str, &'static str, NP, EP>,
    graph: &mut G,
) {
    assert!(!test.all_paths_satisfy_predicate(graph));
}
// port: CheckPathsBetweenNodesTest#edgeIs
fn edge_is(val: &'static str) -> impl Fn(&G, DiGraphEdge) -> bool {
    move |graph, edge| *edge.get_value(graph) == val
}
// port: CheckPathsBetweenNodesTest#testSimple
#[test]
fn test_simple() {
    let mut g = G::create();
    g.create_node("a");
    g.create_node("b");
    g.create_node("c");
    g.create_node("d");

    g.connect("a", "-", "b");
    g.connect("b", "-", "c");
    g.connect("c", "-", "d");
    g.connect("a", "x", "d");

    // Simple case: the sole path from a to d has a matching node.
    assert_good(
        create_test(&g, "a", "d", |v: &&str| *v == "b", edge_is("-")),
        &mut g,
    );
    // Test two edge cases where satisfying node is the first and last node on
    // the path.
    assert_good(
        create_test(&g, "a", "d", |v: &&str| *v == "a", edge_is("-")),
        &mut g,
    );
    assert_good(
        create_test(&g, "a", "d", |v: &&str| *v == "d", edge_is("-")),
        &mut g,
    );

    // Traverse no edges, so no paths.
    assert_good(create_test(&g, "a", "d", FALSE, NO_EDGE), &mut g);

    // No path with matching edges contains b.
    assert_bad(
        create_test(&g, "a", "d", |v: &&str| *v == "b", edge_is("x")),
        &mut g,
    );
}
// port: CheckPathsBetweenNodesTest#testSomeValidPaths
#[test]
fn test_some_valid_paths() {
    let mut g = G::create();
    g.create_node("a");
    g.create_node("b");
    g.create_node("c");
    g.create_node("d");
    g.create_node("e");

    g.connect("a", "1", "b");
    g.connect("b", "2", "c");
    g.connect("b", "3", "e");
    g.connect("e", "4", "d");
    g.connect("c", "5", "d");

    assert_bad(
        create_test(&g, "a", "d", |v: &&str| *v == "c", ALL_EDGE),
        &mut g,
    );
    assert_bad(
        create_test(&g, "a", "d", |v: &&str| *v == "z", ALL_EDGE),
        &mut g,
    );
}
// port: CheckPathsBetweenNodesTest#testManyValidPaths
#[test]
fn test_many_valid_paths() {
    let mut g = G::create();
    g.create_node("a");
    g.create_node("b");
    g.create_node("c1");
    g.create_node("c2");
    g.create_node("c3");
    g.create_node("d");

    g.connect("a", "-", "b");
    g.connect("b", "-", "c1");
    g.connect("b", "-", "c2");
    g.connect("c2", "-", "d");
    g.connect("c1", "-", "d");
    g.connect("a", "-", "c3");
    g.connect("c3", "-", "d");

    assert_good(
        create_test(
            &g,
            "a",
            "d",
            |v: &&str| PrefixPredicate::new("c").apply(v),
            ALL_EDGE,
        ),
        &mut g,
    );
}
// port: CheckPathsBetweenNodesTest#testCycles1
#[test]
fn test_cycles1() {
    let mut g = G::create();
    g.create_node("a");
    g.create_node("b");
    g.create_node("c");
    g.create_node("d");
    g.create_node("e");
    g.create_node("f");

    g.connect("a", "-", "b");
    g.connect("b", "-", "c");
    g.connect("c", "-", "d");
    g.connect("d", "-", "e");
    g.connect("e", "-", "f");
    g.connect("f", "-", "b");

    assert_good(
        create_test(&g, "a", "e", |v: &&str| *v == "b", ALL_EDGE),
        &mut g,
    );
    assert_good(
        create_test(&g, "a", "e", |v: &&str| *v == "c", ALL_EDGE),
        &mut g,
    );
    assert_good(
        create_test(&g, "a", "e", |v: &&str| *v == "d", ALL_EDGE),
        &mut g,
    );
    assert_good(
        create_test(&g, "a", "e", |v: &&str| *v == "e", ALL_EDGE),
        &mut g,
    );
    assert_bad(
        create_test(&g, "a", "e", |v: &&str| *v == "f", ALL_EDGE),
        &mut g,
    );
}
// port: CheckPathsBetweenNodesTest#testCycles2
#[test]
fn test_cycles2() {
    let mut g = G::create();
    g.create_node("a");
    g.create_node("b");
    g.create_node("c");
    g.create_node("d");

    g.connect("a", "-", "b");
    g.connect("b", "-", "c");
    g.connect("c", "-", "b");
    g.connect("b", "-", "d");

    assert_good(
        create_test(&g, "a", "d", |v: &&str| *v == "a", ALL_EDGE),
        &mut g,
    );
    assert_bad(
        create_test(&g, "a", "d", |v: &&str| *v == "z", ALL_EDGE),
        &mut g,
    );
}
// port: CheckPathsBetweenNodesTest#testCycles3
#[test]
fn test_cycles3() {
    let mut g = G::create();
    g.create_node("a");
    g.create_node("b");
    g.create_node("c");
    g.create_node("d");

    g.connect("a", "-", "b");
    g.connect("b", "-", "c");
    g.connect("c", "-", "b");
    g.connect("b", "-", "d");
    g.connect("c", "-", "d");

    assert_good(
        create_test(&g, "a", "d", |v: &&str| *v == "a", ALL_EDGE),
        &mut g,
    );
    assert_bad(
        create_test(&g, "a", "d", |v: &&str| *v == "z", ALL_EDGE),
        &mut g,
    );
}
// port: CheckPathsBetweenNodesTest#testSomePath1
#[test]
fn test_some_path1() {
    let mut g = G::create();
    g.create_node("a");
    g.create_node("b");
    g.create_node("c");
    g.create_node("d");

    g.connect("a", "-", "b");
    g.connect("a", "-", "c");
    g.connect("b", "-", "d");
    g.connect("c", "-", "d");

    assert!(
        create_test(&g, "a", "d", |v: &&str| *v == "b", ALL_EDGE)
            .some_paths_satisfy_predicate(&mut g)
    );
    assert!(
        create_test(&g, "a", "d", |v: &&str| *v == "c", ALL_EDGE)
            .some_paths_satisfy_predicate(&mut g)
    );
    assert!(
        create_test(&g, "a", "d", |v: &&str| *v == "a", ALL_EDGE)
            .some_paths_satisfy_predicate(&mut g)
    );
    assert!(
        create_test(&g, "a", "d", |v: &&str| *v == "d", ALL_EDGE)
            .some_paths_satisfy_predicate(&mut g)
    );
    assert!(
        !create_test(&g, "a", "d", |v: &&str| *v == "NONE", ALL_EDGE)
            .some_paths_satisfy_predicate(&mut g)
    );
}
// port: CheckPathsBetweenNodesTest#testSomePath2
#[test]
fn test_some_path2() {
    // No Paths between nodes, by definition, always false.
    let mut g = G::create();
    g.create_node("a");
    g.create_node("b");

    assert!(
        !create_test(&g, "a", "b", |v: &&str| *v == "b", ALL_EDGE)
            .some_paths_satisfy_predicate(&mut g)
    );
    assert!(
        !create_test(&g, "a", "b", |v: &&str| *v == "d", ALL_EDGE)
            .some_paths_satisfy_predicate(&mut g)
    );
    assert!(
        create_test(&g, "a", "b", |v: &&str| *v == "a", ALL_EDGE)
            .some_paths_satisfy_predicate(&mut g)
    );
}
// port: CheckPathsBetweenNodesTest#testSomePathRevisiting
#[test]
fn test_some_path_revisiting() {
    let mut g = G::create();
    g.create_node("1");
    g.create_node("2a");
    g.create_node("2b");
    g.create_node("3");
    g.create_node("4a");
    g.create_node("4b");
    g.create_node("5");
    g.connect("1", "-", "2a");
    g.connect("1", "-", "2b");
    g.connect("2a", "-", "3");
    g.connect("2b", "-", "3");
    g.connect("3", "-", "4a");
    g.connect("3", "-", "4b");
    g.connect("4a", "-", "5");
    g.connect("4b", "-", "5");

    let p = CountingPredicate::new(|v: &&str| *v == "4a");

    assert!(
        create_test(&g, "1", "5", |v: &&str| p.apply(v), ALL_EDGE)
            .some_paths_satisfy_predicate(&mut g)
    );

    // Make sure we are not doing more traversals than we have to.
    assert_eq!(p.count.get(), 4);
}
// port: CheckPathsBetweenNodesTest#testNonInclusive
#[test]
fn test_non_inclusive() {
    // No Paths between nodes, by definition, always false.
    let mut g = G::create();
    g.create_node("a");
    g.create_node("b");
    g.create_node("c");
    g.connect("a", "-", "b");
    g.connect("b", "-", "c");
    assert!(
        !create_non_inclusive_test(&g, "a", "b", |v: &&str| *v == "a", ALL_EDGE)
            .some_paths_satisfy_predicate(&mut g)
    );
    assert!(
        !create_non_inclusive_test(&g, "a", "b", |v: &&str| *v == "b", ALL_EDGE)
            .some_paths_satisfy_predicate(&mut g)
    );
    assert!(
        create_non_inclusive_test(&g, "a", "c", |v: &&str| *v == "b", ALL_EDGE)
            .some_paths_satisfy_predicate(&mut g)
    );
}
// port: CheckPathsBetweenNodesTest#testConfluentPaths_visitedEdgePreventsRedundantTraversal_allPaths
#[test]
fn test_confluent_paths_visited_edge_prevents_redundant_traversal_all_paths() {
    let mut g = G::create();
    g.create_node("a");
    g.create_node("b");
    g.create_node("c");
    g.create_node("d");
    g.create_node("e");

    g.connect("a", "-", "b");
    g.connect("a", "-", "c");
    g.connect("b", "-", "d");
    g.connect("c", "-", "d");
    g.connect("d", "-", "e");

    // Diamond DAG where paths a -> b -> d -> e and a -> c -> d -> e converge at 'd'.
    // Once edge d -> e is traversed and marked VISITED_EDGE on the first path,
    // the second path converging at 'd' must not re-traverse d -> e.
    let count_e = NodeCountingPredicate::new("e", true);
    assert!(
        create_test(&g, "a", "e", |v: &&str| count_e.apply(v), ALL_EDGE)
            .all_paths_satisfy_predicate(&mut g)
    );
    assert_eq!(count_e.count.get(), 1);
}
// port: CheckPathsBetweenNodesTest#testConfluentPaths_visitedEdgePreventsRedundantTraversal_somePaths
#[test]
fn test_confluent_paths_visited_edge_prevents_redundant_traversal_some_paths() {
    let mut g = G::create();
    g.create_node("a");
    g.create_node("b");
    g.create_node("c");
    g.create_node("d");
    g.create_node("e");

    g.connect("a", "-", "b");
    g.connect("a", "-", "c");
    g.connect("b", "-", "d");
    g.connect("c", "-", "d");
    g.connect("d", "-", "e");

    // Diamond DAG where paths a -> b -> d -> e and a -> c -> d -> e converge at 'd'.
    // In somePathsSatisfyPredicate, when the first path does not satisfy the predicate,
    // edge d -> e is marked VISITED_EDGE. The second path converging at 'd' must not
    // re-traverse d -> e.
    let count_e = NodeCountingPredicate::new("e", false);
    assert!(
        !create_test(&g, "a", "e", |v: &&str| count_e.apply(v), ALL_EDGE)
            .some_paths_satisfy_predicate(&mut g)
    );
    assert_eq!(count_e.count.get(), 1);
}
// port: CheckPathsBetweenNodesTest#testConfluentPaths_withCycles_allPaths
#[test]
fn test_confluent_paths_with_cycles_all_paths() {
    let mut g = G::create();
    g.create_node("a");
    g.create_node("b");
    g.create_node("c");
    g.create_node("d");
    g.create_node("e");
    g.create_node("f");

    g.connect("a", "-", "b");
    g.connect("a", "-", "c");
    g.connect("b", "-", "d");
    g.connect("c", "-", "d");
    g.connect("d", "-", "e");
    g.connect("e", "-", "d"); // cycle on confluent subgraph
    g.connect("d", "-", "f");

    // All paths from 'a' to 'f' converge at 'd'. The cycle e -> d is a back edge,
    // and d -> f is marked VISITED_EDGE once traversed. 'f' must be visited once.
    let count_f = NodeCountingPredicate::new("f", true);
    assert!(
        create_test(&g, "a", "f", |v: &&str| count_f.apply(v), ALL_EDGE)
            .all_paths_satisfy_predicate(&mut g)
    );
    assert_eq!(count_f.count.get(), 1);
}
// port: CheckPathsBetweenNodesTest#testConfluentPaths_withCycles_somePaths
#[test]
fn test_confluent_paths_with_cycles_some_paths() {
    let mut g = G::create();
    g.create_node("a");
    g.create_node("b");
    g.create_node("c");
    g.create_node("d");
    g.create_node("e");
    g.create_node("f");

    g.connect("a", "-", "b");
    g.connect("a", "-", "c");
    g.connect("b", "-", "d");
    g.connect("c", "-", "d");
    g.connect("d", "-", "e");
    g.connect("e", "-", "d"); // cycle on confluent subgraph
    g.connect("d", "-", "f");

    let count_f = NodeCountingPredicate::new("f", false);
    assert!(
        !create_test(&g, "a", "f", |v: &&str| count_f.apply(v), ALL_EDGE)
            .some_paths_satisfy_predicate(&mut g)
    );
    assert_eq!(count_f.count.get(), 1);
}
// port: CheckPathsBetweenNodesTest#testCycles_backEdgeNotTraversed_allPaths
#[test]
fn test_cycles_back_edge_not_traversed_all_paths() {
    let mut g = G::create();
    g.create_node("a");
    g.create_node("b");
    g.create_node("c");
    g.create_node("d");

    g.connect("a", "-", "b");
    g.connect("b", "-", "c");
    g.connect("c", "-", "b"); // back edge to b
    g.connect("b", "-", "d");

    // NodeCountingPredicate on 'b': without back edges, 'b' is visited once.
    // If the back edge c -> b is traversed, 'b' is revisited.
    let count_b = NodeCountingPredicate::new("b", false);
    assert!(
        !create_test(&g, "a", "d", |v: &&str| count_b.apply(v), ALL_EDGE)
            .all_paths_satisfy_predicate(&mut g)
    );
    assert_eq!(count_b.count.get(), 1);
}
// port: CheckPathsBetweenNodesTest#testCycles_backEdgeNotTraversed_somePaths
#[test]
fn test_cycles_back_edge_not_traversed_some_paths() {
    let mut g = G::create();
    g.create_node("a");
    g.create_node("b");
    g.create_node("c");
    g.create_node("d");

    g.connect("a", "-", "b");
    g.connect("b", "-", "c");
    g.connect("c", "-", "b"); // back edge to b
    g.connect("b", "-", "d");

    // NodeCountingPredicate on 'b': without back edges, 'b' is visited once.
    // If the back edge c -> b is traversed, 'b' is revisited.
    let count_b = NodeCountingPredicate::new("b", false);
    assert!(
        !create_test(&g, "a", "d", |v: &&str| count_b.apply(v), ALL_EDGE)
            .some_paths_satisfy_predicate(&mut g)
    );
    assert_eq!(count_b.count.get(), 1);
}
// port: CheckPathsBetweenNodesTest#testCycles_backEdgeToStartNotTraversed
#[test]
fn test_cycles_back_edge_to_start_not_traversed() {
    let mut g = G::create();
    g.create_node("a");
    g.create_node("b");
    g.create_node("c");
    g.create_node("d");

    g.connect("a", "-", "b");
    g.connect("b", "-", "c");
    g.connect("c", "-", "a"); // back edge to start 'a'
    g.connect("b", "-", "d");

    // NodeCountingPredicate on 'a': without back edges, start 'a' is visited once.
    // If the back edge c -> a is traversed, 'a' is revisited.
    let count_a = NodeCountingPredicate::new("a", false);
    assert!(
        !create_test(&g, "a", "d", |v: &&str| count_a.apply(v), ALL_EDGE)
            .all_paths_satisfy_predicate(&mut g)
    );
    assert_eq!(count_a.count.get(), 1);

    let count_some_a = NodeCountingPredicate::new("a", false);
    assert!(
        !create_test(&g, "a", "d", |v: &&str| count_some_a.apply(v), ALL_EDGE)
            .some_paths_satisfy_predicate(&mut g)
    );
    assert_eq!(count_some_a.count.get(), 1);
}

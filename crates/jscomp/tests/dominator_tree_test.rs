/*
 * Copyright 2026 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/graph/DominatorTreeTest.java.

use closure_jscomp::graph::dominator_tree::DominatorTree;
use closure_jscomp::graph::*;
use closure_rhino::fast_hash::IndexMap;
// port: DominatorTreeTest#testSimpleChain
#[test]
fn test_simple_chain() {
    // A -> B -> C
    let mut graph = LinkedDirectedGraph::<&str, &str>::create();
    graph.create_node("A");
    graph.create_node("B");
    graph.create_node("C");
    graph.connect("A", "->", "B");
    graph.connect("B", "->", "C");

    let tree = DominatorTree::compute(&graph, "A");

    assert_eq!(tree.get_immediate_dominator(&"A"), None);
    assert_eq!(tree.get_immediate_dominator(&"B"), Some(&"A"));
    assert_eq!(tree.get_immediate_dominator(&"C"), Some(&"B"));

    assert_eq!(tree.get_dominated_subtree_size(&"A"), 3);
    assert_eq!(tree.get_dominated_subtree_size(&"B"), 2);
    assert_eq!(tree.get_dominated_subtree_size(&"C"), 1);

    assert_eq!(
        tree.get_all_subtree_sizes(),
        &IndexMap::<_, _>::from_iter([("A", 3), ("B", 2), ("C", 1)])
    );
}
// port: DominatorTreeTest#testDiamond
#[test]
fn test_diamond() {
    // A -> {B, C} -> D
    let mut graph = LinkedDirectedGraph::<&str, &str>::create();
    graph.create_node("A");
    graph.create_node("B");
    graph.create_node("C");
    graph.create_node("D");
    graph.connect("A", "->", "B");
    graph.connect("A", "->", "C");
    graph.connect("B", "->", "D");
    graph.connect("C", "->", "D");

    let tree = DominatorTree::compute(&graph, "A");

    assert_eq!(tree.get_immediate_dominator(&"B"), Some(&"A"));
    assert_eq!(tree.get_immediate_dominator(&"C"), Some(&"A"));
    assert_eq!(tree.get_immediate_dominator(&"D"), Some(&"A"));

    assert_eq!(tree.get_dominated_subtree_size(&"A"), 4);
    assert_eq!(tree.get_dominated_subtree_size(&"B"), 1);
    assert_eq!(tree.get_dominated_subtree_size(&"C"), 1);
    assert_eq!(tree.get_dominated_subtree_size(&"D"), 1);
}
// port: DominatorTreeTest#testCycle
#[test]
fn test_cycle() {
    // A -> B -> C -> B
    // Arbitrarily chooses 'B' as the dominator of both 'B' and 'C'.
    // NOTE: in practice, we added DominatorTree for analyzing dependencies in JSCompiler. Cycles
    // between goog.require/goog.requireTyped files can only occur within a single library, where
    // it is valid to have goog.requireType cycles.
    // So this result is not entirely precise, but is still useful for practical purposes.
    let mut graph = LinkedDirectedGraph::<&str, &str>::create();
    graph.create_node("A");
    graph.create_node("B");
    graph.create_node("C");
    graph.connect("A", "->", "B");
    graph.connect("B", "->", "C");
    graph.connect("C", "->", "B");

    let tree = DominatorTree::compute(&graph, "A");

    assert_eq!(tree.get_immediate_dominator(&"B"), Some(&"A"));
    assert_eq!(tree.get_immediate_dominator(&"C"), Some(&"B"));

    assert_eq!(tree.get_dominated_subtree_size(&"A"), 3);
    assert_eq!(tree.get_dominated_subtree_size(&"B"), 2);
    assert_eq!(tree.get_dominated_subtree_size(&"C"), 1);
}
// port: DominatorTreeTest#testBottleneck
#[test]
fn test_bottleneck() {
    // Entry -> A -> B -> {C, D, E}
    let mut graph = LinkedDirectedGraph::<&str, &str>::create();
    graph.create_node("Entry");
    graph.create_node("A");
    graph.create_node("B");
    graph.create_node("C");
    graph.create_node("D");
    graph.create_node("E");
    graph.connect("Entry", "->", "A");
    graph.connect("A", "->", "B");
    graph.connect("B", "->", "C");
    graph.connect("B", "->", "D");
    graph.connect("B", "->", "E");

    let tree = DominatorTree::compute(&graph, "Entry");

    assert_eq!(tree.get_immediate_dominator(&"B"), Some(&"A"));
    assert_eq!(tree.get_dominated_subtree_size(&"B"), 4); // B, C, D, E
    assert_eq!(tree.get_dominated_subtree_size(&"A"), 5); // A, B, C, D, E
}
// port: DominatorTreeTest#testSharedBottleneck
#[test]
fn test_shared_bottleneck() {
    // Entry -> {R1, R2}
    // R1 -> B
    // R2 -> B
    // B -> {C, D}
    let mut graph = LinkedDirectedGraph::<&str, &str>::create();
    graph.create_node("Entry");
    graph.create_node("R1");
    graph.create_node("R2");
    graph.create_node("B");
    graph.create_node("C");
    graph.create_node("D");
    graph.connect("Entry", "->", "R1");
    graph.connect("Entry", "->", "R2");
    graph.connect("R1", "->", "B");
    graph.connect("R2", "->", "B");
    graph.connect("B", "->", "C");
    graph.connect("B", "->", "D");

    let tree = DominatorTree::compute(&graph, "Entry");

    assert_eq!(tree.get_immediate_dominator(&"B"), Some(&"Entry"));
    // Neither R1 nor R2 dominate "B", as there is always a path from Entry to B going through the
    // other.
    assert_eq!(tree.get_dominated_subtree_size(&"R1"), 1); // R1
    assert_eq!(tree.get_dominated_subtree_size(&"R2"), 1); // R2
    assert_eq!(tree.get_dominated_subtree_size(&"B"), 3); // B, C, D
}
// port: DominatorTreeTest#testNodeNotReachableFromEntryPoint_doesNotCountAsDominator
#[test]
fn test_node_not_reachable_from_entry_point_does_not_count_as_dominator() {
    // A -> B
    // OTHER -> B
    let mut graph = LinkedDirectedGraph::<&str, &str>::create();
    graph.create_node("A");
    graph.create_node("B");
    graph.create_node("OTHER");
    graph.connect("A", "->", "B");
    graph.connect("OTHER", "->", "B");

    let tree = DominatorTree::compute(&graph, "A");

    // The dominator tree does not consider "OTHER" to dominate "B", as it is only looking
    // at paths *from the specified entry point* "A" to "B".
    assert_eq!(tree.get_immediate_dominator(&"B"), Some(&"A"));
    assert_eq!(tree.get_immediate_dominator(&"OTHER"), None);
    assert_eq!(tree.get_dominated_subtree_size(&"OTHER"), 0);

    assert_eq!(
        tree.get_all_subtree_sizes(),
        &IndexMap::<_, _>::from_iter([("A", 2), ("B", 1)])
    );
}
// port: DominatorTreeTest#testGetImmediateDominator_nonExistentElement_returnsNull
#[test]
fn test_get_immediate_dominator_non_existent_element_returns_null() {
    let mut graph = LinkedDirectedGraph::<&str, &str>::create();
    graph.create_node("A");

    let tree = DominatorTree::compute(&graph, "A");

    assert_eq!(tree.get_immediate_dominator(&"FOO"), None);
}
// port: DominatorTreeTest#testGetSubtreeSize_nonExistentElement_defaultsToZero
#[test]
fn test_get_subtree_size_non_existent_element_defaults_to_zero() {
    let mut graph = LinkedDirectedGraph::<&str, &str>::create();
    graph.create_node("A");

    let tree = DominatorTree::compute(&graph, "A");

    assert_eq!(tree.get_dominated_subtree_size(&"FOO"), 0);
}

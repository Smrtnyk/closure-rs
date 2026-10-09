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
//   test/com/google/javascript/jscomp/graph/LowestCommonAncestorFinderTest.java.

#![allow(clippy::upper_case_acronyms)] // Preserve the Java test enum spelling.
use closure_jscomp::graph::lowest_common_ancestor_finder::LowestCommonAncestorFinder;
use closure_jscomp::graph::*;
use closure_rhino::fx_hash::IndexSet;
use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
};
struct CaseBuilder {
    graph: LinkedDirectedGraph<i32, ()>,
    roots: Option<IndexSet<i32>>,
    expected: Option<IndexSet<i32>>,
    actual: Option<IndexSet<i32>>,
}
impl CaseBuilder {
    fn new() -> Self {
        Self {
            graph: LinkedDirectedGraph::create(),
            roots: None,
            expected: None,
            actual: None,
        }
    }
    // port: LowestCommonAncestorFinderTest.CaseBuilder#edge
    fn edge(mut self, src: i32, dest: i32) -> Self {
        self.graph.create_node(src);
        self.graph.create_node(dest);
        self.graph.connect(src, (), dest);
        assert_eq!(self.graph.get_edges_between(&src, &dest).len(), 1);
        self
    }
    // port: LowestCommonAncestorFinderTest.CaseBuilder#copyGraph
    fn copy_graph(mut self, src: LinkedDirectedGraph<i32, ()>) -> Self {
        for edge in src.get_edges() {
            self = self.edge(
                *edge.get_source(&src).get_value(&src),
                *edge.get_destination(&src).get_value(&src),
            );
        }
        self
    }
    // port: LowestCommonAncestorFinderTest.CaseBuilder#roots
    fn roots(mut self, roots: &[i32]) -> Self {
        assert!(self.roots.is_none());
        self.roots = Some(roots.iter().copied().collect());
        self
    }
    // port: LowestCommonAncestorFinderTest.CaseBuilder#expect
    fn expect(mut self, expected: &[i32]) -> Self {
        assert!(self.expected.is_none());
        self.expected = Some(expected.iter().copied().collect());
        self
    }
    // port: LowestCommonAncestorFinderTest.CaseBuilder#testAndRenderGraph
    fn test_and_render_graph(mut self) {
        assert!(self.actual.is_none());
        let mut finder = LowestCommonAncestorFinder::new();
        self.actual = Some(finder.find_all(&self.graph, self.roots.as_ref().unwrap()));
        let message = if self.render_graph() {
            "Look for an SVG of the test graph in the test artifacts"
        } else {
            ""
        };
        assert_eq!(self.actual, self.expected, "{message}");
    }
    // port: LowestCommonAncestorFinderTest.CaseBuilder#renderGraph
    fn render_graph(&mut self) -> bool {
        if self.graph.get_node_count() > 100 {
            return false;
        }
        for node in self.graph.get_nodes() {
            let mut purpose = NodePurpose::NONE;
            let value = node.get_value(&self.graph);
            if self.roots.as_ref().unwrap().contains(value) {
                purpose = purpose.mix(NodePurpose::ROOT);
            }
            if self.expected.as_ref().unwrap().contains(value) {
                purpose = purpose.mix(NodePurpose::EXPECTED);
            }
            if self.actual.as_ref().unwrap().contains(value) {
                purpose = purpose.mix(NodePurpose::ACTUAL);
            }
            node.set_annotation(&mut self.graph, Some(Box::new(purpose)));
        }
        true
    }
}
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug)]
#[repr(usize)]
enum NodePurpose {
    NONE,
    ACTUAL,
    EXPECTED,
    ACTUAL_EXPECTED,
    ROOT,
    ACTUAL_ROOT,
    EXPECTED_ROOT,
    ACTUAL_EXPECTED_ROOT,
}
impl NodePurpose {
    // port: LowestCommonAncestorFinderTest.NodePurpose#mix
    fn mix(self, x: Self) -> Self {
        [
            Self::NONE,
            Self::ACTUAL,
            Self::EXPECTED,
            Self::ACTUAL_EXPECTED,
            Self::ROOT,
            Self::ACTUAL_ROOT,
            Self::EXPECTED_ROOT,
            Self::ACTUAL_EXPECTED_ROOT,
        ][self as usize | x as usize]
    }
}
impl Annotation for NodePurpose {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    // port: LowestCommonAncestorFinderTest.NodePurpose#toString
    fn to_string(&self) -> String {
        format!("{self:?}")
    }
}
// port: LowestCommonAncestorFinderTest#makeDigitLattice
fn make_digit_lattice() -> LinkedDirectedGraph<i32, ()> {
    let mut result = LinkedDirectedGraph::create_without_annotations();
    for i in 0..1000 {
        let i_str = i.to_string();
        let window_size = i_str.len() - 1;
        result.create_node(i);
        if window_size == 0 {
            continue;
        }
        for offset in 0..=i_str.len() - window_size {
            let desc_str = &i_str[offset..offset + window_size];
            result.connect_if_not_connected_in_direction(i, (), desc_str.parse().unwrap());
        }
    }
    result
}
// port: LowestCommonAncestorFinderTest#findAll_onTree_withCommonParent
#[test]
fn find_all_on_tree_with_common_parent() {
    CaseBuilder::new() //
        .edge(3, 1)
        .edge(3, 2)
        .roots(&[1, 2])
        .expect(&[3])
        .test_and_render_graph();
}
// port: LowestCommonAncestorFinderTest#findAll_onTree_withCommonGrandParent
#[test]
fn find_all_on_tree_with_common_grand_parent() {
    CaseBuilder::new() //
        .edge(5, 4)
        .edge(5, 3)
        .edge(4, 2)
        .edge(3, 1)
        .roots(&[1, 2])
        .expect(&[5])
        .test_and_render_graph();
}
// port: LowestCommonAncestorFinderTest#findAll_onTree_whenLcaHasParent
#[test]
fn find_all_on_tree_when_lca_has_parent() {
    CaseBuilder::new() //
        .edge(4, 3)
        .edge(3, 1)
        .edge(3, 2)
        .roots(&[1, 2])
        .expect(&[3])
        .test_and_render_graph();
}
// port: LowestCommonAncestorFinderTest#findAll_onDag_withNestedVs_findsSingleLca
#[test]
fn find_all_on_dag_with_nested_vs_finds_single_lca() {
    CaseBuilder::new() //
        .edge(4, 3)
        .edge(4, 2)
        .edge(4, 1)
        .edge(3, 2)
        .edge(3, 1)
        .roots(&[1, 2])
        .expect(&[3])
        .test_and_render_graph();
}
// port: LowestCommonAncestorFinderTest#findAll_onDag_findsMultipleLcas
#[test]
fn find_all_on_dag_finds_multiple_lcas() {
    CaseBuilder::new() //
        .edge(4, 2)
        .edge(4, 1)
        .edge(3, 2)
        .edge(3, 1)
        .roots(&[1, 2])
        .expect(&[3, 4])
        .test_and_render_graph();
}
// port: LowestCommonAncestorFinderTest#findAll_onDag_withCommonParent_findsMultipleLcas
#[test]
fn find_all_on_dag_with_common_parent_finds_multiple_lcas() {
    CaseBuilder::new() //
        .edge(5, 4)
        .edge(5, 3)
        .edge(4, 2)
        .edge(4, 1)
        .edge(3, 2)
        .edge(3, 1)
        .roots(&[1, 2])
        .expect(&[3, 4])
        .test_and_render_graph();
}
// port: LowestCommonAncestorFinderTest#findAll_onDag_withCommonParent_findsMultipleLcas_atDifferentDistances
#[test]
fn find_all_on_dag_with_common_parent_finds_multiple_lcas_at_different_distances() {
    CaseBuilder::new()
        .edge(10, 3)
        .edge(10, 9)
        .edge(9, 8)
        .edge(9, 7)
        .edge(8, 6)
        .edge(7, 5)
        .edge(6, 1)
        .edge(5, 2)
        .edge(3, 2)
        .edge(3, 1)
        .roots(&[1, 2])
        .expect(&[3, 9])
        .test_and_render_graph();
}
// port: LowestCommonAncestorFinderTest#findAll_onDag_sidePathsIntoNonLowest
#[test]
fn find_all_on_dag_side_paths_into_non_lowest() {
    CaseBuilder::new()
        .edge(20, 18)
        .edge(20, 19)
        .edge(19, 17)
        .edge(19, 12)
        .edge(18, 11)
        .edge(17, 15)
        .edge(16, 15)
        .edge(15, 14)
        .edge(15, 2)
        .edge(14, 1)
        .edge(12, 9)
        .edge(11, 10)
        .edge(10, 1)
        .edge(9, 2)
        .roots(&[1, 2])
        .expect(&[15])
        .test_and_render_graph();
}
// port: LowestCommonAncestorFinderTest#findAll_onCyclic_whenLcaInsideCycle_findsSomeMember_deterministically
#[test]
fn find_all_on_cyclic_when_lca_inside_cycle_finds_some_member_deterministically() {
    CaseBuilder::new()
        .edge(1, 2)
        .edge(2, 3)
        .edge(3, 4)
        .edge(4, 1)
        .edge(1, 5)
        .edge(2, 6)
        .roots(&[5, 6])
        .expect(&[1])
        .test_and_render_graph();
}
// port: LowestCommonAncestorFinderTest#findAll_onCyclic_whenLcaInsideCycle_appearsUnique_mayFindOtherMember
#[test]
fn find_all_on_cyclic_when_lca_inside_cycle_appears_unique_may_find_other_member() {
    CaseBuilder::new()
        .edge(1, 2)
        .edge(2, 3)
        .edge(3, 4)
        .edge(4, 1)
        .edge(1, 5)
        // Notice how 3 is a parent of both 5 and 6, yet isn't selected.
        .edge(3, 5)
        .edge(3, 6)
        .roots(&[5, 6])
        .expect(&[1])
        .test_and_render_graph();
}
// port: LowestCommonAncestorFinderTest#findAll_onCyclic_whenLcaBelowCycle_ignoresCycle
#[test]
fn find_all_on_cyclic_when_lca_below_cycle_ignores_cycle() {
    CaseBuilder::new()
        .edge(1, 2)
        .edge(2, 3)
        .edge(3, 1)
        .edge(3, 4)
        .edge(4, 5)
        .edge(4, 6)
        .roots(&[5, 6])
        .expect(&[4])
        .test_and_render_graph();
}
// port: LowestCommonAncestorFinderTest#findAll_onDigitLattice_withUnrelatedRoots_findsMultipleLcas
#[test]
fn find_all_on_digit_lattice_with_unrelated_roots_finds_multiple_lcas() {
    CaseBuilder::new() //
        .copy_graph(make_digit_lattice())
        .roots(&[1, 3, 5])
        .expect(&[135, 153, 315, 351, 531, 513])
        .test_and_render_graph();
}
// port: LowestCommonAncestorFinderTest#findAll_onDigitLattice_withRelatedRoots_findsMultipleLcas
#[test]
fn find_all_on_digit_lattice_with_related_roots_finds_multiple_lcas() {
    CaseBuilder::new() //
        .copy_graph(make_digit_lattice())
        .roots(&[1, 13, 5])
        .expect(&[135, 513])
        .test_and_render_graph();
}
// port: LowestCommonAncestorFinderTest#findAll_onDigitLattice_withOverlappingRoots_findsSingleLca
#[test]
fn find_all_on_digit_lattice_with_overlapping_roots_finds_single_lca() {
    CaseBuilder::new() //
        .copy_graph(make_digit_lattice())
        .roots(&[7, 4, 8, 48, 74])
        .expect(&[748])
        .test_and_render_graph();
}
// port: LowestCommonAncestorFinderTest#findAll_onDigitLattice_noLca
#[test]
fn find_all_on_digit_lattice_no_lca() {
    CaseBuilder::new()
        .copy_graph(make_digit_lattice()) //
        .roots(&[2, 4, 7, 8])
        .expect(&[])
        .test_and_render_graph();
}
// port: LowestCommonAncestorFinderTest#findAll_onDisjoint_noSolution
#[test]
fn find_all_on_disjoint_no_solution() {
    CaseBuilder::new() //
        .edge(4, 2)
        .edge(3, 1)
        .roots(&[1, 2])
        .expect(&[])
        .test_and_render_graph();
}
// port: LowestCommonAncestorFinderTest#findAll_rejectsTooManyRoots
#[test]
fn find_all_rejects_too_many_roots() {
    let mut builder = CaseBuilder::new().expect(&[40]);
    let mut roots = [0; 32];
    for (i, root) in roots.iter_mut().enumerate() {
        *root = i as i32;
        builder = builder.edge(40, *root);
    }
    builder = builder.roots(&roots);
    assert!(catch_unwind(AssertUnwindSafe(|| builder.test_and_render_graph())).is_err());
}

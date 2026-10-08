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
//   src/com/google/javascript/jscomp/graph/FixedPointGraphTraversal.java.

use super::{
    di_graph::{DiGraph, DiGraphNode},
    graph::GraphEdge,
    graph_node::GraphNode,
};
use indexmap::IndexSet;
use std::{collections::VecDeque, marker::PhantomData};

/// Java's `LinkedHashSet` work set. Elements keep their first insertion position
/// and the traversal only ever removes the head, so a FIFO of distinct entries
/// plus a membership set iterates identically, with O(1) head removal.
struct WorkSet {
    queue: VecDeque<DiGraphNode>,
    members: IndexSet<DiGraphNode>,
}
impl WorkSet {
    // port: LinkedHashSet#LinkedHashSet
    fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            members: IndexSet::new(),
        }
    }
    // port: LinkedHashSet#add
    fn add(&mut self, node: DiGraphNode) {
        if self.members.insert(node) {
            self.queue.push_back(node);
        }
    }
    // port: LinkedHashSet#isEmpty
    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
    // port: LinkedHashSet#getFirst
    fn get_first(&self) -> DiGraphNode {
        self.queue[0]
    }
    // port: LinkedHashSet#remove
    fn remove(&mut self, node: DiGraphNode) {
        if self.members.swap_remove(&node) {
            if self.queue.front() == Some(&node) {
                self.queue.pop_front();
            } else if let Some(i) = self.queue.iter().position(|n| *n == node) {
                self.queue.remove(i);
            }
        }
    }
}

pub trait EdgeCallback<N, E, G> {
    // port: FixedPointGraphTraversal.EdgeCallback#traverseEdge
    fn traverse_edge(&mut self, graph: &mut G, source: N, e: E, destination: N) -> bool;
}
impl<N, E, G, F: FnMut(&mut G, N, E, N) -> bool> EdgeCallback<N, E, G> for F {
    // port: FixedPointGraphTraversal.EdgeCallback#traverseEdge
    fn traverse_edge(&mut self, graph: &mut G, source: N, e: E, destination: N) -> bool {
        self(graph, source, e, destination)
    }
}
#[derive(Clone, Copy)]
enum TraversalDirection {
    INWARDS,
    OUTWARDS,
}
pub struct FixedPointGraphTraversal<N, E, C> {
    callback: C,
    traversal_direction: TraversalDirection,
    marker: PhantomData<(N, E)>,
}
impl<N: Clone, E: Clone, C> FixedPointGraphTraversal<N, E, C> {
    pub const NON_HALTING_ERROR_MSG: &str = "Fixed point computation not halting";
    pub const MAX_NODE_COUNT_FOR_ITERATION_LIMIT: usize = 3914;
    // port: FixedPointGraphTraversal#FixedPointGraphTraversal
    fn new(callback: C, traversal_direction: TraversalDirection) -> Self {
        Self {
            callback,
            traversal_direction,
            marker: PhantomData,
        }
    }
    // port: FixedPointGraphTraversal#newTraversal
    pub fn new_traversal(callback: C) -> Self {
        Self::new(callback, TraversalDirection::OUTWARDS)
    }
    // port: FixedPointGraphTraversal#newReverseTraversal
    pub fn new_reverse_traversal(callback: C) -> Self {
        Self::new(callback, TraversalDirection::INWARDS)
    }
    // port: FixedPointGraphTraversal#computeFixedPoint(DiGraph)
    pub fn compute_fixed_point<G: DiGraph<N, E>>(&mut self, graph: &mut G)
    where
        C: EdgeCallback<N, E, G>,
    {
        let nodes = graph
            .get_nodes()
            .into_iter()
            .map(|n| n.get_value(graph).clone())
            .collect::<Vec<_>>();
        self.compute_fixed_point_from_set(graph, nodes);
    }
    // port: FixedPointGraphTraversal#computeFixedPoint(DiGraph,N)
    pub fn compute_fixed_point_from<G: DiGraph<N, E>>(&mut self, graph: &mut G, entry: N)
    where
        C: EdgeCallback<N, E, G>,
    {
        self.compute_fixed_point_from_set(graph, [entry]);
    }
    // port: FixedPointGraphTraversal#computeFixedPoint(DiGraph,Set)
    pub fn compute_fixed_point_from_set<G: DiGraph<N, E>>(
        &mut self,
        graph: &mut G,
        entry_set: impl IntoIterator<Item = N>,
    ) where
        C: EdgeCallback<N, E, G>,
    {
        let mut cycle_count = 0_u64;
        let node_count = graph
            .get_node_count()
            .min(Self::MAX_NODE_COUNT_FOR_ITERATION_LIMIT) as u64;
        let max_iterations = (node_count * node_count * node_count).max(100);
        let mut work_set = WorkSet::new();
        for n in entry_set {
            work_set.add(graph.get_node(&n).expect(""));
        }
        while !work_set.is_empty() && cycle_count < max_iterations {
            let node = work_set.get_first();
            self.visit_node(graph, node, &mut work_set);
            cycle_count += 1;
        }
        closure_rhino::check_state!(cycle_count != max_iterations, Self::NON_HALTING_ERROR_MSG);
    }
    // port: FixedPointGraphTraversal#visitNode
    fn visit_node<G: DiGraph<N, E>>(
        &mut self,
        graph: &mut G,
        node: DiGraphNode,
        work_set: &mut WorkSet,
    ) where
        C: EdgeCallback<N, E, G>,
    {
        work_set.remove(node);
        let source_value = node.get_value(graph).clone();
        let edges = match self.traversal_direction {
            TraversalDirection::OUTWARDS => node.get_out_edges(graph),
            TraversalDirection::INWARDS => node.get_in_edges(graph),
        }
        .to_vec();
        for edge in edges {
            let dest = match self.traversal_direction {
                TraversalDirection::OUTWARDS => edge.get_destination(graph),
                TraversalDirection::INWARDS => edge.get_source(graph),
            };
            let dest_value = dest.get_value(graph).clone();
            let e = edge.get_value(graph).clone();
            if self
                .callback
                .traverse_edge(graph, source_value.clone(), e, dest_value)
            {
                work_set.add(dest);
            }
        }
    }
}

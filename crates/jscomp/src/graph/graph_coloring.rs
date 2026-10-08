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
//   src/com/google/javascript/jscomp/graph/GraphColoring.java.

use super::{
    adjacency_graph::AdjacencyGraph, annotatable::Annotatable, annotation::Annotation,
    graph_node::GraphNode,
};
use std::{
    any::Any,
    cmp::Ordering,
    hash::{Hash, Hasher},
    marker::PhantomData,
};
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub value: i32,
}
impl Color {
    // port: GraphColoring.Color#Color
    pub fn new(value: i32) -> Self {
        Self { value }
    }
    // port: GraphColoring.Color#equals
    pub fn equals(&self, other: &dyn Any) -> bool {
        if let Some(color) = other.downcast_ref::<Color>() {
            self.value == color.value
        } else {
            false
        }
    }
    // port: GraphColoring.Color#hashCode
    pub fn hash_code(&self) -> i32 {
        self.value
    }
}
impl PartialEq for Color {
    // port: GraphColoring.Color#equals
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}
impl Eq for Color {}
impl Hash for Color {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}
impl Annotation for Color {
    // port: GraphColoring.Color#hashCode
    fn hash_code(&self) -> i32 {
        self.value
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn to_string(&self) -> String {
        format!(
            "com.google.javascript.jscomp.graph.GraphColoring$Color@{:x}",
            self.value
        )
    }
}
pub trait GraphColoring<N, E> {
    // port: GraphColoring#color
    fn color<G: AdjacencyGraph<N, E>>(&mut self, graph: &mut G) -> i32;
    // port: GraphColoring#getPartitionSuperNode
    fn get_partition_super_node<G: AdjacencyGraph<N, E>>(&mut self, graph: &G, node: N) -> N;
    // port: GraphColoring#haveSameColor
    fn have_same_color<G: AdjacencyGraph<N, E>>(&self, graph: &G, first: &N, second: &N) -> bool;
    // port: GraphColoring#getGraph
    fn get_graph<'a, G: AdjacencyGraph<N, E>>(&self, graph: &'a G) -> &'a G {
        graph
    }
}
pub type TieBreaker<N> = Box<dyn Fn(&N, &N) -> Ordering>;
pub struct GreedyGraphColoring<N, E> {
    color_to_node_map: Option<Vec<Option<N>>>,
    tie_breaker: Option<TieBreaker<N>>,
    marker: PhantomData<E>,
}
impl<N, E> GreedyGraphColoring<N, E> {
    // port: GraphColoring#GraphColoring
    // port: GraphColoring.GreedyGraphColoring#GreedyGraphColoring(AdjacencyGraph)
    pub fn new() -> Self {
        Self::with_tie_breaker(None)
    }
    // port: GraphColoring.GreedyGraphColoring#GreedyGraphColoring(AdjacencyGraph,Comparator)
    pub fn with_tie_breaker(tie_breaker: Option<TieBreaker<N>>) -> Self {
        Self {
            color_to_node_map: None,
            tie_breaker,
            marker: PhantomData,
        }
    }
}
impl<N, E> Default for GreedyGraphColoring<N, E> {
    fn default() -> Self {
        Self::new()
    }
}
impl<N: Clone, E> GraphColoring<N, E> for GreedyGraphColoring<N, E> {
    // port: GraphColoring.GreedyGraphColoring#color
    fn color<G: AdjacencyGraph<N, E>>(&mut self, graph: &mut G) -> i32 {
        let mut worklist = graph.get_nodes();
        worklist.sort_by(|l, r| {
            let left = l.get_value(graph);
            let right = r.get_value(graph);
            let result = graph.get_weight(right).wrapping_sub(graph.get_weight(left));
            if result == 0 {
                self.tie_breaker
                    .as_ref()
                    .map_or(Ordering::Equal, |t| t(left, right))
            } else {
                result.cmp(&0)
            }
        });
        let mut count = 0;
        loop {
            let color = Color::new(count);
            let mut subgraph = graph.new_sub_graph();
            let mut i = 0;
            while i < worklist.len() {
                let node = worklist[i];
                if subgraph.is_independent_of(graph, node.get_value(graph)) {
                    subgraph.add_node(graph, node.get_value(graph));
                    node.set_annotation(graph, Some(Box::new(color)));
                    worklist.remove(i);
                } else {
                    i += 1;
                }
            }
            count = count.wrapping_add(1);
            if worklist.is_empty() {
                break;
            }
        }
        self.color_to_node_map = Some(vec![None; count as usize]);
        count
    }
    // port: GraphColoring#getPartitionSuperNode
    fn get_partition_super_node<G: AdjacencyGraph<N, E>>(&mut self, graph: &G, node: N) -> N {
        let map = closure_rhino::check_not_null!(
            self.color_to_node_map.as_mut(),
            "No coloring founded. color() should be called first."
        );
        let color = graph
            .get_node(&node)
            .expect("")
            .get_annotation_as::<Color>(graph)
            .expect("");
        map[color.value as usize].get_or_insert(node).clone()
    }
    // port: GraphColoring#haveSameColor
    fn have_same_color<G: AdjacencyGraph<N, E>>(&self, graph: &G, first: &N, second: &N) -> bool {
        closure_rhino::check_not_null!(
            self.color_to_node_map.as_ref(),
            "No coloring founded. color() should be called first."
        );
        let a = graph
            .get_node(first)
            .expect("")
            .get_annotation_as::<Color>(graph)
            .expect("");
        let b = graph
            .get_node(second)
            .expect("")
            .get_annotation_as::<Color>(graph)
            .expect("");
        a == b
    }
}

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
//   src/com/google/javascript/jscomp/graph/CheckPathsBetweenNodes.java.

use super::{
    annotatable::Annotatable,
    annotation::Annotation,
    di_graph::{DiGraph, DiGraphEdge, DiGraphNode},
    graph_node::GraphNode,
};
use std::{any::Any, marker::PhantomData};
macro_rules! sentinel {
    ($name:ident,$java_class:literal) => {
        pub struct $name;
        impl Annotation for $name {
            fn as_any(&self) -> &dyn Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn Any {
                self
            }
            // port: Object#toString (CheckPathsBetweenNodes anonymous singleton)
            fn to_string(&self) -> String {
                // Deviation: singleton identity hashes are fixed, as graph node identity hashes are.
                concat!(
                    "com.google.javascript.jscomp.graph.CheckPathsBetweenNodes$",
                    $java_class,
                    "@0"
                )
                .to_owned()
            }
        }
    };
}
sentinel!(BackEdge, "1");
sentinel!(VisitedEdge, "2");
sentinel!(Gray, "3");
sentinel!(Black, "4");
pub struct CheckPathsBetweenNodes<N, E, NP, EP> {
    node_predicate: NP,
    edge_predicate: EP,
    inclusive: bool,
    start: DiGraphNode,
    end: DiGraphNode,
    marker: PhantomData<(N, E)>,
}
impl<N, E, NP, EP> CheckPathsBetweenNodes<N, E, NP, EP> {
    pub const BACK_EDGE: BackEdge = BackEdge;
    pub const VISITED_EDGE: VisitedEdge = VisitedEdge;
    pub const WHITE: Option<Box<dyn Annotation>> = None;
    pub const GRAY: Gray = Gray;
    pub const BLACK: Black = Black;
    // port: CheckPathsBetweenNodes#CheckPathsBetweenNodes(DiGraph,DiGraphNode,DiGraphNode,Predicate,Predicate)
    pub fn new(a: DiGraphNode, b: DiGraphNode, node_predicate: NP, edge_predicate: EP) -> Self {
        Self::with_inclusive(a, b, node_predicate, edge_predicate, true)
    }
    // port: CheckPathsBetweenNodes#CheckPathsBetweenNodes(DiGraph,DiGraphNode,DiGraphNode,Predicate,Predicate,boolean)
    pub fn with_inclusive(
        a: DiGraphNode,
        b: DiGraphNode,
        node_predicate: NP,
        edge_predicate: EP,
        inclusive: bool,
    ) -> Self {
        Self {
            node_predicate,
            edge_predicate,
            inclusive,
            start: a,
            end: b,
            marker: PhantomData,
        }
    }
    // port: CheckPathsBetweenNodes#allPathsSatisfyPredicate
    pub fn all_paths_satisfy_predicate<G: DiGraph<N, E>>(&self, graph: &mut G) -> bool
    where
        NP: Fn(&N) -> bool,
        EP: Fn(&G, DiGraphEdge) -> bool,
    {
        self.set_up(graph);
        let result = self.check_all_paths_without_back_edges(graph, self.start, self.end);
        self.tear_down(graph);
        result
    }
    // port: CheckPathsBetweenNodes#somePathsSatisfyPredicate
    pub fn some_paths_satisfy_predicate<G: DiGraph<N, E>>(&self, graph: &mut G) -> bool
    where
        NP: Fn(&N) -> bool,
        EP: Fn(&G, DiGraphEdge) -> bool,
    {
        self.set_up(graph);
        let result = self.check_some_paths_without_back_edges(graph, self.start, self.end);
        self.tear_down(graph);
        result
    }
    // port: CheckPathsBetweenNodes#setUp
    fn set_up<G: DiGraph<N, E>>(&self, graph: &mut G)
    where
        EP: Fn(&G, DiGraphEdge) -> bool,
    {
        graph.push_node_annotations();
        graph.push_edge_annotations();
        self.discover_back_edges(graph, self.start);
    }
    // port: CheckPathsBetweenNodes#tearDown
    fn tear_down<G: DiGraph<N, E>>(&self, graph: &mut G) {
        graph.pop_node_annotations();
        graph.pop_edge_annotations();
    }
    // port: CheckPathsBetweenNodes#discoverBackEdges
    fn discover_back_edges<G: DiGraph<N, E>>(&self, graph: &mut G, u: DiGraphNode)
    where
        EP: Fn(&G, DiGraphEdge) -> bool,
    {
        u.set_annotation(graph, Some(Box::new(Gray)));
        for e in u.get_out_edges(graph).to_vec() {
            if self.ignore_edge(graph, e) {
                continue;
            }
            let v = e.get_destination(graph);
            if v.get_annotation(graph).is_none() {
                self.discover_back_edges(graph, v);
            } else if v
                .get_annotation(graph)
                .is_some_and(|a| a.as_any().is::<Gray>())
            {
                e.set_annotation(graph, Some(Box::new(BackEdge)));
            }
        }
        u.set_annotation(graph, Some(Box::new(Black)));
    }
    // port: CheckPathsBetweenNodes#ignoreEdge
    fn ignore_edge<G: DiGraph<N, E>>(&self, graph: &G, e: DiGraphEdge) -> bool
    where
        EP: Fn(&G, DiGraphEdge) -> bool,
    {
        !(self.edge_predicate)(graph, e)
    }
    // port: CheckPathsBetweenNodes#checkAllPathsWithoutBackEdges
    fn check_all_paths_without_back_edges<G: DiGraph<N, E>>(
        &self,
        graph: &mut G,
        a: DiGraphNode,
        b: DiGraphNode,
    ) -> bool
    where
        NP: Fn(&N) -> bool,
        EP: Fn(&G, DiGraphEdge) -> bool,
    {
        if (self.node_predicate)(a.get_value(graph))
            && (self.inclusive || (a != self.start && a != self.end))
        {
            return true;
        }
        if a == b {
            return false;
        }
        for e in a.get_out_edges(graph).to_vec() {
            if e.get_annotation(graph)
                .is_some_and(|a| a.as_any().is::<VisitedEdge>())
            {
                continue;
            }
            e.set_annotation(graph, Some(Box::new(VisitedEdge)));
            if self.ignore_edge(graph, e) {
                continue;
            }
            if e.get_annotation(graph)
                .is_some_and(|a| a.as_any().is::<BackEdge>())
            {
                continue;
            }
            let next = e.get_destination(graph);
            if !self.check_all_paths_without_back_edges(graph, next, b) {
                return false;
            }
        }
        true
    }
    // port: CheckPathsBetweenNodes#checkSomePathsWithoutBackEdges
    fn check_some_paths_without_back_edges<G: DiGraph<N, E>>(
        &self,
        graph: &mut G,
        a: DiGraphNode,
        b: DiGraphNode,
    ) -> bool
    where
        NP: Fn(&N) -> bool,
        EP: Fn(&G, DiGraphEdge) -> bool,
    {
        if (self.node_predicate)(a.get_value(graph))
            && (self.inclusive || (a != self.start && a != self.end))
        {
            return true;
        }
        if a == b {
            return false;
        }
        for e in a.get_out_edges(graph).to_vec() {
            if e.get_annotation(graph)
                .is_some_and(|a| a.as_any().is::<VisitedEdge>())
            {
                continue;
            }
            e.set_annotation(graph, Some(Box::new(VisitedEdge)));
            if self.ignore_edge(graph, e) {
                continue;
            }
            if e.get_annotation(graph)
                .is_some_and(|a| a.as_any().is::<BackEdge>())
            {
                continue;
            }
            let next = e.get_destination(graph);
            if self.check_some_paths_without_back_edges(graph, next, b) {
                return true;
            }
        }
        false
    }
}

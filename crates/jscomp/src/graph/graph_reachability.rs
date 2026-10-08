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
//   src/com/google/javascript/jscomp/graph/GraphReachability.java.

use super::{
    annotatable::Annotatable,
    annotation::Annotation,
    di_graph::DiGraph,
    fixed_point_graph_traversal::{EdgeCallback, FixedPointGraphTraversal},
};
use std::{any::Any, marker::PhantomData};
#[derive(Debug)]
pub struct Reachable;
impl Annotation for Reachable {
    // port: Object#toString (GraphReachability.REACHABLE)
    fn to_string(&self) -> String {
        // Deviation: anonymous singleton's JVM identity hash is replaced by a fixed identity.
        "com.google.javascript.jscomp.graph.GraphReachability$1@0".to_owned()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
pub struct EdgeTuple<N, E> {
    pub source_node: N,
    pub edge: E,
}
impl<N, E> EdgeTuple<N, E> {
    // port: GraphReachability.EdgeTuple#EdgeTuple
    pub fn new(source_node: N, edge: E, _dest_node: N) -> Self {
        Self { source_node, edge }
    }
}
// The predicate may borrow (e.g. the AST it inspects) for the lifetime of the traversal object.
pub type EdgePredicate<'a, N, E> = Box<dyn Fn(&EdgeTuple<N, E>) -> bool + 'a>;
pub struct GraphReachability<'a, N, E> {
    edge_predicate: Option<EdgePredicate<'a, N, E>>,
    marker: PhantomData<(N, E)>,
}
impl<'a, N: Clone, E: Clone> GraphReachability<'a, N, E> {
    pub const REACHABLE: Reachable = Reachable;
    // port: GraphReachability#GraphReachability(DiGraph)
    pub fn new() -> Self {
        Self::with_edge_predicate(None)
    }
    // port: GraphReachability#GraphReachability(DiGraph,Predicate)
    pub fn with_edge_predicate(edge_predicate: Option<EdgePredicate<'a, N, E>>) -> Self {
        Self {
            edge_predicate,
            marker: PhantomData,
        }
    }
    // port: GraphReachability#compute
    pub fn compute<G: DiGraph<N, E>>(&mut self, graph: &mut G, entry: N) {
        graph.clear_node_annotations();
        graph
            .get_node(&entry)
            .expect("")
            .set_annotation(graph, Some(Box::new(Self::REACHABLE)));
        FixedPointGraphTraversal::new_traversal(self).compute_fixed_point_from(graph, entry);
    }
    // port: GraphReachability#recompute
    pub fn recompute<G: DiGraph<N, E>>(&mut self, graph: &mut G, reachable_node: N) {
        let new_reachable = graph.get_node(&reachable_node).expect("");
        closure_rhino::check_state!(
            !new_reachable
                .get_annotation(graph)
                .is_some_and(|a| a.as_any().is::<Reachable>())
        );
        new_reachable.set_annotation(graph, Some(Box::new(Self::REACHABLE)));
        FixedPointGraphTraversal::new_traversal(self)
            .compute_fixed_point_from(graph, reachable_node);
    }
}
impl<N: Clone, E: Clone> Default for GraphReachability<'_, N, E> {
    fn default() -> Self {
        Self::new()
    }
}
impl<N: Clone, E: Clone, G: DiGraph<N, E>> EdgeCallback<N, E, G>
    for &mut GraphReachability<'_, N, E>
{
    // port: GraphReachability#traverseEdge
    fn traverse_edge(&mut self, graph: &mut G, source: N, e: E, destination: N) -> bool {
        if graph
            .get_node(&source)
            .expect("")
            .get_annotation(graph)
            .is_some_and(|a| a.as_any().is::<Reachable>())
            && self
                .edge_predicate
                .as_ref()
                .is_none_or(|p| p(&EdgeTuple::new(source, e, destination.clone())))
        {
            let dest = graph.get_node(&destination).expect("");
            if !dest
                .get_annotation(graph)
                .is_some_and(|a| a.as_any().is::<Reachable>())
            {
                dest.set_annotation(graph, Some(Box::new(Reachable)));
                return true;
            }
        }
        false
    }
}

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
//   src/com/google/javascript/jscomp/graph/AdjacencyGraph.java,
//   src/com/google/javascript/jscomp/graph/DiGraph.java,
//   src/com/google/javascript/jscomp/graph/Graph.java.

use super::{
    annotatable::Annotatable, annotation::Annotation, graph_node::GraphNode, sub_graph::SubGraph,
};
pub trait AdjacencyGraph<N, E>: Sized {
    type Node: GraphNode<N, E, Self>;
    // port: Graph#getNodes
    // port: DiGraph#getNodes
    // port: AdjacencyGraph#getNodes
    fn get_nodes(&self) -> Vec<Self::Node>;
    // port: Graph#getNodeCount
    // port: AdjacencyGraph#getNodeCount
    fn get_node_count(&self) -> usize;
    // port: DiGraph#getNode
    // port: AdjacencyGraph#getNode
    fn get_node(&self, value: &N) -> Option<Self::Node>;
    // port: AdjacencyGraph#newSubGraph
    fn new_sub_graph(&self) -> Box<dyn SubGraph<N, E, Self>>;
    // port: AdjacencyGraph#clearNodeAnnotations
    // port: Graph#clearNodeAnnotations
    fn clear_node_annotations(&mut self) {
        for n in self.get_nodes() {
            n.set_annotation(self, None);
        }
    }
    // port: AdjacencyGraph#getWeight
    fn get_weight(&self, value: &N) -> i32;
    // Arena accessors, implemented by graph wrappers through delegation.
    fn node_value(&self, node: Self::Node) -> &N;
    fn node_annotation(&self, node: Self::Node) -> &Option<Box<dyn Annotation>>;
    fn node_annotation_mut(&mut self, node: Self::Node) -> &mut Option<Box<dyn Annotation>>;
}

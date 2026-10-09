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
//   src/com/google/javascript/jscomp/graph/GraphvizGraph.java.

pub trait GraphvizGraph: Sized {
    type Node: GraphvizNode<Self>;
    type Edge: GraphvizEdge<Self>;
    // port: GraphvizGraph#getName
    fn get_name(&self) -> &str;
    // port: GraphvizGraph#isDirected
    fn is_directed(&self) -> bool;
    // port: GraphvizGraph#getGraphvizNodes
    fn get_graphviz_nodes(&self) -> Vec<Self::Node>;
    // port: GraphvizGraph#getGraphvizEdges
    fn get_graphviz_edges(&self) -> Vec<Self::Edge>;
}
pub trait GraphvizNode<G> {
    // port: GraphvizGraph.GraphvizNode#getId
    fn get_id(&self, graph: &G) -> String;
    // port: GraphvizGraph.GraphvizNode#getColor
    fn get_color(&self, graph: &G) -> &str;
    // port: GraphvizGraph.GraphvizNode#getLabel
    fn get_label(&self, graph: &G) -> String;
}
pub trait GraphvizEdge<G> {
    // port: GraphvizGraph.GraphvizEdge#getNode1Id
    fn get_node1_id(&self, graph: &G) -> String;
    // port: GraphvizGraph.GraphvizEdge#getNode2Id
    fn get_node2_id(&self, graph: &G) -> String;
    // port: GraphvizGraph.GraphvizEdge#getColor
    fn get_color(&self, graph: &G) -> &str;
    // port: GraphvizGraph.GraphvizEdge#getLabel
    fn get_label(&self, graph: &G) -> String;
}

/// Java String.valueOf for edge labels, including null-valued edges. Graphs
/// with arena edge values can instead supply a renderer at construction.
pub trait GraphvizValue {
    // port: String#valueOf(Object)
    fn to_graphviz_string(&self) -> String;
}
impl GraphvizValue for str {
    // port: String#valueOf(Object)
    fn to_graphviz_string(&self) -> String {
        self.to_owned()
    }
}
impl GraphvizValue for String {
    // port: String#valueOf(Object)
    fn to_graphviz_string(&self) -> String {
        self.clone()
    }
}
impl<T: GraphvizValue + ?Sized> GraphvizValue for &T {
    // port: String#valueOf(Object)
    fn to_graphviz_string(&self) -> String {
        (*self).to_graphviz_string()
    }
}
impl<T: GraphvizValue> GraphvizValue for Option<T> {
    // port: String#valueOf(Object)
    fn to_graphviz_string(&self) -> String {
        self.as_ref()
            .map_or_else(|| "null".to_owned(), GraphvizValue::to_graphviz_string)
    }
}
impl GraphvizValue for () {
    // port: String#valueOf(Object)
    fn to_graphviz_string(&self) -> String {
        "null".to_owned()
    }
}
impl GraphvizValue for i32 {
    // port: String#valueOf(Object)
    fn to_graphviz_string(&self) -> String {
        self.to_string()
    }
}
impl GraphvizValue for i64 {
    // port: String#valueOf(Object)
    fn to_graphviz_string(&self) -> String {
        self.to_string()
    }
}
impl GraphvizValue for bool {
    // port: String#valueOf(Object)
    fn to_graphviz_string(&self) -> String {
        self.to_string()
    }
}
impl GraphvizValue for f64 {
    // port: String#valueOf(Object)
    fn to_graphviz_string(&self) -> String {
        closure_rhino::java_lang::double_to_string(*self)
    }
}

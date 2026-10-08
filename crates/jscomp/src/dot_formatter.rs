/*
 * Copyright 2007 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/DotFormatter.java.

use crate::graph::{
    adjacency_graph::AdjacencyGraph, annotatable::Annotatable, graph::GraphEdge,
    graph_node::GraphNode,
};
use crate::{
    AbstractCompiler,
    control_flow_graph::ControlFlowGraph,
    graph::{
        di_graph::DiGraph,
        graph::Graph,
        graphviz_graph::{GraphvizEdge, GraphvizGraph, GraphvizNode},
    },
};
use closure_jstype::js_type::JSType as _;
use closure_rhino::{js_string::JsString, node::NodeId};
use indexmap::IndexMap;

pub struct DotFormatter<'a> {
    assignments: IndexMap<NodeId, i32>,
    key_count: i32,
    builder: String,
    cfg: Option<&'a ControlFlowGraph<NodeId>>,
    print_annotations: bool,
}
impl<'a> DotFormatter<'a> {
    const INDENT: &'static str = "  ";
    const ARROW: &'static str = " -> ";
    const LINE: &'static str = " -- ";
    pub const MAX_LABEL_NAME_LENGTH: usize = 10;
    // port: DotFormatter#DotFormatter()
    fn new() -> Self {
        Self {
            assignments: IndexMap::new(),
            key_count: 0,
            builder: String::new(),
            cfg: None,
            print_annotations: false,
        }
    }
    // port: DotFormatter#DotFormatter(Node, ControlFlowGraph, Appendable, boolean)
    fn new_with_cfg(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        cfg: Option<&'a ControlFlowGraph<NodeId>>,
        print_annotations: bool,
    ) -> Self {
        let mut dot = Self {
            cfg,
            print_annotations,
            ..Self::new()
        };
        dot.format_preamble();
        dot.traverse_nodes(compiler, n);
        dot.format_conclusion();
        dot
    }
    // port: DotFormatter#toDot(Node)
    pub fn to_dot(compiler: &mut AbstractCompiler, n: NodeId) -> String {
        Self::to_dot_with_cfg(compiler, n, None)
    }
    // port: DotFormatter#toDot(Node, ControlFlowGraph)
    pub fn to_dot_with_cfg(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        cfg: Option<&'a ControlFlowGraph<NodeId>>,
    ) -> String {
        Self::new_with_cfg(compiler, n, cfg, false).builder
    }
    // port: DotFormatter#toDot(GraphvizGraph)
    pub fn to_dot_graph(graph: &impl GraphvizGraph) -> String {
        let mut builder = format!(
            "{}{}{} {{\n{}node [color=lightblue2, style=filled];\n",
            if graph.is_directed() {
                "digraph"
            } else {
                "graph"
            },
            Self::INDENT,
            graph.get_name(),
            Self::INDENT
        );
        let edge_symbol = if graph.is_directed() {
            Self::ARROW
        } else {
            Self::LINE
        };
        let mut node_names: Vec<_> = graph
            .get_graphviz_nodes()
            .iter()
            .map(|n| {
                format!(
                    "{} [label=\"{}\" color=\"{}\"]",
                    n.get_id(graph),
                    n.get_label(graph),
                    n.get_color(graph)
                )
            })
            .collect();
        node_names.sort_by(|a, b| JsString::from(a.as_str()).cmp(&JsString::from(b.as_str())));
        for name in node_names {
            builder.push_str(&format!("{}{name};\n", Self::INDENT));
        }
        let mut edges = graph.get_graphviz_edges();
        edges.sort_by(|a, b| {
            JsString::from(a.get_node1_id(graph).as_str())
                .cmp(&JsString::from(b.get_node1_id(graph).as_str()))
                .then_with(|| {
                    JsString::from(a.get_node2_id(graph).as_str())
                        .cmp(&JsString::from(b.get_node2_id(graph).as_str()))
                })
        });
        for e in edges {
            builder.push_str(&format!(
                "{}{}{edge_symbol}{} [label=\"{}\" color=\"{}\"];\n",
                Self::INDENT,
                e.get_node1_id(graph),
                e.get_node2_id(graph),
                e.get_label(graph),
                e.get_color(graph)
            ));
        }
        builder.push_str("}\n");
        builder
    }
    // port: DotFormatter#appendDot
    pub fn append_dot(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        cfg: Option<&'a ControlFlowGraph<NodeId>>,
        builder: &mut String,
    ) {
        builder.push_str(&Self::to_dot_with_cfg(compiler, n, cfg));
    }
    // port: DotFormatter#newInstanceForTesting
    pub fn new_instance_for_testing() -> Self {
        Self::new()
    }
    // port: DotFormatter#traverseNodes
    fn traverse_nodes(&mut self, compiler: &mut AbstractCompiler, parent: NodeId) {
        let key_parent = self.key(compiler, parent);
        let mut next = parent.get_first_child(compiler);
        while let Some(child) = next {
            let key_child = self.key(compiler, child);
            self.builder.push_str(&format!(
                "{}{}{}{} [weight=1];\n",
                Self::INDENT,
                Self::format_node_name(key_parent),
                Self::ARROW,
                Self::format_node_name(key_child)
            ));
            self.traverse_nodes(compiler, child);
            next = child.get_next(compiler);
        }
        if let Some(cfg) = self.cfg.filter(|cfg| cfg.has_node(&Some(parent))) {
            let mut edge_list = vec![];
            for edge in cfg.get_out_edges(&Some(parent)) {
                let succ = edge.get_destination(cfg);
                let to_node = if succ == cfg.get_implicit_return() {
                    "RETURN".into()
                } else {
                    Self::format_node_name(self.key(compiler, succ.get_value(cfg).unwrap()))
                };
                edge_list.push(format!("{}{}{to_node} [label=\"{}\", fontcolor=\"red\", weight=0.01, color=\"red\"];\n",Self::format_node_name(key_parent),Self::ARROW,edge.get_value(cfg)));
            }
            edge_list.sort();
            for edge in edge_list {
                self.builder.push_str(Self::INDENT);
                self.builder.push_str(&edge);
            }
        }
    }
    // port: DotFormatter#key
    pub fn key(&mut self, compiler: &mut AbstractCompiler, n: NodeId) -> i32 {
        if let Some(&key) = self.assignments.get(&n) {
            return key;
        }
        let key = self.key_count;
        self.key_count += 1;
        self.assignments.insert(n, key);
        self.builder.push_str(&format!(
            "{}{} [label=\"{}",
            Self::INDENT,
            Self::format_node_name(key),
            n.get_token(compiler)
        ));
        if n.is_name(compiler)
            || n.is_string_lit(compiler)
            || n.is_get_prop(compiler)
            || n.is_import_star(compiler)
            || n.is_string_key(compiler)
        {
            self.builder.push_str(&Self::get_node_label(compiler, n));
        }
        if let Some(ty) = n.get_jstype(compiler) {
            self.builder.push_str(" : ");
            // type.toString(): the type belongs to the compiler's registry.
            let (registry, ast) = compiler.get_type_registry_field_and_ast();
            let registry = registry.expect("a node with a JSType has a type registry");
            self.builder.push_str(&ty.to_string(registry, ast));
        }
        if self.print_annotations && self.cfg.is_some_and(|cfg| cfg.has_node(&Some(n))) {
            self.append_annotation(compiler, n);
        }
        self.builder.push('"');
        if n.get_jsdoc_info(compiler).is_some() {
            self.builder.push_str(" color=\"green\"");
        }
        self.builder.push_str("];\n");
        key
    }
    // Annotation handling is part of DotFormatter#key in Java.
    // port: DotFormatter#key
    fn append_annotation(&mut self, _compiler: &AbstractCompiler, n: NodeId) {
        let cfg = self.cfg.unwrap();
        if let Some(annotation) = cfg.get_node(&Some(n)).unwrap().get_annotation(cfg) {
            self.builder.push_str("\\n");
            self.builder.push_str(&annotation.to_string());
        }
    }
    // port: DotFormatter#getNodeLabel
    fn get_node_label(compiler: &AbstractCompiler, n: NodeId) -> String {
        let mut content = n.get_string(compiler);
        if content.length() > Self::MAX_LABEL_NAME_LENGTH {
            content = content.substring(0, Self::MAX_LABEL_NAME_LENGTH);
        }
        if content.is_empty() {
            String::new()
        } else {
            format!("({content})")
        }
    }
    // port: DotFormatter#formatNodeName
    fn format_node_name(key: i32) -> String {
        format!("node{key}")
    }
    // port: DotFormatter#formatPreamble
    fn format_preamble(&mut self) {
        self.builder.push_str("digraph AST {\n");
        self.builder.push_str(Self::INDENT);
        self.builder
            .push_str("node [color=lightblue2, style=filled];\n");
    }
    // port: DotFormatter#formatConclusion
    fn format_conclusion(&mut self) {
        self.builder.push_str("}\n");
    }
}

/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ControlFlowAnalysis.java,
//   src/com/google/javascript/jscomp/ControlFlowGraph.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    graph::{
        di_graph::{DiGraph, DiGraphEdge, DiGraphNode},
        graph::Graph,
        graphviz_graph::{GraphvizEdge, GraphvizGraph, GraphvizNode},
        linked_directed_graph::LinkedDirectedGraph,
    },
    node_traversal::NodeTraversal,
    node_util::NodeUtil,
};
use closure_rhino::{node::NodeId, token::Token};
use std::{fmt, hash::Hash};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum Branch {
    ON_TRUE,
    ON_FALSE,
    UNCOND,
    ON_EX,
    SYN_BLOCK,
}
impl Branch {
    // port: ControlFlowGraph.Branch#isConditional
    pub fn is_conditional(self) -> bool {
        self == Self::ON_TRUE || self == Self::ON_FALSE
    }
}
impl fmt::Display for Branch {
    // port: Enum#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
pub type NodeComparator = Box<dyn Fn(DiGraphNode, DiGraphNode) -> std::cmp::Ordering + Send + Sync>;
pub struct ControlFlowGraph<N: Clone + Eq + Hash> {
    pub graph: LinkedDirectedGraph<Option<N>, Branch>,
    implicit_return: DiGraphNode,
    entry: DiGraphNode,
    pub(crate) ast_comparator: bool,
    node_formatter: fn(Option<&AbstractCompiler>, &N) -> String,
}
impl<N: Clone + Eq + Hash> ControlFlowGraph<N> {
    // port: ControlFlowGraph#ControlFlowGraph
    pub fn new(entry: N, node_annotations: bool, edge_annotations: bool) -> Self
    where
        N: fmt::Display,
    {
        Self::new_with_node_formatter(entry, node_annotations, edge_annotations, |_, n| {
            n.to_string()
        })
    }
    // AST values receive their compiler only when a caller formats the graph.
    pub fn new_with_node_formatter(
        entry: N,
        node_annotations: bool,
        edge_annotations: bool,
        node_formatter: fn(Option<&AbstractCompiler>, &N) -> String,
    ) -> Self {
        let mut graph = LinkedDirectedGraph::new_with_value_to_strings(
            node_annotations,
            edge_annotations,
            |_| "null".into(),
            |branch: &Branch| branch.to_string(),
        );
        let implicit_return = graph.create_node(None);
        let entry = graph.create_node(Some(entry));
        Self {
            graph,
            implicit_return,
            entry,
            ast_comparator: false,
            node_formatter,
        }
    }
    pub fn format_node_value(&self, compiler: &AbstractCompiler, value: Option<&N>) -> String {
        self.format_node_value_with_compiler(Some(compiler), value)
    }
    fn format_node_value_with_compiler(
        &self,
        compiler: Option<&AbstractCompiler>,
        value: Option<&N>,
    ) -> String {
        value
            .map(|v| (self.node_formatter)(compiler, v))
            .unwrap_or_else(|| "null".into())
    }
    pub fn with_compiler<'a>(
        &'a self,
        compiler: &'a AbstractCompiler,
    ) -> FormattedControlFlowGraph<'a, N> {
        FormattedControlFlowGraph {
            cfg: self,
            compiler,
        }
    }
    // port: ControlFlowGraph#toString
    fn write_to(
        &self,
        compiler: Option<&AbstractCompiler>,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str("CFG:\n")?;
        for e in self.get_edges() {
            let from = self.format_node_value_with_compiler(
                compiler,
                e.get_source(self).get_value(self).as_ref(),
            );
            let to = self.format_node_value_with_compiler(
                compiler,
                e.get_destination(self).get_value(self).as_ref(),
            );
            writeln!(f, "{from} -> {to}")?;
        }
        Ok(())
    }
    // port: ControlFlowGraph#getImplicitReturn
    pub fn get_implicit_return(&self) -> DiGraphNode {
        self.implicit_return
    }
    // port: ControlFlowGraph#getEntry
    pub fn get_entry(&self) -> DiGraphNode {
        self.entry
    }
    // port: ControlFlowGraph#isImplicitReturn
    pub fn is_implicit_return(&self, node: DiGraphNode) -> bool {
        node == self.implicit_return
    }
    // port: ControlFlowGraph#getOptionalNodeComparator
    // port: ControlFlowAnalysis.AstControlFlowGraph#getOptionalNodeComparator
    pub fn get_optional_node_comparator(&self, is_forward: bool) -> Option<NodeComparator> {
        if !self.ast_comparator {
            return None;
        }
        let priorities: Vec<_> = self
            .get_nodes()
            .iter()
            .map(|n| n.get_priority(self))
            .collect();
        Some(Box::new(move |x, y| {
            if is_forward {
                priorities[x.0].cmp(&priorities[y.0])
            } else {
                priorities[y.0].cmp(&priorities[x.0])
            }
        }))
    }
}
impl ControlFlowGraph<NodeId> {
    // port: ControlFlowAnalysis.AstControlFlowGraph#AstControlFlowGraph
    pub(crate) fn new_ast(entry: NodeId, edge_annotations: bool) -> Self {
        let mut cfg =
            Self::new_with_node_formatter(entry, true, edge_annotations, |compiler, n| {
                n.to_string(compiler.expect("AST CFG formatting requires a compiler"))
            });
        cfg.ast_comparator = true;
        cfg
    }
    // port: ControlFlowGraph#isEnteringNewCfgNode
    pub fn is_entering_new_cfg_node(compiler: &AbstractCompiler, n: NodeId) -> bool {
        let parent = n.get_parent(compiler).unwrap();
        match parent.get_token(compiler) {
            Token::BLOCK | Token::ROOT | Token::SCRIPT | Token::TRY | Token::SWITCH_BODY => true,
            Token::FUNCTION => Some(n) != parent.get_second_child(compiler),
            Token::WHILE | Token::DO | Token::IF | Token::FOR => {
                NodeUtil::get_condition_expression(compiler, parent) != Some(n)
            }
            Token::FOR_IN | Token::CASE | Token::CATCH | Token::WITH => {
                Some(n) != parent.get_first_child(compiler)
            }
            _ => false,
        }
    }
}
pub trait AbstractCfgNodeTraversalCallback {
    // port: NodeTraversal.Callback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>);
}
/// A subclass instance as a NodeTraversal callback. The impl preserves Java's final
/// shouldTraverse: subclasses supply visit only. (A blanket `impl<C: ..> Callback for C` would
/// overlap GuardedCallback's blanket impl, which Rust coherence rejects.)
pub struct AbstractCfgNodeTraversal<C>(pub C);
impl<C: AbstractCfgNodeTraversalCallback> crate::node_traversal::Callback
    for AbstractCfgNodeTraversal<C>
{
    // port: ControlFlowGraph.AbstractCfgNodeTraversalCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if parent.is_none() {
            return true;
        }
        !ControlFlowGraph::is_entering_new_cfg_node(t.get_compiler(), n)
    }
    // port: NodeTraversal.Callback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        AbstractCfgNodeTraversalCallback::visit(&mut self.0, t, n, parent)
    }
}

use crate::graph::{
    adjacency_graph::AdjacencyGraph,
    annotatable::Annotatable,
    annotation::Annotation,
    graph::GraphEdge,
    graph::{GraphAnnotationStacks, SimpleSubGraph},
    graph_node::GraphNode,
    sub_graph::SubGraph,
};

impl<N: Clone + Eq + Hash> AdjacencyGraph<Option<N>, Branch> for ControlFlowGraph<N> {
    type Node = DiGraphNode;
    fn get_nodes(&self) -> Vec<DiGraphNode> {
        self.graph.get_nodes()
    }
    fn get_node_count(&self) -> usize {
        self.graph.get_node_count()
    }
    fn get_node(&self, value: &Option<N>) -> Option<DiGraphNode> {
        self.graph.get_node(value)
    }
    fn new_sub_graph(&self) -> Box<dyn SubGraph<Option<N>, Branch, Self>> {
        Box::new(SimpleSubGraph::<DiGraphNode>::new())
    }
    fn get_weight(&self, value: &Option<N>) -> i32 {
        self.graph.get_weight(value)
    }
    fn node_value(&self, node: DiGraphNode) -> &Option<N> {
        self.graph.node_value(node)
    }
    fn node_annotation(&self, node: DiGraphNode) -> &Option<Box<dyn Annotation>> {
        self.graph.node_annotation(node)
    }
    fn node_annotation_mut(&mut self, node: DiGraphNode) -> &mut Option<Box<dyn Annotation>> {
        self.graph.node_annotation_mut(node)
    }
}

impl<N: Clone + Eq + Hash> Graph<Option<N>, Branch> for ControlFlowGraph<N> {
    type Edge = DiGraphEdge;
    fn value_to_string(&self, value: &Option<N>) -> String {
        self.format_node_value_with_compiler(None, value.as_ref())
    }
    fn connect(&mut self, n1: Option<N>, edge: Branch, n2: Option<N>) {
        self.graph.connect(n1, edge, n2)
    }
    fn disconnect(&mut self, n1: &Option<N>, n2: &Option<N>) {
        self.graph.disconnect(n1, n2)
    }
    fn create_node(&mut self, value: Option<N>) -> DiGraphNode {
        self.graph.create_node(value)
    }
    fn get_edges(&self) -> Vec<DiGraphEdge> {
        self.graph.get_edges()
    }
    fn get_edges_between(&self, n1: &Option<N>, n2: &Option<N>) -> Vec<DiGraphEdge> {
        self.graph.get_edges_between(n1, n2)
    }
    fn get_node_degree(&self, value: &Option<N>) -> i32 {
        self.graph.get_node_degree(value)
    }
    fn get_neighbor_nodes(&self, value: &Option<N>) -> Vec<DiGraphNode> {
        self.graph.get_neighbor_nodes(value)
    }
    fn get_first_edge(&self, n1: &Option<N>, n2: &Option<N>) -> Option<DiGraphEdge> {
        self.graph.get_first_edge(n1, n2)
    }
    fn is_connected(&self, n1: &Option<N>, n2: &Option<N>) -> bool {
        self.graph.is_connected(n1, n2)
    }
    fn is_connected_with_edge(&self, n1: &Option<N>, e: &Branch, n2: &Option<N>) -> bool {
        self.graph.is_connected_with_edge(n1, e, n2)
    }
    fn annotation_stacks(&mut self) -> &mut GraphAnnotationStacks<DiGraphNode, DiGraphEdge> {
        self.graph.annotation_stacks()
    }
    fn edge_value(&self, edge: DiGraphEdge) -> &Branch {
        self.graph.edge_value(edge)
    }
    fn edge_node_a(&self, edge: DiGraphEdge) -> DiGraphNode {
        self.graph.edge_node_a(edge)
    }
    fn edge_node_b(&self, edge: DiGraphEdge) -> DiGraphNode {
        self.graph.edge_node_b(edge)
    }
    fn edge_annotation(&self, edge: DiGraphEdge) -> &Option<Box<dyn Annotation>> {
        self.graph.edge_annotation(edge)
    }
    fn edge_annotation_mut(&mut self, edge: DiGraphEdge) -> &mut Option<Box<dyn Annotation>> {
        self.graph.edge_annotation_mut(edge)
    }
}

impl<N: Clone + Eq + Hash> DiGraph<Option<N>, Branch> for ControlFlowGraph<N> {
    fn get_edges_in_direction(&self, n1: &Option<N>, n2: &Option<N>) -> Vec<DiGraphEdge> {
        self.graph.get_edges_in_direction(n1, n2)
    }
    fn get_out_edges(&self, value: &Option<N>) -> &[DiGraphEdge] {
        self.graph.get_out_edges(value)
    }
    fn get_in_edges(&self, value: &Option<N>) -> &[DiGraphEdge] {
        self.graph.get_in_edges(value)
    }
    fn get_directed_pred_nodes(&self, value: &Option<N>) -> Vec<DiGraphNode> {
        self.graph.get_directed_pred_nodes(value)
    }
    fn get_directed_succ_nodes(&self, value: &Option<N>) -> Vec<DiGraphNode> {
        self.graph.get_directed_succ_nodes(value)
    }
    fn disconnect_in_direction(&mut self, n1: &Option<N>, n2: &Option<N>) {
        self.graph.disconnect_in_direction(n1, n2)
    }
    fn is_connected_in_direction(&self, n1: &Option<N>, n2: &Option<N>) -> bool {
        self.graph.is_connected_in_direction(n1, n2)
    }
    fn is_connected_in_direction_with_edge(
        &self,
        n1: &Option<N>,
        e: &Branch,
        n2: &Option<N>,
    ) -> bool {
        self.graph.is_connected_in_direction_with_edge(n1, e, n2)
    }
    fn node_out_edges(&self, node: DiGraphNode) -> &[DiGraphEdge] {
        self.graph.node_out_edges(node)
    }
    fn node_in_edges(&self, node: DiGraphNode) -> &[DiGraphEdge] {
        self.graph.node_in_edges(node)
    }
    fn node_out_edges_mut(&mut self, node: DiGraphNode) -> &mut Vec<DiGraphEdge> {
        self.graph.node_out_edges_mut(node)
    }
    fn node_in_edges_mut(&mut self, node: DiGraphNode) -> &mut Vec<DiGraphEdge> {
        self.graph.node_in_edges_mut(node)
    }
    fn node_priority(&self, node: DiGraphNode) -> i32 {
        self.graph.node_priority(node)
    }
    fn node_priority_mut(&mut self, node: DiGraphNode) -> &mut i32 {
        self.graph.node_priority_mut(node)
    }
}

impl<N: Clone + Eq + Hash + fmt::Display> GraphvizGraph for ControlFlowGraph<N> {
    type Node = DiGraphNode;
    type Edge = DiGraphEdge;
    fn get_name(&self) -> &str {
        "LinkedGraph"
    }
    fn is_directed(&self) -> bool {
        true
    }
    fn get_graphviz_nodes(&self) -> Vec<DiGraphNode> {
        self.get_nodes()
    }
    fn get_graphviz_edges(&self) -> Vec<DiGraphEdge> {
        self.get_edges()
    }
}
impl<N: Clone + Eq + Hash + fmt::Display> GraphvizNode<ControlFlowGraph<N>> for DiGraphNode {
    fn get_id(&self, _graph: &ControlFlowGraph<N>) -> String {
        format!("LDN{}", self.0)
    }
    fn get_color(&self, _graph: &ControlFlowGraph<N>) -> &str {
        "white"
    }
    fn get_label(&self, graph: &ControlFlowGraph<N>) -> String {
        let mut result =
            graph.format_node_value_with_compiler(None, self.get_value(graph).as_ref());
        if let Some(a) = self.get_annotation(graph) {
            result.push('\n');
            result.push_str(&a.to_string());
        }
        result
    }
}
impl<N: Clone + Eq + Hash + fmt::Display> GraphvizEdge<ControlFlowGraph<N>> for DiGraphEdge {
    fn get_node1_id(&self, graph: &ControlFlowGraph<N>) -> String {
        self.get_source(graph).get_id(graph)
    }
    fn get_node2_id(&self, graph: &ControlFlowGraph<N>) -> String {
        self.get_destination(graph).get_id(graph)
    }
    fn get_color(&self, _graph: &ControlFlowGraph<N>) -> &str {
        "black"
    }
    fn get_label(&self, graph: &ControlFlowGraph<N>) -> String {
        self.get_value(graph).to_string()
    }
}
impl<N: Clone + Eq + Hash + fmt::Display> fmt::Display for ControlFlowGraph<N> {
    // port: ControlFlowGraph#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_to(None, f)
    }
}

/// A print-time borrow supplies the AST without caching node text or storing a compiler in a pass.
pub struct FormattedControlFlowGraph<'a, N: Clone + Eq + Hash> {
    cfg: &'a ControlFlowGraph<N>,
    compiler: &'a AbstractCompiler,
}
impl<N: Clone + Eq + Hash> fmt::Display for FormattedControlFlowGraph<'_, N> {
    // port: ControlFlowGraph#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cfg.write_to(Some(self.compiler), f)
    }
}
impl<N: Clone + Eq + Hash> GraphvizGraph for FormattedControlFlowGraph<'_, N> {
    type Node = DiGraphNode;
    type Edge = DiGraphEdge;
    fn get_name(&self) -> &str {
        "LinkedGraph"
    }
    fn is_directed(&self) -> bool {
        true
    }
    fn get_graphviz_nodes(&self) -> Vec<DiGraphNode> {
        self.cfg.get_nodes()
    }
    fn get_graphviz_edges(&self) -> Vec<DiGraphEdge> {
        self.cfg.get_edges()
    }
}
impl<N: Clone + Eq + Hash> GraphvizNode<FormattedControlFlowGraph<'_, N>> for DiGraphNode {
    fn get_id(&self, _graph: &FormattedControlFlowGraph<'_, N>) -> String {
        format!("LDN{}", self.0)
    }
    fn get_color(&self, _graph: &FormattedControlFlowGraph<'_, N>) -> &str {
        "white"
    }
    fn get_label(&self, graph: &FormattedControlFlowGraph<'_, N>) -> String {
        let mut result = graph
            .cfg
            .format_node_value(graph.compiler, self.get_value(graph.cfg).as_ref());
        if let Some(a) = self.get_annotation(graph.cfg) {
            result.push('\n');
            result.push_str(&a.to_string());
        }
        result
    }
}
impl<N: Clone + Eq + Hash> GraphvizEdge<FormattedControlFlowGraph<'_, N>> for DiGraphEdge {
    fn get_node1_id(&self, graph: &FormattedControlFlowGraph<'_, N>) -> String {
        self.get_source(graph.cfg).get_id(graph)
    }
    fn get_node2_id(&self, graph: &FormattedControlFlowGraph<'_, N>) -> String {
        self.get_destination(graph.cfg).get_id(graph)
    }
    fn get_color(&self, _graph: &FormattedControlFlowGraph<'_, N>) -> &str {
        "black"
    }
    fn get_label(&self, graph: &FormattedControlFlowGraph<'_, N>) -> String {
        self.get_value(graph.cfg).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Compiler, dot_formatter::DotFormatter};

    #[test]
    fn ast_values_are_formatted_after_mutation() {
        let mut compiler = Compiler::new();
        let entry = compiler.new_string_with_token(Token::NAME, "before");
        let mut cfg = ControlFlowGraph::new_ast(entry, false);
        cfg.connect(Some(entry), Branch::UNCOND, None);
        assert_eq!(
            cfg.with_compiler(&compiler).to_string(),
            "CFG:\nNAME before -> null\n"
        );

        entry.set_string(&mut compiler, "after");
        assert_eq!(
            cfg.with_compiler(&compiler).to_string(),
            "CFG:\nNAME after -> null\n"
        );
        let dot = DotFormatter::to_dot_graph(&cfg.with_compiler(&compiler));
        assert!(dot.contains("label=\"NAME after\""));
        assert!(!dot.contains("before"));
    }

    #[test]
    fn display_values_keep_generic_graph_formatting() {
        let mut cfg = ControlFlowGraph::new(7, true, false);
        cfg.connect(Some(7), Branch::UNCOND, None);
        assert_eq!(cfg.to_string(), "CFG:\n7 -> null\n");
        assert!(DotFormatter::to_dot_graph(&cfg).contains("label=\"7\""));
    }
}

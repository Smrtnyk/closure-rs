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
//   src/com/google/javascript/jscomp/ControlFlowAnalysis.java.

#![allow(clippy::collapsible_match)] // Retain the Java token switch and its nested branches.
use crate::graph::{adjacency_graph::AdjacencyGraph, graph_node::GraphNode};
use crate::{
    abstract_compiler::AbstractCompiler,
    control_flow_graph::{Branch, ControlFlowGraph},
    graph::{
        di_graph::{DiGraph, DiGraphNode},
        graph::Graph,
    },
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::java_util::priority_queue::PriorityQueue;
use closure_rhino::jscomp_base::linked_identity_hash_map::LinkedIdentityHashMap;
use closure_rhino::{
    check_argument, check_not_null, check_state, js_string::JsString, node::NodeId, token::Token,
};
use indexmap::{IndexMap, IndexSet};
use std::collections::VecDeque;

pub struct ControlFlowAnalysis {
    cfg: ControlFlowGraph<NodeId>,
    ast_position: Option<LinkedIdentityHashMap<Option<NodeId>, i32>>,
    ast_position_counter: i32,
    priority_counter: i32,
    should_traverse_functions: bool,
    root: NodeId,
    exception_handler: VecDeque<NodeId>,
    // Guava HashMultimap's identity-hash order varies per JVM. Use insertion order.
    finally_map: IndexMap<NodeId, IndexSet<Option<NodeId>>>,
}
#[derive(Default)]
pub struct Builder {
    cfg_root: Option<NodeId>,
    should_traverse_functions: bool,
    edge_annotations: bool,
}
impl Builder {
    // port: ControlFlowAnalysis.Builder#Builder
    pub fn new() -> Self {
        Self::default()
    }
    // port: ControlFlowAnalysis.Builder#setCfgRoot
    pub fn set_cfg_root(mut self, cfg_root: NodeId) -> Self {
        self.cfg_root = Some(cfg_root);
        self
    }
    // port: ControlFlowAnalysis.Builder#setTraverseFunctions
    pub fn set_traverse_functions(mut self, value: bool) -> Self {
        self.should_traverse_functions = value;
        self
    }
    // port: ControlFlowAnalysis.Builder#setIncludeEdgeAnnotations
    pub fn set_include_edge_annotations(mut self, value: bool) -> Self {
        self.edge_annotations = value;
        self
    }
    // port: ControlFlowAnalysis.Builder#setCompiler
    // Compiler is passed to compute_cfg per DESIGN §6, never stored by a pass.
    pub fn set_compiler(self, _compiler: &mut AbstractCompiler) -> Self {
        self
    }
    // port: ControlFlowAnalysis.Builder#computeCfg
    pub fn compute_cfg(self, compiler: &mut AbstractCompiler) -> ControlFlowGraph<NodeId> {
        let root = check_not_null!(self.cfg_root, "Need to call setCfgRoot()");
        let mut cfa = ControlFlowAnalysis::new(
            compiler,
            root,
            self.should_traverse_functions,
            self.edge_annotations,
        );
        cfa.compute_cfg(compiler, root);
        cfa.cfg
    }
}
impl ControlFlowAnalysis {
    // port: ControlFlowAnalysis#ControlFlowAnalysis
    // port: ControlFlowAnalysis#computeCfg
    fn new(
        compiler: &AbstractCompiler,
        root: NodeId,
        should_traverse_functions: bool,
        edge_annotations: bool,
    ) -> Self {
        // The owning Rust CFG is allocated here; Java validates the root before that allocation.
        check_argument!(
            NodeUtil::is_valid_cfg_root(compiler, root),
            "Unexpected control flow graph root %s",
            root.to_string(compiler)
        );
        let cfg =
            ControlFlowGraph::new_ast(Self::compute_fall_through(compiler, root), edge_annotations);
        Self {
            cfg,
            ast_position: None,
            ast_position_counter: 0,
            priority_counter: 0,
            should_traverse_functions,
            root,
            exception_handler: VecDeque::new(),
            finally_map: IndexMap::new(),
        }
    }
    // port: ControlFlowAnalysis#builder
    pub fn builder() -> Builder {
        Builder::new()
    }
    // port: ControlFlowAnalysis#getCfg
    pub fn get_cfg(&self) -> &ControlFlowGraph<NodeId> {
        &self.cfg
    }
    // port: ControlFlowAnalysis#computeCfg
    fn compute_cfg(&mut self, compiler: &mut AbstractCompiler, root: NodeId) {
        self.root = root;
        self.ast_position = Some(LinkedIdentityHashMap::default());
        self.ast_position_counter = 0;
        NodeTraversal::traverse(compiler, root, self);
        self.ast_position_counter += 1;
        self.ast_position
            .as_mut()
            .unwrap()
            .put(None, Some(self.ast_position_counter));
        self.priority_counter = 0;
        self.prioritize_from_entry_node(compiler, self.cfg.get_entry());
        if self.should_traverse_functions {
            for candidate in self.cfg.get_nodes() {
                if candidate
                    .get_value(&self.cfg)
                    .is_some_and(|n| n.is_function(compiler))
                {
                    self.prioritize_from_entry_node(compiler, candidate);
                }
            }
        }
        self.ast_position = None;
        for candidate in self.cfg.get_nodes() {
            if !candidate.has_priority(&self.cfg) {
                self.priority_counter += 1;
                candidate.set_priority(&mut self.cfg, self.priority_counter);
            }
        }
        self.priority_counter += 1;
        self.cfg
            .get_implicit_return()
            .set_priority(&mut self.cfg, self.priority_counter);
    }
    // port: ControlFlowAnalysis#priorityComparator
    // Comparator.comparingInt(digraphNode -> checkNotNull(astPosition.get(value), value)): Java's
    // PriorityQueue calls it only when it compares two queued nodes, so a CFG node without an AST
    // position (an expression-bodied arrow nested in the cfg root's body) fails only there.
    fn priority_comparator<'a>(
        compiler: &'a AbstractCompiler,
        cfg: &'a ControlFlowGraph<NodeId>,
        ast_position: &'a LinkedIdentityHashMap<Option<NodeId>, i32>,
    ) -> impl FnMut(&DiGraphNode, &DiGraphNode) -> std::cmp::Ordering + 'a {
        let key = move |digraph_node: &DiGraphNode| {
            *check_not_null!(
                ast_position.get(digraph_node.get_value(cfg)),
                "%s",
                cfg.format_node_value(compiler, digraph_node.get_value(cfg).as_ref())
            )
        };
        move |a, b| key(a).cmp(&key(b))
    }
    // port: ControlFlowAnalysis#prioritizeFromEntryNode
    fn prioritize_from_entry_node(&mut self, compiler: &AbstractCompiler, entry: DiGraphNode) {
        let mut worklist: PriorityQueue<DiGraphNode> = PriorityQueue::new(10);
        worklist.add(
            entry,
            &mut Self::priority_comparator(
                compiler,
                &self.cfg,
                self.ast_position.as_ref().unwrap(),
            ),
        );

        while !worklist.is_empty() {
            let current = worklist.remove(&mut Self::priority_comparator(
                compiler,
                &self.cfg,
                self.ast_position.as_ref().unwrap(),
            ));
            if current.has_priority(&self.cfg) {
                continue;
            }

            self.priority_counter += 1;
            current.set_priority(&mut self.cfg, self.priority_counter);

            let successors = self.cfg.get_directed_succ_nodes_of(current);
            worklist.add_all(
                successors,
                &mut Self::priority_comparator(
                    compiler,
                    &self.cfg,
                    self.ast_position.as_ref().unwrap(),
                ),
            );
        }
    }
    // port: ControlFlowAnalysis#shouldTraverseIntoChildren
    fn should_traverse_into_children(
        &mut self,
        compiler: &AbstractCompiler,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(compiler) {
            Token::FUNCTION => {
                if self.should_traverse_functions
                    || Some(n) == *self.cfg.get_entry().get_value(&self.cfg)
                {
                    self.exception_handler.push_front(n);
                    return true;
                }
                return false;
            }
            Token::TRY => {
                self.exception_handler.push_front(n);
                return true;
            }
            _ => {}
        }
        if let Some(parent) = parent {
            match parent.get_token(compiler) {
                Token::FOR | Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                    let traverse = Some(n) == parent.get_last_child(compiler);
                    if !traverse {
                        self.ast_position
                            .as_mut()
                            .unwrap()
                            .put(Some(n), Some(self.ast_position_counter));
                        self.ast_position_counter += 1;
                    }
                    return traverse;
                }
                Token::DO => return Some(n) != parent.get_second_child(compiler),
                Token::IF
                | Token::WHILE
                | Token::WITH
                | Token::SWITCH
                | Token::CASE
                | Token::CATCH
                | Token::LABEL => return Some(n) != parent.get_first_child(compiler),
                Token::FUNCTION => return Some(n) == parent.get_last_child(compiler),
                Token::CLASS => {
                    return self.should_traverse_functions
                        && Some(n) == parent.get_last_child(compiler);
                }
                Token::COMPUTED_PROP
                | Token::CONTINUE
                | Token::BREAK
                | Token::EXPR_RESULT
                | Token::VAR
                | Token::LET
                | Token::CONST
                | Token::EXPORT
                | Token::IMPORT
                | Token::RETURN
                | Token::THROW
                | Token::MEMBER_FUNCTION_DEF
                | Token::MEMBER_FIELD_DEF
                | Token::COMPUTED_FIELD_DEF => return false,
                Token::TRY => {
                    if (!NodeUtil::has_finally(compiler, parent)
                        && n == NodeUtil::get_catch_block(compiler, parent))
                        || NodeUtil::is_try_finally_node(compiler, parent, n)
                    {
                        check_state!(self.exception_handler.front() == Some(&parent));
                        self.exception_handler.pop_front();
                    }
                }
                _ => {}
            }
            if parent
                .get_parent(compiler)
                .is_some_and(|p| p.is_arrow_function(compiler))
                && !parent.is_block(compiler)
            {
                return false;
            }
        }
        true
    }
    // port: ControlFlowAnalysis#handleIf
    fn handle_if(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        let then_block = node.get_second_child(compiler).unwrap();
        let else_block = then_block.get_next(compiler);
        self.create_edge(
            node,
            Branch::ON_TRUE,
            Some(Self::compute_fall_through(compiler, then_block)),
        );
        let dest = if let Some(n) = else_block {
            Some(Self::compute_fall_through(compiler, n))
        } else {
            self.compute_follow_node_with_cfa(compiler, node)
        };
        self.create_edge(node, Branch::ON_FALSE, dest);
        self.connect_to_possible_exception_handler(
            compiler,
            node,
            NodeUtil::get_condition_expression(compiler, node).unwrap(),
        );
    }
    // port: ControlFlowAnalysis#handleWhile
    fn handle_while(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        let cond = node.get_first_child(compiler).unwrap();
        self.create_edge(
            node,
            Branch::ON_TRUE,
            Some(Self::compute_fall_through(
                compiler,
                cond.get_next(compiler).unwrap(),
            )),
        );
        if !cond.is_true(compiler) {
            let follow = self.compute_follow_node_with_cfa(compiler, node);
            self.create_edge(node, Branch::ON_FALSE, follow);
        }
        self.connect_to_possible_exception_handler(
            compiler,
            node,
            NodeUtil::get_condition_expression(compiler, node).unwrap(),
        );
    }
    // port: ControlFlowAnalysis#handleDo
    fn handle_do(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        let body = node.get_first_child(compiler).unwrap();
        self.create_edge(
            node,
            Branch::ON_TRUE,
            Some(Self::compute_fall_through(compiler, body)),
        );
        let cond = body.get_next(compiler).unwrap();
        if !cond.is_true(compiler) {
            let follow = self.compute_follow_node_with_cfa(compiler, node);
            self.create_edge(node, Branch::ON_FALSE, follow);
        }
        self.connect_to_possible_exception_handler(
            compiler,
            node,
            NodeUtil::get_condition_expression(compiler, node).unwrap(),
        );
    }
    // port: ControlFlowAnalysis#handleEnhancedFor
    fn handle_enhanced_for(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        let item = node.get_first_child(compiler).unwrap();
        let collection = item.get_next(compiler).unwrap();
        let body = collection.get_next(compiler).unwrap();
        self.create_edge(collection, Branch::UNCOND, Some(node));
        self.create_edge(
            node,
            Branch::ON_TRUE,
            Some(Self::compute_fall_through(compiler, body)),
        );
        let follow = self.compute_follow_node_with_cfa(compiler, node);
        self.create_edge(node, Branch::ON_FALSE, follow);
        if node.is_for_of(compiler) || node.is_for_await_of(compiler) {
            self.connect_to_possible_exception_handler_unconditionally(compiler, node);
        } else {
            self.connect_to_possible_exception_handler(compiler, node, collection);
        }
    }
    // port: ControlFlowAnalysis#handleFor
    fn handle_for(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        let init = node.get_first_child(compiler).unwrap();
        let cond = init.get_next(compiler).unwrap();
        let iter = cond.get_next(compiler).unwrap();
        let body = iter.get_next(compiler).unwrap();
        self.create_edge(init, Branch::UNCOND, Some(node));
        self.create_edge(
            node,
            Branch::ON_TRUE,
            Some(Self::compute_fall_through(compiler, body)),
        );
        if !cond.is_empty(compiler) && !cond.is_true(compiler) {
            let follow = self.compute_follow_node_with_cfa(compiler, node);
            self.create_edge(node, Branch::ON_FALSE, follow);
        }
        self.create_edge(iter, Branch::UNCOND, Some(node));
        self.connect_to_possible_exception_handler(compiler, init, init);
        self.connect_to_possible_exception_handler(compiler, node, cond);
        self.connect_to_possible_exception_handler(compiler, iter, iter);
    }
    // port: ControlFlowAnalysis#handleSwitch
    fn handle_switch(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        let body = node.get_last_child(compiler).unwrap();
        self.create_edge(node, Branch::UNCOND, Some(body));
        let next = Self::get_next_sibling_of_type(
            compiler,
            body.get_first_child(compiler),
            &[Token::CASE, Token::EMPTY],
        );
        let dest = if next.is_some() {
            next
        } else if body.has_children(compiler) {
            body.get_first_child(compiler)
        } else {
            self.compute_follow_node_with_cfa(compiler, node)
        };
        self.create_edge(body, Branch::UNCOND, dest);
        self.connect_to_possible_exception_handler(
            compiler,
            node,
            node.get_first_child(compiler).unwrap(),
        );
    }
    // port: ControlFlowAnalysis#handleCase
    fn handle_case(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        self.create_edge(node, Branch::ON_TRUE, node.get_second_child(compiler));
        let next =
            Self::get_next_sibling_of_type(compiler, node.get_next(compiler), &[Token::CASE]);
        let dest = if let Some(next) = next {
            check_state!(next.is_case(compiler));
            Some(next)
        } else {
            let body = node.get_parent(compiler).unwrap();
            let default = Self::get_next_sibling_of_type(
                compiler,
                body.get_first_child(compiler),
                &[Token::DEFAULT_CASE],
            );
            if default.is_some() {
                default
            } else {
                self.compute_follow_node_with_cfa(compiler, node)
            }
        };
        self.create_edge(node, Branch::ON_FALSE, dest);
        self.connect_to_possible_exception_handler(
            compiler,
            node,
            node.get_first_child(compiler).unwrap(),
        );
    }
    // port: ControlFlowAnalysis#handleDefault
    fn handle_default(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        self.create_edge(node, Branch::UNCOND, node.get_first_child(compiler));
    }
    // port: ControlFlowAnalysis#handleWith
    fn handle_with(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        self.create_edge(node, Branch::UNCOND, node.get_last_child(compiler));
        self.connect_to_possible_exception_handler(
            compiler,
            node,
            node.get_first_child(compiler).unwrap(),
        );
    }
    // port: ControlFlowAnalysis#handleStmtList
    fn handle_stmt_list(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        let parent = node.get_parent(compiler);
        if node.is_block(compiler)
            && parent.is_some_and(|p| {
                p.is_try(compiler) && NodeUtil::get_catch_block(compiler, p) == node
            })
            && !NodeUtil::has_catch_handler(compiler, node)
        {
            return;
        }
        let mut child = node.get_first_child(compiler);
        while child.is_some_and(|c| c.is_function(compiler)) {
            child = child.unwrap().get_next(compiler);
        }
        let dest = if let Some(c) = child {
            Some(Self::compute_fall_through(compiler, c))
        } else {
            self.compute_follow_node_with_cfa(compiler, node)
        };
        self.create_edge(node, Branch::UNCOND, dest);
        if let Some(p) = parent {
            match p.get_token(compiler) {
                Token::DEFAULT_CASE | Token::CASE | Token::TRY => {}
                Token::ROOT => {
                    if node.is_root(compiler) && node.get_next(compiler).is_some() {
                        self.create_edge(node, Branch::UNCOND, node.get_next(compiler));
                    }
                }
                _ => {
                    if node.is_block(compiler) && node.is_synthetic_block(compiler) {
                        let follow = self.compute_follow_node_with_cfa(compiler, node);
                        self.create_edge(node, Branch::SYN_BLOCK, follow);
                    }
                }
            }
        }
    }
    // port: ControlFlowAnalysis#handleFunction
    fn handle_function(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        check_state!(node.is_function(compiler));
        check_state!(node.has_x_children(compiler, 3));
        self.create_edge(
            node,
            Branch::UNCOND,
            Some(Self::compute_fall_through(
                compiler,
                node.get_last_child(compiler).unwrap(),
            )),
        );
        check_state!(self.exception_handler.front() == Some(&node));
        self.exception_handler.pop_front();
    }
    // port: ControlFlowAnalysis#handleExpr
    fn handle_expr(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        let follow = self.compute_follow_node_with_cfa(compiler, node);
        self.create_edge(node, Branch::UNCOND, follow);
        self.connect_to_possible_exception_handler(compiler, node, node);
    }
    // port: ControlFlowAnalysis#handleThrow
    fn handle_throw(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        self.connect_to_possible_exception_handler(compiler, node, node);
    }
    // port: ControlFlowAnalysis#handleTry
    fn handle_try(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        self.create_edge(node, Branch::UNCOND, node.get_first_child(compiler));
    }
    // port: ControlFlowAnalysis#handleCatch
    fn handle_catch(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        self.create_edge(node, Branch::UNCOND, node.get_last_child(compiler));
    }
    // port: ControlFlowAnalysis#handleBreak
    fn handle_break(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        let label = node
            .get_first_child(compiler)
            .map(|n| n.get_string(compiler));
        let mut cur = node;
        let mut previous = None;
        let mut last_jump = node;
        let mut parent = node.get_parent(compiler);
        while !Self::is_break_target(compiler, cur, label.as_ref()) {
            if cur.is_try(compiler)
                && NodeUtil::has_finally(compiler, cur)
                && cur.get_last_child(compiler) != previous
            {
                let dest = Some(Self::compute_fall_through(
                    compiler,
                    cur.get_last_child(compiler).unwrap(),
                ));
                if last_jump == node {
                    self.create_edge(last_jump, Branch::UNCOND, dest);
                } else {
                    self.finally_map.entry(last_jump).or_default().insert(dest);
                }
                last_jump = cur;
            }
            if parent.is_none() {
                if compiler.get_options().can_continue_after_errors() {
                    return;
                }
                panic!("Cannot find break target.");
            }
            previous = Some(cur);
            cur = parent.unwrap();
            parent = cur.get_parent(compiler);
        }
        let follow = self.compute_follow_node_with_cfa(compiler, cur);
        if last_jump == node {
            self.create_edge(last_jump, Branch::UNCOND, follow);
        } else {
            self.finally_map
                .entry(last_jump)
                .or_default()
                .insert(follow);
        }
    }
    // port: ControlFlowAnalysis#handleContinue
    fn handle_continue(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        let label = node
            .get_first_child(compiler)
            .map(|n| n.get_string(compiler));
        let mut cur = node;
        let mut previous = None;
        let mut last_jump = node;
        while !Self::is_continue_target(compiler, cur, label.as_ref()) {
            if cur.is_try(compiler)
                && NodeUtil::has_finally(compiler, cur)
                && cur.get_last_child(compiler) != previous
            {
                if last_jump == node {
                    self.create_edge(last_jump, Branch::UNCOND, cur.get_last_child(compiler));
                } else {
                    self.finally_map.entry(last_jump).or_default().insert(Some(
                        Self::compute_fall_through(compiler, cur.get_last_child(compiler).unwrap()),
                    ));
                }
                last_jump = cur;
            }
            check_state!(cur.has_parent(compiler), "Cannot find continue target.");
            previous = Some(cur);
            cur = cur.get_parent(compiler).unwrap();
        }
        let iter = if cur.is_vanilla_for(compiler) {
            cur.get_child_at_index(compiler, 2).unwrap()
        } else {
            cur
        };
        if last_jump == node {
            self.create_edge(node, Branch::UNCOND, Some(iter));
        } else {
            self.finally_map
                .entry(last_jump)
                .or_default()
                .insert(Some(iter));
        }
    }
    // port: ControlFlowAnalysis#handleReturn
    fn handle_return(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        let mut last_jump = None;
        for handler in self.exception_handler.clone() {
            if handler.is_function(compiler) {
                break;
            }
            if NodeUtil::has_finally(compiler, handler) {
                if let Some(last) = last_jump {
                    self.finally_map.entry(last).or_default().insert(Some(
                        Self::compute_fall_through(
                            compiler,
                            handler.get_last_child(compiler).unwrap(),
                        ),
                    ));
                } else {
                    self.create_edge(node, Branch::UNCOND, handler.get_last_child(compiler));
                }
                last_jump = Some(handler);
            }
        }
        if let Some(child) = node.get_first_child(compiler) {
            self.connect_to_possible_exception_handler(compiler, node, child);
        }
        if let Some(last) = last_jump {
            self.finally_map.entry(last).or_default().insert(None);
        } else {
            self.create_edge(node, Branch::UNCOND, None);
        }
    }
    // port: ControlFlowAnalysis#handleStmt
    fn handle_stmt(&mut self, compiler: &AbstractCompiler, node: NodeId) {
        let follow = self.compute_follow_node_with_cfa(compiler, node);
        self.create_edge(node, Branch::UNCOND, follow);
        self.connect_to_possible_exception_handler(compiler, node, node);
    }
    // port: ControlFlowAnalysis#computeFollowNode(Node, ControlFlowAnalysis)
    pub fn compute_follow_node_with_cfa(
        &mut self,
        compiler: &AbstractCompiler,
        node: NodeId,
    ) -> Option<NodeId> {
        Self::compute_follow_node_from(compiler, node, node, Some(self))
    }
    // port: ControlFlowAnalysis#computeFollowNode(Node)
    pub fn compute_follow_node(compiler: &AbstractCompiler, node: NodeId) -> Option<NodeId> {
        Self::compute_follow_node_from(compiler, node, node, None)
    }
    // port: ControlFlowAnalysis#computeFollowNode(Node, Node, ControlFlowAnalysis)
    fn compute_follow_node_from(
        compiler: &AbstractCompiler,
        from_node: NodeId,
        node: NodeId,
        mut cfa: Option<&mut Self>,
    ) -> Option<NodeId> {
        let parent = node.get_parent(compiler);
        if parent.is_none()
            || parent.is_some_and(|p| p.is_function(compiler))
            || cfa.as_ref().is_some_and(|c| node == c.root)
        {
            return None;
        }
        let parent = parent.unwrap();
        match parent.get_token(compiler) {
            Token::IF => return Self::compute_follow_node_from(compiler, from_node, parent, cfa),
            Token::CASE | Token::DEFAULT_CASE => {
                if let Some(next) = parent.get_next(compiler) {
                    if next.is_case(compiler) {
                        return next.get_second_child(compiler);
                    } else if next.is_default_case(compiler) {
                        return next.get_first_child(compiler);
                    } else {
                        panic!("Not reachable");
                    }
                } else {
                    return Self::compute_follow_node_from(compiler, from_node, parent, cfa);
                }
            }
            Token::FOR => {
                return parent
                    .get_second_child(compiler)
                    .unwrap()
                    .get_next(compiler);
            }
            Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF | Token::WHILE | Token::DO => {
                return Some(parent);
            }
            Token::TRY => {
                if parent.get_first_child(compiler) == Some(node) {
                    if NodeUtil::has_finally(compiler, parent) {
                        return Some(Self::compute_fall_through(
                            compiler,
                            parent.get_last_child(compiler).unwrap(),
                        ));
                    } else {
                        return Self::compute_follow_node_from(compiler, from_node, parent, cfa);
                    }
                } else if NodeUtil::get_catch_block(compiler, parent) == node {
                    if NodeUtil::has_finally(compiler, parent) {
                        return Some(Self::compute_fall_through(
                            compiler,
                            node.get_next(compiler).unwrap(),
                        ));
                    } else {
                        return Self::compute_follow_node_from(compiler, from_node, parent, cfa);
                    }
                } else if parent.get_last_child(compiler) == Some(node) {
                    if let Some(c) = cfa.as_mut() {
                        let finally_nodes = c.finally_map.get(&parent).cloned().unwrap_or_default();
                        for finally_node in finally_nodes {
                            c.create_edge(from_node, Branch::ON_EX, finally_node);
                        }
                    }
                    return Self::compute_follow_node_from(compiler, from_node, parent, cfa);
                }
                panic!("Unexpected TRY child {}", node.to_string(compiler));
            }
            _ => {}
        }
        let mut sibling = node.get_next(compiler);
        while sibling.is_some_and(|n| n.is_function(compiler)) {
            sibling = sibling.unwrap().get_next(compiler);
        }
        if let Some(next) = sibling {
            Some(Self::compute_fall_through(compiler, next))
        } else {
            Self::compute_follow_node_from(compiler, from_node, parent, cfa)
        }
    }
    // port: ControlFlowAnalysis#computeFallThrough
    pub fn compute_fall_through(compiler: &AbstractCompiler, n: NodeId) -> NodeId {
        match n.get_token(compiler) {
            Token::DO | Token::FOR => {
                Self::compute_fall_through(compiler, n.get_first_child(compiler).unwrap())
            }
            Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                n.get_second_child(compiler).unwrap()
            }
            Token::LABEL => {
                Self::compute_fall_through(compiler, n.get_last_child(compiler).unwrap())
            }
            _ => n,
        }
    }
    // port: ControlFlowAnalysis#createEdge
    fn create_edge(&mut self, from: NodeId, branch: Branch, to: Option<NodeId>) {
        self.cfg.create_node(Some(from));
        self.cfg.create_node(to);
        self.cfg.connect_if_not_found(Some(from), branch, to);
    }
    // port: ControlFlowAnalysis#connectToPossibleExceptionHandler
    fn connect_to_possible_exception_handler(
        &mut self,
        compiler: &AbstractCompiler,
        cfg_node: NodeId,
        target: NodeId,
    ) {
        if Self::may_throw_exception(compiler, target) {
            self.connect_to_possible_exception_handler_unconditionally(compiler, cfg_node);
        }
    }
    // port: ControlFlowAnalysis#connectToPossibleExceptionHandlerUnconditionally
    fn connect_to_possible_exception_handler_unconditionally(
        &mut self,
        compiler: &AbstractCompiler,
        cfg_node: NodeId,
    ) {
        if self.exception_handler.is_empty() {
            return;
        }
        let mut last_jump = cfg_node;
        for handler in self.exception_handler.clone() {
            if handler.is_function(compiler) {
                return;
            }
            check_state!(handler.is_try(compiler));
            let catch_block = NodeUtil::get_catch_block(compiler, handler);
            let mut last_jump_in_catch_block = false;
            for ancestor in last_jump.ancestors(compiler) {
                if ancestor == handler {
                    break;
                } else if ancestor == catch_block {
                    last_jump_in_catch_block = true;
                    break;
                }
            }
            if !NodeUtil::has_catch_handler(compiler, catch_block) || last_jump_in_catch_block {
                if last_jump == cfg_node {
                    self.create_edge(cfg_node, Branch::ON_EX, handler.get_last_child(compiler));
                } else {
                    self.finally_map
                        .entry(last_jump)
                        .or_default()
                        .insert(handler.get_last_child(compiler));
                }
            } else if last_jump == cfg_node {
                self.create_edge(cfg_node, Branch::ON_EX, Some(catch_block));
                return;
            } else {
                self.finally_map
                    .entry(last_jump)
                    .or_default()
                    .insert(Some(catch_block));
            }
            last_jump = handler;
        }
    }
    // port: ControlFlowAnalysis#getNextSiblingOfType
    fn get_next_sibling_of_type(
        compiler: &AbstractCompiler,
        mut first: Option<NodeId>,
        types: &[Token],
    ) -> Option<NodeId> {
        while let Some(c) = first {
            for &ty in types {
                if c.get_token(compiler) == ty {
                    return Some(c);
                }
            }
            first = c.get_next(compiler);
        }
        None
    }
    // port: ControlFlowAnalysis#isBreakTarget
    pub fn is_break_target(
        compiler: &AbstractCompiler,
        target: NodeId,
        label: Option<&JsString>,
    ) -> bool {
        Self::is_break_structure(compiler, target, label.is_some())
            && Self::match_label(compiler, target.get_parent(compiler), label)
    }
    // port: ControlFlowAnalysis#isContinueTarget
    pub fn is_continue_target(
        compiler: &AbstractCompiler,
        target: NodeId,
        label: Option<&JsString>,
    ) -> bool {
        NodeUtil::is_loop_structure(compiler, target)
            && Self::match_label(compiler, target.get_parent(compiler), label)
    }
    // port: ControlFlowAnalysis#matchLabel
    fn match_label(
        compiler: &AbstractCompiler,
        mut target: Option<NodeId>,
        label: Option<&JsString>,
    ) -> bool {
        let Some(label) = label else {
            return true;
        };
        while target.is_some_and(|n| n.is_label(compiler)) {
            let n = target.unwrap();
            if &n.get_first_child(compiler).unwrap().get_string(compiler) == label {
                return true;
            }
            target = n.get_parent(compiler);
        }
        false
    }
    // port: ControlFlowAnalysis#mayThrowException
    pub fn may_throw_exception(compiler: &AbstractCompiler, n: NodeId) -> bool {
        match n.get_token(compiler) {
            Token::CALL
            | Token::OPTCHAIN_CALL
            | Token::TAGGED_TEMPLATELIT
            | Token::GETPROP
            | Token::OPTCHAIN_GETPROP
            | Token::GETELEM
            | Token::OPTCHAIN_GETELEM
            | Token::THROW
            | Token::NEW
            | Token::ASSIGN
            | Token::INC
            | Token::DEC
            | Token::INSTANCEOF
            | Token::IN
            | Token::YIELD
            | Token::AWAIT => return true,
            Token::FUNCTION => return false,
            _ => {}
        }
        for c in n.children(compiler) {
            if !ControlFlowGraph::is_entering_new_cfg_node(compiler, c)
                && Self::may_throw_exception(compiler, c)
            {
                return true;
            }
        }
        false
    }
    // port: ControlFlowAnalysis#isBreakStructure
    pub fn is_break_structure(compiler: &AbstractCompiler, n: NodeId, labeled: bool) -> bool {
        match n.get_token(compiler) {
            Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::DO
            | Token::WHILE
            | Token::SWITCH => true,
            Token::BLOCK | Token::IF | Token::TRY | Token::BREAK => labeled,
            _ => false,
        }
    }
    // port: ControlFlowAnalysis#getExceptionHandler
    pub fn get_exception_handler(compiler: &AbstractCompiler, mut n: NodeId) -> Option<NodeId> {
        while !n.is_script(compiler) && !n.is_function(compiler) {
            let catch = Self::get_catch_handler_for_block(compiler, n);
            if catch.is_some() {
                return catch;
            }
            n = n.get_parent(compiler).unwrap();
        }
        None
    }
    // port: ControlFlowAnalysis#getCatchHandlerForBlock
    pub fn get_catch_handler_for_block(
        compiler: &AbstractCompiler,
        block: NodeId,
    ) -> Option<NodeId> {
        if block.is_block(compiler)
            && block
                .get_parent(compiler)
                .is_some_and(|p| p.is_try(compiler) && p.get_first_child(compiler) == Some(block))
        {
            let mut sibling = block.get_next(compiler);
            while let Some(s) = sibling {
                if NodeUtil::has_catch_handler(compiler, s) {
                    return s.get_first_child(compiler);
                }
                sibling = s.get_next(compiler);
            }
        }
        None
    }
}
impl Callback for ControlFlowAnalysis {
    // port: ControlFlowAnalysis#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if self.should_traverse_into_children(t.get_compiler(), n, parent) {
            self.ast_position
                .as_mut()
                .unwrap()
                .put(Some(n), Some(self.ast_position_counter));
            self.ast_position_counter += 1;
            true
        } else {
            false
        }
    }
    // port: ControlFlowAnalysis#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let compiler = t.get_compiler();
        match n.get_token(compiler) {
            Token::IF => self.handle_if(compiler, n),
            Token::WHILE => self.handle_while(compiler, n),
            Token::DO => self.handle_do(compiler, n),
            Token::FOR => self.handle_for(compiler, n),
            Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                self.handle_enhanced_for(compiler, n)
            }
            Token::SWITCH => self.handle_switch(compiler, n),
            Token::CASE => self.handle_case(compiler, n),
            Token::DEFAULT_CASE => self.handle_default(compiler, n),
            Token::BLOCK | Token::ROOT | Token::SCRIPT | Token::MODULE_BODY => {
                self.handle_stmt_list(compiler, n)
            }
            Token::FUNCTION => self.handle_function(compiler, n),
            Token::EXPR_RESULT => self.handle_expr(compiler, n),
            Token::THROW => self.handle_throw(compiler, n),
            Token::TRY => self.handle_try(compiler, n),
            Token::CATCH => self.handle_catch(compiler, n),
            Token::BREAK => self.handle_break(compiler, n),
            Token::CONTINUE => self.handle_continue(compiler, n),
            Token::RETURN => self.handle_return(compiler, n),
            Token::WITH => self.handle_with(compiler, n),
            Token::SWITCH_BODY
            | Token::LABEL
            | Token::CLASS_MEMBERS
            | Token::MEMBER_FUNCTION_DEF
            | Token::MEMBER_FIELD_DEF
            | Token::COMPUTED_FIELD_DEF => {}
            _ => self.handle_stmt(compiler, n),
        }
    }
}

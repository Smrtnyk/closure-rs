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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
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
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::java_util::priority_queue::PriorityQueue;
use closure_rhino::{
    check_argument, check_not_null, check_state,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use std::collections::VecDeque;

pub(crate) type LinearScripts = IndexMap<NodeId, Vec<NodeId>>;

/// Rust-only (performance divergence): per CFG root, the sizes of the graph computed last for
/// it, on any thread: its nodes, edges and recorded AST positions. The next graph of that root
/// reserves them (the passes compute the graph of a function again in every round of the
/// optimizer loop, and the tables of a large graph would otherwise be copied each time they
/// double). Only capacity depends on it; a stale or foreign entry (another compilation in this
/// process) only sizes the tables differently.
struct LastGraphSizes;

type GraphSizes = IndexMap<NodeId, (u32, u32, u32)>;

static LAST_GRAPH_SIZES: std::sync::LazyLock<std::sync::Mutex<GraphSizes>> =
    std::sync::LazyLock::new(Default::default);

impl LastGraphSizes {
    /// Entries kept before the table starts over (a long-lived process compiles many programs).
    const MAX_ROOTS: usize = 1 << 16;

    fn table() -> std::sync::MutexGuard<'static, GraphSizes> {
        LAST_GRAPH_SIZES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The capacities for a graph of `root`: an eighth more than the last sizes (a graph that
    /// grew a little would otherwise copy its full tables once), or room for a small function.
    fn of(root: NodeId) -> (usize, usize, usize) {
        let last = Self::table().get(&root).copied();
        let more = |n: u32| n as usize + n as usize / 8;
        last.map_or((16, 16, 0), |(nodes, edges, positions)| {
            (more(nodes), more(edges), more(positions))
        })
    }

    fn record(root: NodeId, nodes: usize, edges: usize, positions: usize) {
        let narrow = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        let mut sizes = Self::table();
        if sizes.len() >= Self::MAX_ROOTS && !sizes.contains_key(&root) {
            sizes.clear();
        }
        sizes.insert(root, (narrow(nodes), narrow(edges), narrow(positions)));
    }
}

pub struct ControlFlowAnalysis {
    cfg: ControlFlowGraph<NodeId>,
    /// Java's `astPosition` map (`LinkedIdentityHashMap<Node, Integer>`), as the log of its puts
    /// in order; the implicit return's (null) position is `ast_position_counter` once the
    /// traversal is done.
    ///
    /// Rust-only (performance divergence): Java looks positions up in the map whenever its
    /// priority queue compares two CFG nodes. Only CFG nodes are ever looked up, and only after
    /// every position is recorded, so the puts are logged and replayed once, in order (a later
    /// put of a node wins, as in the map), into a table indexed by CFG node
    /// (`cfg_node_positions`).
    ast_position: Vec<(NodeId, i32)>,
    ast_position_counter: i32,
    priority_counter: i32,
    should_traverse_functions: bool,
    can_continue_after_errors: bool,
    root: NodeId,
    linear_scripts: Option<Box<LinearScripts>>,
    exception_handler: VecDeque<NodeId>,
    // Guava LinkedHashMultimap: keys and each key's values in insertion order.
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
        ControlFlowAnalysis::compute_cfg_readonly(
            &compiler.ast,
            compiler.get_options().can_continue_after_errors(),
            root,
            self.should_traverse_functions,
            self.edge_annotations,
        )
    }
}
impl ControlFlowAnalysis {
    // port: ControlFlowAnalysis#ControlFlowAnalysis
    // port: ControlFlowAnalysis#computeCfg
    fn new(
        ast: &Ast,
        can_continue_after_errors: bool,
        root: NodeId,
        should_traverse_functions: bool,
        edge_annotations: bool,
    ) -> Self {
        // The owning Rust CFG is allocated here; Java validates the root before that allocation.
        check_argument!(
            NodeUtil::is_valid_cfg_root(ast, root),
            "Unexpected control flow graph root %s",
            root.to_string(ast)
        );
        let (nodes, edges, positions) = LastGraphSizes::of(root);
        let cfg = ControlFlowGraph::new_ast(
            Self::compute_fall_through(ast, root),
            edge_annotations,
            nodes,
            edges,
        );
        Self {
            cfg,
            ast_position: Vec::with_capacity(positions),
            ast_position_counter: 0,
            priority_counter: 0,
            should_traverse_functions,
            can_continue_after_errors,
            root,
            linear_scripts: None,
            exception_handler: VecDeque::new(),
            finally_map: IndexMap::<_, _>::default(),
        }
    }
    /// Rust-only (performance divergence): computes a CFG read-only against `&Ast` using a direct
    /// traversal instead of allocating `NodeTraversal`, safe to execute concurrently across worker
    /// threads.
    pub(crate) fn compute_cfg_readonly(
        ast: &Ast,
        can_continue_after_errors: bool,
        root: NodeId,
        should_traverse_functions: bool,
        edge_annotations: bool,
    ) -> ControlFlowGraph<NodeId> {
        let mut cfa = Self::new(
            ast,
            can_continue_after_errors,
            root,
            should_traverse_functions,
            edge_annotations,
        );
        cfa.compute_cfg_ast(ast, root);
        cfa.cfg
    }
    // port: ControlFlowAnalysis#builder
    pub fn builder() -> Builder {
        Builder::new()
    }
    /// A function whose CFG visits the function, its body and the listed statements once,
    /// in source order, before the implicit return. Capture node identities before entry
    /// inference, just as building the full CFG would.
    pub(crate) fn linear_function(
        compiler: &Ast,
        function: NodeId,
    ) -> Option<(NodeId, Vec<NodeId>)> {
        if !function.is_function(compiler) {
            return None;
        }
        let name = function.get_first_child(compiler)?;
        let params = name.get_next(compiler)?;
        let body = params.get_next(compiler)?;
        if !name.is_name(compiler)
            || !params.is_param_list(compiler)
            || body.get_next(compiler).is_some()
            || !body.is_block(compiler)
            || body.is_synthetic_block(compiler)
        {
            return None;
        }

        let mut statements = Vec::new();
        for node in body.children(compiler) {
            match node.get_token(compiler) {
                // CFA skips nested functions, including when looking for the next statement.
                Token::FUNCTION => continue,
                Token::EXPR_RESULT | Token::VAR | Token::LET | Token::CONST => {}
                Token::RETURN if node.get_next(compiler).is_none() => {}
                _ => return None,
            }
            // These statement kinds are opaque to CFA. A TRY, even in a malformed
            // expression, is considered by shouldTraverseIntoChildren before its parent.
            if node.children(compiler).any(|child| child.is_try(compiler)) {
                return None;
            }
            statements.push(node);
        }
        Some((body, statements))
    }

    /// In the global graph, contract each straight-line script before the first control-flow
    /// statement into one vertex. No statement in the prefix can be reached a second time.
    /// The separate edge from the externs root to the sources root still bypasses externs.
    pub(crate) fn global_inference_cfg(
        compiler: &mut AbstractCompiler,
        root: NodeId,
    ) -> (ControlFlowGraph<NodeId>, Box<LinearScripts>) {
        let mut scripts = IndexMap::default();
        'prefix: for section in root.children(compiler) {
            if !section.is_root(compiler) {
                break;
            }
            for script in section.children(compiler) {
                if !script.is_script(compiler) {
                    break 'prefix;
                }
                let mut statements = Vec::new();
                for statement in script.children(compiler) {
                    match statement.get_token(compiler) {
                        // CFA skips function declarations when computing fall-through.
                        Token::FUNCTION => continue,
                        Token::EXPR_RESULT | Token::VAR | Token::LET | Token::CONST
                            if !statement
                                .children(compiler)
                                .any(|child| child.is_try(compiler)) => {}
                        Token::EMPTY | Token::DEBUGGER if !statement.has_children(compiler) => {}
                        _ => break 'prefix,
                    }
                    statements.push(statement);
                }
                if !statements.is_empty() {
                    scripts.insert(script, statements);
                }
            }
        }

        let mut cfa = Self::new(
            &compiler.ast,
            compiler.get_options().can_continue_after_errors(),
            root,
            false,
            true,
        );
        cfa.linear_scripts = Some(Box::new(scripts));
        cfa.compute_cfg_ast(&compiler.ast, root);
        (cfa.cfg, cfa.linear_scripts.unwrap())
    }

    // port: ControlFlowAnalysis#getCfg
    pub fn get_cfg(&self) -> &ControlFlowGraph<NodeId> {
        &self.cfg
    }
    // port: ControlFlowAnalysis#computeCfg
    fn compute_cfg_ast(&mut self, ast: &Ast, root: NodeId) {
        self.root = root;
        self.ast_position.clear();
        self.ast_position_counter = 0;
        self.traverse_ast(ast, root, None);
        let recorded_positions = self.ast_position.len();
        self.ast_position_counter += 1;
        // astPosition.put(null, ++astPositionCounter)
        let positions = self.cfg_node_positions();
        self.priority_counter = 0;
        self.prioritize_from_entry_node(ast, self.cfg.get_entry(), &positions);
        if self.should_traverse_functions {
            for candidate in self.cfg.get_nodes() {
                if candidate
                    .get_value(&self.cfg)
                    .is_some_and(|n| n.is_function(ast))
                {
                    self.prioritize_from_entry_node(ast, candidate, &positions);
                }
            }
        }
        self.ast_position = Vec::new();
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
        LastGraphSizes::record(
            root,
            self.cfg.graph.get_node_count(),
            self.cfg.graph.edge_arena_len(),
            recorded_positions,
        );
    }

    /// Rust-only (performance divergence): the AST position of each CFG node's value, indexed by
    /// CFG node (see `ast_position`); `None` where the map has no entry.
    fn cfg_node_positions(&self) -> Vec<Option<i32>> {
        let mut positions = vec![None; self.cfg.get_node_count()];
        for &(n, position) in &self.ast_position {
            if let Some(node) = self.cfg.get_node(&Some(n)) {
                positions[node.0] = Some(position);
            }
        }
        if let Some(node) = self.cfg.get_node(&None) {
            positions[node.0] = Some(self.ast_position_counter);
        }
        positions
    }

    /// Direct read-only AST walk matching `NodeTraversal::traverse` order (including `CLASS`
    /// extends-before-name-before-body and `SCRIPT` linear-script short-circuit).
    fn traverse_ast(&mut self, ast: &Ast, n: NodeId, parent: Option<NodeId>) {
        match n.get_token(ast) {
            Token::SCRIPT => {
                if !self.should_traverse_node(ast, n, parent) {
                    return;
                }
                if self
                    .linear_scripts
                    .as_ref()
                    .is_some_and(|scripts| scripts.contains_key(&n))
                {
                    let follow = self.compute_follow_node_with_cfa(ast, n);
                    self.create_edge(n, Branch::UNCOND, follow);
                    return;
                }
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    let next = c.get_next(ast);
                    self.traverse_ast(ast, c, Some(n));
                    child = next;
                }
                self.visit_node(ast, n);
            }
            Token::CLASS => {
                if !self.should_traverse_node(ast, n, parent) {
                    return;
                }
                let class_name = n.get_first_child(ast).unwrap();
                let extends_clause = class_name.get_next(ast).unwrap();
                let body = extends_clause.get_next(ast).unwrap();
                self.traverse_ast(ast, extends_clause, Some(n));
                self.traverse_ast(ast, class_name, Some(n));
                self.traverse_ast(ast, body, Some(n));
                self.visit_node(ast, n);
            }
            _ => {
                if !self.should_traverse_node(ast, n, parent) {
                    return;
                }
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    let next = c.get_next(ast);
                    self.traverse_ast(ast, c, Some(n));
                    child = next;
                }
                self.visit_node(ast, n);
            }
        }
    }

    fn should_traverse_node(&mut self, ast: &Ast, n: NodeId, parent: Option<NodeId>) -> bool {
        if self.should_traverse_into_children(ast, n, parent) {
            self.ast_position.push((n, self.ast_position_counter));
            self.ast_position_counter += 1;
            true
        } else {
            false
        }
    }

    fn visit_node(&mut self, ast: &Ast, n: NodeId) {
        match n.get_token(ast) {
            Token::IF => self.handle_if(ast, n),
            Token::WHILE => self.handle_while(ast, n),
            Token::DO => self.handle_do(ast, n),
            Token::FOR => self.handle_for(ast, n),
            Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => self.handle_enhanced_for(ast, n),
            Token::SWITCH => self.handle_switch(ast, n),
            Token::CASE => self.handle_case(ast, n),
            Token::DEFAULT_CASE => self.handle_default(ast, n),
            Token::BLOCK | Token::ROOT | Token::SCRIPT | Token::MODULE_BODY => {
                self.handle_stmt_list(ast, n)
            }
            Token::FUNCTION => self.handle_function(ast, n),
            Token::EXPR_RESULT => self.handle_expr(ast, n),
            Token::THROW => self.handle_throw(ast, n),
            Token::TRY => self.handle_try(ast, n),
            Token::CATCH => self.handle_catch(ast, n),
            Token::BREAK => self.handle_break(ast, n),
            Token::CONTINUE => self.handle_continue(ast, n),
            Token::RETURN => self.handle_return(ast, n),
            Token::WITH => self.handle_with(ast, n),
            Token::SWITCH_BODY
            | Token::LABEL
            | Token::CLASS_MEMBERS
            | Token::MEMBER_FUNCTION_DEF
            | Token::MEMBER_FIELD_DEF
            | Token::COMPUTED_FIELD_DEF => {}
            _ => self.handle_stmt(ast, n),
        }
    }

    // port: ControlFlowAnalysis#priorityComparator
    // Comparator.comparingInt(digraphNode -> checkNotNull(astPosition.get(value), value)): Java's
    // PriorityQueue calls it only when it compares two queued nodes, so a CFG node without an AST
    // position (an expression-bodied arrow nested in the cfg root's body) fails only there.
    fn priority_comparator<'a>(
        ast: &'a Ast,
        cfg: &'a ControlFlowGraph<NodeId>,
        positions: &'a [Option<i32>],
    ) -> impl FnMut(&DiGraphNode, &DiGraphNode) -> std::cmp::Ordering + 'a {
        let key = move |digraph_node: &DiGraphNode| {
            check_not_null!(
                positions[digraph_node.0],
                "%s",
                digraph_node
                    .get_value(cfg)
                    .map_or_else(|| "null".to_string(), |n| n.to_string(ast))
            )
        };
        move |a, b| key(a).cmp(&key(b))
    }
    // port: ControlFlowAnalysis#prioritizeFromEntryNode
    fn prioritize_from_entry_node(
        &mut self,
        ast: &Ast,
        entry: DiGraphNode,
        positions: &[Option<i32>],
    ) {
        let mut worklist: PriorityQueue<DiGraphNode> = PriorityQueue::new(10);
        worklist.add(
            entry,
            &mut Self::priority_comparator(ast, &self.cfg, positions),
        );

        while !worklist.is_empty() {
            let current =
                worklist.remove(&mut Self::priority_comparator(ast, &self.cfg, positions));
            if current.has_priority(&self.cfg) {
                continue;
            }

            self.priority_counter += 1;
            current.set_priority(&mut self.cfg, self.priority_counter);

            // Rust-only (performance divergence): the successors are read from the graph in place
            // instead of being copied into a new list.
            let cfg = &self.cfg;
            worklist.add_all(
                cfg.node_out_edges(current)
                    .iter()
                    .map(|edge| cfg.edge_node_b(*edge)),
                &mut Self::priority_comparator(ast, cfg, positions),
            );
        }
    }
    // port: ControlFlowAnalysis#shouldTraverseIntoChildren
    fn should_traverse_into_children(
        &mut self,
        ast: &Ast,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        let compiler = ast;
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
                        self.ast_position.push((n, self.ast_position_counter));
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
    fn handle_if(&mut self, compiler: &Ast, node: NodeId) {
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
    fn handle_while(&mut self, compiler: &Ast, node: NodeId) {
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
    fn handle_do(&mut self, compiler: &Ast, node: NodeId) {
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
    fn handle_enhanced_for(&mut self, compiler: &Ast, node: NodeId) {
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
    fn handle_for(&mut self, compiler: &Ast, node: NodeId) {
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
    fn handle_switch(&mut self, compiler: &Ast, node: NodeId) {
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
    fn handle_case(&mut self, compiler: &Ast, node: NodeId) {
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
    fn handle_default(&mut self, compiler: &Ast, node: NodeId) {
        self.create_edge(node, Branch::UNCOND, node.get_first_child(compiler));
    }
    // port: ControlFlowAnalysis#handleWith
    fn handle_with(&mut self, compiler: &Ast, node: NodeId) {
        self.create_edge(node, Branch::UNCOND, node.get_last_child(compiler));
        self.connect_to_possible_exception_handler(
            compiler,
            node,
            node.get_first_child(compiler).unwrap(),
        );
    }
    // port: ControlFlowAnalysis#handleStmtList
    fn handle_stmt_list(&mut self, compiler: &Ast, node: NodeId) {
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
    fn handle_function(&mut self, compiler: &Ast, node: NodeId) {
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
    fn handle_expr(&mut self, compiler: &Ast, node: NodeId) {
        let follow = self.compute_follow_node_with_cfa(compiler, node);
        self.create_edge(node, Branch::UNCOND, follow);
        self.connect_to_possible_exception_handler(compiler, node, node);
    }
    // port: ControlFlowAnalysis#handleThrow
    fn handle_throw(&mut self, compiler: &Ast, node: NodeId) {
        self.connect_to_possible_exception_handler(compiler, node, node);
    }
    // port: ControlFlowAnalysis#handleTry
    fn handle_try(&mut self, compiler: &Ast, node: NodeId) {
        self.create_edge(node, Branch::UNCOND, node.get_first_child(compiler));
    }
    // port: ControlFlowAnalysis#handleCatch
    fn handle_catch(&mut self, compiler: &Ast, node: NodeId) {
        self.create_edge(node, Branch::UNCOND, node.get_last_child(compiler));
    }
    // port: ControlFlowAnalysis#handleBreak
    fn handle_break(&mut self, compiler: &Ast, node: NodeId) {
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
                if self.can_continue_after_errors {
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
    fn handle_continue(&mut self, compiler: &Ast, node: NodeId) {
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
    fn handle_return(&mut self, compiler: &Ast, node: NodeId) {
        let mut last_jump = None;
        for i in 0..self.exception_handler.len() {
            let handler = self.exception_handler[i];
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
    fn handle_stmt(&mut self, compiler: &Ast, node: NodeId) {
        let follow = self.compute_follow_node_with_cfa(compiler, node);
        self.create_edge(node, Branch::UNCOND, follow);
        self.connect_to_possible_exception_handler(compiler, node, node);
    }
    // port: ControlFlowAnalysis#computeFollowNode(Node, ControlFlowAnalysis)
    pub fn compute_follow_node_with_cfa(&mut self, compiler: &Ast, node: NodeId) -> Option<NodeId> {
        Self::compute_follow_node_from(compiler, node, node, Some(self))
    }
    // port: ControlFlowAnalysis#computeFollowNode(Node)
    pub fn compute_follow_node(compiler: &Ast, node: NodeId) -> Option<NodeId> {
        Self::compute_follow_node_from(compiler, node, node, None)
    }
    // port: ControlFlowAnalysis#computeFollowNode(Node, Node, ControlFlowAnalysis)
    fn compute_follow_node_from(
        compiler: &Ast,
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
    pub fn compute_fall_through(compiler: &Ast, n: NodeId) -> NodeId {
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
        let graph = &mut self.cfg.graph;
        let from = graph.create_node(Some(from));
        let to = graph.create_node(to);
        // Graph#connectIfNotFound checks both directions for this edge value.
        if !graph.is_connected_in_direction_nodes(from, |edge| *edge == branch, to)
            && !graph.is_connected_in_direction_nodes(to, |edge| *edge == branch, from)
        {
            graph.connect_nodes(from, branch, to);
        }
    }
    // port: ControlFlowAnalysis#connectToPossibleExceptionHandler
    fn connect_to_possible_exception_handler(
        &mut self,
        compiler: &Ast,
        cfg_node: NodeId,
        target: NodeId,
    ) {
        // Rust-only (performance divergence): outside a TRY the innermost handler is the
        // function (or there is none), where connecting stops at once; Java asks the pure
        // mayThrowException first.
        if self
            .exception_handler
            .front()
            .is_none_or(|handler| handler.is_function(compiler))
        {
            return;
        }
        if Self::may_throw_exception(compiler, target) {
            self.connect_to_possible_exception_handler_unconditionally(compiler, cfg_node);
        }
    }
    // port: ControlFlowAnalysis#connectToPossibleExceptionHandlerUnconditionally
    fn connect_to_possible_exception_handler_unconditionally(
        &mut self,
        compiler: &Ast,
        cfg_node: NodeId,
    ) {
        let mut last_jump = cfg_node;
        for i in 0..self.exception_handler.len() {
            let handler = self.exception_handler[i];
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
        compiler: &Ast,
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
    pub fn is_break_target(compiler: &Ast, target: NodeId, label: Option<&JsString>) -> bool {
        Self::is_break_structure(compiler, target, label.is_some())
            && Self::match_label(compiler, target.get_parent(compiler), label)
    }
    // port: ControlFlowAnalysis#isContinueTarget
    pub fn is_continue_target(compiler: &Ast, target: NodeId, label: Option<&JsString>) -> bool {
        NodeUtil::is_loop_structure(compiler, target)
            && Self::match_label(compiler, target.get_parent(compiler), label)
    }
    // port: ControlFlowAnalysis#matchLabel
    fn match_label(compiler: &Ast, mut target: Option<NodeId>, label: Option<&JsString>) -> bool {
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
    pub fn may_throw_exception(compiler: &Ast, n: NodeId) -> bool {
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
    pub fn is_break_structure(compiler: &Ast, n: NodeId, labeled: bool) -> bool {
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
    pub fn get_exception_handler(compiler: &Ast, mut n: NodeId) -> Option<NodeId> {
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
    pub fn get_catch_handler_for_block(compiler: &Ast, block: NodeId) -> Option<NodeId> {
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
        self.should_traverse_node(&t.get_compiler().ast, n, parent)
    }
    fn should_traverse_script(
        &mut self,
        t: &mut NodeTraversal<'_>,
        script: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if !self.should_traverse(t, script, parent) {
            return false;
        }
        if self
            .linear_scripts
            .as_ref()
            .is_some_and(|scripts| scripts.contains_key(&script))
        {
            let follow = self.compute_follow_node_with_cfa(&t.get_compiler().ast, script);
            self.create_edge(script, Branch::UNCOND, follow);
            return false;
        }
        true
    }
    // port: ControlFlowAnalysis#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        self.visit_node(&t.get_compiler().ast, n);
    }
}

#[cfg(test)]
mod linear_function_tests {
    use super::*;
    use crate::{Compiler, graph::graph::GraphEdge};
    use closure_rhino::ir::IR;

    #[test]
    fn linear_scripts_keep_the_extern_bypass_and_stop_at_the_first_branch() {
        let mut compiler = Compiler::new();
        let ext = compiler.parse_test_code("var x = 1; function hoisted() {} x = {};");
        let externs = IR::root(&mut compiler, &[ext]);
        let sources = compiler.parse_test_code_many(vec![
            "var y = x;".into(),
            "if (y) y = 2;".into(),
            "y = 'later';".into(),
        ]);
        let root = IR::root(&mut compiler, &[externs, sources]);
        let (compact, scripts) = ControlFlowAnalysis::global_inference_cfg(&mut compiler, root);
        let first_source = sources.get_first_child(&compiler).unwrap();
        assert_eq!(
            scripts.keys().copied().collect::<Vec<_>>(),
            [ext, first_source]
        );
        assert_eq!(
            scripts[&ext],
            [
                ext.get_first_child(&compiler).unwrap(),
                ext.get_last_child(&compiler).unwrap()
            ]
        );
        for statements in scripts.values() {
            for n in statements {
                assert!(compact.get_node(&Some(*n)).is_none());
            }
        }
        let full = ControlFlowAnalysis::builder()
            .set_cfg_root(root)
            .compute_cfg(&mut compiler);
        let predecessors = |cfg: &ControlFlowGraph<NodeId>| {
            cfg.get_directed_pred_nodes(&Some(sources))
                .into_iter()
                .map(|node| node.get_value(cfg).unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            predecessors(&full),
            [ext.get_last_child(&compiler).unwrap(), externs]
        );
        assert_eq!(predecessors(&compact), [ext, externs]);
        let last = sources
            .get_last_child(&compiler)
            .unwrap()
            .get_only_child(&compiler);
        assert!(compact.get_node(&Some(last)).is_some());
    }

    #[test]
    fn linear_function_nodes_follow_the_original_cfg() {
        let mut compiler = Compiler::new();
        let script = compiler.parse_test_code(
            "function f(x = 1) { function g() { throw x; } var y; let z = g; x && g(); return z; }",
        );
        let function = script.get_only_child(&compiler);
        let (body, statements) = ControlFlowAnalysis::linear_function(&compiler, function).unwrap();
        let cfg = ControlFlowAnalysis::builder()
            .set_cfg_root(function)
            .compute_cfg(&mut compiler);
        let mut previous = cfg.get_entry();
        for next in [body].into_iter().chain(statements).map(Some).chain([None]) {
            let edges = cfg.node_out_edges(previous);
            assert_eq!(edges.len(), 1);
            assert_eq!(*edges[0].get_value(&cfg), Branch::UNCOND);
            previous = cfg.edge_node_b(edges[0]);
            assert_eq!(*previous.get_value(&cfg), next);
        }
        assert_eq!(previous, cfg.get_implicit_return());
        body.set_is_synthetic_block(&mut compiler, true);
        assert!(ControlFlowAnalysis::linear_function(&compiler, function).is_none());

        for code in [
            "function f() { if (x) y(); }",
            "function f() { return; x(); }",
            "function f() { try { x(); } catch(e) {} }",
        ] {
            let script = compiler.parse_test_code(code);
            assert!(
                ControlFlowAnalysis::linear_function(&compiler, script.get_only_child(&compiler))
                    .is_none()
            );
        }
    }
}

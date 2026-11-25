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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/DeadAssignmentsElimination.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `DeadAssignmentsElimination.java`.
//!
//! Removes local variable assignments that are useless based on information from
//! `LiveVariablesAnalysis`. If there is an assignment to variable `x` and `x` is dead after this
//! assignment, we know that the current content of `x` will not be read and this assignment is
//! useless.
use crate::{
    AbstractCompiler,
    compiler_pass::CompilerPass,
    control_flow_analysis::ControlFlowAnalysis,
    control_flow_graph::ControlFlowGraph,
    data_flow_analysis::{DataFlowAnalysis, LinearFlowState},
    graph::{adjacency_graph::AdjacencyGraph, graph_node::GraphNode},
    live_variables_analysis::{
        LiveVariableLattice, LiveVariablesAnalysis, MAX_VARIABLES_TO_ANALYZE, PreparedLiveness,
    },
    node_traversal::{Callback, NodeTraversal, ScopedCallback},
    node_util::NodeUtil,
    syntactic_scope_creator::{SyntacticScopeCreator, ensure_ancestor_scope},
    var::VarId,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    check_argument, check_not_null, check_state, ir::IR, js_string::JsString, node::NodeId,
    token::Token,
};
use std::collections::VecDeque;

struct BailoutInformation {
    contains_function: bool,
    contains_removable_assign: bool,
}

pub struct DeadAssignmentsElimination {
    liveness: Option<LiveVariablesAnalysis>,
    function_stack: VecDeque<BailoutInformation>,
    // Rust-only: the cfg-root stack of the NodeTraversal.AbstractCfgCallback superclass
    // (`cfgs`). Java caches the computed graph in that stack; this pass asks for the graph once
    // per function block scope and hands it to LiveVariablesAnalysis, which owns it in Rust.
    cfgs: VecDeque<NodeId>,
}

impl Default for DeadAssignmentsElimination {
    fn default() -> Self {
        Self::new()
    }
}

impl DeadAssignmentsElimination {
    // port: DeadAssignmentsElimination#DeadAssignmentsElimination
    pub fn new() -> Self {
        Self {
            liveness: None,
            function_stack: VecDeque::new(),
            cfgs: VecDeque::new(),
        }
    }

    // port: NodeTraversal.AbstractCfgCallback#getControlFlowGraph
    fn get_control_flow_graph(
        &mut self,
        compiler: &mut AbstractCompiler,
    ) -> ControlFlowGraph<NodeId> {
        let cfg_root = self.cfgs.front().copied();
        check_state!(cfg_root.is_some());
        ControlFlowAnalysis::builder()
            .set_compiler(compiler)
            .set_cfg_root(cfg_root.unwrap())
            .set_include_edge_annotations(true)
            .compute_cfg(compiler)
    }

    // port: DeadAssignmentsElimination#enterScopeWithCfg
    fn enter_scope_with_cfg(&mut self, t: &mut NodeTraversal<'_>) {
        if t.in_function_block_scope() {
            self.function_stack.push_front(BailoutInformation {
                contains_function: false,
                contains_removable_assign: false,
            });
        }
    }

    // port: DeadAssignmentsElimination#exitScopeWithCfg
    fn exit_scope_with_cfg(&mut self, t: &mut NodeTraversal<'_>) {
        if t.in_function_block_scope() {
            self.eliminate_dead_assignments(t);
            self.function_stack.pop_front();
        }
    }

    // port: DeadAssignmentsElimination#eliminateDeadAssignments
    fn eliminate_dead_assignments(&mut self, t: &mut NodeTraversal<'_>) {
        check_argument!(t.in_function_block_scope());
        check_state!(!self.function_stack.is_empty());

        // Skip unchanged functions (note that the scope root is the function block, not the
        // function).
        let function = t.get_scope_root().unwrap().get_parent(t).unwrap();
        if !t.get_compiler().has_scope_changed(function) {
            return;
        }

        let current_function = self.function_stack.front().unwrap();
        // We are not going to do any dead assignment elimination in when there is
        // at least one inner function because in most browsers, when there is a
        // closure, ALL the variables are saved (escaped).
        if current_function.contains_function {
            return;
        }

        // We don't do any dead assignment elimination if there are no assigns
        // to eliminate. :)
        if !current_function.contains_removable_assign {
            return;
        }

        let block_scope = t.get_scope();
        let compiler = t.get_compiler();
        let function_scope = block_scope.get_parent(compiler).unwrap();
        if MAX_VARIABLES_TO_ANALYZE
            < block_scope.get_var_count(compiler) + function_scope.get_var_count(compiler)
        {
            return;
        }

        // Computes liveness information first.
        let cfg = self.get_control_flow_graph(t.get_compiler());
        let compiler = t.get_compiler();
        let mut scope_creator = SyntacticScopeCreator::new();
        let all_vars_declared_in_function = NodeUtil::get_all_vars_declared_in_function(
            compiler,
            &mut scope_creator,
            function_scope,
        );
        let mut liveness = LiveVariablesAnalysis::new(
            compiler,
            cfg,
            function_scope,
            Some(block_scope),
            &mut scope_creator,
            all_vars_declared_in_function,
        );
        liveness.analyze_indexed(compiler);
        self.liveness = Some(liveness);
        self.try_remove_dead_assignments(t.get_compiler());
    }

    // Matches all assignment operators and increment/decrement operators.
    // Does *not* match VAR initialization, since RemoveUnusedVariables
    // will already remove variables that are initialized but unused.
    // port: DeadAssignmentsElimination#isRemovableAssign
    pub fn is_removable_assign(&self, compiler: &AbstractCompiler, n: NodeId) -> bool {
        Self::is_removable_assign_ast(&compiler.ast, n)
    }

    #[inline]
    fn is_removable_assign_ast(ast: &closure_rhino::node::Ast, n: NodeId) -> bool {
        (NodeUtil::is_assignment_op(ast, n) && n.get_first_child(ast).unwrap().is_name(ast))
            || n.is_inc(ast)
            || n.is_dec(ast)
    }

    /// Rust-only (performance divergence): collects eligible leaf functions in exact
    /// `NodeTraversal` post-order (`exitScopeWithCfg`) without creating scopes for unchanged or
    /// non-leaf functions.
    fn collect_leaf_candidates(
        compiler: &AbstractCompiler,
        n: NodeId,
        out: &mut Vec<(NodeId, NodeId)>,
    ) -> (bool, bool) {
        if n.is_function(compiler) {
            let fn_name = n.get_first_child(compiler).unwrap();
            let params = fn_name.get_next(compiler).unwrap();
            let body = params.get_next(compiler).unwrap();
            // Default parameter expressions can contain nested leaf functions; they are visited
            // before `body` enters its function-block scope.
            Self::collect_leaf_candidates(compiler, params, out);
            let (body_has_fn, body_has_assign) = Self::collect_leaf_candidates(compiler, body, out);
            if body.is_block(compiler)
                && !body_has_fn
                && body_has_assign
                && compiler.has_scope_changed(n)
            {
                out.push((n, body));
            }
            return (true, false);
        }
        let mut has_fn = false;
        let mut has_assign = Self::is_removable_assign_ast(&compiler.ast, n);
        let mut child = n.get_first_child(compiler);
        while let Some(c) = child {
            let next = c.get_next(compiler);
            let (cf, ca) = Self::collect_leaf_candidates(compiler, c, out);
            has_fn |= cf;
            has_assign |= ca;
            child = next;
        }
        (has_fn, has_assign)
    }

    /// Try to remove useless assignments from a control flow graph that has been annotated with
    /// liveness information.
    // port: DeadAssignmentsElimination#tryRemoveDeadAssignments
    fn try_remove_dead_assignments(&self, compiler: &mut AbstractCompiler) {
        let liveness = self.liveness.as_ref().unwrap();
        let cfg = liveness.get_cfg();
        let all_vars_in_fn = liveness.get_all_variables();
        for cfg_node in cfg.get_nodes() {
            let Some(n) = *cfg_node.get_value(cfg) else {
                continue;
            };
            let state = liveness.flow_state(cfg_node);
            match n.get_token(compiler) {
                Token::IF | Token::WHILE | Token::DO => {
                    let cond = NodeUtil::get_condition_expression(compiler, n).unwrap();
                    self.try_remove_assignment(compiler, cond, state, all_vars_in_fn);
                    continue;
                }
                Token::FOR | Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                    if n.is_vanilla_for(compiler) {
                        let cond = NodeUtil::get_condition_expression(compiler, n).unwrap();
                        self.try_remove_assignment(compiler, cond, state, all_vars_in_fn);
                    }
                    continue;
                }
                Token::SWITCH | Token::CASE | Token::RETURN => {
                    if n.has_children(compiler) {
                        let first = n.get_first_child(compiler).unwrap();
                        self.try_remove_assignment(compiler, first, state, all_vars_in_fn);
                    }
                    continue;
                    // TODO(user): case VAR: Remove var a=1;a=2;.....
                }
                _ => {}
            }

            self.try_remove_assignment(compiler, n, state, all_vars_in_fn);
        }
    }

    // port: DeadAssignmentsElimination#tryRemoveAssignment(NodeTraversal, Node, LinearFlowState, Map)
    fn try_remove_assignment(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        state: &LinearFlowState<LiveVariableLattice>,
        all_vars_in_fn: &IndexMap<JsString, VarId>,
    ) {
        self.try_remove_assignment_with_expr_root(compiler, n, n, state, all_vars_in_fn);
    }

    /// Determines if any local variables are dead after the instruction `n` and are assigned
    /// within the subtree of `n`. Removes those assignments if there are any.
    // port: DeadAssignmentsElimination#tryRemoveAssignment(NodeTraversal, Node, Node, LinearFlowState, Map)
    fn try_remove_assignment_with_expr_root(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        expr_root: NodeId,
        state: &LinearFlowState<LiveVariableLattice>,
        all_vars_in_fn: &IndexMap<JsString, VarId>,
    ) {
        let parent = n.get_parent(compiler).unwrap();
        let is_declaration_node = NodeUtil::is_name_declaration(compiler, Some(parent));

        if NodeUtil::is_assignment_op(compiler, n)
            || n.is_inc(compiler)
            || n.is_dec(compiler)
            || is_declaration_node
        {
            if parent.is_const(compiler) {
                // Removing the RHS of a const produces as invalid AST.
                return;
            }

            let lhs = if is_declaration_node {
                n
            } else {
                n.get_first_child(compiler).unwrap()
            };
            let mut rhs = NodeUtil::get_r_value_of_l_value(compiler, lhs);

            // Recurse first. Example: dead_x = dead_y = 1; We try to clean up dead_y
            // first.
            if let Some(r) = rhs {
                self.try_remove_assignment_with_expr_root(
                    compiler,
                    r,
                    expr_root,
                    state,
                    all_vars_in_fn,
                );
                rhs = NodeUtil::get_r_value_of_l_value(compiler, lhs);
            }

            // Multiple declarations should be processed from right-to-left to ensure side-effects
            // are run in the correct order.
            if is_declaration_node && lhs.get_next(compiler).is_some() {
                let next = lhs.get_next(compiler).unwrap();
                self.try_remove_assignment_with_expr_root(
                    compiler,
                    next,
                    expr_root,
                    state,
                    all_vars_in_fn,
                );
            }

            // Ignore declarations that don't initialize a value. Dead code removal will kill those
            // nodes. Also ignore the var declaration if it's in a for-loop instantiation since
            // there's not a safe place to move the side-effects.
            if is_declaration_node
                && (rhs.is_none()
                    || NodeUtil::is_any_for(compiler, parent.get_parent(compiler).unwrap()))
            {
                return;
            }

            if !lhs.is_name(compiler) {
                return; // Not a local variable assignment.
            }
            let name = lhs.get_string_ref(compiler);
            let Some(&var) = all_vars_in_fn.get(name) else {
                return;
            };

            let liveness = self.liveness.as_ref().unwrap();
            let index = liveness.get_var_index(name);
            if liveness.ordered_escaped_bits().get(index) {
                return; // Local variable that might be escaped due to closures.
            }

            // If we have an identity assignment such as a=a, always remove it
            // regardless of what the liveness results because it
            // does not change the result afterward.
            if let Some(r) = rhs
                && r.is_name(compiler)
                && r.get_string_ref(compiler) == &var.get_name(compiler)
                && n.is_assign(compiler)
            {
                r.detach(compiler);
                n.replace_with(compiler, r);
                compiler.report_change_to_enclosing_scope(r);
                return;
            }

            if state.get_out().is_live(index) {
                return; // Variable not dead.
            }

            let var_name = var.get_name(compiler);
            if state.get_in().is_live(index)
                && self.is_variable_still_live_within_expression(compiler, n, expr_root, &var_name)
            {
                // The variable is killed here but it is also live before it.
                // This is possible if we have say:
                //    if (X = a && a = C) {..} ; .......; a = S;
                // In this case we are safe to remove "a = C" because it is dead.
                // However if we have:
                //    if (a = C && X = a) {..} ; .......; a = S;
                // removing "a = C" is NOT correct, although the live set at the node
                // is exactly the same.
                // TODO(user): We need more fine grain CFA or we need to keep track
                // of GEN sets when we recurse here.
                return;
            }

            if n.is_assign(compiler) {
                let rhs = rhs.unwrap();
                rhs.detach(compiler);
                n.replace_with(compiler, rhs);
            } else if NodeUtil::is_assignment_op(compiler, n) {
                let rhs = rhs.unwrap();
                rhs.detach(compiler);
                lhs.detach(compiler);
                let op_token = NodeUtil::get_op_from_assignment_op(compiler, n);
                let op = compiler.new_node_with_children2(op_token, lhs, rhs);
                n.replace_with(compiler, op);
            } else if n.is_inc(compiler) || n.is_dec(compiler) {
                if parent.is_expr_result(compiler) {
                    let number = IR::number(compiler, 0.0).srcref(compiler, n);
                    let void_node = IR::void_node(compiler, number);
                    n.replace_with(compiler, void_node);
                } else if n.is_comma(compiler) && Some(n) != parent.get_last_child(compiler) {
                    n.detach(compiler);
                } else if parent.is_vanilla_for(compiler)
                    && NodeUtil::get_condition_expression(compiler, parent) != Some(n)
                {
                    let empty = IR::empty(compiler);
                    n.replace_with(compiler, empty);
                } else {
                    // Cannot replace x = a++ with x = a because that's not valid
                    // when a is not a number.
                    return;
                }
            } else if is_declaration_node {
                let rhs = rhs.unwrap();
                rhs.detach(compiler);
                IR::expr_result(compiler, rhs).insert_after(compiler, parent);
                rhs.get_parent(compiler).unwrap().srcref(compiler, rhs);
            } else {
                // Not reachable.
                panic!("Unknown statement");
            }

            compiler.report_change_to_enclosing_scope(parent);
        } else {
            let mut c = n.get_first_child(compiler);
            while let Some(cur) = c {
                let next = cur.get_next(compiler);
                if !ControlFlowGraph::is_entering_new_cfg_node(compiler, cur) {
                    self.try_remove_assignment_with_expr_root(
                        compiler,
                        cur,
                        expr_root,
                        state,
                        all_vars_in_fn,
                    );
                }
                c = next;
            }
        }
    }

    /// Given a variable, node n in the tree and a sub-tree denoted by exprRoot as the root, this
    /// function returns true if there exists a read of that variable before a write to that
    /// variable that is on the right side of n.
    ///
    /// For example, suppose the node is x = 1:
    ///
    /// y = 1, x = 1; // false, there is no reads at all.
    /// y = 1, x = 1, print(x) // true, there is a read right of n.
    /// y = 1, x = 1, x = 2, print(x) // false, there is a read right of n but
    ///                               // it is after a write.
    // port: DeadAssignmentsElimination#isVariableStillLiveWithinExpression
    fn is_variable_still_live_within_expression(
        &self,
        compiler: &AbstractCompiler,
        mut n: NodeId,
        expr_root: NodeId,
        variable: &JsString,
    ) -> bool {
        while n != expr_root {
            let mut state = VariableLiveness::MAYBE_LIVE;
            let parent = n.get_parent(compiler).unwrap();
            match parent.get_token(compiler) {
                Token::OR
                | Token::AND
                | Token::COALESCE
                | Token::OPTCHAIN_GETPROP
                | Token::OPTCHAIN_GETELEM
                | Token::OPTCHAIN_CALL => {
                    // For short-circuiting and optional chaining operations, if the current node
                    // is the first child, subsequent siblings (the RHS or arguments) are only
                    // conditionally evaluated. Therefore, be conservative: a READ in subsequent
                    // siblings means the variable might be read, but a KILL cannot be guaranteed
                    // to occur.
                    let mut sibling = n.get_next(compiler);
                    while let Some(s) = sibling {
                        state = self.is_variable_read_before_kill(compiler, s, variable);
                        if state != VariableLiveness::MAYBE_LIVE {
                            break;
                        }
                        sibling = s.get_next(compiler);
                    }
                    if Some(n) == parent.get_first_child(compiler)
                        && state == VariableLiveness::KILL
                    {
                        state = VariableLiveness::MAYBE_LIVE;
                    }
                }
                Token::HOOK => {
                    // If current node is the condition, check each following
                    // branch, otherwise it is a conditional branch and the
                    // other branch can be ignored.
                    if let Some(next) = n.get_next(compiler)
                        && let Some(next_next) = next.get_next(compiler)
                    {
                        state = self.check_hook_branch_read_before_kill(
                            compiler, next, next_next, variable,
                        );
                    }
                }
                _ => {
                    let mut sibling = n.get_next(compiler);
                    while let Some(s) = sibling {
                        state = self.is_variable_read_before_kill(compiler, s, variable);
                        if state != VariableLiveness::MAYBE_LIVE {
                            break;
                        }
                        sibling = s.get_next(compiler);
                    }
                }
            }

            // If we see a READ or KILL there is no need to continue.
            if state == VariableLiveness::READ {
                return true;
            } else if state == VariableLiveness::KILL {
                return false;
            }
            n = parent;
        }
        false
    }

    /// Give an expression and a variable. It returns READ, if the first reference of that
    /// variable is a read. It returns KILL, if the first reference of that variable is an
    /// assignment. It returns MAY_LIVE otherwise.
    // port: DeadAssignmentsElimination#isVariableReadBeforeKill
    fn is_variable_read_before_kill(
        &self,
        compiler: &AbstractCompiler,
        n: NodeId,
        variable: &JsString,
    ) -> VariableLiveness {
        if ControlFlowGraph::is_entering_new_cfg_node(compiler, n) {
            // Not a FUNCTION
            return VariableLiveness::MAYBE_LIVE;
        }

        if n.is_name(compiler) && *variable == n.get_string(compiler) {
            let parent = n.get_parent(compiler).unwrap();
            if NodeUtil::is_name_decl_or_simple_assign_lhs(compiler, n, parent) {
                check_state!(parent.is_assign(compiler), "%s", parent.to_string(compiler));
                // The expression to which the assignment is made is evaluated before
                // the RHS is evaluated (normal left to right evaluation) but the KILL
                // occurs after the RHS is evaluated.
                let rhs = n.get_next(compiler).unwrap();
                let state = self.is_variable_read_before_kill(compiler, rhs, variable);
                if state == VariableLiveness::READ {
                    return state;
                }
                return VariableLiveness::KILL;
            } else {
                return VariableLiveness::READ;
            }
        }

        match n.get_token(compiler) {
            Token::OR
            | Token::AND
            | Token::COALESCE
            | Token::OPTCHAIN_GETPROP
            | Token::OPTCHAIN_GETELEM => {
                // Conditionals
                let v1 = self.is_variable_read_before_kill(
                    compiler,
                    n.get_first_child(compiler).unwrap(),
                    variable,
                );
                let v2 = self.is_variable_read_before_kill(
                    compiler,
                    n.get_last_child(compiler).unwrap(),
                    variable,
                );
                // With a AND/OR/COALESCE the first branch always runs, but the second is
                // may not.
                if v1 != VariableLiveness::MAYBE_LIVE {
                    return v1;
                } else if v2 == VariableLiveness::READ {
                    return VariableLiveness::READ;
                } else {
                    return VariableLiveness::MAYBE_LIVE;
                }
            }
            Token::OPTCHAIN_CALL => {
                let v1 = self.is_variable_read_before_kill(
                    compiler,
                    n.get_first_child(compiler).unwrap(),
                    variable,
                );
                if v1 != VariableLiveness::MAYBE_LIVE {
                    return v1;
                }
                let mut c = n.get_second_child(compiler);
                while let Some(cur) = c {
                    let state = self.is_variable_read_before_kill(compiler, cur, variable);
                    if state != VariableLiveness::MAYBE_LIVE {
                        return if state == VariableLiveness::READ {
                            VariableLiveness::READ
                        } else {
                            VariableLiveness::MAYBE_LIVE
                        };
                    }
                    c = cur.get_next(compiler);
                }
                return VariableLiveness::MAYBE_LIVE;
            }
            Token::HOOK => {
                let first = self.is_variable_read_before_kill(
                    compiler,
                    n.get_first_child(compiler).unwrap(),
                    variable,
                );
                if first != VariableLiveness::MAYBE_LIVE {
                    return first;
                }
                return self.check_hook_branch_read_before_kill(
                    compiler,
                    n.get_second_child(compiler).unwrap(),
                    n.get_last_child(compiler).unwrap(),
                    variable,
                );
            }
            _ => {
                // Expressions are evaluated left-right, depth first.
                let mut child = n.get_first_child(compiler);
                while let Some(cur) = child {
                    let state = self.is_variable_read_before_kill(compiler, cur, variable);
                    if state != VariableLiveness::MAYBE_LIVE {
                        return state;
                    }
                    child = cur.get_next(compiler);
                }
            }
        }

        VariableLiveness::MAYBE_LIVE
    }

    // port: DeadAssignmentsElimination#checkHookBranchReadBeforeKill
    fn check_hook_branch_read_before_kill(
        &self,
        compiler: &AbstractCompiler,
        true_case: NodeId,
        false_case: NodeId,
        variable: &JsString,
    ) -> VariableLiveness {
        let v1 = self.is_variable_read_before_kill(compiler, true_case, variable);
        let v2 = self.is_variable_read_before_kill(compiler, false_case, variable);
        // With a hook it is unknown which branch will run, so
        // we must be conservative.  A read by either is a READ, and
        // a KILL is only considered if both KILL.
        if v1 == VariableLiveness::READ || v2 == VariableLiveness::READ {
            VariableLiveness::READ
        } else if v1 == VariableLiveness::KILL && v2 == VariableLiveness::KILL {
            VariableLiveness::KILL
        } else {
            VariableLiveness::MAYBE_LIVE
        }
    }
    /// Rust-only (performance divergence): read-only necessary-condition check run on worker
    /// threads before dropping `BorrowedLiveness` on the same worker thread (zero cross-thread
    /// frees). Returns `true` if any local non-escaped assignment in `liveness` could possibly be
    /// removed by `try_remove_dead_assignments`.
    fn has_potential_dead_assignment(
        ast: &closure_rhino::node::Ast,
        prepared: &PreparedLiveness,
        liveness: &crate::live_variables_analysis::BorrowedLiveness,
    ) -> bool {
        fn expr_has_dead(
            ast: &closure_rhino::node::Ast,
            n: NodeId,
            state: &LinearFlowState<LiveVariableLattice>,
            prepared: &PreparedLiveness,
            liveness: &crate::live_variables_analysis::BorrowedLiveness,
        ) -> bool {
            let parent = n.get_parent(ast).unwrap();
            let is_decl = NodeUtil::is_name_declaration(ast, Some(parent));
            if NodeUtil::is_assignment_op(ast, n) || n.is_inc(ast) || n.is_dec(ast) || is_decl {
                if parent.is_const(ast) {
                    return false;
                }
                let lhs = if is_decl {
                    n
                } else {
                    n.get_first_child(ast).unwrap()
                };
                let rhs = NodeUtil::get_r_value_of_l_value(ast, lhs);
                if let Some(r) = rhs
                    && expr_has_dead(ast, r, state, prepared, liveness)
                {
                    return true;
                }
                if is_decl
                    && let Some(next) = lhs.get_next(ast)
                    && expr_has_dead(ast, next, state, prepared, liveness)
                {
                    return true;
                }
                if is_decl
                    && (rhs.is_none() || NodeUtil::is_any_for(ast, parent.get_parent(ast).unwrap()))
                {
                    return false;
                }
                if !lhs.is_name(ast) {
                    return false;
                }
                let name = lhs.get_string_ref(ast);
                if !prepared.all_vars().contains_key(name) {
                    return false;
                }
                let index = prepared.var_index(name);
                if liveness.ordered_escaped_bits().get(index) {
                    return false;
                }
                if let Some(r) = rhs
                    && r.is_name(ast)
                    && r.get_string_ref(ast) == name
                    && n.is_assign(ast)
                {
                    return true;
                }
                return !state.get_out().is_live(index);
            }
            let mut c = n.get_first_child(ast);
            while let Some(cur) = c {
                if !ControlFlowGraph::is_entering_new_cfg_node(ast, cur)
                    && expr_has_dead(ast, cur, state, prepared, liveness)
                {
                    return true;
                }
                c = cur.get_next(ast);
            }
            false
        }

        let cfg = liveness.cfg();
        for cfg_node in cfg.get_nodes() {
            let Some(n) = *cfg_node.get_value(cfg) else {
                continue;
            };
            let state = liveness.flow_state(cfg_node);
            let target = match n.get_token(ast) {
                Token::IF | Token::WHILE | Token::DO => {
                    NodeUtil::get_condition_expression(ast, n).unwrap()
                }
                Token::FOR | Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                    if n.is_vanilla_for(ast) {
                        NodeUtil::get_condition_expression(ast, n).unwrap()
                    } else {
                        continue;
                    }
                }
                Token::SWITCH | Token::CASE | Token::RETURN => {
                    if let Some(first) = n.get_first_child(ast) {
                        first
                    } else {
                        continue;
                    }
                }
                _ => n,
            };
            if expr_has_dead(ast, target, state, prepared, liveness) {
                return true;
            }
        }
        false
    }
}

impl CompilerPass for DeadAssignmentsElimination {
    // port: DeadAssignmentsElimination#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        // checkNotNull(externs) and checkNotNull(root): a NodeId is never null.
        check_state!(compiler.get_life_cycle_stage().is_normalized());
        // Rust-only (performance divergence): collect eligible leaf functions directly without
        // creating scopes for unchanged or non-leaf functions. On large trees, run read-only CFG
        // and liveness screening in parallel (dropping CFGs/lattices on the same worker thread to
        // avoid cross-thread heap retention), then apply removals only for surviving functions in
        // exact post-order.
        let mut candidates = Vec::new();
        Self::collect_leaf_candidates(compiler, root, &mut candidates);
        if candidates.is_empty() {
            return;
        }
        let can_continue_after_errors = compiler.get_options().can_continue_after_errors();
        let mut scope_creator = SyntacticScopeCreator::new();
        let mut scope_memo = IndexMap::default();
        let use_parallel =
            crate::parallel_parse::parallel_thread_count(candidates.len(), 64, 8) > 1;
        if !use_parallel {
            for (fn_node, body) in candidates {
                let fn_scope = ensure_ancestor_scope(
                    compiler,
                    &mut scope_creator,
                    &mut scope_memo,
                    root,
                    fn_node,
                );
                let block_scope = scope_creator.create_scope(compiler, body, Some(fn_scope));
                if MAX_VARIABLES_TO_ANALYZE
                    < block_scope.get_var_count(compiler) + fn_scope.get_var_count(compiler)
                {
                    continue;
                }
                let all_vars = NodeUtil::get_all_vars_declared_in_function(
                    compiler,
                    &mut scope_creator,
                    fn_scope,
                );
                let prepared = PreparedLiveness::new(
                    compiler,
                    fn_scope,
                    Some(block_scope),
                    &mut scope_creator,
                    all_vars,
                );
                let cfg = ControlFlowAnalysis::compute_cfg_readonly(
                    &compiler.ast,
                    can_continue_after_errors,
                    fn_node,
                    false,
                    true,
                );
                let mut liveness = prepared.into_analysis(cfg);
                liveness.analyze_readonly(&compiler.ast);
                self.liveness = Some(liveness);
                self.try_remove_dead_assignments(compiler);
                self.liveness = None;
            }
            return;
        }
        const CHUNK_SIZE: usize = 4096;
        for chunk in candidates.chunks(CHUNK_SIZE) {
            let mut jobs = Vec::with_capacity(chunk.len());
            for &(fn_node, body) in chunk {
                let fn_scope = ensure_ancestor_scope(
                    compiler,
                    &mut scope_creator,
                    &mut scope_memo,
                    root,
                    fn_node,
                );
                let block_scope = scope_creator.create_scope(compiler, body, Some(fn_scope));
                if MAX_VARIABLES_TO_ANALYZE
                    < block_scope.get_var_count(compiler) + fn_scope.get_var_count(compiler)
                {
                    continue;
                }
                let all_vars = NodeUtil::get_all_vars_declared_in_function(
                    compiler,
                    &mut scope_creator,
                    fn_scope,
                );
                let prepared = PreparedLiveness::new(
                    compiler,
                    fn_scope,
                    Some(block_scope),
                    &mut scope_creator,
                    all_vars,
                );
                jobs.push((fn_node, prepared));
            }
            let threads = crate::parallel_parse::parallel_thread_count(jobs.len(), 64, 8);
            if threads < 2 {
                for (fn_node, prepared) in jobs {
                    let cfg = ControlFlowAnalysis::compute_cfg_readonly(
                        &compiler.ast,
                        can_continue_after_errors,
                        fn_node,
                        false,
                        true,
                    );
                    let mut liveness = prepared.into_analysis(cfg);
                    liveness.analyze_readonly(&compiler.ast);
                    self.liveness = Some(liveness);
                    self.try_remove_dead_assignments(compiler);
                    self.liveness = None;
                }
                continue;
            }
            let ast = &compiler.ast;
            let has_dead: Vec<std::sync::atomic::AtomicBool> = (0..jobs.len())
                .map(|_| std::sync::atomic::AtomicBool::new(false))
                .collect();
            crate::parallel_parse::run_scoped_for_each(
                "closure-dae",
                threads,
                &jobs,
                |idx, (fn_node, prepared)| {
                    let cfg = ControlFlowAnalysis::compute_cfg_readonly(
                        ast,
                        can_continue_after_errors,
                        *fn_node,
                        false,
                        true,
                    );
                    let borrowed = prepared.run_borrowed(ast, cfg, false);
                    if Self::has_potential_dead_assignment(ast, prepared, &borrowed) {
                        has_dead[idx].store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    // `borrowed` (CFG + liveness) drops here on the worker thread.
                },
            );
            for ((fn_node, prepared), flag) in jobs.into_iter().zip(has_dead) {
                if !flag.into_inner() {
                    continue;
                }
                let cfg = ControlFlowAnalysis::compute_cfg_readonly(
                    &compiler.ast,
                    can_continue_after_errors,
                    fn_node,
                    false,
                    true,
                );
                let mut liveness = prepared.into_analysis(cfg);
                liveness.analyze_readonly(&compiler.ast);
                self.liveness = Some(liveness);
                self.try_remove_dead_assignments(compiler);
                self.liveness = None;
            }
        }
    }
}

impl Callback for DeadAssignmentsElimination {
    // port: NodeTraversal.AbstractCfgCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: DeadAssignmentsElimination#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if self.function_stack.is_empty() {
            return;
        }
        if n.is_function(t) {
            self.function_stack.front_mut().unwrap().contains_function = true;
        } else if self.is_removable_assign(t.get_compiler(), n) {
            self.function_stack
                .front_mut()
                .unwrap()
                .contains_removable_assign = true;
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for DeadAssignmentsElimination {
    // port: NodeTraversal.AbstractCfgCallback#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        let current_scope_root = check_not_null!(t.get_scope_root());
        if NodeUtil::is_valid_cfg_root(t, current_scope_root) {
            self.cfgs.push_front(current_scope_root);
        }
        self.enter_scope_with_cfg(t);
    }

    // port: NodeTraversal.AbstractCfgCallback#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        self.exit_scope_with_cfg(t);
        let current_scope_root = check_not_null!(t.get_scope_root());
        if NodeUtil::is_valid_cfg_root(t, current_scope_root) {
            check_not_null!(self.cfgs.pop_front());
        }
    }
}

// The current liveness of the variable
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
enum VariableLiveness {
    MAYBE_LIVE, // May be still live in the current expression tree.
    READ,       // Known there is a read left of it.
    KILL,       // Known there is a write before any read.
}

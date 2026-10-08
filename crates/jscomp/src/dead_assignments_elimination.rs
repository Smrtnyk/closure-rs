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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
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
    graph::{adjacency_graph::AdjacencyGraph, annotatable::Annotatable, graph_node::GraphNode},
    live_variables_analysis::{
        LiveVariableLattice, LiveVariablesAnalysis, MAX_VARIABLES_TO_ANALYZE,
    },
    node_traversal::{Callback, NodeTraversal, ScopedCallback},
    node_util::NodeUtil,
    syntactic_scope_creator::SyntacticScopeCreator,
    var::VarId,
};
use closure_rhino::{
    check_argument, check_not_null, check_state, ir::IR, js_string::JsString, node::NodeId,
    token::Token,
};
use indexmap::IndexMap;
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
        liveness.analyze(compiler);
        let all_vars_in_fn = liveness.get_all_variables().clone();
        self.liveness = Some(liveness);
        self.try_remove_dead_assignments(t, &all_vars_in_fn);
    }

    // Matches all assignment operators and increment/decrement operators.
    // Does *not* match VAR initialization, since RemoveUnusedVariables
    // will already remove variables that are initialized but unused.
    // port: DeadAssignmentsElimination#isRemovableAssign
    pub fn is_removable_assign(&self, compiler: &AbstractCompiler, n: NodeId) -> bool {
        (NodeUtil::is_assignment_op(compiler, n)
            && n.get_first_child(compiler).unwrap().is_name(compiler))
            || n.is_inc(compiler)
            || n.is_dec(compiler)
    }

    /// Try to remove useless assignments from a control flow graph that has been annotated with
    /// liveness information.
    // port: DeadAssignmentsElimination#tryRemoveDeadAssignments
    fn try_remove_dead_assignments(
        &mut self,
        t: &mut NodeTraversal<'_>,
        all_vars_in_fn: &IndexMap<JsString, VarId>,
    ) {
        let cfg = self.liveness.as_ref().unwrap().get_cfg();
        let nodes: Vec<_> = cfg
            .get_nodes()
            .into_iter()
            .map(|cfg_node| {
                (
                    cfg_node
                        .get_annotation_as::<LinearFlowState<LiveVariableLattice>>(cfg)
                        .cloned(),
                    *cfg_node.get_value(cfg),
                )
            })
            .collect();

        for (state, n) in nodes {
            let Some(n) = n else {
                continue;
            };
            let state = state.unwrap();
            match n.get_token(t) {
                Token::IF | Token::WHILE | Token::DO => {
                    let cond = NodeUtil::get_condition_expression(t, n).unwrap();
                    self.try_remove_assignment(t, cond, &state, all_vars_in_fn);
                    continue;
                }
                Token::FOR | Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                    if n.is_vanilla_for(t) {
                        let cond = NodeUtil::get_condition_expression(t, n).unwrap();
                        self.try_remove_assignment(t, cond, &state, all_vars_in_fn);
                    }
                    continue;
                }
                Token::SWITCH | Token::CASE | Token::RETURN => {
                    if n.has_children(t) {
                        let first = n.get_first_child(t).unwrap();
                        self.try_remove_assignment(t, first, &state, all_vars_in_fn);
                    }
                    continue;
                    // TODO(user): case VAR: Remove var a=1;a=2;.....
                }
                _ => {}
            }

            self.try_remove_assignment(t, n, &state, all_vars_in_fn);
        }
    }

    // port: DeadAssignmentsElimination#tryRemoveAssignment(NodeTraversal, Node, LinearFlowState, Map)
    fn try_remove_assignment(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        state: &LinearFlowState<LiveVariableLattice>,
        all_vars_in_fn: &IndexMap<JsString, VarId>,
    ) {
        self.try_remove_assignment_with_expr_root(t, n, n, state, all_vars_in_fn);
    }

    /// Determines if any local variables are dead after the instruction `n` and are assigned
    /// within the subtree of `n`. Removes those assignments if there are any.
    // port: DeadAssignmentsElimination#tryRemoveAssignment(NodeTraversal, Node, Node, LinearFlowState, Map)
    fn try_remove_assignment_with_expr_root(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        expr_root: NodeId,
        state: &LinearFlowState<LiveVariableLattice>,
        all_vars_in_fn: &IndexMap<JsString, VarId>,
    ) {
        let parent = n.get_parent(t).unwrap();
        let is_declaration_node = NodeUtil::is_name_declaration(t, Some(parent));

        if NodeUtil::is_assignment_op(t, n) || n.is_inc(t) || n.is_dec(t) || is_declaration_node {
            if parent.is_const(t) {
                // Removing the RHS of a const produces as invalid AST.
                return;
            }

            let lhs = if is_declaration_node {
                n
            } else {
                n.get_first_child(t).unwrap()
            };
            let mut rhs = NodeUtil::get_r_value_of_l_value(t, lhs);

            // Recurse first. Example: dead_x = dead_y = 1; We try to clean up dead_y
            // first.
            if let Some(r) = rhs {
                self.try_remove_assignment_with_expr_root(t, r, expr_root, state, all_vars_in_fn);
                rhs = NodeUtil::get_r_value_of_l_value(t, lhs);
            }

            // Multiple declarations should be processed from right-to-left to ensure side-effects
            // are run in the correct order.
            if is_declaration_node && lhs.get_next(t).is_some() {
                let next = lhs.get_next(t).unwrap();
                self.try_remove_assignment_with_expr_root(
                    t,
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
                && (rhs.is_none() || NodeUtil::is_any_for(t, parent.get_parent(t).unwrap()))
            {
                return;
            }

            if !lhs.is_name(t) {
                return; // Not a local variable assignment.
            }
            let name = lhs.get_string(t);
            let scope = t.get_scope();
            check_state!(
                scope.is_function_block_scope(t.get_compiler())
                    || scope.is_block_scope(t.get_compiler())
            );
            if !all_vars_in_fn.contains_key(&name) {
                return;
            }
            let var = all_vars_in_fn[&name];

            let liveness = self.liveness.as_ref().unwrap();
            // Java Set<Var>.contains uses the inherited ScopedName equality (name and scope
            // root), because LiveVariablesAnalysis collects escaped vars from recreated scopes.
            let compiler = t.get_compiler();
            if liveness
                .get_escaped_locals()
                .iter()
                .any(|escaped| escaped.equals(compiler, var))
            {
                return; // Local variable that might be escaped due to closures.
            }

            // If we have an identity assignment such as a=a, always remove it
            // regardless of what the liveness results because it
            // does not change the result afterward.
            let var_name = var.get_name(t.get_compiler());
            if let Some(r) = rhs
                && r.is_name(t)
                && r.get_string(t) == var_name
                && n.is_assign(t)
            {
                r.detach(t);
                n.replace_with(t, r);
                t.get_compiler().report_change_to_enclosing_scope(r);
                return;
            }

            let index = liveness.get_var_index(&var_name);
            if state.get_out().is_live(index) {
                return; // Variable not dead.
            }

            if state.get_in().is_live(index)
                && self.is_variable_still_live_within_expression(
                    t.get_compiler(),
                    n,
                    expr_root,
                    &var_name,
                )
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

            if n.is_assign(t) {
                let rhs = rhs.unwrap();
                rhs.detach(t);
                n.replace_with(t, rhs);
            } else if NodeUtil::is_assignment_op(t, n) {
                let rhs = rhs.unwrap();
                rhs.detach(t);
                lhs.detach(t);
                let op_token = NodeUtil::get_op_from_assignment_op(t, n);
                let op = t.new_node_with_children2(op_token, lhs, rhs);
                n.replace_with(t, op);
            } else if n.is_inc(t) || n.is_dec(t) {
                if parent.is_expr_result(t) {
                    let number = IR::number(t, 0.0).srcref(t, n);
                    let void_node = IR::void_node(t, number);
                    n.replace_with(t, void_node);
                } else if n.is_comma(t) && Some(n) != parent.get_last_child(t) {
                    n.detach(t);
                } else if parent.is_vanilla_for(t)
                    && NodeUtil::get_condition_expression(t, parent) != Some(n)
                {
                    let empty = IR::empty(t);
                    n.replace_with(t, empty);
                } else {
                    // Cannot replace x = a++ with x = a because that's not valid
                    // when a is not a number.
                    return;
                }
            } else if is_declaration_node {
                let rhs = rhs.unwrap();
                rhs.detach(t);
                IR::expr_result(t, rhs).insert_after(t, parent);
                rhs.get_parent(t).unwrap().srcref(t, rhs);
            } else {
                // Not reachable.
                panic!("Unknown statement");
            }

            t.get_compiler().report_change_to_enclosing_scope(parent);
        } else {
            let mut c = n.get_first_child(t);
            while let Some(cur) = c {
                let next = cur.get_next(t);
                if !ControlFlowGraph::is_entering_new_cfg_node(t.get_compiler(), cur) {
                    self.try_remove_assignment_with_expr_root(
                        t,
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
}

impl CompilerPass for DeadAssignmentsElimination {
    // port: DeadAssignmentsElimination#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        // checkNotNull(externs) and checkNotNull(root): a NodeId is never null.
        check_state!(compiler.get_life_cycle_stage().is_normalized());
        NodeTraversal::traverse(compiler, root, self);
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

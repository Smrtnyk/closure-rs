/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/PeepholeRemoveDeadCode.java.

//! Port of `PeepholeRemoveDeadCode.java`.
//!
//! Peephole optimization to remove useless code such as IF's with false guard conditions, comma
//! operator's left hand sides with no side effects, etc.

use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::{
        AbstractPeepholeOptimization, AbstractPeepholeOptimizationFields,
    },
    node_util::{NodeUtil, ValueType},
    peephole_fold_constants::PeepholeFoldConstants,
};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    jscomp_base::Tri,
    node::{Ast, NodeId},
    token::Token,
};
use std::collections::VecDeque;

#[derive(Default)]
pub struct PeepholeRemoveDeadCode {
    fields: AbstractPeepholeOptimizationFields,
}

impl PeepholeRemoveDeadCode {
    // port: PeepholeRemoveDeadCode#PeepholeRemoveDeadCode
    pub fn new() -> Self {
        Self::default()
    }

    // port: PeepholeRemoveDeadCode#tryRemoveDefaultValue
    fn try_remove_default_value(
        &self,
        compiler: &mut AbstractCompiler,
        default_value: NodeId,
    ) -> NodeId {
        check_argument!(
            default_value.is_default_value(compiler),
            "%s",
            default_value.to_string(compiler)
        );

        let l_value = default_value.get_first_child(compiler).unwrap();
        let val = default_value.get_second_child(compiler).unwrap();
        let mut remove_val = false;

        // If the default is `undefined` always remove the value
        if val.is_name(compiler) && val.get_string_ref(compiler) == "undefined" {
            remove_val = true;
        }

        // If the `void` application is pure, remove the value
        if val.is_void(compiler) {
            let void_arg = val.get_first_child(compiler).unwrap();
            remove_val = !self.may_have_side_effects(compiler, void_arg);
        }

        if remove_val {
            let detached = l_value.detach(compiler);
            default_value.replace_with(compiler, detached);
            self.report_change_to_enclosing_scope(compiler, l_value);
            return l_value;
        }

        default_value
    }

    // port: PeepholeRemoveDeadCode#tryFoldLabel
    fn try_fold_label(&self, compiler: &mut AbstractCompiler, n: NodeId) -> Option<NodeId> {
        let label_name = n.get_first_child(compiler).unwrap().get_string(compiler);
        let mut stmt = n.get_last_child(compiler).unwrap();
        if stmt.is_empty(compiler) {
            self.report_change_to_enclosing_scope(compiler, n);
            n.detach(compiler);
            return None;
        }

        if stmt.is_block(compiler) && !stmt.has_children(compiler) {
            self.report_change_to_enclosing_scope(compiler, n);
            if n.get_parent(compiler).unwrap().is_label(compiler) {
                // If the parent is itself a label, replace this label
                // with its contained block to keep the AST in a valid state.
                let detached = stmt.detach(compiler);
                n.replace_with(compiler, detached);
            } else {
                n.detach(compiler);
            }
            return None;
        }

        let child = Self::get_only_interesting_child(compiler, stmt);
        if let Some(child) = child {
            stmt = child;
        }
        if stmt.is_break(compiler)
            && stmt.get_first_child(compiler).unwrap().get_string(compiler) == label_name
        {
            if n.get_parent(compiler).unwrap().is_label(compiler) {
                let replacement = IR::block(compiler).srcref(compiler, n);
                n.replace_with(compiler, replacement);
                self.report_change_to_enclosing_scope(compiler, replacement);
                return Some(replacement);
            } else {
                let parent = n.get_parent(compiler).unwrap();
                n.detach(compiler);
                self.report_change_to_enclosing_scope(compiler, parent);
                return None;
            }
        }
        Some(n)
    }

    /// Return the only "interesting" child of `block`, if it has exactly one interesting child,
    /// otherwise return null. For purposes of this method, a node is considered "interesting"
    /// unless it is an empty synthetic block.
    // port: PeepholeRemoveDeadCode#getOnlyInterestingChild
    fn get_only_interesting_child(ast: &Ast, block: NodeId) -> Option<NodeId> {
        if !block.is_block(ast) {
            return None;
        }
        if block.has_one_child(ast) {
            return Some(block.get_only_child(ast));
        }

        let mut ret: Option<NodeId> = None;
        let mut child = block.get_first_child(ast);
        while let Some(c) = child {
            if c.is_synthetic_block(ast) && !c.has_children(ast) {
                // Uninteresting child.
            } else if ret.is_some() {
                // Found more than one interesting child.
                return None;
            } else {
                ret = Some(c);
            }
            child = c.get_next(ast);
        }
        ret
    }

    /// Remove try blocks without catch blocks and with empty or not existent finally blocks. Or,
    /// only leave the finally blocks if try body blocks are empty
    ///
    /// Returns the replacement node, if changed, or the original if not
    // port: PeepholeRemoveDeadCode#tryFoldTry
    fn try_fold_try(&self, compiler: &mut AbstractCompiler, n: NodeId) -> Option<NodeId> {
        check_state!(n.is_try(compiler), "%s", n.to_string(compiler));
        let body = n.get_first_child(compiler).unwrap();
        let catch_block = body.get_next(compiler).unwrap();
        let finally_block = catch_block.get_next(compiler);

        // Removes TRYs that had its CATCH removed and/or empty FINALLY.
        if !catch_block.has_children(compiler)
            && finally_block.is_none_or(|finally_block| !finally_block.has_children(compiler))
        {
            check_state!(!n.get_parent(compiler).unwrap().is_label(compiler));
            body.detach(compiler);
            n.replace_with(compiler, body);
            self.report_change_to_enclosing_scope(compiler, body);
            return Some(body);
        }

        // Only leave FINALLYs if TRYs are not empty
        if !body.has_children(compiler) {
            NodeUtil::redeclare_vars_inside_branch(compiler, catch_block);
            self.report_change_to_enclosing_scope(compiler, n);
            if let Some(finally_block) = finally_block {
                finally_block.detach(compiler);
                check_state!(!n.get_parent(compiler).unwrap().is_label(compiler));
                n.replace_with(compiler, finally_block);
                return Some(finally_block);
            } else {
                check_state!(!n.get_parent(compiler).unwrap().is_label(compiler));
                n.detach(compiler);
                return None;
            }
        }

        Some(n)
    }

    /// Try removing identity assignments and empty destructuring pattern assignments
    ///
    /// Returns the replacement node, if changed, or the original if not
    // port: PeepholeRemoveDeadCode#tryFoldAssignment
    fn try_fold_assignment(&self, compiler: &mut AbstractCompiler, subtree: NodeId) -> NodeId {
        check_state!(subtree.is_assign(compiler));
        let left = subtree.get_first_child(compiler).unwrap();
        let right = subtree.get_last_child(compiler).unwrap();
        if left.is_name(compiler)
            && right.is_name(compiler)
            && left.get_string(compiler) == right.get_string(compiler)
        {
            // Only names
            let detached = right.detach(compiler);
            subtree.replace_with(compiler, detached);
            self.report_change_to_enclosing_scope(compiler, right);
            return right;
        } else if left.is_destructuring_pattern(compiler) && !left.has_children(compiler) {
            // `[] = <expr>` becomes `<expr>`
            // Note that this does potentially change behavior. If `<expr>` is not iterable and
            // this code originally threw, it will no longer throw.
            let detached = right.detach(compiler);
            subtree.replace_with(compiler, detached);
            self.report_change_to_enclosing_scope(compiler, right);
            return right;
        }
        subtree
    }

    /// Try removing identity assignments and empty destructuring pattern assignments
    ///
    /// Returns the replacement node, if changed, or the original if not
    // port: PeepholeRemoveDeadCode#tryOptimizeNameDeclaration
    fn try_optimize_name_declaration(
        &self,
        compiler: &mut AbstractCompiler,
        subtree: NodeId,
    ) -> NodeId {
        check_state!(NodeUtil::is_name_declaration(compiler, Some(subtree)));
        let left = subtree.get_first_child(compiler).unwrap();
        if left.is_destructuring_lhs(compiler) && left.has_two_children(compiler) {
            let pattern = left.get_first_child(compiler).unwrap();
            if !pattern.has_children(compiler) {
                // `var [] = foo();` becomes `foo();`
                let value = left.get_second_child(compiler).unwrap();
                let detached = value.detach(compiler);
                let replacement = IR::expr_result(compiler, detached).srcref(compiler, value);
                subtree.replace_with(compiler, replacement);
                self.report_change_to_enclosing_scope(compiler, value);
            }
        }
        subtree
    }

    /// Try folding EXPR_RESULT nodes by removing useless Ops and expressions.
    ///
    /// Returns the replacement node, if changed, or the original if not
    // port: PeepholeRemoveDeadCode#tryFoldExpr
    fn try_fold_expr(&self, compiler: &mut AbstractCompiler, subtree: NodeId) -> Option<NodeId> {
        let first_child = subtree.get_first_child(compiler).unwrap();
        let result = self.try_simplify_unused_result(compiler, first_child);
        if result.is_none() {
            let parent = subtree.get_parent(compiler).unwrap();
            // If the EXPR_RESULT no longer has any children, remove it as well.
            if parent.is_label(compiler) {
                let replacement = IR::block(compiler).srcref(compiler, subtree);
                subtree.replace_with(compiler, replacement);
                return Some(replacement);
            } else {
                subtree.detach(compiler);
                return None;
            }
        }
        Some(subtree)
    }

    /// Replaces `expression` with an expression that contains only side-effects of the
    /// original.
    ///
    /// This replacement is made under the assumption that the result of `expression` is unused
    /// and therefore it is correct to eliminate non-side-effectful nodes.
    ///
    /// Returns the replacement expression, or `None` if there were no side-effects to preserve.
    // port: PeepholeRemoveDeadCode#trySimplifyUnusedResult
    fn try_simplify_unused_result(
        &self,
        compiler: &mut AbstractCompiler,
        expression: NodeId,
    ) -> Option<NodeId> {
        let mut side_effect_roots: VecDeque<NodeId> = VecDeque::new();
        let at_fixed_point =
            self.try_simplify_unused_result_internal(compiler, expression, &mut side_effect_roots);

        if at_fixed_point {
            // `expression` is in a form that cannot be further optimized.
            Some(expression)
        } else if side_effect_roots.is_empty() {
            self.delete_node(compiler, expression);
            None
        } else if side_effect_roots.front() == Some(&expression) {
            // Expression was a conditional that was transformed. There can't be any other
            // side-effects, but we also can't detach the transformed root.
            check_state!(
                side_effect_roots.len() == 1,
                "%s",
                format_deque(compiler, &side_effect_roots)
            );
            self.report_change_to_enclosing_scope(compiler, expression);
            Some(expression)
        } else {
            let first = side_effect_roots.pop_front().unwrap();
            let mut side_effects = Self::as_detached_expression(compiler, first);

            // Assemble a tree of comma expressions for all the side-effects. The tree must
            // execute the side-effects in FIFO order with respect to the queue. It must also be
            // left leaning to match the parser's preferred structure.
            while let Some(next) = side_effect_roots.pop_front() {
                let next = Self::as_detached_expression(compiler, next);
                side_effects = IR::comma(compiler, side_effects, next).srcref(compiler, next);
            }

            side_effects.insert_before(compiler, expression);
            self.delete_node(compiler, expression);
            Some(side_effects)
        }
    }

    /// Collects any potentially side-effectful subtrees within `tree` into `side_effect_roots`.
    ///
    /// When a node is determined to have side-effects its descendants are not explored. This
    /// method assumes the entire subtree of such a node must be preserved. As a corollary, the
    /// contents of `side_effect_roots` are a forest.
    ///
    /// This operation generally does not mutate `tree`; however, exceptions are made for
    /// expressions that alter control-flow. Such expression will be pruned of their
    /// side-effectless branches. Even in this case, `tree` is never detached.
    ///
    /// `side_effect_roots`: the roots of subtrees determined to have side-effects, in execution
    /// order. Returns `true` iff there is no code to be removed from within `tree`; it is
    /// already at a fixed point for code removal.
    // port: PeepholeRemoveDeadCode#trySimplifyUnusedResultInternal
    fn try_simplify_unused_result_internal(
        &self,
        compiler: &mut AbstractCompiler,
        tree: NodeId,
        side_effect_roots: &mut VecDeque<NodeId>,
    ) -> bool {
        // Special cases for conditional expressions that may be using results.
        match tree.get_token(compiler) {
            Token::OPTCHAIN_GETELEM | Token::OPTCHAIN_GETPROP | Token::OPTCHAIN_CALL => {
                if self.may_have_side_effects(compiler, tree) {
                    side_effect_roots.push_back(tree);
                    Self::has_fixed_point_parent(compiler, tree)
                } else {
                    false
                }
            }
            Token::HOOK => {
                // Try to remove one or more of the conditional children and transform the HOOK
                // to an equivalent operation. Remember that if either value branch still exists,
                // the result of the predicate expression is being used, and so cannot be
                // removed.
                //    x() ? foo() : 1 --> x() && foo()
                //    x() ? 1 : foo() --> x() || foo()
                //    x() ? 1 : 1 --> x()
                //    x ? 1 : 1 --> null
                let second_child = tree.get_second_child(compiler).unwrap();
                let true_node = self.try_simplify_unused_result(compiler, second_child);
                let last_child = tree.get_last_child(compiler).unwrap();
                let false_node = self.try_simplify_unused_result(compiler, last_child);
                if true_node.is_none() && false_node.is_some() {
                    check_state!(
                        tree.has_two_children(compiler),
                        "%s",
                        tree.to_string(compiler)
                    );

                    tree.set_token(compiler, Token::OR);
                    side_effect_roots.push_back(tree);
                    false // The node type was changed.
                } else if true_node.is_some() && false_node.is_none() {
                    check_state!(
                        tree.has_two_children(compiler),
                        "%s",
                        tree.to_string(compiler)
                    );

                    tree.set_token(compiler, Token::AND);
                    side_effect_roots.push_back(tree);
                    false // The node type was changed.
                } else if true_node.is_none() && false_node.is_none() {
                    // Don't bother adding true and false branch children to make the AST valid;
                    // this HOOK is going to be deleted. We just need to collect any side-effects
                    // from the predicate expression.
                    let only_child = tree.get_only_child(compiler);
                    self.try_simplify_unused_result_internal(
                        compiler,
                        only_child,
                        side_effect_roots,
                    );
                    false // This HOOK must be cleaned up.
                } else {
                    side_effect_roots.push_back(tree);
                    Self::has_fixed_point_parent(compiler, tree)
                }
            }
            Token::AND | Token::OR | Token::COALESCE => {
                // Try to remove the second operand from a AND, OR, and COALESCE operations.
                // Remember that if the second
                // child still exists, the result of the first expression is being used, and so
                // cannot be removed.
                //    x() ?? f --> x()
                //    x() || f --> x()
                //    x() && f --> x()
                let last_child = tree.get_last_child(compiler).unwrap();
                let conditional_result_node = self.try_simplify_unused_result(compiler, last_child);
                if conditional_result_node.is_none() {
                    // Don't bother adding a second child to make the AST valid; this op is going
                    // to be deleted. We just need to collect any side-effects from the predicate
                    // first child.
                    let only_child = tree.get_only_child(compiler);
                    self.try_simplify_unused_result_internal(
                        compiler,
                        only_child,
                        side_effect_roots,
                    );
                    false // This op must be cleaned up.
                } else {
                    side_effect_roots.push_back(tree);
                    Self::has_fixed_point_parent(compiler, tree)
                }
            }
            Token::FUNCTION => {
                // Functions that aren't being invoked are dead. If they were invoked we'd see the
                // CALL before arriving here. We don't want to look at any children since they'll
                // never execute.
                false
            }
            _ => {
                // This is the meat of this function. It covers the general case of nodes which
                // are unused
                if self.node_type_may_have_side_effects(compiler, tree) {
                    side_effect_roots.push_back(tree);
                    return Self::has_fixed_point_parent(compiler, tree);
                } else if !tree.has_children(compiler) {
                    return false; // A node must have children or side-effects to be at fixed-point.
                }

                let mut at_fixed_point = Self::has_fixed_point_parent(compiler, tree);
                let mut child = tree.get_first_child(compiler);
                while let Some(c) = child {
                    at_fixed_point &=
                        self.try_simplify_unused_result_internal(compiler, c, side_effect_roots);
                    child = c.get_next(compiler);
                }
                at_fixed_point
            }
        }
    }

    /// Returns an expression executing `expr` which is legal in any expression context.
    ///
    /// `expr`: an attached expression. Returns a detached expression.
    // port: PeepholeRemoveDeadCode#asDetachedExpression
    fn as_detached_expression(ast: &mut Ast, mut expr: NodeId) -> NodeId {
        if let Token::ITER_SPREAD | Token::OBJECT_SPREAD = expr.get_token(ast) {
            match expr.get_parent(ast).unwrap().get_token(ast) {
                Token::ARRAYLIT | Token::NEW | Token::CALL | Token::OPTCHAIN_CALL => {
                    // `Math.sin(...c)`, `Math?.sin(...c)`
                    let detached = expr.detach(ast);
                    expr = IR::arraylit(ast, &[detached]).srcref(ast, expr);
                }
                Token::OBJECTLIT => {
                    let detached = expr.detach(ast);
                    expr = IR::objectlit(ast, &[detached]).srcref(ast, expr);
                }
                _ => panic!("{}", expr.to_string_tree(ast)),
            }
        }

        if expr.has_parent(ast) {
            expr.detach(ast);
        }

        check_state!(IR::may_be_expression(ast, expr), "%s", expr.to_string(ast));
        expr
    }

    /// Returns `true` iff `expr` is parented such that it is valid in a fixed-point
    /// representation of an unused expression tree.
    ///
    /// A fixed-point representation is one in which no further nodes should be changed or
    /// removed when removing unused code. This method assumes that the expression tree in
    /// question is unused, so only side-effects are relevant.
    // port: PeepholeRemoveDeadCode#hasFixedPointParent
    fn has_fixed_point_parent(ast: &Ast, expr: NodeId) -> bool {
        // Most kinds of nodes shouldn't be branches in the fixed-point tree of an unused
        // expression. Those listed below are the only valid kinds.
        let parent = expr.get_parent(ast).unwrap();
        match parent.get_token(ast) {
            Token::AND | Token::COMMA | Token::HOOK | Token::OR | Token::COALESCE => true,
            Token::ARRAYLIT | Token::OBJECTLIT => {
                // Make a special allowance for SPREADs so they remain in a legal context. Parent
                // types other than ARRAYLIT and OBJECTLIT are not fixed-point because they are
                // the tersest legal
                // parents and are known to be side-effect free.
                expr.is_spread(ast)
            }
            _ => {
                // Statments are always fixed-point parents. All other expressions are not.
                NodeUtil::is_statement(ast, parent)
            }
        }
    }

    /// A predicate for matching anything except function nodes.
    // port: PeepholeRemoveDeadCode.MatchUnnamedBreak#apply
    pub const MATCH_UNNAMED_BREAK: fn(&Ast, NodeId) -> bool =
        |ast, n| n.is_break(ast) && !n.has_children(ast);

    // port: PeepholeRemoveDeadCode#removeIfUnnamedBreak
    fn remove_if_unnamed_break(
        &self,
        compiler: &mut AbstractCompiler,
        maybe_break: Option<NodeId>,
    ) {
        if let Some(maybe_break) = maybe_break
            && maybe_break.is_break(compiler)
            && !maybe_break.has_children(compiler)
        {
            self.report_change_to_enclosing_scope(compiler, maybe_break);
            maybe_break.detach(compiler);
        }
    }
}

// Rust-only: Java's `ArrayDeque<Node>.toString()` for a precondition message.
fn format_deque(ast: &Ast, deque: &VecDeque<NodeId>) -> String {
    let items: Vec<String> = deque.iter().map(|n| n.to_string(ast)).collect();
    format!("[{}]", items.join(", "))
}

impl AbstractPeepholeOptimization for PeepholeRemoveDeadCode {
    fn fields(&self) -> &AbstractPeepholeOptimizationFields {
        &self.fields
    }

    fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
        &mut self.fields
    }

    fn get_class_name(&self) -> &'static str {
        "com.google.javascript.jscomp.PeepholeRemoveDeadCode"
    }

    // port: PeepholeRemoveDeadCode#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        subtree: NodeId,
    ) -> Option<NodeId> {
        match subtree.get_token(compiler) {
            Token::ASSIGN => Some(self.try_fold_assignment(compiler, subtree)),
            Token::COMMA => Some(self.try_fold_comma(compiler, subtree)),
            Token::SCRIPT | Token::BLOCK => self.try_optimize_block(compiler, subtree),
            Token::EXPR_RESULT => self.try_fold_expr(compiler, subtree),
            Token::HOOK => Some(self.try_fold_hook(compiler, subtree)),
            Token::SWITCH => Some(self.try_optimize_switch(compiler, subtree)),
            Token::IF => self.try_fold_if(compiler, subtree),
            Token::WHILE => {
                // This pass gets run both before and after denormalize. Hence, the AST could
                // potentially contain WHILE (denormalized).
                // TODO: Ideally, we should optimize this case instead of returning
                Some(subtree)
            }
            Token::FOR => {
                let condition = NodeUtil::get_condition_expression(compiler, subtree);
                if let Some(condition) = condition {
                    self.try_fold_for_condition(compiler, condition);
                }
                self.try_fold_for(compiler, subtree)
            }
            Token::DO => {
                let folded_do = self.try_fold_do_away(compiler, subtree);
                if folded_do.is_do(compiler) {
                    return Some(self.try_fold_empty_do(compiler, folded_do));
                }
                Some(folded_do)
            }
            Token::TRY => self.try_fold_try(compiler, subtree),
            Token::LABEL => self.try_fold_label(compiler, subtree),
            Token::ARRAY_PATTERN => Some(self.try_optimize_array_pattern(compiler, subtree)),
            Token::OBJECT_PATTERN => Some(self.try_optimize_object_pattern(compiler, subtree)),
            Token::VAR | Token::CONST | Token::LET => {
                Some(self.try_optimize_name_declaration(compiler, subtree))
            }
            Token::DEFAULT_VALUE => Some(self.try_remove_default_value(compiler, subtree)),
            Token::OPTCHAIN_GETPROP | Token::OPTCHAIN_CALL | Token::OPTCHAIN_GETELEM => {
                Some(self.try_remove_optional_chaining(compiler, subtree))
            }
            _ => Some(subtree),
        }
    }
}

impl PeepholeRemoveDeadCode {
    // port: PeepholeRemoveDeadCode#tryRemoveSwitchWithSingleCase
    fn try_remove_switch_with_single_case(
        &self,
        compiler: &mut AbstractCompiler,
        switch_node: NodeId,
        should_hoist_condition: bool,
    ) -> NodeId {
        let switch_body = switch_node.get_second_child(compiler).unwrap();
        let case_block = switch_body
            .get_only_child(compiler)
            .get_last_child(compiler)
            .unwrap();
        let last_child = case_block.get_last_child(compiler);
        self.remove_if_unnamed_break(compiler, last_child);
        // Back off if the switch contains statements like "if (a) { break; }"
        if NodeUtil::has(
            compiler,
            case_block,
            &Self::MATCH_UNNAMED_BREAK,
            &NodeUtil::MATCH_NOT_FUNCTION,
        ) {
            return switch_node;
        }
        if should_hoist_condition {
            let condition = switch_node.remove_first_child(compiler).unwrap();
            let hoisted = IR::expr_result(compiler, condition).srcref(compiler, switch_node);
            hoisted.insert_before(compiler, switch_node);
        }
        let detached = case_block.detach(compiler);
        switch_node.replace_with(compiler, detached);
        self.report_change_to_enclosing_scope(compiler, case_block);
        case_block
    }

    // port: PeepholeRemoveDeadCode#tryRemoveSwitch
    fn try_remove_switch(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        let switch_body = n.get_second_child(compiler).unwrap();
        if !switch_body.has_children(compiler) {
            // Remove the switch if there are no remaining cases
            let condition = n.remove_first_child(compiler).unwrap();
            let replacement = IR::expr_result(compiler, condition).srcref(compiler, n);
            n.replace_with(compiler, replacement);
            self.report_change_to_enclosing_scope(compiler, replacement);
            replacement
        } else if switch_body.has_one_child(compiler)
            && switch_body
                .get_only_child(compiler)
                .is_default_case(compiler)
        {
            let condition = n.get_first_child(compiler).unwrap();
            if self.may_have_side_effects(compiler, condition) {
                // Before removing switch, we must preserve the switch condition if it has side
                // effects
                self.try_remove_switch_with_single_case(compiler, n, true)
            } else {
                self.try_remove_switch_with_single_case(compiler, n, false)
            }
        } else {
            n
        }
    }

    /// Remove useless switches and cases.
    // port: PeepholeRemoveDeadCode#tryOptimizeSwitch
    fn try_optimize_switch(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        check_state!(n.is_switch(compiler), "%s", n.to_string(compiler));

        let switch_body = n.get_second_child(compiler).unwrap();
        let default_case = self.try_optimize_default_case(compiler, n);

        // Generally, it is unsafe to remove other cases when the default case is not the last
        // one.
        if (default_case.is_none()
            || switch_body
                .get_last_child(compiler)
                .unwrap()
                .is_default_case(compiler))
            && self.are_all_case_tags_literals(compiler, switch_body)
        {
            let cond = n.get_first_child(compiler).unwrap();
            let mut prev: Option<NodeId> = None;
            let mut next: Option<NodeId>;
            let mut cur: Option<NodeId>;

            // First, remove empty cases where possible: always empty default cases; or when there
            // is no default case, other empty cases that are not the first matching case, may be
            // removable as well.
            let mut found_matching_case = false;
            cur = switch_body.get_first_child(compiler);
            while let Some(c) = cur {
                next = c.get_next(compiler);
                let first_child = c.get_first_child(compiler).unwrap();
                found_matching_case =
                    self.is_first_switch_match(compiler, found_matching_case, cond, first_child);
                if !found_matching_case
                    && !self.may_have_side_effects(compiler, first_child)
                    && self.is_useless_case(compiler, c, prev, default_case)
                {
                    self.remove_case(compiler, n, c);
                } else {
                    prev = Some(c);
                }
                cur = next;
            }

            // Next, optimize switches with constant condition
            if NodeUtil::is_literal_value(compiler, cond, false) {
                let mut case_label: NodeId;
                let mut case_matches = Tri::TRUE;
                // Remove cases until you find one that may match
                cur = switch_body.get_first_child(compiler);
                while let Some(c) = cur {
                    next = c.get_next(compiler);
                    case_label = c.get_first_child(compiler).unwrap();
                    case_matches = PeepholeFoldConstants::evaluate_comparison(
                        self,
                        compiler,
                        Token::SHEQ,
                        cond,
                        case_label,
                    );
                    // Java's `if (TRUE) break; else if (UNKNOWN) break; else removeCase`, as a
                    // match because clippy rejects the identical if-branches.
                    match case_matches {
                        Tri::TRUE => break,
                        Tri::UNKNOWN => break,
                        Tri::FALSE => self.remove_case(compiler, n, c),
                    }
                    cur = next;
                }
                if let Some(matching_case) = cur
                    && case_matches == Tri::TRUE
                {
                    // Skip cases until you find one whose last stm is a removable break
                    let matching_case_block = matching_case.get_last_child(compiler).unwrap();
                    while let Some(c) = cur {
                        let block = c.get_last_child(compiler).unwrap();
                        let last_stm = block.get_last_child(compiler);
                        let mut is_last_stm_removable_break = false;
                        if let Some(last_stm) = last_stm
                            && Self::is_exit(compiler, last_stm)
                        {
                            self.remove_if_unnamed_break(compiler, Some(last_stm));
                            is_last_stm_removable_break = true;
                        }
                        next = c.get_next(compiler);
                        // Remove the fallthrough case labels
                        if c != matching_case {
                            while block.has_children(compiler) {
                                let child = block.remove_first_child(compiler).unwrap();
                                matching_case_block.add_child_to_back(compiler, child);
                            }
                            self.report_change_to_enclosing_scope(compiler, c);
                            c.detach(compiler);
                        }
                        cur = next;
                        if is_last_stm_removable_break {
                            break;
                        }
                    }

                    // Remove any remaining cases
                    while let Some(c) = cur {
                        next = c.get_next(compiler);
                        self.remove_case(compiler, n, c);
                        cur = next;
                    }
                    // If there is one case left, we may be able to fold it
                    cur = cond.get_next(compiler);
                    if let Some(c) = cur
                        && c.get_next(compiler).is_none()
                    {
                        return self.try_remove_switch_with_single_case(compiler, n, false);
                    }
                }
            }
        }

        // Last, try to remove the entire switch if possible
        self.try_remove_switch(compiler, n)
    }

    /// Returns the default case node or null if there is no default case or if the default case
    /// is removed.
    // port: PeepholeRemoveDeadCode#tryOptimizeDefaultCase
    fn try_optimize_default_case(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> Option<NodeId> {
        check_state!(n.is_switch(compiler), "%s", n.to_string(compiler));

        let switch_body = n.get_second_child(compiler).unwrap();
        // the most recently iterated case known to not be removable.
        let mut last_non_removable: Option<NodeId> = None;

        let mut c = switch_body.get_first_child(compiler);
        while let Some(cur) = c {
            if cur.is_default_case(compiler) {
                // Remove any cases that fall-through to the default case
                let mut case_to_remove = match last_non_removable {
                    Some(last_non_removable) => last_non_removable.get_next(compiler),
                    None => switch_body.get_first_child(compiler),
                };
                while case_to_remove != Some(cur) {
                    let to_remove = case_to_remove.unwrap();
                    let next = to_remove.get_next(compiler);
                    self.remove_case(compiler, n, to_remove);
                    case_to_remove = next;
                }

                // Remove the default case if we can
                if self.is_useless_case(compiler, cur, last_non_removable, Some(cur)) {
                    self.remove_case(compiler, n, cur);
                    return None;
                }
                return Some(cur);
            } else {
                check_state!(cur.is_case(compiler));
                let case_condition = cur.get_first_child(compiler).unwrap();
                if cur.get_last_child(compiler).unwrap().has_children(compiler)
                    || self.may_have_side_effects(compiler, case_condition)
                {
                    last_non_removable = Some(cur);
                }
            }
            c = cur.get_next(compiler);
        }
        None
    }

    /// Remove the case from the switch redeclaring any variables declared in it.
    ///
    /// `case_node`: The case to remove.
    // port: PeepholeRemoveDeadCode#removeCase
    fn remove_case(&self, compiler: &mut AbstractCompiler, switch_node: NodeId, case_node: NodeId) {
        NodeUtil::redeclare_vars_inside_branch(compiler, case_node);
        case_node.detach(compiler);
        self.report_change_to_enclosing_scope(compiler, switch_node);
    }

    /// The function assumes that when checking a CASE node there is no DEFAULT_CASE node in the
    /// SWITCH, or the DEFAULT_CASE is the last case in the SWITCH.
    ///
    /// Returns whether the CASE or DEFAULT_CASE block does anything useful.
    // port: PeepholeRemoveDeadCode#isUselessCase
    fn is_useless_case(
        &self,
        compiler: &mut AbstractCompiler,
        case_node: NodeId,
        previous_case: Option<NodeId>,
        default_case: Option<NodeId>,
    ) -> bool {
        check_state!(
            previous_case.is_none_or(|previous_case| {
                previous_case.get_next(compiler) == Some(case_node)
            })
        );
        // A case isn't useless if a previous case falls through to it unless it happens to be
        // the last case in the switch.
        let switch_body = case_node.get_parent(compiler).unwrap();
        if switch_body.get_last_child(compiler) != Some(case_node)
            && let Some(previous_case) = previous_case
        {
            let previous_block = previous_case.get_last_child(compiler).unwrap();
            if !previous_block.has_children(compiler)
                || !Self::is_exit(compiler, previous_block.get_last_child(compiler).unwrap())
            {
                return false;
            }
        }

        let mut executing_case = Some(case_node);
        while let Some(executing) = executing_case {
            check_state!(executing.is_default_case(compiler) || executing.is_case(compiler));
            // We only expect a DEFAULT case if the case we are checking is the
            // DEFAULT case.  Otherwise, we assume the DEFAULT case has already
            // been removed.
            check_state!(case_node == executing || !executing.is_default_case(compiler));
            if !executing.is_default_case(compiler)
                && self
                    .may_have_side_effects(compiler, executing.get_first_child(compiler).unwrap())
            {
                // The case falls thru to a case whose condition has a potential side-effect,
                // removing the candidate case would skip that side-effect, so don't.
                return false;
            }
            let block = executing.get_last_child(compiler).unwrap();
            check_state!(block.is_block(compiler));
            if block.has_children(compiler) {
                let mut block_child = block.get_first_child(compiler);
                while let Some(child) = block_child {
                    // If this is a block with a labelless break, it is useless.
                    match child.get_token(compiler) {
                        Token::BREAK => {
                            // A case with a single labelless break is useless if it is the
                            // default case or if there is no default case. A break to a
                            // different control structure isn't useless.
                            return !child.has_children(compiler)
                                && (default_case.is_none() || default_case == Some(executing));
                        }
                        Token::VAR => {
                            if child.has_one_child(compiler)
                                && child.get_first_first_child(compiler).is_none()
                            {
                                // Variable declarations without initializations are OK.
                                block_child = child.get_next(compiler);
                                continue;
                            }
                            return false;
                        }
                        _ => {
                            return false;
                        }
                    }
                }
            }
            // Look at the fallthrough case
            executing_case = executing.get_next(compiler);
        }
        true
    }

    // port: PeepholeRemoveDeadCode#isFirstSwitchMatch
    fn is_first_switch_match(
        &self,
        compiler: &mut AbstractCompiler,
        found_matching_case: bool,
        condition: NodeId,
        tag: NodeId,
    ) -> bool {
        if found_matching_case {
            return false;
        }
        PeepholeFoldConstants::evaluate_comparison(self, compiler, Token::SHEQ, condition, tag)
            == Tri::TRUE
    }

    // port: PeepholeRemoveDeadCode#areAllCaseTagsLiterals
    fn are_all_case_tags_literals(&self, compiler: &AbstractCompiler, switch_body: NodeId) -> bool {
        let mut case_node = switch_body.get_first_child(compiler);
        while let Some(case) = case_node {
            if case.is_default_case(compiler) {
                case_node = case.get_next(compiler);
                continue;
            }
            let case_tag = case.get_first_child(compiler).unwrap();
            if !NodeUtil::is_literal_value(compiler, case_tag, false) {
                return false;
            }
            case_node = case.get_next(compiler);
        }
        true
    }

    /// Returns whether the node is a control flow exit from the current block.
    // port: PeepholeRemoveDeadCode#isExit
    fn is_exit(ast: &Ast, n: NodeId) -> bool {
        match n.get_token(ast) {
            Token::BREAK | Token::CONTINUE | Token::RETURN | Token::THROW => true,
            Token::SWITCH => Self::is_switch_exit(ast, n),
            Token::TRY => Self::is_try_exit(ast, n),
            _ => false,
        }
    }

    /// Returns whether the block is a control flow exit from the block containing current switch
    /// or try..catch statement.
    // port: PeepholeRemoveDeadCode#isUnconditionalBlockExit
    fn is_unconditional_block_exit(ast: &Ast, n: NodeId) -> bool {
        check_state!(n.is_block(ast), "%s", n.to_string(ast));
        check_state!(
            !n.get_parent(ast).unwrap().is_label(ast),
            "%s",
            n.to_string(ast)
        );

        // Last statement must lead out of the block.
        let Some(last_stm) = n.get_last_child(ast) else {
            return false;
        };
        match last_stm.get_token(ast) {
            Token::BREAK => {
                if !last_stm.has_children(ast) {
                    return false;
                }
                // Last statement is OK - continue with checking others.
            }
            Token::RETURN | Token::THROW => {
                // Last statement is OK - continue with checking others.
            }
            _ => {
                return false;
            }
        }

        // Other statements can be anything except for unlabeled "break". But for simplicity,
        // don't go into inner blocks and complex constructs - instead, allow only the simplest
        // statements.
        let mut child = n.get_first_child(ast).unwrap();
        while child != last_stm {
            match child.get_token(ast) {
                Token::BREAK => {
                    if !child.has_children(ast) {
                        return false;
                    }
                    // This break is OK - continue with checking others.
                }
                Token::RETURN
                | Token::THROW
                | Token::FUNCTION
                | Token::VAR
                | Token::LET
                | Token::CONST
                | Token::EXPR_RESULT => {
                    // This statement is OK - continue with checking others.
                }
                _ => {
                    return false;
                }
            }
            child = child.get_next(ast).unwrap();
        }

        true
    }

    /// Return true if the switch always "exits" (return, throw, etc).
    // port: PeepholeRemoveDeadCode#isSwitchExit
    fn is_switch_exit(ast: &Ast, n: NodeId) -> bool {
        check_state!(n.is_switch(ast), "%s", n.to_string(ast));

        let mut has_default_case = false;

        let switch_body = n.get_second_child(ast).unwrap();
        let mut switch_case = switch_body.get_first_child(ast);
        while let Some(case) = switch_case {
            if case.is_default_case(ast) {
                has_default_case = true;
            }
            let block = case.get_last_child(ast).unwrap();
            if (block.has_children(ast) || case.get_next(ast).is_none())
                && !Self::is_unconditional_block_exit(ast, block)
            {
                return false;
            }
            switch_case = case.get_next(ast);
        }

        has_default_case
    }

    /// Return true if the try..catch always "exits" (return, throw, etc).
    // port: PeepholeRemoveDeadCode#isTryExit
    fn is_try_exit(ast: &Ast, n: NodeId) -> bool {
        check_state!(n.is_try(ast), "%s", n.to_string(ast));

        // finally - regardless of the behavior of the other blocks,
        // an exit from the finally with guarantee that behavior.
        if n.has_x_children(ast, 3)
            && Self::is_unconditional_block_exit(ast, n.get_last_child(ast).unwrap())
        {
            return true;
        }
        // try
        if !Self::is_unconditional_block_exit(ast, n.get_first_child(ast).unwrap()) {
            return false;
        }
        // catch
        let catches = n.get_second_child(ast).unwrap();
        !catches.has_children(ast)
            || Self::is_unconditional_block_exit(
                ast,
                catches.get_only_child(ast).get_last_child(ast).unwrap(),
            )
    }

    // port: PeepholeRemoveDeadCode#tryFoldComma
    fn try_fold_comma(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        // If the left side does nothing replace the comma with the result.
        let parent = n.get_parent(compiler).unwrap();
        let left = n.get_first_child(compiler).unwrap();
        let right = left.get_next(compiler).unwrap();

        let left = self.try_simplify_unused_result(compiler, left);
        if left.is_none_or(|left| !self.may_have_side_effects(compiler, left)) {
            // Fold it!
            right.detach(compiler);
            n.replace_with(compiler, right);
            self.report_change_to_enclosing_scope(compiler, parent);
            return right;
        }
        n
    }

    /// Try removing unneeded block nodes and their useless children
    // port: PeepholeRemoveDeadCode#tryOptimizeBlock
    pub fn try_optimize_block(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> Option<NodeId> {
        // Remove any useless children
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            let next = cur.get_next(compiler); // save c.next, since 'c' may be removed
            if !Self::is_unremovable_node(compiler, cur)
                && !self.may_have_side_effects(compiler, cur)
            {
                // TODO(johnlenz): determine what this is actually removing. Candidates
                //    include: EMPTY nodes, control structures without children
                //    (removing infinite loops), empty try blocks.  What else?
                cur.detach(compiler);
                self.report_change_to_enclosing_scope(compiler, n);
                self.mark_functions_deleted(compiler, cur);
            } else if Self::is_exit(compiler, cur) {
                self.remove_following_nodes(compiler, cur);
                break;
            } else {
                self.try_optimize_conditional_after_assign(compiler, cur);
            }
            c = next;
        }

        if n.is_synthetic_block(compiler)
            || n.is_script(compiler)
            || n.get_parent(compiler).is_none()
        {
            return Some(n);
        }

        // Try to merge the block with its parent, or remove it if it is an empty class static block.
        let parent = n.get_parent(compiler).unwrap();
        let is_ast_normalized = self.is_ast_normalized(compiler);
        if NodeUtil::try_merge_block(compiler, n, is_ast_normalized) {
            self.report_change_to_enclosing_scope(compiler, parent);
            return None;
        } else if parent.is_class_members(compiler) && !n.has_children(compiler) {
            n.detach(compiler);
            self.report_change_to_enclosing_scope(compiler, parent);
            return None;
        }

        Some(n)
    }

    // port: PeepholeRemoveDeadCode#removeFollowingNodes
    fn remove_following_nodes(&mut self, compiler: &mut AbstractCompiler, mut n: NodeId) {
        let parent = n.get_parent(compiler).unwrap();
        if parent.get_last_child(compiler) != Some(n) {
            let mut changed = false;
            while let Some(dead) = n.get_next(compiler) {
                if NodeUtil::is_function_declaration(compiler, dead) {
                    // Don't remove function declarations as they are hoisted
                    n = dead;
                    continue;
                }
                changed = true;
                NodeUtil::redeclare_vars_inside_branch(compiler, dead);
                self.redeclare_if_block_scoped_var(compiler, dead, parent);
                dead.detach(compiler);
                self.mark_functions_deleted(compiler, dead);
            }
            if changed {
                self.report_change_to_enclosing_scope(compiler, parent);
            }
        }
    }

    /// Redeclares the given node's names in the parent scope if they are block-scoped
    /// declarations; otherwise does nothing.
    // port: PeepholeRemoveDeadCode#redeclareIfBlockScopedVar
    fn redeclare_if_block_scoped_var(
        &mut self,
        compiler: &mut AbstractCompiler,
        decl: NodeId,
        parent: NodeId,
    ) {
        // This isn't called on function declarations ever: this method is only used in
        // removeFollowingNodes, which preserves function declarations as they're hoisted. This
        // preconditions check is to make sure nothing else tries reusing this code, since in that
        // case maybe they do want to handle function declarations.
        check_state!(
            !NodeUtil::is_function_declaration(compiler, decl),
            "Unexpected function declaration %s",
            decl.to_string(compiler)
        );
        if !NodeUtil::is_block_scoped_declaration(compiler, decl) {
            return;
        }
        let mut block_scoped_vars: Vec<NodeId> = Vec::new();
        if decl.is_class(compiler) {
            block_scoped_vars.push(decl.get_first_child(compiler).unwrap());
        } else {
            let ast: &mut Ast = compiler;
            NodeUtil::visit_lhs_nodes_in_node(ast, decl, &mut |ast: &mut Ast, n: NodeId| {
                if n.is_name(ast) {
                    block_scoped_vars.push(n);
                }
            });
        }

        let add_before =
            NodeUtil::get_insertion_point_after_all_inner_function_declarations(compiler, parent);
        for name_node in block_scoped_vars {
            let name_string = name_node.get_string(compiler);
            let name = IR::name(compiler, name_string).srcref(compiler, name_node);
            let var = IR::r#let(compiler, name).srcref(compiler, name_node);
            let var_name = var.get_first_child(compiler).unwrap();
            NodeUtil::copy_name_annotations(compiler, name_node, var_name);
            if let Some(add_before) = add_before {
                var.insert_before(compiler, add_before);
            } else {
                parent.add_child_to_back(compiler, var);
            }
        }
        self.add_feature_to_enclosing_script(Feature::LET_DECLARATIONS);
    }

    /// Some nodes that are unremovable don't have side effects so they aren't caught by
    /// mayHaveSideEffects
    // port: PeepholeRemoveDeadCode#isUnremovableNode
    fn is_unremovable_node(ast: &Ast, n: NodeId) -> bool {
        (n.is_block(ast) && n.is_synthetic_block(ast)) || n.is_script(ast)
    }

    // TODO(johnlenz): Consider moving this to a separate peephole pass.
    /// Attempt to replace the condition of if or hook immediately that is a reference to a name
    /// that is assigned immediately before.
    // port: PeepholeRemoveDeadCode#tryOptimizeConditionalAfterAssign
    fn try_optimize_conditional_after_assign(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        let next = n.get_next(compiler);

        // Look for patterns like the following and replace the if-condition with
        // a constant value so it can later be folded:
        //   var a = /a/;
        //   if (a) {foo(a)}
        // or
        //   a = 0;
        //   a ? foo(a) : c;
        // or
        //   a = 0;
        //   a || foo(a);
        // or
        //   a = 0;
        //   a && foo(a)
        // or
        //   a = 0;
        //   a ?? foo(a)
        // TODO(johnlenz): This would be better handled by control-flow sensitive
        // constant propagation. As the other case that I want to handle is:
        //   i=0; for(;i<0;i++){}
        // as right now nothing facilitates removing a loop like that.
        // This is here simply to remove the cruft left behind goog.userAgent and
        // similar cases.

        if !Self::is_simple_assignment(compiler, n) {
            return;
        }
        let Some(conditional_root) = self.get_conditional_root(compiler, next) else {
            return;
        };
        let lhs_assign = self.get_simple_assignment_name(compiler, n);

        let condition = self.get_conditional_statement_condition(compiler, next.unwrap());
        if !lhs_assign.matches_name_node(compiler, condition) {
            return;
        }
        let rhs_assign = self.get_simple_assignment_value(compiler, n);
        match conditional_root.get_token(compiler) {
            Token::AND | Token::OR | Token::IF | Token::HOOK => {
                // conditionals that coerce their condition to a boolean
                let value = NodeUtil::get_boolean_value(compiler, rhs_assign);
                if value != Tri::UNKNOWN {
                    let replacement_condition_node =
                        NodeUtil::boolean_node(compiler, value.to_boolean(true));
                    condition.replace_with(compiler, replacement_condition_node);
                    self.report_change_to_enclosing_scope(compiler, replacement_condition_node);
                }
            }
            Token::COALESCE => {
                // conditional that checks whether its operand is nullish
                let value_type = NodeUtil::get_known_value_type(compiler, rhs_assign);
                match value_type {
                    ValueType::NULL | ValueType::VOID => {
                        let undefined = NodeUtil::new_undefined_node(compiler, Some(condition));
                        condition.replace_with(compiler, undefined);
                        self.report_change_to_enclosing_scope(compiler, conditional_root);
                    }
                    ValueType::NUMBER
                    | ValueType::BIGINT
                    | ValueType::STRING
                    | ValueType::BOOLEAN
                    | ValueType::OBJECT => {
                        // semi-arbitrarily use '0' as a short non-nullish conditional
                        let zero = IR::number(compiler, 0.0).srcref(compiler, condition);
                        condition.replace_with(compiler, zero);
                        self.report_change_to_enclosing_scope(compiler, conditional_root);
                    }
                    ValueType::UNDETERMINED => {}
                }
            }
            _ => panic!(
                "Unhandled condition {}",
                conditional_root.to_string(compiler)
            ),
        }
    }

    /// Returns whether the node is a assignment to a simple name, or simple var declaration with
    /// initialization.
    // port: PeepholeRemoveDeadCode#isSimpleAssignment
    #[allow(clippy::if_same_then_else)] // Java's two branches, kept as written
    fn is_simple_assignment(ast: &Ast, n: NodeId) -> bool {
        // For our purposes we define a simple assignment to be a assignment
        // to a NAME node, or a VAR declaration with one child and a initializer.
        if NodeUtil::is_expr_assign(ast, n) && n.get_first_first_child(ast).unwrap().is_name(ast) {
            return true;
        } else if NodeUtil::is_name_declaration(ast, Some(n))
            && n.has_one_child(ast)
            && n.get_first_first_child(ast).is_some()
        {
            return true;
        }

        false
    }

    /// Returns the name being assigned to.
    // port: PeepholeRemoveDeadCode#getSimpleAssignmentName
    fn get_simple_assignment_name(&self, ast: &Ast, n: NodeId) -> NodeId {
        check_state!(Self::is_simple_assignment(ast, n));
        if NodeUtil::is_expr_assign(ast, n) {
            n.get_first_first_child(ast).unwrap()
        } else {
            // A var declaration.
            n.get_first_child(ast).unwrap()
        }
    }

    /// Returns the value assigned in the simple assignment
    // port: PeepholeRemoveDeadCode#getSimpleAssignmentValue
    fn get_simple_assignment_value(&self, ast: &Ast, n: NodeId) -> NodeId {
        check_state!(Self::is_simple_assignment(ast, n));
        n.get_first_child(ast).unwrap().get_last_child(ast).unwrap()
    }

    /// Returns the root node if a conditional statement or else null
    // port: PeepholeRemoveDeadCode#getConditionalRoot
    fn get_conditional_root(&self, ast: &Ast, n: Option<NodeId>) -> Option<NodeId> {
        // We defined a conditional statement to be a IF or EXPR_RESULT rooted with
        // a HOOK, AND, or OR node.
        let n = n?; // Java: `if (n == null) return null;`
        if n.is_if(ast) {
            return Some(n);
        } else if Self::is_expr_conditional(ast, n) {
            return n.get_first_child(ast);
        }
        None
    }

    /// Returns whether the node is a rooted with a HOOK, AND, OR, or COALESCE node.
    // port: PeepholeRemoveDeadCode#isExprConditional
    fn is_expr_conditional(ast: &Ast, n: NodeId) -> bool {
        if n.is_expr_result(ast) {
            match n.get_first_child(ast).unwrap().get_token(ast) {
                Token::HOOK | Token::AND | Token::OR | Token::COALESCE => {
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    /// Returns the condition of a conditional statement.
    // port: PeepholeRemoveDeadCode#getConditionalStatementCondition
    fn get_conditional_statement_condition(&self, ast: &Ast, n: NodeId) -> NodeId {
        if n.is_if(ast) {
            NodeUtil::get_condition_expression(ast, n).unwrap()
        } else {
            check_state!(Self::is_expr_conditional(ast, n));
            n.get_first_first_child(ast).unwrap()
        }
    }

    /// Try folding IF nodes by removing dead branches.
    ///
    /// Returns the replacement node, if changed, or the original if not
    // port: PeepholeRemoveDeadCode#tryFoldIf
    fn try_fold_if(&self, compiler: &mut AbstractCompiler, n: NodeId) -> Option<NodeId> {
        check_state!(n.is_if(compiler), "%s", n.to_string(compiler));
        let parent = n.get_parent(compiler);
        let parent = check_not_null!(parent);
        let r#type = n.get_token(compiler);
        let mut cond = n.get_first_child(compiler).unwrap();
        let mut then_body = cond.get_next(compiler).unwrap();
        let mut else_body = then_body.get_next(compiler);

        // if (x) { .. } else { } --> if (x) { ... }
        if let Some(else_node) = else_body
            && !self.may_have_side_effects(compiler, else_node)
        {
            else_node.detach(compiler);
            self.report_change_to_enclosing_scope(compiler, n);
            else_body = None;
        }

        // if (x) { } else { ... } --> if (!x) { ... }
        if !self.may_have_side_effects(compiler, then_body)
            && let Some(else_node) = else_body
        {
            else_node.detach(compiler);
            then_body.replace_with(compiler, else_node);
            let not_cond = compiler.new_node(Token::NOT);
            cond.replace_with(compiler, not_cond);
            self.report_change_to_enclosing_scope(compiler, n);
            not_cond.add_child_to_front(compiler, cond);
            cond = not_cond;
            then_body = cond.get_next(compiler).unwrap();
            else_body = None;
        }

        // `if (x()) { }` or `if (x?.()) { }`
        if !self.may_have_side_effects(compiler, then_body) && else_body.is_none() {
            if self.may_have_side_effects(compiler, cond) {
                // `x()` or `x?.()` has side effects, just leave the condition on its own.
                cond.detach(compiler);
                let replacement = NodeUtil::new_expr(compiler, cond);
                n.replace_with(compiler, replacement);
                self.report_change_to_enclosing_scope(compiler, parent);
                return Some(replacement);
            } else {
                // `x()` or `x?.()` has no side effects, the whole tree is useless now.
                NodeUtil::remove_child(compiler, parent, n);
                self.report_change_to_enclosing_scope(compiler, parent);
                return None;
            }
        }

        // Try transforms that apply to both IF and HOOK.
        let cond_value = NodeUtil::get_boolean_value(compiler, cond);
        if cond_value == Tri::UNKNOWN {
            return Some(n); // We can't remove branches otherwise!
        }

        if self.may_have_side_effects(compiler, cond) {
            // Transform "if (a = 2) {x =2}" into "if (true) {a=2;x=2}"
            let new_condition_value = cond_value == Tri::TRUE;
            // Add an elseBody if it is needed.
            if !new_condition_value && else_body.is_none() {
                let new_else = IR::block(compiler).srcref(compiler, n);
                n.add_child_to_back(compiler, new_else);
                else_body = Some(new_else);
            }
            let new_cond = NodeUtil::boolean_node(compiler, new_condition_value);
            cond.replace_with(compiler, new_cond);
            let branch_to_keep = if new_condition_value {
                then_body
            } else {
                else_body.unwrap()
            };
            let cond_statement = IR::expr_result(compiler, cond).srcref(compiler, cond);
            branch_to_keep.add_child_to_front(compiler, cond_statement);
            self.report_change_to_enclosing_scope(compiler, branch_to_keep);
            // Java then sets `cond = newCond;`, which nothing reads afterwards.
        }

        let cond_true = cond_value.to_boolean(true);
        if n.has_two_children(compiler) {
            check_state!(r#type == Token::IF);

            if cond_true {
                // Replace "if (true) { X }" with "X".
                let then_stmt = n.get_second_child(compiler).unwrap();
                then_stmt.detach(compiler);
                n.replace_with(compiler, then_stmt);
                self.report_change_to_enclosing_scope(compiler, then_stmt);
                Some(then_stmt)
            } else {
                // Remove "if (false) { X }" completely.
                NodeUtil::redeclare_vars_inside_branch(compiler, n);
                NodeUtil::remove_child(compiler, parent, n);
                self.report_change_to_enclosing_scope(compiler, parent);
                self.mark_functions_deleted(compiler, n);
                None
            }
        } else {
            // Replace "if (true) { X } else { Y }" with X, or
            // replace "if (false) { X } else { Y }" with Y.
            let true_branch = n.get_second_child(compiler).unwrap();
            let false_branch = true_branch.get_next(compiler).unwrap();
            let branch_to_keep = if cond_true { true_branch } else { false_branch };
            let branch_to_remove = if cond_true { false_branch } else { true_branch };
            NodeUtil::redeclare_vars_inside_branch(compiler, branch_to_remove);
            branch_to_keep.detach(compiler);
            n.replace_with(compiler, branch_to_keep);
            self.report_change_to_enclosing_scope(compiler, branch_to_keep);
            self.mark_functions_deleted(compiler, n);
            Some(branch_to_keep)
        }
    }

    /// Try folding HOOK (?:) if the condition results of the condition is known.
    ///
    /// Returns the replacement node, if changed, or the original if not
    // port: PeepholeRemoveDeadCode#tryFoldHook
    fn try_fold_hook(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        check_state!(n.is_hook(compiler), "%s", n.to_string(compiler));
        let parent = n.get_parent(compiler);
        check_not_null!(parent);
        let cond = n.get_first_child(compiler).unwrap();
        let then_body = cond.get_next(compiler).unwrap();
        let else_body = then_body.get_next(compiler).unwrap();

        let cond_value = NodeUtil::get_boolean_value(compiler, cond);
        // If the result nodes are equivalent, then one of the nodes can be
        // removed and it doesn't matter which.
        if cond_value == Tri::UNKNOWN
            && !self.are_nodes_equal_for_inlining(compiler, then_body, else_body)
        {
            return n; // We can't remove branches otherwise!
        }

        // Transform "(a = 2) ? x =2 : y" into "a=2,x=2"
        let branch_to_keep;
        let branch_to_remove;
        if cond_value.to_boolean(true) {
            branch_to_keep = then_body;
            branch_to_remove = else_body;
        } else {
            branch_to_keep = else_body;
            branch_to_remove = then_body;
        }

        let replacement;
        let cond_has_side_effects = self.may_have_side_effects(compiler, cond);
        // Must detach after checking for side effects, to ensure that the parents
        // of nodes are set correctly.
        n.detach_children(compiler);
        if cond_has_side_effects {
            replacement = IR::comma(compiler, cond, branch_to_keep).srcref(compiler, n);
        } else {
            replacement = branch_to_keep;
            self.mark_functions_deleted(compiler, cond);
        }

        n.replace_with(compiler, replacement);
        self.report_change_to_enclosing_scope(compiler, replacement);
        self.mark_functions_deleted(compiler, branch_to_remove);
        replacement
    }

    /// Removes FORs that always evaluate to false.
    // port: PeepholeRemoveDeadCode#tryFoldFor
    pub fn try_fold_for(&self, compiler: &mut AbstractCompiler, mut n: NodeId) -> Option<NodeId> {
        check_argument!(n.is_vanilla_for(compiler));

        let init = n.get_first_child(compiler).unwrap();
        let cond = init.get_next(compiler).unwrap();
        let increment = cond.get_next(compiler).unwrap();

        if !init.is_empty(compiler) && !NodeUtil::is_name_declaration(compiler, Some(init)) {
            // Java: `init = trySimplifyUnusedResult(init);`, read only by the null check.
            if self.try_simplify_unused_result(compiler, init).is_none() {
                let init = IR::empty(compiler).srcref(compiler, n);
                n.add_child_to_front(compiler, init);
            }
        }

        if !increment.is_empty(compiler) {
            // Java: `increment = trySimplifyUnusedResult(increment);`, read only by the null check.
            if self
                .try_simplify_unused_result(compiler, increment)
                .is_none()
            {
                let increment = IR::empty(compiler).srcref(compiler, n);
                increment.insert_after(compiler, cond);
            }
        }

        // There is an initializer skip it
        if !n.get_first_child(compiler).unwrap().is_empty(compiler) {
            return Some(n);
        }

        if NodeUtil::get_boolean_value(compiler, cond) != Tri::FALSE {
            return Some(n);
        }

        let mut parent = n.get_parent(compiler).unwrap();
        NodeUtil::redeclare_vars_inside_branch(compiler, n);

        if !self.may_have_side_effects(compiler, cond) {
            // Remove the entire loop and any associated labels.
            while parent.is_label(compiler) {
                n = parent;
                parent = parent.get_parent(compiler).unwrap();
            }
            n.detach(compiler);
        } else {
            let detached_cond = cond.detach(compiler);
            let mut statement =
                IR::expr_result(compiler, detached_cond).srcref_if_missing(compiler, cond);
            if parent.is_label(compiler) {
                let block = IR::block(compiler);
                block.srcref_if_missing(compiler, statement);
                block.add_child_to_front(compiler, statement);
                statement = block;
            }
            n.replace_with(compiler, statement);
        }
        self.report_change_to_enclosing_scope(compiler, parent);

        None
    }

    /// Removes DOs that always evaluate to false. This leaves the statements that were in the
    /// loop in a BLOCK node. The block will be removed in a later pass, if possible.
    // port: PeepholeRemoveDeadCode#tryFoldDoAway
    pub fn try_fold_do_away(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        check_argument!(n.is_do(compiler));

        let cond = NodeUtil::get_condition_expression(compiler, n).unwrap();
        if NodeUtil::get_boolean_value(compiler, cond) != Tri::FALSE {
            return n;
        }

        let block = NodeUtil::get_loop_code_block(compiler, n).unwrap();
        if n.get_parent(compiler).unwrap().is_label(compiler)
            || Self::has_unnamed_break_or_continue(compiler, block)
        {
            return n;
        }

        let parent = n.get_parent(compiler).unwrap();
        let detached_block = block.detach(compiler);
        n.replace_with(compiler, detached_block);
        if self.may_have_side_effects(compiler, cond) {
            let detached_cond = cond.detach(compiler);
            let cond_statement = IR::expr_result(compiler, detached_cond).srcref(compiler, cond);
            cond_statement.insert_after(compiler, block);
        }
        self.report_change_to_enclosing_scope(compiler, parent);

        block
    }

    /// Removes DOs that have empty bodies into FORs, which are much easier for the CFA to analyze.
    // port: PeepholeRemoveDeadCode#tryFoldEmptyDo
    pub fn try_fold_empty_do(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        check_argument!(n.is_do(compiler));

        let body = NodeUtil::get_loop_code_block(compiler, n).unwrap();
        if body.is_block(compiler) && !body.has_children(compiler) {
            let cond = NodeUtil::get_condition_expression(compiler, n).unwrap();
            let init = IR::empty(compiler).srcref(compiler, n);
            let detached_cond = cond.detach(compiler);
            let incr = IR::empty(compiler).srcref(compiler, n);
            let detached_body = body.detach(compiler);
            let for_node = IR::for_node(compiler, init, detached_cond, incr, detached_body);
            n.replace_with(compiler, for_node);
            self.report_change_to_enclosing_scope(compiler, for_node);
            return for_node;
        }
        n
    }

    /// Removes string keys with an empty pattern as their child
    // port: PeepholeRemoveDeadCode#tryOptimizeObjectPattern
    pub fn try_optimize_object_pattern(
        &self,
        compiler: &mut AbstractCompiler,
        pattern: NodeId,
    ) -> NodeId {
        check_argument!(
            pattern.is_object_pattern(compiler),
            "%s",
            pattern.to_string(compiler)
        );

        if pattern.has_children(compiler)
            && pattern.get_last_child(compiler).unwrap().is_rest(compiler)
        {
            // don't remove any elements in `const {f: [], ...rest} = obj` because that affects
            // what's assigned to `rest`. only the last element can be object rest.
            return pattern;
        }

        // remove trailing EMPTY nodes and empty destructuring patterns
        let mut child = pattern.get_first_child(compiler);
        while let Some(key) = child {
            child = key.get_next(compiler); // don't put this in the for loop since we might remove `child`

            if !key.is_string_key(compiler) {
                // don't try to remove rest or computed properties, since they might have side
                // effects
                continue;
            }
            let only_child = key.get_only_child(compiler);
            if self.is_removable_destructuring_target(compiler, only_child) {
                // e.g. `const {f: {}} = obj;`
                key.detach(compiler);
                self.report_change_to_enclosing_scope(compiler, pattern);
            }
        }
        pattern
    }

    /// Removes trailing EMPTY nodes and empty array patterns
    // port: PeepholeRemoveDeadCode#tryOptimizeArrayPattern
    pub fn try_optimize_array_pattern(
        &self,
        compiler: &mut AbstractCompiler,
        pattern: NodeId,
    ) -> NodeId {
        check_argument!(
            pattern.is_array_pattern(compiler),
            "%s",
            pattern.to_string(compiler)
        );

        let mut last_child = pattern.get_last_child(compiler);
        while let Some(last) = last_child {
            if last.is_empty(compiler) || self.is_removable_destructuring_target(compiler, last) {
                let prev = last.get_previous(compiler);
                last.detach(compiler);
                last_child = prev;
                self.report_change_to_enclosing_scope(compiler, pattern);
            } else {
                // don't remove any non-trailing empty nodes because that will change the ordering
                // of the other assignments
                // note that this case also covers array pattern rest, which must be the final
                // element
                break;
            }
        }
        pattern
    }

    // port: PeepholeRemoveDeadCode#isRemovableDestructuringTarget
    fn is_removable_destructuring_target(
        &self,
        compiler: &mut AbstractCompiler,
        destructruring_element: NodeId,
    ) -> bool {
        let mut target = destructruring_element;
        let mut default_value = None;
        if destructruring_element.is_default_value(compiler) {
            target = destructruring_element.get_first_child(compiler).unwrap();
            default_value = destructruring_element.get_second_child(compiler);
        }
        if !target.is_destructuring_pattern(compiler) || target.has_children(compiler) {
            return false;
        }
        // only remove default values without side effects
        default_value
            .is_none_or(|default_value| !self.may_have_side_effects(compiler, default_value))
    }

    /// Returns whether a node has any unhandled breaks or continue.
    // port: PeepholeRemoveDeadCode#hasUnnamedBreakOrContinue
    pub fn has_unnamed_break_or_continue(ast: &Ast, n: NodeId) -> bool {
        NodeUtil::has(
            ast,
            n,
            // Check for unlabeled breaks
            &|ast: &Ast, node: NodeId| node.is_break(ast) && !node.has_children(ast),
            // ...inside contexts that can contain breaks.
            &|ast: &Ast, node: NodeId| {
                !IR::may_be_expression(ast, node) // Functions are not visited
                    && !NodeUtil::is_loop_structure(ast, node)
                    && !node.is_switch(ast)
            },
        ) || NodeUtil::has(
            ast,
            n,
            // Check for unlabeled continues
            &|ast: &Ast, node: NodeId| node.is_continue(ast) && !node.has_children(ast),
            // ...inside contexts that can contain continues.
            &|ast: &Ast, node: NodeId| {
                !IR::may_be_expression(ast, node) // Functions are not visited
                    && !NodeUtil::is_loop_structure(ast, node)
            },
        )
    }

    /// Remove always true loop conditions.
    // port: PeepholeRemoveDeadCode#tryFoldForCondition
    fn try_fold_for_condition(&self, compiler: &mut AbstractCompiler, for_condition: NodeId) {
        if self.get_side_effect_free_boolean_value(compiler, for_condition) == Tri::TRUE {
            self.report_change_to_enclosing_scope(compiler, for_condition);
            let empty = IR::empty(compiler);
            for_condition.replace_with(compiler, empty);
        }
    }

    // port: PeepholeRemoveDeadCode#tryRemoveOptionalChaining
    fn try_remove_optional_chaining(
        &self,
        compiler: &mut AbstractCompiler,
        optional_chain: NodeId,
    ) -> NodeId {
        let callee = optional_chain.get_first_child(compiler).unwrap();
        if !NodeUtil::is_null_or_undefined(compiler, callee) {
            return optional_chain;
        }
        let result;
        if self.may_have_side_effects(compiler, callee) {
            // Simplify `(void sideEffectFunction())?.()` to `(void sideEffectFunction())`
            // The optional chain call won't execute but sideEffectFunction() is still evaluated.
            let detached_callee = callee.detach(compiler);
            optional_chain.replace_with(compiler, detached_callee);
            result = callee;
        } else {
            // Remove `(void 0)?.()` and (null)?.() and simplify `(void 0)?.x and null?.x` to void 0
            result = NodeUtil::new_undefined_node(compiler, Some(callee));
            optional_chain.replace_with(compiler, result);
        }
        self.mark_functions_deleted(compiler, optional_chain);
        self.report_change_to_enclosing_scope(compiler, result);
        result
    }
}

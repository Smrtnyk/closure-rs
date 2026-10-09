/*
 * Copyright 2010 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/PeepholeMinimizeConditions.java.

//! Port of `PeepholeMinimizeConditions.java`.
//!
//! A peephole optimization that minimizes conditional expressions according to De Morgan's laws.
//! Also rewrites conditional statements as expressions by replacing them with HOOKs and
//! short-circuit binary operators.
//!
//! Based on PeepholeSubstituteAlternateSyntax

use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::{
        AbstractPeepholeOptimization, AbstractPeepholeOptimizationFields,
    },
    control_flow_analysis::ControlFlowAnalysis,
    minimized_condition::{MeasuredNode, MinimizationStyle, MinimizedCondition},
    node_util::NodeUtil,
};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    jscomp_base::Tri,
    node::{Ast, NodeId},
    token::Token,
};

/// Java's `private static final int AND_PRECEDENCE = NodeUtil.precedence(Token.AND)`.
fn and_precedence() -> i32 {
    NodeUtil::precedence(Token::AND)
}

pub struct PeepholeMinimizeConditions {
    fields: AbstractPeepholeOptimizationFields,
    late: bool,
}

impl PeepholeMinimizeConditions {
    /// `late` When late is false, this mean we are currently running before most of the other
    /// optimizations. In this case we would avoid optimizations that would make the code harder
    /// to analyze (such as using string splitting, merging statements with commas, etc). When
    /// late is true, we would do anything to minimize for size.
    // port: PeepholeMinimizeConditions#PeepholeMinimizeConditions
    pub fn new(late: bool) -> Self {
        Self {
            fields: AbstractPeepholeOptimizationFields::new(),
            late,
        }
    }

    // port: PeepholeMinimizeConditions#tryJoinForCondition
    fn try_join_for_condition(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        if !self.late {
            return;
        }

        let block = n.get_last_child(compiler).unwrap();
        let maybe_if = block.get_first_child(compiler);
        if let Some(maybe_if) = maybe_if
            && maybe_if.is_if(compiler)
        {
            let then_block = maybe_if.get_second_child(compiler).unwrap();
            let maybe_break = then_block.get_first_child(compiler);
            if let Some(maybe_break) = maybe_break
                && maybe_break.is_break(compiler)
                && !maybe_break.has_children(compiler)
            {
                // Preserve the IF ELSE expression is there is one.
                if maybe_if.has_x_children(compiler, 3) {
                    let last = maybe_if.get_last_child(compiler).unwrap().detach(compiler);
                    maybe_if.replace_with(compiler, last);
                } else {
                    NodeUtil::redeclare_vars_inside_branch(compiler, then_block);
                    block.remove_first_child(compiler);
                }

                let if_condition = maybe_if.remove_first_child(compiler).unwrap();
                let fixed_if_condition =
                    IR::not(compiler, if_condition).srcref(compiler, if_condition);

                // OK, join the IF expression with the FOR expression
                let for_condition = NodeUtil::get_condition_expression(compiler, n).unwrap();
                if for_condition.is_empty(compiler) {
                    for_condition.replace_with(compiler, fixed_if_condition);
                    self.report_change_to_enclosing_scope(compiler, fixed_if_condition);
                } else {
                    let replacement = compiler.new_node(Token::AND);
                    for_condition.replace_with(compiler, replacement);
                    replacement.add_child_to_back(compiler, for_condition);
                    replacement.add_child_to_back(compiler, fixed_if_condition);
                    self.report_change_to_enclosing_scope(compiler, replacement);
                }
            }
        }
    }

    /// Use "return x?1:2;" in place of "if(x)return 1;return 2;"
    // port: PeepholeMinimizeConditions#tryReplaceIf
    fn try_replace_if(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        let mut next;
        let mut child = n.get_first_child(compiler);
        while let Some(c) = child {
            next = c.get_next(compiler);
            if c.is_if(compiler) {
                let cond = c.get_first_child(compiler).unwrap();
                let then_branch = cond.get_next(compiler).unwrap();
                let else_branch = then_branch.get_next(compiler);
                let next_node = c.get_next(compiler);

                if let Some(next_node) = next_node
                    && else_branch.is_none()
                    && Self::is_return_block(compiler, then_branch)
                    && next_node.is_if(compiler)
                {
                    let next_cond = next_node.get_first_child(compiler).unwrap();
                    let next_then = next_cond.get_next(compiler).unwrap();
                    let next_else = next_then.get_next(compiler);
                    if self.are_nodes_equal_for_inlining(compiler, then_branch, next_then) {
                        // Transform
                        //   if (x) return 1; if (y) return 1;
                        // to
                        //   if (x||y) return 1;
                        c.detach(compiler);
                        c.detach_children(compiler);
                        let new_cond = compiler.new_node_with_child(Token::OR, cond);
                        next_cond.replace_with(compiler, new_cond);
                        new_cond.add_child_to_back(compiler, next_cond);
                        self.report_change_to_enclosing_scope(compiler, new_cond);
                    } else if let Some(next_else) = next_else
                        && self.are_nodes_equal_for_inlining(compiler, then_branch, next_else)
                    {
                        // Transform
                        //   if (x) return 1; if (y) foo() else return 1;
                        // to
                        //   if (!x&&y) foo() else return 1;
                        c.detach(compiler);
                        c.detach_children(compiler);
                        let not_cond = IR::not(compiler, cond).srcref(compiler, cond);
                        let new_cond = compiler.new_node_with_child(Token::AND, not_cond);
                        next_cond.replace_with(compiler, new_cond);
                        new_cond.add_child_to_back(compiler, next_cond);
                        self.report_change_to_enclosing_scope(compiler, new_cond);
                    }
                } else if let Some(next_node) = next_node
                    && else_branch.is_none()
                    && Self::is_return_block(compiler, then_branch)
                    && Self::is_return_expression(compiler, next_node)
                {
                    let then_expr;
                    // if(x)return; return 1 -> return x?void 0:1
                    if Self::is_return_express_block(compiler, then_branch) {
                        then_expr = Self::get_block_return_expression(compiler, then_branch);
                        then_expr.detach(compiler);
                    } else {
                        then_expr = NodeUtil::new_undefined_node(compiler, Some(c));
                    }

                    let else_expr = next_node.get_first_child(compiler).unwrap();

                    cond.detach(compiler);
                    else_expr.detach(compiler);

                    let hook = IR::hook(compiler, cond, then_expr, else_expr).srcref(compiler, c);
                    let return_node = IR::return_node_with_expression(compiler, hook);
                    c.replace_with(compiler, return_node);
                    next_node.detach(compiler);
                    self.report_change_to_enclosing_scope(compiler, n);
                    // everything else in the block is dead code.
                    break;
                } else if let Some(else_branch) = else_branch
                    && Self::statement_must_exit_parent(compiler, then_branch)
                {
                    else_branch.detach(compiler);
                    else_branch.insert_after(compiler, c);
                    self.report_change_to_enclosing_scope(compiler, n);
                }
            }
            child = next;
        }
        n
    }

    // port: PeepholeMinimizeConditions#statementMustExitParent
    fn statement_must_exit_parent(ast: &Ast, n: NodeId) -> bool {
        match n.get_token(ast) {
            Token::THROW | Token::RETURN => true,
            Token::BLOCK => {
                if n.has_children(ast) {
                    let child = n.get_last_child(ast).unwrap();
                    return Self::statement_must_exit_parent(ast, child);
                }
                false
            }
            // TODO(johnlenz): handle TRY/FINALLY
            _ => false,
        }
    }

    /// Replace duplicate exits in control structures. If the node following the exit node
    /// expression has the same effect as exit node, the node can be replaced or removed. For
    /// example: "while (a) {return f()} return f();" ==> "while (a) {break} return f();"
    /// "while (a) {throw 'ow'} throw 'ow';" ==> "while (a) {break} throw 'ow';"
    ///
    /// `n` An follow control exit expression (a THROW or RETURN node). Returns the replacement
    /// for n, or the original if no change was made.
    // port: PeepholeMinimizeConditions#tryReplaceExitWithBreak
    fn try_replace_exit_with_break(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        let result = n.get_first_child(compiler);

        // Find the enclosing control structure, if any, that a "break" would exit
        // from.
        let mut break_target = n;
        while !ControlFlowAnalysis::is_break_target(
            compiler,
            break_target,
            None, /* no label */
        ) {
            if break_target.is_function(compiler) || break_target.is_script(compiler) {
                // No break target.
                return n;
            }
            break_target = break_target.get_parent(compiler).unwrap();
        }

        let mut follow = ControlFlowAnalysis::compute_follow_node(compiler, break_target);

        // Skip pass all the finally blocks because both the break and return will
        // also trigger all the finally blocks. However, the order of execution is
        // slightly changed. Consider:
        //
        // return a() -> finally { b() } -> return a()
        //
        // which would call a() first. However, changing the first return to a
        // break will result in calling b().

        let prefinally_follows = follow;
        follow = Self::skip_finally_nodes(compiler, follow);

        let inner_follow = ControlFlowAnalysis::compute_follow_node(compiler, n);
        if prefinally_follows != follow
            || inner_follow != Self::skip_finally_nodes(compiler, inner_follow)
        {
            // There were finally clauses
            if !self.is_pure(compiler, result) {
                // Can't defer the exit
                return n;
            }
        }

        if follow.is_none() && (n.is_throw(compiler) || result.is_some()) {
            // Can't complete remove a throw here or a return with a result.
            return n;
        }

        // When follow is null, this mean the follow of a break target is the
        // end of a function. This means a break is same as return.
        if follow.is_none() || self.are_matching_exits(compiler, n, follow.unwrap()) {
            let replacement = IR::break_node(compiler);
            n.replace_with(compiler, replacement);
            self.report_change_to_enclosing_scope(compiler, replacement);
            return replacement;
        }

        n
    }

    /// Remove duplicate exits. If the node following the exit node expression has the same
    /// effect as exit node, the node can be removed. For example: "if (a) {return f()} return
    /// f();" ==> "if (a) {} return f();" "if (a) {throw 'ow'} throw 'ow';" ==> "if (a) {} throw
    /// 'ow';"
    ///
    /// `n` An follow control exit expression (a THROW or RETURN node). Returns the replacement
    /// for n, or the original if no change was made.
    // port: PeepholeMinimizeConditions#tryRemoveRedundantExit
    fn try_remove_redundant_exit(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> Option<NodeId> {
        let exit_expr = n.get_first_child(compiler);

        let mut follow = ControlFlowAnalysis::compute_follow_node(compiler, n);

        // Skip pass all the finally blocks because both the fall through and return
        // will also trigger all the finally blocks.
        let prefinally_follows = follow;
        follow = Self::skip_finally_nodes(compiler, follow);
        if prefinally_follows != follow {
            // There were finally clauses
            if !self.is_pure(compiler, exit_expr) {
                // Can't replace the return
                return Some(n);
            }
        }

        if follow.is_none() && (n.is_throw(compiler) || exit_expr.is_some()) {
            // Can't complete remove a throw here or a return with a result.
            return Some(n);
        }

        // When follow is null, this mean the follow of a break target is the
        // end of a function. This means a break is same as return.
        if follow.is_none() || self.are_matching_exits(compiler, n, follow.unwrap()) {
            self.report_change_to_enclosing_scope(compiler, n);
            n.detach(compiler);
            return None;
        }

        Some(n)
    }

    /// Returns whether the expression does not produces and can not be affected by side-effects.
    // port: PeepholeMinimizeConditions#isPure
    pub fn is_pure(&self, compiler: &mut AbstractCompiler, n: Option<NodeId>) -> bool {
        match n {
            None => true,
            Some(n) => {
                !NodeUtil::can_be_side_effected(compiler, n)
                    && !self.may_have_side_effects(compiler, n)
            }
        }
    }

    /// Returns n or the node following any following finally nodes.
    // port: PeepholeMinimizeConditions#skipFinallyNodes
    pub fn skip_finally_nodes(
        compiler: &AbstractCompiler,
        mut n: Option<NodeId>,
    ) -> Option<NodeId> {
        while let Some(node) = n
            && NodeUtil::is_try_finally_node(compiler, node.get_parent(compiler).unwrap(), node)
        {
            n = ControlFlowAnalysis::compute_follow_node(compiler, node);
        }
        n
    }

    /// Check whether one exit can be replaced with another. Verify: 1) They are identical
    /// expressions 2) If an exception is possible that the statements, the original and the
    /// potential replacement are in the same exception handler.
    // port: PeepholeMinimizeConditions#areMatchingExits
    pub fn are_matching_exits(
        &self,
        compiler: &AbstractCompiler,
        node_this: NodeId,
        node_that: NodeId,
    ) -> bool {
        if !self.is_ast_normalized(compiler)
            && (node_this.is_throw(compiler) || node_this.is_return(compiler))
            && node_this.has_children(compiler)
        {
            // if the ast isn't normalized "return a" or "throw a" may not mean the same thing in
            // different blocks.
            return false;
        }
        node_this.is_equivalent_to(compiler, node_that)
            && (!Self::is_exception_possible(compiler, node_this)
                || Self::get_exception_handler(compiler, node_this)
                    == Self::get_exception_handler(compiler, node_that))
    }

    // port: PeepholeMinimizeConditions#isExceptionPossible
    pub fn is_exception_possible(ast: &Ast, n: NodeId) -> bool {
        // TODO(johnlenz): maybe use ControlFlowAnalysis.mayThrowException?
        check_state!(n.is_return(ast) || n.is_throw(ast), "%s", n.to_string(ast));
        n.is_throw(ast)
            || (n.has_children(ast)
                && !NodeUtil::is_literal_value(ast, n.get_last_child(ast).unwrap(), true))
    }

    // port: PeepholeMinimizeConditions#getExceptionHandler
    pub fn get_exception_handler(compiler: &AbstractCompiler, n: NodeId) -> Option<NodeId> {
        ControlFlowAnalysis::get_exception_handler(compiler, n)
    }

    /// Try to minimize NOT nodes such as !(x==y).
    ///
    /// Returns the replacement for n or the original if no change was made
    // port: PeepholeMinimizeConditions#tryMinimizeNot
    fn try_minimize_not(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        check_argument!(n.is_not(compiler));
        let parent = n.get_parent(compiler).unwrap();

        let not_child = n.get_first_child(compiler).unwrap();
        // negative operator of the current one : == -> != for instance.
        let complement_operator = match not_child.get_token(compiler) {
            Token::EQ => Token::NE,
            Token::NE => Token::EQ,
            Token::SHEQ => Token::SHNE,
            Token::SHNE => Token::SHEQ,
            // GT, GE, LT, LE are not handled in this because !(x<NaN) != x>=NaN.
            _ => {
                return n;
            }
        };
        let new_operator = n.remove_first_child(compiler).unwrap();
        new_operator.set_token(compiler, complement_operator);
        n.replace_with(compiler, new_operator);
        self.report_change_to_enclosing_scope(compiler, parent);
        new_operator
    }

    /// Try to remove leading NOTs from EXPR_RESULTS.
    ///
    /// Returns the replacement for n or the original if no replacement was necessary.
    // port: PeepholeMinimizeConditions#tryMinimizeExprResult
    fn try_minimize_expr_result(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        let original_expr = n.get_first_child(compiler).unwrap();
        let min_cond = MinimizedCondition::from_condition_node(compiler, original_expr);
        let m_node = min_cond.get_minimized(compiler, MinimizationStyle::ALLOW_LEADING_NOT);
        if m_node.is_not(compiler) {
            // Remove the leading NOT in the EXPR_RESULT.
            let without_not = m_node.without_not(compiler);
            self.replace_node(compiler, original_expr, &without_not);
        } else {
            self.replace_node(compiler, original_expr, &m_node);
        }
        n
    }

    /// Try flipping HOOKs that have negated conditions.
    ///
    /// Returns the replacement for n or the original if no replacement was necessary.
    // port: PeepholeMinimizeConditions#tryMinimizeHook
    fn try_minimize_hook(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        let original_cond = n.get_first_child(compiler).unwrap();
        let min_cond = MinimizedCondition::from_condition_node(compiler, original_cond);
        let m_node = min_cond.get_minimized(compiler, MinimizationStyle::ALLOW_LEADING_NOT);
        if m_node.is_not(compiler) {
            // Swap the HOOK
            let then_branch = n.get_second_child(compiler).unwrap();
            let without_not = m_node.without_not(compiler);
            self.replace_node(compiler, original_cond, &without_not);
            then_branch.detach(compiler);
            n.add_child_to_back(compiler, then_branch);
            self.report_change_to_enclosing_scope(compiler, n);
        } else {
            self.replace_node(compiler, original_cond, &m_node);
        }
        n
    }

    /// Try turning IF nodes into smaller HOOKs
    ///
    /// Returns the replacement for n or the original if no replacement was necessary.
    // port: PeepholeMinimizeConditions#tryMinimizeIf
    fn try_minimize_if(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        let parent = n.get_parent(compiler).unwrap();

        let original_cond = n.get_first_child(compiler).unwrap();

        // If the condition is a literal, we'll let other
        // optimizations try to remove useless code.
        if NodeUtil::is_literal_value(compiler, original_cond, true) {
            return n;
        }

        let then_branch = original_cond.get_next(compiler).unwrap();
        let else_branch = then_branch.get_next(compiler);

        let min_cond = MinimizedCondition::from_condition_node(compiler, original_cond);

        // Compute two minimized representations. The first representation counts
        // a leading NOT node, and the second ignores a leading NOT node.
        // If we can fold the if statement into a HOOK or boolean operation,
        // then the NOT node does not matter, and we prefer the second condition.
        // If we cannot fold the if statement, then we prefer the first condition.
        let unnegated_cond = min_cond.get_minimized(compiler, MinimizationStyle::PREFER_UNNEGATED);
        let short_cond = min_cond.get_minimized(compiler, MinimizationStyle::ALLOW_LEADING_NOT);

        let Some(else_branch) = else_branch else {
            if Self::is_foldable_express_block(compiler, then_branch) {
                let expr = Self::get_block_expression(compiler, then_branch);
                if !self.late && Self::is_property_assignment_in_expression(compiler, expr) {
                    // Keep opportunities for CollapseProperties such as
                    // a.longIdentifier || a.longIdentifier = ... -> var a = ...;
                    // until CollapseProperties has been run.
                    self.replace_node(compiler, original_cond, &unnegated_cond);
                    return n;
                }

                if short_cond.is_not(compiler) {
                    // if(!x)bar(); -> x||bar();
                    let without_not = short_cond.without_not(compiler);
                    let replacement_cond = self
                        .replace_node(compiler, original_cond, &without_not)
                        .detach(compiler);
                    let expr_first = expr.remove_first_child(compiler).unwrap();
                    let or = IR::or(compiler, replacement_cond, expr_first).srcref(compiler, n);
                    let new_expr = NodeUtil::new_expr(compiler, or);
                    n.replace_with(compiler, new_expr);
                    self.report_change_to_enclosing_scope(compiler, parent);

                    return new_expr;
                }
                // True, but removed for performance reasons.
                // Preconditions.checkState(shortCond.isEquivalentTo(unnegatedCond));

                // if(x)foo(); -> x&&foo();
                if short_cond.is_lower_precedence_than(compiler, and_precedence())
                    && Self::is_lower_precedence(
                        compiler,
                        expr.get_first_child(compiler).unwrap(),
                        and_precedence(),
                    )
                {
                    // One additional set of parentheses is worth the change even if
                    // there is no immediate code size win. However, two extra pair of
                    // {}, we would have to think twice. (unless we know for sure the
                    // we can further optimize its parent.
                    self.replace_node(compiler, original_cond, &short_cond);
                    return n;
                }

                let replacement_cond = self
                    .replace_node(compiler, original_cond, &short_cond)
                    .detach(compiler);
                let expr_first = expr.remove_first_child(compiler).unwrap();
                let and = IR::and(compiler, replacement_cond, expr_first).srcref(compiler, n);
                let new_expr = NodeUtil::new_expr(compiler, and);
                n.replace_with(compiler, new_expr);
                self.report_change_to_enclosing_scope(compiler, parent);

                return new_expr;
            } else {
                // Try to combine two IF-ELSE
                if NodeUtil::is_statement_block(compiler, then_branch)
                    && then_branch.has_one_child(compiler)
                {
                    let inner_if = then_branch.get_first_child(compiler).unwrap();

                    if inner_if.is_if(compiler) {
                        let inner_cond = inner_if.get_first_child(compiler).unwrap();
                        let inner_then_branch = inner_cond.get_next(compiler).unwrap();
                        let inner_else_branch = inner_then_branch.get_next(compiler);

                        if inner_else_branch.is_none()
                            && !(unnegated_cond
                                .is_lower_precedence_than(compiler, and_precedence())
                                && Self::is_lower_precedence(
                                    compiler,
                                    inner_cond,
                                    and_precedence(),
                                ))
                        {
                            let replacement_cond = self
                                .replace_node(compiler, original_cond, &unnegated_cond)
                                .detach(compiler);
                            n.detach_children(compiler);
                            let inner_cond = inner_cond.detach(compiler);
                            let and = IR::and(compiler, replacement_cond, inner_cond)
                                .srcref(compiler, original_cond);
                            n.add_child_to_back(compiler, and);
                            let inner_then_branch = inner_then_branch.detach(compiler);
                            n.add_child_to_back(compiler, inner_then_branch);
                            self.report_change_to_enclosing_scope(compiler, n);
                            // Not worth trying to fold the current IF-ELSE into && because
                            // the inner IF-ELSE wasn't able to be folded into && anyways.
                            return n;
                        }
                    }
                }
            }
            self.replace_node(compiler, original_cond, &unnegated_cond);
            return n;
        };

        // TODO(dcc) This modifies the siblings of n, which is undesirable for a
        // peephole optimization. This should probably get moved to another pass.
        self.try_remove_repeated_statements(compiler, n);

        // if(!x)foo();else bar(); -> if(x)bar();else foo();
        // An additional set of curly braces isn't worth it.
        if short_cond.is_not(compiler) && !Self::consumes_dangling_else(compiler, else_branch) {
            let without_not = short_cond.without_not(compiler);
            self.replace_node(compiler, original_cond, &without_not);
            then_branch.detach(compiler);
            n.add_child_to_back(compiler, then_branch);
            self.report_change_to_enclosing_scope(compiler, n);
            return n;
        }

        // if(x)return 1;else return 2; -> return x?1:2;
        if Self::is_return_express_block(compiler, then_branch)
            && Self::is_return_express_block(compiler, else_branch)
        {
            let then_expr = Self::get_block_return_expression(compiler, then_branch);
            let else_expr = Self::get_block_return_expression(compiler, else_branch);

            let replacement_cond = self
                .replace_node(compiler, original_cond, &short_cond)
                .detach(compiler);
            then_expr.detach(compiler);
            else_expr.detach(compiler);

            // note - we ignore any cases with "return;", technically this
            // can be converted to "return undefined;" or some variant, but
            // that does not help code size.
            let hook =
                IR::hook(compiler, replacement_cond, then_expr, else_expr).srcref(compiler, n);
            let return_node = IR::return_node_with_expression(compiler, hook);
            n.replace_with(compiler, return_node);
            self.report_change_to_enclosing_scope(compiler, return_node);
            return return_node;
        }

        let then_branch_is_expression_block =
            Self::is_foldable_express_block(compiler, then_branch);
        let else_branch_is_expression_block =
            Self::is_foldable_express_block(compiler, else_branch);

        if then_branch_is_expression_block && else_branch_is_expression_block {
            let then_op = Self::get_block_expression(compiler, then_branch)
                .get_first_child(compiler)
                .unwrap();
            let else_op = Self::get_block_expression(compiler, else_branch)
                .get_first_child(compiler)
                .unwrap();
            if then_op.get_token(compiler) == else_op.get_token(compiler) {
                // if(x)a=1;else a=2; -> a=x?1:2;
                if NodeUtil::is_assignment_op(compiler, then_op) {
                    let lhs = then_op.get_first_child(compiler).unwrap();
                    let else_lhs = else_op.get_first_child(compiler).unwrap();
                    if self.are_nodes_equal_for_inlining(compiler, lhs, else_lhs)
                        // if LHS has side effects, don't proceed [since the optimization
                        // evaluates LHS before cond]
                        // NOTE - there are some circumstances where we can
                        // proceed even if there are side effects...
                        && !self.may_effect_mutable_state(compiler, lhs)
                        && (!self.may_have_side_effects(compiler, original_cond)
                            || (then_op.is_assign(compiler)
                                && then_op.get_first_child(compiler).unwrap().is_name(compiler)))
                    {
                        let replacement_cond = self
                            .replace_node(compiler, original_cond, &short_cond)
                            .detach(compiler);
                        let assign_name = then_op.remove_first_child(compiler).unwrap();
                        let then_expr = then_op.remove_first_child(compiler).unwrap();
                        let else_expr = else_op.get_last_child(compiler).unwrap();
                        else_expr.detach(compiler);

                        let hook_node = IR::hook(compiler, replacement_cond, then_expr, else_expr)
                            .srcref(compiler, n);
                        let then_op_token = then_op.get_token(compiler);
                        let assign = compiler
                            .new_node_with_children2(then_op_token, assign_name, hook_node)
                            .srcref(compiler, then_op);
                        let expr = NodeUtil::new_expr(compiler, assign);
                        n.replace_with(compiler, expr);
                        self.report_change_to_enclosing_scope(compiler, parent);

                        return expr;
                    }
                }
            }
            // if(x)foo();else bar(); -> x?foo():bar()
            let replacement_cond = self
                .replace_node(compiler, original_cond, &short_cond)
                .detach(compiler);
            then_op.detach(compiler);
            else_op.detach(compiler);
            let hook = IR::hook(compiler, replacement_cond, then_op, else_op).srcref(compiler, n);
            let expr = IR::expr_result(compiler, hook);
            n.replace_with(compiler, expr);
            self.report_change_to_enclosing_scope(compiler, parent);
            return expr;
        }

        let then_branch_is_var = Self::is_var_block(compiler, then_branch);
        let else_branch_is_var = Self::is_var_block(compiler, else_branch);

        // if(x)var y=1;else y=2  ->  var y=x?1:2
        if then_branch_is_var
            && else_branch_is_expression_block
            && Self::get_block_expression(compiler, else_branch)
                .get_first_child(compiler)
                .unwrap()
                .is_assign(compiler)
        {
            let var = Self::get_block_var(compiler, then_branch);
            let else_assign = Self::get_block_expression(compiler, else_branch)
                .get_first_child(compiler)
                .unwrap();

            let name1 = var.get_first_child(compiler).unwrap();
            let maybe_name2 = else_assign.get_first_child(compiler).unwrap();

            if name1.has_children(compiler)
                && maybe_name2.is_name(compiler)
                && name1.get_string(compiler) == maybe_name2.get_string(compiler)
            {
                check_state!(name1.has_one_child(compiler));
                let then_expr = name1.remove_first_child(compiler).unwrap();
                let else_expr = else_assign
                    .get_last_child(compiler)
                    .unwrap()
                    .detach(compiler);
                let replacement_cond = self
                    .replace_node(compiler, original_cond, &short_cond)
                    .detach(compiler);
                let hook_node =
                    IR::hook(compiler, replacement_cond, then_expr, else_expr).srcref(compiler, n);
                var.detach(compiler);
                name1.add_child_to_back(compiler, hook_node);
                n.replace_with(compiler, var);
                self.report_change_to_enclosing_scope(compiler, parent);
                return var;
            }

            // if(x)y=1;else var y=2  ->  var y=x?1:2
        } else if else_branch_is_var
            && then_branch_is_expression_block
            && Self::get_block_expression(compiler, then_branch)
                .get_first_child(compiler)
                .unwrap()
                .is_assign(compiler)
        {
            let var = Self::get_block_var(compiler, else_branch);
            let then_assign = Self::get_block_expression(compiler, then_branch)
                .get_first_child(compiler)
                .unwrap();

            let maybe_name1 = then_assign.get_first_child(compiler).unwrap();
            let name2 = var.get_first_child(compiler).unwrap();

            if name2.has_children(compiler)
                && maybe_name1.is_name(compiler)
                && maybe_name1.get_string(compiler) == name2.get_string(compiler)
            {
                let then_expr = then_assign
                    .get_last_child(compiler)
                    .unwrap()
                    .detach(compiler);
                check_state!(name2.has_one_child(compiler));
                let else_expr = name2.remove_first_child(compiler).unwrap();
                let replacement_cond = self
                    .replace_node(compiler, original_cond, &short_cond)
                    .detach(compiler);
                let hook_node =
                    IR::hook(compiler, replacement_cond, then_expr, else_expr).srcref(compiler, n);
                var.detach(compiler);
                name2.add_child_to_back(compiler, hook_node);
                n.replace_with(compiler, var);
                self.report_change_to_enclosing_scope(compiler, parent);

                return var;
            }
        }

        self.replace_node(compiler, original_cond, &unnegated_cond);
        n
    }

    /// Try to remove duplicate statements from IF blocks. For example:
    ///
    /// ```text
    /// if (a) {
    ///   x = 1;
    ///   return true;
    /// } else {
    ///   x = 2;
    ///   return true;
    /// }
    /// ```
    ///
    /// becomes:
    ///
    /// ```text
    /// if (a) {
    ///   x = 1;
    /// } else {
    ///   x = 2;
    /// }
    /// return true;
    /// ```
    ///
    /// `n` The IF node to examine.
    // port: PeepholeMinimizeConditions#tryRemoveRepeatedStatements
    fn try_remove_repeated_statements(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        // Only run this if variable names are guaranteed to be unique. Otherwise bad things can
        // happen: see PeepholeMinimizeConditionsTest#testDontRemoveDuplicateStatementsWithoutNormalization
        if !self.is_ast_normalized(compiler) {
            return;
        }
        check_state!(n.is_if(compiler), "%s", n.to_string(compiler));

        let parent = n.get_parent(compiler).unwrap();
        if !NodeUtil::is_statement_block(compiler, parent) {
            // If the immediate parent is something like a label, we
            // can't move the statement, so bail.
            return;
        }

        let cond = n.get_first_child(compiler).unwrap();
        let true_branch = cond.get_next(compiler);
        let false_branch = true_branch.and_then(|t| t.get_next(compiler));
        check_not_null!(true_branch);
        check_not_null!(false_branch);
        let true_branch = true_branch.unwrap();
        let false_branch = false_branch.unwrap();

        loop {
            let last_true = true_branch.get_last_child(compiler);
            let last_false = false_branch.get_last_child(compiler);
            let (Some(last_true), Some(last_false)) = (last_true, last_false) else {
                break;
            };
            if !self.are_nodes_equal_for_inlining(compiler, last_true, last_false) {
                break;
            }
            last_true.detach(compiler);
            last_false.detach(compiler);
            last_true.insert_after(compiler, n);
            self.report_change_to_enclosing_scope(compiler, parent);
        }
    }

    /// Returns whether the node is a block with a single statement that is an expression.
    // port: PeepholeMinimizeConditions#isFoldableExpressBlock
    #[allow(clippy::if_same_then_else)] // Java's two `return false` branches
    fn is_foldable_express_block(ast: &Ast, n: NodeId) -> bool {
        if n.is_block(ast) && n.has_one_child(ast) {
            let maybe_expr = n.get_first_child(ast).unwrap();
            if maybe_expr.is_expr_result(ast) {
                // IE has a bug where event handlers behave differently when
                // their return value is used vs. when their return value is in
                // an EXPR_RESULT. It's pretty freaking weird. See:
                // http://blickly.github.io/closure-compiler-issues/#291
                // We try to detect this case, and not fold EXPR_RESULTs
                // into other expressions.
                // e.g.:
                // if (e.onchange) {
                //    e.onchange({
                //        _extendedByPrototype: Prototype.emptyFunction,
                //        target: e
                //    });
                //  }
                let expr_child = maybe_expr.get_first_child(ast).unwrap();
                if expr_child.is_call(ast) || expr_child.is_opt_chain_call(ast) {
                    let called_fn = maybe_expr.get_first_first_child(ast).unwrap();

                    // We only have to worry about methods with an implicit 'this'
                    // param, or this doesn't happen.
                    if called_fn.is_get_elem(ast) || called_fn.is_opt_chain_get_elem(ast) {
                        return false;
                    } else if (called_fn.is_get_prop(ast) || called_fn.is_opt_chain_get_prop(ast))
                        && called_fn.get_string_ref(ast).starts_with("on")
                    {
                        return false;
                    }
                }

                return true;
            }
            return false;
        }

        false
    }

    /// Returns the expression node.
    // port: PeepholeMinimizeConditions#getBlockExpression
    fn get_block_expression(ast: &Ast, n: NodeId) -> NodeId {
        check_state!(Self::is_foldable_express_block(ast, n));
        n.get_first_child(ast).unwrap()
    }

    /// Returns whether the node is a block with a single statement that is an return with or
    /// without an expression.
    // port: PeepholeMinimizeConditions#isReturnBlock
    fn is_return_block(ast: &Ast, n: NodeId) -> bool {
        if n.is_block(ast) && n.has_one_child(ast) {
            let first = n.get_first_child(ast).unwrap();
            return first.is_return(ast);
        }

        false
    }

    /// Returns whether the node is a block with a single statement that is an return.
    // port: PeepholeMinimizeConditions#isReturnExpressBlock
    fn is_return_express_block(ast: &Ast, n: NodeId) -> bool {
        if n.is_block(ast) && n.has_one_child(ast) {
            let first = n.get_first_child(ast).unwrap();
            if first.is_return(ast) {
                return first.has_one_child(ast);
            }
        }

        false
    }

    /// Returns whether the node is a single return statement.
    // port: PeepholeMinimizeConditions#isReturnExpression
    fn is_return_expression(ast: &Ast, n: NodeId) -> bool {
        if n.is_return(ast) {
            return n.has_one_child(ast);
        }
        false
    }

    /// Returns the expression that is part of the return.
    // port: PeepholeMinimizeConditions#getBlockReturnExpression
    fn get_block_return_expression(ast: &Ast, n: NodeId) -> NodeId {
        check_state!(Self::is_return_express_block(ast, n));
        n.get_first_first_child(ast).unwrap()
    }

    /// Returns whether the node is a block with a single statement that is a VAR declaration of
    /// a single variable.
    // port: PeepholeMinimizeConditions#isVarBlock
    fn is_var_block(ast: &Ast, n: NodeId) -> bool {
        if n.is_block(ast) && n.has_one_child(ast) {
            let first = n.get_first_child(ast).unwrap();
            if first.is_var(ast) {
                return first.has_one_child(ast);
            }
        }

        false
    }

    /// Returns the var node.
    // port: PeepholeMinimizeConditions#getBlockVar
    fn get_block_var(ast: &Ast, n: NodeId) -> NodeId {
        check_state!(Self::is_var_block(ast, n));
        n.get_first_child(ast).unwrap()
    }

    /// Does a statement consume a 'dangling else'? A statement consumes a 'dangling else' if an
    /// 'else' token following the statement would be considered by the parser to be part of the
    /// statement.
    // port: PeepholeMinimizeConditions#consumesDanglingElse
    fn consumes_dangling_else(ast: &Ast, mut n: NodeId) -> bool {
        loop {
            match n.get_token(ast) {
                Token::IF => {
                    if n.get_child_count(ast) < 3 {
                        return true;
                    }
                    // This IF node has no else clause.
                    n = n.get_last_child(ast).unwrap();
                    continue;
                }
                Token::BLOCK => {
                    if !n.has_one_child(ast) {
                        return false;
                    }
                    // This BLOCK has no curly braces.
                    n = n.get_last_child(ast).unwrap();
                    continue;
                }
                Token::WITH | Token::WHILE | Token::FOR | Token::FOR_IN => {
                    n = n.get_last_child(ast).unwrap();
                    continue;
                }
                _ => {
                    return false;
                }
            }
        }
    }

    /// Whether the node type has lower precedence than "precedence"
    // port: PeepholeMinimizeConditions#isLowerPrecedence
    pub fn is_lower_precedence(ast: &Ast, n: NodeId, precedence: i32) -> bool {
        NodeUtil::precedence(n.get_token(ast)) < precedence
    }

    /// Does the expression contain a property assignment?
    // port: PeepholeMinimizeConditions#isPropertyAssignmentInExpression
    fn is_property_assignment_in_expression(ast: &Ast, n: NodeId) -> bool {
        let is_property_assignment_in_expression_predicate = |ast: &Ast, input: NodeId| {
            input.is_get_prop(ast) && input.get_parent(ast).unwrap().is_assign(ast)
        };
        NodeUtil::has(
            ast,
            n,
            &is_property_assignment_in_expression_predicate,
            &NodeUtil::MATCH_NOT_FUNCTION,
        )
    }

    /// Try to minimize condition expression, as there are additional assumptions that can be
    /// made when it is known that the final result is a boolean.
    ///
    /// Returns the replacement for n, or the original if no change was made.
    // port: PeepholeMinimizeConditions#tryMinimizeCondition
    fn try_minimize_condition(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        let n = self.perform_condition_substitutions(compiler, n);
        let min_cond = MinimizedCondition::from_condition_node(compiler, n);
        let minimized = min_cond.get_minimized(compiler, MinimizationStyle::PREFER_UNNEGATED);
        self.replace_node(compiler, n, &minimized)
    }

    // port: PeepholeMinimizeConditions#replaceNode
    fn replace_node(
        &self,
        compiler: &mut AbstractCompiler,
        original: NodeId,
        measured_node_replacement: &MeasuredNode,
    ) -> NodeId {
        if measured_node_replacement.will_change(original) {
            let replacement = measured_node_replacement.apply_to(compiler, original);
            self.report_change_to_enclosing_scope(compiler, replacement);
            return replacement;
        }
        original
    }

    /// Try to minimize the given condition by applying local substitutions.
    ///
    /// The following types of transformations are performed:
    /// ```text
    ///   x || true        --> true
    ///   x && true        --> x
    ///   x ? false : true --> !x
    ///   x ? true : y     --> x || y
    ///   x ? x : y        --> x || y
    /// ```
    ///
    /// Returns the replacement for n, or the original if no change was made
    // port: PeepholeMinimizeConditions#performConditionSubstitutions
    fn perform_condition_substitutions(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> NodeId {
        let parent = n.get_parent(compiler);

        match n.get_token(compiler) {
            Token::OR | Token::AND => {
                let mut left = n.get_first_child(compiler).unwrap();
                let mut right = n.get_last_child(compiler).unwrap();

                // Because the expression is in a boolean context minimize
                // the children, this can't be done in the general case.
                left = self.perform_condition_substitutions(compiler, left);
                right = self.perform_condition_substitutions(compiler, right);

                // Remove useless conditionals
                // Handle the following cases:
                //   x || false --> x
                //   x && true --> x
                // This works regardless of whether x has side effects.
                //
                // If x does not have side effects:
                //   x || true  --> true
                //   x && false  --> false
                //
                // If x may have side effects:
                //   x || true  --> x,true
                //   x && false  --> x,false
                //
                // In the last two cases, code size may increase slightly (adding
                // some parens because the comma operator has a low precedence) but
                // the new AST is easier for other passes to handle.
                let right_val = self.get_side_effect_free_boolean_value(compiler, right);
                if self.get_side_effect_free_boolean_value(compiler, right) != Tri::UNKNOWN {
                    let r#type = n.get_token(compiler);
                    let replacement: Option<NodeId>;
                    let rval = right_val.to_boolean(true);

                    // (x || FALSE) => x
                    // (x && TRUE) => x
                    if (r#type == Token::OR && !rval) || (r#type == Token::AND && rval) {
                        replacement = Some(left);
                    } else if !self.may_have_side_effects(compiler, left) {
                        replacement = Some(right);
                    } else {
                        // expr_with_sideeffects || true  =>  expr_with_sideeffects, true
                        // expr_with_sideeffects && false  =>  expr_with_sideeffects, false
                        n.detach_children(compiler);
                        replacement = Some(IR::comma(compiler, left, right));
                    }

                    if let Some(replacement) = replacement {
                        n.detach_children(compiler);
                        n.replace_with(compiler, replacement);
                        self.report_change_to_enclosing_scope(compiler, parent.unwrap());
                        return replacement;
                    }
                }
                n
            }
            Token::HOOK => {
                let condition = n.get_first_child(compiler).unwrap();
                let mut true_node = n.get_second_child(compiler).unwrap();
                let mut false_node = n.get_last_child(compiler).unwrap();

                // Because the expression is in a boolean context minimize
                // the result children, this can't be done in the general case.
                // The condition is handled in the general case in #optimizeSubtree
                true_node = self.perform_condition_substitutions(compiler, true_node);
                false_node = self.perform_condition_substitutions(compiler, false_node);

                // Handle five cases:
                //   x ? true : false --> x
                //   x ? false : true --> !x
                //   x ? true : y     --> x || y
                //   x ? y : false    --> x && y

                //   Only when x is NAME, hence x does not have side effects
                //   x ? x : y        --> x || y
                let mut replacement: Option<NodeId> = None;
                let true_node_val = self.get_side_effect_free_boolean_value(compiler, true_node);
                let false_node_val = self.get_side_effect_free_boolean_value(compiler, false_node);
                if true_node_val == Tri::TRUE && false_node_val == Tri::FALSE {
                    // Remove useless conditionals, keep the condition
                    condition.detach(compiler);
                    replacement = Some(condition);
                } else if true_node_val == Tri::FALSE && false_node_val == Tri::TRUE {
                    // Remove useless conditionals, keep the condition
                    condition.detach(compiler);
                    replacement = Some(IR::not(compiler, condition));
                } else if true_node_val == Tri::TRUE {
                    // Remove useless true case.
                    n.detach_children(compiler);
                    replacement = Some(IR::or(compiler, condition, false_node));
                } else if false_node_val == Tri::FALSE {
                    // Remove useless false case
                    n.detach_children(compiler);
                    replacement = Some(IR::and(compiler, condition, true_node));
                } else if !self.may_have_side_effects(compiler, condition)
                    && !self.may_have_side_effects(compiler, true_node)
                    && condition.is_equivalent_to(compiler, true_node)
                {
                    // Remove redundant condition
                    n.detach_children(compiler);
                    replacement = Some(IR::or(compiler, true_node, false_node));
                }

                let mut n = n;
                if let Some(replacement) = replacement {
                    n.replace_with(compiler, replacement);
                    self.report_change_to_enclosing_scope(compiler, replacement);
                    n = replacement;
                }

                n
            }
            _ => {
                // while(true) --> while(1)
                let n_val = self.get_side_effect_free_boolean_value(compiler, n);
                if n_val != Tri::UNKNOWN {
                    let result = n_val.to_boolean(true);
                    let equivalent_result = if result { 1 } else { 0 };
                    return self.maybe_replace_child_with_number(compiler, n, equivalent_result);
                }
                // We can't do anything else currently.
                n
            }
        }
    }

    /// Replaces a node with a number node if the new number node is not equivalent to the current
    /// node.
    ///
    /// Returns the replacement for n if it was replaced, otherwise returns n.
    // port: PeepholeMinimizeConditions#maybeReplaceChildWithNumber
    fn maybe_replace_child_with_number(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        num: i32,
    ) -> NodeId {
        let new_node = IR::number(compiler, f64::from(num));
        if !new_node.is_equivalent_to(compiler, n) {
            n.replace_with(compiler, new_node);
            self.report_change_to_enclosing_scope(compiler, new_node);
            self.mark_functions_deleted(compiler, n);

            return new_node;
        }

        n
    }
}

impl AbstractPeepholeOptimization for PeepholeMinimizeConditions {
    fn fields(&self) -> &AbstractPeepholeOptimizationFields {
        &self.fields
    }

    fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
        &mut self.fields
    }

    fn get_class_name(&self) -> &'static str {
        "com.google.javascript.jscomp.PeepholeMinimizeConditions"
    }

    /// Tries to apply our various peephole minimizations on the passed in node.
    // port: PeepholeMinimizeConditions#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
    ) -> Option<NodeId> {
        match node.get_token(compiler) {
            Token::THROW | Token::RETURN => {
                let result = self.try_remove_redundant_exit(compiler, node);
                if result != Some(node) {
                    return result;
                }
                Some(self.try_replace_exit_with_break(compiler, node))
                // TODO(johnlenz): Maybe remove redundant BREAK and CONTINUE. Overlaps
                // with MinimizeExitPoints.
            }
            Token::NOT => {
                let first = node.get_first_child(compiler).unwrap();
                self.try_minimize_condition(compiler, first);
                Some(self.try_minimize_not(compiler, node))
            }
            Token::IF => {
                let first = node.get_first_child(compiler).unwrap();
                self.perform_condition_substitutions(compiler, first);
                Some(self.try_minimize_if(compiler, node))
            }
            Token::EXPR_RESULT => {
                let first = node.get_first_child(compiler).unwrap();
                self.perform_condition_substitutions(compiler, first);
                Some(self.try_minimize_expr_result(compiler, node))
            }
            Token::HOOK => {
                let first = node.get_first_child(compiler).unwrap();
                self.perform_condition_substitutions(compiler, first);
                Some(self.try_minimize_hook(compiler, node))
            }
            Token::WHILE | Token::DO => {
                let cond = NodeUtil::get_condition_expression(compiler, node).unwrap();
                self.try_minimize_condition(compiler, cond);
                Some(node)
            }
            Token::FOR => {
                self.try_join_for_condition(compiler, node);
                let cond = NodeUtil::get_condition_expression(compiler, node).unwrap();
                self.try_minimize_condition(compiler, cond);
                Some(node)
            }
            Token::BLOCK => Some(self.try_replace_if(compiler, node)),
            _ =>
            // Nothing changed
            {
                Some(node)
            }
        }
    }
}

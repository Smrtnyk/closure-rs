/*
 * Copyright 2026 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 030d049:
//   src/com/google/javascript/jscomp/EvaluationOrder.java.

//! Port of `EvaluationOrder.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::basic_block::BasicBlock;
use crate::scope::ScopeId;
use closure_rhino::check_argument;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;

/// Syntactic rules for the order in which the children of an expression are evaluated, and which
/// children are only conditionally evaluated.
///
/// These predicates only describe expression-level evaluation within a single statement. They
/// deliberately know nothing about statement-level control flow (loops, `try`, `switch`, labels,
/// etc.), which is handled by `BasicBlock::is_block_boundary` and the CFG.
// port: EvaluationOrder
pub struct EvaluationOrder;

impl EvaluationOrder {
    /// Returns whether evaluating `n`'s parent expression may skip evaluating `n`, even when all
    /// of the parent's earlier-evaluated children are evaluated.
    ///
    /// This is true for:
    ///
    /// - the right-hand side of `&&`, `||`, and `??`
    /// - the right-hand side of `&&=`, `||=`, and `??=`
    /// - both arms of `?:`
    /// - every child of an optional chain node other than its receiver
    /// - the default value of a `DEFAULT_VALUE`
    ///
    /// This only looks at `n`'s immediate parent. An unconditional child of a conditional child
    /// is also conditionally executed.
    // port: EvaluationOrder#isConditionalChild
    pub fn is_conditional_child(ast: &Ast, n: NodeId) -> bool {
        let Some(parent) = n.get_parent(ast) else {
            return false;
        };
        match parent.get_token(ast) {
            Token::AND
            | Token::OR
            | Token::COALESCE
            | Token::ASSIGN_AND
            | Token::ASSIGN_OR
            | Token::ASSIGN_COALESCE
            | Token::HOOK
            | Token::OPTCHAIN_GETPROP
            | Token::OPTCHAIN_GETELEM
            | Token::OPTCHAIN_CALL
            | Token::DEFAULT_VALUE => Some(n) != parent.get_first_child(ast),
            _ => false,
        }
    }

    /// Returns whether `parent` evaluates its last child before its other children, rather than
    /// in source order.
    ///
    /// This is true for:
    ///
    /// - `DESTRUCTURING_LHS` with an initializer: `const [a] = rhs`
    /// - `ASSIGN` with a destructuring pattern target: `[a] = rhs`
    /// - `DEFAULT_VALUE` whose target is a name or a destructuring pattern: `[a = dflt]` or
    ///   `[[a] = dflt]`
    ///
    /// A `DEFAULT_VALUE` with a property target, such as `[o[k] = dflt]`, evaluates the target
    /// reference (`o` and `k`) before the default value, so it uses source order. The property
    /// write itself still happens after the default value is evaluated, just like the write in
    /// `o[k] = rhs`.
    ///
    /// A `DEFAULT_VALUE` with a name target is treated as evaluating the default first because
    /// the name is only written, after the default value is evaluated.
    // port: EvaluationOrder#evaluatesLastChildFirst
    pub fn evaluates_last_child_first(ast: &Ast, parent: NodeId) -> bool {
        match parent.get_token(ast) {
            Token::DESTRUCTURING_LHS => parent.has_two_children(ast),
            Token::ASSIGN => parent
                .get_first_child(ast)
                .unwrap()
                .is_destructuring_pattern(ast),
            Token::DEFAULT_VALUE => {
                let target = parent.get_first_child(ast).unwrap();
                // A name target is only written, and the write happens after the default value
                // is evaluated. Visiting the default first is also what lets
                // VariableReferenceCheck report the TDZ error in `function f(x = x) {}`.
                target.is_name(ast) || target.is_destructuring_pattern(ast)
            }
            _ => false,
        }
    }

    /// Returns whether, within a single evaluation of their common parent, `first` is evaluated
    /// before `second`.
    ///
    /// This only describes evaluation order. It does not account for `second` being evaluated
    /// even if `first` is not (e.g. the two arms of a `HOOK`, or a `CASE` body reached by
    /// fallthrough).
    ///
    /// Panics (Java: `IllegalArgumentException`) if the nodes are not siblings.
    // port: EvaluationOrder#isEvaluatedBeforeSibling
    pub fn is_evaluated_before_sibling(ast: &Ast, first: NodeId, second: NodeId) -> bool {
        let parent = first.get_parent(ast);
        check_argument!(
            parent.is_some() && parent == second.get_parent(ast),
            "not siblings: %s, %s",
            first.to_string(ast),
            second.to_string(ast)
        );
        let parent = parent.unwrap();
        if first == second {
            return false;
        }
        if Self::evaluates_last_child_first(ast, parent) {
            // All the nodes that return true from evaluatesLastChildFirst have exactly two
            // children.
            return Some(first) == parent.get_last_child(ast);
        }
        let mut sibling = first.get_next(ast);
        while let Some(s) = sibling {
            if s == second {
                return true;
            }
            sibling = s.get_next(ast);
        }
        false
    }

    /// Returns an `ExecutionOrderCheck` that tests whether nodes execute after `init_node` within
    /// its basic block (bounded by the closest hoist scope of `init_scope`).
    // port: EvaluationOrder#checkExecutionAfter
    pub fn check_execution_after(
        compiler: &AbstractCompiler,
        init_node: NodeId,
        init_scope: ScopeId,
    ) -> ExecutionOrderCheck {
        let scope_root = init_scope
            .get_closest_hoist_scope(compiler)
            .unwrap()
            .get_root_node(compiler);
        // Java: an ImmutableMap.Builder, only looked up by key.
        let mut ancestor_to_child = IndexMap::<NodeId, NodeId>::default();
        let mut prev = init_node;
        let mut curr = init_node.get_parent(compiler);
        while let Some(c) = curr
            && !c.is_root(compiler)
            && prev != scope_root
            && !BasicBlock::is_block_boundary(compiler, prev, Some(c))
        {
            // buildOrThrow: the ancestors are distinct.
            ancestor_to_child.insert(c, prev);
            prev = c;
            curr = c.get_parent(compiler);
        }
        ExecutionOrderCheck {
            scope_root,
            init_ancestor_to_child: ancestor_to_child,
        }
    }
}

/// Precomputed basic-block ancestor state for an initialization node, used to check whether
/// subsequent reference nodes in the same hoist scope are guaranteed to execute after it.
// port: EvaluationOrder.ExecutionOrderCheck
pub struct ExecutionOrderCheck {
    scope_root: NodeId,
    init_ancestor_to_child: IndexMap<NodeId, NodeId>,
}

impl ExecutionOrderCheck {
    /// Returns whether every execution of `ref_node` is preceded by an execution of the
    /// initialization node, within the same hoist scope and basic block.
    ///
    /// This does not require `ref_node` to be executed at all, e.g. it returns true for `ref` in
    /// `init ? ref : 0`.
    // port: EvaluationOrder.ExecutionOrderCheck#isExecutedAfterInit
    pub fn is_executed_after_init(
        &self,
        compiler: &AbstractCompiler,
        ref_node: NodeId,
        ref_scope: ScopeId,
    ) -> bool {
        if ref_scope
            .get_closest_hoist_scope(compiler)
            .unwrap()
            .get_root_node(compiler)
            != self.scope_root
        {
            return false;
        }
        let mut prev = ref_node;
        let mut curr = ref_node.get_parent(compiler);
        while let Some(c) = curr
            && prev != self.scope_root
        {
            if c.is_class_members(compiler) {
                // Computed keys in a class body execute before static initializers regardless of
                // source order, and non-static field initializers only execute upon
                // instantiation.
                return false;
            }
            let init_child = self.init_ancestor_to_child.get(&c).copied();
            if let Some(init_child) = init_child {
                if c.is_case(compiler) {
                    // In `case init: use(ref);`, an earlier case can fall through into the case
                    // body without evaluating the `case` test expression.
                    return false;
                }
                // initChild is not a conditional child of curr, because checkExecutionAfter
                // stops at block boundaries. So evaluation order is sufficient.
                return EvaluationOrder::is_evaluated_before_sibling(compiler, init_child, prev);
            }
            prev = c;
            curr = c.get_parent(compiler);
        }
        false
    }
}

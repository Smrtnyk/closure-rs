/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ExpressionDecomposer.java.

//! Port of `ExpressionDecomposer.java`.
//!
//! Partially or fully decomposes an expression with respect to some sub-expression. Initially
//! this is intended to expand the locations where inlining can occur, but has other uses as well.
//!
//! For example: `var x = y() + z();` becomes `var a = y(); var b = z(); var x = a + b;`.
//!
//! Decomposing, in this context does not mean full decomposition to "atomic" expressions. While
//! it is possible to iteratively apply decomposition to get statements with at most one
//! side-effect, that isn't the intended purpose of this class. The focus is on decomposing "just
//! enough" to "free" a <em>particular</em> subexpression. For example:
//!
//! - Given: `return (alert() + alert()) + z();`
//! - Exposing: `z()`
//! - Sufficient decomposition: `var temp = alert() + alert(); return temp + z();`

use crate::abstract_compiler::AbstractCompiler;
use crate::ast_analyzer::AstAnalyzer;
use crate::ast_factory::AstFactory;
use crate::make_declared_names_unique::ContextualRenamer;
use crate::node_util::NodeUtil;
use crate::optional_chain_rewriter::{OptionalChainRewriter, TmpVarNameCreator};
use crate::scope::Scope;
use closure_jstype::{js_type::JSType, js_type_native::JSTypeNative, object_type::ObjectType};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    jscomp_colors::standard_colors,
    jstype::TypeId,
    node::{NodeId, Prop},
    qualified_name::QualifiedName,
    token::Token,
};
use indexmap::IndexSet;
use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, LazyLock};

/// The type of decomposition that can be performed on an expression.
///
/// @see #canExposeExpression
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DecompositionType {
    UNDECOMPOSABLE,
    MOVABLE,
    DECOMPOSABLE,
}

/// Identify special case logic that needs to be enabled to work around edge cases, such as bad
/// JSVM behavior.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Workaround {
    // IE11 throws an exception for `window.location.assign.call(window.location, url)`
    BROKEN_IE11_LOCATION_ASSIGN,
}

pub struct ExpressionDecomposer {
    ast_analyzer: AstAnalyzer,
    ast_factory: Arc<AstFactory>,
    safe_name_id_supplier: Arc<dyn Fn() -> String + Send + Sync>,
    known_constant_functions: IndexSet<JsString>,
    scope: Scope,
    enabled_workarounds: IndexSet<Workaround>,
    unknown_type: Option<TypeId>,
    temp_name_prefix: String,
    result_name_prefix: String,
}

// An arbitrary limit to prevent catch infinite recursion.
// Raised from 100->1000 on Jan 22, 2021
const MAX_ITERATIONS: i32 = 1000;

/// We must not decompose this method qname.
///
/// See usage location for more information.
static WINDOW_LOCATION_ASSIGN: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("window.location.assign"));

/// A simple class to track two things: - whether side effects have been seen. - the last
/// statement inserted
struct DecompositionState {
    side_effects: bool,
    extract_before_statement: NodeId,
}

impl fmt::Display for DecompositionState {
    // port: ExpressionDecomposer.DecompositionState#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // MoreObjects.toStringHelper; the node is shown by its arena handle, since printing a
        // node needs the arena.
        write!(
            f,
            "DecompositionState{{sideEffects={}, extractBeforeStatement={:?}}}",
            self.side_effects, self.extract_before_statement
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EvaluationDirection {
    FORWARD,
    REVERSE,
}

impl ExpressionDecomposer {
    /// Create an ExpressionDecomposer.
    ///
    /// DO NOT USE THIS METHOD DIRECTLY. Call `compiler.createExpressionDecomposer()` instead so
    /// the compiler can supply the workarounds needed for this compilation.
    ///
    /// @param constFunctionNames set of names known to be constant functions. Used by
    ///     InlineFunctions to prevent this pass from breaking bookkeeping for functions it's
    ///     inlining.
    /// @param enabledWorkarounds indicates which workarounds for bad JSVM behavior should be
    ///     enabled.
    // port: ExpressionDecomposer#ExpressionDecomposer
    pub fn new(
        compiler: &mut AbstractCompiler,
        safe_name_id_supplier: Arc<dyn Fn() -> String + Send + Sync>,
        const_function_names: IndexSet<JsString>,
        scope: Scope,
        enabled_workarounds: IndexSet<Workaround>,
    ) -> Self {
        let ast_analyzer = compiler.get_ast_analyzer();
        let ast_factory = Arc::new(compiler.create_ast_factory());
        let unknown_type =
            if compiler.has_type_checking_run() && !compiler.has_optimization_colors() {
                Some(
                    compiler
                        .get_type_registry()
                        .get_native_type(JSTypeNative::UNKNOWN_TYPE),
                )
            } else {
                None
            };
        Self {
            ast_analyzer,
            ast_factory,
            safe_name_id_supplier,
            known_constant_functions: const_function_names,
            scope,
            enabled_workarounds,
            unknown_type,
            temp_name_prefix: "JSCompiler_temp".to_string(),
            result_name_prefix: "JSCompiler_inline_result".to_string(),
        }
    }

    /// Perform any rewriting necessary so that the specified expression is `MOVABLE`.
    ///
    /// This method is a primary entrypoint into this class. It performs expression decomposition
    /// such that `expression` can be moved to a preceding statement without changing behaviour.
    ///
    /// Exposing `expression` generally doesn't mean that `expression` itself will moved. An
    /// expression is exposed within a larger statement if no preceding expression would interact
    /// with it.
    ///
    /// @see #canExposeExpression
    // port: ExpressionDecomposer#maybeExposeExpression
    pub fn maybe_expose_expression(&mut self, compiler: &mut AbstractCompiler, expression: NodeId) {
        // If the expression needs to exposed.
        let mut i: i32 = 0;
        while DecompositionType::DECOMPOSABLE == self.can_expose_expression(compiler, expression) {
            if !self.expose_expression(compiler, expression) {
                // If `canExposeExpression` returned `DECOMPOSABLE` but `exposeExpression` returned
                // false` nothing was exposed so there's no point in trying again. Indicates a bug
                // in either `canExposeExpression` or `exposeExpression`.
                panic!(
                    "exposeExpression exposed nothing for:\n{}",
                    crate::node_printing::to_string_tree(compiler, expression)
                );
            }
            i += 1;
            if i > MAX_ITERATIONS {
                panic!(
                    "exposeExpression depth exceeded on:\n{}",
                    expression.to_string_tree(compiler)
                );
            }
        }
    }

    /// Perform partial decomposition to get the given expression closer to being `MOVEABLE`.
    ///
    /// @return Whether any modifications furthering exposure were made to the expression.
    // port: ExpressionDecomposer#exposeExpression(Node)
    fn expose_expression(&mut self, compiler: &mut AbstractCompiler, expression: NodeId) -> bool {
        // First rewrite all optional chains containing the expression.
        // This must be done first, because the expression root may be an optional chain, and
        // rewriting it creates a new node to be the expression root.
        self.rewrite_all_containing_optional_chains(compiler, expression);
        let expression_root = Self::find_expression_root(compiler, expression);
        let expression_root = check_not_null!(expression_root);
        check_state!(
            NodeUtil::is_statement(compiler, expression_root),
            "%s",
            expression_root.to_string(compiler)
        );
        self.expose_expression_in_root(compiler, expression_root, expression)
    }

    /// Rewrite `expressionRoot` such that `subExpression` is a `MOVABLE` while maintaining
    /// evaluation order.
    ///
    /// IMPORTANT: This method assumes there are no optional chain parents of subExpression. The
    /// single-argument version of this method takes care of that and should be the only caller of
    /// this method.
    ///
    /// Two types of subexpressions are extracted from the source expression:
    ///
    /// 1. subexpressions with side-effects
    /// 2. conditional expressions that contain `subExpression`, which are transformed into IF
    ///    statements.
    ///
    /// The following terms are used:
    ///
    /// - expressionRoot: The top-level node, before which any extracted expressions should be
    ///   placed.
    /// - nodeWithNonconditionalParent: The node that will be extracted.
    ///
    /// @param expressionRoot The root of the subtree within which to expose `subExpression`.
    /// @param subExpression A descendant of `expressionRoot` to be exposed.
    /// @return Whether any modifications furthering exposure were made to the expression.
    // port: ExpressionDecomposer#exposeExpression(Node, Node)
    fn expose_expression_in_root(
        &mut self,
        compiler: &mut AbstractCompiler,
        expression_root: NodeId,
        sub_expression: NodeId,
    ) -> bool {
        let mut exposed_something = false;

        let node_with_nonconditional_parent =
            Self::find_nonconditional_parent(compiler, sub_expression, expression_root);
        // Before extraction, record whether there are side-effect
        let has_following_side_effects = self
            .ast_analyzer
            .may_have_side_effects(compiler, node_with_nonconditional_parent);

        let expr_injection_point =
            Self::find_injection_point(compiler, node_with_nonconditional_parent)
                .expect("NullPointerException");
        let mut state = DecompositionState {
            side_effects: has_following_side_effects,
            extract_before_statement: expr_injection_point,
        };

        // Extract expressions in the reverse order of their evaluation. This is roughly, traverse
        // up the AST extracting any preceding expressions that may have side-effects or be
        // side-effected.
        let mut last_exposed_subexpression: Option<NodeId> = None;
        let mut expression_to_expose = node_with_nonconditional_parent;
        let mut expression_parent = expression_to_expose
            .get_parent(compiler)
            .expect("NullPointerException");
        while expression_parent != expression_root {
            check_state!(
                !Self::is_conditional_op(compiler, expression_parent)
                    || expression_to_expose.is_first_child_of(compiler, Some(expression_parent)),
                "%s",
                expression_parent.to_string(compiler)
            );

            if expression_parent.is_assign(compiler) {
                if self.is_safe_assign(compiler, expression_parent, state.side_effects) {
                    // It is always safe to inline "foo()" for expressions such as
                    // "a = b = c = foo();"
                    // As the assignment is unaffected by side effect of "foo()"
                    // and the names assigned-to cannot influence the state before
                    // the call to foo.
                    //
                    // This is not true of more complex LHS values, such as
                    // a.x = foo();
                    // next().x = foo();
                    // in these cases the checks below are necessary.
                } else if !expression_to_expose.is_first_child_of(compiler, Some(expression_parent))
                {
                    // Alias "next()" in "next().foo"

                    let left = expression_parent
                        .get_first_child(compiler)
                        .expect("NullPointerException");
                    match left.get_token(compiler) {
                        // e.g. backoff from exposing expression `getObj()` in
                        // `[getObj().propName] = ...` as the RHS must execute first
                        // e.g. backoff from exposing expression`getObj()` in `{a:
                        // getObj().propName} = ...` as the RHS must execute first
                        Token::ARRAY_PATTERN | Token::OBJECT_PATTERN => {}
                        Token::GETELEM | Token::GETPROP => {
                            let left_first = left.get_first_child(compiler);
                            exposed_something = self
                                .decompose_sub_expressions(compiler, left_first, None, &mut state)
                                || exposed_something;
                        }
                        _ => panic!(
                            "Expected a property access or destructuring pattern: {}",
                            left.to_string_tree(compiler)
                        ),
                    }
                }
            } else if expression_parent.is_call(compiler)
                && NodeUtil::is_normal_get(
                    compiler,
                    expression_parent
                        .get_first_child(compiler)
                        .expect("NullPointerException"),
                )
            {
                let callee = expression_parent.get_first_child(compiler).unwrap();
                if callee != expression_to_expose {
                    let callee_next = callee.get_next(compiler);
                    exposed_something = self.decompose_sub_expressions(
                        compiler,
                        callee_next,
                        Some(expression_to_expose),
                        &mut state,
                    ) || exposed_something;
                }

                // Now handle the call expression. We only have to do this if we arrived at
                // decomposing this call through one of the arguments, rather than the callee;
                // otherwise the callee would already be safe.
                if self.is_expression_tree_unsafe(compiler, callee, state.side_effects)
                    && last_exposed_subexpression != callee.get_first_child(compiler)
                {
                    // Either there were preexisting side-effects, or this node has side-effects.
                    state.side_effects = true;
                    // Rewrite the call so "this" is preserved and continue walking up from there.
                    self.rewrite_call_expression(compiler, expression_parent, &mut state);
                    exposed_something = true;
                }
            } else {
                let parent_first = expression_parent.get_first_child(compiler);
                exposed_something = self.decompose_sub_expressions(
                    compiler,
                    parent_first,
                    Some(expression_to_expose),
                    &mut state,
                ) || exposed_something;
            }

            last_exposed_subexpression = Some(expression_to_expose);
            expression_to_expose = expression_parent;
            expression_parent = expression_to_expose
                .get_parent(compiler)
                .expect("NullPointerException");
        }

        // Now extract the expression that the decomposition is being performed to
        // to allow to be moved.  All expressions that need to be evaluated before
        // this have been extracted, so add the expression statement after the
        // other extracted expressions and the original statement (or replace
        // the original statement.
        if node_with_nonconditional_parent == sub_expression {
            // Don't extract the call, as that introduces an extra constant VAR
            // that will simply need to be inlined back.  It will be handled as
            // an EXPRESSION call site type.
            // Node extractedCall = extractExpression(decomposition, expressionRoot);
        } else {
            let parent = node_with_nonconditional_parent
                .get_parent(compiler)
                .expect("NullPointerException");
            let need_result = !parent.is_expr_result(compiler);
            self.extract_conditional(
                compiler,
                node_with_nonconditional_parent,
                expr_injection_point,
                need_result,
            );
            exposed_something = true;
        }

        exposed_something
    }

    /// Rewrite all of the optional chains containing the given subExpression.
    // port: ExpressionDecomposer#rewriteAllContainingOptionalChains
    fn rewrite_all_containing_optional_chains(
        &mut self,
        compiler: &mut AbstractCompiler,
        sub_expression: NodeId,
    ) {
        let temp_name_prefix = self.temp_name_prefix.clone();
        let safe_name_id_supplier = self.safe_name_id_supplier.clone();
        let tmp_var_name_creator: Arc<dyn TmpVarNameCreator + Send + Sync> =
            Arc::new(move |_compiler: &mut AbstractCompiler| {
                Self::get_temp_constant_value_name_with(&temp_name_prefix, &safe_name_id_supplier)
            });
        let mut opt_chain_rewriter_builder = OptionalChainRewriter::builder(compiler);
        opt_chain_rewriter_builder
            .set_tmp_var_name_creator(tmp_var_name_creator)
            .set_scope(self.scope);
        // Rewriting the chains changes the shape of the AST in a way that would interfere
        // with the simple traversal from child to parent done here, so we'll traverse
        // them all first, then rewrite them.
        let mut rewriters: VecDeque<OptionalChainRewriter> = VecDeque::new();

        let mut expr_parent = sub_expression
            .get_parent(compiler)
            .expect("NullPointerException");
        while !NodeUtil::is_statement(compiler, expr_parent) {
            if NodeUtil::is_end_of_full_opt_chain(compiler, expr_parent) {
                // We want to rewrite the outermost chain first, so the last one
                // we find is the first one we rewrite.
                rewriters.push_front(opt_chain_rewriter_builder.build(compiler, expr_parent));
            } else if expr_parent.is_call(compiler) {
                // It is possible to make a non-optional call against an optional chain callee by
                // applying parentheses like this.
                // `(obj?.method)(arg)`
                // I don't think there's a good reason to do that, since it will cause a runtime
                // exception if the chain is ever undefined, but it is allowed, so we must handle
                // it.
                // Fortunately the OptionalChainRewriter knows how to fix the call so it will
                // still get made with the right `this` value.
                let callee = expr_parent
                    .get_first_child(compiler)
                    .expect("NullPointerException");
                if NodeUtil::is_opt_chain_get(compiler, callee) {
                    // By definition callee must end an optional chain, because it is the first
                    // child of a non-optional parent.
                    // checkState(NodeUtil.isEndOfFullOptChain(callee))
                    rewriters.push_front(opt_chain_rewriter_builder.build(compiler, callee));
                }
            }
            expr_parent = expr_parent
                .get_parent(compiler)
                .expect("NullPointerException");
        }
        for rewriter in rewriters.iter_mut() {
            rewriter.rewrite(compiler);
        }
    }

    /// Extract the specified expression from its parent expression.
    ///
    /// @see #canExposeExpression
    // port: ExpressionDecomposer#moveExpression
    pub fn move_expression(&mut self, compiler: &mut AbstractCompiler, expression: NodeId) {
        // TODO(johnlenz): This is not currently used by the function inliner,
        // as moving the call out of the expression before the actual function call
        // causes additional variables to be introduced.  As the variable
        // inliner is improved, this might be a viable option.

        let result_name = self.get_result_value_name();
        let injection_point = Self::find_injection_point(compiler, expression);
        let injection_point = check_not_null!(injection_point);
        let injection_point_parent = injection_point.get_parent(compiler);
        let injection_point_parent = check_not_null!(injection_point_parent);
        check_state!(NodeUtil::is_statement_block(
            compiler,
            injection_point_parent
        ));

        // Replace the expression with a reference to the new name.
        let name = IR::name(compiler, result_name.as_str()).copy_type_from(compiler, expression);
        expression.replace_with(compiler, name);

        // Re-add the expression at the appropriate place.
        let new_expression_root =
            NodeUtil::new_var_node(compiler, result_name.as_str(), Some(expression));
        new_expression_root
            .get_first_child(compiler)
            .expect("NullPointerException")
            .copy_type_from(compiler, expression);
        new_expression_root.insert_before(compiler, injection_point);

        compiler.report_change_to_enclosing_scope(injection_point_parent);
    }

    /// Returns the enclosing expression to decompose
    ///
    /// The intention is to indicate the top-most node that could be rewritten as an if-statement
    /// in order to better expose subExpression for inlining.
    ///
    /// Examples:
    ///
    /// ```text
    /// a = (x() && y()) && subExpression; // result is (x() && y()) && subExpression
    /// a = x() && (y() && subExpression); // result is x() && (y() && subExpression)
    /// a = (x() && subExpression) && y(); // result is x() && subExpression
    /// a = x() && (subExpression && y()); // result is x() && (subExpression && y())
    /// a = (subExpression && x()) && y(); // result is subExpression
    /// a = subExpression && (x() && y()); // result is subExpression
    /// ```
    ///
    /// When subExpression is contained within an optional chain, we want to treat everything
    /// after a `?.` up until the next `?.` as a single conditional operation.
    ///
    /// Examples:
    ///
    /// ```text
    /// a = subExpression.x?.y.z();          // result is subExpression
    /// a = x()?.[subExpression].y;          // result is x()?.[subExpression].y
    /// a = x()?.y.z?.p(subExpression).q?.r; // result is x()?.y.z?.p(subExpression).q
    /// ```
    ///
    /// @param subExpression the expression to consider entire chains
    /// @param expressionRoot a node containing subExpression. The returned node will be a
    ///     descendent of this one.
    // port: ExpressionDecomposer#findNonconditionalParent
    fn find_nonconditional_parent(
        compiler: &AbstractCompiler,
        sub_expression: NodeId,
        expression_root: NodeId,
    ) -> NodeId {
        let mut result = sub_expression;

        let mut child = sub_expression;
        let mut parent = child.get_parent(compiler).expect("NullPointerException");
        while parent != expression_root {
            if Self::is_conditional_op(compiler, parent)
                && !child.is_first_child_of(compiler, Some(parent))
            {
                // subExpression is not part of the first child (which is always executed), so
                // parent decides whether subExpression will be executed or not
                result = parent;
            }
            child = parent;
            parent = child.get_parent(compiler).expect("NullPointerException");
        }
        if NodeUtil::is_opt_chain_node(compiler, result) {
            // the loop above may have left result pointing into the middle of an optional chain
            // for a case like this.
            // `x?.y.z(subExpression).p.q?.r.s`
            // result is currently `x?.y.z(subExpression)`, but we want it to be the full
            // sub-chain containing subExpression
            // `x?.y.z(subExpression).p.q`
            result = NodeUtil::get_end_of_opt_chain_segment(compiler, result);
        }

        result
    }

    /// @param n The node with which to start iterating.
    /// @param stopNode A node after which to stop iterating.
    /// @return Whether any modifications furthering decomposition were made to n.
    // port: ExpressionDecomposer#decomposeSubExpressions
    fn decompose_sub_expressions(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: Option<NodeId>,
        stop_node: Option<NodeId>,
        state: &mut DecompositionState,
    ) -> bool {
        let Some(mut n) = n else {
            return false;
        };
        if Some(n) == stop_node {
            return false;
        }

        let mut decomposed_something = false;

        // Decompose the children in reverse evaluation order. This simplifies determining if any
        // of the children following have side-effects. If they do we need to be more aggressive
        // about removing values from the expression. Reverse order also maintains evaluation
        // order as each extracted statemented is inserted on top of the others.
        let next = n.get_next(compiler);
        decomposed_something = self.decompose_sub_expressions(compiler, next, stop_node, state)
            || decomposed_something;

        // Now this node.

        if NodeUtil::may_be_object_lit_key(compiler, n)
            // TODO(b/111621528): Delete when fixed.
            || n.is_computed_prop(compiler)
        {
            if n.is_computed_prop(compiler) {
                // If the prop is computed we have to fork the decomposition between the key and
                // value. This is because we can't move the property assignment itself;
                // COMPUTED_PROP must remain a child of OBJECTLIT for example.
                //
                // We decompose the value of the prop first because decomposition is in reverse
                // order of evaluation.
                let second = n.get_second_child(compiler);
                decomposed_something = self
                    .decompose_sub_expressions(compiler, second, stop_node, state)
                    || decomposed_something;
            }

            // Decompose the children of the prop rather than the prop itself. In the computed
            // case this will be the key, otherwise it will be the value.
            n = n.get_first_child(compiler).expect("NullPointerException");
        } else if n.is_template_lit_sub(compiler) {
            // A template literal substitution expression like ${f()} is represented in the AST as
            //   TEMPLATELIT_SUB
            //     CALL
            //       NAME f
            // The TEMPLATELIT_SUB node is not actually an expression and can't be extracted, but
            // we may need to extract the expression inside of it.
            n = n.get_first_child(compiler).expect("NullPointerException");
        } else if n.is_spread(compiler) {
            // SPREADs aren't expression but they can still be extracted using temp variables.
            //
            // Because object-spread can trigger getters we assume all spreads have side-effects.
            // TODO(nickreid): Use `assumeGettersArePure` here. It would have been a pain to pipe
            // it down here and write all the tests. Since there are very few cases, and it
            // doesn't affect code removal, we didn't bother initially. Everything always works
            // one way.
        } else if !IR::may_be_expression(compiler, n) {
            // If n is not an expression then it can't be extracted. For example if n is the
            // destructuring pattern on the left side of a VAR statement:
            //   var {pattern} = rhs();
            // See test case: testExposeExpression18
            return decomposed_something;
        }

        // TODO(johnlenz): Move "safety" code to a shared class.
        if self.is_expression_tree_unsafe(compiler, n, state.side_effects) {
            // Either there were preexisting side-effects, or this node has side-effects.
            state.side_effects = true;
            state.extract_before_statement =
                self.extract_expression(compiler, n, state.extract_before_statement);
            decomposed_something = true;
        }
        decomposed_something
    }

    /// @param expr The conditional expression to extract.
    /// @param injectionPoint The node before which the extracted expression would be injected.
    /// @param needResult Whether the result of the expression is required.
    /// @return The node that contains the logic of the expression after extraction.
    // port: ExpressionDecomposer#extractConditional
    fn extract_conditional(
        &mut self,
        compiler: &mut AbstractCompiler,
        expr: NodeId,
        injection_point: NodeId,
        need_result: bool,
    ) -> NodeId {
        let parent = expr.get_parent(compiler).expect("NullPointerException");
        let temp_name = self.get_temp_value_name();

        // Break down the conditional.
        let first = expr
            .get_first_child(compiler)
            .expect("NullPointerException");
        let second = first.get_next(compiler);
        let last = expr.get_last_child(compiler).expect("NullPointerException");

        // Isolate the children nodes.
        expr.detach_children(compiler);

        // Transform the conditional to an IF statement.
        let cond: NodeId;
        let true_expr = self
            .ast_factory
            .create_block(compiler, &[])
            .srcref(compiler, expr);
        let false_expr = self
            .ast_factory
            .create_block(compiler, &[])
            .srcref(compiler, expr);
        match expr.get_token(compiler) {
            Token::HOOK => {
                // a = x?y:z --> if (x) {a=y} else {a=z}
                cond = first;
                let second = second.expect("NullPointerException");
                let result =
                    Self::build_result_expression(compiler, second, need_result, &temp_name);
                let stmt = self.ast_factory.expr_result(compiler, result);
                true_expr.add_child_to_front(compiler, stmt);
                let result = Self::build_result_expression(compiler, last, need_result, &temp_name);
                let stmt = self.ast_factory.expr_result(compiler, result);
                false_expr.add_child_to_front(compiler, stmt);
            }
            Token::AND => {
                // a = x&&y --> if (a=x) {a=y} else {}
                cond = Self::build_result_expression(compiler, first, need_result, &temp_name);
                let result = Self::build_result_expression(compiler, last, need_result, &temp_name);
                let stmt = self.ast_factory.expr_result(compiler, result);
                true_expr.add_child_to_front(compiler, stmt);
            }
            Token::OR => {
                // a = x||y --> if (a=x) {} else {a=y}
                cond = Self::build_result_expression(compiler, first, need_result, &temp_name);
                let result = Self::build_result_expression(compiler, last, need_result, &temp_name);
                let stmt = self.ast_factory.expr_result(compiler, result);
                false_expr.add_child_to_front(compiler, stmt);
            }
            Token::COALESCE => {
                // a = x ?? y --> if ((temp=x)!=null) {a=temp} else {a=y}
                let temp_name_assign = self.get_temp_value_name();
                let temp_var_node_assign = self
                    .ast_factory
                    .create_single_var_name_declaration(compiler, temp_name_assign.as_str())
                    .srcref_tree_if_missing(compiler, expr);
                temp_var_node_assign.insert_before(compiler, injection_point);

                let assign_lhs =
                    Self::build_result_expression(compiler, first, true, &temp_name_assign);
                let null_node = self
                    .ast_factory
                    .create_null(compiler)
                    .srcref(compiler, expr);
                cond = self
                    .ast_factory
                    .create_ne(compiler, assign_lhs, null_node)
                    .srcref(compiler, expr);
                let first_type = AstFactory::type_node(first);
                let temp_name_node = self
                    .ast_factory
                    .create_name(compiler, temp_name_assign.as_str(), first_type)
                    .srcref(compiler, expr);
                let result = Self::build_result_expression(
                    compiler,
                    temp_name_node,
                    need_result,
                    &temp_name,
                );
                let stmt = self.ast_factory.expr_result(compiler, result);
                true_expr.add_child_to_front(compiler, stmt);
                let result = Self::build_result_expression(compiler, last, need_result, &temp_name);
                let stmt = self.ast_factory.expr_result(compiler, result);
                false_expr.add_child_to_front(compiler, stmt);
            }
            // With a valid tree we should never get here.
            _ => panic!("Unexpected expression: {}", expr.to_string(compiler)),
        }

        let if_node = if false_expr.has_children(compiler) {
            self.ast_factory
                .create_if_with_else(compiler, cond, true_expr, false_expr)
        } else {
            self.ast_factory.create_if(compiler, cond, true_expr)
        };
        if_node.srcref_if_missing(compiler, expr);

        if need_result {
            let temp_var_node = self
                .ast_factory
                .create_single_var_name_declaration(compiler, temp_name.as_str())
                .srcref_tree_if_missing(compiler, expr);
            temp_var_node.insert_before(compiler, injection_point);
            if_node.insert_after(compiler, temp_var_node);

            // Replace the expression with the temporary name.
            let replacement_value_node =
                IR::name(compiler, temp_name.as_str()).copy_type_from(compiler, expr);
            expr.replace_with(compiler, replacement_value_node);
        } else {
            // Only conditionals that are the direct child of an expression statement
            // don't need results, for those simply replace the expression statement.
            check_argument!(parent.is_expr_result(compiler));
            parent.replace_with(compiler, if_node);
        }

        if_node
    }

    /// Create an expression tree for an expression.
    ///
    /// If the result of the expression is needed, then:
    ///
    /// ```text
    /// ASSIGN
    ///   tempName
    ///   expr
    /// ```
    ///
    /// otherwise, simply: `expr`
    // port: ExpressionDecomposer#buildResultExpression
    fn build_result_expression(
        compiler: &mut AbstractCompiler,
        expr: NodeId,
        need_result: bool,
        temp_name: &str,
    ) -> NodeId {
        if need_result {
            let name = IR::name(compiler, temp_name).copy_type_from(compiler, expr);
            IR::assign(compiler, name, expr)
                .copy_type_from(compiler, expr)
                .srcref_tree(compiler, expr)
        } else {
            expr
        }
    }

    // port: ExpressionDecomposer#isConstantNameNode
    fn is_constant_name_node(&self, compiler: &mut AbstractCompiler, n: NodeId) -> bool {
        // Non-constant names values may have been changed.
        n.is_name(compiler)
            && (NodeUtil::is_constant_var(compiler, n, Some(self.scope))
                || self
                    .known_constant_functions
                    .contains(&n.get_string(compiler)))
    }

    /// @param expr The expression to extract.
    /// @param injectionPoint The node before which to added the extracted expression.
    /// @return The extracted statement node.
    // port: ExpressionDecomposer#extractExpression
    fn extract_expression(
        &mut self,
        compiler: &mut AbstractCompiler,
        expr: NodeId,
        injection_point: NodeId,
    ) -> NodeId {
        // Since all instances of logical assignment ops are normalized into an expression that
        // separates the logical operation from the assignment, logical assignment ops are
        // never seen here, and its logical operation part is instead handled by
        // extractConditional().
        check_state!(
            !NodeUtil::is_logical_assignment_op(compiler, expr),
            "%s",
            expr.to_string(compiler)
        );
        let parent = expr.get_parent(compiler).expect("NullPointerException");

        let is_lhs_of_assign_op = NodeUtil::is_assignment_op(compiler, parent)
            && !parent.is_assign(compiler)
            && expr.is_first_child_of(compiler, Some(parent));

        let mut first_extracted_node: Option<NodeId> = None;

        // Expressions on the LHS of an assignment-op must have any possible
        // side-effects extracted as the value must be duplicated:
        //    next().foo += 2;
        // becomes:
        //    var t1 = next();
        //    t1.foo = t1.foo + 2;
        if is_lhs_of_assign_op && NodeUtil::is_normal_get(compiler, expr) {
            let mut n = expr.get_first_child(compiler);
            while let Some(cur) = n {
                // Cache next before extracting n because n is mutated or replaced within the
                // loop body during expression extraction.
                let next = cur.get_next(compiler);
                if !cur.is_string_lit(compiler) && !self.is_constant_name_node(compiler, cur) {
                    let extracted_node = self.extract_expression(compiler, cur, injection_point);
                    if first_extracted_node.is_none() {
                        first_extracted_node = Some(extracted_node);
                    }
                }
                n = next;
            }
        }

        // The temp is known to be constant.
        let temp_name = self.get_temp_constant_value_name();
        let replacement_value_node = IR::name(compiler, temp_name.as_str())
            .copy_type_from(compiler, expr)
            .srcref(compiler, expr);

        let temp_name_value: NodeId;

        // If it is ASSIGN_XXX and not a logical assignment operator keep the
        // assignment in place and extract the original value of the LHS operand.
        if is_lhs_of_assign_op {
            check_state!(
                expr.is_name(compiler) || NodeUtil::is_normal_get(compiler, expr),
                "%s",
                expr.to_string(compiler)
            );
            // Transform "x += 2" into "x = temp + 2"
            let op = NodeUtil::get_op_from_assignment_op(compiler, parent);
            let op_node = compiler
                .new_node(op)
                .copy_type_from(compiler, parent)
                .srcref_if_missing(compiler, parent);

            let right_operand = parent
                .get_last_child(compiler)
                .expect("NullPointerException");

            parent.set_token(compiler, Token::ASSIGN);
            right_operand.replace_with(compiler, op_node);
            op_node.add_child_to_front(compiler, replacement_value_node);
            op_node.add_child_to_back(compiler, right_operand);

            // The original expression is still being used, so make a clone.
            temp_name_value = expr.clone_tree(compiler);
        } else if expr.is_spread(compiler) {
            // We need to treat spreads differently because unlike other expressions, they can't
            // be directly assigned to new variables. Instead we wrap them in a literal.
            //
            // We make sure to do `var tmp = [...fn()];` rather than `var tmp = fn()` because the
            // execution of a spread on an arbitrary iterable/object can both have side-effects
            // and be side-effected. However, once done we are then sure that spreading `tmp` is
            // isolated.

            // Replace the expression with the spread for the temporary name.
            let spread_copy = expr.clone_node(compiler);
            spread_copy.add_child_to_back(compiler, replacement_value_node);
            expr.replace_with(compiler, spread_copy);

            // Move the original node into a legal context.
            temp_name_value = match parent.get_token(compiler) {
                Token::ARRAYLIT | Token::CALL | Token::NEW => {
                    let only_child = expr.get_only_child(compiler);
                    self.ast_factory
                        .create_arraylit(compiler, &[expr])
                        .srcref(compiler, only_child)
                }
                Token::OBJECTLIT => {
                    let only_child = expr.get_only_child(compiler);
                    self.ast_factory
                        .create_object_lit(compiler, &[expr])
                        .srcref(compiler, only_child)
                }
                _ => panic!(
                    "Unexpected parent of SPREAD:{}",
                    parent.to_string_tree(compiler)
                ),
            };
        } else {
            // Replace the expression with the temporary name.
            expr.replace_with(compiler, replacement_value_node);

            // Keep the original node so that CALL expressions can still be found
            // and inlined properly.
            temp_name_value = expr;
        }

        // Re-add the expression in the declaration of the temporary name.
        let temp_var_node =
            NodeUtil::new_var_node(compiler, temp_name.as_str(), Some(temp_name_value));
        let temp_var_name_node = temp_var_node
            .get_first_child(compiler)
            .expect("NullPointerException");
        temp_var_name_node.copy_type_from(compiler, temp_name_value);
        temp_var_name_node.set_inferred_constant_var(compiler, true);
        let containing_hoist_scope = self
            .scope
            .get_closest_hoist_scope(compiler)
            .expect("NullPointerException");
        containing_hoist_scope.declare(
            compiler,
            temp_name.as_str(),
            temp_var_name_node,
            /* input= */ None,
        );

        temp_var_node.insert_before(compiler, injection_point);

        let first_extracted_node = first_extracted_node.unwrap_or(temp_var_node);

        check_state!(first_extracted_node.is_var(compiler));
        first_extracted_node
    }

    /// Rewrite the call so "this" is preserved.
    ///
    /// ```text
    /// a.b(c);
    /// ```
    ///
    /// becomes:
    ///
    /// ```text
    /// var temp1 = a; var temp0 = temp1.b;
    /// temp0.call(temp1,c);
    /// ```
    // port: ExpressionDecomposer#rewriteCallExpression
    fn rewrite_call_expression(
        &mut self,
        compiler: &mut AbstractCompiler,
        call: NodeId,
        state: &mut DecompositionState,
    ) {
        check_argument!(call.is_call(compiler), "%s", call.to_string(compiler));
        let first = call
            .get_first_child(compiler)
            .expect("NullPointerException");
        check_argument!(
            NodeUtil::is_normal_get(compiler, first),
            "%s",
            first.to_string(compiler)
        );

        // Find the type of (fn expression).call
        let mut fn_call_type: Option<TypeId> = None;
        if self.ast_factory.is_adding_types() {
            let fn_type = first.get_jstype(compiler).expect("NullPointerException");
            let (registry, ast) = compiler.get_type_registry_and_ast();
            fn_call_type = if fn_type.is_function_type(registry) {
                Some(ObjectType::get_property_type(
                    fn_type
                        .to_maybe_function_type(registry)
                        .expect("NullPointerException"),
                    registry,
                    ast,
                    "call",
                ))
            } else {
                self.unknown_type
            };
        }

        // Extracts the expression representing the function to call. For example:
        //   "a['b'].c" from "a['b'].c()"
        let get_var_node = self.extract_expression(compiler, first, state.extract_before_statement);
        state.extract_before_statement = get_var_node;
        let function_name_node = get_var_node
            .get_first_child(compiler)
            .expect("NullPointerException")
            .clone_node(compiler);

        if call.get_boolean_prop(compiler, Prop::FREE_CALL) {
            // For a free call, we don't need to extract the receiver
            call.get_first_child(compiler)
                .expect("NullPointerException")
                .replace_with(compiler, function_name_node);
            return;
        }

        // Extracts the object reference to be used as "this". For example:
        //   "a['b']" from "a['b'].c"
        let get_expr_node = get_var_node
            .get_first_first_child(compiler)
            .expect("NullPointerException");
        check_argument!(
            NodeUtil::is_normal_get(compiler, get_expr_node),
            "%s",
            get_expr_node.to_string(compiler)
        );
        let orig_this_value = get_expr_node
            .get_first_child(compiler)
            .expect("NullPointerException");
        let receiver_node: NodeId;

        if orig_this_value.is_this(compiler) {
            // No need to create a variable for `this`, just clone it.
            receiver_node = orig_this_value.clone_node(compiler);
        } else if orig_this_value.is_super(compiler) {
            // Original callee was like `super.prop(args)`.
            // The correct way to call the value `super.prop` from a temporary variable is
            // `tmpVar.call(this, args)`, so just create a `this` here.
            let this_type = AstFactory::type_node(orig_this_value);
            receiver_node = self
                .ast_factory
                .create_this(compiler, this_type)
                .srcref(compiler, orig_this_value);
        } else {
            let this_var_node =
                self.extract_expression(compiler, orig_this_value, state.extract_before_statement);
            state.extract_before_statement = this_var_node;
            receiver_node = this_var_node
                .get_first_child(compiler)
                .expect("NullPointerException")
                .clone_node(compiler);
        }

        // CALL
        //   GETPROP "call"
        //     functionName
        //   thisName
        //   original-parameter1
        //   original-parameter2
        //   ...

        // Reuse the existing CALL node instead of creating a new one to avoid breaking
        // InlineFunction's bookkeeping. See b/124253050.
        call.remove_first_child(compiler);
        call.add_child_to_front(compiler, receiver_node);
        let call_type = AstFactory::type_jstype_and_color(
            fn_call_type,
            Some(standard_colors::TOP_OBJECT.clone()),
        );
        let get_prop = self
            .ast_factory
            .create_get_prop(compiler, function_name_node, "call", call_type)
            .srcref_tree_if_missing(compiler, call);
        call.add_child_to_front(compiler, get_prop);
        call.put_boolean_prop(compiler, Prop::FREE_CALL, false);
    }

    /// Allow the temp name to be overridden to make tests more readable.
    // port: ExpressionDecomposer#setTempNamePrefix
    pub fn set_temp_name_prefix(&mut self, prefix: impl Into<String>) {
        self.temp_name_prefix = prefix.into();
    }

    /// Create a unique temp name.
    // port: ExpressionDecomposer#getTempValueName
    fn get_temp_value_name(&self) -> String {
        format!(
            "{}{}{}",
            self.temp_name_prefix,
            ContextualRenamer::UNIQUE_ID_SEPARATOR,
            (self.safe_name_id_supplier)()
        )
    }

    /// Allow the temp name to be overridden to make tests more readable.
    // port: ExpressionDecomposer#setResultNamePrefix
    pub fn set_result_name_prefix(&mut self, prefix: impl Into<String>) {
        self.result_name_prefix = prefix.into();
    }

    /// Create a unique name for call results.
    // port: ExpressionDecomposer#getResultValueName
    fn get_result_value_name(&self) -> String {
        format!(
            "{}{}{}",
            self.result_name_prefix,
            ContextualRenamer::UNIQUE_ID_SEPARATOR,
            (self.safe_name_id_supplier)()
        )
    }

    /// Create a constant unique temp name.
    // port: ExpressionDecomposer#getTempConstantValueName
    fn get_temp_constant_value_name(&self) -> String {
        Self::get_temp_constant_value_name_with(&self.temp_name_prefix, &self.safe_name_id_supplier)
    }

    // The body of getTempConstantValueName, shared with the `this::getTempConstantValueName`
    // method reference handed to OptionalChainRewriter (a closure cannot borrow `self` there).
    fn get_temp_constant_value_name_with(
        temp_name_prefix: &str,
        safe_name_id_supplier: &Arc<dyn Fn() -> String + Send + Sync>,
    ) -> String {
        format!(
            "{}_const{}{}",
            temp_name_prefix,
            ContextualRenamer::UNIQUE_ID_SEPARATOR,
            safe_name_id_supplier()
        )
    }

    // port: ExpressionDecomposer#isTempConstantValueName
    fn is_temp_constant_value_name(&self, compiler: &AbstractCompiler, name: NodeId) -> bool {
        name.is_name(compiler)
            && name
                .get_string(compiler)
                .starts_with(JsString::from(format!(
                    "{}_const{}",
                    self.temp_name_prefix,
                    ContextualRenamer::UNIQUE_ID_SEPARATOR
                )))
    }

    /// @return For the subExpression, find the nearest statement Node before which it can be
    ///     inlined. Null if no such location can be found.
    // port: ExpressionDecomposer#findInjectionPoint
    pub fn find_injection_point(
        compiler: &AbstractCompiler,
        sub_expression: NodeId,
    ) -> Option<NodeId> {
        let expression_root = Self::find_expression_root(compiler, sub_expression);
        let expression_root = check_not_null!(expression_root);

        let mut injection_point = expression_root;

        let mut parent = injection_point
            .get_parent(compiler)
            .expect("NullPointerException");
        while parent.is_label(compiler) {
            injection_point = parent;
            parent = injection_point
                .get_parent(compiler)
                .expect("NullPointerException");
        }

        check_state!(
            NodeUtil::is_statement_block(compiler, parent),
            "%s",
            parent.to_string(compiler)
        );
        Some(injection_point)
    }

    /// @return Whether the node is a conditional op.
    // port: ExpressionDecomposer#isConditionalOp
    fn is_conditional_op(compiler: &AbstractCompiler, n: NodeId) -> bool {
        matches!(
            n.get_token(compiler),
            Token::HOOK
                | Token::AND
                | Token::OR
                | Token::COALESCE
                | Token::OPTCHAIN_GETELEM
                | Token::OPTCHAIN_GETPROP
                | Token::OPTCHAIN_CALL
        )
    }

    /// Finds the statement containing `subExpression`.
    ///
    /// If `subExpression` is not contained by a statement where inlining is known to be possible,
    /// `null` is returned. For example, the condition expression of a WHILE loop.
    // port: ExpressionDecomposer#findExpressionRoot
    fn find_expression_root(compiler: &AbstractCompiler, sub_expression: NodeId) -> Option<NodeId> {
        let mut child = sub_expression;
        for current in child.get_ancestors(compiler) {
            let parent = current.get_parent(compiler);
            match current.get_token(compiler) {
                // Supported expression roots:
                // SWITCH and IF can have multiple children, but the CASE, DEFAULT,
                // or BLOCK will be encountered first for any of the children other
                // than the condition.
                Token::EXPR_RESULT | Token::IF | Token::SWITCH | Token::RETURN | Token::THROW => {
                    check_state!(child.is_first_child_of(compiler, Some(current)));
                    return Some(current);
                }

                // Normalization will remove LABELs from VARs.
                Token::VAR | Token::LET | Token::CONST => {
                    if parent.is_some_and(|p| NodeUtil::is_any_for(compiler, p)) {
                        // Name declarations may not be roots if they're for-loop initializers.
                    } else {
                        return Some(current);
                    }
                }

                // Any of these indicate an unsupported expression:
                Token::FOR if child.is_first_child_of(compiler, Some(current)) => {
                    // Only the initializer of a for-loop could possibly be decomposed since the
                    // other statements need to execute each iteration.
                    return Some(current);
                }
                // fall through
                Token::FOR
                | Token::FOR_IN
                | Token::FOR_OF
                | Token::FOR_AWAIT_OF
                | Token::DO
                | Token::WHILE
                | Token::SCRIPT
                | Token::BLOCK
                | Token::LABEL
                | Token::CASE
                | Token::SWITCH_BODY
                | Token::DEFAULT_CASE
                | Token::DEFAULT_VALUE
                | Token::PARAM_LIST
                // For top-level class declarations, without this, we would have eventually
                // returned `null` due to `case SCRIPT` above. This just makes class expressions
                // behave the same way. Future optimizations may want to better handle class
                // members and static blocks.
                | Token::CLASS => {
                    return None;
                }

                _ => {}
            }
            child = current;
        }

        panic!("Unexpected AST structure.");
    }

    /// Determines if `subExpression` can be moved before `expressionRoot` without changing the
    /// behaviour of the code, or if there is a rewriting that would make such motion possible.
    ///
    /// Walks the AST from `subExpression` to `expressionRoot` and verifies that the portions of
    /// the `expressionRoot` subtree that are evaluated before `subExpression`:
    ///
    /// 1. are unaffected by the side-effects, if any, of the `subExpression`.
    /// 2. have no side-effects that may influence the `subExpression`.
    /// 3. have a syntactically legal rewriting.
    ///
    /// Examples:
    ///
    /// - `expressionRoot` = `a = 1 + x();`, `subExpression` = `x()`, has side-effects: `MOVABLE`
    ///   because the final value of `1` cannot be influenced by `x()`.
    /// - `expressionRoot` = `a = b + x();`, `subExpression` = `x()`, has side-effects:
    ///   `DECOMPOSABLE` because `b` may be modified by `x()`, but `b` can be cached.
    /// - `expressionRoot` = `a = b + x();`, `subExpression` = `x()`, no side-effects: `MOVABLE`
    ///   because `x()` can be computed before or after `b` is resolved.
    /// - `expressionRoot` = `a = (b = c) + x();`, `subExpression` = `x()`, no side-effects, is
    ///   side-effected: `DECOMPOSABLE` because `x()` may read `b`.
    ///
    /// @return `MOVABLE` if `subExpression` can already be moved; `DECOMPOSABLE` if the
    ///     `expressionRoot` subtree could be rewritten such that `subExpression` would be made
    ///     movable; `UNDECOMPOSABLE` otherwise.
    // port: ExpressionDecomposer#canExposeExpression
    pub fn can_expose_expression(
        &self,
        compiler: &mut AbstractCompiler,
        sub_expression: NodeId,
    ) -> DecompositionType {
        let expression_root = Self::find_expression_root(compiler, sub_expression);
        if let Some(expression_root) = expression_root {
            return self.is_subexpression_movable(compiler, expression_root, sub_expression);
        }
        DecompositionType::UNDECOMPOSABLE
    }

    /// @see #canExposeExpression
    // port: ExpressionDecomposer#isSubexpressionMovable
    fn is_subexpression_movable(
        &self,
        compiler: &mut AbstractCompiler,
        expression_root: NodeId,
        sub_expression: NodeId,
    ) -> DecompositionType {
        let mut requires_decomposition = false;
        let mut seen_side_effects = self
            .ast_analyzer
            .may_have_side_effects(compiler, sub_expression);

        if NodeUtil::is_opt_chain_node(compiler, sub_expression)
            && !NodeUtil::is_end_of_full_opt_chain(compiler, sub_expression)
        {
            // e.g `sub?.expression.rest?.of.expression`
            // It is always necessary to decompose the prefix of an optional chain.
            requires_decomposition = true;
        }
        let mut child = sub_expression;
        let mut ancestors = child.ancestors_cursor(compiler);
        while let Some(parent) = ancestors.next(compiler) {
            if NodeUtil::is_name_declaration(compiler, Some(parent))
                && !child.is_first_child_of(compiler, Some(parent))
            {
                // Most declarations are split by `Normalize` but to do so in loops would change
                // the behavior of the code.  We don't current handle this case but it is
                // possible:
                // For this case: `for (let x = 5, y = 2 * x; ...` where `child = y`.
                // As later expressions may reference the earlier expression, it would be
                // necessary to rewrite references when extracting the expressions:
                // `var temp1 = 5; var temp2 = 2 * temp1; for (let x = temp1, y = temp2; ...`
                return DecompositionType::UNDECOMPOSABLE;
            }

            if parent.is_tagged_template_lit(compiler)
                && !parent.get_boolean_prop(compiler, Prop::FREE_CALL)
            {
                // We're looking at something like: something.method`${subExpression()}`
                // You can't use the `.call(something, ...)` trick for a tagged template literal.
                // TODO(b/251958225): Implement decomposition of this case.
                return DecompositionType::UNDECOMPOSABLE;
            }

            if parent == expression_root {
                // Done. The walk back to the root of the expression is complete, and
                // nothing was encountered that blocks the call from being moved.
                return if requires_decomposition {
                    DecompositionType::DECOMPOSABLE
                } else {
                    DecompositionType::MOVABLE
                };
            }

            if Self::is_conditional_op(compiler, parent) {
                // Only the first child is always executed, otherwise it must be
                // decomposed.
                if Some(child) != parent.get_first_child(compiler) {
                    requires_decomposition = true;
                }
            } else {
                // Only inline the call if none of the preceding siblings in the
                // expression have side-effects, and are unaffected by the side-effects,
                // if any, of the call in question.
                // NOTE: The siblings are not always in the order in which they are evaluated, so
                // we call getEvaluationDirection to see in which order to traverse the siblings.

                // SPECIAL CASE: Assignment to a simple name
                if self.is_safe_assign(compiler, parent, seen_side_effects) {
                    // It is always safe to inline "foo()" for expressions such as
                    //   "a = b = c = foo();"
                    // As the assignment is unaffected by side effect of "foo()"
                    // and the names assigned-to cannot influence the state before
                    // the call to foo.
                    //
                    // This is not true of more complex LHS values, such as
                    //    a.x = foo();
                    //    next().x = foo();
                    // in these cases the checks below are necessary.
                } else {
                    // Everything else.
                    let direction = Self::get_evaluation_direction(compiler, parent);
                    let mut n = self.get_first_evaluated_child(compiler, parent, direction);
                    while let Some(cur) = n {
                        if cur == child {
                            // None of the preceding siblings have side-effects.
                            // This is OK.
                            break;
                        }

                        if self.is_expression_tree_unsafe(compiler, cur, seen_side_effects) {
                            seen_side_effects = true;
                            requires_decomposition = true;
                        }
                        n = self.get_next_evaluated_sibling(compiler, cur, direction);
                    }

                    let first = parent.get_first_child(compiler);
                    if requires_decomposition
                        && parent.is_call(compiler)
                        && first.is_some_and(|f| NodeUtil::is_normal_get(compiler, f))
                    {
                        return DecompositionType::DECOMPOSABLE;
                    }
                }
            }
            // Continue looking up the expression tree.
            child = parent;
        }

        // With a valid tree we should never get here.
        panic!("Unexpected.");
    }

    /// Returns the order in which the given node's children should be evaluated.
    ///
    /// In most cases, this is EvaluationDirection.FORWARD because the AST order matches the
    /// actual evaluation order. A few nodes require reversed evaluation instead.
    // port: ExpressionDecomposer#getEvaluationDirection
    fn get_evaluation_direction(compiler: &AbstractCompiler, node: NodeId) -> EvaluationDirection {
        match node.get_token(compiler) {
            Token::DESTRUCTURING_LHS | Token::ASSIGN | Token::DEFAULT_VALUE
                if node
                    .get_first_child(compiler)
                    .expect("NullPointerException")
                    .is_destructuring_pattern(compiler) =>
            {
                // The lhs of a destructuring assignment is evaluated AFTER the rhs. This is only
                // true for destructuring, though, not assignments like "first().x = second()"
                // where "first()" is evaluated first.
                EvaluationDirection::REVERSE
            }
            // fall through
            _ => EvaluationDirection::FORWARD,
        }
    }

    // port: ExpressionDecomposer#getFirstEvaluatedChild
    fn get_first_evaluated_child(
        &self,
        compiler: &AbstractCompiler,
        parent: NodeId,
        direction: EvaluationDirection,
    ) -> Option<NodeId> {
        if direction == EvaluationDirection::FORWARD {
            parent.get_first_child(compiler)
        } else {
            parent.get_last_child(compiler)
        }
    }

    // port: ExpressionDecomposer#getNextEvaluatedSibling
    fn get_next_evaluated_sibling(
        &self,
        compiler: &AbstractCompiler,
        node: NodeId,
        direction: EvaluationDirection,
    ) -> Option<NodeId> {
        if direction == EvaluationDirection::FORWARD {
            node.get_next(compiler)
        } else {
            node.get_previous(compiler)
        }
    }

    /// It is always safe to inline "foo()" for expressions such as "a = b = c = foo();" As the
    /// assignment is unaffected by side effect of "foo()" and the names assigned-to cannot
    /// influence the state before the call to foo.
    ///
    /// It is also safe in cases where the object is constant:
    ///
    /// ```text
    /// CONST_NAME.a = foo()
    /// CONST_NAME[CONST_VALUE] = foo();
    /// ```
    ///
    /// This is not true of more complex LHS values, such as
    ///
    /// ```text
    /// a.x = foo();
    /// next().x = foo();
    /// ```
    ///
    /// in these cases the checks below are necessary.
    ///
    /// @param seenSideEffects If true, check to see if node-tree maybe affected by side-effects,
    ///     otherwise if the tree has side-effects. @see #isExpressionTreeUnsafe
    /// @return Whether the assignment is safe from side-effects.
    // port: ExpressionDecomposer#isSafeAssign
    fn is_safe_assign(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        seen_side_effects: bool,
    ) -> bool {
        if n.is_assign(compiler) {
            let lhs = n.get_first_child(compiler).expect("NullPointerException");
            match lhs.get_token(compiler) {
                Token::NAME => {
                    return true;
                }
                Token::GETPROP => {
                    let lhs_first = lhs.get_first_child(compiler).expect("NullPointerException");
                    return !self.is_expression_tree_unsafe(compiler, lhs_first, seen_side_effects);
                }
                Token::GETELEM => {
                    let lhs_first = lhs.get_first_child(compiler).expect("NullPointerException");
                    let lhs_last = lhs.get_last_child(compiler).expect("NullPointerException");
                    return !self.is_expression_tree_unsafe(compiler, lhs_first, seen_side_effects)
                        && !self.is_expression_tree_unsafe(compiler, lhs_last, seen_side_effects);
                }
                _ => {}
            }
        }
        false
    }

    /// Determines if there is any subexpression below `tree` that would make it incorrect for
    /// some expression that follows `tree`, `E`, to be executed before `tree`.
    ///
    /// @param followingSideEffectsExist whether `E` causes side-effects.
    /// @return `true` if `tree` contains any subexpressions that would make movement incorrect.
    // port: ExpressionDecomposer#isExpressionTreeUnsafe
    fn is_expression_tree_unsafe(
        &self,
        compiler: &mut AbstractCompiler,
        tree: NodeId,
        following_side_effects_exist: bool,
    ) -> bool {
        if tree.is_spread(compiler) {
            // Spread expressions would cause recursive rewriting if not special cased here.
            // When extracted, spreads can't be assigned to a single variable and instead are put
            // into a literal. However, that literal must be spread again at the original site.
            // This check is what prevents the original spread from triggering recursion.
            let only_child = tree.get_only_child(compiler);
            if self.is_temp_constant_value_name(compiler, only_child) {
                return false;
            }
        }

        if following_side_effects_exist {
            // If the call to be inlined has side-effects, check to see if this
            // expression tree can be affected by any side-effects.

            // Assume that "tmp1.call(...)" is safe (where tmp1 is a const temp variable created by
            // ExpressionDecomposer) otherwise we end up trying to decompose the same tree
            // an infinite number of times. Also assume that "fn.call" is safe when "fn" is a
            // known constant function, as decomposing it may mess up InlineFunction's
            // bookkeeping if it is attempting to inline "fn".
            let parent = tree.get_parent(compiler).expect("NullPointerException");
            if NodeUtil::is_object_call_method(compiler, parent, &JsString::from("call"))
                || parent.get_boolean_prop(compiler, Prop::FREE_CALL)
            {
                let callee =
                    if tree.is_get_prop(compiler) && tree.get_string_ref(compiler) == "call" {
                        tree.get_first_child(compiler)
                            .expect("NullPointerException")
                    } else {
                        tree
                    };
                if tree.is_first_child_of(compiler, Some(parent))
                    && (self.is_temp_constant_value_name(compiler, callee)
                        || callee
                            .get_qualified_name(compiler)
                            .is_some_and(|q| self.known_constant_functions.contains(&q)))
                {
                    return false;
                }
            }

            if self
                .enabled_workarounds
                .contains(&Workaround::BROKEN_IE11_LOCATION_ASSIGN)
                && WINDOW_LOCATION_ASSIGN.matches(compiler, tree)
            {
                // IE11 throws an exception if we decompose a call to `window.location.assign`
                // e.g.
                // ```js
                // obj = window.location, method = obj.assign, method.call(obj, url); // exception
                // on IE11
                // ```
                // So we will consider it to be a constant value that doesn't need to be
                // decomposed to protect it against side effects.
                // This is a hack to work around a specific case we've seen in real world code.
                // We know it won't catch some cases, but we would rather miss those than add more
                // sophisticated logic that would also encounter false-positives (e.g. is
                // `x.assign` a reference to `window.location.assign`?).
                //
                // We cannot have canBeSideEffected() (below) do this check for us, because it
                // only accepts simple names as constants.
                return false;
            }
            // This is a superset of "AstAnalyzer.mayHaveSideEffects".
            NodeUtil::can_be_side_effected_with_scope(
                compiler,
                tree,
                &self.known_constant_functions,
                Some(self.scope),
            )
        } else {
            // The function called doesn't have side-effects but check to see if there
            // are side-effects that that may affect it.
            self.ast_analyzer.may_have_side_effects(compiler, tree)
        }
    }
}

/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/OptionalChainRewriter.java.

//! Port of `OptionalChainRewriter.java`: rewrites a single optional chain as one or more nested
//! hook expressions.
//!
//! Optional chains contained in OPTCHAIN_GETELEM indices or OPTCHAIN_CALL arguments are not
//! rewritten.
//!
//! ```text
//!   a?.b[obj?.index]?.c?.(obj?.arg)
//!   // becomes
//!   (tmp0 = a) == null
//!       ? void 0
//!       : (tmp1 = tmp0.b[obj?.index]) == null
//!           ? void 0
//!           : (tmp2 = tmp1.c) == null
//!               ? void 0
//!               : tmp2.call(tmp1, obj?.arg);
//! ```
//!
//! The unit tests for this class are in RewriteOptionalChainingOperatorTest, because it's most
//! convenient to test this class as part of the transpilation pass that uses it.

use crate::abstract_compiler::AbstractCompiler;
use crate::ast_factory::AstFactory;
use crate::node_util::NodeUtil;
use crate::scope::Scope;
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    node::{NodeId, Prop},
};
use std::collections::VecDeque;
use std::sync::Arc;

/// Creates unique names to be used for temporary variables.
///
/// The compiler is passed in because a pass never stores it (DESIGN §6); Java's creators read
/// compiler state such as `compiler.getUniqueIdSupplier()` through a captured reference.
pub trait TmpVarNameCreator {
    /// Creates a unique temporary variable name each time it is called.
    // port: OptionalChainRewriter.TmpVarNameCreator#createTmpVarName
    fn create_tmp_var_name(&self, compiler: &mut AbstractCompiler) -> String;
}

impl<F: Fn(&mut AbstractCompiler) -> String> TmpVarNameCreator for F {
    fn create_tmp_var_name(&self, compiler: &mut AbstractCompiler) -> String {
        self(compiler)
    }
}

pub struct OptionalChainRewriter {
    pub ast_factory: Arc<AstFactory>,
    pub tmp_var_name_creator: Arc<dyn TmpVarNameCreator + Send + Sync>,
    // If a scope is provided, newly created variables will be declared in that scope
    pub scope: Option<Scope>,
    pub chain_parent: NodeId,
    pub whole_chain: NodeId,
    pub insertion_point: NodeId,
    pub deletes_to_delete: VecDeque<NodeId>,
}

pub struct Builder {
    pub ast_factory: Arc<AstFactory>,
    pub tmp_var_name_creator: Option<Arc<dyn TmpVarNameCreator + Send + Sync>>,
    pub scope: Option<Scope>,
}

impl Builder {
    // port: OptionalChainRewriter.Builder#Builder
    fn new(compiler: &mut AbstractCompiler) -> Self {
        Self {
            ast_factory: Arc::new(compiler.create_ast_factory()),
            tmp_var_name_creator: None,
            scope: None,
        }
    }

    // port: OptionalChainRewriter.Builder#setTmpVarNameCreator
    pub fn set_tmp_var_name_creator(
        &mut self,
        tmp_var_name_creator: Arc<dyn TmpVarNameCreator + Send + Sync>,
    ) -> &mut Self {
        self.tmp_var_name_creator = Some(tmp_var_name_creator);
        self
    }

    /// Optionally sets a scope in which the rewriter will declare all temporary variables
    // port: OptionalChainRewriter.Builder#setScope
    pub fn set_scope(&mut self, scope: Scope) -> &mut Self {
        self.scope = Some(scope);
        self
    }

    /// @param wholeChain The last Node in the optional chain. Parent of all the rest.
    // port: OptionalChainRewriter.Builder#build
    pub fn build(
        &self,
        compiler: &mut AbstractCompiler,
        whole_chain: NodeId,
    ) -> OptionalChainRewriter {
        OptionalChainRewriter::new(compiler, self, whole_chain)
    }
}

impl OptionalChainRewriter {
    // port: OptionalChainRewriter#builder
    pub fn builder(compiler: &mut AbstractCompiler) -> Builder {
        Builder::new(compiler)
    }

    // port: OptionalChainRewriter#OptionalChainRewriter
    fn new(compiler: &mut AbstractCompiler, builder: &Builder, whole_chain: NodeId) -> Self {
        // This class will only operate on an entire chain.
        check_argument!(
            NodeUtil::is_end_of_full_opt_chain(compiler, whole_chain),
            "%s",
            whole_chain.to_string(compiler)
        );
        let ast_factory = builder.ast_factory.clone();
        let tmp_var_name_creator = check_not_null!(builder.tmp_var_name_creator.clone());
        let scope = builder.scope;
        let chain_parent = check_not_null!(
            whole_chain.get_parent(compiler),
            "%s",
            whole_chain.to_string(compiler)
        );
        // Insert temporaries just before the enclosing statement.
        let enclosing_statement =
            NodeUtil::get_enclosing_statement(compiler, whole_chain).expect("NullPointerException");
        let insertion_point = if enclosing_statement.get_previous(compiler).is_some()
            && enclosing_statement
                .get_previous(compiler)
                .unwrap()
                .is_label_name(compiler)
        {
            // Do not insert temporaries between a label name and its statement.
            // e.g. `{ label: for (const a of b?.c) {...}`
            // AST shape is
            // BLOCK
            //      LABEL // parent of enclosingStatement - insert above this
            //          LABEL_NAME
            //          enclosingStatement
            enclosing_statement
                .get_parent(compiler)
                .expect("NullPointerException")
        } else {
            enclosing_statement
        };
        Self {
            ast_factory,
            tmp_var_name_creator,
            scope,
            chain_parent,
            whole_chain,
            insertion_point,
            deletes_to_delete: VecDeque::new(),
        }
    }

    /// Rewrites the optional chain as a hook with temporary variables introduced as needed.
    // port: OptionalChainRewriter#rewrite
    pub fn rewrite(&mut self, compiler: &mut AbstractCompiler) {
        check_state!(
            NodeUtil::is_opt_chain_node(compiler, self.whole_chain),
            "already rewritten: %s",
            self.whole_chain.to_string(compiler)
        );

        // `first?.start.second?.start`
        // We search from the end of the chain and push the start nodes onto the stack, so the
        // first one ends up on top.
        let mut start_node_stack: VecDeque<NodeId> = VecDeque::new();
        let mut subchain_end = self.whole_chain;
        while NodeUtil::is_opt_chain_node(compiler, subchain_end) {
            let subchain_start = NodeUtil::get_start_of_opt_chain_segment(compiler, subchain_end);
            start_node_stack.push_front(subchain_start);
            subchain_end = subchain_start
                .get_first_child(compiler)
                .expect("NullPointerException");
        }

        check_state!(!start_node_stack.is_empty());
        // Each time we rewrite the initial segment of the chain, the remaining chain gets wrapped
        // in a hook statement like `(tmp0 = a.b) == null ? void 0 : tmp0.rest?.of.chain?.()`,
        // So wholeChain ends up more deeply nested on each rewrite.
        // We only care about the top-most replacement here.
        let whole_chain = self.whole_chain;
        let first_start = start_node_stack.pop_front().unwrap();
        let opt_chain_replacement =
            self.rewrite_initial_segment(compiler, first_start, whole_chain);
        while let Some(start) = start_node_stack.pop_front() {
            self.rewrite_initial_segment(compiler, start, whole_chain);
        }

        // Handle a non-optional call to an optional chain that ends in an element or property
        // access.
        // `(a?.optional.chain)(arg1)`
        // Writing JavaScript code like this is a bad idea, but it might get automatically
        // generated, so we must handle it.
        // The optional chain could evaluate to `undefined`, which we then try to call as a
        // function. However, if it isn't undefined, we have to preserve the correct `this` value
        // for the call.
        let chain_parent = self.chain_parent;
        if chain_parent.is_call(compiler)
            // If the call is explicitly a free call (e.g. `(0, a?.b)()`), we should not preserve
            // `this`.
            && !chain_parent.get_boolean_prop(compiler, Prop::FREE_CALL)
            // The chain will have been replaced by optChainReplacement during the rewriting above.
            && opt_chain_replacement.is_first_child_of(compiler, Some(chain_parent))
            // The wholeChain variable will still point to the rewritten final Node of the
            // chain. It will no longer be optional.
            && NodeUtil::is_normal_get(compiler, whole_chain)
        {
            let this_value = whole_chain
                .get_first_child(compiler)
                .expect("NullPointerException");
            let tmp_this_node = self.get_sub_expr_name_node(compiler, this_value);
            opt_chain_replacement.detach(compiler);
            chain_parent.add_child_to_front(compiler, tmp_this_node);
            let dot_call_node = self
                .ast_factory
                .create_get_prop_with_unknown_type(compiler, opt_chain_replacement, "call")
                .srcref_tree_if_missing(compiler, opt_chain_replacement);
            chain_parent.add_child_to_front(compiler, dot_call_node);
        }

        // Report changes here; chainParent can get deleted below this.
        // E.g. code `(p = a?.b)=>{ return p}` changes to `let tmp0; (p = (tmp0=a)==null?void
        // 0:tmp0.b)=>{return p}`
        // This requires recording scope changes at two places:
        // 1. the function scope changed from rewriting to HOOK (recorded here),
        // 2. SCRIPT scope changed due to the `let tmp0` inserted (recorded in
        // `declareTempVarName` where declarations are created).
        compiler.report_change_to_enclosing_scope(chain_parent);

        if chain_parent.is_del_prop(compiler) {
            // With rewriting above,
            // `delete a?.b?.c`
            //  synthesizes additional deletes
            // `delete (tmp0 = a) == null ? true : delete (tmp1 = tmp0.b) == null ? true : delete
            // tmp1.c;`
            //
            // But we must generate only:
            // `(tmp0 = a) == null ? true : (tmp1 = tmp0.b) == null ? true : delete tmp1.c;`
            // That is, the preceding deletes for every hook must be removed.

            while let Some(delete) = self.deletes_to_delete.pop_front() {
                check_state!(
                    delete
                        .get_first_child(compiler)
                        .expect("NullPointerException")
                        .is_hook(compiler),
                    "%s",
                    delete.to_string(compiler)
                );
                let hook = delete.get_first_child(compiler).unwrap();
                hook.detach(compiler);
                delete.replace_with(compiler, hook);
                compiler.report_change_to_enclosing_scope(hook);
            }
        }

        // Transpilation of the optional chain adds `let` declarations for temporary variables.
        // NOTE: If this class is being used before transpilation, it's OK to use `let`, since it
        // will be transpiled away, if necessary. If it is being used after transpilation, then
        // using `let` must be OK, because optional chains weren't transpiled away and `let`
        // existed before they did.
        let enclosing_script = NodeUtil::get_enclosing_script(compiler, self.insertion_point)
            .expect("NullPointerException");
        NodeUtil::add_feature_to_script(compiler, enclosing_script, Feature::LET_DECLARATIONS);
    }

    /// Rewrites the first part of a possibly-multi-part optional chain.
    ///
    /// ```text
    /// a()?.b.c?.d;
    /// // becomes
    /// let tmp0;
    /// (tmp0 = a()) == null
    ///     ? void 0
    ///     : tmp0.b.c?d;
    /// ```
    ///
    /// If this optional chain is under a delete, the l-r rewriting must synthesize another
    /// `delete` to ensure the next chain(if present), knows that it must delete the
    /// `fullChainEnd`.
    ///
    /// ```text
    /// delete a?.b.c?.d;
    /// // becomes
    /// let tmp0;
    /// (tmp0 = a()) == null
    ///     ? true
    ///     : delete tmp0.b.c?d;
    /// ```
    ///
    /// @param fullChainStart The very first `?.` node
    /// @param fullChainEnd The very last optional chain node.
    /// @return The hook expression that replaced the chain.
    // port: OptionalChainRewriter#rewriteInitialSegment
    fn rewrite_initial_segment(
        &mut self,
        compiler: &mut AbstractCompiler,
        full_chain_start: NodeId,
        full_chain_end: NodeId,
    ) -> NodeId {
        // `receiverNode?.restOfChain`
        let mut receiver_node = full_chain_start
            .get_first_child(compiler)
            .expect("NullPointerException");
        // for `a?.b.c?.d`, this will be `a?.b.c`, because the NodeUtil method finds the end
        // of the sub-chain, not the full chain.
        let initial_chain_end = NodeUtil::get_end_of_opt_chain_segment(compiler, full_chain_start);

        // Is this optional chain under delete
        let full_chain_end_parent = full_chain_end
            .get_parent(compiler)
            .expect("NullPointerException");
        let is_being_deleted = full_chain_end_parent.is_del_prop(compiler);
        if is_being_deleted {
            self.deletes_to_delete.push_back(full_chain_end_parent);
        }

        // If the receiver is an optional chain, we weren't really given the start of a full
        // chain.
        check_argument!(
            !NodeUtil::is_opt_chain_node(compiler, receiver_node),
            "%s",
            receiver_node.to_string(compiler)
        );

        // change the initial chain's nodes to be non-optional
        NodeUtil::convert_to_non_optional_chain_segment(compiler, initial_chain_end);

        let placeholder = IR::empty(compiler);
        full_chain_end.replace_with(compiler, placeholder);
        // NOTE: convertToNonOptionalChain() above will have made the chain start
        // and all the other nodes in the first segment of the chain non-optional,
        // so fullChainStart.isCall() is the right test here.
        if NodeUtil::is_normal_get(compiler, receiver_node)
            && full_chain_start.is_call(compiler)
            // Check !FREE_CALL to ensure we don't treat free calls on property
            // access (e.g. `(0, expr.prop)?.()`) as method calls.
            && !full_chain_start.get_boolean_prop(compiler, Prop::FREE_CALL)
        {
            // `expr.prop?.(x).y`
            // Needs to become
            // `(t1 = (t0 = expr).prop) == null ? void 0 : t1.call(t0, x).y`
            let this_value = receiver_node
                .get_first_child(compiler)
                .expect("NullPointerException");
            let tmp_this_node = self.get_sub_expr_name_node(compiler, this_value);
            let tmp_receiver_node = self.get_sub_expr_name_node(compiler, receiver_node);
            receiver_node = full_chain_start
                .remove_first_child(compiler)
                .expect("NullPointerException");
            full_chain_start.add_child_to_front(compiler, tmp_this_node);
            let get_prop = self
                .ast_factory
                .create_get_prop_with_unknown_type(compiler, tmp_receiver_node, "call")
                .srcref_tree_if_missing(compiler, receiver_node);
            full_chain_start.add_child_to_front(compiler, get_prop);
        } else {
            // `expr?.x.y`
            // needs to become
            // `((t0 = expr) == null) ? void 0 : t0.x.y`
            let tmp_receiver_node = self.get_sub_expr_name_node(compiler, receiver_node);
            receiver_node = full_chain_start
                .get_first_child(compiler)
                .expect("NullPointerException");
            receiver_node.replace_with(compiler, tmp_receiver_node);
        }
        let null_node = self.ast_factory.create_null(compiler);
        let cond = self
            .ast_factory
            .create_eq(compiler, receiver_node, null_node);
        let true_value = if is_being_deleted {
            self.ast_factory.create_boolean(compiler, true)
        } else {
            self.ast_factory.create_undefined_value(compiler)
        };
        let false_value = if is_being_deleted {
            self.ast_factory.create_del_prop(compiler, full_chain_end)
        } else {
            full_chain_end
        };
        let opt_chain_replacement = self
            .ast_factory
            .create_hook(compiler, cond, true_value, false_value)
            .srcref_tree_if_missing(compiler, full_chain_end);

        placeholder.replace_with(compiler, opt_chain_replacement);

        opt_chain_replacement
    }

    /// Given an expression node, declare a temporary variable to hold that expression and replace
    /// the expression with `(tmp = expr)`.
    ///
    /// e.g. `subExpr.moreExpr` becomes `(tmp = subExpr).moreExpr`, and `let tmp;` gets inserted
    /// before the enclosing statement of this optional chain.
    ///
    /// @param subExpr The sub expression Node
    /// @return A detached NAME node for the temporary variable name and with source info and type
    ///     matching `subExpr`, that may be inserted where needed.
    // port: OptionalChainRewriter#getSubExprNameNode
    pub fn get_sub_expr_name_node(
        &mut self,
        compiler: &mut AbstractCompiler,
        sub_expr: NodeId,
    ) -> NodeId {
        let temp_var_name = self.declare_temp_var_name(compiler, sub_expr);
        let placeholder = IR::empty(compiler);
        sub_expr.replace_with(compiler, placeholder);
        sub_expr.remove_prop(compiler, Prop::DIRECT_EVAL);
        let replacement = self
            .ast_factory
            .create_assign_to_name(compiler, temp_var_name, sub_expr)
            .srcref_tree_if_missing(compiler, sub_expr);
        placeholder.replace_with(compiler, replacement);
        replacement
            .get_first_child(compiler)
            .expect("NullPointerException")
            .clone_node(compiler)
    }

    /// Declare a temporary variable name that will be used to hold the given value.
    ///
    /// The generated declaration has no assignment, it's just `let tmp;`.
    ///
    /// @param valueNode A node from which to copy the source info and type to be used for the new
    ///     variable.
    /// @return the name used for the new temporary variable.
    // port: OptionalChainRewriter#declareTempVarName
    pub fn declare_temp_var_name(
        &mut self,
        compiler: &mut AbstractCompiler,
        value_node: NodeId,
    ) -> String {
        let temp_var_name = self.tmp_var_name_creator.create_tmp_var_name(compiler);
        let declaration_statement = self
            .ast_factory
            .create_single_let_name_declaration(compiler, temp_var_name.as_str())
            .srcref_tree(compiler, value_node);
        declaration_statement
            .get_first_child(compiler)
            .expect("NullPointerException")
            .set_inferred_constant_var(compiler, true);
        declaration_statement.insert_before(compiler, self.insertion_point);
        compiler.report_change_to_enclosing_scope(declaration_statement);
        if let Some(scope) = self.scope {
            let name_node = declaration_statement.get_first_child(compiler);
            scope.declare(compiler, temp_var_name.as_str(), name_node, None);
        }
        temp_var_name
    }
}

/*
 * Copyright 2005 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/InlineFunctions.java.

//! Port of `com.google.javascript.jscomp.InlineFunctions`.
//!
//! Inlines functions that are divided into two types: "direct call node replacement" (aka
//! "direct") and as a block of statements (aka block). Function that can be inlined "directly"
//! functions consist of a single return statement, everything else is must be inlined as a
//! "block". These functions must meet these general requirements: - it is not recursive - the
//! function does not contain another function -- these may be intentional to to limit the scope
//! of closures. - function is called only once OR the size of the inline function is smaller
//! than the call itself. - the function name is not referenced in any other manner
//!
//! "directly" inlined functions must meet these additional requirements: - consists of a single
//! return statement
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_options::{CompilerOptions, InstrumentOption, PropertyCollapseLevel, Reach},
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    function_argument_injector::FunctionArgumentInjector,
    function_injector::{self, CanInlineResult, FunctionInjector, InliningMode},
    js_chunk::JSChunk,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::{MatchDeclaration, MatchShallowStatement, NodeUtil},
    scope::Scope,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use std::{rc::Rc, sync::Arc};

pub static MISSED_REQUIRED_INLINING: DiagnosticType = DiagnosticType::warning(
    "JSC_MISSED_REQUIRED_INLINING",
    "function {1} annotated @requireInlining could not be inlined here",
);

pub struct InlineFunctions {
    // TODO(nicksantos): This needs to be completely rewritten to use scopes
    // to do variable lookups. Right now, it assumes that all functions are
    // uniquely named variables. There's currently a stopgap scope-check
    // to ensure that this doesn't produce invalid code. But in the long run,
    // this needs a major refactor.
    fns: IndexMap<JsString, FunctionState>,
    anon_fns: IndexMap<NodeId, JsString>,

    injector: FunctionInjector,
    function_argument_injector: Rc<FunctionArgumentInjector>,

    reach: Reach,
    assume_minimum_capture: bool,

    enforce_max_size_after_inlining: bool,
    max_size_after_inlining: i32,
}

impl InlineFunctions {
    // port: InlineFunctions#InlineFunctions
    pub fn new(
        compiler: &AbstractCompiler,
        safe_name_id_supplier: Arc<dyn Fn() -> String + Send + Sync>,
        reach: Reach,
        assume_strict_this: bool,
        assume_minimum_capture: bool,
        max_size_after_inlining: i32,
    ) -> Self {
        check_argument!(reach != Reach::NONE);

        // TODO(b/124253050): Update bookkeeping logic and reenable method call inliing.
        // Method call decomposition creates new call nodes after all the analysis
        // is done, which would cause such calls to function to be left behind after
        // the function itself is removed.  The function inliner need to be made
        // aware of these new calls in order to enable it.

        let function_argument_injector =
            Rc::new(FunctionArgumentInjector::new(compiler.get_ast_analyzer()));
        let injector = function_injector::Builder::new(compiler)
            .safe_name_id_supplier(safe_name_id_supplier)
            .assume_strict_this(assume_strict_this)
            .assume_minimum_capture(assume_minimum_capture)
            .function_argument_injector(function_argument_injector.clone())
            .build(compiler);
        Self {
            fns: IndexMap::<_, _>::default(),
            anon_fns: IndexMap::<_, _>::default(),
            injector,
            function_argument_injector,
            reach,
            assume_minimum_capture,
            enforce_max_size_after_inlining: max_size_after_inlining
                != CompilerOptions::UNLIMITED_FUN_SIZE_AFTER_INLINING,
            max_size_after_inlining,
        }
    }

    // port: InlineFunctions#getOrCreateFunctionState
    fn get_or_create_function_state(&mut self, fn_name: &JsString) -> &mut FunctionState {
        self.fns
            .entry(fn_name.clone())
            .or_insert_with(FunctionState::new)
    }

    // port: InlineFunctions#isAlwaysInlinable
    fn is_always_inlinable(ast: &Ast, r#fn: NodeId) -> bool {
        check_argument!(r#fn.is_function(ast));
        let body = NodeUtil::get_function_body(ast, r#fn);
        (!body.has_children(ast))
            || (body.has_one_child(ast) && body.get_first_child(ast).unwrap().is_return(ast))
    }

    // port: InlineFunctions#targetSizeAfterInlineExceedsLimit
    fn target_size_after_inline_exceeds_limit(
        max_size_after_inlining: i32,
        t: &mut NodeTraversal<'_>,
        function_state: &FunctionState,
    ) -> bool {
        let containing_function = t.get_enclosing_function();
        // Always inline at the top level,
        // unless maybeAddFunction has marked functionState as not inlinable.
        let Some(containing_function) = containing_function else {
            return false;
        };
        let inlined_fun = function_state.get_fn().get_function_node(t);
        if Self::is_always_inlinable(t, inlined_fun) {
            return false;
        }

        let inlined_fun_body = NodeUtil::get_function_body(t, inlined_fun);
        let inlined_fun_size =
            NodeUtil::count_ast_size_up_to_limit(t, inlined_fun_body, max_size_after_inlining);
        let target_fun_size =
            NodeUtil::count_ast_size_up_to_limit(t, containing_function, max_size_after_inlining);
        inlined_fun_size + target_fun_size > max_size_after_inlining
    }

    /// Updates the FunctionState object for the given function. Checks if the given function
    /// matches the criteria for an inlinable function.
    // port: InlineFunctions#maybeAddFunction
    #[allow(clippy::nonminimal_bool)] // Java's checkState expression.
    fn maybe_add_function(
        &mut self,
        compiler: &mut AbstractCompiler,
        r#fn: Function,
        chunk: Option<JSChunk>,
    ) {
        let name = r#fn.get_name(compiler);
        self.get_or_create_function_state(&name);
        // Rust-only: `fn` moves into the FunctionState below; its FUNCTION node is read first.
        let fn_node = r#fn.get_function_node(compiler);
        self.update_function_state_for_inlining(compiler, r#fn, chunk, &name);
        let function_state = &self.fns[&name];
        check_state!(
            !(Self::has_require_inlining_annotation(compiler, fn_node)
                && !function_state.can_inline())
        );
    }

    /// Updates the FunctionState object for the given function. Checks if the given function
    /// matches the criteria for an inlinable function.
    ///
    /// Java passes the FunctionState of `name`; here it is looked up in `fns`.
    // port: InlineFunctions#updateFunctionStateForInlining
    fn update_function_state_for_inlining(
        &mut self,
        compiler: &mut AbstractCompiler,
        r#fn: Function,
        chunk: Option<JSChunk>,
        name: &JsString,
    ) {
        // TODO(johnlenz): Maybe "smarten" FunctionState by adding this logic to it?
        let Self {
            fns,
            injector,
            function_argument_injector,
            assume_minimum_capture,
            enforce_max_size_after_inlining,
            max_size_after_inlining,
            ..
        } = self;
        let function_state = fns.get_mut(name).unwrap();

        // If the function has multiple definitions, don't inline it.
        if function_state.has_existing_function_definition() {
            function_state.disallow_inlining(
                compiler,
                DisallowInliningReason::MULTIPLE_FUNCTION_DEFINITIONS,
            );
            return;
        }
        let fn_node = r#fn.get_function_node(compiler);

        if Self::has_no_inline_annotation(compiler, fn_node) {
            function_state
                .disallow_inlining(compiler, DisallowInliningReason::NO_INLINE_ANNOTATION);
            return;
        }

        if *enforce_max_size_after_inlining
            && !Self::is_always_inlinable(compiler, fn_node)
            && *max_size_after_inlining
                <= NodeUtil::count_ast_size_up_to_limit(compiler, fn_node, *max_size_after_inlining)
        {
            function_state
                .disallow_inlining(compiler, DisallowInliningReason::TARGET_SIZE_EXCEEDS_LIMIT);
            return;
        }

        // verify the function hasn't already been marked as "don't inline"
        if function_state.can_inline() {
            // store it for use when inlining.
            function_state.set_fn(r#fn);
            if FunctionInjector::is_direct_call_node_replacement_possible(
                compiler,
                function_state.get_fn().get_function_node(compiler),
            ) {
                function_state.inline_directly(true);
            }

            if Self::has_non_inlinable_param(
                compiler,
                NodeUtil::get_function_parameters(compiler, fn_node),
            ) {
                function_state
                    .disallow_inlining(compiler, DisallowInliningReason::NON_INLINABLE_PARAM);
            }

            // verify the function meets all the requirements.
            // TODO(johnlenz): Minimum requirement checks are about 5% of the
            // run-time cost of this pass.
            if !Self::is_candidate_function(compiler, injector, function_state.get_fn()) {
                // It doesn't meet the requirements.
                function_state
                    .disallow_inlining(compiler, DisallowInliningReason::NOT_CANDIDATE_FUNCTION);
            }

            // Set the chunk and gather names that need temporaries.
            if function_state.can_inline() {
                function_state.set_chunk(chunk);

                let names_to_alias =
                    function_argument_injector.find_modified_parameters(compiler, fn_node);
                if !names_to_alias.is_empty() {
                    function_state.inline_directly(false);
                    function_state.set_names_to_alias(names_to_alias);
                }

                let block = NodeUtil::get_function_body(compiler, fn_node);
                if NodeUtil::references_enclosing_receiver(compiler, block) {
                    function_state.set_references_this(true);
                }

                if NodeUtil::has(compiler, block, &|ast, n| n.is_function(ast), &|_, _| true) {
                    function_state.set_has_inner_functions(true);
                    // If there are inner functions, we can inline into global scope
                    // if there are no local vars or named functions.
                    // TODO(johnlenz): this can be improved by looking at the possible
                    // values for locals.  If there are simple values, or constants
                    // we could still inline.
                    if !*assume_minimum_capture && Self::has_local_names(compiler, fn_node) {
                        function_state
                            .disallow_inlining(compiler, DisallowInliningReason::HAS_LOCAL_NAMES);
                    }
                }
            }

            if fn_node.get_grandparent(compiler).unwrap().is_var(compiler) {
                let block = function_state
                    .get_fn()
                    .get_declaring_block(compiler)
                    .unwrap();
                if block.is_block(compiler)
                    && !block.get_parent(compiler).unwrap().is_function(compiler)
                    && NodeUtil::has(
                        compiler,
                        block,
                        &|ast, n| n.is_let(ast) || n.is_const(ast),
                        &|_, _| true,
                    )
                {
                    // The function might capture a variable that's not in scope at the call
                    // site, so don't inline.
                    function_state.disallow_inlining(
                        compiler,
                        DisallowInliningReason::CAPTURED_VARIABLE_NOT_IN_SCOPE,
                    );
                }
            }

            if fn_node.is_generator_function(compiler) {
                function_state
                    .disallow_inlining(compiler, DisallowInliningReason::GENERATOR_FUNCTION);
            }

            if fn_node.is_async_function(compiler) {
                function_state.disallow_inlining(compiler, DisallowInliningReason::ASYNC_FUNCTION);
            }
        }
    }

    // port: InlineFunctions#hasNoInlineAnnotation
    fn has_no_inline_annotation(ast: &Ast, fn_node: NodeId) -> bool {
        let js_doc_info = NodeUtil::get_best_jsdoc_info(ast, fn_node);
        js_doc_info.is_some_and(|js_doc_info| js_doc_info.is_no_inline())
    }

    // port: InlineFunctions#hasEncourageInliningAnnotation
    fn has_encourage_inlining_annotation(ast: &Ast, fn_node: NodeId) -> bool {
        let js_doc_info = NodeUtil::get_best_jsdoc_info(ast, fn_node);
        js_doc_info.is_some_and(|js_doc_info| js_doc_info.is_encourage_inlining())
    }

    // port: InlineFunctions#hasRequireInliningAnnotation
    fn has_require_inlining_annotation(ast: &Ast, fn_node: NodeId) -> bool {
        let js_doc_info = NodeUtil::get_best_jsdoc_info(ast, fn_node);
        js_doc_info.is_some_and(|js_doc_info| js_doc_info.is_require_inlining())
    }

    /// `fn_node` is the function to inspect. Returns whether the function has parameters,
    /// var/const/let, class, or function declarations.
    // port: InlineFunctions#hasLocalNames
    fn has_local_names(ast: &Ast, fn_node: NodeId) -> bool {
        let block = NodeUtil::get_function_body(ast, fn_node);
        NodeUtil::get_function_parameters(ast, fn_node).has_children(ast)
            || NodeUtil::has(
                ast,
                block,
                &|ast, n| MatchDeclaration.apply(ast, n),
                &|ast, n| MatchShallowStatement.apply(ast, n),
            )
    }

    /// Checks if the given function matches the criteria for an inlinable function.
    // port: InlineFunctions#isCandidateFunction
    fn is_candidate_function(
        compiler: &AbstractCompiler,
        injector: &FunctionInjector,
        r#fn: &Function,
    ) -> bool {
        // Don't inline exported functions.
        let fn_name = r#fn.get_name(compiler);
        if compiler.get_coding_convention().is_exported_name(&fn_name) {
            // TODO(johnlenz): Should we allow internal references to be inlined?
            // An exported name can be replaced externally, any inlined instance
            // would not reflect this change.
            // To allow inlining we need to be able to distinguish between exports
            // that are used in a read-only fashion and those that can be replaced
            // by external definitions.
            return false;
        }

        // Don't inline this special function
        if compiler
            .get_coding_convention()
            .is_property_rename_function(compiler, r#fn.get_name_node(compiler))
        {
            return false;
        }

        let fn_node = r#fn.get_function_node(compiler);
        injector.does_function_meet_minimum_requirements(compiler, &fn_name, fn_node)
    }

    /// Returns whether the name is used in a way that might be a candidate for inlining.
    // port: InlineFunctions#isCandidateUsage
    fn is_candidate_usage(ast: &Ast, name: NodeId) -> bool {
        let parent = name.get_parent(ast).unwrap();
        check_state!(name.is_name(ast));
        if NodeUtil::is_name_declaration(ast, Some(parent)) || parent.is_function(ast) {
            // This is a declaration.  Duplicate declarations are handle during
            // function candidate gathering.
            return true;
        }

        if NodeUtil::is_normal_or_opt_chain_call(ast, parent)
            && parent.get_first_child(ast) == Some(name)
        {
            // This is a normal reference to the function.
            return true;
        }

        // Check for a ".call" to the named function:
        //   CALL
        //     GETPROP/GETELEM
        //       NAME
        //       STRING == "call"
        //     This-Value
        //     Function-parameter-1
        //     ...
        if (parent.is_get_elem(ast)
            && Some(name) == parent.get_first_child(ast)
            && parent.get_second_child(ast).unwrap().is_string_lit(ast)
            && parent.get_second_child(ast).unwrap().get_string_ref(ast) == "call")
            || (parent.is_get_prop(ast) && parent.get_string_ref(ast) == "call")
        {
            let grandparent = name.get_ancestor(ast, 2).unwrap();
            if grandparent.is_call(ast) && grandparent.get_first_child(ast) == Some(parent) {
                // Yep, a ".call".
                return true;
            }
        }
        false
    }

    /// Remove entries that aren't a valid inline candidates, from the list of encountered names.
    // port: InlineFunctions#trimCandidatesNotMeetingMinimumRequirements
    fn trim_candidates_not_meeting_minimum_requirements(&mut self) {
        self.fns.retain(|_, function_state| {
            !(!function_state.has_existing_function_definition() || !function_state.can_inline())
        });
    }

    /// Remove entries from the list of candidates that can't be inlined.
    // port: InlineFunctions#trimCandidatesUsingOnCost
    fn trim_candidates_using_on_cost(&mut self, compiler: &mut AbstractCompiler) {
        let injector = &self.injector;
        self.fns.retain(|_, function_state| {
            if function_state.has_references() {
                // Only inline function if it decreases the code size.
                let lowers_cost = Self::minimize_cost(compiler, injector, function_state);
                if !lowers_cost {
                    // It shouldn't be inlined; remove it from the list.
                    return false;
                }
            } else if !function_state.can_remove() {
                // Don't bother tracking functions without references that can't be
                // removed.
                return false;
            }
            true
        });
    }

    /// Determines if the function is worth inlining and potentially trims references that
    /// increase the cost.
    ///
    /// Returns whether inlining the references lowers the overall cost.
    // port: InlineFunctions#minimizeCost
    fn minimize_cost(
        compiler: &mut AbstractCompiler,
        injector: &FunctionInjector,
        function_state: &mut FunctionState,
    ) -> bool {
        if !Self::inlining_lowers_cost(compiler, injector, function_state) {
            // Try again without Block inlining references
            if function_state.has_block_inlining_references() {
                function_state.set_remove(
                    false,
                    compiler,
                    CannotRemoveReason::BLOCK_INLINING_REFERENCE,
                    ShouldWarnWhenRequireInliningCannotInline::YES,
                );
                function_state.remove_block_inlining_references();
                if !function_state.has_references()
                    || !Self::inlining_lowers_cost(compiler, injector, function_state)
                {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }

    /// Returns whether inlining the function reduces code size.
    // port: InlineFunctions#inliningLowersCost
    fn inlining_lowers_cost(
        compiler: &AbstractCompiler,
        injector: &FunctionInjector,
        function_state: &mut FunctionState,
    ) -> bool {
        function_state.require_inlining(compiler)
            || function_state.encourage_inlining(compiler)
            || {
                let refs: Vec<&function_injector::Reference> =
                    function_state.get_references().map(|r| &r.base).collect();
                injector.inlining_lowers_cost(
                    compiler,
                    function_state.get_chunk(),
                    function_state.get_fn().get_function_node(compiler),
                    &refs,
                    &function_state.get_names_to_alias(),
                    function_state.can_remove(),
                    function_state.get_references_this(),
                )
            }
    }

    /// Size base inlining calculations are thrown off when a function that is being inlined also
    /// contains calls to functions that are slated for inlining.
    ///
    /// Specifically, a clone of the FUNCTION node tree is used when the function is inlined.
    /// Calls in this new tree are not included in the list of function references so they won't
    /// be inlined (which is what we want). Here we mark those functions as non-removable (as they
    /// will have new references in the cloned node trees).
    ///
    /// This prevents a function that would only be inlined because it is referenced once from
    /// being inlined into multiple call sites because the calling function has been inlined in
    /// multiple locations or the function being removed while there are still references.
    // port: InlineFunctions#resolveInlineConflicts
    fn resolve_inline_conflicts(&mut self, compiler: &mut AbstractCompiler) {
        // Rust-only: the function states are visited by index, as the conflicts of one function
        // update the states of the functions it calls.
        for function_state in 0..self.fns.len() {
            self.resolve_inline_conflicts_for_function(compiler, function_state);
        }
    }

    /// Returns whether the function has any parameters that would stop the compiler from
    /// inlining. Currently this includes object patterns, array patterns, and default values.
    // port: InlineFunctions#hasNonInlinableParam
    fn has_non_inlinable_param(ast: &Ast, node: NodeId) -> bool {
        let pred = |_ast: &Ast, input: NodeId| -> bool {
            input.is_default_value(_ast) || input.is_destructuring_pattern(_ast)
        };
        NodeUtil::has(ast, node, &pred, &|_, _| true)
    }

    /// See `resolve_inline_conflicts`. `function_state` is the index of the FunctionState in
    /// `fns`.
    // port: InlineFunctions#resolveInlineConflictsForFunction
    fn resolve_inline_conflicts_for_function(
        &mut self,
        compiler: &mut AbstractCompiler,
        function_state: usize,
    ) {
        // Functions that aren't referenced don't cause conflicts.
        if !self.fns[function_state].has_references() || !self.fns[function_state].can_inline() {
            return;
        }

        let fn_node = self.fns[function_state]
            .get_fn()
            .get_function_node(compiler);
        let names = Self::find_called_functions(compiler, fn_node);
        if !names.is_empty() {
            // Prevent the removal of the referenced functions unless they will be immediately
            // inlined.
            for name in &names {
                let fs_called = self.fns.get_mut(name);
                if let Some(fs_called) = fs_called
                    && fs_called.can_remove()
                {
                    fs_called.set_remove(
                        false,
                        compiler,
                        CannotRemoveReason::CALLED_BY_INLINED_FUNCTION,
                        ShouldWarnWhenRequireInliningCannotInline::NO,
                    );
                    // For functions that can no longer be removed, check if they should
                    // still be inlined.
                    if !Self::minimize_cost(compiler, &self.injector, fs_called)
                        && !fs_called.require_inlining(compiler)
                        && !fs_called.encourage_inlining(compiler)
                    {
                        // It can't be inlined remove it from the list.
                        fs_called.disallow_inlining(compiler, DisallowInliningReason::COST);
                    }
                }
            }

            // Make a copy of the Node, so it isn't changed by other inlines.
            let safe_fn_node = self.fns[function_state]
                .get_fn()
                .get_function_node(compiler)
                .clone_tree(compiler);
            self.fns[function_state].set_safe_fn_node(safe_fn_node);
        }
    }

    /// This functions that may be called directly.
    // port: InlineFunctions#findCalledFunctions(Node)
    fn find_called_functions(ast: &Ast, node: NodeId) -> IndexSet<JsString> {
        let mut changed = IndexSet::<_>::default();
        Self::find_called_functions_into(ast, NodeUtil::get_function_body(ast, node), &mut changed);
        changed
    }

    /// See `find_called_functions`.
    // port: InlineFunctions#findCalledFunctions(Node,Set)
    fn find_called_functions_into(ast: &Ast, node: NodeId, changed: &mut IndexSet<JsString>) {
        // For each referenced function, add a new reference
        if node.is_name(ast) && Self::is_candidate_usage(ast, node) {
            changed.insert(node.get_string(ast));
        }

        let mut c = node.get_first_child(ast);
        while let Some(child) = c {
            Self::find_called_functions_into(ast, child, changed);
            c = child.get_next(ast);
        }
    }

    /// For any call-site that needs it, prepare the call-site for inlining by rewriting the
    /// containing expression.
    // port: InlineFunctions#decomposeExpressions
    fn decompose_expressions(&self, compiler: &mut AbstractCompiler) {
        for function_state in self.fns.values() {
            if function_state.can_inline() {
                for r#ref in function_state.get_references() {
                    if r#ref.requires_decomposition {
                        self.injector.maybe_prepare_call(compiler, &r#ref.base);
                    }
                }
            }
        }
    }

    /// Removed inlined functions that no longer have any references.
    // port: InlineFunctions#removeInlinedFunctions
    fn remove_inlined_functions(&self, compiler: &mut AbstractCompiler) {
        for (name, function_state) in &self.fns {
            if function_state.can_remove() {
                let r#fn = function_state.r#fn.as_ref();
                check_state!(function_state.can_inline());
                check_state!(r#fn.is_some());
                let r#fn = r#fn.unwrap();
                Self::verify_all_references_inlined(compiler, name, function_state);
                r#fn.remove(compiler);
                let fn_node = r#fn.get_function_node(compiler);
                NodeUtil::mark_functions_deleted(compiler, fn_node);
            }
        }
    }

    /// Check to verify that expression rewriting didn't make a call inaccessible.
    // port: InlineFunctions#verifyAllReferencesInlined
    fn verify_all_references_inlined(ast: &Ast, name: &JsString, function_state: &FunctionState) {
        for r#ref in function_state.get_references() {
            if !r#ref.inlined {
                let parent = r#ref.base.call_node.get_parent(ast);
                panic!(
                    "Call site missed ({}).\n call: {}\n parent:  {}",
                    name,
                    r#ref.base.call_node.to_string_tree(ast),
                    match parent {
                        None => "null".to_string(),
                        Some(parent) => parent.to_string_tree(ast),
                    }
                );
            }
        }
    }

    // port: InlineFunctions#checkOptimizationLevelSupportsRequireInlining
    pub fn check_optimization_level_supports_require_inlining(
        options: &CompilerOptions,
    ) -> ShouldRequireInlining {
        // Non-optimized code might not perform all required inlinings.
        let mut problems: Vec<String> = Vec::new();
        if !options.should_optimize() {
            problems.push("non-optimized code".into());
        }
        // Required inlinings don't make sense without inlining. If we don't inline
        // properties we might not be able to inline functions defined on namespaces.
        if !options.should_inline_properties() {
            problems.push("missing shouldInlineProperties".into());
        }
        if !options.should_inline_constant_vars() {
            problems.push("missing inlineConstantVars".into());
        }
        if !options.get_smart_name_removal() {
            problems.push("missing smartNameRemoval".into());
        }
        if !options.get_inline_functions_level().is_on() {
            problems.push("missing inlineFunctions".into());
        }
        if !options.get_inline_functions_level().includes_globals() {
            problems.push("inlineFunctions does not include globals".into());
        }
        // Coverage runs may break some inlining.
        if options.get_instrument_for_coverage_option() != InstrumentOption::NONE {
            problems.push("instrumentForCoverageOption enabled".into());
        }
        // We need to collapse properties or functions might be retained due to
        // being written on an exported namespace.
        if options.get_property_collapse_level() != PropertyCollapseLevel::ALL {
            problems.push("missing property collapsing".into());
        }
        // Avoid inlining failures due to asserts.
        if !options.should_remove_closure_asserts() {
            problems.push("missing removeClosureAsserts".into());
        }
        // Avoid inlining failures due to being unable to devirtualize a method.
        if !options.should_devirtualize_methods() {
            problems.push("missing devirtualizeMethods".into());
        }
        // We need more than one inlining pass for most required inlinings to work. This
        // eliminates 'fast' mode.
        if options.get_max_optimization_loop_iterations() == 1 {
            problems.push(format!(
                "optimizationLoopMaxIterations should be >= 2, but is: {}",
                options.get_max_optimization_loop_iterations()
            ));
        }
        if problems.is_empty() {
            return ShouldRequireInlining::new(true, String::new());
        }
        ShouldRequireInlining::new(false, format!("[{}]", problems.join(", ")))
    }
}

impl CompilerPass for InlineFunctions {
    // port: InlineFunctions#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        check_state!(compiler.get_life_cycle_stage().is_normalized());

        NodeTraversal::traverse(
            compiler,
            root,
            &mut FindCandidateFunctions {
                outer: self,
                calls_seen: 0,
            },
        );
        if self.fns.is_empty() {
            return; // Nothing left to do.
        }
        NodeTraversal::traverse(
            compiler,
            root,
            &mut FindCandidatesReferences::new(
                &mut self.fns,
                &self.anon_fns,
                &mut self.injector,
                self.enforce_max_size_after_inlining,
                self.max_size_after_inlining,
            ),
        );
        self.trim_candidates_not_meeting_minimum_requirements();
        if self.fns.is_empty() {
            return; // Nothing left to do.
        }

        // Store the set of function names eligible for inlining and use this to
        // prevent function names from being moved into temporaries during
        // expression decomposition. If this movement were allowed it would prevent
        // the Inline callback from finding the function calls.
        //
        // This pass already assumes these are constants, so this is safe for anyone
        // using function inlining.
        //
        let fn_names: IndexSet<JsString> = self.fns.keys().cloned().collect();
        self.injector.set_known_constant_functions(fn_names);

        self.trim_candidates_using_on_cost(compiler);
        if self.fns.is_empty() {
            return; // Nothing left to do.
        }
        self.resolve_inline_conflicts(compiler);
        self.decompose_expressions(compiler);
        NodeTraversal::traverse(
            compiler,
            root,
            &mut CallVisitor::new(&mut self.fns, &self.anon_fns, Inline::new(&self.injector)),
        );

        self.remove_inlined_functions(compiler);
    }
}

/// Find functions that might be inlined.
struct FindCandidateFunctions<'a> {
    outer: &'a mut InlineFunctions,
    calls_seen: i32,
}

impl Callback for FindCandidateFunctions<'_> {
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: InlineFunctions.FindCandidateFunctions#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if self.outer.reach.includes_globals() || !t.in_global_hoist_scope() {
            self.find_named_functions(t, n, parent);
            self.find_function_expressions(t, n);
        }
    }
}

impl FindCandidateFunctions<'_> {
    // port: InlineFunctions.FindCandidateFunctions#findNamedFunctions
    fn find_named_functions(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) {
        if !NodeUtil::is_statement(t, n) {
            // There aren't any interesting functions here.
            return;
        }

        match n.get_token(t) {
            Token::VAR | Token::LET | Token::CONST => {
                // Functions expressions in the form of:
                //   var fooFn = function(x) { return ... }
                check_state!(n.has_one_child(t), "%s", n.to_string(t));
                let name_node = n.get_first_child(t).unwrap();
                if name_node.is_name(t)
                    && name_node.has_children(t)
                    && name_node.get_first_child(t).unwrap().is_function(t)
                {
                    let chunk = t.get_chunk();
                    self.outer.maybe_add_function(
                        t.get_compiler(),
                        Function::Var(FunctionVar::new(n)),
                        chunk,
                    );
                }
                // Named functions
                // function Foo(x) { return ... }
            }
            Token::FUNCTION => {
                let parent = parent.unwrap();
                check_state!(NodeUtil::is_statement_block(t, parent) || parent.is_label(t));
                if NodeUtil::is_function_declaration(t, n) {
                    let r#fn = Function::Named(NamedFunction::new(n));
                    let chunk = t.get_chunk();
                    self.outer.maybe_add_function(t.get_compiler(), r#fn, chunk);
                }
            }
            _ => {}
        }
    }

    /// Find function expressions that are called directly in the form of
    /// (function(a,b,...){...})(a,b,...) or (function(a,b,...){...}).call(this,a,b, ...)
    // port: InlineFunctions.FindCandidateFunctions#findFunctionExpressions
    fn find_function_expressions(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        match n.get_token(t) {
            Token::OPTCHAIN_CALL | Token::CALL => {
                // Functions expressions in the form of:
                //   (function(){})();
                let mut fn_node = None;
                if n.get_first_child(t).unwrap().is_function(t) {
                    fn_node = n.get_first_child(t);
                } else if NodeUtil::is_function_object_call(t, n) {
                    let fn_identifying_node = n.get_first_first_child(t).unwrap();
                    if fn_identifying_node.is_function(t) {
                        fn_node = Some(fn_identifying_node);
                    }
                }

                // If an interesting function was discovered, add it.
                if let Some(fn_node) = fn_node {
                    let calls_seen = self.calls_seen;
                    self.calls_seen += 1;
                    let r#fn =
                        Function::Expression(FunctionExpression::new(t, fn_node, calls_seen));
                    // Rust-only: `fn` moves into maybeAddFunction; its name is read first.
                    let name = r#fn.get_name(t);
                    let chunk = t.get_chunk();
                    self.outer.maybe_add_function(t.get_compiler(), r#fn, chunk);
                    self.outer.anon_fns.insert(fn_node, name);
                }
            }
            _ => {}
        }
    }
}

/// See `CallVisitor`.
trait CallVisitorCallback {
    // port: InlineFunctions.CallVisitorCallback#visitCallSite
    fn visit_call_site(
        &mut self,
        t: &mut NodeTraversal<'_>,
        call_node: NodeId,
        function_state: &mut FunctionState,
    );
}

/// Visit call sites for functions in functionMap.
struct CallVisitor<'a, C> {
    callback: C,
    function_map: &'a mut IndexMap<JsString, FunctionState>,
    anon_function_map: &'a IndexMap<NodeId, JsString>,
}

impl<'a, C: CallVisitorCallback> CallVisitor<'a, C> {
    // port: InlineFunctions.CallVisitor#CallVisitor
    fn new(
        fns: &'a mut IndexMap<JsString, FunctionState>,
        anon_fns: &'a IndexMap<NodeId, JsString>,
        callback: C,
    ) -> Self {
        Self {
            function_map: fns,
            anon_function_map: anon_fns,
            callback,
        }
    }

    // port: InlineFunctions.CallVisitor#visit
    fn visit_calls(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::OPTCHAIN_CALL | Token::CALL => {
                // Function calls
                let child = n.get_first_child(t).unwrap();
                let mut name: Option<JsString> = None;
                // NOTE: The normalization pass ensures that local names do not collide with
                // global names.
                if child.is_name(t) {
                    name = Some(child.get_string(t));
                } else if child.is_function(t) {
                    name = self.anon_function_map.get(&child).cloned();
                } else if NodeUtil::is_function_object_call(t, n) {
                    check_state!(NodeUtil::is_normal_or_opt_chain_get(t, child));
                    let fn_identifying_node = child.get_first_child(t).unwrap();
                    if fn_identifying_node.is_name(t) {
                        name = Some(fn_identifying_node.get_string(t));
                    } else if fn_identifying_node.is_function(t) {
                        name = self.anon_function_map.get(&fn_identifying_node).cloned();
                    }
                }

                if let Some(name) = name {
                    let function_state = self.function_map.get_mut(&name);

                    // Only visit call-sites for functions that can be inlined.
                    if let Some(function_state) = function_state {
                        self.callback.visit_call_site(t, n, function_state);
                    }
                }
            }
            _ => {}
        }
    }
}

impl<C: CallVisitorCallback> Callback for CallVisitor<'_, C> {
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self.visit_calls(t, n, parent);
    }
}

/// Find references to functions that are inlinable.
///
/// Java's FindCandidatesReferences is a CallVisitor that is its own CallVisitorCallback; here the
/// callback side is `CandidateReferenceCollector`, which the CallVisitor owns.
struct FindCandidatesReferences<'a> {
    call_visitor: CallVisitor<'a, CandidateReferenceCollector<'a>>,
}

/// The CallVisitorCallback side of FindCandidatesReferences.
struct CandidateReferenceCollector<'a> {
    injector: &'a mut FunctionInjector,
    enforce_max_size_after_inlining: bool,
    max_size_after_inlining: i32,
}

impl<'a> FindCandidatesReferences<'a> {
    // port: InlineFunctions.FindCandidatesReferences#FindCandidatesReferences
    fn new(
        fns: &'a mut IndexMap<JsString, FunctionState>,
        anon_fns: &'a IndexMap<NodeId, JsString>,
        injector: &'a mut FunctionInjector,
        enforce_max_size_after_inlining: bool,
        max_size_after_inlining: i32,
    ) -> Self {
        Self {
            call_visitor: CallVisitor::new(
                fns,
                anon_fns,
                CandidateReferenceCollector {
                    injector,
                    enforce_max_size_after_inlining,
                    max_size_after_inlining,
                },
            ),
        }
    }

    /// Find functions that can be inlined.
    // port: InlineFunctions.FindCandidatesReferences#checkNameUsage
    fn check_name_usage(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: NodeId) {
        check_state!(n.is_name(t), "%s", n.to_string(t));

        if InlineFunctions::is_candidate_usage(t, n) {
            return;
        }

        // Other refs to a function name remove its candidacy for inlining
        let name = n.get_string(t);
        let function_state = self.call_visitor.function_map.get_mut(&name);
        let Some(function_state) = function_state else {
            return;
        };

        // If the name is being assigned to or modified as an L-value (e.g. `bar = something;`,
        // destructuring `[bar] = ...`, or `for (bar of ...)`), it cannot be inlined.
        if NodeUtil::is_l_value(t, n) {
            // Mark the function as uninlinable.
            function_state.disallow_inlining(t.get_compiler(), DisallowInliningReason::REASSIGNED);
        } else if (parent.is_assign(t)
            && parent.get_jsdoc_info_ref(t).is_some()
            && parent.get_jsdoc_info(t).unwrap().is_constant()
            && parent.get_second_child(t) == Some(n))
            || (parent.get_parent(t).is_some()
                && parent.get_parent(t).unwrap().is_const(t)
                && parent.get_first_child(t) == Some(n))
        {
            // e.g. const bar = foo; exports.bar = foo;
            // We can't see through this reference right now, but we will be able
            // to once `bar` is inlined; so we shouldn't remove `foo` right now but
            // this is probably okay with @requireInlining.
            function_state.set_remove(
                false,
                t.get_compiler(),
                CannotRemoveReason::UNREMOVABLE_REFERENCE,
                ShouldWarnWhenRequireInliningCannotInline::NO,
            );
        } else {
            // e.g. var fn = bar; <== we can't inline "bar"
            // As this reference can't be inlined mark the function as
            // unremovable.
            function_state.set_remove_with_context(
                false,
                t.get_compiler(),
                CannotRemoveReason::UNREMOVABLE_REFERENCE,
                ShouldWarnWhenRequireInliningCannotInline::YES,
                /* contextNode= */ Some(parent),
            );
        }
    }
}

impl Callback for FindCandidatesReferences<'_> {
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: InlineFunctions.FindCandidatesReferences#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self.call_visitor.visit_calls(t, n, parent);
        if n.is_name(t) {
            self.check_name_usage(t, n, parent.unwrap());
        }
    }
}

impl CallVisitorCallback for CandidateReferenceCollector<'_> {
    // port: InlineFunctions.FindCandidatesReferences#visitCallSite
    fn visit_call_site(
        &mut self,
        t: &mut NodeTraversal<'_>,
        call_node: NodeId,
        function_state: &mut FunctionState,
    ) {
        let chunk = t.get_chunk();
        self.maybe_add_reference(t, function_state, call_node, chunk);
    }
}

impl CandidateReferenceCollector<'_> {
    // port: InlineFunctions.FindCandidatesReferences#maybeAddReference
    fn maybe_add_reference(
        &mut self,
        t: &mut NodeTraversal<'_>,
        function_state: &mut FunctionState,
        call_node: NodeId,
        chunk: Option<JSChunk>,
    ) {
        if !function_state.can_inline() {
            return;
        }

        let mut mode = if function_state.can_inline_directly() {
            InliningMode::DIRECT
        } else {
            InliningMode::BLOCK
        };
        let mut reference_added =
            self.maybe_add_reference_using_mode(t, function_state, call_node, chunk.clone(), mode);
        if !reference_added && mode == InliningMode::DIRECT {
            // This reference can not be directly inlined, see if
            // block replacement inlining is possible.
            mode = InliningMode::BLOCK;
            reference_added =
                self.maybe_add_reference_using_mode(t, function_state, call_node, chunk, mode);
        }

        if !reference_added {
            // Don't try to remove a function if we can't inline all
            // the references.
            function_state.set_remove_with_context(
                false,
                t.get_compiler(),
                CannotRemoveReason::UNINLINABLE_REFERENCE,
                ShouldWarnWhenRequireInliningCannotInline::NO,
                /* contextNode= */ Some(call_node),
            );
        }
    }

    // port: InlineFunctions.FindCandidatesReferences#maybeAddReferenceUsingMode
    fn maybe_add_reference_using_mode(
        &mut self,
        t: &mut NodeTraversal<'_>,
        function_state: &mut FunctionState,
        call_node: NodeId,
        chunk: Option<JSChunk>,
        mode: InliningMode,
    ) -> bool {
        // If many functions are inlined into the same function F in the same
        // inlining round, then the size of F may exceed the max size.
        // This could be avoided if we bail later, during the inlining phase, eg,
        // in Inline#visitCallSite. However, that is not safe, because at that
        // point expression decomposition has already run, and we want to
        // decompose expressions only for the calls that are actually inlined.
        if self.enforce_max_size_after_inlining
            && InlineFunctions::target_size_after_inline_exceeds_limit(
                self.max_size_after_inlining,
                t,
                function_state,
            )
        {
            return false;
        }

        let scope = t.get_scope();
        let mut candidate = Reference::new(call_node, scope, chunk, mode);
        let fn_node = function_state.get_fn().get_function_node(t);
        let result = self.injector.can_inline_reference_to_function(
            t.get_compiler(),
            &candidate.base,
            fn_node,
            &function_state.get_names_to_alias(),
            function_state.get_references_this(),
            function_state.has_inner_functions(),
        );
        if result != CanInlineResult::NO {
            // Yeah!
            candidate.set_requires_decomposition(result == CanInlineResult::AFTER_PREPARATION);
            function_state.add_reference(candidate);
            return true;
        }

        false
    }
}

/// Inline functions at the call sites.
struct Inline<'a> {
    injector: &'a FunctionInjector,
}

impl<'a> Inline<'a> {
    // port: InlineFunctions.Inline#Inline
    fn new(injector: &'a FunctionInjector) -> Self {
        Self { injector }
    }

    /// Inline a function into the call site.
    // port: InlineFunctions.Inline#inlineFunction
    fn inline_function(
        &self,
        t: &mut NodeTraversal<'_>,
        r#ref: &Reference,
        function_state: &FunctionState,
    ) {
        let r#fn = function_state.get_fn();
        let fn_name = r#fn.get_name(t);
        let fn_node = function_state.get_safe_fn_node(t);

        let new_expr = self
            .injector
            .inline(t.get_compiler(), &r#ref.base, &fn_name, fn_node);
        if new_expr != r#ref.base.call_node {
            t.get_compiler().report_change_to_enclosing_scope(new_expr);
        }
    }
}

impl CallVisitorCallback for Inline<'_> {
    // port: InlineFunctions.Inline#visitCallSite
    fn visit_call_site(
        &mut self,
        t: &mut NodeTraversal<'_>,
        call_node: NodeId,
        function_state: &mut FunctionState,
    ) {
        check_state!(function_state.has_existing_function_definition());
        if function_state.can_inline() {
            let r#ref = function_state.get_reference(call_node).cloned();

            // There are two cases ref can be null: if the call site was introduced
            // because it was part of a function that was inlined during this pass
            // or if the call site was trimmed from the list of references because
            // the function couldn't be inlined at this location.
            if let Some(r#ref) = r#ref {
                self.inline_function(t, &r#ref, function_state);
                // Keep track of references that have been inlined so that
                // we can verify that none have been missed.
                function_state.get_reference_mut(call_node).unwrap().inlined = true;
            }
        }
    }
}

/// Use to track the decisions that have been made about a function.
struct FunctionState {
    r#fn: Option<Function>,
    safe_fn_node: Option<NodeId>,
    inline: bool,
    remove: bool,
    inline_directly: bool,
    references_this: bool,
    has_inner_functions: bool,
    references: Option<IndexMap<NodeId, Reference>>,
    chunk: Option<JSChunk>,
    names_to_alias: Option<IndexSet<JsString>>,

    has_require_inlining_annotation: bool,
    has_require_inlining_annotation_initialized: bool,

    has_encourage_inlining_annotation: bool,
    has_encourage_inlining_annotation_initialized: bool,
}

impl FunctionState {
    // Rust-only: Java's field initializers.
    fn new() -> Self {
        Self {
            r#fn: None,
            safe_fn_node: None,
            inline: true,
            remove: true,
            inline_directly: false,
            references_this: false,
            has_inner_functions: false,
            references: None,
            chunk: None,
            names_to_alias: None,
            has_require_inlining_annotation: false,
            has_require_inlining_annotation_initialized: false,
            has_encourage_inlining_annotation: false,
            has_encourage_inlining_annotation_initialized: false,
        }
    }

    // port: InlineFunctions.FunctionState#hasExistingFunctionDefinition
    fn has_existing_function_definition(&self) -> bool {
        self.r#fn.is_some()
    }

    // port: InlineFunctions.FunctionState#setReferencesThis
    fn set_references_this(&mut self, references_this: bool) {
        self.references_this = references_this;
    }

    // port: InlineFunctions.FunctionState#getReferencesThis
    fn get_references_this(&self) -> bool {
        self.references_this
    }

    // port: InlineFunctions.FunctionState#setHasInnerFunctions
    fn set_has_inner_functions(&mut self, has_inner_functions: bool) {
        self.has_inner_functions = has_inner_functions;
    }

    // port: InlineFunctions.FunctionState#hasInnerFunctions
    fn has_inner_functions(&self) -> bool {
        self.has_inner_functions
    }

    // port: InlineFunctions.FunctionState#removeBlockInliningReferences
    fn remove_block_inlining_references(&mut self) {
        // getReferencesInternal() is an empty immutable map when references is null.
        if let Some(references) = &mut self.references {
            references.retain(|_, r#ref| r#ref.base.mode != InliningMode::BLOCK);
        }
    }

    // port: InlineFunctions.FunctionState#hasBlockInliningReferences
    fn has_block_inlining_references(&self) -> bool {
        for r in self.get_references() {
            if r.base.mode == InliningMode::BLOCK {
                return true;
            }
        }
        false
    }

    // port: InlineFunctions.FunctionState#getFn
    fn get_fn(&self) -> &Function {
        self.r#fn.as_ref().unwrap()
    }

    // port: InlineFunctions.FunctionState#setFn
    fn set_fn(&mut self, r#fn: Function) {
        check_state!(self.r#fn.is_none());
        self.r#fn = Some(r#fn);
    }

    // port: InlineFunctions.FunctionState#getSafeFnNode
    fn get_safe_fn_node(&self, ast: &Ast) -> NodeId {
        match self.safe_fn_node {
            Some(safe_fn_node) => safe_fn_node,
            None => self.get_fn().get_function_node(ast),
        }
    }

    // port: InlineFunctions.FunctionState#setSafeFnNode
    fn set_safe_fn_node(&mut self, safe_fn_node: NodeId) {
        self.safe_fn_node = Some(safe_fn_node);
    }

    // port: InlineFunctions.FunctionState#canInline
    fn can_inline(&self) -> bool {
        self.inline
    }

    // port: InlineFunctions.FunctionState#encourageInlining
    fn encourage_inlining(&mut self, ast: &Ast) -> bool {
        let Some(r#fn) = &self.r#fn else {
            return false;
        };
        if !self.has_encourage_inlining_annotation_initialized {
            self.has_encourage_inlining_annotation_initialized = true;
            self.has_encourage_inlining_annotation =
                InlineFunctions::has_encourage_inlining_annotation(
                    ast,
                    r#fn.get_function_node(ast),
                );
        }
        self.has_encourage_inlining_annotation
    }

    // port: InlineFunctions.FunctionState#requireInlining
    fn require_inlining(&mut self, ast: &Ast) -> bool {
        let Some(r#fn) = &self.r#fn else {
            return false;
        };
        if !self.has_require_inlining_annotation_initialized {
            self.has_require_inlining_annotation_initialized = true;
            self.has_require_inlining_annotation =
                InlineFunctions::has_require_inlining_annotation(ast, r#fn.get_function_node(ast));
        }
        self.has_require_inlining_annotation
    }

    // port: InlineFunctions.FunctionState#disallowInlining(AbstractCompiler,DisallowInliningReason)
    fn disallow_inlining(
        &mut self,
        compiler: &mut AbstractCompiler,
        reason: DisallowInliningReason,
    ) {
        self.disallow_inlining_with_warning(
            compiler,
            reason,
            ShouldWarnWhenRequireInliningCannotInline::YES,
        );
    }

    // port: InlineFunctions.FunctionState#disallowInlining(AbstractCompiler,DisallowInliningReason,ShouldWarnWhenRequireInliningCannotInline)
    fn disallow_inlining_with_warning(
        &mut self,
        compiler: &mut AbstractCompiler,
        reason: DisallowInliningReason,
        should_warn_when_require_inlining_cannot_inline: ShouldWarnWhenRequireInliningCannotInline,
    ) {
        if self.require_inlining(compiler)
            && compiler.is_debug_logging_enabled()
            && should_warn_when_require_inlining_cannot_inline
                == ShouldWarnWhenRequireInliningCannotInline::YES
        {
            let fn_node = self.r#fn.as_ref().unwrap().get_function_node(compiler);
            let error = JSError::make(
                compiler,
                fn_node,
                &MISSED_REQUIRED_INLINING,
                &[reason.name()],
            );
            compiler.report(error);
        }
        self.inline = false;

        // No need to keep references to function that can't be inlined.
        self.references = None;
        // Don't remove functions that we aren't inlining.
        self.remove = false;
    }

    // port: InlineFunctions.FunctionState#canRemove
    fn can_remove(&self) -> bool {
        self.remove
    }

    // port: InlineFunctions.FunctionState#setRemove(boolean,AbstractCompiler,CannotRemoveReason,ShouldWarnWhenRequireInliningCannotInline)
    fn set_remove(
        &mut self,
        remove: bool,
        compiler: &mut AbstractCompiler,
        reason: CannotRemoveReason,
        should_warn_if_require_inlining: ShouldWarnWhenRequireInliningCannotInline,
    ) {
        self.set_remove_with_context(
            remove,
            compiler,
            reason,
            should_warn_if_require_inlining,
            /* contextNode= */ None,
        );
    }

    // port: InlineFunctions.FunctionState#setRemove(boolean,AbstractCompiler,CannotRemoveReason,ShouldWarnWhenRequireInliningCannotInline,Node)
    fn set_remove_with_context(
        &mut self,
        remove: bool,
        compiler: &mut AbstractCompiler,
        reason: CannotRemoveReason,
        should_warn_if_require_inlining: ShouldWarnWhenRequireInliningCannotInline,
        context_node: Option<NodeId>,
    ) {
        if !remove
            && (should_warn_if_require_inlining != ShouldWarnWhenRequireInliningCannotInline::NO)
            && self.require_inlining(compiler)
            && compiler.is_debug_logging_enabled()
        {
            let fn_node = self.r#fn.as_ref().unwrap().get_function_node(compiler);
            let context = match context_node {
                None => "unknown".to_string(),
                Some(context_node) => context_node.to_string(compiler),
            };
            let error = JSError::make(
                compiler,
                fn_node,
                &MISSED_REQUIRED_INLINING,
                &[reason.name(), &context],
            );
            compiler.report(error);
        }
        self.remove = remove;
    }

    // port: InlineFunctions.FunctionState#canInlineDirectly
    fn can_inline_directly(&self) -> bool {
        self.inline_directly
    }

    // port: InlineFunctions.FunctionState#inlineDirectly
    fn inline_directly(&mut self, direct_replacement: bool) {
        self.inline_directly = direct_replacement;
    }

    // port: InlineFunctions.FunctionState#hasReferences
    fn has_references(&self) -> bool {
        self.references
            .as_ref()
            .is_some_and(|references| !references.is_empty())
    }

    /// Java returns an empty immutable map when `references` is null; `None` stands for it.
    // port: InlineFunctions.FunctionState#getReferencesInternal
    fn get_references_internal(&self) -> Option<&IndexMap<NodeId, Reference>> {
        self.references.as_ref()
    }

    // port: InlineFunctions.FunctionState#addReference
    fn add_reference(&mut self, r#ref: Reference) {
        let references = self
            .references
            .get_or_insert_with(IndexMap::<_, _>::default);
        references.insert(r#ref.base.call_node, r#ref);
    }

    // port: InlineFunctions.FunctionState#getReferences
    fn get_references(&self) -> impl Iterator<Item = &Reference> {
        self.get_references_internal()
            .into_iter()
            .flat_map(|references| references.values())
    }

    // port: InlineFunctions.FunctionState#getReference
    fn get_reference(&self, n: NodeId) -> Option<&Reference> {
        self.get_references_internal()
            .and_then(|references| references.get(&n))
    }

    // Rust-only: Java mutates the Reference that getReference returns.
    fn get_reference_mut(&mut self, n: NodeId) -> Option<&mut Reference> {
        self.references
            .as_mut()
            .and_then(|references| references.get_mut(&n))
    }

    // port: InlineFunctions.FunctionState#getNamesToAlias
    fn get_names_to_alias(&self) -> IndexSet<JsString> {
        match &self.names_to_alias {
            None => IndexSet::<_>::default(),
            Some(names_to_alias) => names_to_alias.clone(),
        }
    }

    // port: InlineFunctions.FunctionState#setNamesToAlias
    fn set_names_to_alias(&mut self, names: IndexSet<JsString>) {
        self.names_to_alias = Some(names);
    }

    // port: InlineFunctions.FunctionState#setChunk
    fn set_chunk(&mut self, chunk: Option<JSChunk>) {
        self.chunk = chunk;
    }

    // port: InlineFunctions.FunctionState#getChunk
    fn get_chunk(&self) -> Option<&JSChunk> {
        self.chunk.as_ref()
    }
}

/// Interface for dealing with function declarations and function expressions equally
enum Function {
    Named(NamedFunction),
    Var(FunctionVar),
    Expression(FunctionExpression),
}

impl Function {
    /// Gets the name of the function
    // port: InlineFunctions.Function#getName
    fn get_name(&self, ast: &Ast) -> JsString {
        match self {
            Function::Named(f) => f.get_name(ast),
            Function::Var(f) => f.get_name(ast),
            Function::Expression(f) => f.get_name(),
        }
    }

    /// Gets the name node of the function
    // port: InlineFunctions.Function#getNameNode
    fn get_name_node(&self, ast: &Ast) -> NodeId {
        match self {
            Function::Named(f) => f.get_name_node(ast),
            Function::Var(f) => f.get_name_node(ast),
            Function::Expression(f) => f.get_name_node(),
        }
    }

    /// Gets the function node
    // port: InlineFunctions.Function#getFunctionNode
    fn get_function_node(&self, ast: &Ast) -> NodeId {
        match self {
            Function::Named(f) => f.get_function_node(),
            Function::Var(f) => f.get_function_node(ast),
            Function::Expression(f) => f.get_function_node(),
        }
    }

    /// Removes itself from the JavaScript
    // port: InlineFunctions.Function#remove
    fn remove(&self, compiler: &mut AbstractCompiler) {
        match self {
            Function::Named(f) => f.remove(compiler),
            Function::Var(f) => f.remove(compiler),
            Function::Expression(f) => f.remove(),
        }
    }

    // port: InlineFunctions.Function#getDeclaringBlock
    fn get_declaring_block(&self, ast: &Ast) -> Option<NodeId> {
        match self {
            Function::Named(f) => f.get_declaring_block(ast),
            Function::Var(f) => f.get_declaring_block(ast),
            Function::Expression(f) => f.get_declaring_block(),
        }
    }
}

/// NamedFunction implementation of the Function interface
struct NamedFunction {
    r#fn: NodeId,
}

impl NamedFunction {
    // port: InlineFunctions.NamedFunction#NamedFunction
    fn new(r#fn: NodeId) -> Self {
        Self { r#fn }
    }

    // port: InlineFunctions.NamedFunction#getName
    fn get_name(&self, ast: &Ast) -> JsString {
        self.r#fn.get_first_child(ast).unwrap().get_string(ast)
    }

    // port: InlineFunctions.NamedFunction#getNameNode
    fn get_name_node(&self, ast: &Ast) -> NodeId {
        self.r#fn.get_first_child(ast).unwrap()
    }

    // port: InlineFunctions.NamedFunction#getFunctionNode
    fn get_function_node(&self) -> NodeId {
        self.r#fn
    }

    // port: InlineFunctions.NamedFunction#remove
    fn remove(&self, compiler: &mut AbstractCompiler) {
        compiler.report_change_to_enclosing_scope(self.r#fn);
        let parent = self.r#fn.get_parent(compiler).unwrap();
        NodeUtil::remove_child(compiler, parent, self.r#fn);
        NodeUtil::mark_functions_deleted(compiler, self.r#fn);
    }

    // port: InlineFunctions.NamedFunction#getDeclaringBlock
    fn get_declaring_block(&self, ast: &Ast) -> Option<NodeId> {
        self.r#fn.get_parent(ast)
    }
}

/// FunctionVar implementation of the Function interface
struct FunctionVar {
    var: NodeId,
}

impl FunctionVar {
    // port: InlineFunctions.FunctionVar#FunctionVar
    fn new(var: NodeId) -> Self {
        Self { var }
    }

    // port: InlineFunctions.FunctionVar#getName
    fn get_name(&self, ast: &Ast) -> JsString {
        self.var.get_first_child(ast).unwrap().get_string(ast)
    }

    // port: InlineFunctions.FunctionVar#getNameNode
    fn get_name_node(&self, ast: &Ast) -> NodeId {
        self.var.get_first_child(ast).unwrap()
    }

    // port: InlineFunctions.FunctionVar#getFunctionNode
    fn get_function_node(&self, ast: &Ast) -> NodeId {
        self.var.get_first_first_child(ast).unwrap()
    }

    // port: InlineFunctions.FunctionVar#remove
    fn remove(&self, compiler: &mut AbstractCompiler) {
        compiler.report_change_to_enclosing_scope(self.var);
        let parent = self.var.get_parent(compiler).unwrap();
        NodeUtil::remove_child(compiler, parent, self.var);
        NodeUtil::mark_functions_deleted(compiler, self.var);
    }

    // port: InlineFunctions.FunctionVar#getDeclaringBlock
    fn get_declaring_block(&self, ast: &Ast) -> Option<NodeId> {
        self.var.get_parent(ast)
    }
}

/// FunctionExpression implementation of the Function interface
struct FunctionExpression {
    r#fn: NodeId,
    fake_name: JsString,
    fake_name_node: NodeId,
}

impl FunctionExpression {
    // port: InlineFunctions.FunctionExpression#FunctionExpression
    fn new(ast: &mut Ast, r#fn: NodeId, index: i32) -> Self {
        // A number is not a valid function JavaScript identifier
        // so we don't need to worry about collisions.
        let fake_name = JsString::from(index.to_string());
        let fake_name_node = IR::name(ast, fake_name.clone());
        Self {
            r#fn,
            fake_name,
            fake_name_node,
        }
    }

    // port: InlineFunctions.FunctionExpression#getName
    fn get_name(&self) -> JsString {
        self.fake_name.clone()
    }

    // port: InlineFunctions.FunctionExpression#getNameNode
    fn get_name_node(&self) -> NodeId {
        self.fake_name_node
    }

    // port: InlineFunctions.FunctionExpression#getFunctionNode
    fn get_function_node(&self) -> NodeId {
        self.r#fn
    }

    // port: InlineFunctions.FunctionExpression#remove
    fn remove(&self) {
        // Nothing to do. The function is removed with the call.
    }

    // port: InlineFunctions.FunctionExpression#getDeclaringBlock
    fn get_declaring_block(&self) -> Option<NodeId> {
        None
    }
}

/// Java's `InlineFunctions.Reference extends FunctionInjector.Reference`; `base` holds the
/// superclass part.
#[derive(Clone)]
pub struct Reference {
    pub base: function_injector::Reference,
    requires_decomposition: bool,
    inlined: bool,
}

impl Reference {
    // port: InlineFunctions.Reference#Reference
    fn new(call_node: NodeId, scope: Scope, chunk: Option<JSChunk>, mode: InliningMode) -> Self {
        Self {
            base: function_injector::Reference::new(call_node, scope, chunk, mode),
            requires_decomposition: false,
            inlined: false,
        }
    }

    // port: InlineFunctions.Reference#setRequiresDecomposition
    fn set_requires_decomposition(&mut self, new_val: bool) {
        self.requires_decomposition = new_val;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShouldWarnWhenRequireInliningCannotInline {
    YES,
    NO,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CannotRemoveReason {
    BLOCK_INLINING_REFERENCE,
    CALLED_BY_INLINED_FUNCTION,
    UNINLINABLE_REFERENCE,
    UNREMOVABLE_REFERENCE,
}

impl CannotRemoveReason {
    /// Java's `Enum#name()`.
    pub fn name(self) -> &'static str {
        match self {
            CannotRemoveReason::BLOCK_INLINING_REFERENCE => "BLOCK_INLINING_REFERENCE",
            CannotRemoveReason::CALLED_BY_INLINED_FUNCTION => "CALLED_BY_INLINED_FUNCTION",
            CannotRemoveReason::UNINLINABLE_REFERENCE => "UNINLINABLE_REFERENCE",
            CannotRemoveReason::UNREMOVABLE_REFERENCE => "UNREMOVABLE_REFERENCE",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisallowInliningReason {
    ASYNC_FUNCTION,
    CAPTURED_VARIABLE_NOT_IN_SCOPE,
    COST,
    GENERATOR_FUNCTION,
    HAS_LOCAL_NAMES,
    MULTIPLE_FUNCTION_DEFINITIONS,
    NON_INLINABLE_PARAM,
    NOT_CANDIDATE_FUNCTION,
    NO_INLINE_ANNOTATION,
    REASSIGNED,
    TARGET_SIZE_EXCEEDS_LIMIT,
}

impl DisallowInliningReason {
    /// Java's `Enum#name()`.
    pub fn name(self) -> &'static str {
        match self {
            DisallowInliningReason::ASYNC_FUNCTION => "ASYNC_FUNCTION",
            DisallowInliningReason::CAPTURED_VARIABLE_NOT_IN_SCOPE => {
                "CAPTURED_VARIABLE_NOT_IN_SCOPE"
            }
            DisallowInliningReason::COST => "COST",
            DisallowInliningReason::GENERATOR_FUNCTION => "GENERATOR_FUNCTION",
            DisallowInliningReason::HAS_LOCAL_NAMES => "HAS_LOCAL_NAMES",
            DisallowInliningReason::MULTIPLE_FUNCTION_DEFINITIONS => {
                "MULTIPLE_FUNCTION_DEFINITIONS"
            }
            DisallowInliningReason::NON_INLINABLE_PARAM => "NON_INLINABLE_PARAM",
            DisallowInliningReason::NOT_CANDIDATE_FUNCTION => "NOT_CANDIDATE_FUNCTION",
            DisallowInliningReason::NO_INLINE_ANNOTATION => "NO_INLINE_ANNOTATION",
            DisallowInliningReason::REASSIGNED => "REASSIGNED",
            DisallowInliningReason::TARGET_SIZE_EXCEEDS_LIMIT => "TARGET_SIZE_EXCEEDS_LIMIT",
        }
    }
}

/// Java: `record ShouldRequireInlining(boolean shouldValidate, String optMessage)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShouldRequireInlining {
    should_validate: bool,
    opt_message: String,
}

impl ShouldRequireInlining {
    // port: InlineFunctions.ShouldRequireInlining#ShouldRequireInlining
    pub fn new(should_validate: bool, opt_message: String) -> Self {
        Self {
            should_validate,
            opt_message,
        }
    }

    // port: InlineFunctions.ShouldRequireInlining#shouldValidate
    pub fn should_validate(&self) -> bool {
        self.should_validate
    }

    // port: InlineFunctions.ShouldRequireInlining#optMessage
    pub fn opt_message(&self) -> &str {
        &self.opt_message
    }
}

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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
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
    syntactic_scope_cache::{DeepFunctionStamp, SyntacticScopeCache},
    syntactic_scope_creator::{SyntacticScopeCreator, ensure_ancestor_scope},
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::LazyLock;
use std::{rc::Rc, sync::Arc};

/// `CLOSURE_RS_INLINE_FUNCTION_FACTS`: `off` computes the facts of every candidate function on
/// every run (`FunctionFactsCache`), `verify` computes them anew on every hit and compares.
static FUNCTION_FACTS_MODE: LazyLock<Option<bool>> =
    LazyLock::new(
        || match std::env::var("CLOSURE_RS_INLINE_FUNCTION_FACTS").as_deref() {
            Ok("off") => None,
            Ok("verify") => Some(true),
            _ => Some(false),
        },
    );

/// Rust-only (performance divergence): what `updateFunctionStateForInlining` computes from a
/// candidate function's subtree and name, kept between runs of the pass (each computed when the
/// pass first asks for it). These are functions of the subtree (nested functions included:
/// the size, the minimum requirements and the parameter uses look into them) and of the
/// function's name, so they hold while `DeepFunctionStamp` (with in-place JSDoc and side-effect
/// flags) is current and the name and the size limit are the same.
#[derive(Default)]
pub(crate) struct FunctionFactsCache {
    functions: IndexMap<NodeId, FunctionFacts>,
    /// Rust-only (performance divergence): the call sites, by call node, candidate function and
    /// whether the mode is DIRECT, that `maybeAddReferenceUsingMode` found it could not inline
    /// (`NoInlineSite`).
    no_inline_sites: IndexMap<(NodeId, NodeId, bool), NoInlineSite>,
}

/// Rust-only (performance divergence): a call site that `maybeAddReferenceUsingMode` could not
/// inline the candidate function into. Its answer is a function of the call site's surroundings
/// up to the function around it (the size limit counts that function, nested functions
/// included; the scope, loops, the statement and the arguments are inside it), or up to the
/// statement of the script it is in, and of the candidate's subtree and facts (`FunctionFacts`),
/// so it holds while both `DeepFunctionStamp`s are current. A statement of a script that moves
/// stays outside loops and functions.
struct NoInlineSite {
    site: Arc<DeepFunctionStamp>,
    target: Arc<DeepFunctionStamp>,
}

#[derive(Clone, PartialEq)]
struct BodyFacts {
    modified_parameters: IndexSet<JsString>,
    references_receiver: bool,
    has_inner_closures: bool,
}

struct FunctionFacts {
    stamp: DeepFunctionStamp,
    name: JsString,
    max_size_after_inlining: i32,
    size_reaches_limit: Option<bool>,
    direct_replacement: Option<bool>,
    non_inlinable_param: Option<bool>,
    meets_minimum_requirements: Option<bool>,
    body: Option<BodyFacts>,
    local_names: Option<bool>,
}

impl FunctionFacts {
    /// The kept facts of `fn_node` if still current, else empty ones.
    fn take(
        compiler: &mut AbstractCompiler,
        fn_node: NodeId,
        name: &JsString,
        max_size_after_inlining: i32,
    ) -> Option<Self> {
        (*FUNCTION_FACTS_MODE)?;
        let kept = compiler
            .inline_functions_facts
            .functions
            .swap_remove(&fn_node);
        let own = SyntacticScopeCache::subtree_stamp_with_flags(compiler, fn_node);
        if let Some(kept) = kept
            && kept.name == *name
            && kept.max_size_after_inlining == max_size_after_inlining
            && compiler
                .syntactic_scope_cache
                .deep_function_stamp_is_current(own, &kept.stamp)
        {
            return Some(kept);
        }
        Some(Self {
            stamp: SyntacticScopeCache::deep_function_stamp(compiler, fn_node),
            name: name.clone(),
            max_size_after_inlining,
            size_reaches_limit: None,
            direct_replacement: None,
            non_inlinable_param: None,
            meets_minimum_requirements: None,
            body: None,
            local_names: None,
        })
    }

    fn keep(self, compiler: &mut AbstractCompiler, fn_node: NodeId) {
        compiler
            .inline_functions_facts
            .functions
            .insert(fn_node, self);
    }

    /// The kept fact, or `compute`d and kept; with `CLOSURE_RS_INLINE_FUNCTION_FACTS=verify` a
    /// kept fact is computed again and compared.
    fn fact<T: Clone + PartialEq>(
        slot: &mut Option<T>,
        what: &str,
        compute: impl FnOnce() -> T,
    ) -> T {
        match slot {
            Some(kept) => {
                if *FUNCTION_FACTS_MODE == Some(true) {
                    assert!(
                        compute() == *kept,
                        "InlineFunctions: the kept {what} of a function differs from a new computation"
                    );
                }
                kept.clone()
            }
            None => slot.insert(compute()).clone(),
        }
    }

    fn size_reaches_limit(&mut self, ast: &mut Ast, fn_node: NodeId) -> bool {
        let max = self.max_size_after_inlining;
        Self::fact(&mut self.size_reaches_limit, "size", || {
            max <= NodeUtil::count_ast_size_up_to_limit(ast, fn_node, max)
        })
    }

    fn direct_replacement(&mut self, ast: &Ast, fn_node: NodeId) -> bool {
        Self::fact(&mut self.direct_replacement, "direct replacement", || {
            FunctionInjector::is_direct_call_node_replacement_possible(ast, fn_node)
        })
    }

    fn non_inlinable_param(&mut self, ast: &Ast, fn_node: NodeId) -> bool {
        Self::fact(&mut self.non_inlinable_param, "parameters", || {
            InlineFunctions::has_non_inlinable_param(
                ast,
                NodeUtil::get_function_parameters(ast, fn_node),
            )
        })
    }

    fn meets_minimum_requirements(
        &mut self,
        ast: &Ast,
        injector: &FunctionInjector,
        fn_node: NodeId,
    ) -> bool {
        let name = &self.name;
        Self::fact(
            &mut self.meets_minimum_requirements,
            "minimum requirements",
            || injector.does_function_meet_minimum_requirements(ast, name, fn_node),
        )
    }

    fn body(
        &mut self,
        ast: &Ast,
        injector: &FunctionArgumentInjector,
        fn_node: NodeId,
    ) -> BodyFacts {
        Self::fact(&mut self.body, "parameter uses", || {
            let facts = injector.inspect_function::<true>(ast, fn_node);
            BodyFacts {
                modified_parameters: facts.modified_parameters,
                references_receiver: facts.references_receiver,
                has_inner_closures: facts.has_inner_closures,
            }
        })
    }

    fn local_names(&mut self, ast: &Ast, fn_node: NodeId) -> bool {
        Self::fact(&mut self.local_names, "local names", || {
            InlineFunctions::has_local_names(ast, fn_node)
        })
    }
}

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
            .function_argument_injector(function_argument_injector.clone())
            .build(compiler);
        Self {
            fns: IndexMap::<_, _>::default(),
            anon_fns: IndexMap::<_, _>::default(),
            injector,
            function_argument_injector,
            reach,
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
        ast: &mut Ast,
        containing_function: Option<NodeId>,
        function_state: &FunctionState,
    ) -> bool {
        // Always inline at the top level,
        // unless maybeAddFunction has marked functionState as not inlinable.
        let Some(containing_function) = containing_function else {
            return false;
        };
        let inlined_fun = function_state.get_fn().get_function_node(ast);
        if Self::is_always_inlinable(ast, inlined_fun) {
            return false;
        }

        let inlined_fun_body = NodeUtil::get_function_body(ast, inlined_fun);
        let inlined_fun_size =
            NodeUtil::count_ast_size_up_to_limit(ast, inlined_fun_body, max_size_after_inlining);
        let target_fun_size =
            NodeUtil::count_ast_size_up_to_limit(ast, containing_function, max_size_after_inlining);
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
    /// Java passes the FunctionState of `name`; here it is looked up in `fns`. Rust-only: takes
    /// the kept facts of the function (`FunctionFactsCache`) around the Java method's body.
    fn update_function_state_for_inlining(
        &mut self,
        compiler: &mut AbstractCompiler,
        r#fn: Function,
        chunk: Option<JSChunk>,
        name: &JsString,
    ) {
        // Rust-only (performance divergence): see `FunctionFactsCache`.
        let fn_node = r#fn.get_function_node(compiler);
        let mut facts = FunctionFacts::take(compiler, fn_node, name, self.max_size_after_inlining);
        self.update_function_state_with_facts(compiler, r#fn, chunk, name, &mut facts);
        if let Some(facts) = facts {
            facts.keep(compiler, fn_node);
        }
    }

    // port: InlineFunctions#updateFunctionStateForInlining
    fn update_function_state_with_facts(
        &mut self,
        compiler: &mut AbstractCompiler,
        r#fn: Function,
        chunk: Option<JSChunk>,
        name: &JsString,
        facts: &mut Option<FunctionFacts>,
    ) {
        // TODO(johnlenz): Maybe "smarten" FunctionState by adding this logic to it?
        let Self {
            fns,
            injector,
            function_argument_injector,
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
            && match facts {
                Some(facts) => facts.size_reaches_limit(compiler, fn_node),
                None => {
                    *max_size_after_inlining
                        <= NodeUtil::count_ast_size_up_to_limit(
                            compiler,
                            fn_node,
                            *max_size_after_inlining,
                        )
                }
            }
        {
            function_state
                .disallow_inlining(compiler, DisallowInliningReason::TARGET_SIZE_EXCEEDS_LIMIT);
            return;
        }

        // verify the function hasn't already been marked as "don't inline"
        if function_state.can_inline() {
            // store it for use when inlining.
            function_state.set_fn(r#fn);
            if match facts {
                Some(facts) => facts.direct_replacement(compiler, fn_node),
                None => FunctionInjector::is_direct_call_node_replacement_possible(
                    compiler,
                    function_state.get_fn().get_function_node(compiler),
                ),
            } {
                function_state.inline_directly(true);
            }

            if match facts {
                Some(facts) => facts.non_inlinable_param(compiler, fn_node),
                None => Self::has_non_inlinable_param(
                    compiler,
                    NodeUtil::get_function_parameters(compiler, fn_node),
                ),
            } {
                function_state
                    .disallow_inlining(compiler, DisallowInliningReason::NON_INLINABLE_PARAM);
            }

            // verify the function meets all the requirements.
            // TODO(johnlenz): Minimum requirement checks are about 5% of the
            // run-time cost of this pass.
            if !Self::is_candidate_function(compiler, injector, function_state.get_fn(), facts) {
                // It doesn't meet the requirements.
                function_state
                    .disallow_inlining(compiler, DisallowInliningReason::NOT_CANDIDATE_FUNCTION);
            }

            // Set the chunk and gather names that need temporaries.
            if function_state.can_inline() {
                function_state.set_chunk(chunk);

                let body = match facts {
                    Some(facts) => facts.body(compiler, function_argument_injector, fn_node),
                    None => {
                        let body =
                            function_argument_injector.inspect_function::<true>(compiler, fn_node);
                        BodyFacts {
                            modified_parameters: body.modified_parameters,
                            references_receiver: body.references_receiver,
                            has_inner_closures: body.has_inner_closures,
                        }
                    }
                };
                if !body.modified_parameters.is_empty() {
                    function_state.inline_directly(false);
                    function_state.set_names_to_alias(body.modified_parameters);
                }
                if body.references_receiver {
                    function_state.set_references_this(true);
                }
                if body.has_inner_closures {
                    function_state.set_has_inner_closures(true);
                    // If there are inner functions, we can inline into global scope
                    // if there are no local vars or named functions.
                    // TODO(johnlenz): this can be improved by looking at the possible
                    // values for locals.  If there are simple values, or constants
                    // we could still inline.
                    if match facts {
                        Some(facts) => facts.local_names(compiler, fn_node),
                        None => Self::has_local_names(compiler, fn_node),
                    } {
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
        facts: &mut Option<FunctionFacts>,
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
        match facts {
            // The facts are kept for this name.
            Some(facts) => facts.meets_minimum_requirements(compiler, injector, fn_node),
            None => injector.does_function_meet_minimum_requirements(compiler, &fn_name, fn_node),
        }
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
                if Self::would_duplicate_tagged_template_lit(function_state) {
                    function_state.disallow_inlining(
                        compiler,
                        DisallowInliningReason::TAGGED_TEMPLATE_LITERAL,
                    );
                    return false;
                }
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

    /// Returns whether inlining `function_state` would leave more than one copy of a tagged
    /// template literal in the AST, breaking ECMA-262 `GetTemplateObject` call-site identity.
    // port: InlineFunctions#wouldDuplicateTaggedTemplateLit
    fn would_duplicate_tagged_template_lit(function_state: &FunctionState) -> bool {
        let ref_count = function_state.get_references().count();
        (ref_count > 1 || (ref_count == 1 && !function_state.can_remove()))
            && function_state.has_tagged_template_lit()
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
                    if Self::would_duplicate_tagged_template_lit(fs_called) {
                        fs_called.disallow_inlining_with_warning(
                            compiler,
                            DisallowInliningReason::TAGGED_TEMPLATE_LITERAL,
                            ShouldWarnWhenRequireInliningCannotInline::NO,
                        );
                    } else if !Self::minimize_cost(compiler, &self.injector, fs_called)
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

        // Rust-only (performance divergence): direct AST walks for Walk 1 (FindCandidateFunctions)
        // and Walk 2 (FindCandidatesReferences), fanning out read-only scans across scoped worker
        // threads on large inputs and replaying matched events in exact post-order on the main
        // thread while materializing scopes lazily via `ensure_ancestor_scope`.
        let parallel_units = if compiler.ast.node_count() >= 20_000 {
            collect_inline_fn_units(compiler, root)
        } else {
            Vec::new()
        };
        let threads = crate::parallel_parse::parallel_thread_count(parallel_units.len(), 2, 8);
        if threads >= 2 {
            let reach_includes_globals = self.reach.includes_globals();
            let ast = &compiler.ast;
            let worker_batches = crate::parallel_parse::run_scoped_indexed(
                "closure-inlinefn1",
                threads,
                &parallel_units,
                |_idx, unit| {
                    let mut events = Vec::new();
                    let ok = catch_unwind(AssertUnwindSafe(|| {
                        scan_candidate_functions_unit(
                            ast,
                            unit,
                            reach_includes_globals,
                            &mut events,
                        );
                    }))
                    .is_ok();
                    if !ok {
                        Some(Err(()))
                    } else if !events.is_empty() {
                        Some(Ok(events))
                    } else {
                        None
                    }
                },
            );
            let mut ordered: Vec<Option<Result<Vec<CandidateFnEvent>, ()>>> =
                (0..parallel_units.len()).map(|_| None).collect();
            for batch in worker_batches {
                for (idx, item) in batch {
                    ordered[idx] = Some(item);
                }
            }
            let mut finder = FindCandidateFunctions::new(self, compiler, root);
            let res = catch_unwind(AssertUnwindSafe(|| {
                for (idx, item) in ordered.into_iter().enumerate() {
                    match item {
                        None => {}
                        Some(Err(())) => {
                            let unit = &parallel_units[idx];
                            finder.current_script = unit.script;
                            finder.current_chunk =
                                unit.script.and_then(|s| chunk_of_script(compiler, s));
                            for &node in &unit.nodes {
                                finder.walk_node(compiler, node, unit.parent, true);
                            }
                        }
                        Some(Ok(events)) => {
                            for event in events {
                                finder.replay_event(compiler, event);
                            }
                        }
                    }
                }
            }));
            if let Err(payload) = res {
                NodeTraversal::throw_unexpected_exception_at(
                    compiler,
                    finder.current_script,
                    finder.current_node,
                    payload,
                );
            }
        } else {
            let mut finder = FindCandidateFunctions::new(self, compiler, root);
            let res = catch_unwind(AssertUnwindSafe(|| {
                finder.walk_node(compiler, root, None, true);
            }));
            if let Err(payload) = res {
                NodeTraversal::throw_unexpected_exception_at(
                    compiler,
                    finder.current_script,
                    finder.current_node,
                    payload,
                );
            }
        }
        if self.fns.is_empty() {
            return; // Nothing left to do.
        }
        if threads >= 2 {
            let name_bloom = CallVisitor::<Inline<'_>>::build_name_bloom(&self.fns);
            let candidate_names: IndexSet<JsString> = self.fns.keys().cloned().collect();
            let anon_fns_ref = &self.anon_fns;
            let candidate_names_ref = &candidate_names;
            let ast = &compiler.ast;
            let worker_batches = crate::parallel_parse::run_scoped_indexed(
                "closure-inlinefn2",
                threads,
                &parallel_units,
                |_idx, unit| {
                    let mut events = Vec::new();
                    let ok = catch_unwind(AssertUnwindSafe(|| {
                        scan_candidate_refs_unit(
                            ast,
                            root,
                            unit,
                            name_bloom,
                            candidate_names_ref,
                            anon_fns_ref,
                            &mut events,
                        );
                    }))
                    .is_ok();
                    if !ok {
                        Some(Err(()))
                    } else if !events.is_empty() {
                        Some(Ok(events))
                    } else {
                        None
                    }
                },
            );
            let mut ordered: Vec<Option<Result<Vec<CandidateRefEvent>, ()>>> =
                (0..parallel_units.len()).map(|_| None).collect();
            for batch in worker_batches {
                for (idx, item) in batch {
                    ordered[idx] = Some(item);
                }
            }
            let mut ref_finder = FindCandidatesReferences::new(
                compiler,
                root,
                &mut self.fns,
                &self.anon_fns,
                &mut self.injector,
                self.enforce_max_size_after_inlining,
                self.max_size_after_inlining,
            );
            let res = catch_unwind(AssertUnwindSafe(|| {
                for (idx, item) in ordered.into_iter().enumerate() {
                    match item {
                        None => {}
                        Some(Err(())) => {
                            let unit = &parallel_units[idx];
                            ref_finder.current_script = unit.script;
                            ref_finder.current_chunk =
                                unit.script.and_then(|s| chunk_of_script(compiler, s));
                            for &node in &unit.nodes {
                                ref_finder.walk_node(compiler, node, unit.parent, None, root);
                            }
                        }
                        Some(Ok(events)) => {
                            for event in events {
                                ref_finder.replay_event(compiler, event);
                            }
                        }
                    }
                }
            }));
            if let Err(payload) = res {
                NodeTraversal::throw_unexpected_exception_at(
                    compiler,
                    ref_finder.current_script,
                    ref_finder.current_node,
                    payload,
                );
            }
        } else {
            let mut ref_finder = FindCandidatesReferences::new(
                compiler,
                root,
                &mut self.fns,
                &self.anon_fns,
                &mut self.injector,
                self.enforce_max_size_after_inlining,
                self.max_size_after_inlining,
            );
            let res = catch_unwind(AssertUnwindSafe(|| {
                ref_finder.walk_node(compiler, root, None, None, root);
            }));
            if let Err(payload) = res {
                NodeTraversal::throw_unexpected_exception_at(
                    compiler,
                    ref_finder.current_script,
                    ref_finder.current_node,
                    payload,
                );
            }
        }
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
        // Rust-only (performance divergence): walk only the ancestor paths of surviving inlinable
        // call sites instead of the entire AST in Walk 3.
        let mut allowed_nodes = IndexSet::<NodeId>::default();
        for function_state in self.fns.values() {
            if function_state.can_inline() {
                for r#ref in function_state.get_references() {
                    let mut cur = Some(r#ref.base.call_node);
                    while let Some(node) = cur {
                        if !allowed_nodes.insert(node) {
                            break;
                        }
                        if node == root {
                            break;
                        }
                        cur = node.get_parent(compiler);
                    }
                }
            }
        }
        if !allowed_nodes.is_empty() {
            NodeTraversal::traverse(
                compiler,
                root,
                &mut CallVisitor::new_pruned(
                    &mut self.fns,
                    &self.anon_fns,
                    Inline::new(&self.injector),
                    allowed_nodes,
                ),
            );
        }

        self.remove_inlined_functions(compiler);
    }
}

fn chunk_of_script(compiler: &AbstractCompiler, script: NodeId) -> Option<JSChunk> {
    script
        .get_input_id(compiler)
        .and_then(|id| compiler.get_input(&id))
        .and_then(|input| input.get_chunk())
}

/// Rust-only (performance divergence): read-only work unit for parallel `InlineFunctions`
/// Walk 1 and Walk 2 scans.
struct InlineFnUnit {
    script: Option<NodeId>,
    parent: Option<NodeId>,
    nodes: Vec<NodeId>,
}

fn collect_inline_fn_units(compiler: &AbstractCompiler, root: NodeId) -> Vec<InlineFnUnit> {
    let ast = &compiler.ast;
    if !root.is_root(ast) {
        return Vec::new();
    }
    let mut scripts = Vec::new();
    let mut c = root.get_first_child(ast);
    while let Some(s) = c {
        scripts.push(s);
        c = s.get_next(ast);
    }
    // A large non-module script is split into runs of its top-level statements, so that one big
    // file among several (the chunks a bundler writes) is scanned on several threads too.
    const STMT_CHUNK: usize = 64;
    let mut units = Vec::with_capacity(scripts.len());
    let mut stmts = Vec::new();
    for script in scripts {
        if script.is_script(ast)
            && let Some(first) = script.get_first_child(ast)
            && !first.is_module_body(ast)
        {
            stmts.clear();
            let mut st = Some(first);
            while let Some(s) = st {
                stmts.push(s);
                st = s.get_next(ast);
            }
            if stmts.len() > STMT_CHUNK {
                units.extend(stmts.chunks(STMT_CHUNK).map(|chunk| InlineFnUnit {
                    script: Some(script),
                    parent: Some(script),
                    nodes: chunk.to_vec(),
                }));
                continue;
            }
        }
        units.push(InlineFnUnit {
            script: script.is_script(ast).then_some(script),
            parent: Some(root),
            nodes: vec![script],
        });
    }
    units
}

enum CandidateFnEvent {
    Named {
        node: NodeId,
        parent: Option<NodeId>,
        script: Option<NodeId>,
    },
    Expr {
        call_node: NodeId,
        script: Option<NodeId>,
    },
}

fn scan_candidate_functions_unit(
    ast: &Ast,
    unit: &InlineFnUnit,
    reach_includes_globals: bool,
    events: &mut Vec<CandidateFnEvent>,
) {
    fn walk(
        ast: &Ast,
        n: NodeId,
        parent: Option<NodeId>,
        in_global_hoist_scope: bool,
        reach_includes_globals: bool,
        current_script: Option<NodeId>,
        events: &mut Vec<CandidateFnEvent>,
    ) {
        let token = n.get_token(ast);
        match token {
            Token::SCRIPT => {
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    walk(
                        ast,
                        c,
                        Some(n),
                        in_global_hoist_scope,
                        reach_includes_globals,
                        Some(n),
                        events,
                    );
                    child = c.get_next(ast);
                }
                return;
            }
            Token::FUNCTION => {
                let fn_name = n.get_first_child(ast).unwrap();
                let args = fn_name.get_next(ast).unwrap();
                let body = args.get_next(ast).unwrap();
                let is_decl = parent.is_some() && NodeUtil::is_function_declaration(ast, n);
                walk(
                    ast,
                    fn_name,
                    Some(n),
                    if is_decl {
                        in_global_hoist_scope
                    } else {
                        false
                    },
                    reach_includes_globals,
                    current_script,
                    events,
                );
                walk(
                    ast,
                    args,
                    Some(n),
                    false,
                    reach_includes_globals,
                    current_script,
                    events,
                );
                walk(
                    ast,
                    body,
                    Some(n),
                    false,
                    reach_includes_globals,
                    current_script,
                    events,
                );
            }
            Token::MODULE_BODY => {
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    walk(
                        ast,
                        c,
                        Some(n),
                        false,
                        reach_includes_globals,
                        current_script,
                        events,
                    );
                    child = c.get_next(ast);
                }
                return;
            }
            Token::CLASS => {
                let class_name = n.get_first_child(ast).unwrap();
                let extends_clause = class_name.get_next(ast).unwrap();
                let body = extends_clause.get_next(ast).unwrap();
                walk(
                    ast,
                    extends_clause,
                    Some(n),
                    in_global_hoist_scope,
                    reach_includes_globals,
                    current_script,
                    events,
                );
                walk(
                    ast,
                    class_name,
                    Some(n),
                    in_global_hoist_scope,
                    reach_includes_globals,
                    current_script,
                    events,
                );
                walk(
                    ast,
                    body,
                    Some(n),
                    in_global_hoist_scope,
                    reach_includes_globals,
                    current_script,
                    events,
                );
                return;
            }
            Token::CLASS_MEMBERS => {
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    if c.is_computed_prop(ast) || c.is_computed_field_def(ast) {
                        walk(
                            ast,
                            c.get_first_child(ast).unwrap(),
                            Some(c),
                            in_global_hoist_scope,
                            reach_includes_globals,
                            current_script,
                            events,
                        );
                    }
                    child = c.get_next(ast);
                }
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    match c.get_token(ast) {
                        Token::COMPUTED_PROP => {
                            walk(
                                ast,
                                c.get_last_child(ast).unwrap(),
                                Some(c),
                                in_global_hoist_scope,
                                reach_includes_globals,
                                current_script,
                                events,
                            );
                        }
                        Token::COMPUTED_FIELD_DEF => {
                            if c.has_two_children(ast) {
                                walk(
                                    ast,
                                    c.get_last_child(ast).unwrap(),
                                    Some(c),
                                    false,
                                    reach_includes_globals,
                                    current_script,
                                    events,
                                );
                            }
                        }
                        Token::MEMBER_FIELD_DEF => {
                            walk(
                                ast,
                                c,
                                Some(n),
                                false,
                                reach_includes_globals,
                                current_script,
                                events,
                            );
                        }
                        _ => walk(
                            ast,
                            c,
                            Some(n),
                            in_global_hoist_scope,
                            reach_includes_globals,
                            current_script,
                            events,
                        ),
                    }
                    child = c.get_next(ast);
                }
                return;
            }
            Token::BLOCK => {
                let child_global =
                    in_global_hoist_scope && !NodeUtil::is_class_static_block(ast, n);
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    walk(
                        ast,
                        c,
                        Some(n),
                        child_global,
                        reach_includes_globals,
                        current_script,
                        events,
                    );
                    child = c.get_next(ast);
                }
                return;
            }
            _ => {
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    walk(
                        ast,
                        c,
                        Some(n),
                        in_global_hoist_scope,
                        reach_includes_globals,
                        current_script,
                        events,
                    );
                    child = c.get_next(ast);
                }
            }
        }
        if !reach_includes_globals && in_global_hoist_scope {
            return;
        }
        match token {
            Token::VAR | Token::LET | Token::CONST if NodeUtil::is_statement(ast, n) => {
                if !n.has_one_child(ast) {
                    events.push(CandidateFnEvent::Named {
                        node: n,
                        parent,
                        script: current_script,
                    });
                } else if let Some(name_node) = n.get_first_child(ast)
                    && name_node.is_name(ast)
                    && name_node.has_children(ast)
                    && name_node.get_first_child(ast).unwrap().is_function(ast)
                {
                    events.push(CandidateFnEvent::Named {
                        node: n,
                        parent,
                        script: current_script,
                    });
                }
            }
            Token::FUNCTION if NodeUtil::is_statement(ast, n) => {
                events.push(CandidateFnEvent::Named {
                    node: n,
                    parent,
                    script: current_script,
                });
            }
            Token::OPTCHAIN_CALL | Token::CALL => {
                let first = n.get_first_child(ast).unwrap();
                let is_cand = first.is_function(ast)
                    || (NodeUtil::is_function_object_call(ast, n)
                        && n.get_first_first_child(ast).unwrap().is_function(ast));
                if is_cand {
                    events.push(CandidateFnEvent::Expr {
                        call_node: n,
                        script: current_script,
                    });
                }
            }
            _ => {}
        }
    }

    for &node in &unit.nodes {
        walk(
            ast,
            node,
            unit.parent,
            true,
            reach_includes_globals,
            unit.script,
            events,
        );
    }
}

enum CandidateRefEvent {
    Call {
        call_node: NodeId,
        enclosing_fn: Option<NodeId>,
        scope_root: NodeId,
        script: Option<NodeId>,
    },
    Name {
        node: NodeId,
        parent: NodeId,
        script: Option<NodeId>,
    },
    TaggedTemplateFn {
        fn_node: NodeId,
    },
}

fn scan_candidate_refs_unit(
    ast: &Ast,
    root: NodeId,
    unit: &InlineFnUnit,
    name_bloom: [u64; 64],
    candidate_names: &IndexSet<JsString>,
    anon_fns: &IndexMap<NodeId, JsString>,
    events: &mut Vec<CandidateRefEvent>,
) {
    struct Scanner<'a> {
        ast: &'a Ast,
        name_bloom: [u64; 64],
        candidate_names: &'a IndexSet<JsString>,
        anon_fns: &'a IndexMap<NodeId, JsString>,
        tagged_fns: IndexSet<NodeId>,
        events: &'a mut Vec<CandidateRefEvent>,
    }
    impl Scanner<'_> {
        #[inline]
        fn may_contain_name(&self, name: &JsString) -> bool {
            let h = name.hash_code() as usize;
            (self.name_bloom[(h >> 6) & 63] & (1u64 << (h & 63))) != 0
        }

        fn walk(
            &mut self,
            n: NodeId,
            parent: Option<NodeId>,
            enclosing_fn: Option<NodeId>,
            scope_root: NodeId,
            current_script: Option<NodeId>,
        ) {
            let ast = self.ast;
            let token = n.get_token(ast);
            match token {
                Token::SCRIPT => {
                    let mut child = n.get_first_child(ast);
                    while let Some(c) = child {
                        self.walk(c, Some(n), enclosing_fn, scope_root, Some(n));
                        child = c.get_next(ast);
                    }
                    return;
                }
                Token::FUNCTION => {
                    let fn_name = n.get_first_child(ast).unwrap();
                    let args = fn_name.get_next(ast).unwrap();
                    let body = args.get_next(ast).unwrap();
                    let is_decl = parent.is_some() && NodeUtil::is_function_declaration(ast, n);
                    if is_decl {
                        self.walk(fn_name, Some(n), enclosing_fn, scope_root, current_script);
                    } else {
                        self.walk(fn_name, Some(n), Some(n), n, current_script);
                    }
                    self.walk(args, Some(n), Some(n), n, current_script);
                    self.walk(body, Some(n), Some(n), n, current_script);
                }
                Token::MODULE_BODY => {
                    let mut child = n.get_first_child(ast);
                    while let Some(c) = child {
                        self.walk(c, Some(n), enclosing_fn, n, current_script);
                        child = c.get_next(ast);
                    }
                    return;
                }
                Token::CLASS => {
                    let class_name = n.get_first_child(ast).unwrap();
                    let extends_clause = class_name.get_next(ast).unwrap();
                    let body = extends_clause.get_next(ast).unwrap();
                    let is_expr = NodeUtil::is_class_expression(ast, n);
                    self.walk(
                        extends_clause,
                        Some(n),
                        enclosing_fn,
                        scope_root,
                        current_script,
                    );
                    self.walk(
                        class_name,
                        Some(n),
                        enclosing_fn,
                        if is_expr { n } else { scope_root },
                        current_script,
                    );
                    self.walk(body, Some(n), enclosing_fn, n, current_script);
                    return;
                }
                Token::CLASS_MEMBERS => {
                    let mut child = n.get_first_child(ast);
                    while let Some(c) = child {
                        if c.is_computed_prop(ast) || c.is_computed_field_def(ast) {
                            self.walk(
                                c.get_first_child(ast).unwrap(),
                                Some(c),
                                enclosing_fn,
                                scope_root,
                                current_script,
                            );
                        }
                        child = c.get_next(ast);
                    }
                    let mut child = n.get_first_child(ast);
                    while let Some(c) = child {
                        match c.get_token(ast) {
                            Token::COMPUTED_PROP => {
                                self.walk(
                                    c.get_last_child(ast).unwrap(),
                                    Some(c),
                                    enclosing_fn,
                                    scope_root,
                                    current_script,
                                );
                            }
                            Token::COMPUTED_FIELD_DEF => {
                                if c.has_two_children(ast) {
                                    self.walk(
                                        c.get_last_child(ast).unwrap(),
                                        Some(c),
                                        enclosing_fn,
                                        c,
                                        current_script,
                                    );
                                }
                            }
                            Token::MEMBER_FIELD_DEF => {
                                self.walk(c, Some(n), enclosing_fn, c, current_script);
                            }
                            _ => self.walk(c, Some(n), enclosing_fn, scope_root, current_script),
                        }
                        child = c.get_next(ast);
                    }
                    return;
                }
                Token::BLOCK => {
                    let child_scope = if !n.is_synthetic_block(ast)
                        && parent
                            .is_some_and(|p| !NodeUtil::is_switch_case(ast, p) && !p.is_catch(ast))
                    {
                        n
                    } else {
                        scope_root
                    };
                    let mut child = n.get_first_child(ast);
                    while let Some(c) = child {
                        self.walk(c, Some(n), enclosing_fn, child_scope, current_script);
                        child = c.get_next(ast);
                    }
                    return;
                }
                Token::FOR
                | Token::FOR_IN
                | Token::FOR_OF
                | Token::FOR_AWAIT_OF
                | Token::SWITCH_BODY => {
                    let mut child = n.get_first_child(ast);
                    while let Some(c) = child {
                        self.walk(c, Some(n), enclosing_fn, n, current_script);
                        child = c.get_next(ast);
                    }
                    return;
                }
                _ => {
                    let mut child = n.get_first_child(ast);
                    while let Some(c) = child {
                        self.walk(c, Some(n), enclosing_fn, scope_root, current_script);
                        child = c.get_next(ast);
                    }
                }
            }
            match token {
                Token::OPTCHAIN_CALL | Token::CALL => {
                    let child = n.get_first_child(ast).unwrap();
                    let matched = if child.is_name(ast) {
                        let name = child.get_string_ref(ast);
                        self.may_contain_name(name) && self.candidate_names.contains(name)
                    } else if child.is_function(ast) {
                        self.anon_fns
                            .get(&child)
                            .is_some_and(|name| self.candidate_names.contains(name))
                    } else if NodeUtil::is_function_object_call(ast, n) {
                        let fn_id = child.get_first_child(ast).unwrap();
                        if fn_id.is_name(ast) {
                            let name = fn_id.get_string_ref(ast);
                            self.may_contain_name(name) && self.candidate_names.contains(name)
                        } else if fn_id.is_function(ast) {
                            self.anon_fns
                                .get(&fn_id)
                                .is_some_and(|name| self.candidate_names.contains(name))
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    if matched {
                        self.events.push(CandidateRefEvent::Call {
                            call_node: n,
                            enclosing_fn,
                            scope_root,
                            script: current_script,
                        });
                    }
                }
                Token::NAME => {
                    let name = n.get_string_ref(ast);
                    if self.may_contain_name(name)
                        && self.candidate_names.contains(name)
                        && !InlineFunctions::is_candidate_usage(ast, n)
                    {
                        self.events.push(CandidateRefEvent::Name {
                            node: n,
                            parent: parent.unwrap(),
                            script: current_script,
                        });
                    }
                }
                Token::TAGGED_TEMPLATELIT => {
                    if let Some(ef) = enclosing_fn {
                        self.tagged_fns.insert(ef);
                    }
                }
                Token::FUNCTION
                    if !self.tagged_fns.is_empty() && self.tagged_fns.shift_remove(&n) =>
                {
                    self.events
                        .push(CandidateRefEvent::TaggedTemplateFn { fn_node: n });
                    if let Some(ef) = enclosing_fn {
                        self.tagged_fns.insert(ef);
                    }
                }
                _ => {}
            }
        }
    }

    let mut scanner = Scanner {
        ast,
        name_bloom,
        candidate_names,
        anon_fns,
        tagged_fns: IndexSet::default(),
        events,
    };
    for &node in &unit.nodes {
        scanner.walk(node, unit.parent, None, root, unit.script);
    }
}

/// Find functions that might be inlined.
struct FindCandidateFunctions<'a> {
    outer: &'a mut InlineFunctions,
    calls_seen: i32,
    current_script: Option<NodeId>,
    current_chunk: Option<JSChunk>,
    current_node: Option<NodeId>,
}

impl<'a> FindCandidateFunctions<'a> {
    fn new(outer: &'a mut InlineFunctions, compiler: &AbstractCompiler, root: NodeId) -> Self {
        let current_script = NodeUtil::get_enclosing_script(compiler, root);
        let current_chunk = current_script.and_then(|s| chunk_of_script(compiler, s));
        Self {
            outer,
            calls_seen: 0,
            current_script,
            current_chunk,
            current_node: None,
        }
    }

    fn replay_event(&mut self, compiler: &mut AbstractCompiler, event: CandidateFnEvent) {
        match event {
            CandidateFnEvent::Named {
                node,
                parent,
                script,
            } => {
                if self.current_script != script {
                    self.current_script = script;
                    self.current_chunk = script.and_then(|s| chunk_of_script(compiler, s));
                }
                self.current_node = Some(node);
                self.find_named_functions(compiler, node, parent);
            }
            CandidateFnEvent::Expr { call_node, script } => {
                if self.current_script != script {
                    self.current_script = script;
                    self.current_chunk = script.and_then(|s| chunk_of_script(compiler, s));
                }
                self.current_node = Some(call_node);
                self.find_function_expressions(compiler, call_node);
            }
        }
    }

    fn walk_node(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<NodeId>,
        in_global_hoist_scope: bool,
    ) {
        let token = n.get_token(compiler);
        match token {
            Token::SCRIPT => {
                let prev_script = self.current_script;
                let prev_chunk = self.current_chunk.take();
                self.current_script = Some(n);
                self.current_chunk = chunk_of_script(compiler, n);
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    self.walk_node(compiler, c, Some(n), in_global_hoist_scope);
                    child = c.get_next(compiler);
                }
                self.current_script = prev_script;
                self.current_chunk = prev_chunk;
                return;
            }
            Token::FUNCTION => {
                let fn_name = n.get_first_child(compiler).unwrap();
                let args = fn_name.get_next(compiler).unwrap();
                let body = args.get_next(compiler).unwrap();
                let is_decl = parent.is_some() && NodeUtil::is_function_declaration(compiler, n);
                self.walk_node(
                    compiler,
                    fn_name,
                    Some(n),
                    if is_decl {
                        in_global_hoist_scope
                    } else {
                        false
                    },
                );
                self.walk_node(compiler, args, Some(n), false);
                self.walk_node(compiler, body, Some(n), false);
            }
            Token::MODULE_BODY => {
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    self.walk_node(compiler, c, Some(n), false);
                    child = c.get_next(compiler);
                }
                return;
            }
            Token::CLASS => {
                let class_name = n.get_first_child(compiler).unwrap();
                let extends_clause = class_name.get_next(compiler).unwrap();
                let body = extends_clause.get_next(compiler).unwrap();
                self.walk_node(compiler, extends_clause, Some(n), in_global_hoist_scope);
                self.walk_node(compiler, class_name, Some(n), in_global_hoist_scope);
                self.walk_node(compiler, body, Some(n), in_global_hoist_scope);
                return;
            }
            Token::CLASS_MEMBERS => {
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    if c.is_computed_prop(compiler) || c.is_computed_field_def(compiler) {
                        self.walk_node(
                            compiler,
                            c.get_first_child(compiler).unwrap(),
                            Some(c),
                            in_global_hoist_scope,
                        );
                    }
                    child = c.get_next(compiler);
                }
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    match c.get_token(compiler) {
                        Token::COMPUTED_PROP => {
                            self.walk_node(
                                compiler,
                                c.get_last_child(compiler).unwrap(),
                                Some(c),
                                in_global_hoist_scope,
                            );
                        }
                        Token::COMPUTED_FIELD_DEF => {
                            if c.has_two_children(compiler) {
                                self.walk_node(
                                    compiler,
                                    c.get_last_child(compiler).unwrap(),
                                    Some(c),
                                    false,
                                );
                            }
                        }
                        Token::MEMBER_FIELD_DEF => {
                            self.walk_node(compiler, c, Some(n), false);
                        }
                        _ => self.walk_node(compiler, c, Some(n), in_global_hoist_scope),
                    }
                    child = c.get_next(compiler);
                }
                return;
            }
            Token::BLOCK => {
                let child_global =
                    in_global_hoist_scope && !NodeUtil::is_class_static_block(compiler, n);
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    self.walk_node(compiler, c, Some(n), child_global);
                    child = c.get_next(compiler);
                }
                return;
            }
            _ => {
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    self.walk_node(compiler, c, Some(n), in_global_hoist_scope);
                    child = c.get_next(compiler);
                }
            }
        }
        // port: InlineFunctions.FindCandidateFunctions#visit
        match token {
            Token::VAR | Token::LET | Token::CONST | Token::FUNCTION
                if self.outer.reach.includes_globals() || !in_global_hoist_scope =>
            {
                self.current_node = Some(n);
                self.find_named_functions(compiler, n, parent);
            }
            Token::OPTCHAIN_CALL | Token::CALL
                if self.outer.reach.includes_globals() || !in_global_hoist_scope =>
            {
                self.current_node = Some(n);
                self.find_function_expressions(compiler, n);
            }
            _ => {}
        }
    }

    // port: InlineFunctions.FindCandidateFunctions#findNamedFunctions
    fn find_named_functions(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<NodeId>,
    ) {
        if !NodeUtil::is_statement(compiler, n) {
            // There aren't any interesting functions here.
            return;
        }

        match n.get_token(compiler) {
            Token::VAR | Token::LET | Token::CONST => {
                // Functions expressions in the form of:
                //   var fooFn = function(x) { return ... }
                check_state!(n.has_one_child(compiler), "%s", n.to_string(compiler));
                let name_node = n.get_first_child(compiler).unwrap();
                if name_node.is_name(compiler)
                    && name_node.has_children(compiler)
                    && name_node
                        .get_first_child(compiler)
                        .unwrap()
                        .is_function(compiler)
                {
                    let chunk = self.current_chunk.clone();
                    self.outer.maybe_add_function(
                        compiler,
                        Function::Var(FunctionVar::new(n)),
                        chunk,
                    );
                }
                // Named functions
                // function Foo(x) { return ... }
            }
            Token::FUNCTION => {
                let parent = parent.unwrap();
                check_state!(
                    NodeUtil::is_statement_block(compiler, parent) || parent.is_label(compiler)
                );
                if NodeUtil::is_function_declaration(compiler, n) {
                    let r#fn = Function::Named(NamedFunction::new(n));
                    let chunk = self.current_chunk.clone();
                    self.outer.maybe_add_function(compiler, r#fn, chunk);
                }
            }
            _ => {}
        }
    }

    /// Find function expressions that are called directly in the form of
    /// (function(a,b,...){...})(a,b,...) or (function(a,b,...){...}).call(this,a,b, ...)
    // port: InlineFunctions.FindCandidateFunctions#findFunctionExpressions
    fn find_function_expressions(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        match n.get_token(compiler) {
            Token::OPTCHAIN_CALL | Token::CALL => {
                // Functions expressions in the form of:
                //   (function(){})();
                let mut fn_node = None;
                if n.get_first_child(compiler).unwrap().is_function(compiler) {
                    fn_node = n.get_first_child(compiler);
                } else if NodeUtil::is_function_object_call(compiler, n) {
                    let fn_identifying_node = n.get_first_first_child(compiler).unwrap();
                    if fn_identifying_node.is_function(compiler) {
                        fn_node = Some(fn_identifying_node);
                    }
                }

                // If an interesting function was discovered, add it.
                if let Some(fn_node) = fn_node {
                    let calls_seen = self.calls_seen;
                    self.calls_seen += 1;
                    let r#fn = Function::Expression(FunctionExpression::new(
                        compiler, fn_node, calls_seen,
                    ));
                    // Rust-only: `fn` moves into maybeAddFunction; its name is read first.
                    let name = r#fn.get_name(compiler);
                    let chunk = self.current_chunk.clone();
                    self.outer.maybe_add_function(compiler, r#fn, chunk);
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
    /// Rust-only (performance divergence): optional set of nodes along ancestor paths to surviving
    /// call sites (used in Walk 3) and a 4096-bit Bloom filter over `function_map` keys.
    allowed_nodes: Option<IndexSet<NodeId>>,
    name_bloom: [u64; 64],
}

impl<'a, C: CallVisitorCallback> CallVisitor<'a, C> {
    #[inline]
    fn build_name_bloom(fns: &IndexMap<JsString, FunctionState>) -> [u64; 64] {
        let mut bloom = [0u64; 64];
        for name in fns.keys() {
            let h = name.hash_code() as usize;
            bloom[(h >> 6) & 63] |= 1u64 << (h & 63);
        }
        bloom
    }

    #[inline]
    fn may_contain_name(&self, name: &JsString) -> bool {
        let h = name.hash_code() as usize;
        (self.name_bloom[(h >> 6) & 63] & (1u64 << (h & 63))) != 0
    }

    // port: InlineFunctions.CallVisitor#CallVisitor
    fn new_pruned(
        fns: &'a mut IndexMap<JsString, FunctionState>,
        anon_fns: &'a IndexMap<NodeId, JsString>,
        callback: C,
        allowed_nodes: IndexSet<NodeId>,
    ) -> Self {
        let name_bloom = Self::build_name_bloom(fns);
        Self {
            function_map: fns,
            anon_function_map: anon_fns,
            callback,
            allowed_nodes: Some(allowed_nodes),
            name_bloom,
        }
    }

    // port: InlineFunctions.CallVisitor#visit
    fn visit_calls(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::OPTCHAIN_CALL | Token::CALL => {
                // Function calls
                let child = n.get_first_child(t).unwrap();
                // NOTE: The normalization pass ensures that local names do not collide with
                // global names.
                let fn_state = if child.is_name(t) {
                    let name = child.get_string_ref(t);
                    if self.may_contain_name(name) {
                        self.function_map.get_mut(name)
                    } else {
                        None
                    }
                } else if child.is_function(t) {
                    self.anon_function_map
                        .get(&child)
                        .and_then(|name| self.function_map.get_mut(name))
                } else if NodeUtil::is_function_object_call(t, n) {
                    check_state!(NodeUtil::is_normal_or_opt_chain_get(t, child));
                    let fn_identifying_node = child.get_first_child(t).unwrap();
                    if fn_identifying_node.is_name(t) {
                        let name = fn_identifying_node.get_string_ref(t);
                        if self.may_contain_name(name) {
                            self.function_map.get_mut(name)
                        } else {
                            None
                        }
                    } else if fn_identifying_node.is_function(t) {
                        self.anon_function_map
                            .get(&fn_identifying_node)
                            .and_then(|name| self.function_map.get_mut(name))
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Only visit call-sites for functions that can be inlined.
                if let Some(function_state) = fn_state {
                    self.callback.visit_call_site(t, n, function_state);
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
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        self.allowed_nodes
            .as_ref()
            .is_none_or(|allowed| allowed.contains(&n))
    }

    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self.visit_calls(t, n, parent);
    }
}

/// Find references to functions that are inlinable.
///
/// Rust-only (performance divergence): walks the AST directly instead of through `NodeTraversal`,
/// materializing scopes lazily via `ensure_ancestor_scope` only when a call site matches a
/// candidate function.
struct FindCandidatesReferences<'a> {
    root: NodeId,
    function_map: &'a mut IndexMap<JsString, FunctionState>,
    anon_function_map: &'a IndexMap<NodeId, JsString>,
    name_bloom: [u64; 64],
    injector: &'a mut FunctionInjector,
    enforce_max_size_after_inlining: bool,
    max_size_after_inlining: i32,
    functions_with_tagged_template_lit: IndexSet<NodeId>,
    scope_creator: SyntacticScopeCreator<'static>,
    scope_memo: IndexMap<NodeId, Scope>,
    current_script: Option<NodeId>,
    current_chunk: Option<JSChunk>,
    current_node: Option<NodeId>,
    /// Rust-only (performance divergence): the `DeepFunctionStamp` of each root of call sites and
    /// candidate function taken in this run, and the kept stamps found current (`NoInlineSite`).
    stamps_now: IndexMap<NodeId, Arc<DeepFunctionStamp>>,
    stamps_checked: IndexMap<NodeId, (Arc<DeepFunctionStamp>, bool)>,
}

impl<'a> FindCandidatesReferences<'a> {
    /// Rust-only (performance divergence): the root whose `DeepFunctionStamp` covers what
    /// `maybeAddReferenceUsingMode` reads around `call_node`: the function around it, else the
    /// statement of the script it is in (`None` elsewhere).
    fn site_root(ast: &Ast, call_node: NodeId, enclosing_fn: Option<NodeId>) -> Option<NodeId> {
        if enclosing_fn.is_some() {
            return enclosing_fn;
        }
        let mut node = call_node;
        loop {
            let parent = node.get_parent(ast)?;
            if parent.is_script(ast) {
                return Some(node);
            }
            node = parent;
        }
    }

    /// The stamp of `root` taken in this run.
    fn stamp_now(
        &mut self,
        compiler: &mut AbstractCompiler,
        root: NodeId,
    ) -> Arc<DeepFunctionStamp> {
        if let Some(stamp) = self.stamps_now.get(&root) {
            return Arc::clone(stamp);
        }
        let stamp = Arc::new(SyntacticScopeCache::deep_function_stamp(compiler, root));
        self.stamps_now.insert(root, Arc::clone(&stamp));
        stamp
    }

    /// Whether the kept `stamp` of `root` is current.
    fn is_current(
        &mut self,
        compiler: &mut AbstractCompiler,
        root: NodeId,
        stamp: &Arc<DeepFunctionStamp>,
    ) -> bool {
        if let Some((checked, current)) = self.stamps_checked.get(&root)
            && Arc::ptr_eq(checked, stamp)
        {
            return *current;
        }
        let own = SyntacticScopeCache::subtree_stamp_with_flags(compiler, root);
        let current = compiler
            .syntactic_scope_cache
            .deep_function_stamp_is_current(own, stamp);
        self.stamps_checked
            .insert(root, (Arc::clone(stamp), current));
        current
    }

    // port: InlineFunctions.FindCandidatesReferences#FindCandidatesReferences
    fn new(
        compiler: &AbstractCompiler,
        root: NodeId,
        fns: &'a mut IndexMap<JsString, FunctionState>,
        anon_fns: &'a IndexMap<NodeId, JsString>,
        injector: &'a mut FunctionInjector,
        enforce_max_size_after_inlining: bool,
        max_size_after_inlining: i32,
    ) -> Self {
        let name_bloom = CallVisitor::<Inline<'_>>::build_name_bloom(fns);
        let current_script = NodeUtil::get_enclosing_script(compiler, root);
        let current_chunk = current_script.and_then(|s| chunk_of_script(compiler, s));
        Self {
            root,
            function_map: fns,
            anon_function_map: anon_fns,
            name_bloom,
            injector,
            enforce_max_size_after_inlining,
            max_size_after_inlining,
            functions_with_tagged_template_lit: IndexSet::<_>::default(),
            scope_creator: SyntacticScopeCreator::new(),
            scope_memo: IndexMap::default(),
            stamps_now: IndexMap::default(),
            stamps_checked: IndexMap::default(),
            current_script,
            current_chunk,
            current_node: None,
        }
    }

    #[inline]
    fn may_contain_name(&self, name: &JsString) -> bool {
        let h = name.hash_code() as usize;
        (self.name_bloom[(h >> 6) & 63] & (1u64 << (h & 63))) != 0
    }

    fn replay_event(&mut self, compiler: &mut AbstractCompiler, event: CandidateRefEvent) {
        match event {
            CandidateRefEvent::Call {
                call_node,
                enclosing_fn,
                scope_root,
                script,
            } => {
                if self.current_script != script {
                    self.current_script = script;
                    self.current_chunk = script.and_then(|s| chunk_of_script(compiler, s));
                }
                self.visit_calls(compiler, call_node, enclosing_fn, scope_root);
            }
            CandidateRefEvent::Name {
                node,
                parent,
                script,
            } => {
                if self.current_script != script {
                    self.current_script = script;
                    self.current_chunk = script.and_then(|s| chunk_of_script(compiler, s));
                }
                self.check_name_usage(compiler, node, parent);
            }
            CandidateRefEvent::TaggedTemplateFn { fn_node } => {
                if let Some(function_state) =
                    self.get_function_state_for_function_node(compiler, fn_node)
                {
                    function_state.set_has_tagged_template_lit(true);
                }
            }
        }
    }

    fn walk_node(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<NodeId>,
        enclosing_fn: Option<NodeId>,
        scope_root: NodeId,
    ) {
        let token = n.get_token(compiler);
        match token {
            Token::SCRIPT => {
                let prev_script = self.current_script;
                let prev_chunk = self.current_chunk.take();
                self.current_script = Some(n);
                self.current_chunk = chunk_of_script(compiler, n);
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    self.walk_node(compiler, c, Some(n), enclosing_fn, scope_root);
                    child = c.get_next(compiler);
                }
                self.current_script = prev_script;
                self.current_chunk = prev_chunk;
                return;
            }
            Token::FUNCTION => {
                let fn_name = n.get_first_child(compiler).unwrap();
                let args = fn_name.get_next(compiler).unwrap();
                let body = args.get_next(compiler).unwrap();
                let is_decl = parent.is_some() && NodeUtil::is_function_declaration(compiler, n);
                if is_decl {
                    self.walk_node(compiler, fn_name, Some(n), enclosing_fn, scope_root);
                } else {
                    self.walk_node(compiler, fn_name, Some(n), Some(n), n);
                }
                self.walk_node(compiler, args, Some(n), Some(n), n);
                self.walk_node(compiler, body, Some(n), Some(n), n);
            }
            Token::MODULE_BODY => {
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    self.walk_node(compiler, c, Some(n), enclosing_fn, n);
                    child = c.get_next(compiler);
                }
                return;
            }
            Token::CLASS => {
                let class_name = n.get_first_child(compiler).unwrap();
                let extends_clause = class_name.get_next(compiler).unwrap();
                let body = extends_clause.get_next(compiler).unwrap();
                let is_expr = NodeUtil::is_class_expression(compiler, n);
                self.walk_node(compiler, extends_clause, Some(n), enclosing_fn, scope_root);
                self.walk_node(
                    compiler,
                    class_name,
                    Some(n),
                    enclosing_fn,
                    if is_expr { n } else { scope_root },
                );
                self.walk_node(compiler, body, Some(n), enclosing_fn, n);
                return;
            }
            Token::CLASS_MEMBERS => {
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    if c.is_computed_prop(compiler) || c.is_computed_field_def(compiler) {
                        self.walk_node(
                            compiler,
                            c.get_first_child(compiler).unwrap(),
                            Some(c),
                            enclosing_fn,
                            scope_root,
                        );
                    }
                    child = c.get_next(compiler);
                }
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    match c.get_token(compiler) {
                        Token::COMPUTED_PROP => {
                            self.walk_node(
                                compiler,
                                c.get_last_child(compiler).unwrap(),
                                Some(c),
                                enclosing_fn,
                                scope_root,
                            );
                        }
                        Token::COMPUTED_FIELD_DEF => {
                            if c.has_two_children(compiler) {
                                self.walk_node(
                                    compiler,
                                    c.get_last_child(compiler).unwrap(),
                                    Some(c),
                                    enclosing_fn,
                                    c,
                                );
                            }
                        }
                        Token::MEMBER_FIELD_DEF => {
                            self.walk_node(compiler, c, Some(n), enclosing_fn, c);
                        }
                        _ => self.walk_node(compiler, c, Some(n), enclosing_fn, scope_root),
                    }
                    child = c.get_next(compiler);
                }
                return;
            }
            Token::BLOCK => {
                let child_scope = if !n.is_synthetic_block(compiler)
                    && parent.is_some_and(|p| {
                        !NodeUtil::is_switch_case(compiler, p) && !p.is_catch(compiler)
                    }) {
                    n
                } else {
                    scope_root
                };
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    self.walk_node(compiler, c, Some(n), enclosing_fn, child_scope);
                    child = c.get_next(compiler);
                }
                return;
            }
            Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::SWITCH_BODY => {
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    self.walk_node(compiler, c, Some(n), enclosing_fn, n);
                    child = c.get_next(compiler);
                }
                return;
            }
            _ => {
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    self.walk_node(compiler, c, Some(n), enclosing_fn, scope_root);
                    child = c.get_next(compiler);
                }
            }
        }
        // port: InlineFunctions.FindCandidatesReferences#visit
        match token {
            Token::OPTCHAIN_CALL | Token::CALL => {
                self.visit_calls(compiler, n, enclosing_fn, scope_root);
            }
            Token::NAME => {
                self.check_name_usage(compiler, n, parent.unwrap());
            }
            Token::TAGGED_TEMPLATELIT => {
                if let Some(enclosing_fn) = enclosing_fn {
                    self.functions_with_tagged_template_lit.insert(enclosing_fn);
                }
            }
            Token::FUNCTION
                if !self.functions_with_tagged_template_lit.is_empty()
                    && self.functions_with_tagged_template_lit.shift_remove(&n) =>
            {
                let function_state = self.get_function_state_for_function_node(compiler, n);
                if let Some(function_state) = function_state {
                    function_state.set_has_tagged_template_lit(true);
                }
                if let Some(enclosing_fn) = enclosing_fn {
                    self.functions_with_tagged_template_lit.insert(enclosing_fn);
                }
            }
            _ => {}
        }
    }

    // port: InlineFunctions.FindCandidatesReferences#getFunctionStateForFunctionNode
    fn get_function_state_for_function_node(
        &mut self,
        ast: &Ast,
        fn_node: NodeId,
    ) -> Option<&mut FunctionState> {
        let mut name = self.anon_function_map.get(&fn_node).cloned();
        if name.is_none() {
            if NodeUtil::is_function_declaration(ast, fn_node) {
                name = Some(fn_node.get_first_child(ast).unwrap().get_string(ast));
            } else if fn_node.get_parent(ast).unwrap().is_name(ast)
                && fn_node.get_grandparent(ast).is_some()
                && NodeUtil::is_name_declaration(ast, fn_node.get_grandparent(ast))
            {
                name = Some(fn_node.get_parent(ast).unwrap().get_string(ast));
            }
        }
        let name = name?;
        let function_state = self.function_map.get_mut(&name);
        if let Some(function_state) = function_state
            && function_state.has_existing_function_definition()
            && function_state.get_fn().get_function_node(ast) == fn_node
        {
            return Some(function_state);
        }
        None
    }

    /// Find functions that can be inlined.
    // port: InlineFunctions.FindCandidatesReferences#checkNameUsage
    #[inline]
    fn check_name_usage(&mut self, compiler: &mut AbstractCompiler, n: NodeId, parent: NodeId) {
        // Rust-only (performance divergence): test Bloom filter before is_candidate_usage or
        // hash-map probing; the vast majority of NAME nodes are not candidate functions.
        let name = n.get_string_ref(compiler);
        if !self.may_contain_name(name) {
            return;
        }
        if !self.function_map.contains_key(name) {
            return;
        }

        if InlineFunctions::is_candidate_usage(compiler, n) {
            return;
        }

        self.current_node = Some(n);
        // Other refs to a function name remove its candidacy for inlining
        // Rust-only: the name is read in place (performance divergence).
        let function_state = self
            .function_map
            .get_mut(n.get_string_ref(compiler))
            .unwrap();

        // If the name is being assigned to or modified as an L-value (e.g. `bar = something;`,
        // destructuring `[bar] = ...`, or `for (bar of ...)`), it cannot be inlined.
        if NodeUtil::is_l_value(compiler, n) {
            // Mark the function as uninlinable.
            function_state.disallow_inlining(compiler, DisallowInliningReason::REASSIGNED);
        } else if (parent.is_assign(compiler)
            && parent.get_jsdoc_info_ref(compiler).is_some()
            && parent.get_jsdoc_info(compiler).unwrap().is_constant()
            && parent.get_second_child(compiler) == Some(n))
            || (parent.get_parent(compiler).is_some()
                && parent.get_parent(compiler).unwrap().is_const(compiler)
                && parent.get_first_child(compiler) == Some(n))
        {
            // e.g. const bar = foo; exports.bar = foo;
            // We can't see through this reference right now, but we will be able
            // to once `bar` is inlined; so we shouldn't remove `foo` right now but
            // this is probably okay with @requireInlining.
            function_state.set_remove(
                false,
                compiler,
                CannotRemoveReason::UNREMOVABLE_REFERENCE,
                ShouldWarnWhenRequireInliningCannotInline::NO,
            );
        } else {
            // e.g. var fn = bar; <== we can't inline "bar"
            // As this reference can't be inlined mark the function as
            // unremovable.
            function_state.set_remove_with_context(
                false,
                compiler,
                CannotRemoveReason::UNREMOVABLE_REFERENCE,
                ShouldWarnWhenRequireInliningCannotInline::YES,
                /* contextNode= */ Some(parent),
            );
        }
    }

    // port: InlineFunctions.FindCandidatesReferences#visitCallSite
    fn visit_calls(
        &mut self,
        compiler: &mut AbstractCompiler,
        call_node: NodeId,
        enclosing_fn: Option<NodeId>,
        scope_root: NodeId,
    ) {
        let child = call_node.get_first_child(compiler).unwrap();
        let fn_key: Option<JsString> = if child.is_name(compiler) {
            let name = child.get_string_ref(compiler);
            if self.may_contain_name(name) && self.function_map.contains_key(name) {
                Some(name.clone())
            } else {
                None
            }
        } else if child.is_function(compiler) {
            self.anon_function_map
                .get(&child)
                .filter(|name| self.function_map.contains_key(*name))
                .cloned()
        } else if NodeUtil::is_function_object_call(compiler, call_node) {
            check_state!(NodeUtil::is_normal_or_opt_chain_get(compiler, child));
            let fn_identifying_node = child.get_first_child(compiler).unwrap();
            if fn_identifying_node.is_name(compiler) {
                let name = fn_identifying_node.get_string_ref(compiler);
                if self.may_contain_name(name) && self.function_map.contains_key(name) {
                    Some(name.clone())
                } else {
                    None
                }
            } else if fn_identifying_node.is_function(compiler) {
                self.anon_function_map
                    .get(&fn_identifying_node)
                    .filter(|name| self.function_map.contains_key(*name))
                    .cloned()
            } else {
                None
            }
        } else {
            None
        };

        let Some(fn_key) = fn_key else {
            return;
        };
        self.current_node = Some(call_node);
        let chunk = self.current_chunk.clone();
        self.maybe_add_reference(
            compiler,
            &fn_key,
            call_node,
            chunk,
            enclosing_fn,
            scope_root,
        );
    }

    // port: InlineFunctions.FindCandidatesReferences#maybeAddReference
    fn maybe_add_reference(
        &mut self,
        compiler: &mut AbstractCompiler,
        fn_key: &JsString,
        call_node: NodeId,
        chunk: Option<JSChunk>,
        enclosing_fn: Option<NodeId>,
        scope_root: NodeId,
    ) {
        if !self.function_map[fn_key].can_inline() {
            return;
        }

        let mut mode = if self.function_map[fn_key].can_inline_directly() {
            InliningMode::DIRECT
        } else {
            InliningMode::BLOCK
        };
        let mut reference_added = self.maybe_add_reference_using_mode(
            compiler,
            fn_key,
            call_node,
            chunk.clone(),
            mode,
            enclosing_fn,
            scope_root,
        );
        if !reference_added && mode == InliningMode::DIRECT {
            // This reference can not be directly inlined, see if
            // block replacement inlining is possible.
            mode = InliningMode::BLOCK;
            reference_added = self.maybe_add_reference_using_mode(
                compiler,
                fn_key,
                call_node,
                chunk,
                mode,
                enclosing_fn,
                scope_root,
            );
        }

        if !reference_added {
            // Don't try to remove a function if we can't inline all
            // the references.
            self.function_map
                .get_mut(fn_key)
                .unwrap()
                .set_remove_with_context(
                    false,
                    compiler,
                    CannotRemoveReason::UNINLINABLE_REFERENCE,
                    ShouldWarnWhenRequireInliningCannotInline::NO,
                    /* contextNode= */ Some(call_node),
                );
        }
    }

    // port: InlineFunctions.FindCandidatesReferences#maybeAddReferenceUsingMode
    #[allow(clippy::too_many_arguments)]
    fn maybe_add_reference_using_mode(
        &mut self,
        compiler: &mut AbstractCompiler,
        fn_key: &JsString,
        call_node: NodeId,
        chunk: Option<JSChunk>,
        mode: InliningMode,
        enclosing_fn: Option<NodeId>,
        scope_root: NodeId,
    ) -> bool {
        // Rust-only (performance divergence): see `NoInlineSite`.
        let site = (*FUNCTION_FACTS_MODE).and_then(|verify| {
            let root = Self::site_root(compiler, call_node, enclosing_fn)?;
            let fn_node = self.function_map[fn_key]
                .get_fn()
                .get_function_node(compiler);
            Some((
                root,
                fn_node,
                (call_node, fn_node, mode == InliningMode::DIRECT),
                verify,
            ))
        });
        if let Some((root, fn_node, key, verify)) = site
            && let Some(kept) = compiler
                .inline_functions_facts
                .no_inline_sites
                .get(&key)
                .map(|kept| (Arc::clone(&kept.site), Arc::clone(&kept.target)))
            && self.is_current(compiler, root, &kept.0)
            && self.is_current(compiler, fn_node, &kept.1)
        {
            if !verify {
                return false;
            }
            let added = self.add_reference_using_mode(
                compiler,
                fn_key,
                call_node,
                chunk,
                mode,
                enclosing_fn,
                scope_root,
            );
            assert!(
                !added,
                "InlineFunctions: a call site kept as not inlinable can be inlined"
            );
            return false;
        }
        let added = self.add_reference_using_mode(
            compiler,
            fn_key,
            call_node,
            chunk,
            mode,
            enclosing_fn,
            scope_root,
        );
        if !added && let Some((root, fn_node, key, _)) = site {
            let kept = NoInlineSite {
                site: self.stamp_now(compiler, root),
                target: self.stamp_now(compiler, fn_node),
            };
            compiler
                .inline_functions_facts
                .no_inline_sites
                .insert(key, kept);
        }
        added
    }

    /// The Java method's body (see `maybe_add_reference_using_mode`).
    #[allow(clippy::too_many_arguments)]
    fn add_reference_using_mode(
        &mut self,
        compiler: &mut AbstractCompiler,
        fn_key: &JsString,
        call_node: NodeId,
        chunk: Option<JSChunk>,
        mode: InliningMode,
        enclosing_fn: Option<NodeId>,
        scope_root: NodeId,
    ) -> bool {
        let function_state = &self.function_map[fn_key];
        if self.enforce_max_size_after_inlining
            && InlineFunctions::target_size_after_inline_exceeds_limit(
                self.max_size_after_inlining,
                compiler,
                enclosing_fn,
                function_state,
            )
        {
            return false;
        }

        let scope = ensure_ancestor_scope(
            compiler,
            &mut self.scope_creator,
            &mut self.scope_memo,
            self.root,
            scope_root,
        );
        let mut candidate = Reference::new(call_node, scope, chunk, mode);
        let function_state = &self.function_map[fn_key];
        let fn_node = function_state.get_fn().get_function_node(compiler);
        let result = self.injector.can_inline_reference_to_function(
            compiler,
            &candidate.base,
            fn_node,
            &function_state.get_names_to_alias(),
            function_state.get_references_this(),
            function_state.has_inner_closures(),
        );
        if result != CanInlineResult::NO {
            // Yeah!
            candidate.set_requires_decomposition(result == CanInlineResult::AFTER_PREPARATION);
            self.function_map
                .get_mut(fn_key)
                .unwrap()
                .add_reference(candidate);
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
    has_inner_closures: bool,
    has_tagged_template_lit: bool,
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
            has_inner_closures: false,
            has_tagged_template_lit: false,
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

    /// Whether the function contains inner functions or non-static class field initializers.
    // port: InlineFunctions.FunctionState#setHasInnerClosures
    fn set_has_inner_closures(&mut self, has_inner_closures: bool) {
        self.has_inner_closures = has_inner_closures;
    }

    // port: InlineFunctions.FunctionState#hasInnerClosures
    fn has_inner_closures(&self) -> bool {
        self.has_inner_closures
    }

    // port: InlineFunctions.FunctionState#setHasTaggedTemplateLit
    fn set_has_tagged_template_lit(&mut self, has_tagged_template_lit: bool) {
        self.has_tagged_template_lit = has_tagged_template_lit;
    }

    // port: InlineFunctions.FunctionState#hasTaggedTemplateLit
    fn has_tagged_template_lit(&self) -> bool {
        self.has_tagged_template_lit
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
    TAGGED_TEMPLATE_LITERAL,
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
            DisallowInliningReason::TAGGED_TEMPLATE_LITERAL => "TAGGED_TEMPLATE_LITERAL",
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

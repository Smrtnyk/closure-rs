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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/InlineVariables.java.

//! Port of `InlineVariables.java`.
//!
//! Using the infrastructure provided by [`ReferenceCollector`], identify variables that are used
//! in a way that is safe to move, and then inline them.
//!
//! This pass has two "modes." One mode only inlines variables declared as constants, for legacy
//! compiler clients. The second mode inlines any variable that we can provably inline. Note that
//! the second mode is a superset of the first mode. We only support the first mode for
//! backwards-compatibility with compiler clients that don't want --inline_variables.
//!
//! Rust shape (DESIGN §6/§7): Java's `VarExpert` and `InlineVarAnalysis` class hierarchies become
//! the enums [`VarExpert`] and [`InlineVarAnalysis`]. A `StandardVarExpert` lives in the
//! behavior's expert arena (`InliningBehavior::experts`, Rust-only), so an analysis that Java
//! links to its expert object (the anonymous negative analysis, the `PositiveInlineVarAnalysis`
//! lambdas) keeps the expert's index. Java object identity of `Reference`s (`ref != declaration`)
//! is identity of the elements of the expert's reference list (`std::ptr::eq`). The expert keeps
//! the variable's `ReferenceCollection` as collected when its scope was exited; nothing adds to a
//! collection after its variable's scope is exited, so this is the list Java's expert reads.

use crate::abstract_compiler::AbstractCompiler;
use crate::basic_block::BasicBlock;
use crate::compiler_pass::CompilerPass;
use crate::node_iterators::LocalVarMotion;
use crate::node_traversal::NodeTraversal;
use crate::node_util::NodeUtil;
use crate::reference::Reference;
use crate::reference_collection::ReferenceCollection;
use crate::reference_collector::{Behavior, ReferenceCollector};
use crate::reference_map::ReferenceMap;
use crate::scope::ScopeId;
use crate::syntactic_scope_creator::SyntacticScopeCreator;
use crate::var::VarId;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::{check_argument, check_not_null, check_state};
use std::cell::RefCell;
use std::sync::Arc;

pub struct InlineVariables {
    mode: Mode,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    // Only inline things explicitly marked as constant.
    CONSTANTS_ONLY,
    // Locals only
    LOCALS_ONLY,
    ALL,
}

impl Mode {
    /// Java's `Mode#varPredicate` (`Var::isDeclaredOrInferredConst`, `Var::isLocal`,
    /// `Predicates.alwaysTrue()`).
    // port: InlineVariables.Mode#Mode
    fn var_predicate(self, compiler: &AbstractCompiler, var: VarId) -> bool {
        match self {
            Mode::CONSTANTS_ONLY => var.is_declared_or_inferred_const(compiler),
            Mode::LOCALS_ONLY => var.is_local(compiler),
            Mode::ALL => true,
        }
    }
}

impl InlineVariables {
    /// Java's constructor also takes the compiler, which a pass does not store (DESIGN §6).
    // port: InlineVariables#InlineVariables
    pub fn new(mode: Mode) -> Self {
        Self { mode }
    }
}

impl CompilerPass for InlineVariables {
    // port: InlineVariables#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let mode = self.mode;
        let mut scope_creator = SyntacticScopeCreator::new();
        let mut callback = ReferenceCollector::new_with_filter(
            compiler,
            InliningBehavior::new(mode),
            &mut scope_creator,
            Box::new(move |compiler: &AbstractCompiler, var: VarId| {
                mode.var_predicate(compiler, var)
            }),
        );
        CompilerPass::process(&mut callback, compiler, externs, root);
    }
}

/// Responsible for analyzing a variable to determine if it can be inlined.
///
/// Java's abstract `VarExpert` with its two canonical anonymous instances and
/// `StandardVarExpert` (here an index into the behavior's expert arena).
#[derive(Debug, Clone, Copy)]
enum VarExpert {
    /// `NO_INLINE_SELF_OR_ALIASES_EXPERT`
    NoInlineSelfOrAliases,
    /// `NO_INLINE_SELF_ALIASES_OK_EXPERT`
    NoInlineSelfAliasesOk,
    Standard(usize),
}

/// The result of analyzing a variable to see if it can be inlined.
#[derive(Debug, Clone, Copy)]
enum InlineVarAnalysis {
    /// `NO_INLINE_SELF_OR_ALIASES_ANALYSIS`
    NoInlineSelfOrAliases,
    /// `NO_INLINE_SELF_ALIASES_OK_ANALYSIS`
    NoInlineSelfAliasesOk,
    /// The anonymous negative analysis of `StandardVarExpert#getNegativeInlineVarAnalysis` that
    /// delays calculating safety until it is asked.
    DelayedNegative { expert: usize },
    /// `PositiveInlineVarAnalysis`: indicates that the analyzed variable may be inlined.
    Positive { expert: usize, inliner: Inliner },
    /// `VarIsAliasAnalysis`: indicates that the analyzed variable is an alias and the decision
    /// about whether to inline it must wait until inlining has been done (or not) for the
    /// original value it aliases.
    VarIsAlias { aliased_var: VarId },
}

/// The `Runnable`s a `PositiveInlineVarAnalysis` is created with.
#[derive(Debug, Clone, Copy)]
enum Inliner {
    /// Inline a new `undefined` node for a variable that is never initialized.
    Undefined,
    /// `inlineWellDefinedVariable(v, initValue, referenceInfo.references)`
    WellDefinedVariable { init_value: Option<NodeId> },
    /// `inline(declaration, initialization, singleReadReference)` (reference list indices)
    Inline {
        declaration: usize,
        initialization: usize,
        single_read_reference: usize,
    },
}

impl InlineVarAnalysis {
    /// Should we inline this variable?
    ///
    /// Mutually exclusive with `should_wait_for_aliased_var()`.
    // port: InlineVariables.InlineVarAnalysis#shouldInline
    // port: InlineVariables.InliningBehavior.PositiveInlineVarAnalysis#shouldInline
    fn should_inline(&self) -> bool {
        matches!(self, InlineVarAnalysis::Positive { .. })
    }

    /// True if this variable is an alias.
    ///
    /// The caller should wait for the aliased variable to be handled, then call the expert's
    /// `reanalyze_after_aliased_var()` method.
    ///
    /// Mutually exclusive with `should_inline()`
    // port: InlineVariables.InlineVarAnalysis#shouldWaitForAliasedVar
    // port: InlineVariables.InliningBehavior.VarIsAliasAnalysis#shouldWaitForAliasedVar
    fn should_wait_for_aliased_var(&self) -> bool {
        matches!(self, InlineVarAnalysis::VarIsAlias { .. })
    }

    /// Gets the aliased variable, if this is an alias.
    // port: InlineVariables.InlineVarAnalysis#getAliasedVar
    // port: InlineVariables.InliningBehavior.VarIsAliasAnalysis#getAliasedVar
    fn get_aliased_var(&self) -> VarId {
        match self {
            InlineVarAnalysis::VarIsAlias { aliased_var } => *aliased_var,
            _ => panic!("no aliased Var"),
        }
    }

    /// True if it is safe for aliases of this variable to inline this variable's name.
    // port: InlineVariables.InlineVarAnalysis#isSafeToInlineAliases
    // port: InlineVariables.NO_INLINE_SELF_ALIASES_OK_ANALYSIS#isSafeToInlineAliases
    // port: InlineVariables.InliningBehavior.StandardVarExpert#getNegativeInlineVarAnalysis.isSafeToInlineAliases
    // port: InlineVariables.InliningBehavior.PositiveInlineVarAnalysis#isSafeToInlineAliases
    // port: InlineVariables.InliningBehavior.VarIsAliasAnalysis#isSafeToInlineAliases
    fn is_safe_to_inline_aliases(
        &self,
        compiler: &mut AbstractCompiler,
        experts: &[RefCell<StandardVarExpert>],
    ) -> bool {
        match self {
            InlineVarAnalysis::NoInlineSelfOrAliases => false,
            InlineVarAnalysis::NoInlineSelfAliasesOk => true,
            InlineVarAnalysis::DelayedNegative { expert } => experts[*expert]
                .borrow_mut()
                .is_well_defined_assigned_once(compiler),
            // If we've inlined this variable, then any aliases of it should definitely try to
            // inline themselves with the new value that replaced this variable.
            // If for some reason the logic that requested this analysis decided not to actually
            // do the inlining, it's still safe to inline the name of this variable.
            // If it weren't, we wouldn't have said it was OK to inline its value.
            InlineVarAnalysis::Positive { .. } => true,
            InlineVarAnalysis::VarIsAlias { .. } => panic!("analysis is incomplete"),
        }
    }

    /// Performs the inline operation.
    // port: InlineVariables.InlineVarAnalysis#performInline
    // port: InlineVariables.InliningBehavior.PositiveInlineVarAnalysis#performInline
    fn perform_inline(
        &self,
        compiler: &mut AbstractCompiler,
        experts: &[RefCell<StandardVarExpert>],
    ) {
        match self {
            InlineVarAnalysis::Positive { expert, inliner } => {
                experts[*expert].borrow().run_inliner(compiler, *inliner);
            }
            _ => panic!("cannot inline"),
        }
    }
}

/// Builds up information about nodes in each scope. When exiting the scope, inspects all
/// variables in that scope, and inlines any that we can.
struct InliningBehavior {
    mode: Mode,

    /// Records the analyses of variables that have already been handled in the current scope.
    ///
    /// This is necessary in order for aliases of those variables to determine whether they may
    /// be inlined.
    current_scope_handled_var_analyses_map: IndexMap<VarKey, InlineVarAnalysis>,

    /// Records alias variables that are waiting for the original variables to be handled.
    ///
    /// The value is an object that knows what to do when the original variable is handled.
    var_to_alias_retry_handlers_map: IndexMap<VarKey, Vec<AliasInlineRetryHandler>>,

    /// Rust-only: the `StandardVarExpert` objects created so far (see the module comment).
    experts: Vec<RefCell<StandardVarExpert>>,
}

impl Behavior for InliningBehavior {
    // port: InlineVariables.InliningBehavior#afterExitScope
    fn after_exit_scope(&mut self, t: &mut NodeTraversal<'_>, reference_map: &dyn ReferenceMap) {
        self.do_inlines_for_scope(t, reference_map);
    }
}

/// Retries inlining an alias variable after the original variable has been dealt with.
#[derive(Debug, Clone, Copy)]
struct AliasInlineRetryHandler {
    v: VarId,
    expert: VarExpert,
}

impl AliasInlineRetryHandler {
    // port: InlineVariables.InliningBehavior.AliasInlineRetryHandler#AliasInlineRetryHandler
    fn new(v: VarId, expert: VarExpert) -> Self {
        Self { v, expert }
    }
}

impl InliningBehavior {
    fn new(mode: Mode) -> Self {
        Self {
            mode,
            current_scope_handled_var_analyses_map: IndexMap::<_, _>::default(),
            var_to_alias_retry_handlers_map: IndexMap::<_, _>::default(),
            experts: Vec::new(),
        }
    }

    /// For all variables in this scope, see if they are only used once. If it looks safe to do
    /// so, inline them.
    // port: InlineVariables.InliningBehavior#doInlinesForScope
    fn do_inlines_for_scope(
        &mut self,
        t: &mut NodeTraversal<'_>,
        reference_map: &dyn ReferenceMap,
    ) {
        // Any variables we completed in an earlier scope are now out of scope,
        // so we can clear this map.
        self.current_scope_handled_var_analyses_map.clear();
        let scope = t.get_scope();
        let compiler = t.get_compiler();
        let may_be_a_parameter_modified_via_arguments =
            self.vars_in_this_scope_may_be_modified_using_arguments(compiler, scope, reference_map);
        for v in scope.get_var_iterable(compiler) {
            let reference_info = reference_map.get_references(v);
            let expert = self.create_var_expert(
                compiler,
                v,
                reference_info,
                may_be_a_parameter_modified_via_arguments,
            );
            let analysis = self.analyze(compiler, expert);
            if analysis.should_wait_for_aliased_var() {
                // We need to wait for the aliased variable to be handled.
                let retry_handler = AliasInlineRetryHandler::new(v, expert);
                let aliased_var = analysis.get_aliased_var();
                let aliased_var_analysis = self
                    .current_scope_handled_var_analyses_map
                    .get(&VarKey::of(compiler, aliased_var))
                    .copied();
                if let Some(aliased_var_analysis) = aliased_var_analysis {
                    // We've actually already completed the aliased Var
                    self.handle_aliased_var_completion(
                        compiler,
                        retry_handler,
                        aliased_var,
                        aliased_var_analysis,
                    );
                } else {
                    // wait for completion of the aliased Var
                    self.var_to_alias_retry_handlers_map
                        .entry(VarKey::of(compiler, aliased_var))
                        .or_default()
                        .push(retry_handler);
                }
            } else {
                if analysis.should_inline() {
                    analysis.perform_inline(compiler, &self.experts);
                }
                // Record the results for aliases we may find later
                self.current_scope_handled_var_analyses_map
                    .insert(VarKey::of(compiler, v), analysis);
                // Retry any aliases we saw before handling this Var
                self.retry_aliases(compiler, v, analysis);
            }
        }
    }

    /// Returns true for function scopes (the whole function, not the body), if the function uses
    /// `arguments` in some way other than a few that are known not to change the values of
    /// parameter variables.
    ///
    /// TODO(bradfordcsmith): In strict mode `arguments` is a copy of the parameters, so modifying
    /// it cannot change the parameters. Sloppy mode is now so rare, that we should probably just
    /// ignore the possibility of `arguments` being used to modify a parameter value. We ignore
    /// loose mode issues or simply don't support them ("with" for instance).
    // port: InlineVariables.InliningBehavior#varsInThisScopeMayBeModifiedUsingArguments
    fn vars_in_this_scope_may_be_modified_using_arguments(
        &self,
        compiler: &mut AbstractCompiler,
        scope: ScopeId,
        reference_map: &dyn ReferenceMap,
    ) -> bool {
        if scope.is_function_scope(compiler)
            && !scope.get_root_node(compiler).is_arrow_function(compiler)
        {
            let arguments = scope.get_arguments_var(compiler);
            let refs = arguments.and_then(|arguments| reference_map.get_references(arguments));
            if let Some(refs) = refs
                && !refs.references.is_empty()
            {
                for r in &refs.references {
                    if !self.is_safe_use_of_arguments(compiler, r.get_node()) {
                        return true;
                    }
                }
            }
        }
        false
    }

    // port: InlineVariables.InliningBehavior#retryAliases
    fn retry_aliases(
        &mut self,
        compiler: &mut AbstractCompiler,
        aliased_var: VarId,
        aliased_var_analysis: InlineVarAnalysis,
    ) {
        let handlers = self
            .var_to_alias_retry_handlers_map
            .shift_remove(&VarKey::of(compiler, aliased_var))
            .unwrap_or_default();
        for alias_inline_retry_handler in handlers {
            self.handle_aliased_var_completion(
                compiler,
                alias_inline_retry_handler,
                aliased_var,
                aliased_var_analysis,
            );
        }
    }

    // port: InlineVariables.InliningBehavior.AliasInlineRetryHandler#handleAliasedVarCompletion
    fn handle_aliased_var_completion(
        &mut self,
        compiler: &mut AbstractCompiler,
        handler: AliasInlineRetryHandler,
        aliased_var: VarId,
        aliased_var_analyis: InlineVarAnalysis,
    ) {
        let new_analysis = self.reanalyze_after_aliased_var(
            compiler,
            handler.expert,
            aliased_var,
            aliased_var_analyis,
        );
        check_state!(
            !new_analysis.should_wait_for_aliased_var(),
            "expert for %s asked to wait a second time",
            handler.v.to_string(compiler)
        );
        if new_analysis.should_inline() {
            new_analysis.perform_inline(compiler, &self.experts);
        }
        self.current_scope_handled_var_analyses_map
            .insert(VarKey::of(compiler, handler.v), new_analysis);
        self.retry_aliases(compiler, handler.v, new_analysis);
    }

    /// Called to conduct the initial analysis.
    // port: InlineVariables.VarExpert#analyze
    // port: InlineVariables.NO_INLINE_SELF_OR_ALIASES_EXPERT#analyze
    // port: InlineVariables.NO_INLINE_SELF_ALIASES_OK_EXPERT#analyze
    fn analyze(&self, compiler: &mut AbstractCompiler, expert: VarExpert) -> InlineVarAnalysis {
        match expert {
            VarExpert::NoInlineSelfOrAliases => InlineVarAnalysis::NoInlineSelfOrAliases,
            VarExpert::NoInlineSelfAliasesOk => InlineVarAnalysis::NoInlineSelfAliasesOk,
            VarExpert::Standard(index) => self.experts[index].borrow_mut().analyze(compiler),
        }
    }

    /// Called for a Var that is an alias after the original variable is handled.
    // port: InlineVariables.VarExpert#reanalyzeAfterAliasedVar
    fn reanalyze_after_aliased_var(
        &self,
        compiler: &mut AbstractCompiler,
        expert: VarExpert,
        aliased_var: VarId,
        aliased_var_analysis: InlineVarAnalysis,
    ) -> InlineVarAnalysis {
        match expert {
            VarExpert::Standard(index) => self.experts[index]
                .borrow_mut()
                .reanalyze_after_aliased_var(
                    compiler,
                    aliased_var,
                    aliased_var_analysis,
                    &self.experts,
                ),
            VarExpert::NoInlineSelfOrAliases | VarExpert::NoInlineSelfAliasesOk => {
                panic!("not waiting for an aliased variable")
            }
        }
    }

    /// Creates a VarExpert object appropriate for the given variable.
    // port: InlineVariables.InliningBehavior#createVarExpert
    fn create_var_expert(
        &mut self,
        compiler: &mut AbstractCompiler,
        v: VarId,
        reference_info: Option<&ReferenceCollection>,
        may_be_a_parameter_modified_via_arguments: bool,
    ) -> VarExpert {
        let Some(reference_info) = reference_info else {
            // If we couldn't collect any reference info, don't try to inline and assume it's
            // unsafe to inline aliases of this variable, too.
            return VarExpert::NoInlineSelfOrAliases;
        };

        let is_declared_or_inferred_constant = v.is_declared_or_inferred_const(compiler);
        if !is_declared_or_inferred_constant && self.mode == Mode::CONSTANTS_ONLY {
            // If we're only inlining constants, then we shouldn't inline an alias.
            return VarExpert::NoInlineSelfOrAliases;
        }

        if v.is_extern(compiler) {
            // TODO(bradfordcsmith): Extern variables are generally unsafe to inline.
            return VarExpert::NoInlineSelfOrAliases;
        }

        if compiler.get_coding_convention().is_exported(
            &v.get_name(compiler),
            /* local= */ v.is_local(compiler),
        ) {
            // If the variable is exported, it might be assigned a new value by code we cannot
            // see, so aliases to it are creating snapshots of its state.
            // We cannot inline this variable or its aliases.
            return VarExpert::NoInlineSelfOrAliases;
        }

        if compiler
            .get_coding_convention()
            .is_property_rename_function(compiler, check_not_null!(v.get_name_node(compiler)))
        {
            // It's not terribly likely that anything creates an alias of our special property
            // rename function, but its value never changes, so it should be safe to inline
            // aliases to it.
            return VarExpert::NoInlineSelfAliasesOk;
        }

        let init_data = VarExpertInitData {
            v,
            reference_info: reference_info.clone(),
            is_declared_or_inferred_constant,
            may_be_a_parameter_modified_via_arguments,
        };

        let index = self.experts.len();
        self.experts.push(RefCell::new(StandardVarExpert::new(
            init_data, self.mode, index,
        )));
        VarExpert::Standard(index)
    }

    /// True if `arguments_node` is a use of `arguments` we know won't modify any parameter
    /// values.
    ///
    /// In sloppy mode it is possible to change the value of a function parameter by assigning to
    /// the corresponding entry in `arguments`.
    ///
    /// This method checks for just a few common uses that are known to be safe. However, we may
    /// soon remove this check entirely because unsafe uses aren't worth worrying about. See the
    /// comment on `vars_in_this_scope_may_be_modified_using_arguments`.
    // port: InlineVariables.InliningBehavior#isSafeUseOfArguments
    fn is_safe_use_of_arguments(&self, ast: &Ast, arguments_node: NodeId) -> bool {
        check_argument!(arguments_node.matches_name(ast, "arguments"));
        // `arguments[i]` that is only read, not assigned
        // or `fn.apply(thisArg, arguments)`
        self.is_target_of_property_read(ast, arguments_node)
            || self.is_second_argument_to_dot_apply_method(ast, arguments_node)
    }

    /// True if `n` is the object in a property access expression (`obj[prop]` or `obj.prop` or
    /// an optional chain version of one of those) and the property is only being read, not
    /// modified.
    // port: InlineVariables.InliningBehavior#isTargetOfPropertyRead
    fn is_target_of_property_read(&self, ast: &Ast, n: NodeId) -> bool {
        let get_node = n.get_parent(ast);
        n.is_first_child_of(ast, get_node)
            && NodeUtil::is_normal_or_opt_chain_get(ast, get_node.unwrap())
            && !NodeUtil::is_l_value(ast, get_node.unwrap())
    }

    /// True if `n` is being used in a call like this: `fn.apply(thisArg, n)`.
    // port: InlineVariables.InliningBehavior#isSecondArgumentToDotApplyMethod
    fn is_second_argument_to_dot_apply_method(&self, ast: &Ast, n: NodeId) -> bool {
        let call_node = n.get_parent(ast).unwrap();
        if NodeUtil::is_normal_or_opt_chain_call(ast, call_node) {
            let callee_node = call_node.get_first_child(ast).unwrap();
            if NodeUtil::is_normal_or_opt_chain_get_prop(ast, callee_node)
                && callee_node.get_string_ref(ast) == "apply"
            {
                let this_arg_node = callee_node.get_next(ast);
                if let Some(this_arg_node) = this_arg_node {
                    return this_arg_node.get_next(ast) == Some(n);
                }
            }
        }
        false
    }

    /// Do the actual work of inlining a single declaration into a single reference.
    // port: InlineVariables.InliningBehavior#inline
    fn inline(compiler: &mut AbstractCompiler, decl: &Reference, init: &Reference, r: &Reference) {
        let value = init.get_assigned_value(compiler);
        check_state!(value.is_some());
        let value = value.unwrap();
        // Check for function declarations before the value is moved in the AST.
        let is_function_declaration = NodeUtil::is_function_declaration(compiler, value);
        if is_function_declaration {
            // We're inlining this function declaration because there was only one reference to
            // it, probably the rhs of an assignment statement. Since we're eliminating the only
            // reference, we can get rid of it. See b/277538101.
            let func_name = value.get_first_child(compiler);
            let func_name = check_not_null!(func_name);
            check_state!(func_name.is_name(compiler));
            func_name.set_string(compiler, "");

            // In addition to changing the containing scope, inlining function declarations also
            // changes the function name scope from the containing scope to the inner scope.
            compiler.report_change_to_change_scope(value);
            compiler.report_change_to_enclosing_scope(value.get_parent(compiler).unwrap());
        }
        let detached = value.detach(compiler);
        Self::inline_value(compiler, r.get_node(), detached);
        if !std::ptr::eq(decl, init) {
            let express_root = init.get_grandparent(compiler).unwrap();
            check_state!(express_root.is_expr_result(compiler));
            let express_root_parent = express_root.get_parent(compiler).unwrap();
            NodeUtil::remove_child(compiler, express_root_parent, express_root);
        }
        // Function declarations have already been removed.
        if !is_function_declaration {
            Self::remove_declaration(compiler, decl);
        }
    }

    /// Inline an immutable variable into all of its references.
    // port: InlineVariables.InliningBehavior#inlineWellDefinedVariable
    fn inline_well_defined_variable(
        compiler: &mut AbstractCompiler,
        v: VarId,
        value: Option<NodeId>,
        ref_set: &[Reference],
    ) {
        for r in ref_set {
            if Some(r.get_node()) == v.get_name_node(compiler) {
                Self::remove_declaration(compiler, r);
            } else if r.is_simple_assignment_to_name(compiler) {
                // This is the initialization.
                //
                // Replace the entire assignment with just the value, and use the original value
                // node in case it contains references to variables that still require inlining.
                let parent = r.get_parent(compiler);
                let detached = check_not_null!(value).detach(compiler);
                Self::inline_value(compiler, parent, detached);
            } else {
                let cloned_value = check_not_null!(value).clone_tree(compiler);
                NodeUtil::mark_new_scopes_changed(compiler, cloned_value);
                Self::inline_value(compiler, r.get_node(), cloned_value);
            }
        }
    }

    /// Remove the given VAR declaration.
    // port: InlineVariables.InliningBehavior#removeDeclaration
    fn remove_declaration(compiler: &mut AbstractCompiler, decl: &Reference) {
        let var_node = decl.get_parent(compiler);
        check_state!(
            NodeUtil::is_name_declaration(compiler, Some(var_node)),
            "%s",
            var_node.to_string(compiler)
        );
        let grandparent = decl.get_grandparent(compiler);

        compiler.report_change_to_enclosing_scope(decl.get_node());
        decl.get_node().detach(compiler);
        // Remove var node if empty
        if !var_node.has_children(compiler) {
            NodeUtil::remove_child(compiler, grandparent.unwrap(), var_node);
        }
    }

    // port: InlineVariables.InliningBehavior#inlineValue
    fn inline_value(compiler: &mut AbstractCompiler, to_remove: NodeId, to_insert: NodeId) {
        compiler.report_change_to_enclosing_scope(to_remove);

        // Help type-based optimizations by propagating more specific types from type assertions
        if to_remove.get_color(compiler).is_some() && to_remove.is_color_from_type_cast(compiler) {
            let color = to_remove.get_color(compiler);
            to_insert.set_color(compiler, color);
            to_insert.set_color_from_type_cast(compiler);
        }
        to_remove.replace_with(compiler, to_insert);
        NodeUtil::mark_functions_deleted(compiler, to_remove);
    }

    /// Returns true if the provided reference and declaration can be safely inlined according to
    /// our criteria
    // port: InlineVariables.InliningBehavior#canInline
    fn can_inline(
        compiler: &mut AbstractCompiler,
        declaration: &Reference,
        initialization: &Reference,
        reference: &Reference,
        init_value: Option<NodeId>,
    ) -> bool {
        // If the value is read more than once, skip it.
        // VAR declarations and EXPR_RESULT don't need the value, but other
        // ASSIGN expressions parents do.
        if !std::ptr::eq(declaration, initialization)
            && !initialization
                .get_grandparent(compiler)
                .unwrap()
                .is_expr_result(compiler)
        {
            return false;
        }

        // Be very conservative and do not cross control structures or scope boundaries
        if !same_basic_block(
            declaration.get_basic_block(),
            initialization.get_basic_block(),
        ) || !same_basic_block(declaration.get_basic_block(), reference.get_basic_block())
        {
            return false;
        }

        // Do not inline into a call node. This would change
        // the context in which it was being called. For example,
        //   var a = b.c;
        //   a();
        // should not be inlined, because it calls a in the context of b
        // rather than the context of the window.
        //   var a = b.c;
        //   f(a)
        // is OK.
        check_state!(init_value.is_some());
        let init_value = init_value.unwrap();
        if init_value.is_get_prop(compiler)
            && reference.get_parent(compiler).is_call(compiler)
            && reference.get_parent(compiler).get_first_child(compiler)
                == Some(reference.get_node())
        {
            return false;
        }

        if init_value.is_function(compiler) {
            let call_node = reference.get_parent(compiler);
            if reference.get_parent(compiler).is_call(compiler) {
                let convention = compiler.get_coding_convention();
                // Bug 2388531: Don't inline subclass definitions into class defining
                // calls as this confused class removing logic.
                let relationship = convention.get_classes_defined_by_call(compiler, call_node);
                if relationship.is_some() {
                    return false;
                }

                // issue 668: Don't inline singleton getter methods
                // calls as this confused class removing logic.
                if convention
                    .get_singleton_getter_class_name(compiler, call_node)
                    .is_some()
                {
                    return false;
                }
            }
        }

        if initialization.get_scope() != declaration.get_scope()
            || !initialization
                .get_scope()
                .unwrap()
                .contains(compiler, reference.get_scope().unwrap())
        {
            return false;
        }

        Self::can_move_aggressively(compiler, init_value)
            || Self::can_move_moderately(compiler, initialization, reference)
    }

    /// If the value is a literal, we can cross more boundaries to inline it.
    // port: InlineVariables.InliningBehavior#canMoveAggressively
    fn can_move_aggressively(ast: &Ast, value: NodeId) -> bool {
        // Function expressions and other mutable objects can move within
        // the same basic block.
        NodeUtil::is_literal_value(ast, value, true) || value.is_function(ast)
    }

    /// If the value of a variable is not constant, then it may read or modify state. Therefore it
    /// cannot be moved past anything else that may modify the value being read or read values
    /// that are modified.
    // port: InlineVariables.InliningBehavior#canMoveModerately
    fn can_move_moderately(
        compiler: &mut AbstractCompiler,
        initialization: &Reference,
        reference: &Reference,
    ) -> bool {
        // Check if declaration can be inlined without passing
        // any side-effect causing nodes.
        let mut it =
            if NodeUtil::is_name_declaration(compiler, Some(initialization.get_parent(compiler))) {
                let name = initialization.get_node(); // NAME
                let var = initialization.get_parent(compiler); // VAR/LET/CONST
                let block = initialization.get_grandparent(compiler).unwrap(); // VAR/LET/CONST container
                LocalVarMotion::for_var(compiler, name, var, block)
            } else if initialization.get_parent(compiler).is_assign(compiler) {
                check_state!(
                    initialization
                        .get_grandparent(compiler)
                        .unwrap()
                        .is_expr_result(compiler)
                );
                let name = initialization.get_node(); // NAME
                let assign = initialization.get_parent(compiler); // ASSIGN
                let expr = initialization.get_grandparent(compiler).unwrap(); // EXPR_RESULT
                let block = expr.get_parent(compiler).unwrap(); // EXPR container
                LocalVarMotion::for_assign(compiler, name, assign, expr, block)
            } else {
                panic!(
                    "Unexpected initialization parent\n{}",
                    initialization.get_parent(compiler).to_string_tree(compiler)
                );
            };
        let target_name = reference.get_node();
        while it.has_next() {
            let cur_node = it.next(compiler);
            if cur_node == Some(target_name) {
                return true;
            }
        }

        false
    }

    /// Returns true if the reference is a normal VAR or FUNCTION declaration.
    // port: InlineVariables.InliningBehavior#isValidDeclaration
    fn is_valid_declaration(ast: &Ast, declaration: &Reference) -> bool {
        (NodeUtil::is_name_declaration(ast, Some(declaration.get_parent(ast)))
            && !NodeUtil::is_loop_structure(ast, declaration.get_grandparent(ast).unwrap()))
            || NodeUtil::is_function_declaration(ast, declaration.get_parent(ast))
    }

    /// Returns whether there is a initial value.
    // port: InlineVariables.InliningBehavior#isValidInitialization
    fn is_valid_initialization(ast: &Ast, initialization: Option<&Reference>) -> bool {
        let Some(initialization) = initialization else {
            return false;
        };
        if initialization.is_declaration(ast) {
            // The reference is a FUNCTION declaration or normal VAR declaration
            // with a value.
            if !NodeUtil::is_function_declaration(ast, initialization.get_parent(ast))
                && !initialization.get_node().has_children(ast)
            {
                return false;
            }
        } else {
            let parent = initialization.get_parent(ast);
            check_state!(
                parent.is_assign(ast)
                    && parent.get_first_child(ast) == Some(initialization.get_node())
            );
        }

        true
    }

    /// Returns true if the reference is a candidate for inlining
    // port: InlineVariables.InliningBehavior#isValidReference
    fn is_valid_reference(ast: &Ast, reference: &Reference) -> bool {
        !reference.is_declaration(ast) && !reference.is_lvalue(ast)
    }
}

/// A `Var` as a key of Java's `LinkedHashMap<Var, ..>` / `LinkedHashMultimap<Var, ..>`: `Var`
/// inherits `ScopedName#equals`/`hashCode` (the name and the scope root node).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct VarKey(JsString, NodeId);

impl VarKey {
    fn of(compiler: &AbstractCompiler, var: VarId) -> Self {
        VarKey(var.get_name(compiler), var.get_scope_root(compiler))
    }
}

/// Java's `BasicBlock` identity comparison (`declaration.getBasicBlock() !=
/// initialization.getBasicBlock()`).
fn same_basic_block(a: Option<&Arc<BasicBlock>>, b: Option<&Arc<BasicBlock>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// Used to initialize fields in a `StandardVarExpert` object.
struct VarExpertInitData {
    v: VarId,
    reference_info: ReferenceCollection,
    is_declared_or_inferred_constant: bool,
    may_be_a_parameter_modified_via_arguments: bool,
}

/// Knows how to analyze a variable to determine whether it should be inlined, how to do the
/// inlining, and whether it's OK to inline aliases of the variable.
struct StandardVarExpert {
    v: VarId,
    reference_info: ReferenceCollection,
    /// `referenceInfo.references.get(0)`, as an index into `reference_info.references`.
    declaration: usize,
    is_declared_or_inferred_constant: bool,
    may_be_a_parameter_modified_via_arguments: bool,

    is_never_assigned: InitiallyUnknown<bool>,
    is_well_defined: InitiallyUnknown<bool>,
    is_assigned_once_in_lifetime: InitiallyUnknown<bool>,
    is_referenced_weakly: InitiallyUnknown<bool>,
    is_well_defined_assigned_once: InitiallyUnknown<bool>,
    /// The initializing `Reference`, as an index into `reference_info.references`.
    initialization_reference: InitiallyUnknown<usize>,
    all_references_are_valid: InitiallyUnknown<bool>,

    /// Rust-only: the outer `InlineVariables#mode` and this expert's arena index.
    mode: Mode,
    index: usize,
}

impl StandardVarExpert {
    // port: InlineVariables.InliningBehavior.StandardVarExpert#StandardVarExpert
    fn new(init_data: VarExpertInitData, mode: Mode, index: usize) -> Self {
        // `references.get(0)`: the first reference is the declaration.
        let _ = &init_data.reference_info.references[0];
        Self {
            v: init_data.v,
            reference_info: init_data.reference_info,
            declaration: 0,
            is_declared_or_inferred_constant: init_data.is_declared_or_inferred_constant,
            may_be_a_parameter_modified_via_arguments: init_data
                .may_be_a_parameter_modified_via_arguments,
            is_never_assigned: InitiallyUnknown::new(),
            is_well_defined: InitiallyUnknown::new(),
            is_assigned_once_in_lifetime: InitiallyUnknown::new(),
            is_referenced_weakly: InitiallyUnknown::new(),
            is_well_defined_assigned_once: InitiallyUnknown::new(),
            initialization_reference: InitiallyUnknown::new(),
            all_references_are_valid: InitiallyUnknown::new(),
            mode,
            index,
        }
    }

    fn references(&self) -> &[Reference] {
        &self.reference_info.references
    }

    /// The reference at `index` of the reference list (Java holds the `Reference` itself).
    fn reference(&self, index: usize) -> &Reference {
        &self.reference_info.references[index]
    }

    // port: InlineVariables.InliningBehavior.StandardVarExpert#isNeverAssigned
    fn is_never_assigned(&mut self, ast: &Ast) -> bool {
        if self.is_never_assigned.is_known() {
            self.is_never_assigned.get_known_value().unwrap()
        } else if self.initialization_reference.is_known_not_null()
            || self.is_assigned_once_in_lifetime.is_known_to_be(Some(true))
        {
            self.is_never_assigned
                .set_known_value_once(Some(false))
                .unwrap()
        } else {
            let value = self.reference_info.is_never_assigned(ast);
            self.is_never_assigned
                .set_known_value_once(Some(value))
                .unwrap()
        }
    }

    // port: InlineVariables.InliningBehavior.StandardVarExpert#isWellDefined
    fn is_well_defined(&mut self, ast: &Ast) -> bool {
        if self.is_well_defined.is_known() {
            self.is_well_defined.get_known_value().unwrap()
        } else {
            let value = self.reference_info.is_well_defined(ast);
            self.is_well_defined
                .set_known_value_once(Some(value))
                .unwrap()
        }
    }

    // port: InlineVariables.InliningBehavior.StandardVarExpert#isAssignedOnceInLifetime
    fn is_assigned_once_in_lifetime(&mut self, compiler: &mut AbstractCompiler) -> bool {
        if self.is_assigned_once_in_lifetime.is_known() {
            self.is_assigned_once_in_lifetime.get_known_value().unwrap()
        } else {
            let value = self.reference_info.is_assigned_once_in_lifetime(compiler);
            self.is_assigned_once_in_lifetime
                .set_known_value_once(Some(value))
                .unwrap()
        }
    }

    // port: InlineVariables.InliningBehavior.StandardVarExpert#isReferencedWeakly
    fn is_referenced_weakly(&mut self, ast: &Ast) -> bool {
        if self.is_referenced_weakly.is_known() {
            self.is_referenced_weakly.get_known_value().unwrap()
        } else {
            let value = self.reference_info.is_referenced_weakly(ast);
            self.is_referenced_weakly
                .set_known_value_once(Some(value))
                .unwrap()
        }
    }

    /// A more efficient way to ask if the variable is both well defined and assigned exactly
    /// once in its lifetime.
    // port: InlineVariables.InliningBehavior.StandardVarExpert#isWellDefinedAssignedOnce
    fn is_well_defined_assigned_once(&mut self, compiler: &mut AbstractCompiler) -> bool {
        if self.is_well_defined_assigned_once.is_known() {
            self.is_well_defined_assigned_once
                .get_known_value()
                .unwrap()
        } else {
            // Check first to see if either is already known to be false before calculating
            // anything.
            let value = !self.is_well_defined.is_known_to_be(Some(false))
                && !self.is_assigned_once_in_lifetime.is_known_to_be(Some(false))
                // isWellDefined is generally less expensive to calculate
                && self.is_well_defined(compiler)
                && self.is_assigned_once_in_lifetime(compiler);
            self.is_well_defined_assigned_once
                .set_known_value_once(Some(value))
                .unwrap()
        }
    }

    // port: InlineVariables.InliningBehavior.StandardVarExpert#getInitialization
    fn get_initialization(&mut self, ast: &Ast) -> Option<usize> {
        if self.initialization_reference.is_known() {
            self.initialization_reference.get_known_value()
        } else {
            let init_ref = if self.is_declared_or_inferred_constant {
                self.reference_info
                    .get_initializing_reference_for_constants(ast)
            } else {
                self.reference_info.get_initializing_reference(ast)
            };
            let init_ref = init_ref.map(|r| {
                self.references()
                    .iter()
                    .position(|candidate| std::ptr::eq(candidate, r))
                    .unwrap()
            });
            self.initialization_reference.set_known_value_once(init_ref)
        }
    }

    // port: InlineVariables.InliningBehavior.StandardVarExpert#hasValidDeclaration
    fn has_valid_declaration(&self, ast: &Ast) -> bool {
        InliningBehavior::is_valid_declaration(ast, self.reference(self.declaration))
    }

    // port: InlineVariables.InliningBehavior.StandardVarExpert#hasValidInitialization
    fn has_valid_initialization(&mut self, ast: &Ast) -> bool {
        let initialization = self.get_initialization(ast);
        InliningBehavior::is_valid_initialization(ast, initialization.map(|i| self.reference(i)))
    }

    // port: InlineVariables.InliningBehavior.StandardVarExpert#allReferencesAreValid
    fn all_references_are_valid(&mut self, ast: &Ast) -> bool {
        if self.all_references_are_valid.is_known() {
            self.all_references_are_valid.get_known_value().unwrap()
        } else {
            let has_valid_declaration = self.has_valid_declaration(ast);
            let has_valid_initialization = self.has_valid_initialization(ast);
            let never_assigned = self.is_never_assigned(ast);
            if has_valid_declaration && (has_valid_initialization || never_assigned) {
                let initialization = self.get_initialization(ast);
                let declaration = self.declaration;
                let has_invalid_reference = self.references().iter().enumerate().any(|(i, r)| {
                    i != declaration
                        && Some(i) != initialization
                        && !InliningBehavior::is_valid_reference(ast, r)
                });
                return self
                    .all_references_are_valid
                    .set_known_value_once(Some(!has_invalid_reference))
                    .unwrap();
            }
            self.all_references_are_valid
                .set_known_value_once(Some(false))
                .unwrap()
        }
    }

    // port: InlineVariables.InliningBehavior.StandardVarExpert#analyze
    fn analyze(&mut self, compiler: &mut AbstractCompiler) -> InlineVarAnalysis {
        if has_no_inline_annotation(compiler, self.v) || self.is_referenced_weakly(compiler) {
            return self.get_negative_inline_var_analysis();
        }
        let initialization = self.get_initialization(compiler);
        let init_value =
            initialization.and_then(|i| self.reference(i).get_assigned_value(compiler));
        let mut initial_value_analysis = InitialValueAnalysis::new(init_value);

        if self.is_declared_or_inferred_constant
            && initial_value_analysis.is_immutable_value(compiler)
            // isAssignedOnceInLifetime() is much more expensive than the other checks
            && self.is_assigned_once_in_lifetime(compiler)
        {
            return self.create_inline_well_defined_var_analysis(init_value);
        }

        if self.mode == Mode::CONSTANTS_ONLY {
            // If we're in constants-only mode, don't run more aggressive
            // inlining heuristics. See InlineConstantsTest.
            return self.get_negative_inline_var_analysis();
        }

        if initial_value_analysis.is_alias(compiler, self.v) {
            return InlineVarAnalysis::VarIsAlias {
                aliased_var: initial_value_analysis
                    .get_aliased_var(compiler, self.v)
                    .unwrap(),
            };
        }
        self.analyze_with_initial_value(
            compiler,
            initialization,
            init_value,
            &mut initial_value_analysis,
        )
    }

    // port: InlineVariables.InliningBehavior.StandardVarExpert#analyzeWithInitialValue
    fn analyze_with_initial_value(
        &mut self,
        compiler: &mut AbstractCompiler,
        initialization: Option<usize>,
        init_value: Option<NodeId>,
        initial_value_analysis: &mut InitialValueAnalysis,
    ) -> InlineVarAnalysis {
        let ref_count = self.references().len();

        // TODO(bradfordcsmith): We could remove the `refCount > 1` here, but:
        //  1. That will require some additional logic to handle stuff like named function
        //     expressions and avoiding the removal of side-effects.
        //  2. RemoveUnusedCode will remove those cases anyway.
        //  3. The unit tests will have to be more verbose to make sure stuff we don't want to be
        //  removed is used.
        if ref_count > 1 && self.all_references_are_valid(compiler) {
            if self.reference_info.is_never_assigned(compiler) {
                return InlineVarAnalysis::Positive {
                    expert: self.index,
                    inliner: Inliner::Undefined,
                };
            }
            if self.is_well_defined(compiler)
                && (initial_value_analysis.is_immutable_value(compiler)
                    || (check_not_null!(init_value).is_this(compiler)
                        && !self.reference_info.is_this_rebound(compiler)))
            {
                // if the variable is referenced more than once, we can only
                // inline it if it's immutable and never defined before referenced.
                return self.create_inline_well_defined_var_analysis(init_value);
            }

            let first_read_ref_index = if Some(self.declaration) == initialization {
                1
            } else {
                2
            };
            let num_read_refs = ref_count - first_read_ref_index;
            if num_read_refs == 0 {
                // The only reference is the initialization.
                // Remove the assignment and the variable declaration.
                return self.create_inline_well_defined_var_analysis(init_value);
            }

            if num_read_refs == 1 {
                // The variable is likely only read once, so we can try some more complex inlining
                // heuristics.
                let single_read_reference = first_read_ref_index;
                let initialization = check_not_null!(initialization);
                if InliningBehavior::can_inline(
                    compiler,
                    self.reference(self.declaration),
                    self.reference(initialization),
                    self.reference(single_read_reference),
                    init_value,
                ) {
                    // A custom inline method is needed for this case.
                    return InlineVarAnalysis::Positive {
                        expert: self.index,
                        inliner: Inliner::Inline {
                            declaration: self.declaration,
                            initialization,
                            single_read_reference,
                        },
                    };
                }
            }
        }

        self.get_negative_inline_var_analysis()
    }

    // port: InlineVariables.InliningBehavior.StandardVarExpert#getNegativeInlineVarAnalysis
    fn get_negative_inline_var_analysis(&self) -> InlineVarAnalysis {
        // If we already know whether it is safe to inline aliases of this variable,
        // use canonical analysis values to save on memory space.
        if self.may_be_a_parameter_modified_via_arguments
            || self.mode == Mode::CONSTANTS_ONLY
            || self
                .is_well_defined_assigned_once
                .is_known_to_be(Some(false))
        {
            return InlineVarAnalysis::NoInlineSelfOrAliases;
        }
        if self
            .is_well_defined_assigned_once
            .is_known_to_be(Some(true))
        {
            return InlineVarAnalysis::NoInlineSelfAliasesOk;
        }

        // Delay calculating safety until we're actually asked.
        InlineVarAnalysis::DelayedNegative { expert: self.index }
    }

    // port: InlineVariables.InliningBehavior.StandardVarExpert#createInlineWellDefinedVarAnalysis
    fn create_inline_well_defined_var_analysis(
        &self,
        init_value: Option<NodeId>,
    ) -> InlineVarAnalysis {
        InlineVarAnalysis::Positive {
            expert: self.index,
            inliner: Inliner::WellDefinedVariable { init_value },
        }
    }

    // port: InlineVariables.InliningBehavior.StandardVarExpert#reanalyzeAfterAliasedVar
    fn reanalyze_after_aliased_var(
        &mut self,
        compiler: &mut AbstractCompiler,
        aliased_var: VarId,
        aliased_var_analysis: InlineVarAnalysis,
        experts: &[RefCell<StandardVarExpert>],
    ) -> InlineVarAnalysis {
        let initialization = self.get_initialization(compiler);
        let init_value =
            initialization.and_then(|i| self.reference(i).get_assigned_value(compiler));
        let mut initial_value_analysis = InitialValueAnalysis::new(init_value);

        if self.is_declared_or_inferred_constant
            && initial_value_analysis.is_immutable_value(compiler)
            // isAssignedOnceInLifetime() is much more expensive than the other checks
            && self.is_assigned_once_in_lifetime(compiler)
        {
            return self.create_inline_well_defined_var_analysis(init_value);
        }

        if initial_value_analysis.is_alias(compiler, self.v)
            && initial_value_analysis
                .get_aliased_var(compiler, self.v)
                .is_some_and(|other| aliased_var.equals(compiler, other))
            && aliased_var_analysis.is_safe_to_inline_aliases(compiler, experts)
            && self.is_well_defined_assigned_once(compiler)
        {
            // The variable we aliased couldn't be inlined itself, or it was an alias for another
            // variable that got inlined in its place.
            // However, it is safe to inline the name assigned to this variable now.
            return self.create_inline_well_defined_var_analysis(init_value);
        }
        self.analyze_with_initial_value(
            compiler,
            initialization,
            init_value,
            &mut initial_value_analysis,
        )
    }

    /// Runs the `Runnable` of a `PositiveInlineVarAnalysis` created by this expert.
    // port: InlineVariables.InliningBehavior.StandardVarExpert#analyzeWithInitialValue (lambda)
    // port: InlineVariables.InliningBehavior.StandardVarExpert#createInlineWellDefinedVarAnalysis (lambda)
    fn run_inliner(&self, compiler: &mut AbstractCompiler, inliner: Inliner) {
        match inliner {
            Inliner::Undefined => {
                // Create a new `undefined` node to inline for a variable that is never
                // initialized.
                let src_location = self.reference(self.declaration).get_node();
                let undefined_node = NodeUtil::new_undefined_node(compiler, Some(src_location));
                InliningBehavior::inline_well_defined_variable(
                    compiler,
                    self.v,
                    Some(undefined_node),
                    self.references(),
                );
            }
            Inliner::WellDefinedVariable { init_value } => {
                InliningBehavior::inline_well_defined_variable(
                    compiler,
                    self.v,
                    init_value,
                    self.references(),
                );
            }
            Inliner::Inline {
                declaration,
                initialization,
                single_read_reference,
            } => InliningBehavior::inline(
                compiler,
                self.reference(declaration),
                self.reference(initialization),
                self.reference(single_read_reference),
            ),
        }
    }
}

/// Information about a value used to initialize a variable.
struct InitialValueAnalysis {
    value: Option<NodeId>,
    is_immutable_value: InitiallyUnknown<bool>,
    aliased_var: InitiallyUnknown<VarId>,
}

impl InitialValueAnalysis {
    // port: InlineVariables.InliningBehavior.StandardVarExpert.InitialValueAnalysis#InitialValueAnalysis
    fn new(value: Option<NodeId>) -> Self {
        Self {
            value,
            is_immutable_value: InitiallyUnknown::new(),
            aliased_var: InitiallyUnknown::new(),
        }
    }

    // port: InlineVariables.InliningBehavior.StandardVarExpert.InitialValueAnalysis#isImmutableValue
    fn is_immutable_value(&mut self, ast: &Ast) -> bool {
        if self.is_immutable_value.is_known() {
            self.is_immutable_value.get_known_value().unwrap()
        } else {
            let value = self
                .value
                .is_some_and(|value| NodeUtil::is_immutable_value(ast, value));
            self.is_immutable_value
                .set_known_value_once(Some(value))
                .unwrap()
        }
    }

    /// `v` is the analyzed variable of the enclosing `StandardVarExpert`.
    // port: InlineVariables.InliningBehavior.StandardVarExpert.InitialValueAnalysis#isAlias
    fn is_alias(&mut self, compiler: &mut AbstractCompiler, v: VarId) -> bool {
        self.get_aliased_var(compiler, v).is_some()
    }

    // port: InlineVariables.InliningBehavior.StandardVarExpert.InitialValueAnalysis#getAliasedVar
    fn get_aliased_var(&mut self, compiler: &mut AbstractCompiler, v: VarId) -> Option<VarId> {
        if self.aliased_var.is_known() {
            self.aliased_var.get_known_value()
        } else {
            if let Some(value) = self.value
                && value.is_name(compiler)
            {
                let aliased_name = value.get_string(compiler);
                // Never consider this variable to be an alias of itself.
                let aliased = if aliased_name == v.get_name(compiler) {
                    None
                } else {
                    v.get_scope(compiler).get_var(compiler, aliased_name)
                };
                return self.aliased_var.set_known_value_once(aliased);
            }
            self.aliased_var.set_known_value_once(None)
        }
    }
}

// port: InlineVariables#hasNoInlineAnnotation
fn has_no_inline_annotation(compiler: &AbstractCompiler, var: VarId) -> bool {
    let js_doc_info = var.get_jsdoc_info(compiler);
    js_doc_info.is_some_and(|info| info.is_no_inline())
}

/// Java's `InitiallyUnknown<T>`; the value is `@Nullable T`, so `None` is Java's `null`.
struct InitiallyUnknown<T> {
    is_known: bool,
    value: Option<T>,
}

impl<T: Clone + PartialEq> InitiallyUnknown<T> {
    fn new() -> Self {
        Self {
            is_known: false,
            value: None,
        }
    }

    // port: InlineVariables.InitiallyUnknown#isKnown
    fn is_known(&self) -> bool {
        self.is_known
    }

    // port: InlineVariables.InitiallyUnknown#isKnownNotNull
    fn is_known_not_null(&self) -> bool {
        self.is_known_not_to_be(None)
    }

    // port: InlineVariables.InitiallyUnknown#isKnownToBe
    fn is_known_to_be(&self, other: Option<T>) -> bool {
        self.is_known && self.value == other
    }

    // port: InlineVariables.InitiallyUnknown#isKnownNotToBe
    fn is_known_not_to_be(&self, other: Option<T>) -> bool {
        self.is_known && self.value != other
    }

    // port: InlineVariables.InitiallyUnknown#setKnownValueOnce
    fn set_known_value_once(&mut self, value: Option<T>) -> Option<T> {
        check_state!(!self.is_known, "already known");
        self.value = value.clone();
        self.is_known = true;
        value
    }

    // port: InlineVariables.InitiallyUnknown#getKnownValue
    fn get_known_value(&self) -> Option<T> {
        check_state!(self.is_known, "not yet known");
        self.value.clone()
    }
}

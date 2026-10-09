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
//   src/com/google/javascript/jscomp/OptimizeParameters.java.

use crate::{
    abstract_compiler::{AbstractCompiler, LifeCycleStage},
    ast_analyzer::AstAnalyzer,
    compiler_pass::CompilerPass,
    diagnostic::log_file::LogFile,
    node_util::NodeUtil,
    optimize_calls::{CallGraphCompilerPass, OptimizeCalls, ReferenceMap},
    scope::ScopeId,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId, SideEffectFlags},
    token::Token,
};

/// Optimize function calls and function signatures.
///
/// - Removes optional parameters if no caller specifies it as argument.
/// - Removes arguments at call site to function that ignores the parameter.
/// - Inline a parameter if the function is always called with that constant.
/// - Removes trailing `undefined` values if the callee doesn't query `arguments` or have a rest
///   parameter
pub struct OptimizeParameters {
    ast_analyzer: AstAnalyzer,
    global_scope: Option<ScopeId>,

    // Allocated & cleaned up by process()
    decisions_log: Option<Box<dyn LogFile>>,
}

/// The java.util.BitSet operations this pass uses (set, set(from, to), get, flip(from, to),
/// cardinality, clone), with Java's semantics over non-negative indexes.
#[derive(Clone, Default)]
struct BitSet {
    words: Vec<u64>,
}

impl BitSet {
    fn new() -> Self {
        Self::default()
    }

    // java.util.BitSet#set(int)
    fn set(&mut self, index: i32) {
        let index = usize::try_from(index).unwrap();
        if self.words.len() <= index / 64 {
            self.words.resize(index / 64 + 1, 0);
        }
        self.words[index / 64] |= 1u64 << (index % 64);
    }

    // java.util.BitSet#set(int,int)
    fn set_range(&mut self, from_index: i32, to_index: i32) {
        for i in from_index..to_index {
            self.set(i);
        }
    }

    // java.util.BitSet#get(int)
    fn get(&self, index: i32) -> bool {
        let index = usize::try_from(index).unwrap();
        self.words
            .get(index / 64)
            .is_some_and(|w| w & (1u64 << (index % 64)) != 0)
    }

    // java.util.BitSet#flip(int,int)
    fn flip(&mut self, from_index: i32, to_index: i32) {
        for i in from_index..to_index {
            let index = usize::try_from(i).unwrap();
            if self.words.len() <= index / 64 {
                self.words.resize(index / 64 + 1, 0);
            }
            self.words[index / 64] ^= 1u64 << (index % 64);
        }
    }

    // java.util.BitSet#cardinality()
    fn cardinality(&self) -> i32 {
        self.words.iter().map(|w| w.count_ones() as i32).sum()
    }
}

// port: OptimizeParameters.UnusedParameterOptimizer
struct UnusedParameterOptimizer<'a> {
    // Java: the outer pass's astAnalyzer, read by this inner class.
    ast_analyzer: &'a AstAnalyzer,
    to_remove: Vec<NodeId>,
    to_replace_with_zero: Vec<NodeId>,
}

impl UnusedParameterOptimizer<'_> {
    /// Attempt to eliminate unused parameters by removing them from both the call sites and the
    /// function definitions.
    ///
    /// An unused first parameter: function foo(a, b) {use(b);} foo(1,2); foo(1,3) becomes
    /// function foo(b) {use(b);} foo(2); foo(3);
    ///
    /// `refs`: A list of references to the symbol (name or property) as vetted by
    /// #analyzeCandidate.
    // port: OptimizeParameters.UnusedParameterOptimizer#tryEliminateUnusedArgs
    fn try_eliminate_unused_args(&mut self, compiler: &mut AbstractCompiler, refs: &[NodeId]) {
        // An argument is unused if its position is greater than the number of declared parameters
        // or if it marked as unused.
        let fns = ReferenceMap::get_function_nodes(compiler, refs);
        check_state!(!fns.is_empty());

        // Examine all function definitions that are ever assigned to the symbol to determine:
        // 1. Which formal parameter positions are used by at least one of the definitions?
        // 2. What is the largest number of formal parameters across all of the functions?
        // 3. The lowest formal parameter position that contains a rest parameter that is used.
        // e.g.
        // foo = function(used0, unused1, ...usedRest) {}
        // foo = function(unused0, ...unusedRest) {}
        // In this case maxFormalsCount = 3, lowestUsedRest = 2, and used = { 0, 2 }
        let mut max_formals_count: i32 = 0;
        let mut lowest_used_rest: i32 = i32::MAX;
        let mut used = BitSet::new();
        for &r#fn in fns.values().flatten() {
            let param_list = NodeUtil::get_function_parameters(compiler, r#fn);
            let mut index: i32 = -1;
            let mut c = param_list.get_first_child(compiler);
            while let Some(cur) = c {
                index += 1;
                if !cur.is_unused_parameter(compiler) {
                    used.set(index);
                    if cur.is_rest(compiler) {
                        lowest_used_rest = lowest_used_rest.min(index);
                        if lowest_used_rest == 0 {
                            // don't bother doing anything more, all the parameters are used.
                            return;
                        }
                    }
                }
                c = cur.get_next(compiler);
            }

            max_formals_count = max_formals_count.max(index + 1);
        }

        // every argument slot after the earliest rest is used
        if lowest_used_rest < max_formals_count {
            used.set_range(lowest_used_rest, max_formals_count);
        }

        let mut unused = used.clone();
        unused.flip(0, max_formals_count);

        // If was a used "rest" declaration, there are no trailing parameters to remove, so
        // bail out now if there are no unused formals.
        if lowest_used_rest < max_formals_count && unused.cardinality() == 0 {
            return;
        }

        // NOTE: RemoveUnusedCode removes any trailing unused formals, so we don't need to
        // do for that case.

        let mut unremovable = used.clone();

        // A parameter is removable from a call-site if the parameter value
        // has no side-effects. If all call-sites can be updated, the
        // parameter can be removed from the function definitions.
        // Regardless, the side-effect free values can still be replaced
        // with a simpler expression.

        // 3 values determine the range of removable parameters:
        // the number of formal parameters, the unused parameters bitset,
        // and the position of a used rest parameter (if any).

        // - If the parameter index is higher >= the "rest", it
        // is not a candidate, otherwise if it is in bitset or
        // the greater than the declared formals.

        // To be removed, the candidate must be side-effect free.

        // If not all candidates can be removed, the removable
        // candidates are still removed if there are no following parameters.
        // If there are following parameters the removable candidates are
        // replaced with a literal zero instead.

        // Build a list of parameters to remove
        let mut lowest_spread: i32 = i32::MAX;
        for &n in refs {
            if ReferenceMap::is_normal_or_opt_chain_call_or_new_target(compiler, n) {
                if OptimizeParameters::has_spread_in_first_call_param(compiler, n) {
                    // Bail: with spread in the first argument of .call, we know nothing about any
                    // of the parameters, so bail out now.
                    return;
                }
                let mut param =
                    ReferenceMap::get_first_argument_for_call_or_new_or_dot_call(compiler, n);
                let mut param_index: i32 = 0;
                while let Some(p) = param {
                    if param_index >= max_formals_count {
                        break;
                    }

                    if p.is_spread(compiler) {
                        lowest_spread = lowest_spread.min(param_index);
                        break;
                    }

                    if !unremovable.get(param_index)
                        && self.ast_analyzer.may_have_side_effects(compiler, p)
                    {
                        unremovable.set(param_index);
                    }

                    param = p.get_next(compiler);
                    param_index += 1;
                }
            }
        }

        // Although, a spread prevents the removal of it and all following used slots, it doesn't
        // prevent the replacement of unused values from other call sites
        if lowest_spread < max_formals_count {
            unremovable.set_range(lowest_spread, max_formals_count);
        }

        // Only remove trailing parameters if there isn't a rest arguments in any of the
        // definition sites.
        let mut remove_all_after_index: i32 = i32::MAX;
        if lowest_used_rest == i32::MAX {
            remove_all_after_index = max_formals_count - 1;
        }

        for &n in refs {
            if ReferenceMap::is_normal_or_opt_chain_call_or_new_target(compiler, n)
                && !OptimizeParameters::already_removed(compiler, n)
            {
                let arg = ReferenceMap::get_first_argument_for_call_or_new_or_dot_call(compiler, n);
                self.record_removal_call_arguments(
                    compiler,
                    lowest_used_rest,
                    remove_all_after_index,
                    &unused,
                    &unremovable,
                    arg,
                    0,
                );
            }
        }

        for &r#fn in fns.values().flatten() {
            let param_list = NodeUtil::get_function_parameters(compiler, r#fn);
            let param = param_list.get_first_child(compiler);
            Self::remove_unused_function_parameters(compiler, &unremovable, param, 0);
        }
    }

    /// Either firstRestIndex will be MAX_VALUE or removeAllAfterIndex will be MAX_VALUE
    ///
    /// - `first_rest_index`: The lowest index that might match a rest parameter
    /// - `remove_all_after_index`: The index of the last known parameter, all arguments with
    ///   higher indexes must be unused, but it won't be tracked in unused
    /// - `unused`: All parameter indexes that are known to be unused by the callee
    /// - `unremovable`: All parameter indexes that are used by a callee, or have defaults with
    ///   side effects
    /// - `arg`: The current argument being considered for removal
    /// - `index`: The index of arg in the argument list
    ///
    /// Returns true if all arguments >= index have been removed.
    // port: OptimizeParameters.UnusedParameterOptimizer#recordRemovalCallArguments
    #[allow(clippy::too_many_arguments)] // Java's parameter list plus the compiler.
    fn record_removal_call_arguments(
        &mut self,
        compiler: &mut AbstractCompiler,
        first_rest_index: i32,
        remove_all_after_index: i32,
        unused: &BitSet,
        unremovable: &BitSet,
        arg: Option<NodeId>,
        index: i32,
    ) -> bool {
        let Some(arg) = arg else {
            // base case
            return true;
        };

        if index > remove_all_after_index {
            return self.remove_arg_and_following(compiler, Some(arg));
        }

        if arg.is_spread(compiler) {
            // There is no meaningful "index" after a spread.
            return false;
        }

        let removed_all_trailing = self.record_removal_call_arguments(
            compiler,
            first_rest_index,
            remove_all_after_index,
            unused,
            unremovable,
            arg.get_next(compiler),
            index + 1,
        );
        if index < first_rest_index {
            // An 'undefined' in trailing position calling a function that doesn't reference
            // `arguments` is removable, since the function will see an `undefined` value there
            // even if we remove it.
            let is_removable_trailing_undefined =
                NodeUtil::is_undefined(compiler, arg) && removed_all_trailing;
            if is_removable_trailing_undefined
                && !self.ast_analyzer.may_have_side_effects(compiler, arg)
            {
                self.to_remove.push(arg);
                return true;
            }

            if unused.get(index) {
                // If isRemovableTrailingUndefined is true, then we know the arg has side effects
                // otherwise we would have already returned above.
                let has_side_effects = is_removable_trailing_undefined
                    || self.ast_analyzer.may_have_side_effects(compiler, arg);
                if !has_side_effects {
                    if unremovable.get(index) {
                        self.record_replace_with_zero(compiler, arg);
                    } else {
                        self.to_remove.push(arg);
                        return removed_all_trailing;
                    }
                } else {
                    // If there is a comma operator with side effects on the first node, the
                    // second node can be optimized knowing that the parameter is not used.
                    self.remove_from_comma_if_no_side_effects(compiler, arg);
                }
            }
        }
        false
    }

    /// Replaces a no side-effect value in a comma operator recursively.
    // port: OptimizeParameters.UnusedParameterOptimizer#removeFromCommaIfNoSideEffects
    fn remove_from_comma_if_no_side_effects(
        &mut self,
        compiler: &mut AbstractCompiler,
        arg: NodeId,
    ) {
        if arg.is_comma(compiler) {
            let second_child = arg.get_second_child(compiler).unwrap();
            if !self
                .ast_analyzer
                .may_have_side_effects(compiler, second_child)
            {
                self.record_replace_with_zero(compiler, second_child);
            } else {
                self.remove_from_comma_if_no_side_effects(compiler, second_child);
            }
        }
    }

    /// Records node to be replaced with zero.
    // port: OptimizeParameters.UnusedParameterOptimizer#recordReplaceWithZero
    fn record_replace_with_zero(&mut self, compiler: &AbstractCompiler, arg: NodeId) {
        if !arg.is_number(compiler) || arg.get_double(compiler) != 0.0 {
            self.to_replace_with_zero.push(arg);
        }
    }

    /// Returns true if all following args were removed
    // port: OptimizeParameters.UnusedParameterOptimizer#removeArgAndFollowing
    fn remove_arg_and_following(
        &mut self,
        compiler: &mut AbstractCompiler,
        arg: Option<NodeId>,
    ) -> bool {
        if let Some(arg) = arg {
            let next = arg.get_next(compiler);
            let removed_all = self.remove_arg_and_following(compiler, next);
            if !self.ast_analyzer.may_have_side_effects(compiler, arg) {
                self.to_remove.push(arg);
                return removed_all;
            }
            return false;
        }
        true
    }

    // port: OptimizeParameters.UnusedParameterOptimizer#removeUnusedFunctionParameters
    fn remove_unused_function_parameters(
        compiler: &mut AbstractCompiler,
        unremovable: &BitSet,
        param: Option<NodeId>,
        index: i32,
    ) {
        if let Some(param) = param {
            let next = param.get_next(compiler);
            Self::remove_unused_function_parameters(compiler, unremovable, next, index + 1);
            if !unremovable.get(index) {
                check_state!(param.is_name(compiler)); // update for ES6
                // params are not otherwise referenceable.
                compiler.report_change_to_enclosing_scope(param);
                param.detach(compiler);
            }
        }
    }

    /// Applies optimizations to all previously marked nodes.
    // port: OptimizeParameters.UnusedParameterOptimizer#applyChanges
    fn apply_changes(&mut self, compiler: &mut AbstractCompiler) {
        for &n in &self.to_remove {
            // Don't remove any nodes twice since doing so would violate change reporting
            // constraints.
            if OptimizeParameters::already_removed(compiler, n) {
                continue;
            }
            compiler.report_change_to_enclosing_scope(n);
            n.detach(compiler);
            NodeUtil::mark_functions_deleted(compiler, n);
        }
        for &n in &self.to_replace_with_zero {
            check_state!(!n.is_number(compiler) || n.get_double(compiler) != 0.0);
            // Don't remove any nodes twice since doing so would violate change reporting
            // constraints.
            if OptimizeParameters::already_removed(compiler, n) {
                continue;
            }
            compiler.report_change_to_enclosing_scope(n);
            let zero = IR::number(compiler, 0.0).srcref(compiler, n);
            n.replace_with(compiler, zero);
            NodeUtil::mark_functions_deleted(compiler, n);
        }
    }
}

/// Results of analyzing a candidate function's definition and references.
// port: OptimizeParameters.CandidateAnalysis
struct CandidateAnalysis {
    tagged_template_literals: Vec<NodeId>,
    is_safe_to_optimize: bool,
}

impl CandidateAnalysis {
    // port: OptimizeParameters.CandidateAnalysis#CandidateAnalysis
    fn new(builder: CandidateAnalysisBuilder) -> Self {
        Self {
            tagged_template_literals: builder.tagged_template_literals,
            is_safe_to_optimize: builder.is_safe_to_optimize,
        }
    }

    // port: OptimizeParameters.CandidateAnalysis#shouldConvertTaggedTemplateLiterals
    fn should_convert_tagged_template_literals(&self) -> bool {
        !self.tagged_template_literals.is_empty() && self.is_safe_to_optimize
    }

    // port: OptimizeParameters.CandidateAnalysis#convertTaggedTemplateLiterals
    fn convert_tagged_template_literals(&mut self, compiler: &mut AbstractCompiler) {
        for &tagged_template_literal in &self.tagged_template_literals {
            OptimizeParameters::convert_tagged_template_literal_to_normal_call(
                compiler,
                tagged_template_literal,
            );
        }
        self.tagged_template_literals.clear();
    }

    // port: OptimizeParameters.CandidateAnalysis#isSafeToOptimize
    fn is_safe_to_optimize(&self) -> bool {
        self.is_safe_to_optimize && self.tagged_template_literals.is_empty()
    }
}

/// Builder class for CandidateAnalysis.
// port: OptimizeParameters.CandidateAnalysisBuilder
#[derive(Default)]
struct CandidateAnalysisBuilder {
    tagged_template_literals: Vec<NodeId>,
    is_safe_to_optimize: bool,
}

impl CandidateAnalysisBuilder {
    // port: OptimizeParameters.CandidateAnalysisBuilder#setIsSafeToOptimize
    fn set_is_safe_to_optimize(mut self, is_safe_to_optimize: bool) -> Self {
        self.is_safe_to_optimize = is_safe_to_optimize;
        self
    }

    // port: OptimizeParameters.CandidateAnalysisBuilder#addTaggedTemplateLiteral
    fn add_tagged_template_literal(&mut self, ttl_node: NodeId) -> &mut Self {
        self.tagged_template_literals.push(ttl_node);
        self
    }

    // port: OptimizeParameters.CandidateAnalysisBuilder#build
    fn build(self) -> CandidateAnalysis {
        CandidateAnalysis::new(self)
    }
}

/// Simple container class that keeps tracks of a parameter and whether it should be removed.
// port: OptimizeParameters.Parameter
struct Parameter {
    arg: NodeId,
    should_remove: bool,
    has_side_effects: bool,
    can_be_side_effected: bool,
    may_be_undefined: bool,
}

impl Parameter {
    // port: OptimizeParameters.Parameter#Parameter
    fn new(arg: NodeId, should_remove: bool) -> Self {
        Self {
            should_remove,
            arg,
            has_side_effects: false,
            can_be_side_effected: false,
            may_be_undefined: false,
        }
    }

    // port: OptimizeParameters.Parameter#getArg
    fn get_arg(&self) -> NodeId {
        self.arg
    }

    // port: OptimizeParameters.Parameter#shouldRemove
    fn should_remove(&self) -> bool {
        self.should_remove
    }

    // port: OptimizeParameters.Parameter#setShouldRemove
    fn set_should_remove(&mut self, value: bool) {
        self.should_remove = value;
    }

    // port: OptimizeParameters.Parameter#setHasSideEffects
    fn set_has_side_effects(&mut self, has_side_effects: bool) {
        self.has_side_effects = has_side_effects;
    }

    // port: OptimizeParameters.Parameter#hasSideEffects
    fn has_side_effects(&self) -> bool {
        self.has_side_effects
    }

    // port: OptimizeParameters.Parameter#setCanBeSideEffected
    fn set_can_be_side_effected(&mut self, can_be_side_effected: bool) {
        self.can_be_side_effected = can_be_side_effected;
    }

    // port: OptimizeParameters.Parameter#canBeSideEffected
    fn can_be_side_effected(&self) -> bool {
        self.can_be_side_effected
    }

    // port: OptimizeParameters.Parameter#setMayBeUndefined
    fn set_may_be_undefined(&mut self, may_be_undefined: bool) {
        self.may_be_undefined = may_be_undefined;
    }
}

impl OptimizeParameters {
    // port: OptimizeParameters#OptimizeParameters
    pub fn new(compiler: &AbstractCompiler) -> Self {
        Self {
            ast_analyzer: compiler.get_ast_analyzer(),
            global_scope: None,
            decisions_log: None,
        }
    }

    // Java: decisionsLog.log(format, args). LogFile#log takes the message lazily, which is what
    // Java's `if (decisionsLog.isLogging())` guards achieve.
    fn log(&mut self, message: impl FnOnce() -> String) {
        let mut message = Some(message);
        self.decisions_log
            .as_mut()
            .unwrap()
            .log(&mut || message.take().unwrap()());
    }

    // port: OptimizeParameters#alreadyRemoved
    fn already_removed(ast: &Ast, mut n: NodeId) -> bool {
        let mut parent = n.get_parent(ast);
        while let Some(p) = parent {
            n = p;
            parent = n.get_parent(ast);
        }
        !n.is_root(ast)
    }

    // port: OptimizeParameters#analyzeCandidateName
    fn analyze_candidate_name(
        &mut self,
        compiler: &mut AbstractCompiler,
        key: &JsString,
        refs: &[NodeId],
    ) -> CandidateAnalysis {
        self.analyze_candidate(compiler, "name", key, refs)
    }

    // port: OptimizeParameters#analyzeCandidateProperty
    fn analyze_candidate_property(
        &mut self,
        compiler: &mut AbstractCompiler,
        key: &JsString,
        refs: &[NodeId],
    ) -> CandidateAnalysis {
        self.analyze_candidate(compiler, "property", key, refs)
    }

    /// A function is a candidate for parameter optimization if:
    ///
    /// - if all call sites are known (no aliasing)
    /// - if all definition sites are known (the possible values are known functions)
    /// - there is at least one definition
    /// - none of the calls are tagged template literals, OR the special first argument is unused
    // port: OptimizeParameters#analyzeCandidate
    fn analyze_candidate(
        &mut self,
        compiler: &mut AbstractCompiler,
        ref_kind: &str,
        key: &JsString,
        refs: &[NodeId],
    ) -> CandidateAnalysis {
        let mut analysis_builder = CandidateAnalysisBuilder::default();
        if !OptimizeCalls::may_be_optimizable_name(compiler, key) {
            self.log(|| format!("{ref_kind}\t{key}\tnot an optimizable name"));
            return analysis_builder.set_is_safe_to_optimize(false).build();
        }
        let mut definitions: Vec<NodeId> = Vec::new();
        let mut seen_candidate_use = false;
        for &n in refs {
            // TODO(johnlenz): Determine what to do about ".constructor" references.
            // Currently classes that are super classes or have superclasses aren't optimized
            //
            // if (parent.isCall() && n != parent.getFirstChild() && isClassDefiningCall(parent)) {
            //   continue;
            // } else

            if ReferenceMap::is_normal_or_opt_chain_call_or_new_target(compiler, n) {
                // TODO(johnlenz): filter .apply when we support it
                seen_candidate_use = true;
            } else if n
                .get_parent(compiler)
                .unwrap()
                .is_tagged_template_lit(compiler)
            {
                analysis_builder.add_tagged_template_literal(n.get_parent(compiler).unwrap());
            } else if self.is_candidate_definition(compiler, n) {
                definitions.push(n);
            } else if !OptimizeCalls::is_allowed_reference(compiler, n) {
                // avoid build location string when not logging
                self.log(|| {
                    format!(
                        "{ref_kind}\t{key}\tnot an allowed reference: {}",
                        n.get_location(compiler)
                    )
                });
                // TODO(johnlenz): allow extends clauses.
                return analysis_builder.set_is_safe_to_optimize(false).build();
            }
        }
        if definitions.is_empty() {
            self.log(|| format!("{ref_kind}\t{key}\tno definition found"));
            return analysis_builder.set_is_safe_to_optimize(false).build();
        }
        if !analysis_builder.tagged_template_literals.is_empty() {
            let function_nodes: Vec<NodeId> =
                ReferenceMap::get_function_nodes(compiler, &definitions)
                    .values()
                    .flatten()
                    .copied()
                    .collect();

            for function_node in function_nodes {
                let first_param = NodeUtil::get_function_parameters(compiler, function_node)
                    .get_first_child(compiler);
                if first_param.is_some_and(|first_param| !first_param.is_unused_parameter(compiler))
                {
                    self.log(|| {
                        format!(
                            "{ref_kind}\t{key}\twill not optimize parameters for tagged template literals"
                        )
                    });
                    return analysis_builder.set_is_safe_to_optimize(false).build();
                }
            }
            self.log(|| {
                format!(
                    "{ref_kind}\t{key}\t{}",
                    "first param is unused, so we will convert tagged template literals to normal calls"
                )
            });
            seen_candidate_use = true;
        }

        if !seen_candidate_use {
            self.log(|| format!("{ref_kind}\t{key}\tno usage found"));
            return analysis_builder.set_is_safe_to_optimize(false).build();
        }
        analysis_builder.set_is_safe_to_optimize(true).build()
    }

    /// Converts "tagFunction`template ${1} literal ${2}`" to `tagFunction([], 1, 2)`.
    // port: OptimizeParameters#convertTaggedTemplateLiteralToNormalCall
    fn convert_tagged_template_literal_to_normal_call(
        compiler: &mut AbstractCompiler,
        ttl_node: NodeId,
    ) {
        let callee = check_not_null!(ttl_node.get_first_child(compiler));
        let template_literal = callee.get_next(compiler).unwrap();
        let first_arg = IR::arraylit(compiler, &[]).srcref(compiler, template_literal);
        callee.detach(compiler);
        let call_node =
            NodeUtil::new_call_node(compiler, callee, &[first_arg]).srcref(compiler, ttl_node);
        let mut tl_child = template_literal.get_first_child(compiler);
        while let Some(child) = tl_child {
            if child.is_template_lit_sub(compiler) {
                let arg = child.get_only_child(compiler);
                arg.detach(compiler);
                call_node.add_child_to_back(compiler, arg);
            }
            tl_child = child.get_next(compiler);
        }
        ttl_node.replace_with(compiler, call_node);
        compiler.report_change_to_enclosing_scope(call_node);
    }

    // port: OptimizeParameters#isCandidateDefinition
    #[allow(clippy::if_same_then_else)] // Retain Java control flow.
    fn is_candidate_definition(&self, compiler: &mut AbstractCompiler, n: NodeId) -> bool {
        let parent = n.get_parent(compiler).unwrap();

        let function_expr: NodeId;
        if parent.is_function(compiler) && NodeUtil::is_function_declaration(compiler, parent) {
            function_expr = parent;
        } else if ReferenceMap::is_simple_assignment_target(compiler, n) {
            function_expr = parent.get_last_child(compiler).unwrap();
        } else if n.is_name(compiler) && n.has_children(compiler) {
            function_expr = n.get_first_child(compiler).unwrap();
        } else if Self::is_class_member_definition(compiler, n) {
            function_expr = n.get_first_child(compiler).unwrap();
        } else if parent.is_class(compiler) && n.is_first_child_of(compiler, Some(parent)) {
            // allDefinitionsAreCandidateFunctions() understands classes and will check for
            // candidacy correctly.
            function_expr = parent;
        } else {
            return false; // Couldn't find a function.
        }

        self.all_definitions_are_candidate_functions(compiler, function_expr)
    }

    // port: OptimizeParameters#isClassMemberDefinition
    fn is_class_member_definition(ast: &Ast, n: NodeId) -> bool {
        n.is_member_function_def(ast) && n.get_parent(ast).unwrap().is_class_members(ast)
    }

    /// When class fields are set to RHS with side effects, the order of execution may be moved
    /// into incorrect order if optimized in this pass. Example:
    ///
    /// class C { field2 = alert(2);", constructor(a) {", use(a); } } var c = new C(alert(1));
    ///
    /// would optimize to
    ///
    /// class C { field2 = alert(2);", constructor(a) {", var a = alert(1) use(a); } } var c = new
    /// C(alert(1));
    ///
    /// which would mean alert(2) gets executed before alert(1) - this behavior is incorrect.
    ///
    /// So, if there are any RHS side effects, we skip the optimization.
    // port: OptimizeParameters#classContainsClassFieldWithRHSSideEffects
    fn class_contains_class_field_with_rhs_side_effects(
        &self,
        compiler: &mut AbstractCompiler,
        class_node: NodeId,
    ) -> bool {
        let class_members_node = NodeUtil::get_class_members(compiler, class_node);
        let mut child = class_members_node.get_first_child(compiler);
        while let Some(c) = child {
            if c.is_member_field_def(compiler)
                && !c.is_static_member(compiler)
                && c.has_children(compiler)
                && self
                    .ast_analyzer
                    .may_have_side_effects(compiler, c.get_first_child(compiler).unwrap())
            {
                return true;
            }
            child = c.get_next(compiler);
        }
        false
    }

    // port: OptimizeParameters#allDefinitionsAreCandidateFunctions
    fn all_definitions_are_candidate_functions(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> bool {
        match n.get_token(compiler) {
            Token::CLASS => {
                if NodeUtil::is_named_class_expression(compiler, n) {
                    // name creates an alias, making it hard to be sure we've seen all calls
                    false
                } else {
                    // `class NameNode {`
                    // find the constructor
                    let constructor_member_function_def =
                        NodeUtil::get_es6_class_constructor_member_function_def(compiler, n);
                    match constructor_member_function_def {
                        None => {
                            // unable to find the constructor
                            // TODO(bradfordcsmith): Ideally we should find the parent class
                            // constructor.
                            false
                        }
                        Some(_)
                            if self
                                .class_contains_class_field_with_rhs_side_effects(compiler, n) =>
                        {
                            false
                        }
                        Some(constructor_member_function_def) => {
                            let function_node =
                                constructor_member_function_def.get_only_child(compiler);
                            // "arguments" can refer to all parameters or their count.
                            !NodeUtil::does_function_reference_own_arguments_object(
                                compiler,
                                function_node,
                            )
                                // In `function f(a, b = a) { ... }` it's very difficult to
                                // determine if `a` is movable.
                                && !Self::may_reference_param_before_body(compiler, function_node)
                                // `/** @usedViaDotConstructor */ function f(a) {...}` means back
                                // off.
                                && !Self::is_used_via_dot_constructor(compiler, function_node)
                        }
                    }
                }
            }
            Token::FUNCTION => {
                // Named function expression can refer to themselves,
                !NodeUtil::is_named_function_expression(compiler, n)
                    // "arguments" can refer to all parameters or their count.
                    && !NodeUtil::does_function_reference_own_arguments_object(compiler, n)
                    // In `function f(a, b = a) { ... }` it's very difficult to determine if `a`
                    // is movable.
                    && !Self::may_reference_param_before_body(compiler, n)
                    // `/** @usedViaDotConstructor */ function f(a) {...}` means back off.
                    && !Self::is_used_via_dot_constructor(compiler, n)
            }
            Token::CAST | Token::COMMA => {
                let last = n.get_last_child(compiler).unwrap();
                self.all_definitions_are_candidate_functions(compiler, last)
            }
            Token::HOOK => {
                let second = n.get_second_child(compiler).unwrap();
                let last = n.get_last_child(compiler).unwrap();
                self.all_definitions_are_candidate_functions(compiler, second)
                    && self.all_definitions_are_candidate_functions(compiler, last)
            }
            Token::OR | Token::AND | Token::COALESCE => {
                let first = n.get_first_child(compiler).unwrap();
                let last = n.get_last_child(compiler).unwrap();
                self.all_definitions_are_candidate_functions(compiler, first)
                    && self.all_definitions_are_candidate_functions(compiler, last)
            }
            _ => false,
        }
    }

    /// Returns true if the function is annotated with @usedViaDotConstructor
    ///
    /// In this case we just back off on all parameter optimizations since we cannot detect which
    /// parameters are used via calls through `.constructor`.
    // port: OptimizeParameters#isUsedViaDotConstructor
    fn is_used_via_dot_constructor(ast: &Ast, function: NodeId) -> bool {
        let doc_info = NodeUtil::get_best_jsdoc_info(ast, function);
        doc_info.is_some_and(|doc_info| doc_info.is_used_via_dot_constructor())
    }

    /// Does the function use one of its parameters in code before the body?
    ///
    /// Having that property is risky for inlining. Example `function f(a, b = a) { ... }`. We
    /// can't trivially inline `a` in this case because the inlined var can't precede `b = a`.
    ///
    /// This case is very rare so for now we just back-off completely. If it becomes more common,
    /// we can tighten the detection of problematic cases, or back-off only for the dangerous
    /// params.
    // port: OptimizeParameters#mayReferenceParamBeforeBody
    fn may_reference_param_before_body(compiler: &mut AbstractCompiler, function: NodeId) -> bool {
        let param_list = function.get_second_child(compiler).unwrap();
        if !param_list.has_children(compiler) {
            return false; // Fast path; there can't possibly be back-refs.
        }

        // Java: ArrayListMultimap<String, Node>; only "is any group's member an lvalue" is read,
        // so the group order does not matter.
        let mut names_by_names: IndexMap<JsString, Vec<NodeId>> = IndexMap::<_, _>::default();
        NodeUtil::visit_post_order(compiler, param_list, &mut |ast: &mut Ast, n: NodeId| {
            if n.is_name(ast) {
                names_by_names.entry(n.get_string(ast)).or_default().push(n);
            }
        });

        for names in names_by_names.values() {
            if names.len() == 1 {
                continue; // There can't be back-refs if there's only one ref.
            }

            for &name in names {
                if NodeUtil::is_l_value(compiler, name) {
                    return true; // One ref is a definition, so the rest of might be back-refs.
                }
            }
        }

        false
    }

    /// Removes any optional parameters if no callers specifies it as an argument.
    // port: OptimizeParameters#tryEliminateOptionalArgs
    fn try_eliminate_optional_args(&mut self, compiler: &mut AbstractCompiler, refs: &[NodeId]) {
        // Count the maximum number of arguments passed into this function all
        // all points of the program.
        let mut max_args: i32 = -1;

        for &n in refs {
            if ReferenceMap::is_normal_or_opt_chain_call_or_new_target(compiler, n) {
                if Self::has_spread_in_first_call_param(compiler, n) {
                    // Bail: with spread we must assume all parameters are used, don't waste
                    // any more time.
                    return;
                }
                let mut num_args: i32 = 0;
                let first_arg =
                    ReferenceMap::get_first_argument_for_call_or_new_or_dot_call(compiler, n);
                let mut c = first_arg;
                while let Some(cur) = c {
                    num_args += 1;
                    if cur.is_spread(compiler) {
                        // Bail: with spread we must assume all parameters are used, don't waste
                        // any more time.
                        return;
                    }
                    c = cur.get_next(compiler);
                }

                if num_args > max_args {
                    max_args = num_args;
                }
            }
        }

        let fns = ReferenceMap::get_function_nodes(compiler, refs);
        for &r#fn in fns.values().flatten() {
            self.eliminate_params_after_index(compiler, r#fn, max_args);
        }
    }

    /// Eliminate parameters if they are always constant.
    ///
    /// function foo(a, b) {...} foo(1,2); foo(1,3) becomes function foo(b) { var a = 1 ... }
    /// foo(2); foo(3);
    ///
    /// `refs`: A list of references to the symbol (name or property) as vetted by
    /// #analyzeCandidate.
    // port: OptimizeParameters#tryEliminateConstantArgs
    fn try_eliminate_constant_args(&mut self, compiler: &mut AbstractCompiler, refs: &[NodeId]) {
        let Some(mut parameters) = self.find_fixed_arguments(compiler, refs) else {
            return;
        };

        let fns = ReferenceMap::get_function_nodes(compiler, refs);
        // ImmutableListMultimap#size(): the number of key-value pairs.
        if fns.values().map(Vec::len).sum::<usize>() > 1 {
            // TODO(johnlenz): support moving simple constants.
            // This requires cloning the tree and avoiding adding additional calls/definitions
            // that will invalidate the reference map
            return;
        }

        // Only one definition is currently supported.
        // Iterables.getOnlyElement: at most one value remains after the size check above.
        let r#fn = *fns.values().flatten().next().unwrap();

        let continue_looking = Self::adjust_for_constraints(compiler, r#fn, &mut parameters);
        if !continue_looking {
            return;
        }

        // Found something to do, move the values from the call sites to the function definitions.
        for &n in refs {
            if !Self::already_removed(compiler, n)
                && ReferenceMap::is_normal_or_opt_chain_call_or_new_target(compiler, n)
            {
                Self::optimize_call_site(compiler, &parameters, n);
            }
        }

        self.optimize_function_definition(compiler, &parameters, r#fn);
    }

    /// `refs`: A list of references to the symbol (name or property) as vetted by
    /// #analyzeCandidate.
    ///
    /// Returns a list of Parameter objects, in the declaration order, which represent potentially
    /// movable values fixed values from all call sites or null if there are no candidate values.
    // port: OptimizeParameters#findFixedArguments
    fn find_fixed_arguments(
        &mut self,
        compiler: &mut AbstractCompiler,
        refs: &[NodeId],
    ) -> Option<Vec<Parameter>> {
        let mut parameters: Vec<Parameter> = Vec::new();
        let mut first_call = true;

        // Build a list of parameters to remove
        let mut continue_looking = false;
        for &n in refs {
            if ReferenceMap::is_normal_or_opt_chain_call_or_new_target(compiler, n) {
                // Normally, we ignore the first parameter to a .call expression (the 'this' value)
                // but if it is a spread, we know nothing about any of the parameters, so bail out
                // now.
                if Self::has_spread_in_first_call_param(compiler, n) {
                    continue_looking = false;
                    break;
                }
                let cur = ReferenceMap::get_first_argument_for_call_or_new_or_dot_call(compiler, n);
                if first_call {
                    // Use the first call to construct a list of parameter values of the
                    // function.
                    continue_looking =
                        self.build_initial_parameter_list(compiler, &mut parameters, cur);
                    first_call = false;
                } else {
                    // All the rest must match
                    continue_looking = self.find_fixed_parameters(compiler, &mut parameters, cur);
                }
                if !continue_looking {
                    break;
                }
            }
        }

        if continue_looking {
            Some(parameters)
        } else {
            None
        }
    }

    /// Returns whether the target call is a `.call` expression whose first argument (the `this`
    /// argument) is a spread element (e.g., `foo.call(...arr)`).
    ///
    /// ReferenceMap#getFirstArgumentForCallOrNewOrDotCall skips the first argument to `.call`
    /// (treating it as the `this` value) and callers inspect all subsequent arguments, which
    /// correctly handles spread arguments in normal calls like `foo(p1, ...args)` and `.call`
    /// invocations with an explicit this argument like `foo.call(thisArg, p1, ...args)`. However,
    /// if the first argument to `.call` itself is a spread (e.g., `foo.call(...arr)`), it spreads
    /// across both the `this` value and subsequent formal parameter positions, so we cannot safely
    /// optimize any parameters.
    // port: OptimizeParameters#hasSpreadInFirstCallParam
    fn has_spread_in_first_call_param(compiler: &AbstractCompiler, n: NodeId) -> bool {
        let call = ReferenceMap::get_call_or_new_node_for_target(compiler, n);
        if NodeUtil::is_function_object_call(compiler, call) {
            let first_dot_call_param = call.get_second_child(compiler);
            return first_dot_call_param.is_some_and(|p| p.is_spread(compiler));
        }
        false
    }

    /// Adjust the provided Parameter objects value created by #findFixedArguments for "rest"
    /// value and side-effects which might prevent the motion of the parameters from the call sites
    /// to the function body.
    ///
    /// `parameters`: A list of Parameter objects summarizing all the call-sites and whether any of
    /// the parameters are fixed.
    ///
    /// Returns whether there are any movable parameters.
    // port: OptimizeParameters#adjustForConstraints
    fn adjust_for_constraints(ast: &Ast, r#fn: NodeId, parameters: &mut [Parameter]) -> bool {
        let info = NodeUtil::get_best_jsdoc_info(ast, r#fn);
        if info.is_some_and(|info| info.is_no_inline()) {
            return false;
        }

        let param_list = NodeUtil::get_function_parameters(ast, r#fn);
        let last_formal = param_list.get_last_child(ast);
        let mut rest_index: i32 = i32::MAX;
        let mut last_non_rest_formal: i32 = param_list.get_child_count(ast) - 1;
        let mut formal = last_formal;
        if let Some(last) = last_formal
            && last.is_rest(ast)
        {
            rest_index = last_non_rest_formal;
            last_non_rest_formal -= 1;
            formal = formal.unwrap().get_previous(ast);
        }

        // A parameter with side-effects can move if there are no following parameters
        // that can be affected.

        // A parameter can be moved if it can't be side-effected (a literal),
        // or there are no following side-effects, that aren't moved.

        let mut any_movable = false;
        let mut seen_unmovable_side_effects = false;
        let mut seen_unmoveable_side_effected = false;
        let mut all_rest_value_removable = true;
        let size = parameters.len() as i32;
        let mut i = size - 1;
        while i >= 0 {
            let current = &mut parameters[i as usize];

            // back-off for default values whose default value maybe needed.
            // TODO(johnlenz): handle used default
            if i <= last_non_rest_formal {
                let f = formal.unwrap();
                if f.is_default_value(ast) && current.may_be_undefined {
                    current.should_remove = false;
                }
                formal = f.get_previous(ast);
            }

            // Preserve side-effect ordering, don't move this parameter if:
            // * the current parameter has side-effects and a following
            // parameters that will not be move can be effected.
            // * the current parameter can be effected and a following
            // parameter that will not be moved has side-effects

            if current.should_remove
                && ((seen_unmovable_side_effects && current.can_be_side_effected())
                    || (seen_unmoveable_side_effected && current.has_side_effects()))
            {
                current.should_remove = false;
            }

            // If any values that are part of the rest cannot be moved to the function body,
            // then all the rest values must remain at the callsite.
            if i >= rest_index {
                if all_rest_value_removable {
                    if !current.should_remove {
                        any_movable = false;
                        all_rest_value_removable = false;
                        // revisit the trailing params and remark them now that we know they are
                        // unremovable.
                        for j in (i + 1)..size {
                            let p = &mut parameters[j as usize];
                            p.should_remove = false;
                            if p.can_be_side_effected {
                                seen_unmoveable_side_effected = true;
                            }
                            if p.has_side_effects {
                                seen_unmovable_side_effects = true;
                            }
                        }
                    }
                } else {
                    current.should_remove = false;
                }
            }

            let current = &parameters[i as usize];
            if current.should_remove {
                any_movable = true;
            } else {
                if current.can_be_side_effected {
                    seen_unmoveable_side_effected = true;
                }

                if current.has_side_effects {
                    seen_unmovable_side_effects = true;
                }
            }
            i -= 1;
        }
        any_movable
    }

    /// Determine which parameters use the same expression.
    ///
    /// Returns whether any parameter was found that can be updated.
    // port: OptimizeParameters#findFixedParameters
    fn find_fixed_parameters(
        &self,
        compiler: &mut AbstractCompiler,
        parameters: &mut Vec<Parameter>,
        mut cur: Option<NodeId>,
    ) -> bool {
        let mut any_movable = false;
        let mut index: usize = 0;
        while let Some(c) = cur {
            if index >= parameters.len() {
                let mut p = Parameter::new(c, false);
                self.set_parameter_side_effect_info(compiler, &mut p, c);
                parameters.push(p);
            } else {
                let p = &mut parameters[index];
                if p.should_remove() {
                    let value = p.get_arg();
                    if !c.is_equivalent_to(compiler, value) {
                        p.set_should_remove(false);
                    } else {
                        any_movable = true;
                    }
                }
                p.set_has_side_effects(
                    p.has_side_effects() || self.ast_analyzer.may_have_side_effects(compiler, c),
                );
                p.set_can_be_side_effected(
                    p.can_be_side_effected() || NodeUtil::can_be_side_effected(compiler, c),
                );
            }
            // Back off optimizing arguments following spread
            if c.is_spread(compiler) {
                break;
            }
            cur = c.get_next(compiler);
            index += 1;
        }

        while index < parameters.len() {
            parameters[index].set_should_remove(false);
            index += 1;
        }

        any_movable
    }

    /// Returns whether any parameter was movable.
    // port: OptimizeParameters#buildInitialParameterList
    fn build_initial_parameter_list(
        &self,
        compiler: &mut AbstractCompiler,
        parameters: &mut Vec<Parameter>,
        mut cur: Option<NodeId>,
    ) -> bool {
        let mut any_movable = false;
        while let Some(c) = cur {
            let movable = Self::is_movable_value(compiler, c, self.global_scope.unwrap());
            let mut p = Parameter::new(c, movable);
            self.set_parameter_side_effect_info(compiler, &mut p, c);
            parameters.push(p);
            if movable {
                any_movable = true;
            }
            // Back off optimizing arguments following spread
            if c.is_spread(compiler) {
                break;
            }
            cur = c.get_next(compiler);
        }
        any_movable
    }

    // port: OptimizeParameters#setParameterSideEffectInfo
    fn set_parameter_side_effect_info(
        &self,
        compiler: &mut AbstractCompiler,
        p: &mut Parameter,
        value: NodeId,
    ) {
        p.set_has_side_effects(self.ast_analyzer.may_have_side_effects(compiler, value));
        p.set_can_be_side_effected(NodeUtil::can_be_side_effected(compiler, value));
        if !value.is_spread(compiler) {
            p.set_may_be_undefined(NodeUtil::may_be_undefined(compiler, value));
        }
    }

    /// Returns whether the expression can be safely moved to another function in another scope.
    // port: OptimizeParameters#isMovableValue
    fn is_movable_value(compiler: &mut AbstractCompiler, n: NodeId, global_scope: ScopeId) -> bool {
        // Things that can change value or are inaccessible can't be moved, these
        // are "this", "arguments", local names, and functions that capture local
        // values.
        match n.get_token(compiler) {
            Token::AWAIT
            | Token::YIELD
            | Token::THIS
            | Token::SUPER
            | Token::ITER_SPREAD
            | Token::OBJECT_SPREAD
            | Token::NEW_TARGET
            | Token::IMPORT_META => {
                return false;
            }
            Token::FUNCTION => {
                // Don't move function closures.
                // TODO(johnlenz): Closure that only contain global reference can be
                // moved.
                return false;
            }
            Token::NAME => {
                if n.get_string_ref(compiler) == "arguments" {
                    return false;
                } else {
                    // If it isn't in global scope, then it is in local scope.  This logic depends
                    // on "Normalize" creating distinct local names.
                    let name = n.get_string(compiler);
                    let v = global_scope.get_var(compiler, name);
                    if v.is_none() {
                        return false;
                    }
                }
            }
            _ => {}
        }

        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            if !Self::is_movable_value(compiler, cur, global_scope) {
                return false;
            }
            c = cur.get_next(compiler);
        }
        true
    }

    // port: OptimizeParameters#optimizeFunctionDefinition
    fn optimize_function_definition(
        &mut self,
        compiler: &mut AbstractCompiler,
        parameters: &[Parameter],
        r#fn: NodeId,
    ) {
        let param_list = NodeUtil::get_function_parameters(compiler, r#fn);
        let maybe_rest = param_list.get_last_child(compiler);
        let size = parameters.len() as i32;
        let mut last_parameter = size - 1;
        if let Some(maybe_rest) = maybe_rest
            && maybe_rest.is_rest(compiler)
        {
            let rest_index = param_list.get_child_count(compiler) - 1;
            // If the rest parameter is removable they all are.
            if size < rest_index
                || (rest_index < size && parameters[rest_index as usize].should_remove())
            {
                let value = IR::arraylit(compiler, &[]).srcref(compiler, maybe_rest);
                for i in rest_index..size {
                    let parameter = &parameters[i as usize];

                    check_state!(parameter.should_remove());
                    value.add_child_to_back(compiler, parameters[i as usize].get_arg());
                }
                maybe_rest.detach(compiler);
                let lhs = maybe_rest.remove_first_child(compiler).unwrap();
                self.add_rest_variable_to_function(compiler, r#fn, lhs, value);
            }

            // process the rest.
            last_parameter = (size - 1).min(rest_index - 1);
        }

        let mut i = last_parameter;
        while i >= 0 {
            let parameter = &parameters[i as usize];
            if parameter.should_remove() {
                let mut formal_param = NodeUtil::get_argument_for_function(compiler, r#fn, i);
                if let Some(fp) = formal_param {
                    fp.detach(compiler);
                    if fp.is_default_value(compiler) {
                        // Drop the default value as we should only get here if the default value
                        // isn't going to be used.
                        check_state!(!parameter.may_be_undefined);
                        formal_param = fp.remove_first_child(compiler);
                    }
                }

                let value = parameters[i as usize].get_arg();

                if let Some(formal_param) = formal_param {
                    self.add_variable_declaration_to_function(
                        compiler,
                        r#fn,
                        Some(formal_param),
                        value,
                    );
                } else {
                    // no formal param, just add the value to the function body. This is the case
                    // when the parameter is unused in the function body; we can skip declaring
                    // it and simply adding the argument value to the function body.
                    Self::add_expression_to_function(compiler, r#fn, value);
                }
            }
            i -= 1;
        }
    }

    // port: OptimizeParameters#optimizeCallSite
    fn optimize_call_site(
        compiler: &mut AbstractCompiler,
        parameters: &[Parameter],
        target: NodeId,
    ) {
        let call = ReferenceMap::get_call_or_new_node_for_target(compiler, target);
        let call_may_mutate_args = call.may_mutate_arguments(compiler);
        let mut call_may_mutate_globals_or_throw = call.may_mutate_global_state_or_throw(compiler);

        let mut index = parameters.len() as i32 - 1;
        while index >= 0 {
            let p = &parameters[index as usize];
            if p.should_remove() {
                Self::eliminate_call_target_arg_at(compiler, target, index);

                // Inlining parameters into a function may cause the function call to newly
                // mutate global state if either
                //  1) the parameter itself mutates global state
                //  2) the function may mutate its arguments, and the argument being inlined is
                //     mutable. We're deliberately conservative here - possibly the argument
                //     being inlined is not global state or not mutated in practice - because
                //     it's difficult to test all the edge cases.
                if !call_may_mutate_globals_or_throw
                    && (p.has_side_effects() // case (1)
                        || (call_may_mutate_args
                            && !NodeUtil::is_immutable_value(compiler, p.get_arg())))
                {
                    // case (2)
                    call_may_mutate_globals_or_throw = true;
                    let flags = SideEffectFlags::with_value(call.get_side_effect_flags(compiler))
                        .set_mutates_global_state()
                        .value_of();
                    call.set_side_effect_flags(compiler, flags);
                }
            }
            index -= 1;
        }
    }

    // port: OptimizeParameters#addRestVariableToFunction
    fn add_rest_variable_to_function(
        &mut self,
        compiler: &mut AbstractCompiler,
        function: NodeId,
        lhs: NodeId,
        value: NodeId,
    ) {
        check_state!(lhs.get_parent(compiler).is_none());
        self.add_variable_declaration_to_function(compiler, function, Some(lhs), value);
    }

    /// Adds a variable to the function block after all hoisted functions.
    ///
    /// - `function`: A function node.
    /// - `lhs`: The lhs expression.
    /// - `value`: The initial value of the variable.
    // port: OptimizeParameters#addVariableDeclarationToFunction
    fn add_variable_declaration_to_function(
        &mut self,
        compiler: &mut AbstractCompiler,
        function: NodeId,
        lhs: Option<NodeId>,
        value: NodeId,
    ) {
        check_state!(
            value.get_parent(compiler).is_none(),
            "Value must be detached"
        );
        check_state!(
            lhs.is_some(),
            "formal parameter being declared must not be null"
        );
        let lhs = lhs.unwrap();
        check_state!(
            lhs.get_parent(compiler).is_none(),
            "formal parameter being declared must be detached"
        );
        let mut stmts: Vec<NodeId> = Vec::new();
        if lhs.is_destructuring_pattern(compiler) {
            Self::rewrite_destructuring_pattern(compiler, lhs, value, &mut stmts);
        } else {
            stmts.push(NodeUtil::new_var_node_with_lhs(compiler, lhs, Some(value)));
        }
        Self::insert_statements(compiler, function, &stmts);
    }

    /// Adds an expression to the function block after all hoisted functions.
    // port: OptimizeParameters#addExpressionToFunction
    fn add_expression_to_function(
        compiler: &mut AbstractCompiler,
        function: NodeId,
        value: NodeId,
    ) {
        let stmts: Vec<NodeId> = vec![IR::expr_result(compiler, value).srcref(compiler, value)];
        Self::insert_statements(compiler, function, &stmts);
    }

    /// Insert the statements at the beginning of the function body, but after any hoisted
    /// function declarations so that the AST stays normalized.
    // port: OptimizeParameters#insertStatements
    fn insert_statements(compiler: &mut AbstractCompiler, function: NodeId, stmts: &[NodeId]) {
        if stmts.is_empty() {
            return;
        }
        let block = NodeUtil::get_function_body(compiler, function);
        let insertion_point =
            NodeUtil::get_insertion_point_after_all_inner_function_declarations(compiler, block);
        match insertion_point {
            None => Self::add_statements_to_back(compiler, block, stmts),
            Some(insertion_point) => Self::add_statements_before(compiler, insertion_point, stmts),
        }
        compiler.report_change_to_enclosing_scope(stmts[0]);
    }

    // port: OptimizeParameters#addStatementsToBack
    fn add_statements_to_back(compiler: &mut AbstractCompiler, block: NodeId, stmts: &[NodeId]) {
        for &stmt in stmts {
            block.add_child_to_back(compiler, stmt);
        }
    }

    // port: OptimizeParameters#addStatementsBefore
    fn add_statements_before(
        compiler: &mut AbstractCompiler,
        insertion_point: NodeId,
        stmts: &[NodeId],
    ) {
        for &item in stmts.iter().rev() {
            item.insert_before(compiler, insertion_point);
        }
    }

    /// Removes all formal parameters starting at argIndex.
    // port: OptimizeParameters#eliminateParamsAfter(Node,int)
    fn eliminate_params_after_index(
        &mut self,
        compiler: &mut AbstractCompiler,
        fn_node: NodeId,
        mut arg_index: i32,
    ) {
        let mut formal_arg_ptr =
            NodeUtil::get_function_parameters(compiler, fn_node).get_first_child(compiler);
        while arg_index != 0 && formal_arg_ptr.is_some() {
            formal_arg_ptr = formal_arg_ptr.unwrap().get_next(compiler);
            arg_index -= 1;
        }
        self.eliminate_params_after(compiler, fn_node, formal_arg_ptr);
    }

    // port: OptimizeParameters#eliminateParamsAfter(Node,Node)
    fn eliminate_params_after(
        &mut self,
        compiler: &mut AbstractCompiler,
        fn_node: NodeId,
        formal: Option<NodeId>,
    ) {
        if let Some(formal) = formal {
            // Keep the args in the same order, do the last first.
            let next = formal.get_next(compiler);
            self.eliminate_params_after(compiler, fn_node, next);
            let mut stmts: Vec<NodeId> = Vec::new();
            formal.detach(compiler);
            if formal.is_rest(compiler) {
                check_state!(formal.get_next(compiler).is_none());
                let lhs = formal.remove_first_child(compiler);
                let value = IR::arraylit(compiler, &[]).srcref(compiler, formal);
                self.add_variable_declaration_to_function(compiler, fn_node, lhs, value);
            } else if formal.is_default_value(compiler) {
                let lhs = formal.remove_first_child(compiler);
                let value = formal.get_last_child(compiler).unwrap().detach(compiler);
                self.add_variable_declaration_to_function(compiler, fn_node, lhs, value);
            } else if formal.is_destructuring_pattern(compiler) {
                // Destructuring declarations must have an rhs.
                // NOTE: assigning undefined will cause an exception at runtime if this code is
                // evaluated, which matches the behavior of the input code. It's also possible
                // this method will never be evaluated at runtime. This pass jointly optimizes all
                // methods with the same name.
                let value = NodeUtil::new_undefined_node(compiler, Some(formal));
                Self::rewrite_destructuring_pattern(compiler, formal, value, &mut stmts);
                Self::insert_statements(compiler, fn_node, &stmts);
            } else {
                stmts.push(IR::var(compiler, formal).srcref_if_missing(compiler, formal));
                Self::insert_statements(compiler, fn_node, &stmts);
            }
        }
    }

    // port: OptimizeParameters#rewriteDestructuringPattern
    fn rewrite_destructuring_pattern(
        compiler: &mut AbstractCompiler,
        destructuring_pattern: NodeId,
        value: NodeId,
        stmts: &mut Vec<NodeId>,
    ) {
        check_state!(destructuring_pattern.is_destructuring_pattern(compiler));
        check_state!(
            !destructuring_pattern.has_parent(compiler),
            "Formal parameter must be detached for rewriting"
        );

        let ast: &mut Ast = compiler;
        NodeUtil::visit_lhs_nodes_in_destructuring_pattern(
            ast,
            destructuring_pattern,
            &mut |ast: &mut Ast, name: NodeId| {
                // Add a declaration outside the array pattern for the given name.
                check_state!(
                    name.is_name(ast),
                    // TODO(rishipal): Is this always true? What about var [{a}] = [{a:4}]; ?
                    "lhs in destructuring declaration should be a simple name. (%s)",
                    name.to_string(ast)
                );
                let new_name = IR::name(ast, name.get_string(ast)).srcref(ast, name);
                let new_var = IR::var(ast, new_name).srcref(ast, name);
                stmts.push(new_var);
            },
        );
        let assign = IR::assign(compiler, destructuring_pattern, value)
            .srcref(compiler, destructuring_pattern);
        let expr = IR::expr_result(compiler, assign).srcref(compiler, destructuring_pattern);
        stmts.push(expr);
    }

    /// Eliminates the parameter from a function call.
    // port: OptimizeParameters#eliminateCallTargetArgAt
    fn eliminate_call_target_arg_at(compiler: &mut AbstractCompiler, r: NodeId, arg_index: i32) {
        let call_arg_node =
            ReferenceMap::get_argument_for_call_or_new_or_dot_call(compiler, r, arg_index);
        if let Some(call_arg_node) = call_arg_node {
            NodeUtil::delete_node(compiler, call_arg_node);
        }
    }
}

impl CompilerPass for OptimizeParameters {
    // port: OptimizeParameters#process(Node,Node)
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        check_state!(compiler.get_life_cycle_stage() == LifeCycleStage::NORMALIZED);

        // Java passes `this`; the builder owns its passes here. Each factory call creates a new
        // OptimizeParameters and runs it once (globalScope and decisionsLog are set by each
        // process call), so the pass given to the builder is a new instance built from the same
        // compiler.
        OptimizeCalls::builder()
            .set_compiler(compiler)
            .set_consider_externs(false)
            .add_pass(Box::new(OptimizeParameters::new(compiler)))
            .build()
            .process(compiler, externs, root);
    }
}

impl CallGraphCompilerPass for OptimizeParameters {
    // port: OptimizeParameters#process(Node,Node,ReferenceMap)
    fn process(
        &mut self,
        compiler: &mut AbstractCompiler,
        _externs: NodeId,
        _root: NodeId,
        ref_map: &mut ReferenceMap,
    ) {
        // try-with-resources: the log is closed when dropped; finally: decisionsLog = null.
        let decisions_log =
            compiler.create_or_reopen_indexed_log("OptimizeParameters", "decisions.log", &[]);
        // Save the LogFile into a field to avoid bucket-brigade passing it through a bunch of
        // methods
        self.decisions_log = Some(decisions_log);
        self.global_scope = ref_map.get_global_scope();
        let ref_map: &ReferenceMap = ref_map;

        // Find all function nodes that are possible candidates for parameter removal.
        let mut to_optimize: Vec<&Vec<NodeId>> = Vec::new();

        for (key, refs) in ref_map.get_name_references() {
            let mut candidate_analysis = self.analyze_candidate_name(compiler, key, refs);
            if candidate_analysis.should_convert_tagged_template_literals() {
                candidate_analysis.convert_tagged_template_literals(compiler);
            }
            if candidate_analysis.is_safe_to_optimize() {
                to_optimize.push(refs);
            }
        }

        for (key, refs) in ref_map.get_prop_references() {
            let mut candidate_analysis = self.analyze_candidate_property(compiler, key, refs);
            if candidate_analysis.should_convert_tagged_template_literals() {
                candidate_analysis.convert_tagged_template_literals(compiler);
            }
            if candidate_analysis.is_safe_to_optimize() {
                to_optimize.push(refs);
            }
        }

        // NOTE: The optimization that are perform must be careful to keep the
        // ReferenceMap in a consistent state. They should be careful to not
        // remove or add global references without update the reference map.
        // While adding references to the map is O(1), removing references
        // O(n) where (n) is the number of references.
        //
        // So the most transformative pass should be last. So:
        //
        // - Removing parameters not provided by any call-site only
        // moves the name and default values from the parameter list to the
        // functions' bodies, so no updates are needed if the same node
        // are reused.
        //
        // - Moving parameters that are provided the same value by every
        // call-site to the function bodies, is currently limited to cases
        // where there are only one function definition, so no updates
        // are needed if the same nodes are reused.
        //
        // - Removing parameters that are unreferenced from call-sites, may
        // remove references, as it is run last.

        for refs in &to_optimize {
            self.try_eliminate_optional_args(compiler, refs);
        }

        for refs in &to_optimize {
            self.try_eliminate_constant_args(compiler, refs);
        }

        // tryEliminateUnusedArgs may mutate
        let mut optimizer = UnusedParameterOptimizer {
            ast_analyzer: &self.ast_analyzer,
            to_remove: Vec::new(),
            to_replace_with_zero: Vec::new(),
        };
        for refs in &to_optimize {
            optimizer.try_eliminate_unused_args(compiler, refs);
        }
        optimizer.apply_changes(compiler);
        self.decisions_log = None;
    }
}

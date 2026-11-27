/*
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AbstractScope.java,
//   src/com/google/javascript/jscomp/FlowSensitiveInlineVariables.java.

//! Port of `FlowSensitiveInlineVariables.java`.
//!
//! Inline variables when possible. Using the information from [`MaybeReachingVariableUse`] and
//! [`MustBeReachingVariableDef`], this pass attempts to inline a variable by placing the value at
//! the definition where the variable is used. The basic requirements for inlining are the
//! following:
//!
//! - There is exactly one reaching definition at the use of that variable
//! - There is exactly one use for that definition of the variable
//!
//! Other requirements can be found in `Candidate::can_inline`. Currently this pass does not
//! operate on the global scope due to compilation time.
//!
//! Rust shape: Java's per-function fields `cfg`, `candidates`, `reachingDef` and `reachingUses`
//! ("persistent in the whole execution of enter scope") are locals of `enter_scope`. The CFG is
//! owned by the analysis that annotates it: `MustBeReachingVariableDef` while candidates are
//! gathered, then `MaybeReachingVariableUse` (Java shares one graph object, and the second
//! analysis re-initializes its annotations in the same order).

use crate::abstract_compiler::AbstractCompiler;
use crate::accessor_summary::AccessorSummary;
use crate::compiler_pass::CompilerPass;
use crate::control_flow_analysis::ControlFlowAnalysis;
use crate::control_flow_graph::ControlFlowGraph;
use crate::data_flow_analysis::{DataFlowAnalysis, compute_escaped};
use crate::function_vars::FunctionVars;
use crate::graph::adjacency_graph::AdjacencyGraph;
use crate::graph::check_paths_between_nodes::CheckPathsBetweenNodes;
use crate::live_variables_analysis;
use crate::maybe_reaching_variable_use::MaybeReachingVariableUse;
use crate::must_be_reaching_variable_def::{
    BorrowedMustDef, Definition, MustBeReachingVariableDef, PreparedMustDef,
};
use crate::node_traversal::{Callback, NodeTraversal, ScopedCallback};
use crate::node_util::NodeUtil;
use crate::parallel_parse::ParallelAnalysisMode;
use crate::scope::ScopeId;
use crate::scope_creator::ScopeCreator;
use crate::syntactic_scope_cache::{DeepFunctionStamp, SyntacticScopeCache};
use crate::syntactic_scope_creator::{SyntacticScopeCreator, ensure_ancestor_scope};
use crate::var::VarId;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use closure_rhino::{check_argument, check_not_null, check_state};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::{Arc, LazyLock};

static USE_CLEAN_CACHE: LazyLock<bool> =
    LazyLock::new(|| std::env::var("CLOSURE_RS_FSIV_CACHE").as_deref() != Ok("off"));

/// `CLOSURE_RS_FSIV_CACHE=verify`: a function the clean records would skip is inlined into
/// anyway, and must inline nothing.
static VERIFY_CLEAN_CACHE: LazyLock<bool> =
    LazyLock::new(|| std::env::var("CLOSURE_RS_FSIV_CACHE").as_deref() == Ok("verify"));

/// Rust-only (performance divergence): the stamp a function's clean record is taken under
/// (`FlowInlineCleanCache`).
#[derive(Clone)]
enum CleanKey {
    /// No record is taken (the cache is off).
    None,
    /// A leaf function: `SyntacticScopeCache::subtree_stamp_with_flags`.
    Leaf((u32, u32)),
    /// A function with functions in it: their stamps too (`DeepFunctionStamp`), as which of
    /// its variables they use decides what may be inlined (`computeEscaped`).
    Deep(Arc<DeepFunctionStamp>),
}

/// Rust-only (performance divergence): compiler-owned cache of unchanged leaf functions known to
/// produce zero flow-sensitive inlines under the current purity environment (`AccessorSummary`,
/// `has_reg_exp_global_references`, and `assume_getters_are_pure`). Per-function `SIDE_EFFECT_FLAGS`
/// updates are tracked per subtree via `SyntacticScopeCache::subtree_stamp_with_flags`.
#[derive(Default)]
pub(crate) struct FlowInlineCleanCache {
    accessor_summary: Option<Arc<AccessorSummary>>,
    has_reg_exp_global_references: bool,
    assume_getters_are_pure: bool,
    clean_leaves: IndexMap<NodeId, (u32, u32)>,
    /// The same for functions with functions in them, under their `DeepFunctionStamp`.
    clean_deep: IndexMap<NodeId, Arc<DeepFunctionStamp>>,
}

impl FlowInlineCleanCache {
    fn sync_environment(
        &mut self,
        accessor_summary: Option<&Arc<AccessorSummary>>,
        has_reg_exp_global_references: bool,
        assume_getters_are_pure: bool,
    ) {
        let accessor_same = match (&self.accessor_summary, accessor_summary) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b) || a == b,
            (None, None) => true,
            _ => false,
        };
        if !accessor_same
            || self.has_reg_exp_global_references != has_reg_exp_global_references
            || self.assume_getters_are_pure != assume_getters_are_pure
        {
            self.clean_leaves.clear();
            self.clean_deep.clear();
        }
        self.accessor_summary = accessor_summary.cloned();
        self.has_reg_exp_global_references = has_reg_exp_global_references;
        self.assume_getters_are_pure = assume_getters_are_pure;
    }

    /// Whether `fn_node` has a clean record that still holds: it would inline nothing.
    fn is_clean(compiler: &mut AbstractCompiler, fn_node: NodeId, is_leaf: bool) -> bool {
        let own = SyntacticScopeCache::subtree_stamp_with_flags(compiler, fn_node);
        let cache = &compiler.flow_inline_clean_cache;
        if is_leaf {
            cache.clean_leaves.get(&fn_node) == Some(&own)
        } else {
            cache.clean_deep.get(&fn_node).is_some_and(|kept| {
                compiler
                    .syntactic_scope_cache
                    .deep_function_stamp_is_current(own, kept)
            })
        }
    }

    /// The stamp to take `fn_node`'s clean record under, before its work.
    fn key_now(compiler: &mut AbstractCompiler, fn_node: NodeId, is_leaf: bool) -> CleanKey {
        if is_leaf {
            CleanKey::Leaf(SyntacticScopeCache::subtree_stamp_with_flags(
                compiler, fn_node,
            ))
        } else {
            CleanKey::Deep(Arc::new(SyntacticScopeCache::deep_function_stamp(
                compiler, fn_node,
            )))
        }
    }

    /// Records `fn_node` as clean (it inlined nothing and no inlining was refused for a name of
    /// an outer scope) under `key`, unless it changed since the key was taken (`worked`: its work
    /// ran in between).
    fn record(compiler: &mut AbstractCompiler, fn_node: NodeId, key: &CleanKey, worked: bool) {
        match key {
            CleanKey::None => {}
            CleanKey::Leaf(stamp) => {
                if !worked
                    || SyntacticScopeCache::subtree_stamp_with_flags(compiler, fn_node) == *stamp
                {
                    compiler
                        .flow_inline_clean_cache
                        .clean_leaves
                        .insert(fn_node, *stamp);
                }
            }
            CleanKey::Deep(stamp) => {
                if !worked || {
                    let own = SyntacticScopeCache::subtree_stamp_with_flags(compiler, fn_node);
                    compiler
                        .syntactic_scope_cache
                        .deep_function_stamp_is_current(own, stamp)
                } {
                    compiler
                        .flow_inline_clean_cache
                        .clean_deep
                        .insert(fn_node, Arc::clone(stamp));
                }
            }
        }
    }
}

pub struct FlowSensitiveInlineVariables {
    side_effect_predicate: SideEffectPredicate,
}

struct SideEffectPredicate {
    // Check if there are side effects affecting the value of any of these names
    // (but not properties defined on that name)
    names_to_check: Option<IndexSet<JsString>>,
}

impl SideEffectPredicate {
    // port: FlowSensitiveInlineVariables.SideEffectPredicate#SideEffectPredicate()
    fn new() -> Self {
        Self {
            names_to_check: None,
        }
    }

    // port: FlowSensitiveInlineVariables.SideEffectPredicate#SideEffectPredicate(Set)
    fn new_with_names(names: IndexSet<JsString>) -> Self {
        Self {
            names_to_check: Some(names),
        }
    }

    // port: FlowSensitiveInlineVariables.SideEffectPredicate#apply
    fn apply(&self, compiler: &mut AbstractCompiler, n: Option<NodeId>) -> bool {
        // When the node is null it means, we reached the implicit return
        // where the function returns (possibly without an return statement)
        let Some(n) = n else {
            return false;
        };

        if let Some(names_to_check) = &self.names_to_check
            && n.is_name(compiler)
            && names_to_check.contains(&n.get_string(compiler))
            && NodeUtil::is_l_value(compiler, n)
        {
            // the name is being written to. this is a problem, unless it is part of a top-level
            // assign chain and the write will take place after all CFG node subexpressions are
            // evaluated
            return !is_top_level_assign_target(compiler, n);
        }

        let ast_analyzer = compiler.get_ast_analyzer();
        // TODO(user): We only care about calls to functions that
        // passes one of the dependent variable to a non-side-effect free
        // function.
        if (n.is_call(compiler)
            || n.is_opt_chain_call(compiler)
            || n.is_tagged_template_lit(compiler))
            && ast_analyzer.function_call_has_side_effects(compiler, n)
        {
            return true;
        }

        if n.is_new(compiler) && ast_analyzer.constructor_call_has_side_effects(compiler, n) {
            return true;
        }

        if n.is_del_prop(compiler) {
            return true;
        }

        if (n.is_get_prop(compiler) || n.is_get_elem(compiler)) && NodeUtil::is_l_value(compiler, n)
        {
            return self.names_to_check.is_none() || !is_top_level_assign_target(compiler, n);
        }

        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            if !ControlFlowGraph::<NodeId>::is_entering_new_cfg_node(compiler, cur)
                && self.apply(compiler, Some(cur))
            {
                return true;
            }
            c = cur.get_next(compiler);
        }
        false
    }
}

/// Rust-only (performance divergence): `(fn_node, body, analysis, is_leaf)` of
/// `collect_candidate_functions`.
pub(crate) type CollectedCandidate = (NodeId, NodeId, InlineAnalysis, bool);

/// Rust-only (performance divergence): the candidate functions a walk finds in a top-level unit
/// of a script that is no externs, in walk order (`collect_candidate_functions_in`): a function of
/// the subtree alone.
pub(crate) type FsivUnitRecord = Box<[CollectedCandidate]>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum UnitCacheMode {
    Off,
    On,
    Verify,
}

/// `CLOSURE_RS_FSIV_UNIT_CACHE=off` walks the whole program for candidates in every run
/// (`FsivUnitRecord`); `=verify` walks every unit and compares with the kept record.
static UNIT_CACHE_MODE: LazyLock<UnitCacheMode> =
    LazyLock::new(
        || match std::env::var("CLOSURE_RS_FSIV_UNIT_CACHE").as_deref() {
            Ok("off") => UnitCacheMode::Off,
            Ok("verify") => UnitCacheMode::Verify,
            _ => UnitCacheMode::On,
        },
    );

/// A function can be ruled out before allocating a control-flow graph. A surviving
/// replacement needs a definition and a non-loop read with the same spelling. Collisions
/// in the two name masks only admit additional functions to the ordinary analysis.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum InlineAnalysis {
    NoCandidate,
    DeferUses,
    Full,
}

impl InlineAnalysis {
    // Keep the lookahead's call stack bounded even on a single deeply nested expression.
    const MAX_DEPTH: u8 = 128;

    fn for_body(ast: &Ast, body: NodeId, node_budget: usize) -> Self {
        struct Search {
            remaining: usize,
            definitions: u64,
            reads: u64,
        }
        impl Search {
            // Return whether this subtree contains a forbidden RHS node. Each AST node
            // is inspected once even when initializers contain nested assignments.
            fn visit(&mut self, ast: &Ast, n: NodeId, in_loop: bool, depth: u8) -> Option<bool> {
                if self.remaining == 0
                    || depth == 0
                    || n.is_class(ast)
                    || (NodeUtil::is_logical_assignment_op(ast, n)
                        && n.get_first_child(ast).is_some_and(|lhs| lhs.is_name(ast)))
                {
                    // The backwards analysis checks that logical assignment to a name
                    // has already been lowered. Keep its eager schedule for that case.
                    return None;
                }
                self.remaining -= 1;
                if n.is_function(ast) {
                    // An outer CFG can visit the parameter list of a nested function, but
                    // never its body. Leave complex defaults and patterns to the full pass.
                    let params = n.get_second_child(ast)?;
                    for param in params.children(ast) {
                        if self.remaining == 0 || !param.is_name(ast) || param.has_children(ast) {
                            return None;
                        }
                        self.remaining -= 1;
                    }
                    return Some(false);
                }
                let in_loop = in_loop || NodeUtil::is_loop_structure(ast, n);
                let mut forbidden = Candidate::forbidden_rhs_node(ast, n);
                let mut last_child_forbidden = false;
                for child in n.children(ast) {
                    last_child_forbidden = self.visit(ast, child, in_loop, depth - 1)?;
                    forbidden |= last_child_forbidden;
                }

                if self.definitions & self.reads == 0 {
                    if n.is_name(ast) {
                        let bit = Self::name_bit(ast, n)?;
                        let parent = n.get_parent(ast)?;
                        if n.has_children(ast) && !last_child_forbidden {
                            self.definitions |= bit;
                        }
                        let assignment_target = parent.get_first_child(ast) == Some(n)
                            && NodeUtil::is_assignment_op(ast, parent);
                        if !in_loop
                            && !assignment_target
                            && !NodeUtil::is_name_declaration(ast, Some(parent))
                            && !matches!(
                                parent.get_token(ast),
                                Token::PARAM_LIST | Token::CATCH | Token::INC | Token::DEC
                            )
                        {
                            self.reads |= bit;
                        }
                    } else if n.is_assign(ast)
                        && !last_child_forbidden
                        && n.get_parent(ast)
                            .is_some_and(|p| NodeUtil::is_expr_assign(ast, p))
                        && let Some(target) = n.get_first_child(ast)
                        && target.is_name(ast)
                    {
                        self.definitions |= Self::name_bit(ast, target)?;
                    }
                }
                Some(forbidden)
            }

            fn name_bit(ast: &Ast, n: NodeId) -> Option<u64> {
                Some(1 << (n.string_payload(ast)?.hash_code() as u32 & 63))
            }
        }

        let mut search = Search {
            remaining: node_budget,
            definitions: 0,
            reads: 0,
        };
        if search.visit(ast, body, false, Self::MAX_DEPTH).is_none() {
            Self::Full
        } else if search.reads & search.definitions == 0 {
            Self::NoCandidate
        } else {
            Self::DeferUses
        }
    }
}

/// Whether the given node is the target of a (possibly chained) assignment
// port: FlowSensitiveInlineVariables#isTopLevelAssignTarget
fn is_top_level_assign_target(ast: &Ast, n: NodeId) -> bool {
    let mut ancestor = n.get_parent(ast).unwrap();
    while ancestor.is_assign(ast) {
        ancestor = ancestor.get_parent(ast).unwrap();
    }
    ancestor.is_expr_result(ast)
}

impl Default for FlowSensitiveInlineVariables {
    fn default() -> Self {
        Self::new()
    }
}

impl FlowSensitiveInlineVariables {
    /// Java's constructor also takes the compiler, which a pass does not store (DESIGN §6).
    // port: FlowSensitiveInlineVariables#FlowSensitiveInlineVariables
    pub fn new() -> Self {
        Self {
            side_effect_predicate: SideEffectPredicate::new(),
        }
    }

    // port: FlowSensitiveInlineVariables#isCandidateFunction
    fn is_candidate_function(ast: &Ast, fn_: NodeId) -> bool {
        let fn_body = fn_.get_last_child(ast).unwrap();
        Self::contains_candidate_expressions(ast, fn_body)
    }

    // port: FlowSensitiveInlineVariables#containsCandidateExpressions
    fn contains_candidate_expressions(ast: &Ast, n: NodeId) -> bool {
        if n.is_function(ast) {
            // don't recurse into inner functions or into expressions the can't contain
            // declarations.
            return false;
        }

        if (NodeUtil::is_name_declaration(ast, Some(n)) || Self::is_assignment_to_name(ast, n))
            // if it is a simple assignment
            && n.get_first_child(ast).unwrap().is_name(ast)
        {
            return true;
        }

        let mut c = n.get_first_child(ast);
        while let Some(cur) = c {
            if Self::contains_candidate_expressions(ast, cur) {
                return true;
            }
            c = cur.get_next(ast);
        }
        false
    }

    // port: FlowSensitiveInlineVariables#isAssignmentToName
    fn is_assignment_to_name(ast: &Ast, n: NodeId) -> bool {
        if NodeUtil::is_assignment_op(ast, n) || n.is_dec(ast) || n.is_inc(ast) {
            // if it is a simple assignment
            return n.get_first_child(ast).unwrap().is_name(ast);
        }
        false
    }

    /// Rust-only (performance divergence): collects candidate `(fn_node, body, analysis, is_leaf)`
    /// in exact `NodeTraversal` `enterScope` preorder while skipping extern scripts and functions
    /// ruled out by `is_candidate_function` or `InlineAnalysis::for_body`. Returns whether `n` or
    /// any descendant contains a `FUNCTION` or `CLASS`.
    fn collect_candidate_functions(
        compiler: &AbstractCompiler,
        n: NodeId,
        out: &mut Vec<CollectedCandidate>,
    ) -> bool {
        Self::collect_candidate_functions_in(
            &compiler.ast,
            &|script| Self::is_extern_script(compiler, script),
            n,
            out,
        )
    }

    fn is_extern_script(compiler: &AbstractCompiler, script: NodeId) -> bool {
        script.get_input_id(compiler).is_some_and(|input_id| {
            compiler
                .get_input(&input_id)
                .is_some_and(|input| input.is_extern())
        })
    }

    /// `collect_candidate_functions` with the extern test of a SCRIPT as `is_extern_script`.
    fn collect_candidate_functions_in(
        ast: &Ast,
        is_extern_script: &dyn Fn(NodeId) -> bool,
        n: NodeId,
        out: &mut Vec<CollectedCandidate>,
    ) -> bool {
        Self::collect_candidate_functions_listing(ast, is_extern_script, n, out, &mut None)
    }

    /// `collect_candidate_functions_in`, also listing every function it reaches in `functions`
    /// when given (for `UnitStamps`).
    fn collect_candidate_functions_listing(
        ast: &Ast,
        is_extern_script: &dyn Fn(NodeId) -> bool,
        n: NodeId,
        out: &mut Vec<CollectedCandidate>,
        functions: &mut Option<&mut Vec<NodeId>>,
    ) -> bool {
        if n.is_script(ast) && is_extern_script(n) {
            return false;
        }
        match n.get_token(ast) {
            Token::FUNCTION => {
                if let Some(functions) = functions {
                    functions.push(n);
                }
                let fn_name = n.get_first_child(ast).unwrap();
                let params = fn_name.get_next(ast).unwrap();
                let body = params.get_next(ast).unwrap();
                let params_has_nested = Self::collect_candidate_functions_listing(
                    ast,
                    is_extern_script,
                    params,
                    out,
                    functions,
                );
                let slot = if body.is_block(ast) && Self::is_candidate_function(ast, n) {
                    let analysis = InlineAnalysis::for_body(ast, body, usize::MAX);
                    if !matches!(analysis, InlineAnalysis::NoCandidate) {
                        let idx = out.len();
                        out.push((n, body, analysis, false));
                        Some(idx)
                    } else {
                        None
                    }
                } else {
                    None
                };
                let body_has_nested = Self::collect_candidate_functions_listing(
                    ast,
                    is_extern_script,
                    body,
                    out,
                    functions,
                );
                if let Some(idx) = slot {
                    out[idx].3 = !params_has_nested && !body_has_nested;
                }
                true
            }
            Token::CLASS => {
                let class_name = n.get_first_child(ast).unwrap();
                let extends_clause = class_name.get_next(ast).unwrap();
                let body = extends_clause.get_next(ast).unwrap();
                Self::collect_candidate_functions_listing(
                    ast,
                    is_extern_script,
                    extends_clause,
                    out,
                    functions,
                );
                Self::collect_candidate_functions_listing(
                    ast,
                    is_extern_script,
                    class_name,
                    out,
                    functions,
                );
                Self::collect_candidate_functions_listing(
                    ast,
                    is_extern_script,
                    body,
                    out,
                    functions,
                );
                true
            }
            _ => {
                let mut has_nested = false;
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    let next = c.get_next(ast);
                    has_nested |= Self::collect_candidate_functions_listing(
                        ast,
                        is_extern_script,
                        c,
                        out,
                        functions,
                    );
                    child = next;
                }
                has_nested
            }
        }
    }

    /// Rust-only (performance divergence): `collect_candidate_functions` of `root`, on worker
    /// threads for a large tree. The statements of the scripts that are no externs are walked in
    /// runs (each statement is no function's descendant, so what a walk of `root` finds in it is
    /// what a walk of the statement alone finds, and it holds no SCRIPT) and the candidates are
    /// concatenated in statement order: the order of a walk of `root`.
    fn collect_candidates(compiler: &AbstractCompiler, root: NodeId) -> Vec<CollectedCandidate> {
        let ast = &compiler.ast;
        if *crate::parallel_parse::PARALLEL_ANALYSIS != ParallelAnalysisMode::Off
            && root.is_root(ast)
            // Smaller trees are walked faster than worker threads start.
            && ast.node_count() >= 50_000
            && crate::parallel_parse::parallel_thread_count(usize::MAX, 2, 8) > 1
        {
            let mut units = Vec::new();
            let mut script = root.get_first_child(ast);
            while let Some(s) = script {
                if !s.is_script(ast) {
                    units.push(s);
                } else if !Self::is_extern_script(compiler, s) {
                    units.extend(crate::parallel_parse::statement_units(ast, s));
                }
                script = s.get_next(ast);
            }
            let threads = crate::parallel_parse::parallel_thread_count(units.len(), 2, 8);
            if threads > 1
                && let Some(candidates) = crate::parallel_parse::collect_in_unit_order(
                    "closure-fsiv",
                    threads,
                    &units,
                    |unit, out| {
                        Self::collect_candidate_functions_in(ast, &|_| false, unit, out);
                    },
                )
            {
                return candidates;
            }
        }
        let mut candidates = Vec::new();
        Self::collect_candidate_functions(compiler, root, &mut candidates);
        candidates
    }

    /// Rust-only (performance divergence): what follows the work on `fn_node`: its clean record
    /// when it inlined nothing and no inlining was refused for a name of an outer scope (and, for
    /// `CLOSURE_RS_FSIV_CACHE=verify`, the check that a function a clean record skips inlined
    /// nothing).
    fn after_work(
        compiler: &mut AbstractCompiler,
        fn_node: NodeId,
        key: &CleanKey,
        did_inline: bool,
        rejected_by_outer: bool,
        verify_clean: &IndexSet<NodeId>,
    ) {
        check_state!(
            !(did_inline && verify_clean.contains(&fn_node)),
            "CLOSURE_RS_FSIV_CACHE=verify: FlowSensitiveInlineVariables inlined into %s, which its clean record skips",
            fn_node.to_string(&compiler.ast)
        );
        if !did_inline && !rejected_by_outer {
            FlowInlineCleanCache::record(compiler, fn_node, key, true);
        }
    }

    /// Rust-only (performance divergence): `collect_candidates` with the candidates of the units
    /// unchanged since the last run taken from their records (`FsivUnitRecord`); the others are
    /// walked (on worker threads when there are many). The units are those of the parallel walk
    /// of `collect_candidates`, whose concatenated candidates are those of a walk of `root`.
    fn collect_candidates_with_unit_cache(
        compiler: &mut AbstractCompiler,
        root: NodeId,
    ) -> Vec<CollectedCandidate> {
        use crate::unit_cache::UnitStamps;
        if !root.is_root(&compiler.ast) {
            return Self::collect_candidates(compiler, root);
        }
        let mut units = Vec::new();
        let mut script = root.get_first_child(&compiler.ast);
        while let Some(s) = script {
            if !s.is_script(&compiler.ast) {
                units.push(s);
            } else if !Self::is_extern_script(compiler, s) {
                units.extend(crate::parallel_parse::statement_units(&compiler.ast, s));
            }
            script = s.get_next(&compiler.ast);
        }
        UnitStamps::refresh(compiler, root);
        let mut cache = std::mem::take(&mut compiler.fsiv_unit_cache);
        let verify = *UNIT_CACHE_MODE == UnitCacheMode::Verify;
        let kept: Vec<Option<(UnitStamps, FsivUnitRecord)>> = units
            .iter()
            .map(|&unit| {
                let current = cache.get(compiler, unit).is_some();
                cache.take(unit).filter(|_| current)
            })
            .collect();
        let stale: Vec<NodeId> = units
            .iter()
            .zip(&kept)
            .filter(|(_, kept)| verify || kept.is_none())
            .map(|(&unit, _)| unit)
            .collect();
        let walk = |ast: &Ast, unit: NodeId| {
            let mut found = Vec::new();
            let mut functions = Vec::new();
            Self::collect_candidate_functions_listing(
                ast,
                &|_| false,
                unit,
                &mut found,
                &mut Some(&mut functions),
            );
            (found.into_boxed_slice(), functions)
        };
        let threads = if *crate::parallel_parse::PARALLEL_ANALYSIS != ParallelAnalysisMode::Off
            && compiler.ast.node_count() >= 50_000
        {
            crate::parallel_parse::parallel_thread_count(stale.len(), 2, 8)
        } else {
            1
        };
        let mut walked: Vec<Option<(FsivUnitRecord, Vec<NodeId>)>> =
            (0..stale.len()).map(|_| None).collect();
        {
            let ast = &compiler.ast;
            for batch in crate::parallel_parse::run_scoped_indexed(
                "closure-fsiv",
                threads,
                &stale,
                |_, &unit| Some(walk(ast, unit)),
            ) {
                for (idx, out) in batch {
                    walked[idx] = Some(out);
                }
            }
        }
        let mut walked = walked.into_iter();
        let mut records = Vec::with_capacity(units.len());
        for (&unit, kept) in units.iter().zip(kept) {
            let entry = match kept {
                Some(kept) if !verify => kept,
                kept => {
                    let (found, functions) = walked.next().unwrap().unwrap();
                    if let Some((_, kept)) = &kept {
                        assert!(
                            *kept == found,
                            "CLOSURE_RS_FSIV_UNIT_CACHE=verify: the kept candidates of {} differ from \
                             a new walk\nkept {:?}\nnew  {:?}",
                            unit.to_string(&compiler.ast),
                            kept,
                            found
                        );
                    }
                    (UnitStamps::of(compiler, unit, &functions), found)
                }
            };
            records.push((unit, entry.0, entry.1));
        }
        let candidates = records
            .iter()
            .flat_map(|(_, _, found)| found.iter().copied())
            .collect();
        cache.replace(records);
        compiler.fsiv_unit_cache = cache;
        candidates
    }

    /// Runs full flow-sensitive inlining on one function body. Returns `(did_inline,
    /// rejected_by_outer_scope)` so unchanged leaf functions with zero inlines and no outer-scope
    /// slot rejections can be cached across optimization rounds.
    fn inline_function_body(
        &mut self,
        compiler: &mut AbstractCompiler,
        scope_creator: &mut dyn ScopeCreator,
        function_scope_root: NodeId,
        scope_root: NodeId,
        scope: ScopeId,
        analysis: InlineAnalysis,
    ) -> (bool, bool) {
        // Compute the forward reaching definition.
        let cfg = ControlFlowAnalysis::compute_cfg_readonly(
            &compiler.ast,
            compiler.get_options().can_continue_after_errors(),
            function_scope_root,
            false,
            true,
        );

        let mut escaped: IndexSet<VarId> = IndexSet::<_>::default();
        let scope_parent = scope.get_parent(compiler).unwrap();
        let all_vars_declared_in_function =
            NodeUtil::get_all_vars_declared_in_function(compiler, scope_creator, scope_parent);
        let (all_vars_in_fn, _) = all_vars_declared_in_function.into_parts();
        compute_escaped(
            compiler,
            scope_parent,
            &mut escaped,
            scope_creator,
            &all_vars_in_fn,
        );

        let mut reaching_def =
            MustBeReachingVariableDef::new(compiler, cfg, escaped, all_vars_in_fn);
        reaching_def.analyze_indexed(compiler);
        let mut candidates: VecDeque<Candidate> = VecDeque::new();

        gather_candidates_shallow(compiler, &reaching_def, &mut candidates, scope_root, None);
        if candidates.is_empty() {
            return (false, false);
        }

        // Compute the backward reaching use. The CFG and per-function variable info can be
        // reused.
        let mut reaching_uses = reaching_def.into_use_analysis();
        let mut uses_ready = matches!(analysis, InlineAnalysis::Full);
        if uses_ready {
            reaching_uses.analyze_indexed(compiler);
        }
        let mut did_inline = false;
        let mut rejected_by_outer_scope = false;
        while let Some(mut c) = candidates.pop_front() {
            let candidate_var = check_not_null!(reaching_uses.variable(&c.var_name));
            let candidate_scope = candidate_var.get_scope(compiler);
            if !c.find_definition_and_single_read(compiler) {
                continue;
            }
            if !uses_ready {
                reaching_uses.analyze_indexed(compiler);
                uses_ready = true;
            }
            if c.can_inline(
                compiler,
                &mut reaching_uses,
                &self.side_effect_predicate,
                candidate_scope,
                &mut rejected_by_outer_scope,
            ) {
                c.inline_variable(compiler);
                did_inline = true;
                if !c.def_metadata.depends.is_empty() {
                    candidates.retain(|other| {
                        let c_var = scope.get_var(compiler, c.var_name.clone());
                        let remove = var_set_contains(compiler, &other.def_metadata.depends, c_var)
                            && !var_set_contains_all(
                                compiler,
                                &other.def_metadata.depends,
                                &c.def_metadata.depends,
                            );
                        !remove
                    });
                }
            }
        }
        (did_inline, rejected_by_outer_scope)
    }
}

struct PreparedFlowJob {
    fn_node: NodeId,
    body: NodeId,
    analysis: InlineAnalysis,
    is_leaf: bool,
    start_stamp: CleanKey,
    prepared: PreparedMustDef,
    catch_vars: IndexSet<VarId>,
    exported_local_names: IndexSet<JsString>,
}

impl PreparedFlowJob {
    /// Worker-local screening: runs CFG, `MustBeReachingVariableDef`, shallow candidate
    /// gathering, quick structural filtering, and (when needed) `MaybeReachingVariableUse`
    /// single-use checking. Drops all graphs/lattices on the worker thread (zero cross-thread
    /// frees) and returns whether the function has at least one candidate that could inline.
    fn might_inline(&self, ast: &Ast, can_continue_after_errors: bool) -> bool {
        let cfg = ControlFlowAnalysis::compute_cfg_readonly(
            ast,
            can_continue_after_errors,
            self.fn_node,
            false,
            true,
        );
        let reaching_def = self.prepared.run_borrowed(ast, cfg);
        let mut candidates: Vec<Candidate> = Vec::new();
        gather_candidates_shallow_readonly(
            ast,
            &reaching_def,
            &self.catch_vars,
            &self.exported_local_names,
            &mut candidates,
            self.body,
            None,
        );
        if candidates.is_empty() {
            return false;
        }
        let mut quick_survivors: Vec<Candidate> = Vec::new();
        for mut c in candidates {
            if c.quick_check_ast(ast) {
                quick_survivors.push(c);
            }
        }
        if quick_survivors.is_empty() {
            return false;
        }
        let cfg = reaching_def.into_cfg();
        let reaching_uses = MaybeReachingVariableUse::run_borrowed(
            ast,
            cfg,
            self.prepared.escaped_var_ids(),
            self.prepared.all_vars(),
        );
        quick_survivors
            .iter()
            .any(|c| reaching_uses.has_single_use(&c.var_name, c.get_def_cfg_node()))
    }

    /// Rust-only (performance divergence): whether the candidate `fn_node` may inline, found from
    /// the `Ast` alone (on a worker thread): `Some(false)` when `process` would skip the function
    /// for its variable count or `might_inline` finds nothing, `None` when the function needs its
    /// scopes to tell.
    ///
    /// For a function `FunctionVars::collect` handles, it builds the job `process` builds, with
    /// the variables `FunctionVars` lists instead of those of its scopes: the same names, scopes,
    /// declarations and escaped variables (`CLOSURE_RS_PARALLEL_ANALYSIS=verify` compares them),
    /// so `might_inline` finds the same. As for the screen of a chunk in `process`, the work on
    /// other functions before this one changes nothing that its analysis reads: its own subtree
    /// (an inlining in an enclosing function may move the function, never change it; the
    /// functions in it come after it).
    fn screen(
        ast: &Ast,
        convention: &dyn crate::coding_convention::CodingConvention,
        can_continue_after_errors: bool,
        fn_node: NodeId,
        body: NodeId,
        analysis: InlineAnalysis,
        is_leaf: bool,
    ) -> Option<bool> {
        let vars = FunctionVars::collect(ast, fn_node)?;
        let count = vars.block_var_count() as i32;
        if count > live_variables_analysis::MAX_VARIABLES_TO_ANALYZE + 1 {
            return Some(false);
        }
        if !vars.well_below_max() {
            return None;
        }
        let escaped = vars.escaped(convention);
        let mut catch_vars = IndexSet::default();
        let mut exported_local_names = IndexSet::default();
        for (name, &index) in &vars.all {
            let scope_root = vars.scope_root(index);
            // port: AbstractScope#isCatchScope
            if scope_root.is_block(ast)
                && scope_root.has_one_child(ast)
                && scope_root.get_first_child(ast)?.is_catch(ast)
            {
                catch_vars.insert(FunctionVars::id(index));
            }
            if convention.is_exported(name, /* local= */ true) {
                exported_local_names.insert(name.clone());
            }
        }
        let job = PreparedFlowJob {
            fn_node,
            body,
            analysis,
            is_leaf,
            start_stamp: CleanKey::None,
            prepared: PreparedMustDef::from_function_vars(&vars, &escaped),
            catch_vars,
            exported_local_names,
        };
        Some(job.might_inline(ast, can_continue_after_errors))
    }
}

impl CompilerPass for FlowSensitiveInlineVariables {
    // port: FlowSensitiveInlineVariables#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        if compiler.get_life_cycle_stage().is_normalized()
            && compiler.get_coding_convention().export_tests_are_pure()
        {
            // Rust-only (performance divergence): walk candidate functions directly without
            // creating scopes for externs or non-candidates, skip unchanged leaf functions known
            // to have zero inlines from an earlier round, and screen large batches in parallel
            // across worker threads (dropping CFGs/lattices on workers to avoid cross-thread
            // heap retention).
            let use_cache = *USE_CLEAN_CACHE;
            if use_cache {
                let accessor = compiler.get_accessor_summary().cloned();
                let has_regexp_globals = compiler.has_reg_exp_global_references();
                let assume_getters_pure = compiler.get_options().get_assume_getters_are_pure();
                compiler.flow_inline_clean_cache.sync_environment(
                    accessor.as_ref(),
                    has_regexp_globals,
                    assume_getters_pure,
                );
            }
            let raw_candidates = if *UNIT_CACHE_MODE == UnitCacheMode::Off {
                Self::collect_candidates(compiler, root)
            } else {
                Self::collect_candidates_with_unit_cache(compiler, root)
            };
            if raw_candidates.is_empty() {
                return;
            }
            let mut candidates = Vec::with_capacity(raw_candidates.len());
            // Rust-only: with `CLOSURE_RS_FSIV_CACHE=verify`, the functions a clean record skips.
            let mut verify_clean = IndexSet::<NodeId>::default();
            for (fn_node, body, analysis, is_leaf) in raw_candidates {
                let start_stamp = if use_cache {
                    if FlowInlineCleanCache::is_clean(compiler, fn_node, is_leaf) {
                        if !*VERIFY_CLEAN_CACHE {
                            continue;
                        }
                        verify_clean.insert(fn_node);
                    }
                    FlowInlineCleanCache::key_now(compiler, fn_node, is_leaf)
                } else {
                    CleanKey::None
                };
                candidates.push((fn_node, body, analysis, is_leaf, start_stamp));
            }
            if candidates.is_empty() {
                return;
            }
            let can_continue_after_errors = compiler.get_options().can_continue_after_errors();
            // Match `NodeTraversal::traverse_roots(externs, root)` by rooting the global scope at
            // `externs.get_parent()` so extern globals (e.g. `window`) are visible to
            // `Candidate::is_rhs_safe_to_inline` (`usage_scope.has_slot`).
            let global_scope_root = externs
                .get_parent(compiler)
                .filter(|&p| root.get_parent(compiler) == Some(p))
                .unwrap_or(root);
            let mut scope_creator = SyntacticScopeCreator::new();
            let mut scope_memo = IndexMap::default();
            let use_parallel =
                crate::parallel_parse::parallel_thread_count(candidates.len(), 64, 8) > 1;
            let mode = *crate::parallel_parse::PARALLEL_ANALYSIS;
            let verify = mode == ParallelAnalysisMode::Verify;
            if !use_parallel && !verify {
                // Rust-only (performance divergence): without worker threads the candidates are
                // screened on this thread, as on workers below (`verify` takes that path): the
                // screen needs no scopes, which the candidates it rules out then never get; a
                // leaf gets the clean-leaf record the work would give it.
                if mode == ParallelAnalysisMode::On {
                    let ast = &compiler.ast;
                    let convention = compiler.get_coding_convention();
                    let screened: Vec<bool> = candidates
                        .iter()
                        .map(|&(fn_node, body, analysis, is_leaf, _)| {
                            crate::parallel_parse::catch_unwind_quietly(|| {
                                PreparedFlowJob::screen(
                                    ast,
                                    convention,
                                    can_continue_after_errors,
                                    fn_node,
                                    body,
                                    analysis,
                                    is_leaf,
                                )
                            })
                            .flatten()
                                == Some(false)
                        })
                        .collect();
                    let mut rest = Vec::with_capacity(candidates.len());
                    for (candidate, screened) in candidates.into_iter().zip(screened) {
                        if !screened || verify_clean.contains(&candidate.0) {
                            rest.push(candidate);
                        } else {
                            FlowInlineCleanCache::record(
                                compiler,
                                candidate.0,
                                &candidate.4,
                                false,
                            );
                        }
                    }
                    candidates = rest;
                }
                for (fn_node, body, analysis, _, start_stamp) in candidates {
                    let fn_scope = ensure_ancestor_scope(
                        compiler,
                        &mut scope_creator,
                        &mut scope_memo,
                        global_scope_root,
                        fn_node,
                    );
                    let block_scope = scope_creator.create_scope(compiler, body, Some(fn_scope));
                    if live_variables_analysis::MAX_VARIABLES_TO_ANALYZE
                        < block_scope.get_var_count(compiler)
                    {
                        FlowInlineCleanCache::record(compiler, fn_node, &start_stamp, false);
                        continue;
                    }
                    let (did_inline, rejected_by_outer) = self.inline_function_body(
                        compiler,
                        &mut scope_creator,
                        fn_node,
                        body,
                        block_scope,
                        analysis,
                    );
                    Self::after_work(
                        compiler,
                        fn_node,
                        &start_stamp,
                        did_inline,
                        rejected_by_outer,
                        &verify_clean,
                    );
                }
                return;
            }
            // Rust-only (performance divergence): the candidates `PreparedFlowJob::screen` finds
            // nothing in need no scopes and no work, only the clean-leaf record a leaf would get
            // (with `verify`, they get both, and the work must find nothing).
            let screened: IndexSet<NodeId> = if mode == ParallelAnalysisMode::Off {
                IndexSet::default()
            } else {
                let ast = &compiler.ast;
                let convention = compiler.get_coding_convention();
                let threads = crate::parallel_parse::parallel_thread_count(candidates.len(), 64, 8);
                crate::parallel_parse::run_scoped_indexed(
                    "closure-fsiv",
                    threads,
                    &candidates,
                    |_, &(fn_node, body, analysis, is_leaf, _)| {
                        crate::parallel_parse::catch_unwind_quietly(|| {
                            PreparedFlowJob::screen(
                                ast,
                                convention,
                                can_continue_after_errors,
                                fn_node,
                                body,
                                analysis,
                                is_leaf,
                            )
                        })
                        .flatten()
                        .filter(|&may_inline| !may_inline)
                        .map(|_| fn_node)
                    },
                )
                .into_iter()
                .flatten()
                .map(|(_, fn_node)| fn_node)
                .collect()
            };
            let candidates: Vec<_> = if verify {
                candidates
            } else {
                let mut rest = Vec::with_capacity(candidates.len() - screened.len());
                for candidate in candidates {
                    if !screened.contains(&candidate.0) || verify_clean.contains(&candidate.0) {
                        rest.push(candidate);
                    } else {
                        FlowInlineCleanCache::record(compiler, candidate.0, &candidate.4, false);
                    }
                }
                rest
            };
            const CHUNK_SIZE: usize = 4096;
            for chunk in candidates.chunks(CHUNK_SIZE) {
                let mut jobs = Vec::with_capacity(chunk.len());
                for (fn_node, body, analysis, is_leaf, start_stamp) in chunk {
                    let (fn_node, body, analysis, is_leaf) = (*fn_node, *body, *analysis, *is_leaf);
                    let start_stamp = start_stamp.clone();
                    let fn_scope = ensure_ancestor_scope(
                        compiler,
                        &mut scope_creator,
                        &mut scope_memo,
                        global_scope_root,
                        fn_node,
                    );
                    let block_scope = scope_creator.create_scope(compiler, body, Some(fn_scope));
                    if verify && screened.contains(&fn_node) {
                        let all_vars = NodeUtil::get_all_vars_declared_in_function(
                            compiler,
                            &mut scope_creator,
                            fn_scope,
                        );
                        let mut escaped = IndexSet::default();
                        crate::data_flow_analysis::compute_escaped(
                            compiler,
                            fn_scope,
                            &mut escaped,
                            &mut scope_creator,
                            all_vars.get_all_variables(),
                        );
                        FunctionVars::collect(&compiler.ast, fn_node)
                            .unwrap()
                            .verify(
                                compiler,
                                &all_vars,
                                Some((
                                    fn_scope.get_var_count(compiler),
                                    block_scope.get_var_count(compiler),
                                )),
                                &escaped,
                            );
                    }
                    if live_variables_analysis::MAX_VARIABLES_TO_ANALYZE
                        < block_scope.get_var_count(compiler)
                    {
                        FlowInlineCleanCache::record(compiler, fn_node, &start_stamp, false);
                        continue;
                    }
                    let all_vars = NodeUtil::get_all_vars_declared_in_function(
                        compiler,
                        &mut scope_creator,
                        fn_scope,
                    );
                    let (all_vars_in_fn, _) = all_vars.into_parts();
                    let mut escaped = IndexSet::default();
                    compute_escaped(
                        compiler,
                        fn_scope,
                        &mut escaped,
                        &mut scope_creator,
                        &all_vars_in_fn,
                    );
                    let mut catch_vars = IndexSet::default();
                    let mut exported_local_names = IndexSet::default();
                    for (name, &var) in &all_vars_in_fn {
                        if var.get_scope(compiler).is_catch_scope(compiler) {
                            catch_vars.insert(var);
                        }
                        if compiler
                            .get_coding_convention()
                            .is_exported(name, /* local= */ true)
                        {
                            exported_local_names.insert(name.clone());
                        }
                    }
                    let prepared = PreparedMustDef::new(compiler, &escaped, all_vars_in_fn);
                    jobs.push(PreparedFlowJob {
                        fn_node,
                        body,
                        analysis,
                        is_leaf,
                        start_stamp,
                        prepared,
                        catch_vars,
                        exported_local_names,
                    });
                }
                let threads = crate::parallel_parse::parallel_thread_count(jobs.len(), 64, 8);
                if threads < 2 {
                    for job in jobs {
                        check_state!(
                            !(verify
                                && screened.contains(&job.fn_node)
                                && job.might_inline(&compiler.ast, can_continue_after_errors)),
                            "CLOSURE_RS_PARALLEL_ANALYSIS=verify: FlowSensitiveInlineVariables screened out %s",
                            job.fn_node.to_string(&compiler.ast)
                        );
                        let fn_scope = ensure_ancestor_scope(
                            compiler,
                            &mut scope_creator,
                            &mut scope_memo,
                            global_scope_root,
                            job.fn_node,
                        );
                        let block_scope =
                            scope_creator.create_scope(compiler, job.body, Some(fn_scope));
                        let (did_inline, rejected_by_outer) = self.inline_function_body(
                            compiler,
                            &mut scope_creator,
                            job.fn_node,
                            job.body,
                            block_scope,
                            job.analysis,
                        );
                        Self::after_work(
                            compiler,
                            job.fn_node,
                            &job.start_stamp,
                            did_inline,
                            rejected_by_outer,
                            &verify_clean,
                        );
                    }
                    continue;
                }
                let ast = &compiler.ast;
                let might_inline: Vec<std::sync::atomic::AtomicBool> = (0..jobs.len())
                    .map(|_| std::sync::atomic::AtomicBool::new(false))
                    .collect();
                crate::parallel_parse::run_scoped_for_each(
                    "closure-fsiv",
                    threads,
                    &jobs,
                    |idx, job| {
                        if job.might_inline(ast, can_continue_after_errors) {
                            might_inline[idx].store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                    },
                );
                let mut survivors = Vec::new();
                for (job, flag) in jobs.into_iter().zip(might_inline) {
                    let flag = flag.into_inner();
                    check_state!(
                        !(verify && flag && screened.contains(&job.fn_node)),
                        "CLOSURE_RS_PARALLEL_ANALYSIS=verify: FlowSensitiveInlineVariables screened out %s",
                        job.fn_node.to_string(&compiler.ast)
                    );
                    if flag {
                        survivors.push((
                            job.fn_node,
                            job.body,
                            job.analysis,
                            job.is_leaf,
                            job.start_stamp,
                        ));
                    } else {
                        FlowInlineCleanCache::record(
                            compiler,
                            job.fn_node,
                            &job.start_stamp,
                            false,
                        );
                    }
                }
                if !survivors.is_empty() {
                    scope_memo.clear();
                    for (fn_node, body, analysis, _, start_stamp) in survivors {
                        let fn_scope = ensure_ancestor_scope(
                            compiler,
                            &mut scope_creator,
                            &mut scope_memo,
                            global_scope_root,
                            fn_node,
                        );
                        let block_scope =
                            scope_creator.create_scope(compiler, body, Some(fn_scope));
                        let (did_inline, rejected_by_outer) = self.inline_function_body(
                            compiler,
                            &mut scope_creator,
                            fn_node,
                            body,
                            block_scope,
                            analysis,
                        );
                        Self::after_work(
                            compiler,
                            fn_node,
                            &start_stamp,
                            did_inline,
                            rejected_by_outer,
                            &verify_clean,
                        );
                    }
                }
            }
            return;
        }
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback(self)
            .traverse_roots(externs, root);
    }
}

impl Callback for FlowSensitiveInlineVariables {
    // port: FlowSensitiveInlineVariables#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        !n.is_script(t) || !t.get_input().unwrap().is_extern()
    }

    // port: FlowSensitiveInlineVariables#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {
        // TODO(user): While the helpers do a subtree traversal on the AST, the
        // compiler pass itself only traverse the AST to look for function
        // declarations to perform dataflow analysis on. We could combine
        // the traversal in DataFlowAnalysis's computeEscaped later to save some
        // time.
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for FlowSensitiveInlineVariables {
    // port: FlowSensitiveInlineVariables#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if t.in_global_scope() {
            return; // Don't even brother. All global variables are likely escaped.
        }

        if !t.in_function_block_scope() {
            return; // Only want to do the following if its a function block scope.
        }

        let scope = t.get_scope();

        if live_variables_analysis::MAX_VARIABLES_TO_ANALYZE < scope.get_var_count(t.get_compiler())
        {
            return;
        }

        let function_scope_root = t.get_scope_root().unwrap().get_parent(t).unwrap();

        if !Self::is_candidate_function(t, function_scope_root) {
            return;
        }
        let compiler = t.get_compiler();
        // A normalized function can be wide without making the lookahead recursive calls
        // deeper. Keep the original small scan for callers that can observe export queries
        // or supply a tree before normalization.
        let node_budget = if compiler.get_life_cycle_stage().is_normalized()
            && compiler.get_coding_convention().export_tests_are_pure()
        {
            usize::MAX
        } else {
            128
        };
        let analysis = InlineAnalysis::for_body(t, t.get_scope_root().unwrap(), node_budget);
        if matches!(analysis, InlineAnalysis::NoCandidate) {
            return;
        }

        let scope_root = t.get_scope_root().unwrap();
        let (compiler, scope_creator) = t.get_compiler_and_scope_creator();

        // Compute the forward reaching definition.
        let cfg = ControlFlowAnalysis::builder()
            .set_compiler(compiler)
            .set_cfg_root(function_scope_root)
            .set_include_edge_annotations(true)
            .compute_cfg(compiler);

        let mut escaped: IndexSet<VarId> = IndexSet::<_>::default();
        let scope_parent = scope.get_parent(compiler).unwrap();
        let all_vars_declared_in_function =
            NodeUtil::get_all_vars_declared_in_function(compiler, scope_creator, scope_parent);
        let (all_vars_in_fn, _) = all_vars_declared_in_function.into_parts();
        compute_escaped(
            compiler,
            scope_parent,
            &mut escaped,
            scope_creator,
            &all_vars_in_fn,
        );

        let mut reaching_def =
            MustBeReachingVariableDef::new(compiler, cfg, escaped, all_vars_in_fn);
        reaching_def.analyze_indexed(compiler);
        let mut candidates: VecDeque<Candidate> = VecDeque::new();

        // Using the forward reaching definition search to find all the inline
        // candidates.
        // Rust-only (performance divergence): walk directly over `&AbstractCompiler` instead of
        // allocating a `NodeTraversal` per function and per CFG node.
        gather_candidates_shallow(compiler, &reaching_def, &mut candidates, scope_root, None);
        if candidates.is_empty() {
            return;
        }

        // Compute the backward reaching use. The CFG and per-function variable info can be
        // reused.
        let mut reaching_uses = reaching_def.into_use_analysis();
        let mut uses_ready = matches!(analysis, InlineAnalysis::Full);
        if uses_ready {
            reaching_uses.analyze_indexed(compiler);
        }
        while let Some(mut c) = candidates.pop_front() {
            let candidate_var = check_not_null!(reaching_uses.variable(&c.var_name));
            let candidate_scope = candidate_var.get_scope(compiler);
            if !c.find_definition_and_single_read(compiler) {
                continue;
            }
            // Every replacement asks for reaching uses first, so the analysis still sees the
            // original AST and graph even when earlier candidates failed a local check.
            if !uses_ready {
                reaching_uses.analyze_indexed(compiler);
                uses_ready = true;
            }
            let mut _rejected = false;
            if c.can_inline(
                compiler,
                &mut reaching_uses,
                &self.side_effect_predicate,
                candidate_scope,
                &mut _rejected,
            ) {
                c.inline_variable(compiler);

                // If candidate "c" has dependencies, then inlining it may have introduced new
                // dependencies for our other inlining candidates. MustBeReachingVariableDef uses
                // a dependency graph in its analysis. Generating a new dependency graph will need
                // another CFG computation. Ideally we should iterate to a fixed point, but that
                // can be costly. Therefore, we use a conservative heuristic here: For each
                // candidate "other", we back off if its set of dependencies cannot contain all of
                // "c"'s dependencies.
                if !c.def_metadata.depends.is_empty() {
                    candidates.retain(|other| {
                        let c_var = scope.get_var(compiler, c.var_name.clone());
                        let remove = var_set_contains(compiler, &other.def_metadata.depends, c_var)
                            && !var_set_contains_all(
                                compiler,
                                &other.def_metadata.depends,
                                &c.def_metadata.depends,
                            );
                        !remove
                    });
                }
            }
        }
    }

    // port: FlowSensitiveInlineVariables#exitScope
    fn exit_scope(&mut self, _t: &mut NodeTraversal<'_>) {}
}

/// Java's `Set<Var>#contains`: `Var` inherits `ScopedName#equals` (name and scope root).
fn var_set_contains(
    compiler: &AbstractCompiler,
    set: &IndexSet<VarId>,
    var: Option<VarId>,
) -> bool {
    var.is_some_and(|var| set.iter().any(|&element| element.equals(compiler, var)))
}

/// Java's `Set<Var>#containsAll` with `ScopedName#equals`.
fn var_set_contains_all(
    compiler: &AbstractCompiler,
    set: &IndexSet<VarId>,
    other: &IndexSet<VarId>,
) -> bool {
    other
        .iter()
        .all(|&var| var_set_contains(compiler, set, Some(var)))
}

// port: FlowSensitiveInlineVariables.GatherCandidatesCfgNodeCallback#visit
fn gather_candidates_in_cfg_node(
    compiler: &AbstractCompiler,
    reaching_def: &MustBeReachingVariableDef,
    candidates: &mut VecDeque<Candidate>,
    cfg_node: NodeId,
    n: NodeId,
    parent: NodeId,
) {
    if ControlFlowGraph::is_entering_new_cfg_node(compiler, n) {
        return;
    }
    let mut child = n.get_first_child(compiler);
    while let Some(c) = child {
        gather_candidates_in_cfg_node(compiler, reaching_def, candidates, cfg_node, c, n);
        child = c.get_next(compiler);
    }
    if n.is_name(compiler) {
        // Make sure that the name node is purely a read.
        if (NodeUtil::is_assignment_op(compiler, parent)
            && parent.get_first_child(compiler) == Some(n))
            || NodeUtil::is_name_declaration(compiler, Some(parent))
            || parent.is_inc(compiler)
            || parent.is_dec(compiler)
            || parent.is_param_list(compiler)
            || parent.is_catch(compiler)
            || NodeUtil::is_lhs_by_destructuring(compiler, n)
        {
            return;
        }

        let name = n.get_string_ref(compiler);
        // This pass only runs on local scopes.
        if compiler
            .get_coding_convention()
            .is_exported(name, /* local= */ true)
        {
            return;
        }

        let def = reaching_def.get_def(name, cfg_node);
        // TODO(nicksantos): We need to add some notion of @const outer
        // scope vars. We can inline those just fine.
        if let Some(def) = def
            && !reaching_def.depends_on_outer_scope_vars(compiler, &def)
        {
            candidates.push_back(Candidate::new(compiler, name.clone(), def, n, cfg_node));
        }
    }
}

/// Gathers a list of possible candidates for inlining based only on information from
/// [`MustBeReachingVariableDef`]. The list will be stored in `candidates` and the validity of
/// each inlining Candidate should be later verified with `Candidate::can_inline` when
/// [`MaybeReachingVariableUse`] has been performed.
// port: FlowSensitiveInlineVariables.GatherCandidates#visit
fn gather_candidates_shallow(
    compiler: &AbstractCompiler,
    reaching_def: &MustBeReachingVariableDef,
    candidates: &mut VecDeque<Candidate>,
    n: NodeId,
    parent: Option<NodeId>,
) {
    if let Some(p) = parent
        && p.is_function(compiler)
        && Some(n) != p.get_first_child(compiler)
    {
        return;
    }
    let mut child = n.get_first_child(compiler);
    while let Some(c) = child {
        gather_candidates_shallow(compiler, reaching_def, candidates, c, Some(n));
        child = c.get_next(compiler);
    }
    // The root itself is never a candidate read: its callback receives no parent.
    // A container whose children all start new CFG nodes has no other visits.
    if !n.has_children(compiler)
        || ControlFlowGraph::<NodeId>::children_start_new_cfg_nodes(n.get_token(compiler))
    {
        return;
    }
    if reaching_def.get_cfg().get_node(&Some(n)).is_none() {
        // Not a CFG node.
        return;
    }
    let mut child = n.get_first_child(compiler);
    while let Some(c) = child {
        gather_candidates_in_cfg_node(compiler, reaching_def, candidates, n, c, n);
        child = c.get_next(compiler);
    }
}

#[allow(clippy::too_many_arguments)]
fn gather_candidates_in_cfg_node_readonly(
    ast: &Ast,
    reaching_def: &BorrowedMustDef<'_>,
    catch_vars: &IndexSet<VarId>,
    exported_local_names: &IndexSet<JsString>,
    candidates: &mut Vec<Candidate>,
    cfg_node: NodeId,
    n: NodeId,
    parent: NodeId,
) {
    if ControlFlowGraph::is_entering_new_cfg_node(ast, n) {
        return;
    }
    let mut child = n.get_first_child(ast);
    while let Some(c) = child {
        gather_candidates_in_cfg_node_readonly(
            ast,
            reaching_def,
            catch_vars,
            exported_local_names,
            candidates,
            cfg_node,
            c,
            n,
        );
        child = c.get_next(ast);
    }
    if n.is_name(ast) {
        if (NodeUtil::is_assignment_op(ast, parent) && parent.get_first_child(ast) == Some(n))
            || NodeUtil::is_name_declaration(ast, Some(parent))
            || parent.is_inc(ast)
            || parent.is_dec(ast)
            || parent.is_param_list(ast)
            || parent.is_catch(ast)
            || NodeUtil::is_lhs_by_destructuring(ast, n)
        {
            return;
        }
        let name = n.get_string_ref(ast);
        if exported_local_names.contains(name) {
            return;
        }
        if let Some(def) = reaching_def.get_def(name, cfg_node)
            && !def.unknown_dependencies_flag()
            && !def.depends.iter().any(|v| catch_vars.contains(v))
        {
            candidates.push(Candidate::new(ast, name.clone(), def, n, cfg_node));
        }
    }
}

fn gather_candidates_shallow_readonly(
    ast: &Ast,
    reaching_def: &BorrowedMustDef<'_>,
    catch_vars: &IndexSet<VarId>,
    exported_local_names: &IndexSet<JsString>,
    candidates: &mut Vec<Candidate>,
    n: NodeId,
    parent: Option<NodeId>,
) {
    if let Some(p) = parent
        && p.is_function(ast)
        && Some(n) != p.get_first_child(ast)
    {
        return;
    }
    let mut child = n.get_first_child(ast);
    while let Some(c) = child {
        gather_candidates_shallow_readonly(
            ast,
            reaching_def,
            catch_vars,
            exported_local_names,
            candidates,
            c,
            Some(n),
        );
        child = c.get_next(ast);
    }
    if !n.has_children(ast)
        || ControlFlowGraph::<NodeId>::children_start_new_cfg_nodes(n.get_token(ast))
    {
        return;
    }
    if reaching_def.cfg().get_node(&Some(n)).is_none() {
        return;
    }
    let mut child = n.get_first_child(ast);
    while let Some(c) = child {
        gather_candidates_in_cfg_node_readonly(
            ast,
            reaching_def,
            catch_vars,
            exported_local_names,
            candidates,
            n,
            c,
            n,
        );
        child = c.get_next(ast);
    }
}

/// Models the connection between a definition and a use of that definition.
struct Candidate {
    // Name of the variable.
    var_name: JsString,

    // Nodes related to the definition.
    def: Option<NodeId>,
    def_metadata: Arc<Definition>,

    // Nodes related to the use.
    use_: NodeId,
    use_cfg_node: NodeId,

    // Number of uses of the variable within the current CFG node.
    num_uses_within_cfg_node: i32,
}

impl Candidate {
    // port: FlowSensitiveInlineVariables.Candidate#Candidate
    fn new(
        ast: &Ast,
        var_name: JsString,
        def_metadata: Arc<Definition>,
        use_: NodeId,
        use_cfg_node: NodeId,
    ) -> Self {
        check_argument!(use_.is_name(ast));
        Self {
            var_name,
            def: None,
            def_metadata,
            use_,
            use_cfg_node,
            num_uses_within_cfg_node: 0,
        }
    }

    // port: FlowSensitiveInlineVariables.Candidate#getDefCfgNode
    fn get_def_cfg_node(&self) -> NodeId {
        self.def_metadata.node
    }

    /// Read-only AST screening matching the cheap structural checks of
    /// `find_definition_and_single_read`, safe on worker threads.
    fn quick_check_ast(&mut self, ast: &Ast) -> bool {
        if self.get_def_cfg_node().is_function(ast) {
            return false;
        }
        if NodeUtil::is_within_loop(ast, self.use_) {
            return false;
        }
        self.get_definition(ast, self.get_def_cfg_node());
        let Some(def) = self.def else {
            return false;
        };
        if def.is_assign(ast) && !NodeUtil::is_expr_assign(ast, def.get_parent(ast).unwrap()) {
            return false;
        }
        let rhs = def.get_last_child(ast).unwrap();
        if NodeUtil::has(ast, rhs, &Self::forbidden_rhs_node, &|ast, n| {
            !n.is_function(ast)
        }) {
            return false;
        }
        self.get_num_use_in_use_cfg_node(ast, self.use_cfg_node);
        self.num_uses_within_cfg_node == 1
    }

    /// The part of canInline that only needs the definition and the use's own CFG node.
    fn find_definition_and_single_read(&mut self, compiler: &mut AbstractCompiler) -> bool {
        // Cannot inline a parameter.
        if self.get_def_cfg_node().is_function(compiler) {
            return false;
        }

        if NodeUtil::is_within_loop(compiler, self.use_) {
            return false;
        }
        self.get_definition(compiler, self.get_def_cfg_node());

        // Definition was not found.
        let Some(def) = self.def else {
            return false;
        };

        // Check that the assignment isn't used as a R-Value.
        // TODO(user): Certain cases we can still inline.
        if def.is_assign(compiler)
            && !NodeUtil::is_expr_assign(compiler, def.get_parent(compiler).unwrap())
        {
            return false;
        }

        let rhs = def.get_last_child(compiler).unwrap();
        if Self::forbidden_rhs_node(compiler, rhs) {
            return false;
        }
        self.get_num_use_in_use_cfg_node(compiler, self.use_cfg_node);
        if self.num_uses_within_cfg_node != 1 {
            return false;
        }

        let mut names_to_check: IndexSet<JsString> = IndexSet::<_>::default();
        for &var in &self.def_metadata.depends {
            names_to_check.insert(var.get_name(compiler));
        }

        let side_effect_predicate_with_names = SideEffectPredicate::new_with_names(names_to_check);

        // A subexpression evaluated after the variable has a side effect.
        // Example, for x:
        // x = readProp(b), modifyProp(b); print(x);
        if check_post_expressions(
            compiler,
            def,
            self.get_def_cfg_node(),
            &side_effect_predicate_with_names,
        ) {
            return false;
        }

        // Similar check as the above but this time, all the sub-expressions
        // evaluated before the variable.
        // x = readProp(b); modifyProp(b), print(x);
        if check_pre_expressions(
            compiler,
            self.use_,
            self.use_cfg_node,
            &side_effect_predicate_with_names,
        ) {
            return false;
        }

        // TODO(user): Side-effect is OK sometimes. As long as there are no
        // side-effect function down all paths to the use. Once we have all the
        // side-effect analysis tool.
        let ast_analyzer = compiler.get_ast_analyzer();
        if ast_analyzer.may_have_side_effects(compiler, rhs) {
            return false;
        }

        true
    }

    // port: FlowSensitiveInlineVariables.Candidate#canInline
    fn can_inline(
        &mut self,
        compiler: &mut AbstractCompiler,
        reaching_uses: &mut MaybeReachingVariableUse,
        side_effect_predicate: &SideEffectPredicate,
        scope: ScopeId,
        rejected_by_outer_scope: &mut bool,
    ) -> bool {
        if !reaching_uses.has_single_use(&self.var_name, self.get_def_cfg_node()) {
            return false;
        }

        if !self.is_rhs_safe_to_inline(compiler, scope, rejected_by_outer_scope) {
            return false;
        }

        // We can skip the side effect check along the paths of two nodes if
        // they are just next to each other.
        if NodeUtil::is_statement_block(
            compiler,
            self.get_def_cfg_node().get_parent(compiler).unwrap(),
        ) && self.get_def_cfg_node().get_next(compiler) != Some(self.use_cfg_node)
        {
            // Similar side effect check as above but this time the side effect is
            // else where along the path.
            // x = readProp(b); while(modifyProp(b)) {}; print(x);
            let cfg = reaching_uses.get_cfg_mut();
            let def_graph_node = cfg.get_node(&Some(self.get_def_cfg_node())).unwrap();
            let use_graph_node = cfg.get_node(&Some(self.use_cfg_node)).unwrap();
            let compiler_cell = RefCell::new(&mut *compiler);
            let path_check = CheckPathsBetweenNodes::with_inclusive(
                def_graph_node,
                use_graph_node,
                |n: &Option<NodeId>| {
                    side_effect_predicate.apply(&mut compiler_cell.borrow_mut(), *n)
                },
                |_: &ControlFlowGraph<NodeId>, _| true,
                false,
            );
            if path_check.some_paths_satisfy_predicate(cfg) {
                return false;
            }
        }

        true
    }

    /// Actual transformation.
    // port: FlowSensitiveInlineVariables.Candidate#inlineVariable
    fn inline_variable(&mut self, compiler: &mut AbstractCompiler) {
        let def = self.def.unwrap();
        let mut def_parent = def.get_parent(compiler).unwrap();
        let use_parent = self.use_.get_parent(compiler).unwrap();
        if def.is_assign(compiler) {
            let rhs = def.get_last_child(compiler).unwrap();
            rhs.detach(compiler);
            // Oh yes! I have grandparent to remove this.
            check_state!(def_parent.is_expr_result(compiler));
            while def_parent.get_parent(compiler).unwrap().is_label(compiler) {
                def_parent = def_parent.get_parent(compiler).unwrap();
            }
            compiler.report_change_to_enclosing_scope(def_parent);
            def_parent.detach(compiler);
            self.use_.replace_with(compiler, rhs);
        } else if NodeUtil::is_name_declaration(compiler, Some(def_parent)) {
            let rhs = def.get_last_child(compiler).unwrap();
            if def_parent.is_const(compiler) {
                // If it is a const var we don't want to remove the rhs of the variable
                let undefined = compiler.new_string_with_token(Token::NAME, "undefined");
                rhs.replace_with(compiler, undefined);
                self.use_.replace_with(compiler, rhs);
            } else {
                rhs.detach(compiler);
                self.use_.replace_with(compiler, rhs);
            }
        } else {
            panic!("No other definitions can be inlined.");
        }
        compiler.report_change_to_enclosing_scope(use_parent);
    }

    /// Set the def node
    ///
    /// `n`: A node that has a corresponding CFG node in the CFG.
    // port: FlowSensitiveInlineVariables.Candidate#getDefinition
    fn get_definition(&mut self, compiler: &Ast, n: NodeId) {
        // Rust-only (performance divergence): walk CFG node directly over `&Ast`
        // without constructing a `NodeTraversal`.
        // port: FlowSensitiveInlineVariables.Candidate#getDefinition.visit
        fn walk(
            compiler: &Ast,
            var_name: &JsString,
            def: &mut Option<NodeId>,
            n: NodeId,
            is_root: bool,
        ) {
            if !is_root && ControlFlowGraph::is_entering_new_cfg_node(compiler, n) {
                return;
            }
            let mut child = n.get_first_child(compiler);
            while let Some(c) = child {
                walk(compiler, var_name, def, c, false);
                child = c.get_next(compiler);
            }
            match n.get_token(compiler) {
                Token::NAME
                    if n.get_string_ref(compiler) == var_name && n.has_children(compiler) =>
                {
                    *def = Some(n);
                }
                Token::ASSIGN => {
                    let lhs = n.get_first_child(compiler).unwrap();
                    if lhs.is_name(compiler) && lhs.get_string_ref(compiler) == var_name {
                        *def = Some(n);
                    }
                }
                _ => {}
            }
        }
        walk(compiler, &self.var_name, &mut self.def, n, true);
    }

    /// Computes the number of uses of the variable varName and store it in
    /// numUseWithinUseCfgNode.
    // port: FlowSensitiveInlineVariables.Candidate#getNumUseInUseCfgNode
    fn get_num_use_in_use_cfg_node(&mut self, compiler: &Ast, cfg_node: NodeId) {
        self.num_uses_within_cfg_node = 0;
        // port: FlowSensitiveInlineVariables.Candidate#getNumUseInUseCfgNode.isAssignChain
        fn is_assign_chain(ast: &Ast, child: NodeId, ancestor: NodeId) -> bool {
            let mut n = child;
            while n != ancestor {
                if !n.is_assign(ast) {
                    return false;
                }
                n = n.get_parent(ast).unwrap();
            }
            true
        }
        // port: FlowSensitiveInlineVariables.Candidate#getNumUseInUseCfgNode.visit
        fn walk(
            compiler: &Ast,
            var_name: &JsString,
            cfg_node: NodeId,
            count: &mut i32,
            n: NodeId,
            parent: Option<NodeId>,
        ) {
            if parent.is_some() && ControlFlowGraph::is_entering_new_cfg_node(compiler, n) {
                return;
            }
            let mut child = n.get_first_child(compiler);
            while let Some(c) = child {
                walk(compiler, var_name, cfg_node, count, c, Some(n));
                child = c.get_next(compiler);
            }
            if n.is_name(compiler) && n.get_string_ref(compiler) == var_name {
                // We make a special exception when the entire cfgNode is a chain
                // of assignments, since in that case the assignment statements
                // will happen after the inlining of the right hand side.
                let parent = parent.unwrap();
                if parent.is_assign(compiler)
                    && parent.get_first_child(compiler) == Some(n)
                    && is_assign_chain(compiler, parent, cfg_node)
                {
                    // Don't count lhs of top-level assignment chain
                } else {
                    *count += 1;
                }
            }
        }
        walk(
            compiler,
            &self.var_name,
            cfg_node,
            &mut self.num_uses_within_cfg_node,
            cfg_node,
            None,
        );
    }

    fn forbidden_rhs_node(ast: &Ast, n: NodeId) -> bool {
        matches!(
            n.get_token(ast),
            Token::GETELEM
                | Token::GETPROP
                | Token::OPTCHAIN_GETPROP
                | Token::OPTCHAIN_GETELEM
                | Token::CLASS
                | Token::ARRAYLIT
                | Token::OBJECTLIT
                | Token::REGEXP
                | Token::NEW
        )
    }

    /// Check if the definition we're considering inline has anything that makes inlining unsafe
    /// (that hasn't already been caught).
    ///
    /// `usage_scope`: The scope we will inline the variable into.
    // port: FlowSensitiveInlineVariables.Candidate#isRhsSafeToInline
    fn is_rhs_safe_to_inline(
        &self,
        compiler: &AbstractCompiler,
        usage_scope: ScopeId,
        rejected_by_outer_scope: &mut bool,
    ) -> bool {
        let def_last_child = self.def.unwrap().get_last_child(compiler).unwrap();
        // Don't inline definitions with an R-Value that has:
        // 1) GETELEM, OPTCHAIN_GETELEM (e.g: foo?.['bar']), GETPROP, OPTCHAIN_GETPROP (e.g:
        // foo?.bar), CLASS, ARRAYLIT,
        // OBJECTLIT, REGEXP
        // 2) anything that creates a new object.
        // Example:
        // var x = a.b.c; j.c = 1; print(x);
        // Inlining print(a.b.c) is not safe - consider if j were an alias to a.b.
        if NodeUtil::has(
            compiler,
            def_last_child,
            &Self::forbidden_rhs_node,
            &|ast, n| !n.is_function(ast),
        ) {
            return false;
        }

        // Don't inline definitions with an rvalue referencing names that are not declared in the
        // usage's scope. (Unlike the above check, this includes names referenced inside function
        // expressions in the rvalue).
        // e.g. the name "a" below in the definition of "b":
        //   {
        //     let a = 3;
        //     var b = a;
        //   }
        // return b;   // "a" is not declared in this scope so we can't inline this to "return a;"
        if NodeUtil::has(
            compiler,
            def_last_child,
            &|ast: &Ast, input: NodeId| {
                if input.is_name(ast) {
                    let name = input.get_string(ast);
                    if !name.is_empty() && !usage_scope.has_slot(compiler, name) {
                        return true; // unsafe to inline.
                    }
                }
                false
            },
            &|_: &Ast, _: NodeId| true,
        ) {
            *rejected_by_outer_scope = true;
            return false;
        }
        true
    }
}

/// Given an expression by its root and sub-expression n, return true if the predicate is true
/// for some expression evaluated after n.
///
/// NOTE: this doesn't correctly check destructuring patterns, because their order of evaluation
/// is different from AST traversal order, but currently this is ok because
/// FlowSensitiveInlineVariables never inlines variable assignments inside destructuring.
///
/// Example:
///
/// NotChecked(), NotChecked(), n, Checked(), Checked();
// port: FlowSensitiveInlineVariables#checkPostExpressions
fn check_post_expressions(
    compiler: &mut AbstractCompiler,
    n: NodeId,
    expression_root: NodeId,
    predicate: &SideEffectPredicate,
) -> bool {
    let mut p = n;
    while p != expression_root {
        let mut cur = p.get_next(compiler);
        while let Some(c) = cur {
            if predicate.apply(compiler, Some(c)) {
                return true;
            }
            cur = c.get_next(compiler);
        }
        p = p.get_parent(compiler).unwrap();
    }
    false
}

/// Given an expression by its root and sub-expression n, return true if the predicate is true
/// for some expression evaluated before n.
///
/// In most cases evaluation order follows left-to-right AST order. Destructuring pattern
/// evaluation is an exception.
///
/// Example:
///
/// Checked(), Checked(), n, NotChecked(), NotChecked();
// port: FlowSensitiveInlineVariables#checkPreExpressions
fn check_pre_expressions(
    compiler: &mut AbstractCompiler,
    n: NodeId,
    expression_root: NodeId,
    predicate: &SideEffectPredicate,
) -> bool {
    let mut p = n;
    while p != expression_root {
        let oldest_sibling = p
            .get_parent(compiler)
            .unwrap()
            .get_first_child(compiler)
            .unwrap();
        // Evaluate a destructuring assignment right-to-left.
        if oldest_sibling.is_destructuring_pattern(compiler) {
            if p.is_destructuring_pattern(compiler)
                && let Some(next) = p.get_next(compiler)
                && predicate.apply(compiler, Some(next))
            {
                return true;
            }
            p = p.get_parent(compiler).unwrap();
            continue;
        }
        let mut cur = oldest_sibling;
        while cur != p {
            if predicate.apply(compiler, Some(cur)) {
                return true;
            }
            cur = cur.get_next(compiler).unwrap();
        }
        p = p.get_parent(compiler).unwrap();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use closure_rhino::ir::IR;

    #[test]
    fn lookahead_handles_wide_bodies_and_falls_back_on_deep_expressions() {
        let mut ast = Ast::new();
        let body = IR::block(&mut ast);
        let local = IR::name(&mut ast, "local");
        let value = IR::number(&mut ast, 42.0);
        let declaration = IR::var_with_value(&mut ast, local, value);
        body.add_child_to_back(&mut ast, declaration);
        for _ in 0..256 {
            let empty = IR::empty(&mut ast);
            body.add_child_to_back(&mut ast, empty);
        }
        assert!(matches!(
            InlineAnalysis::for_body(&ast, body, 128),
            InlineAnalysis::Full
        ));
        assert!(matches!(
            InlineAnalysis::for_body(&ast, body, usize::MAX),
            InlineAnalysis::NoCandidate
        ));

        let local = IR::name(&mut ast, "local");
        let read = IR::expr_result(&mut ast, local);
        body.add_child_to_back(&mut ast, read);
        assert!(matches!(
            InlineAnalysis::for_body(&ast, body, usize::MAX),
            InlineAnalysis::DeferUses
        ));

        let mut expr = IR::name(&mut ast, "deep");
        for _ in 0..128 {
            expr = IR::not(&mut ast, expr);
        }
        let deep = IR::expr_result(&mut ast, expr);
        body.add_child_to_back(&mut ast, deep);
        assert!(matches!(
            InlineAnalysis::for_body(&ast, body, usize::MAX),
            InlineAnalysis::Full
        ));
    }
}

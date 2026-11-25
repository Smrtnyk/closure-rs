/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CoalesceVariableNames.java,
//   src/com/google/javascript/jscomp/MemoizedScopeCreator.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `CoalesceVariableNames.java`.
//!
//! Reuse variable names if possible.
//!
//! For example, from `var x = 1; print(x); var y = 2; print(y);` to
//! `var x = 1; print(x); x = 2; print(x)`. The benefits are slightly shorter code because of the
//! removed `var` declaration, less unique variables in hope for better renaming, and finally
//! better gzip compression.
//!
//! The pass operates similar to a typical register allocator found in an optimizing compiler by
//! first computing live ranges with `LiveVariablesAnalysis` and a variable interference graph.
//! It colors the graph in the same order as `GreedyGraphColoring` to determine which variable
//! names can be merged safely.
use crate::{
    AbstractCompiler, AllVarsDeclaredInFunction,
    abstract_compiler::LifeCycleStage,
    ast_factory::AstFactory,
    compiler_pass::CompilerPass,
    control_flow_analysis::ControlFlowAnalysis,
    control_flow_graph::ControlFlowGraph,
    data_flow_analysis::DataFlowAnalysis,
    graph::{adjacency_graph::AdjacencyGraph, graph_node::GraphNode},
    live_variables_analysis::{
        LiveVariableLattice, LiveVariablesAnalysis, MAX_VARIABLES_TO_ANALYZE, PreparedLiveness,
    },
    memoized_scope_creator::MemoizedScopeCreator,
    node_traversal::{Callback, NodeTraversal, ScopedCallback},
    node_util::NodeUtil,
    scope::ScopeId,
    scope_creator::ScopeCreator,
    syntactic_scope_creator::{SyntacticScopeCreator, ensure_ancestor_scope},
    var::VarId,
};
use closure_parsing::parser::feature_set::FeatureSet;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use std::{cell::RefCell, collections::BTreeSet, collections::VecDeque, rc::Rc};

// Rust-only: Java shares one MemoizedScopeCreator object between this pass (the field) and its
// NodeTraversal. The traversal borrows its creator mutably while it runs, so both hold this
// shared handle.
#[derive(Clone)]
struct SharedScopeCreator(Rc<RefCell<MemoizedScopeCreator<'static>>>);

impl ScopeCreator for SharedScopeCreator {
    // port: MemoizedScopeCreator#createScope
    fn create_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<ScopeId>,
    ) -> ScopeId {
        self.0.borrow_mut().create_scope(compiler, n, parent)
    }
}

// Rust-only (performance divergence): the pass only analyzes functions below the Java
// variable limit. Each row of the interference graph fits in one mask.
const _: () = assert!(MAX_VARIABLES_TO_ANALYZE <= u128::BITS as i32);

struct Coloring {
    colors: IndexMap<VarId, usize>,
    representatives: Vec<Option<VarId>>,
}

impl Coloring {
    /// GreedyGraphColoring colors each maximal independent set in stable order.
    fn from_interference(
        variables: &[VarId],
        neighbors: &[u128],
        eligible: u128,
        tie_breaker: impl Fn(VarId) -> i32,
    ) -> Self {
        let mut order = Vec::new();
        for (index, &var) in variables.iter().enumerate() {
            let bit = 1u128 << index;
            if eligible & bit != 0 {
                // LinkedUndirectedGraph counts both ends of a self-edge in the degree.
                let degree = neighbors[index].count_ones() + u32::from(neighbors[index] & bit != 0);
                order.push((index, std::cmp::Reverse(degree), tie_breaker(var)));
            }
        }
        order.sort_by_key(|&(_, degree, tie)| (degree, tie));

        let mut colors = IndexMap::with_capacity_and_hasher(order.len(), Default::default());
        let mut representatives = Vec::new();
        let mut remaining = eligible;
        while remaining != 0 {
            let mut blocked = 0;
            for &(index, _, _) in &order {
                let bit = 1u128 << index;
                if remaining & bit != 0 && blocked & bit == 0 {
                    colors.insert(variables[index], representatives.len());
                    remaining &= !bit;
                    blocked |= neighbors[index];
                }
            }
            // getPartitionSuperNode picks the first member encountered during AST traversal.
            representatives.push(None);
        }
        Self {
            colors,
            representatives,
        }
    }
}

struct FunctionColoring {
    all_vars: IndexMap<JsString, VarId>,
    ordered_vars: Vec<VarId>,
    live_uninit_loop_lets: IndexSet<VarId>,
    coloring: Coloring,
}

/// Rust-only (performance divergence): per-function inputs prepared on the main thread so CFG,
/// liveness, interference-graph construction, and greedy coloring can run concurrently on worker
/// threads without retaining CFGs or liveness lattices in memory.
struct PreparedCoalesceJob {
    fn_node: NodeId,
    prepared: PreparedLiveness,
    es3_escape_2_params: bool,
    variables_by_name: IndexMap<JsString, u128>,
    base_eligible: u128,
    parameters: u128,
    var_indices: Vec<i32>,
    uninit_loop_lets: Vec<(usize, NodeId, i32)>,
}

/// Rust-only (performance divergence): compact fixed-size coloring result returned by worker
/// threads without heap allocations (zero cross-thread frees). `color_by_var[i]` holds the color
/// index of `ordered_vars[i]` or `u8::MAX` if ineligible.
struct CompactColoringOutput {
    color_by_var: [u8; MAX_VARIABLES_TO_ANALYZE as usize],
    num_colors: u8,
    live_uninit_mask: u128,
}

impl PreparedCoalesceJob {
    fn run_borrowed(
        &self,
        ast: &Ast,
        can_continue_after_errors: bool,
    ) -> Option<CompactColoringOutput> {
        let cfg = ControlFlowAnalysis::compute_cfg_readonly(
            ast,
            can_continue_after_errors,
            self.fn_node,
            false,
            true,
        );
        let borrowed = self
            .prepared
            .run_borrowed(ast, cfg, self.es3_escape_2_params);
        let variables = self.prepared.ordered_vars();
        let mut eligible = self.base_eligible;
        let escaped_bits = borrowed.ordered_escaped_bits();
        for index in 0..variables.len() {
            if escaped_bits.get(index as i32) {
                eligible &= !(1u128 << index);
            }
        }
        if eligible.count_ones() < 2 {
            return None;
        }
        let parameters = self.parameters & eligible;
        let cfg_ref = borrowed.cfg();
        let mut neighbors = [0u128; MAX_VARIABLES_TO_ANALYZE as usize];
        let neighbors = &mut neighbors[..variables.len()];
        for cfg_node in cfg_ref.get_nodes() {
            if cfg_ref.is_implicit_return(cfg_node) {
                continue;
            }
            let state = borrowed.flow_state(cfg_node);
            let live_in = live_bits(state.get_in());
            let live_out = live_bits(state.get_out());
            mark_conflicts(neighbors, live_in, live_in);
            mark_conflicts(neighbors, live_out, live_out);
            let root = cfg_node.get_value(cfg_ref).unwrap();
            let mut live_range_checker = LiveRangeChecker {
                root,
                variables_by_name: &self.variables_by_name,
                writes: 0,
                reads: 0,
            };
            live_range_checker.check(ast, root);
            live_range_checker.set_crossing_variables(neighbors, live_out);
        }
        for (index, row) in neighbors.iter_mut().enumerate() {
            *row &= eligible;
            if parameters & (1u128 << index) != 0 {
                *row |= parameters;
            }
        }
        let mut order: [(usize, std::cmp::Reverse<u32>, i32); MAX_VARIABLES_TO_ANALYZE as usize] =
            [(0, std::cmp::Reverse(0), 0); MAX_VARIABLES_TO_ANALYZE as usize];
        let mut order_len = 0usize;
        for (index, &row) in neighbors.iter().enumerate().take(variables.len()) {
            let bit = 1u128 << index;
            if eligible & bit != 0 {
                let degree = row.count_ones() + u32::from(row & bit != 0);
                order[order_len] = (index, std::cmp::Reverse(degree), self.var_indices[index]);
                order_len += 1;
            }
        }
        let order = &mut order[..order_len];
        order.sort_by_key(|&(_, degree, tie)| (degree, tie));
        let mut color_by_var = [u8::MAX; MAX_VARIABLES_TO_ANALYZE as usize];
        let mut num_colors = 0u8;
        let mut remaining = eligible;
        while remaining != 0 {
            let mut blocked = 0u128;
            for &(index, _, _) in order.iter() {
                let bit = 1u128 << index;
                if remaining & bit != 0 && blocked & bit == 0 {
                    color_by_var[index] = num_colors;
                    remaining &= !bit;
                    blocked |= neighbors[index];
                }
            }
            num_colors += 1;
        }
        // When every color group has size 1, no variable is renamed to another representative.
        if usize::from(num_colors) == order_len {
            return None;
        }
        let mut live_uninit_mask = 0u128;
        for &(var_ord_idx, let_node, var_index) in &self.uninit_loop_lets {
            if var_index >= 0
                && let Some(cfg_node) = cfg_ref.get_node(&Some(let_node))
                && borrowed.flow_state(cfg_node).get_out().is_live(var_index)
            {
                live_uninit_mask |= 1u128 << var_ord_idx;
            }
        }
        Some(CompactColoringOutput {
            color_by_var,
            num_colors,
            live_uninit_mask,
        })
    }

    fn materialize(self, out: CompactColoringOutput) -> FunctionColoring {
        let ordered = self.prepared.ordered_vars();
        let mut colors = IndexMap::with_capacity_and_hasher(ordered.len(), Default::default());
        let mut live_uninit_loop_lets = IndexSet::default();
        for (index, &v) in ordered.iter().enumerate() {
            let color = out.color_by_var[index];
            if color != u8::MAX {
                colors.insert(v, usize::from(color));
            }
            if out.live_uninit_mask & (1u128 << index) != 0 {
                live_uninit_loop_lets.insert(v);
            }
        }
        let coloring = Coloring {
            colors,
            representatives: vec![None; usize::from(out.num_colors)],
        };
        FunctionColoring {
            all_vars: self.prepared.into_all_vars(),
            ordered_vars: Vec::new(),
            live_uninit_loop_lets,
            coloring,
        }
    }
}

pub struct CoalesceVariableNames {
    scope_creator: SharedScopeCreator,
    // An eligible function with no possible merges still hides its enclosing coloring.
    colorings: Vec<Option<FunctionColoring>>,
    use_pseudo_names: bool,
    ast_factory: AstFactory,
    /// A stack of shouldOptimizeScope results.
    should_optimize_scope_stack: VecDeque<bool>,
    // Rust-only: the cfg-root stack of the NodeTraversal.AbstractCfgCallback superclass.
    cfgs: VecDeque<NodeId>,
    /// Rust-only (performance divergence): precomputed colorings from parallel Phase 2 when
    /// `!use_pseudo_names && export_tests_are_pure()`. Absent key means `should_optimize_scope`
    /// returned `None` (too many vars).
    precomputed: Option<IndexMap<NodeId, Option<FunctionColoring>>>,
}

impl CoalesceVariableNames {
    /// `use_pseudo_names`: For debug purposes, when merging variable foo and bar to foo, rename
    /// both variable to foo_bar.
    // port: CoalesceVariableNames#CoalesceVariableNames
    pub fn new(compiler: &mut AbstractCompiler, use_pseudo_names: bool) -> Self {
        // The code is normalized at this point in the compilation process. This allows us to use
        // the fact that all variables have been given unique names. We can hoist coalesced
        // variables to VARS because we know that shadowing can't occur.
        check_state!(compiler.get_life_cycle_stage().is_normalized());

        Self {
            colorings: Vec::new(),
            use_pseudo_names,
            ast_factory: compiler.create_ast_factory(),
            scope_creator: SharedScopeCreator(Rc::new(RefCell::new(MemoizedScopeCreator::new(
                Box::new(SyntacticScopeCreator::new()),
            )))),
            should_optimize_scope_stack: VecDeque::new(),
            cfgs: VecDeque::new(),
            precomputed: None,
        }
    }

    // port: NodeTraversal.AbstractCfgCallback#getControlFlowGraph
    fn get_control_flow_graph(
        &mut self,
        compiler: &mut AbstractCompiler,
    ) -> ControlFlowGraph<NodeId> {
        let cfg_root = self.cfgs.front().copied();
        check_state!(cfg_root.is_some());
        ControlFlowAnalysis::builder()
            .set_compiler(compiler)
            .set_cfg_root(cfg_root.unwrap())
            .set_include_edge_annotations(true)
            .compute_cfg(compiler)
    }

    /// Returns populated AllVarsDeclaredInFunction object iff shouldOptimizeScope is true.
    // port: CoalesceVariableNames#shouldOptimizeScope
    fn should_optimize_scope(
        t: &mut NodeTraversal<'_>,
        scope_creator: &mut dyn ScopeCreator,
    ) -> Option<AllVarsDeclaredInFunction> {
        // TODO(user): We CAN do this in the global scope, just need to be
        // careful when something is exported. Liveness uses bit-vector for live
        // sets so I don't see compilation time will be a problem for running this
        // pass in the global scope.

        if t.get_scope_root().unwrap().is_function(t) {
            let scope = t.get_scope();
            let all_vars_declared_in_function =
                NodeUtil::get_all_vars_declared_in_function(t.get_compiler(), scope_creator, scope);
            if MAX_VARIABLES_TO_ANALYZE
                > all_vars_declared_in_function
                    .get_all_variables_in_order()
                    .len() as i32
            {
                return Some(all_vars_declared_in_function);
            }
        }

        None
    }

    // port: CoalesceVariableNames#enterScopeWithCfg
    fn enter_scope_with_cfg(&mut self, t: &mut NodeTraversal<'_>) {
        if let Some(precomputed) = &mut self.precomputed {
            let scope_root = t.get_scope_root().unwrap();
            if scope_root.is_function(t)
                && let Some(coloring) = precomputed.swap_remove(&scope_root)
            {
                self.should_optimize_scope_stack.push_front(true);
                self.colorings.push(coloring);
            } else {
                self.should_optimize_scope_stack.push_front(false);
            }
            return;
        }
        let mut scope_creator = self.scope_creator.clone();
        let Some(all_vars_declared_in_function) =
            Self::should_optimize_scope(t, &mut scope_creator)
        else {
            self.should_optimize_scope_stack.push_front(false);
            return;
        };
        self.should_optimize_scope_stack.push_front(true);
        let variables = all_vars_declared_in_function.get_all_variables_in_order();
        let compiler = t.get_compiler();
        // Parameters always interfere with one another. Preserve the escape analysis's
        // export callbacks for coding conventions that can observe those calls.
        if (variables.len() < 2 || variables.iter().all(|v| v.is_param(compiler)))
            && compiler.get_coding_convention().export_tests_are_pure()
        {
            self.colorings.push(None);
            return;
        }

        let scope = t.get_scope();
        check_state!(
            scope.is_function_scope(t.get_compiler()),
            "%s",
            scope.to_string(t.get_compiler())
        );

        // live variables analysis is based off of the control flow graph
        let cfg = self.get_control_flow_graph(t.get_compiler());

        let compiler = t.get_compiler();
        let mut liveness = LiveVariablesAnalysis::new(
            compiler,
            cfg,
            scope,
            None,
            &mut scope_creator,
            all_vars_declared_in_function,
        );

        if FeatureSet::ES3.contains(compiler.get_options().get_output_feature_set()) {
            // If the function has exactly 2 params, mark them as escaped. This is a work-around
            // for a bug in IE 8 and below, where it throws an exception if you write to the
            // parameters of the callback in a sort(). See
            // http://blickly.github.io/closure-compiler-issues/#58 and
            // https://www.zachleat.com/web/array-sort/
            let enclosing_function = scope.get_root_node(compiler);
            if NodeUtil::get_function_parameters(compiler, enclosing_function)
                .has_two_children(compiler)
            {
                liveness.mark_all_parameters_escaped(compiler);
            }
        }

        liveness.analyze_indexed(compiler);
        let coloring = Self::compute_variable_names_interference_graph(compiler, &liveness);
        if !self.use_pseudo_names && coloring.representatives.len() == coloring.colors.len() {
            self.colorings.push(None);
            return;
        }
        let mut live_uninit_loop_lets = IndexSet::default();
        let cfg = liveness.get_cfg();
        for &v in liveness.get_all_variables_in_order() {
            if let Some(name_node) = v.get_name_node(compiler)
                && Self::is_uninitialized_let_name_in_loop_body(compiler, name_node)
            {
                let let_node = name_node.get_parent(compiler).unwrap();
                let cfg_node = check_not_null!(cfg.get_node(&Some(let_node)));
                let var_index = liveness.get_var_index(&v.get_name(compiler));
                if var_index >= 0 && liveness.flow_state(cfg_node).get_out().is_live(var_index) {
                    live_uninit_loop_lets.insert(v);
                }
            }
        }
        let ordered_vars = if self.use_pseudo_names {
            liveness.get_all_variables_in_order().to_vec()
        } else {
            Vec::new()
        };
        let all_vars = liveness.into_all_vars();
        self.colorings.push(Some(FunctionColoring {
            all_vars,
            ordered_vars,
            live_uninit_loop_lets,
            coloring,
        }));
    }

    // port: CoalesceVariableNames#exitScopeWithCfg
    fn exit_scope_with_cfg(&mut self, _t: &mut NodeTraversal<'_>) {
        if !self.should_optimize_scope_stack.pop_front().unwrap() {
            return;
        }
        self.colorings.pop().unwrap();
    }

    /// Updates declarations of the given name node and the coalesced variable. Firstly, it
    /// converts the coalesced variable's declaration into a `var` if it is a `const` or a `let`.
    /// Secondly, it removes the declaration of the given name node if it is under a parent which
    /// is a name declaration or if the given name is a destructuring LHS name under a
    /// destructuring LHS declaration.
    ///
    /// For example, when coalescing the names `a` and `b` below, it changes:
    ///
    /// ```text
    ///   const a = 1;
    ///   const [{prop: b} = {prop: undefined}] = [{prop: 10}]; // declares `b`
    /// ```
    ///
    /// to
    ///
    /// ```text
    ///   var a = 1;
    ///   [{prop: a} = {prop: undefined}] = [{prop: 10}]; // coalescing renames `b` to `a` and this
    ///                                                   // method undeclares it by removing the
    ///                                                   // `const`
    /// ```
    // port: CoalesceVariableNames#updateDeclarationsPostCoalescing
    fn update_declarations_post_coalescing(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        coalesced_var: VarId,
        original_var: VarId,
        parent: NodeId,
    ) {
        check_state!(
            n.is_name(compiler),
            "trying to update the declaration of a non-name node"
        );
        if NodeUtil::is_name_declaration(compiler, Some(parent))
            || (NodeUtil::get_enclosing_type(compiler, n, Token::DESTRUCTURING_LHS).is_some()
                && NodeUtil::is_lhs_by_destructuring(compiler, n)
                // Consider this to be a destructuring LHS declaration only if we don't hit an
                // assignment like `[a,b] = [3,4]` first.
                && !self.is_name_inside_destructuring_assignment(compiler, n))
        {
            // convert the coalesced variable's declaration into a `var` if it is a `const` or a
            // `let`
            self.make_declaration_var(compiler, coalesced_var);
            // remove the declaration of the given name node as it has been coalesced with
            // coalescedVar
            self.remove_var_declaration(compiler, n, original_var);
        }
    }

    /// Is the given name node inside the LHS of a destructuring pattern assignment like
    /// `[a,b,c] = [1,2,3]`.
    ///
    /// Importantly, for this pass, it means that the name is not inside a declaration like
    /// `const [a,b,c] = [1,2,3]` and must not be undeclared.
    // port: CoalesceVariableNames#isNameInsideDestructuringAssignment
    fn is_name_inside_destructuring_assignment(
        &self,
        compiler: &AbstractCompiler,
        n: NodeId,
    ) -> bool {
        let enclosing_target = NodeUtil::get_root_target(compiler, n);
        if enclosing_target.is_destructuring_pattern(compiler)
            && enclosing_target
                .get_parent(compiler)
                .unwrap()
                .is_assign(compiler)
        {
            // This is a destructuring pattern assignment.
            return true;
        }
        false
    }

    /// In order to determine when it is appropriate to coalesce two variables, we use a live
    /// variables analysis to make sure they are not alive at the same time. We take every CFG
    /// node and determine which pairs of variables are alive at the same time. These pairs are
    /// set to true in a bit map. We take every pairing of variables and use the bit map to check
    /// if the two variables are alive at the same time. If two variables are alive at the same
    /// time, we create an edge between them in the interference graph. The interference graph is
    /// the input to a graph coloring algorithm that ensures any interfering variables are marked
    /// in different color groups, while variables that can safely be coalesced are assigned the
    /// same color group.
    ///
    /// The cfg and the escaped set (we don't want to coalesce any escaped variables) are those of
    /// `liveness`, which owns them in Rust.
    // port: CoalesceVariableNames#computeVariableNamesInterferenceGraph
    fn compute_variable_names_interference_graph(
        compiler: &mut AbstractCompiler,
        liveness: &LiveVariablesAnalysis,
    ) -> Coloring {
        let escaped = liveness.get_escaped_locals();
        let variables = liveness.get_all_variables_in_order();
        let mut eligible = 0u128;
        let mut parameters = 0u128;
        let mut variables_by_name = IndexMap::<JsString, u128>::default();
        let mut var_indices = Vec::with_capacity(variables.len());
        for (index, &v) in variables.iter().enumerate() {
            let bit = 1u128 << index;
            let name = v.get_name(compiler);
            var_indices.push(liveness.get_var_index(&name));
            // LiveRangeChecker matches by spelling, including declarations in different scopes.
            *variables_by_name.entry(name).or_default() |= bit;
            // Java Set<Var>.contains uses the inherited ScopedName equality.
            if escaped.iter().any(|e| e.equals(compiler, v)) {
                continue;
            }

            // NOTE(user): In theory, we CAN coalesce function names just like any variables. Our
            // Liveness analysis captures this just like it as described in the specification.
            // However, we saw some zipped and unzipped size increase after this. We are not
            // totally sure why that is but, for now, we will respect the dead functions and not
            // play around with it
            if v.get_parent_node(compiler).unwrap().is_function(compiler) {
                continue;
            }

            // NOTE: we skip class declarations for a combination of two reasons:
            // 1. they are block-scoped, so we would need to rewrite them as class expressions
            //      e.g. `class C {}` -> `var C = class {}` to avoid incorrect semantics
            //      (see testDontCoalesceClassDeclarationsWithDestructuringDeclaration).
            //    This is possible but increases pre-gzip code size and complexity.
            // 2. since function declaration coalescing seems to cause a size regression (as
            //    discussed above) we assume that coalescing class names may cause a similar size
            //    regression.
            if v.get_parent_node(compiler).unwrap().is_class(compiler) {
                continue;
            }

            // Skip lets and consts that have multiple variables declared in them, otherwise this
            // produces incorrect semantics. See test case "testCapture".
            // Skipping vars technically isn't needed for correct semantics, but works around a
            // Safari bug for var redeclarations
            // (https://github.com/google/closure-compiler/issues/3164)
            if Self::is_in_multiple_lvalue_decl(compiler, v) {
                continue;
            }

            eligible |= bit;
            if v.is_param(compiler) {
                parameters |= bit;
            }
        }
        Self::compute_coloring_readonly(
            &compiler.ast,
            liveness,
            &variables_by_name,
            eligible,
            parameters,
            &var_indices,
        )
    }

    fn compute_coloring_readonly(
        ast: &Ast,
        liveness: &LiveVariablesAnalysis,
        variables_by_name: &IndexMap<JsString, u128>,
        eligible: u128,
        parameters: u128,
        var_indices: &[i32],
    ) -> Coloring {
        let cfg = liveness.get_cfg();
        let variables = liveness.get_all_variables_in_order();
        let mut neighbors = vec![0u128; variables.len()];
        // Go through every CFG node in the program and look at variables that are live.
        // Set the pair of live variables in interferenceBitSet so we can add an edge between them.
        for cfg_node in cfg.get_nodes() {
            if cfg.is_implicit_return(cfg_node) {
                continue;
            }

            let state = liveness.flow_state(cfg_node);

            // Check the live states and add edge when possible. An edge between two variables
            // means that they are alive at overlapping times, which means that their
            // variable names cannot be coalesced.
            let live_in = live_bits(state.get_in());
            let live_out = live_bits(state.get_out());
            mark_conflicts(&mut neighbors, live_in, live_in);
            mark_conflicts(&mut neighbors, live_out, live_out);

            let root = cfg_node.get_value(cfg).unwrap();
            let mut live_range_checker = LiveRangeChecker {
                root,
                variables_by_name,
                writes: 0,
                reads: 0,
            };
            live_range_checker.check(ast, root);
            live_range_checker.set_crossing_variables(&mut neighbors, live_out);
        }

        for (index, row) in neighbors.iter_mut().enumerate() {
            *row &= eligible;
            if parameters & (1u128 << index) != 0 {
                *row |= parameters;
            }
        }
        let mut index_by_var =
            IndexMap::<VarId, i32>::with_capacity_and_hasher(variables.len(), Default::default());
        for (&v, &idx) in variables.iter().zip(var_indices.iter()) {
            index_by_var.insert(v, idx);
        }
        Coloring::from_interference(variables, &neighbors, eligible, |v| index_by_var[&v])
    }

    /// Returns whether this variable's declaration also declares other names.
    ///
    /// For example, this would return true for `x` in `let [x, y, z] = []`;
    // port: CoalesceVariableNames#isInMultipleLvalueDecl
    fn is_in_multiple_lvalue_decl(compiler: &mut AbstractCompiler, v: VarId) -> bool {
        let declaration_type = v.declaration_type(compiler);
        match declaration_type {
            Some(Token::LET | Token::CONST | Token::VAR) => {
                let name_decl = NodeUtil::get_enclosing_node(
                    compiler,
                    v.get_node(compiler).unwrap(),
                    &|ast, n| NodeUtil::is_name_declaration(ast, Some(n)),
                )
                .unwrap();

                let mut count = 0; // for lambda access
                NodeUtil::visit_lhs_nodes_in_node(compiler, name_decl, &mut |_, _lhs| count += 1);
                count > 1
            }
            _ => false,
        }
    }

    /// Remove variable declaration if the variable has been coalesced with another variable that
    /// has already been declared.
    ///
    /// A precondition is that if the variable has already been declared, it must be the only
    /// lvalue in said declaration. For example, this method will not accept `var x = 1, y = 2`.
    /// In theory we could leave in the `var` declaration, but var shadowing of params triggers a
    /// Safari bug: https://bugs.webkit.org/show_bug.cgi?id=182414 Another
    ///
    /// `name`: name node of the variable being coalesced
    // port: CoalesceVariableNames#removeVarDeclaration
    fn remove_var_declaration(
        &self,
        compiler: &mut AbstractCompiler,
        name: NodeId,
        original_var: VarId,
    ) {
        let var = NodeUtil::get_enclosing_node(compiler, name, &|ast, n| {
            NodeUtil::is_name_declaration(ast, Some(n))
        })
        .unwrap();
        let parent = var.get_parent(compiler).unwrap();

        if var
            .get_first_child(compiler)
            .unwrap()
            .is_destructuring_lhs(compiler)
        {
            // convert `const [x] = arr` to `[x] = arr`
            // a precondition for this method is that `x` is the only lvalue in the destructuring
            // pattern
            let destructuring_lhs = var.get_first_child(compiler).unwrap();
            let pattern = destructuring_lhs.remove_first_child(compiler).unwrap();
            if NodeUtil::is_enhanced_for(compiler, parent) {
                var.replace_with(compiler, pattern);
            } else {
                let rvalue = var
                    .get_first_first_child(compiler)
                    .unwrap()
                    .detach(compiler);
                let assign = IR::assign(compiler, pattern, rvalue).srcref(compiler, var);
                let expr = NodeUtil::new_expr(compiler, assign);
                var.replace_with(compiler, expr);
            }
        } else if NodeUtil::is_enhanced_for(compiler, parent) {
            // convert `for (let x of ...` to `for (x of ...`
            let detached = name.detach(compiler);
            var.replace_with(compiler, detached);
        } else {
            // either `var x = 0;` or `var x;`
            check_state!(
                var.has_one_child(compiler) && var.get_first_child(compiler) == Some(name),
                "%s",
                var.to_string(compiler)
            );
            if name.has_children(compiler) {
                // convert `let x = 0;` to `x = 0;`
                let value = name.remove_first_child(compiler).unwrap();
                name.detach(compiler);
                let mut assign = IR::assign(compiler, name, value).srcref(compiler, name);

                // We don't need to wrapped it with EXPR node if it is within a FOR.
                if !parent.is_vanilla_for(compiler) {
                    assign = NodeUtil::new_expr(compiler, assign);
                }
                var.replace_with(compiler, assign);
            } else if self.is_uninitialized_let_name_in_loop_body_and_live(
                compiler,
                name,
                original_var,
            ) {
                name.detach(compiler);
                let undefined_value = self
                    .ast_factory
                    .create_undefined_value(compiler)
                    .srcref_tree(compiler, name);
                let mut assign = IR::assign(compiler, name, undefined_value).srcref(compiler, name);
                if !parent.is_vanilla_for(compiler) {
                    assign = NodeUtil::new_expr(compiler, assign);
                }
                var.replace_with(compiler, assign);
            } else {
                // convert `let x;` to ``
                // and `for (let x;;) {}` to `for (;;) {}`
                // We can expect uninitialized declarations at this point and it's okay to remove
                // them and they've been coalesced with another declaration.
                NodeUtil::remove_child(compiler, parent, var);
            }
        }
    }

    /// Convert `const` or `let` declarations to `var` declarations.
    ///
    /// This method should be called on the first declared variable of a group that are being
    /// coalesced.
    ///
    /// Because the code has already been normalized by the time this pass runs, we can safely
    /// redeclare any let and const coalesced variables as vars
    // port: CoalesceVariableNames#makeDeclarationVar
    fn make_declaration_var(&self, compiler: &mut AbstractCompiler, coalesced_name: VarId) {
        if coalesced_name.is_const(compiler) || coalesced_name.is_let(compiler) {
            let name_node = check_not_null!(
                coalesced_name.get_name_node(compiler),
                "%s",
                coalesced_name.to_string(compiler)
            );
            if Self::is_uninitialized_let_name_in_loop_body(compiler, name_node) {
                // We need to make sure that within a loop:
                //
                // `let x;`
                // becomes
                // `var x = void 0;`
                //
                // If we don't we won't be correctly resetting the variable to undefined on each
                // loop iteration once we turn it into a var declaration.
                //
                // Note that all other cases will already have an initializer.
                // const x = 1; // constant requires an initializer
                // let {x, y} = obj; // destructuring requires an initializer
                // let [x, y] = iterable; // destructuring requires an initializer
                let undefined_value = self
                    .ast_factory
                    .create_undefined_value(compiler)
                    .srcref_tree(compiler, name_node);
                name_node.add_child_to_front(compiler, undefined_value);
            }
            // find the declaration node in a way that works normal and destructuring
            // declarations.
            let decl_node = NodeUtil::get_enclosing_node(
                compiler,
                name_node.get_parent(compiler).unwrap(),
                &|ast, n| NodeUtil::is_name_declaration(ast, Some(n)),
            )
            .unwrap();
            // normalization ensures that all variables in a function are uniquely named, so it's
            // OK to turn a `const` or `let` into a `var`.
            decl_node.set_token(compiler, Token::VAR);
        }
    }

    // port: CoalesceVariableNames#isUninitializedLetNameInLoopBody
    fn is_uninitialized_let_name_in_loop_body(
        compiler: &AbstractCompiler,
        name_node: NodeId,
    ) -> bool {
        check_state!(
            name_node.is_name(compiler),
            "%s",
            name_node.to_string(compiler)
        );
        let let_node = name_node.get_parent(compiler).unwrap();
        if !let_node.is_let(compiler) {
            // We're looking for `let name;`
            // Note that in the case of destructuring an initializer always exists.
            // `let {name} = initializerRequiredHere;
            return false;
        }
        if name_node.has_one_child(compiler) {
            // `let name = child;` has an initializer
            return false;
        }

        let let_parent = let_node.get_parent(compiler).unwrap();
        if NodeUtil::is_loop_structure(compiler, let_parent) {
            // `for (let x; ...`
            // `for (let x in ...`
            // `for (let x of ...`
            // `for await (let x of ...`
            // In all these cases the variable gets initialized on each loop iteration
            return false;
        }
        // Inside a loop body, but not the loop control node itself
        NodeUtil::is_within_loop(compiler, let_parent)
    }

    // port: CoalesceVariableNames#isUninitializedLetNameInLoopBodyAndLive
    fn is_uninitialized_let_name_in_loop_body_and_live(
        &self,
        compiler: &AbstractCompiler,
        name_node: NodeId,
        original_var: VarId,
    ) -> bool {
        if !Self::is_uninitialized_let_name_in_loop_body(compiler, name_node) {
            return false;
        }
        self.colorings
            .last()
            .unwrap()
            .as_ref()
            .unwrap()
            .live_uninit_loop_lets
            .contains(&original_var)
    }

    fn collect_functions(ast: &Ast, n: NodeId, out: &mut Vec<NodeId>) {
        if n.is_function(ast) {
            out.push(n);
        }
        let mut child = n.get_first_child(ast);
        while let Some(c) = child {
            let next = c.get_next(ast);
            Self::collect_functions(ast, c, out);
            child = next;
        }
    }
}

impl CompilerPass for CoalesceVariableNames {
    // port: CoalesceVariableNames#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        // checkNotNull(externs) and checkNotNull(root): a NodeId is never null.
        let mut scope_creator = self.scope_creator.clone();
        // Rust-only (performance divergence): precompute CFG, liveness, interference graphs, and
        // greedy colorings in parallel across functions (dropping CFGs and liveness immediately on
        // workers), then apply rewrites sequentially in exact NodeTraversal order.
        if !self.use_pseudo_names && compiler.get_coding_convention().export_tests_are_pure() {
            let mut functions = Vec::new();
            Self::collect_functions(&compiler.ast, root, &mut functions);
            if crate::parallel_parse::parallel_thread_count(functions.len(), 64, 8) > 1 {
                let is_es3 =
                    FeatureSet::ES3.contains(compiler.get_options().get_output_feature_set());
                let can_continue_after_errors = compiler.get_options().can_continue_after_errors();
                let mut scope_memo = IndexMap::default();
                let mut precomputed: IndexMap<NodeId, Option<FunctionColoring>> =
                    IndexMap::default();
                let mut any_merges = false;
                const CHUNK_SIZE: usize = 4096;
                for chunk in functions.chunks(CHUNK_SIZE) {
                    let mut jobs = Vec::with_capacity(chunk.len());
                    for &fn_node in chunk {
                        let scope = ensure_ancestor_scope(
                            compiler,
                            &mut scope_creator,
                            &mut scope_memo,
                            root,
                            fn_node,
                        );
                        let all_vars = NodeUtil::get_all_vars_declared_in_function(
                            compiler,
                            &mut scope_creator,
                            scope,
                        );
                        let vars = all_vars.get_all_variables_in_order();
                        if MAX_VARIABLES_TO_ANALYZE <= vars.len() as i32 {
                            continue;
                        }
                        if vars.len() < 2 || vars.iter().all(|v| v.is_param(compiler)) {
                            precomputed.insert(fn_node, None);
                            continue;
                        }
                        let prepared = PreparedLiveness::new(
                            compiler,
                            scope,
                            None,
                            &mut scope_creator,
                            all_vars,
                        );
                        let ordered = prepared.ordered_vars();
                        let mut base_eligible = 0u128;
                        let mut parameters = 0u128;
                        let mut variables_by_name = IndexMap::<JsString, u128>::default();
                        let mut var_indices = Vec::with_capacity(ordered.len());
                        let mut uninit_loop_lets = Vec::new();
                        let mut name_to_index = IndexMap::<JsString, i32>::default();
                        for (idx, &v) in ordered.iter().enumerate() {
                            name_to_index.insert(v.get_name(compiler), idx as i32);
                        }
                        for (index, &v) in ordered.iter().enumerate() {
                            let bit = 1u128 << index;
                            let name = v.get_name(compiler);
                            let var_index = name_to_index[&name];
                            var_indices.push(var_index);
                            *variables_by_name.entry(name).or_default() |= bit;
                            if let Some(name_node) = v.get_name_node(compiler)
                                && Self::is_uninitialized_let_name_in_loop_body(compiler, name_node)
                            {
                                let let_node = name_node.get_parent(compiler).unwrap();
                                uninit_loop_lets.push((index, let_node, var_index));
                            }
                            if v.get_parent_node(compiler).unwrap().is_function(compiler)
                                || v.get_parent_node(compiler).unwrap().is_class(compiler)
                                || Self::is_in_multiple_lvalue_decl(compiler, v)
                            {
                                continue;
                            }
                            base_eligible |= bit;
                            if v.is_param(compiler) {
                                parameters |= bit;
                            }
                        }
                        let es3_escape_2_params = is_es3
                            && NodeUtil::get_function_parameters(compiler, fn_node)
                                .has_two_children(compiler);
                        jobs.push(PreparedCoalesceJob {
                            fn_node,
                            prepared,
                            es3_escape_2_params,
                            variables_by_name,
                            base_eligible,
                            parameters,
                            var_indices,
                            uninit_loop_lets,
                        });
                    }
                    let threads = crate::parallel_parse::parallel_thread_count(jobs.len(), 64, 8);
                    if threads < 2 {
                        for job in jobs {
                            let fn_node = job.fn_node;
                            let out = job.run_borrowed(&compiler.ast, can_continue_after_errors);
                            let res = out.map(|o| job.materialize(o));
                            any_merges |= res.is_some();
                            precomputed.insert(fn_node, res);
                        }
                    } else {
                        let ast = &compiler.ast;
                        let mut outputs: Vec<Option<CompactColoringOutput>> =
                            (0..jobs.len()).map(|_| None).collect();
                        let worker_batches = crate::parallel_parse::run_scoped_indexed(
                            "closure-coalesce",
                            threads,
                            &jobs,
                            |_idx, job| job.run_borrowed(ast, can_continue_after_errors),
                        );
                        for batch in worker_batches {
                            for (idx, out) in batch {
                                outputs[idx] = Some(out);
                            }
                        }
                        for (job, out) in jobs.into_iter().zip(outputs) {
                            let fn_node = job.fn_node;
                            let res = out.map(|o| job.materialize(o));
                            any_merges |= res.is_some();
                            precomputed.insert(fn_node, res);
                        }
                    }
                }
                if !any_merges {
                    compiler.set_life_cycle_stage(LifeCycleStage::RAW);
                    return;
                }
                self.precomputed = Some(precomputed);
            }
        }
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback(self)
            .set_scope_creator(&mut scope_creator)
            .traverse(root);
        self.precomputed = None;
        compiler.set_life_cycle_stage(LifeCycleStage::RAW);
    }
}

impl Callback for CoalesceVariableNames {
    // port: NodeTraversal.AbstractCfgCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CoalesceVariableNames#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        let Some(Some(function)) = self.colorings.last_mut() else {
            return;
        };
        if !n.is_name(t) || parent.unwrap().is_function(t) {
            return;
        }
        let parent = parent.unwrap();
        let Some(&var) = function.all_vars.get(n.get_string_ref(t)) else {
            return;
        };
        let coloring = &mut function.coloring;
        let Some(&color) = coloring.colors.get(&var) else {
            return;
        };
        let coalesced_var = *coloring.representatives[color].get_or_insert(var);

        if !self.use_pseudo_names {
            if var.equals(t.get_compiler(), coalesced_var) {
                // The coalesced name is itself, nothing to do.
                return;
            }

            // Rename.
            let coalesced_name = coalesced_var.get_name(t.get_compiler());
            n.set_string(t, coalesced_name);
            t.get_compiler().report_change_to_enclosing_scope(n);
            self.update_declarations_post_coalescing(
                t.get_compiler(),
                n,
                coalesced_var,
                var,
                parent,
            );
        } else {
            // This code block is slow but since usePseudoName is for debugging,
            // we should not sacrifice performance for non-debugging compilation to
            // make this fast.
            let mut all_merged_names: BTreeSet<JsString> = BTreeSet::new();
            for &i_var in &function.ordered_vars {
                // Look for all the variables that can be merged (in the graph by now)
                // and it is merged with the current coalescedVar.
                if coloring.colors.get(&i_var) == Some(&color) {
                    all_merged_names.insert(i_var.get_name(t.get_compiler()));
                }
            }

            // Keep its original name.
            if all_merged_names.len() == 1 {
                return;
            }

            // Joiner.on("_").join(allMergedNames)
            let mut pseudo_name = JsString::from("");
            for (i, merged_name) in all_merged_names.iter().enumerate() {
                if i > 0 {
                    pseudo_name = pseudo_name.concat(&JsString::from("_"));
                }
                pseudo_name = pseudo_name.concat(merged_name);
            }

            loop {
                let scope = t.get_scope();
                if !scope.has_slot(t.get_compiler(), &pseudo_name) {
                    break;
                }
                pseudo_name = pseudo_name.concat(&JsString::from("$"));
            }

            // Rename.
            n.set_string(t, pseudo_name);
            t.get_compiler().report_change_to_enclosing_scope(n);

            if var.equals(t.get_compiler(), coalesced_var) {
                return;
            }
            self.update_declarations_post_coalescing(
                t.get_compiler(),
                n,
                coalesced_var,
                var,
                parent,
            );
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for CoalesceVariableNames {
    // port: NodeTraversal.AbstractCfgCallback#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        let current_scope_root = check_not_null!(t.get_scope_root());
        if NodeUtil::is_valid_cfg_root(t, current_scope_root) {
            self.cfgs.push_front(current_scope_root);
        }
        self.enter_scope_with_cfg(t);
    }

    // port: NodeTraversal.AbstractCfgCallback#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        self.exit_scope_with_cfg(t);
        let current_scope_root = check_not_null!(t.get_scope_root());
        if NodeUtil::is_valid_cfg_root(t, current_scope_root) {
            check_not_null!(self.cfgs.pop_front());
        }
    }
}

/// The lattice and interference graph share declaration indices.
fn live_bits(state: &LiveVariableLattice) -> u128 {
    let mut bits = 0;
    let mut i = state.next_set_bit(0);
    while i >= 0 {
        bits |= 1u128 << i;
        i = state.next_set_bit(i + 1);
    }
    bits
}

fn mark_conflicts(rows: &mut [u128], mut variables: u128, conflicts: u128) {
    while variables != 0 {
        rows[variables.trailing_zeros() as usize] |= conflicts;
        variables &= variables - 1;
    }
}

/// Track writes and the reads that follow the first write in one CFG node.
struct LiveRangeChecker<'a> {
    root: NodeId,
    variables_by_name: &'a IndexMap<JsString, u128>,
    writes: u128,
    reads: u128,
}

impl LiveRangeChecker<'_> {
    // port: CoalesceVariableNames.LiveRangeChecker#check
    fn check(&mut self, compiler: &Ast, n: NodeId) {
        // For most AST nodes, traverse the subtree in postorder because that's how the
        // expressions are evaluated.
        if n == self.root || !ControlFlowGraph::is_entering_new_cfg_node(compiler, n) {
            if (n.is_destructuring_lhs(compiler) && n.has_two_children(compiler))
                || (n.is_assign(compiler)
                    && n.get_first_child(compiler)
                        .unwrap()
                        .is_destructuring_pattern(compiler))
                || n.is_default_value(compiler)
            {
                // Evaluate the rhs of a destructuring assignment/declaration before the lhs.
                self.check(compiler, n.get_second_child(compiler).unwrap());
                self.check(compiler, n.get_first_child(compiler).unwrap());
            } else {
                let mut c = n.get_first_child(compiler);
                while let Some(cur) = c {
                    self.check(compiler, cur);
                    c = cur.get_next(compiler);
                }
            }
            if Self::should_visit(compiler, n) {
                self.visit(compiler, n, n.get_parent(compiler).unwrap());
            }
        }
    }

    // port: CoalesceVariableNames.LiveRangeChecker#visit
    fn visit(&mut self, compiler: &Ast, n: NodeId, parent: NodeId) {
        // port: CoalesceVariableNames.LiveRangeChecker#isAssignTo
        if n.is_name(compiler) {
            let bits = self
                .variables_by_name
                .get(n.get_string_ref(compiler))
                .copied()
                .unwrap_or(0);
            if parent.is_param_list(compiler)
                || (NodeUtil::is_name_declaration(compiler, Some(parent))
                    && n.has_children(compiler))
                || NodeUtil::is_lhs_by_destructuring(compiler, n)
            {
                self.writes |= bits;
            }
            // port: CoalesceVariableNames.LiveRangeChecker#isReadFrom
            if self.writes != 0 && !NodeUtil::is_name_decl_or_simple_assign_lhs(compiler, n, parent)
            {
                self.reads |= bits;
            }
        } else if NodeUtil::is_assignment_op(compiler, n) {
            let lhs = n.get_first_child(compiler).unwrap();
            if lhs.is_name(compiler) {
                self.writes |= self
                    .variables_by_name
                    .get(lhs.get_string_ref(compiler))
                    .copied()
                    .unwrap_or(0);
            }
        }
    }

    // port: CoalesceVariableNames.LiveRangeChecker#setCrossingVariables
    fn set_crossing_variables(&self, rows: &mut [u128], live_out: u128) {
        if self.writes != 0 {
            // Every value live on exit crosses every write, even the last write in the node.
            let reads = self.reads | live_out;
            mark_conflicts(rows, self.writes, reads);
            mark_conflicts(rows, reads, self.writes);
        }
    }

    /// Returns whether any LiveRangeChecker would be interested in the node.
    // port: CoalesceVariableNames.LiveRangeChecker#shouldVisit
    fn should_visit(compiler: &Ast, n: NodeId) -> bool {
        n.is_name(compiler)
            || (n.has_children(compiler) && n.get_first_child(compiler).unwrap().is_name(compiler))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{
        graph::Graph,
        graph_coloring::{GraphColoring, GreedyGraphColoring},
        linked_undirected_graph::LinkedUndirectedGraph,
    };
    use std::num::NonZeroU32;

    #[test]
    fn compact_coloring_matches_linked_graph_partitions() {
        // Include both mask words, isolated vertices, self-edges and equal tie breakers.
        for (count, density) in [(1, 0), (12, 0), (12, 100), (65, 15), (99, 35)] {
            let mut graph = LinkedUndirectedGraph::<VarId, ()>::new_with_value_to_strings(
                true,
                true,
                |v| format!("{v:?}"),
                |_| String::new(),
            );
            let variables: Vec<_> = (1..=count)
                .map(|i| VarId(NonZeroU32::new(i).unwrap()))
                .collect();
            let mut neighbors = vec![0u128; variables.len()];
            let mut eligible = 0;
            for (i, &var) in variables.iter().enumerate() {
                if i % 7 != 1 {
                    graph.create_node(var);
                    eligible |= 1u128 << i;
                }
            }
            let mut sample = 42u32;
            for (i, &left) in variables.iter().enumerate() {
                for (j, &right) in variables.iter().enumerate().skip(i) {
                    sample = sample.wrapping_mul(1664525).wrapping_add(1013904223);
                    if eligible & (1u128 << i) != 0
                        && eligible & (1u128 << j) != 0
                        && sample % 100 < density
                    {
                        graph.connect_if_not_found(left, (), right);
                        neighbors[i] |= 1u128 << j;
                        neighbors[j] |= 1u128 << i;
                    }
                }
            }
            let tie = |v: VarId| (v.0.get() % 5) as i32;
            let mut reference =
                GreedyGraphColoring::with_tie_breaker(Some(Box::new(move |a, b| {
                    tie(*a).cmp(&tie(*b))
                })));
            reference.color(&mut graph);
            let mut compact = Coloring::from_interference(&variables, &neighbors, eligible, tie);
            // Visit in reverse declaration order to exercise lazy representative selection.
            for (i, &var) in variables.iter().enumerate().rev() {
                if eligible & (1u128 << i) == 0 {
                    assert!(!compact.colors.contains_key(&var));
                    continue;
                }
                let color = compact.colors[&var];
                assert_eq!(
                    *compact.representatives[color].get_or_insert(var),
                    reference.get_partition_super_node(&graph, var)
                );
            }
        }
    }
}

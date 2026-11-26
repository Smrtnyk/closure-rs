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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/CheckUnreachableCode.java.

use crate::parallel_parse::ParallelAnalysisMode;
use crate::{
    control_flow_analysis::ControlFlowAnalysis,
    control_flow_graph::{Branch, ControlFlowGraph},
    diagnostic_type::DiagnosticType,
    graph::{
        adjacency_graph::AdjacencyGraph,
        annotatable::Annotatable,
        graph_node::GraphNode,
        graph_reachability::{EdgeTuple, GraphReachability, Reachable},
    },
    js_error::JSError,
    node_traversal::{AbstractCfgCallback, Callback, NodeTraversal, ScopedCallback},
    node_util::NodeUtil,
};
use closure_rhino::{
    check_state,
    fast_hash::IndexSet,
    jscomp_base::tri::Tri,
    node::{Ast, NodeId},
};

// port: CheckUnreachableCode#UNREACHABLE_CODE
pub static UNREACHABLE_CODE: DiagnosticType =
    DiagnosticType::warning("JSC_UNREACHABLE_CODE", "unreachable code");

/// Use [`ControlFlowGraph`] and [`GraphReachability`] to inform user about unreachable code.
pub struct CheckUnreachableCode<'a> {
    base: AbstractCfgCallback<'a>,
    /// Rust-only (performance divergence): whether `precheck` ran (at the first scope).
    prechecked: bool,
}

/// Rust-only (performance divergence): the least number of CFG roots per worker thread for which
/// `CheckUnreachableCode::precheck` runs.
const PRECHECK_MIN_ROOTS_PER_THREAD: usize = 64;

impl CheckUnreachableCode<'_> {
    // port: CheckUnreachableCode#CheckUnreachableCode
    pub fn new() -> Self {
        let mut base = AbstractCfgCallback::new();
        base.set_enter_scope_with_cfg(Self::enter_scope_with_cfg)
            .set_should_traverse(Self::should_traverse);
        Self {
            base,
            prechecked: false,
        }
    }

    /// Rust-only (performance divergence): the CFG roots below (and including) `root`, the first
    /// scope root of the traversal, for which this check reports nothing; `None` when there are
    /// too few of them to use worker threads.
    ///
    /// Java computes the graph of every CFG root the traversal enters, marks what is reachable
    /// from its entry (`initScope`) and reports, in `shouldTraverse`, each traversed node that has
    /// a graph node not marked reachable, a line number and is no EMPTY or BREAK. A root whose
    /// graph has no such node at all has no report and no `recompute`: the traversal neither
    /// needs its graph nor its marks, whatever nodes it then visits. Worker threads compute the
    /// graph and the marks of every root from `&Ast` (the pass changes no node), exactly as
    /// `get_control_flow_graph` and `initScope` do, and keep the roots without such a node. The
    /// traversal skips the graph of those (`AbstractCfgCallback::is_current_cfg_skipped`) and
    /// computes the others as before. A root whose check panics on a worker is not kept, so the
    /// traversal meets the panic where Java would throw. Straight-line functions are left out:
    /// `enter_scope_with_cfg` knows their graphs to be fully reachable without building them.
    /// The workers build the graph with `compute_cfg_readonly`, the function the traversal's
    /// `get_control_flow_graph` builds it with.
    fn precheck(compiler: &crate::AbstractCompiler, root: NodeId) -> Option<IndexSet<NodeId>> {
        if *crate::parallel_parse::PARALLEL_ANALYSIS == ParallelAnalysisMode::Off
            || crate::parallel_parse::parallel_thread_count(
                usize::MAX,
                PRECHECK_MIN_ROOTS_PER_THREAD,
                8,
            ) < 2
        {
            return None;
        }
        let ast: &Ast = compiler;
        let mut roots = Vec::new();
        // The scopes `NodeTraversal` enters below `root` that are CFG roots: functions, module
        // bodies and class static blocks (a SCRIPT is no scope root below the global one).
        if NodeUtil::is_valid_cfg_root(ast, root)
            && ControlFlowAnalysis::linear_function(ast, root).is_none()
        {
            roots.push(root);
        }
        let mut stack = Vec::new();
        let mut child = root.get_last_child(ast);
        while let Some(c) = child {
            stack.push(c);
            child = c.get_previous(ast);
        }
        while let Some(n) = stack.pop() {
            // A straight-line function needs no graph at all (`enter_scope_with_cfg`).
            if (n.is_function(ast) && ControlFlowAnalysis::linear_function(ast, n).is_none())
                || n.is_module_body(ast)
                || NodeUtil::is_class_static_block(ast, n)
            {
                roots.push(n);
            }
            let mut child = n.get_last_child(ast);
            while let Some(c) = child {
                stack.push(c);
                child = c.get_previous(ast);
            }
        }
        let threads = crate::parallel_parse::parallel_thread_count(
            roots.len(),
            PRECHECK_MIN_ROOTS_PER_THREAD,
            8,
        );
        if threads < 2 {
            return None;
        }
        let can_continue_after_errors = compiler.get_options().can_continue_after_errors();
        let batches = crate::parallel_parse::run_scoped_indexed(
            "closure-unreachable",
            threads,
            &roots,
            |_, &cfg_root| {
                crate::parallel_parse::catch_unwind_quietly(|| {
                    Self::reports_nothing(ast, can_continue_after_errors, cfg_root)
                })
                .filter(|&clean| clean)
                .map(|_| cfg_root)
            },
        );
        Some(batches.into_iter().flatten().map(|(_, r)| r).collect())
    }

    /// Rust-only (performance divergence): whether the graph of `cfg_root`, marked as `initScope`
    /// marks it, has no node that `shouldTraverse` would report.
    fn reports_nothing(ast: &Ast, can_continue_after_errors: bool, cfg_root: NodeId) -> bool {
        let mut cfg = crate::control_flow_analysis::ControlFlowAnalysis::compute_cfg_readonly(
            ast,
            can_continue_after_errors,
            cfg_root,
            false,
            true,
        );
        Self::init_scope(ast, &mut cfg);
        Self::graph_reports_nothing(ast, &cfg)
    }

    /// Rust-only (performance divergence): see `reports_nothing`.
    fn graph_reports_nothing(ast: &Ast, cfg: &ControlFlowGraph<NodeId>) -> bool {
        // `init_scope` notes a graph whose valued nodes are all reachable.
        cfg.all_valued_nodes_reachable
            || cfg.get_nodes().into_iter().all(|g_node| {
                let Some(n) = *g_node.get_value(cfg) else {
                    return true;
                };
                g_node
                    .get_annotation(cfg)
                    .is_some_and(|a| a.as_any().is::<Reachable>())
                    || n.get_lineno(ast) == -1
                    || n.is_empty(ast)
                    || n.is_break(ast)
            })
    }

    // port: CheckUnreachableCode#enterScopeWithCfg
    fn enter_scope_with_cfg(this: &mut AbstractCfgCallback<'_>, t: &mut NodeTraversal<'_>) {
        let scope_root = t.get_scope_root().unwrap();
        if NodeUtil::is_valid_cfg_root(t, scope_root) {
            // Rust-only (performance divergence): the graph of a function whose body runs its
            // statements once, in order (`ControlFlowAnalysis::linear_function`), joins its nodes
            // by unconditional edges from the entry, so every node is reachable and none is
            // reported: the graph is neither built nor walked.
            if ControlFlowAnalysis::linear_function(t, scope_root).is_some() {
                #[cfg(debug_assertions)]
                {
                    let compiler = t.get_compiler();
                    let mut cfg = ControlFlowAnalysis::builder()
                        .set_cfg_root(scope_root)
                        .set_include_edge_annotations(true)
                        .compute_cfg(compiler);
                    Self::init_scope(compiler, &mut cfg);
                    debug_assert!(cfg.all_valued_nodes_reachable);
                }
                this.mark_front_fully_reachable();
                return;
            }
            let skipped = this.is_current_cfg_skipped();
            if skipped && *crate::parallel_parse::PARALLEL_ANALYSIS != ParallelAnalysisMode::Verify
            {
                // Rust-only (performance divergence): see `precheck`.
                return;
            }
            let compiler = t.get_compiler();
            let cfg = this.get_control_flow_graph_mut(compiler);
            Self::init_scope(compiler, cfg);
            if skipped {
                check_state!(
                    Self::graph_reports_nothing(compiler, cfg),
                    "CLOSURE_RS_PARALLEL_ANALYSIS=verify: unreachable code precheck skipped %s",
                    scope_root.to_string(compiler)
                );
            }
        }
    }

    // port: CheckUnreachableCode#shouldTraverse
    fn should_traverse(
        this: &mut AbstractCfgCallback<'_>,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if this.front_is_fully_reachable() || this.is_current_cfg_skipped() {
            // Rust-only (performance divergence): see `enter_scope_with_cfg` and `precheck`.
            // (With `verify`, the graph of a skipped root was computed at the scope's entry, so
            // the root is no longer skipped.)
            return true;
        }
        let compiler = t.get_compiler();
        let cfg = this.get_control_flow_graph_mut(compiler);
        // Rust-only (performance divergence): with every node of the graph reachable, no node is
        // reported; the lookup of `n` among the graph's nodes is skipped.
        if cfg.all_valued_nodes_reachable {
            return true;
        }
        let g_node = cfg.get_node(&Some(n));
        if let Some(g_node) = g_node
            && !g_node
                .get_annotation(cfg)
                .is_some_and(|a| a.as_any().is::<Reachable>())
        {
            // Only report error when there are some line number informations.
            // There are synthetic nodes with no line number informations, nodes
            // introduce by other passes (although not likely since this pass should
            // be executed early) or some rhino bug.
            if n.get_lineno(compiler) != -1
                // Allow spurious semi-colons and spurious breaks.
                && !n.is_empty(compiler)
                && !n.is_break(compiler)
            {
                let error = JSError::make(compiler, n, &UNREACHABLE_CODE, &[]);
                compiler.report(error);
                // From now on, we are going to assume the user fixed the error and not
                // give more warning related to code section reachable from this node.
                GraphReachability::new().recompute(cfg, Some(n));

                // Saves time by not traversing children.
                return false;
            }
        }
        true
    }

    // port: CheckUnreachableCode#initScope
    fn init_scope(ast: &Ast, control_flow_graph: &mut ControlFlowGraph<NodeId>) {
        let entry = *control_flow_graph.get_entry().get_value(control_flow_graph);
        GraphReachability::with_edge_predicate(Some(Box::new(move |input| {
            Self::is_reachable(ast, input)
        })))
        .compute(control_flow_graph, entry);
        control_flow_graph.all_valued_nodes_reachable =
            control_flow_graph.get_nodes().into_iter().all(|node| {
                node.get_value(control_flow_graph).is_none()
                    || node
                        .get_annotation(control_flow_graph)
                        .is_some_and(|a| a.as_any().is::<Reachable>())
            });
    }

    // port: CheckUnreachableCode#isReachable
    fn is_reachable(ast: &Ast, input: &EdgeTuple<Option<NodeId>, Branch>) -> bool {
        let branch = input.edge;
        if !branch.is_conditional() {
            return true;
        }
        let predecessor = input.source_node.unwrap();
        let condition = NodeUtil::get_condition_expression(ast, predecessor);

        // TODO(user): Handle more complicated expression like true == true,
        // etc....
        if let Some(condition) = condition {
            let val = NodeUtil::get_boolean_value(ast, condition);
            if val != Tri::UNKNOWN {
                return val.to_boolean(true) == (branch == Branch::ON_TRUE);
            }
        }
        true
    }
}

impl Default for CheckUnreachableCode<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl Callback for CheckUnreachableCode<'_> {
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        Callback::should_traverse(&mut self.base, t, n, parent)
    }

    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        Callback::visit(&mut self.base, t, n, parent)
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for CheckUnreachableCode<'_> {
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if !self.prechecked {
            // Rust-only (performance divergence): see `precheck`.
            self.prechecked = true;
            let root = t.get_scope_root().unwrap();
            if let Some(roots) = Self::precheck(t.get_compiler(), root) {
                self.base.set_skip_roots(roots);
            }
        }
        self.base.enter_scope(t);
    }

    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        self.base.exit_scope(t);
    }
}

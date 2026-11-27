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
    token::Token,
};
use std::sync::LazyLock;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ShortcutMode {
    Off,
    On,
    Verify,
}

/// `CLOSURE_RS_REACHABLE_SHORTCUT=off` computes every graph; `=verify` computes the graphs the
/// shortcut skips too and checks that nothing in them is reported.
static SHORTCUT_MODE: LazyLock<ShortcutMode> =
    LazyLock::new(
        || match std::env::var("CLOSURE_RS_REACHABLE_SHORTCUT").as_deref() {
            Ok("off") => ShortcutMode::Off,
            Ok("verify") => ShortcutMode::Verify,
            _ => ShortcutMode::On,
        },
    );

/// Rust-only (performance divergence): whether the CFG root `root` needs no graph: every node of
/// its control flow graph is reachable along the edges `CheckUnreachableCode::is_reachable`
/// keeps, read from the statements alone; false when unsure. Java computes the graph and its
/// reachability for every CFG root; when this holds no node is reported. It holds for every
/// function `ControlFlowAnalysis::linear_function` accepts (statements run once, in order).
///
/// `ControlFlowAnalysis` makes graph nodes of the statements of statement lists, of the branches
/// and bodies of control structures and of a loop's head parts; nested functions are none (a
/// statement list and the search for the next statement pass over them). A node is reachable
/// when the statements before it in its list can complete normally. Lists are read in order:
/// `return`, `throw`, unlabeled `break` and `continue` do not complete normally, an `if`
/// completes when one branch does, a loop always does (its condition can be false). The shortcut
/// gives up on a statement after one that cannot complete, on a condition with a known boolean
/// value (a branch is not followed), on a `for` without a condition, on a `for` whose increment
/// follows only a body that cannot complete, and on `do`, `switch`, `try`, labels, `with`,
/// labeled jumps and synthetic blocks, whose edges depend on more than this.
pub(crate) fn needs_no_graph(ast: &Ast, root: NodeId) -> bool {
    // Whether `n` can complete normally; None when unsure.
    fn statement(ast: &Ast, n: NodeId) -> Option<bool> {
        let token = n.get_token(ast);
        match token {
            Token::BLOCK => {
                if n.is_synthetic_block(ast) {
                    return None;
                }
                list(ast, n)
            }
            Token::ROOT | Token::SCRIPT | Token::MODULE_BODY => list(ast, n),
            Token::EXPR_RESULT
            | Token::VAR
            | Token::LET
            | Token::CONST
            | Token::EMPTY
            | Token::DEBUGGER
            | Token::CLASS
            | Token::IMPORT
            | Token::EXPORT
            | Token::RETURN
            | Token::THROW => {
                // `ControlFlowAnalysis` considers a TRY child before its parent, even in a
                // malformed tree.
                if n.children(ast).any(|child| child.is_try(ast)) {
                    return None;
                }
                Some(!matches!(token, Token::RETURN | Token::THROW))
            }
            Token::BREAK | Token::CONTINUE => (!n.has_children(ast)).then_some(false),
            Token::IF => {
                let condition = n.get_first_child(ast)?;
                if NodeUtil::get_boolean_value(ast, condition) != Tri::UNKNOWN {
                    return None;
                }
                let then_branch = condition.get_next(ast)?;
                let then_completes = statement(ast, then_branch)?;
                let else_completes = match then_branch.get_next(ast) {
                    Some(else_branch) => statement(ast, else_branch)?,
                    None => true,
                };
                Some(then_completes || else_completes)
            }
            Token::WHILE => {
                let condition = n.get_first_child(ast)?;
                if NodeUtil::get_boolean_value(ast, condition) != Tri::UNKNOWN {
                    return None;
                }
                statement(ast, condition.get_next(ast)?)?;
                Some(true)
            }
            Token::FOR => {
                let init = n.get_first_child(ast)?;
                let condition = init.get_next(ast)?;
                let increment = condition.get_next(ast)?;
                let body = increment.get_next(ast)?;
                if condition.is_empty(ast)
                    || NodeUtil::get_boolean_value(ast, condition) != Tri::UNKNOWN
                {
                    return None;
                }
                if !statement(ast, body)? && !increment.is_empty(ast) {
                    return None;
                }
                Some(true)
            }
            Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                statement(ast, n.get_last_child(ast)?)?;
                Some(true)
            }
            _ => None,
        }
    }
    // Whether the statements of `n` can complete normally; None when unsure.
    fn list(ast: &Ast, n: NodeId) -> Option<bool> {
        let mut completes = true;
        for child in n.children(ast) {
            if child.is_function(ast) {
                continue;
            }
            if !completes {
                return None;
            }
            completes = statement(ast, child)?;
        }
        Some(completes)
    }
    match root.get_token(ast) {
        Token::FUNCTION => {
            let Some(body) = root.get_last_child(ast) else {
                return false;
            };
            !body.is_block(ast) || (!body.is_synthetic_block(ast) && list(ast, body).is_some())
        }
        Token::ROOT | Token::SCRIPT | Token::MODULE_BODY | Token::BLOCK => {
            list(ast, root).is_some()
        }
        _ => false,
    }
}

/// Rust-only (performance divergence): whether the shortcut settles the CFG root `root` without
/// a graph (`needs_no_graph`, unless `CLOSURE_RS_REACHABLE_SHORTCUT=off`). Neither the traversal
/// nor the worker precheck builds a graph for such a root.
pub(crate) fn settled_without_graph(ast: &Ast, root: NodeId) -> bool {
    *SHORTCUT_MODE != ShortcutMode::Off && needs_no_graph(ast, root)
}

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
    /// traversal meets the panic where Java would throw. The roots `settled_without_graph` settles
    /// are left out: `enter_scope_with_cfg` knows their graphs to report nothing without building
    /// them.
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
        if NodeUtil::is_valid_cfg_root(ast, root) && !settled_without_graph(ast, root) {
            roots.push(root);
        }
        let mut stack = Vec::new();
        let mut child = root.get_last_child(ast);
        while let Some(c) = child {
            stack.push(c);
            child = c.get_previous(ast);
        }
        while let Some(n) = stack.pop() {
            // A root `needs_no_graph` settles needs no graph at all (`enter_scope_with_cfg`).
            if (n.is_function(ast)
                || n.is_module_body(ast)
                || NodeUtil::is_class_static_block(ast, n))
                && !settled_without_graph(ast, n)
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
            // Rust-only (performance divergence): a root whose graph has every node reachable
            // (`needs_no_graph`) reports none: the graph is neither built nor walked.
            if settled_without_graph(t, scope_root) {
                if cfg!(debug_assertions) || *SHORTCUT_MODE == ShortcutMode::Verify {
                    let compiler = t.get_compiler();
                    let mut cfg = ControlFlowAnalysis::builder()
                        .set_cfg_root(scope_root)
                        .set_include_edge_annotations(true)
                        .compute_cfg(compiler);
                    Self::init_scope(compiler, &mut cfg);
                    // An unreachable node that `should_traverse` would not report (no line, an
                    // empty statement, a break; such as the empty increment of a `for` whose body
                    // cannot complete) is allowed.
                    assert!(
                        Self::graph_reports_nothing(compiler, &cfg),
                        "CheckUnreachableCode shortcut: a node would be reported in the root at {}",
                        scope_root.get_location(compiler)
                    );
                }
                this.mark_front_fully_reachable();
                return;
            }
            if *SHORTCUT_MODE == ShortcutMode::Verify {
                assert!(
                    ControlFlowAnalysis::linear_function(t, scope_root).is_none(),
                    "CheckUnreachableCode shortcut: a straight-line function needs a graph at {}",
                    scope_root.get_location(t)
                );
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

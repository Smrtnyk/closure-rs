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
//   src/com/google/javascript/jscomp/CheckUnreachableCode.java.

use crate::{
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
    jscomp_base::tri::Tri,
    node::{Ast, NodeId},
};

// port: CheckUnreachableCode#UNREACHABLE_CODE
pub static UNREACHABLE_CODE: DiagnosticType =
    DiagnosticType::warning("JSC_UNREACHABLE_CODE", "unreachable code");

/// Use [`ControlFlowGraph`] and [`GraphReachability`] to inform user about unreachable code.
pub struct CheckUnreachableCode<'a> {
    base: AbstractCfgCallback<'a>,
}

impl CheckUnreachableCode<'_> {
    // port: CheckUnreachableCode#CheckUnreachableCode
    pub fn new() -> Self {
        let mut base = AbstractCfgCallback::new();
        base.set_enter_scope_with_cfg(Self::enter_scope_with_cfg)
            .set_should_traverse(Self::should_traverse);
        Self { base }
    }

    // port: CheckUnreachableCode#enterScopeWithCfg
    fn enter_scope_with_cfg(this: &mut AbstractCfgCallback<'_>, t: &mut NodeTraversal<'_>) {
        let scope_root = t.get_scope_root().unwrap();
        if NodeUtil::is_valid_cfg_root(t, scope_root) {
            let compiler = t.get_compiler();
            let cfg = this.get_control_flow_graph_mut(compiler);
            Self::init_scope(compiler, cfg);
        }
    }

    // port: CheckUnreachableCode#shouldTraverse
    fn should_traverse(
        this: &mut AbstractCfgCallback<'_>,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        let compiler = t.get_compiler();
        let cfg = this.get_control_flow_graph_mut(compiler);
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
        Some(&mut self.base)
    }
}

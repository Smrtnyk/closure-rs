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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/DataFlowAnalysis.java.

use crate::graph::{
    adjacency_graph::AdjacencyGraph, annotatable::Annotatable, graph::GraphEdge,
    graph_node::GraphNode,
};
use crate::{
    AbstractCompiler,
    control_flow_graph::{Branch, ControlFlowGraph, NodeComparator},
    graph::{
        di_graph::{DiGraph, DiGraphEdge, DiGraphNode},
        graph::Graph,
    },
};
use closure_rhino::check_state;
use indexmap::IndexSet;
use std::{collections::VecDeque, fmt, hash::Hash};

pub const MAX_STEPS_PER_NODE: i32 = 20000;
/// Rust-only: Java's `Object#equals` on lattice elements, as DataFlowAnalysis#flow uses it
/// (`!before.equals(after)`). It receives the compiler because LinkedFlowScope#equals reads the
/// compiler-owned typed scopes (TypeInference's lattice); value lattices compare with `==`.
pub trait LatticeEquals {
    fn lattice_equals(&self, compiler: &mut AbstractCompiler, other: &Self) -> bool;
}
/// The join operation. Java joiners that need the compiler hold it (LinkedFlowScope's
/// FlowScopeJoinOp); here it is passed to each call (DESIGN §6).
pub trait FlowJoiner<L> {
    // port: DataFlowAnalysis.FlowJoiner#joinFlow
    fn join_flow(&mut self, compiler: &mut AbstractCompiler, input: L);
    // port: DataFlowAnalysis.FlowJoiner#finish
    fn finish(self: Box<Self>) -> L;
}
/// The branch operation. `A` is the analysis that created the brancher: a Java brancher is an
/// inner class of its analysis (TypeInference#createFlowBrancher), so it is handed the analysis.
pub trait FlowBrancher<L, A: ?Sized> {
    // port: DataFlowAnalysis.FlowBrancher#branchFlow
    fn branch_flow(
        &mut self,
        analysis: &mut A,
        compiler: &mut AbstractCompiler,
        branch: Branch,
    ) -> L;
}
impl<L, A: ?Sized, F: FnMut(Branch) -> L> FlowBrancher<L, A> for F {
    fn branch_flow(
        &mut self,
        _analysis: &mut A,
        _compiler: &mut AbstractCompiler,
        branch: Branch,
    ) -> L {
        self(branch)
    }
}
pub struct DataFlowAnalysisState<N: Clone + Eq + Hash> {
    pub cfg: ControlFlowGraph<N>,
    work_queue: UniqueQueue,
}
impl<N: Clone + Eq + Hash> DataFlowAnalysisState<N> {
    // port: DataFlowAnalysis#DataFlowAnalysis
    pub fn new(cfg: ControlFlowGraph<N>, is_forward: bool, is_branched: bool) -> Self {
        if is_branched {
            check_state!(is_forward);
        }
        let work_queue = UniqueQueue::new(cfg.get_optional_node_comparator(is_forward));
        Self { cfg, work_queue }
    }
    pub fn into_cfg(self) -> ControlFlowGraph<N> {
        self.cfg
    }
}
pub trait DataFlowAnalysis<
    N: Clone + Eq + Hash,
    L: Clone + LatticeEquals + crate::graph::annotation::Annotation,
>
{
    fn state(&self) -> &DataFlowAnalysisState<N>;
    fn state_mut(&mut self) -> &mut DataFlowAnalysisState<N>;
    // port: DataFlowAnalysis#isForward
    fn is_forward(&self) -> bool;
    // port: DataFlowAnalysis#createFlowJoiner
    fn create_flow_joiner(&self) -> Box<dyn FlowJoiner<L>>;
    // port: DataFlowAnalysis#flowThrough
    fn flow_through(&mut self, compiler: &mut AbstractCompiler, node: N, input: L) -> L;
    // port: DataFlowAnalysis#createInitialEstimateLattice
    fn create_initial_estimate_lattice(&self) -> L;
    // port: DataFlowAnalysis#createEntryLattice
    fn create_entry_lattice(&mut self, compiler: &mut AbstractCompiler) -> L;
    // port: DataFlowAnalysis#isBranched
    fn is_branched(&self) -> bool {
        false
    }
    // port: DataFlowAnalysis#createFlowBrancher
    fn create_flow_brancher(
        &mut self,
        _compiler: &mut AbstractCompiler,
        _node: N,
        _output: L,
    ) -> Box<dyn FlowBrancher<L, Self>> {
        panic!("UnsupportedOperationException")
    }
    // port: DataFlowAnalysis#getCfg
    fn get_cfg(&self) -> &ControlFlowGraph<N> {
        &self.state().cfg
    }
    fn get_cfg_mut(&mut self) -> &mut ControlFlowGraph<N> {
        &mut self.state_mut().cfg
    }
    // port: DataFlowAnalysis#join
    fn join(&self, compiler: &mut AbstractCompiler, lattice_a: L, lattice_b: L) -> L {
        let mut joiner = self.create_flow_joiner();
        joiner.join_flow(compiler, lattice_a);
        joiner.join_flow(compiler, lattice_b);
        joiner.finish()
    }
    // port: DataFlowAnalysis#analyze
    fn analyze(&mut self, compiler: &mut AbstractCompiler) {
        self.initialize();
        while !self.state().work_queue.is_empty() {
            let cur_node = self.state_mut().work_queue.remove_first();
            let cur_state = cur_node
                .get_annotation_as_mut::<LinearFlowState<L>>(self.get_cfg_mut())
                .unwrap();
            let steps = cur_state.step_count;
            cur_state.step_count += 1;
            if steps > MAX_STEPS_PER_NODE {
                panic!(
                    "Dataflow analysis appears to diverge around: {}",
                    self.node_to_string(
                        compiler,
                        cur_node.get_value(self.get_cfg()).clone().unwrap()
                    )
                );
            }
            self.join_inputs(compiler, cur_node);
            if self.flow(compiler, cur_node) {
                let next_nodes = if self.is_forward() {
                    self.get_cfg().get_directed_succ_nodes_of(cur_node)
                } else {
                    self.get_cfg().get_directed_pred_nodes_of(cur_node)
                };
                for next_node in next_nodes {
                    if next_node != self.get_cfg().get_implicit_return() {
                        self.state_mut().work_queue.add(next_node);
                    }
                }
            }
        }
        if self.is_forward() {
            self.join_inputs(compiler, self.get_cfg().get_implicit_return());
        }
    }
    fn node_to_string(&self, compiler: &AbstractCompiler, node: N) -> String {
        self.get_cfg().format_node_value(compiler, Some(&node))
    }
    // port: DataFlowAnalysis#initialize
    fn initialize(&mut self) {
        self.state_mut().work_queue.clear();
        for node in self.get_cfg().get_nodes() {
            let state = LinearFlowState::new(
                self.create_initial_estimate_lattice(),
                self.create_initial_estimate_lattice(),
            );
            node.set_annotation(self.get_cfg_mut(), Some(Box::new(state)));
            if node != self.get_cfg().get_implicit_return() {
                self.state_mut().work_queue.add(node);
            }
        }
        if self.is_branched() {
            for edge in self.get_cfg().get_edges() {
                let lattice = self.create_initial_estimate_lattice();
                edge.set_annotation(self.get_cfg_mut(), Some(Box::new(lattice)));
            }
        }
    }
    // port: DataFlowAnalysis#flow
    fn flow(&mut self, compiler: &mut AbstractCompiler, node: DiGraphNode) -> bool {
        let state = node
            .get_annotation_as::<LinearFlowState<L>>(self.get_cfg())
            .unwrap();
        let value = node.get_value(self.get_cfg()).clone().unwrap();
        if self.is_forward() {
            let before = state.get_out().clone();
            let input = state.get_in().clone();
            let out = self.flow_through(compiler, value.clone(), input);
            node.get_annotation_as_mut::<LinearFlowState<L>>(self.get_cfg_mut())
                .unwrap()
                .set_out(out.clone());
            let mut changed = !before.lattice_equals(compiler, &out);
            if self.is_branched() {
                let edges: Vec<_> = node
                    .get_out_edges(self.get_cfg())
                    .iter()
                    .map(|&e| (e, *e.get_value(self.get_cfg())))
                    .collect();
                let mut brancher = self.create_flow_brancher(compiler, value, out);
                for (edge, branch) in edges {
                    let before = edge.get_annotation_as::<L>(self.get_cfg()).unwrap().clone();
                    let result = brancher.branch_flow(self, compiler, branch);
                    edge.set_annotation(self.get_cfg_mut(), Some(Box::new(result)));
                    if !changed {
                        let after = edge.get_annotation_as::<L>(self.get_cfg()).unwrap().clone();
                        changed = !before.lattice_equals(compiler, &after);
                    }
                }
            }
            changed
        } else {
            let before = state.get_in().clone();
            let input = state.get_out().clone();
            let result = self.flow_through(compiler, value, input);
            let changed = !before.lattice_equals(compiler, &result);
            node.get_annotation_as_mut::<LinearFlowState<L>>(self.get_cfg_mut())
                .unwrap()
                .set_in(result);
            changed
        }
    }
    // port: DataFlowAnalysis#joinInputs
    fn join_inputs(&mut self, compiler: &mut AbstractCompiler, node: DiGraphNode) {
        if self.is_forward() && self.get_cfg().get_entry() == node {
            let entry = self.create_entry_lattice(compiler);
            node.get_annotation_as_mut::<LinearFlowState<L>>(self.get_cfg_mut())
                .unwrap()
                .set_in(entry);
            return;
        }
        let edges = if self.is_forward() {
            node.get_in_edges(self.get_cfg())
        } else {
            node.get_out_edges(self.get_cfg())
        };
        let result = match edges.len() {
            0 => return,
            1 => {
                let edge = edges[0];
                self.get_input_from_edge(compiler, edge)
            }
            _ => {
                let edges = edges.to_vec();
                let mut joiner = self.create_flow_joiner();
                for edge in edges {
                    let input = self.get_input_from_edge(compiler, edge);
                    joiner.join_flow(compiler, input);
                }
                joiner.finish()
            }
        };
        let forward = self.is_forward();
        let state = node
            .get_annotation_as_mut::<LinearFlowState<L>>(self.get_cfg_mut())
            .unwrap();
        if forward {
            state.set_in(result);
        } else {
            state.set_out(result);
        }
    }
    // port: DataFlowAnalysis#getInputFromEdge
    fn get_input_from_edge(&mut self, compiler: &mut AbstractCompiler, edge: DiGraphEdge) -> L {
        if self.is_branched() {
            edge.get_annotation_as::<L>(self.get_cfg()).unwrap().clone()
        } else if self.is_forward() {
            edge.get_source(self.get_cfg())
                .get_annotation_as::<LinearFlowState<L>>(self.get_cfg())
                .unwrap()
                .get_out()
                .clone()
        } else {
            let node = edge.get_destination(self.get_cfg());
            if node == self.get_cfg().get_implicit_return() {
                return self.create_entry_lattice(compiler);
            }
            node.get_annotation_as::<LinearFlowState<L>>(self.get_cfg())
                .unwrap()
                .get_in()
                .clone()
        }
    }
}
#[derive(Clone)]
pub struct LinearFlowState<L> {
    step_count: i32,
    input: L,
    output: L,
}
impl<L> LinearFlowState<L> {
    // port: DataFlowAnalysis.LinearFlowState#LinearFlowState
    pub fn new(input: L, output: L) -> Self {
        Self {
            step_count: 0,
            input,
            output,
        }
    }
    // port: DataFlowAnalysis.LinearFlowState#getStepCount
    pub fn get_step_count(&self) -> i32 {
        self.step_count
    }
    // port: DataFlowAnalysis.LinearFlowState#getIn
    pub fn get_in(&self) -> &L {
        &self.input
    }
    // port: DataFlowAnalysis.LinearFlowState#setIn
    fn set_in(&mut self, input: L) {
        self.input = input;
    }
    // port: DataFlowAnalysis.LinearFlowState#getOut
    pub fn get_out(&self) -> &L {
        &self.output
    }
    // port: DataFlowAnalysis.LinearFlowState#setOut
    fn set_out(&mut self, output: L) {
        self.output = output;
    }
}
impl<L: fmt::Display> fmt::Display for LinearFlowState<L> {
    // port: DataFlowAnalysis.LinearFlowState#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "IN: {} OUT: {}", self.input, self.output)
    }
}
pub struct UniqueQueue {
    seen_set: IndexSet<DiGraphNode>,
    queue: VecDeque<DiGraphNode>,
    priority: Option<NodeComparator>,
}
impl UniqueQueue {
    // port: DataFlowAnalysis.UniqueQueue#UniqueQueue
    pub fn new(priority: Option<NodeComparator>) -> Self {
        Self {
            seen_set: IndexSet::new(),
            queue: VecDeque::new(),
            priority,
        }
    }
    // port: DataFlowAnalysis.UniqueQueue#isEmpty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
    // port: DataFlowAnalysis.UniqueQueue#removeFirst
    pub fn remove_first(&mut self) -> DiGraphNode {
        let t = self.queue.pop_front().unwrap();
        self.seen_set.shift_remove(&t);
        t
    }
    // port: DataFlowAnalysis.UniqueQueue#add
    pub fn add(&mut self, t: DiGraphNode) {
        if self.seen_set.insert(t) {
            if let Some(compare) = &self.priority {
                // Priorities are unique per CFG node (CFA assigns ++priorityCounter).
                let position = self
                    .queue
                    .iter()
                    .position(|&x| compare(t, x).is_lt())
                    .unwrap_or(self.queue.len());
                self.queue.insert(position, t);
            } else {
                self.queue.push_back(t);
            }
        }
    }
    // port: DataFlowAnalysis.UniqueQueue#clear
    pub fn clear(&mut self) {
        self.seen_set.clear();
        self.queue.clear();
    }
}
// port: DataFlowAnalysis#computeEscaped
pub fn compute_escaped(
    compiler: &mut AbstractCompiler,
    js_scope: crate::scope::ScopeId,
    escaped: &mut IndexSet<crate::var::VarId>,
    scope_creator: &mut dyn crate::scope_creator::ScopeCreator,
    all_vars_in_fn: &indexmap::IndexMap<closure_rhino::js_string::JsString, crate::var::VarId>,
) {
    use crate::node_traversal::{Callback, NodeTraversal};
    use closure_rhino::{check_argument, node::NodeId};
    check_argument!(js_scope.is_function_scope(compiler));
    struct Finder<'a> {
        js_scope: crate::scope::ScopeId,
        escaped: &'a mut IndexSet<crate::var::VarId>,
    }
    impl Callback for Finder<'_> {
        fn should_traverse(
            &mut self,
            _t: &mut NodeTraversal<'_>,
            _n: NodeId,
            _parent: Option<NodeId>,
        ) -> bool {
            true
        }
        // port: DataFlowAnalysis#computeEscaped.visit
        fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
            if !n.is_name(t) || parent.is_some_and(|p| p.is_function(t)) {
                return;
            }
            let name = n.get_string(t);
            let scope = t.get_scope();
            let compiler = t.get_compiler();
            let Some(var) = scope.get_var(compiler, &name) else {
                return;
            };
            let variable_cfg_scope = var.get_scope(compiler).get_closest_cfg_root_scope(compiler);
            if variable_cfg_scope != self.js_scope {
                return;
            }
            let reference_cfg_scope = scope.get_closest_cfg_root_scope(compiler);
            if reference_cfg_scope != variable_cfg_scope
                || is_referenced_in_non_static_class_field(compiler, scope, variable_cfg_scope)
            {
                self.escaped.insert(var);
            }
        }
    }
    let mut finder = Finder { js_scope, escaped };
    NodeTraversal::builder()
        .set_compiler(compiler)
        .set_scope_creator(scope_creator)
        .set_callback(&mut finder)
        .traverse_at_scope(js_scope);
    for &var in all_vars_in_fn.values() {
        if var.get_parent_node(compiler).unwrap().is_catch(compiler)
            || compiler
                .get_coding_convention()
                .is_exported_name(&var.get_name(compiler))
        {
            escaped.insert(var);
        }
    }
}
// port: DataFlowAnalysis#isReferencedInNonStaticClassField
fn is_referenced_in_non_static_class_field(
    compiler: &AbstractCompiler,
    ref_scope: crate::scope::ScopeId,
    target_scope: crate::scope::ScopeId,
) -> bool {
    let mut s = Some(ref_scope);
    while let Some(scope) = s {
        if scope == target_scope {
            break;
        }
        let root = scope.get_root_node(compiler);
        if (root.is_member_field_def(compiler) || root.is_computed_field_def(compiler))
            && !root.is_static_member(compiler)
        {
            return true;
        }
        s = scope.get_parent(compiler);
    }
    false
}

impl<L: crate::graph::annotation::Annotation> crate::graph::annotation::Annotation
    for LinearFlowState<L>
{
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    // port: DataFlowAnalysis.LinearFlowState#toString
    fn to_string(&self) -> String {
        format!(
            "IN: {} OUT: {}",
            self.input.to_string(),
            self.output.to_string()
        )
    }
}

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
//   src/com/google/javascript/jscomp/MaybeReachingVariableUse.java.

use crate::graph::{adjacency_graph::AdjacencyGraph, annotatable::Annotatable, graph::GraphEdge};
use crate::{
    AbstractCompiler,
    control_flow_graph::{Branch, ControlFlowGraph},
    data_flow_analysis::{
        DataFlowAnalysis, DataFlowAnalysisState, FlowJoiner, LatticeEquals, LinearFlowState,
    },
    graph::di_graph::DiGraph,
    node_util::NodeUtil,
    var::VarId,
};
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_not_null, check_state, hamt_pmap::HamtPMap, js_string::JsString, node::NodeId,
    token::Token,
};
#[derive(Clone)]
pub struct ReachingUses {
    may_use_pmap: HamtPMap<VarId, HamtPMap<NodeId, NodeId>>,
}
impl Default for ReachingUses {
    fn default() -> Self {
        Self::new()
    }
}
impl ReachingUses {
    // port: MaybeReachingVariableUse.ReachingUses#ReachingUses()
    pub fn new() -> Self {
        Self {
            may_use_pmap: HamtPMap::empty(),
        }
    }
    // port: MaybeReachingVariableUse.ReachingUses#ReachingUses(ReachingUses)
    pub fn copy(other: &Self) -> Self {
        Self {
            may_use_pmap: other.may_use_pmap.clone(),
        }
    }
    // port: MaybeReachingVariableUse.ReachingUses#get
    pub fn get(&self, v: VarId) -> Vec<NodeId> {
        self.may_use_pmap
            .get(&v)
            .map(|uses| uses.keys().copied().collect())
            .unwrap_or_default()
    }
    // port: MaybeReachingVariableUse.ReachingUses#removeAll
    pub fn remove_all(&mut self, v: VarId) {
        self.may_use_pmap = self.may_use_pmap.minus(&v);
    }
    // port: MaybeReachingVariableUse.ReachingUses#put
    pub fn put(&mut self, v: VarId, n: NodeId) {
        let values = self
            .may_use_pmap
            .get(&v)
            .cloned()
            .unwrap_or_else(HamtPMap::empty);
        let new_values = values.plus(n, n);
        if !new_values.ptr_eq(&values) {
            self.may_use_pmap = self.may_use_pmap.plus(v, new_values);
        }
    }
    // port: MaybeReachingVariableUse.ReachingUses#join
    pub fn join(&mut self, other: &Self) {
        self.may_use_pmap =
            self.may_use_pmap
                .reconcile(&other.may_use_pmap, &mut |_: &VarId,
                                                      this_val: Option<
                    &HamtPMap<NodeId, NodeId>,
                >,
                                                      that_val: Option<
                    &HamtPMap<NodeId, NodeId>,
                >| {
                    if this_val.is_none() {
                        return that_val.unwrap().clone();
                    }
                    if that_val.is_none() {
                        return this_val.unwrap().clone();
                    }
                    this_val.unwrap().reconcile(
                        that_val.unwrap(),
                        &mut |node: &NodeId, _: Option<&NodeId>, _: Option<&NodeId>| *node,
                    )
                });
    }
    // port: MaybeReachingVariableUse.ReachingUses#equalMaps
    fn equal_maps(map1: &HamtPMap<NodeId, NodeId>, map2: &HamtPMap<NodeId, NodeId>) -> bool {
        map1.ptr_eq(map2) || map1.equivalent(map2, Self::equal_nodes)
    }
    // port: MaybeReachingVariableUse.ReachingUses#equalNodes
    fn equal_nodes(n1: &NodeId, n2: &NodeId) -> bool {
        n1 == n2
    }
    // port: MaybeReachingVariableUse.ReachingUses#hashCode
    pub fn hash_code(&self) -> i32 {
        panic!("the hashcode of this object is not stable")
    }
}
impl PartialEq for ReachingUses {
    // port: MaybeReachingVariableUse.ReachingUses#equals
    fn eq(&self, other: &Self) -> bool {
        other
            .may_use_pmap
            .equivalent(&self.may_use_pmap, Self::equal_maps)
    }
}
impl LatticeEquals for ReachingUses {
    // Rust-only (LatticeEquals): MaybeReachingVariableUse.ReachingUses#equals, as DataFlowAnalysis#flow calls it
    fn lattice_equals(&self, _compiler: &mut AbstractCompiler, other: &Self) -> bool {
        self == other
    }
}
pub struct ReachingUsesJoinOp {
    result: ReachingUses,
}
impl FlowJoiner<ReachingUses> for ReachingUsesJoinOp {
    // port: MaybeReachingVariableUse.ReachingUsesJoinOp#joinFlow
    fn join_flow(&mut self, _compiler: &mut AbstractCompiler, uses: ReachingUses) {
        self.result.join(&uses);
    }
    // port: MaybeReachingVariableUse.ReachingUsesJoinOp#finish
    fn finish(self: Box<Self>) -> ReachingUses {
        self.result
    }
}
pub struct MaybeReachingVariableUse {
    state: DataFlowAnalysisState<NodeId>,
    escaped: IndexSet<VarId>,
    all_vars_in_fn: IndexMap<JsString, VarId>,
}
impl MaybeReachingVariableUse {
    // port: MaybeReachingVariableUse#MaybeReachingVariableUse
    pub fn new(
        cfg: ControlFlowGraph<NodeId>,
        escaped: IndexSet<VarId>,
        all_vars_in_fn: IndexMap<JsString, VarId>,
    ) -> Self {
        Self {
            state: DataFlowAnalysisState::new(cfg, false, false),
            escaped,
            all_vars_in_fn,
        }
    }
    pub fn into_cfg(self) -> ControlFlowGraph<NodeId> {
        self.state.into_cfg()
    }
    // port: MaybeReachingVariableUse#hasExceptionHandler
    fn has_exception_handler(&self, cfg_node: NodeId) -> bool {
        for edge in self.get_cfg().get_out_edges(&Some(cfg_node)) {
            if *edge.get_value(self.get_cfg()) == Branch::ON_EX {
                return true;
            }
        }
        false
    }
    // port: MaybeReachingVariableUse#computeMayUse
    fn compute_may_use(
        &self,
        compiler: &AbstractCompiler,
        n: NodeId,
        cfg_node: NodeId,
        output: &mut ReachingUses,
        conditional: bool,
    ) {
        match n.get_token(compiler) {
            Token::BLOCK | Token::ROOT | Token::FUNCTION => {}
            Token::NAME => {
                if NodeUtil::is_lhs_by_destructuring(compiler, n) {
                    if !conditional {
                        self.remove_from_use_if_local(compiler, &n.get_string(compiler), output);
                    }
                } else {
                    self.add_to_use_if_local(compiler, &n.get_string(compiler), cfg_node, output);
                }
            }
            Token::WHILE | Token::DO | Token::IF | Token::FOR => {
                let cond = NodeUtil::get_condition_expression(compiler, n).unwrap();
                self.compute_may_use(compiler, cond, cfg_node, output, conditional);
            }
            Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                let mut lhs = n.get_first_child(compiler).unwrap();
                let rhs = lhs.get_next(compiler).unwrap();
                if NodeUtil::is_name_declaration(compiler, Some(lhs)) {
                    lhs = lhs.get_last_child(compiler).unwrap();
                    if lhs.is_destructuring_lhs(compiler) {
                        lhs = lhs.get_first_child(compiler).unwrap();
                    }
                }
                if lhs.is_destructuring_pattern(compiler) {
                    self.compute_may_use(compiler, lhs, cfg_node, output, true);
                }
                self.compute_may_use(compiler, rhs, cfg_node, output, conditional);
            }
            Token::AND
            | Token::OR
            | Token::COALESCE
            | Token::OPTCHAIN_GETPROP
            | Token::OPTCHAIN_GETELEM => {
                self.compute_may_use(
                    compiler,
                    n.get_last_child(compiler).unwrap(),
                    cfg_node,
                    output,
                    true,
                );
                self.compute_may_use(
                    compiler,
                    n.get_first_child(compiler).unwrap(),
                    cfg_node,
                    output,
                    conditional,
                );
            }
            Token::OPTCHAIN_CALL => {
                let mut c = n.get_last_child(compiler);
                while c != n.get_first_child(compiler) {
                    let child = c.unwrap();
                    self.compute_may_use(compiler, child, cfg_node, output, true);
                    c = child.get_previous(compiler);
                }
                self.compute_may_use(
                    compiler,
                    n.get_first_child(compiler).unwrap(),
                    cfg_node,
                    output,
                    conditional,
                );
            }
            Token::HOOK => {
                self.compute_may_use(
                    compiler,
                    n.get_last_child(compiler).unwrap(),
                    cfg_node,
                    output,
                    true,
                );
                self.compute_may_use(
                    compiler,
                    n.get_second_child(compiler).unwrap(),
                    cfg_node,
                    output,
                    true,
                );
                self.compute_may_use(
                    compiler,
                    n.get_first_child(compiler).unwrap(),
                    cfg_node,
                    output,
                    conditional,
                );
            }
            Token::VAR | Token::LET | Token::CONST => {
                check_state!(
                    n.has_children(compiler),
                    "AST should be normalized (%s)",
                    n.to_string(compiler)
                );
                let var_name = n.get_first_child(compiler).unwrap();
                if var_name.is_destructuring_lhs(compiler) {
                    self.compute_may_use(
                        compiler,
                        var_name.get_first_child(compiler).unwrap(),
                        cfg_node,
                        output,
                        conditional,
                    );
                    self.compute_may_use(
                        compiler,
                        var_name.get_second_child(compiler).unwrap(),
                        cfg_node,
                        output,
                        conditional,
                    );
                } else if let Some(child) = var_name.get_first_child(compiler) {
                    self.compute_may_use(compiler, child, cfg_node, output, conditional);
                    if !conditional {
                        self.remove_from_use_if_local(
                            compiler,
                            &var_name.get_string(compiler),
                            output,
                        );
                    }
                }
            }
            Token::DEFAULT_VALUE => {
                let first = n.get_first_child(compiler).unwrap();
                let second = n.get_second_child(compiler).unwrap();
                if first.is_destructuring_pattern(compiler) {
                    self.compute_may_use(compiler, first, cfg_node, output, conditional);
                    self.compute_may_use(compiler, second, cfg_node, output, true);
                } else if first.is_name(compiler) {
                    if !conditional {
                        self.remove_from_use_if_local(
                            compiler,
                            &first.get_string(compiler),
                            output,
                        );
                    }
                    self.compute_may_use(compiler, second, cfg_node, output, true);
                } else {
                    self.compute_may_use(compiler, second, cfg_node, output, true);
                    self.compute_may_use(compiler, first, cfg_node, output, conditional);
                }
            }
            _ => {
                if NodeUtil::is_assignment_op(compiler, n)
                    && n.get_first_child(compiler).unwrap().is_name(compiler)
                {
                    check_state!(!NodeUtil::is_logical_assignment_op(compiler, n));
                    let name = n.get_first_child(compiler).unwrap();
                    if !conditional {
                        self.remove_from_use_if_local(compiler, &name.get_string(compiler), output);
                    }
                    if !n.is_assign(compiler) {
                        self.add_to_use_if_local(
                            compiler,
                            &name.get_string(compiler),
                            cfg_node,
                            output,
                        );
                    }
                    self.compute_may_use(
                        compiler,
                        name.get_next(compiler).unwrap(),
                        cfg_node,
                        output,
                        conditional,
                    );
                } else if n.is_assign(compiler)
                    && n.get_first_child(compiler)
                        .unwrap()
                        .is_destructuring_pattern(compiler)
                {
                    self.compute_may_use(
                        compiler,
                        n.get_first_child(compiler).unwrap(),
                        cfg_node,
                        output,
                        conditional,
                    );
                    self.compute_may_use(
                        compiler,
                        n.get_second_child(compiler).unwrap(),
                        cfg_node,
                        output,
                        conditional,
                    );
                } else {
                    let mut c = n.get_last_child(compiler);
                    while let Some(child) = c {
                        self.compute_may_use(compiler, child, cfg_node, output, conditional);
                        c = child.get_previous(compiler);
                    }
                }
            }
        }
    }
    // port: MaybeReachingVariableUse#addToUseIfLocal
    fn add_to_use_if_local(
        &self,
        compiler: &AbstractCompiler,
        name: &JsString,
        node: NodeId,
        uses: &mut ReachingUses,
    ) {
        let Some(&var) = self.all_vars_in_fn.get(name) else {
            return;
        };
        // Java Set<Var>.contains uses inherited ScopedName equality across recreated scopes.
        if !self
            .escaped
            .iter()
            .any(|escaped| escaped.equals(compiler, var))
        {
            uses.put(var, node);
        }
    }
    // port: MaybeReachingVariableUse#removeFromUseIfLocal
    fn remove_from_use_if_local(
        &self,
        compiler: &AbstractCompiler,
        name: &JsString,
        uses: &mut ReachingUses,
    ) {
        let Some(&var) = self.all_vars_in_fn.get(name) else {
            return;
        };
        // Java Set<Var>.contains uses inherited ScopedName equality across recreated scopes.
        if !self
            .escaped
            .iter()
            .any(|escaped| escaped.equals(compiler, var))
        {
            uses.remove_all(var);
        }
    }
    // port: MaybeReachingVariableUse#getUses
    pub fn get_uses(&self, name: &JsString, def_node: NodeId) -> Vec<NodeId> {
        let n = check_not_null!(self.get_cfg().get_node(&Some(def_node)));
        let state = n
            .get_annotation_as::<LinearFlowState<ReachingUses>>(self.get_cfg())
            .unwrap();
        self.all_vars_in_fn
            .get(name)
            .map(|&v| state.get_out().get(v))
            .unwrap_or_default()
    }
}
impl DataFlowAnalysis<NodeId, ReachingUses> for MaybeReachingVariableUse {
    fn state(&self) -> &DataFlowAnalysisState<NodeId> {
        &self.state
    }
    fn state_mut(&mut self) -> &mut DataFlowAnalysisState<NodeId> {
        &mut self.state
    }
    // port: MaybeReachingVariableUse#isForward
    fn is_forward(&self) -> bool {
        false
    }
    // port: MaybeReachingVariableUse#createEntryLattice
    fn create_entry_lattice(&mut self, _compiler: &mut AbstractCompiler) -> ReachingUses {
        ReachingUses::new()
    }
    // port: MaybeReachingVariableUse#createInitialEstimateLattice
    fn create_initial_estimate_lattice(&self) -> ReachingUses {
        ReachingUses::new()
    }
    // port: MaybeReachingVariableUse#createFlowJoiner
    fn create_flow_joiner(&self) -> Box<dyn FlowJoiner<ReachingUses>> {
        Box::new(ReachingUsesJoinOp {
            result: ReachingUses::new(),
        })
    }
    // port: MaybeReachingVariableUse#flowThrough
    fn flow_through(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        input: ReachingUses,
    ) -> ReachingUses {
        let mut output = ReachingUses::copy(&input);
        let conditional = self.has_exception_handler(n);
        self.compute_may_use(compiler, n, n, &mut output, conditional);
        output
    }
    fn node_to_string(&self, compiler: &AbstractCompiler, node: NodeId) -> String {
        node.to_string(compiler)
    }
}

impl crate::graph::annotation::Annotation for ReachingUses {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    // port: Object#toString (ReachingUses inherits Object's hashCode call)
    fn to_string(&self) -> String {
        format!(
            "com.google.javascript.jscomp.MaybeReachingVariableUse$ReachingUses@{:x}",
            self.hash_code() as u32
        )
    }
}

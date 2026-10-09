/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/LiveVariablesAnalysis.java.

use crate::graph::graph::GraphEdge;
use crate::{
    AbstractCompiler, AllVarsDeclaredInFunction,
    control_flow_graph::{Branch, ControlFlowGraph},
    data_flow_analysis::{
        DataFlowAnalysis, DataFlowAnalysisState, FlowJoiner, LatticeEquals, compute_escaped,
    },
    graph::di_graph::DiGraph,
    node_util::NodeUtil,
    scope::ScopeId,
    scope_creator::ScopeCreator,
    var::VarId,
};
use closure_rhino::{
    check_state, java_util::bit_set::BitSet, js_string::JsString, node::NodeId, token::Token,
};
use indexmap::{IndexMap, IndexSet};
use std::fmt;
pub const MAX_VARIABLES_TO_ANALYZE: i32 = 100;
#[derive(Clone, Debug)]
pub struct LiveVariableLattice {
    pub live_set: BitSet,
}
impl LiveVariableLattice {
    // port: LiveVariablesAnalysis.LiveVariableLattice#LiveVariableLattice(int)
    pub fn new(num_vars: i32) -> Self {
        Self {
            live_set: BitSet::new(num_vars),
        }
    }
    // port: LiveVariablesAnalysis.LiveVariableLattice#LiveVariableLattice(LiveVariableLattice)
    pub fn copy(other: &Self) -> Self {
        other.clone()
    }
    // port: LiveVariablesAnalysis.LiveVariableLattice#isLive
    pub fn is_live(&self, index: i32) -> bool {
        self.live_set.get(index)
    }
    // port: LiveVariablesAnalysis.LiveVariableLattice#nextSetBit
    pub fn next_set_bit(&self, from_index: i32) -> i32 {
        self.live_set.next_set_bit(from_index)
    }
    // port: LiveVariablesAnalysis.LiveVariableLattice#hashCode
    pub fn hash_code(&self) -> i32 {
        self.live_set.hash_code()
    }
}
impl PartialEq for LiveVariableLattice {
    // port: LiveVariablesAnalysis.LiveVariableLattice#equals
    fn eq(&self, other: &Self) -> bool {
        self.live_set == other.live_set
    }
}
impl Eq for LiveVariableLattice {}
impl std::hash::Hash for LiveVariableLattice {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.hash_code().hash(h);
    }
}
impl LatticeEquals for LiveVariableLattice {
    // Rust-only (LatticeEquals): LiveVariablesAnalysis.LiveVariableLattice#equals, as DataFlowAnalysis#flow calls it
    fn lattice_equals(&self, _compiler: &mut AbstractCompiler, other: &Self) -> bool {
        self == other
    }
}
impl fmt::Display for LiveVariableLattice {
    // port: LiveVariablesAnalysis.LiveVariableLattice#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.live_set.fmt(f)
    }
}
pub struct LiveVariableJoinOp {
    result: LiveVariableLattice,
}
impl FlowJoiner<LiveVariableLattice> for LiveVariableJoinOp {
    // port: LiveVariablesAnalysis.LiveVariableJoinOp#joinFlow
    fn join_flow(&mut self, _compiler: &mut AbstractCompiler, x: LiveVariableLattice) {
        self.result.live_set.or(&x.live_set);
    }
    // port: LiveVariablesAnalysis.LiveVariableJoinOp#finish
    fn finish(self: Box<Self>) -> LiveVariableLattice {
        self.result
    }
}
pub struct LiveVariablesAnalysis {
    state: DataFlowAnalysisState<NodeId>,
    js_scope: ScopeId,
    js_scope_child: Option<ScopeId>,
    escaped: IndexSet<VarId>,
    scope_variables: IndexMap<JsString, i32>,
    ordered_vars: Vec<VarId>,
    all_vars_in_fn: IndexMap<JsString, VarId>,
}
impl LiveVariablesAnalysis {
    // port: LiveVariablesAnalysis#LiveVariablesAnalysis
    pub fn new(
        compiler: &mut AbstractCompiler,
        cfg: ControlFlowGraph<NodeId>,
        js_scope: ScopeId,
        js_scope_child: Option<ScopeId>,
        scope_creator: &mut dyn ScopeCreator,
        all_vars_declared_in_function: AllVarsDeclaredInFunction,
    ) -> Self {
        check_state!(
            js_scope.is_function_scope(compiler),
            "%s",
            js_scope.to_string(compiler)
        );
        let mut analysis = Self {
            state: DataFlowAnalysisState::new(cfg, false, false),
            js_scope,
            js_scope_child,
            escaped: IndexSet::new(),
            scope_variables: IndexMap::new(),
            ordered_vars: all_vars_declared_in_function
                .get_all_variables_in_order()
                .to_vec(),
            all_vars_in_fn: all_vars_declared_in_function.get_all_variables().clone(),
        };
        compute_escaped(
            compiler,
            js_scope,
            &mut analysis.escaped,
            scope_creator,
            &analysis.all_vars_in_fn,
        );
        analysis.add_scope_variables(compiler);
        analysis
    }
    // port: LiveVariablesAnalysis#addScopeVariables
    fn add_scope_variables(&mut self, compiler: &AbstractCompiler) {
        for (num, &v) in self.ordered_vars.iter().enumerate() {
            self.scope_variables
                .insert(v.get_name(compiler), num as i32);
        }
    }
    // port: LiveVariablesAnalysis#getEscapedLocals
    pub fn get_escaped_locals(&self) -> &IndexSet<VarId> {
        &self.escaped
    }
    // port: LiveVariablesAnalysis#getAllVariables
    pub fn get_all_variables(&self) -> &IndexMap<JsString, VarId> {
        &self.all_vars_in_fn
    }
    // port: LiveVariablesAnalysis#getAllVariablesInOrder
    pub fn get_all_variables_in_order(&self) -> &[VarId] {
        &self.ordered_vars
    }
    // port: LiveVariablesAnalysis#getVarIndex
    pub fn get_var_index(&self, var: &JsString) -> i32 {
        self.scope_variables[var]
    }
    pub fn into_cfg(self) -> ControlFlowGraph<NodeId> {
        self.state.into_cfg()
    }
    // port: LiveVariablesAnalysis#computeGenKill
    fn compute_gen_kill(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        r#gen: &mut BitSet,
        kill: &mut BitSet,
        conditional: bool,
    ) {
        match n.get_token(compiler) {
            Token::SCRIPT | Token::ROOT | Token::FUNCTION | Token::BLOCK => {}
            Token::WHILE | Token::DO | Token::IF | Token::FOR => self.compute_gen_kill(
                compiler,
                NodeUtil::get_condition_expression(compiler, n).unwrap(),
                r#gen,
                kill,
                conditional,
            ),
            Token::FOR_OF | Token::FOR_AWAIT_OF | Token::FOR_IN => {
                let mut lhs = n.get_first_child(compiler).unwrap();
                if NodeUtil::is_name_declaration(compiler, Some(lhs)) {
                    lhs = lhs.get_last_child(compiler).unwrap();
                }
                self.compute_gen_kill(compiler, lhs, r#gen, kill, conditional);
            }
            Token::LET | Token::CONST | Token::VAR => {
                for c in n.children(compiler).collect::<Vec<_>>() {
                    if c.is_name(compiler) {
                        if let Some(child) = c.get_first_child(compiler) {
                            self.compute_gen_kill(compiler, child, r#gen, kill, conditional);
                            if !conditional {
                                self.add_to_set_if_local(compiler, c, kill);
                            }
                        }
                    } else {
                        check_state!(
                            c.is_destructuring_lhs(compiler),
                            "%s",
                            c.to_string(compiler)
                        );
                        if !conditional {
                            NodeUtil::visit_lhs_nodes_in_node(compiler, c, &mut |compiler, lhs| {
                                self.add_to_set_if_local(compiler, lhs, kill)
                            });
                        }
                        self.compute_gen_kill(
                            compiler,
                            c.get_first_child(compiler).unwrap(),
                            r#gen,
                            kill,
                            conditional,
                        );
                        self.compute_gen_kill(
                            compiler,
                            c.get_second_child(compiler).unwrap(),
                            r#gen,
                            kill,
                            conditional,
                        );
                    }
                }
            }
            Token::AND
            | Token::OR
            | Token::COALESCE
            | Token::OPTCHAIN_GETELEM
            | Token::OPTCHAIN_GETPROP => {
                self.compute_gen_kill(
                    compiler,
                    n.get_first_child(compiler).unwrap(),
                    r#gen,
                    kill,
                    conditional,
                );
                self.compute_gen_kill(
                    compiler,
                    n.get_last_child(compiler).unwrap(),
                    r#gen,
                    kill,
                    true,
                );
            }
            Token::OPTCHAIN_CALL => {
                self.compute_gen_kill(
                    compiler,
                    n.get_first_child(compiler).unwrap(),
                    r#gen,
                    kill,
                    conditional,
                );
                let mut c = n.get_second_child(compiler);
                while let Some(child) = c {
                    self.compute_gen_kill(compiler, child, r#gen, kill, true);
                    c = child.get_next(compiler);
                }
            }
            Token::HOOK => {
                self.compute_gen_kill(
                    compiler,
                    n.get_first_child(compiler).unwrap(),
                    r#gen,
                    kill,
                    conditional,
                );
                self.compute_gen_kill(
                    compiler,
                    n.get_second_child(compiler).unwrap(),
                    r#gen,
                    kill,
                    true,
                );
                self.compute_gen_kill(
                    compiler,
                    n.get_last_child(compiler).unwrap(),
                    r#gen,
                    kill,
                    true,
                );
            }
            Token::NAME => {
                if n.get_string_ref(compiler) == "arguments" {
                    self.mark_all_parameters_escaped(compiler);
                } else if !NodeUtil::is_lhs_by_destructuring(compiler, n) {
                    self.add_to_set_if_local(compiler, n, r#gen);
                }
            }
            _ => {
                if NodeUtil::is_assignment_op(compiler, n)
                    && n.get_first_child(compiler).unwrap().is_name(compiler)
                {
                    let lhs = n.get_first_child(compiler).unwrap();
                    if !conditional {
                        self.add_to_set_if_local(compiler, lhs, kill);
                    }
                    if !n.is_assign(compiler) {
                        self.add_to_set_if_local(compiler, lhs, r#gen);
                    }
                    self.compute_gen_kill(
                        compiler,
                        lhs.get_next(compiler).unwrap(),
                        r#gen,
                        kill,
                        conditional,
                    );
                } else if n.is_assign(compiler)
                    && n.get_first_child(compiler)
                        .unwrap()
                        .is_destructuring_pattern(compiler)
                {
                    if !conditional {
                        NodeUtil::visit_lhs_nodes_in_node(compiler, n, &mut |compiler, child| {
                            if child.is_name(compiler) {
                                self.add_to_set_if_local(compiler, child, kill);
                            }
                        });
                    }
                    self.compute_gen_kill(
                        compiler,
                        n.get_first_child(compiler).unwrap(),
                        r#gen,
                        kill,
                        conditional,
                    );
                    self.compute_gen_kill(
                        compiler,
                        n.get_second_child(compiler).unwrap(),
                        r#gen,
                        kill,
                        conditional,
                    );
                } else {
                    for c in n.children(compiler).collect::<Vec<_>>() {
                        self.compute_gen_kill(compiler, c, r#gen, kill, conditional);
                    }
                }
            }
        }
    }
    // port: LiveVariablesAnalysis#addToSetIfLocal
    fn add_to_set_if_local(&self, compiler: &AbstractCompiler, node: NodeId, set: &mut BitSet) {
        check_state!(node.is_name(compiler), "%s", node.to_string(compiler));
        let name = node.get_string(compiler);
        let Some(&var) = self.all_vars_in_fn.get(&name) else {
            return;
        };
        let local_scope = var.get_scope(compiler);
        let local = if local_scope.is_function_block_scope(compiler) {
            Self::is_declared_in_function_block_or_parameter(compiler, local_scope, &name)
        } else if local_scope == self.js_scope && self.js_scope_child.is_some() {
            Self::is_declared_in_function_block_or_parameter(
                compiler,
                self.js_scope_child.unwrap(),
                &name,
            )
        } else {
            local_scope.has_own_slot(compiler, &name)
        };
        if !local {
            return;
        }
        // Java Set<Var>.contains uses inherited ScopedName equality across recreated scopes.
        if !self
            .escaped
            .iter()
            .any(|escaped| escaped.equals(compiler, var))
        {
            set.set(self.get_var_index(&var.get_name(compiler)));
        }
    }
    // port: LiveVariablesAnalysis#isDeclaredInFunctionBlockOrParameter
    fn is_declared_in_function_block_or_parameter(
        compiler: &AbstractCompiler,
        scope: ScopeId,
        name: &JsString,
    ) -> bool {
        check_state!(scope.is_function_block_scope(compiler));
        scope.has_own_slot(compiler, name)
            || scope
                .get_parent(compiler)
                .unwrap()
                .has_own_slot(compiler, name)
    }
    // port: LiveVariablesAnalysis#markAllParametersEscaped
    pub fn mark_all_parameters_escaped(&mut self, compiler: &mut AbstractCompiler) {
        let params =
            NodeUtil::get_function_parameters(compiler, self.js_scope.get_root_node(compiler));
        for param in params.children(compiler).collect::<Vec<_>>() {
            if param.is_name(compiler) {
                self.escaped.insert(
                    self.js_scope
                        .get_var(compiler, param.get_string(compiler))
                        .unwrap(),
                );
            }
        }
    }
}
impl DataFlowAnalysis<NodeId, LiveVariableLattice> for LiveVariablesAnalysis {
    fn state(&self) -> &DataFlowAnalysisState<NodeId> {
        &self.state
    }
    fn state_mut(&mut self) -> &mut DataFlowAnalysisState<NodeId> {
        &mut self.state
    }
    // port: LiveVariablesAnalysis#isForward
    fn is_forward(&self) -> bool {
        false
    }
    // port: LiveVariablesAnalysis#createEntryLattice
    fn create_entry_lattice(&mut self, _compiler: &mut AbstractCompiler) -> LiveVariableLattice {
        LiveVariableLattice::new(self.ordered_vars.len() as i32)
    }
    // port: LiveVariablesAnalysis#createInitialEstimateLattice
    fn create_initial_estimate_lattice(&self) -> LiveVariableLattice {
        LiveVariableLattice::new(self.ordered_vars.len() as i32)
    }
    // port: LiveVariablesAnalysis#createFlowJoiner
    fn create_flow_joiner(&self) -> Box<dyn FlowJoiner<LiveVariableLattice>> {
        Box::new(LiveVariableJoinOp {
            result: LiveVariableLattice::new(self.ordered_vars.len() as i32),
        })
    }
    // port: LiveVariablesAnalysis#flowThrough
    fn flow_through(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
        input: LiveVariableLattice,
    ) -> LiveVariableLattice {
        let mut r#gen = BitSet::new(input.live_set.size());
        let mut kill = BitSet::new(input.live_set.size());
        let mut conditional = false;
        for edge in self.get_cfg().get_out_edges(&Some(node)) {
            if *edge.get_value(self.get_cfg()) == Branch::ON_EX {
                conditional = true;
            }
        }
        self.compute_gen_kill(compiler, node, &mut r#gen, &mut kill, conditional);
        let mut result = LiveVariableLattice::copy(&input);
        result.live_set.and_not(&kill);
        result.live_set.or(&r#gen);
        result
    }
    fn node_to_string(&self, compiler: &AbstractCompiler, node: NodeId) -> String {
        node.to_string(compiler)
    }
}

impl crate::graph::annotation::Annotation for LiveVariableLattice {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn to_string(&self) -> String {
        self.live_set.to_string()
    }
}

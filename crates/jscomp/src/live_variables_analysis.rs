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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/LiveVariablesAnalysis.java.

use crate::graph::adjacency_graph::AdjacencyGraph;
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
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_state,
    java_util::bit_set::BitSet,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
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
/// Rust-only (performance divergence): scope/variable metadata extracted on the main thread so
/// `ControlFlowAnalysis` + `LiveVariablesAnalysis::analyze_readonly` can run concurrently on
/// worker threads with only `&Ast`.
pub(crate) struct PreparedLiveness {
    escaped: IndexSet<VarId>,
    scope_variables: IndexMap<JsString, i32>,
    ordered_vars: Vec<VarId>,
    all_vars_in_fn: IndexMap<JsString, VarId>,
    local_var_indices: IndexMap<JsString, i32>,
    escaped_bits: BitSet,
    ordered_escaped_bits: BitSet,
    param_vars: Vec<(VarId, Option<i32>, Option<i32>)>,
}

/// Rust-only (performance divergence): worker-local liveness result borrowing `PreparedLiveness`
/// metadata from the main thread so all CFG and lattice allocations stay and drop on the same
/// worker thread (zero cross-thread frees).
pub(crate) struct BorrowedLiveness {
    state: DataFlowAnalysisState<NodeId>,
    ordered_escaped_bits: BitSet,
}

impl BorrowedLiveness {
    pub(crate) fn cfg(&self) -> &ControlFlowGraph<NodeId> {
        &self.state.cfg
    }

    pub(crate) fn flow_state(
        &self,
        node: crate::graph::di_graph::DiGraphNode,
    ) -> &crate::data_flow_analysis::LinearFlowState<LiveVariableLattice> {
        self.state.node_flow(node)
    }

    pub(crate) fn ordered_escaped_bits(&self) -> &BitSet {
        &self.ordered_escaped_bits
    }
}

impl PreparedLiveness {
    pub(crate) fn new(
        compiler: &mut AbstractCompiler,
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
        let (all_vars_in_fn, ordered_vars) = all_vars_declared_in_function.into_parts();
        let mut escaped = IndexSet::<_>::default();
        compute_escaped(
            compiler,
            js_scope,
            &mut escaped,
            scope_creator,
            &all_vars_in_fn,
        );
        let mut scope_variables = IndexMap::<JsString, i32>::default();
        scope_variables.reserve(ordered_vars.len());
        let mut ordered_escaped_bits = BitSet::new(ordered_vars.len() as i32);
        for (num, &v) in ordered_vars.iter().enumerate() {
            scope_variables.insert(v.get_name(compiler), num as i32);
            if escaped.iter().any(|e| e.equals(compiler, v)) {
                ordered_escaped_bits.set(num as i32);
            }
        }
        let mut local_var_indices = IndexMap::<JsString, i32>::default();
        local_var_indices.reserve(all_vars_in_fn.len());
        let mut escaped_bits = BitSet::new(ordered_vars.len() as i32);
        for (name, &var) in &all_vars_in_fn {
            let local_scope = var.get_scope(compiler);
            let local = if local_scope.is_function_block_scope(compiler) {
                LiveVariablesAnalysis::is_declared_in_function_block_or_parameter(
                    compiler,
                    local_scope,
                    name,
                )
            } else if local_scope == js_scope && js_scope_child.is_some() {
                LiveVariablesAnalysis::is_declared_in_function_block_or_parameter(
                    compiler,
                    js_scope_child.unwrap(),
                    name,
                )
            } else {
                local_scope.has_own_slot(compiler, name)
            };
            if local {
                let idx = scope_variables[&var.get_name(compiler)];
                local_var_indices.insert(name.clone(), idx);
                if escaped.iter().any(|e| e.equals(compiler, var)) {
                    escaped_bits.set(idx);
                }
            }
        }
        let params = NodeUtil::get_function_parameters(compiler, js_scope.get_root_node(compiler));
        let mut param_vars = Vec::new();
        let mut child = params.get_first_child(compiler);
        while let Some(param) = child {
            child = param.get_next(compiler);
            if param.is_name(compiler) {
                let param_var = js_scope.get_var_of_node(compiler, param).unwrap();
                let name = param.get_string_ref(compiler);
                let idx = local_var_indices
                    .get(name)
                    .copied()
                    .filter(|_| all_vars_in_fn[name].equals(compiler, param_var));
                let ord_idx = ordered_vars
                    .iter()
                    .position(|&v| v.equals(compiler, param_var))
                    .map(|p| p as i32);
                param_vars.push((param_var, idx, ord_idx));
            }
        }
        Self {
            escaped,
            scope_variables,
            ordered_vars,
            all_vars_in_fn,
            local_var_indices,
            escaped_bits,
            ordered_escaped_bits,
            param_vars,
        }
    }

    pub(crate) fn ordered_vars(&self) -> &[VarId] {
        &self.ordered_vars
    }

    pub(crate) fn all_vars(&self) -> &IndexMap<JsString, VarId> {
        &self.all_vars_in_fn
    }

    pub(crate) fn var_index(&self, name: &JsString) -> i32 {
        self.scope_variables[name]
    }

    pub(crate) fn into_all_vars(self) -> IndexMap<JsString, VarId> {
        self.all_vars_in_fn
    }

    /// Runs backward liveness analysis borrowing this `PreparedLiveness` so worker threads perform
    /// zero cross-thread allocations or frees of variable maps.
    pub(crate) fn run_borrowed(
        &self,
        ast: &Ast,
        cfg: ControlFlowGraph<NodeId>,
        escape_params_immediately: bool,
    ) -> BorrowedLiveness {
        let num_vars = self.ordered_vars.len() as i32;
        let mut state = DataFlowAnalysisState::new(cfg, false, false);
        let mut escaped_bits = self.escaped_bits.clone();
        let mut ordered_escaped_bits = self.ordered_escaped_bits.clone();
        if escape_params_immediately {
            for &(_, idx, ord_idx) in &self.param_vars {
                if let Some(idx) = idx {
                    escaped_bits.set(idx);
                }
                if let Some(ord_idx) = ord_idx {
                    ordered_escaped_bits.set(ord_idx);
                }
            }
        }
        let local_var_indices = &self.local_var_indices;
        let param_vars = &self.param_vars;
        state.run_indexed_readonly(
            ast,
            false,
            || LiveVariableLattice::new(num_vars),
            || LiveVariableLattice::new(num_vars),
            |acc, x| acc.live_set.or(&x.live_set),
            |cfg, cfg_node, node, mut input| {
                let mut r#gen = BitSet::new(input.live_set.size());
                let mut kill = BitSet::new(input.live_set.size());
                let conditional = cfg
                    .node_out_edges(cfg_node)
                    .iter()
                    .any(|&edge| *edge.get_value(cfg) == Branch::ON_EX);
                LiveVariablesAnalysis::compute_gen_kill_ast_borrowed(
                    ast,
                    node,
                    &mut r#gen,
                    &mut kill,
                    conditional,
                    local_var_indices,
                    &mut escaped_bits,
                    &mut ordered_escaped_bits,
                    param_vars,
                );
                input.live_set.and_not(&kill);
                input.live_set.or(&r#gen);
                input
            },
        );
        BorrowedLiveness {
            state,
            ordered_escaped_bits,
        }
    }

    pub(crate) fn into_analysis(self, cfg: ControlFlowGraph<NodeId>) -> LiveVariablesAnalysis {
        LiveVariablesAnalysis {
            state: DataFlowAnalysisState::new(cfg, false, false),
            escaped: self.escaped,
            scope_variables: self.scope_variables,
            ordered_vars: self.ordered_vars,
            all_vars_in_fn: self.all_vars_in_fn,
            local_var_indices: self.local_var_indices,
            escaped_bits: self.escaped_bits,
            ordered_escaped_bits: self.ordered_escaped_bits,
            param_vars: self.param_vars,
        }
    }
}

pub struct LiveVariablesAnalysis {
    state: DataFlowAnalysisState<NodeId>,
    escaped: IndexSet<VarId>,
    scope_variables: IndexMap<JsString, i32>,
    ordered_vars: Vec<VarId>,
    all_vars_in_fn: IndexMap<JsString, VarId>,
    /// Rust-only (performance divergence): precomputed local-variable indices, escaped bitsets,
    /// and parameter variable bindings so `compute_gen_kill`, `add_to_set_if_local`, and
    /// `mark_all_parameters_escaped` operate read-only on `&Ast` without scope lookups.
    local_var_indices: IndexMap<JsString, i32>,
    escaped_bits: BitSet,
    ordered_escaped_bits: BitSet,
    param_vars: Vec<(VarId, Option<i32>, Option<i32>)>,
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
        PreparedLiveness::new(
            compiler,
            js_scope,
            js_scope_child,
            scope_creator,
            all_vars_declared_in_function,
        )
        .into_analysis(cfg)
    }
    /// Rust-only (performance divergence): runs indexed backward liveness analysis read-only
    /// against `&Ast` without requiring `&mut AbstractCompiler`, safe on worker threads.
    pub(crate) fn analyze_readonly(&mut self, ast: &Ast) {
        let num_vars = self.ordered_vars.len() as i32;
        let state = &mut self.state;
        let local_var_indices = &self.local_var_indices;
        let escaped_bits = &mut self.escaped_bits;
        let ordered_escaped_bits = &mut self.ordered_escaped_bits;
        let escaped = &mut self.escaped;
        let param_vars = &self.param_vars;
        state.run_indexed_readonly(
            ast,
            false,
            || LiveVariableLattice::new(num_vars),
            || LiveVariableLattice::new(num_vars),
            |acc, x| acc.live_set.or(&x.live_set),
            |cfg, cfg_node, node, mut input| {
                let mut r#gen = BitSet::new(input.live_set.size());
                let mut kill = BitSet::new(input.live_set.size());
                let conditional = cfg
                    .node_out_edges(cfg_node)
                    .iter()
                    .any(|&edge| *edge.get_value(cfg) == Branch::ON_EX);
                Self::compute_gen_kill_ast(
                    ast,
                    node,
                    &mut r#gen,
                    &mut kill,
                    conditional,
                    local_var_indices,
                    escaped_bits,
                    ordered_escaped_bits,
                    escaped,
                    param_vars,
                );
                input.live_set.and_not(&kill);
                input.live_set.or(&r#gen);
                input
            },
        );
    }
    pub(crate) fn ordered_escaped_bits(&self) -> &BitSet {
        &self.ordered_escaped_bits
    }
    pub(crate) fn into_all_vars(self) -> IndexMap<JsString, VarId> {
        self.all_vars_in_fn
    }
    pub(crate) fn mark_all_parameters_escaped_readonly(&mut self) {
        Self::mark_params_escaped(
            &mut self.escaped,
            &mut self.escaped_bits,
            &mut self.ordered_escaped_bits,
            &self.param_vars,
        );
    }
    fn mark_params_escaped(
        escaped: &mut IndexSet<VarId>,
        escaped_bits: &mut BitSet,
        ordered_escaped_bits: &mut BitSet,
        param_vars: &[(VarId, Option<i32>, Option<i32>)],
    ) {
        for &(param_var, idx, ord_idx) in param_vars {
            escaped.insert(param_var);
            if let Some(idx) = idx {
                escaped_bits.set(idx);
            }
            if let Some(ord_idx) = ord_idx {
                ordered_escaped_bits.set(ord_idx);
            }
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
    #[allow(clippy::too_many_arguments)]
    fn compute_gen_kill_ast_borrowed(
        ast: &Ast,
        n: NodeId,
        r#gen: &mut BitSet,
        kill: &mut BitSet,
        conditional: bool,
        local_var_indices: &IndexMap<JsString, i32>,
        escaped_bits: &mut BitSet,
        ordered_escaped_bits: &mut BitSet,
        param_vars: &[(VarId, Option<i32>, Option<i32>)],
    ) {
        Self::compute_gen_kill_ast_inner(
            ast,
            n,
            r#gen,
            kill,
            conditional,
            local_var_indices,
            escaped_bits,
            ordered_escaped_bits,
            None,
            param_vars,
        );
    }

    // port: LiveVariablesAnalysis#computeGenKill
    #[allow(clippy::too_many_arguments)]
    fn compute_gen_kill_ast(
        ast: &Ast,
        n: NodeId,
        r#gen: &mut BitSet,
        kill: &mut BitSet,
        conditional: bool,
        local_var_indices: &IndexMap<JsString, i32>,
        escaped_bits: &mut BitSet,
        ordered_escaped_bits: &mut BitSet,
        escaped: &mut IndexSet<VarId>,
        param_vars: &[(VarId, Option<i32>, Option<i32>)],
    ) {
        Self::compute_gen_kill_ast_inner(
            ast,
            n,
            r#gen,
            kill,
            conditional,
            local_var_indices,
            escaped_bits,
            ordered_escaped_bits,
            Some(escaped),
            param_vars,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_gen_kill_ast_inner(
        ast: &Ast,
        n: NodeId,
        r#gen: &mut BitSet,
        kill: &mut BitSet,
        conditional: bool,
        local_var_indices: &IndexMap<JsString, i32>,
        escaped_bits: &mut BitSet,
        ordered_escaped_bits: &mut BitSet,
        mut escaped: Option<&mut IndexSet<VarId>>,
        param_vars: &[(VarId, Option<i32>, Option<i32>)],
    ) {
        match n.get_token(ast) {
            Token::SCRIPT | Token::ROOT | Token::FUNCTION | Token::BLOCK => {}
            Token::WHILE | Token::DO | Token::IF | Token::FOR => Self::compute_gen_kill_ast_inner(
                ast,
                NodeUtil::get_condition_expression(ast, n).unwrap(),
                r#gen,
                kill,
                conditional,
                local_var_indices,
                escaped_bits,
                ordered_escaped_bits,
                escaped,
                param_vars,
            ),
            Token::FOR_OF | Token::FOR_AWAIT_OF | Token::FOR_IN => {
                let mut lhs = n.get_first_child(ast).unwrap();
                if NodeUtil::is_name_declaration(ast, Some(lhs)) {
                    lhs = lhs.get_last_child(ast).unwrap();
                }
                Self::compute_gen_kill_ast_inner(
                    ast,
                    lhs,
                    r#gen,
                    kill,
                    conditional,
                    local_var_indices,
                    escaped_bits,
                    ordered_escaped_bits,
                    escaped,
                    param_vars,
                );
            }
            Token::LET | Token::CONST | Token::VAR => {
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    child = c.get_next(ast);
                    if c.is_name(ast) {
                        if let Some(init) = c.get_first_child(ast) {
                            Self::compute_gen_kill_ast_inner(
                                ast,
                                init,
                                r#gen,
                                kill,
                                conditional,
                                local_var_indices,
                                escaped_bits,
                                ordered_escaped_bits,
                                escaped.as_deref_mut(),
                                param_vars,
                            );
                            if !conditional {
                                Self::add_to_set_if_local_ast(
                                    ast,
                                    c,
                                    kill,
                                    local_var_indices,
                                    escaped_bits,
                                );
                            }
                        }
                    } else {
                        check_state!(c.is_destructuring_lhs(ast), "%s", c.to_string(ast));
                        if !conditional {
                            NodeUtil::visit_lhs_nodes_in_node_readonly(ast, c, &mut |lhs| {
                                Self::add_to_set_if_local_ast(
                                    ast,
                                    lhs,
                                    kill,
                                    local_var_indices,
                                    escaped_bits,
                                )
                            });
                        }
                        Self::compute_gen_kill_ast_inner(
                            ast,
                            c.get_first_child(ast).unwrap(),
                            r#gen,
                            kill,
                            conditional,
                            local_var_indices,
                            escaped_bits,
                            ordered_escaped_bits,
                            escaped.as_deref_mut(),
                            param_vars,
                        );
                        Self::compute_gen_kill_ast_inner(
                            ast,
                            c.get_second_child(ast).unwrap(),
                            r#gen,
                            kill,
                            conditional,
                            local_var_indices,
                            escaped_bits,
                            ordered_escaped_bits,
                            escaped.as_deref_mut(),
                            param_vars,
                        );
                    }
                }
            }
            Token::AND
            | Token::OR
            | Token::COALESCE
            | Token::OPTCHAIN_GETELEM
            | Token::OPTCHAIN_GETPROP => {
                Self::compute_gen_kill_ast_inner(
                    ast,
                    n.get_first_child(ast).unwrap(),
                    r#gen,
                    kill,
                    conditional,
                    local_var_indices,
                    escaped_bits,
                    ordered_escaped_bits,
                    escaped.as_deref_mut(),
                    param_vars,
                );
                Self::compute_gen_kill_ast_inner(
                    ast,
                    n.get_last_child(ast).unwrap(),
                    r#gen,
                    kill,
                    true,
                    local_var_indices,
                    escaped_bits,
                    ordered_escaped_bits,
                    escaped,
                    param_vars,
                );
            }
            Token::OPTCHAIN_CALL => {
                Self::compute_gen_kill_ast_inner(
                    ast,
                    n.get_first_child(ast).unwrap(),
                    r#gen,
                    kill,
                    conditional,
                    local_var_indices,
                    escaped_bits,
                    ordered_escaped_bits,
                    escaped.as_deref_mut(),
                    param_vars,
                );
                let mut c = n.get_second_child(ast);
                while let Some(child) = c {
                    Self::compute_gen_kill_ast_inner(
                        ast,
                        child,
                        r#gen,
                        kill,
                        true,
                        local_var_indices,
                        escaped_bits,
                        ordered_escaped_bits,
                        escaped.as_deref_mut(),
                        param_vars,
                    );
                    c = child.get_next(ast);
                }
            }
            Token::HOOK => {
                Self::compute_gen_kill_ast_inner(
                    ast,
                    n.get_first_child(ast).unwrap(),
                    r#gen,
                    kill,
                    conditional,
                    local_var_indices,
                    escaped_bits,
                    ordered_escaped_bits,
                    escaped.as_deref_mut(),
                    param_vars,
                );
                Self::compute_gen_kill_ast_inner(
                    ast,
                    n.get_second_child(ast).unwrap(),
                    r#gen,
                    kill,
                    true,
                    local_var_indices,
                    escaped_bits,
                    ordered_escaped_bits,
                    escaped.as_deref_mut(),
                    param_vars,
                );
                Self::compute_gen_kill_ast_inner(
                    ast,
                    n.get_last_child(ast).unwrap(),
                    r#gen,
                    kill,
                    true,
                    local_var_indices,
                    escaped_bits,
                    ordered_escaped_bits,
                    escaped,
                    param_vars,
                );
            }
            Token::NAME => {
                if n.get_string_ref(ast) == "arguments" {
                    for &(param_var, idx, ord_idx) in param_vars {
                        if let Some(esc) = escaped.as_deref_mut() {
                            esc.insert(param_var);
                        }
                        if let Some(idx) = idx {
                            escaped_bits.set(idx);
                        }
                        if let Some(ord_idx) = ord_idx {
                            ordered_escaped_bits.set(ord_idx);
                        }
                    }
                } else if !NodeUtil::is_lhs_by_destructuring(ast, n) {
                    Self::add_to_set_if_local_ast(ast, n, r#gen, local_var_indices, escaped_bits);
                }
            }
            _ => {
                if NodeUtil::is_assignment_op(ast, n)
                    && n.get_first_child(ast).unwrap().is_name(ast)
                {
                    let lhs = n.get_first_child(ast).unwrap();
                    if !conditional {
                        Self::add_to_set_if_local_ast(
                            ast,
                            lhs,
                            kill,
                            local_var_indices,
                            escaped_bits,
                        );
                    }
                    if !n.is_assign(ast) {
                        Self::add_to_set_if_local_ast(
                            ast,
                            lhs,
                            r#gen,
                            local_var_indices,
                            escaped_bits,
                        );
                    }
                    Self::compute_gen_kill_ast_inner(
                        ast,
                        lhs.get_next(ast).unwrap(),
                        r#gen,
                        kill,
                        conditional,
                        local_var_indices,
                        escaped_bits,
                        ordered_escaped_bits,
                        escaped,
                        param_vars,
                    );
                } else if n.is_assign(ast)
                    && n.get_first_child(ast)
                        .unwrap()
                        .is_destructuring_pattern(ast)
                {
                    if !conditional {
                        NodeUtil::visit_lhs_nodes_in_node_readonly(ast, n, &mut |child| {
                            if child.is_name(ast) {
                                Self::add_to_set_if_local_ast(
                                    ast,
                                    child,
                                    kill,
                                    local_var_indices,
                                    escaped_bits,
                                );
                            }
                        });
                    }
                    Self::compute_gen_kill_ast_inner(
                        ast,
                        n.get_first_child(ast).unwrap(),
                        r#gen,
                        kill,
                        conditional,
                        local_var_indices,
                        escaped_bits,
                        ordered_escaped_bits,
                        escaped.as_deref_mut(),
                        param_vars,
                    );
                    Self::compute_gen_kill_ast_inner(
                        ast,
                        n.get_second_child(ast).unwrap(),
                        r#gen,
                        kill,
                        conditional,
                        local_var_indices,
                        escaped_bits,
                        ordered_escaped_bits,
                        escaped,
                        param_vars,
                    );
                } else {
                    let mut child = n.get_first_child(ast);
                    while let Some(c) = child {
                        child = c.get_next(ast);
                        Self::compute_gen_kill_ast_inner(
                            ast,
                            c,
                            r#gen,
                            kill,
                            conditional,
                            local_var_indices,
                            escaped_bits,
                            ordered_escaped_bits,
                            escaped.as_deref_mut(),
                            param_vars,
                        );
                    }
                }
            }
        }
    }
    // port: LiveVariablesAnalysis#addToSetIfLocal
    #[inline]
    fn add_to_set_if_local_ast(
        ast: &Ast,
        node: NodeId,
        set: &mut BitSet,
        local_var_indices: &IndexMap<JsString, i32>,
        escaped_bits: &BitSet,
    ) {
        check_state!(node.is_name(ast), "%s", node.to_string(ast));
        let name = node.get_string_ref(ast);
        let Some(&idx) = local_var_indices.get(name) else {
            return;
        };
        if !escaped_bits.get(idx) {
            set.set(idx);
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
    pub fn mark_all_parameters_escaped(&mut self, _compiler: &mut AbstractCompiler) {
        self.mark_all_parameters_escaped_readonly();
    }
}
impl DataFlowAnalysis<NodeId, LiveVariableLattice> for LiveVariablesAnalysis {
    fn state(&self) -> &DataFlowAnalysisState<NodeId> {
        &self.state
    }
    fn state_mut(&mut self) -> &mut DataFlowAnalysisState<NodeId> {
        &mut self.state
    }
    fn analyze_indexed(&mut self, compiler: &mut AbstractCompiler) {
        self.analyze_readonly(&compiler.ast);
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
        let cfg = self.get_cfg();
        let conditional = cfg.get_node(&Some(node)).is_some_and(|gn| {
            cfg.node_out_edges(gn)
                .iter()
                .any(|&edge| *edge.get_value(cfg) == Branch::ON_EX)
        });
        Self::compute_gen_kill_ast(
            &compiler.ast,
            node,
            &mut r#gen,
            &mut kill,
            conditional,
            &self.local_var_indices,
            &mut self.escaped_bits,
            &mut self.ordered_escaped_bits,
            &mut self.escaped,
            &self.param_vars,
        );
        // Java copies the shared input; here the input is owned (performance divergence).
        let mut result = input;
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

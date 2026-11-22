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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/MaybeReachingVariableUse.java.

use crate::graph::{adjacency_graph::AdjacencyGraph, graph::GraphEdge};
use crate::{
    AbstractCompiler,
    control_flow_graph::{Branch, ControlFlowGraph},
    data_flow_analysis::{DataFlowAnalysis, DataFlowAnalysisState, FlowJoiner, LatticeEquals},
    graph::di_graph::DiGraph,
    node_util::NodeUtil,
    var::VarId,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_not_null, check_state, hamt_pmap::HamtPMap, js_string::JsString, node::NodeId,
    token::Token,
};
/// A variable with one reaching read needs only its CFG node. A second distinct
/// node promotes it to the same persistent set used for larger unions.
#[derive(Clone, PartialEq)]
enum UseSites {
    One(NodeId),
    Several(HamtPMap<NodeId, NodeId>),
}

impl UseSites {
    fn with_node(&self, node: NodeId) -> Self {
        match self {
            Self::One(first) if *first == node => Self::One(node),
            Self::One(first) => {
                Self::Several(HamtPMap::empty().plus(*first, *first).plus(node, node))
            }
            Self::Several(sites) => Self::Several(sites.plus(node, node)),
        }
    }

    fn union(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::One(node), sites) | (sites, Self::One(node)) => sites.with_node(*node),
            (Self::Several(a), Self::Several(b)) => {
                Self::Several(a.union_with(b, |node, _, _| *node))
            }
        }
    }

    fn equivalent(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Several(a), Self::Several(b)) => a.equivalent(b, PartialEq::eq),
            _ => self == other,
        }
    }
}

#[derive(Clone)]
pub struct ReachingUses {
    may_use_pmap: HamtPMap<VarId, UseSites>,
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
        match self.may_use_pmap.get(&v) {
            None => Vec::new(),
            Some(UseSites::One(node)) => vec![*node],
            Some(UseSites::Several(sites)) => sites.keys().copied().collect(),
        }
    }
    // port: MaybeReachingVariableUse.ReachingUses#removeAll
    pub fn remove_all(&mut self, v: VarId) {
        self.may_use_pmap = self.may_use_pmap.minus(&v);
    }
    // port: MaybeReachingVariableUse.ReachingUses#put
    pub fn put(&mut self, v: VarId, n: NodeId) {
        let sites = match self.may_use_pmap.get(&v) {
            Some(old) => {
                let sites = old.with_node(n);
                if &sites == old {
                    return;
                }
                sites
            }
            None => UseSites::One(n),
        };
        self.may_use_pmap = self.may_use_pmap.plus(v, sites);
    }
    // port: MaybeReachingVariableUse.ReachingUses#join
    pub fn join(&mut self, other: &Self) {
        self.may_use_pmap = self
            .may_use_pmap
            .union_with(&other.may_use_pmap, |_, left, right| left.union(right));
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
            .equivalent(&self.may_use_pmap, UseSites::equivalent)
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
    pub(crate) fn variable(&self, name: &JsString) -> Option<VarId> {
        self.all_vars_in_fn.get(name).copied()
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
                        self.remove_from_use_if_local(compiler, n.get_string_ref(compiler), output);
                    }
                } else {
                    self.add_to_use_if_local(
                        compiler,
                        n.get_string_ref(compiler),
                        cfg_node,
                        output,
                    );
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
                            var_name.get_string_ref(compiler),
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
                            first.get_string_ref(compiler),
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
                        self.remove_from_use_if_local(
                            compiler,
                            name.get_string_ref(compiler),
                            output,
                        );
                    }
                    if !n.is_assign(compiler) {
                        self.add_to_use_if_local(
                            compiler,
                            name.get_string_ref(compiler),
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
        let state = self.flow_state(n);
        self.all_vars_in_fn
            .get(name)
            .map(|&v| state.get_out().get(v))
            .unwrap_or_default()
    }

    /// `FlowSensitiveInlineVariables.Candidate#hasExactlyOne` on the reaching-use set.
    pub(crate) fn has_single_use(&self, name: &JsString, def_node: NodeId) -> bool {
        let n = check_not_null!(self.get_cfg().get_node(&Some(def_node)));
        matches!(
            self.all_vars_in_fn.get(name).and_then(|var| self
                .flow_state(n)
                .get_out()
                .may_use_pmap
                .get(var)),
            Some(UseSites::One(_))
        )
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
        // Java copies the shared input; here the input is owned (performance divergence).
        let mut output = input;
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

#[cfg(test)]
mod tests {
    use super::*;
    use closure_rhino::node::Ast;
    use std::num::NonZeroU32;

    #[test]
    fn joins_count_distinct_cfg_nodes_and_preserve_set_iteration_order() {
        let mut ast = Ast::new();
        let [a, b, c] = std::array::from_fn(|_| ast.new_node(Token::EMPTY));
        let var = VarId(NonZeroU32::new(1).unwrap());
        let mut left = ReachingUses::new();
        left.put(var, a);
        let mut right = left.clone();
        right.put(var, a);
        left.join(&right);
        assert!(matches!(left.may_use_pmap.get(&var), Some(UseSites::One(n)) if *n == a));

        right.put(var, b);
        left.put(var, c);
        left.join(&right);
        let expected = HamtPMap::empty().plus(a, a).plus(c, c).plus(b, b);
        assert_eq!(left.get(var), expected.keys().copied().collect::<Vec<_>>());
        right.remove_all(var);
        right.put(var, c);
        right.join(&left);
        assert!(left == right);
        left.remove_all(var);
        assert!(left.get(var).is_empty());
        assert_eq!(right.get(var).len(), 3);
    }
}

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
//   src/com/google/javascript/jscomp/MustBeReachingVariableDef.java.

use crate::graph::{adjacency_graph::AdjacencyGraph, annotatable::Annotatable};
use crate::{
    AbstractCompiler,
    control_flow_graph::{
        AbstractCfgNodeTraversal, AbstractCfgNodeTraversalCallback, ControlFlowGraph,
    },
    data_flow_analysis::{
        DataFlowAnalysis, DataFlowAnalysisState, FlowJoiner, LatticeEquals, LinearFlowState,
    },
    graph::graph::Graph,
    node_traversal::NodeTraversal,
    node_util::NodeUtil,
    var::VarId,
};
use closure_rhino::{
    check_argument, check_state, java_lang::JavaHashCode, js_string::JsString, node::NodeId,
    token::Token,
};
use indexmap::{IndexMap, IndexSet};
use std::sync::Arc;
#[derive(Debug)]
pub struct Definition {
    pub node: NodeId,
    pub depends: IndexSet<VarId>,
    unknown_dependencies: bool,
}
impl Definition {
    // port: MustBeReachingVariableDef.Definition#Definition
    pub fn new(node: NodeId) -> Self {
        Self {
            node,
            depends: IndexSet::new(),
            unknown_dependencies: false,
        }
    }
    // port: MustBeReachingVariableDef.Definition#toString
    pub fn to_string(&self, compiler: &AbstractCompiler) -> String {
        format!("Definition@{}", self.node.to_string(compiler))
    }
    // port: MustBeReachingVariableDef.Definition#hashCode
    pub fn hash_code(&self) -> i32 {
        self.node.hash_code()
    }
}
impl PartialEq for Definition {
    // port: MustBeReachingVariableDef.Definition#equals
    fn eq(&self, other: &Self) -> bool {
        self.node == other.node
    }
}
impl Eq for Definition {}
#[derive(Clone, Default)]
pub struct MustDef {
    pub reaching_def: IndexMap<VarId, Option<Arc<Definition>>>,
}
impl MustDef {
    // port: MustBeReachingVariableDef.MustDef#MustDef()
    pub fn new() -> Self {
        Self::default()
    }
    // port: MustBeReachingVariableDef.MustDef#MustDef(Collection)
    pub fn new_with_vars(
        compiler: &AbstractCompiler,
        vars: impl IntoIterator<Item = VarId>,
    ) -> Self {
        let mut result = Self::new();
        for var in vars {
            result.reaching_def.insert(
                var,
                Some(Arc::new(Definition::new(
                    var.get_scope(compiler).get_root_node(compiler),
                ))),
            );
        }
        result
    }
    // port: MustBeReachingVariableDef.MustDef#MustDef(MustDef)
    pub fn copy(other: &Self) -> Self {
        Self {
            reaching_def: other.reaching_def.clone(),
        }
    }
    // port: MustBeReachingVariableDef.MustDef#hashCode
    pub fn hash_code(&self) -> i32 {
        self.reaching_def.iter().fold(0i32, |sum, (v, def)| {
            sum.wrapping_add(JavaHashCode::hash_code(v) ^ def.as_ref().map_or(0, |d| d.hash_code()))
        })
    }
}
impl PartialEq for MustDef {
    // port: MustBeReachingVariableDef.MustDef#equals
    fn eq(&self, other: &Self) -> bool {
        self.reaching_def.len() == other.reaching_def.len()
            && self
                .reaching_def
                .iter()
                .all(|(k, v)| other.reaching_def.get(k) == Some(v))
    }
}
impl LatticeEquals for MustDef {
    // Rust-only (LatticeEquals): MustBeReachingVariableDef.MustDef#equals, as DataFlowAnalysis#flow calls it
    fn lattice_equals(&self, _compiler: &mut AbstractCompiler, other: &Self) -> bool {
        self == other
    }
}
pub struct MustDefJoin {
    result: MustDef,
}
impl MustDefJoin {
    // port: MustBeReachingVariableDef.MustDefJoin#mergeVarDef
    fn merge_var_def(&mut self, var: VarId, def: Option<Arc<Definition>>) {
        let result_def = if def.is_none() {
            None
        } else if !self.result.reaching_def.contains_key(&var) {
            def
        } else if self.result.reaching_def.get(&var) == Some(&def) {
            return;
        } else {
            None
        };
        self.result.reaching_def.insert(var, result_def);
    }
}
impl FlowJoiner<MustDef> for MustDefJoin {
    // port: MustBeReachingVariableDef.MustDefJoin#joinFlow
    fn join_flow(&mut self, _compiler: &mut AbstractCompiler, input: MustDef) {
        for (var, def) in input.reaching_def {
            self.merge_var_def(var, def);
        }
    }
    // port: MustBeReachingVariableDef.MustDefJoin#finish
    fn finish(self: Box<Self>) -> MustDef {
        self.result
    }
}
pub struct MustBeReachingVariableDef {
    state: DataFlowAnalysisState<NodeId>,
    escaped: IndexSet<VarId>,
    all_vars_in_fn: IndexMap<JsString, VarId>,
}
impl MustBeReachingVariableDef {
    // port: MustBeReachingVariableDef#MustBeReachingVariableDef
    pub fn new(
        _compiler: &AbstractCompiler,
        cfg: ControlFlowGraph<NodeId>,
        escaped: IndexSet<VarId>,
        all_vars_in_fn: IndexMap<JsString, VarId>,
    ) -> Self {
        Self {
            state: DataFlowAnalysisState::new(cfg, true, false),
            escaped,
            all_vars_in_fn,
        }
    }
    pub fn into_cfg(self) -> ControlFlowGraph<NodeId> {
        self.state.into_cfg()
    }
    // port: MustBeReachingVariableDef#computeMustDef
    fn compute_must_def(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        cfg_node: NodeId,
        output: &mut MustDef,
        conditional: bool,
    ) {
        match n.get_token(compiler) {
            Token::BLOCK | Token::ROOT | Token::FUNCTION => {}
            Token::WHILE | Token::DO | Token::IF | Token::FOR => self.compute_must_def(
                compiler,
                NodeUtil::get_condition_expression(compiler, n).unwrap(),
                cfg_node,
                output,
                conditional,
            ),
            Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                let mut lhs = n.get_first_child(compiler).unwrap();
                let rhs = lhs.get_next(compiler).unwrap();
                if NodeUtil::is_name_declaration(compiler, Some(lhs)) {
                    lhs = lhs.get_last_child(compiler).unwrap();
                }
                if lhs.is_name(compiler) {
                    self.add_to_def_if_local(
                        compiler,
                        &lhs.get_string(compiler),
                        None,
                        Some(rhs),
                        output,
                    );
                } else if lhs.is_destructuring_lhs(compiler) {
                    lhs = lhs.get_first_child(compiler).unwrap();
                }
                if lhs.is_destructuring_pattern(compiler) {
                    self.compute_must_def(compiler, lhs, cfg_node, output, true);
                }
            }
            Token::OPTCHAIN_GETPROP => self.compute_must_def(
                compiler,
                n.get_first_child(compiler).unwrap(),
                cfg_node,
                output,
                conditional,
            ),
            Token::AND | Token::OR | Token::COALESCE | Token::OPTCHAIN_GETELEM => {
                self.compute_must_def(
                    compiler,
                    n.get_first_child(compiler).unwrap(),
                    cfg_node,
                    output,
                    conditional,
                );
                self.compute_must_def(
                    compiler,
                    n.get_last_child(compiler).unwrap(),
                    cfg_node,
                    output,
                    true,
                );
            }
            Token::OPTCHAIN_CALL => {
                self.compute_must_def(
                    compiler,
                    n.get_first_child(compiler).unwrap(),
                    cfg_node,
                    output,
                    conditional,
                );
                let mut c = n.get_second_child(compiler);
                while let Some(child) = c {
                    self.compute_must_def(compiler, child, cfg_node, output, true);
                    c = child.get_next(compiler);
                }
            }
            Token::HOOK => {
                self.compute_must_def(
                    compiler,
                    n.get_first_child(compiler).unwrap(),
                    cfg_node,
                    output,
                    conditional,
                );
                self.compute_must_def(
                    compiler,
                    n.get_second_child(compiler).unwrap(),
                    cfg_node,
                    output,
                    true,
                );
                self.compute_must_def(
                    compiler,
                    n.get_last_child(compiler).unwrap(),
                    cfg_node,
                    output,
                    true,
                );
            }
            Token::LET | Token::CONST | Token::VAR => {
                let mut c = n.get_first_child(compiler);
                while let Some(child) = c {
                    if child.has_children(compiler) {
                        if child.is_name(compiler) {
                            self.compute_must_def(
                                compiler,
                                child.get_first_child(compiler).unwrap(),
                                cfg_node,
                                output,
                                conditional,
                            );
                            self.add_to_def_if_local(
                                compiler,
                                &child.get_string(compiler),
                                if conditional { None } else { Some(cfg_node) },
                                child.get_first_child(compiler),
                                output,
                            );
                        } else {
                            check_state!(
                                child.is_destructuring_lhs(compiler),
                                "%s",
                                child.to_string(compiler)
                            );
                            self.compute_must_def(
                                compiler,
                                child.get_second_child(compiler).unwrap(),
                                cfg_node,
                                output,
                                conditional,
                            );
                            self.compute_must_def(
                                compiler,
                                child.get_first_child(compiler).unwrap(),
                                cfg_node,
                                output,
                                conditional,
                            );
                        }
                    }
                    c = child.get_next(compiler);
                }
            }
            Token::DEFAULT_VALUE => {
                let first = n.get_first_child(compiler).unwrap();
                let second = n.get_second_child(compiler).unwrap();
                if first.is_destructuring_pattern(compiler) {
                    self.compute_must_def(compiler, second, cfg_node, output, true);
                    self.compute_must_def(compiler, first, cfg_node, output, conditional);
                } else if first.is_name(compiler) {
                    self.compute_must_def(compiler, second, cfg_node, output, true);
                    self.add_to_def_if_local(
                        compiler,
                        &first.get_string(compiler),
                        if conditional { None } else { Some(cfg_node) },
                        None,
                        output,
                    );
                } else {
                    self.compute_must_def(compiler, first, cfg_node, output, conditional);
                    self.compute_must_def(compiler, second, cfg_node, output, true);
                }
            }
            Token::NAME => {
                if NodeUtil::is_lhs_by_destructuring(compiler, n) {
                    self.add_to_def_if_local(
                        compiler,
                        &n.get_string(compiler),
                        if conditional { None } else { Some(cfg_node) },
                        None,
                        output,
                    );
                } else if n.get_string_ref(compiler) == "arguments" {
                    self.escape_parameters(compiler, output);
                }
            }
            _ => {
                if NodeUtil::is_assignment_op(compiler, n) {
                    let lhs = n.get_first_child(compiler).unwrap();
                    if lhs.is_name(compiler) {
                        self.compute_must_def(
                            compiler,
                            lhs.get_next(compiler).unwrap(),
                            cfg_node,
                            output,
                            conditional,
                        );
                        self.add_to_def_if_local(
                            compiler,
                            &lhs.get_string(compiler),
                            if conditional { None } else { Some(cfg_node) },
                            n.get_last_child(compiler),
                            output,
                        );
                        return;
                    } else if NodeUtil::is_normal_get(compiler, lhs) {
                        let obj = n.get_first_first_child(compiler).unwrap();
                        if obj.is_name(compiler) && obj.get_string_ref(compiler) == "arguments" {
                            self.escape_parameters(compiler, output);
                        }
                    } else if lhs.is_destructuring_pattern(compiler) {
                        self.compute_must_def(
                            compiler,
                            n.get_second_child(compiler).unwrap(),
                            cfg_node,
                            output,
                            conditional,
                        );
                        self.compute_must_def(compiler, lhs, cfg_node, output, conditional);
                        return;
                    }
                }
                if n.is_dec(compiler) || n.is_inc(compiler) {
                    let target = n.get_first_child(compiler).unwrap();
                    if target.is_name(compiler) {
                        self.add_to_def_if_local(
                            compiler,
                            &target.get_string(compiler),
                            if conditional { None } else { Some(cfg_node) },
                            None,
                            output,
                        );
                        return;
                    }
                }
                let mut c = n.get_first_child(compiler);
                while let Some(child) = c {
                    self.compute_must_def(compiler, child, cfg_node, output, conditional);
                    c = child.get_next(compiler);
                }
            }
        }
    }
    // port: MustBeReachingVariableDef#addToDefIfLocal
    fn add_to_def_if_local(
        &self,
        compiler: &mut AbstractCompiler,
        name: &JsString,
        node: Option<NodeId>,
        r_value: Option<NodeId>,
        def: &mut MustDef,
    ) {
        let Some(&var) = self.all_vars_in_fn.get(name) else {
            return;
        };
        for other in def.reaching_def.values_mut() {
            if other.as_ref().is_some_and(|d| d.depends.contains(&var)) {
                *other = None;
            }
        }
        // Java Set<Var>.contains uses inherited ScopedName equality across recreated scopes.
        if !self
            .escaped
            .iter()
            .any(|escaped| escaped.equals(compiler, var))
        {
            if let Some(node) = node {
                let mut definition = Definition::new(node);
                if let Some(r_value) = r_value {
                    self.compute_dependence(compiler, &mut definition, r_value);
                }
                def.reaching_def.insert(var, Some(Arc::new(definition)));
            } else {
                def.reaching_def.insert(var, None);
            }
        }
    }
    // port: MustBeReachingVariableDef#escapeParameters
    fn escape_parameters(&self, compiler: &AbstractCompiler, output: &mut MustDef) {
        for &v in self.all_vars_in_fn.values() {
            if Self::is_parameter(compiler, v) {
                output.reaching_def.insert(v, None);
            }
        }
        for value in output.reaching_def.values_mut() {
            if value.as_ref().is_some_and(|d| {
                d.depends
                    .iter()
                    .any(|&dep| Self::is_parameter(compiler, dep))
            }) {
                *value = None;
            }
        }
    }
    // port: MustBeReachingVariableDef#isParameter
    fn is_parameter(compiler: &AbstractCompiler, v: VarId) -> bool {
        v.is_param(compiler)
    }
    // port: MustBeReachingVariableDef#computeDependence
    fn compute_dependence(
        &self,
        compiler: &mut AbstractCompiler,
        def: &mut Definition,
        r_value: NodeId,
    ) {
        struct Dependence<'a> {
            def: &'a mut Definition,
            all_vars_in_fn: &'a IndexMap<JsString, VarId>,
        }
        impl AbstractCfgNodeTraversalCallback for Dependence<'_> {
            // port: MustBeReachingVariableDef#computeDependence.visit
            fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
                if n.is_name(t) {
                    if let Some(&dep) = self.all_vars_in_fn.get(&n.get_string(t)) {
                        self.def.depends.insert(dep);
                    } else {
                        self.def.unknown_dependencies = true;
                    }
                }
            }
        }
        NodeTraversal::traverse(
            compiler,
            r_value,
            &mut AbstractCfgNodeTraversal(Dependence {
                def,
                all_vars_in_fn: &self.all_vars_in_fn,
            }),
        );
    }
    // port: MustBeReachingVariableDef#getDef
    pub fn get_def(&self, name: &JsString, use_node: NodeId) -> Option<Arc<Definition>> {
        check_argument!(self.get_cfg().has_node(&Some(use_node)));
        let n = self.get_cfg().get_node(&Some(use_node)).unwrap();
        let state = n
            .get_annotation_as::<LinearFlowState<MustDef>>(self.get_cfg())
            .unwrap();
        self.all_vars_in_fn
            .get(name)
            .and_then(|var| state.get_in().reaching_def.get(var))
            .cloned()
            .flatten()
    }
    // port: MustBeReachingVariableDef#getDefNode
    pub fn get_def_node(&self, name: &JsString, use_node: NodeId) -> Option<NodeId> {
        self.get_def(name, use_node).map(|d| d.node)
    }
    // port: MustBeReachingVariableDef#dependsOnOuterScopeVars
    pub fn depends_on_outer_scope_vars(
        &self,
        compiler: &AbstractCompiler,
        def: &Definition,
    ) -> bool {
        if def.unknown_dependencies {
            return true;
        }
        for &s in &def.depends {
            if s.get_scope(compiler).is_catch_scope(compiler) {
                return true;
            }
        }
        false
    }
}
impl DataFlowAnalysis<NodeId, MustDef> for MustBeReachingVariableDef {
    fn state(&self) -> &DataFlowAnalysisState<NodeId> {
        &self.state
    }
    fn state_mut(&mut self) -> &mut DataFlowAnalysisState<NodeId> {
        &mut self.state
    }
    // port: MustBeReachingVariableDef#isForward
    fn is_forward(&self) -> bool {
        true
    }
    // port: MustBeReachingVariableDef#createEntryLattice
    fn create_entry_lattice(&mut self, compiler: &mut AbstractCompiler) -> MustDef {
        MustDef::new_with_vars(compiler, self.all_vars_in_fn.values().copied())
    }
    // port: MustBeReachingVariableDef#createInitialEstimateLattice
    fn create_initial_estimate_lattice(&self) -> MustDef {
        MustDef::new()
    }
    // port: MustBeReachingVariableDef#createFlowJoiner
    fn create_flow_joiner(&self) -> Box<dyn FlowJoiner<MustDef>> {
        Box::new(MustDefJoin {
            result: MustDef::new(),
        })
    }
    // port: MustBeReachingVariableDef#flowThrough
    fn flow_through(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        input: MustDef,
    ) -> MustDef {
        let mut output = MustDef::copy(&input);
        self.compute_must_def(compiler, n, n, &mut output, false);
        output
    }
    fn node_to_string(&self, compiler: &AbstractCompiler, node: NodeId) -> String {
        node.to_string(compiler)
    }
}

impl crate::graph::annotation::Annotation for MustDef {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    // port: Object#toString (MustDef inherits Object's hashCode call)
    fn to_string(&self) -> String {
        format!(
            "com.google.javascript.jscomp.MustBeReachingVariableDef$MustDef@{:x}",
            self.hash_code() as u32
        )
    }
}

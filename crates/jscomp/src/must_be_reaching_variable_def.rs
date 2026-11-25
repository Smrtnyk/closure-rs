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
//   src/com/google/javascript/jscomp/MustBeReachingVariableDef.java.

use crate::graph::adjacency_graph::AdjacencyGraph;
use crate::{
    AbstractCompiler,
    control_flow_graph::ControlFlowGraph,
    data_flow_analysis::{DataFlowAnalysis, DataFlowAnalysisState, FlowJoiner, LatticeEquals},
    graph::graph::Graph,
    node_util::NodeUtil,
    var::VarId,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, check_state,
    java_lang::JavaHashCode,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use std::rc::Rc;
#[derive(Clone, Debug)]
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
            depends: IndexSet::<_>::default(),
            unknown_dependencies: false,
        }
    }
    fn record_dependency(&mut self, name: &JsString, variables: &IndexMap<JsString, VarId>) {
        if let Some(&var) = variables.get(name) {
            self.depends.insert(var);
        } else {
            self.unknown_dependencies = true;
        }
    }

    pub(crate) fn unknown_dependencies_flag(&self) -> bool {
        self.unknown_dependencies
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
/// Rust-only (performance divergence): the map is shared copy-on-write (`Rc::make_mut` before a
/// change), so the lattice elements DataFlowAnalysis copies (Java shares references to them) cost
/// no map copy. The map and its definitions are shared with `Rc`, not `Arc`: one thread runs an
/// analysis, and each copy of a map counts every definition in it (with `Arc`, a locked
/// instruction each). What workers share is `PreparedMustDef`, which holds no lattice.
#[derive(Clone, Default)]
pub struct MustDef {
    pub reaching_def: Rc<IndexMap<VarId, Option<Rc<Definition>>>>,
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
        let reaching_def = Rc::make_mut(&mut result.reaching_def);
        for var in vars {
            reaching_def.insert(
                var,
                Some(Rc::new(Definition::new(
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
        Rc::ptr_eq(&self.reaching_def, &other.reaching_def)
            || self.reaching_def.len() == other.reaching_def.len()
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
    fn merge_var_def(&mut self, var: VarId, def: &Option<Rc<Definition>>) {
        Self::merge_var_def_into(&mut self.result, var, def);
    }

    fn merge_var_def_into(result: &mut MustDef, var: VarId, def: &Option<Rc<Definition>>) {
        let result_def = match result.reaching_def.get(&var) {
            None => def.clone(),
            Some(existing) if existing.is_none() || existing == def => return,
            Some(_) => None,
        };
        Rc::make_mut(&mut result.reaching_def).insert(var, result_def);
    }

    fn join_into(result: &mut MustDef, input: &MustDef) {
        if result.reaching_def.is_empty() {
            *result = input.clone();
            return;
        }
        if Rc::ptr_eq(&result.reaching_def, &input.reaching_def) {
            return;
        }
        for (&var, def) in input.reaching_def.iter() {
            Self::merge_var_def_into(result, var, def);
        }
    }
}
impl FlowJoiner<MustDef> for MustDefJoin {
    // port: MustBeReachingVariableDef.MustDefJoin#joinFlow
    fn join_flow(&mut self, _compiler: &mut AbstractCompiler, input: MustDef) {
        // Rust-only shortcut: merging into an empty result inserts every entry of the input in
        // order, which gives the input itself.
        if self.result.reaching_def.is_empty() {
            self.result = input;
            return;
        }
        if Rc::ptr_eq(&self.result.reaching_def, &input.reaching_def) {
            return;
        }
        for (var, def) in input.reaching_def.iter() {
            self.merge_var_def(*var, def);
        }
    }
    // port: MustBeReachingVariableDef.MustDefJoin#finish
    fn finish(self: Box<Self>) -> MustDef {
        self.result
    }
}

/// Rust-only (performance divergence): precomputed per-function metadata for read-only
/// `MustBeReachingVariableDef` execution on `&Ast` (including worker threads with zero
/// cross-thread frees).
pub(crate) struct PreparedMustDef {
    escaped_var_ids: IndexSet<VarId>,
    param_var_ids: IndexSet<VarId>,
    all_vars_in_fn: IndexMap<JsString, VarId>,
    /// The definitions of the entry lattice (`MustDef#MustDef(Collection)`), from which each run
    /// builds its own lattice.
    entry_definitions: Vec<(VarId, Definition)>,
}

pub(crate) struct BorrowedMustDef<'a> {
    state: DataFlowAnalysisState<NodeId>,
    prepared: &'a PreparedMustDef,
}

impl<'a> BorrowedMustDef<'a> {
    pub(crate) fn cfg(&self) -> &ControlFlowGraph<NodeId> {
        &self.state.cfg
    }

    pub(crate) fn get_def(&self, name: &JsString, use_node: NodeId) -> Option<Rc<Definition>> {
        let n = self.state.cfg.get_node(&Some(use_node))?;
        let state = self.state.node_flow::<MustDef>(n);
        self.prepared
            .all_vars_in_fn
            .get(name)
            .and_then(|var| state.get_in().reaching_def.get(var))
            .cloned()
            .flatten()
    }

    pub(crate) fn into_cfg(self) -> ControlFlowGraph<NodeId> {
        self.state.into_cfg()
    }
}

impl PreparedMustDef {
    pub(crate) fn new(
        compiler: &AbstractCompiler,
        escaped: &IndexSet<VarId>,
        all_vars_in_fn: IndexMap<JsString, VarId>,
    ) -> Self {
        let mut escaped_var_ids = IndexSet::default();
        let mut param_var_ids = IndexSet::default();
        let escaped_keys: Option<IndexSet<(JsString, NodeId)>> = if escaped.len() > 4 {
            let mut set = IndexSet::default();
            set.reserve(escaped.len());
            for &e in escaped {
                set.insert((e.get_name(compiler), e.get_scope_root(compiler)));
            }
            Some(set)
        } else {
            None
        };
        for &var in all_vars_in_fn.values() {
            let is_esc = if escaped.is_empty() {
                false
            } else if escaped.contains(&var) {
                true
            } else if let Some(keys) = &escaped_keys {
                keys.contains(&(var.get_name(compiler), var.get_scope_root(compiler)))
            } else {
                escaped.iter().any(|e| e.equals(compiler, var))
            };
            if is_esc {
                escaped_var_ids.insert(var);
            }
            if var.is_param(compiler) {
                param_var_ids.insert(var);
            }
        }
        let entry_definitions = all_vars_in_fn
            .values()
            .map(|&var| {
                (
                    var,
                    Definition::new(var.get_scope(compiler).get_root_node(compiler)),
                )
            })
            .collect();
        Self {
            escaped_var_ids,
            param_var_ids,
            all_vars_in_fn,
            entry_definitions,
        }
    }

    /// The entry lattice of a run: `MustDef::new_with_vars` of the variables of the function.
    fn entry_lattice(&self) -> MustDef {
        let mut result = MustDef::new();
        let reaching_def = Rc::make_mut(&mut result.reaching_def);
        for (var, definition) in &self.entry_definitions {
            reaching_def.insert(*var, Some(Rc::new(definition.clone())));
        }
        result
    }

    pub(crate) fn escaped_var_ids(&self) -> &IndexSet<VarId> {
        &self.escaped_var_ids
    }

    pub(crate) fn all_vars(&self) -> &IndexMap<JsString, VarId> {
        &self.all_vars_in_fn
    }

    pub(crate) fn run_borrowed<'a>(
        &'a self,
        ast: &Ast,
        cfg: ControlFlowGraph<NodeId>,
    ) -> BorrowedMustDef<'a> {
        let mut state = DataFlowAnalysisState::new(cfg, true, false);
        let entry = &self.entry_lattice();
        state.run_indexed_readonly(
            ast,
            true,
            MustDef::new,
            || entry.clone(),
            MustDefJoin::join_into,
            |_cfg, _cfg_node, n, mut output| {
                MustBeReachingVariableDef::compute_must_def_ast(
                    ast,
                    n,
                    n,
                    &mut output,
                    false,
                    &self.all_vars_in_fn,
                    &self.escaped_var_ids,
                    &self.param_var_ids,
                );
                output
            },
        );
        BorrowedMustDef {
            state,
            prepared: self,
        }
    }
}

pub struct MustBeReachingVariableDef {
    state: DataFlowAnalysisState<NodeId>,
    escaped: IndexSet<VarId>,
    escaped_var_ids: IndexSet<VarId>,
    param_var_ids: IndexSet<VarId>,
    all_vars_in_fn: IndexMap<JsString, VarId>,
    entry_lattice: MustDef,
}
impl MustBeReachingVariableDef {
    // port: MustBeReachingVariableDef#MustBeReachingVariableDef
    pub fn new(
        compiler: &AbstractCompiler,
        cfg: ControlFlowGraph<NodeId>,
        escaped: IndexSet<VarId>,
        all_vars_in_fn: IndexMap<JsString, VarId>,
    ) -> Self {
        let prepared = PreparedMustDef::new(compiler, &escaped, all_vars_in_fn);
        let entry_lattice = prepared.entry_lattice();
        Self {
            state: DataFlowAnalysisState::new(cfg, true, false),
            escaped,
            escaped_var_ids: prepared.escaped_var_ids,
            param_var_ids: prepared.param_var_ids,
            all_vars_in_fn: prepared.all_vars_in_fn,
            entry_lattice,
        }
    }
    pub fn into_cfg(self) -> ControlFlowGraph<NodeId> {
        self.state.into_cfg()
    }
    pub(crate) fn into_use_analysis(
        self,
    ) -> crate::maybe_reaching_variable_use::MaybeReachingVariableUse {
        crate::maybe_reaching_variable_use::MaybeReachingVariableUse::new_with_escaped_ids(
            self.state.into_cfg(),
            self.escaped,
            self.escaped_var_ids,
            self.all_vars_in_fn,
        )
    }
    pub(crate) fn analyze_readonly(&mut self, ast: &Ast) {
        let entry = &self.entry_lattice;
        let all_vars_in_fn = &self.all_vars_in_fn;
        let escaped_var_ids = &self.escaped_var_ids;
        let param_var_ids = &self.param_var_ids;
        self.state.run_indexed_readonly(
            ast,
            true,
            MustDef::new,
            || entry.clone(),
            MustDefJoin::join_into,
            |_cfg, _cfg_node, n, mut output| {
                Self::compute_must_def_ast(
                    ast,
                    n,
                    n,
                    &mut output,
                    false,
                    all_vars_in_fn,
                    escaped_var_ids,
                    param_var_ids,
                );
                output
            },
        );
    }
    // port: MustBeReachingVariableDef#computeMustDef
    #[allow(clippy::too_many_arguments)]
    fn compute_must_def_ast(
        ast: &Ast,
        n: NodeId,
        cfg_node: NodeId,
        output: &mut MustDef,
        conditional: bool,
        all_vars_in_fn: &IndexMap<JsString, VarId>,
        escaped_var_ids: &IndexSet<VarId>,
        param_var_ids: &IndexSet<VarId>,
    ) {
        match n.get_token(ast) {
            Token::BLOCK | Token::ROOT | Token::FUNCTION => {}
            Token::WHILE | Token::DO | Token::IF | Token::FOR => Self::compute_must_def_ast(
                ast,
                NodeUtil::get_condition_expression(ast, n).unwrap(),
                cfg_node,
                output,
                conditional,
                all_vars_in_fn,
                escaped_var_ids,
                param_var_ids,
            ),
            Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                let mut lhs = n.get_first_child(ast).unwrap();
                let rhs = lhs.get_next(ast).unwrap();
                if NodeUtil::is_name_declaration(ast, Some(lhs)) {
                    lhs = lhs.get_last_child(ast).unwrap();
                }
                if lhs.is_name(ast) {
                    Self::add_to_def_if_local_ast(
                        ast,
                        lhs.get_string_ref(ast),
                        None,
                        Some(rhs),
                        output,
                        all_vars_in_fn,
                        escaped_var_ids,
                    );
                } else if lhs.is_destructuring_lhs(ast) {
                    lhs = lhs.get_first_child(ast).unwrap();
                }
                if lhs.is_destructuring_pattern(ast) {
                    Self::compute_must_def_ast(
                        ast,
                        lhs,
                        cfg_node,
                        output,
                        true,
                        all_vars_in_fn,
                        escaped_var_ids,
                        param_var_ids,
                    );
                }
            }
            Token::OPTCHAIN_GETPROP => Self::compute_must_def_ast(
                ast,
                n.get_first_child(ast).unwrap(),
                cfg_node,
                output,
                conditional,
                all_vars_in_fn,
                escaped_var_ids,
                param_var_ids,
            ),
            Token::AND | Token::OR | Token::COALESCE | Token::OPTCHAIN_GETELEM => {
                Self::compute_must_def_ast(
                    ast,
                    n.get_first_child(ast).unwrap(),
                    cfg_node,
                    output,
                    conditional,
                    all_vars_in_fn,
                    escaped_var_ids,
                    param_var_ids,
                );
                Self::compute_must_def_ast(
                    ast,
                    n.get_last_child(ast).unwrap(),
                    cfg_node,
                    output,
                    true,
                    all_vars_in_fn,
                    escaped_var_ids,
                    param_var_ids,
                );
            }
            Token::OPTCHAIN_CALL => {
                Self::compute_must_def_ast(
                    ast,
                    n.get_first_child(ast).unwrap(),
                    cfg_node,
                    output,
                    conditional,
                    all_vars_in_fn,
                    escaped_var_ids,
                    param_var_ids,
                );
                let mut c = n.get_second_child(ast);
                while let Some(child) = c {
                    Self::compute_must_def_ast(
                        ast,
                        child,
                        cfg_node,
                        output,
                        true,
                        all_vars_in_fn,
                        escaped_var_ids,
                        param_var_ids,
                    );
                    c = child.get_next(ast);
                }
            }
            Token::HOOK => {
                Self::compute_must_def_ast(
                    ast,
                    n.get_first_child(ast).unwrap(),
                    cfg_node,
                    output,
                    conditional,
                    all_vars_in_fn,
                    escaped_var_ids,
                    param_var_ids,
                );
                Self::compute_must_def_ast(
                    ast,
                    n.get_second_child(ast).unwrap(),
                    cfg_node,
                    output,
                    true,
                    all_vars_in_fn,
                    escaped_var_ids,
                    param_var_ids,
                );
                Self::compute_must_def_ast(
                    ast,
                    n.get_last_child(ast).unwrap(),
                    cfg_node,
                    output,
                    true,
                    all_vars_in_fn,
                    escaped_var_ids,
                    param_var_ids,
                );
            }
            Token::LET | Token::CONST | Token::VAR => {
                let mut c = n.get_first_child(ast);
                while let Some(child) = c {
                    if child.has_children(ast) {
                        if child.is_name(ast) {
                            Self::compute_must_def_ast(
                                ast,
                                child.get_first_child(ast).unwrap(),
                                cfg_node,
                                output,
                                conditional,
                                all_vars_in_fn,
                                escaped_var_ids,
                                param_var_ids,
                            );
                            Self::add_to_def_if_local_ast(
                                ast,
                                child.get_string_ref(ast),
                                if conditional { None } else { Some(cfg_node) },
                                child.get_first_child(ast),
                                output,
                                all_vars_in_fn,
                                escaped_var_ids,
                            );
                        } else {
                            check_state!(
                                child.is_destructuring_lhs(ast),
                                "%s",
                                child.to_string(ast)
                            );
                            Self::compute_must_def_ast(
                                ast,
                                child.get_second_child(ast).unwrap(),
                                cfg_node,
                                output,
                                conditional,
                                all_vars_in_fn,
                                escaped_var_ids,
                                param_var_ids,
                            );
                            Self::compute_must_def_ast(
                                ast,
                                child.get_first_child(ast).unwrap(),
                                cfg_node,
                                output,
                                conditional,
                                all_vars_in_fn,
                                escaped_var_ids,
                                param_var_ids,
                            );
                        }
                    }
                    c = child.get_next(ast);
                }
            }
            Token::DEFAULT_VALUE => {
                let first = n.get_first_child(ast).unwrap();
                let second = n.get_second_child(ast).unwrap();
                if first.is_destructuring_pattern(ast) {
                    Self::compute_must_def_ast(
                        ast,
                        second,
                        cfg_node,
                        output,
                        true,
                        all_vars_in_fn,
                        escaped_var_ids,
                        param_var_ids,
                    );
                    Self::compute_must_def_ast(
                        ast,
                        first,
                        cfg_node,
                        output,
                        conditional,
                        all_vars_in_fn,
                        escaped_var_ids,
                        param_var_ids,
                    );
                } else if first.is_name(ast) {
                    Self::compute_must_def_ast(
                        ast,
                        second,
                        cfg_node,
                        output,
                        true,
                        all_vars_in_fn,
                        escaped_var_ids,
                        param_var_ids,
                    );
                    Self::add_to_def_if_local_ast(
                        ast,
                        first.get_string_ref(ast),
                        if conditional { None } else { Some(cfg_node) },
                        None,
                        output,
                        all_vars_in_fn,
                        escaped_var_ids,
                    );
                } else {
                    Self::compute_must_def_ast(
                        ast,
                        first,
                        cfg_node,
                        output,
                        conditional,
                        all_vars_in_fn,
                        escaped_var_ids,
                        param_var_ids,
                    );
                    Self::compute_must_def_ast(
                        ast,
                        second,
                        cfg_node,
                        output,
                        true,
                        all_vars_in_fn,
                        escaped_var_ids,
                        param_var_ids,
                    );
                }
            }
            Token::NAME => {
                if NodeUtil::is_lhs_by_destructuring(ast, n) {
                    Self::add_to_def_if_local_ast(
                        ast,
                        n.get_string_ref(ast),
                        if conditional { None } else { Some(cfg_node) },
                        None,
                        output,
                        all_vars_in_fn,
                        escaped_var_ids,
                    );
                } else if n.get_string_ref(ast) == "arguments" {
                    Self::escape_parameters_ast(output, param_var_ids);
                }
            }
            _ => {
                if NodeUtil::is_assignment_op(ast, n) {
                    let lhs = n.get_first_child(ast).unwrap();
                    if lhs.is_name(ast) {
                        Self::compute_must_def_ast(
                            ast,
                            lhs.get_next(ast).unwrap(),
                            cfg_node,
                            output,
                            conditional,
                            all_vars_in_fn,
                            escaped_var_ids,
                            param_var_ids,
                        );
                        Self::add_to_def_if_local_ast(
                            ast,
                            lhs.get_string_ref(ast),
                            if conditional { None } else { Some(cfg_node) },
                            n.get_last_child(ast),
                            output,
                            all_vars_in_fn,
                            escaped_var_ids,
                        );
                        return;
                    } else if NodeUtil::is_normal_get(ast, lhs) {
                        let obj = n.get_first_first_child(ast).unwrap();
                        if obj.is_name(ast) && obj.get_string_ref(ast) == "arguments" {
                            Self::escape_parameters_ast(output, param_var_ids);
                        }
                    } else if lhs.is_destructuring_pattern(ast) {
                        Self::compute_must_def_ast(
                            ast,
                            n.get_second_child(ast).unwrap(),
                            cfg_node,
                            output,
                            conditional,
                            all_vars_in_fn,
                            escaped_var_ids,
                            param_var_ids,
                        );
                        Self::compute_must_def_ast(
                            ast,
                            lhs,
                            cfg_node,
                            output,
                            conditional,
                            all_vars_in_fn,
                            escaped_var_ids,
                            param_var_ids,
                        );
                        return;
                    }
                }
                if n.is_dec(ast) || n.is_inc(ast) {
                    let target = n.get_first_child(ast).unwrap();
                    if target.is_name(ast) {
                        Self::add_to_def_if_local_ast(
                            ast,
                            target.get_string_ref(ast),
                            if conditional { None } else { Some(cfg_node) },
                            None,
                            output,
                            all_vars_in_fn,
                            escaped_var_ids,
                        );
                        return;
                    }
                }
                let mut c = n.get_first_child(ast);
                while let Some(child) = c {
                    Self::compute_must_def_ast(
                        ast,
                        child,
                        cfg_node,
                        output,
                        conditional,
                        all_vars_in_fn,
                        escaped_var_ids,
                        param_var_ids,
                    );
                    c = child.get_next(ast);
                }
            }
        }
    }
    // port: MustBeReachingVariableDef#addToDefIfLocal
    fn add_to_def_if_local_ast(
        ast: &Ast,
        name: &JsString,
        node: Option<NodeId>,
        r_value: Option<NodeId>,
        def: &mut MustDef,
        all_vars_in_fn: &IndexMap<JsString, VarId>,
        escaped_var_ids: &IndexSet<VarId>,
    ) {
        let Some(&var) = all_vars_in_fn.get(name) else {
            return;
        };
        let depends_on_var = |other: &Option<Rc<Definition>>| {
            other.as_ref().is_some_and(|d| d.depends.contains(&var))
        };
        if def.reaching_def.values().any(depends_on_var) {
            for other in Rc::make_mut(&mut def.reaching_def).values_mut() {
                if depends_on_var(other) {
                    *other = None;
                }
            }
        }
        if !escaped_var_ids.contains(&var) {
            if let Some(node) = node {
                let mut definition = Definition::new(node);
                if let Some(r_value) = r_value {
                    Self::compute_dependence_ast(ast, &mut definition, r_value, all_vars_in_fn);
                }
                Rc::make_mut(&mut def.reaching_def).insert(var, Some(Rc::new(definition)));
            } else {
                Rc::make_mut(&mut def.reaching_def).insert(var, None);
            }
        }
    }
    // port: MustBeReachingVariableDef#escapeParameters
    fn escape_parameters_ast(output: &mut MustDef, param_var_ids: &IndexSet<VarId>) {
        if param_var_ids.is_empty() {
            return;
        }
        for &v in param_var_ids {
            Rc::make_mut(&mut output.reaching_def).insert(v, None);
        }
        let depends_on_parameter = |value: &Option<Rc<Definition>>| {
            value
                .as_ref()
                .is_some_and(|d| d.depends.iter().any(|dep| param_var_ids.contains(dep)))
        };
        if output.reaching_def.values().any(depends_on_parameter) {
            for value in Rc::make_mut(&mut output.reaching_def).values_mut() {
                if depends_on_parameter(value) {
                    *value = None;
                }
            }
        }
    }
    // port: MustBeReachingVariableDef#computeDependence
    fn compute_dependence_ast(
        ast: &Ast,
        def: &mut Definition,
        r_value: NodeId,
        all_vars_in_fn: &IndexMap<JsString, VarId>,
    ) {
        // A dotted access stores the property spelling on the node. Only its receiver
        // can reference a variable, and primitive literal leaves have no dependencies.
        let mut receiver = r_value;
        while matches!(
            receiver.get_token(ast),
            Token::GETPROP | Token::OPTCHAIN_GETPROP
        ) && receiver.has_one_child(ast)
        {
            receiver = receiver.get_first_child(ast).unwrap();
        }
        if !receiver.has_children(ast) {
            match receiver.get_token(ast) {
                Token::NAME => {
                    if let Some(name) = receiver.string_payload(ast) {
                        def.record_dependency(name, all_vars_in_fn);
                        return;
                    }
                }
                Token::NUMBER
                | Token::BIGINT
                | Token::STRINGLIT
                | Token::TRUE
                | Token::FALSE
                | Token::NULL
                | Token::THIS => return,
                _ => {}
            }
        }

        // Rust-only (performance divergence): walk CFG node directly on `&Ast` instead of
        // allocating a `NodeTraversal` per non-trivial assignment.
        // port: MustBeReachingVariableDef#computeDependence.visit
        fn walk(
            ast: &Ast,
            n: NodeId,
            is_root: bool,
            def: &mut Definition,
            all_vars_in_fn: &IndexMap<JsString, VarId>,
        ) {
            if !is_root && ControlFlowGraph::is_entering_new_cfg_node(ast, n) {
                return;
            }
            if n.is_name(ast) {
                def.record_dependency(n.get_string_ref(ast), all_vars_in_fn);
            }
            let mut c = n.get_first_child(ast);
            while let Some(child) = c {
                walk(ast, child, false, def, all_vars_in_fn);
                c = child.get_next(ast);
            }
        }
        walk(ast, r_value, true, def, all_vars_in_fn);
    }
    // port: MustBeReachingVariableDef#getDef
    pub fn get_def(&self, name: &JsString, use_node: NodeId) -> Option<Rc<Definition>> {
        check_argument!(self.get_cfg().has_node(&Some(use_node)));
        let n = self.get_cfg().get_node(&Some(use_node)).unwrap();
        let state = self.flow_state(n);
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
    fn analyze_indexed(&mut self, compiler: &mut AbstractCompiler) {
        self.analyze_readonly(&compiler.ast);
    }
    // port: MustBeReachingVariableDef#isForward
    fn is_forward(&self) -> bool {
        true
    }
    // port: MustBeReachingVariableDef#createEntryLattice
    fn create_entry_lattice(&mut self, _compiler: &mut AbstractCompiler) -> MustDef {
        self.entry_lattice.clone()
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
        // Java copies the shared input; here the input is owned (performance divergence).
        let mut output = input;
        Self::compute_must_def_ast(
            &compiler.ast,
            n,
            n,
            &mut output,
            false,
            &self.all_vars_in_fn,
            &self.escaped_var_ids,
            &self.param_var_ids,
        );
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

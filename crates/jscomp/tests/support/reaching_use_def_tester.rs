/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/ReachingUseDefTester.java.

use closure_jscomp::{
    Compiler,
    abstract_compiler::LifeCycleStage,
    compiler_options::{CompilerOptions, LanguageMode},
    control_flow_analysis::ControlFlowAnalysis,
    control_flow_graph::ControlFlowGraph,
    data_flow_analysis::{DataFlowAnalysis, compute_escaped},
    google_coding_convention::GoogleCodingConvention,
    maybe_reaching_variable_use::MaybeReachingVariableUse,
    must_be_reaching_variable_def::MustBeReachingVariableDef,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    scope::ScopeId,
    syntactic_scope_creator::SyntacticScopeCreator,
};
use closure_rhino::fx_hash::IndexSet;
use closure_rhino::node::NodeId;
use std::sync::Arc;
pub struct ReachingUseDefTester {
    compiler: Compiler,
    scope_creator: SyntacticScopeCreator<'static>,
    label_finder: LabelFinder,
    root: Option<NodeId>,
    reaching_use: Option<MaybeReachingVariableUse>,
    reaching_def: Option<MustBeReachingVariableDef>,
}
impl ReachingUseDefTester {
    // port: ReachingUseDefTester#ReachingUseDefTester
    fn new(
        compiler: Compiler,
        scope_creator: SyntacticScopeCreator<'static>,
        label_finder: LabelFinder,
    ) -> Self {
        Self {
            compiler,
            scope_creator,
            label_finder,
            root: None,
            reaching_use: None,
            reaching_def: None,
        }
    }
    // port: ReachingUseDefTester#create
    pub fn create() -> Self {
        let compiler = Self::create_compiler();
        Self::new(compiler, SyntacticScopeCreator::new(), LabelFinder::new())
    }
    // port: ReachingUseDefTester#createCompiler
    fn create_compiler() -> Compiler {
        let mut compiler = Compiler::new();
        compiler.set_life_cycle_stage(LifeCycleStage::NORMALIZED);
        let mut options = CompilerOptions::new();
        options.set_coding_convention(Arc::new(GoogleCodingConvention::new()));
        options.set_language(LanguageMode::UNSUPPORTED);
        compiler.init_options(options);
        compiler
    }
    // port: ReachingUseDefTester#computeReachingUses
    pub fn compute_reaching_uses(&mut self, src: &str, is_async: bool) {
        let script = self.parse_script(src, is_async);
        self.root = script.get_first_child(&self.compiler);
        let root = self.root.unwrap();
        let scope = self.compute_function_block_scope(script, root);
        let cfg = self.compute_cfg(root);
        let mut escaped = IndexSet::<_>::default();
        let parent = scope.get_parent(&self.compiler).unwrap();
        let all_vars = NodeUtil::get_all_vars_declared_in_function(
            &mut self.compiler,
            &mut self.scope_creator,
            parent,
        );
        let vars = all_vars.get_all_variables().clone();
        compute_escaped(
            &mut self.compiler,
            parent,
            &mut escaped,
            &mut self.scope_creator,
            &vars,
        );
        let mut reaching_use = MaybeReachingVariableUse::new(cfg, escaped, vars);
        reaching_use.analyze(&mut self.compiler);
        self.reaching_use = Some(reaching_use);
    }
    // port: ReachingUseDefTester#computeReachingDef
    pub fn compute_reaching_def(&mut self, src: &str) {
        let script = self.parse_script(src, false);
        self.root = script.get_first_child(&self.compiler);
        let root = self.root.unwrap();
        let scope = self.compute_function_block_scope(script, root);
        let cfg = self.compute_cfg(root);
        let mut escaped = IndexSet::<_>::default();
        let parent = scope.get_parent(&self.compiler).unwrap();
        let all_vars = NodeUtil::get_all_vars_declared_in_function(
            &mut self.compiler,
            &mut self.scope_creator,
            parent,
        );
        let vars = all_vars.get_all_variables().clone();
        compute_escaped(
            &mut self.compiler,
            parent,
            &mut escaped,
            &mut self.scope_creator,
            &vars,
        );
        let mut reaching_def = MustBeReachingVariableDef::new(&self.compiler, cfg, escaped, vars);
        reaching_def.analyze(&mut self.compiler);
        self.reaching_def = Some(reaching_def);
    }
    // port: ReachingUseDefTester#parseScript
    fn parse_script(&mut self, src: &str, is_async: bool) -> NodeId {
        let src = format!(
            "{}function _FUNCTION(param1, param2){{{src}}}",
            if is_async { "async " } else { "" }
        );
        let script = self.compiler.parse_test_code(src.as_str());
        assert!(self.compiler.get_errors().is_empty());
        script
    }
    // port: ReachingUseDefTester#computeFunctionBlockScope
    fn compute_function_block_scope(&mut self, script: NodeId, function: NodeId) -> ScopeId {
        let block = function.get_last_child(&self.compiler).unwrap();
        let global = self
            .scope_creator
            .create_scope(&mut self.compiler, script, None);
        let scope =
            self.scope_creator
                .create_scope(&mut self.compiler, self.root.unwrap(), Some(global));
        self.scope_creator
            .create_scope(&mut self.compiler, block, Some(scope))
    }
    // port: ReachingUseDefTester#computeCfg
    fn compute_cfg(&mut self, function: NodeId) -> ControlFlowGraph<NodeId> {
        ControlFlowAnalysis::builder()
            .set_cfg_root(function)
            .set_include_edge_annotations(true)
            .compute_cfg(&mut self.compiler)
    }
    // port: ReachingUseDefTester#getComputedUses
    pub fn get_computed_uses(&self) -> IndexSet<NodeId> {
        self.reaching_use
            .as_ref()
            .unwrap()
            .get_uses(&"x".into(), self.label_finder.extracted_def.unwrap())
            .into_iter()
            .collect()
    }
    // port: ReachingUseDefTester#getComputedDef
    pub fn get_computed_def(&self) -> Option<NodeId> {
        self.reaching_def
            .as_ref()
            .unwrap()
            .get_def_node(&"x".into(), self.label_finder.extracted_uses[0])
    }
    // port: ReachingUseDefTester#extractDefAndUsesFromInputLabels
    pub fn extract_def_and_uses_from_input_labels(&mut self) {
        NodeTraversal::builder()
            .set_compiler(&mut self.compiler)
            .set_scope_creator(&mut self.scope_creator)
            .set_callback(&mut self.label_finder)
            .traverse(self.root.unwrap());
        assert!(
            self.label_finder.extracted_def.is_some(),
            "Code should have an instruction labeled D"
        );
        assert!(
            !self.label_finder.extracted_uses.is_empty(),
            "Code should have an instruction label starting withing U"
        );
    }
    // port: ReachingUseDefTester#getExtractedUses
    pub fn get_extracted_uses(&self) -> &[NodeId] {
        &self.label_finder.extracted_uses
    }
    // port: ReachingUseDefTester#getExtractedDef
    pub fn get_extracted_def(&self) -> NodeId {
        self.label_finder.extracted_def.unwrap()
    }
}
struct LabelFinder {
    extracted_def: Option<NodeId>,
    extracted_uses: Vec<NodeId>,
}
impl LabelFinder {
    // port: ReachingUseDefTester.LabelFinder#LabelFinder
    fn new() -> Self {
        Self {
            extracted_def: None,
            extracted_uses: vec![],
        }
    }
}
impl Callback for LabelFinder {
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }
    // port: ReachingUseDefTester.LabelFinder#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_label(t) {
            let label = n.get_first_child(t).unwrap().get_string(t);
            if label == "D" {
                assert!(
                    self.extracted_def.is_none(),
                    "Multiple D: labels in test src"
                );
                self.extracted_def = n.get_last_child(t);
            } else if label.starts_with("U") {
                self.extracted_uses.push(n.get_last_child(t).unwrap());
            }
        }
    }
}

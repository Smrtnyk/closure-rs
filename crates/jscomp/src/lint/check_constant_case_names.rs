/*
 * Copyright 2019 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/lint/CheckConstantCaseNames.java.

//! Checks that module-level CONSTANT_CASE names are declared `const` (or @const) and are not
//! reassigned.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{js_string::JsString, node::NodeId, token::Token};

// port: CheckConstantCaseNames#MISSING_CONST_PROPERTY
pub static MISSING_CONST_PROPERTY: DiagnosticType = DiagnosticType::disabled(
    "JSC_MISSING_CONST_ON_CONSTANT_CASE",
    "CONSTANT_CASE name \"{0}\" is constant-by-convention, so must be explicitly `const` or @const",
);

// port: CheckConstantCaseNames#REASSIGNED_CONSTANT_CASE_NAME
pub static REASSIGNED_CONSTANT_CASE_NAME: DiagnosticType = DiagnosticType::disabled(
    "JSC_REASSIGNED_CONSTANT_CASE_NAME",
    "CONSTANT_CASE name \"{0}\" is constant-by-convention but is reassigned. Use camelCase instead.",
);

pub struct CheckConstantCaseNames {
    /// Maps CONSTANT_CASE module-level names to their initializing NAME node
    invalid_names_per_module: IndexMap<JsString, NodeId>,
    /// Subset of variables in `invalidNamesPerModule` that are mutated post-declaration.
    reassigned_names: IndexSet<JsString>,
}

impl CheckConstantCaseNames {
    // port: CheckConstantCaseNames#CheckConstantCaseNames
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        // Java keeps `convention = compiler.getCodingConvention()`; the convention is read from
        // the compiler at each use instead (DESIGN 6: a pass never stores the compiler).
        Self {
            invalid_names_per_module: IndexMap::<_, _>::default(),
            reassigned_names: IndexSet::<_>::default(),
        }
    }

    // port: CheckConstantCaseNames#reportWarningsAndClear
    fn report_warnings_and_clear(&mut self, compiler: &mut AbstractCompiler) {
        for &name_node in self.invalid_names_per_module.values() {
            let name = name_node.get_string(compiler);
            if self.reassigned_names.contains(&name) {
                compiler.report(JSError::make(
                    compiler,
                    name_node,
                    &REASSIGNED_CONSTANT_CASE_NAME,
                    &[&name.to_string()],
                ));
            } else {
                compiler.report(JSError::make(
                    compiler,
                    name_node,
                    &MISSING_CONST_PROPERTY,
                    &[&name.to_string()],
                ));
            }
        }
        self.invalid_names_per_module = IndexMap::<_, _>::default();
        self.reassigned_names = IndexSet::<_>::default();
    }
}

impl CompilerPass for CheckConstantCaseNames {
    // port: CheckConstantCaseNames#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckConstantCaseNames {
    // port: CheckConstantCaseNames#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        // Only need to warn for module-level names, so don't visit other files.
        if n.is_script(t) {
            return n.has_children(t) && n.get_first_child(t).unwrap().is_module_body(t);
        }
        true
    }

    // port: CheckConstantCaseNames#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_module_body(t) {
            self.report_warnings_and_clear(t.get_compiler());
            return;
        }
        match n.get_token(t) {
            Token::VAR | Token::LET => {
                // Skip CONST as it automatically meets the criteria and only look for
                // module-level vars.
                if !t.in_module_scope() {
                    return;
                }
                let info = n.get_jsdoc_info(t);
                if info.is_some_and(|info| info.has_const_annotation()) {
                    return;
                }
                let compiler = t.get_compiler();
                let mut names: Vec<NodeId> = Vec::new();
                NodeUtil::visit_lhs_nodes_in_node(
                    compiler,
                    n,
                    &mut |_: &mut AbstractCompiler, name: NodeId| names.push(name),
                );
                for name in names {
                    let name_string = name.get_string(t);
                    if t.get_compiler()
                        .get_coding_convention()
                        .is_constant(&name_string)
                    {
                        self.invalid_names_per_module.insert(name_string, name);
                    }
                }
            }
            Token::NAME => {
                if !self.invalid_names_per_module.contains_key(&n.get_string(t)) {
                    return;
                }
                if !NodeUtil::is_l_value(t, n) {
                    return;
                }
                // Verify this name is referring to the actual module-level var and not a local
                // shadow.
                let scope = t.get_scope();
                let name = n.get_string(t);
                let v = scope.get_var(t.get_compiler(), &name).unwrap();
                if v.get_scope_root(t.get_compiler()).is_module_body(t) {
                    self.reassigned_names.insert(name);
                }
            }
            _ => {}
        }
    }
}

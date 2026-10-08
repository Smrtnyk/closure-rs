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
//   src/com/google/javascript/jscomp/ForbidDynamicImportUsage.java.

//! Warns at any usage of Dynamic Import expressions that they are unable to be transpiled.
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{
    node::{Ast, NodeId},
    token::Token,
};

// port: ForbidDynamicImportUsage#DYNAMIC_IMPORT_USAGE
pub static DYNAMIC_IMPORT_USAGE: DiagnosticType = DiagnosticType::error(
    "JSC_DYNAMIC_IMPORT_USAGE",
    "Dynamic import expressions cannot be transpiled.",
);

/// Warns at any usage of Dynamic Import expressions that they are unable to be transpiled.
// port: ForbidDynamicImportUsage
pub struct ForbidDynamicImportUsage;

impl ForbidDynamicImportUsage {
    // port: ForbidDynamicImportUsage#ForbidDynamicImportUsage
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self
    }

    // port: ForbidDynamicImportUsage#getFeatureSetOfScript
    fn get_feature_set_of_script(ast: &Ast, n: NodeId) -> FeatureSet {
        let feature_set = NodeUtil::get_feature_set_of_script(ast, n);
        feature_set.unwrap_or(FeatureSet::BARE_MINIMUM)
    }
}

impl CompilerPass for ForbidDynamicImportUsage {
    // port: ForbidDynamicImportUsage#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
        compiler.mark_feature_not_allowed(Feature::DYNAMIC_IMPORT);
    }
}

impl Callback for ForbidDynamicImportUsage {
    // port: ForbidDynamicImportUsage#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_script(t) {
            // We only want to traverse scripts that contain dynamic imports, so we can generate
            // error messages that point to them.
            Self::get_feature_set_of_script(t, n).contains(Feature::DYNAMIC_IMPORT)
        } else {
            true
        }
    }

    // port: ForbidDynamicImportUsage#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        #[allow(clippy::single_match)]
        match n.get_token(t) {
            Token::DYNAMIC_IMPORT => t.report(n, &DYNAMIC_IMPORT_USAGE, &[]),
            _ => {}
        }
    }
}

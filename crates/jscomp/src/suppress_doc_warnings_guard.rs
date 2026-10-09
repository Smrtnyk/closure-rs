/*
 * Copyright 2010 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/SuppressDocWarningsGuard.java.

use crate::{
    abstract_compiler::AbstractCompiler, check_level::CheckLevel,
    diagnostic_group::DiagnosticGroup, diagnostic_groups, js_error::JSError, node_util::NodeUtil,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{jsdoc_info::JSDocInfo, node::NodeId};
use std::sync::Arc;
pub struct SuppressDocWarningsGuard {
    suppressors: IndexMap<String, Arc<DiagnosticGroup>>,
}
impl SuppressDocWarningsGuard {
    // port: SuppressDocWarningsGuard#SuppressDocWarningsGuard
    pub fn new(suppressors: IndexMap<String, Arc<DiagnosticGroup>>) -> Self {
        Self {
            suppressors: Self::create_suppressors(suppressors),
        }
    }
    // port: SuppressDocWarningsGuard#createSuppressors
    fn create_suppressors(
        mut suppressors: IndexMap<String, Arc<DiagnosticGroup>>,
    ) -> IndexMap<String, Arc<DiagnosticGroup>> {
        suppressors.insert(
            "missingProperties".into(),
            Arc::new(DiagnosticGroup::new_from_groups(&[
                diagnostic_groups::MISSING_PROPERTIES.clone(),
                diagnostic_groups::STRICT_MISSING_PROPERTIES.clone(),
            ])),
        );
        suppressors.insert(
            "checkTypes".into(),
            Arc::new(DiagnosticGroup::new_from_groups(&[
                diagnostic_groups::CHECK_TYPES.clone(),
                diagnostic_groups::STRICT_CHECK_TYPES.clone(),
            ])),
        );
        suppressors
    }
    // port: SuppressDocWarningsGuard#level
    pub fn level(&self, compiler: &AbstractCompiler, error: &JSError) -> Option<CheckLevel> {
        let node = error
            .node()
            .or_else(|| Self::get_script_node_by_source_name(compiler, error))?;
        if let Some(level) = self.get_check_level_from_ancestors(compiler, error, node) {
            return Some(level);
        }
        if let Some(script) = Self::get_script_node_by_source_name(compiler, error)
            && let Some(info) = script.get_jsdoc_info(compiler)
        {
            return self.get_check_level_from_info(error, &info);
        }
        None
    }
    // port: SuppressDocWarningsGuard#getCheckLevelFromAncestors
    fn get_check_level_from_ancestors(
        &self,
        compiler: &AbstractCompiler,
        error: &JSError,
        node: NodeId,
    ) -> Option<CheckLevel> {
        let mut current = Some(node);
        while let Some(n) = current {
            let info = if n.is_function(compiler) || n.is_class(compiler) {
                NodeUtil::get_best_jsdoc_info(compiler, n)
            } else if n.is_script(compiler) {
                n.get_jsdoc_info(compiler)
            } else if NodeUtil::is_name_declaration(compiler, Some(n))
                || NodeUtil::may_be_object_lit_key(compiler, n)
                || n.is_computed_prop(compiler)
                || n.is_member_field_def(compiler)
                || n.is_computed_field_def(compiler)
                || n.get_parent(compiler)
                    .is_some_and(|p| p.is_expr_result(compiler))
            {
                NodeUtil::get_best_jsdoc_info(compiler, n)
            } else {
                None
            };
            if let Some(info) = info
                && let Some(level) = self.get_check_level_from_info(error, &info)
            {
                return Some(level);
            }
            current = n.get_parent(compiler);
        }
        None
    }
    // port: SuppressDocWarningsGuard#getCheckLevelFromInfo
    fn get_check_level_from_info(&self, error: &JSError, info: &JSDocInfo) -> Option<CheckLevel> {
        for suppressor in info.get_suppressions() {
            if let Some(group) = self.suppressors.get(&suppressor.to_string_lossy())
                && group.matches(error)
            {
                return Some(CheckLevel::OFF);
            }
        }
        None
    }
    // port: SuppressDocWarningsGuard#getScriptNodeBySourceName
    fn get_script_node_by_source_name(
        compiler: &AbstractCompiler,
        error: &JSError,
    ) -> Option<NodeId> {
        let node = compiler.get_script_node(error.source_name()?)?;
        assert!(node.is_script(compiler));
        Some(node)
    }
    // port: SuppressDocWarningsGuard#getPriority
    pub fn get_priority(&self) -> i32 {
        crate::warnings_guard::Priority::SUPPRESS_DOC.value()
    }
}

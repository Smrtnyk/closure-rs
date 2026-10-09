/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/lint/CheckGoogModuleTypeScriptName.java.

//! Checks that goog.module statement matches the generated TypeScript module namespace, which is
//! based on the file path.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{js_string::JsString, node::NodeId};

// port: CheckGoogModuleTypeScriptName#MODULE_NAMESPACE_MISMATCHES_TYPESCRIPT_NAMESPACE
pub static MODULE_NAMESPACE_MISMATCHES_TYPESCRIPT_NAMESPACE: DiagnosticType =
    DiagnosticType::disabled(
        "JSC_MODULE_NAMESPACE_MISMATCHES_TYPESCRIPT_NAMESPACE",
        "goog.module namespace does not match the future TypeScript namespace, which is generated from the file path. The correct namespace is: \"{0}\"",
    );

// port: CheckGoogModuleTypeScriptName#allowedDirectories
// MOE::begin_strip
const ALLOWED_DIRECTORIES: [&str; 3] = [
    "google3/gws/",
    "google3/java/com/google/gws/",
    "google3/javascript/search/",
];
// MOE::end_strip

pub struct CheckGoogModuleTypeScriptName {
    finished: bool,
}

impl CheckGoogModuleTypeScriptName {
    // port: CheckGoogModuleTypeScriptName#CheckGoogModuleTypeScriptName
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self { finished: false }
    }

    // port: CheckGoogModuleTypeScriptName#checkGoogModuleNamespace
    fn check_goog_module_namespace(t: &mut NodeTraversal<'_>, n: NodeId) {
        let original_namespace = n
            .get_first_child(t)
            .unwrap()
            .get_second_child(t)
            .unwrap()
            .get_string(t);
        let source_name = t.get_source_name();
        let Some(source_name) = source_name else {
            return;
        };
        // Java String operations on UTF-16 code units.
        let mut source_name = JsString::from(source_name);
        // MOE::begin_strip
        let google3_index = source_name.index_of("google3");
        if google3_index == -1 {
            source_name = JsString::from("google3/").concat(&source_name);
        } else if google3_index != 0 {
            source_name = source_name.substring_from(google3_index as usize);
        }
        // MOE::end_strip
        let replaced: Vec<u16> = source_name
            .as_units()
            .iter()
            .map(|&c| {
                if c == u16::from(b'/') {
                    u16::from(b'.')
                } else {
                    c
                }
            })
            .collect();
        let replacement_namespace =
            JsString::from_units(replaced).substring(0, source_name.length() - ".js".len());
        if original_namespace != replacement_namespace {
            for allowed_directory in ALLOWED_DIRECTORIES {
                if source_name.starts_with(JsString::from(allowed_directory)) {
                    t.report(
                        n,
                        &MODULE_NAMESPACE_MISMATCHES_TYPESCRIPT_NAMESPACE,
                        &[&replacement_namespace.to_string()],
                    );
                    return;
                }
            }
        }
    }
}

impl CompilerPass for CheckGoogModuleTypeScriptName {
    // port: CheckGoogModuleTypeScriptName#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckGoogModuleTypeScriptName {
    // port: CheckGoogModuleTypeScriptName#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        _n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        // Traverse top-level statements until a `goog.module` statement is found.
        !self.finished
            && parent.is_none_or(|parent| {
                parent.is_root(t) || parent.is_script(t) || parent.is_module_body(t)
            })
    }

    // port: CheckGoogModuleTypeScriptName#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if NodeUtil::is_goog_module_call(t, n) {
            Self::check_goog_module_namespace(t, n);
            self.finished = true;
        }
    }
}

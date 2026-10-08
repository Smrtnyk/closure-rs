/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/lint/CheckDuplicateCase.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
};
use closure_rhino::{js_string::JsString, node::NodeId};
use indexmap::IndexSet;

// port: CheckDuplicateCase#DUPLICATE_CASE
pub static DUPLICATE_CASE: DiagnosticType = DiagnosticType::warning(
    "JSC_DUPLICATE_CASE",
    "Duplicate case in a switch statement.",
);

/// Check for duplicate case labels in a switch statement Eg: switch (foo) { case 1: case 1: }
///
/// This is normally an indication of a programmer error.
///
/// Inspired by ESLint
/// (https://github.com/eslint/eslint/blob/master/lib/rules/no-duplicate-case.js)
pub struct CheckDuplicateCase;

impl CheckDuplicateCase {
    // port: CheckDuplicateCase#CheckDuplicateCase
    pub fn new() -> Self {
        Self
    }
}

impl Default for CheckDuplicateCase {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for CheckDuplicateCase {
    // port: CheckDuplicateCase#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckDuplicateCase {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckDuplicateCase#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_switch(t) {
            let switch_body = n.get_second_child(t).unwrap();
            let mut cases: IndexSet<JsString> = IndexSet::new();
            let mut curr = switch_body.get_first_child(t);
            while let Some(c) = curr {
                let first = c.get_first_child(t).unwrap();
                let source = t.get_compiler().to_source_for_node_utf16(first);
                if !cases.insert(source) {
                    t.report(c, &DUPLICATE_CASE, &[]);
                }
                curr = c.get_next(t);
            }
        }
    }
}

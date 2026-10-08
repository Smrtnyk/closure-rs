/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/lint/CheckVar.java.

//! Checks when the `var` keyword is used.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
};
use closure_rhino::node::NodeId;

// port: CheckVar#VAR
pub static VAR: DiagnosticType =
    DiagnosticType::disabled("JSC_VAR", "Using `var` (prefer `const` or `let`).");

pub struct CheckVar;

impl CheckVar {
    // port: CheckVar#CheckVar
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self
    }
}

impl CompilerPass for CheckVar {
    // port: CheckVar#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckVar {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckVar#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_var(t) {
            t.report(n, &VAR, &[]);
        }
    }
}

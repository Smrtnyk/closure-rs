/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CheckDebuggerStatement.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
};
use closure_rhino::node::NodeId;

// port: CheckDebuggerStatement#DEBUGGER_STATEMENT_PRESENT
pub static DEBUGGER_STATEMENT_PRESENT: DiagnosticType = DiagnosticType::disabled(
    "JSC_DEBUGGER_STATEMENT_PRESENT",
    "Using the debugger statement can halt your application if the user has a JavaScript debugger running.\nTo disable this check when you want to do debugging, you can suppress this message like this:\n  /** @suppress '{'checkDebuggerStatement'}' */\n  debugger;",
);

/// [`CheckDebuggerStatement`] checks for the presence of the "debugger" statement in JavaScript
/// code. It is appropriate to use this statement while developing JavaScript; however, it is
/// generally undesirable to include it in production code.
pub struct CheckDebuggerStatement;

impl CheckDebuggerStatement {
    // port: CheckDebuggerStatement#CheckDebuggerStatement
    pub fn new() -> Self {
        Self
    }
}

impl Default for CheckDebuggerStatement {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for CheckDebuggerStatement {
    // port: CheckDebuggerStatement#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckDebuggerStatement {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckDebuggerStatement#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_debugger(t) {
            t.report(n, &DEBUGGER_STATEMENT_PRESENT, &[]);
        }
    }
}

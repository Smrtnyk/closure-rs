/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/lint/CheckUnusedLabels.java.

//! Check for unused labels blocks. This can help catching errors like:
//!
//! ```js
//! function f() {
//!   return
//!     a: 2  // Not an object!
//! }
//! ```

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
};
use closure_rhino::{js_string::JsString, node::NodeId, token::Token};

// port: CheckUnusedLabels#UNUSED_LABEL
pub static UNUSED_LABEL: DiagnosticType =
    DiagnosticType::disabled("JSC_UNUSED_LABEL", "Unused label {0}.");

struct LabelContext {
    name: JsString,
    parent: Option<Box<LabelContext>>,
    used: bool,
}

impl LabelContext {
    // port: CheckUnusedLabels.LabelContext#LabelContext
    fn new(name: JsString, parent: Option<Box<LabelContext>>) -> Self {
        Self {
            name,
            parent,
            used: false,
        }
    }
}

pub struct CheckUnusedLabels {
    current_context: Option<Box<LabelContext>>,
}

impl CheckUnusedLabels {
    // port: CheckUnusedLabels#CheckUnusedLabels
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self {
            current_context: None,
        }
    }
}

impl CompilerPass for CheckUnusedLabels {
    // port: CheckUnusedLabels#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckUnusedLabels {
    // port: CheckUnusedLabels#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(t) {
            Token::BREAK | Token::CONTINUE => {
                if n.has_children(t) {
                    let label = n.get_first_child(t).unwrap().get_string(t);
                    let mut temp = self.current_context.as_deref_mut();
                    while let Some(context) = temp {
                        if context.name == label {
                            context.used = true;
                            break;
                        }
                        temp = context.parent.as_deref_mut();
                    }
                }
                return false;
            }
            Token::LABEL => {
                let name = n.get_first_child(t).unwrap().get_string(t);
                self.current_context = Some(Box::new(LabelContext::new(
                    name,
                    self.current_context.take(),
                )));
            }
            _ => {}
        }
        true
    }

    // port: CheckUnusedLabels#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_label(t)
            && let Some(current_context) = self.current_context.take()
        {
            if !current_context.used {
                let name = n.get_first_child(t).unwrap().get_string(t);
                t.report(n, &UNUSED_LABEL, &[&name.to_string()]);
            }
            self.current_context = current_context.parent;
        }
    }
}

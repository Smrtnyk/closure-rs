/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/Es6CheckModule.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{node::NodeId, token::Token};

// port: Es6CheckModule#ES6_MODULE_REFERENCES_THIS
pub static ES6_MODULE_REFERENCES_THIS: DiagnosticType = DiagnosticType::warning(
    "ES6_MODULE_REFERENCES_THIS",
    "The body of an ES6 module cannot reference 'this'.",
);

// port: Es6CheckModule#IMPORT_CANNOT_BE_REASSIGNED
pub static IMPORT_CANNOT_BE_REASSIGNED: DiagnosticType = DiagnosticType::error(
    "JSC_IMPORT_CANNOT_BE_REASSIGNED",
    "Assignment to constant variable \"{0}\".",
);

/// Checks that ES6 modules do not reference `this` in their body and do not reassign imports.
pub struct Es6CheckModule;

impl Es6CheckModule {
    // port: Es6CheckModule#Es6CheckModule
    pub fn new() -> Self {
        Self
    }
}

impl Default for Es6CheckModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for Es6CheckModule {
    // port: Es6CheckModule#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for Es6CheckModule {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: Es6CheckModule#visit
    #[allow(clippy::collapsible_match)] // Retain Java control flow.
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::THIS => {
                if t.in_module_hoist_scope() {
                    t.report(n, &ES6_MODULE_REFERENCES_THIS, &[]);
                }
            }
            Token::GETPROP | Token::GETELEM => {
                if NodeUtil::is_l_value(t, n)
                    && !NodeUtil::is_declaration_l_value(t, n)
                    && n.get_first_child(t).unwrap().is_name(t)
                {
                    let scope = t.get_scope();
                    let name = n.get_first_child(t).unwrap().get_string(t);
                    let var = scope.get_var(t.get_compiler(), &name);
                    if let Some(var) = var {
                        let name_node = var.get_name_node(t.get_compiler());
                        if let Some(name_node) = name_node
                            && name_node.is_import_star(t)
                        {
                            // import * as M from '';
                            // M.x = 2;
                            let name = name_node.get_string(t).to_string();
                            let error =
                                JSError::make(t, n, &IMPORT_CANNOT_BE_REASSIGNED, &[name.as_str()]);
                            t.get_compiler().report(error);
                        }
                    }
                }
            }
            Token::NAME => {
                if NodeUtil::is_l_value(t, n) && !NodeUtil::is_declaration_l_value(t, n) {
                    let scope = t.get_scope();
                    let name = n.get_string(t);
                    let var = scope.get_var(t.get_compiler(), &name);
                    if let Some(var) = var {
                        let name_node = var.get_name_node(t.get_compiler());
                        if let Some(name_node) = name_node
                            && name_node != n
                            && NodeUtil::is_imported_name(t, name_node)
                        {
                            // import { x } from '';
                            // x = 2;
                            let name = name_node.get_string(t).to_string();
                            let error =
                                JSError::make(t, n, &IMPORT_CANNOT_BE_REASSIGNED, &[name.as_str()]);
                            t.get_compiler().report(error);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

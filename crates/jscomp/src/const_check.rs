/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ConstCheck.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `ConstCheck.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use crate::var::VarId;
use closure_rhino::check_not_null;
use closure_rhino::js_string::JsString;
use closure_rhino::node::NodeId;
use closure_rhino::token::Token;
use indexmap::IndexSet;

// port: ConstCheck#CONST_REASSIGNED_VALUE_ERROR
pub static CONST_REASSIGNED_VALUE_ERROR: DiagnosticType = DiagnosticType::warning(
    "JSC_CONSTANT_REASSIGNED_VALUE_ERROR",
    "constant {0} assigned a value more than once.\nOriginal definition at {1}",
);

/// Verifies that constants are only assigned a value once.
///
/// e.g.
///
/// ```text
/// XX = 3;    // error!
/// XX++;      // error!
/// ```
// TODO(tbreisacher): Consider merging this with CheckAccessControls so that all
// const-related checks are in the same place.
pub struct ConstCheck {
    initialized_constants: IndexSet<VarId>,
}

impl ConstCheck {
    /// Creates an instance.
    // port: ConstCheck#ConstCheck
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self {
            initialized_constants: IndexSet::new(),
        }
    }

    /// Gets whether a variable is a constant initialized to a literal value at the point where it
    /// is declared.
    // port: ConstCheck#isConstant
    fn is_constant(compiler: &AbstractCompiler, var: Option<VarId>, name_node: NodeId) -> bool {
        let var = check_not_null!(
            var,
            "Found unexpected undeclared name %s",
            name_node.to_string(compiler)
        );
        var.is_const(compiler) || var.is_declared_or_inferred_const(compiler)
    }

    /// Reports a reassigned constant error.
    // port: ConstCheck#reportError
    fn report_error(compiler: &mut AbstractCompiler, n: NodeId, var: VarId, name: &str) {
        let info = NodeUtil::get_best_jsdoc_info(compiler, n);
        if info.is_none_or(|info| !info.get_suppressions().contains(&JsString::from("const"))) {
            let decl_node = var.get_node(compiler).unwrap();
            let declared_position = format!(
                "{}:{}",
                java_string(decl_node.get_source_file_name(compiler)),
                decl_node.get_lineno(compiler)
            );
            let error = JSError::make(
                compiler,
                n,
                &CONST_REASSIGNED_VALUE_ERROR,
                &[name, &declared_position],
            );
            compiler.report(error);
        }
    }
}

/// Java's string conversion of a nullable String (`"" + s`).
fn java_string(s: Option<String>) -> String {
    s.unwrap_or_else(|| "null".to_string())
}

impl CompilerPass for ConstCheck {
    // port: ConstCheck#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        NodeTraversal::traverse_roots(compiler, self, externs, root);
    }
}

impl Callback for ConstCheck {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ConstCheck#visit
    #[allow(clippy::collapsible_match)] // Retain Java control flow.
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::NAME => {
                if NodeUtil::is_name_declaration(t, parent) {
                    let name = n.get_string(t);
                    let scope = t.get_scope();
                    let var = scope.get_var(t.get_compiler(), name.clone());
                    if Self::is_constant(t.get_compiler(), var, n) {
                        let var = var.unwrap();
                        // If a constant is declared in externs, add it to initializedConstants to
                        // indicate that it is initialized externally.
                        if n.is_from_externs(t) {
                            self.initialized_constants.insert(var);
                        } else if n.has_children(t) && !self.initialized_constants.insert(var) {
                            Self::report_error(t.get_compiler(), n, var, &name.to_string_lossy());
                        }
                    }
                }
            }
            Token::ASSIGN
            | Token::ASSIGN_BITOR
            | Token::ASSIGN_BITXOR
            | Token::ASSIGN_BITAND
            | Token::ASSIGN_LSH
            | Token::ASSIGN_RSH
            | Token::ASSIGN_URSH
            | Token::ASSIGN_ADD
            | Token::ASSIGN_SUB
            | Token::ASSIGN_MUL
            | Token::ASSIGN_DIV
            | Token::ASSIGN_MOD
            | Token::ASSIGN_EXPONENT => {
                let lhs = n.get_first_child(t).unwrap();
                if lhs.is_name(t) {
                    let name = lhs.get_string(t);
                    let scope = t.get_scope();
                    let var = scope.get_var(t.get_compiler(), name.clone());
                    if Self::is_constant(t.get_compiler(), var, lhs)
                        && !self.initialized_constants.insert(var.unwrap())
                    {
                        Self::report_error(
                            t.get_compiler(),
                            n,
                            var.unwrap(),
                            &name.to_string_lossy(),
                        );
                    } else if var.is_some_and(|var| var.is_goog_module_exports(t.get_compiler()))
                        && !self.initialized_constants.insert(var.unwrap())
                    {
                        let source_file_name = java_string(n.get_source_file_name(t));
                        let compiler = t.get_compiler();
                        let error = JSError::make(
                            compiler,
                            n,
                            &CONST_REASSIGNED_VALUE_ERROR,
                            &["exports", &source_file_name],
                        );
                        compiler.report(error);
                    }
                }
            }
            Token::INC | Token::DEC => {
                let lhs = n.get_first_child(t).unwrap();
                if lhs.is_name(t) {
                    let name = lhs.get_string(t);
                    let scope = t.get_scope();
                    let var = scope.get_var(t.get_compiler(), name.clone());
                    if Self::is_constant(t.get_compiler(), var, lhs) {
                        Self::report_error(
                            t.get_compiler(),
                            n,
                            var.unwrap(),
                            &name.to_string_lossy(),
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

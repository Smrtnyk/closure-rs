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
//   src/com/google/javascript/jscomp/lint/CheckMissingSemicolon.java.

//! Checks for missing semicolons.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    source_file::SourceFile,
};
use closure_rhino::node::{Ast, NodeId};
use std::any::Any;

// port: CheckMissingSemicolon#MISSING_SEMICOLON
pub static MISSING_SEMICOLON: DiagnosticType =
    DiagnosticType::disabled("JSC_MISSING_SEMICOLON", "Missing semicolon");

pub struct CheckMissingSemicolon;

impl CheckMissingSemicolon {
    // port: CheckMissingSemicolon#CheckMissingSemicolon
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self
    }

    // port: CheckMissingSemicolon#shouldHaveSemicolon
    fn should_have_semicolon(ast: &Ast, statement: NodeId) -> bool {
        if statement.is_function(ast)
            || statement.is_class(ast)
            || statement.is_block(ast)
            || statement.is_label_name(ast)
            || statement.is_module_body(ast)
            || (NodeUtil::is_control_structure(ast, statement) && !statement.is_do(ast))
        {
            return false;
        }
        if statement.is_export(ast) {
            return Self::should_have_semicolon(ast, statement.get_first_child(ast).unwrap());
        }
        true
    }

    // port: CheckMissingSemicolon#checkSemicolon
    fn check_semicolon(t: &mut NodeTraversal<'_>, n: NodeId) {
        let static_source_file = n.get_static_source_file(t);
        if let Some(static_source_file) = static_source_file
            && let Some(source_file) =
                (static_source_file.as_ref() as &dyn Any).downcast_ref::<SourceFile>()
        {
            let code = match source_file.get_code() {
                Ok(code) => code,
                // We can't read the original source file. Just skip this check.
                Err(_) => return,
            };
            let length = n.get_length(t);
            if length == 0 {
                // This check needs node lengths to work correctly. If we're not in IDE mode, we
                // don't have that information, so just skip the check.
                return;
            }
            let position = n.get_source_offset(t) + length - 1;
            let ends_with_semicolon = code.char_at(position as usize) == u16::from(b';');
            if !ends_with_semicolon {
                t.report(n, &MISSING_SEMICOLON, &[]);
            }
        }
    }
}

impl CompilerPass for CheckMissingSemicolon {
    // port: CheckMissingSemicolon#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckMissingSemicolon {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckMissingSemicolon#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if !n.is_script(t) && NodeUtil::is_statement(t, n) && Self::should_have_semicolon(t, n) {
            Self::check_semicolon(t, n);
        }
    }
}

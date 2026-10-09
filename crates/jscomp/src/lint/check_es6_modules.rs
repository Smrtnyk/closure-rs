/*
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/lint/CheckEs6Modules.java.

//! Checks that ES6 modules do not import the same module twice and do not use the default
//! export.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    js_string::JsString,
    node::{NodeId, Prop},
    token::Token,
};

// port: CheckEs6Modules#DUPLICATE_IMPORT
pub static DUPLICATE_IMPORT: DiagnosticType = DiagnosticType::warning(
    "JSC_DUPLICATE_IMPORT",
    "The module \"{0}\" has already been imported at {1}, {2}.",
);

// port: CheckEs6Modules#NO_DEFAULT_EXPORT
pub static NO_DEFAULT_EXPORT: DiagnosticType = DiagnosticType::warning(
    "JSC_DEFAULT_EXPORT",
    "Do not use the default export. There is no way to force consistent naming when imported.",
);

pub struct CheckEs6Modules {
    import_specifiers: IndexMap<JsString, NodeId>,
}

impl CheckEs6Modules {
    // port: CheckEs6Modules#CheckEs6Modules
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self {
            import_specifiers: IndexMap::<_, _>::default(),
        }
    }

    // port: CheckEs6Modules#visitImport
    fn visit_import(&mut self, t: &mut NodeTraversal<'_>, import_node: NodeId) {
        let specifier = import_node.get_last_child(t).unwrap().get_string(t);
        let duplicate_import = match self.import_specifiers.get(&specifier) {
            Some(&existing) => Some(existing),
            None => {
                self.import_specifiers
                    .insert(specifier.clone(), import_node);
                None
            }
        };
        if let Some(duplicate_import) = duplicate_import {
            let lineno = duplicate_import.get_lineno(t).to_string();
            let charno = duplicate_import.get_charno(t).to_string();
            t.report(
                import_node,
                &DUPLICATE_IMPORT,
                &[&specifier.to_string(), &lineno, &charno],
            );
        }
    }

    // port: CheckEs6Modules#visitExport
    fn visit_export(t: &mut NodeTraversal<'_>, export: NodeId) {
        if export.get_boolean_prop(t, Prop::EXPORT_DEFAULT) {
            t.report(export, &NO_DEFAULT_EXPORT, &[]);
        }
    }
}

impl CompilerPass for CheckEs6Modules {
    // port: CheckEs6Modules#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckEs6Modules {
    // port: CheckEs6Modules#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(t) {
            Token::ROOT | Token::MODULE_BODY => true,
            Token::SCRIPT => n.get_boolean_prop(t, Prop::ES6_MODULE),
            Token::IMPORT => {
                self.visit_import(t, n);
                false
            }
            Token::EXPORT => {
                Self::visit_export(t, n);
                false
            }
            _ => false,
        }
    }

    // port: CheckEs6Modules#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_module_body(t) {
            self.import_specifiers.clear();
        }
    }
}

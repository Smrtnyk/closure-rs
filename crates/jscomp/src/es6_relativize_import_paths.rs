/*
 * Copyright 2004 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/Es6RelativizeImportPaths.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Rewrites ES6 import paths to be relative after resolving according to the compiler's module
//! resolver.
//!
//! Useful for servers that wish to preserve ES6 modules, meaning their paths need to be valid in
//! the browser.
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    deps::module_loader::ModuleLoader,
    es6_rewrite_modules::Es6RewriteModules,
    node_traversal::{Callback, NodeTraversal},
};
use closure_rhino::{
    java_lang::{unix_path::UnixPath, uri::URI},
    js_string::JsString,
    node::NodeId,
    token::Token,
};

/// Rewrites ES6 import paths to be relative after resolving according to the compiler's module
/// resolver.
// port: Es6RelativizeImportPaths
pub struct Es6RelativizeImportPaths;

impl Es6RelativizeImportPaths {
    // port: Es6RelativizeImportPaths#Es6RelativizeImportPaths
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self
    }
}

impl CompilerPass for Es6RelativizeImportPaths {
    // port: Es6RelativizeImportPaths#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let mut script = root.get_first_child(compiler);
        while let Some(s) = script {
            if Es6RewriteModules::is_es6_module_root(compiler, s) {
                NodeTraversal::traverse(compiler, s, &mut Rewriter);
            }
            script = s.get_next(compiler);
        }
    }
}

// port: Es6RelativizeImportPaths.Rewriter
struct Rewriter;

impl Callback for Rewriter {
    // port: Es6RelativizeImportPaths.Rewriter#shouldTraverse
    fn should_traverse(
        &mut self,
        node_traversal: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(node_traversal) {
            Token::ROOT | Token::MODULE_BODY | Token::SCRIPT => true,
            Token::IMPORT => {
                Self::visit_import(node_traversal, n);
                false
            }
            Token::EXPORT => {
                Self::visit_export(node_traversal, n);
                false
            }
            _ => false,
        }
    }

    // port: NodeTraversal.AbstractPreOrderCallback#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}

impl Rewriter {
    // port: Es6RelativizeImportPaths.Rewriter#visitImport
    fn visit_import(t: &mut NodeTraversal<'_>, import_decl: NodeId) {
        let specifier_node = import_decl.get_last_child(t).unwrap();
        let specifier = specifier_node.get_string(t).to_string_lossy();

        // Leave relative and truly absolute paths (those with a scheme) alone. Only transform
        // absolute paths without a scheme (starting with "/") and ambiguous paths.
        if ModuleLoader::is_relative_identifier(&specifier) {
            return;
        } else {
            match URI::new(&specifier) {
                Ok(specifier_uri) => {
                    if specifier_uri.is_absolute() {
                        return;
                    }
                }
                Err(_) => return,
            }
        }

        let input = t.get_input().unwrap().clone();
        let path = input.get_path(t.get_compiler());
        let mut script_path = path.to_string();

        // If the script path has a scheme / host / port then just use the path part.
        match URI::new(&script_path) {
            Ok(uri) => {
                script_path = uri.get_path().expect("java.lang.NullPointerException");
            }
            Err(_) => return,
        }

        let mut new_specifier = path.resolve_module_as_path(&specifier).to_string();

        // If a module root is stripped then this won't start with "/" when it probably should.
        if !new_specifier.starts_with('/') {
            new_specifier = format!("/{new_specifier}");
        }

        new_specifier = UnixPath::of(&script_path)
            .get_parent()
            .expect("java.lang.NullPointerException")
            .relativize(&UnixPath::of(&new_specifier))
            .to_string();

        // Relativizing two paths with the same directory yields an ambiguous path rather than one
        // starting with "./".
        if ModuleLoader::is_ambiguous_identifier(&new_specifier) {
            new_specifier = format!("./{new_specifier}");
        }

        if new_specifier != specifier {
            specifier_node.set_string(t, JsString::from(new_specifier.as_str()));
            t.report_code_change_at_node(specifier_node);
        }
    }

    // port: Es6RelativizeImportPaths.Rewriter#visitExport
    fn visit_export(t: &mut NodeTraversal<'_>, export: NodeId) {
        if export.has_two_children(t) {
            // export from
            Self::visit_import(t, export);
        }
    }
}

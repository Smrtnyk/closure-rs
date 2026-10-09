/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/lint/CheckExtraRequires.java.

//! Walks the AST looking for usages of qualified names, and 'goog.require's of those names. Then,
//! reconciles the two lists, and reports warning for any unnecessary require statements.

#![allow(clippy::collapsible_match)] // Preserve Java switch cases with nested ifs.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument,
    js_string::JsString,
    node::{Ast, NodeId},
    qualified_name::QualifiedName,
    token::Token,
};
use std::sync::LazyLock;

// port: CheckExtraRequires#EXTRA_REQUIRE_WARNING
pub static EXTRA_REQUIRE_WARNING: DiagnosticType = DiagnosticType::disabled(
    "JSC_EXTRA_REQUIRE_WARNING",
    "extra require: ''{0}'' is never referenced in this file",
);

// TODO(b/130215517): This should eventually be removed and exceptions suppressed
// port: CheckExtraRequires#DEFAULT_EXTRA_NAMESPACES
const DEFAULT_EXTRA_NAMESPACES: [&str; 2] = ["goog.testing.asserts", "goog.testing.jsunit"];

// port: CheckExtraRequires#GOOG_MODULE_GET
static GOOG_MODULE_GET: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.module.get"));

pub struct CheckExtraRequires {
    /// Keys are the local name of a required namespace. Values are the goog.require CALL node.
    requires: IndexMap<JsString, NodeId>,

    /// Adding an entry to usages indicates that the name (either a fully qualified or local name)
    /// is used and can be required.  Note that since usages are name-based and not scoped, any
    /// usage that shadows an unused require in that file will cause the extra require warning to
    /// be missed.
    usages: IndexSet<JsString>,

    /// This is only relevant for the standalone CheckExtraRequires run. This is used to restrict
    /// the linter rule only for the modules listed in this set
    requires_to_remove: Option<IndexSet<String>>,
}

impl CheckExtraRequires {
    /// @param requiresToRemove providing a non-null set to this parameter will result in only
    ///     removing the goog.requires that are in this set. If this is null, it will attempt to
    ///     remove all the unnecessary requires.
    // port: CheckExtraRequires#CheckExtraRequires
    pub fn new(_compiler: &AbstractCompiler, requires_to_remove: Option<IndexSet<String>>) -> Self {
        Self {
            requires: IndexMap::<_, _>::default(),
            usages: IndexSet::<_>::default(),
            requires_to_remove,
        }
    }

    // port: CheckExtraRequires#extractNamespace
    fn extract_namespace(ast: &Ast, call: NodeId, primitive_names: &[&str]) -> Option<JsString> {
        let callee = call.get_first_child(ast).unwrap();
        if !callee.is_get_prop(ast) {
            return None;
        }
        for &primitive_name in primitive_names {
            if callee.matches_qualified_name(ast, primitive_name) {
                let target = callee.get_next(ast);
                if let Some(target) = target
                    && target.is_string_lit(ast)
                {
                    return Some(target.get_string(ast));
                }
            }
        }
        None
    }

    // port: CheckExtraRequires#extractNamespaceIfRequire
    fn extract_namespace_if_require(ast: &Ast, call: NodeId) -> Option<JsString> {
        Self::extract_namespace(ast, call, &["goog.require", "goog.requireType"])
    }

    // port: CheckExtraRequires#extractNamespaceIfForwardDeclare
    fn extract_namespace_if_forward_declare(ast: &Ast, call: NodeId) -> Option<JsString> {
        Self::extract_namespace(ast, call, &["goog.forwardDeclare"])
    }

    // port: CheckExtraRequires#reset
    fn reset(&mut self) {
        self.usages.clear();
        self.requires.clear();
    }

    // port: CheckExtraRequires#visitScriptNode
    fn visit_script_node(&self, compiler: &mut AbstractCompiler) {
        // For every goog.require, check that there is a usage and warn if there is not.
        for (require, &call) in &self.requires {
            if !self.usages.contains(require)
                && self
                    .requires_to_remove
                    .as_ref()
                    .is_none_or(|r| r.contains(&require.to_string_lossy()))
            {
                Self::report_extra_require_warning(compiler, call, require);
            }
        }
    }

    // port: CheckExtraRequires#reportExtraRequireWarning
    fn report_extra_require_warning(
        compiler: &mut AbstractCompiler,
        call: NodeId,
        require: &JsString,
    ) {
        if DEFAULT_EXTRA_NAMESPACES.iter().any(|ns| require == *ns) {
            return;
        }
        let js_doc = NodeUtil::get_best_jsdoc_info(compiler, call);
        if js_doc.is_some_and(|js_doc| {
            js_doc
                .get_suppressions()
                .contains(&JsString::from("extraRequire"))
        }) {
            // There is a @suppress {extraRequire} on the call node or its enclosing statement.
            // This is one of the acceptable places for a @suppress, per
            // https://github.com/google/closure-compiler/wiki/@suppress-annotations
            return;
        }
        compiler.report(JSError::make(
            compiler,
            call,
            &EXTRA_REQUIRE_WARNING,
            &[&require.to_string()],
        ));
    }

    /// @param localName The name that should be used in this file.
    ///
    /// ```text
    /// Require style                        | localName
    /// -------------------------------------|----------
    /// goog.require('foo.bar');             | foo.bar
    /// var bar = goog.require('foo.bar');   | bar
    /// var {qux} = goog.require('foo.bar'); | qux
    /// import {qux} from 'foo.bar';         | qux
    /// ```
    // port: CheckExtraRequires#visitRequire
    fn visit_require(&mut self, local_name: JsString, node: NodeId) {
        self.requires.entry(local_name).or_insert(node);
    }

    // port: CheckExtraRequires#visitImportNode
    fn visit_import_node(&mut self, ast: &Ast, import_node: NodeId) {
        let default_import = import_node.get_first_child(ast).unwrap();
        if default_import.is_name(ast) {
            self.visit_require(default_import.get_string(ast), import_node);
        }
        let named_imports = default_import.get_next(ast).unwrap();
        if named_imports.is_import_specs(ast) {
            let mut import_spec = named_imports.get_first_child(ast);
            while let Some(spec) = import_spec {
                self.visit_require(
                    spec.get_last_child(ast).unwrap().get_string(ast),
                    import_node,
                );
                import_spec = spec.get_next(ast);
            }
        }
    }

    // port: CheckExtraRequires#visitForwardDeclare
    fn visit_forward_declare(
        &mut self,
        ast: &Ast,
        namespace: JsString,
        forward_declare_call: NodeId,
        parent: NodeId,
    ) {
        self.visit_goog_require(ast, namespace, forward_declare_call, parent);
    }

    // port: CheckExtraRequires#visitGoogRequire
    fn visit_goog_require(
        &mut self,
        ast: &Ast,
        namespace: JsString,
        goog_require_call: NodeId,
        parent: NodeId,
    ) {
        if parent.is_name(ast) {
            self.visit_require(parent.get_string(ast), goog_require_call);
        } else if parent.is_destructuring_lhs(ast)
            && parent.get_first_child(ast).unwrap().is_object_pattern(ast)
        {
            if parent.get_first_child(ast).unwrap().has_children(ast) {
                let mut string_key = parent.get_first_first_child(ast);
                while let Some(key) = string_key {
                    string_key = key.get_next(ast);
                    let import_name = key.get_first_child(ast).unwrap();
                    if !import_name.is_name(ast) {
                        // invalid reported elsewhere
                        continue;
                    }
                    self.visit_require(import_name.get_string(ast), import_name);
                }
            } else {
                self.visit_require(namespace, goog_require_call);
            }
        } else {
            self.visit_require(namespace, goog_require_call);
        }
    }

    // port: CheckExtraRequires#visitCallNode
    fn visit_call_node(&mut self, ast: &Ast, call: NodeId, parent: NodeId) {
        let required = Self::extract_namespace_if_require(ast, call);
        if let Some(required) = required {
            self.visit_goog_require(ast, required, call, parent);
            return;
        }
        let declare = Self::extract_namespace_if_forward_declare(ast, call);
        if let Some(declare) = declare {
            self.visit_forward_declare(ast, declare, call, parent);
            return;
        }
        let callee = call.get_first_child(ast).unwrap();
        if GOOG_MODULE_GET.matches(ast, callee)
            && call.get_second_child(ast).unwrap().is_string_lit(ast)
        {
            self.usages
                .insert(call.get_second_child(ast).unwrap().get_string(ast));
        }
    }

    // port: CheckExtraRequires#addUsagesOfAllPrefixes
    fn add_usages_of_all_prefixes(&mut self, ast: &Ast, mut qualified_name: QualifiedName) {
        self.usages.insert(qualified_name.join(ast));
        // For "foo.bar.baz.qux" add usages for "foo.bar.baz.qux", "foo.bar.baz",
        // "foo.bar", and "foo" because any of those might be a require that
        // we need to include.
        while !qualified_name.is_simple(ast) {
            qualified_name = qualified_name.get_owner(ast).unwrap();
            self.usages.insert(qualified_name.join(ast));
        }
    }

    // port: CheckExtraRequires#visitQualifiedName
    fn visit_qualified_name(&mut self, ast: &Ast, n: NodeId) {
        check_argument!(n.is_qualified_name(ast), "%s", n.to_string(ast));
        self.add_usages_of_all_prefixes(ast, n.get_qualified_name_object(ast).unwrap());
    }

    // port: CheckExtraRequires#maybeAddJsDocUsages
    fn maybe_add_js_doc_usages(&mut self, ast: &Ast, n: NodeId) {
        let Some(info) = n.get_jsdoc_info(ast) else {
            return;
        };
        for e in info.get_type_expressions() {
            for type_name in e.get_all_type_names(ast) {
                self.add_usages_of_all_prefixes(ast, QualifiedName::of(type_name));
            }
        }
    }
}

impl CompilerPass for CheckExtraRequires {
    // port: CheckExtraRequires#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        self.reset();
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckExtraRequires {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckExtraRequires#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self.maybe_add_js_doc_usages(t, n);
        match n.get_token(t) {
            Token::NAME => {
                let parent = parent.unwrap();
                if !NodeUtil::is_l_value(t, n)
                    && !parent.is_get_prop(t)
                    && !parent.is_import_spec(t)
                {
                    self.visit_qualified_name(t, n);
                }
            }
            Token::GETPROP => {
                // If parent is a GETPROP, they will handle all the usages.
                if !parent.unwrap().is_get_prop(t) && n.is_qualified_name(t) {
                    self.visit_qualified_name(t, n);
                }
            }
            Token::CALL => self.visit_call_node(t, n, parent.unwrap()),
            Token::SCRIPT => {
                self.visit_script_node(t.get_compiler());
                self.reset();
            }
            Token::IMPORT => self.visit_import_node(t, n),
            _ => {}
        }
    }
}

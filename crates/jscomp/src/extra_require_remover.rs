/*
 * Copyright 2024 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ExtraRequireRemover.java.

#![allow(clippy::collapsible_match)] // Retain the Java switches and their nested branches.
//! Port of `com.google.javascript.jscomp.ExtraRequireRemover`.
//!
//! Walks the AST looking for usages of qualified names, and 'goog.require's of those names. Then,
//! reconciles the two lists, and removes any unnecessary require statements.
//!
//! ```text
//!                Input js                    |     Generated i.js (with ExtraRequireRemover)
//! --------------------------------------------------------------------------------------------
//!     const Foo1 = goog.require('Foo');      |       const Foo1 = goog.require('Foo');
//!     const Bar1 = goog.require('Bar');      |
//!     /** @type {!Foo1} */                   |        /** @type {!Foo1} *\
//!     let foo;                               |        let foo;
//!     let a = A1();                          |        let a = A1();
//! ```
//!
//! Note how `const Bar1 = goog.require('Bar');` is removed bc it is not used in the .i.js file.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, check_state,
    js_string::JsString,
    node::{Ast, NodeId, Prop},
    qualified_name::QualifiedName,
    token::Token,
};
use std::sync::LazyLock;

// port: ExtraRequireRemover#DEFAULT_EXTRA_NAMESPACES
const DEFAULT_EXTRA_NAMESPACES: &[&str] = &[
    "goog.testing.asserts",
    "goog.testing.jsunit",
    "goog.labs.testing.Environment",
];

// port: ExtraRequireRemover#GOOG_REQUIRE
static GOOG_REQUIRE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.require"));
// port: ExtraRequireRemover#GOOG_REQUIRE_TYPE
static GOOG_REQUIRE_TYPE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.requireType"));
// port: ExtraRequireRemover#GOOG_FORWARD_DECLARE
static GOOG_FORWARD_DECLARE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.forwardDeclare"));
// port: ExtraRequireRemover#GOOG_MODULE_GET
static GOOG_MODULE_GET: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.module.get"));

pub struct ExtraRequireRemover {
    /// Keys are the local name of a required namespace. Values are the goog.require CALL node.
    requires: IndexMap<JsString, NodeId>,
    /// Adding an entry to `usages` indicates that the name (either a fully qualified or local
    /// name) is used and can be required. Note that since `usages` are name-based and not scoped,
    /// any usage that shadows an unused require in that file will cause the extra require not to
    /// be removed.
    usages: IndexSet<JsString>,
}

impl ExtraRequireRemover {
    // port: ExtraRequireRemover#ExtraRequireRemover
    pub fn new() -> Self {
        Self {
            requires: IndexMap::<_, _>::default(),
            usages: IndexSet::<_>::default(),
        }
    }

    /// Extracts the namespace from matching goog.require, goog.requireType, and
    /// goog.forwardDeclare nodes.
    ///
    /// Ex: `goog.require('foo.bar');` will return `foo.bar`
    // port: ExtraRequireRemover#extractNamespace
    fn extract_namespace(
        ast: &Ast,
        call: NodeId,
        primitive_names: &[&QualifiedName],
    ) -> Option<JsString> {
        let callee = call.get_first_child(ast).unwrap();
        if !callee.is_get_prop(ast) {
            return None;
        }
        for primitive_name in primitive_names {
            if primitive_name.matches(ast, callee) {
                let target = callee.get_next(ast);
                if let Some(target) = target.filter(|target| target.is_string_lit(ast)) {
                    return Some(target.get_string(ast));
                }
            }
        }
        None
    }

    /// Extract namespace from goog.require and goog.requireType nodes
    // port: ExtraRequireRemover#extractNamespaceIfRequire
    fn extract_namespace_if_require(ast: &Ast, call: NodeId) -> Option<JsString> {
        Self::extract_namespace(
            ast,
            call,
            &[&GOOG_REQUIRE, &GOOG_REQUIRE_TYPE, &GOOG_FORWARD_DECLARE],
        )
    }

    // port: ExtraRequireRemover#reset
    fn reset(&mut self) {
        self.usages.clear();
        self.requires.clear();
    }

    /// For every goog.require, check that there is a usage and remove the import if the
    /// goog.require is not found in `usages`
    // port: ExtraRequireRemover#removeExtraRequiresInScript
    fn remove_extra_requires_in_script(&mut self, compiler: &mut AbstractCompiler) {
        let entries: Vec<(JsString, NodeId)> = self
            .requires
            .iter()
            .map(|(require, call)| (require.clone(), *call))
            .collect();
        for (require, call) in entries {
            self.remove_extra_require(compiler, call, &require);
        }
    }

    /// Reconciles the two lists (`usages` and `requires`) and removes requires that are used
    /// (found in `usages`). There are different cases in which we determine to remove an
    /// unnecessary goog.require:
    ///
    /// 1) Extra requires with local-level suppressions should not be removed. (@suppress
    ///    {extraRequire})
    ///
    /// 2) Remove expression statements. ex: const x = goog.require('foo');
    ///
    /// 3) Remove aliased goog.require imports: const bar = goog.require('foo.bar');
    ///
    /// 4) Remove destructured goog.require imports if the identifier(s) are unused: const {x,
    ///    unused} = goog.require('foo.bar');
    // port: ExtraRequireRemover#removeExtraRequire
    fn remove_extra_require(
        &mut self,
        compiler: &mut AbstractCompiler,
        call: NodeId,
        require: &JsString,
    ) {
        if self.usages.contains(require) {
            return;
        }

        // If a goog.require contains a /** @suppress {extraRequire} */ local-level suppression
        // (suppression right above the import line), we will NOT prune the goog.require. This
        // suppression indicates that the goog.require is actually needed even though it does not
        // appear to be referenced in the file, so don't prune it.
        let js_doc = NodeUtil::get_best_jsdoc_info(compiler, call);
        if js_doc.is_some_and(|js_doc| {
            js_doc
                .get_suppressions()
                .contains(&JsString::from("extraRequire"))
        }) {
            return;
        }

        // Workaround for tsickle bug affecting JSPB imports.
        // TODO: b/520110716 - Remove this once the bug is fixed.
        if Self::is_tsickle_proto_call(compiler, call) {
            return;
        }

        // Case 1: remove if this is an expression statement.
        // ex: goog.require('foo');
        let call_parent = call.get_parent(compiler).unwrap();
        if call_parent.is_expr_result(compiler) {
            // all unaliased imports in goog.module are not pruned regardless of if they are used
            // or not
            if !call
                .get_grandparent(compiler)
                .unwrap()
                .is_module_body(compiler)
            {
                compiler.report_change_to_enclosing_scope(call_parent);
                call_parent.detach(compiler);
            }
            return;
        }

        // Case 2: this is a 'default' import esque thing remove the entire statement.
        // ex: Remove `const bar = goog.require('foo.bar');`
        if call_parent.is_name(compiler) {
            let name_declaration = call.get_grandparent(compiler).unwrap();
            check_state!(NodeUtil::is_name_declaration(
                compiler,
                call.get_grandparent(compiler)
            ));
            let declaration_parent = name_declaration.get_parent(compiler).unwrap();
            compiler.report_change_to_enclosing_scope(declaration_parent);
            name_declaration.detach(compiler);
            return;
        }

        // Case 3: this is part of a destructuring import.
        //   const {unused} = goog.require('completely.unused');
        //   const {x, unused} = goog.require('foo.bar');
        // Prune destructured imports that are partially unused by pruning the
        // unused identifier(s). If all the identifiers are unused, prune the entire
        // import.
        check_state!(
            call_parent.is_string_key(compiler),
            "{}",
            call_parent.to_string(compiler)
        );
        let object_pattern = call.get_grandparent(compiler).unwrap();
        // the 'const {unused} = goog.require('completely.unused');' case
        if object_pattern.has_one_child(compiler) {
            let name_declaration = object_pattern.get_grandparent(compiler).unwrap();
            check_state!(
                NodeUtil::is_name_declaration(compiler, Some(name_declaration)),
                "{}",
                name_declaration.to_string(compiler)
            );
            let declaration_parent = name_declaration.get_parent(compiler).unwrap();
            compiler.report_change_to_enclosing_scope(declaration_parent);
            name_declaration.detach(compiler);
            return;
        }

        // 'const {x, unused} = goog.require('foo.bar');' in this case just remove 'unused'.
        let grandparent = call.get_grandparent(compiler).unwrap();
        compiler.report_change_to_enclosing_scope(grandparent);
        call_parent.detach(compiler);
    }

    /// `local_name`: the name that should be used in this file.
    ///
    /// ```text
    /// Require style                        | localName
    /// -------------------------------------|----------
    /// goog.require('foo.bar');             | foo.bar
    /// var bar = goog.require('foo.bar');   | bar
    /// var {qux} = goog.require('foo.bar'); | qux
    /// import {qux} from 'foo.bar';         | qux
    /// ```
    ///
    /// `namespace`: the namespace being imported. May be identical to localName in the case of
    /// `goog.require('foo.bar');`
    // port: ExtraRequireRemover#visitRequire
    fn visit_require(&mut self, local_name: JsString, namespace: &JsString, node: NodeId) {
        if DEFAULT_EXTRA_NAMESPACES.iter().any(|n| namespace == *n) {
            return;
        }
        self.requires.entry(local_name).or_insert(node);
    }

    /// Visits local names and adds the name to `usages`.
    ///
    /// Ex: `const A = goog.require('foo');` add `A` to `usages`
    ///
    /// If it is a destructured import, add each identifier to `usages`.
    ///
    /// Ex: `const {Foo, Bar, Baz} = goog.require('foo');` add `Foo`, `Bar`, and `Baz` to `usages`
    // port: ExtraRequireRemover#visitGoogRequire
    fn visit_goog_require(
        &mut self,
        ast: &Ast,
        namespace: JsString,
        goog_require_call: NodeId,
        parent: NodeId,
    ) {
        // local names
        if parent.is_name(ast) {
            self.visit_require(parent.get_string(ast), &namespace, goog_require_call);
        } else if parent.is_destructuring_lhs(ast)
            && parent.get_first_child(ast).unwrap().is_object_pattern(ast)
        {
            // destructured imports
            if parent.get_first_child(ast).unwrap().has_children(ast) {
                // loop through identifiers in destructured import
                let mut string_key = parent.get_first_first_child(ast);
                while let Some(key) = string_key {
                    let import_name = key.get_first_child(ast).unwrap();
                    self.visit_require(import_name.get_string(ast), &namespace, import_name);
                    string_key = key.get_next(ast);
                }
            } else {
                self.visit_require(namespace.clone(), &namespace, goog_require_call);
            }
        } else {
            self.visit_require(namespace.clone(), &namespace, goog_require_call);
        }
    }

    /// Extract the namespace and check if the qualified name matches the callee of
    /// `goog.require`, `goog.requireType`, or `goog.forwardDeclare`. If so, we will visit those
    /// corresponding nodes and add those usages.
    ///
    /// Ex: Qualified name is 'goog.module.get('foo')' and callee is 'goog.require('foo')'
    // port: ExtraRequireRemover#visitCallNode
    fn visit_call_node(&mut self, ast: &Ast, call: NodeId, parent: NodeId) {
        let required = Self::extract_namespace_if_require(ast, call);
        if let Some(required) = required {
            self.visit_goog_require(ast, required, call, parent);
            return;
        }
        let callee = call.get_first_child(ast).unwrap();
        // Java NPEs when goog.module.get has no argument.
        if GOOG_MODULE_GET.matches(ast, callee)
            && call.get_second_child(ast).unwrap().is_string_lit(ast)
        {
            self.usages
                .insert(call.get_second_child(ast).unwrap().get_string(ast));
        }
    }

    /// For qualified name node, we want to add all usages for each prefix of the name, because
    /// one of them might be required.
    ///
    /// For 'foo.bar.baz.qux' add usages for 'foo.bar.baz.qux', 'foo.bar.baz', 'foo.bar', and
    /// 'foo' because any of those might be a require that we need to include.
    // port: ExtraRequireRemover#addUsagesOfAllPrefixes
    fn add_usages_of_all_prefixes(&mut self, ast: &Ast, mut qualified_name: QualifiedName) {
        self.usages.insert(qualified_name.join(ast));
        while !qualified_name.is_simple(ast) {
            qualified_name = qualified_name.get_owner(ast).unwrap();
            self.usages.insert(qualified_name.join(ast));
        }
    }

    // port: ExtraRequireRemover#visitQualifiedName
    fn visit_qualified_name(&mut self, ast: &Ast, n: NodeId) {
        check_argument!(n.is_qualified_name(ast), "{}", n.to_string(ast));
        self.add_usages_of_all_prefixes(ast, n.get_qualified_name_object(ast).unwrap());
    }

    /// If the node has corresponding jsdoc info then go through its type names and track usages
    // port: ExtraRequireRemover#maybeAddJsDocUsages
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

    // port: ExtraRequireRemover#isTsickleProtoCall
    fn is_tsickle_proto_call(ast: &Ast, call: NodeId) -> bool {
        // Java NPEs on a null source file name.
        if !call.is_call(ast)
            || !call
                .get_source_file_name(ast)
                .unwrap()
                .ends_with(".closure.js")
        {
            return false;
        }
        let module_id = call.get_second_child(ast);
        module_id.is_some_and(|module_id| {
            module_id.is_string_lit(ast) && module_id.get_string(ast).starts_with("proto.")
        })
    }
}

impl Default for ExtraRequireRemover {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for ExtraRequireRemover {
    // port: ExtraRequireRemover#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        self.reset();
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for ExtraRequireRemover {
    // port: ExtraRequireRemover#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        // Skip ES modules. We could support them in the future if desired.
        !n.is_module_body(t)
            || n.get_parent(t)
                .unwrap()
                .get_boolean_prop(t, Prop::GOOG_MODULE)
    }

    /// Visits each corresponding node and tracks all goog.requires as well as their usages
    // port: ExtraRequireRemover#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self.maybe_add_js_doc_usages(t, n);
        match n.get_token(t) {
            Token::NAME => {
                if !NodeUtil::is_l_value(t, n) && !parent.unwrap().is_get_prop(t) {
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
                self.remove_extra_requires_in_script(t.get_compiler());
                self.reset();
            }
            _ => {}
        }
    }
}

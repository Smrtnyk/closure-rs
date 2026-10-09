/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/ijs/ConvertToTypedInterface.java,
//   src/com/google/javascript/jscomp/ijs/ProcessConstJsdocCallback.java.

#![allow(clippy::collapsible_match)] // Retain the Java switches and their nested branches.
//! Port of `com.google.javascript.jscomp.ijs.ConvertToTypedInterface`.
//!
//! The goal of this pass is to shrink the AST, preserving only typing, not behavior.
//!
//! To do this, it does things like removing function/method bodies, rvalues that are not needed,
//! expressions that are not declarations, etc.
//!
//! This is conceptually similar to the ijar tool[1] that bazel uses to shrink jars into minimal
//! versions that can be used equivalently for compilation of downstream dependencies.
//!
//! [1] https://github.com/bazelbuild/bazel/blob/master/third_party/ijar/README.txt

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    ijs::{
        class_util::ClassUtil, file_info::FileInfo, jsdoc_util::JsdocUtil,
        potential_declaration::PotentialDeclaration,
        process_const_jsdoc_callback::ProcessConstJsdocCallback,
    },
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    rewrite_caller_code_location::GOOG_CALLER_LOCATION_QUALIFIED_NAME,
    scope::ScopeId,
    var::VarId,
};
use closure_jstype::{js_type::Nullability, prelude::*};
use closure_rhino::{
    check_argument, check_state,
    ir::IR,
    js_string::JsString,
    jsdoc_info::{JSDocInfo, Visibility},
    node::NodeId,
    qualified_name::QualifiedName,
    token::Token,
};
use std::sync::{Arc, LazyLock};

// port: ConvertToTypedInterface#CONSTANT_WITH_SUGGESTED_TYPE
pub static CONSTANT_WITH_SUGGESTED_TYPE: DiagnosticType = DiagnosticType::warning(
    "JSC_CONSTANT_WITH_SUGGESTED_TYPE",
    "Constants in top-level should have types explicitly specified.\nYou may want specify this type as:\t@const '{'{0}'}'",
);

// port: ConvertToTypedInterface#CONSTANT_WITHOUT_EXPLICIT_TYPE
pub static CONSTANT_WITHOUT_EXPLICIT_TYPE: DiagnosticType = DiagnosticType::warning(
    "JSC_CONSTANT_WITHOUT_EXPLICIT_TYPE",
    "Constants in top-level should have types explicitly specified.",
);

// port: ConvertToTypedInterface#GOOG_SCOPE_HIDDEN_TYPE
pub static GOOG_SCOPE_HIDDEN_TYPE: DiagnosticType = DiagnosticType::warning(
    "JSC_GOOG_SCOPE_HIDDEN_TYPE",
    "Found a goog.scope local type declaration.\nIf you can't yet migrate this file to goog.module, use a /** @private */ declaration within the goog.provide namespace.",
);

// port: ConvertToTypedInterface#CALLS_TO_PRESERVE
const CALLS_TO_PRESERVE: &[&str] = &[
    "Polymer",
    "goog.addSingletonGetter",
    "goog.define",
    "goog.forwardDeclare",
    "goog.module",
    "goog.module.declareLegacyNamespace",
    "goog.declareModuleId",
    "goog.provide",
    "goog.require",
    "goog.requireType",
];

pub struct ConvertToTypedInterface;

impl ConvertToTypedInterface {
    // port: ConvertToTypedInterface#ConvertToTypedInterface
    pub fn new() -> Self {
        Self
    }

    // port: ConvertToTypedInterface#maybeReport
    fn maybe_report(
        compiler: &mut AbstractCompiler,
        node: NodeId,
        diagnostic: &'static DiagnosticType,
        fillers: &[&str],
    ) {
        // Java NPEs on a null source name.
        let source_name = NodeUtil::get_source_name(compiler, node).unwrap();
        if source_name.ends_with("_test.js")
            || source_name.ends_with("_test.closure.js")
            || source_name.ends_with("_test.tsx.cl.js")
        {
            // Allow _test.js files and their tsickle generated
            // equivalents to avoid emitting errors at .i.js generation time.
            // We expect these files to not be consumed by any other downstream libraries.
            return;
        }
        let error = JSError::make(compiler, node, diagnostic, fillers);
        compiler.report(error);
    }

    // port: ConvertToTypedInterface#maybeWarnForConstWithoutExplicitType
    fn maybe_warn_for_const_without_explicit_type(
        compiler: &mut AbstractCompiler,
        decl: &PotentialDeclaration,
    ) {
        if decl.is_const_to_be_inferred(compiler)
            && !decl.get_lhs().is_from_externs(compiler)
            && !JsdocUtil::is_private(decl.get_js_doc(compiler).as_deref())
        {
            let name_node = decl.get_lhs();
            match name_node.get_jstype(compiler) {
                None => {
                    Self::maybe_report(compiler, name_node, &CONSTANT_WITHOUT_EXPLICIT_TYPE, &[]);
                }
                Some(jstype) => {
                    let annotation = {
                        let (registry, ast) = compiler.get_type_registry_and_ast();
                        jstype.to_annotation_string(registry, ast, Nullability::EXPLICIT)
                    };
                    Self::maybe_report(
                        compiler,
                        name_node,
                        &CONSTANT_WITH_SUGGESTED_TYPE,
                        &[&annotation],
                    );
                }
            }
        }
    }

    // port: ConvertToTypedInterface#processFile
    fn process_file(&mut self, compiler: &mut AbstractCompiler, script_node: NodeId) {
        check_argument!(script_node.is_script(compiler));
        let source_file_name = script_node.get_source_file_name(compiler);
        if AbstractCompiler::is_fill_file_name(source_file_name.as_deref().unwrap_or_default()) {
            script_node.detach(compiler);
            return;
        }

        let script_js_doc = script_node.get_jsdoc_info(compiler);
        if let Some(script_js_doc) = script_js_doc.filter(|doc| doc.is_closure_unaware_code()) {
            // If we are generating type summary files, then those files won't contain the method
            // content from within the closure-unaware section, and the entire file effectively
            // becomes closure-aware again (as it is generated code that describes a module shape).
            let mut script_js_doc_builder = script_js_doc.to_builder();
            script_js_doc_builder.remove_closure_unaware_code();
            script_node.set_jsdoc_info(compiler, script_js_doc_builder.build());
        }

        let mut current_file = FileInfo::new(source_file_name.as_deref().unwrap_or_default());
        NodeTraversal::traverse(compiler, script_node, &mut RemoveNonDeclarations);
        NodeTraversal::traverse(
            compiler,
            script_node,
            &mut PropagateConstJsdoc {
                current_file: &mut current_file,
            },
        );
        SimplifyDeclarations::new(&mut current_file).simplify_all(compiler);
    }

    // port: ConvertToTypedInterface#findNameDeclaration
    fn find_name_declaration(
        compiler: &mut AbstractCompiler,
        scope: ScopeId,
        rhs: NodeId,
    ) -> Option<VarId> {
        if !rhs.is_name(compiler) {
            return None;
        }
        let name = rhs.get_string(compiler);
        scope.get_var(compiler, &name)
    }

    // port: ConvertToTypedInterface#isSymbolProp
    pub(crate) fn is_symbol_prop(ast: &closure_rhino::node::Ast, lhs: NodeId) -> bool {
        lhs.is_get_prop(ast)
            && lhs
                .get_first_child(ast)
                .unwrap()
                .matches_name(ast, "Symbol")
    }

    // port: ConvertToTypedInterface#shouldPreserveAssignment
    fn should_preserve_assignment(expr: NodeId, t: &mut NodeTraversal<'_>) -> bool {
        let lhs = expr.get_first_child(t).unwrap();
        // Ignore assignments in function bodies, unless they're also a constructor with a this.
        // prop.
        if !t.in_global_hoist_scope() && !t.in_module_hoist_scope() {
            return ClassUtil::is_this_prop_inside_class_with_name(t, lhs);
        }

        // Well-known symbol properties, like Foo.prototype[Symbol.iterator] = function() {};
        if lhs.is_get_elem(t) && Self::is_symbol_prop(t, lhs.get_second_child(t).unwrap()) {
            return lhs.get_first_child(t).unwrap().is_qualified_name(t);
        }
        // Assignments to names don't have global typechecking side-effects even within the 'hoist
        // scope'
        if lhs.is_name(t) {
            return t.in_global_scope() || t.in_module_scope();
        }
        lhs.is_qualified_name(t)
    }
}

impl Default for ConvertToTypedInterface {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for ConvertToTypedInterface {
    // port: ConvertToTypedInterface#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let mut script = root.get_first_child(compiler);
        while let Some(s) = script {
            self.process_file(compiler, s);
            // As in Java, the next sibling is read after processFile (a detached fill file has
            // none, which ends the loop).
            script = s.get_next(compiler);
        }
    }
}

static GOOG_SCOPE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.scope"));

// port: ConvertToTypedInterface.RemoveNonDeclarations
struct RemoveNonDeclarations;

impl RemoveNonDeclarations {
    /// Does three simplifications to const/let/var nodes.
    ///
    /// - Splits them so that each declaration is a separate statement.
    /// - Removes non-import and non-alias destructuring statements, which we assume are not type
    ///   declarations.
    /// - Moves inline JSDoc annotations onto the declaration nodes.
    // port: ConvertToTypedInterface.RemoveNonDeclarations#splitNameDeclarationsAndRemoveDestructuring
    fn split_name_declarations_and_remove_destructuring(n: NodeId, t: &mut NodeTraversal<'_>) {
        check_argument!(NodeUtil::is_name_declaration(t, Some(n)));
        let shared_jsdoc = n.get_jsdoc_info(t);
        let is_export = n.get_parent(t).unwrap().is_export(t);
        let statement = if is_export {
            n.get_parent(t).unwrap()
        } else {
            n
        };
        while n.has_children(t) {
            let lhs_to_split = n.get_last_child(t).unwrap();
            let name_jsdoc = lhs_to_split.get_jsdoc_info(t);
            lhs_to_split.set_jsdoc_info(t, None);
            let merged_jsdoc = JsdocUtil::merge_jsdocs(shared_jsdoc.clone(), name_jsdoc.as_deref());
            if n.has_one_child(t) {
                n.set_jsdoc_info(t, merged_jsdoc);
                return;
            }
            // A name declaration with more than one LHS is split into separate declarations.
            let rhs = if lhs_to_split.has_children(t) {
                lhs_to_split.remove_first_child(t)
            } else {
                None
            };
            let detached = lhs_to_split.detach(t);
            let token = n.get_token(t);
            let mut new_declaration =
                NodeUtil::new_declaration(t, detached, rhs, token).srcref(t, n);
            new_declaration.set_jsdoc_info(t, merged_jsdoc);
            if is_export {
                new_declaration = IR::export(t, new_declaration).srcref(t, statement);
            }
            new_declaration.insert_after(t, statement);
            t.report_code_change();
        }
    }
}

impl Callback for RemoveNonDeclarations {
    // port: ConvertToTypedInterface.RemoveNonDeclarations#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(t) {
            Token::FUNCTION => {
                if !ClassUtil::is_constructor(t, n) || !ClassUtil::has_named_class(t, n) {
                    let body = n.get_last_child(t).unwrap();
                    if !body.is_block(t) || body.has_children(t) {
                        t.report_code_change_at_node(body);
                        let block = IR::block(t).srcref(t, body);
                        body.replace_with(t, block);
                        NodeUtil::mark_functions_deleted(t.get_compiler(), body);
                    }
                }
                true
            }
            Token::MEMBER_FIELD_DEF => {
                if ClassUtil::is_member_field_def_inside_class_with_name(t, n) {
                    return true;
                }
                // We will remove fields in anonymous classes
                NodeUtil::delete_node(t.get_compiler(), n);
                false
            }
            Token::EXPR_RESULT => {
                let expr = n.get_first_child(t).unwrap();
                match expr.get_token(t) {
                    Token::CALL => {
                        let callee = expr.get_first_child(t).unwrap();
                        check_state!(!GOOG_SCOPE.matches(t, callee));
                        if callee
                            .get_qualified_name(t)
                            .is_some_and(|name| CALLS_TO_PRESERVE.iter().any(|c| name == *c))
                        {
                            return true;
                        }
                        NodeUtil::delete_node(t.get_compiler(), n);
                        false
                    }
                    Token::ASSIGN => {
                        if ConvertToTypedInterface::should_preserve_assignment(expr, t) {
                            return true;
                        }
                        NodeUtil::delete_node(t.get_compiler(), n);
                        false
                    }
                    Token::GETPROP => {
                        if !expr.is_qualified_name(t) || expr.get_jsdoc_info(t).is_none() {
                            NodeUtil::delete_node(t.get_compiler(), n);
                            return false;
                        }
                        true
                    }
                    Token::GETELEM => {
                        if ConvertToTypedInterface::is_symbol_prop(
                            t,
                            expr.get_second_child(t).unwrap(),
                        ) && expr.get_jsdoc_info(t).is_some()
                        {
                            return true;
                        }

                        NodeUtil::delete_node(t.get_compiler(), n);
                        false
                    }
                    _ => {
                        NodeUtil::delete_node(t.get_compiler(), n);
                        false
                    }
                }
            }
            Token::COMPUTED_PROP => {
                if ClassUtil::is_computed_member_inside_class_with_name(t, n) {
                    return true;
                }
                if !NodeUtil::is_lhs_by_destructuring(t, n.get_second_child(t).unwrap()) {
                    NodeUtil::delete_node(t.get_compiler(), n);
                }
                false
            }
            Token::COMPUTED_FIELD_DEF => {
                if ClassUtil::is_computed_member_inside_class_with_name(t, n) {
                    return true;
                }
                NodeUtil::delete_node(t.get_compiler(), n);
                false
            }
            Token::THROW
            | Token::RETURN
            | Token::BREAK
            | Token::CONTINUE
            | Token::DEBUGGER
            | Token::EMPTY => {
                if NodeUtil::is_statement_parent(t, parent.unwrap()) {
                    NodeUtil::delete_node(t.get_compiler(), n);
                }
                false
            }
            Token::LABEL | Token::IF | Token::SWITCH | Token::CASE | Token::WHILE => {
                // First child can't have declaration. Statement itself will be removed
                // post-order.
                let first = n.get_first_child(t).unwrap();
                NodeUtil::delete_node(t.get_compiler(), first);
                true
            }
            Token::TRY | Token::DO => {
                // Second child can't have declarations. Statement itself will be removed
                // post-order.
                let second = n.get_second_child(t).unwrap();
                NodeUtil::delete_node(t.get_compiler(), second);
                true
            }
            Token::FOR | Token::FOR_OF | Token::FOR_AWAIT_OF | Token::FOR_IN => {
                if n.get_token(t) == Token::FOR {
                    let second = n.get_second_child(t).unwrap();
                    NodeUtil::delete_node(t.get_compiler(), second);
                    // fall-through
                }
                let second = n.get_second_child(t).unwrap();
                NodeUtil::delete_node(t.get_compiler(), second);
                let initializer = n.remove_first_child(t).unwrap();
                if initializer.is_var(t) {
                    n.get_last_child(t)
                        .unwrap()
                        .add_child_to_front(t, initializer);
                }
                true
            }
            Token::CONST | Token::LET => {
                if !t.in_global_scope() && !t.in_module_scope() {
                    let parent = parent.unwrap();
                    NodeUtil::remove_child(t, parent, n);
                    t.report_code_change_at_node(parent);
                    return false;
                }
                true
            }
            Token::VAR => {
                if !t.in_global_hoist_scope() && !t.in_module_hoist_scope() {
                    let parent = parent.unwrap();
                    NodeUtil::remove_child(t, parent, n);
                    t.report_code_change_at_node(parent);
                    return false;
                }
                true
            }
            Token::MODULE_BODY
            | Token::CLASS
            | Token::DEFAULT_CASE
            | Token::BLOCK
            | Token::EXPORT
            | Token::IMPORT => true,
            _ => {
                check_state!(!NodeUtil::is_statement(t, n), "{:?}", n.get_token(t));
                true
            }
        }
    }

    // port: ConvertToTypedInterface.RemoveNonDeclarations#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::TRY
            | Token::LABEL
            | Token::DEFAULT_CASE
            | Token::CASE
            | Token::DO
            | Token::WHILE
            | Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::IF => {
                if n.has_parent(t) {
                    let children = n.remove_children(t);
                    parent.unwrap().add_children_after(t, children, Some(n));
                    // We don't need the special valid-AST-preserving behavior of
                    // `NodeUtil.removeChild()` here. We always want to remove exactly and only
                    // `n`, so we use `n.detach()`.
                    //
                    // Also, the shouldTraverse() method intentionally puts the AST in an invalid
                    // state by moving children to parents where they are not valid temporarily.
                    // `NodeUtil.removeChild()` would throw an exception here if it noticed the
                    // invalid AST state.
                    n.detach(t);
                    t.report_code_change();
                }
            }
            Token::SWITCH => {
                // shouldTraverse() removed the switch condition already, so we just need to
                // handle the cases.
                if n.has_parent(t) {
                    check_state!(
                        n.has_one_child(t),
                        "malfrmed sWITCH %s",
                        n.to_string_tree(t)
                    );
                    let children = n.get_first_child(t).unwrap().remove_children(t);
                    parent.unwrap().add_children_after(t, children, Some(n));
                    n.detach(t);
                    t.report_code_change();
                }
            }
            Token::VAR | Token::LET | Token::CONST => {
                Self::split_name_declarations_and_remove_destructuring(n, t)
            }
            Token::BLOCK => {
                let parent = parent.unwrap();
                if !parent.is_function(t) {
                    let children = n.remove_children(t);
                    parent.add_children_after(t, children, Some(n));
                    n.detach(t);
                    t.report_code_change_at_node(parent);
                }
            }
            _ => {}
        }
    }
}

// port: ConvertToTypedInterface.PropagateConstJsdoc
struct PropagateConstJsdoc<'a> {
    current_file: &'a mut FileInfo,
}

impl ProcessConstJsdocCallback for PropagateConstJsdoc<'_> {
    fn current_file(&mut self) -> &mut FileInfo {
        self.current_file
    }

    // port: ConvertToTypedInterface.PropagateConstJsdoc#processConstWithRhs
    fn process_const_with_rhs(&mut self, t: &mut NodeTraversal<'_>, name_node: NodeId) {
        check_argument!(
            name_node.is_qualified_name(t)
                || name_node.is_string_key(t)
                || name_node.is_destructuring_lhs(t)
                || name_node.is_member_field_def(t),
            "{}",
            name_node.to_string(t)
        );
        let jsdoc_node = NodeUtil::get_best_jsdoc_info_node(t, name_node).unwrap();
        let original_jsdoc = jsdoc_node.get_jsdoc_info(t);
        let rhs = NodeUtil::get_r_value_of_l_value(t, name_node).unwrap();
        let mut new_jsdoc = JsdocUtil::get_jsdoc_for_rhs(t, rhs, original_jsdoc.as_deref());
        if new_jsdoc.is_none() && ClassUtil::is_this_prop_inside_class_with_name(t, name_node) {
            let scope = t.get_scope();
            let decl = ConvertToTypedInterface::find_name_declaration(t.get_compiler(), scope, rhs);
            new_jsdoc =
                JsdocUtil::get_jsdoc_for_name(t.get_compiler(), decl, original_jsdoc.as_deref());
        }
        if new_jsdoc.is_some() {
            jsdoc_node.set_jsdoc_info(t, new_jsdoc);
            t.report_code_change();
        }
    }
}

impl Callback for PropagateConstJsdoc<'_> {
    // port: ProcessConstJsdocCallback (NodeTraversal.AbstractPostOrderCallback#shouldTraverse)
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ProcessConstJsdocCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        ProcessConstJsdocCallback::visit(self, t, n, parent);
    }
}

/// Levels of JSDoc, starting from those most likely to be on the canonical declaration.
// port: ConvertToTypedInterface.SimplifyDeclarations.TypingLevel
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum TypingLevel {
    TypedJsdocDeclaration,
    UntypedJsdocDeclaration,
    NoJsdoc,
}

// port: ConvertToTypedInterface.SimplifyDeclarations
struct SimplifyDeclarations<'a> {
    current_file: &'a mut FileInfo,
}

impl<'a> SimplifyDeclarations<'a> {
    // port: ConvertToTypedInterface.SimplifyDeclarations#SimplifyDeclarations
    fn new(current_file: &'a mut FileInfo) -> Self {
        Self { current_file }
    }

    // port: ConvertToTypedInterface.SimplifyDeclarations#countDots
    fn count_dots(name: &JsString) -> i32 {
        let mut count = 0;
        for i in 0..name.length() {
            if name.char_at(i) == u16::from(b'.') {
                count += 1;
            }
        }
        count
    }

    // port: ConvertToTypedInterface.SimplifyDeclarations#DECLARATIONS_FIRST
    fn declarations_first_key(
        ast: &closure_rhino::node::Ast,
        decl: &PotentialDeclaration,
    ) -> TypingLevel {
        let Some(jsdoc) = decl.get_js_doc(ast) else {
            return TypingLevel::NoJsdoc;
        };
        if jsdoc.get_type_nodes().is_empty() {
            return TypingLevel::UntypedJsdocDeclaration;
        }
        TypingLevel::TypedJsdocDeclaration
    }

    // port: ConvertToTypedInterface.SimplifyDeclarations#removeDuplicateDeclarations
    fn remove_duplicate_declarations(&mut self, compiler: &mut AbstractCompiler) {
        let names: Vec<JsString> = self
            .current_file
            .get_declarations()
            .keys()
            .cloned()
            .collect();
        for name in names {
            if name.starts_with("this.") {
                continue;
            }
            let decl_list = self.current_file.get_declarations().get_mut(&name).unwrap();
            // Java's List.sort is stable, as is sort_by_cached_key.
            decl_list.sort_by_cached_key(|decl| Self::declarations_first_key(compiler, decl));
            while decl_list.len() > 1 {
                // Don't remove the first declaration (at index 0)
                let decl = decl_list.remove(1);
                decl.remove(compiler);
            }
        }
    }

    // port: ConvertToTypedInterface.SimplifyDeclarations#simplifyAll
    fn simplify_all(&mut self, compiler: &mut AbstractCompiler) {
        // Remove duplicate assignments to the same symbol
        self.remove_duplicate_declarations(compiler);

        // Simplify all names in the top-level scope.
        // ImmutableList.sortedCopyOf(SHORT_TO_LONG, ...) is a stable sort.
        let mut seen_names: Vec<JsString> = self
            .current_file
            .get_declarations()
            .keys()
            .cloned()
            .collect();
        seen_names.sort_by_cached_key(Self::count_dots);

        for name in seen_names {
            let decls = self.current_file.get_declarations()[&name].clone();
            for decl in &decls {
                self.process_declaration(compiler, &name, decl);
            }
        }
    }

    // port: ConvertToTypedInterface.SimplifyDeclarations#processDeclaration
    fn process_declaration(
        &mut self,
        compiler: &mut AbstractCompiler,
        name: &JsString,
        decl: &PotentialDeclaration,
    ) {
        if self.should_remove(compiler, name, decl) {
            decl.remove(compiler);
            return;
        }

        if decl.break_down_destructure(compiler) {
            ConvertToTypedInterface::maybe_report(
                compiler,
                decl.get_lhs(),
                &CONSTANT_WITHOUT_EXPLICIT_TYPE,
                &[],
            );
        }

        if let Some(rhs) = decl.get_rhs().filter(|rhs| rhs.is_function(compiler)) {
            Self::process_function(compiler, rhs);
        } else if let Some(rhs) = decl.get_rhs().filter(|rhs| Self::is_class(compiler, *rhs)) {
            Self::process_class(compiler, rhs);
        }
        Self::set_undeclared_to_unusable_type(compiler, decl);
        self.elide_private_member_type(compiler, decl);
        decl.simplify(compiler);
    }

    // port: ConvertToTypedInterface.SimplifyDeclarations#processClass
    fn process_class(compiler: &mut AbstractCompiler, n: NodeId) {
        check_argument!(Self::is_class(compiler, n));
        let mut member = n
            .get_last_child(compiler)
            .unwrap()
            .get_first_child(compiler);
        while let Some(m) = member {
            let next = m.get_next(compiler);
            match m.get_token(compiler) {
                Token::MEMBER_FIELD_DEF | Token::COMPUTED_FIELD_DEF => {
                    // MEMBER_FIELD_DEF's are handled in ProcessConstJsdocCallback which is
                    // called in the traversal by its subclass PropogateConstJsdoc.
                    // If no jsdoc is present on the MEMBER_FIELD_DEF, we will be add a Jsdoc in
                    // `setUndeclaredToUnusableType()`.
                    // No further simplification is needed for MEMBER_FIELD_DEF's when we call
                    // `simplifyAll()` so we will break.
                }
                Token::EMPTY => {
                    // a lonely `;` in a class body can just be deleted.
                    NodeUtil::delete_node(compiler, m);
                }
                Token::MEMBER_FUNCTION_DEF | Token::GETTER_DEF | Token::SETTER_DEF => {
                    let function = m.get_last_child(compiler).unwrap();
                    Self::process_function(compiler, function);
                }
                Token::COMPUTED_PROP => {
                    check_state!(
                        m.get_second_child(compiler).unwrap().is_function(compiler),
                        "Non-function computed class member: %s",
                        m.to_string(compiler)
                    );
                    let function = m.get_second_child(compiler).unwrap();
                    Self::process_function(compiler, function);
                }
                token => panic!("{token:?} should not be handled by processClass"),
            }
            member = next;
        }
    }

    // port: ConvertToTypedInterface.SimplifyDeclarations#processFunction
    fn process_function(compiler: &mut AbstractCompiler, n: NodeId) {
        check_argument!(n.is_function(compiler));
        let param_list = n.get_second_child(compiler).unwrap();
        Self::process_function_parameters(compiler, param_list);
    }

    // port: ConvertToTypedInterface.SimplifyDeclarations#processFunctionParameters
    fn process_function_parameters(compiler: &mut AbstractCompiler, param_list: NodeId) {
        check_argument!(param_list.is_param_list(compiler));
        let mut arg = param_list.get_first_child(compiler);
        while let Some(a) = arg {
            if a.is_default_value(compiler) {
                let rhs = a.get_last_child(compiler).unwrap();
                if rhs.is_call(compiler)
                    && GOOG_CALLER_LOCATION_QUALIFIED_NAME
                        .matches(compiler, rhs.get_first_child(compiler).unwrap())
                {
                    arg = a.get_next(compiler);
                    continue;
                }
                let undefined = NodeUtil::new_undefined_node(compiler, Some(rhs));
                rhs.replace_with(compiler, undefined);
                compiler.report_change_to_enclosing_scope(a);
            }
            arg = a.get_next(compiler);
        }
    }

    // port: ConvertToTypedInterface.SimplifyDeclarations#isClass
    fn is_class(ast: &closure_rhino::node::Ast, n: NodeId) -> bool {
        n.is_class(ast)
    }

    // port: ConvertToTypedInterface.SimplifyDeclarations#rootName
    fn root_name(qualified_name: &JsString) -> JsString {
        let dot_index = qualified_name.index_of(".");
        if dot_index == -1 {
            return qualified_name.clone();
        }
        qualified_name.substring(0, dot_index as usize)
    }

    // port: ConvertToTypedInterface.SimplifyDeclarations#shouldRemove
    fn should_remove(
        &mut self,
        compiler: &mut AbstractCompiler,
        name: &JsString,
        decl: &PotentialDeclaration,
    ) -> bool {
        if decl.is_detached(compiler) {
            return true;
        }
        if Self::root_name(name).starts_with("$jscomp") {
            // These are created by goog.scope processing, but clash with each other
            // and should not be depended on.
            if decl.get_rhs().is_some_and(|rhs| rhs.is_class(compiler))
                || decl
                    .get_js_doc(compiler)
                    .is_some_and(|jsdoc| jsdoc.contains_type_definition())
            {
                ConvertToTypedInterface::maybe_report(
                    compiler,
                    decl.get_lhs(),
                    &GOOG_SCOPE_HIDDEN_TYPE,
                    &[],
                );
            }
            return true;
        }
        // This looks like an update rather than a declaration in this file.
        !name.starts_with("this.")
            && !decl.is_definite_declaration(compiler)
            && !decl.get_lhs().is_member_field_def(compiler)
            && !self.current_file.is_prefix_provided(name)
            && !self.current_file.is_strict_prefix_declared(name)
    }

    // port: ConvertToTypedInterface.SimplifyDeclarations#setUndeclaredToUnusableType
    fn set_undeclared_to_unusable_type(
        compiler: &mut AbstractCompiler,
        decl: &PotentialDeclaration,
    ) {
        let name_node = decl.get_lhs();
        let jsdoc = decl.get_js_doc(compiler);
        if decl.should_preserve(compiler)
            || NodeUtil::is_namespace_decl(compiler, name_node)
            || decl
                .get_rhs()
                .is_some_and(|rhs| NodeUtil::is_call_to(compiler, rhs, "Symbol"))
            || (jsdoc
                .as_ref()
                .is_some_and(|jsdoc| jsdoc.contains_declaration())
                && !decl.is_const_to_be_inferred(compiler))
        {
            return;
        }
        ConvertToTypedInterface::maybe_warn_for_const_without_explicit_type(compiler, decl);
        let jsdoc_node = NodeUtil::get_best_jsdoc_info_node(compiler, name_node).unwrap();
        let new_jsdoc = JsdocUtil::get_unusable_type_jsdoc(compiler, jsdoc.as_deref());
        jsdoc_node.set_jsdoc_info(compiler, new_jsdoc);
    }

    // port: ConvertToTypedInterface.SimplifyDeclarations#elidePrivateMemberType
    fn elide_private_member_type(
        &self,
        compiler: &mut AbstractCompiler,
        decl: &PotentialDeclaration,
    ) {
        if self.should_elide_private_member_type(compiler, decl) {
            let jsdoc: Arc<JSDocInfo> = decl.get_js_doc(compiler).unwrap();
            let jsdoc_node = NodeUtil::get_best_jsdoc_info_node(compiler, decl.get_lhs()).unwrap();
            let new_jsdoc = JsdocUtil::replace_annotated_types_with_unusable_type(compiler, &jsdoc);
            jsdoc_node.set_jsdoc_info(compiler, new_jsdoc);
        }
    }

    // port: ConvertToTypedInterface.SimplifyDeclarations#shouldElidePrivateMemberType
    fn should_elide_private_member_type(
        &self,
        compiler: &AbstractCompiler,
        decl: &PotentialDeclaration,
    ) -> bool {
        // TODO: b/465855752 - roll out elison for JS files as well after depot cleanup.
        if !self.current_file.is_from_type_script() {
            return false;
        }
        let jsdoc = decl.get_js_doc(compiler);
        let Some(jsdoc) = jsdoc.filter(|jsdoc| jsdoc.get_visibility() == Visibility::PRIVATE)
        else {
            return false;
        };
        // Preserve JSDoc declaring a new type name, to avoid loosening types of other public
        // members.
        if jsdoc.is_constructor_or_interface()
            || jsdoc.has_enum_parameter_type()
            || jsdoc.has_typedef_type()
        {
            return false;
        }
        // Simple name declarations marked @private are not really treated as private if exported
        // from a goog.module. (Ultimately we should probably ban marking a local name @private so
        // that we can drop this special case.)
        if decl.get_lhs().is_name(compiler) {
            return false;
        }
        JsdocUtil::has_annotated_type(Some(&jsdoc))
    }
}

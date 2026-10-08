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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/ijs/ProcessConstJsdocCallback.java.

#![allow(clippy::collapsible_match, clippy::needless_return)] // Retain the Java switches and statement order.
//! Port of `com.google.javascript.jscomp.ijs.ProcessConstJsdocCallback`.
//!
//! A callback that calls the abstract method on every "inferrable const". This is a constant
//! declaration for which there is no declared type and an RHS is present. This is useful for
//! giving warnings like the CONSTANT_WITHOUT_EXPLICIT_TYPE diagnostic.
//!
//! As a side effect, this callback also populates the given FileInfo (assumed empty) with all of
//! the declarations found throughout the compilation.
//!
//! Java's abstract `AbstractPostOrderCallback` subclass is a trait: implementors supply the
//! `currentFile` field and `processConstWithRhs`, and forward `Callback#visit` to
//! [`ProcessConstJsdocCallback::visit`] (`shouldTraverse` returns true, as in
//! `AbstractPostOrderCallback`).

use crate::{
    ijs::{
        class_util::ClassUtil, file_info::FileInfo, potential_declaration::PotentialDeclaration,
    },
    node_traversal::NodeTraversal,
    node_util::NodeUtil,
};
use closure_rhino::{
    check_argument, check_state, node::NodeId, qualified_name::QualifiedName, token::Token,
};
use std::sync::LazyLock;

// port: ProcessConstJsdocCallback#GOOG_DEFINE
static GOOG_DEFINE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.define"));
// port: ProcessConstJsdocCallback#GOOG_PROVIDE
static GOOG_PROVIDE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.provide"));
// port: ProcessConstJsdocCallback#GOOG_REQUIRE
static GOOG_REQUIRE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.require"));
// port: ProcessConstJsdocCallback#CJS_REQUIRE
static CJS_REQUIRE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("require"));

pub trait ProcessConstJsdocCallback {
    /// Java's `private final FileInfo currentFile` field.
    fn current_file(&mut self) -> &mut FileInfo;

    // port: ProcessConstJsdocCallback#processConstWithRhs
    fn process_const_with_rhs(&mut self, t: &mut NodeTraversal<'_>, lhs: NodeId);

    // port: ProcessConstJsdocCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::CLASS => {
                if NodeUtil::is_statement_parent(t, parent.unwrap()) {
                    let name = n.get_first_child(t).unwrap();
                    self.current_file().record_name_declaration(t, name);
                }
            }
            Token::MEMBER_FIELD_DEF => {
                if NodeUtil::get_r_value_of_l_value(t, n).is_some() {
                    self.process_declaration_with_rhs(t, n);
                }
                self.current_file().record_member_field_def(t, n);
            }
            Token::FUNCTION => {
                if NodeUtil::is_statement_parent(t, parent.unwrap()) {
                    let name = n.get_first_child(t).unwrap();
                    self.current_file().record_name_declaration(t, name);
                } else if ClassUtil::is_class_method(t, n) && ClassUtil::has_named_class(t, n) {
                    self.current_file().record_method(t, n);
                }
            }
            Token::EXPR_RESULT => {
                let expr = n.get_first_child(t).unwrap();
                match expr.get_token(t) {
                    Token::CALL => {
                        let callee = expr.get_first_child(t).unwrap();
                        if GOOG_PROVIDE.matches(t, callee) {
                            let provided = expr.get_last_child(t).unwrap().get_string(t);
                            self.current_file().mark_provided(Some(provided));
                        } else if GOOG_REQUIRE.matches(t, callee) || CJS_REQUIRE.matches(t, callee)
                        {
                            let imported = expr.get_last_child(t).unwrap().get_string(t);
                            self.current_file().record_import(imported);
                        } else if GOOG_DEFINE.matches(t, callee) {
                            self.current_file().record_define(t, expr);
                        }
                    }
                    Token::ASSIGN => {
                        let lhs = expr.get_first_child(t).unwrap();
                        let rhs = expr.get_last_child(t);
                        self.record_declaration(t, lhs, rhs);
                    }
                    Token::GETPROP => {
                        self.current_file().record_name_declaration(t, expr);
                    }
                    Token::GETELEM => {}
                    _ => panic!("Unexpected declaration: {}", expr.to_string(t)),
                }
            }
            Token::VAR | Token::CONST | Token::LET => {
                check_state!(n.has_one_child(t), "{}", n.to_string(t));
                let lhs = n.get_first_child(t).unwrap();
                let rhs = lhs.get_last_child(t);
                self.record_declaration(t, lhs, rhs);
            }
            Token::STRING_KEY => {
                if parent.unwrap().is_object_lit(t) && n.has_one_child(t) {
                    self.process_declaration_with_rhs(t, n);
                    self.current_file().record_string_key_declaration(t, n);
                }
            }
            _ => {}
        }
    }

    // port: ProcessConstJsdocCallback#recordDeclaration
    fn record_declaration(&mut self, t: &mut NodeTraversal<'_>, lhs: NodeId, rhs: Option<NodeId>) {
        if let Some(rhs) = rhs.filter(|rhs| {
            rhs.is_call(t)
                && GOOG_DEFINE.matches(t, rhs.get_first_child(t).unwrap())
                && lhs.is_qualified_name(t)
        }) {
            self.current_file().record_define(t, rhs);
        } else if lhs.is_get_elem(t) {
            return;
        } else {
            self.record_name_declaration(t, lhs, rhs);
            if !lhs.is_destructuring_lhs(t) && rhs.is_some() {
                self.process_declaration_with_rhs(t, lhs);
            }
        }
    }

    // port: ProcessConstJsdocCallback#recordNameDeclaration
    fn record_name_declaration(
        &mut self,
        t: &mut NodeTraversal<'_>,
        lhs: NodeId,
        rhs: Option<NodeId>,
    ) {
        let lhs_parent = lhs.get_parent(t).unwrap();
        check_argument!(
            NodeUtil::is_name_declaration(t, Some(lhs_parent)) || lhs_parent.is_assign(t)
        );
        let is_import = PotentialDeclaration::is_import_rhs(t, rhs);
        let is_alias = PotentialDeclaration::is_alias_declaration(t, lhs, rhs);
        let mut names: Vec<NodeId> = Vec::new();
        NodeUtil::visit_lhs_nodes_in_node(t.get_compiler(), lhs_parent, &mut |_, name| {
            names.push(name)
        });
        // The consumer only records declarations (no AST change), so recording after the visit
        // keeps Java's order and effects.
        for name in names {
            if is_alias || is_import {
                self.current_file().record_alias_declaration(t, name);
            } else {
                self.current_file().record_name_declaration(t, name);
            }
        }
    }

    // port: ProcessConstJsdocCallback#processDeclarationWithRhs
    fn process_declaration_with_rhs(&mut self, t: &mut NodeTraversal<'_>, lhs: NodeId) {
        check_argument!(
            lhs.is_qualified_name(t) || lhs.is_string_key(t) || lhs.is_member_field_def(t),
            "{}",
            lhs.to_string(t)
        );
        check_state!(
            NodeUtil::get_r_value_of_l_value(t, lhs).is_some(),
            "{}",
            lhs.to_string(t)
        );
        if PotentialDeclaration::is_const_to_be_inferred_node(t, lhs) {
            self.process_const_with_rhs(t, lhs);
        }
    }
}

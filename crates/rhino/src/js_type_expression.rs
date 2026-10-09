/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Bob Jervis
 *   Google Inc.
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/rhino/JSTypeExpression.java.

//! Port of the JSDoc type AST wrapper.
use crate::fast_hash::IndexSet;
use crate::{
    js_string::JsString,
    node::{Ast, NodeId},
    simple_source_file::SimpleSourceFile,
    static_source_file::SourceKind,
    token::Token,
};
use std::sync::Arc;

pub const IMPLICIT_TEMPLATE_BOUND_SOURCE: &str = "<IMPLICIT_TEMPLATE_BOUND>";
#[derive(Debug)]
pub struct JSTypeExpression {
    root: NodeId,
    source_name: String,
}
impl JSTypeExpression {
    // port: JSTypeExpression#JSTypeExpression
    pub fn new(root: NodeId, source_name: impl Into<String>) -> Self {
        Self {
            root,
            source_name: source_name.into(),
        }
    }
    /// Rust-only: this expression with another root node (for `Ast::append_preparsed`).
    pub fn with_root(&self, root: NodeId) -> Self {
        Self {
            root,
            source_name: self.source_name.clone(),
        }
    }
    // port: JSTypeExpression#IMPLICIT_TEMPLATE_BOUND
    pub fn implicit_template_bound(ast: &mut Ast) -> Arc<Self> {
        if let Some(expr) = &ast.implicit_template_bound {
            let expr = expr.clone();
            if ast.preparse && ast.preparse_bound_first_use.is_none() {
                // Rust-only: a preparse arena's placeholder (see `Ast::new_for_preparse`).
                ast.preparse_bound_first_use = Some(ast.node_count());
            }
            return expr;
        }
        let root = ast.new_node(Token::QMARK);
        root.set_static_source_file(
            ast,
            Some(Arc::new(SimpleSourceFile::new(
                IMPLICIT_TEMPLATE_BOUND_SOURCE,
                SourceKind::STRONG,
            ))),
        );
        let expr = Arc::new(Self::new(root, IMPLICIT_TEMPLATE_BOUND_SOURCE));
        ast.implicit_template_bound = Some(expr.clone());
        expr
    }
    // port: JSTypeExpression#replaceNamesWithUnknownType
    pub fn replace_names_with_unknown_type(
        &self,
        ast: &mut Ast,
        names: &IndexSet<JsString>,
    ) -> Self {
        let old_expr_root = self.root.clone_tree(ast);
        let new_expr_root = Self::replace_names(ast, Some(old_expr_root), names).unwrap();
        Self::new(new_expr_root, self.source_name.clone())
    }
    // port: JSTypeExpression#replaceNames
    fn replace_names(
        ast: &mut Ast,
        n: Option<NodeId>,
        names: &IndexSet<JsString>,
    ) -> Option<NodeId> {
        let n = n?;
        let mut child = n.get_first_child(ast);
        while let Some(c) = child {
            let next = c.get_next(ast);
            Self::replace_names(ast, Some(c), names);
            child = next;
        }
        if n.is_string_lit(ast) && names.contains(&Self::base_name(n.get_string(ast))) {
            let q_mark = ast.new_node(Token::QMARK);
            if n.has_children(ast) && !n.get_first_child(ast).unwrap().is_block(ast) {
                let children = n.remove_children(ast);
                q_mark.add_children_to_back(ast, children);
            }
            if n.has_parent(ast) {
                n.replace_with(ast, q_mark);
            }
            return Some(q_mark);
        }
        Some(n)
    }
    // port: JSTypeExpression#baseName
    fn base_name(name: JsString) -> JsString {
        let dot_index = name.index_of_char(b'.' as u16);
        if dot_index == -1 {
            name
        } else {
            name.substring(0, dot_index as usize)
        }
    }
    // port: JSTypeExpression#getAllTypeNodes
    pub fn get_all_type_nodes(&self, ast: &Ast) -> Vec<NodeId> {
        let mut builder = Vec::new();
        Self::visit_all_type_nodes(ast, Some(self.root), &mut |n| builder.push(n));
        builder
    }
    // port: JSTypeExpression#getAllTypeNames
    pub fn get_all_type_names(&self, ast: &Ast) -> IndexSet<JsString> {
        let mut builder = IndexSet::<_>::default();
        Self::visit_all_type_nodes(ast, Some(self.root), &mut |n| {
            builder.insert(n.get_string(ast));
        });
        builder
    }
    // port: JSTypeExpression#visitAllTypeNodes
    fn visit_all_type_nodes(ast: &Ast, n: Option<NodeId>, visitor: &mut impl FnMut(NodeId)) {
        let Some(n) = n else {
            return;
        };
        for child in n.children(ast) {
            Self::visit_all_type_nodes(ast, Some(child), visitor);
        }
        if n.is_string_lit(ast) {
            visitor(n);
        }
    }
    // port: JSTypeExpression#makeOptionalArg
    pub fn make_optional_arg(ast: &mut Ast, expr: Arc<Self>) -> Arc<Self> {
        if expr.is_optional_arg(ast) || expr.is_var_args(ast) {
            expr
        } else {
            let equals = ast.new_node_with_child(Token::EQUALS, expr.root);
            equals.clone_props_from(ast, expr.root);
            Arc::new(Self::new(equals, expr.source_name.clone()))
        }
    }
    // port: JSTypeExpression#makeVarArgs
    pub fn make_var_args(ast: &mut Ast, expr: Arc<Self>) -> Arc<Self> {
        if expr.is_optional_arg(ast) || expr.is_var_args(ast) {
            expr
        } else {
            let equals = ast.new_node_with_child(Token::ITER_REST, expr.root);
            equals.clone_props_from(ast, expr.root);
            Arc::new(Self::new(equals, expr.source_name.clone()))
        }
    }
    // port: JSTypeExpression#isOptionalArg
    pub fn is_optional_arg(&self, ast: &Ast) -> bool {
        self.root.get_token(ast) == Token::EQUALS
    }
    // port: JSTypeExpression#isVarArgs
    pub fn is_var_args(&self, ast: &Ast) -> bool {
        self.root.get_token(ast) == Token::ITER_REST
    }
    // pending: JSTypeExpression#evaluate requires JSTypeRegistry
    // port: JSTypeExpression#isEquivalentTo
    pub fn is_equivalent_to(&self, ast: &Ast, other: Option<&Self>) -> bool {
        self.is_equivalent_to_across(ast, ast, other)
    }
    // port: JSTypeExpression#isEquivalentTo (separate owning arenas)
    pub fn is_equivalent_to_across(&self, ast: &Ast, ast_b: &Ast, other: Option<&Self>) -> bool {
        other.is_some_and(|o| self.root.is_equivalent_to_across(ast, ast_b, o.root))
    }
    // port: JSTypeExpression#getRoot
    pub fn get_root(&self) -> NodeId {
        self.root
    }
    // port: JSTypeExpression#getSourceName
    pub fn get_source_name(&self) -> &str {
        &self.source_name
    }
    // port: JSTypeExpression#toString
    pub fn to_string(&self, ast: &Ast) -> String {
        crate::java_lang::charset::utf8_encoded_text(self.to_string_utf16(ast).as_units())
    }
    // port: JSTypeExpression#toString
    pub fn to_string_utf16(&self, ast: &Ast) -> JsString {
        self.to_string_with_types_utf16(ast, &mut crate::node::no_registry_jstype_printer)
    }
    // port: JSTypeExpression#toString
    /// `toString` with a registry-aware JSType printer (see [`crate::node::JSTypePrinter`]).
    pub fn to_string_with_types_utf16(
        &self,
        ast: &Ast,
        type_printer: &mut crate::node::JSTypePrinter<'_>,
    ) -> JsString {
        JsString::from("type: ")
            .concat(&self.root.to_string_tree_with_types_utf16(ast, type_printer))
    }
    // port: JSTypeExpression#copy
    pub fn copy(&self, ast: &mut Ast) -> Self {
        Self::new(self.root.clone_tree(ast), self.source_name.clone())
    }
    // port: JSTypeExpression#isExplicitUnknownTemplateBound
    pub fn is_explicit_unknown_template_bound(&self, ast: &Ast) -> bool {
        self.root.get_token(ast) == Token::QMARK
            && !self.root.has_children(ast)
            && self.source_name != IMPLICIT_TEMPLATE_BOUND_SOURCE
    }
    // port: JSTypeExpression#getRecordPropertyNames
    pub fn get_record_property_names(&self, ast: &Ast) -> IndexSet<JsString> {
        let mut builder = IndexSet::<_>::default();
        Self::get_record_property_names_recursive(ast, Some(self.root), &mut builder);
        builder
    }
    // port: JSTypeExpression#getRecordPropertyNamesRecursive
    fn get_record_property_names_recursive(
        ast: &Ast,
        n: Option<NodeId>,
        names: &mut IndexSet<JsString>,
    ) {
        let Some(n) = n else {
            return;
        };
        for child in n.children(ast) {
            Self::get_record_property_names_recursive(ast, Some(child), names);
        }
        if n.is_string_key(ast) {
            names.insert(n.get_string(ast));
        }
    }
}

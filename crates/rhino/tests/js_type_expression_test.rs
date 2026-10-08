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
 *   Nick Santos
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/rhino/JSTypeExpressionTest.java.

use closure_rhino::{js_type_expression::JSTypeExpression, node::Ast, token::Token};
use std::sync::Arc;

// port: JSTypeExpressionTest#getTestExpression
fn get_test_expression(ast: &mut Ast) -> JSTypeExpression {
    let a = ast.new_string("foo.Bar");
    let b = ast.new_string("string");
    let c = ast.new_node(Token::PIPE);
    c.add_child_to_back(ast, a);
    c.add_child_to_back(ast, b);
    let d = ast.new_string("Object");
    let e = ast.new_string("string");
    let f = ast.new_node(Token::PIPE);
    f.add_child_to_back(ast, c);
    f.add_child_to_back(ast, d);
    f.add_child_to_back(ast, e);
    JSTypeExpression::new(f, "")
}
// port: JSTypeExpressionTest#testGetAllTypeNames
#[test]
fn test_get_all_type_names() {
    let mut ast = Ast::new();
    let expr = get_test_expression(&mut ast);
    let mut names: Vec<_> = expr.get_all_type_names(&ast).into_iter().collect();
    names.sort();
    assert_eq!(names, vec!["Object", "foo.Bar", "string"]);
}
// port: JSTypeExpressionTest#testGetAllTypeNodes
#[test]
fn test_get_all_type_nodes() {
    let mut ast = Ast::new();
    let expr = get_test_expression(&mut ast);
    let mut names: Vec<_> = expr
        .get_all_type_nodes(&ast)
        .into_iter()
        .map(|n| n.get_string(&ast))
        .collect();
    names.sort();
    assert_eq!(names, vec!["Object", "foo.Bar", "string", "string"]);
}
// port: JSTypeExpressionTest#testIsExplicitUnknownTemplateBound
#[test]
fn test_is_explicit_unknown_template_bound() {
    let mut ast = Ast::new();
    let root = ast.new_node(Token::QMARK);
    assert!(JSTypeExpression::new(root, "").is_explicit_unknown_template_bound(&ast));
    let implicit = JSTypeExpression::implicit_template_bound(&mut ast);
    let copy = Arc::new(implicit.copy(&mut ast));
    assert!(!Arc::ptr_eq(&copy, &implicit));
    assert!(!implicit.is_explicit_unknown_template_bound(&ast));
    assert!(!copy.is_explicit_unknown_template_bound(&ast));
    let root = ast.new_node(Token::STAR);
    assert!(!JSTypeExpression::new(root, "").is_explicit_unknown_template_bound(&ast));
    let child = ast.new_string("number");
    let root = ast.new_node_with_child(Token::QMARK, child);
    assert!(!JSTypeExpression::new(root, "").is_explicit_unknown_template_bound(&ast));
}

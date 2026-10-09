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
 *   John Lenz
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
//   test/com/google/javascript/rhino/IRTest.java.

use closure_rhino::{ir::IR, node::Ast, token::Token};
use std::panic::{AssertUnwindSafe, catch_unwind};
// port: IRTest#testEmpty
#[test]
fn test_empty() {
    let mut ast = Ast::new();
    let v1 = IR::empty(&mut ast);
    assert_eq!(v1.to_string_tree(&ast), "EMPTY\n");
}

// port: IRTest#testNewTarget
#[test]
fn test_new_target() {
    let mut ast = Ast::new();
    let v1 = IR::new_target(&mut ast);
    assert_eq!(v1.to_string_tree(&ast), "NEW_TARGET\n");
}

// port: IRTest#testFunction
#[test]
fn test_function() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "hi");
    let v2 = IR::param_list(&mut ast, &[]);
    let v3 = IR::block(&mut ast);
    let v4 = IR::function(&mut ast, v1, v2, v3);
    assert_eq!(
        v4.to_string_tree(&ast),
        "FUNCTION hi\n    NAME hi\n    PARAM_LIST\n    BLOCK\n"
    );
}

// port: IRTest#testArrowFunction
#[test]
fn test_arrow_function() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "hi");
    let v2 = IR::param_list(&mut ast, &[]);
    let v3 = IR::block(&mut ast);
    let v4 = IR::arrow_function(&mut ast, v1, v2, v3);
    assert_eq!(
        v4.to_string_tree(&ast),
        "FUNCTION hi [arrow_fn: 1]\n    NAME hi\n    PARAM_LIST\n    BLOCK\n"
    );
}

// port: IRTest#testParamList
#[test]
fn test_param_list() {
    let mut ast = Ast::new();
    let v1 = IR::param_list(&mut ast, &[]);
    assert_eq!(v1.to_string_tree(&ast), "PARAM_LIST\n");
    let v2 = IR::name(&mut ast, "a");
    let v3 = IR::name(&mut ast, "b");
    let v4 = IR::param_list(&mut ast, &[v2, v3]);
    assert_eq!(
        v4.to_string_tree(&ast),
        "PARAM_LIST\n    NAME a\n    NAME b\n"
    );
}

// port: IRTest#testBlock
#[test]
fn test_block() {
    let mut ast = Ast::new();
    let v1 = IR::block(&mut ast);
    assert_eq!(v1.to_string_tree(&ast), "BLOCK\n");
    let v2 = IR::empty(&mut ast);
    let v3 = IR::empty(&mut ast);
    let v4 = IR::block_with_children(&mut ast, &[v2, v3]);
    assert_eq!(v4.to_string_tree(&ast), "BLOCK\n    EMPTY\n    EMPTY\n");
    let v5 = IR::empty(&mut ast);
    let v6 = IR::empty(&mut ast);
    let v7 = vec![v5, v6];
    let v8 = IR::block_with_list(&mut ast, &v7);
    assert_eq!(v8.to_string_tree(&ast), "BLOCK\n    EMPTY\n    EMPTY\n");
}

// port: IRTest#testScript
#[test]
fn test_script() {
    let mut ast = Ast::new();
    let v1 = IR::script(&mut ast);
    assert_eq!(v1.to_string_tree(&ast), "SCRIPT\n");
    let v2 = IR::empty(&mut ast);
    let v3 = IR::empty(&mut ast);
    let v4 = IR::script_with_children(&mut ast, &[v2, v3]);
    assert_eq!(v4.to_string_tree(&ast), "SCRIPT\n    EMPTY\n    EMPTY\n");
    let v5 = IR::empty(&mut ast);
    let v6 = IR::empty(&mut ast);
    let v7 = vec![v5, v6];
    let v8 = IR::script_with_list(&mut ast, &v7);
    assert_eq!(v8.to_string_tree(&ast), "SCRIPT\n    EMPTY\n    EMPTY\n");
}

// port: IRTest#testScriptThrows
#[test]
fn test_script_throws() {
    let mut ast = Ast::new();
    let caught = catch_unwind(AssertUnwindSafe(|| {
        let ret = IR::return_node(&mut ast);
        IR::script_with_children(&mut ast, &[ret]);
    }))
    .is_err();
    assert!(caught, "expected exception was not seen");
}

// port: IRTest#testVar
#[test]
fn test_var() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "a");
    let v2 = IR::var(&mut ast, v1);
    assert_eq!(v2.to_string_tree(&ast), "VAR\n    NAME a\n");
    let v3 = IR::name(&mut ast, "a");
    let v4 = IR::true_node(&mut ast);
    let v5 = IR::var_with_value(&mut ast, v3, v4);
    assert_eq!(v5.to_string_tree(&ast), "VAR\n    NAME a\n        TRUE\n");
}

// port: IRTest#testReturn
#[test]
fn test_return() {
    let mut ast = Ast::new();
    let v1 = IR::return_node(&mut ast);
    assert_eq!(v1.to_string_tree(&ast), "RETURN\n");
    let v2 = IR::name(&mut ast, "a");
    let v3 = IR::return_node_with_expression(&mut ast, v2);
    assert_eq!(v3.to_string_tree(&ast), "RETURN\n    NAME a\n");
}

// port: IRTest#testThrow
#[test]
fn test_throw() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "a");
    let v2 = IR::throw_node(&mut ast, v1);
    assert_eq!(v2.to_string_tree(&ast), "THROW\n    NAME a\n");
}

// port: IRTest#testExprResult
#[test]
fn test_expr_result() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "a");
    let v2 = IR::expr_result(&mut ast, v1);
    assert_eq!(v2.to_string_tree(&ast), "EXPR_RESULT\n    NAME a\n");
}

// port: IRTest#testIf
#[test]
fn test_if() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "a");
    let v2 = IR::block(&mut ast);
    let v3 = IR::if_node(&mut ast, v1, v2);
    assert_eq!(v3.to_string_tree(&ast), "IF\n    NAME a\n    BLOCK\n");
    let v4 = IR::name(&mut ast, "a");
    let v5 = IR::block(&mut ast);
    let v6 = IR::block(&mut ast);
    let v7 = IR::if_node_with_else(&mut ast, v4, v5, v6);
    assert_eq!(
        v7.to_string_tree(&ast),
        "IF\n    NAME a\n    BLOCK\n    BLOCK\n"
    );
}

// port: IRTest#testIssue727_1
#[test]
fn test_issue727_1() {
    let mut ast = Ast::new();
    let v1 = IR::block(&mut ast);
    let v2 = IR::block(&mut ast);
    let v3 = IR::try_finally(&mut ast, v1, v2);
    assert_eq!(
        v3.to_string_tree(&ast),
        "TRY\n    BLOCK\n    BLOCK\n    BLOCK\n"
    );
}

// port: IRTest#testIssue727_2
#[test]
fn test_issue727_2() {
    let mut ast = Ast::new();
    let v1 = IR::block(&mut ast);
    let v2 = IR::name(&mut ast, "e");
    let v3 = IR::block(&mut ast);
    let v4 = IR::catch_node(&mut ast, v2, v3);
    let v5 = IR::try_catch(&mut ast, v1, v4);
    assert_eq!(
        v5.to_string_tree(&ast),
        "TRY\n    BLOCK\n    BLOCK\n        CATCH\n            NAME e\n            BLOCK\n"
    );
}

// port: IRTest#testIssue727_3
#[test]
fn test_issue727_3() {
    let mut ast = Ast::new();
    let v1 = IR::block(&mut ast);
    let v2 = IR::name(&mut ast, "e");
    let v3 = IR::block(&mut ast);
    let v4 = IR::catch_node(&mut ast, v2, v3);
    let v5 = IR::block(&mut ast);
    let v6 = IR::try_catch_finally(&mut ast, v1, v4, v5);
    assert_eq!(
        v6.to_string_tree(&ast),
        "TRY\n    BLOCK\n    BLOCK\n        CATCH\n            NAME e\n            BLOCK\n    BLOCK\n"
    );
}

// port: IRTest#testAdd
#[test]
fn test_add() {
    let mut ast = Ast::new();
    let v1 = IR::number(&mut ast, 1.0);
    let v2 = IR::cast(&mut ast, v1, None);
    let v3 = IR::number(&mut ast, 2.0);
    let v4 = IR::add(&mut ast, v2, v3);
    assert_eq!(
        v4.to_string_tree(&ast),
        "ADD\n    CAST\n        NUMBER 1.0\n    NUMBER 2.0\n"
    );
}

// port: IRTest#testVarWithTemplateLitOnRHS
#[test]
fn test_var_with_template_lit_on_rhs() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "x");
    let v2 = IR::string(&mut ast, "");
    let v3 = ast.new_node_with_child(Token::TEMPLATELIT, v2);
    let v4 = IR::var_with_value(&mut ast, v1, v3);
    let v5 = [
        "VAR",
        "    NAME x",
        "        TEMPLATELIT",
        "            STRINGLIT ",
        "",
    ]
    .join("\n");
    assert_eq!(v4.to_string_tree(&ast), v5);
    let v6 = IR::name(&mut ast, "x");
    let v7 = IR::name(&mut ast, "y");
    let v8 = IR::string(&mut ast, "");
    let v9 = ast.new_node_with_children2(Token::TAGGED_TEMPLATELIT, v7, v8);
    let v10 = IR::var_with_value(&mut ast, v6, v9);
    let v11 = [
        "VAR",
        "    NAME x",
        "        TAGGED_TEMPLATELIT",
        "            NAME y",
        "            STRINGLIT ",
        "",
    ]
    .join("\n");
    assert_eq!(v10.to_string_tree(&ast), v11);
}

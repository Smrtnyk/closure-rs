/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/AstManipulationsTest.java.

//! Port of `AstManipulationsTest.java`.

use closure_jscomp::ast_manipulations::AstManipulations;
use closure_rhino::{
    ir::IR,
    node::{Ast, NodeId},
    token::Token,
};

fn assert_node_equal(ast: &Ast, actual: NodeId, expected: NodeId) {
    assert!(
        actual.is_equivalent_to(ast, expected),
        "expected:\n{}\nactual:\n{}",
        expected.to_string_tree(ast),
        actual.to_string_tree(ast)
    );
}

// port: AstManipulationsTest#fuseExpressions_fusesTwoNumbers
#[test]
fn fuse_expressions_fuses_two_numbers() {
    let mut ast = Ast::new();
    let one = IR::number(&mut ast, 1.0);
    let two = IR::number(&mut ast, 2.0);

    let actual = AstManipulations::fuse_expressions(&mut ast, one, two);
    let one_clone = one.clone_node(&mut ast);
    let two_clone = two.clone_node(&mut ast);
    let expected = IR::comma(&mut ast, one_clone, two_clone);
    assert_node_equal(&ast, actual, expected);
}

// port: AstManipulationsTest#fuseExpressions_dropsEmptySecondExpression
#[test]
fn fuse_expressions_drops_empty_second_expression() {
    let mut ast = Ast::new();
    let one = IR::number(&mut ast, 1.0);
    let empty = IR::empty(&mut ast);

    let actual = AstManipulations::fuse_expressions(&mut ast, one, empty);
    let expected = one.clone_node(&mut ast);
    assert_node_equal(&ast, actual, expected);
}

// port: AstManipulationsTest#fuseExpressions_keepsEmptyFirstExpression
#[test]
fn fuse_expressions_keeps_empty_first_expression() {
    let mut ast = Ast::new();
    let empty = IR::empty(&mut ast);
    let one = IR::number(&mut ast, 1.0);

    // Note: it should be fine to return one instead of this comma
    let actual = AstManipulations::fuse_expressions(&mut ast, empty, one);
    let empty_clone = empty.clone_node(&mut ast);
    let one_clone = one.clone_node(&mut ast);
    let expected = ast.new_node_with_children2(Token::COMMA, empty_clone, one_clone);
    assert_node_equal(&ast, actual, expected);
}

// port: AstManipulationsTest#fuseExpressions_fusesNumberIntoComma
#[test]
fn fuse_expressions_fuses_number_into_comma() {
    let mut ast = Ast::new();
    let zero = IR::number(&mut ast, 0.0);
    let one = IR::number(&mut ast, 1.0);
    let zero_comma_one = IR::comma(&mut ast, zero, one);
    let two = IR::number(&mut ast, 2.0);

    let actual = AstManipulations::fuse_expressions(&mut ast, zero_comma_one, two);
    let zero_comma_one_clone = zero_comma_one.clone_tree(&mut ast);
    let two_clone = two.clone_node(&mut ast);
    let expected = IR::comma(&mut ast, zero_comma_one_clone, two_clone);
    assert_node_equal(&ast, actual, expected);
}

// port: AstManipulationsTest#fuseExpressions_fusesCommaIntoNumber
#[test]
fn fuse_expressions_fuses_comma_into_number() {
    let mut ast = Ast::new();
    let zero = IR::number(&mut ast, 0.0);
    let one = IR::number(&mut ast, 1.0);
    let two = IR::number(&mut ast, 2.0);
    let one_comma_two = IR::comma(&mut ast, one, two);

    // Utility keeps commas in the left branch of the comma tree, i.e.
    //  COMMA
    //    COMMA
    //      NUMBER 0
    //      NUMBER 1
    //    NUMBER 2
    // instead of this:
    //  COMMA
    //    NUMBER 0
    //    COMMA
    //      NUMBER 1
    //      NUMBER 2
    // to make unit-testing passes that use this utility easier. Writing 'expected' JS code
    // for the latter tree requires more parentheses.
    let actual = AstManipulations::fuse_expressions(&mut ast, zero, one_comma_two);
    let zero_clone = zero.clone_node(&mut ast);
    let one_clone = one.clone_node(&mut ast);
    let inner = IR::comma(&mut ast, zero_clone, one_clone);
    let two_clone = two.clone_node(&mut ast);
    let expected = IR::comma(&mut ast, inner, two_clone);
    assert_node_equal(&ast, actual, expected);
}

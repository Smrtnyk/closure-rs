/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/NodeIteratorsTest.java.

//! Port of `NodeIteratorsTest.java`.
//!
//! In each test, we find the declaration of "X" in the local scope, construct a list of all
//! nodes where X is guaranteed to retain its original value, and compare those nodes against an
//! expected list of tokens.
use closure_jscomp::compiler::Compiler;
use closure_jscomp::node_iterators::{FunctionlessLocalScope, LocalVarMotion};
use closure_jscomp::node_util::NodeUtil;
use closure_rhino::token::Token;

// port: NodeIteratorsTest#testBasic
#[test]
fn test_basic() {
    test_var_motion_with_code("var X = 3;", &[Token::VAR, Token::SCRIPT]);
}

// port: NodeIteratorsTest#testNamedFunction
#[test]
fn test_named_function() {
    test_var_motion_with_code("var X = 3; function f() {}", &[Token::VAR, Token::SCRIPT]);
}

// port: NodeIteratorsTest#testNamedFunction2
#[test]
fn test_named_function2() {
    test_var_motion_with_code(
        "var X = 3; function f() {} var Y;",
        &[Token::VAR, Token::NAME, Token::VAR, Token::SCRIPT],
    );
}

// port: NodeIteratorsTest#testFunctionExpression
#[test]
fn test_function_expression() {
    test_var_motion_with_code(
        "var X = 3, Y = function() {}; 3;",
        &[
            Token::NAME,
            Token::VAR,
            Token::NUMBER,
            Token::EXPR_RESULT,
            Token::SCRIPT,
        ],
    );
}

// port: NodeIteratorsTest#testFunctionExpression2
#[test]
fn test_function_expression2() {
    test_var_motion_with_code(
        "var X = 3; var Y = function() {}; 3;",
        &[
            Token::VAR,
            Token::NAME,
            Token::VAR,
            Token::NUMBER,
            Token::EXPR_RESULT,
            Token::SCRIPT,
        ],
    );
}

// port: NodeIteratorsTest#testHaltAtVarRef
#[test]
fn test_halt_at_var_ref() {
    test_var_motion_with_code(
        "var X, Y = 3; var Z = X;",
        &[Token::NUMBER, Token::NAME, Token::VAR, Token::NAME],
    );
}

// port: NodeIteratorsTest#testHaltAtVarRef2
#[test]
fn test_halt_at_var_ref2() {
    test_var_motion_with_code(
        "var X, Y = 3; (function() {})(3, X);",
        &[
            Token::NUMBER,
            Token::NAME,
            Token::VAR,
            Token::NUMBER,
            Token::NAME,
        ],
    );
}

// port: NodeIteratorsTest#testHaltAtVarRef3
#[test]
fn test_halt_at_var_ref3() {
    test_var_motion_with_code(
        "var X, Y = 3; X;",
        &[Token::NUMBER, Token::NAME, Token::VAR, Token::NAME],
    );
}

// port: NodeIteratorsTest#testHaltAtSideEffects
#[test]
fn test_halt_at_side_effects() {
    test_var_motion_with_code(
        "var X, Y = 3; var Z = B(3);",
        &[
            Token::NUMBER,
            Token::NAME,
            Token::VAR,
            Token::NAME,
            Token::NUMBER,
        ],
    );
}

// port: NodeIteratorsTest#testHaltAtSideEffects2
#[test]
fn test_halt_at_side_effects2() {
    test_var_motion_with_code(
        "var A = 1, X = A, Y = 3; delete A;",
        &[Token::NUMBER, Token::NAME, Token::VAR, Token::NAME],
    );
}

// port: NodeIteratorsTest#testHaltAtSideEffects3
#[test]
fn test_halt_at_side_effects3() {
    test_var_motion_with_code(
        "var A = 1, X = A, Y = 3; A++;",
        &[Token::NUMBER, Token::NAME, Token::VAR, Token::NAME],
    );
}

// port: NodeIteratorsTest#testHaltAtSideEffects4
#[test]
fn test_halt_at_side_effects4() {
    test_var_motion_with_code(
        "var A = 1, X = A, Y = 3; A--;",
        &[Token::NUMBER, Token::NAME, Token::VAR, Token::NAME],
    );
}

// port: NodeIteratorsTest#testHaltAtSideEffects5
#[test]
fn test_halt_at_side_effects5() {
    test_var_motion_with_code(
        "var A = 1, X = A, Y = 3; A = 'a';",
        &[
            Token::NUMBER,
            Token::NAME,
            Token::VAR,
            Token::NAME,
            Token::STRINGLIT,
        ],
    );
}

// port: NodeIteratorsTest#testNoHaltReadWhenValueIsImmutable
#[test]
fn test_no_halt_read_when_value_is_immutable() {
    test_var_motion_with_code(
        "var X = 1, Y = 3; alert();",
        &[Token::NUMBER, Token::NAME, Token::VAR, Token::NAME],
    );
}

// port: NodeIteratorsTest#testHaltReadWhenValueHasSideEffects
#[test]
fn test_halt_read_when_value_has_side_effects() {
    test_var_motion_with_code(
        "var X = f(), Y = 3; alert();",
        &[Token::NUMBER, Token::NAME, Token::VAR],
    );
}

// port: NodeIteratorsTest#testCatchBlock
#[test]
fn test_catch_block() {
    test_var_motion_with_code(
        "var X = 1; try { 4; } catch (X) {}",
        &[Token::VAR, Token::NUMBER, Token::EXPR_RESULT, Token::BLOCK],
    );
}

// port: NodeIteratorsTest#testIfBranch
#[test]
fn test_if_branch() {
    test_var_motion_with_code("var X = foo(); if (X) {}", &[Token::VAR, Token::NAME]);
}

// port: NodeIteratorsTest#testLet
#[test]
fn test_let() {
    test_var_motion_with_code("let X = foo(); if (X) {}", &[Token::LET, Token::NAME]);
}

// port: NodeIteratorsTest#testConst
#[test]
fn test_const() {
    test_var_motion_with_code("const X = foo(); if (X) {}", &[Token::CONST, Token::NAME]);
}

// port: NodeIteratorsTest#testVarMotionWithCode
fn test_var_motion_with_code(code: &str, expected_tokens: &[Token]) {
    let mut ancestors = Vec::new();

    // Add an empty node to the beginning of the code and start there.
    let mut compiler = Compiler::new();
    let root = compiler.parse_test_code(format!(";{code}"));
    let mut n = Some(root);
    while let Some(node) = n {
        ancestors.insert(0, node);
        n = node.get_first_child(&compiler);
    }

    let mut search_it = FunctionlessLocalScope::new(&compiler, &ancestors);
    let mut found = false;
    while search_it.has_next(&compiler) {
        let n = search_it.next(&compiler);
        if n.is_name(&compiler)
            && NodeUtil::is_name_declaration(&compiler, search_it.current_parent(&compiler))
            && n.get_string(&compiler) == "X"
        {
            found = true;
            break;
        }
    }

    assert!(
        found,
        "Variable X not found! {}",
        root.to_string_tree(&compiler)
    );

    let current_ancestors = search_it.current_ancestors();
    assert!(current_ancestors.len() >= 3);

    let mut move_it = LocalVarMotion::for_var(
        &mut compiler,
        current_ancestors[0],
        current_ancestors[1],
        current_ancestors[2],
    );
    let mut actual_tokens = Vec::new();
    while move_it.has_next() {
        let next = move_it.next(&mut compiler).unwrap();
        actual_tokens.push(next.get_token(&compiler));
    }

    assert_eq!(actual_tokens, expected_tokens);
}

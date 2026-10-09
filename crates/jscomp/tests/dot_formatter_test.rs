/*
 * Copyright 2007 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/DotFormatterTest.java.

use closure_jscomp::{Compiler, dot_formatter::DotFormatter};
use closure_rhino::token::Token;

// port: DotFormatterTest#testKeyAssignementSequential
#[test]
fn test_key_assignement_sequential() {
    let mut compiler = Compiler::new();
    let mut dot = DotFormatter::new_instance_for_testing();
    assert_eq!(
        {
            let node = compiler.new_node(Token::BLOCK);
            dot.key(&mut compiler, node)
        },
        0
    );
    assert_eq!(
        {
            let node = compiler.new_node(Token::BLOCK);
            dot.key(&mut compiler, node)
        },
        1
    );
    assert_eq!(
        {
            let node = compiler.new_node(Token::BLOCK);
            dot.key(&mut compiler, node)
        },
        2
    );
    assert_eq!(
        {
            let node = compiler.new_node(Token::BLOCK);
            dot.key(&mut compiler, node)
        },
        3
    );
    assert_eq!(
        {
            let node = compiler.new_node(Token::BLOCK);
            dot.key(&mut compiler, node)
        },
        4
    );
}

// port: DotFormatterTest#testKeyAssignementOncePerNode
#[test]
fn test_key_assignement_once_per_node() {
    let mut compiler = Compiler::new();
    let mut dot = DotFormatter::new_instance_for_testing();
    let node0 = compiler.new_node(Token::BLOCK);
    let node1 = compiler.new_node(Token::BLOCK);
    let node2 = compiler.new_node(Token::BLOCK);

    assert_eq!(dot.key(&mut compiler, node0), 0);
    assert_eq!(dot.key(&mut compiler, node1), 1);
    assert_eq!(dot.key(&mut compiler, node2), 2);
    assert_eq!(dot.key(&mut compiler, node0), 0);
    assert_eq!(dot.key(&mut compiler, node1), 1);
    assert_eq!(dot.key(&mut compiler, node2), 2);
}

// port: DotFormatterTest#testToDotSimple
#[test]
fn test_to_dot_simple() {
    let mut compiler = Compiler::new();
    let ast = compiler.new_node(Token::BITOR);

    let expected = r#"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="BITOR"];
}
"#;
    test(&mut compiler, expected, ast);
}

// port: DotFormatterTest#testToDotSimpleName
#[test]
fn test_to_dot_simple_name() {
    let mut compiler = Compiler::new();
    let ast = compiler.new_string_with_token(Token::NAME, "dummy");

    let expected = r#"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="NAME(dummy)"];
}
"#;
    test(&mut compiler, expected, ast);
}

// port: DotFormatterTest#testToDotSimpleStringKey
#[test]
fn test_to_dot_simple_string_key() {
    let mut compiler = Compiler::new();
    let ast = compiler.new_string_with_token(Token::STRING_KEY, "key");

    let expected = r#"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="STRING_KEY(key)"];
}
"#;
    test(&mut compiler, expected, ast);
}

// port: DotFormatterTest#testToDot3Elements_nodesWithoutNames
#[test]
fn test_to_dot3_elements_nodes_without_names() {
    let mut compiler = Compiler::new();
    let ast = compiler.new_node(Token::BLOCK);
    let child = compiler.new_string_with_token(Token::NAME, "");
    ast.add_child_to_back(&mut compiler, child);
    let child = compiler.new_string_with_token(Token::STRINGLIT, "");
    ast.add_child_to_back(&mut compiler, child);

    let expected = r#"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="BLOCK"];
  node1 [label="NAME"];
  node0 -> node1 [weight=1];
  node2 [label="STRINGLIT"];
  node0 -> node2 [weight=1];
}
"#;
    test(&mut compiler, expected, ast);
}

// port: DotFormatterTest#testToDot3Elements_NodesCreatedWithNewString
#[test]
fn test_to_dot3_elements_nodes_created_with_new_string() {
    let mut compiler = Compiler::new();
    let ast = compiler.new_node(Token::BLOCK);
    let child = compiler.new_string_with_token(Token::NAME, "a");
    ast.add_child_to_back(&mut compiler, child);
    let child = compiler.new_string_with_token(Token::STRINGLIT, "b");
    ast.add_child_to_back(&mut compiler, child);

    let expected = r#"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="BLOCK"];
  node1 [label="NAME(a)"];
  node0 -> node1 [weight=1];
  node2 [label="STRINGLIT(b)"];
  node0 -> node2 [weight=1];
}
"#;
    test(&mut compiler, expected, ast);
}

// port: DotFormatterTest#testImportStarLabel
#[test]
fn test_import_star_label() {
    let mut compiler = Compiler::new();
    let ast = compiler.new_node(Token::IMPORT);
    let child = compiler.new_node(Token::EMPTY);
    ast.add_child_to_back(&mut compiler, child);
    let child = compiler.new_string_with_token(Token::IMPORT_STAR, "name");
    ast.add_child_to_back(&mut compiler, child);
    let child = compiler.new_string_with_token(Token::STRINGLIT, "module-name");
    ast.add_child_to_back(&mut compiler, child);

    let expected = r#"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="IMPORT"];
  node1 [label="EMPTY"];
  node0 -> node1 [weight=1];
  node2 [label="IMPORT_STAR(name)"];
  node0 -> node2 [weight=1];
  node3 [label="STRINGLIT(module-nam)"];
  node0 -> node3 [weight=1];
}
"#;
    test(&mut compiler, expected, ast);
}

// port: DotFormatterTest#test
fn test(compiler: &mut Compiler, expected: &str, ast: closure_rhino::node::NodeId) {
    assert_eq!(DotFormatter::to_dot(compiler, ast), expected);
}

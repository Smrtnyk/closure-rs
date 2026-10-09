/*
 * Copyright 2013 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/MinimizedConditionTest.java.

//! Port of `MinimizedConditionTest.java`: tests for `MinimizedCondition` in isolation. Tests for
//! the containing PeepholeMinimizeConditions pass are in PeepholeMinimizeConditionsTest.

use closure_jscomp::{
    Compiler,
    compiler_options::CompilerOptions,
    minimized_condition::{MinimizationStyle, MinimizedCondition},
    source_file::SourceFile,
};
use closure_rhino::{check_state, node::NodeId};
use std::sync::Arc;

// port: MinimizedConditionTest#parseExpr
fn parse_expr(code: &str) -> (Compiler, NodeId) {
    let mut compiler = Compiler::new();
    let input = [Arc::new(SourceFile::from_code("code", code))];
    let externs: Vec<Arc<SourceFile>> = Vec::new();
    let options = CompilerOptions::new();
    compiler.init(&externs, &input, options);
    let root = compiler.parse_inputs();
    let errors: Vec<String> = compiler
        .get_errors()
        .iter()
        .map(|e| e.to_string())
        .collect();
    assert!(
        root.is_some(),
        "Unexpected parse error(s): {}",
        errors.join("\n")
    );
    let root = root.unwrap();
    let externs_root = root.get_first_child(&compiler).unwrap();
    let main_root = externs_root.get_next(&compiler).unwrap();
    let script = main_root.get_first_child(&compiler).unwrap();
    let expr_result = script.get_first_child(&compiler).unwrap();
    let n = expr_result.get_first_child(&compiler).unwrap();
    (compiler, n)
}

// port: MinimizedConditionTest#cloneAttachedTree
fn clone_attached_tree(compiler: &mut Compiler, n: NodeId) -> NodeId {
    let parent = n.get_parent(compiler).unwrap();
    check_state!(parent.get_first_child(compiler) == Some(n));
    parent
        .clone_tree(compiler)
        .get_first_child(compiler)
        .unwrap()
}

/// Tests minimization of input condition.
///
/// `input` is input code containing a condition, `positive` the representation expected to be
/// produced when penalizing a leading NOT, `negative` the representation expected to be produced
/// when not penalizing a leading NOT.
// port: MinimizedConditionTest#minCond
fn min_cond(input: &str, positive: &str, negative: &str) {
    let (mut compiler, input_node) = parse_expr(input);
    let clone1 = clone_attached_tree(&mut compiler, input_node);
    let result1 = MinimizedCondition::from_condition_node(&mut compiler, clone1);
    let clone2 = clone_attached_tree(&mut compiler, input_node);
    let result2 = MinimizedCondition::from_condition_node(&mut compiler, clone2);
    let (positive_compiler, positive_node) = parse_expr(positive);
    let (negative_compiler, negative_node) = parse_expr(negative);
    // With counting the leading NOT node:
    let positive_result = result1
        .get_minimized(&mut compiler, MinimizationStyle::PREFER_UNNEGATED)
        .build_replacement(&mut compiler);
    // Without counting the leading NOT node:
    let negative_result = result2
        .get_minimized(&mut compiler, MinimizationStyle::ALLOW_LEADING_NOT)
        .build_replacement(&mut compiler);
    if !positive_result.is_equivalent_to_across(&compiler, &positive_compiler, positive_node) {
        let expected_tree = positive_node.to_string_tree(&positive_compiler);
        let actual_tree = positive_result.to_string_tree(&compiler);
        panic!(
            "Not equal:\nExpected: {}\nBut was : {}\nExpected tree:\n{}\nActual tree:\n{}",
            positive,
            compiler.to_source_for_node(positive_result),
            expected_tree,
            actual_tree
        );
    }
    if !negative_result.is_equivalent_to_across(&compiler, &negative_compiler, negative_node) {
        let expected_tree = negative_node.to_string_tree(&negative_compiler);
        let actual_tree = negative_result.to_string_tree(&compiler);
        panic!(
            "Not equal:\nExpected: {}\nBut was : {}\nExpected tree:\n{}\nActual tree:\n{}",
            negative,
            compiler.to_source_for_node(negative_result),
            expected_tree,
            actual_tree
        );
    }
}

// port: MinimizedConditionTest#testTryMinimizeCondSimple
#[test]
fn test_try_minimize_cond_simple() {
    min_cond("x", "x", "x");
    min_cond("!x", "!x", "!x");
    min_cond("!!x", "x", "x");
    min_cond("!(x && y)", "!x || !y", "!(x && y)");
}

// port: MinimizedConditionTest#testMinimizeDemorganSimple
#[test]
fn test_minimize_demorgan_simple() {
    min_cond("!(x&&y)", "!x||!y", "!(x&&y)");
    min_cond("!(x||y)", "!x&&!y", "!(x||y)");
    min_cond("!x||!y", "!x||!y", "!(x&&y)");
    min_cond("!x&&!y", "!x&&!y", "!(x||y)");
    min_cond("!(x && y && z)", "!(x && y && z)", "!(x && y && z)");
    min_cond("(!a||!b)&&c", "(!a||!b)&&c", "!(a&&b||!c)");
    min_cond("(!a||!b)&&(c||d)", "!(a&&b||!c&&!d)", "!(a&&b||!c&&!d)");
}

// port: MinimizedConditionTest#testMinimizeBug8494751
#[test]
fn test_minimize_bug8494751() {
    min_cond(
        "x && (y===2 || !f()) && (y===3 || !h())",
        // TODO(tbreisacher): The 'positive' option could be better:
        // "x && !((y!==2 && f()) || (y!==3 && h()))",
        "!(!x || (y!==2 && f()) || (y!==3 && h()))",
        "!(!x || (y!==2 && f()) || (y!==3 && h()))",
    );

    min_cond(
        "x && (y===2 || !f?.()) && (y===3 || !h?.())",
        "!(!x || (y!==2 && f?.()) || (y!==3 && h?.()))",
        "!(!x || (y!==2 && f?.()) || (y!==3 && h?.()))",
    );
}

// port: MinimizedConditionTest#testMinimizeComplementableOperator
#[test]
fn test_minimize_complementable_operator() {
    min_cond(
        "0===c && (2===a || 1===a)",
        "0===c && (2===a || 1===a)",
        "!(0!==c || 2!==a && 1!==a)",
    );
}

// port: MinimizedConditionTest#testMinimizeHook
#[test]
fn test_minimize_hook() {
    min_cond("!(x ? y : z)", "(x ? !y : !z)", "!(x ? y : z)");
}

// port: MinimizedConditionTest#testMinimizeComma
#[test]
fn test_minimize_comma() {
    min_cond("!(inc(), test())", "inc(), !test()", "!(inc(), test())");
    min_cond(
        "!(inc?.(), test?.())",
        "inc?.(), !test?.()",
        "!(inc?.(), test?.())",
    );
    min_cond("!((x,y)&&z)", "(x,!y)||!z", "!((x,y)&&z)");
}

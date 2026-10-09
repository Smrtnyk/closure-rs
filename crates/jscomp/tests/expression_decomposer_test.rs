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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/ExpressionDecomposerTest.java.

//! Port of `ExpressionDecomposerTest.java` (141 @Test methods). The 102 cases with
//! shouldTestTypes = true run TypeCheck#processForTesting with a SemanticReverseAbstractInterpreter
//! before and after decomposing and compare the JSType annotation strings.
//!
//! Note: functions "foo" and "goo" are external functions in the helper.
#![allow(clippy::too_many_lines)]

use closure_jscomp::{
    Compiler,
    compiler_options::{CompilerOptions, LanguageMode},
    expression_decomposer::{DecompositionType, ExpressionDecomposer},
    google_coding_convention::GoogleCodingConvention,
    node_traversal::NodeTraversal,
    normalize::NormalizeStatements,
    scope::Scope,
    semantic_reverse_abstract_interpreter::SemanticReverseAbstractInterpreter,
    source_info_check::SourceInfoCheck,
    type_check::TypeCheck,
};
use closure_jstype::js_type::{JSType, Nullability};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::{
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use std::sync::Arc;

type NodeFinder = Box<dyn Fn(&mut Compiler, NodeId) -> NodeId>;

struct ExpressionDecomposerTest {
    known_constants: IndexSet<JsString>,
    /// The language out to set in the compiler options. If null, use the default.
    language_out: Option<LanguageMode>,
    // Whether we should run type checking and test the type information in the output expression
    should_test_types: bool,
}

impl ExpressionDecomposerTest {
    // port: ExpressionDecomposerTest#setUp
    fn set_up() -> Self {
        Self {
            known_constants: IndexSet::<_>::default(),
            should_test_types: true,
            language_out: None,
        }
    }

    // port: ExpressionDecomposerTest#helperCanExposeExpression
    fn helper_can_expose_expression(
        &self,
        expected_result: DecompositionType,
        code: &str,
        node_finder_fn: NodeFinder,
    ) {
        let mut compiler = self.get_compiler();
        let decomposer = self.create_decomposer(&mut compiler);
        let tree = parse(&mut compiler, code);

        let externs_root = parse(&mut compiler, "function goo() {} function foo() {}");
        let _ = externs_root;

        let expresion_node = node_finder_fn(&mut compiler, tree);

        let result = decomposer.can_expose_expression(&mut compiler, expresion_node);
        assert_eq!(result, expected_result, "{code}");
    }

    // port: ExpressionDecomposerTest#helperExposeExpression
    fn helper_expose_expression(
        &self,
        code: &str,
        compiler_to_node_finder: NodeFinder,
        expected_result: &str,
    ) {
        let mut compiler = self.get_compiler();

        let expected_root = parse(&mut compiler, expected_result);
        let tree = parse(&mut compiler, code);

        if self.should_test_types {
            process_for_typecheck(&mut compiler, tree);
        }

        let mut decomposer = self.create_decomposer(&mut compiler);
        decomposer.set_temp_name_prefix("temp");
        decomposer.set_result_name_prefix("result");

        let node_finder = compiler_to_node_finder;
        let expr = node_finder(&mut compiler, tree);

        let result = decomposer.can_expose_expression(&mut compiler, expr);
        assert_eq!(result, DecompositionType::DECOMPOSABLE);

        decomposer.maybe_expose_expression(&mut compiler, expr);
        validate_source_info(&mut compiler, tree);
        assert_node_equal(&mut compiler, tree, expected_root);

        if self.should_test_types {
            let (mut decompose_compiler, decompose_then_type_check) =
                self.helper_expose_expression_then_type_check(code, &node_finder);
            check_type_strings_equal_as_tree(
                &mut decompose_compiler,
                decompose_then_type_check,
                &mut compiler,
                tree,
            );
        }
    }

    /// Returns the compiler too: the tree lives in that compiler's arena.
    // port: ExpressionDecomposerTest#helperExposeExpressionThenTypeCheck
    fn helper_expose_expression_then_type_check(
        &self,
        code: &str,
        node_finder: &NodeFinder,
    ) -> (Compiler, NodeId) {
        let mut compiler = self.get_compiler();
        let tree = parse(&mut compiler, code);

        let mut decomposer = self.create_decomposer(&mut compiler);
        decomposer.set_temp_name_prefix("temp");
        decomposer.set_result_name_prefix("result");

        let expr = node_finder(&mut compiler, tree);

        decomposer.maybe_expose_expression(&mut compiler, expr);
        process_for_typecheck(&mut compiler, tree);

        (compiler, tree)
    }

    // port: ExpressionDecomposerTest#createDecomposer
    fn create_decomposer(&self, compiler: &mut Compiler) -> ExpressionDecomposer {
        let scope = new_scope(compiler);
        let supplier = compiler.get_unique_name_id_supplier();
        compiler.create_expression_decomposer(supplier, self.known_constants.clone(), scope)
    }

    // port: ExpressionDecomposerTest#helperMoveExpression
    fn helper_move_expression(
        &self,
        code: &str,
        compiler_to_node_finder: NodeFinder,
        expected_result: &str,
    ) {
        let mut compiler = self.get_compiler();

        let node_finder = compiler_to_node_finder;

        let expected_root = parse(&mut compiler, expected_result);
        let tree = parse(&mut compiler, code);
        let original_tree = tree.clone_tree(&mut compiler);

        if self.should_test_types {
            process_for_typecheck(&mut compiler, tree);
        }

        let mut decomposer = self.create_decomposer(&mut compiler);
        decomposer.set_temp_name_prefix("temp");
        decomposer.set_result_name_prefix("result");

        let expr = node_finder(&mut compiler, tree);

        decomposer.move_expression(&mut compiler, expr);
        validate_source_info(&mut compiler, tree);
        assert_node_equal(&mut compiler, tree, expected_root);

        if self.should_test_types {
            // find a basis for comparison:
            let original_expr = node_finder(&mut compiler, original_tree);

            decomposer.move_expression(&mut compiler, original_expr);
            process_for_typecheck(&mut compiler, original_tree);

            // TODO(bradfordcsmith): Don't assume type check + decompose gives the same results as
            // decompose + type check.
            // There are legitimate cases where the types will be different from one order to
            // another, but not actually wrong.
            check_type_strings_equal_as_tree_same_compiler(&mut compiler, original_tree, tree);
        }
    }

    // port: ExpressionDecomposerTest#getCompiler
    fn get_compiler(&self) -> Compiler {
        let mut compiler = Compiler::new();
        let mut options = CompilerOptions::new();
        // If the specific test case requested an output language level,
        // use it. Otherwise, keep the default.
        if let Some(language_out) = self.language_out {
            options.set_language_out(language_out);
        }
        options.set_coding_convention(Arc::new(GoogleCodingConvention::new()));
        options.set_pretty_print(true);
        // Don't prefix the compiler output with `"use strict";`.
        // It's noise for these tests, and it interferes with tests that
        // want to use compiler.toSource() to string match expressions.
        options.set_emit_use_strict(false);
        options.set_language_in(LanguageMode::UNSUPPORTED);
        compiler.init_options(options);
        compiler
    }
}

/// The expected and actual trees live in the arenas of two different compilers (Java compares
/// nodes of two Compiler instances directly).
// port: ExpressionDecomposerTest#checkTypeStringsEqualAsTree
fn check_type_strings_equal_as_tree(
    expected_compiler: &mut Compiler,
    root_expected: NodeId,
    actual_compiler: &mut Compiler,
    root_actual: NodeId,
) {
    let expected_type = root_expected.get_jstype(expected_compiler);
    let actual_type = root_actual.get_jstype(actual_compiler);

    match (expected_type, actual_type) {
        (Some(expected_type), Some(actual_type)) => {
            let expected_unknown = {
                let (reg, ast) = expected_compiler.get_type_registry_and_ast();
                expected_type.is_unknown_type(reg, ast)
            };
            let actual_unknown = {
                let (reg, ast) = actual_compiler.get_type_registry_and_ast();
                actual_type.is_unknown_type(reg, ast)
            };
            if expected_unknown && actual_unknown {
                // continue
            } else {
                // we can't compare actual equality because the types are from different runs of
                // the type inference, so we just compare the strings.
                let actual_string = {
                    let (reg, ast) = actual_compiler.get_type_registry_and_ast();
                    actual_type.to_annotation_string(reg, ast, Nullability::EXPLICIT)
                };
                let expected_string = {
                    let (reg, ast) = expected_compiler.get_type_registry_and_ast();
                    expected_type.to_annotation_string(reg, ast, Nullability::EXPLICIT)
                };
                assert_eq!(
                    actual_string,
                    expected_string,
                    "Expected {} but got {}",
                    node_to_source(expected_compiler, root_expected),
                    node_to_source(actual_compiler, root_actual)
                );
            }
        }
        (expected_type, actual_type) => {
            assert!(
                expected_type.is_none() && actual_type.is_none(),
                "Expected {} but got {}",
                node_to_source(expected_compiler, root_expected),
                node_to_source(actual_compiler, root_actual)
            );
        }
    }

    let mut child1 = root_expected.get_first_child(expected_compiler);
    let mut child2 = root_actual.get_first_child(actual_compiler);
    while let Some(c1) = child1 {
        let c2 = child2.expect("NullPointerException");
        check_type_strings_equal_as_tree(expected_compiler, c1, actual_compiler, c2);
        child1 = c1.get_next(expected_compiler);
        child2 = c2.get_next(actual_compiler);
    }
}

/// checkTypeStringsEqualAsTree for two trees of the same compiler (helperMoveExpression).
// port: ExpressionDecomposerTest#checkTypeStringsEqualAsTree
fn check_type_strings_equal_as_tree_same_compiler(
    compiler: &mut Compiler,
    root_expected: NodeId,
    root_actual: NodeId,
) {
    let expected_type = root_expected.get_jstype(compiler);
    let actual_type = root_actual.get_jstype(compiler);

    match (expected_type, actual_type) {
        (Some(expected_type), Some(actual_type)) => {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            if expected_type.is_unknown_type(reg, ast) && actual_type.is_unknown_type(reg, ast) {
                // continue
            } else {
                // we can't compare actual equality because the types are from different runs of
                // the type inference, so we just compare the strings.
                let actual_string =
                    actual_type.to_annotation_string(reg, ast, Nullability::EXPLICIT);
                let expected_string =
                    expected_type.to_annotation_string(reg, ast, Nullability::EXPLICIT);
                if actual_string != expected_string {
                    panic!(
                        "Expected {} but got {}: {actual_string:?} != {expected_string:?}",
                        node_to_source(compiler, root_expected),
                        node_to_source(compiler, root_actual)
                    );
                }
            }
        }
        (expected_type, actual_type) => {
            assert!(
                expected_type.is_none() && actual_type.is_none(),
                "Expected {} but got {}",
                node_to_source(compiler, root_expected),
                node_to_source(compiler, root_actual)
            );
        }
    }

    let mut child1 = root_expected.get_first_child(compiler);
    let mut child2 = root_actual.get_first_child(compiler);
    while let Some(c1) = child1 {
        let c2 = child2.expect("NullPointerException");
        check_type_strings_equal_as_tree_same_compiler(compiler, c1, c2);
        child1 = c1.get_next(compiler);
        child2 = c2.get_next(compiler);
    }
}

// port: ExpressionDecomposerTest#processForTypecheck
fn process_for_typecheck(compiler: &mut Compiler, js_root: NodeId) {
    let externs = IR::root(compiler, &[]);
    let js = IR::root(compiler, &[js_root]);
    let root = IR::root(compiler, &[externs, js]);
    let interpreter = SemanticReverseAbstractInterpreter::new(compiler.get_type_registry());
    let first = root.get_first_child(compiler);
    let second = root
        .get_second_child(compiler)
        .expect("NullPointerException");
    TypeCheck::new(compiler, interpreter).process_for_testing(compiler, first, second);
    compiler.set_type_checking_has_run(true);
}

/// Compare the node-trees of actual and expected (NodeSubject#isEqualTo with the compiler's
/// toSource as serializer).
fn assert_node_equal(compiler: &mut Compiler, actual: NodeId, expected: NodeId) {
    if !actual.is_equivalent_to(compiler, expected) {
        let actual_source = compiler.to_source_for_node(actual);
        let expected_source = compiler.to_source_for_node(expected);
        panic!(
            "Node tree inequality:\nexpected:\n{expected_source}\nbut was:\n{actual_source}\n{}",
            actual.to_string_tree(compiler)
        );
    }
}

/// Provides a `toString()` method that contains the source code a node represents where
/// possible, or some explanatory text when not possible.
// port: ExpressionDecomposerTest.NodeToSource#toString
fn node_to_source(compiler: &mut Compiler, node: NodeId) -> String {
    if node.is_template_lit_string(compiler) {
        // A string part of a template literal cannot be printed as code on its own.
        format!(
            "[Template literal string: '{}']",
            node.get_raw_string(compiler).to_string_lossy()
        )
    } else if node.is_template_lit_sub(compiler) {
        // The template literal substitution node cannot itself be turned into source code,
        // but we can do that for the expression inside of it.
        let only_child = node.get_only_child(compiler);
        format!(
            "[Template literal substitution: '{}']",
            compiler.to_source_for_node(only_child)
        )
    } else {
        compiler.to_source_for_node(node)
    }
}

// NodeUtil#findPreorder, with predicates that print through the (mutable) compiler.
fn find_preorder(
    compiler: &mut Compiler,
    node: NodeId,
    pred: &dyn Fn(&mut Compiler, NodeId) -> bool,
    traverse_children_pred: &dyn Fn(&mut Compiler, NodeId) -> bool,
) -> Option<NodeId> {
    if pred(compiler, node) {
        return Some(node);
    }
    if !traverse_children_pred(compiler, node) {
        return None;
    }
    let mut c = node.get_first_child(compiler);
    while let Some(child) = c {
        let result = find_preorder(compiler, child, pred, traverse_children_pred);
        if result.is_some() {
            return result;
        }
        c = child.get_next(compiler);
    }
    None
}

// port: ExpressionDecomposerTest#exprMatchesStr
fn expr_matches_str(expr_string: &'static str) -> NodeFinder {
    Box::new(move |compiler: &mut Compiler, root: NodeId| {
        // When matching trim off the trailing newline added by the compiler's pretty-print
        // option.
        let is_a_match = |compiler: &mut Compiler, node: NodeId| {
            expr_string == node_to_source(compiler, node).trim()
        };
        let contains_a_match = |compiler: &mut Compiler, node: NodeId| {
            node_to_source(compiler, node).contains(expr_string)
        };
        let matching_node = find_preorder(compiler, root, &is_a_match, &contains_a_match);
        match matching_node {
            Some(n) => n,
            None => {
                let source = compiler.to_source_for_node(root);
                panic!("Expected node `{expr_string}` was not found in `{source}`");
            }
        }
    })
}

// port: ExpressionDecomposerTest#findClass
fn find_class(ast: &Ast, n: NodeId) -> Option<NodeId> {
    if n.is_class(ast) {
        return Some(n);
    }
    let mut child = n.get_first_child(ast);
    while let Some(c) = child {
        let maybe_class = find_class(ast, c);
        if maybe_class.is_some() {
            return maybe_class;
        }
        child = c.get_next(ast);
    }
    None
}

fn find_class_finder() -> NodeFinder {
    Box::new(|compiler: &mut Compiler, root: NodeId| {
        find_class(compiler, root).expect("NullPointerException")
    })
}

// port: ExpressionDecomposerTest#validateSourceInfo
fn validate_source_info(compiler: &mut Compiler, subtree: NodeId) {
    SourceInfoCheck::new(compiler).set_check_sub_tree(compiler, subtree);
    // Source information problems are reported as compiler errors.
    assert!(
        compiler.get_errors().is_empty(),
        "{:?}",
        compiler.get_errors()
    );
}

// port: ExpressionDecomposerTest#parse
fn parse(compiler: &mut Compiler, js: &str) -> NodeId {
    let n = compiler.parse_test_code(js);
    let mut normalize = NormalizeStatements::new(compiler, false, None);
    NodeTraversal::traverse(compiler, n, &mut normalize);
    assert!(
        compiler.get_errors().is_empty(),
        "{:?}",
        compiler.get_errors()
    );
    n
}

// port: ExpressionDecomposerTest#newScope
fn new_scope(compiler: &mut Compiler) -> Scope {
    let root = compiler.new_node(Token::ROOT);
    Scope::create_global_scope(compiler, root)
}

use DecompositionType::{DECOMPOSABLE, MOVABLE, UNDECOMPOSABLE};

// port: ExpressionDecomposerTest#testWindowLocationAssign
#[test]
fn test_window_location_assign() {
    let mut t = ExpressionDecomposerTest::set_up();
    // avoid decomposing `window.location.assign` when the output code could
    // end up running on IE11. See more explanation in ExpressionDecomposer.java.
    t.language_out = Some(LanguageMode::ECMASCRIPT5);
    t.helper_can_expose_expression(
        MOVABLE,
        "window.location.assign(foo())",
        expr_matches_str("foo()"),
    );
    t.helper_move_expression(
        "window.location.assign(foo())",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo();
window.location.assign(result$jscomp$0)
",
    );
    // confirm that the default behavior does not treat window.location.assign
    // specially
    t.language_out = None;
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "window.location.assign(foo())",
        expr_matches_str("foo()"),
    );
    t.helper_expose_expression(
        "window.location.assign(foo())",
        expr_matches_str("foo()"),
        "var temp_const$jscomp$1 = window.location;
var temp_const$jscomp$0 = temp_const$jscomp$1.assign;
temp_const$jscomp$0.call(temp_const$jscomp$1, foo());
",
    );
}

// port: ExpressionDecomposerTest#testObjectDestructuring_withComputedKey_doesNotCrash
#[test]
fn test_object_destructuring_with_computed_key_does_not_crash() {
    let t = ExpressionDecomposerTest::set_up();
    // computed prop is found to be decomposable
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "var a; ({ [foo()]: a} = obj);",
        expr_matches_str("foo()"),
    );

    // TODO(b/339040894): Fix this crash.
    let ex = assert_throws_illegal_state(|| {
        t.helper_expose_expression(
            "var a; ({ [foo()]: a} = obj);", //
            expr_matches_str("foo()"),
            "var a;
var temp_const$jscomp$0 = obj;
var temp_const$jscomp$1 = foo();
({ [temp_const$jscomp$1]: a} = temp_const$jscomp$0);
",
        );
    });
    assert!(ex.contains("exposeExpression exposed nothing"), "{ex}");
}

// port: ExpressionDecomposerTest#testCannotExpose_expression1
#[test]
fn test_cannot_expose_expression1() {
    let t = ExpressionDecomposerTest::set_up();
    // Can't move or decompose some classes of expressions.
    t.helper_can_expose_expression(UNDECOMPOSABLE, "while(foo());", expr_matches_str("foo()"));
    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "while(x = goo()&&foo()){}",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "while(x += goo()&&foo()){}",
        expr_matches_str("foo()"),
    );

    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "do{}while(foo());",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(UNDECOMPOSABLE, "for(;foo(););", expr_matches_str("foo()"));
    // This case could be supported for loops without conditional continues
    // by moving the increment into the loop body.
    t.helper_can_expose_expression(UNDECOMPOSABLE, "for(;;foo());", expr_matches_str("foo()"));
    t.helper_can_expose_expression(MOVABLE, "for(foo();;);", expr_matches_str("foo()"));

    // This is potentially doable but a bit too complex currently.
    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "switch(1){case foo():;}",
        expr_matches_str("foo()"),
    );
}

// port: ExpressionDecomposerTest#testCanExposeExpression2
#[test]
fn test_can_expose_expression2() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(MOVABLE, "foo()", expr_matches_str("foo()"));
    t.helper_can_expose_expression(MOVABLE, "x = foo()", expr_matches_str("foo()"));
    t.helper_can_expose_expression(MOVABLE, "var x = foo()", expr_matches_str("foo()"));
    t.helper_can_expose_expression(MOVABLE, "const x = foo()", expr_matches_str("foo()"));
    t.helper_can_expose_expression(MOVABLE, "let x = foo()", expr_matches_str("foo()"));
    t.helper_can_expose_expression(MOVABLE, "if(foo()){}", expr_matches_str("foo()"));
    t.helper_can_expose_expression(MOVABLE, "switch(foo()){}", expr_matches_str("foo()"));
    t.helper_can_expose_expression(MOVABLE, "switch(foo()){}", expr_matches_str("foo()"));
    t.helper_can_expose_expression(
        MOVABLE,
        "function f(){ return foo();}",
        expr_matches_str("foo()"),
    );

    t.helper_can_expose_expression(MOVABLE, "x = foo() && 1", expr_matches_str("foo()"));
    t.helper_can_expose_expression(MOVABLE, "x = foo() || 1", expr_matches_str("foo()"));
    t.helper_can_expose_expression(MOVABLE, "x = foo() ? 0 : 1", expr_matches_str("foo()"));
    t.helper_can_expose_expression(
        MOVABLE,
        "(function(a){b = a})(foo())",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        MOVABLE,
        "function f(){ throw foo();}",
        expr_matches_str("foo()"),
    );
}

// port: ExpressionDecomposerTest#nullishCoalesceMovable
#[test]
fn nullish_coalesce_movable() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(MOVABLE, "x = foo() ?? 1", expr_matches_str("foo()"));
}

// port: ExpressionDecomposerTest#nullishCoalesceDecomposable
#[test]
fn nullish_coalesce_decomposable() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "var x = null ?? foo()",
        expr_matches_str("foo()"),
    );
}

// port: ExpressionDecomposerTest#nullishCoalesceUnDecomposable
#[test]
fn nullish_coalesce_un_decomposable() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "while(x = goo()??foo()){}",
        expr_matches_str("foo()"),
    );
}

// port: ExpressionDecomposerTest#assignCoalesceMovable
#[test]
fn assign_coalesce_movable() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "x ??= goo() + foo()",
        expr_matches_str("foo()"),
    );
}

// port: ExpressionDecomposerTest#optChainMovable
#[test]
fn opt_chain_movable() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(MOVABLE, "foo()?.x", expr_matches_str("foo()"));
    t.helper_can_expose_expression(MOVABLE, "foo()?.[x]", expr_matches_str("foo()"));
    t.helper_can_expose_expression(MOVABLE, "foo()?.()", expr_matches_str("foo()"));
}

// port: ExpressionDecomposerTest#optChainDecomposable
#[test]
fn opt_chain_decomposable() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(DECOMPOSABLE, "x?.[foo()]", expr_matches_str("foo()"));
    t.helper_can_expose_expression(DECOMPOSABLE, "x?.(foo())", expr_matches_str("foo()"));
}

// port: ExpressionDecomposerTest#optChainAllowMethodCallDecomposable
#[test]
fn opt_chain_allow_method_call_decomposable() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(DECOMPOSABLE, "x?.y(foo())", expr_matches_str("foo()"));
}

// port: ExpressionDecomposerTest#optChainUnDecomposable
#[test]
fn opt_chain_un_decomposable() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "while(x = y?.[foo()]){}",
        expr_matches_str("foo()"),
    );
}

// port: ExpressionDecomposerTest#testCanExposeExpression3
#[test]
fn test_can_expose_expression3() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(DECOMPOSABLE, "x = 0 && foo()", expr_matches_str("foo()"));
    t.helper_can_expose_expression(DECOMPOSABLE, "x = 1 || foo()", expr_matches_str("foo()"));
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "var x = 1 ? foo() : 0",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "const x = 1 ? foo() : 0",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "let x = 1 ? foo() : 0",
        expr_matches_str("foo()"),
    );

    t.helper_can_expose_expression(DECOMPOSABLE, "goo() && foo()", expr_matches_str("foo()"));
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "x = goo() && foo()",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "x += goo() && foo()",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "var x = goo() && foo()",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "const x = goo() && foo()",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "let x = goo() && foo()",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "if(goo() && foo()){}",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "switch(goo() && foo()){}",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "switch(goo() && foo()){}",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "switch(x = goo() && foo()){}",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "function f(){ return goo() && foo();}",
        expr_matches_str("foo()"),
    );
}

// port: ExpressionDecomposerTest#testCanExposeExpression_compoundDeclaration_inForInitializer_firstElement
#[test]
fn test_can_expose_expression_compound_declaration_in_for_initializer_first_element() {
    let t = ExpressionDecomposerTest::set_up();
    // VAR will already be hoisted by `Normalize`.
    t.helper_can_expose_expression(
        MOVABLE,
        "for (var x = foo(), y = 5;;) {}",
        expr_matches_str("foo()"),
    );

    t.helper_can_expose_expression(
        MOVABLE,
        "for (let x = foo(), y = 5;;) {}",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        MOVABLE,
        "for (const x = foo(), y = 5;;) {}",
        expr_matches_str("foo()"),
    );
}

// port: ExpressionDecomposerTest#testCanExposeExpression_compoundDeclaration_inForInitializer_nthElement
#[test]
fn test_can_expose_expression_compound_declaration_in_for_initializer_nth_element() {
    let t = ExpressionDecomposerTest::set_up();
    // TODO(b/121157467) FOR introduces complex scoping that isn't currently `Normalize`d.
    // Since in some cases we'd effectively end up having to `Normalize` these, decomposition just
    // bails for now.

    // VAR will already be hoisted by `Normalize`.
    t.helper_can_expose_expression(
        MOVABLE,
        "for (var x = 8, y = foo();;) {}",
        expr_matches_str("foo()"),
    );

    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "for (let x = 8, y = foo();;) {}",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "for (const x = 8, y = foo();;) {}",
        expr_matches_str("foo()"),
    );
}

// port: ExpressionDecomposerTest#testCannotExpose_expression4
#[test]
fn test_cannot_expose_expression4() {
    let t = ExpressionDecomposerTest::set_up();
    // 'this' must be preserved in call.
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "if (goo.a(1, foo()));",
        expr_matches_str("foo()"),
    );
}

// port: ExpressionDecomposerTest#testCannotExpose_expression5
#[test]
fn test_cannot_expose_expression5() {
    let t = ExpressionDecomposerTest::set_up();
    // 'this' must be preserved in call.
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "if (goo['a'](foo()));",
        expr_matches_str("foo()"),
    );
}

// port: ExpressionDecomposerTest#testCannotExpose_expression6
#[test]
fn test_cannot_expose_expression6() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "z:if (goo.a(1, foo()));",
        expr_matches_str("foo()"),
    );
}

// port: ExpressionDecomposerTest#testCanExposeExpression7
#[test]
fn test_can_expose_expression7() {
    let t = ExpressionDecomposerTest::set_up();
    // Verify calls to function expressions are movable.
    t.helper_can_expose_expression(
        MOVABLE,
        "(function(map){descriptions_=map})(
  function(){
    var ret={};
    ret[INIT]='a';
    ret[MIGRATION_BANNER_DISMISS]='b';
    return ret
  }());
",
        Box::new(|compiler: &mut Compiler, root_node: NodeId| {
            // Dig out the inner IIFE call and return it as the expression we want to ensure is
            // movable.
            assert_eq!(root_node.get_token(compiler), Token::SCRIPT);
            let expr_result = root_node.get_only_child(compiler);
            assert_eq!(expr_result.get_token(compiler), Token::EXPR_RESULT);
            let outer_call = expr_result.get_only_child(compiler);
            assert!(outer_call.is_call(compiler));
            let inner_iife_call = outer_call.get_second_child(compiler).unwrap();
            assert!(inner_iife_call.is_call(compiler));
            inner_iife_call
        }),
    );
}

// port: ExpressionDecomposerTest#testCanExposeExpression8
#[test]
fn test_can_expose_expression8() {
    let t = ExpressionDecomposerTest::set_up();
    // Can it be decompose?
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "HangoutStarter.prototype.launchHangout = function() {
  var self = a.b;
  var myUrl = new goog.Uri(
      getDomServices_(self).getDomHelper().getWindow().location.href);
};
",
        expr_matches_str("getDomServices_(self)"),
    );
    // Verify it is properly expose the target expression.
    t.helper_expose_expression(
        "HangoutStarter.prototype.launchHangout = function() {
  var self = a.b;
  var myUrl =
      new goog.Uri(getDomServices_(self).getDomHelper().getWindow().location.href);
};
",
        expr_matches_str("getDomServices_(self)"),
        "HangoutStarter.prototype.launchHangout = function() {
  var self = a.b;
  var temp_const$jscomp$0 = goog.Uri;
  var myUrl = new temp_const$jscomp$0(
      getDomServices_(self).getDomHelper().getWindow().location.href);
}
",
    );
    // Verify the results can be properly moved.
    t.helper_move_expression(
        "HangoutStarter.prototype.launchHangout = function() {
  var self = a.b;
  var temp_const$jscomp$0 = goog.Uri;
  var myUrl = new temp_const$jscomp$0(
      getDomServices_(self).getDomHelper().getWindow().location.href);
}
",
        expr_matches_str("getDomServices_(self)"),
        "HangoutStarter.prototype.launchHangout = function() {
  var self=a.b;
  var temp_const$jscomp$0=goog.Uri;
  var result$jscomp$0=getDomServices_(self);
  var myUrl=new temp_const$jscomp$0(
      result$jscomp$0.getDomHelper().getWindow().location.href);
}
",
    );
}

// port: ExpressionDecomposerTest#testCannotExpose_expression9
#[test]
fn test_cannot_expose_expression9() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "function *f() { for (let x of yield y) {} }",
        expr_matches_str("yield y"),
    );
}

// port: ExpressionDecomposerTest#testCannotExpose_forAwaitOf
#[test]
fn test_cannot_expose_for_await_of() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "async function *f() { for await (let x of yield y) {} }",
        expr_matches_str("yield y"),
    );
}

// port: ExpressionDecomposerTest#testCanExposeExpression10
#[test]
fn test_can_expose_expression10() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "function *f() { for (let x in yield y) {} }",
        expr_matches_str("yield y"),
    );
}

// port: ExpressionDecomposerTest#testCannotExpose_expression11
#[test]
fn test_cannot_expose_expression11() {
    let t = ExpressionDecomposerTest::set_up();
    // expressions in parameter lists
    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "function f(x = foo()) {}",
        expr_matches_str("foo()"),
    );

    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "function f({[foo()]: x}) {}",
        expr_matches_str("foo()"),
    );

    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "(function (x = foo()) {})()",
        expr_matches_str("foo()"),
    );

    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "(function ({[foo()]: x}) {})()",
        expr_matches_str("foo()"),
    );
}

// port: ExpressionDecomposerTest#testCanExpose_aCall_withSpreadSibling
#[test]
fn test_can_expose_a_call_with_spread_sibling() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(DECOMPOSABLE, "f(...x, y());", expr_matches_str("y()"));
    t.helper_can_expose_expression(DECOMPOSABLE, "f(y(), ...x);", expr_matches_str("y()"));

    t.helper_can_expose_expression(DECOMPOSABLE, "new D(...x, y());", expr_matches_str("y()"));
    t.helper_can_expose_expression(DECOMPOSABLE, "new D(y(), ...x);", expr_matches_str("y()"));

    t.helper_can_expose_expression(DECOMPOSABLE, "[...x, y()];", expr_matches_str("y()"));
    t.helper_can_expose_expression(DECOMPOSABLE, "({...x, z: y()});", expr_matches_str("y()"));

    // Array- and object-literal instantiations cannot be side-effected.
    t.helper_can_expose_expression(MOVABLE, "[y(), ...x];", expr_matches_str("y()"));
    t.helper_can_expose_expression(MOVABLE, "({z: y(), ...x});", expr_matches_str("y()"));

    t.helper_can_expose_expression(DECOMPOSABLE, "f(...y());", expr_matches_str("y()"));
    t.helper_can_expose_expression(DECOMPOSABLE, "f(...y(), x);", expr_matches_str("y()"));
    t.helper_can_expose_expression(DECOMPOSABLE, "f(x, ...y());", expr_matches_str("y()"));

    t.helper_can_expose_expression(DECOMPOSABLE, "f(...x, x, y());", expr_matches_str("y()"));
    t.helper_can_expose_expression(DECOMPOSABLE, "f(...x, ...x, y());", expr_matches_str("y()"));
}

// port: ExpressionDecomposerTest#testCanExpose_anExpression_withSpreadRelative_ifInDifferentFunction
#[test]
fn test_can_expose_an_expression_with_spread_relative_if_in_different_function() {
    let t = ExpressionDecomposerTest::set_up();
    // TODO(b/121004488): There are potential decompositions that weren't implemented.
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "f(function() { [...x]; }, y());",
        expr_matches_str("y()"),
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "f(function() { ({...x}); }, y());",
        expr_matches_str("y()"),
    );

    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "f(y(), () => [...x]);",
        expr_matches_str("y()"),
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "f(y(), () => ({...x}));",
        expr_matches_str("y()"),
    );

    t.helper_can_expose_expression(MOVABLE, "[() => f(...x), y()];", expr_matches_str("y()"));

    t.helper_can_expose_expression(
        MOVABLE,
        "[
   class {
     f(x) { return [...x]; }
   },
  y()
];
",
        expr_matches_str("y()"),
    );
}

// port: ExpressionDecomposerTest#testCanExposeExpression12
#[test]
fn test_can_expose_expression12() {
    let t = ExpressionDecomposerTest::set_up();
    // Test destructuring rhs is evaluated before the lhs
    t.helper_can_expose_expression(
        MOVABLE,
        "const {a, b = goo()} = foo();",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        MOVABLE,
        "const [a, b = goo()] = foo();",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        MOVABLE,
        "({a, b = goo()} = foo());",
        expr_matches_str("foo()"),
    );
    // Default value expressions are conditional, which would make the expressions complex.
    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        // default value inside array pattern
        "[{ [foo()]: a } = goo()] = arr;",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        //
        // computed property inside object pattern; decomposed
        "({ [foo()]: a = goo()} = arr);",
        expr_matches_str("foo()"),
    );
    // default value expressions are conditional, which would make the expressions complex
    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        //
        // default value inside object pattern
        "({ [foo()]: a = goo()} = arr);",
        expr_matches_str("goo()"),
    );
    t.helper_expose_expression(
        "var Di = I(() => {
  function zv() {
    JSCOMPILER_PRESERVE(e), [getObj().propName] = CN();
  }
  function CN() {
    return [1];
  }
});
",
        expr_matches_str("CN()"),
        "var Di = I(() => {
  function zv() {
    var temp_const$jscomp$0 = JSCOMPILER_PRESERVE(e);
// TODO(b/339701959): We decided to back off here because of b/338660589. But we should
// optimize this to:
//   var temp_const$jscomp$1 = CN();
//   var temp_const$jscomp$2 = getObj();
//   temp_const$jscomp$0, [temp_const$jscomp$2.propName] = temp_const$jscomp$1;
    temp_const$jscomp$0, [getObj().propName] = CN();
  }
  function CN() {
    return [1];
  }
});
",
    );
    t.helper_expose_expression(
        "var Di = I(() => {
  function zv() {
    JSCOMPILER_PRESERVE(e), ({x: getObj().propName} = CN());
  }
  function CN() {
    return {x: 1};
  }
});
",
        expr_matches_str("CN()"),
        "var Di = I(() => {
  function zv() {
    var temp_const$jscomp$0 = JSCOMPILER_PRESERVE(e);
// TODO(b/339701959): We decided to back off here because of b/338246627. But we should
// optimize this to:
//   var temp_const$jscomp$1 = CN();
//   var temp_const$jscomp$2 = getObj();
//   temp_const$jscomp$0, {x: temp_const$jscomp$2.propName} = temp_const$jscomp$1;
    temp_const$jscomp$0, {x:getObj().propName} = CN();
  }
  function CN() {
    return {x: 1};
  }
});
",
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "var Di = I(() => {
  function zv() {
    JSCOMPILER_PRESERVE(e), [f] = CN();
  }
  function CN() {
    return [1];
  }
});
",
        expr_matches_str("CN()"),
    );
    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "var Di = I(() => {
  function zv() {
    JSCOMPILER_PRESERVE(e), [f = foo()] = CN();
  }
  function CN() {
    return [1];
  }
});
",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "var Di = I(() => {
  function zv() {
    JSCOMPILER_PRESERVE(e), ({f: g} = CN());
  }
  function CN() {
    return {f: 1};
  }
});
",
        expr_matches_str("CN()"),
    );
    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "var Di = I(() => {
  function zv() {
    JSCOMPILER_PRESERVE(e), ({f: g = goo()} = CN());
  }
  function CN() {
    return {f: 1};
  }
});
",
        expr_matches_str("goo()"),
    );
}

// port: ExpressionDecomposerTest#testObjectDestructuring_withDefaultValue_generatesValidAST
#[test]
fn test_object_destructuring_with_default_value_generates_valid_ast() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "var d; ({c: d = 4} = condition ? y() :  {c: 1});",
        expr_matches_str("y()"),
        "var d;
var temp$jscomp$0;
if (condition) {
  temp$jscomp$0 = y();
} else {
  temp$jscomp$0 = {c: 1};
}
({c: d = 4} = temp$jscomp$0);
",
    );
}

// port: ExpressionDecomposerTest#testObjectDestructuring_withDefaultValue_withComputedKey
#[test]
fn test_object_destructuring_with_default_value_with_computed_key() {
    let t = ExpressionDecomposerTest::set_up();
    // default value expressions are conditional, which would make the expressions complex
    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "var a; ({ [foo()]: a = bar()} = baz());",
        expr_matches_str("bar()"),
    );

    t.helper_can_expose_expression(
        MOVABLE,
        "var a; ({ [foo()]: a = bar()} = baz());",
        expr_matches_str("baz()"),
    );

    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "var a; ({ [foo()]: a = bar()} = baz());",
        expr_matches_str("foo()"),
    );
}

// port: ExpressionDecomposerTest#testArrayDestructuring_withDefaultValue_generatesValidAST
#[test]
fn test_array_destructuring_with_default_value_generates_valid_ast() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "var [c = 4] = condition ? y() :  [c = 2];",
        expr_matches_str("y()"),
        "var c;
var temp$jscomp$0;
if (condition) {
  temp$jscomp$0 = y();
} else {
  temp$jscomp$0 = [c = 2];
}
[c = 4] = temp$jscomp$0;
",
    );
}

// port: ExpressionDecomposerTest#testCanExposeExpressionInTemplateLiteralSubstitution
#[test]
fn test_can_expose_expression_in_template_literal_substitution() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(
        MOVABLE,
        "const result = `${foo()}`;",
        expr_matches_str("foo()"),
    );

    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "const obj = {f(x) {}}; obj.f(`${foo()}`);",
        expr_matches_str("foo()"),
    );

    t.helper_can_expose_expression(
        MOVABLE,
        "const result = `${foo()} ${goo()}`;",
        expr_matches_str("foo()"),
    );

    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "const result = `${foo()} ${goo()}`;",
        expr_matches_str("goo()"),
    );
}

// port: ExpressionDecomposerTest#testCannotExpose_defaultValueInParamList
#[test]
fn test_cannot_expose_default_value_in_param_list() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "function fn(a = g()) {}",
        expr_matches_str("g()"),
    );
}

// port: ExpressionDecomposerTest#testCannotExpose_defaultValueInDestructuring
#[test]
fn test_cannot_expose_default_value_in_destructuring() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "let {x = fn()} = y;",
        expr_matches_str("fn()"),
    );

    t.helper_can_expose_expression(
        UNDECOMPOSABLE,
        "let [x = fn()] = y;",
        expr_matches_str("fn()"),
    );
}

// port: ExpressionDecomposerTest#testMoveExpression1
#[test]
fn test_move_expression1() {
    let t = ExpressionDecomposerTest::set_up();
    // There isn't a reason to do this, but it works.
    t.helper_move_expression(
        "foo()",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo(); result$jscomp$0;",
    );
}

// port: ExpressionDecomposerTest#testMoveExpression2
#[test]
fn test_move_expression2() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "x = foo()",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo(); x = result$jscomp$0;",
    );
}

// port: ExpressionDecomposerTest#testMoveExpression3
#[test]
fn test_move_expression3() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "var x = foo()",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo(); var x = result$jscomp$0;",
    );
}

// port: ExpressionDecomposerTest#testMoveExpression4
#[test]
fn test_move_expression4() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "const x = foo()",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo(); const x = result$jscomp$0;",
    );
}

// port: ExpressionDecomposerTest#testMoveExpression5
#[test]
fn test_move_expression5() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "let x = foo()",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo(); let x = result$jscomp$0;",
    );
}

// port: ExpressionDecomposerTest#testMoveExpression6
#[test]
fn test_move_expression6() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "if(foo()){}",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo(); if (result$jscomp$0);",
    );
}

// port: ExpressionDecomposerTest#testMoveExpression7
#[test]
fn test_move_expression7() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "switch(foo()){}",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo(); switch(result$jscomp$0){}",
    );
}

// port: ExpressionDecomposerTest#testMoveExpression8
#[test]
fn test_move_expression8() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "switch(1 + foo()){}",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo(); switch(1 + result$jscomp$0){}",
    );
}

// port: ExpressionDecomposerTest#testMoveExpression9
#[test]
fn test_move_expression9() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "function f(){ return foo();}",
        expr_matches_str("foo()"),
        "function f(){ var result$jscomp$0 = foo(); return result$jscomp$0;}",
    );
}

// port: ExpressionDecomposerTest#testMoveExpression10
#[test]
fn test_move_expression10() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "x = foo() && 1",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo(); x = result$jscomp$0 && 1",
    );
}

// port: ExpressionDecomposerTest#testMoveExpression11
#[test]
fn test_move_expression11() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "x = foo() || 1",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo(); x = result$jscomp$0 || 1",
    );
}

// port: ExpressionDecomposerTest#testMoveExpression12
#[test]
fn test_move_expression12() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "x = foo() ? 0 : 1",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo(); x = result$jscomp$0 ? 0 : 1",
    );
}

// port: ExpressionDecomposerTest#testMoveExpressionNullishCoalesce
#[test]
fn test_move_expression_nullish_coalesce() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "x = foo() ?? 0",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo(); x = result$jscomp$0 ?? 0",
    );
}

// port: ExpressionDecomposerTest#testMoveExpressionOptionalChain
#[test]
fn test_move_expression_optional_chain() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "foo()?.x",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo(); result$jscomp$0?.x",
    );
}

// port: ExpressionDecomposerTest#testMoveExpression13
#[test]
fn test_move_expression13() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "const {a, b} = foo();",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo(); const {a, b} = result$jscomp$0;",
    );
}

// port: ExpressionDecomposerTest#testMoveExpression14
#[test]
fn test_move_expression14() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "({a, b} = foo());",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo(); ({a, b} = result$jscomp$0);",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression1
#[test]
fn test_expose_expression1() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "x = 0 && foo()",
        expr_matches_str("foo()"),
        "var temp$jscomp$0; if (temp$jscomp$0 = 0) temp$jscomp$0 = foo(); x = temp$jscomp$0;",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression2
#[test]
fn test_expose_expression2() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "x = 1 || foo()",
        expr_matches_str("foo()"),
        "var temp$jscomp$0; if (temp$jscomp$0 = 1); else temp$jscomp$0=foo(); x = temp$jscomp$0;",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression3
#[test]
fn test_expose_expression3() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "var x = 1 ? foo() : 0",
        expr_matches_str("foo()"),
        "var temp$jscomp$0;
if (1) temp$jscomp$0 = foo(); else temp$jscomp$0 = 0;
var x = temp$jscomp$0;
",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression4
#[test]
fn test_expose_expression4() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "const x = 1 ? foo() : 0",
        expr_matches_str("foo()"),
        "var temp$jscomp$0;
if (1) temp$jscomp$0 = foo(); else temp$jscomp$0 = 0;
const x = temp$jscomp$0;
",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression5
#[test]
fn test_expose_expression5() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "let x = 1 ? foo() : 0",
        expr_matches_str("foo()"),
        "var temp$jscomp$0;
if (1) temp$jscomp$0 = foo(); else temp$jscomp$0 = 0;
let x = temp$jscomp$0;
",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression6
#[test]
fn test_expose_expression6() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "goo() && foo()",
        expr_matches_str("foo()"),
        "if (goo()) foo();",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionNullishCoalesceNoResult
#[test]
fn expose_expression_nullish_coalesce_no_result() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "goo() ?? foo()",
        expr_matches_str("foo()"),
        "var temp$jscomp$1;
if((temp$jscomp$1 = goo()) != null) temp$jscomp$1;
else foo()
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionOptionalGetElem
#[test]
fn expose_expression_optional_get_elem() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "a = x?.[foo()]",
        expr_matches_str("foo()"),
        "let temp_const$jscomp$0;
var temp$jscomp$1;
if ((temp_const$jscomp$0 = x) == null) {
  temp$jscomp$1 = void 0;
} else {
  temp$jscomp$1 = temp_const$jscomp$0[foo()];
}
a = temp$jscomp$1;
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionOptChainCallChain
#[test]
fn expose_expression_opt_chain_call_chain() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "a = x?.(a).y.z[foo()]",
        expr_matches_str("foo()"),
        "let temp_const$jscomp$0;
var temp$jscomp$1;
if ((temp_const$jscomp$0 = x) == null) {
  temp$jscomp$1 = void 0;
} else {
  var temp_const$jscomp$2 = temp_const$jscomp$0(a).y.z;
  temp$jscomp$1 = temp_const$jscomp$2[foo()];
}
a = temp$jscomp$1;
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionOptChainCallChainNoResult
#[test]
fn expose_expression_opt_chain_call_chain_no_result() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "x?.(a)[y].z[foo()]",
        expr_matches_str("foo()"),
        "let temp_const$jscomp$0;
if ((temp_const$jscomp$0 = x) == null) {
  void 0;
} else {
  var temp_const$jscomp$2 = temp_const$jscomp$0(a)[y].z;
  temp_const$jscomp$2[foo()];
}
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionOptionalGetPropChain
#[test]
fn expose_expression_optional_get_prop_chain() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "a = x?.y.z[foo()]",
        expr_matches_str("foo()"),
        "let temp_const$jscomp$0;
var temp$jscomp$1;
if ((temp_const$jscomp$0 = x) == null) {
  temp$jscomp$1 = void 0;
} else {
  var temp_const$jscomp$2 = temp_const$jscomp$0.y.z;
  temp$jscomp$1 = temp_const$jscomp$2[foo()];
}
a = temp$jscomp$1;
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionOptionalGetPropChainNoResult
#[test]
fn expose_expression_optional_get_prop_chain_no_result() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "x?.y.z[foo()]",
        expr_matches_str("foo()"),
        "let temp_const$jscomp$0;
if ((temp_const$jscomp$0 = x) == null) {
  void 0;
} else {
  var temp_const$jscomp$2 = temp_const$jscomp$0.y.z;
  temp_const$jscomp$2[foo()];
}
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionOptionalGetElemChain
#[test]
fn expose_expression_optional_get_elem_chain() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "a = x?.[y].z[foo()];",
        expr_matches_str("foo()"),
        "let temp_const$jscomp$0;
var temp$jscomp$1;
if ((temp_const$jscomp$0 = x) == null) {
  temp$jscomp$1 = void 0;
} else {
  var temp_const$jscomp$2 = temp_const$jscomp$0[y].z;
  temp$jscomp$1 = temp_const$jscomp$2[foo()];
}
a = temp$jscomp$1;
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionOptionalGetElemChainNoResult
#[test]
fn expose_expression_optional_get_elem_chain_no_result() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "x?.[y].z[foo()]",
        expr_matches_str("foo()"),
        "let temp_const$jscomp$0;
if ((temp_const$jscomp$0 = x) == null) {
  void 0;
} else {
  var temp_const$jscomp$2 = temp_const$jscomp$0[y].z;
  temp_const$jscomp$2[foo()];
}
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionOptionalGetElemWithCall
#[test]
fn expose_expression_optional_get_elem_with_call() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "a = x.y?.[z](foo())",
        expr_matches_str("foo()"),
        "let temp_const$jscomp$0;
var temp$jscomp$1;
if ((temp_const$jscomp$0 = x.y) == null) {
  temp$jscomp$1 = void 0;
} else {
  var temp_const$jscomp$3 = temp_const$jscomp$0;
  var temp_const$jscomp$2 = temp_const$jscomp$3[z];
  temp$jscomp$1 = temp_const$jscomp$2.call(temp_const$jscomp$3, foo());
}
a = temp$jscomp$1;
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionGetElemWithOptChainCall
#[test]
fn expose_expression_get_elem_with_opt_chain_call() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "a = x.y[z]?.(foo(), d)",
        expr_matches_str("foo()"),
        "let temp_const$jscomp$0;
let temp_const$jscomp$1;
var temp$jscomp$2;
if ((temp_const$jscomp$1 = (temp_const$jscomp$0 = x.y)[z]) == null) {
  temp$jscomp$2 = void 0;
} else {
  temp$jscomp$2 = temp_const$jscomp$1.call(temp_const$jscomp$0, foo(), d);
}
a = temp$jscomp$2;
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionOptionalGetPropWithCall
#[test]
fn expose_expression_optional_get_prop_with_call() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "a = x.y?.z(foo(1))",
        expr_matches_str("foo(1)"),
        "let temp_const$jscomp$0;
var temp$jscomp$1;
if ((temp_const$jscomp$0 = x.y) == null) {
  temp$jscomp$1 = void 0;
} else {
  var temp_const$jscomp$3 = temp_const$jscomp$0;
  var temp_const$jscomp$2 = temp_const$jscomp$3.z;
  temp$jscomp$1 = temp_const$jscomp$2.call(temp_const$jscomp$3, foo(1));
}
a = temp$jscomp$1;
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionOptionalGetPropWithCallTwiceRewriteCall
#[test]
fn expose_expression_optional_get_prop_with_call_twice_rewrite_call() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "a = x.y?.z(foo(1))",
        expr_matches_str("foo(1)"),
        "let temp_const$jscomp$0;
var temp$jscomp$1;
if ((temp_const$jscomp$0 = x.y) == null) {
  temp$jscomp$1 = void 0;
} else {
  var temp_const$jscomp$3 = temp_const$jscomp$0;
  var temp_const$jscomp$2 = temp_const$jscomp$3.z;
  temp$jscomp$1 = temp_const$jscomp$2.call(temp_const$jscomp$3, foo(1));
}
a = temp$jscomp$1;
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionGetPropWithOptChainCall
#[test]
fn expose_expression_get_prop_with_opt_chain_call() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "a = x.y.z?.(foo())",
        expr_matches_str("foo()"),
        "let temp_const$jscomp$0;
let temp_const$jscomp$1;
var temp$jscomp$2;
if ((temp_const$jscomp$1 = (temp_const$jscomp$0 = x.y).z) == null) {
  temp$jscomp$2 = void 0;
} else {
  temp$jscomp$2 = temp_const$jscomp$1.call(temp_const$jscomp$0, foo());
}
a = temp$jscomp$2;
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionNewOptChainAfterRewriteCall
#[test]
fn expose_expression_new_opt_chain_after_rewrite_call() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "a = x?.y(foo())?.z.q",
        expr_matches_str("foo()"),
        "let temp_const$jscomp$0;
let temp_const$jscomp$1;
var temp$jscomp$2;
if ((temp_const$jscomp$0 = x) == null) {
  temp$jscomp$2 = void 0;
} else {
  var temp_const$jscomp$4 = temp_const$jscomp$0;
  var temp_const$jscomp$3 = temp_const$jscomp$4.y;
  temp$jscomp$2 =
      (temp_const$jscomp$1 =
          temp_const$jscomp$3.call(temp_const$jscomp$4, foo())) == null
              ? void 0 : temp_const$jscomp$1.z.q;
}
a = temp$jscomp$2;
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionNewOptChainAfter
#[test]
fn expose_expression_new_opt_chain_after() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression("a = x?.y[foo()]?.z.q", expr_matches_str("foo()"), "let temp_const$jscomp$0;
let temp_const$jscomp$1;
var temp$jscomp$2;
if ((temp_const$jscomp$0 = x) == null) {
  temp$jscomp$2 = void 0;
} else {
  var temp_const$jscomp$3 = temp_const$jscomp$0.y;
  temp$jscomp$2 = (temp_const$jscomp$1 = temp_const$jscomp$3[foo()]) == null ? void 0 : temp_const$jscomp$1.z.q;
}
a = temp$jscomp$2;
");
}

// port: ExpressionDecomposerTest#exposeExpressionNotImmediatelyFollowedByNewChain
#[test]
fn expose_expression_not_immediately_followed_by_new_chain() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "a = x?.y[foo()].z.q?.b.c",
        expr_matches_str("foo()"),
        "let temp_const$jscomp$0;
let temp_const$jscomp$1;
var temp$jscomp$2;
if ((temp_const$jscomp$0 = x) == null) {
  temp$jscomp$2 = void 0;
} else {
  var temp_const$jscomp$3 = temp_const$jscomp$0.y;
  temp$jscomp$2 = (temp_const$jscomp$1 = temp_const$jscomp$3[foo()].z.q) == null
      ? void 0 : temp_const$jscomp$1.b.c;
}
a = temp$jscomp$2;
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionBreakingOutOfOptionalChain
#[test]
fn expose_expression_breaking_out_of_optional_chain() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "a = (x?.y[foo()]).z.q",
        expr_matches_str("foo()"),
        "let temp_const$jscomp$0;
var temp$jscomp$1;
if ((temp_const$jscomp$0 = x) == null) {
  temp$jscomp$1 = void 0;
} else {
  var temp_const$jscomp$2 = temp_const$jscomp$0.y;
  temp$jscomp$1 = temp_const$jscomp$2[foo()];
}
a = temp$jscomp$1.z.q;
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionCallBreakingOutOfOptionalChain
#[test]
fn expose_expression_call_breaking_out_of_optional_chain() {
    let mut t = ExpressionDecomposerTest::set_up();
    t.should_test_types = false;
    // Performing a non-optional call on an optional chain is not good coding, because you could
    // end up trying to call `undefined` as a function, but it is allowed and must be supported.
    t.helper_expose_expression(
        "a = (x?.y.z)(foo())",
        expr_matches_str("foo()"),
        "let temp_const$jscomp$0;
let temp_const$jscomp$1;
var temp_const$jscomp$3 =
    (temp_const$jscomp$0 = x) == null
        ? void 0 : (temp_const$jscomp$1 = temp_const$jscomp$0.y).z;
// This double .call is unfortunate but not incorrect.
// Maybe we should make the ExpressionDecomposer recognize `.call` and avoid creating
// another one? We'd run the risk of breaking \"real\" methods called `.call`, which
// are allowed.
var temp_const$jscomp$2 = temp_const$jscomp$3.call;
// The temp_const$jscomp$1 argument gets type '?' when we decompose,
// which is correct, but TypeInference on this expected code appears to give it
// `undefined`. This is why we've set `shouldTestTypes = false;` above
a = temp_const$jscomp$2.call(temp_const$jscomp$3, temp_const$jscomp$1, foo());
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionFreeCallBreakingOutOfOptionalChain
#[test]
fn expose_expression_free_call_breaking_out_of_optional_chain() {
    let t = ExpressionDecomposerTest::set_up();
    // Performing a non-optional call on an optional chain is not good coding, because you could
    // end up trying to call `undefined` as a function, but it is allowed and must be supported.
    t.helper_expose_expression(
        "a = (x?.y.z())(foo())",
        expr_matches_str("foo()"),
        "var temp_const$jscomp$0 = x?.y.z();
a = temp_const$jscomp$0(foo());
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionCallAtEndOfOptionalChain
#[test]
fn expose_expression_call_at_end_of_optional_chain() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "a = x?.y.z(foo())",
        expr_matches_str("foo()"),
        "let temp_const$jscomp$0;
var temp$jscomp$1;
if ((temp_const$jscomp$0 = x) == null) {
  temp$jscomp$1 = void 0;
} else {
  var temp_const$jscomp$3 = temp_const$jscomp$0.y;
  var temp_const$jscomp$2 = temp_const$jscomp$3.z;
  temp$jscomp$1 = temp_const$jscomp$2.call(temp_const$jscomp$3, foo());
}
a = temp$jscomp$1;
",
    );
}

// port: ExpressionDecomposerTest#testBug117935266_expose_call_target
#[test]
fn test_bug117935266_expose_call_target() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "function first() {
  alert('first');
  return '';
}
// alert must be preserved before the first side-effect
alert(first().method(alert('second')).method(alert('third')));
",
        expr_matches_str("first()"),
        "function first() {
  alert('first');
      return '';
}
var temp_const$jscomp$0 = alert;
temp_const$jscomp$0(first().method(
    alert('second')).method(alert('third')));
",
    );
}

// port: ExpressionDecomposerTest#testBug117935266_move_call_target
#[test]
fn test_bug117935266_move_call_target() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "function first() {
  alert('first');
      return '';
}
var temp_const$jscomp$0 = alert;
temp_const$jscomp$0(first().toString(
    alert('second')).toString(alert('third')));
",
        expr_matches_str("first()"),
        "function first() {
  alert('first');
      return '';
}
var temp_const$jscomp$0 = alert;
var result$jscomp$0 = first();
temp_const$jscomp$0(result$jscomp$0.toString(
    alert('second')).toString(alert('third')));
",
    );
}

// port: ExpressionDecomposerTest#testBug117935266_expose_call_parameters
#[test]
fn test_bug117935266_expose_call_parameters() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "alert(fn(first(), second(), third()));",
        expr_matches_str("first()"),
        "var temp_const$jscomp$1 = alert;
var temp_const$jscomp$0 = fn;
temp_const$jscomp$1(temp_const$jscomp$0(first(), second(), third()));
",
    );
    t.helper_expose_expression(
        "alert(fn(first(), second(), third()));",
        expr_matches_str("second()"),
        "var temp_const$jscomp$2 = alert;
var temp_const$jscomp$1 = fn;
var temp_const$jscomp$0 = first();
temp_const$jscomp$2(temp_const$jscomp$1(temp_const$jscomp$0, second(), third()));
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionAfterTwoOptionalChains
#[test]
fn expose_expression_after_two_optional_chains() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "a = x?.y.z?.q(foo());",
        expr_matches_str("foo()"),
        "let temp_const$jscomp$0;
let temp_const$jscomp$1;
var temp$jscomp$2;
if ((temp_const$jscomp$0 = x) == null) {
  temp$jscomp$2 = void 0;
} else {
  var temp$jscomp$3;
  if ((temp_const$jscomp$1 = temp_const$jscomp$0.y.z) == null) {
    temp$jscomp$3 = void 0;
  } else {
    var temp_const$jscomp$5 = temp_const$jscomp$1;
    var temp_const$jscomp$4 = temp_const$jscomp$5.q;
    temp$jscomp$3 = temp_const$jscomp$4.call(temp_const$jscomp$5, foo());
  }
  temp$jscomp$2 = temp$jscomp$3;
}
a = temp$jscomp$2;
",
    );
}

// port: ExpressionDecomposerTest#exposeAnOptionalChain
#[test]
fn expose_an_optional_chain() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "a = foo(arg1, opt?.chain())",
        expr_matches_str("opt?.chain()"),
        "var temp_const$jscomp$1 = foo;
var temp_const$jscomp$0 = arg1;
a = temp_const$jscomp$1(temp_const$jscomp$0, opt?.chain());
",
    );
}

// port: ExpressionDecomposerTest#exposePartOfAnOptionalChain
#[test]
fn expose_part_of_an_optional_chain() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(
        MOVABLE,
        // The non-optional part is fine to move.
        "nonOptional.part?.optional.chain?.continues()",
        expr_matches_str("nonOptional.part"),
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "nonOptional.part?.optional.chain?.continues()",
        // Cannot just move part of an optional chain.
        expr_matches_str("nonOptional.part?.optional"),
    );
    t.helper_expose_expression(
        "nonOptional.part?.optional.chain?.continues()",
        expr_matches_str("nonOptional.part?.optional"),
        "let temp_const$jscomp$0;
let temp_const$jscomp$1;
if ((temp_const$jscomp$0 = nonOptional.part) == null) {
  void 0;
} else {
// `temp_const$jscomp$0.optional` is the expression we were trying to expose, and it
// is now movable.
  (temp_const$jscomp$1 = temp_const$jscomp$0.optional.chain) == null
      ? void 0 : temp_const$jscomp$1.continues();
}
",
    );
}

// port: ExpressionDecomposerTest#canExposeMethodCallee
#[test]
fn can_expose_method_callee() {
    let t = ExpressionDecomposerTest::set_up();
    // TODO(b/161802885): This should probably return DECOMPOSABLE, because it is not safe to
    // replace `foo.bar` with a temporary variable containing its value.
    t.helper_can_expose_expression(MOVABLE, "foo.bar()", expr_matches_str("foo.bar"));

    // TODO(b/161802885): This should probably return DECOMPOSABLE, because it is not safe to
    // replace `foo?.bar` with a temporary variable containing its value.
    t.helper_can_expose_expression(MOVABLE, "(foo?.bar)()", expr_matches_str("foo?.bar"));
}

// port: ExpressionDecomposerTest#testExposeExpression7
#[test]
fn test_expose_expression7() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "x = goo() && foo()",
        expr_matches_str("foo()"),
        "var temp$jscomp$0; if (temp$jscomp$0 = goo()) temp$jscomp$0 = foo(); x = temp$jscomp$0;",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionNullishCoalesce
#[test]
fn expose_expression_nullish_coalesce() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "x = goo() ?? foo()",
        expr_matches_str("foo()"),
        "var temp$jscomp$1;var temp$jscomp$0;
if((temp$jscomp$1 = goo()) != null) temp$jscomp$0 = temp$jscomp$1;
else temp$jscomp$0=foo(); x = temp$jscomp$0;
",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression8
#[test]
fn test_expose_expression8() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "var x = 1 + (goo() && foo())",
        expr_matches_str("foo()"),
        "var temp$jscomp$0;
if (temp$jscomp$0 = goo()) temp$jscomp$0 = foo();
var x = 1 + temp$jscomp$0;
",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression9
#[test]
fn test_expose_expression9() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "const x = 1 + (goo() && foo())",
        expr_matches_str("foo()"),
        "var temp$jscomp$0;
if (temp$jscomp$0 = goo()) temp$jscomp$0 = foo();
const x = 1 + temp$jscomp$0;
",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression10
#[test]
fn test_expose_expression10() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "let x = 1 + (goo() && foo())",
        expr_matches_str("foo()"),
        "var temp$jscomp$0;
if (temp$jscomp$0 = goo()) temp$jscomp$0 = foo();
let x = 1 + temp$jscomp$0;
",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression11
#[test]
fn test_expose_expression11() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "if(goo() && foo());",
        expr_matches_str("foo()"),
        "var temp$jscomp$0;
if (temp$jscomp$0 = goo()) temp$jscomp$0 = foo();
if(temp$jscomp$0);
",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression12
#[test]
fn test_expose_expression12() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "switch(goo() && foo()){}",
        expr_matches_str("foo()"),
        "var temp$jscomp$0;
if (temp$jscomp$0 = goo()) temp$jscomp$0 = foo();
switch(temp$jscomp$0){}
",
    );
}

// port: ExpressionDecomposerTest#exposeExpressionNullishCoalesceSwitch
#[test]
fn expose_expression_nullish_coalesce_switch() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "switch(goo() ?? foo()){}",
        expr_matches_str("foo()"),
        "var temp$jscomp$1;var temp$jscomp$0;
if((temp$jscomp$1 = goo()) != null) temp$jscomp$0 = temp$jscomp$1;
else temp$jscomp$0 = foo(); switch(temp$jscomp$0){}
",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression13
#[test]
fn test_expose_expression13() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "switch(1 + goo() + foo()){}",
        expr_matches_str("foo()"),
        "var temp_const$jscomp$0 = 1 + goo(); switch(temp_const$jscomp$0 + foo()){}",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression_inVanillaForInitializer_simpleExpression
#[test]
fn test_expose_expression_in_vanilla_for_initializer_simple_expression() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "for (x = goo() + foo();;) {}",
        expr_matches_str("foo()"),
        "var temp_const$jscomp$0 = goo();
for (x = temp_const$jscomp$0 + foo();;) {}
",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression_inVanillaForInitializer_usingLabel
#[test]
fn test_expose_expression_in_vanilla_for_initializer_using_label() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "LABEL: for (x = goo() + foo();;) {}",
        expr_matches_str("foo()"),
        "var temp_const$jscomp$0 = goo();
LABEL: for (x = temp_const$jscomp$0 + foo();;) {}
",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression_inVanillaForInitializer_singleDeclaration_withLetOrConst
#[test]
fn test_expose_expression_in_vanilla_for_initializer_single_declaration_with_let_or_const() {
    let t = ExpressionDecomposerTest::set_up();
    for declaration_keyword in ["let", "const"] {
        t.helper_expose_expression(
            &format!("for ({declaration_keyword} x = goo() + foo();;) {{}}"),
            expr_matches_str("foo()"),
            &"var temp_const$jscomp$0 = goo();
for (DECLATION_KEYWORD x = temp_const$jscomp$0 + foo();;) {}
"
            .replace("DECLATION_KEYWORD", declaration_keyword),
        );
    }
}

// port: ExpressionDecomposerTest#testExposeExpression_inVanillaForInitializer_firstDeclaration_withLetOrConst
#[test]
fn test_expose_expression_in_vanilla_for_initializer_first_declaration_with_let_or_const() {
    let t = ExpressionDecomposerTest::set_up();
    for declaration_keyword in ["let", "const"] {
        t.helper_expose_expression(
            &format!("for ({declaration_keyword} x = goo() + foo(), y = 5;;) {{}}"),
            expr_matches_str("foo()"),
            &"var temp_const$jscomp$0 = goo();
for (DECLATION_KEYWORD x = temp_const$jscomp$0 + foo(), y = 5;;) {}
"
            .replace("DECLATION_KEYWORD", declaration_keyword),
        );
    }
}

// port: ExpressionDecomposerTest#testExposeExpression14
#[test]
fn test_expose_expression14() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "function f(){ return goo() && foo();}",
        expr_matches_str("foo()"),
        "function f() {
  var temp$jscomp$0; if (temp$jscomp$0 = goo()) temp$jscomp$0 = foo();
  return temp$jscomp$0;
}
",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression15
#[test]
fn test_expose_expression15() {
    let t = ExpressionDecomposerTest::set_up();
    // TODO(johnlenz): We really want a constant marking pass.
    // The value "goo" should be constant, but it isn't known to be so.
    t.helper_expose_expression(
        "if (goo(1, goo(2), (1 ? foo() : 0)));",
        expr_matches_str("foo()"),
        "var temp_const$jscomp$1 = goo;
var temp_const$jscomp$0 = goo(2);
var temp$jscomp$2;
if (1) temp$jscomp$2 = foo(); else temp$jscomp$2 = 0;
if (temp_const$jscomp$1(1, temp_const$jscomp$0, temp$jscomp$2));
",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression16
#[test]
fn test_expose_expression16() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "throw bar() && foo();",
        expr_matches_str("foo()"),
        "var temp$jscomp$0; if (temp$jscomp$0 = bar()) temp$jscomp$0=foo(); throw temp$jscomp$0;",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression17
#[test]
fn test_expose_expression17() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "x.foo(y())",
        expr_matches_str("y()"),
        "var temp_const$jscomp$1 = x;
var temp_const$jscomp$0 = temp_const$jscomp$1.foo;
temp_const$jscomp$0.call(temp_const$jscomp$1, y());
",
    );
}

// port: ExpressionDecomposerTest#testExposeFreeCall
#[test]
fn test_expose_free_call() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "(0,x.foo)(y())",
        expr_matches_str("y()"),
        "var temp_const$jscomp$0 = x.foo;
temp_const$jscomp$0(y());
",
    );
}

// port: ExpressionDecomposerTest#testExposeTemplateLiteralFreeCall
#[test]
fn test_expose_template_literal_free_call() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "foo`${x()}${y()}`",
        expr_matches_str("y()"),
        "var temp_const$jscomp$1 = foo;
var temp_const$jscomp$0 = x();
temp_const$jscomp$1`${temp_const$jscomp$0}${y()}`;
",
    );
}

// port: ExpressionDecomposerTest#testCanExposeTaggedTemplateLiteralInterpolation
#[test]
fn test_can_expose_tagged_template_literal_interpolation() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(DECOMPOSABLE, "x`${y()}`", expr_matches_str("y()"));
    // TODO(b/251958225): Implement decomposition for this case.
    t.helper_can_expose_expression(UNDECOMPOSABLE, "x.foo`${y()}`", expr_matches_str("y()"));
}

// port: ExpressionDecomposerTest#testExposeExpression18
#[test]
fn test_expose_expression18() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "const {a, b, c} = condition ?
  y() :
  {a: 0, b: 0, c: 1};
",
        expr_matches_str("y()"),
        "var temp$jscomp$0;
if (condition) {
  temp$jscomp$0 = y();
} else {
  temp$jscomp$0 = {a: 0, b: 0, c: 1};
}
const {a, b, c} = temp$jscomp$0;
",
    );
}

// port: ExpressionDecomposerTest#testMoveClass1
#[test]
fn test_move_class1() {
    let mut t = ExpressionDecomposerTest::set_up();
    t.should_test_types = false;
    // types don't come out quite the same before and after decomposition
    // TODO(bradfordcsmith): See TODO in helperMoveExpression()
    t.helper_move_expression(
        "alert(class X {});",
        find_class_finder(),
        "var result$jscomp$0 = class X {}; alert(result$jscomp$0);",
    );
}

// port: ExpressionDecomposerTest#testMoveClass2
#[test]
fn test_move_class2() {
    let mut t = ExpressionDecomposerTest::set_up();
    t.should_test_types = false;
    // types don't come out quite the same before and after decomposition
    // TODO(bradfordcsmith): See TODO in helperMoveExpression()
    t.helper_move_expression(
        "console.log(1, 2, class X {});",
        find_class_finder(),
        "var result$jscomp$0 = class X {}; console.log(1, 2, result$jscomp$0);",
    );
}

// port: ExpressionDecomposerTest#testMoveYieldExpression1
#[test]
fn test_move_yield_expression1() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "function *f() { return { a: yield 1, c: foo(yield 2, yield 3) }; }",
        expr_matches_str("yield 1"),
        "function *f() {
  var result$jscomp$0 = yield 1;
  return { a: result$jscomp$0, c: foo(yield 2, yield 3) };
}
",
    );
    t.helper_move_expression(
        "function *f() { return { a: 0, c: foo(yield 2, yield 3) }; }",
        expr_matches_str("yield 2"),
        "function *f() {
  var result$jscomp$0 = yield 2;
  return { a: 0, c: foo(result$jscomp$0, yield 3) };
}
",
    );
    t.helper_move_expression(
        "function *f() { return { a: 0, c: foo(1, yield 3) }; }",
        expr_matches_str("yield 3"),
        "function *f() {
  var result$jscomp$0 = yield 3;
  return { a: 0, c: foo(1, result$jscomp$0) };
}
",
    );
}

// port: ExpressionDecomposerTest#testMoveYieldExpression2
#[test]
fn test_move_yield_expression2() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "function *f() { return (yield 1) || (yield 2); }",
        expr_matches_str("yield 1"),
        "function *f() {
  var result$jscomp$0 = yield 1;
  return result$jscomp$0 || (yield 2);
}
",
    );
}

// port: ExpressionDecomposerTest#testMoveYieldExpression3
#[test]
fn test_move_yield_expression3() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "function *f() { return x.y(yield 1); }",
        expr_matches_str("yield 1"),
        "function *f() {
  var result$jscomp$0 = yield 1;
  return x.y(result$jscomp$0);
}
",
    );
}

// port: ExpressionDecomposerTest#testExposeYieldExpression1
#[test]
fn test_expose_yield_expression1() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "function *f(x) { return x || (yield 2); }",
        expr_matches_str("yield 2"),
        "function *f(x) {
  var temp$jscomp$0;
  if (temp$jscomp$0=x); else temp$jscomp$0 = yield 2;
  return temp$jscomp$0
}
",
    );
}

// port: ExpressionDecomposerTest#testExposeYieldExpression2
#[test]
fn test_expose_yield_expression2() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "function *f() { return x.y(yield 1); }",
        expr_matches_str("yield 1"),
        "function *f() {
  var temp_const$jscomp$1 = x;
  var temp_const$jscomp$0 = temp_const$jscomp$1.y;
  return temp_const$jscomp$0.call(temp_const$jscomp$1, yield 1);
}
",
    );
}

// port: ExpressionDecomposerTest#testExposeYieldExpression3
#[test]
fn test_expose_yield_expression3() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "function *f() { return g.call(yield 1); }",
        expr_matches_str("yield 1"),
        "function *f() {
  var temp_const$jscomp$1 = g;
  var temp_const$jscomp$0 = temp_const$jscomp$1.call;
  return temp_const$jscomp$0.call(temp_const$jscomp$1, yield 1);
}
",
    );
}

// port: ExpressionDecomposerTest#testExposeYieldExpression4
#[test]
fn test_expose_yield_expression4() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "function *f() { return g.apply([yield 1, yield 2]); }",
        expr_matches_str("yield 1"),
        "function *f() {
  var temp_const$jscomp$1 = g;
  var temp_const$jscomp$0 = temp_const$jscomp$1.apply;
  return temp_const$jscomp$0.call(temp_const$jscomp$1, [yield 1, yield 2]);
}
",
    );
}

// port: ExpressionDecomposerTest#testExposePlusEquals1
#[test]
fn test_expose_plus_equals1() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "var x = 0; x += foo() + 1",
        expr_matches_str("foo()"),
        "var x = 0; var temp_const$jscomp$0 = x; x = temp_const$jscomp$0 + (foo() + 1);",
    );
    t.helper_expose_expression(
        "var x = 0; y = (x += foo()) + x",
        expr_matches_str("foo()"),
        "var x = 0; var temp_const$jscomp$0 = x; y = (x = temp_const$jscomp$0 + foo()) + x",
    );
}

// port: ExpressionDecomposerTest#testExposePlusEquals2
#[test]
fn test_expose_plus_equals2() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "var x = {}; x.a += foo() + 1",
        expr_matches_str("foo()"),
        "var x = {}; var temp_const$jscomp$0 = x;
var temp_const$jscomp$1 = temp_const$jscomp$0.a;
temp_const$jscomp$0.a = temp_const$jscomp$1 + (foo() + 1);
",
    );
    t.helper_expose_expression(
        "var x = {}; y = (x.a += foo()) + x.a",
        expr_matches_str("foo()"),
        "var x = {}; var temp_const$jscomp$0 = x;
var temp_const$jscomp$1 = temp_const$jscomp$0.a;
y = (temp_const$jscomp$0.a = temp_const$jscomp$1 + foo()) + x.a
",
    );
}

// port: ExpressionDecomposerTest#testExposePlusEquals3
#[test]
fn test_expose_plus_equals3() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "/** @const */ var XX = {}; XX.a += foo() + 1",
        expr_matches_str("foo()"),
        "var XX = {};
var temp_const$jscomp$0 = XX.a;
XX.a = temp_const$jscomp$0 + (foo() + 1);
",
    );
    t.helper_expose_expression(
        "var XX = {}; y = (XX.a += foo()) + XX.a",
        expr_matches_str("foo()"),
        "var XX = {};
var temp_const$jscomp$0 = XX.a;
y = (XX.a = temp_const$jscomp$0 + foo()) + XX.a
",
    );
}

// port: ExpressionDecomposerTest#testExposePlusEquals4
#[test]
fn test_expose_plus_equals4() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "var x = {}; goo().a += foo() + 1",
        expr_matches_str("foo()"),
        "var x = {};
var temp_const$jscomp$0 = goo();
var temp_const$jscomp$1 = temp_const$jscomp$0.a;
temp_const$jscomp$0.a = temp_const$jscomp$1 + (foo() + 1);
",
    );
    t.helper_expose_expression(
        "var x = {}; y = (goo().a += foo()) + goo().a",
        expr_matches_str("foo()"),
        "var x = {};
var temp_const$jscomp$0 = goo();
var temp_const$jscomp$1 = temp_const$jscomp$0.a;
y = (temp_const$jscomp$0.a = temp_const$jscomp$1 + foo()) + goo().a
",
    );
}

// port: ExpressionDecomposerTest#testExposePlusEquals5
#[test]
fn test_expose_plus_equals5() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "var x = {}; goo().a.b += foo() + 1",
        expr_matches_str("foo()"),
        "var x = {};
var temp_const$jscomp$0 = goo().a;
var temp_const$jscomp$1 = temp_const$jscomp$0.b;
temp_const$jscomp$0.b = temp_const$jscomp$1 + (foo() + 1);
",
    );
    t.helper_expose_expression(
        "var x = {}; y = (goo().a.b += foo()) + goo().a",
        expr_matches_str("foo()"),
        "var x = {};
var temp_const$jscomp$0 = goo().a;
var temp_const$jscomp$1 = temp_const$jscomp$0.b;
y = (temp_const$jscomp$0.b = temp_const$jscomp$1 + foo()) + goo().a
",
    );
}

// port: ExpressionDecomposerTest#testExposePlusEquals6
#[test]
fn test_expose_plus_equals6() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "var obj = {}; var nextKey = function() {}; obj[nextKey()] += foo() + 1",
        expr_matches_str("foo()"),
        "var obj = {};
var nextKey = function() {};
var temp_const$jscomp$0 = obj;
var temp_const$jscomp$1 = nextKey();
var temp_const$jscomp$2 = temp_const$jscomp$0[temp_const$jscomp$1];
temp_const$jscomp$0[temp_const$jscomp$1] = temp_const$jscomp$2 + (foo() + 1);
",
    );
    t.helper_expose_expression(
        "var obj = {}; var nextKey = function() {}; y = (obj[nextKey()] += foo()) + obj[nextKey()]",
        expr_matches_str("foo()"),
        "var obj = {};
var nextKey = function() {};
var temp_const$jscomp$0 = obj;
var temp_const$jscomp$1 = nextKey();
var temp_const$jscomp$2 = temp_const$jscomp$0[temp_const$jscomp$1];
y = (temp_const$jscomp$0[temp_const$jscomp$1] = temp_const$jscomp$2 + foo()) + obj[nextKey()]
",
    );
}

// port: ExpressionDecomposerTest#testExposePlusEquals_getElemCallReceiverAndKey
#[test]
fn test_expose_plus_equals_get_elem_call_receiver_and_key() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "var x = {}; goo()[goo()] += foo() + 1",
        expr_matches_str("foo()"),
        "var x = {};
var temp_const$jscomp$0 = goo();
var temp_const$jscomp$1 = goo();
var temp_const$jscomp$2 = temp_const$jscomp$0[temp_const$jscomp$1];
temp_const$jscomp$0[temp_const$jscomp$1] = temp_const$jscomp$2 + (foo() + 1);
",
    );
}

// port: ExpressionDecomposerTest#testExposeLogicalAssignment1
#[test]
fn test_expose_logical_assignment1() {
    let t = ExpressionDecomposerTest::set_up();
    // Part of the work here is being done by Normalize, which converts all
    // instances of logical assignment operators into an larger expression
    // that separates the logical operation from the assignment.
    t.helper_expose_expression(
        "let x = 0; x ||= foo() + 1",
        expr_matches_str("foo()"),
        "let x = 0;
if (x) {
} else {
   x = foo() + 1;
}
",
    );
    t.helper_expose_expression(
        "let x = 0; x &&= foo() + 1",
        expr_matches_str("foo()"),
        "let x = 0;
if (x) {
   x = foo() + 1;
}
",
    );
    t.helper_expose_expression(
        "let x = 0; x ??= foo() + 1",
        expr_matches_str("foo()"),
        "let x = 0;
var temp$jscomp$1;
if ((temp$jscomp$1 = x) != null) {
   temp$jscomp$1;
} else {
   x = foo() + 1;
}
",
    );
}

// port: ExpressionDecomposerTest#testExposeLogicalAssignment2
#[test]
fn test_expose_logical_assignment2() {
    let t = ExpressionDecomposerTest::set_up();
    // Part of the work here is being done by Normalize, which converts all
    // instances of logical assignment operators into an larger expression
    // that separates the logical operation from the assignment.
    t.helper_expose_expression(
        "let x = {}; x.a ||= foo() + 1",
        expr_matches_str("foo()"),
        "let x = {};
let $jscomp$logical$assign$tmpm1146332801$0;
if (($jscomp$logical$assign$tmpm1146332801$0 = x).a) {
} else {
   var temp_const$jscomp$1 = $jscomp$logical$assign$tmpm1146332801$0;
   temp_const$jscomp$1.a = foo() + 1;
}
",
    );
    t.helper_expose_expression(
        "let x = {}; x[a] &&= foo() + 1",
        expr_matches_str("foo()"),
        "let x = {};
let $jscomp$logical$assign$tmpm1146332801$0;
let $jscomp$logical$assign$tmpindexm1146332801$0;
if (($jscomp$logical$assign$tmpm1146332801$0 = x)
    [$jscomp$logical$assign$tmpindexm1146332801$0 = a]) {
    var temp_const$jscomp$2 = $jscomp$logical$assign$tmpm1146332801$0;
    var temp_const$jscomp$1 = $jscomp$logical$assign$tmpindexm1146332801$0;
    temp_const$jscomp$2[temp_const$jscomp$1] = foo() + 1;
}
",
    );
}

// port: ExpressionDecomposerTest#testExposeObjectLit1
#[test]
fn test_expose_object_lit1() {
    let t = ExpressionDecomposerTest::set_up();
    // Validate that getter and setters methods are seen as side-effect
    // free and that values can move past them.  We don't need to be
    // concerned with exposing the getter or setter here but the
    // decomposer does not have a method of exposing properties, only variables.
    t.helper_move_expression(
        "var x = {get a() {}, b: foo()};",
        expr_matches_str("foo()"),
        "var result$jscomp$0=foo();var x = {get a() {}, b: result$jscomp$0};",
    );
    t.helper_move_expression(
        "var x = {set a(p) {}, b: foo()};",
        expr_matches_str("foo()"),
        "var result$jscomp$0=foo();var x = {set a(p) {}, b: result$jscomp$0};",
    );
}

// port: ExpressionDecomposerTest#testMoveSpread_siblingOfCall_outOfArrayLiteral_usesTempArray
#[test]
fn test_move_spread_sibling_of_call_out_of_array_literal_uses_temp_array() {
    let mut t = ExpressionDecomposerTest::set_up();
    t.should_test_types = false;
    // types don't come out quite the same before and after decomposition
    // TODO(bradfordcsmith): See TODO in helperMoveExpression()
    t.helper_expose_expression(
        "[...x, foo()];",
        expr_matches_str("foo()"),
        "var temp_const$jscomp$0 = [...x];
[...temp_const$jscomp$0, foo()];
",
    );
}

// port: ExpressionDecomposerTest#testMoveSpread_siblingOfCall_outOfObjectLiteral_usesTempObject
#[test]
fn test_move_spread_sibling_of_call_out_of_object_literal_uses_temp_object() {
    let mut t = ExpressionDecomposerTest::set_up();
    t.should_test_types = false;
    // types don't come out quite the same before and after decomposition
    // TODO(bradfordcsmith): See TODO in helperMoveExpression()
    t.helper_expose_expression(
        "({...x, y: foo()});",
        expr_matches_str("foo()"),
        "var temp_const$jscomp$0 = {...x};
({...temp_const$jscomp$0, y: foo()});
",
    );
}

// port: ExpressionDecomposerTest#testMoveSpread_siblingOfCall_outOfFunctionCall_usesTempArray
#[test]
fn test_move_spread_sibling_of_call_out_of_function_call_uses_temp_array() {
    let mut t = ExpressionDecomposerTest::set_up();
    t.should_test_types = false;
    // types don't come out quite the same before and after decomposition
    // TODO(bradfordcsmith): See TODO in helperMoveExpression()
    t.helper_expose_expression(
        "function f() { }
f(...x, foo());
",
        expr_matches_str("foo()"),
        "function f() { }
var temp_const$jscomp$1 = f;
var temp_const$jscomp$0 = [...x];
temp_const$jscomp$1(...temp_const$jscomp$0, foo());
",
    );
}

// port: ExpressionDecomposerTest#testMoveSpreadParent_siblingOfCall_outOfFunctionCall_usesNoTempArray
#[test]
fn test_move_spread_parent_sibling_of_call_out_of_function_call_uses_no_temp_array() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "function f() { }
f([...x], foo());
",
        expr_matches_str("foo()"),
        "function f() { }
var temp_const$jscomp$1 = f;
var temp_const$jscomp$0 = [...x];
temp_const$jscomp$1(temp_const$jscomp$0, foo());
",
    );
}

// port: ExpressionDecomposerTest#testMoveSpreadParent_siblingOfCall_outOfFunctionCall_usesNoTempObject
#[test]
fn test_move_spread_parent_sibling_of_call_out_of_function_call_uses_no_temp_object() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "function f() { }
f({...x}, foo());
",
        expr_matches_str("foo()"),
        "function f() { }
var temp_const$jscomp$1 = f;
var temp_const$jscomp$0 = {...x};
temp_const$jscomp$1(temp_const$jscomp$0, foo());
",
    );
}

// port: ExpressionDecomposerTest#testExposeExpressionInTemplateLibSub
#[test]
fn test_expose_expression_in_template_lib_sub() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "` ${ foo() }  ${ goo() } `;",
        expr_matches_str("goo()"),
        "var temp_const$jscomp$0 = foo(); ` ${ temp_const$jscomp$0 }  ${ goo() } `;",
    );
}

// port: ExpressionDecomposerTest#testExposeSubExpressionInTemplateLibSub
#[test]
fn test_expose_sub_expression_in_template_lib_sub() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "` ${ foo() + goo() } `;",
        expr_matches_str("goo()"),
        "var temp_const$jscomp$0 = foo(); ` ${ temp_const$jscomp$0 + goo() } `;",
    );
}

// port: ExpressionDecomposerTest#testMoveExpressionInTemplateLibSub
#[test]
fn test_move_expression_in_template_lib_sub() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "` ${ foo() }  ${ goo() } `;",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo(); ` ${ result$jscomp$0 }  ${ goo() } `;",
    );
}

// port: ExpressionDecomposerTest#testExposeExpression_computedProp_withPureKey
#[test]
fn test_expose_expression_computed_prop_with_pure_key() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(
        MOVABLE,
        "({
  ['a' + 'b']: foo(),
});
",
        expr_matches_str("foo()"),
    );
}

// port: ExpressionDecomposerTest#testExposeObjectLitValue_computedProp_withImpureKey
#[test]
fn test_expose_object_lit_value_computed_prop_with_impure_key() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "({
  [goo()]: foo(),
});
",
        expr_matches_str("foo()"),
        "var temp_const$jscomp$0 = goo();
({
  [temp_const$jscomp$0]: foo(),
});
",
    );
}

// port: ExpressionDecomposerTest#testExposeObjectLitValue_computedProp_asEarlierSibling_withImpureKeyAndValue
#[test]
fn test_expose_object_lit_value_computed_prop_as_earlier_sibling_with_impure_key_and_value() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "({
  [goo()]: qux(),
  bar: foo(),
});
",
        expr_matches_str("foo()"),
        "var temp_const$jscomp$1 = goo();
var temp_const$jscomp$0 = qux();
({
  [temp_const$jscomp$1]: temp_const$jscomp$0,
  bar: foo(),
});
",
    );
}

// port: ExpressionDecomposerTest#testExposeObjectLitValue_memberFunctions_asEarlierSiblings_arePure
#[test]
fn test_expose_object_lit_value_member_functions_as_earlier_siblings_are_pure() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(
        MOVABLE,
        "({
  a() { },
  get b() { },
  set b(v) { },

  bar: foo(),
});
",
        expr_matches_str("foo()"),
    );
}

// port: ExpressionDecomposerTest#testMoveSuperCall
#[test]
fn test_move_super_call() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_move_expression(
        "class A { constructor() { super(foo()) } }",
        expr_matches_str("foo()"),
        "class A{constructor(){var result$jscomp$0=foo();super(result$jscomp$0)}}",
    );
}

// port: ExpressionDecomposerTest#testMoveSuperCall_noSideEffects
#[test]
fn test_move_super_call_no_side_effects() {
    let t = ExpressionDecomposerTest::set_up();
    // String() is being used since it's known to not have side-effects.
    t.helper_move_expression(
        "class A { constructor() { super(String()) } }",
        expr_matches_str("String()"),
        "class A{constructor(){var result$jscomp$0=String();super(result$jscomp$0)}}",
    );
}

// port: ExpressionDecomposerTest#testExposeSuperCall
#[test]
fn test_expose_super_call() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_expose_expression(
        "class A { constructor() { super(goo(), foo()) } }",
        expr_matches_str("foo()"),
        "class A{ constructor(){
   var temp_const$jscomp$0=goo();
   super(temp_const$jscomp$0, foo())
}}
",
    );
}

// port: ExpressionDecomposerTest#testExposeSuperCall_noSideEffects
#[test]
fn test_expose_super_call_no_side_effects() {
    let t = ExpressionDecomposerTest::set_up();
    // String() is being used since it's known to not have side-effects.
    t.helper_expose_expression(
        "class A { constructor() { super(goo(), String()) } }",
        expr_matches_str("String()"),
        "class A{ constructor(){
   var temp_const$jscomp$0=goo();
   super(temp_const$jscomp$0, String())
}}
",
    );
}

// port: ExpressionDecomposerTest#testCannotDecomposeSuperMethodCall
#[test]
fn test_cannot_decompose_super_method_call() {
    let t = ExpressionDecomposerTest::set_up();
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "class A extends B { fn() { super.method(foo()) } }",
        expr_matches_str("foo()"),
    );
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "class A extends B { fn() { super['method'](foo()) } }",
        expr_matches_str("foo()"),
    );
}

// port: ExpressionDecomposerTest#canMovePastFnDotCall
#[test]
fn can_move_past_fn_dot_call() {
    let mut t = ExpressionDecomposerTest::set_up();
    t.known_constants.insert(JsString::from("fn"));
    t.helper_can_expose_expression(MOVABLE, "fn.call(foo());", expr_matches_str("foo()"));
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "unknownIfFn.call(foo());",
        expr_matches_str("foo()"),
    );
    t.helper_move_expression(
        "fn.call(foo());",
        expr_matches_str("foo()"),
        "var result$jscomp$0 = foo(); fn.call(result$jscomp$0);",
    );
}

// port: ExpressionDecomposerTest#testDecomposeEs6Features
#[test]
fn test_decompose_es6_features() {
    let mut t = ExpressionDecomposerTest::set_up();
    t.known_constants.insert(JsString::from("tag"));
    t.known_constants.insert(JsString::from("sink"));
    // Tagged template literals
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "sink(tag`x`, foo())",
        expr_matches_str("foo()"),
    );

    // Await expressions
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "async function f() { sink(await 0, foo()); }",
        expr_matches_str("foo()"),
    );

    // Dynamic imports
    t.helper_can_expose_expression(
        DECOMPOSABLE,
        "sink(import('m'), foo())",
        expr_matches_str("foo()"),
    );
}

/// Java's `assertThrows(IllegalStateException.class, ..)`: returns the panic message.
fn assert_throws_illegal_state(f: impl FnOnce()) -> String {
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .expect_err("expected IllegalStateException");
    if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = err.downcast_ref::<&str>() {
        s.to_string()
    } else {
        String::new()
    }
}

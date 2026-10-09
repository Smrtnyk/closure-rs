/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AstValidator.java,
//   src/com/google/javascript/jscomp/CompilerOptions.java,
//   test/com/google/javascript/jscomp/AstValidatorTest.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java.

//! Port of the `AstValidatorTest.java` tests that check hand-built trees (no corpus record).
//!
//! The record-driven tests (`valid`/`invalid`/`testFeatureValidation`/`parseValidScript`, which
//! run `testSame` with type checking) replay from corpus/unit/records.
//!
//! Java nodes belong to no compiler; here the test owns the arena the trees are built in, and
//! `doCheck` lends it to the fresh compiler it creates (swapping the arenas in and out).
use closure_jscomp::ast_validator::{AstValidator, TypeInfoValidation, ViolationHandler};
use closure_jscomp::check_level::CheckLevel;
use closure_jscomp::compiler::Compiler;
use closure_jscomp::compiler_options::CompilerOptions;
use closure_jscomp::diagnostic_groups;
use closure_jstype::JSTypeNative;
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::input_id::InputId;
use closure_rhino::ir::IR;
use closure_rhino::js_type_expression::JSTypeExpression;
use closure_rhino::jscomp_colors::standard_colors;
use closure_rhino::jsdoc_info::JSDocInfo;
use closure_rhino::node::{Ast, NodeId, ObjectProp, Prop};
use closure_rhino::simple_source_file::SimpleSourceFile;
use closure_rhino::static_source_file::SourceKind;
use closure_rhino::token::Token;
use closure_testing::compiler_test_case::CompilerTestCase;
use closure_testing::replay::replay_dsl::DslValue;
use num_bigint::BigInt;
use std::sync::Arc;

#[allow(clippy::upper_case_acronyms)] // Preserve the Java test enum spelling.
#[derive(Clone, Copy)]
enum Check {
    SCRIPT,
    STATEMENT,
    EXPRESSION,
}

/// The anonymous `ViolationHandler` of `createValidator`.
struct MessageCollector {
    messages: Vec<String>,
}
impl ViolationHandler for MessageCollector {
    // port: AstValidatorTest#createValidator (ViolationHandler#handleViolation)
    fn handle_violation(&mut self, _ast: &Ast, message: &str, _n: Option<NodeId>) {
        self.messages.push(message.to_string());
    }
}

struct AstValidatorTest {
    case: CompilerTestCase,
    /// The arena of the hand-built trees.
    ast: Ast,
    last_check_violation_messages: Vec<String>,
    type_info_validation_mode: TypeInfoValidation,
}

// port: CompilerOptions.LanguageMode (enum constant as the harness takes it)
fn language(name: &str) -> DslValue {
    DslValue::Enum {
        class: "com.google.javascript.jscomp.CompilerOptions$LanguageMode".into(),
        name: name.into(),
    }
}

impl AstValidatorTest {
    // port: AstValidatorTest#setUp
    fn set_up() -> Self {
        let mut case = CompilerTestCase::new("");
        case.set_up();
        case.disable_ast_validation().unwrap();
        case.disable_normalize().unwrap();
        case.enable_type_check().unwrap();
        Self {
            case,
            ast: Ast::new(),
            last_check_violation_messages: Vec::new(),
            type_info_validation_mode: TypeInfoValidation::JSTYPE,
        }
    }

    // port: AstValidatorTest#getOptions
    fn get_options(&self) -> CompilerOptions {
        let mut options = self.case.get_options().unwrap();
        options.set_warning_level(diagnostic_groups::MODULE_LOAD.clone(), CheckLevel::OFF);
        options
    }

    // port: CompilerTestCase#setAcceptedLanguage
    fn set_accepted_language(&mut self, lang: &str) {
        self.case.set_accepted_language(language(lang)).unwrap();
    }

    // port: CompilerTestCase#setLanguage
    fn set_language(&mut self, lang_in: &str, lang_out: &str) {
        self.case
            .set_language(language(lang_in), language(lang_out))
            .unwrap();
    }

    /// Runs `f` on a fresh compiler that holds this test's arena.
    // port: CompilerTestCase#createCompiler (with the test's nodes)
    fn with_compiler<T>(&mut self, f: impl FnOnce(&mut Compiler) -> T) -> T {
        let handle = self.case.create_compiler().unwrap();
        let mut compiler = handle.borrow_mut();
        let compiler: &mut Compiler = &mut compiler;
        std::mem::swap::<Ast>(compiler, &mut self.ast);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut *compiler)));
        std::mem::swap::<Ast>(compiler, &mut self.ast);
        result.unwrap_or_else(|e| std::panic::resume_unwind(e))
    }

    /// Perform validation check.
    // port: AstValidatorTest#doCheck
    fn do_check(&mut self, n: NodeId, level: Check) -> &[String] {
        let options = self.get_options();
        let mode = self.type_info_validation_mode;
        let messages = self.with_compiler(|compiler| {
            compiler.init_options(options);
            // port: AstValidatorTest#createValidator
            let mut collector = MessageCollector {
                messages: Vec::new(),
            };
            {
                let mut validator = AstValidator::new_with_handler(
                    compiler,
                    &mut collector,
                    /* validateScriptFeatures= */ true,
                    /* shouldValidateRequiredInlinings= */ true,
                );
                validator.set_type_validation_mode(mode);
                match level {
                    Check::SCRIPT => validator.validate_script(compiler, n),
                    Check::STATEMENT => validator.validate_statement(compiler, n),
                    Check::EXPRESSION => validator.validate_expression(compiler, n),
                }
            }
            collector.messages
        });
        self.last_check_violation_messages = messages;
        &self.last_check_violation_messages
    }

    // port: AstValidatorTest#expectInvalid(Node,Check)
    fn expect_invalid(&mut self, n: NodeId, level: Check) {
        assert!(!self.do_check(n, level).is_empty());
    }

    // port: AstValidatorTest#expectInvalid(Node,Check,String...)
    fn expect_invalid_with(&mut self, n: NodeId, level: Check, validation_messages: &[&str]) {
        // containsExactlyElementsIn: same elements with the same multiplicities, any order.
        let mut actual: Vec<String> = self.do_check(n, level).to_vec();
        let mut expected: Vec<String> = validation_messages.iter().map(|s| s.to_string()).collect();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
    }

    // port: AstValidatorTest#expectValid
    fn expect_valid(&mut self, n: NodeId, level: Check) {
        let messages = self.do_check(n, level);
        assert!(messages.is_empty(), "{messages:?}");
    }

    /// Add source location information to a tree of nodes.
    // port: AstValidatorTest#setTestSourceLocationForTree
    fn set_test_source_location_for_tree(&mut self, n: NodeId) {
        assert!(!n.is_root(&self.ast), "ROOT nodes don't get source files");
        n.set_source_file_for_testing(&mut self.ast, "testcode");
        // make sure all child nodes get the same source file.
        n.srcref_tree(&mut self.ast, n);
    }
}

// port: AstValidatorTest#testIsParenthesizedProperty
#[test]
fn test_is_parenthesized_property() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    let n = IR::string(&mut t.ast, "a");
    n.set_is_parenthesized(&mut t.ast, true);
    t.set_test_source_location_for_tree(n);

    t.expect_valid(n, Check::EXPRESSION);

    n.set_token(&mut t.ast, Token::STRING_KEY); // A string key cannot be parenthesized
    // We have to put the STRING_KEY into an object and give it a child, so we have an
    // expression to validate that is valid other than the bad parenthesized property.
    let obj_node = IR::objectlit(&mut t.ast, &[n]);
    let zero = IR::number(&mut t.ast, 0.0);
    n.add_child_to_front(&mut t.ast, zero);
    obj_node.srcref_tree(&mut t.ast, n);
    t.expect_invalid_with(
        obj_node,
        Check::EXPRESSION,
        &["non-expression is parenthesized"],
    );
}

// port: AstValidatorTest#testTopLevelForAwaitOf
#[test]
fn test_top_level_for_await_of() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    t.set_language("ECMASCRIPT_NEXT", "ECMASCRIPT5");

    let ast = &mut t.ast;
    let for_await_of_node = ast.new_node(Token::FOR_AWAIT_OF);
    let x = IR::name(ast, "x");
    for_await_of_node.add_child_to_back(ast, x);
    let y = IR::name(ast, "y");
    for_await_of_node.add_child_to_back(ast, y);
    let block = IR::block(ast);
    for_await_of_node.add_child_to_back(ast, block);

    let script = IR::script_with_children(ast, &[for_await_of_node]);
    t.set_test_source_location_for_tree(script);

    t.expect_invalid_with(
        for_await_of_node,
        Check::STATEMENT,
        &["'await' expression is not within an async function"],
    );

    // Top-level await is only allowed in modules.
    script.put_boolean_prop(&mut t.ast, Prop::ES6_MODULE, true);
    t.expect_valid(for_await_of_node, Check::STATEMENT);
}

// port: AstValidatorTest#testValidScript
#[test]
fn test_valid_script() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    let n = t.ast.new_node(Token::SCRIPT);
    t.expect_invalid(n, Check::SCRIPT);
    n.set_input_id(&mut t.ast, Some(Arc::new(InputId::new("something_input"))));
    n.set_static_source_file(
        &mut t.ast,
        Some(Arc::new(SimpleSourceFile::new(
            "something",
            SourceKind::STRONG,
        ))),
    );
    t.expect_valid(n, Check::SCRIPT);
    t.expect_invalid(n, Check::STATEMENT);
    t.expect_invalid(n, Check::EXPRESSION);
}

// port: AstValidatorTest#testValidStatement1
#[test]
fn test_valid_statement1() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    let n = t.ast.new_node(Token::RETURN);
    t.set_test_source_location_for_tree(n);
    t.expect_invalid(n, Check::EXPRESSION);
    t.expect_valid(n, Check::STATEMENT);
    t.expect_invalid(n, Check::SCRIPT);
}

// port: AstValidatorTest#testValidExpression1
#[test]
fn test_valid_expression1() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    let empty = t.ast.new_node(Token::EMPTY);
    let n = t.ast.new_node_with_child(Token::ARRAYLIT, empty);
    t.set_test_source_location_for_tree(n);
    t.expect_valid(n, Check::EXPRESSION);
    t.expect_invalid(n, Check::STATEMENT);
    t.expect_invalid(n, Check::SCRIPT);
}

// port: AstValidatorTest#testValidExpression2
#[test]
fn test_valid_expression2() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    let true_node = t.ast.new_node(Token::TRUE);
    let n = t.ast.new_node_with_child(Token::NOT, true_node);
    t.set_test_source_location_for_tree(n);
    t.expect_valid(n, Check::EXPRESSION);
    t.expect_invalid(n, Check::STATEMENT);
    t.expect_invalid(n, Check::SCRIPT);
}

// port: AstValidatorTest#testInvalidConst
#[test]
fn test_invalid_const() {
    let mut t = AstValidatorTest::set_up();
    let mut n = t.ast.new_node(Token::CONST);
    t.expect_invalid(n, Check::STATEMENT);

    let x = IR::name(&mut t.ast, "x");
    n.add_child_to_back(&mut t.ast, x);
    t.expect_invalid(n, Check::STATEMENT);

    n = t.ast.new_node(Token::CONST);
    let lhs = t.ast.new_node(Token::DESTRUCTURING_LHS);
    n.add_child_to_back(&mut t.ast, lhs);
    let pattern = t.ast.new_node(Token::OBJECT_PATTERN);
    n.get_first_child(&t.ast)
        .unwrap()
        .add_child_to_back(&mut t.ast, pattern);

    t.expect_invalid(n, Check::STATEMENT);
}

// port: AstValidatorTest#testInvalidConstLanguageLevel
#[test]
fn test_invalid_const_language_level() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    let x = IR::name(&mut t.ast, "x");
    let three = IR::number(&mut t.ast, 3.0);
    let n = IR::const_node(&mut t.ast, x, three);
    t.set_test_source_location_for_tree(n);

    t.set_accepted_language("ECMASCRIPT5");
    t.expect_invalid(n, Check::STATEMENT);

    t.set_accepted_language("ECMASCRIPT_2015");
    t.expect_valid(n, Check::STATEMENT);
}

// port: AstValidatorTest#testInvalidLetLanguageLevel
#[test]
fn test_invalid_let_language_level() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    let x = IR::name(&mut t.ast, "x");
    let three = IR::number(&mut t.ast, 3.0);
    let n = IR::let_with_value(&mut t.ast, x, three);
    t.set_test_source_location_for_tree(n);

    t.set_accepted_language("ECMASCRIPT5");
    t.expect_invalid(n, Check::STATEMENT);

    t.set_accepted_language("ECMASCRIPT_2015");
    t.expect_valid(n, Check::STATEMENT);
}

// port: AstValidatorTest#testNewTargetIsValidExpression
#[test]
fn test_new_target_is_valid_expression() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    let n = t.ast.new_node(Token::NEW_TARGET);
    t.set_test_source_location_for_tree(n);
    t.expect_valid(n, Check::EXPRESSION);
}

// port: AstValidatorTest#testImportMetaIsValidExpression
#[test]
fn test_import_meta_is_valid_expression() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    let n = t.ast.new_node(Token::IMPORT_META);
    t.set_test_source_location_for_tree(n);
    t.expect_valid(n, Check::EXPRESSION);
}

// port: AstValidatorTest#testCastOnLeftSideOfAssign
#[test]
fn test_cast_on_left_side_of_assign() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    let ast = &mut t.ast;
    let mut jsdoc = JSDocInfo::builder();
    let number = IR::string(ast, "number");
    jsdoc.record_type(Some(Arc::new(JSTypeExpression::new(
        number,
        "<AstValidatorTest>",
    ))));
    let x = IR::name(ast, "x");
    let cast = IR::cast(ast, x, jsdoc.build());
    let zero = IR::number(ast, 0.0);
    let assign = ast.new_node_with_children2(Token::ASSIGN, cast, zero);
    let n = IR::expr_result(ast, assign);
    t.set_test_source_location_for_tree(n);
    t.expect_valid(n, Check::STATEMENT);
}

// port: AstValidatorTest#testInvalidEmptyStatement
#[test]
fn test_invalid_empty_statement() {
    let mut t = AstValidatorTest::set_up();
    let true_node = t.ast.new_node(Token::TRUE);
    let n = t.ast.new_node_with_child(Token::EMPTY, true_node);
    t.set_test_source_location_for_tree(n);
    t.expect_invalid(n, Check::STATEMENT);
    n.detach_children(&mut t.ast);
    t.expect_valid(n, Check::STATEMENT);
}

// port: AstValidatorTest#testInvalidNumberStatement
#[test]
fn test_invalid_number_statement() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    let mut n = IR::number(&mut t.ast, 1.0);
    t.set_test_source_location_for_tree(n);
    t.expect_invalid(n, Check::STATEMENT);
    n = IR::expr_result(&mut t.ast, n);
    t.set_test_source_location_for_tree(n);
    t.expect_valid(n, Check::STATEMENT);
}

// port: AstValidatorTest#testInvalidBigIntStatement
#[test]
fn test_invalid_big_int_statement() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    let mut n = IR::bigint(&mut t.ast, BigInt::from(1));
    t.set_test_source_location_for_tree(n);
    t.expect_invalid(n, Check::STATEMENT);
    n = IR::expr_result(&mut t.ast, n);
    t.set_test_source_location_for_tree(n);
    t.expect_valid(n, Check::STATEMENT);
}

// port: AstValidatorTest#testAwaitExpressionNonAsyncFunction
#[test]
fn test_await_expression_non_async_function() {
    let mut t = AstValidatorTest::set_up();
    t.set_language("ECMASCRIPT_NEXT", "ECMASCRIPT5");
    let ast = &mut t.ast;
    let await_node = ast.new_node(Token::AWAIT);
    let one = IR::number(ast, 1.0);
    await_node.add_child_to_back(ast, one);
    let name = IR::name(ast, "foo");
    let params = IR::param_list(ast, &[]);
    let ret = IR::return_node_with_expression(ast, await_node);
    let body = IR::block_with_child(ast, ret);
    let parent_function = IR::function(ast, name, params, body);
    parent_function.set_is_async_function(ast, false);
    t.expect_invalid(await_node, Check::EXPRESSION);
}

// port: AstValidatorTest#testYieldExpressionNonGeneratorFunction
#[test]
fn test_yield_expression_non_generator_function() {
    let mut t = AstValidatorTest::set_up();
    let ast = &mut t.ast;
    let yield_node = ast.new_node(Token::YIELD);
    let one = IR::number(ast, 1.0);
    yield_node.add_child_to_back(ast, one);
    let name = IR::name(ast, "foo");
    let params = IR::param_list(ast, &[]);
    let ret = IR::return_node_with_expression(ast, yield_node);
    let body = IR::block_with_child(ast, ret);
    let parent_function = IR::function(ast, name, params, body);
    parent_function.set_is_generator_function(ast, false);
    t.expect_invalid(yield_node, Check::EXPRESSION);
}

// port: AstValidatorTest#testTopLevelAwait
#[test]
fn test_top_level_await() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    t.set_language("ECMASCRIPT_NEXT", "ECMASCRIPT5");
    let ast = &mut t.ast;
    let await_node = ast.new_node(Token::AWAIT);
    let one = IR::number(ast, 1.0);
    await_node.add_child_to_back(ast, one);
    let expr = IR::expr_result(ast, await_node);
    let script = IR::script_with_children(ast, &[expr]);
    t.set_test_source_location_for_tree(script);

    t.expect_invalid_with(
        await_node,
        Check::EXPRESSION,
        &["'await' expression is not within an async function"],
    );

    // Top-level await is only allowed in modules.
    script.put_boolean_prop(&mut t.ast, Prop::ES6_MODULE, true);
    t.expect_valid(await_node, Check::EXPRESSION);
}

// port: AstValidatorTest#testYieldExpressionNoFunction
#[test]
fn test_yield_expression_no_function() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    t.set_language("ECMASCRIPT_NEXT", "ECMASCRIPT5");
    let n = t.ast.new_node(Token::YIELD);
    let one = IR::number(&mut t.ast, 1.0);
    n.add_child_to_back(&mut t.ast, one);
    t.expect_invalid(n, Check::EXPRESSION);
}

// port: AstValidatorTest#testInvalidArrayPattern0
#[test]
fn test_invalid_array_pattern0() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    t.set_accepted_language("ECMASCRIPT_2015");

    // [...x = 1] = [];
    let ast = &mut t.ast;
    let x = IR::name(ast, "x");
    let default_arraylit = IR::arraylit(ast, &[]);
    let default_value = ast.new_node_with_children2(Token::DEFAULT_VALUE, x, default_arraylit);
    let iter_rest = ast.new_node_with_child(Token::ITER_REST, default_value);
    let pattern = ast.new_node_with_child(Token::ARRAY_PATTERN, iter_rest);
    let rhs = IR::arraylit(ast, &[]);
    let n = IR::assign(ast, pattern, rhs);
    t.expect_invalid(n, Check::EXPRESSION);
}

// port: AstValidatorTest#testInvalidObjectRestForLanguageLevel
#[test]
fn test_invalid_object_rest_for_language_level() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    let ast = &mut t.ast;
    let x = IR::name(ast, "x");
    let rest = IR::object_rest(ast, x);
    let pattern = IR::object_pattern(ast, &[rest]);
    let rhs = IR::objectlit(ast, &[]);
    let n = IR::assign(ast, pattern, rhs);
    t.set_test_source_location_for_tree(n);

    t.set_accepted_language("ECMASCRIPT_2015");
    t.expect_invalid(n, Check::EXPRESSION);

    t.set_accepted_language("ECMASCRIPT_2018");
    t.expect_valid(n, Check::EXPRESSION);
}

// port: AstValidatorTest#testInvalidArrayRestForLanguageLevel
#[test]
fn test_invalid_array_rest_for_language_level() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    let ast = &mut t.ast;
    let x = IR::name(ast, "x");
    let rest = IR::iter_rest(ast, x);
    let pattern = IR::array_pattern(ast, &[rest]);
    let rhs = IR::arraylit(ast, &[]);
    let n = IR::assign(ast, pattern, rhs);
    t.set_test_source_location_for_tree(n);

    t.set_accepted_language("ECMASCRIPT5");
    t.expect_invalid(n, Check::EXPRESSION);

    t.set_accepted_language("ECMASCRIPT_2015");
    t.expect_valid(n, Check::EXPRESSION);
}

// port: AstValidatorTest#testInvalidDestructuringDeclaration
#[test]
fn test_invalid_destructuring_declaration() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    t.set_accepted_language("ECMASCRIPT_2015");

    // missing a right-hand side
    let pattern = IR::object_pattern(&mut t.ast, &[]);
    let lhs = t.ast.new_node_with_child(Token::DESTRUCTURING_LHS, pattern);
    let mut n = IR::var(&mut t.ast, lhs);
    t.expect_invalid(n, Check::STATEMENT);

    let pattern = IR::object_pattern(&mut t.ast, &[]);
    let lhs = t.ast.new_node_with_child(Token::DESTRUCTURING_LHS, pattern);
    n = IR::r#let(&mut t.ast, lhs);
    t.expect_invalid(n, Check::STATEMENT);

    let pattern = IR::object_pattern(&mut t.ast, &[]);
    let lhs = t.ast.new_node_with_child(Token::DESTRUCTURING_LHS, pattern);
    n = t.ast.new_node_with_child(Token::CONST, lhs);
    t.expect_invalid(n, Check::STATEMENT);
}

// port: AstValidatorTest#testInvalidDestructuringAssignment
#[test]
fn test_invalid_destructuring_assignment() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    t.set_accepted_language("ECMASCRIPT_2015");

    let ast = &mut t.ast;
    let inner = ast.new_node(Token::ARRAY_PATTERN);
    let pattern = ast.new_node_with_child(Token::OBJECT_PATTERN, inner);
    let rhs = IR::objectlit(ast, &[]);
    let mut n = IR::assign(ast, pattern, rhs);
    t.expect_invalid(n, Check::EXPRESSION);

    let ast = &mut t.ast;
    let key = IR::string(ast, "x");
    let one = IR::number(ast, 1.0);
    let computed = IR::computed_prop(ast, key, one);
    let pattern = ast.new_node_with_child(Token::ARRAY_PATTERN, computed);
    let rhs = IR::objectlit(ast, &[]);
    n = IR::assign(ast, pattern, rhs);
    t.expect_invalid(n, Check::EXPRESSION);

    let ast = &mut t.ast;
    let stringkey = IR::string_key(ast, "x");
    let key = IR::string(ast, "x");
    let one = IR::number(ast, 1.0);
    let computed = IR::computed_prop(ast, key, one);
    stringkey.add_child_to_front(ast, computed);
    let pattern = ast.new_node_with_child(Token::OBJECT_PATTERN, stringkey);
    let rhs = IR::objectlit(ast, &[]);
    n = IR::assign(ast, pattern, rhs);
    t.expect_invalid(n, Check::EXPRESSION);
}

// port: AstValidatorTest#testSuperInvalidAtScriptLevel
#[test]
fn test_super_invalid_at_script_level() {
    let mut t = AstValidatorTest::set_up();
    let ast = &mut t.ast;
    let script_node = IR::script(ast);
    let super_node = IR::super_node(ast);
    let getprop = IR::getprop(ast, super_node, "prop");
    let super_statement_node = IR::expr_result(ast, getprop);
    script_node.add_child_to_back(ast, super_statement_node);

    t.expect_invalid(super_statement_node, Check::STATEMENT);
}

// port: AstValidatorTest#superInvalidWithOptChainCall
#[test]
fn super_invalid_with_opt_chain_call() {
    let mut t = AstValidatorTest::set_up();
    let super_node = IR::super_node(&mut t.ast);
    let opt_chain_call_node = IR::start_opt_chain_call(&mut t.ast, super_node, &[]);

    t.expect_invalid(opt_chain_call_node, Check::STATEMENT);
}

// port: AstValidatorTest#superInvalidWithOptionalGetProp
#[test]
fn super_invalid_with_optional_get_prop() {
    let mut t = AstValidatorTest::set_up();
    let super_node = IR::super_node(&mut t.ast);
    let opt_chain_get_prop_node = IR::start_opt_chain_getprop(&mut t.ast, super_node, "prop");

    t.expect_invalid(opt_chain_get_prop_node, Check::STATEMENT);
}

// port: AstValidatorTest#superInvalidWithOptionalGetElem
#[test]
fn super_invalid_with_optional_get_elem() {
    let mut t = AstValidatorTest::set_up();
    let super_node = IR::super_node(&mut t.ast);
    let expr_node = IR::name(&mut t.ast, "expr");
    let opt_chain_get_elem_node = IR::start_opt_chain_getelem(&mut t.ast, super_node, expr_node);

    t.expect_invalid(opt_chain_get_elem_node, Check::STATEMENT);
}

// port: AstValidatorTest#optChainGetPropInvalidWithNoStartOfChain
#[test]
fn opt_chain_get_prop_invalid_with_no_start_of_chain() {
    let mut t = AstValidatorTest::set_up();
    let expr = IR::name(&mut t.ast, "expr");
    let inner_get_prop = IR::continue_opt_chain_getprop(&mut t.ast, expr, "prop1");
    let outter_get_prop = IR::continue_opt_chain_getprop(&mut t.ast, inner_get_prop, "prop2");

    t.expect_invalid(outter_get_prop, Check::STATEMENT);
}

// port: AstValidatorTest#optChainGetElemInvalidWithNoStartOfChain
#[test]
fn opt_chain_get_elem_invalid_with_no_start_of_chain() {
    let mut t = AstValidatorTest::set_up();
    let ast = &mut t.ast;
    let expr = IR::name(ast, "expr");
    let prop1 = IR::name(ast, "prop1");
    let inner_get_elem = IR::continue_opt_chain_getelem(ast, expr, prop1);
    let prop2 = IR::name(ast, "prop2");
    let outter_get_elem = IR::continue_opt_chain_getelem(ast, inner_get_elem, prop2);

    t.expect_invalid(outter_get_elem, Check::STATEMENT);
}

// port: AstValidatorTest#optChainCallInvalidWithNoStartOfChain
#[test]
fn opt_chain_call_invalid_with_no_start_of_chain() {
    let mut t = AstValidatorTest::set_up();
    let ast = &mut t.ast;
    let f = IR::name(ast, "f");
    let arg1 = IR::name(ast, "arg1");
    let inner_call = IR::continue_opt_chain_call(ast, f, &[arg1]);
    let arg2 = IR::name(ast, "arg2");
    let outter_call = IR::continue_opt_chain_call(ast, inner_call, &[arg2]);

    t.expect_invalid(outter_call, Check::STATEMENT);
}

// port: AstValidatorTest#testValidFeatureInScript
#[test]
fn test_valid_feature_in_script() {
    let mut t = AstValidatorTest::set_up();
    // Since we're building the AST by hand, there won't be any types on it.
    t.type_info_validation_mode = TypeInfoValidation::NONE;

    t.set_accepted_language("ECMASCRIPT_2015");

    let n = t.ast.new_node(Token::SCRIPT);
    n.set_input_id(&mut t.ast, Some(Arc::new(InputId::new("something_input"))));
    n.set_static_source_file(
        &mut t.ast,
        Some(Arc::new(SimpleSourceFile::new(
            "something",
            SourceKind::STRONG,
        ))),
    );
    t.expect_valid(n, Check::SCRIPT);

    let a = IR::name(&mut t.ast, "a");
    let three = IR::number(&mut t.ast, 3.0);
    let let_node = IR::let_with_value(&mut t.ast, a, three);
    n.add_child_to_front(&mut t.ast, let_node);
    n.srcref_tree(&mut t.ast, n);
    t.expect_invalid(n, Check::SCRIPT);

    n.put_prop(
        &mut t.ast,
        Prop::FEATURE_SET,
        Some(ObjectProp::Opaque(Arc::new(
            FeatureSet::BARE_MINIMUM.with(Feature::LET_DECLARATIONS),
        ))),
    );
    t.expect_valid(n, Check::SCRIPT);

    t.set_accepted_language("ECMASCRIPT3"); // resets compiler's allowable featureSet
    t.expect_invalid(n, Check::SCRIPT);
    // violation reported from {@code validateFeature} call of LET because
    // `!allowbleFeatures.has(feature)`
    assert!(
        t.last_check_violation_messages
            .contains(&"AST should not contain let declaration".to_string())
    );
    // violation reported from {@code validateScript} call because script's `FEATURE_SET` is not
    // a subset of compiler's allowable featureSet.
    assert!(t.last_check_violation_messages.contains(
        &"SCRIPT node contains these unallowable features:[let declaration]".to_string()
    ));
}

// port: AstValidatorTest#testValidatesColorInfoOnExpression
#[test]
fn test_validates_color_info_on_expression() {
    let mut t = AstValidatorTest::set_up();
    t.type_info_validation_mode = TypeInfoValidation::COLOR;

    let ast = &mut t.ast;
    let foo = IR::string(ast, "foo");
    let expr = IR::expr_result(ast, foo);
    let script = IR::script_with_children(ast, &[expr]);
    t.set_test_source_location_for_tree(script);

    t.expect_invalid(expr, Check::STATEMENT);

    t.with_compiler(|compiler| {
        let string_type = compiler
            .get_type_registry()
            .get_native_type(JSTypeNative::STRING_TYPE);
        foo.set_jstype(compiler, Some(string_type));
    });
    t.expect_invalid(expr, Check::STATEMENT);

    foo.set_jstype(&mut t.ast, None);
    foo.set_color(&mut t.ast, Some(standard_colors::STRING.clone()));

    t.expect_valid(expr, Check::STATEMENT);
}

// port: AstValidatorTest#checkRequiredInlinings_failsWithRemainingRequiredInlinings
#[test]
fn check_required_inlinings_fails_with_remaining_required_inlinings() {
    let mut t = AstValidatorTest::set_up();
    // Write a reference to a function that required inlining. We need an enclosing statement
    // because the expression checker will look for a parent.
    let ast = &mut t.ast;
    let my_function = IR::name(ast, "myFunction");
    let expr = IR::expr_result(ast, my_function);
    let script = IR::script_with_children(ast, &[expr]);
    IR::root(ast, &[script]);

    // Add the requireInlining annotation.
    let mut js_doc_info = JSDocInfo::builder();
    js_doc_info.record_require_inlining();
    my_function.set_jsdoc_info(ast, js_doc_info.build());

    // A surviving requireInlining annotation should cause a validation failure.
    t.expect_invalid_with(
        my_function,
        Check::EXPRESSION,
        &[
            "Type information missing\n/** @requireInlining */ myFunction",
            "@requireInlining node failed to be inlined.",
            "Type information missing\n/** @requireInlining */ myFunction",
        ],
    );
}

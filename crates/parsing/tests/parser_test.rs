/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2014 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeUtil.java,
//   test/com/google/javascript/jscomp/parsing/ParserTest.java.

#![allow(unused_mut)] // Java locals are mutable; retaining their declaration order aids source review.
use closure_jscomp::{Compiler, testing::code_sub_tree::CodeSubTree};
use closure_jstype::{prelude::*, testing::base_js_type_test_case::BaseJSTypeTestCase};
use closure_parsing::{
    config::{Config, JsDocParsing, LanguageMode, RunMode, StrictMode},
    ir_factory::IRFactory,
    js_doc_info_parser::BAD_TYPE_WIKI_LINK,
    parser::{
        feature_set::{Feature, FeatureSet},
        trees::comment::Comment,
    },
    parser_runner::{ParseResult, ParserRunner},
};
use closure_rhino::{
    error_reporter::ErrorReporter,
    ir::IR,
    js_string::JsString,
    node::{Ast, JsDocComparison, NodeId, RecursionMode, SideEffectComparison, TypeComparison},
    simple_source_file::SimpleSourceFile,
    static_source_file::SourceKind,
    testing::test_error_reporter::TestErrorReporter,
    token::Token,
};
use num_bigint::BigInt;
use std::{collections::VecDeque, sync::Arc};

struct Harness {
    types: BaseJSTypeTestCase,
    mode: LanguageMode,
    parsing_mode: JsDocParsing,
    strict_mode: StrictMode,
    is_ide_mode: bool,
    expected_features: FeatureSet,
}
impl std::ops::Deref for Harness {
    type Target = BaseJSTypeTestCase;
    fn deref(&self) -> &Self::Target {
        &self.types
    }
}
impl std::ops::DerefMut for Harness {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.types
    }
}
impl Harness {
    // port: ParserTest#setUp
    fn new() -> Self {
        Self {
            types: BaseJSTypeTestCase::new(),
            mode: LanguageMode::UNSUPPORTED,
            parsing_mode: JsDocParsing::INCLUDE_DESCRIPTIONS_NO_WHITESPACE,
            strict_mode: StrictMode::STRICT,
            is_ide_mode: false,
            expected_features: FeatureSet::BARE_MINIMUM,
        }
    }
    // port: ParserTest#assertNodeHasJSDocInfoWithJSType
    fn assert_node_has_jsdoc_info_with_js_type(&mut self, node: NodeId, js_type: TypeId) {
        let info = node.get_jsdoc_info(&self.types.ast);
        assert!(
            info.is_some(),
            "Node has no JSDocInfo: {}",
            node.to_string(&self.types.ast)
        );
        self.types
            .assert_type_equals_expression(js_type, &info.unwrap().get_type().unwrap());
    }
    // port: ParserTest#parseError
    fn parse_error(&mut self, source: impl Into<JsString>, errors: &[&str]) -> Option<NodeId> {
        assert!(!errors.is_empty());
        let mut reporter = TestErrorReporter::new();
        reporter.expect_all_errors(errors);
        let config = self.create_config();
        let result = ParserRunner::parse(
            &mut self.types.ast,
            Arc::new(SimpleSourceFile::new("input", SourceKind::STRONG)),
            source.into(),
            &config,
            &mut reporter,
        );
        assert!(result.features.contains(self.expected_features));
        reporter.verify_has_encountered_all_warnings_and_errors();
        result.ast
    }
    // port: ParserTest#parseWarning
    fn parse_warning(&mut self, source: impl Into<JsString>, warnings: &[&str]) -> NodeId {
        self.do_parse(source, warnings).ast.unwrap()
    }
    // port: ParserTest#doParse(String, String...)
    fn do_parse(&mut self, source: impl Into<JsString>, warnings: &[&str]) -> ParseResult {
        let mut reporter = TestErrorReporter::new();
        reporter.expect_all_warnings(warnings);
        self.do_parse_with_reporter(source, &mut reporter)
    }
    // port: ParserTest#doParse(String, TestErrorReporter)
    fn do_parse_with_reporter(
        &mut self,
        source: impl Into<JsString>,
        reporter: &mut TestErrorReporter,
    ) -> ParseResult {
        let config = self.create_config();
        let result = ParserRunner::parse(
            &mut self.types.ast,
            Arc::new(SimpleSourceFile::new("input", SourceKind::STRONG)),
            source.into(),
            &config,
            reporter,
        );
        assert!(result.features.contains(self.expected_features));
        reporter.verify_has_encountered_all_warnings_and_errors();
        self.assert_source_info_present(result.ast.unwrap());
        result
    }
    // port: ParserTest#assertSourceInfoPresent
    fn assert_source_info_present(&self, node: NodeId) {
        let mut deque = VecDeque::from([node]);
        while let Some(node) = deque.pop_front() {
            assert!(
                node.get_lineno(&self.types.ast) >= 0,
                "Source information must be present on {}",
                node.to_string(&self.types.ast)
            );
            assert!(
                node.get_length(&self.types.ast) >= 0,
                "Source length must be nonnegative on {}",
                node.to_string(&self.types.ast)
            );
            deque.extend(node.children(&self.types.ast));
        }
    }
    // port: ParserTest#parse
    fn parse(&mut self, source: impl Into<JsString>) -> NodeId {
        self.parse_warning(source, &[])
    }
    // port: ParserTest#parseComments
    fn parse_comments(&mut self, source: impl Into<JsString>) -> Vec<Comment> {
        self.do_parse(source, &[]).comments
    }
    // port: ParserTest#createConfig
    fn create_config(&self) -> Config {
        if self.is_ide_mode {
            ParserRunner::create_config_full(
                self.mode,
                self.parsing_mode,
                RunMode::KEEP_GOING,
                None,
                true,
                self.strict_mode,
            )
        } else {
            ParserRunner::create_config(self.mode, None, self.strict_mode)
        }
    }
    // port: ParserTest#expectFeatures
    fn expect_features(&mut self, features: &[Feature]) {
        self.expected_features = FeatureSet::BARE_MINIMUM.with_features(features);
    }
    // port: ParserTest#testLinenoCharnoBinop
    fn test_lineno_charno_binop(&mut self, binop: &str) {
        let op = self
            .parse(format!("var a = 89 {binop} 76;"))
            .get_first_child(&self.types.ast)
            .unwrap()
            .get_first_first_child(&self.types.ast)
            .unwrap();
        assert_eq!(op.get_lineno(&self.types.ast), 1);
        assert_eq!(op.get_charno(&self.types.ast), 8);
    }
    // port: ParserTest#testMethodInObjectLiteral
    fn test_method_in_object_literal(&mut self, js: &str) {
        self.mode = LanguageMode::ECMASCRIPT_2015;
        self.strict_mode = StrictMode::SLOPPY;
        self.parse(js);
        self.mode = LanguageMode::ECMASCRIPT5;
        let warning = requires_language_mode_message(Feature::MEMBER_DECLARATIONS);
        self.parse_warning(js, &[&warning]);
    }
    // port: ParserTest#testShorthandObjectProperty
    fn test_shorthand_object_property(&mut self, js: &str) {
        self.mode = LanguageMode::ECMASCRIPT_2015;
        self.strict_mode = StrictMode::SLOPPY;
        self.parse(js);
        self.mode = LanguageMode::ECMASCRIPT5;
        let warning = requires_language_mode_message(Feature::SHORTHAND_OBJECT_PROPERTIES);
        self.parse_warning(js, &[&warning]);
    }
    // port: ParserTest#testComputedProperty
    fn test_computed_property(&mut self, js: &str) {
        self.mode = LanguageMode::ECMASCRIPT_2015;
        self.strict_mode = StrictMode::SLOPPY;
        self.parse(js);
        self.mode = LanguageMode::ECMASCRIPT5;
        let warning = requires_language_mode_message(Feature::COMPUTED_PROPERTIES);
        self.parse_warning(js, &[&warning]);
    }
    // port: ParserTest#testTemplateLiteral
    fn test_template_literal(&mut self, s: &str) -> NodeId {
        self.mode = LanguageMode::ECMASCRIPT5;
        self.strict_mode = StrictMode::SLOPPY;
        let warning = requires_language_mode_message(Feature::TEMPLATE_LITERALS);
        self.parse_warning(s, &[&warning]);
        self.mode = LanguageMode::ECMASCRIPT_2015;
        self.parse(s)
    }
    // port: ParserTest#assertSimpleTemplateLiteral
    fn assert_simple_template_literal(&mut self, expected_contents: &str, literal: &str) {
        let node = self
            .test_template_literal(literal)
            .get_first_first_child(&self.types.ast)
            .unwrap();
        assert_eq!(node.get_token(&self.types.ast), Token::TEMPLATELIT);
        assert_eq!(node.get_child_count(&self.types.ast), 1);
        let first = node.get_first_child(&self.types.ast).unwrap();
        assert_eq!(first.get_token(&self.types.ast), Token::TEMPLATELIT_STRING);
        assert_eq!(
            first.get_cooked_string(&self.types.ast),
            Some(JsString::from(expected_contents))
        );
    }
    // port: ParserTest#doAsyncArrowFunctionTest
    fn do_async_arrow_function_test(&mut self, arrow_function_source: &str) {
        self.expect_features(&[Feature::ASYNC_FUNCTIONS, Feature::ARROW_FUNCTIONS]);
        for m in LanguageMode::VALUES {
            self.mode = m;
            self.strict_mode = if m == LanguageMode::ECMASCRIPT3 {
                StrictMode::SLOPPY
            } else {
                StrictMode::STRICT
            };
            if m.feature_set().has(Feature::ASYNC_FUNCTIONS) {
                self.parse(arrow_function_source);
            } else if m.feature_set().has(Feature::ARROW_FUNCTIONS) {
                let warning = requires_language_mode_message(Feature::ASYNC_FUNCTIONS);
                self.parse_warning(arrow_function_source, &[&warning]);
            } else {
                let arrow = requires_language_mode_message(Feature::ARROW_FUNCTIONS);
                let async_fn = requires_language_mode_message(Feature::ASYNC_FUNCTIONS);
                self.parse_warning(arrow_function_source, &[&arrow, &async_fn]);
            }
        }
    }
}
// port: ParserTest#requiresLanguageModeMessage
fn requires_language_mode_message(feature: Feature) -> String {
    IRFactory::language_feature_warning_message(feature)
}
// port: ParserTest#script
// port: ParserTest#createScript
fn script(ast: &mut Ast, stmt: NodeId) -> NodeId {
    ast.new_node_with_child(Token::SCRIPT, stmt)
}
// port: ParserTest#expr
fn expr(ast: &mut Ast, n: NodeId) -> NodeId {
    ast.new_node_with_child(Token::EXPR_RESULT, n)
}
// port: ParserTest#regex
fn regex(ast: &mut Ast, text: impl Into<JsString>, flag: Option<&str>) -> NodeId {
    let text = ast.new_string(text);
    let mut children = vec![text];
    if let Some(flag) = flag {
        children.push(ast.new_string(flag));
    }
    let result = ast.new_node(Token::REGEXP);
    for child in children {
        result.add_child_to_back(ast, child);
    }
    result
}
// port: ParserTest#freeCall
fn free_call(ast: &mut Ast, n: NodeId) -> NodeId {
    n.put_boolean_prop(ast, NodeId::FREE_CALL, true);
    n
}
// NodeSubject#isEqualToInternal, which names Closure's Rhino-derived NodeSubject (MPL-1.1 /
// GPL-2.0-or-later), is in its own file.
#[path = "rhino/parser_test.rs"]
mod rhino;

// port: ParserTest#assertNodeEquality
fn assert_node_equality(ast: &Ast, expected: NodeId, found: NodeId) {
    rhino::is_equal_to(ast, expected, found);
}
// port: ParserTest#assertNodeHasJSDocInfoWithNoJSType
fn assert_node_has_jsdoc_info_with_no_js_type(ast: &Ast, node: NodeId) {
    let info = node.get_jsdoc_info(ast).expect("Node has no JSDocInfo");
    assert!(info.get_type().is_none(), "JSDoc unexpectedly has type");
}
// port: ParserTest#assertNodeHasNoJSDocInfo
fn assert_node_has_no_jsdoc_info(ast: &Ast, node: NodeId) {
    assert!(
        node.get_jsdoc_info(ast).is_none(),
        "Node has unexpected JSDocInfo"
    );
}
// port: NodeUtil#getSourceName
#[allow(dead_code)] // All callers in ParserTest require the pending JSType registry assertions.
fn get_source_name(ast: &Ast, mut n: NodeId) -> Option<String> {
    loop {
        if let Some(source) = n.get_source_file_name(ast) {
            return Some(source);
        }
        n = n.get_parent(ast)?;
    }
}
/// `CodeSubTree.findFirstNode` (crates/jscomp/src/testing/code_sub_tree.rs) on a compiler that
/// holds this test's arena for the call (Java nodes belong to no compiler).
fn find_first_node(
    ast: &mut Ast,
    root_node: NodeId,
    predicate: fn(NodeId, &Ast) -> bool,
) -> NodeId {
    let mut compiler = Compiler::new();
    std::mem::swap::<Ast>(&mut compiler, ast);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        CodeSubTree::find_first_node(&mut compiler, root_node, |ast, node| predicate(node, ast))
    }));
    std::mem::swap::<Ast>(&mut compiler, ast);
    result.unwrap_or_else(|e| std::panic::resume_unwind(e))
}
// port: NodeUtil#getClassMembers
fn get_class_members(ast: &Ast, n: NodeId) -> NodeId {
    assert!(n.is_class(ast));
    n.get_last_child(ast).unwrap()
}

const TRAILING_COMMA_MESSAGE: &str =
    "Trailing comma is not legal in an ECMA-262 object initializer";
const MISSING_GT_MESSAGE: &str = "Bad type annotation. missing closing > See https://github.com/google/closure-compiler/wiki/Annotating-JavaScript-for-the-Closure-Compiler for more information.";
const UNNECESSARY_BRACES_MESSAGE: &str = "Bad type annotation. braces are not required here See https://github.com/google/closure-compiler/wiki/Annotating-JavaScript-for-the-Closure-Compiler for more information.";
const NAME_NOT_RECOGNIZED_MESSAGE: &str = "name not recognized due to syntax error.";
const UNLABELED_BREAK: &str = "unlabelled break must be inside loop or switch";
const UNEXPECTED_CONTINUE: &str = "continue must be inside loop";
const UNEXPECTED_RETURN: &str = "return must be inside function";
const UNEXPECTED_LABELLED_CONTINUE: &str = "continue can only use labels of iteration statements";
const UNEXPECTED_YIELD: &str = "yield must be inside generator function";
const UNEXPECTED_AWAIT: &str = "await must be inside asynchronous function";
const UNEXPECTED_FOR_AWAIT_OF: &str = "'for-await-of' used in a non-async function context";
const UNDEFINED_LABEL: &str = "undefined label";
const HTML_COMMENT_WARNING: &str = "In some cases, '<!--' and '-->' are treated as a '//' for legacy reasons. Removing this from your code is safe for all browsers currently in use.";
const INVALID_ASSIGNMENT_TARGET: &str = "invalid assignment target";
const INVALID_FOR_AWAIT_INIT: &str = "for-await-of statement may not have initializer";
const INVALID_FOR_AWAIT_MULT_DECLS: &str =
    "for-await-of statement may not have more than one variable declaration";
const SEMICOLON_EXPECTED: &str = "Semi-colon expected";
const INVALID_PRIVATE_ID: &str = "Private identifiers may not be used in this context";
const PRIVATE_FIELD_NOT_DEFINED: &str = "Private fields must be declared in an enclosing class";
const PRIVATE_METHOD_NOT_DEFINED: &str = "Private methods must be declared in an enclosing class";
const PRIVATE_FIELD_DELETED: &str = "Private fields cannot be deleted";
const STRING_CONTINUATIONS_WARNING: &str = "String continuations are not recommended. See https://google.github.io/styleguide/jsguide.html#features-strings-no-line-continuations";

// port: ParserTest#testParseUnescapedLineSep
#[test]
fn test_parse_unescaped_line_sep() {
    let mut h = Harness::new();
    let tmp1 = h.parse("` `;");
    let _ = tmp1;
    h.expect_features(&[Feature::UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP]);
    let tmp2 = h.parse("\" \";");
    let _ = tmp2;
    let tmp3 = h.parse("' ';");
    let _ = tmp3;
}

// port: ParserTest#testParseUnescapedParagraphSep
#[test]
fn test_parse_unescaped_paragraph_sep() {
    let mut h = Harness::new();
    let tmp1 = h.parse("` `;");
    let _ = tmp1;
    h.expect_features(&[Feature::UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP]);
    let tmp2 = h.parse("\" \";");
    let _ = tmp2;
    let tmp3 = h.parse("' ';");
    let _ = tmp3;
}

// port: ParserTest#testOptionalCatchBinding
#[test]
fn test_optional_catch_binding() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::OPTIONAL_CATCH_BINDING]);
    let tmp1 = h.parse("try {} catch {}");
    let _ = tmp1;
    let tmp2 = h.parse("try {} catch {} finally {}");
    let _ = tmp2;
}

// port: ParserTest#testOptionalCatchBindingSourceInfo
#[test]
fn test_optional_catch_binding_source_info() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::OPTIONAL_CATCH_BINDING]);
    let tmp1 = h.parse("try {} catch     {}");
    let mut result = tmp1;
    let mut catch_node = result
        .get_first_first_child(&h.ast)
        .unwrap()
        .get_next(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(catch_node.unwrap().get_token(&h.ast), Token::CATCH);
    let mut empty_node = catch_node.unwrap().get_first_child(&h.ast);
    assert_eq!(empty_node.unwrap().get_token(&h.ast), Token::EMPTY);
    assert_eq!(empty_node.unwrap().get_length(&h.ast), 5_i32);
}

// port: ParserTest#testExponentOperator
#[test]
fn test_exponent_operator() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "-x**y",
        &["Unary operator '-' requires parentheses before '**'"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "+x**y",
        &["Unary operator '+' requires parentheses before '**'"],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "!x**y",
        &["Unary operator '!' requires parentheses before '**'"],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "~x**y",
        &["Unary operator '~' requires parentheses before '**'"],
    );
    let _ = tmp4;
    let tmp5 = h.parse_error(
        "typeof x**y",
        &["Unary operator 'typeof' requires parentheses before '**'"],
    );
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "void x**y",
        &["Unary operator 'void' requires parentheses before '**'"],
    );
    let _ = tmp6;
    let tmp7 = h.parse_error(
        "delete x**y",
        &["Unary operator 'delete' requires parentheses before '**'"],
    );
    let _ = tmp7;
    let tmp8 = h.parse_error(
        "await x**y",
        &["Unary operator 'await' requires parentheses before '**'"],
    );
    let _ = tmp8;
    let tmp9 = h.parse_error(
        "async function f() { await x ** y; }",
        &["Unary operator 'await' requires parentheses before '**'"],
    );
    let _ = tmp9;
    let tmp10 = h.parse_error(
        "async () => { await x ** y; };",
        &["Unary operator 'await' requires parentheses before '**'"],
    );
    let _ = tmp10;
    let tmp11 = h.parse_error(
        "({ async f() { await x ** y; } });",
        &["Unary operator 'await' requires parentheses before '**'"],
    );
    let _ = tmp11;
    let tmp12 = h.parse_error(
        "class C { async m() { await x ** y; } }",
        &["Unary operator 'await' requires parentheses before '**'"],
    );
    let _ = tmp12;
    h.expect_features(&[Feature::EXPONENT_OP]);
    let tmp13 = h.parse("x**y");
    let _ = tmp13;
    let tmp14 = h.parse("-(x**y)");
    let _ = tmp14;
    let tmp15 = h.parse("(-x)**y");
    let _ = tmp15;
    let tmp16 = h.parse("async function f() { (await x) ** y; }");
    let _ = tmp16;
    let tmp17 = h.parse("x**-y");
    let _ = tmp17;
    let tmp18 = h.parse("async function f() { x ** await y; }");
    let _ = tmp18;
    let tmp19 = h.parse("x/y**z");
    let _ = tmp19;
    let tmp20 = h.parse("2 ** 3 > 3");
    let _ = tmp20;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp21 = h.parse_warning(
        "x**y",
        &[&requires_language_mode_message(Feature::EXPONENT_OP)],
    );
    let _ = tmp21;
}

// port: ParserTest#testExponentAssignmentOperator
#[test]
fn test_exponent_assignment_operator() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::EXPONENT_OP]);
    let tmp1 = h.parse("x**=y;");
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp2 = h.parse_warning(
        "x**=y;",
        &[&requires_language_mode_message(Feature::EXPONENT_OP)],
    );
    let _ = tmp2;
}

// port: ParserTest#testFunction
#[test]
fn test_function() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var f = function(x,y,z) { return 0; }");
    let _ = tmp1;
    let tmp2 = h.parse("function f(x,y,z) { return 0; }");
    let _ = tmp2;
    h.is_ide_mode = true;
    let tmp3 = h.parse_error("function f(x y z) {}", &["',' expected"]);
    let _ = tmp3;
}

// port: ParserTest#testFunctionTrailingComma
#[test]
fn test_function_trailing_comma() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var f = function(x,y,z,) {}");
    let _ = tmp1;
    let tmp2 = h.parse("function f(x,y,z,) {}");
    let _ = tmp2;
}

// port: ParserTest#testFunctionTrailingCommaPreES8
#[test]
fn test_function_trailing_comma_pre_es8() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT_2016;
    let tmp1 = h.parse_error(
        "var f = function(x,y,z,) {}",
        &["Invalid trailing comma in formal parameter list"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "function f(x,y,z,) {}",
        &["Invalid trailing comma in formal parameter list"],
    );
    let _ = tmp2;
}

// port: ParserTest#testFunctionExtraTrailingComma
#[test]
fn test_function_extra_trailing_comma() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("var f = function(x,y,z,,) {}", &["')' expected"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("function f(x,y,z,,) {}", &["')' expected"]);
    let _ = tmp2;
}

// port: ParserTest#testCallTrailingComma
#[test]
fn test_call_trailing_comma() {
    let mut h = Harness::new();
    let tmp1 = h.parse("f(x,y,z,);");
    let _ = tmp1;
}

// port: ParserTest#testCallTrailingCommaPreES8
#[test]
fn test_call_trailing_comma_pre_es8() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT_2016;
    let tmp1 = h.parse_error("f(x,y,z,);", &["Invalid trailing comma in arguments list"]);
    let _ = tmp1;
}

// port: ParserTest#testCallExtraTrailingComma
#[test]
fn test_call_extra_trailing_comma() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("f(x,y,z,,);", &["')' expected"]);
    let _ = tmp1;
}

// port: ParserTest#testTrailingCommaArray
#[test]
fn test_trailing_comma_array() {
    let mut h = Harness::new();
    let tmp1 = h.parse("[1, 2, 3,]");
    let mut array_lit = tmp1.get_only_child(&h.ast).get_only_child(&h.ast);
    assert_eq!(array_lit.get_token(&h.ast), Token::ARRAYLIT);
    assert!(array_lit.has_trailing_comma(&h.ast));
}

// port: ParserTest#testTrailingCommaObject
#[test]
fn test_trailing_comma_object() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var obj = {a:1,b:2,};");
    let mut objectlit = tmp1
        .get_only_child(&h.ast)
        .get_only_child(&h.ast)
        .get_only_child(&h.ast);
    assert_eq!(objectlit.get_token(&h.ast), Token::OBJECTLIT);
    assert!(objectlit.has_trailing_comma(&h.ast));
}

// port: ParserTest#testTrailingCommaParamList
#[test]
fn test_trailing_comma_param_list() {
    let mut h = Harness::new();
    let tmp1 = h.parse("function f(a, b,) {}");
    let mut param_list = tmp1.get_only_child(&h.ast).get_second_child(&h.ast);
    assert_eq!(param_list.unwrap().get_token(&h.ast), Token::PARAM_LIST);
    assert!(param_list.unwrap().has_trailing_comma(&h.ast));
}

// port: ParserTest#testTrailingCommaParamListArrow
#[test]
fn test_trailing_comma_param_list_arrow() {
    let mut h = Harness::new();
    let tmp1 = h.parse("x = (a, b,) => {};");
    let mut param_list = tmp1
        .get_only_child(&h.ast)
        .get_only_child(&h.ast)
        .get_second_child(&h.ast)
        .unwrap()
        .get_second_child(&h.ast);
    assert_eq!(param_list.unwrap().get_token(&h.ast), Token::PARAM_LIST);
    assert!(param_list.unwrap().has_trailing_comma(&h.ast));
    let tmp2 = h.parse("x = (a,) => {};");
    param_list = tmp2
        .get_only_child(&h.ast)
        .get_only_child(&h.ast)
        .get_second_child(&h.ast)
        .unwrap()
        .get_second_child(&h.ast);
    assert_eq!(param_list.unwrap().get_token(&h.ast), Token::PARAM_LIST);
    assert!(param_list.unwrap().has_trailing_comma(&h.ast));
    let tmp3 = h.parse("x = async (a, b,) => {};");
    param_list = tmp3
        .get_only_child(&h.ast)
        .get_only_child(&h.ast)
        .get_second_child(&h.ast)
        .unwrap()
        .get_second_child(&h.ast);
    assert_eq!(param_list.unwrap().get_token(&h.ast), Token::PARAM_LIST);
    assert!(param_list.unwrap().has_trailing_comma(&h.ast));
}

// port: ParserTest#testTrailingCommaCallNode
#[test]
fn test_trailing_comma_call_node() {
    let mut h = Harness::new();
    let tmp1 = h.parse("f(a, b,);");
    let mut call = tmp1.get_only_child(&h.ast).get_only_child(&h.ast);
    assert_eq!(call.get_token(&h.ast), Token::CALL);
    assert!(call.has_trailing_comma(&h.ast));
}

// port: ParserTest#testTrailingCommaNewNode
#[test]
fn test_trailing_comma_new_node() {
    let mut h = Harness::new();
    let tmp1 = h.parse("new f(a, b,);");
    let mut call = tmp1.get_only_child(&h.ast).get_only_child(&h.ast);
    assert_eq!(call.get_token(&h.ast), Token::NEW);
    assert!(call.has_trailing_comma(&h.ast));
}

// port: ParserTest#testTrailingCommaOptChainCallNode
#[test]
fn test_trailing_comma_opt_chain_call_node() {
    let mut h = Harness::new();
    let tmp1 = h.parse("f?.(a, b,);");
    let mut call = tmp1.get_only_child(&h.ast).get_only_child(&h.ast);
    assert_eq!(call.get_token(&h.ast), Token::OPTCHAIN_CALL);
    assert!(call.has_trailing_comma(&h.ast));
}

// port: ParserTest#testArrayWithElisions
#[test]
fn test_array_with_elisions() {
    let mut h = Harness::new();
    let tmp1 = h.parse("[  , 1,   ,]");
    let mut array_lit = tmp1.get_only_child(&h.ast).get_only_child(&h.ast);
    assert_eq!(array_lit.get_token(&h.ast), Token::ARRAYLIT);
    assert_eq!(array_lit.get_child_count(&h.ast), 3);
    assert_eq!(
        array_lit.get_first_child(&h.ast).unwrap().get_token(&h.ast),
        Token::EMPTY
    );
    assert_eq!(
        array_lit
            .get_first_child(&h.ast)
            .unwrap()
            .get_charno(&h.ast),
        3_i32
    );
    assert_eq!(
        array_lit
            .get_first_child(&h.ast)
            .unwrap()
            .get_length(&h.ast),
        0_i32
    );
    assert_eq!(
        array_lit
            .get_second_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::NUMBER
    );
    assert_eq!(
        array_lit
            .get_child_at_index(&h.ast, 2)
            .unwrap()
            .get_token(&h.ast),
        Token::EMPTY
    );
    assert_eq!(
        array_lit
            .get_child_at_index(&h.ast, 2)
            .unwrap()
            .get_length(&h.ast),
        0_i32
    );
    assert_eq!(
        array_lit
            .get_child_at_index(&h.ast, 2)
            .unwrap()
            .get_charno(&h.ast),
        10_i32
    );
    assert!(array_lit.has_trailing_comma(&h.ast));
}

// port: ParserTest#testWhile
#[test]
fn test_while() {
    let mut h = Harness::new();
    let tmp1 = h.parse("while(1) { break; }");
    let _ = tmp1;
}

// port: ParserTest#testNestedWhile
#[test]
fn test_nested_while() {
    let mut h = Harness::new();
    let tmp1 = h.parse("while(1) { while(1) { break; } }");
    let _ = tmp1;
}

// port: ParserTest#testBreak
#[test]
fn test_break() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("break;", &[UNLABELED_BREAK]);
    let _ = tmp1;
}

// port: ParserTest#testContinue
#[test]
fn test_continue() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("continue;", &[UNEXPECTED_CONTINUE]);
    let _ = tmp1;
}

// port: ParserTest#testBreakCrossFunction
#[test]
fn test_break_cross_function() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "while(1) { var f = function() { break; } }",
        &[UNLABELED_BREAK],
    );
    let _ = tmp1;
}

// port: ParserTest#testBreakCrossFunctionInFor
#[test]
fn test_break_cross_function_in_for() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "while(1) {for(var f = function () { break; };;) {}}",
        &[UNLABELED_BREAK],
    );
    let _ = tmp1;
}

// port: ParserTest#testBreakInForOf
#[test]
fn test_break_in_for_of() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::FOR_OF]);
    let tmp1 = h.parse("for (var x of [1, 2, 3]) {\n  if (x == 2) break;\n}\n");
    let _ = tmp1;
}

// port: ParserTest#testContinueToSwitch
#[test]
fn test_continue_to_switch() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("switch(1) {case(1): continue; }", &[UNEXPECTED_CONTINUE]);
    let _ = tmp1;
}

// port: ParserTest#testContinueToSwitchWithNoCases
#[test]
fn test_continue_to_switch_with_no_cases() {
    let mut h = Harness::new();
    let tmp1 = h.parse("switch(1){}");
    let _ = tmp1;
}

// port: ParserTest#testContinueToSwitchWithTwoCases
#[test]
fn test_continue_to_switch_with_two_cases() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "switch(1){case(1):break;case(2):continue;}",
        &[UNEXPECTED_CONTINUE],
    );
    let _ = tmp1;
}

// port: ParserTest#testContinueToSwitchWithDefault
#[test]
fn test_continue_to_switch_with_default() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "switch(1){case(1):break;case(2):default:continue;}",
        &[UNEXPECTED_CONTINUE],
    );
    let _ = tmp1;
}

// port: ParserTest#testContinueToLabelSwitch
#[test]
fn test_continue_to_label_switch() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "while(1) {a: switch(1) {case(1): continue a; }}",
        &[UNEXPECTED_LABELLED_CONTINUE],
    );
    let _ = tmp1;
}

// port: ParserTest#testContinueOutsideSwitch
#[test]
fn test_continue_outside_switch() {
    let mut h = Harness::new();
    let tmp1 = h.parse("b: while(1) { a: switch(1) { case(1): continue b; } }");
    let _ = tmp1;
}

// port: ParserTest#testContinueNotCrossFunction1
#[test]
fn test_continue_not_cross_function1() {
    let mut h = Harness::new();
    let tmp1 = h.parse("a:switch(1){case(1):var f = function(){a:while(1){continue a;}}}");
    let _ = tmp1;
}

// port: ParserTest#testContinueNotCrossFunction2
#[test]
fn test_continue_not_cross_function2() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "a:switch(1){case(1):var f = function(){while(1){continue a;}}}",
        &[&format!("{}{}", UNDEFINED_LABEL, " \"a\"")],
    );
    let _ = tmp1;
}

// port: ParserTest#testContinueInForOf
#[test]
fn test_continue_in_for_of() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::FOR_OF]);
    let tmp1 = h.parse("for (var x of [1, 2, 3]) {\n  if (x == 2) continue;\n}\n");
    let _ = tmp1;
}

// port: ParserTest#testVarSourceLocations
#[test]
fn test_var_source_locations() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    let tmp1 = h.parse("var x, y = 1;");
    let mut n = tmp1;
    let mut var = n.get_first_child(&h.ast);
    assert_eq!(var.unwrap().get_token(&h.ast), Token::VAR);
    let mut x = var.unwrap().get_first_child(&h.ast);
    assert_eq!(x.unwrap().get_token(&h.ast), Token::NAME);
    assert_eq!(x.unwrap().get_charno(&h.ast), ("var ".len()) as i32);
    let mut y = x.unwrap().get_next(&h.ast);
    assert_eq!(y.unwrap().get_token(&h.ast), Token::NAME);
    assert_eq!(y.unwrap().get_charno(&h.ast), ("var x, ".len()) as i32);
    assert_eq!(y.unwrap().get_length(&h.ast), ("y = 1".len()) as i32);
}

// port: ParserTest#testSourceLocationsNonAscii
#[test]
fn test_source_locations_non_ascii() {
    let mut h = Harness::new();
    let tmp1 = h.parse("'안녕세계!'");
    let mut n = tmp1;
    let mut expr_result = n.get_first_child(&h.ast);
    let mut string = expr_result.unwrap().get_first_child(&h.ast);
    assert_eq!(string.unwrap().get_token(&h.ast), Token::STRINGLIT);
    assert_eq!(string.unwrap().get_length(&h.ast), 7_i32);
}

// port: ParserTest#testNumericSeparatorOnDecimal
#[test]
fn test_numeric_separator_on_decimal() {
    let mut h = Harness::new();
    let tmp1 = h.parse("1_000_000;");
    let mut result = tmp1;
    h.expect_features(&[Feature::NUMERIC_SEPARATOR]);
    assert_eq!(result.get_token(&h.ast), Token::SCRIPT);
    let mut expr_result = result.get_only_child(&h.ast);
    assert_eq!(expr_result.get_token(&h.ast), Token::EXPR_RESULT);
    let mut number_node = expr_result.get_only_child(&h.ast);
    assert!(number_node.is_number(&h.ast));
    assert_eq!(number_node.get_double(&h.ast), (1000000) as f64);
}

// port: ParserTest#testNumericSeparatorOnBinary
#[test]
fn test_numeric_separator_on_binary() {
    let mut h = Harness::new();
    let tmp1 = h.parse("0b1_0000;");
    let mut result = tmp1;
    h.expect_features(&[Feature::NUMERIC_SEPARATOR]);
    assert_eq!(result.get_token(&h.ast), Token::SCRIPT);
    let mut expr_result = result.get_only_child(&h.ast);
    assert_eq!(expr_result.get_token(&h.ast), Token::EXPR_RESULT);
    let mut number_node = expr_result.get_only_child(&h.ast);
    assert!(number_node.is_number(&h.ast));
    assert_eq!(number_node.get_double(&h.ast), (16) as f64);
}

// port: ParserTest#testNumericSeparatorOnOctal
#[test]
fn test_numeric_separator_on_octal() {
    let mut h = Harness::new();
    let tmp1 = h.parse("0o01_00;");
    let mut result = tmp1;
    h.expect_features(&[Feature::NUMERIC_SEPARATOR]);
    assert_eq!(result.get_token(&h.ast), Token::SCRIPT);
    let mut expr_result = result.get_only_child(&h.ast);
    assert_eq!(expr_result.get_token(&h.ast), Token::EXPR_RESULT);
    let mut number_node = expr_result.get_only_child(&h.ast);
    assert!(number_node.is_number(&h.ast));
    assert_eq!(number_node.get_double(&h.ast), (64) as f64);
}

// port: ParserTest#testNumericSeparatorOnHex
#[test]
fn test_numeric_separator_on_hex() {
    let mut h = Harness::new();
    let tmp1 = h.parse("0x01_01");
    let mut result = tmp1;
    h.expect_features(&[Feature::NUMERIC_SEPARATOR]);
    assert_eq!(result.get_token(&h.ast), Token::SCRIPT);
    let mut expr_result = result.get_only_child(&h.ast);
    assert_eq!(expr_result.get_token(&h.ast), Token::EXPR_RESULT);
    let mut number_node = expr_result.get_only_child(&h.ast);
    assert!(number_node.is_number(&h.ast));
    assert_eq!(number_node.get_double(&h.ast), (257) as f64);
}

// port: ParserTest#testNumericSeparatorOnBigInt
#[test]
fn test_numeric_separator_on_big_int() {
    let mut h = Harness::new();
    let tmp1 = h.parse("1_000n");
    let mut bigint = tmp1.get_only_child(&h.ast).get_only_child(&h.ast);
    assert_eq!(bigint.get_token(&h.ast), Token::BIGINT);
    assert!(bigint.is_big_int(&h.ast));
    assert_eq!(
        *bigint.get_big_int(&h.ast),
        closure_rhino::java_lang::parse_big_integer(&JsString::from("1000"), 10).unwrap()
    );
}

// port: ParserTest#testTrailingNumericSeparator
#[test]
fn test_trailing_numeric_separator() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("1000_", &["Trailing numeric separator"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("0b0001_", &["Trailing numeric separator"]);
    let _ = tmp2;
    let tmp3 = h.parse_error("0o100_", &["Trailing numeric separator"]);
    let _ = tmp3;
    let tmp4 = h.parse_error("0x0F_", &["Trailing numeric separator"]);
    let _ = tmp4;
    let tmp5 = h.parse_error("1000_n", &["Trailing numeric separator"]);
    let _ = tmp5;
    let tmp6 = h.parse_error("0b0001_n", &["Trailing numeric separator"]);
    let _ = tmp6;
    let tmp7 = h.parse_error("0o100_n", &["Trailing numeric separator"]);
    let _ = tmp7;
    let tmp8 = h.parse_error("0x0F_n", &["Trailing numeric separator"]);
    let _ = tmp8;
}

// port: ParserTest#testNumericSeparatorWithDecimalPoint
#[test]
#[allow(clippy::approx_constant)] // Java asserts the parsed value of the literal 3.1_41, which is not PI.
fn test_numeric_separator_with_decimal_point() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("3_.141", &["Trailing numeric separator"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("3._141", &["Trailing numeric separator"]);
    let _ = tmp2;
    let tmp3 = h.parse_error("3._", &["Trailing numeric separator"]);
    let _ = tmp3;
    let tmp4 = h.parse_error("0._141", &["Trailing numeric separator"]);
    let _ = tmp4;
    let tmp5 = h.parse_error("0._", &["Trailing numeric separator"]);
    let _ = tmp5;
    let tmp6 = h.parse_error("3._e1", &["Trailing numeric separator"]);
    let _ = tmp6;
    let tmp7 = h.parse_error("3.1_e1", &["Trailing numeric separator"]);
    let _ = tmp7;
    let tmp8 = h.parse_error("3.141_", &["Trailing numeric separator"]);
    let _ = tmp8;
    let tmp9 = h.parse_error(".141_", &["Trailing numeric separator"]);
    let _ = tmp9;
    let tmp10 = h.parse_error("_.1", &[SEMICOLON_EXPECTED]);
    let _ = tmp10;
    let tmp11 = h.parse_error("._141", &["primary expression expected"]);
    let _ = tmp11;
    let tmp12 = h.parse("3.1_41;");
    let mut result = tmp12;
    h.expect_features(&[Feature::NUMERIC_SEPARATOR]);
    assert_eq!(result.get_token(&h.ast), Token::SCRIPT);
    let mut expr_result = result.get_only_child(&h.ast);
    assert_eq!(expr_result.get_token(&h.ast), Token::EXPR_RESULT);
    let mut number_node = expr_result.get_only_child(&h.ast);
    assert!(number_node.is_number(&h.ast));
    assert_eq!(number_node.get_double(&h.ast), 3.141_f64);
}

// port: ParserTest#testNumericSeparatorWarning
#[test]
fn test_numeric_separator_warning() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT_2020;
    let tmp1 = h.parse_warning(
        "1_000",
        &[&requires_language_mode_message(Feature::NUMERIC_SEPARATOR)],
    );
    let _ = tmp1;
}

// port: ParserTest#testReturn
#[test]
fn test_return() {
    let mut h = Harness::new();
    let tmp1 = h.parse("function foo() { return 1; }");
    let _ = tmp1;
    let tmp2 = h.parse_error("return;", &[UNEXPECTED_RETURN]);
    let _ = tmp2;
    let tmp3 = h.parse_error("return 1;", &[UNEXPECTED_RETURN]);
    let _ = tmp3;
}

// port: ParserTest#testThrow
#[test]
fn test_throw() {
    let mut h = Harness::new();
    let tmp1 = h.parse("throw Error();");
    let _ = tmp1;
    let tmp2 = h.parse("throw new Error();");
    let _ = tmp2;
    let tmp3 = h.parse("throw '';");
    let _ = tmp3;
    let tmp4 = h.parse_error("throw;", &["semicolon/newline not allowed after 'throw'"]);
    let _ = tmp4;
    let tmp5 = h.parse_error(
        "throw\nError();",
        &["semicolon/newline not allowed after 'throw'"],
    );
    let _ = tmp5;
}

// port: ParserTest#testLabel1
#[test]
fn test_label1() {
    let mut h = Harness::new();
    let tmp1 = h.parse("foo:bar");
    let _ = tmp1;
}

// port: ParserTest#testLabel2
#[test]
fn test_label2() {
    let mut h = Harness::new();
    let tmp1 = h.parse("{foo:bar}");
    let _ = tmp1;
}

// port: ParserTest#testLabel3
#[test]
fn test_label3() {
    let mut h = Harness::new();
    let tmp1 = h.parse("foo:bar:baz");
    let _ = tmp1;
}

// port: ParserTest#testDuplicateLabelWithoutBraces
#[test]
fn test_duplicate_label_without_braces() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("foo:foo:bar", &["Duplicate label \"foo\""]);
    let _ = tmp1;
}

// port: ParserTest#testDuplicateLabelWithBraces
#[test]
fn test_duplicate_label_with_braces() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("foo:{bar;foo:baz}", &["Duplicate label \"foo\""]);
    let _ = tmp1;
}

// port: ParserTest#testDuplicateLabelWithFor
#[test]
fn test_duplicate_label_with_for() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("foo:for(;;){foo:bar}", &["Duplicate label \"foo\""]);
    let _ = tmp1;
}

// port: ParserTest#testNonDuplicateLabelSiblings
#[test]
fn test_non_duplicate_label_siblings() {
    let mut h = Harness::new();
    let tmp1 = h.parse("foo:1;foo:2");
    let _ = tmp1;
}

// port: ParserTest#testNonDuplicateLabelCrossFunction
#[test]
fn test_non_duplicate_label_cross_function() {
    let mut h = Harness::new();
    let tmp1 = h.parse("foo:(function(){foo:2})");
    let _ = tmp1;
}

// port: ParserTest#testLabeledFunctionDeclaration
#[test]
fn test_labeled_function_declaration() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "foo:function f() {}",
        &["Lexical declarations are only allowed at top level or inside a block."],
    );
    let _ = tmp1;
}

// port: ParserTest#testLabeledClassDeclaration
#[test]
fn test_labeled_class_declaration() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "foo:class Foo {}",
        &["Lexical declarations are only allowed at top level or inside a block."],
    );
    let _ = tmp1;
}

// port: ParserTest#testLabeledLetDeclaration
#[test]
fn test_labeled_let_declaration() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "foo: let x = 0;",
        &["Lexical declarations are only allowed at top level or inside a block."],
    );
    let _ = tmp1;
}

// port: ParserTest#testLabeledConstDeclaration
#[test]
fn test_labeled_const_declaration() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "foo: const x = 0;",
        &["Lexical declarations are only allowed at top level or inside a block."],
    );
    let _ = tmp1;
}

// port: ParserTest#testMethodNamedStatic
#[test]
fn test_method_named_static() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C { static(a, b) {} }");
    let _ = tmp1;
}

// port: ParserTest#testLinenoCharnoAssign1
#[test]
fn test_lineno_charno_assign1() {
    let mut h = Harness::new();
    let tmp1 = h.parse("a = b");
    let mut assign = tmp1.get_first_first_child(&h.ast);
    assert_eq!(assign.unwrap().get_token(&h.ast), Token::ASSIGN);
    assert_eq!(assign.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(assign.unwrap().get_charno(&h.ast), 0_i32);
}

// port: ParserTest#testLinenoCharnoAssign2
#[test]
fn test_lineno_charno_assign2() {
    let mut h = Harness::new();
    let tmp1 = h.parse("\n a.g.h.k    =  45");
    let mut assign = tmp1.get_first_first_child(&h.ast);
    assert_eq!(assign.unwrap().get_token(&h.ast), Token::ASSIGN);
    assert_eq!(assign.unwrap().get_lineno(&h.ast), 2_i32);
    assert_eq!(assign.unwrap().get_charno(&h.ast), 1_i32);
}

// port: ParserTest#testLinenoCharnoCall
#[test]
fn test_lineno_charno_call() {
    let mut h = Harness::new();
    let tmp1 = h.parse("\n foo(123);");
    let mut call = tmp1.get_first_first_child(&h.ast);
    assert_eq!(call.unwrap().get_token(&h.ast), Token::CALL);
    assert_eq!(call.unwrap().get_lineno(&h.ast), 2_i32);
    assert_eq!(call.unwrap().get_charno(&h.ast), 1_i32);
}

// port: ParserTest#testLinenoCharnoGetProp1
#[test]
fn test_lineno_charno_get_prop1() {
    let mut h = Harness::new();
    let tmp1 = h.parse("\n foo.bar");
    let mut getprop = tmp1.get_first_first_child(&h.ast);
    assert_eq!(getprop.unwrap().get_token(&h.ast), Token::GETPROP);
    assert_eq!(getprop.unwrap().get_lineno(&h.ast), 2_i32);
    assert_eq!(getprop.unwrap().get_charno(&h.ast), 5_i32);
    assert_eq!(getprop.unwrap().get_string(&h.ast), JsString::from("bar"));
}

// port: ParserTest#testLinenoCharnoGetProp2
#[test]
fn test_lineno_charno_get_prop2() {
    let mut h = Harness::new();
    let tmp1 = h.parse("\n foo.\nbar");
    let mut getprop = tmp1.get_first_first_child(&h.ast);
    assert_eq!(getprop.unwrap().get_token(&h.ast), Token::GETPROP);
    assert_eq!(getprop.unwrap().get_lineno(&h.ast), 3_i32);
    assert_eq!(getprop.unwrap().get_charno(&h.ast), 0_i32);
    assert_eq!(getprop.unwrap().get_string(&h.ast), JsString::from("bar"));
}

// port: ParserTest#testLinenoCharnoGetelem1
#[test]
fn test_lineno_charno_getelem1() {
    let mut h = Harness::new();
    let tmp1 = h.parse("\n foo[123]");
    let mut call = tmp1.get_first_first_child(&h.ast);
    assert_eq!(call.unwrap().get_token(&h.ast), Token::GETELEM);
    assert_eq!(call.unwrap().get_lineno(&h.ast), 2_i32);
    assert_eq!(call.unwrap().get_charno(&h.ast), 1_i32);
}

// port: ParserTest#testLinenoCharnoGetelem2
#[test]
fn test_lineno_charno_getelem2() {
    let mut h = Harness::new();
    let tmp1 = h.parse("\n   \n foo()[123]");
    let mut call = tmp1.get_first_first_child(&h.ast);
    assert_eq!(call.unwrap().get_token(&h.ast), Token::GETELEM);
    assert_eq!(call.unwrap().get_lineno(&h.ast), 3_i32);
    assert_eq!(call.unwrap().get_charno(&h.ast), 1_i32);
}

// port: ParserTest#testLinenoCharnoGetelem3
#[test]
fn test_lineno_charno_getelem3() {
    let mut h = Harness::new();
    let tmp1 = h.parse("\n   \n (8 + kl)[123]");
    let mut call = tmp1.get_first_first_child(&h.ast);
    assert_eq!(call.unwrap().get_token(&h.ast), Token::GETELEM);
    assert_eq!(call.unwrap().get_lineno(&h.ast), 3_i32);
    assert_eq!(call.unwrap().get_charno(&h.ast), 1_i32);
}

// port: ParserTest#testLinenoCharnoForComparison
#[test]
fn test_lineno_charno_for_comparison() {
    let mut h = Harness::new();
    let tmp1 = h.parse("for (; i < j;){}");
    let mut lt = tmp1
        .get_first_child(&h.ast)
        .unwrap()
        .get_second_child(&h.ast);
    assert_eq!(lt.unwrap().get_token(&h.ast), Token::LT);
    assert_eq!(lt.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(lt.unwrap().get_charno(&h.ast), 7_i32);
}

// port: ParserTest#testLinenoCharnoHook
#[test]
fn test_lineno_charno_hook() {
    let mut h = Harness::new();
    let tmp1 = h.parse("\n a ? 9 : 0");
    let mut n = tmp1.get_first_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::HOOK);
    assert_eq!(n.unwrap().get_lineno(&h.ast), 2_i32);
    assert_eq!(n.unwrap().get_charno(&h.ast), 1_i32);
}

// port: ParserTest#testLinenoCharnoArrayLiteral
#[test]
fn test_lineno_charno_array_literal() {
    let mut h = Harness::new();
    let tmp1 = h.parse("\n  [8, 9]");
    let mut n = tmp1.get_first_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::ARRAYLIT);
    assert_eq!(n.unwrap().get_lineno(&h.ast), 2_i32);
    assert_eq!(n.unwrap().get_charno(&h.ast), 2_i32);
    n = n.unwrap().get_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::NUMBER);
    assert_eq!(n.unwrap().get_lineno(&h.ast), 2_i32);
    assert_eq!(n.unwrap().get_charno(&h.ast), 3_i32);
    n = n.unwrap().get_next(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::NUMBER);
    assert_eq!(n.unwrap().get_lineno(&h.ast), 2_i32);
    assert_eq!(n.unwrap().get_charno(&h.ast), 6_i32);
}

// port: ParserTest#testLinenoCharnoObjectLiteral
#[test]
fn test_lineno_charno_object_literal() {
    let mut h = Harness::new();
    let tmp1 = h.parse("\n\n var a = {a:0\n,b :1};");
    let mut n = tmp1
        .get_first_first_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::OBJECTLIT);
    assert_eq!(n.unwrap().get_lineno(&h.ast), 3_i32);
    assert_eq!(n.unwrap().get_charno(&h.ast), 9_i32);
    let mut key = n.unwrap().get_first_child(&h.ast);
    assert_eq!(key.unwrap().get_token(&h.ast), Token::STRING_KEY);
    assert_eq!(key.unwrap().get_lineno(&h.ast), 3_i32);
    assert_eq!(key.unwrap().get_charno(&h.ast), 10_i32);
    let mut value = key.unwrap().get_first_child(&h.ast);
    assert_eq!(value.unwrap().get_token(&h.ast), Token::NUMBER);
    assert_eq!(value.unwrap().get_lineno(&h.ast), 3_i32);
    assert_eq!(value.unwrap().get_charno(&h.ast), 12_i32);
    key = key.unwrap().get_next(&h.ast);
    assert_eq!(key.unwrap().get_token(&h.ast), Token::STRING_KEY);
    assert_eq!(key.unwrap().get_lineno(&h.ast), 4_i32);
    assert_eq!(key.unwrap().get_charno(&h.ast), 1_i32);
    value = key.unwrap().get_first_child(&h.ast);
    assert_eq!(value.unwrap().get_token(&h.ast), Token::NUMBER);
    assert_eq!(value.unwrap().get_lineno(&h.ast), 4_i32);
    assert_eq!(value.unwrap().get_charno(&h.ast), 4_i32);
}

// port: ParserTest#testLinenoCharnoObjectLiteralMemberFunction
#[test]
fn test_lineno_charno_object_literal_member_function() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var a = {\n fn() {} };");
    let mut n = tmp1
        .get_first_first_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::OBJECTLIT);
    assert_eq!(n.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(n.unwrap().get_charno(&h.ast), 8_i32);
    let mut key = n.unwrap().get_first_child(&h.ast);
    assert_eq!(key.unwrap().get_token(&h.ast), Token::MEMBER_FUNCTION_DEF);
    assert_eq!(key.unwrap().get_lineno(&h.ast), 2_i32);
    assert_eq!(key.unwrap().get_charno(&h.ast), 1_i32);
    assert_eq!(key.unwrap().get_length(&h.ast), 2_i32);
    let mut value = key.unwrap().get_first_child(&h.ast);
    assert_eq!(value.unwrap().get_token(&h.ast), Token::FUNCTION);
    assert_eq!(value.unwrap().get_lineno(&h.ast), 2_i32);
    assert_eq!(value.unwrap().get_charno(&h.ast), 1_i32);
    assert_eq!(value.unwrap().get_length(&h.ast), 7_i32);
}

// port: ParserTest#testLinenoCharnoEs6Class
#[test]
fn test_lineno_charno_es6_class() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C {\n  fn1() {}\n  static fn2() {}\n };");
    let mut n = tmp1.get_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::CLASS);
    assert_eq!(n.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(n.unwrap().get_charno(&h.ast), 0_i32);
    let mut members = get_class_members(&h.ast, n.unwrap());
    assert_eq!(members.get_token(&h.ast), Token::CLASS_MEMBERS);
    let mut member_fn = members.get_first_child(&h.ast);
    assert_eq!(
        member_fn.unwrap().get_token(&h.ast),
        Token::MEMBER_FUNCTION_DEF
    );
    assert_eq!(member_fn.unwrap().get_lineno(&h.ast), 2_i32);
    assert_eq!(member_fn.unwrap().get_charno(&h.ast), 2_i32);
    assert_eq!(member_fn.unwrap().get_length(&h.ast), 3_i32);
    let mut fn_node = member_fn.unwrap().get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    assert_eq!(fn_node.unwrap().get_lineno(&h.ast), 2_i32);
    assert_eq!(fn_node.unwrap().get_charno(&h.ast), 2_i32);
    assert_eq!(fn_node.unwrap().get_length(&h.ast), 8_i32);
    member_fn = member_fn.unwrap().get_next(&h.ast);
    assert_eq!(
        member_fn.unwrap().get_token(&h.ast),
        Token::MEMBER_FUNCTION_DEF
    );
    assert_eq!(member_fn.unwrap().get_lineno(&h.ast), 3_i32);
    assert_eq!(member_fn.unwrap().get_charno(&h.ast), 9_i32);
    assert_eq!(member_fn.unwrap().get_length(&h.ast), 3_i32);
    assert!(member_fn.unwrap().is_static_member(&h.ast));
    fn_node = member_fn.unwrap().get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    assert_eq!(fn_node.unwrap().get_lineno(&h.ast), 3_i32);
    assert_eq!(fn_node.unwrap().get_charno(&h.ast), 2_i32);
    assert_eq!(fn_node.unwrap().get_length(&h.ast), 15_i32);
}

// port: ParserTest#testLinenoCharnoAdd
#[test]
fn test_lineno_charno_add() {
    let mut h = Harness::new();
    h.test_lineno_charno_binop("+");
}

// port: ParserTest#testLinenoCharnoSub
#[test]
fn test_lineno_charno_sub() {
    let mut h = Harness::new();
    h.test_lineno_charno_binop("-");
}

// port: ParserTest#testLinenoCharnoMul
#[test]
fn test_lineno_charno_mul() {
    let mut h = Harness::new();
    h.test_lineno_charno_binop("*");
}

// port: ParserTest#testLinenoCharnoDiv
#[test]
fn test_lineno_charno_div() {
    let mut h = Harness::new();
    h.test_lineno_charno_binop("/");
}

// port: ParserTest#testLinenoCharnoMod
#[test]
fn test_lineno_charno_mod() {
    let mut h = Harness::new();
    h.test_lineno_charno_binop("%");
}

// port: ParserTest#testLinenoCharnoShift
#[test]
fn test_lineno_charno_shift() {
    let mut h = Harness::new();
    h.test_lineno_charno_binop("<<");
}

// port: ParserTest#testLinenoCharnoBinaryAnd
#[test]
fn test_lineno_charno_binary_and() {
    let mut h = Harness::new();
    h.test_lineno_charno_binop("&");
}

// port: ParserTest#testLinenoCharnoAnd
#[test]
fn test_lineno_charno_and() {
    let mut h = Harness::new();
    h.test_lineno_charno_binop("&&");
}

// port: ParserTest#testLinenoCharnoBinaryOr
#[test]
fn test_lineno_charno_binary_or() {
    let mut h = Harness::new();
    h.test_lineno_charno_binop("|");
}

// port: ParserTest#testLinenoCharnoOr
#[test]
fn test_lineno_charno_or() {
    let mut h = Harness::new();
    h.test_lineno_charno_binop("||");
}

// port: ParserTest#testLinenoCharnoLt
#[test]
fn test_lineno_charno_lt() {
    let mut h = Harness::new();
    h.test_lineno_charno_binop("<");
}

// port: ParserTest#testLinenoCharnoLe
#[test]
fn test_lineno_charno_le() {
    let mut h = Harness::new();
    h.test_lineno_charno_binop("<=");
}

// port: ParserTest#testLinenoCharnoGt
#[test]
fn test_lineno_charno_gt() {
    let mut h = Harness::new();
    h.test_lineno_charno_binop(">");
}

// port: ParserTest#testLinenoCharnoGe
#[test]
fn test_lineno_charno_ge() {
    let mut h = Harness::new();
    h.test_lineno_charno_binop(">=");
}

// port: ParserTest#testLinenoCharnoNegativeNumber
#[test]
fn test_lineno_charno_negative_number() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var x = -1000");
    let mut n = tmp1
        .get_first_child(&h.ast)
        .unwrap()
        .get_first_first_child(&h.ast);
    assert_eq!(n.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(n.unwrap().get_charno(&h.ast), 8_i32);
    assert_eq!(n.unwrap().get_length(&h.ast), 5_i32);
}

// port: ParserTest#testJSDocAttachment7
#[test]
fn test_js_doc_attachment7() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/** */var a;");
    let mut var_node = tmp1.get_first_child(&h.ast);
    assert_eq!(var_node.unwrap().get_token(&h.ast), Token::VAR);
    let mut name_node = var_node.unwrap().get_first_child(&h.ast);
    assert_eq!(name_node.unwrap().get_token(&h.ast), Token::NAME);
    assert!(name_node.unwrap().get_jsdoc_info(&h.ast).is_none());
}

// port: ParserTest#testJSDocAttachment8
#[test]
fn test_js_doc_attachment8() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/** x */var a;");
    let mut var_node = tmp1.get_first_child(&h.ast);
    assert_eq!(var_node.unwrap().get_token(&h.ast), Token::VAR);
    let mut name_node = var_node.unwrap().get_first_child(&h.ast);
    assert_eq!(name_node.unwrap().get_token(&h.ast), Token::NAME);
    assert!(name_node.unwrap().get_jsdoc_info(&h.ast).is_none());
}

// port: ParserTest#testJSDocAttachment9
#[test]
fn test_js_doc_attachment9() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/** \n x */var a;");
    let mut var_node = tmp1.get_first_child(&h.ast);
    assert_eq!(var_node.unwrap().get_token(&h.ast), Token::VAR);
    let mut name_node = var_node.unwrap().get_first_child(&h.ast);
    assert_eq!(name_node.unwrap().get_token(&h.ast), Token::NAME);
    assert!(name_node.unwrap().get_jsdoc_info(&h.ast).is_none());
}

// port: ParserTest#testJSDocAttachment10
#[test]
fn test_js_doc_attachment10() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/** x\n */var a;");
    let mut var_node = tmp1.get_first_child(&h.ast);
    assert_eq!(var_node.unwrap().get_token(&h.ast), Token::VAR);
    let mut name_node = var_node.unwrap().get_first_child(&h.ast);
    assert_eq!(name_node.unwrap().get_token(&h.ast), Token::NAME);
    assert!(name_node.unwrap().get_jsdoc_info(&h.ast).is_none());
}

// port: ParserTest#testJSDocAttachment12
#[test]
fn test_js_doc_attachment12() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var a = {/** @type {Object} */ b: c};");
    let mut var_node = tmp1.get_first_child(&h.ast);
    let mut object_lit_node = var_node.unwrap().get_first_first_child(&h.ast);
    assert_eq!(object_lit_node.unwrap().get_token(&h.ast), Token::OBJECTLIT);
    assert!(
        object_lit_node
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: ParserTest#testJSDocAttachment13
#[test]
fn test_js_doc_attachment13() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/** foo */ var a;");
    let mut var_node = tmp1.get_first_child(&h.ast);
    assert!(var_node.unwrap().get_jsdoc_info(&h.ast).is_some());
}

// port: ParserTest#testJSDocAttachment14
#[test]
fn test_js_doc_attachment14() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/** */ var a;");
    let mut var_node = tmp1.get_first_child(&h.ast);
    assert!(var_node.unwrap().get_jsdoc_info(&h.ast).is_none());
}

// port: ParserTest#testJSDocAttachment15
#[test]
fn test_js_doc_attachment15() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/** \n * \n */ var a;");
    let mut var_node = tmp1.get_first_child(&h.ast);
    assert!(var_node.unwrap().get_jsdoc_info(&h.ast).is_none());
}

// port: ParserTest#testJSDocAttachment16
#[test]
fn test_js_doc_attachment16() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/** @private */ x(); function f() {};");
    let mut expr_call = tmp1.get_first_child(&h.ast);
    assert_eq!(expr_call.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    assert!(
        expr_call
            .unwrap()
            .get_next(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_none()
    );
    assert!(
        expr_call
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: ParserTest#testJSDocAttachmentForCastFnCall
#[test]
fn test_js_doc_attachment_for_cast_fn_call() {
    let mut h = Harness::new();
    let tmp1 =
        h.parse("function f() {\n  return /** @type {string} */ (g(1 /** @desc x */));\n};\n");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut cast = fn_node
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_first_child(&h.ast);
    assert_eq!(cast.unwrap().get_token(&h.ast), Token::CAST);
}

// port: ParserTest#testJSDocAttachmentForCastName
#[test]
fn test_js_doc_attachment_for_cast_name() {
    let mut h = Harness::new();
    let tmp1 = h.parse("function f() {\n  var x = /** @type {string} */ (y);\n};\n");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut cast = fn_node
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_first_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(cast.unwrap().get_token(&h.ast), Token::CAST);
}

// port: ParserTest#testJSDocAttachmentForCastLhs
#[test]
fn test_js_doc_attachment_for_cast_lhs() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/** some jsdoc */ (/** @type {?} */ (a)).b = 0;");
    let mut expr = tmp1.get_only_child(&h.ast);
    let mut lhs = expr.get_first_first_child(&h.ast);
    let mut cast = lhs.unwrap().get_first_child(&h.ast);
    assert_eq!(cast.unwrap().get_token(&h.ast), Token::CAST);
    assert!(cast.unwrap().get_jsdoc_info(&h.ast).is_some());
    assert!(
        !cast
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .as_ref()
            .unwrap()
            .has_type()
    );
}

// port: ParserTest#testJSDocAttachment19
#[test]
fn test_js_doc_attachment19() {
    let mut h = Harness::new();
    let tmp1 = h.parse("function f() {\n  /** @type {string} */\n  return;\n};\n");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut ret = fn_node
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(ret.unwrap().get_token(&h.ast), Token::RETURN);
    assert!(ret.unwrap().get_jsdoc_info(&h.ast).is_some());
}

// port: ParserTest#testJSDocAttachment20
#[test]
fn test_js_doc_attachment20() {
    let mut h = Harness::new();
    let tmp1 = h.parse("function f() {\n  /** @type {string} */\n  if (true) return;\n};\n");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut ret = fn_node
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(ret.unwrap().get_token(&h.ast), Token::IF);
    assert!(ret.unwrap().get_jsdoc_info(&h.ast).is_some());
}

// port: ParserTest#testJSDocAttachment21
#[test]
fn test_js_doc_attachment21() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::CONST_DECLARATIONS]);
    let tmp1 = h.parse("/** @param {string} x */ const f = function() {};");
    let _ = tmp1;
    h.expect_features(&[Feature::LET_DECLARATIONS]);
    let tmp2 = h.parse("/** @param {string} x */ let f = function() {};");
    let _ = tmp2;
}

// port: ParserTest#testJSDocAttachment22
#[test]
fn test_js_doc_attachment22() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::MODULES]);
    let tmp1 = h.parse("/** @param {string} x */ export function f(x) {};");
    let mut n = tmp1;
    let mut export = n.get_first_first_child(&h.ast);
    assert_eq!(export.unwrap().get_token(&h.ast), Token::EXPORT);
    assert!(export.unwrap().get_jsdoc_info(&h.ast).is_none());
    assert!(
        export
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
    assert!(
        export
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .as_ref()
            .unwrap()
            .has_parameter("x")
    );
}

// port: ParserTest#testNodeInsideParens_simpleString
#[test]
fn test_node_inside_parens_simple_string() {
    let mut h = Harness::new();
    let tmp1 = h.parse("('a')");
    let mut expr_res = tmp1.get_first_child(&h.ast);
    let mut str = expr_res.unwrap().get_first_child(&h.ast);
    assert_eq!(expr_res.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    assert_eq!(str.unwrap().get_token(&h.ast), Token::STRINGLIT);
    assert!(str.unwrap().get_is_parenthesized(&h.ast));
}

// port: ParserTest#testNodesInsideParens
#[test]
fn test_nodes_inside_parens() {
    let mut h = Harness::new();
    let tmp1 = h.parse("((x&&y) && z)");
    let mut and_expr_res = tmp1.get_first_child(&h.ast);
    assert_eq!(and_expr_res.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    let mut and_node = and_expr_res.unwrap().get_first_child(&h.ast);
    assert_eq!(and_node.unwrap().get_token(&h.ast), Token::AND);
    assert!(and_node.unwrap().get_is_parenthesized(&h.ast));
    let mut x_andy = and_node.unwrap().get_first_child(&h.ast);
    assert_eq!(x_andy.unwrap().get_token(&h.ast), Token::AND);
    assert!(x_andy.unwrap().get_is_parenthesized(&h.ast));
    let mut x = x_andy.unwrap().get_first_child(&h.ast);
    let mut y = x_andy.unwrap().get_second_child(&h.ast);
    assert_eq!(x.unwrap().get_token(&h.ast), Token::NAME);
    assert_eq!(y.unwrap().get_token(&h.ast), Token::NAME);
    assert!(!x.unwrap().get_is_parenthesized(&h.ast));
    assert!(!y.unwrap().get_is_parenthesized(&h.ast));
    let mut z = and_node.unwrap().get_second_child(&h.ast);
    assert_eq!(z.unwrap().get_token(&h.ast), Token::NAME);
    assert!(!z.unwrap().get_is_parenthesized(&h.ast));
}

// port: ParserTest#testInlineNonJSDocCommentAttachmentToVar
#[test]
fn test_inline_non_js_doc_comment_attachment_to_var() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("let /* blah */ x = 'a';");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut non_js_doc_comment = let_node
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap()
        .get_non_jsdoc_comment(&h.ast);
    assert!(non_js_doc_comment.is_some());
    assert!(non_js_doc_comment.as_ref().unwrap().is_inline());
    assert!(
        !non_js_doc_comment
            .as_ref()
            .unwrap()
            .is_ending_as_line_comment()
    );
    assert!(
        non_js_doc_comment
            .as_ref()
            .unwrap()
            .get_comment_string()
            .index_of("/* blah */")
            >= 0
    );
}

// port: ParserTest#testNoInlineNonJSDocCommentAttachmentWithoutParsingMode
#[test]
fn test_no_inline_non_js_doc_comment_attachment_without_parsing_mode() {
    let mut h = Harness::new();
    let tmp1 = h.parse("let /* blah */ x = 'a';");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut non_js_doc_comment = let_node
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap()
        .get_non_jsdoc_comment(&h.ast);
    assert!(non_js_doc_comment.is_none());
}

// port: ParserTest#testInlineNonJSDocCommentAttachmentToObjPatNormalProp
#[test]
fn test_inline_non_js_doc_comment_attachment_to_obj_pat_normal_prop() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("let { normalProp: /* blah */ normalPropTarget } = {};");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut destructuring_lhs = let_node.unwrap().get_first_child(&h.ast);
    let mut object_pattern = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut normal_prop = object_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(normal_prop.unwrap().get_token(&h.ast), Token::STRING_KEY);
    let mut normal_prop_target = normal_prop.unwrap().get_only_child(&h.ast);
    assert!(
        normal_prop_target
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("/* blah */")
            >= 0
    );
}

// port: ParserTest#testNoInlineNonJSDocCommentAttachmentToObjWithoutParsingMode
#[test]
fn test_no_inline_non_js_doc_comment_attachment_to_obj_without_parsing_mode() {
    let mut h = Harness::new();
    let tmp1 = h.parse("let { normalProp: /* blah */ normalPropTarget } = {};");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut destructuring_lhs = let_node.unwrap().get_first_child(&h.ast);
    let mut object_pattern = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut normal_prop = object_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(normal_prop.unwrap().get_token(&h.ast), Token::STRING_KEY);
    let mut normal_prop_target = normal_prop.unwrap().get_only_child(&h.ast);
    assert!(normal_prop_target.get_non_jsdoc_comment(&h.ast).is_none());
}

// port: ParserTest#testInlineJSDocAttachmentToObjPatNormalPropKey
#[test]
fn test_inline_js_doc_attachment_to_obj_pat_normal_prop_key() {
    let mut h = Harness::new();
    let tmp1 = h.parse("let { /** string */ normalProp: normalProp } = {};");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut destructuring_lhs = let_node.unwrap().get_first_child(&h.ast);
    let mut object_pattern = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut normal_prop = object_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(normal_prop.unwrap().get_token(&h.ast), Token::STRING_KEY);
    assert_node_has_no_jsdoc_info(&h.ast, normal_prop.unwrap());
}

// port: ParserTest#testInlineNonJSDocCommentAttachmentToObjPatNormalPropKey
#[test]
fn test_inline_non_js_doc_comment_attachment_to_obj_pat_normal_prop_key() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("let { /* blah */ normalProp: normalProp } = {};");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut destructuring_lhs = let_node.unwrap().get_first_child(&h.ast);
    let mut object_pattern = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut normal_prop_key = object_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(
        normal_prop_key.unwrap().get_token(&h.ast),
        Token::STRING_KEY
    );
    assert!(
        normal_prop_key
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("/* blah */")
            >= 0
    );
}

// port: ParserTest#testInlineNonJSDocCommentAttachment_numberAsKey
#[test]
fn test_inline_non_js_doc_comment_attachment_number_as_key() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("var { /*comment */ 3: x} = foo();");
    let mut script = tmp1;
    let mut var = script.get_first_child(&h.ast);
    let mut destructuring_lhs = var.unwrap().get_first_child(&h.ast);
    let mut obj_lit = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut three = obj_lit.unwrap().get_first_child(&h.ast);
    let mut x = three.unwrap().get_first_child(&h.ast);
    assert!(three.unwrap().get_non_jsdoc_comment(&h.ast).is_some());
    assert_eq!(
        three
            .unwrap()
            .get_non_jsdoc_comment(&h.ast)
            .as_ref()
            .unwrap()
            .get_comment_string(),
        JsString::from("/*comment */")
    );
    assert!(
        three
            .unwrap()
            .get_non_jsdoc_comment(&h.ast)
            .as_ref()
            .unwrap()
            .is_inline()
    );
    assert!(x.unwrap().get_non_jsdoc_comment(&h.ast).is_none());
}

// port: ParserTest#testInlineNonJSDocCommentAttachment_quotedKey
#[test]
fn test_inline_non_js_doc_comment_attachment_quoted_key() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("var { /*comment */ 'a': x} = foo();");
    let mut script = tmp1;
    let mut var = script.get_first_child(&h.ast);
    let mut destructuring_lhs = var.unwrap().get_first_child(&h.ast);
    let mut obj_lit = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut a = obj_lit.unwrap().get_first_child(&h.ast);
    let mut x = a.unwrap().get_first_child(&h.ast);
    assert!(a.unwrap().get_non_jsdoc_comment(&h.ast).is_some());
    assert_eq!(
        a.unwrap()
            .get_non_jsdoc_comment(&h.ast)
            .as_ref()
            .unwrap()
            .get_comment_string(),
        JsString::from("/*comment */")
    );
    assert!(
        a.unwrap()
            .get_non_jsdoc_comment(&h.ast)
            .as_ref()
            .unwrap()
            .is_inline()
    );
    assert!(x.unwrap().get_non_jsdoc_comment(&h.ast).is_none());
}

// port: ParserTest#testInlineNonJSDocAttachmentToObjPatShorthandProp
#[test]
fn test_inline_non_js_doc_attachment_to_obj_pat_shorthand_prop() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("let { /* blah */ shorthandProp } = {};");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut destructuring_lhs = let_node.unwrap().get_first_child(&h.ast);
    let mut object_pattern = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut shorthand_prop = object_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(shorthand_prop.unwrap().get_token(&h.ast), Token::STRING_KEY);
    assert!(
        shorthand_prop
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("/* blah */")
            >= 0
    );
}

// port: ParserTest#testInlineNonJSDocCommentAttachmentToObjPatNormalPropWithDefault
#[test]
fn test_inline_non_js_doc_comment_attachment_to_obj_pat_normal_prop_with_default() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 =
        h.parse("let { normalPropWithDefault: /* blah */ normalPropWithDefault = 'hi' } = {};");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut destructuring_lhs = let_node.unwrap().get_first_child(&h.ast);
    let mut object_pattern = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut normal_prop_with_default = object_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(
        normal_prop_with_default.unwrap().get_token(&h.ast),
        Token::STRING_KEY
    );
    let mut normal_prop_default_value = normal_prop_with_default.unwrap().get_only_child(&h.ast);
    assert_eq!(
        normal_prop_default_value.get_token(&h.ast),
        Token::DEFAULT_VALUE
    );
    let mut normal_prop_with_default_target = normal_prop_default_value.get_first_child(&h.ast);
    assert!(
        normal_prop_with_default_target
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("/* blah */")
            >= 0
    );
}

// port: ParserTest#testInlineNonJSDocCommentAttachmentToObjPatShorthandWithDefault
#[test]
fn test_inline_non_js_doc_comment_attachment_to_obj_pat_shorthand_with_default() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("let { /* blah */ shorthandPropWithDefault = 'lo' } = {};");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut destructuring_lhs = let_node.unwrap().get_first_child(&h.ast);
    let mut object_pattern = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut shorthand_prop_with_default = object_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(
        shorthand_prop_with_default.unwrap().get_token(&h.ast),
        Token::STRING_KEY
    );
    let mut shorthand_prop_default_value =
        shorthand_prop_with_default.unwrap().get_only_child(&h.ast);
    assert_eq!(
        shorthand_prop_default_value.get_token(&h.ast),
        Token::DEFAULT_VALUE
    );
    let mut shorthand_prop_with_default_target =
        shorthand_prop_default_value.get_first_child(&h.ast);
    assert!(
        shorthand_prop_with_default_target
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("/* blah */")
            >= 0
    );
}

// port: ParserTest#testObjLitKeyNonJSDocComment
#[test]
fn test_obj_lit_key_non_js_doc_comment() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("a = {\n// blah\n1n: 3};");
    let mut expr_result = tmp1.get_first_child(&h.ast);
    let mut assign_node = expr_result.unwrap().get_first_child(&h.ast);
    let mut object_lit = assign_node.unwrap().get_second_child(&h.ast);
    let mut string_key = object_lit.unwrap().get_first_child(&h.ast);
    assert!(
        string_key
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("// blah")
            >= 0
    );
}

// port: ParserTest#testLabelNonJSDocComment
#[test]
fn test_label_non_js_doc_comment() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("\n// blah\nlabel:\nfor(;;){ continue label; }");
    let mut label = tmp1.get_first_child(&h.ast);
    assert_eq!(label.unwrap().get_token(&h.ast), Token::LABEL);
    assert!(
        label
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("// blah")
            >= 0
    );
}

// port: ParserTest#testFieldNonJSDocComment
#[test]
fn test_field_non_js_doc_comment() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("class C{\n// blah\n field }");
    let mut field = tmp1
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(field.unwrap().get_token(&h.ast), Token::MEMBER_FIELD_DEF);
    assert!(
        field
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("// blah")
            >= 0
    );
}

// port: ParserTest#testPropertyNameAssignmentNonJSDocComment
#[test]
fn test_property_name_assignment_non_js_doc_comment() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("let {key: \n// blah\nvarName} = someObject");
    let mut var_name = tmp1
        .get_first_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(var_name.unwrap().get_token(&h.ast), Token::NAME);
    assert!(
        var_name
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("// blah")
            >= 0
    );
}

// port: ParserTest#testGetPropCallNonJSDocComment
#[test]
fn test_get_prop_call_non_js_doc_comment() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("foo.\n// blah\nbaz(3);");
    let mut expr_result = tmp1.get_first_child(&h.ast);
    let mut call = expr_result.unwrap().get_first_child(&h.ast);
    assert_eq!(call.unwrap().get_token(&h.ast), Token::CALL);
    let mut baz_access = call.unwrap().get_first_child(&h.ast);
    assert!(
        baz_access
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("// blah")
            >= 0
    );
}

// port: ParserTest#testGetPropOptionalCallNonJSDocComment
#[test]
fn test_get_prop_optional_call_non_js_doc_comment() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("foo?.\n// blah\nbaz(3);");
    let mut expr_result = tmp1.get_first_child(&h.ast);
    let mut call_node = expr_result.unwrap().get_first_child(&h.ast);
    assert_eq!(call_node.unwrap().get_token(&h.ast), Token::OPTCHAIN_CALL);
    let mut baz_access = call_node.unwrap().get_first_child(&h.ast);
    assert!(
        baz_access
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("// blah")
            >= 0
    );
}

// port: ParserTest#testGetPropAssignmentNonJSDocComment
#[test]
fn test_get_prop_assignment_non_js_doc_comment() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("foo.\n// blah\nbaz = 5;");
    let mut expr_result = tmp1.get_first_child(&h.ast);
    let mut assign = expr_result.unwrap().get_first_child(&h.ast);
    assert_eq!(assign.unwrap().get_token(&h.ast), Token::ASSIGN);
    let mut baz_access = assign.unwrap().get_first_child(&h.ast);
    assert!(
        baz_access
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("// blah")
            >= 0
    );
}

// port: ParserTest#testNonJSDocCommentsOnAdjacentNodes
#[test]
fn test_non_js_doc_comments_on_adjacent_nodes() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("// comment before GETPROP\na(\n// comment on GETPROP\n)\n.b();\n// comment after GETPROP\nc();\n");
    let mut root = tmp1;
    let mut expr_result_a = root.get_first_child(&h.ast);
    assert_eq!(expr_result_a.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    assert_eq!(
        expr_result_a.unwrap().get_non_jsdoc_comment_string(&h.ast),
        JsString::from("// comment before GETPROP")
    );
    let mut node_b = expr_result_a.unwrap().get_first_first_child(&h.ast);
    assert_eq!(node_b.unwrap().get_token(&h.ast), Token::GETPROP);
    assert_eq!(
        node_b.unwrap().get_non_jsdoc_comment_string(&h.ast),
        JsString::from("// comment on GETPROP")
    );
    let mut expr_result_c = root.get_last_child(&h.ast);
    assert_eq!(expr_result_c.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    assert_eq!(
        expr_result_c.unwrap().get_non_jsdoc_comment_string(&h.ast),
        JsString::from("// comment after GETPROP")
    );
}

// port: ParserTest#testInlineJSDocAttachmentToObjPatComputedPropKey
#[test]
fn test_inline_js_doc_attachment_to_obj_pat_computed_prop_key() {
    let mut h = Harness::new();
    let tmp1 = h.parse("let { /** string */ ['computedProp']: computedProp } = {};");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut destructuring_lhs = let_node.unwrap().get_first_child(&h.ast);
    let mut object_pattern = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut computed_prop = object_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(
        computed_prop.unwrap().get_token(&h.ast),
        Token::COMPUTED_PROP
    );
    assert_node_has_no_jsdoc_info(&h.ast, computed_prop.unwrap());
}

// port: ParserTest#testInlineJSDocAttachmentToObjLitNormalProp
#[test]
fn test_inline_js_doc_attachment_to_obj_lit_normal_prop() {
    let mut h = Harness::new();
    let tmp1 = h.parse("let x = { normalProp: /** string */ normalPropTarget };");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut x_node = let_node.unwrap().get_first_child(&h.ast);
    let mut object_lit = x_node.unwrap().get_first_child(&h.ast);
    let mut normal_prop = object_lit.unwrap().get_first_child(&h.ast);
    assert_eq!(normal_prop.unwrap().get_token(&h.ast), Token::STRING_KEY);
    let mut normal_prop_target = normal_prop.unwrap().get_only_child(&h.ast);
    assert_node_has_jsdoc_info_with_no_js_type(&h.ast, normal_prop_target);
}

// port: ParserTest#testInlineJSDocAttachmentToObjLitNormalPropKey
#[test]
fn test_inline_js_doc_attachment_to_obj_lit_normal_prop_key() {
    let mut h = Harness::new();
    let tmp1 = h.parse("let x = { /** string */ normalProp: normalProp };");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut x_node = let_node.unwrap().get_first_child(&h.ast);
    let mut object_lit = x_node.unwrap().get_first_child(&h.ast);
    let mut normal_prop = object_lit.unwrap().get_first_child(&h.ast);
    assert_eq!(normal_prop.unwrap().get_token(&h.ast), Token::STRING_KEY);
    assert_node_has_jsdoc_info_with_no_js_type(&h.ast, normal_prop.unwrap());
}

// port: ParserTest#testInlineJSDocAttachmentToObjLitShorthandProp
#[test]
fn test_inline_js_doc_attachment_to_obj_lit_shorthand_prop() {
    let mut h = Harness::new();
    let tmp1 = h.parse("let x = { /** string */ shorthandProp };");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut x_node = let_node.unwrap().get_first_child(&h.ast);
    let mut object_lit = x_node.unwrap().get_first_child(&h.ast);
    let mut shorthand_prop_key = object_lit.unwrap().get_first_child(&h.ast);
    assert_eq!(
        shorthand_prop_key.unwrap().get_token(&h.ast),
        Token::STRING_KEY
    );
    assert_node_has_jsdoc_info_with_no_js_type(&h.ast, shorthand_prop_key.unwrap());
    let mut shorthand_prop_target = shorthand_prop_key.unwrap().get_only_child(&h.ast);
    assert_node_has_no_jsdoc_info(&h.ast, shorthand_prop_target);
}

// port: ParserTest#testInline_BlockCommentAttachment
#[test]
fn test_inline_block_comment_attachment() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(/* blah */ x) {}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut x_node = fn_node
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert!(
        x_node
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("/* blah */")
            >= 0
    );
}

// port: ParserTest#testInline_LineCommentAttachment
#[test]
fn test_inline_line_comment_attachment() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f( // blah\n x) {}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut x_node = fn_node
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert!(
        x_node
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("// blah")
            >= 0
    );
}

// port: ParserTest#testInlineNonJSDocComments_TrailingAndNonTrailing_ParamList
#[test]
fn test_inline_non_js_doc_comments_trailing_and_non_trailing_param_list() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(x /* first */ , /* second */ y ) {}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut param_list_node = fn_node.unwrap().get_second_child(&h.ast);
    let mut x_node = param_list_node.unwrap().get_first_child(&h.ast);
    let mut y_node = param_list_node.unwrap().get_second_child(&h.ast);
    assert!(
        x_node
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast)
            .index_of("/* first */")
            >= 0
    );
    assert_eq!(
        y_node.unwrap().get_non_jsdoc_comment_string(&h.ast),
        JsString::from("/* second */")
    );
}

// port: ParserTest#testInlineNonJSDocTrailingComments_ParamList
#[test]
fn test_inline_non_js_doc_trailing_comments_param_list() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(x /* first */ , y ) {}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut param_list_node = fn_node.unwrap().get_second_child(&h.ast);
    let mut x_node = param_list_node.unwrap().get_first_child(&h.ast);
    let mut y_node = x_node.unwrap().get_next(&h.ast);
    assert!(
        x_node
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast)
            .index_of("/* first */")
            >= 0
    );
    assert!(
        y_node
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .is_empty()
    );
}

// port: ParserTest#testInlineNonJSDocTrailingComments_formalParamList_SingleParam
#[test]
fn test_inline_non_js_doc_trailing_comments_formal_param_list_single_param() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(x /* first */) {}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut param_list_node = fn_node.unwrap().get_second_child(&h.ast);
    let mut x_node = param_list_node.unwrap().get_first_child(&h.ast);
    assert_eq!(x_node.unwrap().get_token(&h.ast), Token::NAME);
    assert!(
        x_node
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast)
            .index_of("/* first */")
            >= 0
    );
}

// port: ParserTest#testInlineNonJSDocTrailingComments_ParamList_MultiLine
#[test]
fn test_inline_non_js_doc_trailing_comments_param_list_multi_line() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(\n  x,// first\n  y // second\n){}\n");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut param_list_node = fn_node.unwrap().get_second_child(&h.ast);
    let mut x_node = param_list_node.unwrap().get_first_child(&h.ast);
    let mut y_node = param_list_node.unwrap().get_second_child(&h.ast);
    assert_eq!(
        x_node
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("// first")
    );
    assert_eq!(
        y_node
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("// second")
    );
}

// port: ParserTest#testInlineNonJSDocTrailingComments_ParamList_MultiLine_BlockComments
#[test]
fn test_inline_non_js_doc_trailing_comments_param_list_multi_line_block_comments() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(\n  x, /* first */\n  y /* second */\n) {}\n");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut param_list_node = fn_node.unwrap().get_second_child(&h.ast);
    let mut x_node = param_list_node.unwrap().get_first_child(&h.ast);
    let mut y_node = param_list_node.unwrap().get_second_child(&h.ast);
    assert_eq!(
        x_node
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("/* first */")
    );
    assert_eq!(
        y_node
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("/* second */")
    );
}

// port: ParserTest#testInlineNonJSDocTrailingComments_ParamList_MultiLine_SingleBlockComments
#[test]
fn test_inline_non_js_doc_trailing_comments_param_list_multi_line_single_block_comments() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(x, /* first */\ny\n) {}\n");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut param_list_node = fn_node.unwrap().get_second_child(&h.ast);
    let mut x_node = param_list_node.unwrap().get_first_child(&h.ast);
    let mut y_node = param_list_node.unwrap().get_second_child(&h.ast);
    assert_eq!(
        x_node
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("/* first */")
    );
    assert!(y_node.unwrap().get_non_jsdoc_comment(&h.ast).is_none());
}

// port: ParserTest#testNonJSDocTrailingCommentOnConstant
#[test]
fn test_non_js_doc_trailing_comment_on_constant() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("const A = 1; // comment");
    let mut cnst = tmp1.get_first_child(&h.ast);
    assert_eq!(cnst.unwrap().get_token(&h.ast), Token::CONST);
    assert_eq!(
        cnst.unwrap().get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("// comment")
    );
}

// port: ParserTest#testNonJSDocTrailingCommentOnConstantNoWhitespace
#[test]
fn test_non_js_doc_trailing_comment_on_constant_no_whitespace() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("const A = 1;// comment");
    let mut cnst = tmp1.get_first_child(&h.ast);
    assert_eq!(cnst.unwrap().get_token(&h.ast), Token::CONST);
    assert_eq!(
        cnst.unwrap().get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("// comment")
    );
}

// port: ParserTest#testNonJSDocTrailingCommentOnConstantWithMoreWhitespace
#[test]
fn test_non_js_doc_trailing_comment_on_constant_with_more_whitespace() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("const A = 1;   // comment");
    let mut cnst = tmp1.get_first_child(&h.ast);
    assert_eq!(cnst.unwrap().get_token(&h.ast), Token::CONST);
    assert_eq!(
        cnst.unwrap().get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("// comment")
    );
}

// port: ParserTest#testNonJSDocTrailingCommentOnConstantFollowedByConstant
#[test]
fn test_non_js_doc_trailing_comment_on_constant_followed_by_constant() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("const A = 1; // comment\n\nconst B = 2;\n");
    let mut cnst = tmp1.get_first_child(&h.ast);
    assert_eq!(cnst.unwrap().get_token(&h.ast), Token::CONST);
    assert_eq!(
        cnst.unwrap().get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("// comment")
    );
}

// port: ParserTest#testMultipleNonJSDocTrailingComments
#[test]
fn test_multiple_non_js_doc_trailing_comments() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("const A = 1; /* A1 */ /* A2 */ // A3\n\nconst B = 2;\n");
    let mut n = tmp1;
    let mut a = n.get_first_child(&h.ast);
    assert_eq!(a.unwrap().get_token(&h.ast), Token::CONST);
    let mut b = n.get_last_child(&h.ast);
    assert_eq!(a.unwrap().get_token(&h.ast), Token::CONST);
    assert_eq!(
        b.unwrap().get_non_jsdoc_comment_string(&h.ast),
        JsString::from("/* A2 */// A3")
    );
}

// port: ParserTest#testNonJSDocTrailingCommentAfterFunction
#[test]
fn test_non_js_doc_trailing_comment_after_function() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function foo(){} // comment");
    let mut n = tmp1.get_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::FUNCTION);
    assert_eq!(
        n.unwrap().get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("// comment")
    );
}

// port: ParserTest#testNonJSDocTrailingCommentAfterFunctionCall
#[test]
fn test_non_js_doc_trailing_comment_after_function_call() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function g(){ f(); // comment\nf();}\n");
    let mut n = tmp1;
    let mut expr_res = n
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(expr_res.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    assert_eq!(
        expr_res
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("// comment")
    );
}

// port: ParserTest#testNonJSDocTrailingCommentAfterFunctionCallInBlock
#[test]
fn test_non_js_doc_trailing_comment_after_function_call_in_block() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 =
        h.parse("if (true) {\n  f1(); // comment1 on f1()\n  // comment2\n  // comment3\n}\n");
    let mut n = tmp1;
    let mut expr_res = n
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(expr_res.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    assert_eq!(
        expr_res
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("// comment1 on f1()\n// comment2\n// comment3")
    );
}

// port: ParserTest#testLastNonJSDocCommentInBlock
#[test]
fn test_last_non_js_doc_comment_in_block() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("if (true) {\n  f();\n  /* comment */\n}\n");
    let mut n = tmp1;
    let mut expr_res = n
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(expr_res.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    assert_eq!(
        expr_res
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("\n/* comment */")
    );
}

// port: ParserTest#testLastNonJSDocCommentInBlockWithBlankLines
#[test]
fn test_last_non_js_doc_comment_in_block_with_blank_lines() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("if (true) {\n  f();\n\n\n  /* comment */\n}\n");
    let mut n = tmp1;
    let mut expr_res = n
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(expr_res.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    assert_eq!(
        expr_res
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("\n/* comment */")
    );
}

// port: ParserTest#testInlineCommentInFunctionCallInBlock
#[test]
fn test_inline_comment_in_function_call_in_block() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("if (true) {\n  f(0, 1 /* comment */);}\n");
    let mut n = tmp1;
    let mut expr_res = n
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(expr_res.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    assert_eq!(
        expr_res
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("\n/* comment */")
    );
}

// port: ParserTest#testNonJSDocBigCommentInbetween
#[test]
fn test_non_js_doc_big_comment_inbetween() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("let x = 0; // comment on x\n//\n// more comment\nlet y = 1;\n");
    let mut n = tmp1;
    let mut fst_let_decl = n.get_first_child(&h.ast);
    let mut snd_let_decl = n.get_last_child(&h.ast);
    assert_eq!(fst_let_decl.unwrap().get_token(&h.ast), Token::LET);
    assert_eq!(
        fst_let_decl
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("// comment on x")
    );
    assert_eq!(snd_let_decl.unwrap().get_token(&h.ast), Token::LET);
    assert_eq!(
        snd_let_decl.unwrap().get_non_jsdoc_comment_string(&h.ast),
        JsString::from("//\n// more comment")
    );
}

// port: ParserTest#testInlineNonJSDocCommentsOnSeparateLetDeclarations
#[test]
fn test_inline_non_js_doc_comments_on_separate_let_declarations() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("let a /* leading a */ = {c} /* trailing */; let /* leading b */ b  = {d};");
    let mut n = tmp1;
    let mut let_a_decl = n.get_first_child(&h.ast);
    let mut let_b_decl = n.get_last_child(&h.ast);
    assert_eq!(let_a_decl.unwrap().get_token(&h.ast), Token::LET);
    assert_eq!(let_b_decl.unwrap().get_token(&h.ast), Token::LET);
    assert!(
        let_a_decl
            .unwrap()
            .get_first_first_child(&h.ast)
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("/* leading a */")
            >= 0
    );
    assert!(
        let_a_decl
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast)
            .index_of("/* trailing */")
            >= 0
    );
    assert!(
        let_b_decl
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("/* leading b */")
            >= 0
    );
}

// port: ParserTest#testMultipleInline_LineCommentsAttachment
#[test]
fn test_multiple_inline_line_comments_attachment() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(\n  // blah1\n  x,\n  // blah2\n  y) {}\n");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut x_node = fn_node
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert!(
        x_node
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("// blah1")
            >= 0
    );
    let mut y_node = fn_node
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap()
        .get_second_child(&h.ast);
    assert_eq!(
        y_node.unwrap().get_non_jsdoc_comment_string(&h.ast),
        JsString::from("// blah2")
    );
}

// port: ParserTest#testMultipleInline_MixedCommentsAttachment
#[test]
fn test_multiple_inline_mixed_comments_attachment() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(\n  /* blah1 */ x,\n  // blah2\n  y\n) {}\n");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut x_node = fn_node
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert!(
        x_node
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("/* blah1 */")
            >= 0
    );
    let mut y_node = fn_node
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap()
        .get_second_child(&h.ast);
    assert_eq!(
        y_node.unwrap().get_non_jsdoc_comment_string(&h.ast),
        JsString::from("// blah2")
    );
}

// port: ParserTest#testMultipleInline_NonJSDocCommentsGetAttachedToSameNode
#[test]
fn test_multiple_inline_non_js_doc_comments_get_attached_to_same_node() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(/* blah1 */\n// blah\n x) {}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut x_node = fn_node
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert!(
        x_node
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of(JsString::from("/* blah1 */\n// blah"))
            >= 0
    );
}

// port: ParserTest#testBoth_TrailingAndNonTrailing_NonJSDocCommentsGetAttachedToSameNode_MultiLine
#[test]
fn test_both_trailing_and_non_trailing_non_js_doc_comments_get_attached_to_same_node_multi_line() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(\n     /* blah1 */\n     x // blah\n  ) {}\n");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut x_node = fn_node
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert!(
        x_node
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("/* blah1 */")
            >= 0
    );
    assert!(
        x_node
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast)
            .index_of("// blah")
            >= 0
    );
}

// port: ParserTest#testEndOfFileNonJSDocComments_lineComment
#[test]
fn test_end_of_file_non_js_doc_comments_line_comment() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f1() {}\n// first\nf1();\n// second\n");
    let mut script_node = tmp1;
    assert_eq!(script_node.get_token(&h.ast), Token::SCRIPT);
    let mut expr_node = script_node.get_last_child(&h.ast);
    assert_eq!(expr_node.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    assert_eq!(
        script_node.get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("// second")
    );
}

// port: ParserTest#testEndOfFileNonJSDocComments_blockComment
#[test]
fn test_end_of_file_non_js_doc_comments_block_comment() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f1() {}\n// first\nf1();\n/* second */\n");
    let mut script_node = tmp1;
    assert_eq!(script_node.get_token(&h.ast), Token::SCRIPT);
    let mut expr_node = script_node.get_last_child(&h.ast);
    assert_eq!(expr_node.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    assert_eq!(
        script_node.get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("/* second */")
    );
}

// port: ParserTest#testEndOfFileNonJSDocComments_manyComments
#[test]
fn test_end_of_file_non_js_doc_comments_many_comments() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f1() {}\n// first\nf1();\n// second\n/* third */\n// fourth\n");
    let mut script_node = tmp1;
    assert_eq!(script_node.get_token(&h.ast), Token::SCRIPT);
    let mut expr_node = script_node.get_last_child(&h.ast);
    assert_eq!(expr_node.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    assert_eq!(
        script_node.get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("// second\n/* third */\n// fourth")
    );
}

// port: ParserTest#testEndOfFileNonJSDocComments
#[test]
fn test_end_of_file_non_js_doc_comments() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f1() {}\nif (true) {\n// first\nf1(); // second\n}\n// third\n");
    let mut script_node = tmp1;
    assert_eq!(script_node.get_token(&h.ast), Token::SCRIPT);
    let mut expr_node = script_node
        .get_last_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(expr_node.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    assert_eq!(
        expr_node.unwrap().get_non_jsdoc_comment_string(&h.ast),
        JsString::from("// first")
    );
    let mut call_node = script_node
        .get_last_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast);
    assert_eq!(
        call_node
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("// second")
    );
    assert_eq!(
        script_node.get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("// third")
    );
}

// port: ParserTest#testBoth_TrailingAndNonTrailing_NonJSDocCommentsGetAttachedToSameNode_SingleLine
#[test]
fn test_both_trailing_and_non_trailing_non_js_doc_comments_get_attached_to_same_node_single_line() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(/* blah1 */ x // blah\n) {}\n");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut x_node = fn_node
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert!(
        x_node
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("/* blah1 */")
            >= 0
    );
    assert!(
        x_node
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast)
            .index_of("/ blah")
            >= 0
    );
}

// port: ParserTest#testInlineTrailingNonJSDocComments_FunctionArgsAndBody
#[test]
fn test_inline_trailing_non_js_doc_comments_function_args_and_body() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(x /* first */ ) { /* second */ let y;}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut x_node = fn_node
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap()
        .get_only_child(&h.ast);
    let mut y_node = fn_node
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert!(
        x_node
            .get_trailing_non_jsdoc_comment_string(&h.ast)
            .index_of("/* first */")
            >= 0
    );
    assert!(
        y_node
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("/* second */")
            >= 0
    );
}

// port: ParserTest#testInlineNonJSDocComments_FunctionCall
#[test]
fn test_inline_non_js_doc_comments_function_call() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(x) {let y;}; f( /* first */  1)");
    let mut expr_res = tmp1.get_last_child(&h.ast);
    assert_eq!(expr_res.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    let mut call = expr_res.unwrap().get_first_child(&h.ast);
    let mut one_arg_node = call.unwrap().get_second_child(&h.ast);
    assert!(
        one_arg_node
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("/* first */")
            >= 0
    );
}

// port: ParserTest#testInlineTrailingNonJSDocComments_MultipleArgs
#[test]
fn test_inline_trailing_non_js_doc_comments_multiple_args() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(x, y) {}; f( 1 /* first */, 2 );");
    let mut expr_res = tmp1.get_last_child(&h.ast);
    assert_eq!(expr_res.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    let mut call = expr_res.unwrap().get_first_child(&h.ast);
    let mut one_arg_node = call.unwrap().get_second_child(&h.ast);
    let mut two_arg_node = one_arg_node.unwrap().get_next(&h.ast);
    assert!(
        one_arg_node
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast)
            .index_of("/* first */")
            >= 0
    );
    assert!(
        two_arg_node
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .is_empty()
    );
}

// port: ParserTest#testInlineTrailingNonJSDocComments_SingleArgument
#[test]
fn test_inline_trailing_non_js_doc_comments_single_argument() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(x, y) {}; f( 1 /* first */);");
    let mut expr_res = tmp1.get_last_child(&h.ast);
    assert_eq!(expr_res.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    let mut call = expr_res.unwrap().get_first_child(&h.ast);
    let mut one_arg_node = call.unwrap().get_second_child(&h.ast);
    assert!(
        one_arg_node
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast)
            .index_of("/* first */")
            >= 0
    );
}

// port: ParserTest#testInlineJSDocAttachment3
#[test]
fn test_inline_js_doc_attachment3() {
    let mut h = Harness::new();
    let tmp1 = h.parse("function f(/** @type {string} */ x) {}");
    let _ = tmp1;
}

// port: ParserTest#testInlineJSDocAttachment4
#[test]
fn test_inline_js_doc_attachment4() {
    let mut h = Harness::new();
    let tmp1 = h.parse("function f(/**\n * @type {string}\n */ x) {}\n");
    let _ = tmp1;
}

// port: ParserTest#testInlineJSDocWithOptionalType
#[test]
fn test_inline_js_doc_with_optional_type() {
    let mut h = Harness::new();
    let tmp1 = h.parse("function f(/** string= */ x) {}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut info = fn_node
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap()
        .get_jsdoc_info(&h.ast);
    assert!(
        info.as_ref()
            .unwrap()
            .get_type()
            .as_ref()
            .unwrap()
            .is_optional_arg(&h.ast)
    );
}

// port: ParserTest#testInlineJSDocWithVarArgs
#[test]
fn test_inline_js_doc_with_var_args() {
    let mut h = Harness::new();
    let tmp1 = h.parse("function f(/** ...string */ x) {}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut info = fn_node
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap()
        .get_jsdoc_info(&h.ast);
    assert!(
        info.as_ref()
            .unwrap()
            .get_type()
            .as_ref()
            .unwrap()
            .is_var_args(&h.ast)
    );
}

// port: ParserTest#testIncorrectJSDocDoesNotAlterJSParsing1
#[test]
fn test_incorrect_js_doc_does_not_alter_js_parsing1() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var a = [1,2]");
    let tmp2 = h.parse_warning(
        "/** @type {Array<number} */var a = [1,2]",
        &[MISSING_GT_MESSAGE],
    );
    assert_node_equality(&h.ast, tmp1, tmp2);
}

// port: ParserTest#testIncorrectJSDocDoesNotAlterJSParsing2
#[test]
fn test_incorrect_js_doc_does_not_alter_js_parsing2() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var a = [1,2]");
    let tmp2 = h.parse_warning(
        "/** @type {Array.<number}*/var a = [1,2]",
        &[MISSING_GT_MESSAGE],
    );
    assert_node_equality(&h.ast, tmp1, tmp2);
}

// port: ParserTest#testIncorrectJSDocDoesNotAlterJSParsing3
#[test]
fn test_incorrect_js_doc_does_not_alter_js_parsing3() {
    let mut h = Harness::new();
    let tmp1 = h.parse("C.prototype.say=function(nums) {alert(nums.join(','));};");
    let tmp2 = h.parse_warning("/** @param {Array.<number} nums */\nC.prototype.say=function(nums) {alert(nums.join(','));};\n", &[MISSING_GT_MESSAGE]);
    assert_node_equality(&h.ast, tmp1, tmp2);
}

// port: ParserTest#testIncorrectJSDocDoesNotAlterJSParsing4
#[test]
fn test_incorrect_js_doc_does_not_alter_js_parsing4() {
    let mut h = Harness::new();
    let tmp1 = h.parse("C.prototype.say=function(nums) {alert(nums.join(','));};");
    let tmp2 = h.parse(
        "/** @return {boolean} */\nC.prototype.say=function(nums) {alert(nums.join(','));};\n",
    );
    assert_node_equality(&h.ast, tmp1, tmp2);
}

// port: ParserTest#testIncorrectJSDocDoesNotAlterJSParsing5
#[test]
fn test_incorrect_js_doc_does_not_alter_js_parsing5() {
    let mut h = Harness::new();
    let tmp1 = h.parse("C.prototype.say=function(nums) {alert(nums.join(','));};");
    let tmp2 = h.parse("/** @param {boolean} this is some string*/\nC.prototype.say=function(nums) {alert(nums.join(','));};\n");
    assert_node_equality(&h.ast, tmp1, tmp2);
}

// port: ParserTest#testIncorrectJSDocDoesNotAlterJSParsing6
#[test]
fn test_incorrect_js_doc_does_not_alter_js_parsing6() {
    let mut h = Harness::new();
    let tmp1 = h.parse("C.prototype.say=function(nums) {alert(nums.join(','));};");
    let tmp2 = h.parse_warning(
        "/** @param {bool!*%E$} */\nC.prototype.say=function(nums) {alert(nums.join(','));};\n",
        &[
            &format!(
                "{}{}",
                "Bad type annotation. expected closing }", BAD_TYPE_WIKI_LINK
            ),
            &format!(
                "{}{}",
                "Bad type annotation. expecting a variable name in a @param tag.",
                BAD_TYPE_WIKI_LINK
            ),
        ],
    );
    assert_node_equality(&h.ast, tmp1, tmp2);
}

// port: ParserTest#testIncorrectJSDocDoesNotAlterJSParsing7
#[test]
fn test_incorrect_js_doc_does_not_alter_js_parsing7() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    let tmp1 = h.parse("C.prototype.say=function(nums) {alert(nums.join(','));};");
    let tmp2 = h.parse_warning(
        "/** @see */\nC.prototype.say=function(nums) {alert(nums.join(','));};\n",
        &["@see tag missing description"],
    );
    assert_node_equality(&h.ast, tmp1, tmp2);
}

// port: ParserTest#testIncorrectJSDocDoesNotAlterJSParsing8
#[test]
fn test_incorrect_js_doc_does_not_alter_js_parsing8() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    let tmp1 = h.parse("C.prototype.say=function(nums) {alert(nums.join(','));};");
    let tmp2 = h.parse_warning(
        "/** @author */\nC.prototype.say=function(nums) {alert(nums.join(','));};\n",
        &["@author tag missing author"],
    );
    assert_node_equality(&h.ast, tmp1, tmp2);
}

// port: ParserTest#testIncorrectJSDocDoesNotAlterJSParsing9
#[test]
fn test_incorrect_js_doc_does_not_alter_js_parsing9() {
    let mut h = Harness::new();
    let tmp1 = h.parse("C.prototype.say=function(nums) {alert(nums.join(','));};");
    let tmp2 = h.parse_warning(
        "/** @someillegaltag */\nC.prototype.say=function(nums) {alert(nums.join(','));};\n",
        &[&format!(
            "{}{}",
            "illegal use of unknown JSDoc tag \"someillegaltag\"; ignoring it. Place another",
            " character before the @ to stop JSCompiler from parsing it as an annotation."
        )],
    );
    assert_node_equality(&h.ast, tmp1, tmp2);
}

// port: ParserTest#testMisplacedDescAnnotation_noWarning
#[test]
fn test_misplaced_desc_annotation_no_warning() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/** @desc Foo. */ var MSG_BAR = goog.getMsg('hello');");
    let _ = tmp1;
    let tmp2 = h.parse("/** @desc Foo. */ x.y.z.MSG_BAR = goog.getMsg('hello');");
    let _ = tmp2;
    let tmp3 = h.parse("/** @desc Foo. */ MSG_BAR = goog.getMsg('hello');");
    let _ = tmp3;
    let tmp4 = h.parse("var msgs = {/** @desc x */ MSG_X: goog.getMsg('x')}");
    let _ = tmp4;
}

// port: ParserTest#testUnescapedSlashInRegexpCharClass
#[test]
fn test_unescaped_slash_in_regexp_char_class() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var foo = /[/]/;");
    let _ = tmp1;
    let tmp2 = h.parse("var foo = /[hi there/]/;");
    let _ = tmp2;
    let tmp3 = h.parse("var foo = /[/yo dude]/;");
    let _ = tmp3;
    let tmp4 = h.parse("var foo = /\\/[@#$/watashi/wa/suteevu/desu]/;");
    let _ = tmp4;
}

// port: ParserTest#testMalformedRegexp
#[test]
fn test_malformed_regexp() {
    let mut h = Harness::new();
    let mut js = ("var x = com\\").to_string();
    let tmp1 = h.parse_error(js.as_str(), &["Invalid escape sequence"]);
    let _ = tmp1;
    js = "(function() {\n  var url=\"\";\n  switch(true)\n  {\n    case /a.com\\/g|l.i/N/.test(url):\n      return \"\";\n    case /b.com\\/T/.test(url):\n      return \"\";\n  }\n}\n)();\n".to_string();
    let tmp2 = h.parse_error(js.as_str(), &["primary expression expected"]);
    let _ = tmp2;
}

// port: ParserTest#testPostfixExpression
#[test]
fn test_postfix_expression() {
    let mut h = Harness::new();
    let tmp1 = h.parse("a++");
    let _ = tmp1;
    let tmp2 = h.parse("a.b--");
    let _ = tmp2;
    let tmp3 = h.parse("a[0]++");
    let _ = tmp3;
    let tmp4 = h.parse("/** @type {number} */ (a)++;");
    let _ = tmp4;
    let tmp5 = h.parse_error("a()++", &["Invalid postfix increment operand."]);
    let _ = tmp5;
    let tmp6 = h.parse_error("(new C)--", &["Invalid postfix decrement operand."]);
    let _ = tmp6;
    let tmp7 = h.parse_error("this++", &["Invalid postfix increment operand."]);
    let _ = tmp7;
    let tmp8 = h.parse_error("(a--)++", &["Invalid postfix increment operand."]);
    let _ = tmp8;
    let tmp9 = h.parse_error("(+a)++", &["Invalid postfix increment operand."]);
    let _ = tmp9;
    let tmp10 = h.parse_error("[1,2]++", &["Invalid postfix increment operand."]);
    let _ = tmp10;
    let tmp11 = h.parse_error("'literal'++", &["Invalid postfix increment operand."]);
    let _ = tmp11;
    let tmp12 = h.parse_error(
        "/** @type {number} */ (a())++;",
        &["Invalid postfix increment operand."],
    );
    let _ = tmp12;
}

// port: ParserTest#testUnaryExpression
#[test]
fn test_unary_expression() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("delete a.b");
    let _ = tmp1;
    let tmp2 = h.parse("delete a[0]");
    let _ = tmp2;
    let tmp3 = h.parse("void f()");
    let _ = tmp3;
    let tmp4 = h.parse("typeof new C");
    let _ = tmp4;
    let tmp5 = h.parse("++a[0]");
    let _ = tmp5;
    let tmp6 = h.parse("--a.b");
    let _ = tmp6;
    let tmp7 = h.parse("+{a: 1}");
    let _ = tmp7;
    let tmp8 = h.parse("-[1,2]");
    let _ = tmp8;
    let tmp9 = h.parse("~'42'");
    let _ = tmp9;
    h.expect_features(&[Feature::SUPER]);
    let tmp10 = h.parse("!super.a");
    let _ = tmp10;
    h.expect_features(&[]);
    let tmp11 = h.parse_error(
        "delete f()",
        &["Invalid delete operand. Only properties can be deleted."],
    );
    let _ = tmp11;
    let tmp12 = h.parse_error("++a++", &["Invalid prefix increment operand."]);
    let _ = tmp12;
    let tmp13 = h.parse_error("--{a: 1}", &["Invalid prefix decrement operand."]);
    let _ = tmp13;
    let tmp14 = h.parse_error("++this", &["Invalid prefix increment operand."]);
    let _ = tmp14;
    let tmp15 = h.parse_error("++(-a)", &["Invalid prefix increment operand."]);
    let _ = tmp15;
    let tmp16 = h.parse_error("++{a: 1}", &["Invalid prefix increment operand."]);
    let _ = tmp16;
    let tmp17 = h.parse_error("++'literal'", &["Invalid prefix increment operand."]);
    let _ = tmp17;
    let tmp18 = h.parse_error("++delete a.b", &["Invalid prefix increment operand."]);
    let _ = tmp18;
}

// port: ParserTest#testUnaryExpressionWithBigInt
#[test]
fn test_unary_expression_with_big_int() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("+1n", &["Cannot convert a BigInt value to a number"]);
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "delete 6n",
        &["Invalid delete operand. Only properties can be deleted."],
    );
    let _ = tmp2;
}

// port: ParserTest#testAutomaticSemicolonInsertion
#[test]
fn test_automatic_semicolon_insertion() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var x = 1\nvar y = 2");
    let tmp2 = h.parse("var x = 1; var y = 2;");
    assert_node_equality(&h.ast, tmp1, tmp2);
    let tmp3 = h.parse("var x = 1\n, y = 2");
    let tmp4 = h.parse("var x = 1, y = 2;");
    assert_node_equality(&h.ast, tmp3, tmp4);
    let tmp5 = h.parse("x = 1\ny = 2");
    let tmp6 = h.parse("x = 1; y = 2;");
    assert_node_equality(&h.ast, tmp5, tmp6);
    let tmp7 = h.parse("x = 1\n;y = 2");
    let tmp8 = h.parse("x = 1; y = 2;");
    assert_node_equality(&h.ast, tmp7, tmp8);
    let tmp9 = h.parse("if (true) 1\n; else {}");
    let tmp10 = h.parse("if (true) 1; else {}");
    assert_node_equality(&h.ast, tmp9, tmp10);
    let tmp11 = h.parse("if (x)\n;else{}");
    let tmp12 = h.parse("if (x) {} else {}");
    assert_node_equality(&h.ast, tmp11, tmp12);
}

// port: ParserTest#testAutomaticSemicolonInsertion_curly
#[test]
fn test_automatic_semicolon_insertion_curly() {
    let mut h = Harness::new();
    let tmp1 = h.parse("while (true) { 1 }");
    let tmp2 = h.parse("while (true) { 1; }");
    assert_node_equality(&h.ast, tmp1, tmp2);
}

// port: ParserTest#testAutomaticSemicolonInsertion_doWhile
#[test]
fn test_automatic_semicolon_insertion_do_while() {
    let mut h = Harness::new();
    let tmp1 = h.parse("do {} while (true) 1;");
    let tmp2 = h.parse("do {} while (true); 1;");
    assert_node_equality(&h.ast, tmp1, tmp2);
}

// port: ParserTest#testAutomaticSemicolonInsertion_examplesFromSpec
#[test]
fn test_automatic_semicolon_insertion_examples_from_spec() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("{ 1 2 } 3", &[SEMICOLON_EXPECTED]);
    let _ = tmp1;
    let tmp2 = h.parse("{ 1\n2 } 3");
    let tmp3 = h.parse("{ 1; 2; } 3;");
    assert_node_equality(&h.ast, tmp2, tmp3);
    let tmp4 = h.parse_error("for (a; b\n)", &["';' expected"]);
    let _ = tmp4;
    let tmp5 = h.parse("function f() { return\na + b }");
    let tmp6 = h.parse("function f() { return; a + b; }");
    assert_node_equality(&h.ast, tmp5, tmp6);
    let tmp7 = h.parse("a = b\n++c");
    let tmp8 = h.parse("a = b; ++c;");
    assert_node_equality(&h.ast, tmp7, tmp8);
    let tmp9 = h.parse_error("if (a > b)\nelse c = d", &["primary expression expected"]);
    let _ = tmp9;
    let tmp10 = h.parse("a = b + c\n(d + e).print()");
    let tmp11 = h.parse("a = b + c(d + e).print()");
    assert_node_equality(&h.ast, tmp10, tmp11);
}

// port: ParserTest#testAutomaticSemicolonInsertion_restrictedRules
#[test]
fn test_automatic_semicolon_insertion_restricted_rules() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("x\n++;", &["primary expression expected"]);
    let _ = tmp1;
    let tmp2 = h.parse("function f() { return\n1; }");
    let tmp3 = h.parse("function f() { return;1; }");
    assert_node_equality(&h.ast, tmp2, tmp3);
    let tmp4 = h.parse("while (true) { continue\nlabel; }");
    let tmp5 = h.parse("while (true) { continue; label; }");
    assert_node_equality(&h.ast, tmp4, tmp5);
    let tmp6 = h.parse("while (true) { break\nlabel; }");
    let tmp7 = h.parse("while (true) { break; label; }");
    assert_node_equality(&h.ast, tmp6, tmp7);
    let tmp8 = h.parse_error(
        "throw\n1;",
        &["semicolon/newline not allowed after 'throw'"],
    );
    let _ = tmp8;
    let tmp9 = h.parse_error("yield\nvalue;", &["primary expression expected"]);
    let _ = tmp9;
    let tmp10 = h.parse_error("()\n=> 1;", &["No newline allowed before '=>'"]);
    let _ = tmp10;
}

// port: ParserTest#testMethodInObjectLiteral
#[test]
fn test_method_in_object_literal() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::MEMBER_DECLARATIONS]);
    h.test_method_in_object_literal("var a = {b() {}};");
    h.test_method_in_object_literal("var a = {b() { alert('b'); }};");
    h.expect_features(&[]);
    let tmp1 = h.parse_error(
        "var a = {static b() { alert('b'); }};",
        &["Cannot use keyword in short object literal"],
    );
    let _ = tmp1;
}

// port: ParserTest#testBigIntAsObjectLiteralPropertyName
#[test]
fn test_big_int_as_object_literal_property_name() {
    let mut h = Harness::new();
    let tmp1 = h.parse("({1n() {}, 1n: 0})");
    let mut object_lit = tmp1.get_only_child(&h.ast).get_only_child(&h.ast);
    assert_eq!(object_lit.get_token(&h.ast), Token::OBJECTLIT);
    let mut computed_prop = object_lit.get_first_child(&h.ast);
    assert_eq!(
        computed_prop.unwrap().get_token(&h.ast),
        Token::COMPUTED_PROP
    );
    let mut bigint = computed_prop.unwrap().get_first_child(&h.ast);
    assert_eq!(bigint.unwrap().get_token(&h.ast), Token::BIGINT);
    let mut string_key = object_lit.get_last_child(&h.ast);
    assert_eq!(string_key.unwrap().get_token(&h.ast), Token::STRING_KEY);
}

// port: ParserTest#testShorthandObjectProperty
#[test]
fn test_shorthand_object_property() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::SHORTHAND_OBJECT_PROPERTIES]);
    h.test_shorthand_object_property("var a = {b};");
    h.test_shorthand_object_property("var a = {b, c};");
    h.test_shorthand_object_property("var a = {b, c: d, e};");
    h.test_shorthand_object_property("var a = {type};");
    h.test_shorthand_object_property("var a = {declare};");
    h.test_shorthand_object_property("var a = {namespace};");
    h.test_shorthand_object_property("var a = {module};");
    h.expect_features(&[]);
    let tmp1 = h.parse_error("var a = { '!@#$%' };", &["':' expected"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("var a = { 123 };", &["':' expected"]);
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "var a = { let };",
        &["Cannot use keyword in short object literal"],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "var a = { else };",
        &["Cannot use keyword in short object literal"],
    );
    let _ = tmp4;
}

// port: ParserTest#testComputedPropertiesObjLit
#[test]
fn test_computed_properties_obj_lit() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::COMPUTED_PROPERTIES]);
    h.test_computed_property("var x = {  [prop + '_']() {} }");
    h.test_computed_property("var x = {  'abc'() {} }");
    h.test_computed_property("var x = {  123() {} }");
    h.test_computed_property("var x = {  get [prop + '_']() {} }");
    h.test_computed_property("var x = { set [prop + '_'](val) {} }");
    h.mode = LanguageMode::ECMASCRIPT_2015;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("var x = { *[prop + '_']() {} }");
    let _ = tmp1;
    let tmp2 = h.parse("var x = { *'abc'() {} }");
    let _ = tmp2;
    let tmp3 = h.parse("var x = { *123() {} }");
    let _ = tmp3;
    h.mode = LanguageMode::ECMASCRIPT_2017;
    let tmp4 = h.parse("var x = { async [prop + '_']() {} }");
    let _ = tmp4;
    let tmp5 = h.parse("var x = { async 'abc'() {} }");
    let _ = tmp5;
    let tmp6 = h.parse("var x = { async 123() {} }");
    let _ = tmp6;
}

// port: ParserTest#testComputedMethodClass
#[test]
fn test_computed_method_class() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::CLASSES, Feature::COMPUTED_PROPERTIES]);
    let tmp1 = h.parse("class X { [prop + '_']() {} }");
    let _ = tmp1;
    let tmp2 = h.parse("class X { 'abc'() {} }");
    let _ = tmp2;
    let tmp3 = h.parse("class X { 123() {} }");
    let _ = tmp3;
    let tmp4 = h.parse("class X { static [prop + '_']() {} }");
    let _ = tmp4;
    let tmp5 = h.parse("class X { static 'abc'() {} }");
    let _ = tmp5;
    let tmp6 = h.parse("class X { static 123() {} }");
    let _ = tmp6;
    let tmp7 = h.parse("class X { *[prop + '_']() {} }");
    let _ = tmp7;
    let tmp8 = h.parse("class X { *'abc'() {} }");
    let _ = tmp8;
    let tmp9 = h.parse("class X { *123() {} }");
    let _ = tmp9;
    let tmp10 = h.parse("class X { async [prop + '_']() {} }");
    let _ = tmp10;
    let tmp11 = h.parse("class X { async 'abc'() {} }");
    let _ = tmp11;
    let tmp12 = h.parse("class X { async 123() {} }");
    let _ = tmp12;
}

// port: ParserTest#testBigIntComputedMethodClass
#[test]
fn test_big_int_computed_method_class() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::CLASSES, Feature::COMPUTED_PROPERTIES]);
    let mut class_members: Option<NodeId>;
    let mut computed_prop: Option<NodeId>;
    let mut bigint: Option<NodeId>;
    let mut bigint_value =
        closure_rhino::java_lang::parse_big_integer(&JsString::from("123"), 10).unwrap();
    let tmp1 = h.parse("class X { 123n() {} }");
    class_members = tmp1.get_only_child(&h.ast).get_last_child(&h.ast);
    assert_eq!(
        class_members.unwrap().get_token(&h.ast),
        Token::CLASS_MEMBERS
    );
    computed_prop = Some(class_members.unwrap().get_only_child(&h.ast));
    assert_eq!(
        computed_prop.unwrap().get_token(&h.ast),
        Token::COMPUTED_PROP
    );
    bigint = computed_prop.unwrap().get_first_child(&h.ast);
    assert_eq!(bigint.unwrap().get_token(&h.ast), Token::BIGINT);
    assert!(bigint.unwrap().is_big_int(&h.ast));
    assert_eq!(*bigint.unwrap().get_big_int(&h.ast), bigint_value);
    let tmp2 = h.parse("class X { static 123n() {} }");
    class_members = tmp2.get_only_child(&h.ast).get_last_child(&h.ast);
    assert_eq!(
        class_members.unwrap().get_token(&h.ast),
        Token::CLASS_MEMBERS
    );
    computed_prop = Some(class_members.unwrap().get_only_child(&h.ast));
    assert_eq!(
        computed_prop.unwrap().get_token(&h.ast),
        Token::COMPUTED_PROP
    );
    assert!(computed_prop.unwrap().is_static_member(&h.ast));
    bigint = computed_prop.unwrap().get_first_child(&h.ast);
    assert_eq!(bigint.unwrap().get_token(&h.ast), Token::BIGINT);
    assert!(bigint.unwrap().is_big_int(&h.ast));
    assert_eq!(*bigint.unwrap().get_big_int(&h.ast), bigint_value);
    let tmp3 = h.parse("class X { *123n() {} }");
    class_members = tmp3.get_only_child(&h.ast).get_last_child(&h.ast);
    assert_eq!(
        class_members.unwrap().get_token(&h.ast),
        Token::CLASS_MEMBERS
    );
    computed_prop = Some(class_members.unwrap().get_only_child(&h.ast));
    assert_eq!(
        computed_prop.unwrap().get_token(&h.ast),
        Token::COMPUTED_PROP
    );
    bigint = computed_prop.unwrap().get_first_child(&h.ast);
    assert_eq!(bigint.unwrap().get_token(&h.ast), Token::BIGINT);
    assert!(bigint.unwrap().is_big_int(&h.ast));
    assert_eq!(*bigint.unwrap().get_big_int(&h.ast), bigint_value);
    let tmp4 = h.parse("class X { async 123n() {} }");
    class_members = tmp4.get_only_child(&h.ast).get_last_child(&h.ast);
    assert_eq!(
        class_members.unwrap().get_token(&h.ast),
        Token::CLASS_MEMBERS
    );
    computed_prop = Some(class_members.unwrap().get_only_child(&h.ast));
    assert_eq!(
        computed_prop.unwrap().get_token(&h.ast),
        Token::COMPUTED_PROP
    );
    bigint = computed_prop.unwrap().get_first_child(&h.ast);
    assert_eq!(bigint.unwrap().get_token(&h.ast), Token::BIGINT);
    assert!(bigint.unwrap().is_big_int(&h.ast));
    assert_eq!(*bigint.unwrap().get_big_int(&h.ast), bigint_value);
}

// port: ParserTest#testComputedProperty
#[test]
fn test_computed_property() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::COMPUTED_PROPERTIES]);
    h.test_computed_property(
        "var prop = 'some complex expression';\n\nvar x = {\n  [prop]: 'foo'\n}\n",
    );
    h.test_computed_property(
        "var prop = 'some complex expression';\n\nvar x = {\n  [prop + '!']: 'foo'\n}\n",
    );
    h.test_computed_property("var prop;\n\nvar x = {\n  [prop = 'some expr']: 'foo'\n}\n");
    h.test_computed_property("var x = {\n  [1 << 8]: 'foo'\n}\n");
    let mut js = ("var x = {\n  [1 << 8]: 'foo',\n  [1 << 7]: 'bar'\n}\n").to_string();
    h.mode = LanguageMode::ECMASCRIPT_2015;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse(js.as_str());
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT5;
    let mut warning = (requires_language_mode_message(Feature::COMPUTED_PROPERTIES)).to_string();
    let tmp2 = h.parse_warning(js.as_str(), &[&warning, &warning]);
    let _ = tmp2;
}

// port: ParserTest#testTrailingCommaWarning1
#[test]
fn test_trailing_comma_warning1() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var a = ['foo', 'bar'];");
    let _ = tmp1;
}

// port: ParserTest#testTrailingCommaWarning2
#[test]
fn test_trailing_comma_warning2() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var a = ['foo',,'bar'];");
    let _ = tmp1;
}

// port: ParserTest#testTrailingCommaWarning3
#[test]
fn test_trailing_comma_warning3() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    h.expect_features(&[Feature::TRAILING_COMMA]);
    let tmp1 = h.parse_warning("var a = ['foo', 'bar',];", &[TRAILING_COMMA_MESSAGE]);
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT5;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp2 = h.parse("var a = ['foo', 'bar',];");
    let _ = tmp2;
}

// port: ParserTest#testTrailingCommaWarning4
#[test]
fn test_trailing_comma_warning4() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    h.expect_features(&[Feature::TRAILING_COMMA]);
    let tmp1 = h.parse_warning("var a = [,];", &[TRAILING_COMMA_MESSAGE]);
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT5;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp2 = h.parse("var a = [,];");
    let _ = tmp2;
}

// port: ParserTest#testTrailingCommaWarning5
#[test]
fn test_trailing_comma_warning5() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var a = {'foo': 'bar'};");
    let _ = tmp1;
}

// port: ParserTest#testTrailingCommaWarning6
#[test]
fn test_trailing_comma_warning6() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    h.expect_features(&[Feature::TRAILING_COMMA]);
    let tmp1 = h.parse_warning("var a = {'foo': 'bar',};", &[TRAILING_COMMA_MESSAGE]);
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT5;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp2 = h.parse("var a = {'foo': 'bar',};");
    let _ = tmp2;
}

// port: ParserTest#testTrailingCommaWarning7
#[test]
fn test_trailing_comma_warning7() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("var a = {,};", &["'}' expected"]);
    let _ = tmp1;
}

// port: ParserTest#testCatchClauseForbidden
#[test]
fn test_catch_clause_forbidden() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("try { } catch (e if true) {}", &["')' expected"]);
    let _ = tmp1;
}

// port: ParserTest#testConstForbidden
#[test]
fn test_const_forbidden() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT5;
    h.expect_features(&[Feature::CONST_DECLARATIONS]);
    let tmp1 = h.parse_warning(
        "const x = 3;",
        &[&requires_language_mode_message(Feature::CONST_DECLARATIONS)],
    );
    let _ = tmp1;
}

// port: ParserTest#testAnonymousFunctionExpression
#[test]
fn test_anonymous_function_expression() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT5;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("function () {}", &["'identifier' expected"]);
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    let tmp2 = h.parse_error("function () {}", &["'identifier' expected"]);
    let _ = tmp2;
    h.is_ide_mode = true;
    let tmp3 = h.parse_error(
        "function () {}",
        &["'identifier' expected", "unnamed function statement"],
    );
    let _ = tmp3;
}

// port: ParserTest#testAnonymousFunctionExpressionInClosureUnawareCode
#[test]
fn test_anonymous_function_expression_in_closure_unaware_code() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT_2015;
    let tmp1 = h.parse_error(
        "/** @closureUnaware */ (function() { function () {} }).call(globalThis)",
        &["'identifier' expected"],
    );
    let _ = tmp1;
}

// port: ParserTest#testArrayDestructuringVar
#[test]
fn test_array_destructuring_var() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT5;
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::ARRAY_DESTRUCTURING]);
    let tmp1 = h.parse_warning(
        "var [x,y] = foo();",
        &[&requires_language_mode_message(
            Feature::ARRAY_DESTRUCTURING,
        )],
    );
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    let tmp2 = h.parse("var [x,y] = foo();");
    let _ = tmp2;
}

// port: ParserTest#testLHSOfNonVanillaEqualsOperator
#[test]
fn test_lhs_of_non_vanilla_equals_operator() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    let tmp1 = h.parse("for (let [i,j] = [2,0]; j < 2; [i,j]  =  [j, j+1]) {}");
    let _ = tmp1;
    let tmp2 = h.parse("for (let [i,j] = [2,0]; j < 2; {i,j}  =  [j, j+1]) {}");
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "for (let [i,j] = [2,0]; j < 2; [i,j]  +=  [j, j+1]) {}",
        &["invalid assignment target"],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "for (let [i,j] = [2,0]; j < 2; {i,j}  +=  [j, j+1]) {}",
        &["invalid assignment target"],
    );
    let _ = tmp4;
    let tmp5 = h.parse("let [i,j]  =  [2, 2];");
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "let [i,j]  +=  [2, 2];",
        &["destructuring must have an initializer"],
    );
    let _ = tmp6;
}

// port: ParserTest#testArrayDestructuringVarInvalid
#[test]
fn test_array_destructuring_var_invalid() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "var [x,y[15]] = foo();",
        &["Only an identifier or destructuring pattern is allowed here."],
    );
    let _ = tmp1;
}

// port: ParserTest#testArrayDestructuringAssign
#[test]
fn test_array_destructuring_assign() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT5;
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::ARRAY_DESTRUCTURING]);
    let tmp1 = h.parse_warning(
        "[x,y] = foo();",
        &[&requires_language_mode_message(
            Feature::ARRAY_DESTRUCTURING,
        )],
    );
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    let tmp2 = h.parse("[x,y] = foo();");
    let _ = tmp2;
    let tmp3 = h.parse("[x,y[15]] = foo();");
    let _ = tmp3;
}

// port: ParserTest#testArrayDestructuringInitializer
#[test]
fn test_array_destructuring_initializer() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::ARRAY_DESTRUCTURING]);
    let tmp1 = h.parse("var [x=1,y] = foo();");
    let _ = tmp1;
    let tmp2 = h.parse("[x=1,y] = foo();");
    let _ = tmp2;
    let tmp3 = h.parse("var [x,y=2] = foo();");
    let _ = tmp3;
    let tmp4 = h.parse("[x,y=2] = foo();");
    let _ = tmp4;
    let tmp5 = h.parse("var [[a] = ['b']] = [];");
    let _ = tmp5;
    let tmp6 = h.parse("[[a] = ['b']] = [];");
    let _ = tmp6;
    let tmp7 = h.parse("[[a.x] = ['b']] = [];");
    let _ = tmp7;
}

// port: ParserTest#testArrayDestructuringInitializerInvalid
#[test]
fn test_array_destructuring_initializer_invalid() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "var [[a.x] = ['b']] = [];",
        &["Only an identifier or destructuring pattern is allowed here."],
    );
    let _ = tmp1;
}

// port: ParserTest#testArrayDestructuringDeclarationRest
#[test]
fn test_array_destructuring_declaration_rest() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::ARRAY_DESTRUCTURING, Feature::ARRAY_PATTERN_REST]);
    let tmp1 = h.parse("var [first, ...rest] = foo();");
    let _ = tmp1;
    let tmp2 = h.parse("let [first, ...rest] = foo();");
    let _ = tmp2;
    let tmp3 = h.parse("const [first, ...rest] = foo();");
    let _ = tmp3;
    let tmp4 = h.parse("var [first, {a, b}, ...[re, st, ...{length}]] = foo();");
    let _ = tmp4;
    h.expect_features(&[]);
    let tmp5 = h.parse_error(
        "var [first, ...more = 'default'] = foo();",
        &["A default value cannot be specified after '...'"],
    );
    let _ = tmp5;
    let tmp6 = h.parse_error("var [first, ...more, last] = foo();", &["']' expected"]);
    let _ = tmp6;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp7 = h.parse_warning(
        "var [first, ...rest] = foo();",
        &[
            &requires_language_mode_message(Feature::ARRAY_DESTRUCTURING),
            &requires_language_mode_message(Feature::ARRAY_PATTERN_REST),
        ],
    );
    let _ = tmp7;
}

// port: ParserTest#testObjectDestructuringDeclarationRest
#[test]
fn test_object_destructuring_declaration_rest() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING, Feature::OBJECT_PATTERN_REST]);
    let tmp1 = h.parse("var {first, ...rest} = foo();");
    let _ = tmp1;
    let tmp2 = h.parse("let {first, ...rest} = foo();");
    let _ = tmp2;
    let tmp3 = h.parse("const {first, ...rest} = foo();");
    let _ = tmp3;
    h.expect_features(&[]);
    let tmp4 = h.parse_error(
        "var {first, ...more = 'default'} = foo();",
        &["A default value cannot be specified after '...'"],
    );
    let _ = tmp4;
    let tmp5 = h.parse_error("var {first, ...more, last} = foo();", &["'}' expected"]);
    let _ = tmp5;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    let tmp6 = h.parse_warning(
        "var {first, ...rest} = foo();",
        &[&requires_language_mode_message(
            Feature::OBJECT_PATTERN_REST,
        )],
    );
    let _ = tmp6;
}

// port: ParserTest#testArrayLiteralDeclarationSpread
#[test]
fn test_array_literal_declaration_spread() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::SPREAD_EXPRESSIONS]);
    let tmp1 = h.parse("var o = [first, ...spread];");
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp2 = h.parse_warning(
        "var o = [first, ...spread];",
        &[&requires_language_mode_message(Feature::SPREAD_EXPRESSIONS)],
    );
    let _ = tmp2;
}

// port: ParserTest#testObjectLiteralDeclarationSpread
#[test]
fn test_object_literal_declaration_spread() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::OBJECT_LITERALS_WITH_SPREAD]);
    let tmp1 = h.parse("var o = {first: 1, ...spread};");
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp2 = h.parse_warning(
        "var o = {first: 1, ...spread};",
        &[&requires_language_mode_message(
            Feature::OBJECT_LITERALS_WITH_SPREAD,
        )],
    );
    let _ = tmp2;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    let tmp3 = h.parse_warning(
        "var o = {first: 1, ...spread};",
        &[&requires_language_mode_message(
            Feature::OBJECT_LITERALS_WITH_SPREAD,
        )],
    );
    let _ = tmp3;
}

// port: ParserTest#testArrayDestructuringAssignRest
#[test]
fn test_array_destructuring_assign_rest() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::ARRAY_DESTRUCTURING, Feature::ARRAY_PATTERN_REST]);
    let tmp1 = h.parse("[first, ...rest] = foo();");
    let _ = tmp1;
    let tmp2 = h.parse("[first, {a, b}, ...[re, st, ...{length}]] = foo();");
    let _ = tmp2;
    let tmp3 = h.parse("[x, ...y[15]] = foo();");
    let _ = tmp3;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp4 = h.parse_warning(
        "var [first, ...rest] = foo();",
        &[
            &requires_language_mode_message(Feature::ARRAY_DESTRUCTURING),
            &requires_language_mode_message(Feature::ARRAY_PATTERN_REST),
        ],
    );
    let _ = tmp4;
}

// port: ParserTest#testObjectDestructuringAssignRest
#[test]
fn test_object_destructuring_assign_rest() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING, Feature::OBJECT_PATTERN_REST]);
    let tmp1 = h.parse("const {first, ...rest} = foo();");
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    let tmp2 = h.parse_warning(
        "var {first, ...rest} = foo();",
        &[&requires_language_mode_message(
            Feature::OBJECT_PATTERN_REST,
        )],
    );
    let _ = tmp2;
}

// port: ParserTest#testArrayDestructuringAssignRestInvalid
#[test]
fn test_array_destructuring_assign_rest_invalid() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "var [x, ...y[15]] = foo();",
        &["Only an identifier or destructuring pattern is allowed here."],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "[first, ...more = 'default'] = foo();",
        &["A default value cannot be specified after '...'"],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error("var [first, ...more, last] = foo();", &["']' expected"]);
    let _ = tmp3;
}

// port: ParserTest#testArrayDestructuringFnDeclaration
#[test]
fn test_array_destructuring_fn_declaration() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::ARRAY_DESTRUCTURING]);
    let tmp1 = h.parse("function f([x, y]) { use(x); use(y); }");
    let _ = tmp1;
    let tmp2 = h.parse("function f([x, [y, z]]) {}");
    let _ = tmp2;
    let tmp3 = h.parse("function f([x, {y, foo: z}]) {}");
    let _ = tmp3;
    let tmp4 = h.parse("function f([x, y] = [1, 2]) { use(x); use(y); }");
    let _ = tmp4;
    let tmp5 = h.parse("function f([x1, x2]) {}");
    let _ = tmp5;
}

// port: ParserTest#testArrayDestructuringFnDeclarationInvalid
#[test]
fn test_array_destructuring_fn_declaration_invalid() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "function f([a[0], x]) {}",
        &["Only an identifier or destructuring pattern is allowed here."],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "function f([a, [x.foo]]) {}",
        &["Only an identifier or destructuring pattern is allowed here."],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "function f([a, {foo: x.foo}]) {}",
        &["Only an identifier or destructuring pattern is allowed here."],
    );
    let _ = tmp3;
}

// port: ParserTest#testObjectDestructuringVar
#[test]
fn test_object_destructuring_var() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING]);
    let tmp1 = h.parse("var {x, y} = foo();");
    let _ = tmp1;
    let tmp2 = h.parse("var {x: x, y: y} = foo();");
    let _ = tmp2;
    let tmp3 = h.parse("var {x: {y, z}} = foo();");
    let _ = tmp3;
    let tmp4 = h.parse("var {x: {y: {z}}} = foo();");
    let _ = tmp4;
    let tmp5 = h.parse("var {} = foo();");
    let _ = tmp5;
}

// port: ParserTest#testObjectDestructuringVarInvalid
#[test]
fn test_object_destructuring_var_invalid() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("var {x.a, y} = foo();", &["'}' expected"]);
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "var {a: x.a, y} = foo();",
        &["Only an identifier or destructuring pattern is allowed here."],
    );
    let _ = tmp2;
}

// port: ParserTest#testObjectDestructuringVarWithInitializer
#[test]
fn test_object_destructuring_var_with_initializer() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING, Feature::DEFAULT_PARAMETERS]);
    let tmp1 = h.parse("var {x = 1} = foo();");
    let _ = tmp1;
    let tmp2 = h.parse("var {x: {y = 1}} = foo();");
    let _ = tmp2;
    let tmp3 = h.parse("var {x: y = 1} = foo();");
    let _ = tmp3;
    let tmp4 = h.parse("var {x: v1 = 5, y: v2 = 'str'} = foo();");
    let _ = tmp4;
    let tmp5 = h.parse("var {k1: {k2 : x} = bar(), k3: y} = foo();");
    let _ = tmp5;
}

// port: ParserTest#testObjectDestructuringAssign
#[test]
fn test_object_destructuring_assign() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("({x, y}) = foo();", &["invalid assignment target"]);
    let _ = tmp1;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING]);
    let tmp2 = h.parse("({x, y} = foo());");
    let _ = tmp2;
    let tmp3 = h.parse("({x: x, y: y} = foo());");
    let _ = tmp3;
    let tmp4 = h.parse("({x: {y, z}} = foo());");
    let _ = tmp4;
    let tmp5 = h.parse("({k1: {k2 : x} = bar(), k3: y} = foo());");
    let _ = tmp5;
    let tmp6 = h.parse("({} = foo());");
    let _ = tmp6;
}

// port: ParserTest#testObjectDestructuringAssignWithInitializer
#[test]
fn test_object_destructuring_assign_with_initializer() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("({x = 1}) = foo();", &["invalid assignment target"]);
    let _ = tmp1;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING]);
    let tmp2 = h.parse("({x = 1} = foo());");
    let _ = tmp2;
    let tmp3 = h.parse("({x: {y = 1}} = foo());");
    let _ = tmp3;
    let tmp4 = h.parse("({x: y = 1} = foo());");
    let _ = tmp4;
    let tmp5 = h.parse("({x: v1 = 5, y: v2 = 'str'} = foo());");
    let _ = tmp5;
    let tmp6 = h.parse("({k1: {k2 : x} = bar(), k3: y} = foo());");
    let _ = tmp6;
}

// port: ParserTest#testObjectDestructuringWithInitializerInvalid
#[test]
fn test_object_destructuring_with_initializer_invalid() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("var {{x}} = foo();", &["'}' expected"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("({{x}}) = foo();", &["'}' expected"]);
    let _ = tmp2;
    let tmp3 = h.parse_error("({{a} = {a: 'b'}}) = foo();", &["'}' expected"]);
    let _ = tmp3;
    let tmp4 = h.parse_error("({{a : b} = {a: 'b'}}) = foo();", &["'}' expected"]);
    let _ = tmp4;
}

// port: ParserTest#testObjectDestructuringFnDeclaration
#[test]
fn test_object_destructuring_fn_declaration() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING]);
    let tmp1 = h.parse("function f({x, y}) { use(x); use(y); }");
    let _ = tmp1;
    let tmp2 = h.parse("function f({w, x: {y, z}}) {}");
    let _ = tmp2;
    let tmp3 = h.parse("function f({x, y} = {x:1, y:2}) {}");
    let _ = tmp3;
    let tmp4 = h.parse("function f({x1, x2}) {}");
    let _ = tmp4;
}

// port: ParserTest#testObjectDestructuringFnDeclarationInvalid
#[test]
fn test_object_destructuring_fn_declaration_invalid() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("function f({a[0], x}) {}", &["'}' expected"]);
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "function f({foo: a[0], x}) {}",
        &["Only an identifier or destructuring pattern is allowed here."],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "function f({a, foo: [x.foo]}) {}",
        &["Only an identifier or destructuring pattern is allowed here."],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "function f({a, x: {foo: x.foo}}) {}",
        &["Only an identifier or destructuring pattern is allowed here."],
    );
    let _ = tmp4;
}

// port: ParserTest#testObjectDestructuringComputedProp
#[test]
fn test_object_destructuring_computed_prop() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("var {[x]} = z;", &["':' expected"]);
    let _ = tmp1;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING]);
    let tmp2 = h.parse("var {[x]: y} = z;");
    let _ = tmp2;
    let tmp3 = h.parse("var { [foo()] : [x,y,z] = bar() } = baz();");
    let _ = tmp3;
}

// port: ParserTest#testObjectDestructuringStringAndNumberKeys
#[test]
fn test_object_destructuring_string_and_number_keys() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("var { 'hello world' } = foo();", &["':' expected"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("var { 4 } = foo();", &["':' expected"]);
    let _ = tmp2;
    let tmp3 = h.parse_error("var { 'hello' = 'world' } = foo();", &["':' expected"]);
    let _ = tmp3;
    let tmp4 = h.parse_error("var { 2 = 5 } = foo();", &["':' expected"]);
    let _ = tmp4;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING]);
    let tmp5 = h.parse("var {'s': x} = foo();");
    let _ = tmp5;
    let tmp6 = h.parse("var {3: x} = foo();");
    let _ = tmp6;
}

// port: ParserTest#testObjectNumberKeysSpecial
#[test]
fn test_object_number_keys_special() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var a = {12345678901234567890: 2}");
    let mut n = tmp1;
    let mut object_lit = n
        .get_first_child(&h.ast)
        .unwrap()
        .get_first_first_child(&h.ast);
    assert_eq!(object_lit.unwrap().get_token(&h.ast), Token::OBJECTLIT);
    let mut number = object_lit.unwrap().get_first_child(&h.ast);
    assert_eq!(number.unwrap().get_token(&h.ast), Token::STRING_KEY);
    assert_eq!(
        number.unwrap().get_string(&h.ast),
        JsString::from("12345678901234567000")
    );
}

// port: ParserTest#testObjectDestructuringKeywordKeys
#[test]
fn test_object_destructuring_keyword_keys() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING]);
    let tmp1 = h.parse("var {if: x, else: y} = foo();");
    let _ = tmp1;
    let tmp2 = h.parse("var {while: x=1, for: y} = foo();");
    let _ = tmp2;
    let tmp3 = h.parse("var {type} = foo();");
    let _ = tmp3;
    let tmp4 = h.parse("var {declare} = foo();");
    let _ = tmp4;
    let tmp5 = h.parse("var {module} = foo();");
    let _ = tmp5;
    let tmp6 = h.parse("var {namespace} = foo();");
    let _ = tmp6;
}

// port: ParserTest#testObjectDestructuringKeywordKeysInvalid
#[test]
fn test_object_destructuring_keyword_keys_invalid() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "var {while} = foo();",
        &["cannot use keyword 'while' here."],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "var {implements} = foo();",
        &["cannot use keyword 'implements' here."],
    );
    let _ = tmp2;
}

// port: ParserTest#testObjectDestructuringComplexTarget
#[test]
fn test_object_destructuring_complex_target() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error(
        "var {foo: bar.x} = baz();",
        &["Only an identifier or destructuring pattern is allowed here."],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "var {foo: bar[x]} = baz();",
        &["Only an identifier or destructuring pattern is allowed here."],
    );
    let _ = tmp2;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING]);
    let tmp3 = h.parse("({foo: bar.x} = baz());");
    let _ = tmp3;
    let tmp4 = h.parse("for ({foo: bar.x} in baz());");
    let _ = tmp4;
    let tmp5 = h.parse("({foo: bar[x]} = baz());");
    let _ = tmp5;
    let tmp6 = h.parse("for ({foo: bar[x]} in baz());");
    let _ = tmp6;
}

// port: ParserTest#testObjectDestructuringExtraParens
#[test]
fn test_object_destructuring_extra_parens() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING]);
    let tmp1 = h.parse("({x: y} = z);");
    let _ = tmp1;
    let tmp2 = h.parse("({x: (y)} = z);");
    let _ = tmp2;
    let tmp3 = h.parse("({x: ((y))} = z);");
    let _ = tmp3;
    h.expect_features(&[Feature::ARRAY_DESTRUCTURING]);
    let tmp4 = h.parse("([x] = y);");
    let _ = tmp4;
    let tmp5 = h.parse("[(x), y] = z;");
    let _ = tmp5;
    let tmp6 = h.parse("[x, (y)] = z;");
    let _ = tmp6;
}

// port: ParserTest#testObjectDestructuringExtraParensInvalid
#[test]
fn test_object_destructuring_extra_parens_invalid() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("[x, ([y])] = z;", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp1;
    let tmp2 = h.parse_error("[x, (([y]))] = z;", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp2;
}

// port: ParserTest#testObjectLiteralCannotUseDestructuring
#[test]
fn test_object_literal_cannot_use_destructuring() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error(
        "var o = {x = 5}",
        &["Default value cannot appear at top level of an object literal."],
    );
    let _ = tmp1;
}

// port: ParserTest#testMixedDestructuring
#[test]
fn test_mixed_destructuring() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::ARRAY_DESTRUCTURING, Feature::OBJECT_DESTRUCTURING]);
    let tmp1 = h.parse("var {x: [y, z]} = foo();");
    let _ = tmp1;
    let tmp2 = h.parse("var [x, {y, z}] = foo();");
    let _ = tmp2;
    let tmp3 = h.parse("({x: [y, z]} = foo());");
    let _ = tmp3;
    let tmp4 = h.parse("[x, {y, z}] = foo();");
    let _ = tmp4;
    let tmp5 = h.parse("function f({x: [y, z]}) {}");
    let _ = tmp5;
    let tmp6 = h.parse("function f([x, {y, z}]) {}");
    let _ = tmp6;
}

// port: ParserTest#testMixedDestructuringWithInitializer
#[test]
fn test_mixed_destructuring_with_initializer() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::ARRAY_DESTRUCTURING, Feature::OBJECT_DESTRUCTURING]);
    let tmp1 = h.parse("var {x: [y, z] = [1, 2]} = foo();");
    let _ = tmp1;
    let tmp2 = h.parse("var [x, {y, z} = {y: 3, z: 4}] = foo();");
    let _ = tmp2;
    let tmp3 = h.parse("({x: [y, z] = [1, 2]} = foo());");
    let _ = tmp3;
    let tmp4 = h.parse("[x, {y, z} = {y: 3, z: 4}] = foo();");
    let _ = tmp4;
    let tmp5 = h.parse("function f({x: [y, z] = [1, 2]}) {}");
    let _ = tmp5;
    let tmp6 = h.parse("function f([x, {y, z} = {y: 3, z: 4}]) {}");
    let _ = tmp6;
}

// port: ParserTest#testDestructuringNoRHS
#[test]
fn test_destructuring_no_rhs() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("var {x: y};", &["destructuring must have an initializer"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("let {x: y};", &["destructuring must have an initializer"]);
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "const {x: y};",
        &["const variables must have an initializer"],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error("var {x};", &["destructuring must have an initializer"]);
    let _ = tmp4;
    let tmp5 = h.parse_error("let {x};", &["destructuring must have an initializer"]);
    let _ = tmp5;
    let tmp6 = h.parse_error("const {x};", &["const variables must have an initializer"]);
    let _ = tmp6;
    let tmp7 = h.parse_error("var [x, y];", &["destructuring must have an initializer"]);
    let _ = tmp7;
    let tmp8 = h.parse_error("let [x, y];", &["destructuring must have an initializer"]);
    let _ = tmp8;
    let tmp9 = h.parse_error(
        "const [x, y];",
        &["const variables must have an initializer"],
    );
    let _ = tmp9;
}

// port: ParserTest#testComprehensions
#[test]
fn test_comprehensions() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let mut error = (format!(
        "{}{}",
        "unsupported language feature:", " array/generator comprehensions"
    ))
    .to_string();
    let tmp1 = h.parse_error("[for (x of y) z];", &[&error]);
    let _ = tmp1;
    let tmp2 = h.parse_error("[for ({x,y} of z) x+y];", &[&error]);
    let _ = tmp2;
    let tmp3 = h.parse_error("[for (x of y) if (x<10) z];", &[&error]);
    let _ = tmp3;
    let tmp4 = h.parse_error("[for (a = 5 of v) a];", &["'identifier' expected"]);
    let _ = tmp4;
    let tmp5 = h.parse_error("(for (x of y) z);", &[&error]);
    let _ = tmp5;
    let tmp6 = h.parse_error("(for ({x,y} of z) x+y);", &[&error]);
    let _ = tmp6;
    let tmp7 = h.parse_error("(for (x of y) if (x<10) z);", &[&error]);
    let _ = tmp7;
    let tmp8 = h.parse_error("(for (a = 5 of v) a);", &["'identifier' expected"]);
    let _ = tmp8;
}

// port: ParserTest#testLetForbidden1
#[test]
fn test_let_forbidden1() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT5;
    h.expect_features(&[Feature::LET_DECLARATIONS]);
    let tmp1 = h.parse_warning(
        "let x = 3;",
        &[&requires_language_mode_message(Feature::LET_DECLARATIONS)],
    );
    let _ = tmp1;
}

// port: ParserTest#testLetForbidden2
#[test]
fn test_let_forbidden2() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT5;
    h.expect_features(&[Feature::LET_DECLARATIONS]);
    let tmp1 = h.parse_warning(
        "function f() { let x = 3; };",
        &[&requires_language_mode_message(Feature::LET_DECLARATIONS)],
    );
    let _ = tmp1;
}

// port: ParserTest#testBlockScopedFunctionDeclaration
#[test]
fn test_block_scoped_function_declaration() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::BLOCK_SCOPED_FUNCTION_DECLARATION]);
    let tmp1 = h.parse("{ function foo() {} }");
    let _ = tmp1;
    let tmp2 = h.parse("if (true) { function foo() {} }");
    let _ = tmp2;
    let tmp3 = h.parse("{ function* gen() {} }");
    let _ = tmp3;
    let tmp4 = h.parse("if (true) function foo() {}");
    let _ = tmp4;
    let tmp5 = h.parse("if (true) function foo() {} else {}");
    let _ = tmp5;
    let tmp6 = h.parse("if (true) {} else function foo() {}");
    let _ = tmp6;
    let tmp7 = h.parse("if (true) function foo() {} else function foo() {}");
    let _ = tmp7;
    h.mode = LanguageMode::ECMASCRIPT5;
    h.expect_features(&[]);
    let tmp8 = h.parse("function foo() {}");
    let _ = tmp8;
    let tmp9 = h.parse("(function foo() {})");
    let _ = tmp9;
    let tmp10 = h.parse("function foo() { function bar() {} }");
    let _ = tmp10;
    let tmp11 = h.parse("{ var foo = function() {}; }");
    let _ = tmp11;
    let tmp12 = h.parse("{ var foo = function bar() {}; }");
    let _ = tmp12;
    let tmp13 = h.parse("{ (function() {})(); }");
    let _ = tmp13;
    let tmp14 = h.parse("{ (function foo() {})(); }");
    let _ = tmp14;
    let tmp15 = h.parse_warning(
        "{ function f() {} }",
        &[&requires_language_mode_message(
            Feature::BLOCK_SCOPED_FUNCTION_DECLARATION,
        )],
    );
    let _ = tmp15;
}

// port: ParserTest#testLetForbidden3
#[test]
fn test_let_forbidden3() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp1 = h.parse_error("function f() { var let = 3; }", &["'identifier' expected"]);
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    let tmp2 = h.parse_error("function f() { var let = 3; }", &["'identifier' expected"]);
    let _ = tmp2;
}

// port: ParserTest#testYieldForbidden
#[test]
fn test_yield_forbidden() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "function f() { yield 3; }",
        &["primary expression expected"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "function f(x = yield 3) { return x }",
        &["primary expression expected"],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "function *f(x = yield 3) { return x }",
        &["`yield` is illegal in parameter default value."],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "function *f() { return function*(x = yield 1) { return x; } }",
        &["`yield` is illegal in parameter default value."],
    );
    let _ = tmp4;
}

// port: ParserTest#testGenerator
#[test]
fn test_generator() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::GENERATORS]);
    let tmp1 = h.parse("var obj = { *f() { yield 3; } };");
    let _ = tmp1;
    let tmp2 = h.parse("function* f() { yield 3; }");
    let _ = tmp2;
    let tmp3 = h.parse("function f() { return function* g() {} }");
    let _ = tmp3;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp4 = h.parse_warning(
        "function* f() { yield 3; }",
        &[&requires_language_mode_message(Feature::GENERATORS)],
    );
    let _ = tmp4;
    let tmp5 = h.parse_warning(
        "var obj = { * f() { yield 3; } };",
        &[
            &requires_language_mode_message(Feature::GENERATORS),
            &requires_language_mode_message(Feature::MEMBER_DECLARATIONS),
        ],
    );
    let _ = tmp5;
}

// port: ParserTest#testBracelessFunctionForbidden
#[test]
fn test_braceless_function_forbidden() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("var sq = function(x) x * x;", &["'{' expected"]);
    let _ = tmp1;
}

// port: ParserTest#testGeneratorsForbidden
#[test]
fn test_generators_forbidden() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("var i = (x for (x in obj));", &["')' expected"]);
    let _ = tmp1;
}

// port: ParserTest#testGettersForbidden1
#[test]
fn test_getters_forbidden1() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    h.expect_features(&[Feature::GETTER]);
    let tmp1 = h.parse_error(
        "var x = {get foo() { return 3; }};",
        &[closure_parsing::ir_factory::GETTER_ERROR_MESSAGE],
    );
    let _ = tmp1;
}

// port: ParserTest#testGettersForbidden2
#[test]
fn test_getters_forbidden2() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    let tmp1 = h.parse_error("var x = {get foo bar() { return 3; }};", &["'(' expected"]);
    let _ = tmp1;
}

// port: ParserTest#testGettersForbidden3
#[test]
fn test_getters_forbidden3() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    let tmp1 = h.parse_error(
        "var x = {a getter:function b() { return 3; }};",
        &["'}' expected"],
    );
    let _ = tmp1;
}

// port: ParserTest#testGettersForbidden4
#[test]
fn test_getters_forbidden4() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    let tmp1 = h.parse_error(
        "var x = {\"a\" getter:function b() { return 3; }};",
        &["':' expected"],
    );
    let _ = tmp1;
}

// port: ParserTest#testGettersForbidden5
#[test]
fn test_getters_forbidden5() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    h.expect_features(&[Feature::GETTER]);
    let tmp1 = h.parse_error(
        "var x = {a: 2, get foo() { return 3; }};",
        &[closure_parsing::ir_factory::GETTER_ERROR_MESSAGE],
    );
    let _ = tmp1;
}

// port: ParserTest#testGettersForbidden6
#[test]
fn test_getters_forbidden6() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    h.expect_features(&[Feature::GETTER]);
    let tmp1 = h.parse_error(
        "var x = {get 'foo'() { return 3; }};",
        &[closure_parsing::ir_factory::GETTER_ERROR_MESSAGE],
    );
    let _ = tmp1;
}

// port: ParserTest#testSettersForbidden
#[test]
fn test_setters_forbidden() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    h.expect_features(&[Feature::SETTER]);
    let tmp1 = h.parse_error(
        "var x = {set foo(a) { y = 3; }};",
        &[closure_parsing::ir_factory::SETTER_ERROR_MESSAGE],
    );
    let _ = tmp1;
}

// port: ParserTest#testSettersForbidden2
#[test]
fn test_setters_forbidden2() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    let tmp1 = h.parse_error(
        "var x = {a setter:function b() { return 3; }};",
        &["'}' expected"],
    );
    let _ = tmp1;
}

// port: ParserTest#testFileOverviewJSDoc1
#[test]
fn test_file_overview_js_doc1() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    let tmp1 = h.parse("/** @fileoverview Hi mom! */ function Foo() {}");
    let mut n = tmp1;
    assert_eq!(
        n.get_first_child(&h.ast).unwrap().get_token(&h.ast),
        Token::FUNCTION
    );
    assert!(n.get_jsdoc_info(&h.ast).is_some());
    assert!(
        n.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_none()
    );
    assert_eq!(
        n.get_jsdoc_info(&h.ast)
            .as_ref()
            .unwrap()
            .get_file_overview(),
        Some(JsString::from("Hi mom!"))
    );
}

// port: ParserTest#testFileOverviewJSDoc_notOnTopOfFile
#[test]
fn test_file_overview_js_doc_not_on_top_of_file() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    let tmp1 = h.parse("// some comment \n let x; /** @fileoverview Hi mom! */ class Foo {}");
    let mut n = tmp1;
    assert_eq!(n.get_token(&h.ast), Token::SCRIPT);
    assert!(n.get_jsdoc_info(&h.ast).is_some());
    assert_eq!(
        n.get_jsdoc_info(&h.ast)
            .as_ref()
            .unwrap()
            .get_file_overview(),
        Some(JsString::from("Hi mom!"))
    );
    let mut let_node = n.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    assert!(let_node.unwrap().get_jsdoc_info(&h.ast).is_none());
    let mut class_node = n.get_second_child(&h.ast);
    assert_eq!(class_node.unwrap().get_token(&h.ast), Token::CLASS);
    assert!(class_node.unwrap().get_jsdoc_info(&h.ast).is_none());
}

// port: ParserTest#testFileOverviewJSDocDoesNotHoseParsing
#[test]
fn test_file_overview_js_doc_does_not_hose_parsing() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/** @fileoverview Hi mom! \n */ function Foo() {}");
    assert_eq!(
        tmp1.get_first_child(&h.ast).unwrap().get_token(&h.ast),
        Token::FUNCTION
    );
    let tmp2 = h.parse("/** @fileoverview Hi mom! \n * * * */ function Foo() {}");
    assert_eq!(
        tmp2.get_first_child(&h.ast).unwrap().get_token(&h.ast),
        Token::FUNCTION
    );
    let tmp3 = h.parse("/** @fileoverview \n * x */ function Foo() {}");
    assert_eq!(
        tmp3.get_first_child(&h.ast).unwrap().get_token(&h.ast),
        Token::FUNCTION
    );
    let tmp4 = h.parse("/** @fileoverview \n * x \n */ function Foo() {}");
    assert_eq!(
        tmp4.get_first_child(&h.ast).unwrap().get_token(&h.ast),
        Token::FUNCTION
    );
}

// port: ParserTest#testFileOverviewJSDoc2
#[test]
fn test_file_overview_js_doc2() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    let tmp1 = h.parse("/** @fileoverview Hi mom! */\n /** @constructor */ function Foo() {}\n");
    let mut n = tmp1;
    assert!(n.get_jsdoc_info(&h.ast).is_some());
    assert_eq!(
        n.get_jsdoc_info(&h.ast)
            .as_ref()
            .unwrap()
            .get_file_overview(),
        Some(JsString::from("Hi mom!"))
    );
    assert!(
        n.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
    assert!(
        !n.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .as_ref()
            .unwrap()
            .has_file_overview()
    );
    assert!(
        n.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .as_ref()
            .unwrap()
            .is_constructor()
    );
}

// port: ParserTest#testFileoverview_firstOneWins
#[test]
fn test_fileoverview_first_one_wins() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    let tmp1 = h.parse("/** @fileoverview First */\n/** @fileoverview Second */\n");
    let mut n = tmp1;
    assert!(n.get_jsdoc_info(&h.ast).is_some());
    assert_eq!(
        n.get_jsdoc_info(&h.ast)
            .as_ref()
            .unwrap()
            .get_file_overview(),
        Some(JsString::from("First"))
    );
}

// port: ParserTest#testFileoverview_firstOneWins_implicitFileoverview
#[test]
fn test_fileoverview_first_one_wins_implicit_fileoverview() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    let tmp1 = h.parse("/** @typeSummary */\n/** @fileoverview Second */\n");
    let mut n = tmp1;
    assert!(n.get_jsdoc_info(&h.ast).is_some());
    assert!(n.get_jsdoc_info(&h.ast).as_ref().unwrap().is_type_summary());
    assert!(
        n.get_jsdoc_info(&h.ast)
            .as_ref()
            .unwrap()
            .get_file_overview()
            .is_none()
    );
}

// port: ParserTest#testFileoverview_firstOneWins_suppressionsAccumulate
#[test]
fn test_fileoverview_first_one_wins_suppressions_accumulate() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    let tmp1 = h.parse(
        "/** @fileoverview @suppress {const} */\n/** @fileoverview @suppress {checkTypes} */\n",
    );
    let mut n = tmp1;
    assert!(n.get_jsdoc_info(&h.ast).is_some());
    assert_eq!(
        n.get_jsdoc_info(&h.ast)
            .as_ref()
            .unwrap()
            .get_suppressions(),
        [JsString::from("const"), JsString::from("checkTypes")]
            .into_iter()
            .collect::<closure_rhino::fx_hash::IndexSet<_>>()
    );
}

// port: ParserTest#testFileoverview_firstOneWins_externsAccumulate
#[test]
fn test_fileoverview_first_one_wins_externs_accumulate() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    let tmp1 = h.parse("/** @fileoverview First */\n/** @externs */\n");
    let mut n = tmp1;
    assert!(n.get_jsdoc_info(&h.ast).is_some());
    assert_eq!(
        n.get_jsdoc_info(&h.ast)
            .as_ref()
            .unwrap()
            .get_file_overview(),
        Some(JsString::from("First"))
    );
    assert!(n.get_jsdoc_info(&h.ast).as_ref().unwrap().is_externs());
}

// port: ParserTest#testFileoverview_typeSummaryAccumulates_withExterns
#[test]
fn test_fileoverview_type_summary_accumulates_with_externs() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    let tmp1 = h.parse("/** @fileoverview @typeSummary */\n/** @externs */\n;\n");
    let mut n = tmp1;
    assert!(n.get_jsdoc_info(&h.ast).is_some());
    assert!(n.get_jsdoc_info(&h.ast).as_ref().unwrap().is_externs());
    assert!(n.get_jsdoc_info(&h.ast).as_ref().unwrap().is_type_summary());
}

// port: ParserTest#testImportantComment
#[test]
fn test_important_comment() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    let tmp1 = h.parse("/*! Hi mom! */ function Foo() {}");
    let mut n = tmp1;
    assert_eq!(
        n.get_first_child(&h.ast).unwrap().get_token(&h.ast),
        Token::FUNCTION
    );
    assert!(n.get_jsdoc_info(&h.ast).is_some());
    assert!(
        n.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_none()
    );
    assert_eq!(
        n.get_jsdoc_info(&h.ast).as_ref().unwrap().get_license(),
        Some(JsString::from(" Hi mom! "))
    );
}

// port: ParserTest#testBlockCommentParsed
#[test]
fn test_block_comment_parsed() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("/* Hi mom! */ \n function Foo() {}");
    let mut n = tmp1;
    assert_eq!(
        n.get_first_child(&h.ast).unwrap().get_token(&h.ast),
        Token::FUNCTION
    );
    assert_eq!(
        n.get_first_child(&h.ast)
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast),
        JsString::from("/* Hi mom! */")
    );
    assert!(
        !n.get_first_child(&h.ast)
            .unwrap()
            .get_non_jsdoc_comment(&h.ast)
            .as_ref()
            .unwrap()
            .is_inline()
    );
}

// port: ParserTest#testBlockCommentNotParsedWithoutParsingMode
#[test]
fn test_block_comment_not_parsed_without_parsing_mode() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/* Hi mom! */ \n function Foo() {}");
    let mut n = tmp1;
    assert_eq!(
        n.get_first_child(&h.ast).unwrap().get_token(&h.ast),
        Token::FUNCTION
    );
    assert!(
        n.get_first_child(&h.ast)
            .unwrap()
            .get_non_jsdoc_comment(&h.ast)
            .is_none()
    );
}

// port: ParserTest#testLineCommentParsed
#[test]
fn test_line_comment_parsed() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("// Hi mom! \n function Foo() {}");
    let mut n = tmp1;
    assert_eq!(
        n.get_first_child(&h.ast).unwrap().get_token(&h.ast),
        Token::FUNCTION
    );
    assert_eq!(
        n.get_first_child(&h.ast)
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast),
        JsString::from("// Hi mom!")
    );
}

// port: ParserTest#testManyIndividualCommentsGetAttached
#[test]
fn test_many_individual_comments_get_attached() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("// Hi Mom! \n function Foo() {} \n // Hi Dad! \n  function Bar() {}");
    let mut n = tmp1;
    assert_eq!(
        n.get_first_child(&h.ast).unwrap().get_token(&h.ast),
        Token::FUNCTION
    );
    assert_eq!(
        n.get_first_child(&h.ast)
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast),
        JsString::from("// Hi Mom!")
    );
    assert_eq!(
        n.get_second_child(&h.ast).unwrap().get_token(&h.ast),
        Token::FUNCTION
    );
    assert_eq!(
        n.get_second_child(&h.ast)
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast),
        JsString::from("// Hi Dad!")
    );
}

// port: ParserTest#testIndividualCommentsAroundClasses
#[test]
fn test_individual_comments_around_classes() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("// comment A \n class A{} // trailing a \n // comment B \n  class Bar{}");
    let mut n = tmp1;
    let mut class_a = n.get_first_child(&h.ast);
    let mut class_b = n.get_second_child(&h.ast);
    assert_eq!(class_a.unwrap().get_token(&h.ast), Token::CLASS);
    assert_eq!(
        class_a.unwrap().get_non_jsdoc_comment_string(&h.ast),
        JsString::from("// comment A")
    );
    assert_eq!(
        class_a
            .unwrap()
            .get_trailing_non_jsdoc_comment_string(&h.ast),
        JsString::from("// trailing a")
    );
    assert_eq!(class_b.unwrap().get_token(&h.ast), Token::CLASS);
    assert_eq!(
        class_b.unwrap().get_non_jsdoc_comment_string(&h.ast),
        JsString::from("// comment B")
    );
}

// port: ParserTest#testMultipleLinedCommentsAttachedToSameNode
#[test]
fn test_multiple_lined_comments_attached_to_same_node() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("// Hi Mom! \n // And Dad! \n  function Foo() {} ");
    let mut n = tmp1;
    assert_eq!(
        n.get_first_child(&h.ast).unwrap().get_token(&h.ast),
        Token::FUNCTION
    );
    assert_eq!(
        n.get_first_child(&h.ast)
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast),
        JsString::from("// Hi Mom!\n// And Dad!")
    );
}

// port: ParserTest#testEmptyLinesBetweenCommentsIsPreserved
#[test]
fn test_empty_lines_between_comments_is_preserved() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("/* Hi Mom!*/ \n\n\n // And Dad! \n  function Foo() {} ");
    let mut n = tmp1;
    assert_eq!(
        n.get_first_child(&h.ast).unwrap().get_token(&h.ast),
        Token::FUNCTION
    );
    assert_eq!(
        n.get_first_child(&h.ast)
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast),
        JsString::from("/* Hi Mom!*/\n\n\n// And Dad!")
    );
}

// port: ParserTest#testObjectLiteralDoc1
#[test]
fn test_object_literal_doc1() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var x = {/** @type {number} */ 1: 2};");
    let mut n = tmp1;
    let mut object_lit = n
        .get_first_first_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(object_lit.unwrap().get_token(&h.ast), Token::OBJECTLIT);
    let mut number = object_lit.unwrap().get_first_child(&h.ast);
    assert_eq!(number.unwrap().get_token(&h.ast), Token::STRING_KEY);
    assert!(number.unwrap().get_jsdoc_info(&h.ast).is_some());
}

// port: ParserTest#testDuplicatedParam
#[test]
fn test_duplicated_param() {
    let mut h = Harness::new();
    let tmp1 = h.parse_warning("function foo(x, x) {}", &["Duplicate parameter name \"x\""]);
    let _ = tmp1;
    let tmp2 = h.parse_warning(
        "function foo(x, n, x) {}",
        &["Duplicate parameter name \"x\""],
    );
    let _ = tmp2;
    let tmp3 = h.parse_warning(
        "function foo(n, x, x) {}",
        &["Duplicate parameter name \"x\""],
    );
    let _ = tmp3;
    let tmp4 = h.parse_warning(
        "function foo(x, {x}) {}",
        &["Duplicate parameter name \"x\""],
    );
    let _ = tmp4;
    let tmp5 = h.parse_warning(
        "function foo(x, {n: {x}}) {}",
        &["Duplicate parameter name \"x\""],
    );
    let _ = tmp5;
    let tmp6 = h.parse_warning(
        "function foo(x, [x]) {}",
        &["Duplicate parameter name \"x\""],
    );
    let _ = tmp6;
    let tmp7 = h.parse_warning(
        "function foo(x, ...x) {}",
        &["Duplicate parameter name \"x\""],
    );
    let _ = tmp7;
    let tmp8 = h.parse_warning(
        "function foo(...[x, x]) {}",
        &["Duplicate parameter name \"x\""],
    );
    let _ = tmp8;
    let tmp9 = h.parse_warning(
        "function foo(...[x,,x]) {}",
        &["Duplicate parameter name \"x\""],
    );
    let _ = tmp9;
    let tmp10 = h.parse_warning(
        "function foo(x, x = 1) {}",
        &["Duplicate parameter name \"x\""],
    );
    let _ = tmp10;
    let tmp11 = h.parse_warning(
        "function foo([x, x] = [1, 2]) {}",
        &["Duplicate parameter name \"x\""],
    );
    let _ = tmp11;
    let tmp12 = h.parse_warning(
        "function foo({x, x} = {x: 1}) {}",
        &["Duplicate parameter name \"x\""],
    );
    let _ = tmp12;
    let tmp13 = h.parse_warning(
        "function foo(x, {[n()]: x}) {}",
        &["Duplicate parameter name \"x\""],
    );
    let _ = tmp13;
    let tmp14 = h.parse_warning("function foo(x = x) {}", &[]);
    let _ = tmp14;
    let tmp15 = h.parse_warning("function foo({x: x}) {}", &[]);
    let _ = tmp15;
    let tmp16 = h.parse_warning("function foo(foo) {}", &[]);
    let _ = tmp16;
}

// port: ParserTest#testLetAsIdentifier
#[test]
fn test_let_as_identifier() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("var let");
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp2 = h.parse("var let");
    let _ = tmp2;
    h.mode = LanguageMode::ECMASCRIPT5;
    h.strict_mode = StrictMode::STRICT;
    let tmp3 = h.parse_error("var let", &["'identifier' expected"]);
    let _ = tmp3;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp4 = h.parse("var let");
    let _ = tmp4;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    h.strict_mode = StrictMode::STRICT;
    let tmp5 = h.parse_error("var let", &["'identifier' expected"]);
    let _ = tmp5;
}

// port: ParserTest#testLet
#[test]
fn test_let() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::LET_DECLARATIONS]);
    let tmp1 = h.parse("let x;");
    let _ = tmp1;
    let tmp2 = h.parse("let x = 1;");
    let _ = tmp2;
    let tmp3 = h.parse("let x, y = 2;");
    let _ = tmp3;
    let tmp4 = h.parse("let x = 1, y = 2;");
    let _ = tmp4;
}

// port: ParserTest#testConst
#[test]
fn test_const() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("const x;", &["const variables must have an initializer"]);
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "const x, y = 2;",
        &["const variables must have an initializer"],
    );
    let _ = tmp2;
    h.expect_features(&[Feature::CONST_DECLARATIONS]);
    let tmp3 = h.parse("const x = 1;");
    let _ = tmp3;
    let tmp4 = h.parse("const x = 1, y = 2;");
    let _ = tmp4;
}

// port: ParserTest#testYield1
#[test]
fn test_yield1() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("var yield");
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp2 = h.parse("var yield");
    let _ = tmp2;
    h.mode = LanguageMode::ECMASCRIPT5;
    h.strict_mode = StrictMode::STRICT;
    let tmp3 = h.parse_error("var yield", &["'identifier' expected"]);
    let _ = tmp3;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp4 = h.parse("var yield");
    let _ = tmp4;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    h.strict_mode = StrictMode::STRICT;
    let tmp5 = h.parse_error("var yield", &["'identifier' expected"]);
    let _ = tmp5;
}

// port: ParserTest#testYield2
#[test]
fn test_yield2() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::GENERATORS]);
    let tmp1 = h.parse("function * f() { yield; }");
    let _ = tmp1;
    let tmp2 = h.parse("function * f() { yield /a/i; }");
    let _ = tmp2;
    h.expect_features(&[]);
    let tmp3 = h.parse_error(
        "function * f() { 1 + yield; }",
        &["primary expression expected"],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "function * f() { 1 + yield 2; }",
        &["primary expression expected"],
    );
    let _ = tmp4;
    let tmp5 = h.parse_error(
        "function * f() { yield 1 + yield 2; }",
        &["primary expression expected"],
    );
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "function * f() { yield(1) + yield(2); }",
        &["primary expression expected"],
    );
    let _ = tmp6;
    h.expect_features(&[Feature::GENERATORS]);
    let tmp7 = h.parse("function * f() { (yield 1) + (yield 2); }");
    let _ = tmp7;
    let tmp8 = h.parse("function * f() { yield * yield; }");
    let _ = tmp8;
    h.expect_features(&[]);
    let tmp9 = h.parse_error(
        "function * f() { yield + yield; }",
        &["primary expression expected"],
    );
    let _ = tmp9;
    h.expect_features(&[Feature::GENERATORS]);
    let tmp10 = h.parse("function * f() { (yield) + (yield); }");
    let _ = tmp10;
    let tmp11 = h.parse("function * f() { return yield; }");
    let _ = tmp11;
    let tmp12 = h.parse("function * f() { return yield 1; }");
    let _ = tmp12;
    let tmp13 =
        h.parse("function * f() {\n  yield * // line break allowed here\n      [1, 2, 3];\n}\n");
    let _ = tmp13;
    h.expect_features(&[]);
    let tmp14 = h.parse_error(
        "function * f() {\n  yield // line break not allowed here\n      *[1, 2, 3];\n}\n",
        &["'}' expected"],
    );
    let _ = tmp14;
    let tmp15 = h.parse_error(
        "function * f() { yield *; }",
        &["yield* requires an expression"],
    );
    let _ = tmp15;
}

// port: ParserTest#testYield3
#[test]
fn test_yield3() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::GENERATORS]);
    let tmp1 = h.parse("function * f() { yield , yield; }");
    let _ = tmp1;
}

// port: ParserTest#testStringLineContinuationWarningsByMode
#[test]
fn test_string_line_continuation_warnings_by_mode() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::STRING_CONTINUATION]);
    h.strict_mode = StrictMode::SLOPPY;
    h.mode = LanguageMode::ECMASCRIPT3;
    let tmp1 = h.parse_warning(
        "'one\\\ntwo';",
        &[
            &requires_language_mode_message(Feature::STRING_CONTINUATION),
            STRING_CONTINUATIONS_WARNING,
        ],
    );
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp2 = h.parse_warning("'one\\\ntwo';", &[STRING_CONTINUATIONS_WARNING]);
    let _ = tmp2;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    let tmp3 = h.parse_warning("'one\\\ntwo';", &[STRING_CONTINUATIONS_WARNING]);
    let _ = tmp3;
}

// port: ParserTest#testStringLineContinuationNormalization
#[test]
fn test_string_line_continuation_normalization() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::STRING_CONTINUATION]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_warning("'one\\\ntwo';", &[STRING_CONTINUATIONS_WARNING]);
    let mut n = tmp1;
    assert_eq!(
        n.get_first_first_child(&h.ast).unwrap().get_string(&h.ast),
        JsString::from("onetwo")
    );
    let tmp2 = h.parse_warning("'one\\\rtwo';", &[STRING_CONTINUATIONS_WARNING]);
    n = tmp2;
    assert_eq!(
        n.get_first_first_child(&h.ast).unwrap().get_string(&h.ast),
        JsString::from("onetwo")
    );
    let tmp3 = h.parse_warning("'one\\\r\ntwo';", &[STRING_CONTINUATIONS_WARNING]);
    n = tmp3;
    assert_eq!(
        n.get_first_first_child(&h.ast).unwrap().get_string(&h.ast),
        JsString::from("onetwo")
    );
    let tmp4 = h.parse_warning("'one \\\ntwo';", &[STRING_CONTINUATIONS_WARNING]);
    n = tmp4;
    assert_eq!(
        n.get_first_first_child(&h.ast).unwrap().get_string(&h.ast),
        JsString::from("one two")
    );
    let tmp5 = h.parse_warning("'one\\\n two';", &[STRING_CONTINUATIONS_WARNING]);
    n = tmp5;
    assert_eq!(
        n.get_first_first_child(&h.ast).unwrap().get_string(&h.ast),
        JsString::from("one two")
    );
}

// port: ParserTest#testStringContinuationIssue3492
#[test]
fn test_string_continuation_issue3492() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::STRING_CONTINUATION]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_warning(
        "function x() {\n        a = \"\\\n        \\ \\\n        \";\n};\n",
        &[
            "Unnecessary escape: '\\ ' is equivalent to just ' '",
            STRING_CONTINUATIONS_WARNING,
        ],
    );
    let _ = tmp1;
}

// port: ParserTest#testStringLiteral
#[test]
fn test_string_literal() {
    let mut h = Harness::new();
    let tmp1 = h.parse("'foo'");
    let mut n = tmp1;
    let mut string_node = n.get_first_first_child(&h.ast);
    assert_eq!(string_node.unwrap().get_token(&h.ast), Token::STRINGLIT);
    assert_eq!(
        string_node.unwrap().get_string(&h.ast),
        JsString::from("foo")
    );
}

// port: ParserTest#testUseTemplateLiteral
#[test]
fn test_use_template_literal() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::TEMPLATE_LITERALS]);
    let tmp1 = h.test_template_literal("f`hello world`;");
    let _ = tmp1;
    let tmp2 = h.test_template_literal("`hello ${name} ${world}`.length;");
    let _ = tmp2;
}

// port: ParserTest#testTemplateLiterals
#[test]
fn test_template_literals() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::TEMPLATE_LITERALS]);
    let tmp1 = h.test_template_literal("``");
    let _ = tmp1;
    let tmp2 = h.test_template_literal("`\"`");
    let _ = tmp2;
    let tmp3 = h.test_template_literal("`\\``");
    let _ = tmp3;
    let tmp4 = h.test_template_literal("`hello world`;");
    let _ = tmp4;
    let tmp5 = h.test_template_literal("`hello\nworld`;");
    let _ = tmp5;
    let tmp6 = h.test_template_literal("`string containing \\`escaped\\` backticks`;");
    let _ = tmp6;
    let tmp7 = h.test_template_literal("{ `in block` }");
    let _ = tmp7;
    let tmp8 = h.test_template_literal("{ `in ${block}` }");
    let _ = tmp8;
}

// port: ParserTest#testEscapedTemplateLiteral
#[test]
fn test_escaped_template_literal() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::TEMPLATE_LITERALS]);
    h.assert_simple_template_literal("${escaped}", "`\\${escaped}`");
}

// port: ParserTest#testTemplateLiteralWithNulChar
#[test]
fn test_template_literal_with_nul_char() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::TEMPLATE_LITERALS]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("var test = `\nhello\\0`");
    let _ = tmp1;
}

// port: ParserTest#testTemplateLiteralWithNewline
#[test]
fn test_template_literal_with_newline() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::TEMPLATE_LITERALS]);
    h.assert_simple_template_literal("hello\nworld", "`hello\nworld`");
    h.assert_simple_template_literal("\n", "`\r`");
    h.assert_simple_template_literal("\n", "`\r\n`");
    h.assert_simple_template_literal("\\\n", "`\\\\\n`");
    h.assert_simple_template_literal("\\\n", "`\\\\\r\n`");
    h.assert_simple_template_literal("\r\n", "`\\r\\n`");
    h.assert_simple_template_literal("\\r\\n", "`\\\\r\\\\n`");
}

// port: ParserTest#testTemplateLiteralWithLineContinuation
#[test]
fn test_template_literal_with_line_continuation() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::TEMPLATE_LITERALS]);
    let tmp1 = h.parse_warning("`string \\\ncontinuation`", &[STRING_CONTINUATIONS_WARNING]);
    let mut n = tmp1;
    let mut template_literal = n.get_first_first_child(&h.ast);
    let mut string_node = template_literal.unwrap().get_first_child(&h.ast);
    assert_eq!(
        string_node.unwrap().get_token(&h.ast),
        Token::TEMPLATELIT_STRING
    );
    assert_eq!(
        string_node.unwrap().get_cooked_string(&h.ast),
        Some(JsString::from("string continuation"))
    );
}

// port: ParserTest#testTemplateLiteralSubstitution
#[test]
fn test_template_literal_substitution() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::TEMPLATE_LITERALS]);
    let tmp1 = h.parse("`hello ${name}`;");
    let _ = tmp1;
    let tmp2 = h.parse("`hello ${name} ${world}`;");
    let _ = tmp2;
    let tmp3 = h.parse("`hello ${name }`");
    let _ = tmp3;
    h.expect_features(&[]);
    let tmp4 = h.parse_error(
        "`hello ${name",
        &["Expected '}' after expression in template literal"],
    );
    let _ = tmp4;
    let tmp5 = h.parse_error(
        "`hello ${name tail}",
        &["Expected '}' after expression in template literal"],
    );
    let _ = tmp5;
}

// port: ParserTest#testUnterminatedTemplateLiteral
#[test]
fn test_unterminated_template_literal() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("`hello", &["Unterminated template literal"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("`hello\\`", &["Unterminated template literal"]);
    let _ = tmp2;
}

// port: ParserTest#testTemplateLiteralOctalEscapes
#[test]
fn test_template_literal_octal_escapes() {
    let mut h = Harness::new();
    h.assert_simple_template_literal("\u{0000}", "`\\0`");
    h.assert_simple_template_literal("aaa\u{0000}aaa", "`aaa\\0aaa`");
}

// port: ParserTest#testIncorrectEscapeSequenceInTemplateLiteral
#[test]
fn test_incorrect_escape_sequence_in_template_literal() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("`hello\\x`", &["Hex digit expected"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("`hello\\1`", &["Invalid escape sequence"]);
    let _ = tmp2;
    let tmp3 = h.parse_error("`hello\\2`", &["Invalid escape sequence"]);
    let _ = tmp3;
    let tmp4 = h.parse_error("`hello\\3`", &["Invalid escape sequence"]);
    let _ = tmp4;
    let tmp5 = h.parse_error("`hello\\4`", &["Invalid escape sequence"]);
    let _ = tmp5;
    let tmp6 = h.parse_error("`hello\\5`", &["Invalid escape sequence"]);
    let _ = tmp6;
    let tmp7 = h.parse_error("`hello\\6`", &["Invalid escape sequence"]);
    let _ = tmp7;
    let tmp8 = h.parse_error("`hello\\7`", &["Invalid escape sequence"]);
    let _ = tmp8;
    let tmp9 = h.parse_error("`hello\\8`", &["Invalid escape sequence"]);
    let _ = tmp9;
    let tmp10 = h.parse_error("`hello\\9`", &["Invalid escape sequence"]);
    let _ = tmp10;
    let tmp11 = h.parse_error("`hello\\00`", &["Invalid escape sequence"]);
    let _ = tmp11;
    let tmp12 = h.parse_error("`hello\\01`", &["Invalid escape sequence"]);
    let _ = tmp12;
    let tmp13 = h.parse_error("`hello\\02`", &["Invalid escape sequence"]);
    let _ = tmp13;
    let tmp14 = h.parse_error("`hello\\03`", &["Invalid escape sequence"]);
    let _ = tmp14;
    let tmp15 = h.parse_error("`hello\\04`", &["Invalid escape sequence"]);
    let _ = tmp15;
    let tmp16 = h.parse_error("`hello\\05`", &["Invalid escape sequence"]);
    let _ = tmp16;
    let tmp17 = h.parse_error("`hello\\06`", &["Invalid escape sequence"]);
    let _ = tmp17;
    let tmp18 = h.parse_error("`hello\\07`", &["Invalid escape sequence"]);
    let _ = tmp18;
    let tmp19 = h.parse_error("`hello\\08`", &["Invalid escape sequence"]);
    let _ = tmp19;
    let tmp20 = h.parse_error("`hello\\09`", &["Invalid escape sequence"]);
    let _ = tmp20;
    let tmp21 = h.parse_error("`\n\\1`", &["Invalid escape sequence"]);
    let _ = tmp21;
    let tmp22 = h.parse_error("`\n\\1 ${0}`", &["Invalid escape sequence"]);
    let _ = tmp22;
}

// port: ParserTest#testTemplateLiteralSubstitutionWithCast
#[test]
fn test_template_literal_substitution_with_cast() {
    let mut h = Harness::new();
    let tmp1 = h.parse("`${ /** @type {?} */ (3)}`");
    let mut root = tmp1;
    let mut expr_result = root.get_first_child(&h.ast);
    let mut template_literal = expr_result.unwrap().get_first_child(&h.ast);
    assert_eq!(
        template_literal.unwrap().get_token(&h.ast),
        Token::TEMPLATELIT
    );
    let mut substitution = template_literal.unwrap().get_second_child(&h.ast);
    assert_eq!(
        substitution.unwrap().get_token(&h.ast),
        Token::TEMPLATELIT_SUB
    );
    let mut cast = substitution.unwrap().get_first_child(&h.ast);
    assert_eq!(cast.unwrap().get_token(&h.ast), Token::CAST);
    let mut number = cast.unwrap().get_first_child(&h.ast);
    assert_eq!(number.unwrap().get_token(&h.ast), Token::NUMBER);
}

// port: ParserTest#testExponentialLiterals
#[test]
fn test_exponential_literals() {
    let mut h = Harness::new();
    let tmp1 = h.parse("0e0");
    let _ = tmp1;
    let tmp2 = h.parse("0E0");
    let _ = tmp2;
    let tmp3 = h.parse("0E1");
    let _ = tmp3;
    let tmp4 = h.parse("1E0");
    let _ = tmp4;
    let tmp5 = h.parse("1E-0");
    let _ = tmp5;
    let tmp6 = h.parse("10E10");
    let _ = tmp6;
    let tmp7 = h.parse("10E-10");
    let _ = tmp7;
    let tmp8 = h.parse("1.0E1");
    let _ = tmp8;
    let tmp9 = h.parse_error("01E0", &[SEMICOLON_EXPECTED]);
    let _ = tmp9;
    let tmp10 = h.parse_error("0E", &["Exponent part must contain at least one digit"]);
    let _ = tmp10;
    let tmp11 = h.parse_error("1E-", &["Exponent part must contain at least one digit"]);
    let _ = tmp11;
    let tmp12 = h.parse_error("1E1.1", &[SEMICOLON_EXPECTED]);
    let _ = tmp12;
}

// port: ParserTest#testBigIntLiteralZero
#[test]
fn test_big_int_literal_zero() {
    let mut h = Harness::new();
    let tmp1 = h.parse("0n;");
    let mut bigint = tmp1.get_only_child(&h.ast).get_only_child(&h.ast);
    assert_eq!(bigint.get_token(&h.ast), Token::BIGINT);
    assert_eq!(bigint.get_lineno(&h.ast), 1_i32);
    assert_eq!(bigint.get_charno(&h.ast), 0_i32);
    assert_eq!(bigint.get_length(&h.ast), 2_i32);
    assert!(bigint.is_big_int(&h.ast));
    assert_eq!(*bigint.get_big_int(&h.ast), BigInt::from(0));
}

// port: ParserTest#testBigIntLiteralPositive
#[test]
fn test_big_int_literal_positive() {
    let mut h = Harness::new();
    let tmp1 = h.parse("1n;");
    let mut bigint = tmp1.get_only_child(&h.ast).get_only_child(&h.ast);
    assert_eq!(bigint.get_token(&h.ast), Token::BIGINT);
    assert_eq!(bigint.get_lineno(&h.ast), 1_i32);
    assert_eq!(bigint.get_charno(&h.ast), 0_i32);
    assert_eq!(bigint.get_length(&h.ast), 2_i32);
    assert!(bigint.is_big_int(&h.ast));
    assert_eq!(*bigint.get_big_int(&h.ast), BigInt::from(1));
}

// port: ParserTest#testBigIntLiteralNegative
#[test]
fn test_big_int_literal_negative() {
    let mut h = Harness::new();
    let tmp1 = h.parse("-1n;");
    let mut neg = tmp1.get_only_child(&h.ast).get_only_child(&h.ast);
    assert_eq!(neg.get_token(&h.ast), Token::NEG);
    assert_eq!(neg.get_lineno(&h.ast), 1_i32);
    assert_eq!(neg.get_charno(&h.ast), 0_i32);
    assert_eq!(neg.get_length(&h.ast), 3_i32);
    let mut bigint = neg.get_only_child(&h.ast);
    assert_eq!(bigint.get_token(&h.ast), Token::BIGINT);
    assert_eq!(bigint.get_lineno(&h.ast), 1_i32);
    assert_eq!(bigint.get_charno(&h.ast), 1_i32);
    assert_eq!(bigint.get_length(&h.ast), 2_i32);
    assert!(bigint.is_big_int(&h.ast));
    assert_eq!(*bigint.get_big_int(&h.ast), BigInt::from(1));
}

// port: ParserTest#testBigIntLiteralBinary
#[test]
fn test_big_int_literal_binary() {
    let mut h = Harness::new();
    let tmp1 = h.parse("0b10000n;");
    let mut bigint = tmp1.get_only_child(&h.ast).get_only_child(&h.ast);
    assert_eq!(bigint.get_token(&h.ast), Token::BIGINT);
    assert_eq!(bigint.get_lineno(&h.ast), 1_i32);
    assert_eq!(bigint.get_charno(&h.ast), 0_i32);
    assert_eq!(bigint.get_length(&h.ast), 8_i32);
    assert!(bigint.is_big_int(&h.ast));
    assert_eq!(
        *bigint.get_big_int(&h.ast),
        closure_rhino::java_lang::parse_big_integer(&JsString::from("16"), 10).unwrap()
    );
}

// port: ParserTest#testBigIntLiteralOctal
#[test]
fn test_big_int_literal_octal() {
    let mut h = Harness::new();
    let tmp1 = h.parse("0o100n;");
    let mut bigint = tmp1.get_only_child(&h.ast).get_only_child(&h.ast);
    assert_eq!(bigint.get_token(&h.ast), Token::BIGINT);
    assert_eq!(bigint.get_lineno(&h.ast), 1_i32);
    assert_eq!(bigint.get_charno(&h.ast), 0_i32);
    assert_eq!(bigint.get_length(&h.ast), 6_i32);
    assert!(bigint.is_big_int(&h.ast));
    assert_eq!(
        *bigint.get_big_int(&h.ast),
        closure_rhino::java_lang::parse_big_integer(&JsString::from("64"), 10).unwrap()
    );
}

// port: ParserTest#testBigIntLiteralHex
#[test]
fn test_big_int_literal_hex() {
    let mut h = Harness::new();
    let tmp1 = h.parse("0xFn;");
    let mut bigint = tmp1.get_only_child(&h.ast).get_only_child(&h.ast);
    assert_eq!(bigint.get_token(&h.ast), Token::BIGINT);
    assert_eq!(bigint.get_lineno(&h.ast), 1_i32);
    assert_eq!(bigint.get_charno(&h.ast), 0_i32);
    assert_eq!(bigint.get_length(&h.ast), 4_i32);
    assert!(bigint.is_big_int(&h.ast));
    assert_eq!(
        *bigint.get_big_int(&h.ast),
        closure_rhino::java_lang::parse_big_integer(&JsString::from("15"), 10).unwrap()
    );
}

// port: ParserTest#testBigIntInFunctionStatement
#[test]
fn test_big_int_in_function_statement() {
    let mut h = Harness::new();
    let tmp1 = h.parse("function f(/** @type {bigint} */ x) { 0n + x }");
    let mut add = tmp1
        .get_only_child(&h.ast)
        .get_last_child(&h.ast)
        .unwrap()
        .get_only_child(&h.ast)
        .get_only_child(&h.ast);
    assert_eq!(add.get_token(&h.ast), Token::ADD);
    assert!(add.get_first_child(&h.ast).unwrap().is_big_int(&h.ast));
    assert_eq!(
        *add.get_first_child(&h.ast).unwrap().get_big_int(&h.ast),
        BigInt::from(0)
    );
    assert!(add.get_last_child(&h.ast).unwrap().is_name(&h.ast));
    assert_eq!(
        add.get_last_child(&h.ast).unwrap().get_string(&h.ast),
        JsString::from("x")
    );
}

// port: ParserTest#testBigIntLiteralErrors
#[test]
fn test_big_int_literal_errors() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "01n;",
        &["SyntaxError: nonzero BigInt can't have leading zero"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(".1n", &["Semi-colon expected"]);
    let _ = tmp2;
    let tmp3 = h.parse_error("0.1n", &["Semi-colon expected"]);
    let _ = tmp3;
    let tmp4 = h.parse_error("1e1n", &["Semi-colon expected"]);
    let _ = tmp4;
}

// port: ParserTest#testBigIntLiteralWarning
#[test]
fn test_big_int_literal_warning() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT_2019;
    let tmp1 = h.parse_warning(
        "1n;",
        &["This language feature is only supported for ECMASCRIPT_2020 mode or better: bigint"],
    );
    let _ = tmp1;
}

// port: ParserTest#testBigIntFeatureRecorded
#[test]
fn test_big_int_feature_recorded() {
    let mut h = Harness::new();
    let tmp1 = h.parse("1n;");
    let _ = tmp1;
    h.expect_features(&[Feature::BIGINT]);
}

// port: ParserTest#testBigIntLiteralInCall
#[test]
fn test_big_int_literal_in_call() {
    let mut h = Harness::new();
    let tmp1 = h.parse("alert(1n)");
    let _ = tmp1;
}

// port: ParserTest#testBinaryLiterals
#[test]
fn test_binary_literals() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::BINARY_LITERALS]);
    h.mode = LanguageMode::ECMASCRIPT3;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_warning(
        "0b0001;",
        &[&requires_language_mode_message(Feature::BINARY_LITERALS)],
    );
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp2 = h.parse_warning(
        "0b0001;",
        &[&requires_language_mode_message(Feature::BINARY_LITERALS)],
    );
    let _ = tmp2;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    let tmp3 = h.parse("0b0001;");
    let _ = tmp3;
}

// port: ParserTest#testOctalLiterals
#[test]
fn test_octal_literals() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::OCTAL_LITERALS]);
    h.mode = LanguageMode::ECMASCRIPT3;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_warning(
        "0o0001;",
        &[&requires_language_mode_message(Feature::OCTAL_LITERALS)],
    );
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp2 = h.parse_warning(
        "0o0001;",
        &[&requires_language_mode_message(Feature::OCTAL_LITERALS)],
    );
    let _ = tmp2;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    let tmp3 = h.parse("0o0001;");
    let _ = tmp3;
}

// port: ParserTest#testOctalEscapes
#[test]
fn test_octal_escapes() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("var x = 'This is a © copyright symbol.'");
    let mut n = tmp1;
    assert!(n.get_first_first_child(&h.ast).unwrap().is_name(&h.ast));
    assert_eq!(
        n.get_first_first_child(&h.ast).unwrap().get_string(&h.ast),
        JsString::from("x")
    );
    assert!(
        n.get_first_first_child(&h.ast)
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .is_string_lit(&h.ast)
    );
    assert_eq!(
        n.get_first_first_child(&h.ast)
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("This is a © copyright symbol.")
    );
}

// port: ParserTest#testOldStyleOctalLiterals
#[test]
fn test_old_style_octal_literals() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_warning(
        "0001;",
        &["Octal integer literals are not supported in strict mode."],
    );
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp2 = h.parse_warning(
        "0001;",
        &["Octal integer literals are not supported in strict mode."],
    );
    let _ = tmp2;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    let tmp3 = h.parse_warning(
        "0001;",
        &["Octal integer literals are not supported in strict mode."],
    );
    let _ = tmp3;
}

// port: ParserTest#testOldStyleOctalLiterals_strictMode
#[test]
fn test_old_style_octal_literals_strict_mode() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::STRICT;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp1 = h.parse_error(
        "0001;",
        &["Octal integer literals are not supported in strict mode."],
    );
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    let tmp2 = h.parse_error(
        "0001;",
        &["Octal integer literals are not supported in strict mode."],
    );
    let _ = tmp2;
}

// port: ParserTest#testInvalidOctalLiterals
#[test]
fn test_invalid_octal_literals() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("0o08;", &["Invalid octal digit in octal literal."]);
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp2 = h.parse_error("0o08;", &["Invalid octal digit in octal literal."]);
    let _ = tmp2;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    let tmp3 = h.parse_error("0o08;", &["Invalid octal digit in octal literal."]);
    let _ = tmp3;
}

// port: ParserTest#testInvalidOldStyleOctalLiterals
#[test]
fn test_invalid_old_style_octal_literals() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("08;", &["Invalid octal digit in octal literal."]);
    let _ = tmp1;
    let tmp2 = h.parse_error("01238;", &["Invalid octal digit in octal literal."]);
    let _ = tmp2;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp3 = h.parse_error("08;", &["Invalid octal digit in octal literal."]);
    let _ = tmp3;
    let tmp4 = h.parse_error("01238;", &["Invalid octal digit in octal literal."]);
    let _ = tmp4;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    let tmp5 = h.parse_error("08;", &["Invalid octal digit in octal literal."]);
    let _ = tmp5;
    let tmp6 = h.parse_error("01238;", &["Invalid octal digit in octal literal."]);
    let _ = tmp6;
}

// port: ParserTest#testGetter_ObjectLiteral_Es3
#[test]
fn test_getter_object_literal_es3() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::GETTER]);
    h.mode = LanguageMode::ECMASCRIPT3;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error(
        "var x = {get 1(){}};",
        &[closure_parsing::ir_factory::GETTER_ERROR_MESSAGE],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "var x = {get 'a'(){}};",
        &[closure_parsing::ir_factory::GETTER_ERROR_MESSAGE],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "var x = {get a(){}};",
        &[closure_parsing::ir_factory::GETTER_ERROR_MESSAGE],
    );
    let _ = tmp3;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp4 = h.parse("var x = {get 1(){}};");
    let _ = tmp4;
    let tmp5 = h.parse("var x = {get 'a'(){}};");
    let _ = tmp5;
    let tmp6 = h.parse("var x = {get a(){}};");
    let _ = tmp6;
}

// port: ParserTest#testGetter_ObjectLiteral_Es5
#[test]
fn test_getter_object_literal_es5() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::GETTER]);
    h.mode = LanguageMode::ECMASCRIPT5;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("var x = {get 1(){}};");
    let _ = tmp1;
    let tmp2 = h.parse("var x = {get 'a'(){}};");
    let _ = tmp2;
    let tmp3 = h.parse("var x = {get a(){}};");
    let _ = tmp3;
}

// port: ParserTest#testGetterInvalid_ObjectLiteral_EsNext
#[test]
fn test_getter_invalid_object_literal_es_next() {
    let mut h = Harness::new();
    h.expect_features(&[]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("var x = {get a(b){}};", &["')' expected"]);
    let _ = tmp1;
}

// port: ParserTest#testGetter_Computed_ObjectLiteral_Es6
#[test]
fn test_getter_computed_object_literal_es6() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::GETTER, Feature::COMPUTED_PROPERTIES]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("var x = {get [1](){}};");
    let _ = tmp1;
    let tmp2 = h.parse("var x = {get ['a'](){}};");
    let _ = tmp2;
    let tmp3 = h.parse("var x = {get [a](){}};");
    let _ = tmp3;
}

// port: ParserTest#testGetterInvalid_Computed_ObjectLiteral_EsNext
#[test]
fn test_getter_invalid_computed_object_literal_es_next() {
    let mut h = Harness::new();
    h.expect_features(&[]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("var x = {get [a](b){}};", &["')' expected"]);
    let _ = tmp1;
}

// port: ParserTest#testGetter_ClassSyntax
#[test]
fn test_getter_class_syntax() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::CLASSES, Feature::GETTER]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("class Foo { get 1() {} };");
    let _ = tmp1;
    let tmp2 = h.parse("class Foo { get 'a'() {} };");
    let _ = tmp2;
    let tmp3 = h.parse("class Foo { get a() {} };");
    let _ = tmp3;
}

// port: ParserTest#testGetterInvalid_ClassSyntax_EsNext
#[test]
fn test_getter_invalid_class_syntax_es_next() {
    let mut h = Harness::new();
    h.expect_features(&[]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("class Foo { get a(b) {} };", &["')' expected"]);
    let _ = tmp1;
}

// port: ParserTest#testGetter_Computed_ClassSyntax
#[test]
fn test_getter_computed_class_syntax() {
    let mut h = Harness::new();
    h.expect_features(&[
        Feature::CLASSES,
        Feature::GETTER,
        Feature::COMPUTED_PROPERTIES,
    ]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("class Foo { get [1]() {} };");
    let _ = tmp1;
    let tmp2 = h.parse("class Foo { get ['a']() {} };");
    let _ = tmp2;
    let tmp3 = h.parse("class Foo { get [a]() {} };");
    let _ = tmp3;
}

// port: ParserTest#testGetterInvalid_Computed_ClassSyntax_EsNext
#[test]
fn test_getter_invalid_computed_class_syntax_es_next() {
    let mut h = Harness::new();
    h.expect_features(&[]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("class Foo { get [a](b) {} };", &["')' expected"]);
    let _ = tmp1;
}

// port: ParserTest#testSetter_ObjectLiteral_Es3
#[test]
fn test_setter_object_literal_es3() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::SETTER]);
    h.mode = LanguageMode::ECMASCRIPT3;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error(
        "var x = {set 1(x){}};",
        &[closure_parsing::ir_factory::SETTER_ERROR_MESSAGE],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "var x = {set 'a'(x){}};",
        &[closure_parsing::ir_factory::SETTER_ERROR_MESSAGE],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "var x = {set a(x){}};",
        &[closure_parsing::ir_factory::SETTER_ERROR_MESSAGE],
    );
    let _ = tmp3;
}

// port: ParserTest#testSetter_ObjectLiteral_Es5
#[test]
fn test_setter_object_literal_es5() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::SETTER]);
    h.mode = LanguageMode::ECMASCRIPT5;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("var x = {set 1(x){}};");
    let _ = tmp1;
    let tmp2 = h.parse("var x = {set 'a'(x){}};");
    let _ = tmp2;
    let tmp3 = h.parse("var x = {set a(x){}};");
    let _ = tmp3;
}

// port: ParserTest#testSetter_ObjectLiteral_Es6
#[test]
fn test_setter_object_literal_es6() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::SETTER]);
    h.mode = LanguageMode::ECMASCRIPT_2015;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("var x = {set 1(x){}};");
    let _ = tmp1;
    let tmp2 = h.parse("var x = {set 'a'(x){}};");
    let _ = tmp2;
    let tmp3 = h.parse("var x = {set a(x){}};");
    let _ = tmp3;
    let tmp4 = h.parse("var x = {set setter(x = 5) {}};");
    let _ = tmp4;
    let tmp5 = h.parse("var x = {set setter(x = a) {}};");
    let _ = tmp5;
    let tmp6 = h.parse("var x = {set setter(x = a + 5) {}};");
    let _ = tmp6;
    let tmp7 = h.parse("var x = {set setter([x, y, z]) {}};");
    let _ = tmp7;
    let tmp8 = h.parse("var x = {set setter([x, y, ...z]) {}};");
    let _ = tmp8;
    let tmp9 = h.parse("var x = {set setter([x, y, z] = [1, 2, 3]) {}};");
    let _ = tmp9;
    let tmp10 = h.parse("var x = {set setter([x = 1, y = 2, z = 3]) {}};");
    let _ = tmp10;
    let tmp11 = h.parse("var x = {set setter({x, y, z}) {}};");
    let _ = tmp11;
    let tmp12 = h.parse("var x = {set setter({x, y, z} = {x: 1, y: 2, z: 3}) {}};");
    let _ = tmp12;
    let tmp13 = h.parse("var x = {set setter({x = 1, y = 2, z = 3}) {}};");
    let _ = tmp13;
}

// port: ParserTest#testSetterInvalid_ObjectLiteral_EsNext
#[test]
fn test_setter_invalid_object_literal_es_next() {
    let mut h = Harness::new();
    h.expect_features(&[]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error(
        "var x = {set a() {}};",
        &["Setter must have exactly 1 parameter, found 0"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "var x = {set a(x, y) {}};",
        &["Setter must have exactly 1 parameter, found 2"],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "var x = {set a(...x, y) {}};",
        &["Setter must have exactly 1 parameter, found 2"],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "var x = {set a(...x) {}};",
        &["Setter must not have a rest parameter"],
    );
    let _ = tmp4;
}

// port: ParserTest#testSetter_Computed_ObjectLiteral_Es6
#[test]
fn test_setter_computed_object_literal_es6() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::SETTER, Feature::COMPUTED_PROPERTIES]);
    h.mode = LanguageMode::ECMASCRIPT_2015;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("var x = {set [setter](x = 5) {}};");
    let _ = tmp1;
    let tmp2 = h.parse("var x = {set [setter](x = a) {}};");
    let _ = tmp2;
    let tmp3 = h.parse("var x = {set [setter](x = a + 5) {}};");
    let _ = tmp3;
    let tmp4 = h.parse("var x = {set [setter]([x, y, z]) {}};");
    let _ = tmp4;
    let tmp5 = h.parse("var x = {set [setter]([x, y, ...z]) {}};");
    let _ = tmp5;
    let tmp6 = h.parse("var x = {set [setter]([x, y, z] = [1, 2, 3]) {}};");
    let _ = tmp6;
    let tmp7 = h.parse("var x = {set [setter]([x = 1, y = 2, z = 3]) {}};");
    let _ = tmp7;
    let tmp8 = h.parse("var x = {set [setter]({x, y, z}) {}};");
    let _ = tmp8;
    let tmp9 = h.parse("var x = {set [setter]({x, y, z} = {x: 1, y: 2, z: 3}) {}};");
    let _ = tmp9;
    let tmp10 = h.parse("var x = {set [setter]({x = 1, y = 2, z = 3}) {}};");
    let _ = tmp10;
}

// port: ParserTest#testSetterInvalid_Computed_ObjectLiteral_EsNext
#[test]
fn test_setter_invalid_computed_object_literal_es_next() {
    let mut h = Harness::new();
    h.expect_features(&[]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error(
        "var x = {set [setter]() {}};",
        &["Setter must have exactly 1 parameter, found 0"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "var x = {set [setter](x, y) {}};",
        &["Setter must have exactly 1 parameter, found 2"],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "var x = {set [setter](...x, y) {}};",
        &["Setter must have exactly 1 parameter, found 2"],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "var x = {set [setter](...x) {}};",
        &["Setter must not have a rest parameter"],
    );
    let _ = tmp4;
}

// port: ParserTest#testSetter_ClassSyntax
#[test]
fn test_setter_class_syntax() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::CLASSES, Feature::SETTER]);
    let tmp1 = h.parse("class Foo { set setter(x = 5) {} };");
    let _ = tmp1;
    let tmp2 = h.parse("class Foo { set setter(x = a) {} };");
    let _ = tmp2;
    let tmp3 = h.parse("class Foo { set setter(x = a + 5) {} };");
    let _ = tmp3;
    let tmp4 = h.parse("class Foo { set setter([x, y, z]) {} };");
    let _ = tmp4;
    let tmp5 = h.parse("class Foo { set setter([x, y, ...z]) {}};");
    let _ = tmp5;
    let tmp6 = h.parse("class Foo { set setter([x, y, z] = [1, 2, 3]) {} };");
    let _ = tmp6;
    let tmp7 = h.parse("class Foo { set setter([x = 1, y = 2, z = 3]) {} };");
    let _ = tmp7;
    let tmp8 = h.parse("class Foo { set setter({x, y, z}) {}};");
    let _ = tmp8;
    let tmp9 = h.parse("class Foo { set setter({x, y, z} = {x: 1, y: 2, z: 3}) {} };");
    let _ = tmp9;
    let tmp10 = h.parse("class Foo { set setter({x = 1, y = 2, z = 3}) {} };");
    let _ = tmp10;
}

// port: ParserTest#testSetterInvalid_ClassSyntax_EsNext
#[test]
fn test_setter_invalid_class_syntax_es_next() {
    let mut h = Harness::new();
    h.expect_features(&[]);
    let tmp1 = h.parse_error(
        "class Foo { set setter() {} };",
        &["Setter must have exactly 1 parameter, found 0"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "class Foo { set setter(x, y) {} };",
        &["Setter must have exactly 1 parameter, found 2"],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "class Foo { set setter(...x, y) {} };",
        &["Setter must have exactly 1 parameter, found 2"],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "class Foo { set setter(...x) {} };",
        &["Setter must not have a rest parameter"],
    );
    let _ = tmp4;
}

// port: ParserTest#testSetter_Computed_ClassSyntax
#[test]
fn test_setter_computed_class_syntax() {
    let mut h = Harness::new();
    h.expect_features(&[
        Feature::CLASSES,
        Feature::SETTER,
        Feature::COMPUTED_PROPERTIES,
    ]);
    h.mode = LanguageMode::ECMASCRIPT_2015;
    let tmp1 = h.parse("class Foo { set [setter](x = 5) {} };");
    let _ = tmp1;
    let tmp2 = h.parse("class Foo { set [setter](x = a) {} };");
    let _ = tmp2;
    let tmp3 = h.parse("class Foo { set [setter](x = a + 5) {} };");
    let _ = tmp3;
    let tmp4 = h.parse("class Foo { set [setter]([x, y, z]) {} };");
    let _ = tmp4;
    let tmp5 = h.parse("class Foo { set [setter]([x, y, ...z]) {}};");
    let _ = tmp5;
    let tmp6 = h.parse("class Foo { set [setter]([x, y, z] = [1, 2, 3]) {} };");
    let _ = tmp6;
    let tmp7 = h.parse("class Foo { set [setter]([x = 1, y = 2, z = 3]) {} };");
    let _ = tmp7;
    let tmp8 = h.parse("class Foo { set [setter]({x, y, z}) {}};");
    let _ = tmp8;
    let tmp9 = h.parse("class Foo { set [setter]({x, y, z} = {x: 1, y: 2, z: 3}) {} };");
    let _ = tmp9;
    let tmp10 = h.parse("class Foo { set [setter]({x = 1, y = 2, z = 3}) {} };");
    let _ = tmp10;
}

// port: ParserTest#testSetterInvalid_Computed_ClassSyntax_EsNext
#[test]
fn test_setter_invalid_computed_class_syntax_es_next() {
    let mut h = Harness::new();
    h.expect_features(&[]);
    let tmp1 = h.parse_error(
        "class Foo { set [setter]() {} };",
        &["Setter must have exactly 1 parameter, found 0"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "class Foo { set [setter](x, y) {} };",
        &["Setter must have exactly 1 parameter, found 2"],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "class Foo { set [setter](...x, y) {} };",
        &["Setter must have exactly 1 parameter, found 2"],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "class Foo { set [setter](...x) {} };",
        &["Setter must not have a rest parameter"],
    );
    let _ = tmp4;
}

// port: ParserTest#testLamestWarningEver
#[test]
fn test_lamest_warning_ever() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var x = /** @type {undefined} */ (y);");
    let _ = tmp1;
    let tmp2 = h.parse("var x = /** @type {void} */ (y);");
    let _ = tmp2;
}

// port: ParserTest#testUnfinishedComment
#[test]
fn test_unfinished_comment() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("/** this is a comment ", &["unterminated comment"]);
    let _ = tmp1;
}

// port: ParserTest#testHtmlStartCommentAtStartOfLine
#[test]
fn test_html_start_comment_at_start_of_line() {
    let mut h = Harness::new();
    let tmp1 = h.parse_warning(
        "<!-- This text is ignored.\nalert(1)",
        &[HTML_COMMENT_WARNING],
    );
    let _ = tmp1;
}

// port: ParserTest#testHtmlStartComment
#[test]
fn test_html_start_comment() {
    let mut h = Harness::new();
    let tmp1 = h.parse_warning(
        "alert(1) <!-- This text is ignored.\nalert(2)",
        &[HTML_COMMENT_WARNING],
    );
    let _ = tmp1;
}

// port: ParserTest#testHtmlEndCommentAtStartOfLine
#[test]
fn test_html_end_comment_at_start_of_line() {
    let mut h = Harness::new();
    let tmp1 = h.parse_warning(
        "alert(1)\n --> This text is ignored.",
        &[HTML_COMMENT_WARNING],
    );
    let _ = tmp1;
}

// port: ParserTest#testHtmlEndComment
#[test]
fn test_html_end_comment() {
    let mut h = Harness::new();
    let tmp1 = h.parse("while (x --> 0) {\n  alert(1)\n}");
    let _ = tmp1;
}

// port: ParserTest#testParseBlockDescription
#[test]
fn test_parse_block_description() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    let tmp1 = h.parse("/** This is a variable. */ var x;");
    let mut n = tmp1;
    let mut var = n.get_first_child(&h.ast);
    assert!(var.unwrap().get_jsdoc_info(&h.ast).is_some());
    assert_eq!(
        var.unwrap()
            .get_jsdoc_info(&h.ast)
            .as_ref()
            .unwrap()
            .get_block_description(),
        Some(JsString::from("This is a variable."))
    );
}

// port: ParserTest#testUnnamedFunctionStatement
#[test]
fn test_unnamed_function_statement() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("function() {};", &["'identifier' expected"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("if (true) { function() {}; }", &["'identifier' expected"]);
    let _ = tmp2;
    let tmp3 = h.parse("function f() {};");
    let _ = tmp3;
    let tmp4 = h.parse("(function f() {});");
    let _ = tmp4;
    let tmp5 = h.parse("(function () {});");
    let _ = tmp5;
}

// port: ParserTest#testReservedKeywords
#[test]
fn test_reserved_keywords() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::ES3_KEYWORDS_AS_IDENTIFIERS]);
    h.mode = LanguageMode::ECMASCRIPT3;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("var boolean;", &["identifier is a reserved word"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("function boolean() {};", &["identifier is a reserved word"]);
    let _ = tmp2;
    let tmp3 = h.parse_error("boolean = 1;", &["identifier is a reserved word"]);
    let _ = tmp3;
    h.expect_features(&[]);
    let tmp4 = h.parse_error("class = 1;", &["'identifier' expected"]);
    let _ = tmp4;
    let tmp5 = h.parse_error("public = 2;", &["primary expression expected"]);
    let _ = tmp5;
    h.mode = LanguageMode::ECMASCRIPT5;
    h.expect_features(&[Feature::ES3_KEYWORDS_AS_IDENTIFIERS]);
    let tmp6 = h.parse("var boolean;");
    let _ = tmp6;
    let tmp7 = h.parse("function boolean() {};");
    let _ = tmp7;
    let tmp8 = h.parse("boolean = 1;");
    let _ = tmp8;
    h.expect_features(&[]);
    let tmp9 = h.parse_error("class = 1;", &["'identifier' expected"]);
    let _ = tmp9;
    let tmp10 = h.parse_error("var import = 0;", &["'identifier' expected"]);
    let _ = tmp10;
    h.mode = LanguageMode::ECMASCRIPT5;
    h.expect_features(&[Feature::ES3_KEYWORDS_AS_IDENTIFIERS]);
    let tmp11 = h.parse("var boolean;");
    let _ = tmp11;
    let tmp12 = h.parse("function boolean() {};");
    let _ = tmp12;
    let tmp13 = h.parse("boolean = 1;");
    let _ = tmp13;
    h.expect_features(&[]);
    let tmp14 = h.parse_error("public = 2;", &["primary expression expected"]);
    let _ = tmp14;
    let tmp15 = h.parse_error("class = 1;", &["'identifier' expected"]);
    let _ = tmp15;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp16 = h.parse_error("const else = 1;", &["'identifier' expected"]);
    let _ = tmp16;
}

// port: ParserTest#testTypeScriptKeywords
#[test]
fn test_type_script_keywords() {
    let mut h = Harness::new();
    let tmp1 = h.parse("type = 2;");
    let _ = tmp1;
    let tmp2 = h.parse("var type = 3;");
    let _ = tmp2;
    let tmp3 = h.parse("type\nx = 5");
    let _ = tmp3;
    let tmp4 = h.parse("while (i--) { type = types[i]; }");
    let _ = tmp4;
    let tmp5 = h.parse("declare = 2;");
    let _ = tmp5;
    let tmp6 = h.parse("var declare = 3;");
    let _ = tmp6;
    let tmp7 = h.parse("declare\nx = 5");
    let _ = tmp7;
    let tmp8 = h.parse("while (i--) { declare = declares[i]; }");
    let _ = tmp8;
    let tmp9 = h.parse("module = 2;");
    let _ = tmp9;
    let tmp10 = h.parse("var module = 3;");
    let _ = tmp10;
    let tmp11 = h.parse("module\nx = 5");
    let _ = tmp11;
    let tmp12 = h.parse("while (i--) { module = module[i]; }");
    let _ = tmp12;
}

// port: ParserTest#testKeywordsAsProperties1
#[test]
fn test_keywords_as_properties1() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::KEYWORDS_AS_PROPERTIES]);
    h.mode = LanguageMode::ECMASCRIPT3;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_warning(
        "var x = {function: 1};",
        &[closure_parsing::ir_factory::INVALID_ES3_PROP_NAME],
    );
    let _ = tmp1;
    let tmp2 = h.parse_warning(
        "x.function;",
        &[closure_parsing::ir_factory::INVALID_ES3_PROP_NAME],
    );
    let _ = tmp2;
    let tmp3 = h.parse_warning(
        "var x = {class: 1};",
        &[closure_parsing::ir_factory::INVALID_ES3_PROP_NAME],
    );
    let _ = tmp3;
    h.expect_features(&[]);
    let tmp4 = h.parse("var x = {'class': 1};");
    let _ = tmp4;
    h.expect_features(&[Feature::KEYWORDS_AS_PROPERTIES]);
    let tmp5 = h.parse_warning(
        "x.class;",
        &[closure_parsing::ir_factory::INVALID_ES3_PROP_NAME],
    );
    let _ = tmp5;
    h.expect_features(&[]);
    let tmp6 = h.parse("x['class'];");
    let _ = tmp6;
    let tmp7 = h.parse("var x = {let: 1};");
    let _ = tmp7;
    let tmp8 = h.parse("x.let;");
    let _ = tmp8;
    let tmp9 = h.parse("var x = {yield: 1};");
    let _ = tmp9;
    let tmp10 = h.parse("x.yield;");
    let _ = tmp10;
    h.expect_features(&[Feature::KEYWORDS_AS_PROPERTIES]);
    let tmp11 = h.parse_warning(
        "x.prototype.catch = function() {};",
        &[closure_parsing::ir_factory::INVALID_ES3_PROP_NAME],
    );
    let _ = tmp11;
    let tmp12 = h.parse_warning(
        "x().catch();",
        &[closure_parsing::ir_factory::INVALID_ES3_PROP_NAME],
    );
    let _ = tmp12;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp13 = h.parse("var x = {function: 1};");
    let _ = tmp13;
    let tmp14 = h.parse("x.function;");
    let _ = tmp14;
    let tmp15 = h.parse("var x = {get function(){} };");
    let _ = tmp15;
    let tmp16 = h.parse("var x = {set function(a){} };");
    let _ = tmp16;
    let tmp17 = h.parse("var x = {class: 1};");
    let _ = tmp17;
    let tmp18 = h.parse("x.class;");
    let _ = tmp18;
    h.expect_features(&[]);
    let tmp19 = h.parse("var x = {let: 1};");
    let _ = tmp19;
    let tmp20 = h.parse("x.let;");
    let _ = tmp20;
    let tmp21 = h.parse("var x = {yield: 1};");
    let _ = tmp21;
    let tmp22 = h.parse("x.yield;");
    let _ = tmp22;
    h.expect_features(&[Feature::KEYWORDS_AS_PROPERTIES]);
    let tmp23 = h.parse("x.prototype.catch = function() {};");
    let _ = tmp23;
    let tmp24 = h.parse("x().catch();");
    let _ = tmp24;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp25 = h.parse("var x = {function: 1};");
    let _ = tmp25;
    let tmp26 = h.parse("x.function;");
    let _ = tmp26;
    let tmp27 = h.parse("var x = {get function(){} };");
    let _ = tmp27;
    let tmp28 = h.parse("var x = {set function(a){} };");
    let _ = tmp28;
    let tmp29 = h.parse("var x = {class: 1};");
    let _ = tmp29;
    let tmp30 = h.parse("x.class;");
    let _ = tmp30;
    h.expect_features(&[]);
    let tmp31 = h.parse("var x = {let: 1};");
    let _ = tmp31;
    let tmp32 = h.parse("x.let;");
    let _ = tmp32;
    let tmp33 = h.parse("var x = {yield: 1};");
    let _ = tmp33;
    let tmp34 = h.parse("x.yield;");
    let _ = tmp34;
    h.expect_features(&[Feature::KEYWORDS_AS_PROPERTIES]);
    let tmp35 = h.parse("x.prototype.catch = function() {};");
    let _ = tmp35;
    let tmp36 = h.parse("x().catch();");
    let _ = tmp36;
}

// port: ParserTest#testKeywordsAsProperties2
#[test]
fn test_keywords_as_properties2() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var x = {get 'function'(){} };");
    let _ = tmp1;
    let tmp2 = h.parse("var x = {get 1(){} };");
    let _ = tmp2;
    let tmp3 = h.parse("var x = {set 'function'(a){} };");
    let _ = tmp3;
    let tmp4 = h.parse("var x = {set 1(a){} };");
    let _ = tmp4;
}

// port: ParserTest#testKeywordsAsProperties3
#[test]
fn test_keywords_as_properties3() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var x = {get 'function'(){} };");
    let _ = tmp1;
    let tmp2 = h.parse("var x = {get 1(){} };");
    let _ = tmp2;
    let tmp3 = h.parse("var x = {set 'function'(a){} };");
    let _ = tmp3;
    let tmp4 = h.parse("var x = {set 1(a){} };");
    let _ = tmp4;
}

// port: ParserTest#testKeywordsAsPropertiesInExterns1
#[test]
fn test_keywords_as_properties_in_externs1() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("/** @fileoverview\n@externs\n*/\n var x = {function: 1};");
    let _ = tmp1;
}

// port: ParserTest#testKeywordsAsPropertiesInExterns2
#[test]
fn test_keywords_as_properties_in_externs2() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("/** @fileoverview\n@externs\n*/\n var x = {}; x.function + 1;");
    let _ = tmp1;
}

// port: ParserTest#testExternsLanguageMode
#[test]
fn test_externs_language_mode() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT3;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("/** @fileoverview\n@externs\n*/\n const x = 1;");
    let _ = tmp1;
}

// port: ParserTest#testUnicodeInIdentifiers
#[test]
fn test_unicode_in_identifiers() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var à");
    let _ = tmp1;
    let tmp2 = h.parse("var cosθ");
    let _ = tmp2;
    let tmp3 = h.parse("if(true){foo=α}");
    let _ = tmp3;
    let tmp4 = h.parse("if(true){foo=Δ}else bar()");
    let _ = tmp4;
}

// port: ParserTest#testUnicodeEscapeInIdentifiers
#[test]
fn test_unicode_escape_in_identifiers() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var \\u{00fb}");
    let _ = tmp1;
    let tmp2 = h.parse("var \\u{00fb}test\\u{00fb}");
    let _ = tmp2;
    let tmp3 = h.parse("Js\\u{00C7}ompiler");
    let _ = tmp3;
    let tmp4 = h.parse("Js\\u{0043}ompiler");
    let _ = tmp4;
    let tmp5 = h.parse("if(true){foo=\\u{03b5}}");
    let _ = tmp5;
    let tmp6 = h.parse("if(true){foo=\\u{03b5}}else bar()");
    let _ = tmp6;
}

// port: ParserTest#testUnicodePointEscapeInIdentifiers
#[test]
fn test_unicode_point_escape_in_identifiers() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var \\u{0043}");
    let _ = tmp1;
    let tmp2 = h.parse("var \\u{0043}test\\u{0043}");
    let _ = tmp2;
    let tmp3 = h.parse("var \\u{0043}test\\u{0043}");
    let _ = tmp3;
    let tmp4 = h.parse("var \\u{0043}test\\u{0043}");
    let _ = tmp4;
    let tmp5 = h.parse("Js\\u{0043}ompiler");
    let _ = tmp5;
    let tmp6 = h.parse("Js\\u{275}ompiler");
    let _ = tmp6;
    let tmp7 = h.parse("var \\u{0043};{43}");
    let _ = tmp7;
}

// port: ParserTest#testUnicodePointEscapeStringLiterals
#[test]
fn test_unicode_point_escape_string_literals() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var i = '\\u{0043}ompiler'");
    let _ = tmp1;
    let tmp2 = h.parse("var i = '\\u{43}ompiler'");
    let _ = tmp2;
    let tmp3 = h.parse("var i = '\\u{1f42a}ompiler'");
    let _ = tmp3;
    let tmp4 = h.parse("var i = '\\u{2603}ompiler'");
    let _ = tmp4;
    let tmp5 = h.parse("var i = '\\u{1}ompiler'");
    let _ = tmp5;
}

// port: ParserTest#testUnicodePointEscapeTemplateLiterals
#[test]
fn test_unicode_point_escape_template_literals() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var i = `\\u{0043}ompiler`");
    let _ = tmp1;
    let tmp2 = h.parse("var i = `\\u{43}ompiler`");
    let _ = tmp2;
    let tmp3 = h.parse("var i = `\\u{1f42a}ompiler`");
    let _ = tmp3;
    let tmp4 = h.parse("var i = `\\u{2603}ompiler`");
    let _ = tmp4;
    let tmp5 = h.parse("var i = `\\u{1}ompiler`");
    let _ = tmp5;
}

// port: ParserTest#testInvalidUnicodePointEscapeInIdentifiers
#[test]
fn test_invalid_unicode_point_escape_in_identifiers() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("var \\u{defg", &["Invalid escape sequence"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("var \\u{03b5", &["Invalid escape sequence"]);
    let _ = tmp2;
    let tmp3 = h.parse_error("var \\u43{43}", &["Invalid escape sequence"]);
    let _ = tmp3;
    let tmp4 = h.parse_error("var \\u{defgRestOfIdentifier", &["Invalid escape sequence"]);
    let _ = tmp4;
    let tmp5 = h.parse_error("var \\u{03b5}}", &["primary expression expected"]);
    let _ = tmp5;
    let tmp6 = h.parse_error("var \\u{03b5}}}", &["primary expression expected"]);
    let _ = tmp6;
    let tmp7 = h.parse_error("var \\u{03b5}{}", &[SEMICOLON_EXPECTED]);
    let _ = tmp7;
    let tmp8 = h.parse_error("var \\u{0043}{43}", &[SEMICOLON_EXPECTED]);
    let _ = tmp8;
    let tmp9 = h.parse_error("var \\u{DEFG}", &["Invalid escape sequence"]);
    let _ = tmp9;
    let tmp10 = h.parse_error("Js\\u{}ompiler", &["Invalid escape sequence"]);
    let _ = tmp10;
    let tmp11 = h.parse_error("Js\\u{99}ompiler", &["Invalid escape sequence"]);
    let _ = tmp11;
    let tmp12 = h.parse_error("Js\\u{10000}ompiler", &["Invalid escape sequence"]);
    let _ = tmp12;
    let tmp13 = h.parse_error("Js\\u{10041}ompiler", &["Invalid escape sequence"]);
    let _ = tmp13;
    let tmp14 = h.parse_error("Js\\u{110041}ompiler", &["Invalid escape sequence"]);
    let _ = tmp14;
}

// port: ParserTest#testInvalidUnicodePointEscapeStringLiterals
#[test]
fn test_invalid_unicode_point_escape_string_literals() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("var i = '\\u{defg'", &["Hex digit expected"]);
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "var i = '\\u{defgRestOfIdentifier'",
        &["Hex digit expected"],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error("var i = '\\u{DEFG}'", &["Hex digit expected"]);
    let _ = tmp3;
    let tmp4 = h.parse_error("var i = 'Js\\u{}ompiler'", &["Empty unicode escape"]);
    let _ = tmp4;
    let tmp5 = h.parse_error("var i = '\\u{345", &["Hex digit expected"]);
    let _ = tmp5;
    let tmp6 = h.parse_error("var i = '\\u{110000}'", &["Undefined Unicode code-point"]);
    let _ = tmp6;
}

// port: ParserTest#testInvalidUnicodePointEscapeTemplateLiterals
#[test]
fn test_invalid_unicode_point_escape_template_literals() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("var i = `\\u{defg`", &["Hex digit expected"]);
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "var i = `\\u{defgRestOfIdentifier`",
        &["Hex digit expected"],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error("var i = `\\u{DEFG}`", &["Hex digit expected"]);
    let _ = tmp3;
    let tmp4 = h.parse_error("var i = `Js\\u{}ompiler`", &["Empty unicode escape"]);
    let _ = tmp4;
    let tmp5 = h.parse_error("var i = `\\u{345`", &["Hex digit expected"]);
    let _ = tmp5;
    let tmp6 = h.parse_error("var i = `\\u{110000}`", &["Undefined Unicode code-point"]);
    let _ = tmp6;
}

// port: ParserTest#testEs2018LiftIllegalEscapeSequenceRestrictionOnTaggedTemplates
#[test]
fn test_es2018_lift_illegal_escape_sequence_restriction_on_tagged_templates() {
    let mut h = Harness::new();
    let tmp1 = h.parse("latex`\\unicode`");
    let _ = tmp1;
    let tmp2 = h.parse("foo`\\xerxes`");
    let _ = tmp2;
    let tmp3 = h.parse("bar`\\u{h}ere`");
    let _ = tmp3;
    let tmp4 = h.parse("bar`\\u{43`");
    let _ = tmp4;
    let tmp5 = h.parse_error("foo`\\unicode", &["Unterminated template literal"]);
    let _ = tmp5;
    let tmp6 = h.parse_error("var bad = `\\unicode`;", &["Hex digit expected"]);
    let _ = tmp6;
}

// port: ParserTest#testInvalidEscape
#[test]
fn test_invalid_escape() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("var \\x39abc", &["Invalid escape sequence"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("var abc\\t", &["Invalid escape sequence"]);
    let _ = tmp2;
}

// port: ParserTest#testUnnecessaryEscape
#[test]
fn test_unnecessary_escape() {
    let mut h = Harness::new();
    let tmp1 = h.parse_warning(
        "var str = '\\a'",
        &["Unnecessary escape: '\\a' is equivalent to just 'a'"],
    );
    let _ = tmp1;
    let tmp2 = h.parse("var str = '\\u{8}'");
    let _ = tmp2;
    let tmp3 = h.parse_warning(
        "var str = '\\c'",
        &["Unnecessary escape: '\\c' is equivalent to just 'c'"],
    );
    let _ = tmp3;
    let tmp4 = h.parse_warning(
        "var str = '\\d'",
        &["Unnecessary escape: '\\d' is equivalent to just 'd'"],
    );
    let _ = tmp4;
    let tmp5 = h.parse_warning(
        "var str = '\\e'",
        &["Unnecessary escape: '\\e' is equivalent to just 'e'"],
    );
    let _ = tmp5;
    let tmp6 = h.parse("var str = '\\u{c}'");
    let _ = tmp6;
    let tmp7 = h.parse("var str = '\\/'");
    let _ = tmp7;
    let tmp8 = h.parse("var str = '\\0'");
    let _ = tmp8;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp9 = h.parse_warning(
        "var str = '\\1'",
        &["Unnecessary escape: '\\1' is equivalent to just '1'"],
    );
    let _ = tmp9;
    let tmp10 = h.parse_warning(
        "var str = '\\2'",
        &["Unnecessary escape: '\\2' is equivalent to just '2'"],
    );
    let _ = tmp10;
    let tmp11 = h.parse_warning(
        "var str = '\\3'",
        &["Unnecessary escape: '\\3' is equivalent to just '3'"],
    );
    let _ = tmp11;
    let tmp12 = h.parse_warning(
        "var str = '\\4'",
        &["Unnecessary escape: '\\4' is equivalent to just '4'"],
    );
    let _ = tmp12;
    let tmp13 = h.parse_warning(
        "var str = '\\5'",
        &["Unnecessary escape: '\\5' is equivalent to just '5'"],
    );
    let _ = tmp13;
    let tmp14 = h.parse_warning(
        "var str = '\\6'",
        &["Unnecessary escape: '\\6' is equivalent to just '6'"],
    );
    let _ = tmp14;
    let tmp15 = h.parse_warning(
        "var str = '\\7'",
        &["Unnecessary escape: '\\7' is equivalent to just '7'"],
    );
    let _ = tmp15;
    let tmp16 = h.parse_warning(
        "var str = '\\8'",
        &["Unnecessary escape: '\\8' is equivalent to just '8'"],
    );
    let _ = tmp16;
    let tmp17 = h.parse_warning(
        "var str = '\\9'",
        &["Unnecessary escape: '\\9' is equivalent to just '9'"],
    );
    let _ = tmp17;
    let tmp18 = h.parse_warning(
        "var str = '\\%'",
        &["Unnecessary escape: '\\%' is equivalent to just '%'"],
    );
    let _ = tmp18;
    let tmp19 = h.parse_warning(
        "var str = '\\$'",
        &["Unnecessary escape: '\\$' is equivalent to just '$'"],
    );
    let _ = tmp19;
}

// port: ParserTest#testUnnecessaryEscapeUntaggedTemplateLiterals
#[test]
fn test_unnecessary_escape_untagged_template_literals() {
    let mut h = Harness::new();
    let tmp1 = h.parse_warning(
        "var str = `\\a`",
        &["Unnecessary escape: '\\a' is equivalent to just 'a'"],
    );
    let _ = tmp1;
    let tmp2 = h.parse("var str = `\\u{8}`");
    let _ = tmp2;
    let tmp3 = h.parse_warning(
        "var str = `\\c`",
        &["Unnecessary escape: '\\c' is equivalent to just 'c'"],
    );
    let _ = tmp3;
    let tmp4 = h.parse_warning(
        "var str = `\\d`",
        &["Unnecessary escape: '\\d' is equivalent to just 'd'"],
    );
    let _ = tmp4;
    let tmp5 = h.parse_warning(
        "var str = `\\e`",
        &["Unnecessary escape: '\\e' is equivalent to just 'e'"],
    );
    let _ = tmp5;
    let tmp6 = h.parse("var str = `\\u{c}`");
    let _ = tmp6;
    let tmp7 = h.parse_warning(
        "var str = `\\/`",
        &["Unnecessary escape: '\\/' is equivalent to just '/'"],
    );
    let _ = tmp7;
    let tmp8 = h.parse("var str = `\\0`");
    let _ = tmp8;
    let tmp9 = h.parse_warning(
        "var str = `\\%`",
        &["Unnecessary escape: '\\%' is equivalent to just '%'"],
    );
    let _ = tmp9;
    let tmp10 = h.parse_warning(
        "var str = `\\\"`",
        &["Unnecessary escape: '\\\"' is equivalent to just '\"'"],
    );
    let _ = tmp10;
    let tmp11 = h.parse_warning(
        "var str = `\\'`",
        &["Unnecessary escape: \"\\'\" is equivalent to just \"'\""],
    );
    let _ = tmp11;
    let tmp12 = h.parse("var str = `\\$`");
    let _ = tmp12;
    let tmp13 = h.parse("var str = `\\``");
    let _ = tmp13;
}

// port: ParserTest#testUnnecessaryEscapeTaggedTemplateLiterals
#[test]
fn test_unnecessary_escape_tagged_template_literals() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::TEMPLATE_LITERALS]);
    let tmp1 = h.parse("var str = String.raw`\\a`");
    let _ = tmp1;
    let tmp2 = h.parse("var str = String.raw`\\u{8}`");
    let _ = tmp2;
    let tmp3 = h.parse("var str = String.raw`\\c`");
    let _ = tmp3;
    let tmp4 = h.parse("var str = String.raw`\\d`");
    let _ = tmp4;
    let tmp5 = h.parse("var str = String.raw`\\e`");
    let _ = tmp5;
    let tmp6 = h.parse("var str = String.raw`\\u{c}`");
    let _ = tmp6;
    let tmp7 = h.parse("var str = String.raw`\\/`");
    let _ = tmp7;
    let tmp8 = h.parse("var str = String.raw`\\0`");
    let _ = tmp8;
    let tmp9 = h.parse("var str = String.raw`\\8`");
    let _ = tmp9;
    let tmp10 = h.parse("var str = String.raw`\\9`");
    let _ = tmp10;
    let tmp11 = h.parse("var str = String.raw`\\%`");
    let _ = tmp11;
    let tmp12 = h.parse("var str = String.raw`\\$`");
    let _ = tmp12;
    let tmp13 = h.parse("var str = String.raw`\\``");
    let _ = tmp13;
}

// port: ParserTest#testEOFInUnicodeEscape
#[test]
fn test_eof_in_unicode_escape() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("var \\u1", &["Invalid escape sequence"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("var \\u12", &["Invalid escape sequence"]);
    let _ = tmp2;
    let tmp3 = h.parse_error("var \\u123", &["Invalid escape sequence"]);
    let _ = tmp3;
}

// port: ParserTest#testEndOfIdentifierInUnicodeEscape
#[test]
fn test_end_of_identifier_in_unicode_escape() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("var \\u1 = 1;", &["Invalid escape sequence"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("var \\u12 = 2;", &["Invalid escape sequence"]);
    let _ = tmp2;
    let tmp3 = h.parse_error("var \\u123 = 3;", &["Invalid escape sequence"]);
    let _ = tmp3;
}

// port: ParserTest#testInvalidUnicodeEscape
#[test]
fn test_invalid_unicode_escape() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("var \\uDEFG", &["Invalid escape sequence"]);
    let _ = tmp1;
}

// port: ParserTest#testUnicodeEscapeInvalidIdentifierStart
#[test]
fn test_unicode_escape_invalid_identifier_start() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "var \\u{0037}yler",
        &["Character '7' (U+0037) is not a valid identifier start char"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "var \\u{37}yler",
        &["Character '7' (U+0037) is not a valid identifier start char"],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error("var \\u{0020}space", &["Invalid escape sequence"]);
    let _ = tmp3;
}

// port: ParserTest#testUnicodeEscapeInvalidIdentifierChar
#[test]
fn test_unicode_escape_invalid_identifier_char() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("var sp\\u{0020}ce", &["Invalid escape sequence"]);
    let _ = tmp1;
}

// port: ParserTest#testKeywordAsIdentifier
#[test]
fn test_keyword_as_identifier() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("var while;", &["'identifier' expected"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("var wh\\u{0069}le;", &["'identifier' expected"]);
    let _ = tmp2;
    let tmp3 = h.parse_error("var v\\u{0061}r;", &["'identifier' expected"]);
    let _ = tmp3;
}

// port: ParserTest#testUnicodeEscapeInKeywords
#[test]
fn test_unicode_escape_in_keywords() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("v\\u{0061}r b = 1;", &["'identifier' expected"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("i\\u{0066} (b) { }", &["'identifier' expected"]);
    let _ = tmp2;
    let tmp3 = h.parse_error("f\\u{0075}nction foo() {}", &["'identifier' expected"]);
    let _ = tmp3;
    let tmp4 = h.parse_error("l\\u{0065}t x = 1;", &["'identifier' expected"]);
    let _ = tmp4;
    let tmp5 = h.parse_error("l\\u{0065}t = 1;", &["'identifier' expected"]);
    let _ = tmp5;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp6 = h.parse("l\\u{0065}t = 1;");
    let _ = tmp6;
}

// port: ParserTest#testUnicodeEscapeInPropertyNames
#[test]
fn test_unicode_escape_in_property_names() {
    let mut h = Harness::new();
    let tmp1 = h.parse("obj.v\\u{0061}r");
    let _ = tmp1;
    let tmp2 = h.parse("var x = { v\\u{0061}r: 1 };");
    let _ = tmp2;
}

// port: ParserTest#testGetPropFunctionName
#[test]
fn test_get_prop_function_name() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("function a.b() {}", &["'(' expected"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("var x = function a.b() {}", &["'(' expected"]);
    let _ = tmp2;
}

// port: ParserTest#testIdeModePartialTree
#[test]
fn test_ide_mode_partial_tree() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("function Foo() {} f.", &["'identifier' expected"]);
    let mut partial_tree = tmp1;
    assert!(partial_tree.is_none());
    h.is_ide_mode = true;
    let tmp2 = h.parse_error("function Foo() {} f.", &["'identifier' expected"]);
    partial_tree = tmp2;
    assert!(partial_tree.is_some());
}

// port: ParserTest#testForEach
#[test]
fn test_for_each() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("function f(stamp, status) {\n  for each ( var curTiming in this.timeLog.timings ) {\n    if ( curTiming.callId == stamp ) {\n      curTiming.flag = status;\n      break;\n    }\n  }\n};\n", &["'(' expected"]);
    let _ = tmp1;
}

// port: ParserTest#testValidTypeAnnotation1
#[test]
fn test_valid_type_annotation1() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/** @type {string} */ var o = 'str';");
    let _ = tmp1;
    let tmp2 = h.parse("var /** @type {string} */ o = 'str', /** @type {number} */ p = 0;");
    let _ = tmp2;
    let tmp3 = h.parse("/** @type {function():string} */ function o() { return 'str'; }");
    let _ = tmp3;
    let tmp4 = h.parse("var o = {}; /** @type {string} */ o.prop = 'str';");
    let _ = tmp4;
    let tmp5 = h.parse("var o = {}; /** @type {string} */ o['prop'] = 'str';");
    let _ = tmp5;
    let tmp6 = h.parse("var o = { /** @type {string} */ prop : 'str' };");
    let _ = tmp6;
    let tmp7 = h.parse("var o = { /** @type {string} */ 'prop' : 'str' };");
    let _ = tmp7;
    let tmp8 = h.parse("var o = { /** @type {string} */ 1 : 'str' };");
    let _ = tmp8;
}

// port: ParserTest#testValidTypeAnnotation2
#[test]
fn test_valid_type_annotation2() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::GETTER]);
    let tmp1 = h.parse("var o = { /** @type {string} */ get prop() { return 'str' }};");
    let _ = tmp1;
    h.expect_features(&[Feature::SETTER]);
    let tmp2 = h.parse("var o = { /** @type {string} */ set prop(s) {}};");
    let _ = tmp2;
}

// port: ParserTest#testValidTypeAnnotation3
#[test]
fn test_valid_type_annotation3() {
    let mut h = Harness::new();
    let tmp1 = h.parse("try {} catch (/** @type {Error} */ e) {}");
    let _ = tmp1;
}

// port: ParserTest#testValidTypeAnnotation4
#[test]
fn test_valid_type_annotation4() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::MODULES]);
    let tmp1 = h.parse("/** @type {number} */ export var x = 3;");
    let _ = tmp1;
}

// port: ParserTest#testTypeofJsdoc
#[test]
fn test_typeof_jsdoc() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var b = 0;");
    let tmp2 = h.parse("var /** typeof a */ b = 0;");
    assert_node_equality(&h.ast, tmp1, tmp2);
    let tmp3 = h.parse("var b = 0;");
    let tmp4 = h.parse_warning(
        "var /** typeof {a} */ b = 0;",
        &[UNNECESSARY_BRACES_MESSAGE],
    );
    assert_node_equality(&h.ast, tmp3, tmp4);
    let tmp5 = h.parse("var b = 0;");
    let tmp6 = h.parse_warning(
        "var /** typeof <a> */ b = 0;",
        &[NAME_NOT_RECOGNIZED_MESSAGE],
    );
    assert_node_equality(&h.ast, tmp5, tmp6);
}

// port: ParserTest#testParsingAssociativity
#[test]
fn test_parsing_associativity() {
    let mut h = Harness::new();
    let tmp1 = h.parse("x * y * z");
    let tmp2 = h.parse("(x * y) * z");
    assert_node_equality(&h.ast, tmp1, tmp2);
    let tmp3 = h.parse("x + y + z");
    let tmp4 = h.parse("(x + y) + z");
    assert_node_equality(&h.ast, tmp3, tmp4);
    let tmp5 = h.parse("x | y | z");
    let tmp6 = h.parse("(x | y) | z");
    assert_node_equality(&h.ast, tmp5, tmp6);
    let tmp7 = h.parse("x & y & z");
    let tmp8 = h.parse("(x & y) & z");
    assert_node_equality(&h.ast, tmp7, tmp8);
    let tmp9 = h.parse("x ^ y ^ z");
    let tmp10 = h.parse("(x ^ y) ^ z");
    assert_node_equality(&h.ast, tmp9, tmp10);
    let tmp11 = h.parse("x || y || z");
    let tmp12 = h.parse("(x || y) || z");
    assert_node_equality(&h.ast, tmp11, tmp12);
    let tmp13 = h.parse("x && y && z");
    let tmp14 = h.parse("(x && y) && z");
    assert_node_equality(&h.ast, tmp13, tmp14);
}

// port: ParserTest#testIssue1116
#[test]
fn test_issue1116() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/**/");
    let _ = tmp1;
}

// port: ParserTest#testUnterminatedStringLiteral
#[test]
fn test_unterminated_string_literal() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "var unterm = 'forgot closing quote",
        &["Unterminated string literal"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "var unterm = 'forgot closing quote\nalert(unterm);\n",
        &["Unterminated string literal"],
    );
    let _ = tmp2;
    let mut js = ("var unterm = ' \\\n \\a \n").to_string();
    let mut tmp3 = TestErrorReporter::new();
    tmp3.expect_all_warnings(&["Unnecessary escape: '\\a' is equivalent to just 'a'"]);
    tmp3.expect_all_errors(&["Unterminated string literal"]);
    let mut test_error_reporter = tmp3;
    let mut file = Arc::new(SimpleSourceFile::new("input", SourceKind::STRONG));
    let tmp4 = h.create_config();
    let tmp5 = ParserRunner::parse(
        &mut h.ast,
        file.clone(),
        JsString::from(js),
        &tmp4,
        &mut test_error_reporter,
    );
    let _ = tmp5;
    test_error_reporter.verify_has_encountered_all_warnings_and_errors();
}

// port: ParserTest#testUnterminatedRegExp
#[test]
fn test_unterminated_reg_exp() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "var unterm = /forgot trailing slash",
        &["Expected '/' in regular expression literal"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "var unterm = /forgot trailing slash\nalert(unterm);\n",
        &["Expected '/' in regular expression literal"],
    );
    let _ = tmp2;
}

// port: ParserTest#testRegExp
#[test]
fn test_reg_exp() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/a/");
    let tmp2 = regex(&mut h.ast, "a", None);
    let tmp3 = expr(&mut h.ast, tmp2);
    let tmp4 = script(&mut h.ast, tmp3);
    assert_node_equality(&h.ast, tmp1, tmp4);
    let tmp5 = h.parse("/\\\\/");
    let tmp6 = regex(&mut h.ast, "\\\\", None);
    let tmp7 = expr(&mut h.ast, tmp6);
    let tmp8 = script(&mut h.ast, tmp7);
    assert_node_equality(&h.ast, tmp5, tmp8);
    let tmp9 = h.parse("/\\s/");
    let tmp10 = regex(&mut h.ast, "\\s", None);
    let tmp11 = expr(&mut h.ast, tmp10);
    let tmp12 = script(&mut h.ast, tmp11);
    assert_node_equality(&h.ast, tmp9, tmp12);
    let tmp13 = h.parse("/\\u{000A}/");
    let tmp14 = regex(&mut h.ast, "\\u{000A}", None);
    let tmp15 = expr(&mut h.ast, tmp14);
    let tmp16 = script(&mut h.ast, tmp15);
    assert_node_equality(&h.ast, tmp13, tmp16);
    let tmp17 = h.parse("/[\\]]/");
    let tmp18 = regex(&mut h.ast, "[\\]]", None);
    let tmp19 = expr(&mut h.ast, tmp18);
    let tmp20 = script(&mut h.ast, tmp19);
    assert_node_equality(&h.ast, tmp17, tmp20);
    let tmp21 = h.parse("/\\k<group>/");
    let tmp22 = regex(&mut h.ast, "\\k<group>", None);
    let tmp23 = expr(&mut h.ast, tmp22);
    let tmp24 = script(&mut h.ast, tmp23);
    assert_node_equality(&h.ast, tmp21, tmp24);
    let tmp25 = h.parse("/(?<group>a)\\k<group>/");
    let tmp26 = regex(&mut h.ast, "(?<group>a)\\k<group>", None);
    let tmp27 = expr(&mut h.ast, tmp26);
    let tmp28 = script(&mut h.ast, tmp27);
    assert_node_equality(&h.ast, tmp25, tmp28);
}

// port: ParserTest#testRegExpError
#[test]
fn test_reg_exp_error() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("/a\\/", &["Expected '/' in regular expression literal"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("/\\ca\\/", &["Expected '/' in regular expression literal"]);
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "/\u{8}.\\/",
        &["Expected '/' in regular expression literal"],
    );
    let _ = tmp3;
}

// port: ParserTest#testRegExpUnicode
#[test]
fn test_reg_exp_unicode() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/\\u{10fA}/");
    let tmp2 = regex(&mut h.ast, "\\u{10fA}", None);
    let tmp3 = expr(&mut h.ast, tmp2);
    let tmp4 = script(&mut h.ast, tmp3);
    assert_node_equality(&h.ast, tmp1, tmp4);
    let tmp5 = h.parse("/\\u{10fA}/u");
    let tmp6 = regex(&mut h.ast, "\\u{10fA}", Some("u"));
    let tmp7 = expr(&mut h.ast, tmp6);
    let tmp8 = script(&mut h.ast, tmp7);
    assert_node_equality(&h.ast, tmp5, tmp8);
    let tmp9 = h.parse("/\\u{1fA}/u");
    let tmp10 = regex(&mut h.ast, "\\u{1fA}", Some("u"));
    let tmp11 = expr(&mut h.ast, tmp10);
    let tmp12 = script(&mut h.ast, tmp11);
    assert_node_equality(&h.ast, tmp9, tmp12);
    let tmp13 = h.parse("/\\u{10FFFF}/u");
    let tmp14 = regex(&mut h.ast, "\\u{10FFFF}", Some("u"));
    let tmp15 = expr(&mut h.ast, tmp14);
    let tmp16 = script(&mut h.ast, tmp15);
    assert_node_equality(&h.ast, tmp13, tmp16);
}

// port: ParserTest#testRegExpFlags
#[test]
fn test_reg_exp_flags() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/a/");
    let _ = tmp1;
    let tmp2 = h.parse("/a/i");
    let _ = tmp2;
    let tmp3 = h.parse("/a/g");
    let _ = tmp3;
    let tmp4 = h.parse("/a/m");
    let _ = tmp4;
    let tmp5 = h.parse("/a/ig");
    let _ = tmp5;
    let tmp6 = h.parse("/a/gm");
    let _ = tmp6;
    let tmp7 = h.parse("/a/mgi");
    let _ = tmp7;
    let tmp8 = h.parse_error("/a/a", &["Invalid RegExp flag 'a'"]);
    let _ = tmp8;
    let tmp9 = h.parse_error("/a/b", &["Invalid RegExp flag 'b'"]);
    let _ = tmp9;
    let tmp10 = h.parse_error(
        "/a/abc",
        &[
            "Invalid RegExp flag 'a'",
            "Invalid RegExp flag 'b'",
            "Invalid RegExp flag 'c'",
        ],
    );
    let _ = tmp10;
}

// port: ParserTest#testES6RegExpFlags
#[test]
fn test_es6_reg_exp_flags() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::REGEXP_FLAG_Y]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("/a/y");
    let _ = tmp1;
    h.expect_features(&[Feature::REGEXP_FLAG_U]);
    let tmp2 = h.parse("/a/u");
    let _ = tmp2;
    h.mode = LanguageMode::ECMASCRIPT5;
    h.expect_features(&[Feature::REGEXP_FLAG_Y]);
    let tmp3 = h.parse_warning(
        "/a/y",
        &[&requires_language_mode_message(Feature::REGEXP_FLAG_Y)],
    );
    let _ = tmp3;
    h.expect_features(&[Feature::REGEXP_FLAG_U]);
    let tmp4 = h.parse_warning(
        "/a/u",
        &[&requires_language_mode_message(Feature::REGEXP_FLAG_U)],
    );
    let _ = tmp4;
    let tmp5 = h.parse_warning(
        "/a/yu",
        &[
            &requires_language_mode_message(Feature::REGEXP_FLAG_Y),
            &requires_language_mode_message(Feature::REGEXP_FLAG_U),
        ],
    );
    let _ = tmp5;
}

// port: ParserTest#testES2018RegExpFlagS
#[test]
fn test_es2018_reg_exp_flag_s() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::REGEXP_FLAG_S]);
    let tmp1 = h.parse("/a/s");
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    h.expect_features(&[Feature::REGEXP_FLAG_S]);
    let tmp2 = h.parse_warning(
        "/a/s",
        &[&requires_language_mode_message(Feature::REGEXP_FLAG_S)],
    );
    let _ = tmp2;
    let tmp3 = h.parse_warning(
        "/a/us",
        &[&requires_language_mode_message(Feature::REGEXP_FLAG_S)],
    );
    let _ = tmp3;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp4 = h.parse_warning(
        "/a/us",
        &[
            &requires_language_mode_message(Feature::REGEXP_FLAG_U),
            &requires_language_mode_message(Feature::REGEXP_FLAG_S),
        ],
    );
    let _ = tmp4;
}

// port: ParserTest#testES2022RegExpFlagD
#[test]
fn test_es2022_reg_exp_flag_d() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::REGEXP_FLAG_D]);
    let tmp1 = h.parse("/a/d");
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    h.expect_features(&[Feature::REGEXP_FLAG_D]);
    let tmp2 = h.parse_warning(
        "/a/d",
        &[&requires_language_mode_message(Feature::REGEXP_FLAG_D)],
    );
    let _ = tmp2;
    let tmp3 = h.parse_warning(
        "/a/ud",
        &[&requires_language_mode_message(Feature::REGEXP_FLAG_D)],
    );
    let _ = tmp3;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp4 = h.parse_warning(
        "/a/ud",
        &[
            &requires_language_mode_message(Feature::REGEXP_FLAG_U),
            &requires_language_mode_message(Feature::REGEXP_FLAG_D),
        ],
    );
    let _ = tmp4;
}

// port: ParserTest#testDefaultParameters
#[test]
fn test_default_parameters() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::DEFAULT_PARAMETERS]);
    let tmp1 = h.parse("function f(a, b=0) {}");
    let _ = tmp1;
    let tmp2 = h.parse("function f(a, b=0, c) {}");
    let _ = tmp2;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp3 = h.parse_warning(
        "function f(a, b=0) {}",
        &[&requires_language_mode_message(Feature::DEFAULT_PARAMETERS)],
    );
    let _ = tmp3;
}

// port: ParserTest#testDefaultParameterInlineNonJSDocComment
#[test]
fn test_default_parameter_inline_non_js_doc_comment() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    h.expect_features(&[Feature::DEFAULT_PARAMETERS]);
    let tmp1 = h.parse("function f(/* number */ a = 0) {}");
    let mut function_node = tmp1.get_first_child(&h.ast);
    let mut parameter_list = function_node.unwrap().get_second_child(&h.ast);
    let mut default_value = parameter_list.unwrap().get_first_child(&h.ast);
    assert_eq!(
        default_value.unwrap().get_token(&h.ast),
        Token::DEFAULT_VALUE
    );
    let mut a_name = default_value.unwrap().get_first_child(&h.ast);
    assert_eq!(a_name.unwrap().get_token(&h.ast), Token::NAME);
    assert!(
        a_name
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("/* number */")
            >= 0
    );
}

// port: ParserTest#testRestParameters
#[test]
fn test_rest_parameters() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("(...xs, x) => xs", &["')' expected"]);
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "function f(...a[0]) {}",
        &["Only an identifier or destructuring pattern is allowed here."],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "function f(...y, z) {}",
        &["A rest parameter must be last in a parameter list."],
    );
    let _ = tmp3;
    h.expect_features(&[Feature::REST_PARAMETERS]);
    let tmp4 = h.parse("function f(...b) {}");
    let _ = tmp4;
    let tmp5 = h.parse("(...xs) => xs");
    let _ = tmp5;
    let tmp6 = h.parse("(x, ...xs) => xs");
    let _ = tmp6;
    let tmp7 = h.parse("(x, y, ...xs) => xs");
    let _ = tmp7;
}

// port: ParserTest#testTrailingCommaAfterRestParameters
#[test]
fn test_trailing_comma_after_rest_parameters() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error(
        "function f(...a,) {}",
        &["A trailing comma must not follow a rest parameter."],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error("((...xs,) => xs)", &["')' expected"]);
    let _ = tmp2;
}

// port: ParserTest#testDestructuredRestParameters
#[test]
fn test_destructured_rest_parameters() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error(
        "function f(...[a[0]]) {}",
        &["Only an identifier or destructuring pattern is allowed here."],
    );
    let _ = tmp1;
    h.expect_features(&[Feature::REST_PARAMETERS, Feature::ARRAY_DESTRUCTURING]);
    let tmp2 = h.parse("(...[x]) => xs");
    let _ = tmp2;
    let tmp3 = h.parse("(...[x, y]) => xs");
    let _ = tmp3;
    let tmp4 = h.parse("(a, b, c, ...[x, y, z]) => x");
    let _ = tmp4;
}

// port: ParserTest#testRestParameters_ES5
#[test]
fn test_rest_parameters_es5() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT5;
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::REST_PARAMETERS]);
    let tmp1 = h.parse_warning(
        "function f(...b) {}",
        &[&requires_language_mode_message(Feature::REST_PARAMETERS)],
    );
    let _ = tmp1;
}

// port: ParserTest#testExpressionsThatLookLikeParameters1
#[test]
fn test_expressions_that_look_like_parameters1() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("();", &["invalid parenthesized expression"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("(...xs);", &["invalid parenthesized expression"]);
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "(x, ...xs);",
        &["A rest parameter must be in a parameter list."],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "(a, b, c, ...xs);",
        &["A rest parameter must be in a parameter list."],
    );
    let _ = tmp4;
}

// port: ParserTest#testExpressionsThatLookLikeParameters2
#[test]
fn test_expressions_that_look_like_parameters2() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("!()", &["invalid parenthesized expression"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("().method", &["invalid parenthesized expression"]);
    let _ = tmp2;
    let tmp3 = h.parse_error("() || a", &["invalid parenthesized expression"]);
    let _ = tmp3;
    let tmp4 = h.parse_error("() && a", &["invalid parenthesized expression"]);
    let _ = tmp4;
    let tmp5 = h.parse_error("x = ()", &["invalid parenthesized expression"]);
    let _ = tmp5;
}

// port: ParserTest#testExpressionsThatLookLikeParameters3
#[test]
fn test_expressions_that_look_like_parameters3() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("!(...x)", &["invalid parenthesized expression"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("(...x).method", &["invalid parenthesized expression"]);
    let _ = tmp2;
    let tmp3 = h.parse_error("(...x) || a", &["invalid parenthesized expression"]);
    let _ = tmp3;
    let tmp4 = h.parse_error("(...x) && a", &["invalid parenthesized expression"]);
    let _ = tmp4;
    let tmp5 = h.parse_error("x = (...x)", &["invalid parenthesized expression"]);
    let _ = tmp5;
}

// port: ParserTest#testDefaultParametersWithRestParameters
#[test]
fn test_default_parameters_with_rest_parameters() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error(
        "function f(a=1, ...b=3) {}",
        &["A default value cannot be specified after '...'"],
    );
    let _ = tmp1;
    h.expect_features(&[Feature::DEFAULT_PARAMETERS, Feature::REST_PARAMETERS]);
    let tmp2 = h.parse("function f(a=0, ...b) {}");
    let _ = tmp2;
    let tmp3 = h.parse("function f(a, b=0, ...c) {}");
    let _ = tmp3;
    let tmp4 = h.parse("function f(a, b=0, c=1, ...d) {}");
    let _ = tmp4;
}

// port: ParserTest#testClass1
#[test]
fn test_class1() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::CLASSES]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("class C {}");
    let _ = tmp1;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp2 = h.parse_warning(
        "class C {}",
        &[&requires_language_mode_message(Feature::CLASSES)],
    );
    let _ = tmp2;
    h.mode = LanguageMode::ECMASCRIPT3;
    let tmp3 = h.parse_warning(
        "class C {}",
        &[&requires_language_mode_message(Feature::CLASSES)],
    );
    let _ = tmp3;
}

// port: ParserTest#testClass2
#[test]
fn test_class2() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::CLASSES]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("class C {}");
    let _ = tmp1;
    let tmp2 = h.parse("class C {\n  member() {}\n  get prop() {}\n  set prop(a) {}\n}\n");
    let _ = tmp2;
    let tmp3 = h.parse(
        "class C {\n  static member() {}\n  static get prop() {}\n  static set prop(a) {}\n}\n",
    );
    let _ = tmp3;
}

// port: ParserTest#testClass3
#[test]
fn test_class3() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::CLASSES]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("class C {\n  member() {};\n  get prop() {};\n  set prop(a) {};\n}\n");
    let _ = tmp1;
    let tmp2 = h.parse(
        "class C {\n  static member() {};\n  static get prop() {};\n  static set prop(a) {};\n}\n",
    );
    let _ = tmp2;
}

// port: ParserTest#testClassKeywordsAsMethodNames
#[test]
fn test_class_keywords_as_method_names() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::CLASSES, Feature::KEYWORDS_AS_PROPERTIES]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("class KeywordMethods {\n  continue() {}\n  throw() {}\n  else() {}\n}\n");
    let _ = tmp1;
}

// port: ParserTest#testClassReservedWordsAsMethodNames
#[test]
fn test_class_reserved_words_as_method_names() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::CLASSES, Feature::KEYWORDS_AS_PROPERTIES]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("class C {\n  import() {};\n  get break() {};\n  set break(a) {};\n}\n");
    let _ = tmp1;
    let tmp2 = h.parse("class C {\n  static import() {};\n  static get break() {};\n  static set break(a) {};\n}\n");
    let _ = tmp2;
}

// port: ParserTest#testClass_semicolonsInBodyAreIgnored
#[test]
fn test_class_semicolons_in_body_are_ignored() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C {\n  foo() {};;;;;;\n}\n");
    let mut tree = tmp1;
    let mut members = tree
        .get_first_child(&h.ast)
        .unwrap()
        .get_child_at_index(&h.ast, 2);
    assert!(members.unwrap().is_class_members(&h.ast));
    assert_eq!(members.unwrap().get_child_count(&h.ast), 1);
}

// port: ParserTest#testClass_constructorMember_legalModifiers
#[test]
fn test_class_constructor_member_legal_modifiers() {
    let mut h = Harness::new();
    let mut error_msg =
        ("Class constructor may not be getter, setter, async, or generator.").to_string();
    let tmp1 = h.parse("class A { constructor() { } }");
    let _ = tmp1;
    let tmp2 = h.parse("class A { static constructor() { } }");
    let _ = tmp2;
    let tmp3 = h.parse("class A { static get constructor() { } }");
    let _ = tmp3;
    let tmp4 = h.parse("class A { static set constructor(x) { } }");
    let _ = tmp4;
    let tmp5 = h.parse("class A { static async constructor() { } }");
    let _ = tmp5;
    let tmp6 = h.parse("class A { static *constructor() { } }");
    let _ = tmp6;
    let tmp7 = h.parse_error("class A { get constructor() { } }", &[&error_msg]);
    let _ = tmp7;
    let tmp8 = h.parse_error("class A { set constructor(x) { } }", &[&error_msg]);
    let _ = tmp8;
    let tmp9 = h.parse_error("class A { async constructor() { } }", &[&error_msg]);
    let _ = tmp9;
    let tmp10 = h.parse_error("class A { *constructor() { } }", &[&error_msg]);
    let _ = tmp10;
    let tmp11 = h.parse("class A { 'constructor'() { } }");
    let _ = tmp11;
    let tmp12 = h.parse("class A { get 'constructor'() { } }");
    let _ = tmp12;
    let tmp13 = h.parse("class A { set 'constructor'(x) { } }");
    let _ = tmp13;
    let tmp14 = h.parse("class A { async 'constructor'() { } }");
    let _ = tmp14;
    let tmp15 = h.parse("class A { *'constructor'() { } }");
    let _ = tmp15;
    let tmp16 = h.parse("class A { ['constructor']() { } }");
    let _ = tmp16;
    let tmp17 = h.parse("class A { get ['constructor']() { } }");
    let _ = tmp17;
    let tmp18 = h.parse("class A { set ['constructor'](x) { } }");
    let _ = tmp18;
    let tmp19 = h.parse("class A { async ['constructor']() { } }");
    let _ = tmp19;
    let tmp20 = h.parse("class A { *['constructor']() { } }");
    let _ = tmp20;
}

// port: ParserTest#testClass_constructorMember_atMostOne
#[test]
fn test_class_constructor_member_at_most_one() {
    let mut h = Harness::new();
    let mut error_msg = ("Class may have only one constructor.").to_string();
    let tmp1 = h.parse("class A { }");
    let _ = tmp1;
    let tmp2 = h.parse("class A { constructor() { } }");
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "class A { constructor() { } constructor() { } }",
        &[&error_msg],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "class A { constructor() { } constructor() { } constructor() { } }",
        &[&error_msg, &error_msg],
    );
    let _ = tmp4;
    let tmp5 = h.parse("class A { constructor() { } 'constructor'() { } }");
    let _ = tmp5;
    let tmp6 = h.parse("class A { constructor() { } ['constructor']() { } }");
    let _ = tmp6;
    let tmp7 = h.parse("class A { constructor() { } static constructor() { } }");
    let _ = tmp7;
}

// port: ParserTest#testSingleFieldNoInitializerNoSemicolon
#[test]
fn test_single_field_no_initializer_no_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{ field }");
    let mut n = tmp1.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::MEMBER_FIELD_DEF
    );
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("field")
    );
}

// port: ParserTest#testSingleFieldWithInitializerNoSemicolon
#[test]
fn test_single_field_with_initializer_no_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{field = 2}");
    let mut n = tmp1
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::MEMBER_FIELD_DEF);
    let tmp2 = IR::number(&mut h.ast, 2.0_f64);
    assert_node_equality(&h.ast, tmp2, n.unwrap().get_first_child(&h.ast).unwrap());
}

// port: ParserTest#testSingleFieldNoInitializerWithSemicolon
#[test]
fn test_single_field_no_initializer_with_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{ field; }");
    let mut n = tmp1.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("field")
    );
}

// port: ParserTest#testSingleFieldWithInitializerWithSemicolon
#[test]
fn test_single_field_with_initializer_with_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{field = dog;}");
    let mut n = tmp1
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::MEMBER_FIELD_DEF);
    let tmp2 = IR::name(&mut h.ast, "dog");
    assert_node_equality(&h.ast, tmp2, n.unwrap().get_first_child(&h.ast).unwrap());
}

// port: ParserTest#testSingleComputedFieldNoInitializerNoSemicolon
#[test]
fn test_single_computed_field_no_initializer_no_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{['field']}");
    let mut n = tmp1
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::COMPUTED_FIELD_DEF);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("field")
    );
}

// port: ParserTest#testSingleComputedFieldWithInitializerNoSemicolon
#[test]
fn test_single_computed_field_with_initializer_no_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{['field'] = 2}");
    let mut n = tmp1
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::COMPUTED_FIELD_DEF);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("field")
    );
    let tmp2 = IR::number(&mut h.ast, 2.0_f64);
    assert_node_equality(&h.ast, tmp2, n.unwrap().get_last_child(&h.ast).unwrap());
}

// port: ParserTest#testSingleComputedFieldNoInitializerWithSemicolon
#[test]
fn test_single_computed_field_no_initializer_with_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{['field'];}");
    let mut n = tmp1
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::COMPUTED_FIELD_DEF);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("field")
    );
}

// port: ParserTest#testSingleComputedFieldWithInitializerWithSemicolon
#[test]
fn test_single_computed_field_with_initializer_with_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{['field'] = dog;}");
    let mut n = tmp1
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::COMPUTED_FIELD_DEF);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("field")
    );
    let tmp2 = IR::name(&mut h.ast, "dog");
    assert_node_equality(&h.ast, tmp2, n.unwrap().get_last_child(&h.ast).unwrap());
}

// port: ParserTest#testMultipleFieldsNoInitializerNoLineBreakNoSemicolon
#[test]
fn test_multiple_fields_no_initializer_no_line_break_no_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("class C{ a b c }", &["Semi-colon expected"]);
    let _ = tmp1;
}

// port: ParserTest#testMultipleFieldsNoInitializerWithLineBreakNoSemicolon
#[test]
fn test_multiple_fields_no_initializer_with_line_break_no_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{ a \n b \n c }");
    let mut n = tmp1.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::MEMBER_FIELD_DEF
    );
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("a")
    );
    assert_eq!(
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::MEMBER_FIELD_DEF
    );
    assert_eq!(
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("b")
    );
    assert_eq!(
        n.unwrap().get_last_child(&h.ast).unwrap().get_token(&h.ast),
        Token::MEMBER_FIELD_DEF
    );
    assert_eq!(
        n.unwrap()
            .get_last_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("c")
    );
}

// port: ParserTest#testMultipleFieldsNoLineBreakNoSemicolon
#[test]
fn test_multiple_fields_no_line_break_no_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("class C{ a = 2 b = 4 c = 5}", &["Semi-colon expected"]);
    let _ = tmp1;
}

// port: ParserTest#testMultipleFieldsWithLineBreakNoSemicolon
#[test]
fn test_multiple_fields_with_line_break_no_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{ a = 2 \n b = 4 \n c = 5}");
    let mut n = tmp1.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::MEMBER_FIELD_DEF
    );
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("a")
    );
    let tmp2 = IR::number(&mut h.ast, 2.0_f64);
    assert_node_equality(
        &h.ast,
        tmp2,
        n.unwrap().get_first_first_child(&h.ast).unwrap(),
    );
    assert_eq!(
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::MEMBER_FIELD_DEF
    );
    assert_eq!(
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("b")
    );
    let tmp3 = IR::number(&mut h.ast, 4.0_f64);
    assert_node_equality(
        &h.ast,
        tmp3,
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap(),
    );
    assert_eq!(
        n.unwrap().get_last_child(&h.ast).unwrap().get_token(&h.ast),
        Token::MEMBER_FIELD_DEF
    );
    assert_eq!(
        n.unwrap()
            .get_last_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("c")
    );
    let tmp4 = IR::number(&mut h.ast, 5.0_f64);
    assert_node_equality(
        &h.ast,
        tmp4,
        n.unwrap()
            .get_last_child(&h.ast)
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap(),
    );
}

// port: ParserTest#testMultipleFieldsNoLineBreakWithSemicolon
#[test]
fn test_multiple_fields_no_line_break_with_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{field = 2; hi = 3;}");
    let mut n = tmp1.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::MEMBER_FIELD_DEF
    );
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("field")
    );
    let tmp2 = IR::number(&mut h.ast, 2.0_f64);
    assert_node_equality(
        &h.ast,
        tmp2,
        n.unwrap().get_first_first_child(&h.ast).unwrap(),
    );
    assert_eq!(
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("hi")
    );
    let tmp3 = IR::number(&mut h.ast, 3.0_f64);
    assert_node_equality(
        &h.ast,
        tmp3,
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap(),
    );
}

// port: ParserTest#testMultipleFieldsWithLineBreakWithSemicolon
#[test]
fn test_multiple_fields_with_line_break_with_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{field = 2; \n hi = 3;}");
    let mut n = tmp1.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::MEMBER_FIELD_DEF
    );
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("field")
    );
    let tmp2 = IR::number(&mut h.ast, 2.0_f64);
    assert_node_equality(
        &h.ast,
        tmp2,
        n.unwrap().get_first_first_child(&h.ast).unwrap(),
    );
    assert_eq!(
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("hi")
    );
    let tmp3 = IR::number(&mut h.ast, 3.0_f64);
    assert_node_equality(
        &h.ast,
        tmp3,
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap(),
    );
}

// port: ParserTest#testMultipleComputedFieldsNoLineBreakNoSemicolon
#[test]
fn test_multiple_computed_fields_no_line_break_no_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{ ['a'] = 2 ['b'] = 4 ['c'] = 5}");
    let mut n = tmp1.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::COMPUTED_FIELD_DEF
    );
}

// port: ParserTest#testMultipleComputedFieldsWithLineBreakNoSemicolon
#[test]
fn test_multiple_computed_fields_with_line_break_no_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{ ['a'] = 2 \n ['b'] = 4 \n ['c'] = 5}");
    let mut n = tmp1.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::COMPUTED_FIELD_DEF
    );
}

// port: ParserTest#testMultipleComputedFieldsNoLineBreakWithSemicolon
#[test]
fn test_multiple_computed_fields_no_line_break_with_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{['field'] = 2; ['hi'] = 3;}");
    let mut n = tmp1.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::COMPUTED_FIELD_DEF
    );
    assert_eq!(
        n.unwrap()
            .get_first_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("field")
    );
    let tmp2 = IR::number(&mut h.ast, 2.0_f64);
    assert_node_equality(
        &h.ast,
        tmp2,
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_last_child(&h.ast)
            .unwrap(),
    );
    assert_eq!(
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::COMPUTED_FIELD_DEF
    );
    assert_eq!(
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("hi")
    );
    let tmp3 = IR::number(&mut h.ast, 3.0_f64);
    assert_node_equality(
        &h.ast,
        tmp3,
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .get_last_child(&h.ast)
            .unwrap(),
    );
}

// port: ParserTest#testMultipleComputedFieldsWithLineBreaksWithSemicolons
#[test]
fn test_multiple_computed_fields_with_line_breaks_with_semicolons() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{['field'] = 2; \n ['hi'] = 3;}");
    let mut n = tmp1.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::COMPUTED_FIELD_DEF
    );
    assert_eq!(
        n.unwrap()
            .get_first_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("field")
    );
    let tmp2 = IR::number(&mut h.ast, 2.0_f64);
    assert_node_equality(
        &h.ast,
        tmp2,
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_last_child(&h.ast)
            .unwrap(),
    );
    assert_eq!(
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::COMPUTED_FIELD_DEF
    );
    assert_eq!(
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("hi")
    );
    let tmp3 = IR::number(&mut h.ast, 3.0_f64);
    assert_node_equality(
        &h.ast,
        tmp3,
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .get_last_child(&h.ast)
            .unwrap(),
    );
}

// port: ParserTest#testMultipleMixedFieldsNoLineBreakWithSemicolon
#[test]
fn test_multiple_mixed_fields_no_line_break_with_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{  b = 4; ['a'] = 2 }");
    let mut n = tmp1.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::MEMBER_FIELD_DEF
    );
    assert_eq!(
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::COMPUTED_FIELD_DEF
    );
}

// port: ParserTest#testMultipleMixedFieldsNoLineBreakNoSemicolon
#[test]
fn test_multiple_mixed_fields_no_line_break_no_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("class C{ ['a'] = 2 b = 4}", &["Semi-colon expected"]);
    let _ = tmp1;
    let tmp2 = h.parse("class C{b = 4 ['a'] = 2 }");
    let mut n = tmp2.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::MEMBER_FIELD_DEF
    );
}

// port: ParserTest#testMultipleMixedFieldsWithLineBreakNoSemicolon
#[test]
fn test_multiple_mixed_fields_with_line_break_no_semicolon() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{b = 4 \n ['a'] = 2 }");
    let mut n = tmp1.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::MEMBER_FIELD_DEF
    );
}

// port: ParserTest#testComputedFieldStringLit
#[test]
fn test_computed_field_string_lit() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C {'x' = 2;}");
    let mut n = tmp1
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::COMPUTED_FIELD_DEF);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::STRINGLIT
    );
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("x")
    );
    assert_eq!(
        n.unwrap().get_last_child(&h.ast).unwrap().get_token(&h.ast),
        Token::NUMBER
    );
    let tmp2 = IR::number(&mut h.ast, 2.0_f64);
    assert_node_equality(&h.ast, tmp2, n.unwrap().get_last_child(&h.ast).unwrap());
}

// port: ParserTest#testComputedFieldName
#[test]
fn test_computed_field_name() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C {[a] = 2;}");
    let mut n = tmp1
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::COMPUTED_FIELD_DEF);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::NAME
    );
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("a")
    );
    assert_eq!(
        n.unwrap().get_last_child(&h.ast).unwrap().get_token(&h.ast),
        Token::NUMBER
    );
    let tmp2 = IR::number(&mut h.ast, 2.0_f64);
    assert_node_equality(&h.ast, tmp2, n.unwrap().get_last_child(&h.ast).unwrap());
}

// port: ParserTest#testComputedFieldNumber
#[test]
fn test_computed_field_number() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{1 = 2;}");
    let mut n = tmp1
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::COMPUTED_FIELD_DEF);
    assert_eq!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::NUMBER
    );
    let tmp2 = IR::number(&mut h.ast, 1.0_f64);
    assert_node_equality(&h.ast, tmp2, n.unwrap().get_first_child(&h.ast).unwrap());
    assert_eq!(
        n.unwrap().get_last_child(&h.ast).unwrap().get_token(&h.ast),
        Token::NUMBER
    );
    let tmp3 = IR::number(&mut h.ast, 2.0_f64);
    assert_node_equality(&h.ast, tmp3, n.unwrap().get_last_child(&h.ast).unwrap());
}

// port: ParserTest#testStaticClassFields
#[test]
fn test_static_class_fields() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{static field = 2;}");
    let mut n = tmp1.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .is_static_member(&h.ast)
    );
    let tmp2 = h.parse("class C{static field = 2; hi = 3;}");
    n = tmp2.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .is_static_member(&h.ast)
    );
    let tmp3 = h.parse("class C{static field = 2; static hi = 3;}");
    n = tmp3.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .is_static_member(&h.ast)
    );
    assert!(
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .is_static_member(&h.ast)
    );
}

// port: ParserTest#testStaticClassComputedFields
#[test]
fn test_static_class_computed_fields() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C{static ['field'] = 2;}");
    let mut n = tmp1.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .is_static_member(&h.ast)
    );
    let tmp2 = h.parse("class C{static ['field'] = 2; ['hi'] = 3;}");
    n = tmp2.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .is_static_member(&h.ast)
    );
    let tmp3 = h.parse("class C{static ['field'] = 2; static ['hi'] = 3;}");
    n = tmp3.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .is_static_member(&h.ast)
    );
    assert!(
        n.unwrap()
            .get_second_child(&h.ast)
            .unwrap()
            .is_static_member(&h.ast)
    );
    let tmp4 = h.parse("class C{static 'hi' = 2;}");
    n = tmp4.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .is_static_member(&h.ast)
    );
    let tmp5 = h.parse("class C{static 1 = 2;}");
    n = tmp5.get_first_child(&h.ast).unwrap().get_last_child(&h.ast);
    assert!(
        n.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .is_static_member(&h.ast)
    );
}

// port: ParserTest#testClassField_es2020
#[test]
fn test_class_field_es2020() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT_2020;
    h.expect_features(&[Feature::PUBLIC_CLASS_FIELDS]);
    let tmp1 = h.parse_warning(
        "class C{field = 2;}",
        &[&requires_language_mode_message(
            Feature::PUBLIC_CLASS_FIELDS,
        )],
    );
    let _ = tmp1;
}

// port: ParserTest#testClassComputedField_es2020
#[test]
fn test_class_computed_field_es2020() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT_2020;
    h.expect_features(&[Feature::PUBLIC_CLASS_FIELDS]);
    let tmp1 = h.parse_warning(
        "class C{['a'] = 2;}",
        &[&requires_language_mode_message(
            Feature::PUBLIC_CLASS_FIELDS,
        )],
    );
    let _ = tmp1;
}

// port: ParserTest#testClassComputedField_es2020_noWarningOrRecordingOfFeaturesInClosureUnawareCode
#[test]
fn test_class_computed_field_es2020_no_warning_or_recording_of_features_in_closure_unaware_code() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT_2020;
    h.expect_features(&[Feature::PUBLIC_CLASS_FIELDS]);
    let tmp1 = h.parse_warning("/** @fileoverview @closureUnaware */\n/** @closureUnaware */ (\n  function() { class C{ a = 2;} }).call(globalThis);\n", &["@closureUnaware annotation is not allowed in this compilation", "@closureUnaware annotation is not allowed in this compilation"]);
    let _ = tmp1;
    h.expect_features(&[Feature::CLASSES]);
}

// port: ParserTest#testClassComputedField_es2020_warnsForCodeOutsideClosureUnawareRange
#[test]
fn test_class_computed_field_es2020_warns_for_code_outside_closure_unaware_range() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT_2020;
    h.expect_features(&[Feature::PUBLIC_CLASS_FIELDS]);
    let tmp1 = h.parse_warning("/** @fileoverview @closureUnaware */\n/** @closureUnaware */ (\n  function() {\n    class C{ a = 2;}\n}).call(globalThis);\nclass C{\n  a = 2;\n}\n", &["@closureUnaware annotation is not allowed in this compilation", "This language feature is only supported for ECMASCRIPT_2022 mode or better: Public class fields", "@closureUnaware annotation is not allowed in this compilation"]);
    let _ = tmp1;
    h.expect_features(&[Feature::CLASSES, Feature::PUBLIC_CLASS_FIELDS]);
}

// port: ParserTest#testPrivateProperty_unstable
#[test]
fn test_private_property_unstable() {
    let mut h = Harness::new();
    h.mode = LanguageMode::UNSTABLE;
    h.expect_features(&[Feature::PRIVATE_ELEMENTS]);
    let tmp1 = h.parse_warning(
        "class C { #f = 2; }",
        &[&requires_language_mode_message(Feature::PRIVATE_ELEMENTS)],
    );
    let _ = tmp1;
}

// port: ParserTest#testPrivateProperty_singleClassMember
#[test]
fn test_private_property_single_class_member() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::PRIVATE_ELEMENTS]);
    let tmp1 = h.parse("class C { #f; }");
    let _ = tmp1;
    let tmp2 = h.parse("class C { #m() {} }");
    let _ = tmp2;
    let tmp3 = h.parse("class C { *#g() {} }");
    let _ = tmp3;
    let tmp4 = h.parse("class C { get #g() {} }");
    let _ = tmp4;
    let tmp5 = h.parse("class C { set #s(x) {} }");
    let _ = tmp5;
    let tmp6 = h.parse("class C { get #p() {} set #p(x) {} }");
    let _ = tmp6;
    let tmp7 = h.parse("class C { async #a() {} }");
    let _ = tmp7;
    let tmp8 = h.parse("class C { async *#ag() {} }");
    let _ = tmp8;
    let tmp9 = h.parse("class C { static #sf; }");
    let _ = tmp9;
    let tmp10 = h.parse("class C { static #sm() {} }");
    let _ = tmp10;
    let tmp11 = h.parse("class C { static *#sg() {} }");
    let _ = tmp11;
    let tmp12 = h.parse("class C { static get #sg() {} }");
    let _ = tmp12;
    let tmp13 = h.parse("class C { static set #ss(x) {} }");
    let _ = tmp13;
    let tmp14 = h.parse("class C { static get #sp() {} static set #sp(x) {} }");
    let _ = tmp14;
    let tmp15 = h.parse("class C { static async #sa() {} }");
    let _ = tmp15;
    let tmp16 = h.parse("class C { static async *#sag() {} }");
    let _ = tmp16;
}

// port: ParserTest#testPrivateProperty_duplicateIdentifierOnInstanceAndStatic
#[test]
fn test_private_property_duplicate_identifier_on_instance_and_static() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::PRIVATE_ELEMENTS]);
    let tmp1 = h.parse_error(
        "class C { #x; static #x; }",
        &["Identifier '#x' has already been declared"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error("class C { #x; static [#x]; }", &[INVALID_PRIVATE_ID]);
    let _ = tmp2;
}

// port: ParserTest#testPrivateProperty_definition_linenocharno
#[test]
fn test_private_property_definition_linenocharno() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C {\n  #pf = 1;\n  #pm() {}\n}\n");
    let mut n = tmp1.get_first_child(&h.ast);
    let mut members = get_class_members(&h.ast, n.unwrap());
    let mut private_field = members.get_first_child(&h.ast);
    assert_eq!(private_field.unwrap().get_lineno(&h.ast), 2);
    assert_eq!(private_field.unwrap().get_charno(&h.ast), 2);
    assert_eq!(private_field.unwrap().get_length(&h.ast), 8);
    let mut private_method = members.get_last_child(&h.ast);
    assert_eq!(private_method.unwrap().get_lineno(&h.ast), 3);
    assert_eq!(private_method.unwrap().get_charno(&h.ast), 2);
    assert_eq!(private_method.unwrap().get_length(&h.ast), 3);
}

// port: ParserTest#testPrivateProperty_multipleClassMembers
#[test]
fn test_private_property_multiple_class_members() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C { #f; #g; }");
    let _ = tmp1;
    let tmp2 = h.parse("class C { #m() {} #n() {} }");
    let _ = tmp2;
    let tmp3 = h.parse("class C { get #g() {} get #h() {} }");
    let _ = tmp3;
    let tmp4 = h.parse("class C { set #s(x) {} set #t(x) {} }");
    let _ = tmp4;
    let tmp5 = h.parse("class C { get #s() {} set #s(x) {} get #t() {} set #t(x) {} }");
    let _ = tmp5;
    let tmp6 = h.parse("class C { static #sf; static #sg; }");
    let _ = tmp6;
    let tmp7 = h.parse("class C { static #sm() {} static #sn() {} }");
    let _ = tmp7;
    let tmp8 = h.parse("class C { static get #sg() {} static get #sh() {} }");
    let _ = tmp8;
    let tmp9 = h.parse("class C { static set #ss(x) {} static set #st(x) {} }");
    let _ = tmp9;
    let tmp10 = h.parse("class C { static get #ss() {} static set #ss(x) {}\nstatic get #st() {} static set #st(x) {} }\n");
    let _ = tmp10;
    let tmp11 = h.parse("class C { #a; #b; c() {} d() {} get #e() {} set #e(x) {} set #f(x) {} }");
    let _ = tmp11;
    let tmp12 =
        h.parse("class C { static #a; #b; static c() {} d() {} static get #e() {} set #f(x) {} }");
    let _ = tmp12;
    let tmp13 = h.parse("class C { static #a; static #b; static c() {} static d() {}\nstatic get #e() {} static set #f(x) {} }\n");
    let _ = tmp13;
}

// port: ParserTest#testPrivateProperty_invalid_identifierAlreadyDeclared
#[test]
fn test_private_property_invalid_identifier_already_declared() {
    let mut h = Harness::new();
    let mut expected_error = ("Identifier '#p' has already been declared").to_string();
    let tmp1 = h.parse_error("class C { #p; #p; }", &[&expected_error]);
    let _ = tmp1;
    let tmp2 = h.parse_error("class C { #p() {} #p() {} }", &[&expected_error]);
    let _ = tmp2;
    let tmp3 = h.parse_error("class C { get #p() {} get #p() {} }", &[&expected_error]);
    let _ = tmp3;
    let tmp4 = h.parse_error("class C { set #p(x) {} set #p(x) {} }", &[&expected_error]);
    let _ = tmp4;
    let tmp5 = h.parse_error("class C { static #p; static #p; }", &[&expected_error]);
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "class C { static #p() {} static #p() {} }",
        &[&expected_error],
    );
    let _ = tmp6;
    let tmp7 = h.parse_error(
        "class C { static get #p() {} static get #p() {} }",
        &[&expected_error],
    );
    let _ = tmp7;
    let tmp8 = h.parse_error(
        "class C { static set #p(x) {} static set #p(x) {} }",
        &[&expected_error],
    );
    let _ = tmp8;
    let tmp9 = h.parse_error("class C { #p; #p() {} }", &[&expected_error]);
    let _ = tmp9;
    let tmp10 = h.parse_error("class C { #p; get #p() {} }", &[&expected_error]);
    let _ = tmp10;
    let tmp11 = h.parse_error("class C { #p; set #p(x) {} }", &[&expected_error]);
    let _ = tmp11;
    let tmp12 = h.parse_error("class C { #p() {} #p; }", &[&expected_error]);
    let _ = tmp12;
    let tmp13 = h.parse_error("class C { #p() {} get #p() {} }", &[&expected_error]);
    let _ = tmp13;
    let tmp14 = h.parse_error("class C { #p() {} set #p(x) {} }", &[&expected_error]);
    let _ = tmp14;
    let tmp15 = h.parse_error("class C { get #p() {} #p; }", &[&expected_error]);
    let _ = tmp15;
    let tmp16 = h.parse_error("class C { get #p() {} #p() {} }", &[&expected_error]);
    let _ = tmp16;
    let tmp17 = h.parse("class C { get #p() {} set #p(x) {} }");
    let _ = tmp17;
    let tmp18 = h.parse_error("class C { set #p(x) {} #p; }", &[&expected_error]);
    let _ = tmp18;
    let tmp19 = h.parse_error("class C { set #p(x) {} #p() {} }", &[&expected_error]);
    let _ = tmp19;
    let tmp20 = h.parse("class C { set #p(x) {} get #p() {} }");
    let _ = tmp20;
    let tmp21 = h.parse_error("class C { #p; static #p; }", &[&expected_error]);
    let _ = tmp21;
    let tmp22 = h.parse_error("class C { #p() {} static #p() {} }", &[&expected_error]);
    let _ = tmp22;
    let tmp23 = h.parse_error(
        "class C { get #p() {} static get #p() {} }",
        &[&expected_error],
    );
    let _ = tmp23;
    let tmp24 = h.parse_error(
        "class C { set #p(x) {} static set #p(x) {} }",
        &[&expected_error],
    );
    let _ = tmp24;
}

// port: ParserTest#testPrivateProperty_invalid_identifierAlreadyDeclared_crossStaticGetterSetter
#[test]
fn test_private_property_invalid_identifier_already_declared_cross_static_getter_setter() {
    let mut h = Harness::new();
    let mut expected_error = ("Identifier '#p' has already been declared").to_string();
    let tmp1 = h.parse_error(
        "class C { get #p() {} static set #p(x) {} }",
        &[&expected_error],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "class C { set #p(x) {} static get #p() {} }",
        &[&expected_error],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "class C { static get #p() {} set #p(x) {} }",
        &[&expected_error],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "class C { static set #p(x) {} get #p() {} }",
        &[&expected_error],
    );
    let _ = tmp4;
}

// port: ParserTest#testPrivateProperty_classMembersReferencingOtherPrivateField
#[test]
fn test_private_property_class_members_referencing_other_private_field() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::PRIVATE_ELEMENTS]);
    let tmp1 = h.parse("class C { #f = 1; f = this.#f; }");
    let _ = tmp1;
    let tmp2 = h.parse("class C { static #sf = 1; static sf = this.#sf; }");
    let _ = tmp2;
    let tmp3 = h.parse("class C { static #sf = 1; static sf = C.#sf; }");
    let _ = tmp3;
    let tmp4 = h.parse("class C { static #sf; static { this.#sf = 1; } }");
    let _ = tmp4;
    let tmp5 = h.parse("class C { static #sf; static { C.#sf = 1; } }");
    let _ = tmp5;
}

// port: ParserTest#testPrivateProperty_reference_linenocharno
#[test]
fn test_private_property_reference_linenocharno() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C {\n  #pf1 = 1;\n  #pf2 = this.#pf1;\n}\n");
    let mut n = tmp1.get_first_child(&h.ast);
    let mut members = get_class_members(&h.ast, n.unwrap());
    let mut field2 = members.get_last_child(&h.ast);
    assert_eq!(field2.unwrap().get_token(&h.ast), Token::MEMBER_FIELD_DEF);
    let mut field2_get_prop = field2.unwrap().get_first_child(&h.ast);
    assert_eq!(field2_get_prop.unwrap().get_token(&h.ast), Token::GETPROP);
    assert_eq!(
        field2_get_prop.unwrap().get_string(&h.ast),
        JsString::from("#pf1")
    );
    assert_eq!(field2_get_prop.unwrap().get_lineno(&h.ast), 3);
    assert_eq!(field2_get_prop.unwrap().get_charno(&h.ast), 14);
    assert_eq!(field2_get_prop.unwrap().get_length(&h.ast), 4);
}

// port: ParserTest#testPrivateProperty_classMethodsReferencingOtherPrivateProp
#[test]
fn test_private_property_class_methods_referencing_other_private_prop() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::PRIVATE_ELEMENTS]);
    let tmp1 = h.parse("class C { #f = 1; method() { this.#f = 2; } }");
    let _ = tmp1;
    let tmp2 = h.parse("class C { #f = 1; #g = 2; method() { this.#f = this.#g; } }");
    let _ = tmp2;
    let tmp3 = h.parse("class C { #f = 1; method() { const t = this; t.#f = 2; } }");
    let _ = tmp3;
    let tmp4 = h.parse("class C { #f = 1; method() { const f = () => { this.#f = 2; }; } }");
    let _ = tmp4;
    let tmp5 = h.parse("class C { #f = 1; method() { const x = [this.#f]; } }");
    let _ = tmp5;
    let tmp6 = h.parse("class C { #f = 1; method() { const x = {y: this.#f}; } }");
    let _ = tmp6;
    let tmp7 = h.parse("class C { #pm() { this.#pm(); } }");
    let _ = tmp7;
    let tmp8 = h.parse("class C { static #f = 1; static method() { this.#f = 2; } }");
    let _ = tmp8;
    let tmp9 = h.parse("class C { static #f = 1; static method() { C.#f = 2; } }");
    let _ = tmp9;
    let tmp10 = h.parse("class C { static #f = 1; static method() { const x = [this.#f]; } }");
    let _ = tmp10;
    let tmp11 = h.parse("class C { static #f = 1; static method() { const x = {y: C.#f}; } }");
    let _ = tmp11;
    let tmp12 = h.parse("class C { static #pm() { this.#pm(); } }");
    let _ = tmp12;
    let tmp13 = h.parse("class C { static #pm() { C.#pm(); } }");
    let _ = tmp13;
}

// port: ParserTest#testPrivateProperty_nestedClasses
#[test]
fn test_private_property_nested_classes() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::PRIVATE_ELEMENTS]);
    let tmp1 = h.parse("class Outer {\n  #of1;\n  #of2;\n  om() {\n    this.#of1;\n    this.#of2;\n    class Inner {\n      #if1;\n      #if2;\n      im() {\n        this.#of1;\n        this.#of2;\n        this.#if1;\n        this.#if2;\n      }\n    }\n  }\n}\n");
    let _ = tmp1;
}

// port: ParserTest#testPrivateProperty_nestedClasses_invalid_referenceInnerFieldFromOuter
#[test]
fn test_private_property_nested_classes_invalid_reference_inner_field_from_outer() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("class Outer {\n  #of1;\n  #of2;\n  om() {\n    this.#of1;\n    this.#of2;\n    this.#if1; // Invalid\n    this.#if2; // Invalid\n    class Inner {\n      #if1;\n      #if2;\n      im() {\n        this.#of1;\n        this.#of2;\n        this.#if1;\n        this.#if2;\n      }\n    }\n  }\n}\n", &[PRIVATE_FIELD_NOT_DEFINED, PRIVATE_FIELD_NOT_DEFINED]);
    let _ = tmp1;
}

// port: ParserTest#testPrivateProperty_classMethodsReferencingOtherPrivatePropViaOptionalChain
#[test]
fn test_private_property_class_methods_referencing_other_private_prop_via_optional_chain() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::PRIVATE_ELEMENTS]);
    let tmp1 = h.parse("class C { #f = 1; method() { const t = this; t?.#f; } }");
    let _ = tmp1;
    let tmp2 = h.parse("class C { #pm() { const t = this; t?.#pm(); } }");
    let _ = tmp2;
}

// port: ParserTest#testPrivateProperty_destructuredAssignmentDefaultValue
#[test]
fn test_private_property_destructured_assignment_default_value() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C { #px = 1; method(x) { const { y = this.#px } = x; } }");
    let _ = tmp1;
    let tmp2 = h.parse("class C { #px = 1; method(x) { const { y: z = this.#px } = x; } }");
    let _ = tmp2;
}

// port: ParserTest#testPrivateProperty_invalid_nonExistentPrivateProp
#[test]
fn test_private_property_invalid_non_existent_private_prop() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "class C { #f = this.#missing; }",
        &[PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "class C { method() { this.#missing = 1; } }",
        &[PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "class C { #f = 1; method() { this.#f = this.#missing; } }",
        &[PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "class C { method() { const t = this; t.#missing; } }",
        &[PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp4;
    let tmp5 = h.parse_error(
        "class C { method() { const t = this; t?.#missing; } }",
        &[PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "class C { method() { const f = () => { this.#missing; }; } }",
        &[PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp6;
    let tmp7 = h.parse_error(
        "class C { method() { class D { method() { this.#missing = 1; } } } }",
        &[PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp7;
    let tmp8 = h.parse_error(
        "class C { method() { class D { method() { const x = this.#missing; } } } }",
        &[PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp8;
    let tmp9 = h.parse_error(
        "class C { method() { this.#missing(); } }",
        &[PRIVATE_METHOD_NOT_DEFINED],
    );
    let _ = tmp9;
    let tmp10 = h.parse_error(
        "class C { static #f = this.#missing; }",
        &[PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp10;
    let tmp11 = h.parse_error(
        "class C { static #f = C.#missing; }",
        &[PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp11;
    let tmp12 = h.parse_error(
        "class C { static { this.#missing; } }",
        &[PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp12;
    let tmp13 = h.parse_error(
        "class C { static { C.#missing; } }",
        &[PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp13;
    let tmp14 = h.parse_error(
        "class C { static { this.#missing(); } }",
        &[PRIVATE_METHOD_NOT_DEFINED],
    );
    let _ = tmp14;
    let tmp15 = h.parse_error(
        "class C { static { C.#missing(); } }",
        &[PRIVATE_METHOD_NOT_DEFINED],
    );
    let _ = tmp15;
}

// port: ParserTest#testPrivateProperty_invalid_subclassAccessingBasePrivateField
#[test]
fn test_private_property_invalid_subclass_accessing_base_private_field() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("class Base {\n  #baseField = 1;\n}\nclass Sub extends Base {\n  constructor() {\n    super();\n    this.#baseField = 2;\n  }\n}\n", &[PRIVATE_FIELD_NOT_DEFINED]);
    let _ = tmp1;
}

// port: ParserTest#testPrivateProperty_invalid_deletePrivateField
#[test]
fn test_private_property_invalid_delete_private_field() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "class C { #f = 1; method() { delete this.#f; } }",
        &[PRIVATE_FIELD_DELETED],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "class C { #f = 1; method() { const t = this; delete t.#f; } }",
        &[PRIVATE_FIELD_DELETED],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "class C { #f = 1; method() { const t = this; delete t?.#f; } }",
        &[PRIVATE_FIELD_DELETED],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "class C { #f = 1; method() { const a = {b: this}; delete ((a.b).#f); } }",
        &[PRIVATE_FIELD_DELETED],
    );
    let _ = tmp4;
    let tmp5 = h.parse_error(
        "class C { static #f = 1; static method() { delete this.#f; } }",
        &[PRIVATE_FIELD_DELETED],
    );
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "class C { static #f = 1; static method() { delete C.#f; } }",
        &[PRIVATE_FIELD_DELETED],
    );
    let _ = tmp6;
    let tmp7 = h.parse_error(
        "class C { static #f = 1; static { delete this.#f; } }",
        &[PRIVATE_FIELD_DELETED],
    );
    let _ = tmp7;
    let tmp8 = h.parse_error(
        "class C { static #f = 1; static { delete C.#f; } }",
        &[PRIVATE_FIELD_DELETED],
    );
    let _ = tmp8;
}

// port: ParserTest#testPrivateProperty_invalid_deleteUndeclaredPrivateField
#[test]
fn test_private_property_invalid_delete_undeclared_private_field() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "class C { method() { delete this.#f; } }",
        &[PRIVATE_FIELD_DELETED, PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "class C { method() { const t = this; delete t.#f; } }",
        &[PRIVATE_FIELD_DELETED, PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "class C { method() { const t = this; delete t?.#f; } }",
        &[PRIVATE_FIELD_DELETED, PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "class C { method() { const a = {b: this}; delete ((a.b).#f); } }",
        &[PRIVATE_FIELD_DELETED, PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp4;
    let tmp5 = h.parse_error(
        "class C { static method() { delete this.#f; } }",
        &[PRIVATE_FIELD_DELETED, PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "class C { static method() { delete C.#f; } }",
        &[PRIVATE_FIELD_DELETED, PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp6;
    let tmp7 = h.parse_error(
        "class C { static { delete this.#f; } }",
        &[PRIVATE_FIELD_DELETED, PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp7;
    let tmp8 = h.parse_error(
        "class C { static { delete C.#f; } }",
        &[PRIVATE_FIELD_DELETED, PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp8;
}

// port: ParserTest#testPrivateProperty_invalid_objectLiteralProperty
#[test]
fn test_private_property_invalid_object_literal_property() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("const x = { #pf: 1 }", &[INVALID_PRIVATE_ID]);
    let _ = tmp1;
    let tmp2 = h.parse_error("const x = { #pm() {} }", &[INVALID_PRIVATE_ID]);
    let _ = tmp2;
    let tmp3 = h.parse_error("const x = { get #pp() {} }", &[INVALID_PRIVATE_ID]);
    let _ = tmp3;
    let tmp4 = h.parse_error("const x = { set #ps(x) {} }", &[INVALID_PRIVATE_ID]);
    let _ = tmp4;
    let tmp5 = h.parse_error(
        "class C { method() { const x = { #pf: 1 }; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "class C { method() { const x = { #pm() {} }; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp6;
    let tmp7 = h.parse_error(
        "class C { method() { const x = { get #pp() {} }; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp7;
    let tmp8 = h.parse_error(
        "class C { method() { const x = { set #ps(x) {} }; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp8;
}

// port: ParserTest#testPrivateProperty_invalid_referencePrivateOnSuper
#[test]
fn test_private_property_invalid_reference_private_on_super() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "class C { #f = 1; m() { return super.#f; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "class C { #f = 1; m(v) { super.#f = v; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "class C { #f = 1; m() { super.#f += 1; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "class C { #f = 1; m() { super.#f++; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp4;
    let tmp5 = h.parse_error(
        "class C { #f = 1; m() { ++super.#f; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "class C { #m() {} test() { super.#m(); } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp6;
    let tmp7 = h.parse_error(
        "class C { get #g() {} m() { return super.#g; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp7;
    let tmp8 = h.parse_error(
        "class C { set #s(v) {} m(v) { super.#s = v; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp8;
    let tmp9 = h.parse_error(
        "class C { #f = 1; m() { super?.#f; } }",
        &["Optional chaining is forbidden in super?."],
    );
    let _ = tmp9;
    let tmp10 = h.parse_error(
        "class C { #f = 1; m() { [super.#f] = [1]; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp10;
    let tmp11 = h.parse_error(
        "class C { #f = 1; m() { ({ x: super.#f } = { x: 1 }); } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp11;
    let tmp12 = h.parse_error("class Base {\n  #priv() {}\n  #field = 1;\n  static {\n    class Sub extends Base {\n      m() {\n        return super.#priv();\n      }\n      getF() {\n        return super.#field;\n      }\n    }\n  }\n}\n", &[INVALID_PRIVATE_ID, INVALID_PRIVATE_ID]);
    let _ = tmp12;
}

// port: ParserTest#testPrivateProperty_invalid_destructuredAssignment
#[test]
fn test_private_property_invalid_destructured_assignment() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("const { #px } = x;", &[INVALID_PRIVATE_ID]);
    let _ = tmp1;
    let tmp2 = h.parse_error("const { x: #px } = x;", &[INVALID_PRIVATE_ID]);
    let _ = tmp2;
    let tmp3 = h.parse_error("const { x = #px } = x;", &[INVALID_PRIVATE_ID]);
    let _ = tmp3;
    let tmp4 = h.parse_error("const { x: y = #px } = x;", &[INVALID_PRIVATE_ID]);
    let _ = tmp4;
    let tmp5 = h.parse_error(
        "class C { method() { const { #px } = x; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "class C { method() { const { x: #px } = x; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp6;
    let tmp7 = h.parse_error(
        "class C { method() { const { x = #px } = x; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp7;
    let tmp8 = h.parse_error(
        "class C { method() { const { x: y = #px } = x; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp8;
}

// port: ParserTest#testPrivateProperty_invalid_variableName
#[test]
fn test_private_property_invalid_variable_name() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("const #pv = 1;", &[INVALID_PRIVATE_ID]);
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "class C { method() { const #pv = 1; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp2;
}

// port: ParserTest#testPrivateProperty_invalid_functionName
#[test]
fn test_private_property_invalid_function_name() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("function #pf() {}", &[INVALID_PRIVATE_ID]);
    let _ = tmp1;
    let tmp2 = h.parse_error("function* #pf() {}", &[INVALID_PRIVATE_ID]);
    let _ = tmp2;
    let tmp3 = h.parse_error("async function #pf() {}", &[INVALID_PRIVATE_ID]);
    let _ = tmp3;
    let tmp4 = h.parse_error("async function* #pf() {}", &[INVALID_PRIVATE_ID]);
    let _ = tmp4;
    let tmp5 = h.parse_error(
        "class C { method() { function #pf() {} } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "class C { method() { function* #pf() {} } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp6;
    let tmp7 = h.parse_error(
        "class C { method() { async function #pf() {} } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp7;
    let tmp8 = h.parse_error(
        "class C { method() { async function* #pf() {} } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp8;
}

// port: ParserTest#testPrivateProperty_invalid_paramName
#[test]
fn test_private_property_invalid_param_name() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("function f(#p) {}", &[INVALID_PRIVATE_ID]);
    let _ = tmp1;
    let tmp2 = h.parse_error("class C { method(#p) {} }", &[INVALID_PRIVATE_ID]);
    let _ = tmp2;
    let tmp3 = h.parse_error("class C { #p; method(#p) {} }", &[INVALID_PRIVATE_ID]);
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "class C { method() { function f(#p) {} } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp4;
}

// port: ParserTest#testPrivateProperty_invalid_className
#[test]
fn test_private_property_invalid_class_name() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("class #PC {}", &[INVALID_PRIVATE_ID]);
    let _ = tmp1;
    let tmp2 = h.parse_error("class C extends #PSC {}", &[INVALID_PRIVATE_ID]);
    let _ = tmp2;
}

// port: ParserTest#testPrivateProperty_invalid_referencePrivateOutsideClass
#[test]
fn test_private_property_invalid_reference_private_outside_class() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "class C { #f = 1; } const c = new C(); c.#f;",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp1;
}

// port: ParserTest#testPrivateProperty_invalid_referencePrivateOutsideClassViaOptionalChain
#[test]
fn test_private_property_invalid_reference_private_outside_class_via_optional_chain() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "class C { #f = 1; } const c = new C(); c?.#f;",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp1;
}

// port: ParserTest#testPrivateProperty_invalid_referencePrivatePropFromObjectLiteral
#[test]
fn test_private_property_invalid_reference_private_prop_from_object_literal() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("const o = {}; o.#f = 1;", &[INVALID_PRIVATE_ID]);
    let _ = tmp1;
}

// port: ParserTest#testPrivateProperty_invalid_referencePrivatePropFromObjectLiteralViaOptionalChain
#[test]
fn test_private_property_invalid_reference_private_prop_from_object_literal_via_optional_chain() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("const o = {}; o?.#f;", &[INVALID_PRIVATE_ID]);
    let _ = tmp1;
}

// port: ParserTest#testPrivateProperty_invalid_importName
#[test]
fn test_private_property_invalid_import_name() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("import #pi from './someModule'", &[INVALID_PRIVATE_ID]);
    let _ = tmp1;
    let tmp2 = h.parse_error("import {#pi} from './someModule'", &[INVALID_PRIVATE_ID]);
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "import {x as #pi} from './someModule'",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error("import * as #pi from './someModule'", &[INVALID_PRIVATE_ID]);
    let _ = tmp4;
}

// port: ParserTest#testPrivateProperty_invalid_exportName
#[test]
fn test_private_property_invalid_export_name() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("export const #px = 1", &[INVALID_PRIVATE_ID]);
    let _ = tmp1;
    let tmp2 = h.parse_error("export var #px = 1", &[INVALID_PRIVATE_ID]);
    let _ = tmp2;
    let tmp3 = h.parse_error("export function #pf() {}", &[INVALID_PRIVATE_ID]);
    let _ = tmp3;
    let tmp4 = h.parse_error("export class #pc {}", &[INVALID_PRIVATE_ID]);
    let _ = tmp4;
    let tmp5 = h.parse_error("export {#px}", &[INVALID_PRIVATE_ID]);
    let _ = tmp5;
    let tmp6 = h.parse_error("export {x as #px}", &[INVALID_PRIVATE_ID]);
    let _ = tmp6;
    let tmp7 = h.parse_error("export {#px as default}", &[INVALID_PRIVATE_ID]);
    let _ = tmp7;
    let tmp8 = h.parse_error("export {#y as class}", &[INVALID_PRIVATE_ID]);
    let _ = tmp8;
    let tmp9 = h.parse_error(
        "export {x as #px} from './someModule'",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp9;
    let tmp10 = h.parse_error(
        "export {default as #pd} from './someModule'",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp10;
    let tmp11 = h.parse_error(
        "export {#px as default} from './someModule'",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp11;
    let tmp12 = h.parse_error(
        "export {#pc as class} from './someModule'",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp12;
    let tmp13 = h.parse_error("export {#px} from './someModule'", &[INVALID_PRIVATE_ID]);
    let _ = tmp13;
}

// port: ParserTest#testPrivateProperty_invalid_labeledStatement
#[test]
fn test_private_property_invalid_labeled_statement() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("#pl: while (true) {}", &[INVALID_PRIVATE_ID]);
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "while (true) { break #pl; }",
        &[INVALID_PRIVATE_ID, "undefined label \"#pl\""],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "while (true) { continue #pl; }",
        &[INVALID_PRIVATE_ID, "undefined label \"#pl\""],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "class C { method() { #pl: while (true) {} } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp4;
    let tmp5 = h.parse_error(
        "class C { method() { while (true) { break #pl; } } }",
        &[INVALID_PRIVATE_ID, "undefined label \"#pl\""],
    );
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "class C { method() { while (true) { continue #pl; } } }",
        &[INVALID_PRIVATE_ID, "undefined label \"#pl\""],
    );
    let _ = tmp6;
}

// port: ParserTest#testPrivateProperty_inOperatorWithPrivateProp_valid
#[test]
fn test_private_property_in_operator_with_private_prop_valid() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::PRIVATE_ELEMENTS]);
    let tmp1 = h.parse("class C { #f = 1; static isC(x) { return #f in x; } }");
    let _ = tmp1;
    let tmp2 = h.parse("class C { #f = this; m() { return #f in this.#f; } }");
    let _ = tmp2;
}

// port: ParserTest#testPrivateProperty_inOperatorWithPrivateProp_linenocharno
#[test]
fn test_private_property_in_operator_with_private_prop_linenocharno() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C {\n  #f = 1;\n  static isC(x) {\n    return #f in x;\n  }\n}\n");
    let mut n = tmp1.get_first_child(&h.ast);
    let mut members = get_class_members(&h.ast, n.unwrap());
    let mut static_method = members.get_last_child(&h.ast);
    assert_eq!(
        static_method.unwrap().get_token(&h.ast),
        Token::MEMBER_FUNCTION_DEF
    );
    let mut method_block = static_method
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast);
    assert_eq!(method_block.unwrap().get_token(&h.ast), Token::BLOCK);
    let mut return_statement = method_block.unwrap().get_first_child(&h.ast);
    assert_eq!(return_statement.unwrap().get_token(&h.ast), Token::RETURN);
    let mut in_expression = return_statement.unwrap().get_first_child(&h.ast);
    assert_eq!(in_expression.unwrap().get_token(&h.ast), Token::IN);
    let mut in_expression_left = in_expression.unwrap().get_first_child(&h.ast);
    assert_eq!(in_expression_left.unwrap().get_token(&h.ast), Token::NAME);
    assert_eq!(
        in_expression_left.unwrap().get_string(&h.ast),
        JsString::from("#f")
    );
    assert_eq!(in_expression_left.unwrap().get_lineno(&h.ast), 4);
    assert_eq!(in_expression_left.unwrap().get_charno(&h.ast), 11);
    assert_eq!(in_expression_left.unwrap().get_length(&h.ast), 2);
}

// port: ParserTest#testPrivateProperty_inOperatorWithPrivateProp_invalid_nonExistentPrivateProp
#[test]
fn test_private_property_in_operator_with_private_prop_invalid_non_existent_private_prop() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::PRIVATE_ELEMENTS]);
    let tmp1 = h.parse_error(
        "class C { static isC(x) { return #missing in x; } }",
        &[PRIVATE_FIELD_NOT_DEFINED],
    );
    let _ = tmp1;
}

// port: ParserTest#testPrivateProperty_inOperatorWithPrivateProp_invalid_privatePropOnRhs
#[test]
fn test_private_property_in_operator_with_private_prop_invalid_private_prop_on_rhs() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::PRIVATE_ELEMENTS]);
    let tmp1 = h.parse_error(
        "class C { #f = 1; static isC(x) { return #f in #f; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp1;
}

// port: ParserTest#testPrivateProperty_inOperatorWithPrivateProp_invalid_surroundingParenthesis
#[test]
fn test_private_property_in_operator_with_private_prop_invalid_surrounding_parenthesis() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::PRIVATE_ELEMENTS]);
    let tmp1 = h.parse_error(
        "class C { #f = 1; static isC(x) { return (#f) in x; } }",
        &[INVALID_PRIVATE_ID],
    );
    let _ = tmp1;
}

// port: ParserTest#testPrivateProperty_inOperatorWithPrivateProp_invalid_notInClass
#[test]
fn test_private_property_in_operator_with_private_prop_invalid_not_in_class() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("const o = {}; if (#f in o) {}", &[INVALID_PRIVATE_ID]);
    let _ = tmp1;
}

// port: ParserTest#testPrivateProperty_stringKeyThatLooksLikePrivateProp
#[test]
fn test_private_property_string_key_that_looks_like_private_prop() {
    let mut h = Harness::new();
    let tmp1 = h.parse("const o = {'#notAPrivateProp': 1};");
    let _ = tmp1;
}

// port: ParserTest#testEmptyClassStaticBlock
#[test]
fn test_empty_class_static_block() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C { static { } }");
    let _ = tmp1;
    let tmp2 = h.parse("let a = class { static { } };");
    let _ = tmp2;
}

// port: ParserTest#testReturnInClassStaticBlock
#[test]
fn test_return_in_class_static_block() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "function f() {class C { static { return; } }}",
        &["return must be inside function"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "class C { static { return; } }",
        &["return must be inside function"],
    );
    let _ = tmp2;
    let tmp3 = h.parse("class C {static {function f() {return;}}}");
    let _ = tmp3;
}

// port: ParserTest#testContinueInClassStaticBlock
#[test]
fn test_continue_in_class_static_block() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("class C {static {continue;}}", &[UNEXPECTED_CONTINUE]);
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "for (let i = 1; i < 2; i++) {class C {static {continue;}}}",
        &[UNEXPECTED_CONTINUE],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "while (true) {class C {static {continue;}}}",
        &[UNEXPECTED_CONTINUE],
    );
    let _ = tmp3;
    let tmp4 = h.parse("class C {static {while (true) {continue;}}}");
    let _ = tmp4;
    let tmp5 = h.parse("class C {static {for (let i = 1; i < 2; i++) {continue;}}}");
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "x: for (let i = 1; i < 2; i++) {class C {static {continue x;}}}",
        &[&format!("{}{}", UNDEFINED_LABEL, " \"x\"")],
    );
    let _ = tmp6;
    let tmp7 = h.parse_error(
        "x: while (true) {class C {static {continue x;}}}",
        &[&format!("{}{}", UNDEFINED_LABEL, " \"x\"")],
    );
    let _ = tmp7;
    let tmp8 = h.parse("class C {static {x: while (true) {continue x;}}}");
    let _ = tmp8;
    let tmp9 = h.parse("class C {static {x: for (let i = 1; i < 2; i++) {continue x;}}}");
    let _ = tmp9;
    let tmp10 = h.parse_error(
        "class C {static {x: { while (true) {continue x;}}}}",
        &[UNEXPECTED_LABELLED_CONTINUE],
    );
    let _ = tmp10;
    let tmp11 = h.parse_error(
        "class C {static {x: {for (let i = 1; i < 2; i++) {continue x;}}}}",
        &[UNEXPECTED_LABELLED_CONTINUE],
    );
    let _ = tmp11;
}

// port: ParserTest#testBreakInClassStaticBlock
#[test]
fn test_break_in_class_static_block() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("class C {static {break;}}", &[UNLABELED_BREAK]);
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "for (let i = 1; i < 2; i++) {class C {static {break;}}}",
        &[UNLABELED_BREAK],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "while (true) {class C {static {break;}}}",
        &[UNLABELED_BREAK],
    );
    let _ = tmp3;
    let tmp4 = h.parse("class C {static {while (true) {break;}}}");
    let _ = tmp4;
    let tmp5 = h.parse("class C {static {for (let i = 1; i < 2; i++) {break;}}}");
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "x: for (let i = 1; i < 2; i++) {class C {static {break x;}}}",
        &[&format!("{}{}", UNDEFINED_LABEL, " \"x\"")],
    );
    let _ = tmp6;
    let tmp7 = h.parse_error(
        "x: while (true) {class C {static {break x;}}}",
        &[&format!("{}{}", UNDEFINED_LABEL, " \"x\"")],
    );
    let _ = tmp7;
    let tmp8 = h.parse("class C {static {x: while (true) {break x;}}}");
    let _ = tmp8;
    let tmp9 = h.parse("class C {static {x: for (let i = 1; i < 2; i++) {break x;}}}");
    let _ = tmp9;
    let tmp10 = h.parse("class C {static {x: { while (true) {break x;}}}}");
    let _ = tmp10;
    let tmp11 = h.parse("class C {static {x: {for (let i = 1; i < 2; i++) {break x;}}}}");
    let _ = tmp11;
}

// port: ParserTest#testYieldInClassStaticBlock
#[test]
fn test_yield_in_class_static_block() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "class C {static {var ind; yield;}}",
        &["primary expression expected"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "function* f(ind) {class C{ static {yield ind; ind++;}}}",
        &[UNEXPECTED_YIELD],
    );
    let _ = tmp2;
    let tmp3 = h.parse("class C{ static {function* f(ind) {yield ind; ind++;}}}");
    let _ = tmp3;
}

// port: ParserTest#testClassStaticBlock_this
#[test]
fn test_class_static_block_this() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C {\nstatic field1 = 1; static field2 = 2; static field3 = 3;\nstatic {\nlet x = this.field1; let y = this.field2; let z = this.field3;\n}\n}\n");
    let _ = tmp1;
    let tmp2 = h.parse("class C { static { this.field1 = 1; this.field2 = 2; this.field3 = 3; } }");
    let _ = tmp2;
    let tmp3 = h.parse("let a = class {\nstatic field1 = 1; static field2 = 2; static field3 = 3;\nstatic {\nlet x = this.field1; let y = this.field2; let z = this.field3;\n}\n};\n");
    let _ = tmp3;
    let tmp4 =
        h.parse("let a = class { static { this.field1 = 1; this.field2 = 2; this.field3 = 3; } };");
    let _ = tmp4;
    let tmp5 = h.parse("class C {\nstatic field1 = 1;\nstatic {\nfunction incr() { return ++A.field1; }\nconsole.log(incr());\nif(incr()) {\nthis.field2 = 2;\n}\n}\n}\n");
    let _ = tmp5;
    let tmp6 = h.parse(
        "class C {\nstatic field1 = 1;\nstatic {\ntry {\nthis.field1 = 2;\n}\ncatch {\n}\n}\n}\n",
    );
    let _ = tmp6;
}

// port: ParserTest#testClassStaticBlock_inheritance
#[test]
fn test_class_static_block_inheritance() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class Base {} class C extends Base { static { super(); } }");
    let _ = tmp1;
    let tmp2 = h.parse("class Base { static y; } class C extends Base { static { super.y; } }");
    let _ = tmp2;
    let tmp3 = h.parse("class Base { y; } class C extends Base { static { super.y; } }");
    let _ = tmp3;
}

// port: ParserTest#testClassExtendsLeftHandSideExpression
#[test]
fn test_class_extends_left_hand_side_expression() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class A {} class B extends (0, A) {}");
    let _ = tmp1;
    let tmp2 = h.parse_error("class A {} class B extends 0, A {}", &["'{' expected"]);
    let _ = tmp2;
}

// port: ParserTest#testMultipleClassStaticBlocks
#[test]
fn test_multiple_class_static_blocks() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C { static { } static { } }");
    let _ = tmp1;
    let tmp2 = h.parse("let a = class { static { } static { } };");
    let _ = tmp2;
    let tmp3 = h.parse("class C {\nstatic field1 = 1; static field2 = 2; static field3 = 3;\nstatic {\nlet x = this.field1; let y = this.field2;\n}\nstatic {\nlet z = this.field3;\n}\n}\n");
    let _ = tmp3;
    let tmp4 = h.parse(
        "class C { static { this.field1 = 1; this.field2 = 2; } static { this.field3 = 3; } }",
    );
    let _ = tmp4;
    let tmp5 = h.parse("let a = class {\nstatic field1 = 1; static field2 = 2; static field3 = 3;\nstatic {\nlet x = this.field1; let y = this.field2;\n}\nstatic {\nlet z = this.field3;\n}\n};\n");
    let _ = tmp5;
    let tmp6 = h.parse("let a = class {\nstatic {\nthis.field1 = 1; this.field2 = 2;\n}\nstatic {\n this.field3 = 3;\n}\n};\n");
    let _ = tmp6;
}

// port: ParserTest#testClassStaticBlock_linenocharno
#[test]
fn test_class_static_block_linenocharno() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C {\n static {}\n }");
    let mut n = tmp1.get_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::CLASS);
    assert_eq!(n.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(n.unwrap().get_charno(&h.ast), 0_i32);
    let mut members = get_class_members(&h.ast, n.unwrap());
    assert_eq!(members.get_token(&h.ast), Token::CLASS_MEMBERS);
    let mut static_block = members.get_first_child(&h.ast);
    assert_eq!(static_block.unwrap().get_token(&h.ast), Token::BLOCK);
    assert_eq!(static_block.unwrap().get_lineno(&h.ast), 2_i32);
    assert_eq!(static_block.unwrap().get_charno(&h.ast), 8_i32);
    assert_eq!(static_block.unwrap().get_length(&h.ast), 2_i32);
}

// port: ParserTest#testClassStaticBlock_invalid
#[test]
fn test_class_static_block_invalid() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("class { {} }", &["'identifier' expected"]);
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "class { static { static { } } }",
        &["'identifier' expected"],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "var o = { static {} };",
        &["Cannot use keyword in short object literal"],
    );
    let _ = tmp3;
}

// port: ParserTest#testClassStaticSuper
#[test]
fn test_class_static_super() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class Bar {\n  static double(n) {\n    return n * 2\n  }\n}\nclass Baz extends Bar {\n  // Used from a static field initializer.\n  static val1 = super.double(6);\n\n  static val2;\n  static {\n    // Used from within a static block.\n    Baz.val2 = super.double(5);\n  }\n}\n");
    let _ = tmp1;
}

// port: ParserTest#testSuper1
#[test]
fn test_super1() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::SUPER]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("super;");
    let _ = tmp1;
    let tmp2 = h.parse("function f() {super;};");
    let _ = tmp2;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp3 = h.parse_warning("super;", &[&requires_language_mode_message(Feature::SUPER)]);
    let _ = tmp3;
    h.mode = LanguageMode::ECMASCRIPT3;
    let tmp4 = h.parse_warning("super;", &[&requires_language_mode_message(Feature::SUPER)]);
    let _ = tmp4;
}

// port: ParserTest#testNewTarget
#[test]
fn test_new_target() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::NEW_TARGET]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("new.target;", &["new.target must be inside a function"]);
    let _ = tmp1;
    let tmp2 = h.parse("function f() { new.target; };");
    let _ = tmp2;
    h.mode = LanguageMode::ECMASCRIPT3;
    let tmp3 = h.parse_warning(
        "class C { f() { new.target; } }",
        &[
            &requires_language_mode_message(Feature::CLASSES),
            &requires_language_mode_message(Feature::MEMBER_DECLARATIONS),
            &requires_language_mode_message(Feature::NEW_TARGET),
        ],
    );
    let _ = tmp3;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp4 = h.parse_warning(
        "class C { f() { new.target; } }",
        &[
            &requires_language_mode_message(Feature::CLASSES),
            &requires_language_mode_message(Feature::MEMBER_DECLARATIONS),
            &requires_language_mode_message(Feature::NEW_TARGET),
        ],
    );
    let _ = tmp4;
    h.mode = LanguageMode::ECMASCRIPT_2015;
    h.expect_features(&[
        Feature::CLASSES,
        Feature::MEMBER_DECLARATIONS,
        Feature::NEW_TARGET,
    ]);
    let tmp5 = h.parse("class C { f() { new.target; } }");
    let _ = tmp5;
}

// port: ParserTest#testNewDotSomethingInvalid
#[test]
fn test_new_dot_something_invalid() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("function f(){new.something}", &["'target' expected"]);
    let _ = tmp1;
}

// port: ParserTest#hookWithDecimalNotParsedAsOptionalChaining
#[test]
fn hook_with_decimal_not_parsed_as_optional_chaining() {
    let mut h = Harness::new();
    let tmp1 = h.parse("a?.1:2");
    let mut n = tmp1.get_first_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::HOOK);
    assert_eq!(n.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(n.unwrap().get_charno(&h.ast), 0_i32);
    let tmp2 = IR::name(&mut h.ast, "a");
    assert_node_equality(&h.ast, tmp2, n.unwrap().get_first_child(&h.ast).unwrap());
    let tmp3 = IR::number(&mut h.ast, 0.1_f64);
    assert_node_equality(&h.ast, tmp3, n.unwrap().get_second_child(&h.ast).unwrap());
    let tmp4 = IR::number(&mut h.ast, 2.0_f64);
    assert_node_equality(&h.ast, tmp4, n.unwrap().get_last_child(&h.ast).unwrap());
}

// port: ParserTest#optChainGetProp
#[test]
fn opt_chain_get_prop() {
    let mut h = Harness::new();
    let tmp1 = h.parse("a?.b");
    let mut n = tmp1.get_first_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::OPTCHAIN_GETPROP);
    assert!(n.unwrap().is_optional_chain_start(&h.ast));
    assert_eq!(n.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(n.unwrap().get_charno(&h.ast), 3_i32);
    assert_eq!(n.unwrap().get_length(&h.ast), 1_i32);
    let tmp2 = IR::name(&mut h.ast, "a");
    assert_node_equality(&h.ast, tmp2, n.unwrap().get_first_child(&h.ast).unwrap());
    assert_eq!(n.unwrap().get_string(&h.ast), JsString::from("b"));
}

// port: ParserTest#optChainGetPropWithKeyword
#[test]
fn opt_chain_get_prop_with_keyword() {
    let mut h = Harness::new();
    let tmp1 = h.parse("a?.finally");
    let mut n = tmp1.get_first_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::OPTCHAIN_GETPROP);
    assert!(n.unwrap().is_optional_chain_start(&h.ast));
    assert_eq!(n.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(n.unwrap().get_charno(&h.ast), 3_i32);
    assert_eq!(n.unwrap().get_length(&h.ast), 7_i32);
    let tmp2 = IR::name(&mut h.ast, "a");
    assert_node_equality(&h.ast, tmp2, n.unwrap().get_first_child(&h.ast).unwrap());
    assert_eq!(n.unwrap().get_string(&h.ast), JsString::from("finally"));
}

// port: ParserTest#optChainGetElem
#[test]
fn opt_chain_get_elem() {
    let mut h = Harness::new();
    let tmp1 = h.parse("a?.[1]");
    let mut n = tmp1.get_first_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::OPTCHAIN_GETELEM);
    assert!(n.unwrap().is_optional_chain_start(&h.ast));
    assert_eq!(n.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(n.unwrap().get_charno(&h.ast), 0_i32);
    assert_eq!(n.unwrap().get_length(&h.ast), 6_i32);
    let tmp2 = IR::name(&mut h.ast, "a");
    assert_node_equality(&h.ast, tmp2, n.unwrap().get_first_child(&h.ast).unwrap());
    let tmp3 = IR::number(&mut h.ast, 1.0_f64);
    assert_node_equality(&h.ast, tmp3, n.unwrap().get_second_child(&h.ast).unwrap());
}

// port: ParserTest#optChainCall
#[test]
fn opt_chain_call() {
    let mut h = Harness::new();
    let tmp1 = h.parse("a?.()");
    let mut n = tmp1.get_first_first_child(&h.ast);
    assert_eq!(n.unwrap().get_token(&h.ast), Token::OPTCHAIN_CALL);
    assert!(n.unwrap().is_optional_chain_start(&h.ast));
    assert_eq!(n.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(n.unwrap().get_charno(&h.ast), 0_i32);
    assert_eq!(n.unwrap().get_length(&h.ast), 5_i32);
    let tmp2 = IR::name(&mut h.ast, "a");
    assert_node_equality(&h.ast, tmp2, n.unwrap().get_first_child(&h.ast).unwrap());
}

// port: ParserTest#optChainStartOfChain_innerChainIsArgOfACall
#[test]
fn opt_chain_start_of_chain_inner_chain_is_arg_of_a_call() {
    let mut h = Harness::new();
    let tmp1 = h.parse("a?.b?.(x?.y);");
    let mut opt_chain_call = tmp1.get_first_first_child(&h.ast);
    assert!(opt_chain_call.unwrap().is_opt_chain_call(&h.ast));
    let mut opt_chain_get_prop = opt_chain_call.unwrap().get_first_child(&h.ast);
    let mut opt_chain_arg = opt_chain_call.unwrap().get_last_child(&h.ast);
    assert!(opt_chain_get_prop.unwrap().is_opt_chain_get_prop(&h.ast));
    assert!(opt_chain_get_prop.unwrap().is_optional_chain_start(&h.ast));
    assert!(opt_chain_arg.unwrap().is_opt_chain_get_prop(&h.ast));
    assert!(opt_chain_arg.unwrap().is_optional_chain_start(&h.ast));
}

// port: ParserTest#optChainStartOfChain_optChainGetProp
#[test]
fn opt_chain_start_of_chain_opt_chain_get_prop() {
    let mut h = Harness::new();
    let tmp1 = h.parse("a?.b.c");
    let mut outer_get = tmp1.get_first_first_child(&h.ast);
    let mut inner_get = outer_get.unwrap().get_first_child(&h.ast);
    assert_eq!(
        outer_get.unwrap().get_token(&h.ast),
        Token::OPTCHAIN_GETPROP
    );
    assert!(!outer_get.unwrap().is_optional_chain_start(&h.ast));
    assert_eq!(outer_get.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(outer_get.unwrap().get_charno(&h.ast), 5_i32);
    assert_eq!(outer_get.unwrap().get_length(&h.ast), 1_i32);
    assert_eq!(
        inner_get.unwrap().get_token(&h.ast),
        Token::OPTCHAIN_GETPROP
    );
    assert!(inner_get.unwrap().is_optional_chain_start(&h.ast));
    assert_eq!(inner_get.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(inner_get.unwrap().get_charno(&h.ast), 3_i32);
    assert_eq!(inner_get.unwrap().get_length(&h.ast), 1_i32);
    assert_eq!(outer_get.unwrap().get_string(&h.ast), JsString::from("c"));
}

// port: ParserTest#optChainStartOfChain_optChainGetElem
#[test]
fn opt_chain_start_of_chain_opt_chain_get_elem() {
    let mut h = Harness::new();
    let tmp1 = h.parse("a?.[b][c]");
    let mut outer_get = tmp1.get_first_first_child(&h.ast);
    let mut inner_get = outer_get.unwrap().get_first_child(&h.ast);
    assert_eq!(
        outer_get.unwrap().get_token(&h.ast),
        Token::OPTCHAIN_GETELEM
    );
    assert!(!outer_get.unwrap().is_optional_chain_start(&h.ast));
    assert_eq!(outer_get.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(outer_get.unwrap().get_charno(&h.ast), 0_i32);
    assert_eq!(outer_get.unwrap().get_length(&h.ast), 9_i32);
    assert_eq!(
        inner_get.unwrap().get_token(&h.ast),
        Token::OPTCHAIN_GETELEM
    );
    assert!(inner_get.unwrap().is_optional_chain_start(&h.ast));
    assert_eq!(inner_get.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(inner_get.unwrap().get_charno(&h.ast), 0_i32);
    assert_eq!(inner_get.unwrap().get_length(&h.ast), 6_i32);
    let tmp2 = IR::name(&mut h.ast, "c");
    assert_node_equality(
        &h.ast,
        tmp2,
        outer_get.unwrap().get_second_child(&h.ast).unwrap(),
    );
}

// port: ParserTest#optChainStartOfChain_optChainCall
#[test]
fn opt_chain_start_of_chain_opt_chain_call() {
    let mut h = Harness::new();
    let tmp1 = h.parse("a?.()(b)");
    let mut outer_call = tmp1.get_first_first_child(&h.ast);
    let mut inner_call = outer_call.unwrap().get_first_child(&h.ast);
    assert_eq!(outer_call.unwrap().get_token(&h.ast), Token::OPTCHAIN_CALL);
    assert!(!outer_call.unwrap().is_optional_chain_start(&h.ast));
    assert_eq!(outer_call.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(outer_call.unwrap().get_charno(&h.ast), 0_i32);
    assert_eq!(outer_call.unwrap().get_length(&h.ast), 8_i32);
    assert_eq!(inner_call.unwrap().get_token(&h.ast), Token::OPTCHAIN_CALL);
    assert!(inner_call.unwrap().is_optional_chain_start(&h.ast));
    assert_eq!(inner_call.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(inner_call.unwrap().get_charno(&h.ast), 0_i32);
    assert_eq!(inner_call.unwrap().get_length(&h.ast), 5_i32);
    let tmp2 = IR::name(&mut h.ast, "b");
    assert_node_equality(
        &h.ast,
        tmp2,
        outer_call.unwrap().get_second_child(&h.ast).unwrap(),
    );
}

// port: ParserTest#optChainParens_optChainGetProp
#[test]
fn opt_chain_parens_opt_chain_get_prop() {
    let mut h = Harness::new();
    let tmp1 = h.parse("(a?.b).c");
    let mut outer_get = tmp1.get_first_first_child(&h.ast);
    let mut inner_get = outer_get.unwrap().get_first_child(&h.ast);
    assert_eq!(outer_get.unwrap().get_token(&h.ast), Token::GETPROP);
    assert_eq!(outer_get.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(outer_get.unwrap().get_charno(&h.ast), 7_i32);
    assert_eq!(outer_get.unwrap().get_length(&h.ast), 1_i32);
    assert_eq!(
        inner_get.unwrap().get_token(&h.ast),
        Token::OPTCHAIN_GETPROP
    );
    assert!(inner_get.unwrap().is_optional_chain_start(&h.ast));
    assert_eq!(inner_get.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(inner_get.unwrap().get_charno(&h.ast), 4_i32);
    assert_eq!(inner_get.unwrap().get_length(&h.ast), 1_i32);
    assert_eq!(outer_get.unwrap().get_string(&h.ast), JsString::from("c"));
}

// port: ParserTest#callExpressionBeforeOptionalGetProp
#[test]
fn call_expression_before_optional_get_prop() {
    let mut h = Harness::new();
    let tmp1 = h.parse("a()?.b");
    let mut get = tmp1.get_first_first_child(&h.ast);
    let mut call = get.unwrap().get_first_child(&h.ast);
    assert_eq!(get.unwrap().get_token(&h.ast), Token::OPTCHAIN_GETPROP);
    assert_eq!(get.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(get.unwrap().get_charno(&h.ast), 5_i32);
    assert_eq!(get.unwrap().get_length(&h.ast), 1_i32);
    assert_eq!(call.unwrap().get_token(&h.ast), Token::CALL);
    assert_eq!(call.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(call.unwrap().get_charno(&h.ast), 0_i32);
    assert_eq!(call.unwrap().get_length(&h.ast), 3_i32);
}

// port: ParserTest#optChainChain
#[test]
fn opt_chain_chain() {
    let mut h = Harness::new();
    let tmp1 = h.parse("a?.b?.c");
    let _ = tmp1;
    let tmp2 = h.parse("a.b?.c");
    let _ = tmp2;
    let tmp3 = h.parse("a?.b?.[1]");
    let _ = tmp3;
    let tmp4 = h.parse("a?.b?.()");
    let _ = tmp4;
    let tmp5 = h.parse("a?.[1]?.b");
    let _ = tmp5;
    let tmp6 = h.parse("a?.[1]?.b()");
    let _ = tmp6;
    let tmp7 = h.parse("a?.b?.c?.d");
    let _ = tmp7;
    let tmp8 = h.parse("a?.b?.c?.[1]");
    let _ = tmp8;
    let tmp9 = h.parse("a?.b?.c?.()");
    let _ = tmp9;
    let tmp10 = h.parse("a?.(c)?.b");
    let _ = tmp10;
    let tmp11 = h.parse("a().b?.c");
    let _ = tmp11;
}

// port: ParserTest#optChainAssignError
#[test]
fn opt_chain_assign_error() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("a?.b = c", &["invalid assignment target"]);
    let _ = tmp1;
}

// port: ParserTest#optChainConstructorError
#[test]
fn opt_chain_constructor_error() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "new a?.()",
        &["Optional chaining is forbidden in construction contexts."],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "new a?.b()",
        &["Optional chaining is forbidden in construction contexts."],
    );
    let _ = tmp2;
}

// port: ParserTest#optChainTemplateLiteralError
#[test]
fn opt_chain_template_literal_error() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "a?.()?.`hello`",
        &["template literal cannot be used within optional chaining"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "a?.`hello`",
        &["template literal cannot be used within optional chaining"],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "a?.b`hello`",
        &["template literal cannot be used within optional chaining"],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "a?.b\n`hello`",
        &["template literal cannot be used within optional chaining"],
    );
    let _ = tmp4;
}

// port: ParserTest#optChainMiscErrors
#[test]
fn opt_chain_misc_errors() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("super?.()", &["Optional chaining is forbidden in super?."]);
    let _ = tmp1;
    let tmp2 = h.parse_error("super?.foo", &["Optional chaining is forbidden in super?."]);
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "new?.target",
        &["Optional chaining is forbidden in `new?.target` contexts."],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "import?.('foo')",
        &["Optional chaining is forbidden in import?."],
    );
    let _ = tmp4;
}

// port: ParserTest#optChainDeleteValid
#[test]
fn opt_chain_delete_valid() {
    let mut h = Harness::new();
    let tmp1 = h.parse("delete a?.b");
    let _ = tmp1;
}

// port: ParserTest#optChainEs2019
#[test]
fn opt_chain_es2019() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::OPTIONAL_CHAINING]);
    h.mode = LanguageMode::ECMASCRIPT_2019;
    let tmp1 = h.parse_warning(
        "a?.b",
        &[&requires_language_mode_message(Feature::OPTIONAL_CHAINING)],
    );
    let _ = tmp1;
}

// port: ParserTest#optChainSyntaxError
#[test]
fn opt_chain_syntax_error() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("a?.{}", &["syntax error: { not allowed in optional chain"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("{a:x}?.a", &["primary expression expected"]);
    let _ = tmp2;
    let tmp3 = h.parse("({a:x})?.a");
    let _ = tmp3;
}

// port: ParserTest#testArrow1
#[test]
fn test_arrow1() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::ARROW_FUNCTIONS]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("()=>1;");
    let _ = tmp1;
    let tmp2 = h.parse("()=>{}");
    let _ = tmp2;
    let tmp3 = h.parse("(a,b) => a + b;");
    let _ = tmp3;
    let tmp4 = h.parse("a => b");
    let _ = tmp4;
    let tmp5 = h.parse("a => { return b }");
    let _ = tmp5;
    let tmp6 = h.parse("a => b");
    let _ = tmp6;
    let tmp7 = h.parse("var x = (a => b);");
    let _ = tmp7;
    h.mode = LanguageMode::ECMASCRIPT5;
    let tmp8 = h.parse_warning(
        "a => b",
        &[&requires_language_mode_message(Feature::ARROW_FUNCTIONS)],
    );
    let _ = tmp8;
    h.mode = LanguageMode::ECMASCRIPT3;
    let tmp9 = h.parse_warning(
        "a => b;",
        &[&requires_language_mode_message(Feature::ARROW_FUNCTIONS)],
    );
    let _ = tmp9;
}

// port: ParserTest#testArrowInvalid1
#[test]
fn test_arrow_invalid1() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("*()=>1;", &["primary expression expected"]);
    let _ = tmp1;
    let tmp2 = h.parse_error("var f = x\n=>2", &["No newline allowed before '=>'"]);
    let _ = tmp2;
    let tmp3 = h.parse_error("f = (x,y)\n=>2;", &["No newline allowed before '=>'"]);
    let _ = tmp3;
    let tmp4 = h.parse_error("f( (x,y)\n=>2)", &["No newline allowed before '=>'"]);
    let _ = tmp4;
}

// port: ParserTest#testAsyncFunction
#[test]
fn test_async_function() {
    let mut h = Harness::new();
    let mut async_function_expression_source = ("f = async function() {};").to_string();
    let mut async_function_declaration_source = ("async function f() {}").to_string();
    h.expect_features(&[Feature::ASYNC_FUNCTIONS]);
    for m in LanguageMode::VALUES {
        h.mode = m;
        h.strict_mode = if m == LanguageMode::ECMASCRIPT3 {
            StrictMode::SLOPPY
        } else {
            StrictMode::STRICT
        };
        if m.feature_set().has(Feature::ASYNC_FUNCTIONS) {
            let tmp1 = h.parse(async_function_expression_source.as_str());
            let _ = tmp1;
            let tmp2 = h.parse(async_function_declaration_source.as_str());
            let _ = tmp2;
        } else {
            let tmp3 = h.parse_warning(
                async_function_expression_source.as_str(),
                &[&requires_language_mode_message(Feature::ASYNC_FUNCTIONS)],
            );
            let _ = tmp3;
            let tmp4 = h.parse_warning(
                async_function_declaration_source.as_str(),
                &[&requires_language_mode_message(Feature::ASYNC_FUNCTIONS)],
            );
            let _ = tmp4;
        }
    }
}

// port: ParserTest#testAsyncNamedFunction
#[test]
fn test_async_named_function() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT_2015;
    h.expect_features(&[
        Feature::CLASSES,
        Feature::MEMBER_DECLARATIONS,
        Feature::CONST_DECLARATIONS,
        Feature::LET_DECLARATIONS,
    ]);
    let tmp1 = h.parse("class C {\n  async(x) { return x; }\n}\nconst c = new C();\nc.async(1);\nlet foo = async(5);\n");
    let _ = tmp1;
}

// port: ParserTest#testAsyncGeneratorFunction
#[test]
fn test_async_generator_function() {
    let mut h = Harness::new();
    h.expect_features(&[
        Feature::ASYNC_FUNCTIONS,
        Feature::GENERATORS,
        Feature::ASYNC_GENERATORS,
    ]);
    let tmp1 = h.parse("async function *f(){}");
    let _ = tmp1;
    let tmp2 = h.parse("f = async function *(){}");
    let _ = tmp2;
    let tmp3 = h.parse("class C { async *foo(){} }");
    let _ = tmp3;
}

// port: ParserTest#testAsyncArrowFunction
#[test]
fn test_async_arrow_function() {
    let mut h = Harness::new();
    h.do_async_arrow_function_test("f = async (x) => x + 1");
    h.do_async_arrow_function_test("f = async x => x + 1");
}

// port: ParserTest#testAsyncArrowInvalid
#[test]
fn test_async_arrow_invalid() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("f = not_async (x) => x + 1;", &["'=>' unexpected"]);
    let _ = tmp1;
}

// port: ParserTest#testAsyncMethod
#[test]
fn test_async_method() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::ASYNC_FUNCTIONS]);
    let tmp1 = h.parse("o={async m(){}}");
    let _ = tmp1;
    let tmp2 = h.parse("o={async [a+b](){}}");
    let _ = tmp2;
    let tmp3 = h.parse("class C{async m(){}}");
    let _ = tmp3;
    let tmp4 = h.parse("class C{static async m(){}}");
    let _ = tmp4;
    let tmp5 = h.parse("class C{async [a+b](){}}");
    let _ = tmp5;
    let tmp6 = h.parse("class C{static async [a+b](){}}");
    let _ = tmp6;
}

// port: ParserTest#testInvalidAsyncMethod
#[test]
fn test_invalid_async_method() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::MEMBER_DECLARATIONS]);
    let tmp1 = h.parse("o={async(){}}");
    let _ = tmp1;
    let tmp2 = h.parse("class C{async(){}}");
    let _ = tmp2;
    let tmp3 = h.parse("class C{static async(){}}");
    let _ = tmp3;
    h.expect_features(&[]);
    let tmp4 = h.parse("o={async:false}");
    let _ = tmp4;
    let tmp5 = h.parse_error("o={async\nm(){}}", &["'}' expected"]);
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "o={static async\nm(){}}",
        &["Cannot use keyword in short object literal"],
    );
    let _ = tmp6;
    h.expect_features(&[Feature::PUBLIC_CLASS_FIELDS]);
    let tmp7 = h.parse("class C{async};");
    let _ = tmp7;
    let tmp8 = h.parse("class C{async\nm(){}}");
    let _ = tmp8;
    let tmp9 = h.parse("class C{static async\nm(){}}");
    let _ = tmp9;
}

// port: ParserTest#testAwait_inFunctions
#[test]
fn test_await_in_functions() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::ASYNC_FUNCTIONS]);
    let tmp1 = h.parse("async function f(p) { await p }");
    let _ = tmp1;
    let tmp2 = h.parse("async function f(p) { async function f2() { await p; } }");
    let _ = tmp2;
    let tmp3 = h.parse("f = async function(p) { await p }");
    let _ = tmp3;
    let tmp4 = h.parse("f = async(p) => await p");
    let _ = tmp4;
    let tmp5 = h.parse("class C{ async m(p) { await p }}");
    let _ = tmp5;
    let tmp6 = h.parse("class C{ static async m(p) { await p }}");
    let _ = tmp6;
}

// port: ParserTest#testAwait_inMethods
#[test]
fn test_await_in_methods() {
    let mut h = Harness::new();
    let tmp1 = h.parse("class C { async method() { await 2; } }");
    let _ = tmp1;
    let tmp2 = h.parse("const x = { async method() { await 2; } }");
    let _ = tmp2;
}

// port: ParserTest#testAwait_inObjectLiteralComputedPropertyName
#[test]
fn test_await_in_object_literal_computed_property_name() {
    let mut h = Harness::new();
    let tmp1 = h.parse("async function f() { return { [await 1]: 1 }; }");
    let _ = tmp1;
    let tmp2 = h.parse("async function f() { return { [await 2]() {} }; }");
    let _ = tmp2;
    let tmp3 = h.parse("async function f() { return { *[await 3]() {} }; }");
    let _ = tmp3;
    let tmp4 = h.parse("async function f() { return { async [await 4]() {} }; }");
    let _ = tmp4;
    let tmp5 = h.parse("async function f() { return { async *[await 5]() {} }; }");
    let _ = tmp5;
    let tmp6 = h.parse("async function f() { return { get [await 6]() {} }; }");
    let _ = tmp6;
    let tmp7 = h.parse("async function f() { return { set [await 7](x) {} }; }");
    let _ = tmp7;
    let tmp8 = h.parse("const f = async () => ({ [await 8]: 1 });");
    let _ = tmp8;
}

// port: ParserTest#testAwait_inClassComputedPropertyName
#[test]
fn test_await_in_class_computed_property_name() {
    let mut h = Harness::new();
    let tmp1 = h.parse("async function f() { return class { [await 1] = 1 }; }");
    let _ = tmp1;
    let tmp2 = h.parse("async function f() { return class { [await 2]() {} }; }");
    let _ = tmp2;
    let tmp3 = h.parse("async function f() { return class { *[await 3]() {} }; }");
    let _ = tmp3;
    let tmp4 = h.parse("async function f() { return class { async [await 4]() {} }; }");
    let _ = tmp4;
    let tmp5 = h.parse("async function f() { return class { async *[await 5]() {} }; }");
    let _ = tmp5;
    let tmp6 = h.parse("async function f() { return class { get [await 6]() {} }; }");
    let _ = tmp6;
    let tmp7 = h.parse("async function f() { return class { set [await 7](x) {} }; }");
    let _ = tmp7;
    let tmp8 = h.parse("const f = async () => class { [await 8] = 1 };");
    let _ = tmp8;
}

// port: ParserTest#testAwait_invalid_missingExpression
#[test]
fn test_await_invalid_missing_expression() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "async function f() { await; }",
        &["primary expression expected"],
    );
    let _ = tmp1;
}

// port: ParserTest#testAwait_invalid_nonAsyncFunction
#[test]
fn test_await_invalid_non_async_function() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("function f() { return await 5; }", &[UNEXPECTED_AWAIT]);
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "async function f() { function f2() { return await 5; } }",
        &[UNEXPECTED_AWAIT],
    );
    let _ = tmp2;
}

// port: ParserTest#testAwait_invalid_nonAsyncMethod
#[test]
fn test_await_invalid_non_async_method() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("class C { method() { await 1; } }", &[UNEXPECTED_AWAIT]);
    let _ = tmp1;
    let tmp2 = h.parse_error("const x = { method() { await 2; } }", &[UNEXPECTED_AWAIT]);
    let _ = tmp2;
}

// port: ParserTest#testAwait_invalid_defaultParameter
#[test]
fn test_await_invalid_default_parameter() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "async function f(x = await 15) { return x; }",
        &["`await` is illegal in parameter default value."],
    );
    let _ = tmp1;
}

// port: ParserTest#testAwait_invalid_inClassStaticBlock
#[test]
fn test_await_invalid_in_class_static_block() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("class C { static { await 1; } }", &[UNEXPECTED_AWAIT]);
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "async function f() { class C { static { await 1; } } }",
        &[UNEXPECTED_AWAIT],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "async () => { class C { static { await 1; } } }",
        &[UNEXPECTED_AWAIT],
    );
    let _ = tmp3;
    let tmp4 = h.parse("class C { static { async function f() { await 1; } } }");
    let _ = tmp4;
    let tmp5 = h.parse("class C { static { async () => { await 1; } } }");
    let _ = tmp5;
}

// port: ParserTest#testAwait_invalid_inClassFieldInitializer
#[test]
fn test_await_invalid_in_class_field_initializer() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("class C { f = await 1; }", &[UNEXPECTED_AWAIT]);
    let _ = tmp1;
    let tmp2 = h.parse_error("class C { static f = await 1; }", &[UNEXPECTED_AWAIT]);
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "async function f() { class C { static f = await 1; } }",
        &[UNEXPECTED_AWAIT],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "async () => { class C { static f = await 1; } }",
        &[UNEXPECTED_AWAIT],
    );
    let _ = tmp4;
    let tmp5 = h.parse("class C { static f = async function f() { await 1; } }");
    let _ = tmp5;
    let tmp6 = h.parse("class C { static f = async () => { await 1; } }");
    let _ = tmp6;
}

// port: ParserTest#testAwait_inClassMethods
#[test]
fn test_await_in_class_methods() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("class C { method() { await 1; } }", &[UNEXPECTED_AWAIT]);
    let _ = tmp1;
    let tmp2 = h.parse("class C { async method() { await 2; } }");
    let _ = tmp2;
}

// port: ParserTest#testAwait_topLevelInEsModule
#[test]
fn test_await_top_level_in_es_module() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::TOP_LEVEL_AWAIT]);
    let tmp1 = h.parse("export const x = await 15;");
    let _ = tmp1;
    let tmp2 = h.parse("export const x = 1; await 15;");
    let _ = tmp2;
}

// port: ParserTest#testAwait_topLevelInObjectLiteral
#[test]
fn test_await_top_level_in_object_literal() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::TOP_LEVEL_AWAIT]);
    let tmp1 = h.parse("export const x = 1; const obj = { key: await x };");
    let _ = tmp1;
}

// port: ParserTest#testAwait_topLevelInBlock
#[test]
fn test_await_top_level_in_block() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::TOP_LEVEL_AWAIT]);
    let tmp1 = h.parse("export const x = 1; { await x; }");
    let _ = tmp1;
}

// port: ParserTest#testAwait_topLevelInLoops
#[test]
fn test_await_top_level_in_loops() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::TOP_LEVEL_AWAIT]);
    let tmp1 = h.parse("export const x = 1; for (;;) { await x; }");
    let _ = tmp1;
    let tmp2 = h.parse("export const x = 1; while (true) { await x; }");
    let _ = tmp2;
    let tmp3 = h.parse("export const x = 1; do { await x; } while (true);");
    let _ = tmp3;
}

// port: ParserTest#testAwait_topLevelInTryCatch
#[test]
fn test_await_top_level_in_try_catch() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::TOP_LEVEL_AWAIT]);
    let tmp1 = h.parse("export const x = 1; try { await x; } catch (e) {}");
    let _ = tmp1;
    let tmp2 = h.parse("export const x = 1; try {} catch (e) { await x; }");
    let _ = tmp2;
    let tmp3 = h.parse("export const x = 1; try {} finally { await x; }");
    let _ = tmp3;
}

// port: ParserTest#testAwait_topLevelInClassExtends
#[test]
fn test_await_top_level_in_class_extends() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::TOP_LEVEL_AWAIT]);
    let tmp1 = h.parse("export const x = class {}; class C extends (await x) {}");
    let _ = tmp1;
}

// port: ParserTest#testAwait_invalid_topLevelInScript
#[test]
fn test_await_invalid_top_level_in_script() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("const x = await 15;", &[UNEXPECTED_AWAIT]);
    let _ = tmp1;
    let tmp2 = h.parse_error("await 15;", &[UNEXPECTED_AWAIT]);
    let _ = tmp2;
}

// port: ParserTest#testAwait_invalid_topLevelInClassExtends
#[test]
fn test_await_invalid_top_level_in_class_extends() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "export const x = 1; class C extends await x {}",
        &["'{' expected"],
    );
    let _ = tmp1;
}

// port: ParserTest#testAwait_invalid_topLevelInGoogModule
#[test]
fn test_await_invalid_top_level_in_goog_module() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("goog.module('m'); await 1;", &[UNEXPECTED_AWAIT]);
    let _ = tmp1;
}

// port: ParserTest#testAwait_invalid_topLevelInGoogProvide
#[test]
fn test_await_invalid_top_level_in_goog_provide() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("goog.provide('m'); await 1;", &[UNEXPECTED_AWAIT]);
    let _ = tmp1;
}

// port: ParserTest#testAwait_topLevelInObjectLiteralComputedPropertyName
#[test]
fn test_await_top_level_in_object_literal_computed_property_name() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::TOP_LEVEL_AWAIT]);
    let tmp1 = h.parse("export const x = { [await 1]: 1 };");
    let _ = tmp1;
    let tmp2 = h.parse("export const x = { [await 2]() {} };");
    let _ = tmp2;
    let tmp3 = h.parse("export const x = { *[await 3]() {} };");
    let _ = tmp3;
    let tmp4 = h.parse("export const x = { async [await 4]() {} };");
    let _ = tmp4;
    let tmp5 = h.parse("export const x = { async *[await 5]() {} };");
    let _ = tmp5;
    let tmp6 = h.parse("export const x = { get [await 6]() {} };");
    let _ = tmp6;
    let tmp7 = h.parse("export const x = { set [await 7](x) {} };");
    let _ = tmp7;
    let tmp8 = h.parse("export const x = 1; const y = { [await 8]: 8 };");
    let _ = tmp8;
}

// port: ParserTest#testAwait_topLevelAwaitInClassComputedPropertyName
#[test]
fn test_await_top_level_await_in_class_computed_property_name() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::TOP_LEVEL_AWAIT]);
    let tmp1 = h.parse("export class C { [await 1] = 1 };");
    let _ = tmp1;
    let tmp2 = h.parse("export class C { [await 2]() {} };");
    let _ = tmp2;
    let tmp3 = h.parse("export class C { *[await 3]() {} };");
    let _ = tmp3;
    let tmp4 = h.parse("export class C { async [await 4]() {} };");
    let _ = tmp4;
    let tmp5 = h.parse("export class C { async *[await 5]() {} };");
    let _ = tmp5;
    let tmp6 = h.parse("export class C { get [await 6]() {} };");
    let _ = tmp6;
    let tmp7 = h.parse("export class C { set [await 7](x) {} };");
    let _ = tmp7;
    let tmp8 = h.parse("export class C { [await 8] = 1 };");
    let _ = tmp8;
    let tmp9 = h.parse("export const x = 1; class C { [await 9] = 1 };");
    let _ = tmp9;
}

// port: ParserTest#testAwait_invalid_topLevelAwaitInClassComputedPropertyNameInScript
#[test]
fn test_await_invalid_top_level_await_in_class_computed_property_name_in_script() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("class C { [await 1] = 1 };", &[UNEXPECTED_AWAIT]);
    let _ = tmp1;
}

// port: ParserTest#testForAwaitOf_inFunctions
#[test]
fn test_for_await_of_in_functions() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::FOR_AWAIT_OF]);
    let tmp1 = h.parse("async () => { for await (a of b) c;}");
    let _ = tmp1;
    let tmp2 = h.parse("async () => { for await (var a of b) c;}");
    let _ = tmp2;
    let tmp3 = h.parse("async () => { for await (a.x of b) c;}");
    let _ = tmp3;
    let tmp4 = h.parse("async () => { for await ([a1, a2, a3] of b) c;}");
    let _ = tmp4;
    let tmp5 = h.parse("async () => { for await (const {x, y, z} of b) c;}");
    let _ = tmp5;
    let tmp6 = h.parse("async () => { for await (const {x, y = 2, z} of b) c;}");
    let _ = tmp6;
    h.expect_features(&[Feature::FOR_AWAIT_OF, Feature::LET_DECLARATIONS]);
    let tmp7 = h.parse("async () => { for await (let a of b) c;}");
    let _ = tmp7;
    h.expect_features(&[Feature::FOR_AWAIT_OF, Feature::CONST_DECLARATIONS]);
    let tmp8 = h.parse("async () => { for await (const a of b) c;}");
    let _ = tmp8;
}

// port: ParserTest#testForAwaitOf_invalid_declWithInitializer
#[test]
fn test_for_await_of_invalid_decl_with_initializer() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error(
        "async () => { for await (a = 1 of b) c;}",
        &[INVALID_ASSIGNMENT_TARGET],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "async () => { for await (var a = 1 of b) c;}",
        &[INVALID_FOR_AWAIT_INIT],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "async () => { for await (let a = 1 of b) c;}",
        &[INVALID_FOR_AWAIT_INIT],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "async () => { for await (const a = 1 of b) c;}",
        &[INVALID_FOR_AWAIT_INIT],
    );
    let _ = tmp4;
    let tmp5 = h.parse_error(
        "async () => { for await (let {a} = {} of b) c;}",
        &[INVALID_FOR_AWAIT_INIT],
    );
    let _ = tmp5;
}

// port: ParserTest#testForAwaitOf_invalid_multipleDecls
#[test]
fn test_for_await_of_invalid_multiple_decls() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error(
        "async () => { for await (a, b of c) d;}",
        &[INVALID_ASSIGNMENT_TARGET],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "async () => { for await (var a, b of c) d;}",
        &[INVALID_FOR_AWAIT_MULT_DECLS],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "async () => { for await (let a, b of c) d;}",
        &[INVALID_FOR_AWAIT_MULT_DECLS],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "async () => { for await (const a, b of c) d;}",
        &[INVALID_FOR_AWAIT_MULT_DECLS],
    );
    let _ = tmp4;
}

// port: ParserTest#testForAwaitOf_topLevelInEsModule
#[test]
fn test_for_await_of_top_level_in_es_module() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::TOP_LEVEL_AWAIT]);
    let tmp1 = h.parse("export const x = 1; for await (let a of b) foo();");
    let _ = tmp1;
}

// port: ParserTest#testForAwaitOf_invalid_topLevelInScript
#[test]
fn test_for_await_of_invalid_top_level_in_script() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("for await (let a of b) foo();", &[UNEXPECTED_FOR_AWAIT_OF]);
    let _ = tmp1;
}

// port: ParserTest#testFor_ES5
#[test]
fn test_for_es5() {
    let mut h = Harness::new();
    let tmp1 = h.parse("for (var x; x != 10; x = next()) {}");
    let _ = tmp1;
    let tmp2 = h.parse("for (var x; x != 10; x = next());");
    let _ = tmp2;
    let tmp3 = h.parse("for (var x = 0; x != 10; x++) {}");
    let _ = tmp3;
    let tmp4 = h.parse("for (var x = 0; x != 10; x++);");
    let _ = tmp4;
    let tmp5 = h.parse("var x; for (x; x != 10; x = next()) {}");
    let _ = tmp5;
    let tmp6 = h.parse("var x; for (x; x != 10; x = next());");
    let _ = tmp6;
    let tmp7 = h.parse_error("for (x in {};;) {}", &["')' expected"]);
    let _ = tmp7;
}

// port: ParserTest#testFor_ES6
#[test]
fn test_for_es6() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::LET_DECLARATIONS]);
    let tmp1 = h.parse("for (let x; x != 10; x = next()) {}");
    let _ = tmp1;
    let tmp2 = h.parse("for (let x; x != 10; x = next());");
    let _ = tmp2;
    let tmp3 = h.parse("for (let x = 0; x != 10; x++) {}");
    let _ = tmp3;
    let tmp4 = h.parse("for (let x = 0; x != 10; x++);");
    let _ = tmp4;
    h.expect_features(&[Feature::CONST_DECLARATIONS]);
    let tmp5 = h.parse("for (const x = 0; x != 10; x++) {}");
    let _ = tmp5;
    let tmp6 = h.parse("for (const x = 0; x != 10; x++);");
    let _ = tmp6;
}

// port: ParserTest#testForConstNoInitializer
#[test]
fn test_for_const_no_initializer() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "for (const x; x != 10; x = next()) {}",
        &["const variables must have an initializer"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "for (const x; x != 10; x = next());",
        &["const variables must have an initializer"],
    );
    let _ = tmp2;
}

// port: ParserTest#testForIn_ES6
#[test]
fn test_for_in_es6() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("for (a in b) c;");
    let _ = tmp1;
    let tmp2 = h.parse("for (var a in b) c;");
    let _ = tmp2;
    h.expect_features(&[Feature::LET_DECLARATIONS]);
    let tmp3 = h.parse("for (let a in b) c;");
    let _ = tmp3;
    h.expect_features(&[Feature::CONST_DECLARATIONS]);
    let tmp4 = h.parse("for (const a in b) c;");
    let _ = tmp4;
    h.expect_features(&[]);
    let tmp5 = h.parse_error("for (a,b in c) d;", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "for (var a,b in c) d;",
        &["for-in statement may not have more than one variable declaration"],
    );
    let _ = tmp6;
    let tmp7 = h.parse_error(
        "for (let a,b in c) d;",
        &["for-in statement may not have more than one variable declaration"],
    );
    let _ = tmp7;
    let tmp8 = h.parse_error(
        "for (const a,b in c) d;",
        &["for-in statement may not have more than one variable declaration"],
    );
    let _ = tmp8;
    let tmp9 = h.parse_error("for (a=1 in b) c;", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp9;
    let tmp10 = h.parse_error(
        "for (let a=1 in b) c;",
        &["for-in statement may not have initializer"],
    );
    let _ = tmp10;
    let tmp11 = h.parse_error(
        "for (const a=1 in b) c;",
        &["for-in statement may not have initializer"],
    );
    let _ = tmp11;
    let tmp12 = h.parse_error(
        "for (var a=1 in b) c;",
        &["for-in statement may not have initializer"],
    );
    let _ = tmp12;
    let tmp13 = h.parse_error("for (\"a\" in b) c;", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp13;
}

// port: ParserTest#testForIn_ES5
#[test]
fn test_for_in_es5() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT5;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("for (a in b) c;");
    let _ = tmp1;
    let tmp2 = h.parse("for (var a in b) c;");
    let _ = tmp2;
    let tmp3 = h.parse_error("for (a=1 in b) c;", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp3;
    let tmp4 = h.parse_warning(
        "for (var a=1 in b) c;",
        &["for-in statement should not have initializer"],
    );
    let _ = tmp4;
}

// port: ParserTest#testForInDestructuring
#[test]
fn test_for_in_destructuring() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING]);
    let tmp1 = h.parse("for ({a} in b) c;");
    let _ = tmp1;
    let tmp2 = h.parse("for (var {a} in b) c;");
    let _ = tmp2;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING, Feature::LET_DECLARATIONS]);
    let tmp3 = h.parse("for (let {a} in b) c;");
    let _ = tmp3;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING, Feature::CONST_DECLARATIONS]);
    let tmp4 = h.parse("for (const {a} in b) c;");
    let _ = tmp4;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING]);
    let tmp5 = h.parse("for ({a: b} in c) d;");
    let _ = tmp5;
    let tmp6 = h.parse("for (var {a: b} in c) d;");
    let _ = tmp6;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING, Feature::LET_DECLARATIONS]);
    let tmp7 = h.parse("for (let {a: b} in c) d;");
    let _ = tmp7;
    h.expect_features(&[Feature::OBJECT_DESTRUCTURING, Feature::CONST_DECLARATIONS]);
    let tmp8 = h.parse("for (const {a: b} in c) d;");
    let _ = tmp8;
    h.expect_features(&[Feature::ARRAY_DESTRUCTURING]);
    let tmp9 = h.parse("for ([a] in b) c;");
    let _ = tmp9;
    let tmp10 = h.parse("for (var [a] in b) c;");
    let _ = tmp10;
    h.expect_features(&[Feature::ARRAY_DESTRUCTURING, Feature::LET_DECLARATIONS]);
    let tmp11 = h.parse("for (let [a] in b) c;");
    let _ = tmp11;
    h.expect_features(&[Feature::ARRAY_DESTRUCTURING, Feature::CONST_DECLARATIONS]);
    let tmp12 = h.parse("for (const [a] in b) c;");
    let _ = tmp12;
}

// port: ParserTest#testForInDestructuringInvalid
#[test]
fn test_for_in_destructuring_invalid() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("for ({a: b} = foo() in c) d;", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "for (var {a: b} = foo() in c) d;",
        &["for-in statement may not have initializer"],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "for (let {a: b} = foo() in c) d;",
        &["for-in statement may not have initializer"],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "for (const {a: b} = foo() in c) d;",
        &["for-in statement may not have initializer"],
    );
    let _ = tmp4;
    let tmp5 = h.parse_error("for ([a] = foo() in b) c;", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "for (var [a] = foo() in b) c;",
        &["for-in statement may not have initializer"],
    );
    let _ = tmp6;
    let tmp7 = h.parse_error(
        "for (let [a] = foo() in b) c;",
        &["for-in statement may not have initializer"],
    );
    let _ = tmp7;
    let tmp8 = h.parse_error(
        "for (const [a] = foo() in b) c;",
        &["for-in statement may not have initializer"],
    );
    let _ = tmp8;
}

// port: ParserTest#testForOf1
#[test]
fn test_for_of1() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::FOR_OF]);
    let tmp1 = h.parse("for(a of b) c;");
    let _ = tmp1;
    let tmp2 = h.parse("for(var a of b) c;");
    let _ = tmp2;
    h.expect_features(&[Feature::FOR_OF, Feature::LET_DECLARATIONS]);
    let tmp3 = h.parse("for(let a of b) c;");
    let _ = tmp3;
    h.expect_features(&[Feature::FOR_OF, Feature::CONST_DECLARATIONS]);
    let tmp4 = h.parse("for(const a of b) c;");
    let _ = tmp4;
}

// port: ParserTest#testForOf2
#[test]
fn test_for_of2() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("for(a=1 of b) c;", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "for(var a=1 of b) c;",
        &["for-of statement may not have initializer"],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "for(let a=1 of b) c;",
        &["for-of statement may not have initializer"],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "for(const a=1 of b) c;",
        &["for-of statement may not have initializer"],
    );
    let _ = tmp4;
}

// port: ParserTest#testForOf3
#[test]
fn test_for_of3() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error(
        "for(var a, b of c) d;",
        &["for-of statement may not have more than one variable declaration"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "for(let a, b of c) d;",
        &["for-of statement may not have more than one variable declaration"],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "for(const a, b of c) d;",
        &["for-of statement may not have more than one variable declaration"],
    );
    let _ = tmp3;
}

// port: ParserTest#testForOf4
#[test]
fn test_for_of4() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("for(a, b of c) d;", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp1;
}

// port: ParserTest#testDestructuringInForLoops
#[test]
fn test_destructuring_in_for_loops() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error(
        "for (var {x: y} = foo() in bar()) {}",
        &["for-in statement may not have initializer"],
    );
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "for (let {x: y} = foo() in bar()) {}",
        &["for-in statement may not have initializer"],
    );
    let _ = tmp2;
    let tmp3 = h.parse_error(
        "for (const {x: y} = foo() in bar()) {}",
        &["for-in statement may not have initializer"],
    );
    let _ = tmp3;
    let tmp4 = h.parse_error(
        "for (var {x: y} = foo() of bar()) {}",
        &["for-of statement may not have initializer"],
    );
    let _ = tmp4;
    let tmp5 = h.parse_error(
        "for (let {x: y} = foo() of bar()) {}",
        &["for-of statement may not have initializer"],
    );
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "for (const {x: y} = foo() of bar()) {}",
        &["for-of statement may not have initializer"],
    );
    let _ = tmp6;
    let tmp7 = h.parse_error(
        "for (var {x: y};;) {}",
        &["destructuring must have an initializer"],
    );
    let _ = tmp7;
    let tmp8 = h.parse_error(
        "for (let {x: y};;) {}",
        &["destructuring must have an initializer"],
    );
    let _ = tmp8;
    let tmp9 = h.parse_error(
        "for (const {x: y};;) {}",
        &["const variables must have an initializer"],
    );
    let _ = tmp9;
}

// port: ParserTest#testInvalidDestructuring
#[test]
fn test_invalid_destructuring() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("for ({x: 5} in foo()) {}", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp1;
    let tmp2 = h.parse_error("for ({x: 'str'} in foo()) {}", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp2;
    let tmp3 = h.parse_error("var {x: 5} = foo();", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp3;
    let tmp4 = h.parse_error("var {x: 'str'} = foo();", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp4;
    let tmp5 = h.parse_error("({x: 5} = foo());", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp5;
    let tmp6 = h.parse_error("({x: 'str'} = foo());", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp6;
    let tmp7 = h.parse_error("function f({method(){}}) {}", &["'}' expected"]);
    let _ = tmp7;
    let tmp8 = h.parse_error("function f({method(){}} = foo()) {}", &["'}' expected"]);
    let _ = tmp8;
}

// port: ParserTest#testForOfPatterns
#[test]
fn test_for_of_patterns() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::FOR_OF, Feature::OBJECT_DESTRUCTURING]);
    let tmp1 = h.parse("for({x} of b) c;");
    let _ = tmp1;
    let tmp2 = h.parse("for({x: y} of b) c;");
    let _ = tmp2;
    h.expect_features(&[Feature::FOR_OF, Feature::ARRAY_DESTRUCTURING]);
    let tmp3 = h.parse("for([x, y] of b) c;");
    let _ = tmp3;
    let tmp4 = h.parse("for([x, ...y] of b) c;");
    let _ = tmp4;
    h.expect_features(&[
        Feature::FOR_OF,
        Feature::OBJECT_DESTRUCTURING,
        Feature::LET_DECLARATIONS,
    ]);
    let tmp5 = h.parse("for(let {x} of b) c;");
    let _ = tmp5;
    let tmp6 = h.parse("for(let {x: y} of b) c;");
    let _ = tmp6;
    h.expect_features(&[
        Feature::FOR_OF,
        Feature::ARRAY_DESTRUCTURING,
        Feature::LET_DECLARATIONS,
    ]);
    let tmp7 = h.parse("for(let [x, y] of b) c;");
    let _ = tmp7;
    let tmp8 = h.parse("for(let [x, ...y] of b) c;");
    let _ = tmp8;
    h.expect_features(&[
        Feature::FOR_OF,
        Feature::OBJECT_DESTRUCTURING,
        Feature::CONST_DECLARATIONS,
    ]);
    let tmp9 = h.parse("for(const {x} of b) c;");
    let _ = tmp9;
    let tmp10 = h.parse("for(const {x: y} of b) c;");
    let _ = tmp10;
    h.expect_features(&[
        Feature::FOR_OF,
        Feature::ARRAY_DESTRUCTURING,
        Feature::CONST_DECLARATIONS,
    ]);
    let tmp11 = h.parse("for(const [x, y] of b) c;");
    let _ = tmp11;
    let tmp12 = h.parse("for(const [x, ...y] of b) c;");
    let _ = tmp12;
    h.expect_features(&[Feature::FOR_OF, Feature::OBJECT_DESTRUCTURING]);
    let tmp13 = h.parse("for(var {x} of b) c;");
    let _ = tmp13;
    let tmp14 = h.parse("for(var {x: y} of b) c;");
    let _ = tmp14;
    h.expect_features(&[Feature::FOR_OF, Feature::ARRAY_DESTRUCTURING]);
    let tmp15 = h.parse("for(var [x, y] of b) c;");
    let _ = tmp15;
    let tmp16 = h.parse("for(var [x, ...y] of b) c;");
    let _ = tmp16;
}

// port: ParserTest#testForOfPatternsWithInitializer
#[test]
fn test_for_of_patterns_with_initializer() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse_error("for({x}=a of b) c;", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp1;
    let tmp2 = h.parse_error("for({x: y}=a of b) c;", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp2;
    let tmp3 = h.parse_error("for([x, y]=a of b) c;", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp3;
    let tmp4 = h.parse_error("for([x, ...y]=a of b) c;", &[INVALID_ASSIGNMENT_TARGET]);
    let _ = tmp4;
    let tmp5 = h.parse_error(
        "for(let {x}=a of b) c;",
        &["for-of statement may not have initializer"],
    );
    let _ = tmp5;
    let tmp6 = h.parse_error(
        "for(let {x: y}=a of b) c;",
        &["for-of statement may not have initializer"],
    );
    let _ = tmp6;
    let tmp7 = h.parse_error(
        "for(let [x, y]=a of b) c;",
        &["for-of statement may not have initializer"],
    );
    let _ = tmp7;
    let tmp8 = h.parse_error(
        "for(let [x, ...y]=a of b) c;",
        &["for-of statement may not have initializer"],
    );
    let _ = tmp8;
    let tmp9 = h.parse_error(
        "for(const {x}=a of b) c;",
        &["for-of statement may not have initializer"],
    );
    let _ = tmp9;
    let tmp10 = h.parse_error(
        "for(const {x: y}=a of b) c;",
        &["for-of statement may not have initializer"],
    );
    let _ = tmp10;
    let tmp11 = h.parse_error(
        "for(const [x, y]=a of b) c;",
        &["for-of statement may not have initializer"],
    );
    let _ = tmp11;
    let tmp12 = h.parse_error(
        "for(const [x, ...y]=a of b) c;",
        &["for-of statement may not have initializer"],
    );
    let _ = tmp12;
}

// port: ParserTest#testImport
#[test]
fn test_import() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::MODULES]);
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("import 'someModule'");
    let _ = tmp1;
    let tmp2 = h.parse("import d from './someModule'");
    let _ = tmp2;
    let tmp3 = h.parse("import {} from './someModule'");
    let _ = tmp3;
    let tmp4 = h.parse("import {x, y} from './someModule'");
    let _ = tmp4;
    let tmp5 = h.parse("import {x as x1, y as y1} from './someModule'");
    let _ = tmp5;
    let tmp6 = h.parse("import {x as x1, y as y1, } from './someModule'");
    let _ = tmp6;
    let tmp7 = h.parse("import {default as d, class as c} from './someModule'");
    let _ = tmp7;
    let tmp8 = h.parse("import d, {x as x1, y as y1} from './someModule'");
    let _ = tmp8;
    let tmp9 = h.parse("import * as sm from './someModule'");
    let _ = tmp9;
    h.expect_features(&[]);
    let tmp10 = h.parse_error(
        "import class from './someModule'",
        &["cannot use keyword 'class' here."],
    );
    let _ = tmp10;
    let tmp11 = h.parse_error(
        "import * as class from './someModule'",
        &["'identifier' expected"],
    );
    let _ = tmp11;
    let tmp12 = h.parse_error(
        "import {a as class} from './someModule'",
        &["'identifier' expected"],
    );
    let _ = tmp12;
    let tmp13 = h.parse_error("import {class} from './someModule'", &["'as' expected"]);
    let _ = tmp13;
}

// port: ParserTest#testExport
#[test]
fn test_export() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::MODULES]);
    let tmp1 = h.parse("export const x = 1");
    let _ = tmp1;
    let tmp2 = h.parse("export var x = 1");
    let _ = tmp2;
    let tmp3 = h.parse("export function f() {}");
    let _ = tmp3;
    let tmp4 = h.parse("export class c {}");
    let _ = tmp4;
    let tmp5 = h.parse("export {x, y}");
    let _ = tmp5;
    let tmp6 = h.parse("export {x as x1}");
    let _ = tmp6;
    let tmp7 = h.parse("export {x as x1, y as x2}");
    let _ = tmp7;
    let tmp8 = h.parse("export {x as default, y as class}");
    let _ = tmp8;
    h.expect_features(&[]);
    let tmp9 = h.parse_error(
        "export {default as x}",
        &["cannot use keyword 'default' here."],
    );
    let _ = tmp9;
    let tmp10 = h.parse_error(
        "export {package as x}",
        &["cannot use keyword 'package' here."],
    );
    let _ = tmp10;
    let tmp11 = h.parse_error("export {package}", &["cannot use keyword 'package' here."]);
    let _ = tmp11;
    h.expect_features(&[Feature::MODULES]);
    let tmp12 = h.parse("export {x as x1, y as y1} from './someModule'");
    let _ = tmp12;
    let tmp13 = h.parse("export {x as x1, y as y1, } from './someModule'");
    let _ = tmp13;
    let tmp14 = h.parse("export {default as d} from './someModule'");
    let _ = tmp14;
    let tmp15 = h.parse("export {d as default, c as class} from './someModule'");
    let _ = tmp15;
    let tmp16 = h.parse("export {default as default, class as class} from './someModule'");
    let _ = tmp16;
    let tmp17 = h.parse("export {class} from './someModule'");
    let _ = tmp17;
    let tmp18 = h.parse("export * from './someModule'");
    let _ = tmp18;
    h.expect_features(&[]);
    let tmp19 = h.parse_error("export * as s from './someModule';", &["'from' expected"]);
    let _ = tmp19;
}

// port: ParserTest#testExportAsync
#[test]
fn test_export_async() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::MODULES, Feature::ASYNC_FUNCTIONS]);
    let tmp1 = h.parse("export async function f() {}");
    let _ = tmp1;
}

// port: ParserTest#testImportExportTypescriptKeyword
#[test]
fn test_import_export_typescript_keyword() {
    let mut h = Harness::new();
    let tmp1 = h.parse("export { namespace };");
    let _ = tmp1;
    let tmp2 = h.parse("import { namespace } from './input0.js';");
    let _ = tmp2;
}

// port: ParserTest#testGoogModule
#[test]
fn test_goog_module() {
    let mut h = Harness::new();
    let tmp1 = h.parse("goog.module('example');");
    let mut tree = tmp1;
    assert_eq!(tree.get_token(&h.ast), Token::SCRIPT);
    assert!(tree.get_static_source_file(&h.ast).is_some());
    assert_eq!(
        tree.get_first_child(&h.ast).unwrap().get_token(&h.ast),
        Token::MODULE_BODY
    );
    assert!(
        tree.get_first_child(&h.ast)
            .unwrap()
            .get_static_source_file(&h.ast)
            .is_some()
    );
}

// port: ParserTest#testShebang
#[test]
fn test_shebang() {
    let mut h = Harness::new();
    let tmp1 = h.parse("#!/usr/bin/node\n var x = 1;");
    let _ = tmp1;
    let tmp2 = h.parse_error(
        "var x = 1; \n #!/usr/bin/node",
        &["Shebang comment must be at the start of the file"],
    );
    let _ = tmp2;
}

// port: ParserTest#testInvalidPoundUsage
#[test]
fn test_invalid_pound_usage() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "var x = 1; \n# Wrong-style comment",
        &["Invalid usage of #"],
    );
    let _ = tmp1;
}

// port: ParserTest#testLookaheadGithubIssue699
#[test]
fn test_lookahead_github_issue699() {
    let mut h = Harness::new();
    let mut start = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let tmp1 = h.parse("[1,[1,[1,[1,[1,[1,\n[1,[1,[1,[1,[1,[1,\n[1,[1,[1,[1,[1,[1,\n[1,[1,[1,[1,[1,[1,\n[1,[1,[1,[1,[1,[1,\n[1,[1,\n[1]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]\n");
    let _ = tmp1;
    let mut stop = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    assert!((stop - start) < 5000);
}

// port: ParserTest#testInvalidHandling1
#[test]
fn test_invalid_handling1() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/**\n * @fileoverview Definition.\n * @mods {ns.bar}\n * @modName mod\n *\n * @extends {ns.bar}\n * @author someone\n */\n\ngoog.provide('ns.foo');\n");
    let _ = tmp1;
}

// port: ParserTest#testUtf8
#[test]
fn test_utf8() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT5;
    h.strict_mode = StrictMode::SLOPPY;
    let tmp1 = h.parse("﻿function f() {}\n");
    let mut n = tmp1;
    let mut fn_node = n.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
}

// port: ParserTest#testParseDeep1
#[test]
fn test_parse_deep1() {
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(|| {
            let mut h = Harness::new();
            let mut code = ("var x; x = \n").to_string();
            for i in 1..15000 {
                code.push_str(&format!("  '{i}' +\n"));
            }
            code.push_str("'end';n");
            let tmp1 = h.parse(code.as_str());
            let _ = tmp1;
        })
        .unwrap()
        .join()
        .unwrap();
}

// port: ParserTest#testParseDeep2
#[test]
fn test_parse_deep2() {
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(|| {
            let mut h = Harness::new();
            let mut code = ("var x; x = \n").to_string();
            for i in 1..15000 {
                code.push_str(&format!("  '{i}' +\n"));
            }
            code.push_str("'end'; /** a comment */\n");
            let tmp1 = h.parse(code.as_str());
            let _ = tmp1;
        })
        .unwrap()
        .join()
        .unwrap();
}

// port: ParserTest#testParseDeep3
#[test]
fn test_parse_deep3() {
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(|| {
            let mut h = Harness::new();
            let mut code = ("var x; x = \n").to_string();
            for i in 1..15000 {
                code.push_str(&format!("  '{i}' +\n"));
            }
            code.push_str("  /** @type {string} */ (x);\n");
            let tmp1 = h.parse(code.as_str());
            let _ = tmp1;
        })
        .unwrap()
        .join()
        .unwrap();
}

// port: ParserTest#testParseInlineSourceMap
#[test]
fn test_parse_inline_source_map() {
    let mut h = Harness::new();
    let mut code = ("var X = (function () {\n    function X(input) {\n        this.y = input;\n    }\n    return X;\n}());\nconsole.log(new X(1));\n//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiZm9vLmpzIiwic291cmNlUm9vdCI6IiIsInNvdXJjZXMiOlsiZm9vLnRzIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUFBO0lBR0UsV0FBWSxLQUFhO1FBQ3ZCLElBQUksQ0FBQyxDQUFDLEdBQUcsS0FBSyxDQUFDO0lBQ2pCLENBQUM7SUFDSCxRQUFDO0FBQUQsQ0FBQyxBQU5ELElBTUM7QUFFRCxPQUFPLENBQUMsR0FBRyxDQUFDLElBQUksQ0FBQyxDQUFDLENBQUMsQ0FBQyxDQUFDLENBQUMifQ==\n").to_string();
    let tmp1 = h.do_parse(code.as_str(), &[]);
    let mut result = tmp1;
    assert_eq!(
        result.source_map_url,
        Some(JsString::from(
            "data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiZm9vLmpzIiwic291cmNlUm9vdCI6IiIsInNvdXJjZXMiOlsiZm9vLnRzIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUFBO0lBR0UsV0FBWSxLQUFhO1FBQ3ZCLElBQUksQ0FBQyxDQUFDLEdBQUcsS0FBSyxDQUFDO0lBQ2pCLENBQUM7SUFDSCxRQUFDO0FBQUQsQ0FBQyxBQU5ELElBTUM7QUFFRCxPQUFPLENBQUMsR0FBRyxDQUFDLElBQUksQ0FBQyxDQUFDLENBQUMsQ0FBQyxDQUFDLENBQUMifQ=="
        ))
    );
}

// port: ParserTest#testParseSourceMapRelativeURL
#[test]
fn test_parse_source_map_relative_url() {
    let mut h = Harness::new();
    let mut code = ("var X = (function () {\n    function X(input) {\n        this.y = input;\n    }\n    return X;\n}());\nconsole.log(new X(1));\n//# sourceMappingURL=somefile.js.map\n").to_string();
    let tmp1 = h.do_parse(code.as_str(), &[]);
    let mut result = tmp1;
    assert_eq!(
        result.source_map_url,
        Some(JsString::from("somefile.js.map"))
    );
}

// port: ParserTest#testParseSourceMapAbsoluteURL
#[test]
fn test_parse_source_map_absolute_url() {
    let mut h = Harness::new();
    let mut code =
        ("console.log('asdf');\n//# sourceMappingURL=/some/absolute/path/to/somefile.js.map\n")
            .to_string();
    let tmp1 = h.do_parse(code.as_str(), &[]);
    let mut result = tmp1;
    assert_eq!(
        result.source_map_url,
        Some(JsString::from("/some/absolute/path/to/somefile.js.map"))
    );
}

// port: ParserTest#testParseSourceMapAbsoluteURLHTTP
#[test]
fn test_parse_source_map_absolute_urlhttp() {
    let mut h = Harness::new();
    let mut code = ("console.log('asdf');\n//# sourceMappingURL=http://google.com/some/absolute/path/to/somefile.js.map\n").to_string();
    let tmp1 = h.do_parse(code.as_str(), &[]);
    let mut result = tmp1;
    assert_eq!(
        result.source_map_url,
        Some(JsString::from(
            "http://google.com/some/absolute/path/to/somefile.js.map"
        ))
    );
}

// port: ParserTest#testIncorrectAssignmentDoesntCrash
#[test]
fn test_incorrect_assignment_doesnt_crash() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error("[1 + 2] = 3;", &["invalid assignment target"]);
    let _ = tmp1;
    h.is_ide_mode = true;
    let tmp2 = h.parse_error(
        "[1 + 2] = 3;",
        &[
            "invalid assignment target",
            "']' expected",
            "invalid assignment target",
            "Semi-colon expected",
            "Semi-colon expected",
            "primary expression expected",
            "invalid assignment target",
            "Semi-colon expected",
            "primary expression expected",
            "Semi-colon expected",
        ],
    );
    let _ = tmp2;
}

// port: ParserTest#testDynamicImport
#[test]
fn test_dynamic_import() {
    let mut h = Harness::new();
    let mut dynamic_import_uses = [
        "import('foo')",
        "import('foo').then(function(a) { return a; })",
        "var moduleNamespace = import('foo')",
        "Promise.all([import('foo')]).then(function(a) { return a; })",
        "function foo() { foo(); import('foo'); } foo();",
    ];
    h.expect_features(&[Feature::DYNAMIC_IMPORT]);
    for m in LanguageMode::VALUES {
        h.mode = m;
        h.strict_mode = if m == LanguageMode::ECMASCRIPT3 {
            StrictMode::SLOPPY
        } else {
            StrictMode::STRICT
        };
        if m.feature_set().has(Feature::DYNAMIC_IMPORT) {
            for import_use_source in dynamic_import_uses.iter().copied() {
                let tmp1 = h.parse(import_use_source);
                let _ = tmp1;
            }
        } else {
            for import_use_source in dynamic_import_uses.iter().copied() {
                let tmp2 = h.parse_warning(
                    import_use_source,
                    &[&requires_language_mode_message(Feature::DYNAMIC_IMPORT)],
                );
                let _ = tmp2;
            }
        }
    }
    h.mode = LanguageMode::ECMASCRIPT_2020;
    let tmp3 = h.parse_error(
        "function foo() { import bar from './someModule'; }",
        &["'(' expected"],
    );
    let _ = tmp3;
}

// port: ParserTest#testAwaitDynamicImport
#[test]
fn test_await_dynamic_import() {
    let mut h = Harness::new();
    let mut await_dynamic_import_uses = [
        "(async function() { return await import('foo'); })()",
        "(async function() { await import('foo').then(function(a) { return a; }); })()",
        "(async function() { var moduleNamespace = await import('foo'); })()",
        "(async function() {\nawait Promise.all([import('foo')]).then(function(a) { return a; }); })()\n",
    ];
    h.expect_features(&[Feature::DYNAMIC_IMPORT, Feature::ASYNC_FUNCTIONS]);
    for import_use_source in await_dynamic_import_uses.iter().copied() {
        let tmp1 = h.parse(import_use_source);
        let _ = tmp1;
    }
}

// port: ParserTest#testImportMeta
#[test]
fn test_import_meta() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::MODULES, Feature::IMPORT_META]);
    let tmp1 = h.parse("import.meta");
    let mut tree = tmp1;
    let tmp2 = IR::import_meta(&mut h.ast);
    let tmp3 = IR::expr_result(&mut h.ast, tmp2);
    assert_node_equality(&h.ast, tmp3, tree.get_first_first_child(&h.ast).unwrap());
}

// port: ParserTest#testImportMeta_es5
#[test]
fn test_import_meta_es5() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT5;
    h.expect_features(&[Feature::MODULES, Feature::IMPORT_META]);
    let tmp1 = h.parse_warning(
        "import.meta",
        &[
            &requires_language_mode_message(Feature::MODULES),
            &requires_language_mode_message(Feature::IMPORT_META),
        ],
    );
    let _ = tmp1;
}

// port: ParserTest#testImportMeta_es6
#[test]
fn test_import_meta_es6() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT_2015;
    h.expect_features(&[Feature::MODULES, Feature::IMPORT_META]);
    let tmp1 = h.parse_warning(
        "import.meta",
        &[&requires_language_mode_message(Feature::IMPORT_META)],
    );
    let _ = tmp1;
}

// port: ParserTest#testImportMeta_inExpression
#[test]
fn test_import_meta_in_expression() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::MODULES, Feature::IMPORT_META]);
    let tmp1 = h.parse("import.meta.url");
    let mut prop_tree = tmp1;
    let tmp2 = IR::import_meta(&mut h.ast);
    let tmp3 = IR::getprop(&mut h.ast, tmp2, "url");
    let tmp4 = IR::expr_result(&mut h.ast, tmp3);
    assert_node_equality(
        &h.ast,
        tmp4,
        prop_tree.get_first_first_child(&h.ast).unwrap(),
    );
    let tmp5 = h.parse("f(import.meta.url)");
    let mut call_tree = tmp5;
    let tmp6 = IR::name(&mut h.ast, "f");
    let tmp7 = IR::import_meta(&mut h.ast);
    let tmp8 = IR::getprop(&mut h.ast, tmp7, "url");
    let tmp9 = IR::call(&mut h.ast, tmp6, &[tmp8]);
    let tmp10 = free_call(&mut h.ast, tmp9);
    let tmp11 = IR::expr_result(&mut h.ast, tmp10);
    assert_node_equality(
        &h.ast,
        tmp11,
        call_tree.get_first_first_child(&h.ast).unwrap(),
    );
}

// port: ParserTest#testImportMeta_asDotProperty
#[test]
fn test_import_meta_as_dot_property() {
    let mut h = Harness::new();
    let tmp1 = h.parse("x.import.meta");
    let mut tree = tmp1;
    let tmp2 = IR::name(&mut h.ast, "x");
    let tmp3 = IR::getprop_with_more_props(&mut h.ast, tmp2, "import", &[JsString::from("meta")]);
    let tmp4 = IR::expr_result(&mut h.ast, tmp3);
    assert_node_equality(&h.ast, tmp4, tree.get_first_child(&h.ast).unwrap());
}

// port: ParserTest#testNullishCoalesce
#[test]
fn test_nullish_coalesce() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::NULL_COALESCE_OP]);
    let tmp1 = h.parse("x??y");
    let mut tree = tmp1;
    let tmp2 = IR::name(&mut h.ast, "x");
    let tmp3 = IR::name(&mut h.ast, "y");
    let tmp4 = IR::coalesce(&mut h.ast, tmp2, tmp3);
    let tmp5 = IR::expr_result(&mut h.ast, tmp4);
    assert_node_equality(&h.ast, tmp5, tree.get_first_child(&h.ast).unwrap());
}

// port: ParserTest#testNullishCoalesce_es2019
#[test]
fn test_nullish_coalesce_es2019() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT_2019;
    h.expect_features(&[Feature::NULL_COALESCE_OP]);
    let tmp1 = h.parse_warning(
        "x??y",
        &[&requires_language_mode_message(Feature::NULL_COALESCE_OP)],
    );
    let _ = tmp1;
}

// port: ParserTest#testNullishCoalesce_withLogicalAND_shouldFail
#[test]
fn test_nullish_coalesce_with_logical_and_should_fail() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "x&&y??z",
        &["Logical OR and logical AND require parentheses when used with '??'"],
    );
    let _ = tmp1;
}

// port: ParserTest#testNullishCoalesce_withLogicalOR_shouldFail
#[test]
fn test_nullish_coalesce_with_logical_or_should_fail() {
    let mut h = Harness::new();
    let tmp1 = h.parse_error(
        "x??y||z",
        &["Logical OR and logical AND require parentheses when used with '??'"],
    );
    let _ = tmp1;
}

// port: ParserTest#testNullishCoalesce_withLogicalANDinParens
#[test]
fn test_nullish_coalesce_with_logical_an_din_parens() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::NULL_COALESCE_OP]);
    let tmp1 = h.parse("(x&&y)??z");
    let mut tree = tmp1;
    let tmp2 = IR::name(&mut h.ast, "x");
    let tmp3 = IR::name(&mut h.ast, "y");
    let tmp4 = IR::and(&mut h.ast, tmp2, tmp3);
    let tmp5 = IR::name(&mut h.ast, "z");
    let tmp6 = IR::coalesce(&mut h.ast, tmp4, tmp5);
    let tmp7 = IR::expr_result(&mut h.ast, tmp6);
    assert_node_equality(&h.ast, tmp7, tree.get_first_child(&h.ast).unwrap());
}

// port: ParserTest#testNullishCoalesce_chaining
#[test]
fn test_nullish_coalesce_chaining() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::NULL_COALESCE_OP]);
    let tmp1 = h.parse("x??y??z");
    let mut tree = tmp1;
    let mut expr = tree.get_first_child(&h.ast);
    let mut coalesce = expr.unwrap().get_first_child(&h.ast);
    let tmp2 = IR::name(&mut h.ast, "x");
    let tmp3 = IR::name(&mut h.ast, "y");
    let tmp4 = IR::coalesce(&mut h.ast, tmp2, tmp3);
    let tmp5 = IR::name(&mut h.ast, "z");
    let tmp6 = IR::coalesce(&mut h.ast, tmp4, tmp5);
    let tmp7 = IR::expr_result(&mut h.ast, tmp6);
    assert_node_equality(&h.ast, tmp7, expr.unwrap());
    assert_eq!(expr.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(expr.unwrap().get_charno(&h.ast), 0_i32);
    assert_eq!(expr.unwrap().get_length(&h.ast), 7_i32);
    assert_eq!(coalesce.unwrap().get_token(&h.ast), Token::COALESCE);
    assert_eq!(coalesce.unwrap().get_lineno(&h.ast), 1_i32);
    assert_eq!(coalesce.unwrap().get_charno(&h.ast), 0_i32);
    assert_eq!(coalesce.unwrap().get_length(&h.ast), 7_i32);
    assert_eq!(
        coalesce
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_token(&h.ast),
        Token::COALESCE
    );
    assert_eq!(
        coalesce
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_lineno(&h.ast),
        1_i32
    );
    assert_eq!(
        coalesce
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_charno(&h.ast),
        0_i32
    );
    assert_eq!(
        coalesce
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_length(&h.ast),
        4_i32
    );
}

// port: ParserTest#testAssignOR
#[test]
fn test_assign_or() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::LOGICAL_ASSIGNMENT]);
    let tmp1 = h.parse("x||=y");
    let mut tree = tmp1;
    let tmp2 = IR::name(&mut h.ast, "x");
    let tmp3 = IR::name(&mut h.ast, "y");
    let tmp4 = IR::assign_or(&mut h.ast, tmp2, tmp3);
    let tmp5 = IR::expr_result(&mut h.ast, tmp4);
    assert_node_equality(&h.ast, tmp5, tree.get_first_child(&h.ast).unwrap());
}

// port: ParserTest#testAssignOr_es2020
#[test]
fn test_assign_or_es2020() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT_2020;
    h.expect_features(&[Feature::LOGICAL_ASSIGNMENT]);
    let tmp1 = h.parse_warning(
        "x||=y",
        &[&requires_language_mode_message(Feature::LOGICAL_ASSIGNMENT)],
    );
    let _ = tmp1;
}

// port: ParserTest#testAssignAnd
#[test]
fn test_assign_and() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::LOGICAL_ASSIGNMENT]);
    let tmp1 = h.parse("x&&=y");
    let mut tree = tmp1;
    let tmp2 = IR::name(&mut h.ast, "x");
    let tmp3 = IR::name(&mut h.ast, "y");
    let tmp4 = IR::assign_and(&mut h.ast, tmp2, tmp3);
    let tmp5 = IR::expr_result(&mut h.ast, tmp4);
    assert_node_equality(&h.ast, tmp5, tree.get_first_child(&h.ast).unwrap());
}

// port: ParserTest#testAssignAnd_es2020
#[test]
fn test_assign_and_es2020() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT_2020;
    h.expect_features(&[Feature::LOGICAL_ASSIGNMENT]);
    let tmp1 = h.parse_warning(
        "x&&=y",
        &[&requires_language_mode_message(Feature::LOGICAL_ASSIGNMENT)],
    );
    let _ = tmp1;
}

// port: ParserTest#testAssignCoalesce
#[test]
fn test_assign_coalesce() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::LOGICAL_ASSIGNMENT]);
    let tmp1 = h.parse("x??=y");
    let mut tree = tmp1;
    let tmp2 = IR::name(&mut h.ast, "x");
    let tmp3 = IR::name(&mut h.ast, "y");
    let tmp4 = IR::assign_coalesce(&mut h.ast, tmp2, tmp3);
    let tmp5 = IR::expr_result(&mut h.ast, tmp4);
    assert_node_equality(&h.ast, tmp5, tree.get_first_child(&h.ast).unwrap());
}

// port: ParserTest#testAssignCoalesce_es2020
#[test]
fn test_assign_coalesce_es2020() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT_2020;
    h.expect_features(&[Feature::LOGICAL_ASSIGNMENT]);
    let tmp1 = h.parse_warning(
        "x??=y",
        &[&requires_language_mode_message(Feature::LOGICAL_ASSIGNMENT)],
    );
    let _ = tmp1;
}

// port: ParserTest#testNoDuplicateComments_arrow_fn
#[test]
fn test_no_duplicate_comments_arrow_fn() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse_comments("const a = (/** number */ n) => {}");
    let mut comments = tmp1;
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0_usize].value, JsString::from("/** number */"));
}

// port: ParserTest#testIndirectCallName
#[test]
fn test_indirect_call_name() {
    let mut h = Harness::new();
    let tmp1 = h.parse("(0, foo)();");
    let mut script = tmp1;
    assert_eq!(script.get_token(&h.ast), Token::SCRIPT);
    let mut expr_result = script.get_first_child(&h.ast);
    assert_eq!(expr_result.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    let mut call = expr_result.unwrap().get_first_child(&h.ast);
    assert!(
        call.unwrap().is_call(&h.ast)
            || call.unwrap().is_opt_chain_call(&h.ast)
            || call.unwrap().is_tagged_template_lit(&h.ast)
    );
    assert!(call.unwrap().get_boolean_prop(&h.ast, NodeId::FREE_CALL));
    assert!(call.unwrap().has_children(&h.ast));
    assert!(
        call.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .is_name(&h.ast)
    );
    assert_eq!(
        call.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_string(&h.ast),
        JsString::from("foo")
    );
}

// port: ParserTest#testIndirectCallGetProp
#[test]
fn test_indirect_call_get_prop() {
    let mut h = Harness::new();
    let tmp1 = h.parse("(0, foo.bar)();");
    let mut script = tmp1;
    assert_eq!(script.get_token(&h.ast), Token::SCRIPT);
    let mut expr_result = script.get_first_child(&h.ast);
    assert_eq!(expr_result.unwrap().get_token(&h.ast), Token::EXPR_RESULT);
    let mut call = expr_result.unwrap().get_first_child(&h.ast);
    assert!(
        call.unwrap().is_call(&h.ast)
            || call.unwrap().is_opt_chain_call(&h.ast)
            || call.unwrap().is_tagged_template_lit(&h.ast)
    );
    assert!(call.unwrap().get_boolean_prop(&h.ast, NodeId::FREE_CALL));
    assert!(call.unwrap().has_children(&h.ast));
    assert!(
        call.unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .matches_qualified_name(&h.ast, JsString::from("foo.bar"))
    );
}

// port: ParserTest#testFreeCall1
#[test]
fn test_free_call1() {
    let mut h = Harness::new();
    let tmp1 = h.parse("foo();");
    let mut script = tmp1;
    assert_eq!(script.get_token(&h.ast), Token::SCRIPT);
    let mut first_expr = script.get_first_child(&h.ast);
    let mut call = first_expr.unwrap().get_first_child(&h.ast);
    assert_eq!(call.unwrap().get_token(&h.ast), Token::CALL);
    assert!(
        call.unwrap().is_call(&h.ast)
            || call.unwrap().is_opt_chain_call(&h.ast)
            || call.unwrap().is_tagged_template_lit(&h.ast)
    );
    assert!(call.unwrap().get_boolean_prop(&h.ast, NodeId::FREE_CALL));
}

// port: ParserTest#testFreeCall2
#[test]
fn test_free_call2() {
    let mut h = Harness::new();
    let tmp1 = h.parse("x.foo();");
    let mut script = tmp1;
    assert_eq!(script.get_token(&h.ast), Token::SCRIPT);
    let mut first_expr = script.get_first_child(&h.ast);
    let mut call = first_expr.unwrap().get_first_child(&h.ast);
    assert_eq!(call.unwrap().get_token(&h.ast), Token::CALL);
    assert!(!call.unwrap().get_boolean_prop(&h.ast, NodeId::FREE_CALL));
}

// port: ParserTest#testTaggedTemplateFreeCall1
#[test]
fn test_tagged_template_free_call1() {
    let mut h = Harness::new();
    let tmp1 = h.parse("foo``;");
    let mut script = tmp1;
    assert_eq!(script.get_token(&h.ast), Token::SCRIPT);
    let mut first_expr = script.get_first_child(&h.ast);
    let mut call = first_expr.unwrap().get_first_child(&h.ast);
    assert_eq!(call.unwrap().get_token(&h.ast), Token::TAGGED_TEMPLATELIT);
    assert!(
        call.unwrap().is_call(&h.ast)
            || call.unwrap().is_opt_chain_call(&h.ast)
            || call.unwrap().is_tagged_template_lit(&h.ast)
    );
    assert!(call.unwrap().get_boolean_prop(&h.ast, NodeId::FREE_CALL));
}

// port: ParserTest#testTaggedTemplateFreeCall2
#[test]
fn test_tagged_template_free_call2() {
    let mut h = Harness::new();
    let tmp1 = h.parse("x.foo``;");
    let mut script = tmp1;
    assert_eq!(script.get_token(&h.ast), Token::SCRIPT);
    let mut first_expr = script.get_first_child(&h.ast);
    let mut call = first_expr.unwrap().get_first_child(&h.ast);
    assert_eq!(call.unwrap().get_token(&h.ast), Token::TAGGED_TEMPLATELIT);
    assert!(!call.unwrap().get_boolean_prop(&h.ast, NodeId::FREE_CALL));
}

// port: ParserTest#optionalFreeCall1
#[test]
fn optional_free_call1() {
    let mut h = Harness::new();
    let tmp1 = h.parse("foo?.();");
    let mut script = tmp1;
    assert_eq!(script.get_token(&h.ast), Token::SCRIPT);
    let mut first_expr = script.get_first_child(&h.ast);
    let mut call = first_expr.unwrap().get_first_child(&h.ast);
    assert_eq!(call.unwrap().get_token(&h.ast), Token::OPTCHAIN_CALL);
    assert!(
        call.unwrap().is_call(&h.ast)
            || call.unwrap().is_opt_chain_call(&h.ast)
            || call.unwrap().is_tagged_template_lit(&h.ast)
    );
    assert!(call.unwrap().get_boolean_prop(&h.ast, NodeId::FREE_CALL));
}

// port: ParserTest#optChainFreeCall
#[test]
fn opt_chain_free_call() {
    let mut h = Harness::new();
    let tmp1 = h.parse("x?.foo();");
    let mut script = tmp1;
    assert_eq!(script.get_token(&h.ast), Token::SCRIPT);
    let mut first_expr = script.get_first_child(&h.ast);
    let mut call = first_expr.unwrap().get_first_child(&h.ast);
    assert_eq!(call.unwrap().get_token(&h.ast), Token::OPTCHAIN_CALL);
    assert!(!call.unwrap().get_boolean_prop(&h.ast, NodeId::FREE_CALL));
}

// port: ParserTest#testIsInClosureUnawareSubtreeProperty_isCorrectlySet
#[test]
fn test_is_in_closure_unaware_subtree_property_is_correctly_set() {
    let mut h = Harness::new();
    let tmp1 = h.parse_warning("/**\n * @fileoverview\n * @closureUnaware\n */\ngoog.module('a.b');\n/** @closureUnaware */\n(function() {\n  const x = 5;\n}).call(globalThis);\nlet string1 = \"ClosureAwareString\";\n/** @closureUnaware */\n(function() {\n  const y = 5;\n}).call(globalThis);\n", &["@closureUnaware annotation is not allowed in this compilation", "@closureUnaware annotation is not allowed in this compilation", "@closureUnaware annotation is not allowed in this compilation"]);
    let mut script = tmp1;
    let mut module_body = script.get_first_child(&h.ast);
    let mut first_unaware_function_expr_result = module_body.unwrap().get_second_child(&h.ast);
    let mut aware_let = first_unaware_function_expr_result.unwrap().get_next(&h.ast);
    let mut second_unaware_function_expr_result = aware_let.unwrap().get_next(&h.ast);
    let mut first_unaware_shadow_host = first_unaware_function_expr_result
        .unwrap()
        .get_first_first_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    let mut second_unaware_shadow_host = second_unaware_function_expr_result
        .unwrap()
        .get_first_first_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    let mut first_shadow_root = first_unaware_shadow_host
        .unwrap()
        .get_closure_unaware_shadow(&h.ast);
    let mut second_shadow_root = second_unaware_shadow_host
        .unwrap()
        .get_closure_unaware_shadow(&h.ast);
    let mut first_unaware_function_function_node =
        find_first_node(&mut h.ast, first_shadow_root.unwrap(), NodeId::is_function);
    let mut second_unaware_function_function_node =
        find_first_node(&mut h.ast, second_shadow_root.unwrap(), NodeId::is_function);
    assert!(first_unaware_shadow_host.unwrap().is_name(&h.ast));
    assert_eq!(
        first_unaware_shadow_host.unwrap().get_string(&h.ast),
        JsString::from("$jscomp_wrap_closure_unaware_code")
    );
    assert!(second_unaware_shadow_host.unwrap().is_name(&h.ast));
    assert_eq!(
        second_unaware_shadow_host.unwrap().get_string(&h.ast),
        JsString::from("$jscomp_wrap_closure_unaware_code")
    );
    assert!(
        first_unaware_shadow_host
            .unwrap()
            .get_closure_unaware_shadow(&h.ast)
            .is_some()
    );
    assert!(
        !first_unaware_shadow_host
            .unwrap()
            .get_is_in_closure_unaware_subtree(&h.ast)
    );
    assert!(
        second_unaware_shadow_host
            .unwrap()
            .get_closure_unaware_shadow(&h.ast)
            .is_some()
    );
    assert!(
        !second_unaware_shadow_host
            .unwrap()
            .get_is_in_closure_unaware_subtree(&h.ast)
    );
    assert!(first_unaware_function_function_node.get_is_in_closure_unaware_subtree(&h.ast));
    assert!(second_unaware_function_function_node.get_is_in_closure_unaware_subtree(&h.ast));
    assert!(!aware_let.unwrap().get_is_in_closure_unaware_subtree(&h.ast));
    assert!(NodeId::validate_memory_sensitive_property_guarantees(
        &h.ast,
        first_unaware_function_function_node,
        true,
        first_unaware_function_function_node
            .get_first_child(&h.ast)
            .unwrap(),
        true
    ));
    assert!(NodeId::validate_memory_sensitive_property_guarantees(
        &h.ast,
        second_unaware_function_function_node,
        true,
        second_unaware_function_function_node
            .get_first_child(&h.ast)
            .unwrap(),
        true
    ));
    assert!(NodeId::validate_memory_sensitive_property_guarantees(
        &h.ast,
        first_unaware_function_function_node,
        true,
        second_unaware_function_function_node,
        true
    ));
    assert!(NodeId::validate_memory_sensitive_property_guarantees(
        &h.ast,
        first_unaware_function_function_node,
        true,
        aware_let.unwrap(),
        false
    ));
    assert!(NodeId::validate_memory_sensitive_property_guarantees(
        &h.ast,
        second_unaware_function_function_node,
        true,
        aware_let.unwrap(),
        false
    ));
    assert!(NodeId::validate_memory_sensitive_property_guarantees(
        &h.ast,
        first_unaware_function_function_node,
        true,
        aware_let.unwrap(),
        false
    ));
}

// port: ParserTest#closureUnawareAnnotation_specifyMinificationLevel_inFileOverview_succeeds
#[test]
fn closure_unaware_annotation_specify_minification_level_in_file_overview_succeeds() {
    let mut h = Harness::new();
    let mut tmp1 = TestErrorReporter::new();
    tmp1.expect_all_warnings(&[
        "@closureUnaware annotation is not allowed in this compilation",
        "@closureUnaware annotation is not allowed in this compilation",
    ]);
    let mut error_reporter = tmp1;
    let tmp2 = h.do_parse_with_reporter("/** @fileoverview @closureUnaware {SIMPLE} */\n/** @closureUnaware */ (\n  function() {}).call(globalThis);\n", &mut error_reporter);
    let _ = tmp2;
}

// port: ParserTest#closureUnawareAnnotation_specifyMinificationLevel_inPerFunctionDoc_errors
#[test]
fn closure_unaware_annotation_specify_minification_level_in_per_function_doc_errors() {
    let mut h = Harness::new();
    let mut tmp1 = TestErrorReporter::new();
    tmp1.expect_all_errors(&[
        "@closureUnaware mode can only be specified at the fileoverview level",
    ]);
    tmp1.expect_all_warnings(&[
        "@closureUnaware annotation is not allowed in this compilation",
        "@closureUnaware annotation is not allowed in this compilation",
    ]);
    let mut error_reporter = tmp1;
    let tmp2 = h.do_parse_with_reporter("/** @fileoverview @closureUnaware */\n/** @closureUnaware {SIMPLE} */ (\n  function() {}).call(globalThis);\n", &mut error_reporter);
    let _ = tmp2;
}

// port: ParserTest#closureUnawareAnnotation_specifyMinificationLevel_inMultipleDoc_errors
#[test]
fn closure_unaware_annotation_specify_minification_level_in_multiple_doc_errors() {
    let mut h = Harness::new();
    let mut tmp1 = TestErrorReporter::new();
    tmp1.expect_all_errors(&[
        "@closureUnaware mode can only be specified at the fileoverview level",
    ]);
    tmp1.expect_all_warnings(&[
        "@closureUnaware annotation is not allowed in this compilation",
        "@closureUnaware annotation is not allowed in this compilation",
    ]);
    let mut error_reporter = tmp1;
    let tmp2 = h.do_parse_with_reporter("/** @fileoverview @closureUnaware {SIMPLE} */\n/** @closureUnaware {SIMPLE} */ (\n  function() {}).call(globalThis);\n", &mut error_reporter);
    let _ = tmp2;
}

// port: ParserTest#closureUnawareAnnotation_usedOutsideFileWithClosureUnaware_errors
#[test]
fn closure_unaware_annotation_used_outside_file_with_closure_unaware_errors() {
    let mut h = Harness::new();
    let mut tmp1 = TestErrorReporter::new();
    tmp1.expect_all_errors(&[
        "invalid @closureUnaware node: must be in a file annotated with @closureUnaware",
    ]);
    tmp1.expect_all_warnings(&["@closureUnaware annotation is not allowed in this compilation"]);
    let mut error_reporter = tmp1;
    let tmp2 = h.do_parse_with_reporter(
        "/** @closureUnaware */ (\n  function() {}).call(globalThis);\n",
        &mut error_reporter,
    );
    let _ = tmp2;
}

// port: ParserTest#closureUnawareAnnotation_onNonFunctionCall_errors
#[test]
fn closure_unaware_annotation_on_non_function_call_errors() {
    let mut h = Harness::new();
    h.mode = LanguageMode::ECMASCRIPT_2020;
    h.expect_features(&[Feature::CONST_DECLARATIONS]);
    let mut tmp1 = TestErrorReporter::new();
    tmp1.expect_all_errors(&["@closureUnaware root must be function"]);
    tmp1.expect_all_warnings(&[
        "@closureUnaware annotation is not allowed in this compilation",
        "@closureUnaware annotation is not allowed in this compilation",
    ]);
    let mut error_reporter = tmp1;
    let tmp2 = h.do_parse_with_reporter(
        "/** @fileoverview @closureUnaware */\n/** @closureUnaware */ const x = 0;\n",
        &mut error_reporter,
    );
    let _ = tmp2;
}

// port: ParserTest#testParse
#[test]
fn test_parse() {
    let mut h = Harness::new();
    h.strict_mode = StrictMode::SLOPPY;
    let a = h.ast.new_string_with_token(Token::NAME, "a");
    let b = h.ast.new_string_with_token(Token::NAME, "b");
    a.add_child_to_front(&mut h.ast, b);
    let number = h.ast.new_number(3.0);
    let expr = h.ast.new_node_with_child(Token::EXPR_RESULT, number);
    let first = script(&mut h.ast, expr);
    let var = h.ast.new_node_with_child(Token::VAR, a);
    let second = script(&mut h.ast, var);
    let test_cases = [("3;", first), ("var a = b;", second)];
    for (code, node) in test_cases {
        let found = h.parse(code);
        assert_node_equality(&h.ast, node, found);
    }
}

// port: ParserTest#testPrivateProperty_invalid_lineNumber
#[test]
fn test_private_property_invalid_line_number() {
    let mut h = Harness::new();
    #[derive(Default)]
    struct Reporter {
        errors: Vec<String>,
        lines: Vec<i32>,
        cols: Vec<i32>,
    }
    impl ErrorReporter for Reporter {
        // port: ParserTest#testPrivateProperty_invalid_lineNumber (ErrorReporter.warning)
        fn warning(&mut self, _message: &str, _source_name: &str, _line: i32, _line_offset: i32) {}
        // port: ParserTest#testPrivateProperty_invalid_lineNumber (ErrorReporter.error)
        fn error(&mut self, message: &str, _source_name: &str, line: i32, line_offset: i32) {
            self.errors.push(message.to_owned());
            self.lines.push(line);
            self.cols.push(line_offset);
        }
    }
    let mut reporter = Reporter::default();
    let config = h.create_config();
    ParserRunner::parse(
        &mut h.ast,
        Arc::new(SimpleSourceFile::new("testcode", SourceKind::STRONG)),
        "const x = #foo;\nconst y = {\n  #pf: 1,\n};\nconst z = #bar;".into(),
        &config,
        &mut reporter,
    );
    assert_eq!(
        reporter.errors,
        vec![INVALID_PRIVATE_ID, INVALID_PRIVATE_ID, INVALID_PRIVATE_ID]
    );
    assert_eq!(reporter.lines, vec![1, 3, 5]);
    assert_eq!(reporter.cols, vec![10, 2, 10]);
}

// port: ParserTest#testJSDocAttachment1
#[test]
fn test_js_doc_attachment1() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/** @type {number} */var a;");
    let mut var_node = tmp1.get_first_child(&h.ast);
    assert_eq!(var_node.unwrap().get_token(&h.ast), Token::VAR);
    h.assert_node_has_jsdoc_info_with_js_type(var_node.unwrap(), h.types.number_type);
    let mut var_name_node = var_node.unwrap().get_first_child(&h.ast);
    assert_eq!(var_name_node.unwrap().get_token(&h.ast), Token::NAME);
    assert!(var_name_node.unwrap().get_jsdoc_info(&h.ast).is_none());
    h.strict_mode = StrictMode::SLOPPY;
    h.expect_features(&[Feature::LET_DECLARATIONS]);
    let tmp2 = h.parse("/** @type {number} */let a;");
    let mut let_node = tmp2.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    h.assert_node_has_jsdoc_info_with_js_type(let_node.unwrap(), h.types.number_type);
    let mut let_name_node = let_node.unwrap().get_first_child(&h.ast);
    assert_eq!(let_name_node.unwrap().get_token(&h.ast), Token::NAME);
    assert!(let_name_node.unwrap().get_jsdoc_info(&h.ast).is_none());
    h.expect_features(&[Feature::CONST_DECLARATIONS]);
    let tmp3 = h.parse("/** @type {number} */const a = 0;");
    let mut const_node = tmp3.get_first_child(&h.ast);
    assert_eq!(const_node.unwrap().get_token(&h.ast), Token::CONST);
    h.assert_node_has_jsdoc_info_with_js_type(const_node.unwrap(), h.types.number_type);
    let mut const_name_node = const_node.unwrap().get_first_child(&h.ast);
    assert_eq!(const_name_node.unwrap().get_token(&h.ast), Token::NAME);
    assert!(const_name_node.unwrap().get_jsdoc_info(&h.ast).is_none());
}

// port: ParserTest#testJSDocAttachment2
#[test]
fn test_js_doc_attachment2() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/** @type {number} */var a,b;");
    let mut var_node = tmp1.get_first_child(&h.ast);
    assert_eq!(var_node.unwrap().get_token(&h.ast), Token::VAR);
    h.assert_node_has_jsdoc_info_with_js_type(var_node.unwrap(), h.types.number_type);
    let mut name_node1 = var_node.unwrap().get_first_child(&h.ast);
    assert_eq!(name_node1.unwrap().get_token(&h.ast), Token::NAME);
    assert!(name_node1.unwrap().get_jsdoc_info(&h.ast).is_none());
    let mut name_node2 = name_node1.unwrap().get_next(&h.ast);
    assert_eq!(name_node2.unwrap().get_token(&h.ast), Token::NAME);
    assert!(name_node2.unwrap().get_jsdoc_info(&h.ast).is_none());
}

// port: ParserTest#testJSDocAttachment3
#[test]
fn test_js_doc_attachment3() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/** @type {number} */goog.FOO = 5;");
    let mut assign_node = tmp1.get_first_first_child(&h.ast);
    assert_eq!(assign_node.unwrap().get_token(&h.ast), Token::ASSIGN);
    h.assert_node_has_jsdoc_info_with_js_type(assign_node.unwrap(), h.types.number_type);
}

// port: ParserTest#testJSDocAttachment4
#[test]
fn test_js_doc_attachment4() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var a, /** @define {number} */ b = 5;");
    let mut var_node = tmp1.get_first_child(&h.ast);
    assert_eq!(var_node.unwrap().get_token(&h.ast), Token::VAR);
    assert!(var_node.unwrap().get_jsdoc_info(&h.ast).is_none());
    let mut a = var_node.unwrap().get_first_child(&h.ast);
    assert!(a.unwrap().get_jsdoc_info(&h.ast).is_none());
    let mut b = a.unwrap().get_next(&h.ast);
    let mut info = b.unwrap().get_jsdoc_info(&h.ast);
    assert!(info.is_some());
    assert!(info.as_ref().unwrap().is_define());
    h.types.assert_type_equals_expression(
        h.types.number_type,
        &info.as_ref().unwrap().get_type().unwrap(),
    );
}

// port: ParserTest#testJSDocAttachment5
#[test]
fn test_js_doc_attachment5() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var /** @type {number} */a, /** @define {number} */b = 5;");
    let mut var_node = tmp1.get_first_child(&h.ast);
    assert_eq!(var_node.unwrap().get_token(&h.ast), Token::VAR);
    assert!(var_node.unwrap().get_jsdoc_info(&h.ast).is_none());
    let mut a = var_node.unwrap().get_first_child(&h.ast);
    assert!(a.unwrap().get_jsdoc_info(&h.ast).is_some());
    let mut info = a.unwrap().get_jsdoc_info(&h.ast);
    assert!(info.is_some());
    assert!(!info.as_ref().unwrap().is_define());
    h.types.assert_type_equals_expression(
        h.types.number_type,
        &info.as_ref().unwrap().get_type().unwrap(),
    );
    let mut b = a.unwrap().get_next(&h.ast);
    info = b.unwrap().get_jsdoc_info(&h.ast);
    assert!(info.is_some());
    assert!(info.as_ref().unwrap().is_define());
    h.types.assert_type_equals_expression(
        h.types.number_type,
        &info.as_ref().unwrap().get_type().unwrap(),
    );
}

// port: ParserTest#testJSDocAttachment6
#[test]
fn test_js_doc_attachment6() {
    let mut h = Harness::new();
    let tmp1 = h.parse(
        "var a = /** @param {number} index */5;\n/** @return {boolean} */function f(index){}\n",
    );
    let mut function_node = tmp1.get_second_child(&h.ast);
    assert_eq!(function_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut info = function_node.unwrap().get_jsdoc_info(&h.ast);
    assert!(info.is_some());
    assert!(!info.as_ref().unwrap().has_parameter("index"));
    assert!(info.as_ref().unwrap().has_return_type());
    h.types.assert_type_equals_expression(
        h.types.boolean_type,
        &info.as_ref().unwrap().get_return_type().unwrap(),
    );
}

// port: ParserTest#testJSDocAttachment11
#[test]
fn test_js_doc_attachment11() {
    let mut h = Harness::new();
    let tmp1 = h.parse("/** @type {{x : number, 'y' : string, z}} */var a;");
    let mut var_node = tmp1.get_first_child(&h.ast);
    assert_eq!(var_node.unwrap().get_token(&h.ast), Token::VAR);
    let mut tmp2 = h.types.create_record_type_builder();
    tmp2.add_property("x", h.types.number_type, None);
    tmp2.add_property("y", h.types.string_type, None);
    tmp2.add_property("z", h.types.unknown_type, None);
    let tmp3 = tmp2.build(&mut h.types.reg, &h.types.ast);
    h.assert_node_has_jsdoc_info_with_js_type(var_node.unwrap(), tmp3);
    let mut name_node = var_node.unwrap().get_first_child(&h.ast);
    assert_eq!(name_node.unwrap().get_token(&h.ast), Token::NAME);
    assert!(name_node.unwrap().get_jsdoc_info(&h.ast).is_none());
}

// port: ParserTest#testInlineJSDocAttachmentToVar
#[test]
fn test_inline_js_doc_attachment_to_var() {
    let mut h = Harness::new();
    let tmp1 = h.parse("let /** string */ x = 'a';");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut info = let_node
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap()
        .get_jsdoc_info(&h.ast);
    assert!(info.is_some());
    h.types.assert_type_equals_expression(
        h.types.string_type,
        &info.as_ref().unwrap().get_type().unwrap(),
    );
}

// port: ParserTest#testInlineJSDocAttachmentToObjPatNormalProp
#[test]
fn test_inline_js_doc_attachment_to_obj_pat_normal_prop() {
    let mut h = Harness::new();
    let tmp1 = h.parse("let { normalProp: /** string */ normalPropTarget } = {};");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut destructuring_lhs = let_node.unwrap().get_first_child(&h.ast);
    let mut object_pattern = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut normal_prop = object_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(normal_prop.unwrap().get_token(&h.ast), Token::STRING_KEY);
    let mut normal_prop_target = normal_prop.unwrap().get_only_child(&h.ast);
    h.assert_node_has_jsdoc_info_with_js_type(normal_prop_target, h.types.string_type);
}

// port: ParserTest#testInlineJSDocAttachmentToObjPatShorthandProp
#[test]
fn test_inline_js_doc_attachment_to_obj_pat_shorthand_prop() {
    let mut h = Harness::new();
    let tmp1 = h.parse("let { /** string */ shorthandProp } = {};");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut destructuring_lhs = let_node.unwrap().get_first_child(&h.ast);
    let mut object_pattern = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut shorthand_prop = object_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(shorthand_prop.unwrap().get_token(&h.ast), Token::STRING_KEY);
    let mut shorthand_prop_target = shorthand_prop.unwrap().get_only_child(&h.ast);
    h.assert_node_has_jsdoc_info_with_js_type(shorthand_prop_target, h.types.string_type);
}

// port: ParserTest#testInlineJSDocAttachmentToObjPatNormalPropWithDefault
#[test]
fn test_inline_js_doc_attachment_to_obj_pat_normal_prop_with_default() {
    let mut h = Harness::new();
    let tmp1 =
        h.parse("let { normalPropWithDefault: /** string */ normalPropWithDefault = 'hi' } = {};");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut destructuring_lhs = let_node.unwrap().get_first_child(&h.ast);
    let mut object_pattern = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut normal_prop_with_default = object_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(
        normal_prop_with_default.unwrap().get_token(&h.ast),
        Token::STRING_KEY
    );
    let mut normal_prop_default_value = normal_prop_with_default.unwrap().get_only_child(&h.ast);
    assert_eq!(
        normal_prop_default_value.get_token(&h.ast),
        Token::DEFAULT_VALUE
    );
    let mut normal_prop_with_default_target = normal_prop_default_value.get_first_child(&h.ast);
    h.assert_node_has_jsdoc_info_with_js_type(
        normal_prop_with_default_target.unwrap(),
        h.types.string_type,
    );
}

// port: ParserTest#testInlineJSDocAttachmentToObjPatShorthandWithDefault
#[test]
fn test_inline_js_doc_attachment_to_obj_pat_shorthand_with_default() {
    let mut h = Harness::new();
    let tmp1 = h.parse("let { /** string */ shorthandPropWithDefault = 'lo' } = {};");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut destructuring_lhs = let_node.unwrap().get_first_child(&h.ast);
    let mut object_pattern = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut shorthand_prop_with_default = object_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(
        shorthand_prop_with_default.unwrap().get_token(&h.ast),
        Token::STRING_KEY
    );
    let mut shorthand_prop_default_value =
        shorthand_prop_with_default.unwrap().get_only_child(&h.ast);
    assert_eq!(
        shorthand_prop_default_value.get_token(&h.ast),
        Token::DEFAULT_VALUE
    );
    let mut shorthand_prop_with_default_target =
        shorthand_prop_default_value.get_first_child(&h.ast);
    h.assert_node_has_jsdoc_info_with_js_type(
        shorthand_prop_with_default_target.unwrap(),
        h.types.string_type,
    );
}

// port: ParserTest#testInlineJSDocAttachmentToObjPatComputedProp
#[test]
fn test_inline_js_doc_attachment_to_obj_pat_computed_prop() {
    let mut h = Harness::new();
    let tmp1 = h.parse("let { ['computedProp']: /** string */ computedProp } = {};");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut destructuring_lhs = let_node.unwrap().get_first_child(&h.ast);
    let mut object_pattern = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut computed_prop = object_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(
        computed_prop.unwrap().get_token(&h.ast),
        Token::COMPUTED_PROP
    );
    let mut computed_prop_target = computed_prop.unwrap().get_second_child(&h.ast);
    h.assert_node_has_jsdoc_info_with_js_type(computed_prop_target.unwrap(), h.types.string_type);
}

// port: ParserTest#testInlineJSDocAttachmentToObjPatComputedPropWithDefault
#[test]
fn test_inline_js_doc_attachment_to_obj_pat_computed_prop_with_default() {
    let mut h = Harness::new();
    let tmp1 =
        h.parse("let { ['computedPropWithDefault']: /** string */ computedProp = 'go' } = {};");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut destructuring_lhs = let_node.unwrap().get_first_child(&h.ast);
    let mut object_pattern = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut computed_prop_with_default = object_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(
        computed_prop_with_default.unwrap().get_token(&h.ast),
        Token::COMPUTED_PROP
    );
    let mut computed_prop_default_value =
        computed_prop_with_default.unwrap().get_second_child(&h.ast);
    assert_eq!(
        computed_prop_default_value.unwrap().get_token(&h.ast),
        Token::DEFAULT_VALUE
    );
    let mut computed_prop_with_default_target =
        computed_prop_default_value.unwrap().get_first_child(&h.ast);
    h.assert_node_has_jsdoc_info_with_js_type(
        computed_prop_with_default_target.unwrap(),
        h.types.string_type,
    );
}

// port: ParserTest#testInlineJSDocAttachmentToObjPatNormalPropWithQualifiedName
#[test]
fn test_inline_js_doc_attachment_to_obj_pat_normal_prop_with_qualified_name() {
    let mut h = Harness::new();
    let tmp1 = h.parse("({ normalProp: /** string */ ns.normalPropTarget } = {});");
    let mut expr_result = tmp1.get_first_child(&h.ast);
    let mut assign_node = expr_result.unwrap().get_first_child(&h.ast);
    assert_eq!(assign_node.unwrap().get_token(&h.ast), Token::ASSIGN);
    let mut object_pattern = assign_node.unwrap().get_first_child(&h.ast);
    let mut normal_prop = object_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(normal_prop.unwrap().get_token(&h.ast), Token::STRING_KEY);
    let mut ns_normal_prop_target = normal_prop.unwrap().get_only_child(&h.ast);
    h.assert_node_has_jsdoc_info_with_js_type(ns_normal_prop_target, h.types.string_type);
}

// port: ParserTest#testInlineJSDocAttachmentToObjPatNormalPropWithQualifiedNameWithDefault
#[test]
fn test_inline_js_doc_attachment_to_obj_pat_normal_prop_with_qualified_name_with_default() {
    let mut h = Harness::new();
    let tmp1 = h.parse("({ normalProp: /** string */ ns.normalPropTarget = 'foo' } = {});");
    let mut expr_result = tmp1.get_first_child(&h.ast);
    let mut assign_node = expr_result.unwrap().get_first_child(&h.ast);
    assert_eq!(assign_node.unwrap().get_token(&h.ast), Token::ASSIGN);
    let mut object_pattern = assign_node.unwrap().get_first_child(&h.ast);
    let mut normal_prop = object_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(normal_prop.unwrap().get_token(&h.ast), Token::STRING_KEY);
    let mut default_value = normal_prop.unwrap().get_first_child(&h.ast);
    assert_eq!(
        default_value.unwrap().get_token(&h.ast),
        Token::DEFAULT_VALUE
    );
    let mut ns_normal_prop_target = default_value.unwrap().get_first_child(&h.ast);
    h.assert_node_has_jsdoc_info_with_js_type(ns_normal_prop_target.unwrap(), h.types.string_type);
}

// port: ParserTest#testInlineJSDocAttachmentToArrayPatElement
#[test]
fn test_inline_js_doc_attachment_to_array_pat_element() {
    let mut h = Harness::new();
    let tmp1 = h.parse("let [/** string */ x] = [];");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut destructuring_lhs = let_node.unwrap().get_first_child(&h.ast);
    let mut array_pattern = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut x_var_name = array_pattern.unwrap().get_first_child(&h.ast);
    h.assert_node_has_jsdoc_info_with_js_type(x_var_name.unwrap(), h.types.string_type);
}

// port: ParserTest#testInlineJSDocAttachmentToArrayPatElementWithDefault
#[test]
fn test_inline_js_doc_attachment_to_array_pat_element_with_default() {
    let mut h = Harness::new();
    let tmp1 = h.parse("let [/** string */ x = 'hi'] = [];");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut destructuring_lhs = let_node.unwrap().get_first_child(&h.ast);
    let mut array_pattern = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut default_value = array_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(
        default_value.unwrap().get_token(&h.ast),
        Token::DEFAULT_VALUE
    );
    let mut x_var_name = default_value.unwrap().get_first_child(&h.ast);
    h.assert_node_has_jsdoc_info_with_js_type(x_var_name.unwrap(), h.types.string_type);
}

// port: ParserTest#testInlineJSDocAttachmentToArrayPatElementQualifiedName
#[test]
fn test_inline_js_doc_attachment_to_array_pat_element_qualified_name() {
    let mut h = Harness::new();
    let tmp1 = h.parse("[/** string */ x.y.z] = [];");
    let mut expr_result = tmp1.get_first_child(&h.ast);
    let mut assign_node = expr_result.unwrap().get_first_child(&h.ast);
    assert_eq!(assign_node.unwrap().get_token(&h.ast), Token::ASSIGN);
    let mut array_pattern = assign_node.unwrap().get_first_child(&h.ast);
    assert_eq!(
        array_pattern.unwrap().get_token(&h.ast),
        Token::ARRAY_PATTERN
    );
    let mut x_yz_name = array_pattern.unwrap().get_first_child(&h.ast);
    h.assert_node_has_jsdoc_info_with_js_type(x_yz_name.unwrap(), h.types.string_type);
}

// port: ParserTest#testInlineJSDocAttachmentToArrayPatElementQualifiedNameWithDefault
#[test]
fn test_inline_js_doc_attachment_to_array_pat_element_qualified_name_with_default() {
    let mut h = Harness::new();
    let tmp1 = h.parse("[/** string */ x.y.z = 'foo'] = [];");
    let mut expr_result = tmp1.get_first_child(&h.ast);
    let mut assign_node = expr_result.unwrap().get_first_child(&h.ast);
    assert_eq!(assign_node.unwrap().get_token(&h.ast), Token::ASSIGN);
    let mut array_pattern = assign_node.unwrap().get_first_child(&h.ast);
    assert_eq!(
        array_pattern.unwrap().get_token(&h.ast),
        Token::ARRAY_PATTERN
    );
    let mut default_value = array_pattern.unwrap().get_only_child(&h.ast);
    assert_eq!(default_value.get_token(&h.ast), Token::DEFAULT_VALUE);
    let mut x_yz_name = default_value.get_first_child(&h.ast);
    h.assert_node_has_jsdoc_info_with_js_type(x_yz_name.unwrap(), h.types.string_type);
}

// port: ParserTest#testInlineJSDocAttachmentToArrayPatElementAfterElision
#[test]
fn test_inline_js_doc_attachment_to_array_pat_element_after_elision() {
    let mut h = Harness::new();
    let tmp1 = h.parse("let [, /** string */ x] = [];");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut destructuring_lhs = let_node.unwrap().get_first_child(&h.ast);
    let mut array_pattern = destructuring_lhs.unwrap().get_first_child(&h.ast);
    let mut empty = array_pattern.unwrap().get_first_child(&h.ast);
    assert_eq!(empty.unwrap().get_token(&h.ast), Token::EMPTY);
    assert_eq!(empty.unwrap().get_charno(&h.ast), 5);
    assert_eq!(empty.unwrap().get_length(&h.ast), 1);
    let mut x_var_name = array_pattern.unwrap().get_second_child(&h.ast);
    h.assert_node_has_jsdoc_info_with_js_type(x_var_name.unwrap(), h.types.string_type);
}

// port: ParserTest#testJSDocAttachmentToObjLitNormalPropKey
#[test]
fn test_js_doc_attachment_to_obj_lit_normal_prop_key() {
    let mut h = Harness::new();
    let tmp1 = h.parse("let x = { /** @type {string} */ normalProp: normalProp };");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut x_node = let_node.unwrap().get_first_child(&h.ast);
    let mut object_lit = x_node.unwrap().get_first_child(&h.ast);
    let mut normal_prop = object_lit.unwrap().get_first_child(&h.ast);
    assert_eq!(normal_prop.unwrap().get_token(&h.ast), Token::STRING_KEY);
    h.assert_node_has_jsdoc_info_with_js_type(normal_prop.unwrap(), h.types.string_type);
}

// port: ParserTest#testJSDocAttachmentToObjLitShorthandProp
#[test]
fn test_js_doc_attachment_to_obj_lit_shorthand_prop() {
    let mut h = Harness::new();
    let tmp1 = h.parse("let x = { /** @type {string} */ shorthandProp };");
    let mut let_node = tmp1.get_first_child(&h.ast);
    assert_eq!(let_node.unwrap().get_token(&h.ast), Token::LET);
    let mut x_node = let_node.unwrap().get_first_child(&h.ast);
    let mut object_lit = x_node.unwrap().get_first_child(&h.ast);
    let mut shorthand_prop_key = object_lit.unwrap().get_first_child(&h.ast);
    assert_eq!(
        shorthand_prop_key.unwrap().get_token(&h.ast),
        Token::STRING_KEY
    );
    h.assert_node_has_jsdoc_info_with_js_type(shorthand_prop_key.unwrap(), h.types.string_type);
    let mut shorthand_prop_target = shorthand_prop_key.unwrap().get_only_child(&h.ast);
    assert_node_has_no_jsdoc_info(&h.ast, shorthand_prop_target);
}

// port: ParserTest#testInlineJSDocAttachment1
#[test]
fn test_inline_js_doc_attachment1() {
    let mut h = Harness::new();
    let tmp1 = h.parse("function f(/** string */ x) {}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut info = fn_node
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap()
        .get_jsdoc_info(&h.ast);
    assert!(info.is_some());
    h.types.assert_type_equals_expression(
        h.types.string_type,
        &info.as_ref().unwrap().get_type().unwrap(),
    );
}

// port: ParserTest#testBothJSDocAndNonJSDocCommentsGetAttached
#[test]
fn test_both_js_doc_and_non_js_doc_comments_get_attached() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(/** string */ // nonJSDoc\n x) {}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut x_node = fn_node
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    let mut info = x_node.unwrap().get_jsdoc_info(&h.ast);
    assert!(info.is_some());
    h.assert_node_has_jsdoc_info_with_js_type(x_node.unwrap(), h.types.string_type);
    assert!(
        x_node
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("// nonJSDoc")
            >= 0
    );
}

// port: ParserTest#testBothNonJSDocAndJSDocCommentsGetAttached
#[test]
fn test_both_non_js_doc_and_js_doc_comments_get_attached() {
    let mut h = Harness::new();
    h.is_ide_mode = true;
    h.parsing_mode = JsDocParsing::INCLUDE_ALL_COMMENTS;
    let tmp1 = h.parse("function f(// nonJSDoc\n /** string */ x) {}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut x_node = fn_node
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast);
    let mut info = x_node.unwrap().get_jsdoc_info(&h.ast);
    assert!(info.is_some());
    h.assert_node_has_jsdoc_info_with_js_type(x_node.unwrap(), h.types.string_type);
    assert!(
        x_node
            .unwrap()
            .get_non_jsdoc_comment_string(&h.ast)
            .index_of("// nonJSDoc")
            >= 0
    );
}

// port: ParserTest#testInlineJSDocAttachment2
#[test]
fn test_inline_js_doc_attachment2() {
    let mut h = Harness::new();
    let tmp1 = h.parse("function f(/** ? */ x) {}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut info = fn_node
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap()
        .get_jsdoc_info(&h.ast);
    assert!(info.is_some());
    h.types.assert_type_equals_expression(
        h.types.unknown_type,
        &info.as_ref().unwrap().get_type().unwrap(),
    );
}

// port: ParserTest#testInlineJSDocAttachment5
#[test]
fn test_inline_js_doc_attachment5() {
    let mut h = Harness::new();
    let tmp1 = h.parse("var /** string */ x = 'asdf';");
    let mut vardecl = tmp1.get_first_child(&h.ast);
    let mut info = vardecl
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap()
        .get_jsdoc_info(&h.ast);
    assert!(info.is_some());
    assert!(info.as_ref().unwrap().has_type());
    h.types.assert_type_equals_expression(
        h.types.string_type,
        &info.as_ref().unwrap().get_type().unwrap(),
    );
}

// port: ParserTest#testInlineJSDocAttachment6
#[test]
fn test_inline_js_doc_attachment6() {
    let mut h = Harness::new();
    let tmp1 = h.parse("function f(/** {attr: number} */ x) {}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut info = fn_node
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap()
        .get_jsdoc_info(&h.ast);
    assert!(info.is_some());
    let mut tmp2 = h.types.create_record_type_builder();
    tmp2.add_property("attr", h.types.number_type, None);
    let tmp3 = tmp2.build(&mut h.types.reg, &h.types.ast);
    h.types
        .assert_type_equals_expression(tmp3, &info.as_ref().unwrap().get_type().unwrap());
}

// port: ParserTest#testInlineJSDocReturnType
#[test]
fn test_inline_js_doc_return_type() {
    let mut h = Harness::new();
    let tmp1 = h.parse("function /** string */ f(x) {}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut info = fn_node
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap()
        .get_jsdoc_info(&h.ast);
    assert!(info.as_ref().unwrap().has_type());
    h.types.assert_type_equals_expression(
        h.types.string_type,
        &info.as_ref().unwrap().get_type().unwrap(),
    );
}

// port: ParserTest#testInlineJSDocReturnType_generator1
#[test]
fn test_inline_js_doc_return_type_generator1() {
    let mut h = Harness::new();
    let tmp1 = h.parse("function * /** string */ f(x) {}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut info = fn_node
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap()
        .get_jsdoc_info(&h.ast);
    assert!(info.as_ref().unwrap().has_type());
    h.types.assert_type_equals_expression(
        h.types.string_type,
        &info.as_ref().unwrap().get_type().unwrap(),
    );
}

// port: ParserTest#testInlineJSDocReturnType_generator2
#[test]
fn test_inline_js_doc_return_type_generator2() {
    let mut h = Harness::new();
    let tmp1 = h.parse("function /** string */ *f(x) {}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut info = fn_node
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap()
        .get_jsdoc_info(&h.ast);
    assert!(info.as_ref().unwrap().has_type());
    h.types.assert_type_equals_expression(
        h.types.string_type,
        &info.as_ref().unwrap().get_type().unwrap(),
    );
}

// port: ParserTest#testInlineJSDocReturnType_async
#[test]
fn test_inline_js_doc_return_type_async() {
    let mut h = Harness::new();
    let tmp1 = h.parse("async function /** string */ f(x) {}");
    let mut fn_node = tmp1.get_first_child(&h.ast);
    assert_eq!(fn_node.unwrap().get_token(&h.ast), Token::FUNCTION);
    let mut info = fn_node
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap()
        .get_jsdoc_info(&h.ast);
    assert!(info.as_ref().unwrap().has_type());
    h.types.assert_type_equals_expression(
        h.types.string_type,
        &info.as_ref().unwrap().get_type().unwrap(),
    );
}

// port: ParserTest#testDefaultParameterInlineJSDoc
#[test]
fn test_default_parameter_inline_js_doc() {
    let mut h = Harness::new();
    h.expect_features(&[Feature::DEFAULT_PARAMETERS]);
    let tmp1 = h.parse("function f(/** number */ a = 0) {}");
    let mut function_node = tmp1.get_first_child(&h.ast);
    let mut parameter_list = function_node.unwrap().get_second_child(&h.ast);
    let mut default_value = parameter_list.unwrap().get_first_child(&h.ast);
    assert_eq!(
        default_value.unwrap().get_token(&h.ast),
        Token::DEFAULT_VALUE
    );
    let mut a_name = default_value.unwrap().get_first_child(&h.ast);
    assert_eq!(a_name.unwrap().get_token(&h.ast), Token::NAME);
    h.assert_node_has_jsdoc_info_with_js_type(a_name.unwrap(), h.types.number_type);
}

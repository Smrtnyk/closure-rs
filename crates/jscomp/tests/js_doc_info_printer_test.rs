/*
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
//   test/com/google/javascript/jscomp/JSDocInfoPrinterTest.java.

#![allow(unused_mut)]
use closure_jscomp::js_doc_info_printer::JSDocInfoPrinter;
use closure_parsing::js_doc_info_parser::JsDocInfoParser;
use closure_rhino::fx_hash::IndexSet;
use closure_rhino::{
    ir::IR, js_type_expression::JSTypeExpression, jsdoc_info::JSDocInfo, node::Ast, token::Token,
};
use std::sync::Arc;

// port: JSDocInfoPrinterTest#testBasic
#[test]
fn test_basic() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_constancy();
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(js_doc_info_printer.print(&ast, &info), "/** @const */ ");
    builder.record_constructor();
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @constructor */ "
    );
    builder.record_suppressions(&IndexSet::<_>::from_iter([
        "globalThis".into(),
        "uselessCode".into(),
    ]));
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @suppress {globalThis,uselessCode}\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testSuppressions
#[test]
fn test_suppressions() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_suppressions_with_description(
        &IndexSet::<_>::from_iter(["globalThis".into(), "uselessCode".into()]),
        "Common description.",
    );
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @suppress {globalThis,uselessCode} Common description.\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testSuppressions_multipleLineDescription
#[test]
fn test_suppressions_multiple_line_description() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_suppressions_with_description(
        &IndexSet::<_>::from_iter(["globalThis".into(), "uselessCode".into()]),
        "Common description.\n More on another line.",
    );
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @suppress {globalThis,uselessCode} Common description.\n * More on another line.\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testSuppressions_multiple
#[test]
fn test_suppressions_multiple() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_suppressions_with_description(
        &IndexSet::<_>::from_iter(["globalThis".into(), "uselessCode".into()]),
        "Common description.",
    );
    builder.record_suppressions(&IndexSet::<_>::from_iter(["const".into()])); // has no description

    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @suppress {globalThis,uselessCode} Common description.\n * @suppress {const}\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testSuppressions_multiple_printOrder
#[test]
fn test_suppressions_multiple_print_order() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_suppressions(&IndexSet::<_>::from_iter(["const".into()])); // has no description
    builder.record_suppressions_with_description(
        &IndexSet::<_>::from_iter(["uselessCode".into(), "globalThis".into()]),
        "Common description.",
    );

    let mut info = builder.build_and_reset().unwrap();
    // @suppress printed in order in which it is recorded(parsed)
    // warnings inside a suppress printed in natural order for consistency
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @suppress {const}\n * @suppress {globalThis,uselessCode} Common description.\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testFinal
#[test]
fn test_final() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_finality();
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(js_doc_info_printer.print(&ast, &info), "/** @final */ ");
}

// port: JSDocInfoPrinterTest#testDescTag
#[test]
fn test_desc_tag() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_description("foo");
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @desc foo\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testMultilineDescTag
#[test]
fn test_multiline_desc_tag() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_description("foo\nbar");
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @desc foo\n * bar\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testRecordTag
#[test]
fn test_record_tag() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_implicit_match();
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(js_doc_info_printer.print(&ast, &info), "/** @record */ ");
}

// port: JSDocInfoPrinterTest#testTemplate
#[test]
fn test_template() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_template_type_name(&mut ast, "T");
    builder.record_template_type_name(&mut ast, "U");

    let mut info = builder.build_and_reset().unwrap();

    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @template T\n * @template U\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testTemplateBound_single
#[test]
fn test_template_bound_single() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "!Array<number>").unwrap(),
        "",
    )));
    builder.record_template_type_name_with_bound(&mut ast, "T", expr0);

    let mut info = builder.build_and_reset().unwrap();

    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @template {!Array<number>} T\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testTemplateBound_nullabilityIsPreserved
#[test]
fn test_template_bound_nullability_is_preserved() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "?Array<number>").unwrap(),
        "",
    )));
    builder.record_template_type_name_with_bound(&mut ast, "T", expr0);

    let mut info = builder.build_and_reset().unwrap();

    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @template {?Array<number>} T\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testTemplateBound_explicitlyOnUnknown_isOmitted
#[test]
fn test_template_bound_explicitly_on_unknown_is_omitted() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "?").unwrap(),
        "",
    )));
    builder.record_template_type_name_with_bound(&mut ast, "T", expr0);

    let mut info = builder.build_and_reset().unwrap();

    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @template T\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testTemplatesBound_multiple
#[test]
fn test_templates_bound_multiple() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "!Array<number>").unwrap(),
        "",
    )));
    builder.record_template_type_name_with_bound(&mut ast, "T", expr0);
    let expr1 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "boolean").unwrap(),
        "",
    )));
    builder.record_template_type_name_with_bound(&mut ast, "U", expr1);

    let mut info = builder.build_and_reset().unwrap();

    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @template {!Array<number>} T\n * @template {boolean} U\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testTemplatesBound_mixedWithUnbounded
#[test]
fn test_templates_bound_mixed_with_unbounded() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "!Object").unwrap(),
        "",
    )));
    builder.record_template_type_name_with_bound(&mut ast, "T", expr0);
    builder.record_template_type_name(&mut ast, "S");
    builder.record_template_type_name(&mut ast, "R");
    let expr1 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "*").unwrap(),
        "",
    )));
    builder.record_template_type_name_with_bound(&mut ast, "U", expr1);
    builder.record_template_type_name(&mut ast, "Q");

    let mut info = builder.build_and_reset().unwrap();

    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @template {!Object} T\n * @template S\n * @template R\n * @template {*} U\n * @template Q\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testTypeTransformationLanguageTemplate
#[test]
fn test_type_transformation_language_template() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_type_transformation("T", IR::string(&mut ast, "Promise"));
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @template T := \"Promise\" =:\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testParam
#[test]
fn test_param() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "number").unwrap(),
        "<testParam>",
    )));
    builder.record_parameter("foo", expr0);
    let expr1 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "string").unwrap(),
        "<testParam>",
    )));
    builder.record_parameter("bar", expr1);
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @param {number} foo\n * @param {string} bar\n */\n"
    );

    let expr2 = Some(Arc::new(JSTypeExpression::new(
        {
            let child = IR::string(&mut ast, "number");
            ast.new_node_with_child(Token::EQUALS, child)
        },
        "<testParam>",
    )));
    builder.record_parameter("foo", expr2);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @param {number=} foo\n */\n"
    );

    let expr3 = Some(Arc::new(JSTypeExpression::new(
        {
            let child = IR::string(&mut ast, "number");
            ast.new_node_with_child(Token::ITER_REST, child)
        },
        "<testParam>",
    )));
    builder.record_parameter("foo", expr3);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @param {...number} foo\n */\n"
    );

    let expr4 = Some(Arc::new(JSTypeExpression::new(
        {
            let child = IR::empty(&mut ast);
            ast.new_node_with_child(Token::ITER_REST, child)
        },
        "<testParam>",
    )));
    builder.record_parameter("foo", expr4);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @param {...} foo\n */\n"
    );

    builder.record_parameter("foo", None);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @param foo\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testRecordTypes
#[test]
fn test_record_types() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "{foo: number}").unwrap(),
        "<testRecordTypes>",
    )));
    builder.record_type(expr0);
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {{foo:number}} */ "
    );

    let expr1 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "{foo}").unwrap(),
        "<testRecordTypes>",
    )));
    builder.record_type(expr1);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {{foo}} */ "
    );

    let expr2 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "{foo, bar}").unwrap(),
        "<testRecordTypes>",
    )));
    builder.record_type(expr2);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {{foo,bar}} */ "
    );

    let expr3 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "{foo: number, bar}").unwrap(),
        "<testRecordTypes>",
    )));
    builder.record_type(expr3);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {{foo:number,bar}} */ "
    );

    let expr4 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "{foo, bar: number}").unwrap(),
        "<testRecordTypes>",
    )));
    builder.record_type(expr4);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {{foo,bar:number}} */ "
    );
}

// port: JSDocInfoPrinterTest#testTypeof
#[test]
fn test_typeof() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "typeof foo").unwrap(),
        "<testTypeof>",
    )));
    builder.record_type(expr0);
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {typeof foo} */ "
    );
}

// port: JSDocInfoPrinterTest#testTypes
#[test]
fn test_types() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "number|string").unwrap(),
        "<testTypes>",
    )));
    builder.record_return_type(expr0);
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @return {(number|string)}\n */\n"
    );

    let expr1 = Some(Arc::new(JSTypeExpression::new(
        {
            let child = IR::string(&mut ast, "number");
            ast.new_node_with_child(Token::ITER_REST, child)
        },
        "<testTypes>",
    )));
    builder.record_parameter("foo", expr1);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @param {...number} foo\n */\n"
    );
    let expr2 = Some(Arc::new(JSTypeExpression::new(
        ast.new_node(Token::QMARK),
        "<testTypes>",
    )));
    builder.record_typedef(expr2);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @typedef {?} */ "
    );
    let expr3 = Some(Arc::new(JSTypeExpression::new(
        ast.new_node(Token::VOID),
        "<testTypes>",
    )));
    builder.record_type(expr3);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {void} */ "
    );

    // Object types
    let expr4 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "{foo:number,bar:string}").unwrap(),
        "<testTypes>",
    )));
    builder.record_enum_parameter_type(expr4);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @enum {{foo:number,bar:string}} */ "
    );

    let expr5 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "{foo:(number|string)}").unwrap(),
        "<testTypes>",
    )));
    builder.record_enum_parameter_type(expr5);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @enum {{foo:(number|string)}} */ "
    );

    // Nullable/non-Noneable types.
    let expr6 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "?Object").unwrap(),
        "<testTypes>",
    )));
    builder.record_type(expr6);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {?Object} */ "
    );
    let expr7 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "!Object").unwrap(),
        "<testTypes>",
    )));
    builder.record_type(expr7);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {!Object} */ "
    );

    // Array types
    let expr8 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "!Array<(number|string)>").unwrap(),
        "<testTypes>",
    )));
    builder.record_type(expr8);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {!Array<(number|string)>} */ "
    );
    let expr9 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "Array").unwrap(),
        "<testTypes>",
    )));
    builder.record_type(expr9);
    builder.record_inline_type();
    info = builder.build_and_reset().unwrap();
    assert_eq!(js_doc_info_printer.print(&ast, &info), "/** Array */ ");

    // Other template types
    let expr10 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "!Set<number|string>").unwrap(),
        "<testTypes>",
    )));
    builder.record_type(expr10);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {!Set<(number|string)>} */ "
    );
    let expr11 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "!Map<!Foo, !Bar<!Baz|string>>").unwrap(),
        "<testTypes>",
    )));
    builder.record_type(expr11);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {!Map<!Foo,!Bar<(!Baz|string)>>} */ "
    );
    let expr12 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "Map").unwrap(),
        "<testTypes>",
    )));
    builder.record_type(expr12);
    builder.record_inline_type();
    info = builder.build_and_reset().unwrap();
    assert_eq!(js_doc_info_printer.print(&ast, &info), "/** Map */ ");
}

// port: JSDocInfoPrinterTest#testInheritance
#[test]
fn test_inheritance() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "Foo").unwrap(),
        "<testInheritance>",
    )));
    builder.record_implemented_interface(&ast, expr0);
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @implements {Foo}\n */\n"
    );

    let expr1 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "!Foo").unwrap(),
        "<testInheritance>",
    )));
    builder.record_implemented_interface(&ast, expr1);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @implements {Foo}\n */\n"
    );

    let expr2 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "Foo").unwrap(),
        "<testInheritance>",
    )));
    builder.record_base_type(expr2);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @extends {Foo}\n */\n"
    );

    let expr3 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "!Foo").unwrap(),
        "<testInheritance>",
    )));
    builder.record_base_type(expr3);
    let expr4 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "Bar").unwrap(),
        "<testInheritance>",
    )));
    builder.record_implemented_interface(&ast, expr4);
    let expr5 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "Bar.Baz").unwrap(),
        "<testInheritance>",
    )));
    builder.record_implemented_interface(&ast, expr5);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @extends {Foo}\n * @implements {Bar}\n * @implements {Bar.Baz}\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testInterfaceInheritance
#[test]
fn test_interface_inheritance() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_interface();
    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "Foo").unwrap(),
        "<testInterfaceInheritance>",
    )));
    builder.record_extended_interface(&ast, expr0);
    let expr1 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "Bar").unwrap(),
        "<testInterfaceInheritance>",
    )));
    builder.record_extended_interface(&ast, expr1);
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @interface\n * @extends {Foo}\n * @extends {Bar}\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testFunctions
#[test]
fn test_functions() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "function()").unwrap(),
        "<testFunctions>",
    )));
    builder.record_type(expr0);
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {function()} */ "
    );

    let expr1 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "function(foo,bar)").unwrap(),
        "<testFunctions>",
    )));
    builder.record_type(expr1);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {function(foo,bar)} */ "
    );

    let expr2 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "function(foo):number").unwrap(),
        "<testFunctions>",
    )));
    builder.record_type(expr2);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {function(foo):number} */ "
    );

    let expr3 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "function(new:goog,number)").unwrap(),
        "<testFunctions>",
    )));
    builder.record_type(expr3);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {function(new:goog,number)} */ "
    );

    let expr4 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "function(this:number,...)").unwrap(),
        "<testFunctions>",
    )));
    builder.record_type(expr4);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {function(this:number,...)} */ "
    );

    let expr5 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "function(...number)").unwrap(),
        "<testFunctions>",
    )));
    builder.record_type(expr5);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {function(...number)} */ "
    );

    let expr6 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "function():void").unwrap(),
        "<testFunctions>",
    )));
    builder.record_type(expr6);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {function():void} */ "
    );

    let expr7 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "function():number").unwrap(),
        "<testFunctions>",
    )));
    builder.record_type(expr7);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {function():number} */ "
    );

    let expr8 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "function(string):number").unwrap(),
        "<testFunctions>",
    )));
    builder.record_type(expr8);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {function(string):number} */ "
    );

    let expr9 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "function(this:foo):?").unwrap(),
        "<testFunctions>",
    )));
    builder.record_type(expr9);
    info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @type {function(this:foo):?} */ "
    );
}

// port: JSDocInfoPrinterTest#testDefines
#[test]
fn test_defines() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "string").unwrap(),
        "<testDefines>",
    )));
    builder.record_define_type(expr0);
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @define {string} */ "
    );
}

// port: JSDocInfoPrinterTest#testConstDefines
#[test]
fn test_const_defines() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "string").unwrap(),
        "<testDefines>",
    )));
    builder.record_define_type(expr0);
    builder.record_constancy();
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @define {string} */ "
    );
}

// port: JSDocInfoPrinterTest#testBlockDescription
#[test]
fn test_block_description() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_block_description("Description of the thing");
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * Description of the thing\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testParamDescriptions
#[test]
fn test_param_descriptions() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "number").unwrap(),
        "<testParam>",
    )));
    builder.record_parameter("foo", expr0);
    let expr1 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "string").unwrap(),
        "<testParam>",
    )));
    builder.record_parameter("bar", expr1);
    // The parser will retain leading whitespace for descriptions.
    builder.record_parameter_description("foo", " A number for foo");
    builder.record_parameter_description("bar", " A multline\n     description for bar");
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @param {number} foo A number for foo\n * @param {string} bar A multline\n *     description for bar\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testReturnDescription
#[test]
fn test_return_description() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "boolean").unwrap(),
        "<testReturn>",
    )));
    builder.record_return_type(expr0);
    builder.record_return_description("The return value");
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @return {boolean} The return value\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testAllDescriptions
#[test]
fn test_all_descriptions() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_block_description("Description of the thing");
    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "number").unwrap(),
        "<testParam>",
    )));
    builder.record_parameter("foo", expr0);
    let expr1 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "string").unwrap(),
        "<testParam>",
    )));
    builder.record_parameter("bar", expr1);
    builder.record_parameter_description("foo", " A number for foo");
    builder.record_parameter_description("bar", " A multline\n     description for bar");
    let expr2 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "boolean").unwrap(),
        "<testReturn>",
    )));
    builder.record_return_type(expr2);
    builder.record_return_description("The return value");
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * Description of the thing\n *\n * @param {number} foo A number for foo\n * @param {string} bar A multline\n *     description for bar\n * @return {boolean} The return value\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testDeprecated
#[test]
fn test_deprecated() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_deprecated();
    builder.record_deprecation_reason("See {@link otherClass} for more info.");
    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "string").unwrap(),
        "<testDeprecated>",
    )));
    builder.record_type(expr0);
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @type {string}\n * @deprecated See {@link otherClass} for more info.\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testDeprecated_noReason
#[test]
fn test_deprecated_no_reason() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_deprecated();
    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "string").unwrap(),
        "<testDeprecated>",
    )));
    builder.record_type(expr0);
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @type {string}\n * @deprecated\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testJSDocIsPopulated_withSeeReferenceAlone
#[test]
fn test_js_doc_is_populated_with_see_reference_alone() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_reference("SomeClassName for more details");
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @see SomeClassName for more details\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testJSDocIsPopulated_withAuthorAlone
#[test]
fn test_js_doc_is_populated_with_author_alone() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_author("John Doe.");
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @author John Doe.\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testSeeReferencePrintedWithOtherAnnotations
#[test]
fn test_see_reference_printed_with_other_annotations() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "string").unwrap(),
        "<testSee>",
    )));
    builder.record_type(expr0);
    builder.record_reference("SomeClassName for more details");
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @see SomeClassName for more details\n * @type {string}\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testAuthorReferencePrintedWithOtherAnnotations
#[test]
fn test_author_reference_printed_with_other_annotations() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    let expr0 = Some(Arc::new(JSTypeExpression::new(
        JsDocInfoParser::parse_type_string(&mut ast, "string").unwrap(),
        "<testAuthor>",
    )));
    builder.record_type(expr0);
    builder.record_author("John Doe.");
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @author John Doe.\n * @type {string}\n */\n"
    );
}

// port: JSDocInfoPrinterTest#testExterns
#[test]
fn test_externs() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_externs();
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(js_doc_info_printer.print(&ast, &info), "/** @externs */ ");
}

// port: JSDocInfoPrinterTest#testTypeSummary
#[test]
fn test_type_summary() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_type_summary();
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @typeSummary */ "
    );
}

// port: JSDocInfoPrinterTest#testExport
#[test]
fn test_export() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_export();
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(js_doc_info_printer.print(&ast, &info), "/** @export */ ");
}

// port: JSDocInfoPrinterTest#testAbstract
#[test]
fn test_abstract() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_abstract();
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(js_doc_info_printer.print(&ast, &info), "/** @abstract */ ");
}

// port: JSDocInfoPrinterTest#testImplicitCast
#[test]
fn test_implicit_cast() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_implicit_cast();
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @implicitCast */ "
    );
}

// port: JSDocInfoPrinterTest#testClosurePrimitive
#[test]
fn test_closure_primitive() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_closure_primitive_id("testPrimitive");
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @closurePrimitive {testPrimitive} */ "
    );
}

// port: JSDocInfoPrinterTest#testNoCollapse
#[test]
fn test_no_collapse() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_no_collapse();
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @nocollapse */ "
    );
}

// port: JSDocInfoPrinterTest#testNgInject
#[test]
fn test_ng_inject() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_ng_inject(true);
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(js_doc_info_printer.print(&ast, &info), "/** @ngInject */ ");
}

// port: JSDocInfoPrinterTest#testTsType
#[test]
fn test_ts_type() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_ts_type("():string");
    let mut info = builder.build_and_reset().unwrap();

    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @tsType ():string */ "
    );
}

// port: JSDocInfoPrinterTest#testTsType_multipleTsTypes
#[test]
fn test_ts_type_multiple_ts_types() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_ts_type("():string");
    builder.record_ts_type("(x:string):number");
    let mut info = builder.build_and_reset().unwrap();

    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/** @tsType ():string @tsType (x:string):number */ "
    );
}

// port: JSDocInfoPrinterTest#testPolymer
#[test]
fn test_polymer() {
    let mut ast = Ast::default();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let js_doc_info_printer = JSDocInfoPrinter::new_with_print_desc(false, true);

    builder.record_polymer();
    builder.record_polymer_behavior();
    builder.record_mixin_function();
    builder.record_mixin_class();
    builder.record_custom_element();
    let mut info = builder.build_and_reset().unwrap();
    assert_eq!(
        js_doc_info_printer.print(&ast, &info),
        "/**\n * @polymer\n * @polymerBehavior\n * @mixinFunction\n * @mixinClass\n * @customElement\n */\n"
    );
}

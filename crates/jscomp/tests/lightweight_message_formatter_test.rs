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
//   test/com/google/javascript/jscomp/LightweightMessageFormatterTest.java.

use closure_jscomp::{
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    lightweight_message_formatter::{LightweightMessageFormatter, LineNumberingFormatter},
    message_formatter::MessageFormatter,
    region::Region,
    simple_region::SimpleRegion,
    source_excerpt_provider::{ExcerptFormatter, SourceExcerpt, SourceExcerptProvider},
    sourcemap_mapping_placeholder::OriginalMapping,
};
use closure_rhino::{
    check_state,
    ir::IR,
    node::{Ast, NodeId},
};
use std::sync::Arc;
static FOO_TYPE: DiagnosticType = DiagnosticType::error("TEST_FOO", "error description here");
const ORIGINAL_SOURCE_FILE: &str = "original/source.html";
struct TestSource {
    source: String,
    original_source: Option<String>,
    end_line_number: i32,
}
impl SourceExcerptProvider for TestSource {
    // port: LightweightMessageFormatterTest.SourceExcerptProvider#getSourceLine
    fn get_source_line(&self, name: Option<&str>, _line: i32) -> Option<String> {
        if name == Some(ORIGINAL_SOURCE_FILE) {
            self.original_source.clone()
        } else {
            Some(self.source.clone())
        }
    }
    // port: LightweightMessageFormatterTest.SourceExcerptProvider#getSourceLines
    fn get_source_lines(
        &self,
        name: Option<&str>,
        line: i32,
        _length: i32,
    ) -> Option<Box<dyn Region>> {
        check_state!(
            self.end_line_number != -1,
            "Must provide the end of the source lines"
        );
        Some(Box::new(SimpleRegion::new(
            line,
            self.end_line_number,
            self.get_source_line(name, line).unwrap(),
        )))
    }
    // port: LightweightMessageFormatterTest.SourceExcerptProvider#getSourceRegion
    fn get_source_region(&self, _name: Option<&str>, _line: i32) -> Option<Box<dyn Region>> {
        panic!("")
    }
    // port: LightweightMessageFormatterTest.SourceExcerptProvider#getSourceMapping
    fn get_source_mapping(
        &self,
        _name: Option<&str>,
        _line: i32,
        _col: i32,
    ) -> Option<OriginalMapping> {
        self.original_source.as_ref().map(|_| {
            OriginalMapping::new_builder()
                .set_original_file(ORIGINAL_SOURCE_FILE)
                .set_line_number(3)
                .set_column_position(15)
                .build()
        })
    }
}
// port: LightweightMessageFormatterTest#source(String,String,int)
fn source(text: &str, original: Option<&str>, end: i32) -> Arc<dyn SourceExcerptProvider> {
    Arc::new(TestSource {
        source: text.into(),
        original_source: original.map(str::to_owned),
        end_line_number: end,
    })
}
// port: LightweightMessageFormatterTest#formatter(String,SourceExcerpt,int)
fn formatter_full(text: &str, end: i32) -> LightweightMessageFormatter {
    LightweightMessageFormatter::new_with_format(source(text, None, end), SourceExcerpt::FULL)
}
// port: LightweightMessageFormatterTest#formatter(String)
fn formatter(text: &str) -> LightweightMessageFormatter {
    LightweightMessageFormatter::new(source(text, None, -1))
}
// port: LightweightMessageFormatterTest#formatter(String,String)
fn formatter_original(text: &str, original: &str) -> LightweightMessageFormatter {
    LightweightMessageFormatter::new(source(text, Some(original), -1))
}
// port: LightweightMessageFormatterTest#format
fn format(region: Option<SimpleRegion>) -> Option<String> {
    LineNumberingFormatter.format_region(region.as_ref().map(|r| r as &dyn Region))
}
// port: LightweightMessageFormatterTest#region
fn region(start: i32, end: i32, source: &str) -> Option<SimpleRegion> {
    Some(SimpleRegion::new(start, end, source))
}
// port: LightweightMessageFormatterTest#testNull
#[test]
fn test_null() {
    assert_eq!(format(None), None);
}
// port: LightweightMessageFormatterTest#testOneLineRegion
#[test]
fn test_one_line_region() {
    assert_eq!(
        format(region(5, 5, "hello world")).as_deref(),
        Some("  5| hello world")
    );
}
// port: LightweightMessageFormatterTest#testTwoLineRegion
#[test]
fn test_two_line_region() {
    assert_eq!(
        format(region(5, 6, "hello world\nfoo bar")).as_deref(),
        Some("  5| hello world\n  6| foo bar")
    );
}
// port: LightweightMessageFormatterTest#testThreeLineRegionAcrossNumberRange
#[test]
fn test_three_line_region_across_number_range() {
    assert_eq!(
        format(region(9, 11, "hello world\nfoo bar\nanother one")).as_deref(),
        Some("   9| hello world\n  10| foo bar\n  11| another one")
    );
}
// port: LightweightMessageFormatterTest#testThreeLineRegionEmptyLine
#[test]
fn test_three_line_region_empty_line() {
    assert_eq!(
        format(region(7, 9, "hello world\n\nanother one")).as_deref(),
        Some("  7| hello world\n  8| \n  9| another one")
    );
}
// port: LightweightMessageFormatterTest#testOnlyOneEmptyLine
#[test]
fn test_only_one_empty_line() {
    assert_eq!(format(region(7, 7, "")), None);
}
// port: LightweightMessageFormatterTest#testTwoEmptyLines
#[test]
fn test_two_empty_lines() {
    assert_eq!(format(region(7, 8, "\n")).as_deref(), Some("  7| "));
}
// port: LightweightMessageFormatterTest#testThreeLineRemoveLastEmptyLine
#[test]
fn test_three_line_remove_last_empty_line() {
    assert_eq!(
        format(region(7, 9, "hello world\nfoobar\n")).as_deref(),
        Some("  7| hello world\n  8| foobar")
    );
}
// The Node#newString setup of the test nodes, which names Closure's Rhino-derived Node
// (MPL-1.1 / GPL-2.0-or-later), is in its own file.
#[path = "rhino/lightweight_message_formatter_test.rs"]
mod rhino;
use rhino::node;
// port: LightweightMessageFormatterTest#testFormatErrorSpaces
#[test]
fn test_format_error_spaces() {
    let mut ast = Ast::new();
    let n = node(&mut ast, "foobar", 5, 8, 6);
    let error = JSError::make(&ast, n, &FOO_TYPE, &[]);
    assert_eq!(
        formatter("    if (foobar) {").format_error(&ast, &error),
        "javascript/complex.js:5:8: ERROR - [TEST_FOO] error description here\n    if (foobar) {\n        ^^^^^^\n"
    );
}
// port: LightweightMessageFormatterTest#testFormatErrorTabs
#[test]
fn test_format_error_tabs() {
    let mut ast = Ast::new();
    let n = node(&mut ast, "foobar", 5, 6, 6);
    let error = JSError::make(&ast, n, &FOO_TYPE, &[]);
    assert_eq!(
        formatter("\t\tif (foobar) {").format_error(&ast, &error),
        "javascript/complex.js:5:6: ERROR - [TEST_FOO] error description here\n\t\tif (foobar) {\n\t\t    ^^^^^^\n"
    );
}
// port: LightweightMessageFormatterTest#testFormatErrorSpaceEndOfLine1
#[test]
fn test_format_error_space_end_of_line1() {
    let error = JSError::make_with_source_location("javascript/complex.js", 1, 10, &FOO_TYPE, &[]);
    assert_eq!(
        formatter("assert (1;").format_error(&Ast::new(), &error),
        "javascript/complex.js:1:10: ERROR - [TEST_FOO] error description here\nassert (1;\n          ^\n"
    );
}
// port: LightweightMessageFormatterTest#testFormatErrorSpaceEndOfLine2
#[test]
fn test_format_error_space_end_of_line2() {
    let error = JSError::make_with_source_location("javascript/complex.js", 6, 7, &FOO_TYPE, &[]);
    assert_eq!(
        formatter("if (foo").format_error(&Ast::new(), &error),
        "javascript/complex.js:6:7: ERROR - [TEST_FOO] error description here\nif (foo\n       ^\n"
    );
}
// port: LightweightMessageFormatterTest#testFormatErrorOriginalSource
#[test]
fn test_format_error_original_source() {
    let mut ast = Ast::new();
    let n = node(&mut ast, "foobar", 5, 8, 6);
    let error = JSError::make(&ast, n, &FOO_TYPE, &[]);
    assert_eq!(
        formatter_original("    if (foobar) {", "<div ng-show='(foo'>").format_error(&ast, &error),
        "javascript/complex.js:5:8: \nOriginally at:\noriginal/source.html:3:15: ERROR - [TEST_FOO] error description here\n<div ng-show='(foo'>\n               ^^^^^\n"
    );
}
// port: LightweightMessageFormatterTest#testMultiline_oneLineErrorMessage
#[test]
fn test_multiline_one_line_error_message() {
    let mut ast = Ast::new();
    let n = node(&mut ast, "foobar", 5, 8, 6);
    let error = JSError::make(&ast, n, &FOO_TYPE, &[]);
    assert_eq!(
        formatter_full("    if (foobar) {", 5).format_error(&ast, &error),
        "javascript/complex.js:5:8: ERROR - [TEST_FOO] error description here\n  5|     if (foobar) {\n             ^^^^^^\n"
    );
}
// port: LightweightMessageFormatterTest#testMultiline_twoLineErrorMessage
#[test]
fn test_multiline_two_line_error_message() {
    let mut ast = Ast::new();
    let foobar = IR::string(&mut ast, "foobar");
    let baz = IR::string(&mut ast, "baz");
    let n = IR::or(&mut ast, foobar, baz);
    n.set_lineno_charno(&mut ast, 5, 4);
    n.set_length(&mut ast, "foobar\n      || baz".len() as i32);
    n.set_source_file_for_testing(&mut ast, "javascript/complex.js");
    let error = JSError::make(&ast, n, &FOO_TYPE, &[]);
    assert_eq!(
        formatter_full("if (foobar\n      || baz) {", 6).format_error(&ast, &error),
        "javascript/complex.js:5:4: ERROR - [TEST_FOO] error description here\n  5| if (foobar\n         ^^^^^^\n  6|       || baz) {\n     ^^^^^^^^^^^^\n"
    );
}
// port: LightweightMessageFormatterTest#testMultiline_charNoOutOfBoundsCrashes
#[test]
fn test_multiline_char_no_out_of_bounds_crashes() {
    let mut ast = Ast::new();
    let n = node(&mut ast, "foobar", 5, 800, 6);
    let error = JSError::make(&ast, n, &FOO_TYPE, &[]);
    assert!(formatter_full("    if (foobar) {",5).format_error(&ast,&error).contains("javascript/complex.js:5:800: ERROR - [TEST_FOO] error description here\nSource excerpt could not be formatted. This may indicate a bug in the compiler."));
}
// port: LightweightMessageFormatterTest#testMultiline_charNoOutOfBoundsCrashes2
#[test]
fn test_multiline_char_no_out_of_bounds_crashes2() {
    let mut ast = Ast::new();
    let n = node(&mut ast, "foobar", 0, 2, 0);
    let error = JSError::make(&ast, n, &FOO_TYPE, &[]);
    assert!(formatter_full("b\n/** @export {(number|undefined)} */\n",5).format_error(&ast,&error).contains("javascript/complex.js: ERROR - [TEST_FOO] error description here\nSource excerpt could not be formatted. This may indicate a bug in the compiler."));
}
// port: LightweightMessageFormatterTest#testMultiline_longTruncatedErrorMessage
#[test]
fn test_multiline_long_truncated_error_message() {
    let mut ast = Ast::new();
    let a = IR::name(&mut ast, "a");
    let n = IR::getprop_with_more_props(&mut ast, a, "b", &["c".into(), "d".into(), "e".into()]);
    n.set_lineno_charno(&mut ast, 8, 0);
    n.set_length(&mut ast, "a\n .b\n .c\n .d\n .e".len() as i32);
    n.set_source_file_for_testing(&mut ast, "javascript/complex.js");
    let error = JSError::make(&ast, n, &FOO_TYPE, &[]);
    assert_eq!(
        formatter_full("a\n .b\n .c\n .d\n .e + 1;", 12).format_error(&ast, &error),
        "javascript/complex.js:8:0: ERROR - [TEST_FOO] error description here\n   8| a\n      ^\n   9|  .b\n      ^^^\n...\n  11|  .d\n      ^^^\n  12|  .e + 1;\n      ^^^\n"
    );
}
// port: LightweightMessageFormatterTest#testMultiline_nodeLengthOutOfBounds
#[test]
fn test_multiline_node_length_out_of_bounds() {
    let mut ast = Ast::new();
    let foobar = ast.new_string("foobar");
    foobar.set_lineno_charno(&mut ast, 5, 2);
    foobar.set_length(&mut ast, 6);
    let baz = ast.new_string("baz");
    baz.set_lineno_charno(&mut ast, 6, 6);
    baz.set_length(&mut ast, 3);
    let n = IR::or(&mut ast, foobar, baz);
    n.set_lineno_charno(&mut ast, 5, 4);
    n.set_length(&mut ast, 1000);
    n.set_source_file_for_testing(&mut ast, "javascript/complex.js");
    let error = JSError::make(&ast, n, &FOO_TYPE, &[]);
    assert_eq!(
        formatter_full("if (foobar", 5).format_error(&ast, &error),
        "javascript/complex.js:5:4: ERROR - [TEST_FOO] error description here\n  5| if (foobar\n         ^^^^^^\n"
    );
}
// port: LightweightMessageFormatterTest#testMultiline_errorWithoutAssociatedNode
#[test]
fn test_multiline_error_without_associated_node() {
    let error = JSError::make_with_source_location("javascript/complex.js", 5, 4, &FOO_TYPE, &[]);
    assert_eq!(
        formatter_full("if (foobar", 5).format_error(&Ast::new(), &error),
        "javascript/complex.js:5:4: ERROR - [TEST_FOO] error description here\n  5| if (foobar\n         ^\n"
    );
}
// port: LightweightMessageFormatterTest#testMultiline_errorWithoutAssociatedNodeAndNegativeChar
#[test]
fn test_multiline_error_without_associated_node_and_negative_char() {
    let error = JSError::make_with_source_location("javascript/complex.js", 5, -1, &FOO_TYPE, &[]);
    assert_eq!(
        formatter_full("if (foobar", 5).format_error(&Ast::new(), &error),
        "javascript/complex.js:5: ERROR - [TEST_FOO] error description here\n  5| if (foobar\n"
    );
}

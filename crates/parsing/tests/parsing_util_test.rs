/*
 * Copyright 2019 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/parsing/ParsingUtilTest.java.

use closure_parsing::{
    config::{JsDocParsing, LanguageMode, RunMode, StrictMode},
    parser_runner::ParserRunner,
    parsing_util::ParsingUtil,
};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
    simple_source_file::SimpleSourceFile,
    static_source_file::SourceKind,
    testing::test_error_reporter::TestErrorReporter,
};
use std::sync::Arc;
// port: ParsingUtilTest#assertPatternDeclaresNames
fn assert_pattern_declares_names(pattern: &str, expected: &[&str]) {
    let mut ast = Ast::new();
    let node = parse_pattern(&mut ast, pattern);
    let mut seen = IndexSet::<_>::default();
    ParsingUtil::get_param_or_pattern_names(&ast, node, &mut |n| {
        seen.insert(n);
    });
    let mut actual = seen
        .into_iter()
        .map(|n| {
            assert!(n.is_name(&ast));
            n.get_string(&ast)
        })
        .collect::<Vec<_>>();
    let mut expected = expected
        .iter()
        .map(|s| JsString::from(*s))
        .collect::<Vec<_>>();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}
// port: ParsingUtilTest#parsePattern
fn parse_pattern(ast: &mut Ast, pattern: &str) -> NodeId {
    let script = parse(ast, &format!("function foo({pattern}){{}}"), &[]);
    assert!(script.is_script(ast));
    let function = script.get_first_child(ast).unwrap();
    assert!(function.is_function(ast));
    let params = function.get_second_child(ast).unwrap();
    assert!(params.is_param_list(ast));
    params
}
// port: ParsingUtilTest#parse
fn parse(ast: &mut Ast, source: &str, warnings: &[&str]) -> NodeId {
    let mut reporter = TestErrorReporter::new();
    reporter.expect_all_warnings(warnings);
    let config = ParserRunner::create_config_full(
        LanguageMode::ES_NEXT,
        JsDocParsing::INCLUDE_DESCRIPTIONS_NO_WHITESPACE,
        RunMode::KEEP_GOING,
        None,
        true,
        StrictMode::STRICT,
    );
    let script = ParserRunner::parse(
        ast,
        Arc::new(SimpleSourceFile::new("input", SourceKind::STRONG)),
        source.into(),
        &config,
        &mut reporter,
    )
    .ast
    .unwrap();
    reporter.verify_has_encountered_all_warnings_and_errors();
    script
}
// port: ParsingUtilTest#testNamePatternDeclaresName
#[test]
fn test_name_pattern_declares_name() {
    assert_pattern_declares_names("x", &["x"]);
    assert!(std::panic::catch_unwind(|| assert_pattern_declares_names("x", &["y"])).is_err());
}
// port: ParsingUtilTest#testEmptyPatternDeclaresNoNames
#[test]
fn test_empty_pattern_declares_no_names() {
    for s in ["[]", "{}", "[,]"] {
        assert_pattern_declares_names(s, &[]);
    }
}
// port: ParsingUtilTest#testNamesDeclaredInArrayPattern
#[test]
fn test_names_declared_in_array_pattern() {
    for (pattern, names) in [
        ("[x]", vec!["x"]),
        ("[x, y]", vec!["x", "y"]),
        ("[x, y, ...z]", vec!["x", "y", "z"]),
        ("[...z]", vec!["z"]),
        ("[[x, y]]", vec!["x", "y"]),
        ("[...[x, y]]", vec!["x", "y"]),
        ("[[x], [y]]", vec!["x", "y"]),
    ] {
        assert_pattern_declares_names(pattern, &names);
    }
}
// port: ParsingUtilTest#testNamesDeclaredInObjectPattern
#[test]
fn test_names_declared_in_object_pattern() {
    for (pattern, names) in [
        ("{x}", vec!["x"]),
        ("{x, y}", vec!["x", "y"]),
        ("{x, y, ...z}", vec!["x", "y", "z"]),
        ("{...z}", vec!["z"]),
        ("{x: y}", vec!["y"]),
        ("{[x]: y}", vec!["y"]),
        ("{[x()]: y}", vec!["y"]),
    ] {
        assert_pattern_declares_names(pattern, &names);
    }
}
// port: ParsingUtilTest#testNamesDeclaredInDefaultValue
#[test]
fn test_names_declared_in_default_value() {
    for (pattern, names) in [
        ("x = 1", vec!["x"]),
        ("[x, y] = [1, 2]", vec!["x", "y"]),
        ("{x, y} = {x: 1, y: 2}", vec!["x", "y"]),
    ] {
        assert_pattern_declares_names(pattern, &names);
    }
}
// port: ParsingUtilTest#testNamesDeclaredInRest
#[test]
fn test_names_declared_in_rest() {
    assert_pattern_declares_names("...x", &["x"]);
    assert_pattern_declares_names("...[x, y, z]", &["x", "y", "z"]);
}
// port: ParsingUtilTest#testNamesDeclaredInParamList
#[test]
fn test_names_declared_in_param_list() {
    assert_pattern_declares_names("x, y, z", &["x", "y", "z"]);
    assert_pattern_declares_names("[x1, x2], {y1, y2}, z = 0", &["x1", "x2", "y1", "y2", "z"]);
}

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
//   test/com/google/javascript/jscomp/CompilerOptionsTest.java.

use closure_jscomp::compiler_options::{BrowserFeaturesetYear, CompilerOptions, LanguageMode};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{node::Ast, token::Token};
// port: CompilerOptionsTest#testBrowserFeaturesetYearOptionSetsLanguageOut
#[test]
fn test_browser_featureset_year_option_sets_language_out() {
    let mut options = CompilerOptions::new();
    for (year, expected) in [
        (2012, LanguageMode::ECMASCRIPT5_STRICT.to_feature_set()),
        (2019, LanguageMode::ECMASCRIPT_2017.to_feature_set()),
        (2020, FeatureSet::BROWSER_2020),
        (2021, FeatureSet::BROWSER_2021),
        (2022, FeatureSet::BROWSER_2022),
        (2023, FeatureSet::BROWSER_2023),
        (2024, FeatureSet::BROWSER_2024),
        (2025, FeatureSet::BROWSER_2025),
        (2026, FeatureSet::BROWSER_2026),
    ] {
        options.set_browser_featureset_year(year);
        assert_eq!(options.get_output_feature_set(), expected);
    }
}
// port: CompilerOptionsTest#testBrowserFeaturesetYearOptionSetsAssumeES5
#[test]
fn test_browser_featureset_year_option_sets_assume_es5() {
    let mut options = CompilerOptions::new();
    let mut ast = Ast::new();
    options.set_browser_featureset_year(2012);
    assert_eq!(
        options.get_define_replacements(&mut ast)["$jscomp.ASSUME_ES5"].get_token(&ast),
        Token::FALSE
    );
    options.set_browser_featureset_year(2019);
    assert_eq!(
        options.get_define_replacements(&mut ast)["$jscomp.ASSUME_ES5"].get_token(&ast),
        Token::TRUE
    );
}
// port: CompilerOptionsTest#testBrowserFeaturesetYearOptionSetsAssumeES6
#[test]
fn test_browser_featureset_year_option_sets_assume_es6() {
    let mut options = CompilerOptions::new();
    let mut ast = Ast::new();
    for (year, name, token) in [
        (2012, "$jscomp.ASSUME_ES6", Token::FALSE),
        (2018, "$jscomp.ASSUME_ES6", Token::TRUE),
        (2020, "$jscomp.ASSUME_ES2020", Token::FALSE),
        (2021, "$jscomp.ASSUME_ES2020", Token::TRUE),
    ] {
        options.set_browser_featureset_year(year);
        assert_eq!(
            options.get_define_replacements(&mut ast)[name].get_token(&ast),
            token
        );
    }
}
// port: CompilerOptionsTest#testMinimumBrowserFeatureSetYearRequiredFor
#[test]
fn test_minimum_browser_feature_set_year_required_for() {
    for (feature, year) in [
        (Feature::GETTER, BrowserFeaturesetYear::YEAR_2012),
        (Feature::CLASSES, BrowserFeaturesetYear::YEAR_2018),
        (
            Feature::REGEXP_UNICODE_PROPERTY_ESCAPE,
            BrowserFeaturesetYear::YEAR_2021,
        ),
    ] {
        assert_eq!(
            BrowserFeaturesetYear::minimum_required_for(feature),
            Some(year)
        );
    }
}
// port: CompilerOptionsTest#testMinimumBrowserFeatureSetYearRequiredFor_returnsUnspecifiedIfUnsupported
#[test]
fn test_minimum_browser_feature_set_year_required_for_returns_unspecified_if_unsupported() {
    assert_eq!(
        BrowserFeaturesetYear::minimum_required_for(Feature::ES_NEXT_RUNTIME),
        None
    );
}
// port: CompilerOptionsTest#testDefines
#[test]
fn test_defines() {
    let mut options = CompilerOptions::new();
    options.set_define_to_boolean_literal("trueVar", true);
    options.set_define_to_boolean_literal("falseVar", false);
    options.set_define_to_number_literal("threeVar", 3);
    options.set_define_to_string_literal("strVar", "str");
    let mut ast = Ast::new();
    let actual = options.get_define_replacements(&mut ast);
    let true_node = ast.new_node(Token::TRUE);
    let false_node = ast.new_node(Token::FALSE);
    let number_node = ast.new_number(3.0);
    let string_node = ast.new_string("str");
    for (expected, name) in [
        (true_node, "trueVar"),
        (false_node, "falseVar"),
        (number_node, "threeVar"),
        (string_node, "strVar"),
    ] {
        assert!(expected.is_equivalent_to(&ast, actual[name]));
    }
}
// port: CompilerOptionsTest#testLanguageModeFromString
#[test]
fn test_language_mode_from_string() {
    for (input, expected) in [
        ("ECMASCRIPT3", Some(LanguageMode::ECMASCRIPT3)),
        ("  es3  ", Some(LanguageMode::ECMASCRIPT3)),
        ("junk", None),
        ("ECMASCRIPT_2020", Some(LanguageMode::ECMASCRIPT_2020)),
        ("  es_2020  ", Some(LanguageMode::ECMASCRIPT_2020)),
        ("  es2020  ", None),
        ("junk", None),
    ] {
        assert_eq!(LanguageMode::from_string(Some(input)), expected);
    }
}
// port: CompilerOptionsTest#testEmitUseStrictWorksInEs3
#[test]
fn test_emit_use_strict_works_in_es3() {
    let mut options = CompilerOptions::new();
    options.set_emit_use_strict(true);
    assert!(options.should_emit_use_strict());
}
// port: CompilerOptionsTest#testRemoveRegexFromPath
#[test]
fn test_remove_regex_from_path() {
    let options = CompilerOptions::new();
    let pattern = options
        .get_conformance_remove_regex_from_path()
        .as_ref()
        .unwrap();
    assert_eq!(
        pattern
            .matcher("/blaze-out/k8-fastbin/bin/some/path")
            .replace_all(""),
        "some/path"
    );
    assert_eq!(
        pattern
            .matcher("blaze-out/k8-fastbin/bin/some/path")
            .replace_all(""),
        "some/path"
    );
    assert_eq!(
        pattern
            .matcher("google3/blaze-out/k8-fastbin/bin/some/path")
            .replace_all(""),
        "some/path"
    );
    assert_eq!(
        pattern
            .matcher("google3/blaze-out/k8-fastbin/genfiles/some/path")
            .replace_all(""),
        "some/path"
    );
    assert_eq!(
        pattern
            .matcher("something/google3/blaze-out/k8-fastbin/some/path")
            .replace_all(""),
        "blaze-out/k8-fastbin/some/path"
    );
    assert_eq!(
        pattern
            .matcher("/something/google3/blaze-out/k8-fastbin/bin/some/path")
            .replace_all(""),
        "some/path"
    );
    assert_eq!(
        pattern.matcher("google3/some/path").replace_all(""),
        "some/path"
    );
    assert_eq!(
        pattern
            .matcher("google3/foo/blaze-out/some/path")
            .replace_all(""),
        "foo/blaze-out/some/path"
    );
    assert_eq!(
        pattern
            .matcher("google3/blaze-out/foo/blaze-out/some/path")
            .replace_all(""),
        "blaze-out/foo/blaze-out/some/path"
    );
    assert_eq!(
        pattern
            .matcher("bazel-out/k8-fastbin/bin/some/path")
            .replace_all(""),
        "some/path"
    );
    assert_eq!(
        pattern
            .matcher("bazel-out/k8-fastbin/genfiles/some/path")
            .replace_all(""),
        "some/path"
    );
}
// port: CompilerOptionsTest#testAllInstanceFieldsArePrivate
#[test]
fn test_all_instance_fields_are_private() {
    let source = include_str!("../src/compiler_options.rs");
    let fields = source
        .split("pub struct CompilerOptions {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    for field in fields.lines().filter(|line| line.contains(':')) {
        assert!(
            !field.trim_start().starts_with("pub"),
            "All instance fields in CompilerOptions must be private: {field}"
        );
    }
}
// port: CompilerOptionsTest#testTranspilePublicClassFields
#[test]
fn test_transpile_public_class_fields() {
    let mut options = CompilerOptions::new();
    options.set_browser_featureset_year(2023);
    assert!(!options.get_transpile_public_class_fields());
    assert!(
        options
            .get_output_feature_set()
            .has(Feature::PUBLIC_CLASS_FIELDS)
    );
    options.set_transpile_public_class_fields(true);
    assert!(options.get_transpile_public_class_fields());
    assert!(
        !options
            .get_output_feature_set()
            .has(Feature::PUBLIC_CLASS_FIELDS)
    );
    options.set_transpile_public_class_fields(false);
    assert!(!options.get_transpile_public_class_fields());
    assert!(
        options
            .get_output_feature_set()
            .has(Feature::PUBLIC_CLASS_FIELDS)
    );
}

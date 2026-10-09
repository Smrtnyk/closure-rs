/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/TypeMatchingStrategyTest.java.

//! Port of `TypeMatchingStrategyTest.java`.

use closure_jscomp::{
    Compiler,
    compiler_options::CompilerOptions,
    source_file::SourceFile,
    type_matching_strategy::TypeMatchingStrategy::{
        self, EXACT, LOOSE, STRICT_NULLABILITY, SUBTYPES,
    },
};
use closure_jstype::js_type::JSType;
use std::sync::Arc;

// port: TypeMatchingStrategyTest#EXTERNS
const EXTERNS: &str = "/** @constructor */
var OtherType = function() {};
/** @constructor */
var SuperType = function() {};
/** @constructor @extends {SuperType} */
var SubType = function() {};
";

// port: TypeMatchingStrategyTest#testMatch_default
#[test]
fn test_match_default() {
    assert_match(LOOSE, "!SuperType", "!SuperType", true, false);
    assert_match(LOOSE, "!SuperType", "?SuperType", true, false);
    assert_match(LOOSE, "!SuperType", "SuperType|undefined", true, false);
    assert_match(LOOSE, "!SuperType", "SuperType|void", true, false);
    assert_match(LOOSE, "!SuperType", "!SubType", true, false);
    assert_match(LOOSE, "!SuperType", "?SubType", true, false);
    assert_match(LOOSE, "!SuperType", "SubType|undefined", true, false);
    assert_match(LOOSE, "!SuperType", "SubType|void", true, false);
    assert_match(LOOSE, "!SuperType", "number", false, false);
    assert_match(LOOSE, "!SuperType", "*", true, true);
    assert_match(LOOSE, "!SuperType", "?", true, true);
    assert_match(LOOSE, "?", "string", true, false);
}

// port: TypeMatchingStrategyTest#testMatch_respectNullability
#[test]
fn test_match_respect_nullability() {
    assert_match(STRICT_NULLABILITY, "!SuperType", "!SuperType", true, false);
    assert_match(STRICT_NULLABILITY, "!SuperType", "?SuperType", false, false);
    assert_match(
        STRICT_NULLABILITY,
        "!SuperType",
        "SuperType|undefined",
        false,
        false,
    );
    assert_match(
        STRICT_NULLABILITY,
        "!SuperType",
        "SuperType|void",
        false,
        false,
    );
    assert_match(STRICT_NULLABILITY, "!SuperType", "!SubType", true, false);
    assert_match(STRICT_NULLABILITY, "!SuperType", "?SubType", false, false);
    assert_match(
        STRICT_NULLABILITY,
        "!SuperType",
        "SubType|undefined",
        false,
        false,
    );
    assert_match(
        STRICT_NULLABILITY,
        "!SuperType",
        "SubType|void",
        false,
        false,
    );
    assert_match(STRICT_NULLABILITY, "!SuperType", "number", false, false);
    assert_match(STRICT_NULLABILITY, "!SuperType", "*", true, true);
    assert_match(STRICT_NULLABILITY, "!SuperType", "?", true, true);
    assert_match(STRICT_NULLABILITY, "?", "string", true, false);
}

// port: TypeMatchingStrategyTest#testMatch_subtypes
#[test]
fn test_match_subtypes() {
    assert_match(SUBTYPES, "!SuperType", "!SuperType", true, false);
    assert_match(SUBTYPES, "!SuperType", "?SuperType", false, false);
    assert_match(SUBTYPES, "!SuperType", "!OtherType", false, false);
    assert_match(SUBTYPES, "!SuperType", "SuperType|undefined", false, false);
    assert_match(SUBTYPES, "!SuperType", "SuperType|void", false, false);
    assert_match(SUBTYPES, "!SuperType", "!SubType", true, false);
    assert_match(SUBTYPES, "!SuperType", "?SubType", false, false);
    assert_match(SUBTYPES, "!SuperType", "SubType|undefined", false, false);
    assert_match(SUBTYPES, "!SuperType", "SubType|void", false, false);
    assert_match(SUBTYPES, "!SuperType", "number", false, false);
    assert_match(SUBTYPES, "!SuperType", "*", false, false);
    assert_match(SUBTYPES, "!SuperType", "?", false, false);
    assert_match(SUBTYPES, "?SuperType", "!SuperType", true, false);
    assert_match(SUBTYPES, "?SuperType", "?SuperType", true, false);
    assert_match(SUBTYPES, "?SuperType", "?OtherType", false, false);
    assert_match(SUBTYPES, "?SuperType", "SuperType|undefined", false, false);
    assert_match(SUBTYPES, "?SuperType", "SuperType|void", false, false);
    assert_match(SUBTYPES, "?SuperType", "SuperType|OtherType", false, false);
    assert_match(SUBTYPES, "?SuperType", "!SubType", true, false);
    assert_match(SUBTYPES, "?SuperType", "?SubType", true, false);
    assert_match(SUBTYPES, "?SuperType", "SubType|undefined", false, false);
    assert_match(SUBTYPES, "?SuperType", "SubType|void", false, false);
    assert_match(SUBTYPES, "?SuperType", "number", false, false);
    assert_match(SUBTYPES, "?SuperType", "*", false, false);
    assert_match(SUBTYPES, "?SuperType", "?", false, false);
    assert_match(SUBTYPES, "?", "string", true, false);
    assert_match(SUBTYPES, "?", "?", true, false);
    assert_match(SUBTYPES, "?", "*", true, false);
    assert_match(SUBTYPES, "*", "?", false, false);
}

// port: TypeMatchingStrategyTest#testMatch_exact
#[test]
fn test_match_exact() {
    assert_match(EXACT, "!SuperType", "!SuperType", true, false);
    assert_match(EXACT, "!SuperType", "?SuperType", false, false);
    assert_match(EXACT, "!SuperType", "SuperType|undefined", false, false);
    assert_match(EXACT, "!SuperType", "SuperType|void", false, false);
    assert_match(EXACT, "!SuperType", "!SubType", false, false);
    assert_match(EXACT, "!SuperType", "?SubType", false, false);
    assert_match(EXACT, "!SuperType", "SubType|undefined", false, false);
    assert_match(EXACT, "!SuperType", "SubType|void", false, false);
    assert_match(EXACT, "!SuperType", "number", false, false);
    assert_match(EXACT, "!SuperType", "*", false, false);
    assert_match(EXACT, "!SuperType", "?", false, false);
    assert_match(EXACT, "?", "string", true, false);
}

// port: TypeMatchingStrategyTest#assertMatch
fn assert_match(
    type_matching_strategy: TypeMatchingStrategy,
    template_type: &str,
    type_: &str,
    is_match: bool,
    is_loose_match: bool,
) {
    // It's important that the test uses the same compiler to compile the template type and the
    // type to be matched. Otherwise, equal types won't be considered equal.
    let mut compiler = Compiler::new();
    compiler.disable_threads();
    let mut options = CompilerOptions::new();
    options.set_check_types(true);
    options.set_checks_only(true);

    compiler.compile(
        &[Arc::new(SourceFile::from_code("externs", EXTERNS))],
        &[Arc::new(SourceFile::from_code(
            "test",
            format!("/** @type {{{template_type}}} */ var x; /** @type {{{type_}}} */ var y;"),
        ))],
        options,
    );
    let root = compiler.get_root().unwrap();
    let script = root
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let x_node = script.get_first_child(&compiler).unwrap();
    let y_node = script.get_last_child(&compiler).unwrap();
    let template_js_type = x_node
        .get_first_child(&compiler)
        .unwrap()
        .get_jstype(&compiler)
        .unwrap();
    let js_type = y_node
        .get_first_child(&compiler)
        .unwrap()
        .get_jstype(&compiler);

    let (registry, ast) = compiler.get_type_registry_and_ast();
    let match_result = type_matching_strategy.r#match(registry, ast, template_js_type, js_type);
    let message = if is_match {
        format!(
            "'{}' should match '{}'",
            template_js_type.to_string(registry, ast),
            js_type.map_or_else(|| "null".to_string(), |t| t.to_string(registry, ast))
        )
    } else {
        format!("'{template_type}' should not match '{type_}'")
    };
    assert_eq!(match_result.is_match(), is_match, "{message}");
    let message = if is_loose_match {
        format!("'{template_type}' should loosely match '{type_}'")
    } else {
        format!("'{template_type}' should not loosely match '{type_}'")
    };
    assert_eq!(match_result.is_loose_match(), is_loose_match, "{message}");
}

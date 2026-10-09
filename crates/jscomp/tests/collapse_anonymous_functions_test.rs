/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CollapseAnonymousFunctionsTest.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java.

//! Post-call assertions of CollapseAnonymousFunctionsTest#testLet and #testConst
//! (corpus/unit/rust_unit_tests/CollapseAnonymousFunctionsTest.md): they check `IS_CONSTANT_NAME`
//! on output NAME nodes, which neither the tree comparison nor the post-call snapshot of the
//! replayed records covers. Every test of the class also replays as a corpus record
//! (crates/testing). A local fixture reproduces the part of CompilerTestCase#test these two tests
//! use: parse `testcode` with CompilerTestCase#getOptions, normalize the input (enableNormalize:
//! CompilerTestCase#normalizeActualCode), run the processor, then compare the result with the parse
//! of the expected code (enableNormalizeExpectedOutput is off in this class).
use closure_jscomp::{
    Compiler,
    check_level::CheckLevel,
    collapse_anonymous_functions::CollapseAnonymousFunctions,
    compiler_options::{CompilerOptions, LanguageMode},
    compiler_pass::CompilerPass,
    diagnostic_groups,
    google_coding_convention::GoogleCodingConvention,
    normalize::Normalize,
    source_file::SourceFile,
};
use closure_rhino::{
    node::{NodeId, Prop},
    testing::node_subject::assert_node,
};
use std::sync::Arc;

// port: CompilerTestCase#GENERATED_SRC_NAME
const GENERATED_SRC_NAME: &str = "testcode";

// port: CompilerTestCase#getOptions
fn get_options() -> CompilerOptions {
    let mut options = CompilerOptions::new();
    options.set_language_in(LanguageMode::UNSUPPORTED);
    options.set_emit_use_strict(false);
    options.set_language_out(LanguageMode::NO_TRANSPILE);
    options.set_preserve_type_annotations(true);
    options.set_assume_getters_are_pure(false);
    options.set_check_symbols(true);
    options.set_warning_level(
        diagnostic_groups::INVALID_CASTS.clone(),
        CheckLevel::WARNING,
    );
    options.set_warning_level(
        diagnostic_groups::MISPLACED_MSG_ANNOTATION.clone(),
        CheckLevel::WARNING,
    );
    options.set_warning_level(
        diagnostic_groups::MISSING_PROPERTIES.clone(),
        CheckLevel::WARNING,
    );
    options.set_coding_convention(Arc::new(GoogleCodingConvention::new()));
    options.set_polymer_version(Some(1));
    options
}

// port: CompilerTestCase#parse (one input named `testcode`, no externs)
fn parse(code: &str) -> Compiler {
    let mut compiler = Compiler::new();
    let input = Arc::new(SourceFile::from_code(GENERATED_SRC_NAME, code));
    compiler.init(&[], &[input], get_options());
    compiler.parse_inputs();
    assert!(
        compiler.get_errors().is_empty(),
        "Unexpected parse error(s)"
    );
    assert!(
        compiler.get_warnings().is_empty(),
        "Unexpected parse warning(s)"
    );
    compiler
}

// port: CompilerTestCase#test(String, String) (enableNormalize; getProcessor returns
// CollapseAnonymousFunctions)
fn test(js: &str, expected: &str) -> Compiler {
    let mut compiler = parse(js);
    let externs_root = compiler.get_externs_root().unwrap();
    let main_root = compiler.get_js_root().unwrap();
    // port: CompilerTestCase#normalizeActualCode
    Normalize::create_normalize_for_optimizations(&mut compiler).process(
        &mut compiler,
        externs_root,
        main_root,
    );
    CollapseAnonymousFunctions::new(&compiler).process(&mut compiler, externs_root, main_root);
    assert!(compiler.get_errors().is_empty(), "Unexpected error(s)");
    assert!(compiler.get_warnings().is_empty(), "Unexpected warning(s)");

    let expected_compiler = parse(expected);
    let expected_root = expected_compiler.get_js_root().unwrap();
    if let Err(message) = assert_node(main_root).check_equal_to_across(
        &compiler,
        &expected_compiler,
        expected_root,
        true,
    ) {
        panic!("{message}");
    }
    compiler
}

// The Node#getBooleanProp(Node.IS_CONSTANT_NAME) helper, which names Closure's Rhino-derived
// Node (MPL-1.1 / GPL-2.0-or-later), is in its own file.
#[path = "rhino/collapse_anonymous_functions_test.rs"]
mod rhino;
use rhino::is_constant_name;

// port: CollapseAnonymousFunctionsTest#testLet
#[test]
fn test_let() {
    let compiler = test("let f = function() {};", "function f() {}");
    let fn_name = compiler
        .get_js_root()
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    assert!(fn_name.is_name(&compiler));
    assert_eq!(fn_name.get_string(&compiler), "f");
    // The function name `f` is not marked as IS_CONSTANT_NAME property as LHS declaration is
    // `let`.
    assert!(!is_constant_name(&compiler, fn_name));

    // The function name `f` and call name `f` are not marked as IS_CONSTANT_NAME property as LHS
    // declaration is `let`
    let compiler = test("let f = function() {}; f();", "function f(){} f();");
    let fn_name = compiler
        .get_js_root()
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    assert!(fn_name.is_name(&compiler));
    assert_eq!(fn_name.get_string(&compiler), "f");
    assert!(!is_constant_name(&compiler, fn_name));
    let called_fn_name = fn_name
        .get_parent(&compiler)
        .unwrap()
        .get_next(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    assert!(called_fn_name.is_name(&compiler));
    assert_eq!(called_fn_name.get_string(&compiler), "f");
    assert!(!is_constant_name(&compiler, called_fn_name));
}

// port: CollapseAnonymousFunctionsTest#testConst
#[test]
fn test_const() {
    // The function name `f` gets marked as IS_CONSTANT_NAME property as LHS declaration is `const`.
    let compiler = test("const f = function() {};", "function f() {}");
    let fn_name = compiler
        .get_js_root()
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    assert!(fn_name.is_name(&compiler));
    assert_eq!(fn_name.get_string(&compiler), "f");
    assert!(is_constant_name(&compiler, fn_name));

    // The function name `f` and call name `f` stays marked as IS_CONSTANT_NAME property
    let compiler = test("const f = function() {}; f();", "function f(){} f();");
    let fn_name = compiler
        .get_js_root()
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    assert!(fn_name.is_name(&compiler));
    assert_eq!(fn_name.get_string(&compiler), "f");
    assert!(is_constant_name(&compiler, fn_name));
    let called_fn_name = fn_name
        .get_parent(&compiler)
        .unwrap()
        .get_next(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    assert!(called_fn_name.is_name(&compiler));
    assert_eq!(called_fn_name.get_string(&compiler), "f");
    assert!(is_constant_name(&compiler, called_fn_name));
}

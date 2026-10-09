/*
 * Copyright 2005 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/RenameVarsTest.java.

//! RenameVarsTest methods with checks the replayed corpus records cannot make: the post-call
//! source-position assertions of testRenameSimple (corpus/unit/rust_unit_tests/RenameVarsTest.md)
//! and testPrevUsedMapWithDuplicates, which throws before its CompilerTestCase call. Every other
//! RenameVarsTest method replays as a corpus record (crates/testing). A local fixture reproduces the
//! part of CompilerTestCase#test that testRenameSimple uses: parse `testcode` (externs: one empty
//! `externs` file) with CompilerTestCase#getOptions, run RenameVarsTest#getProcessor's RenameVars,
//! then compare the result with the parse of the expected code.
use closure_jscomp::{
    Compiler,
    check_level::CheckLevel,
    compiler_options::{CompilerOptions, LanguageMode},
    default_name_generator::DefaultNameGenerator,
    diagnostic_groups,
    google_coding_convention::GoogleCodingConvention,
    rename_vars::RenameVars,
    source_file::SourceFile,
    variable_map::VariableMap,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{js_string::JsString, node::NodeId, testing::node_subject::assert_node};
use std::sync::Arc;

// port: CompilerTestCase#GENERATED_SRC_NAME
const GENERATED_SRC_NAME: &str = "testcode";
// port: RenameVarsTest#DEFAULT_PREFIX
const DEFAULT_PREFIX: &str = "";

// port: CompilerTestCase#getOptions (RenameVarsTest#getCodingConvention: GoogleCodingConvention)
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

// port: CompilerTestCase#parse (externs: CompilerTestCase() default, one empty `externs` file)
fn parse(code: &str) -> Compiler {
    let mut compiler = Compiler::new();
    let externs = Arc::new(SourceFile::from_code("externs", ""));
    let input = Arc::new(SourceFile::from_code(GENERATED_SRC_NAME, code));
    compiler.init(&[externs], &[input], get_options());
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

// port: RenameVarsTest#getProcessor (the RenameVarsTest#setUp defaults)
fn get_processor() -> RenameVars {
    RenameVars::new(
        Some(JsString::from(DEFAULT_PREFIX)),
        /* localRenamingOnly= */ false,
        /* generatePseudoNames= */ false,
        /* preferStableNames= */ false,
        Some(Arc::new(VariableMap::new(&IndexMap::<_, _>::default()))),
        IndexSet::<_>::default(),
        None,
        Box::new(DefaultNameGenerator::new()),
    )
}

// port: CompilerTestCase#test(String, String)
fn test(js: &str, expected: &str) -> Compiler {
    let mut compiler = parse(js);
    let externs_root = compiler.get_externs_root().unwrap();
    let main_root = compiler.get_js_root().unwrap();
    get_processor().process(&mut compiler, externs_root, main_root);
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

// The NodeSubject assertions of the source-info checks, which name Closure's Rhino-derived
// NodeSubject (MPL-1.1 / GPL-2.0-or-later), are in their own file.
#[path = "rhino/rename_vars_test.rs"]
mod rhino;
use rhino::{assert_name, assert_position, one_child};

// port: RenameVarsTest#testRenameSimple
#[test]
fn test_rename_simple() {
    let compiler = test(
        "function foo(v1, v2) {\n  return v1;\n}\nfoo();\n",
        "function a(b, c) {\n  return b;\n}\na();\n",
    );

    // Do a sanity check on the source info
    let last_script = compiler
        .get_root()
        .unwrap()
        .get_last_child(&compiler) // first child is externs tree, last / second is sources
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let last_script_name = last_script.get_source_file_name(&compiler);
    let name = Some(&last_script_name);

    // function foo(v1, v2) { ... }
    let fn_node = last_script.get_first_child(&compiler).unwrap();
    assert!(fn_node.is_function(&compiler));
    assert_position(&compiler, fn_node, name, 1, 0, 37);

    // function foo(v1, v2) {
    //          ^^^
    let fn_name = fn_node.get_first_child(&compiler).unwrap();
    assert_name(&compiler, fn_name, "a");
    assert_position(&compiler, fn_name, name, 1, 9, 3);

    // function foo(v1, v2) {
    //             ^^^^^^^^
    let param_list = fn_node.get_second_child(&compiler).unwrap();
    assert!(param_list.is_param_list(&compiler));
    assert_position(&compiler, param_list, name, 1, 12, 8);
    assert_eq!(param_list.get_child_count(&compiler), 2);

    // function foo(v1, v2) {
    //              ^^
    let first_param = param_list.get_first_child(&compiler).unwrap();
    assert_name(&compiler, first_param, "b");
    assert_position(&compiler, first_param, name, 1, 13, 2);

    // function foo(v1, v2) {
    //                  ^^
    let second_param = param_list.get_second_child(&compiler).unwrap();
    assert_name(&compiler, second_param, "c");
    assert_position(&compiler, second_param, name, 1, 17, 2);

    //  { ... }
    let fn_body = fn_node.get_last_child(&compiler).unwrap();
    assert!(fn_body.is_block(&compiler));
    assert_position(&compiler, fn_body, name, 1, 21, 16);

    // return v1;
    let return_node = one_child(&compiler, fn_body);
    assert!(return_node.is_return(&compiler));
    assert_position(&compiler, return_node, name, 2, 2, 10);

    // return v1;
    //        ^^
    let returned = one_child(&compiler, return_node);
    assert_name(&compiler, returned, "b");
    assert_position(&compiler, returned, None, 2, 9, 2);

    // foo();
    let expr_result = last_script.get_last_child(&compiler).unwrap();
    assert!(expr_result.is_expr_result(&compiler)); // foo(); (statement)
    assert_position(&compiler, expr_result, name, 4, 0, 6);
    let call = one_child(&compiler, expr_result); // foo() (expression)
    assert!(call.is_call(&compiler));
    assert_position(&compiler, call, name, 4, 0, 5);
    let callee = one_child(&compiler, call); // foo (function name)
    assert_name(&compiler, callee, "a");
    assert_position(&compiler, callee, None, 4, 0, 3);
}

// port: RenameVarsTest#makeVariableMap
fn make_variable_map(key_val_pairs: &[&str]) -> VariableMap {
    assert!(key_val_pairs.len().is_multiple_of(2));

    // ImmutableMap.Builder#buildOrThrow: a duplicate key throws.
    let mut rename_map = IndexMap::<_, _>::default();
    for pair in key_val_pairs.chunks(2) {
        let previous = rename_map.insert(JsString::from(pair[0]), JsString::from(pair[1]));
        assert!(
            previous.is_none(),
            "Multiple entries with same key: {}",
            pair[0]
        );
    }

    VariableMap::new(&rename_map)
}

// port: RenameVarsTest#testPrevUsedMapWithDuplicates
#[test]
fn test_prev_used_map_with_duplicates() {
    // Java: makeVariableMap throws IllegalArgumentException (ImmutableBiMap rejects the duplicate
    // value) before testSame("") runs; reaching the end of the try block is an AssertionError.
    let thrown = std::panic::catch_unwind(|| make_variable_map(&["Foo", "z", "Bar", "z"]))
        .expect_err("expected IllegalArgumentException");
    let message = thrown
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| thrown.downcast_ref::<&str>().copied())
        .unwrap();
    assert_eq!(message, "Multiple entries with same value: Bar=z and Foo=z");
}

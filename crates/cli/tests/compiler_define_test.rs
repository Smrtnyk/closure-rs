/*
 * Copyright 2010 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CompilerTest.java.

//! The seven `CompilerTest` define-override cases (CompilerTest.java). They exercise
//! `AbstractCommandLineRunner.createDefineReplacements`, so they live with the cli crate
//! (closure-jscomp cannot depend on closure-cli).
use closure_cli::abstract_command_line_runner::AbstractCommandLineRunner;
use closure_jscomp::compiler_options::CompilerOptions;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    node::{Ast, NodeId},
    token::Token,
};

type Runner = AbstractCommandLineRunner<(), ()>;

fn strings(defines: &[&str]) -> Vec<String> {
    defines.iter().map(|d| (*d).to_string()).collect()
}

// port: CompilerTest#testDefineNoOverriding
#[test]
fn test_define_no_overriding() {
    let mut ast = Ast::new();
    let empty_map: IndexMap<String, NodeId> = IndexMap::<_, _>::default();
    let defines: Vec<String> = Vec::new();
    assert_define_overrides(&mut ast, &empty_map, &defines);
}

// port: CompilerTest#testDefineOverriding1
#[test]
fn test_define_overriding1() {
    let mut ast = Ast::new();
    let defines = strings(&[
        "COMPILED",
        "DEF_TRUE=true",
        "DEF_FALSE=false",
        "DEF_NUMBER=5.5",
        "DEF_STRING='bye'",
    ]);
    let mut expected = IndexMap::<_, _>::default();
    expected.insert("COMPILED".to_string(), ast.new_node(Token::TRUE));
    expected.insert("DEF_TRUE".to_string(), ast.new_node(Token::TRUE));
    expected.insert("DEF_FALSE".to_string(), ast.new_node(Token::FALSE));
    expected.insert("DEF_NUMBER".to_string(), ast.new_number(5.5));
    expected.insert("DEF_STRING".to_string(), ast.new_string("bye"));
    assert_define_overrides(&mut ast, &expected, &defines);
}

// port: CompilerTest#testDefineOverriding2
#[test]
fn test_define_overriding2() {
    let mut ast = Ast::new();
    let defines = strings(&["DEF_STRING='='"]);
    let mut expected = IndexMap::<_, _>::default();
    expected.insert("DEF_STRING".to_string(), ast.new_string("="));
    assert_define_overrides(&mut ast, &expected, &defines);
}

// port: CompilerTest#testDefineOverriding3
#[test]
fn test_define_overriding3() {
    let mut ast = Ast::new();
    let defines = strings(&["a.DEBUG"]);
    let mut expected = IndexMap::<_, _>::default();
    expected.insert("a.DEBUG".to_string(), ast.new_node(Token::TRUE));
    assert_define_overrides(&mut ast, &expected, &defines);
}

// port: CompilerTest#testBadDefineOverriding1
#[test]
fn test_bad_define_overriding1() {
    let defines = strings(&["DEF_STRING="]);
    let mut options = CompilerOptions::new();

    assert!(Runner::create_define_replacements(&defines, &mut options).is_err());
}

// port: CompilerTest#testBadDefineOverriding2
#[test]
fn test_bad_define_overriding2() {
    let defines = strings(&["=true"]);
    let mut options = CompilerOptions::new();

    assert!(Runner::create_define_replacements(&defines, &mut options).is_err());
}

// port: CompilerTest#testBadDefineOverriding3
#[test]
fn test_bad_define_overriding3() {
    let defines = strings(&["DEF_STRING='''"]);
    let mut options = CompilerOptions::new();

    assert!(Runner::create_define_replacements(&defines, &mut options).is_err());
}

// port: CompilerTest#assertDefineOverrides
fn assert_define_overrides(ast: &mut Ast, expected: &IndexMap<String, NodeId>, defines: &[String]) {
    let mut options = CompilerOptions::new();
    Runner::create_define_replacements(defines, &mut options).unwrap();
    let actual = options.get_define_replacements(ast);

    // equality of nodes compares by reference, so instead,
    // compare the maps manually using Node.checkTreeEqualsSilent
    assert_eq!(actual.len(), expected.len());
    for (key, value) in expected {
        assert!(actual.contains_key(key), "{key}");

        let actual_node = actual[key];
        assert!(value.is_equivalent_to(ast, actual_node), "{key}");
    }
}

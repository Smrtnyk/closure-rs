/*
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
//   test/com/google/javascript/jscomp/RewriteGlobalDeclarationsForTryCatchWrappingTest.java.

#[path = "support/pass_test_case.rs"]
mod pass_test_case;

use closure_jscomp::{
    Compiler,
    rewrite_global_declarations_for_try_catch_wrapping::RewriteGlobalDeclarationsForTryCatchWrapping,
};
use closure_testing::testing::js_chunk_graph_builder::JSChunkGraphBuilder;
use pass_test_case::{PassTestCase, expected, srcs, srcs_chunks};

// port: RewriteGlobalDeclarationsForTryCatchWrappingTest#getProcessor
fn get_processor(_compiler: &mut Compiler) -> RewriteGlobalDeclarationsForTryCatchWrapping {
    RewriteGlobalDeclarationsForTryCatchWrapping::new()
}

fn t() -> PassTestCase {
    PassTestCase::new("")
}

fn test(js: &str, exp: &str) {
    t().test(get_processor, srcs(js), expected(exp));
}

fn test_same(js: &str) {
    t().test_same(get_processor, srcs(js));
}

// port: RewriteGlobalDeclarationsForTryCatchWrappingTest#testFunctionDeclarations
#[test]
fn test_function_declarations() {
    test(
        "a; function f(){} function g(){}",
        "var f = function(){}; var g = function(){}; a;",
    );
}

// port: RewriteGlobalDeclarationsForTryCatchWrappingTest#testFunctionDeclarationsInBlock
#[test]
fn test_function_declarations_in_block() {
    test_same("a; if (1) { function f(){} function g(){} }");
}

// port: RewriteGlobalDeclarationsForTryCatchWrappingTest#testClassDeclarations
#[test]
fn test_class_declarations() {
    // classes don't hoist
    test(
        "a; class B{} class C{}",
        "a; var B = class{}; var C = class{};",
    );
}

// port: RewriteGlobalDeclarationsForTryCatchWrappingTest#testClassDeclarationsInBlock
#[test]
fn test_class_declarations_in_block() {
    test_same("a; if (1) { class B{} class C{} }");
}

// port: RewriteGlobalDeclarationsForTryCatchWrappingTest#testFunctionDeclarationsInModule
#[test]
fn test_function_declarations_in_module() {
    t().test(
        get_processor,
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("a; function f(){} function g(){}")
                .build(),
        ),
        expected("var f = function(){}; var g = function(){}; a"),
    );
}

// port: RewriteGlobalDeclarationsForTryCatchWrappingTest#testGeneratorDeclarations
#[test]
fn test_generator_declarations() {
    test(
        "a; function *f(){} function *g(){}",
        "var f = function* (){}; var g = function* (){}; a;",
    );
}

// port: RewriteGlobalDeclarationsForTryCatchWrappingTest#testFunctionDeclarationsInEs6Module
#[test]
fn test_function_declarations_in_es6_module() {
    // NOTE: Currently this pass always runs after module transpilation.
    // No current uses of this pass would benefit from ES6 module support:
    // a) RescopeGlobalSymbols, which relies on this pass, only cares about the global scope.
    // b) ES6 module output cannot be wrapped in a try/catch.
    test_same("a; function f(){} function g(){} export default 5;");
}

// port: RewriteGlobalDeclarationsForTryCatchWrappingTest#testFunctionsExpression
#[test]
fn test_functions_expression() {
    test_same("a; f = function(){}");
}

// port: RewriteGlobalDeclarationsForTryCatchWrappingTest#testNoMoveDeepFunctionDeclarations
#[test]
fn test_no_move_deep_function_declarations() {
    test_same("a; if (a) function f(){};");
    test_same("a; if (a) { function f(){} }");
}

// port: RewriteGlobalDeclarationsForTryCatchWrappingTest#testMigrateLetConstToVar1
#[test]
fn test_migrate_let_const_to_var1() {
    test("let a = 1;", "var a = 1");
    test("const a = 2;", "var a = 2");
}

// port: RewriteGlobalDeclarationsForTryCatchWrappingTest#testMigrateLetConstToVar2
#[test]
fn test_migrate_let_const_to_var2() {
    test_same("{let a = 1;}");
    test_same("{const a = 2;}");
}

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
//   test/com/google/javascript/jscomp/FunctionToBlockMutatorTest.java.

//! Port of `FunctionToBlockMutatorTest.java`.

use closure_jscomp::{
    Compiler,
    compiler_pass::CompilerPass,
    function_to_block_mutator::FunctionToBlockMutator,
    gather_getter_and_setter_properties::GatherGetterAndSetterProperties,
    node_traversal::{Callback, NodeTraversal},
    normalize::Normalize,
    pure_function_identifier::Driver,
    source_file::SourceFile,
    source_info_check::SourceInfoCheck,
    validity_check::VerifyConstants,
};
use closure_rhino::{
    check_state, js_string::JsString, node::NodeId, testing::node_subject::assert_node,
};
use std::sync::Arc;

struct FunctionToBlockMutatorTest {
    needs_default_result: bool,
    is_call_in_loop: bool,
}

impl FunctionToBlockMutatorTest {
    // port: FunctionToBlockMutatorTest#setUp
    fn set_up() -> Self {
        Self {
            needs_default_result: false,
            is_call_in_loop: false,
        }
    }

    // port: FunctionToBlockMutatorTest#helperMutate(String,String,String)
    fn helper_mutate3(&self, code: &str, expected_result: &str, fn_name: &str) {
        self.helper_mutate(code, expected_result, fn_name, Some("result"));
    }

    // port: FunctionToBlockMutatorTest#helperMutate(String,String,String,String)
    fn helper_mutate(
        &self,
        code: &str,
        expected_result: &str,
        fn_name: &str,
        result_name: Option<&str>,
    ) {
        let mut compiler = Compiler::new();
        compiler.init_compiler_options_if_testing();
        let mutator =
            FunctionToBlockMutator::new(&compiler, compiler.get_unique_name_id_supplier());

        let options = compiler.get_options().clone();
        compiler.init(
            &[],
            &[Arc::new(SourceFile::from_code("[testcode]", code))],
            options,
        );
        compiler.parse();
        let script = compiler
            .get_root()
            .unwrap()
            .get_second_child(&compiler)
            .unwrap()
            .get_first_child(&compiler)
            .unwrap();

        let externs = compiler.get_externs_root().unwrap();
        let js = compiler.get_js_root().unwrap();

        Normalize::create_normalize_for_optimizations(&mut compiler).process(
            &mut compiler,
            externs,
            js,
        );
        GatherGetterAndSetterProperties::update(&mut compiler, externs, js);
        Driver::new().process(&mut compiler, externs, js);

        let fn_node = find_function(&compiler, script, fn_name).unwrap();

        let expected_root = compiler.parse_test_code(expected_result);
        check_state!(compiler.get_error_count() == 0);
        let expected = expected_root.get_first_child(&compiler).unwrap();

        let fn_name = JsString::from(fn_name);
        let callname = fn_name.clone();
        let result_name = result_name.map(JsString::from);
        let needs_default_result = self.needs_default_result;
        let is_call_in_loop = self.is_call_in_loop;
        // inline tester
        let tester = move |t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>| -> bool {
            let result = mutator.mutate(
                t.get_compiler(),
                &fn_name,
                fn_node,
                n,
                result_name.as_ref(),
                needs_default_result,
                is_call_in_loop,
            );
            validate_source_info(t.get_compiler(), result);
            // Rust-only: no usingSerializer(compiler::toSource) (it only formats a failure).
            assert_node(result).is_equal_to(t, expected);

            n.replace_with(t, result);
            VerifyConstants::new(t.get_compiler(), false).process(t.get_compiler(), externs, js);
            Normalize::builder(t.get_compiler())
                .assert_on_change(true)
                .build()
                .process(t.get_compiler(), externs, js);
            result.replace_with(t, n);

            true
        };

        let mut test = TestCallback::new(callname, Box::new(tester));
        NodeTraversal::traverse(&mut compiler, script, &mut test);
    }
}

// port: FunctionToBlockMutatorTest#validateSourceInfo
fn validate_source_info(compiler: &mut Compiler, subtree: NodeId) {
    SourceInfoCheck::new(compiler).set_check_sub_tree(compiler, subtree);
    assert!(
        compiler.get_errors().is_empty(),
        "{:?}",
        compiler.get_errors()
    );
}

type Method = Box<dyn FnMut(&mut NodeTraversal<'_>, NodeId, Option<NodeId>) -> bool>;

struct TestCallback {
    callname: JsString,
    method: Method,
    complete: bool,
}

impl TestCallback {
    // port: FunctionToBlockMutatorTest.TestCallback#TestCallback
    fn new(callname: JsString, method: Method) -> Self {
        Self {
            callname,
            method,
            complete: false,
        }
    }
}

impl Callback for TestCallback {
    // port: FunctionToBlockMutatorTest.TestCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _node_traversal: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        !self.complete
    }

    // port: FunctionToBlockMutatorTest.TestCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_call(t) {
            let first = n.get_first_child(t).unwrap();
            if first.is_name(t) && first.get_string(t) == self.callname {
                self.complete = (self.method)(t, n, parent);
            }
        }

        if parent.is_none() {
            assert!(self.complete);
        }
    }
}

// port: FunctionToBlockMutatorTest#findFunction
fn find_function(compiler: &Compiler, n: NodeId, name: &str) -> Option<NodeId> {
    if n.is_function(compiler) && n.get_first_child(compiler).unwrap().get_string(compiler) == name
    {
        return Some(n);
    }

    let mut c = n.get_first_child(compiler);
    while let Some(child) = c {
        let result = find_function(compiler, child, name);
        if result.is_some() {
            return result;
        }
        c = child.get_next(compiler);
    }

    None
}

// port: FunctionToBlockMutatorTest#testMutateNoReturnWithoutResultAssignment
#[test]
fn test_mutate_no_return_without_result_assignment() {
    let t = FunctionToBlockMutatorTest::set_up();
    t.helper_mutate3("function foo(){}; foo();", "{}", "foo");
}

// port: FunctionToBlockMutatorTest#testMutateNoReturnWithResultAssignment
#[test]
fn test_mutate_no_return_with_result_assignment() {
    let mut t = FunctionToBlockMutatorTest::set_up();
    t.needs_default_result = true;
    t.helper_mutate3(
        "function foo(){}; var result = foo();",
        "{result = void 0}",
        "foo",
    );
}

// port: FunctionToBlockMutatorTest#testMutateNoValueReturnWithoutResultAssignment
#[test]
fn test_mutate_no_value_return_without_result_assignment() {
    let t = FunctionToBlockMutatorTest::set_up();
    t.helper_mutate("function foo(){return;}; foo();", "{}", "foo", None);
}

// port: FunctionToBlockMutatorTest#testMutateNoValueReturnWithResultAssignment
#[test]
fn test_mutate_no_value_return_with_result_assignment() {
    let t = FunctionToBlockMutatorTest::set_up();
    t.helper_mutate3(
        "function foo(){return;}; var result = foo();",
        "{result = void 0}",
        "foo",
    );
}

// port: FunctionToBlockMutatorTest#testMutateValueReturnWithoutResultAssignment
#[test]
fn test_mutate_value_return_without_result_assignment() {
    let t = FunctionToBlockMutatorTest::set_up();
    t.helper_mutate(
        "function foo(){return true;}; foo();",
        "{true;}",
        "foo",
        None,
    );
}

// port: FunctionToBlockMutatorTest#testMutateValueReturnWithResultAssignment
#[test]
fn test_mutate_value_return_with_result_assignment() {
    let mut t = FunctionToBlockMutatorTest::set_up();
    t.needs_default_result = true;
    t.helper_mutate(
        "function foo(){return true;}; var x=foo();",
        "{x=true}",
        "foo",
        Some("x"),
    );
}

// port: FunctionToBlockMutatorTest#testMutateWithMultipleReturns
#[test]
fn test_mutate_with_multiple_returns() {
    let mut t = FunctionToBlockMutatorTest::set_up();
    t.needs_default_result = true;
    t.helper_mutate3("function foo(){ if (0) {return 0} else {return 1} }; var result=foo();", "{\n  JSCompiler_inline_label_foo_0: {\n    if (0) {\n      result = 0;\n      break JSCompiler_inline_label_foo_0\n    } else {\n      result = 1;\n      break JSCompiler_inline_label_foo_0\n    }\n    result=void 0\n  }\n}\n", "foo");
}

// port: FunctionToBlockMutatorTest#testMutateWithParameters1
#[test]
fn test_mutate_with_parameters1() {
    let t = FunctionToBlockMutatorTest::set_up();
    // Simple call with useless parameter
    t.helper_mutate(
        "function foo(a){return true;}; foo(x);",
        "{true}",
        "foo",
        None,
    );
}

// port: FunctionToBlockMutatorTest#testMutateWithParameters2
#[test]
fn test_mutate_with_parameters2() {
    let t = FunctionToBlockMutatorTest::set_up();
    // Simple call with parameter
    t.helper_mutate("function foo(a){return x;}; foo(x);", "{x}", "foo", None);
}

// port: FunctionToBlockMutatorTest#testMutateWithParameters3
#[test]
fn test_mutate_with_parameters3() {
    let t = FunctionToBlockMutatorTest::set_up();
    // Parameter has side-effects.
    t.helper_mutate(
        "function foo(a){return a;}; function x() { foo(x++); }",
        "{ x++ }",
        "foo",
        None,
    );
}

// port: FunctionToBlockMutatorTest#testMutate8
#[test]
fn test_mutate8() {
    let t = FunctionToBlockMutatorTest::set_up();
    // Parameter has side-effects.
    t.helper_mutate(
        "function foo(a){return a+a;}; foo(x++);",
        "{var a$jscomp$inline_0 = x++; a$jscomp$inline_0 + a$jscomp$inline_0;}",
        "foo",
        None,
    );
}

// port: FunctionToBlockMutatorTest#testMutate8_withConstVar
#[test]
fn test_mutate8_with_const_var() {
    let t = FunctionToBlockMutatorTest::set_up();
    // Parameter has side-effects.
    t.helper_mutate(
        "function foo(/** @const */ a){return a+a;}; foo(x++);",
        "{var a$jscomp$inline_0 = x++; a$jscomp$inline_0 + a$jscomp$inline_0;}",
        "foo",
        None,
    );
}

// port: FunctionToBlockMutatorTest#testMutateInitializeUninitializedVars1
#[test]
fn test_mutate_initialize_uninitialized_vars1() {
    let mut t = FunctionToBlockMutatorTest::set_up();
    t.is_call_in_loop = true;
    t.helper_mutate(
        "function foo(a){var b;return a;}; foo(1);",
        "{var b$jscomp$inline_1 = void 0; 1;}",
        "foo",
        None,
    );
}

// port: FunctionToBlockMutatorTest#testMutateInitializeUninitializedVars2
#[test]
fn test_mutate_initialize_uninitialized_vars2() {
    let t = FunctionToBlockMutatorTest::set_up();
    t.helper_mutate("function foo(a) {var b; for(b in c)return a;}; foo(1);", "{\n  JSCompiler_inline_label_foo_2:\n  {\n    var b$jscomp$inline_1;\n    for (b$jscomp$inline_1 in c) {\n      1;\n      break JSCompiler_inline_label_foo_2;\n    }\n  }\n}\n", "foo", None);
}

// port: FunctionToBlockMutatorTest#testMutateInitializeUninitializedVarsNestedFunction
#[test]
fn test_mutate_initialize_uninitialized_vars_nested_function() {
    let mut t = FunctionToBlockMutatorTest::set_up();
    t.is_call_in_loop = true;
    t.helper_mutate(
        "function foo(a){\n  var b = function(){ var c; };\n  return a;\n};\nfoo(1);\n",
        "{\n  var b$jscomp$inline_1 = function(){ var c$jscomp$inline_2; };\n  1;\n}\n",
        "foo",
        None,
    );
}

// port: FunctionToBlockMutatorTest#testMutateInitializeUninitializedLets1
#[test]
fn test_mutate_initialize_uninitialized_lets1() {
    let mut t = FunctionToBlockMutatorTest::set_up();
    t.is_call_in_loop = true;
    t.helper_mutate(
        "function foo(a){let b;return a;}; foo(1);",
        "{let b$jscomp$inline_1 = void 0; 1;}",
        "foo",
        None,
    );
}

// port: FunctionToBlockMutatorTest#testMutateInitializeUninitializedLets2
#[test]
fn test_mutate_initialize_uninitialized_lets2() {
    let t = FunctionToBlockMutatorTest::set_up();
    t.helper_mutate("function foo(a) {for(let b in c)return a;}; foo(1);", "{\n  JSCompiler_inline_label_foo_2:\n  {\n    for (let b$jscomp$inline_1 in c) {\n      1;\n      break JSCompiler_inline_label_foo_2;\n    }\n  }\n}\n", "foo", None);
}

// port: FunctionToBlockMutatorTest#testMutateInitializeUninitializedVarInLoop
#[test]
fn test_mutate_initialize_uninitialized_var_in_loop() {
    let mut t = FunctionToBlockMutatorTest::set_up();
    t.is_call_in_loop = true;
    // Demonstrates that `b` should be initialized to `undefined` because
    // it is used in a loop.
    t.helper_mutate("function foo(a) {\n  for (var i = 0; i < 10; i++) {\n    var b;\n    return a;\n  }\n}\nfoo(1);\n", "{\n  JSCompiler_inline_label_foo_3:\n  {\n    var b$jscomp$inline_2 = void 0;\n    var i$jscomp$inline_1 = 0;\n    for (; i$jscomp$inline_1 < 10; i$jscomp$inline_1++) {\n      1;\n      break JSCompiler_inline_label_foo_3;\n    }\n  }\n}\n", "foo", None);
}

// port: FunctionToBlockMutatorTest#testMutateInitializedVarInLoop
#[test]
fn test_mutate_initialized_var_in_loop() {
    let mut t = FunctionToBlockMutatorTest::set_up();
    t.is_call_in_loop = true;
    // Demonstrates that b's initializer should not be hoisted out of the loop or
    // lost when "foo" is inlined into the body of a loop
    t.helper_mutate("function foo(a) {\n  for (var i = 0; i < 10; i++) {\n    var b = 1;\n    return a;\n  }\n}\nfoo(1);\n", "{\n  JSCompiler_inline_label_foo_3:\n  {\n    var b$jscomp$inline_2 = void 0;\n    var i$jscomp$inline_1 = 0;\n    for (; i$jscomp$inline_1 < 10; i$jscomp$inline_1++) {\n      b$jscomp$inline_2 = 1;\n      1;\n      break JSCompiler_inline_label_foo_3;\n    }\n  }\n}\n", "foo", None);
}

// port: FunctionToBlockMutatorTest#testMutateCallInLoopVars1
#[test]
fn test_mutate_call_in_loop_vars1() {
    let mut t = FunctionToBlockMutatorTest::set_up();
    let src = "function foo(a) {\n  var B = bar();\n  a;\n};\nfoo(1);\n";
    // baseline: outside a loop, the constant remains constant.
    t.is_call_in_loop = false;
    t.helper_mutate(src, "{var B$jscomp$inline_1 = bar(); 1;}", "foo", None);
    // ... in a loop, the constant-ness is removed.
    // TODO(johnlenz): update this test to look for the const annotation.
    t.is_call_in_loop = true;
    t.helper_mutate(src, "{var B$jscomp$inline_1 = bar(); 1;}", "foo", None);
}

// port: FunctionToBlockMutatorTest#testMutateFunctionDefinition
#[test]
fn test_mutate_function_definition() {
    let t = FunctionToBlockMutatorTest::set_up();
    // Function declarations are rewritten as function expressions.
    t.helper_mutate(
        "function foo(a){function g(){}}; foo(1);",
        "{var g$jscomp$inline_1 = function() {};}",
        "foo",
        None,
    );
}

// port: FunctionToBlockMutatorTest#testMutateFunctionDefinitionHoisting
#[test]
fn test_mutate_function_definition_hoisting() {
    let t = FunctionToBlockMutatorTest::set_up();
    t.helper_mutate("function foo(a){\n  var b = g(a);\n  function g(c){ return c; }\n  var c = i();\n  function h(){}\n  function i(){}\n}\nfoo(1);\n", "{\n  var g$jscomp$inline_1 = function(c$jscomp$inline_6) {return c$jscomp$inline_6};\n  var h$jscomp$inline_2 = function(){};\n  var i$jscomp$inline_3 = function(){};\n  var b$jscomp$inline_4 = g$jscomp$inline_1(1);\n  var c$jscomp$inline_5 = i$jscomp$inline_3();\n}\n", "foo", None);
}

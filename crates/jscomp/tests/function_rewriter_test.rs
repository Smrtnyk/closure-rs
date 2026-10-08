/*
 * Copyright 2006 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/FunctionRewriterTest.java.

//! Port of `FunctionRewriterTest.java`: tests for `FunctionRewriter`.
use closure_jscomp::{compiler_pass::CompilerPass, function_rewriter::FunctionRewriter};
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc};

// port: FunctionRewriterTest#RETURNARG_HELPER
const RETURNARG_HELPER: &str = "function JSCompiler_returnArg(JSCompiler_returnArg_value){\n  return function() { return JSCompiler_returnArg_value }\n}\n";
// port: FunctionRewriterTest#GET_HELPER
const GET_HELPER: &str = "function JSCompiler_get(JSCompiler_get_name){\n  return function() { return this[JSCompiler_get_name] }\n}\n";
// port: FunctionRewriterTest#SET_HELPER
const SET_HELPER: &str = "function JSCompiler_set(JSCompiler_set_name) {\n  return function(JSCompiler_set_value){\n    this[JSCompiler_set_name]=JSCompiler_set_value\n  }\n}\n";
// port: FunctionRewriterTest#EMPTY_HELPER
const EMPTY_HELPER: &str = "function JSCompiler_emptyFn() {\n  return function(){}\n}\n";
// port: FunctionRewriterTest#IDENTITY_HELPER
const IDENTITY_HELPER: &str = "function JSCompiler_identityFn() {\n  return function(JSCompiler_identityFn_value) {\n      return JSCompiler_identityFn_value\n  }\n}\n";

struct FunctionRewriterTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for Hooks {
    // port: FunctionRewriterTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let pass: Box<dyn CompilerPass> = Box::new(FunctionRewriter::new());
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "FunctionRewriterTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl FunctionRewriterTest {
    // port: FunctionRewriterTest#setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        harness.enable_normalize().unwrap();
        // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
        harness.enable_normalize_expected_output().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "FunctionRewriterTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::new(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
            },
        }
    }

    fn test(&mut self, js: &str, expected: &str) {
        self.harness
            .test_strings(&mut self.hooks, js, expected)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    // port: FunctionRewriterTest#checkCompilesTo
    fn check_compiles_to(
        &mut self,
        src: &str,
        expected_hdr: &str,
        expected_body: &str,
        repetitions: i32,
    ) {
        let mut src_buffer = String::new();
        let mut expected_buffer = String::new();

        expected_buffer.push_str(expected_hdr);

        for idx in 0..repetitions {
            if idx != 0 {
                src_buffer.push(';');
                expected_buffer.push(';');
            }
            src_buffer.push_str(src);
            expected_buffer.push_str(expected_body);
        }
        self.test(&src_buffer, &expected_buffer);
    }

    // port: FunctionRewriterTest#checkCompilesToSame
    fn check_compiles_to_same(&mut self, src: &str, repetitions: i32) {
        self.check_compiles_to(src, "", src, repetitions);
    }
}

// port: FunctionRewriterTest#testEs6Class
#[test]
fn test_es6_class() {
    let mut t = FunctionRewriterTest::new();
    // There is never any benefit to replacing ES6 class methods
    t.check_compiles_to_same(
        "class C {\n  constructor(x = 1) { // looks like a setter\n    this.x_ = x;\n  }\n  get x() {\n    return this.x_\n  }\n  getX() {\n    return this.x_\n  }\n  set x(x) {\n    this.x_ = x;\n  }\n  setX(x = 3) {\n    this.x_ = x;\n  }\n  getConst() {\n    return 1;\n  }\n  empty() {}\n  identity(x) { return x; }\n}\n",
        10,
    );
}

// port: FunctionRewriterTest#testReplaceReturnConst1
#[test]
fn test_replace_return_const1() {
    let mut t = FunctionRewriterTest::new();
    let source = "a.prototype.foo = function() {return \"foobar\"}";
    t.check_compiles_to_same(source, 3);
    t.check_compiles_to(
        source,
        RETURNARG_HELPER,
        "a.prototype.foo = JSCompiler_returnArg(\"foobar\")",
        4,
    );
}

// port: FunctionRewriterTest#testReplaceReturnConst2
#[test]
fn test_replace_return_const2() {
    let mut t = FunctionRewriterTest::new();
    t.check_compiles_to_same("a.prototype.foo = function() {return foobar}", 10);
}

// port: FunctionRewriterTest#testReplaceReturnConst3
#[test]
fn test_replace_return_const3() {
    let mut t = FunctionRewriterTest::new();
    let source = "a.prototype.foo = function() {return void 0;}";
    t.check_compiles_to_same(source, 3);
    t.check_compiles_to(
        source,
        RETURNARG_HELPER,
        "a.prototype.foo = JSCompiler_returnArg(void 0)",
        4,
    );
}

// port: FunctionRewriterTest#testReplaceGetter1
#[test]
fn test_replace_getter1() {
    let mut t = FunctionRewriterTest::new();
    let source = "a.prototype.foo = function() {return this.foo_}";
    t.check_compiles_to_same(source, 3);
    t.check_compiles_to(
        source,
        GET_HELPER,
        "a.prototype.foo = JSCompiler_get(\"foo_\")",
        4,
    );
}

// port: FunctionRewriterTest#testReplaceGetter2
#[test]
fn test_replace_getter2() {
    let mut t = FunctionRewriterTest::new();
    t.check_compiles_to_same("a.prototype.foo = function() {return}", 10);
}

// port: FunctionRewriterTest#testDontReplaceGetterInArrow
#[test]
fn test_dont_replace_getter_in_arrow() {
    let mut t = FunctionRewriterTest::new();
    t.check_compiles_to_same(
        "class C { foo() { return (x) => { return this.x; }; } }",
        10,
    );
    t.check_compiles_to_same("class C { foo() { return (x) => this.x; } }", 10);
}

// port: FunctionRewriterTest#testReplaceSetter1
#[test]
fn test_replace_setter1() {
    let mut t = FunctionRewriterTest::new();
    let source = "a.prototype.foo = function(v) {this.foo_ = v}";
    t.check_compiles_to_same(source, 4);
    t.check_compiles_to(
        source,
        SET_HELPER,
        "a.prototype.foo = JSCompiler_set(\"foo_\")",
        5,
    );
}

// port: FunctionRewriterTest#testReplaceSetterWithDefault
#[test]
fn test_replace_setter_with_default() {
    let mut t = FunctionRewriterTest::new();
    let source = "a.prototype.foo = function(v = 1) {this.foo_ = v}";
    t.check_compiles_to_same(source, 10);
}

// port: FunctionRewriterTest#testReplaceSetterWithDestructuring
#[test]
fn test_replace_setter_with_destructuring() {
    let mut t = FunctionRewriterTest::new();
    let source = "a.prototype.foo = function({v}) {this.foo_ = v}";
    t.check_compiles_to_same(source, 10);
}

// port: FunctionRewriterTest#testReplaceSetter2
#[test]
fn test_replace_setter2() {
    let mut t = FunctionRewriterTest::new();
    let source = "a.prototype.foo = function(v, v2) {this.foo_ = v}";
    t.check_compiles_to_same(source, 3);
    t.check_compiles_to(
        source,
        SET_HELPER,
        "a.prototype.foo = JSCompiler_set(\"foo_\")",
        4,
    );
}

// port: FunctionRewriterTest#testReplaceSetter3
#[test]
fn test_replace_setter3() {
    let mut t = FunctionRewriterTest::new();
    t.check_compiles_to_same("a.prototype.foo = function() {this.foo_ = v}", 10);
}

// port: FunctionRewriterTest#testReplaceSetter4
#[test]
fn test_replace_setter4() {
    let mut t = FunctionRewriterTest::new();
    t.check_compiles_to_same("a.prototype.foo = function(v, v2) {this.foo_ = v2}", 10);
}

// port: FunctionRewriterTest#testDontReplaceSetterInArrow
#[test]
fn test_dont_replace_setter_in_arrow() {
    let mut t = FunctionRewriterTest::new();
    t.check_compiles_to_same("class C { foo() { return (x) => { this.x = x; }; } }", 10);
    t.check_compiles_to_same("class C { foo() { return (x) => this.x = x; } }", 10);
}

// port: FunctionRewriterTest#testReplaceEmptyFunction1
#[test]
fn test_replace_empty_function1() {
    let mut t = FunctionRewriterTest::new();
    let source = "a.prototype.foo = function() {}";
    t.check_compiles_to_same(source, 4);
    t.check_compiles_to(
        source,
        EMPTY_HELPER,
        "a.prototype.foo = JSCompiler_emptyFn()",
        5,
    );
}

// port: FunctionRewriterTest#testReplaceEmptyFunction2
#[test]
fn test_replace_empty_function2() {
    let mut t = FunctionRewriterTest::new();
    t.check_compiles_to_same("function foo() {}", 10);
}

// port: FunctionRewriterTest#testReplaceEmptyFunction3
#[test]
fn test_replace_empty_function3() {
    let mut t = FunctionRewriterTest::new();
    let source = "var foo = function() {}";
    t.check_compiles_to_same(source, 4);
    t.check_compiles_to(source, EMPTY_HELPER, "var foo = JSCompiler_emptyFn()", 5);
}

// port: FunctionRewriterTest#testReplaceEmptyFunction4
#[test]
fn test_replace_empty_function4() {
    let mut t = FunctionRewriterTest::new();
    let source = "var foo = async function() {}";
    t.check_compiles_to_same(source, 10);
}

// port: FunctionRewriterTest#testReplaceEmptyFunction5
#[test]
fn test_replace_empty_function5() {
    let mut t = FunctionRewriterTest::new();
    let source = "var foo = function *() {}";
    t.check_compiles_to_same(source, 10);
}

// port: FunctionRewriterTest#testReplaceEmptyArrowFunction
#[test]
fn test_replace_empty_arrow_function() {
    let mut t = FunctionRewriterTest::new();
    let source = "var foo = () => {}";
    t.check_compiles_to_same(source, 10);
    t.check_compiles_to(source, EMPTY_HELPER, "var foo = JSCompiler_emptyFn()", 30);
}

// port: FunctionRewriterTest#testReplaceIdentityFunction1
#[test]
fn test_replace_identity_function1() {
    let mut t = FunctionRewriterTest::new();
    let source = "a.prototype.foo = function(a) {return a}";
    t.check_compiles_to_same(source, 2);
    t.check_compiles_to(
        source,
        IDENTITY_HELPER,
        "a.prototype.foo = JSCompiler_identityFn()",
        3,
    );
}

// port: FunctionRewriterTest#testReplaceIdentityFunctionWithDefault
#[test]
fn test_replace_identity_function_with_default() {
    let mut t = FunctionRewriterTest::new();
    t.check_compiles_to_same("a.prototype.foo = function(a = 1) {return a}", 10);
}

// port: FunctionRewriterTest#testReplaceIdentityFunctionWithDestructuring
#[test]
fn test_replace_identity_function_with_destructuring() {
    let mut t = FunctionRewriterTest::new();
    t.check_compiles_to_same("a.prototype.foo = function({a}) {return a}", 10);
}

// port: FunctionRewriterTest#testReplaceIdentityFunction2
#[test]
fn test_replace_identity_function2() {
    let mut t = FunctionRewriterTest::new();
    t.check_compiles_to_same("a.prototype.foo = function(a) {return a + 1}", 10);
}

// port: FunctionRewriterTest#testReplaceIdentityFunctionAsArrow
#[test]
fn test_replace_identity_function_as_arrow() {
    let mut t = FunctionRewriterTest::new();
    t.check_compiles_to(
        "a.prototype.foo = (a) => { return a; }",
        IDENTITY_HELPER,
        "a.prototype.foo = JSCompiler_identityFn()",
        20,
    );
}

// port: FunctionRewriterTest#testReplaceIdentityFunctionAsBlocklessArrow
#[test]
fn test_replace_identity_function_as_blockless_arrow() {
    let mut t = FunctionRewriterTest::new();
    let source = "a.prototype.foo = (a) => a";
    t.check_compiles_to_same(source, 2);
    t.check_compiles_to(
        source,
        IDENTITY_HELPER,
        "a.prototype.foo = JSCompiler_identityFn()",
        50,
    );
}

// port: FunctionRewriterTest#testIssue538
#[test]
fn test_issue538() {
    let mut t = FunctionRewriterTest::new();
    t.check_compiles_to_same(
        "/** @constructor */\nWebInspector.Setting = function() {}\nWebInspector.Setting.prototype = {\n    get name0(){return this._name;},\n    get name1(){return this._name;},\n    get name2(){return this._name;},\n    get name3(){return this._name;},\n    get name4(){return this._name;},\n    get name5(){return this._name;},\n    get name6(){return this._name;},\n    get name7(){return this._name;},\n    get name8(){return this._name;},\n    get name9(){return this._name;},\n}\n",
        1,
    );
}

// port: FunctionRewriterTest#testNoRewriteWithNonSimpleParameters
#[test]
fn test_no_rewrite_with_non_simple_parameters() {
    let mut t = FunctionRewriterTest::new();
    // Empty function with default parameter side effect
    t.check_compiles_to_same("a.prototype.foo = function(a = audit()) {}", 10);
    // Empty function with rest parameter
    t.check_compiles_to_same("a.prototype.foo = function(...a) {}", 10);
    // Empty function with destructuring parameter
    t.check_compiles_to_same("a.prototype.foo = function({a}) {}", 10);

    // Return constant with default parameter side effect
    t.check_compiles_to_same("a.prototype.foo = function(a = audit()) { return 1; }", 10);
    // Return constant with rest parameter
    t.check_compiles_to_same("a.prototype.foo = function(...a) { return 1; }", 10);

    // Getter with default parameter side effect
    t.check_compiles_to_same(
        "a.prototype.foo = function(a = audit()) { return this.x; }",
        10,
    );
    // Getter with destructuring parameter
    t.check_compiles_to_same("a.prototype.foo = function({a}) { return this.x; }", 10);

    // Setter with default parameter side effect in a second parameter
    t.check_compiles_to_same(
        "a.prototype.foo = function(v, a = audit()) { this.x = v; }",
        10,
    );
    // Setter with destructuring in a second parameter
    t.check_compiles_to_same("a.prototype.foo = function(v, {a}) { this.x = v; }", 10);

    // Identity with default parameter side effect in a second parameter
    t.check_compiles_to_same(
        "a.prototype.foo = function(v, a = audit()) { return v; }",
        10,
    );
    // Identity with destructuring in a second parameter
    t.check_compiles_to_same("a.prototype.foo = function(v, {a}) { return v; }", 10);
}

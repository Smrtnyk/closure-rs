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
//   test/com/google/javascript/jscomp/StrictModeCheckTest.java.

//! Port of StrictModeCheckTest (a CompilerTestCase) on the Rust CompilerTestCase port.
mod var_checks_support;

use closure_jscomp::{
    check_level::CheckLevel,
    strict_mode_check::{
        ARGUMENTS_ASSIGNMENT, ARGUMENTS_CALLEE_FORBIDDEN, ARGUMENTS_CALLER_FORBIDDEN,
        ARGUMENTS_DECLARATION, DELETE_VARIABLE, DUPLICATE_MEMBER, EVAL_ASSIGNMENT,
        EVAL_DECLARATION, FUNCTION_ARGUMENTS_PROP_FORBIDDEN, FUNCTION_CALLER_FORBIDDEN,
        StrictModeCheck, USE_OF_WITH,
    },
};
use closure_testing::compiler_test_case::CompilerTestCase;
use var_checks_support::{Hooks, default_externs, test_same, test_warning};

// port: StrictModeCheckTest#EXTERNS
fn externs() -> String {
    default_externs() + "var arguments; function eval(str) {}"
}

// port: StrictModeCheckTest#setUp (with StrictModeCheckTest#StrictModeCheckTest and #getProcessor)
fn set_up() -> (CompilerTestCase, Hooks) {
    let mut harness = CompilerTestCase::new(externs());
    harness.set_up();
    harness.disable_type_check().unwrap();
    // port: StrictModeCheckTest#getProcessor
    let hooks = Hooks::new("StrictModeCheckTest", |c| {
        Box::new(StrictModeCheck::new(&c.borrow(), CheckLevel::WARNING))
    });
    (harness, hooks)
}

// port: StrictModeCheckTest#testSameEs6Strict
fn test_same_es6_strict(t: &mut CompilerTestCase, h: &mut Hooks, js: &str) {
    test_same(t, h, js);
}

#[test]
fn test_use_of_with1() {
    let (mut t, mut h) = set_up();
    test_warning(&mut t, &mut h, "var a; with(a){}", &USE_OF_WITH);
}

#[test]
fn test_use_of_with2() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "var a;\n/** @suppress {with} */\nwith(a){}\n",
    );
}

#[test]
fn test_use_of_with3() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "function f(expr, context) {\n  try {\n    /** @suppress{with} */ with (context) {\n      return eval('[' + expr + '][0]');\n    }\n  } catch (e) {\n    return null;\n  }\n};\n",
    );
}

#[test]
fn test_eval2() {
    let (mut t, mut h) = set_up();
    test_warning(&mut t, &mut h, "function foo(eval) {}", &EVAL_DECLARATION);
}

#[test]
fn test_eval3() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "function foo() {} foo.eval = 3;");
}

#[test]
fn test_eval4() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "function foo() { var eval = 3; }",
        &EVAL_DECLARATION,
    );
}

#[test]
fn test_eval5() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "/** @suppress {duplicate} */ function eval() {}",
        &EVAL_DECLARATION,
    );
}

#[test]
fn test_eval6() {
    let (mut t, mut h) = set_up();
    test_warning(&mut t, &mut h, "try {} catch (eval) {}", &EVAL_DECLARATION);
}

#[test]
fn test_eval7() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "var o = {eval: 3};");
}

#[test]
fn test_eval8() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "var a; eval: while (true) { a = 3; }");
}

#[test]
fn test_unknown_variable3() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "try {} catch (ex) { ex = 3; }");
}

#[test]
fn test_unknown_variable4() {
    let (mut t, mut h) = set_up();
    t.disable_type_check().unwrap();
    test_same_es6_strict(&mut t, &mut h, "function foo(a) { let b; a = b; }");
    test_same_es6_strict(&mut t, &mut h, "function foo(a) { const b = 42; a = b; }");
}

#[test]
fn test_arguments() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "function foo(arguments) {}",
        &ARGUMENTS_DECLARATION,
    );
}

#[test]
fn test_arguments2() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "function foo() { var arguments = 3; }",
        &ARGUMENTS_DECLARATION,
    );
}

#[test]
fn test_arguments3() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "/** @suppress {duplicate,checkTypes} */ function arguments() {}",
        &ARGUMENTS_DECLARATION,
    );
}

#[test]
fn test_arguments4() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "try {} catch (arguments) {}",
        &ARGUMENTS_DECLARATION,
    );
}

#[test]
fn test_arguments5() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "var o = {arguments: 3};");
}

#[test]
fn test_arguments6() {
    let (mut t, mut h) = set_up();
    t.disable_type_check().unwrap();
    test_same(&mut t, &mut h, "(() => arguments)();");
}

#[test]
fn test_arguments_callee() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "function foo() {arguments.callee}",
        &ARGUMENTS_CALLEE_FORBIDDEN,
    );
}

#[test]
fn test_arguments_caller() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "function foo() {arguments.caller}",
        &ARGUMENTS_CALLER_FORBIDDEN,
    );
}

#[test]
fn test_function_caller_prop() {
    let (mut t, mut h) = set_up();
    t.enable_type_check().unwrap();
    test_warning(
        &mut t,
        &mut h,
        "function foo() {foo.caller}",
        &FUNCTION_CALLER_FORBIDDEN,
    );
}

#[test]
fn test_function_arguments_prop() {
    let (mut t, mut h) = set_up();
    t.enable_type_check().unwrap();
    test_warning(
        &mut t,
        &mut h,
        "function foo() {foo.arguments}",
        &FUNCTION_ARGUMENTS_PROP_FORBIDDEN,
    );
}

#[test]
fn test_eval_assignment() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "/** @suppress {checkTypes} */ function foo() { eval = []; }",
        &EVAL_ASSIGNMENT,
    );
}

#[test]
fn test_assign_to_arguments() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "function foo() { arguments = []; }",
        &ARGUMENTS_ASSIGNMENT,
    );
}

#[test]
fn test_delete_var() {
    let (mut t, mut h) = set_up();
    test_warning(&mut t, &mut h, "var a; delete a", &DELETE_VARIABLE);
}

#[test]
fn test_delete_function() {
    let (mut t, mut h) = set_up();
    test_warning(&mut t, &mut h, "function a() {} delete a", &DELETE_VARIABLE);
}

#[test]
fn test_delete_argument() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "function b(a) { delete a; }",
        &DELETE_VARIABLE,
    );
}

#[test]
fn test_valid_delete() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "var obj = { a: 0 }; delete obj.a;");
    test_same(
        &mut t,
        &mut h,
        "var obj = { a: function() {} }; delete obj.a;",
    );
    test_same_es6_strict(&mut t, &mut h, "var obj = { a(){} }; delete obj.a;");
    test_same_es6_strict(&mut t, &mut h, "var obj = { a }; delete obj.a;");
}

#[test]
fn test_delete_property() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "/** @suppress {checkTypes} */ function f(obj) { delete obj.a; }",
    );
}

#[test]
fn test_allow_numbers_as_objlit_keys() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "var o = {1: 3, 2: 4};");
}

#[test]
fn test_duplicate_object_literal_key() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "var o = {a: 1, b: 2, c: 3};");
    test_same(&mut t, &mut h, "var x = { get a() {}, set a(p) {} };");
    test_warning(
        &mut t,
        &mut h,
        "var o = {a: 1, b: 2, a: 3};",
        &DUPLICATE_MEMBER,
    );
    test_warning(
        &mut t,
        &mut h,
        "var x = { get a() {}, get a() {} };",
        &DUPLICATE_MEMBER,
    );
    test_warning(
        &mut t,
        &mut h,
        "var x = { get a() {}, a: 1 };",
        &DUPLICATE_MEMBER,
    );
    test_warning(
        &mut t,
        &mut h,
        "var x = { set a(p) {}, a: 1 };",
        &DUPLICATE_MEMBER,
    );
    test_same(
        &mut t,
        &mut h,
        "'use strict';\n/** @constructor */ function App() {}\nApp.prototype = {\n  get appData() { return this.appData_; },\n  set appData(data) { this.appData_ = data; }\n};\n",
    );
    test_warning(&mut t, &mut h, "var x = {a: 2, a(){}}", &DUPLICATE_MEMBER);
    test_warning(&mut t, &mut h, "var x = {a, a(){}}", &DUPLICATE_MEMBER);
    test_warning(&mut t, &mut h, "var x = {a(){}, a(){}}", &DUPLICATE_MEMBER);
}

#[test]
fn test_function_decl() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "function g() {}");
    test_same(&mut t, &mut h, "var g = function() {};");
    test_same(&mut t, &mut h, "(function() {})();");
    test_same(&mut t, &mut h, "(function() {});");
    test_same(&mut t, &mut h, &in_fn("function g() {}"));
    test_same(&mut t, &mut h, &in_fn("var g = function() {};"));
    test_same(&mut t, &mut h, &in_fn("(function() {})();"));
    test_same(&mut t, &mut h, &in_fn("(function() {});"));
    test_same(&mut t, &mut h, "{var g = function () {}}");
    test_same(&mut t, &mut h, "{(function g() {})()}");
    test_same(&mut t, &mut h, "var x;if (x) {var g = function () {}}");
    test_same(&mut t, &mut h, "var x;if (x) {(function g() {})()}");
}

#[test]
fn test_class() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "class A {\n  method1() {}\n  method2() {}\n}\n",
    );
    // Duplicate class methods test
    test_warning(
        &mut t,
        &mut h,
        "class A {\n  method1() {}\n  method1() {}\n}\n",
        &DUPLICATE_MEMBER,
    );
    // Function declaration / call test.
    // The two following tests should have reported FUNCTION_CALLER_FORBIDDEN and
    // FUNCTION_ARGUMENTS_PROP_FORBIDDEN. Typecheck needed for them to work.
    // TODO(user): Add tests for these after typecheck supports class.
    test_same(
        &mut t,
        &mut h,
        "class A {\n  method() {this.method.caller}\n}\n",
    );
    test_same(
        &mut t,
        &mut h,
        "class A {\n  method() {this.method.arguments}\n}\n",
    );
    // Duplicate obj literal key in classes
    test_warning(
        &mut t,
        &mut h,
        "class A {\n  method() {\n    var obj = {a : 1, a : 2}\n  }\n}\n",
        &DUPLICATE_MEMBER,
    );
    // Delete test. Class methods are configurable, thus deletable.
    test_same(
        &mut t,
        &mut h,
        "class A {\n  methodA() {}\n  methodB() {delete this.methodA}\n}\n",
    );
    // Use of with test
    test_warning(
        &mut t,
        &mut h,
        "class A {\n  constructor() {this.x = 1;}\n  method() {\n    with (this.x) {}\n  }\n}\n",
        &USE_OF_WITH,
    );
    // Eval errors test
    test_warning(
        &mut t,
        &mut h,
        "class A {\n  method(eval) {}\n}\n",
        &EVAL_DECLARATION,
    );
    test_warning(
        &mut t,
        &mut h,
        "class A {\n  method() {var eval = 1;}\n}\n",
        &EVAL_DECLARATION,
    );
    test_warning(
        &mut t,
        &mut h,
        "class A {\n  method() {eval = 1}\n}\n",
        &EVAL_ASSIGNMENT,
    );
    // Use of 'arguments'
    test_warning(
        &mut t,
        &mut h,
        "class A {\n  method(arguments) {}\n}\n",
        &ARGUMENTS_DECLARATION,
    );
    test_warning(
        &mut t,
        &mut h,
        "class A {\n  method() {var arguments = 1;}\n}\n",
        &ARGUMENTS_DECLARATION,
    );
    test_warning(
        &mut t,
        &mut h,
        "class A {\n  method() {arguments = 1}\n}\n",
        &ARGUMENTS_ASSIGNMENT,
    );
    test_warning(
        &mut t,
        &mut h,
        "class A {\n  method() {arguments.callee}\n}\n",
        &ARGUMENTS_CALLEE_FORBIDDEN,
    );
    test_warning(
        &mut t,
        &mut h,
        "class A {\n  method() {arguments.caller}\n}\n",
        &ARGUMENTS_CALLER_FORBIDDEN,
    );
}

#[test]
fn test_computed_prop_in_class() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "class Example {\n  [computed()]() {}\n  [computed()]() {}\n}\n",
    );
}

#[test]
fn test_static_and_nonstatic_method_with_same_name() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "class Example {\n  foo() {}\n  static foo() {}\n}\n",
    );
}

#[test]
fn test_static_and_nonstatic_getter_with_same_name() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "class Example {\n  get foo() {}\n  static get foo() {}\n}\n",
    );
}

#[test]
fn test_static_and_nonstatic_setter_with_same_name() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "class Example {\n  set foo(x) {}\n  static set foo(x) {}\n}\n",
    );
}

#[test]
fn test_class_with_empty_members() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "class Foo { dup() {}; dup() {}; }",
        &DUPLICATE_MEMBER,
    );
}

#[test]
fn test_class_duplicate_field_error() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "class Example {\n  a;\n  a;\n}\n",
        &DUPLICATE_MEMBER,
    );
    test_warning(
        &mut t,
        &mut h,
        "class Example {\n  static a = 2;\n  static a = 3;\n}\n",
        &DUPLICATE_MEMBER,
    );
}

#[test]
fn test_class_duplicate_static_field_error() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "class Example {\n  static a = 2;\n  static a = 3;\n}\n",
        &DUPLICATE_MEMBER,
    );
    test_warning(
        &mut t,
        &mut h,
        "class Example {\n  static a;\n  static a;\n}\n",
        &DUPLICATE_MEMBER,
    );
}

#[test]
fn test_class_fields_no_error() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "class Example {\n  a;\n  static a;\n}\n");
}

#[test]
fn test_class_static_blocks_duplicates() {
    let (mut t, mut h) = set_up();
    // testing that duplicates are ok in class static blocks
    test_same(&mut t, &mut h, "class Foo {static {this.a; this.a;}}");
    // even if they are in 2 different static blocks
    test_same(
        &mut t,
        &mut h,
        "class Foo {static {this.a} static {this.a}}",
    );
    // testing that duplicates between class static blocks and public fields are ok
    test_same(&mut t, &mut h, "class Foo {static x; static {this.x}}");
    // duplicate functions are also okay
    test_same(
        &mut t,
        &mut h,
        "class Foo {static f() {}; static{this.f = function g() {}}}",
    );
    // testing that duplicates in obj literals are still invalid
    test_warning(
        &mut t,
        &mut h,
        "class Foo {static {var obj = {a : 1, a : 2};}}",
        &DUPLICATE_MEMBER,
    );
}

#[test]
fn test_class_static_blocks_with() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "class A {\n  static x;\n  static {\n    with (this.x) {}\n  }\n}\n",
        &USE_OF_WITH,
    );
}

#[test]
fn test_class_static_blocks_eval() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "class A { static {var eval;}}",
        &EVAL_DECLARATION,
    );
    test_warning(
        &mut t,
        &mut h,
        "class A { static {function eval() {};}}",
        &EVAL_DECLARATION,
    );
    test_warning(
        &mut t,
        &mut h,
        "class A { static {this.e = function eval() {};}}",
        &EVAL_DECLARATION,
    );
    test_warning(
        &mut t,
        &mut h,
        "class A { static {eval = 3;}}",
        &EVAL_ASSIGNMENT,
    );
    // eval as a field is okay
    test_same(&mut t, &mut h, "class A { static {this.eval;}}");
}

#[test]
fn test_class_static_blocks_arguments() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "class A {\n  static {var arguments = 1;}\n}\n",
        &ARGUMENTS_DECLARATION,
    );
    test_warning(
        &mut t,
        &mut h,
        "class A {\n  static {arguments = 1}\n}\n",
        &ARGUMENTS_ASSIGNMENT,
    );
    test_warning(
        &mut t,
        &mut h,
        "class A {\n  static {arguments.callee}\n}\n",
        &ARGUMENTS_CALLEE_FORBIDDEN,
    );
    test_warning(
        &mut t,
        &mut h,
        "class A {\n  static {arguments.caller}\n}\n",
        &ARGUMENTS_CALLER_FORBIDDEN,
    );
}

#[test]
fn test_class_static_block_delete() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "class A {static{this.a; delete this.a;}}");
    test_same(&mut t, &mut h, "class A {static a; static{delete this.a;}}");
    test_warning(
        &mut t,
        &mut h,
        "class A {static a; static{var a; delete a;}}",
        &DELETE_VARIABLE,
    );
    test_same(
        &mut t,
        &mut h,
        "class A {static f() {}; static{delete this.f;}}",
    );
}

// port: StrictModeCheckTest#inFn
fn in_fn(body: &str) -> String {
    format!("function func() {{{body}}}")
}

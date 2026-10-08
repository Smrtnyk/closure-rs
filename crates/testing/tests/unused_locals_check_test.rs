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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/UnusedLocalsCheckTest.java.

//! Port of UnusedLocalsCheckTest (a CompilerTestCase) on the Rust CompilerTestCase port: test that
//! warnings are generated in appropriate cases and appropriate cases only by
//! VariableReferenceCheck.
mod var_checks_support;

use closure_jscomp::{
    check_level::CheckLevel,
    deps::module_loader::INVALID_MODULE_PATH,
    diagnostic_groups::UNUSED_LOCAL_VARIABLE,
    variable_reference_check::{EARLY_REFERENCE, UNUSED_LOCAL_ASSIGNMENT, VariableReferenceCheck},
};
use closure_testing::compiler_test_case::CompilerTestCase;
use var_checks_support::{Hooks, test_same, test_warning};

// port: UnusedLocalsCheckTest#setUp (with #getOptions and #getProcessor)
fn set_up() -> (CompilerTestCase, Hooks) {
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    harness.ignore_warnings(&[&INVALID_MODULE_PATH]).unwrap();
    // port: UnusedLocalsCheckTest#getProcessor
    // Treats bad reads as errors, and reports bad write warnings.
    let hooks = Hooks::new("UnusedLocalsCheckTest", |c| {
        Box::new(VariableReferenceCheck::new(&c.borrow()))
    })
    // port: UnusedLocalsCheckTest#getOptions
    .with_options(|options| {
        options.set_warning_level(UNUSED_LOCAL_VARIABLE.clone(), CheckLevel::WARNING);
    });
    (harness, hooks)
}

// port: UnusedLocalsCheckTest#assertEarlyReferenceWarning
fn assert_early_reference_warning(t: &mut CompilerTestCase, h: &mut Hooks, js: &str) {
    test_warning(t, h, js, &EARLY_REFERENCE);
}

/// Expects the JS to generate one unused local error.
// port: UnusedLocalsCheckTest#assertUnused
fn assert_unused(t: &mut CompilerTestCase, h: &mut Hooks, js: &str) {
    test_warning(t, h, js, &UNUSED_LOCAL_ASSIGNMENT);
}

/// Expects the JS to generate no errors or warnings.
// port: UnusedLocalsCheckTest#assertNoWarning
fn assert_no_warning(t: &mut CompilerTestCase, h: &mut Hooks, js: &str) {
    test_same(t, h, js);
}

#[test]
fn test_unused_local_var() {
    let (mut t, mut h) = set_up();
    assert_unused(&mut t, &mut h, "function f() { var a; }");
    assert_unused(&mut t, &mut h, "function f() { var a = 2; }");
    assert_unused(&mut t, &mut h, "function f() { var a; a = 2; }");
}

#[test]
fn test_unused_local_var_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_unused(&mut t, &mut h, "export function f() { var a; }");
}

#[test]
fn test_unused_typedef_in_module() {
    let (mut t, mut h) = set_up();
    assert_unused(&mut t, &mut h, "goog.module('m'); var x;");
    assert_unused(&mut t, &mut h, "goog.module('m'); let x;");
    test_same(
        &mut t,
        &mut h,
        "goog.module('m'); /** @typedef {string} */ var x;",
    );
    test_same(
        &mut t,
        &mut h,
        "goog.module('m'); /** @typedef {string} */ let x;",
    );
}

#[test]
fn test_unused_typedef_in_es6_module() {
    let (mut t, mut h) = set_up();
    assert_unused(&mut t, &mut h, "import 'm'; var x;");
    assert_unused(&mut t, &mut h, "import 'm'; let x;");
    test_same(
        &mut t,
        &mut h,
        "import 'm'; /** @typedef {string} */ var x;",
    );
}

#[test]
fn test_alias_in_module() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "goog.module('m');\nconst x = goog.require('x');\nconst y = x.y;\n/** @type {y} */ var z;\nalert(z);\n",
    );
}

#[test]
fn test_alias_in_es6_module() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "import 'm';\nimport x from 'x';\nexport const y = x.y;\nexport /** @type {y} */ var z;\nalert(z);\n",
    );
}

#[test]
fn test_unused_import() {
    let (mut t, mut h) = set_up();
    // TODO(b/64566470): This test should give an UNUSED_LOCAL_ASSIGNMENT error for x.
    test_same(&mut t, &mut h, "import x from 'Foo';");
}

#[test]
fn test_exported_type() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "export class Foo {}\nexport /** @type {Foo} */ var y;\n",
    );
}

/// Inside a goog.scope, don't warn because the alias might be used in a type annotation.
#[test]
fn test_unused_local_var_in_goog_scope() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "goog.scope(function f() { var a; });");
    test_same(
        &mut t,
        &mut h,
        "goog.scope(function f() { /** @typedef {some.long.name} */ var a; });",
    );
    test_same(
        &mut t,
        &mut h,
        "goog.scope(function f() { var a = some.long.name; });",
    );
}

#[test]
fn test_unused_local_let() {
    let (mut t, mut h) = set_up();
    assert_unused(&mut t, &mut h, "function f() { let a; }");
    assert_unused(&mut t, &mut h, "function f() { let a = 2; }");
    assert_unused(&mut t, &mut h, "function f() { let a; a = 2; }");
}

#[test]
fn test_unused_local_let_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_unused(&mut t, &mut h, "export function f() { let a; }");
}

#[test]
fn test_unused_local_const() {
    let (mut t, mut h) = set_up();
    assert_unused(&mut t, &mut h, "function f() { const a = 2; }");
}

#[test]
fn test_unused_local_const_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_unused(&mut t, &mut h, "export function f() { const a = 2; }");
}

#[test]
fn test_unused_local_arg_no_warning() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "function f(a) {}");
}

#[test]
fn test_unused_local_arg_no_warning_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "export function f(a) {}");
}

#[test]
fn test_unused_global_no_warning() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "var a = 2;");
}

#[test]
fn test_unused_global_no_warning_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "export var a = 2;");
}

#[test]
fn test_unused_global_in_block_no_warning() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "if (true) { var a = 2; }");
}

#[test]
fn test_unused_local_in_block() {
    let (mut t, mut h) = set_up();
    assert_unused(&mut t, &mut h, "if (true) { let a = 2; }");
    assert_unused(&mut t, &mut h, "if (true) { const a = 2; }");
}

#[test]
fn test_unused_assigned_in_inner_function() {
    let (mut t, mut h) = set_up();
    assert_unused(
        &mut t,
        &mut h,
        "function f() { var x = 1; function g() { x = 2; } }",
    );
}

#[test]
fn test_unused_assigned_in_inner_function_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_unused(
        &mut t,
        &mut h,
        "export function f() { var x = 1; function g() { x = 2; } }",
    );
}

#[test]
fn test_increment_decrement_result_used() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "function f() { var x = 5; while (x-- > 0) {} }",
    );
    assert_no_warning(
        &mut t,
        &mut h,
        "function f() { var x = -5; while (x++ < 0) {} }",
    );
    assert_no_warning(
        &mut t,
        &mut h,
        "function f() { var x = 5; while (--x > 0) {} }",
    );
    assert_no_warning(
        &mut t,
        &mut h,
        "function f() { var x = -5; while (++x < 0) {} }",
    );
}

#[test]
fn test_increment_decrement_result_used_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "export function f() { var x = 5; while (x-- > 0) {} }",
    );
}

#[test]
fn test_used_in_inner_function() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "function f() { var x = 1; function g() { use(x); } }",
    );
}

#[test]
fn test_used_in_inner_function_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "export function f() { var x = 1; function g() { use(x); } }",
    );
}

#[test]
fn test_used_in_shorthand_obj_lit() {
    let (mut t, mut h) = set_up();
    assert_early_reference_warning(&mut t, &mut h, "var z = {x}; z(); var x;");
    test_same(&mut t, &mut h, "var {x} = foo();");
    test_same(&mut t, &mut h, "var {x} = {};");
    // TODO(moz): Maybe add a warning for this case
    test_same(&mut t, &mut h, "function f() { var x = 1; return {x}; }");
}

#[test]
fn test_used_in_shorthand_obj_lit_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_early_reference_warning(&mut t, &mut h, "export var z = {x}; z(); var x;");
    test_same(&mut t, &mut h, "export var {x} = foo();");
}

#[test]
fn test_unused_catch() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "function f() { try {} catch (x) {} }");
}

#[test]
fn test_unused_catch_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "export function f() { try {} catch (x) {} }",
    );
}

#[test]
fn test_increment_counts_as_use() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "var a = 2; var b = []; b[a++] = 1;");
}

#[test]
fn test_increment_counts_as_use_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "export var a = 2; var b = []; b[a++] = 1;");
}

#[test]
fn test_for_in() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "for (var prop in obj) {}");
    assert_no_warning(&mut t, &mut h, "for (prop in obj) {}");
    assert_no_warning(&mut t, &mut h, "var prop; for (prop in obj) {}");
}

#[test]
fn test_unused_compound_assign() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "var x = 0; function f() { return x += 1; }");
    assert_no_warning(&mut t, &mut h, "var x = 0; var f = () => x += 1;");
    assert_no_warning(
        &mut t,
        &mut h,
        "function f(elapsed) {\n  let fakeMs = 0;\n  stubs.replace(Date, 'now', () => fakeMs += elapsed);\n}\n",
    );
    assert_no_warning(
        &mut t,
        &mut h,
        "function f(elapsed) {\n  let fakeMs = 0;\n  stubs.replace(Date, 'now', () => fakeMs -= elapsed);\n}\n",
    );
}

#[test]
fn test_unused_compound_assign_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "export function f(elapsed) {\n  let fakeMs = 0;\n  stubs.replace(Date, 'now', () => fakeMs -= elapsed);\n}\n",
    );
}

#[test]
fn test_chained_assign() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "var a, b = 0, c; a = b = c; alert(a);");
    assert_unused(
        &mut t,
        &mut h,
        "function foo() {\n  var a, b = 0, c;\n  a = b = c;\n  alert(a);\n}\nfoo();\n",
    );
}

#[test]
fn test_chained_assign_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "export var a, b = 0, c; a = b = c; alert(a);",
    );
}

#[test]
fn test_goog_module() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "goog.module('example'); var X = 3; use(X);");
    assert_unused(&mut t, &mut h, "goog.module('example'); var X = 3;");
}

#[test]
fn test_es6_module() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "import 'example'; var X = 3; use(X);");
    assert_unused(&mut t, &mut h, "import 'example'; var X = 3;");
}

#[test]
fn test_goog_module_bundled() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "goog.loadModule(function(exports) { 'use strict';\ngoog.module('example'); var X = 3; use(X);\nreturn exports; });\n",
    );
    assert_unused(
        &mut t,
        &mut h,
        "goog.loadModule(function(exports) { 'use strict';\ngoog.module('example'); var X = 3;\nreturn exports; });\n",
    );
}

#[test]
fn test_goog_module_destructuring() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "goog.module('example'); var {x} = goog.require('y'); use(x);",
    );
    // We could warn here, but it's already caught by the extra require check.
    assert_no_warning(
        &mut t,
        &mut h,
        "goog.module('example'); var {x} = goog.require('y');",
    );
}

#[test]
fn test_es6_module_destructuring() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "import 'example'; import {x} from 'y'; use(x);",
    );
    assert_no_warning(
        &mut t,
        &mut h,
        "import 'example'; import {x as x} from 'y'; use(x);",
    );
    assert_no_warning(
        &mut t,
        &mut h,
        "import 'example'; import {y as x} from 'y'; use(x);",
    );
}

#[test]
fn test_goog_module_require() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "goog.module('example'); var X = goog.require('foo.X'); use(X);",
    );
    // We could warn here, but it's already caught by the extra require check.
    assert_no_warning(
        &mut t,
        &mut h,
        "goog.module('example'); var X = goog.require('foo.X');",
    );
}

#[test]
fn test_es6_module_import() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "import 'example'; import X from 'foo.X'; use(X);",
    );
}

#[test]
fn test_goog_module_forward_declare() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "goog.module('example');\n\nvar X = goog.forwardDeclare('foo.X');\n\n/** @type {X} */ var x = 0;\nalert(x);\n",
    );
    assert_no_warning(
        &mut t,
        &mut h,
        "goog.module('example'); var X = goog.forwardDeclare('foo.X');",
    );
}

#[test]
fn test_goog_module_require_type() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "goog.module('example'); var X = goog.requireType('foo.X');",
    );
}

#[test]
fn test_goog_module_used_in_type_annotation() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "goog.module('example'); var X = goog.require('foo.X'); /** @type {X} */ var y; use(y);",
    );
}

#[test]
fn test_es6_module_used_in_type_annotation() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "import 'example'; import X from 'foo.X'; export /** @type {X} */ var y; use(y);",
    );
}

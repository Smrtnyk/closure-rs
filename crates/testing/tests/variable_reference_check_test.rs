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
//   test/com/google/javascript/jscomp/VariableReferenceCheckTest.java.

//! Port of VariableReferenceCheckTest (a CompilerTestCase) on the Rust CompilerTestCase port: test
//! that warnings are generated in appropriate cases and appropriate cases only by
//! VariableReferenceCheck.
mod var_checks_support;

use closure_jscomp::{
    deps::module_loader::INVALID_MODULE_PATH,
    var_check::VAR_MULTIPLY_DECLARED_ERROR,
    variable_reference_check::{
        DECLARATION_NOT_DIRECTLY_IN_BLOCK, EARLY_EXPORTS_REFERENCE, EARLY_REFERENCE,
        EARLY_REFERENCE_ERROR, REASSIGNED_CONSTANT, REDECLARED_VARIABLE, REDECLARED_VARIABLE_ERROR,
        VariableReferenceCheck,
    },
};
use closure_testing::{compiler_test_case::CompilerTestCase, jscomp_api::SourceFile};
use var_checks_support::{
    Hooks, error_with_message, externs, srcs, srcs_files, srcs_n, test_error, test_error_parts,
    test_same, test_same_parts, test_warning,
};

// port: VariableReferenceCheckTest#LET_RUN
const LET_RUN: &str = "let a = 1; let b = 2; let c = a + b, d = c;";

// port: VariableReferenceCheckTest#VARIABLE_RUN
const VARIABLE_RUN: &str = "var a = 1; var b = 2; var c = a + b, d = c;";

// CompilerTestCase#setUp with VariableReferenceCheckTest#getProcessor
fn set_up() -> (CompilerTestCase, Hooks) {
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    // port: VariableReferenceCheckTest#getProcessor
    // Treats bad reads as errors, and reports bad write warnings.
    let hooks = Hooks::new("VariableReferenceCheckTest", |c| {
        Box::new(VariableReferenceCheck::new(&c.borrow()))
    });
    (harness, hooks)
}

// port: VariableReferenceCheckTest#assertRedeclare
fn assert_redeclare(t: &mut CompilerTestCase, h: &mut Hooks, js: &str) {
    test_warning(t, h, js, &REDECLARED_VARIABLE);
}

// port: VariableReferenceCheckTest#assertRedeclareError
fn assert_redeclare_error(t: &mut CompilerTestCase, h: &mut Hooks, js: &str) {
    test_error(t, h, js, &REDECLARED_VARIABLE_ERROR);
}

// port: VariableReferenceCheckTest#assertReassign
fn assert_reassign(t: &mut CompilerTestCase, h: &mut Hooks, js: &str) {
    test_error(t, h, js, &REASSIGNED_CONSTANT);
}

// port: VariableReferenceCheckTest#assertRedeclareGlobal
fn assert_redeclare_global(t: &mut CompilerTestCase, h: &mut Hooks, js: &str) {
    test_error(t, h, js, &VAR_MULTIPLY_DECLARED_ERROR);
}

/// Expects the JS to generate one bad-write warning.
// port: VariableReferenceCheckTest#assertEarlyReferenceWarning
fn assert_early_reference_warning(t: &mut CompilerTestCase, h: &mut Hooks, js: &str) {
    test_warning(t, h, js, &EARLY_REFERENCE);
}

// port: VariableReferenceCheckTest#assertEarlyReferenceError
fn assert_early_reference_error(t: &mut CompilerTestCase, h: &mut Hooks, js: &str) {
    test_error(t, h, js, &EARLY_REFERENCE_ERROR);
}

/// Expects the JS to generate no errors or warnings.
// port: VariableReferenceCheckTest#assertNoWarning
fn assert_no_warning(t: &mut CompilerTestCase, h: &mut Hooks, js: &str) {
    test_same(t, h, js);
}

#[test]
fn test_with_import_meta() {
    let (mut t, mut h) = set_up();
    // just to confirm that presence of import.meta does not cause a compiler crash
    test_same(
        &mut t,
        &mut h,
        "export function g() { return import.meta; }",
    );
}

#[test]
fn test_double_try_catch() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "function g() {\n  return f;\n\n  function f() {\n    try {\n    } catch (e) {\n      alert(e);\n    }\n    try {\n    } catch (e) {\n      alert(e);\n    }\n  }\n}\n",
    );
}

#[test]
fn test_double_try_catch_with_es6_modules() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "export\nfunction g() {\n  return f;\n\n  function f() {\n    try {\n    } catch (e) {\n      alert(e);\n    }\n    try {\n    } catch (e) {\n      alert(e);\n    }\n  }\n}\n",
    );
}

#[test]
fn test_correct_code() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "function foo(d) { (function() { d.foo(); }); d.bar(); } ",
    );
    assert_no_warning(
        &mut t,
        &mut h,
        "function foo() { bar(); } function bar() { foo(); } ",
    );
    assert_no_warning(&mut t, &mut h, "function f(d) { d = 3; }");
    assert_no_warning(&mut t, &mut h, VARIABLE_RUN);
    assert_no_warning(&mut t, &mut h, "if (a) { var x; }");
    assert_no_warning(
        &mut t,
        &mut h,
        &["function f() { ", VARIABLE_RUN, "}"].concat(),
    );
    assert_no_warning(&mut t, &mut h, LET_RUN);
    assert_no_warning(&mut t, &mut h, &["function f() { ", LET_RUN, "}"].concat());
    assert_no_warning(&mut t, &mut h, "try { let e; } catch (e) { let x; }");
}

#[test]
fn test_correct_code_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "export function foo(d) { (function() { d.foo(); }); d.bar(); } ",
    );
}

#[test]
fn test_correct_shadowing() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        &[VARIABLE_RUN, "function f() { ", VARIABLE_RUN, "}"].concat(),
    );
}

#[test]
fn test_correct_shadowing_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        &[VARIABLE_RUN, "export function f() { ", VARIABLE_RUN, "}"].concat(),
    );
}

#[test]
fn test_correct_redeclare() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "function f() { if (1) { var a = 2; } else { var a = 3; } }",
    );
}

#[test]
fn test_correct_redeclare_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "export function f() { if (1) { var a = 2; } else { var a = 3; } }",
    );
}

#[test]
fn test_correct_recursion() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "function f() { var x = function() { x(); }; }",
    );
}

#[test]
fn test_correct_recursion_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "export function f() { var x = function() { x(); }; }",
    );
}

#[test]
fn test_correct_catch() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "function f() { try { var x = 2; } catch (x) {} }",
    );
    assert_no_warning(
        &mut t,
        &mut h,
        "function f(e) { e = 3; try {} catch (e) {} }",
    );
}

#[test]
fn test_correct_catch_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "export function f() { try { var x = 2; } catch (x) {} }",
    );
}

#[test]
fn test_redeclare() {
    let (mut t, mut h) = set_up();
    // Only test local scope since global scope is covered elsewhere
    assert_redeclare(&mut t, &mut h, "function f() { var a = 2; var a = 3; }");
    assert_redeclare(&mut t, &mut h, "function f(a) { var a = 2; }");
    assert_redeclare(&mut t, &mut h, "function f(a) { if (!a) var a = 6; }");
    // NOTE: We decided to not give warnings to the following cases. The function won't be
    // overwritten at runtime anyway.
    assert_no_warning(&mut t, &mut h, "function f() { var f = 1; }");
    assert_no_warning(&mut t, &mut h, "function f() { let f = 1; }");
}

#[test]
fn test_redeclare_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_redeclare(
        &mut t,
        &mut h,
        "export function f() { var a = 2; var a = 3; }",
    );
    assert_no_warning(&mut t, &mut h, "export function f() { let f = 1; }");
    // In an ES6 module vars are in the module scope, not global, so they are covered here.
    assert_redeclare(&mut t, &mut h, "export var a = 2; var a = 3;");
    assert_redeclare(&mut t, &mut h, "export var a = 2; if (a) var a = 3;");
    assert_redeclare(
        &mut t,
        &mut h,
        "function f() {} function f() {} export {f};",
    );
}

#[test]
fn test_issue166a() {
    let (mut t, mut h) = set_up();
    assert_redeclare_error(
        &mut t,
        &mut h,
        "try { throw 1 } catch(e) { /** @suppress {duplicate} */ var e=2 }",
    );
}

#[test]
fn test_issue166b() {
    let (mut t, mut h) = set_up();
    assert_redeclare_error(
        &mut t,
        &mut h,
        "function a() { try { throw 1 } catch(e) { /** @suppress {duplicate} */ var e=2 } };",
    );
}

#[test]
fn test_issue166b_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_redeclare_error(
        &mut t,
        &mut h,
        "export function a() {\n  try {\n    throw 1\n  } catch (e) {\n      /** @suppress {duplicate} */\n      var e = 2\n  }\n};\n",
    );
}

#[test]
fn test_issue166c() {
    let (mut t, mut h) = set_up();
    assert_redeclare_error(
        &mut t,
        &mut h,
        "var e = 0; try { throw 1 } catch(e) { /** @suppress {duplicate} */ var e=2 }",
    );
}

#[test]
fn test_issue166d() {
    let (mut t, mut h) = set_up();
    assert_redeclare_error(
        &mut t,
        &mut h,
        "function a() {\n  var e = 0; try { throw 1 } catch(e) {\n    /** @suppress {duplicate} */ var e = 2;\n  }\n};\n",
    );
}

#[test]
fn test_issue166e() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "var e = 2; try { throw 1 } catch(e) {}");
}

#[test]
fn test_issue166e_with_es6_modules() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "export var e = 2; try { throw 1 } catch(e) {}",
    );
}

#[test]
fn test_issue166f() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "function a() {\n  var e = 2;\n  try { throw 1 } catch(e) {}\n}\n",
    );
}

#[test]
fn test_early_reference() {
    let (mut t, mut h) = set_up();
    assert_early_reference_warning(&mut t, &mut h, "function f() { a = 2; var a = 3; }");
}

#[test]
fn test_early_reference_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_early_reference_warning(&mut t, &mut h, "export function f() { a = 2; var a = 3; }");
}

#[test]
fn test_correct_early_reference() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "var goog = goog || {}");
    assert_no_warning(
        &mut t,
        &mut h,
        "var google = google || window['google'] || {}",
    );
    assert_no_warning(&mut t, &mut h, "function f() { a = 2; } var a = 2;");
}

#[test]
fn test_correct_early_reference_logical_assignment() {
    let (mut t, mut h) = set_up();
    // These patterns are normalized away
    assert_no_warning(&mut t, &mut h, "function f() { a ||= {}; } let a;");
    assert_no_warning(&mut t, &mut h, "function f() { a &&= {}; } let a;");
    assert_no_warning(&mut t, &mut h, "function f() { a ??= {}; } let a;");
}

#[test]
fn test_correct_early_reference_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "export function f() { a = 2; } var a = 2;");
}

#[test]
fn test_unreferenced_bleeding_function() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "var x = function y() {}");
    assert_no_warning(&mut t, &mut h, "var x = function y() {}; var y = 1;");
}

#[test]
fn test_unreferenced_bleeding_function_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "export var x = function y() {}");
}

#[test]
fn test_referenced_bleeding_function() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "var x = function y() { return y(); }");
}

#[test]
fn test_referenced_bleeding_function_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "export var x = function y() { return y(); }",
    );
}

#[test]
fn test_var_shadows_function_name() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "var x = function y() { var y; }");
    assert_no_warning(&mut t, &mut h, "var x = function y() { let y; }");
}

#[test]
fn test_var_shadows_function_name_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "export var x = function y() { var y; }");
    assert_no_warning(&mut t, &mut h, "export var x = function y() { let y; }");
}

#[test]
fn test_double_declaration() {
    let (mut t, mut h) = set_up();
    assert_redeclare(&mut t, &mut h, "function x(y) { if (true) { var y; } }");
}

#[test]
fn test_double_declaration2() {
    let (mut t, mut h) = set_up();
    assert_redeclare(
        &mut t,
        &mut h,
        "function x() { var y; if (true) { var y; } }",
    );
}

#[test]
fn test_double_declaration_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_redeclare(
        &mut t,
        &mut h,
        "export function x(y) { if (true) { var y; } }",
    );
}

#[test]
fn test_hoisted_function1() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "f(); function f() {}");
}

#[test]
fn test_hoisted_function2() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "function g() { f(); function f() {} }");
}

#[test]
fn test_hoisted_function_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "export function g() { f(); function f() {} }",
    );
}

#[test]
fn test_non_hoisted_function() {
    let (mut t, mut h) = set_up();
    assert_early_reference_warning(&mut t, &mut h, "if (true) { f(); function f() {} }");
}

#[test]
fn test_non_hoisted_function2() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "if (false) { function f() {} f(); }");
}

#[test]
fn test_non_hoisted_function3() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "function g() { if (false) { function f() {} f(); }}",
    );
}

#[test]
fn test_non_hoisted_function4() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "if (false) { function f() {} }  f();");
}

#[test]
fn test_non_hoisted_function5() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "function g() { if (false) { function f() {} }  f(); }",
    );
}

#[test]
fn test_non_hoisted_function6() {
    let (mut t, mut h) = set_up();
    assert_early_reference_warning(&mut t, &mut h, "if (false) { f(); function f() {} }");
}

#[test]
fn test_non_hoisted_function7() {
    let (mut t, mut h) = set_up();
    assert_early_reference_warning(
        &mut t,
        &mut h,
        "function g() { if (false) { f(); function f() {} }}",
    );
}

#[test]
fn test_non_hoisted_function_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_early_reference_warning(
        &mut t,
        &mut h,
        "export function g() { if (false) { f(); function f() {} }}",
    );
}

#[test]
fn test_non_hoisted_recursive_function1() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "if (false) { function f() { f(); }}");
}

#[test]
fn test_non_hoisted_recursive_function2() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "function g() { if (false) { function f() { f(); }}}",
    );
}

#[test]
fn test_non_hoisted_recursive_function3() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "function g() { if (false) { function f() { f(); g(); }}}",
    );
}

#[test]
fn test_non_hoisted_recursive_function_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "export function g() { if (false) { function f() { f(); g(); }}}",
    );
}

#[test]
fn test_for_of() {
    let (mut t, mut h) = set_up();
    assert_early_reference_error(
        &mut t,
        &mut h,
        "for (let x of []) { console.log(x); let x = 123; }",
    );
    assert_no_warning(&mut t, &mut h, "for (let x of []) { let x; }");
}

#[test]
fn test_for_await_of() {
    let (mut t, mut h) = set_up();
    assert_early_reference_error(
        &mut t,
        &mut h,
        "async () => { for await (let x of []) { console.log(x); let x = 123; } }",
    );
    assert_no_warning(
        &mut t,
        &mut h,
        "async () => { for (let x of []) { let x; } }",
    );
}

#[test]
fn test_destructuring_in_for() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "for (let [key, val] of X){}");
    test_same(
        &mut t,
        &mut h,
        "for (let [key, [nestKey, nestVal], val] of X){}",
    );
    test_same(&mut t, &mut h, "var {x: a, y: b} = {x: 1, y: 2}; a++; b++;");
    test_warning(
        &mut t,
        &mut h,
        "a++; var {x: a} = {x: 1};",
        &EARLY_REFERENCE,
    );
}

#[test]
fn test_suppress_duplicate_first() {
    let (mut t, mut h) = set_up();
    let code = "/** @suppress {duplicate} */ var google; var google";
    test_same(&mut t, &mut h, code);
}

#[test]
fn test_suppress_duplicate_second() {
    let (mut t, mut h) = set_up();
    let code = "var google; /** @suppress {duplicate} */ var google";
    test_same(&mut t, &mut h, code);
}

#[test]
fn test_suppress_duplicate_fileoverview() {
    let (mut t, mut h) = set_up();
    let code =
        "/** @fileoverview @suppress {duplicate} */\n/** @type {?} */ var google;\n var google\n";
    test_same(&mut t, &mut h, code);
}

#[test]
fn test_no_warn_duplicate_in_externs2() {
    let (mut t, mut h) = set_up();
    // Verify we don't complain about early references in externs
    let externs_code = "window; var window;";
    let code = "";
    test_same_parts(&mut t, &mut h, vec![externs(externs_code), srcs(code)]);
}

#[test]
fn test_no_warn_duplicate_in_externs_with_es6_modules() {
    let (mut t, mut h) = set_up();
    let externs_code = "export var google; /** @suppress {duplicate} */ var google";
    let code = "";
    test_same_parts(&mut t, &mut h, vec![externs(externs_code), srcs(code)]);
}

#[test]
fn test_import_star() {
    let (mut t, mut h) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![srcs_files(vec![
            SourceFile::from_code("foo.js", ""),
            SourceFile::from_code("bar.js", "import * as ns from './foo.js'"),
        ])],
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
fn test_goog_module_duplicate_require() {
    let (mut t, mut h) = set_up();
    assert_redeclare_error(
        &mut t,
        &mut h,
        "goog.module('bar'); const X = goog.require('foo.X'); const X = goog.require('foo.X');",
    );
    assert_redeclare_error(
        &mut t,
        &mut h,
        "goog.module('bar'); let X = goog.require('foo.X'); let X = goog.require('foo.X');",
    );
    assert_redeclare_error(
        &mut t,
        &mut h,
        "goog.module('bar'); const X = goog.require('foo.X'); let X = goog.require('foo.X');",
    );
    assert_redeclare_error(
        &mut t,
        &mut h,
        "goog.module('bar'); let X = goog.require('foo.X'); const X = goog.require('foo.X');",
    );
}

#[test]
fn test_goog_provide_ok() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "goog.provide('foo');");
    assert_no_warning(&mut t, &mut h, "goog.provide('foo'); foo = 0;");
    assert_no_warning(&mut t, &mut h, "goog.provide('foo'); var foo = 0;");
    assert_no_warning(&mut t, &mut h, "goog.provide('foo.bar');");
    assert_no_warning(&mut t, &mut h, "goog.provide('foo.bar'); foo.bar = 0;");
}

#[test]
fn test_undeclared_let() {
    let (mut t, mut h) = set_up();
    assert_early_reference_error(&mut t, &mut h, "if (a) { x = 3; let x;}");
    assert_early_reference_error(
        &mut t,
        &mut h,
        "var x = 1;\nif (true) {\n  x++;\n  let x = 3;\n}\n",
    );
}

#[test]
fn test_undeclared_let_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_early_reference_error(
        &mut t,
        &mut h,
        "export var x = 1;\nif (true) {\n  x++;\n  let x = 3;\n}\n",
    );
}

#[test]
fn test_undeclared_const() {
    let (mut t, mut h) = set_up();
    assert_early_reference_error(&mut t, &mut h, "if (a) { x = 3; const x = 3;}");
    // For the following, IE 11 gives "Assignment to const", but technically
    // they are also undeclared references, which get caught in the first place.
    assert_early_reference_error(
        &mut t,
        &mut h,
        "var x = 1;\nif (true) {\n  x++;\n  const x = 3;\n}\n",
    );
    assert_early_reference_error(&mut t, &mut h, "a = 1; const a = 0;");
    assert_early_reference_error(&mut t, &mut h, "a++; const a = 0;");
}

#[test]
fn test_illegal_let_shadowing() {
    let (mut t, mut h) = set_up();
    assert_redeclare_error(&mut t, &mut h, "if (a) { let x; var x;}");
    assert_redeclare_error(&mut t, &mut h, "if (a) { let x; let x;}");
    assert_redeclare_error(
        &mut t,
        &mut h,
        "function f() {\n  let x;\n  if (a) {\n    var x;\n  }\n}\n",
    );
    assert_no_warning(
        &mut t,
        &mut h,
        "function f() {\n  if (a) {\n    let x;\n  }\n  var x;\n}\n",
    );
    assert_no_warning(
        &mut t,
        &mut h,
        "function f() {\n  if (a) { let x; }\n  if (b) { var x; }\n}\n",
    );
    assert_redeclare_error(&mut t, &mut h, "let x; var x;");
    assert_redeclare_error(&mut t, &mut h, "var x; let x;");
    assert_redeclare_error(&mut t, &mut h, "let x; let x;");
}

#[test]
fn test_illegal_let_shadowing_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_redeclare_error(
        &mut t,
        &mut h,
        "export function f() {\n  let x;\n  if (a) {\n    var x;\n  }\n}\n",
    );
    assert_no_warning(
        &mut t,
        &mut h,
        "export function f() {\n  if (a) {\n    let x;\n  }\n  var x;\n}\n",
    );
    assert_redeclare_error(&mut t, &mut h, "export let x; var x;");
}

#[test]
fn test_duplicate_let_const() {
    let (mut t, mut h) = set_up();
    assert_redeclare_error(&mut t, &mut h, "let x, x;");
    assert_redeclare_error(&mut t, &mut h, "const x = 0, x = 0;");
}

#[test]
fn test_redeclare_in_label() {
    let (mut t, mut h) = set_up();
    assert_redeclare_global(&mut t, &mut h, "a: var x, x;");
}

#[test]
fn test_illegal_block_scoped_early_reference() {
    let (mut t, mut h) = set_up();
    assert_early_reference_error(&mut t, &mut h, "let x = x");
    assert_early_reference_error(&mut t, &mut h, "let [x] = x");
    assert_early_reference_error(&mut t, &mut h, "const x = x");
    assert_early_reference_error(&mut t, &mut h, "let x = x || 0");
    assert_early_reference_error(&mut t, &mut h, "const x = x || 0");
    // In the following cases, "x" might not be reachable but we warn anyways
    assert_early_reference_error(&mut t, &mut h, "let x = expr || x");
    assert_early_reference_error(&mut t, &mut h, "const x = expr || x");
    assert_early_reference_error(&mut t, &mut h, "X; class X {};");
}

#[test]
fn test_illegal_const_shadowing() {
    let (mut t, mut h) = set_up();
    assert_redeclare_error(&mut t, &mut h, "if (a) { const x = 3; var x;}");
    assert_redeclare_error(
        &mut t,
        &mut h,
        "function f() {\n  const x = 3;\n  if (a) {\n    var x;\n  }\n}\n",
    );
}

#[test]
fn test_illegal_const_shadowing_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_redeclare_error(
        &mut t,
        &mut h,
        "export function f() {\n  const x = 3;\n  if (a) {\n    var x;\n  }\n}\n",
    );
}

#[test]
fn test_var_shadowing() {
    let (mut t, mut h) = set_up();
    assert_redeclare_global(&mut t, &mut h, "if (a) { var x; var x;}");
    assert_redeclare_error(&mut t, &mut h, "if (a) { var x; let x;}");
    assert_redeclare(&mut t, &mut h, "function f() { var x; if (a) { var x; }}");
    assert_redeclare_error(&mut t, &mut h, "function f() { if (a) { var x; } let x;}");
    assert_no_warning(&mut t, &mut h, "function f() { var x; if (a) { let x; }}");
    assert_no_warning(
        &mut t,
        &mut h,
        "function f() {\n  if (a) { var x; }\n  if (b) { let x; }\n}\n",
    );
}

#[test]
fn test_var_shadowing_with_es6_modules01() {
    let (mut t, mut h) = set_up();
    assert_redeclare(
        &mut t,
        &mut h,
        "export function f() { var x; if (a) { var x; }}",
    );
}

#[test]
fn test_var_shadowing_with_es6_modules02() {
    let (mut t, mut h) = set_up();
    assert_redeclare_error(
        &mut t,
        &mut h,
        "export function f() { if (a) { var x; } let x;}",
    );
}

#[test]
fn test_var_shadowing_with_es6_modules03() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "export function f() { var x; if (a) { let x; }}",
    );
}

#[test]
fn test_var_shadowing_with_es6_modules04() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "function f() {\n  if (a) { var x; }\n  if (b) { let x; }\n}\n",
    );
}

#[test]
fn test_parameter_shadowing() {
    let (mut t, mut h) = set_up();
    assert_redeclare_error(&mut t, &mut h, "function f(x) { let x; }");
    assert_redeclare_error(&mut t, &mut h, "function f(x) { const x = 3; }");
    assert_redeclare_error(&mut t, &mut h, "function f(X) { class X {} }");
    assert_redeclare(&mut t, &mut h, "function f(x) { function x() {} }");
    assert_redeclare(&mut t, &mut h, "function f(x) { var x; }");
    assert_redeclare(&mut t, &mut h, "function f(x=3) { var x; }");
    assert_no_warning(&mut t, &mut h, "function f(...x) {}");
    assert_redeclare(&mut t, &mut h, "function f(...x) { var x; }");
    assert_redeclare(&mut t, &mut h, "function f(...x) { function x() {} }");
    assert_redeclare(&mut t, &mut h, "function f(x=3) { function x() {} }");
    assert_no_warning(&mut t, &mut h, "function f(x) { if (true) { let x; } }");
    assert_no_warning(
        &mut t,
        &mut h,
        "function outer(x) {\n  function inner() {\n    let x = 1;\n  }\n}\n",
    );
    assert_no_warning(
        &mut t,
        &mut h,
        "function outer(x) {\n  function inner() {\n    var x = 1;\n  }\n}\n",
    );
    assert_redeclare(&mut t, &mut h, "function f({a, b}) { var a = 2 }");
    assert_redeclare(&mut t, &mut h, "function f({a, b}) { if (!a) var a = 6; }");
}

#[test]
fn test_parameter_shadowing_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_redeclare_error(&mut t, &mut h, "export function f(x) { let x; }");
    assert_redeclare(&mut t, &mut h, "export function f(x) { function x() {} }");
    assert_redeclare(&mut t, &mut h, "export function f(x=3) { var x; }");
    assert_no_warning(&mut t, &mut h, "export function f(...x) {}");
    assert_no_warning(
        &mut t,
        &mut h,
        "export function outer(x) {\n  function inner() {\n    var x = 1;\n  }\n}\n",
    );
}

#[test]
fn test_reassigned_const() {
    let (mut t, mut h) = set_up();
    assert_reassign(&mut t, &mut h, "const a = 0; a = 1;");
    assert_reassign(&mut t, &mut h, "const a = 0; a++;");
}

#[test]
fn test_logical_reassigned_const() {
    let (mut t, mut h) = set_up();
    // These patterns are normalized away
    assert_reassign(&mut t, &mut h, "const a = 0; a ||= 1;");
    assert_reassign(&mut t, &mut h, "const a = 1; a &&= 1;");
    assert_reassign(&mut t, &mut h, "const a = null; a ??= 1;");
}

#[test]
fn test_let_const_not_directly_in_block() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "if (true) var x = 3;");
    test_error(
        &mut t,
        &mut h,
        "if (true) let x = 3;",
        &DECLARATION_NOT_DIRECTLY_IN_BLOCK,
    );
    test_error(
        &mut t,
        &mut h,
        "if (true) const x = 3;",
        &DECLARATION_NOT_DIRECTLY_IN_BLOCK,
    );
    test_error(
        &mut t,
        &mut h,
        "if (true) class C {}",
        &DECLARATION_NOT_DIRECTLY_IN_BLOCK,
    );
    test_error(
        &mut t,
        &mut h,
        "if (true) function f() {}",
        &DECLARATION_NOT_DIRECTLY_IN_BLOCK,
    );
}

#[test]
fn test_function_hoisting() {
    let (mut t, mut h) = set_up();
    assert_early_reference_warning(&mut t, &mut h, "if (true) { f(); function f() {} }");
}

#[test]
fn test_function_hoisting_redeclaration1() {
    let (mut t, mut h) = set_up();
    let js: &[&str] = &["var x;", "function x() {}"];
    let message = "Variable x declared more than once. First occurrence: testcode0:1:4";
    test_error_parts(
        &mut t,
        &mut h,
        vec![
            srcs_n(js),
            error_with_message(&VAR_MULTIPLY_DECLARED_ERROR, message),
        ],
    );
}

#[test]
fn test_function_hoisting_redeclaration2() {
    let (mut t, mut h) = set_up();
    let js: &[&str] = &["function x() {}", "var x;"];
    let message = "Variable x declared more than once. First occurrence: testcode0:1:9";
    test_error_parts(
        &mut t,
        &mut h,
        vec![
            srcs_n(js),
            error_with_message(&VAR_MULTIPLY_DECLARED_ERROR, message),
        ],
    );
}

#[test]
fn test_arrow_function() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "var f = x => { return x+1; };");
    assert_no_warning(
        &mut t,
        &mut h,
        "var odds = [1,2,3,4].filter((n) => n%2 == 1)",
    );
    assert_redeclare(&mut t, &mut h, "var f = x => {var x;}");
    assert_redeclare_error(&mut t, &mut h, "var f = x => {let x;}");
}

#[test]
fn test_arrow_function_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "export var f = x => { return x+1; };");
    assert_redeclare(&mut t, &mut h, "export var f = x => {var x;}");
    assert_redeclare_error(&mut t, &mut h, "export var f = x => {let x;}");
}

#[test]
fn test_try_catch() {
    let (mut t, mut h) = set_up();
    assert_redeclare_error(
        &mut t,
        &mut h,
        "function f() {\n  try {\n    let e = 0;\n    if (true) {\n      let e = 1;\n    }\n  } catch (e) {\n    let e;\n  }\n}\n",
    );
    assert_redeclare_error(
        &mut t,
        &mut h,
        "function f() {\n  try {\n    let e = 0;\n    if (true) {\n      let e = 1;\n    }\n  } catch (e) {\n      var e;\n  }\n}\n",
    );
    assert_redeclare_error(
        &mut t,
        &mut h,
        "function f() {\n  try {\n    let e = 0;\n    if (true) {\n      let e = 1;\n    }\n  } catch (e) {\n    function e() {\n      var e;\n    }\n  }\n}\n",
    );
}

#[test]
fn test_try_catch_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_redeclare_error(
        &mut t,
        &mut h,
        "export function f() {\n  try {\n    let e = 0;\n    if (true) {\n      let e = 1;\n    }\n  } catch (e) {\n    let e;\n  }\n}\n",
    );
}

#[test]
fn test_class() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "class A { f() { return 1729; } }");
}

#[test]
fn test_class_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "export class A { f() { return 1729; } }");
}

#[test]
fn test_redeclare_class_name() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "var Clazz = class Foo {}; var Foo = 3;");
}

#[test]
fn test_redeclare_class_name_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "export var Clazz = class Foo {}; var Foo = 3;",
    );
}

#[test]
fn test_class_extend() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "class A {} class C extends A {} C = class extends A {}",
    );
}

#[test]
fn test_class_extend_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "export class A {} class C extends A {} C = class extends A {}",
    );
}

/// Variable reference before declaration error should not appear for non-static public fields
#[test]
fn test_non_static_public_fields() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "class Foo { x = bar;} let bar = 1;");
    assert_no_warning(
        &mut t,
        &mut h,
        "class Foo { x = Enum.X; } const Enum = { X: 1 }",
    );
    assert_no_warning(&mut t, &mut h, "class Foo { x = new Bar(); } class Bar {}");
}

#[test]
fn test_static_public_fields() {
    let (mut t, mut h) = set_up();
    assert_early_reference_error(&mut t, &mut h, "class Bar { static x = y; } const y = 3;");
    assert_early_reference_error(
        &mut t,
        &mut h,
        "class Foo { static x = new Bar(); } class Bar {}",
    );
    assert_early_reference_error(
        &mut t,
        &mut h,
        "class Bar { static x = Enum.A; } let Enum = { A: 'str' }",
    );
}

#[test]
fn test_array_pattern() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "var [a] = [1];");
    assert_no_warning(&mut t, &mut h, "var [a, b] = [1, 2];");
    assert_early_reference_warning(&mut t, &mut h, "alert(a); var [a] = [1];");
    assert_early_reference_warning(&mut t, &mut h, "alert(b); var [a, b] = [1, 2];");
    assert_early_reference_warning(&mut t, &mut h, "[a] = [1]; var a;");
    assert_early_reference_warning(&mut t, &mut h, "[a, b] = [1]; var b;");
}

#[test]
fn test_array_pattern_with_es6_modules01() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "export var [a] = [1];");
}

#[test]
fn test_array_pattern_default_value() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "var [a = 1] = [2];");
    assert_no_warning(&mut t, &mut h, "var [a = 1] = [];");
    assert_early_reference_warning(&mut t, &mut h, "alert(a); var [a = 1] = [2];");
    assert_early_reference_warning(&mut t, &mut h, "alert(a); var [a = 1] = [];");
    assert_early_reference_warning(&mut t, &mut h, "alert(a); var [a = b] = [1];");
    assert_early_reference_warning(&mut t, &mut h, "alert(a); var [a = b] = [];");
}

#[test]
fn test_array_pattern_default_value_with_es6_modules01() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "export var [a = 1] = [2];");
}

#[test]
fn test_object_pattern() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "var {a: b} = {a: 1};");
    assert_no_warning(&mut t, &mut h, "var {a: b} = {};");
    assert_no_warning(&mut t, &mut h, "var {a} = {a: 1};");
    // 'a' is not declared at all, so the 'a' passed to alert() references
    // the global variable 'a', and there is no warning.
    assert_no_warning(&mut t, &mut h, "alert(a); var {a: b} = {};");
    assert_no_warning(&mut t, &mut h, "alert(a); var {a: {a: b}} = {};");
    assert_early_reference_warning(&mut t, &mut h, "alert(b); var {a: b} = {a: 1};");
    assert_early_reference_warning(&mut t, &mut h, "alert(a); var {a} = {a: 1};");
    assert_early_reference_warning(&mut t, &mut h, "({a: b} = {}); var a, b;");
}

#[test]
fn test_object_pattern_rest() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "var {a: b, ...r} = {a: 1};");
    assert_no_warning(&mut t, &mut h, "var {a: b, ...r} = {};");
    assert_no_warning(&mut t, &mut h, "var {a, ...r} = {a: 1};");
    assert_no_warning(&mut t, &mut h, "alert(r);");
    assert_early_reference_warning(&mut t, &mut h, "alert(r); var {...r} = {a: 1};");
    assert_no_warning(&mut t, &mut h, "({...a} = {});");
    assert_early_reference_warning(&mut t, &mut h, "({...a} = {}); var a;");
}

#[test]
fn test_object_pattern_with_es6_modules01() {
    let (mut t, mut h) = set_up();
    assert_no_warning(&mut t, &mut h, "export var {a: b} = {a: 1};");
}

#[test]
fn test_object_pattern_default_value() {
    let (mut t, mut h) = set_up();
    assert_early_reference_warning(&mut t, &mut h, "alert(b); var {a: b = c} = {a: 1};");
    assert_early_reference_warning(&mut t, &mut h, "alert(b); var c; var {a: b = c} = {a: 1};");
    assert_early_reference_warning(&mut t, &mut h, "var {a: b = c} = {a: 1}; var c;");
    assert_early_reference_warning(&mut t, &mut h, "alert(b); var {a: b = c} = {};");
    assert_early_reference_warning(&mut t, &mut h, "alert(a); var {a = c} = {a: 1};");
    assert_early_reference_warning(&mut t, &mut h, "alert(a); var {a = c} = {};");
}

#[test]
fn test_object_pattern_default_value_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_early_reference_warning(&mut t, &mut h, "export var {a: b = c} = {a: 1}; var c;");
}

/// We can't catch all possible runtime errors but it's useful to have some basic checks.
#[test]
fn test_default_param() {
    let (mut t, mut h) = set_up();
    assert_early_reference_error(&mut t, &mut h, "function f(x=a) { let a; }");
    assert_early_reference_error(
        &mut t,
        &mut h,
        "function f(x=a) { let a; }\nfunction g(x=1) { var a; }\n",
    );
    assert_early_reference_error(&mut t, &mut h, "function f(x=a) { var a; }");
    assert_early_reference_error(&mut t, &mut h, "function f(x=a()) { function a() {} }");
    assert_early_reference_error(&mut t, &mut h, "function f(x=[a]) { var a; }");
    assert_early_reference_error(&mut t, &mut h, "function f(x={a}) { let a; }");
    assert_early_reference_error(&mut t, &mut h, "function f(x=y, y=2) {}");
    assert_early_reference_error(&mut t, &mut h, "function f(x={y}, y=2) {}");
    assert_early_reference_error(&mut t, &mut h, "function f(x=x) {}");
    assert_early_reference_error(&mut t, &mut h, "function f([x]=x) {}");
    // x within a function isn't referenced at the time the default value for x is evaluated.
    assert_no_warning(&mut t, &mut h, "function f(x=()=>x) {}");
    assert_no_warning(&mut t, &mut h, "function f(x=a) {}");
    assert_no_warning(&mut t, &mut h, "function f(x=a) {} var a;");
    assert_no_warning(&mut t, &mut h, "let b; function f(x=b) { var b; }");
    assert_no_warning(
        &mut t,
        &mut h,
        "function f(y = () => x, x = 5) { return y(); }",
    );
    assert_no_warning(&mut t, &mut h, "function f(x = new foo.bar()) {}");
    assert_no_warning(
        &mut t,
        &mut h,
        "var foo = {}; foo.bar = class {}; function f(x = new foo.bar()) {}",
    );
}

#[test]
fn test_default_param_with_es6_modules() {
    let (mut t, mut h) = set_up();
    assert_early_reference_error(&mut t, &mut h, "export function f(x=a) { let a; }");
    assert_no_warning(&mut t, &mut h, "export function f(x=()=>x) {}");
}

#[test]
fn test_destructuring() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "function f() {\n  var obj = {a:1, b:2};\n  var {a:c, b:d} = obj;\n}\n",
    );
    test_same(
        &mut t,
        &mut h,
        "function f() {\n  var obj = {a:1, b:2};\n  var {a, b} = obj;\n}\n",
    );
    assert_redeclare(
        &mut t,
        &mut h,
        "function f() {\n  var obj = {a:1, b:2};\n  var {a:c, b:d} = obj;\n  var c = b;\n}\n",
    );
    assert_early_reference_warning(
        &mut t,
        &mut h,
        "function f() {\n  var {a:c, b:d} = obj;\n  var obj = {a:1, b:2};\n}\n",
    );
    assert_early_reference_warning(
        &mut t,
        &mut h,
        "function f() {\n  var {a, b} = obj;\n  var obj = {a:1, b:2};\n}\n",
    );
    assert_early_reference_warning(
        &mut t,
        &mut h,
        "function f() {\n  var e = c;\n  var {a:c, b:d} = {a:1, b:2};\n}\n",
    );
}

#[test]
fn test_destructuring_with_es6_modules() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "export function f() {\n  var obj = {a:1, b:2};\n  var {a:c, b:d} = obj;\n}\n",
    );
    assert_redeclare(
        &mut t,
        &mut h,
        "export function f() {\n  var obj = {a:1, b:2};\n  var {a:c, b:d} = obj;\n  var c = b;\n}\n",
    );
    assert_early_reference_warning(
        &mut t,
        &mut h,
        "export function f() {\n  var {a:c, b:d} = obj;\n  var obj = {a:1, b:2};\n}\n",
    );
}

#[test]
fn test_destructuring_in_loop() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "for (let {length: x} in obj) {}");
    test_same(&mut t, &mut h, "for (let [{length: z}, w] in obj) {}");
}

#[test]
fn test_referencing_previously_declared_variable_in_const() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "const [a, b = a] = [];");
    // TODO(b/111441110): don't error on this. it's valid code.
    assert_early_reference_error(&mut t, &mut h, "for (const [a, b = a] of []);");
}

#[test]
fn test_early_reference_in_inner_block() {
    let (mut t, mut h) = set_up();
    assert_early_reference_error(&mut t, &mut h, "for (x of [1, 2, 3]) {} let x;");
    assert_early_reference_error(&mut t, &mut h, "{ x; } let x;");
    assert_early_reference_error(&mut t, &mut h, "{ C; } class C {}");
    assert_early_reference_warning(&mut t, &mut h, "{ x; }  var x;");
}

#[test]
fn test_early_variable_reference_inside_function() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "function f() { x; } let x; f(); ");
    test_same(
        &mut t,
        &mut h,
        "function f() { const f = () => x; let x = 3; return f; }",
    );
    // NOTE: this will cause an error at runtime, but we don't report it because we don't track
    // where `f` is being called.
    test_same(&mut t, &mut h, "function f() { x; } f(); let x;");
    test_same(&mut t, &mut h, "function f() { x; } f(); var x;");
}

#[test]
fn test_enhanced_for_loop_temporal_dead_zone() {
    let (mut t, mut h) = set_up();
    assert_early_reference_error(&mut t, &mut h, "for (let x of [x]);");
    assert_early_reference_error(&mut t, &mut h, "for (let x in [x]);");
    assert_early_reference_error(&mut t, &mut h, "for (const x of [x]);");
    test_same(&mut t, &mut h, "for (var x of [x]);");
    test_same(&mut t, &mut h, "for (let x of [() => x]);");
    test_same(&mut t, &mut h, "let x = 1; for (let y of [x]);");
}

#[test]
fn test_enhanced_for_loop_temporal_dead_zone_with_es6_modules() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "export let x = 1; for (let y of [x]);");
}

#[test]
fn test_redeclare_variable_from_import() {
    let (mut t, mut h) = set_up();
    t.ignore_warnings(&[&INVALID_MODULE_PATH]).unwrap();
    assert_redeclare_error(&mut t, &mut h, "import {x} from 'whatever'; let x = 0;");
    assert_redeclare_error(&mut t, &mut h, "import {x} from 'whatever'; const x = 0;");
    assert_redeclare_error(&mut t, &mut h, "import {x} from 'whatever'; var x = 0;");
    assert_redeclare_error(
        &mut t,
        &mut h,
        "import {x} from 'whatever'; function x() {}",
    );
    assert_redeclare_error(&mut t, &mut h, "import {x} from 'whatever'; class x {}");
    assert_redeclare_error(&mut t, &mut h, "import x from 'whatever'; let x = 0;");
    assert_redeclare_error(
        &mut t,
        &mut h,
        "import * as ns from 'whatever'; let ns = 0;",
    );
    assert_redeclare_error(
        &mut t,
        &mut h,
        "import {y as x} from 'whatever'; let x = 0;",
    );
    assert_redeclare_error(&mut t, &mut h, "import {x} from 'whatever'; let {x} = {};");
    assert_redeclare_error(&mut t, &mut h, "import {x} from 'whatever'; let [x] = [];");
    assert_redeclare_error(&mut t, &mut h, "import {x, x} from 'whatever';");
    assert_redeclare_error(&mut t, &mut h, "import {x, y as x} from 'whatever';");
    assert_redeclare_error(&mut t, &mut h, "import {z as x, y as x} from 'whatever';");
    assert_redeclare_error(
        &mut t,
        &mut h,
        "import {x} from 'first'; import {x} from 'second';",
    );
    assert_redeclare_error(
        &mut t,
        &mut h,
        "import {x} from 'first'; import {a as x} from 'second';",
    );
    assert_redeclare_error(
        &mut t,
        &mut h,
        "import {b as x} from 'first'; import {a as x} from 'second';",
    );
    test_same(
        &mut t,
        &mut h,
        "import {x} from 'whatever'; function f() { let x = 0; }",
    );
    test_same(
        &mut t,
        &mut h,
        "import {x as x} from 'whatever'; function f() { let x = 0; }",
    );
    test_same(
        &mut t,
        &mut h,
        "import {y as x} from 'whatever'; function f() { let x = 0; }",
    );
}

#[test]
fn test_ok_exports_ref_in_goog_module() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "goog.module('m');");
    test_same(
        &mut t,
        &mut h,
        "goog.module('m'); exports.Foo = 0; exports.Bar = 0;",
    );
    test_same(&mut t, &mut h, "goog.module('m'); exports = 0;");
    test_same(
        &mut t,
        &mut h,
        "goog.module('m'); exports = class {}; exports.Foo = class {};",
    );
    test_same(
        &mut t,
        &mut h,
        "goog.module('m'); function f() { exports = 0; }",
    );
    // Bad style but warn elsewhere
    test_same(
        &mut t,
        &mut h,
        "goog.module('m'); function f() { return exports; } exports = 1;",
    );
}

#[test]
fn test_bad_early_exports_ref_in_goog_module() {
    let (mut t, mut h) = set_up();
    test_error(
        &mut t,
        &mut h,
        "goog.module('m'); exports.x = 0; exports = {};",
        &EARLY_EXPORTS_REFERENCE,
    );
    test_error(
        &mut t,
        &mut h,
        "goog.module('m'); exports.x = 0; exports = class Bar {};",
        &EARLY_EXPORTS_REFERENCE,
    );
    test_error(
        &mut t,
        &mut h,
        "goog.module('m'); /** @typedef {string} */ exports.x; exports = {};",
        &EARLY_EXPORTS_REFERENCE,
    );
}

#[test]
fn test_reference_in_switch_condition_shadowed_in_switch_body() {
    let (mut t, mut h) = set_up();
    assert_no_warning(
        &mut t,
        &mut h,
        "const x = 0;\nswitch (x) {\n  case 0:\n  let x;\n  break;\n  default: break;\n}\n",
    );
}

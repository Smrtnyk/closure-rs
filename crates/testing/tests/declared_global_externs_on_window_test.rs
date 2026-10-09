/*
 * Copyright 2014 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/DeclaredGlobalExternsOnWindowTest.java.

//! Port of DeclaredGlobalExternsOnWindowTest (a CompilerTestCase) on the Rust CompilerTestCase
//! port.
mod var_checks_support;

use closure_jscomp::{
    declared_global_externs_on_window::DeclaredGlobalExternsOnWindow,
    type_validator::TYPE_MISMATCH_WARNING,
};
use closure_testing::compiler_test_case::CompilerTestCase;
use var_checks_support::{
    Hooks, expected, externs, minimal_externs, srcs, test_extern_changes_parts, test_same_parts,
    warning,
};

// port: DeclaredGlobalExternsOnWindowTest#setUp (with #getProcessor)
fn set_up() -> (CompilerTestCase, Hooks) {
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    harness.enable_type_check().unwrap();
    harness.allow_externs_changes().unwrap();
    harness.enable_run_type_check_after_processing().unwrap();
    // port: DeclaredGlobalExternsOnWindowTest#getProcessor
    let hooks = Hooks::new("DeclaredGlobalExternsOnWindowTest", |c| {
        Box::new(DeclaredGlobalExternsOnWindow::new(&c.borrow()))
    });
    (harness, hooks)
}

// CompilerTestCase#testExternChanges(Externs,Sources,Expected) as the Java tests call it
fn test_extern_changes(
    t: &mut CompilerTestCase,
    h: &mut Hooks,
    externs_code: &str,
    srcs_code: &str,
    expected_code: &str,
) {
    test_extern_changes_parts(
        t,
        h,
        vec![
            externs(externs_code),
            srcs(srcs_code),
            expected(expected_code),
        ],
    );
}

#[test]
fn test_window_property1a() {
    let (mut t, mut h) = set_up();
    test_extern_changes(
        &mut t,
        &mut h,
        "var window; var a;",
        "",
        "var window;var a;window.a",
    );
}

// No "var window;" so use "this" instead.
#[test]
fn test_window_property1b() {
    let (mut t, mut h) = set_up();
    test_extern_changes(&mut t, &mut h, "var a;", "", "var a;this.a");
}

#[test]
fn test_window_property2() {
    let (mut t, mut h) = set_up();
    test_extern_changes(&mut t, &mut h, "", "var a", "");
}

#[test]
fn test_window_property3a() {
    let (mut t, mut h) = set_up();
    test_extern_changes(
        &mut t,
        &mut h,
        "var window; function f() {}",
        "var b",
        "var window;function f(){}window.f;",
    );
}

// No "var window;" so use "this" instead.
#[test]
fn test_window_property3b() {
    let (mut t, mut h) = set_up();
    test_extern_changes(
        &mut t,
        &mut h,
        "function f() {}",
        "var b",
        "function f(){}this.f",
    );
}

#[test]
fn test_window_property4() {
    let (mut t, mut h) = set_up();
    test_extern_changes(&mut t, &mut h, "", "function f() {}", "");
}

#[test]
fn test_window_property5a() {
    let (mut t, mut h) = set_up();
    test_extern_changes(
        &mut t,
        &mut h,
        "var window; var x = function f() {}",
        "var b",
        "var window;var x=function f(){};window.x;",
    );
    test_extern_changes(
        &mut t,
        &mut h,
        "var window; var x = function () {}",
        "var b",
        "var window;var x=function(){};window.x;",
    );
}

// No "var window;" so use "this" instead.
#[test]
fn test_window_property5b() {
    let (mut t, mut h) = set_up();
    test_extern_changes(
        &mut t,
        &mut h,
        "var x = function f() {};",
        "var b",
        "var x=function f(){};this.x",
    );
}

#[test]
fn test_window_property5c() {
    let (mut t, mut h) = set_up();
    test_extern_changes(
        &mut t,
        &mut h,
        "var window; var x = ()=>{}",
        "var b",
        "var window;var x=()=>{};window.x;",
    );
}

#[test]
fn test_window_property6() {
    let (mut t, mut h) = set_up();
    test_extern_changes(
        &mut t,
        &mut h,
        "var window; /** @const {number} */ var n;",
        "",
        "var window;\n\
         /** @const {number} */ var n;\n\
         /** @const {number} @suppress {const,duplicate} */ window.n;\n",
    );
}

#[test]
fn test_window_property7() {
    let (mut t, mut h) = set_up();
    test_extern_changes(
        &mut t,
        &mut h,
        "var window; /** @const */ var ns = {}",
        "",
        "var window;\n\
         /** @const */ var ns = {};\n\
         /** @suppress {const,duplicate} @const */ window.ns = ns;\n",
    );
}

#[test]
fn test_name_aliasing() {
    let (mut t, mut h) = set_up();
    test_extern_changes(
        &mut t,
        &mut h,
        "var window;\n\
         /** @const */\n\
         var ns = {};\n\
         /** @const */\n\
         var ns2 = ns;\n",
        "",
        "var window;\n\
         /** @const */\n\
         var ns = {};\n\
         /** @const */\n\
         var ns2 = ns;\n\
         /** @suppress {const,duplicate} @const */\n\
         window.ns = ns;\n\
         /** @suppress {const,duplicate} @const */\n\
         window.ns2 = ns;\n",
    );
}

#[test]
fn test_qualified_name_aliasing() {
    let (mut t, mut h) = set_up();
    test_extern_changes(
        &mut t,
        &mut h,
        "var window;\n\
         /** @const */\n\
         var ns = {};\n\
         /** @type {number} A very important constant */\n\
         ns.THE_NUMBER;\n\
         /** @const */\n\
         var num = ns.THE_NUMBER;\n",
        "",
        "var window;\n\
         /** @const */\n\
         var ns = {};\n\
         /** @type {number} A very important constant */\n\
         ns.THE_NUMBER;\n\
         /** @const */\n\
         var num = ns.THE_NUMBER;\n\
         /** @suppress {const,duplicate} @const */\n\
         window.ns=ns;\n\
         /** @suppress {const,duplicate} @const */\n\
         window.num = ns.THE_NUMBER;\n",
    );
}

#[test]
fn test_window_property8() {
    let (mut t, mut h) = set_up();
    test_extern_changes(
        &mut t,
        &mut h,
        "var window; /** @constructor */ function Foo() {}",
        "",
        "var window;\n\
         /** @constructor */ function Foo(){}\n\
         /** @constructor @suppress {const,duplicate} */ window.Foo = Foo;\n",
    );
}

#[test]
fn test_enum_window_property() {
    let (mut t, mut h) = set_up();
    test_extern_changes(
        &mut t,
        &mut h,
        "var window; /** @enum {string} */ var Enum = { A: 'str' };",
        "",
        "var window;\n\
         /** @enum {string} */ var Enum = { A: 'str' };\n\
         /** @enum {string} @suppress {const,duplicate} */ window.Enum = Enum;\n",
    );
}

/// Test to make sure the compiler knows the type of "window.x" is the same as that of "x".
#[test]
fn test_window_property_with_js_doc() {
    let (mut t, mut h) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs(&format!(
                "{}var window;\n/** @type {{string}} */ var x;\n",
                minimal_externs()
            )),
            srcs("/** @param {number} n*/\nfunction f(n) {}\nf(window.x);\n"),
            warning(&TYPE_MISMATCH_WARNING),
        ],
    );
}

#[test]
fn test_enum() {
    let (mut t, mut h) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs(&format!(
                "{}/** @enum {{string}} */ var Enum = {{FOO: 'foo', BAR: 'bar'}};",
                minimal_externs()
            )),
            srcs("/** @param {Enum} e*/\nfunction f(e) {}\nf(window.Enum.FOO);\n"),
        ],
    );
}

/// Test to make sure that if Foo is a constructor, Foo is considered to be the same type as
/// window.Foo.
#[test]
fn test_constructor_is_same_type() {
    let (mut t, mut h) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs(&format!(
                "{}var window;\n/** @constructor */ function Foo() {{}}\n",
                minimal_externs()
            )),
            srcs("/** @param {!window.Foo} f*/\nfunction bar(f) {}\nbar(new Foo());\n"),
        ],
    );

    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs(&format!(
                "{}/** @constructor */ function Foo() {{}}",
                minimal_externs()
            )),
            srcs("/** @param {!Foo} f*/\nfunction bar(f) {}\nbar(new window.Foo());\n"),
        ],
    );
}

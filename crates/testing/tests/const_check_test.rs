/*
 * Copyright 2007 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/ConstCheckTest.java.

//! Port of ConstCheckTest (a CompilerTestCase) on the Rust CompilerTestCase port.
mod var_checks_support;

use closure_jscomp::{
    compiler_pass::CompilerPass,
    const_check::{CONST_REASSIGNED_VALUE_ERROR, ConstCheck},
    infer_consts::InferConsts,
};
use closure_testing::compiler_test_case::CompilerTestCase;
use var_checks_support::{
    Hooks, externs, srcs, srcs_n, test_no_warning, test_same, test_same_parts, test_warning_parts,
    warning,
};

/// ConstCheckTest#getProcessor's lambda.
struct Processor;

impl CompilerPass for Processor {
    // port: ConstCheckTest#getProcessor (the lambda's process)
    fn process(
        &mut self,
        compiler: &mut closure_jscomp::abstract_compiler::AbstractCompiler,
        externs: closure_rhino::node::NodeId,
        root: closure_rhino::node::NodeId,
    ) {
        InferConsts::new(compiler).process(compiler, externs, root);
        ConstCheck::new(compiler).process(compiler, externs, root);
    }
}

// port: ConstCheckTest#setUp (with #getProcessor)
fn set_up() -> (CompilerTestCase, Hooks) {
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    harness.enable_create_module_map().unwrap();
    // port: ConstCheckTest#getProcessor
    let hooks = Hooks::new("ConstCheckTest", |_| Box::new(Processor));
    (harness, hooks)
}

// port: ConstCheckTest#testWarning
fn test_warning(t: &mut CompilerTestCase, h: &mut Hooks, js: &str) {
    var_checks_support::test_warning(t, h, js, &CONST_REASSIGNED_VALUE_ERROR);
}

#[test]
fn test_constant_definition1() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "/** @const */ var XYZ = 1;");
}

#[test]
fn test_constant_definition2() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "/** @const */ var a$b$XYZ = 1;");
}

#[test]
fn test_constant_definition3() {
    let (mut t, mut h) = set_up();
    test_no_warning(&mut t, &mut h, "const xyz=1;");
}

#[test]
fn test_constant_definition4() {
    let (mut t, mut h) = set_up();
    println!("HELLO");
    test_no_warning(&mut t, &mut h, "const a$b$xyz = 1;");
}

#[test]
fn test_constant_initialized_in_anonymous_namespace1() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "/** @const */ var XYZ; (function(){ XYZ = 1; })();",
    );
}

#[test]
fn test_constant_initialized_in_anonymous_namespace2() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "/** @const */ var a$b$XYZ; (function(){ a$b$XYZ = 1; })();",
    );
}

#[test]
fn test_object_modified() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "/** @const */ var IE = true, XYZ = {a:1,b:1}; if (IE) XYZ['c'] = 1;",
    );
}

#[test]
fn test_object_property_initialized_late() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "/** @const */ var XYZ = {}; for (var i = 0; i < 10; i++) { XYZ[i] = i; }",
    );
}

#[test]
fn test_object_redefined1() {
    let (mut t, mut h) = set_up();
    test_error(&mut t, &mut h, "/** @const */ var XYZ = {}; XYZ = 2;");
}

#[test]
fn test_object_redefined2() {
    let (mut t, mut h) = set_up();
    test_error(&mut t, &mut h, "const xyz = {}; xyz = 2;");
}

#[test]
fn test_constant_redefined1() {
    let (mut t, mut h) = set_up();
    test_error(&mut t, &mut h, "/** @const */ var XYZ = 1; XYZ = 2;");
}

#[test]
fn test_constant_redefined2() {
    let (mut t, mut h) = set_up();
    test_error(
        &mut t,
        &mut h,
        "/** @const */ var a$b$XYZ = 1; a$b$XYZ = 2;",
    );
}

// test will be caught be earlier pass, but demonstrates that it returns error upon const
// reassigning
#[test]
fn test_constant_redefined3() {
    let (mut t, mut h) = set_up();
    test_warning(&mut t, &mut h, "const xyz = 1; xyz = 2;");
}

#[test]
fn test_constant_redefined4() {
    let (mut t, mut h) = set_up();
    test_error(&mut t, &mut h, "/** @const */ let XYZ = 1; XYZ = 2;");
}

#[test]
fn test_constant_redefined_in_local_scope1() {
    let (mut t, mut h) = set_up();
    test_error(
        &mut t,
        &mut h,
        "/** @const */ var XYZ = 1; (function(){ XYZ = 2; })();",
    );
}

#[test]
fn test_constant_redefined_in_local_scope2() {
    let (mut t, mut h) = set_up();
    test_error(
        &mut t,
        &mut h,
        "/** @const */ var a$b$XYZ = 1; (function(){ a$b$XYZ = 2; })();",
    );
}

#[test]
fn test_constant_redefined_in_local_scope_out_of_order() {
    let (mut t, mut h) = set_up();
    test_error(
        &mut t,
        &mut h,
        "function f() { XYZ = 2; } /** @const */ var XYZ = 1;",
    );
}

#[test]
fn test_constant_post_incremented1() {
    let (mut t, mut h) = set_up();
    test_error(&mut t, &mut h, "/** @const */ var XYZ = 1; XYZ++;");
}

#[test]
fn test_constant_post_incremented2() {
    let (mut t, mut h) = set_up();
    test_error(&mut t, &mut h, "/** @const */ var a$b$XYZ = 1; a$b$XYZ++;");
}

#[test]
fn test_constant_pre_incremented1() {
    let (mut t, mut h) = set_up();
    test_error(&mut t, &mut h, "/** @const */ var XYZ = 1; ++XYZ;");
}

#[test]
fn test_constant_pre_incremented2() {
    let (mut t, mut h) = set_up();
    test_error(&mut t, &mut h, "/** @const */ var a$b$XYZ = 1; ++a$b$XYZ;");
}

#[test]
fn test_constant_post_decremented1() {
    let (mut t, mut h) = set_up();
    test_error(&mut t, &mut h, "/** @const */ var XYZ = 1; XYZ--;");
}

#[test]
fn test_constant_post_decremented2() {
    let (mut t, mut h) = set_up();
    test_error(&mut t, &mut h, "/** @const */ var a$b$XYZ = 1; a$b$XYZ--;");
}

#[test]
fn test_constant_pre_decremented1() {
    let (mut t, mut h) = set_up();
    test_error(&mut t, &mut h, "/** @const */ var XYZ = 1; --XYZ;");
}

#[test]
fn test_constant_pre_decremented2() {
    let (mut t, mut h) = set_up();
    test_error(&mut t, &mut h, "/** @const */ var a$b$XYZ = 1; --a$b$XYZ;");
}

#[test]
fn test_constant_pre_decremented3() {
    let (mut t, mut h) = set_up();
    test_warning(&mut t, &mut h, "const xyz = 1; --xyz;");
}

#[test]
fn test_abbreviated_arithmetic_assignment1() {
    let (mut t, mut h) = set_up();
    test_error(&mut t, &mut h, "/** @const */ var XYZ = 1; XYZ += 2;");
}

#[test]
fn test_abbreviated_arithmetic_assignment2() {
    let (mut t, mut h) = set_up();
    test_error(
        &mut t,
        &mut h,
        "/** @const */ var a$b$XYZ = 1; a$b$XYZ %= 2;",
    );
}

#[test]
fn test_abbreviated_arithmetic_assignment3() {
    let (mut t, mut h) = set_up();
    test_error(
        &mut t,
        &mut h,
        "/** @const */ var a$b$XYZ = 1; a$b$XYZ **= 2;",
    );
}

#[test]
fn test_abbreviated_bit_assignment1() {
    let (mut t, mut h) = set_up();
    test_error(&mut t, &mut h, "/** @const */ var XYZ = 1; XYZ |= 2;");
}

#[test]
fn test_abbreviated_bit_assignment2() {
    let (mut t, mut h) = set_up();
    test_error(
        &mut t,
        &mut h,
        "/** @const */ var a$b$XYZ = 1; a$b$XYZ &= 2;",
    );
}

#[test]
fn test_abbreviated_shift_assignment1() {
    let (mut t, mut h) = set_up();
    test_error(&mut t, &mut h, "/** @const */ var XYZ = 1; XYZ >>= 2;");
}

#[test]
fn test_abbreviated_shift_assignment2() {
    let (mut t, mut h) = set_up();
    test_error(
        &mut t,
        &mut h,
        "/** @const */ var a$b$XYZ = 1; a$b$XYZ <<= 2;",
    );
}

#[test]
fn test_const_annotation1() {
    let (mut t, mut h) = set_up();
    test_warning(&mut t, &mut h, "/** @const */ var XYZ = 1; XYZ = 2;");
}

#[test]
fn test_const_annotation2() {
    let (mut t, mut h) = set_up();
    test_warning(&mut t, &mut h, "/** @const */ let x = 1; x = 2;");
}

#[test]
fn test_const_annotation3() {
    let (mut t, mut h) = set_up();
    test_warning(&mut t, &mut h, "/** @const */ const xyz = 1; xyz = 2;");
}

#[test]
fn test_const_suppression_in_file_js_doc() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "/**\n * @fileoverview\n * @suppress {const}\n */\n/** @const */ var xyz = 1; xyz = 3;\n",
    );
}

#[test]
fn test_const_suppression_on_assignment() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "/** @const */ var xyz = 1; /** @suppress {const} */ xyz = 3;",
    );
}

#[test]
fn test_const_suppression_on_add_assign() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "/** @const */ var xyz = 1; /** @suppress {const} */ xyz += 1;",
    );
}

#[test]
fn test_const_suppression_on_var() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "/** @const */ var xyz = 1;\n/** @suppress {const} */ var xyz = 3;",
    );
}

// If there are two 'var' statements for the same variable, one in externs and
// one in the JS, there is no normalization, and the suppression remains on the
// statement in the JS.
#[test]
fn test_const_suppression_on_var_from_externs() {
    let (mut t, mut h) = set_up();
    let externs_code = "/** @const */ var xyz;";
    let js = "/** @suppress {const} */ var xyz = 3;";
    test_same_parts(&mut t, &mut h, vec![externs(externs_code), srcs(js)]);
}

#[test]
fn test_const_suppression_on_inc() {
    let (mut t, mut h) = set_up();
    test_same(
        &mut t,
        &mut h,
        "/** @const */ var xyz = 1; /** @suppress {const} */ xyz++;",
    );
}

#[test]
fn test_const_name_in_externs() {
    let (mut t, mut h) = set_up();
    let externs_code = "/** @const */ var FOO;";
    let js = "FOO = 1;";
    test_warning_parts(
        &mut t,
        &mut h,
        vec![
            externs(externs_code),
            srcs(js),
            warning(&CONST_REASSIGNED_VALUE_ERROR),
        ],
    );
}

#[test]
fn test_goog_module_exports_shadowing_global() {
    let (mut t, mut h) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![srcs_n(&[
            "/** @const */ var exports = {};",
            "goog.module('m'); exports = class {};",
        ])],
    );
}

#[test]
fn test_goog_module_exports_reassigned() {
    let (mut t, mut h) = set_up();
    test_warning(
        &mut t,
        &mut h,
        "goog.module('m'); exports = class {}; exports = class {};",
    );
}

#[test]
fn test_goog_provide_root_namespace_explicitly_declared() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "goog.provide('a'); var a = {};");
    test_same(&mut t, &mut h, "goog.provide('a'); var a = {}; a = 1;");
    var_checks_support::test_warning(
        &mut t,
        &mut h,
        "goog.provide('a'); /** @const */ var a = {}; a = 1;",
        &CONST_REASSIGNED_VALUE_ERROR,
    );
}

#[test]
fn test_goog_provide_root_namespace_implicitly_declared() {
    let (mut t, mut h) = set_up();
    test_same(&mut t, &mut h, "goog.provide('a'); a = {};");
    test_same(&mut t, &mut h, "goog.provide('a'); a = {}; a = 1;");
    test_same(&mut t, &mut h, "goog.provide('a'); a.B = class {};");
    test_same(&mut t, &mut h, "goog.provide('a.b.c'); a = class {};");
    test_same_parts(
        &mut t,
        &mut h,
        vec![srcs_n(&["goog.provide('a.b');", "goog.provide('a.c');"])],
    );
    test_same(&mut t, &mut h, "goog.provide('a'); a = 0; a++;");
}

// port: ConstCheckTest#testError
fn test_error(t: &mut CompilerTestCase, h: &mut Hooks, js: &str) {
    var_checks_support::test_warning(t, h, js, &CONST_REASSIGNED_VALUE_ERROR);
}

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
//   test/com/google/javascript/jscomp/DefaultCodingConventionTest.java.

use closure_jscomp::Compiler;
use closure_jscomp::{coding_convention::AssertionFunctionSpec, node_util::NodeUtil};
use closure_jscomp::{coding_conventions::CodingConventions, source_file::SourceFile};
use closure_jstype::js_type_registry::JSTypeRegistry;
use closure_rhino::error_reporter::NullErrorReporter;
use closure_rhino::node::NodeId;
use closure_rhino::{ir::IR, js_string::JsString, node::Ast};

// port: DefaultCodingConventionTest#testVarAndOptionalParams
#[test]
fn test_var_and_optional_params() {
    let conv = CodingConventions::get_default();
    let mut ast = Ast::new();
    let a = IR::name(&mut ast, "a");
    let b = IR::name(&mut ast, "b");
    let args = IR::param_list(&mut ast, &[a, b]);
    let a = IR::name(&mut ast, "opt_a");
    let b = IR::name(&mut ast, "opt_b");
    let opt_args = IR::param_list(&mut ast, &[a, b]);
    let more = IR::name(&mut ast, "more");
    let rest_node = IR::iter_rest(&mut ast, more);
    let rest = IR::param_list(&mut ast, &[rest_node]);
    assert!(!conv.is_var_args_parameter(&ast, args.get_first_child(&ast).unwrap()));
    assert!(!conv.is_var_args_parameter(&ast, args.get_last_child(&ast).unwrap()));
    assert!(!conv.is_var_args_parameter(&ast, opt_args.get_first_child(&ast).unwrap()));
    assert!(!conv.is_var_args_parameter(&ast, opt_args.get_last_child(&ast).unwrap()));
    assert!(!conv.is_optional_parameter(&ast, args.get_first_child(&ast).unwrap()));
    assert!(!conv.is_optional_parameter(&ast, args.get_last_child(&ast).unwrap()));
    assert!(!conv.is_optional_parameter(&ast, opt_args.get_first_child(&ast).unwrap()));
    assert!(!conv.is_optional_parameter(&ast, opt_args.get_last_child(&ast).unwrap()));
    assert!(conv.is_var_args_parameter(&ast, rest.get_last_child(&ast).unwrap()));
    assert!(!conv.is_optional_parameter(&ast, rest.get_first_child(&ast).unwrap()));
}

// port: DefaultCodingConventionTest#testInlineName
#[test]
fn test_inline_name() {
    let conv = CodingConventions::get_default();
    assert!(!conv.is_constant(&JsString::from("a")));
    assert!(!conv.is_constant(&JsString::from("XYZ123_")));
    assert!(!conv.is_constant(&JsString::from("ABC")));
    assert!(!conv.is_constant(&JsString::from("ABCdef")));
    assert!(!conv.is_constant(&JsString::from("aBC")));
    assert!(!conv.is_constant(&JsString::from("A")));
    assert!(!conv.is_constant(&JsString::from("_XYZ123")));
    assert!(!conv.is_constant(&JsString::from("a$b$XYZ123_")));
    assert!(!conv.is_constant(&JsString::from("a$b$ABC_DEF")));
    assert!(!conv.is_constant(&JsString::from("a$b$A")));
    assert!(!conv.is_constant(&JsString::from("a$b$a")));
    assert!(!conv.is_constant(&JsString::from("a$b$ABCdef")));
    assert!(!conv.is_constant(&JsString::from("a$b$aBC")));
    assert!(!conv.is_constant(&JsString::from("a$b$")));
    assert!(!conv.is_constant(&JsString::from("$")));
}

// port: DefaultCodingConventionTest#testExportedName
#[test]
fn test_exported_name() {
    let conv = CodingConventions::get_default();
    assert!(!conv.is_exported_name(&JsString::from("_a")));
    assert!(!conv.is_exported_name(&JsString::from("_a_")));
    assert!(!conv.is_exported_name(&JsString::from("a")));
    assert!(!conv.is_exported(&JsString::from("$super"), false));
    assert!(conv.is_exported(&JsString::from("$super"), true));
    assert!(conv.is_exported_name(&JsString::from("$super")));
}

// port: DefaultCodingConventionTest#testEnumKey
#[test]
fn test_enum_key() {
    let conv = CodingConventions::get_default();
    assert!(conv.is_valid_enum_key(Some(&JsString::from("A"))));
    assert!(conv.is_valid_enum_key(Some(&JsString::from("123"))));
    assert!(conv.is_valid_enum_key(Some(&JsString::from("FOO_BAR"))));
    assert!(conv.is_valid_enum_key(Some(&JsString::from("a"))));
    assert!(conv.is_valid_enum_key(Some(&JsString::from("someKeyInCamelCase"))));
    assert!(conv.is_valid_enum_key(Some(&JsString::from("_FOO_BAR"))));
}

// port: DefaultCodingConventionTest#testInheritanceDetection1
#[test]
fn test_inheritance_detection1() {
    assert_not_class_defining("goog.foo(A, B);");
}

// port: DefaultCodingConventionTest#testInheritanceDetection2
#[test]
fn test_inheritance_detection2() {
    assert_not_class_defining("goog.inherits(A, B);");
}

// port: DefaultCodingConventionTest#testInheritanceDetection3
#[test]
fn test_inheritance_detection3() {
    assert_not_class_defining("A.inherits(B);");
}

// port: DefaultCodingConventionTest#testInheritanceDetection4
#[test]
fn test_inheritance_detection4() {
    assert_not_class_defining("goog.inherits(goog.A, goog.B);");
}

// port: DefaultCodingConventionTest#testInheritanceDetection5
#[test]
fn test_inheritance_detection5() {
    assert_not_class_defining("goog.A.inherits(goog.B);");
}

// port: DefaultCodingConventionTest#testInheritanceDetection6
#[test]
fn test_inheritance_detection6() {
    assert_not_class_defining("A.inherits(this.B);");
}

// port: DefaultCodingConventionTest#testInheritanceDetection7
#[test]
fn test_inheritance_detection7() {
    assert_not_class_defining("this.A.inherits(B);");
}

// port: DefaultCodingConventionTest#testInheritanceDetection8
#[test]
fn test_inheritance_detection8() {
    assert_not_class_defining("goog.inherits(A, B, C);");
}

// port: DefaultCodingConventionTest#testInheritanceDetection9
#[test]
fn test_inheritance_detection9() {
    assert_not_class_defining("A.mixin(B.prototype);");
}

// port: DefaultCodingConventionTest#testInheritanceDetection11
#[test]
fn test_inheritance_detection11() {
    assert_defines_classes("$jscomp.inherits(A, B)", "A", "B");
}

// port: DefaultCodingConventionTest#testInheritanceDetection12
#[test]
fn test_inheritance_detection12() {
    assert_defines_classes("$jscomp$inherits(A, B)", "A", "B");
}

// port: DefaultCodingConventionTest#testInheritanceDetectionPostCollapseProperties
#[test]
fn test_inheritance_detection_post_collapse_properties() {
    assert_not_class_defining("goog$inherits(A, B);");
    assert_not_class_defining("goog$inherits(A);");
}

// port: DefaultCodingConventionTest#testFunctionBind
#[test]
fn test_function_bind() {
    assert_not_function_bind("goog.bind(f)");
    assert_not_function_bind("goog$bind(f)");
    assert_not_function_bind("goog.partial(f)");
    assert_not_function_bind("goog$partial(f)");
    assert_function_bind("(function(){}).bind()");
    assert_function_bind("(function(){}).bind(obj)");
    assert_function_bind("(function(){}).bind(obj, p1)");
    assert_not_function_bind("Function.prototype.bind.call()");
    assert_function_bind("Function.prototype.bind.call(obj)");
    assert_function_bind("Function.prototype.bind.call(obj, p1)");
}

// port: DefaultCodingConventionTest#testPackageNames
#[test]
fn test_package_names() {
    assert_package_name("foo.js", "");
    assert_package_name("foo/bar.js", "foo");
    assert_package_name("foo/bar/baz.js", "foo/bar");
    assert_package_name("foo/bar/baz/quux.js", "foo/bar/baz");
    assert_package_name("foo/test/bar.js", "foo/test");
    assert_package_name("foo/testxyz/bar.js", "foo/testxyz");
}

// port: DefaultCodingConventionTest#makeReturnTypeAssertion_assertsOnFirstArg
#[test]
fn make_return_type_assertion_asserts_on_first_arg() {
    let spec = AssertionFunctionSpec::for_matches_return()
        .set_function_name("assertNumber")
        .build();
    let (compiler, n) = parse_test_code("assertNumber(0, 1, 2, 3);");
    let call_node = n.get_first_child(&compiler).unwrap();
    assert!(call_node.is_call(&compiler));
    let first_arg = NodeUtil::get_argument_for_call_or_new(&compiler, call_node, 0);
    let asserted_arg = spec.get_asserted_arg(&compiler, first_arg).unwrap();
    assert!(asserted_arg.is_number(&compiler));
    assert_eq!(asserted_arg.get_double(&compiler), 0.0);
}

// port: DefaultCodingConventionTest#makeTruthyAssertion_assertsOnFirstArg
#[test]
fn make_truthy_assertion_asserts_on_first_arg() {
    let spec = AssertionFunctionSpec::for_truthy()
        .set_function_name("assertTruthy")
        .build();
    let (compiler, n) = parse_test_code("assertTruthy(0, 1, 2, 3);");
    let call_node = n.get_first_child(&compiler).unwrap();
    assert!(call_node.is_call(&compiler));
    let first_arg = NodeUtil::get_argument_for_call_or_new(&compiler, call_node, 0);
    let asserted_arg = spec.get_asserted_arg(&compiler, first_arg).unwrap();
    assert!(asserted_arg.is_number(&compiler));
    assert_eq!(asserted_arg.get_double(&compiler), 0.0);
}

// port: DefaultCodingConventionTest#makeTruthyAssertion_withParamIndexOfTwo_assertsOnThirdArg
#[test]
fn make_truthy_assertion_with_param_index_of_two_asserts_on_third_arg() {
    let spec = AssertionFunctionSpec::for_truthy()
        .set_function_name("assertTruthy")
        .set_param_index(2)
        .build();
    let (compiler, n) = parse_test_code("assertTruthy(0, 1, 2, 3);");
    let call_node = n.get_first_child(&compiler).unwrap();
    assert!(call_node.is_call(&compiler));
    let first_arg = NodeUtil::get_argument_for_call_or_new(&compiler, call_node, 0);
    let asserted_arg = spec.get_asserted_arg(&compiler, first_arg).unwrap();
    assert!(asserted_arg.is_number(&compiler));
    assert_eq!(asserted_arg.get_double(&compiler), 2.0);
}

// port: DefaultCodingConventionTest#assertPackageName
fn assert_package_name(filename: &str, expected_package_name: &str) {
    let conv = CodingConventions::get_default();
    let source_file = SourceFile::from_code(filename, "");
    assert_eq!(
        conv.get_package_name(&source_file).as_deref(),
        Some(expected_package_name)
    );
}

// port: DefaultCodingConventionTest#assertFunctionBind
fn assert_function_bind(code: &str) {
    let conv = CodingConventions::get_default();
    let (mut compiler, n) = parse_test_code(code);
    let mut registry = JSTypeRegistry::new(&mut compiler, Box::new(NullErrorReporter), Vec::new());
    assert!(
        conv.describe_function_bind(
            &compiler,
            Some(&mut registry),
            n.get_first_child(&compiler).unwrap(),
            false
        )
        .is_some()
    );
}

// port: DefaultCodingConventionTest#assertNotFunctionBind
fn assert_not_function_bind(code: &str) {
    let conv = CodingConventions::get_default();
    let (mut compiler, n) = parse_test_code(code);
    let mut registry = JSTypeRegistry::new(&mut compiler, Box::new(NullErrorReporter), Vec::new());
    assert!(
        conv.describe_function_bind(
            &compiler,
            Some(&mut registry),
            n.get_first_child(&compiler).unwrap(),
            false
        )
        .is_none()
    );
}

// port: DefaultCodingConventionTest#assertNotClassDefining
fn assert_not_class_defining(code: &str) {
    let conv = CodingConventions::get_default();
    let (compiler, n) = parse_test_code(code);
    assert!(
        conv.get_classes_defined_by_call(&compiler, n.get_first_child(&compiler).unwrap())
            .is_none()
    );
}

// port: DefaultCodingConventionTest#assertDefinesClasses
fn assert_defines_classes(code: &str, subclass_name: &str, superclass_name: &str) {
    let conv = CodingConventions::get_default();
    let (compiler, n) = parse_test_code(code);
    let classes = conv
        .get_classes_defined_by_call(&compiler, n.get_first_child(&compiler).unwrap())
        .unwrap();
    assert_eq!(classes.subclass_name, subclass_name);
    assert_eq!(classes.superclass_name, superclass_name);
}

// port: DefaultCodingConventionTest#parseTestCode
fn parse_test_code(code: &str) -> (Compiler, NodeId) {
    let mut compiler = Compiler::new();
    let script = compiler.parse_test_code(code);
    let n = script.get_first_child(&compiler).unwrap();
    (compiler, n)
}

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
//   test/com/google/javascript/jscomp/GoogleCodingConventionTest.java.

use closure_jscomp::Compiler;
use closure_jscomp::{
    coding_convention::CodingConvention, google_coding_convention::GoogleCodingConvention,
    source_file::SourceFile,
};
use closure_rhino::node::NodeId;
use closure_rhino::{ir::IR, js_string::JsString, node::Ast};

// port: GoogleCodingConventionTest#testVarAndOptionalParams
#[test]
fn test_var_and_optional_params() {
    let conv = GoogleCodingConvention::new();
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
    assert!(conv.is_optional_parameter(&ast, opt_args.get_first_child(&ast).unwrap()));
    assert!(conv.is_optional_parameter(&ast, opt_args.get_last_child(&ast).unwrap()));
    assert!(conv.is_var_args_parameter(&ast, rest.get_last_child(&ast).unwrap()));
    assert!(!conv.is_optional_parameter(&ast, rest.get_first_child(&ast).unwrap()));
}

// port: GoogleCodingConventionTest#testIsConstant
#[test]
fn test_is_constant() {
    let conv = GoogleCodingConvention::new();
    assert!(!conv.is_constant(&JsString::from("a")));
    assert!(conv.is_constant(&JsString::from("XYZ123_")));
    assert!(conv.is_constant(&JsString::from("ABC")));
    assert!(!conv.is_constant(&JsString::from("ABCdef")));
    assert!(!conv.is_constant(&JsString::from("aBC")));
    assert!(!conv.is_constant(&JsString::from("A")));
    assert!(!conv.is_constant(&JsString::from("_XYZ123")));
    assert!(conv.is_constant(&JsString::from("a$b$XYZ123_")));
    assert!(conv.is_constant(&JsString::from("a$b$ABC_DEF")));
    assert!(conv.is_constant(&JsString::from("a$b$A")));
    assert!(!conv.is_constant(&JsString::from("a$b$a")));
    assert!(!conv.is_constant(&JsString::from("a$b$ABCdef")));
    assert!(!conv.is_constant(&JsString::from("a$b$aBC")));
    assert!(!conv.is_constant(&JsString::from("a$b$")));
    assert!(!conv.is_constant(&JsString::from("$")));
    assert!(conv.is_constant(&JsString::from("$A")));
    assert!(!conv.is_constant(&JsString::from("$a")));
}

// port: GoogleCodingConventionTest#testIsConstantKey
#[test]
fn test_is_constant_key() {
    let conv = GoogleCodingConvention::new();
    assert!(!conv.is_constant_key(&JsString::from("a")));
    assert!(conv.is_constant_key(&JsString::from("XYZ123_")));
    assert!(conv.is_constant_key(&JsString::from("ABC")));
    assert!(!conv.is_constant_key(&JsString::from("ABCdef")));
    assert!(!conv.is_constant_key(&JsString::from("aBC")));
    assert!(conv.is_constant_key(&JsString::from("A")));
    assert!(!conv.is_constant_key(&JsString::from("_XYZ123")));
    assert!(!conv.is_constant_key(&JsString::from("a$b$ABC")));
    assert!(!conv.is_constant_key(&JsString::from("$")));
}

// port: GoogleCodingConventionTest#testExportedName
#[test]
fn test_exported_name() {
    let conv = GoogleCodingConvention::new();
    assert!(conv.is_exported_name(&JsString::from("_a")));
    assert!(conv.is_exported_name(&JsString::from("_a_")));
    assert!(!conv.is_exported_name(&JsString::from("a")));
    assert!(!conv.is_exported(&JsString::from("$super"), false));
    assert!(conv.is_exported(&JsString::from("$super"), true));
    assert!(conv.is_exported_name(&JsString::from("$super")));
}

// port: GoogleCodingConventionTest#testEnumKey
#[test]
fn test_enum_key() {
    let conv = GoogleCodingConvention::new();
    assert!(conv.is_valid_enum_key(Some(&JsString::from("A"))));
    assert!(conv.is_valid_enum_key(Some(&JsString::from("123"))));
    assert!(conv.is_valid_enum_key(Some(&JsString::from("FOO_BAR"))));
    assert!(!conv.is_valid_enum_key(Some(&JsString::from("a"))));
    assert!(!conv.is_valid_enum_key(Some(&JsString::from("someKeyInCamelCase"))));
    assert!(!conv.is_valid_enum_key(Some(&JsString::from("_FOO_BAR"))));
}

// port: GoogleCodingConventionTest#testInheritanceDetection1
#[test]
fn test_inheritance_detection1() {
    assert_not_class_defining("goog.foo(A, B);");
}

// port: GoogleCodingConventionTest#testInheritanceDetection2
#[test]
fn test_inheritance_detection2() {
    assert_defines_classes("goog.inherits(A, B);", "A", "B");
}

// port: GoogleCodingConventionTest#testInheritanceDetection3
#[test]
fn test_inheritance_detection3() {
    assert_not_class_defining("A.inherits(B);");
}

// port: GoogleCodingConventionTest#testInheritanceDetection4
#[test]
fn test_inheritance_detection4() {
    assert_defines_classes("goog.inherits(goog.A, goog.B);", "goog.A", "goog.B");
}

// port: GoogleCodingConventionTest#testInheritanceDetection5
#[test]
fn test_inheritance_detection5() {
    assert_not_class_defining("goog.A.inherits(goog.B);");
}

// port: GoogleCodingConventionTest#testInheritanceDetection6
#[test]
fn test_inheritance_detection6() {
    assert_not_class_defining("A.inherits(this.B);");
}

// port: GoogleCodingConventionTest#testInheritanceDetection7
#[test]
fn test_inheritance_detection7() {
    assert_not_class_defining("this.A.inherits(B);");
}

// port: GoogleCodingConventionTest#testInheritanceDetection8
#[test]
fn test_inheritance_detection8() {
    assert_defines_classes("goog.inherits(A, B, C);", "A", "B");
}

// port: GoogleCodingConventionTest#testInheritanceDetection9
#[test]
fn test_inheritance_detection9() {
    assert_not_class_defining("A.mixin(B.prototype);");
}

// port: GoogleCodingConventionTest#testInheritanceDetectionPostCollapseProperties
#[test]
fn test_inheritance_detection_post_collapse_properties() {
    assert_defines_classes("goog$inherits(A, B);", "A", "B");
    assert_not_class_defining("goog$inherits(A);");
}

// port: GoogleCodingConventionTest#testPackageNames
#[test]
fn test_package_names() {
    assert_package_name("foo.js", "");
    assert_package_name("foo/bar.js", "foo");
    assert_package_name("foo/bar/baz.js", "foo/bar");
    assert_package_name("foo/bar/baz/quux.js", "foo/bar/baz");
    assert_package_name("foo/test/bar.js", "foo");
    assert_package_name("foo/tests/bar.js", "foo");
    assert_package_name("foo/testing/bar.js", "foo");
    assert_package_name("foo/jstest/bar.js", "foo/jstest");
    assert_package_name("foo/bar/test/baz.js", "foo/bar");
    assert_package_name("foo/bar/tests/baz.js", "foo/bar");
    assert_package_name("foo/bar/testing/baz.js", "foo/bar");
    assert_package_name("foo/bar/testament/baz.js", "foo/bar/testament");
    assert_package_name("foo/test/bar/baz.js", "foo/test/bar");
    assert_package_name("foo/bar/baz/test/quux.js", "foo/bar/baz");
    assert_package_name("foo/bar/baz/tests/quux.js", "foo/bar/baz");
    assert_package_name("foo/bar/baz/testing/quux.js", "foo/bar/baz");
    assert_package_name("foo/bar/baz/unittests/quux.js", "foo/bar/baz/unittests");
    assert_package_name("foo/bar/test/baz/quux.js", "foo/bar/test/baz");
    assert_package_name("foo/test/bar/baz/quux.js", "foo/test/bar/baz");
    assert_package_name("bazel-out/host/genfiles/bar/baz/quux.js", "bar/baz");
    assert_package_name("bazel-out/host/genfiles/foo/test/bar.js", "foo");
    assert_package_name("bazel-out/host/bin/bar/baz/quux.js", "bar/baz");
    assert_package_name("bazel-out/host/bin/foo/test/bar.js", "foo");
}

// port: GoogleCodingConventionTest#assertPackageName
fn assert_package_name(filename: &str, expected_package_name: &str) {
    let conv = GoogleCodingConvention::new();
    let source_file = SourceFile::from_code(filename, "");
    assert_eq!(
        conv.get_package_name(&source_file).as_deref(),
        Some(expected_package_name)
    );
}

// port: GoogleCodingConventionTest#assertNotClassDefining
fn assert_not_class_defining(code: &str) {
    let conv = GoogleCodingConvention::new();
    let (compiler, n) = parse_test_code(code);
    assert!(
        conv.get_classes_defined_by_call(&compiler, n.get_first_child(&compiler).unwrap())
            .is_none()
    );
}

// port: GoogleCodingConventionTest#assertDefinesClasses
fn assert_defines_classes(code: &str, subclass_name: &str, superclass_name: &str) {
    let conv = GoogleCodingConvention::new();
    let (compiler, n) = parse_test_code(code);
    let classes = conv
        .get_classes_defined_by_call(&compiler, n.get_first_child(&compiler).unwrap())
        .unwrap();
    assert_eq!(classes.subclass_name, subclass_name);
    assert_eq!(classes.superclass_name, superclass_name);
}

// port: GoogleCodingConventionTest#parseTestCode
fn parse_test_code(code: &str) -> (Compiler, NodeId) {
    let mut compiler = Compiler::new();
    let script = compiler.parse_test_code(code);
    let n = script.get_first_child(&compiler).unwrap();
    (compiler, n)
}

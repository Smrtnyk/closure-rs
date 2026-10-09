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
//   test/com/google/javascript/jscomp/ClosureCodingConventionTest.java.

use closure_jscomp::Compiler;
use closure_jscomp::{
    closure_coding_convention::ClosureCodingConvention, coding_convention::CodingConvention,
};
use closure_jstype::js_type_registry::JSTypeRegistry;
use closure_rhino::error_reporter::NullErrorReporter;
use closure_rhino::node::NodeId;
use closure_rhino::{js_string::JsString, node::Ast, token::Token};

// port: ClosureCodingConventionTest#testVarAndOptionalParams
#[test]
fn test_var_and_optional_params() {
    let conv = ClosureCodingConvention::new();
    let mut ast = Ast::new();
    let a = ast.new_string_with_token(Token::NAME, "a");
    let b = ast.new_string_with_token(Token::NAME, "b");
    let args = ast.new_node_with_children2(Token::PARAM_LIST, a, b);
    let a = ast.new_string_with_token(Token::NAME, "opt_a");
    let b = ast.new_string_with_token(Token::NAME, "opt_b");
    let opt_args = ast.new_node_with_children2(Token::PARAM_LIST, a, b);
    assert!(!conv.is_var_args_parameter(&ast, args.get_first_child(&ast).unwrap()));
    assert!(!conv.is_var_args_parameter(&ast, args.get_last_child(&ast).unwrap()));
    assert!(!conv.is_var_args_parameter(&ast, opt_args.get_first_child(&ast).unwrap()));
    assert!(!conv.is_var_args_parameter(&ast, opt_args.get_last_child(&ast).unwrap()));
    assert!(!conv.is_optional_parameter(&ast, args.get_first_child(&ast).unwrap()));
    assert!(!conv.is_optional_parameter(&ast, args.get_last_child(&ast).unwrap()));
    assert!(!conv.is_optional_parameter(&ast, opt_args.get_first_child(&ast).unwrap()));
    assert!(!conv.is_optional_parameter(&ast, opt_args.get_last_child(&ast).unwrap()));
}

// port: ClosureCodingConventionTest#testInlineName
#[test]
fn test_inline_name() {
    let conv = ClosureCodingConvention::new();
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

// port: ClosureCodingConventionTest#testExportedName
#[test]
fn test_exported_name() {
    let conv = ClosureCodingConvention::new();
    assert!(!conv.is_exported_name(&JsString::from("_a")));
    assert!(!conv.is_exported_name(&JsString::from("_a_")));
    assert!(!conv.is_exported_name(&JsString::from("a")));
    assert!(!conv.is_exported(&JsString::from("$super"), false));
    assert!(conv.is_exported(&JsString::from("$super"), true));
    assert!(conv.is_exported_name(&JsString::from("$super")));
}

// port: ClosureCodingConventionTest#testEnumKey
#[test]
fn test_enum_key() {
    let conv = ClosureCodingConvention::new();
    assert!(conv.is_valid_enum_key(Some(&JsString::from("A"))));
    assert!(conv.is_valid_enum_key(Some(&JsString::from("123"))));
    assert!(conv.is_valid_enum_key(Some(&JsString::from("FOO_BAR"))));
    assert!(conv.is_valid_enum_key(Some(&JsString::from("a"))));
    assert!(conv.is_valid_enum_key(Some(&JsString::from("someKeyInCamelCase"))));
    assert!(conv.is_valid_enum_key(Some(&JsString::from("_FOO_BAR"))));
}

// port: ClosureCodingConventionTest#testInheritanceDetection1
#[test]
fn test_inheritance_detection1() {
    assert_not_class_defining("goog.foo(A, B);");
}

// port: ClosureCodingConventionTest#testInheritanceDetection2
#[test]
fn test_inheritance_detection2() {
    assert_defines_classes("goog.inherits(A, B);", "A", "B");
}

// port: ClosureCodingConventionTest#testInheritanceDetection3
#[test]
fn test_inheritance_detection3() {
    assert_not_class_defining("A.inherits(B);");
}

// port: ClosureCodingConventionTest#testInheritanceDetection4
#[test]
fn test_inheritance_detection4() {
    assert_defines_classes("goog.inherits(goog.A, goog.B);", "goog.A", "goog.B");
}

// port: ClosureCodingConventionTest#testInheritanceDetection5
#[test]
fn test_inheritance_detection5() {
    assert_not_class_defining("goog.A.inherits(goog.B);");
}

// port: ClosureCodingConventionTest#testInheritanceDetection6
#[test]
fn test_inheritance_detection6() {
    assert_not_class_defining("A.inherits(this.B);");
}

// port: ClosureCodingConventionTest#testInheritanceDetection7
#[test]
fn test_inheritance_detection7() {
    assert_not_class_defining("this.A.inherits(B);");
}

// port: ClosureCodingConventionTest#testInheritanceDetection8
#[test]
fn test_inheritance_detection8() {
    assert_defines_classes("goog.inherits(A, B, C);", "A", "B");
}

// port: ClosureCodingConventionTest#testInheritanceDetection9
#[test]
fn test_inheritance_detection9() {
    assert_not_class_defining("A.mixin(B.prototype);");
}

// port: ClosureCodingConventionTest#testInheritanceDetection11
#[test]
fn test_inheritance_detection11() {
    assert_not_class_defining("A.mixin(B)");
}

// port: ClosureCodingConventionTest#testInheritanceDetection15
#[test]
fn test_inheritance_detection15() {
    assert_defines_classes("$jscomp.inherits(A, B)", "A", "B");
}

// port: ClosureCodingConventionTest#testInheritanceDetection16
#[test]
fn test_inheritance_detection16() {
    assert_defines_classes("$jscomp$inherits(A, B)", "A", "B");
}

// port: ClosureCodingConventionTest#testInheritanceDetection17
#[test]
fn test_inheritance_detection17() {
    assert_defines_classes(
        "ValueType.mixin(A, B, 5, goog.reflect.objectProperty('foo', A))",
        "A",
        "B",
    );
}

// port: ClosureCodingConventionTest#testInheritanceDetection18
#[test]
fn test_inheritance_detection18() {
    assert_not_class_defining("ValueType.mixin(A, 5, goog.reflect.objectProperty('foo', A))");
}

// port: ClosureCodingConventionTest#testInheritanceDetectionPostCollapseProperties
#[test]
fn test_inheritance_detection_post_collapse_properties() {
    assert_defines_classes("goog$inherits(A, B);", "A", "B");
    assert_not_class_defining("goog$inherits(A);");
}

// port: ClosureCodingConventionTest#testObjectLiteralCast
#[test]
fn test_object_literal_cast() {
    assert_not_object_literal_cast("goog.reflect.object();");
    assert_not_object_literal_cast("goog.reflect.object(A);");
    assert_not_object_literal_cast("goog.reflect.object(1, {});");
    assert_object_literal_cast("goog.reflect.object(A, {});");
    assert_not_object_literal_cast("$jscomp.reflectObject();");
    assert_not_object_literal_cast("$jscomp.reflectObject(A);");
    assert_not_object_literal_cast("$jscomp.reflectObject(1, {});");
    assert_object_literal_cast("$jscomp.reflectObject(A, {});");
}

// port: ClosureCodingConventionTest#testFunctionBind
#[test]
fn test_function_bind() {
    assert_not_function_bind("goog.bind()");
    assert_function_bind("goog.bind(f)");
    assert_function_bind("goog.bind(f, obj)");
    assert_function_bind("goog.bind(f, obj, p1)");
    assert_not_function_bind("goog$bind()");
    assert_function_bind("goog$bind(f)");
    assert_function_bind("goog$bind(f, obj)");
    assert_function_bind("goog$bind(f, obj, p1)");
    assert_not_function_bind("goog.partial()");
    assert_function_bind("goog.partial(f)");
    assert_function_bind("goog.partial(f, obj)");
    assert_function_bind("goog.partial(f, obj, p1)");
    assert_not_function_bind("goog$partial()");
    assert_function_bind("goog$partial(f)");
    assert_function_bind("goog$partial(f, obj)");
    assert_function_bind("goog$partial(f, obj, p1)");
    assert_function_bind("(function(){}).bind()");
    assert_function_bind("(function(){}).bind(obj)");
    assert_function_bind("(function(){}).bind(obj, p1)");
    assert_not_function_bind("Function.prototype.bind.call()");
    assert_function_bind("Function.prototype.bind.call(obj)");
    assert_function_bind("Function.prototype.bind.call(obj, p1)");
}

// port: ClosureCodingConventionTest#testRequire
#[test]
fn test_require() {
    assert_require("goog.require('foo')");
    assert_not_require("goog.require(foo)");
    assert_not_require("goog.require()");
    assert_not_require("foo()");
}

// port: ClosureCodingConventionTest#testApplySubclassRelationship
#[test]
fn test_apply_subclass_relationship() {
    use closure_jscomp::coding_convention::SubclassType;
    use closure_jstype::{
        prelude::{FunctionType, JSTypeRegistry, ObjectType},
        rhino::nominal_type_builder::NominalTypeBuilder,
    };
    use closure_rhino::error_reporter::NullErrorReporter;

    let conv = ClosureCodingConvention::new();
    let mut ast = Ast::new();
    let mut registry = JSTypeRegistry::new(&mut ast, Box::new(NullErrorReporter), Vec::new());
    let mut closer = registry.get_resolver().open_for_definition();
    let node_a = ast.new_node(Token::FUNCTION);
    let ctor_a = registry.create_constructor_type(
        &ast,
        Some(JsString::from("A")),
        Some(node_a),
        Some(registry.create_parameters(&[])),
        None,
        None,
        false,
    );
    let node_b = ast.new_node(Token::FUNCTION);
    let ctor_b = registry.create_constructor_type(
        &ast,
        Some(JsString::from("B")),
        Some(node_b),
        Some(registry.create_parameters(&[])),
        None,
        None,
        false,
    );
    let instance_a = ctor_a.get_instance_type(&registry).unwrap();
    let parent = NominalTypeBuilder::new(&mut registry, &ast, ctor_a, instance_a);
    let instance_b = ctor_b.get_instance_type(&registry).unwrap();
    let child = NominalTypeBuilder::new(&mut registry, &ast, ctor_b, instance_b);
    conv.apply_subclass_relationship(&ast, &mut registry, &parent, &child, SubclassType::INHERITS);

    assert!(ctor_b.get_prototype(&mut registry, &ast).has_own_property(
        &mut registry,
        &ast,
        "constructor"
    ));
    assert_eq!(
        ctor_b.get_prototype(&mut registry, &ast).get_property_node(
            &mut registry,
            &ast,
            "constructor"
        ),
        Some(node_b)
    );
    assert!(ctor_b.has_own_property(&mut registry, &ast, "superClass_"));
    assert_eq!(
        ctor_b.get_property_node(&mut registry, &ast, "superClass_"),
        Some(node_b)
    );
    closer.close(&mut registry, &ast);
}

// port: ClosureCodingConventionTest#testDescribeCachingCall
#[test]
fn test_describe_caching_call() {
    assert_caching_call("goog.reflect.cache(obj, 10, function() {})");
    assert_caching_call("goog.reflect.cache(obj, 10, function() {}, function() {})");
    assert_caching_call("goog$reflect$cache(obj, 10, function() {})");
    assert_caching_call("goog$reflect$cache(obj, 10, function() {}, function() {})");
    assert_not_caching_call("goog.reflect.cache()");
    assert_not_caching_call("goog.reflect.cache(obj)");
    assert_not_caching_call("goog.reflect.cache(obj, 10)");
    assert_not_caching_call("foo.cache(obj, 10, function() {}, function() {})");
}

// port: ClosureCodingConventionTest#assertFunctionBind
fn assert_function_bind(code: &str) {
    let conv = ClosureCodingConvention::new();
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

// port: ClosureCodingConventionTest#assertNotFunctionBind
fn assert_not_function_bind(code: &str) {
    let conv = ClosureCodingConvention::new();
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

// port: ClosureCodingConventionTest#assertRequire
fn assert_require(code: &str) {
    let conv = ClosureCodingConvention::new();
    let (compiler, n) = parse_test_code(code);
    assert!(
        conv.extract_class_name_if_require(&compiler, n.get_first_child(&compiler).unwrap(), n)
            .is_some()
    );
}

// port: ClosureCodingConventionTest#assertNotRequire
fn assert_not_require(code: &str) {
    let conv = ClosureCodingConvention::new();
    let (compiler, n) = parse_test_code(code);
    assert!(
        conv.extract_class_name_if_require(&compiler, n.get_first_child(&compiler).unwrap(), n)
            .is_none()
    );
}

// port: ClosureCodingConventionTest#assertNotObjectLiteralCast
fn assert_not_object_literal_cast(code: &str) {
    let conv = ClosureCodingConvention::new();
    let (compiler, n) = parse_test_code(code);
    assert!(
        conv.get_object_literal_cast(&compiler, n.get_first_child(&compiler).unwrap())
            .is_none()
    );
}

// port: ClosureCodingConventionTest#assertObjectLiteralCast
fn assert_object_literal_cast(code: &str) {
    let conv = ClosureCodingConvention::new();
    let (compiler, n) = parse_test_code(code);
    assert!(
        conv.get_object_literal_cast(&compiler, n.get_first_child(&compiler).unwrap())
            .is_some()
    );
}

// port: ClosureCodingConventionTest#assertNotClassDefining
fn assert_not_class_defining(code: &str) {
    let conv = ClosureCodingConvention::new();
    let (compiler, n) = parse_test_code(code);
    assert!(
        conv.get_classes_defined_by_call(&compiler, n.get_first_child(&compiler).unwrap())
            .is_none()
    );
}

// port: ClosureCodingConventionTest#assertDefinesClasses
fn assert_defines_classes(code: &str, subclass_name: &str, superclass_name: &str) {
    let conv = ClosureCodingConvention::new();
    let (compiler, n) = parse_test_code(code);
    let classes = conv
        .get_classes_defined_by_call(&compiler, n.get_first_child(&compiler).unwrap())
        .unwrap();
    assert_eq!(classes.subclass_name, subclass_name);
    assert_eq!(classes.superclass_name, superclass_name);
}

// port: ClosureCodingConventionTest#assertCachingCall
fn assert_caching_call(code: &str) {
    let conv = ClosureCodingConvention::new();
    let (compiler, n) = parse_test_code(code);
    assert!(
        conv.describe_caching_call(&compiler, n.get_first_child(&compiler).unwrap())
            .is_some()
    );
}

// port: ClosureCodingConventionTest#assertNotCachingCall
fn assert_not_caching_call(code: &str) {
    let conv = ClosureCodingConvention::new();
    let (compiler, n) = parse_test_code(code);
    assert!(
        conv.describe_caching_call(&compiler, n.get_first_child(&compiler).unwrap())
            .is_none()
    );
}

// port: ClosureCodingConventionTest#parseTestCode
fn parse_test_code(code: &str) -> (Compiler, NodeId) {
    let mut compiler = Compiler::new();
    let script = compiler.parse_test_code(code);
    let n = script.get_first_child(&compiler).unwrap();
    (compiler, n)
}

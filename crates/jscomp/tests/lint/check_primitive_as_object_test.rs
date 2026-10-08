/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/lint/CheckPrimitiveAsObjectTest.java.

use super::support::LintTestCase;
use closure_jscomp::lint::check_primitive_as_object::{
    CheckPrimitiveAsObject, NEW_PRIMITIVE_OBJECT, PRIMITIVE_OBJECT_DECLARATION,
};

// port: CheckPrimitiveAsObjectTest#getProcessor
fn case() -> LintTestCase {
    LintTestCase::new(|c| Box::new(CheckPrimitiveAsObject::new(c)))
}

// port: CheckPrimitiveAsObjectTest#testWarningForBooleanObjectCreation
#[test]
fn test_warning_for_boolean_object_creation() {
    let t = case();
    t.test_warning("new Boolean(false)", &NEW_PRIMITIVE_OBJECT);
}

// port: CheckPrimitiveAsObjectTest#testWarningForNumberObjectCreation
#[test]
fn test_warning_for_number_object_creation() {
    let t = case();
    t.test_warning("new Number(5)", &NEW_PRIMITIVE_OBJECT);
}

// port: CheckPrimitiveAsObjectTest#testWarningForStringObjectCreation
#[test]
fn test_warning_for_string_object_creation() {
    let t = case();
    t.test_warning("new String(\"hello\")", &NEW_PRIMITIVE_OBJECT);
}

// port: CheckPrimitiveAsObjectTest#testNoWarningForObjectCreation
#[test]
fn test_no_warning_for_object_creation() {
    let t = case();
    t.test_same("new Object()");
}

// port: CheckPrimitiveAsObjectTest#testNoWarningForQualifiedClassCreation
#[test]
fn test_no_warning_for_qualified_class_creation() {
    let t = case();
    t.test_same("new my.qualified.ClassName()");
}

// port: CheckPrimitiveAsObjectTest#testWarningForBooleanTypeDeclaration
#[test]
fn test_warning_for_boolean_type_declaration() {
    let t = case();
    t.test_warning(
        "/** @type {Boolean} */ var x;",
        &PRIMITIVE_OBJECT_DECLARATION,
    );
}

// port: CheckPrimitiveAsObjectTest#testWarningForBooleanTypeDeclaration_withES6Modules
#[test]
fn test_warning_for_boolean_type_declaration_with_es6_modules() {
    let t = case();
    t.test_warning(
        "export /** @type {Boolean} */ var x;",
        &PRIMITIVE_OBJECT_DECLARATION,
    );
}

// port: CheckPrimitiveAsObjectTest#testWarningForNumberTypeDeclaration
#[test]
fn test_warning_for_number_type_declaration() {
    let t = case();
    t.test_warning(
        "/** @type {Number} */ var x;",
        &PRIMITIVE_OBJECT_DECLARATION,
    );
}

// port: CheckPrimitiveAsObjectTest#testWarningForNumberTypeDeclaration_withES6Modules
#[test]
fn test_warning_for_number_type_declaration_with_es6_modules() {
    let t = case();
    t.test_warning(
        "export /** @type {Number} */ var x;",
        &PRIMITIVE_OBJECT_DECLARATION,
    );
}

// port: CheckPrimitiveAsObjectTest#testWarningForStringTypeDeclaration
#[test]
fn test_warning_for_string_type_declaration() {
    let t = case();
    t.test_warning(
        "/** @type {String} */ var x;",
        &PRIMITIVE_OBJECT_DECLARATION,
    );
}

// port: CheckPrimitiveAsObjectTest#testWarningForStringTypeDeclaration_withES6Modules
#[test]
fn test_warning_for_string_type_declaration_with_es6_modules() {
    let t = case();
    t.test_warning(
        "export /** @type {String} */ var x;",
        &PRIMITIVE_OBJECT_DECLARATION,
    );
}

// port: CheckPrimitiveAsObjectTest#testWarningForBooleanInsideTypeDeclaration
#[test]
fn test_warning_for_boolean_inside_type_declaration() {
    let t = case();
    t.test_warning(
        "/** @type {function(): Boolean} */ var x;",
        &PRIMITIVE_OBJECT_DECLARATION,
    );
    t.test_warning(
        "/** @type {function(Boolean)} */ var x;",
        &PRIMITIVE_OBJECT_DECLARATION,
    );
    t.test_warning(
        "/** @type {{b: Boolean}} */ var x;",
        &PRIMITIVE_OBJECT_DECLARATION,
    );
}

// port: CheckPrimitiveAsObjectTest#testWarningForBooleanInsideTypeDeclaration_withES6Modules
#[test]
fn test_warning_for_boolean_inside_type_declaration_with_es6_modules() {
    let t = case();
    t.test_warning(
        "export /** @type {function(): Boolean} */ var x;",
        &PRIMITIVE_OBJECT_DECLARATION,
    );
}

// port: CheckPrimitiveAsObjectTest#testWarningForNumberParameterDeclaration
#[test]
fn test_warning_for_number_parameter_declaration() {
    let t = case();
    t.test_warning(
        r#"/**
 * @param {Number=} x
 * @return {number}
 */
function f(x) {
  return x + 1;
}
"#,
        &PRIMITIVE_OBJECT_DECLARATION,
    );
}

// port: CheckPrimitiveAsObjectTest#testWarningForNumberParameterDeclaration_withES6Modules
#[test]
fn test_warning_for_number_parameter_declaration_with_es6_modules() {
    let t = case();
    t.test_warning(
        r#"export
/**
 * @param {Number=} x
 * @return {number}
 */
function f(x) {
  return x + 1;
}
"#,
        &PRIMITIVE_OBJECT_DECLARATION,
    );
}

// port: CheckPrimitiveAsObjectTest#testWarningForBooleanParameterDeclarationInTypedef
#[test]
fn test_warning_for_boolean_parameter_declaration_in_typedef() {
    let t = case();
    t.test_warning(
        r#"/**
 * @typedef {function(Boolean=)}
 */
var takesOptionalBoolean;
"#,
        &PRIMITIVE_OBJECT_DECLARATION,
    );
}

// port: CheckPrimitiveAsObjectTest#testWarningForBooleanParameterDeclarationInTypedef_withES6Modules
#[test]
fn test_warning_for_boolean_parameter_declaration_in_typedef_with_es6_modules() {
    let t = case();
    t.test_warning(
        r#"export
/**
 * @typedef {function(Boolean=)}
 */
var takesOptionalBoolean;
"#,
        &PRIMITIVE_OBJECT_DECLARATION,
    );
}

// port: CheckPrimitiveAsObjectTest#testWarningForNumberReturnDeclaration
#[test]
fn test_warning_for_number_return_declaration() {
    let t = case();
    t.test_warning(
        r#"/**
 * @return {Number}
 */
function f() {
  return 5;
}
"#,
        &PRIMITIVE_OBJECT_DECLARATION,
    );
}

// port: CheckPrimitiveAsObjectTest#testWarningForNumberReturnDeclaration_withES6Modules
#[test]
fn test_warning_for_number_return_declaration_with_es6_modules() {
    let t = case();
    t.test_warning(
        r#"export
/**
 * @return {Number}
 */
function f() {
  return 5;
}
"#,
        &PRIMITIVE_OBJECT_DECLARATION,
    );
}

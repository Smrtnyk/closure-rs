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
//   test/com/google/javascript/jscomp/lint/CheckEnumsTest.java.

use super::support::LintTestCase;
use closure_jscomp::lint::check_enums::{
    COMPUTED_PROP_NAME_IN_ENUM, CheckEnums, DUPLICATE_ENUM_VALUE, ENUM_PROP_NOT_CONSTANT,
    ENUM_TYPE_NOT_STRING_OR_NUMBER, NON_STATIC_INITIALIZER_STRING_VALUE_IN_ENUM,
    SHORTHAND_ASSIGNMENT_IN_ENUM,
};

// port: CheckEnumsTest#getProcessor
// port: CheckEnumsTest#getOptions
fn case() -> LintTestCase {
    LintTestCase::new(|c| Box::new(CheckEnums::new(c))).with_lint_checks_warning()
}

// port: CheckEnumsTest#testCheckEnums
#[test]
fn test_check_enums() {
    let t = case();
    t.test_same("/** @enum {number} */ ns.Enum = {A: 1, B: 2};");
    t.test_same("/** @enum {string} */ ns.Enum = {A: 'foo', B: 'bar'};");
    t.test_warning(
        "/** @enum {number} */ ns.Enum = {A: 1, B: 1};",
        &DUPLICATE_ENUM_VALUE,
    );
    t.test_warning(
        "/** @enum {string} */ ns.Enum = {A: 'foo', B: 'foo'};",
        &DUPLICATE_ENUM_VALUE,
    );
    t.test_same("/** @enum {number} */ let Enum = {A: 1, B: 2};");
    t.test_same("/** @enum {string} */ let Enum = {A: 'foo', B: 'bar'};");
    t.test_warning(
        "/** @enum {number} */ let Enum = {A: 1, B: 1};",
        &DUPLICATE_ENUM_VALUE,
    );
    t.test_no_warning("/** @enum {number} */ let Enum = {A: 1+1, B: 1+1};");
    // uncaught dup values
    t.test_warning(
        "/** @enum {string} */ let Enum = {A: 'foo', B: 'foo'};",
        &DUPLICATE_ENUM_VALUE,
    );
    t.test_no_warning("/** @enum {number} */ let Enum = {A: 'a'+'b', B: 'a'+'b'};");
    // uncaught dup values
    t.test_warning(
        "/** @enum {number} */ let Enum = {A};",
        &SHORTHAND_ASSIGNMENT_IN_ENUM,
    );
    t.test_warning(
        "/** @enum {string} */ let Enum = {['prop' + f()]: 'foo'};",
        &COMPUTED_PROP_NAME_IN_ENUM,
    );
    t.test_warning(
        "/** @enum {number} */ let E = { a: 1 };",
        &ENUM_PROP_NOT_CONSTANT,
    );
    t.test_warning(
        "/** @enum {number} */ let E = { ABC: 1, abc: 2 };",
        &ENUM_PROP_NOT_CONSTANT,
    );
}

// port: CheckEnumsTest#testCheckValidEnums_withES6Modules
#[test]
fn test_check_valid_enums_with_es6_modules() {
    let t = case();
    t.test_same("export /** @enum {number} */ let Enum = {A: 1, B: 2};");
}

// port: CheckEnumsTest#testCheckInvalidEnums_withES6Modules01
#[test]
fn test_check_invalid_enums_with_es6_modules01() {
    let t = case();
    t.test_warning(
        "export /** @enum {number} */ let Enum = {A: 1, B: 1};",
        &DUPLICATE_ENUM_VALUE,
    );
}

// port: CheckEnumsTest#testCheckInvalidEnums_withES6Modules02
#[test]
fn test_check_invalid_enums_with_es6_modules02() {
    let t = case();
    t.test_warning(
        "export /** @enum {number} */ let Enum = {A};",
        &SHORTHAND_ASSIGNMENT_IN_ENUM,
    );
}

// port: CheckEnumsTest#testCheckInvalidEnums_withES6Modules03
#[test]
fn test_check_invalid_enums_with_es6_modules03() {
    let t = case();
    t.test_warning(
        "export /** @enum {string} */ let Enum = {['prop' + f()]: 'foo'};",
        &COMPUTED_PROP_NAME_IN_ENUM,
    );
}

// port: CheckEnumsTest#testCheckInvalidEnums_withES6Modules04
#[test]
fn test_check_invalid_enums_with_es6_modules04() {
    let t = case();
    t.test_warning(
        "export /** @enum {number} */ let E = { a: 1 };",
        &ENUM_PROP_NOT_CONSTANT,
    );
}

// port: CheckEnumsTest#testEnumTypeIsDeclaredStringOrNumber
#[test]
fn test_enum_type_is_declared_string_or_number() {
    let t = case();
    t.test_no_warning("/** @enum {number} */ let E = { A: 1 };");
    t.test_no_warning("/** @enum {number} */ let E = { A: 1+1 };");
    t.test_no_warning("/** @enum {string} */ let E = { A: 'static str' };");
    t.test_no_warning("/** @enum {string} */ let E = { A: `template lit` };");
    t.test_warning(
        "/** @enum {?number} */ let E = { A: 1 };",
        &ENUM_TYPE_NOT_STRING_OR_NUMBER,
    );
    t.test_warning(
        "/** @enum {!string} */ let E = { A: 'static str' };",
        &ENUM_TYPE_NOT_STRING_OR_NUMBER,
    );
    t.test_warning(
        "/** @enum {boolean} */ let E = {A: true };",
        &ENUM_TYPE_NOT_STRING_OR_NUMBER,
    );
    t.test_warning(
        "function foo() {} /** @enum {Function} */ let E = {A: foo };",
        &ENUM_TYPE_NOT_STRING_OR_NUMBER,
    );
    t.test_warning(
        "function foo() {} /** @enum {?} */ let E = {A: foo() };",
        &ENUM_TYPE_NOT_STRING_OR_NUMBER,
    );
    t.test_warning(
        r#"/** @typedef {number} */
var someName;
/** @enum {!someName} */
const EnumTwo = { FOO: EnumOne.FOO }
"#,
        &ENUM_TYPE_NOT_STRING_OR_NUMBER,
    );
    t.test_warning(
        r#"/** @typedef {string} */
var someName;
/** @enum {!someName} */
const EnumTwo = { FOO: 'some' }
"#,
        &ENUM_TYPE_NOT_STRING_OR_NUMBER,
    );
    t.test_warning(
        r#"/** @enum {number} */
let EnumOne = { FOO: 1, BAR: 2 };
/** @enum {!EnumOne}*/
let EnumTwo = { FOO: EnumOne.FOO }
"#,
        &ENUM_TYPE_NOT_STRING_OR_NUMBER,
    );
    t.test_warning(
        r#"/** @enum {string} */
let EnumOne = { FOO: 'foo',  BAR: 'bar' };
/** @enum {!EnumOne}*/
let EnumTwo = { FOO: EnumOne.FOO }
"#,
        &ENUM_TYPE_NOT_STRING_OR_NUMBER,
    );
    t.test_warning(
        " /** @enum {{x: number}} */ var E = {A: true };",
        &ENUM_TYPE_NOT_STRING_OR_NUMBER,
    );
}

// port: CheckEnumsTest#testNonStaticNumber_noWarning
#[test]
fn test_non_static_number_no_warning() {
    let t = case();
    // We do not warn on number enum values that are non-statically initialized.
    t.test_no_warning("let fooVal = 1; /** @enum {number} */ let E = {A: fooVal };");
    t.test_no_warning("let obj = {fooVal : 2}; /** @enum {number} */ let E = {A: obj.fooVal };");
    t.test_no_warning(
        "let obj = {fooVal : 2}; /** @enum {number} */ let E = {A: obj.fooVal + 1 };",
    );
    t.test_no_warning(
        "let obj = {fooVal : 2}; /** @enum {number} */ let E = {A: obj.fooVal - obj.fooVal};",
    );
    t.test_no_warning("function foo() {return 3;} /** @enum {number} */ let E = { A: foo() };");
}

// port: CheckEnumsTest#testDefaultEnums_noWarning
#[test]
fn test_default_enums_no_warning() {
    let t = case();
    // Default enums are of number type; we must not warn when non-statically initialized
    t.test_no_warning("let fooVal = 1; /** @enum */ let E = {A: fooVal };");
    t.test_no_warning("let obj = {fooVal : 2}; /** @enum */ let E = {A: obj.fooVal };");
    t.test_no_warning("function foo() {return 3;} /** @enum */ let E = { A: foo() };");
}

// port: CheckEnumsTest#testStringEnums_withArithmeticOperations
#[test]
fn test_string_enums_with_arithmetic_operations() {
    let t = case();
    // String enum values computed using arithmetic get reported
    t.test_warning(
        "/** @enum {string} */ let E = { A: 'a'+ 2};",
        &NON_STATIC_INITIALIZER_STRING_VALUE_IN_ENUM,
    );
    // 'a2'
    t.test_warning(
        "/** @enum {string} */ let E = { A: 'a'+ 'b' };",
        &NON_STATIC_INITIALIZER_STRING_VALUE_IN_ENUM,
    );
    // 'ab'
}

// port: CheckEnumsTest#testNonStaticStringValue_reportsWarning
#[test]
fn test_non_static_string_value_reports_warning() {
    let t = case();
    t.test_warning(
        "let fooVal = ''; /** @enum {string} */ let E = {A: fooVal };",
        &NON_STATIC_INITIALIZER_STRING_VALUE_IN_ENUM,
    );
    // non-statically initialized with name
    t.test_warning(
        r#"/** @return {string} */
function foo() {
  return ''
}
/** @enum {string} */
let E = { A: foo() };
"#,
        &NON_STATIC_INITIALIZER_STRING_VALUE_IN_ENUM,
    );
    // non-statically initialized with call
    // call to a function missing `@return` annotation
    t.test_warning(
        r#"function foo() {
  return ''
}
/** @enum {string} */
let E = { A: foo() };
"#,
        &NON_STATIC_INITIALIZER_STRING_VALUE_IN_ENUM,
    );
    // call to a tagged template function
    t.test_warning(
        r#"/** @return {string} */
function tag(x) {
  return x
}
/** @enum {string} */
let E = { A: tag`some` };
"#,
        &NON_STATIC_INITIALIZER_STRING_VALUE_IN_ENUM,
    );
    t.test_warning(
        "let obj = {fooVal : ''}; /** @enum {string} */ let E = {A: obj.fooVal };",
        &NON_STATIC_INITIALIZER_STRING_VALUE_IN_ENUM,
    );
    // non-statically initialized with GETPROP
    t.test_warning(
        "let obj = {fooVal : ''}; /** @enum {string} */ let E = {A: obj.fooVal + '' };",
        &NON_STATIC_INITIALIZER_STRING_VALUE_IN_ENUM,
    );
    // non-statically initialized with a
    // GETPROP operand in `+`
    t.test_warning(
        "/** @type {number} */ let num = 5; /** @enum {string} */ let E = { A: 'a'+ num };",
        &NON_STATIC_INITIALIZER_STRING_VALUE_IN_ENUM,
    );
    // non-statically initialized with var `num`.
    t.test_warning(
        "/** @type {number} */ let num = 5; /** @enum {string} */ let E = { A: `a${num}`};",
        &NON_STATIC_INITIALIZER_STRING_VALUE_IN_ENUM,
    );
    // non-statically initialized with
    // template lit substitution.
    t.test_warning(
        r#"/** @enum {string} */
let EnumOne = {  FOO: 'foo',  BAR: 'bar'};
/** @enum {string}*/
let EnumTwo = { FOO: EnumOne.FOO}
"#,
        &NON_STATIC_INITIALIZER_STRING_VALUE_IN_ENUM,
    );
    // using another string enum as value gets
    // reported
}

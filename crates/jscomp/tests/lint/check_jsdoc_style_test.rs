/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/lint/CheckJSDocStyleTest.java.

use super::support::LintTestCase;
use closure_jscomp::closure_coding_convention::ClosureCodingConvention;
use closure_jscomp::{
    check_level::CheckLevel,
    lint::check_jsdoc_style::{
        ALL_DIAGNOSTICS, CLASS_DISALLOWED_JSDOC, CheckJSDocStyle,
        EXTERNS_FILES_SHOULD_BE_ANNOTATED, INCORRECT_ANNOTATION_ON_GETTER_SETTER,
        INCORRECT_PARAM_NAME, LICENSE_CONTAINS_AT_EXTERNS, MISSING_JSDOC, MISSING_PARAMETER_JSDOC,
        MISSING_RETURN_JSDOC, MIXED_PARAM_JSDOC_STYLES, OPTIONAL_PARAM_NOT_MARKED_OPTIONAL,
        PREFER_BACKTICKS_TO_AT_SIGN_CODE, WRONG_NUMBER_OF_PARAMS,
    },
};
use closure_parsing::config::JsDocParsing;
use std::sync::Arc;

// port: CheckJSDocStyleTest#CheckJSDocStyleTest
// port: CheckJSDocStyleTest#setUp
// port: CheckJSDocStyleTest#getProcessor
// port: CheckJSDocStyleTest#getOptions
// port: CheckJSDocStyleTest#getCodingConvention
fn case() -> LintTestCase {
    // The fixture installs GoogleCodingConvention, as setUp does.
    case_with(false)
}

/// `codingConvention = new ClosureCodingConvention()` before the test runs.
fn case_with(closure_convention: bool) -> LintTestCase {
    let mut t =
        LintTestCase::new(|c| Box::new(CheckJSDocStyle::new(c))).with_options(move |options| {
            options
                .set_parse_js_doc_documentation(JsDocParsing::INCLUDE_DESCRIPTIONS_NO_WHITESPACE);
            options.set_warning_level(ALL_DIAGNOSTICS.clone(), CheckLevel::WARNING);
            if closure_convention {
                options.set_coding_convention(Arc::new(ClosureCodingConvention::new()));
            }
        });
    t.parse_js_doc_documentation = JsDocParsing::INCLUDE_DESCRIPTIONS_NO_WHITESPACE;
    t.externs = "/** @fileoverview\n * @externs\n */".to_string();
    t
}

// port: CheckJSDocStyleTest#inIIFE
fn in_iife(js: &str) -> String {
    format!("(function() {{\n{js}\n}})()")
}

// port: CheckJSDocStyleTest#testValidSuppress_onDeclaration
#[test]
fn test_valid_suppress_on_declaration() {
    let t = case();
    t.test_same("/** @const */ var global = this;");
    t.test_same("/** @const */ goog.global = this;");
}

// port: CheckJSDocStyleTest#testValidSuppress_withES6Modules01
#[test]
fn test_valid_suppress_with_es6_modules01() {
    let t = case();
    t.test_same("export /** @suppress {missingRequire} */ var x = new y.Z();");
}

// port: CheckJSDocStyleTest#testValidSuppress_withES6Modules03
#[test]
fn test_valid_suppress_with_es6_modules03() {
    let t = case();
    t.test_same("export /** @const @suppress {duplicate} */ var google = {};");
}

// port: CheckJSDocStyleTest#testExtraneousClassAnnotations
#[test]
fn test_extraneous_class_annotations() {
    let t = case();
    t.test_warning(
        r#"/**
 * @constructor
 */
var X = class {};
"#,
        &CLASS_DISALLOWED_JSDOC,
    );
    t.test_warning(
        r#"/**
 * @constructor
 */
class X {};
"#,
        &CLASS_DISALLOWED_JSDOC,
    );
    // TODO(tbreisacher): Warn for @extends too. We need to distinguish between cases like this
    // which are totally redundant...
    t.test_same(
        r#"/**
 * @extends {Y}
 */
class X extends Y {};
"#,
    );
    // ... and ones like this which are not.
    t.test_same(
        r#"/**
 * @extends {Y<number>}
 */
class X extends Y {};
"#,
    );
    t.test_same(
        r#"/**
 * @implements {Z}
 */
class X extends Y {};
"#,
    );
    t.test_same(
        r#"/**
 * @interface
 * @extends {Y}
 */
class X extends Y {};
"#,
    );
    t.test_same(
        r#"/**
 * @record
 * @extends {Y}
 */
class X extends Y {};
"#,
    );
}

// port: CheckJSDocStyleTest#testInvalidExtraneousClassAnnotations_withES6Modules
#[test]
fn test_invalid_extraneous_class_annotations_with_es6_modules() {
    let t = case();
    t.test_warning(
        r#"export
/**
 * @constructor
 */
var X = class {};
"#,
        &CLASS_DISALLOWED_JSDOC,
    );
}

// port: CheckJSDocStyleTest#testValidExtraneousClassAnnotations_withES6Modules
#[test]
fn test_valid_extraneous_class_annotations_with_es6_modules() {
    let t = case();
    t.test_same("export /** @extends {Y} */ class X extends Y {};");
}

// port: CheckJSDocStyleTest#testNestedArrowFunctions
#[test]
fn test_nested_arrow_functions() {
    let t = case();
    t.test_same(
        r#"/**
 * @param {Object} a
 * @return {function(Object): boolean}
 */
var haskellStyleEquals = a => b => a == b;
"#,
    );
}

// port: CheckJSDocStyleTest#testNestedArrowFunctions_withES6Modules
#[test]
fn test_nested_arrow_functions_with_es6_modules() {
    let t = case();
    t.test_same(
        r#"export
/**
 * @param {Object} a
 * @return {function(Object): boolean}
 */
var haskellStyleEquals = a => b => a == b;
"#,
    );
}

// port: CheckJSDocStyleTest#testGetterSetterMissingJsDoc
#[test]
fn test_getter_setter_missing_js_doc() {
    let t = case();
    t.test_warning(
        "class Foo { get twentyone() { return 21; } }",
        &MISSING_JSDOC,
    );
    t.test_warning(
        "class Foo { set someString(s) { this.someString_ = s; } }",
        &MISSING_JSDOC,
    );
    t.test_same("class Foo { /** @return {number} */ get twentyone() { return 21; } }");
    t.test_same(
        "class Foo { /** @param {string} s */ set someString(s) { this.someString_ = s; } }",
    );
}

// port: CheckJSDocStyleTest#testTypeAnnotationOnGetterSetter
#[test]
fn test_type_annotation_on_getter_setter() {
    let t = case();
    t.test_warning(
        "class Foo { /** @type {number} */ get twentyone() { return 21; } }",
        &INCORRECT_ANNOTATION_ON_GETTER_SETTER,
    );
    t.test_warning(
        "class Foo { /** @type {string} s */ set someString(s) { this.someString_ = s; } }",
        &INCORRECT_ANNOTATION_ON_GETTER_SETTER,
    );
    t.test_no_warning("class Foo { set someString( /** string */ s) { this.someString_ = s; } }");
}

// port: CheckJSDocStyleTest#testGetterSetter_withES6Modules
#[test]
fn test_getter_setter_with_es6_modules() {
    let t = case();
    t.test_same("export class Foo { /** @return {number} */ get twentyone() { return 21; } }");
}

// port: CheckJSDocStyleTest#testMissingJsDoc
#[test]
fn test_missing_js_doc() {
    let t = case();
    t.test_warning("function f() {}", &MISSING_JSDOC);
    t.test_warning("var f = function() {}", &MISSING_JSDOC);
    t.test_warning("let f = function() {}", &MISSING_JSDOC);
    t.test_warning("const f = function() {}", &MISSING_JSDOC);
    t.test_warning("foo.bar = function() {}", &MISSING_JSDOC);
    t.test_warning("Foo.prototype.bar = function() {}", &MISSING_JSDOC);
    t.test_warning("class Foo { bar() {} }", &MISSING_JSDOC);
    t.test_warning("class Foo { constructor(x) {} }", &MISSING_JSDOC);
    t.test_warning("var Foo = class { bar() {} };", &MISSING_JSDOC);
    t.test_warning("if (COMPILED) { var f = function() {}; }", &MISSING_JSDOC);
    t.test_warning("var f = async function() {};", &MISSING_JSDOC);
    t.test_warning("async function f() {};", &MISSING_JSDOC);
    t.test_warning("Polymer({ method() {} });", &MISSING_JSDOC);
    t.test_warning("Polymer({ method: function() {} });", &MISSING_JSDOC);
    t.test_same("/** @return {string} */ function f() {}");
    t.test_same("/** @return {string} */ var f = function() {}");
    t.test_same("/** @return {string} */ let f = function() {}");
    t.test_same("/** @return {string} */ const f = function() {}");
    t.test_same("/** @return {string} */ foo.bar = function() {}");
    t.test_same("/** @return {string} */ Foo.prototype.bar = function() {}");
    t.test_same("class Foo { /** @return {string} */ bar() {} }");
    t.test_same("class Foo { constructor(/** string */ x) {} }");
    t.test_same("var Foo = class { /** @return {string} */ bar() {} };");
    t.test_same("/** @param {string} s */ var f = async function(s) {};");
    t.test_same("/** @param {string} s */ async function f(s) {};");
    t.test_same("Polymer({ /** @return {null} */ method() {} });");
    t.test_same("Polymer({ /** @return {null} */ method: function() {} });");
}

// port: CheckJSDocStyleTest#testMissingJsDoc_withES6Modules01
#[test]
fn test_missing_js_doc_with_es6_modules01() {
    let t = case();
    t.test_warning("export function f() {}", &MISSING_JSDOC);
}

// port: CheckJSDocStyleTest#testMissingJsDoc_withES6Modules02
#[test]
fn test_missing_js_doc_with_es6_modules02() {
    let t = case();
    t.test_warning("export var f = function() {}", &MISSING_JSDOC);
}

// port: CheckJSDocStyleTest#testMissingJsDoc_withES6Modules03
#[test]
fn test_missing_js_doc_with_es6_modules03() {
    let t = case();
    t.test_warning("export let f = function() {}", &MISSING_JSDOC);
}

// port: CheckJSDocStyleTest#testMissingJsDoc_withES6Modules04
#[test]
fn test_missing_js_doc_with_es6_modules04() {
    let t = case();
    t.test_warning("export const f = function() {}", &MISSING_JSDOC);
}

// port: CheckJSDocStyleTest#testMissingJsDoc_withES6Modules09
#[test]
fn test_missing_js_doc_with_es6_modules09() {
    let t = case();
    t.test_warning("export var f = async function() {};", &MISSING_JSDOC);
}

// port: CheckJSDocStyleTest#testMissingJsDoc_noWarningIfInlineJsDocIsPresent
#[test]
fn test_missing_js_doc_no_warning_if_inline_js_doc_is_present() {
    let t = case();
    t.test_same("function /** string */ f() {}");
    t.test_same("function f(/** string */ x) {}");
    t.test_same("var f = function(/** string */ x) {}");
    t.test_same("let f = function(/** string */ x) {}");
    t.test_same("const f = function(/** string */ x) {}");
    t.test_same("foo.bar = function(/** string */ x) {}");
    t.test_same("Foo.prototype.bar = function(/** string */ x) {}");
    t.test_same("class Foo { bar(/** string */ x) {} }");
    t.test_same("var Foo = class { bar(/** string */ x) {} };");
}

// port: CheckJSDocStyleTest#testMissingJsDoc_noWarningIfInlineJsDocIsPresent_withES6Modules
#[test]
fn test_missing_js_doc_no_warning_if_inline_js_doc_is_present_with_es6_modules() {
    let t = case();
    t.test_same("export function /** string */ f() {}");
}

// port: CheckJSDocStyleTest#testMissingJsDoc_noWarningIfNotTopLevel
#[test]
fn test_missing_js_doc_no_warning_if_not_top_level() {
    let t = case();
    t.test_same(&in_iife("function f() {}"));
    t.test_same(&in_iife("var f = function() {}"));
    t.test_same(&in_iife("let f = function() {}"));
    t.test_same(&in_iife("const f = function() {}"));
    t.test_same(&in_iife("foo.bar = function() {}"));
    t.test_same(&in_iife("class Foo { bar() {} }"));
    t.test_same(&in_iife("var Foo = class { bar() {} };"));
    t.test_same("myArray.forEach(function(elem) { alert(elem); });");
    t.test_same(
        r#"Polymer({
  is: 'example-elem',
  /** @return {null} */
  someMethod: function() {},
});
"#,
    );
    t.test_same(
        r#"Polymer({
  is: 'example-elem',
  /** @return {null} */
  someMethod() {},
});
"#,
    );
}

// port: CheckJSDocStyleTest#testMissingJsDoc_noWarningIfNotTopLevelAndNoParams
#[test]
fn test_missing_js_doc_no_warning_if_not_top_level_and_no_params() {
    let t = case();
    t.test_same(
        r#"describe('a karma test', function() {
  /** @ngInject */
  var helperFunction = function($compile, $rootScope) {};
})
"#,
    );
}

// port: CheckJSDocStyleTest#testMissingJsDoc_noWarning_wizConstructorAndDeps
#[test]
fn test_missing_js_doc_no_warning_wiz_constructor_and_deps() {
    let t = case();
    // Exempt Wiz controller constructor and deps() method because Wiz automatically adds JSDoc
    // NOTE(lharker@): right now this does not warn because of b/124061048: the behavior is correct
    // but for the wrong reason.
    t.test_same(
        r#"goog.module('a.b.MyController');
class MyController extends SomeParentController {
  static deps() { return {model: 0}; }
  constructor({model}) {}
}
registerController(MY_CONTROLLER, MyController);
"#,
    );
}

// port: CheckJSDocStyleTest#testMissingJsDoc_noWarningOnTestFunctions
#[test]
fn test_missing_js_doc_no_warning_on_test_functions() {
    let t = case();
    t.test_same("function testSomeFunctionality() {}");
    t.test_same("var testSomeFunctionality = function() {};");
    t.test_same("let testSomeFunctionality = function() {};");
    t.test_same("window.testSomeFunctionality = function() {};");
    t.test_same("const testSomeFunctionality = function() {};");
    t.test_same("function setUp() {}");
    t.test_same("function tearDown() {}");
    t.test_same("var setUp = function() {};");
    t.test_same("var tearDown = function() {};");
}

// port: CheckJSDocStyleTest#testMissingJsDoc_noWarningOnTestMethods
#[test]
fn test_missing_js_doc_no_warning_on_test_methods() {
    let t = case();
    t.test_same("class MyClass { testSomeFunctionality() {} }");
    t.test_same("goog.module('mod'); class MyClass { testSomeFunctionality() {} }");
    t.test_same("a.b.c = class { testSomeFunctionality() {} }");
    t.test_same("class MyClass { setUp() {} }");
    t.test_same("class MyClass { tearDown() {} }");
}

// port: CheckJSDocStyleTest#testMissingJsDoc_noWarningOnTestFunctions_withES6Modules
#[test]
fn test_missing_js_doc_no_warning_on_test_functions_with_es6_modules() {
    let t = case();
    t.test_same("export function testSomeFunctionality() {}");
}

// port: CheckJSDocStyleTest#testMissingJsDoc_noWarningOnEmptyConstructor
#[test]
fn test_missing_js_doc_no_warning_on_empty_constructor() {
    let t = case();
    t.test_same("class Foo { constructor() {} }");
}

// port: CheckJSDocStyleTest#testMissingJsDoc_noWarningOnEmptyConstructor_withES6Modules
#[test]
fn test_missing_js_doc_no_warning_on_empty_constructor_with_es6_modules() {
    let t = case();
    t.test_same("export class Foo { constructor() {} }");
}

// port: CheckJSDocStyleTest#testMissingJsDoc_googModule
#[test]
fn test_missing_js_doc_goog_module() {
    let t = case();
    t.test_warning("goog.module('a.b.c'); function f() {}", &MISSING_JSDOC);
    t.test_warning(
        "goog.module('a.b.c'); var f = function() {};",
        &MISSING_JSDOC,
    );
    // TODO(b/124061048): these should also warn for missing JSDoc
    t.test_same("goog.module('a.b.c'); class Foo { constructor(x) {} }");
    t.test_same("goog.module('a.b.c'); class Foo { someMethod() {} }");
}

// port: CheckJSDocStyleTest#testMissingJsDoc_ES6Module01
#[test]
fn test_missing_js_doc_es6_module01() {
    let t = case();
    t.test_warning("export default abc; function f() {}", &MISSING_JSDOC);
}

// port: CheckJSDocStyleTest#testMissingJsDoc_ES6Module02
#[test]
fn test_missing_js_doc_es6_module02() {
    let t = case();
    t.test_warning("export default abc; var f = function() {};", &MISSING_JSDOC);
}

// port: CheckJSDocStyleTest#testMissingJsDoc_ES6Module03
#[test]
fn test_missing_js_doc_es6_module03() {
    let t = case();
    t.test_warning("export function f() {};", &MISSING_JSDOC);
}

// port: CheckJSDocStyleTest#testMissingJsDoc_ES6Module04
#[test]
fn test_missing_js_doc_es6_module04() {
    let t = case();
    t.test_warning("export default function () {}", &MISSING_JSDOC);
}

// port: CheckJSDocStyleTest#testMissingJsDoc_ES6Module05
#[test]
fn test_missing_js_doc_es6_module05() {
    let t = case();
    t.test_warning("export default (foo) => { alert(foo); }", &MISSING_JSDOC);
}

// port: CheckJSDocStyleTest#testMissingJsDoc_googModule_noWarning
#[test]
fn test_missing_js_doc_goog_module_no_warning() {
    let t = case();
    t.test_same("goog.module('a.b.c'); /** @type {function()} */ function f() {}");
    t.test_same("goog.module('a.b.c'); /** @type {function()} */ var f = function() {};");
    // No param constructors do not require JSDoc
    t.test_same("goog.module('a.b.c'); class Foo { constructor() {} }");
}

// port: CheckJSDocStyleTest#testMissingJsDoc_ES6Module_noWarning01
#[test]
fn test_missing_js_doc_es6_module_no_warning01() {
    let t = case();
    t.test_same("export default abc; /** @type {function()} */ function f() {}");
}

// port: CheckJSDocStyleTest#testMissingJsDoc_ES6Module_noWarning02
#[test]
fn test_missing_js_doc_es6_module_no_warning02() {
    let t = case();
    t.test_same("export default abc; /** @type {function()} */ var f = function() {};");
}

// port: CheckJSDocStyleTest#testMissingParam_noWarning
#[test]
fn test_missing_param_no_warning() {
    let t = case();
    t.test_same(
        r#"/**
 * @param {string} x
 * @param {string} y
 */
function f(x, y) {}
"#,
    );
    t.test_same(
        r#"/**
 * @param {string=} x
 */
function f(x = 1) {}
"#,
    );
    t.test_same(
        r#"/**
 * @param {number=} x
 * @param {number=} y
 * @param {number=} z
 */
function f(x = 1, y = 2, z = 3) {}
"#,
    );
    t.test_same(
        r#"/**
 * @param {...string} args
 */
function f(...args) {}
"#,
    );
    t.test_same(
        r#"(function() {
  myArray.forEach(function(elem) { alert(elem); });
})();
"#,
    );
    t.test_same(
        r#"(function() {
  myArray.forEach(elem => alert(elem));
})();
"#,
    );
    t.test_same("/** @type {function(number)} */ function f(x) {}");
    t.test_same("function f(/** string */ inlineArg) {}");
    t.test_same("/** @export */ function f(/** string */ inlineArg) {}");
    t.test_same("class Foo { constructor(/** string */ inlineArg) {} }");
    t.test_same("class Foo { method(/** string */ inlineArg) {} }");
    t.test_same("/** @export */ class Foo { constructor(/** string */ inlineArg) {} }");
    t.test_same("class Foo { /** @export */ method(/** string */ inlineArg) {} }");
}

// port: CheckJSDocStyleTest#testMissingParam_noWarning_withES6Modules
#[test]
fn test_missing_param_no_warning_with_es6_modules() {
    let t = case();
    t.test_same("export class Foo { /** @export */ method(/** string */ inlineArg) {} }");
}

// port: CheckJSDocStyleTest#testMissingParam
#[test]
fn test_missing_param() {
    let t = case();
    t.test_warning(
        r#"/**
 * @param {string} x
// No @param for y.
 */
function f(x, y) {}
"#,
        &WRONG_NUMBER_OF_PARAMS,
    );
    t.test_warning(
        r#"/**
 * @param {string} x
 */
function f(x = 1) {}
"#,
        &OPTIONAL_PARAM_NOT_MARKED_OPTIONAL,
    );
    t.test_warning(
        r#"/**
 * @param {string} x
// No @param for y.
 */
function f(x, y = 1) {}
"#,
        &WRONG_NUMBER_OF_PARAMS,
    );
    t.test_warning(
        "function f(/** string */ x, y) {}",
        &MISSING_PARAMETER_JSDOC,
    );
    t.test_warning(
        "function f(x, /** string */ y) {}",
        &MISSING_PARAMETER_JSDOC,
    );
    t.test_warning("function /** string */ f(x) {}", &MISSING_PARAMETER_JSDOC);
    t.test_warning(
        &in_iife("function f(/** string */ x, y) {}"),
        &MISSING_PARAMETER_JSDOC,
    );
    t.test_warning(
        &in_iife("function f(x, /** string */ y) {}"),
        &MISSING_PARAMETER_JSDOC,
    );
    t.test_warning(
        &in_iife("function /** string */ f(x) {}"),
        &MISSING_PARAMETER_JSDOC,
    );
}

// port: CheckJSDocStyleTest#testMissingParam_withES6Modules01
#[test]
fn test_missing_param_with_es6_modules01() {
    let t = case();
    t.test_warning(
        r#"export
/**
 * @param {string} x
// No @param for y.
 */
function f(x, y) {}
"#,
        &WRONG_NUMBER_OF_PARAMS,
    );
}

// port: CheckJSDocStyleTest#testMissingParam_withES6Modules02
#[test]
fn test_missing_param_with_es6_modules02() {
    let t = case();
    t.test_warning(
        "export /** @param {string} x */ function f(x = 1) {}",
        &OPTIONAL_PARAM_NOT_MARKED_OPTIONAL,
    );
}

// port: CheckJSDocStyleTest#testMissingParam_withES6Modules03
#[test]
fn test_missing_param_with_es6_modules03() {
    let t = case();
    t.test_warning(
        "export function f(/** string */ x, y) {}",
        &MISSING_PARAMETER_JSDOC,
    );
}

// port: CheckJSDocStyleTest#testMissingParamWithDestructuringPattern
#[test]
fn test_missing_param_with_destructuring_pattern() {
    let t = case();
    t.test_warning(
        r#"/**
 * @param {string} namedParam
 * @return {void}
 */
function f(namedParam, {destructuring:pattern}) {
}
"#,
        &WRONG_NUMBER_OF_PARAMS,
    );
    t.test_warning(
        r#"/**
 * @param {string} namedParam
 * @return {void}
 */
function f({destructuring:pattern}, namedParam) {
}
"#,
        &WRONG_NUMBER_OF_PARAMS,
    );
    t.test_warning(
        r#"/**
 * @param {string} namedParam
 * @return {void}
 */
function f(namedParam, [pattern]) {
}
"#,
        &WRONG_NUMBER_OF_PARAMS,
    );
    t.test_warning(
        r#"/**
 * @param {string} namedParam
 * @return {void}
 */
function f([pattern], namedParam) {
}
"#,
        &WRONG_NUMBER_OF_PARAMS,
    );
    t.test_warning(
        r#"/**
 * @param {{
 *   a: (string|undefined),
 *   b: (number|undefined),
 *   c: (boolean|undefined)
 * }} obj
 */
function create({a = 'hello', b = 8, c = false} = {}) {}
"#,
        &OPTIONAL_PARAM_NOT_MARKED_OPTIONAL,
    );
    // Same as above except there's an '=' to indicate that it's optional.
    t.test_same(
        r#"/**
 * @param {{
 *   a: (string|undefined),
 *   b: (number|undefined),
 *   c: (boolean|undefined)
 * }=} obj
 */
function create({a = 'hello', b = 8, c = false} = {}) {}
"#,
    );
}

// port: CheckJSDocStyleTest#testMissingParam_defaultValue
#[test]
fn test_missing_param_default_value() {
    let t = case();
    t.test_warning(
        r#"/**
 * @param {string} x
// No @param for y.
 */
function f(x, y = 0) {}
"#,
        &WRONG_NUMBER_OF_PARAMS,
    );
    t.test_warning(
        r#"/**
 * @param {string} x
 * @param {number} y
 */
function f(x, y = 0) {}
"#,
        &OPTIONAL_PARAM_NOT_MARKED_OPTIONAL,
    );
    t.test_no_warning(
        r#"/**
 * @param {string} x
 * @param {number=} y
 */
function f(x, y = 0) {}
"#,
    );
    t.test_warning(
        "function f(/** string */ x, y = 0) {}",
        &MISSING_PARAMETER_JSDOC,
    );
    t.test_warning(
        "function f(/** string */ x, /** number */ y = 0) {}",
        &OPTIONAL_PARAM_NOT_MARKED_OPTIONAL,
    );
    t.test_no_warning("function f(/** string */ x, /** number= */ y = 0) {}");
}

// port: CheckJSDocStyleTest#testMissingParam_rest
#[test]
fn test_missing_param_rest() {
    let t = case();
    t.test_warning(
        r#"/**
 * @param {string} x
// No @param for y.
 */
function f(x, ...y) {}
"#,
        &WRONG_NUMBER_OF_PARAMS,
    );
    t.test_no_warning(
        r#"/**
 * @param {string} x
 * @param {...number} y
 */
function f(x, ...y) {}
"#,
    );
    t.test_warning(
        "function f(/** string */ x, ...y) {}",
        &MISSING_PARAMETER_JSDOC,
    );
    t.test_no_warning("function f(/** string */ x, /** ...number */ ...y) {}");
}

// port: CheckJSDocStyleTest#testInvalidMissingParamWithDestructuringPattern_withES6Modules01
#[test]
fn test_invalid_missing_param_with_destructuring_pattern_with_es6_modules01() {
    let t = case();
    t.test_warning(
        r#"export
/**
 * @param {string} namedParam
 * @return {void}
 */
function f(namedParam, {destructuring:pattern}) {
}
"#,
        &WRONG_NUMBER_OF_PARAMS,
    );
}

// port: CheckJSDocStyleTest#testInvalidMissingParamWithDestructuringPattern_withES6Modules02
#[test]
fn test_invalid_missing_param_with_destructuring_pattern_with_es6_modules02() {
    let t = case();
    t.test_warning(
        r#"export
/**
 * @param {{
 *   a: (string|undefined),
 *   b: (number|undefined),
 *   c: (boolean|undefined)
 * }} obj
 */
function create({a = 'hello', b = 8, c = false} = {}) {}
"#,
        &OPTIONAL_PARAM_NOT_MARKED_OPTIONAL,
    );
}

// port: CheckJSDocStyleTest#testValidMissingParamWithDestructuringPattern_withES6Modules
#[test]
fn test_valid_missing_param_with_destructuring_pattern_with_es6_modules() {
    let t = case();
    t.test_same(
        r#"export
/**
 * @param {{
 *   a: (string|undefined),
 *   b: (number|undefined),
 *   c: (boolean|undefined)
 * }=} obj
 */
function create({a = 'hello', b = 8, c = false} = {}) {}
"#,
    );
}

// port: CheckJSDocStyleTest#testMissingParamWithDestructuringPatternWithDefault
#[test]
fn test_missing_param_with_destructuring_pattern_with_default() {
    let t = case();
    t.test_warning(
        r#"/**
 * @param {string} namedParam
 * @return {void}
 */
function f(namedParam, {destructuring:pattern} = defaultValue) {
}
"#,
        &WRONG_NUMBER_OF_PARAMS,
    );
    t.test_warning(
        r#"/**
 * @param {string} namedParam
 * @return {void}
 */
function f(namedParam, [pattern] = defaultValue) {
}
"#,
        &WRONG_NUMBER_OF_PARAMS,
    );
}

// port: CheckJSDocStyleTest#testMissingParamWithDestructuringPatternWithDefault_withES6Modules
#[test]
fn test_missing_param_with_destructuring_pattern_with_default_with_es6_modules() {
    let t = case();
    t.test_warning(
        r#"export
/**
 * @param {string} namedParam
 * @return {void}
 */
function f(namedParam, {destructuring:pattern} = defaultValue) {
}
"#,
        &WRONG_NUMBER_OF_PARAMS,
    );
}

// port: CheckJSDocStyleTest#testParamWithNoTypeInfo
#[test]
fn test_param_with_no_type_info() {
    let t = case();
    t.test_same(
        r#"/**
 * @param x A param with no type information.
 */
function f(x) { }
"#,
    );
}

// port: CheckJSDocStyleTest#testParamWithNoTypeInfo_optional
#[test]
fn test_param_with_no_type_info_optional() {
    let t = case();
    t.test_warning(
        r#"/**
 * @param x A param with no type information.
 */
function f(x = undefined) { }
"#,
        &OPTIONAL_PARAM_NOT_MARKED_OPTIONAL,
    );
}

// port: CheckJSDocStyleTest#testParamWithNoTypeInfo_withES6Modules
#[test]
fn test_param_with_no_type_info_with_es6_modules() {
    let t = case();
    t.test_same(
        r#"export
/**
 * @param x A param with no type information.
 */
function f(x) { }
"#,
    );
}

// port: CheckJSDocStyleTest#testMissingPrivate_noWarningWithClosureConvention
#[test]
fn test_missing_private_no_warning_with_closure_convention() {
    let t = case_with(true);
    t.test_same(
        r#"/**
 * @return {number}
 * @private
 */
X.prototype.foo = function() { return 0; }
"#,
    );
}

// port: CheckJSDocStyleTest#testMissingPrivate
#[test]
fn test_missing_private() {
    let t = case();
    t.test_same(
        r#"/**
 * @return {number}
 * @private
 */
X.prototype.foo_ = function() { return 0; }
"#,
    );
    t.test_same(
        r#"/**
 * @type {number}
 * @private
 */
X.prototype.foo_ = 0;
"#,
    );
    t.test_same(
        r#"/** @type {number} */
X.prototype['@some_special_property'] = 0;
"#,
    );
}

// port: CheckJSDocStyleTest#testNoPrivateWarningsWithSuppressions
#[test]
fn test_no_private_warnings_with_suppressions() {
    let t = case();
    t.test_no_warning(
        r#"goog.module('mod');
class Foo {
  constructor() {
    /** @private {number} */
    this.n_;
    /** @private {number} */
    this.m_;
  }
  setUp() {
    /** @suppress {checkTypes} */
    this.n_ = ' not a number ';
    this.m_ = 1;
  }
  testSomething() {
    alert(this.n_ + this.m_);
  }
}
"#,
    );
}

// port: CheckJSDocStyleTest#testMissingPrivate_dontWarnOnObjectLiteral
#[test]
fn test_missing_private_dont_warn_on_object_literal() {
    let t = case();
    t.test_same(
        r#"var obj = {
  /** @return {number} */
  foo_() { return 0; }
}
"#,
    );
}

// port: CheckJSDocStyleTest#testMissingPrivate_dontWarnOnObjectLiteral_withES6Modules
#[test]
fn test_missing_private_dont_warn_on_object_literal_with_es6_modules() {
    let t = case();
    t.test_same("export var obj = { /** @return {number} */ foo_() { return 0; } }");
}

// port: CheckJSDocStyleTest#testOptionalArgs
#[test]
fn test_optional_args() {
    let t = case();
    t.test_same(
        r#"/**
 * @param {number=} n
 */
function f(n) {}
"#,
    );
    t.test_same_warning(
        r#"/**
 * @param {number} opt_n
 */
function f(opt_n) {}
"#,
        &OPTIONAL_PARAM_NOT_MARKED_OPTIONAL,
    );
    t.test_same(
        r#"/**
 * @param {number=} opt_n
 */
function f(opt_n) {}
"#,
    );
}

// port: CheckJSDocStyleTest#testValidOptionalArgs_withES6Modules
#[test]
fn test_valid_optional_args_with_es6_modules() {
    let t = case();
    t.test_same("export /** @param {number=} n */ function f(n) {}");
}

// port: CheckJSDocStyleTest#testInvalidOptionalArgs_withES6Modules
#[test]
fn test_invalid_optional_args_with_es6_modules() {
    let t = case();
    t.test_same_warning(
        "export /** @param {number} opt_n */ function f(opt_n) {}",
        &OPTIONAL_PARAM_NOT_MARKED_OPTIONAL,
    );
}

// port: CheckJSDocStyleTest#testParamsOutOfOrder
#[test]
fn test_params_out_of_order() {
    let t = case();
    t.test_warning(
        r#"/**
 * @param {?} second
 * @param {?} first
 */
function f(first, second) {}
"#,
        &INCORRECT_PARAM_NAME,
    );
}

// port: CheckJSDocStyleTest#testParamsOutOfOrder_withES6Modules
#[test]
fn test_params_out_of_order_with_es6_modules() {
    let t = case();
    t.test_warning(
        r#"export
/**
 * @param {?} second
 * @param {?} first
 */
function f(first, second) {}
"#,
        &INCORRECT_PARAM_NAME,
    );
}

// port: CheckJSDocStyleTest#testMixedStyles
#[test]
fn test_mixed_styles() {
    let t = case();
    t.test_warning(
        r#"/**
 * @param {?} first
 * @param {string} second
 */
function f(first, /** string */ second) {}
"#,
        &MIXED_PARAM_JSDOC_STYLES,
    );
}

// port: CheckJSDocStyleTest#testMixedStyles_withES6Modules
#[test]
fn test_mixed_styles_with_es6_modules() {
    let t = case();
    t.test_warning(
        r#"export
/**
 * @param {?} first
 * @param {string} second
 */
function f(first, /** string */ second) {}
"#,
        &MIXED_PARAM_JSDOC_STYLES,
    );
}

// port: CheckJSDocStyleTest#testDestructuring
#[test]
fn test_destructuring() {
    let t = case();
    t.test_same(
        r#"/**
 * @param {{x: number, y: number}} point
 */
function getDistanceFromZero({x, y}) {}
"#,
    );
    t.test_same("function getDistanceFromZero(/** {x: number, y: number} */ {x, y}) {}");
}

// port: CheckJSDocStyleTest#testDestructuring_withES6Modules
#[test]
fn test_destructuring_with_es6_modules() {
    let t = case();
    t.test_same("export function getDistanceFromZero(/** {x: number, y: number} */ {x, y}) {}");
}

// port: CheckJSDocStyleTest#testMissingReturn_functionStatement_noWarning
#[test]
fn test_missing_return_function_statement_no_warning() {
    let t = case();
    t.test_same("/** @param {number} x */ function f(x) {}");
    t.test_same("/** @constructor */ function f() {}");
    t.test_same("/** @param {number} x */ function f(x) { function bar() { return x; } }");
    t.test_same("/** @param {number} x */ function f(x) { return; }");
    t.test_same(
        r#"/** @param {number} x
 * @return {number} */ function f(x) { return x; }"#,
    );
    t.test_same("/** @param {number} x */ function /** number */ f(x) { return x; }");
}

// port: CheckJSDocStyleTest#testMissingReturn_functionStatement_noWarning_withES6Modules
#[test]
fn test_missing_return_function_statement_no_warning_with_es6_modules() {
    let t = case();
    t.test_same("export /** @param {number} x */ function f(x) {}");
}

// port: CheckJSDocStyleTest#testMissingReturn_assign_noWarning
#[test]
fn test_missing_return_assign_no_warning() {
    let t = case();
    t.test_same("/** @param {number} x */ f = function(x) {}");
    t.test_same("/** @constructor */ f = function() {}");
    t.test_same("/** @param {number} x */ f = function(x) { function bar() { return x; } }");
    t.test_same("/** @param {number} x */ f = function(x) { return; }");
    t.test_same(
        r#"/** @param {number} x
 * @return {number} */ f = function(x) { return x; }"#,
    );
}

// port: CheckJSDocStyleTest#testMissingParamOrReturn_warnOnOverrideMethodsAndFields
#[test]
fn test_missing_param_or_return_warn_on_override_methods_and_fields() {
    let t = case();
    t.test_warning(
        r#"/**
 * @override
 * @param {string} x
 */
 Foo.Bar = function(x) { return x; }
"#,
        &MISSING_RETURN_JSDOC,
    );
    t.test_warning(
        r#"/**
 * @override
 * @param {string} x
 */
 function f(x) { return x; }
"#,
        &MISSING_RETURN_JSDOC,
    );
    t.test_warning(
        r#"/**
 * @override
 * @return {string}
 */
 Foo.Bar = function(x) { return x; }
"#,
        &MISSING_PARAMETER_JSDOC,
    );
    t.test_warning(
        r#"/**
 * @override
 * @param {string} x
 */
Foo.bar = function(x, y) {}
"#,
        &WRONG_NUMBER_OF_PARAMS,
    );
    // also test `@inheritDoc` annotations
    t.test_warning(
        r#"/**
 * @inheritDoc
 * @return {string} x
 */
function f(x) { return x; }
"#,
        &MISSING_PARAMETER_JSDOC,
    );
    t.test_warning(
        r#"/**
 * @inheritDoc
 * @return {string} x
 */
Foo.Bar = function(x) { return x; }
"#,
        &MISSING_PARAMETER_JSDOC,
    );
    // inline param type
    t.test_no_warning(
        r#"/**
 * @override
 * @return {string}
 */
var f = function(/** string */ x) { return x; }
"#,
    );
}

// port: CheckJSDocStyleTest#testMissingReturn_var_noWarning
#[test]
fn test_missing_return_var_no_warning() {
    let t = case();
    t.test_same("/** @param {number} x */ var f = function(x) {}");
    t.test_same("/** @constructor */ var f = function() {}");
    t.test_same("/** @param {number} x */ var f = function(x) { function bar() { return x; } }");
    t.test_same("/** @param {number} x */ var f = function(x) { return; }");
    t.test_same(
        r#"/** @param {number} x
 * @return {number} */ var f = function(x) { return x; }"#,
    );
    t.test_same("/** @const {function(number): number} */ var f = function(x) { return x; }");
}

// port: CheckJSDocStyleTest#testMissingReturn_constructor_noWarning
#[test]
fn test_missing_return_constructor_no_warning() {
    let t = case();
    t.test_same("/** @constructor */ var C = function() { return null; }");
}

// port: CheckJSDocStyleTest#testMissingReturn_class_constructor_noWarning
#[test]
fn test_missing_return_class_constructor_no_warning() {
    let t = case();
    t.test_same("class C { /** @param {Array} x */ constructor(x) { return x; } }");
}

// port: CheckJSDocStyleTest#testMissingReturn_var_noWarning_withES6Modules
#[test]
fn test_missing_return_var_no_warning_with_es6_modules() {
    let t = case();
    t.test_same("export /** @param {number} x */ var f = function(x) {}");
}

// port: CheckJSDocStyleTest#testMissingReturn_functionStatement
#[test]
fn test_missing_return_function_statement() {
    let t = case();
    t.test_warning(
        "/** @param {number} x */ function f(x) { return x; }",
        &MISSING_RETURN_JSDOC,
    );
    t.test_warning(
        r#"/** @param {number} x */
function f(x) {
  /** @param {number} x */
  function bar(x) {
    return x;
  }
}
"#,
        &MISSING_RETURN_JSDOC,
    );
    t.test_warning(
        "/** @param {number} x */ function f(x) { if (true) { return x; } }",
        &MISSING_RETURN_JSDOC,
    );
    t.test_warning(
        "/** @param {number} x @constructor */ function f(x) { return x; }",
        &MISSING_RETURN_JSDOC,
    );
}

// port: CheckJSDocStyleTest#testMissingReturn_functionStatement_withES6Modules
#[test]
fn test_missing_return_function_statement_with_es6_modules() {
    let t = case();
    t.test_warning(
        "export /** @param {number} x */ function f(x) { return x; }",
        &MISSING_RETURN_JSDOC,
    );
}

// port: CheckJSDocStyleTest#testMissingReturn_assign
#[test]
fn test_missing_return_assign() {
    let t = case();
    t.test_warning(
        "/** @param {number} x */ f = function(x) { return x; }",
        &MISSING_RETURN_JSDOC,
    );
    t.test_warning(
        r#"/** @param {number} x */
function f(x) {
  /** @param {number} x */
  bar = function(x) {
    return x;
  }
}
"#,
        &MISSING_RETURN_JSDOC,
    );
    t.test_warning(
        "/** @param {number} x */ f = function(x) { if (true) { return x; } }",
        &MISSING_RETURN_JSDOC,
    );
    t.test_warning(
        "/** @param {number} x @constructor */ f = function(x) { return x; }",
        &MISSING_RETURN_JSDOC,
    );
}

// port: CheckJSDocStyleTest#testMissingReturn_assign_withES6Modules
#[test]
fn test_missing_return_assign_with_es6_modules() {
    let t = case();
    t.test_warning(
        r#"/** @param {number} x */
export
function f(x) {
  /** @param {number} x */
  bar = function(x) {
    return x;
  }
}
"#,
        &MISSING_RETURN_JSDOC,
    );
}

// port: CheckJSDocStyleTest#testMissingReturn_var
#[test]
fn test_missing_return_var() {
    let t = case();
    t.test_warning(
        "/** @param {number} x */ var f = function(x) { return x; }",
        &MISSING_RETURN_JSDOC,
    );
    t.test_warning(
        r#"/** @param {number} x */
function f(x) {
  /** @param {number} x */
  var bar = function(x) {
    return x;
  }
}
"#,
        &MISSING_RETURN_JSDOC,
    );
    t.test_warning(
        "/** @param {number} x */ var f = function(x) { if (true) { return x; } }",
        &MISSING_RETURN_JSDOC,
    );
    t.test_warning(
        "/** @param {number} x @constructor */ var f = function(x) { return x; }",
        &MISSING_RETURN_JSDOC,
    );
}

// port: CheckJSDocStyleTest#testMissingReturn_var_withES6Modules
#[test]
fn test_missing_return_var_with_es6_modules() {
    let t = case();
    t.test_warning(
        "export /** @param {number} x */ var f = function(x) { return x; }",
        &MISSING_RETURN_JSDOC,
    );
}

// port: CheckJSDocStyleTest#testExternsAnnotation
#[test]
fn test_externs_annotation() {
    let t = case();
    t.test_externs_warnings(
        "function Example() {}",
        &[""],
        &[&EXTERNS_FILES_SHOULD_BE_ANNOTATED],
    );
    t.test_same_externs(
        r#"/** @fileoverview Some super cool externs.
 * @externs
 */ function Example() {}"#,
        &[""],
    );
    t.test_same_externs(
        r#"/** @fileoverview Some super cool externs.
 * @externs
 */
/** @constructor */ function Example() {}
/** @param {number} x */ function example2(x) {}
"#,
        &[""],
    );
    t.test_sources(
        &[
            r#"/** @fileoverview Some externs.
 * @externs
 */ /** @const */ var example;"#,
            r#"/** @fileoverview Some more.
 * @externs
 */ /** @const */ var example2;"#,
        ],
        &[""],
    );
}

// port: CheckJSDocStyleTest#testInvalidExternsAnnotation_withES6Modules
#[test]
fn test_invalid_externs_annotation_with_es6_modules() {
    let t = case();
    t.test_externs_warnings(
        "export function Example() {}",
        &[""],
        &[&EXTERNS_FILES_SHOULD_BE_ANNOTATED],
    );
}

// port: CheckJSDocStyleTest#testValidExternsAnnotation_withES6Modules
#[test]
fn test_valid_externs_annotation_with_es6_modules() {
    let t = case();
    t.test_same_externs(
        r#"export /** @fileoverview Some super cool externs.
 * @externs
 */
function Example() {}
"#,
        &[""],
    );
}

// port: CheckJSDocStyleTest#testValidLicenseCommentWithoutExterns
#[test]
fn test_valid_license_comment_without_externs() {
    let t = case();
    t.test_same_sources(&[r#"/**
 * @license
 * Copyright 2024 Google LLC
 */

function Example() {}
"#]);
}

// port: CheckJSDocStyleTest#testValidLicenseCommentAndExterns_separateBlocks
#[test]
fn test_valid_license_comment_and_externs_separate_blocks() {
    let t = case();
    t.test_same_externs(
        r#"/**
 * @license
 * Copyright 2024 Google LLC
 */
/**
 * @fileoverview Some super cool externs.
 * @externs
 */

function Example() {}
"#,
        &[""],
    );
}

// port: CheckJSDocStyleTest#testValidLicenseCommentAndExterns_sameBlocks
#[test]
fn test_valid_license_comment_and_externs_same_blocks() {
    let t = case();
    t.test_same_externs(
        r#"/**
 * @fileoverview Some super cool externs.
 * @externs
 * @license
 * Copyright 2024 Google LLC
 */

function Example() {}
"#,
        &[""],
    );
}

// port: CheckJSDocStyleTest#testInvalidLicenseComment_containsExterns
#[test]
fn test_invalid_license_comment_contains_externs() {
    let t = case();
    t.test_warnings_sources(
        &[r#"/**
 * @license
 * Copyright 2024 Google LLC
 * @externs - oh no, this tag is treated as part of the @license!
 */

function Example() {}
"#],
        &[&LICENSE_CONTAINS_AT_EXTERNS],
    );
}

// port: CheckJSDocStyleTest#testAtSignCodeDetectedWhenPresent
#[test]
fn test_at_sign_code_detected_when_present() {
    let t = case();
    t.test_warning(
        "/** blah blah {@code blah blah} blah blah */ function f() {}",
        &PREFER_BACKTICKS_TO_AT_SIGN_CODE,
    );
}

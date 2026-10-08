/*
 * Copyright 2018 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/lint/CheckNullabilityModifiersTest.java.

use super::support::LintTestCase;
use closure_jscomp::lint::check_nullability_modifiers::{
    CheckNullabilityModifiers, MISSING_NULLABILITY_MODIFIER_JSDOC,
    NULL_MISSING_NULLABILITY_MODIFIER_JSDOC, REDUNDANT_NULLABILITY_MODIFIER_JSDOC,
};

// port: CheckNullabilityModifiersTest#getProcessor
// port: CheckNullabilityModifiersTest#getOptions
fn case() -> LintTestCase {
    LintTestCase::new(|c| Box::new(CheckNullabilityModifiers::new(c))).with_lint_checks_warning()
}

// port: CheckNullabilityModifiersTest#checkNoWarning
fn check_no_warning(t: &LintTestCase, js: &[&str]) {
    t.test_same_sources(js);
}

// port: CheckNullabilityModifiersTest#checkMissingWarning
fn check_missing_warning(t: &LintTestCase, js: &[&str]) {
    t.test_warning_sources(js, &MISSING_NULLABILITY_MODIFIER_JSDOC);
}

// port: CheckNullabilityModifiersTest#checkNullMissingWarning
fn check_null_missing_warning(t: &LintTestCase, js: &[&str]) {
    t.test_warning_sources(js, &NULL_MISSING_NULLABILITY_MODIFIER_JSDOC);
}

// port: CheckNullabilityModifiersTest#checkRedundantWarning
fn check_redundant_warning(t: &LintTestCase, js: &[&str]) {
    t.test_warning_sources(js, &REDUNDANT_NULLABILITY_MODIFIER_JSDOC);
}

// port: CheckNullabilityModifiersTest#testPrimitiveType
#[test]
fn test_primitive_type() {
    let t = case();
    check_redundant_warning(&t, &["/** @type {!boolean} */ var x;"]);
    check_redundant_warning(&t, &["/** @type {!number} */ var x;"]);
    check_redundant_warning(&t, &["/** @type {!bigint} */ var x;"]);
    check_redundant_warning(&t, &["/** @type {!string} */ var x;"]);
    check_redundant_warning(&t, &["/** @type {!symbol} */ var x;"]);
    check_redundant_warning(&t, &["/** @type {!undefined} */ var x;"]);
    check_redundant_warning(&t, &["/** @type {!void} */ var x;"]);
    check_no_warning(&t, &["/** @type {?boolean} */ var x;"]);
    check_no_warning(&t, &["/** @type {boolean} */ var x;"]);
}

// port: CheckNullabilityModifiersTest#testReferenceType
#[test]
fn test_reference_type() {
    let t = case();
    check_missing_warning(&t, &["/** @type {Object} */ var x;"]);
    check_missing_warning(&t, &["/** @type {Function} */ var x;"]);
    check_missing_warning(&t, &["/** @type {Symbol} */ var x;"]);
    check_no_warning(&t, &["/** @type {?Object} */ var x;"]);
    check_no_warning(&t, &["/** @type {!Object} */ var x;"]);
}

// port: CheckNullabilityModifiersTest#testRecordType
#[test]
fn test_record_type() {
    let t = case();
    check_redundant_warning(&t, &["/** @type {!{foo: string}} */ var x;"]);
    check_redundant_warning(&t, &["/** @type {{foo: !string}} */ var x;"]);
    check_missing_warning(&t, &["/** @type {{foo: Object}} */ var x;"]);
    check_no_warning(&t, &["/** @type {?{foo: string}} */ var x;"]);
    check_no_warning(&t, &["/** @type {{foo: string}} */ var x;"]);
    check_no_warning(&t, &["/** @type {{foo: ?string}} */ var x;"]);
    check_no_warning(&t, &["/** @type {{foo: !Object}} */ var x;"]);
    check_no_warning(&t, &["/** @type {{foo: ?Object}} */ var x;"]);
}

// port: CheckNullabilityModifiersTest#testFunctionType
#[test]
fn test_function_type() {
    let t = case();
    check_redundant_warning(&t, &["/** @type {!function()} */ function f(){}"]);
    check_redundant_warning(&t, &["/** @type {function(!string)} */ function f(x){}"]);
    check_redundant_warning(&t, &["/** @type {function(): !string} */ function f(x){}"]);
    check_missing_warning(&t, &["/** @type {function(Object)} */ function f(x){}"]);
    check_missing_warning(&t, &["/** @type {function(): Object} */ function f(x){}"]);
    check_no_warning(&t, &["/** @type {function()} */ function f(){}"]);
    check_no_warning(
        &t,
        &["/** @type {function(?string): ?string} */ function f(x){}"],
    );
    check_no_warning(
        &t,
        &["/** @type {function(string): string} */ function f(x){}"],
    );
    check_no_warning(
        &t,
        &["/** @type {function(!Object): ?Object} */ function f(x){}"],
    );
    check_no_warning(&t, &["/** @type {function(new:Object)} */ function f(){}"]);
    check_no_warning(&t, &["/** @type {function(this:Object)} */ function f(){}"]);
}

// port: CheckNullabilityModifiersTest#testUnionType
#[test]
fn test_union_type() {
    let t = case();
    check_redundant_warning(&t, &["/** @type {!Object|!string} */ var x;"]);
    check_missing_warning(&t, &["/** @type {Object|string} */ var x;"]);
    check_no_warning(&t, &["/** @type {?Object|string} */ var x;"]);
    check_no_warning(&t, &["/** @type {!Object|string} */ var x;"]);
}

// port: CheckNullabilityModifiersTest#testEnumType
#[test]
fn test_enum_type() {
    let t = case();
    check_redundant_warning(&t, &["/** @enum {!boolean} */ var x;"]);
    check_redundant_warning(&t, &["/** @enum {!number} */ var x;"]);
    check_redundant_warning(&t, &["/** @enum {!bigint} */ var x;"]);
    check_redundant_warning(&t, &["/** @enum {!string} */ var x;"]);
    check_redundant_warning(&t, &["/** @enum {!symbol} */ var x;"]);
    check_no_warning(&t, &["/** @enum {?string} */ var x;"]);
    check_no_warning(&t, &["/** @enum {string} */ var x;"]);
    check_no_warning(&t, &["/** @enum {Object} */ var x;"]);
    check_no_warning(&t, &["/** @enum {?Object} */ var x;"]);
    check_no_warning(&t, &["/** @enum {!Object} */ var x;"]);
}

// port: CheckNullabilityModifiersTest#testTemplateDefinitionType
#[test]
fn test_template_definition_type() {
    let t = case();
    check_no_warning(&t, &["/** @param {T} x @template T */", "function f(x){}"]);
    check_no_warning(
        &t,
        &[
            "/** @param {S} x @return {T} @template S,T */",
            "function f(x){}",
        ],
    );
    check_no_warning(
        &t,
        &[r#"/** @constructor @template T */ function Foo(){}
/** @param {T} x */ Foo.prototype.bar = function(x){};
"#],
    );
}

// port: CheckNullabilityModifiersTest#testTemplateDefinitionTypeWithTtl
#[test]
fn test_template_definition_type_with_ttl() {
    let t = case();
    check_no_warning(
        &t,
        &[r#"/**
 * @param {T} x
 * @param {U} y
 * @template T
 * @template U := T =:
 */
function f(x, y) {}
"#],
    );
    check_no_warning(
        &t,
        &[r#"/**
 * @param {T} x
 * @param {U} y
 * @template T
// note that "Object" below is /not/ nullable
 * @template U := cond(isUnknown(T), "Object", T) =:
 */
function f(x, y) {}
"#],
    );
}

// port: CheckNullabilityModifiersTest#testTemplateInstantiationType
#[test]
fn test_template_instantiation_type() {
    let t = case();
    check_redundant_warning(&t, &["/** @type {!Array<!string>} */ var x;"]);
    check_missing_warning(&t, &["/** @type {Array<string>} */ var x;"]);
    check_missing_warning(&t, &["/** @type {!Array<Object>} */ var x;"]);
    check_no_warning(&t, &["/** @type {!Array<?string>} */ var x;"]);
    check_no_warning(&t, &["/** @type {!Array<string>} */ var x;"]);
    check_no_warning(&t, &["/** @type {!Array<?Object>} */ var x;"]);
    check_no_warning(&t, &["/** @type {!Array<!Object>} */ var x;"]);
}

// port: CheckNullabilityModifiersTest#testParamType
#[test]
fn test_param_type() {
    let t = case();
    check_redundant_warning(&t, &["/** @param {!string} x */ function f(x){}"]);
    check_missing_warning(&t, &["/** @param {Object} x */ function f(x){}"]);
    check_no_warning(&t, &["/** @param {?string} x */ function f(x){}"]);
    check_no_warning(&t, &["/** @param {string} x */ function f(x){}"]);
    check_no_warning(&t, &["/** @param {?Object} x */ function f(x){}"]);
    check_no_warning(&t, &["/** @param {!Object} x */ function f(x){}"]);
}

// port: CheckNullabilityModifiersTest#testParamMissingType
#[test]
fn test_param_missing_type() {
    let t = case();
    check_no_warning(&t, &["/** @param x */ function f(x){}"]);
}

// port: CheckNullabilityModifiersTest#testReturnType
#[test]
fn test_return_type() {
    let t = case();
    check_redundant_warning(&t, &["/** @return {!string} */ function f(){}"]);
    check_missing_warning(&t, &["/** @return {Object} */ function f(){}"]);
    check_no_warning(&t, &["/** @return {?string} */ function f(){}"]);
    check_no_warning(&t, &["/** @return {string} */ function f(){}"]);
    check_no_warning(&t, &["/** @return {?Object} */ function f(){}"]);
    check_no_warning(&t, &["/** @return {!Object} */ function f(){}"]);
}

// port: CheckNullabilityModifiersTest#testTypedefType
#[test]
fn test_typedef_type() {
    let t = case();
    check_redundant_warning(&t, &["/** @typedef {!string} */ var x;"]);
    check_missing_warning(&t, &["/** @typedef {Object} */ var x;"]);
    check_no_warning(&t, &["/** @typedef {?string} */ var x;"]);
    check_no_warning(&t, &["/** @typedef {string} */ var x;"]);
    check_no_warning(&t, &["/** @typedef {?Object} */ var x;"]);
    check_no_warning(&t, &["/** @typedef {!Object} */ var x;"]);
}

// port: CheckNullabilityModifiersTest#testThisType
#[test]
fn test_this_type() {
    let t = case();
    check_redundant_warning(&t, &["/** @this {!string} */ function f(){}"]);
    check_no_warning(&t, &["/** @this {?string} */ function f(){}"]);
    check_no_warning(&t, &["/** @this {string} */ function f(){}"]);
    check_no_warning(&t, &["/** @this {Object} */ function f(){}"]);
    check_no_warning(&t, &["/** @this {?Object} */ function f(){}"]);
    check_no_warning(&t, &["/** @this {!Object} */ function f(){}"]);
}

// port: CheckNullabilityModifiersTest#testBaseType
#[test]
fn test_base_type() {
    let t = case();
    check_no_warning(&t, &["/** @extends {Object} */ function f(){}"]);
    check_no_warning(&t, &["/** @implements {Object} */ function f(){}"]);
}

// port: CheckNullabilityModifiersTest#testTypeOf
#[test]
fn test_type_of() {
    let t = case();
    check_no_warning(&t, &["/** @type {typeof Object} */ var x;"]);
}

// port: CheckNullabilityModifiersTest#testEndPosition
#[test]
fn test_end_position() {
    let t = case();
    check_redundant_warning(&t, &["/** @type {string!} */ var x;"]);
    check_no_warning(&t, &["/** @type {string?} */ var x;"]);
    check_no_warning(&t, &["/** @type {Object!} */ var x;"]);
    check_no_warning(&t, &["/** @type {Object?} */ var x;"]);
}

// port: CheckNullabilityModifiersTest#testMultipleFiles
#[test]
fn test_multiple_files() {
    let t = case();
    check_missing_warning(
        &t,
        &[
            "/** @param {T} x @return {T} @template T */ function f(x){}",
            "/** @param {T} x */ function g(x){}",
        ],
    );
}

// port: CheckNullabilityModifiersTest#testSetToNull
#[test]
fn test_set_to_null() {
    let t = case();
    check_null_missing_warning(&t, &["/** @type {Object} */ var x = null;"]);
    check_null_missing_warning(
        &t,
        &["/** @constructor */ function C() {} /** @private {Object} */ C.prop = null;"],
    );
    check_null_missing_warning(
        &t,
        &["/** @constructor */ function C() { /** @private {Object} */ this.foo = null; }"],
    );
    check_null_missing_warning(
        &t,
        &["/** @constructor */ function C() { /** @private {Object|string} */ this.foo = null; }"],
    );
    check_null_missing_warning(
        &t,
        &["/** @constructor */ function C() { /** @private {!String|Symbol} */ this.foo = null; }"],
    );
    check_no_warning(&t, &["/** @type {?Object} */ var x = null;"]);
    check_no_warning(
        &t,
        &["/** @constructor */ function C() {} /** @private {?Symbol} */ C.prop = null;"],
    );
    check_no_warning(
        &t,
        &["/** @constructor */ function C() { /** @private {?Object} */ this.foo = null; }"],
    );
    // don't recommend making 'Type' nullable since it's not the root of the type expression
    check_missing_warning(&t, &["/** @type {!Array<Type>} */ let arr = null;"]);
    check_missing_warning(&t, &["/** @type {?{prop: Type}} */ let o = null;"]);
}

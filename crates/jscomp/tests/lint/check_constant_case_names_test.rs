/*
 * Copyright 2019 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/lint/CheckConstantCaseNamesTest.java.

use super::support::LintTestCase;
use closure_jscomp::lint::check_constant_case_names::{
    CheckConstantCaseNames, MISSING_CONST_PROPERTY, REASSIGNED_CONSTANT_CASE_NAME,
};

// port: CheckConstantCaseNamesTest#getProcessor
// port: CheckConstantCaseNamesTest#getOptions
fn case() -> LintTestCase {
    LintTestCase::new(|c| Box::new(CheckConstantCaseNames::new(c))).with_lint_checks_warning()
}

// port: CheckConstantCaseNamesTest#emptyScriptIsOk
#[test]
fn empty_script_is_ok() {
    let t = case();
    t.test_no_warning("");
}

// port: CheckConstantCaseNamesTest#emptyModuleIsOk
#[test]
fn empty_module_is_ok() {
    let t = case();
    t.test_no_warning("goog.module('mod');");
    t.test_no_warning("export {};");
}

// port: CheckConstantCaseNamesTest#constantCaseNameDeclarations_requireExplicitConst
#[test]
fn constant_case_name_declarations_require_explicit_const() {
    let t = case();
    t.test_warning("goog.module('m'); var ABC;", &MISSING_CONST_PROPERTY);
    t.test_warning("goog.module('m'); var ABC = 0;", &MISSING_CONST_PROPERTY);
    t.test_warning("goog.module('m'); let ABC = 0;", &MISSING_CONST_PROPERTY);
    t.test_warning(
        "goog.module('m'); var ABC_DEF_0 = 0;",
        &MISSING_CONST_PROPERTY,
    );
    t.test_warning(
        "goog.module('m'); var abc = 0, DEF = 1;",
        &MISSING_CONST_PROPERTY,
    );
    t.test_warnings_sources(
        &["goog.module('m'); var ABC = 0, DEF = 1;"],
        &[&MISSING_CONST_PROPERTY, &MISSING_CONST_PROPERTY],
    );
    t.test_warning(
        "goog.module('m'); var {ABC} = obj;",
        &MISSING_CONST_PROPERTY,
    );
    t.test_warning(
        "goog.module('m'); var {abc, DEF} = obj;",
        &MISSING_CONST_PROPERTY,
    );
    t.test_warning("let ABC = 0; export {ABC};", &MISSING_CONST_PROPERTY);
}

// port: CheckConstantCaseNamesTest#constantCaseNameDeclarations_whenReassigned_requireCamelCase
#[test]
fn constant_case_name_declarations_when_reassigned_require_camel_case() {
    let t = case();
    t.test_warning(
        "goog.module('m'); var ABC = 0; ABC = 1;",
        &REASSIGNED_CONSTANT_CASE_NAME,
    );
    t.test_warning(
        "goog.module('m'); let ABC = 0; ABC = 1;",
        &REASSIGNED_CONSTANT_CASE_NAME,
    );
    t.test_warning(
        "goog.module('m'); let ABC = 0; [[ABC]] = [[1]];",
        &REASSIGNED_CONSTANT_CASE_NAME,
    );
}

// port: CheckConstantCaseNamesTest#constantCaseNameDeclarations_whenReassignedInFunction_requireCamelCase
#[test]
fn constant_case_name_declarations_when_reassigned_in_function_require_camel_case() {
    let t = case();
    t.test_warning(
        "goog.module('m'); var ABC = 0; exports = function() { ABC = 1; };",
        &REASSIGNED_CONSTANT_CASE_NAME,
    );
    t.test_warning(
        "goog.module('m'); let ABC = 0; exports = function() { ABC = 1; };",
        &REASSIGNED_CONSTANT_CASE_NAME,
    );
}

// port: CheckConstantCaseNamesTest#constantCaseNameDeclarations_localShadowAreNotReassignments
#[test]
fn constant_case_name_declarations_local_shadow_are_not_reassignments() {
    let t = case();
    t.test_warning(
        "goog.module('m'); let ABC = 0; if (cond) { let ABC = 0; ABC = 1; }",
        &MISSING_CONST_PROPERTY,
    );
    t.test_warning(
        "goog.module('m'); let ABC = 0; exports = function(ABC) { ABC = 1; };",
        &MISSING_CONST_PROPERTY,
    );
}

// port: CheckConstantCaseNamesTest#constantCaseNameDeclarations_usagesAreNotReassignments
#[test]
fn constant_case_name_declarations_usages_are_not_reassignments() {
    let t = case();
    t.test_warning(
        "goog.module('m'); let ABC = 0; alert(ABC);",
        &MISSING_CONST_PROPERTY,
    );
    t.test_warning(
        "goog.module('m'); let ABC = 0; const {d = ABC} = {};",
        &MISSING_CONST_PROPERTY,
    );
    t.test_warning(
        "goog.module('m'); let ABC = 0; let d = 1; export {d as ABC};",
        &MISSING_CONST_PROPERTY,
    );
}

// port: CheckConstantCaseNamesTest#constantCaseNameDeclarations_appearingInMultipleFiles_getDistinctErrors
#[test]
fn constant_case_name_declarations_appearing_in_multiple_files_get_distinct_errors() {
    let t = case();
    t.test_warnings_sources(
        &["goog.module('m'); let ABC = 0;", "let ABC = 0; ABC = 1;"],
        &[&MISSING_CONST_PROPERTY],
    );
    t.test_warnings_sources(
        &[
            "goog.module('m1'); let ABC = 0;",
            "goog.module('m2'); let ABC = 0; ABC = 1;",
        ],
        &[&MISSING_CONST_PROPERTY, &REASSIGNED_CONSTANT_CASE_NAME],
    );
}

// port: CheckConstantCaseNamesTest#constantCaseNameDeclarations_okIfAlreadyConst
#[test]
fn constant_case_name_declarations_ok_if_already_const() {
    let t = case();
    t.test_same("goog.module('m'); const ABC = 0;");
    t.test_same("goog.module('m'); const ABC = 0;");
    t.test_same("goog.module('m'); const ABC_DEF_0 = 0;");
    t.test_same("goog.module('m'); /** @const */ var ABC = 0;");
    t.test_same("goog.module('m'); /** @const */ let ABC = 0;");
    t.test_same("goog.module('m'); /** @const */ var ABC_DEF_0 = 0;");
    t.test_same("goog.module('m'); const {ABC} = obj;");
}

// port: CheckConstantCaseNamesTest#nonConstantCaseNameDeclarations_dontRequireConst
#[test]
fn non_constant_case_name_declarations_dont_require_const() {
    let t = case();
    t.test_same("goog.module('m'); var abc = 0;");
    t.test_same("goog.module('m'); let abc = 0;");
    t.test_same("goog.module('m'); var AbcDef = 0;");
    t.test_same("goog.module('m'); var abc;");
}

// port: CheckConstantCaseNamesTest#nonModuleLevelNamesDontCauseWarning
#[test]
fn non_module_level_names_dont_cause_warning() {
    let t = case();
    t.test_same("var ABC = 0;");
    t.test_same("goog.module('m'); function fn() { var ABC = 0; }");
}

// port: CheckConstantCaseNamesTest#constantCasePropertyDeclarations_dontRequireExplicitConst
#[test]
fn constant_case_property_declarations_dont_require_explicit_const() {
    let t = case();
    t.test_same("goog.module('m'); /** @private */ a.NAME = 'name';");
}

// port: CheckConstantCaseNamesTest#constantCaseObjectLiteralKeys_dontRequireExplicitConst
#[test]
fn constant_case_object_literal_keys_dont_require_explicit_const() {
    let t = case();
    t.test_same("goog.module('m'); const Colors = {RED: 0, YELLOW: 1, GREEN: 2}");
    t.test_same("goog.module('m'); ns.Colors = {RED: 0, YELLOW: 1, GREEN: 2}");
}

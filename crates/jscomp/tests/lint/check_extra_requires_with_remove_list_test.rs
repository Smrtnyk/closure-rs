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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/lint/CheckExtraRequiresWithRemoveListTest.java.

use super::support::LintTestCase;
use closure_jscomp::{
    check_level::CheckLevel,
    diagnostic_groups,
    lint::check_extra_requires::{CheckExtraRequires, EXTRA_REQUIRE_WARNING},
};
use closure_rhino::fast_hash::IndexSet;

// port: CheckExtraRequiresWithRemoveListTest#getOptions
// port: CheckExtraRequiresWithRemoveListTest#getProcessor
fn case() -> LintTestCase {
    LintTestCase::new(|c| {
        Box::new(CheckExtraRequires::new(
            c,
            Some(IndexSet::<_>::from_iter([
                "xx".to_string(),
                "yy".to_string(),
                "zz".to_string(),
            ])),
        ))
    })
    .with_options(|options| {
        options.set_warning_level(diagnostic_groups::EXTRA_REQUIRE.clone(), CheckLevel::ERROR);
        options.set_warning_level(diagnostic_groups::MODULE_LOAD.clone(), CheckLevel::OFF);
    })
}

// port: CheckExtraRequiresWithRemoveListTest#testNoChange
#[test]
fn test_no_change() {
    let t = case();
    t.test_same("goog.require('aa');");
    t.test_same("goog.require('xx.foo');");
}

// port: CheckExtraRequiresWithRemoveListTest#testAffected
#[test]
fn test_affected() {
    let t = case();
    t.test_error_sources(&["goog.require('xx');"], &EXTRA_REQUIRE_WARNING);
    t.test_error_sources(&["goog.require('yy');"], &EXTRA_REQUIRE_WARNING);
    t.test_error_sources(&["goog.require('zz');"], &EXTRA_REQUIRE_WARNING);
}

// port: CheckExtraRequiresWithRemoveListTest#testShouldntRemove
#[test]
fn test_shouldnt_remove() {
    let t = case();
    t.test_same("goog.require('xx'); var x = new xx();");
}

// port: CheckExtraRequiresWithRemoveListTest#testUsedInJsDoc
#[test]
fn test_used_in_js_doc() {
    let t = case();
    t.test_same(
        r#"goog.require('xx');
/** @type {xx}*/
var a;
"#,
    );
}

// port: CheckExtraRequiresWithRemoveListTest#testUsedInSubNamespace
#[test]
fn test_used_in_sub_namespace() {
    let t = case();
    t.test_same("goog.require('xx.foo'); var a = xx.foo();");
}

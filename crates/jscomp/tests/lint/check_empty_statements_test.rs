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
//   test/com/google/javascript/jscomp/lint/CheckEmptyStatementsTest.java.

use super::support::LintTestCase;
use closure_jscomp::lint::check_empty_statements::{CheckEmptyStatements, USELESS_EMPTY_STATEMENT};

// port: CheckEmptyStatementsTest#getProcessor
// port: CheckEmptyStatementsTest#getOptions
fn case() -> LintTestCase {
    LintTestCase::new(|c| Box::new(CheckEmptyStatements::new(c))).with_lint_checks_warning()
}

// port: CheckEmptyStatementsTest#testWarning(String)
fn test_warning_js(t: &LintTestCase, js: &str) {
    t.test_warning(js, &USELESS_EMPTY_STATEMENT);
}

// port: CheckEmptyStatementsTest#testWarning
#[test]
fn test_warning() {
    let t = case();
    test_warning_js(&t, "function f() {};");
    test_warning_js(&t, "var x;;");
    test_warning_js(&t, "alert(1);;");
}

// port: CheckEmptyStatementsTest#testWarning_withES6Modules01
#[test]
fn test_warning_with_es6_modules01() {
    let t = case();
    test_warning_js(&t, "export function f() {};;");
}

// port: CheckEmptyStatementsTest#testWarning_withES6Modules02
#[test]
fn test_warning_with_es6_modules02() {
    let t = case();
    test_warning_js(&t, "export var x;;");
}

// port: CheckEmptyStatementsTest#testNoWarning
#[test]
fn test_no_warning() {
    let t = case();
    t.test_same("function f() {}");
    t.test_same("var x;");
    t.test_same("alert(1);");
    t.test_same("if (x); y;");
}

// port: CheckEmptyStatementsTest#testNoWarning_withES6Modules01
#[test]
fn test_no_warning_with_es6_modules01() {
    let t = case();
    t.test_same("export function f() {}");
}

// port: CheckEmptyStatementsTest#testNoWarning_withES6Modules02
#[test]
fn test_no_warning_with_es6_modules02() {
    let t = case();
    t.test_same("export var x;");
}

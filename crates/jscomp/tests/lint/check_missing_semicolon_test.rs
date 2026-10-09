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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/lint/CheckMissingSemicolonTest.java.

use super::support::LintTestCase;
use closure_jscomp::diagnostic_groups;
use closure_jscomp::lint::check_missing_semicolon::{CheckMissingSemicolon, MISSING_SEMICOLON};

// port: CheckMissingSemicolonTest#getProcessor
// port: CheckMissingSemicolonTest#getOptions
fn case() -> LintTestCase {
    LintTestCase::new(|c| Box::new(CheckMissingSemicolon::new(c))).with_lint_checks_warning()
}

// port: CheckMissingSemicolonTest#testWarning(String)
fn test_warning_js(t: &LintTestCase, js: &str) {
    t.test_warning(js, &MISSING_SEMICOLON);
}

// port: CheckMissingSemicolonTest#testWarning
#[test]
fn test_warning() {
    let t = case();
    test_warning_js(&t, "var x");
    test_warning_js(&t, "alert(1)");
    test_warning_js(&t, "do { things; } while (true)");
}

// port: CheckMissingSemicolonTest#testNoWarning_withSemicolon
#[test]
fn test_no_warning_with_semicolon() {
    let t = case();
    t.test_same("var x;");
    t.test_same("alert(1);");
}

// port: CheckMissingSemicolonTest#testNoWarning_controlStructure
#[test]
fn test_no_warning_control_structure() {
    let t = case();
    t.test_same("if (true) {}");
    t.test_same("while (true) {}");
    t.test_same("do { things; } while (true);");
    t.test_same("switch (n) { case 3: alert(4); }");
    t.test_same("switch (n) { case 5: { alert(6); } }");
    t.test_same("alert(1); LABEL: while (true) { alert(2); }");
    t.test_same("for (;;) {}");
    t.test_same("for (x of y) {}");
    t.test_same("for (x in y) {}");
}

// port: CheckMissingSemicolonTest#testNoWarning_functionOrClass
#[test]
fn test_no_warning_function_or_class() {
    let t = case();
    t.test_same("function f() {}");
    t.test_same("function* f() {}");
    t.test_same("class Example {}");
}

// port: CheckMissingSemicolonTest#testWarning_export
#[test]
fn test_warning_export() {
    let mut t = case();
    t.ignored_groups
        .push(diagnostic_groups::MODULE_LOAD.clone());
    test_warning_js(&t, "export var x = 3");
    test_warning_js(&t, "export * from './other'");
}

// port: CheckMissingSemicolonTest#testNoWarning_export
#[test]
fn test_no_warning_export() {
    let t = case();
    t.test_same("export function f() {}");
    t.test_same("export class C {}");
}

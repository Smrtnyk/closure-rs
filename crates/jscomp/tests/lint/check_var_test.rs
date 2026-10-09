/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/lint/CheckVarTest.java.

use super::support::LintTestCase;
use closure_jscomp::lint::check_var::{CheckVar, VAR};

// port: CheckVarTest#getProcessor
// port: CheckVarTest#getOptions
fn case() -> LintTestCase {
    LintTestCase::new(|c| Box::new(CheckVar::new(c))).with_lint_checks_warning()
}

// port: CheckVarTest#testWarning(String)
fn test_warning_js(t: &LintTestCase, js: &str) {
    t.test_warning(js, &VAR);
}

// port: CheckVarTest#testWarning
#[test]
fn test_warning() {
    let t = case();
    test_warning_js(&t, "var x;");
    test_warning_js(&t, "var x = 12;");
    test_warning_js(&t, "export var x;");
    test_warning_js(&t, "function f() { var x = 12; return x; }");
}

// port: CheckVarTest#testSuppressWarning
#[test]
fn test_suppress_warning() {
    let t = case();
    t.test_same(
        r#"/**
 * @fileoverview
 * @suppress {lintVarDeclarations}
 */
var x;
var x12 = 12;
export var x;
function f() { var x = 12; return x; }
"#,
    );
}

// port: CheckVarTest#testNoWarning
#[test]
fn test_no_warning() {
    let t = case();
    t.test_same("function f() { return 'var'; }");
    t.test_same("let x;");
    t.test_same("let x = 12;");
    t.test_same("const x = 12;");
    t.test_same("function f() { let x; x = 12; return x; }");
    t.test_same("function f() { const x = 12; return x; }");
}

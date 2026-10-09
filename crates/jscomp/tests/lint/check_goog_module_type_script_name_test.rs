/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/lint/CheckGoogModuleTypeScriptNameTest.java.

use super::support::LintTestCase;
use closure_jscomp::lint::check_goog_module_type_script_name::{
    CheckGoogModuleTypeScriptName, MODULE_NAMESPACE_MISMATCHES_TYPESCRIPT_NAMESPACE,
};

// port: CheckGoogModuleTypeScriptNameTest#getProcessor
// port: CheckGoogModuleTypeScriptNameTest#getOptions
fn case() -> LintTestCase {
    LintTestCase::new(|c| Box::new(CheckGoogModuleTypeScriptName::new(c)))
        .with_lint_checks_warning()
}

// port: CheckGoogModuleTypeScriptNameTest#testNoWarning
#[test]
fn test_no_warning() {
    let t = case();
    t.test_no_warning_files(&[(
        "gws/js/test.js",
        "goog.module('google3.gws.js.test');\nalert(1);",
    )]);
}

// port: CheckGoogModuleTypeScriptNameTest#testNoWarning_notOnAllowList
#[test]
fn test_no_warning_not_on_allow_list() {
    let t = case();
    t.test_no_warning_files(&[(
        "javascript/apps/foo/test.js",
        "goog.module('test');\nalert(1);",
    )]);
}

// port: CheckGoogModuleTypeScriptNameTest#testNoWarning_noModule
#[test]
fn test_no_warning_no_module() {
    let t = case();
    t.test_no_warning(
        r#"goog.provide('test');
alert(1)
"#,
    );
}

// port: CheckGoogModuleTypeScriptNameTest#testWarning
#[test]
fn test_warning() {
    let t = case();
    t.test_warning_files_containing(
        &[("gws/js/test.js", "goog.module('test');\nalert(1);")],
        &MODULE_NAMESPACE_MISMATCHES_TYPESCRIPT_NAMESPACE,
        "The correct namespace is: \"google3.gws.js.test\"",
    );
}

// MOE::begin_strip
// port: CheckGoogModuleTypeScriptNameTest#testWarningWithFullSourceName
#[test]
fn test_warning_with_full_source_name() {
    let t = case();
    t.test_warning_files_containing(
        &[(
            "/google/src/cloud/someuser/someworkspace/google3/gws/js/test.js",
            "goog.module('test');\nalert(1);",
        )],
        &MODULE_NAMESPACE_MISMATCHES_TYPESCRIPT_NAMESPACE,
        "The correct namespace is: \"google3.gws.js.test\"",
    );
}
// MOE::end_strip

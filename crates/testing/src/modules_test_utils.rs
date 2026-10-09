/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/ModulesTestUtils.java.

//! Port of test/com/google/javascript/jscomp/ModulesTestUtils.java: test utilities for testing
//! modules, used by Es6RewriteModulesTest, Es6RewriteModulesBeforeTypeCheckingTest and
//! ProcessCommonJSModulesTest.
use crate::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    jscomp_api::{DiagnosticType, SourceFile},
    throwable::Throwable,
};
use std::sync::Arc;

// port: ModulesTestUtils#testModules
pub fn test_modules(
    test: &mut CompilerTestCase,
    hooks: &mut impl CompilerTestCaseHooks,
    file_name: &str,
    input: &str,
    expected: &str,
) -> Result<(), Throwable> {
    // Shared with ProcessCommonJSModulesTest.
    let inputs = vec![
        Arc::new(SourceFile::from_code(
            "other.js",
            "goog.provide('module$other');",
        )),
        Arc::new(SourceFile::from_code(
            "yet_another.js",
            "goog.provide('module$yet_another');",
        )),
        Arc::new(SourceFile::from_code(file_name, input)),
    ];
    let expecteds = vec![
        Arc::new(SourceFile::from_code(
            "other.js",
            "goog.provide('module$other');",
        )),
        Arc::new(SourceFile::from_code(
            "yet_another.js",
            "goog.provide('module$yet_another');",
        )),
        Arc::new(SourceFile::from_code(file_name, expected)),
    ];
    test.test(
        hooks,
        vec![
            TestPart::Sources(CompilerTestCase::srcs_files(inputs)),
            TestPart::Expected(CompilerTestCase::expected_files(expecteds)),
        ],
    )
}

// port: ModulesTestUtils#testModulesError
pub fn test_modules_error(
    test: &mut CompilerTestCase,
    hooks: &mut impl CompilerTestCaseHooks,
    input: &str,
    error: &'static DiagnosticType,
) -> Result<(), Throwable> {
    let inputs = vec![
        Arc::new(SourceFile::from_code("other.js", "")),
        Arc::new(SourceFile::from_code("testcode.js", input)),
    ];
    test.test_error_sources(
        hooks,
        CompilerTestCase::srcs_files(inputs),
        CompilerTestCase::error(error),
    )
}

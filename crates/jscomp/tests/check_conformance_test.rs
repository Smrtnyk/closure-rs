/*
 * Copyright 2006 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 8ac03b6:
//   test/com/google/javascript/jscomp/CheckConformanceTest.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java.

//! The `CheckConformanceTest` methods that call `CheckConformance` directly instead of through the
//! recorded harness. The harness methods are covered by the unit records
//! (`crates/testing/tests/conformance_records.rs`).

use closure_jscomp::{
    Compiler,
    check_conformance::CheckConformance,
    compiler_options::CompilerOptions,
    conformance_config::{ConformanceConfig, Requirement},
};

// port: CompilerTestCase#createCompiler
fn create_compiler() -> Compiler {
    let mut compiler = Compiler::new();
    compiler.init_options(CompilerOptions::new());
    compiler
}

// port: CheckConformanceTest#testMergeRequirementsErrorMessage
#[test]
fn test_merge_requirements_error_message() {
    let mut compiler = create_compiler();
    let mut builder = ConformanceConfig::new_builder();
    let mut root = Requirement::new_builder();
    root.set_rule_id("a").set_error_message("Root message.");
    builder.add_requirement(root);
    let mut extending = Requirement::new_builder();
    extending
        .set_extends("a")
        .set_error_message("Project message.");
    builder.add_requirement(extending);
    let mut check_conformance = CheckConformance::new_empty(&mut compiler);
    let requirements = check_conformance.merge_requirements(
        &mut compiler,
        &[builder.build()],
        /* is_library_level_conformance_reporting_mode= */ false,
    );
    assert_eq!(requirements.len(), 1);
    assert_eq!(
        requirements[0].get_error_message(),
        "Root message. Project message."
    );
}

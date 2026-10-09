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
//   test/com/google/javascript/jscomp/ConformanceIntegrationTest.java.

//! Port of `ConformanceIntegrationTest.java` (tests for ConformancePassConfig).

use closure_jscomp::{
    Compiler, check_conformance, compiler_options::CompilerOptions,
    conformance_config::ConformanceConfig, conformance_pass_config::ConformancePassConfig,
    default_pass_config::DefaultPassConfig, protobuf::text_format, source_file::SourceFile,
};
use std::sync::Arc;

// port: ConformanceIntegrationTest#allowsValidCode
#[test]
fn allows_valid_code() {
    let compiler = run_checks(create_compiler_options(), "const x = 0;");

    assert!(compiler.get_warnings().is_empty());
    assert!(compiler.get_errors().is_empty());
}

// port: ConformanceIntegrationTest#bansCallsToName
#[test]
fn bans_calls_to_name() {
    let compiler = run_checks(create_compiler_options(), "bannedName('3');");

    assert!(compiler.get_errors().is_empty());
    assert_eq!(compiler.get_warnings().len(), 1);
    assert!(std::ptr::eq(
        compiler.get_warnings()[0].get_type(),
        &check_conformance::CONFORMANCE_VIOLATION
    ));
}

// port: ConformanceIntegrationTest#bansCallsToName_inEsModule
#[test]
fn bans_calls_to_name_in_es_module() {
    let mut options = create_compiler_options();
    options.set_bad_rewrite_modules_before_typechecking_that_we_want_to_get_rid_of(false);

    let compiler = run_checks(options, "let x = 1; bannedName('3'); export {};");

    assert!(compiler.get_errors().is_empty());
    assert_eq!(compiler.get_warnings().len(), 1);
    assert!(std::ptr::eq(
        compiler.get_warnings()[0].get_type(),
        &check_conformance::CONFORMANCE_VIOLATION
    ));
}

// port: ConformanceIntegrationTest#DEFAULT_CONFORMANCE
const DEFAULT_CONFORMANCE: &str = "requirement: {
  type: BANNED_NAME
  value: 'bannedName'
   error_message: 'bannedName is not allowed'
}
";

// port: ConformanceIntegrationTest#createCompilerOptions
fn create_compiler_options() -> CompilerOptions {
    let mut options = CompilerOptions::new();
    options.set_closure_pass(true);
    options.set_check_types(true);
    let mut builder = ConformanceConfig::new_builder();
    if let Err(e) = text_format::merge(DEFAULT_CONFORMANCE, &mut builder) {
        panic!("{e:?}");
    }
    options.set_conformance_config(builder.build());
    options
}

// port: ConformanceIntegrationTest#runChecks
fn run_checks(options: CompilerOptions, input: &str) -> Compiler {
    let mut compiler = Compiler::new();

    compiler.set_pass_config(Box::new(ConformancePassConfig::new(Box::new(
        DefaultPassConfig::new(options.clone()),
    ))));
    compiler.init(
        &[],
        &[Arc::new(SourceFile::from_code("test.js", input))],
        options,
    );
    compiler.parse_for_compilation();
    assert!(compiler.get_warnings().is_empty());
    assert!(compiler.get_errors().is_empty());

    compiler.check();

    compiler
}

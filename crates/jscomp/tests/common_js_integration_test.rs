/*
 * Copyright 2012 The Closure Compiler Authors.
 * Copyright 2013 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/integration/CommonJSIntegrationTest.java,
//   test/com/google/javascript/jscomp/integration/IntegrationTestCase.java.

//! Port of CommonJSIntegrationTest (integration/CommonJSIntegrationTest.java). The tests run on
//! crates/testing's native IntegrationTestCase. The other 15 tests compile with the whole ADVANCED
//! pipeline and ES5 output; they are not ported as tests here.
use closure_jscomp::{
    compiler_options::{CompilerOptions, LanguageMode},
    deps::module_loader::ResolutionMode,
    diagnostic_groups,
    google_coding_convention::GoogleCodingConvention,
    warning_level::WarningLevel,
};
use closure_rhino::js_string::JsString;
use closure_testing::integration::integration_test_case::IntegrationTestCase;
use std::sync::Arc;

// port: CommonJSIntegrationTest#createCompilerOptions
fn create_compiler_options() -> CompilerOptions {
    let mut options = CompilerOptions::new();
    options.set_language_out(LanguageMode::ECMASCRIPT5);
    options.set_coding_convention(Arc::new(GoogleCodingConvention::new()));
    WarningLevel::VERBOSE.set_options_for_warning_level(&mut options);
    options.set_process_common_js_modules(true);
    options.set_closure_pass(true);
    options.set_module_resolution_mode(ResolutionMode::NODE);
    // For debugging failures
    options.set_pretty_print(true);
    options.set_preserve_type_annotations(true);
    options
}

// port: IntegrationTestCase#setUp (DEFAULT_EXTERNS)
fn set_up() -> IntegrationTestCase {
    IntegrationTestCase::set_up(IntegrationTestCase::default_externs().unwrap())
}

// port: CommonJSIntegrationTest#testEsModuleImportNonDefaultFromCJS
#[test]
fn test_es_module_import_non_default_from_cjs() {
    let mut t = set_up();
    t.test_warning(
        create_compiler_options(),
        &[
            JsString::from("module.exports = {foo: 1, bar: function() { return 'bar'; }};"),
            JsString::from("import {foo} from './i0';"),
        ],
        None,
        diagnostic_groups::MODULE_IMPORT.clone(),
    )
    .unwrap();
}

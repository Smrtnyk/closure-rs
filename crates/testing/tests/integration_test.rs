/*
 * Copyright 2009 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 977e2cf:
//   test/com/google/javascript/jscomp/integration/IntegrationTest.java.

//! Port of the IntegrationTest tests whose assertions after the hooked `compile` read compiler
//! state that no record holds (corpus/unit/rust_unit_tests/IntegrationTest.md). The other tests
//! of the class replay from corpus/unit/records (alpha_integration_records.rs).
use closure_jscomp::{
    compiler_options::{CompilerOptions, DevMode, IncrementalCheckMode, LanguageMode},
    google_coding_convention::GoogleCodingConvention,
};
use closure_rhino::js_string::JsString;
use closure_testing::integration::integration_test_case::{
    IntegrationTestCase, check_unexpected_errors_or_warnings,
};
use std::sync::Arc;

// port: IntegrationTestCase#setUp (DEFAULT_EXTERNS)
fn set_up() -> IntegrationTestCase {
    IntegrationTestCase::set_up(IntegrationTestCase::default_externs().unwrap())
}

/// Creates a CompilerOptions object with google coding conventions.
// port: IntegrationTest#createCompilerOptions
fn create_compiler_options() -> CompilerOptions {
    let mut options = CompilerOptions::new();
    options.set_language_out(LanguageMode::ECMASCRIPT3);
    options.set_dev_mode(DevMode::EVERY_PASS);
    options.set_coding_convention(Arc::new(GoogleCodingConvention::new()));
    options.set_rename_prefix_namespace_assume_cross_chunk_names(true);
    options.set_assume_getters_are_pure(false);
    options.set_pretty_print(true);
    options
}

// port: IntegrationTest#testGenerateIjsWithHoistedExternsInChunks
#[test]
fn test_generate_ijs_with_hoisted_externs_in_chunks() {
    let mut t = set_up();
    let mut options = create_compiler_options();
    options.set_incremental_checks(IncrementalCheckMode::GENERATE_IJS);

    let compiler = t
        .compile(
            options,
            &[
                JsString::from("/** @externs */ /** @constructor */ function MyExternType() {}"),
                JsString::from("/** @type {!MyExternType} */ var x = new MyExternType();"),
            ],
        )
        .unwrap();
    {
        let compiler = compiler.borrow();
        check_unexpected_errors_or_warnings(&compiler.get_errors(), &compiler.get_warnings(), 0)
            .unwrap();
    }

    let chunks = compiler.borrow().get_chunks().unwrap();
    assert_eq!(
        compiler
            .borrow_mut()
            .to_source_for_chunk(&chunks[0])
            .to_string(),
        "/** @fileoverview @typeSummary */\n\
         /** @externs */ \n\
         /** @constructor */ \n\
         function MyExternType() {\n\
         }\n\
         ;"
    );
    assert_eq!(
        compiler
            .borrow_mut()
            .to_source_for_chunk(&chunks[1])
            .to_string(),
        "/** @fileoverview @typeSummary */\n\
         /** @type {!MyExternType} */ \n\
         var x;\n"
    );
}

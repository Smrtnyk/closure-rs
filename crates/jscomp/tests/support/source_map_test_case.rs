/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   test/com/google/debugging/sourcemap/SourceMapTestCase.java.

//! Compiler-dependent SourceMapTestCase helpers. Token discovery and mapping
//! validation remain shared with the sourcemap crate's single port.
#![allow(dead_code)] // Java base methods are used by different integration test binaries.

#[path = "../../../sourcemap/tests/common/mod.rs"]
pub mod common;
pub use common::source_map_test_case as mapping_checks;

use closure_jscomp::{
    Compiler,
    black_hole_error_manager::BlackHoleErrorManager,
    check_level::CheckLevel,
    compiler_options::CompilerOptions,
    diagnostic_groups::CHECK_USELESS_CODE,
    js_error::JSError,
    source_file::SourceFile,
    source_map::{DetailLevel, Format, SourceMap},
    warnings_guard::WarningsGuard,
};
use closure_rhino::js_string::JsString;
use closure_sourcemap::gson::{Gson, JsonElement, Target};
use closure_sourcemap::source_map_consumer_v3::SourceMapConsumerV3;
use std::{
    any::Any,
    fmt,
    sync::{Arc, Mutex},
};

const GSON: Gson = Gson::new();
const JSON_MAP_TYPE: Target = Target::JsonObject;

// port: SourceMapTestCase#RunResult
pub struct RunResult {
    pub generated_source: JsString,
    pub source_map: Arc<Mutex<SourceMap>>,
    pub source_map_file_content: String,
}

pub struct SourceMapTestCase {
    pub detail_level: DetailLevel,
    pub source_map_include_sources_content: bool,
    pub checks: mapping_checks::SourceMapTestCase,
}
impl Default for SourceMapTestCase {
    // port: SourceMapTestCase#SourceMapTestCase
    fn default() -> Self {
        Self {
            detail_level: DetailLevel::ALL,
            source_map_include_sources_content: false,
            checks: Default::default(),
        }
    }
}
impl SourceMapTestCase {
    // port: SourceMapTestCase#setUp
    pub fn set_up(&mut self) {
        self.detail_level = DetailLevel::ALL;
    }
    // port: SourceMapTestCase#getCompilerOptions
    pub fn get_compiler_options(&self, format: Format) -> CompilerOptions {
        let mut options = CompilerOptions::new();
        options.set_source_map_output_path("testcode_source_map.out".into());
        options.set_source_map_format(format);
        options.set_source_map_detail_level(self.detail_level);
        options.set_source_map_include_sources_content(self.source_map_include_sources_content);
        options.add_warnings_guard(Arc::new(CheckUselessCodeGuard));
        options
    }
}

#[derive(Debug)]
struct CheckUselessCodeGuard;
impl WarningsGuard for CheckUselessCodeGuard {
    // port: SourceMapTestCase#getCompilerOptions.level
    fn level(&self, error: &JSError) -> Option<CheckLevel> {
        if CHECK_USELESS_CODE.matches(error) {
            return Some(CheckLevel::OFF);
        }
        None
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl fmt::Display for CheckUselessCodeGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SourceMapTestCase.WarningsGuard")
    }
}

pub trait SourceMapTest {
    fn test_case(&self) -> &SourceMapTestCase;
    // port: SourceMapTestCase#getSourceMapFormat
    fn get_source_map_format(&self) -> Format;
    // port: SourceMapTestCase#getSourceMapConsumer
    fn get_source_map_consumer(&self) -> SourceMapConsumerV3;
    fn get_compiler_options(&self) -> CompilerOptions {
        self.test_case()
            .get_compiler_options(self.get_source_map_format())
    }
    // port: SourceMapTestCase#checkSourceMap(String,ImmutableMap)
    fn check_source_map(&self, js: &str, expected_map: JsonElement) {
        self.check_source_map_for_file("testcode", js, expected_map);
    }
    // port: SourceMapTestCase#checkSourceMap(String,String,ImmutableMap)
    fn check_source_map_for_file(&self, file_name: &str, js: &str, expected_map: JsonElement) {
        let result = self.compile(js, file_name);
        let round_tripped_expected_map = GSON
            .from_json(GSON.to_json(&expected_map), JSON_MAP_TYPE)
            .unwrap();
        let result_map = GSON
            .from_json(result.source_map_file_content.as_str(), JSON_MAP_TYPE)
            .unwrap();
        assert_eq!(
            result_map, round_tripped_expected_map,
            "{}",
            result.generated_source
        );
        let result_map_map = GSON
            .from_json(get_source_map(&result), JSON_MAP_TYPE)
            .unwrap();
        assert_eq!(
            result_map_map, round_tripped_expected_map,
            "{}",
            result.generated_source
        );
    }
    // port: SourceMapTestCase#compileAndCheck
    fn compile_and_check(&self, js: &str) {
        let input_name = "testcode";
        let result = self.compile(js, input_name);
        self.test_case().checks.check(
            &input_name.into(),
            &js.into(),
            &result.generated_source,
            &result.source_map_file_content.into(),
        );
    }
    // port: SourceMapTestCase#compile(String,String)
    fn compile(&self, js: impl Into<JsString>, file_name: &str) -> RunResult {
        self.compile_two(js, file_name, None, None)
    }
    // port: SourceMapTestCase#compile(String,String,String,String)
    fn compile_two(
        &self,
        js1: impl Into<JsString>,
        file_name1: &str,
        js2: Option<JsString>,
        file_name2: Option<&str>,
    ) -> RunResult {
        // BlackHoleErrorManager uses the same SortingErrorManager collection as
        // Java's TestErrorManager; this fixture asserts Result's errors/warnings.
        let mut compiler = Compiler::new_with_error_manager(Box::new(BlackHoleErrorManager::new()));
        let mut options = self.get_compiler_options();
        options.set_checks_only(true);
        let mut inputs = vec![Arc::new(SourceFile::from_code(file_name1, js1))];
        if let (Some(js2), Some(file_name2)) = (js2, file_name2) {
            inputs.push(Arc::new(SourceFile::from_code(file_name2, js2)));
        }
        let externs = [Arc::new(SourceFile::from_code("externs", ""))];
        let result = compiler.compile(&externs, &inputs, options);
        assert!(
            result.errors.is_empty(),
            "compilation failed with errors: {:?}",
            result.errors
        );
        assert!(
            result.warnings.is_empty(),
            "compilation failed with warnings: {:?}",
            result.warnings
        );
        assert!(result.success, "compilation failed (other reason)");
        let source = compiler.to_source();
        let mut sb = String::new();
        let source_map = result.source_map.unwrap();
        {
            let mut map = source_map.lock().unwrap();
            map.validate(true);
            map.append_to(&mut sb, "testcode").unwrap();
        }
        RunResult {
            generated_source: source,
            source_map,
            source_map_file_content: sb,
        }
    }
}

// port: SourceMapTestCase#getSourceMap
pub fn get_source_map(result: &RunResult) -> String {
    let mut sb = String::new();
    result
        .source_map
        .lock()
        .unwrap()
        .append_to(&mut sb, "testcode")
        .unwrap();
    sb
}

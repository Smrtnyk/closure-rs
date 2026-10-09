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
//   test/com/google/javascript/jscomp/SourceMapJsLangTest.java.

#[path = "support/source_map_test_case.rs"]
mod source_map_test_case;
use closure_jscomp::{
    compiler_options::CompilerOptions, source_file::SourceFile, source_map::Format,
    source_map_input::SourceMapInput,
};
use closure_rhino::fast_hash::IndexMap;
use closure_sourcemap::source_map_consumer_v3::SourceMapConsumerV3;
use source_map_test_case::{SourceMapTest as _, SourceMapTestCase};
use std::{path::PathBuf, sync::Arc};

struct SourceMapJsLangTest {
    base: SourceMapTestCase,
    file_content: String,
    file_name: String,
    pretty_printed: bool,
    input_maps: IndexMap<String, Arc<SourceMapInput>>,
}
const DATA_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/testdata/source_map_js_lang_test"
);
// port: SourceMapJsLangTest#data
fn data() -> Vec<String> {
    let mut test_files = Vec::new();
    for test_file in std::fs::read_dir(DATA_DIR).unwrap() {
        let name = test_file.unwrap().file_name().into_string().unwrap();
        if let Some(name) = name.strip_suffix(".jsdata") {
            test_files.push(name.into());
        }
    }
    test_files
}
impl SourceMapJsLangTest {
    // port: SourceMapJsLangTest#SourceMapJsLangTest
    fn new(file_name: String) -> Self {
        let file_content =
            std::fs::read_to_string(PathBuf::from(DATA_DIR).join(format!("{file_name}.jsdata")))
                .unwrap();
        Self {
            base: Default::default(),
            file_content,
            file_name,
            pretty_printed: false,
            input_maps: IndexMap::<_, _>::default(),
        }
    }
    // port: SourceMapJsLangTest#testSourceMapsInCollapsedCodeWork
    fn test_source_maps_in_collapsed_code_work(&self) {
        let result = self.compile(self.file_content.as_str(), &self.file_name);
        self.base.checks.check(
            &self.file_name.clone().into(),
            &self.file_content.clone().into(),
            &result.generated_source,
            &result.source_map_file_content.into(),
        );
    }
    // port: SourceMapJsLangTest#testSourceMapsInPrettyPrintedCodeWork
    fn test_source_maps_in_pretty_printed_code_work(&mut self) {
        self.pretty_printed = true;
        let result = self.compile(self.file_content.as_str(), &self.file_name);
        self.base.checks.check(
            &self.file_name.clone().into(),
            &self.file_content.clone().into(),
            &result.generated_source,
            &result.source_map_file_content.into(),
        );
    }
    // port: SourceMapJsLangTest#testRepeatedCompilation
    fn test_repeated_compilation(&mut self) {
        let first_compilation = self.compile(self.file_content.as_str(), &self.file_name);
        let new_file_name = format!("{}.compiled", self.file_name);
        self.input_maps.insert(
            new_file_name.clone(),
            Arc::new(SourceMapInput::new(Arc::new(SourceFile::from_code(
                "sourcemap",
                first_compilation.source_map_file_content,
            )))),
        );
        self.pretty_printed = true;
        let second_compilation = self.compile(first_compilation.generated_source, &new_file_name);
        self.base.checks.check(
            &self.file_name.clone().into(),
            &self.file_content.clone().into(),
            &second_compilation.generated_source,
            &second_compilation.source_map_file_content.into(),
        );
    }
}
impl source_map_test_case::SourceMapTest for SourceMapJsLangTest {
    fn test_case(&self) -> &SourceMapTestCase {
        &self.base
    }
    // port: SourceMapJsLangTest#getSourceMapFormat
    fn get_source_map_format(&self) -> Format {
        Format::V3
    }
    // port: SourceMapJsLangTest#getSourceMapConsumer
    fn get_source_map_consumer(&self) -> SourceMapConsumerV3 {
        SourceMapConsumerV3::new()
    }
    // port: SourceMapJsLangTest#getCompilerOptions
    fn get_compiler_options(&self) -> CompilerOptions {
        let mut options = self.base.get_compiler_options(self.get_source_map_format());
        options.set_pretty_print(self.pretty_printed);
        if !self.input_maps.is_empty() {
            options.set_apply_input_source_maps(true);
            options.set_input_source_maps(self.input_maps.clone());
        }
        options
    }
}
#[test]
fn test_source_maps_in_collapsed_code_work() {
    for file_name in data() {
        SourceMapJsLangTest::new(file_name).test_source_maps_in_collapsed_code_work();
    }
}
#[test]
fn test_source_maps_in_pretty_printed_code_work() {
    for file_name in data() {
        SourceMapJsLangTest::new(file_name).test_source_maps_in_pretty_printed_code_work();
    }
}
#[test]
fn test_repeated_compilation() {
    for file_name in data() {
        SourceMapJsLangTest::new(file_name).test_repeated_compilation();
    }
}

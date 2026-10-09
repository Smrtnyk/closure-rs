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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/SourceMapTest.java.

#[path = "support/source_map_test_case.rs"]
mod source_map_test_case;
use closure_jscomp::{
    Compiler,
    compiler_options::CompilerOptions,
    source_file::SourceFile,
    source_map::{Format, LocationMapping, PrefixLocationMapping},
    source_map_input::SourceMapInput,
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_sourcemap::source_map_consumer_v3::SourceMapConsumerV3;
use source_map_test_case::{SourceMapTest as _, SourceMapTestCase, get_source_map};
use std::{fmt, sync::Arc};

struct SourceMapTest {
    base: SourceMapTestCase,
    mappings: Option<Vec<Arc<dyn LocationMapping + Send + Sync>>>,
    input_maps: IndexMap<String, Arc<SourceMapInput>>,
}
impl SourceMapTest {
    // port: SourceMapTest#SourceMapTest
    fn new() -> Self {
        let mut test = Self {
            base: Default::default(),
            mappings: None,
            input_maps: IndexMap::<_, _>::default(),
        };
        test.set_up();
        test
    }
    // port: SourceMapTest#setUp
    fn set_up(&mut self) {
        self.base.set_up();
        self.input_maps = IndexMap::<_, _>::default();
    }
    // port: SourceMapTest#checkSourceMap2
    fn check_source_map2(
        &self,
        js1: &str,
        file1: &str,
        js2: &str,
        file2: &str,
        expected_map: &str,
    ) {
        let result = self.compile_two(js1, file1, Some(js2.into()), Some(file2));
        assert_eq!(result.source_map_file_content, expected_map);
        assert_eq!(get_source_map(&result), result.source_map_file_content);
    }
}
impl source_map_test_case::SourceMapTest for SourceMapTest {
    fn test_case(&self) -> &SourceMapTestCase {
        &self.base
    }
    // port: SourceMapTest#getCompilerOptions
    fn get_compiler_options(&self) -> CompilerOptions {
        let mut options = self.base.get_compiler_options(self.get_source_map_format());
        if let Some(mappings) = &self.mappings {
            options.set_source_map_location_mappings(mappings.clone());
        }
        if !self.input_maps.is_empty() {
            options.set_apply_input_source_maps(true);
            options.set_input_source_maps(self.input_maps.clone());
        }
        options
    }
    // port: SourceMapTest#getSourceMapFormat
    fn get_source_map_format(&self) -> Format {
        Format::V3
    }
    // port: SourceMapTest#getSourceMapConsumer
    fn get_source_map_consumer(&self) -> SourceMapConsumerV3 {
        SourceMapConsumerV3::new()
    }
}
struct LambdaLocationMapping;
impl LocationMapping for LambdaLocationMapping {
    // port: SourceMapTest#testLambdaReplacement.lambda
    fn map(&self, location: &JsString) -> Option<JsString> {
        Some(JsString::from("mapped/").concat(location))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl fmt::Display for LambdaLocationMapping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SourceMapTest.lambda")
    }
}

// port: SourceMapTest#testPrefixReplacement1
#[test]
fn test_prefix_replacement1() {
    let mut test = SourceMapTest::new();
    test.mappings = Some(vec![Arc::new(PrefixLocationMapping::new("pre/", ""))]);
    test.check_source_map2(
        "alert(1);",
        &Compiler::join_path_parts(&["pre", "file1"]),
        "alert(2);",
        &Compiler::join_path_parts(&["pre", "file2"]),
        r#"{
"version":3,
"file":"testcode",
"lineCount":1,
"mappings":"A,aAAAA,KAAA,CAAM,CAAN,C,CCAAA,KAAA,CAAM,CAAN;",
"sources":["file1","file2"],
"names":["alert"]
}
"#,
    );
}

// port: SourceMapTest#testPrefixReplacement2
#[test]
fn test_prefix_replacement2() {
    let mut test = SourceMapTest::new();
    test.mappings = Some(vec![Arc::new(PrefixLocationMapping::new(
        "pre/file", "src",
    ))]);
    test.check_source_map2(
        "alert(1);",
        &Compiler::join_path_parts(&["pre", "file1"]),
        "alert(2);",
        "pre/file2",
        r#"{
"version":3,
"file":"testcode",
"lineCount":1,
"mappings":"A,aAAAA,KAAA,CAAM,CAAN,C,CCAAA,KAAA,CAAM,CAAN;",
"sources":["src1","src2"],
"names":["alert"]
}
"#,
    );
}

// port: SourceMapTest#testPrefixReplacement3
#[test]
fn test_prefix_replacement3() {
    let mut test = SourceMapTest::new();
    test.mappings = Some(vec![
        Arc::new(PrefixLocationMapping::new("file1", "x")),
        Arc::new(PrefixLocationMapping::new("file2", "y")),
    ]);
    test.check_source_map2(
        "alert(1);",
        "file1",
        "alert(2);",
        "file2",
        r#"{
"version":3,
"file":"testcode",
"lineCount":1,
"mappings":"A,aAAAA,KAAA,CAAM,CAAN,C,CCAAA,KAAA,CAAM,CAAN;",
"sources":["x","y"],
"names":["alert"]
}
"#,
    );
}

// port: SourceMapTest#testPrefixReplacement4
#[test]
fn test_prefix_replacement4() {
    let mut test = SourceMapTest::new();
    test.mappings = Some(vec![
        Arc::new(PrefixLocationMapping::new("file1", "x")),
        Arc::new(PrefixLocationMapping::new("file", "y")),
    ]);
    test.check_source_map2(
        "alert(1);",
        "file1",
        "alert(2);",
        "file2",
        r#"{
"version":3,
"file":"testcode",
"lineCount":1,
"mappings":"A,aAAAA,KAAA,CAAM,CAAN,C,CCAAA,KAAA,CAAM,CAAN;",
"sources":["x","y2"],
"names":["alert"]
}
"#,
    );
}

// port: SourceMapTest#testLambdaReplacement
#[test]
fn test_lambda_replacement() {
    let mut test = SourceMapTest::new();
    test.mappings = Some(vec![Arc::new(LambdaLocationMapping)]);
    test.check_source_map2(
        "alert(1);",
        "file1",
        "alert(2);",
        "file2",
        r#"{
"version":3,
"file":"mapped/testcode",
"lineCount":1,
"mappings":"A,aAAAA,KAAA,CAAM,CAAN,C,CCAAA,KAAA,CAAM,CAAN;",
"sources":["mapped/file1","mapped/file2"],
"names":["alert"]
}
"#,
    );
}

// port: SourceMapTest#testRepeatedCompilation
#[test]
fn test_repeated_compilation() {
    let mut test = SourceMapTest::new();
    let file_content = "function foo() {} alert(foo());";
    let file_name = "foo.js";
    let first_compilation = test.compile(file_content, file_name);
    let new_file_name = format!("{file_name}.compiled");
    test.input_maps.insert(
        new_file_name.clone(),
        Arc::new(SourceMapInput::new(Arc::new(SourceFile::from_code(
            "sourcemap",
            first_compilation.source_map_file_content,
        )))),
    );
    let second_compilation = test.compile(first_compilation.generated_source, &new_file_name);
    test.base.checks.check(
        &file_name.into(),
        &file_content.into(),
        &second_compilation.generated_source,
        &second_compilation.source_map_file_content.into(),
    );
}

// port: SourceMapTest#testIntermediateFilesOmitUnmappedCode
#[test]
fn test_intermediate_files_omit_unmapped_code() {
    let mut test = SourceMapTest::new();
    let file = "generated.tsx.js";
    let code = "'use strict';function foo(){}alert(foo());";
    let input_map = r#"{
"version":3,
"file":"testcode",
"lineCount":1,
"mappings":"A,aAAAA,QAASA,IAAG,EAAG,E,KAAG,CAAMA,GAAA,EAAN;",
"sources":["foo.js"],
"names":["foo"]
}
"#;
    test.input_maps.insert(
        file.into(),
        Arc::new(SourceMapInput::new(Arc::new(SourceFile::from_code(
            "sourcemap",
            input_map,
        )))),
    );
    let compilation = test.compile(code, file);
    assert_eq!(
        compilation.source_map_file_content,
        r#"{
"version":3,
"file":"testcode",
"lineCount":1,
"mappings":"A,aAAAA,QAASA,IAAG,EAAG,E,MAASA,GAAAA;",
"sources":["foo.js"],
"names":["foo"]
}
"#
    );
}

// port: SourceMapTest#testEmptySourceMapsAreIgnored
#[test]
fn test_empty_source_maps_are_ignored() {
    let mut test = SourceMapTest::new();
    let file = "input.js";
    let code = "inputCode";
    let input_map = r#"{
"version":3,
"names":[],
"mappings":"",
"sources":["foo/bar/baz.js"],
"sourcesContent":["inputCode"],
"file":"foo/bar/baz.js"
}
"#;
    test.input_maps.insert(
        file.into(),
        Arc::new(SourceMapInput::new(Arc::new(SourceFile::from_code(
            "sourcemap",
            input_map,
        )))),
    );
    let compilation = test.compile(code, file);
    assert_eq!(
        compilation.source_map_file_content,
        r#"{
"version":3,
"file":"testcode",
"lineCount":1,
"mappings":"A,aAAAA;",
"sources":["input.js"],
"names":["inputCode"]
}
"#
    );
}

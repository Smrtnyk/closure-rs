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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/debugging/sourcemap/SourceMapConsumerGeneratorTest.java.

use closure_rhino::js_string::JsString;
use closure_sourcemap::{
    file_position::FilePosition, source_map_consumer_v3::SourceMapConsumerV3,
    source_map_format::SourceMapFormat, source_map_generator_factory::SourceMapGeneratorFactory,
};
use indexmap::{IndexMap, IndexSet};

// port: SourceMapConsumerGeneratorTest#test
#[test]
fn test() {
    let source_files = IndexMap::<JsString, JsString>::from([
        ("file_one.txt".into(), "content of file one".into()),
        ("file_two.txt".into(), "content of\nfile two".into()),
        ("file_three.txt".into(), "even\n  more\n    content".into()),
    ]);
    let concatenated_file: JsString = source_files
        .iter()
        .map(|(key, value)| format!("// {key}\n{value}"))
        .collect::<Vec<_>>()
        .join("\n")
        .into();
    let sourcemap = generate_source_map(&concatenated_file);
    validate_source_map(&source_files, &concatenated_file, &sourcemap);
}

// Preserve the Java indexed loops over the Splitter list and each Java char.
#[allow(clippy::needless_range_loop)]
// port: SourceMapConsumerGeneratorTest#generateSourceMap
fn generate_source_map(file: &JsString) -> JsString {
    let mut generator = SourceMapGeneratorFactory::get_instance(SourceMapFormat::V3);
    let lines: Vec<_> = file
        .as_units()
        .split(|&c| c == b'\n' as u16)
        .map(|line| JsString::from_units(line.to_vec()))
        .collect();
    let mut current_source_file = JsString::from("");
    let mut source_file_line_number = 0;
    for i in 0..lines.len() {
        let line = &lines[i];
        if line.starts_with("// ") {
            current_source_file = line.substring(3, line.length());
            source_file_line_number = 0;
            continue;
        }
        let mut current_word = Vec::new();
        for j in 0..line.length() {
            let c = line.char_at(j);
            if c == b' ' as u16 {
                if !current_word.is_empty() {
                    let start_pos = j - current_word.len();
                    generator.add_mapping(
                        Some(current_source_file.clone()),
                        Some(JsString::from_units(current_word.clone())),
                        FilePosition::new(source_file_line_number, start_pos as i32),
                        FilePosition::new(i as i32, start_pos as i32),
                        FilePosition::new(i as i32, j as i32 - 1),
                    );
                    current_word = Vec::new();
                }
            } else {
                current_word.push(c);
            }
        }
        if !current_word.is_empty() {
            let start_pos = line.length() - current_word.len();
            generator.add_mapping(
                Some(current_source_file.clone()),
                Some(JsString::from_units(current_word)),
                FilePosition::new(source_file_line_number, start_pos as i32),
                FilePosition::new(i as i32, start_pos as i32),
                FilePosition::new(i as i32, line.length() as i32 - 1),
            );
        }
        source_file_line_number += 1;
    }
    let mut sourcemap = String::new();
    generator
        .append_to(&mut sourcemap, Some("test".into()))
        .unwrap()
}

// port: SourceMapConsumerGeneratorTest#validateSourceMap
fn validate_source_map(
    source_files: &IndexMap<JsString, JsString>,
    concatenated_file: &JsString,
    sourcemap: &JsString,
) {
    let mut consumer = SourceMapConsumerV3::new();
    consumer.parse(sourcemap).unwrap();
    let mut mappings_count = 0;
    let mut seen_symbols = IndexSet::new();
    consumer.visit_mappings(&mut |source_name: Option<&JsString>,
                                  symbol_name: Option<&JsString>,
                                  source_start_position: FilePosition,
                                  start_position: FilePosition,
                                  _end_position| {
        mappings_count += 1;
        seen_symbols.insert(symbol_name.cloned());
        let source_file_content = &source_files[source_name.unwrap()];
        assert!(
            get_rest_of_line_that_starts_at_position(source_start_position, source_file_content)
                .starts_with(symbol_name.unwrap())
        );
        assert!(
            get_rest_of_line_that_starts_at_position(start_position, concatenated_file)
                .starts_with(symbol_name.unwrap())
        );
    });
    assert_eq!(mappings_count, 11);
    assert_eq!(
        seen_symbols,
        IndexSet::<Option<JsString>>::from_iter(
            ["content", "of", "file", "one", "two", "even", "more"].map(|s| Some(s.into()))
        )
    );
}

// port: SourceMapConsumerGeneratorTest#getRestOfLineThatStartsAtPosition
fn get_rest_of_line_that_starts_at_position(position: FilePosition, text: &JsString) -> JsString {
    let lines: Vec<_> = text
        .as_units()
        .split(|&c| c == b'\n' as u16)
        .map(|line| JsString::from_units(line.to_vec()))
        .collect();
    let line = &lines[position.get_line() as usize];
    line.substring(position.get_column() as usize, line.length())
}

/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/Compiler.java.

use closure_jscomp::{
    check_level::CheckLevel, diagnostic_type::DiagnosticType, error_format::ErrorFormat,
    error_handler::ErrorHandler, error_manager::ErrorManager, js_error::JSError,
    print_stream_error_report_generator::PrintStreamErrorReportGenerator, region::Region,
    sorting_error_manager::SortingErrorManager, source_excerpt_provider::SourceExcerptProvider,
    source_file::SourceFile, sourcemap_mapping_placeholder::OriginalMapping,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{node::Ast, static_source_file::SourceKind, token::Token};
use serde_json::Value;
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};
struct Provider {
    files: IndexMap<String, Option<SourceFile>>,
    mappings: IndexMap<(Option<String>, i32, i32), OriginalMapping>,
}
impl Provider {
    // port: Compiler#getSourceFileByName
    fn source_file(&self, name: Option<&str>, line: i32) -> Option<&SourceFile> {
        if line < 1 {
            return None;
        }
        self.files
            .get(name?)
            .and_then(Option::as_ref)
            .filter(|file| !file.is_stub_source_file_for_already_provided_input())
    }
}
impl SourceExcerptProvider for Provider {
    // port: Compiler#getSourceLine
    fn get_source_line(&self, name: Option<&str>, line: i32) -> Option<String> {
        self.source_file(name, line)?.get_line(line)
    }
    // port: Compiler#getSourceLines
    fn get_source_lines(
        &self,
        name: Option<&str>,
        line: i32,
        length: i32,
    ) -> Option<Box<dyn Region>> {
        self.source_file(name, line)?
            .get_lines(line, length)
            .map(|r| Box::new(r) as Box<dyn Region>)
    }
    // port: Compiler#getSourceRegion
    fn get_source_region(&self, name: Option<&str>, line: i32) -> Option<Box<dyn Region>> {
        self.source_file(name, line)?
            .get_region(line)
            .map(|r| Box::new(r) as Box<dyn Region>)
    }
    // port: Compiler#getSourceMapping (recorded answers from the JVM)
    fn get_source_mapping(
        &self,
        name: Option<&str>,
        line: i32,
        column: i32,
    ) -> Option<OriginalMapping> {
        self.mappings
            .get(&(name.map(str::to_owned), line, column))
            .cloned()
    }
}
#[derive(Clone)]
struct Buffer(Arc<Mutex<Vec<u8>>>);
impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn level(value: &Value) -> CheckLevel {
    match value.as_str().unwrap() {
        "ERROR" => CheckLevel::ERROR,
        "WARNING" => CheckLevel::WARNING,
        "OFF" => CheckLevel::OFF,
        s => panic!("unexpected level {s}"),
    }
}
fn number(value: &Value) -> i32 {
    value.as_i64().unwrap() as i32
}
pub fn render(row: &Value, types: &mut IndexMap<String, &'static DiagnosticType>) -> String {
    let mut provider = Provider {
        files: IndexMap::<_, _>::default(),
        mappings: IndexMap::<_, _>::default(),
    };
    for (name, source) in row["sources"].as_object().unwrap() {
        let file = if source.is_null() {
            None
        } else if source["stub"].as_bool().unwrap() {
            Some(SourceFile::stub_source_file(name, SourceKind::STRONG))
        } else {
            if let Some(units) = source["code_utf16"].as_array() {
                Some(SourceFile::from_code(
                    name,
                    closure_rhino::js_string::JsString::from_units(
                        units
                            .iter()
                            .map(|u| u.as_u64().unwrap() as u16)
                            .collect::<Vec<_>>(),
                    ),
                ))
            } else {
                source["code"]
                    .as_str()
                    .map(|code| SourceFile::from_code(name, code))
            }
        };
        provider.files.insert(name.clone(), file);
    }
    let mut ast = Ast::new();
    let mut reports = Vec::new();
    for d in row["diagnostics"].as_array().unwrap() {
        let source_name = d["source_name"].as_str().map(str::to_owned);
        let lineno = number(&d["lineno"]);
        let charno = number(&d["charno"]);
        if !d["mapping"].is_null() {
            let m = &d["mapping"];
            provider.mappings.insert(
                (source_name.clone(), lineno, charno),
                OriginalMapping::new_builder()
                    .set_original_file(m["original_file"].as_str().unwrap())
                    .set_line_number(number(&m["line_number"]))
                    .set_column_position(number(&m["column_position"]))
                    .build(),
            );
        }
        let key = d["key"].as_str().unwrap();
        let type_ = *types.entry(key.into()).or_insert_with(|| {
            Box::leak(Box::new(DiagnosticType::make(
                String::leak(key.into()),
                level(&d["type_level"]),
                String::leak(d["format"].as_str().unwrap().into()),
            )))
        });
        assert_eq!(type_.level, level(&d["type_level"]));
        let node = if d["has_node"].as_bool().unwrap() {
            let n = ast.new_node(Token::EMPTY);
            n.set_length(&mut ast, number(&d["node_length"]));
            Some(n)
        } else {
            None
        };
        reports.push((
            level(&d["level"]),
            JSError {
                type_,
                description: d["description"].as_str().unwrap().into(),
                source_name,
                lineno,
                charno,
                length: number(&d["length"]),
                node,
                default_level: level(&d["default_level"]),
                requirement: None,
            },
        ));
    }
    let buffer = Buffer(Arc::new(Mutex::new(Vec::new())));
    let formatter = ErrorFormat::FULL.to_formatter(Some(Arc::new(provider)), false);
    let generator = PrintStreamErrorReportGenerator::new(
        formatter,
        buffer.clone(),
        number(&row["summary_detail_level"]),
    );
    let mut manager = SortingErrorManager::new(vec![Box::new(generator)]);
    for (level, error) in reports {
        manager.report(level, error);
    }
    manager.set_typed_percent(row["typed_percent"].as_f64().unwrap());
    assert_eq!(manager.get_error_count(), number(&row["error_count"]));
    assert_eq!(manager.get_warning_count(), number(&row["warning_count"]));
    manager.generate_report(&ast);
    String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap()
}
pub fn unified_diff(expected: &str, actual: &str) -> String {
    let a: Vec<_> = expected.split_inclusive('\n').collect();
    let b: Vec<_> = actual.split_inclusive('\n').collect();
    let prefix = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let start = prefix.saturating_sub(3);
    let a_end = (a.len() - suffix + 3).min(a.len());
    let b_end = (b.len() - suffix + 3).min(b.len());
    let mut result = format!(
        "--- Java\n+++ Rust\n@@ -{},{} +{},{} @@\n",
        start + 1,
        a_end - start,
        start + 1,
        b_end - start
    );
    for line in &a[start..prefix] {
        result.push(' ');
        result.push_str(line);
    }
    for line in &a[prefix..a.len() - suffix] {
        result.push('-');
        result.push_str(line);
        if !line.ends_with('\n') {
            result.push('\n');
        }
    }
    for line in &b[prefix..b.len() - suffix] {
        result.push('+');
        result.push_str(line);
        if !line.ends_with('\n') {
            result.push('\n');
        }
    }
    for line in &a[a.len() - suffix..a_end] {
        result.push(' ');
        result.push_str(line);
    }
    result
}
pub fn exclusion(row: &Value) -> Option<&'static str> {
    if row["stderr"] == row["report_stderr"] {
        None
    } else if row["stderr"].as_str().unwrap().contains("\n\tat ") || row["exception"].is_string() {
        Some("Java compiler stack trace outside report generator")
    } else {
        Some("CLI text outside report generator")
    }
}

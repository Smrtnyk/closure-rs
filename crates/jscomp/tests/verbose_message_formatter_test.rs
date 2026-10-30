/*
 * Copyright 2026 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/VerboseMessageFormatterTest.java.

use closure_jscomp::{
    diagnostic_type::DiagnosticType, js_error::JSError, message_formatter::MessageFormatter,
    region::Region, simple_region::SimpleRegion, source_excerpt_provider::SourceExcerptProvider,
    sourcemap_mapping_placeholder::OriginalMapping,
    verbose_message_formatter::VerboseMessageFormatter,
};
use closure_rhino::node::Ast;
use std::sync::Arc;

static FOO_TYPE: DiagnosticType = DiagnosticType::error("TEST_FOO", "error description here");
static WARNING_TYPE: DiagnosticType =
    DiagnosticType::warning("TEST_WARN", "warning description here");

/// Java: JSError.make(String,int,int,DiagnosticType), with a nullable source name.
fn make(
    source_name: Option<&str>,
    lineno: i32,
    charno: i32,
    type_: &'static DiagnosticType,
) -> JSError {
    let mut error =
        JSError::make_with_source_location(source_name.unwrap_or(""), lineno, charno, type_, &[]);
    error.source_name = source_name.map(str::to_owned);
    error
}

// port: VerboseMessageFormatterTest#testFormatError_validLineNumber
#[test]
fn test_format_error_valid_line_number() {
    let formatter = VerboseMessageFormatter::new(empty_source_provider());
    let error = make(Some("test.js"), 10, 5, &FOO_TYPE);

    let formatted = formatter.format_error(&Ast::new(), &error);

    assert!(formatted.contains("line 10"));
    assert!(!formatted.contains("(unknown line)"));
    assert_eq!(
        formatted,
        "ERROR: error description here at test.js line 10 ."
    );
}

// port: VerboseMessageFormatterTest#testFormatError_unknownLineNumber
#[test]
fn test_format_error_unknown_line_number() {
    let formatter = VerboseMessageFormatter::new(empty_source_provider());
    let error = make(Some("test.js"), -1, -1, &FOO_TYPE);

    let formatted = formatter.format_error(&Ast::new(), &error);

    assert!(formatted.contains("line (unknown line)"));
    assert!(!formatted.contains("-1"));
    assert_eq!(
        formatted,
        "ERROR: error description here at test.js line (unknown line) ."
    );
}

// port: VerboseMessageFormatterTest#testFormatError_unknownSourceName
#[test]
fn test_format_error_unknown_source_name() {
    let formatter = VerboseMessageFormatter::new(empty_source_provider());
    let error = make(None, 10, 5, &FOO_TYPE);

    let formatted = formatter.format_error(&Ast::new(), &error);

    assert!(formatted.contains("(unknown source)"));
    assert!(formatted.contains("line 10"));
    assert_eq!(
        formatted,
        "ERROR: error description here at (unknown source) line 10 ."
    );
}

// port: VerboseMessageFormatterTest#testFormatError_unknownSourceAndLineNumber
#[test]
fn test_format_error_unknown_source_and_line_number() {
    let formatter = VerboseMessageFormatter::new(empty_source_provider());
    let error = make(None, -1, -1, &FOO_TYPE);

    let formatted = formatter.format_error(&Ast::new(), &error);

    assert!(formatted.contains("(unknown source)"));
    assert!(formatted.contains("line (unknown line)"));
    assert_eq!(
        formatted,
        "ERROR: error description here at (unknown source) line (unknown line) ."
    );
}

struct RegionProvider;
impl SourceExcerptProvider for RegionProvider {
    // port: VerboseMessageFormatterTest.SourceExcerptProvider#getSourceLine
    fn get_source_line(&self, _name: Option<&str>, _line: i32) -> Option<String> {
        None
    }
    // port: VerboseMessageFormatterTest.SourceExcerptProvider#getSourceLines
    fn get_source_lines(
        &self,
        _name: Option<&str>,
        _line: i32,
        _length: i32,
    ) -> Option<Box<dyn Region>> {
        None
    }
    // port: VerboseMessageFormatterTest.SourceExcerptProvider#getSourceRegion
    fn get_source_region(&self, name: Option<&str>, line: i32) -> Option<Box<dyn Region>> {
        if name == Some("test.js") && line == 10 {
            return Some(Box::new(SimpleRegion::new(10, 10, "const x = 1;")));
        }
        None
    }
    // port: VerboseMessageFormatterTest.SourceExcerptProvider#getSourceMapping
    fn get_source_mapping(
        &self,
        _name: Option<&str>,
        _line: i32,
        _col: i32,
    ) -> Option<OriginalMapping> {
        None
    }
}

// port: VerboseMessageFormatterTest#testFormatError_withSourceRegion
#[test]
fn test_format_error_with_source_region() {
    let formatter = VerboseMessageFormatter::new(Arc::new(RegionProvider));
    let error = make(Some("test.js"), 10, 0, &FOO_TYPE);

    let formatted = formatter.format_error(&Ast::new(), &error);

    assert_eq!(
        formatted,
        "ERROR: error description here at test.js line 10 :\n\nconst x = 1;"
    );
}

// port: VerboseMessageFormatterTest#testFormatWarning_validLineNumber
#[test]
fn test_format_warning_valid_line_number() {
    let formatter = VerboseMessageFormatter::new(empty_source_provider());
    let warning = make(Some("test.js"), 42, 0, &WARNING_TYPE);

    let formatted = formatter.format_warning(&Ast::new(), &warning);

    assert!(formatted.starts_with("WARNING: "));
    assert!(formatted.contains("line 42"));
    assert_eq!(
        formatted,
        "WARNING: warning description here at test.js line 42 ."
    );
}

struct EmptySourceProvider;
impl SourceExcerptProvider for EmptySourceProvider {
    // port: VerboseMessageFormatterTest.SourceExcerptProvider#getSourceLine
    fn get_source_line(&self, _name: Option<&str>, _line: i32) -> Option<String> {
        None
    }
    // port: VerboseMessageFormatterTest.SourceExcerptProvider#getSourceLines
    fn get_source_lines(
        &self,
        _name: Option<&str>,
        _line: i32,
        _length: i32,
    ) -> Option<Box<dyn Region>> {
        None
    }
    // port: VerboseMessageFormatterTest.SourceExcerptProvider#getSourceRegion
    fn get_source_region(&self, _name: Option<&str>, _line: i32) -> Option<Box<dyn Region>> {
        None
    }
    // port: VerboseMessageFormatterTest.SourceExcerptProvider#getSourceMapping
    fn get_source_mapping(
        &self,
        _name: Option<&str>,
        _line: i32,
        _col: i32,
    ) -> Option<OriginalMapping> {
        None
    }
}

// port: VerboseMessageFormatterTest#emptySourceProvider
fn empty_source_provider() -> Arc<dyn SourceExcerptProvider> {
    Arc::new(EmptySourceProvider)
}

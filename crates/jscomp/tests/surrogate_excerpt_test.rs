/*
 * Copyright 2026 The closure-rs Authors.
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
// Ported from closure-rs' own Java oracle tooling: crates/jscomp/tests/java/SurrogateExcerpt.java.

//! Lone surrogates in source text reach stderr the way Java's UTF-8 PrintStream writes them:
//! each unpaired surrogate becomes '?', and caret columns count UTF-16 units. The expected text
//! was produced by the reference jar (crates/jscomp/tests/java/SurrogateExcerpt.java).
use closure_jscomp::{
    check_level::CheckLevel, diagnostic_type::DiagnosticType, error_format::ErrorFormat,
    error_handler::ErrorHandler, error_manager::ErrorManager, js_error::JSError,
    print_stream_error_manager::PrintStreamErrorManager, region::Region,
    source_excerpt_provider::SourceExcerptProvider, source_file::SourceFile,
    sourcemap_mapping_placeholder::OriginalMapping,
};
use closure_rhino::{java_lang::charset::utf8_encoded_text, js_string::JsString, node::Ast};
use std::sync::Arc;

static T: DiagnosticType = DiagnosticType::error("JSC_T", "bad {0}");

struct Provider(SourceFile);
impl SourceExcerptProvider for Provider {
    // port: SurrogateExcerpt.SourceExcerptProvider#getSourceLine
    fn get_source_line(&self, _name: Option<&str>, line: i32) -> Option<String> {
        self.0.get_line(line)
    }
    // port: SurrogateExcerpt.SourceExcerptProvider#getSourceLines
    fn get_source_lines(
        &self,
        _name: Option<&str>,
        line: i32,
        length: i32,
    ) -> Option<Box<dyn Region>> {
        self.0
            .get_lines(line, length)
            .map(|r| Box::new(r) as Box<dyn Region>)
    }
    // port: SurrogateExcerpt.SourceExcerptProvider#getSourceRegion
    fn get_source_region(&self, _name: Option<&str>, line: i32) -> Option<Box<dyn Region>> {
        self.0
            .get_region(line)
            .map(|r| Box::new(r) as Box<dyn Region>)
    }
    // port: SurrogateExcerpt.SourceExcerptProvider#getSourceMapping
    fn get_source_mapping(
        &self,
        _name: Option<&str>,
        _line: i32,
        _column: i32,
    ) -> Option<OriginalMapping> {
        None
    }
}

// port: SurrogateExcerpt#main
#[test]
fn lone_surrogates_print_as_question_marks() {
    let mut code: Vec<u16> = "var x = '".encode_utf16().collect();
    code.push(0xd800);
    code.extend("';\nvar y = '".encode_utf16());
    code.extend([0xdc00, 0xd83d, 0xde00]);
    code.extend("z';\n".encode_utf16());
    let file = SourceFile::from_code("a.js", JsString::from_units(code));
    assert_eq!(file.get_line(1).as_deref(), Some("var x = '?';"));
    let provider: Arc<dyn SourceExcerptProvider> = Arc::new(Provider(file));
    let lone = utf8_encoded_text(&[0xd800]);
    let ast = Ast::new();
    let mut out = Vec::new();
    for format in [
        ErrorFormat::SINGLELINE,
        ErrorFormat::FULL,
        ErrorFormat::MULTILINE,
    ] {
        let mut m = PrintStreamErrorManager::new(
            format.to_formatter(Some(provider.clone()), false),
            Vec::new(),
        );
        m.report(
            CheckLevel::ERROR,
            JSError::make_with_source_location("a.js", 1, 8, &T, &[&lone]),
        );
        m.report(
            CheckLevel::WARNING,
            JSError::make_with_source_location("a.js", 2, 12, &T, &["q"]),
        );
        m.generate_report(&ast);
        out.extend(m.stream);
    }
    let expected = "\
a.js:2:12: WARNING - [JSC_T] bad q
var y = '?\u{1F600}z';
            ^

a.js:1:8: ERROR - [JSC_T] bad ?
var x = '?';
        ^

1 error(s), 1 warning(s)
a.js:2:12: WARNING - [JSC_T] bad q
  2| var y = '?\u{1F600}z';
                 ^

a.js:1:8: ERROR - [JSC_T] bad ?
  1| var x = '?';
             ^

1 error(s), 1 warning(s)
a.js:2:12: WARNING - [JSC_T] bad q
  1| var x = '?';
  2| var y = '?\u{1F600}z';

a.js:1:8: ERROR - [JSC_T] bad ?
  1| var x = '?';
  2| var y = '?\u{1F600}z';

1 error(s), 1 warning(s)
";
    assert_eq!(String::from_utf8(out).unwrap(), expected);
}

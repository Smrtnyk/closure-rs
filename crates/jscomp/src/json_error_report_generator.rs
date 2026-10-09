/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/JsonErrorReportGenerator.java.

use crate::{
    check_level::CheckLevel,
    error_manager::ErrorManager,
    lightweight_message_formatter::LineNumberingFormatter,
    sorting_error_manager::{ErrorReportGenerator, SortingErrorManager},
    source_excerpt_provider::{SourceExcerpt, SourceExcerptProvider},
};
use closure_rhino::{
    java_lang::formatter::format_one_decimal, js_string::JsString, node::Ast, token_util::TokenUtil,
};
use closure_sourcemap::gson::stream::json_writer::JsonWriter;
use std::{io::Write, sync::Arc};

/// An error report generator that prints error and warning data to the print stream as an array
/// of JSON objects.
pub struct JsonErrorReportGenerator {
    stream: Box<dyn Write + Send>,
    source_excerpt_provider: Arc<dyn SourceExcerptProvider>,
}
static EXCERPT_FORMATTER: LineNumberingFormatter = LineNumberingFormatter;
impl JsonErrorReportGenerator {
    // port: JsonErrorReportGenerator#JsonErrorReportGenerator
    pub fn new(
        stream: Box<dyn Write + Send>,
        source_excerpt_provider: Arc<dyn SourceExcerptProvider>,
    ) -> Self {
        Self {
            stream,
            source_excerpt_provider,
        }
    }
}
impl ErrorReportGenerator for JsonErrorReportGenerator {
    // port: JsonErrorReportGenerator#generateReport
    fn generate_report(&mut self, manager: &mut SortingErrorManager, _ast: &Ast) {
        let mut json_writer = JsonWriter::new(Vec::new());
        json_writer.begin_array();
        for message in manager.get_sorted_diagnostics() {
            let source_name = message.error.source_name();
            let line_number = message.error.get_line_number();
            let charno = message.error.charno();

            json_writer.begin_object();
            json_writer.name("level").value(Some(JsString::from(
                if message.level == CheckLevel::ERROR {
                    "error"
                } else {
                    "warning"
                },
            )));
            json_writer
                .name("description")
                .value(Some(JsString::from(message.error.description())));
            json_writer
                .name("key")
                .value(Some(JsString::from(message.error.get_type().key)));
            if let Some(requirement) = message.error.requirement() {
                json_writer.name("requirement").begin_object();
                json_writer
                    .name("ruleId")
                    .value(Some(JsString::from(requirement.get_rule_id())));
                json_writer.name("configFiles").begin_array();
                for config_file in requirement.get_config_file_list() {
                    json_writer.value(Some(JsString::from(config_file.as_str())));
                }
                json_writer.end_array();
                json_writer.end_object();
            }
            json_writer
                .name("source")
                .value(source_name.map(JsString::from));
            json_writer.name("line").value_long(i64::from(line_number));
            json_writer.name("column").value_long(i64::from(charno));
            let node = message.error.node();
            let region_length = message.error.length();
            if node.is_some() && region_length > 0 {
                json_writer
                    .name("length")
                    .value_long(i64::from(message.error.length()));
            }

            // extract source excerpt
            let source_excerpt = SourceExcerpt::LINE.get_without_length(
                &*self.source_excerpt_provider,
                source_name,
                line_number,
                &EXCERPT_FORMATTER,
            );
            if let Some(source_excerpt) = source_excerpt {
                let source_excerpt = JsString::from(source_excerpt);
                let mut b: Vec<u16> = source_excerpt.as_units().to_vec();
                b.push(u16::from(b'\n'));

                // padding equal to the excerpt and arrow at the end
                // charno == sourceExcerpt.length() means something is missing
                // at the end of the line
                if 0 <= charno && charno as usize <= source_excerpt.length() {
                    for i in 0..charno as usize {
                        let c = source_excerpt.char_at(i);
                        if TokenUtil::is_whitespace(i32::from(c)) {
                            b.push(c);
                        } else {
                            b.push(u16::from(b' '));
                        }
                    }
                    if node.is_none() {
                        b.push(u16::from(b'^'));
                    } else {
                        let length =
                            1.max(region_length.min(source_excerpt.length() as i32 - charno));
                        for _i in 0..length {
                            b.push(u16::from(b'^'));
                        }
                    }
                }

                json_writer
                    .name("context")
                    .value(Some(JsString::from_units(b)));
            }

            let mapping = self.source_excerpt_provider.get_source_mapping(
                source_name,
                message.error.get_line_number(),
                message.error.charno(),
            );

            if let Some(mapping) = mapping {
                json_writer.name("originalLocation").begin_object();
                json_writer
                    .name("source")
                    .value(Some(mapping.get_original_file()));
                json_writer
                    .name("line")
                    .value_long(i64::from(mapping.get_line_number()));
                json_writer
                    .name("column")
                    .value_long(i64::from(mapping.get_column_position()));
                json_writer.end_object();
            }

            json_writer.end_object();
        }
        // Java: getSourceMapping's reports reached the manager during the loop above; they
        // count in the summary but are not in the array.
        manager.report_deferred();

        let mut summary_builder = String::new();
        if manager.get_typed_percent() > 0.0 {
            summary_builder.push_str(&format!(
                "{} error(s), {} warning(s), {}% typed",
                manager.get_error_count(),
                manager.get_warning_count(),
                format_one_decimal(manager.get_typed_percent())
            ));
        } else {
            summary_builder.push_str(&format!(
                "{} error(s), {} warning(s)",
                manager.get_error_count(),
                manager.get_warning_count()
            ));
        }
        json_writer.begin_object();
        json_writer
            .name("level")
            .value(Some(JsString::from("info")));
        json_writer
            .name("description")
            .value(Some(JsString::from(summary_builder)));
        json_writer.end_object();

        json_writer.end_array();
        let buffered_stream =
            closure_rhino::java_lang::string::get_bytes_utf8(&json_writer.into_string());
        let _ = self.stream.write_all(&buffered_stream);
        let _ = self.stream.flush();
    }
}

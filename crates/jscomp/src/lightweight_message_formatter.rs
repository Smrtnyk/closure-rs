/*
 * Copyright 2007 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AbstractMessageFormatter.java,
//   src/com/google/javascript/jscomp/LightweightMessageFormatter.java.

use crate::{
    abstract_message_formatter::AbstractMessageFormatter,
    check_level::CheckLevel,
    js_error::JSError,
    message_formatter::MessageFormatter,
    region::Region,
    source_excerpt_provider::{ExcerptFormatter, SourceExcerpt, SourceExcerptProvider},
};
use closure_rhino::{
    node::{Ast, NodeId},
    token_util::TokenUtil,
};
use std::sync::Arc;
pub struct LightweightMessageFormatter {
    base: AbstractMessageFormatter,
    default_format: SourceExcerpt,
    include_location: bool,
    include_level: bool,
}
impl LightweightMessageFormatter {
    // port: LightweightMessageFormatter#LightweightMessageFormatter(SourceExcerptProvider)
    pub fn new(source: Arc<dyn SourceExcerptProvider>) -> Self {
        Self::new_with_format(source, SourceExcerpt::LINE)
    }
    // port: LightweightMessageFormatter#LightweightMessageFormatter(SourceExcerptProvider,SourceExcerpt)
    pub fn new_with_format(
        source: Arc<dyn SourceExcerptProvider>,
        default_format: SourceExcerpt,
    ) -> Self {
        Self {
            base: AbstractMessageFormatter::new(Some(source)),
            default_format,
            include_location: true,
            include_level: true,
        }
    }
    // port: LightweightMessageFormatter#withoutSource
    // port: LightweightMessageFormatter#LightweightMessageFormatter()
    pub fn without_source() -> Self {
        Self {
            base: AbstractMessageFormatter::new(None),
            default_format: SourceExcerpt::LINE,
            include_location: true,
            include_level: true,
        }
    }
    // port: AbstractMessageFormatter#setColorize
    pub fn set_colorize(&mut self, colorize: bool) {
        self.base.set_colorize(colorize);
    }
    // port: LightweightMessageFormatter#setIncludeLocation
    pub fn set_include_location(&mut self, include_location: bool) -> &mut Self {
        self.include_location = include_location;
        self
    }
    // port: LightweightMessageFormatter#setIncludeLevel
    pub fn set_include_level(&mut self, include_level: bool) -> &mut Self {
        self.include_level = include_level;
        self
    }
    // port: LightweightMessageFormatter#format
    fn format(&self, ast: &Ast, error: &JSError, warning: bool) -> String {
        let source = self.base.get_source();
        let mut source_name = error.source_name().map(str::to_owned);
        let mut line_number = error.get_line_number();
        let mut charno = error.charno();
        let mut b = String::new();
        let mut bold_line = String::new();
        let mapping = source.and_then(|s| {
            s.get_source_mapping(error.source_name(), error.get_line_number(), error.charno())
        });
        if self.include_location {
            if let Some(mapping) = &mapping {
                Self::append_position(&mut b, source_name.as_deref(), line_number, charno);
                source_name = Some(mapping.get_original_file().to_string_lossy());
                line_number = mapping.get_line_number();
                charno = mapping.get_column_position();
                b.push_str("\nOriginally at:\n");
            }
            Self::append_position(&mut bold_line, source_name.as_deref(), line_number, charno);
        }
        if self.include_level {
            bold_line.push_str(&self.base.get_level_name(if warning {
                CheckLevel::WARNING
            } else {
                CheckLevel::ERROR
            }));
            bold_line.push_str(" - [");
            bold_line.push_str(error.get_type().key);
            bold_line.push_str("] ");
        }
        bold_line.push_str(error.description());
        b.push_str(&self.base.maybe_embolden(&bold_line));
        b.push('\n');
        b.push_str(&self.get_excerpt_with_position_with_format(
            ast,
            error,
            source_name.as_deref(),
            line_number,
            charno,
            if mapping.is_some() {
                SourceExcerpt::LINE
            } else {
                self.default_format
            },
        ));
        b
    }
    // port: LightweightMessageFormatter#getExcerptWithPosition(JSError)
    pub fn get_excerpt_with_position(&self, ast: &Ast, error: &JSError) -> String {
        self.get_excerpt_with_position_with_length(
            ast,
            error,
            error.source_name(),
            error.get_line_number(),
            error.charno(),
            error.length(),
            self.default_format,
        )
    }
    // port: LightweightMessageFormatter#getExcerptWithPosition(JSError,String,int,int,SourceExcerpt)
    fn get_excerpt_with_position_with_format(
        &self,
        ast: &Ast,
        error: &JSError,
        source_name: Option<&str>,
        line_number: i32,
        charno: i32,
        format: SourceExcerpt,
    ) -> String {
        self.get_excerpt_with_position_with_length(
            ast,
            error,
            source_name,
            line_number,
            charno,
            error.length(),
            format,
        )
    }
    // port: LightweightMessageFormatter#getExcerptWithPosition(JSError,String,int,int,int,SourceExcerpt)
    #[allow(clippy::too_many_arguments)]
    fn get_excerpt_with_position_with_length(
        &self,
        ast: &Ast,
        error: &JSError,
        source_name: Option<&str>,
        line_number: i32,
        charno: i32,
        length: i32,
        format: SourceExcerpt,
    ) -> String {
        let mut b = String::new();
        let cooked_length = if charno >= 0 && length >= 0 {
            charno.wrapping_add(length)
        } else {
            -1
        };
        let source_excerpt = self.base.get_source().and_then(|source| {
            format.get(
                source.as_ref(),
                source_name,
                line_number,
                cooked_length,
                &LineNumberingFormatter,
            )
        });
        let Some(source_excerpt) = source_excerpt else {
            return String::new();
        };
        if self.pad_source_excerpt(ast, error, &source_excerpt, charno, length, format, &mut b) {
            b
        } else {
            "Source excerpt could not be formatted. This may indicate a bug in the compiler.".into()
        }
    }
    // port: LightweightMessageFormatter#padSourceExcerpt
    #[allow(clippy::too_many_arguments)]
    fn pad_source_excerpt(
        &self,
        ast: &Ast,
        error: &JSError,
        source_excerpt: &str,
        charno: i32,
        length: i32,
        format: SourceExcerpt,
        b: &mut String,
    ) -> bool {
        if format == SourceExcerpt::FULL {
            if charno >= 0 {
                if !self.pad_multiple_lines(ast, charno, source_excerpt, b, error.node()) {
                    return false;
                }
            } else {
                b.push_str(source_excerpt);
                b.push('\n');
            }
        } else {
            b.push_str(source_excerpt);
            b.push('\n');
            if format == SourceExcerpt::LINE
                && charno >= 0
                && charno as usize <= source_excerpt.encode_utf16().count()
                && !self.pad_line(charno, source_excerpt, b, length, error.node())
            {
                return false;
            }
        }
        true
    }
    // port: LightweightMessageFormatter#appendPosition
    fn append_position(b: &mut String, source_name: Option<&str>, line_number: i32, charno: i32) {
        if let Some(source_name) = source_name {
            b.push_str(source_name);
            if line_number > 0 {
                b.push(':');
                b.push_str(&line_number.to_string());
                if charno >= 0 {
                    b.push(':');
                    b.push_str(&charno.to_string());
                }
            }
            b.push_str(": ");
        }
    }
    // port: LightweightMessageFormatter#padLine
    fn pad_line(
        &self,
        charno: i32,
        source_excerpt: &str,
        b: &mut String,
        err_length: i32,
        error_node: Option<NodeId>,
    ) -> bool {
        let units: Vec<u16> = source_excerpt.encode_utf16().collect();
        if charno > units.len() as i32 {
            return false;
        }
        let whitespace: Vec<u16> = units[..charno as usize]
            .iter()
            .map(|&c| {
                if TokenUtil::is_whitespace(c as i32) {
                    c
                } else {
                    32
                }
            })
            .collect();
        b.push_str(&String::from_utf16_lossy(&whitespace));
        let length = if error_node.is_none() {
            1
        } else {
            1.max(err_length.min(units.len() as i32 - charno))
        };
        b.push_str(&"^".repeat(length as usize));
        b.push('\n');
        true
    }
    // port: LightweightMessageFormatter#padMultipleLines
    fn pad_multiple_lines(
        &self,
        ast: &Ast,
        start_charno: i32,
        source_excerpt: &str,
        b: &mut String,
        error_node: Option<NodeId>,
    ) -> bool {
        if error_node.is_none() {
            b.push_str(source_excerpt);
            b.push('\n');
            let pipe = source_excerpt
                .encode_utf16()
                .position(|c| c == 124)
                .map_or(-1, |n| n as i32);
            return self.pad_line(
                start_charno.wrapping_add(pipe).wrapping_add(2),
                source_excerpt,
                b,
                -1,
                None,
            );
        }
        // Splitter.on('\\n') keeps empty pieces, and indexing below counts UTF-16 units.
        let lines: Vec<&str> = source_excerpt.split('\n').collect();
        let requires_truncation = lines.len() > 4;
        let truncation_start = 2;
        let truncation_end = lines.len().saturating_sub(2);
        let mut remaining_length = error_node.unwrap().get_length(ast);
        let mut charno = start_charno;
        for (i, line) in lines.iter().enumerate() {
            if requires_truncation && i == truncation_start {
                b.push_str("...\n");
            }
            let should_print_line =
                !requires_truncation || i < truncation_start || i >= truncation_end;
            let pipe = line
                .encode_utf16()
                .position(|c| c == 124)
                .map_or(-1, |n| n as i32);
            let char_with_line_number_offset = charno.wrapping_add(pipe).wrapping_add(2);
            if should_print_line {
                b.push_str(line);
                b.push('\n');
                if !self.pad_line(
                    char_with_line_number_offset,
                    line,
                    b,
                    remaining_length,
                    error_node,
                ) {
                    return false;
                }
            }
            remaining_length = remaining_length.wrapping_sub(
                (line.encode_utf16().count() as i32)
                    .wrapping_add(1)
                    .wrapping_sub(char_with_line_number_offset),
            );
            charno = 0;
        }
        true
    }
}
impl MessageFormatter for LightweightMessageFormatter {
    // port: LightweightMessageFormatter#formatError
    fn format_error(&self, ast: &Ast, error: &JSError) -> String {
        self.format(ast, error, false)
    }
    // port: LightweightMessageFormatter#formatWarning
    fn format_warning(&self, ast: &Ast, warning: &JSError) -> String {
        self.format(ast, warning, true)
    }
}
pub struct LineNumberingFormatter;
impl ExcerptFormatter for LineNumberingFormatter {
    // port: LightweightMessageFormatter.LineNumberingFormatter#formatLine
    fn format_line(&self, line: Option<&str>, _line_number: i32) -> Option<String> {
        line.map(str::to_owned)
    }
    // port: LightweightMessageFormatter.LineNumberingFormatter#formatRegion
    fn format_region(&self, region: Option<&dyn Region>) -> Option<String> {
        let region = region?;
        let code: Vec<u16> = region.get_source_excerpt().encode_utf16().collect();
        if code.is_empty() {
            return None;
        }
        let number_length = region.get_ending_line_number().to_string().len();
        let mut builder = Vec::<u16>::new();
        let mut start = 0;
        let mut line_number = region.get_beginning_line_number();
        loop {
            let end = code[start..]
                .iter()
                .position(|&c| c == 10)
                .map(|n| start + n);
            let line = if let Some(end) = end {
                &code[start..end]
            } else {
                &code[start..]
            };
            if end.is_none() && line.is_empty() {
                builder.pop();
                return Some(String::from_utf16_lossy(&builder));
            }
            builder.extend("  ".encode_utf16());
            let spaces = number_length as i32 - line_number.to_string().len() as i32;
            if spaces < 0 {
                panic!("count is negative: {spaces}");
            }
            builder.extend(std::iter::repeat_n(32, spaces as usize));
            builder.extend(line_number.to_string().encode_utf16());
            builder.extend("| ".encode_utf16());
            builder.extend(line);
            if let Some(end) = end {
                builder.push(10);
                start = end + 1;
                line_number += 1;
            } else {
                break;
            }
        }
        Some(String::from_utf16_lossy(&builder))
    }
}

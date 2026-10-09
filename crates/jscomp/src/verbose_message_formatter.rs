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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/AbstractMessageFormatter.java,
//   src/com/google/javascript/jscomp/VerboseMessageFormatter.java.

use crate::{
    abstract_message_formatter::AbstractMessageFormatter, check_level::CheckLevel,
    js_error::JSError, message_formatter::MessageFormatter,
    source_excerpt_provider::SourceExcerptProvider,
};
use closure_rhino::node::Ast;
use std::sync::Arc;
pub struct VerboseMessageFormatter {
    base: AbstractMessageFormatter,
}
impl VerboseMessageFormatter {
    // port: VerboseMessageFormatter#VerboseMessageFormatter
    pub fn new(source: Arc<dyn SourceExcerptProvider>) -> Self {
        Self {
            base: AbstractMessageFormatter::new(Some(source)),
        }
    }
    // port: AbstractMessageFormatter#setColorize
    pub fn set_colorize(&mut self, colorize: bool) {
        self.base.set_colorize(colorize);
    }
    // port: VerboseMessageFormatter#format
    fn format(&self, message: &JSError) -> String {
        let source_name = message.source_name();
        let line_number = message.get_line_number();
        let region = self
            .base
            .get_source()
            .unwrap()
            .get_source_region(source_name, line_number);
        let source = source_name
            .filter(|s| !s.is_empty())
            .unwrap_or("(unknown source)");
        // Preserve Java's counterintuitive condition: nonnegative line numbers
        // print "(unknown line)" in this formatter.
        let line = if line_number < 0 {
            line_number.to_string()
        } else {
            "(unknown line)".into()
        };
        let excerpt = region.map_or(".".into(), |r| format!(":\n\n{}", r.get_source_excerpt()));
        format!(
            "{} at {source} line {line} {excerpt}",
            message.description()
        )
    }
}
impl MessageFormatter for VerboseMessageFormatter {
    // port: VerboseMessageFormatter#formatError
    fn format_error(&self, _ast: &Ast, error: &JSError) -> String {
        format!(
            "{}: {}",
            self.base.get_level_name(CheckLevel::ERROR),
            self.format(error)
        )
    }
    // port: VerboseMessageFormatter#formatWarning
    fn format_warning(&self, _ast: &Ast, warning: &JSError) -> String {
        format!(
            "{}: {}",
            self.base.get_level_name(CheckLevel::WARNING),
            self.format(warning)
        )
    }
}

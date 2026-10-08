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
//   src/com/google/javascript/jscomp/PrintStreamErrorManager.java.

use crate::{
    basic_error_manager::BasicErrorManager, check_level::CheckLevel, error_manager::ErrorManager,
    js_error::JSError, lightweight_message_formatter::LightweightMessageFormatter,
    message_formatter::MessageFormatter, sorting_error_manager::SortingErrorManager,
};
use closure_rhino::{java_lang::formatter::format_one_decimal, node::Ast};
use std::io::Write;
pub struct PrintStreamErrorManager<W: Write + Send> {
    manager: SortingErrorManager,
    formatter: Box<dyn MessageFormatter>,
    pub stream: W,
    summary_detail_level: i32,
}
impl<W: Write + Send> PrintStreamErrorManager<W> {
    // port: PrintStreamErrorManager#PrintStreamErrorManager(MessageFormatter,PrintStream)
    pub fn new(formatter: Box<dyn MessageFormatter>, stream: W) -> Self {
        Self {
            manager: Self::new_base(),
            formatter,
            stream,
            summary_detail_level: 1,
        }
    }
    // port: PrintStreamErrorManager#PrintStreamErrorManager(PrintStream)
    pub fn new_without_source(stream: W) -> Self {
        Self::new(
            Box::new(LightweightMessageFormatter::without_source()),
            stream,
        )
    }
    // port: PrintStreamErrorManager#setSummaryDetailLevel
    pub fn set_summary_detail_level(&mut self, level: i32) {
        self.summary_detail_level = level;
    }
}
impl<W: Write + Send> BasicErrorManager for PrintStreamErrorManager<W> {
    fn sorting_manager(&self) -> &SortingErrorManager {
        &self.manager
    }
    fn sorting_manager_mut(&mut self) -> &mut SortingErrorManager {
        &mut self.manager
    }
    // port: PrintStreamErrorManager#println
    fn println(&mut self, ast: &Ast, level: CheckLevel, error: &JSError) {
        let message = error
            .format(ast, level, self.formatter.as_ref())
            .unwrap_or_else(|| "null".into());
        let _ = writeln!(self.stream, "{message}");
    }
    // port: PrintStreamErrorManager#printSummary
    fn print_summary(&mut self) {
        if self.summary_detail_level >= 3
            || self.summary_detail_level >= 1
                && self.get_error_count() + self.get_warning_count() > 0
            || self.summary_detail_level >= 2 && self.get_typed_percent() > 0.0
        {
            if self.get_typed_percent() > 0.0 {
                let _ = writeln!(
                    self.stream,
                    "{} error(s), {} warning(s), {}% typed",
                    self.get_error_count(),
                    self.get_warning_count(),
                    format_one_decimal(self.get_typed_percent())
                );
            } else {
                let _ = writeln!(
                    self.stream,
                    "{} error(s), {} warning(s)",
                    self.get_error_count(),
                    self.get_warning_count()
                );
            }
        }
    }
}

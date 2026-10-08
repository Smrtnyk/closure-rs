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
//   src/com/google/javascript/jscomp/PrintStreamErrorReportGenerator.java.

use crate::{
    check_level::CheckLevel,
    error_manager::ErrorManager,
    js_error::JSError,
    message_formatter::MessageFormatter,
    sorting_error_manager::{ErrorReportGenerator, SortingErrorManager},
};
use closure_rhino::{java_lang::formatter::format_one_decimal, node::Ast};
use std::io::Write;
pub struct PrintStreamErrorReportGenerator<W: Write + Send> {
    formatter: Box<dyn MessageFormatter>,
    pub stream: W,
    summary_detail_level: i32,
}
impl<W: Write + Send> PrintStreamErrorReportGenerator<W> {
    // port: PrintStreamErrorReportGenerator#PrintStreamErrorReportGenerator
    pub fn new(formatter: Box<dyn MessageFormatter>, stream: W, summary_detail_level: i32) -> Self {
        Self {
            formatter,
            stream,
            summary_detail_level,
        }
    }
    // port: PrintStreamErrorReportGenerator#println
    fn println(&mut self, ast: &Ast, level: CheckLevel, error: &JSError) {
        let message = error
            .format(ast, level, self.formatter.as_ref())
            .unwrap_or_else(|| "null".into());
        let _ = writeln!(self.stream, "{message}");
    }
    // port: PrintStreamErrorReportGenerator#printSummary
    fn print_summary(&mut self, manager: &dyn ErrorManager) {
        if self.summary_detail_level >= 3
            || self.summary_detail_level >= 1
                && manager.get_error_count() + manager.get_warning_count() > 0
            || self.summary_detail_level >= 2 && manager.get_typed_percent() > 0.0
        {
            if manager.get_typed_percent() > 0.0 {
                let _ = writeln!(
                    self.stream,
                    "{} error(s), {} warning(s), {}% typed",
                    manager.get_error_count(),
                    manager.get_warning_count(),
                    format_one_decimal(manager.get_typed_percent())
                );
            } else {
                let _ = writeln!(
                    self.stream,
                    "{} error(s), {} warning(s)",
                    manager.get_error_count(),
                    manager.get_warning_count()
                );
            }
        }
    }
}
impl<W: Write + Send> ErrorReportGenerator for PrintStreamErrorReportGenerator<W> {
    // port: PrintStreamErrorReportGenerator#generateReport
    fn generate_report(&mut self, manager: &SortingErrorManager, ast: &Ast) {
        for e in manager.get_sorted_diagnostics() {
            self.println(ast, e.level, &e.error);
        }
        self.print_summary(manager);
    }
}

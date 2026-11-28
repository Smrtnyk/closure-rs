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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
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
    fn println(
        formatter: &dyn MessageFormatter,
        stream: &mut impl Write,
        ast: &Ast,
        level: CheckLevel,
        error: &JSError,
    ) {
        let message = error
            .format(ast, level, formatter)
            .unwrap_or_else(|| "null".into());
        let _ = writeln!(stream, "{message}");
    }
    // port: PrintStreamErrorReportGenerator#printSummary
    fn print_summary(
        summary_detail_level: i32,
        stream: &mut impl Write,
        manager: &dyn ErrorManager,
    ) {
        if summary_detail_level >= 3
            || summary_detail_level >= 1
                && manager.get_error_count() + manager.get_warning_count() > 0
            || summary_detail_level >= 2 && manager.get_typed_percent() > 0.0
        {
            if manager.get_typed_percent() > 0.0 {
                let _ = writeln!(
                    stream,
                    "{} error(s), {} warning(s), {}% typed",
                    manager.get_error_count(),
                    manager.get_warning_count(),
                    format_one_decimal(manager.get_typed_percent())
                );
            } else {
                let _ = writeln!(
                    stream,
                    "{} error(s), {} warning(s)",
                    manager.get_error_count(),
                    manager.get_warning_count()
                );
            }
        }
    }
}
/// Rust-only (performance divergence): the text of one report, written to the generator's
/// stream in one piece when the report ends or a message's formatting panics (Java prints each
/// message and each line end as it goes; on a stream that is not buffered, such as stderr, that is
/// two system calls per message). The stream receives the same bytes in the same order, and
/// nothing else is written while a report is generated, so no other output can come between them.
struct ReportText<'a, W: Write> {
    stream: &'a mut W,
    text: Vec<u8>,
}
impl<W: Write> Drop for ReportText<'_, W> {
    fn drop(&mut self) {
        if !self.text.is_empty() {
            let _ = self.stream.write_all(&self.text);
        }
    }
}
impl<W: Write + Send> ErrorReportGenerator for PrintStreamErrorReportGenerator<W> {
    // port: PrintStreamErrorReportGenerator#generateReport
    fn generate_report(&mut self, manager: &mut SortingErrorManager, ast: &Ast) {
        let mut out = ReportText {
            stream: &mut self.stream,
            text: Vec::new(),
        };
        for e in manager.get_sorted_diagnostics() {
            Self::println(
                self.formatter.as_ref(),
                &mut out.text,
                ast,
                e.level,
                &e.error,
            );
        }
        // Java: the formatter's reports (an unresolvable input source map) reached the manager
        // during the loop above; they count in the summary but are not printed.
        manager.report_deferred();
        Self::print_summary(self.summary_detail_level, &mut out.text, manager);
    }
}

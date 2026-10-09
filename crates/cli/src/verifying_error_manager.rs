/*
 * Copyright 2017 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/ThreadSafeDelegatingErrorManager.java,
//   src/com/google/javascript/jscomp/VerifyingErrorManager.java.

//! VerifyingErrorManager lives in the cli crate because it reports
//! AbstractCommandLineRunner's diagnostic types, and closure-jscomp cannot depend on closure-cli.
use crate::abstract_command_line_runner::{AMBIGUOUS_EXPECTATION, EXPECTED_DIAGNOSTIC_NOT_FOUND};
use closure_jscomp::{
    check_level::CheckLevel, deps::js_file_line_parser::SharedErrorManager,
    error_handler::ErrorHandler, error_manager::ErrorManager, js_error::JSError,
    lightweight_message_formatter::LightweightMessageFormatter,
    message_formatter::MessageFormatter,
};
use closure_rhino::{java_lang::pattern::Pattern, node::Ast};

/// An ErrorManager that verifies expected diagnostics and reports missing or unexpected ones.
///
/// It wraps a delegate ErrorManager. Expected diagnostics (passed as regexes) are swallowed if
/// matched. Unexpected diagnostics are passed to the delegate. At the end of compilation (when
/// generateReport is called), any unmatched expectations are reported as errors to the delegate.
pub struct VerifyingErrorManager {
    // ThreadSafeDelegatingErrorManager's delegated field (the Java superclass).
    delegated: SharedErrorManager,
    unmatched_expectations: Vec<Pattern>,
    formatter: LightweightMessageFormatter,
    // A formatter without source never reads the AST; the Rust formatter API takes one.
    no_ast: Ast,
}
impl VerifyingErrorManager {
    // port: VerifyingErrorManager#VerifyingErrorManager
    pub fn new(delegate: SharedErrorManager, expected_diagnostics: Vec<String>) -> Self {
        let formatter = LightweightMessageFormatter::without_source();
        let mut unmatched_expectations = Vec::new();
        for exp in &expected_diagnostics {
            unmatched_expectations.push(Pattern::compile(exp));
        }
        Self {
            delegated: delegate,
            unmatched_expectations,
            formatter,
            no_ast: Ast::new(),
        }
    }
    // port: ThreadSafeDelegatingErrorManager#report
    fn super_report(&self, level: CheckLevel, error: JSError) {
        self.delegated.lock().unwrap().report(level, error);
    }
    // port: VerifyingErrorManager#reportMissingExpectations
    fn report_missing_expectations(&mut self) {
        for p in &self.unmatched_expectations {
            self.super_report(
                CheckLevel::ERROR,
                JSError::make_without_location(&EXPECTED_DIAGNOSTIC_NOT_FOUND, &[&p.to_string()]),
            );
        }
        self.unmatched_expectations.clear();
    }
    // port: VerifyingErrorManager#formatErrorForMatching
    fn format_error_for_matching(&self, level: CheckLevel, error: &JSError) -> String {
        if level == CheckLevel::ERROR {
            self.formatter.format_error(&self.no_ast, error)
        } else {
            self.formatter.format_warning(&self.no_ast, error)
        }
    }
}
impl ErrorHandler for VerifyingErrorManager {
    // port: VerifyingErrorManager#report
    fn report(&mut self, level: CheckLevel, error: JSError) {
        let formatted_error = self.format_error_for_matching(level, &error);

        let mut matched_pattern: Option<Pattern> = None;
        let mut i = 0;
        while i < self.unmatched_expectations.len() {
            let p = self.unmatched_expectations[i].clone();
            if p.matcher(&formatted_error).find() {
                let Some(matched) = &matched_pattern else {
                    matched_pattern = Some(p);
                    self.unmatched_expectations.remove(i);
                    continue;
                };

                if matched.to_string() == p.to_string() {
                    // Allow duplicates that are identical.
                    i += 1;
                    continue;
                }

                self.super_report(
                    CheckLevel::ERROR,
                    JSError::make_without_location(
                        &AMBIGUOUS_EXPECTATION,
                        &[&formatted_error, &matched.to_string(), &p.to_string()],
                    ),
                );
                self.unmatched_expectations.remove(i);
                continue;
            }
            i += 1;
        }

        if matched_pattern.is_none() {
            self.super_report(level, error);
        }
    }
}
impl ErrorManager for VerifyingErrorManager {
    // port: VerifyingErrorManager#generateReport
    fn generate_report(&mut self, ast: &Ast) {
        self.report_missing_expectations();
        self.delegated.lock().unwrap().generate_report(ast);
    }
    // port: ThreadSafeDelegatingErrorManager#getErrorCount
    fn get_error_count(&self) -> i32 {
        self.delegated.lock().unwrap().get_error_count()
    }
    // port: ThreadSafeDelegatingErrorManager#getWarningCount
    fn get_warning_count(&self) -> i32 {
        self.delegated.lock().unwrap().get_warning_count()
    }
    // port: ThreadSafeDelegatingErrorManager#getErrors
    fn get_errors(&self) -> Vec<JSError> {
        self.delegated.lock().unwrap().get_errors()
    }
    // port: ThreadSafeDelegatingErrorManager#getWarnings
    fn get_warnings(&self) -> Vec<JSError> {
        self.delegated.lock().unwrap().get_warnings()
    }
    // port: ThreadSafeDelegatingErrorManager#setTypedPercent
    fn set_typed_percent(&mut self, typed_percent: f64) {
        self.delegated
            .lock()
            .unwrap()
            .set_typed_percent(typed_percent);
    }
    // port: ThreadSafeDelegatingErrorManager#getTypedPercent
    fn get_typed_percent(&self) -> f64 {
        self.delegated.lock().unwrap().get_typed_percent()
    }
    // port: ThreadSafeDelegatingErrorManager#hasHaltingErrors
    fn has_halting_errors(&self) -> bool {
        self.delegated.lock().unwrap().has_halting_errors()
    }
}

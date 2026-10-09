/*
 * Copyright 2007 The Closure Compiler Authors.
 * Copyright 2015 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/BasicErrorManager.java,
//   src/com/google/javascript/jscomp/SortingErrorManager.java,
//   src/com/google/javascript/jscomp/testing/TestErrorManager.java.

//! Port of testing/TestErrorManager.java: an error manager that asserts the errors and warnings
//! it receives against the expected ones.
//!
//! Java extends BasicErrorManager and overrides `report`; closure-jscomp's BasicErrorManager trait
//! supplies `report` through a blanket impl, so this type implements ErrorHandler/ErrorManager
//! itself with the same SortingErrorManager base, forwarding exactly as the blanket impls do.
use closure_jscomp::check_level::CheckLevel;
use closure_jscomp::error_handler::ErrorHandler;
use closure_jscomp::error_manager::ErrorManager;
use closure_jscomp::js_error::JSError;
use closure_jscomp::sorting_error_manager::SortingErrorManager;
use closure_rhino::node::Ast;

// port: TestErrorManager
pub struct TestErrorManager {
    base: SortingErrorManager,
    error_index: usize,
    errors: Vec<String>,
    warning_index: usize,
    warnings: Vec<String>,
}

impl Default for TestErrorManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TestErrorManager {
    // port: TestErrorManager#TestErrorManager (BasicErrorManager#BasicErrorManager)
    pub fn new() -> Self {
        Self {
            base: SortingErrorManager::new(Vec::new()),
            error_index: 0,
            errors: Vec::new(),
            warning_index: 0,
            warnings: Vec::new(),
        }
    }

    // port: TestErrorManager#expectErrors
    pub fn expect_errors(&mut self, expected_errors: &[&str]) {
        self.error_index = 0;
        self.errors = expected_errors.iter().map(|s| (*s).to_string()).collect();
    }

    // port: TestErrorManager#expectWarnings
    pub fn expect_warnings(&mut self, expected_warnings: &[&str]) {
        self.warning_index = 0;
        self.warnings = expected_warnings.iter().map(|s| (*s).to_string()).collect();
    }

    // port: TestErrorManager#println
    pub fn println(&mut self, _ast: &Ast, _level: CheckLevel, _error: &JSError) {
        // no-op
    }

    // port: TestErrorManager#printSummary
    fn print_summary(&mut self) {
        // no-op
    }

    // port: TestErrorManager#hasEncounteredAllErrors
    pub fn has_encountered_all_errors(&self) -> bool {
        self.error_index == self.errors.len()
    }

    // port: TestErrorManager#hasEncounteredAllWarnings
    pub fn has_encountered_all_warnings(&self) -> bool {
        self.warning_index == self.warnings.len()
    }
}

impl ErrorHandler for TestErrorManager {
    // port: TestErrorManager#report
    fn report(&mut self, level: CheckLevel, error: JSError) {
        self.base.report(level, error.clone());
        match level {
            CheckLevel::ERROR => {
                if self.error_index >= self.errors.len() {
                    panic!("Unexpected error: {error}");
                }
                let expected = &self.errors[self.error_index];
                self.error_index += 1;
                assert_eq!(expected.as_str(), error.description());
            }
            CheckLevel::WARNING => {
                // Java compares errorIndex here (sic).
                if self.error_index >= self.warnings.len() {
                    panic!("Unexpected warning: {error}");
                }
                let expected = &self.warnings[self.warning_index];
                self.warning_index += 1;
                assert_eq!(expected.as_str(), error.description());
            }
            CheckLevel::OFF => {
                // no-op
            }
        }
    }
}

impl ErrorManager for TestErrorManager {
    // port: BasicErrorManager#generateReport
    fn generate_report(&mut self, ast: &Ast) {
        for message in self.base.get_sorted_diagnostics() {
            self.println(ast, message.level, &message.error);
        }
        self.print_summary();
    }
    // port: SortingErrorManager#getErrorCount
    fn get_error_count(&self) -> i32 {
        self.base.get_error_count()
    }
    // port: SortingErrorManager#getWarningCount
    fn get_warning_count(&self) -> i32 {
        self.base.get_warning_count()
    }
    // port: SortingErrorManager#getErrors
    fn get_errors(&self) -> Vec<JSError> {
        self.base.get_errors()
    }
    // port: SortingErrorManager#getWarnings
    fn get_warnings(&self) -> Vec<JSError> {
        self.base.get_warnings()
    }
    // port: SortingErrorManager#setTypedPercent
    fn set_typed_percent(&mut self, typed_percent: f64) {
        self.base.set_typed_percent(typed_percent);
    }
    // port: SortingErrorManager#getTypedPercent
    fn get_typed_percent(&self) -> f64 {
        self.base.get_typed_percent()
    }
    // port: SortingErrorManager#hasHaltingErrors
    fn has_halting_errors(&self) -> bool {
        self.base.has_halting_errors()
    }
}

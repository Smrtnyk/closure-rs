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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/ThreadSafeDelegatingErrorManager.java.

use crate::{
    check_level::CheckLevel,
    conformance_config::{
        LibraryLevelNonAllowlistedConformanceViolationsBehavior, Requirement, RequirementScopeEntry,
    },
    error_handler::ErrorHandler,
    error_manager::ErrorManager,
    js_error::JSError,
};
use closure_rhino::node::Ast;
use std::sync::Mutex;
pub struct ThreadSafeDelegatingErrorManager {
    delegated: Mutex<Box<dyn ErrorManager>>,
}
impl ThreadSafeDelegatingErrorManager {
    // port: ThreadSafeDelegatingErrorManager#ThreadSafeDelegatingErrorManager
    pub fn new(delegated: Box<dyn ErrorManager>) -> Self {
        Self {
            delegated: Mutex::new(delegated),
        }
    }
    // port: ThreadSafeDelegatingErrorManager#report
    pub fn report(&self, level: CheckLevel, error: JSError) {
        self.delegated.lock().unwrap().report(level, error);
    }
    // port: ThreadSafeDelegatingErrorManager#generateReport
    pub fn generate_report(&self, ast: &Ast) {
        self.delegated.lock().unwrap().generate_report(ast);
    }
}
impl ErrorHandler for ThreadSafeDelegatingErrorManager {
    // port: ThreadSafeDelegatingErrorManager#report
    fn report(&mut self, level: CheckLevel, error: JSError) {
        self.delegated.lock().unwrap().report(level, error);
    }
}
impl ErrorManager for ThreadSafeDelegatingErrorManager {
    // port: ThreadSafeDelegatingErrorManager#generateReport
    fn generate_report(&mut self, ast: &Ast) {
        self.delegated.lock().unwrap().generate_report(ast)
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
    fn set_typed_percent(&mut self, value: f64) {
        self.delegated.lock().unwrap().set_typed_percent(value)
    }
    // port: ThreadSafeDelegatingErrorManager#getTypedPercent
    fn get_typed_percent(&self) -> f64 {
        self.delegated.lock().unwrap().get_typed_percent()
    }
    // port: ThreadSafeDelegatingErrorManager#hasHaltingErrors
    fn has_halting_errors(&self) -> bool {
        self.delegated.lock().unwrap().has_halting_errors()
    }
    // port: ThreadSafeDelegatingErrorManager#shouldReportConformanceViolation
    fn should_report_conformance_violation(
        &mut self,
        requirement: &Requirement,
        allowlist_entry: Option<&RequirementScopeEntry>,
        diagnostic: &JSError,
        behavior: LibraryLevelNonAllowlistedConformanceViolationsBehavior,
        is_allowlisted: bool,
    ) -> bool {
        self.delegated
            .lock()
            .unwrap()
            .should_report_conformance_violation(
                requirement,
                allowlist_entry,
                diagnostic,
                behavior,
                is_allowlisted,
            )
    }
}

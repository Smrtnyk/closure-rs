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
//   src/com/google/javascript/jscomp/ErrorManager.java.

use crate::{
    check_level::CheckLevel,
    conformance_config::{
        LibraryLevelNonAllowlistedConformanceViolationsBehavior, Requirement, RequirementScopeEntry,
    },
    error_handler::ErrorHandler,
    js_error::JSError,
};
use closure_rhino::node::Ast;
// port: ErrorManager#report (the redeclaration is inherited from ErrorHandler)
pub trait ErrorManager: ErrorHandler + Send {
    // port: ErrorManager#generateReport
    fn generate_report(&mut self, ast: &Ast);
    // port: ErrorManager#getErrorCount
    fn get_error_count(&self) -> i32;
    // port: ErrorManager#getWarningCount
    fn get_warning_count(&self) -> i32;
    // port: ErrorManager#getErrors
    fn get_errors(&self) -> Vec<JSError>;
    // port: ErrorManager#getWarnings
    fn get_warnings(&self) -> Vec<JSError>;
    // port: ErrorManager#setTypedPercent
    fn set_typed_percent(&mut self, typed_percent: f64);
    // port: ErrorManager#getTypedPercent
    fn get_typed_percent(&self) -> f64;
    // port: ErrorManager#hasHaltingErrors
    fn has_halting_errors(&self) -> bool {
        self.get_error_count() != 0
            && self
                .get_errors()
                .iter()
                .any(|error| error.get_type().level == CheckLevel::ERROR)
    }
    /// Return true if the conformance violation should be reported. This is called even if the
    /// violation is allowlisted and override implementations all can return true despite
    /// allowlisting (see the Java documentation: the allowlist check happens at the call site in
    /// `AbstractRule.report`).
    // port: ErrorManager#shouldReportConformanceViolation
    fn should_report_conformance_violation(
        &mut self,
        _requirement: &Requirement,
        _allowlist_entry: Option<&RequirementScopeEntry>,
        _diagnostic: &JSError,
        behavior: LibraryLevelNonAllowlistedConformanceViolationsBehavior,
        _is_allowlisted: bool,
    ) -> bool {
        behavior != LibraryLevelNonAllowlistedConformanceViolationsBehavior::RECORD_ONLY
    }
}

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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
pub struct ThreadSafeDelegatingErrorManager {
    delegated: Mutex<Box<dyn ErrorManager>>,
    /// Rust-only (D-025): set (under the `delegated` lock) when `hasHaltingErrors` found none,
    /// cleared (under the lock) by every call that may change the delegate. The compiler shares
    /// it, so its frequent `hasHaltingErrors` (CombinedCompilerPass asks at every node) needs no
    /// lock while nothing was reported since.
    no_halting_errors: Arc<AtomicBool>,
}
impl ThreadSafeDelegatingErrorManager {
    // port: ThreadSafeDelegatingErrorManager#ThreadSafeDelegatingErrorManager
    pub fn new(delegated: Box<dyn ErrorManager>) -> Self {
        Self {
            delegated: Mutex::new(delegated),
            no_halting_errors: Arc::new(AtomicBool::new(false)),
        }
    }
    /// Rust-only: the shared "no halting errors" flag, see `no_halting_errors`.
    pub(crate) fn no_halting_errors_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.no_halting_errors)
    }
    /// Rust-only: locks the delegate for a call that may change it.
    fn delegate_for_change(&self) -> std::sync::MutexGuard<'_, Box<dyn ErrorManager>> {
        let guard = self.delegated.lock().unwrap();
        self.no_halting_errors.store(false, Ordering::Relaxed);
        guard
    }
    // port: ThreadSafeDelegatingErrorManager#report
    pub fn report(&self, level: CheckLevel, error: JSError) {
        self.delegate_for_change().report(level, error);
    }
    // port: ThreadSafeDelegatingErrorManager#generateReport
    pub fn generate_report(&self, ast: &Ast) {
        self.delegate_for_change().generate_report(ast);
    }
}
impl ErrorHandler for ThreadSafeDelegatingErrorManager {
    // port: ThreadSafeDelegatingErrorManager#report
    fn report(&mut self, level: CheckLevel, error: JSError) {
        self.delegate_for_change().report(level, error);
    }
}
impl ErrorManager for ThreadSafeDelegatingErrorManager {
    // port: ThreadSafeDelegatingErrorManager#generateReport
    fn generate_report(&mut self, ast: &Ast) {
        self.delegate_for_change().generate_report(ast)
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
        self.delegate_for_change().set_typed_percent(value)
    }
    // port: ThreadSafeDelegatingErrorManager#getTypedPercent
    fn get_typed_percent(&self) -> f64 {
        self.delegated.lock().unwrap().get_typed_percent()
    }
    // port: ThreadSafeDelegatingErrorManager#hasHaltingErrors
    fn has_halting_errors(&self) -> bool {
        let delegated = self.delegated.lock().unwrap();
        let halting = delegated.has_halting_errors();
        if !halting {
            self.no_halting_errors.store(true, Ordering::Relaxed);
        }
        halting
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
        self.delegate_for_change()
            .should_report_conformance_violation(
                requirement,
                allowlist_entry,
                diagnostic,
                behavior,
                is_allowlisted,
            )
    }
}
